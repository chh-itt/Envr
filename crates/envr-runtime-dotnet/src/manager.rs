use crate::index::{
    DotnetSdkRelease, blocking_http_client, load_sdk_releases, pick_install_file,
    resolve_dotnet_version,
};
use envr_domain::runtime::{InstallRequest, RuntimeVersion};
use envr_download::extract;
use envr_error::{EnvrError, EnvrResult};
use envr_platform::links::{LinkType, ensure_link};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

#[derive(Debug, Clone)]
pub struct DotnetPaths {
    runtime_root: PathBuf,
}

impl DotnetPaths {
    pub fn new(runtime_root: PathBuf) -> Self {
        Self { runtime_root }
    }

    pub fn dotnet_home(&self) -> PathBuf {
        self.runtime_root.join("runtimes").join("dotnet")
    }

    pub fn versions_dir(&self) -> PathBuf {
        self.dotnet_home().join("versions")
    }

    pub fn current_link(&self) -> PathBuf {
        self.dotnet_home().join("current")
    }

    pub fn cache_dir(&self) -> PathBuf {
        self.runtime_root.join("cache").join("dotnet")
    }

    pub fn version_dir(&self, version_label: &str) -> PathBuf {
        self.versions_dir().join(version_label)
    }
}

fn dotnet_executable(home: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        home.join("dotnet.exe")
    }
    #[cfg(not(windows))]
    {
        home.join("dotnet")
    }
}

pub fn dotnet_installation_valid(home: &Path) -> bool {
    let exe = dotnet_executable(home);
    if !exe.is_file() {
        return false;
    }
    let sdk = home.join("sdk");
    if !sdk.is_dir() {
        return false;
    }
    match fs::read_dir(&sdk) {
        Ok(mut it) => it.next().is_some(),
        Err(_) => false,
    }
}

pub fn list_installed_versions(paths: &DotnetPaths) -> EnvrResult<Vec<RuntimeVersion>> {
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
        if dotnet_installation_valid(&p) {
            out.push(RuntimeVersion(e.file_name().to_string_lossy().into_owned()));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

pub fn read_current(paths: &DotnetPaths) -> EnvrResult<Option<RuntimeVersion>> {
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
            .ok_or_else(|| EnvrError::Runtime("invalid dotnet current link".into()))?
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
        .ok_or_else(|| EnvrError::Runtime("invalid dotnet current pointer".into()))?
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

fn validate_dotnet_installation(home: &Path) -> EnvrResult<()> {
    if !dotnet_installation_valid(home) {
        return Err(EnvrError::Validation(
            "dotnet install did not produce a valid SDK layout".into(),
        ));
    }
    let exe = dotnet_executable(home);
    let out = std::process::Command::new(&exe)
        .env("DOTNET_ROOT", home)
        .env("DOTNET_MULTILEVEL_LOOKUP", "0")
        .arg("--version")
        .output()
        .map_err(|e| {
            EnvrError::Runtime(envr_platform::process::classify_spawn_failure_message(
                Some(envr_domain::runtime::RuntimeKind::Dotnet),
                "dotnet --version",
                &e,
            ))
        })?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr).to_string();
        if let Some(diag) = envr_platform::process::classify_exit_failure_message(
            Some(envr_domain::runtime::RuntimeKind::Dotnet),
            "dotnet --version",
            out.status,
            &stderr,
        ) {
            return Err(EnvrError::Runtime(diag));
        }
        return Err(EnvrError::Runtime(format!(
            "dotnet --version failed: {}",
            stderr
        )));
    }
    Ok(())
}

pub struct DotnetManager {
    pub paths: DotnetPaths,
    releases_index_url: String,
    client: reqwest::blocking::Client,
}

impl DotnetManager {
    pub fn try_new(runtime_root: PathBuf, releases_index_url: String) -> EnvrResult<Self> {
        Ok(Self {
            paths: DotnetPaths::new(runtime_root),
            releases_index_url,
            client: blocking_http_client()?,
        })
    }

    pub fn load_releases(&self) -> EnvrResult<Vec<DotnetSdkRelease>> {
        load_sdk_releases(&self.client, &self.releases_index_url)
    }

    pub fn resolve_spec(&self, spec: &str) -> EnvrResult<RuntimeVersion> {
        let releases = self.load_releases()?;
        let v = resolve_dotnet_version(&releases, spec)?;
        Ok(RuntimeVersion(v))
    }

    pub fn install_from_spec(&self, request: &InstallRequest) -> EnvrResult<RuntimeVersion> {
        let releases = self.load_releases()?;
        let version = resolve_dotnet_version(&releases, &request.spec.0)?;
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
        releases: &[DotnetSdkRelease],
        version: &RuntimeVersion,
        progress_downloaded: Option<&Arc<AtomicU64>>,
        progress_total: Option<&Arc<AtomicU64>>,
        cancel: Option<&Arc<AtomicBool>>,
    ) -> EnvrResult<RuntimeVersion> {
        let release = releases
            .iter()
            .find(|r| r.version == version.0)
            .ok_or_else(|| EnvrError::Validation(format!("dotnet sdk not found: {}", version.0)))?;
        let file = pick_install_file(&release.files, &version.0)?;
        let cache_file = self.paths.cache_dir().join(&version.0).join(&file.name);
        download_to_path(
            &self.client,
            &file.url,
            &cache_file,
            progress_downloaded,
            progress_total,
            cancel,
        )?;

        let final_dir = self.paths.version_dir(&version.0);
        use envr_platform::install_layout;
        install_layout::ensure_final_parent(&final_dir)?;
        let staging = tempfile::tempdir_in(self.paths.cache_dir().join(&version.0))
            .map_err(EnvrError::from)?;
        extract::extract_archive(&cache_file, staging.path())?;
        let staging_final = install_layout::sibling_staging_path(&final_dir)?;
        install_layout::remove_if_exists(&staging_final)?;
        install_layout::hoist_directory_children(staging.path(), &staging_final)?;
        let install_root = fs::canonicalize(&staging_final).map_err(EnvrError::from)?;
        if let Err(e) = validate_dotnet_installation(&install_root) {
            let _ = fs::remove_dir_all(&staging_final);
            return Err(e);
        }
        install_layout::commit_staging_dir(&staging_final, &final_dir)?;
        self.set_current(version)?;
        Ok(RuntimeVersion(version.0.clone()))
    }

    pub fn set_current(&self, version: &RuntimeVersion) -> EnvrResult<()> {
        let dir = self.paths.version_dir(&version.0);
        if !dotnet_installation_valid(&dir) {
            return Err(EnvrError::Validation(format!(
                "dotnet sdk {} is not installed",
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installation_valid_requires_exe_and_sdk_dir() {
        let tmp = tempfile::tempdir().expect("tmp");
        let home = tmp.path().join("sdk-home");
        fs::create_dir_all(home.join("sdk").join("8.0.100")).expect("sdk dir");
        #[cfg(windows)]
        fs::write(home.join("dotnet.exe"), []).expect("exe");
        #[cfg(not(windows))]
        fs::write(home.join("dotnet"), []).expect("exe");
        assert!(dotnet_installation_valid(&home));
    }

    #[test]
    fn read_current_supports_pointer_file() {
        let tmp = tempfile::tempdir().expect("tmp");
        let root = tmp.path().to_path_buf();
        let paths = DotnetPaths::new(root.clone());
        let vhome = paths.version_dir("8.0.100");
        fs::create_dir_all(&vhome).expect("vhome");
        fs::create_dir_all(paths.dotnet_home()).expect("dotnet home");
        fs::write(paths.current_link(), vhome.display().to_string()).expect("current pointer");

        let got = read_current(&paths).expect("read").expect("some");
        assert_eq!(got.0, "8.0.100");
    }

    #[test]
    fn list_installed_filters_invalid_dirs() {
        let tmp = tempfile::tempdir().expect("tmp");
        let paths = DotnetPaths::new(tmp.path().to_path_buf());
        let good = paths.version_dir("8.0.100");
        let bad = paths.version_dir("broken");

        fs::create_dir_all(good.join("sdk").join("8.0.100")).expect("good sdk");
        #[cfg(windows)]
        fs::write(good.join("dotnet.exe"), []).expect("exe");
        #[cfg(not(windows))]
        fs::write(good.join("dotnet"), []).expect("exe");
        fs::create_dir_all(&bad).expect("bad dir");

        let listed = list_installed_versions(&paths).expect("list");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].0, "8.0.100");
    }
}
