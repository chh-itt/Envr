//! Resolve Clojure tools GitHub releases API URL.

pub const DEFAULT_CLOJURE_RELEASES_API_URL: &str =
    "https://api.github.com/repos/clojure/brew-install/releases?per_page=100";

pub fn resolved_clojure_releases_api_url() -> String {
    if let Ok(s) = std::env::var("ENVR_CLOJURE_GITHUB_RELEASES_URL") {
        let t = s.trim();
        if !t.is_empty() {
            return t.to_string();
        }
    }
    if let Ok(p) = std::env::var("ENVR_GITHUB_API_PROXY_PREFIX") {
        let p = p.trim().trim_end_matches('/');
        if !p.is_empty() {
            return format!("{}/{}", p, DEFAULT_CLOJURE_RELEASES_API_URL);
        }
    }
    DEFAULT_CLOJURE_RELEASES_API_URL.to_string()
}
