use crate::CliExit;
use crate::CliPathProfile;
use crate::CliUxPolicy;
use crate::cli::GlobalArgs;
use crate::commands::common;
use crate::output::{self, fmt_template};

use envr_domain::runtime::parse_runtime_kind;
use envr_error::{EnvrError, EnvrResult};
use envr_shim_core::pick_version_home;
use serde_json::json;
use std::path::PathBuf;

fn next_steps_for_check_ok() -> Vec<(&'static str, String)> {
    vec![(
        "project_validate_remote",
        envr_core::i18n::tr_key(
            "cli.next_step.check.project_validate_remote",
            "可执行 `envr project validate --check-remote` 校验远端可用性。",
            "Run `envr project validate --check-remote` to validate remote availability.",
        ),
    )]
}

fn next_steps_for_check_failure() -> Vec<(&'static str, String)> {
    vec![
        (
            "project_sync_install",
            envr_core::i18n::tr_key(
                "cli.next_step.check.project_sync_install",
                "可执行 `envr project sync --install` 安装缺失的项目 pin 运行时。",
                "Run `envr project sync --install` to install missing pinned runtimes.",
            ),
        ),
        (
            "run_doctor",
            envr_core::i18n::tr_key(
                "cli.next_step.check.run_doctor",
                "若仍失败，可执行 `envr doctor` 排查环境问题。",
                "If failures persist, run `envr doctor` for environment diagnostics.",
            ),
        ),
    ]
}

/// Body for [`crate::commands::dispatch`]; errors are finished at the dispatch boundary.
pub(crate) fn run_inner(g: &GlobalArgs, path: PathBuf, github_annotations: bool) -> EnvrResult<CliExit> {
    let session = CliPathProfile::new(path, None).load_project()?;
    let Some((cfg, loc)) = session.project.as_ref() else {
        return Err(EnvrError::Validation(fmt_template(
            &envr_core::i18n::tr_key(
                "cli.err.no_project_config",
                "自 {path} 向上未找到 `.envr.toml` 或 `.envr.local.toml`",
                "no `.envr.toml` or `.envr.local.toml` found searching upward from {path}",
            ),
            &[("path", &session.ctx.working_dir.display().to_string())],
        )));
    };

    let runtime_root = common::session_runtime_root()?;

    let mut problems = Vec::new();
    for (key, rt) in &cfg.runtimes {
        if parse_runtime_kind(key).is_err() {
            problems.push(fmt_template(
                &envr_core::i18n::tr_key(
                    "cli.err.unknown_runtime_key",
                    "未知的运行时键 `{key}`（应为 node、python 或 java）",
                    "unknown runtime key `{key}` (expected node, python, or java)",
                ),
                &[("key", key.as_str())],
            ));
            continue;
        }
        if let Some(spec) = &rt.version {
            let vd = runtime_root.join("runtimes").join(key).join("versions");
            if let Err(e) = pick_version_home(&vd, spec) {
                problems.push(format!("{key}: {e}"));
            }
        }
    }

    if !problems.is_empty() {
        let msg = envr_core::i18n::tr_key(
            "cli.check.fail_heading",
            "项目配置检查失败",
            "project configuration check failed",
        );
        let mut data = json!({
            "config_dir": loc.dir.to_string_lossy(),
            "lock": loc.lock_file.as_ref().map(|p| json!({
                "path": p.to_string_lossy(),
                "version": 1,
                "matched": true,
            })),
            "issues": problems,
            "github_annotations": github_annotations,
        });
        if github_annotations {
            let ann = problems
                .iter()
                .map(|p| format!("::warning file=.envr.toml,line=1,col=1::{p}"))
                .collect::<Vec<_>>();
            data = output::with_next_steps(data, next_steps_for_check_failure());
            if CliUxPolicy::from_global(g).human_text_primary() {
                for line in ann {
                    eprintln!("{line}");
                }
            }
        } else {
            data = output::with_next_steps(data, next_steps_for_check_failure());
        }
        let code = output::emit_failure_envelope(
            g,
            crate::codes::err::PROJECT_CHECK_FAILED,
            &msg,
            data,
            &[],
            1,
        );
        if CliUxPolicy::from_global(g).human_text_primary() {
            for p in &problems {
                eprintln!("envr:   - {p}");
            }
        }
        return Ok(code);
    }

    let mut data = serde_json::json!({
        "config_dir": loc.dir.to_string_lossy(),
        "base_file": loc.base_file.as_ref().map(|p| p.to_string_lossy().to_string()),
        "local_file": loc.local_file.as_ref().map(|p| p.to_string_lossy().to_string()),
        "lock": loc.lock_file.as_ref().map(|p| serde_json::json!({
            "path": p.to_string_lossy(),
            "version": 1,
            "matched": true,
        })),
        "pinned_runtimes": cfg.runtimes.len(),
    });
    data = output::with_next_steps(data, next_steps_for_check_ok());
    Ok(output::emit_ok(
        g,
        crate::codes::ok::PROJECT_CONFIG_OK,
        data,
        || {
            if CliUxPolicy::from_global(g).human_text_primary() {
                println!(
                    "{}",
                    fmt_template(
                        &envr_core::i18n::tr_key(
                            "cli.check.ok",
                            "项目配置正常（根目录 {path}）",
                            "project config ok (root {path})",
                        ),
                        &[("path", &loc.dir.display().to_string())],
                    )
                );
            }
        },
    ))
}
