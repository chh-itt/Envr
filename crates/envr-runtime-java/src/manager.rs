//! Install / uninstall / `current` for Temurin JDKs under the envr data root.

use crate::index::{
    adoptium_arch, adoptium_assets_version_range_segment, adoptium_os, blocking_http_client,
    load_java_index, normalize_openjdk_version_label, resolve_java_version,
};
use crate::vendor::JavaVendor;
use envr_domain::runtime::{RuntimeVersion, VersionSpec};
use envr_download::{checksum, extract};
use envr_error::{EnvrError, EnvrResult};
use envr_platform::links::{LinkType, ensure_link};
use serde::Deserialize;
use std::{
    fs,
    path::{Path, PathBuf},
};

/// Layout: `{runtime_root}/runtimes/java/versions/<openjdk_version>/...`, `current` → version dir,
/// `JAVA_HOME` file with the resolved absolute JDK root (for shells).
#[derive(Debug, Clone)]
pub struct JavaPaths {
    runtime_root: PathBuf,
}

impl JavaPaths {
    pub fn new(runtime_root: PathBuf) -> Self {
        Self { runtime_root }
    }

    pub fn java_home(&self) -> PathBuf {
        self.runtime_root.join("runtimes").join("java")
    }

    pub fn versions_dir(&self) -> PathBuf {
        self.java_home().join("versions")
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
    let name = resolved
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
        return Ok(());
    }
    let abs = fs::canonicalize(&current).map_err(EnvrError::from)?;
    if let Some(parent) = marker.parent() {
        fs::create_dir_all(parent).map_err(EnvrError::from)?;
    }
    fs::write(&marker, format!("{}\n", abs.display())).map_err(EnvrError::from)?;
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
    let r = client
        .get(url)
        .send()
        .map_err(|e| EnvrError::Download(e.to_string()))?;
    if !r.status().is_success() {
        return Err(EnvrError::Download(format!("GET {url} -> {}", r.status())));
    }
    let body = r.text().map_err(|e| EnvrError::Download(e.to_string()))?;
    serde_json::from_str(&body).map_err(|e| EnvrError::Validation(e.to_string()))
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

fn download_to_path(client: &reqwest::blocking::Client, url: &str, path: &Path) -> EnvrResult<()> {
    let mut response = client
        .get(url)
        .send()
        .map_err(|e| EnvrError::Download(e.to_string()))?;
    if !response.status().is_success() {
        return Err(EnvrError::Download(format!(
            "GET {} -> {}",
            url,
            response.status()
        )));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(EnvrError::from)?;
    }
    let mut f = fs::File::create(path).map_err(EnvrError::from)?;
    response
        .copy_to(&mut f)
        .map_err(|e| EnvrError::Download(e.to_string()))?;
    Ok(())
}

/// Temurin archives ship a single top-level `jdk-...` directory; promote it to `final_dir`.
pub fn promote_single_root_dir(staging: &Path, final_dir: &Path) -> EnvrResult<()> {
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
    if final_dir.exists() {
        fs::remove_dir_all(final_dir).map_err(EnvrError::from)?;
    }
    // `fs::rename(src, dst)` requires the destination parent to exist.
    // Ensure it to avoid Windows `os error 3` when the runtimes dir hasn't been created yet.
    let parent = final_dir
        .parent()
        .ok_or_else(|| EnvrError::Validation("final_dir has no parent".into()))?;
    fs::create_dir_all(parent).map_err(EnvrError::from)?;
    fs::rename(&inner, final_dir).map_err(EnvrError::from)?;
    Ok(())
}

pub struct JavaManager {
    pub paths: JavaPaths,
    api_base: String,
    vendor: JavaVendor,
    client: reqwest::blocking::Client,
}

impl JavaManager {
    pub fn try_new(
        runtime_root: PathBuf,
        api_base: String,
        vendor: JavaVendor,
    ) -> EnvrResult<Self> {
        Ok(Self {
            paths: JavaPaths::new(runtime_root),
            api_base,
            vendor,
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

    pub fn install_from_spec(&self, spec: &VersionSpec) -> EnvrResult<RuntimeVersion> {
        let index = load_java_index(
            &self.client,
            &self.api_base,
            self.vendor,
            std::env::consts::OS,
            std::env::consts::ARCH,
        )?;
        let label = resolve_java_version(&index, &spec.0)?;
        self.install_resolved_label(&label)
    }

    /// Installs a resolved Adoptium `openjdk_version` label (e.g. `24.0.2+12`).
    pub fn install_resolved_label(
        &self,
        openjdk_version_label: &str,
    ) -> EnvrResult<RuntimeVersion> {
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
        download_to_path(&self.client, &pkg.link, &cache_file)?;
        checksum::verify_sha256_hex(&cache_file, &pkg.checksum)?;

        let staging_parent = self.paths.cache_dir().join(&version_key);
        fs::create_dir_all(&staging_parent).map_err(EnvrError::from)?;
        let staging = tempfile::tempdir_in(&staging_parent).map_err(EnvrError::from)?;
        extract::extract_archive(&cache_file, staging.path())?;

        let final_dir = self.paths.version_dir(&version_key);
        promote_single_root_dir(staging.path(), &final_dir)?;

        if !java_installation_valid(&final_dir) {
            return Err(EnvrError::Validation(
                "extracted jdk layout missing java binary".into(),
            ));
        }

        let v = RuntimeVersion(version_key);
        self.set_current(&v)?;
        Ok(v)
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
        let paths = JavaPaths::new(tmp.path().to_path_buf());
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
        let paths = JavaPaths::new(tmp.path().to_path_buf());
        fs::create_dir_all(paths.versions_dir()).expect("mkdir");
        let vdir = paths.version_dir("21.0.1+7");
        fs::create_dir_all(vdir.join("bin")).expect("bin");
        fs::write(vdir.join("bin").join("java.exe"), []).expect("java");
        let listed = list_installed_versions(&paths).expect("list");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].0, "21.0.1+7");
    }
}
