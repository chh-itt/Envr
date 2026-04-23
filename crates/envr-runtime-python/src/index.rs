//! CPython release index via `python.org` downloads API (OData-style JSON).
//!
//! - Releases: `GET /api/v2/downloads/release/`
//! - Files: `GET /api/v2/downloads/release_file/`

use envr_domain::runtime::{RemoteFilter, RuntimeVersion};
use envr_download::blocking::build_blocking_http_client;
use envr_error::{EnvrError, EnvrResult, ErrorCode};
use regex::Regex;
use serde::Deserialize;
use std::{
    collections::{HashMap, HashSet},
    sync::OnceLock,
    time::Duration,
};

fn python_triple_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)Python\s+(\d+)\.(\d+)\.(\d+)").expect("python version regex")
    })
}

pub const DEFAULT_PYTHON_RELEASES_URL: &str = "https://www.python.org/api/v2/downloads/release/";
pub const DEFAULT_PYTHON_RELEASE_FILES_URL: &str =
    "https://www.python.org/api/v2/downloads/release_file/";

const OS_WINDOWS_URI_PART: &str = "/downloads/os/1/";
const OS_MACOS_URI_PART: &str = "/downloads/os/2/";
const OS_SOURCE_URI_PART: &str = "/downloads/os/3/";

#[derive(Debug, Deserialize)]
struct ApiList<T> {
    value: Vec<T>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PyRelease {
    pub name: String,
    pub slug: String,
    pub is_published: bool,
    pub pre_release: bool,
    pub show_on_download_page: bool,
    pub resource_uri: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PyReleaseFile {
    pub name: String,
    pub os: String,
    pub release: String,
    pub is_source: bool,
    pub url: String,
    #[serde(default)]
    pub sha256_sum: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct SemKey(u64, u64, u64);

fn semver_key_from_name(name: &str) -> Option<SemKey> {
    let c = python_triple_re().captures(name)?;
    let major: u64 = c.get(1)?.as_str().parse().ok()?;
    let minor: u64 = c.get(2)?.as_str().parse().ok()?;
    let patch: u64 = c.get(3)?.as_str().parse().ok()?;
    Some(SemKey(major, minor, patch))
}

pub fn normalize_python_version_label(name: &str) -> Option<String> {
    let c = python_triple_re().captures(name)?;
    Some(format!(
        "{}.{}.{}",
        c.get(1)?.as_str(),
        c.get(2)?.as_str(),
        c.get(3)?.as_str()
    ))
}

pub fn release_id_from_uri(uri: &str) -> Option<u32> {
    uri.trim_end_matches('/').rsplit('/').next()?.parse().ok()
}

pub fn blocking_http_client() -> EnvrResult<reqwest::blocking::Client> {
    build_blocking_http_client(
        concat!("envr-runtime-python/", env!("CARGO_PKG_VERSION")),
        Some(Duration::from_secs(120)),
    )
}

pub fn fetch_json(client: &reqwest::blocking::Client, url: &str) -> EnvrResult<String> {
    let response = client
        .get(url)
        .send()
        .map_err(|e| EnvrError::with_source(ErrorCode::Download, format!("request failed for {url}"), e))?;
    if !response.status().is_success() {
        return Err(EnvrError::Download(format!(
            "GET {} -> {}",
            url,
            response.status()
        )));
    }
    response
        .text()
        .map_err(|e| EnvrError::with_source(ErrorCode::Download, format!("read body failed for {url}"), e))
}

pub fn parse_release_list(json: &str) -> EnvrResult<Vec<PyRelease>> {
    // python.org may return either:
    // 1) root array: `[ { ...release... }, ... ]`
    // 2) object with `value`: `{ "value": [ ... ] }`
    let v: serde_json::Value =
        serde_json::from_str(json)
            .map_err(|e| EnvrError::with_source(ErrorCode::Validation, "invalid python releases json", e))?;
    if v.is_array() {
        serde_json::from_value::<Vec<PyRelease>>(v)
            .map_err(|e| EnvrError::with_source(ErrorCode::Validation, "invalid python releases json", e))
    } else if v.get("value").is_some() {
        let list: ApiList<PyRelease> =
            serde_json::from_value(v)
                .map_err(|e| EnvrError::with_source(ErrorCode::Validation, "invalid python releases json", e))?;
        Ok(list.value)
    } else {
        Err(EnvrError::Validation(
            "python release api: unexpected json shape".into(),
        ))
    }
}

pub fn parse_release_file_list(json: &str) -> EnvrResult<Vec<PyReleaseFile>> {
    // python.org may return either:
    // 1) root array
    // 2) object with `value`
    let v: serde_json::Value =
        serde_json::from_str(json)
            .map_err(|e| EnvrError::with_source(ErrorCode::Validation, "invalid python release_files json", e))?;
    if v.is_array() {
        serde_json::from_value::<Vec<PyReleaseFile>>(v)
            .map_err(|e| {
                EnvrError::with_source(ErrorCode::Validation, "invalid python release_files json", e)
            })
    } else if v.get("value").is_some() {
        let list: ApiList<PyReleaseFile> =
            serde_json::from_value(v).map_err(|e| {
                EnvrError::with_source(ErrorCode::Validation, "invalid python release_files json", e)
            })?;
        Ok(list.value)
    } else {
        Err(EnvrError::Validation(
            "python release_file api: unexpected json shape".into(),
        ))
    }
}

fn index_files_by_release(files: &[PyReleaseFile]) -> HashMap<u32, Vec<PyReleaseFile>> {
    let mut m: HashMap<u32, Vec<PyReleaseFile>> = HashMap::new();
    for f in files {
        if let Some(rid) = release_id_from_uri(&f.release) {
            m.entry(rid).or_default().push(f.clone());
        }
    }
    m
}

fn os_is_windows(os: &str) -> bool {
    os.contains(OS_WINDOWS_URI_PART)
}

fn os_is_macos(os: &str) -> bool {
    os.contains(OS_MACOS_URI_PART)
}

fn os_is_source(os: &str) -> bool {
    os.contains(OS_SOURCE_URI_PART)
}

/// Returns whether this release ships at least one artifact usable on `(os, arch)`.
///
/// Linux uses the official source `.tar.xz` (build-from-source) as the portable baseline.
pub fn release_has_platform_assets(files: &[PyReleaseFile], os: &str, arch: &str) -> bool {
    match os {
        "windows" => files
            .iter()
            .any(|f| !f.is_source && os_is_windows(&f.os) && windows_file_matches_arch(f, arch)),
        "macos" => files.iter().any(|f| !f.is_source && os_is_macos(&f.os)),
        "linux" => files.iter().any(|f| {
            f.is_source && os_is_source(&f.os) && f.url.to_ascii_lowercase().ends_with(".tar.xz")
        }),
        _ => false,
    }
}

fn windows_file_matches_arch(f: &PyReleaseFile, arch: &str) -> bool {
    let u = f.url.to_ascii_lowercase();
    let n = f.name.to_ascii_lowercase();
    match arch {
        "x86_64" => {
            (u.contains("amd64") || n.contains("64-bit"))
                && !u.contains("arm64")
                && !n.contains("arm64")
        }
        "aarch64" => u.contains("arm64") || n.contains("arm64"),
        "x86" => u.contains("win32") || u.contains("embed-win32") || n.contains("32-bit"),
        _ => false,
    }
}

#[derive(Debug, Clone)]
pub struct PythonIndex {
    pub releases: Vec<PyRelease>,
    pub files_by_release: HashMap<u32, Vec<PyReleaseFile>>,
}

pub fn load_python_index(
    client: &reqwest::blocking::Client,
    releases_url: &str,
    files_url: &str,
) -> EnvrResult<PythonIndex> {
    let rel_json = fetch_json(client, releases_url)?;
    let file_json = fetch_json(client, files_url)?;
    let releases = parse_release_list(&rel_json)?;
    let files = parse_release_file_list(&file_json)?;
    let files_by_release = index_files_by_release(&files);
    Ok(PythonIndex {
        releases,
        files_by_release,
    })
}

fn candidate_releases(index: &PythonIndex, os: &str, arch: &str) -> Vec<(SemKey, String, u32)> {
    let mut out = Vec::new();
    for r in &index.releases {
        if !r.is_published || !r.show_on_download_page || r.pre_release {
            continue;
        }
        let Some(sem) = semver_key_from_name(&r.name) else {
            continue;
        };
        let Some(ver) = normalize_python_version_label(&r.name) else {
            continue;
        };
        let Some(rid) = release_id_from_uri(&r.resource_uri) else {
            continue;
        };
        let files = index
            .files_by_release
            .get(&rid)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        if !release_has_platform_assets(files, os, arch) {
            continue;
        }
        out.push((sem, ver, rid));
    }
    out.sort_by(|a, b| b.0.cmp(&a.0));
    out
}

pub fn list_remote_versions(
    index: &PythonIndex,
    os: &str,
    arch: &str,
    filter: &RemoteFilter,
) -> EnvrResult<Vec<RuntimeVersion>> {
    let rows = candidate_releases(index, os, arch);
    let mut out: Vec<RuntimeVersion> = rows
        .into_iter()
        .map(|(_, v, _)| RuntimeVersion(v))
        .collect();

    if let Some(prefix) = &filter.prefix {
        let p = prefix.trim().to_ascii_lowercase();
        if !p.is_empty() {
            out.retain(|rv| rv.0.to_ascii_lowercase().starts_with(&p));
        }
    }
    Ok(out)
}

/// Latest patch version per Python major.minor line for GUI list rows.
///
/// For Python labels like `3.14.3`, the "major line" here is `3.14` (install spec may be `3.14`).
pub fn list_latest_patch_per_major(
    index: &PythonIndex,
    os: &str,
    arch: &str,
) -> EnvrResult<Vec<RuntimeVersion>> {
    // `candidate_releases` is already sorted by semantic version descending,
    // so the first occurrence for each (major, minor) is the latest patch for that line.
    let rows = candidate_releases(index, os, arch);
    let mut seen: HashSet<(u64, u64)> = HashSet::new();
    let mut out: Vec<RuntimeVersion> = Vec::new();
    for (sem, ver, _rid) in rows {
        if seen.insert((sem.0, sem.1)) {
            out.push(RuntimeVersion(ver));
        }
    }
    Ok(out)
}

pub fn resolve_python_version(
    index: &PythonIndex,
    os: &str,
    arch: &str,
    spec: &str,
) -> EnvrResult<String> {
    let spec_trim = spec.trim();
    if spec_trim.is_empty() {
        return Err(EnvrError::Validation("empty python version spec".into()));
    }
    let rows = candidate_releases(index, os, arch);
    if rows.is_empty() {
        return Err(EnvrError::Validation(
            "no published python releases for this platform".into(),
        ));
    }

    let lower = spec_trim.to_ascii_lowercase();
    if matches!(lower.as_str(), "latest" | "stable" | "current") {
        return Ok(rows[0].1.clone());
    }

    if let Ok(sem) = parse_partial_spec(spec_trim) {
        return pick_partial(&rows, sem);
    }

    let norm = spec_trim.strip_prefix('v').unwrap_or(spec_trim);
    if rows.iter().any(|(_, ver, _)| ver == norm) {
        return Ok(norm.to_string());
    }

    Err(EnvrError::Validation(format!(
        "no python release matches spec {spec:?} for this platform"
    )))
}

#[derive(Debug, Clone, Copy)]
enum SpecMatch {
    Major(u64),
    MajorMinor(u64, u64),
    Exact(u64, u64, u64),
}

fn parse_partial_spec(s: &str) -> EnvrResult<SpecMatch> {
    let t = s.trim().strip_prefix('v').unwrap_or(s.trim());
    let parts: Vec<&str> = t.split('.').collect();
    match parts.len() {
        1 => {
            let major: u64 = parts[0]
                .parse()
                .map_err(|_| EnvrError::Validation(format!("invalid version spec: {s}")))?;
            Ok(SpecMatch::Major(major))
        }
        2 => {
            let major: u64 = parts[0]
                .parse()
                .map_err(|_| EnvrError::Validation(format!("invalid version spec: {s}")))?;
            let minor: u64 = parts[1]
                .parse()
                .map_err(|_| EnvrError::Validation(format!("invalid version spec: {s}")))?;
            Ok(SpecMatch::MajorMinor(major, minor))
        }
        3 => {
            let major: u64 = parts[0]
                .parse()
                .map_err(|_| EnvrError::Validation(format!("invalid version spec: {s}")))?;
            let minor: u64 = parts[1]
                .parse()
                .map_err(|_| EnvrError::Validation(format!("invalid version spec: {s}")))?;
            let patch: u64 = parts[2]
                .parse()
                .map_err(|_| EnvrError::Validation(format!("invalid version spec: {s}")))?;
            Ok(SpecMatch::Exact(major, minor, patch))
        }
        _ => Err(EnvrError::Validation(format!(
            "unsupported python version spec: {s}"
        ))),
    }
}

/// API release id for a resolved version label (`3.12.1`) on this host.
pub fn release_id_for_version_label(
    index: &PythonIndex,
    version_label: &str,
    os: &str,
    arch: &str,
) -> EnvrResult<u32> {
    let rows = candidate_releases(index, os, arch);
    rows.iter()
        .find(|(_, v, _)| v == version_label)
        .map(|(_, _, id)| *id)
        .ok_or_else(|| {
            EnvrError::Validation(format!(
                "python {version_label} is not available for this platform"
            ))
        })
}

/// Artifact used for `envr` installs: Windows embeddable `.zip` (+ bootstrap pip); Unix source `.tar.xz` (+ `make install` + ensurepip).
pub fn pick_install_artifact(
    files: &[PyReleaseFile],
    os: &str,
    arch: &str,
) -> EnvrResult<PyReleaseFile> {
    match os {
        "windows" => pick_windows_embed_zip(files, arch),
        "linux" | "macos" => pick_source_xz_tarball(files),
        _ => Err(EnvrError::Platform(format!(
            "unsupported OS for python install: {os}"
        ))),
    }
}

fn pick_windows_embed_zip(files: &[PyReleaseFile], arch: &str) -> EnvrResult<PyReleaseFile> {
    let mut candidates: Vec<PyReleaseFile> = files
        .iter()
        .filter(|f| {
            !f.is_source
                && os_is_windows(&f.os)
                && windows_file_matches_arch(f, arch)
                && f.url.to_ascii_lowercase().ends_with(".zip")
                && f.url.to_ascii_lowercase().contains("embed")
                && !f.name.to_ascii_lowercase().contains("debug")
        })
        .cloned()
        .collect();
    candidates.sort_by(|a, b| b.url.len().cmp(&a.url.len()));
    candidates.into_iter().next().ok_or_else(|| {
        EnvrError::Validation("no Windows embeddable zip for this CPU architecture".into())
    })
}

fn pick_source_xz_tarball(files: &[PyReleaseFile]) -> EnvrResult<PyReleaseFile> {
    files
        .iter()
        .find(|f| {
            f.is_source && os_is_source(&f.os) && f.url.to_ascii_lowercase().ends_with(".tar.xz")
        })
        .cloned()
        .ok_or_else(|| {
            EnvrError::Validation(
                "no XZ source tarball in release (required for Linux/macOS install)".into(),
            )
        })
}

fn pick_partial(rows: &[(SemKey, String, u32)], spec: SpecMatch) -> EnvrResult<String> {
    let best = match spec {
        SpecMatch::Exact(ma, mi, p) => {
            let key = SemKey(ma, mi, p);
            rows.iter().find(|(k, _, _)| *k == key)
        }
        SpecMatch::MajorMinor(ma, mi) => rows
            .iter()
            .filter(|(k, _, _)| k.0 == ma && k.1 == mi)
            .max_by_key(|(k, _, _)| *k),
        SpecMatch::Major(ma) => rows
            .iter()
            .filter(|(k, _, _)| k.0 == ma)
            .max_by_key(|(k, _, _)| *k),
    };
    let Some((_, v, _)) = best else {
        return Err(EnvrError::Validation(
            "no matching python release for this spec".into(),
        ));
    };
    Ok(v.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semver_parse_name() {
        assert_eq!(
            semver_key_from_name("Python 3.12.1"),
            Some(SemKey(3, 12, 1))
        );
        assert_eq!(
            normalize_python_version_label("Python 3.13.0"),
            Some("3.13.0".into())
        );
    }

    #[test]
    fn windows_arch_filter() {
        let f = PyReleaseFile {
            name: "e".into(),
            os: "https://www.python.org/api/v2/downloads/os/1/".into(),
            release: "x".into(),
            is_source: false,
            url: "https://x/python-3.12.1-amd64.exe".into(),
            sha256_sum: None,
        };
        assert!(windows_file_matches_arch(&f, "x86_64"));
        assert!(!windows_file_matches_arch(&f, "x86"));
    }

    #[test]
    fn list_and_resolve_uses_fixture() {
        let index = PythonIndex {
            releases: vec![
                PyRelease {
                    name: "Python 3.11.0".into(),
                    slug: "python-3110".into(),
                    is_published: true,
                    pre_release: false,
                    show_on_download_page: true,
                    resource_uri: "https://www.python.org/api/v2/downloads/release/1/".into(),
                },
                PyRelease {
                    name: "Python 3.12.1".into(),
                    slug: "python-3121".into(),
                    is_published: true,
                    pre_release: false,
                    show_on_download_page: true,
                    resource_uri: "https://www.python.org/api/v2/downloads/release/2/".into(),
                },
            ],
            files_by_release: HashMap::from([(
                2,
                vec![PyReleaseFile {
                    name: "xz".into(),
                    os: "https://www.python.org/api/v2/downloads/os/3/".into(),
                    release: "https://www.python.org/api/v2/downloads/release/2/".into(),
                    is_source: true,
                    url: "https://www.python.org/ftp/python/3.12.1/Python-3.12.1.tar.xz".into(),
                    sha256_sum: None,
                }],
            )]),
        };
        let list = list_remote_versions(&index, "linux", "x86_64", &RemoteFilter::default())
            .expect("list");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].0, "3.12.1");
        let r = resolve_python_version(&index, "linux", "x86_64", "3.12").expect("r");
        assert_eq!(r, "3.12.1");
    }

    #[test]
    fn pick_install_embed_windows() {
        let files = vec![
            PyReleaseFile {
                name: "embed".into(),
                os: "https://www.python.org/api/v2/downloads/os/1/".into(),
                release: "r".into(),
                is_source: false,
                url: "https://x/python-3.12.1-embed-amd64.zip".into(),
                sha256_sum: None,
            },
            PyReleaseFile {
                name: "exe".into(),
                os: "https://www.python.org/api/v2/downloads/os/1/".into(),
                release: "r".into(),
                is_source: false,
                url: "https://x/python-3.12.1-amd64.exe".into(),
                sha256_sum: None,
            },
        ];
        let p = pick_install_artifact(&files, "windows", "x86_64").expect("pick");
        assert!(p.url.contains("embed"));
    }

    #[test]
    fn pick_install_source_unix() {
        let files = vec![PyReleaseFile {
            name: "src".into(),
            os: "https://www.python.org/api/v2/downloads/os/3/".into(),
            release: "r".into(),
            is_source: true,
            url: "https://x/Python-3.12.1.tar.xz".into(),
            sha256_sum: None,
        }];
        let p = pick_install_artifact(&files, "linux", "x86_64").expect("pick");
        assert!(p.url.ends_with(".tar.xz"));
    }
}
