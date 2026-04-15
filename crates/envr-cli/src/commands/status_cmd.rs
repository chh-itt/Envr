//! `envr status` — project + active runtime summary.
use crate::CliExit;
use crate::CliUxPolicy;

use crate::CliPathProfile;
use crate::cli::{GlobalArgs, ProjectPathProfileArgs};
use crate::commands::project_status::{
    build_project_status_from_loaded, format_prompt_segment, status_to_json,
};
use crate::output::{self, fmt_template};

use envr_error::EnvrResult;
use serde_json::json;

fn next_steps_for_status(
    st: &crate::commands::project_status::ProjectStatus,
) -> Vec<(&'static str, String)> {
    let mut steps = Vec::new();
    if st.project_dir.is_none() {
        steps.push((
            "init_project_config",
            envr_core::i18n::tr_key(
                "cli.next_step.status.init_project",
                "可执行 `envr init` 创建项目配置。",
                "Run `envr init` to create project configuration.",
            ),
        ));
    } else {
        steps.push((
            "check_project_health",
            envr_core::i18n::tr_key(
                "cli.next_step.status.check_project_health",
                "可执行 `envr check` 检查项目 pin 与本地运行时是否匹配。",
                "Run `envr check` to verify project pins against local runtimes.",
            ),
        ));
    }
    if st.rows.iter().any(|r| !r.ok) {
        steps.push((
            "run_doctor",
            envr_core::i18n::tr_key(
                "cli.next_step.status.run_doctor",
                "存在未解析运行时，建议执行 `envr doctor` 进一步诊断。",
                "Some runtimes are unresolved; run `envr doctor` for deeper diagnostics.",
            ),
        ));
    }
    steps
}
/// Body for [`crate::commands::dispatch`]; errors are finished at the dispatch boundary.
pub(crate) fn run_inner(g: &GlobalArgs, project: ProjectPathProfileArgs) -> EnvrResult<CliExit> {
    let ProjectPathProfileArgs { path, profile } = project;
    let session = CliPathProfile::new(path, profile).load_project()?;
    let st = build_project_status_from_loaded(&session.ctx, &session.project)?;
    let mut data = status_to_json(&st);
    data = output::with_next_steps(data, next_steps_for_status(&st));
    Ok(output::emit_ok(
        g,
        crate::codes::ok::PROJECT_STATUS,
        data,
        || {
            if !CliUxPolicy::from_global(g).human_text_primary() {
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
                        &[
                            ("kind", r.kind.as_str()),
                            ("ver", r.active_version.as_str()),
                        ],
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
                            &[
                                ("kind", r.kind.as_str()),
                                ("ver", r.active_version.as_str()),
                            ],
                        ),
                    },
                    envr_shim_core::WhichRuntimeSource::GlobalCurrent => fmt_template(
                        &envr_core::i18n::tr_key(
                            "cli.status.row_global",
                            "  {kind}: {ver}（全局 current）",
                            "  {kind}: {ver} (global current)",
                        ),
                        &[
                            ("kind", r.kind.as_str()),
                            ("ver", r.active_version.as_str()),
                        ],
                    ),
                };
                println!("{line}");
            }
        },
    ))
}

/// Body for [`crate::commands::dispatch`]; errors are finished at the dispatch boundary.
pub(crate) fn run_hook_prompt_inner(
    g: &GlobalArgs,
    project: ProjectPathProfileArgs,
) -> EnvrResult<CliExit> {
    let ProjectPathProfileArgs { path, profile } = project;
    let session = CliPathProfile::new(path, profile).load_project()?;
    let st = build_project_status_from_loaded(&session.ctx, &session.project)?;
    let segment = format_prompt_segment(&st);
    let data = json!({ "segment": segment });
    Ok(output::emit_ok(
        g,
        crate::codes::ok::HOOK_PROMPT,
        data,
        || {
            print!("{segment}");
        },
    ))
}
