//! `envr diagnostics export` — zip bundle for bug reports (doctor JSON, system/env summary, recent logs).

use crate::cli::{DiagnosticsCmd, GlobalArgs, OutputFormat};
use crate::commands::common;
use crate::commands::doctor::{self, DoctorReport};
use crate::output;

use envr_core::logging::resolve_log_dir;
use envr_core::runtime::service::RuntimeService;
use envr_error::EnvrError;
use serde_json::json;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use zip::ZipWriter;
use zip::write::FileOptions;

const MAX_LOG_FILES: usize = 8;
const MAX_LOG_BYTES_PER_FILE: usize = 512 * 1024;

pub fn run(g: &GlobalArgs, service: &RuntimeService, cmd: DiagnosticsCmd) -> i32 {
    match cmd {
        DiagnosticsCmd::Export { output } => export_zip(g, service, output),
    }
}

fn export_zip(g: &GlobalArgs, service: &RuntimeService, output: Option<PathBuf>) -> i32 {
    let report = match doctor::build_doctor_report(service) {
        Ok(r) => r,
        Err(e) => return common::print_envr_error(g, e),
    };

    let zip_path = match output {
        Some(p) => p,
        None => {
            let secs = SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            std::env::current_dir()
                .map(|cwd| cwd.join(format!("envr-diagnostics-{secs}.zip")))
                .unwrap_or_else(|_| PathBuf::from(format!("envr-diagnostics-{secs}.zip")))
        }
    };

    if let Some(parent) = zip_path.parent()
        && let Err(e) = fs::create_dir_all(parent)
    {
        return emit_export_error(g, EnvrError::from(e), zip_path.as_path());
    }

    let doctor_json = match serde_json::to_string_pretty(&report.to_json()) {
        Ok(s) => s,
        Err(e) => {
            return emit_export_error(
                g,
                EnvrError::Runtime(format!("serialize doctor: {e}")),
                zip_path.as_path(),
            );
        }
    };

    let system_txt = build_system_txt();
    let environment_txt = build_environment_txt(&report);

    match write_diagnostic_zip(&zip_path, &doctor_json, &system_txt, &environment_txt) {
        Ok(()) => emit_export_ok(g, &zip_path),
        Err(e) => emit_export_error(g, e, zip_path.as_path()),
    }
}

fn emit_export_ok(g: &GlobalArgs, zip_path: &Path) -> i32 {
    let path_str = zip_path.display().to_string();
    let data = json!({ "path": path_str });
    output::emit_ok(g, "diagnostics_export_ok", data, || {
        println!("wrote diagnostics bundle: {}", zip_path.display());
    })
}

fn emit_export_error(g: &GlobalArgs, err: EnvrError, zip_path: &Path) -> i32 {
    let path_str = zip_path.display().to_string();
    match g.output_format.unwrap_or(OutputFormat::Text) {
        OutputFormat::Json => {
            let msg = format!("diagnostics export failed: {err}");
            output::write_envelope(
                false,
                Some("diagnostics_export_failed"),
                &msg,
                json!({ "path": path_str }),
                &[err.to_string()],
            );
        }
        OutputFormat::Text => {
            eprintln!("envr: failed to write {}: {err}", zip_path.display());
        }
    }
    output::exit_code_for_error(&err)
}

fn build_system_txt() -> String {
    format!(
        "cli_version: {}\ncompile_target: {}\nos: {}\narch: {}\n",
        env!("CARGO_PKG_VERSION"),
        option_env!("TARGET").unwrap_or("(unknown)"),
        std::env::consts::OS,
        std::env::consts::ARCH,
    )
}

/// Safe keys: show values. Other `ENVR_*`: key only (values may contain secrets).
fn build_environment_txt(report: &DoctorReport) -> String {
    let mut out = String::from("# envr diagnostics — selected environment\n\n");
    out.push_str("## ENVR_* (values redacted except allowlist)\n\n");
    let mut keys: Vec<String> = std::env::vars()
        .map(|(k, _)| k)
        .filter(|k| k.starts_with("ENVR_"))
        .collect();
    keys.sort();
    keys.dedup();

    let allow_value = |k: &str| {
        matches!(
            k,
            "ENVR_RUNTIME_ROOT" | "ENVR_LOG_DIR" | "ENVR_PROFILE" | "ENVR_OUTPUT_FORMAT"
        )
    };

    for k in &keys {
        if allow_value(k) {
            let v = std::env::var(k).unwrap_or_default();
            out.push_str(&format!("{k}={v}\n"));
        } else {
            out.push_str(&format!("{k}=<redacted>\n"));
        }
    }

    out.push_str("\n## Resolved runtime root (from doctor)\n\n");
    out.push_str(&format!("{}\n", report.root.display()));
    if let Some(ref o) = report.env_override {
        out.push_str(&format!("\nENVR_RUNTIME_ROOT (raw env): {o}\n"));
    }
    out
}

fn write_diagnostic_zip(
    zip_path: &Path,
    doctor_json: &str,
    system_txt: &str,
    environment_txt: &str,
) -> Result<(), EnvrError> {
    let file = File::create(zip_path).map_err(EnvrError::from)?;
    let mut zip = ZipWriter::new(file);
    let opts: FileOptions<'_, ()> = FileOptions::default();

    zip.start_file("doctor.json", opts)
        .map_err(|e| EnvrError::Runtime(format!("zip doctor.json: {e}")))?;
    zip.write_all(doctor_json.as_bytes())
        .map_err(EnvrError::from)?;

    zip.start_file("system.txt", opts)
        .map_err(|e| EnvrError::Runtime(format!("zip system.txt: {e}")))?;
    zip.write_all(system_txt.as_bytes())
        .map_err(EnvrError::from)?;

    zip.start_file("environment.txt", opts)
        .map_err(|e| EnvrError::Runtime(format!("zip environment.txt: {e}")))?;
    zip.write_all(environment_txt.as_bytes())
        .map_err(EnvrError::from)?;

    append_recent_logs(&mut zip, opts)?;

    zip.finish()
        .map_err(|e| EnvrError::Runtime(format!("zip finish: {e}")))?;
    Ok(())
}

fn append_recent_logs(
    zip: &mut ZipWriter<File>,
    opts: FileOptions<'_, ()>,
) -> Result<(), EnvrError> {
    let log_dir = match resolve_log_dir() {
        Ok(p) => p,
        Err(_) => return Ok(()),
    };

    if !log_dir.is_dir() {
        return Ok(());
    }

    let mut entries: Vec<(PathBuf, SystemTime)> = Vec::new();
    let read_dir = match fs::read_dir(&log_dir) {
        Ok(d) => d,
        Err(_) => return Ok(()),
    };

    for ent in read_dir.flatten() {
        let path = ent.path();
        if path.extension().is_some_and(|e| e == "log") {
            let meta = ent.metadata().ok();
            let mtime = meta
                .and_then(|m| m.modified().ok())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            entries.push((path, mtime));
        }
    }

    entries.sort_by_key(|(_, t)| std::cmp::Reverse(*t));
    entries.truncate(MAX_LOG_FILES);

    for (path, _) in entries {
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if name.contains('/') || name.contains('\\') || name.contains("..") {
            continue;
        }
        let zip_name = format!("logs/{name}");
        let mut body = Vec::new();
        let f = match File::open(&path) {
            Ok(f) => f,
            Err(_) => continue,
        };
        let mut limited = f.take(MAX_LOG_BYTES_PER_FILE as u64);
        limited.read_to_end(&mut body).map_err(EnvrError::from)?;

        zip.start_file(zip_name, opts)
            .map_err(|e| EnvrError::Runtime(format!("zip log {name}: {e}")))?;
        zip.write_all(&body).map_err(EnvrError::from)?;
    }

    Ok(())
}
