use crate::index::{
    ElmInstallableRow, blocking_http_client, fetch_elm_installable_rows_with_fallback,
    list_remote_latest_per_major_lines, list_remote_versions, resolve_elm_version,
};
use envr_domain::runtime::{InstallRequest, RemoteFilter, RuntimeVersion};
use envr_error::{EnvrError, EnvrResult};
use envr_platform::links::ensure_runtime_current_symlink_or_pointer;
use flate2::read::GzDecoder;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::SystemTime;

#[derive(Debug, Clone)]
pub struct ElmPaths {
    runtime_root: PathBuf,
}
impl ElmPaths {
    pub fn new(runtime_root: PathBuf) -> Self {
        Self { runtime_root }
    }
    pub fn home(&self) -> PathBuf {
        self.runtime_root.join("runtimes").join("elm")
    }
    pub fn versions_dir(&self) -> PathBuf {
        self.home().join("versions")
    }
    pub fn current_link(&self) -> PathBuf {
        self.home().join("current")
    }
    pub fn cache_dir(&self) -> PathBuf {
        self.runtime_root.join("cache").join("elm")
    }
    pub fn version_dir(&self, version: &str) -> PathBuf {
        self.versions_dir().join(version)
    }
    pub fn releases_cache_path(&self) -> PathBuf {
        self.cache_dir().join("releases.json")
    }
    pub fn latest_cache_path(&self) -> PathBuf {
        self.cache_dir().join("latest_per_major.json")
    }
}

fn first_existing(cands: &[PathBuf]) -> Option<PathBuf> {
    cands.iter().find(|p| p.is_file()).cloned()
}
pub fn elm_tool_candidate(home: &Path) -> Option<PathBuf> {
    first_existing(&[
        home.join("elm.exe"),
        home.join("elm"),
        home.join("bin").join("elm.exe"),
        home.join("bin").join("elm"),
    ])
}
pub fn elm_installation_valid(home: &Path) -> bool {
    elm_tool_candidate(home).is_some()
}

pub fn list_installed_versions(paths: &ElmPaths) -> EnvrResult<Vec<RuntimeVersion>> {
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
        if elm_installation_valid(&p) {
            out.push(RuntimeVersion(e.file_name().to_string_lossy().into_owned()));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

pub fn read_current(paths: &ElmPaths) -> EnvrResult<Option<RuntimeVersion>> {
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
        let name = resolved.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
        return if name.is_empty() { Ok(None) } else { Ok(Some(RuntimeVersion(name))) };
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
    let name = resolved.file_name().and_then(|s| s.to_str()).unwrap_or("").to_string();
    if name.is_empty() {
        Ok(None)
    } else {
        Ok(Some(RuntimeVersion(name)))
    }
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
    let mut response = client.get(url).send().map_err(|e| EnvrError::Download(e.to_string()))?;
    if !response.status().is_success() {
        return Err(EnvrError::Download(format!("GET {url} -> {}", response.status())));
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
        let n = response.read(&mut buf).map_err(|e| EnvrError::Download(e.to_string()))?;
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

fn inflate_gz_to_executable(gz_path: &Path, out_exe: &Path) -> EnvrResult<()> {
    let src = fs::File::open(gz_path).map_err(EnvrError::from)?;
    let mut decoder = GzDecoder::new(src);
    let mut out = fs::File::create(out_exe).map_err(EnvrError::from)?;
    std::io::copy(&mut decoder, &mut out).map_err(EnvrError::from)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = out.metadata().map_err(EnvrError::from)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(out_exe, perms).map_err(EnvrError::from)?;
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub struct ElmManager {
    paths: ElmPaths,
    releases_api_url: String,
}
impl ElmManager {
    pub fn try_new(runtime_root: PathBuf, releases_api_url: String) -> EnvrResult<Self> {
        Ok(Self {
            paths: ElmPaths::new(runtime_root),
            releases_api_url,
        })
    }
    fn index_cache_ttl_secs() -> u64 {
        std::env::var("ENVR_ELM_RELEASES_CACHE_TTL_SECS")
            .ok()
            .or_else(|| std::env::var("ENVR_ELM_INDEX_CACHE_TTL_SECS").ok())
            .and_then(|s| s.trim().parse::<u64>().ok())
            .unwrap_or(3600)
    }
    fn latest_cache_ttl_secs() -> u64 {
        std::env::var("ENVR_ELM_REMOTE_CACHE_TTL_SECS")
            .ok()
            .and_then(|s| s.trim().parse::<u64>().ok())
            .unwrap_or(86400)
    }
    fn load_cached_rows(&self) -> Option<Vec<ElmInstallableRow>> {
        let path = self.paths.releases_cache_path();
        let meta = fs::metadata(&path).ok()?;
        let age = SystemTime::now().duration_since(meta.modified().ok()?).ok()?.as_secs();
        if age > Self::index_cache_ttl_secs() {
            return None;
        }
        let text = fs::read_to_string(&path).ok()?;
        serde_json::from_str::<Vec<ElmInstallableRow>>(&text).ok()
    }
    fn save_cached_rows(&self, rows: &[ElmInstallableRow]) -> EnvrResult<()> {
        fs::create_dir_all(self.paths.cache_dir()).map_err(EnvrError::from)?;
        let text = serde_json::to_string_pretty(rows).map_err(|e| EnvrError::Download(e.to_string()))?;
        fs::write(self.paths.releases_cache_path(), text).map_err(EnvrError::from)?;
        Ok(())
    }
    fn fetch_rows(&self, force_refresh: bool) -> EnvrResult<Vec<ElmInstallableRow>> {
        if !force_refresh && let Some(rows) = self.load_cached_rows() {
            return Ok(rows);
        }
        let client = blocking_http_client()?;
        let rows = fetch_elm_installable_rows_with_fallback(&client, &self.releases_api_url)?;
        self.save_cached_rows(&rows)?;
        Ok(rows)
    }
    pub fn list_remote(&self, filter: &RemoteFilter) -> EnvrResult<Vec<RuntimeVersion>> {
        Ok(list_remote_versions(&self.fetch_rows(false)?, filter))
    }
    pub fn list_remote_latest_per_major(&self) -> EnvrResult<Vec<RuntimeVersion>> {
        let path = self.paths.latest_cache_path();
        if let Ok(meta) = fs::metadata(&path)
            && let Ok(age) = SystemTime::now().duration_since(meta.modified().map_err(EnvrError::from)?)
            && age.as_secs() <= Self::latest_cache_ttl_secs()
            && let Ok(text) = fs::read_to_string(&path)
            && let Ok(v) = serde_json::from_str::<Vec<String>>(&text)
        {
            return Ok(v.into_iter().map(RuntimeVersion).collect());
        }
        let latest = list_remote_latest_per_major_lines(&self.fetch_rows(false)?);
        fs::create_dir_all(self.paths.cache_dir()).map_err(EnvrError::from)?;
        let labels: Vec<String> = latest.iter().map(|v| v.0.clone()).collect();
        let text = serde_json::to_string_pretty(&labels).map_err(|e| EnvrError::Download(e.to_string()))?;
        fs::write(&path, text).map_err(EnvrError::from)?;
        Ok(latest)
    }
    pub fn resolve_label(&self, spec: &str) -> EnvrResult<String> {
        resolve_elm_version(&self.fetch_rows(false)?, spec)
            .ok_or_else(|| EnvrError::Validation(format!("unknown elm version spec: {spec}")))
    }
    pub fn set_current(&self, version: &RuntimeVersion) -> EnvrResult<()> {
        let dir = self.paths.version_dir(&version.0);
        if !dir.is_dir() || !elm_installation_valid(&dir) {
            return Err(EnvrError::Validation(format!("elm version not installed: {}", version.0)));
        }
        ensure_runtime_current_symlink_or_pointer(&dir, &self.paths.current_link())
    }
    pub fn uninstall(&self, version: &RuntimeVersion) -> EnvrResult<()> {
        let dir = self.paths.version_dir(&version.0);
        if dir.exists() {
            fs::remove_dir_all(&dir).map_err(EnvrError::from)?;
        }
        Ok(())
    }
    pub fn install_from_spec(&self, request: &InstallRequest) -> EnvrResult<RuntimeVersion> {
        let label = self.resolve_label(&request.spec.0)?;
        let rows = self.fetch_rows(false)?;
        let row = rows
            .iter()
            .find(|r| r.version == label)
            .ok_or_else(|| EnvrError::Validation(format!("elm version not found in index: {label}")))?;
        let final_dir = self.paths.version_dir(&label);
        if final_dir.is_dir() && elm_installation_valid(&final_dir) {
            return Ok(RuntimeVersion(label));
        }
        fs::create_dir_all(&final_dir).map_err(EnvrError::from)?;
        let client = blocking_http_client()?;
        let tmp = tempfile::tempdir().map_err(EnvrError::from)?;
        let archive_name = row.url.split('/').next_back().unwrap_or("elm.gz");
        let archive_path = tmp.path().join(archive_name);
        download_to_path(
            &client,
            &row.url,
            &archive_path,
            request.progress_downloaded.as_ref(),
            request.progress_total.as_ref(),
            request.cancel.as_ref(),
        )?;
        #[cfg(windows)]
        let exe_name = "elm.exe";
        #[cfg(not(windows))]
        let exe_name = "elm";
        let out_exe = final_dir.join(exe_name);
        inflate_gz_to_executable(&archive_path, &out_exe)?;
        if !elm_installation_valid(&final_dir) {
            return Err(EnvrError::Validation("elm install validation failed".into()));
        }
        Ok(RuntimeVersion(label))
    }
}

