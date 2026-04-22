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
}

fn timings_enabled() -> bool {
    std::env::var_os("ENVR_SHIM_TRACE_TIMING").is_some_and(|v| !v.is_empty())
}

fn emit_timing_report(core_cmd: CoreCommand, timings: &ShimTimings) {
    if !timings_enabled() {
        return;
    }
    eprintln!(
        "envr-shim timing: cmd={core_cmd:?} prepare_total_us={} settings_i18n_us={} parse_us={} resolve_us={} node_hint_us={} child_wait_us={} pip_sync_us={}",
        timings.prepare_total.as_micros(),
        timings.settings_i18n.as_micros(),
        timings.parse_invocation.as_micros(),
        timings.resolve_core.as_micros(),
        timings.node_hint.as_micros(),
        timings.child_wait.as_micros(),
        timings.pip_sync.as_micros(),
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

fn is_global_skip_stem(stem: &str) -> bool {
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
fn strip_windows_verbatim_prefix(p: &Path) -> std::path::PathBuf {
    let s = p.as_os_str().to_string_lossy();
    if let Some(stripped) = s.strip_prefix(r"\\?\") {
        std::path::PathBuf::from(stripped)
    } else {
        p.to_path_buf()
    }
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

fn pnpm_global_mutation(args: &[OsString]) -> bool {
    let mut saw_sub = false;
    let mut saw_global = false;
    for a in args {
        let s = a.to_string_lossy();
        let t = s.trim();
        if t.is_empty() {
            continue;
        }
        if !saw_sub {
            saw_sub = matches!(
                t,
                "add" | "install" | "i" | "remove" | "rm" | "uninstall" | "update" | "up"
            );
            continue;
        }
        if t == "--" {
            break;
        }
        if t == "-g" || t == "--global" {
            saw_global = true;
        }
    }
    saw_sub && saw_global
}

fn yarn_global_mutation(args: &[OsString]) -> bool {
    let ts: Vec<String> = args
        .iter()
        .map(|s| s.to_string_lossy().trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .collect();
    if ts.len() >= 2
        && ts[0] == "global"
        && matches!(ts[1].as_str(), "add" | "remove" | "upgrade" | "up")
    {
        return true;
    }
    // Some wrappers still accept `-g/--global`.
    let mut saw_pkg_sub = false;
    let mut saw_global = false;
    for t in &ts {
        if !saw_pkg_sub {
            saw_pkg_sub = matches!(t.as_str(), "add" | "remove" | "upgrade" | "up");
            continue;
        }
        if t == "--" {
            break;
        }
        if t == "-g" || t == "--global" {
            saw_global = true;
        }
    }
    saw_pkg_sub && saw_global
}

fn should_sync_node_globals_for_forward(stem: &str, args: &[OsString], status_ok: bool) -> bool {
    if !status_ok {
        return false;
    }
    match stem {
        "pnpm" => pnpm_global_mutation(args),
        "yarn" | "yarnpkg" => yarn_global_mutation(args),
        _ => false,
    }
}

fn npm_global_mutation_succeeded(cmd: CoreCommand, args: &[OsString], status_ok: bool) -> bool {
    if !status_ok || !matches!(cmd, CoreCommand::Npm) {
        return false;
    }
    let mut saw_sub = false;
    let mut saw_global = false;
    for a in args {
        let s = a.to_string_lossy();
        let t = s.trim();
        if t.is_empty() {
            continue;
        }
        if !saw_sub {
            saw_sub = matches!(
                t,
                "install" | "i" | "add" | "update" | "up" | "upgrade" | "uninstall" | "remove" | "rm"
            );
            continue;
        }
        if t == "--" {
            break;
        }
        if t == "-g" || t == "--global" {
            saw_global = true;
        }
    }
    saw_sub && saw_global
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

fn sync_node_global_shims_best_effort(runtime_root: &Path, npm_executable: &Path) {
    let warn = |msg: String| {
        eprintln!("envr-shim: warning: {msg}");
    };

    let shims_dir = runtime_root.join("shims");
    if let Err(err) = fs::create_dir_all(&shims_dir) {
        warn(format!(
            "failed to create shims directory {}: {}",
            shims_dir.display(),
            err
        ));
        return;
    }

    #[cfg(windows)]
    let npm_executable = strip_windows_verbatim_prefix(npm_executable);
    #[cfg(not(windows))]
    let npm_executable = npm_executable.to_path_buf();

    let out = match if is_windows_cmd_script(&npm_executable) {
        Command::new("cmd")
            .args(["/d", "/c"])
            .arg(&npm_executable)
            .args(["bin", "-g"])
            .output()
    } else {
        Command::new(&npm_executable).args(["bin", "-g"]).output()
    } {
        Ok(o) => o,
        Err(err) => {
            warn(format!(
                "failed to run `{} bin -g`: {}",
                npm_executable.display(),
                err
            ));
            return;
        }
    };
    if !out.status.success() {
        warn(format!(
            "`{} bin -g` failed: {}",
            npm_executable.display(),
            String::from_utf8_lossy(&out.stderr)
        ));
        return;
    }
    let bin_dir = std::path::PathBuf::from(String::from_utf8_lossy(&out.stdout).trim().to_string());
    sync_node_global_shims_from_bin_dir_best_effort(runtime_root, &bin_dir);
}

fn sync_node_global_shims_from_bin_dir_best_effort(runtime_root: &Path, bin_dir: &Path) {
    let warn = |msg: String| {
        eprintln!("envr-shim: warning: {msg}");
    };
    let shims_dir = runtime_root.join("shims");
    if let Err(err) = fs::create_dir_all(&shims_dir) {
        warn(format!(
            "failed to create shims directory {}: {}",
            shims_dir.display(),
            err
        ));
        return;
    }

    if !bin_dir.is_dir() {
        warn(format!("npm global bin directory not found at {}", bin_dir.display()));
        return;
    }

    let Ok(entries) = fs::read_dir(&bin_dir) else {
        warn(format!("failed to read npm global bin directory {}", bin_dir.display()));
        return;
    };

    for e in entries.flatten() {
        let path = e.path();
        if !path.is_file() {
            continue;
        }
        #[cfg(windows)]
        {
            let ext = path
                .extension()
                .and_then(|x| x.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            if !matches!(ext.as_str(), "cmd" | "exe" | "bat" | "com") {
                continue;
            }
        }
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if stem.is_empty() || is_global_skip_stem(&stem) {
            continue;
        }
        #[cfg(windows)]
        {
            let dst = shims_dir.join(format!("{stem}.cmd"));
            let body = format!("@echo off\r\ncall \"{}\" %*\r\n", path.display());
            if let Err(err) = fs::write(&dst, body) {
                warn(format!(
                    "failed to write node global shim {}: {}",
                    dst.display(),
                    err
                ));
            }
        }
        #[cfg(not(windows))]
        {
            let dst = shims_dir.join(&stem);
            if dst.exists() && let Err(err) = fs::remove_file(&dst) {
                warn(format!(
                    "failed to replace node global shim {}: {}",
                    dst.display(),
                    err
                ));
                continue;
            }
            if let Err(err) = std::os::unix::fs::symlink(&path, &dst) {
                warn(format!(
                    "failed to link node global shim {} -> {}: {}",
                    dst.display(),
                    path.display(),
                    err
                ));
            }
        }
    }
}

#[cfg(windows)]
fn maybe_run_windows_node_forward_helper(args: &[OsString]) -> Option<i32> {
    // Usage: envr-shim __forward-node-global <target> <stem> [user args...]
    if args.get(1).and_then(|s| s.to_str()) != Some("__forward-node-global") {
        return None;
    }
    let Some(target) = args.get(2).and_then(|s| s.to_str()) else {
        eprintln!("envr-shim: invalid __forward-node-global args: missing target");
        return Some(2);
    };
    let Some(stem) = args.get(3).and_then(|s| s.to_str()) else {
        eprintln!("envr-shim: invalid __forward-node-global args: missing stem");
        return Some(2);
    };
    let forward: Vec<OsString> = args.iter().skip(4).cloned().collect();
    let target = strip_windows_verbatim_prefix(std::path::Path::new(target));
    let status = match if is_js_entry_script(&target) {
        let Some(node_exe) = find_node_exe_for_script(&target) else {
            eprintln!(
                "envr-shim: cannot locate node.exe for forwarded script `{}`",
                target.display()
            );
            return Some(1);
        };
        Command::new(node_exe).arg(&target).args(&forward).status()
    } else if is_windows_cmd_script(&target) {
        Command::new("cmd")
            .args(["/d", "/c"])
            .arg(&target)
            .args(&forward)
            .status()
    } else {
        Command::new(&target).args(&forward).status()
    } {
        Ok(s) => s,
        Err(e) => {
            eprintln!(
                "envr-shim: failed to spawn forwarded tool `{}`: {e}",
                target.display()
            );
            return Some(1);
        }
    };
    let code = status.code().unwrap_or(0xFF);
    if should_sync_node_globals_for_forward(stem, &forward, status.success())
        && let Ok(ctx) = ShimContext::from_process_env()
    {
        let bin_dir = target.parent().map(std::path::Path::to_path_buf);
        if let Some(bin_dir) = bin_dir {
            sync_node_global_shims_from_bin_dir_best_effort(&ctx.runtime_root, &bin_dir);
        }
    }
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
    if npm_global_mutation_succeeded(core_cmd, &forward, status.success()) {
        sync_node_global_shims_best_effort(&ctx.runtime_root, &resolved.executable);
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
    fn npm_global_mutation_detected() {
        assert!(npm_global_mutation_succeeded(
            CoreCommand::Npm,
            &os_args(&["install", "-g", "pnpm"]),
            true
        ));
        assert!(npm_global_mutation_succeeded(
            CoreCommand::Npm,
            &os_args(&["remove", "--global", "@anthropic-ai/claude-code"]),
            true
        ));
        assert!(!npm_global_mutation_succeeded(
            CoreCommand::Npm,
            &os_args(&["install", "@anthropic-ai/claude-code"]),
            true
        ));
    }

    #[test]
    fn pnpm_global_mutation_detected() {
        assert!(pnpm_global_mutation(&os_args(&["add", "-g", "tsx"])));
        assert!(!pnpm_global_mutation(&os_args(&["add", "tsx"])));
    }

    #[test]
    fn yarn_global_mutation_detected() {
        assert!(yarn_global_mutation(&os_args(&["global", "add", "tsx"])));
        assert!(yarn_global_mutation(&os_args(&["add", "-g", "tsx"])));
        assert!(!yarn_global_mutation(&os_args(&["add", "tsx"])));
    }

    #[cfg(windows)]
    #[test]
    fn strip_windows_verbatim_prefix_handles_path() {
        let p = std::path::Path::new(r"\\?\D:\runtime\node\npm.cmd");
        assert_eq!(
            strip_windows_verbatim_prefix(p),
            std::path::PathBuf::from(r"D:\runtime\node\npm.cmd")
        );
    }
}
