mod index;
mod manager;

pub use index::{
    DEFAULT_PHP_WINDOWS_RELEASES_JSON_URL, PhpReleasesIndex, ReleaseLine, blocking_http_client,
    fetch_php_windows_releases_json, list_remote_versions, parse_php_windows_index,
    resolve_php_version,
};
pub use manager::{
    PhpManager, PhpPaths, list_installed_versions, php_installation_valid, read_current,
};

use envr_domain::runtime::{
    InstallRequest, RemoteFilter, ResolvedVersion, RuntimeKind, RuntimeProvider, RuntimeVersion,
    VersionSpec,
};
use envr_error::{EnvrError, EnvrResult};
use envr_platform::paths::current_platform_paths;

pub struct PhpRuntimeProvider {
    releases_json_url: String,
    runtime_root_override: Option<std::path::PathBuf>,
}

impl PhpRuntimeProvider {
    pub fn new() -> Self {
        Self {
            releases_json_url: DEFAULT_PHP_WINDOWS_RELEASES_JSON_URL.to_string(),
            runtime_root_override: None,
        }
    }

    pub fn with_releases_json_url(mut self, url: impl Into<String>) -> Self {
        self.releases_json_url = url.into();
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

    fn manager(&self) -> EnvrResult<PhpManager> {
        PhpManager::try_new(self.runtime_root()?, self.releases_json_url.clone())
    }

    fn load_index(&self) -> EnvrResult<PhpReleasesIndex> {
        let client = blocking_http_client()?;
        let body = fetch_php_windows_releases_json(&client, &self.releases_json_url)?;
        parse_php_windows_index(&body)
    }
}

impl Default for PhpRuntimeProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeProvider for PhpRuntimeProvider {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::Php
    }

    fn list_installed(&self) -> EnvrResult<Vec<RuntimeVersion>> {
        let paths = PhpPaths::new(self.runtime_root()?);
        list_installed_versions(&paths)
    }

    fn current(&self) -> EnvrResult<Option<RuntimeVersion>> {
        let paths = PhpPaths::new(self.runtime_root()?);
        read_current(&paths)
    }

    fn set_current(&self, version: &RuntimeVersion) -> EnvrResult<()> {
        self.manager()?.set_current(version)
    }

    fn list_remote(&self, filter: &RemoteFilter) -> EnvrResult<Vec<RuntimeVersion>> {
        if !cfg!(windows) {
            return Err(EnvrError::Platform(
                "php remote listing is currently supported on Windows only".into(),
            ));
        }
        let idx = self.load_index()?;
        list_remote_versions(&idx, filter)
    }

    fn resolve(&self, spec: &VersionSpec) -> EnvrResult<ResolvedVersion> {
        if !cfg!(windows) {
            return Err(EnvrError::Platform(
                "php resolve is currently supported on Windows only".into(),
            ));
        }
        let idx = self.load_index()?;
        let v = resolve_php_version(&idx, &spec.0)?;
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
