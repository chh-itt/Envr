use crate::index::{
    DEFAULT_GITHUB_TAGS_API, ErlangRelease, blocking_http_client, fetch_all_tags,
    resolve_erlang_version, tags_to_releases,
};
use envr_domain::runtime::{InstallRequest, RuntimeVersion};
use envr_download::blocking::download_url_to_path_resumable;
use envr_download::extract;
use envr_error::{EnvrError, EnvrResult};
use envr_platform::links::{LinkType, ensure_link};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64};

#[derive(Debug, Clone)]
pub struct ErlangPaths {
    runtime_root: PathBuf,
}

impl ErlangPaths {
    pub fn new(runtime_root: PathBuf) -> Self {
        Self { runtime_root }
    }

    pub fn erlang_home(&self) -> PathBuf {
        self.runtime_root.join("runtimes").join("erlang")
    }

    pub fn versions_dir(&self) -> PathBuf {
        self.erlang_home().join("versions")
    }

    pub fn current_link(&self) -> PathBuf {
        self.erlang_home().join("current")
    }

    pub fn cache_dir(&self) -> PathBuf {
        self.runtime_root.join("cache").join("erlang")
    }

    pub fn version_dir(&self, version_label: &str) -> PathBuf {
        self.versions_dir().join(version_label)
    }
}

fn erl_executable(home: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        home.join("bin").join("erl.exe")
    }
    #[cfg(not(windows))]
    {
        home.join("bin").join("erl")
    }
}

fn erlc_executable(home: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        home.join("bin").join("erlc.exe")
    }
    #[cfg(not(windows))]
    {
        home.join("bin").join("erlc")
    }
}

pub fn erlang_installation_valid(home: &Path) -> bool {
    erl_executable(home).is_file() && erlc_executable(home).is_file()
}

pub fn list_installed_versions(paths: &ErlangPaths) -> EnvrResult<Vec<RuntimeVersion>> {
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
        if erlang_installation_valid(&p) {
            out.push(RuntimeVersion(e.file_name().to_string_lossy().into_owned()));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

pub fn read_current(paths: &ErlangPaths) -> EnvrResult<Option<RuntimeVersion>> {
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
            .ok_or_else(|| EnvrError::Runtime("invalid erlang current link".into()))?
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
        .ok_or_else(|| EnvrError::Runtime("invalid erlang current pointer".into()))?
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

fn validate_erlang_installation(home: &Path) -> EnvrResult<()> {
    if !erlang_installation_valid(home) {
        return Err(EnvrError::Validation(
            "erlang install did not produce a valid runtime layout".into(),
        ));
    }
    let exe = erl_executable(home);
    let out = Command::new(&exe)
        .arg("-noshell")
        .arg("-eval")
        .arg("halt().")
        .output()
        .map_err(|e| EnvrError::Runtime(format!("erl probe failed to start: {e}")))?;
    if !out.status.success() {
        return Err(EnvrError::Runtime(format!(
            "erl probe failed: {}",
            String::from_utf8_lossy(&out.stderr)
        )));
    }
    Ok(())
}

pub struct ErlangManager {
    pub paths: ErlangPaths,
    tags_api_url: String,
    client: reqwest::blocking::Client,
}

impl ErlangManager {
    pub fn try_new(runtime_root: PathBuf, tags_api_url: String) -> EnvrResult<Self> {
        Ok(Self {
            paths: ErlangPaths::new(runtime_root),
            tags_api_url,
            client: blocking_http_client()?,
        })
    }

    pub fn load_releases(&self) -> EnvrResult<Vec<ErlangRelease>> {
        let tags = fetch_all_tags(&self.client, &self.tags_api_url)?;
        tags_to_releases(&tags)
    }

    pub fn resolve_spec(&self, spec: &str) -> EnvrResult<RuntimeVersion> {
        let releases = self.load_releases()?;
        let version = resolve_erlang_version(&releases, spec)?;
        Ok(RuntimeVersion(version))
    }

    pub fn install_from_spec(&self, request: &InstallRequest) -> EnvrResult<RuntimeVersion> {
        let releases = self.load_releases()?;
        let version = resolve_erlang_version(&releases, &request.spec.0)?;
        self.install_resolved_version(
            &releases,
            &RuntimeVersion(version),
            request.progress_downloaded.as_ref(),
            request.progress_total.as_ref(),
            request.cancel.as_ref(),
        )
    }

    pub fn install_resolved_version(
        &self,
        releases: &[ErlangRelease],
        version: &RuntimeVersion,
        progress_downloaded: Option<&Arc<AtomicU64>>,
        progress_total: Option<&Arc<AtomicU64>>,
        cancel: Option<&Arc<AtomicBool>>,
    ) -> EnvrResult<RuntimeVersion> {
        let release = releases
            .iter()
            .find(|r| r.version == version.0)
            .ok_or_else(|| EnvrError::Validation(format!("erlang release not found: {}", version.0)))?;

        let file_name = release
            .url
            .rsplit('/')
            .next()
            .ok_or_else(|| EnvrError::Validation("erlang release url missing filename".into()))?;
        let cache_file = self.paths.cache_dir().join(&version.0).join(file_name);
        download_url_to_path_resumable(
            &self.client,
            &release.url,
            &cache_file,
            progress_downloaded,
            progress_total,
            cancel,
        )?;

        use envr_platform::install_layout;
        let final_dir = self.paths.version_dir(&version.0);
        install_layout::ensure_final_parent(&final_dir)?;
        let staging_final = install_layout::sibling_staging_path(&final_dir)?;
        install_layout::remove_if_exists(&staging_final)?;
        fs::create_dir_all(&staging_final).map_err(EnvrError::from)?;
        extract::extract_archive(&cache_file, &staging_final)?;

        if let Err(e) = validate_erlang_installation(&staging_final) {
            let _ = fs::remove_dir_all(&staging_final);
            return Err(e);
        }
        install_layout::commit_staging_dir(&staging_final, &final_dir)?;
        self.set_current(version)?;
        Ok(RuntimeVersion(version.0.clone()))
    }

    pub fn set_current(&self, version: &RuntimeVersion) -> EnvrResult<()> {
        let dir = self.paths.version_dir(&version.0);
        if !erlang_installation_valid(&dir) {
            return Err(EnvrError::Validation(format!(
                "erlang {} is not installed",
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

impl Default for ErlangManager {
    fn default() -> Self {
        Self::try_new(PathBuf::new(), DEFAULT_GITHUB_TAGS_API.to_string())
            .expect("default erlang manager")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installation_valid_requires_erl_and_erlc() {
        let tmp = tempfile::tempdir().expect("tmp");
        let home = tmp.path().join("otp");
        std::fs::create_dir_all(home.join("bin")).expect("bin");
        #[cfg(windows)]
        {
            std::fs::write(home.join("bin").join("erl.exe"), []).expect("erl");
            std::fs::write(home.join("bin").join("erlc.exe"), []).expect("erlc");
        }
        #[cfg(not(windows))]
        {
            std::fs::write(home.join("bin").join("erl"), []).expect("erl");
            std::fs::write(home.join("bin").join("erlc"), []).expect("erlc");
        }
        assert!(erlang_installation_valid(&home));
    }

    #[test]
    fn read_current_supports_pointer_file() {
        let tmp = tempfile::tempdir().expect("tmp");
        let root = tmp.path().to_path_buf();
        let paths = ErlangPaths::new(root.clone());
        let vhome = paths.version_dir("27.3.4.10");
        std::fs::create_dir_all(&vhome).expect("vhome");
        std::fs::create_dir_all(paths.erlang_home()).expect("home");
        std::fs::write(paths.current_link(), vhome.display().to_string()).expect("current");
        let got = read_current(&paths).expect("read").expect("some");
        assert_eq!(got.0, "27.3.4.10");
    }
}

