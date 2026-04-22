//! envr-shim: resolve a core tool via [`envr_shim_core`], then exec (Unix) or spawn and forward exit code (Windows).

mod node_engines_hint;
mod shim_i18n;

use envr_config::settings::{Settings, settings_path_from_platform};
use envr_error::EnvrError;
use envr_shim_core::{
    CoreCommand, ResolvedShim, ShimContext, ShimSettingsSnapshot, parse_shim_invocation,
    resolve_core_shim_command_with_settings, runtime_version_label_from_executable,
};
use std::ffi::OsString;
use std::collections::HashMap;
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
    let platform = envr_platform::paths::current_platform_paths().ok()?;
    let path = settings_path_from_platform(&platform);
    Settings::load_or_default_from(&path).ok()
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

fn npm_is_global_mutation(args: &[OsString]) -> bool {
    let mut saw_global = false;
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
        if t == "-g" || t == "--global" {
            saw_global = true;
            continue;
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
    saw_global && saw_mutating_subcommand
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
new CLIs are not exposed on PATH. Use `npm install -g <pkg>` then `envr shim sync --globals`, \
or run via `npx <cmd>` from this project."
    );
}

#[cfg(windows)]
fn normalize_windows_path_for_cmd(raw: &Path) -> String {
    let s = raw.display().to_string();
    if let Some(rest) = s.strip_prefix(r"\\?\") {
        return rest.to_string();
    }
    if let Some(rest) = s.strip_prefix("//?/") {
        return rest.replace('/', "\\");
    }
    s
}

#[cfg(windows)]
fn is_windows_system_command_stem(stem: &str) -> bool {
    let windir = std::env::var_os("WINDIR").unwrap_or_else(|| "C:\\Windows".into());
    let system32 = std::path::PathBuf::from(windir).join("System32");
    for ext in ["exe", "cmd", "bat", "com"] {
        if system32.join(format!("{stem}.{ext}")).is_file() {
            return true;
        }
    }
    false
}

fn is_node_global_skip_stem(stem: &str) -> bool {
    matches!(
        stem,
        "node"
            | "npm"
            | "npx"
            | "corepack"
            | "yarn"
            | "python"
            | "python3"
            | "pip"
            | "pip3"
            | "java"
            | "javac"
            | "clojure"
            | "clj"
            | "groovy"
            | "groovyc"
            | "terraform"
            | "v"
            | "gleam"
            | "janet"
            | "jpm"
            | "dart"
            | "flutter"
            | "php"
            | "bun"
            | "bunx"
            | "dotnet"
            | "erl"
            | "erlc"
            | "escript"
    )
}

#[cfg(windows)]
fn windows_global_bin_target_priority(ext: Option<&str>) -> i32 {
    match ext.unwrap_or("").to_ascii_lowercase().as_str() {
        "cmd" => 100,
        "exe" => 90,
        "bat" => 80,
        "com" => 70,
        "ps1" => 10,
        _ => 0,
    }
}

#[cfg(windows)]
fn sync_node_global_shims_best_effort(runtime_root: &Path, npm_executable: &Path, extra_env: &[(String, String)]) {
    let warn = |msg: String| eprintln!("envr-shim: warning: {msg}");
    let resolve_global_bin = || -> Option<std::path::PathBuf> {
        let mut bin_cmd = Command::new(npm_executable);
        bin_cmd.args(["bin", "-g"]);
        for (k, v) in extra_env {
            bin_cmd.env(k, v);
        }
        if let Ok(out) = bin_cmd.output()
            && out.status.success()
        {
            let p = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !p.is_empty() {
                return Some(std::path::PathBuf::from(p));
            }
        }

        // npm v10+ can remove/deprecate `npm bin`; fall back to prefix-derived bin dir.
        let mut prefix_cmd = Command::new(npm_executable);
        prefix_cmd.args(["prefix", "-g"]);
        for (k, v) in extra_env {
            prefix_cmd.env(k, v);
        }
        let out = match prefix_cmd.output() {
            Ok(x) => x,
            Err(err) => {
                warn(format!("npm prefix -g failed to spawn: {err}"));
                return None;
            }
        };
        if !out.status.success() {
            warn(format!(
                "npm prefix -g failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            ));
            return None;
        }
        let prefix = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if prefix.is_empty() {
            return None;
        }
        #[cfg(windows)]
        {
            Some(std::path::PathBuf::from(prefix))
        }
        #[cfg(not(windows))]
        {
            Some(std::path::PathBuf::from(prefix).join("bin"))
        }
    };

    let Some(global_bin) = resolve_global_bin() else {
        return;
    };
    if !global_bin.is_dir() {
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

    let entries = match fs::read_dir(&global_bin) {
        Ok(x) => x,
        Err(_) => return,
    };
    let mut best: HashMap<String, (i32, std::path::PathBuf)> = HashMap::new();
    for e in entries.flatten() {
        let p = e.path();
        if !p.is_file() {
            continue;
        }
        let stem = p
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if stem.is_empty() || is_node_global_skip_stem(&stem) {
            continue;
        }
        if is_windows_system_command_stem(&stem) {
            continue;
        }
        let prio = windows_global_bin_target_priority(p.extension().and_then(|x| x.to_str()));
        match best.get(&stem) {
            None => {
                best.insert(stem, (prio, p));
            }
            Some((old_prio, _)) if prio > *old_prio => {
                best.insert(stem, (prio, p));
            }
            _ => {}
        }
    }

    for (stem, (_, target)) in best {
        let dst = shims_dir.join(format!("{stem}.cmd"));
        let target_s = normalize_windows_path_for_cmd(&target);
        let body = format!("@echo off\r\ncall \"{}\" %*\r\n", target_s);
        if let Err(err) = fs::write(&dst, body) {
            warn(format!("failed to write node global shim {}: {}", dst.display(), err));
        }
    }
}

#[cfg(not(windows))]
fn sync_node_global_shims_best_effort(
    _runtime_root: &Path,
    _npm_executable: &Path,
    _extra_env: &[(String, String)],
) {
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
    if status.success() && matches!(core_cmd, CoreCommand::Npm) && npm_is_global_mutation(&forward) {
        let npm_sync_started = Instant::now();
        sync_node_global_shims_best_effort(&ctx.runtime_root, &resolved.executable, &resolved.extra_env);
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
    fn npm_global_mutation_detection() {
        assert!(npm_is_global_mutation(&os_args(&["install", "-g", "pnpm"])));
        assert!(npm_is_global_mutation(&os_args(&["--global", "i", "typescript"])));
        assert!(!npm_is_global_mutation(&os_args(&["install", "pnpm"])));
        assert!(!npm_is_global_mutation(&os_args(&["--global", "config", "get", "prefix"])));
    }
}
