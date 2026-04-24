mod index;
mod manager;

pub use envr_platform::lua_binaries::lua_installation_valid;
pub use index::{
    DEFAULT_LUA_DOWNLOAD_PAGE_URL, LuaHostKind, blocking_http_client, fetch_download_page,
    list_remote_latest_per_major_lines, list_remote_versions, parse_installable_versions,
    resolve_lua_version, sourceforge_tools_download_url, tools_executable_filename,
};
pub use manager::{LuaManager, LuaPaths, list_installed_versions, read_current};

use envr_config::env_context::runtime_root;
use envr_domain::installer::SpecDrivenInstaller;
use envr_domain::runtime::{
    InstallRequest, RemoteFilter, ResolvedVersion, RuntimeKind, RuntimeProvider, RuntimeVersion,
    VersionSpec,
};
use envr_error::EnvrResult;
use std::path::PathBuf;

pub struct LuaRuntimeProvider {
    download_page_url: String,
    runtime_root_override: Option<PathBuf>,
}

impl LuaRuntimeProvider {
    pub fn new() -> Self {
        Self {
            download_page_url: DEFAULT_LUA_DOWNLOAD_PAGE_URL.to_string(),
            runtime_root_override: None,
        }
    }

    pub fn with_download_page_url(mut self, url: impl Into<String>) -> Self {
        self.download_page_url = url.into();
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

    fn manager(&self) -> EnvrResult<LuaManager> {
        LuaManager::try_new(self.runtime_root()?, self.download_page_url.clone())
    }
}

impl Default for LuaRuntimeProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeProvider for LuaRuntimeProvider {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::Lua
    }

    fn list_installed(&self) -> EnvrResult<Vec<RuntimeVersion>> {
        let paths = LuaPaths::new(self.runtime_root()?);
        list_installed_versions(&paths)
    }

    fn current(&self) -> EnvrResult<Option<RuntimeVersion>> {
        let paths = LuaPaths::new(self.runtime_root()?);
        read_current(&paths)
    }

    fn set_current(&self, version: &RuntimeVersion) -> EnvrResult<()> {
        self.manager()?.set_current(version)
    }

    fn list_remote(&self, filter: &RemoteFilter) -> EnvrResult<Vec<RuntimeVersion>> {
        self.manager()?.list_remote(filter)
    }

    fn try_load_remote_latest_per_major_from_disk(&self) -> Vec<RuntimeVersion> {
        self.manager()
            .map(|m| m.try_load_remote_latest_per_major_from_disk())
            .unwrap_or_default()
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
        let paths = LuaPaths::new(self.runtime_root()?);
        Ok((vec![paths.version_dir(&version.0)], None))
    }
}
