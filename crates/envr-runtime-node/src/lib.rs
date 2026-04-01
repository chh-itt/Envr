mod index;

pub use index::{
    DEFAULT_NODE_INDEX_JSON_URL, NodeRelease, blocking_http_client, fetch_node_index,
    list_remote_versions, normalize_node_version, parse_node_index, release_has_platform,
    resolve_node_version,
};

use envr_domain::runtime::{
    InstallRequest, RemoteFilter, ResolvedVersion, RuntimeKind, RuntimeProvider, RuntimeVersion,
    VersionSpec,
};
use envr_error::{EnvrError, EnvrResult};

/// Node.js runtime provider (remote index + resolution in [`index`]).
pub struct NodeRuntimeProvider {
    index_json_url: String,
}

impl NodeRuntimeProvider {
    pub fn new() -> Self {
        Self {
            index_json_url: DEFAULT_NODE_INDEX_JSON_URL.to_string(),
        }
    }

    pub fn with_index_json_url(url: impl Into<String>) -> Self {
        Self {
            index_json_url: url.into(),
        }
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
        Ok(vec![])
    }

    fn current(&self) -> EnvrResult<Option<RuntimeVersion>> {
        Ok(None)
    }

    fn set_current(&self, _version: &RuntimeVersion) -> EnvrResult<()> {
        Err(EnvrError::Runtime("not implemented".to_string()))
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
        Ok(self.resolve(&request.spec)?.version)
    }

    fn uninstall(&self, _version: &RuntimeVersion) -> EnvrResult<()> {
        Err(EnvrError::Runtime("not implemented".to_string()))
    }
}
