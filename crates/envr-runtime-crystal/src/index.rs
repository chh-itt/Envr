//! Crystal: GitHub `crystal-lang/crystal` releases JSON → per-host tarball/zip URL + optional sha256.

use envr_domain::runtime::{RemoteFilter, RuntimeKind, RuntimeVersion, version_line_key_for_kind};
use envr_download::blocking::build_blocking_http_client;
use envr_error::{EnvrError, EnvrResult, ErrorCode};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::cmp::Ordering;
use std::collections::HashSet;
use std::sync::LazyLock;
use std::time::Duration;

pub const DEFAULT_CRYSTAL_GITHUB_RELEASES_URL: &str =
    "https://api.github.com/repos/crystal-lang/crystal/releases";

const CRYSTAL_GITHUB_RELEASES_ATOM_URL: &str =
    "https://github.com/crystal-lang/crystal/releases.atom";

static ATOM_RELEASE_TAG_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"https://github\.com/crystal-lang/crystal/releases/tag/([^"<>]+)"#)
        .expect("atom release tag regex")
});

pub fn blocking_http_client() -> EnvrResult<reqwest::blocking::Client> {
    build_blocking_http_client(
        concat!(
            "envr-runtime-crystal/",
            env!("CARGO_PKG_VERSION"),
            " (https://crystal-lang.org; envr)"
        ),
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

/// True when `url` targets the GitHub REST API (including via `https://*/https://api.github.com/...` proxies).
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

/// If `url` is wrapped by a third-party mirror (`…/https://api.github.com/…`), return the inner API URL.
fn strip_known_github_api_proxy_prefix(url: &str) -> Option<String> {
    let u = url.trim();
    const NEEDLE: &str = "https://api.github.com/";
    let i = u.find(NEEDLE)?;
    Some(u[i..].to_string())
}

fn candidate_crystal_releases_api_bases(primary: &str) -> Vec<String> {
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
    push(DEFAULT_CRYSTAL_GITHUB_RELEASES_URL);
    out
}

fn synthetic_crystal_release_asset_url(tag: &str, host_slug: &str) -> Option<String> {
    let ext = match host_slug {
        "linux-x86_64" | "linux-aarch64" | "darwin-universal" => "tar.gz",
        "windows-x86_64-msvc" | "windows-aarch64-gnu" => "zip",
        _ => return None,
    };
    let fname = format!("crystal-{tag}-1-{host_slug}.{ext}");
    Some(format!(
        "https://github.com/crystal-lang/crystal/releases/download/{tag}/{fname}"
    ))
}

fn fetch_release_index_via_github_api(
    client: &reqwest::blocking::Client,
    base_url: &str,
    host_slug: &str,
) -> EnvrResult<Vec<CrystalReleaseRow>> {
    let mut seen = HashSet::new();
    let mut merged: Vec<CrystalReleaseRow> = Vec::new();
    for page in 1_u32..=50 {
        let url = if base_url.contains('?') {
            format!("{base_url}&per_page=100&page={page}")
        } else {
            format!("{base_url}?per_page=100&page={page}")
        };
        let body = fetch_text(client, &url)?;
        let v: Value = serde_json::from_str(&body).map_err(|e| {
            EnvrError::with_source(ErrorCode::Validation, "invalid github releases json", e)
        })?;
        let page_len = v.as_array().map(|a| a.len()).unwrap_or(0);
        if page_len == 0 {
            break;
        }
        let rows = parse_github_releases_for_host(&body, host_slug)?;
        for r in rows {
            if seen.insert(r.version.clone()) {
                merged.push(r);
            }
        }
        if page_len < 100 {
            break;
        }
    }
    Ok(merged)
}

fn fetch_release_index_via_releases_atom(
    client: &reqwest::blocking::Client,
    host_slug: &str,
) -> EnvrResult<Vec<CrystalReleaseRow>> {
    let mut seen_tags = HashSet::new();
    let mut tags_in_order = Vec::new();
    for page in 1_u32..=50 {
        let url = if page == 1 {
            CRYSTAL_GITHUB_RELEASES_ATOM_URL.to_string()
        } else {
            format!("{CRYSTAL_GITHUB_RELEASES_ATOM_URL}?page={page}")
        };
        let body = fetch_text(client, &url)?;
        let mut new_this_page = 0usize;
        for cap in ATOM_RELEASE_TAG_RE.captures_iter(&body) {
            let Some(m) = cap.get(1) else {
                continue;
            };
            let tag = m.as_str().trim().trim_end_matches('/');
            if tag.is_empty() || !is_stable_crystal_tag(tag) {
                continue;
            }
            if seen_tags.insert(tag.to_string()) {
                tags_in_order.push(tag.to_string());
                new_this_page += 1;
            }
        }
        if page == 1 && new_this_page == 0 && seen_tags.is_empty() {
            return Err(EnvrError::Download(
                "crystal: releases.atom contained no stable release links".into(),
            ));
        }
        if page > 1 && new_this_page == 0 {
            break;
        }
    }
    let mut rows = Vec::new();
    for tag in tags_in_order {
        let version = tag
            .trim_start_matches('v')
            .trim_start_matches('V')
            .to_string();
        let Some(download_url) = synthetic_crystal_release_asset_url(&tag, host_slug) else {
            continue;
        };
        rows.push(CrystalReleaseRow {
            version,
            download_url,
            sha256_hex: None,
        });
    }
    rows.sort_by(|a, b| cmp_semver_release_labels(&b.version, &a.version));
    if rows.is_empty() {
        return Err(EnvrError::Download(
            "crystal: atom index produced no installable rows for this host".into(),
        ));
    }
    Ok(rows)
}

/// Paginated GitHub releases API, trying fallbacks, then `github.com/.../releases.atom` + synthetic asset URLs.
pub fn fetch_all_crystal_release_rows(
    client: &reqwest::blocking::Client,
    api_base_url: &str,
    host_slug: &str,
) -> EnvrResult<Vec<CrystalReleaseRow>> {
    for base in candidate_crystal_releases_api_bases(api_base_url) {
        match fetch_release_index_via_github_api(client, &base, host_slug) {
            Ok(rows) if !rows.is_empty() => return Ok(rows),
            Ok(_) | Err(_) => {}
        }
    }
    fetch_release_index_via_releases_atom(client, host_slug)
}

/// Maps host to a selector token used against GitHub asset `name` patterns.
pub fn crystal_host_slug() -> EnvrResult<&'static str> {
    use std::env::consts::{ARCH, OS};
    match (OS, ARCH) {
        ("linux", "x86_64") => Ok("linux-x86_64"),
        ("linux", "aarch64") => Ok("linux-aarch64"),
        ("macos", "x86_64") | ("macos", "aarch64") => Ok("darwin-universal"),
        ("windows", "x86_64") => Ok("windows-x86_64-msvc"),
        ("windows", "aarch64") => Ok("windows-aarch64-gnu"),
        _ => Err(EnvrError::Validation(format!(
            "no official Crystal GitHub asset mapping for host {OS}-{ARCH}; see docs/runtime/crystal-integration-plan.md"
        ))),
    }
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

/// `tag_name` must look like a stable `x.y.z` (numeric segments, at least three).
pub fn is_stable_crystal_tag(tag: &str) -> bool {
    let t = tag.trim().trim_start_matches('v').trim_start_matches('V');
    let parts: Vec<&str> = t.split('.').collect();
    if parts.len() < 3 {
        return false;
    }
    parts
        .iter()
        .take(3)
        .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
}

fn normalize_sha256_digest(d: &str) -> String {
    let t = d.trim();
    t.strip_prefix("sha256:").unwrap_or(t).trim().to_string()
}

/// Pick `(browser_download_url, optional_sha256_hex)` for this host from one release's `assets` array.
pub fn pick_crystal_asset_for_host(
    assets: &[Value],
    host_slug: &str,
) -> Option<(String, Option<String>)> {
    for a in assets {
        let name = a.get("name").and_then(|x| x.as_str())?;
        let url = a.get("browser_download_url").and_then(|x| x.as_str())?;
        if url.is_empty() {
            continue;
        }
        let ok = match host_slug {
            "linux-x86_64" => name.ends_with("-linux-x86_64.tar.gz") && !name.contains("bundled"),
            "linux-aarch64" => name.ends_with("-linux-aarch64.tar.gz") && !name.contains("bundled"),
            "darwin-universal" => name.contains("darwin-universal") && name.ends_with(".tar.gz"),
            "windows-x86_64-msvc" => name.contains("windows-x86_64-msvc") && name.ends_with(".zip"),
            "windows-aarch64-gnu" => name.contains("windows-aarch64-gnu") && name.ends_with(".zip"),
            _ => false,
        };
        if !ok {
            continue;
        }
        let sha = a
            .get("digest")
            .and_then(|x| x.as_str())
            .map(normalize_sha256_digest)
            .filter(|s| !s.is_empty());
        return Some((url.to_string(), sha));
    }
    None
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrystalReleaseRow {
    pub version: String,
    pub download_url: String,
    pub sha256_hex: Option<String>,
}

/// Deserialize rows written to disk cache (not GitHub API shape).
pub fn parse_cached_install_rows(json: &str) -> EnvrResult<Vec<CrystalReleaseRow>> {
    serde_json::from_str(json).map_err(|e| {
        EnvrError::with_source(
            ErrorCode::Validation,
            "invalid crystal release rows json",
            e,
        )
    })
}

/// Parse GitHub releases **array** JSON into installable rows for `host_slug` (newest first).
pub fn parse_github_releases_for_host(
    json: &str,
    host_slug: &str,
) -> EnvrResult<Vec<CrystalReleaseRow>> {
    let v: Value = serde_json::from_str(json).map_err(|e| {
        EnvrError::with_source(ErrorCode::Validation, "invalid crystal releases json", e)
    })?;
    let arr = v
        .as_array()
        .ok_or_else(|| EnvrError::Validation("GitHub releases JSON must be an array".into()))?;
    let mut out = Vec::new();
    for rel in arr {
        let Some(obj) = rel.as_object() else {
            continue;
        };
        if obj.get("draft").and_then(|x| x.as_bool()) == Some(true) {
            continue;
        }
        if obj.get("prerelease").and_then(|x| x.as_bool()) == Some(true) {
            continue;
        }
        let tag = obj
            .get("tag_name")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .trim();
        if tag.is_empty() || !is_stable_crystal_tag(tag) {
            continue;
        }
        let version = tag
            .trim_start_matches('v')
            .trim_start_matches('V')
            .to_string();
        let assets = obj
            .get("assets")
            .and_then(|x| x.as_array())
            .map(|x| x.as_slice())
            .unwrap_or(&[]);
        let Some((url, sha)) = pick_crystal_asset_for_host(assets, host_slug) else {
            continue;
        };
        out.push(CrystalReleaseRow {
            version,
            download_url: url,
            sha256_hex: sha,
        });
    }
    out.sort_by(|a, b| cmp_semver_release_labels(&b.version, &a.version));
    Ok(out)
}

pub fn list_remote_versions(
    rows: &[CrystalReleaseRow],
    filter: &RemoteFilter,
) -> Vec<RuntimeVersion> {
    let mut keys: Vec<String> = rows.iter().map(|r| r.version.clone()).collect();
    if let Some(prefix) = filter.prefix.as_deref() {
        let p = prefix.trim();
        if !p.is_empty() {
            keys.retain(|k| k.starts_with(p));
        }
    }
    keys.into_iter().map(RuntimeVersion).collect()
}

pub fn list_remote_latest_per_major_lines(rows: &[CrystalReleaseRow]) -> Vec<RuntimeVersion> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for r in rows {
        if let Some(line) = version_line_key_for_kind(RuntimeKind::Crystal, &r.version) {
            if seen.insert(line) {
                out.push(RuntimeVersion(r.version.clone()));
            }
        }
    }
    out
}

pub fn resolve_crystal_version(rows: &[CrystalReleaseRow], spec: &str) -> EnvrResult<String> {
    let s = spec.trim().trim_start_matches('v').trim_start_matches('V');
    if s.is_empty() {
        return Err(EnvrError::Validation("empty crystal version spec".into()));
    }
    let candidates: Vec<String> = rows.iter().map(|r| r.version.clone()).collect();
    if candidates.iter().any(|k| k == s) {
        return Ok(s.to_string());
    }

    use envr_domain::runtime::numeric_version_segments;
    if let Some(parts) = numeric_version_segments(s) {
        match parts.len() {
            1 => {
                let major = parts[0];
                let best = candidates
                    .iter()
                    .filter(|k| {
                        numeric_version_segments(k).is_some_and(|p| !p.is_empty() && p[0] == major)
                    })
                    .max_by(|a, b| cmp_semver_release_labels(a, b))
                    .map(|x| x.as_str());
                return best
                    .ok_or_else(|| {
                        EnvrError::Validation(format!(
                            "no crystal release matches major `{s}` for this host"
                        ))
                    })
                    .map(|x| x.to_string());
            }
            2 => {
                let line = format!("{}.{}", parts[0], parts[1]);
                let best = candidates
                    .iter()
                    .filter(|k| {
                        version_line_key_for_kind(RuntimeKind::Crystal, k).as_deref()
                            == Some(line.as_str())
                    })
                    .max_by(|a, b| cmp_semver_release_labels(a, b))
                    .map(|x| x.as_str());
                return best
                    .ok_or_else(|| {
                        EnvrError::Validation(format!(
                            "no crystal release matches line `{line}` for this host"
                        ))
                    })
                    .map(|x| x.to_string());
            }
            _ => {
                if parts.len() >= 3 {
                    let best = candidates
                        .iter()
                        .filter(|k| {
                            numeric_version_segments(k).is_some_and(|p| {
                                p.len() >= 3
                                    && p[0] == parts[0]
                                    && p[1] == parts[1]
                                    && p[2] == parts[2]
                            })
                        })
                        .max_by(|a, b| cmp_semver_release_labels(a, b))
                        .map(|x| x.as_str());
                    if let Some(b) = best {
                        return Ok(b.to_string());
                    }
                }
            }
        }
    }

    let pfx = s.to_string();
    candidates
        .iter()
        .filter(|k| k.starts_with(&pfx))
        .max_by(|a, b| cmp_semver_release_labels(a, b))
        .cloned()
        .ok_or_else(|| {
            EnvrError::Validation(format!(
                "could not resolve crystal version `{spec}` for this host"
            ))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_parses_linux_x86_64_rows() {
        let json = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/crystal_releases_snippet.json"),
        )
        .expect("read");
        let rows = parse_github_releases_for_host(&json, "linux-x86_64").expect("parse");
        assert!(rows.iter().any(|r| r.version == "1.20.0"));
        let r = rows.iter().find(|r| r.version == "1.20.0").expect("row");
        assert!(r.download_url.contains("linux-x86_64.tar.gz"));
        assert!(!r.download_url.contains("bundled"));
    }

    #[test]
    fn resolve_line_and_exact() {
        let json = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/crystal_releases_snippet.json"),
        )
        .expect("read");
        let rows = parse_github_releases_for_host(&json, "linux-x86_64").expect("parse");
        assert_eq!(
            resolve_crystal_version(&rows, "1.20").expect("line"),
            "1.20.0"
        );
        assert_eq!(
            resolve_crystal_version(&rows, "1.20.0").expect("exact"),
            "1.20.0"
        );
    }

    #[test]
    fn atom_snippet_collects_stable_tags_only() {
        let xml = r#"<feed>
        <entry><link href="https://github.com/crystal-lang/crystal/releases/tag/1.20.0"/></entry>
        <entry><link href="https://github.com/crystal-lang/crystal/releases/tag/2.0.0-rc1"/></entry>
        <entry><link href="https://github.com/crystal-lang/crystal/releases/tag/1.19.2"/></entry>
    </feed>"#;
        let mut seen = HashSet::new();
        let mut tags = Vec::new();
        for cap in ATOM_RELEASE_TAG_RE.captures_iter(xml) {
            let t = cap.get(1).unwrap().as_str();
            if is_stable_crystal_tag(t) && seen.insert(t.to_string()) {
                tags.push(t.to_string());
            }
        }
        assert!(tags.contains(&"1.20.0".to_string()));
        assert!(tags.contains(&"1.19.2".to_string()));
        assert!(!tags.iter().any(|x| x.contains("rc")));
    }

    #[test]
    fn synthetic_linux_url_matches_official_layout() {
        let u = synthetic_crystal_release_asset_url("1.20.0", "linux-x86_64").expect("url");
        assert_eq!(
            u,
            "https://github.com/crystal-lang/crystal/releases/download/1.20.0/crystal-1.20.0-1-linux-x86_64.tar.gz"
        );
    }

    #[test]
    fn strip_proxy_prefix_recover_api_url() {
        let wrapped =
            "https://ghproxy.net/https://api.github.com/repos/crystal-lang/crystal/releases";
        assert_eq!(
            strip_known_github_api_proxy_prefix(wrapped).as_deref(),
            Some("https://api.github.com/repos/crystal-lang/crystal/releases")
        );
    }

    #[test]
    fn candidate_api_bases_dedupes_direct_url() {
        let primary = "https://api.github.com/repos/crystal-lang/crystal/releases";
        let v = candidate_crystal_releases_api_bases(primary);
        assert_eq!(
            v,
            vec!["https://api.github.com/repos/crystal-lang/crystal/releases".to_string()]
        );
    }
}
