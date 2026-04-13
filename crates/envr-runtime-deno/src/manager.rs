use crate::index::{
    DEFAULT_DENO_TAGS_API, Tag, blocking_http_client, fetch_all_tags, list_remote_versions,
    resolve_deno_version,
};
use envr_config::settings::{
    Settings, deno_official_release_zip_url, deno_release_zip_url, settings_path_from_platform,
};
use envr_domain::runtime::{InstallRequest, RuntimeVersion};
use envr_download::{checksum, extract};
use envr_error::{EnvrError, EnvrResult};
use envr_platform::links::{LinkType, ensure_link};
use envr_platform::paths::current_platform_paths;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

#[derive(Debug, Clone)]
pub struct DenoPaths {
    runtime_root: PathBuf,
}

impl DenoPaths {
    pub fn new(runtime_root: PathBuf) -> Self {
        Self { runtime_root }
    }

    pub fn deno_home(&self) -> PathBuf {
        self.runtime_root.join("runtimes").join("deno")
    }

    pub fn versions_dir(&self) -> PathBuf {
        self.deno_home().join("versions")
    }

    pub fn current_link(&self) -> PathBuf {
        self.deno_home().join("current")
    }

    pub fn cache_dir(&self) -> PathBuf {
        self.runtime_root.join("cache").join("deno")
    }

    pub fn version_dir(&self, version_label: &str) -> PathBuf {
        self.versions_dir().join(version_label)
    }
}

pub fn deno_installation_valid(home: &Path) -> bool {
    #[cfg(windows)]
    {
        home.join("deno.exe").is_file() || home.join("bin").join("deno.exe").is_file()
    }
    #[cfg(not(windows))]
    {
        home.join("deno").is_file() || home.join("bin").join("deno").is_file()
    }
}

pub fn list_installed_versions(paths: &DenoPaths) -> EnvrResult<Vec<RuntimeVersion>> {
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
        if deno_installation_valid(&p) {
            out.push(RuntimeVersion(e.file_name().to_string_lossy().into_owned()));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

pub fn read_current(paths: &DenoPaths) -> EnvrResult<Option<RuntimeVersion>> {
    let cur = paths.current_link();
    if !cur.exists() {
        return Ok(None);
    }
    // Prefer symlink/junction, but on some Windows environments we may fall back to a pointer file:
    // `current` contains the absolute target dir.
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

fn exact_semver_spec(spec: &str) -> Option<String> {
    let s = spec.trim().trim_start_matches('v');
    let mut it = s.split('.');
    let a = it.next()?;
    let b = it.next()?;
    let c = it.next()?;
    if it.next().is_some() {
        return None;
    }
    if [a, b, c]
        .iter()
        .all(|p| !p.is_empty() && p.chars().all(|ch| ch.is_ascii_digit()))
        && a.parse::<u64>().ok()? >= 1
    {
        return Some(format!("{a}.{b}.{c}"));
    }
    None
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
        return Err(EnvrError::Download("download cancelled".to_string()));
    }
    let mut response = client
        .get(url)
        .send()
        .map_err(|e| EnvrError::Download(e.to_string()))?;
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

fn download_to_path_with_fallback(
    client: &reqwest::blocking::Client,
    primary_url: &str,
    fallback_url: Option<&str>,
    path: &Path,
    progress_downloaded: Option<&Arc<AtomicU64>>,
    progress_total: Option<&Arc<AtomicU64>>,
    cancel: Option<&Arc<AtomicBool>>,
) -> EnvrResult<()> {
    let first = download_to_path(
        client,
        primary_url,
        path,
        progress_downloaded,
        progress_total,
        cancel,
    );
    let Some(fallback) = fallback_url.filter(|u| *u != primary_url) else {
        return first;
    };
    first.or_else(|e| {
        download_to_path(
            client,
            fallback,
            path,
            progress_downloaded,
            progress_total,
            cancel,
        )
        .map_err(|e2| {
            EnvrError::Download(format!(
                "{e} (retried official URL {fallback}: {e2})"
            ))
        })
    })
}

fn promote_archive(staging: &Path, final_dir: &Path) -> EnvrResult<()> {
    use envr_platform::install_layout;

    install_layout::ensure_final_parent(final_dir)?;
    let staging_final = install_layout::sibling_staging_path(final_dir)?;
    install_layout::remove_if_exists(&staging_final)?;
    install_layout::hoist_directory_children(staging, &staging_final)?;
    if !deno_installation_valid(&staging_final) {
        let _ = fs::remove_dir_all(&staging_final);
        return Err(EnvrError::Validation(
            "extracted deno layout missing deno executable".into(),
        ));
    }
    install_layout::commit_staging_dir(&staging_final, final_dir)?;
    Ok(())
}

pub struct DenoManager {
    pub paths: DenoPaths,
    tags_api: String,
    client: reqwest::blocking::Client,
}

impl DenoManager {
    pub fn try_new(runtime_root: PathBuf, tags_api: String) -> EnvrResult<Self> {
        Ok(Self {
            paths: DenoPaths::new(runtime_root),
            tags_api,
            client: blocking_http_client()?,
        })
    }

    fn load_tags(&self) -> EnvrResult<Vec<Tag>> {
        fetch_all_tags(&self.client, &self.tags_api)
    }

    pub fn list_remote(
        &self,
        filter: &envr_domain::runtime::RemoteFilter,
    ) -> EnvrResult<Vec<RuntimeVersion>> {
        let tags = self.load_tags()?;
        list_remote_versions(&tags, filter)
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
        let v = resolve_deno_version(&tags, &request.spec.0)?;
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
        let platform = current_platform_paths()?;
        let path = settings_path_from_platform(&platform);
        let settings = Settings::load_or_default_from(&path)?;
        let url = deno_release_zip_url(&settings, &version.0)?;
        let official_url = deno_official_release_zip_url(&version.0)?;
        fs::create_dir_all(self.paths.cache_dir()).map_err(EnvrError::from)?;
        let cache_file = self.paths.cache_dir().join(&version.0).join("deno.zip");
        download_to_path_with_fallback(
            &self.client,
            &url,
            Some(&official_url),
            &cache_file,
            progress_downloaded,
            progress_total,
            cancel,
        )?;

        // Optional checksum: dl.deno.land has `.sha256sum` files, but we keep install robust without it.
        let sha_url = format!("{url}.sha256sum");
        if let Ok(s) = self.client.get(&sha_url).send().and_then(|r| r.text()) {
            let hash = s.split_whitespace().next().unwrap_or("").trim().to_string();
            if hash.len() >= 64 {
                let _ = checksum::verify_sha256_hex(&cache_file, &hash);
            }
        }

        let staging_parent = self.paths.cache_dir().join(&version.0);
        fs::create_dir_all(&staging_parent).map_err(EnvrError::from)?;
        let staging = tempfile::tempdir_in(&staging_parent).map_err(EnvrError::from)?;
        extract::extract_archive(&cache_file, staging.path())?;

        let final_dir = self.paths.version_dir(&version.0);
        promote_archive(staging.path(), &final_dir)?;
        self.set_current(version)?;
        Ok(RuntimeVersion(version.0.clone()))
    }

    pub fn set_current(&self, version: &RuntimeVersion) -> EnvrResult<()> {
        let dir = self.paths.version_dir(&version.0);
        if !deno_installation_valid(&dir) {
            return Err(EnvrError::Validation(format!(
                "deno {} is not installed",
                version.0
            )));
        }
        let target = fs::canonicalize(&dir).unwrap_or(dir.clone());
        let link = self.paths.current_link();
        if let Err(_e) = ensure_link(LinkType::Soft, &target, &link) {
            // Fall back to pointer file on Windows when symlink/junction creation is blocked.
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
}

impl Default for DenoManager {
    fn default() -> Self {
        let root = envr_platform::paths::current_platform_paths()
            .map(|p| p.runtime_root)
            .unwrap_or_else(|_| PathBuf::from("."));
        Self::try_new(root, DEFAULT_DENO_TAGS_API.to_string()).expect("manager")
    }
}
