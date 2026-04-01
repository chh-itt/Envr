use crate::cli::{GlobalArgs, OutputFormat};
use crate::commands::common;
use crate::output;

use envr_config::project_config::load_project_config;
use envr_domain::runtime::parse_runtime_kind;
use envr_error::EnvrError;
use envr_shim_core::{ShimContext, pick_version_home};
use serde_json::json;
use std::path::PathBuf;

pub fn run(g: &GlobalArgs, path: PathBuf) -> i32 {
    let loaded = match load_project_config(&path) {
        Ok(l) => l,
        Err(e) => return common::print_envr_error(g, e),
    };
    let Some((cfg, loc)) = loaded else {
        return common::print_envr_error(
            g,
            EnvrError::Validation(format!(
                "no `.envr.toml` or `.envr.local.toml` found searching upward from {}",
                path.display()
            )),
        );
    };

    let ctx = match ShimContext::from_process_env() {
        Ok(c) => c,
        Err(e) => return common::print_envr_error(g, e),
    };

    let mut problems = Vec::new();
    for (key, rt) in &cfg.runtimes {
        if parse_runtime_kind(key).is_err() {
            problems.push(format!(
                "unknown runtime key `{key}` (expected node, python, or java)"
            ));
            continue;
        }
        if let Some(spec) = &rt.version {
            let vd = ctx.runtime_root.join("runtimes").join(key).join("versions");
            if let Err(e) = pick_version_home(&vd, spec) {
                problems.push(format!("{key}: {e}"));
            }
        }
    }

    if !problems.is_empty() {
        let msg = "project configuration check failed";
        match g.output_format.unwrap_or(OutputFormat::Text) {
            OutputFormat::Json => {
                let data = json!({
                    "config_dir": loc.dir.to_string_lossy(),
                    "issues": problems,
                });
                output::write_envelope(false, Some("project_check_failed"), msg, data, &[]);
            }
            OutputFormat::Text => {
                eprintln!("envr: {msg}");
                for p in &problems {
                    eprintln!("  - {p}");
                }
            }
        }
        return 1;
    }

    let data = serde_json::json!({
        "config_dir": loc.dir.to_string_lossy(),
        "base_file": loc.base_file.as_ref().map(|p| p.to_string_lossy().to_string()),
        "local_file": loc.local_file.as_ref().map(|p| p.to_string_lossy().to_string()),
        "pinned_runtimes": cfg.runtimes.len(),
    });
    output::emit_ok(g, "project_config_ok", data, || {
        if !g.quiet {
            println!("project config ok (root {})", loc.dir.display());
        }
    })
}
