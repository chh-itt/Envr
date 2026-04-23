use envr_domain::runtime::{
    RemoteFilter, RuntimeKind, RuntimeVersion, numeric_version_segments, version_line_key_for_kind,
};
use envr_download::blocking::build_blocking_http_client;
use envr_error::{EnvrError, EnvrResult, ErrorCode};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::HashSet;
use std::sync::LazyLock;
use std::time::Duration;

pub const DEFAULT_TERRAFORM_INDEX_URL: &str = "https://releases.hashicorp.com/terraform/";

static TERRAFORM_VERSION_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"/terraform/([0-9]+\.[0-9]+\.[0-9]+)/"#).expect("terraform version regex")
});

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerraformIndexRow {
    pub version: String,
}

pub fn blocking_http_client() -> EnvrResult<reqwest::blocking::Client> {
    build_blocking_http_client(
        concat!("envr-runtime-terraform/", env!("CARGO_PKG_VERSION")),
        Some(Duration::from_secs(120)),
    )
}

pub fn fetch_index_text(client: &reqwest::blocking::Client, url: &str) -> EnvrResult<String> {
    let response = client
        .get(url)
        .send()
        .map_err(|e| EnvrError::with_source(ErrorCode::Download, format!("request failed for {url}"), e))?;
    if !response.status().is_success() {
        return Err(EnvrError::Download(format!(
            "GET {url} -> {}",
            response.status()
        )));
    }
    response
        .text()
        .map_err(|e| EnvrError::with_source(ErrorCode::Download, format!("read body failed for {url}"), e))
}

fn cmp_semver_desc(a: &str, b: &str) -> Ordering {
    match (numeric_version_segments(a), numeric_version_segments(b)) {
        (Some(va), Some(vb)) => vb.cmp(&va),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => b.cmp(a),
    }
}

pub fn parse_versions_from_index_html(html: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for cap in TERRAFORM_VERSION_RE.captures_iter(html) {
        let Some(m) = cap.get(1) else {
            continue;
        };
        let version = m.as_str().to_string();
        if seen.insert(version.clone()) {
            out.push(version);
        }
    }
    out.sort_by(|a, b| cmp_semver_desc(a, b));
    out
}

pub fn list_remote_versions(
    rows: &[TerraformIndexRow],
    filter: &RemoteFilter,
) -> Vec<RuntimeVersion> {
    let mut labels: Vec<String> = rows.iter().map(|r| r.version.clone()).collect();
    if let Some(prefix) = filter.prefix.as_deref() {
        let p = prefix.trim();
        if !p.is_empty() {
            labels.retain(|v| v.starts_with(p));
        }
    }
    labels.into_iter().map(RuntimeVersion).collect()
}

pub fn list_remote_latest_per_major_lines(rows: &[TerraformIndexRow]) -> Vec<RuntimeVersion> {
    let mut seen = HashSet::<String>::new();
    let mut out = Vec::new();
    for r in rows {
        if let Some(line) = version_line_key_for_kind(RuntimeKind::Terraform, &r.version)
            && seen.insert(line)
        {
            out.push(RuntimeVersion(r.version.clone()));
        }
    }
    out
}

pub fn resolve_terraform_version(rows: &[TerraformIndexRow], spec: &str) -> EnvrResult<String> {
    let s = spec.trim().trim_start_matches('v').trim_start_matches('V');
    if s.is_empty() {
        return Err(EnvrError::Validation("empty terraform version spec".into()));
    }
    let labels: Vec<&str> = rows.iter().map(|r| r.version.as_str()).collect();
    if labels.iter().any(|x| *x == s) {
        return Ok(s.to_string());
    }
    if let Some(parts) = numeric_version_segments(s) {
        match parts.len() {
            1 => {
                let major = parts[0];
                if let Some(best) = rows.iter().map(|r| r.version.as_str()).find(|v| {
                    numeric_version_segments(v).is_some_and(|p| !p.is_empty() && p[0] == major)
                }) {
                    return Ok(best.to_string());
                }
            }
            2 => {
                let major = parts[0];
                let minor = parts[1];
                if let Some(best) = rows.iter().map(|r| r.version.as_str()).find(|v| {
                    numeric_version_segments(v)
                        .is_some_and(|p| p.len() >= 2 && p[0] == major && p[1] == minor)
                }) {
                    return Ok(best.to_string());
                }
            }
            _ => {}
        }
    }
    Err(EnvrError::Validation(format!(
        "no terraform release matches spec `{s}`"
    )))
}

pub fn terraform_platform_tuple() -> EnvrResult<&'static str> {
    use std::env::consts::{ARCH, OS};
    match (OS, ARCH) {
        ("windows", "x86_64") => Ok("windows_amd64"),
        ("windows", "aarch64") => Ok("windows_arm64"),
        ("linux", "x86_64") => Ok("linux_amd64"),
        ("linux", "aarch64") => Ok("linux_arm64"),
        ("macos", "x86_64") => Ok("darwin_amd64"),
        ("macos", "aarch64") => Ok("darwin_arm64"),
        _ => Err(EnvrError::Validation(format!(
            "no official Terraform build mapped for host {OS}-{ARCH}"
        ))),
    }
}

pub fn artifact_url(index_url: &str, version: &str, platform_tuple: &str) -> String {
    let base = index_url.trim().trim_end_matches('/');
    format!("{base}/{version}/terraform_{version}_{platform_tuple}.zip")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_versions_from_hashicorp_links() {
        let html = r#"
        <a href="https://releases.hashicorp.com/terraform/1.14.8/">terraform_1.14.8</a>
        <a href="https://releases.hashicorp.com/terraform/1.15.0-rc1/">terraform_1.15.0-rc1</a>
        <a href="/terraform/1.13.5/">terraform_1.13.5</a>
        "#;
        let out = parse_versions_from_index_html(html);
        assert_eq!(out, vec!["1.14.8".to_string(), "1.13.5".to_string()]);
    }
}
