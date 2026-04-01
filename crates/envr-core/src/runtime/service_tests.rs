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
