use envr_domain::runtime::{RemoteFilter, RuntimeVersion, numeric_version_segments};
use envr_download::blocking::build_blocking_http_client;
use envr_error::{EnvrError, EnvrResult, ErrorCode};
use envr_runtime_github_release::{GhRepo, InstallableRow};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::time::Duration;

pub const DEFAULT_LUAU_RELEASES_API_URL: &str = "https://api.github.com/repos/luau-lang/luau/releases";
const LUAU_REPO: GhRepo = GhRepo {
    owner: "luau-lang",
    name: "luau",
};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GhAsset {
    pub name: String,
    pub browser_download_url: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct GhRelease {
    pub tag_name: String,
    #[serde(default)]
    pub draft: bool,
    #[serde(default)]
    pub prerelease: bool,
    #[serde(default)]
    pub assets: Vec<GhAsset>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LuauInstallableRow {
    pub version: String,
    pub url: String,
}

pub fn blocking_http_client() -> EnvrResult<reqwest::blocking::Client> {
    build_blocking_http_client(
        concat!("envr-runtime-luau/", env!("CARGO_PKG_VERSION")),
        Some(Duration::from_secs(120)),
    )
}

fn github_api_auth_token() -> Option<String> {
    ["GITHUB_TOKEN", "GH_TOKEN", "ENVR_GITHUB_TOKEN"]
        .into_iter()
        .find_map(|k| std::env::var(k).ok())
        .and_then(|s| {
            let t = s.trim();
            if t.is_empty() {
                None
            } else {
                Some(t.to_string())
            }
        })
}

fn fetch_text(client: &reqwest::blocking::Client, url: &str) -> EnvrResult<String> {
    let mut req = client
        .get(url)
        .header("Accept", "application/vnd.github+json");
    if url.contains("api.github.com") {
        req = req.header("X-GitHub-Api-Version", "2022-11-28");
        if let Some(tok) = github_api_auth_token() {
            req = req.header("Authorization", format!("Bearer {tok}"));
        }
    }
    let response = req.send().map_err(|e| {
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

fn cmp_release_labels(a: &str, b: &str) -> Ordering {
    match (numeric_version_segments(a), numeric_version_segments(b)) {
        (Some(va), Some(vb)) => va.cmp(&vb),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => a.cmp(b),
    }
}

fn host_asset_candidates() -> Vec<&'static str> {
    use std::env::consts::{ARCH, OS};
    match (OS, ARCH) {
        ("windows", "x86_64") => vec!["luau-windows.zip"],
        ("linux", "x86_64") => vec!["luau-ubuntu.zip"],
        ("macos", "x86_64") | ("macos", "aarch64") => vec!["luau-macos.zip"],
        _ => vec![],
    }
}

fn pick_asset<'a>(assets: &'a [GhAsset]) -> Option<&'a GhAsset> {
    let cands = host_asset_candidates();
    assets
        .iter()
        .find(|a| cands.iter().any(|name| a.name.eq_ignore_ascii_case(name)))
}

fn normalize_release_label(tag: &str) -> Option<String> {
    let t = tag.trim().trim_start_matches('v').trim_start_matches('V');
    if numeric_version_segments(t).is_some() {
        Some(t.to_string())
    } else {
        None
    }
}

fn synthetic_asset_url(tag: &str) -> Option<String> {
    let fname = host_asset_candidates().into_iter().next()?;
    Some(format!(
        "https://github.com/luau-lang/luau/releases/download/{tag}/{fname}"
    ))
}

fn fetch_github_releases_index(
    client: &reqwest::blocking::Client,
    releases_api_url: &str,
) -> EnvrResult<Vec<GhRelease>> {
    let mut page = 1;
    let mut out = Vec::new();
    loop {
        let sep = if releases_api_url.contains('?') { '&' } else { '?' };
        let url = format!("{releases_api_url}{sep}per_page=100&page={page}");
        let text = fetch_text(client, &url)?;
        let arr: Vec<GhRelease> = serde_json::from_str(&text).map_err(|e| {
            EnvrError::with_source(ErrorCode::Validation, "invalid github releases json", e)
        })?;
        if arr.is_empty() {
            break;
        }
        let n = arr.len();
        out.extend(arr);
        if n < 100 {
            break;
        }
        page += 1;
    }
    Ok(out)
}

pub fn fetch_luau_installable_rows(
    client: &reqwest::blocking::Client,
    releases_api_url: &str,
) -> EnvrResult<Vec<LuauInstallableRow>> {
    let mut by_version: HashMap<String, String> = HashMap::new();

    // Primary: GitHub Releases API (full fidelity assets).
    if let Ok(releases) = fetch_github_releases_index(client, releases_api_url) {
        for rel in releases {
            if rel.draft || rel.prerelease {
                continue;
            }
            let Some(version) = normalize_release_label(&rel.tag_name) else {
                continue;
            };
            let Some(asset) = pick_asset(&rel.assets) else {
                continue;
            };
            by_version
                .entry(version)
                .or_insert(asset.browser_download_url.clone());
        }
    }

    // Fallbacks: `github.com/.../releases` HTML / Atom; we cannot enumerate assets reliably, so we
    // synthesize the canonical asset URL by host.
    if by_version.is_empty() {
        let rows: Vec<InstallableRow> =
            if let Ok(rows) = envr_runtime_github_release::fetch_rows_via_html(
                client,
                LUAU_REPO,
                normalize_release_label,
                |tag, _version| synthetic_asset_url(tag),
                cmp_release_labels,
            ) && !rows.is_empty()
            {
                rows
            } else {
                envr_runtime_github_release::fetch_rows_via_atom(
                    client,
                    LUAU_REPO,
                    normalize_release_label,
                    |tag, _version| synthetic_asset_url(tag),
                    cmp_release_labels,
                )?
            };

        for r in rows {
            by_version.entry(r.version).or_insert(r.url);
        }
    }

    let mut out: Vec<LuauInstallableRow> = by_version
        .into_iter()
        .map(|(version, url)| LuauInstallableRow { version, url })
        .collect();
    out.sort_by(|a, b| cmp_release_labels(&a.version, &b.version));
    Ok(out)
}

pub fn list_remote_versions(rows: &[LuauInstallableRow], filter: &RemoteFilter) -> Vec<RuntimeVersion> {
    let mut out: Vec<RuntimeVersion> = rows
        .iter()
        .filter(|r| {
            filter
                .prefix
                .as_ref()
                .map(|p| r.version.starts_with(p.trim()))
                .unwrap_or(true)
        })
        .map(|r| RuntimeVersion(r.version.clone()))
        .collect();
    out.sort_by(|a, b| cmp_release_labels(&a.0, &b.0));
    out.dedup_by(|a, b| a.0 == b.0);
    out
}

pub fn list_remote_latest_per_major_lines(rows: &[LuauInstallableRow]) -> Vec<RuntimeVersion> {
    let mut best: HashMap<String, String> = HashMap::new();
    for r in rows {
        let Some(parts) = numeric_version_segments(&r.version) else {
            continue;
        };
        if parts.is_empty() {
            continue;
        }
        let line = if parts.len() >= 2 {
            format!("{}.{}", parts[0], parts[1])
        } else {
            parts[0].to_string()
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
    let mut out: Vec<RuntimeVersion> = best.into_values().map(RuntimeVersion).collect();
    out.sort_by(|a, b| cmp_release_labels(&a.0, &b.0));
    out
}

pub fn resolve_luau_version(rows: &[LuauInstallableRow], spec: &str) -> Option<String> {
    let s = spec.trim().trim_start_matches('v').trim_start_matches('V');
    if s.is_empty() || s.eq_ignore_ascii_case("latest") {
        return rows
            .iter()
            .map(|r| r.version.clone())
            .max_by(|a, b| cmp_release_labels(a, b));
    }
    if rows.iter().any(|r| r.version == s) {
        return Some(s.to_string());
    }
    let mut best: Option<String> = None;
    for r in rows {
        if r.version == s || r.version.starts_with(&format!("{s}.")) {
            match &best {
                None => best = Some(r.version.clone()),
                Some(prev) => {
                    if cmp_release_labels(prev, &r.version) == Ordering::Less {
                        best = Some(r.version.clone());
                    }
                }
            }
        }
    }
    best
}
