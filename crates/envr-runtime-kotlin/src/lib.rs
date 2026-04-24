//! Kotlin JVM compiler runtime (JetBrains `kotlin-compiler` zip), `runtimes/kotlin/versions/<label>/`.

mod index;
mod manager;

pub use index::{
    DEFAULT_KOTLIN_RELEASES_API_URL, blocking_http_client, fetch_releases_json,
    installable_pairs_from_releases, list_remote_latest_per_major_lines, list_remote_versions,
    resolve_kotlin_version,
};
pub use manager::{
    KotlinManager, KotlinPaths, kotlin_installation_valid, kotlin_tool_candidate,
    list_installed_versions, promote_kotlin_extracted_tree, read_current,
};

use envr_config::env_context::runtime_root;
use envr_domain::installer::install_via_manager;
use envr_domain::runtime::{
    InstallRequest, RemoteFilter, ResolvedVersion, RuntimeKind, RuntimeProvider, RuntimeVersion,
    VersionSpec,
};
use envr_error::EnvrResult;
use std::path::PathBuf;

pub struct KotlinRuntimeProvider {
    releases_api_url: String,
    runtime_root_override: Option<PathBuf>,
}

impl KotlinRuntimeProvider {
    pub fn new() -> Self {
        Self {
            releases_api_url: DEFAULT_KOTLIN_RELEASES_API_URL.to_string(),
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

    fn manager(&self) -> EnvrResult<KotlinManager> {
        KotlinManager::try_new(self.runtime_root()?, self.releases_api_url.clone())
    }
}

impl Default for KotlinRuntimeProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeProvider for KotlinRuntimeProvider {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::Kotlin
    }

    fn list_installed(&self) -> EnvrResult<Vec<RuntimeVersion>> {
        let paths = KotlinPaths::new(self.runtime_root()?);
        list_installed_versions(&paths)
    }

    fn current(&self) -> EnvrResult<Option<RuntimeVersion>> {
        let paths = KotlinPaths::new(self.runtime_root()?);
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
        let path = KotlinPaths::new(root)
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
        install_via_manager(self.manager(), request)
    }

    fn uninstall(&self, version: &RuntimeVersion) -> EnvrResult<()> {
        self.manager()?.uninstall(version)
    }

    fn uninstall_dry_run_targets(
        &self,
        version: &RuntimeVersion,
    ) -> EnvrResult<(Vec<PathBuf>, Option<String>)> {
        let paths = KotlinPaths::new(self.runtime_root()?);
        Ok((vec![paths.version_dir(&version.0)], None))
    }
}
