use crate::index::{
    DEFAULT_BUN_TAGS_API, Tag, blocking_http_client, fetch_tags, parse_tags, resolve_bun_version,
};
use crate::mirror::{load_settings, maybe_mirror_url};
use envr_domain::runtime::{RuntimeVersion, VersionSpec};
use envr_download::{checksum, extract};
use envr_error::{EnvrError, EnvrResult};
use envr_platform::links::{LinkType, ensure_link};
use std::fs;
use std::path::{Path, PathBuf};

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

pub fn bun_installation_valid(home: &Path) -> bool {
    #[cfg(windows)]
    {
        home.join("bun.exe").is_file()
    }
    #[cfg(not(windows))]
    {
        home.join("bun").is_file()
    }
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
    let target = match fs::read_link(&cur) {
        Ok(t) => t,
        Err(_) => return Ok(None),
    };
    let resolved = if target.is_relative() {
        cur.parent().map(|p| p.join(&target)).unwrap_or(target)
    } else {
        target
    };
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

fn download_to_path(client: &reqwest::blocking::Client, url: &str, path: &Path) -> EnvrResult<()> {
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
    let mut f = fs::File::create(path).map_err(EnvrError::from)?;
    response
        .copy_to(&mut f)
        .map_err(|e| EnvrError::Download(e.to_string()))?;
    Ok(())
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

fn promote_archive(staging: &Path, final_dir: &Path) -> EnvrResult<()> {
    if final_dir.exists() {
        fs::remove_dir_all(final_dir).map_err(EnvrError::from)?;
    }
    fs::create_dir_all(final_dir).map_err(EnvrError::from)?;
    for e in fs::read_dir(staging).map_err(EnvrError::from)? {
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
        let body = fetch_tags(&self.client, &url)?;
        parse_tags(&body)
    }

    pub fn install_from_spec(&self, spec: &VersionSpec) -> EnvrResult<RuntimeVersion> {
        let tags = self.load_tags()?;
        let v = resolve_bun_version(&tags, &spec.0)?;
        self.install_resolved_version(&RuntimeVersion(v))
    }

    pub fn install_resolved_version(&self, version: &RuntimeVersion) -> EnvrResult<RuntimeVersion> {
        let os = std::env::consts::OS;
        let arch = std::env::consts::ARCH;
        let asset = pick_bun_asset(os, arch)?;
        let tag = format!("bun-v{}", version.0);

        let base = format!("https://github.com/oven-sh/bun/releases/download/{tag}");
        let settings = load_settings()?;
        let shasums_url = maybe_mirror_url(&settings, &format!("{base}/SHASUMS256.txt"))?;
        let shasums_text = self
            .client
            .get(&shasums_url)
            .send()
            .map_err(|e| EnvrError::Download(e.to_string()))?
            .text()
            .map_err(|e| EnvrError::Download(e.to_string()))?;
        let entries = parse_shasums256(&shasums_text)?;
        let (sha, _name) = entries
            .iter()
            .find(|(_, n)| n == asset)
            .cloned()
            .ok_or_else(|| {
                EnvrError::Validation(format!("missing {asset} in bun SHASUMS256 for {tag}"))
            })?;

        fs::create_dir_all(self.paths.cache_dir()).map_err(EnvrError::from)?;
        let cache_file = self.paths.cache_dir().join(&version.0).join(asset);
        let url = maybe_mirror_url(&settings, &format!("{base}/{asset}"))?;
        download_to_path(&self.client, &url, &cache_file)?;
        checksum::verify_sha256_hex(&cache_file, &sha)?;

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
        ensure_link(LinkType::Soft, &dir, self.paths.current_link())?;
        Ok(())
    }

    pub fn uninstall(&self, version: &RuntimeVersion) -> EnvrResult<()> {
        let dir = self.paths.version_dir(&version.0);
        if dir.is_dir() {
            fs::remove_dir_all(&dir).map_err(EnvrError::from)?;
        }
        if read_current(&self.paths)?.is_some_and(|c| c.0 == version.0) {
            let _ = fs::remove_file(self.paths.current_link());
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
