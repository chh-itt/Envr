//! Resolve `extends = [...]` in `.envr.toml` by fetching remote layers (HTTPS) and merging.

use crate::project_config::ProjectConfig;
use envr_error::{EnvrError, EnvrResult, ErrorCode};
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    fs,
    io::Write,
    path::PathBuf,
    time::{Duration, SystemTime},
};

const MAX_EXTENDS_DEPTH: u32 = 16;
const FETCH_TIMEOUT: Duration = Duration::from_secs(45);
const CACHE_TTL: Duration = Duration::from_secs(3600);

/// Turn a user `extends` entry into a fetchable URL.
pub fn normalize_extend_url(raw: &str) -> EnvrResult<String> {
    let s = raw.trim();
    if s.is_empty() {
        return Err(EnvrError::Config("extends entry is empty".to_string()));
    }
    let lower = s.to_ascii_lowercase();
    if lower.starts_with("https://") || lower.starts_with("http://") {
        return Ok(s.to_string());
    }
    let t = s.trim_start_matches('/');
    if t.len() >= "github.com/".len()
        && t[.."github.com/".len()].eq_ignore_ascii_case("github.com/")
    {
        return github_shorthand_to_raw_url(s);
    }
    Ok(format!("https://{s}"))
}

fn github_shorthand_to_raw_url(s: &str) -> EnvrResult<String> {
    let t = s.trim().trim_start_matches('/');
    let prefix = "github.com/";
    let rest = if t.len() >= prefix.len() && t[..prefix.len()].eq_ignore_ascii_case(prefix) {
        &t[prefix.len()..]
    } else {
        return Err(EnvrError::Config(format!(
            "invalid GitHub extends shorthand (expected github.com/owner/repo/ref[/path]): {s}"
        )));
    };
    let parts: Vec<&str> = rest
        .split('/')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect();
    if parts.len() < 3 {
        return Err(EnvrError::Config(format!(
            "GitHub extends needs owner/repo/ref (and optional path): {s}"
        )));
    }
    let owner = parts[0];
    let repo = parts[1];
    let git_ref = parts[2];
    let path = if parts.len() == 3 {
        ".envr.toml".to_string()
    } else {
        parts[3..].join("/")
    };
    Ok(format!(
        "https://raw.githubusercontent.com/{owner}/{repo}/{git_ref}/{path}"
    ))
}

fn url_cache_path(url: &str) -> EnvrResult<PathBuf> {
    let root = crate::settings::resolve_runtime_root()?;
    let mut h = Sha256::new();
    h.update(url.as_bytes());
    let digest = h.finalize();
    let name: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    Ok(root
        .join("cache")
        .join("project-extends")
        .join(format!("{name}.toml")))
}

fn read_cache_if_fresh(path: &PathBuf) -> Option<String> {
    let meta = fs::metadata(path).ok()?;
    let modified = meta.modified().ok()?;
    let age = SystemTime::now().duration_since(modified).ok()?;
    if age > CACHE_TTL {
        return None;
    }
    fs::read_to_string(path).ok()
}

fn write_cache_atomic(path: &PathBuf, body: &str) -> EnvrResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(EnvrError::from)?;
    }
    let tmp = path.with_extension("tmp");
    {
        let mut f = fs::File::create(&tmp).map_err(EnvrError::from)?;
        f.write_all(body.as_bytes()).map_err(EnvrError::from)?;
    }
    fs::rename(&tmp, path).map_err(EnvrError::from)?;
    Ok(())
}

/// GET `url` (HTTPS/HTTP) with a short on-disk cache under the runtime root.
pub fn fetch_extend_body(url: &str) -> EnvrResult<String> {
    let lower = url.to_ascii_lowercase();
    if !(lower.starts_with("https://") || lower.starts_with("http://")) {
        return Err(EnvrError::Config(format!(
            "extends URL must use http or https scheme after normalization: {url}"
        )));
    }

    if let Ok(cache_path) = url_cache_path(url)
        && let Some(hit) = read_cache_if_fresh(&cache_path)
    {
        return Ok(hit);
    }

    let body = ureq::get(url)
        .timeout(FETCH_TIMEOUT)
        .call()
        .map_err(|e| {
            EnvrError::with_source(
                ErrorCode::Config,
                format!("extends fetch failed for {url}"),
                e,
            )
        })?
        .into_string()
        .map_err(|e| {
            EnvrError::with_source(
                ErrorCode::Config,
                format!("extends read body failed for {url}"),
                e,
            )
        })?;

    if let Ok(cache_path) = url_cache_path(url) {
        let _ = write_cache_atomic(&cache_path, &body);
    }

    Ok(body)
}

/// Resolve `extends` recursively; remote layers are merged first (first URL lowest precedence),
/// then `cfg` overlays the stack. Clears `cfg.extends` on success.
pub fn resolve_extends(cfg: ProjectConfig) -> EnvrResult<ProjectConfig> {
    let mut visited = HashSet::<String>::new();
    resolve_extends_inner(cfg, 0, &mut visited)
}

fn resolve_extends_inner(
    mut cfg: ProjectConfig,
    depth: u32,
    visited: &mut HashSet<String>,
) -> EnvrResult<ProjectConfig> {
    if depth > MAX_EXTENDS_DEPTH {
        return Err(EnvrError::Config(format!(
            "extends nesting deeper than {MAX_EXTENDS_DEPTH} levels"
        )));
    }

    let urls: Vec<String> = std::mem::take(&mut cfg.extends);
    if urls.is_empty() {
        return Ok(cfg);
    }

    let mut acc = ProjectConfig::default();
    for raw in urls {
        let norm = normalize_extend_url(&raw)?;
        if !visited.insert(norm.clone()) {
            return Err(EnvrError::Config(format!(
                "extends cycle or duplicate fetch: {norm}"
            )));
        }
        let body = fetch_extend_body(&norm)?;
        let mut remote =
            crate::project_config::parse_project_config_str(&body).map_err(|e| {
                EnvrError::with_source(
                    ErrorCode::Config,
                    format!("failed to parse extended config {norm}"),
                    e,
                )
            })?;
        remote = resolve_extends_inner(remote, depth + 1, visited)?;
        visited.remove(&norm);
        acc = remote.merge_over(acc);
    }

    Ok(cfg.merge_over(acc))
}

/// Test hook: same as [`resolve_extends`] but bodies come from `fetch` instead of the network.
pub fn resolve_extends_with_fetch<F>(cfg: ProjectConfig, fetch: &mut F) -> EnvrResult<ProjectConfig>
where
    F: FnMut(&str) -> EnvrResult<String>,
{
    let mut visited = HashSet::<String>::new();
    resolve_extends_inner_with_fetch(cfg, 0, &mut visited, fetch)
}

fn resolve_extends_inner_with_fetch<F>(
    mut cfg: ProjectConfig,
    depth: u32,
    visited: &mut HashSet<String>,
    fetch: &mut F,
) -> EnvrResult<ProjectConfig>
where
    F: FnMut(&str) -> EnvrResult<String>,
{
    if depth > MAX_EXTENDS_DEPTH {
        return Err(EnvrError::Config(format!(
            "extends nesting deeper than {MAX_EXTENDS_DEPTH} levels"
        )));
    }

    let urls: Vec<String> = std::mem::take(&mut cfg.extends);
    if urls.is_empty() {
        return Ok(cfg);
    }

    let mut acc = ProjectConfig::default();
    for raw in urls {
        let norm = normalize_extend_url(&raw)?;
        if !visited.insert(norm.clone()) {
            return Err(EnvrError::Config(format!(
                "extends cycle or duplicate fetch: {norm}"
            )));
        }
        let body = fetch(&norm)?;
        let mut remote =
            crate::project_config::parse_project_config_str(&body).map_err(|e| {
                EnvrError::with_source(
                    ErrorCode::Config,
                    format!("failed to parse extended config {norm}"),
                    e,
                )
            })?;
        remote = resolve_extends_inner_with_fetch(remote, depth + 1, visited, fetch)?;
        visited.remove(&norm);
        acc = remote.merge_over(acc);
    }

    Ok(cfg.merge_over(acc))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_github_shorthand() {
        assert_eq!(
            normalize_extend_url("github.com/acme/shared/main").expect("ok"),
            "https://raw.githubusercontent.com/acme/shared/main/.envr.toml"
        );
        assert_eq!(
            normalize_extend_url("github.com/acme/shared/main/ci/base.toml").expect("ok"),
            "https://raw.githubusercontent.com/acme/shared/main/ci/base.toml"
        );
    }

    #[test]
    fn resolve_extends_mock_fetch() {
        let mut top = ProjectConfig::default();
        top.extends.push("https://example.com/base.toml".into());
        top.env.insert("B".into(), "local".into());

        let mut fetch = |url: &str| -> EnvrResult<String> {
            assert_eq!(url, "https://example.com/base.toml");
            Ok(r#"
[env]
A = "from-remote"
"#
            .to_string())
        };
        let got = resolve_extends_with_fetch(top, &mut fetch).expect("resolve");
        assert_eq!(got.env.get("A").map(String::as_str), Some("from-remote"));
        assert_eq!(got.env.get("B").map(String::as_str), Some("local"));
    }
}
