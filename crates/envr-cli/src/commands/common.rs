use crate::cli::GlobalArgs;

use envr_config::settings::resolve_runtime_root;
use envr_core::runtime::service::RuntimeService;
use envr_domain::runtime::RuntimeKind;
use envr_error::EnvrError;

pub fn kind_label(kind: RuntimeKind) -> &'static str {
    match kind {
        RuntimeKind::Node => "node",
        RuntimeKind::Python => "python",
        RuntimeKind::Java => "java",
    }
}

pub fn runtime_service() -> Result<RuntimeService, EnvrError> {
    let root = effective_runtime_root()?;
    RuntimeService::with_runtime_root(root)
}

/// Data directory for envr runtimes (`ENVR_RUNTIME_ROOT`, then `settings.toml`, then platform default).
pub(crate) fn effective_runtime_root() -> Result<std::path::PathBuf, EnvrError> {
    resolve_runtime_root()
}

pub fn print_envr_error(g: &GlobalArgs, err: EnvrError) -> i32 {
    crate::output::emit_envr_error(g, err)
}

pub fn missing_positional(g: &GlobalArgs, cmd: &str, example: &str) -> i32 {
    crate::output::emit_validation(g, cmd, example)
}
