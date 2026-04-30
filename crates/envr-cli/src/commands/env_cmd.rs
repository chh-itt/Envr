use crate::CliExit;
use crate::CliPathProfile;
use crate::CliUxPolicy;
use crate::cli::{EnvShellKind, GlobalArgs, ProjectPathProfileArgs};
use crate::commands::child_env;
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

/// Body for [`crate::commands::dispatch`]; errors are finished at the dispatch boundary.
pub(crate) fn run_inner(
    g: &GlobalArgs,
    project: ProjectPathProfileArgs,
    shell: EnvShellKind,
    diff: bool,
) -> EnvrResult<CliExit> {
    let ProjectPathProfileArgs { path, profile } = project;
    let session = CliPathProfile::new(path, profile).load_project()?;
    let env_map = child_env::collect_run_env(&session.ctx, false, session.project_config())?;
    let diff_map = if diff {
        let mut out = Map::new();
        let current: std::collections::HashMap<String, String> = std::env::vars().collect();
        for (k, v) in &env_map {
            if current.get(k) != Some(v) {
                out.insert(k.clone(), Value::String(v.clone()));
            }
        }
        out
    } else {
        Map::new()
    };

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
        "vars": if diff { Value::Object(diff_map) } else { Value::Object(vars) },
        "diff": diff,
    });
    Ok(output::emit_ok(
        g,
        crate::codes::ok::PROJECT_ENV,
        data,
        || {
            if CliUxPolicy::from_global(g).human_text_primary() {
                for k in &keys {
                    if let Some(v) = env_map.get(k) {
                        emit_pair(shell, k, v);
                    }
                }
            }
        },
    ))
}
