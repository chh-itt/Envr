//! Julia releases via official `versions.json` (julialang S3).

use envr_domain::runtime::{RemoteFilter, RuntimeKind, RuntimeVersion, version_line_key_for_kind};
use envr_error::{EnvrError, EnvrResult};
use serde_json::{Map, Value};
use std::cmp::Ordering;
use std::collections::HashSet;
use std::time::Duration;

pub const DEFAULT_JULIA_VERSIONS_JSON_URL: &str =
    "https://julialang-s3.julialang.org/bin/versions.json";

pub fn blocking_http_client() -> EnvrResult<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(120))
        .user_agent(concat!("envr-runtime-julia/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| EnvrError::Download(e.to_string()))
}

pub fn fetch_versions_json(client: &reqwest::blocking::Client, url: &str) -> EnvrResult<String> {
    let response = client
        .get(url)
        .send()
        .map_err(|e| EnvrError::Download(e.to_string()))?;
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

pub fn parse_versions_root(json: &str) -> EnvrResult<Map<String, Value>> {
    let v: Value = serde_json::from_str(json).map_err(|e| EnvrError::Validation(e.to_string()))?;
    match v {
        Value::Object(m) => Ok(m),
        _ => Err(EnvrError::Validation(
            "julia versions.json must be a JSON object".into(),
        )),
    }
}

/// Semver `x.y.z` keys only (exclude non-release entries).
pub fn is_stable_semver_key(key: &str) -> bool {
    let parts: Vec<&str> = key.split('.').collect();
    if parts.len() < 3 {
        return false;
    }
    parts
        .iter()
        .take(3)
        .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
}

#[derive(Debug, Clone, Copy)]
pub struct JuliaHostTarget {
    pub os: &'static str,
    pub arch: &'static str,
}

/// Map Rust host to `versions.json` `os` + `arch` fields.
pub fn julia_host_target() -> EnvrResult<JuliaHostTarget> {
    use std::env::consts::{ARCH, OS};
    match (OS, ARCH) {
        ("windows", "x86_64") => Ok(JuliaHostTarget {
            os: "winnt",
            arch: "x86_64",
        }),
        ("windows", "aarch64") => Ok(JuliaHostTarget {
            os: "winnt",
            arch: "aarch64",
        }),
        ("linux", "x86_64") => Ok(JuliaHostTarget {
            os: "linux",
            arch: "x86_64",
        }),
        ("linux", "aarch64") => Ok(JuliaHostTarget {
            os: "linux",
            arch: "aarch64",
        }),
        ("macos", "x86_64") => Ok(JuliaHostTarget { os: "mac", arch: "x86_64" }),
        ("macos", "aarch64") => Ok(JuliaHostTarget { os: "mac", arch: "aarch64" }),
        _ => Err(EnvrError::Validation(format!(
            "no Julia build mapping for host {OS}-{ARCH}; see docs/runtime/julia-integration-plan.md"
        ))),
    }
}

pub fn julia_cache_platform_tag(target: JuliaHostTarget) -> String {
    format!("{}_{}", target.os, target.arch)
}

fn file_entry_matches_host(f: &Value, host: JuliaHostTarget) -> bool {
    let Some(obj) = f.as_object() else {
        return false;
    };
    if obj.get("kind").and_then(|x| x.as_str()) != Some("archive") {
        return false;
    }
    obj.get("os").and_then(|x| x.as_str()) == Some(host.os)
        && obj.get("arch").and_then(|x| x.as_str()) == Some(host.arch)
}

/// Prefer `zip` on Windows, `tar.gz` on Linux/macOS.
fn artifact_score(extension: &str, host: JuliaHostTarget) -> i32 {
    if host.os == "winnt" {
        match extension {
            "zip" => 10,
            "tar.gz" => 5,
            _ => 0,
        }
    } else {
        match extension {
            "tar.gz" => 10,
            _ => 0,
        }
    }
}

/// Pick best installable file for this host from a version object `{ "files": [...], "stable": ... }`.
pub fn pick_file_for_host(version_entry: &Value, host: JuliaHostTarget) -> Option<&Value> {
    let files = version_entry.get("files")?.as_array()?;
    let mut best: Option<(&Value, i32)> = None;
    for f in files {
        if !file_entry_matches_host(f, host) {
            continue;
        }
        let ext = f.get("extension").and_then(|x| x.as_str()).unwrap_or("");
        let score = artifact_score(ext, host);
        if score <= 0 {
            continue;
        }
        let url = f.get("url").and_then(|x| x.as_str()).unwrap_or("");
        if url.is_empty() {
            continue;
        }
        match best {
            None => best = Some((f, score)),
            Some((_, s0)) if score > s0 => best = Some((f, score)),
            _ => {}
        }
    }
    best.map(|(f, _)| f)
}

pub fn version_has_installable_artifact(version_entry: &Value, host: JuliaHostTarget) -> bool {
    pick_file_for_host(version_entry, host).is_some()
}

fn version_is_stable_record(version_entry: &Value) -> bool {
    version_entry
        .get("stable")
        .and_then(|v| v.as_bool())
        .unwrap_or(true)
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

pub fn list_stable_versions_with_platform(
    root: &Map<String, Value>,
    host: JuliaHostTarget,
) -> Vec<String> {
    let mut keys: Vec<String> = root
        .iter()
        .filter(|(k, v)| {
            is_stable_semver_key(k)
                && version_is_stable_record(v)
                && version_has_installable_artifact(v, host)
        })
        .map(|(k, _)| k.clone())
        .collect();
    keys.sort_by(|a, b| cmp_semver_release_labels(b, a));
    keys
}

pub fn list_remote_versions(
    root: &Map<String, Value>,
    host: JuliaHostTarget,
    filter: &RemoteFilter,
) -> EnvrResult<Vec<RuntimeVersion>> {
    let mut keys = list_stable_versions_with_platform(root, host);
    if let Some(prefix) = filter.prefix.as_deref() {
        let p = prefix.trim();
        if !p.is_empty() {
            keys.retain(|k| k.starts_with(p));
        }
    }
    Ok(keys.into_iter().map(RuntimeVersion).collect())
}

pub fn list_remote_latest_per_major_lines(
    root: &Map<String, Value>,
    host: JuliaHostTarget,
) -> Vec<RuntimeVersion> {
    let keys = list_stable_versions_with_platform(root, host);
    let mut seen_lines = HashSet::<String>::new();
    let mut out = Vec::new();
    for k in keys {
        if let Some(line) = version_line_key_for_kind(RuntimeKind::Julia, &k) {
            if seen_lines.insert(line) {
                out.push(RuntimeVersion(k));
            }
        }
    }
    out
}

pub fn find_version_entry<'a>(
    root: &'a Map<String, Value>,
    version_label: &str,
) -> EnvrResult<&'a Value> {
    root.get(version_label).ok_or_else(|| {
        EnvrError::Validation(format!("julia version `{version_label}` not found in index"))
    })
}

pub fn resolve_julia_version(
    root: &Map<String, Value>,
    host: JuliaHostTarget,
    spec: &str,
) -> EnvrResult<String> {
    let s = spec.trim().trim_start_matches('v').trim_start_matches('V');
    if s.is_empty() {
        return Err(EnvrError::Validation("empty julia version spec".into()));
    }

    let candidates = list_stable_versions_with_platform(root, host);
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
                            "no julia release matches major `{s}` for this host"
                        ))
                    })
                    .map(|x| x.to_string());
            }
            2 => {
                let line = format!("{}.{}", parts[0], parts[1]);
                let best = candidates
                    .iter()
                    .filter(|k| {
                        version_line_key_for_kind(RuntimeKind::Julia, k).as_deref()
                            == Some(line.as_str())
                    })
                    .max_by(|a, b| cmp_semver_release_labels(a, b))
                    .map(|x| x.as_str());
                return best
                    .ok_or_else(|| {
                        EnvrError::Validation(format!(
                            "no julia release matches line `{line}` for this host"
                        ))
                    })
                    .map(|x| x.to_string());
            }
            3 | _ => {
                let best = candidates
                    .iter()
                    .filter(|k| {
                        numeric_version_segments(k).is_some_and(|p| {
                            p.len() >= 3 && p[0] == parts[0] && p[1] == parts[1] && p[2] == parts[2]
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

    let pfx = s.to_string();
    candidates
        .iter()
        .filter(|k| k.starts_with(&pfx))
        .max_by(|a, b| cmp_semver_release_labels(a, b))
        .cloned()
        .ok_or_else(|| {
            EnvrError::Validation(format!(
                "could not resolve julia version `{spec}` for this host"
            ))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!("../tests/fixtures/julia_versions_snippet.json");

    #[test]
    fn fixture_picks_linux_and_windows_artifacts() {
        let m = parse_versions_root(FIXTURE).expect("parse");
        let linux = JuliaHostTarget {
            os: "linux",
            arch: "x86_64",
        };
        let e = m.get("1.10.5").expect("ver");
        let f = pick_file_for_host(e, linux).expect("linux file");
        assert!(f
            .get("url")
            .and_then(|u| u.as_str())
            .unwrap()
            .contains("linux-x86_64.tar.gz"));

        let win = JuliaHostTarget {
            os: "winnt",
            arch: "x86_64",
        };
        let f2 = pick_file_for_host(e, win).expect("win file");
        assert!(f2
            .get("url")
            .and_then(|u| u.as_str())
            .unwrap()
            .ends_with(".zip"));
    }

    #[test]
    fn resolve_exact_and_minor_line() {
        let m = parse_versions_root(FIXTURE).expect("parse");
        let host = JuliaHostTarget {
            os: "linux",
            arch: "x86_64",
        };
        assert_eq!(
            resolve_julia_version(&m, host, "1.10.5").expect("ex"),
            "1.10.5"
        );
        assert_eq!(
            resolve_julia_version(&m, host, "1.10").expect("line"),
            "1.10.5"
        );
    }
}
