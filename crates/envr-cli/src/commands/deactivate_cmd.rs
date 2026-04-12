//! `envr deactivate` / `envr off` — explain how to leave a hooked project environment.
//!
//! Actual restoration runs in the shell where `eval "$(envr hook …)"` is loaded: the hook script
//! defines an `envr` function that intercepts `deactivate`/`off` and calls `_envr_hook_restore`.

use crate::cli::GlobalArgs;
use crate::commands::common;
use crate::output::{self, fmt_template};

use serde_json::json;

pub fn run(g: &GlobalArgs) -> i32 {
    let data = json!({
        "hint": "hook_shell_only",
        "docs": "After eval \"$(envr hook bash)\" or eval \"$(envr hook zsh)\", run `envr deactivate` or `envr off` to restore saved variables. You can also call `envr_deactivate` if defined.",
    });
    output::emit_ok(g, "deactivate_hint", data, || {
        if !g.quiet {
            eprintln!(
                "{}",
                envr_core::i18n::tr_key(
                    "cli.deactivate.hint",
                    "此命令在已加载 envr hook 的 shell 中生效：请先执行 eval \"$(envr hook bash)\"（或 zsh），再运行 envr deactivate / envr off，或调用 envr_deactivate。",
                    "Use this in a shell where the envr hook is loaded: run eval \"$(envr hook bash)\" (or zsh), then `envr deactivate` / `envr off`, or call `envr_deactivate`.",
                )
            );
            if let Ok(root) = common::effective_runtime_root() {
                let p = root.display().to_string();
                eprintln!(
                    "{}",
                    fmt_template(
                        &envr_core::i18n::tr_key(
                            "cli.deactivate.runtime_root",
                            "当前运行时根目录：{path}",
                            "Runtime root: {path}",
                        ),
                        &[("path", &p)],
                    )
                );
            }
        }
    })
}
