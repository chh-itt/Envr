use envr_domain::runtime::{
    InstallRequest, RemoteFilter, ResolvedVersion, RuntimeKind, RuntimeProvider, RuntimeVersion,
    VersionSpec,
};
use envr_error::{EnvrError, EnvrResult};
use std::collections::HashMap;
use std::path::PathBuf;

pub struct RuntimeService {
    providers: HashMap<RuntimeKind, Box<dyn RuntimeProvider>>,
}

impl RuntimeService {
    pub fn new(providers: Vec<Box<dyn RuntimeProvider>>) -> EnvrResult<Self> {
        let mut map = HashMap::new();
        for p in providers {
            if map.contains_key(&p.kind()) {
                return Err(EnvrError::Validation(format!(
                    "duplicate provider for {:?}",
                    p.kind()
                )));
            }
            map.insert(p.kind(), p);
        }
        Ok(Self { providers: map })
    }

    pub fn with_defaults() -> EnvrResult<Self> {
        Self::new(vec![
            Box::new(envr_runtime_node::NodeRuntimeProvider::new()),
            Box::new(envr_runtime_python::PythonRuntimeProvider::new()),
            Box::new(envr_runtime_java::JavaRuntimeProvider::new()),
            Box::new(envr_runtime_go::GoRuntimeProvider::new()),
            Box::new(envr_runtime_rust::RustRuntimeProvider::new()),
            Box::new(envr_runtime_ruby::RubyRuntimeProvider::new()),
            Box::new(envr_runtime_php::PhpRuntimeProvider::new()),
            Box::new(envr_runtime_deno::DenoRuntimeProvider::new()),
            Box::new(envr_runtime_bun::BunRuntimeProvider::new()),
            Box::new(envr_runtime_dotnet::DotnetRuntimeProvider::new()),
        ])
    }

    /// Same as [`Self::with_defaults`], but all providers use this runtime root (e.g. from `ENVR_RUNTIME_ROOT`).
    pub fn with_runtime_root(root: PathBuf) -> EnvrResult<Self> {
        Self::new(vec![
            Box::new(envr_runtime_node::NodeRuntimeProvider::new().with_runtime_root(root.clone())),
            Box::new(
                envr_runtime_python::PythonRuntimeProvider::new().with_runtime_root(root.clone()),
            ),
            Box::new(envr_runtime_java::JavaRuntimeProvider::new().with_runtime_root(root.clone())),
            Box::new(envr_runtime_go::GoRuntimeProvider::new().with_runtime_root(root.clone())),
            Box::new(envr_runtime_rust::RustRuntimeProvider::new().with_runtime_root(root.clone())),
            Box::new(envr_runtime_ruby::RubyRuntimeProvider::new().with_runtime_root(root.clone())),
            Box::new(envr_runtime_php::PhpRuntimeProvider::new().with_runtime_root(root.clone())),
            Box::new(envr_runtime_deno::DenoRuntimeProvider::new().with_runtime_root(root.clone())),
            Box::new(envr_runtime_bun::BunRuntimeProvider::new().with_runtime_root(root.clone())),
            Box::new(envr_runtime_dotnet::DotnetRuntimeProvider::new().with_runtime_root(root)),
        ])
    }

    fn provider(&self, kind: RuntimeKind) -> EnvrResult<&dyn RuntimeProvider> {
        self.providers
            .get(&kind)
            .map(|b| b.as_ref())
            .ok_or_else(|| EnvrError::Validation(format!("provider not registered: {kind:?}")))
    }

    pub fn list_installed(&self, kind: RuntimeKind) -> EnvrResult<Vec<RuntimeVersion>> {
        self.provider(kind)?.list_installed()
    }

    pub fn list_remote(
        &self,
        kind: RuntimeKind,
        filter: &RemoteFilter,
    ) -> EnvrResult<Vec<RuntimeVersion>> {
        self.provider(kind)?.list_remote(filter)
    }

    pub fn list_remote_majors(&self, kind: RuntimeKind) -> EnvrResult<Vec<String>> {
        self.provider(kind)?.list_remote_majors()
    }

    pub fn list_remote_latest_per_major(
        &self,
        kind: RuntimeKind,
    ) -> EnvrResult<Vec<RuntimeVersion>> {
        self.provider(kind)?.list_remote_latest_per_major()
    }

    pub fn try_load_remote_latest_per_major_from_disk(
        &self,
        kind: RuntimeKind,
    ) -> Vec<RuntimeVersion> {
        self.providers
            .get(&kind)
            .map(|p| p.try_load_remote_latest_per_major_from_disk())
            .unwrap_or_default()
    }

    pub fn resolve(&self, kind: RuntimeKind, spec: &VersionSpec) -> EnvrResult<ResolvedVersion> {
        self.provider(kind)?.resolve(spec)
    }

    pub fn install(
        &self,
        kind: RuntimeKind,
        request: &InstallRequest,
    ) -> EnvrResult<RuntimeVersion> {
        self.provider(kind)?.install(request)
    }

    pub fn uninstall(&self, kind: RuntimeKind, version: &RuntimeVersion) -> EnvrResult<()> {
        self.provider(kind)?.uninstall(version)
    }

    pub fn uninstall_dry_run_targets(
        &self,
        kind: RuntimeKind,
        version: &RuntimeVersion,
    ) -> EnvrResult<(Vec<PathBuf>, Option<String>)> {
        self.provider(kind)?.uninstall_dry_run_targets(version)
    }

    pub fn current(&self, kind: RuntimeKind) -> EnvrResult<Option<RuntimeVersion>> {
        self.provider(kind)?.current()
    }

    /// `Some(true)` = global active PHP is TS, `Some(false)` = NTS/legacy; `None` = no global current.
    pub fn php_global_current_want_ts(&self) -> EnvrResult<Option<bool>> {
        let root = envr_config::settings::resolve_runtime_root()?;
        let paths = envr_runtime_php::PhpPaths::new(root);
        envr_runtime_php::read_current_global_want_ts(&paths)
    }

    pub fn set_current(&self, kind: RuntimeKind, version: &RuntimeVersion) -> EnvrResult<()> {
        self.provider(kind)?.set_current(version)
    }
}
