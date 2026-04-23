use envr_config::settings::{Settings, settings_path_from_platform};
use envr_error::{EnvrError, EnvrResult, ErrorCode};
use reqwest::Url;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use crate::registry::MirrorRegistry;
use crate::strategy::{ResolvedMirror, join_url, mirror_base_url, resolve_mirror};

#[derive(Debug, Clone)]
struct CachedSettings {
    loaded_at: Instant,
    settings: Settings,
}

static SETTINGS_CACHE: OnceLock<Mutex<Option<CachedSettings>>> = OnceLock::new();

fn settings_cache_ttl() -> Duration {
    const DEFAULT_SECS: u64 = 5;
    std::env::var("ENVR_MIRROR_SETTINGS_CACHE_TTL_SECS")
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok())
        .filter(|v| *v > 0)
        .map(Duration::from_secs)
        .unwrap_or(Duration::from_secs(DEFAULT_SECS))
}

fn load_settings_uncached() -> EnvrResult<Settings> {
    let platform = envr_platform::paths::current_platform_paths()?;
    let path = settings_path_from_platform(&platform);
    Settings::load_or_default_from(&path)
}

/// Load settings with a small in-process cache to avoid repeated disk IO in hot paths.
pub fn load_settings_cached() -> EnvrResult<Settings> {
    let ttl = settings_cache_ttl();
    let cell = SETTINGS_CACHE.get_or_init(|| Mutex::new(None));
    let mut guard = cell.lock().expect("mirror settings cache mutex");
    if let Some(cached) = guard.as_ref()
        && cached.loaded_at.elapsed() <= ttl
    {
        return Ok(cached.settings.clone());
    }
    let settings = load_settings_uncached()?;
    *guard = Some(CachedSettings {
        loaded_at: Instant::now(),
        settings: settings.clone(),
    });
    Ok(settings)
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
