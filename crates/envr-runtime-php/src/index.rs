use envr_domain::runtime::{RemoteFilter, RuntimeVersion};
use envr_error::{EnvrError, EnvrResult};
use serde::Deserialize;
use std::collections::HashMap;
use std::time::Duration;

pub const DEFAULT_PHP_WINDOWS_RELEASES_JSON_URL: &str =
    "https://downloads.php.net/~windows/releases/releases.json";

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
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(45))
        .user_agent(concat!("envr-runtime-php/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| EnvrError::Download(e.to_string()))
}

pub fn fetch_php_windows_releases_json(
    client: &reqwest::blocking::Client,
    url: &str,
) -> EnvrResult<String> {
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

pub fn parse_php_windows_index(json: &str) -> EnvrResult<PhpReleasesIndex> {
    serde_json::from_str(json).map_err(|e| EnvrError::Validation(e.to_string()))
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

pub fn pick_windows_zip(
    line: &ReleaseLine,
    want_ts: Option<bool>,
    arch: &str,
) -> EnvrResult<(String, String)> {
    // Variant keys: `nts-vs17-x64`, `ts-vs17-x64`, etc.
    let arch_key = match arch {
        "x86_64" => "x64",
        "x86" => "x86",
        "aarch64" => "arm64",
        other => other,
    };
    let mut keys: Vec<&str> = line.builds.keys().map(|s| s.as_str()).collect();
    keys.sort();

    let prefer_which = match want_ts {
        Some(true) => vec!["ts", "nts"],
        Some(false) => vec!["nts", "ts"],
        None => vec!["nts", "ts"],
    };
    for which in prefer_which {
        if let Some(k) = keys
            .iter()
            .find(|k| k.contains(which) && k.ends_with(arch_key) && k.contains("vs"))
        {
            let v = line
                .builds
                .get(*k)
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            let entry: BuildEntry =
                serde_json::from_value(v).map_err(|e| EnvrError::Validation(e.to_string()))?;
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
