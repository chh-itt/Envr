mod index;
mod vendor;

pub use index::{
    DEFAULT_ADOPTIUM_API_BASE, JavaIndex, JavaVersionEntry, adoptium_arch, adoptium_os,
    blocking_http_client, fetch_available_lts_majors, fetch_release_versions, list_remote_versions,
    load_java_index, resolve_java_version,
};
pub use vendor::JavaVendor;

use envr_domain::runtime::{
    InstallRequest, RemoteFilter, ResolvedVersion, RuntimeKind, RuntimeProvider, RuntimeVersion,
    VersionSpec,
};
use envr_error::{EnvrError, EnvrResult};

/// JDK runtime provider (Adoptium Temurin index; install layout in T021).
pub struct JavaRuntimeProvider {
    api_base: String,
    vendor: JavaVendor,
}

impl JavaRuntimeProvider {
    pub fn new() -> Self {
        Self {
            api_base: DEFAULT_ADOPTIUM_API_BASE.to_string(),
            vendor: JavaVendor::default(),
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
        Ok(vec![])
    }

    fn current(&self) -> EnvrResult<Option<RuntimeVersion>> {
        Ok(None)
    }

    fn set_current(&self, _version: &RuntimeVersion) -> EnvrResult<()> {
        Err(EnvrError::Runtime("not implemented".to_string()))
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
        let resolved = self.resolve(&request.spec)?;
        Ok(resolved.version)
    }

    fn uninstall(&self, _version: &RuntimeVersion) -> EnvrResult<()> {
        Err(EnvrError::Runtime("not implemented".to_string()))
    }
}
