//! Resolve GitHub releases **REST API** base URL (official, env override, optional proxy prefix).
//!
//! We intentionally **do not** prepend public mirrors like `ghproxy.net` to `api.github.com` by default:
//! those proxies often return **403** for the REST API. When the API is blocked or rate-limited,
//! `fetch_all_rows_for_host` falls back to `github.com/.../releases.atom` plus synthetic download URLs.

pub fn resolved_crystal_releases_api_url(default_api: &str) -> String {
    if let Ok(s) = std::env::var("ENVR_CRYSTAL_GITHUB_RELEASES_URL") {
        let t = s.trim();
        if !t.is_empty() {
            return t.to_string();
        }
    }
    if let Ok(p) = std::env::var("ENVR_GITHUB_API_PROXY_PREFIX") {
        let p = p.trim().trim_end_matches('/');
        if !p.is_empty() {
            return format!("{p}/{default_api}");
        }
    }
    default_api.to_string()
}
