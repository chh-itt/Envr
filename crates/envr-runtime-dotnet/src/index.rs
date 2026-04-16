use envr_error::{EnvrError, EnvrResult};
use serde::Deserialize;
use serde::de::Deserializer;
use std::cmp::Ordering;
use std::time::Duration;

pub const DEFAULT_RELEASES_INDEX_URL: &str =
    "https://dotnetcli.blob.core.windows.net/dotnet/release-metadata/releases-index.json";

#[derive(Debug, Clone)]
pub struct DotnetSdkRelease {
    pub version: String,
    pub files: Vec<DotnetFile>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DotnetFile {
    pub name: String,
    pub rid: Option<String>,
    pub url: String,
}

#[derive(Debug, Deserialize)]
struct ReleasesIndexDoc {
    #[serde(rename = "releases-index")]
    releases_index: Vec<ReleaseChannelEntry>,
}

#[derive(Debug, Deserialize)]
struct ReleaseChannelEntry {
    #[serde(rename = "channel-version")]
    channel_version: String,
    #[serde(rename = "releases.json")]
    releases_json_url: String,
}

#[derive(Debug, Deserialize)]
struct ChannelReleasesDoc {
    #[serde(default, deserialize_with = "null_to_default_vec")]
    releases: Vec<ChannelReleaseEntry>,
}

#[derive(Debug, Deserialize)]
struct ChannelReleaseEntry {
    sdk: Option<SdkEntry>,
    #[serde(default, deserialize_with = "null_to_default_vec")]
    sdks: Vec<SdkEntry>,
}

#[derive(Debug, Clone, Deserialize)]
struct SdkEntry {
    version: String,
    #[serde(default, deserialize_with = "null_to_default_vec")]
    files: Vec<DotnetFile>,
}

fn null_to_default_vec<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<Vec<T>>::deserialize(deserializer).map(|opt| opt.unwrap_or_default())
}

pub fn blocking_http_client() -> EnvrResult<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(90))
        .user_agent(concat!("envr-runtime-dotnet/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| EnvrError::Download(e.to_string()))
}

fn fetch_text(client: &reqwest::blocking::Client, url: &str) -> EnvrResult<String> {
    let r = client
        .get(url)
        .send()
        .map_err(|e| EnvrError::Download(e.to_string()))?;
    if !r.status().is_success() {
        return Err(EnvrError::Download(format!("GET {url} -> {}", r.status())));
    }
    r.text().map_err(|e| EnvrError::Download(e.to_string()))
}

fn parse_triplet(v: &str) -> Option<(u64, u64, u64)> {
    let t = v.trim().trim_start_matches('v');
    let mut it = t.split('.');
    let a = it.next()?.parse::<u64>().ok()?;
    let b = it.next()?.parse::<u64>().ok()?;
    let c = it.next()?.parse::<u64>().ok()?;
    if it.next().is_some() {
        return None;
    }
    Some((a, b, c))
}

fn parse_major_minor(v: &str) -> Option<(u64, u64)> {
    let t = v.trim().trim_start_matches('v');
    let mut it = t.split('.');
    let a = it.next()?.parse::<u64>().ok()?;
    let b = it.next()?.parse::<u64>().ok()?;
    if it.next().is_some() {
        return None;
    }
    Some((a, b))
}

fn cmp_triplet(a: &str, b: &str) -> Ordering {
    parse_triplet(a).cmp(&parse_triplet(b))
}

pub fn load_sdk_releases(
    client: &reqwest::blocking::Client,
    releases_index_url: &str,
) -> EnvrResult<Vec<DotnetSdkRelease>> {
    let idx_body = fetch_text(client, releases_index_url)?;
    let idx: ReleasesIndexDoc =
        serde_json::from_str(&idx_body).map_err(|e| EnvrError::Validation(e.to_string()))?;

    let mut out = Vec::<DotnetSdkRelease>::new();
    for channel in idx.releases_index {
        if parse_major_minor(&channel.channel_version).is_none() {
            continue;
        }
        let body = fetch_text(client, &channel.releases_json_url)?;
        let rel: ChannelReleasesDoc =
            serde_json::from_str(&body).map_err(|e| EnvrError::Validation(e.to_string()))?;
        for r in rel.releases {
            if let Some(sdk) = r.sdk {
                out.push(DotnetSdkRelease {
                    version: sdk.version,
                    files: sdk.files,
                });
            }
            for sdk in r.sdks {
                out.push(DotnetSdkRelease {
                    version: sdk.version,
                    files: sdk.files,
                });
            }
        }
    }

    out.sort_by(|a, b| cmp_triplet(&a.version, &b.version));
    out.dedup_by(|a, b| a.version == b.version);
    Ok(out)
}

fn host_rid() -> EnvrResult<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "x86_64") => Ok("win-x64"),
        ("windows", "aarch64") => Ok("win-arm64"),
        ("linux", "x86_64") => Ok("linux-x64"),
        ("linux", "aarch64") => Ok("linux-arm64"),
        ("macos", "x86_64") => Ok("osx-x64"),
        ("macos", "aarch64") => Ok("osx-arm64"),
        (os, arch) => Err(EnvrError::Platform(format!(
            "unsupported host for dotnet install: {os}-{arch}"
        ))),
    }
}

pub fn pick_install_file(files: &[DotnetFile], version: &str) -> EnvrResult<DotnetFile> {
    let rid = host_rid()?;
    let version_lc = version.to_ascii_lowercase();
    let rid_lc = rid.to_ascii_lowercase();
    let mut best: Option<(usize, DotnetFile)> = None;

    for f in files {
        let name_lc = f.name.to_ascii_lowercase();
        let url_lc = f.url.to_ascii_lowercase();
        let rid_match = f
            .rid
            .as_deref()
            .map(|s| s.eq_ignore_ascii_case(rid))
            .unwrap_or(true);
        if !rid_match {
            continue;
        }
        let archive_ok = name_lc.ends_with(".zip")
            || name_lc.ends_with(".tar.gz")
            || url_lc.ends_with(".zip")
            || url_lc.ends_with(".tar.gz");
        if !archive_ok {
            continue;
        }
        let sdk_hint = name_lc.contains("sdk") || url_lc.contains("sdk");
        if !sdk_hint {
            continue;
        }

        let mut score = 0usize;
        if name_lc.contains(&version_lc) || url_lc.contains(&version_lc) {
            score += 2;
        }
        if name_lc.contains(&rid_lc) || url_lc.contains(&rid_lc) {
            score += 2;
        }
        if f.rid.as_deref().is_some() {
            score += 1;
        }
        if name_lc.starts_with("dotnet-sdk-") {
            score += 1;
        }

        match &best {
            Some((best_score, _)) if *best_score >= score => {}
            _ => best = Some((score, f.clone())),
        }
    }

    best.map(|(_, file)| file).ok_or_else(|| {
        EnvrError::Validation(format!(
            "no dotnet sdk artifact for version {version} on {rid}"
        ))
    })
}

pub fn resolve_dotnet_version(releases: &[DotnetSdkRelease], spec: &str) -> EnvrResult<String> {
    let raw = spec.trim().trim_start_matches('v');
    if parse_triplet(raw).is_some() {
        if releases.iter().any(|r| r.version == raw) {
            return Ok(raw.to_string());
        }
        return Err(EnvrError::Validation(format!(
            "dotnet sdk version not found: {raw}"
        )));
    }
    let mut parts = raw.split('.');
    let major = parts.next().unwrap_or("");
    let minor = parts.next();
    if major.is_empty() || !major.chars().all(|c| c.is_ascii_digit()) {
        return Err(EnvrError::Validation(format!(
            "unsupported dotnet spec {spec:?}; use major (8), major.minor (8.0), or full (8.0.100)"
        )));
    }
    let mut candidates: Vec<&DotnetSdkRelease> = releases
        .iter()
        .filter(|r| {
            let mut it = r.version.split('.');
            let a = it.next().unwrap_or("");
            let b = it.next().unwrap_or("");
            if a != major {
                return false;
            }
            if let Some(m) = minor {
                return b == m;
            }
            true
        })
        .collect();
    candidates.sort_by(|a, b| cmp_triplet(&a.version, &b.version));
    candidates
        .last()
        .map(|r| r.version.clone())
        .ok_or_else(|| EnvrError::Validation(format!("no dotnet sdk matches spec {spec:?}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn releases() -> Vec<DotnetSdkRelease> {
        vec![
            DotnetSdkRelease {
                version: "8.0.100".into(),
                files: vec![],
            },
            DotnetSdkRelease {
                version: "8.0.201".into(),
                files: vec![],
            },
            DotnetSdkRelease {
                version: "9.0.100".into(),
                files: vec![],
            },
        ]
    }

    #[test]
    fn resolve_full_version_keeps_exact() {
        let got = resolve_dotnet_version(&releases(), "8.0.100").expect("resolve");
        assert_eq!(got, "8.0.100");
    }

    #[test]
    fn resolve_major_picks_latest_line() {
        let got = resolve_dotnet_version(&releases(), "8").expect("resolve");
        assert_eq!(got, "8.0.201");
    }

    #[test]
    fn resolve_major_minor_picks_latest_patch() {
        let got = resolve_dotnet_version(&releases(), "8.0").expect("resolve");
        assert_eq!(got, "8.0.201");
    }

    #[test]
    fn resolve_invalid_spec_rejected() {
        let err = resolve_dotnet_version(&releases(), "latest").expect_err("bad");
        assert!(matches!(err, EnvrError::Validation(_)));
    }
}
