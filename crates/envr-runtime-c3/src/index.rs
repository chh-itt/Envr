use envr_domain::runtime::{
    RemoteFilter, RuntimeKind, RuntimeVersion, numeric_version_segments, version_line_key_for_kind,
};
use envr_download::blocking::build_blocking_http_client;
use envr_error::EnvrResult;
use envr_runtime_github_release::{GhAsset, GhRelease, GhRepo};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::sync::LazyLock;
use std::time::Duration;

pub const DEFAULT_C3_RELEASES_API_URL: &str = "https://api.github.com/repos/c3lang/c3c/releases";
const C3_REPO: GhRepo = GhRepo {
    owner: "c3lang",
    name: "c3c",
};
static TAG_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^v?(\d+\.\d+\.\d+)$").expect("c3 tag regex"));

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct C3InstallableRow {
    pub version: String,
    pub url: String,
}

pub fn blocking_http_client() -> EnvrResult<reqwest::blocking::Client> {
    build_blocking_http_client(
        concat!("envr-runtime-c3/", env!("CARGO_PKG_VERSION")),
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

fn label_from_tag(tag: &str) -> Option<String> {
    TAG_RE
        .captures(tag.trim())
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
}

fn c3_primary_asset_filename() -> Option<&'static str> {
    use std::env::consts::{ARCH, OS};
    match (OS, ARCH) {
        ("windows", "x86_64") => Some("c3-windows.zip"),
        ("linux", "x86_64") => Some("c3-linux.tar.gz"),
        ("macos", "aarch64") => Some("c3-macos.zip"),
        _ => None,
    }
}

fn pick_asset(assets: &[GhAsset]) -> Option<&GhAsset> {
    let expected = c3_primary_asset_filename()?;
    assets.iter().find(|a| a.name == expected)
}

fn installable_rows_from_releases(releases: &[GhRelease]) -> Vec<C3InstallableRow> {
    envr_runtime_github_release::installable_rows_from_releases(
        releases,
        true,
        label_from_tag,
        |assets| pick_asset(assets).map(|a| a.browser_download_url.clone()),
        cmp_release_labels,
    )
    .into_iter()
    .map(|r| C3InstallableRow {
        version: r.version,
        url: r.url,
    })
    .collect()
}

fn fetch_github_releases_index(
    client: &reqwest::blocking::Client,
    releases_api_url: &str,
) -> EnvrResult<Vec<GhRelease>> {
    envr_runtime_github_release::fetch_github_releases_index(
        client,
        releases_api_url,
        DEFAULT_C3_RELEASES_API_URL,
    )
}

fn make_synthetic_url(tag: &str, _version: &str) -> Option<String> {
    let fname = c3_primary_asset_filename()?;
    Some(format!(
        "https://github.com/c3lang/c3c/releases/download/{tag}/{fname}"
    ))
}

pub fn fetch_c3_installable_rows_with_fallback(
    client: &reqwest::blocking::Client,
    releases_api_url: &str,
) -> EnvrResult<Vec<C3InstallableRow>> {
    if let Ok(releases) = fetch_github_releases_index(client, releases_api_url) {
        let rows = installable_rows_from_releases(&releases);
        if !rows.is_empty() {
            return Ok(rows);
        }
    }
    if let Ok(rows) = envr_runtime_github_release::fetch_rows_via_html(
        client,
        C3_REPO,
        label_from_tag,
        make_synthetic_url,
        cmp_release_labels,
    ) && !rows.is_empty()
    {
        return Ok(rows
            .into_iter()
            .map(|r| C3InstallableRow {
                version: r.version,
                url: r.url,
            })
            .collect());
    }
    let rows = envr_runtime_github_release::fetch_rows_via_atom(
        client,
        C3_REPO,
        label_from_tag,
        make_synthetic_url,
        cmp_release_labels,
    )?;
    Ok(rows
        .into_iter()
        .map(|r| C3InstallableRow {
            version: r.version,
            url: r.url,
        })
        .collect())
}

pub fn list_remote_versions(
    rows: &[C3InstallableRow],
    filter: &RemoteFilter,
) -> Vec<RuntimeVersion> {
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

pub fn list_remote_latest_per_major_lines(rows: &[C3InstallableRow]) -> Vec<RuntimeVersion> {
    let mut best: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for r in rows {
        let Some(line) = version_line_key_for_kind(RuntimeKind::C3, &r.version) else {
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

pub fn resolve_c3_version(rows: &[C3InstallableRow], spec: &str) -> Option<String> {
    let t = spec.trim().trim_start_matches('v');
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
