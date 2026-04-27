use crate::index::{
    LuauInstallableRow, blocking_http_client, fetch_luau_installable_rows,
    list_remote_latest_per_major_lines, list_remote_versions, resolve_luau_version,
};
use envr_domain::installer::{SpecDrivenInstaller, install_progress_handles};
use envr_domain::runtime::{InstallRequest, RemoteFilter, RuntimeVersion};
use envr_download::extract;
use envr_error::{EnvrError, EnvrResult, ErrorCode};
use envr_platform::install_layout;
use envr_platform::links::ensure_runtime_current_symlink_or_pointer;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Debug, Clone)]
pub struct LuauPaths {
    runtime_root: PathBuf,
}

impl LuauPaths {
    pub fn new(runtime_root: PathBuf) -> Self {
        Self { runtime_root }
    }
    pub fn home(&self) -> PathBuf {
        self.runtime_root.join("runtimes").join("luau")
    }
    pub fn versions_dir(&self) -> PathBuf {
        self.home().join("versions")
    }
    pub fn current_link(&self) -> PathBuf {
        self.home().join("current")
    }
    pub fn cache_dir(&self) -> PathBuf {
        self.runtime_root.join("cache").join("luau")
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

pub fn luau_tool_candidate(home: &Path) -> Option<PathBuf> {
    first_existing(&[
        home.join("luau.exe"),
        home.join("luau"),
        home.join("bin").join("luau.exe"),
        home.join("bin").join("luau"),
    ])
}

pub fn luau_analyze_tool_candidate(home: &Path) -> Option<PathBuf> {
    first_existing(&[
        home.join("luau-analyze.exe"),
        home.join("luau-analyze"),
        home.join("bin").join("luau-analyze.exe"),
        home.join("bin").join("luau-analyze"),
    ])
}

pub fn luau_installation_valid(home: &Path) -> bool {
    luau_tool_candidate(home).is_some()
}

fn find_distribution_root(staging: &Path) -> Option<PathBuf> {
    if luau_installation_valid(staging) {
        return Some(staging.to_path_buf());
    }
    let entries = fs::read_dir(staging).ok()?;
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() && luau_installation_valid(&p) {
            return Some(p);
        }
    }
    None
}

fn promote_luau_extract(staging: &Path, final_dir: &Path) -> EnvrResult<()> {
    let root = find_distribution_root(staging).ok_or_else(|| {
        EnvrError::Validation(format!(
            "extracted luau layout missing luau executable under {}",
            staging.display()
        ))
    })?;

    install_layout::ensure_final_parent(final_dir)?;
    let staging_final = install_layout::sibling_staging_path(final_dir)?;
    install_layout::remove_if_exists(&staging_final)?;

    if root == staging {
        fs::create_dir_all(&staging_final).map_err(EnvrError::from)?;
        install_layout::hoist_directory_children(staging, &staging_final)?;
    } else {
        fs::rename(&root, &staging_final).map_err(EnvrError::from)?;
    }

    if !luau_installation_valid(&staging_final) {
        let _ = fs::remove_dir_all(&staging_final);
        return Err(EnvrError::Validation(
            "promoted luau tree missing luau executable".into(),
        ));
    }

    install_layout::commit_staging_dir(&staging_final, final_dir)?;
    Ok(())
}

pub fn list_installed_versions(paths: &LuauPaths) -> EnvrResult<Vec<RuntimeVersion>> {
    let dir = paths.versions_dir();
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for entry in fs::read_dir(&dir).map_err(EnvrError::from)? {
        let entry = entry.map_err(EnvrError::from)?;
        if !entry.file_type().map_err(EnvrError::from)?.is_dir() {
            continue;
        }
        let p = entry.path();
        if luau_installation_valid(&p) {
            out.push(RuntimeVersion(
                entry.file_name().to_string_lossy().into_owned(),
            ));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

pub fn read_current(paths: &LuauPaths) -> EnvrResult<Option<RuntimeVersion>> {
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
        return if name.is_empty() {
            Ok(None)
        } else {
            Ok(Some(RuntimeVersion(name)))
        };
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
        Ok(None)
    } else {
        Ok(Some(RuntimeVersion(name)))
    }
}

#[derive(Debug, Clone)]
pub struct LuauManager {
    paths: LuauPaths,
    releases_api_url: String,
}

impl LuauManager {
    pub fn try_new(runtime_root: PathBuf, releases_api_url: String) -> EnvrResult<Self> {
        Ok(Self {
            paths: LuauPaths::new(runtime_root),
            releases_api_url,
        })
    }

    fn index_cache_ttl_secs() -> u64 {
        std::env::var("ENVR_LUAU_RELEASES_CACHE_TTL_SECS")
            .ok()
            .or_else(|| std::env::var("ENVR_LUAU_INDEX_CACHE_TTL_SECS").ok())
            .and_then(|s| s.trim().parse::<u64>().ok())
            .unwrap_or(3600)
    }

    fn latest_cache_ttl_secs() -> u64 {
        std::env::var("ENVR_LUAU_REMOTE_CACHE_TTL_SECS")
            .ok()
            .and_then(|s| s.trim().parse::<u64>().ok())
            .unwrap_or(86400)
    }

    fn load_cached_rows(&self) -> Option<Vec<LuauInstallableRow>> {
        let path = self.paths.releases_cache_path();
        let meta = fs::metadata(&path).ok()?;
        let age = SystemTime::now()
            .duration_since(meta.modified().ok()?)
            .ok()?
            .as_secs();
        if age > Self::index_cache_ttl_secs() {
            return None;
        }
        let text = fs::read_to_string(&path).ok()?;
        serde_json::from_str::<Vec<LuauInstallableRow>>(&text).ok()
    }

    fn save_cached_rows(&self, rows: &[LuauInstallableRow]) -> EnvrResult<()> {
        fs::create_dir_all(self.paths.cache_dir()).map_err(EnvrError::from)?;
        let text = serde_json::to_string_pretty(rows).map_err(|e| {
            EnvrError::with_source(ErrorCode::Download, "serialize luau rows cache", e)
        })?;
        fs::write(self.paths.releases_cache_path(), text).map_err(EnvrError::from)?;
        let _ = fs::remove_file(self.paths.latest_cache_path());
        Ok(())
    }

    fn fetch_rows(&self, force_refresh: bool) -> EnvrResult<Vec<LuauInstallableRow>> {
        if !force_refresh && let Some(rows) = self.load_cached_rows() {
            return Ok(rows);
        }
        let client = blocking_http_client()?;
        let rows = fetch_luau_installable_rows(&client, &self.releases_api_url)?;
        self.save_cached_rows(&rows)?;
        Ok(rows)
    }

    pub fn list_remote(&self, filter: &RemoteFilter) -> EnvrResult<Vec<RuntimeVersion>> {
        Ok(list_remote_versions(
            &self.fetch_rows(filter.force_index_refresh)?,
            filter,
        ))
    }

    pub fn list_remote_latest_per_major(&self) -> EnvrResult<Vec<RuntimeVersion>> {
        let rows = self.fetch_rows(false)?;
        let fresh = list_remote_latest_per_major_lines(&rows);
        let path = self.paths.latest_cache_path();
        if let Ok(meta) = fs::metadata(&path)
            && let Ok(age) =
                SystemTime::now().duration_since(meta.modified().map_err(EnvrError::from)?)
            && age.as_secs() <= Self::latest_cache_ttl_secs()
            && let Ok(text) = fs::read_to_string(&path)
            && let Ok(cached) = serde_json::from_str::<Vec<String>>(&text)
        {
            return Ok(cached.into_iter().map(RuntimeVersion).collect());
        }
        fs::create_dir_all(self.paths.cache_dir()).map_err(EnvrError::from)?;
        let labels: Vec<String> = fresh.iter().map(|v| v.0.clone()).collect();
        let text = serde_json::to_string_pretty(&labels).map_err(|e| {
            EnvrError::with_source(ErrorCode::Download, "serialize luau latest cache", e)
        })?;
        fs::write(&path, text).map_err(EnvrError::from)?;
        Ok(fresh)
    }

    pub fn resolve_label(&self, spec: &str) -> EnvrResult<String> {
        let rows = self.fetch_rows(false)?;
        if let Some(v) = resolve_luau_version(&rows, spec) {
            return Ok(v);
        }
        let rows = self.fetch_rows(true)?;
        resolve_luau_version(&rows, spec)
            .ok_or_else(|| EnvrError::Validation(format!("unknown luau version spec: {spec}")))
    }

    pub fn set_current(&self, version: &RuntimeVersion) -> EnvrResult<()> {
        let dir = self.paths.version_dir(&version.0);
        if !dir.is_dir() || !luau_installation_valid(&dir) {
            return Err(EnvrError::Validation(format!(
                "luau version not installed: {}",
                version.0
            )));
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
}

impl SpecDrivenInstaller for LuauManager {
    fn install_from_spec(&self, request: &InstallRequest) -> EnvrResult<RuntimeVersion> {
        let label = self.resolve_label(&request.spec.0)?;
        let rows = self.fetch_rows(false)?;
        let row = rows.iter().find(|r| r.version == label).ok_or_else(|| {
            EnvrError::Validation(format!("luau version not found in index: {label}"))
        })?;

        let final_dir = self.paths.version_dir(&label);
        if final_dir.is_dir() && luau_installation_valid(&final_dir) {
            return Ok(RuntimeVersion(label));
        }

        fs::create_dir_all(self.paths.cache_dir()).map_err(EnvrError::from)?;
        let client = blocking_http_client()?;
        let cache = self
            .paths
            .cache_dir()
            .join(format!("luau-{}.zip", label.replace(['/', '\\'], "_")));
        let (downloaded, total, cancel) = install_progress_handles(request);
        envr_download::blocking::download_url_to_path_resumable(
            &client, &row.url, &cache, downloaded, total, cancel,
        )?;

        let staging_parent = self.paths.cache_dir().join("extract_staging");
        fs::create_dir_all(&staging_parent).map_err(EnvrError::from)?;
        let staging = tempfile::tempdir_in(&staging_parent).map_err(EnvrError::from)?;
        extract::extract_archive(&cache, staging.path())?;
        promote_luau_extract(staging.path(), &final_dir)?;

        if !luau_installation_valid(&final_dir) {
            return Err(EnvrError::Validation(
                "luau install validation failed".into(),
            ));
        }

        Ok(RuntimeVersion(label))
    }
}
