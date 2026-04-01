use envr_domain::runtime::{RemoteFilter, RuntimeVersion};
use envr_error::{EnvrError, EnvrResult};
use serde::Deserialize;
use std::time::Duration;

pub const DEFAULT_GO_DL_JSON_URL: &str = "https://go.dev/dl/?mode=json&include=all";

#[derive(Debug, Clone, Deserialize)]
pub struct GoRelease {
    pub version: String,
    #[serde(default)]
    pub stable: bool,
    #[serde(default)]
    pub files: Vec<GoDistFile>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GoDistFile {
    pub filename: String,
    #[serde(default)]
    pub os: String,
    #[serde(default)]
    pub arch: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub sha256: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct SemKey(u64, u64, u64);

fn semver_key_from_go_label(version: &str) -> Option<SemKey> {
    let s = version.trim().strip_prefix("go")?;
    let base = s.split('-').next().unwrap_or(s);
    let mut p = base.split('.');
    let major: u64 = p.next()?.parse().ok()?;
    let minor: u64 = p.next().unwrap_or("0").parse().ok()?;
    let patch: u64 = p.next().unwrap_or("0").parse().ok()?;
    Some(SemKey(major, minor, patch))
}

pub fn normalize_go_version(version: &str) -> String {
    version
        .trim()
        .strip_prefix("go")
        .unwrap_or(version.trim())
        .to_string()
}

pub fn blocking_http_client() -> EnvrResult<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(45))
        .user_agent(concat!("envr-runtime-go/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| EnvrError::Download(e.to_string()))
}

pub fn fetch_go_index(client: &reqwest::blocking::Client, url: &str) -> EnvrResult<String> {
    let response = client
        .get(url)
        .send()
        .map_err(|e| EnvrError::Download(e.to_string()))?;
    if !response.status().is_success() {
        return Err(EnvrError::Download(format!(
            "GET {} -> {}",
            url,
            response.status()
        )));
    }
    response
        .text()
        .map_err(|e| EnvrError::Download(e.to_string()))
}

pub fn parse_go_index(json: &str) -> EnvrResult<Vec<GoRelease>> {
    serde_json::from_str(json).map_err(|e| EnvrError::Validation(e.to_string()))
}

pub fn list_remote_versions(
    releases: &[GoRelease],
    filter: &RemoteFilter,
) -> EnvrResult<Vec<RuntimeVersion>> {
    let mut items: Vec<(SemKey, String)> = releases
        .iter()
        .filter_map(|r| {
            semver_key_from_go_label(&r.version).map(|k| (k, normalize_go_version(&r.version)))
        })
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

pub fn resolve_go_version(releases: &[GoRelease], spec: &str) -> EnvrResult<String> {
    let mut items: Vec<(SemKey, String, bool)> = releases
        .iter()
        .filter_map(|r| {
            semver_key_from_go_label(&r.version)
                .map(|k| (k, normalize_go_version(&r.version), r.stable))
        })
        .collect();
    if items.is_empty() {
        return Err(EnvrError::Validation("no go versions in index".into()));
    }
    items.sort_by(|a, b| b.0.cmp(&a.0));

    let s = spec.trim().trim_start_matches('v').to_ascii_lowercase();
    if s.is_empty() {
        return Err(EnvrError::Validation("empty go version spec".into()));
    }
    if s == "latest" || s == "stable" {
        if let Some((_, v, _)) = items.iter().find(|(_, _, stable)| *stable) {
            return Ok(v.clone());
        }
        return Ok(items[0].1.clone());
    }
    if let Some((_, v, _)) = items.iter().find(|(_, v, _)| v.eq_ignore_ascii_case(&s)) {
        return Ok(v.clone());
    }
    if s.chars().all(|c| c.is_ascii_digit() || c == '.')
        && let Some((_, v, _)) = items.iter().find(|(_, v, _)| v.starts_with(&s))
    {
        return Ok(v.clone());
    }
    Err(EnvrError::Validation(format!(
        "no Go release matches spec {spec:?}"
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_major_minor_picks_latest_patch() {
        let rel = vec![
            GoRelease {
                version: "go1.22.4".into(),
                stable: true,
                files: vec![],
            },
            GoRelease {
                version: "go1.22.6".into(),
                stable: true,
                files: vec![],
            },
            GoRelease {
                version: "go1.23.0".into(),
                stable: true,
                files: vec![],
            },
        ];
        let got = resolve_go_version(&rel, "1.22").expect("resolve");
        assert_eq!(got, "1.22.6");
    }
}
