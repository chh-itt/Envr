//! Install / uninstall / `current` for CPython under the envr data root.

use crate::index::{
    PythonIndex, blocking_http_client, load_python_index, pick_install_artifact,
    release_id_for_version_label, resolve_python_version,
};
use envr_config::settings::{
    pip_registry_urls_for_bootstrap, python_download_url_candidates, python_get_pip_url,
    settings_path_from_platform,
};
use envr_domain::runtime::{RuntimeVersion, VersionSpec};
use envr_download::{checksum, extract};
use envr_error::{EnvrError, EnvrResult};
use envr_platform::links::{LinkType, ensure_link};
use std::{
    ffi::OsStr,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Duration,
};

#[derive(Debug, Clone)]
pub struct PythonPaths {
    runtime_root: PathBuf,
}

impl PythonPaths {
    pub fn new(runtime_root: PathBuf) -> Self {
        Self { runtime_root }
    }

    pub fn python_home(&self) -> PathBuf {
        self.runtime_root.join("runtimes").join("python")
    }

    pub fn versions_dir(&self) -> PathBuf {
        self.python_home().join("versions")
    }

    pub fn current_link(&self) -> PathBuf {
        self.python_home().join("current")
    }

    pub fn cache_dir(&self) -> PathBuf {
        self.runtime_root.join("cache").join("python")
    }

    pub fn version_dir(&self, version_label: &str) -> PathBuf {
        self.versions_dir().join(version_label)
    }
}

fn download_to_path(
    client: &reqwest::blocking::Client,
    url: &str,
    path: &Path,
    progress_downloaded: Option<&Arc<AtomicU64>>,
    progress_total: Option<&Arc<AtomicU64>>,
    cancel: Option<&Arc<AtomicBool>>,
) -> EnvrResult<()> {
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
    if let Some(total) = progress_total {
        total.store(response.content_length().unwrap_or(0), Ordering::Relaxed);
    }
    if let Some(downloaded) = progress_downloaded {
        downloaded.store(0, Ordering::Relaxed);
    }
    let mut buf = vec![0_u8; 64 * 1024];
    let mut wrote = 0_u64;
    loop {
        if cancel.is_some_and(|c| c.load(Ordering::Relaxed)) {
            return Err(EnvrError::Runtime("download cancelled".into()));
        }
        let n = response
            .read(&mut buf)
            .map_err(|e| EnvrError::Download(e.to_string()))?;
        if n == 0 {
            break;
        }
        f.write_all(&buf[..n]).map_err(EnvrError::from)?;
        wrote = wrote.saturating_add(n as u64);
        if let Some(downloaded) = progress_downloaded {
            downloaded.store(wrote, Ordering::Relaxed);
        }
    }
    Ok(())
}

fn verify_sha256_if_present(path: &Path, hex: &Option<String>) -> EnvrResult<()> {
    if let Some(h) = hex {
        let t = h.trim();
        if t.len() >= 64 {
            checksum::verify_sha256_hex(path, t)?;
        }
    }
    Ok(())
}

/// Embeddable layout: many files in one directory (no single root to promote).
fn lone_child_directory(staging: &Path) -> EnvrResult<PathBuf> {
    let mut found: Option<PathBuf> = None;
    for e in fs::read_dir(staging).map_err(EnvrError::from)? {
        let e = e.map_err(EnvrError::from)?;
        if !e.path().is_dir() {
            continue;
        }
        if found.is_some() {
            return Err(EnvrError::Validation(format!(
                "expected one source directory under {}, found more than one",
                staging.display()
            )));
        }
        found = Some(e.path());
    }
    found.ok_or_else(|| {
        EnvrError::Validation(format!(
            "expected one source directory under {}, found 0",
            staging.display()
        ))
    })
}

fn fix_windows_embed_pth(home: &Path) -> EnvrResult<()> {
    for e in fs::read_dir(home).map_err(EnvrError::from)? {
        let e = e.map_err(EnvrError::from)?;
        let p = e.path();
        if p.extension() == Some(OsStr::new("_pth")) && p.is_file() {
            let mut s = fs::read_to_string(&p).map_err(EnvrError::from)?;
            let has_active_import_site = s.lines().any(|line| {
                let t = line.trim();
                !t.starts_with('#') && t == "import site"
            });
            if !has_active_import_site {
                if !s.ends_with('\n') {
                    s.push('\n');
                }
                s.push_str("import site\n");
                fs::write(&p, s).map_err(EnvrError::from)?;
            }
        }
    }
    Ok(())
}

fn load_settings_snapshot() -> Option<envr_config::settings::Settings> {
    (|| {
        let platform = envr_platform::paths::current_platform_paths().ok()?;
        let path = settings_path_from_platform(&platform);
        envr_config::settings::Settings::load_or_default_from(&path).ok()
    })()
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

fn load_python_bootstrap_from_settings() -> (Vec<String>, Vec<String>) {
    if let Some(st) = load_settings_snapshot() {
        let primary = python_get_pip_url(&st).to_string();
        let mut urls = vec![primary.clone()];
        // Always keep official as fallback: domestic mirrors can lag behind and break on new Python versions.
        let official = envr_config::settings::GET_PIP_URL_OFFICIAL.to_string();
        if primary != official {
            urls.push(official);
        }
        (
            urls,
            pip_registry_urls_for_bootstrap(&st)
                .into_iter()
                .map(ToString::to_string)
                .collect(),
        )
    } else {
        (
            vec![envr_config::settings::GET_PIP_URL_OFFICIAL.to_string()],
            vec![envr_config::settings::PIP_INDEX_OFFICIAL.to_string()],
        )
    }
}

fn download_with_fallbacks(
    client: &reqwest::blocking::Client,
    urls: &[String],
    path: &Path,
    progress_downloaded: Option<&Arc<AtomicU64>>,
    progress_total: Option<&Arc<AtomicU64>>,
    cancel: Option<&Arc<AtomicBool>>,
) -> EnvrResult<()> {
    let mut last_err: Option<EnvrError> = None;
    for url in urls {
        match download_to_path(
            client,
            url,
            path,
            progress_downloaded,
            progress_total,
            cancel,
        ) {
            Ok(()) => return Ok(()),
            Err(e) => last_err = Some(e),
        }
    }
    Err(last_err.unwrap_or_else(|| EnvrError::Download("no download urls available".into())))
}

fn ensure_cached_get_pip(
    client: &reqwest::blocking::Client,
    paths: &PythonPaths,
    source_urls: &[String],
    progress_downloaded: Option<&Arc<AtomicU64>>,
    progress_total: Option<&Arc<AtomicU64>>,
    cancel: Option<&Arc<AtomicBool>>,
) -> EnvrResult<PathBuf> {
    let cache_file = paths.cache_dir().join("bootstrap").join("get-pip.py");
    let stale = fs::metadata(&cache_file)
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.elapsed().ok())
        .is_none_or(|age| age > Duration::from_secs(24 * 3600));
    if cache_file.is_file() && !stale {
        return Ok(cache_file);
    }
    if let Some(parent) = cache_file.parent() {
        fs::create_dir_all(parent).map_err(EnvrError::from)?;
    }
    let pid = std::process::id();
    let tmp = cache_file.with_extension(format!("tmp.{pid}"));
    download_with_fallbacks(
        client,
        source_urls,
        &tmp,
        progress_downloaded,
        progress_total,
        cancel,
    )?;
    // Best-effort atomic replacement. Parallel writers may race; winner keeps a valid script.
    if cache_file.exists() {
        let _ = fs::remove_file(&cache_file);
    }
    fs::rename(&tmp, &cache_file).map_err(EnvrError::from)?;
    Ok(cache_file)
}

fn run_get_pip(
    py: &Path,
    script: &Path,
    home: &Path,
    pip_index_url: Option<&str>,
) -> EnvrResult<std::process::Output> {
    Command::new(py)
        .arg(script)
        .args(["--no-warn-script-location", "--disable-pip-version-check"])
        .args(
            pip_index_url
                .map(|u| ["--index-url", u])
                .unwrap_or(["", ""])
                .into_iter()
                .filter(|s| !s.is_empty()),
        )
        .current_dir(home)
        .output()
        .map_err(|e| EnvrError::Runtime(format!("get-pip: {e}")))
}

fn bootstrap_pip_windows(
    client: &reqwest::blocking::Client,
    paths: &PythonPaths,
    home: &Path,
    progress_downloaded: Option<&Arc<AtomicU64>>,
    progress_total: Option<&Arc<AtomicU64>>,
    cancel: Option<&Arc<AtomicBool>>,
) -> EnvrResult<()> {
    let py = python_executable(home)
        .ok_or_else(|| EnvrError::Runtime("python.exe missing after embeddable install".into()))?;
    let (get_pip_urls, pip_index_urls) = load_python_bootstrap_from_settings();
    let script = ensure_cached_get_pip(
        client,
        paths,
        &get_pip_urls,
        progress_downloaded,
        progress_total,
        cancel,
    )?;
    let mut used_index = pip_index_urls.first().map(String::as_str);
    let mut out = run_get_pip(&py, &script, home, used_index)?;
    if !out.status.success() {
        let stderr_lc = String::from_utf8_lossy(&out.stderr).to_ascii_lowercase();
        if stderr_lc.contains("no module named 'distutils'")
            || stderr_lc.contains("no module named distutils")
        {
            // Cached mirror script can be stale for newer Python (e.g. 3.14+). Retry once with fresh official script.
            let _ = fs::remove_file(&script);
            let script = ensure_cached_get_pip(
                client,
                paths,
                &[envr_config::settings::GET_PIP_URL_OFFICIAL.to_string()],
                progress_downloaded,
                progress_total,
                cancel,
            )?;
            out = run_get_pip(&py, &script, home, used_index)?;
        }
    }
    if !out.status.success() {
        // Retry with pip index fallbacks first (domestic -> fallback domestic -> official).
        for idx in pip_index_urls.iter().skip(1) {
            used_index = Some(idx.as_str());
            out = run_get_pip(&py, &script, home, used_index)?;
            if out.status.success() {
                break;
            }
        }
    }
    if !out.status.success() {
        if used_index != Some(envr_config::settings::PIP_INDEX_OFFICIAL) {
            // Last resort: force fresh official get-pip + official index.
            let _ = fs::remove_file(&script);
            let script = ensure_cached_get_pip(
                client,
                paths,
                &[envr_config::settings::GET_PIP_URL_OFFICIAL.to_string()],
                progress_downloaded,
                progress_total,
                cancel,
            )?;
            out = run_get_pip(
                &py,
                &script,
                home,
                Some(envr_config::settings::PIP_INDEX_OFFICIAL),
            )?;
        }
    }
    if !out.status.success() {
        let stdout = String::from_utf8_lossy(&out.stdout);
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(EnvrError::Runtime(format!(
            "get-pip.py failed (exit={}); stdout={}; stderr={}",
            out.status,
            stdout.trim(),
            stderr.trim()
        )));
    }
    Ok(())
}

fn build_cpython_unix(src_root: &Path, install_prefix: &Path) -> EnvrResult<()> {
    let prefix = install_prefix.to_string_lossy();
    let cfg = Command::new("./configure")
        .current_dir(src_root)
        .arg(format!("--prefix={prefix}"))
        .arg("--with-ensurepip=install")
        .status()
        .map_err(|e| {
            EnvrError::Runtime(format!(
                "failed to run ./configure (install build tools / dev packages): {e}"
            ))
        })?;
    if !cfg.success() {
        return Err(EnvrError::Runtime(
            "./configure failed — on Linux install build-essential, libssl-dev, zlib1g-dev, libffi-dev, etc."
                .into(),
        ));
    }

    let jobs = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .max(1);

    let mk = Command::new("make")
        .current_dir(src_root)
        .arg(format!("-j{jobs}"))
        .status()
        .map_err(|e| EnvrError::Runtime(format!("make failed to start: {e}")))?;
    if !mk.success() {
        return Err(EnvrError::Runtime("make failed".into()));
    }

    let ins = Command::new("make")
        .current_dir(src_root)
        .arg("install")
        .status()
        .map_err(|e| EnvrError::Runtime(format!("make install failed to start: {e}")))?;
    if !ins.success() {
        return Err(EnvrError::Runtime("make install failed".into()));
    }
    Ok(())
}

fn verify_python_and_pip(home: &Path) -> EnvrResult<()> {
    #[cfg(windows)]
    {
        // Re-ensure embeddable _pth enables site-packages before probing pip.
        fix_windows_embed_pth(home)?;
    }
    let py = python_executable(home).ok_or_else(|| {
        EnvrError::Runtime("installation layout missing python executable".into())
    })?;
    let first = Command::new(&py)
        .args(["-m", "pip", "--version"])
        .output()
        .map_err(|e| EnvrError::Runtime(format!("pip check: {e}")))?;
    if first.status.success() {
        return Ok(());
    }

    // Some Python layouts can still recover pip via stdlib ensurepip.
    let ensure = Command::new(&py)
        .args(["-m", "ensurepip", "--upgrade"])
        .output()
        .map_err(|e| EnvrError::Runtime(format!("ensurepip check: {e}")))?;
    if ensure.status.success() {
        let second = Command::new(&py)
            .args(["-m", "pip", "--version"])
            .output()
            .map_err(|e| EnvrError::Runtime(format!("pip re-check: {e}")))?;
        if second.status.success() {
            return Ok(());
        }
    }

    let first_out = String::from_utf8_lossy(&first.stdout);
    let first_err = String::from_utf8_lossy(&first.stderr);
    let ensure_out = String::from_utf8_lossy(&ensure.stdout);
    let ensure_err = String::from_utf8_lossy(&ensure.stderr);
    Err(EnvrError::Runtime(format!(
        "`python -m pip --version` failed after install; pip_stdout={}; pip_stderr={}; ensurepip_exit={}; ensurepip_stdout={}; ensurepip_stderr={}",
        first_out.trim(),
        first_err.trim(),
        ensure.status,
        ensure_out.trim(),
        ensure_err.trim(),
    )))
}

pub fn python_executable(home: &Path) -> Option<PathBuf> {
    #[cfg(windows)]
    {
        let p = home.join("python.exe");
        if p.is_file() { Some(p) } else { None }
    }
    #[cfg(not(windows))]
    {
        let bin = home.join("bin");
        for name in ["python3", "python"] {
            let p = bin.join(name);
            if p.is_file() {
                return Some(p);
            }
        }
        let entries = fs::read_dir(&bin).ok()?;
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with("python3.") && e.path().is_file() {
                return Some(e.path());
            }
        }
        None
    }
}

pub fn python_installation_valid(home: &Path) -> bool {
    python_executable(home).is_some()
}

pub fn list_installed_versions(paths: &PythonPaths) -> EnvrResult<Vec<RuntimeVersion>> {
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
        if python_installation_valid(&p) {
            out.push(RuntimeVersion(e.file_name().to_string_lossy().into_owned()));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

pub fn read_current(paths: &PythonPaths) -> EnvrResult<Option<RuntimeVersion>> {
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
        .ok_or_else(|| EnvrError::Runtime("invalid python current link".into()))?
        .to_string_lossy()
        .into_owned();
    Ok(Some(RuntimeVersion(name)))
}

pub struct PythonManager {
    pub paths: PythonPaths,
    releases_url: String,
    files_url: String,
    client: reqwest::blocking::Client,
}

impl PythonManager {
    pub fn try_new(
        runtime_root: PathBuf,
        releases_url: String,
        files_url: String,
    ) -> EnvrResult<Self> {
        Ok(Self {
            paths: PythonPaths::new(runtime_root),
            releases_url,
            files_url,
            client: blocking_http_client()?,
        })
    }

    fn load_index(&self) -> EnvrResult<PythonIndex> {
        load_python_index(&self.client, &self.releases_url, &self.files_url)
    }

    pub fn install_from_spec(
        &self,
        spec: &VersionSpec,
        progress_downloaded: Option<&Arc<AtomicU64>>,
        progress_total: Option<&Arc<AtomicU64>>,
        cancel: Option<&Arc<AtomicBool>>,
    ) -> EnvrResult<RuntimeVersion> {
        let index = self.load_index()?;
        let os = std::env::consts::OS;
        let arch = std::env::consts::ARCH;
        let label = resolve_python_version(&index, os, arch, &spec.0)?;
        self.install_resolved_version(
            &index,
            &RuntimeVersion(label),
            progress_downloaded,
            progress_total,
            cancel,
        )
    }

    pub fn install_resolved_version(
        &self,
        index: &PythonIndex,
        version: &RuntimeVersion,
        progress_downloaded: Option<&Arc<AtomicU64>>,
        progress_total: Option<&Arc<AtomicU64>>,
        cancel: Option<&Arc<AtomicBool>>,
    ) -> EnvrResult<RuntimeVersion> {
        let os = std::env::consts::OS;
        let arch = std::env::consts::ARCH;
        let rid = release_id_for_version_label(index, &version.0, os, arch)?;
        let files = index
            .files_by_release
            .get(&rid)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let artifact = pick_install_artifact(files, os, arch)?;

        fs::create_dir_all(self.paths.cache_dir()).map_err(EnvrError::from)?;
        let fname = artifact.url.rsplit('/').next().unwrap_or("download.bin");
        let cache_file = self.paths.cache_dir().join(&version.0).join(fname);
        let download_urls = if let Some(st) = load_settings_snapshot() {
            python_download_url_candidates(&st, &artifact.url)
        } else {
            vec![artifact.url.clone()]
        };
        download_with_fallbacks(
            &self.client,
            &download_urls,
            &cache_file,
            progress_downloaded,
            progress_total,
            cancel,
        )?;
        verify_sha256_if_present(&cache_file, &artifact.sha256_sum)?;

        let final_dir = self.paths.version_dir(&version.0);
        use envr_platform::install_layout;
        install_layout::ensure_final_parent(&final_dir)?;

        if os == "windows" {
            fs::create_dir_all(self.paths.cache_dir().join(&version.0)).map_err(EnvrError::from)?;
            let staging = tempfile::tempdir_in(self.paths.cache_dir().join(&version.0))
                .map_err(EnvrError::from)?;
            extract::extract_archive(&cache_file, staging.path())?;
            let staging_final = install_layout::sibling_staging_path(&final_dir)?;
            install_layout::remove_if_exists(&staging_final)?;
            install_layout::hoist_directory_children(staging.path(), &staging_final)?;
            fix_windows_embed_pth(&staging_final)?;
            bootstrap_pip_windows(
                &self.client,
                &self.paths,
                &staging_final,
                progress_downloaded,
                progress_total,
                cancel,
            )?;
            let install_root = fs::canonicalize(&staging_final).map_err(EnvrError::from)?;
            if !python_installation_valid(&install_root) {
                let _ = fs::remove_dir_all(&staging_final);
                return Err(EnvrError::Validation(
                    "python install did not produce a usable prefix".into(),
                ));
            }
            if let Err(e) = verify_python_and_pip(&install_root) {
                let _ = fs::remove_dir_all(&staging_final);
                return Err(e);
            }
            install_layout::commit_staging_dir(&staging_final, &final_dir)?;
        } else {
            fs::create_dir_all(self.paths.cache_dir().join(&version.0)).map_err(EnvrError::from)?;
            let staging = tempfile::tempdir_in(self.paths.cache_dir().join(&version.0))
                .map_err(EnvrError::from)?;
            extract::extract_archive(&cache_file, staging.path())?;
            let src_root = lone_child_directory(staging.path())?;
            let staging_final = install_layout::sibling_staging_path(&final_dir)?;
            install_layout::remove_if_exists(&staging_final)?;
            if let Err(e) = build_cpython_unix(&src_root, &staging_final) {
                let _ = fs::remove_dir_all(&staging_final);
                return Err(e);
            }
            let install_root = fs::canonicalize(&staging_final).map_err(EnvrError::from)?;
            if !python_installation_valid(&install_root) {
                let _ = fs::remove_dir_all(&staging_final);
                return Err(EnvrError::Validation(
                    "python install did not produce a usable prefix".into(),
                ));
            }
            if let Err(e) = verify_python_and_pip(&install_root) {
                let _ = fs::remove_dir_all(&staging_final);
                return Err(e);
            }
            install_layout::commit_staging_dir(&staging_final, &final_dir)?;
        }

        let install_root = fs::canonicalize(&final_dir).map_err(EnvrError::from)?;
        if !python_installation_valid(&install_root) {
            return Err(EnvrError::Validation(
                "python install did not produce a usable prefix".into(),
            ));
        }
        verify_python_and_pip(&install_root)?;
        self.set_current(version)?;
        Ok(RuntimeVersion(version.0.clone()))
    }

    pub fn set_current(&self, version: &RuntimeVersion) -> EnvrResult<()> {
        let dir = self.paths.version_dir(&version.0);
        if !python_installation_valid(&dir) {
            return Err(EnvrError::Validation(format!(
                "python {} is not installed",
                version.0
            )));
        }
        let abs = fs::canonicalize(&dir).map_err(EnvrError::from)?;
        ensure_link(LinkType::Soft, &abs, self.paths.current_link())?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn lone_child_finds_python_dir() {
        let tmp = tempfile::tempdir().expect("tmp");
        fs::create_dir(tmp.path().join("Python-9.9.9")).expect("d");
        let p = lone_child_directory(tmp.path()).expect("ok");
        assert!(p.ends_with("Python-9.9.9"));
    }

    #[test]
    fn fix_pth_appends_import_site() {
        let tmp = tempfile::tempdir().expect("tmp");
        let pth = tmp.path().join("python399._pth");
        let mut f = fs::File::create(&pth).expect("c");
        writeln!(f, "python399.zip").expect("w");
        fix_windows_embed_pth(tmp.path()).expect("fix");
        let s = fs::read_to_string(&pth).expect("r");
        assert!(s.contains("import site"));
    }
}
