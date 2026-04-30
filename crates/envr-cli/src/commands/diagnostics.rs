//! `envr diagnostics export` — zip bundle for bug reports (doctor JSON, system/env summary, recent logs).
use crate::CliExit;

use crate::cli::GlobalArgs;
use crate::commands::doctor::DoctorReport;
use crate::commands::doctor_analyzer;
use crate::output;

use envr_core::logging::resolve_log_dir;
use envr_core::runtime::service::RuntimeService;
use envr_error::{EnvrError, EnvrResult, ErrorCode};
use serde_json::json;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use zip::ZipWriter;
use zip::write::FileOptions;

const MAX_LOG_FILES: usize = 8;
const MAX_LOG_BYTES_PER_FILE: usize = 512 * 1024;

/// Body for [`crate::commands::dispatch`]; errors are finished at the dispatch boundary.
pub(crate) fn export_zip_inner(
    g: &GlobalArgs,
    service: &RuntimeService,
    output: Option<PathBuf>,
) -> EnvrResult<CliExit> {
    let report = doctor_analyzer::build_doctor_report(service)?;

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
        return Ok(emit_export_error(g, EnvrError::from(e), zip_path.as_path()));
    }

    let doctor_json = match serde_json::to_string_pretty(&report.to_json()) {
        Ok(s) => s,
        Err(e) => {
            return Ok(emit_export_error(
                g,
                EnvrError::with_source(ErrorCode::Runtime, "serialize doctor", e),
                zip_path.as_path(),
            ));
        }
    };

    let system_txt = build_system_txt();
    let environment_txt = build_environment_txt(&report);
    let provider_state_json = build_provider_state_json(&report);

    match write_diagnostic_zip(
        &zip_path,
        &doctor_json,
        &system_txt,
        &environment_txt,
        &provider_state_json,
    ) {
        Ok(()) => Ok(emit_export_ok(g, &zip_path)),
        Err(e) => Ok(emit_export_error(g, e, zip_path.as_path())),
    }
}

fn emit_export_ok(g: &GlobalArgs, zip_path: &Path) -> crate::CliExit {
    let path_str = zip_path.display().to_string();
    let data = json!({ "path": path_str });
    output::emit_ok(g, crate::codes::ok::DIAGNOSTICS_EXPORT_OK, data, || {
        println!(
            "{}",
            output::fmt_template(
                &envr_core::i18n::tr_key(
                    "cli.diagnostics.bundle_wrote",
                    "已写入诊断包：{path}",
                    "wrote diagnostics bundle: {path}",
                ),
                &[("path", &zip_path.display().to_string())],
            )
        );
    })
}

fn emit_export_error(g: &GlobalArgs, err: EnvrError, zip_path: &Path) -> crate::CliExit {
    let path_str = zip_path.display().to_string();
    let msg = output::fmt_template(
        &envr_core::i18n::tr_key(
            "cli.diagnostics.write_failed",
            "写入 {path} 失败：{detail}",
            "failed to write {path}: {detail}",
        ),
        &[
            ("path", &zip_path.display().to_string()),
            ("detail", &err.to_string()),
        ],
    );
    let data = json!({ "path": path_str });
    let diags = vec![err.to_string()];
    let code = output::exit_code_for_error(&err);
    output::emit_failure_envelope(
        g,
        crate::codes::err::DIAGNOSTICS_EXPORT_FAILED,
        &msg,
        data,
        &diags,
        code,
    )
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

fn build_provider_state_json(report: &DoctorReport) -> String {
    let state = json!({
        "runtime_root": report.root.to_string_lossy(),
        "path_shadowing": report.path_shadowing.as_ref().map(|p| json!({
            "tool": p.tool,
            "executable": p.executable,
            "first_path_directory": p.first_path_directory,
            "shims_directory": p.shims_directory,
        })),
        "path_conflicts": report.path_conflicts.iter().map(|p| json!({
            "tool": p.tool,
            "executable": p.executable,
            "first_path_directory": p.first_path_directory,
            "shims_directory": p.shims_directory,
        })).collect::<Vec<_>>(),
        "shims_dir_writable": report.shims_dir_writable,
        "kinds": report.kinds.iter().map(|(k, n, cur)| json!({
            "kind": k,
            "installed_count": n,
            "current_version": cur,
        })).collect::<Vec<_>>(),
    });
    serde_json::to_string_pretty(&state).unwrap_or_else(|_| "{}".to_string())
}

fn write_diagnostic_zip(
    zip_path: &Path,
    doctor_json: &str,
    system_txt: &str,
    environment_txt: &str,
    provider_state_json: &str,
) -> Result<(), EnvrError> {
    let file = File::create(zip_path).map_err(EnvrError::from)?;
    let mut zip = ZipWriter::new(file);
    let opts: FileOptions<'_, ()> = FileOptions::default();

    zip.start_file("doctor.json", opts)
        .map_err(|e| EnvrError::with_source(ErrorCode::Runtime, "zip doctor.json", e))?;
    zip.write_all(doctor_json.as_bytes())
        .map_err(EnvrError::from)?;

    zip.start_file("system.txt", opts)
        .map_err(|e| EnvrError::with_source(ErrorCode::Runtime, "zip system.txt", e))?;
    zip.write_all(system_txt.as_bytes())
        .map_err(EnvrError::from)?;

    zip.start_file("environment.txt", opts)
        .map_err(|e| EnvrError::with_source(ErrorCode::Runtime, "zip environment.txt", e))?;
    zip.write_all(environment_txt.as_bytes())
        .map_err(EnvrError::from)?;

    zip.start_file("provider-state.json", opts)
        .map_err(|e| EnvrError::with_source(ErrorCode::Runtime, "zip provider-state.json", e))?;
    zip.write_all(provider_state_json.as_bytes())
        .map_err(EnvrError::from)?;

    append_recent_logs(&mut zip, opts)?;

    zip.finish()
        .map_err(|e| EnvrError::with_source(ErrorCode::Runtime, "zip finish", e))?;
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

        zip.start_file(zip_name, opts).map_err(|e| {
            EnvrError::with_source(ErrorCode::Runtime, format!("zip log {name}"), e)
        })?;
        zip.write_all(&body).map_err(EnvrError::from)?;
    }

    Ok(())
}
