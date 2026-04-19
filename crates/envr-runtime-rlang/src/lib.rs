//! R language (CRAN Windows): rversions JSON + silent Inno installer, `runtimes/r/versions/<ver>/`.

mod index;
mod manager;

pub use index::{
    DEFAULT_RVERSIONS_JSON_URL, DEFAULT_RVERSIONS_RELEASE_WIN_URL, blocking_http_client, cran_windows_r_installer_url,
    fetch_text, list_remote_latest_per_major_lines, list_remote_versions, parse_latest_win_release_version,
    parse_r_versions_list, resolve_r_version,
};
pub use envr_platform::bin_tool_layout::rlang_installation_valid;
pub use manager::{RlangManager, RlangPaths, list_installed_versions, read_current};

use envr_domain::runtime::{
    InstallRequest, RemoteFilter, ResolvedVersion, RuntimeKind, RuntimeProvider, RuntimeVersion,
    VersionSpec,
};
use envr_error::EnvrResult;
use envr_platform::paths::current_platform_paths;
use std::path::PathBuf;

pub struct RlangRuntimeProvider {
    versions_url: String,
    release_win_url: String,
    runtime_root_override: Option<PathBuf>,
}

impl RlangRuntimeProvider {
    pub fn new() -> Self {
        Self {
            versions_url: DEFAULT_RVERSIONS_JSON_URL.to_string(),
            release_win_url: DEFAULT_RVERSIONS_RELEASE_WIN_URL.to_string(),
            runtime_root_override: None,
        }
    }

    pub fn with_versions_url(mut self, url: impl Into<String>) -> Self {
        self.versions_url = url.into();
        self
    }

    pub fn with_release_win_url(mut self, url: impl Into<String>) -> Self {
        self.release_win_url = url.into();
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

    fn manager(&self) -> EnvrResult<RlangManager> {
        RlangManager::try_new(
            self.runtime_root()?,
            self.versions_url.clone(),
            self.release_win_url.clone(),
        )
    }

    fn remote_latest_per_major_cache_path(&self) -> EnvrResult<PathBuf> {
        let paths = RlangPaths::new(self.runtime_root()?);
        Ok(paths
            .cache_dir()
            .join("remote_latest_per_major_win.json"))
    }

    fn remote_cache_ttl_secs() -> u64 {
        const DEFAULT: u64 = 24 * 60 * 60;
        std::env::var("ENVR_RLANG_REMOTE_CACHE_TTL_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT)
    }
}

impl Default for RlangRuntimeProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeProvider for RlangRuntimeProvider {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::RLang
    }

    fn list_installed(&self) -> EnvrResult<Vec<RuntimeVersion>> {
        let paths = RlangPaths::new(self.runtime_root()?);
        list_installed_versions(&paths)
    }

    fn current(&self) -> EnvrResult<Option<RuntimeVersion>> {
        let paths = RlangPaths::new(self.runtime_root()?);
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
            let paths = RlangPaths::new(self.runtime_root()?);
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
        let paths = RlangPaths::new(self.runtime_root()?);
        Ok((vec![paths.version_dir(&version.0)], None))
    }
}
