use envr_domain::runtime::{
    RemoteFilter, RuntimeKind, RuntimeVersion, numeric_version_segments, version_line_key_for_kind,
};
use envr_download::blocking::build_blocking_http_client;
use envr_error::EnvrResult;
use envr_runtime_github_release::GhRepo;
pub use envr_runtime_github_release::{GhAsset, GhRelease};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::sync::LazyLock;
use std::time::Duration;

pub const DEFAULT_ELM_RELEASES_API_URL: &str = "https://api.github.com/repos/elm/compiler/releases";
const ELM_REPO: GhRepo = GhRepo {
    owner: "elm",
    name: "compiler",
};
static TAG_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^v?(\d+\.\d+\.\d+)$").expect("elm tag regex"));

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElmInstallableRow {
    pub version: String,
    pub url: String,
}

pub fn blocking_http_client() -> EnvrResult<reqwest::blocking::Client> {
    build_blocking_http_client(
        concat!("envr-runtime-elm/", env!("CARGO_PKG_VERSION")),
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
fn elm_asset_candidates() -> Vec<&'static str> {
    use std::env::consts::{ARCH, OS};
    match (OS, ARCH) {
        ("windows", "x86_64") => vec!["binary-for-windows-64-bit.gz"],
        ("linux", "x86_64") => vec!["binary-for-linux-64-bit.gz"],
        ("macos", "x86_64") => vec!["binary-for-mac-64-bit.gz"],
        ("macos", "aarch64") => vec!["binary-for-mac-64-bit-ARM.gz", "binary-for-mac-64-bit.gz"],
        _ => vec![],
    }
}
fn pick_asset<'a>(assets: &'a [GhAsset]) -> Option<&'a GhAsset> {
    let cands = elm_asset_candidates();
    assets.iter().find(|a| cands.iter().any(|n| a.name == *n))
}
pub fn installable_rows_from_releases(releases: &[GhRelease]) -> Vec<ElmInstallableRow> {
    envr_runtime_github_release::installable_rows_from_releases(
        releases,
        false,
        label_from_tag,
        |assets| pick_asset(assets).map(|a| a.browser_download_url.clone()),
        cmp_release_labels,
    )
    .into_iter()
    .map(|r| ElmInstallableRow {
        version: r.version,
        url: r.url,
    })
    .collect()
}
pub fn fetch_elm_github_releases_index(
    client: &reqwest::blocking::Client,
    releases_api_url: &str,
) -> EnvrResult<Vec<GhRelease>> {
    envr_runtime_github_release::fetch_github_releases_index(
        client,
        releases_api_url,
        DEFAULT_ELM_RELEASES_API_URL,
    )
}

fn make_synthetic_url(tag: &str, _version: &str) -> Option<String> {
    let asset = elm_asset_candidates().first().copied()?;
    Some(format!(
        "https://github.com/elm/compiler/releases/download/{tag}/{asset}"
    ))
}
pub fn fetch_elm_installable_rows_with_fallback(
    client: &reqwest::blocking::Client,
    releases_api_url: &str,
) -> EnvrResult<Vec<ElmInstallableRow>> {
    if let Ok(releases) = fetch_elm_github_releases_index(client, releases_api_url) {
        let rows = installable_rows_from_releases(&releases);
        if !rows.is_empty() {
            return Ok(rows);
        }
    }
    if let Ok(rows) = envr_runtime_github_release::fetch_rows_via_html(
        client,
        ELM_REPO,
        label_from_tag,
        make_synthetic_url,
        cmp_release_labels,
    ) && !rows.is_empty()
    {
        return Ok(rows
            .into_iter()
            .map(|r| ElmInstallableRow {
                version: r.version,
                url: r.url,
            })
            .collect());
    }
    let rows = envr_runtime_github_release::fetch_rows_via_atom(
        client,
        ELM_REPO,
        label_from_tag,
        make_synthetic_url,
        cmp_release_labels,
    )?;
    Ok(rows
        .into_iter()
        .map(|r| ElmInstallableRow {
            version: r.version,
            url: r.url,
        })
        .collect())
}
pub fn list_remote_versions(
    rows: &[ElmInstallableRow],
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
pub fn list_remote_latest_per_major_lines(rows: &[ElmInstallableRow]) -> Vec<RuntimeVersion> {
    let mut best: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for r in rows {
        let Some(line) = version_line_key_for_kind(RuntimeKind::Elm, &r.version) else {
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
pub fn resolve_elm_version(rows: &[ElmInstallableRow], spec: &str) -> Option<String> {
    let t = spec.trim();
    if t.is_empty() {
        return None;
    }
    if let Some(v) = label_from_tag(t) {
        return Some(v);
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
