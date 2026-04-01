use envr_domain::runtime::{RemoteFilter, RuntimeVersion};
use envr_error::{EnvrError, EnvrResult};
use serde::Deserialize;
use std::time::Duration;

pub const DEFAULT_DENO_TAGS_API: &str =
    "https://api.github.com/repos/denoland/deno/tags?per_page=100";

#[derive(Debug, Clone, Deserialize)]
pub struct Tag {
    pub name: String,
}

pub fn blocking_http_client() -> EnvrResult<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(45))
        .user_agent(concat!("envr-runtime-deno/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| EnvrError::Download(e.to_string()))
}

pub fn fetch_tags(client: &reqwest::blocking::Client, url: &str) -> EnvrResult<String> {
    let response = client
        .get(url)
        .send()
        .map_err(|e| EnvrError::Download(e.to_string()))?;
    if !response.status().is_success() {
        return Err(EnvrError::Download(format!(
            "GET {} -> {}",
            url,
            response.status()
        )));
    }
    response
        .text()
        .map_err(|e| EnvrError::Download(e.to_string()))
}

pub fn parse_tags(json: &str) -> EnvrResult<Vec<Tag>> {
    serde_json::from_str(json).map_err(|e| EnvrError::Validation(e.to_string()))
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

pub fn list_remote_versions(
    tags: &[Tag],
    filter: &RemoteFilter,
) -> EnvrResult<Vec<RuntimeVersion>> {
    let mut items: Vec<(SemKey, String)> = tags
        .iter()
        .filter_map(|t| {
            normalize_deno_version(&t.name).and_then(|v| semver_key(&v).map(|k| (k, v)))
        })
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
    let mut items: Vec<(SemKey, String)> = tags
        .iter()
        .filter_map(|t| {
            normalize_deno_version(&t.name).and_then(|v| semver_key(&v).map(|k| (k, v)))
        })
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
