use envr_domain::runtime::{RemoteFilter, RuntimeVersion};
use envr_download::blocking::build_blocking_http_client;
use envr_error::{EnvrError, EnvrResult, ErrorCode};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

pub const DEFAULT_BUN_TAGS_API: &str = "https://api.github.com/repos/oven-sh/bun/tags?per_page=100";

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Tag {
    pub name: String,
}

pub fn blocking_http_client() -> EnvrResult<reqwest::blocking::Client> {
    build_blocking_http_client(
        concat!("envr-runtime-bun/", env!("CARGO_PKG_VERSION")),
        Some(Duration::from_secs(45)),
    )
}

pub fn fetch_tags(client: &reqwest::blocking::Client, url: &str) -> EnvrResult<String> {
    let response = client.get(url).send().map_err(|e| {
        EnvrError::with_source(ErrorCode::Download, format!("request failed for {url}"), e)
    })?;
    if !response.status().is_success() {
        return Err(EnvrError::Download(format!(
            "GET {} -> {}",
            url,
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

fn parse_github_next_link(link: Option<&reqwest::header::HeaderValue>) -> Option<String> {
    let raw = link?.to_str().ok()?;
    for part in raw.split(',') {
        let part = part.trim();
        if !part.contains("rel=\"next\"") && !part.contains("rel=next") {
            continue;
        }
        let start = part.find('<')? + 1;
        let end = part.find('>')?;
        return Some(part[start..end].to_string());
    }
    None
}

fn max_tag_pages() -> usize {
    const DEFAULT: usize = 2;
    std::env::var("ENVR_BUN_TAGS_MAX_PAGES")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .filter(|n| *n > 0)
        .unwrap_or(DEFAULT)
}

/// Fetches all Bun tags via GitHub pagination (`Link: rel="next"`).
pub fn fetch_all_tags(client: &reqwest::blocking::Client, start_url: &str) -> EnvrResult<Vec<Tag>> {
    let mut all = Vec::new();
    let mut next = Some(start_url.to_string());
    let mut pages = 0usize;
    let max_pages = max_tag_pages();
    while let Some(url) = next.take() {
        if pages >= max_pages {
            break;
        }
        pages += 1;
        let response = match client.get(&url).send() {
            Ok(r) => r,
            Err(e) => {
                if !all.is_empty() {
                    break;
                }
                return Err(EnvrError::with_source(
                    ErrorCode::Download,
                    format!("request failed for {url}"),
                    e,
                ));
            }
        };
        if !response.status().is_success() {
            // GitHub may deny deep pagination (403) under stricter limits.
            // Keep already-fetched tags usable instead of failing all operations.
            if response.status() == reqwest::StatusCode::FORBIDDEN && !all.is_empty() {
                break;
            }
            return Err(EnvrError::Download(format!(
                "GET {url} -> {}",
                response.status()
            )));
        }
        let headers = response.headers().clone();
        let body = response.text().map_err(|e| {
            EnvrError::with_source(
                ErrorCode::Download,
                format!("read body failed for {url}"),
                e,
            )
        })?;
        let mut page = parse_tags(&body)?;
        all.append(&mut page);
        next = parse_github_next_link(headers.get("link"));
    }
    Ok(all)
}

pub fn parse_tags(json: &str) -> EnvrResult<Vec<Tag>> {
    serde_json::from_str(json)
        .map_err(|e| EnvrError::with_source(ErrorCode::Validation, "invalid github tags json", e))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct SemKey(u64, u64, u64);

fn semver_key(s: &str) -> Option<SemKey> {
    let s = s.trim().trim_start_matches('v');
    let base = s
        .split('-')
        .next()
        .unwrap_or(s)
        .split('+')
        .next()
        .unwrap_or(s);
    let mut p = base.split('.');
    let a: u64 = p.next()?.parse().ok()?;
    let b: u64 = p.next().unwrap_or("0").parse().ok()?;
    let c: u64 = p.next().unwrap_or("0").parse().ok()?;
    Some(SemKey(a, b, c))
}

pub fn normalize_bun_version(tag: &str) -> Option<String> {
    // Tags look like `bun-v1.3.11`
    let t = tag.trim();
    let rest = t
        .strip_prefix("bun-v")
        .or_else(|| t.strip_prefix("v"))
        .unwrap_or(t);
    semver_key(rest)?;
    Some(rest.to_string())
}

/// Latest patch version per major line, newest majors first.
pub fn list_latest_patch_per_major_from_tags(tags: &[Tag]) -> Vec<String> {
    let mut best: HashMap<u64, (SemKey, String)> = HashMap::new();
    for t in tags {
        let Some(v) = normalize_bun_version(&t.name) else {
            continue;
        };
        let Some(k) = semver_key(&v) else {
            continue;
        };
        // Keep in sync with [`list_remote_versions`]: 0.x is not installable via envr's release path.
        if k.0 < 1 {
            continue;
        }
        let major = k.0;
        let entry = best.entry(major).or_insert((k, v.clone()));
        if k > entry.0 {
            *entry = (k, v);
        }
    }
    let mut majors: Vec<u64> = best.keys().cloned().collect();
    majors.sort_by(|a, b| b.cmp(a));
    majors
        .into_iter()
        .filter_map(|m| best.remove(&m).map(|(_, s)| s))
        .collect()
}

pub fn list_remote_versions(
    tags: &[Tag],
    filter: &RemoteFilter,
) -> EnvrResult<Vec<RuntimeVersion>> {
    let mut items: Vec<(SemKey, String)> = tags
        .iter()
        .filter_map(|t| normalize_bun_version(&t.name).and_then(|v| semver_key(&v).map(|k| (k, v))))
        // Historical 0.x tags often don't publish installable release assets for envr's
        // current download path (`releases/download/bun-v*/bun-*.zip`), causing 404 on install.
        // Hide them from remote list and resolver to keep install UX consistent.
        .filter(|(k, _)| k.0 >= 1)
        .collect();
    items.sort_by(|a, b| b.0.cmp(&a.0));
    let mut out: Vec<RuntimeVersion> = items.into_iter().map(|(_, v)| RuntimeVersion(v)).collect();
    if let Some(prefix) = &filter.prefix {
        let p = prefix.trim().trim_start_matches('v').to_ascii_lowercase();
        if !p.is_empty() {
            out.retain(|v| v.0.to_ascii_lowercase().starts_with(&p));
        }
    }
    Ok(out)
}

pub fn resolve_bun_version(tags: &[Tag], spec: &str) -> EnvrResult<String> {
    let s = spec.trim().trim_start_matches('v').to_ascii_lowercase();
    if s.is_empty() {
        return Err(EnvrError::Validation("empty bun version spec".into()));
    }
    if s.starts_with("0.") {
        let msg = if cfg!(windows) {
            format!(
                "bun 0.x has no official Windows release assets; use bun >= 1.0 (spec {spec:?})"
            )
        } else {
            format!(
                "bun 0.x is not supported for managed install in envr; use bun >= 1.0 (spec {spec:?})"
            )
        };
        return Err(EnvrError::Validation(msg));
    }
    let mut items: Vec<(SemKey, String)> = tags
        .iter()
        .filter_map(|t| normalize_bun_version(&t.name).and_then(|v| semver_key(&v).map(|k| (k, v))))
        .filter(|(k, _)| k.0 >= 1)
        .collect();
    if items.is_empty() {
        return Err(EnvrError::Validation("no bun versions in index".into()));
    }
    items.sort_by(|a, b| b.0.cmp(&a.0));
    if s == "latest" {
        return Ok(items[0].1.clone());
    }
    if let Some((_, v)) = items.iter().find(|(_, v)| v.eq_ignore_ascii_case(&s)) {
        return Ok(v.clone());
    }
    if s.chars().all(|c| c.is_ascii_digit() || c == '.')
        && let Some((_, v)) = items.iter().find(|(_, v)| v.starts_with(&s))
    {
        return Ok(v.clone());
    }
    Err(EnvrError::Validation(format!(
        "no Bun release matches spec {spec:?}"
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latest_per_major_skips_uninstallable_zero_major_line() {
        let tags = vec![
            Tag {
                name: "bun-v0.8.1".into(),
            },
            Tag {
                name: "bun-v1.0.0".into(),
            },
        ];
        let latest = list_latest_patch_per_major_from_tags(&tags);
        assert_eq!(latest, vec!["1.0.0"]);
    }
}
