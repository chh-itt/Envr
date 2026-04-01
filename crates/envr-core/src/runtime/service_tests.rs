use super::service::RuntimeService;
use envr_domain::runtime::RuntimeKind;

#[test]
fn defaults_providers_registered() {
    let svc = RuntimeService::with_defaults().expect("defaults");
    // Smoke: default stack registers Node/Python/Java. Avoid remote indexes in CI (covered in runtime crates).
    for kind in [RuntimeKind::Node, RuntimeKind::Python, RuntimeKind::Java] {
        let _ = svc.list_installed(kind).expect("list_installed");
    }
}

#[test]
fn with_runtime_root_registers_providers() {
    let dir = tempfile::tempdir().expect("tempdir");
    let svc = RuntimeService::with_runtime_root(dir.path().to_path_buf()).expect("svc");
    for kind in [RuntimeKind::Node, RuntimeKind::Python, RuntimeKind::Java] {
        let _ = svc.list_installed(kind).expect("list_installed");
    }
}
