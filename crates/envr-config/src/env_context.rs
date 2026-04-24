use crate::settings::{Settings, settings_path_from_platform};
use envr_error::EnvrResult;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
struct CachedSettings {
    loaded_at: Instant,
    settings: Settings,
}

static SETTINGS_CACHE: OnceLock<Mutex<Option<CachedSettings>>> = OnceLock::new();

fn settings_cache_ttl() -> Duration {
    const DEFAULT_SECS: u64 = 5;
    std::env::var("ENVR_CONTEXT_SETTINGS_CACHE_TTL_SECS")
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

/// Load settings through process-level context cache.
pub fn load_settings_cached() -> EnvrResult<Settings> {
    let ttl = settings_cache_ttl();
    let cell = SETTINGS_CACHE.get_or_init(|| Mutex::new(None));
    let mut guard = cell.lock().expect("env context settings cache mutex");
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

pub fn clear_settings_cache() {
    if let Some(cell) = SETTINGS_CACHE.get()
        && let Ok(mut guard) = cell.lock()
    {
        *guard = None;
    }
}

/// Effective runtime data root for this process.
///
/// Delegates to [`crate::settings::resolve_runtime_root`]: process override (CLI `--runtime-root`),
/// then `ENVR_RUNTIME_ROOT`, then `settings.toml` `paths.runtime_root`, then the platform default.
pub fn runtime_root() -> EnvrResult<PathBuf> {
    crate::settings::resolve_runtime_root()
}
