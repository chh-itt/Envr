//! Install / uninstall / `current` selection for Node.js runtimes under the envr data root.

use crate::index::{
    NodeRelease, blocking_http_client, fetch_node_index, node_version_v_prefix,
    normalize_node_version, parse_node_index, resolve_node_version,
};
use envr_domain::installer::{
    SpecDrivenInstaller, execute_install_pipeline, install_progress_handles,
};
use envr_domain::runtime::{InstallRequest, RuntimeVersion};
use envr_download::{blocking::download_url_to_path_resumable, checksum, extract};
use envr_error::{EnvrError, EnvrResult, ErrorCode};
use envr_platform::links::{LinkType, ensure_link};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    sync::atomic::{AtomicBool, AtomicU64},
};

/// Layout: `{runtime_root}/runtimes/node/versions/<semver>/...` and `current` → version dir.
#[derive(Debug, Clone)]
pub struct NodePaths {
    runtime_root: PathBuf,
}

impl NodePaths {
    pub fn new(runtime_root: PathBuf) -> Self {
        Self { runtime_root }
    }

    pub fn node_home(&self) -> PathBuf {
        self.runtime_root.join("runtimes").join("node")
    }

    pub fn versions_dir(&self) -> PathBuf {
        self.node_home().join("versions")
    }

    pub fn current_link(&self) -> PathBuf {
        self.node_home().join("current")
    }

    pub fn cache_dir(&self) -> PathBuf {
        self.runtime_root.join("cache").join("node")
    }

    pub fn version_dir(&self, version_label: &str) -> PathBuf {
        self.versions_dir().join(version_label)
    }
}

pub fn dist_root_from_index_json_url(index_json_url: &str) -> String {
    index_json_url
        .trim()
        .trim_end_matches(|c: char| c == '/' || c.is_whitespace())
        .trim_end_matches("index.json")
        .trim_end_matches(|c: char| c == '/' || c.is_whitespace())
        .to_string()
}

fn join_url_path(dist_root: &str, rel: &str) -> String {
    let root = dist_root.trim_end_matches('/');
    let rel = rel.trim_start_matches('/');
    format!("{root}/{rel}")
}

fn err_version_spec_invalid(msg: impl Into<String>) -> EnvrError {
    EnvrError::Context {
        code: ErrorCode::RuntimeVersionSpecInvalid,
        message: msg.into(),
        source: Box::new(std::io::Error::other("runtime-version-spec-invalid")),
    }
}

fn err_version_not_found(msg: impl Into<String>) -> EnvrError {
    EnvrError::Context {
        code: ErrorCode::RuntimeVersionNotFound,
        message: msg.into(),
        source: Box::new(std::io::Error::other("runtime-version-not-found")),
    }
}

/// Parse `SHASUMS256.txt` lines into `(hex, filename)`.
pub fn parse_shasums256(text: &str) -> EnvrResult<Vec<(String, String)>> {
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (hash, name) = if let Some(idx) = line.find(" *") {
            (line[..idx].trim(), line[idx + 2..].trim())
        } else {
            let mut parts = line.split_whitespace();
            let h = parts.next().ok_or_else(|| {
                err_version_spec_invalid("malformed SHASUMS256 line (missing hash)")
            })?;
            let n = parts.next().ok_or_else(|| {
                err_version_spec_invalid("malformed SHASUMS256 line (missing name)")
            })?;
            (h, n)
        };
        if hash.len() >= 64 && !name.is_empty() {
            out.push((hash.to_string(), name.to_string()));
        }
    }
    Ok(out)
}

fn preferred_suffixes(os: &str, arch: &str) -> EnvrResult<&'static [&'static str]> {
    match (os, arch) {
        ("windows", "x86_64") => Ok(&["-win-x64.zip"]),
        ("windows", "aarch64") => Ok(&["-win-arm64.zip"]),
        ("windows", "x86") => Ok(&["-win-x86.zip"]),
        ("linux", "x86_64") => Ok(&["-linux-x64.tar.xz", "-linux-x64.tar.gz"]),
        ("linux", "aarch64") => Ok(&["-linux-arm64.tar.xz", "-linux-arm64.tar.gz"]),
        ("linux", "arm") | ("linux", "armv7") => {
            Ok(&["-linux-armv7l.tar.xz", "-linux-armv7l.tar.gz"])
        }
        ("macos", "x86_64") => Ok(&["-darwin-x64.tar.gz"]),
        ("macos", "aarch64") => Ok(&["-darwin-arm64.tar.gz"]),
        _ => Err(err_version_not_found(format!(
            "unsupported host for node install: {os}-{arch}"
        ))),
    }
}

fn set_current_pointer_file(cur: &Path, abs_target_dir: &Path) -> EnvrResult<()> {
    if cur.exists() {
        if cur.is_dir() {
            // For junction/symlink this should remove only the link, not the versions dir.
            // If it removes more than desired, it's still constrained to whatever `current`
            // pointed to previously.
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

pub fn pick_node_dist_artifact(
    entries: &[(String, String)],
    os: &str,
    arch: &str,
    version_v: &str,
) -> EnvrResult<(String, String)> {
    let prefix = format!("node-{version_v}");
    let suffixes = preferred_suffixes(os, arch)?;
    for sfx in suffixes {
        let needle = format!("{prefix}{sfx}");
        if let Some((h, n)) = entries.iter().find(|(_, n)| n == &needle) {
            return Ok((h.clone(), n.clone()));
        }
    }
    Err(err_version_not_found(format!(
        "no node dist file for {version_v} on {os}-{arch} (check SHASUMS256)"
    )))
}

pub fn node_installation_valid(home: &Path) -> bool {
    #[cfg(windows)]
    {
        home.join("node.exe").is_file() || home.join("bin").join("node.exe").is_file()
    }
    #[cfg(not(windows))]
    {
        home.join("bin").join("node").is_file()
    }
}

pub fn list_installed_versions(paths: &NodePaths) -> EnvrResult<Vec<RuntimeVersion>> {
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
        if node_installation_valid(&p) {
            out.push(RuntimeVersion(e.file_name().to_string_lossy().into_owned()));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

pub fn read_current(paths: &NodePaths) -> EnvrResult<Option<RuntimeVersion>> {
    let cur = paths.current_link();
    if !cur.exists() {
        return Ok(None);
    }
    // 1) Usual case: `current` is a symlink/junction.
    if let Ok(target) = fs::read_link(&cur) {
        let resolved = if target.is_relative() {
            cur.parent().map(|p| p.join(&target)).unwrap_or(target)
        } else {
            target
        };
        let name = resolved
            .file_name()
            .ok_or_else(|| err_version_not_found("invalid node current link"))?
            .to_string_lossy()
            .into_owned();
        return Ok(Some(RuntimeVersion(name)));
    }

    // 2) Fallback: `current` is a pointer file whose content is the absolute target dir.
    let s = fs::read_to_string(&cur).map_err(EnvrError::from)?;
    let t = s.trim();
    if t.is_empty() {
        return Ok(None);
    }
    let target = PathBuf::from(t);
    if !target.is_dir() {
        return Ok(None);
    }
    let name = target
        .file_name()
        .ok_or_else(|| err_version_not_found("invalid node current pointer"))?
        .to_string_lossy()
        .into_owned();
    Ok(Some(RuntimeVersion(name)))
}

pub struct NodeManager {
    pub paths: NodePaths,
    index_json_url: String,
    client: reqwest::blocking::Client,
}

impl NodeManager {
    pub fn try_new(runtime_root: PathBuf, index_json_url: String) -> EnvrResult<Self> {
        Ok(Self {
            paths: NodePaths::new(runtime_root),
            index_json_url,
            client: blocking_http_client()?,
        })
    }

    fn dist_root(&self) -> String {
        dist_root_from_index_json_url(&self.index_json_url)
    }

    fn load_releases(&self) -> EnvrResult<Vec<NodeRelease>> {
        let body = fetch_node_index(&self.client, &self.index_json_url)?;
        parse_node_index(&body)
    }

    pub fn install_resolved_version(
        &self,
        version: &RuntimeVersion,
        progress_downloaded: Option<&Arc<AtomicU64>>,
        progress_total: Option<&Arc<AtomicU64>>,
        cancel: Option<&Arc<AtomicBool>>,
    ) -> EnvrResult<RuntimeVersion> {
        let os = std::env::consts::OS;
        let arch = std::env::consts::ARCH;
        let version_v = node_version_v_prefix(&version.0);

        let shasums_url = join_url_path(&self.dist_root(), &format!("{version_v}/SHASUMS256.txt"));
        let shasums_text = self
            .client
            .get(&shasums_url)
            .send()
            .map_err(|e| {
                EnvrError::with_source(
                    ErrorCode::RemoteIndexFetchFailed,
                    format!("request failed for {shasums_url}"),
                    e,
                )
            })
            .and_then(|r| {
                if !r.status().is_success() {
                    return Err(EnvrError::Context {
                        code: ErrorCode::RemoteIndexFetchFailed,
                        message: format!("GET {} -> {}", shasums_url, r.status()),
                        source: Box::new(std::io::Error::other("remote-index-fetch-failed")),
                    });
                }
                r.text().map_err(|e| {
                    EnvrError::with_source(
                        ErrorCode::RemoteIndexFetchFailed,
                        format!("read body failed for {shasums_url}"),
                        e,
                    )
                })
            })?;

        let entries = parse_shasums256(&shasums_text)?;
        let (sha_expect, filename) = pick_node_dist_artifact(&entries, os, arch, &version_v)?;
        let artifact_url = join_url_path(&self.dist_root(), &format!("{version_v}/{filename}"));

        let cache_file = self.paths.cache_dir().join(&version.0).join(&filename);
        let final_dir = self.paths.version_dir(&version.0);
        execute_install_pipeline(
            cancel,
            || fs::create_dir_all(self.paths.cache_dir()).map_err(EnvrError::from),
            || {
                download_url_to_path_resumable(
                    &self.client,
                    &artifact_url,
                    &cache_file,
                    progress_downloaded,
                    progress_total,
                    cancel,
                )
            },
            || checksum::verify_sha256_hex(&cache_file, &sha_expect),
            || {
                let staging_parent = self.paths.cache_dir().join(&version.0);
                fs::create_dir_all(&staging_parent).map_err(EnvrError::from)?;
                let staging = tempfile::tempdir_in(&staging_parent).map_err(EnvrError::from)?;
                extract::extract_archive(&cache_file, staging.path())?;
                envr_platform::install_layout::commit_single_root_dir(
                    staging.path(),
                    &final_dir,
                    node_installation_valid,
                    "empty node archive",
                    "expected exactly one root directory in node archive",
                    "expected node archive root to be a directory",
                    "extracted node layout missing node binary",
                )
            },
            || {
                if !node_installation_valid(&final_dir) {
                    return Err(err_version_not_found(
                        "extracted node layout missing node binary",
                    ));
                }
                self.set_current(version)?;
                Ok(RuntimeVersion(normalize_node_version(&version.0)))
            },
        )
    }

    pub fn set_current(&self, version: &RuntimeVersion) -> EnvrResult<()> {
        let dir = self.paths.version_dir(&version.0);
        if !node_installation_valid(&dir) {
            return Err(err_version_not_found(format!(
                "node {} is not installed",
                version.0
            )));
        }
        let abs = fs::canonicalize(&dir).map_err(EnvrError::from)?;
        let cur = self.paths.current_link();
        match ensure_link(LinkType::Soft, &abs, &cur) {
            Ok(()) => Ok(()),
            // Windows may forbid creating symlinks/junctions in some environments
            // (e.g. missing "Create symbolic links" privilege).
            // Fall back to a pointer file that shim resolution can follow.
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

impl SpecDrivenInstaller for NodeManager {
    fn install_from_spec(&self, request: &InstallRequest) -> EnvrResult<RuntimeVersion> {
        let releases = self.load_releases()?;
        let os = std::env::consts::OS;
        let arch = std::env::consts::ARCH;
        let label = resolve_node_version(&releases, os, arch, &request.spec.0)?;
        let (downloaded, total, cancel) = install_progress_handles(request);
        self.install_resolved_version(&RuntimeVersion(label), downloaded, total, cancel)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dist_root_trims_index_json() {
        assert_eq!(
            dist_root_from_index_json_url("https://nodejs.org/dist/index.json"),
            "https://nodejs.org/dist"
        );
    }

    #[test]
    fn parse_shasums_accepts_gnu_format() {
        let h64 = "a".repeat(64);
        let text = format!("{h64}  node-v1.0.0-linux-x64.tar.xz\n{h64} *node-v1.0.0-win-x64.zip\n");
        let p = parse_shasums256(&text).expect("parse");
        assert_eq!(p.len(), 2);
        assert_eq!(p[0].1, "node-v1.0.0-linux-x64.tar.xz");
        assert_eq!(p[1].1, "node-v1.0.0-win-x64.zip");
    }

    #[test]
    fn pick_prefers_linux_xz() {
        let h64 = "b".repeat(64);
        let entries = vec![
            (h64.clone(), "node-v10.0.0-linux-x64.tar.gz".to_string()),
            (h64.clone(), "node-v10.0.0-linux-x64.tar.xz".to_string()),
        ];
        let (_, n) = pick_node_dist_artifact(&entries, "linux", "x86_64", "v10.0.0").expect("pick");
        assert_eq!(n, "node-v10.0.0-linux-x64.tar.xz");
    }

    #[test]
    fn pick_artifact_not_found_has_structured_code() {
        let entries = vec![("x".repeat(64), "node-v10.0.0-win-x64.zip".to_string())];
        let err =
            pick_node_dist_artifact(&entries, "linux", "x86_64", "v10.0.0").expect_err("no dist");
        assert_eq!(err.code(), ErrorCode::RuntimeVersionNotFound);
    }

    #[test]
    fn parse_shasums_malformed_has_structured_code() {
        let err = parse_shasums256("only_hash_without_name").expect_err("malformed");
        assert_eq!(err.code(), ErrorCode::RuntimeVersionSpecInvalid);
    }

    #[test]
    fn promote_single_root_moves_inner() {
        let tmp = tempfile::tempdir().expect("tmp");
        let staging = tmp.path().join("st");
        let inner = staging.join("node-v0.0.0");
        fs::create_dir_all(&inner).expect("mkdir");
        #[cfg(windows)]
        fs::write(inner.join("node.exe"), []).expect("node");
        #[cfg(not(windows))]
        {
            fs::create_dir_all(inner.join("bin")).expect("bin");
            fs::write(inner.join("bin").join("node"), []).expect("node");
        }
        let fin = tmp.path().join("out");
        envr_platform::install_layout::commit_single_root_dir(
            &staging,
            &fin,
            node_installation_valid,
            "empty node archive",
            "expected exactly one root directory in node archive",
            "expected node archive root to be a directory",
            "extracted node layout missing node binary",
        )
        .expect("promote");
        assert!(node_installation_valid(&fin));
    }
}
