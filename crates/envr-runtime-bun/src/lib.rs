mod index;
mod manager;
mod mirror;

pub use index::{
    DEFAULT_BUN_TAGS_API, Tag, blocking_http_client, fetch_tags, list_remote_versions, parse_tags,
    resolve_bun_version,
};
pub use manager::{
    BunManager, BunPaths, bun_installation_valid, list_installed_versions, read_current,
};

use envr_domain::runtime::{
    InstallRequest, RemoteFilter, ResolvedVersion, RuntimeKind, RuntimeProvider, RuntimeVersion,
    VersionSpec,
};
use envr_error::EnvrResult;
use envr_platform::paths::current_platform_paths;

pub struct BunRuntimeProvider {
    tags_api: String,
    runtime_root_override: Option<std::path::PathBuf>,
}

impl BunRuntimeProvider {
    pub fn new() -> Self {
        Self {
            tags_api: DEFAULT_BUN_TAGS_API.to_string(),
            runtime_root_override: None,
        }
    }

    pub fn with_tags_api(mut self, url: impl Into<String>) -> Self {
        self.tags_api = url.into();
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

    fn manager(&self) -> EnvrResult<BunManager> {
        BunManager::try_new(self.runtime_root()?, self.tags_api.clone())
    }

    fn load_tags(&self) -> EnvrResult<Vec<Tag>> {
        let client = blocking_http_client()?;
        let settings = mirror::load_settings()?;
        let url = mirror::maybe_mirror_url(&settings, &self.tags_api)?;
        let body = fetch_tags(&client, &url)?;
        parse_tags(&body)
    }
}

impl Default for BunRuntimeProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeProvider for BunRuntimeProvider {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::Bun
    }

    fn list_installed(&self) -> EnvrResult<Vec<RuntimeVersion>> {
        let paths = BunPaths::new(self.runtime_root()?);
        list_installed_versions(&paths)
    }

    fn current(&self) -> EnvrResult<Option<RuntimeVersion>> {
        let paths = BunPaths::new(self.runtime_root()?);
        read_current(&paths)
    }

    fn set_current(&self, version: &RuntimeVersion) -> EnvrResult<()> {
        self.manager()?.set_current(version)
    }

    fn list_remote(&self, filter: &RemoteFilter) -> EnvrResult<Vec<RuntimeVersion>> {
        let tags = self.load_tags()?;
        list_remote_versions(&tags, filter)
    }

    fn resolve(&self, spec: &VersionSpec) -> EnvrResult<ResolvedVersion> {
        let tags = self.load_tags()?;
        let v = resolve_bun_version(&tags, &spec.0)?;
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
