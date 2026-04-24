use crate::index::{
    ZigPlatformArtifact, artifact_for_platform, blocking_http_client, fetch_index_json,
    find_version_entry, list_remote_latest_per_major_lines, list_remote_versions, parse_index_root,
    resolve_zig_version, zig_json_platform_key,
};
use envr_domain::installer::{SpecDrivenInstaller, install_progress_handles};
use envr_domain::runtime::{InstallRequest, RemoteFilter, RuntimeVersion};
use envr_download::{blocking::download_url_to_path_resumable, checksum, extract};
use envr_error::{EnvrError, EnvrResult};
use envr_platform::bin_tool_layout::zig_installation_valid;
use envr_platform::links::ensure_runtime_current_symlink_or_pointer;
use serde_json::Map;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::SystemTime;

#[derive(Debug, Clone)]
pub struct ZigPaths {
    runtime_root: PathBuf,
}

impl ZigPaths {
    pub fn new(runtime_root: PathBuf) -> Self {
        Self { runtime_root }
    }

    pub fn zig_home(&self) -> PathBuf {
        self.runtime_root.join("runtimes").join("zig")
    }

    pub fn versions_dir(&self) -> PathBuf {
        self.zig_home().join("versions")
    }

    pub fn current_link(&self) -> PathBuf {
        self.zig_home().join("current")
    }

    pub fn cache_dir(&self) -> PathBuf {
        self.runtime_root.join("cache").join("zig")
    }

    pub fn index_cache_file(&self) -> PathBuf {
        self.cache_dir().join("index.json")
    }

    pub fn version_dir(&self, version_label: &str) -> PathBuf {
        self.versions_dir().join(version_label)
    }
}

pub fn list_installed_versions(paths: &ZigPaths) -> EnvrResult<Vec<RuntimeVersion>> {
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
        if zig_installation_valid(&p) {
            out.push(RuntimeVersion(e.file_name().to_string_lossy().into_owned()));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

/// Match deno-style `current`: symlink/junction, or pointer file on Windows.
pub fn read_current(paths: &ZigPaths) -> EnvrResult<Option<RuntimeVersion>> {
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

pub fn promote_single_root_dir(staging: &Path, final_dir: &Path) -> EnvrResult<()> {
    use envr_platform::install_layout;

    let mut iter = fs::read_dir(staging).map_err(EnvrError::from)?;
    let first = iter
        .next()
        .transpose()
        .map_err(EnvrError::from)?
        .ok_or_else(|| EnvrError::Validation("empty zig archive".into()))?;
    if iter.next().transpose().map_err(EnvrError::from)?.is_some() {
        return Err(EnvrError::Validation(
            "expected exactly one root directory in zig archive".into(),
        ));
    }
    let inner = first.path();
    if !inner.is_dir() {
        return Err(EnvrError::Validation(
            "expected zig archive root to be a directory".into(),
        ));
    }
    install_layout::ensure_final_parent(final_dir)?;
    let staging_final = install_layout::sibling_staging_path(final_dir)?;
    install_layout::remove_if_exists(&staging_final)?;

    fs::rename(&inner, &staging_final).map_err(EnvrError::from)?;

    if !zig_installation_valid(&staging_final) {
        let _ = fs::remove_dir_all(&staging_final);
        return Err(EnvrError::Validation(
            "extracted zig layout missing zig executable".into(),
        ));
    }

    install_layout::commit_staging_dir(&staging_final, final_dir)?;
    Ok(())
}

pub struct ZigManager {
    pub paths: ZigPaths,
    index_url: String,
    client: reqwest::blocking::Client,
}

impl ZigManager {
    pub fn try_new(runtime_root: PathBuf, index_url: String) -> EnvrResult<Self> {
        Ok(Self {
            paths: ZigPaths::new(runtime_root),
            index_url,
            client: blocking_http_client()?,
        })
    }

    fn index_ttl_secs() -> u64 {
        const DEFAULT: u64 = 60 * 60;
        std::env::var("ENVR_ZIG_INDEX_CACHE_TTL_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT)
    }

    pub fn load_index_map(&self) -> EnvrResult<Map<String, serde_json::Value>> {
        let cache_path = self.paths.index_cache_file();
        let ttl = Self::index_ttl_secs();
        if let Ok(meta) = fs::metadata(&cache_path) {
            if let Ok(modified) = meta.modified() {
                if let Ok(age) = SystemTime::now().duration_since(modified) {
                    if age.as_secs() < ttl {
                        if let Ok(body) = fs::read_to_string(&cache_path) {
                            if let Ok(m) = parse_index_root(&body) {
                                return Ok(m);
                            }
                        }
                    }
                }
            }
        }
        let body = fetch_index_json(&self.client, &self.index_url)?;
        fs::create_dir_all(self.paths.cache_dir()).map_err(EnvrError::from)?;
        envr_platform::fs_atomic::write_atomic(&cache_path, body.as_bytes())
            .map_err(EnvrError::from)?;
        parse_index_root(&body)
    }

    pub fn list_remote(&self, filter: &RemoteFilter) -> EnvrResult<Vec<RuntimeVersion>> {
        let m = self.load_index_map()?;
        let plat = zig_json_platform_key()?;
        list_remote_versions(&m, plat, filter)
    }

    pub fn list_remote_latest_per_major(&self) -> EnvrResult<Vec<RuntimeVersion>> {
        let m = self.load_index_map()?;
        let plat = zig_json_platform_key()?;
        Ok(list_remote_latest_per_major_lines(&m, plat))
    }

    pub fn resolve_label(&self, spec: &str) -> EnvrResult<String> {
        let m = self.load_index_map()?;
        let plat = zig_json_platform_key()?;
        resolve_zig_version(&m, plat, spec)
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
        let m = self.load_index_map()?;
        let plat = zig_json_platform_key()?;
        let entry = find_version_entry(&m, version_label)?;
        let ZigPlatformArtifact {
            tarball_url,
            shasum_hex,
        } = artifact_for_platform(entry, plat).ok_or_else(|| {
            EnvrError::Validation(format!(
                "no official Zig tarball for `{version_label}` on platform `{plat}`"
            ))
        })?;
        let ext = if tarball_url.ends_with(".zip") {
            ".zip"
        } else if tarball_url.ends_with(".tar.xz") {
            ".tar.xz"
        } else {
            return Err(EnvrError::Validation(format!(
                "unsupported zig archive URL suffix: {tarball_url}"
            )));
        };
        let cache_dir = self.paths.cache_dir().join(version_label);
        fs::create_dir_all(&cache_dir).map_err(EnvrError::from)?;
        let archive_path = cache_dir.join(format!("zig{ext}"));
        download_to_path(
            &self.client,
            &tarball_url,
            &archive_path,
            progress_downloaded,
            progress_total,
            cancel,
        )?;
        if !shasum_hex.is_empty() {
            checksum::verify_sha256_hex(&archive_path, shasum_hex.trim())?;
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
        if !zig_installation_valid(&dir) {
            return Err(EnvrError::Validation(format!(
                "zig {} is not installed under {}",
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

impl SpecDrivenInstaller for ZigManager {
    fn install_from_spec(&self, request: &InstallRequest) -> EnvrResult<RuntimeVersion> {
        let label = self.resolve_label(&request.spec.0)?;
        let (downloaded, total, cancel) = install_progress_handles(request);
        self.install_resolved_version(&label, downloaded, total, cancel)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DEFAULT_ZIG_INDEX_URL;

    fn write_mock_zig_exe(home: &Path) {
        fs::create_dir_all(home).expect("create zig home");
        #[cfg(windows)]
        fs::write(home.join("zig.exe"), b"").expect("touch zig.exe");
        #[cfg(not(windows))]
        fs::write(home.join("zig"), b"").expect("touch zig");
    }

    #[test]
    fn uninstall_removes_selected_version_and_clears_current() {
        let tmp = tempfile::tempdir().expect("tmp");
        let runtime_root = tmp.path().to_path_buf();
        let mgr = ZigManager::try_new(runtime_root.clone(), DEFAULT_ZIG_INDEX_URL.to_string())
            .expect("zig manager");
        let ver = RuntimeVersion("0.14.1".to_string());
        let home = mgr.paths.version_dir(&ver.0);
        write_mock_zig_exe(&home);

        mgr.set_current(&ver).expect("set current");
        assert_eq!(
            read_current(&mgr.paths).expect("read current before uninstall"),
            Some(ver.clone())
        );

        mgr.uninstall(&ver).expect("uninstall");
        assert!(
            !home.exists(),
            "version directory should be removed: {}",
            home.display()
        );
        assert_eq!(
            read_current(&mgr.paths).expect("read current after uninstall"),
            None
        );
    }
}
