use crate::index::{
    DEFAULT_BUN_TAGS_API, Tag, blocking_http_client, fetch_all_tags, resolve_bun_version,
};
use crate::mirror::{load_settings, maybe_mirror_url};
use envr_domain::runtime::{InstallRequest, RuntimeVersion};
use envr_download::{checksum, extract};
use envr_error::{EnvrError, EnvrResult};
use envr_platform::links::{LinkType, ensure_link};
use std::error::Error;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

fn error_chain_message(err: &dyn Error) -> String {
    let mut s = err.to_string();
    let mut cur = err.source();
    while let Some(x) = cur {
        s.push_str(": ");
        s.push_str(&x.to_string());
        cur = x.source();
    }
    s
}

fn exact_semver_spec(spec: &str) -> Option<String> {
    let s = spec.trim().trim_start_matches('v');
    let mut it = s.split('.');
    let a = it.next()?;
    let b = it.next()?;
    let c = it.next()?;
    if it.next().is_some() {
        return None;
    }
    if [a, b, c].iter().all(|p| !p.is_empty() && p.chars().all(|ch| ch.is_ascii_digit())) {
        if a.parse::<u64>().ok()? >= 1 {
            return Some(format!("{a}.{b}.{c}"));
        }
    }
    None
}

#[derive(Debug, Clone)]
pub struct BunPaths {
    runtime_root: PathBuf,
}

impl BunPaths {
    pub fn new(runtime_root: PathBuf) -> Self {
        Self { runtime_root }
    }

    pub fn bun_home(&self) -> PathBuf {
        self.runtime_root.join("runtimes").join("bun")
    }

    pub fn versions_dir(&self) -> PathBuf {
        self.bun_home().join("versions")
    }

    pub fn current_link(&self) -> PathBuf {
        self.bun_home().join("current")
    }

    pub fn cache_dir(&self) -> PathBuf {
        self.runtime_root.join("cache").join("bun")
    }

    pub fn version_dir(&self, version_label: &str) -> PathBuf {
        self.versions_dir().join(version_label)
    }
}

fn bun_binary_candidates(home: &Path) -> [PathBuf; 2] {
    #[cfg(windows)]
    {
        [home.join("bun.exe"), home.join("bin").join("bun.exe")]
    }
    #[cfg(not(windows))]
    {
        [home.join("bun"), home.join("bin").join("bun")]
    }
}

pub fn bun_installation_valid(home: &Path) -> bool {
    bun_binary_candidates(home).iter().any(|p| p.is_file())
}

pub fn list_installed_versions(paths: &BunPaths) -> EnvrResult<Vec<RuntimeVersion>> {
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
        if bun_installation_valid(&p) {
            out.push(RuntimeVersion(e.file_name().to_string_lossy().into_owned()));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

pub fn read_current(paths: &BunPaths) -> EnvrResult<Option<RuntimeVersion>> {
    let cur = paths.current_link();
    if !cur.exists() {
        return Ok(None);
    }
    let resolved = if cur.is_file() {
        let Ok(s) = fs::read_to_string(&cur) else {
            return Ok(None);
        };
        let t = s.trim();
        if t.is_empty() {
            return Ok(None);
        }
        PathBuf::from(t)
    } else {
        let target = match fs::read_link(&cur) {
            Ok(t) => t,
            Err(_) => return Ok(None),
        };
        if target.is_relative() {
            cur.parent().map(|p| p.join(&target)).unwrap_or(target)
        } else {
            target
        }
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

fn download_to_path_once(
    client: &reqwest::blocking::Client,
    url: &str,
    path: &Path,
    progress_downloaded: Option<&Arc<AtomicU64>>,
    progress_total: Option<&Arc<AtomicU64>>,
    cancel: Option<&Arc<AtomicBool>>,
) -> EnvrResult<()> {
    if cancel.is_some_and(|c| c.load(Ordering::Relaxed)) {
        return Err(EnvrError::Download("download cancelled".to_string()));
    }
    let mut response = client
        .get(url)
        .send()
        .map_err(|e| EnvrError::Download(error_chain_message(&e)))?;
    if !response.status().is_success() {
        return Err(EnvrError::Download(format!(
            "GET {} -> {}",
            url,
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
            return Err(EnvrError::Download("download cancelled".to_string()));
        }
        let n = response
            .read(&mut buf)
            .map_err(|e| EnvrError::Download(e.to_string()))?;
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

fn should_retry_download_error(msg: &str) -> bool {
    let m = msg.to_ascii_lowercase();
    m.contains("timed out")
        || m.contains("timeout")
        || m.contains("connection reset")
        || m.contains("connection refused")
        || m.contains("connection aborted")
        || m.contains("error 10054")
        || m.contains("error 10060")
}

fn download_to_path_with_retries(
    client: &reqwest::blocking::Client,
    url: &str,
    path: &Path,
    progress_downloaded: Option<&Arc<AtomicU64>>,
    progress_total: Option<&Arc<AtomicU64>>,
    cancel: Option<&Arc<AtomicBool>>,
) -> EnvrResult<()> {
    const MAX_ATTEMPTS: usize = 3;
    let mut last_err: Option<EnvrError> = None;
    for attempt in 1..=MAX_ATTEMPTS {
        match download_to_path_once(
            client,
            url,
            path,
            progress_downloaded,
            progress_total,
            cancel,
        ) {
            Ok(()) => return Ok(()),
            Err(e) => {
                let msg = e.to_string();
                last_err = Some(e);
                if attempt >= MAX_ATTEMPTS || !should_retry_download_error(&msg) {
                    break;
                }
                thread::sleep(Duration::from_millis(300 * attempt as u64));
            }
        }
    }
    Err(last_err.unwrap_or_else(|| EnvrError::Download("download failed".into())))
}

/// If `fallback_url` differs from `url`, retries the download from `fallback_url` when the first attempt fails.
fn download_to_path(
    client: &reqwest::blocking::Client,
    url: &str,
    path: &Path,
    progress_downloaded: Option<&Arc<AtomicU64>>,
    progress_total: Option<&Arc<AtomicU64>>,
    cancel: Option<&Arc<AtomicBool>>,
    fallback_url: Option<&str>,
) -> EnvrResult<()> {
    let primary = download_to_path_with_retries(
        client,
        url,
        path,
        progress_downloaded,
        progress_total,
        cancel,
    );
    let Some(fb) = fallback_url.filter(|fb| *fb != url) else {
        return primary;
    };
    primary.or_else(|e| {
        download_to_path_with_retries(
            client,
            fb,
            path,
            progress_downloaded,
            progress_total,
            cancel,
        )
        .map_err(|e2| {
            EnvrError::Download(format!(
                "{e} (retried official URL {fb}: {e2})"
            ))
        })
    })
}

fn get_text(client: &reqwest::blocking::Client, url: &str) -> EnvrResult<String> {
    let response = client
        .get(url)
        .send()
        .map_err(|e| EnvrError::Download(error_chain_message(&e)))?;
    if !response.status().is_success() {
        return Err(EnvrError::Download(format!(
            "GET {} -> {}",
            url,
            response.status()
        )));
    }
    response
        .text()
        .map_err(|e| EnvrError::Download(e.to_string()))
}

/// Tries `primary` first; if it differs from `official`, retries `official` on failure (broken mirror / proxy).
fn get_text_with_official_fallback(
    client: &reqwest::blocking::Client,
    primary: &str,
    official: &str,
) -> EnvrResult<String> {
    let first = get_text(client, primary);
    if primary == official {
        return first;
    }
    first.or_else(|e| {
        get_text(client, official).map_err(|e2| {
            EnvrError::Download(format!(
                "{e} (retried official URL {official}: {e2})"
            ))
        })
    })
}

fn parse_shasums256(text: &str) -> EnvrResult<Vec<(String, String)>> {
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.split_whitespace();
        let h = parts.next().unwrap_or("").trim();
        let n = parts.next().unwrap_or("").trim();
        if h.len() >= 64 && !n.is_empty() {
            out.push((h.to_string(), n.to_string()));
        }
    }
    Ok(out)
}

fn pick_bun_asset(os: &str, arch: &str) -> EnvrResult<&'static str> {
    match (os, arch) {
        ("windows", "x86_64") => Ok("bun-windows-x64.zip"),
        ("windows", "aarch64") => Ok("bun-windows-aarch64.zip"),
        ("linux", "x86_64") => Ok("bun-linux-x64.zip"),
        ("linux", "aarch64") => Ok("bun-linux-aarch64.zip"),
        ("macos", "x86_64") => Ok("bun-darwin-x64.zip"),
        ("macos", "aarch64") => Ok("bun-darwin-aarch64.zip"),
        _ => Err(EnvrError::Platform(format!(
            "unsupported host for bun install: {os}-{arch}"
        ))),
    }
}

fn source_dir_for_promotion(staging: &Path) -> EnvrResult<PathBuf> {
    let mut entries = Vec::new();
    for e in fs::read_dir(staging).map_err(EnvrError::from)? {
        entries.push(e.map_err(EnvrError::from)?);
    }
    if entries.len() == 1 {
        let only = entries.remove(0).path();
        if only.is_dir() && bun_binary_candidates(&only).iter().any(|p| p.is_file()) {
            return Ok(only);
        }
    }
    Ok(staging.to_path_buf())
}

fn promote_archive(staging: &Path, final_dir: &Path) -> EnvrResult<()> {
    if final_dir.exists() {
        fs::remove_dir_all(final_dir).map_err(EnvrError::from)?;
    }
    fs::create_dir_all(final_dir).map_err(EnvrError::from)?;
    let source_dir = source_dir_for_promotion(staging)?;
    for e in fs::read_dir(&source_dir).map_err(EnvrError::from)? {
        let e = e.map_err(EnvrError::from)?;
        let from = e.path();
        let to = final_dir.join(e.file_name());
        fs::rename(&from, &to).map_err(EnvrError::from)?;
    }
    Ok(())
}

pub struct BunManager {
    pub paths: BunPaths,
    tags_api: String,
    client: reqwest::blocking::Client,
}

impl BunManager {
    pub fn try_new(runtime_root: PathBuf, tags_api: String) -> EnvrResult<Self> {
        Ok(Self {
            paths: BunPaths::new(runtime_root),
            tags_api,
            client: blocking_http_client()?,
        })
    }

    fn load_tags(&self) -> EnvrResult<Vec<Tag>> {
        let settings = load_settings()?;
        let url = maybe_mirror_url(&settings, &self.tags_api)?;
        fetch_all_tags(&self.client, &url)
    }

    pub fn install_from_spec(&self, request: &InstallRequest) -> EnvrResult<RuntimeVersion> {
        if let Some(v) = exact_semver_spec(&request.spec.0) {
            return self.install_resolved_version(
                &RuntimeVersion(v),
                request.progress_downloaded.as_ref(),
                request.progress_total.as_ref(),
                request.cancel.as_ref(),
            );
        }
        let tags = self.load_tags()?;
        let v = resolve_bun_version(&tags, &request.spec.0)?;
        self.install_resolved_version(
            &RuntimeVersion(v),
            request.progress_downloaded.as_ref(),
            request.progress_total.as_ref(),
            request.cancel.as_ref(),
        )
    }

    pub fn install_resolved_version(
        &self,
        version: &RuntimeVersion,
        progress_downloaded: Option<&Arc<AtomicU64>>,
        progress_total: Option<&Arc<AtomicU64>>,
        cancel: Option<&Arc<AtomicBool>>,
    ) -> EnvrResult<RuntimeVersion> {
        if cancel.is_some_and(|c| c.load(Ordering::Relaxed)) {
            return Err(EnvrError::Download("download cancelled".to_string()));
        }
        let os = std::env::consts::OS;
        let arch = std::env::consts::ARCH;
        let asset = pick_bun_asset(os, arch)?;
        let tag = format!("bun-v{}", version.0);

        let base = format!("https://github.com/oven-sh/bun/releases/download/{tag}");
        let settings = load_settings()?;
        let direct_shasums = format!("{base}/SHASUMS256.txt");
        let shasums_url = maybe_mirror_url(&settings, &direct_shasums)?;
        let expected_sha = get_text_with_official_fallback(&self.client, &shasums_url, &direct_shasums)
            .ok()
            .and_then(|text| parse_shasums256(&text).ok())
            .and_then(|entries| {
                entries
                    .into_iter()
                    .find(|(_, n)| n == asset)
                    .map(|(sha, _)| sha)
            });

        fs::create_dir_all(self.paths.cache_dir()).map_err(EnvrError::from)?;
        let cache_file = self.paths.cache_dir().join(&version.0).join(asset);
        let direct_zip = format!("{base}/{asset}");
        let zip_url = maybe_mirror_url(&settings, &direct_zip)?;
        download_to_path(
            &self.client,
            &zip_url,
            &cache_file,
            progress_downloaded,
            progress_total,
            cancel,
            Some(direct_zip.as_str()),
        )?;
        if let Some(sha) = expected_sha.as_deref() {
            checksum::verify_sha256_hex(&cache_file, sha)?;
        }

        let staging_parent = self.paths.cache_dir().join(&version.0);
        fs::create_dir_all(&staging_parent).map_err(EnvrError::from)?;
        let staging = tempfile::tempdir_in(&staging_parent).map_err(EnvrError::from)?;
        extract::extract_archive(&cache_file, staging.path())?;

        let final_dir = self.paths.version_dir(&version.0);
        promote_archive(staging.path(), &final_dir)?;
        if !bun_installation_valid(&final_dir) {
            return Err(EnvrError::Validation(
                "extracted bun layout missing bun executable".into(),
            ));
        }
        self.set_current(version)?;
        Ok(RuntimeVersion(version.0.clone()))
    }

    pub fn set_current(&self, version: &RuntimeVersion) -> EnvrResult<()> {
        let dir = self.paths.version_dir(&version.0);
        if !bun_installation_valid(&dir) {
            return Err(EnvrError::Validation(format!(
                "bun {} is not installed",
                version.0
            )));
        }
        let target = fs::canonicalize(&dir).unwrap_or(dir.clone());
        let link = self.paths.current_link();
        if let Err(_e) = ensure_link(LinkType::Soft, &target, &link) {
            #[cfg(windows)]
            {
                if let Some(parent) = link.parent() {
                    fs::create_dir_all(parent).map_err(EnvrError::from)?;
                }
                fs::write(&link, target.display().to_string()).map_err(EnvrError::from)?;
                return Ok(());
            }
            #[cfg(not(windows))]
            {
                return Err(_e);
            }
        }
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

    pub fn clear_download_cache(&self) -> EnvrResult<()> {
        let dir = self.paths.cache_dir();
        if dir.is_dir() {
            fs::remove_dir_all(&dir).map_err(EnvrError::from)?;
        }
        Ok(())
    }
}

impl Default for BunManager {
    fn default() -> Self {
        let root = envr_platform::paths::current_platform_paths()
            .map(|p| p.runtime_root)
            .unwrap_or_else(|_| PathBuf::from("."));
        Self::try_new(root, DEFAULT_BUN_TAGS_API.to_string()).expect("manager")
    }
}
