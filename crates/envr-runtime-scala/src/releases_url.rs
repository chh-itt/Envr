//! Resolve Scala 3 GitHub **releases REST API** URL (official, env override, optional proxy prefix).
//!
//! Mirrors the Crystal runtime `releases_url` pattern (`envr-runtime-crystal`):
//! public mirrors that wrap `api.github.com` often return **403** for the REST API, so we do not
//! prepend them by default. When the API is blocked or rate-limited, the index fetch falls back
//! to `github.com/.../releases.atom` plus synthetic `releases/download/...` asset URLs.

pub const DEFAULT_SCALA_RELEASES_API_URL: &str =
    "https://api.github.com/repos/scala/scala3/releases?per_page=100";

pub fn resolved_scala_releases_api_url() -> String {
    if let Ok(s) = std::env::var("ENVR_SCALA_GITHUB_RELEASES_URL") {
        let t = s.trim();
        if !t.is_empty() {
            return t.to_string();
        }
    }
    if let Ok(p) = std::env::var("ENVR_GITHUB_API_PROXY_PREFIX") {
        let p = p.trim().trim_end_matches('/');
        if !p.is_empty() {
            return format!("{}/{}", p, DEFAULT_SCALA_RELEASES_API_URL);
        }
    }
    DEFAULT_SCALA_RELEASES_API_URL.to_string()
}
