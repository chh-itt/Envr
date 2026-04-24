use crate::index::{
    SbclInstallableRow, blocking_http_client, fetch_sbcl_installable_rows_with_fallback,
    list_remote_latest_per_major_lines, list_remote_versions, resolve_sbcl_version,
};
use envr_domain::installer::{SpecDrivenInstaller, install_progress_handles};
use envr_domain::runtime::{InstallRequest, RemoteFilter, RuntimeVersion};
use envr_download::extract;
use envr_error::{EnvrError, EnvrResult, ErrorCode};
use envr_platform::install_layout;
use envr_platform::links::ensure_runtime_current_symlink_or_pointer;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
#[cfg(windows)]
use std::process::Command;
use std::time::SystemTime;

#[derive(Debug, Clone)]
pub struct SbclPaths {
    runtime_root: PathBuf,
}

impl SbclPaths {
    pub fn new(runtime_root: PathBuf) -> Self {
        Self { runtime_root }
    }
    pub fn sbcl_home(&self) -> PathBuf {
        self.runtime_root.join("runtimes").join("sbcl")
    }
    pub fn versions_dir(&self) -> PathBuf {
        self.sbcl_home().join("versions")
    }
    pub fn current_link(&self) -> PathBuf {
        self.sbcl_home().join("current")
    }
    pub fn cache_dir(&self) -> PathBuf {
        self.runtime_root.join("cache").join("sbcl")
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

#[cfg(windows)]
fn find_named_executable_recursive(root: &Path, base: &str) -> Option<PathBuf> {
    let with_ext = format!("{base}.exe");
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = fs::read_dir(&dir) else {
            continue;
        };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_file() {
                if p.file_name().and_then(|n| n.to_str()) == Some(base)
                    || p.file_name().and_then(|n| n.to_str()) == Some(with_ext.as_str())
                {
                    return Some(p);
                }
            } else if p.is_dir() {
                stack.push(p);
            }
        }
    }
    None
}

pub fn sbcl_tool_candidate(home: &Path) -> Option<PathBuf> {
    // Common layouts:
    // - <root>/bin/sbcl(.exe)
    // - <root>/sbcl(.exe)
    // - <root>/<nested>/bin/sbcl
    first_existing(&[
        home.join("sbcl.exe"),
        home.join("sbcl"),
        home.join("bin").join("sbcl.exe"),
        home.join("bin").join("sbcl"),
    ])
}

pub fn sbcl_installation_valid(home: &Path) -> bool {
    sbcl_tool_candidate(home).is_some()
}

#[cfg(windows)]
fn install_from_msi_admin_unpack(msi: &Path, final_dir: &Path) -> EnvrResult<()> {
    let staging_parent = msi
        .parent()
        .map(|p| p.join("msi_admin_unpack"))
        .unwrap_or_else(|| PathBuf::from("msi_admin_unpack"));
    fs::create_dir_all(&staging_parent).map_err(EnvrError::from)?;
    let staging = tempfile::tempdir_in(&staging_parent).map_err(EnvrError::from)?;
    let target = staging.path();

    let status = Command::new("msiexec")
        .args([
            "/a",
            &msi.display().to_string(),
            "/qn",
            &format!("TARGETDIR={}", target.display()),
        ])
        .status()
        .map_err(|e| {
            EnvrError::with_source(
                ErrorCode::Runtime,
                "failed to spawn msiexec for SBCL MSI (is msiexec on PATH?)",
                e,
            )
        })?;
    if !status.success() {
        return Err(EnvrError::Runtime(format!(
            "msiexec administrative unpack failed with status {status} for {}",
            msi.display()
        )));
    }

    let sbcl_exe = find_named_executable_recursive(target, "sbcl").ok_or_else(|| {
        EnvrError::Validation(format!(
            "MSI unpack did not contain sbcl.exe under {}",
            target.display()
        ))
    })?;
    let src_dir = sbcl_exe.parent().ok_or_else(|| {
        EnvrError::Validation("sbcl.exe from MSI has no parent directory".into())
    })?;

    install_layout::ensure_final_parent(final_dir)?;
    let staging_final = install_layout::sibling_staging_path(final_dir)?;
    install_layout::remove_if_exists(&staging_final)?;
    install_layout::move_dir(src_dir, &staging_final)?;
    install_layout::commit_staging_dir(&staging_final, final_dir)?;

    if !sbcl_installation_valid(final_dir) {
        return Err(EnvrError::Validation(
            "sbcl MSI install produced no sbcl.exe under managed layout".into(),
        ));
    }
    Ok(())
}

pub fn list_installed_versions(paths: &SbclPaths) -> EnvrResult<Vec<RuntimeVersion>> {
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
        if sbcl_installation_valid(&p) {
            out.push(RuntimeVersion(e.file_name().to_string_lossy().into_owned()));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

pub fn read_current(paths: &SbclPaths) -> EnvrResult<Option<RuntimeVersion>> {
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

fn promote_sbcl_extracted_tree(staging: &Path, final_dir: &Path) -> EnvrResult<()> {
    install_layout::ensure_final_parent(final_dir)?;
    let staging_final = install_layout::sibling_staging_path(final_dir)?;
    install_layout::remove_if_exists(&staging_final)?;

    if sbcl_installation_valid(staging) {
        install_layout::move_dir(staging, &staging_final)?;
        install_layout::commit_staging_dir(&staging_final, final_dir)?;
        return Ok(());
    }

    let mut iter = fs::read_dir(staging).map_err(EnvrError::from)?;
    let first = iter.next().transpose().map_err(EnvrError::from)?;
    let only_one = iter.next().transpose().map_err(EnvrError::from)?.is_none();
    if let (Some(root), true) = (first, only_one) {
        let root_path = root.path();
        if root_path.is_dir() && sbcl_installation_valid(&root_path) {
            install_layout::move_dir(&root_path, &staging_final)?;
            install_layout::commit_staging_dir(&staging_final, final_dir)?;
            return Ok(());
        }
        // Some SBCL archives contain `sbcl-<ver>-<tuple>/` with `bin/sbcl`.
        if root_path.is_dir() {
            let nested = root_path;
            if sbcl_installation_valid(&nested) {
                install_layout::move_dir(&nested, &staging_final)?;
                install_layout::commit_staging_dir(&staging_final, final_dir)?;
                return Ok(());
            }
        }
    }

    for e in fs::read_dir(staging).map_err(EnvrError::from)? {
        let e = e.map_err(EnvrError::from)?;
        if !e.file_type().map_err(EnvrError::from)?.is_dir() {
            continue;
        }
        let p = e.path();
        if sbcl_installation_valid(&p) {
            install_layout::move_dir(&p, &staging_final)?;
            install_layout::commit_staging_dir(&staging_final, final_dir)?;
            return Ok(());
        }
    }

    Err(EnvrError::Validation(
        "extracted sbcl layout missing sbcl executable".into(),
    ))
}

#[derive(Debug, Clone)]
pub struct SbclManager {
    paths: SbclPaths,
    releases_api_url: String,
}

impl SbclManager {
    pub fn try_new(runtime_root: PathBuf, releases_api_url: String) -> EnvrResult<Self> {
        Ok(Self {
            paths: SbclPaths::new(runtime_root),
            releases_api_url,
        })
    }

    fn index_cache_ttl_secs() -> u64 {
        std::env::var("ENVR_SBCL_RELEASES_CACHE_TTL_SECS")
            .ok()
            .or_else(|| std::env::var("ENVR_SBCL_INDEX_CACHE_TTL_SECS").ok())
            .and_then(|s| s.trim().parse::<u64>().ok())
            .unwrap_or(3600)
    }

    fn latest_cache_ttl_secs() -> u64 {
        std::env::var("ENVR_SBCL_REMOTE_CACHE_TTL_SECS")
            .ok()
            .and_then(|s| s.trim().parse::<u64>().ok())
            .unwrap_or(86400)
    }

    fn load_cached_rows(&self) -> Option<Vec<SbclInstallableRow>> {
        let path = self.paths.releases_cache_path();
        let meta = fs::metadata(&path).ok()?;
        let age = SystemTime::now().duration_since(meta.modified().ok()?).ok()?.as_secs();
        if age > Self::index_cache_ttl_secs() {
            return None;
        }
        let text = fs::read_to_string(&path).ok()?;
        serde_json::from_str::<Vec<SbclInstallableRow>>(&text).ok()
    }

    fn save_cached_rows(&self, rows: &[SbclInstallableRow]) -> EnvrResult<()> {
        fs::create_dir_all(self.paths.cache_dir()).map_err(EnvrError::from)?;
        let text = serde_json::to_string_pretty(rows)
            .map_err(|e| EnvrError::with_source(ErrorCode::Download, "serialize sbcl rows cache", e))?;
        fs::write(self.paths.releases_cache_path(), text).map_err(EnvrError::from)?;
        let _ = fs::remove_file(self.paths.latest_cache_path());
        Ok(())
    }

    fn fetch_rows(&self, force_refresh: bool) -> EnvrResult<Vec<SbclInstallableRow>> {
        if !force_refresh && let Some(rows) = self.load_cached_rows() {
            return Ok(rows);
        }
        let client = blocking_http_client()?;
        let rows = fetch_sbcl_installable_rows_with_fallback(&client, &self.releases_api_url)?;
        self.save_cached_rows(&rows)?;
        Ok(rows)
    }

    pub fn list_remote(&self, filter: &RemoteFilter) -> EnvrResult<Vec<RuntimeVersion>> {
        Ok(list_remote_versions(&self.fetch_rows(filter.force_index_refresh)?, filter))
    }

    pub fn list_remote_latest_per_major(&self) -> EnvrResult<Vec<RuntimeVersion>> {
        let rows = self.fetch_rows(false)?;
        let fresh = list_remote_latest_per_major_lines(&rows);
        let fresh_set: BTreeSet<String> = fresh.iter().map(|v| v.0.clone()).collect();

        let path = self.paths.latest_cache_path();
        if let Ok(meta) = fs::metadata(&path)
            && let Ok(age) = SystemTime::now().duration_since(meta.modified().map_err(EnvrError::from)?)
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
        let text = serde_json::to_string_pretty(&labels)
            .map_err(|e| EnvrError::with_source(ErrorCode::Download, "serialize sbcl latest cache", e))?;
        fs::write(&path, text).map_err(EnvrError::from)?;
        Ok(fresh)
    }

    pub fn resolve_label(&self, spec: &str) -> EnvrResult<String> {
        let rows = self.fetch_rows(false)?;
        if let Some(v) = resolve_sbcl_version(&rows, spec) {
            return Ok(v);
        }
        let rows = self.fetch_rows(true)?;
        resolve_sbcl_version(&rows, spec).ok_or_else(|| {
            EnvrError::Validation(format!("unknown sbcl version spec: {spec}"))
        })
    }

    pub fn set_current(&self, version: &RuntimeVersion) -> EnvrResult<()> {
        let dir = self.paths.version_dir(&version.0);
        if !dir.is_dir() || !sbcl_installation_valid(&dir) {
            return Err(EnvrError::Validation(format!("sbcl version not installed: {}", version.0)));
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

impl SpecDrivenInstaller for SbclManager {
    fn install_from_spec(&self, request: &InstallRequest) -> EnvrResult<RuntimeVersion> {
        let label = self.resolve_label(&request.spec.0)?;
        let rows = if cfg!(windows) {
            // Windows asset selection changed over time (MSI is the reliable binary surface).
            // Force refresh to avoid stale cached URLs (e.g. old tarball rows).
            self.fetch_rows(true)?
        } else {
            self.fetch_rows(false)?
        };
        let row = rows
            .iter()
            .find(|r| r.version == label)
            .ok_or_else(|| EnvrError::Validation(format!("sbcl version not found in index: {label}")))?;

        let final_dir = self.paths.version_dir(&label);
        if final_dir.is_dir() && sbcl_installation_valid(&final_dir) {
            return Ok(RuntimeVersion(label));
        }

        fs::create_dir_all(self.paths.cache_dir()).map_err(EnvrError::from)?;
        let client = blocking_http_client()?;
        // Defensive rewrite: on Windows, some older cached rows used the `.tar.bz2` URL which may
        // not contain `sbcl.exe`. Prefer `.msi` when possible.
        let effective_url = if cfg!(windows) && row.url.to_ascii_lowercase().ends_with(".tar.bz2")
        {
            row.url
                .trim_end_matches(".tar.bz2")
                .to_string()
                + ".msi"
        } else {
            row.url.clone()
        };

        let cache_file = self.paths.cache_dir().join(
            effective_url
                .split('/')
                .next_back()
                .unwrap_or("sbcl-archive"),
        );
        let (downloaded, total, cancel) = install_progress_handles(request);
        envr_download::blocking::download_url_to_path_resumable(
            &client,
            &effective_url,
            &cache_file,
            downloaded,
            total,
            cancel,
        )?;

        let is_msi = effective_url.to_ascii_lowercase().contains(".msi");
        #[cfg(windows)]
        if is_msi {
            install_from_msi_admin_unpack(&cache_file, &final_dir)?;
            return Ok(RuntimeVersion(label));
        }
        if is_msi {
            return Err(EnvrError::Validation(
                "sbcl MSI installs are only supported on Windows".into(),
            ));
        }

        let staging_parent = self.paths.cache_dir().join("extract_staging");
        fs::create_dir_all(&staging_parent).map_err(EnvrError::from)?;
        let staging = tempfile::tempdir_in(&staging_parent).map_err(EnvrError::from)?;
        extract::extract_archive(&cache_file, staging.path())?;
        promote_sbcl_extracted_tree(staging.path(), &final_dir)?;
        if !sbcl_installation_valid(&final_dir) {
            return Err(EnvrError::Validation("sbcl install validation failed".into()));
        }
        Ok(RuntimeVersion(label))
    }
}

