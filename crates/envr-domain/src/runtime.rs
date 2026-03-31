use envr_error::{EnvrError, EnvrResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RuntimeKind {
    Node,
    Python,
    Java,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionSpec(pub String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeVersion(pub String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallRequest {
    pub spec: VersionSpec,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteFilter {
    pub prefix: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedVersion {
    pub version: RuntimeVersion,
}

pub trait RuntimeProvider: Send + Sync {
    fn kind(&self) -> RuntimeKind;

    fn list_installed(&self) -> EnvrResult<Vec<RuntimeVersion>>;
    fn current(&self) -> EnvrResult<Option<RuntimeVersion>>;
    fn set_current(&self, version: &RuntimeVersion) -> EnvrResult<()>;

    fn list_remote(&self, filter: &RemoteFilter) -> EnvrResult<Vec<RuntimeVersion>>;
    fn resolve(&self, spec: &VersionSpec) -> EnvrResult<ResolvedVersion>;

    fn install(&self, request: &InstallRequest) -> EnvrResult<RuntimeVersion>;
    fn uninstall(&self, version: &RuntimeVersion) -> EnvrResult<()>;
}

pub fn parse_runtime_kind(s: &str) -> EnvrResult<RuntimeKind> {
    match s.to_ascii_lowercase().as_str() {
        "node" => Ok(RuntimeKind::Node),
        "python" => Ok(RuntimeKind::Python),
        "java" => Ok(RuntimeKind::Java),
        _ => Err(EnvrError::Validation(format!("unknown runtime kind: {s}"))),
    }
}
