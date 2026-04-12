use crate::cli::GlobalArgs;
use crate::commands::cli_install_progress;
use crate::commands::common::{self, kind_label};
use crate::output::{self, fmt_template};

use envr_core::runtime::service::RuntimeService;
use envr_domain::runtime::{
    RuntimeKind, RuntimeVersion, VersionSpec, parse_runtime_kind,
};

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

    let rv = runtime_version.trim().to_string();
    let headline = fmt_template(
        &envr_core::i18n::tr_key(
            "cli.install.downloading",
            "正在下载 {kind} {version}…",
            "Downloading {kind} {version}…",
        ),
        &[
            ("kind", kind_label(kind)),
            ("version", rv.as_str()),
        ],
    );
    let spec = VersionSpec(rv);
    let (request, guard) =
        cli_install_progress::install_request_with_progress(g, spec, headline);
    let res = service.install(kind, &request);
    guard.finish();
    match res {
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
