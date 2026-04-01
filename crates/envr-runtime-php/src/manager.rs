use crate::index::{
    DEFAULT_PHP_WINDOWS_RELEASES_JSON_URL, ReleaseLine, blocking_http_client,
    fetch_php_windows_releases_json, parse_php_windows_index, pick_windows_zip,
    resolve_php_version,
};
use envr_domain::runtime::{RuntimeVersion, VersionSpec};
use envr_download::{checksum, extract};
use envr_error::{EnvrError, EnvrResult};
use envr_platform::links::{LinkType, ensure_link};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct PhpPaths {
    runtime_root: PathBuf,
}

impl PhpPaths {
    pub fn new(runtime_root: PathBuf) -> Self {
        Self { runtime_root }
    }

    pub fn php_home(&self) -> PathBuf {
        self.runtime_root.join("runtimes").join("php")
    }

    pub fn versions_dir(&self) -> PathBuf {
        self.php_home().join("versions")
    }

    pub fn current_link(&self) -> PathBuf {
        self.php_home().join("current")
    }

    pub fn cache_dir(&self) -> PathBuf {
        self.runtime_root.join("cache").join("php")
    }

    pub fn version_dir(&self, version_label: &str) -> PathBuf {
        self.versions_dir().join(version_label)
    }
}

pub fn php_installation_valid(home: &Path) -> bool {
    #[cfg(windows)]
    {
        home.join("php.exe").is_file()
    }
    #[cfg(not(windows))]
    {
        home.join("bin").join("php").is_file() || home.join("php").is_file()
    }
}

pub fn list_installed_versions(paths: &PhpPaths) -> EnvrResult<Vec<RuntimeVersion>> {
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
        if php_installation_valid(&p) {
            out.push(RuntimeVersion(e.file_name().to_string_lossy().into_owned()));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

pub fn read_current(paths: &PhpPaths) -> EnvrResult<Option<RuntimeVersion>> {
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

fn move_dir_contents(src: &Path, dst: &Path) -> EnvrResult<()> {
    fs::create_dir_all(dst).map_err(EnvrError::from)?;
    for e in fs::read_dir(src).map_err(EnvrError::from)? {
        let e = e.map_err(EnvrError::from)?;
        let from = e.path();
        let to = dst.join(e.file_name());
        if to.exists() {
            if to.is_dir() {
                fs::remove_dir_all(&to).map_err(EnvrError::from)?;
            } else {
                fs::remove_file(&to).map_err(EnvrError::from)?;
            }
        }
        fs::rename(&from, &to).map_err(EnvrError::from)?;
    }
    Ok(())
}

fn promote_archive_layout(staging: &Path, final_dir: &Path) -> EnvrResult<()> {
    // Prefer "single root directory" layouts.
    let mut iter = fs::read_dir(staging).map_err(EnvrError::from)?;
    let first = iter.next().transpose().map_err(EnvrError::from)?;
    let second = iter.next().transpose().map_err(EnvrError::from)?;

    if let (Some(first), None) = (first, second) {
        let p = first.path();
        if p.is_dir() && php_installation_valid(&p) {
            if final_dir.exists() {
                fs::remove_dir_all(final_dir).map_err(EnvrError::from)?;
            }
            fs::rename(&p, final_dir).map_err(EnvrError::from)?;
            return Ok(());
        }
    }

    // Otherwise assume files live at the archive root.
    if final_dir.exists() {
        fs::remove_dir_all(final_dir).map_err(EnvrError::from)?;
    }
    fs::create_dir_all(final_dir).map_err(EnvrError::from)?;
    move_dir_contents(staging, final_dir)?;
    Ok(())
}

pub struct PhpManager {
    pub paths: PhpPaths,
    releases_json_url: String,
    client: reqwest::blocking::Client,
}

impl PhpManager {
    pub fn try_new(runtime_root: PathBuf, releases_json_url: String) -> EnvrResult<Self> {
        Ok(Self {
            paths: PhpPaths::new(runtime_root),
            releases_json_url,
            client: blocking_http_client()?,
        })
    }

    fn load_index(&self) -> EnvrResult<std::collections::HashMap<String, ReleaseLine>> {
        let body = fetch_php_windows_releases_json(&self.client, &self.releases_json_url)?;
        parse_php_windows_index(&body)
    }

    pub fn install_from_spec(&self, spec: &VersionSpec) -> EnvrResult<RuntimeVersion> {
        if !cfg!(windows) {
            return Err(EnvrError::Platform(
                "php install is currently supported on Windows only".into(),
            ));
        }
        let idx = self.load_index()?;
        let version = resolve_php_version(&idx, &spec.0)?;
        self.install_resolved_version(&RuntimeVersion(version))
    }

    pub fn install_resolved_version(&self, version: &RuntimeVersion) -> EnvrResult<RuntimeVersion> {
        if !cfg!(windows) {
            return Err(EnvrError::Platform(
                "php install is currently supported on Windows only".into(),
            ));
        }
        let idx = self.load_index()?;
        let line = idx
            .values()
            .find(|l| l.version == version.0)
            .ok_or_else(|| {
                EnvrError::Validation(format!("php version not found: {}", version.0))
            })?;

        let (zip_name, sha) = pick_windows_zip(line, None, std::env::consts::ARCH)?;
        fs::create_dir_all(self.paths.cache_dir()).map_err(EnvrError::from)?;
        let cache_file = self.paths.cache_dir().join(&version.0).join(&zip_name);
        let url = format!(
            "{}/{}",
            DEFAULT_PHP_WINDOWS_RELEASES_JSON_URL
                .trim_end_matches("releases.json")
                .trim_end_matches('/'),
            zip_name
        );

        download_to_path(&self.client, &url, &cache_file)?;
        if !sha.trim().is_empty() {
            checksum::verify_sha256_hex(&cache_file, sha.trim())?;
        }

        let staging_parent = self.paths.cache_dir().join(&version.0);
        fs::create_dir_all(&staging_parent).map_err(EnvrError::from)?;
        let staging = tempfile::tempdir_in(&staging_parent).map_err(EnvrError::from)?;
        extract::extract_archive(&cache_file, staging.path())?;

        let final_dir = self.paths.version_dir(&version.0);
        promote_archive_layout(staging.path(), &final_dir)?;
        if !php_installation_valid(&final_dir) {
            return Err(EnvrError::Validation(
                "extracted php layout missing php executable".into(),
            ));
        }

        self.set_current(version)?;
        Ok(RuntimeVersion(version.0.clone()))
    }

    pub fn set_current(&self, version: &RuntimeVersion) -> EnvrResult<()> {
        let dir = self.paths.version_dir(&version.0);
        if !php_installation_valid(&dir) {
            return Err(EnvrError::Validation(format!(
                "php {} is not installed",
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
