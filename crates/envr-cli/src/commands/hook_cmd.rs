//! `envr hook` — shell snippets for direnv-style directory activation.
use crate::CliExit;

use crate::cli::GlobalArgs;
use crate::commands::child_env;
use crate::output;
use crate::CliPathProfile;

use envr_error::EnvrResult;

use serde_json::json;
use std::path::PathBuf;

pub(crate) const HOOK_BASH: &str = include_str!("../../shell/hook.bash.inc");
pub(crate) const HOOK_ZSH: &str = include_str!("../../shell/hook.zsh.inc");

pub(crate) fn emit_hook_script(g: &GlobalArgs, shell: &str, body: &str) -> CliExit {
    let data = json!({
        "shell": shell,
        "script": body,
    });
    output::emit_ok(g, crate::codes::ok::SHELL_HOOK, data, || {
        print!("{body}");
    })
}

/// Body for [`crate::commands::dispatch`]; errors are finished at the dispatch boundary.
pub(crate) fn run_keys_inner(g: &GlobalArgs, path: PathBuf) -> EnvrResult<CliExit> {
    let session = CliPathProfile::new(path, None).load_project()?;
    let keys = child_env::hook_env_restore_keys(&session.ctx)?;
    let data = json!({
        "path": session.ctx.working_dir.to_string_lossy(),
        "keys": keys,
    });
    Ok(output::emit_ok(g, crate::codes::ok::HOOK_KEYS, data, || {
        // Always print one key per line on stdout (used by eval'd hooks); ignore --quiet.
        for k in &keys {
            println!("{k}");
        }
    }))
}
