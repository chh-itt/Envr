use envr_domain::runtime::{
    RemoteFilter, RuntimeKind, RuntimeVersion, numeric_version_segments, version_line_key_for_kind,
};
use envr_download::blocking::build_blocking_http_client;
use envr_error::{EnvrError, EnvrResult, ErrorCode};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::HashSet;
use std::time::Duration;

pub const DEFAULT_FLUTTER_RELEASES_WINDOWS_URL: &str =
    "https://storage.googleapis.com/flutter_infra_release/releases/releases_windows.json";
pub const DEFAULT_FLUTTER_RELEASES_LINUX_URL: &str =
    "https://storage.googleapis.com/flutter_infra_release/releases/releases_linux.json";
pub const DEFAULT_FLUTTER_RELEASES_MACOS_URL: &str =
    "https://storage.googleapis.com/flutter_infra_release/releases/releases_macos.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlutterIndexRow {
    pub version: String,
    pub url: String,
    pub sha256: Option<String>,
    pub dart_sdk_version: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FlutterReleasesJson {
    base_url: String,
    releases: Vec<FlutterReleaseItem>,
}

#[derive(Debug, Deserialize)]
struct FlutterReleaseItem {
    channel: String,
    version: Option<String>,
    archive: String,
    sha256: Option<String>,
    dart_sdk_version: Option<String>,
}

pub fn blocking_http_client() -> EnvrResult<reqwest::blocking::Client> {
    build_blocking_http_client(
        concat!("envr-runtime-flutter/", env!("CARGO_PKG_VERSION")),
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

fn host_target() -> EnvrResult<&'static str> {
    use std::env::consts::{ARCH, OS};
    match (OS, ARCH) {
        ("windows", "x86_64") => Ok("windows-x64"),
        ("linux", "x86_64") => Ok("linux-x64"),
        ("macos", "x86_64") => Ok("macos-x64"),
        ("macos", "aarch64") => Ok("macos-arm64"),
        _ => Err(EnvrError::Validation(format!(
            "no Flutter SDK mapped for host {OS}-{ARCH}"
        ))),
    }
}

pub fn releases_json_url_for_host() -> EnvrResult<&'static str> {
    match host_target()? {
        "windows-x64" => Ok(DEFAULT_FLUTTER_RELEASES_WINDOWS_URL),
        "linux-x64" => Ok(DEFAULT_FLUTTER_RELEASES_LINUX_URL),
        "macos-x64" | "macos-arm64" => Ok(DEFAULT_FLUTTER_RELEASES_MACOS_URL),
        _ => Err(EnvrError::Validation("unsupported flutter host".into())),
    }
}

fn release_matches_host_archive(archive: &str, host: &str) -> bool {
    match host {
        "windows-x64" => {
            archive.contains("stable/windows/flutter_windows_") && archive.ends_with(".zip")
        }
        "linux-x64" => {
            archive.contains("stable/linux/flutter_linux_") && archive.ends_with(".tar.xz")
        }
        "macos-x64" => {
            archive.contains("stable/macos/flutter_macos_")
                && !archive.contains("flutter_macos_arm64_")
                && archive.ends_with(".zip")
        }
        "macos-arm64" => {
            archive.contains("stable/macos/flutter_macos_arm64_") && archive.ends_with(".zip")
        }
        _ => false,
    }
}

pub fn parse_rows_from_releases_json(json: &str) -> EnvrResult<Vec<FlutterIndexRow>> {
    let payload: FlutterReleasesJson = serde_json::from_str(json).map_err(|e| {
        EnvrError::with_source(ErrorCode::Validation, "invalid flutter releases json", e)
    })?;
    let host = host_target()?;
    let mut out = Vec::<FlutterIndexRow>::new();
    let mut seen = HashSet::<String>::new();
    for r in payload.releases {
        if r.channel != "stable" {
            continue;
        }
        let Some(version_raw) = r.version else {
            continue;
        };
        let version = version_raw.trim().to_string();
        if !version.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            continue;
        }
        if numeric_version_segments(&version).is_none_or(|p| p.len() < 2) {
            continue;
        }
        if !release_matches_host_archive(&r.archive, host) {
            continue;
        }
        if !seen.insert(version.clone()) {
            continue;
        }
        let base = payload.base_url.trim_end_matches('/');
        let rel = r.archive.trim_start_matches('/');
        out.push(FlutterIndexRow {
            version,
            url: format!("{base}/{rel}"),
            sha256: r.sha256,
            dart_sdk_version: r.dart_sdk_version,
        });
    }
    out.sort_by(|a, b| cmp_semver_desc(&a.version, &b.version));
    Ok(out)
}

pub fn list_remote_versions(
    rows: &[FlutterIndexRow],
    filter: &RemoteFilter,
) -> Vec<RuntimeVersion> {
    let mut labels: Vec<String> = rows.iter().map(|r| r.version.clone()).collect();
    if let Some(prefix) = filter.prefix.as_deref() {
        let p = prefix.trim();
        if !p.is_empty() {
            labels.retain(|v| v.starts_with(p));
        }
    }
    labels.into_iter().map(RuntimeVersion).collect()
}

pub fn list_remote_latest_per_major_lines(rows: &[FlutterIndexRow]) -> Vec<RuntimeVersion> {
    let mut seen = HashSet::<String>::new();
    let mut out = Vec::new();
    for r in rows {
        if let Some(line) = version_line_key_for_kind(RuntimeKind::Flutter, &r.version)
            && seen.insert(line)
        {
            out.push(RuntimeVersion(r.version.clone()));
        }
    }
    out
}

pub fn resolve_flutter_version(rows: &[FlutterIndexRow], spec: &str) -> EnvrResult<String> {
    let s = spec.trim().trim_start_matches('v').trim_start_matches('V');
    if s.is_empty() {
        return Err(EnvrError::Validation("empty flutter version spec".into()));
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
        "no flutter release matches spec `{s}`"
    )))
}
