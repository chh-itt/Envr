use crate::index::{
    DEFAULT_DENO_TAGS_API, Tag, blocking_http_client, fetch_tags, list_remote_versions, parse_tags,
    resolve_deno_version,
};
use envr_domain::runtime::{RuntimeVersion, VersionSpec};
use envr_download::{checksum, extract};
use envr_error::{EnvrError, EnvrResult};
use envr_platform::links::{LinkType, ensure_link};
use std::fs;
use std::path::{Path, PathBuf};

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

fn dl_url_for(version: &str) -> EnvrResult<String> {
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    let tuple = match (os, arch) {
        ("windows", "x86_64") => "x86_64-pc-windows-msvc",
        ("windows", "aarch64") => "aarch64-pc-windows-msvc",
        ("linux", "x86_64") => "x86_64-unknown-linux-gnu",
        ("linux", "aarch64") => "aarch64-unknown-linux-gnu",
        ("macos", "x86_64") => "x86_64-apple-darwin",
        ("macos", "aarch64") => "aarch64-apple-darwin",
        _ => {
            return Err(EnvrError::Platform(format!(
                "unsupported host for deno install: {os}-{arch}"
            )));
        }
    };
    Ok(format!(
        "https://dl.deno.land/release/v{version}/deno-{tuple}.zip"
    ))
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

fn promote_archive(staging: &Path, final_dir: &Path) -> EnvrResult<()> {
    // Deno zips usually contain the binary at root; keep it in `final_dir`.
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
        let body = fetch_tags(&self.client, &self.tags_api)?;
        parse_tags(&body)
    }

    pub fn list_remote(
        &self,
        filter: &envr_domain::runtime::RemoteFilter,
    ) -> EnvrResult<Vec<RuntimeVersion>> {
        let tags = self.load_tags()?;
        list_remote_versions(&tags, filter)
    }

    pub fn install_from_spec(&self, spec: &VersionSpec) -> EnvrResult<RuntimeVersion> {
        let tags = self.load_tags()?;
        let v = resolve_deno_version(&tags, &spec.0)?;
        self.install_resolved_version(&RuntimeVersion(v))
    }

    pub fn install_resolved_version(&self, version: &RuntimeVersion) -> EnvrResult<RuntimeVersion> {
        let url = dl_url_for(&version.0)?;
        fs::create_dir_all(self.paths.cache_dir()).map_err(EnvrError::from)?;
        let cache_file = self.paths.cache_dir().join(&version.0).join("deno.zip");
        download_to_path(&self.client, &url, &cache_file)?;

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
        if !deno_installation_valid(&final_dir) {
            return Err(EnvrError::Validation(
                "extracted deno layout missing deno executable".into(),
            ));
        }
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

impl Default for DenoManager {
    fn default() -> Self {
        let root = envr_platform::paths::current_platform_paths()
            .map(|p| p.runtime_root)
            .unwrap_or_else(|_| PathBuf::from("."));
        Self::try_new(root, DEFAULT_DENO_TAGS_API.to_string()).expect("manager")
    }
}
