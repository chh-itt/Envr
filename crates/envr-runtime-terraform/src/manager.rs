use crate::index::{
    TerraformIndexRow, artifact_url, blocking_http_client, fetch_index_text,
    list_remote_latest_per_major_lines, list_remote_versions, parse_versions_from_index_html,
    resolve_terraform_version, terraform_platform_tuple,
};
use envr_domain::installer::{
    SpecDrivenInstaller, execute_install_pipeline, install_progress_handles,
};
use envr_domain::runtime::{InstallRequest, RemoteFilter, RuntimeVersion};
use envr_download::{blocking::download_url_to_path_resumable, extract};
use envr_error::{EnvrError, EnvrResult, ErrorCode};
use envr_platform::links::ensure_runtime_current_symlink_or_pointer;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::time::SystemTime;

#[derive(Debug, Clone)]
pub struct TerraformPaths {
    runtime_root: PathBuf,
}

impl TerraformPaths {
    pub fn new(runtime_root: PathBuf) -> Self {
        Self { runtime_root }
    }
    pub fn terraform_home(&self) -> PathBuf {
        self.runtime_root.join("runtimes").join("terraform")
    }
    pub fn versions_dir(&self) -> PathBuf {
        self.terraform_home().join("versions")
    }
    pub fn current_link(&self) -> PathBuf {
        self.terraform_home().join("current")
    }
    pub fn cache_dir(&self) -> PathBuf {
        self.runtime_root.join("cache").join("terraform")
    }
    pub fn version_dir(&self, version: &str) -> PathBuf {
        self.versions_dir().join(version)
    }
}

fn first_existing(cands: &[PathBuf]) -> Option<PathBuf> {
    cands.iter().find(|p| p.is_file()).cloned()
}

pub fn terraform_tool_candidate(home: &Path) -> Option<PathBuf> {
    first_existing(&[
        home.join("terraform.exe"),
        home.join("terraform"),
        home.join("bin").join("terraform.exe"),
        home.join("bin").join("terraform"),
    ])
}

pub fn terraform_installation_valid(home: &Path) -> bool {
    terraform_tool_candidate(home).is_some()
}

pub fn list_installed_versions(paths: &TerraformPaths) -> EnvrResult<Vec<RuntimeVersion>> {
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
        if terraform_installation_valid(&p) {
            out.push(RuntimeVersion(e.file_name().to_string_lossy().into_owned()));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

pub fn read_current(paths: &TerraformPaths) -> EnvrResult<Option<RuntimeVersion>> {
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

fn promote_terraform_extracted_tree(staging: &Path, final_dir: &Path) -> EnvrResult<()> {
    envr_platform::install_layout::commit_hoisted_children(
        staging,
        final_dir,
        terraform_installation_valid,
        "extracted terraform layout missing terraform executable",
    )
}

pub struct TerraformManager {
    pub paths: TerraformPaths,
    index_url: String,
    client: reqwest::blocking::Client,
}

impl TerraformManager {
    pub fn try_new(runtime_root: PathBuf, index_url: String) -> EnvrResult<Self> {
        Ok(Self {
            paths: TerraformPaths::new(runtime_root),
            index_url,
            client: blocking_http_client()?,
        })
    }

    fn index_ttl_secs() -> u64 {
        const DEFAULT: u64 = 6 * 60 * 60;
        std::env::var("ENVR_TERRAFORM_INDEX_CACHE_TTL_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT)
    }

    fn cache_path(&self) -> PathBuf {
        self.paths.cache_dir().join("index_versions.json")
    }

    fn load_rows(&self) -> EnvrResult<Vec<TerraformIndexRow>> {
        let ttl = Self::index_ttl_secs();
        let cache_path = self.cache_path();
        if let Ok(meta) = fs::metadata(&cache_path)
            && let Ok(modified) = meta.modified()
            && let Ok(age) = SystemTime::now().duration_since(modified)
            && age.as_secs() <= ttl
            && let Ok(body) = fs::read_to_string(&cache_path)
            && let Ok(rows) = serde_json::from_str::<Vec<TerraformIndexRow>>(&body)
            && !rows.is_empty()
        {
            return Ok(rows);
        }
        let html = fetch_index_text(&self.client, &self.index_url)?;
        let versions = parse_versions_from_index_html(&html);
        let rows: Vec<TerraformIndexRow> = versions
            .into_iter()
            .map(|version| TerraformIndexRow { version })
            .collect();
        if rows.is_empty() {
            return Err(EnvrError::Validation(
                "terraform: no stable versions parsed from releases index".into(),
            ));
        }
        let _ = (|| -> EnvrResult<()> {
            fs::create_dir_all(self.paths.cache_dir())?;
            let s = serde_json::to_string(&rows).map_err(|e| {
                EnvrError::with_source(ErrorCode::Validation, "json encode terraform rows", e)
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
        resolve_terraform_version(&rows, spec)
    }

    pub fn set_current(&self, version: &RuntimeVersion) -> EnvrResult<()> {
        let dir = self.paths.version_dir(&version.0);
        if !terraform_installation_valid(&dir) {
            return Err(EnvrError::Validation(format!(
                "terraform {} is not installed under {}",
                version.0,
                dir.display()
            )));
        }
        ensure_runtime_current_symlink_or_pointer(&dir, &self.paths.current_link())?;
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

impl SpecDrivenInstaller for TerraformManager {
    fn install_from_spec(&self, request: &InstallRequest) -> EnvrResult<RuntimeVersion> {
        let label = self.resolve_label(&request.spec.0)?;
        let platform = terraform_platform_tuple()?;
        let url = artifact_url(&self.index_url, &label, platform);
        let cache_dir = self.paths.cache_dir().join(&label);
        let archive_path = cache_dir.join("terraform.zip");
        let (downloaded, total, cancel) = install_progress_handles(request);
        let final_dir = self.paths.version_dir(&label);
        execute_install_pipeline(
            cancel,
            || fs::create_dir_all(&cache_dir).map_err(EnvrError::from),
            || download_to_path(&self.client, &url, &archive_path, downloaded, total, cancel),
            || Ok(()),
            || {
                let staging_parent = cache_dir.join("extract_staging");
                fs::create_dir_all(&staging_parent).map_err(EnvrError::from)?;
                let staging = tempfile::tempdir_in(&staging_parent).map_err(EnvrError::from)?;
                extract::extract_archive(&archive_path, staging.path())?;
                promote_terraform_extracted_tree(staging.path(), &final_dir)
            },
            || {
                let resolved = RuntimeVersion(label.clone());
                self.set_current(&resolved)?;
                Ok(resolved)
            },
        )
    }
}
