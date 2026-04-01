mod index;
mod manager;

pub use index::{
    DEFAULT_NODE_INDEX_JSON_URL, NodeRelease, blocking_http_client, fetch_node_index,
    list_remote_versions, node_version_v_prefix, normalize_node_version, parse_node_index,
    release_has_platform, resolve_node_version,
};
pub use manager::{
    NodeManager, NodePaths, dist_root_from_index_json_url, list_installed_versions,
    node_installation_valid, parse_shasums256, pick_node_dist_artifact, promote_single_root_dir,
    read_current,
};

use envr_domain::runtime::{
    InstallRequest, RemoteFilter, ResolvedVersion, RuntimeKind, RuntimeProvider, RuntimeVersion,
    VersionSpec,
};
use envr_error::EnvrResult;
use envr_platform::paths::current_platform_paths;

/// Node.js runtime provider (remote index, install layout under envr data root).
pub struct NodeRuntimeProvider {
    index_json_url: String,
    /// When `None`, uses [`current_platform_paths`].
    runtime_root_override: Option<std::path::PathBuf>,
}

impl NodeRuntimeProvider {
    pub fn new() -> Self {
        Self {
            index_json_url: DEFAULT_NODE_INDEX_JSON_URL.to_string(),
            runtime_root_override: None,
        }
    }

    pub fn with_index_json_url(url: impl Into<String>) -> Self {
        Self {
            index_json_url: url.into(),
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

    fn manager(&self) -> EnvrResult<NodeManager> {
        NodeManager::try_new(self.runtime_root()?, self.index_json_url.clone())
    }

    fn load_releases(&self) -> EnvrResult<Vec<NodeRelease>> {
        let client = index::blocking_http_client()?;
        let body = index::fetch_node_index(&client, &self.index_json_url)?;
        index::parse_node_index(&body)
    }
}

impl Default for NodeRuntimeProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeProvider for NodeRuntimeProvider {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::Node
    }

    fn list_installed(&self) -> EnvrResult<Vec<RuntimeVersion>> {
        let paths = NodePaths::new(self.runtime_root()?);
        list_installed_versions(&paths)
    }

    fn current(&self) -> EnvrResult<Option<RuntimeVersion>> {
        let paths = NodePaths::new(self.runtime_root()?);
        read_current(&paths)
    }

    fn set_current(&self, version: &RuntimeVersion) -> EnvrResult<()> {
        self.manager()?.set_current(version)
    }

    fn list_remote(&self, filter: &RemoteFilter) -> EnvrResult<Vec<RuntimeVersion>> {
        let releases = self.load_releases()?;
        list_remote_versions(
            &releases,
            std::env::consts::OS,
            std::env::consts::ARCH,
            filter,
        )
    }

    fn resolve(&self, spec: &VersionSpec) -> EnvrResult<ResolvedVersion> {
        let releases = self.load_releases()?;
        let v = resolve_node_version(
            &releases,
            std::env::consts::OS,
            std::env::consts::ARCH,
            &spec.0,
        )?;
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
