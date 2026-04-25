//! `envr debug` — quick introspection for bug reports.
use crate::CliExit;
use crate::CliUxPolicy;

use crate::cli::GlobalArgs;
use crate::commands::common;
use crate::output;

use envr_config::settings::{settings_path_from_platform, validate_settings_file};
use envr_error::EnvrResult;
use envr_platform::paths::current_platform_paths;
use envr_download::snapshot_download_control_plane_stats;
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
pub(crate) fn info_inner(g: &GlobalArgs) -> EnvrResult<CliExit> {
    let mut envr_vars: Vec<(String, String)> = std::env::vars()
        .filter(|(k, _)| k.starts_with("ENVR_"))
        .collect();
    envr_vars.sort_by(|a, b| a.0.cmp(&b.0));

    let platform = current_platform_paths().ok();
    let settings_path = platform.as_ref().map(settings_path_from_platform);
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
    let dl = snapshot_download_control_plane_stats();

    let data = json!({
        "cwd": cwd,
        "settings_path": settings_path.as_ref().map(|p| p.display().to_string()),
        "settings_valid": settings_ok,
        "runtime_root": runtime_root.as_ref().map(|p| p.display().to_string()),
        "runtime_root_children_sample": runtime_children,
        "envr_env": envr_vars.iter().map(|(k,v)| json!({"key": k, "value": v})).collect::<Vec<_>>(),
        "rust_log": std::env::var("RUST_LOG").unwrap_or_default(),
        "log_dir": log_dir,
        "download_control_plane": {
            "blocking_pool_hits": dl.blocking_pool_hits,
            "blocking_pool_misses": dl.blocking_pool_misses,
            "async_pool_hits": dl.async_pool_hits,
            "async_pool_misses": dl.async_pool_misses,
            "retry_scheduled": dl.retry_scheduled,
            "blocking_queue_wait_events": dl.blocking_queue_wait_events,
            "blocking_queue_wait_total_micros": dl.blocking_queue_wait_total_micros,
            "async_queue_wait_events": dl.async_queue_wait_events,
            "async_queue_wait_total_micros": dl.async_queue_wait_total_micros,
            "blocking_in_flight": dl.blocking_in_flight,
            "blocking_in_flight_peak": dl.blocking_in_flight_peak,
            "async_in_flight": dl.async_in_flight,
            "async_in_flight_peak": dl.async_in_flight_peak,
        },
    });

    Ok(output::emit_ok(
        g,
        crate::codes::ok::DEBUG_INFO,
        data,
        || {
            if !CliUxPolicy::from_global(g).human_text_primary() {
                return;
            }
            println!("cwd: {cwd}");
            if let Some(ref sp) = settings_path {
                println!("settings.toml: {}", sp.display(),);
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
            println!(
                "download cp: pool(b hit/miss={} / {} ; a hit/miss={} / {}), queue_wait(events b/a={} / {}), in_flight(peak b/a={} / {})",
                dl.blocking_pool_hits,
                dl.blocking_pool_misses,
                dl.async_pool_hits,
                dl.async_pool_misses,
                dl.blocking_queue_wait_events,
                dl.async_queue_wait_events,
                dl.blocking_in_flight_peak,
                dl.async_in_flight_peak
            );
            let rl = std::env::var("RUST_LOG").unwrap_or_default();
            if !rl.is_empty() {
                println!("RUST_LOG: {rl}");
            }
            println!("ENVR_* ({}):", envr_vars.len());
            for (k, v) in &envr_vars {
                println!("  {k}={v}");
            }
        },
    ))
}
