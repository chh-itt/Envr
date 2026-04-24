use crate::index::{lua_host_kind, sourceforge_tools_download_url, tools_executable_filename};
use envr_domain::installer::{
    SpecDrivenInstaller, execute_install_pipeline, install_progress_handles,
};
use envr_domain::runtime::{InstallRequest, RemoteFilter, RuntimeVersion};
use envr_download::{checksum, extract};
use envr_error::{EnvrError, EnvrResult, ErrorCode};
use envr_mirror::resolver::{load_settings_cached, maybe_mirror_url};
use envr_platform::install_layout;
use envr_platform::links::ensure_runtime_current_symlink_or_pointer;
use envr_platform::lua_binaries::lua_installation_valid;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::SystemTime;

#[derive(Debug, Clone)]
pub struct LuaPaths {
    runtime_root: PathBuf,
}

impl LuaPaths {
    pub fn new(runtime_root: PathBuf) -> Self {
        Self { runtime_root }
    }

    pub fn lua_home(&self) -> PathBuf {
        self.runtime_root.join("runtimes").join("lua")
    }

    pub fn versions_dir(&self) -> PathBuf {
        self.lua_home().join("versions")
    }

    pub fn current_link(&self) -> PathBuf {
        self.lua_home().join("current")
    }

    pub fn cache_dir(&self) -> PathBuf {
        self.runtime_root.join("cache").join("lua")
    }

    pub fn version_dir(&self, version_label: &str) -> PathBuf {
        self.versions_dir().join(version_label)
    }
}

fn find_lua_distribution_root(extract_root: &Path) -> Option<PathBuf> {
    if lua_installation_valid(extract_root) {
        return Some(extract_root.to_path_buf());
    }
    let entries = fs::read_dir(extract_root).ok()?;
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() && lua_installation_valid(&p) {
            return Some(p);
        }
    }
    None
}

pub fn promote_lua_extracted_tree(staging: &Path, final_dir: &Path) -> EnvrResult<()> {
    let root = find_lua_distribution_root(staging).ok_or_else(|| {
        EnvrError::Validation(format!(
            "extracted lua layout missing lua executable under {}",
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

    if !lua_installation_valid(&staging_final) {
        let _ = fs::remove_dir_all(&staging_final);
        return Err(EnvrError::Validation(
            "promoted lua tree missing lua executable".into(),
        ));
    }

    install_layout::commit_staging_dir(&staging_final, final_dir)?;
    Ok(())
}

pub fn list_installed_versions(paths: &LuaPaths) -> EnvrResult<Vec<RuntimeVersion>> {
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
        if lua_installation_valid(&p) {
            out.push(RuntimeVersion(e.file_name().to_string_lossy().into_owned()));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

pub fn read_current(paths: &LuaPaths) -> EnvrResult<Option<RuntimeVersion>> {
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
    let mut response = client.get(url).send().map_err(|e| {
        EnvrError::with_source(ErrorCode::Download, format!("request failed for {url}"), e)
    })?;
    if !response.status().is_success() {
        return Err(EnvrError::Download(format!(
            "GET {url} -> {}",
            response.status()
        )));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(EnvrError::from)?;
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
        let n = response.read(&mut buf).map_err(|e| {
            EnvrError::with_source(
                ErrorCode::Download,
                format!("read response body failed for {url}"),
                e,
            )
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

fn try_fetch_sha256_sidecar(
    client: &reqwest::blocking::Client,
    archive_url: &str,
) -> Option<String> {
    let sum_url = format!("{archive_url}.sha256");
    let response = client.get(&sum_url).send().ok()?;
    if !response.status().is_success() {
        return None;
    }
    let body = response.text().ok()?;
    let token = body.split_whitespace().next()?.trim();
    if token.len() == 64 && token.chars().all(|c| c.is_ascii_hexdigit()) {
        Some(token.to_ascii_lowercase())
    } else {
        None
    }
}

pub struct LuaManager {
    pub paths: LuaPaths,
    download_page_url: String,
    client: reqwest::blocking::Client,
}

impl LuaManager {
    pub fn try_new(runtime_root: PathBuf, download_page_url: String) -> EnvrResult<Self> {
        Ok(Self {
            paths: LuaPaths::new(runtime_root),
            download_page_url,
            client: crate::index::blocking_http_client()?,
        })
    }

    fn index_ttl_secs() -> u64 {
        const DEFAULT: u64 = 24 * 60 * 60;
        std::env::var("ENVR_LUA_INDEX_CACHE_TTL_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT)
    }

    fn index_cache_path(&self) -> PathBuf {
        self.paths.cache_dir().join("download_page.html")
    }

    pub fn load_version_list(&self) -> EnvrResult<Vec<String>> {
        let cache_path = self.index_cache_path();
        let ttl = Self::index_ttl_secs();
        let settings = load_settings_cached()?;
        let offline = settings.mirror.mode == envr_config::settings::MirrorMode::Offline;

        if (offline || Self::file_is_within_ttl(&cache_path, ttl))
            && let Ok(body) = fs::read_to_string(&cache_path)
        {
            let parsed = crate::index::parse_installable_versions(&body);
            if !parsed.is_empty() {
                return Ok(parsed);
            }
            let _ = fs::remove_file(&cache_path);
        }

        if offline {
            return Err(EnvrError::Download(format!(
                "offline mode: missing cached lua index at {} (run `envr cache index sync` after going online)",
                cache_path.display()
            )));
        }

        let url = maybe_mirror_url(&settings, &self.download_page_url)?;
        let body = crate::index::fetch_download_page(&self.client, &url)?;
        let _ = (|| -> EnvrResult<()> {
            fs::create_dir_all(self.paths.cache_dir())?;
            envr_platform::fs_atomic::write_atomic(&cache_path, body.as_bytes())?;
            Ok(())
        })();
        let parsed = crate::index::parse_installable_versions(&body);
        if parsed.is_empty() {
            return Err(EnvrError::Validation(
                "lua download page contained no Win64 tool rows; upstream HTML may have changed"
                    .into(),
            ));
        }
        Ok(parsed)
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

    fn remote_latest_per_major_cache_path(&self) -> PathBuf {
        self.paths.cache_dir().join("remote_latest_per_major.json")
    }

    pub fn list_remote(&self, filter: &RemoteFilter) -> EnvrResult<Vec<RuntimeVersion>> {
        let v = self.load_version_list()?;
        crate::index::list_remote_versions(&v, filter)
    }

    pub fn list_remote_latest_per_major(&self) -> EnvrResult<Vec<RuntimeVersion>> {
        let v = self.load_version_list()?;
        Ok(crate::index::list_remote_latest_per_major_lines(&v))
    }

    pub fn try_load_remote_latest_per_major_from_disk(&self) -> Vec<RuntimeVersion> {
        let path = self.remote_latest_per_major_cache_path();
        let Some(list) =
            envr_platform::cache_recovery::read_json_string_list(&path, None, |xs| !xs.is_empty())
        else {
            return Vec::new();
        };
        list.into_iter().map(RuntimeVersion).collect()
    }

    pub fn persist_remote_latest_per_major_cache(&self, list: &[RuntimeVersion]) -> EnvrResult<()> {
        fs::create_dir_all(self.paths.cache_dir())?;
        let path = self.remote_latest_per_major_cache_path();
        let labels: Vec<&str> = list.iter().map(|v| v.0.as_str()).collect();
        let s = serde_json::to_string(&labels).map_err(|e| {
            EnvrError::with_source(ErrorCode::Validation, "json encode lua latest labels", e)
        })?;
        envr_platform::fs_atomic::write_atomic(&path, s.as_bytes())?;
        Ok(())
    }

    pub fn list_remote_latest_per_major_cached(&self) -> EnvrResult<Vec<RuntimeVersion>> {
        let ttl_secs = Self::index_ttl_secs();
        let cache_file = self.remote_latest_per_major_cache_path();
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
        let v = self.load_version_list()?;
        crate::index::resolve_lua_version(&v, spec)
    }

    pub fn install_resolved_version(
        &self,
        version_label: &str,
        progress_downloaded: Option<&Arc<AtomicU64>>,
        progress_total: Option<&Arc<AtomicU64>>,
        cancel: Option<&Arc<AtomicBool>>,
    ) -> EnvrResult<RuntimeVersion> {
        let host = lua_host_kind()?;
        let settings = load_settings_cached()?;
        let url = maybe_mirror_url(
            &settings,
            &sourceforge_tools_download_url(version_label, host)?,
        )?;
        let sha256 = try_fetch_sha256_sidecar(&self.client, &url);

        let fname = tools_executable_filename(version_label, host)?;
        let ext = if fname.ends_with(".zip") {
            ".zip"
        } else if fname.ends_with(".tar.gz") {
            ".tar.gz"
        } else {
            return Err(EnvrError::Validation(format!(
                "unsupported lua archive suffix: {fname}"
            )));
        };

        let cache_dir = self.paths.cache_dir().join(version_label);
        let archive_path = cache_dir.join(format!("lua{ext}"));
        let final_dir = self.paths.version_dir(version_label);
        execute_install_pipeline(
            cancel,
            || fs::create_dir_all(&cache_dir).map_err(EnvrError::from),
            || {
                download_to_path(
                    &self.client,
                    &url,
                    &archive_path,
                    progress_downloaded,
                    progress_total,
                    cancel,
                )
            },
            || {
                if let Some(ref h) = sha256 {
                    checksum::verify_sha256_hex(&archive_path, h)?;
                }
                Ok(())
            },
            || {
                let staging_parent = cache_dir.join("extract_staging");
                fs::create_dir_all(&staging_parent).map_err(EnvrError::from)?;
                let staging = tempfile::tempdir_in(&staging_parent).map_err(EnvrError::from)?;
                extract::extract_archive(&archive_path, staging.path())?;
                promote_lua_extracted_tree(staging.path(), &final_dir)
            },
            || {
                let resolved = RuntimeVersion(version_label.to_string());
                self.set_current(&resolved)?;
                Ok(resolved)
            },
        )
    }

    pub fn set_current(&self, version: &RuntimeVersion) -> EnvrResult<()> {
        let dir = self.paths.version_dir(&version.0);
        if !lua_installation_valid(&dir) {
            return Err(EnvrError::Validation(format!(
                "lua {} is not installed under {}",
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

impl SpecDrivenInstaller for LuaManager {
    fn install_from_spec(&self, request: &InstallRequest) -> EnvrResult<RuntimeVersion> {
        let label = self.resolve_label(&request.spec.0)?;
        let (downloaded, total, cancel) = install_progress_handles(request);
        self.install_resolved_version(&label, downloaded, total, cancel)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch_lua_exe(home: &Path) {
        fs::create_dir_all(home).expect("mkdir");
        #[cfg(windows)]
        fs::write(home.join("lua.exe"), b"").expect("touch");
        #[cfg(not(windows))]
        fs::write(home.join("lua"), b"").expect("touch");
    }

    #[test]
    fn uninstall_clears_current() {
        let tmp = tempfile::tempdir().expect("tmp");
        let mgr = LuaManager::try_new(
            tmp.path().to_path_buf(),
            crate::index::DEFAULT_LUA_DOWNLOAD_PAGE_URL.to_string(),
        )
        .expect("mgr");
        let ver = RuntimeVersion("5.4.8".to_string());
        let home = mgr.paths.version_dir(&ver.0);
        touch_lua_exe(&home);
        mgr.set_current(&ver).expect("set");
        assert_eq!(read_current(&mgr.paths).expect("cur"), Some(ver.clone()));
        mgr.uninstall(&ver).expect("rm");
        assert!(!home.exists());
        assert_eq!(read_current(&mgr.paths).expect("cur2"), None);
    }
}
