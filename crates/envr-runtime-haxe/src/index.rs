use envr_domain::runtime::{
    RemoteFilter, RuntimeKind, RuntimeVersion, numeric_version_segments, version_line_key_for_kind,
};
use envr_download::blocking::build_blocking_http_client;
use envr_error::EnvrResult;
use envr_runtime_github_release::{GhAsset, GhRelease, GhRepo};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::LazyLock;
use std::time::Duration;

pub const DEFAULT_HAXE_RELEASES_API_URL: &str =
    "https://api.github.com/repos/HaxeFoundation/haxe/releases";
const HAXE_REPO: GhRepo = GhRepo {
    owner: "HaxeFoundation",
    name: "haxe",
};
static TAG_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^v?(\d+\.\d+\.\d+)$").expect("haxe tag regex"));

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HaxeInstallableRow {
    pub version: String,
    pub url: String,
}

pub fn blocking_http_client() -> EnvrResult<reqwest::blocking::Client> {
    build_blocking_http_client(
        concat!("envr-runtime-haxe/", env!("CARGO_PKG_VERSION")),
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

fn label_from_tag(tag: &str) -> Option<String> {
    TAG_RE
        .captures(tag.trim())
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
}

fn asset_priority(name: &str, version: &str) -> Option<u8> {
    use std::env::consts::{ARCH, OS};
    let v = version;
    match (OS, ARCH) {
        ("windows", "x86_64") => {
            if name == format!("haxe-{v}-win64.zip") {
                Some(0)
            } else {
                None
            }
        }
        ("linux", "x86_64") => {
            if name == format!("haxe-{v}-linux64.tar.gz") {
                Some(0)
            } else {
                None
            }
        }
        ("macos", "x86_64") | ("macos", "aarch64") => {
            if name == format!("haxe-{v}-osx.tar.gz") {
                Some(0)
            } else {
                None
            }
        }
        _ => None,
    }
}

fn pick_asset<'a>(assets: &'a [GhAsset], version: &str) -> Option<&'a GhAsset> {
    assets
        .iter()
        .filter_map(|a| asset_priority(&a.name, version).map(|p| (p, a)))
        .min_by_key(|(p, _)| *p)
        .map(|(_, a)| a)
}

fn installable_rows_from_releases(releases: &[GhRelease]) -> Vec<HaxeInstallableRow> {
    let mut out = Vec::new();
    for rel in releases {
        if rel.draft || rel.prerelease {
            continue;
        }
        let Some(version) = label_from_tag(&rel.tag_name) else {
            continue;
        };
        let Some(asset) = pick_asset(&rel.assets, &version) else {
            continue;
        };
        out.push(HaxeInstallableRow {
            version,
            url: asset.browser_download_url.clone(),
        });
    }
    out.sort_by(|a, b| cmp_release_labels(&a.version, &b.version));
    out.dedup_by(|a, b| a.version == b.version);
    out
}

fn fetch_github_releases_index(
    client: &reqwest::blocking::Client,
    releases_api_url: &str,
) -> EnvrResult<Vec<GhRelease>> {
    envr_runtime_github_release::fetch_github_releases_index(
        client,
        releases_api_url,
        DEFAULT_HAXE_RELEASES_API_URL,
    )
}

fn make_synthetic_url(tag: &str, version: &str) -> Option<String> {
    use std::env::consts::{ARCH, OS};
    let v = version;
    let file = match (OS, ARCH) {
        ("windows", "x86_64") => format!("haxe-{v}-win64.zip"),
        ("linux", "x86_64") => format!("haxe-{v}-linux64.tar.gz"),
        ("macos", "x86_64") | ("macos", "aarch64") => format!("haxe-{v}-osx.tar.gz"),
        _ => return None,
    };
    Some(format!(
        "https://github.com/HaxeFoundation/haxe/releases/download/{tag}/{file}"
    ))
}

pub fn fetch_haxe_installable_rows_with_fallback(
    client: &reqwest::blocking::Client,
    releases_api_url: &str,
) -> EnvrResult<Vec<HaxeInstallableRow>> {
    if let Ok(releases) = fetch_github_releases_index(client, releases_api_url) {
        let rows = installable_rows_from_releases(&releases);
        if !rows.is_empty() {
            return Ok(rows);
        }
    }
    if let Ok(rows) = envr_runtime_github_release::fetch_rows_via_html(
        client,
        HAXE_REPO,
        label_from_tag,
        make_synthetic_url,
        cmp_release_labels,
    ) && !rows.is_empty()
    {
        return Ok(rows
            .into_iter()
            .map(|r| HaxeInstallableRow {
                version: r.version,
                url: r.url,
            })
            .collect());
    }
    let rows = envr_runtime_github_release::fetch_rows_via_atom(
        client,
        HAXE_REPO,
        label_from_tag,
        make_synthetic_url,
        cmp_release_labels,
    )?;
    Ok(rows
        .into_iter()
        .map(|r| HaxeInstallableRow {
            version: r.version,
            url: r.url,
        })
        .collect())
}

pub fn list_remote_versions(
    rows: &[HaxeInstallableRow],
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

pub fn list_remote_latest_per_major_lines(rows: &[HaxeInstallableRow]) -> Vec<RuntimeVersion> {
    let mut best: HashMap<String, String> = HashMap::new();
    for r in rows {
        let Some(line) = version_line_key_for_kind(RuntimeKind::Haxe, &r.version) else {
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

pub fn resolve_haxe_version(rows: &[HaxeInstallableRow], spec: &str) -> Option<String> {
    let t = spec.trim().trim_start_matches('v');
    if t.is_empty() {
        return None;
    }
    if rows.iter().any(|r| r.version == t) {
        return Some(t.to_string());
    }
    if t.chars().all(|c| c.is_ascii_digit() || c == '.') {
        let prefix = format!("{t}.");
        let mut matches: Vec<&str> = rows
            .iter()
            .map(|r| r.version.as_str())
            .filter(|v| *v == t || v.starts_with(&prefix))
            .collect();
        matches.sort_by(|a, b| cmp_release_labels(a, b));
        if let Some(best) = matches.last() {
            return Some((*best).to_string());
        }
    }
    None
}
