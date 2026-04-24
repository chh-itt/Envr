use crate::index::{
    GhRelease, blocking_http_client, fetch_clojure_github_releases_index,
    installable_pairs_from_releases, list_remote_latest_per_major_lines, list_remote_versions,
    resolve_clojure_version,
};
use envr_domain::installer::{SpecDrivenInstaller, install_progress_handles};
use envr_domain::runtime::{InstallRequest, RemoteFilter, RuntimeVersion};
use envr_download::{blocking::download_url_to_path_resumable, extract};
use envr_error::{EnvrError, EnvrResult, ErrorCode};
use envr_platform::install_layout;
use envr_platform::links::ensure_runtime_current_symlink_or_pointer;
use envr_shim_core::{ShimContext, resolve_runtime_home_for_lang};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::SystemTime;

#[derive(Debug, Clone)]
pub struct ClojurePaths {
    runtime_root: PathBuf,
}

impl ClojurePaths {
    pub fn new(runtime_root: PathBuf) -> Self {
        Self { runtime_root }
    }

    pub fn clojure_home(&self) -> PathBuf {
        self.runtime_root.join("runtimes").join("clojure")
    }

    pub fn versions_dir(&self) -> PathBuf {
        self.clojure_home().join("versions")
    }

    pub fn current_link(&self) -> PathBuf {
        self.clojure_home().join("current")
    }

    pub fn cache_dir(&self) -> PathBuf {
        self.runtime_root.join("cache").join("clojure")
    }

    pub fn version_dir(&self, version_label: &str) -> PathBuf {
        self.versions_dir().join(version_label)
    }
}

fn first_existing(candidates: &[PathBuf]) -> Option<PathBuf> {
    candidates.iter().find(|p| p.is_file()).cloned()
}

fn tool_locations(home: &Path) -> [PathBuf; 2] {
    [home.to_path_buf(), home.join("bin")]
}

pub fn clojure_tool_candidate(home: &Path, stem: &str) -> Option<PathBuf> {
    let mut cands = Vec::new();
    for base in tool_locations(home) {
        cands.push(base.join(format!("{stem}.cmd")));
        cands.push(base.join(format!("{stem}.bat")));
        cands.push(base.join(format!("{stem}.exe")));
        cands.push(base.join(stem));
    }
    first_existing(&cands)
}

pub fn clojure_installation_valid(home: &Path) -> bool {
    clojure_tool_candidate(home, "clj").is_some()
        && clojure_tool_candidate(home, "clojure").is_some()
}

fn find_clojure_distribution_root(extract_root: &Path) -> Option<PathBuf> {
    if clojure_installation_valid(extract_root) || has_clojure_module_layout(extract_root) {
        return Some(extract_root.to_path_buf());
    }
    let entries = fs::read_dir(extract_root).ok()?;
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() && (clojure_installation_valid(&p) || has_clojure_module_layout(&p)) {
            return Some(p);
        }
    }
    None
}

fn has_clojure_module_layout(home: &Path) -> bool {
    home.join("ClojureTools")
        .join("ClojureTools.psm1")
        .is_file()
        || home
            .join("ClojureTools")
            .join("ClojureTools.psd1")
            .is_file()
}

#[cfg(windows)]
fn ensure_windows_clojure_launchers(home: &Path) -> EnvrResult<()> {
    if clojure_installation_valid(home) {
        return Ok(());
    }
    let module_psd1 = home.join("ClojureTools").join("ClojureTools.psd1");
    let module_psm1 = home.join("ClojureTools").join("ClojureTools.psm1");
    if !module_psd1.is_file() && !module_psm1.is_file() {
        return Ok(());
    }
    let mk = |tool: &str| -> String {
        format!(
            "@echo off\r\nsetlocal\r\nset \"__ENVR_PS=%SystemRoot%\\System32\\WindowsPowerShell\\v1.0\\powershell.exe\"\r\nif exist \"%ProgramFiles%\\PowerShell\\7\\pwsh.exe\" set \"__ENVR_PS=%ProgramFiles%\\PowerShell\\7\\pwsh.exe\"\r\nif exist \"%ProgramFiles(x86)%\\PowerShell\\7\\pwsh.exe\" set \"__ENVR_PS=%ProgramFiles(x86)%\\PowerShell\\7\\pwsh.exe\"\r\nset \"__ENVR_MOD=%~dp0ClojureTools\\ClojureTools.psd1\"\r\nif not exist \"%__ENVR_MOD%\" set \"__ENVR_MOD=%~dp0ClojureTools\\ClojureTools.psm1\"\r\n\"%__ENVR_PS%\" -NoProfile -ExecutionPolicy Bypass -Command \"Import-Module -Force '%__ENVR_MOD%'; {tool} %*\"\r\n"
        )
    };
    fs::write(home.join("clj.cmd"), mk("clj")).map_err(EnvrError::from)?;
    fs::write(home.join("clojure.cmd"), mk("clojure")).map_err(EnvrError::from)?;
    Ok(())
}

#[cfg(not(windows))]
fn ensure_windows_clojure_launchers(_home: &Path) -> EnvrResult<()> {
    Ok(())
}

pub fn promote_clojure_extracted_tree(staging: &Path, final_dir: &Path) -> EnvrResult<()> {
    let root = find_clojure_distribution_root(staging).ok_or_else(|| {
        EnvrError::Validation(format!(
            "extracted clojure layout missing clj/clojure under {}",
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

    // Windows `clojure-tools.zip` is a PowerShell module (`ClojureTools/*`) without `clj.cmd` / `clojure.cmd`.
    // Materialize launchers so envr shim resolution has stable executables.
    if has_clojure_module_layout(&staging_final) {
        ensure_windows_clojure_launchers(&staging_final)?;
    }

    if !clojure_installation_valid(&staging_final) {
        let _ = fs::remove_dir_all(&staging_final);
        return Err(EnvrError::Validation(
            "promoted clojure tree missing clj/clojure".into(),
        ));
    }

    install_layout::commit_staging_dir(&staging_final, final_dir)?;
    Ok(())
}

pub fn list_installed_versions(paths: &ClojurePaths) -> EnvrResult<Vec<RuntimeVersion>> {
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
        if clojure_installation_valid(&p) {
            out.push(RuntimeVersion(e.file_name().to_string_lossy().into_owned()));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

pub fn read_current(paths: &ClojurePaths) -> EnvrResult<Option<RuntimeVersion>> {
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

fn ensure_java_preflight(runtime_root: &Path, clojure_version_label: &str) -> EnvrResult<()> {
    let working_dir = std::env::current_dir().unwrap_or_else(|_| runtime_root.to_path_buf());
    let ctx = ShimContext::with_runtime_root(runtime_root.to_path_buf(), working_dir, None);
    let java_home = resolve_runtime_home_for_lang(&ctx, "java", None)?;
    let label = java_home.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let Some(maj) = envr_domain::kotlin_java::jdk_dir_label_effective_major(label) else {
        return Err(EnvrError::Validation(format!(
            "could not parse Java major from `{label}` under {}",
            java_home.display()
        )));
    };
    if maj < 8 {
        return Err(EnvrError::Validation(format!(
            "Clojure 需要 Java 8+。当前 JDK 目录名 `{label}` 推断主版本为 {maj}。请先安装并执行 `envr use java <版本>`。详见 docs/runtime/clojure.md。"
        )));
    }
    if let Some(msg) =
        envr_domain::clojure_java::clojure_jdk_mismatch_message(clojure_version_label, label)
    {
        return Err(EnvrError::Validation(msg));
    }
    Ok(())
}

pub struct ClojureManager {
    pub paths: ClojurePaths,
    releases_api_url: String,
    client: reqwest::blocking::Client,
}

impl ClojureManager {
    pub fn try_new(runtime_root: PathBuf, releases_api_url: String) -> EnvrResult<Self> {
        Ok(Self {
            paths: ClojurePaths::new(runtime_root),
            releases_api_url,
            client: blocking_http_client()?,
        })
    }

    fn index_ttl_secs() -> u64 {
        const DEFAULT: u64 = 6 * 60 * 60;
        std::env::var("ENVR_CLOJURE_INDEX_CACHE_TTL_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT)
    }

    fn releases_cache_path(&self) -> PathBuf {
        self.paths.cache_dir().join("github_releases.json")
    }

    fn file_is_within_ttl(path: &Path, ttl_secs: u64) -> bool {
        if ttl_secs == 0 {
            return false;
        }
        let Ok(meta) = fs::metadata(path) else {
            return false;
        };
        let Ok(mtime) = meta.modified() else {
            return false;
        };
        let Ok(age) = SystemTime::now().duration_since(mtime) else {
            return false;
        };
        age.as_secs() <= ttl_secs
    }

    fn load_pairs(&self) -> EnvrResult<Vec<(String, String)>> {
        let cache_path = self.releases_cache_path();
        let ttl = Self::index_ttl_secs();
        if Self::file_is_within_ttl(&cache_path, ttl) {
            if let Ok(body) = fs::read_to_string(&cache_path) {
                if let Ok(releases) = serde_json::from_str::<Vec<GhRelease>>(&body) {
                    let pairs = installable_pairs_from_releases(&releases);
                    if !pairs.is_empty() {
                        return Ok(pairs);
                    }
                }
            }
        }
        let releases = fetch_clojure_github_releases_index(&self.client, &self.releases_api_url)?;
        let body = serde_json::to_string(&releases).map_err(|e| {
            EnvrError::with_source(ErrorCode::Validation, "clojure releases json", e)
        })?;
        let _ = (|| -> EnvrResult<()> {
            fs::create_dir_all(self.paths.cache_dir())?;
            envr_platform::fs_atomic::write_atomic(&cache_path, body.as_bytes())?;
            Ok(())
        })();
        let pairs = installable_pairs_from_releases(&releases);
        if pairs.is_empty() {
            return Err(EnvrError::Validation(
                "clojure: no installable archives in releases response for this platform".into(),
            ));
        }
        Ok(pairs)
    }

    pub fn list_remote(&self, filter: &RemoteFilter) -> EnvrResult<Vec<RuntimeVersion>> {
        let pairs = self.load_pairs()?;
        Ok(list_remote_versions(&pairs, filter))
    }

    pub fn list_remote_latest_per_major(&self) -> EnvrResult<Vec<RuntimeVersion>> {
        let pairs = self.load_pairs()?;
        Ok(list_remote_latest_per_major_lines(&pairs))
    }

    pub fn try_load_remote_latest_per_major_from_disk(&self) -> Vec<RuntimeVersion> {
        let path = self.paths.cache_dir().join("remote_latest_per_major.json");
        let Some(list) =
            envr_platform::cache_recovery::read_json_string_list(&path, None, |xs| !xs.is_empty())
        else {
            return Vec::new();
        };
        list.into_iter().map(RuntimeVersion).collect()
    }

    pub fn persist_remote_latest_per_major_cache(&self, list: &[RuntimeVersion]) -> EnvrResult<()> {
        fs::create_dir_all(self.paths.cache_dir())?;
        let path = self.paths.cache_dir().join("remote_latest_per_major.json");
        let labels: Vec<&str> = list.iter().map(|v| v.0.as_str()).collect();
        let s = serde_json::to_string(&labels).map_err(|e| {
            EnvrError::with_source(
                ErrorCode::Validation,
                "json encode clojure latest labels",
                e,
            )
        })?;
        envr_platform::fs_atomic::write_atomic(&path, s.as_bytes())?;
        Ok(())
    }

    pub fn list_remote_latest_per_major_cached(&self) -> EnvrResult<Vec<RuntimeVersion>> {
        let ttl_secs = Self::index_ttl_secs();
        let cache_file = self.paths.cache_dir().join("remote_latest_per_major.json");
        if let Some(list) = envr_platform::cache_recovery::read_json_string_list(
            &cache_file,
            Some(ttl_secs),
            |xs| !xs.is_empty(),
        ) {
            return Ok(list.into_iter().map(RuntimeVersion).collect());
        }
        let list = self.list_remote_latest_per_major()?;
        let _ = self.persist_remote_latest_per_major_cache(&list);
        Ok(list)
    }

    pub fn resolve_label(&self, spec: &str) -> EnvrResult<String> {
        let pairs = self.load_pairs()?;
        resolve_clojure_version(&pairs, spec)
    }

    fn cache_download_name(url: &str) -> &'static str {
        if url.contains(".tar.gz") {
            "clojure-tools.tar.gz"
        } else {
            "clojure-tools.zip"
        }
    }

    pub fn install_resolved_version(
        &self,
        version_label: &str,
        artifact_url: &str,
        progress_downloaded: Option<&Arc<AtomicU64>>,
        progress_total: Option<&Arc<AtomicU64>>,
        cancel: Option<&Arc<AtomicBool>>,
    ) -> EnvrResult<RuntimeVersion> {
        ensure_java_preflight(&self.paths.runtime_root, version_label)?;
        if cancel.is_some_and(|c| c.load(Ordering::Relaxed)) {
            return Err(EnvrError::Download("download cancelled".into()));
        }

        let cache_dir = self.paths.cache_dir().join(version_label);
        fs::create_dir_all(&cache_dir).map_err(EnvrError::from)?;
        let archive_path = cache_dir.join(Self::cache_download_name(artifact_url));
        download_to_path(
            &self.client,
            artifact_url,
            &archive_path,
            progress_downloaded,
            progress_total,
            cancel,
        )?;

        let staging_parent = cache_dir.join("extract_staging");
        fs::create_dir_all(&staging_parent).map_err(EnvrError::from)?;
        let staging = tempfile::tempdir_in(&staging_parent).map_err(EnvrError::from)?;
        extract::extract_archive(&archive_path, staging.path())?;
        let final_dir = self.paths.version_dir(version_label);
        promote_clojure_extracted_tree(staging.path(), &final_dir)?;
        self.set_current(&RuntimeVersion(version_label.to_string()))?;
        Ok(RuntimeVersion(version_label.to_string()))
    }

    pub fn set_current(&self, version: &RuntimeVersion) -> EnvrResult<()> {
        ensure_java_preflight(&self.paths.runtime_root, &version.0)?;
        let dir = self.paths.version_dir(&version.0);
        if !clojure_installation_valid(&dir) {
            return Err(EnvrError::Validation(format!(
                "clojure {} is not installed under {}",
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

impl SpecDrivenInstaller for ClojureManager {
    fn install_from_spec(&self, request: &InstallRequest) -> EnvrResult<RuntimeVersion> {
        let label = self.resolve_label(&request.spec.0)?;
        let pairs = self.load_pairs()?;
        let url = pairs
            .iter()
            .find(|(l, _)| l == &label)
            .map(|(_, u)| u.as_str())
            .ok_or_else(|| {
                EnvrError::Validation(format!("clojure release `{label}` has no download URL"))
            })?;
        let (downloaded, total, cancel) = install_progress_handles(request);
        self.install_resolved_version(&label, url, downloaded, total, cancel)
    }
}
