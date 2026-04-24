//! envr-shim: resolve a core tool via [`envr_shim_core`], then exec (Unix) or spawn and forward exit code (Windows).

mod node_engines_hint;
mod shim_i18n;

use envr_config::env_context::load_settings_cached;
use envr_config::settings::Settings;
use envr_error::EnvrError;
use envr_shim_core::{
    CoreCommand, ResolvedShim, ShimContext, ShimSettingsSnapshot, parse_shim_invocation,
    resolve_core_shim_command_with_settings, runtime_version_label_from_executable,
};
use std::ffi::OsString;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, Default)]
struct ShimTimings {
    prepare_total: Duration,
    settings_i18n: Duration,
    parse_invocation: Duration,
    resolve_core: Duration,
    node_hint: Duration,
    child_wait: Duration,
    pip_sync: Duration,
    npm_sync: Duration,
}

fn timings_enabled() -> bool {
    std::env::var_os("ENVR_SHIM_TRACE_TIMING").is_some_and(|v| !v.is_empty())
}

fn emit_timing_report(core_cmd: CoreCommand, timings: &ShimTimings) {
    if !timings_enabled() {
        return;
    }
    eprintln!(
        "envr-shim timing: cmd={core_cmd:?} prepare_total_us={} settings_i18n_us={} parse_us={} resolve_us={} node_hint_us={} child_wait_us={} pip_sync_us={} npm_sync_us={}",
        timings.prepare_total.as_micros(),
        timings.settings_i18n.as_micros(),
        timings.parse_invocation.as_micros(),
        timings.resolve_core.as_micros(),
        timings.node_hint.as_micros(),
        timings.child_wait.as_micros(),
        timings.pip_sync.as_micros(),
        timings.npm_sync.as_micros(),
    );
}

fn load_settings_for_invocation() -> Option<Settings> {
    load_settings_cached().ok()
}

fn prepare(
    args: &[OsString],
) -> Result<
    (
        CoreCommand,
        ShimContext,
        ShimSettingsSnapshot,
        ResolvedShim,
        Vec<OsString>,
        ShimTimings,
    ),
    EnvrError,
> {
    let mut timings = ShimTimings::default();
    let prepare_started = Instant::now();
    let settings_started = Instant::now();
    let settings = load_settings_for_invocation();
    if let Some(st) = settings.as_ref() {
        shim_i18n::bootstrap_with_locale(st.i18n.locale);
    } else {
        shim_i18n::bootstrap();
    }
    let snapshot = settings
        .as_ref()
        .map(ShimSettingsSnapshot::from_settings)
        .unwrap_or_else(ShimSettingsSnapshot::from_disk);
    timings.settings_i18n = settings_started.elapsed();
    let ctx = ShimContext::from_process_env()?;
    let parse_started = Instant::now();
    let (cmd, forward) = parse_shim_invocation(args)?;
    timings.parse_invocation = parse_started.elapsed();
    let resolve_started = Instant::now();
    let resolved = resolve_core_shim_command_with_settings(cmd, &ctx, &snapshot)?;
    timings.resolve_core = resolve_started.elapsed();
    timings.prepare_total = prepare_started.elapsed();
    Ok((cmd, ctx, snapshot, resolved, forward, timings))
}

fn maybe_node_engines_hint(cmd: CoreCommand, ctx: &ShimContext, active_label: Option<&str>) {
    if matches!(cmd, CoreCommand::Node) {
        if let Some(label) = active_label {
            node_engines_hint::maybe_emit(ctx, label);
        }
    }
}

fn is_python_core_stem(stem: &str) -> bool {
    matches!(stem, "python" | "python3" | "pip" | "pip3")
}

fn npm_install_is_local_without_global(args: &[OsString]) -> bool {
    let mut saw_install_subcommand = false;
    let mut saw_global_flag = false;
    let mut saw_pkg_operand = false;
    for a in args {
        let s = a.to_string_lossy();
        let t = s.trim();
        if t.is_empty() {
            continue;
        }
        if !saw_install_subcommand {
            saw_install_subcommand = matches!(t, "install" | "i" | "add");
            continue;
        }
        if t == "--" {
            break;
        }
        if t == "-g" || t == "--global" {
            saw_global_flag = true;
            continue;
        }
        if t.starts_with('-') {
            continue;
        }
        saw_pkg_operand = true;
    }
    saw_install_subcommand && saw_pkg_operand && !saw_global_flag
}

fn npm_is_package_mutation(args: &[OsString]) -> bool {
    let mut saw_mutating_subcommand = false;
    for a in args {
        let s = a.to_string_lossy();
        let t = s.trim();
        if t.is_empty() {
            continue;
        }
        if t == "--" {
            break;
        }
        if t.starts_with('-') {
            continue;
        }
        if matches!(
            t,
            "install"
                | "i"
                | "add"
                | "uninstall"
                | "remove"
                | "rm"
                | "update"
                | "up"
                | "link"
        ) {
            saw_mutating_subcommand = true;
        }
    }
    saw_mutating_subcommand
}

fn maybe_print_npm_local_install_hint(core_cmd: CoreCommand, forward: &[OsString], status_ok: bool) {
    if !status_ok || !matches!(core_cmd, CoreCommand::Npm) {
        return;
    }
    if !npm_install_is_local_without_global(forward) {
        return;
    }
    eprintln!(
        "envr-shim: npm install ran in local mode (no -g/--global); \
new CLIs are not exposed on PATH. Use `npm install -g <pkg>`, \
or run via `npx <cmd>` from this project."
    );
}

fn sync_globals_via_envr_cli_best_effort(runtime_root: &Path) {
    let warn = |msg: String| eprintln!("envr-shim: warning: {msg}");
    let Ok(cur) = std::env::current_exe() else {
        return;
    };
    let Some(dir) = cur.parent() else {
        return;
    };
    #[cfg(windows)]
    let candidates = [dir.join("er.exe"), dir.join("envr.exe"), dir.join("er.cmd")];
    #[cfg(not(windows))]
    let candidates = [dir.join("er"), dir.join("envr"), dir.join("er.cmd")];

    let Some(cli_exe) = candidates.iter().find(|p| p.is_file()) else {
        warn("skip auto global shim refresh: envr CLI executable not found beside envr-shim".into());
        return;
    };
    let status = Command::new(cli_exe)
        .args(["shim", "sync", "--globals"])
        .env("ENVR_RUNTIME_ROOT", runtime_root.as_os_str())
        .status();
    if let Ok(s) = status {
        if !s.success() {
            warn(format!("auto global shim refresh failed with status {s}"));
        }
    } else if let Err(err) = status {
        warn(format!("failed to run auto global shim refresh: {err}"));
    }
}

#[cfg(windows)]
fn strip_windows_verbatim_prefix(p: &Path) -> std::path::PathBuf {
    let s = p.as_os_str().to_string_lossy();
    if let Some(rest) = s.strip_prefix(r"\\?\") {
        return std::path::PathBuf::from(rest);
    }
    if let Some(rest) = s.strip_prefix("//?/") {
        return std::path::PathBuf::from(rest.replace('/', "\\"));
    }
    p.to_path_buf()
}

#[cfg(windows)]
fn is_windows_cmd_script(p: &Path) -> bool {
    matches!(
        p.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .as_deref(),
        Some("cmd" | "bat" | "com")
    )
}

#[cfg(windows)]
fn is_js_entry_script(p: &Path) -> bool {
    matches!(
        p.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .as_deref(),
        Some("js" | "cjs" | "mjs")
    )
}

#[cfg(windows)]
fn find_node_exe_for_script(script: &Path) -> Option<std::path::PathBuf> {
    let mut cur = script.parent()?;
    loop {
        let cand = cur.join("node.exe");
        if cand.is_file() {
            return Some(cand);
        }
        cur = cur.parent()?;
    }
}

#[cfg(windows)]
fn maybe_run_windows_node_forward_helper(args: &[OsString]) -> Option<i32> {
    // Backward compatibility for old generated stubs:
    // envr-shim __forward-node-global <target> <stem> [user args...]
    if args.get(1).and_then(|s| s.to_str()) != Some("__forward-node-global") {
        return None;
    }
    let Some(target) = args.get(2).and_then(|s| s.to_str()) else {
        eprintln!("envr-shim: invalid __forward-node-global args: missing target");
        return Some(2);
    };
    let target = strip_windows_verbatim_prefix(std::path::Path::new(target));
    let forward: Vec<OsString> = args.iter().skip(4).cloned().collect();
    let status = if is_js_entry_script(&target) {
        if let Some(node_exe) = find_node_exe_for_script(&target) {
            Command::new(node_exe).arg(&target).args(&forward).status()
        } else {
            // Fallback: resolve node from PATH.
            Command::new("node").arg(&target).args(&forward).status()
        }
    } else if is_windows_cmd_script(&target) {
        Command::new("cmd")
            .args(["/d", "/c"])
            .arg(&target)
            .args(&forward)
            .status()
    } else {
        Command::new(&target).args(&forward).status()
    };
    let code = match status {
        Ok(s) => s.code().unwrap_or(0xFF),
        Err(e) => {
            eprintln!(
                "envr-shim: failed to spawn forwarded tool `{}`: {e}",
                target.display()
            );
            1
        }
    };
    Some(code)
}

fn sync_python_script_shims_best_effort(runtime_root: &Path, pip_executable: &Path) {
    let warn = |msg: String| {
        eprintln!("envr-shim: warning: {msg}");
    };
    let Some(script_dir) = pip_executable.parent() else {
        warn(format!(
            "skip python script shim sync: invalid pip path {}",
            pip_executable.display()
        ));
        return;
    };
    if !script_dir.is_dir() {
        warn(format!(
            "skip python script shim sync: scripts directory not found at {}",
            script_dir.display()
        ));
        return;
    }
    let shims_dir = runtime_root.join("shims");
    if let Err(err) = fs::create_dir_all(&shims_dir) {
        warn(format!(
            "failed to create shims directory {}: {}",
            shims_dir.display(),
            err
        ));
        return;
    }

    let Ok(entries) = fs::read_dir(script_dir) else {
        warn(format!(
            "failed to read python scripts directory {}",
            script_dir.display()
        ));
        return;
    };
    for e in entries.flatten() {
        let path = e.path();
        if !path.is_file() {
            continue;
        }
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if stem.is_empty() || is_python_core_stem(&stem) {
            continue;
        }
        #[cfg(windows)]
        {
            let dst = shims_dir.join(format!("{stem}.cmd"));
            let body = format!("@echo off\r\ncall \"{}\" %*\r\n", path.display());
            if let Err(err) = fs::write(&dst, body) {
                warn(format!(
                    "failed to write python script shim {}: {}",
                    dst.display(),
                    err
                ));
            }
        }
        #[cfg(not(windows))]
        {
            let dst = shims_dir.join(&stem);
            if dst.exists() {
                if let Err(err) = fs::remove_file(&dst) {
                    warn(format!(
                        "failed to replace python script shim {}: {}",
                        dst.display(),
                        err
                    ));
                    continue;
                }
            }
            if let Err(err) = std::os::unix::fs::symlink(&path, &dst) {
                warn(format!(
                    "failed to link python script shim {} -> {}: {}",
                    dst.display(),
                    path.display(),
                    err
                ));
            }
        }
    }
}

#[cfg(unix)]
fn main() {
    let args: Vec<OsString> = std::env::args_os().collect();
    let (core_cmd, ctx, _settings, resolved, forward, mut timings) = match prepare(&args) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };
    let active_label = runtime_version_label_from_executable(&resolved.executable);
    let node_hint_started = Instant::now();
    maybe_node_engines_hint(core_cmd, &ctx, active_label.as_deref());
    timings.node_hint = node_hint_started.elapsed();
    emit_timing_report(core_cmd, &timings);

    use std::os::unix::process::CommandExt;
    let mut cmd = Command::new(&resolved.executable);
    cmd.args(&forward);
    for (k, v) in &resolved.extra_env {
        cmd.env(k, v);
    }
    let err = cmd.exec();
    eprintln!("envr-shim: exec failed: {err}");
    std::process::exit(1);
}

#[cfg(windows)]
fn main() {
    let args: Vec<OsString> = std::env::args_os().collect();
    if let Some(code) = maybe_run_windows_node_forward_helper(&args) {
        std::process::exit(code);
    }
    let (core_cmd, ctx, _settings, resolved, forward, mut timings) = match prepare(&args) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };
    let active_label = runtime_version_label_from_executable(&resolved.executable);
    let node_hint_started = Instant::now();
    maybe_node_engines_hint(core_cmd, &ctx, active_label.as_deref());
    timings.node_hint = node_hint_started.elapsed();

    let mut cmd = Command::new(&resolved.executable);
    cmd.args(&forward);
    for (k, v) in &resolved.extra_env {
        cmd.env(k, v);
    }
    let child_started = Instant::now();
    let status = match cmd.status() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("envr-shim: failed to spawn target: {e}");
            std::process::exit(1);
        }
    };
    timings.child_wait = child_started.elapsed();
    if status.success() && matches!(core_cmd, CoreCommand::Pip) {
        let pip_sync_started = Instant::now();
        sync_python_script_shims_best_effort(&ctx.runtime_root, &resolved.executable);
        timings.pip_sync = pip_sync_started.elapsed();
    }
    if status.success() && matches!(core_cmd, CoreCommand::Npm) && npm_is_package_mutation(&forward) {
        let npm_sync_started = Instant::now();
        sync_globals_via_envr_cli_best_effort(&ctx.runtime_root);
        timings.npm_sync = npm_sync_started.elapsed();
    }
    maybe_print_npm_local_install_hint(core_cmd, &forward, status.success());
    emit_timing_report(core_cmd, &timings);
    std::process::exit(status.code().unwrap_or(0xFF));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn os_args(xs: &[&str]) -> Vec<OsString> {
        xs.iter().map(|s| OsString::from(*s)).collect()
    }

    #[test]
    fn npm_install_without_global_is_detected() {
        assert!(npm_install_is_local_without_global(&os_args(&[
            "install",
            "@anthropic-ai/claude-code@2.1.110",
        ])));
        assert!(npm_install_is_local_without_global(&os_args(&[
            "i",
            "typescript",
        ])));
    }

    #[test]
    fn npm_global_install_is_not_reported_local() {
        assert!(!npm_install_is_local_without_global(&os_args(&[
            "install",
            "-g",
            "@anthropic-ai/claude-code@2.1.110",
        ])));
        assert!(!npm_install_is_local_without_global(&os_args(&[
            "add",
            "--global",
            "pnpm",
        ])));
    }

    #[test]
    fn npm_package_mutation_detection() {
        assert!(npm_is_package_mutation(&os_args(&["install", "-g", "pnpm"])));
        assert!(npm_is_package_mutation(&os_args(&["install", "pnpm"])));
        assert!(npm_is_package_mutation(&os_args(&["remove", "--global", "pnpm"])));
        assert!(!npm_is_package_mutation(&os_args(&["--global", "config", "get", "prefix"])));
    }

    #[cfg(windows)]
    #[test]
    fn strip_windows_verbatim_prefix_handles_both_forms() {
        let p1 = std::path::Path::new(r"\\?\D:\runtime\node\npm.cmd");
        assert_eq!(
            strip_windows_verbatim_prefix(p1),
            std::path::PathBuf::from(r"D:\runtime\node\npm.cmd")
        );
        let p2 = std::path::Path::new("//?/D:/runtime/node/npm.cmd");
        assert_eq!(
            strip_windows_verbatim_prefix(p2),
            std::path::PathBuf::from(r"D:\runtime\node\npm.cmd")
        );
    }
}
