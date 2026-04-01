//! envr-shim: resolve a core tool via [`envr_shim_core`], then exec (Unix) or spawn and forward exit code (Windows).

use envr_error::EnvrError;
use envr_shim_core::{ResolvedShim, ShimContext, parse_shim_invocation, resolve_core_shim_command};
use std::ffi::OsString;
use std::process::Command;

fn prepare(args: &[OsString]) -> Result<(ResolvedShim, Vec<OsString>), EnvrError> {
    let ctx = ShimContext::from_process_env()?;
    let (cmd, forward) = parse_shim_invocation(args)?;
    let resolved = resolve_core_shim_command(cmd, &ctx)?;
    Ok((resolved, forward))
}

#[cfg(unix)]
fn main() {
    let args: Vec<OsString> = std::env::args_os().collect();
    let (resolved, forward) = match prepare(&args) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };

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
    let (resolved, forward) = match prepare(&args) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };

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
    std::process::exit(status.code().unwrap_or(0xFF));
}
