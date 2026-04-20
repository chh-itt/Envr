use envr_domain::runtime::{RemoteFilter, RuntimeKind, RuntimeVersion, version_line_key_for_kind};
use envr_error::{EnvrError, EnvrResult};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::HashSet;
use std::time::Duration;

pub const DEFAULT_V_RELEASES_API_URL: &str = "https://api.github.com/repos/vlang/v/releases";

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
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(120))
        .user_agent(concat!("envr-runtime-v/", env!("CARGO_PKG_VERSION")))
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
    let mut req = client
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28");
    if let Some(tok) = github_api_auth_token() {
        req = req.header("Authorization", format!("Bearer {tok}"));
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

pub fn pick_v_asset_url(assets: &[GhAsset]) -> Option<String> {
    for name in v_asset_candidates() {
        if let Some(a) = assets.iter().find(|a| a.name == name) {
            return Some(a.browser_download_url.clone());
        }
    }
    None
}

pub fn fetch_v_github_releases_index(
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

pub fn list_remote_versions(rows: &[VInstallableRow], filter: &RemoteFilter) -> Vec<RuntimeVersion> {
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
    if labels.iter().any(|x| *x == s) {
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
