use crate::index::{
    blocking_http_client, cran_windows_r_installer_url, fetch_text,
    list_remote_latest_per_major_lines, list_remote_versions, parse_latest_win_release_version,
    parse_r_versions_list, resolve_r_version,
};
use envr_domain::installer::{SpecDrivenInstaller, install_progress_handles};
use envr_domain::runtime::{InstallRequest, RemoteFilter, RuntimeVersion};
use envr_error::{EnvrError, EnvrResult, ErrorCode};
use envr_platform::bin_tool_layout::rlang_installation_valid;
use envr_platform::links::ensure_runtime_current_symlink_or_pointer;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::SystemTime;

#[derive(Debug, Clone)]
pub struct RlangPaths {
    runtime_root: PathBuf,
}

impl RlangPaths {
    pub fn new(runtime_root: PathBuf) -> Self {
        Self { runtime_root }
    }

    pub fn rlang_home(&self) -> PathBuf {
        self.runtime_root.join("runtimes").join("r")
    }

    pub fn versions_dir(&self) -> PathBuf {
        self.rlang_home().join("versions")
    }

    pub fn current_link(&self) -> PathBuf {
        self.rlang_home().join("current")
    }

    pub fn cache_dir(&self) -> PathBuf {
        self.runtime_root.join("cache").join("r")
    }

    pub fn versions_json_cache(&self) -> PathBuf {
        self.cache_dir().join("r-versions.json")
    }

    pub fn release_win_json_cache(&self) -> PathBuf {
        self.cache_dir().join("r-release-win.json")
    }

    pub fn version_dir(&self, version_label: &str) -> PathBuf {
        self.versions_dir().join(version_label)
    }
}

pub fn list_installed_versions(paths: &RlangPaths) -> EnvrResult<Vec<RuntimeVersion>> {
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
        if rlang_installation_valid(&p) {
            out.push(RuntimeVersion(e.file_name().to_string_lossy().into_owned()));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

pub fn read_current(paths: &RlangPaths) -> EnvrResult<Option<RuntimeVersion>> {
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

#[cfg(windows)]
fn run_cran_r_windows_installer(installer: &Path, target_dir: &Path) -> EnvrResult<()> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    let dir_os = target_dir.as_os_str().to_string_lossy().to_string();
    let status = std::process::Command::new(installer)
        .raw_arg("/VERYSILENT")
        .raw_arg("/SUPPRESSMSGBOXES")
        .raw_arg("/NORESTART")
        .raw_arg(format!("/DIR={dir_os}"))
        .creation_flags(CREATE_NO_WINDOW)
        .status()
        .map_err(|e| {
            EnvrError::with_source(ErrorCode::Runtime, "failed to spawn R installer", e)
        })?;
    if !status.success() {
        return Err(EnvrError::Runtime(format!(
            "R Windows installer exited with status {status}"
        )));
    }
    Ok(())
}

#[cfg(not(windows))]
fn run_cran_r_windows_installer(_installer: &Path, _target_dir: &Path) -> EnvrResult<()> {
    Err(EnvrError::Validation(
        "R managed install is only implemented on Windows in this release".into(),
    ))
}

fn require_windows_managed_r() -> EnvrResult<()> {
    #[cfg(windows)]
    {
        Ok(())
    }
    #[cfg(not(windows))]
    {
        Err(EnvrError::Validation(
            "envr R: managed install and remote index are only supported on Windows in this release"
                .into(),
        ))
    }
}

pub struct RlangManager {
    pub paths: RlangPaths,
    versions_url: String,
    release_win_url: String,
    client: reqwest::blocking::Client,
}

impl RlangManager {
    pub fn try_new(
        runtime_root: PathBuf,
        versions_url: String,
        release_win_url: String,
    ) -> EnvrResult<Self> {
        Ok(Self {
            paths: RlangPaths::new(runtime_root),
            versions_url,
            release_win_url,
            client: blocking_http_client()?,
        })
    }

    fn index_ttl_secs() -> u64 {
        const DEFAULT: u64 = 60 * 60;
        std::env::var("ENVR_RLANG_INDEX_CACHE_TTL_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT)
    }

    fn load_versions_list_cached(&self) -> EnvrResult<Vec<String>> {
        require_windows_managed_r()?;
        let cache_path = self.paths.versions_json_cache();
        let ttl = Self::index_ttl_secs();
        if let Ok(meta) = fs::metadata(&cache_path) {
            if let Ok(modified) = meta.modified() {
                if let Ok(age) = SystemTime::now().duration_since(modified) {
                    if age.as_secs() < ttl {
                        if let Ok(body) = fs::read_to_string(&cache_path) {
                            if let Ok(v) = parse_r_versions_list(&body) {
                                if !v.is_empty() {
                                    return Ok(v);
                                }
                            }
                        }
                    }
                }
            }
        }
        let body = fetch_text(&self.client, &self.versions_url)?;
        fs::create_dir_all(self.paths.cache_dir()).map_err(EnvrError::from)?;
        envr_platform::fs_atomic::write_atomic(&cache_path, body.as_bytes())
            .map_err(EnvrError::from)?;
        parse_r_versions_list(&body)
    }

    fn load_latest_win_version_cached(&self) -> EnvrResult<String> {
        let cache_path = self.paths.release_win_json_cache();
        let ttl = Self::index_ttl_secs();
        if let Ok(meta) = fs::metadata(&cache_path) {
            if let Ok(modified) = meta.modified() {
                if let Ok(age) = SystemTime::now().duration_since(modified) {
                    if age.as_secs() < ttl {
                        if let Ok(body) = fs::read_to_string(&cache_path) {
                            if let Ok(v) = parse_latest_win_release_version(&body) {
                                if !v.is_empty() {
                                    return Ok(v);
                                }
                            }
                        }
                    }
                }
            }
        }
        let body = fetch_text(&self.client, &self.release_win_url)?;
        fs::create_dir_all(self.paths.cache_dir()).map_err(EnvrError::from)?;
        envr_platform::fs_atomic::write_atomic(&cache_path, body.as_bytes())
            .map_err(EnvrError::from)?;
        parse_latest_win_release_version(&body)
    }

    pub fn list_remote(&self, filter: &RemoteFilter) -> EnvrResult<Vec<RuntimeVersion>> {
        let v = self.load_versions_list_cached()?;
        Ok(list_remote_versions(&v, filter))
    }

    pub fn list_remote_latest_per_major(&self) -> EnvrResult<Vec<RuntimeVersion>> {
        let v = self.load_versions_list_cached()?;
        Ok(list_remote_latest_per_major_lines(&v))
    }

    pub fn resolve_label(&self, spec: &str) -> EnvrResult<String> {
        let v = self.load_versions_list_cached()?;
        resolve_r_version(&v, spec)
    }

    pub fn install_resolved_version(
        &self,
        version_label: &str,
        progress_downloaded: Option<&Arc<AtomicU64>>,
        progress_total: Option<&Arc<AtomicU64>>,
        cancel: Option<&Arc<AtomicBool>>,
    ) -> EnvrResult<RuntimeVersion> {
        require_windows_managed_r()?;
        if cancel.is_some_and(|c| c.load(Ordering::Relaxed)) {
            return Err(EnvrError::Download("download cancelled".into()));
        }
        let versions = self.load_versions_list_cached()?;
        if !versions.iter().any(|v| v == version_label) {
            return Err(EnvrError::Validation(format!(
                "R version `{version_label}` is not in the r-versions index for this host policy"
            )));
        }
        let latest = self.load_latest_win_version_cached()?;
        let url = cran_windows_r_installer_url(version_label, &latest);

        let final_dir = self.paths.version_dir(version_label);
        if final_dir.exists() {
            let _ = fs::remove_dir_all(&final_dir);
        }
        fs::create_dir_all(self.paths.cache_dir()).map_err(EnvrError::from)?;
        let installer_path = self
            .paths
            .cache_dir()
            .join(format!("R-{version_label}-win-installer.exe"));
        download_to_path(
            &self.client,
            &url,
            &installer_path,
            progress_downloaded,
            progress_total,
            cancel,
        )?;

        fs::create_dir_all(self.paths.versions_dir()).map_err(EnvrError::from)?;
        run_cran_r_windows_installer(&installer_path, &final_dir)?;

        if !rlang_installation_valid(&final_dir) {
            let _ = fs::remove_dir_all(&final_dir);
            return Err(EnvrError::Validation(
                "R install finished but bin/R.exe or bin/Rscript.exe is missing".into(),
            ));
        }

        let _ = fs::remove_file(&installer_path);
        self.set_current(&RuntimeVersion(version_label.to_string()))?;
        Ok(RuntimeVersion(version_label.to_string()))
    }

    pub fn set_current(&self, version: &RuntimeVersion) -> EnvrResult<()> {
        let dir = self.paths.version_dir(&version.0);
        if !rlang_installation_valid(&dir) {
            return Err(EnvrError::Validation(format!(
                "cannot set current R to {}: installation invalid or missing",
                version.0
            )));
        }
        let link = self.paths.current_link();
        ensure_runtime_current_symlink_or_pointer(&dir, &link)?;
        Ok(())
    }

    pub fn uninstall(&self, version: &RuntimeVersion) -> EnvrResult<()> {
        let dir = self.paths.version_dir(&version.0);
        if dir.exists() {
            fs::remove_dir_all(&dir).map_err(EnvrError::from)?;
        }
        if read_current(&self.paths)?.as_ref() == Some(version) {
            remove_path_if_exists(&self.paths.current_link());
        }
        Ok(())
    }
}

impl SpecDrivenInstaller for RlangManager {
    fn install_from_spec(&self, request: &InstallRequest) -> EnvrResult<RuntimeVersion> {
        let label = self.resolve_label(&request.spec.0)?;
        let (downloaded, total, cancel) = install_progress_handles(request);
        self.install_resolved_version(&label, downloaded, total, cancel)
    }
}
