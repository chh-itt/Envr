pub const GROOVY_APACHE_PRIMARY_INDEX_URL: &str = "https://dlcdn.apache.org/groovy/";
pub const GROOVY_APACHE_ARCHIVE_INDEX_URL: &str = "https://archive.apache.org/dist/groovy/";

pub fn resolved_groovy_primary_index_url() -> String {
    std::env::var("ENVR_GROOVY_PRIMARY_INDEX_URL")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| GROOVY_APACHE_PRIMARY_INDEX_URL.to_string())
}

pub fn resolved_groovy_archive_index_url() -> String {
    std::env::var("ENVR_GROOVY_ARCHIVE_INDEX_URL")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| GROOVY_APACHE_ARCHIVE_INDEX_URL.to_string())
}
