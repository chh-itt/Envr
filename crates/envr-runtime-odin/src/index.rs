//! Odin: prebuilt monthly toolchains from `odin-lang/Odin` GitHub releases.

use envr_domain::runtime::{RemoteFilter, RuntimeKind, RuntimeVersion, numeric_version_segments, version_line_key_for_kind};
use envr_download::blocking::build_blocking_http_client;
use envr_error::{EnvrError, EnvrResult, ErrorCode};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::cmp::Ordering;
use std::collections::HashSet;
use std::sync::LazyLock;
use std::time::Duration;

pub const DEFAULT_ODIN_RELEASES_API_URL: &str = "https://api.github.com/repos/odin-lang/Odin/releases";
const ODIN_RELEASES_ATOM_URL: &str = "https://github.com/odin-lang/Odin/releases.atom";

static ATOM_RELEASE_TAG_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"https://github\.com/odin-lang/Odin/releases/tag/([^"<>]+)"#)
        .expect("odin atom release tag regex")
});

static ODIN_DEV_TAG_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^dev-(\d{4})-(\d{2})([a-z])?$").expect("odin dev tag regex")
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
    pub assets: Vec<GhAsset>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OdinInstallableRow {
    pub version: String,
    pub url: String,
}

pub fn blocking_http_client() -> EnvrResult<reqwest::blocking::Client> {
    build_blocking_http_client(
        concat!("envr-runtime-odin/", env!("CARGO_PKG_VERSION")),
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

fn url_is_github_api(url: &str) -> bool {
    url.contains("api.github.com")
}

pub fn fetch_text(client: &reqwest::blocking::Client, url: &str) -> EnvrResult<String> {
    let mut req = client
        .get(url)
        .header("Accept", "application/vnd.github+json");
    if url_is_github_api(url) {
        req = req.header("X-GitHub-Api-Version", "2022-11-28");
        if let Some(tok) = github_api_auth_token() {
            req = req.header("Authorization", format!("Bearer {tok}"));
        }
    }
    let response = req
        .send()
        .map_err(|e| EnvrError::with_source(ErrorCode::Download, format!("request failed for {url}"), e))?;
    if !response.status().is_success() {
        return Err(EnvrError::Download(format!(
            "GET {url} -> {}",
            response.status()
        )));
    }
    response
        .text()
        .map_err(|e| EnvrError::with_source(ErrorCode::Download, format!("read body failed for {url}"), e))
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

fn label_from_dev_tag(tag: &str) -> Option<String> {
    let caps = ODIN_DEV_TAG_RE.captures(tag.trim())?;
    let year = caps.get(1)?.as_str();
    let month = caps.get(2)?.as_str();
    let suffix = caps.get(3).map(|m| m.as_str());

    if let Some(s) = suffix {
        let c = s.chars().next()?;
        if !('a'..='z').contains(&c.to_ascii_lowercase()) {
            return None;
        }
        let n = (c.to_ascii_lowercase() as u8).saturating_sub(b'a') as u64 + 1;
        Some(format!("{year}.{month}.{n}"))
    } else {
        Some(format!("{year}.{month}"))
    }
}

pub fn odin_asset_prefix_candidates() -> Vec<&'static str> {
    use std::env::consts::{ARCH, OS};
    match (OS, ARCH) {
        ("windows", "x86_64") => vec!["odin-windows-amd64-"],
        ("linux", "x86_64") => vec!["odin-linux-amd64-"],
        ("linux", "aarch64") => vec!["odin-linux-arm64-", "odin-linux-aarch64-", "odin-linux-amd64-"],
        ("macos", "x86_64") => vec!["odin-macos-amd64-"],
        ("macos", "aarch64") => vec!["odin-macos-arm64-", "odin-macos-aarch64-", "odin-macos-amd64-"],
        _ => vec![],
    }
}

fn odin_asset_extension_candidates() -> Vec<&'static str> {
    use std::env::consts::OS;
    match OS {
        "windows" => vec![".zip"],
        _ => vec![".tar.gz"],
    }
}

fn pick_asset_for_tag<'a>(assets: &'a [GhAsset], tag: &str) -> Option<&'a GhAsset> {
    let prefixes = odin_asset_prefix_candidates();
    if prefixes.is_empty() {
        return None;
    }
    let exts = odin_asset_extension_candidates();
    assets.iter().find(|a| {
        prefixes.iter().any(|p| a.name.starts_with(p))
            && exts.iter().any(|e| a.name.ends_with(e))
            && a.name.contains(tag)
    })
}

pub fn installable_rows_from_releases(releases: &[GhRelease]) -> Vec<OdinInstallableRow> {
    let mut out = Vec::new();
    for rel in releases {
        if rel.draft || rel.prerelease {
            continue;
        }
        let tag = rel.tag_name.trim();
        let Some(label) = label_from_dev_tag(tag) else {
            continue;
        };
        let Some(asset) = pick_asset_for_tag(&rel.assets, tag) else {
            continue;
        };
        out.push(OdinInstallableRow {
            version: label,
            url: asset.browser_download_url.clone(),
        });
    }
    out.sort_by(|a, b| cmp_release_labels(&a.version, &b.version));
    out.dedup_by(|a, b| a.version == b.version);
    out
}

pub fn fetch_odin_github_releases_index(
    client: &reqwest::blocking::Client,
    releases_api_url: &str,
) -> EnvrResult<Vec<GhRelease>> {
    let mut all = Vec::new();
    for base in candidate_api_bases(releases_api_url, DEFAULT_ODIN_RELEASES_API_URL) {
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
            let v: Value = serde_json::from_str(&text)
                .map_err(|e| EnvrError::with_source(ErrorCode::Validation, "invalid github releases json", e))?;
            let Some(arr) = v.as_array() else {
                ok = false;
                break;
            };
            if arr.is_empty() {
                break;
            }
            for item in arr {
                let r: GhRelease = serde_json::from_value(item.clone())
                    .map_err(|e| EnvrError::with_source(ErrorCode::Validation, "invalid github release entry", e))?;
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
            "failed to fetch odin releases index (all API candidates failed)".into(),
        ))
    } else {
        Ok(all)
    }
}

fn synthetic_asset_name(tag: &str) -> Option<String> {
    let prefixes = odin_asset_prefix_candidates();
    let exts = odin_asset_extension_candidates();
    let prefix = prefixes.first().copied()?;
    let ext = exts.first().copied()?;
    Some(format!("{prefix}{tag}{ext}"))
}

fn fetch_tags_via_atom(client: &reqwest::blocking::Client) -> EnvrResult<Vec<String>> {
    let text = fetch_text(client, ODIN_RELEASES_ATOM_URL)?;
    let mut tags = Vec::new();
    let mut seen = HashSet::<String>::new();
    for cap in ATOM_RELEASE_TAG_RE.captures_iter(&text) {
        let t = cap.get(1).map(|m| m.as_str()).unwrap_or("").trim();
        if t.is_empty() {
            continue;
        }
        if seen.insert(t.to_string()) {
            tags.push(t.to_string());
        }
    }
    Ok(tags)
}

pub fn fetch_odin_installable_rows_with_fallback(
    client: &reqwest::blocking::Client,
    releases_api_url: &str,
) -> EnvrResult<Vec<OdinInstallableRow>> {
    if let Ok(releases) = fetch_odin_github_releases_index(client, releases_api_url) {
        let rows = installable_rows_from_releases(&releases);
        if !rows.is_empty() {
            return Ok(rows);
        }
    }
    // Atom fallback: tags -> synthetic download URLs.
    let tags = fetch_tags_via_atom(client)?;
    let mut out = Vec::new();
    for tag in tags {
        let Some(label) = label_from_dev_tag(&tag) else {
            continue;
        };
        let Some(asset) = synthetic_asset_name(&tag) else {
            continue;
        };
        let url = format!(
            "https://github.com/odin-lang/Odin/releases/download/{tag}/{asset}"
        );
        out.push(OdinInstallableRow { version: label, url });
    }
    out.sort_by(|a, b| cmp_release_labels(&a.version, &b.version));
    out.dedup_by(|a, b| a.version == b.version);
    Ok(out)
}

pub fn list_remote_versions(
    rows: &[OdinInstallableRow],
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

pub fn list_remote_latest_per_major_lines(rows: &[OdinInstallableRow]) -> Vec<RuntimeVersion> {
    let mut best: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for r in rows {
        let Some(line) = version_line_key_for_kind(RuntimeKind::Odin, &r.version) else {
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

pub fn resolve_odin_version(rows: &[OdinInstallableRow], spec: &str) -> Option<String> {
    let t = spec.trim();
    if t.is_empty() {
        return None;
    }
    // Allow specifying upstream tag directly.
    if let Some(label) = label_from_dev_tag(t) {
        return Some(label);
    }
    // Exact label.
    if rows.iter().any(|r| r.version == t) {
        return Some(t.to_string());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn odin_dev_tag_maps_to_numeric_label() {
        assert_eq!(label_from_dev_tag("dev-2026-04").as_deref(), Some("2026.04"));
        assert_eq!(label_from_dev_tag("dev-2025-12a").as_deref(), Some("2025.12.1"));
        assert_eq!(label_from_dev_tag("DEV-2025-12B").as_deref(), Some("2025.12.2"));
    }

    #[test]
    fn odin_label_is_numeric_segments() {
        assert_eq!(numeric_version_segments("2026.04").unwrap(), vec![2026, 4]);
        assert_eq!(numeric_version_segments("2025.12.1").unwrap(), vec![2025, 12, 1]);
    }
}

