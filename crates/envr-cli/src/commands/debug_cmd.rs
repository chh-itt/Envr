//! `envr debug` — quick introspection for bug reports.

use crate::cli::GlobalArgs;
use crate::commands::common;
use crate::output;

use envr_config::settings::{settings_path_from_platform, validate_settings_file};
use envr_error::EnvrResult;
use envr_platform::paths::current_platform_paths;
use serde_json::json;

fn summarize_dir(path: &std::path::Path, max_entries: usize) -> Vec<String> {
    let Ok(rd) = std::fs::read_dir(path) else {
        return Vec::new();
    };
    let mut names: Vec<String> = rd
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names.truncate(max_entries);
    names
}

/// Body for [`crate::commands::dispatch`]; errors are finished at the dispatch boundary.
pub(crate) fn info_inner(g: &GlobalArgs) -> EnvrResult<i32> {
    let mut envr_vars: Vec<(String, String)> = std::env::vars()
        .filter(|(k, _)| k.starts_with("ENVR_"))
        .collect();
    envr_vars.sort_by(|a, b| a.0.cmp(&b.0));

    let platform = current_platform_paths().ok();
    let settings_path = platform
        .as_ref()
        .map(settings_path_from_platform);
    let settings_ok = settings_path
        .as_ref()
        .map(|p| validate_settings_file(p).is_ok());

    let runtime_root = common::session_runtime_root().ok();
    let runtime_children = runtime_root
        .as_ref()
        .map(|r| summarize_dir(r, 40))
        .unwrap_or_default();

    let cwd = std::env::current_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "(unknown)".into());

    let log_dir = envr_core::logging::resolve_log_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "(unknown)".into());

    let data = json!({
        "cwd": cwd,
        "settings_path": settings_path.as_ref().map(|p| p.display().to_string()),
        "settings_valid": settings_ok,
        "runtime_root": runtime_root.as_ref().map(|p| p.display().to_string()),
        "runtime_root_children_sample": runtime_children,
        "envr_env": envr_vars.iter().map(|(k,v)| json!({"key": k, "value": v})).collect::<Vec<_>>(),
        "rust_log": std::env::var("RUST_LOG").unwrap_or_default(),
        "log_dir": log_dir,
    });

    Ok(output::emit_ok(g, "debug_info", data, || {
        if g.quiet {
            return;
        }
        println!("cwd: {cwd}");
        if let Some(ref sp) = settings_path {
            println!(
                "settings.toml: {}",
                sp.display(),
            );
            match settings_ok {
                Some(true) => println!("  (validate: ok)"),
                Some(false) => println!("  (validate: FAILED — run `envr config validate`)"),
                None => println!("  (validate: skipped)"),
            }
        }
        if let Some(ref root) = runtime_root {
            println!("runtime root: {}", root.display());
            if !runtime_children.is_empty() {
                println!("  top entries: {}", runtime_children.join(", "));
            }
        }
        println!("log dir: {log_dir}");
        let rl = std::env::var("RUST_LOG").unwrap_or_default();
        if !rl.is_empty() {
            println!("RUST_LOG: {rl}");
        }
        println!("ENVR_* ({}):", envr_vars.len());
        for (k, v) in &envr_vars {
            println!("  {k}={v}");
        }
    }))
}
