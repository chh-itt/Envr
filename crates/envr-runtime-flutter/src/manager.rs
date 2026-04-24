use crate::index::{
    FlutterIndexRow, blocking_http_client, fetch_text, list_remote_latest_per_major_lines,
    list_remote_versions, parse_rows_from_releases_json, releases_json_url_for_host,
    resolve_flutter_version,
};
use envr_domain::installer::{
    SpecDrivenInstaller, execute_install_pipeline, install_progress_handles,
};
use envr_domain::runtime::{InstallRequest, RemoteFilter, RuntimeVersion};
use envr_download::{blocking::download_url_to_path_resumable, extract};
use envr_error::{EnvrError, EnvrResult, ErrorCode};
use envr_platform::bin_tool_layout::flutter_installation_valid;
use envr_platform::links::ensure_runtime_current_symlink_or_pointer;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::time::SystemTime;

#[derive(Debug, Clone)]
pub struct FlutterPaths {
    runtime_root: PathBuf,
}

impl FlutterPaths {
    pub fn new(runtime_root: PathBuf) -> Self {
        Self { runtime_root }
    }
    pub fn flutter_home(&self) -> PathBuf {
        self.runtime_root.join("runtimes").join("flutter")
    }
    pub fn versions_dir(&self) -> PathBuf {
        self.flutter_home().join("versions")
    }
    pub fn current_link(&self) -> PathBuf {
        self.flutter_home().join("current")
    }
    pub fn cache_dir(&self) -> PathBuf {
        self.runtime_root.join("cache").join("flutter")
    }
    pub fn version_dir(&self, version: &str) -> PathBuf {
        self.versions_dir().join(version)
    }
    pub fn index_cache_file(&self) -> PathBuf {
        self.cache_dir().join("index_rows.json")
    }
}

pub fn list_installed_versions(paths: &FlutterPaths) -> EnvrResult<Vec<RuntimeVersion>> {
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
        if flutter_installation_valid(&p) {
            out.push(RuntimeVersion(e.file_name().to_string_lossy().into_owned()));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

pub fn read_current(paths: &FlutterPaths) -> EnvrResult<Option<RuntimeVersion>> {
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
    download_url_to_path_resumable(
        client,
        url,
        path,
        progress_downloaded,
        progress_total,
        cancel,
    )
}

fn should_strip_flutter_git() -> bool {
    matches!(
        std::env::var("ENVR_FLUTTER_STRIP_GIT")
            .ok()
            .as_deref()
            .map(str::trim),
        Some("1" | "true" | "TRUE" | "yes" | "YES" | "on" | "ON")
    )
}

fn delete_flutter_git_dir(install_root: &Path) -> EnvrResult<()> {
    let dot_git = install_root.join(".git");
    if dot_git.exists() {
        fs::remove_dir_all(&dot_git).map_err(EnvrError::from)?;
    }
    Ok(())
}

fn promote_single_root_dir(staging: &Path, final_dir: &Path) -> EnvrResult<()> {
    use envr_platform::install_layout;
    install_layout::commit_single_root_dir(
        staging,
        final_dir,
        |root| {
            if !flutter_installation_valid(root) {
                return false;
            }
            // Flutter tooling expects repo metadata for normal operation; keep `.git` by default.
            if should_strip_flutter_git() {
                let _ = delete_flutter_git_dir(root);
            }
            true
        },
        "empty flutter archive",
        "expected exactly one root directory in flutter archive",
        "expected flutter archive root to be a directory",
        "extracted flutter layout missing flutter executable",
    )
}

pub struct FlutterManager {
    pub paths: FlutterPaths,
    releases_url: String,
    client: reqwest::blocking::Client,
}

impl FlutterManager {
    pub fn try_new(runtime_root: PathBuf, releases_url: Option<String>) -> EnvrResult<Self> {
        Ok(Self {
            paths: FlutterPaths::new(runtime_root),
            releases_url: releases_url.unwrap_or(releases_json_url_for_host()?.to_string()),
            client: blocking_http_client()?,
        })
    }

    fn index_ttl_secs() -> u64 {
        const DEFAULT: u64 = 6 * 60 * 60;
        std::env::var("ENVR_FLUTTER_INDEX_CACHE_TTL_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT)
    }

    fn load_rows(&self) -> EnvrResult<Vec<FlutterIndexRow>> {
        let cache_path = self.paths.index_cache_file();
        let ttl = Self::index_ttl_secs();
        if let Ok(meta) = fs::metadata(&cache_path)
            && let Ok(modified) = meta.modified()
            && let Ok(age) = SystemTime::now().duration_since(modified)
            && age.as_secs() < ttl
            && let Ok(body) = fs::read_to_string(&cache_path)
            && let Ok(rows) = serde_json::from_str::<Vec<FlutterIndexRow>>(&body)
            && !rows.is_empty()
        {
            return Ok(rows);
        }
        let body = fetch_text(&self.client, &self.releases_url)?;
        let rows = parse_rows_from_releases_json(&body)?;
        if rows.is_empty() {
            return Err(EnvrError::Validation(
                "flutter: no installable stable versions for this host".into(),
            ));
        }
        let _ = (|| -> EnvrResult<()> {
            fs::create_dir_all(self.paths.cache_dir())?;
            let s = serde_json::to_string(&rows).map_err(|e| {
                EnvrError::with_source(ErrorCode::Validation, "json encode flutter rows", e)
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
        resolve_flutter_version(&rows, spec)
    }

    pub fn set_current(&self, version: &RuntimeVersion) -> EnvrResult<()> {
        let dir = self.paths.version_dir(&version.0);
        if !flutter_installation_valid(&dir) {
            return Err(EnvrError::Validation(format!(
                "flutter {} is not installed under {}",
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

impl SpecDrivenInstaller for FlutterManager {
    fn install_from_spec(&self, request: &InstallRequest) -> EnvrResult<RuntimeVersion> {
        let rows = self.load_rows()?;
        let label = resolve_flutter_version(&rows, &request.spec.0)?;
        let row = rows
            .iter()
            .find(|r| r.version == label)
            .ok_or_else(|| EnvrError::Validation(format!("flutter release `{label}` not found")))?;
        let cache_dir = self.paths.cache_dir().join(&label);
        fs::create_dir_all(&cache_dir).map_err(EnvrError::from)?;
        let filename = if row.url.ends_with(".tar.xz") {
            "flutter.tar.xz"
        } else {
            "flutter.zip"
        };
        let archive_path = cache_dir.join(filename);
        let (downloaded, total, cancel) = install_progress_handles(request);
        let final_dir = self.paths.version_dir(&label);
        execute_install_pipeline(
            cancel,
            || fs::create_dir_all(&cache_dir).map_err(EnvrError::from),
            || {
                download_to_path(
                    &self.client,
                    &row.url,
                    &archive_path,
                    downloaded,
                    total,
                    cancel,
                )
            },
            || Ok(()),
            || {
                let staging_parent = cache_dir.join("extract_staging");
                fs::create_dir_all(&staging_parent).map_err(EnvrError::from)?;
                let staging = tempfile::tempdir_in(&staging_parent).map_err(EnvrError::from)?;
                extract::extract_archive(&archive_path, staging.path())?;
                promote_single_root_dir(staging.path(), &final_dir)
            },
            || {
                let resolved = RuntimeVersion(label.clone());
                self.set_current(&resolved)?;
                Ok(resolved)
            },
        )
    }
}
