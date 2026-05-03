//! `envr hook` — shell snippets for direnv-style directory activation.
use crate::CliExit;

use crate::CliPathProfile;
use crate::CliUxPolicy;
use crate::cli::GlobalArgs;
use crate::cli::HookShell;
use crate::commands::child_env;
use crate::output;

use envr_error::EnvrResult;

use serde_json::json;
use std::env;
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
    Ok(output::emit_ok(
        g,
        crate::codes::ok::HOOK_KEYS,
        data,
        || {
            if CliUxPolicy::from_global(g).human_text_primary() {
                println!("hook path: {}", session.ctx.working_dir.display());
                for line in hooks {
                    println!("{line}");
                }
            }
        },
    ))
}

pub(crate) fn doctor_inner(g: &GlobalArgs, shell: HookShell, path: PathBuf) -> EnvrResult<CliExit> {
    let session = CliPathProfile::new(path.clone(), None).load_project()?;
    let profile_state = shell_profile_state(shell);
    let body = hook_doctor(
        shell,
        &path,
        &session.ctx.working_dir,
        profile_state.as_deref(),
    );
    let data = json!({
        "shell": format!("{shell:?}").to_lowercase(),
        "path": session.ctx.working_dir.to_string_lossy(),
        "profile_state": profile_state,
        "recommendations": body,
    });
    Ok(output::emit_ok(
        g,
        crate::codes::ok::HOOK_KEYS,
        data,
        || {
            if CliUxPolicy::from_global(g).human_text_primary() {
                for line in body {
                    println!("{line}");
                }
            }
        },
    ))
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

fn hook_doctor(
    shell: HookShell,
    path: &Path,
    root: &Path,
    profile_state: Option<&str>,
) -> Vec<String> {
    let mut lines = vec![format!("hook shell: {shell:?}")];
    lines.push(format!("profile root: {}", root.display()));
    lines.push(format!("cwd: {}", path.display()));
    if let Some(state) = profile_state {
        lines.push(state.to_string());
    }
    lines.push(match shell {
        HookShell::Bash => "next step: eval \"$(envr hook bash)\" in bash".to_string(),
        HookShell::Zsh => "next step: eval \"$(envr hook zsh)\" in zsh".to_string(),
        HookShell::Powershell => {
            "next step: invoke `envr hook powershell` from your PowerShell profile; if needed, check $PROFILE first".to_string()
        }
    });
    lines
}

fn shell_profile_state(shell: HookShell) -> Option<String> {
    match shell {
        HookShell::Powershell => {
            let profile = env::var_os("PROFILE")
                .or_else(|| env::var_os("PSPROFILE"))
                .or_else(|| {
                    env::var_os("USERPROFILE").map(|home| {
                        PathBuf::from(home)
                            .join(r"Documents\PowerShell\Microsoft.PowerShell_profile.ps1")
                            .into_os_string()
                    })
                });
            Some(match profile {
                Some(path) => format!("powershell profile: {}", PathBuf::from(path).display()),
                None => "powershell profile: not detected; run `echo $PROFILE` to inspect it"
                    .to_string(),
            })
        }
        HookShell::Bash => Some(match env::var_os("BASH_VERSION") {
            Some(_) => "bash shell detected: yes".to_string(),
            None => "bash shell detected: not confirmed".to_string(),
        }),
        HookShell::Zsh => Some(match env::var_os("ZSH_VERSION") {
            Some(_) => "zsh shell detected: yes".to_string(),
            None => "zsh shell detected: not confirmed".to_string(),
        }),
    }
}
