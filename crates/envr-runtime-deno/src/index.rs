use envr_config::settings::DENO_NPMIRROR_BINARY_BASE;
use envr_domain::runtime::{RemoteFilter, RuntimeVersion};
use envr_download::blocking::build_blocking_http_client;
use envr_error::{EnvrError, EnvrResult, ErrorCode};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

pub const DEFAULT_DENO_TAGS_API: &str =
    "https://api.github.com/repos/denoland/deno/tags?per_page=100";

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Tag {
    pub name: String,
}

pub fn blocking_http_client() -> EnvrResult<reqwest::blocking::Client> {
    build_blocking_http_client(
        concat!("envr-runtime-deno/", env!("CARGO_PKG_VERSION")),
        Some(Duration::from_secs(45)),
    )
}

pub fn fetch_tags(client: &reqwest::blocking::Client, url: &str) -> EnvrResult<String> {
    envr_runtime_github_release::fetch_text(client, url)
}

fn max_tag_pages() -> usize {
    const DEFAULT: usize = 2;
    std::env::var("ENVR_DENO_TAGS_MAX_PAGES")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .filter(|n| *n > 0)
        .unwrap_or(DEFAULT)
}

#[derive(Debug, Clone, Deserialize)]
struct MirrorEntry {
    name: String,
    #[allow(dead_code)]
    r#type: Option<String>,
}

fn fetch_tags_from_npmmirror(client: &reqwest::blocking::Client) -> EnvrResult<Vec<Tag>> {
    let index_url = format!("{DENO_NPMIRROR_BINARY_BASE}/");
    let text = envr_runtime_github_release::fetch_text(client, &index_url)?;
    let entries: Vec<MirrorEntry> = serde_json::from_str(&text).map_err(|e| {
        EnvrError::with_source(
            ErrorCode::Validation,
            "invalid npmmirror deno index json",
            e,
        )
    })?;
    let mut out = Vec::new();
    for e in entries {
        let raw = e.name.trim().trim_end_matches('/');
        if !raw.starts_with('v') {
            continue;
        }
        if normalize_deno_version(raw).is_none() {
            continue;
        }
        out.push(Tag {
            name: raw.to_string(),
        });
    }
    Ok(out)
}

/// Fetches paginated tags from GitHub until empty/short page.
pub fn fetch_all_tags(client: &reqwest::blocking::Client, start_url: &str) -> EnvrResult<Vec<Tag>> {
    let mut all = Vec::new();
    let max_pages = max_tag_pages();
    for page in 1..=max_pages {
        let url = if page == 1 {
            start_url.to_string()
        } else {
            let sep = if start_url.contains('?') { '&' } else { '?' };
            format!("{start_url}{sep}page={page}")
        };
        let body = match fetch_tags(client, &url) {
            Ok(t) => t,
            Err(e) => {
                if !all.is_empty() {
                    break;
                }
                // GitHub API can be rate-limited in some regions/IPs; fallback to npmmirror index.
                if let Ok(mirror_tags) = fetch_tags_from_npmmirror(client)
                    && !mirror_tags.is_empty()
                {
                    return Ok(mirror_tags);
                }
                return Err(e);
            }
        };
        let mut page = parse_tags(&body)?;
        if page.is_empty() {
            break;
        }
        let page_len = page.len();
        all.append(&mut page);
        if page_len < 100 {
            break;
        }
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

pub fn normalize_deno_version(tag: &str) -> Option<String> {
    let t = tag.trim();
    let rest = t.strip_prefix('v').unwrap_or(t);
    semver_key(rest)?;
    Some(rest.to_string())
}

/// Latest patch version per **major** line (e.g. one row per `1.x`, `2.x`), newest majors first.
pub fn list_latest_patch_per_major_from_tags(tags: &[Tag]) -> Vec<String> {
    let mut best: HashMap<u64, (SemKey, String)> = HashMap::new();
    for t in tags {
        let Some(v) = normalize_deno_version(&t.name) else {
            continue;
        };
        let Some(k) = semver_key(&v) else {
            continue;
        };
        // Keep in sync with [`list_remote_versions`]: 0.x is not supported for managed install.
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
        .filter_map(|t| {
            normalize_deno_version(&t.name).and_then(|v| semver_key(&v).map(|k| (k, v)))
        })
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

pub fn resolve_deno_version(tags: &[Tag], spec: &str) -> EnvrResult<String> {
    let s = spec.trim().trim_start_matches('v').to_ascii_lowercase();
    if s.is_empty() {
        return Err(EnvrError::Validation("empty deno version spec".into()));
    }
    if s.starts_with("0.") {
        return Err(EnvrError::Validation(format!(
            "deno 0.x is not supported for managed install (spec {spec:?})"
        )));
    }
    let mut items: Vec<(SemKey, String)> = tags
        .iter()
        .filter_map(|t| {
            normalize_deno_version(&t.name).and_then(|v| semver_key(&v).map(|k| (k, v)))
        })
        .filter(|(k, _)| k.0 >= 1)
        .collect();
    if items.is_empty() {
        return Err(EnvrError::Validation("no deno versions in index".into()));
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
        "no Deno release matches spec {spec:?}"
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn latest_per_major_skips_uninstallable_zero_major_line() {
        let tags = vec![
            Tag {
                name: "v0.8.1".into(),
            },
            Tag {
                name: "v1.0.0".into(),
            },
        ];
        let latest = list_latest_patch_per_major_from_tags(&tags);
        assert_eq!(latest, vec!["1.0.0"]);
    }
}
