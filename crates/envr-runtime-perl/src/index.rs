//! Perl: Windows uses **Strawberry Perl** portable zips from `StrawberryPerl/Perl-Dist-Strawberry`;
//! Linux/macOS use **skaji/relocatable-perl** tarballs.

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

pub const DEFAULT_STRAWBERRY_RELEASES_URL: &str =
    "https://api.github.com/repos/StrawberryPerl/Perl-Dist-Strawberry/releases";

pub const DEFAULT_RELOCATABLE_RELEASES_URL: &str =
    "https://api.github.com/repos/skaji/relocatable-perl/releases";

const RELOCATABLE_RELEASES_ATOM_URL: &str =
    "https://github.com/skaji/relocatable-perl/releases.atom";

const STRAWBERRY_RELEASES_ATOM_URL: &str =
    "https://github.com/StrawberryPerl/Perl-Dist-Strawberry/releases.atom";

static RELOCATABLE_ATOM_TAG_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"https://github\.com/skaji/relocatable-perl/releases/tag/([^"<>]+)"#)
        .expect("relocatable atom tag regex")
});

static STRAWBERRY_ATOM_TAG_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"https://github\.com/StrawberryPerl/Perl-Dist-Strawberry/releases/tag/([^"<>]+)"#)
        .expect("strawberry atom tag regex")
});

/// Stable release tags like `SP_54221_64bit` / `SP_54021_64bit_UCRT` (five digits encode x.y.z.w).
static STRAWBERRY_STABLE_SP_TAG_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^SP_(\d{5})_64bit(?:_UCRT)?$").expect("strawberry stable sp tag regex")
});

static STRAWBERRY_PORTABLE_ZIP_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^strawberry-perl-(\d+(?:\.\d+)*)-64bit-portable\.zip$")
        .expect("strawberry portable zip regex")
});

pub fn blocking_http_client() -> EnvrResult<reqwest::blocking::Client> {
    build_blocking_http_client(
        concat!(
            "envr-runtime-perl/",
            env!("CARGO_PKG_VERSION"),
            " (https://www.perl.org; envr)"
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PerlUpstream {
    StrawberryWindows64,
    RelocatableUnix,
}

pub fn perl_upstream() -> EnvrResult<PerlUpstream> {
    use std::env::consts::{ARCH, OS};
    match (OS, ARCH) {
        ("windows", "x86_64") => Ok(PerlUpstream::StrawberryWindows64),
        ("linux", "x86_64") | ("linux", "aarch64") | ("macos", "x86_64") | ("macos", "aarch64") => {
            Ok(PerlUpstream::RelocatableUnix)
        }
        _ => Err(EnvrError::Validation(format!(
            "no managed Perl distribution mapping for host {OS}-{ARCH}; see docs/runtime/perl-integration-plan.md"
        ))),
    }
}

/// Base name without extension for the relocatable-perl archive for this host (e.g. `perl-linux-amd64`).
pub fn relocatable_archive_stem() -> EnvrResult<&'static str> {
    use std::env::consts::{ARCH, OS};
    match (OS, ARCH) {
        ("linux", "x86_64") => Ok("perl-linux-amd64"),
        ("linux", "aarch64") => Ok("perl-linux-arm64"),
        ("macos", "x86_64") => Ok("perl-darwin-amd64"),
        ("macos", "aarch64") => Ok("perl-darwin-arm64"),
        _ => Err(EnvrError::Validation(
            "relocatable-perl stem requested on unsupported host".into(),
        )),
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

fn is_stable_relocatable_tag(tag: &str) -> bool {
    let t = tag.trim().trim_start_matches('v').trim_start_matches('V');
    let parts: Vec<&str> = t.split('.').collect();
    if parts.len() < 3 {
        return false;
    }
    parts
        .iter()
        .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
}

fn fetch_release_pages(client: &reqwest::blocking::Client, base_url: &str) -> EnvrResult<String> {
    let mut merged = String::from("[");
    let mut first = true;
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
        let arr = v
            .as_array()
            .ok_or_else(|| EnvrError::Validation("GitHub releases JSON must be an array".into()))?;
        for rel in arr {
            if !first {
                merged.push(',');
            }
            first = false;
            merged.push_str(&serde_json::to_string(rel).map_err(|e| {
                EnvrError::with_source(
                    ErrorCode::Validation,
                    "serialize github release page entry",
                    e,
                )
            })?);
        }
        if page_len < 100 {
            break;
        }
    }
    merged.push(']');
    Ok(merged)
}

fn parse_strawberry_rows(json: &str) -> EnvrResult<Vec<PerlReleaseRow>> {
    let v: Value = serde_json::from_str(json).map_err(|e| {
        EnvrError::with_source(ErrorCode::Validation, "invalid github releases json", e)
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
        let assets = obj
            .get("assets")
            .and_then(|x| x.as_array())
            .map(|x| x.as_slice())
            .unwrap_or(&[]);
        for a in assets {
            let Some(name) = a.get("name").and_then(|x| x.as_str()) else {
                continue;
            };
            let Some(cap) = STRAWBERRY_PORTABLE_ZIP_RE.captures(name) else {
                continue;
            };
            let ver = cap.get(1).map(|m| m.as_str()).unwrap_or("").to_string();
            if ver.is_empty() {
                continue;
            }
            let Some(url) = a.get("browser_download_url").and_then(|x| x.as_str()) else {
                continue;
            };
            if url.is_empty() {
                continue;
            }
            let sha = a
                .get("digest")
                .and_then(|x| x.as_str())
                .map(|d| {
                    d.trim()
                        .strip_prefix("sha256:")
                        .unwrap_or(d)
                        .trim()
                        .to_string()
                })
                .filter(|s| !s.is_empty());
            out.push(PerlReleaseRow {
                version: ver,
                download_url: url.to_string(),
                sha256_hex: sha,
            });
        }
    }
    out.sort_by(|a, b| cmp_semver_release_labels(&b.version, &a.version));
    Ok(out)
}

fn asset_sha256(a: &Value) -> Option<String> {
    a.get("digest")
        .and_then(|x| x.as_str())
        .map(|d| {
            d.trim()
                .strip_prefix("sha256:")
                .unwrap_or(d)
                .trim()
                .to_string()
        })
        .filter(|s| !s.is_empty())
}

fn pick_skaji_asset_url(assets: &[Value], stem: &str) -> Option<(String, Option<String>)> {
    let xz = format!("{stem}.tar.xz");
    let gz = format!("{stem}.tar.gz");
    for a in assets {
        let name = a.get("name").and_then(|x| x.as_str())?;
        if name != xz {
            continue;
        }
        let url = a.get("browser_download_url").and_then(|x| x.as_str())?;
        if url.is_empty() {
            continue;
        }
        return Some((url.to_string(), asset_sha256(a)));
    }
    for a in assets {
        let name = a.get("name").and_then(|x| x.as_str())?;
        if name != gz {
            continue;
        }
        let url = a.get("browser_download_url").and_then(|x| x.as_str())?;
        if url.is_empty() {
            continue;
        }
        return Some((url.to_string(), asset_sha256(a)));
    }
    None
}

fn parse_skaji_rows(json: &str, stem: &str) -> EnvrResult<Vec<PerlReleaseRow>> {
    let v: Value = serde_json::from_str(json).map_err(|e| {
        EnvrError::with_source(ErrorCode::Validation, "invalid github releases json", e)
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
        if tag.is_empty() || !is_stable_relocatable_tag(tag) {
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
        let Some((download_url, sha256_hex)) = pick_skaji_asset_url(assets, stem) else {
            continue;
        };
        out.push(PerlReleaseRow {
            version,
            download_url,
            sha256_hex,
        });
    }
    out.sort_by(|a, b| cmp_semver_release_labels(&b.version, &a.version));
    Ok(out)
}

fn synthetic_skaji_download_url(tag: &str, stem: &str, prefer_xz: bool) -> Option<String> {
    let ext = if prefer_xz { "tar.xz" } else { "tar.gz" };
    Some(format!(
        "https://github.com/skaji/relocatable-perl/releases/download/{tag}/{stem}.{ext}"
    ))
}

/// Decode `SP_54221_64bit` style tags to `5.42.2.1` (five digits: major, minor two, patch, rev).
fn version_from_strawberry_stable_tag(tag: &str) -> Option<String> {
    let t = tag.trim();
    if t.to_ascii_lowercase().contains("beta") {
        return None;
    }
    if t.to_ascii_lowercase().starts_with("dev_") {
        return None;
    }
    let cap = STRAWBERRY_STABLE_SP_TAG_RE.captures(t)?;
    let digits = cap.get(1)?.as_str();
    if digits.len() != 5 {
        return None;
    }
    let major = digits[0..1].parse::<u64>().ok()?;
    let minor = digits[1..3].parse::<u64>().ok()?;
    let patch = digits[3..4].parse::<u64>().ok()?;
    let rev = digits[4..5].parse::<u64>().ok()?;
    Some(format!("{major}.{minor}.{patch}.{rev}"))
}

fn synthetic_strawberry_portable_zip_url(tag: &str, version_label: &str) -> String {
    format!(
        "https://github.com/StrawberryPerl/Perl-Dist-Strawberry/releases/download/{tag}/strawberry-perl-{version_label}-64bit-portable.zip"
    )
}

fn fetch_strawberry_via_atom(
    client: &reqwest::blocking::Client,
) -> EnvrResult<Vec<PerlReleaseRow>> {
    let mut seen_tags = HashSet::new();
    let mut tags_in_order = Vec::new();
    for page in 1_u32..=50 {
        let url = if page == 1 {
            STRAWBERRY_RELEASES_ATOM_URL.to_string()
        } else {
            format!("{STRAWBERRY_RELEASES_ATOM_URL}?page={page}")
        };
        let body = fetch_text(client, &url)?;
        let mut new_this_page = 0usize;
        for cap in STRAWBERRY_ATOM_TAG_RE.captures_iter(&body) {
            let Some(m) = cap.get(1) else {
                continue;
            };
            let tag = m.as_str().trim().trim_end_matches('/');
            if tag.is_empty() {
                continue;
            }
            if seen_tags.insert(tag.to_string()) {
                tags_in_order.push(tag.to_string());
                new_this_page += 1;
            }
        }
        if page == 1 && new_this_page == 0 && seen_tags.is_empty() {
            return Err(EnvrError::Download(
                "perl: Strawberry releases.atom contained no release links".into(),
            ));
        }
        if page > 1 && new_this_page == 0 {
            break;
        }
    }
    let mut rows = Vec::new();
    let mut seen_versions = HashSet::new();
    for tag in tags_in_order {
        let Some(version) = version_from_strawberry_stable_tag(&tag) else {
            continue;
        };
        if !seen_versions.insert(version.clone()) {
            continue;
        }
        rows.push(PerlReleaseRow {
            version: version.clone(),
            download_url: synthetic_strawberry_portable_zip_url(&tag, &version),
            sha256_hex: None,
        });
    }
    rows.sort_by(|a, b| cmp_semver_release_labels(&b.version, &a.version));
    if rows.is_empty() {
        return Err(EnvrError::Download(
            "perl: Strawberry atom index produced no portable rows for this host".into(),
        ));
    }
    Ok(rows)
}

fn fetch_relocatable_via_atom(
    client: &reqwest::blocking::Client,
    stem: &str,
) -> EnvrResult<Vec<PerlReleaseRow>> {
    let mut seen_tags = HashSet::new();
    let mut tags_in_order = Vec::new();
    for page in 1_u32..=50 {
        let url = if page == 1 {
            RELOCATABLE_RELEASES_ATOM_URL.to_string()
        } else {
            format!("{RELOCATABLE_RELEASES_ATOM_URL}?page={page}")
        };
        let body = fetch_text(client, &url)?;
        let mut new_this_page = 0usize;
        for cap in RELOCATABLE_ATOM_TAG_RE.captures_iter(&body) {
            let Some(m) = cap.get(1) else {
                continue;
            };
            let tag = m.as_str().trim().trim_end_matches('/');
            if tag.is_empty() || !is_stable_relocatable_tag(tag) {
                continue;
            }
            if seen_tags.insert(tag.to_string()) {
                tags_in_order.push(tag.to_string());
                new_this_page += 1;
            }
        }
        if page == 1 && new_this_page == 0 && seen_tags.is_empty() {
            return Err(EnvrError::Download(
                "perl: relocatable releases.atom contained no stable release links".into(),
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
        let download_url = synthetic_skaji_download_url(&tag, stem, true)
            .or_else(|| synthetic_skaji_download_url(&tag, stem, false))
            .ok_or_else(|| {
                EnvrError::Download("perl: could not build skaji download URL".into())
            })?;
        rows.push(PerlReleaseRow {
            version,
            download_url,
            sha256_hex: None,
        });
    }
    rows.sort_by(|a, b| cmp_semver_release_labels(&b.version, &a.version));
    if rows.is_empty() {
        return Err(EnvrError::Download(
            "perl: atom index produced no installable rows for this host".into(),
        ));
    }
    Ok(rows)
}

pub fn fetch_all_perl_release_rows(
    client: &reqwest::blocking::Client,
    api_base_url: &str,
    upstream: PerlUpstream,
) -> EnvrResult<Vec<PerlReleaseRow>> {
    match upstream {
        PerlUpstream::StrawberryWindows64 => {
            for base in candidate_api_bases(api_base_url, DEFAULT_STRAWBERRY_RELEASES_URL) {
                match fetch_release_pages(client, &base) {
                    Ok(merged) => {
                        if let Ok(rows) = parse_strawberry_rows(&merged) {
                            if !rows.is_empty() {
                                return Ok(rows);
                            }
                        }
                    }
                    Err(_) => continue,
                }
            }
            fetch_strawberry_via_atom(client)
        }
        PerlUpstream::RelocatableUnix => {
            let stem = relocatable_archive_stem()?;
            for base in candidate_api_bases(api_base_url, DEFAULT_RELOCATABLE_RELEASES_URL) {
                let merged = match fetch_release_pages(client, &base) {
                    Ok(m) => m,
                    Err(_) => continue,
                };
                let rows = parse_skaji_rows(&merged, stem)?;
                if !rows.is_empty() {
                    return Ok(rows);
                }
            }
            fetch_relocatable_via_atom(client, stem)
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerlReleaseRow {
    pub version: String,
    pub download_url: String,
    pub sha256_hex: Option<String>,
}

pub fn parse_cached_install_rows(json: &str) -> EnvrResult<Vec<PerlReleaseRow>> {
    serde_json::from_str(json).map_err(|e| {
        EnvrError::with_source(ErrorCode::Validation, "invalid cached install rows json", e)
    })
}

pub fn list_remote_versions(rows: &[PerlReleaseRow], filter: &RemoteFilter) -> Vec<RuntimeVersion> {
    let mut keys: Vec<String> = rows.iter().map(|r| r.version.clone()).collect();
    if let Some(prefix) = filter.prefix.as_deref() {
        let p = prefix.trim();
        if !p.is_empty() {
            keys.retain(|k| k.starts_with(p));
        }
    }
    keys.into_iter().map(RuntimeVersion).collect()
}

pub fn list_remote_latest_per_major_lines(rows: &[PerlReleaseRow]) -> Vec<RuntimeVersion> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for r in rows {
        if let Some(line) = version_line_key_for_kind(RuntimeKind::Perl, &r.version) {
            if seen.insert(line) {
                out.push(RuntimeVersion(r.version.clone()));
            }
        }
    }
    out
}

pub fn resolve_perl_version(rows: &[PerlReleaseRow], spec: &str) -> EnvrResult<String> {
    let s = spec.trim().trim_start_matches('v').trim_start_matches('V');
    if s.is_empty() {
        return Err(EnvrError::Validation("empty perl version spec".into()));
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
                            "no perl release matches major `{s}` for this host"
                        ))
                    })
                    .map(|x| x.to_string());
            }
            2 => {
                let line = format!("{}.{}", parts[0], parts[1]);
                let best = candidates
                    .iter()
                    .filter(|k| {
                        version_line_key_for_kind(RuntimeKind::Perl, k).as_deref()
                            == Some(line.as_str())
                    })
                    .max_by(|a, b| cmp_semver_release_labels(a, b))
                    .map(|x| x.as_str());
                return best
                    .ok_or_else(|| {
                        EnvrError::Validation(format!(
                            "no perl release matches line `{line}` for this host"
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
                "could not resolve perl version `{spec}` for this host"
            ))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strawberry_sp_tag_decodes_portable_version() {
        assert_eq!(
            version_from_strawberry_stable_tag("SP_54221_64bit").as_deref(),
            Some("5.42.2.1")
        );
        assert_eq!(
            version_from_strawberry_stable_tag("SP_54021_64bit_UCRT").as_deref(),
            Some("5.40.2.1")
        );
        assert_eq!(
            version_from_strawberry_stable_tag("SP_54022_64bit").as_deref(),
            Some("5.40.2.2")
        );
        assert!(version_from_strawberry_stable_tag("SP_54221_64bit_beta1").is_none());
        assert!(version_from_strawberry_stable_tag("dev_54201_beta1_20250709").is_none());
    }

    #[test]
    fn strawberry_fixture_parses_portable_rows() {
        let json = r#"[
          {"draft": false, "prerelease": false, "assets": [
            {"name": "strawberry-perl-5.40.2.1-64bit-portable.zip",
             "browser_download_url": "https://example.com/p.zip"}
          ]}
        ]"#;
        let rows = parse_strawberry_rows(json).expect("parse");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].version, "5.40.2.1");
    }

    #[test]
    fn skaji_fixture_prefers_tar_xz() {
        let stem = "perl-linux-amd64";
        let json = r#"[
          {"draft": false, "prerelease": false, "tag_name": "5.42.2.0", "assets": [
            {"name": "perl-linux-amd64.tar.gz", "browser_download_url": "https://example.com/a.tar.gz"},
            {"name": "perl-linux-amd64.tar.xz", "browser_download_url": "https://example.com/a.tar.xz"}
          ]}
        ]"#;
        let rows = parse_skaji_rows(json, stem).expect("parse");
        assert_eq!(rows[0].version, "5.42.2.0");
        assert!(rows[0].download_url.ends_with(".tar.xz"));
    }

    #[test]
    fn resolve_major_minor_exact() {
        let rows = vec![
            PerlReleaseRow {
                version: "5.40.1.0".into(),
                download_url: "u1".into(),
                sha256_hex: None,
            },
            PerlReleaseRow {
                version: "5.40.2.1".into(),
                download_url: "u2".into(),
                sha256_hex: None,
            },
        ];
        assert_eq!(
            resolve_perl_version(&rows, "5.40").expect("maj"),
            "5.40.2.1"
        );
        assert_eq!(
            resolve_perl_version(&rows, "5.40.2").expect("line"),
            "5.40.2.1"
        );
        assert_eq!(
            resolve_perl_version(&rows, "5.40.2.1").expect("exact"),
            "5.40.2.1"
        );
    }
}
