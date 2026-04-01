//! Open [`RuntimeService`] the same way as the CLI (`ENVR_RUNTIME_ROOT` + platform default).

use envr_core::runtime::service::RuntimeService;
use envr_error::EnvrResult;
use std::path::PathBuf;

pub fn open_runtime_service() -> EnvrResult<RuntimeService> {
    RuntimeService::with_runtime_root(runtime_root()?)
}

fn runtime_root() -> EnvrResult<PathBuf> {
    if let Ok(p) = std::env::var("ENVR_RUNTIME_ROOT")
        && !p.is_empty()
    {
        return Ok(PathBuf::from(p));
    }
    Ok(envr_platform::paths::current_platform_paths()?.runtime_root)
}
