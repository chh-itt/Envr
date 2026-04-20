mod index;
mod manager;

pub use index::{
    DEFAULT_TERRAFORM_INDEX_URL, TerraformIndexRow, artifact_url, blocking_http_client,
    fetch_index_text, list_remote_latest_per_major_lines, list_remote_versions,
    parse_versions_from_index_html, resolve_terraform_version, terraform_platform_tuple,
};
pub use manager::{
    TerraformManager, TerraformPaths, list_installed_versions, read_current,
    terraform_installation_valid, terraform_tool_candidate,
};

use envr_domain::runtime::{
    InstallRequest, RemoteFilter, ResolvedVersion, RuntimeKind, RuntimeProvider, RuntimeVersion,
    VersionSpec,
};
use envr_error::EnvrResult;
use envr_platform::paths::current_platform_paths;
use std::path::PathBuf;

pub struct TerraformRuntimeProvider {
    index_url: String,
    runtime_root_override: Option<PathBuf>,
}

impl TerraformRuntimeProvider {
    pub fn new() -> Self {
        Self {
            index_url: DEFAULT_TERRAFORM_INDEX_URL.to_string(),
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

    fn manager(&self) -> EnvrResult<TerraformManager> {
        TerraformManager::try_new(self.runtime_root()?, self.index_url.clone())
    }
}

impl Default for TerraformRuntimeProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeProvider for TerraformRuntimeProvider {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::Terraform
    }

    fn list_installed(&self) -> EnvrResult<Vec<RuntimeVersion>> {
        list_installed_versions(&TerraformPaths::new(self.runtime_root()?))
    }

    fn current(&self) -> EnvrResult<Option<RuntimeVersion>> {
        read_current(&TerraformPaths::new(self.runtime_root()?))
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
        let p = TerraformPaths::new(self.runtime_root()?).version_dir(&version.0);
        Ok((vec![p], None))
    }
}
