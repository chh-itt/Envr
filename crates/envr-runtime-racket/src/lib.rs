mod index;
mod manager;

pub use index::{
    DEFAULT_RACKET_ALL_VERSIONS_URL, RacketInstallableRow, blocking_http_client,
    fetch_racket_installable_rows, list_remote_latest_per_major_lines, list_remote_versions,
    resolve_racket_version,
};
pub use manager::{
    RacketManager, RacketPaths, list_installed_versions, racket_installation_valid, read_current,
};

use envr_config::env_context::runtime_root;
use envr_domain::runtime::{
    InstallRequest, RemoteFilter, ResolvedVersion, RuntimeKind, RuntimeProvider, RuntimeVersion,
    VersionSpec,
};
use envr_error::EnvrResult;
use std::path::PathBuf;

pub struct RacketRuntimeProvider {
    all_versions_url: String,
    runtime_root_override: Option<PathBuf>,
}

impl RacketRuntimeProvider {
    pub fn new() -> Self {
        Self {
            all_versions_url: DEFAULT_RACKET_ALL_VERSIONS_URL.to_string(),
            runtime_root_override: None,
        }
    }
    pub fn with_all_versions_url(mut self, url: impl Into<String>) -> Self {
        self.all_versions_url = url.into();
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
    fn manager(&self) -> EnvrResult<RacketManager> {
        RacketManager::try_new(self.runtime_root()?, self.all_versions_url.clone())
    }
}

impl Default for RacketRuntimeProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeProvider for RacketRuntimeProvider {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::Racket
    }
    fn list_installed(&self) -> EnvrResult<Vec<RuntimeVersion>> {
        list_installed_versions(&RacketPaths::new(self.runtime_root()?))
    }
    fn current(&self) -> EnvrResult<Option<RuntimeVersion>> {
        read_current(&RacketPaths::new(self.runtime_root()?))
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
        Ok((vec![RacketPaths::new(self.runtime_root()?).version_dir(&version.0)], None))
    }
}

