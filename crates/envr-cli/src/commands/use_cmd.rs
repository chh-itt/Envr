use crate::cli::{GlobalArgs, OutputFormat};
use crate::commands::common::{self, kind_label};

use envr_core::runtime::service::RuntimeService;
use envr_domain::runtime::{RuntimeKind, RuntimeVersion, VersionSpec, parse_runtime_kind};

pub fn run(
    g: &GlobalArgs,
    service: &RuntimeService,
    lang: Option<String>,
    runtime_version: Option<String>,
) -> i32 {
    let Some(lang) = lang else {
        return common::missing_positional(g, "use", "envr use node 20");
    };
    let Some(ver) = runtime_version else {
        return common::missing_positional(g, "use", "envr use node 20");
    };

    let kind = match parse_runtime_kind(lang.trim()) {
        Ok(k) => k,
        Err(e) => return common::print_envr_error(g, e),
    };

    let spec = VersionSpec(ver);
    let resolved = match service.resolve(kind, &spec) {
        Ok(r) => r,
        Err(e) => return common::print_envr_error(g, e),
    };

    match service.set_current(kind, &resolved.version) {
        Ok(()) => print_success(g, kind, &resolved.version),
        Err(e) => common::print_envr_error(g, e),
    }
}

fn print_success(g: &GlobalArgs, kind: RuntimeKind, v: &RuntimeVersion) -> i32 {
    match g.output_format.unwrap_or(OutputFormat::Text) {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::json!({
                    "success": true,
                    "message": "current_runtime_set",
                    "data": {
                        "kind": kind_label(kind),
                        "version": v.0,
                    },
                    "diagnostics": [],
                })
            );
        }
        OutputFormat::Text => {
            if !g.quiet {
                println!("{} current set to {}", kind_label(kind), v.0);
            }
        }
    }
    0
}
