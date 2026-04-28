use envr_domain::runtime::{
    RemoteFilter, RuntimeKind, RuntimeVersion, numeric_version_segments, version_line_key_for_kind,
};
use envr_download::blocking::build_blocking_http_client;
use envr_error::{EnvrError, EnvrResult, ErrorCode};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::HashSet;
use std::time::Duration;

pub const DEFAULT_DART_BUCKET_LIST_API_URL: &str = "https://storage.googleapis.com/storage/v1/b/dart-archive/o?prefix=channels/stable/release/&delimiter=/";
pub const DEFAULT_DART_LATEST_VERSION_URL: &str =
    "https://storage.googleapis.com/dart-archive/channels/stable/release/latest/VERSION";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DartIndexRow {
    pub version: String,
    pub url: String,
}

#[derive(Debug, Deserialize)]
struct DartBucketListResponse {
    #[serde(default)]
    prefixes: Vec<String>,
}

pub fn blocking_http_client() -> EnvrResult<reqwest::blocking::Client> {
    build_blocking_http_client(
        concat!("envr-runtime-dart/", env!("CARGO_PKG_VERSION")),
        Some(Duration::from_secs(120)),
    )
}

pub fn fetch_text(client: &reqwest::blocking::Client, url: &str) -> EnvrResult<String> {
    let response = client.get(url).send().map_err(|e| {
        EnvrError::with_source(ErrorCode::Download, format!("request failed for {url}"), e)
    })?;
    if !response.status().is_success() {
        return Err(EnvrError::Download(format!(
            "GET {url} -> {}",
            response.status()
        )));
    }
    response.text().map_err(|e| {
        EnvrError::with_source(
            ErrorCode::Download,
            format!("read body failed for {url}"),
            e,
        )
    })
}

fn cmp_semver_desc(a: &str, b: &str) -> Ordering {
    match (numeric_version_segments(a), numeric_version_segments(b)) {
        (Some(va), Some(vb)) => vb.cmp(&va),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => b.cmp(a),
    }
}

fn parse_version_from_prefix(prefix: &str) -> Option<String> {
    let p = prefix.trim().trim_end_matches('/');
    let marker = "channels/stable/release/";
    let i = p.find(marker)?;
    let v = &p[(i + marker.len())..];
    if v.is_empty() {
        return None;
    }
    if !v.chars().all(|c| c.is_ascii_digit() || c == '.') {
        return None;
    }
    let parts = numeric_version_segments(v)?;
    if parts.len() < 2 {
        return None;
    }
    Some(v.to_string())
}

pub fn dart_platform_tuple() -> EnvrResult<&'static str> {
    use std::env::consts::{ARCH, OS};
    match (OS, ARCH) {
        ("windows", "x86_64") => Ok("windows-x64"),
        ("windows", "aarch64") => Ok("windows-arm64"),
        ("linux", "x86_64") => Ok("linux-x64"),
        ("linux", "aarch64") => Ok("linux-arm64"),
        ("macos", "x86_64") => Ok("macos-x64"),
        ("macos", "aarch64") => Ok("macos-arm64"),
        _ => Err(EnvrError::Validation(format!(
            "no official Dart SDK mapped for host {OS}-{ARCH}"
        ))),
    }
}

pub fn artifact_url(version: &str, platform_tuple: &str) -> String {
    format!(
        "https://storage.googleapis.com/dart-archive/channels/stable/release/{version}/sdk/dartsdk-{platform_tuple}-release.zip"
    )
}

pub fn parse_rows_from_bucket_list_json(
    json: &str,
    platform_tuple: &str,
) -> EnvrResult<Vec<DartIndexRow>> {
    let payload: DartBucketListResponse = serde_json::from_str(json).map_err(|e| {
        EnvrError::with_source(ErrorCode::Validation, "invalid dart bucket list json", e)
    })?;
    let mut out = Vec::<DartIndexRow>::new();
    let mut seen = HashSet::<String>::new();
    for prefix in payload.prefixes {
        let Some(version) = parse_version_from_prefix(&prefix) else {
            continue;
        };
        if seen.insert(version.clone()) {
            out.push(DartIndexRow {
                url: artifact_url(&version, platform_tuple),
                version,
            });
        }
    }
    out.sort_by(|a, b| cmp_semver_desc(&a.version, &b.version));
    Ok(out)
}

pub fn list_remote_versions(rows: &[DartIndexRow], filter: &RemoteFilter) -> Vec<RuntimeVersion> {
    let mut labels: Vec<String> = rows.iter().map(|r| r.version.clone()).collect();
    if let Some(prefix) = filter.prefix.as_deref() {
        let p = prefix.trim();
        if !p.is_empty() {
            labels.retain(|v| v.starts_with(p));
        }
    }
    labels.into_iter().map(RuntimeVersion).collect()
}

pub fn list_remote_latest_per_major_lines(rows: &[DartIndexRow]) -> Vec<RuntimeVersion> {
    let mut seen = HashSet::<String>::new();
    let mut out = Vec::new();
    for r in rows {
        if let Some(line) = version_line_key_for_kind(RuntimeKind::Dart, &r.version)
            && seen.insert(line)
        {
            out.push(RuntimeVersion(r.version.clone()));
        }
    }
    out
}

pub fn resolve_dart_version(rows: &[DartIndexRow], spec: &str) -> EnvrResult<String> {
    let s = spec.trim().trim_start_matches('v').trim_start_matches('V');
    if s.is_empty() {
        return Err(EnvrError::Validation("empty dart version spec".into()));
    }
    let labels: Vec<&str> = rows.iter().map(|r| r.version.as_str()).collect();
    if labels.contains(&s) {
        return Ok(s.to_string());
    }
    if let Some(parts) = numeric_version_segments(s) {
        match parts.len() {
            1 => {
                let major = parts[0];
                if let Some(best) = rows.iter().map(|r| r.version.as_str()).find(|v| {
                    numeric_version_segments(v).is_some_and(|p| !p.is_empty() && p[0] == major)
                }) {
                    return Ok(best.to_string());
                }
            }
            2 => {
                let major = parts[0];
                let minor = parts[1];
                if let Some(best) = rows.iter().map(|r| r.version.as_str()).find(|v| {
                    numeric_version_segments(v)
                        .is_some_and(|p| p.len() >= 2 && p[0] == major && p[1] == minor)
                }) {
                    return Ok(best.to_string());
                }
            }
            _ => {}
        }
    }
    Err(EnvrError::Validation(format!(
        "no dart release matches spec `{s}`"
    )))
}
