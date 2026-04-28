//! Parse `https://ziglang.org/download/index.json` and map host → Zig JSON platform keys.

use envr_domain::runtime::{RemoteFilter, RuntimeKind, RuntimeVersion, version_line_key_for_kind};
use envr_download::blocking::build_blocking_http_client;
use envr_error::{EnvrError, EnvrResult, ErrorCode};
use serde_json::{Map, Value};
use std::cmp::Ordering;
use std::collections::HashSet;
use std::time::Duration;

pub const DEFAULT_ZIG_INDEX_URL: &str = "https://ziglang.org/download/index.json";

pub fn blocking_http_client() -> EnvrResult<reqwest::blocking::Client> {
    build_blocking_http_client(
        concat!("envr-runtime-zig/", env!("CARGO_PKG_VERSION")),
        Some(Duration::from_secs(60)),
    )
}

pub fn fetch_index_json(client: &reqwest::blocking::Client, url: &str) -> EnvrResult<String> {
    let response = client.get(url).send().map_err(|e| {
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

pub fn parse_index_root(json: &str) -> EnvrResult<Map<String, Value>> {
    let v: Value = serde_json::from_str(json)
        .map_err(|e| EnvrError::with_source(ErrorCode::Validation, "invalid zig index json", e))?;
    match v {
        Value::Object(m) => Ok(m),
        _ => Err(EnvrError::Validation(
            "zig index.json must be a JSON object".into(),
        )),
    }
}

/// Zig `index.json` keys like `0.14.1` (exclude `master` and non-release keys).
pub fn is_stable_release_top_key(key: &str) -> bool {
    if key == "master" {
        return false;
    }
    let parts: Vec<&str> = key.split('.').collect();
    if parts.len() < 3 {
        return false;
    }
    parts
        .iter()
        .take(3)
        .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
}

/// Map Rust host OS/ARCH to `index.json` platform object keys (see Zig download docs).
pub fn zig_json_platform_key() -> EnvrResult<&'static str> {
    use std::env::consts::{ARCH, OS};
    match (OS, ARCH) {
        ("windows", "x86_64") => Ok("x86_64-windows"),
        ("windows", "aarch64") => Ok("aarch64-windows"),
        ("windows", "x86") | ("windows", "i686") => Ok("x86-windows"),
        ("linux", "x86_64") => Ok("x86_64-linux"),
        ("linux", "aarch64") => Ok("aarch64-linux"),
        ("linux", "arm") | ("linux", "armv7") => Ok("armv7a-linux"),
        ("linux", "riscv64") => Ok("riscv64-linux"),
        ("linux", "powerpc64") => Ok("powerpc64le-linux"),
        ("linux", "s390x") => Ok("s390x-linux"),
        ("linux", "loongarch64") => Ok("loongarch64-linux"),
        ("macos", "x86_64") => Ok("x86_64-macos"),
        ("macos", "aarch64") => Ok("aarch64-macos"),
        ("freebsd", "x86_64") => Ok("x86_64-freebsd"),
        ("freebsd", "aarch64") => Ok("aarch64-freebsd"),
        _ => Err(EnvrError::Validation(format!(
            "no official Zig build mapped for host {OS}-{ARCH}; see docs/runtime/zig-integration-plan.md"
        ))),
    }
}

#[derive(Debug, Clone)]
pub struct ZigPlatformArtifact {
    pub tarball_url: String,
    pub shasum_hex: String,
}

/// Read `tarball` + `shasum` for a concrete platform entry (not `src` / `bootstrap`).
pub fn artifact_for_platform(
    version_entry: &Value,
    platform_key: &str,
) -> Option<ZigPlatformArtifact> {
    let obj = version_entry.as_object()?;
    if matches!(
        platform_key,
        "src" | "bootstrap" | "docs" | "stdDocs" | "notes"
    ) {
        return None;
    }
    let plat = obj.get(platform_key)?;
    let obj = plat.as_object()?;
    let tarball = obj.get("tarball")?.as_str()?.to_string();
    let shasum = obj.get("shasum")?.as_str()?.to_string();
    if tarball.is_empty() {
        return None;
    }
    Some(ZigPlatformArtifact {
        tarball_url: tarball,
        shasum_hex: shasum,
    })
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

/// Sorted descending: newest stable first.
pub fn list_stable_versions_with_platform(
    index: &Map<String, Value>,
    platform_key: &str,
) -> Vec<String> {
    let mut keys: Vec<String> = index
        .iter()
        .filter(|(k, v)| {
            is_stable_release_top_key(k) && artifact_for_platform(v, platform_key).is_some()
        })
        .map(|(k, _)| k.clone())
        .collect();
    keys.sort_by(|a, b| cmp_semver_release_labels(b, a));
    keys
}

pub fn list_remote_versions(
    index: &Map<String, Value>,
    platform_key: &str,
    filter: &RemoteFilter,
) -> EnvrResult<Vec<RuntimeVersion>> {
    let mut keys = list_stable_versions_with_platform(index, platform_key);
    if let Some(prefix) = filter.prefix.as_deref() {
        let p = prefix.trim();
        if !p.is_empty() {
            keys.retain(|k| k.starts_with(p));
        }
    }
    Ok(keys.into_iter().map(RuntimeVersion).collect())
}

/// One representative full version per `version_line_key_for_kind(Zig, …)` line (latest patch),
/// iteration order same as descending full-version sort.
pub fn list_remote_latest_per_major_lines(
    index: &Map<String, Value>,
    platform_key: &str,
) -> Vec<RuntimeVersion> {
    let keys = list_stable_versions_with_platform(index, platform_key);
    let mut seen_lines = HashSet::<String>::new();
    let mut out = Vec::new();
    for k in keys {
        if let Some(line) = version_line_key_for_kind(RuntimeKind::Zig, &k) {
            if seen_lines.insert(line) {
                out.push(RuntimeVersion(k));
            }
        }
    }
    out
}

/// Resolve index entry for a **directory label** (`0.14.1` or the opaque `master` version string).
pub fn find_version_entry<'a>(
    index: &'a Map<String, Value>,
    version_label: &str,
) -> EnvrResult<&'a Value> {
    if let Some(e) = index.get(version_label) {
        return Ok(e);
    }
    if let Some(e) = index.get("master") {
        if let Some(v) = e.get("version").and_then(|x| x.as_str()) {
            if v == version_label {
                return Ok(e);
            }
        }
    }
    Err(EnvrError::Validation(format!(
        "zig version `{version_label}` not found in index"
    )))
}

pub fn resolve_zig_version(
    index: &Map<String, Value>,
    platform_key: &str,
    spec: &str,
) -> EnvrResult<String> {
    let s = spec.trim().trim_start_matches('v').trim_start_matches('V');
    if s.is_empty() {
        return Err(EnvrError::Validation("empty zig version spec".into()));
    }
    if s.eq_ignore_ascii_case("master") {
        let Some(entry) = index.get("master") else {
            return Err(EnvrError::Validation(
                "zig index has no `master` entry".into(),
            ));
        };
        let Some(ver) = entry.get("version").and_then(|v| v.as_str()) else {
            return Err(EnvrError::Validation(
                "zig `master` entry missing `version`".into(),
            ));
        };
        if artifact_for_platform(entry, platform_key).is_none() {
            return Err(EnvrError::Validation(format!(
                "no official Zig build for platform `{platform_key}` (requested master)"
            )));
        }
        return Ok(ver.to_string());
    }

    let candidates = list_stable_versions_with_platform(index, platform_key);
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
                            "no zig release matches major `{s}` for this host"
                        ))
                    })
                    .map(|x| x.to_string());
            }
            2 => {
                let line = format!("{}.{}", parts[0], parts[1]);
                let best = candidates
                    .iter()
                    .filter(|k| {
                        version_line_key_for_kind(RuntimeKind::Zig, k).as_deref()
                            == Some(line.as_str())
                    })
                    .max_by(|a, b| cmp_semver_release_labels(a, b))
                    .map(|x| x.as_str());
                return best
                    .ok_or_else(|| {
                        EnvrError::Validation(format!(
                            "no zig release matches line `{line}` for this host"
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
                "could not resolve zig version `{spec}` for this host (check spec and platform `{platform_key}`)"
            ))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!("../tests/fixtures/zig_index_snippet.json");

    #[test]
    fn contract_fixture_parses_and_maps_platform() {
        let m = parse_index_root(FIXTURE).expect("parse");
        assert!(m.contains_key("0.14.1"));
        assert!(m.contains_key("master"));
        let e = m.get("0.14.1").expect("0.14.1");
        let a = artifact_for_platform(e, "x86_64-windows").expect("win artifact");
        assert!(a.tarball_url.ends_with(".zip"));
        assert_eq!(a.shasum_hex.len(), 64);
    }

    #[test]
    fn resolve_exact_and_minor_line() {
        let m = parse_index_root(FIXTURE).expect("parse");
        assert_eq!(
            resolve_zig_version(&m, "x86_64-windows", "0.14.1").expect("exact"),
            "0.14.1"
        );
        assert_eq!(
            resolve_zig_version(&m, "x86_64-windows", "0.14").expect("line"),
            "0.14.1"
        );
    }

    #[test]
    fn list_remote_respects_prefix() {
        let m = parse_index_root(FIXTURE).expect("parse");
        let v = list_remote_versions(
            &m,
            "x86_64-windows",
            &RemoteFilter {
                prefix: Some("0.14.".into()),
                ..Default::default()
            },
        )
        .expect("list");
        assert!(v.iter().any(|x| x.0 == "0.14.1"));
    }

    #[test]
    fn resolve_master_and_find_entry() {
        let m = parse_index_root(FIXTURE).expect("parse");
        let v = resolve_zig_version(&m, "x86_64-windows", "master").expect("master");
        assert!(v.contains("dev"));
        let e = find_version_entry(&m, &v).expect("entry");
        assert!(artifact_for_platform(e, "x86_64-windows").is_some());
    }
}
