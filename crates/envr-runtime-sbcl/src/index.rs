use envr_domain::runtime::{
    RemoteFilter, RuntimeKind, RuntimeVersion, numeric_version_segments, version_line_key_for_kind,
};
use envr_download::blocking::build_blocking_http_client;
use envr_error::{EnvrError, EnvrResult, ErrorCode};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::cmp::Ordering;
use std::collections::HashSet;
use std::sync::LazyLock;
use std::time::Duration;

pub const DEFAULT_SBCL_BIN_RELEASES_API_URL: &str =
    "https://api.github.com/repos/roswell/sbcl_bin/releases";
const SBCL_BIN_RELEASES_ATOM_URL: &str = "https://github.com/roswell/sbcl_bin/releases.atom";

static ATOM_RELEASE_TAG_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"https://github\.com/roswell/sbcl_bin/releases/tag/([^"<>]+)"#)
        .expect("sbcl_bin atom release tag regex")
});
static HTML_RELEASE_TAG_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"/roswell/sbcl_bin/releases/tag/([^"<>/]+)"#)
        .expect("sbcl_bin html release tag regex")
});
static TAG_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^v?(\d+\.\d+\.\d+)$").expect("sbcl tag regex"));

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
pub struct SbclInstallableRow {
    pub version: String,
    pub url: String,
}

pub fn blocking_http_client() -> EnvrResult<reqwest::blocking::Client> {
    build_blocking_http_client(
        concat!("envr-runtime-sbcl/", env!("CARGO_PKG_VERSION")),
        Some(Duration::from_secs(120)),
    )
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
    let response = req
        .send()
        .map_err(|e| EnvrError::with_source(ErrorCode::Download, format!("request failed for {url}"), e))?;
    if !response.status().is_success() {
        return Err(EnvrError::Download(format!("GET {url} -> {}", response.status())));
    }
    response
        .text()
        .map_err(|e| EnvrError::with_source(ErrorCode::Download, format!("read body failed for {url}"), e))
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

fn asset_priority(name: &str, version: &str) -> Option<u8> {
    // Prefer assets that contain a usable SBCL executable for the current OS/arch.
    //
    // NOTE: On Windows, `sbcl_bin`'s `*-windows-binary.tar.bz2` does not necessarily ship `sbcl.exe`
    // (it may be a build tree). The `*.msi` is the reliable install surface, so prefer MSI.
    use std::env::consts::{ARCH, OS};
    let v = version;
    match (OS, ARCH) {
        ("windows", "x86_64") => {
            if name == format!("sbcl-{v}-x86-64-windows-binary.msi") {
                Some(0)
            } else if name == format!("sbcl-{v}-x86-64-windows-binary.tar.bz2") {
                Some(10)
            } else {
                None
            }
        }
        ("windows", "aarch64") => {
            if name == format!("sbcl-{v}-arm64-windows-binary.msi") {
                Some(0)
            } else if name == format!("sbcl-{v}-arm64-windows-binary.tar.bz2") {
                Some(10)
            } else {
                None
            }
        }
        ("linux", "x86_64") => {
            if name == format!("sbcl-{v}-x86-64-linux-binary.tar.bz2") {
                Some(0)
            } else {
                None
            }
        }
        ("linux", "aarch64") => {
            if name == format!("sbcl-{v}-arm64-linux-binary.tar.bz2") {
                Some(0)
            } else {
                None
            }
        }
        ("macos", "x86_64") => {
            if name == format!("sbcl-{v}-x86-64-darwin-binary.tar.bz2") {
                Some(0)
            } else {
                None
            }
        }
        ("macos", "aarch64") => {
            if name == format!("sbcl-{v}-arm64-darwin-binary.tar.bz2") {
                Some(0)
            } else {
                None
            }
        }
        _ => None,
    }
}

fn pick_asset<'a>(assets: &'a [GhAsset], version: &str) -> Option<&'a GhAsset> {
    assets
        .iter()
        .filter_map(|a| asset_priority(&a.name, version).map(|p| (p, a)))
        .min_by_key(|(p, _)| *p)
        .map(|(_, a)| a)
}

fn installable_rows_from_releases(releases: &[GhRelease]) -> Vec<SbclInstallableRow> {
    let mut out = Vec::new();
    for rel in releases {
        if rel.draft {
            continue;
        }
        let Some(version) = label_from_tag(&rel.tag_name) else {
            continue;
        };
        let Some(asset) = pick_asset(&rel.assets, &version) else {
            continue;
        };
        out.push(SbclInstallableRow {
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
            serde_json::from_str(&text).map_err(|e| {
                EnvrError::with_source(ErrorCode::Validation, "invalid github releases json", e)
            })?;
        let Some(arr) = v.as_array() else {
            return Err(EnvrError::Download(
                "sbcl_bin releases API returned non-array payload".into(),
            ));
        };
        if arr.is_empty() {
            break;
        }
        for item in arr {
            let r: GhRelease =
                serde_json::from_value(item.clone()).map_err(|e| {
                    EnvrError::with_source(ErrorCode::Validation, "invalid github release entry", e)
                })?;
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
    let v = version;
    let file = match (OS, ARCH) {
        ("windows", "x86_64") => format!("sbcl-{v}-x86-64-windows-binary.msi"),
        ("windows", "aarch64") => format!("sbcl-{v}-arm64-windows-binary.msi"),
        ("linux", "x86_64") => format!("sbcl-{v}-x86-64-linux-binary.tar.bz2"),
        ("linux", "aarch64") => format!("sbcl-{v}-arm64-linux-binary.tar.bz2"),
        ("macos", "x86_64") => format!("sbcl-{v}-x86-64-darwin-binary.tar.bz2"),
        ("macos", "aarch64") => format!("sbcl-{v}-arm64-darwin-binary.tar.bz2"),
        _ => return None,
    };
    Some(format!(
        "https://github.com/roswell/sbcl_bin/releases/download/{tag}/{file}"
    ))
}

fn fetch_rows_via_html(client: &reqwest::blocking::Client) -> EnvrResult<Vec<SbclInstallableRow>> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    let mut empty_pages = 0usize;
    for page in 1..=30 {
        let url = format!("https://github.com/roswell/sbcl_bin/releases?page={page}");
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
            out.push(SbclInstallableRow { version, url });
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

fn fetch_rows_via_atom(client: &reqwest::blocking::Client) -> EnvrResult<Vec<SbclInstallableRow>> {
    let mut out = Vec::new();
    let text = fetch_text(client, SBCL_BIN_RELEASES_ATOM_URL)?;
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
        out.push(SbclInstallableRow { version, url });
    }
    out.sort_by(|a, b| cmp_release_labels(&a.version, &b.version));
    Ok(out)
}

pub fn fetch_sbcl_installable_rows_with_fallback(
    client: &reqwest::blocking::Client,
    releases_api_url: &str,
) -> EnvrResult<Vec<SbclInstallableRow>> {
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

pub fn list_remote_versions(rows: &[SbclInstallableRow], filter: &RemoteFilter) -> Vec<RuntimeVersion> {
    let mut out: Vec<RuntimeVersion> = rows
        .iter()
        .filter(|r| filter.prefix.as_ref().map(|p| r.version.starts_with(p)).unwrap_or(true))
        .map(|r| RuntimeVersion(r.version.clone()))
        .collect();
    out.sort_by(|a, b| cmp_release_labels(&a.0, &b.0));
    out.dedup_by(|a, b| a.0 == b.0);
    out
}

pub fn list_remote_latest_per_major_lines(rows: &[SbclInstallableRow]) -> Vec<RuntimeVersion> {
    let mut best: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for r in rows {
        let Some(line) = version_line_key_for_kind(RuntimeKind::Sbcl, &r.version) else { continue; };
        match best.get(&line) {
            None => { best.insert(line, r.version.clone()); }
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

pub fn resolve_sbcl_version(rows: &[SbclInstallableRow], spec: &str) -> Option<String> {
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

