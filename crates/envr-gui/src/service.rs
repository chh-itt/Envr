//! Open [`RuntimeService`] the same way as the CLI (`ENVR_RUNTIME_ROOT` + platform default).

use envr_core::runtime::service::RuntimeService;
use envr_error::EnvrResult;
use std::path::PathBuf;

pub fn open_runtime_service() -> EnvrResult<RuntimeService> {
    RuntimeService::with_runtime_root(runtime_root()?)
}

fn runtime_root() -> EnvrResult<PathBuf> {
    envr_config::settings::resolve_runtime_root()
}
