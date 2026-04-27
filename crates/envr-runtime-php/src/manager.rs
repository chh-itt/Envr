use crate::index::{
    ReleaseLine, blocking_http_client, fetch_php_windows_releases_json, parse_php_windows_index,
    pick_windows_zip, resolve_php_version,
};
use envr_config::php_layout;
use envr_domain::installer::{
    SpecDrivenInstaller, execute_install_pipeline, install_progress_handles,
};
use envr_domain::runtime::{InstallRequest, RuntimeVersion};
use envr_download::{blocking::download_url_to_path_resumable, checksum, extract};
use envr_error::{EnvrError, EnvrResult, ErrorCode};
use envr_platform::links::{LinkType, ensure_link};
use reqwest::header::HeaderMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

fn err_version_not_found(msg: impl Into<String>) -> EnvrError {
    EnvrError::Context {
        code: ErrorCode::RuntimeVersionNotFound,
        message: msg.into(),
        source: Box::new(std::io::Error::other("runtime-version-not-found")),
    }
}

#[derive(Debug, Clone)]
pub struct PhpPaths {
    runtime_root: PathBuf,
}

impl PhpPaths {
    pub fn new(runtime_root: PathBuf) -> Self {
        Self { runtime_root }
    }

    pub fn php_home(&self) -> PathBuf {
        self.runtime_root.join("runtimes").join("php")
    }

    pub fn versions_dir(&self) -> PathBuf {
        self.php_home().join("versions")
    }

    /// Single global selection for `php` (one active version for NTS + TS combined).
    pub fn current_link(&self) -> PathBuf {
        self.php_home().join("current")
    }

    pub fn cache_dir(&self) -> PathBuf {
        self.runtime_root.join("cache").join("php")
    }

    pub fn version_dir_for(&self, semver: &str, want_ts: bool) -> PathBuf {
        self.versions_dir()
            .join(php_layout::version_dir_name(semver, want_ts))
    }
}

pub fn php_installation_valid(home: &Path) -> bool {
    #[cfg(windows)]
    {
        home.join("php.exe").is_file()
    }
    #[cfg(not(windows))]
    {
        home.join("bin").join("php").is_file() || home.join("php").is_file()
    }
}

pub fn list_installed_versions(paths: &PhpPaths, want_ts: bool) -> EnvrResult<Vec<RuntimeVersion>> {
    let dir = paths.versions_dir();
    if !dir.is_dir() {
        return Ok(vec![]);
    }
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::<String>::new();
    for e in fs::read_dir(&dir).map_err(EnvrError::from)? {
        let e = e.map_err(EnvrError::from)?;
        if !e.file_type().map_err(EnvrError::from)?.is_dir() {
            continue;
        }
        let p = e.path();
        let name = e.file_name().to_string_lossy().into_owned();
        if !php_layout::dir_matches_build_flavor(&name, want_ts) {
            continue;
        }
        if php_installation_valid(&p) {
            let label = php_layout::display_version_label_from_dir_name(&name);
            if seen.insert(label.clone()) {
                out.push(RuntimeVersion(label));
            }
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

/// All registered installs under `versions/` (no Windows NTS/TS directory filter). Uses [`Path::is_dir`] so symlinks to prefixes count.
pub fn list_installed_versions_unfiltered(paths: &PhpPaths) -> EnvrResult<Vec<RuntimeVersion>> {
    let dir = paths.versions_dir();
    if !dir.is_dir() {
        return Ok(vec![]);
    }
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::<String>::new();
    for e in fs::read_dir(&dir).map_err(EnvrError::from)? {
        let e = e.map_err(EnvrError::from)?;
        let p = e.path();
        if !p.is_dir() {
            continue;
        }
        if !php_installation_valid(&p) {
            continue;
        }
        let Some(fname) = p.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        let label = php_layout::display_version_label_from_dir_name(fname);
        if seen.insert(label.clone()) {
            out.push(RuntimeVersion(label));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

fn version_label_from_registered_versions_dir(
    paths: &PhpPaths,
    home_canon: &Path,
) -> Option<String> {
    let vd = paths.versions_dir();
    let rd = fs::read_dir(&vd).ok()?;
    for e in rd.flatten() {
        let p = e.path();
        if !p.is_dir() {
            continue;
        }
        if !php_installation_valid(&p) {
            continue;
        }
        let c = fs::canonicalize(&p).ok()?;
        if c == *home_canon {
            let fname = p.file_name()?.to_str()?;
            return Some(php_layout::display_version_label_from_dir_name(fname));
        }
    }
    None
}

/// Migration: older builds used `current-ts` / `current-nts` separately; resolve in a fixed order.
fn php_global_current_link_candidates(paths: &PhpPaths) -> [PathBuf; 3] {
    let h = paths.php_home();
    [
        h.join("current"),
        h.join("current-ts"),
        h.join("current-nts"),
    ]
}

/// Canonical target directory for the active global PHP (if any).
pub fn resolve_global_php_current_target(paths: &PhpPaths) -> EnvrResult<Option<PathBuf>> {
    for link in php_global_current_link_candidates(paths) {
        if let Some(t) = resolve_current_link_target(&link)? {
            return Ok(Some(t));
        }
    }
    Ok(None)
}

fn resolve_current_link_target(link: &Path) -> EnvrResult<Option<PathBuf>> {
    if !link.exists() {
        return Ok(None);
    }
    if link.is_file() {
        let s = fs::read_to_string(link).map_err(EnvrError::from)?;
        let t = s.trim();
        let target = PathBuf::from(t);
        return Ok(Some(fs::canonicalize(&target).map_err(EnvrError::from)?));
    }
    if let Ok(t) = fs::read_link(link) {
        let resolved = if t.is_relative() {
            link.parent().map(|p| p.join(&t)).unwrap_or(t)
        } else {
            t
        };
        return Ok(Some(fs::canonicalize(&resolved).map_err(EnvrError::from)?));
    }
    if link.is_dir() {
        return Ok(Some(fs::canonicalize(link).map_err(EnvrError::from)?));
    }
    Ok(None)
}

/// Global active version label (independent of NTS/TS UI tab).
pub fn read_current(paths: &PhpPaths) -> EnvrResult<Option<RuntimeVersion>> {
    let Some(resolved) = resolve_global_php_current_target(paths)? else {
        return Ok(None);
    };
    let home_canon = fs::canonicalize(&resolved).unwrap_or(resolved);
    let label = if let Some(l) = version_label_from_registered_versions_dir(paths, &home_canon) {
        l
    } else {
        let Some(name) = home_canon.file_name().and_then(|s| s.to_str()) else {
            return Ok(None);
        };
        if name.is_empty() {
            return Ok(None);
        }
        php_layout::display_version_label_from_dir_name(name)
    };
    Ok(Some(RuntimeVersion(label)))
}

/// Whether the global active PHP is a TS build (`None` = no global current).
pub fn read_current_global_want_ts(paths: &PhpPaths) -> EnvrResult<Option<bool>> {
    #[cfg(not(windows))]
    {
        let _ = paths;
        return Ok(None);
    }
    #[cfg(windows)]
    {
        let Some(resolved) = resolve_global_php_current_target(paths)? else {
            return Ok(None);
        };
        let Some(name) = resolved.file_name().and_then(|s| s.to_str()) else {
            return Ok(None);
        };
        if name.is_empty() {
            return Ok(None);
        }
        Ok(Some(php_layout::dir_flavor_is_ts(name)))
    }
}

pub(crate) fn remove_stale_split_current_files(paths: &PhpPaths) {
    let h = paths.php_home();
    let _ = fs::remove_file(h.join("current-ts"));
    let _ = fs::remove_file(h.join("current-nts"));
}

fn promote_archive_layout(staging: &Path, final_dir: &Path) -> EnvrResult<()> {
    use envr_platform::install_layout;

    install_layout::ensure_final_parent(final_dir)?;
    let staging_final = install_layout::sibling_staging_path(final_dir)?;
    install_layout::remove_if_exists(&staging_final)?;

    // Prefer "single root directory" layouts.
    let mut iter = fs::read_dir(staging).map_err(EnvrError::from)?;
    let first = iter.next().transpose().map_err(EnvrError::from)?;
    let second = iter.next().transpose().map_err(EnvrError::from)?;

    if let (Some(first), None) = (first, second) {
        let p = first.path();
        if p.is_dir() && php_installation_valid(&p) {
            fs::rename(&p, &staging_final).map_err(EnvrError::from)?;
            install_layout::commit_staging_dir(&staging_final, final_dir)?;
            return Ok(());
        }
    }

    install_layout::hoist_directory_children(staging, &staging_final)?;
    if !php_installation_valid(&staging_final) {
        let _ = fs::remove_dir_all(&staging_final);
        return Err(err_version_not_found(
            "extracted php layout missing php executable",
        ));
    }
    install_layout::commit_staging_dir(&staging_final, final_dir)?;
    Ok(())
}

pub struct PhpManager {
    pub paths: PhpPaths,
    releases_json_url: String,
    releases_base_url: String,
    want_ts: bool,
    client: reqwest::blocking::Client,
}

impl PhpManager {
    pub fn try_new(
        runtime_root: PathBuf,
        releases_json_url: String,
        want_ts: bool,
    ) -> EnvrResult<Self> {
        let releases_base_url = releases_json_url
            .trim_end_matches("releases.json")
            .trim_end_matches('/')
            .to_string();
        Ok(Self {
            paths: PhpPaths::new(runtime_root),
            releases_json_url,
            releases_base_url,
            want_ts,
            client: blocking_http_client()?,
        })
    }

    fn load_index(&self) -> EnvrResult<std::collections::HashMap<String, ReleaseLine>> {
        let body = fetch_php_windows_releases_json(&self.client, &self.releases_json_url)?;
        parse_php_windows_index(&body)
    }

    pub fn install_resolved_version(
        &self,
        version: &RuntimeVersion,
        progress_downloaded: Option<&Arc<AtomicU64>>,
        progress_total: Option<&Arc<AtomicU64>>,
        cancel: Option<&Arc<AtomicBool>>,
    ) -> EnvrResult<RuntimeVersion> {
        if !cfg!(windows) {
            return Err(EnvrError::Platform(
                "php install is currently supported on Windows only".into(),
            ));
        }
        let idx = self.load_index()?;
        let line = idx
            .values()
            .find(|l| l.version == version.0)
            .ok_or_else(|| {
                err_version_not_found(format!("php version not found: {}", version.0))
            })?;

        let (zip_name, sha) = pick_windows_zip(line, Some(self.want_ts), std::env::consts::ARCH)?;
        fs::create_dir_all(self.paths.cache_dir()).map_err(EnvrError::from)?;
        let storage_key = php_layout::version_dir_name(&version.0, self.want_ts);
        let cache_file = self.paths.cache_dir().join(&storage_key).join(&zip_name);
        let url = format!("{}/{}", self.releases_base_url, zip_name);
        let final_dir = self.paths.version_dir_for(&version.0, self.want_ts);
        execute_install_pipeline(
            cancel,
            || fs::create_dir_all(self.paths.cache_dir()).map_err(EnvrError::from),
            || {
                download_to_path(
                    &self.client,
                    &url,
                    &cache_file,
                    progress_downloaded,
                    progress_total,
                    cancel,
                )
            },
            || {
                if !sha.trim().is_empty() {
                    checksum::verify_sha256_hex(&cache_file, sha.trim())?;
                }
                Ok(())
            },
            || {
                let staging_parent = self.paths.cache_dir().join(&storage_key);
                fs::create_dir_all(&staging_parent).map_err(EnvrError::from)?;
                let staging = tempfile::tempdir_in(&staging_parent).map_err(EnvrError::from)?;
                extract::extract_archive(&cache_file, staging.path())?;
                promote_archive_layout(staging.path(), &final_dir)
            },
            || {
                self.set_current(version)?;
                Ok(RuntimeVersion(version.0.clone()))
            },
        )
    }

    pub fn set_current(&self, version: &RuntimeVersion) -> EnvrResult<()> {
        let dir = self.resolve_installed_version_dir(&version.0)?;
        ensure_link(LinkType::Soft, &dir, self.paths.current_link())?;
        remove_stale_split_current_files(&self.paths);
        Ok(())
    }

    fn resolve_installed_version_dir(&self, display_semver: &str) -> EnvrResult<PathBuf> {
        let flavored = self.paths.version_dir_for(display_semver, self.want_ts);
        if flavored.is_dir() && php_installation_valid(&flavored) {
            return Ok(flavored);
        }
        if !self.want_ts {
            let legacy = self.paths.versions_dir().join(display_semver);
            if legacy.is_dir() && php_installation_valid(&legacy) {
                return Ok(legacy);
            }
        }
        Err(err_version_not_found(format!(
            "php {} is not installed ({})",
            display_semver,
            if self.want_ts { "TS" } else { "NTS" }
        )))
    }

    pub fn uninstall(&self, version: &RuntimeVersion) -> EnvrResult<()> {
        let dir = self.resolve_installed_version_dir(&version.0)?;
        let dir_canon = fs::canonicalize(&dir).unwrap_or(dir.clone());
        let was_global = resolve_global_php_current_target(&self.paths)?
            .is_some_and(|g| fs::canonicalize(&g).unwrap_or(g) == dir_canon);
        if dir.is_dir() {
            fs::remove_dir_all(&dir).map_err(EnvrError::from)?;
        }
        if was_global {
            let _ = fs::remove_file(self.paths.current_link());
            remove_stale_split_current_files(&self.paths);
        }
        Ok(())
    }
}

impl SpecDrivenInstaller for PhpManager {
    fn install_from_spec(&self, request: &InstallRequest) -> EnvrResult<RuntimeVersion> {
        if !cfg!(windows) {
            return Err(EnvrError::Platform(
                "php install is currently supported on Windows only".into(),
            ));
        }
        let idx = self.load_index()?;
        let version = resolve_php_version(&idx, &request.spec.0)?;
        let (downloaded, total, cancel) = install_progress_handles(request);
        self.install_resolved_version(&RuntimeVersion(version), downloaded, total, cancel)
    }
}

fn content_length_from_headers(headers: &HeaderMap) -> Option<u64> {
    headers
        .get(reqwest::header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
}

/// Some mirrors omit `Content-Length` on chunked GET bodies; HEAD often still reports size for static zips.
fn probe_total_bytes_via_head(client: &reqwest::blocking::Client, url: &str) -> Option<u64> {
    let resp = client.head(url).send().ok()?;
    if !resp.status().is_success() {
        return None;
    }
    content_length_from_headers(resp.headers()).or_else(|| resp.content_length())
}

/// Some CDNs omit `Content-Length` on GET but return `Content-Range: bytes 0-0/<total>` for `Range: bytes=0-0`.
fn probe_total_bytes_via_content_range(
    client: &reqwest::blocking::Client,
    url: &str,
) -> Option<u64> {
    use reqwest::header::{CONTENT_RANGE, RANGE};
    let resp = client.get(url).header(RANGE, "bytes=0-0").send().ok()?;
    if !(resp.status().is_success() || resp.status() == reqwest::StatusCode::PARTIAL_CONTENT) {
        return None;
    }
    let cr = resp.headers().get(CONTENT_RANGE)?.to_str().ok()?;
    let rest = cr.strip_prefix("bytes ")?;
    let slash = rest.rfind('/')?;
    rest[slash + 1..].trim().parse().ok()
}

fn download_to_path(
    client: &reqwest::blocking::Client,
    url: &str,
    path: &Path,
    progress_downloaded: Option<&Arc<AtomicU64>>,
    progress_total: Option<&Arc<AtomicU64>>,
    cancel: Option<&Arc<AtomicBool>>,
) -> EnvrResult<()> {
    if let Some(t) = progress_total {
        let mut total = probe_total_bytes_via_head(client, url).unwrap_or(0);
        if total == 0 {
            total = probe_total_bytes_via_content_range(client, url).unwrap_or(0);
        }
        t.store(total, Ordering::Relaxed);
    }
    download_url_to_path_resumable(client, url, path, progress_downloaded, None, cancel)
}

#[cfg(test)]
mod download_tests {
    use super::*;
    use reqwest::header::{CONTENT_LENGTH, HeaderMap};

    #[test]
    fn content_length_from_headers_parses() {
        let mut h = HeaderMap::new();
        h.insert(CONTENT_LENGTH, "12345".parse().unwrap());
        assert_eq!(content_length_from_headers(&h), Some(12345));
    }

    #[test]
    fn promote_layout_missing_php_binary_has_structured_code() {
        let tmp = tempfile::tempdir().expect("tmp");
        let staging = tmp.path().join("staging");
        let out = tmp.path().join("out");
        std::fs::create_dir_all(&staging).expect("mkdir");
        std::fs::write(staging.join("README.txt"), b"no php exe").expect("write");
        let err = promote_archive_layout(&staging, &out).expect_err("should fail");
        assert_eq!(err.code(), ErrorCode::RuntimeVersionNotFound);
    }
}
