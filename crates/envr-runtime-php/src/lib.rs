mod index;
mod manager;

pub use index::{
    PhpReleasesIndex, ReleaseLine, blocking_http_client, fetch_php_windows_releases_json,
    list_latest_stable_per_minor_line_for_build, list_remote_versions, parse_php_windows_index,
    resolve_php_version,
};
pub use manager::{
    PhpManager, PhpPaths, list_installed_versions, php_installation_valid, read_current,
    read_current_global_want_ts, resolve_global_php_current_target,
};

use envr_config::settings::{
    PhpWindowsBuildFlavor, Settings, php_windows_releases_json_url, settings_path_from_platform,
};
use envr_domain::runtime::{
    InstallRequest, RemoteFilter, ResolvedVersion, RuntimeKind, RuntimeProvider, RuntimeVersion,
    VersionSpec,
};
use envr_error::{EnvrError, EnvrResult};
use envr_platform::paths::current_platform_paths;
use std::path::{Path, PathBuf};

pub struct PhpRuntimeProvider {
    runtime_root_override: Option<std::path::PathBuf>,
}

impl PhpRuntimeProvider {
    pub fn new() -> Self {
        Self {
            runtime_root_override: None,
        }
    }

    pub fn with_runtime_root(mut self, root: std::path::PathBuf) -> Self {
        self.runtime_root_override = Some(root);
        self
    }

    fn runtime_root(&self) -> EnvrResult<std::path::PathBuf> {
        Ok(match &self.runtime_root_override {
            Some(p) => p.clone(),
            None => current_platform_paths()?.runtime_root,
        })
    }

    fn manager(&self) -> EnvrResult<PhpManager> {
        let (json_url, want_ts) = self.resolved_remote_settings()?;
        PhpManager::try_new(self.runtime_root()?, json_url, want_ts)
    }

    fn load_index(&self) -> EnvrResult<PhpReleasesIndex> {
        let client = blocking_http_client()?;
        let (json_url, _want_ts) = self.resolved_remote_settings()?;
        let body = fetch_php_windows_releases_json(&client, &json_url)?;
        parse_php_windows_index(&body)
    }

    fn resolved_remote_settings(&self) -> EnvrResult<(String, bool)> {
        let platform = current_platform_paths()?;
        let path = settings_path_from_platform(&platform);
        let s = Settings::load_or_default_from(&path).unwrap_or_default();
        let url = php_windows_releases_json_url(&s).to_string();
        let want_ts = matches!(s.runtime.php.windows_build, PhpWindowsBuildFlavor::Ts);
        Ok((url, want_ts))
    }

    fn remote_cache_ttl_secs() -> u64 {
        const DEFAULT: u64 = 24 * 60 * 60;
        std::env::var("ENVR_PHP_REMOTE_CACHE_TTL_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT)
    }

    fn file_is_within_ttl(path: &Path, ttl_secs: u64) -> bool {
        if ttl_secs == 0 {
            return false;
        }
        let Ok(meta) = std::fs::metadata(path) else {
            return false;
        };
        let Ok(mtime) = meta.modified() else {
            return false;
        };
        let Ok(age) = std::time::SystemTime::now().duration_since(mtime) else {
            return false;
        };
        age.as_secs() <= ttl_secs
    }

    fn remote_latest_per_major_cache_path(&self) -> EnvrResult<PathBuf> {
        let paths = PhpPaths::new(self.runtime_root()?);
        let os = std::env::consts::OS;
        let arch = std::env::consts::ARCH;
        let (_, want_ts) = self.resolved_remote_settings()?;
        let flavor = if want_ts { "ts" } else { "nts" };
        Ok(paths
            .cache_dir()
            .join(format!("remote_latest_per_major_{os}_{arch}_{flavor}.json")))
    }
}

impl Default for PhpRuntimeProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeProvider for PhpRuntimeProvider {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::Php
    }

    fn list_installed(&self) -> EnvrResult<Vec<RuntimeVersion>> {
        let paths = PhpPaths::new(self.runtime_root()?);
        let (_, want_ts) = self.resolved_remote_settings()?;
        list_installed_versions(&paths, want_ts)
    }

    fn current(&self) -> EnvrResult<Option<RuntimeVersion>> {
        let paths = PhpPaths::new(self.runtime_root()?);
        read_current(&paths)
    }

    fn set_current(&self, version: &RuntimeVersion) -> EnvrResult<()> {
        self.manager()?.set_current(version)
    }

    fn list_remote(&self, filter: &RemoteFilter) -> EnvrResult<Vec<RuntimeVersion>> {
        if !cfg!(windows) {
            return Err(EnvrError::Platform(
                "php remote listing is currently supported on Windows only".into(),
            ));
        }
        let idx = self.load_index()?;
        list_remote_versions(&idx, filter)
    }

    fn try_load_remote_latest_per_major_from_disk(&self) -> Vec<RuntimeVersion> {
        let Ok(path) = self.remote_latest_per_major_cache_path() else {
            return Vec::new();
        };
        let Ok(s) = std::fs::read_to_string(&path) else {
            return Vec::new();
        };
        let Ok(list) = serde_json::from_str::<Vec<String>>(&s) else {
            return Vec::new();
        };
        list.into_iter().map(RuntimeVersion).collect()
    }

    fn list_remote_latest_per_major(&self) -> EnvrResult<Vec<RuntimeVersion>> {
        let ttl_secs = Self::remote_cache_ttl_secs();
        let cache_file = self.remote_latest_per_major_cache_path()?;
        if Self::file_is_within_ttl(&cache_file, ttl_secs)
            && let Ok(s) = std::fs::read_to_string(&cache_file)
            && let Ok(list) = serde_json::from_str::<Vec<String>>(&s)
            && !list.is_empty()
        {
            return Ok(list.into_iter().map(RuntimeVersion).collect());
        }

        let idx = self.load_index()?;
        let (_, want_ts) = self.resolved_remote_settings()?;
        let arch = std::env::consts::ARCH;
        let list = list_latest_stable_per_minor_line_for_build(&idx, want_ts, arch)?;
        let _ = (|| -> EnvrResult<()> {
            let paths = PhpPaths::new(self.runtime_root()?);
            std::fs::create_dir_all(paths.cache_dir())?;
            let strings: Vec<String> = list.iter().map(|v| v.0.clone()).collect();
            let s = serde_json::to_string(&strings)
                .map_err(|e| envr_error::EnvrError::Validation(e.to_string()))?;
            std::fs::write(&cache_file, s)?;
            Ok(())
        })();
        Ok(list)
    }

    fn resolve(&self, spec: &VersionSpec) -> EnvrResult<ResolvedVersion> {
        if !cfg!(windows) {
            return Err(EnvrError::Platform(
                "php resolve is currently supported on Windows only".into(),
            ));
        }
        let idx = self.load_index()?;
        let v = resolve_php_version(&idx, &spec.0)?;
        Ok(ResolvedVersion {
            version: RuntimeVersion(v),
        })
    }

    fn install(&self, request: &InstallRequest) -> EnvrResult<RuntimeVersion> {
        self.manager()?.install_from_spec(request)
    }

    fn uninstall(&self, version: &RuntimeVersion) -> EnvrResult<()> {
        self.manager()?.uninstall(version)
    }
}
