//! Clojure CLI tools bundles from `clojure/brew-install` GitHub releases.

use crate::releases_url::DEFAULT_CLOJURE_RELEASES_API_URL;
use envr_domain::runtime::{RemoteFilter, RuntimeKind, RuntimeVersion, version_line_key_for_kind};
use envr_error::{EnvrError, EnvrResult};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::HashSet;
use std::sync::LazyLock;
use std::time::Duration;

const CLOJURE_RELEASES_ATOM_URL: &str = "https://github.com/clojure/brew-install/releases.atom";

static ATOM_RELEASE_TAG_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"https://github\.com/clojure/brew-install/releases/tag/([^"<>]+)"#)
        .expect("atom release tag regex")
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

pub fn blocking_http_client() -> EnvrResult<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(120))
        .user_agent(concat!(
            "envr-runtime-clojure/",
            env!("CARGO_PKG_VERSION"),
            " (https://clojure.org; envr)"
        ))
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

pub fn fetch_text(client: &reqwest::blocking::Client, url: &str) -> EnvrResult<String> {
    let mut req = client.get(url);
    if url_is_github_api(url) {
        req = req
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28");
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

pub fn fetch_releases_json(client: &reqwest::blocking::Client, url: &str) -> EnvrResult<String> {
    fetch_text(client, url)
}

fn cmp_semver_release_labels(a: &str, b: &str) -> Ordering {
    use envr_domain::runtime::numeric_version_segments;
    match (numeric_version_segments(a), numeric_version_segments(b)) {
        (Some(va), Some(vb)) => va.cmp(&vb),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => a.cmp(b),
    }
}

fn label_from_tag(tag: &str) -> Option<String> {
    let mut t = tag.trim();
    if let Some(rest) = t.strip_prefix('v') {
        t = rest;
    } else if let Some(rest) = t.strip_prefix('V') {
        t = rest;
    }
    if t.is_empty() {
        return None;
    }
    if !t.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        return None;
    }
    Some(t.to_string())
}

pub fn clojure_asset_candidates(label: &str) -> Vec<String> {
    let mut out = Vec::new();
    match std::env::consts::OS {
        "windows" => {
            out.push("clojure-tools.zip".to_string());
            out.push(format!("clojure-tools-{label}.zip"));
        }
        _ => {
            out.push(format!("clojure-tools-{label}.tar.gz"));
            out.push("clojure-tools.tar.gz".to_string());
            out.push("clojure-tools.zip".to_string());
        }
    }
    out
}

pub fn pick_clojure_asset_url(assets: &[GhAsset], label: &str) -> Option<String> {
    for name in clojure_asset_candidates(label) {
        if let Some(a) = assets.iter().find(|a| a.name == name) {
            return Some(a.browser_download_url.clone());
        }
    }
    None
}

pub fn installable_pairs_from_releases(releases: &[GhRelease]) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for rel in releases {
        if rel.draft || rel.prerelease {
            continue;
        }
        let Some(label) = label_from_tag(&rel.tag_name) else {
            continue;
        };
        if !label.chars().all(|c| c.is_ascii_digit() || c == '.') {
            continue;
        }
        let Some(url) = pick_clojure_asset_url(&rel.assets, &label) else {
            continue;
        };
        out.push((label, url));
    }
    out.sort_by(|a, b| cmp_semver_release_labels(&b.0, &a.0));
    out.dedup_by(|a, b| a.0 == b.0);
    out
}

pub fn list_remote_versions(pairs: &[(String, String)], filter: &RemoteFilter) -> Vec<RuntimeVersion> {
    let mut labels: Vec<String> = pairs.iter().map(|(l, _)| l.clone()).collect();
    if let Some(prefix) = filter.prefix.as_deref() {
        let p = prefix.trim();
        if !p.is_empty() {
            labels.retain(|k| k.starts_with(p));
        }
    }
    labels.into_iter().map(RuntimeVersion).collect()
}

pub fn list_remote_latest_per_major_lines(pairs: &[(String, String)]) -> Vec<RuntimeVersion> {
    let mut seen = HashSet::<String>::new();
    let mut out = Vec::new();
    for (label, _) in pairs {
        if let Some(line) = version_line_key_for_kind(RuntimeKind::Clojure, label) {
            if seen.insert(line) {
                out.push(RuntimeVersion(label.clone()));
            }
        }
    }
    out
}

pub fn resolve_clojure_version(pairs: &[(String, String)], spec: &str) -> EnvrResult<String> {
    let s = spec.trim().trim_start_matches('v').trim_start_matches('V');
    if s.is_empty() {
        return Err(EnvrError::Validation("empty clojure version spec".into()));
    }
    let labels: Vec<&str> = pairs.iter().map(|(l, _)| l.as_str()).collect();
    if labels.iter().any(|k| *k == s) {
        return Ok(s.to_string());
    }
    use envr_domain::runtime::numeric_version_segments;
    if let Some(parts) = numeric_version_segments(s) {
        match parts.len() {
            1 => {
                let major = parts[0];
                if let Some(best) = pairs.iter().map(|(l, _)| l.as_str()).find(|label| {
                    numeric_version_segments(label).is_some_and(|p| !p.is_empty() && p[0] == major)
                }) {
                    return Ok(best.to_string());
                }
            }
            2 => {
                let major = parts[0];
                let minor = parts[1];
                if let Some(best) = pairs.iter().map(|(l, _)| l.as_str()).find(|label| {
                    numeric_version_segments(label)
                        .is_some_and(|p| p.len() >= 2 && p[0] == major && p[1] == minor)
                }) {
                    return Ok(best.to_string());
                }
            }
            _ => {}
        }
    }
    Err(EnvrError::Validation(format!(
        "no clojure release matches spec `{s}` (try a full label like 1.12.4.1629)"
    )))
}

fn strip_known_github_api_proxy_prefix(url: &str) -> Option<String> {
    let u = url.trim();
    const NEEDLE: &str = "https://api.github.com/";
    let i = u.find(NEEDLE)?;
    Some(u[i..].to_string())
}

fn candidate_clojure_releases_api_bases(primary: &str) -> Vec<String> {
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
    push(DEFAULT_CLOJURE_RELEASES_API_URL);
    out
}

fn synthetic_clojure_release(tag: &str) -> Option<GhRelease> {
    let label = label_from_tag(tag)?;
    let fname = clojure_asset_candidates(&label).into_iter().next()?;
    let url = format!("https://github.com/clojure/brew-install/releases/download/{tag}/{fname}");
    Some(GhRelease {
        tag_name: tag.to_string(),
        draft: false,
        prerelease: false,
        assets: vec![GhAsset {
            name: fname,
            browser_download_url: url,
        }],
    })
}

fn fetch_clojure_releases_via_github_api(
    client: &reqwest::blocking::Client,
    base_url: &str,
) -> EnvrResult<Vec<GhRelease>> {
    let max_pages = std::env::var("ENVR_CLOJURE_GITHUB_RELEASES_MAX_PAGES")
        .ok()
        .and_then(|s| s.parse::<u32>().ok())
        .filter(|n| *n > 0)
        .unwrap_or(8);
    let mut merged: Vec<GhRelease> = Vec::new();
    let mut seen_tags = HashSet::<String>::new();
    for page in 1_u32..=max_pages {
        let url = if base_url.contains('?') {
            format!("{base_url}&page={page}")
        } else {
            format!("{base_url}?per_page=100&page={page}")
        };
        let body = match fetch_text(client, &url) {
            Ok(b) => b,
            Err(e) => {
                if page == 1 {
                    return Err(e);
                }
                break;
            }
        };
        let page_releases: Vec<GhRelease> =
            serde_json::from_str(&body).map_err(|e| EnvrError::Validation(e.to_string()))?;
        let page_len = page_releases.len();
        if page_len == 0 {
            break;
        }
        for rel in page_releases {
            if seen_tags.insert(rel.tag_name.clone()) {
                merged.push(rel);
            }
        }
        if page_len < 100 {
            break;
        }
    }
    Ok(merged)
}

fn fetch_clojure_releases_via_releases_atom(
    client: &reqwest::blocking::Client,
) -> EnvrResult<Vec<GhRelease>> {
    let mut seen_tags = HashSet::new();
    let mut tags_in_order = Vec::new();
    for page in 1_u32..=50 {
        let url = if page == 1 {
            CLOJURE_RELEASES_ATOM_URL.to_string()
        } else {
            format!("{CLOJURE_RELEASES_ATOM_URL}?page={page}")
        };
        let body = fetch_text(client, &url)?;
        let mut new_this_page = 0usize;
        for cap in ATOM_RELEASE_TAG_RE.captures_iter(&body) {
            let Some(m) = cap.get(1) else {
                continue;
            };
            let tag = m.as_str().trim().trim_end_matches('/');
            if tag.is_empty() || label_from_tag(tag).is_none() {
                continue;
            }
            if seen_tags.insert(tag.to_string()) {
                tags_in_order.push(tag.to_string());
                new_this_page += 1;
            }
        }
        if page == 1 && new_this_page == 0 && seen_tags.is_empty() {
            return Err(EnvrError::Download(
                "clojure: releases.atom contained no release tag links".into(),
            ));
        }
        if page > 1 && new_this_page == 0 {
            break;
        }
    }
    let mut out: Vec<GhRelease> = tags_in_order
        .into_iter()
        .filter_map(|t| synthetic_clojure_release(&t))
        .collect();
    if out.is_empty() {
        return Err(EnvrError::Download(
            "clojure: atom index produced no installable rows for this platform".into(),
        ));
    }
    out.sort_by(|a, b| {
        let la = label_from_tag(&a.tag_name).unwrap_or_default();
        let lb = label_from_tag(&b.tag_name).unwrap_or_default();
        cmp_semver_release_labels(&lb, &la)
    });
    Ok(out)
}

pub fn fetch_clojure_github_releases_index(
    client: &reqwest::blocking::Client,
    api_base_url: &str,
) -> EnvrResult<Vec<GhRelease>> {
    for base in candidate_clojure_releases_api_bases(api_base_url) {
        match fetch_clojure_releases_via_github_api(client, &base) {
            Ok(rows) if !rows.is_empty() => return Ok(rows),
            Ok(_) | Err(_) => {}
        }
    }
    fetch_clojure_releases_via_releases_atom(client)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_windows_zip_candidate_first_on_windows() {
        if std::env::consts::OS == "windows" {
            let cs = clojure_asset_candidates("1.12.4.1629");
            assert_eq!(cs[0], "clojure-tools.zip");
        }
    }

    #[test]
    fn candidate_api_bases_include_default() {
        let wrapped = "https://ghproxy.net/https://api.github.com/repos/clojure/brew-install/releases?per_page=100";
        let bases = candidate_clojure_releases_api_bases(wrapped);
        assert!(bases.iter().any(|b| b == DEFAULT_CLOJURE_RELEASES_API_URL));
    }
}
