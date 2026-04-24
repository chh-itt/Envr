//! Nim stable matrix from nim-lang.org/install.html (links → nim-lang/nightlies).

use envr_domain::runtime::{RemoteFilter, RuntimeKind, RuntimeVersion, version_line_key_for_kind};
use envr_download::blocking::build_blocking_http_client;
use envr_error::{EnvrError, EnvrResult, ErrorCode};
use regex::Regex;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;
use std::time::Duration;

pub const DEFAULT_NIM_INSTALL_HTML_URL: &str = "https://nim-lang.org/install.html";

static NIM_URL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"https://github\.com/nim-lang/nightlies/releases/download/[A-Za-z0-9._-]+/nim-(\d+\.\d+\.\d+)-(windows_x64|windows_x32|linux_x64|linux_x32|linux_arm64|linux_armv7l|macosx_x64|macosx_arm64)\.(zip|tar\.xz)",
    )
    .expect("nim url regex")
});

pub fn blocking_http_client() -> EnvrResult<reqwest::blocking::Client> {
    build_blocking_http_client(
        concat!("envr-runtime-nim/", env!("CARGO_PKG_VERSION")),
        Some(Duration::from_secs(120)),
    )
}

pub fn fetch_install_html(client: &reqwest::blocking::Client, url: &str) -> EnvrResult<String> {
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

/// Maps Rust host to the Nim download table column id.
pub fn nim_host_platform_slot() -> EnvrResult<&'static str> {
    use std::env::consts::{ARCH, OS};
    match (OS, ARCH) {
        ("windows", "x86_64") => Ok("windows_x64"),
        ("windows", "x86") => Ok("windows_x32"),
        ("linux", "x86_64") => Ok("linux_x64"),
        ("linux", "x86") => Ok("linux_x32"),
        ("linux", "aarch64") => Ok("linux_arm64"),
        ("linux", "arm") => Ok("linux_armv7l"),
        ("macos", "x86_64") => Ok("macosx_x64"),
        ("macos", "aarch64") => Ok("macosx_arm64"),
        _ => Err(EnvrError::Validation(format!(
            "no Nim prebuild mapping for host {OS}-{ARCH}; see docs/runtime/nim-integration-plan.md"
        ))),
    }
}

pub fn nim_cache_platform_tag(slot: &str) -> String {
    slot.replace('.', "_")
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

/// `version` → platform slot → absolute download URL (from parsed HTML).
pub type NimUrlIndex = HashMap<String, HashMap<String, String>>;

pub fn parse_install_html(html: &str) -> NimUrlIndex {
    let mut out: NimUrlIndex = HashMap::new();
    for cap in NIM_URL_RE.captures_iter(html) {
        let ver = cap
            .get(1)
            .map(|m| m.as_str().to_string())
            .unwrap_or_default();
        let slot = cap
            .get(2)
            .map(|m| m.as_str().to_string())
            .unwrap_or_default();
        let url = cap
            .get(0)
            .map(|m| m.as_str().to_string())
            .unwrap_or_default();
        if ver.is_empty() || slot.is_empty() || url.is_empty() {
            continue;
        }
        out.entry(ver).or_default().insert(slot, url);
    }
    out
}

pub fn versions_for_slot(index: &NimUrlIndex, slot: &str) -> Vec<String> {
    let mut keys: Vec<String> = index
        .iter()
        .filter(|(_, m)| m.contains_key(slot))
        .map(|(k, _)| k.clone())
        .collect();
    keys.sort_by(|a, b| cmp_semver_release_labels(b, a));
    keys
}

pub fn list_remote_versions(
    index: &NimUrlIndex,
    slot: &str,
    filter: &RemoteFilter,
) -> EnvrResult<Vec<RuntimeVersion>> {
    let mut keys = versions_for_slot(index, slot);
    if let Some(prefix) = filter.prefix.as_deref() {
        let p = prefix.trim();
        if !p.is_empty() {
            keys.retain(|k| k.starts_with(p));
        }
    }
    Ok(keys.into_iter().map(RuntimeVersion).collect())
}

pub fn list_remote_latest_per_major_lines(index: &NimUrlIndex, slot: &str) -> Vec<RuntimeVersion> {
    let keys = versions_for_slot(index, slot);
    let mut seen: HashSet<String> = HashSet::new();
    let mut out = Vec::new();
    for k in keys {
        if let Some(line) = version_line_key_for_kind(RuntimeKind::Nim, &k) {
            if seen.insert(line) {
                out.push(RuntimeVersion(k));
            }
        }
    }
    out
}

pub fn find_urls_for_version<'a>(
    index: &'a NimUrlIndex,
    version_label: &str,
) -> Option<&'a HashMap<String, String>> {
    index.get(version_label)
}

pub fn resolve_nim_version(index: &NimUrlIndex, slot: &str, spec: &str) -> EnvrResult<String> {
    let s = spec.trim().trim_start_matches('v').trim_start_matches('V');
    if s.is_empty() {
        return Err(EnvrError::Validation("empty nim version spec".into()));
    }
    let candidates = versions_for_slot(index, slot);
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
                            "no nim release matches major `{s}` for this host"
                        ))
                    })
                    .map(|x| x.to_string());
            }
            2 => {
                let line = format!("{}.{}", parts[0], parts[1]);
                let best = candidates
                    .iter()
                    .filter(|k| {
                        version_line_key_for_kind(RuntimeKind::Nim, k).as_deref()
                            == Some(line.as_str())
                    })
                    .max_by(|a, b| cmp_semver_release_labels(a, b))
                    .map(|x| x.as_str());
                return best
                    .ok_or_else(|| {
                        EnvrError::Validation(format!(
                            "no nim release matches line `{line}` for this host"
                        ))
                    })
                    .map(|x| x.to_string());
            }
            _ => {
                let best = candidates
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
                            "no nim release matches exact `{s}` for this host"
                        ))
                    })
                    .map(|x| x.to_string());
            }
        }
    }
    Err(EnvrError::Validation(format!(
        "could not resolve nim version spec `{spec}`"
    )))
}

pub fn pick_download_url(
    index: &NimUrlIndex,
    version_label: &str,
    slot: &str,
) -> EnvrResult<String> {
    let urls = find_urls_for_version(index, version_label).ok_or_else(|| {
        EnvrError::Validation(format!("nim version `{version_label}` not found in index"))
    })?;
    urls.get(slot).cloned().ok_or_else(|| {
        EnvrError::Validation(format!(
            "no nim prebuild for `{version_label}` on platform slot `{slot}`"
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn fixture_parses_urls_and_resolves_line() {
        let html = fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/nim_install_snippet.html"
        ))
        .expect("read fixture");
        let idx = parse_install_html(&html);
        assert!(
            idx.contains_key("2.0.0"),
            "expected 2.0.0 in index, keys={:?}",
            idx.keys().collect::<Vec<_>>()
        );
        let u = pick_download_url(&idx, "2.0.0", "linux_x64").expect("url");
        assert!(u.contains("nim-2.0.0-linux_x64.tar.xz"));
        let v = resolve_nim_version(&idx, "linux_x64", "2.0").expect("resolve");
        assert_eq!(v, "2.0.0");
    }
}
