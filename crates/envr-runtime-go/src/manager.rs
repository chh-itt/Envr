use envr_domain::installer::{
    SpecDrivenInstaller, execute_install_pipeline, install_progress_handles,
};
use envr_domain::runtime::{InstallRequest, RuntimeVersion};
use envr_download::{blocking::download_url_to_path_resumable, checksum, extract};
use envr_error::{EnvrError, EnvrResult};
use envr_platform::links::{LinkType, ensure_link};
use reqwest::blocking::Client;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::index::{
    GoDistFile, GoRelease, blocking_http_client, fetch_go_index, go_dl_arch_for_rust,
    go_dl_os_for_rust, normalize_go_version, parse_go_index, resolve_go_version,
};

#[derive(Debug, Clone)]
pub struct GoPaths {
    runtime_root: PathBuf,
}

impl GoPaths {
    pub fn new(runtime_root: PathBuf) -> Self {
        Self { runtime_root }
    }

    pub fn go_home(&self) -> PathBuf {
        self.runtime_root.join("runtimes").join("go")
    }

    pub fn versions_dir(&self) -> PathBuf {
        self.go_home().join("versions")
    }

    pub fn current_link(&self) -> PathBuf {
        self.go_home().join("current")
    }

    pub fn version_dir(&self, version_label: &str) -> PathBuf {
        self.versions_dir().join(version_label)
    }

    pub fn cache_dir(&self) -> PathBuf {
        self.runtime_root.join("cache").join("go")
    }
}

pub fn go_installation_valid(home: &std::path::Path) -> bool {
    #[cfg(windows)]
    {
        home.join("bin").join("go.exe").is_file()
    }
    #[cfg(not(windows))]
    {
        home.join("bin").join("go").is_file()
    }
}

pub fn list_installed_versions(paths: &GoPaths) -> EnvrResult<Vec<RuntimeVersion>> {
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
        if go_installation_valid(&p) {
            out.push(RuntimeVersion(e.file_name().to_string_lossy().into_owned()));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

pub fn read_current(paths: &GoPaths) -> EnvrResult<Option<RuntimeVersion>> {
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

#[derive(Debug, Clone)]
pub struct GoManager {
    paths: GoPaths,
    dl_json_url: String,
    dl_base_url: String,
    client: Client,
}

impl GoManager {
    pub fn try_new(
        runtime_root: PathBuf,
        dl_json_url: String,
        dl_base_url: String,
    ) -> EnvrResult<Self> {
        let paths = GoPaths::new(runtime_root);
        fs::create_dir_all(paths.versions_dir()).map_err(EnvrError::from)?;
        Ok(Self {
            paths,
            dl_json_url,
            dl_base_url,
            client: blocking_http_client()?,
        })
    }

    fn load_releases(&self) -> EnvrResult<Vec<GoRelease>> {
        let body = fetch_go_index(&self.client, &self.dl_json_url)?;
        parse_go_index(&body)
    }

    fn pick_dist_file<'a>(&self, release: &'a GoRelease) -> EnvrResult<&'a GoDistFile> {
        let os = go_dl_os_for_rust(std::env::consts::OS);
        let arch = go_dl_arch_for_rust(std::env::consts::ARCH);
        let want_ext = if os == "windows" { ".zip" } else { ".tar.gz" };
        release
            .files
            .iter()
            .find(|f| {
                f.os == os
                    && f.arch == arch
                    && (f.kind == "archive" || f.kind.is_empty())
                    && f.filename.ends_with(want_ext)
            })
            .ok_or_else(|| {
                EnvrError::Validation(format!(
                    "no Go archive for {} on {}-{}",
                    release.version, os, arch
                ))
            })
    }

    fn download_to_path(
        &self,
        url: &str,
        path: &Path,
        progress_downloaded: Option<&Arc<AtomicU64>>,
        progress_total: Option<&Arc<AtomicU64>>,
        cancel: Option<&Arc<AtomicBool>>,
    ) -> EnvrResult<()> {
        download_url_to_path_resumable(
            &self.client,
            url,
            path,
            progress_downloaded,
            progress_total,
            cancel,
        )
    }

    pub fn install_resolved_version(
        &self,
        version: &RuntimeVersion,
        progress_downloaded: Option<&Arc<AtomicU64>>,
        progress_total: Option<&Arc<AtomicU64>>,
        cancel: Option<&Arc<AtomicBool>>,
    ) -> EnvrResult<RuntimeVersion> {
        let want = format!("go{}", normalize_go_version(&version.0));
        let releases = self.load_releases()?;
        let release = releases
            .iter()
            .find(|r| r.version.eq_ignore_ascii_case(&want))
            .ok_or_else(|| EnvrError::Validation(format!("Go release not found: {}", version.0)))?;
        let dist = self.pick_dist_file(release)?;

        let cache_file = self
            .paths
            .cache_dir()
            .join(normalize_go_version(&version.0))
            .join(&dist.filename);
        let base = self.dl_base_url.trim_end_matches('/');
        let url = format!("{base}/dl/{}", dist.filename);
        let final_dir = self.paths.version_dir(&normalize_go_version(&version.0));
        let normalized = RuntimeVersion(normalize_go_version(&version.0));
        execute_install_pipeline(
            cancel,
            || fs::create_dir_all(self.paths.cache_dir()).map_err(EnvrError::from),
            || {
                self.download_to_path(
                    &url,
                    &cache_file,
                    progress_downloaded,
                    progress_total,
                    cancel,
                )
            },
            || {
                if !dist.sha256.trim().is_empty() {
                    checksum::verify_sha256_hex(&cache_file, dist.sha256.trim())?;
                }
                Ok(())
            },
            || {
                let staging_parent = self
                    .paths
                    .cache_dir()
                    .join(normalize_go_version(&version.0));
                fs::create_dir_all(&staging_parent).map_err(EnvrError::from)?;
                let staging = tempfile::tempdir_in(&staging_parent).map_err(EnvrError::from)?;
                extract::extract_archive(&cache_file, staging.path())?;
                envr_platform::install_layout::commit_single_root_dir(
                    staging.path(),
                    &final_dir,
                    go_installation_valid,
                    "empty go archive",
                    "expected exactly one root directory in go archive",
                    "expected go archive root to be a directory",
                    "extracted go layout missing go binary",
                )
            },
            || {
                self.set_current(&normalized)?;
                Ok(normalized)
            },
        )
    }

    pub fn set_current(&self, version: &RuntimeVersion) -> EnvrResult<()> {
        let dir = self.paths.version_dir(&version.0);
        if !go_installation_valid(&dir) {
            return Err(EnvrError::Validation(format!(
                "Go version {} is not installed under {}",
                version.0,
                dir.display()
            )));
        }
        ensure_link(LinkType::Soft, &dir, self.paths.current_link())
    }

    pub fn uninstall(&self, version: &RuntimeVersion) -> EnvrResult<()> {
        let dir = self.paths.version_dir(&version.0);
        if dir.is_dir() {
            fs::remove_dir_all(&dir).map_err(EnvrError::from)?;
        }
        if let Some(cur) = read_current(&self.paths)?
            && cur.0 == version.0
        {
            let link = self.paths.current_link();
            if link.exists() {
                fs::remove_file(link).map_err(EnvrError::from)?;
            }
        }
        Ok(())
    }
}

impl SpecDrivenInstaller for GoManager {
    fn install_from_spec(&self, request: &InstallRequest) -> EnvrResult<RuntimeVersion> {
        let releases = self.load_releases()?;
        let label = resolve_go_version(&releases, &request.spec.0)?;
        let (downloaded, total, cancel) = install_progress_handles(request);
        self.install_resolved_version(&RuntimeVersion(label), downloaded, total, cancel)
    }
}

// (promote_single_root_dir) migrated to envr_platform::install_layout::commit_single_root_dir
