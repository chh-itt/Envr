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

pub const DEFAULT_BABASHKA_RELEASES_API_URL: &str =
    "https://api.github.com/repos/babashka/babashka/releases";
const BABASHKA_REPO: GhRepo = GhRepo {
    owner: "babashka",
    name: "babashka",
};
static TAG_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^v?(\d+\.\d+\.\d+)$").expect("bb tag regex"));

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BabashkaInstallableRow {
    pub version: String,
    pub url: String,
}

pub fn blocking_http_client() -> EnvrResult<reqwest::blocking::Client> {
    build_blocking_http_client(
        concat!("envr-runtime-babashka/", env!("CARGO_PKG_VERSION")),
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

fn asset_priority(name: &str) -> Option<u8> {
    use std::env::consts::{ARCH, OS};
    match (OS, ARCH) {
        ("windows", "x86_64") => {
            if name.ends_with("-windows-amd64.zip") {
                Some(0)
            } else {
                None
            }
        }
        ("linux", "x86_64") => {
            if name.ends_with("-linux-amd64-static.tar.gz") {
                Some(0)
            } else if name.ends_with("-linux-amd64.tar.gz") {
                Some(1)
            } else {
                None
            }
        }
        ("linux", "aarch64") => {
            if name.ends_with("-linux-aarch64-static.tar.gz") {
                Some(0)
            } else {
                None
            }
        }
        ("macos", "x86_64") => {
            if name.ends_with("-macos-amd64.tar.gz") {
                Some(0)
            } else {
                None
            }
        }
        ("macos", "aarch64") => {
            if name.ends_with("-macos-aarch64.tar.gz") {
                Some(0)
            } else {
                None
            }
        }
        _ => None,
    }
}

fn pick_asset(assets: &[GhAsset]) -> Option<&GhAsset> {
    assets
        .iter()
        .filter_map(|a| asset_priority(&a.name).map(|p| (p, a)))
        .min_by_key(|(p, _)| *p)
        .map(|(_, a)| a)
}

fn installable_rows_from_releases(releases: &[GhRelease]) -> Vec<BabashkaInstallableRow> {
    envr_runtime_github_release::installable_rows_from_releases(
        releases,
        true,
        label_from_tag,
        |assets| pick_asset(assets).map(|a| a.browser_download_url.clone()),
        cmp_release_labels,
    )
    .into_iter()
    .map(|r| BabashkaInstallableRow {
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
        DEFAULT_BABASHKA_RELEASES_API_URL,
    )
}

fn make_synthetic_url(tag: &str, version: &str) -> Option<String> {
    use std::env::consts::{ARCH, OS};
    let file = match (OS, ARCH) {
        ("windows", "x86_64") => format!("babashka-{version}-windows-amd64.zip"),
        ("linux", "x86_64") => format!("babashka-{version}-linux-amd64-static.tar.gz"),
        ("linux", "aarch64") => format!("babashka-{version}-linux-aarch64-static.tar.gz"),
        ("macos", "x86_64") => format!("babashka-{version}-macos-amd64.tar.gz"),
        ("macos", "aarch64") => format!("babashka-{version}-macos-aarch64.tar.gz"),
        _ => return None,
    };
    Some(format!(
        "https://github.com/babashka/babashka/releases/download/{tag}/{file}"
    ))
}

pub fn fetch_babashka_installable_rows_with_fallback(
    client: &reqwest::blocking::Client,
    releases_api_url: &str,
) -> EnvrResult<Vec<BabashkaInstallableRow>> {
    if let Ok(releases) = fetch_github_releases_index(client, releases_api_url) {
        let rows = installable_rows_from_releases(&releases);
        if !rows.is_empty() {
            return Ok(rows);
        }
    }
    if let Ok(rows) = envr_runtime_github_release::fetch_rows_via_html(
        client,
        BABASHKA_REPO,
        label_from_tag,
        make_synthetic_url,
        cmp_release_labels,
    ) && !rows.is_empty()
    {
        return Ok(rows
            .into_iter()
            .map(|r| BabashkaInstallableRow {
                version: r.version,
                url: r.url,
            })
            .collect());
    }
    let rows = envr_runtime_github_release::fetch_rows_via_atom(
        client,
        BABASHKA_REPO,
        label_from_tag,
        make_synthetic_url,
        cmp_release_labels,
    )?;
    Ok(rows
        .into_iter()
        .map(|r| BabashkaInstallableRow {
            version: r.version,
            url: r.url,
        })
        .collect())
}

pub fn list_remote_versions(
    rows: &[BabashkaInstallableRow],
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

pub fn list_remote_latest_per_major_lines(rows: &[BabashkaInstallableRow]) -> Vec<RuntimeVersion> {
    let mut best: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for r in rows {
        let Some(line) = version_line_key_for_kind(RuntimeKind::Babashka, &r.version) else {
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

pub fn resolve_babashka_version(rows: &[BabashkaInstallableRow], spec: &str) -> Option<String> {
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
