use crate::index::{
    CrystalReleaseRow, blocking_http_client, crystal_host_slug, fetch_all_crystal_release_rows,
    list_remote_latest_per_major_lines, list_remote_versions, parse_cached_install_rows,
    resolve_crystal_version,
};
use envr_domain::crystal_paths;
use envr_domain::installer::{SpecDrivenInstaller, install_progress_handles};
use envr_domain::runtime::{InstallRequest, RemoteFilter, RuntimeVersion};
use envr_download::{blocking::download_url_to_path_resumable_with_headers, checksum, extract};
use envr_error::{EnvrError, EnvrResult, ErrorCode};
use envr_platform::links::ensure_runtime_current_symlink_or_pointer;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::SystemTime;

#[derive(Debug, Clone)]
pub struct CrystalPaths {
    runtime_root: PathBuf,
}

impl CrystalPaths {
    pub fn new(runtime_root: PathBuf) -> Self {
        Self { runtime_root }
    }

    pub fn crystal_home(&self) -> PathBuf {
        self.runtime_root.join("runtimes").join("crystal")
    }

    pub fn versions_dir(&self) -> PathBuf {
        self.crystal_home().join("versions")
    }

    pub fn current_link(&self) -> PathBuf {
        self.crystal_home().join("current")
    }

    pub fn cache_dir(&self) -> PathBuf {
        self.runtime_root.join("cache").join("crystal")
    }

    pub fn index_cache_file(&self, host_slug: &str) -> PathBuf {
        self.cache_dir().join(format!("releases_{host_slug}.json"))
    }

    pub fn version_dir(&self, version_label: &str) -> PathBuf {
        self.versions_dir().join(version_label)
    }
}

/// True when `home` looks like a Crystal distribution root (delegates to `envr_domain::crystal_paths`).
pub fn crystal_installation_valid(home: &Path) -> bool {
    crystal_paths::crystal_home_has_compiler(home)
}

pub fn list_installed_versions(paths: &CrystalPaths) -> EnvrResult<Vec<RuntimeVersion>> {
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
        if crystal_installation_valid(&p) {
            out.push(RuntimeVersion(e.file_name().to_string_lossy().into_owned()));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

pub fn read_current(paths: &CrystalPaths) -> EnvrResult<Option<RuntimeVersion>> {
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
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        reqwest::header::ACCEPT,
        reqwest::header::HeaderValue::from_static("application/octet-stream"),
    );
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

fn collect_archive_root_directories(staging: &Path) -> EnvrResult<Vec<PathBuf>> {
    let mut dirs = Vec::new();
    for e in fs::read_dir(staging).map_err(EnvrError::from)? {
        let e = e.map_err(EnvrError::from)?;
        if !e.file_type().map_err(EnvrError::from)?.is_dir() {
            continue;
        }
        let name = e.file_name();
        let ns = name.to_string_lossy();
        if ns == "__MACOSX" {
            continue;
        }
        dirs.push(e.path());
    }
    Ok(dirs)
}

fn pick_crystal_home_directory(root_dirs: &[PathBuf]) -> EnvrResult<PathBuf> {
    if root_dirs.is_empty() {
        return Err(EnvrError::Validation(
            "crystal archive contained no top-level directories (only files or metadata like __MACOSX)"
                .into(),
        ));
    }
    let valid: Vec<PathBuf> = root_dirs
        .iter()
        .filter(|p| crystal_installation_valid(p))
        .cloned()
        .collect();
    match valid.len() {
        0 => {
            if root_dirs.len() == 1 {
                Ok(root_dirs[0].clone())
            } else {
                Err(EnvrError::Validation(
                    "crystal archive had several root folders but none looked like a Crystal home (bin/crystal, bin/crystal.exe, or crystal.exe at package root)"
                        .into(),
                ))
            }
        }
        1 => Ok(valid[0].clone()),
        _ => Err(EnvrError::Validation(
            "crystal archive contained multiple Crystal installation roots".into(),
        )),
    }
}

/// Crystal official archives may be either:
/// - one top-level directory (typical `.tar.gz`), or
/// - a **flat** tree (`bin/`, `lib/`, …) at the extract root (common Windows `.zip`).
///
/// macOS junk (`__MACOSX`) is ignored when counting root directories.
pub fn promote_single_root_dir(staging: &Path, final_dir: &Path) -> EnvrResult<()> {
    use envr_platform::install_layout;

    install_layout::ensure_final_parent(final_dir)?;
    let staging_final = install_layout::sibling_staging_path(final_dir)?;
    install_layout::remove_if_exists(&staging_final)?;

    let commit_if_valid = |home: &Path| -> EnvrResult<()> {
        if !crystal_installation_valid(home) {
            let _ = fs::remove_dir_all(home);
            return Err(EnvrError::Validation(
                "extracted crystal layout missing compiler (expected bin/crystal, bin/crystal.exe, or crystal.exe at root)"
                    .into(),
            ));
        }
        install_layout::commit_staging_dir(home, final_dir)
    };

    // Flat layout: `bin/crystal(.exe)` or Windows portable `crystal.exe` at extract root.
    if crystal_installation_valid(staging) {
        fs::create_dir_all(&staging_final).map_err(EnvrError::from)?;
        install_layout::hoist_directory_children(staging, &staging_final)?;
        return commit_if_valid(&staging_final);
    }

    let root_dirs = collect_archive_root_directories(staging)?;
    let inner = pick_crystal_home_directory(&root_dirs)?;
    fs::rename(&inner, &staging_final).map_err(EnvrError::from)?;
    commit_if_valid(&staging_final)
}

pub struct CrystalManager {
    pub paths: CrystalPaths,
    releases_url: String,
    client: reqwest::blocking::Client,
    host_slug: &'static str,
}

impl CrystalManager {
    pub fn try_new(runtime_root: PathBuf, releases_url: String) -> EnvrResult<Self> {
        Ok(Self {
            paths: CrystalPaths::new(runtime_root),
            releases_url,
            client: blocking_http_client()?,
            host_slug: crystal_host_slug()?,
        })
    }

    fn index_ttl_secs() -> u64 {
        const DEFAULT: u64 = 60 * 60;
        std::env::var("ENVR_CRYSTAL_RELEASES_CACHE_TTL_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .or_else(|| {
                std::env::var("ENVR_CRYSTAL_INDEX_CACHE_TTL_SECS")
                    .ok()
                    .and_then(|s| s.parse().ok())
            })
            .unwrap_or(DEFAULT)
    }

    fn load_rows(&self) -> EnvrResult<Vec<CrystalReleaseRow>> {
        let cache_path = self.paths.index_cache_file(self.host_slug);
        let ttl = Self::index_ttl_secs();
        if let Ok(meta) = fs::metadata(&cache_path) {
            if let Ok(modified) = meta.modified() {
                if let Ok(age) = SystemTime::now().duration_since(modified) {
                    if age.as_secs() < ttl {
                        if let Ok(body) = fs::read_to_string(&cache_path) {
                            if let Ok(rows) = parse_cached_install_rows(&body) {
                                if !rows.is_empty() {
                                    return Ok(rows);
                                }
                            }
                        }
                    }
                }
            }
        }
        let rows =
            fetch_all_crystal_release_rows(&self.client, &self.releases_url, self.host_slug)?;
        fs::create_dir_all(self.paths.cache_dir()).map_err(EnvrError::from)?;
        let body = serde_json::to_string(&rows).map_err(|e| {
            EnvrError::with_source(ErrorCode::Validation, "json encode crystal rows", e)
        })?;
        envr_platform::fs_atomic::write_atomic(&cache_path, body.as_bytes())
            .map_err(EnvrError::from)?;
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
        resolve_crystal_version(&rows, spec)
    }

    fn row_for_version<'a>(
        &'a self,
        rows: &'a [CrystalReleaseRow],
        version_label: &str,
    ) -> EnvrResult<&'a CrystalReleaseRow> {
        rows.iter()
            .find(|r| r.version == version_label)
            .ok_or_else(|| {
                EnvrError::Validation(format!(
                    "crystal {version_label} not found in GitHub releases index for host `{}`",
                    self.host_slug
                ))
            })
    }

    pub fn install_resolved_version(
        &self,
        version_label: &str,
        progress_downloaded: Option<&Arc<AtomicU64>>,
        progress_total: Option<&Arc<AtomicU64>>,
        cancel: Option<&Arc<AtomicBool>>,
    ) -> EnvrResult<RuntimeVersion> {
        if cancel.is_some_and(|c| c.load(Ordering::Relaxed)) {
            return Err(EnvrError::Download("download cancelled".into()));
        }
        let rows = self.load_rows()?;
        let row = self.row_for_version(&rows, version_label)?;
        let url = row.download_url.as_str();
        let ext = if url.ends_with(".zip") {
            ".zip"
        } else if url.ends_with(".tar.gz") {
            ".tar.gz"
        } else {
            return Err(EnvrError::Validation(format!(
                "unsupported crystal archive URL suffix: {url}"
            )));
        };
        let cache_dir = self.paths.cache_dir().join(version_label);
        fs::create_dir_all(&cache_dir).map_err(EnvrError::from)?;
        let archive_path = cache_dir.join(format!("crystal{ext}"));
        download_to_path(
            &self.client,
            url,
            &archive_path,
            progress_downloaded,
            progress_total,
            cancel,
        )?;
        if let Some(hex) = row.sha256_hex.as_deref() {
            let t = hex.trim();
            if !t.is_empty() {
                checksum::verify_sha256_hex(&archive_path, t)?;
            }
        }
        let staging_parent = cache_dir.join("extract_staging");
        fs::create_dir_all(&staging_parent).map_err(EnvrError::from)?;
        let staging = tempfile::tempdir_in(&staging_parent).map_err(EnvrError::from)?;
        extract::extract_archive(&archive_path, staging.path())?;
        let final_dir = self.paths.version_dir(version_label);
        promote_single_root_dir(staging.path(), &final_dir)?;
        self.set_current(&RuntimeVersion(version_label.to_string()))?;
        Ok(RuntimeVersion(version_label.to_string()))
    }

    pub fn set_current(&self, version: &RuntimeVersion) -> EnvrResult<()> {
        let dir = self.paths.version_dir(&version.0);
        if !crystal_installation_valid(&dir) {
            return Err(EnvrError::Validation(format!(
                "crystal {} is not installed under {}",
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

impl SpecDrivenInstaller for CrystalManager {
    fn install_from_spec(&self, request: &InstallRequest) -> EnvrResult<RuntimeVersion> {
        let label = self.resolve_label(&request.spec.0)?;
        let (downloaded, total, cancel) = install_progress_handles(request);
        self.install_resolved_version(&label, downloaded, total, cancel)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::DEFAULT_CRYSTAL_GITHUB_RELEASES_URL;

    fn write_mock_crystal_bin(home: &Path) {
        fs::create_dir_all(home.join("bin")).expect("bin");
        #[cfg(windows)]
        fs::write(home.join("bin").join("crystal.exe"), b"").expect("touch");
        #[cfg(not(windows))]
        fs::write(home.join("bin").join("crystal"), b"").expect("touch");
    }

    #[test]
    fn promote_accepts_flat_extract_root_layout() {
        let tmp = tempfile::tempdir().expect("tmp");
        let staging = tmp.path().join("extract");
        fs::create_dir_all(&staging).expect("mkdir");
        write_mock_crystal_bin(&staging);
        let dest_parent = tmp.path().join("versions");
        let final_dir = dest_parent.join("1.14.0");
        promote_single_root_dir(&staging, &final_dir).expect("promote flat layout");
        assert!(crystal_installation_valid(&final_dir));
    }

    /// Official Windows portable zip: `crystal.exe` at archive root with `lib/`, `src/`, etc.
    #[test]
    fn promote_accepts_portable_exe_at_root_with_peer_directories() {
        let tmp = tempfile::tempdir().expect("tmp");
        let staging = tmp.path().join("extract");
        fs::create_dir_all(staging.join("lib")).expect("lib");
        fs::create_dir_all(staging.join("src")).expect("src");
        #[cfg(windows)]
        fs::write(staging.join("crystal.exe"), b"").expect("exe");
        #[cfg(not(windows))]
        fs::write(staging.join("crystal"), b"").expect("crystal");
        let final_dir = tmp.path().join("versions").join("1.14.0");
        promote_single_root_dir(&staging, &final_dir).expect("promote portable root");
        assert!(crystal_installation_valid(&final_dir));
    }

    #[test]
    fn promote_accepts_single_wrapper_directory() {
        let tmp = tempfile::tempdir().expect("tmp");
        let staging = tmp.path().join("extract");
        fs::create_dir_all(&staging).expect("mkdir");
        let inner = staging.join("crystal-1.14.0-1-windows");
        write_mock_crystal_bin(&inner);
        let final_dir = tmp.path().join("versions").join("1.14.0");
        promote_single_root_dir(&staging, &final_dir).expect("promote nested layout");
        assert!(crystal_installation_valid(&final_dir));
    }

    #[test]
    fn uninstall_removes_selected_version_and_clears_current() {
        let tmp = tempfile::tempdir().expect("tmp");
        let runtime_root = tmp.path().to_path_buf();
        let mgr = CrystalManager::try_new(
            runtime_root.clone(),
            DEFAULT_CRYSTAL_GITHUB_RELEASES_URL.to_string(),
        )
        .expect("crystal manager");
        let ver = RuntimeVersion("1.14.0".to_string());
        let home = mgr.paths.version_dir(&ver.0);
        write_mock_crystal_bin(&home);

        mgr.set_current(&ver).expect("set current");
        assert_eq!(
            read_current(&mgr.paths).expect("read current before uninstall"),
            Some(ver.clone())
        );

        mgr.uninstall(&ver).expect("uninstall");
        assert!(!home.exists(), "version dir removed: {}", home.display());
        assert_eq!(
            read_current(&mgr.paths).expect("read current after uninstall"),
            None
        );
    }
}
