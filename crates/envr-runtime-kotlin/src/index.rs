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
const KOTLIN_MAVEN_METADATA_URL: &str =
    "https://repo1.maven.org/maven2/org/jetbrains/kotlin/kotlin-compiler/maven-metadata.xml";
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

fn compiler_label_from_asset_name(name: &str) -> Option<String> {
    let s = name.trim();
    let rest = s.strip_prefix("kotlin-compiler-")?;
    let label = rest.strip_suffix(".zip")?;
    if label.is_empty() {
        return None;
    }
    if !label.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        return None;
    }
    Some(label.to_string())
}

/// `(version_label, zip_url)` sorted newest-first (semver when parseable).
pub fn installable_pairs_from_releases(releases: &[GhRelease]) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for rel in releases {
        if rel.draft {
            continue;
        }
        for asset in &rel.assets {
            let Some(label) = compiler_label_from_asset_name(&asset.name) else {
                continue;
            };
            out.push((label, asset.browser_download_url.clone()));
        }
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

fn parse_maven_versions(metadata_xml: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = metadata_xml;
    loop {
        let Some(start) = rest.find("<version>") else {
            break;
        };
        let after_start = &rest[start + "<version>".len()..];
        let Some(end) = after_start.find("</version>") else {
            break;
        };
        let v = after_start[..end].trim();
        if !v.is_empty() && v.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            out.push(v.to_string());
        }
        rest = &after_start[end + "</version>".len()..];
    }
    out
}

fn fetch_pairs_via_maven_metadata(
    client: &reqwest::blocking::Client,
) -> EnvrResult<Vec<(String, String)>> {
    let xml = envr_runtime_github_release::fetch_text(client, KOTLIN_MAVEN_METADATA_URL)?;
    let mut versions = parse_maven_versions(&xml);
    versions.sort_by(|a, b| cmp_semver_release_labels(b, a));
    versions.dedup();
    Ok(versions
        .into_iter()
        .map(|v| {
            let tag = format!("v{v}");
            let url = make_synthetic_url(&tag, &v);
            (v, url)
        })
        .collect())
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

    if let Ok(rows) = envr_runtime_github_release::fetch_rows_via_atom(
        client,
        KOTLIN_REPO,
        label_from_tag,
        |tag, version| Some(make_synthetic_url(tag, version)),
        cmp_semver_release_labels,
    ) && !rows.is_empty()
    {
        return Ok(rows.into_iter().map(|r| (r.version, r.url)).collect());
    }

    if let Ok(pairs) = fetch_pairs_via_maven_metadata(client)
        && !pairs.is_empty()
    {
        return Ok(pairs);
    }

    Ok(Vec::new())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installable_pairs_accept_prerelease_labels_when_zip_exists() {
        let releases = vec![
            GhRelease {
                tag_name: "v2.3.21-RC2".into(),
                draft: false,
                prerelease: true,
                assets: vec![envr_runtime_github_release::GhAsset {
                    name: "kotlin-compiler-2.3.21-RC2.zip".into(),
                    browser_download_url: "https://example.test/2.3.21-RC2.zip".into(),
                }],
            },
            GhRelease {
                tag_name: "v2.3.21".into(),
                draft: false,
                prerelease: false,
                assets: vec![envr_runtime_github_release::GhAsset {
                    name: "kotlin-compiler-2.3.21.zip".into(),
                    browser_download_url: "https://example.test/2.3.21.zip".into(),
                }],
            },
        ];

        let pairs = installable_pairs_from_releases(&releases);
        let labels: Vec<&str> = pairs.iter().map(|(v, _)| v.as_str()).collect();
        assert!(labels.contains(&"2.3.21-RC2"));
        assert!(labels.contains(&"2.3.21"));
    }

    #[test]
    fn installable_pairs_do_not_depend_on_tag_name_shape() {
        let releases = vec![GhRelease {
            tag_name: "build-irrelevant".into(),
            draft: false,
            prerelease: false,
            assets: vec![
                envr_runtime_github_release::GhAsset {
                    name: "kotlin-compiler-2.4.0-Beta2.zip".into(),
                    browser_download_url: "https://example.test/2.4.0-Beta2.zip".into(),
                },
                envr_runtime_github_release::GhAsset {
                    name: "kotlin-compiler-2.4.0-Beta2.zip.sha256".into(),
                    browser_download_url: "https://example.test/2.4.0-Beta2.zip.sha256".into(),
                },
            ],
        }];
        let pairs = installable_pairs_from_releases(&releases);
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0].0, "2.4.0-Beta2");
    }

    #[test]
    fn parse_maven_versions_extracts_version_nodes() {
        let xml = r#"
<metadata>
  <versioning>
    <versions>
      <version>2.3.21</version>
      <version>2.4.0-Beta2</version>
    </versions>
  </versioning>
</metadata>
"#;
        let out = parse_maven_versions(xml);
        assert_eq!(out, vec!["2.3.21", "2.4.0-Beta2"]);
    }
}
