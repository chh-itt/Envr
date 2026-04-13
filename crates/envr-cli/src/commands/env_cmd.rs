use crate::cli::{EnvShellKind, GlobalArgs, ProjectPathProfileArgs};
use crate::CliPathProfile;
use crate::commands::child_env;
use crate::CommandOutcome;
use crate::output;

use envr_error::EnvrResult;
use serde_json::{Map, Value, json};

fn posix_shell_quote(val: &str) -> String {
    format!("'{}'", val.replace('\'', "'\\''"))
}

fn emit_pair(shell: EnvShellKind, key: &str, val: &str) {
    match shell {
        EnvShellKind::Posix => {
            println!("export {}={}", key, posix_shell_quote(val));
        }
        EnvShellKind::Cmd => {
            println!("set \"{}={}\"", key, val.replace('"', "\"\""));
        }
        EnvShellKind::Powershell => {
            println!("$env:{} = '{}'", key, val.replace('\'', "''"));
        }
    }
}

pub fn run(g: &GlobalArgs, project: ProjectPathProfileArgs, shell: EnvShellKind) -> i32 {
    CommandOutcome::from_result(run_inner(g, project, shell)).finish(g)
}

fn run_inner(
    g: &GlobalArgs,
    project: ProjectPathProfileArgs,
    shell: EnvShellKind,
) -> EnvrResult<i32> {
    let ProjectPathProfileArgs { path, profile } = project;
    let session = CliPathProfile::new(path, profile).load_project()?;
    let env_map = child_env::collect_run_env(&session.ctx, false, session.project_config())?;

    let mut keys: Vec<_> = env_map.keys().cloned().collect();
    keys.sort();

    let shell_str = match shell {
        EnvShellKind::Posix => "posix",
        EnvShellKind::Cmd => "cmd",
        EnvShellKind::Powershell => "powershell",
    };

    let mut vars = Map::new();
    for k in &keys {
        if let Some(v) = env_map.get(k) {
            vars.insert(k.clone(), Value::String(v.clone()));
        }
    }

    let data = json!({
        "shell": shell_str,
        "vars": vars,
    });
    Ok(output::emit_ok(g, "project_env", data, || {
        if !g.quiet {
            for k in &keys {
                if let Some(v) = env_map.get(k) {
                    emit_pair(shell, k, v);
                }
            }
        }
    }))
}
