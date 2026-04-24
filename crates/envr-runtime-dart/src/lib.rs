mod index;
mod manager;

pub use index::{
    DEFAULT_DART_BUCKET_LIST_API_URL, DEFAULT_DART_LATEST_VERSION_URL, DartIndexRow, artifact_url,
    blocking_http_client, dart_platform_tuple, fetch_text, list_remote_latest_per_major_lines,
    list_remote_versions, parse_rows_from_bucket_list_json, resolve_dart_version,
};
pub use manager::{DartManager, DartPaths, list_installed_versions, read_current};

use envr_config::env_context::runtime_root;
use envr_domain::runtime::{
    InstallRequest, RemoteFilter, ResolvedVersion, RuntimeKind, RuntimeProvider, RuntimeVersion,
    VersionSpec,
};
use envr_error::EnvrResult;
use std::path::PathBuf;

pub struct DartRuntimeProvider {
    bucket_list_api_url: String,
    runtime_root_override: Option<PathBuf>,
}

impl DartRuntimeProvider {
    pub fn new() -> Self {
        Self {
            bucket_list_api_url: DEFAULT_DART_BUCKET_LIST_API_URL.to_string(),
            runtime_root_override: None,
        }
    }

    pub fn with_bucket_list_api_url(mut self, url: impl Into<String>) -> Self {
        self.bucket_list_api_url = url.into();
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

    fn manager(&self) -> EnvrResult<DartManager> {
        DartManager::try_new(self.runtime_root()?, self.bucket_list_api_url.clone())
    }
}

impl Default for DartRuntimeProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeProvider for DartRuntimeProvider {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::Dart
    }

    fn list_installed(&self) -> EnvrResult<Vec<RuntimeVersion>> {
        list_installed_versions(&DartPaths::new(self.runtime_root()?))
    }

    fn current(&self) -> EnvrResult<Option<RuntimeVersion>> {
        read_current(&DartPaths::new(self.runtime_root()?))
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
    ) -> EnvrResult<(Vec<PathBuf>, Option<String>)> {
        let p = DartPaths::new(self.runtime_root()?).version_dir(&version.0);
        Ok((vec![p], None))
    }
}
