use envr_config::env_context::load_settings_cached as load_settings_from_context;
use envr_config::settings::Settings;
use envr_error::{EnvrError, EnvrResult, ErrorCode};
use reqwest::Url;

use crate::registry::MirrorRegistry;
use crate::strategy::{ResolvedMirror, join_url, mirror_base_url, resolve_mirror};

/// Load settings via the process-level context cache.
pub fn load_settings_cached() -> EnvrResult<Settings> {
    load_settings_from_context()
}

/// Resolve a URL according to mirror settings.
///
/// - `offline` rejects network URL access.
/// - `official` returns `original` unchanged.
/// - non-official mirrors use generic proxy rewrite:
///   `<mirror_base>/<origin_host>/<origin_path>?<query>`.
pub fn maybe_mirror_url(settings: &Settings, original: &str) -> EnvrResult<String> {
    let reg = MirrorRegistry::with_presets()?;
    match resolve_mirror(settings, &reg)? {
        ResolvedMirror::Offline => Err(EnvrError::Download(format!(
            "mirror.mode=offline: refusing network request to {original}"
        ))),
        ResolvedMirror::Mirror(m) if m.is_official => Ok(original.to_string()),
        ResolvedMirror::Mirror(m) => {
            let u = Url::parse(original)
                .map_err(|e| EnvrError::with_source(ErrorCode::Validation, "invalid url", e))?;
            let base = mirror_base_url(&m)?;
            let host = u
                .host_str()
                .ok_or_else(|| EnvrError::Validation("url missing host".into()))?;
            let mut rel = format!("{host}{}", u.path());
            if let Some(q) = u.query() {
                rel.push('?');
                rel.push_str(q);
            }
            Ok(join_url(&base, &rel)?.to_string())
        }
    }
}

/// Convenience helper that loads settings from disk cache and resolves mirror URL.
pub fn maybe_mirror_url_from_disk(original: &str) -> EnvrResult<String> {
    let settings = load_settings_cached()?;
    maybe_mirror_url(&settings, original)
}
