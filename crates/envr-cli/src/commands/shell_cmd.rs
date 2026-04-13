//! Interactive subshell with the same merged environment as `envr env` / `collect_run_env`.

use crate::cli::{GlobalArgs, ProjectPathProfileArgs};
use crate::CliPathProfile;
use crate::commands::child_env;
use crate::CommandOutcome;
use crate::output::{self, fmt_template};

use envr_error::EnvrResult;
use serde_json::json;
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::Command;

pub fn run(g: &GlobalArgs, project: ProjectPathProfileArgs, shell: Option<PathBuf>) -> i32 {
    CommandOutcome::from_result(run_inner(g, project, shell)).finish(g)
}

fn run_inner(
    g: &GlobalArgs,
    project: ProjectPathProfileArgs,
    shell: Option<PathBuf>,
) -> EnvrResult<i32> {
    let ProjectPathProfileArgs { path, profile } = project;
    let session = CliPathProfile::new(path, profile).load_project()?;
    let ctx = &session.ctx;

    let env_map = child_env::collect_run_env(ctx, false, session.project_config())?;

    let (program, extra_args) = resolve_shell_invocation(shell)?;

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

    let status = cmd.status()?;
    let code = status.code().unwrap_or(1);
    if code == 0 {
        Ok(output::emit_ok(g, "shell_exited", base_data, || {}))
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
        Ok(output::emit_failure_envelope(
            g, "shell_exit", &msg, fail_data, &[], code,
        ))
    }
}

fn resolve_shell_invocation(
    override_path: Option<PathBuf>,
) -> Result<(PathBuf, Vec<OsString>), envr_error::EnvrError> {
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
