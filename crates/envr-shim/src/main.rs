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
    ),
    EnvrError,
> {
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
    let ctx = ShimContext::from_process_env()?;
    let (cmd, forward) = parse_shim_invocation(args)?;
    let resolved = resolve_core_shim_command_with_settings(cmd, &ctx, &snapshot)?;
    Ok((cmd, ctx, snapshot, resolved, forward))
}

fn maybe_node_engines_hint(
    cmd: CoreCommand,
    ctx: &ShimContext,
    active_label: Option<&str>,
) {
    if matches!(cmd, CoreCommand::Node) {
        if let Some(label) = active_label {
            node_engines_hint::maybe_emit(ctx, label);
        }
    }
}

fn is_python_core_stem(stem: &str) -> bool {
    matches!(stem, "python" | "python3" | "pip" | "pip3")
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
    let (core_cmd, ctx, _settings, resolved, forward) = match prepare(&args) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };
    let active_label = runtime_version_label_from_executable(&resolved.executable);
    maybe_node_engines_hint(core_cmd, &ctx, active_label.as_deref());

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
    let (core_cmd, ctx, _settings, resolved, forward) = match prepare(&args) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };
    let active_label = runtime_version_label_from_executable(&resolved.executable);
    maybe_node_engines_hint(core_cmd, &ctx, active_label.as_deref());

    let mut cmd = Command::new(&resolved.executable);
    cmd.args(&forward);
    for (k, v) in &resolved.extra_env {
        cmd.env(k, v);
    }
    let status = match cmd.status() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("envr-shim: failed to spawn target: {e}");
            std::process::exit(1);
        }
    };
    if status.success() && matches!(core_cmd, CoreCommand::Pip) {
        sync_python_script_shims_best_effort(&ctx.runtime_root, &resolved.executable);
    }
    std::process::exit(status.code().unwrap_or(0xFF));
}
