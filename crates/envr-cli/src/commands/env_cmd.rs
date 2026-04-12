use crate::cli::EnvShellKind;
use crate::cli::GlobalArgs;
use crate::commands::child_env;
use crate::commands::common;
use crate::output;

use serde_json::{Map, Value, json};
use std::path::PathBuf;

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

pub fn run(g: &GlobalArgs, path: PathBuf, profile: Option<String>, shell: EnvShellKind) -> i32 {
    let ctx = match common::shim_context_for(path, profile) {
        Ok(c) => c,
        Err(e) => return common::print_envr_error(g, e),
    };

    let env_map = match child_env::collect_run_env(&ctx, false) {
        Ok(m) => m,
        Err(e) => return common::print_envr_error(g, e),
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
        "vars": vars,
    });
    output::emit_ok(g, "project_env", data, || {
        if !g.quiet {
            for k in &keys {
                if let Some(v) = env_map.get(k) {
                    emit_pair(shell, k, v);
                }
            }
        }
    })
}
