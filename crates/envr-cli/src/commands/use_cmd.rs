use crate::cli::GlobalArgs;
use crate::commands::common::{self, kind_label};
use crate::output::{self, fmt_template};

use envr_core::runtime::service::RuntimeService;
use envr_domain::runtime::{RuntimeKind, RuntimeVersion, VersionSpec, parse_runtime_kind};

pub fn run(
    g: &GlobalArgs,
    service: &RuntimeService,
    runtime: String,
    runtime_version: String,
) -> i32 {
    let kind = match parse_runtime_kind(runtime.trim()) {
        Ok(k) => k,
        Err(e) => return common::print_envr_error(g, e),
    };

    let spec = VersionSpec(runtime_version.clone());
    let resolved = match service.resolve(kind, &spec) {
        Ok(r) => r,
        Err(e) => return common::print_envr_error(g, e),
    };

    match service.set_current(kind, &resolved.version) {
        Ok(()) => print_success(g, kind, &resolved.version),
        Err(e) => common::print_envr_error(g, enrich_not_installed_error(e, kind, &resolved.version.0)),
    }
}

fn enrich_not_installed_error(err: envr_error::EnvrError, kind: RuntimeKind, version: &str) -> envr_error::EnvrError {
    let msg = err.to_string().to_ascii_lowercase();
    if msg.contains("not installed") {
        return envr_error::EnvrError::Validation(fmt_template(
            &envr_core::i18n::tr_key(
                "cli.use.not_installed_suggestion",
                "{kind} {version} 未安装。可先执行：envr install {kind} {version}",
                "{kind} {version} is not installed. Try: envr install {kind} {version}",
            ),
            &[
                ("kind", kind_label(kind)),
                ("version", version),
            ],
        ));
    }
    err
}

fn print_success(g: &GlobalArgs, kind: RuntimeKind, v: &RuntimeVersion) -> i32 {
    let data = serde_json::json!({
        "kind": kind_label(kind),
        "version": v.0,
    });
    output::emit_ok(g, "current_runtime_set", data, || {
        if !g.quiet {
            println!(
                "{}",
                fmt_template(
                    &envr_core::i18n::tr_key(
                        "cli.use.ok",
                        "已将 {kind} 的 current 设为 {version}",
                        "{kind} current set to {version}",
                    ),
                    &[("kind", kind_label(kind)), ("version", &v.0)],
                )
            );
        }
    })
}
