use crate::cli::GlobalArgs;

use envr_config::settings::resolve_runtime_root;
use envr_core::runtime::service::RuntimeService;
use envr_domain::runtime::RuntimeKind;
use envr_error::{EnvrError, EnvrResult};
use envr_shim_core::ShimContext;
use std::path::PathBuf;

/// Resolve the effective runtime root for this process (re-reads `settings.toml` each call so edits
/// in another terminal are picked up on the next `exec` / `run` / `which`, unless `ENVR_RUNTIME_ROOT` is set).
pub fn session_runtime_root() -> EnvrResult<PathBuf> {
    effective_runtime_root()
}

/// [`ShimContext`] for CLI commands: cached `runtime_root`, merged `profile` (`--profile` wins over `ENVR_PROFILE`).
pub fn shim_context_for(path: PathBuf, profile_cli: Option<String>) -> EnvrResult<ShimContext> {
    let runtime_root = session_runtime_root()?;
    let working_dir = std::fs::canonicalize(&path).unwrap_or(path);
    let profile = profile_cli
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| {
            std::env::var("ENVR_PROFILE")
                .ok()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        });
    Ok(ShimContext::with_runtime_root(runtime_root, working_dir, profile))
}

pub fn kind_label(kind: RuntimeKind) -> &'static str {
    match kind {
        RuntimeKind::Node => "node",
        RuntimeKind::Python => "python",
        RuntimeKind::Java => "java",
        RuntimeKind::Go => "go",
        RuntimeKind::Rust => "rust",
        RuntimeKind::Php => "php",
        RuntimeKind::Deno => "deno",
        RuntimeKind::Bun => "bun",
    }
}

pub fn runtime_service() -> Result<RuntimeService, EnvrError> {
    let root = session_runtime_root()?;
    RuntimeService::with_runtime_root(root)
}

/// Run `f` with a resolved [`RuntimeService`], or print an error and return its exit code.
pub fn with_runtime_service<F>(g: &GlobalArgs, f: F) -> i32
where
    F: FnOnce(&RuntimeService) -> i32,
{
    let service = match runtime_service() {
        Ok(s) => s,
        Err(e) => return print_envr_error(g, e),
    };
    f(&service)
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
