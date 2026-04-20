//! Scala 3 runtime (`scala/scala3` GitHub releases), `runtimes/scala/versions/<label>/`.

mod releases_url;
mod index;
mod manager;

pub use index::{
    blocking_http_client, fetch_releases_json, fetch_scala_github_releases_index,
    installable_pairs_from_releases, list_remote_latest_per_major_lines, list_remote_versions,
    resolve_scala_version,
};
pub use releases_url::{DEFAULT_SCALA_RELEASES_API_URL, resolved_scala_releases_api_url};
pub use manager::{
    ScalaManager, ScalaPaths, scala_installation_valid, scala_tool_candidate,
    list_installed_versions, promote_scala_extracted_tree, read_current,
};

use envr_domain::runtime::{
    InstallRequest, RemoteFilter, ResolvedVersion, RuntimeKind, RuntimeProvider, RuntimeVersion,
    VersionSpec,
};
use envr_error::EnvrResult;
use envr_platform::paths::current_platform_paths;
use std::path::PathBuf;

pub struct ScalaRuntimeProvider {
    releases_api_url: String,
    runtime_root_override: Option<PathBuf>,
}

impl ScalaRuntimeProvider {
    pub fn new() -> Self {
        Self {
            releases_api_url: resolved_scala_releases_api_url(),
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

    fn manager(&self) -> EnvrResult<ScalaManager> {
        ScalaManager::try_new(self.runtime_root()?, self.releases_api_url.clone())
    }
}

impl Default for ScalaRuntimeProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeProvider for ScalaRuntimeProvider {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::Scala
    }

    fn list_installed(&self) -> EnvrResult<Vec<RuntimeVersion>> {
        let paths = ScalaPaths::new(self.runtime_root()?);
        list_installed_versions(&paths)
    }

    fn current(&self) -> EnvrResult<Option<RuntimeVersion>> {
        let paths = ScalaPaths::new(self.runtime_root()?);
        read_current(&paths)
    }

    fn set_current(&self, version: &RuntimeVersion) -> EnvrResult<()> {
        self.manager()?.set_current(version)
    }

    fn list_remote(&self, filter: &RemoteFilter) -> EnvrResult<Vec<RuntimeVersion>> {
        self.manager()?.list_remote(filter)
    }

    fn try_load_remote_latest_installable_per_major_from_disk(&self) -> Vec<RuntimeVersion> {
        let Ok(root) = self.runtime_root() else {
            return Vec::new();
        };
        let path = ScalaPaths::new(root)
            .cache_dir()
            .join("remote_latest_per_major.json");
        let Some(list) =
            envr_platform::cache_recovery::read_json_string_list(&path, None, |xs| !xs.is_empty())
        else {
            return Vec::new();
        };
        list.into_iter().map(RuntimeVersion).collect()
    }

    fn list_remote_latest_per_major(&self) -> EnvrResult<Vec<RuntimeVersion>> {
        self.manager()?.list_remote_latest_per_major_cached()
    }

    fn resolve(&self, spec: &VersionSpec) -> EnvrResult<ResolvedVersion> {
        let label = self.manager()?.resolve_label(&spec.0)?;
        Ok(ResolvedVersion {
            version: RuntimeVersion(label),
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
        let paths = ScalaPaths::new(self.runtime_root()?);
        Ok((vec![paths.version_dir(&version.0)], None))
    }
}
