use envr_domain::runtime::{RemoteFilter, RuntimeVersion};
use envr_error::{EnvrError, EnvrResult};
use serde::Deserialize;
use std::collections::HashMap;
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

/// Map Rust `std::env::consts::OS` to Go download JSON `os` field (`macos` → `darwin`).
pub fn go_dl_os_for_rust(os: &str) -> &str {
    match os {
        "macos" => "darwin",
        other => other,
    }
}

/// Map Rust `std::env::consts::ARCH` to Go download JSON `arch` (`x86_64` → `amd64`, etc.).
pub fn go_dl_arch_for_rust(arch: &str) -> &str {
    match arch {
        "x86_64" => "amd64",
        "aarch64" => "arm64",
        "x86" | "i686" => "386",
        other => other,
    }
}

/// True when this release has an installable archive for `GOOS`/`GOARCH` (archive or empty kind).
pub fn go_release_has_installable_archive(release: &GoRelease, os: &str, arch: &str) -> bool {
    let go_os = go_dl_os_for_rust(os);
    let go_arch = go_dl_arch_for_rust(arch);
    let want_ext = if go_os == "windows" {
        ".zip"
    } else {
        ".tar.gz"
    };
    release.files.iter().any(|f| {
        f.os == go_os
            && f.arch == go_arch
            && (f.kind == "archive" || f.kind.is_empty())
            && f.filename.ends_with(want_ext)
    })
}

/// Latest **stable** patch for each minor line (`1.<minor>`), newest lines first.
/// Used by the GUI as `list_remote_latest_per_major` (Go "major" in UI terms is `1.xx`).
pub fn list_latest_stable_per_minor_line(
    releases: &[GoRelease],
    os: &str,
    arch: &str,
) -> EnvrResult<Vec<RuntimeVersion>> {
    let mut best: HashMap<(u64, u64), (SemKey, String)> = HashMap::new();
    for r in releases {
        if !r.stable {
            continue;
        }
        if !go_release_has_installable_archive(r, os, arch) {
            continue;
        }
        let Some(k) = semver_key_from_go_label(&r.version) else {
            continue;
        };
        let line = (k.0, k.1);
        let label = normalize_go_version(&r.version);
        best.entry(line)
            .and_modify(|(mk, mv)| {
                if k > *mk {
                    *mk = k;
                    *mv = label.clone();
                }
            })
            .or_insert((k, label));
    }
    if best.is_empty() {
        return Err(EnvrError::Validation(
            "no stable go releases for this platform in index".into(),
        ));
    }
    let mut items: Vec<(SemKey, String)> = best.into_values().collect();
    items.sort_by(|a, b| b.0.cmp(&a.0));
    Ok(items.into_iter().map(|(_, s)| RuntimeVersion(s)).collect())
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

    #[test]
    fn latest_per_minor_line_picks_newest_patch_and_platform() {
        let rel = vec![
            GoRelease {
                version: "go1.22.4".into(),
                stable: true,
                files: vec![GoDistFile {
                    filename: "go1.22.4.linux-amd64.tar.gz".into(),
                    os: "linux".into(),
                    arch: "amd64".into(),
                    kind: "archive".into(),
                    sha256: String::new(),
                }],
            },
            GoRelease {
                version: "go1.22.6".into(),
                stable: true,
                files: vec![GoDistFile {
                    filename: "go1.22.6.linux-amd64.tar.gz".into(),
                    os: "linux".into(),
                    arch: "amd64".into(),
                    kind: "archive".into(),
                    sha256: String::new(),
                }],
            },
            GoRelease {
                version: "go1.23.0".into(),
                stable: true,
                files: vec![GoDistFile {
                    filename: "go1.23.0.linux-amd64.tar.gz".into(),
                    os: "linux".into(),
                    arch: "amd64".into(),
                    kind: "archive".into(),
                    sha256: String::new(),
                }],
            },
        ];
        let got = list_latest_stable_per_minor_line(&rel, "linux", "x86_64").expect("list");
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].0, "1.23.0");
        assert_eq!(got[1].0, "1.22.6");
    }
}
