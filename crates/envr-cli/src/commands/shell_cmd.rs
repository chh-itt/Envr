//! Interactive subshell with the same merged environment as `envr env` / `collect_run_env`.

use crate::cli::{GlobalArgs, OutputFormat};
use crate::commands::child_env;
use crate::commands::common;
use crate::output::{self, fmt_template};

use serde_json::json;
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::Command;

pub fn run(g: &GlobalArgs, path: PathBuf, profile: Option<String>, shell: Option<PathBuf>) -> i32 {
    let ctx = match common::shim_context_for(path, profile) {
        Ok(c) => c,
        Err(e) => return common::print_envr_error(g, e),
    };

    let env_map = match child_env::collect_run_env(&ctx, false) {
        Ok(m) => m,
        Err(e) => return common::print_envr_error(g, e),
    };

    let (program, extra_args) = match resolve_shell_invocation(shell) {
        Ok(p) => p,
        Err(e) => return common::print_envr_error(g, e),
    };

    let mut cmd = Command::new(&program);
    cmd.args(extra_args);
    cmd.env_clear();
    for (k, v) in &env_map {
        cmd.env(k, v);
    }
    cmd.current_dir(&ctx.working_dir);

    let base_data = json!({
        "shell": program.to_string_lossy(),
        "cwd": ctx.working_dir.to_string_lossy(),
    });

    let status = match cmd.status() {
        Ok(s) => s,
        Err(e) => return common::print_envr_error(g, e.into()),
    };

    let code = status.code().unwrap_or(1);
    if code == 0 {
        output::emit_ok(g, "shell_exited", base_data, || {});
        0
    } else {
        let msg = fmt_template(
            &envr_core::i18n::tr_key(
                "cli.shell.exit_nonzero",
                "子 shell 退出，代码 {code}",
                "subshell exited with code {code}",
            ),
            &[("code", &code.to_string())],
        );
        let fail_data = json!({
            "shell": program.to_string_lossy(),
            "cwd": ctx.working_dir.to_string_lossy(),
            "exit_code": code,
        });
        match g.output_format.unwrap_or(OutputFormat::Text) {
            OutputFormat::Json => {
                output::write_envelope(false, Some("shell_exit"), &msg, fail_data, &[]);
            }
            OutputFormat::Text => {
                output::print_error_text("shell_exit", &msg);
            }
        }
        code
    }
}

fn resolve_shell_invocation(override_path: Option<PathBuf>) -> Result<(PathBuf, Vec<OsString>), envr_error::EnvrError> {
    if let Some(p) = override_path {
        return Ok((p, Vec::new()));
    }

    if let Ok(p) = std::env::var("ENVR_SHELL") {
        let t = p.trim();
        if !t.is_empty() {
            return Ok((PathBuf::from(t), Vec::new()));
        }
    }

    #[cfg(windows)]
    {
        let comspec = std::env::var("ComSpec").unwrap_or_else(|_| "cmd.exe".into());
        Ok((PathBuf::from(comspec), vec![OsString::from("/K")]))
    }

    #[cfg(not(windows))]
    {
        let sh = std::env::var("SHELL")
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/bin/sh"));
        Ok((sh, Vec::new()))
    }
}
