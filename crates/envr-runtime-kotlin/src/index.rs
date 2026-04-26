//! Kotlin compiler bundles from JetBrains GitHub releases (`kotlin-compiler-<ver>.zip`).

use envr_domain::runtime::{RemoteFilter, RuntimeKind, RuntimeVersion, version_line_key_for_kind};
use envr_download::blocking::build_blocking_http_client;
use envr_error::{EnvrError, EnvrResult};
use envr_runtime_github_release::{GhRelease, GhRepo};
use std::cmp::Ordering;
use std::collections::HashSet;
use std::time::Duration;

pub const DEFAULT_KOTLIN_RELEASES_API_URL: &str =
    "https://api.github.com/repos/JetBrains/kotlin/releases?per_page=100";
const KOTLIN_REPO: GhRepo = GhRepo {
    owner: "JetBrains",
    name: "kotlin",
};

pub fn blocking_http_client() -> EnvrResult<reqwest::blocking::Client> {
    build_blocking_http_client(
        concat!("envr-runtime-kotlin/", env!("CARGO_PKG_VERSION")),
        Some(Duration::from_secs(120)),
    )
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

fn label_from_tag(tag: &str) -> Option<String> {
    let t = tag.trim().strip_prefix('v')?;
    if t.is_empty() {
        return None;
    }
    if !t.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        return None;
    }
    Some(t.to_string())
}

fn compiler_zip_url(release: &GhRelease, label: &str) -> Option<String> {
    let want = format!("kotlin-compiler-{label}.zip");
    release
        .assets
        .iter()
        .find(|a| a.name == want)
        .map(|a| a.browser_download_url.clone())
}

/// `(version_label, zip_url)` sorted newest-first (semver when parseable).
pub fn installable_pairs_from_releases(releases: &[GhRelease]) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for rel in releases {
        if rel.draft {
            continue;
        }
        let Some(label) = label_from_tag(&rel.tag_name) else {
            continue;
        };
        if !label
            .chars()
            .all(|c| c.is_ascii_digit() || c == '.' || c == '-')
        {
            continue;
        }
        // Skip odd tags without a matching compiler zip (e.g. metadata-only tags).
        let Some(url) = compiler_zip_url(rel, &label) else {
            continue;
        };
        out.push((label, url));
    }
    out.sort_by(|a, b| cmp_semver_release_labels(&b.0, &a.0));
    out.dedup_by(|a, b| a.0 == b.0);
    out
}

fn fetch_github_releases_index(
    client: &reqwest::blocking::Client,
    releases_api_url: &str,
) -> EnvrResult<Vec<GhRelease>> {
    envr_runtime_github_release::fetch_github_releases_index(
        client,
        releases_api_url,
        DEFAULT_KOTLIN_RELEASES_API_URL,
    )
}

fn make_synthetic_url(tag: &str, version: &str) -> String {
    format!(
        "https://github.com/JetBrains/kotlin/releases/download/{tag}/kotlin-compiler-{version}.zip"
    )
}

pub fn fetch_kotlin_installable_pairs_with_fallback(
    client: &reqwest::blocking::Client,
    releases_api_url: &str,
) -> EnvrResult<Vec<(String, String)>> {
    if let Ok(releases) = fetch_github_releases_index(client, releases_api_url) {
        let pairs = installable_pairs_from_releases(&releases);
        if !pairs.is_empty() {
            return Ok(pairs);
        }
    }

    if let Ok(rows) = envr_runtime_github_release::fetch_rows_via_html(
        client,
        KOTLIN_REPO,
        label_from_tag,
        |tag, version| Some(make_synthetic_url(tag, version)),
        cmp_semver_release_labels,
    ) && !rows.is_empty()
    {
        return Ok(rows.into_iter().map(|r| (r.version, r.url)).collect());
    }

    let rows = envr_runtime_github_release::fetch_rows_via_atom(
        client,
        KOTLIN_REPO,
        label_from_tag,
        |tag, version| Some(make_synthetic_url(tag, version)),
        cmp_semver_release_labels,
    )?;
    Ok(rows.into_iter().map(|r| (r.version, r.url)).collect())
}

pub fn list_remote_versions(
    pairs: &[(String, String)],
    filter: &RemoteFilter,
) -> Vec<RuntimeVersion> {
    let mut labels: Vec<String> = pairs.iter().map(|(l, _)| l.clone()).collect();
    if let Some(prefix) = filter.prefix.as_deref() {
        let p = prefix.trim();
        if !p.is_empty() {
            labels.retain(|k| k.starts_with(p));
        }
    }
    labels.into_iter().map(RuntimeVersion).collect()
}

pub fn list_remote_latest_per_major_lines(pairs: &[(String, String)]) -> Vec<RuntimeVersion> {
    let mut seen = HashSet::<String>::new();
    let mut out = Vec::new();
    for (label, _) in pairs {
        if let Some(line) = version_line_key_for_kind(RuntimeKind::Kotlin, label) {
            if seen.insert(line) {
                out.push(RuntimeVersion(label.clone()));
            }
        }
    }
    out
}

pub fn resolve_kotlin_version(pairs: &[(String, String)], spec: &str) -> EnvrResult<String> {
    let s = spec.trim().trim_start_matches('v').trim_start_matches('V');
    if s.is_empty() {
        return Err(EnvrError::Validation("empty kotlin version spec".into()));
    }
    let labels: Vec<&str> = pairs.iter().map(|(l, _)| l.as_str()).collect();
    if labels.iter().any(|k| *k == s) {
        return Ok(s.to_string());
    }

    use envr_domain::runtime::numeric_version_segments;
    if let Some(parts) = numeric_version_segments(s) {
        match parts.len() {
            1 => {
                let major = parts[0];
                let best = pairs.iter().map(|(l, _)| l.as_str()).find(|label| {
                    numeric_version_segments(label).is_some_and(|p| !p.is_empty() && p[0] == major)
                });
                if let Some(b) = best {
                    return Ok(b.to_string());
                }
            }
            2 => {
                let major = parts[0];
                let minor = parts[1];
                let best = pairs.iter().map(|(l, _)| l.as_str()).find(|label| {
                    numeric_version_segments(label)
                        .is_some_and(|p| p.len() >= 2 && p[0] == major && p[1] == minor)
                });
                if let Some(b) = best {
                    return Ok(b.to_string());
                }
            }
            _ => {}
        }
    }

    Err(EnvrError::Validation(format!(
        "no kotlin release matches spec `{s}` (try a full label like 2.0.21)"
    )))
}
