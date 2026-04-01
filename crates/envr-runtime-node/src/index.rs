//! Node.js `index.json` parsing, platform filtering, and version resolution.
//!
//! Upstream format: <https://nodejs.org/dist/index.json>

use envr_domain::runtime::{RemoteFilter, RuntimeVersion};
use envr_error::{EnvrError, EnvrResult};
use serde::Deserialize;
use std::time::Duration;

/// Official Node distribution index (JSON array of releases).
pub const DEFAULT_NODE_INDEX_JSON_URL: &str = "https://nodejs.org/dist/index.json";

#[derive(Debug, Clone)]
pub struct NodeRelease {
    pub version: String,
    pub files: Vec<String>,
    pub lts_codename: Option<String>,
}

#[derive(Debug, Deserialize)]
struct NodeReleaseJson {
    version: String,
    files: Vec<String>,
    lts: serde_json::Value,
}

impl TryFrom<NodeReleaseJson> for NodeRelease {
    type Error = EnvrError;

    fn try_from(j: NodeReleaseJson) -> Result<Self, Self::Error> {
        let lts_codename = match &j.lts {
            serde_json::Value::String(s) if !s.is_empty() => Some(s.clone()),
            serde_json::Value::Bool(true) => Some("true".to_string()),
            _ => None,
        };
        Ok(Self {
            version: j.version,
            files: j.files,
            lts_codename,
        })
    }
}

/// Parse the JSON body returned by `index.json`.
pub fn parse_node_index(json: &str) -> EnvrResult<Vec<NodeRelease>> {
    let raw: Vec<NodeReleaseJson> =
        serde_json::from_str(json).map_err(|e| EnvrError::Validation(e.to_string()))?;
    raw.into_iter()
        .map(NodeRelease::try_from)
        .collect::<EnvrResult<Vec<_>>>()
}

pub fn fetch_node_index(client: &reqwest::blocking::Client, url: &str) -> EnvrResult<String> {
    let response = client
        .get(url)
        .send()
        .map_err(|e| EnvrError::Download(e.to_string()))?;
    if !response.status().is_success() {
        return Err(EnvrError::Download(format!(
            "index request failed: {} {}",
            response.status(),
            url
        )));
    }
    response
        .text()
        .map_err(|e| EnvrError::Download(e.to_string()))
}

pub fn blocking_http_client() -> EnvrResult<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(45))
        .user_agent(concat!("envr-runtime-node/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| EnvrError::Download(e.to_string()))
}

/// Returns `true` if this release ships binaries for `(os, arch)` as exposed in `index.json`
/// `files` entries (`win-*`, `linux-*`, `osx-*-tar` / `osx-*-pkg`, …).
pub fn release_has_platform(files: &[String], os: &str, arch: &str) -> bool {
    let files: Vec<&str> = files.iter().map(String::as_str).collect();
    match (os, arch) {
        ("windows", "x86_64") => files.iter().any(|f| f.starts_with("win-x64")),
        ("windows", "aarch64") => files.iter().any(|f| f.starts_with("win-arm64")),
        ("windows", "x86") => files.iter().any(|f| f.starts_with("win-x86")),
        ("linux", "x86_64") => files.contains(&"linux-x64"),
        ("linux", "aarch64") => files.contains(&"linux-arm64"),
        ("linux", "arm") | ("linux", "armv7") => files.contains(&"linux-armv7l"),
        ("macos", "x86_64") => files
            .iter()
            .any(|f| *f == "osx-x64-tar" || *f == "osx-x64-pkg"),
        ("macos", "aarch64") => files
            .iter()
            .any(|f| *f == "osx-arm64-tar" || *f == "osx-arm64-pkg"),
        _ => false,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct SemVerKey(u64, u64, u64);

fn semver_key(version: &str) -> EnvrResult<SemVerKey> {
    let s = version.strip_prefix('v').unwrap_or(version);
    let base = s
        .split_once('-')
        .map(|(a, _)| a)
        .unwrap_or(s)
        .split('+')
        .next()
        .unwrap_or(s);
    let mut parts = base.split('.');
    let major: u64 = parts
        .next()
        .ok_or_else(|| EnvrError::Validation(format!("invalid node semver: {version}")))?
        .parse()
        .map_err(|_| EnvrError::Validation(format!("invalid node semver: {version}")))?;
    let minor: u64 = parts
        .next()
        .unwrap_or("0")
        .parse()
        .map_err(|_| EnvrError::Validation(format!("invalid node semver: {version}")))?;
    let patch: u64 = parts
        .next()
        .unwrap_or("0")
        .parse()
        .map_err(|_| EnvrError::Validation(format!("invalid node semver: {version}")))?;
    Ok(SemVerKey(major, minor, patch))
}

/// Strips a leading `v` for stable display / storage in [`RuntimeVersion`].
pub fn normalize_node_version(version: &str) -> String {
    version.strip_prefix('v').unwrap_or(version).to_string()
}

fn normalize_prefix(prefix: &str) -> String {
    prefix
        .trim()
        .strip_prefix('v')
        .unwrap_or(prefix.trim())
        .to_ascii_lowercase()
}

pub fn list_remote_versions(
    releases: &[NodeRelease],
    os: &str,
    arch: &str,
    filter: &RemoteFilter,
) -> EnvrResult<Vec<RuntimeVersion>> {
    let mut items: Vec<(SemVerKey, String)> = releases
        .iter()
        .filter(|r| release_has_platform(&r.files, os, arch))
        .map(|r| {
            let key = semver_key(&r.version)?;
            let v = normalize_node_version(&r.version);
            Ok((key, v))
        })
        .collect::<EnvrResult<Vec<_>>>()?;

    items.sort_by(|a, b| b.0.cmp(&a.0));

    let mut out: Vec<RuntimeVersion> = items.into_iter().map(|(_, v)| RuntimeVersion(v)).collect();

    if let Some(prefix) = &filter.prefix {
        let p = normalize_prefix(prefix);
        if !p.is_empty() {
            out.retain(|rv| rv.0.to_ascii_lowercase().starts_with(&p));
        }
    }

    Ok(out)
}

pub fn resolve_node_version(
    releases: &[NodeRelease],
    os: &str,
    arch: &str,
    spec: &str,
) -> EnvrResult<String> {
    let spec_trim = spec.trim();
    if spec_trim.is_empty() {
        return Err(EnvrError::Validation("empty node version spec".into()));
    }

    let candidates: Vec<&NodeRelease> = releases
        .iter()
        .filter(|r| release_has_platform(&r.files, os, arch))
        .collect();

    if candidates.is_empty() {
        return Err(EnvrError::Validation(format!(
            "no node releases for platform {os}-{arch}"
        )));
    }

    let lower = spec_trim.to_ascii_lowercase();
    match lower.as_str() {
        "latest" | "current" => pick_highest_semver(&candidates),
        "lts" => pick_highest_lts(&candidates, None),
        s if s.starts_with("lts-") || s.starts_with("lts/") => {
            let raw = s
                .strip_prefix("lts-")
                .or_else(|| s.strip_prefix("lts/"))
                .unwrap_or("")
                .trim();
            if raw.is_empty() {
                pick_highest_lts(&candidates, None)
            } else {
                pick_highest_lts(&candidates, Some(raw))
            }
        }
        _ => pick_from_spec(&candidates, spec_trim),
    }
}

fn pick_highest_semver(candidates: &[&NodeRelease]) -> EnvrResult<String> {
    let mut best: Option<(&NodeRelease, SemVerKey)> = None;
    for r in candidates {
        let key = semver_key(&r.version)?;
        best = match best {
            None => Some((*r, key)),
            Some((_, bk)) if key > bk => Some((*r, key)),
            Some(prev) => Some(prev),
        };
    }
    let Some((r, _)) = best else {
        return Err(EnvrError::Validation("no matching node releases".into()));
    };
    Ok(normalize_node_version(&r.version))
}

fn pick_highest_lts(candidates: &[&NodeRelease], codename: Option<&str>) -> EnvrResult<String> {
    let wanted = codename.map(|c| c.to_ascii_lowercase());
    let pool: Vec<&NodeRelease> = candidates
        .iter()
        .copied()
        .filter(|r| r.lts_codename.is_some())
        .filter(|r| {
            wanted.as_ref().is_none_or(|w| {
                r.lts_codename
                    .as_ref()
                    .is_some_and(|cn| cn.to_ascii_lowercase() == *w)
            })
        })
        .collect();

    if pool.is_empty() {
        return Err(EnvrError::Validation(match wanted {
            Some(w) => format!("no LTS node releases for codename {w:?} on this platform"),
            None => "no LTS node releases on this platform".into(),
        }));
    }

    pick_highest_semver(&pool)
}

fn pick_from_spec(candidates: &[&NodeRelease], spec: &str) -> EnvrResult<String> {
    let norm = normalize_node_version(spec);

    if let Some(r) = candidates
        .iter()
        .find(|r| normalize_node_version(&r.version) == norm)
    {
        return Ok(normalize_node_version(&r.version));
    }

    // Bare major: "22" / "v22"
    if let Some(major) = parse_major_only(spec) {
        return pick_highest_major(candidates, major);
    }

    // Major.minor line: "22.10" / "v22.10"
    if let Some((major, minor)) = parse_major_minor(spec) {
        return pick_highest_minor_line(candidates, major, minor);
    }

    Err(EnvrError::Validation(format!(
        "no node release matches spec {spec:?} for this platform"
    )))
}

fn parse_major_only(spec: &str) -> Option<u64> {
    let s = spec.trim();
    let s = s.strip_prefix('v').unwrap_or(s);
    if s.contains('.') {
        return None;
    }
    s.parse().ok()
}

fn parse_major_minor(spec: &str) -> Option<(u64, u64)> {
    let s = spec.trim();
    let s = s.strip_prefix('v').unwrap_or(s);
    let mut it = s.split('.');
    let a: u64 = it.next()?.parse().ok()?;
    let b: u64 = it.next()?.parse().ok()?;
    if it.next().is_some() {
        return None;
    }
    Some((a, b))
}

fn pick_highest_major(candidates: &[&NodeRelease], major: u64) -> EnvrResult<String> {
    let mut best: Option<(&NodeRelease, SemVerKey)> = None;
    for r in candidates {
        let k = semver_key(&r.version)?;
        if k.0 != major {
            continue;
        }
        best = match best {
            None => Some((*r, k)),
            Some((_br, bk)) if k > bk => Some((*r, k)),
            Some(x) => Some(x),
        };
    }
    let Some((r, _)) = best else {
        return Err(EnvrError::Validation(format!(
            "no node release for major {major} on this platform"
        )));
    };
    Ok(normalize_node_version(&r.version))
}

fn pick_highest_minor_line(
    candidates: &[&NodeRelease],
    major: u64,
    minor: u64,
) -> EnvrResult<String> {
    let mut best: Option<(&NodeRelease, SemVerKey)> = None;
    for r in candidates {
        let k = semver_key(&r.version)?;
        if k.0 != major || k.1 != minor {
            continue;
        }
        best = match best {
            None => Some((*r, k)),
            Some((_br, bk)) if k > bk => Some((*r, k)),
            Some(x) => Some(x),
        };
    }
    let Some((r, _)) = best else {
        return Err(EnvrError::Validation(format!(
            "no node release for {major}.{minor}.x on this platform"
        )));
    };
    Ok(normalize_node_version(&r.version))
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"[
        {"version":"v30.0.0","date":"2026-01-01","files":["linux-x64","osx-x64-tar","win-x64-zip"],"lts":false},
        {"version":"v24.2.0","date":"2026-01-01","files":["linux-x64","win-x64-zip"],"lts":"Krypton"},
        {"version":"v24.1.0","date":"2026-01-01","files":["linux-x64","win-x64-zip"],"lts":"Krypton"},
        {"version":"v22.10.0","date":"2026-01-01","files":["linux-x64"],"lts":"Jod"},
        {"version":"v22.9.0","date":"2026-01-01","files":["linux-x64"],"lts":"Jod"},
        {"version":"v22.8.0","date":"2026-01-01","files":["linux-x64"],"lts":false}
    ]"#;

    fn parsed() -> Vec<NodeRelease> {
        parse_node_index(FIXTURE).expect("parse")
    }

    #[test]
    fn parse_index_smoke() {
        let rel = parsed();
        assert_eq!(rel.len(), 6);
        assert_eq!(rel[2].lts_codename.as_deref(), Some("Krypton"));
        assert!(rel[5].lts_codename.is_none());
    }

    #[test]
    fn platform_linux_vs_macos() {
        let rel = parsed();
        assert!(release_has_platform(&rel[0].files, "linux", "x86_64"));
        assert!(release_has_platform(&rel[0].files, "macos", "x86_64"));
        assert!(!release_has_platform(&rel[3].files, "macos", "x86_64"));
    }

    #[test]
    fn list_remote_orders_newest_first_and_prefix() {
        let rel = parsed();
        let all = list_remote_versions(&rel, "linux", "x86_64", &RemoteFilter { prefix: None })
            .expect("list");
        assert_eq!(all[0].0, "30.0.0");
        assert_eq!(all.last().unwrap().0, "22.8.0");

        let p22 = list_remote_versions(
            &rel,
            "linux",
            "x86_64",
            &RemoteFilter {
                prefix: Some("22".into()),
            },
        )
        .expect("list");
        assert_eq!(p22.len(), 3);
        assert!(p22.iter().all(|v| v.0.starts_with("22.")));
    }

    #[test]
    fn resolve_latest_and_lts() {
        let rel = parsed();
        assert_eq!(
            resolve_node_version(&rel, "linux", "x86_64", "latest").expect("r"),
            "30.0.0"
        );
        assert_eq!(
            resolve_node_version(&rel, "linux", "x86_64", "lts").expect("r"),
            "24.2.0"
        );
    }

    #[test]
    fn resolve_lts_codename() {
        let rel = parsed();
        assert_eq!(
            resolve_node_version(&rel, "linux", "x86_64", "lts-jod").expect("r"),
            "22.10.0"
        );
        assert_eq!(
            resolve_node_version(&rel, "linux", "x86_64", "lts/Jod").expect("r"),
            "22.10.0"
        );
    }

    #[test]
    fn resolve_major_and_minor_line() {
        let rel = parsed();
        assert_eq!(
            resolve_node_version(&rel, "linux", "x86_64", "22").expect("r"),
            "22.10.0"
        );
        assert_eq!(
            resolve_node_version(&rel, "linux", "x86_64", "22.9").expect("r"),
            "22.9.0"
        );
    }

    #[test]
    fn resolve_exact() {
        let rel = parsed();
        assert_eq!(
            resolve_node_version(&rel, "linux", "x86_64", "v22.9.0").expect("r"),
            "22.9.0"
        );
    }
}
