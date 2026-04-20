//! Localized `--help` copy for the clap tree, driven by static path-keyed tables (no `match` on command name).
//!
//! Each [`CmdHelpSpec`] is keyed by the exact subcommand path from the root (e.g. `&["config", "get"]`).
//! Keep this in sync with [`super::Cli`]'s `Subcommand` tree and [`super::command_spec::CommandSpec::help_path`].

use clap::Command;

#[derive(Clone, Copy)]
pub(crate) struct I18n(pub &'static str, pub &'static str, pub &'static str);

pub(crate) struct ArgHelpSpec {
    pub id: &'static str,
    pub help: I18n,
    pub long_help: Option<I18n>,
}

pub(crate) struct CmdHelpSpec {
    /// Path from the root `envr` command: `["hook", "bash"]`, `["install"]`, etc.
    pub path: &'static [&'static str],
    pub about: I18n,
    pub after_long_help: Option<I18n>,
    pub args: &'static [ArgHelpSpec],
}

#[inline]
fn tr(i: I18n) -> String {
    envr_core::i18n::tr_key(i.0, i.1, i.2)
}

fn apply_cmd_spec(cmd: &mut Command, spec: &CmdHelpSpec) {
    let mut next = cmd.clone().about(tr(spec.about));
    if let Some(al) = spec.after_long_help {
        next = next.after_long_help(tr(al));
    }
    for a in spec.args {
        let help = a.help;
        let long = a.long_help;
        next = next.mut_arg(a.id, |arg| {
            let mut arg = arg.help(tr(help));
            if let Some(lh) = long {
                arg = arg.long_help(tr(lh));
            }
            arg
        });
    }
    *cmd = next;
}

fn spec_for_path(path: &[String]) -> Option<&'static CmdHelpSpec> {
    HELP_BY_PATH.iter().find(|row| {
        row.path.len() == path.len()
            && row
                .path
                .iter()
                .zip(path.iter())
                .all(|(seg, got)| *seg == got.as_str())
    })
}

fn walk_apply(cmd: &mut Command, path: &mut Vec<String>) {
    if let Some(spec) = spec_for_path(path) {
        apply_cmd_spec(cmd, spec);
    }
    for nested in cmd.get_subcommands_mut() {
        path.push(nested.get_name().to_string());
        walk_apply(nested, path);
        path.pop();
    }
}

/// Root `envr` about, long help sections, and global flags (same keys as legacy `cli_help::patch_root`).
pub(crate) fn apply_root_help(cmd: &mut Command) {
    *cmd = cmd
        .clone()
        .about(tr(I18n(
            "cli.help.about",
            "语言运行时版本管理器",
            "Language runtime version manager",
        )))
        .after_long_help(format!(
            "{}\n\n{}",
            tr(I18n(
                "cli.help.command_groups",
                "命令分组（与上方列表顺序一致）：\n  • 运行时管理 — install / use / list / current / uninstall / which / remote / rust / why / resolve / exec / run / env / template / shell / hook / deactivate / prune\n  • 项目与配置 — init / check / status / project / import / export / profile / config / alias\n  • 数据与环境 — shim / cache / bundle\n  • 诊断与信息 — doctor / debug / diagnostics / completion / help / update",
                "Command groups (same order as the list above):\n  • Runtime management — install, use, list, current, uninstall, which, remote, rust, why, resolve, exec, run, env, template, shell, hook, deactivate, prune\n  • Project & configuration — init, check, status, project, import, export, profile, config, alias\n  • Data & environment — shim, cache, bundle\n  • Diagnostics & information — doctor, debug, diagnostics, completion, help, update",
            )),
            tr(I18n(
                "cli.help.command_tiers",
                "命令层级（与 CLI 设计文档 §2 一致；完整对照表见 docs/cli/commands.md）：\n  • L1 核心 — install · use · list · current · uninstall · which · remote · doctor\n  • L2 增强 — config · alias · prune · update · resolve · shell；另含 init · check · project · hook · deactivate · why · rust\n  • L3 自动化 — exec · run · env · import · export · profile · status · template\n  • 平台与数据 — shim · cache · bundle · debug · diagnostics · completion · help · update",
                "Design tiers (CLI design doc §2; full matrix: docs/cli/commands.md):\n  • L1 essential — install, use, list, current, uninstall, which, remote, doctor\n  • L2 enhanced — config, alias, prune, update, resolve, shell; also init, check, project, hook, deactivate, why, rust\n  • L3 automation — exec, run, env, import, export, profile, status, template\n  • Platform & data — shim, cache, bundle, debug, diagnostics, completion, help, update",
            )),
        ))
        .mut_arg("output_format", |a| {
            a.help(tr(I18n(
                "cli.help.global.format",
                "输出格式（text 或 json）。默认：text。",
                "Output format (`text` or `json`). Default: `text`.",
            )))
            .long_help(tr(I18n(
                "cli.help.global.format_long",
                "text：人类可读文本；json：单行 JSON 信封，便于脚本解析。",
                "`text`: human-readable. `json`: one JSON line per envelope for automation.",
            )))
        })
        .mut_arg("porcelain", |a| {
            a.help(tr(I18n(
                "cli.help.global.porcelain",
                "脚本友好纯文本输出（无标签/装饰，等价 --plain）。",
                "Script-friendly plain text output (no labels/decorations; alias: --plain).",
            )))
        })
        .mut_arg("quiet", |a| {
            a.help(tr(I18n(
                "cli.help.global.quiet",
                "抑制非错误输出。",
                "Suppress non-error output.",
            )))
            .long_help(tr(I18n(
                "cli.help.global.quiet_long",
                "开启时，text 模式下的 envr 自身错误仅打印一行 `[E_*]` 标签（便于脚本 grep）；JSON 信封的 message 也缩短为同一标签。",
                "In `text` mode, envr errors print only one `[E_*]` line (easy to grep); in `json` mode the envelope `message` is shortened to the same tag.",
            )))
        })
        .mut_arg("no_color", |a| {
            a.help(tr(I18n(
                "cli.help.global.no_color",
                "禁用终端 ANSI 颜色。",
                "Disable ANSI color in terminal output.",
            )))
        })
        .mut_arg("runtime_root", |a| {
            a.help(tr(I18n(
                "cli.help.global.runtime_root",
                "覆盖运行时根目录（设置 ENVR_RUNTIME_ROOT）。",
                "Override runtime root directory (sets `ENVR_RUNTIME_ROOT`).",
            )))
        })
        .mut_arg("debug", |a| {
            a.help(tr(I18n(
                "cli.help.global.debug",
                "调试：tracing 输出到 stderr，且未设置 RUST_LOG 时默认为 debug。",
                "Debug: emit tracing to stderr; default `RUST_LOG=debug` when unset.",
            )))
        });
}

/// Apply localized about / long help / per-arg help for every subcommand node.
pub(crate) fn apply_subcommand_tree_help(cmd: &mut Command) {
    for sc in cmd.get_subcommands_mut() {
        let mut path = vec![sc.get_name().to_string()];
        walk_apply(sc, &mut path);
    }
}

// --- Static registry (single source for localized help strings) ---

include!("table.inc");

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::Cli;
    use crate::cli::metadata::{all_command_keys, metadata_for_key};
    use clap::CommandFactory as _;
    use std::collections::HashSet;

    #[test]
    fn help_paths_are_unique() {
        let mut seen = HashSet::new();
        for row in HELP_BY_PATH {
            let key = row.path.join("/");
            assert!(seen.insert(key.clone()), "duplicate help path `{key}`");
        }
    }

    #[test]
    fn every_command_key_has_help_spec() {
        for key in all_command_keys() {
            let path = metadata_for_key(key).help_path;
            assert!(
                HELP_BY_PATH.iter().any(|r| r.path == path),
                "missing help spec for CommandKey::{key:?} path {path:?}"
            );
        }
    }

    fn collect_paths(cmd: &clap::Command, out: &mut Vec<Vec<String>>, prefix: &mut Vec<String>) {
        for sc in cmd.get_subcommands() {
            prefix.push(sc.get_name().to_string());
            out.push(prefix.clone());
            collect_paths(sc, out, prefix);
            prefix.pop();
        }
    }

    #[test]
    fn every_cli_subcommand_path_has_help_spec() {
        let mut cmd = Cli::command();
        apply_root_help(&mut cmd);
        apply_subcommand_tree_help(&mut cmd);

        let mut paths = Vec::new();
        collect_paths(&cmd, &mut paths, &mut Vec::new());

        for path in &paths {
            assert!(
                spec_for_path(path).is_some(),
                "clap has subcommand path {:?} but HELP_BY_PATH has no entry",
                path
            );
        }
    }
}
