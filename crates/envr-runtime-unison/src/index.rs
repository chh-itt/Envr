use envr_domain::runtime::{
    RemoteFilter, RuntimeVersion, numeric_version_segments,
};
use envr_error::{EnvrError, EnvrResult};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::cmp::Ordering;
use std::collections::HashSet;
use std::sync::LazyLock;
use std::time::Duration;

pub const DEFAULT_UNISON_RELEASES_API_URL: &str =
    "https://api.github.com/repos/unisonweb/unison/releases";
const UNISON_RELEASES_ATOM_URL: &str = "https://github.com/unisonweb/unison/releases.atom";

static ATOM_RELEASE_TAG_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"https://github\.com/unisonweb/unison/releases/tag/([^"<>]+)"#)
        .expect("unison atom release tag regex")
});
static HTML_RELEASE_TAG_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"/unisonweb/unison/releases/tag/([^"<>/]+)"#)
        .expect("unison html release tag regex")
});

// Tags are "release/1.2.0" or URL-encoded "release%2F1.2.0"
static TAG_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(?:release/|release%2F)(\d+\.\d+\.\d+)$").expect("unison tag regex")
});

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
    #[serde(default)]
    pub assets: Vec<GhAsset>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnisonInstallableRow {
    pub version: String,
    pub url: String,
}

pub fn blocking_http_client() -> EnvrResult<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(120))
        .user_agent(concat!("envr-runtime-unison/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| EnvrError::Download(e.to_string()))
}

fn github_api_auth_token() -> Option<String> {
    ["GITHUB_TOKEN", "GH_TOKEN", "ENVR_GITHUB_TOKEN"]
        .into_iter()
        .find_map(|k| std::env::var(k).ok())
        .and_then(|s| {
            let t = s.trim();
            if t.is_empty() { None } else { Some(t.to_string()) }
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

fn label_from_tag(tag: &str) -> Option<String> {
    TAG_RE
        .captures(tag.trim())
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
}

fn cmp_release_labels(a: &str, b: &str) -> Ordering {
    match (numeric_version_segments(a), numeric_version_segments(b)) {
        (Some(va), Some(vb)) => va.cmp(&vb),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => a.cmp(b),
    }
}

fn unison_asset_candidates() -> Vec<&'static str> {
    use std::env::consts::{ARCH, OS};
    match (OS, ARCH) {
        ("windows", "x86_64") => vec!["ucm-windows-x64.zip"],
        ("linux", "x86_64") => vec!["ucm-linux-x64.tar.gz"],
        ("linux", "aarch64") => vec!["ucm-linux-arm64.tar.gz"],
        ("macos", "x86_64") => vec!["ucm-macos-x64.tar.gz"],
        ("macos", "aarch64") => vec!["ucm-macos-arm64.tar.gz", "ucm-macos-x64.tar.gz"],
        _ => vec![],
    }
}

fn pick_asset<'a>(assets: &'a [GhAsset]) -> Option<&'a GhAsset> {
    let cands = unison_asset_candidates();
    assets.iter().find(|a| cands.iter().any(|name| a.name == *name))
}

fn installable_rows_from_releases(releases: &[GhRelease]) -> Vec<UnisonInstallableRow> {
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
        out.push(UnisonInstallableRow {
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
                "unison releases API returned non-array payload".into(),
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

fn fetch_rows_via_html(client: &reqwest::blocking::Client) -> EnvrResult<Vec<UnisonInstallableRow>> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    let cands = unison_asset_candidates();
    if cands.is_empty() {
        return Ok(out);
    }
    let mut empty_pages = 0usize;
    for page in 1..=30 {
        let url = format!("https://github.com/unisonweb/unison/releases?page={page}");
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
            // HTML fallback can't enumerate assets reliably; assume canonical asset name.
            let url = format!(
                "https://github.com/unisonweb/unison/releases/download/{tag}/{}",
                cands[0]
            );
            out.push(UnisonInstallableRow { version, url });
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

fn fetch_rows_via_atom(client: &reqwest::blocking::Client) -> EnvrResult<Vec<UnisonInstallableRow>> {
    let text = fetch_text(client, UNISON_RELEASES_ATOM_URL)?;
    let cands = unison_asset_candidates();
    if cands.is_empty() {
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
        let url = format!(
            "https://github.com/unisonweb/unison/releases/download/{tag}/{}",
            cands[0]
        );
        out.push(UnisonInstallableRow { version, url });
    }
    out.sort_by(|a, b| cmp_release_labels(&a.version, &b.version));
    Ok(out)
}

pub fn fetch_unison_installable_rows_with_fallback(
    client: &reqwest::blocking::Client,
    releases_api_url: &str,
) -> EnvrResult<Vec<UnisonInstallableRow>> {
    if let Ok(releases) = fetch_github_releases_index(client, releases_api_url) {
        let rows = installable_rows_from_releases(&releases);
        if !rows.is_empty() {
            return Ok(rows);
        }
    }
    if let Ok(rows) = fetch_rows_via_html(client)
        && !rows.is_empty()
    {
        return Ok(rows);
    }
    fetch_rows_via_atom(client)
}

pub fn list_remote_versions(rows: &[UnisonInstallableRow], filter: &RemoteFilter) -> Vec<RuntimeVersion> {
    let mut out: Vec<RuntimeVersion> = rows
        .iter()
        .filter(|r| {
            filter
                .prefix
                .as_ref()
                .map(|p| r.version.starts_with(p.trim()))
                .unwrap_or(true)
        })
        .map(|r| RuntimeVersion(r.version.clone()))
        .collect();
    out.sort_by(|a, b| cmp_release_labels(&a.0, &b.0));
    out.dedup_by(|a, b| a.0 == b.0);
    out
}

pub fn list_remote_latest_per_major_lines(rows: &[UnisonInstallableRow]) -> Vec<RuntimeVersion> {
    use std::collections::HashMap;
    let mut best: HashMap<String, String> = HashMap::new();
    for r in rows {
        let Some(parts) = numeric_version_segments(&r.version) else {
            continue;
        };
        if parts.len() < 2 {
            continue;
        }
        let line = format!("{}.{}", parts[0], parts[1]);
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
    let mut out: Vec<RuntimeVersion> = best.into_values().map(RuntimeVersion).collect();
    out.sort_by(|a, b| cmp_release_labels(&a.0, &b.0));
    out
}

pub fn resolve_unison_version(rows: &[UnisonInstallableRow], spec: &str) -> Option<String> {
    let s = spec.trim();
    if s.is_empty() {
        return None;
    }
    if s.eq_ignore_ascii_case("latest") {
        return rows.iter().map(|r| r.version.clone()).max_by(|a, b| cmp_release_labels(a, b));
    }
    // exact
    if rows.iter().any(|r| r.version == s) {
        return Some(s.to_string());
    }
    // prefix (major / major.minor)
    let mut best: Option<String> = None;
    for r in rows {
        if r.version == s || r.version.starts_with(&format!("{s}.")) {
            match &best {
                None => best = Some(r.version.clone()),
                Some(prev) => {
                    if cmp_release_labels(prev, &r.version) == Ordering::Less {
                        best = Some(r.version.clone());
                    }
                }
            }
        }
    }
    best
}

