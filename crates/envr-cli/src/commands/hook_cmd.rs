//! `envr hook` — shell snippets for direnv-style directory activation.
use crate::CliExit;

use crate::CliPathProfile;
use crate::cli::GlobalArgs;
use crate::cli::HookShell;
use crate::commands::child_env;
use crate::output;
use crate::CliUxPolicy;

use envr_error::EnvrResult;

use serde_json::json;
use std::path::{Path, PathBuf};

pub(crate) const HOOK_BASH: &str = include_str!("../../shell/hook.bash.inc");
pub(crate) const HOOK_ZSH: &str = include_str!("../../shell/hook.zsh.inc");
pub(crate) const HOOK_POWERSHELL: &str = include_str!("../../shell/hook.powershell.inc");

pub(crate) fn emit_hook_script(g: &GlobalArgs, shell: &str, body: &str) -> CliExit {
    let data = json!({
        "shell": shell,
        "script": body,
    });
    output::emit_ok(g, crate::codes::ok::SHELL_HOOK, data, || {
        print!("{body}");
    })
}

pub(crate) fn status_inner(g: &GlobalArgs, path: PathBuf) -> EnvrResult<CliExit> {
    let session = CliPathProfile::new(path.clone(), None).load_project()?;
    let hooks = hook_status(&path, &session.ctx.working_dir);
    let data = json!({
        "path": session.ctx.working_dir.to_string_lossy(),
        "hooks": hooks,
    });
    Ok(output::emit_ok(g, crate::codes::ok::HOOK_KEYS, data, || {
        if CliUxPolicy::from_global(g).human_text_primary() {
            println!("hook path: {}", session.ctx.working_dir.display());
            for line in hooks {
                println!("{line}");
            }
        }
    }))
}

pub(crate) fn doctor_inner(g: &GlobalArgs, shell: HookShell, path: PathBuf) -> EnvrResult<CliExit> {
    let session = CliPathProfile::new(path.clone(), None).load_project()?;
    let body = hook_doctor(shell, &path, &session.ctx.working_dir);
    let data = json!({
        "shell": format!("{shell:?}").to_lowercase(),
        "path": session.ctx.working_dir.to_string_lossy(),
        "recommendations": body,
    });
    Ok(output::emit_ok(g, crate::codes::ok::HOOK_KEYS, data, || {
        if CliUxPolicy::from_global(g).human_text_primary() {
            for line in body {
                println!("{line}");
            }
        }
    }))
}

/// Body for [`crate::commands::dispatch`]; errors are finished at the dispatch boundary.
pub(crate) fn run_keys_inner(g: &GlobalArgs, path: PathBuf) -> EnvrResult<CliExit> {
    let session = CliPathProfile::new(path, None).load_project()?;
    let keys = child_env::hook_env_restore_keys(&session.ctx)?;
    let data = json!({
        "path": session.ctx.working_dir.to_string_lossy(),
        "keys": keys,
    });
    Ok(output::emit_ok(
        g,
        crate::codes::ok::HOOK_KEYS,
        data,
        || {
            // Always print one key per line on stdout (used by eval'd hooks); ignore --quiet.
            for k in &keys {
                println!("{k}");
            }
        },
    ))
}

fn hook_status(path: &Path, root: &Path) -> Vec<String> {
    let mut lines = vec![format!("selected profile root: {}", root.display())];
    let mut candidates = vec![path.join(".envr.toml"), path.join(".envr.local.toml")];
    if let Some(parent) = path.parent() {
        candidates.push(parent.join(".envr.toml"));
        candidates.push(parent.join(".envr.local.toml"));
    }
    for candidate in candidates {
        if candidate.is_file() {
            lines.push(format!("found profile: {}", candidate.display()));
        }
    }
    lines
}

fn hook_doctor(shell: HookShell, path: &Path, root: &Path) -> Vec<String> {
    let mut lines = vec![format!("hook shell: {shell:?}")];
    lines.push(format!("profile root: {}", root.display()));
    lines.push(format!("cwd: {}", path.display()));
    lines.push(match shell {
        HookShell::Bash => "next step: eval \"$(envr hook bash)\" in bash".to_string(),
        HookShell::Zsh => "next step: eval \"$(envr hook zsh)\" in zsh".to_string(),
        HookShell::Powershell => {
            "next step: add `envr hook powershell` snippet to your PowerShell profile".to_string()
        }
    });
    lines
}
