use super::service::RuntimeService;
use envr_domain::runtime::{
    InstallRequest, RemoteFilter, ResolvedVersion, RuntimeKind, RuntimeProvider, RuntimeVersion,
    VersionSpec,
};
use envr_error::{EnvrError, EnvrResult};

struct StubProvider {
    kind: RuntimeKind,
}

impl RuntimeProvider for StubProvider {
    fn kind(&self) -> RuntimeKind {
        self.kind
    }

    fn list_installed(&self) -> EnvrResult<Vec<RuntimeVersion>> {
        Ok(vec![RuntimeVersion("1.0.0".to_string())])
    }

    fn current(&self) -> EnvrResult<Option<RuntimeVersion>> {
        Ok(None)
    }

    fn set_current(&self, _version: &RuntimeVersion) -> EnvrResult<()> {
        Ok(())
    }

    fn list_remote(&self, _filter: &RemoteFilter) -> EnvrResult<Vec<RuntimeVersion>> {
        Ok(vec![])
    }

    fn resolve(&self, _spec: &VersionSpec) -> EnvrResult<ResolvedVersion> {
        Ok(ResolvedVersion {
            version: RuntimeVersion("2.0.0".to_string()),
        })
    }

    fn install(&self, _request: &InstallRequest) -> EnvrResult<RuntimeVersion> {
        Ok(RuntimeVersion("3.0.0".to_string()))
    }

    fn uninstall(&self, _version: &RuntimeVersion) -> EnvrResult<()> {
        Ok(())
    }
}

#[test]
fn new_rejects_duplicate_provider_kind() {
    let res = RuntimeService::new(vec![
        Box::new(StubProvider {
            kind: RuntimeKind::Node,
        }),
        Box::new(StubProvider {
            kind: RuntimeKind::Node,
        }),
    ]);
    assert!(matches!(res, Err(EnvrError::Validation(_))));
}

#[test]
fn provider_not_registered_errors() {
    let svc = RuntimeService::new(vec![]).expect("empty map is ok");
    let err = svc
        .list_installed(RuntimeKind::Node)
        .expect_err("no provider");
    assert!(matches!(err, EnvrError::Validation(_)));
}

#[test]
fn passthrough_methods_delegate_to_stub_provider() {
    let svc = RuntimeService::new(vec![Box::new(StubProvider {
        kind: RuntimeKind::Go,
    })])
    .expect("svc");

    assert_eq!(
        svc.list_installed(RuntimeKind::Go).expect("list"),
        vec![RuntimeVersion("1.0.0".to_string())]
    );
    assert_eq!(svc.current(RuntimeKind::Go).expect("current"), None);
    svc.set_current(RuntimeKind::Go, &RuntimeVersion("1.0.0".to_string()))
        .expect("set_current");
    assert!(
        svc.list_remote(RuntimeKind::Go, &RemoteFilter { prefix: None })
            .expect("remote")
            .is_empty()
    );
    assert_eq!(
        svc.resolve(RuntimeKind::Go, &VersionSpec("x".to_string()))
            .expect("resolve")
            .version,
        RuntimeVersion("2.0.0".to_string())
    );
    assert_eq!(
        svc.install(
            RuntimeKind::Go,
            &InstallRequest {
                spec: VersionSpec("latest".to_string()),
                progress_downloaded: None,
                progress_total: None,
                cancel: None,
            },
        )
        .expect("install"),
        RuntimeVersion("3.0.0".to_string())
    );
    svc.uninstall(RuntimeKind::Go, &RuntimeVersion("3.0.0".to_string()))
        .expect("uninstall");
}

#[test]
fn defaults_providers_registered() {
    let svc = RuntimeService::with_defaults().expect("defaults");
    // Smoke: default stack registers Node/Python/Java. Avoid remote indexes in CI (covered in runtime crates).
    for kind in [
        RuntimeKind::Node,
        RuntimeKind::Python,
        RuntimeKind::Java,
        RuntimeKind::Go,
        RuntimeKind::Rust,
        RuntimeKind::Php,
        RuntimeKind::Deno,
        RuntimeKind::Bun,
    ] {
        let _ = svc.list_installed(kind).expect("list_installed");
    }
}

#[test]
fn with_runtime_root_registers_providers() {
    let dir = tempfile::tempdir().expect("tempdir");
    let svc = RuntimeService::with_runtime_root(dir.path().to_path_buf()).expect("svc");
    for kind in [
        RuntimeKind::Node,
        RuntimeKind::Python,
        RuntimeKind::Java,
        RuntimeKind::Go,
        RuntimeKind::Rust,
        RuntimeKind::Php,
        RuntimeKind::Deno,
        RuntimeKind::Bun,
    ] {
        let _ = svc.list_installed(kind).expect("list_installed");
    }
}
