use envr_domain::runtime::{RemoteFilter, RuntimeVersion};
use envr_download::blocking::build_blocking_http_client;
use envr_error::{EnvrError, EnvrResult, ErrorCode};
use serde::Deserialize;
use std::collections::HashMap;
use std::time::Duration;

#[derive(Debug, Clone, Deserialize)]
pub struct PackageFile {
    pub path: String,
    #[serde(default)]
    pub sha256: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BuildEntry {
    #[serde(default)]
    pub zip: Option<PackageFile>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ReleaseLine {
    pub version: String,

    // Dynamic keys like `nts-vs17-x64`, `ts-vs17-x64`, etc.
    #[serde(flatten)]
    pub builds: HashMap<String, serde_json::Value>,
}

pub type PhpReleasesIndex = HashMap<String, ReleaseLine>;

pub fn blocking_http_client() -> EnvrResult<reqwest::blocking::Client> {
    build_blocking_http_client(
        concat!("envr-runtime-php/", env!("CARGO_PKG_VERSION")),
        Some(Duration::from_secs(45)),
    )
}

pub fn fetch_php_windows_releases_json(
    client: &reqwest::blocking::Client,
    url: &str,
) -> EnvrResult<String> {
    let response = client
        .get(url)
        .send()
        .map_err(|e| EnvrError::with_source(ErrorCode::Download, format!("request failed for {url}"), e))?;
    if !response.status().is_success() {
        return Err(EnvrError::Download(format!(
            "GET {} -> {}",
            url,
            response.status()
        )));
    }
    response
        .text()
        .map_err(|e| EnvrError::with_source(ErrorCode::Download, format!("read body failed for {url}"), e))
}

pub fn parse_php_windows_index(json: &str) -> EnvrResult<PhpReleasesIndex> {
    serde_json::from_str(json)
        .map_err(|e| EnvrError::with_source(ErrorCode::Validation, "invalid php windows index json", e))
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

fn is_stable_version(s: &str) -> bool {
    let t = s.trim().trim_start_matches('v');
    !t.contains('-')
}

pub fn list_remote_versions(
    idx: &PhpReleasesIndex,
    filter: &RemoteFilter,
) -> EnvrResult<Vec<RuntimeVersion>> {
    let mut items: Vec<(SemKey, String)> = idx
        .values()
        .filter_map(|line| semver_key(&line.version).map(|k| (k, line.version.clone())))
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

pub fn resolve_php_version(idx: &PhpReleasesIndex, spec: &str) -> EnvrResult<String> {
    let s = spec.trim().trim_start_matches('v').to_ascii_lowercase();
    if s.is_empty() {
        return Err(EnvrError::Validation("empty php version spec".into()));
    }
    let mut items: Vec<(SemKey, String)> = idx
        .values()
        .filter_map(|line| semver_key(&line.version).map(|k| (k, line.version.clone())))
        .collect();
    if items.is_empty() {
        return Err(EnvrError::Validation("no php versions in index".into()));
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
        "no PHP release matches spec {spec:?}"
    )))
}

/// Latest stable patch per `major.minor` line, **only for rows that have an installable zip**
/// for the requested Windows build (NTS vs TS) and CPU arch.
///
/// NTS and TS lists differ when a minor line only ships one flavor in the index.
pub fn list_latest_stable_per_minor_line_for_build(
    idx: &PhpReleasesIndex,
    want_ts: bool,
    arch: &str,
) -> EnvrResult<Vec<RuntimeVersion>> {
    let mut best: std::collections::BTreeMap<(u64, u64), (SemKey, String)> =
        std::collections::BTreeMap::new();
    for line in idx.values() {
        if !is_stable_version(&line.version) {
            continue;
        }
        if pick_windows_zip(line, Some(want_ts), arch).is_err() {
            continue;
        }
        let Some(k) = semver_key(&line.version) else {
            continue;
        };
        let key = (k.0, k.1);
        match best.get(&key) {
            Some((old, _)) if *old >= k => {}
            _ => {
                best.insert(key, (k, line.version.clone()));
            }
        }
    }
    let mut out: Vec<(SemKey, String)> = best.into_values().collect();
    out.sort_by(|a, b| b.0.cmp(&a.0));
    Ok(out.into_iter().map(|(_, v)| RuntimeVersion(v)).collect())
}

pub fn pick_windows_zip(
    line: &ReleaseLine,
    want_ts: Option<bool>,
    arch: &str,
) -> EnvrResult<(String, String)> {
    // Variant keys: `nts-vs17-x64`, `ts-vs17-x64`, etc.
    //
    // IMPORTANT: do not use `str::contains("ts")` — the substring `ts` appears inside `nts`,
    // so TS selection would incorrectly match NTS keys first.
    let arch_key = match arch {
        "x86_64" => "x64",
        "x86" => "x86",
        "aarch64" => "arm64",
        other => other,
    };
    let mut keys: Vec<&str> = line.builds.keys().map(|s| s.as_str()).collect();
    keys.sort();

    fn key_matches_flavor(k: &str, want_ts: bool) -> bool {
        let kl = k.to_ascii_lowercase();
        if want_ts {
            kl.starts_with("ts-")
        } else {
            kl.starts_with("nts-")
        }
    }

    let flavor_order: Vec<bool> = match want_ts {
        Some(true) => vec![true],
        Some(false) => vec![false],
        // Ambiguous: prefer NTS (typical CLI SAPI), then TS.
        None => vec![false, true],
    };

    for wt in flavor_order {
        if let Some(k) = keys.iter().find(|k| {
            let kl = k.to_ascii_lowercase();
            key_matches_flavor(k, wt) && kl.contains("vs") && kl.ends_with(arch_key)
        }) {
            let v = line
                .builds
                .get(*k)
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            let entry: BuildEntry =
                serde_json::from_value(v).map_err(|e| {
                    EnvrError::with_source(ErrorCode::Validation, "invalid php build entry json", e)
                })?;
            if let Some(z) = entry.zip
                && !z.path.is_empty()
            {
                return Ok((z.path, z.sha256));
            }
        }
    }
    Err(EnvrError::Validation(format!(
        "no suitable windows zip for php {} on arch {arch}",
        line.version
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn line(builds: HashMap<String, serde_json::Value>) -> ReleaseLine {
        ReleaseLine {
            version: "8.4.0".into(),
            builds,
        }
    }

    #[test]
    fn pick_windows_zip_ts_does_not_match_nts_key() {
        let mut builds = HashMap::new();
        builds.insert(
            "nts-vs17-x64".into(),
            serde_json::json!({ "zip": { "path": "php-nts.zip", "sha256": "" } }),
        );
        builds.insert(
            "ts-vs17-x64".into(),
            serde_json::json!({ "zip": { "path": "php-ts.zip", "sha256": "" } }),
        );
        let l = line(builds);
        let (path, _) = pick_windows_zip(&l, Some(true), "x86_64").expect("ts");
        assert!(path.contains("php-ts"), "got {path}");
        let (path_nts, _) = pick_windows_zip(&l, Some(false), "x86_64").expect("nts");
        assert!(path_nts.contains("php-nts"), "got {path_nts}");
    }

    #[test]
    fn list_latest_per_minor_line_respects_ts_nts_availability() {
        let mut idx: PhpReleasesIndex = HashMap::new();

        let mut only_nts = HashMap::new();
        only_nts.insert(
            "nts-vs17-x64".into(),
            serde_json::json!({ "zip": { "path": "a.zip", "sha256": "" } }),
        );
        idx.insert(
            "8.3.0".into(),
            ReleaseLine {
                version: "8.3.0".into(),
                builds: only_nts,
            },
        );

        let mut both = HashMap::new();
        both.insert(
            "nts-vs17-x64".into(),
            serde_json::json!({ "zip": { "path": "b-nts.zip", "sha256": "" } }),
        );
        both.insert(
            "ts-vs17-x64".into(),
            serde_json::json!({ "zip": { "path": "b-ts.zip", "sha256": "" } }),
        );
        idx.insert(
            "8.4.0".into(),
            ReleaseLine {
                version: "8.4.0".into(),
                builds: both,
            },
        );

        let nts_list =
            list_latest_stable_per_minor_line_for_build(&idx, false, "x86_64").expect("nts list");
        assert!(nts_list.iter().any(|v| v.0 == "8.3.0"));
        assert!(nts_list.iter().any(|v| v.0 == "8.4.0"));

        let ts_list =
            list_latest_stable_per_minor_line_for_build(&idx, true, "x86_64").expect("ts");
        assert!(!ts_list.iter().any(|v| v.0 == "8.3.0"));
        assert!(ts_list.iter().any(|v| v.0 == "8.4.0"));
    }
}
