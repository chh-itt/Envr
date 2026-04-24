use std::{
    cell::RefCell, collections::HashMap, fs, path::PathBuf, sync::OnceLock, time::SystemTime,
};

use envr_error::EnvrResult;
use envr_platform::paths::EnvrPaths;

use super::{Settings, file_mtime};

thread_local! {
    static SETTINGS_FILE_CACHE: RefCell<HashMap<PathBuf, (Option<SystemTime>, Settings)>> =
        RefCell::new(HashMap::new());
}

thread_local! {
    static RESOLVE_RUNTIME_ROOT_CACHE: RefCell<Option<(PathBuf, Option<SystemTime>, PathBuf)>> =
        const { RefCell::new(None) };
}

static PROCESS_RUNTIME_ROOT_OVERRIDE: OnceLock<PathBuf> = OnceLock::new();

pub(super) fn settings_file_cache_get(
    path: &PathBuf,
    mtime: Option<SystemTime>,
) -> Option<Settings> {
    SETTINGS_FILE_CACHE.with(|c| {
        c.borrow()
            .get(path)
            .and_then(|(m2, s)| if m2 == &mtime { Some(s.clone()) } else { None })
    })
}

pub(super) fn settings_file_cache_insert(
    path: PathBuf,
    mtime: Option<SystemTime>,
    settings: Settings,
) {
    SETTINGS_FILE_CACHE.with(|c| {
        c.borrow_mut().insert(path, (mtime, settings));
    });
}

pub(super) fn settings_file_cache_remove(path: &PathBuf) {
    SETTINGS_FILE_CACHE.with(|c| {
        c.borrow_mut().remove(path);
    });
}

pub(super) fn runtime_root_cache_clear() {
    RESOLVE_RUNTIME_ROOT_CACHE.with(|c| *c.borrow_mut() = None);
}

/// Set a process-wide runtime root override (preferred over `ENVR_RUNTIME_ROOT` and `settings.toml`).
///
/// Intended for early startup configuration (CLI global `--runtime-root`) without mutating the
/// process environment.
///
/// Returns `true` when the override was set by this call; `false` when it was already set.
pub fn set_process_runtime_root_override(path: PathBuf) -> bool {
    let trimmed = path.to_string_lossy().trim().to_string();
    if trimmed.is_empty() {
        return false;
    }
    PROCESS_RUNTIME_ROOT_OVERRIDE
        .set(PathBuf::from(trimmed))
        .is_ok()
}

pub fn process_runtime_root_override() -> Option<&'static PathBuf> {
    PROCESS_RUNTIME_ROOT_OVERRIDE.get()
}

/// Clears in-process caches for [`Settings::load_or_default_from`] and [`resolve_runtime_root`].
pub fn reset_settings_load_caches() {
    SETTINGS_FILE_CACHE.with(|c| c.borrow_mut().clear());
    runtime_root_cache_clear();
}

pub struct SettingsCache {
    path: PathBuf,
    cached: Settings,
    last_modified: Option<SystemTime>,
}

impl SettingsCache {
    pub fn new(path: impl Into<PathBuf>) -> EnvrResult<Self> {
        let path = path.into();
        let cached = Settings::load_or_default_from(&path)?;
        let last_modified = file_mtime(&path).ok();
        Ok(Self {
            path,
            cached,
            last_modified,
        })
    }

    pub fn get(&mut self) -> EnvrResult<&Settings> {
        let mtime = file_mtime(&self.path).ok();
        if mtime != self.last_modified {
            self.cached = Settings::load_or_default_from(&self.path)?;
            self.last_modified = mtime;
        }
        Ok(&self.cached)
    }

    /// Reread `settings.toml` from disk even when mtime is unchanged (e.g. after external CLI edit in same second).
    pub fn reload(&mut self) -> EnvrResult<&Settings> {
        self.cached = Settings::load_or_default_from(&self.path)?;
        self.last_modified = file_mtime(&self.path).ok();
        Ok(&self.cached)
    }

    pub fn set_and_persist(&mut self, settings: Settings) -> EnvrResult<()> {
        settings.save_to(&self.path)?;
        self.cached = settings;
        self.last_modified = file_mtime(&self.path).ok();
        Ok(())
    }

    /// Replace cached settings without any disk I/O.
    ///
    /// Useful for GUI async flows where the settings were already loaded/saved
    /// off the UI thread.
    pub fn set_cached(&mut self, settings: Settings) {
        self.cached = settings;
        // Keep mtime tracking consistent so `get()` can stay in-memory unless disk changed.
        self.last_modified = file_mtime(&self.path).ok();
    }

    /// In-memory settings (last load / [`Self::set_cached`] / [`Self::reload`]).
    ///
    /// Prefer this over [`Self::get`] when syncing UI immediately after [`Self::set_cached`]:
    /// `get()` may re-read disk if mtime differs slightly; a failed parse would replace the cache
    /// with defaults and wipe fields like `paths.runtime_root`.
    pub fn snapshot(&self) -> &Settings {
        &self.cached
    }
}

pub fn settings_path_from_platform(paths: &EnvrPaths) -> PathBuf {
    paths.settings_file.clone()
}

/// Effective runtime data root: `ENVR_RUNTIME_ROOT` wins, then `settings.toml` `paths.runtime_root`,
/// then the platform default (`EnvrPaths::runtime_root`).
pub fn resolve_runtime_root() -> EnvrResult<PathBuf> {
    if let Some(p) = process_runtime_root_override() {
        return Ok(p.clone());
    }
    if let Ok(p) = std::env::var("ENVR_RUNTIME_ROOT") {
        let t = p.trim();
        if !t.is_empty() {
            return Ok(PathBuf::from(t));
        }
    }

    let platform = envr_platform::paths::current_platform_paths()?;
    let settings_path = settings_path_from_platform(&platform);
    let mtime = fs::metadata(&settings_path)
        .ok()
        .and_then(|m| m.modified().ok());

    if let Some(root) = RESOLVE_RUNTIME_ROOT_CACHE.with(|c| {
        c.borrow().as_ref().and_then(|(p, m2, root)| {
            if p == &settings_path && m2 == &mtime {
                Some(root.clone())
            } else {
                None
            }
        })
    }) {
        return Ok(root);
    }

    let settings = Settings::load_or_default_from(&settings_path)?;
    let root = if let Some(ref r) = settings.paths.runtime_root {
        let t = r.trim();
        if !t.is_empty() {
            PathBuf::from(t)
        } else {
            platform.runtime_root.clone()
        }
    } else {
        platform.runtime_root.clone()
    };

    RESOLVE_RUNTIME_ROOT_CACHE.with(|c| {
        *c.borrow_mut() = Some((settings_path, mtime, root.clone()));
    });
    Ok(root)
}
