mod index;
mod manager;
mod vendor;

pub use index::{
    DEFAULT_ADOPTIUM_API_BASE, JavaIndex, JavaVersionEntry, adoptium_arch,
    adoptium_assets_version_range_segment, adoptium_os, blocking_http_client,
    fetch_available_lts_majors, fetch_release_versions, find_version_entry, list_remote_versions,
    load_java_index, normalize_openjdk_version_label, resolve_java_version,
};
pub use manager::{
    JavaManager, JavaPaths, java_installation_valid, list_installed_versions,
    promote_single_root_dir, read_current, read_java_home_export, sync_java_home_export,
};
pub use vendor::JavaVendor;

use envr_domain::runtime::{
    InstallRequest, RemoteFilter, ResolvedVersion, RuntimeKind, RuntimeProvider, RuntimeVersion,
    VersionSpec,
};
use envr_error::EnvrResult;
use envr_platform::paths::current_platform_paths;

/// JDK runtime provider (Adoptium Temurin: index, download, `current`, `JAVA_HOME` marker file).
pub struct JavaRuntimeProvider {
    api_base: String,
    vendor: JavaVendor,
    runtime_root_override: Option<std::path::PathBuf>,
}

impl JavaRuntimeProvider {
    pub fn new() -> Self {
        Self {
            api_base: DEFAULT_ADOPTIUM_API_BASE.to_string(),
            vendor: JavaVendor::default(),
            runtime_root_override: None,
        }
    }

    pub fn with_api_base(mut self, base: impl Into<String>) -> Self {
        self.api_base = base.into();
        self
    }

    pub fn with_vendor(mut self, vendor: JavaVendor) -> Self {
        self.vendor = vendor;
        self
    }

    pub fn with_runtime_root(mut self, root: std::path::PathBuf) -> Self {
        self.runtime_root_override = Some(root);
        self
    }

    fn runtime_root(&self) -> EnvrResult<std::path::PathBuf> {
        Ok(match &self.runtime_root_override {
            Some(p) => p.clone(),
            None => current_platform_paths()?.runtime_root,
        })
    }

    fn manager(&self) -> EnvrResult<JavaManager> {
        JavaManager::try_new(self.runtime_root()?, self.api_base.clone(), self.vendor)
    }

    fn load_index(&self) -> EnvrResult<JavaIndex> {
        let client = blocking_http_client()?;
        load_java_index(
            &client,
            &self.api_base,
            self.vendor,
            std::env::consts::OS,
            std::env::consts::ARCH,
        )
    }
}

impl Default for JavaRuntimeProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeProvider for JavaRuntimeProvider {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::Java
    }

    fn list_installed(&self) -> EnvrResult<Vec<RuntimeVersion>> {
        let paths = JavaPaths::new(self.runtime_root()?);
        list_installed_versions(&paths)
    }

    fn current(&self) -> EnvrResult<Option<RuntimeVersion>> {
        let paths = JavaPaths::new(self.runtime_root()?);
        read_current(&paths)
    }

    fn set_current(&self, version: &RuntimeVersion) -> EnvrResult<()> {
        self.manager()?.set_current(version)
    }

    fn list_remote(&self, filter: &RemoteFilter) -> EnvrResult<Vec<RuntimeVersion>> {
        let index = self.load_index()?;
        list_remote_versions(&index, filter)
    }

    fn resolve(&self, spec: &VersionSpec) -> EnvrResult<ResolvedVersion> {
        let index = self.load_index()?;
        let v = resolve_java_version(&index, &spec.0)?;
        Ok(ResolvedVersion {
            version: RuntimeVersion(v),
        })
    }

    fn install(&self, request: &InstallRequest) -> EnvrResult<RuntimeVersion> {
        self.manager()?.install_from_spec(&request.spec)
    }

    fn uninstall(&self, version: &RuntimeVersion) -> EnvrResult<()> {
        self.manager()?.uninstall(version)
    }
}
