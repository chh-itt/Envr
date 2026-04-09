mod installer;
mod manager;

pub use installer::{RustChannel, install_rustup_managed};
pub use manager::{RustManager, RustPaths, RustupMode};

use envr_domain::runtime::{
    InstallRequest, RemoteFilter, ResolvedVersion, RuntimeKind, RuntimeProvider, RuntimeVersion,
    VersionSpec,
};
use envr_error::{EnvrError, EnvrResult};
use envr_platform::paths::current_platform_paths;

pub struct RustRuntimeProvider {
    runtime_root_override: Option<std::path::PathBuf>,
}

impl RustRuntimeProvider {
    pub fn new() -> Self {
        Self {
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

    fn manager(&self) -> EnvrResult<RustManager> {
        RustManager::try_new(self.runtime_root()?)
    }
}

impl Default for RustRuntimeProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeProvider for RustRuntimeProvider {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::Rust
    }

    fn list_installed(&self) -> EnvrResult<Vec<RuntimeVersion>> {
        self.manager()?.list_installed_toolchains()
    }

    fn current(&self) -> EnvrResult<Option<RuntimeVersion>> {
        self.manager()?.active_toolchain()
    }

    fn set_current(&self, version: &RuntimeVersion) -> EnvrResult<()> {
        self.manager()?.set_default(version)
    }

    fn list_remote(&self, _filter: &RemoteFilter) -> EnvrResult<Vec<RuntimeVersion>> {
        Err(EnvrError::Validation(
            "rust remote listing is not supported; use `rustup toolchain list`".into(),
        ))
    }

    fn resolve(&self, spec: &VersionSpec) -> EnvrResult<ResolvedVersion> {
        let s = spec.0.trim();
        if s.is_empty() {
            return Err(EnvrError::Validation("empty rust toolchain spec".into()));
        }
        let resolved = match s.to_ascii_lowercase().as_str() {
            "latest" => "stable".to_string(),
            _ => s.to_string(),
        };
        Ok(ResolvedVersion {
            version: RuntimeVersion(resolved),
        })
    }

    fn install(&self, request: &InstallRequest) -> EnvrResult<RuntimeVersion> {
        self.manager()?.install_toolchain(&request.spec)
    }

    fn uninstall(&self, version: &RuntimeVersion) -> EnvrResult<()> {
        self.manager()?.uninstall_toolchain(version)
    }
}
