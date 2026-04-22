use envr_domain::runtime::{
    RemoteFilter, RuntimeKind, RuntimeVersion, numeric_version_segments, version_line_key_for_kind,
};
use envr_error::{EnvrError, EnvrResult};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::cmp::Ordering;
use std::collections::HashSet;
use std::sync::LazyLock;
use std::time::Duration;

pub const DEFAULT_BABASHKA_RELEASES_API_URL: &str =
    "https://api.github.com/repos/babashka/babashka/releases";
const BABASHKA_RELEASES_ATOM_URL: &str = "https://github.com/babashka/babashka/releases.atom";

static ATOM_RELEASE_TAG_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"https://github\.com/babashka/babashka/releases/tag/([^"<>]+)"#)
        .expect("bb atom release tag regex")
});
static HTML_RELEASE_TAG_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"/babashka/babashka/releases/tag/([^"<>/]+)"#).expect("bb html release tag regex")
});
static TAG_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^v?(\d+\.\d+\.\d+)$").expect("bb tag regex"));

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GhAsset {
    pub name: String,
    pub browser_download_url: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GhRelease {
    pub tag_name: String,
    #[serde(default)]
    pub draft: bool,
    #[serde(default)]
    pub prerelease: bool,
    pub assets: Vec<GhAsset>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BabashkaInstallableRow {
    pub version: String,
    pub url: String,
}

pub fn blocking_http_client() -> EnvrResult<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(120))
        .user_agent(concat!("envr-runtime-babashka/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| EnvrError::Download(e.to_string()))
}

fn github_api_auth_token() -> Option<String> {
    ["GITHUB_TOKEN", "GH_TOKEN", "ENVR_GITHUB_TOKEN"]
        .into_iter()
        .find_map(|k| std::env::var(k).ok())
        .and_then(|s| {
            let t = s.trim();
            if t.is_empty() {
                None
            } else {
                Some(t.to_string())
            }
        })
}

fn fetch_text(client: &reqwest::blocking::Client, url: &str) -> EnvrResult<String> {
    let mut req = client.get(url).header("Accept", "application/vnd.github+json");
    if url.contains("api.github.com") {
        req = req.header("X-GitHub-Api-Version", "2022-11-28");
        if let Some(tok) = github_api_auth_token() {
            req = req.header("Authorization", format!("Bearer {tok}"));
        }
    }
    let response = req.send().map_err(|e| EnvrError::Download(e.to_string()))?;
    if !response.status().is_success() {
        return Err(EnvrError::Download(format!("GET {url} -> {}", response.status())));
    }
    response.text().map_err(|e| EnvrError::Download(e.to_string()))
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

fn pick_asset<'a>(assets: &'a [GhAsset]) -> Option<&'a GhAsset> {
    assets
        .iter()
        .filter_map(|a| asset_priority(&a.name).map(|p| (p, a)))
        .min_by_key(|(p, _)| *p)
        .map(|(_, a)| a)
}

fn installable_rows_from_releases(releases: &[GhRelease]) -> Vec<BabashkaInstallableRow> {
    let mut out = Vec::new();
    for rel in releases {
        if rel.draft {
            continue;
        }
        let Some(version) = label_from_tag(&rel.tag_name) else {
            continue;
        };
        let Some(asset) = pick_asset(&rel.assets) else {
            continue;
        };
        out.push(BabashkaInstallableRow {
            version,
            url: asset.browser_download_url.clone(),
        });
    }
    out.sort_by(|a, b| cmp_release_labels(&a.version, &b.version));
    out.dedup_by(|a, b| a.version == b.version);
    out
}

fn fetch_github_releases_index(
    client: &reqwest::blocking::Client,
    releases_api_url: &str,
) -> EnvrResult<Vec<GhRelease>> {
    let mut page = 1;
    let mut out = Vec::new();
    loop {
        let url = format!("{releases_api_url}?per_page=100&page={page}");
        let text = fetch_text(client, &url)?;
        let v: Value =
            serde_json::from_str(&text).map_err(|e| EnvrError::Download(e.to_string()))?;
        let Some(arr) = v.as_array() else {
            return Err(EnvrError::Download(
                "babashka releases API returned non-array payload".into(),
            ));
        };
        if arr.is_empty() {
            break;
        }
        for item in arr {
            let r: GhRelease =
                serde_json::from_value(item.clone()).map_err(|e| EnvrError::Download(e.to_string()))?;
            out.push(r);
        }
        if arr.len() < 100 {
            break;
        }
        page += 1;
    }
    Ok(out)
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

fn fetch_rows_via_html(client: &reqwest::blocking::Client) -> EnvrResult<Vec<BabashkaInstallableRow>> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    let mut empty_pages = 0usize;
    for page in 1..=30 {
        let url = format!("https://github.com/babashka/babashka/releases?page={page}");
        let text = match fetch_text(client, &url) {
            Ok(t) => t,
            Err(_) => break,
        };
        let mut found = 0usize;
        for cap in HTML_RELEASE_TAG_RE.captures_iter(&text) {
            let tag = cap.get(1).map(|m| m.as_str()).unwrap_or("").trim();
            let Some(version) = label_from_tag(tag) else {
                continue;
            };
            found += 1;
            if !seen.insert(version.clone()) {
                continue;
            }
            let Some(url) = make_synthetic_url(tag, &version) else {
                continue;
            };
            out.push(BabashkaInstallableRow { version, url });
        }
        if found == 0 {
            empty_pages += 1;
            if empty_pages >= 2 {
                break;
            }
        } else {
            empty_pages = 0;
        }
    }
    out.sort_by(|a, b| cmp_release_labels(&a.version, &b.version));
    out.dedup_by(|a, b| a.version == b.version);
    Ok(out)
}

fn fetch_rows_via_atom(client: &reqwest::blocking::Client) -> EnvrResult<Vec<BabashkaInstallableRow>> {
    let mut out = Vec::new();
    let text = fetch_text(client, BABASHKA_RELEASES_ATOM_URL)?;
    let mut seen = HashSet::new();
    for cap in ATOM_RELEASE_TAG_RE.captures_iter(&text) {
        let tag = cap.get(1).map(|m| m.as_str()).unwrap_or("").trim();
        let Some(version) = label_from_tag(tag) else {
            continue;
        };
        if !seen.insert(version.clone()) {
            continue;
        }
        let Some(url) = make_synthetic_url(tag, &version) else {
            continue;
        };
        out.push(BabashkaInstallableRow { version, url });
    }
    out.sort_by(|a, b| cmp_release_labels(&a.version, &b.version));
    Ok(out)
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
    if let Ok(rows) = fetch_rows_via_html(client) && !rows.is_empty() {
        return Ok(rows);
    }
    fetch_rows_via_atom(client)
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

