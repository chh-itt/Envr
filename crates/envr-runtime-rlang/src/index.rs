//! R (CRAN Windows) version list via rversions JSON + CRAN installer URL rules.

use envr_domain::runtime::{RemoteFilter, RuntimeKind, RuntimeVersion, version_line_key_for_kind};
use envr_error::{EnvrError, EnvrResult};
use serde_json::Value;
use std::cmp::Ordering;
use std::time::Duration;

pub const DEFAULT_RVERSIONS_JSON_URL: &str = "https://rversions.r-pkg.org/r-versions";
pub const DEFAULT_RVERSIONS_RELEASE_WIN_URL: &str = "https://rversions.r-pkg.org/r-release-win";

pub fn blocking_http_client() -> EnvrResult<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(120))
        .user_agent(concat!("envr-runtime-rlang/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| EnvrError::Download(e.to_string()))
}

pub fn fetch_text(client: &reqwest::blocking::Client, url: &str) -> EnvrResult<String> {
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

fn cmp_semver_release_labels(a: &str, b: &str) -> Ordering {
    use envr_domain::runtime::numeric_version_segments;
    match (numeric_version_segments(a), numeric_version_segments(b)) {
        (Some(va), Some(vb)) => va.cmp(&vb),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => a.cmp(b),
    }
}

fn is_stable_three_part(version: &str) -> bool {
    use envr_domain::runtime::numeric_version_segments;
    numeric_version_segments(version).is_some_and(|p| p.len() >= 3)
}

/// Parse `r-versions` JSON array; returns newest-first semver labels (e.g. `4.4.2`).
pub fn parse_r_versions_list(json: &str) -> EnvrResult<Vec<String>> {
    let v: Value = serde_json::from_str(json).map_err(|e| EnvrError::Validation(e.to_string()))?;
    let arr = v
        .as_array()
        .ok_or_else(|| EnvrError::Validation("r-versions JSON must be an array".into()))?;
    let mut out = Vec::new();
    for item in arr {
        let Some(ver) = item.get("version").and_then(|x| x.as_str()) else {
            continue;
        };
        if !is_stable_three_part(ver) {
            continue;
        }
        out.push(ver.to_string());
    }
    out.sort_by(|a, b| cmp_semver_release_labels(b, a));
    Ok(out)
}

/// Latest Windows release version string from `r-release-win` (first array element).
pub fn parse_latest_win_release_version(json: &str) -> EnvrResult<String> {
    let v: Value = serde_json::from_str(json).map_err(|e| EnvrError::Validation(e.to_string()))?;
    let arr = v
        .as_array()
        .ok_or_else(|| EnvrError::Validation("r-release-win JSON must be an array".into()))?;
    let first = arr
        .first()
        .ok_or_else(|| EnvrError::Validation("r-release-win JSON array is empty".into()))?;
    let ver = first
        .get("version")
        .and_then(|x| x.as_str())
        .ok_or_else(|| EnvrError::Validation("r-release-win missing version".into()))?;
    Ok(ver.to_string())
}

/// CRAN Windows installer URL for `version` given the current **latest** Windows patch (from r-release-win).
pub fn cran_windows_r_installer_url(version: &str, latest_win_version: &str) -> String {
    if version == latest_win_version {
        format!("https://cran.r-project.org/bin/windows/base/R-{version}-win.exe")
    } else {
        format!("https://cran.r-project.org/bin/windows/base/old/{version}/R-{version}-win.exe")
    }
}

pub fn list_remote_versions(versions: &[String], filter: &RemoteFilter) -> Vec<RuntimeVersion> {
    let mut keys: Vec<String> = versions.to_vec();
    if let Some(prefix) = filter.prefix.as_deref() {
        let p = prefix.trim();
        if !p.is_empty() {
            keys.retain(|k| k.starts_with(p));
        }
    }
    keys.into_iter().map(RuntimeVersion).collect()
}

pub fn list_remote_latest_per_major_lines(versions: &[String]) -> Vec<RuntimeVersion> {
    use std::collections::HashSet;
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for k in versions {
        if let Some(line) = version_line_key_for_kind(RuntimeKind::RLang, k) {
            if seen.insert(line) {
                out.push(RuntimeVersion(k.clone()));
            }
        }
    }
    out
}

pub fn resolve_r_version(versions: &[String], spec: &str) -> EnvrResult<String> {
    let s = spec.trim().trim_start_matches('v').trim_start_matches('V');
    if s.is_empty() {
        return Err(EnvrError::Validation("empty R version spec".into()));
    }
    if versions.iter().any(|k| k == s) {
        return Ok(s.to_string());
    }

    use envr_domain::runtime::numeric_version_segments;
    if let Some(parts) = numeric_version_segments(s) {
        match parts.len() {
            1 => {
                let major = parts[0];
                let best = versions
                    .iter()
                    .filter(|k| {
                        numeric_version_segments(k).is_some_and(|p| !p.is_empty() && p[0] == major)
                    })
                    .max_by(|a, b| cmp_semver_release_labels(a, b))
                    .map(|x| x.as_str());
                return best
                    .ok_or_else(|| {
                        EnvrError::Validation(format!(
                            "no R release matches major `{s}` for Windows"
                        ))
                    })
                    .map(|x| x.to_string());
            }
            2 => {
                let line = format!("{}.{}", parts[0], parts[1]);
                let best = versions
                    .iter()
                    .filter(|k| {
                        version_line_key_for_kind(RuntimeKind::RLang, k).as_deref()
                            == Some(line.as_str())
                    })
                    .max_by(|a, b| cmp_semver_release_labels(a, b))
                    .map(|x| x.as_str());
                return best
                    .ok_or_else(|| {
                        EnvrError::Validation(format!(
                            "no R release matches line `{line}` for Windows"
                        ))
                    })
                    .map(|x| x.to_string());
            }
            _ => {
                let best = versions
                    .iter()
                    .filter(|k| {
                        numeric_version_segments(k).is_some_and(|p| {
                            p.len() >= 3 && p[0] == parts[0] && p[1] == parts[1] && p[2] == parts[2]
                        })
                    })
                    .max_by(|a, b| cmp_semver_release_labels(a, b))
                    .map(|x| x.as_str());
                return best
                    .ok_or_else(|| {
                        EnvrError::Validation(format!(
                            "no R release matches exact `{s}` for Windows"
                        ))
                    })
                    .map(|x| x.to_string());
            }
        }
    }
    Err(EnvrError::Validation(format!(
        "could not resolve R version spec `{spec}`"
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_versions_and_cran_url_rule() {
        let json = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/r_versions_snippet.json"),
        )
        .expect("read fixture");
        let vs = parse_r_versions_list(&json).expect("parse");
        assert!(vs.iter().any(|v| v == "4.4.2"));
        let latest = "4.4.2";
        assert_eq!(
            cran_windows_r_installer_url("4.4.2", latest),
            "https://cran.r-project.org/bin/windows/base/R-4.4.2-win.exe"
        );
        assert_eq!(
            cran_windows_r_installer_url("4.3.3", latest),
            "https://cran.r-project.org/bin/windows/base/old/4.3.3/R-4.3.3-win.exe"
        );
        let resolved = resolve_r_version(&vs, "4.4").expect("resolve line");
        assert_eq!(resolved, "4.4.2");
    }
}
