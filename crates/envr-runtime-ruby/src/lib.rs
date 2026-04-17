mod index;
mod manager;

pub use index::{
    DEFAULT_RUBY_RELEASES_URL, RubyRelease, list_latest_patch_per_major, parse_ruby_releases,
    resolve_ruby_version,
};
pub use manager::{
    RubyManager, RubyPaths, list_installed_versions, read_current, ruby_installation_valid,
};

use envr_domain::runtime::{
    InstallRequest, RemoteFilter, ResolvedVersion, RuntimeKind, RuntimeProvider, RuntimeVersion,
    VersionSpec,
};
use envr_error::EnvrResult;
use envr_platform::paths::current_platform_paths;
use std::collections::BTreeSet;
use std::path::PathBuf;

pub struct RubyRuntimeProvider {
    releases_url: String,
    runtime_root_override: Option<PathBuf>,
}

impl RubyRuntimeProvider {
    pub fn new() -> Self {
        Self {
            releases_url: DEFAULT_RUBY_RELEASES_URL.to_string(),
            runtime_root_override: None,
        }
    }

    pub fn with_releases_url(mut self, url: impl Into<String>) -> Self {
        self.releases_url = url.into();
        self
    }

    pub fn with_runtime_root(mut self, root: PathBuf) -> Self {
        self.runtime_root_override = Some(root);
        self
    }

    fn runtime_root(&self) -> EnvrResult<PathBuf> {
        Ok(match &self.runtime_root_override {
            Some(p) => p.clone(),
            None => current_platform_paths()?.runtime_root,
        })
    }

    fn manager(&self) -> EnvrResult<RubyManager> {
        RubyManager::try_new(self.runtime_root()?, self.releases_url.clone())
    }

    fn remote_cache_ttl_secs() -> u64 {
        const DEFAULT: u64 = 24 * 60 * 60;
        std::env::var("ENVR_RUBY_REMOTE_CACHE_TTL_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT)
    }

    fn remote_latest_per_major_cache_file(&self) -> Option<PathBuf> {
        let root = self.runtime_root().ok()?;
        let paths = RubyPaths::new(root);
        Some(
            paths
                .cache_dir()
                .join("remote_latest_per_major_installer.json"),
        )
    }
}

impl Default for RubyRuntimeProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeProvider for RubyRuntimeProvider {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::Ruby
    }

    fn list_installed(&self) -> EnvrResult<Vec<RuntimeVersion>> {
        let paths = RubyPaths::new(self.runtime_root()?);
        list_installed_versions(&paths)
    }

    fn current(&self) -> EnvrResult<Option<RuntimeVersion>> {
        let paths = RubyPaths::new(self.runtime_root()?);
        read_current(&paths)
    }

    fn set_current(&self, version: &RuntimeVersion) -> EnvrResult<()> {
        self.manager()?.set_current(version)
    }

    fn list_remote(&self, filter: &RemoteFilter) -> EnvrResult<Vec<RuntimeVersion>> {
        #[cfg(windows)]
        {
            let releases = self.manager()?.load_installer_releases()?;
            return index::list_remote_versions(&releases, filter);
        }
        #[cfg(not(windows))]
        {
            let releases = self.manager()?.load_releases()?;
            index::list_remote_versions(&releases, filter)
        }
    }

    fn list_remote_majors(&self) -> EnvrResult<Vec<String>> {
        #[cfg(windows)]
        let releases = self.manager()?.load_installer_releases()?;
        #[cfg(not(windows))]
        let releases = self.manager()?.load_releases()?;
        let mut majors = BTreeSet::<String>::new();
        for release in releases {
            if let Some(major) = release.version.split('.').next()
                && !major.is_empty()
            {
                majors.insert(major.to_string());
            }
        }
        Ok(majors.into_iter().collect())
    }

    fn list_remote_latest_per_major(&self) -> EnvrResult<Vec<RuntimeVersion>> {
        let ttl_secs = Self::remote_cache_ttl_secs();
        if let Some(path) = self.remote_latest_per_major_cache_file()
            && let Some(list) =
                envr_platform::cache_recovery::read_json_string_list(&path, Some(ttl_secs), |xs| {
                    !xs.is_empty()
                })
        {
            return Ok(list.into_iter().map(RuntimeVersion).collect());
        }
        #[cfg(windows)]
        {
            let releases = self.manager()?.load_installer_releases()?;
            let list = index::list_latest_per_major_from_installer_releases(&releases)?;
            let _ = (|| -> EnvrResult<()> {
                let root = self.runtime_root()?;
                let paths = RubyPaths::new(root);
                std::fs::create_dir_all(paths.cache_dir())?;
                let strings: Vec<String> = list.iter().map(|v| v.0.clone()).collect();
                let s = serde_json::to_string(&strings)
                    .map_err(|e| envr_error::EnvrError::Validation(e.to_string()))?;
                let cache_file = paths
                    .cache_dir()
                    .join("remote_latest_per_major_installer.json");
                envr_platform::fs_atomic::write_atomic(&cache_file, s.as_bytes())?;
                Ok(())
            })();
            return Ok(list);
        }
        #[cfg(not(windows))]
        {
            let releases = self.manager()?.load_releases()?;
            let list = index::list_latest_patch_per_major(&releases)?;
            let _ = (|| -> EnvrResult<()> {
                let root = self.runtime_root()?;
                let paths = RubyPaths::new(root);
                std::fs::create_dir_all(paths.cache_dir())?;
                let strings: Vec<String> = list.iter().map(|v| v.0.clone()).collect();
                let s = serde_json::to_string(&strings)
                    .map_err(|e| envr_error::EnvrError::Validation(e.to_string()))?;
                let cache_file = paths
                    .cache_dir()
                    .join("remote_latest_per_major_installer.json");
                envr_platform::fs_atomic::write_atomic(&cache_file, s.as_bytes())?;
                Ok(())
            })();
            Ok(list)
        }
    }

    fn try_load_remote_latest_per_major_from_disk(&self) -> Vec<RuntimeVersion> {
        let Some(path) = self.remote_latest_per_major_cache_file() else {
            return Vec::new();
        };
        let Some(list) =
            envr_platform::cache_recovery::read_json_string_list(&path, None, |xs| !xs.is_empty())
        else {
            return Vec::new();
        };
        list.into_iter().map(RuntimeVersion).collect()
    }

    fn resolve(&self, spec: &VersionSpec) -> EnvrResult<ResolvedVersion> {
        let version = self.manager()?.resolve_spec(&spec.0)?;
        Ok(ResolvedVersion { version })
    }

    fn install(&self, request: &InstallRequest) -> EnvrResult<RuntimeVersion> {
        self.manager()?.install_from_spec(request)
    }

    fn uninstall(&self, version: &RuntimeVersion) -> EnvrResult<()> {
        self.manager()?.uninstall(version)
    }

    fn uninstall_dry_run_targets(
        &self,
        version: &RuntimeVersion,
    ) -> EnvrResult<(Vec<PathBuf>, Option<String>)> {
        let paths = RubyPaths::new(self.runtime_root()?);
        Ok((vec![paths.version_dir(&version.0)], None))
    }
}
