mod index;

pub use index::{
    DEFAULT_PYTHON_RELEASE_FILES_URL, DEFAULT_PYTHON_RELEASES_URL, PyRelease, PyReleaseFile,
    PythonIndex, blocking_http_client, fetch_json, list_remote_versions, load_python_index,
    normalize_python_version_label, parse_release_file_list, parse_release_list,
    release_has_platform_assets, resolve_python_version,
};

use envr_domain::runtime::{
    InstallRequest, RemoteFilter, ResolvedVersion, RuntimeKind, RuntimeProvider, RuntimeVersion,
    VersionSpec,
};
use envr_error::{EnvrError, EnvrResult};

pub struct PythonRuntimeProvider {
    releases_url: String,
    files_url: String,
}

impl PythonRuntimeProvider {
    pub fn new() -> Self {
        Self {
            releases_url: DEFAULT_PYTHON_RELEASES_URL.to_string(),
            files_url: DEFAULT_PYTHON_RELEASE_FILES_URL.to_string(),
        }
    }

    pub fn with_api_urls(releases_url: impl Into<String>, files_url: impl Into<String>) -> Self {
        Self {
            releases_url: releases_url.into(),
            files_url: files_url.into(),
        }
    }

    fn load_index(&self) -> EnvrResult<PythonIndex> {
        let client = index::blocking_http_client()?;
        index::load_python_index(&client, &self.releases_url, &self.files_url)
    }
}

impl Default for PythonRuntimeProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeProvider for PythonRuntimeProvider {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::Python
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
        let idx = self.load_index()?;
        list_remote_versions(&idx, std::env::consts::OS, std::env::consts::ARCH, filter)
    }

    fn resolve(&self, spec: &VersionSpec) -> EnvrResult<ResolvedVersion> {
        let idx = self.load_index()?;
        let v =
            resolve_python_version(&idx, std::env::consts::OS, std::env::consts::ARCH, &spec.0)?;
        Ok(ResolvedVersion {
            version: RuntimeVersion(v),
        })
    }

    fn install(&self, request: &InstallRequest) -> EnvrResult<RuntimeVersion> {
        Ok(self.resolve(&request.spec)?.version)
    }

    fn uninstall(&self, _version: &RuntimeVersion) -> EnvrResult<()> {
        Err(EnvrError::Runtime("not implemented".to_string()))
    }
}
