use envr_domain::runtime::{
    RemoteFilter, RuntimeKind, RuntimeVersion, numeric_version_segments, version_line_key_for_kind,
};
use envr_download::blocking::build_blocking_http_client;
use envr_error::{EnvrError, EnvrResult, ErrorCode};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::sync::LazyLock;
use std::time::Duration;

pub const DEFAULT_RACKET_ALL_VERSIONS_URL: &str = "https://download.racket-lang.org/all-versions.html";
static VERSION_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"Version\s+(\d+(?:\.\d+){0,2})").expect("racket version regex"));

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RacketInstallableRow {
    pub version: String,
    pub url: String,
}

pub fn blocking_http_client() -> EnvrResult<reqwest::blocking::Client> {
    build_blocking_http_client(
        concat!("envr-runtime-racket/", env!("CARGO_PKG_VERSION")),
        Some(Duration::from_secs(120)),
    )
}

fn cmp_release_labels(a: &str, b: &str) -> Ordering {
    match (numeric_version_segments(a), numeric_version_segments(b)) {
        (Some(va), Some(vb)) => va.cmp(&vb),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => a.cmp(b),
    }
}

fn host_asset_name(version: &str) -> Option<String> {
    use std::env::consts::{ARCH, OS};
    match (OS, ARCH) {
        ("windows", "x86_64") => Some(format!("racket-minimal-{version}-x86_64-win32-cs.tgz")),
        _ => None,
    }
}

pub fn fetch_racket_installable_rows(
    client: &reqwest::blocking::Client,
    all_versions_url: &str,
) -> EnvrResult<Vec<RacketInstallableRow>> {
    let text = client
        .get(all_versions_url)
        .send()
        .map_err(|e| {
            EnvrError::with_source(
                ErrorCode::Download,
                format!("request failed for {all_versions_url}"),
                e,
            )
        })?
        .text()
        .map_err(|e| {
            EnvrError::with_source(
                ErrorCode::Download,
                format!("read body failed for {all_versions_url}"),
                e,
            )
        })?;
    let mut out = Vec::new();
    for cap in VERSION_RE.captures_iter(&text) {
        let version = cap.get(1).map(|m| m.as_str().trim().to_string()).unwrap_or_default();
        if version.is_empty() {
            continue;
        }
        let Some(asset) = host_asset_name(&version) else {
            continue;
        };
        out.push(RacketInstallableRow {
            version: version.clone(),
            url: format!("https://download.racket-lang.org/releases/{version}/installers/{asset}"),
        });
    }
    out.sort_by(|a, b| cmp_release_labels(&a.version, &b.version));
    out.dedup_by(|a, b| a.version == b.version);
    Ok(out)
}

pub fn list_remote_versions(rows: &[RacketInstallableRow], filter: &RemoteFilter) -> Vec<RuntimeVersion> {
    let mut out: Vec<RuntimeVersion> = rows
        .iter()
        .filter(|r| {
            filter
                .prefix
                .as_ref()
                .map(|p| r.version.starts_with(p))
                .unwrap_or(true)
        })
        .map(|r| RuntimeVersion(r.version.clone()))
        .collect();
    out.sort_by(|a, b| cmp_release_labels(&a.0, &b.0));
    out.dedup_by(|a, b| a.0 == b.0);
    out
}

pub fn list_remote_latest_per_major_lines(rows: &[RacketInstallableRow]) -> Vec<RuntimeVersion> {
    let mut best: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for r in rows {
        let Some(line) = version_line_key_for_kind(RuntimeKind::Racket, &r.version) else {
            continue;
        };
        match best.get(&line) {
            None => {
                best.insert(line, r.version.clone());
            }
            Some(prev) => {
                if cmp_release_labels(prev, &r.version) == Ordering::Less {
                    best.insert(line, r.version.clone());
                }
            }
        }
    }
    let mut out: Vec<RuntimeVersion> = best.values().cloned().map(RuntimeVersion).collect();
    out.sort_by(|a, b| cmp_release_labels(&a.0, &b.0));
    out
}

pub fn resolve_racket_version(rows: &[RacketInstallableRow], spec: &str) -> Option<String> {
    let t = spec.trim();
    if t.is_empty() {
        return None;
    }
    if rows.iter().any(|r| r.version == t) {
        return Some(t.to_string());
    }
    if t.chars().all(|c| c.is_ascii_digit() || c == '.') {
        let prefix = format!("{t}.");
        let mut matches: Vec<&str> = rows
            .iter()
            .map(|r| r.version.as_str())
            .filter(|v| *v == t || v.starts_with(&prefix))
            .collect();
        matches.sort_by(|a, b| cmp_release_labels(a, b));
        if let Some(best) = matches.last() {
            return Some((*best).to_string());
        }
    }
    None
}

