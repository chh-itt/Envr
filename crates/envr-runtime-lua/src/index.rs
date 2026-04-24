//! LuaBinaries: HTML index (`download.html`) + deterministic SourceForge artifact URLs per host.

use envr_domain::runtime::{
    RemoteFilter, RuntimeKind, RuntimeVersion, numeric_version_segments, version_line_key_for_kind,
};
use envr_download::blocking::build_blocking_http_client;
use envr_error::{EnvrError, EnvrResult, ErrorCode};
use regex::Regex;
use std::cmp::Ordering;
use std::collections::HashSet;
use std::sync::LazyLock;
use std::time::Duration;

pub const DEFAULT_LUA_DOWNLOAD_PAGE_URL: &str = "https://luabinaries.sourceforge.net/download.html";

static LUA_WIN64_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"lua-(\d+)\.(\d+)\.(\d+)_Win64_bin\.zip").expect("lua win64 regex")
});

pub fn blocking_http_client() -> EnvrResult<reqwest::blocking::Client> {
    build_blocking_http_client(
        concat!("envr-runtime-lua/", env!("CARGO_PKG_VERSION")),
        Some(Duration::from_secs(120)),
    )
}

pub fn fetch_download_page(client: &reqwest::blocking::Client, url: &str) -> EnvrResult<String> {
    let response = client.get(url).send().map_err(|e| {
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

/// Parse installable semver labels from the LuaBinaries download page body.
pub fn parse_installable_versions(html: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    for cap in LUA_WIN64_RE.captures_iter(html) {
        let major: u32 = cap
            .get(1)
            .and_then(|m| m.as_str().parse().ok())
            .unwrap_or(0);
        let minor: u32 = cap
            .get(2)
            .and_then(|m| m.as_str().parse().ok())
            .unwrap_or(0);
        let patch: u32 = cap
            .get(3)
            .and_then(|m| m.as_str().parse().ok())
            .unwrap_or(0);
        // Skip legacy 5.1 naming (lua5_1_5_...) — only `lua-X.Y.Z_*` Win64 rows.
        if major < 5 || (major == 5 && minor < 2) {
            continue;
        }
        let label = format!("{major}.{minor}.{patch}");
        seen.insert(label);
    }
    let mut out: Vec<String> = seen.into_iter().collect();
    out.sort_by(|a, b| cmp_semver_desc(b, a));
    out
}

fn cmp_semver_desc(a: &str, b: &str) -> Ordering {
    match (numeric_version_segments(a), numeric_version_segments(b)) {
        (Some(va), Some(vb)) => va.cmp(&vb),
        (Some(_), None) => Ordering::Greater,
        (None, Some(_)) => Ordering::Less,
        (None, None) => a.cmp(b),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LuaHostKind {
    WindowsX64,
    LinuxX64Glibc,
    MacOsX64,
}

pub fn lua_host_kind() -> EnvrResult<LuaHostKind> {
    use std::env::consts::{ARCH, OS};
    match (OS, ARCH) {
        ("windows", "x86_64") => Ok(LuaHostKind::WindowsX64),
        ("linux", "x86_64") => Ok(LuaHostKind::LinuxX64Glibc),
        ("macos", "x86_64") => Ok(LuaHostKind::MacOsX64),
        (os, arch) => Err(EnvrError::Validation(format!(
            "envr-managed Lua (LuaBinaries) is not wired for host {os}-{arch} yet; see docs/runtime/lua-integration-plan.md"
        ))),
    }
}

fn linux_tools_middle_label(version: &str) -> &'static str {
    let Some(parts) = numeric_version_segments(version) else {
        return "Linux319_64";
    };
    let major = parts.first().copied().unwrap_or(0);
    let minor = parts.get(1).copied().unwrap_or(0);
    if major > 5 || (major == 5 && minor >= 4) {
        "Linux54_64"
    } else {
        "Linux319_64"
    }
}

/// File name inside `Tools Executables/` on SourceForge.
pub fn tools_executable_filename(version: &str, host: LuaHostKind) -> EnvrResult<String> {
    let v = version.trim();
    if numeric_version_segments(v).is_none() {
        return Err(EnvrError::Validation(format!(
            "invalid lua version label: {v}"
        )));
    }
    Ok(match host {
        LuaHostKind::WindowsX64 => format!("lua-{v}_Win64_bin.zip"),
        LuaHostKind::LinuxX64Glibc => format!("lua-{v}_{}_bin.tar.gz", linux_tools_middle_label(v)),
        LuaHostKind::MacOsX64 => format!("lua-{v}_MacOS1011_bin.tar.gz"),
    })
}

pub fn sourceforge_tools_download_url(version: &str, host: LuaHostKind) -> EnvrResult<String> {
    let v = version.trim();
    let file = tools_executable_filename(v, host)?;
    Ok(format!(
        "https://downloads.sourceforge.net/project/luabinaries/{v}/Tools%20Executables/{file}"
    ))
}

pub fn versions_for_filter(versions: &[String], filter: &RemoteFilter) -> Vec<String> {
    let mut keys: Vec<String> = versions.to_vec();
    if let Some(prefix) = filter.prefix.as_deref() {
        let p = prefix.trim();
        if !p.is_empty() {
            keys.retain(|k| k.starts_with(p));
        }
    }
    keys
}

pub fn list_remote_versions(
    versions: &[String],
    filter: &RemoteFilter,
) -> EnvrResult<Vec<RuntimeVersion>> {
    Ok(versions_for_filter(versions, filter)
        .into_iter()
        .map(RuntimeVersion)
        .collect())
}

pub fn list_remote_latest_per_major_lines(versions: &[String]) -> Vec<RuntimeVersion> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut out = Vec::new();
    for k in versions {
        if let Some(line) = version_line_key_for_kind(RuntimeKind::Lua, k) {
            if seen.insert(line) {
                out.push(RuntimeVersion(k.clone()));
            }
        }
    }
    out
}

pub fn resolve_lua_version(versions: &[String], spec: &str) -> EnvrResult<String> {
    let s = spec.trim().trim_start_matches('v').trim_start_matches('V');
    if s.is_empty() || s.eq_ignore_ascii_case("latest") {
        return versions
            .first()
            .cloned()
            .ok_or_else(|| EnvrError::Validation("no remote lua versions available".into()));
    }
    if versions.iter().any(|k| k == s) {
        return Ok(s.to_string());
    }
    if let Some(parts) = numeric_version_segments(s) {
        match parts.len() {
            1 => {
                let major = parts[0];
                let best = versions
                    .iter()
                    .filter(|k| {
                        numeric_version_segments(k).is_some_and(|p| !p.is_empty() && p[0] == major)
                    })
                    .max_by(|a, b| cmp_semver_desc(a, b))
                    .map(|x| x.as_str());
                return best
                    .ok_or_else(|| {
                        EnvrError::Validation(format!(
                            "no lua release matches major `{s}` in the LuaBinaries index"
                        ))
                    })
                    .map(|x| x.to_string());
            }
            2 => {
                let line = format!("{}.{}", parts[0], parts[1]);
                let best = versions
                    .iter()
                    .filter(|k| {
                        version_line_key_for_kind(RuntimeKind::Lua, k).as_deref()
                            == Some(line.as_str())
                    })
                    .max_by(|a, b| cmp_semver_desc(a, b))
                    .map(|x| x.as_str());
                return best
                    .ok_or_else(|| {
                        EnvrError::Validation(format!(
                            "no lua release matches line `{line}` in the LuaBinaries index"
                        ))
                    })
                    .map(|x| x.to_string());
            }
            _ => {
                let best = versions
                    .iter()
                    .filter(|k| {
                        numeric_version_segments(k).is_some_and(|p| {
                            p.len() >= 3 && p[0] == parts[0] && p[1] == parts[1] && p[2] == parts[2]
                        })
                    })
                    .max_by(|a, b| cmp_semver_desc(a, b))
                    .map(|x| x.as_str());
                return best
                    .ok_or_else(|| {
                        EnvrError::Validation(format!(
                            "no lua release matches exact `{s}` in the LuaBinaries index"
                        ))
                    })
                    .map(|x| x.to_string());
            }
        }
    }
    Err(EnvrError::Validation(format!(
        "could not resolve lua version spec `{spec}`"
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_installable_versions_finds_semvers() {
        let html = r#"<a href="https://sourceforge.net/projects/luabinaries/files/5.4.8/Tools%20Executables/lua-5.4.8_Win64_bin.zip/download">win</a>
        <a href=".../lua-5.3.6_Win64_bin.zip">x</a>"#;
        let v = parse_installable_versions(html);
        assert!(v.contains(&"5.4.8".to_string()));
        assert!(v.contains(&"5.3.6".to_string()));
    }

    #[test]
    fn linux_tools_middle_label_rules() {
        assert_eq!(linux_tools_middle_label("5.3.6"), "Linux319_64");
        assert_eq!(linux_tools_middle_label("5.4.8"), "Linux54_64");
        assert_eq!(linux_tools_middle_label("5.5.0"), "Linux54_64");
    }

    #[test]
    fn resolve_exact_and_line() {
        let v = vec!["5.4.8".into(), "5.4.7".into(), "5.3.6".into()];
        assert_eq!(resolve_lua_version(&v, "5.4.7").expect("ex"), "5.4.7");
        assert_eq!(resolve_lua_version(&v, "5.4").expect("line"), "5.4.8");
        assert_eq!(resolve_lua_version(&v, "5").expect("maj"), "5.4.8");
    }
}
