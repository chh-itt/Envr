use envr_domain::runtime::{RemoteFilter, RuntimeKind, RuntimeVersion, version_line_key_for_kind};
use envr_download::blocking::build_blocking_http_client;
use envr_error::{EnvrError, EnvrResult, ErrorCode};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::HashSet;
use std::sync::LazyLock;
use std::time::Duration;

pub const DEFAULT_V_RELEASES_API_URL: &str = "https://api.github.com/repos/vlang/v/releases";
const V_RELEASES_ATOM_URL: &str = "https://github.com/vlang/v/releases.atom";

static ATOM_RELEASE_TAG_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"https://github\.com/vlang/v/releases/tag/([^"<>]+)"#)
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VInstallableRow {
    pub version: String,
    pub url: String,
}

pub fn blocking_http_client() -> EnvrResult<reqwest::blocking::Client> {
    build_blocking_http_client(
        concat!("envr-runtime-v/", env!("CARGO_PKG_VERSION")),
        Some(Duration::from_secs(120)),
    )
}

fn fetch_text(client: &reqwest::blocking::Client, url: &str) -> EnvrResult<String> {
    envr_runtime_github_release::fetch_text(client, url)
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
    if !t.chars().all(|c| c.is_ascii_digit() || c == '.') {
        return None;
    }
    Some(t.to_string())
}

pub fn v_asset_candidates() -> Vec<&'static str> {
    use std::env::consts::{ARCH, OS};
    match (OS, ARCH) {
        ("windows", _) => vec!["v_windows.zip"],
        ("linux", "x86_64") => vec!["v_linux.zip"],
        ("linux", "aarch64") => vec!["v_linux_arm64.zip", "v_linux.zip"],
        ("macos", "aarch64") => vec!["v_macos_arm64.zip", "v_macos_x86_64.zip"],
        ("macos", "x86_64") => vec!["v_macos_x86_64.zip"],
        _ => vec![],
    }
}

fn synthetic_v_release(tag: &str) -> Option<GhRelease> {
    let _label = label_from_tag(tag)?;
    let fname = v_asset_candidates().into_iter().next()?;
    let url = format!("https://github.com/vlang/v/releases/download/{tag}/{fname}");
    Some(GhRelease {
        tag_name: tag.to_string(),
        draft: false,
        prerelease: false,
        assets: vec![GhAsset {
            name: fname.to_string(),
            browser_download_url: url,
        }],
    })
}

pub fn pick_v_asset_url(assets: &[GhAsset]) -> Option<String> {
    for name in v_asset_candidates() {
        if let Some(a) = assets.iter().find(|a| a.name == name) {
            return Some(a.browser_download_url.clone());
        }
    }
    None
}

fn strip_known_github_api_proxy_prefix(url: &str) -> Option<String> {
    let u = url.trim();
    const NEEDLE: &str = "https://api.github.com/";
    let i = u.find(NEEDLE)?;
    Some(u[i..].to_string())
}

fn candidate_v_releases_api_bases(primary: &str) -> Vec<String> {
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
    push(DEFAULT_V_RELEASES_API_URL);
    out
}

fn fetch_v_releases_via_github_api(
    client: &reqwest::blocking::Client,
    api_base_url: &str,
) -> EnvrResult<Vec<GhRelease>> {
    let max_pages = std::env::var("ENVR_V_GITHUB_RELEASES_MAX_PAGES")
        .ok()
        .and_then(|s| s.parse::<u32>().ok())
        .filter(|n| *n > 0)
        .unwrap_or(8);
    let mut merged = Vec::<GhRelease>::new();
    let mut seen_tags = HashSet::<String>::new();
    for page in 1_u32..=max_pages {
        let url = if api_base_url.contains('?') {
            format!("{api_base_url}&page={page}")
        } else {
            format!("{api_base_url}?per_page=100&page={page}")
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
        let page_releases: Vec<GhRelease> = serde_json::from_str(&body).map_err(|e| {
            EnvrError::with_source(ErrorCode::Validation, "invalid github releases json", e)
        })?;
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

fn fetch_v_releases_via_releases_atom(
    client: &reqwest::blocking::Client,
) -> EnvrResult<Vec<GhRelease>> {
    let mut seen_tags = HashSet::new();
    let mut tags_in_order = Vec::new();
    for page in 1_u32..=50 {
        let url = if page == 1 {
            V_RELEASES_ATOM_URL.to_string()
        } else {
            format!("{V_RELEASES_ATOM_URL}?page={page}")
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
                "v: releases.atom contained no release tag links".into(),
            ));
        }
        if page > 1 && new_this_page == 0 {
            break;
        }
    }
    let mut out: Vec<GhRelease> = tags_in_order
        .into_iter()
        .filter_map(|t| synthetic_v_release(&t))
        .collect();
    if out.is_empty() {
        return Err(EnvrError::Download(
            "v: atom index produced no installable rows for this platform".into(),
        ));
    }
    out.sort_by(|a, b| {
        let la = label_from_tag(&a.tag_name).unwrap_or_default();
        let lb = label_from_tag(&b.tag_name).unwrap_or_default();
        cmp_semver_release_labels(&lb, &la)
    });
    Ok(out)
}

pub fn fetch_v_github_releases_index(
    client: &reqwest::blocking::Client,
    api_base_url: &str,
) -> EnvrResult<Vec<GhRelease>> {
    for base in candidate_v_releases_api_bases(api_base_url) {
        match fetch_v_releases_via_github_api(client, &base) {
            Ok(rows) if !rows.is_empty() => return Ok(rows),
            Ok(_) | Err(_) => {}
        }
    }
    fetch_v_releases_via_releases_atom(client)
}

pub fn installable_rows_from_releases(releases: &[GhRelease]) -> Vec<VInstallableRow> {
    let mut out = Vec::new();
    for rel in releases {
        if rel.draft || rel.prerelease {
            continue;
        }
        let Some(label) = label_from_tag(&rel.tag_name) else {
            continue;
        };
        let Some(url) = pick_v_asset_url(&rel.assets) else {
            continue;
        };
        out.push(VInstallableRow {
            version: label,
            url,
        });
    }
    out.sort_by(|a, b| cmp_semver_release_labels(&b.version, &a.version));
    out.dedup_by(|a, b| a.version == b.version);
    out
}

pub fn list_remote_versions(
    rows: &[VInstallableRow],
    filter: &RemoteFilter,
) -> Vec<RuntimeVersion> {
    let mut labels: Vec<String> = rows.iter().map(|r| r.version.clone()).collect();
    if let Some(prefix) = filter.prefix.as_deref() {
        let p = prefix.trim();
        if !p.is_empty() {
            labels.retain(|v| v.starts_with(p));
        }
    }
    labels.into_iter().map(RuntimeVersion).collect()
}

pub fn list_remote_latest_per_major_lines(rows: &[VInstallableRow]) -> Vec<RuntimeVersion> {
    let mut seen = HashSet::<String>::new();
    let mut out = Vec::new();
    for r in rows {
        if let Some(line) = version_line_key_for_kind(RuntimeKind::V, &r.version)
            && seen.insert(line)
        {
            out.push(RuntimeVersion(r.version.clone()));
        }
    }
    out
}

pub fn resolve_v_version(rows: &[VInstallableRow], spec: &str) -> EnvrResult<String> {
    let s = spec.trim().trim_start_matches('v').trim_start_matches('V');
    if s.is_empty() {
        return Err(EnvrError::Validation("empty v version spec".into()));
    }
    let labels: Vec<&str> = rows.iter().map(|r| r.version.as_str()).collect();
    if labels.contains(&s) {
        return Ok(s.to_string());
    }
    if let Some(parts) = envr_domain::runtime::numeric_version_segments(s) {
        match parts.len() {
            1 => {
                let major = parts[0];
                if let Some(best) = rows.iter().map(|r| r.version.as_str()).find(|v| {
                    envr_domain::runtime::numeric_version_segments(v)
                        .is_some_and(|p| !p.is_empty() && p[0] == major)
                }) {
                    return Ok(best.to_string());
                }
            }
            2 => {
                let major = parts[0];
                let minor = parts[1];
                if let Some(best) = rows.iter().map(|r| r.version.as_str()).find(|v| {
                    envr_domain::runtime::numeric_version_segments(v)
                        .is_some_and(|p| p.len() >= 2 && p[0] == major && p[1] == minor)
                }) {
                    return Ok(best.to_string());
                }
            }
            _ => {}
        }
    }
    Err(EnvrError::Validation(format!(
        "no v release matches spec `{s}`"
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidate_api_bases_include_default_and_strip_proxy_wrappers() {
        let wrapped =
            "https://ghproxy.net/https://api.github.com/repos/vlang/v/releases?per_page=100";
        let bases = candidate_v_releases_api_bases(wrapped);
        assert!(bases.iter().any(|b| b == DEFAULT_V_RELEASES_API_URL));
        assert!(
            bases
                .iter()
                .any(|b| b == "https://api.github.com/repos/vlang/v/releases?per_page=100")
        );
    }

    #[test]
    fn installable_rows_keep_stable_only_and_order_desc() {
        let asset_name = v_asset_candidates().into_iter().next().expect("asset");
        let rows = installable_rows_from_releases(&[
            GhRelease {
                tag_name: "0.5.1".into(),
                draft: false,
                prerelease: false,
                assets: vec![GhAsset {
                    name: asset_name.to_string(),
                    browser_download_url: format!("https://example/{asset_name}"),
                }],
            },
            GhRelease {
                tag_name: "0.5.2-rc1".into(),
                draft: false,
                prerelease: true,
                assets: vec![GhAsset {
                    name: asset_name.to_string(),
                    browser_download_url: format!("https://example/{asset_name}"),
                }],
            },
            GhRelease {
                tag_name: "0.4.12".into(),
                draft: false,
                prerelease: false,
                assets: vec![GhAsset {
                    name: asset_name.to_string(),
                    browser_download_url: format!("https://example/{asset_name}"),
                }],
            },
        ]);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].version, "0.5.1");
        assert_eq!(rows[1].version, "0.4.12");
    }

    #[test]
    fn synthetic_release_uses_host_primary_asset_name() {
        let rel = synthetic_v_release("0.5.1").expect("synthetic");
        let cands = v_asset_candidates();
        let first = cands.first().expect("asset");
        assert_eq!(rel.assets.len(), 1);
        assert_eq!(rel.assets[0].name, *first);
        assert!(
            rel.assets[0]
                .browser_download_url
                .contains("releases/download/0.5.1/")
        );
    }
}
