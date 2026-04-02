use crate::cli::GlobalArgs;
use crate::commands::common::{self, kind_label};
use crate::output::{self, fmt_template};

use envr_core::runtime::service::RuntimeService;
use envr_domain::runtime::{
    InstallRequest, RuntimeKind, RuntimeVersion, VersionSpec, parse_runtime_kind,
};

pub fn run(
    g: &GlobalArgs,
    service: &RuntimeService,
    lang: Option<String>,
    runtime_version: Option<String>,
) -> i32 {
    let Some(lang) = lang else {
        return common::missing_positional(g, "install", "envr install node 20");
    };
    let Some(ver) = runtime_version else {
        return common::missing_positional(g, "install", "envr install node 20");
    };

    let kind = match parse_runtime_kind(lang.trim()) {
        Ok(k) => k,
        Err(e) => return common::print_envr_error(g, e),
    };

    let request = InstallRequest {
        spec: VersionSpec(ver),
    };

    match service.install(kind, &request) {
        Ok(v) => print_success(g, kind, &v),
        Err(e) => common::print_envr_error(g, e),
    }
}

fn print_success(g: &GlobalArgs, kind: RuntimeKind, v: &RuntimeVersion) -> i32 {
    let data = serde_json::json!({
        "kind": kind_label(kind),
        "version": v.0,
    });
    output::emit_ok(g, "installed", data, || {
        if !g.quiet {
            println!(
                "{}",
                fmt_template(
                    &envr_core::i18n::tr_key(
                        "cli.install.ok",
                        "已安装 {kind} {version}",
                        "{kind} {version} installed",
                    ),
                    &[("kind", kind_label(kind)), ("version", &v.0)],
                )
            );
        }
    })
}
