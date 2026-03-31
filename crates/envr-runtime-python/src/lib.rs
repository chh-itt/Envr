use envr_domain::runtime::{
    InstallRequest, RemoteFilter, ResolvedVersion, RuntimeKind, RuntimeProvider, RuntimeVersion,
    VersionSpec,
};
use envr_error::{EnvrError, EnvrResult};

pub struct PythonRuntimeProvider;

impl PythonRuntimeProvider {
    pub fn new() -> Self {
        Self
    }
}

impl Default for PythonRuntimeProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl RuntimeProvider for PythonRuntimeProvider {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::Python
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

    fn list_remote(&self, _filter: &RemoteFilter) -> EnvrResult<Vec<RuntimeVersion>> {
        Ok(vec![])
    }

    fn resolve(&self, spec: &VersionSpec) -> EnvrResult<ResolvedVersion> {
        Ok(ResolvedVersion {
            version: RuntimeVersion(spec.0.clone()),
        })
    }

    fn install(&self, request: &InstallRequest) -> EnvrResult<RuntimeVersion> {
        Ok(RuntimeVersion(request.spec.0.clone()))
    }

    fn uninstall(&self, _version: &RuntimeVersion) -> EnvrResult<()> {
        Err(EnvrError::Runtime("not implemented".to_string()))
    }
}
