//! JDK version discovery via the Eclipse Adoptium v3 API.

use crate::vendor::JavaVendor;
use envr_domain::runtime::{RemoteFilter, RuntimeVersion};
use envr_error::{EnvrError, EnvrResult};
use serde::Deserialize;
use std::cmp::Reverse;
use std::time::Duration;

pub const DEFAULT_ADOPTIUM_API_BASE: &str = "https://api.adoptium.net";

#[derive(Debug, Clone, Deserialize)]
pub struct JavaVersionEntry {
    pub build: u32,
    pub major: u32,
    pub minor: u32,
    pub security: u32,
    pub openjdk_version: String,
    pub semver: String,
    #[serde(default)]
    pub optional: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ReleaseVersionsBody {
    versions: Vec<JavaVersionEntry>,
}

#[derive(Debug, Deserialize)]
struct AvailableReleasesBody {
    #[serde(default)]
    available_lts_releases: Vec<u32>,
}

#[derive(Debug, Clone)]
pub struct JavaIndex {
    pub versions: Vec<JavaVersionEntry>,
    pub lts_majors: Vec<u32>,
}

fn version_sort_key(v: &JavaVersionEntry) -> (u32, u32, u32, u32) {
    (v.major, v.minor, v.security, v.build)
}

pub fn adoptium_os(host_os: &str) -> EnvrResult<&'static str> {
    match host_os {
        "windows" => Ok("windows"),
        "linux" => Ok("linux"),
        "macos" => Ok("mac"),
        _ => Err(EnvrError::Platform(format!(
            "unsupported OS for adoptium: {host_os}"
        ))),
    }
}

pub fn adoptium_arch(host_arch: &str) -> EnvrResult<&'static str> {
    match host_arch {
        "x86_64" => Ok("x64"),
        "aarch64" => Ok("aarch64"),
        "x86" => Ok("x32"),
        _ => Err(EnvrError::Platform(format!(
            "unsupported CPU arch for adoptium: {host_arch}"
        ))),
    }
}

pub fn blocking_http_client() -> EnvrResult<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(90))
        .user_agent(concat!("envr-runtime-java/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| EnvrError::Download(e.to_string()))
}

fn fetch_text(client: &reqwest::blocking::Client, url: &str) -> EnvrResult<String> {
    let r = client
        .get(url)
        .send()
        .map_err(|e| EnvrError::Download(e.to_string()))?;
    if !r.status().is_success() {
        return Err(EnvrError::Download(format!("GET {url} -> {}", r.status())));
    }
    r.text().map_err(|e| EnvrError::Download(e.to_string()))
}

pub fn fetch_available_lts_majors(
    client: &reqwest::blocking::Client,
    api_base: &str,
) -> EnvrResult<Vec<u32>> {
    let url = format!(
        "{}/v3/info/available_releases",
        api_base.trim_end_matches('/')
    );
    let body = fetch_text(client, &url)?;
    let parsed: AvailableReleasesBody =
        serde_json::from_str(&body).map_err(|e| EnvrError::Validation(e.to_string()))?;
    Ok(parsed.available_lts_releases)
}

pub fn fetch_release_versions(
    client: &reqwest::blocking::Client,
    api_base: &str,
    vendor: JavaVendor,
    os: &str,
    arch: &str,
) -> EnvrResult<Vec<JavaVersionEntry>> {
    let base = api_base.trim_end_matches('/');
    let v = vendor.adoptium_vendor_param();
    let url = format!(
        "{base}/v3/info/release_versions?release_type=ga&vendor={v}\
         &heap_size=normal&image_type=jdk&jvm_impl=hotspot&os={os}&architecture={arch}"
    );
    let body = fetch_text(client, &url)?;
    let parsed: ReleaseVersionsBody =
        serde_json::from_str(&body).map_err(|e| EnvrError::Validation(e.to_string()))?;
    Ok(parsed.versions)
}

pub fn load_java_index(
    client: &reqwest::blocking::Client,
    api_base: &str,
    vendor: JavaVendor,
    host_os: &str,
    host_arch: &str,
) -> EnvrResult<JavaIndex> {
    let os = adoptium_os(host_os)?;
    let arch = adoptium_arch(host_arch)?;
    let lts_majors = fetch_available_lts_majors(client, api_base)?;
    let mut versions = fetch_release_versions(client, api_base, vendor, os, arch)?;
    versions.sort_by_key(|b| Reverse(version_sort_key(b)));
    Ok(JavaIndex {
        versions,
        lts_majors,
    })
}

pub fn list_remote_versions(
    index: &JavaIndex,
    filter: &RemoteFilter,
) -> EnvrResult<Vec<RuntimeVersion>> {
    let mut out: Vec<RuntimeVersion> = index
        .versions
        .iter()
        .map(|v| RuntimeVersion(v.openjdk_version.clone()))
        .collect();

    if let Some(prefix) = &filter.prefix {
        let p = prefix.trim().to_ascii_lowercase();
        if !p.is_empty() {
            out.retain(|rv| rv.0.to_ascii_lowercase().starts_with(&p));
        }
    }
    Ok(out)
}

fn is_lts_entry(v: &JavaVersionEntry, lts_majors: &[u32]) -> bool {
    let opt_lts = v
        .optional
        .as_deref()
        .is_some_and(|s| s.eq_ignore_ascii_case("lts"));
    opt_lts || lts_majors.contains(&v.major)
}

pub fn resolve_java_version(index: &JavaIndex, spec: &str) -> EnvrResult<String> {
    let t = spec.trim();
    if t.is_empty() {
        return Err(EnvrError::Validation("empty java version spec".into()));
    }
    if index.versions.is_empty() {
        return Err(EnvrError::Validation(
            "no adoptium jdk releases for this platform".into(),
        ));
    }

    let lower = t.to_ascii_lowercase();
    if matches!(lower.as_str(), "latest" | "current" | "stable") {
        return Ok(index.versions[0].openjdk_version.clone());
    }

    if lower == "lts" {
        let best = index
            .versions
            .iter()
            .filter(|v| is_lts_entry(v, &index.lts_majors))
            .max_by_key(|v| version_sort_key(v));
        let Some(v) = best else {
            return Err(EnvrError::Validation(
                "no LTS jdk releases in adoptium index for this platform".into(),
            ));
        };
        return Ok(v.openjdk_version.clone());
    }

    if let Ok(partial) = parse_partial_spec(t) {
        return pick_partial(index, partial);
    }

    let norm = t.strip_prefix('v').unwrap_or(t);
    if let Some(v) = index
        .versions
        .iter()
        .find(|v| v.openjdk_version == norm || v.semver == norm)
    {
        return Ok(v.openjdk_version.clone());
    }

    Err(EnvrError::Validation(format!(
        "no adoptium jdk matches spec {spec:?} for this platform"
    )))
}

#[derive(Debug, Clone, Copy)]
enum SpecMatch {
    Major(u32),
    MajorMinor(u32, u32),
    MajorMinorSecurity(u32, u32, u32),
}

fn parse_partial_spec(s: &str) -> EnvrResult<SpecMatch> {
    let t = s.trim().strip_prefix('v').unwrap_or(s.trim());
    let parts: Vec<&str> = t.split('.').collect();
    match parts.len() {
        1 => {
            let major: u32 = parts[0]
                .parse()
                .map_err(|_| EnvrError::Validation(format!("invalid java version spec: {s}")))?;
            Ok(SpecMatch::Major(major))
        }
        2 => {
            let major: u32 = parts[0]
                .parse()
                .map_err(|_| EnvrError::Validation(format!("invalid java version spec: {s}")))?;
            let minor: u32 = parts[1]
                .parse()
                .map_err(|_| EnvrError::Validation(format!("invalid java version spec: {s}")))?;
            Ok(SpecMatch::MajorMinor(major, minor))
        }
        3 => {
            let major: u32 = parts[0]
                .parse()
                .map_err(|_| EnvrError::Validation(format!("invalid java version spec: {s}")))?;
            let minor: u32 = parts[1]
                .parse()
                .map_err(|_| EnvrError::Validation(format!("invalid java version spec: {s}")))?;
            let sec: u32 = parts[2]
                .parse()
                .map_err(|_| EnvrError::Validation(format!("invalid java version spec: {s}")))?;
            Ok(SpecMatch::MajorMinorSecurity(major, minor, sec))
        }
        _ => Err(EnvrError::Validation(format!(
            "unsupported java version spec: {s}"
        ))),
    }
}

fn pick_partial(index: &JavaIndex, spec: SpecMatch) -> EnvrResult<String> {
    let best = match spec {
        SpecMatch::Major(major) => index
            .versions
            .iter()
            .filter(|v| v.major == major)
            .max_by_key(|v| version_sort_key(v)),
        SpecMatch::MajorMinor(major, minor) => index
            .versions
            .iter()
            .filter(|v| v.major == major && v.minor == minor)
            .max_by_key(|v| version_sort_key(v)),
        SpecMatch::MajorMinorSecurity(major, minor, sec) => index
            .versions
            .iter()
            .filter(|v| v.major == major && v.minor == minor && v.security == sec)
            .max_by_key(|v| version_sort_key(v)),
    };
    let Some(v) = best else {
        return Err(EnvrError::Validation(
            "no matching adoptium jdk for this spec".into(),
        ));
    };
    Ok(v.openjdk_version.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_index() -> JavaIndex {
        JavaIndex {
            versions: vec![
                JavaVersionEntry {
                    build: 10,
                    major: 25,
                    minor: 0,
                    security: 2,
                    openjdk_version: "25.0.2+10-LTS".into(),
                    semver: "25.0.2+10.0.LTS".into(),
                    optional: Some("LTS".into()),
                },
                JavaVersionEntry {
                    build: 9,
                    major: 21,
                    minor: 0,
                    security: 6,
                    openjdk_version: "21.0.6+9-LTS".into(),
                    semver: "21.0.6+9.0.LTS".into(),
                    optional: Some("LTS".into()),
                },
                JavaVersionEntry {
                    build: 12,
                    major: 24,
                    minor: 0,
                    security: 2,
                    openjdk_version: "24.0.2+12".into(),
                    semver: "24.0.2+12".into(),
                    optional: None,
                },
            ],
            lts_majors: vec![21, 25],
        }
    }

    #[test]
    fn resolve_latest_and_lts() {
        let idx = sample_index();
        assert_eq!(
            resolve_java_version(&idx, "latest").expect("r"),
            "25.0.2+10-LTS"
        );
        assert_eq!(
            resolve_java_version(&idx, "lts").expect("r"),
            "25.0.2+10-LTS"
        );
    }

    #[test]
    fn resolve_major_line() {
        let idx = sample_index();
        assert_eq!(resolve_java_version(&idx, "21").expect("r"), "21.0.6+9-LTS");
    }

    #[test]
    fn list_remote_prefix() {
        let idx = sample_index();
        let list = list_remote_versions(
            &idx,
            &RemoteFilter {
                prefix: Some("21".into()),
            },
        )
        .expect("list");
        assert_eq!(list.len(), 1);
        assert!(list[0].0.starts_with("21"));
    }
}
