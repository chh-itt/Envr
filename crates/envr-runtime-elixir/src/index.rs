use envr_domain::runtime::{RemoteFilter, RuntimeVersion};
use envr_download::blocking::build_blocking_http_client;
use envr_error::{EnvrError, EnvrResult, ErrorCode};
use regex::Regex;
use std::cmp::Ordering;
use std::time::Duration;

pub const DEFAULT_BUILDS_INDEX_URL: &str = "https://builds.hex.pm/builds/elixir/builds.txt";
pub const DEFAULT_BUILDS_BASE_URL: &str = "https://builds.hex.pm/builds/elixir";
pub const DEFAULT_OTP_SERIES: &str = "27";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElixirBuild {
    pub version: String,
    pub otp: String,
    pub url: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct SemVerKey(u64, u64, u64);

fn semver_key(version: &str) -> EnvrResult<SemVerKey> {
    let mut parts = version.trim().trim_start_matches('v').split('.');
    let major = parts
        .next()
        .ok_or_else(|| EnvrError::Validation(format!("invalid elixir version: {version}")))?
        .parse::<u64>()
        .map_err(|_| EnvrError::Validation(format!("invalid elixir version: {version}")))?;
    let minor = parts
        .next()
        .unwrap_or("0")
        .parse::<u64>()
        .map_err(|_| EnvrError::Validation(format!("invalid elixir version: {version}")))?;
    let patch = parts
        .next()
        .unwrap_or("0")
        .parse::<u64>()
        .map_err(|_| EnvrError::Validation(format!("invalid elixir version: {version}")))?;
    Ok(SemVerKey(major, minor, patch))
}

fn is_full_semver(version: &str) -> bool {
    let trimmed = version.trim().trim_start_matches('v');
    let parts: Vec<&str> = trimmed.split('.').collect();
    parts.len() == 3
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
}

pub(crate) fn cmp_semver(a: &str, b: &str) -> Ordering {
    let ka = semver_key(a).unwrap_or(SemVerKey(0, 0, 0));
    let kb = semver_key(b).unwrap_or(SemVerKey(0, 0, 0));
    ka.cmp(&kb)
}

pub fn blocking_http_client() -> EnvrResult<reqwest::blocking::Client> {
    build_blocking_http_client(
        concat!("envr-runtime-elixir/", env!("CARGO_PKG_VERSION")),
        Some(Duration::from_secs(120)),
    )
}

pub fn fetch_builds_index(client: &reqwest::blocking::Client, url: &str) -> EnvrResult<String> {
    let response = client
        .get(url)
        .send()
        .map_err(|e| EnvrError::with_source(ErrorCode::Download, format!("request failed for {url}"), e))?;
    if !response.status().is_success() {
        return Err(EnvrError::Download(format!(
            "builds index request failed: {} {}",
            response.status(),
            url
        )));
    }
    response
        .text()
        .map_err(|e| EnvrError::with_source(ErrorCode::Download, format!("read body failed for {url}"), e))
}

pub fn parse_elixir_builds(index_text: &str, base_url: &str) -> EnvrResult<Vec<ElixirBuild>> {
    let re = Regex::new(r"^(v\d+\.\d+\.\d+)(?:-otp-(\d+))?\b")
        .map_err(|e| EnvrError::with_source(ErrorCode::Validation, "invalid elixir build line regex", e))?;
    let mut out = Vec::<ElixirBuild>::new();
    for line in index_text.lines() {
        let Some(cap) = re.captures(line.trim()) else {
            continue;
        };
        let full_tag = cap
            .get(0)
            .map(|m| m.as_str())
            .ok_or_else(|| EnvrError::Validation("missing elixir artifact tag".into()))?;
        let version = cap
            .get(1)
            .map(|m| m.as_str().trim_start_matches('v').to_string())
            .ok_or_else(|| EnvrError::Validation("missing elixir version".into()))?;
        let otp = cap
            .get(2)
            .map(|m| m.as_str().to_string())
            .unwrap_or_default();
        let url = format!("{base_url}/{full_tag}.zip");
        out.push(ElixirBuild { version, otp, url });
    }
    out.sort_by(|a, b| cmp_semver(&a.version, &b.version));
    out.dedup_by(|a, b| a.version == b.version && a.otp == b.otp);
    if out.is_empty() {
        return Err(EnvrError::Validation(
            "no elixir builds parsed from builds index".into(),
        ));
    }
    Ok(out)
}

pub fn filter_builds_for_otp(builds: &[ElixirBuild], otp: &str) -> Vec<ElixirBuild> {
    let mut out: Vec<ElixirBuild> = builds.iter().filter(|b| b.otp == otp).cloned().collect();
    out.sort_by(|a, b| cmp_semver(&a.version, &b.version));
    out
}

/// Prefer one OTP series, but degrade gracefully when that series is absent in upstream index.
pub fn select_builds_prefer_otp(builds: &[ElixirBuild], preferred_otp: &str) -> Vec<ElixirBuild> {
    let preferred = filter_builds_for_otp(builds, preferred_otp);
    if !preferred.is_empty() {
        return preferred;
    }

    // Fallback to the highest numeric OTP line available.
    let mut otp_series: Vec<u64> = builds
        .iter()
        .filter_map(|b| b.otp.parse::<u64>().ok())
        .collect();
    otp_series.sort_unstable();
    otp_series.dedup();
    if let Some(max_otp) = otp_series.last().copied() {
        let fallback = filter_builds_for_otp(builds, &max_otp.to_string());
        if !fallback.is_empty() {
            return fallback;
        }
    }

    // Last resort: keep all parseable build rows, sorted and deduplicated by version.
    let mut out = builds.to_vec();
    out.sort_by(|a, b| cmp_semver(&a.version, &b.version));
    out.dedup_by(|a, b| a.version == b.version);
    out
}

fn normalize_prefix(prefix: &str) -> String {
    prefix.trim().trim_start_matches('v').to_ascii_lowercase()
}

pub fn list_remote_versions(
    builds: &[ElixirBuild],
    filter: &RemoteFilter,
) -> EnvrResult<Vec<RuntimeVersion>> {
    let mut items: Vec<RuntimeVersion> = builds
        .iter()
        .map(|b| RuntimeVersion(b.version.clone()))
        .collect();
    items.sort_by(|a, b| cmp_semver(&b.0, &a.0));
    items.dedup_by(|a, b| a.0 == b.0);
    if let Some(prefix) = &filter.prefix {
        let p = normalize_prefix(prefix);
        if !p.is_empty() {
            items.retain(|v| v.0.to_ascii_lowercase().starts_with(&p));
        }
    }
    Ok(items)
}

pub fn list_latest_per_major(builds: &[ElixirBuild]) -> EnvrResult<Vec<RuntimeVersion>> {
    use std::collections::BTreeMap;
    let mut best: BTreeMap<u64, String> = BTreeMap::new();
    for b in builds {
        let key = semver_key(&b.version)?;
        best.entry(key.0)
            .and_modify(|cur| {
                if cmp_semver(&b.version, cur) == Ordering::Greater {
                    *cur = b.version.clone();
                }
            })
            .or_insert_with(|| b.version.clone());
    }
    let mut out: Vec<RuntimeVersion> = best.into_values().map(RuntimeVersion).collect();
    out.sort_by(|a, b| cmp_semver(&b.0, &a.0));
    Ok(out)
}

pub fn resolve_elixir_version(builds: &[ElixirBuild], spec: &str) -> EnvrResult<String> {
    let raw = spec.trim().trim_start_matches('v');
    if is_full_semver(raw) {
        if builds.iter().any(|b| b.version == raw) {
            return Ok(raw.to_string());
        }
        return Err(EnvrError::Validation(format!(
            "elixir version not found: {raw}"
        )));
    }
    let parts: Vec<&str> = raw.split('.').collect();
    if parts.is_empty() || parts.iter().any(|p| p.is_empty()) {
        return Err(EnvrError::Validation(format!(
            "unsupported elixir spec {spec:?}; use major (1), major.minor (1.19), or full (1.19.5)"
        )));
    }
    if !parts.iter().all(|p| p.chars().all(|c| c.is_ascii_digit())) {
        return Err(EnvrError::Validation(format!(
            "unsupported elixir spec {spec:?}; use major (1), major.minor (1.19), or full (1.19.5)"
        )));
    }
    let mut candidates: Vec<&ElixirBuild> = builds
        .iter()
        .filter(|b| {
            let ver_parts: Vec<&str> = b.version.split('.').collect();
            if parts.len() > ver_parts.len() {
                return false;
            }
            parts
                .iter()
                .zip(ver_parts.iter())
                .all(|(want, got)| want == got)
        })
        .collect();
    candidates.sort_by(|a, b| cmp_semver(&a.version, &b.version));
    candidates
        .last()
        .map(|b| b.version.clone())
        .ok_or_else(|| EnvrError::Validation(format!("no elixir version matches spec {spec:?}")))
}

pub fn pick_build_for_version(builds: &[ElixirBuild], version: &str) -> EnvrResult<ElixirBuild> {
    builds
        .iter()
        .find(|b| b.version == version)
        .cloned()
        .ok_or_else(|| EnvrError::Validation(format!("no elixir build for version {version}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_builds() -> Vec<ElixirBuild> {
        vec![
            ElixirBuild {
                version: "1.17.3".into(),
                otp: "27".into(),
                url: "https://example/v1.17.3-otp-27.zip".into(),
            },
            ElixirBuild {
                version: "1.18.2".into(),
                otp: "27".into(),
                url: "https://example/v1.18.2-otp-27.zip".into(),
            },
            ElixirBuild {
                version: "1.18.3".into(),
                otp: "27".into(),
                url: "https://example/v1.18.3-otp-27.zip".into(),
            },
            ElixirBuild {
                version: "1.19.0".into(),
                otp: "28".into(),
                url: "https://example/v1.19.0-otp-28.zip".into(),
            },
        ]
    }

    #[test]
    fn parse_elixir_builds_extracts_expected_rows() {
        let parsed = parse_elixir_builds(
            "v1.18.3-otp-27.zip\nv1.18.2-otp-27.zip\nv1.19.0-otp-28.zip\n",
            "https://builds.hex.pm/builds/elixir",
        )
        .expect("parse builds");

        assert_eq!(parsed.len(), 3);
        assert_eq!(parsed[0].version, "1.18.2");
        assert_eq!(parsed[1].version, "1.18.3");
        assert_eq!(parsed[2].version, "1.19.0");
    }

    #[test]
    fn list_latest_per_major_picks_highest_for_each_major() {
        let latest = list_latest_per_major(&sample_builds()).expect("list latest");
        assert_eq!(latest, vec![RuntimeVersion("1.19.0".into())]);
    }

    #[test]
    fn resolve_elixir_version_supports_major_and_minor_specs() {
        let builds = sample_builds();
        assert_eq!(
            resolve_elixir_version(&builds, "1").expect("resolve"),
            "1.19.0"
        );
        assert_eq!(
            resolve_elixir_version(&builds, "1.18").expect("resolve"),
            "1.18.3"
        );
        assert_eq!(
            resolve_elixir_version(&builds, "1.18.2").expect("resolve"),
            "1.18.2"
        );
    }

    #[test]
    fn select_builds_prefer_otp_falls_back_to_highest_available() {
        let builds = sample_builds();
        let selected = select_builds_prefer_otp(&builds, "26");
        assert!(selected.iter().all(|b| b.otp == "28"));
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].version, "1.19.0");
    }
}
