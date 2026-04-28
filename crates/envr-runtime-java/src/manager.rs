//! Install / uninstall / `current` for Java JDKs (Adoptium, Azul Zulu, Dragonwell, etc.) under the envr data root.

use crate::index::{
    adoptium_arch, adoptium_assets_version_range_segment, adoptium_os, blocking_http_client,
    load_java_index, normalize_openjdk_version_label, resolve_java_version,
};
use crate::vendor::JavaVendor;
use envr_config::settings::JavaDownloadSource;
use envr_domain::installer::{SpecDrivenInstaller, install_via_version_spec};
use envr_domain::runtime::{InstallRequest, RuntimeVersion, VersionSpec};
use envr_download::{blocking::download_url_to_path_resumable_with_headers, checksum, extract};
use envr_error::{EnvrError, EnvrResult, ErrorCode};
use envr_platform::links::{LinkType, ensure_link};
use serde::Deserialize;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64},
    },
};

fn parse_major_spec(spec: &str) -> Option<u32> {
    let t = spec.trim().strip_prefix('v').unwrap_or(spec.trim());
    let major = t.split('.').next()?;
    major.parse::<u32>().ok()
}

fn is_major_only_java_label(label: &str) -> bool {
    let t = label.trim().strip_prefix('v').unwrap_or(label.trim());
    let no_plus = t.split('+').next().unwrap_or(t).trim();
    let no_lts = no_plus
        .strip_suffix("-LTS")
        .or_else(|| no_plus.strip_suffix("-lts"))
        .unwrap_or(no_plus)
        .trim();
    !no_lts.contains('.') && no_lts.parse::<u32>().is_ok()
}

fn parse_java_major_from_label(label: &str) -> EnvrResult<u32> {
    let t = label.trim().strip_prefix('v').unwrap_or(label.trim());
    let core = t.split('+').next().unwrap_or(t).trim();
    let core = core
        .strip_suffix("-LTS")
        .or_else(|| core.strip_suffix("-lts"))
        .unwrap_or(core)
        .trim();
    let major_str = core
        .split('.')
        .next()
        .ok_or_else(|| EnvrError::Validation(format!("bad java version label: {label:?}")))?;
    major_str
        .parse::<u32>()
        .map_err(|_| EnvrError::Validation(format!("bad java major in label: {label:?}")))
}

fn java_label_triplet_and_build(label: &str) -> Option<(u32, u32, u32, Option<u32>)> {
    let t = label.trim().strip_prefix('v').unwrap_or(label.trim());
    let (core, plus) = t
        .split_once('+')
        .map(|(a, b)| (a.trim(), Some(b.trim())))
        .unwrap_or((t.trim(), None));
    let core = core
        .strip_suffix("-LTS")
        .or_else(|| core.strip_suffix("-lts"))
        .unwrap_or(core)
        .trim();
    let build = plus.and_then(|p| p.parse::<u32>().ok());
    let parts: Vec<&str> = core.split('.').collect();
    let (maj, min, sec) = match parts.as_slice() {
        [a] => (a.parse().ok()?, 0u32, 0u32),
        [a, b] => (a.parse().ok()?, b.parse().ok()?, 0u32),
        [a, b, c] => (a.parse().ok()?, b.parse().ok()?, c.parse().ok()?),
        _ => return None,
    };
    Some((maj, min, sec, build))
}

#[derive(Debug, Deserialize)]
struct ZuluPkgRow {
    name: String,
    download_url: String,
    #[serde(default)]
    java_version: Vec<u32>,
    #[serde(default)]
    openjdk_build_number: Option<u32>,
}

fn zulu_headless_jdk_row(p: &ZuluPkgRow) -> bool {
    let n = p.name.to_ascii_lowercase();
    n.contains("-ca-jdk") && !n.contains("crac") && !n.contains("-fx-")
}

fn zulu_row_sort_key(p: &ZuluPkgRow) -> (u32, u32, u32, u32) {
    let j = &p.java_version;
    let maj = *j.first().unwrap_or(&0);
    let min = *j.get(1).unwrap_or(&0);
    let sec = *j.get(2).unwrap_or(&0);
    let b = p.openjdk_build_number.unwrap_or(0);
    (maj, min, sec, b)
}

fn zulu_fetch_packages_json(
    client: &reqwest::blocking::Client,
    major: u32,
    os: &str,
    arch: &str,
    page_size: u32,
) -> EnvrResult<Vec<ZuluPkgRow>> {
    let azul_os = match os {
        "windows" => "windows",
        "linux" => "linux",
        "mac" => "macos",
        _ => {
            return Err(EnvrError::Platform(format!(
                "unsupported OS for Zulu metadata: {os}"
            )));
        }
    };
    let azul_arch = match arch {
        "x64" => "x86-64",
        "aarch64" => "arm64",
        "x32" => "x86",
        _ => {
            return Err(EnvrError::Platform(format!(
                "unsupported CPU arch for Zulu metadata: {arch}"
            )));
        }
    };
    let url = format!(
        "https://api.azul.com/metadata/v1/zulu/packages/?java_version={major}&os={azul_os}&arch={azul_arch}\
         &archive_type=zip&java_package_type=jdk&release_status=ga&availability_types=ca&page_size={page_size}"
    );
    let r = client.get(&url).send().map_err(|e| {
        EnvrError::with_source(ErrorCode::Download, format!("request failed for {url}"), e)
    })?;
    if !r.status().is_success() {
        return Err(EnvrError::Download(format!("GET {url} -> {}", r.status())));
    }
    let body = r.text().map_err(|e| {
        EnvrError::with_source(
            ErrorCode::Download,
            format!("read body failed for {url}"),
            e,
        )
    })?;
    serde_json::from_str(&body)
        .map_err(|e| EnvrError::with_source(ErrorCode::Validation, "invalid zulu packages json", e))
}

fn zulu_preferred_download_url(
    client: &reqwest::blocking::Client,
    major: u32,
    os: &str,
    arch: &str,
) -> EnvrResult<String> {
    let mut rows = zulu_fetch_packages_json(client, major, os, arch, 100)?;
    rows.retain(zulu_headless_jdk_row);
    rows.sort_by_key(|b| std::cmp::Reverse(zulu_row_sort_key(b)));
    let Some(best) = rows.first() else {
        return Err(EnvrError::Validation(format!(
            "no Zulu JDK package found for Java {major} on {os}-{arch}"
        )));
    };
    Ok(best.download_url.clone())
}

fn zulu_download_url_matching_label(
    client: &reqwest::blocking::Client,
    label: &str,
    os: &str,
    arch: &str,
) -> EnvrResult<String> {
    let major = parse_java_major_from_label(label)?;
    let mut rows = zulu_fetch_packages_json(client, major, os, arch, 100)?;
    rows.retain(zulu_headless_jdk_row);
    let Some((want_maj, want_min, want_sec, want_build)) = java_label_triplet_and_build(label)
    else {
        return zulu_preferred_download_url(client, major, os, arch);
    };
    let mut filtered: Vec<&ZuluPkgRow> = rows
        .iter()
        .filter(|p| {
            let j = &p.java_version;
            let maj = *j.first().unwrap_or(&0);
            let min = *j.get(1).unwrap_or(&0);
            let sec = *j.get(2).unwrap_or(&0);
            maj == want_maj && min == want_min && sec == want_sec
        })
        .collect();
    if let Some(b) = want_build {
        let with_build: Vec<&ZuluPkgRow> = filtered
            .iter()
            .copied()
            .filter(|p| p.openjdk_build_number == Some(b))
            .collect();
        if !with_build.is_empty() {
            filtered = with_build;
        }
    }
    if filtered.is_empty() {
        return Err(EnvrError::Validation(format!(
            "no Zulu package matches label {label:?} on {os}-{arch}"
        )));
    }
    filtered.sort_by_key(|b| std::cmp::Reverse(zulu_row_sort_key(b)));
    Ok(filtered[0].download_url.clone())
}

#[derive(Debug, Deserialize)]
struct GhRelease {
    assets: Vec<GhAsset>,
}

#[derive(Debug, Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
}

fn dragonwell_latest_download_url(
    client: &reqwest::blocking::Client,
    major: u32,
    host_os: &str,
    host_arch: &str,
) -> EnvrResult<String> {
    let suffix = match (host_os, host_arch) {
        ("windows", "x86_64") => "_x64_windows.zip",
        ("linux", "x86_64") => "_x64_linux.tar.gz",
        ("linux", "aarch64") => "_aarch64_linux.tar.gz",
        _ => {
            return Err(EnvrError::Platform(format!(
                "Alibaba Dragonwell has no published build for {host_os}-{host_arch} in this channel (e.g. use Temurin on macOS)"
            )));
        }
    };
    let repo = format!("dragonwell{major}");
    let api_url = format!("https://api.github.com/repos/dragonwell-project/{repo}/releases/latest");
    let r = client
        .get(&api_url)
        .header(reqwest::header::ACCEPT, "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .map_err(|e| {
            EnvrError::with_source(
                ErrorCode::Download,
                format!("request failed for {api_url}"),
                e,
            )
        })?;
    if !r.status().is_success() {
        return Err(EnvrError::Download(format!(
            "GET {api_url} -> {}",
            r.status()
        )));
    }
    let body = r.text().map_err(|e| {
        EnvrError::with_source(
            ErrorCode::Download,
            format!("read body failed for {api_url}"),
            e,
        )
    })?;
    let parsed: GhRelease = serde_json::from_str(&body).map_err(|e| {
        EnvrError::with_source(
            ErrorCode::Validation,
            "invalid dragonwell latest release json",
            e,
        )
    })?;

    let mut candidates: Vec<&GhAsset> = parsed
        .assets
        .iter()
        .filter(|a| {
            a.name.ends_with(suffix)
                && !a.name.ends_with(".sha256.txt")
                && !a.name.ends_with(".json")
                && !a.name.contains("-sbom")
        })
        .collect();
    if candidates.is_empty() {
        return Err(EnvrError::Validation(format!(
            "no Dragonwell asset matching {suffix:?} in latest {repo} GitHub release"
        )));
    }
    candidates.sort_by_key(|a| {
        let tier = if a.name.contains("Standard") {
            0u8
        } else if a.name.contains("Extended") {
            1u8
        } else {
            2u8
        };
        (tier, &a.name)
    });
    Ok(candidates[0].browser_download_url.clone())
}

fn cache_path_for_major_vendor_url(
    cache_dir: &Path,
    vendor_dir: &str,
    major: u32,
    artifact_url: &str,
) -> PathBuf {
    let ext = if artifact_url.ends_with(".tar.gz") {
        "tar.gz"
    } else {
        "zip"
    };
    cache_dir.join(format!("latest-{vendor_dir}-{major}.{ext}"))
}

/// Layout: `{runtime_root}/runtimes/java/versions/<openjdk_version>/...`, `current` → version dir,
/// `JAVA_HOME` file with the resolved absolute JDK root (for shells).
#[derive(Debug, Clone)]
pub struct JavaPaths {
    runtime_root: PathBuf,
    distro_dir: String,
}

impl JavaPaths {
    pub fn new(runtime_root: PathBuf, distro_dir: impl Into<String>) -> Self {
        Self {
            runtime_root,
            distro_dir: distro_dir.into(),
        }
    }

    pub fn java_home(&self) -> PathBuf {
        self.runtime_root.join("runtimes").join("java")
    }

    pub fn versions_dir(&self) -> PathBuf {
        self.java_home().join("versions").join(&self.distro_dir)
    }

    pub fn current_link(&self) -> PathBuf {
        self.java_home().join("current")
    }

    pub fn cache_dir(&self) -> PathBuf {
        self.runtime_root.join("cache").join("java")
    }

    pub fn version_dir(&self, version_label: &str) -> PathBuf {
        self.versions_dir().join(version_label)
    }

    /// Single-line file: absolute `JAVA_HOME` for the `current` JDK (UTF-8).
    pub fn java_home_export_file(&self) -> PathBuf {
        self.java_home().join("JAVA_HOME")
    }
}

pub fn java_installation_valid(home: &Path) -> bool {
    #[cfg(windows)]
    {
        home.join("bin").join("java.exe").is_file()
    }
    #[cfg(not(windows))]
    {
        home.join("bin").join("java").is_file()
    }
}

pub fn list_installed_versions(paths: &JavaPaths) -> EnvrResult<Vec<RuntimeVersion>> {
    let dir = paths.versions_dir();
    if !dir.is_dir() {
        return Ok(vec![]);
    }
    let mut out = Vec::new();
    for e in fs::read_dir(&dir).map_err(EnvrError::from)? {
        let e = e.map_err(EnvrError::from)?;
        if !e.file_type().map_err(EnvrError::from)?.is_dir() {
            continue;
        }
        let p = e.path();
        if java_installation_valid(&p) {
            out.push(RuntimeVersion(e.file_name().to_string_lossy().into_owned()));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

pub fn read_current(paths: &JavaPaths) -> EnvrResult<Option<RuntimeVersion>> {
    let cur = paths.current_link();
    if !cur.exists() {
        return Ok(None);
    }
    let target = match fs::read_link(&cur) {
        Ok(t) => t,
        Err(_) => return Ok(None),
    };
    let resolved = if target.is_relative() {
        cur.parent().map(|p| p.join(&target)).unwrap_or(target)
    } else {
        target
    };
    let resolved_abs = fs::canonicalize(&resolved).map_err(EnvrError::from)?;
    let distro_root = match fs::canonicalize(paths.versions_dir()) {
        Ok(p) => p,
        // Distro folder not created yet => no install/current for this distro.
        Err(_) => return Ok(None),
    };
    // Distro-scoped view: only treat `current` as active when it points under
    // `.../java/versions/<distro>/...`.
    if !resolved_abs.starts_with(&distro_root) {
        return Ok(None);
    }
    let name = resolved_abs
        .file_name()
        .ok_or_else(|| EnvrError::Runtime("invalid java current link".into()))?
        .to_string_lossy()
        .into_owned();
    Ok(Some(RuntimeVersion(name)))
}

fn remove_path_if_exists(path: &Path) {
    if fs::symlink_metadata(path).is_err() {
        return;
    }
    if fs::remove_file(path).is_ok() {
        return;
    }
    if fs::remove_dir(path).is_ok() {
        return;
    }
    let _ = fs::remove_dir_all(path);
}

/// Writes [`JavaPaths::java_home_export_file`] from the canonical `current` link target, or removes it.
pub fn sync_java_home_export(paths: &JavaPaths) -> EnvrResult<()> {
    let marker = paths.java_home_export_file();
    let current = paths.current_link();
    if !current.exists() {
        let _ = fs::remove_file(&marker);
        sync_user_java_home(None)?;
        return Ok(());
    }
    let abs = fs::canonicalize(&current).map_err(EnvrError::from)?;
    if let Some(parent) = marker.parent() {
        fs::create_dir_all(parent).map_err(EnvrError::from)?;
    }
    envr_platform::fs_atomic::write_atomic(&marker, format!("{}\n", abs.display()).as_bytes())
        .map_err(EnvrError::from)?;
    sync_user_java_home(Some(&abs))?;
    Ok(())
}

fn sync_user_java_home(home: Option<&Path>) -> EnvrResult<()> {
    #[cfg(windows)]
    {
        use std::process::Command;
        let value = home
            .map(envr_platform::path_norm::normalize_fs_path_string_lossy)
            .unwrap_or_default();
        let out = Command::new("setx")
            .args(["JAVA_HOME", &value])
            .output()
            .map_err(EnvrError::from)?;
        if !out.status.success() {
            return Err(EnvrError::Runtime(format!(
                "setx JAVA_HOME failed: {}",
                String::from_utf8_lossy(&out.stderr)
            )));
        }
    }
    #[cfg(not(windows))]
    {
        let _ = home;
    }
    Ok(())
}

pub fn read_java_home_export(paths: &JavaPaths) -> EnvrResult<Option<PathBuf>> {
    let marker = paths.java_home_export_file();
    if !marker.is_file() {
        return Ok(None);
    }
    let s = fs::read_to_string(&marker).map_err(EnvrError::from)?;
    let line = s.lines().next().unwrap_or("").trim();
    if line.is_empty() {
        return Ok(None);
    }
    Ok(Some(PathBuf::from(line)))
}

#[derive(Debug, Deserialize)]
struct PackageInfo {
    link: String,
    checksum: String,
    name: String,
}

#[derive(Debug, Deserialize)]
struct ReleaseBinary {
    os: String,
    architecture: String,
    package: PackageInfo,
}

#[derive(Debug, Deserialize)]
struct VersionData {
    openjdk_version: String,
}

#[derive(Debug, Deserialize)]
struct AssetRelease {
    binaries: Vec<ReleaseBinary>,
    version_data: VersionData,
}

fn fetch_json<T: serde::de::DeserializeOwned>(
    client: &reqwest::blocking::Client,
    url: &str,
) -> EnvrResult<T> {
    let r = client.get(url).send().map_err(|e| {
        EnvrError::with_source(ErrorCode::Download, format!("request failed for {url}"), e)
    })?;
    if !r.status().is_success() {
        return Err(EnvrError::Download(format!("GET {url} -> {}", r.status())));
    }
    let body = r.text().map_err(|e| {
        EnvrError::with_source(
            ErrorCode::Download,
            format!("read body failed for {url}"),
            e,
        )
    })?;
    serde_json::from_str(&body)
        .map_err(|e| EnvrError::with_source(ErrorCode::Validation, "invalid json response", e))
}

fn pick_release<'a>(
    releases: &'a [AssetRelease],
    want_label: &str,
) -> EnvrResult<&'a AssetRelease> {
    let want = normalize_openjdk_version_label(want_label);
    releases
        .iter()
        .find(|r| normalize_openjdk_version_label(&r.version_data.openjdk_version) == want)
        .ok_or_else(|| {
            EnvrError::Validation(format!(
                "no adoptium binary for resolved jdk {want_label:?} on this platform"
            ))
        })
}

fn pick_binary<'a>(
    release: &'a AssetRelease,
    adoptium_os: &str,
    adoptium_arch: &str,
) -> EnvrResult<&'a PackageInfo> {
    let b = release
        .binaries
        .iter()
        .find(|b| b.os == adoptium_os && b.architecture == adoptium_arch)
        .ok_or_else(|| {
            EnvrError::Validation(format!(
                "no jdk package for {adoptium_os}-{adoptium_arch} in release {}",
                release.version_data.openjdk_version
            ))
        })?;
    Ok(&b.package)
}

/// Parent directory URL for anti-hotlink checks on some domestic mirrors.
fn mirror_download_referer(url: &str) -> Option<String> {
    const HOSTS: &[&str] = &[
        "mirrors.tuna.tsinghua.edu.cn",
        "mirrors.ustc.edu.cn",
        "mirrors.huaweicloud.com",
    ];
    if !HOSTS.iter().any(|h| url.contains(h)) {
        return None;
    }
    let (dir, _file) = url.rsplit_once('/')?;
    if dir.is_empty() || !dir.starts_with("http") {
        return None;
    }
    Some(format!("{}/", dir.trim_end_matches('/')))
}

fn download_to_path(
    client: &reqwest::blocking::Client,
    url: &str,
    path: &Path,
    progress_downloaded: Option<&Arc<AtomicU64>>,
    progress_total: Option<&Arc<AtomicU64>>,
    cancel: Option<&Arc<AtomicBool>>,
    referer: Option<&str>,
) -> EnvrResult<()> {
    let mut headers = reqwest::header::HeaderMap::new();
    if let Some(r) = referer {
        let hv = reqwest::header::HeaderValue::from_str(r).map_err(|e| {
            EnvrError::with_source(ErrorCode::Validation, "invalid Java referer header", e)
        })?;
        headers.insert(reqwest::header::REFERER, hv);
    }
    download_url_to_path_resumable_with_headers(
        client,
        url,
        path,
        progress_downloaded,
        progress_total,
        cancel,
        Some(&headers),
    )
}

/// Temurin archives ship a single top-level `jdk-...` directory; promote it to `final_dir`.
pub fn promote_single_root_dir(staging: &Path, final_dir: &Path) -> EnvrResult<()> {
    use envr_platform::install_layout;

    let mut iter = fs::read_dir(staging).map_err(EnvrError::from)?;
    let first = iter
        .next()
        .transpose()
        .map_err(EnvrError::from)?
        .ok_or_else(|| EnvrError::Validation("empty jdk archive".into()))?;
    if iter.next().transpose().map_err(EnvrError::from)?.is_some() {
        return Err(EnvrError::Validation(
            "expected exactly one root directory in jdk archive".into(),
        ));
    }
    let inner = first.path();
    if !inner.is_dir() {
        return Err(EnvrError::Validation(
            "expected jdk archive root to be a directory".into(),
        ));
    }
    install_layout::ensure_final_parent(final_dir)?;
    let staging_final = install_layout::sibling_staging_path(final_dir)?;
    install_layout::remove_if_exists(&staging_final)?;

    fs::rename(&inner, &staging_final).map_err(EnvrError::from)?;

    if !java_installation_valid(&staging_final) {
        let _ = fs::remove_dir_all(&staging_final);
        return Err(EnvrError::Validation(
            "extracted jdk layout missing java binary".into(),
        ));
    }

    install_layout::commit_staging_dir(&staging_final, final_dir)?;
    Ok(())
}

pub struct JavaManager {
    pub paths: JavaPaths,
    api_base: String,
    vendor: JavaVendor,
    download_source: JavaDownloadSource,
    client: reqwest::blocking::Client,
}

impl JavaManager {
    fn supported_lts_majors(vendor: JavaVendor) -> &'static [u32] {
        match vendor {
            JavaVendor::EclipseTemurin | JavaVendor::OpenJdk => &[8, 11, 17, 21, 25],
            JavaVendor::OracleOpenJdk => &[17, 21, 25],
            JavaVendor::AmazonCorretto => &[8, 11, 17, 21],
            JavaVendor::Microsoft => &[11, 17, 21, 25],
            JavaVendor::OracleJdk => &[21, 25],
            JavaVendor::AzulZulu | JavaVendor::AlibabaDragonwell => &[8, 11, 17, 21, 25],
        }
    }

    pub fn try_new(
        runtime_root: PathBuf,
        api_base: String,
        vendor: JavaVendor,
        download_source: JavaDownloadSource,
    ) -> EnvrResult<Self> {
        Ok(Self {
            paths: JavaPaths::new(runtime_root, vendor.dir_name()),
            api_base,
            vendor,
            download_source,
            client: blocking_http_client()?,
        })
    }

    fn assets_url(&self, range_segment: &str, os: &str, arch: &str) -> String {
        let base = self.api_base.trim_end_matches('/');
        let v = self.vendor.adoptium_vendor_param();
        format!(
            "{base}/v3/assets/version/{range_segment}\
             ?project=jdk&release_type=ga&vendor={v}\
             &os={os}&architecture={arch}&image_type=jdk&jvm_impl=hotspot&heap_size=normal\
             &sort_method=DATE&sort_order=DESC"
        )
    }

    fn first_matching_temurin_filename(html: &str, major: u32, arch: &str) -> Option<String> {
        let needle = format!("OpenJDK{major}U-jdk_{arch}_windows_hotspot_");
        html.split('"')
            .find(|s| s.starts_with(&needle) && s.ends_with(".zip"))
            .map(|s| s.to_string())
    }

    fn fetch_temurin_mirror_candidates(&self, major: u32, arch: &str) -> Vec<String> {
        let mut out = Vec::new();
        let tuna_dir =
            format!("https://mirrors.tuna.tsinghua.edu.cn/Adoptium/{major}/jdk/{arch}/windows/");
        if let Ok(r) = self.client.get(&tuna_dir).send()
            && r.status().is_success()
            && let Ok(body) = r.text()
            && let Some(name) = Self::first_matching_temurin_filename(&body, major, arch)
        {
            out.push(format!("{tuna_dir}{name}"));
        }

        // USTC layout note: major 25 may be unavailable.
        let ustc_dir = format!(
            "https://mirrors.ustc.edu.cn/adoptium/releases/temurin{major}-binaries/LatestRelease/"
        );
        if let Ok(r) = self.client.get(&ustc_dir).send()
            && r.status().is_success()
            && let Ok(body) = r.text()
            && let Some(name) = Self::first_matching_temurin_filename(&body, major, arch)
        {
            out.push(format!("{ustc_dir}{name}"));
        }
        out
    }

    fn latest_binary_url_candidates_for_major(
        &self,
        major: u32,
        os: &str,
        arch: &str,
    ) -> Vec<String> {
        if matches!(
            self.vendor,
            JavaVendor::AzulZulu | JavaVendor::AlibabaDragonwell
        ) {
            return Vec::new();
        }
        let official = match self.vendor {
            JavaVendor::EclipseTemurin | JavaVendor::OpenJdk => Some(format!(
                "{}/v3/binary/latest/{major}/ga/{os}/{arch}/jdk/hotspot/normal/eclipse",
                self.api_base.trim_end_matches('/')
            )),
            JavaVendor::OracleOpenJdk => {
                if os != "windows" {
                    None
                } else {
                    Some(format!(
                        "https://download.java.net/java/GA/jdk{major}/latest/GPL/openjdk-{major}_windows-{arch}_bin.zip"
                    ))
                }
            }
            JavaVendor::AmazonCorretto => {
                if os != "windows" {
                    None
                } else {
                    Some(format!(
                        "https://corretto.aws/downloads/latest/amazon-corretto-{major}-{arch}-windows-jdk.zip"
                    ))
                }
            }
            JavaVendor::Microsoft => {
                if os != "windows" {
                    None
                } else {
                    Some(format!(
                        "https://aka.ms/download-jdk/microsoft-jdk-{major}-windows-{arch}.zip"
                    ))
                }
            }
            JavaVendor::OracleJdk => {
                if os != "windows" {
                    return Vec::new();
                }
                Some(format!(
                    "https://download.oracle.com/java/{major}/latest/jdk-{major}_windows-{arch}_bin.zip"
                ))
            }
            // Returned early at top of `latest_binary_url_candidates_for_major`.
            JavaVendor::AzulZulu | JavaVendor::AlibabaDragonwell => None,
        };

        let mut out = Vec::new();
        if self.download_source == JavaDownloadSource::Domestic {
            match self.vendor {
                JavaVendor::EclipseTemurin | JavaVendor::OpenJdk => {
                    if os == "windows" {
                        out.push(format!(
                            "https://mirrors.huaweicloud.com/openjdk/{major}/openjdk-{major}_windows-{arch}_bin.zip"
                        ));
                    }
                    out.extend(self.fetch_temurin_mirror_candidates(major, arch));
                }
                JavaVendor::OracleOpenJdk => {
                    out.push(format!(
                        "https://mirrors.huaweicloud.com/openjdk/{major}/openjdk-{major}_windows-{arch}_bin.zip"
                    ));
                }
                _ => {}
            }
        }
        if let Some(u) = official {
            out.push(u);
        }
        out
    }

    fn promote_cached_archive_to_major_version_dir(
        &self,
        major: u32,
        cache_file: &Path,
    ) -> EnvrResult<RuntimeVersion> {
        let staging_parent = self.paths.cache_dir().join(format!("extract-{major}"));
        fs::create_dir_all(&staging_parent).map_err(EnvrError::from)?;
        let staging = tempfile::tempdir_in(&staging_parent).map_err(EnvrError::from)?;
        extract::extract_archive(cache_file, staging.path())?;

        let final_dir = self.paths.version_dir(&major.to_string());
        promote_single_root_dir(staging.path(), &final_dir)?;
        Ok(RuntimeVersion(major.to_string()))
    }

    fn install_latest_major(
        &self,
        major: u32,
        progress_downloaded: Option<&Arc<AtomicU64>>,
        progress_total: Option<&Arc<AtomicU64>>,
        cancel: Option<&Arc<AtomicBool>>,
    ) -> EnvrResult<RuntimeVersion> {
        if !Self::supported_lts_majors(self.vendor).contains(&major) {
            return Err(EnvrError::Validation(format!(
                "selected Java distribution does not support LTS major {major}"
            )));
        }
        let host_os = std::env::consts::OS;
        let host_arch = std::env::consts::ARCH;
        let os = adoptium_os(host_os)?;
        let arch = adoptium_arch(host_arch)?;

        fs::create_dir_all(self.paths.cache_dir()).map_err(EnvrError::from)?;

        match self.vendor {
            JavaVendor::AzulZulu => {
                let url = zulu_preferred_download_url(&self.client, major, os, arch)?;
                let cache_file = cache_path_for_major_vendor_url(
                    &self.paths.cache_dir(),
                    self.vendor.dir_name(),
                    major,
                    &url,
                );
                let referer = mirror_download_referer(&url);
                download_to_path(
                    &self.client,
                    &url,
                    &cache_file,
                    progress_downloaded,
                    progress_total,
                    cancel,
                    referer.as_deref(),
                )?;
                return self.promote_cached_archive_to_major_version_dir(major, &cache_file);
            }
            JavaVendor::AlibabaDragonwell => {
                let url = dragonwell_latest_download_url(&self.client, major, host_os, host_arch)?;
                let cache_file = cache_path_for_major_vendor_url(
                    &self.paths.cache_dir(),
                    self.vendor.dir_name(),
                    major,
                    &url,
                );
                let referer = mirror_download_referer(&url);
                download_to_path(
                    &self.client,
                    &url,
                    &cache_file,
                    progress_downloaded,
                    progress_total,
                    cancel,
                    referer.as_deref(),
                )?;
                return self.promote_cached_archive_to_major_version_dir(major, &cache_file);
            }
            _ => {}
        }

        let urls = self.latest_binary_url_candidates_for_major(major, os, arch);
        if urls.is_empty() {
            return Err(EnvrError::Platform(format!(
                "java vendor {:?} unsupported on {os}-{arch}",
                self.vendor
            )));
        }

        let mut failures: Vec<String> = Vec::new();
        let mut chosen_cache: Option<PathBuf> = None;
        for url in &urls {
            let cache_file = cache_path_for_major_vendor_url(
                &self.paths.cache_dir(),
                self.vendor.dir_name(),
                major,
                url,
            );
            let referer = mirror_download_referer(url);
            match download_to_path(
                &self.client,
                url,
                &cache_file,
                progress_downloaded,
                progress_total,
                cancel,
                referer.as_deref(),
            ) {
                Ok(()) => {
                    chosen_cache = Some(cache_file);
                    break;
                }
                Err(e) => failures.push(format!("{url} -> {e}")),
            }
        }
        let Some(cache_file) = chosen_cache else {
            return Err(EnvrError::Download(format!(
                "all download attempts failed (mirrors only for Temurin/Oracle OpenJDK). Per-URL: {}. \
                 If this was fast (<1–2 min), check proxy/VPN/firewall or try Java 下载源「官方」; \
                 broken system HTTP proxies sometimes break rustls clients.",
                failures.join(" | ")
            )));
        };

        self.promote_cached_archive_to_major_version_dir(major, &cache_file)
    }

    fn fetch_releases_for_label(
        &self,
        openjdk_version_label: &str,
        adoptium_os: &str,
        adoptium_arch: &str,
    ) -> EnvrResult<Vec<AssetRelease>> {
        let seg = adoptium_assets_version_range_segment(openjdk_version_label)?;
        let url = self.assets_url(&seg, adoptium_os, adoptium_arch);
        fetch_json(&self.client, &url)
    }

    pub fn install_for_spec(
        &self,
        spec: &VersionSpec,
        progress_downloaded: Option<&Arc<AtomicU64>>,
        progress_total: Option<&Arc<AtomicU64>>,
        cancel: Option<&Arc<AtomicBool>>,
    ) -> EnvrResult<RuntimeVersion> {
        if let Some(major) = parse_major_spec(&spec.0)
            && Self::supported_lts_majors(self.vendor).contains(&major)
        {
            return self.install_latest_major(major, progress_downloaded, progress_total, cancel);
        }
        let index = load_java_index(
            &self.client,
            &self.api_base,
            self.vendor,
            std::env::consts::OS,
            std::env::consts::ARCH,
        )?;
        let label = resolve_java_version(&index, &spec.0)?;
        self.install_resolved_label(&label, progress_downloaded, progress_total, cancel)
    }

    fn install_zulu_resolved_label(
        &self,
        label: &str,
        progress_downloaded: Option<&Arc<AtomicU64>>,
        progress_total: Option<&Arc<AtomicU64>>,
        cancel: Option<&Arc<AtomicBool>>,
    ) -> EnvrResult<RuntimeVersion> {
        let host_os = std::env::consts::OS;
        let host_arch = std::env::consts::ARCH;
        let os = adoptium_os(host_os)?;
        let arch = adoptium_arch(host_arch)?;

        if is_major_only_java_label(label) {
            let major = parse_java_major_from_label(label)?;
            if Self::supported_lts_majors(self.vendor).contains(&major) {
                return self.install_latest_major(
                    major,
                    progress_downloaded,
                    progress_total,
                    cancel,
                );
            }
        }

        let major = parse_java_major_from_label(label)?;
        if !Self::supported_lts_majors(self.vendor).contains(&major) {
            return Err(EnvrError::Validation(format!(
                "Azul Zulu: unsupported Java major {major}"
            )));
        }
        let url = zulu_download_url_matching_label(&self.client, label, os, arch)?;
        fs::create_dir_all(self.paths.cache_dir()).map_err(EnvrError::from)?;
        let cache_file = cache_path_for_major_vendor_url(
            &self.paths.cache_dir(),
            self.vendor.dir_name(),
            major,
            &url,
        );
        let referer = mirror_download_referer(&url);
        download_to_path(
            &self.client,
            &url,
            &cache_file,
            progress_downloaded,
            progress_total,
            cancel,
            referer.as_deref(),
        )?;
        self.promote_cached_archive_to_major_version_dir(major, &cache_file)
    }

    /// Installs a resolved `openjdk_version` label: Adoptium assets for most vendors; Zulu / Dragonwell use their own endpoints.
    pub fn install_resolved_label(
        &self,
        openjdk_version_label: &str,
        progress_downloaded: Option<&Arc<AtomicU64>>,
        progress_total: Option<&Arc<AtomicU64>>,
        cancel: Option<&Arc<AtomicBool>>,
    ) -> EnvrResult<RuntimeVersion> {
        match self.vendor {
            JavaVendor::AlibabaDragonwell => {
                let major = parse_java_major_from_label(openjdk_version_label)?;
                if !Self::supported_lts_majors(self.vendor).contains(&major) {
                    return Err(EnvrError::Validation(format!(
                        "Alibaba Dragonwell: unsupported Java major {major}"
                    )));
                }
                return self.install_latest_major(
                    major,
                    progress_downloaded,
                    progress_total,
                    cancel,
                );
            }
            JavaVendor::AzulZulu => {
                return self.install_zulu_resolved_label(
                    openjdk_version_label,
                    progress_downloaded,
                    progress_total,
                    cancel,
                );
            }
            _ => {}
        }

        let host_os = std::env::consts::OS;
        let host_arch = std::env::consts::ARCH;
        let os = adoptium_os(host_os)?;
        let arch = adoptium_arch(host_arch)?;

        let releases = self.fetch_releases_for_label(openjdk_version_label, os, arch)?;
        let release = pick_release(&releases, openjdk_version_label)?;
        let pkg = pick_binary(release, os, arch)?;

        let version_key = release.version_data.openjdk_version.clone();

        fs::create_dir_all(self.paths.cache_dir()).map_err(EnvrError::from)?;
        let cache_file = self.paths.cache_dir().join(&version_key).join(&pkg.name);
        let referer = mirror_download_referer(&pkg.link);
        download_to_path(
            &self.client,
            &pkg.link,
            &cache_file,
            progress_downloaded,
            progress_total,
            cancel,
            referer.as_deref(),
        )?;
        checksum::verify_sha256_hex(&cache_file, &pkg.checksum)?;

        let staging_parent = self.paths.cache_dir().join(&version_key);
        fs::create_dir_all(&staging_parent).map_err(EnvrError::from)?;
        let staging = tempfile::tempdir_in(&staging_parent).map_err(EnvrError::from)?;
        extract::extract_archive(&cache_file, staging.path())?;

        let final_dir = self.paths.version_dir(&version_key);
        promote_single_root_dir(staging.path(), &final_dir)?;

        Ok(RuntimeVersion(version_key))
    }

    pub fn set_current(&self, version: &RuntimeVersion) -> EnvrResult<()> {
        let dir = self.paths.version_dir(&version.0);
        if !java_installation_valid(&dir) {
            return Err(EnvrError::Validation(format!(
                "jdk {} is not installed",
                version.0
            )));
        }
        let abs = fs::canonicalize(&dir).map_err(EnvrError::from)?;
        ensure_link(LinkType::Soft, &abs, self.paths.current_link())?;
        sync_java_home_export(&self.paths)?;
        Ok(())
    }

    pub fn uninstall(&self, version: &RuntimeVersion) -> EnvrResult<()> {
        let dir = self.paths.version_dir(&version.0);
        if dir.is_dir() {
            fs::remove_dir_all(&dir).map_err(EnvrError::from)?;
        }
        if read_current(&self.paths)?.is_some_and(|c| c.0 == version.0) {
            remove_path_if_exists(&self.paths.current_link());
            sync_java_home_export(&self.paths)?;
        }
        Ok(())
    }
}

impl SpecDrivenInstaller for JavaManager {
    fn install_from_spec(&self, request: &InstallRequest) -> EnvrResult<RuntimeVersion> {
        install_via_version_spec(request, |spec, downloaded, total, cancel| {
            self.install_for_spec(spec, downloaded, total, cancel)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn promote_single_root_moves_inner() {
        let tmp = tempfile::tempdir().expect("tmp");
        let staging = tmp.path().join("st");
        let inner = staging.join("jdk-0.0.0");
        fs::create_dir_all(inner.join("bin")).expect("bin");
        #[cfg(windows)]
        fs::write(inner.join("bin").join("java.exe"), []).expect("java");
        #[cfg(not(windows))]
        fs::write(inner.join("bin").join("java"), []).expect("java");
        let fin = tmp.path().join("out");
        promote_single_root_dir(&staging, &fin).expect("promote");
        assert!(java_installation_valid(&fin));
    }

    #[cfg(unix)]
    #[test]
    fn list_current_and_java_home_file() {
        let tmp = tempfile::tempdir().expect("tmp");
        let paths = JavaPaths::new(tmp.path().to_path_buf(), "temurin");
        fs::create_dir_all(paths.versions_dir()).expect("mkdir");
        let vdir = paths.version_dir("21.0.1+7");
        fs::create_dir_all(vdir.join("bin")).expect("bin");
        fs::write(vdir.join("bin").join("java"), []).expect("java");

        ensure_link(LinkType::Soft, &vdir, paths.current_link()).expect("link");
        sync_java_home_export(&paths).expect("sync");
        let cur = read_current(&paths).expect("cur").expect("some");
        assert_eq!(cur.0, "21.0.1+7");
        let home = read_java_home_export(&paths).expect("read").expect("path");
        assert!(java_installation_valid(&home));
        let listed = list_installed_versions(&paths).expect("list");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].0, "21.0.1+7");
    }

    #[cfg(windows)]
    #[test]
    fn list_installed_finds_jdk_layout() {
        let tmp = tempfile::tempdir().expect("tmp");
        let paths = JavaPaths::new(tmp.path().to_path_buf(), "temurin");
        fs::create_dir_all(paths.versions_dir()).expect("mkdir");
        let vdir = paths.version_dir("21.0.1+7");
        fs::create_dir_all(vdir.join("bin")).expect("bin");
        fs::write(vdir.join("bin").join("java.exe"), []).expect("java");
        let listed = list_installed_versions(&paths).expect("list");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].0, "21.0.1+7");
    }
}
