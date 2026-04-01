use envr_error::{EnvrError, EnvrResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RuntimeKind {
    Node,
    Python,
    Java,
    Go,
    Rust,
    Php,
    Deno,
    Bun,
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
        "go" => Ok(RuntimeKind::Go),
        "rust" => Ok(RuntimeKind::Rust),
        "php" => Ok(RuntimeKind::Php),
        "deno" => Ok(RuntimeKind::Deno),
        "bun" => Ok(RuntimeKind::Bun),
        _ => Err(EnvrError::Validation(format!("unknown runtime kind: {s}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_runtime_kind_accepts_ascii_case() {
        assert_eq!(parse_runtime_kind("NODE").expect("node"), RuntimeKind::Node);
        assert_eq!(
            parse_runtime_kind("Python").expect("py"),
            RuntimeKind::Python
        );
    }

    #[test]
    fn parse_runtime_kind_rejects_unknown() {
        let err = parse_runtime_kind("ruby").expect_err("unknown");
        assert!(matches!(err, EnvrError::Validation(_)));
    }
}
