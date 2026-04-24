use crate::index::{
    blocking_http_client, fetch_versions_json, find_version_entry, julia_host_target,
    list_remote_latest_per_major_lines, list_remote_versions, parse_versions_root,
    pick_file_for_host, resolve_julia_version,
};
use envr_domain::installer::{
    SpecDrivenInstaller, execute_install_pipeline, install_progress_handles,
};
use envr_domain::runtime::{InstallRequest, RemoteFilter, RuntimeVersion};
use envr_download::{checksum, extract};
use envr_error::{EnvrError, EnvrResult, ErrorCode};
use envr_platform::bin_tool_layout::julia_installation_valid;
use envr_platform::links::ensure_runtime_current_symlink_or_pointer;
use serde_json::Value;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::SystemTime;

#[derive(Debug, Clone)]
pub struct JuliaPaths {
    runtime_root: PathBuf,
}

impl JuliaPaths {
    pub fn new(runtime_root: PathBuf) -> Self {
        Self { runtime_root }
    }

    pub fn julia_home(&self) -> PathBuf {
        self.runtime_root.join("runtimes").join("julia")
    }

    pub fn versions_dir(&self) -> PathBuf {
        self.julia_home().join("versions")
    }

    pub fn current_link(&self) -> PathBuf {
        self.julia_home().join("current")
    }

    pub fn cache_dir(&self) -> PathBuf {
        self.runtime_root.join("cache").join("julia")
    }

    pub fn versions_json_cache(&self) -> PathBuf {
        self.cache_dir().join("versions.json")
    }

    pub fn version_dir(&self, version_label: &str) -> PathBuf {
        self.versions_dir().join(version_label)
    }
}

pub fn list_installed_versions(paths: &JuliaPaths) -> EnvrResult<Vec<RuntimeVersion>> {
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
        if julia_installation_valid(&p) {
            out.push(RuntimeVersion(e.file_name().to_string_lossy().into_owned()));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

pub fn read_current(paths: &JuliaPaths) -> EnvrResult<Option<RuntimeVersion>> {
    let cur = paths.current_link();
    if !cur.exists() {
        return Ok(None);
    }
    if cur.is_file() {
        let s = fs::read_to_string(&cur).map_err(EnvrError::from)?;
        let t = s.trim();
        if t.is_empty() {
            return Ok(None);
        }
        let target = PathBuf::from(t);
        let resolved = fs::canonicalize(&target).unwrap_or(target);
        let name = resolved
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        if name.is_empty() {
            return Ok(None);
        }
        return Ok(Some(RuntimeVersion(name)));
    }
    let Ok(target) = fs::read_link(&cur) else {
        return Ok(None);
    };
    let resolved = if target.is_relative() {
        cur.parent().map(|p| p.join(&target)).unwrap_or(target)
    } else {
        target
    };
    let resolved = fs::canonicalize(&resolved).unwrap_or(resolved);
    let name = resolved
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();
    if name.is_empty() {
        return Ok(None);
    }
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

fn download_to_path(
    client: &reqwest::blocking::Client,
    url: &str,
    path: &Path,
    progress_downloaded: Option<&Arc<AtomicU64>>,
    progress_total: Option<&Arc<AtomicU64>>,
    cancel: Option<&Arc<AtomicBool>>,
) -> EnvrResult<()> {
    if cancel.is_some_and(|c| c.load(Ordering::Relaxed)) {
        return Err(EnvrError::Download("download cancelled".into()));
    }
    let mut response = client.get(url).send().map_err(|e| {
        EnvrError::with_source(ErrorCode::Download, format!("request failed for {url}"), e)
    })?;
    if !response.status().is_success() {
        return Err(EnvrError::Download(format!(
            "GET {url} -> {}",
            response.status()
        )));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(EnvrError::from)?;
    }
    if let Some(t) = progress_total {
        t.store(response.content_length().unwrap_or(0), Ordering::Relaxed);
    }
    if let Some(d) = progress_downloaded {
        d.store(0, Ordering::Relaxed);
    }
    let mut f = fs::File::create(path).map_err(EnvrError::from)?;
    let mut buf = [0u8; 64 * 1024];
    loop {
        if cancel.is_some_and(|c| c.load(Ordering::Relaxed)) {
            return Err(EnvrError::Download("download cancelled".into()));
        }
        let n = response.read(&mut buf).map_err(|e| {
            EnvrError::with_source(
                ErrorCode::Download,
                format!("read response body failed for {url}"),
                e,
            )
        })?;
        if n == 0 {
            break;
        }
        f.write_all(&buf[..n]).map_err(EnvrError::from)?;
        if let Some(d) = progress_downloaded {
            d.fetch_add(n as u64, Ordering::Relaxed);
        }
    }
    Ok(())
}

pub fn promote_single_root_dir(staging: &Path, final_dir: &Path) -> EnvrResult<()> {
    use envr_platform::install_layout;

    let mut iter = fs::read_dir(staging).map_err(EnvrError::from)?;
    let first = iter
        .next()
        .transpose()
        .map_err(EnvrError::from)?
        .ok_or_else(|| EnvrError::Validation("empty julia archive".into()))?;
    if iter.next().transpose().map_err(EnvrError::from)?.is_some() {
        return Err(EnvrError::Validation(
            "expected exactly one root directory in julia archive".into(),
        ));
    }
    let inner = first.path();
    if !inner.is_dir() {
        return Err(EnvrError::Validation(
            "expected julia archive root to be a directory".into(),
        ));
    }
    install_layout::ensure_final_parent(final_dir)?;
    let staging_final = install_layout::sibling_staging_path(final_dir)?;
    install_layout::remove_if_exists(&staging_final)?;

    fs::rename(&inner, &staging_final).map_err(EnvrError::from)?;

    if !julia_installation_valid(&staging_final) {
        let _ = fs::remove_dir_all(&staging_final);
        return Err(EnvrError::Validation(
            "extracted julia layout missing bin/julia".into(),
        ));
    }

    install_layout::commit_staging_dir(&staging_final, final_dir)?;
    Ok(())
}

pub struct JuliaManager {
    pub paths: JuliaPaths,
    versions_url: String,
    client: reqwest::blocking::Client,
}

impl JuliaManager {
    pub fn try_new(runtime_root: PathBuf, versions_url: String) -> EnvrResult<Self> {
        Ok(Self {
            paths: JuliaPaths::new(runtime_root),
            versions_url,
            client: blocking_http_client()?,
        })
    }

    fn index_ttl_secs() -> u64 {
        const DEFAULT: u64 = 60 * 60;
        std::env::var("ENVR_JULIA_VERSIONS_CACHE_TTL_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT)
    }

    pub fn load_versions_map(&self) -> EnvrResult<serde_json::Map<String, Value>> {
        let cache_path = self.paths.versions_json_cache();
        let ttl = Self::index_ttl_secs();
        if let Ok(meta) = fs::metadata(&cache_path) {
            if let Ok(modified) = meta.modified() {
                if let Ok(age) = SystemTime::now().duration_since(modified) {
                    if age.as_secs() < ttl {
                        if let Ok(body) = fs::read_to_string(&cache_path) {
                            if let Ok(m) = parse_versions_root(&body) {
                                return Ok(m);
                            }
                        }
                    }
                }
            }
        }
        let body = fetch_versions_json(&self.client, &self.versions_url)?;
        fs::create_dir_all(self.paths.cache_dir()).map_err(EnvrError::from)?;
        envr_platform::fs_atomic::write_atomic(&cache_path, body.as_bytes())
            .map_err(EnvrError::from)?;
        parse_versions_root(&body)
    }

    pub fn list_remote(&self, filter: &RemoteFilter) -> EnvrResult<Vec<RuntimeVersion>> {
        let m = self.load_versions_map()?;
        let host = julia_host_target()?;
        list_remote_versions(&m, host, filter)
    }

    pub fn list_remote_latest_per_major(&self) -> EnvrResult<Vec<RuntimeVersion>> {
        let m = self.load_versions_map()?;
        let host = julia_host_target()?;
        Ok(list_remote_latest_per_major_lines(&m, host))
    }

    pub fn resolve_label(&self, spec: &str) -> EnvrResult<String> {
        let m = self.load_versions_map()?;
        let host = julia_host_target()?;
        resolve_julia_version(&m, host, spec)
    }

    pub fn install_resolved_version(
        &self,
        version_label: &str,
        progress_downloaded: Option<&Arc<AtomicU64>>,
        progress_total: Option<&Arc<AtomicU64>>,
        cancel: Option<&Arc<AtomicBool>>,
    ) -> EnvrResult<RuntimeVersion> {
        let m = self.load_versions_map()?;
        let host = julia_host_target()?;
        let entry = find_version_entry(&m, version_label)?;
        let file = pick_file_for_host(entry, host).ok_or_else(|| {
            EnvrError::Validation(format!(
                "no portable Julia archive for `{version_label}` on this platform"
            ))
        })?;
        let url = file
            .get("url")
            .and_then(|u| u.as_str())
            .ok_or_else(|| EnvrError::Validation("julia file entry missing url".into()))?;
        let sha256 = file
            .get("sha256")
            .and_then(|s| s.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty());

        let ext = if url.to_ascii_lowercase().ends_with(".zip") {
            ".zip"
        } else if url.to_ascii_lowercase().ends_with(".tar.gz") {
            ".tar.gz"
        } else {
            return Err(EnvrError::Validation(format!(
                "unsupported julia archive URL suffix: {url}"
            )));
        };

        let cache_dir = self.paths.cache_dir().join(version_label);
        let archive_path = cache_dir.join(format!("julia{ext}"));
        let final_dir = self.paths.version_dir(version_label);
        execute_install_pipeline(
            cancel,
            || fs::create_dir_all(&cache_dir).map_err(EnvrError::from),
            || {
                download_to_path(
                    &self.client,
                    url,
                    &archive_path,
                    progress_downloaded,
                    progress_total,
                    cancel,
                )
            },
            || {
                if let Some(h) = sha256 {
                    checksum::verify_sha256_hex(&archive_path, h)?;
                }
                Ok(())
            },
            || {
                let staging_parent = cache_dir.join("extract_staging");
                fs::create_dir_all(&staging_parent).map_err(EnvrError::from)?;
                let staging = tempfile::tempdir_in(&staging_parent).map_err(EnvrError::from)?;
                extract::extract_archive(&archive_path, staging.path())?;
                promote_single_root_dir(staging.path(), &final_dir)
            },
            || {
                let resolved = RuntimeVersion(version_label.to_string());
                self.set_current(&resolved)?;
                Ok(resolved)
            },
        )
    }

    pub fn set_current(&self, version: &RuntimeVersion) -> EnvrResult<()> {
        let dir = self.paths.version_dir(&version.0);
        if !julia_installation_valid(&dir) {
            return Err(EnvrError::Validation(format!(
                "julia {} is not installed under {}",
                version.0,
                dir.display()
            )));
        }
        let link = self.paths.current_link();
        ensure_runtime_current_symlink_or_pointer(&dir, &link)?;
        Ok(())
    }

    pub fn uninstall(&self, version: &RuntimeVersion) -> EnvrResult<()> {
        let dir = self.paths.version_dir(&version.0);
        if dir.is_dir() {
            fs::remove_dir_all(&dir).map_err(EnvrError::from)?;
        }
        if read_current(&self.paths)?.is_some_and(|c| c.0 == version.0) {
            remove_path_if_exists(&self.paths.current_link());
        }
        Ok(())
    }
}

impl SpecDrivenInstaller for JuliaManager {
    fn install_from_spec(&self, request: &InstallRequest) -> EnvrResult<RuntimeVersion> {
        let label = self.resolve_label(&request.spec.0)?;
        let (downloaded, total, cancel) = install_progress_handles(request);
        self.install_resolved_version(&label, downloaded, total, cancel)
    }
}
