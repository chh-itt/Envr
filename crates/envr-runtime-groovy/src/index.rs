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

static APACHE_GROOVY_VERSION_DIR_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"href=["']([0-9]+\.[0-9]+\.[0-9]+)/["']"#).expect("groovy version dir regex")
});

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroovyIndexRow {
    pub version: String,
    pub url: String,
}

pub fn blocking_http_client() -> EnvrResult<reqwest::blocking::Client> {
    build_blocking_http_client(
        concat!(
            "envr-runtime-groovy/",
            env!("CARGO_PKG_VERSION"),
            " (https://groovy-lang.org; envr)"
        ),
        Some(Duration::from_secs(120)),
    )
}

pub fn fetch_text(client: &reqwest::blocking::Client, url: &str) -> EnvrResult<String> {
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

fn cmp_semver_labels_desc(a: &str, b: &str) -> Ordering {
    match (numeric_version_segments(a), numeric_version_segments(b)) {
        (Some(va), Some(vb)) => vb.cmp(&va),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => b.cmp(a),
    }
}

pub fn parse_groovy_versions_from_index_html(html: &str) -> Vec<String> {
    let mut set = HashSet::new();
    let mut out = Vec::new();
    for cap in APACHE_GROOVY_VERSION_DIR_RE.captures_iter(html) {
        let Some(m) = cap.get(1) else {
            continue;
        };
        let v = m.as_str().trim();
        if set.insert(v.to_string()) {
            out.push(v.to_string());
        }
    }
    out.sort_by(|a, b| cmp_semver_labels_desc(a, b));
    out
}

fn ensure_slash(s: &str) -> String {
    let t = s.trim();
    if t.ends_with('/') {
        t.to_string()
    } else {
        format!("{t}/")
    }
}

pub fn binary_zip_url_for(base_index_url: &str, version: &str) -> String {
    format!(
        "{}{}/distribution/apache-groovy-binary-{}.zip",
        ensure_slash(base_index_url),
        version,
        version
    )
}

pub fn merge_rows(
    primary_base: &str,
    archive_base: &str,
    primary_html: &str,
    archive_html: &str,
) -> Vec<GroovyIndexRow> {
    let primary_versions = parse_groovy_versions_from_index_html(primary_html);
    let archive_versions = parse_groovy_versions_from_index_html(archive_html);
    let primary_set: HashSet<String> = primary_versions.iter().cloned().collect();
    let mut rows = Vec::new();
    for v in primary_versions {
        rows.push(GroovyIndexRow {
            url: binary_zip_url_for(primary_base, &v),
            version: v,
        });
    }
    for v in archive_versions {
        if primary_set.contains(&v) {
            continue;
        }
        rows.push(GroovyIndexRow {
            url: binary_zip_url_for(archive_base, &v),
            version: v,
        });
    }
    rows.sort_by(|a, b| cmp_semver_labels_desc(&a.version, &b.version));
    rows
}

pub fn list_remote_versions(rows: &[GroovyIndexRow], filter: &RemoteFilter) -> Vec<RuntimeVersion> {
    let mut labels: Vec<String> = rows.iter().map(|r| r.version.clone()).collect();
    if let Some(prefix) = filter.prefix.as_deref() {
        let p = prefix.trim();
        if !p.is_empty() {
            labels.retain(|v| v.starts_with(p));
        }
    }
    labels.into_iter().map(RuntimeVersion).collect()
}

pub fn list_remote_latest_per_major_lines(rows: &[GroovyIndexRow]) -> Vec<RuntimeVersion> {
    let mut seen = HashSet::<String>::new();
    let mut out = Vec::new();
    for r in rows {
        if let Some(line) = version_line_key_for_kind(RuntimeKind::Groovy, &r.version)
            && seen.insert(line)
        {
            out.push(RuntimeVersion(r.version.clone()));
        }
    }
    out
}

pub fn resolve_groovy_version(rows: &[GroovyIndexRow], spec: &str) -> EnvrResult<String> {
    let s = spec.trim().trim_start_matches('v').trim_start_matches('V');
    if s.is_empty() {
        return Err(EnvrError::Validation("empty groovy version spec".into()));
    }
    let labels: Vec<&str> = rows.iter().map(|r| r.version.as_str()).collect();
    if labels.contains(&s) {
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
        "no groovy release matches spec `{s}` (try full label like 4.0.31)"
    )))
}
