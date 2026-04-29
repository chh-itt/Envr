use envr_domain::runtime::{RemoteFilter, RuntimeVersion};
use envr_download::blocking::build_blocking_http_client;
use envr_error::{EnvrError, EnvrResult, ErrorCode};
use regex::Regex;
use std::cmp::Ordering;
use std::time::Duration;

pub const DEFAULT_RUBY_RELEASES_URL: &str = "https://www.ruby-lang.org/en/downloads/releases/";
pub const DEFAULT_RUBYINSTALLER_DOWNLOADS_URL: &str = "https://rubyinstaller.org/downloads/";

pub fn blocking_http_client() -> EnvrResult<reqwest::blocking::Client> {
    // RubyInstaller `.7z` assets are large; in some networks the request may
    // need multiple tens of seconds just for TLS + first bytes.
    build_blocking_http_client(
        concat!("envr-runtime-ruby/", env!("CARGO_PKG_VERSION")),
        Some(Duration::from_secs(180)),
    )
}

pub fn fetch_release_page(client: &reqwest::blocking::Client, url: &str) -> EnvrResult<String> {
    let response = client.get(url).send().map_err(|e| {
        EnvrError::with_source(ErrorCode::Download, format!("request failed for {url}"), e)
    })?;
    if !response.status().is_success() {
        return Err(EnvrError::Download(format!(
            "release page request failed: {} {}",
            response.status(),
            url
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RubyRelease {
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RubyInstallerArtifact {
    pub ruby_version: String,
    pub installer_version: String,
    pub arch: String,
    pub url: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct SemVerKey(u64, u64, u64);

fn semver_key(version: &str) -> EnvrResult<SemVerKey> {
    let mut parts = version.trim().trim_start_matches('v').split('.');
    let major = parts
        .next()
        .ok_or_else(|| EnvrError::Validation(format!("invalid ruby version: {version}")))?
        .parse::<u64>()
        .map_err(|_| EnvrError::Validation(format!("invalid ruby version: {version}")))?;
    let minor = parts
        .next()
        .unwrap_or("0")
        .parse::<u64>()
        .map_err(|_| EnvrError::Validation(format!("invalid ruby version: {version}")))?;
    let patch = parts
        .next()
        .unwrap_or("0")
        .parse::<u64>()
        .map_err(|_| EnvrError::Validation(format!("invalid ruby version: {version}")))?;
    Ok(SemVerKey(major, minor, patch))
}

fn is_full_semver(version: &str) -> bool {
    let trimmed = version.trim().trim_start_matches('v');
    let parts: Vec<&str> = trimmed.split('.').collect();
    parts.len() == 3
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
}

pub(crate) fn cmp_semver(a: &str, b: &str) -> Ordering {
    let ka = semver_key(a).unwrap_or(SemVerKey(0, 0, 0));
    let kb = semver_key(b).unwrap_or(SemVerKey(0, 0, 0));
    ka.cmp(&kb)
}

/// Latest semver **per major** (first component) using only versions published as RubyInstaller
/// `.7z` artifacts. This avoids offering ruby-lang.org releases that do not yet have installers.
pub fn parse_ruby_releases(html: &str) -> EnvrResult<Vec<RubyRelease>> {
    let re = Regex::new(r"Ruby\s+(\d+\.\d+\.\d+)").map_err(|e| {
        EnvrError::with_source(ErrorCode::Validation, "invalid ruby version regex", e)
    })?;
    let mut versions: Vec<RubyRelease> = re
        .captures_iter(html)
        .filter_map(|caps| caps.get(1).map(|m| m.as_str().to_string()))
        .map(|version| RubyRelease { version })
        .collect();
    versions.sort_by(|a, b| cmp_semver(&a.version, &b.version));
    versions.dedup_by(|a, b| a.version == b.version);
    if versions.is_empty() {
        return Err(EnvrError::Validation(
            "no ruby releases parsed from official release page".into(),
        ));
    }
    Ok(versions)
}

#[cfg(windows)]
pub fn parse_rubyinstaller_7z_artifacts(html: &str) -> EnvrResult<Vec<RubyInstallerArtifact>> {
    let re = Regex::new(
        r#"https://github\.com/oneclick/rubyinstaller2/releases/download/RubyInstaller-(\d+\.\d+\.\d+-\d+)/rubyinstaller-\d+\.\d+\.\d+-\d+-(x64|x86|arm)\.7z\.asc"#,
    )
    .map_err(|e| EnvrError::with_source(ErrorCode::Validation, "invalid rubyinstaller version regex", e))?;
    let mut out = Vec::new();
    for caps in re.captures_iter(html) {
        let installer_version = caps
            .get(1)
            .map(|m| m.as_str().to_string())
            .ok_or_else(|| EnvrError::Validation("missing installer version".into()))?;
        let arch = caps
            .get(2)
            .map(|m| m.as_str().to_string())
            .ok_or_else(|| EnvrError::Validation("missing rubyinstaller arch".into()))?;
        let ruby_version = installer_version
            .rsplit_once('-')
            .map(|(v, _)| v.to_string())
            .unwrap_or_else(|| installer_version.clone());
        let full = caps
            .get(0)
            .map(|m| m.as_str().to_string())
            .ok_or_else(|| EnvrError::Validation("missing rubyinstaller artifact url".into()))?;
        out.push(RubyInstallerArtifact {
            ruby_version,
            installer_version,
            arch,
            url: full.trim_end_matches(".asc").to_string(),
        });
    }
    out.sort_by(|a, b| cmp_semver(&a.ruby_version, &b.ruby_version));
    out.dedup_by(|a, b| a.url == b.url);
    if out.is_empty() {
        return Err(EnvrError::Validation(
            "no rubyinstaller 7z artifacts parsed from downloads page".into(),
        ));
    }
    Ok(out)
}

fn normalize_prefix(prefix: &str) -> String {
    prefix.trim().trim_start_matches('v').to_ascii_lowercase()
}

pub fn list_remote_versions(
    releases: &[RubyRelease],
    filter: &RemoteFilter,
) -> EnvrResult<Vec<RuntimeVersion>> {
    let mut items: Vec<RuntimeVersion> = releases
        .iter()
        .map(|r| RuntimeVersion(r.version.clone()))
        .collect();
    items.sort_by(|a, b| cmp_semver(&b.0, &a.0));
    if let Some(prefix) = &filter.prefix {
        let p = normalize_prefix(prefix);
        if !p.is_empty() {
            items.retain(|v| v.0.to_ascii_lowercase().starts_with(&p));
        }
    }
    Ok(items)
}

pub fn list_latest_patch_per_major(releases: &[RubyRelease]) -> EnvrResult<Vec<RuntimeVersion>> {
    use std::collections::BTreeMap;
    let mut by_major_minor = BTreeMap::<(u64, u64), RuntimeVersion>::new();
    for r in releases {
        let key = semver_key(&r.version)?;
        by_major_minor.insert((key.0, key.1), RuntimeVersion(r.version.clone()));
    }
    let mut out: Vec<RuntimeVersion> = by_major_minor.into_values().collect();
    out.sort_by(|a, b| cmp_semver(&b.0, &a.0));
    Ok(out)
}

pub fn resolve_ruby_version(releases: &[RubyRelease], spec: &str) -> EnvrResult<String> {
    let raw = spec.trim().trim_start_matches('v');
    if is_full_semver(raw) {
        if releases.iter().any(|r| r.version == raw) {
            return Ok(raw.to_string());
        }
        return Err(EnvrError::Validation(format!(
            "ruby version not found: {raw}"
        )));
    }

    let parts: Vec<&str> = raw.split('.').collect();
    if parts.is_empty() || parts.iter().any(|p| p.is_empty()) {
        return Err(EnvrError::Validation(format!(
            "unsupported ruby spec {spec:?}; use major (3), major.minor (3.3), or full (3.3.6)"
        )));
    }
    if !parts.iter().all(|p| p.chars().all(|c| c.is_ascii_digit())) {
        return Err(EnvrError::Validation(format!(
            "unsupported ruby spec {spec:?}; use major (3), major.minor (3.3), or full (3.3.6)"
        )));
    }

    let mut candidates: Vec<&RubyRelease> = releases
        .iter()
        .filter(|r| {
            let version_parts: Vec<&str> = r.version.split('.').collect();
            if parts.len() > version_parts.len() {
                return false;
            }
            parts
                .iter()
                .zip(version_parts.iter())
                .all(|(want, got)| want == got)
        })
        .collect();
    candidates.sort_by(|a, b| cmp_semver(&a.version, &b.version));
    candidates
        .last()
        .map(|r| r.version.clone())
        .ok_or_else(|| EnvrError::Validation(format!("no ruby version matches spec {spec:?}")))
}

#[cfg(windows)]
pub fn host_rubyinstaller_arch() -> EnvrResult<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "x86_64") => Ok("x64"),
        ("windows", "x86") => Ok("x86"),
        ("windows", "aarch64") => Ok("arm"),
        (os, arch) => Err(EnvrError::Platform(format!(
            "rubyinstaller unsupported host: {os}-{arch}"
        ))),
    }
}

#[cfg(windows)]
pub fn pick_rubyinstaller_artifact(
    artifacts: &[RubyInstallerArtifact],
    version: &str,
) -> EnvrResult<RubyInstallerArtifact> {
    let arch = host_rubyinstaller_arch()?;
    artifacts
        .iter()
        .find(|a| a.ruby_version == version && a.arch == arch)
        .cloned()
        .ok_or_else(|| {
            EnvrError::Validation(format!(
                "no rubyinstaller 7z artifact for version {version} on arch {arch}"
            ))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ruby_releases_extracts_unique_versions() {
        let html = r#"
            <a>Ruby 3.3.6</a>
            <a>Ruby 3.4.1</a>
            <a>Ruby 3.3.6</a>
        "#;
        let got = parse_ruby_releases(html).expect("parse");
        assert_eq!(
            got,
            vec![
                RubyRelease {
                    version: "3.3.6".into()
                },
                RubyRelease {
                    version: "3.4.1".into()
                }
            ]
        );
    }

    #[test]
    fn resolve_ruby_version_supports_major_and_minor_prefix() {
        let releases = vec![
            RubyRelease {
                version: "3.2.9".into(),
            },
            RubyRelease {
                version: "3.3.6".into(),
            },
            RubyRelease {
                version: "3.3.7".into(),
            },
        ];
        assert_eq!(resolve_ruby_version(&releases, "3").unwrap(), "3.3.7");
        assert_eq!(resolve_ruby_version(&releases, "3.3").unwrap(), "3.3.7");
        assert_eq!(resolve_ruby_version(&releases, "3.2.9").unwrap(), "3.2.9");
    }

    #[test]
    fn parse_rubyinstaller_7z_artifacts_derives_download_url() {
        let html = r#"
            <a href="https://github.com/oneclick/rubyinstaller2/releases/download/RubyInstaller-3.3.11-1/rubyinstaller-3.3.11-1-x64.7z.asc">sig</a>
        "#;
        let got = parse_rubyinstaller_7z_artifacts(html).expect("parse");
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].ruby_version, "3.3.11");
        assert_eq!(got[0].installer_version, "3.3.11-1");
        assert_eq!(got[0].arch, "x64");
        assert_eq!(
            got[0].url,
            "https://github.com/oneclick/rubyinstaller2/releases/download/RubyInstaller-3.3.11-1/rubyinstaller-3.3.11-1-x64.7z"
        );
    }
}
