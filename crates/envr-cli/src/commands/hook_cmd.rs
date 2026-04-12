//! `envr hook` — shell snippets for direnv-style directory activation.

use crate::cli::{GlobalArgs, HookCmd};
use crate::commands::child_env;
use crate::commands::common;
use crate::output;

use serde_json::json;
use std::path::PathBuf;

const HOOK_BASH: &str = include_str!("../../shell/hook.bash.inc");
const HOOK_ZSH: &str = include_str!("../../shell/hook.zsh.inc");

pub fn run(g: &GlobalArgs, sub: HookCmd) -> i32 {
    match sub {
        HookCmd::Bash => emit_hook_script(g, "bash", HOOK_BASH),
        HookCmd::Zsh => emit_hook_script(g, "zsh", HOOK_ZSH),
        HookCmd::Keys { path } => run_keys(g, path),
        HookCmd::Prompt { path, profile } => {
            super::status_cmd::run_hook_prompt(g, path, profile)
        }
    }
}

fn emit_hook_script(g: &GlobalArgs, shell: &str, body: &str) -> i32 {
    let data = json!({
        "shell": shell,
        "script": body,
    });
    output::emit_ok(g, "shell_hook", data, || {
        print!("{body}");
    })
}

fn run_keys(g: &GlobalArgs, path: PathBuf) -> i32 {
    let ctx = match common::shim_context_for(path, None) {
        Ok(c) => c,
        Err(e) => return common::print_envr_error(g, e),
    };
    let keys = match child_env::hook_env_restore_keys(&ctx) {
        Ok(k) => k,
        Err(e) => return common::print_envr_error(g, e),
    };
    let data = json!({
        "path": ctx.working_dir.to_string_lossy(),
        "keys": keys,
    });
    output::emit_ok(g, "hook_keys", data, || {
        // Always print one key per line on stdout (used by eval'd hooks); ignore --quiet.
        for k in &keys {
            println!("{k}");
        }
    })
}
