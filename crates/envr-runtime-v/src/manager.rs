use crate::index::{
    VInstallableRow, blocking_http_client, fetch_v_github_releases_index,
    installable_rows_from_releases, list_remote_latest_per_major_lines, list_remote_versions,
    resolve_v_version,
};
use envr_domain::runtime::{InstallRequest, RemoteFilter, RuntimeVersion};
use envr_download::extract;
use envr_error::{EnvrError, EnvrResult, ErrorCode};
use envr_platform::links::ensure_runtime_current_symlink_or_pointer;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::SystemTime;

#[derive(Debug, Clone)]
pub struct VPaths {
    runtime_root: PathBuf,
}

impl VPaths {
    pub fn new(runtime_root: PathBuf) -> Self {
        Self { runtime_root }
    }
    pub fn v_home(&self) -> PathBuf {
        self.runtime_root.join("runtimes").join("v")
    }
    pub fn versions_dir(&self) -> PathBuf {
        self.v_home().join("versions")
    }
    pub fn current_link(&self) -> PathBuf {
        self.v_home().join("current")
    }
    pub fn cache_dir(&self) -> PathBuf {
        self.runtime_root.join("cache").join("v")
    }
    pub fn version_dir(&self, version: &str) -> PathBuf {
        self.versions_dir().join(version)
    }
}

fn first_existing(cands: &[PathBuf]) -> Option<PathBuf> {
    cands.iter().find(|p| p.is_file()).cloned()
}

pub fn v_tool_candidate(home: &Path) -> Option<PathBuf> {
    first_existing(&[
        home.join("v.exe"),
        home.join("v"),
        home.join("bin").join("v.exe"),
        home.join("bin").join("v"),
    ])
}

pub fn v_installation_valid(home: &Path) -> bool {
    v_tool_candidate(home).is_some()
}

pub fn list_installed_versions(paths: &VPaths) -> EnvrResult<Vec<RuntimeVersion>> {
    let dir = paths.versions_dir();
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for e in fs::read_dir(&dir).map_err(EnvrError::from)? {
        let e = e.map_err(EnvrError::from)?;
        if !e.file_type().map_err(EnvrError::from)?.is_dir() {
            continue;
        }
        let p = e.path();
        if v_installation_valid(&p) {
            out.push(RuntimeVersion(e.file_name().to_string_lossy().into_owned()));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

pub fn read_current(paths: &VPaths) -> EnvrResult<Option<RuntimeVersion>> {
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
    let mut response = client
        .get(url)
        .send()
        .map_err(|e| EnvrError::with_source(ErrorCode::Download, format!("request failed for {url}"), e))?;
    if !response.status().is_success() {
        return Err(EnvrError::Download(format!(
            "GET {url} -> {}",
            response.status()
        )));
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
        let n = response
            .read(&mut buf)
            .map_err(|e| {
                EnvrError::with_source(ErrorCode::Download, format!("read response body failed for {url}"), e)
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

fn promote_v_extracted_tree(staging: &Path, final_dir: &Path) -> EnvrResult<()> {
    use envr_platform::install_layout;
    install_layout::ensure_final_parent(final_dir)?;
    let staging_final = install_layout::sibling_staging_path(final_dir)?;
    install_layout::remove_if_exists(&staging_final)?;

    let mut iter = fs::read_dir(staging).map_err(EnvrError::from)?;
    let first = iter.next().transpose().map_err(EnvrError::from)?;
    let only_one = iter.next().transpose().map_err(EnvrError::from)?.is_none();
    if let (Some(root), true) = (first, only_one) {
        let root_path = root.path();
        if root_path.is_dir() && v_installation_valid(&root_path) {
            fs::rename(&root_path, &staging_final).map_err(EnvrError::from)?;
            install_layout::commit_staging_dir(&staging_final, final_dir)?;
            return Ok(());
        }
    }

    fs::create_dir_all(&staging_final).map_err(EnvrError::from)?;
    install_layout::hoist_directory_children(staging, &staging_final)?;
    if !v_installation_valid(&staging_final) {
        let _ = fs::remove_dir_all(&staging_final);
        return Err(EnvrError::Validation(
            "extracted V layout missing v executable".into(),
        ));
    }
    install_layout::commit_staging_dir(&staging_final, final_dir)?;
    Ok(())
}

pub struct VManager {
    pub paths: VPaths,
    releases_api_url: String,
    client: reqwest::blocking::Client,
}

impl VManager {
    pub fn try_new(runtime_root: PathBuf, releases_api_url: String) -> EnvrResult<Self> {
        Ok(Self {
            paths: VPaths::new(runtime_root),
            releases_api_url,
            client: blocking_http_client()?,
        })
    }

    fn index_ttl_secs() -> u64 {
        const DEFAULT: u64 = 6 * 60 * 60;
        std::env::var("ENVR_V_INDEX_CACHE_TTL_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT)
    }

    fn cache_path(&self) -> PathBuf {
        self.paths.cache_dir().join("github_releases.json")
    }

    fn load_rows(&self) -> EnvrResult<Vec<VInstallableRow>> {
        let ttl = Self::index_ttl_secs();
        let cache_path = self.cache_path();
        if let Ok(meta) = fs::metadata(&cache_path)
            && let Ok(modified) = meta.modified()
            && let Ok(age) = SystemTime::now().duration_since(modified)
            && age.as_secs() <= ttl
            && let Ok(body) = fs::read_to_string(&cache_path)
            && let Ok(rows) = serde_json::from_str::<Vec<VInstallableRow>>(&body)
            && !rows.is_empty()
        {
            return Ok(rows);
        }
        let releases = fetch_v_github_releases_index(&self.client, &self.releases_api_url)?;
        let rows = installable_rows_from_releases(&releases);
        if rows.is_empty() {
            return Err(EnvrError::Validation(
                "v: no installable stable releases for this host".into(),
            ));
        }
        let _ = (|| -> EnvrResult<()> {
            fs::create_dir_all(self.paths.cache_dir())?;
            let s =
                serde_json::to_string(&rows).map_err(|e| {
                    EnvrError::with_source(ErrorCode::Validation, "json encode v rows", e)
                })?;
            envr_platform::fs_atomic::write_atomic(&cache_path, s.as_bytes())?;
            Ok(())
        })();
        Ok(rows)
    }

    pub fn list_remote(&self, filter: &RemoteFilter) -> EnvrResult<Vec<RuntimeVersion>> {
        let rows = self.load_rows()?;
        Ok(list_remote_versions(&rows, filter))
    }

    pub fn list_remote_latest_per_major(&self) -> EnvrResult<Vec<RuntimeVersion>> {
        let rows = self.load_rows()?;
        Ok(list_remote_latest_per_major_lines(&rows))
    }

    pub fn resolve_label(&self, spec: &str) -> EnvrResult<String> {
        let rows = self.load_rows()?;
        resolve_v_version(&rows, spec)
    }

    pub fn install_from_spec(&self, request: &InstallRequest) -> EnvrResult<RuntimeVersion> {
        let rows = self.load_rows()?;
        let label = resolve_v_version(&rows, &request.spec.0)?;
        let row = rows
            .iter()
            .find(|r| r.version == label)
            .ok_or_else(|| EnvrError::Validation(format!("v release `{label}` not found")))?;
        let cache_dir = self.paths.cache_dir().join(&label);
        fs::create_dir_all(&cache_dir).map_err(EnvrError::from)?;
        let archive_path = cache_dir.join("v.zip");
        download_to_path(
            &self.client,
            &row.url,
            &archive_path,
            request.progress_downloaded.as_ref(),
            request.progress_total.as_ref(),
            request.cancel.as_ref(),
        )?;
        let staging_parent = cache_dir.join("extract_staging");
        fs::create_dir_all(&staging_parent).map_err(EnvrError::from)?;
        let staging = tempfile::tempdir_in(&staging_parent).map_err(EnvrError::from)?;
        extract::extract_archive(&archive_path, staging.path())?;
        let final_dir = self.paths.version_dir(&label);
        promote_v_extracted_tree(staging.path(), &final_dir)?;
        self.set_current(&RuntimeVersion(label.clone()))?;
        Ok(RuntimeVersion(label))
    }

    pub fn set_current(&self, version: &RuntimeVersion) -> EnvrResult<()> {
        let dir = self.paths.version_dir(&version.0);
        if !v_installation_valid(&dir) {
            return Err(EnvrError::Validation(format!(
                "v {} is not installed under {}",
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

