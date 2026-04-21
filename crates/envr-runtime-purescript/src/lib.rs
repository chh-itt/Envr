mod index;
mod manager;

pub use index::{
    DEFAULT_PURESCRIPT_RELEASES_API_URL, GhAsset, GhRelease, PurescriptInstallableRow,
    blocking_http_client, fetch_purescript_github_releases_index, installable_rows_from_releases,
    list_remote_latest_per_major_lines, list_remote_versions, purescript_asset_candidates,
    resolve_purescript_version,
};
pub use manager::{
    PurescriptManager, PurescriptPaths, list_installed_versions, purescript_installation_valid,
    read_current,
};

use envr_domain::runtime::{
    InstallRequest, RemoteFilter, ResolvedVersion, RuntimeKind, RuntimeProvider, RuntimeVersion,
    VersionSpec,
};
use envr_error::EnvrResult;
use envr_platform::paths::current_platform_paths;
use std::path::PathBuf;

pub struct PurescriptRuntimeProvider {
    releases_api_url: String,
    runtime_root_override: Option<PathBuf>,
}

impl PurescriptRuntimeProvider {
    pub fn new() -> Self {
        Self {
            releases_api_url: DEFAULT_PURESCRIPT_RELEASES_API_URL.to_string(),
            runtime_root_override: None,
        }
    }

    pub fn with_releases_api_url(mut self, url: impl Into<String>) -> Self {
        self.releases_api_url = url.into();
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

    fn manager(&self) -> EnvrResult<PurescriptManager> {
        PurescriptManager::try_new(self.runtime_root()?, self.releases_api_url.clone())
    }
}

impl Default for PurescriptRuntimeProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeProvider for PurescriptRuntimeProvider {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::Purescript
    }

    fn list_installed(&self) -> EnvrResult<Vec<RuntimeVersion>> {
        list_installed_versions(&PurescriptPaths::new(self.runtime_root()?))
    }

    fn current(&self) -> EnvrResult<Option<RuntimeVersion>> {
        read_current(&PurescriptPaths::new(self.runtime_root()?))
    }

    fn set_current(&self, version: &RuntimeVersion) -> EnvrResult<()> {
        self.manager()?.set_current(version)
    }

    fn list_remote(&self, filter: &RemoteFilter) -> EnvrResult<Vec<RuntimeVersion>> {
        self.manager()?.list_remote(filter)
    }

    fn list_remote_latest_per_major(&self) -> EnvrResult<Vec<RuntimeVersion>> {
        self.manager()?.list_remote_latest_per_major()
    }

    fn resolve(&self, spec: &VersionSpec) -> EnvrResult<ResolvedVersion> {
        Ok(ResolvedVersion {
            version: RuntimeVersion(self.manager()?.resolve_label(&spec.0)?),
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
    ) -> EnvrResult<(Vec<std::path::PathBuf>, Option<String>)> {
        let p = PurescriptPaths::new(self.runtime_root()?).version_dir(&version.0);
        Ok((vec![p], None))
    }
}

