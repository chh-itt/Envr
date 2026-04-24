use crate::index::{
    DartIndexRow, blocking_http_client, dart_platform_tuple, fetch_text,
    list_remote_latest_per_major_lines, list_remote_versions, parse_rows_from_bucket_list_json,
    resolve_dart_version,
};
use envr_domain::installer::{
    SpecDrivenInstaller, execute_install_pipeline, install_progress_handles,
};
use envr_domain::runtime::{InstallRequest, RemoteFilter, RuntimeVersion};
use envr_download::{blocking::download_url_to_path_resumable, extract};
use envr_error::{EnvrError, EnvrResult, ErrorCode};
use envr_platform::bin_tool_layout::dart_installation_valid;
use envr_platform::links::ensure_runtime_current_symlink_or_pointer;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Debug, Clone)]
pub struct DartPaths {
    runtime_root: PathBuf,
}

impl DartPaths {
    pub fn new(runtime_root: PathBuf) -> Self {
        Self { runtime_root }
    }
    pub fn dart_home(&self) -> PathBuf {
        self.runtime_root.join("runtimes").join("dart")
    }
    pub fn versions_dir(&self) -> PathBuf {
        self.dart_home().join("versions")
    }
    pub fn current_link(&self) -> PathBuf {
        self.dart_home().join("current")
    }
    pub fn cache_dir(&self) -> PathBuf {
        self.runtime_root.join("cache").join("dart")
    }
    pub fn version_dir(&self, version: &str) -> PathBuf {
        self.versions_dir().join(version)
    }
    pub fn index_cache_file(&self) -> PathBuf {
        self.cache_dir().join("index_rows.json")
    }
}

pub fn list_installed_versions(paths: &DartPaths) -> EnvrResult<Vec<RuntimeVersion>> {
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
        if dart_installation_valid(&p) {
            out.push(RuntimeVersion(e.file_name().to_string_lossy().into_owned()));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

pub fn read_current(paths: &DartPaths) -> EnvrResult<Option<RuntimeVersion>> {
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

// download/promote helpers are centralized in envr-download + envr-platform::install_layout.

pub struct DartManager {
    pub paths: DartPaths,
    bucket_list_api_url: String,
    client: reqwest::blocking::Client,
}

impl DartManager {
    pub fn try_new(runtime_root: PathBuf, bucket_list_api_url: String) -> EnvrResult<Self> {
        Ok(Self {
            paths: DartPaths::new(runtime_root),
            bucket_list_api_url,
            client: blocking_http_client()?,
        })
    }

    fn index_ttl_secs() -> u64 {
        const DEFAULT: u64 = 6 * 60 * 60;
        std::env::var("ENVR_DART_INDEX_CACHE_TTL_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT)
    }

    fn load_rows(&self) -> EnvrResult<Vec<DartIndexRow>> {
        fn normalize_rows(rows: Vec<DartIndexRow>) -> Vec<DartIndexRow> {
            rows.into_iter()
                .filter(|r| {
                    envr_domain::runtime::numeric_version_segments(&r.version)
                        .is_some_and(|p| p.len() >= 2)
                })
                .collect()
        }
        let cache_path = self.paths.index_cache_file();
        let ttl = Self::index_ttl_secs();
        if let Ok(meta) = fs::metadata(&cache_path)
            && let Ok(modified) = meta.modified()
            && let Ok(age) = SystemTime::now().duration_since(modified)
            && age.as_secs() < ttl
            && let Ok(body) = fs::read_to_string(&cache_path)
            && let Ok(rows) = serde_json::from_str::<Vec<DartIndexRow>>(&body).map(normalize_rows)
            && !rows.is_empty()
        {
            return Ok(rows);
        }
        let body = fetch_text(&self.client, &self.bucket_list_api_url)?;
        let rows = normalize_rows(parse_rows_from_bucket_list_json(
            &body,
            dart_platform_tuple()?,
        )?);
        if rows.is_empty() {
            return Err(EnvrError::Validation(
                "dart: no installable stable versions for this host".into(),
            ));
        }
        let _ = (|| -> EnvrResult<()> {
            fs::create_dir_all(self.paths.cache_dir())?;
            let s = serde_json::to_string(&rows).map_err(|e| {
                EnvrError::with_source(ErrorCode::Validation, "json encode dart rows", e)
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
        resolve_dart_version(&rows, spec)
    }

    pub fn set_current(&self, version: &RuntimeVersion) -> EnvrResult<()> {
        let dir = self.paths.version_dir(&version.0);
        if !dart_installation_valid(&dir) {
            return Err(EnvrError::Validation(format!(
                "dart {} is not installed under {}",
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

impl SpecDrivenInstaller for DartManager {
    fn install_from_spec(&self, request: &InstallRequest) -> EnvrResult<RuntimeVersion> {
        let rows = self.load_rows()?;
        let label = resolve_dart_version(&rows, &request.spec.0)?;
        let row = rows
            .iter()
            .find(|r| r.version == label)
            .ok_or_else(|| EnvrError::Validation(format!("dart release `{label}` not found")))?;
        let cache_dir = self.paths.cache_dir().join(&label);
        let archive_path = cache_dir.join("dartsdk.zip");
        let (downloaded, total, cancel) = install_progress_handles(request);
        let final_dir = self.paths.version_dir(&label);
        execute_install_pipeline(
            cancel,
            || fs::create_dir_all(&cache_dir).map_err(EnvrError::from),
            || {
                download_url_to_path_resumable(
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
                envr_platform::install_layout::commit_single_root_dir(
                    staging.path(),
                    &final_dir,
                    dart_installation_valid,
                    "empty dart archive",
                    "expected exactly one root directory in dart archive",
                    "expected dart archive root to be a directory",
                    "extracted dart layout missing dart executable",
                )
            },
            || {
                let resolved = RuntimeVersion(label.clone());
                self.set_current(&resolved)?;
                Ok(resolved)
            },
        )
    }
}
