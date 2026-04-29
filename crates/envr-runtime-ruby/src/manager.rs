use crate::RubyRelease;
use crate::index::{
    blocking_http_client, fetch_release_page, parse_ruby_releases, resolve_ruby_version,
};
use envr_domain::installer::SpecDrivenInstaller;
use envr_domain::runtime::{InstallRequest, RuntimeVersion};
#[cfg(windows)]
use envr_error::ErrorCode;
use envr_error::{EnvrError, EnvrResult};
use envr_platform::links::{LinkType, ensure_link};
use std::fs;
use std::path::{Path, PathBuf};

#[cfg(windows)]
use crate::index::{
    host_rubyinstaller_arch, parse_rubyinstaller_7z_artifacts, pick_rubyinstaller_artifact,
};
#[cfg(windows)]
use envr_domain::installer::{execute_install_pipeline, install_progress_handles};
#[cfg(windows)]
use envr_download::blocking::download_url_to_path_resumable;
#[cfg(windows)]
use envr_download::extract;
#[cfg(windows)]
use std::collections::HashSet;
#[cfg(windows)]
use std::process::Command;
#[cfg(windows)]
use std::sync::Arc;
#[cfg(windows)]
use std::sync::atomic::{AtomicBool, AtomicU64};

#[derive(Debug, Clone)]
pub struct RubyPaths {
    runtime_root: PathBuf,
}

impl RubyPaths {
    pub fn new(runtime_root: PathBuf) -> Self {
        Self { runtime_root }
    }

    pub fn ruby_home(&self) -> PathBuf {
        self.runtime_root.join("runtimes").join("ruby")
    }

    pub fn versions_dir(&self) -> PathBuf {
        self.ruby_home().join("versions")
    }

    pub fn current_link(&self) -> PathBuf {
        self.ruby_home().join("current")
    }

    pub fn cache_dir(&self) -> PathBuf {
        self.runtime_root.join("cache").join("ruby")
    }

    pub fn version_dir(&self, version_label: &str) -> PathBuf {
        self.versions_dir().join(version_label)
    }
}

fn ruby_executable(home: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        home.join("bin").join("ruby.exe")
    }
    #[cfg(not(windows))]
    {
        home.join("bin").join("ruby")
    }
}

pub fn ruby_installation_valid(home: &Path) -> bool {
    ruby_executable(home).is_file() && gem_executable(home).is_file()
}

fn gem_executable(home: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        home.join("bin").join("gem.cmd")
    }
    #[cfg(not(windows))]
    {
        home.join("bin").join("gem")
    }
}

#[cfg(windows)]
fn bundle_executable_candidates(home: &Path) -> Vec<PathBuf> {
    vec![
        home.join("bin").join("bundle.cmd"),
        home.join("bin").join("bundle.bat"),
        home.join("bin").join("bundle.exe"),
    ]
}

pub fn list_installed_versions(paths: &RubyPaths) -> EnvrResult<Vec<RuntimeVersion>> {
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
        if ruby_installation_valid(&p) {
            out.push(RuntimeVersion(e.file_name().to_string_lossy().into_owned()));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

pub fn read_current(paths: &RubyPaths) -> EnvrResult<Option<RuntimeVersion>> {
    let cur = paths.current_link();
    if !cur.exists() {
        return Ok(None);
    }
    if let Ok(target) = fs::read_link(&cur) {
        let resolved = if target.is_relative() {
            cur.parent().map(|p| p.join(&target)).unwrap_or(target)
        } else {
            target
        };
        let name = resolved
            .file_name()
            .ok_or_else(|| EnvrError::Runtime("invalid ruby current link".into()))?
            .to_string_lossy()
            .into_owned();
        return Ok(Some(RuntimeVersion(name)));
    }
    let s = fs::read_to_string(&cur).map_err(EnvrError::from)?;
    let t = s.trim();
    if t.is_empty() {
        return Ok(None);
    }
    let target = PathBuf::from(t);
    let name = target
        .file_name()
        .ok_or_else(|| EnvrError::Runtime("invalid ruby current pointer".into()))?
        .to_string_lossy()
        .into_owned();
    Ok(Some(RuntimeVersion(name)))
}

fn set_current_pointer_file(cur: &Path, abs_target_dir: &Path) -> EnvrResult<()> {
    if cur.exists() {
        if cur.is_dir() {
            fs::remove_dir_all(cur).map_err(EnvrError::from)?;
        } else {
            fs::remove_file(cur).map_err(EnvrError::from)?;
        }
    }
    if let Some(parent) = cur.parent() {
        fs::create_dir_all(parent).map_err(EnvrError::from)?;
    }
    envr_platform::fs_atomic::write_atomic(
        cur,
        abs_target_dir.to_string_lossy().to_string().as_bytes(),
    )
    .map_err(EnvrError::from)?;
    Ok(())
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

#[cfg(windows)]
pub(crate) fn maybe_promote_single_root_dir(staging: &Path) -> EnvrResult<()> {
    if ruby_executable(staging).is_file() && gem_executable(staging).is_file() {
        return Ok(());
    }

    let mut dirs: Vec<PathBuf> = Vec::new();
    let mut has_other = false;
    for e in fs::read_dir(staging).map_err(EnvrError::from)? {
        let e = e.map_err(EnvrError::from)?;
        let ty = e.file_type().map_err(EnvrError::from)?;
        if ty.is_dir() {
            dirs.push(e.path());
        } else {
            has_other = true;
        }
    }
    if has_other || dirs.len() != 1 {
        return Ok(());
    }

    let inner = dirs.pop().expect("len=1");
    envr_platform::install_layout::hoist_directory_children(&inner, staging)?;
    fs::remove_dir_all(&inner).map_err(EnvrError::from)?;
    Ok(())
}

#[cfg(windows)]
pub(crate) fn validate_ruby_installation(home: &Path) -> EnvrResult<()> {
    if !ruby_installation_valid(home) {
        return Err(EnvrError::Validation(
            "ruby install did not produce a valid runtime layout".into(),
        ));
    }
    if !bundle_executable_candidates(home)
        .iter()
        .any(|p| p.is_file())
    {
        return Err(EnvrError::Validation(
            "ruby install missing bundle executable in runtime bin directory".into(),
        ));
    }
    let ruby = ruby_executable(home);
    let out = Command::new(&ruby).arg("--version").output().map_err(|e| {
        EnvrError::with_source(ErrorCode::Runtime, "ruby --version failed to start", e)
    })?;
    if !out.status.success() {
        return Err(EnvrError::Runtime(format!(
            "ruby --version failed: {}",
            String::from_utf8_lossy(&out.stderr)
        )));
    }
    Ok(())
}

#[cfg(windows)]
pub(crate) fn extract_7z_with_bsdtar(archive: &Path, dest: &Path) -> EnvrResult<()> {
    fs::create_dir_all(dest).map_err(EnvrError::from)?;
    let bsdtar = "bsdtar";
    let status = Command::new(bsdtar)
        .args(["-xf"])
        .arg(archive)
        .args(["-C"])
        .arg(dest)
        .status()
        .map_err(|e| {
            EnvrError::with_source(ErrorCode::Runtime, "failed to start bsdtar for ruby 7z", e)
        })?;
    if !status.success() {
        return Err(EnvrError::Runtime(format!(
            "bsdtar failed extracting ruby archive {}",
            archive.display()
        )));
    }
    Ok(())
}

pub struct RubyManager {
    pub paths: RubyPaths,
    releases_url: String,
    #[cfg(windows)]
    rubyinstaller_downloads_url: String,
    client: reqwest::blocking::Client,
}

impl RubyManager {
    pub fn try_new(runtime_root: PathBuf, releases_url: String) -> EnvrResult<Self> {
        Ok(Self {
            paths: RubyPaths::new(runtime_root),
            releases_url,
            #[cfg(windows)]
            rubyinstaller_downloads_url: "https://rubyinstaller.org/downloads/".to_string(),
            client: blocking_http_client()?,
        })
    }

    pub fn load_releases(&self) -> EnvrResult<Vec<RubyRelease>> {
        let html = fetch_release_page(&self.client, &self.releases_url)?;
        parse_ruby_releases(&html)
    }

    #[cfg(windows)]
    pub fn load_installer_releases(&self) -> EnvrResult<Vec<RubyRelease>> {
        let html = fetch_release_page(&self.client, &self.rubyinstaller_downloads_url)?;
        let artifacts = parse_rubyinstaller_7z_artifacts(&html)?;
        let arch = host_rubyinstaller_arch()?;
        let mut seen = HashSet::new();
        let mut out: Vec<RubyRelease> = Vec::new();
        for a in artifacts.into_iter() {
            if a.arch != arch {
                continue;
            }
            if seen.insert(a.ruby_version.clone()) {
                out.push(RubyRelease {
                    version: a.ruby_version,
                });
            }
        }
        out.sort_by(|a, b| crate::index::cmp_semver(&a.version, &b.version));
        Ok(out)
    }

    pub fn resolve_spec(&self, spec: &str) -> EnvrResult<RuntimeVersion> {
        #[cfg(windows)]
        {
            let releases = self.load_installer_releases()?;
            Ok(RuntimeVersion(resolve_ruby_version(&releases, spec)?))
        }
        #[cfg(not(windows))]
        {
            let releases = self.load_releases()?;
            Ok(RuntimeVersion(resolve_ruby_version(&releases, spec)?))
        }
    }

    #[cfg(windows)]
    fn install_resolved_version(
        &self,
        version: &RuntimeVersion,
        progress_downloaded: Option<&Arc<AtomicU64>>,
        progress_total: Option<&Arc<AtomicU64>>,
        cancel: Option<&Arc<AtomicBool>>,
    ) -> EnvrResult<RuntimeVersion> {
        let downloads_html = fetch_release_page(&self.client, &self.rubyinstaller_downloads_url)?;
        let artifacts = parse_rubyinstaller_7z_artifacts(&downloads_html)?;
        let artifact = pick_rubyinstaller_artifact(&artifacts, &version.0)?;
        let file_name =
            artifact.url.rsplit('/').next().ok_or_else(|| {
                EnvrError::Validation("rubyinstaller url missing filename".into())
            })?;
        let cache_file = self.paths.cache_dir().join(&version.0).join(file_name);
        let final_dir = self.paths.version_dir(&version.0);
        execute_install_pipeline(
            cancel,
            || fs::create_dir_all(self.paths.cache_dir()).map_err(EnvrError::from),
            || {
                download_url_to_path_resumable(
                    &self.client,
                    &artifact.url,
                    &cache_file,
                    progress_downloaded,
                    progress_total,
                    cancel,
                )
            },
            || Ok(()),
            || {
                use envr_platform::install_layout;
                install_layout::ensure_final_parent(&final_dir)?;
                let staging_final = install_layout::sibling_staging_path(&final_dir)?;
                install_layout::remove_if_exists(&staging_final)?;
                fs::create_dir_all(&staging_final).map_err(EnvrError::from)?;

                if file_name.ends_with(".7z") {
                    extract_7z_with_bsdtar(&cache_file, &staging_final)?;
                } else {
                    extract::extract_archive(&cache_file, &staging_final)?;
                }

                maybe_promote_single_root_dir(&staging_final)?;
                if let Err(e) = validate_ruby_installation(&staging_final) {
                    let _ = fs::remove_dir_all(&staging_final);
                    return Err(e);
                }
                install_layout::commit_staging_dir(&staging_final, &final_dir)
            },
            || {
                self.set_current(version)?;
                Ok(RuntimeVersion(version.0.clone()))
            },
        )
    }

    pub fn set_current(&self, version: &RuntimeVersion) -> EnvrResult<()> {
        let dir = self.paths.version_dir(&version.0);
        if !ruby_installation_valid(&dir) {
            return Err(EnvrError::Validation(format!(
                "ruby {} is not installed",
                version.0
            )));
        }
        let abs = fs::canonicalize(&dir).map_err(EnvrError::from)?;
        let cur = self.paths.current_link();
        match ensure_link(LinkType::Soft, &abs, &cur) {
            Ok(()) => Ok(()),
            Err(EnvrError::Io(e)) if e.raw_os_error() == Some(1314) => {
                set_current_pointer_file(&cur, &abs)?;
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    pub fn uninstall(&self, version: &RuntimeVersion) -> EnvrResult<()> {
        let current = read_current(&self.paths)?;
        let dir = self.paths.version_dir(&version.0);
        if fs::symlink_metadata(&dir).is_err() {
            return Err(EnvrError::Validation(format!(
                "ruby {} is not installed",
                version.0
            )));
        }
        fs::remove_dir_all(&dir).map_err(EnvrError::from)?;
        if current.as_ref().is_some_and(|v| v.0 == version.0) {
            remove_path_if_exists(&self.paths.current_link());
        }
        Ok(())
    }
}

impl SpecDrivenInstaller for RubyManager {
    fn install_from_spec(&self, request: &InstallRequest) -> EnvrResult<RuntimeVersion> {
        #[cfg(not(windows))]
        {
            let _ = request;
            Err(EnvrError::Platform(
                "ruby install is currently implemented only for Windows RubyInstaller archives"
                    .into(),
            ))
        }
        #[cfg(windows)]
        {
            let version = self.resolve_spec(&request.spec.0)?;
            let (downloaded, total, cancel) = install_progress_handles(request);
            self.install_resolved_version(&version, downloaded, total, cancel)
        }
    }
}
