mod index;
mod manager;

pub use index::{
    DEFAULT_GO_DL_JSON_URL, GoRelease, blocking_http_client, fetch_go_index, list_remote_versions,
    normalize_go_version, parse_go_index, resolve_go_version,
};
pub use manager::{
    GoManager, GoPaths, go_installation_valid, list_installed_versions, read_current,
};

use envr_domain::runtime::{
    InstallRequest, RemoteFilter, ResolvedVersion, RuntimeKind, RuntimeProvider, RuntimeVersion,
    VersionSpec,
};
use envr_error::EnvrResult;
use envr_platform::paths::current_platform_paths;

pub struct GoRuntimeProvider {
    dl_json_url: String,
    runtime_root_override: Option<std::path::PathBuf>,
}

impl GoRuntimeProvider {
    pub fn new() -> Self {
        Self {
            dl_json_url: DEFAULT_GO_DL_JSON_URL.to_string(),
            runtime_root_override: None,
        }
    }

    pub fn with_dl_json_url(url: impl Into<String>) -> Self {
        Self {
            dl_json_url: url.into(),
            runtime_root_override: None,
        }
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

    fn manager(&self) -> EnvrResult<GoManager> {
        GoManager::try_new(self.runtime_root()?, self.dl_json_url.clone())
    }

    fn load_releases(&self) -> EnvrResult<Vec<GoRelease>> {
        let client = blocking_http_client()?;
        let body = fetch_go_index(&client, &self.dl_json_url)?;
        parse_go_index(&body)
    }
}

impl Default for GoRuntimeProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeProvider for GoRuntimeProvider {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::Go
    }

    fn list_installed(&self) -> EnvrResult<Vec<RuntimeVersion>> {
        let paths = GoPaths::new(self.runtime_root()?);
        list_installed_versions(&paths)
    }

    fn current(&self) -> EnvrResult<Option<RuntimeVersion>> {
        let paths = GoPaths::new(self.runtime_root()?);
        read_current(&paths)
    }

    fn set_current(&self, version: &RuntimeVersion) -> EnvrResult<()> {
        self.manager()?.set_current(version)
    }

    fn list_remote(&self, filter: &RemoteFilter) -> EnvrResult<Vec<RuntimeVersion>> {
        let releases = self.load_releases()?;
        list_remote_versions(&releases, filter)
    }

    fn resolve(&self, spec: &VersionSpec) -> EnvrResult<ResolvedVersion> {
        let releases = self.load_releases()?;
        let v = resolve_go_version(&releases, &spec.0)?;
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
