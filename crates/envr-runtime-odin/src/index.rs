//! Odin: prebuilt monthly toolchains from `odin-lang/Odin` GitHub releases.

use envr_domain::runtime::{
    RemoteFilter, RuntimeKind, RuntimeVersion, numeric_version_segments, version_line_key_for_kind,
};
use envr_download::blocking::build_blocking_http_client;
use envr_error::EnvrResult;
use envr_runtime_github_release::GhRepo;
pub use envr_runtime_github_release::{GhAsset, GhRelease};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::sync::LazyLock;
use std::time::Duration;

pub const DEFAULT_ODIN_RELEASES_API_URL: &str =
    "https://api.github.com/repos/odin-lang/Odin/releases";
const ODIN_REPO: GhRepo = GhRepo {
    owner: "odin-lang",
    name: "Odin",
};

static ODIN_DEV_TAG_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^dev-(\d{4})-(\d{2})([a-z])?$").expect("odin dev tag regex"));

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OdinInstallableRow {
    pub version: String,
    pub url: String,
}

pub fn blocking_http_client() -> EnvrResult<reqwest::blocking::Client> {
    build_blocking_http_client(
        concat!("envr-runtime-odin/", env!("CARGO_PKG_VERSION")),
        Some(Duration::from_secs(120)),
    )
}

fn cmp_release_labels(a: &str, b: &str) -> Ordering {
    match (numeric_version_segments(a), numeric_version_segments(b)) {
        (Some(va), Some(vb)) => va.cmp(&vb),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => a.cmp(b),
    }
}

fn label_from_dev_tag(tag: &str) -> Option<String> {
    let caps = ODIN_DEV_TAG_RE.captures(tag.trim())?;
    let year = caps.get(1)?.as_str();
    let month = caps.get(2)?.as_str();
    let suffix = caps.get(3).map(|m| m.as_str());

    if let Some(s) = suffix {
        let c = s.chars().next()?.to_ascii_lowercase();
        if !c.is_ascii_lowercase() {
            return None;
        }
        let n = (c as u8).saturating_sub(b'a') as u64 + 1;
        Some(format!("{year}.{month}.{n}"))
    } else {
        Some(format!("{year}.{month}"))
    }
}

pub fn odin_asset_prefix_candidates() -> Vec<&'static str> {
    use std::env::consts::{ARCH, OS};
    match (OS, ARCH) {
        ("windows", "x86_64") => vec!["odin-windows-amd64-"],
        ("linux", "x86_64") => vec!["odin-linux-amd64-"],
        ("linux", "aarch64") => vec![
            "odin-linux-arm64-",
            "odin-linux-aarch64-",
            "odin-linux-amd64-",
        ],
        ("macos", "x86_64") => vec!["odin-macos-amd64-"],
        ("macos", "aarch64") => vec![
            "odin-macos-arm64-",
            "odin-macos-aarch64-",
            "odin-macos-amd64-",
        ],
        _ => vec![],
    }
}

fn odin_asset_extension_candidates() -> Vec<&'static str> {
    use std::env::consts::OS;
    match OS {
        "windows" => vec![".zip"],
        _ => vec![".tar.gz"],
    }
}

fn pick_asset_for_tag<'a>(assets: &'a [GhAsset], tag: &str) -> Option<&'a GhAsset> {
    let prefixes = odin_asset_prefix_candidates();
    if prefixes.is_empty() {
        return None;
    }
    let exts = odin_asset_extension_candidates();
    assets.iter().find(|a| {
        prefixes.iter().any(|p| a.name.starts_with(p))
            && exts.iter().any(|e| a.name.ends_with(e))
            && a.name.contains(tag)
    })
}

pub fn installable_rows_from_releases(releases: &[GhRelease]) -> Vec<OdinInstallableRow> {
    let mut out = Vec::new();
    for rel in releases {
        if rel.draft || rel.prerelease {
            continue;
        }
        let tag = rel.tag_name.trim();
        let Some(label) = label_from_dev_tag(tag) else {
            continue;
        };
        let Some(asset) = pick_asset_for_tag(&rel.assets, tag) else {
            continue;
        };
        out.push(OdinInstallableRow {
            version: label,
            url: asset.browser_download_url.clone(),
        });
    }
    out.sort_by(|a, b| cmp_release_labels(&a.version, &b.version));
    out.dedup_by(|a, b| a.version == b.version);
    out
}

pub fn fetch_odin_github_releases_index(
    client: &reqwest::blocking::Client,
    releases_api_url: &str,
) -> EnvrResult<Vec<GhRelease>> {
    envr_runtime_github_release::fetch_github_releases_index(
        client,
        releases_api_url,
        DEFAULT_ODIN_RELEASES_API_URL,
    )
}

fn synthetic_asset_name(tag: &str) -> Option<String> {
    let prefixes = odin_asset_prefix_candidates();
    let exts = odin_asset_extension_candidates();
    let prefix = prefixes.first().copied()?;
    let ext = exts.first().copied()?;
    Some(format!("{prefix}{tag}{ext}"))
}

fn make_synthetic_url(tag: &str, _version: &str) -> Option<String> {
    let asset = synthetic_asset_name(tag)?;
    Some(format!(
        "https://github.com/odin-lang/Odin/releases/download/{tag}/{asset}"
    ))
}

pub fn fetch_odin_installable_rows_with_fallback(
    client: &reqwest::blocking::Client,
    releases_api_url: &str,
) -> EnvrResult<Vec<OdinInstallableRow>> {
    if let Ok(releases) = fetch_odin_github_releases_index(client, releases_api_url) {
        let rows = installable_rows_from_releases(&releases);
        if !rows.is_empty() {
            return Ok(rows);
        }
    }
    let rows = envr_runtime_github_release::fetch_rows_via_atom(
        client,
        ODIN_REPO,
        label_from_dev_tag,
        make_synthetic_url,
        cmp_release_labels,
    )?;
    Ok(rows
        .into_iter()
        .map(|r| OdinInstallableRow {
            version: r.version,
            url: r.url,
        })
        .collect())
}

pub fn list_remote_versions(
    rows: &[OdinInstallableRow],
    filter: &RemoteFilter,
) -> Vec<RuntimeVersion> {
    let mut out: Vec<RuntimeVersion> = rows
        .iter()
        .filter(|r| {
            filter
                .prefix
                .as_ref()
                .map(|p| r.version.starts_with(p))
                .unwrap_or(true)
        })
        .map(|r| RuntimeVersion(r.version.clone()))
        .collect();
    out.sort_by(|a, b| cmp_release_labels(&a.0, &b.0));
    out.dedup_by(|a, b| a.0 == b.0);
    out
}

pub fn list_remote_latest_per_major_lines(rows: &[OdinInstallableRow]) -> Vec<RuntimeVersion> {
    let mut best: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for r in rows {
        let Some(line) = version_line_key_for_kind(RuntimeKind::Odin, &r.version) else {
            continue;
        };
        match best.get(&line) {
            None => {
                best.insert(line, r.version.clone());
            }
            Some(prev) => {
                if cmp_release_labels(prev, &r.version) == Ordering::Less {
                    best.insert(line, r.version.clone());
                }
            }
        }
    }
    let mut out: Vec<RuntimeVersion> = best.values().cloned().map(RuntimeVersion).collect();
    out.sort_by(|a, b| cmp_release_labels(&a.0, &b.0));
    out
}

pub fn resolve_odin_version(rows: &[OdinInstallableRow], spec: &str) -> Option<String> {
    let t = spec.trim();
    if t.is_empty() {
        return None;
    }
    // Allow specifying upstream tag directly.
    if let Some(label) = label_from_dev_tag(t) {
        return Some(label);
    }
    // Exact label.
    if rows.iter().any(|r| r.version == t) {
        return Some(t.to_string());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn odin_dev_tag_maps_to_numeric_label() {
        assert_eq!(
            label_from_dev_tag("dev-2026-04").as_deref(),
            Some("2026.04")
        );
        assert_eq!(
            label_from_dev_tag("dev-2025-12a").as_deref(),
            Some("2025.12.1")
        );
        assert_eq!(
            label_from_dev_tag("DEV-2025-12B").as_deref(),
            Some("2025.12.2")
        );
    }

    #[test]
    fn odin_label_is_numeric_segments() {
        assert_eq!(numeric_version_segments("2026.04").unwrap(), vec![2026, 4]);
        assert_eq!(
            numeric_version_segments("2025.12.1").unwrap(),
            vec![2025, 12, 1]
        );
    }
}
