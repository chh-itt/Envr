//! Use-cases for runtime installation group (`install`, `use`, `list`, `current`, `uninstall`).
//!
//! These functions do **not** print or build JSON envelopes. They return domain data that adapters
//! can present via `output` + `presenter`.

use envr_core::runtime::service::RuntimeService;
use envr_domain::runtime::{
    RuntimeKind, RuntimeVersion, VersionSpec, parse_runtime_kind, runtime_descriptor,
};
use envr_error::{EnvrError, EnvrResult};

/// Parse the runtime kind (shared by multiple commands).
#[inline]
pub fn parse_kind(runtime: &str) -> EnvrResult<RuntimeKind> {
    parse_runtime_kind(runtime.trim())
}

/// Resolve a version spec and set it as global `current`.
pub fn set_current(
    service: &RuntimeService,
    kind: RuntimeKind,
    spec: VersionSpec,
) -> EnvrResult<RuntimeVersion> {
    let index = service.index_port(kind)?;
    let resolved = index.resolve(&spec)?;
    let installer = service.installer_port(kind)?;
    installer
        .set_current(&resolved.version)
        .map_err(|e| enrich_not_installed_error(e, kind, &resolved.version.0))?;
    Ok(resolved.version)
}

fn enrich_not_installed_error(err: EnvrError, kind: RuntimeKind, version: &str) -> EnvrError {
    let msg = err.to_string().to_ascii_lowercase();
    if msg.contains("not installed") {
        return EnvrError::Validation(crate::output::fmt_template(
            &envr_core::i18n::tr_key(
                "cli.use.not_installed_suggestion",
                "{kind} {version} 未安装。可先执行：envr install {kind} {version}",
                "{kind} {version} is not installed. Try: envr install {kind} {version}",
            ),
            &[("kind", kind_label(kind)), ("version", version)],
        ));
    }
    err
}

#[inline]
fn kind_label(kind: RuntimeKind) -> &'static str {
    runtime_descriptor(kind).key
}
