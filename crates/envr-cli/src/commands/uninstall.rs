use crate::cli::{GlobalArgs, OutputFormat};
use crate::commands::common::{self, kind_label};
use crate::output::{self, fmt_template, print_error_text, write_envelope};

use envr_core::runtime::service::RuntimeService;
use envr_domain::runtime::{RuntimeKind, RuntimeVersion, parse_runtime_kind};
use envr_error::EnvrError;
use serde_json::json;
use std::io::{self, IsTerminal, Write};

pub fn run(
    g: &GlobalArgs,
    service: &RuntimeService,
    runtime: String,
    runtime_version: String,
    dry_run: bool,
    force: bool,
    yes: bool,
) -> i32 {
    let kind = match parse_runtime_kind(runtime.trim()) {
        Ok(k) => k,
        Err(e) => return common::print_envr_error(g, e),
    };

    let version = RuntimeVersion(runtime_version);

    let current = match service.current(kind) {
        Ok(c) => c,
        Err(e) => return common::print_envr_error(g, e),
    };

    let is_active = current
        .as_ref()
        .is_some_and(|c| c.0 == version.0);

    let (paths, external) = match service.uninstall_dry_run_targets(kind, &version) {
        Ok(p) => p,
        Err(e) => return common::print_envr_error(g, e),
    };

    if dry_run {
        let would_refuse = is_active && !force;
        let data = json!({
            "kind": kind_label(kind),
            "version": version.0,
            "would_refuse_active_without_force": would_refuse,
            "paths": paths.iter().map(|p| p.display().to_string()).collect::<Vec<_>>(),
            "external_command": external,
        });
        let msg = envr_core::i18n::tr_key(
            "cli.uninstall.dry_run_message",
            "卸载预演",
            "uninstall dry-run",
        );
        if would_refuse {
            let refuse_msg = envr_core::i18n::tr_key(
                "cli.uninstall.err_active",
                "无法卸载当前全局激活版本 {kind} {version}。请先 `envr use` 切换到其他版本，或添加 `--force`。",
                "refusing to uninstall active {kind} {version}: switch away with `envr use`, or pass `--force`.",
            );
            let refuse_msg = fmt_template(
                &refuse_msg,
                &[
                    ("kind", kind_label(kind)),
                    ("version", &version.0),
                ],
            );
            match g.output_format.unwrap_or(OutputFormat::Text) {
                OutputFormat::Json => {
                    write_envelope(false, Some("validation"), &refuse_msg, data, &[]);
                }
                OutputFormat::Text => {
                    print_dry_run_text(g, &paths, external.as_deref());
                    print_error_text("validation", &refuse_msg);
                }
            }
            return 1;
        }

        return output::emit_ok(g, &msg, data, || {
            print_dry_run_text(g, &paths, external.as_deref());
        });
    }

    if is_active && !force {
        let msg = envr_core::i18n::tr_key(
            "cli.uninstall.err_active",
            "无法卸载当前全局激活版本 {kind} {version}。请先 `envr use` 切换到其他版本，或添加 `--force`。",
            "refusing to uninstall active {kind} {version}: switch away with `envr use`, or pass `--force`.",
        );
        let msg = fmt_template(
            &msg,
            &[
                ("kind", kind_label(kind)),
                ("version", &version.0),
            ],
        );
        return common::print_envr_error(
            g,
            EnvrError::Validation(msg),
        );
    }

    if !yes {
        if matches!(g.output_format, Some(OutputFormat::Json)) {
            return output::emit_validation(
                g,
                "uninstall",
                "envr uninstall --yes node 20.0.0",
            );
        }
        if !io::stdin().is_terminal() {
            return output::emit_validation(
                g,
                "uninstall",
                "envr uninstall --yes node 20.0.0",
            );
        }
        let prompt = fmt_template(
            &envr_core::i18n::tr_key(
                "cli.uninstall.prompt",
                "确定要卸载 {kind} {version} 吗？ [y/N] ",
                "Remove {kind} {version}? [y/N] ",
            ),
            &[
                ("kind", kind_label(kind)),
                ("version", &version.0),
            ],
        );
        let _ = io::stderr().write_all(prompt.as_bytes());
        let _ = io::stderr().flush();
        let mut line = String::new();
        if io::stdin().read_line(&mut line).is_err() {
            let aborted = envr_core::i18n::tr_key("cli.uninstall.aborted", "已取消", "aborted");
            print_error_text("aborted", &aborted);
            return 1;
        }
        let ok = matches!(
            line.trim().to_ascii_lowercase().as_str(),
            "y" | "yes"
        );
        if !ok {
            let aborted = envr_core::i18n::tr_key("cli.uninstall.aborted", "已取消", "aborted");
            print_error_text("aborted", &aborted);
            return 1;
        }
    }

    if crate::commands::cli_install_progress::wants_cli_text_feedback(g) {
        let msg = fmt_template(
            &envr_core::i18n::tr_key(
                "cli.uninstall.removing",
                "正在卸载 {kind} {version}…",
                "Removing {kind} {version}…",
            ),
            &[
                ("kind", kind_label(kind)),
                ("version", &version.0),
            ],
        );
        let _ = writeln!(io::stderr(), "{msg}");
    }

    match service.uninstall(kind, &version) {
        Ok(()) => print_success(g, kind, &version),
        Err(e) => common::print_envr_error(g, e),
    }
}

fn print_dry_run_text(
    g: &GlobalArgs,
    paths: &[std::path::PathBuf],
    external: Option<&str>,
) {
    let header = envr_core::i18n::tr_key(
        "cli.uninstall.dry_run_header",
        "将删除以下内容：",
        "Would remove:",
    );
    if output::use_terminal_styles(g) {
        println!("\x1b[1m{header}\x1b[0m");
    } else {
        println!("{header}");
    }
    for p in paths {
        println!("  {}", p.display());
    }
    if let Some(cmd) = external {
        let hint = envr_core::i18n::tr_key(
            "cli.uninstall.dry_run_external",
            "将执行：{cmd}",
            "Would run: {cmd}",
        );
        println!("{}", fmt_template(&hint, &[("cmd", cmd)]));
    }
}

fn print_success(g: &GlobalArgs, kind: RuntimeKind, v: &RuntimeVersion) -> i32 {
    let data = json!({
        "kind": kind_label(kind),
        "version": v.0,
    });
    output::emit_ok(g, "uninstalled", data, || {
        if !g.quiet {
            println!(
                "{}",
                fmt_template(
                    &envr_core::i18n::tr_key(
                        "cli.uninstall.ok",
                        "已卸载 {kind} {version}",
                        "{kind} {version} uninstalled",
                    ),
                    &[("kind", kind_label(kind)), ("version", &v.0)],
                )
            );
        }
    })
}
