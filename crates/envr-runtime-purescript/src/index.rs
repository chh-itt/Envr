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

pub const DEFAULT_PURESCRIPT_RELEASES_API_URL: &str =
    "https://api.github.com/repos/purescript/purescript/releases";
const PURESCRIPT_RELEASES_ATOM_URL: &str = "https://github.com/purescript/purescript/releases.atom";

static ATOM_RELEASE_TAG_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"https://github\.com/purescript/purescript/releases/tag/([^"<>]+)"#)
        .expect("purescript atom release tag regex")
});
static RELEASES_PAGE_TAG_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"/purescript/purescript/releases/tag/([^"<>/]+)"#)
        .expect("purescript releases page tag regex")
});
static TAG_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^v?(\d+\.\d+\.\d+)$").expect("purescript tag regex"));

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
pub struct PurescriptInstallableRow {
    pub version: String,
    pub url: String,
}

pub fn blocking_http_client() -> EnvrResult<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(120))
        .user_agent(concat!("envr-runtime-purescript/", env!("CARGO_PKG_VERSION")))
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

fn url_is_github_api(url: &str) -> bool {
    url.contains("api.github.com")
}

fn fetch_text(client: &reqwest::blocking::Client, url: &str) -> EnvrResult<String> {
    let mut req = client.get(url).header("Accept", "application/vnd.github+json");
    if url_is_github_api(url) {
        req = req.header("X-GitHub-Api-Version", "2022-11-28");
        if let Some(tok) = github_api_auth_token() {
            req = req.header("Authorization", format!("Bearer {tok}"));
        }
    }
    let response = req.send().map_err(|e| EnvrError::Download(e.to_string()))?;
    if !response.status().is_success() {
        return Err(EnvrError::Download(format!(
            "GET {url} -> {}",
            response.status()
        )));
    }
    response
        .text()
        .map_err(|e| EnvrError::Download(e.to_string()))
}

fn strip_known_github_api_proxy_prefix(url: &str) -> Option<String> {
    let u = url.trim();
    const NEEDLE: &str = "https://api.github.com/";
    let i = u.find(NEEDLE)?;
    Some(u[i..].to_string())
}

fn candidate_api_bases(primary: &str, default_url: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut push = |s: &str| {
        let t = s.trim();
        if t.is_empty() {
            return;
        }
        if !out.iter().any(|x| x == t) {
            out.push(t.to_string());
        }
    };
    push(primary);
    if let Some(inner) = strip_known_github_api_proxy_prefix(primary) {
        push(&inner);
    }
    push(default_url);
    out
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

pub fn purescript_asset_candidates() -> Vec<&'static str> {
    use std::env::consts::{ARCH, OS};
    match (OS, ARCH) {
        ("windows", "x86_64") => vec!["win64.tar.gz"],
        ("linux", "x86_64") => vec!["linux64.tar.gz"],
        ("linux", "aarch64") => vec!["linux-arm64.tar.gz", "linux64.tar.gz"],
        ("macos", "x86_64") => vec!["macos.tar.gz"],
        ("macos", "aarch64") => vec!["macos-arm64.tar.gz", "macos.tar.gz"],
        _ => vec![],
    }
}

fn pick_asset<'a>(assets: &'a [GhAsset]) -> Option<&'a GhAsset> {
    let cands = purescript_asset_candidates();
    assets.iter().find(|a| cands.iter().any(|n| a.name == *n))
}

pub fn installable_rows_from_releases(releases: &[GhRelease]) -> Vec<PurescriptInstallableRow> {
    let mut out = Vec::new();
    for rel in releases {
        if rel.draft || rel.prerelease {
            continue;
        }
        let Some(version) = label_from_tag(&rel.tag_name) else {
            continue;
        };
        let Some(asset) = pick_asset(&rel.assets) else {
            continue;
        };
        out.push(PurescriptInstallableRow {
            version,
            url: asset.browser_download_url.clone(),
        });
    }
    out.sort_by(|a, b| cmp_release_labels(&a.version, &b.version));
    out.dedup_by(|a, b| a.version == b.version);
    out
}

pub fn fetch_purescript_github_releases_index(
    client: &reqwest::blocking::Client,
    releases_api_url: &str,
) -> EnvrResult<Vec<GhRelease>> {
    let mut all = Vec::new();
    for base in candidate_api_bases(releases_api_url, DEFAULT_PURESCRIPT_RELEASES_API_URL) {
        let mut ok = true;
        let mut page = 1;
        let mut acc = Vec::new();
        loop {
            let url = format!("{base}?per_page=100&page={page}");
            let text = match fetch_text(client, &url) {
                Ok(t) => t,
                Err(_) => {
                    ok = false;
                    break;
                }
            };
            let v: Value =
                serde_json::from_str(&text).map_err(|e| EnvrError::Download(e.to_string()))?;
            let Some(arr) = v.as_array() else {
                ok = false;
                break;
            };
            if arr.is_empty() {
                break;
            }
            for item in arr {
                let r: GhRelease = serde_json::from_value(item.clone())
                    .map_err(|e| EnvrError::Download(e.to_string()))?;
                acc.push(r);
            }
            if arr.len() < 100 {
                break;
            }
            page += 1;
        }
        if ok && !acc.is_empty() {
            all = acc;
            break;
        }
    }
    if all.is_empty() {
        Err(EnvrError::Download(
            "failed to fetch purescript releases index (all API candidates failed)".into(),
        ))
    } else {
        Ok(all)
    }
}

fn fetch_rows_via_atom(client: &reqwest::blocking::Client) -> EnvrResult<Vec<PurescriptInstallableRow>> {
    let text = fetch_text(client, PURESCRIPT_RELEASES_ATOM_URL)?;
    let asset = purescript_asset_candidates().first().copied().unwrap_or("");
    if asset.is_empty() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for cap in ATOM_RELEASE_TAG_RE.captures_iter(&text) {
        let tag = cap.get(1).map(|m| m.as_str()).unwrap_or("").trim();
        let Some(version) = label_from_tag(tag) else {
            continue;
        };
        if !seen.insert(version.clone()) {
            continue;
        }
        out.push(PurescriptInstallableRow {
            version,
            url: format!(
                "https://github.com/purescript/purescript/releases/download/{tag}/{asset}"
            ),
        });
    }
    out.sort_by(|a, b| cmp_release_labels(&a.version, &b.version));
    Ok(out)
}

fn fetch_rows_via_releases_pages(
    client: &reqwest::blocking::Client,
) -> EnvrResult<Vec<PurescriptInstallableRow>> {
    let asset = purescript_asset_candidates().first().copied().unwrap_or("");
    if asset.is_empty() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    let mut empty_pages = 0u8;
    for page in 1..=20 {
        let url = format!("https://github.com/purescript/purescript/releases?page={page}");
        let text = match fetch_text(client, &url) {
            Ok(t) => t,
            Err(_) => break,
        };
        let mut found_on_page = 0usize;
        for cap in RELEASES_PAGE_TAG_RE.captures_iter(&text) {
            let tag = cap.get(1).map(|m| m.as_str()).unwrap_or("").trim();
            let Some(version) = label_from_tag(tag) else {
                continue;
            };
            if !seen.insert(version.clone()) {
                continue;
            }
            found_on_page += 1;
            out.push(PurescriptInstallableRow {
                version,
                url: format!(
                    "https://github.com/purescript/purescript/releases/download/{tag}/{asset}"
                ),
            });
        }
        if found_on_page == 0 {
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

pub fn fetch_purescript_installable_rows_with_fallback(
    client: &reqwest::blocking::Client,
    releases_api_url: &str,
) -> EnvrResult<Vec<PurescriptInstallableRow>> {
    if let Ok(releases) = fetch_purescript_github_releases_index(client, releases_api_url) {
        let rows = installable_rows_from_releases(&releases);
        if !rows.is_empty() {
            return Ok(rows);
        }
    }
    let mut rows = fetch_rows_via_atom(client)?;
    // `releases.atom` often exposes only recent entries; supplement with paged HTML tags.
    if rows.len() < 5 {
        let mut page_rows = fetch_rows_via_releases_pages(client)?;
        rows.append(&mut page_rows);
        rows.sort_by(|a, b| cmp_release_labels(&a.version, &b.version));
        rows.dedup_by(|a, b| a.version == b.version);
    }
    Ok(rows)
}

pub fn list_remote_versions(
    rows: &[PurescriptInstallableRow],
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

pub fn list_remote_latest_per_major_lines(rows: &[PurescriptInstallableRow]) -> Vec<RuntimeVersion> {
    let mut best: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for r in rows {
        let Some(line) = version_line_key_for_kind(RuntimeKind::Purescript, &r.version) else {
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

pub fn resolve_purescript_version(rows: &[PurescriptInstallableRow], spec: &str) -> Option<String> {
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
    // Accept line-like specs (e.g. 0.15) and resolve to latest patch in that line.
    if t.chars().all(|c| c.is_ascii_digit() || c == '.') {
        let mut cands: Vec<&str> = rows
            .iter()
            .filter_map(|r| {
                if r.version == t || r.version.starts_with(&format!("{t}.")) {
                    Some(r.version.as_str())
                } else {
                    None
                }
            })
            .collect();
        cands.sort_by(|a, b| cmp_release_labels(a, b));
        if let Some(last) = cands.last() {
            return Some((*last).to_string());
        }
    }
    None
}

