use crate::index::{
    UnisonInstallableRow, blocking_http_client, fetch_unison_installable_rows_with_fallback,
    list_remote_latest_per_major_lines, list_remote_versions, resolve_unison_version,
};
use envr_domain::installer::{SpecDrivenInstaller, install_progress_handles};
use envr_domain::runtime::{InstallRequest, RemoteFilter, RuntimeVersion};
use envr_download::extract;
use envr_error::{EnvrError, EnvrResult, ErrorCode};
use envr_platform::links::ensure_runtime_current_symlink_or_pointer;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Debug, Clone)]
pub struct UnisonPaths {
    runtime_root: PathBuf,
}

impl UnisonPaths {
    pub fn new(runtime_root: PathBuf) -> Self {
        Self { runtime_root }
    }
    pub fn home(&self) -> PathBuf {
        self.runtime_root.join("runtimes").join("unison")
    }
    pub fn versions_dir(&self) -> PathBuf {
        self.home().join("versions")
    }
    pub fn current_link(&self) -> PathBuf {
        self.home().join("current")
    }
    pub fn cache_dir(&self) -> PathBuf {
        self.runtime_root.join("cache").join("unison")
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

pub fn ucm_tool_candidate(home: &Path) -> Option<PathBuf> {
    first_existing(&[
        home.join("ucm.exe"),
        home.join("ucm.cmd"),
        home.join("ucm.bat"),
        home.join("ucm"),
        home.join("bin").join("ucm.exe"),
        home.join("bin").join("ucm.cmd"),
        home.join("bin").join("ucm.bat"),
        home.join("bin").join("ucm"),
    ])
}

pub fn unison_installation_valid(home: &Path) -> bool {
    ucm_tool_candidate(home).is_some()
}

fn unison_candidate_dirs(staging: &Path) -> EnvrResult<Vec<PathBuf>> {
    // Some upstream archives nest the real payload multiple levels deep. Keep the scan bounded
    // to avoid pathological trees.
    const MAX_DEPTH: usize = 8;

    fn rec(base: &Path, cur: &Path, depth: usize, out: &mut Vec<PathBuf>) -> Result<(), EnvrError> {
        if depth > MAX_DEPTH {
            return Ok(());
        }
        if cur.is_dir() {
            out.push(cur.to_path_buf());
        }
        let entries = match fs::read_dir(cur) {
            Ok(v) => v,
            Err(e) => return Err(EnvrError::from(e)),
        };
        for e in entries {
            let e = e.map_err(EnvrError::from)?;
            let ft = e.file_type().map_err(EnvrError::from)?;
            if !ft.is_dir() {
                continue;
            }
            let p = e.path();
            // Safety: avoid following odd paths that escape the staging root.
            if p.strip_prefix(base).is_err() {
                continue;
            }
            rec(base, &p, depth + 1, out)?;
        }
        Ok(())
    }

    let mut out = Vec::new();
    if staging.is_dir() {
        rec(staging, staging, 0, &mut out)?;
    }
    out.sort();
    out.dedup();
    Ok(out)
}

fn depth_key(staging: &Path, p: &Path) -> (usize, String) {
    let depth = p
        .strip_prefix(staging)
        .map(|r| r.components().count())
        .unwrap_or(usize::MAX);
    (depth, p.display().to_string())
}

fn pick_ucm_home_from_candidates(staging: &Path, candidates: &[PathBuf]) -> EnvrResult<PathBuf> {
    let mut val: Vec<PathBuf> = candidates
        .iter()
        .filter(|p| unison_installation_valid(p))
        .cloned()
        .collect();
    match val.len() {
        0 => Err(EnvrError::Validation(
            "extracted unison layout missing ucm executable".into(),
        )),
        1 => Ok(val[0].clone()),
        _ => {
            val.sort_by(|a, b| depth_key(staging, a).cmp(&depth_key(staging, b)));
            Ok(val[0].clone())
        }
    }
}

fn promote_ucm_extract(staging: &Path, final_dir: &Path) -> EnvrResult<()> {
    use envr_platform::install_layout;
    install_layout::ensure_final_parent(final_dir)?;
    let staging_final = install_layout::sibling_staging_path(final_dir)?;
    install_layout::remove_if_exists(&staging_final)?;
    let commit_if_valid = |home: &Path| -> EnvrResult<()> {
        if !unison_installation_valid(home) {
            return Err(EnvrError::Validation(
                "extracted unison layout missing ucm executable".into(),
            ));
        }
        install_layout::commit_staging_dir(home, final_dir)
    };
    if unison_installation_valid(staging) {
        fs::create_dir_all(&staging_final).map_err(EnvrError::from)?;
        install_layout::hoist_directory_children(staging, &staging_final)?;
        return commit_if_valid(&staging_final);
    }
    let cands = unison_candidate_dirs(staging)?;
    let inner = pick_ucm_home_from_candidates(staging, &cands)?;
    if inner == staging {
        fs::create_dir_all(&staging_final).map_err(EnvrError::from)?;
        install_layout::hoist_directory_children(staging, &staging_final)?;
        return commit_if_valid(&staging_final);
    }
    // Keep the "version home" stable: hoist the chosen payload dir's children into our staging
    // target instead of renaming only that subdir (which can drop sibling files that `ucm` needs).
    fs::create_dir_all(&staging_final).map_err(EnvrError::from)?;
    install_layout::hoist_directory_children(&inner, &staging_final)?;
    commit_if_valid(&staging_final)
}

pub fn list_installed_versions(paths: &UnisonPaths) -> EnvrResult<Vec<RuntimeVersion>> {
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
        if unison_installation_valid(&p) {
            out.push(RuntimeVersion(e.file_name().to_string_lossy().into_owned()));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

pub fn read_current(paths: &UnisonPaths) -> EnvrResult<Option<RuntimeVersion>> {
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
    if name.is_empty() { Ok(None) } else { Ok(Some(RuntimeVersion(name))) }
}

#[derive(Debug, Clone)]
pub struct UnisonManager {
    paths: UnisonPaths,
    releases_api_url: String,
}

impl UnisonManager {
    pub fn try_new(runtime_root: PathBuf, releases_api_url: String) -> EnvrResult<Self> {
        Ok(Self {
            paths: UnisonPaths::new(runtime_root),
            releases_api_url,
        })
    }

    fn index_cache_ttl_secs() -> u64 {
        std::env::var("ENVR_UNISON_RELEASES_CACHE_TTL_SECS")
            .ok()
            .or_else(|| std::env::var("ENVR_UNISON_INDEX_CACHE_TTL_SECS").ok())
            .and_then(|s| s.trim().parse::<u64>().ok())
            .unwrap_or(3600)
    }

    fn latest_cache_ttl_secs() -> u64 {
        std::env::var("ENVR_UNISON_REMOTE_CACHE_TTL_SECS")
            .ok()
            .and_then(|s| s.trim().parse::<u64>().ok())
            .unwrap_or(86400)
    }

    fn load_cached_rows(&self) -> Option<Vec<UnisonInstallableRow>> {
        let path = self.paths.releases_cache_path();
        let meta = fs::metadata(&path).ok()?;
        let age = SystemTime::now().duration_since(meta.modified().ok()?).ok()?.as_secs();
        if age > Self::index_cache_ttl_secs() {
            return None;
        }
        let text = fs::read_to_string(&path).ok()?;
        serde_json::from_str::<Vec<UnisonInstallableRow>>(&text).ok()
    }

    fn save_cached_rows(&self, rows: &[UnisonInstallableRow]) -> EnvrResult<()> {
        fs::create_dir_all(self.paths.cache_dir()).map_err(EnvrError::from)?;
        let text =
            serde_json::to_string_pretty(rows)
                .map_err(|e| EnvrError::with_source(ErrorCode::Download, "serialize unison rows cache", e))?;
        fs::write(self.paths.releases_cache_path(), text).map_err(EnvrError::from)?;
        let _ = fs::remove_file(self.paths.latest_cache_path());
        Ok(())
    }

    fn fetch_rows(&self, force_refresh: bool) -> EnvrResult<Vec<UnisonInstallableRow>> {
        if !force_refresh && let Some(rows) = self.load_cached_rows() {
            return Ok(rows);
        }
        let client = blocking_http_client()?;
        let rows = fetch_unison_installable_rows_with_fallback(&client, &self.releases_api_url)?;
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
        let fresh_set: BTreeSet<String> = fresh.iter().map(|v| v.0.clone()).collect();

        let path = self.paths.latest_cache_path();
        if let Ok(meta) = fs::metadata(&path)
            && let Ok(age) =
                SystemTime::now().duration_since(meta.modified().map_err(EnvrError::from)?)
            && age.as_secs() <= Self::latest_cache_ttl_secs()
            && let Ok(text) = fs::read_to_string(&path)
            && let Ok(cached) = serde_json::from_str::<Vec<String>>(&text)
        {
            let cached_set: BTreeSet<String> = cached.iter().cloned().collect();
            if cached_set == fresh_set {
                return Ok(cached.into_iter().map(RuntimeVersion).collect());
            }
        }

        fs::create_dir_all(self.paths.cache_dir()).map_err(EnvrError::from)?;
        let labels: Vec<String> = fresh.iter().map(|v| v.0.clone()).collect();
        let text =
            serde_json::to_string_pretty(&labels)
                .map_err(|e| EnvrError::with_source(ErrorCode::Download, "serialize unison latest cache", e))?;
        fs::write(&path, text).map_err(EnvrError::from)?;
        Ok(fresh)
    }

    pub fn resolve_label(&self, spec: &str) -> EnvrResult<String> {
        let rows = self.fetch_rows(false)?;
        if let Some(v) = resolve_unison_version(&rows, spec) {
            return Ok(v);
        }
        let rows = self.fetch_rows(true)?;
        resolve_unison_version(&rows, spec).ok_or_else(|| {
            EnvrError::Validation(format!("unknown unison version spec: {spec}"))
        })
    }

    pub fn set_current(&self, version: &RuntimeVersion) -> EnvrResult<()> {
        let dir = self.paths.version_dir(&version.0);
        if !dir.is_dir() || !unison_installation_valid(&dir) {
            return Err(EnvrError::Validation(format!(
                "unison version not installed: {}",
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

impl SpecDrivenInstaller for UnisonManager {
    fn install_from_spec(&self, request: &InstallRequest) -> EnvrResult<RuntimeVersion> {
        let label = self.resolve_label(&request.spec.0)?;
        let rows = self.fetch_rows(false)?;
        let row = rows
            .iter()
            .find(|r| r.version == label)
            .ok_or_else(|| EnvrError::Validation(format!("unison version not found in index: {label}")))?;

        let final_dir = self.paths.version_dir(&label);
        if final_dir.is_dir() && unison_installation_valid(&final_dir) {
            return Ok(RuntimeVersion(label));
        }

        fs::create_dir_all(self.paths.cache_dir()).map_err(EnvrError::from)?;
        let client = blocking_http_client()?;

        let archive_ext = if row.url.ends_with(".zip") { "zip" } else { "tar.gz" };
        let cache = self.paths.cache_dir().join(format!(
            "ucm-{}.{}",
            label.replace(['/', '\\'], "_"),
            archive_ext
        ));
        let (downloaded, total, cancel) = install_progress_handles(request);
        envr_download::blocking::download_url_to_path_resumable(
            &client,
            &row.url,
            &cache,
            downloaded,
            total,
            cancel,
        )?;

        let staging_parent = self.paths.cache_dir().join("extract_staging");
        fs::create_dir_all(&staging_parent).map_err(EnvrError::from)?;
        let staging = tempfile::tempdir_in(&staging_parent).map_err(EnvrError::from)?;
        extract::extract_archive(&cache, staging.path())?;
        promote_ucm_extract(staging.path(), &final_dir)?;

        if !unison_installation_valid(&final_dir) {
            return Err(EnvrError::Validation("unison install validation failed".into()));
        }

        Ok(RuntimeVersion(label))
    }
}

