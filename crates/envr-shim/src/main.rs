//! envr-shim: resolve a core tool via [`envr_shim_core`], then exec (Unix) or spawn and forward exit code (Windows).

mod node_engines_hint;
mod shim_i18n;

use envr_error::EnvrError;
use envr_shim_core::{
    CoreCommand, ResolvedShim, ShimContext, parse_shim_invocation, resolve_core_shim_command,
};
use std::ffi::OsString;
use std::fs;
use std::path::Path;
use std::process::Command;

fn prepare(
    args: &[OsString],
) -> Result<(CoreCommand, ShimContext, ResolvedShim, Vec<OsString>), EnvrError> {
    let ctx = ShimContext::from_process_env()?;
    let (cmd, forward) = parse_shim_invocation(args)?;
    let resolved = resolve_core_shim_command(cmd, &ctx)?;
    Ok((cmd, ctx, resolved, forward))
}

fn maybe_node_engines_hint(cmd: CoreCommand, ctx: &ShimContext) {
    if matches!(cmd, CoreCommand::Node) {
        node_engines_hint::maybe_emit(ctx);
    }
}

fn is_python_core_stem(stem: &str) -> bool {
    matches!(stem, "python" | "python3" | "pip" | "pip3")
}

fn sync_python_script_shims_best_effort(runtime_root: &Path, pip_executable: &Path) {
    let Some(script_dir) = pip_executable.parent() else {
        return;
    };
    if !script_dir.is_dir() {
        return;
    }
    let shims_dir = runtime_root.join("shims");
    let _ = fs::create_dir_all(&shims_dir);

    let Ok(entries) = fs::read_dir(script_dir) else {
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
            let _ = fs::write(dst, body);
        }
        #[cfg(not(windows))]
        {
            let dst = shims_dir.join(&stem);
            if dst.exists() {
                let _ = fs::remove_file(&dst);
            }
            let _ = std::os::unix::fs::symlink(&path, &dst);
        }
    }
}

#[cfg(unix)]
fn main() {
    shim_i18n::bootstrap();
    let args: Vec<OsString> = std::env::args_os().collect();
    let (core_cmd, ctx, resolved, forward) = match prepare(&args) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };
    maybe_node_engines_hint(core_cmd, &ctx);

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
    shim_i18n::bootstrap();
    let args: Vec<OsString> = std::env::args_os().collect();
    let (core_cmd, ctx, resolved, forward) = match prepare(&args) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };
    maybe_node_engines_hint(core_cmd, &ctx);

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
