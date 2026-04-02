use crate::cli::GlobalArgs;
use crate::commands::common;
use crate::output;

use envr_error::EnvrError;
use envr_shim_core::{
    ShimContext, normalize_invoked_basename, parse_core_command, resolve_core_shim_command,
};

pub fn run(g: &GlobalArgs, name: Option<String>) -> i32 {
    let Some(name) = name else {
        return common::missing_positional(g, "which", "envr which node");
    };

    let base = normalize_invoked_basename(name.trim());
    let Some(cmd) = parse_core_command(&base) else {
        let err = EnvrError::Validation(crate::output::fmt_template(
            &envr_core::i18n::tr_key(
                "cli.err.unknown_tool",
                "未知工具 `{name}`（可试 node、npm、npx、python、pip、java、javac、bun、bunx）",
                "unknown tool `{name}` (try node, npm, npx, python, pip, java, javac, bun, bunx)",
            ),
            &[("name", name.trim())],
        ));
        return common::print_envr_error(g, err);
    };

    let ctx = match ShimContext::from_process_env() {
        Ok(c) => c,
        Err(e) => return common::print_envr_error(g, e),
    };

    match resolve_core_shim_command(cmd, &ctx) {
        Ok(shim) => {
            let data = serde_json::json!({
                "executable": shim.executable.to_string_lossy(),
                "extra_env": shim.extra_env.iter().map(|(k, v)| {
                    serde_json::json!({ "key": k, "value": v })
                }).collect::<Vec<_>>(),
            });
            output::emit_ok(g, "resolved_executable", data, || {
                println!("{}", shim.executable.display());
                for (k, v) in &shim.extra_env {
                    eprintln!("{k}={v}");
                }
            })
        }
        Err(e) => common::print_envr_error(g, e),
    }
}
