//! Crystal runtime: GitHub `crystal-lang/crystal` releases JSON, per-host tarball/zip, `versions/<label>/` layout.

mod index;
mod manager;
mod releases_url;

pub use index::{
    DEFAULT_CRYSTAL_GITHUB_RELEASES_URL, blocking_http_client, crystal_host_slug, fetch_text,
    list_remote_latest_per_major_lines, list_remote_versions, parse_cached_install_rows,
    parse_github_releases_for_host, resolve_crystal_version,
};
pub use manager::{
    CrystalManager, CrystalPaths, crystal_installation_valid, list_installed_versions,
    promote_single_root_dir, read_current,
};

use envr_config::env_context::runtime_root;
use envr_domain::installer::SpecDrivenInstaller;
use envr_domain::runtime::{
    InstallRequest, RemoteFilter, ResolvedVersion, RuntimeKind, RuntimeProvider, RuntimeVersion,
    VersionSpec,
};
use envr_error::{EnvrError, EnvrResult, ErrorCode};
use std::path::PathBuf;

pub struct CrystalRuntimeProvider {
    releases_url: String,
    runtime_root_override: Option<PathBuf>,
}

impl CrystalRuntimeProvider {
    pub fn new() -> Self {
        Self {
            releases_url: releases_url::resolved_crystal_releases_api_url(
                DEFAULT_CRYSTAL_GITHUB_RELEASES_URL,
            ),
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
            None => runtime_root()?,
        })
    }

    fn manager(&self) -> EnvrResult<CrystalManager> {
        CrystalManager::try_new(self.runtime_root()?, self.releases_url.clone())
    }

    fn remote_latest_per_major_cache_path(&self) -> EnvrResult<PathBuf> {
        let slug = crystal_host_slug()?;
        let paths = CrystalPaths::new(self.runtime_root()?);
        Ok(paths
            .cache_dir()
            .join(format!("remote_latest_per_major_{slug}.json")))
    }

    fn remote_cache_ttl_secs() -> u64 {
        const DEFAULT: u64 = 24 * 60 * 60;
        std::env::var("ENVR_CRYSTAL_REMOTE_CACHE_TTL_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT)
    }
}

impl Default for CrystalRuntimeProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeProvider for CrystalRuntimeProvider {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::Crystal
    }

    fn list_installed(&self) -> EnvrResult<Vec<RuntimeVersion>> {
        let paths = CrystalPaths::new(self.runtime_root()?);
        list_installed_versions(&paths)
    }

    fn current(&self) -> EnvrResult<Option<RuntimeVersion>> {
        let paths = CrystalPaths::new(self.runtime_root()?);
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

    fn list_remote_latest_installable_per_major(&self) -> EnvrResult<Vec<RuntimeVersion>> {
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
            let paths = CrystalPaths::new(self.runtime_root()?);
            std::fs::create_dir_all(paths.cache_dir())?;
            let strings: Vec<String> = list.iter().map(|v| v.0.clone()).collect();
            let s = serde_json::to_string(&strings)
                .map_err(|e| EnvrError::with_source(ErrorCode::Validation, "json encode crystal latest labels", e))?;
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
        let paths = CrystalPaths::new(self.runtime_root()?);
        Ok((vec![paths.version_dir(&version.0)], None))
    }
}
