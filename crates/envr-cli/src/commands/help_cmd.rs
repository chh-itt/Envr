//! Supplemental CLI help (`envr help …`).

use crate::cli::{GlobalArgs, HelpCmd, OutputFormat};
use crate::CommandOutcome;
use crate::output;
use envr_error::EnvrResult;
use serde_json::json;

pub fn run(g: &GlobalArgs, cmd: HelpCmd) -> i32 {
    match cmd {
        HelpCmd::Shortcuts => CommandOutcome::from_result(shortcuts_inner(g)).finish(g),
    }
}

fn shortcuts_inner(g: &GlobalArgs) -> EnvrResult<i32> {
    let note = envr_core::i18n::tr_key(
        "cli.help.shortcuts.note",
        "以上在 clap 解析之前改写 argv。用户自定义名称见 runtime root 下 config/aliases.toml（优先级高于内置简写）。",
        "These rewrite argv before clap parses. User-defined names live in `config/aliases.toml` under the runtime root and override built-ins.",
    );
    let rows: Vec<_> = crate::cli::BUILTIN_ARGV_SHORTHANDS
        .iter()
        .map(|(a, b)| json!({ "argv_token": a, "expands_to": b }))
        .collect();
    let data = json!({
        "builtin_shorthands": rows,
        "note": note,
    });
    Ok(output::emit_ok(g, "help_shortcuts", data, || {
        if g.quiet {
            return;
        }
        println!(
            "{}",
            envr_core::i18n::tr_key(
                "cli.help.shortcuts.title",
                "内置 argv 简写（preprocess_cli_args）",
                "Built-in argv shorthands (preprocess_cli_args)",
            )
        );
        for (from, to) in crate::cli::BUILTIN_ARGV_SHORTHANDS {
            println!("  {from:<28} → {to}");
        }
        println!();
        println!("{note}");
        if matches!(
            g.effective_output_format(),
            OutputFormat::Text
        ) {
            println!(
                "{}",
                envr_core::i18n::tr_key(
                    "cli.help.shortcuts.completion_hint",
                    "补全脚本在文件头注释中指向本主题：`envr completion <shell>`",
                    "Completion scripts include a header comment pointing here: `envr completion <shell>`",
                )
            );
        }
    }))
}
