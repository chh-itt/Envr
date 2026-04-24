mod index;
mod manager;

pub use index::{
    DEFAULT_FLUTTER_RELEASES_LINUX_URL, DEFAULT_FLUTTER_RELEASES_MACOS_URL,
    DEFAULT_FLUTTER_RELEASES_WINDOWS_URL, FlutterIndexRow, blocking_http_client, fetch_text,
    list_remote_latest_per_major_lines, list_remote_versions, parse_rows_from_releases_json,
    releases_json_url_for_host, resolve_flutter_version,
};
pub use manager::{FlutterManager, FlutterPaths, list_installed_versions, read_current};

use envr_config::env_context::runtime_root;
use envr_domain::runtime::{
    InstallRequest, RemoteFilter, ResolvedVersion, RuntimeKind, RuntimeProvider, RuntimeVersion,
    VersionSpec,
};
use envr_error::EnvrResult;
use std::path::PathBuf;

pub struct FlutterRuntimeProvider {
    releases_url_override: Option<String>,
    runtime_root_override: Option<PathBuf>,
}

impl FlutterRuntimeProvider {
    pub fn new() -> Self {
        Self {
            releases_url_override: None,
            runtime_root_override: None,
        }
    }

    pub fn with_releases_url(mut self, url: impl Into<String>) -> Self {
        self.releases_url_override = Some(url.into());
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

    fn manager(&self) -> EnvrResult<FlutterManager> {
        FlutterManager::try_new(self.runtime_root()?, self.releases_url_override.clone())
    }
}

impl Default for FlutterRuntimeProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeProvider for FlutterRuntimeProvider {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::Flutter
    }

    fn list_installed(&self) -> EnvrResult<Vec<RuntimeVersion>> {
        list_installed_versions(&FlutterPaths::new(self.runtime_root()?))
    }

    fn current(&self) -> EnvrResult<Option<RuntimeVersion>> {
        read_current(&FlutterPaths::new(self.runtime_root()?))
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
        let p = FlutterPaths::new(self.runtime_root()?).version_dir(&version.0);
        Ok((vec![p], None))
    }
}
