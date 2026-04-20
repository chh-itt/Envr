mod index;
mod manager;
mod releases_url;

pub use index::{
    GroovyIndexRow, binary_zip_url_for, blocking_http_client, list_remote_latest_per_major_lines,
    list_remote_versions, merge_rows, parse_groovy_versions_from_index_html,
    resolve_groovy_version,
};
pub use manager::{
    GroovyManager, GroovyPaths, groovy_installation_valid, groovy_tool_candidate,
    list_installed_versions, promote_groovy_extracted_tree, read_current,
};
pub use releases_url::{
    GROOVY_APACHE_ARCHIVE_INDEX_URL, GROOVY_APACHE_PRIMARY_INDEX_URL,
    resolved_groovy_archive_index_url, resolved_groovy_primary_index_url,
};

use envr_domain::runtime::{
    InstallRequest, RemoteFilter, ResolvedVersion, RuntimeKind, RuntimeProvider, RuntimeVersion,
    VersionSpec,
};
use envr_error::EnvrResult;
use envr_platform::paths::current_platform_paths;
use std::path::PathBuf;

pub struct GroovyRuntimeProvider {
    primary_index_url: String,
    archive_index_url: String,
    runtime_root_override: Option<PathBuf>,
}

impl GroovyRuntimeProvider {
    pub fn new() -> Self {
        Self {
            primary_index_url: resolved_groovy_primary_index_url(),
            archive_index_url: resolved_groovy_archive_index_url(),
            runtime_root_override: None,
        }
    }

    pub fn with_index_urls(
        mut self,
        primary: impl Into<String>,
        archive: impl Into<String>,
    ) -> Self {
        self.primary_index_url = primary.into();
        self.archive_index_url = archive.into();
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

    fn manager(&self) -> EnvrResult<GroovyManager> {
        GroovyManager::try_new(
            self.runtime_root()?,
            self.primary_index_url.clone(),
            self.archive_index_url.clone(),
        )
    }
}

impl Default for GroovyRuntimeProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeProvider for GroovyRuntimeProvider {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::Groovy
    }

    fn list_installed(&self) -> EnvrResult<Vec<RuntimeVersion>> {
        list_installed_versions(&GroovyPaths::new(self.runtime_root()?))
    }

    fn current(&self) -> EnvrResult<Option<RuntimeVersion>> {
        read_current(&GroovyPaths::new(self.runtime_root()?))
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
        let path = GroovyPaths::new(root)
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
        let p = GroovyPaths::new(self.runtime_root()?).version_dir(&version.0);
        Ok((vec![p], None))
    }
}
