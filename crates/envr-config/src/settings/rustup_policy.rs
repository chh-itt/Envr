use super::{RustDownloadSource, Settings, runtime_sources::prefer_domestic_source};

/// Returns `RUSTUP_DIST_SERVER` when a non-official mirror is selected, otherwise `None`.
pub fn rustup_dist_server_from_settings(s: &Settings) -> Option<String> {
    if prefer_domestic_source(
        s,
        matches!(s.runtime.rust.download_source, RustDownloadSource::Domestic),
        matches!(s.runtime.rust.download_source, RustDownloadSource::Auto),
    ) {
        Some("https://mirrors.ustc.edu.cn/rust-static".to_string())
    } else {
        None
    }
}

/// Returns `RUSTUP_UPDATE_ROOT` when a non-official mirror is selected, otherwise `None`.
pub fn rustup_update_root_from_settings(s: &Settings) -> Option<String> {
    if prefer_domestic_source(
        s,
        matches!(s.runtime.rust.download_source, RustDownloadSource::Domestic),
        matches!(s.runtime.rust.download_source, RustDownloadSource::Auto),
    ) {
        Some("https://mirrors.ustc.edu.cn/rust-static/rustup".to_string())
    } else {
        None
    }
}
