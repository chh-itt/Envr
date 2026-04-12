//! `envr status` — project + active runtime summary.

use crate::cli::GlobalArgs;
use crate::commands::common;
use crate::commands::project_status::{build_project_status, format_prompt_segment, status_to_json};
use crate::output::{self, fmt_template};

use serde_json::json;
use std::path::PathBuf;

pub fn run(g: &GlobalArgs, path: PathBuf, profile: Option<String>) -> i32 {
    let ctx = match common::shim_context_for(path, profile) {
        Ok(c) => c,
        Err(e) => return common::print_envr_error(g, e),
    };
    let st = match build_project_status(&ctx) {
        Ok(s) => s,
        Err(e) => return common::print_envr_error(g, e),
    };
    let data = status_to_json(&st);
    output::emit_ok(g, "project_status", data, || {
        if g.quiet {
            return;
        }
        println!(
            "{}",
            fmt_template(
                &envr_core::i18n::tr_key(
                    "cli.status.working_dir",
                    "工作目录：{path}",
                    "Working directory: {path}",
                ),
                &[("path", &st.working_dir.display().to_string())],
            )
        );
        if let Some(ref p) = st.project_dir {
            println!(
                "{}",
                fmt_template(
                    &envr_core::i18n::tr_key(
                        "cli.status.project",
                        "项目根：{path}",
                        "Project: {path}",
                    ),
                    &[("path", &p.display().to_string())],
                )
            );
        } else {
            println!(
                "{}",
                envr_core::i18n::tr_key(
                    "cli.status.no_project",
                    "未找到 `.envr.toml`（自当前目录向上搜索）。",
                    "No `.envr.toml` found (searching upward from the working directory).",
                )
            );
            println!(
                "{}",
                envr_core::i18n::tr_key(
                    "cli.status.no_project_hint1",
                    "提示：可运行 `envr init` 添加项目配置，或 `envr doctor` 检查本机环境。",
                    "Tip: run `envr init` to add a project config, or `envr doctor` to verify your setup.",
                )
            );
            println!(
                "{}",
                envr_core::i18n::tr_key(
                    "cli.status.no_project_hint2",
                    "有项目后可用 `envr project sync` 按 pin 安装/对齐运行时版本。",
                    "Once a project exists, use `envr project sync` to install or align versions with pins.",
                )
            );
        }
        if let Some(ref prof) = st.profile {
            println!(
                "{}",
                fmt_template(
                    &envr_core::i18n::tr_key(
                        "cli.status.profile",
                        "Profile：{name}",
                        "Profile: {name}",
                    ),
                    &[("name", prof.as_str())],
                )
            );
        }
        println!();
        for r in &st.rows {
            if !r.ok {
                let detail = r.detail.as_deref().unwrap_or("");
                println!(
                    "{}",
                    fmt_template(
                        &envr_core::i18n::tr_key(
                            "cli.status.row_err",
                            "  {kind}: 无法解析（{detail}）",
                            "  {kind}: not resolved ({detail})",
                        ),
                        &[("kind", r.kind.as_str()), ("detail", detail)],
                    )
                );
                continue;
            }
            let line = match r.source {
                envr_shim_core::WhichRuntimeSource::PathProxyBypass => fmt_template(
                    &envr_core::i18n::tr_key(
                        "cli.status.row_sys",
                        "  {kind}: {ver}（系统 PATH）",
                        "  {kind}: {ver} (system PATH)",
                    ),
                    &[("kind", r.kind.as_str()), ("ver", r.active_version.as_str())],
                ),
                envr_shim_core::WhichRuntimeSource::ProjectPin => match &r.pin {
                    Some(p) => fmt_template(
                        &envr_core::i18n::tr_key(
                            "cli.status.row_project_spec",
                            "  {kind}: {ver}（项目 pin：{spec}）",
                            "  {kind}: {ver} (project pin: {spec})",
                        ),
                        &[
                            ("kind", r.kind.as_str()),
                            ("ver", r.active_version.as_str()),
                            ("spec", p.as_str()),
                        ],
                    ),
                    None => fmt_template(
                        &envr_core::i18n::tr_key(
                            "cli.status.row_project",
                            "  {kind}: {ver}（项目配置）",
                            "  {kind}: {ver} (from project)",
                        ),
                        &[("kind", r.kind.as_str()), ("ver", r.active_version.as_str())],
                    ),
                },
                envr_shim_core::WhichRuntimeSource::GlobalCurrent => fmt_template(
                    &envr_core::i18n::tr_key(
                        "cli.status.row_global",
                        "  {kind}: {ver}（全局 current）",
                        "  {kind}: {ver} (global current)",
                    ),
                    &[("kind", r.kind.as_str()), ("ver", r.active_version.as_str())],
                ),
            };
            println!("{line}");
        }
    })
}

/// `envr hook prompt` — one line for PS1 (plain text); JSON envelope when `--format json`.
pub fn run_hook_prompt(g: &GlobalArgs, path: PathBuf, profile: Option<String>) -> i32 {
    let ctx = match common::shim_context_for(path, profile) {
        Ok(c) => c,
        Err(e) => return common::print_envr_error(g, e),
    };
    let st = match build_project_status(&ctx) {
        Ok(s) => s,
        Err(e) => return common::print_envr_error(g, e),
    };
    let segment = format_prompt_segment(&st);
    let data = json!({ "segment": segment });
    output::emit_ok(g, "hook_prompt", data, || {
        print!("{segment}");
    })
}
