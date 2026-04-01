use crate::cli::GlobalArgs;

use envr_core::runtime::service::RuntimeService;
use envr_domain::runtime::RuntimeKind;
use envr_error::EnvrError;
use std::path::PathBuf;

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

/// Data directory for envr runtimes (honours `ENVR_RUNTIME_ROOT`, then platform defaults).
pub(crate) fn effective_runtime_root() -> Result<PathBuf, EnvrError> {
    if let Ok(p) = std::env::var("ENVR_RUNTIME_ROOT")
        && !p.is_empty()
    {
        return Ok(PathBuf::from(p));
    }
    Ok(envr_platform::paths::current_platform_paths()?.runtime_root)
}

pub fn print_envr_error(g: &GlobalArgs, err: EnvrError) -> i32 {
    crate::output::emit_envr_error(g, err)
}

pub fn missing_positional(g: &GlobalArgs, cmd: &str, example: &str) -> i32 {
    crate::output::emit_validation(g, cmd, example)
}
