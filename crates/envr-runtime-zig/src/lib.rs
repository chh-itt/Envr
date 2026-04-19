//! Zig toolchain runtime: official `index.json`, per-host tarball/zip, `versions/<label>/` layout.

mod index;
mod manager;

pub use index::{
    DEFAULT_ZIG_INDEX_URL, artifact_for_platform, blocking_http_client, fetch_index_json,
    find_version_entry, is_stable_release_top_key, list_remote_latest_per_major_lines,
    list_remote_versions, list_stable_versions_with_platform, parse_index_root, resolve_zig_version,
    zig_json_platform_key,
};
pub use envr_platform::bin_tool_layout::zig_installation_valid;
pub use manager::{
    ZigManager, ZigPaths, list_installed_versions, promote_single_root_dir, read_current,
};

use envr_domain::runtime::{
    InstallRequest, RemoteFilter, ResolvedVersion, RuntimeKind, RuntimeProvider, RuntimeVersion,
    VersionSpec,
};
use envr_error::EnvrResult;
use envr_platform::paths::current_platform_paths;
use std::path::PathBuf;

pub struct ZigRuntimeProvider {
    index_url: String,
    runtime_root_override: Option<PathBuf>,
}

impl ZigRuntimeProvider {
    pub fn new() -> Self {
        Self {
            index_url: DEFAULT_ZIG_INDEX_URL.to_string(),
            runtime_root_override: None,
        }
    }

    pub fn with_index_url(mut self, url: impl Into<String>) -> Self {
        self.index_url = url.into();
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

    fn manager(&self) -> EnvrResult<ZigManager> {
        ZigManager::try_new(self.runtime_root()?, self.index_url.clone())
    }

    fn remote_latest_per_major_cache_path(&self) -> EnvrResult<PathBuf> {
        let plat = zig_json_platform_key()?;
        let paths = ZigPaths::new(self.runtime_root()?);
        Ok(paths
            .cache_dir()
            .join(format!("remote_latest_per_major_{plat}.json")))
    }

    fn remote_cache_ttl_secs() -> u64 {
        const DEFAULT: u64 = 24 * 60 * 60;
        std::env::var("ENVR_ZIG_REMOTE_CACHE_TTL_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT)
    }
}

impl Default for ZigRuntimeProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeProvider for ZigRuntimeProvider {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::Zig
    }

    fn list_installed(&self) -> EnvrResult<Vec<RuntimeVersion>> {
        let paths = ZigPaths::new(self.runtime_root()?);
        list_installed_versions(&paths)
    }

    fn current(&self) -> EnvrResult<Option<RuntimeVersion>> {
        let paths = ZigPaths::new(self.runtime_root()?);
        read_current(&paths)
    }

    fn set_current(&self, version: &RuntimeVersion) -> EnvrResult<()> {
        self.manager()?.set_current(version)
    }

    fn list_remote(&self, filter: &RemoteFilter) -> EnvrResult<Vec<RuntimeVersion>> {
        self.manager()?.list_remote(filter)
    }

    fn try_load_remote_latest_installable_per_major_from_disk(&self) -> Vec<RuntimeVersion> {
        let Ok(path) = self.remote_latest_per_major_cache_path() else {
            return Vec::new();
        };
        let Some(list) =
            envr_platform::cache_recovery::read_json_string_list(&path, None, |xs| !xs.is_empty())
        else {
            return Vec::new();
        };
        list.into_iter().map(RuntimeVersion).collect()
    }

    fn list_remote_latest_per_major(&self) -> EnvrResult<Vec<RuntimeVersion>> {
        let ttl_secs = Self::remote_cache_ttl_secs();
        let cache_file = self.remote_latest_per_major_cache_path()?;

        if let Some(list) = envr_platform::cache_recovery::read_json_string_list(
            &cache_file,
            Some(ttl_secs),
            |xs| !xs.is_empty(),
        ) {
            return Ok(list.into_iter().map(RuntimeVersion).collect());
        }

        let list = self.manager()?.list_remote_latest_per_major()?;

        let _ = (|| -> EnvrResult<()> {
            let paths = ZigPaths::new(self.runtime_root()?);
            std::fs::create_dir_all(paths.cache_dir())?;
            let strings: Vec<String> = list.iter().map(|v| v.0.clone()).collect();
            let s = serde_json::to_string(&strings)
                .map_err(|e| envr_error::EnvrError::Validation(e.to_string()))?;
            envr_platform::fs_atomic::write_atomic(&cache_file, s.as_bytes())?;
            Ok(())
        })();

        Ok(list)
    }

    fn resolve(&self, spec: &VersionSpec) -> EnvrResult<ResolvedVersion> {
        let v = self.manager()?.resolve_label(&spec.0)?;
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

    fn uninstall_dry_run_targets(
        &self,
        version: &RuntimeVersion,
    ) -> EnvrResult<(Vec<PathBuf>, Option<String>)> {
        let paths = ZigPaths::new(self.runtime_root()?);
        Ok((vec![paths.version_dir(&version.0)], None))
    }
}
