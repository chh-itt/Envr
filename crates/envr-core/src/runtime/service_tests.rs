use super::service::RuntimeService;
use envr_domain::runtime::{InstallRequest, RemoteFilter, RuntimeKind, VersionSpec};

#[test]
fn defaults_include_node_python_java() {
    let svc = RuntimeService::with_defaults().expect("defaults");
    let filter = RemoteFilter { prefix: None };

    // Node/Python `list_remote` hit official indexes; covered in envr-runtime-* crate tests.
    let _ = svc.list_remote(RuntimeKind::Java, &filter).expect("java");
}

#[test]
fn install_and_resolve_are_routed() {
    let svc = RuntimeService::with_defaults().expect("defaults");
    let spec = VersionSpec("1.2.3".to_string());

    let resolved = svc.resolve(RuntimeKind::Java, &spec).expect("resolve");
    assert_eq!(resolved.version.0, "1.2.3");

    let installed = svc
        .install(RuntimeKind::Java, &InstallRequest { spec })
        .expect("install");
    assert_eq!(installed.0, "1.2.3");
}
