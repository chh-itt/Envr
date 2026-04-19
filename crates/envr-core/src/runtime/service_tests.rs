use super::service::RuntimeService;
use envr_domain::runtime::{
    InstallRequest, RemoteFilter, ResolvedVersion, RuntimeKind, RuntimeProvider, RuntimeVersion,
    VersionSpec,
};
use envr_error::{EnvrError, EnvrResult};
use std::path::PathBuf;

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
        Ok(vec![
            RuntimeVersion("28.4.2".to_string()),
            RuntimeVersion("27.3.4.10".to_string()),
            RuntimeVersion("27.3.4.8".to_string()),
        ])
    }

    fn list_remote_installable(&self, _filter: &RemoteFilter) -> EnvrResult<Vec<RuntimeVersion>> {
        self.list_remote(&RemoteFilter { prefix: None })
    }

    fn list_remote_latest_per_major(&self) -> EnvrResult<Vec<RuntimeVersion>> {
        Ok(vec![
            RuntimeVersion("28.4.2".to_string()),
            RuntimeVersion("27.3.4.10".to_string()),
        ])
    }

    fn list_remote_latest_installable_per_major(&self) -> EnvrResult<Vec<RuntimeVersion>> {
        self.list_remote_latest_per_major()
    }

    fn try_load_remote_latest_installable_per_major_from_disk(&self) -> Vec<RuntimeVersion> {
        vec![RuntimeVersion("26.2.5.19".to_string())]
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

    fn uninstall_dry_run_targets(
        &self,
        _version: &RuntimeVersion,
    ) -> EnvrResult<(Vec<PathBuf>, Option<String>)> {
        Ok((vec![], None))
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
    let tmp = tempfile::tempdir().expect("tmp");
    let svc = RuntimeService::new_with_cache_root_for_tests(
        vec![Box::new(StubProvider {
            kind: RuntimeKind::Go,
        })],
        tmp.path().to_path_buf(),
    )
    .expect("svc");

    assert_eq!(
        svc.list_installed(RuntimeKind::Go).expect("list"),
        vec![RuntimeVersion("1.0.0".to_string())]
    );
    assert_eq!(svc.current(RuntimeKind::Go).expect("current"), None);
    svc.set_current(RuntimeKind::Go, &RuntimeVersion("1.0.0".to_string()))
        .expect("set_current");
    assert_eq!(
        svc.list_remote(RuntimeKind::Go, &RemoteFilter { prefix: None })
            .expect("remote"),
        vec![
            RuntimeVersion("28.4.2".to_string()),
            RuntimeVersion("27.3.4.10".to_string()),
            RuntimeVersion("27.3.4.8".to_string()),
        ]
    );
    assert_eq!(
        svc.list_remote_latest_per_major(RuntimeKind::Go)
            .expect("latest"),
        vec![
            RuntimeVersion("28.4.2".to_string()),
            RuntimeVersion("27.3.4.10".to_string()),
        ]
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

    let disk_major_rows = svc
        .list_major_rows_cached(RuntimeKind::Go)
        .expect("major rows cached");
    assert_eq!(disk_major_rows.len(), 1);
    assert_eq!(disk_major_rows[0].major_key, "26.2");
    assert_eq!(
        disk_major_rows[0]
            .latest_installable
            .as_ref()
            .expect("latest")
            .0,
        "26.2.5.19"
    );

    let refreshed_major_rows = svc
        .refresh_major_rows_remote(RuntimeKind::Go)
        .expect("major rows refreshed");
    assert_eq!(refreshed_major_rows.len(), 2);
    assert!(refreshed_major_rows.iter().any(|r| r.major_key == "28.4"));
    assert!(refreshed_major_rows.iter().any(|r| r.major_key == "27.3"));

    let cached_after_refresh = svc
        .list_major_rows_cached(RuntimeKind::Go)
        .expect("major rows cached after refresh");
    assert_eq!(cached_after_refresh.len(), 2);

    let children_before = svc
        .list_children_cached(RuntimeKind::Go, "27.3")
        .expect("children before");
    assert!(children_before.is_empty());

    let refreshed_children = svc
        .refresh_children_remote(RuntimeKind::Go, "27.3")
        .expect("children refresh");
    assert_eq!(refreshed_children.len(), 2);
    assert!(
        refreshed_children
            .iter()
            .all(|v| v.version.0.starts_with("27."))
    );

    let cached_children = svc
        .list_children_cached(RuntimeKind::Go, "27.3")
        .expect("children cached");
    assert_eq!(cached_children.len(), 2);

    let children_from_full_only = svc
        .list_children_cached(RuntimeKind::Go, "28.4")
        .expect("children from full snapshot without per-major file");
    assert_eq!(children_from_full_only.len(), 1);
    assert_eq!(children_from_full_only[0].version.0, "28.4.2");

    svc.remove_unified_version_list_cache_dir(RuntimeKind::Go)
        .expect("remove unified cache");
    let empty = svc
        .list_children_cached(RuntimeKind::Go, "27.3")
        .expect("after remove");
    assert!(empty.is_empty());
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
        RuntimeKind::Ruby,
        RuntimeKind::Elixir,
        RuntimeKind::Erlang,
        RuntimeKind::Php,
        RuntimeKind::Deno,
        RuntimeKind::Bun,
        RuntimeKind::Dotnet,
        RuntimeKind::Zig,
        RuntimeKind::Julia,
        RuntimeKind::Nim,
        RuntimeKind::Crystal,
        RuntimeKind::RLang,
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
        RuntimeKind::Ruby,
        RuntimeKind::Elixir,
        RuntimeKind::Erlang,
        RuntimeKind::Php,
        RuntimeKind::Deno,
        RuntimeKind::Bun,
        RuntimeKind::Dotnet,
        RuntimeKind::Zig,
        RuntimeKind::Julia,
        RuntimeKind::Nim,
        RuntimeKind::Crystal,
        RuntimeKind::RLang,
    ] {
        let _ = svc.list_installed(kind).expect("list_installed");
    }
}
