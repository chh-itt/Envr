use crate::index::{
    PerlReleaseRow, PerlUpstream, blocking_http_client, fetch_all_perl_release_rows,
    list_remote_latest_per_major_lines, list_remote_versions, parse_cached_install_rows,
    perl_upstream, resolve_perl_version,
};
use envr_domain::installer::{SpecDrivenInstaller, install_progress_handles};
use envr_domain::runtime::{InstallRequest, RemoteFilter, RuntimeVersion};
use envr_download::{checksum, extract};
use envr_error::{EnvrError, EnvrResult, ErrorCode};
use envr_platform::bin_tool_layout::perl_installation_valid;
use envr_platform::links::ensure_runtime_current_symlink_or_pointer;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::SystemTime;

#[derive(Debug, Clone)]
pub struct PerlPaths {
    runtime_root: PathBuf,
}

impl PerlPaths {
    pub fn new(runtime_root: PathBuf) -> Self {
        Self { runtime_root }
    }

    pub fn perl_home(&self) -> PathBuf {
        self.runtime_root.join("runtimes").join("perl")
    }

    pub fn versions_dir(&self) -> PathBuf {
        self.perl_home().join("versions")
    }

    pub fn current_link(&self) -> PathBuf {
        self.perl_home().join("current")
    }

    pub fn cache_dir(&self) -> PathBuf {
        self.runtime_root.join("cache").join("perl")
    }

    pub fn index_cache_file(&self, slug: &str) -> PathBuf {
        self.cache_dir().join(format!("releases_{slug}.json"))
    }

    pub fn version_dir(&self, version_label: &str) -> PathBuf {
        self.versions_dir().join(version_label)
    }
}

pub fn list_installed_versions(paths: &PerlPaths) -> EnvrResult<Vec<RuntimeVersion>> {
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
        if perl_installation_valid(&p) {
            out.push(RuntimeVersion(e.file_name().to_string_lossy().into_owned()));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

pub fn read_current(paths: &PerlPaths) -> EnvrResult<Option<RuntimeVersion>> {
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
    let mut response = client
        .get(url)
        .header("Accept", "application/octet-stream")
        .send()
        .map_err(|e| {
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

fn perl_candidate_dirs(staging: &Path) -> EnvrResult<Vec<PathBuf>> {
    let mut cands = Vec::new();
    if staging.is_dir() {
        cands.push(staging.to_path_buf());
    }
    for e in fs::read_dir(staging).map_err(EnvrError::from)? {
        let e = e.map_err(EnvrError::from)?;
        if !e.file_type().map_err(EnvrError::from)?.is_dir() {
            continue;
        }
        let name = e.file_name().to_string_lossy().to_string();
        if name == "__MACOSX" {
            continue;
        }
        let p = e.path();
        cands.push(p.clone());
        if let Ok(rd2) = fs::read_dir(&p) {
            for e2 in rd2.flatten() {
                if e2.file_type().map_err(EnvrError::from)?.is_dir() {
                    let n2 = e2.file_name().to_string_lossy().to_string();
                    if n2 == "__MACOSX" {
                        continue;
                    }
                    cands.push(e2.path());
                }
            }
        }
    }
    Ok(cands)
}

fn depth_key(staging: &Path, p: &Path) -> (usize, String) {
    let depth = p
        .strip_prefix(staging)
        .map(|r| r.components().count())
        .unwrap_or(usize::MAX);
    let key = p.display().to_string();
    (depth, key)
}

fn pick_perl_home_from_candidates(staging: &Path, candidates: &[PathBuf]) -> EnvrResult<PathBuf> {
    let mut val: Vec<PathBuf> = candidates
        .iter()
        .filter(|p| perl_installation_valid(p))
        .cloned()
        .collect();
    val.sort();
    val.dedup();
    match val.len() {
        0 => Err(EnvrError::Validation(
            "perl archive did not contain bin/perl (expected Strawberry portable or skaji relocatable layout)"
                .into(),
        )),
        1 => Ok(val[0].clone()),
        _ => {
            val.sort_by(|a, b| depth_key(staging, a).cmp(&depth_key(staging, b)));
            Ok(val[0].clone())
        }
    }
}

/// Promote extracted tree into `final_dir` (single version home with `bin/perl`).
pub fn promote_perl_extract(staging: &Path, final_dir: &Path) -> EnvrResult<()> {
    use envr_platform::install_layout;

    install_layout::ensure_final_parent(final_dir)?;
    let staging_final = install_layout::sibling_staging_path(final_dir)?;
    install_layout::remove_if_exists(&staging_final)?;

    let commit_if_valid = |home: &Path| -> EnvrResult<()> {
        if !perl_installation_valid(home) {
            let _ = fs::remove_dir_all(home);
            return Err(EnvrError::Validation(
                "extracted perl layout missing bin/perl".into(),
            ));
        }
        install_layout::commit_staging_dir(home, final_dir)
    };

    if perl_installation_valid(staging) {
        fs::create_dir_all(&staging_final).map_err(EnvrError::from)?;
        install_layout::hoist_directory_children(staging, &staging_final)?;
        return commit_if_valid(&staging_final);
    }

    let cands = perl_candidate_dirs(staging)?;
    let inner = pick_perl_home_from_candidates(staging, &cands)?;
    if inner == staging {
        fs::create_dir_all(&staging_final).map_err(EnvrError::from)?;
        install_layout::hoist_directory_children(staging, &staging_final)?;
        return commit_if_valid(&staging_final);
    }
    fs::rename(&inner, &staging_final).map_err(EnvrError::from)?;
    commit_if_valid(&staging_final)
}

pub struct PerlManager {
    pub paths: PerlPaths,
    releases_url: String,
    client: reqwest::blocking::Client,
    upstream: PerlUpstream,
    index_slug: String,
}

impl PerlManager {
    pub fn try_new(runtime_root: PathBuf, releases_url: String) -> EnvrResult<Self> {
        let upstream = perl_upstream()?;
        let index_slug = match upstream {
            PerlUpstream::StrawberryWindows64 => "strawberry_win64".to_string(),
            PerlUpstream::RelocatableUnix => {
                format!("reloc_{}", crate::index::relocatable_archive_stem()?)
            }
        };
        Ok(Self {
            paths: PerlPaths::new(runtime_root),
            releases_url,
            client: blocking_http_client()?,
            upstream,
            index_slug,
        })
    }

    fn index_ttl_secs() -> u64 {
        const DEFAULT: u64 = 60 * 60;
        std::env::var("ENVR_PERL_RELEASES_CACHE_TTL_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .or_else(|| {
                std::env::var("ENVR_PERL_INDEX_CACHE_TTL_SECS")
                    .ok()
                    .and_then(|s| s.parse().ok())
            })
            .unwrap_or(DEFAULT)
    }

    fn load_rows(&self) -> EnvrResult<Vec<PerlReleaseRow>> {
        let cache_path = self.paths.index_cache_file(&self.index_slug);
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
        let rows = fetch_all_perl_release_rows(&self.client, &self.releases_url, self.upstream)?;
        fs::create_dir_all(self.paths.cache_dir()).map_err(EnvrError::from)?;
        let body = serde_json::to_string(&rows).map_err(|e| {
            EnvrError::with_source(ErrorCode::Validation, "json encode perl rows", e)
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
        resolve_perl_version(&rows, spec)
    }

    fn row_for_version<'a>(
        &'a self,
        rows: &'a [PerlReleaseRow],
        version_label: &str,
    ) -> EnvrResult<&'a PerlReleaseRow> {
        rows.iter()
            .find(|r| r.version == version_label)
            .ok_or_else(|| {
                EnvrError::Validation(format!(
                    "perl {version_label} not found in releases index for this host"
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
        } else if url.ends_with(".tar.xz") {
            ".tar.xz"
        } else if url.ends_with(".tar.gz") {
            ".tar.gz"
        } else {
            return Err(EnvrError::Validation(format!(
                "unsupported perl archive URL suffix: {url}"
            )));
        };
        let cache_dir = self.paths.cache_dir().join(version_label);
        fs::create_dir_all(&cache_dir).map_err(EnvrError::from)?;
        let archive_path = cache_dir.join(format!("perl{ext}"));
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
        promote_perl_extract(staging.path(), &final_dir)?;
        self.set_current(&RuntimeVersion(version_label.to_string()))?;
        Ok(RuntimeVersion(version_label.to_string()))
    }

    pub fn set_current(&self, version: &RuntimeVersion) -> EnvrResult<()> {
        let dir = self.paths.version_dir(&version.0);
        if !perl_installation_valid(&dir) {
            return Err(EnvrError::Validation(format!(
                "perl {} is not installed under {}",
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

impl SpecDrivenInstaller for PerlManager {
    fn install_from_spec(&self, request: &InstallRequest) -> EnvrResult<RuntimeVersion> {
        let label = self.resolve_label(&request.spec.0)?;
        let (downloaded, total, cancel) = install_progress_handles(request);
        self.install_resolved_version(&label, downloaded, total, cancel)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use envr_platform::bin_tool_layout::resolve_perl_exe;

    fn write_mock_perl_bin(home: &Path) {
        fs::create_dir_all(home.join("bin")).expect("bin");
        #[cfg(windows)]
        fs::write(home.join("bin").join("perl.exe"), b"").expect("touch");
        #[cfg(not(windows))]
        fs::write(home.join("bin").join("perl"), b"").expect("touch");
    }

    #[test]
    fn promote_nested_perl_subdir() {
        let tmp = tempfile::tempdir().expect("tmp");
        let staging = tmp.path().join("extract");
        let inner = staging.join("portable").join("perl");
        fs::create_dir_all(&inner).expect("mkdir");
        write_mock_perl_bin(&inner);
        let final_dir = tmp.path().join("versions").join("5.40.2.1");
        promote_perl_extract(&staging, &final_dir).expect("promote");
        assert!(perl_installation_valid(&final_dir));
        assert!(resolve_perl_exe(&final_dir).is_some());
    }

    #[test]
    fn promote_flat_bin_layout() {
        let tmp = tempfile::tempdir().expect("tmp");
        let staging = tmp.path().join("extract");
        fs::create_dir_all(&staging).expect("mkdir");
        write_mock_perl_bin(&staging);
        let final_dir = tmp.path().join("versions").join("5.42.0.0");
        promote_perl_extract(&staging, &final_dir).expect("promote");
        assert!(perl_installation_valid(&final_dir));
    }

    #[test]
    fn uninstall_removes_version_and_clears_current() {
        let tmp = tempfile::tempdir().expect("tmp");
        let runtime_root = tmp.path().to_path_buf();
        let mgr = PerlManager {
            paths: PerlPaths::new(runtime_root.clone()),
            releases_url: String::new(),
            client: blocking_http_client().expect("client"),
            upstream: perl_upstream().expect("host"),
            index_slug: "test".into(),
        };
        let ver = RuntimeVersion("5.40.0.0".to_string());
        let home = mgr.paths.version_dir(&ver.0);
        write_mock_perl_bin(&home);
        mgr.set_current(&ver).expect("set current");
        assert_eq!(
            read_current(&mgr.paths).expect("read current"),
            Some(ver.clone())
        );
        mgr.uninstall(&ver).expect("uninstall");
        assert!(!home.exists());
        assert_eq!(read_current(&mgr.paths).expect("after"), None);
    }
}
