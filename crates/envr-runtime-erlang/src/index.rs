use envr_domain::runtime::{RemoteFilter, RuntimeVersion};
use envr_download::blocking::build_blocking_http_client;
use envr_error::{EnvrError, EnvrResult, ErrorCode};
use serde::Deserialize;
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::time::Duration;

pub const DEFAULT_GITHUB_TAGS_API: &str =
    "https://api.github.com/repos/erlang/otp/tags?per_page=100";

#[derive(Debug, Clone, Deserialize)]
pub struct GithubTag {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErlangRelease {
    pub version: String,
    pub url: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct SemKey {
    parts: Vec<u64>,
}

impl Ord for SemKey {
    fn cmp(&self, other: &Self) -> Ordering {
        let max_len = self.parts.len().max(other.parts.len());
        for i in 0..max_len {
            let a = self.parts.get(i).copied().unwrap_or(0);
            let b = other.parts.get(i).copied().unwrap_or(0);
            match a.cmp(&b) {
                Ordering::Equal => continue,
                non_eq => return non_eq,
            }
        }
        Ordering::Equal
    }
}

impl PartialOrd for SemKey {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

fn semver_key(s: &str) -> Option<SemKey> {
    let t = s.trim().trim_start_matches('v');
    let parts: Vec<u64> = t
        .split('.')
        .map(|p| p.parse::<u64>().ok())
        .collect::<Option<Vec<_>>>()?;
    if parts.is_empty() {
        return None;
    }
    Some(SemKey { parts })
}

pub(crate) fn cmp_semver(a: &str, b: &str) -> Ordering {
    semver_key(a).cmp(&semver_key(b))
}

pub fn normalize_otp_version(tag: &str) -> Option<String> {
    let t = tag.trim();
    let rest = t.strip_prefix("OTP-").or_else(|| t.strip_prefix("otp-"))?;
    if rest.contains("-rc") {
        return None;
    }
    semver_key(rest)?;
    Some(rest.to_string())
}

fn release_asset_url_for_host(version: &str) -> EnvrResult<String> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "x86_64") | ("windows", "aarch64") => Ok(format!(
            "https://github.com/erlang/otp/releases/download/OTP-{version}/otp_win64_{version}.zip"
        )),
        (os, arch) => Err(EnvrError::Platform(format!(
            "managed Erlang install is currently unsupported on {os}-{arch}; only Windows is supported"
        ))),
    }
}

fn release_from_tag(tag: &GithubTag) -> EnvrResult<Option<ErlangRelease>> {
    let Some(version) = normalize_otp_version(&tag.name) else {
        return Ok(None);
    };
    let url = release_asset_url_for_host(&version)?;
    Ok(Some(ErlangRelease { version, url }))
}

pub fn blocking_http_client() -> EnvrResult<reqwest::blocking::Client> {
    build_blocking_http_client(
        concat!("envr-runtime-erlang/", env!("CARGO_PKG_VERSION")),
        Some(Duration::from_secs(90)),
    )
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
    // GitHub tags are ordered by recency. A too-small page cap can hide still-supported
    // majors (e.g. OTP 27) from "latest per major" view when recent tags are dominated by OTP 28.
    const DEFAULT: usize = 8;
    std::env::var("ENVR_ERLANG_TAGS_MAX_PAGES")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .filter(|n| *n > 0)
        .unwrap_or(DEFAULT)
}

pub fn fetch_all_tags(
    client: &reqwest::blocking::Client,
    start_url: &str,
) -> EnvrResult<Vec<GithubTag>> {
    let mut out = Vec::<GithubTag>::new();
    let mut next = Some(start_url.to_string());
    let mut pages = 0usize;
    let max_pages = max_tag_pages();
    while let Some(url) = next.take() {
        if pages >= max_pages {
            break;
        }
        pages += 1;
        let response = client
            .get(&url)
            .send()
            .map_err(|e| EnvrError::with_source(ErrorCode::Download, format!("request failed for {url}"), e))?;
        if !response.status().is_success() {
            return Err(EnvrError::Download(format!(
                "GET {url} -> {}",
                response.status()
            )));
        }
        let headers = response.headers().clone();
        let body = response
            .text()
            .map_err(|e| EnvrError::with_source(ErrorCode::Download, format!("read body failed for {url}"), e))?;
        let mut page: Vec<GithubTag> =
            serde_json::from_str(&body)
                .map_err(|e| EnvrError::with_source(ErrorCode::Validation, "invalid github tags json", e))?;
        out.append(&mut page);
        next = parse_github_next_link(headers.get("link"));
    }
    Ok(out)
}

pub fn tags_to_releases(tags: &[GithubTag]) -> EnvrResult<Vec<ErlangRelease>> {
    let mut out = Vec::<ErlangRelease>::new();
    for tag in tags {
        if let Some(release) = release_from_tag(tag)? {
            out.push(release);
        }
    }
    out.sort_by(|a, b| cmp_semver(&b.version, &a.version));
    out.dedup_by(|a, b| a.version == b.version);
    Ok(out)
}

pub fn list_remote_versions(
    releases: &[ErlangRelease],
    filter: &RemoteFilter,
) -> EnvrResult<Vec<RuntimeVersion>> {
    let mut versions: Vec<RuntimeVersion> = releases
        .iter()
        .map(|r| RuntimeVersion(r.version.clone()))
        .collect();
    if let Some(prefix) = &filter.prefix {
        let p = prefix.trim().trim_start_matches('v').to_ascii_lowercase();
        if !p.is_empty() {
            versions.retain(|v| v.0.to_ascii_lowercase().starts_with(&p));
        }
    }
    versions.sort_by(|a, b| cmp_semver(&b.0, &a.0));
    versions.dedup_by(|a, b| a.0 == b.0);
    Ok(versions)
}

pub fn list_latest_per_major(releases: &[ErlangRelease]) -> EnvrResult<Vec<RuntimeVersion>> {
    let mut by_major = BTreeMap::<u64, String>::new();
    for release in releases {
        let Some(key) = semver_key(&release.version) else {
            continue;
        };
        let Some(major) = key.parts.first().copied() else {
            continue;
        };
        by_major
            .entry(major)
            .and_modify(|cur| {
                if cmp_semver(&release.version, cur) == Ordering::Greater {
                    *cur = release.version.clone();
                }
            })
            .or_insert_with(|| release.version.clone());
    }
    let mut out: Vec<RuntimeVersion> = by_major.into_values().map(RuntimeVersion).collect();
    out.sort_by(|a, b| cmp_semver(&b.0, &a.0));
    Ok(out)
}

pub fn resolve_erlang_version(releases: &[ErlangRelease], spec: &str) -> EnvrResult<String> {
    let raw = spec.trim().trim_start_matches('v');
    let parts: Vec<&str> = raw.split('.').collect();
    if parts.is_empty()
        || parts.len() > 4
        || parts
            .iter()
            .any(|p| p.is_empty() || !p.chars().all(|c| c.is_ascii_digit()))
    {
        return Err(EnvrError::Validation(format!(
            "unsupported erlang spec {spec:?}; use major (27), major.minor (27.3), or full (27.3.4.10)"
        )));
    }
    if let Some(exact) = releases.iter().find(|r| r.version == raw) {
        return Ok(exact.version.clone());
    }
    let mut candidates: Vec<&ErlangRelease> = releases
        .iter()
        .filter(|r| {
            let rp: Vec<&str> = r.version.split('.').collect();
            if parts.len() > rp.len() {
                return false;
            }
            parts.iter().zip(rp.iter()).all(|(want, got)| want == got)
        })
        .collect();
    candidates.sort_by(|a, b| cmp_semver(&a.version, &b.version));
    candidates
        .last()
        .map(|r| r.version.clone())
        .ok_or_else(|| EnvrError::Validation(format!("no erlang version matches spec {spec:?}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_releases() -> Vec<ErlangRelease> {
        vec![
            ErlangRelease {
                version: "26.2.5.19".into(),
                url: "https://example/26.zip".into(),
            },
            ErlangRelease {
                version: "27.3.4.8".into(),
                url: "https://example/27a.zip".into(),
            },
            ErlangRelease {
                version: "27.3.4.10".into(),
                url: "https://example/27b.zip".into(),
            },
        ]
    }

    #[test]
    fn normalize_otp_version_filters_rc_and_normalizes() {
        assert_eq!(
            normalize_otp_version("OTP-27.3.4.10").as_deref(),
            Some("27.3.4.10")
        );
        assert_eq!(normalize_otp_version("OTP-28.0-rc3"), None);
        assert_eq!(normalize_otp_version("v27.3.4.10"), None);
    }

    #[test]
    fn list_latest_per_major_picks_highest_patch() {
        let latest = list_latest_per_major(&sample_releases()).expect("latest");
        assert_eq!(
            latest,
            vec![
                RuntimeVersion("27.3.4.10".into()),
                RuntimeVersion("26.2.5.19".into())
            ]
        );
    }

    #[test]
    fn resolve_erlang_version_supports_major_minor_and_full() {
        let releases = sample_releases();
        assert_eq!(
            resolve_erlang_version(&releases, "27").expect("major"),
            "27.3.4.10"
        );
        assert_eq!(
            resolve_erlang_version(&releases, "27.3").expect("major minor"),
            "27.3.4.10"
        );
        assert_eq!(
            resolve_erlang_version(&releases, "27.3.4.8").expect("full"),
            "27.3.4.8"
        );
    }
}
