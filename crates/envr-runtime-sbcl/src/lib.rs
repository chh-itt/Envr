mod index;
mod manager;

pub use index::{
    DEFAULT_SBCL_BIN_RELEASES_API_URL, SbclInstallableRow, blocking_http_client,
    fetch_sbcl_installable_rows_with_fallback, list_remote_latest_per_major_lines,
    list_remote_versions, resolve_sbcl_version,
};
pub use manager::{SbclManager, SbclPaths, list_installed_versions, read_current, sbcl_installation_valid};

use envr_config::env_context::runtime_root;
use envr_domain::installer::SpecDrivenInstaller;
use envr_domain::runtime::{
    InstallRequest, RemoteFilter, ResolvedVersion, RuntimeKind, RuntimeProvider, RuntimeVersion,
    VersionSpec,
};
use envr_error::EnvrResult;
use std::path::PathBuf;

pub struct SbclRuntimeProvider {
    releases_api_url: String,
    runtime_root_override: Option<PathBuf>,
}

impl SbclRuntimeProvider {
    pub fn new() -> Self {
        Self {
            releases_api_url: DEFAULT_SBCL_BIN_RELEASES_API_URL.to_string(),
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
            None => runtime_root()?,
        })
    }

    fn manager(&self) -> EnvrResult<SbclManager> {
        SbclManager::try_new(self.runtime_root()?, self.releases_api_url.clone())
    }
}

impl Default for SbclRuntimeProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeProvider for SbclRuntimeProvider {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::Sbcl
    }

    fn list_installed(&self) -> EnvrResult<Vec<RuntimeVersion>> {
        list_installed_versions(&SbclPaths::new(self.runtime_root()?))
    }

    fn current(&self) -> EnvrResult<Option<RuntimeVersion>> {
        read_current(&SbclPaths::new(self.runtime_root()?))
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
        Ok((vec![SbclPaths::new(self.runtime_root()?).version_dir(&version.0)], None))
    }
}

