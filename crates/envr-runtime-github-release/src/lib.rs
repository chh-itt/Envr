use envr_error::{EnvrError, EnvrResult, ErrorCode};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::cmp::Ordering;
use std::collections::HashSet;

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
pub struct InstallableRow {
    pub version: String,
    pub url: String,
}

#[derive(Debug, Clone, Copy)]
pub struct GhRepo {
    pub owner: &'static str,
    pub name: &'static str,
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

pub fn fetch_text(client: &reqwest::blocking::Client, url: &str) -> EnvrResult<String> {
    let mut req = client
        .get(url)
        .header("Accept", "application/vnd.github+json");
    if url.contains("api.github.com") {
        req = req.header("X-GitHub-Api-Version", "2022-11-28");
        if let Some(tok) = github_api_auth_token() {
            req = req.header("Authorization", format!("Bearer {tok}"));
        }
    }
    let response = req.send().map_err(|e| {
        EnvrError::with_source(ErrorCode::Download, format!("request failed for {url}"), e)
    })?;
    if !response.status().is_success() {
        return Err(EnvrError::Download(format!(
            "GET {url} -> {}",
            response.status()
        )));
    }
    response.text().map_err(|e| {
        EnvrError::with_source(
            ErrorCode::Download,
            format!("read body failed for {url}"),
            e,
        )
    })
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
        if !t.is_empty() && !out.iter().any(|x| x == t) {
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

pub fn fetch_github_releases_index(
    client: &reqwest::blocking::Client,
    releases_api_url: &str,
    default_releases_api_url: &str,
) -> EnvrResult<Vec<GhRelease>> {
    let mut all = Vec::new();
    for base in candidate_api_bases(releases_api_url, default_releases_api_url) {
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
            let v: Value = serde_json::from_str(&text).map_err(|e| {
                EnvrError::with_source(ErrorCode::Validation, "invalid github releases json", e)
            })?;
            let Some(arr) = v.as_array() else {
                ok = false;
                break;
            };
            if arr.is_empty() {
                break;
            }
            for item in arr {
                let r: GhRelease = serde_json::from_value(item.clone()).map_err(|e| {
                    EnvrError::with_source(ErrorCode::Validation, "invalid github release entry", e)
                })?;
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
            "failed to fetch github releases index (all API candidates failed)".into(),
        ))
    } else {
        Ok(all)
    }
}

pub fn installable_rows_from_releases(
    releases: &[GhRelease],
    include_prerelease: bool,
    label_from_tag: impl Fn(&str) -> Option<String>,
    pick_asset_url: impl Fn(&[GhAsset]) -> Option<String>,
    cmp: impl Fn(&str, &str) -> Ordering,
) -> Vec<InstallableRow> {
    let mut out = Vec::new();
    for rel in releases {
        if rel.draft || (!include_prerelease && rel.prerelease) {
            continue;
        }
        let Some(version) = label_from_tag(&rel.tag_name) else {
            continue;
        };
        let Some(url) = pick_asset_url(&rel.assets) else {
            continue;
        };
        out.push(InstallableRow { version, url });
    }
    out.sort_by(|a, b| cmp(&a.version, &b.version));
    out.dedup_by(|a, b| a.version == b.version);
    out
}

fn atom_release_tag_re(repo: GhRepo) -> Regex {
    Regex::new(&format!(
        r#"https://github\.com/{}/{}/releases/tag/([^"<>]+)"#,
        regex::escape(repo.owner),
        regex::escape(repo.name)
    ))
    .expect("atom release tag regex")
}

fn html_release_tag_re(repo: GhRepo) -> Regex {
    Regex::new(&format!(
        r#"/{}/{}/releases/tag/([^"<>/]+)"#,
        regex::escape(repo.owner),
        regex::escape(repo.name)
    ))
    .expect("html release tag regex")
}

fn releases_page_url(repo: GhRepo, page: usize) -> String {
    format!(
        "https://github.com/{}/{}/releases?page={page}",
        repo.owner, repo.name
    )
}

fn atom_url(repo: GhRepo) -> String {
    format!(
        "https://github.com/{}/{}/releases.atom",
        repo.owner, repo.name
    )
}

pub fn fetch_rows_via_html(
    client: &reqwest::blocking::Client,
    repo: GhRepo,
    label_from_tag: impl Fn(&str) -> Option<String>,
    synth_url: impl Fn(&str, &str) -> Option<String>,
    cmp: impl Fn(&str, &str) -> Ordering,
) -> EnvrResult<Vec<InstallableRow>> {
    let re = html_release_tag_re(repo);
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    let mut empty_pages = 0usize;
    for page in 1..=30 {
        let url = releases_page_url(repo, page);
        let text = match fetch_text(client, &url) {
            Ok(t) => t,
            Err(_) => break,
        };
        let mut found = 0usize;
        for cap in re.captures_iter(&text) {
            let tag = cap.get(1).map(|m| m.as_str()).unwrap_or("").trim();
            let Some(version) = label_from_tag(tag) else {
                continue;
            };
            found += 1;
            if !seen.insert(version.clone()) {
                continue;
            }
            let Some(download_url) = synth_url(tag, &version) else {
                continue;
            };
            out.push(InstallableRow {
                version,
                url: download_url,
            });
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
    out.sort_by(|a, b| cmp(&a.version, &b.version));
    out.dedup_by(|a, b| a.version == b.version);
    Ok(out)
}

pub fn fetch_rows_via_atom(
    client: &reqwest::blocking::Client,
    repo: GhRepo,
    label_from_tag: impl Fn(&str) -> Option<String>,
    synth_url: impl Fn(&str, &str) -> Option<String>,
    cmp: impl Fn(&str, &str) -> Ordering,
) -> EnvrResult<Vec<InstallableRow>> {
    let text = fetch_text(client, &atom_url(repo))?;
    let re = atom_release_tag_re(repo);
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for cap in re.captures_iter(&text) {
        let tag = cap.get(1).map(|m| m.as_str()).unwrap_or("").trim();
        let Some(version) = label_from_tag(tag) else {
            continue;
        };
        if !seen.insert(version.clone()) {
            continue;
        }
        let Some(download_url) = synth_url(tag, &version) else {
            continue;
        };
        out.push(InstallableRow {
            version,
            url: download_url,
        });
    }
    out.sort_by(|a, b| cmp(&a.version, &b.version));
    Ok(out)
}
