//! JDK version discovery via the Eclipse Adoptium v3 API.

use crate::vendor::JavaVendor;
use envr_domain::runtime::{RemoteFilter, RuntimeVersion};
use envr_download::blocking::build_blocking_http1_only_client;
use envr_error::{EnvrError, EnvrResult, ErrorCode};
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

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct LatestAssetsVersionData {
    openjdk_version: String,
    #[serde(default)]
    semver: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct LatestAssetsRelease {
    version_data: LatestAssetsVersionData,
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
    // JDK zips are large (often 150–300MB+). Keep a generous total timeout so slow links
    // do not abort mid-body (otherwise surfaces like a "bad mirror").
    //
    // Some domestic mirrors and middleboxes behave poorly with HTTP/2 + rustls; HTTP/1.1 only
    // avoids a class of fast failures. A browser-like User-Agent reduces odd 403/connection drops.
    let ua = format!(
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36 envr-runtime-java/{}",
        env!("CARGO_PKG_VERSION")
    );
    build_blocking_http1_only_client(&ua, Some(Duration::from_secs(900)))
}

fn fetch_text(client: &reqwest::blocking::Client, url: &str) -> EnvrResult<String> {
    let r = client.get(url).send().map_err(|e| {
        EnvrError::with_source(ErrorCode::Download, format!("request failed for {url}"), e)
    })?;
    if !r.status().is_success() {
        return Err(EnvrError::Download(format!("GET {url} -> {}", r.status())));
    }
    r.text().map_err(|e| {
        EnvrError::with_source(
            ErrorCode::Download,
            format!("read body failed for {url}"),
            e,
        )
    })
}

#[allow(dead_code)]
fn url_exists(client: &reqwest::blocking::Client, url: &str) -> bool {
    match client.head(url).send() {
        Ok(r) => r.status().is_success(),
        Err(_) => false,
    }
}

#[allow(dead_code)]
fn zulu_latest_package_download_url(
    client: &reqwest::blocking::Client,
    major: u32,
    os: &str,
    arch: &str,
) -> Option<String> {
    #[derive(Deserialize)]
    struct ZuluPkg {
        download_url: String,
    }
    let azul_os = match os {
        "windows" => "windows",
        "linux" => "linux",
        "mac" => "macos",
        _ => return None,
    };
    let azul_arch = match arch {
        "x64" => "x86-64",
        "aarch64" => "arm64",
        "x32" => "x86",
        _ => return None,
    };
    let url = format!(
        "https://api.azul.com/metadata/v1/zulu/packages/?java_version={major}&os={azul_os}&arch={azul_arch}&archive_type=zip&java_package_type=jdk&release_status=ga&availability_types=ca&page=1&page_size=1"
    );
    let body = fetch_text(client, &url).ok()?;
    let parsed: Vec<ZuluPkg> = serde_json::from_str(&body).ok()?;
    parsed.first().map(|p| p.download_url.clone())
}

#[allow(dead_code)]
fn vendor_latest_binary_url(
    vendor: JavaVendor,
    major: u32,
    os: &str,
    arch: &str,
) -> Option<String> {
    match vendor {
        JavaVendor::EclipseTemurin | JavaVendor::OpenJdk => Some(format!(
            "{}/v3/binary/latest/{major}/ga/{os}/{arch}/jdk/hotspot/normal/eclipse",
            DEFAULT_ADOPTIUM_API_BASE
        )),
        JavaVendor::OracleOpenJdk => {
            if os != "windows" {
                return None;
            }
            Some(format!(
                "https://download.java.net/java/GA/jdk{major}/latest/GPL/openjdk-{major}_windows-{arch}_bin.zip"
            ))
        }
        JavaVendor::AmazonCorretto => {
            if os != "windows" {
                return None;
            }
            Some(format!(
                "https://corretto.aws/downloads/latest/amazon-corretto-{major}-{arch}-windows-jdk.zip"
            ))
        }
        JavaVendor::Microsoft => {
            if os != "windows" {
                return None;
            }
            Some(format!(
                "https://aka.ms/download-jdk/microsoft-jdk-{major}-windows-{arch}.zip"
            ))
        }
        JavaVendor::OracleJdk => {
            if os != "windows" {
                return None;
            }
            Some(format!(
                "https://download.oracle.com/java/{major}/latest/jdk-{major}_windows-{arch}_bin.zip"
            ))
        }
        JavaVendor::AzulZulu | JavaVendor::AlibabaDragonwell => None,
    }
}

#[allow(dead_code)]
fn synthetic_lts_entries_via_binary_latest(
    client: &reqwest::blocking::Client,
    vendor: JavaVendor,
    os: &str,
    arch: &str,
    lts_majors: &[u32],
) -> Vec<JavaVersionEntry> {
    fn supported(vendor: JavaVendor, major: u32) -> bool {
        static_lts_majors_for_vendor(vendor).contains(&major)
    }
    let mut out = Vec::new();
    for m in lts_majors {
        if !supported(vendor, *m) {
            continue;
        }
        let url = vendor_latest_binary_url(vendor, *m, os, arch);
        let Some(url) = url else {
            continue;
        };
        if !url_exists(client, &url) {
            continue;
        }
        out.push(JavaVersionEntry {
            build: 0,
            major: *m,
            minor: 0,
            security: 0,
            openjdk_version: m.to_string(),
            semver: m.to_string(),
            optional: Some("LTS".to_string()),
        });
    }
    out
}

fn static_lts_majors_for_vendor(vendor: JavaVendor) -> &'static [u32] {
    match vendor {
        JavaVendor::EclipseTemurin | JavaVendor::OpenJdk => &[8, 11, 17, 21, 25],
        JavaVendor::OracleOpenJdk => &[17, 21, 25],
        JavaVendor::AmazonCorretto => &[8, 11, 17, 21],
        JavaVendor::Microsoft => &[11, 17, 21, 25],
        JavaVendor::OracleJdk => &[21, 25],
        JavaVendor::AzulZulu | JavaVendor::AlibabaDragonwell => &[8, 11, 17, 21, 25],
    }
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
    let parsed: AvailableReleasesBody = serde_json::from_str(&body).map_err(|e| {
        EnvrError::with_source(ErrorCode::Validation, "invalid available_releases json", e)
    })?;
    Ok(parsed.available_lts_releases)
}

pub fn fetch_release_versions(
    client: &reqwest::blocking::Client,
    api_base: &str,
    vendor: JavaVendor,
    os: &str,
    arch: &str,
) -> EnvrResult<Vec<JavaVersionEntry>> {
    if matches!(vendor, JavaVendor::AzulZulu | JavaVendor::AlibabaDragonwell) {
        return Err(EnvrError::Validation(format!(
            "Java vendor {vendor:?} is not indexed via api.adoptium.net release_versions"
        )));
    }
    let base = api_base.trim_end_matches('/');
    let v = vendor.adoptium_vendor_param();
    let url = format!(
        "{base}/v3/info/release_versions?release_type=ga&vendor={v}\
         &heap_size=normal&image_type=jdk&jvm_impl=hotspot&os={os}&architecture={arch}\
         &page_size=1000"
    );
    let body = fetch_text(client, &url)?;
    let parsed: ReleaseVersionsBody = serde_json::from_str(&body).map_err(|e| {
        EnvrError::with_source(ErrorCode::Validation, "invalid release_versions json", e)
    })?;
    Ok(parsed.versions)
}

#[allow(dead_code)]
fn parse_openjdk_triplet(label: &str) -> Option<(u32, u32, u32)> {
    let t = label.trim().strip_suffix("-LTS").unwrap_or(label.trim());
    let core = t.split('+').next().unwrap_or(t);
    let mut it = core.split('.');
    let major = it.next()?.parse::<u32>().ok()?;
    let minor = it.next().and_then(|s| s.parse::<u32>().ok()).unwrap_or(0);
    let sec = it.next().and_then(|s| s.parse::<u32>().ok()).unwrap_or(0);
    Some((major, minor, sec))
}

#[allow(dead_code)]
fn fetch_latest_lts_versions_via_assets_latest(
    client: &reqwest::blocking::Client,
    api_base: &str,
    vendor: JavaVendor,
    os: &str,
    arch: &str,
    lts_majors: &[u32],
) -> EnvrResult<Vec<JavaVersionEntry>> {
    let base = api_base.trim_end_matches('/');
    let v = vendor.adoptium_vendor_param();
    let mut out = Vec::new();
    let mut last_err: Option<EnvrError> = None;
    for m in lts_majors {
        let url = format!(
            "{base}/v3/assets/latest/{m}/hotspot?vendor={v}&heap_size=normal&image_type=jdk&os={os}&architecture={arch}"
        );
        let body = match fetch_text(client, &url) {
            Ok(b) => b,
            Err(e) => {
                last_err = Some(e);
                continue;
            }
        };
        let parsed: Vec<LatestAssetsRelease> = match serde_json::from_str(&body).map_err(|e| {
            EnvrError::with_source(ErrorCode::Validation, "invalid latest assets json", e)
        }) {
            Ok(p) => p,
            Err(e) => {
                last_err = Some(e);
                continue;
            }
        };
        let Some(first) = parsed.first() else {
            continue;
        };
        let label = first.version_data.openjdk_version.clone();
        let Some((major, minor, security)) = parse_openjdk_triplet(&label) else {
            continue;
        };
        out.push(JavaVersionEntry {
            build: 0,
            major,
            minor,
            security,
            openjdk_version: label.clone(),
            semver: first.version_data.semver.clone().unwrap_or(label),
            optional: Some("LTS".to_string()),
        });
    }
    if out.is_empty() {
        return Err(last_err.unwrap_or_else(|| {
            EnvrError::Validation(
                "no LTS releases available for selected Java distribution on this platform".into(),
            )
        }));
    }
    Ok(out)
}

pub fn load_java_index(
    client: &reqwest::blocking::Client,
    api_base: &str,
    vendor: JavaVendor,
    host_os: &str,
    host_arch: &str,
) -> EnvrResult<JavaIndex> {
    let _ = client;
    let _ = api_base;
    let _ = host_os;
    let _ = host_arch;

    // Fast path: Java remote rows are rendered from a curated support matrix,
    // avoiding network on each page enter/tab switch.
    let lts_majors = static_lts_majors_for_vendor(vendor).to_vec();
    let mut versions = lts_majors
        .iter()
        .map(|m| JavaVersionEntry {
            build: 0,
            major: *m,
            minor: 0,
            security: 0,
            openjdk_version: m.to_string(),
            semver: m.to_string(),
            optional: Some("LTS".to_string()),
        })
        .collect::<Vec<_>>();
    versions.sort_by_key(|b| Reverse(version_sort_key(b)));
    Ok(JavaIndex {
        versions,
        lts_majors,
    })
}

/// Normalizes an Adoptium `openjdk_version` string for equality checks (trim, strip `-LTS`, lowercase).
pub fn normalize_openjdk_version_label(s: &str) -> String {
    let trimmed = s.trim();
    let no_lts = trimmed
        .strip_suffix("-LTS")
        .or_else(|| trimmed.strip_suffix("-lts"))
        .unwrap_or(trimmed)
        .trim();
    no_lts.to_ascii_lowercase()
}

/// Path segment for `GET /v3/assets/version/{segment}` (percent-encoded Maven range `[lo,hi)`).
pub fn adoptium_assets_version_range_segment(openjdk_version_label: &str) -> EnvrResult<String> {
    let trimmed = openjdk_version_label.trim();
    let no_lts = trimmed
        .strip_suffix("-LTS")
        .or_else(|| trimmed.strip_suffix("-lts"))
        .unwrap_or(trimmed)
        .trim();
    let (triplet, _) = no_lts.split_once('+').unwrap_or((no_lts, ""));
    let parts: Vec<&str> = triplet.split('.').collect();
    let (maj, min, sec) = match parts.as_slice() {
        [a] => (
            a.parse::<u32>().map_err(|_| {
                EnvrError::Validation(format!("bad java version: {openjdk_version_label}"))
            })?,
            0u32,
            0u32,
        ),
        [a, b] => (
            a.parse::<u32>().map_err(|_| {
                EnvrError::Validation(format!("bad java version: {openjdk_version_label}"))
            })?,
            b.parse::<u32>().map_err(|_| {
                EnvrError::Validation(format!("bad java version: {openjdk_version_label}"))
            })?,
            0u32,
        ),
        [a, b, c] => (
            a.parse::<u32>().map_err(|_| {
                EnvrError::Validation(format!("bad java version: {openjdk_version_label}"))
            })?,
            b.parse::<u32>().map_err(|_| {
                EnvrError::Validation(format!("bad java version: {openjdk_version_label}"))
            })?,
            c.parse::<u32>().map_err(|_| {
                EnvrError::Validation(format!("bad java version: {openjdk_version_label}"))
            })?,
        ),
        _ => {
            return Err(EnvrError::Validation(format!(
                "bad java version: {openjdk_version_label}"
            )));
        }
    };
    let lo = format!("{maj}.{min}.{sec}");
    let hi_sec = sec.checked_add(1).ok_or_else(|| {
        EnvrError::Validation(format!(
            "java version patch overflow: {openjdk_version_label}"
        ))
    })?;
    let hi = format!("{maj}.{min}.{hi_sec}");
    let raw = format!("[{lo},{hi})");
    Ok(percent_encode_adoptium_range(&raw))
}

fn percent_encode_adoptium_range(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '[' => out.push_str("%5B"),
            ']' => out.push_str("%5D"),
            '(' => out.push_str("%28"),
            ')' => out.push_str("%29"),
            ',' => out.push_str("%2C"),
            '+' => out.push_str("%2B"),
            ' ' => out.push_str("%20"),
            _ => out.push(c),
        }
    }
    out
}

pub fn find_version_entry<'a>(
    index: &'a JavaIndex,
    openjdk_version_label: &str,
) -> Option<&'a JavaVersionEntry> {
    let want = normalize_openjdk_version_label(openjdk_version_label);
    index.versions.iter().find(|e| {
        normalize_openjdk_version_label(&e.openjdk_version) == want
            || e.openjdk_version == openjdk_version_label
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
                ..Default::default()
            },
        )
        .expect("list");
        assert_eq!(list.len(), 1);
        assert!(list[0].0.starts_with("21"));
    }

    #[test]
    fn assets_range_segment() {
        assert_eq!(
            adoptium_assets_version_range_segment("24.0.2+12").expect("seg"),
            "%5B24.0.2%2C24.0.3%29"
        );
        assert_eq!(
            adoptium_assets_version_range_segment("25+36-LTS").expect("seg"),
            "%5B25.0.0%2C25.0.1%29"
        );
    }
}
