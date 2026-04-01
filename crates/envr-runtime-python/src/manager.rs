//! Install / uninstall / `current` for CPython under the envr data root.

use crate::index::{
    PythonIndex, blocking_http_client, load_python_index, pick_install_artifact,
    release_id_for_version_label, resolve_python_version,
};
use envr_domain::runtime::{RuntimeVersion, VersionSpec};
use envr_download::{checksum, extract};
use envr_error::{EnvrError, EnvrResult};
use envr_platform::links::{LinkType, ensure_link};
use std::{
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

const GET_PIP_URL: &str = "https://bootstrap.pypa.io/get-pip.py";

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
    let mut dirs: Vec<PathBuf> = fs::read_dir(staging)
        .map_err(EnvrError::from)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .map(|e| e.path())
        .collect();
    dirs.sort();
    if dirs.len() != 1 {
        return Err(EnvrError::Validation(format!(
            "expected one source directory under {}, found {}",
            staging.display(),
            dirs.len()
        )));
    }
    Ok(dirs.into_iter().next().expect("len checked"))
}

fn fix_windows_embed_pth(home: &Path) -> EnvrResult<()> {
    for e in fs::read_dir(home).map_err(EnvrError::from)? {
        let e = e.map_err(EnvrError::from)?;
        let p = e.path();
        if p.extension() == Some(OsStr::new("_pth")) && p.is_file() {
            let mut s = fs::read_to_string(&p).map_err(EnvrError::from)?;
            if !s.contains("import site") {
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

fn bootstrap_pip_windows(client: &reqwest::blocking::Client, home: &Path) -> EnvrResult<()> {
    let py = python_executable(home)
        .ok_or_else(|| EnvrError::Runtime("python.exe missing after embeddable install".into()))?;
    let tmp = home.join(".envr-get-pip.py");
    download_to_path(client, GET_PIP_URL, &tmp)?;
    let st = Command::new(&py)
        .arg(&tmp)
        .args(["--no-warn-script-location"])
        .current_dir(home)
        .status()
        .map_err(|e| EnvrError::Runtime(format!("get-pip: {e}")))?;
    let _ = fs::remove_file(&tmp);
    if !st.success() {
        return Err(EnvrError::Runtime("get-pip.py failed".into()));
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
    let py = python_executable(home).ok_or_else(|| {
        EnvrError::Runtime("installation layout missing python executable".into())
    })?;
    let st = Command::new(&py)
        .args(["-m", "pip", "--version"])
        .status()
        .map_err(|e| EnvrError::Runtime(format!("pip check: {e}")))?;
    if !st.success() {
        return Err(EnvrError::Runtime(
            "`python -m pip --version` failed after install".into(),
        ));
    }
    Ok(())
}

pub fn python_executable(home: &Path) -> Option<PathBuf> {
    #[cfg(windows)]
    {
        let p = home.join("python.exe");
        if p.is_file() {
            Some(p)
        } else {
            None
        }
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

    pub fn install_from_spec(&self, spec: &VersionSpec) -> EnvrResult<RuntimeVersion> {
        let index = self.load_index()?;
        let os = std::env::consts::OS;
        let arch = std::env::consts::ARCH;
        let label = resolve_python_version(&index, os, arch, &spec.0)?;
        self.install_resolved_version(&index, &RuntimeVersion(label))
    }

    pub fn install_resolved_version(
        &self,
        index: &PythonIndex,
        version: &RuntimeVersion,
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
        download_to_path(&self.client, &artifact.url, &cache_file)?;
        verify_sha256_if_present(&cache_file, &artifact.sha256_sum)?;

        let final_dir = self.paths.version_dir(&version.0);
        if final_dir.exists() {
            fs::remove_dir_all(&final_dir).map_err(EnvrError::from)?;
        }

        if os == "windows" {
            fs::create_dir_all(&final_dir).map_err(EnvrError::from)?;
            extract::extract_archive(&cache_file, &final_dir)?;
            fix_windows_embed_pth(&final_dir)?;
            bootstrap_pip_windows(&self.client, &final_dir)?;
        } else {
            fs::create_dir_all(self.paths.cache_dir().join(&version.0)).map_err(EnvrError::from)?;
            let staging = tempfile::tempdir_in(self.paths.cache_dir().join(&version.0))
                .map_err(EnvrError::from)?;
            extract::extract_archive(&cache_file, staging.path())?;
            let src_root = lone_child_directory(staging.path())?;
            fs::create_dir_all(self.paths.versions_dir()).map_err(EnvrError::from)?;
            let abs_prefix = fs::canonicalize(self.paths.versions_dir())
                .map_err(EnvrError::from)?
                .join(&version.0);
            build_cpython_unix(&src_root, &abs_prefix)?;
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
            let _ = fs::remove_file(self.paths.current_link());
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
