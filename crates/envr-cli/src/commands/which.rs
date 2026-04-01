use crate::cli::{GlobalArgs, OutputFormat};
use crate::commands::common;

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
        let err = EnvrError::Validation(format!(
            "unknown tool {name:?} (try node, npm, npx, python, pip, java, javac)"
        ));
        return common::print_envr_error(g, err);
    };

    let ctx = match ShimContext::from_process_env() {
        Ok(c) => c,
        Err(e) => return common::print_envr_error(g, e),
    };

    match resolve_core_shim_command(cmd, &ctx) {
        Ok(shim) => {
            match g.output_format.unwrap_or(OutputFormat::Text) {
                OutputFormat::Json => {
                    println!(
                        "{}",
                        serde_json::json!({
                            "success": true,
                            "data": {
                                "executable": shim.executable.to_string_lossy(),
                                "extra_env": shim.extra_env.iter().map(|(k, v)| {
                                    serde_json::json!({ "key": k, "value": v })
                                }).collect::<Vec<_>>(),
                            },
                            "diagnostics": [],
                        })
                    );
                }
                OutputFormat::Text => {
                    println!("{}", shim.executable.display());
                    for (k, v) in &shim.extra_env {
                        eprintln!("{k}={v}");
                    }
                }
            }
            0
        }
        Err(e) => common::print_envr_error(g, e),
    }
}
