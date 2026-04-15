//! Explain runtime resolution for the current project directory (`envr why <runtime>`).
use crate::CliExit;
use crate::CliUxPolicy;

use crate::CliPathProfile;
use crate::cli::{GlobalArgs, ProjectPathProfileArgs};
use crate::output::{self, fmt_template};

use envr_domain::runtime::{RuntimeKind, parse_runtime_kind};
use envr_error::EnvrError;
use envr_shim_core::resolve_runtime_home_for_lang_with_project;
use serde_json::json;
/// Body for [`crate::commands::dispatch`]; errors are finished at the dispatch boundary.
pub(crate) fn run_inner(
    g: &GlobalArgs,
    runtime: String,
    spec: Option<String>,
    project: ProjectPathProfileArgs,
) -> envr_error::EnvrResult<CliExit> {
    let ProjectPathProfileArgs { path, profile } = project;
    let lang = runtime.trim().to_ascii_lowercase();
    let kind = parse_runtime_kind(&lang)?;

    if kind == RuntimeKind::Rust {
        let msg = envr_core::i18n::tr_key(
            "cli.why.rust_unsupported",
            "Rust 由 envr 托管的 rustup 解析；请使用 `envr rust` / `rustup show` 查看工具链。",
            "Rust is resolved via envr-managed rustup; use `envr rust` / `rustup show` for toolchain details.",
        );
        return Err(EnvrError::Validation(msg));
    }

    let spec_trim = spec
        .as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());
    let spec_deref = spec_trim.as_deref();

    let session = CliPathProfile::new(path, profile).load_project()?;
    let loaded = &session.project;
    let cfg = session.project_config();

    let pin = loaded.as_ref().and_then(|(c, _)| {
        c.runtimes
            .get(&lang)
            .and_then(|r| r.version.as_ref())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    });

    let resolution = if spec_deref.is_some() {
        "spec_override"
    } else if pin.is_some() {
        "project_pin"
    } else {
        "global_current"
    };

    let home = resolve_runtime_home_for_lang_with_project(&session.ctx, &lang, spec_deref, cfg)?;
    let home = std::fs::canonicalize(&home).unwrap_or(home);

    let project_json = loaded.as_ref().map(|(_, loc)| {
        json!({
            "config_dir": loc.dir.to_string_lossy(),
            "base_file": loc.base_file.as_ref().map(|p| p.to_string_lossy().to_string()),
            "local_file": loc.local_file.as_ref().map(|p| p.to_string_lossy().to_string()),
            "pin": pin.clone(),
        })
    });

    let data = json!({
        "lang": lang,
        "working_dir": session.ctx.working_dir.to_string_lossy(),
        "profile": session.ctx.profile,
        "spec_override": spec_trim.clone(),
        "project": project_json,
        "resolution": resolution,
        "resolved_home": home.to_string_lossy(),
    });

    Ok(output::emit_ok(g, crate::codes::ok::WHY_RUNTIME, data, || {
        if CliUxPolicy::from_global(g).human_text_primary() {
            println!(
                "{}",
                fmt_template(
                    &envr_core::i18n::tr_key(
                        "cli.why.working_dir",
                        "工作目录：{path}",
                        "Working directory: {path}",
                    ),
                    &[("path", &session.ctx.working_dir.display().to_string())],
                )
            );
            if let Some((_, loc)) = loaded {
                println!(
                    "{}",
                    fmt_template(
                        &envr_core::i18n::tr_key(
                            "cli.why.config_dir",
                            "项目配置目录：{path}",
                            "Project config directory: {path}",
                        ),
                        &[("path", &loc.dir.display().to_string())],
                    )
                );
                if let Some(p) = &loc.base_file {
                    println!(
                        "{} {}",
                        envr_core::i18n::tr_key("cli.why.base_file", "  base", "  base"),
                        p.display()
                    );
                }
                if let Some(p) = &loc.local_file {
                    println!(
                        "{} {}",
                        envr_core::i18n::tr_key("cli.why.local_file", "  local", "  local"),
                        p.display()
                    );
                }
            } else {
                println!(
                    "{}",
                    envr_core::i18n::tr_key(
                        "cli.why.no_project_config",
                        "未找到 `.envr.toml` / `.envr.local.toml`（自工作目录向上搜索）。",
                        "No `.envr.toml` / `.envr.local.toml` found (searching upward from the working directory).",
                    )
                );
            }
            if let Some(ref s) = spec_trim {
                println!(
                    "{}",
                    fmt_template(
                        &envr_core::i18n::tr_key(
                            "cli.why.spec_override",
                            "`--spec {spec}`：本次解析忽略项目 pin，按该 spec 在 `versions` 下选择目录。",
                            "`--spec {spec}`: this resolution ignores the project pin and picks under `versions` from this spec.",
                        ),
                        &[("spec", s.as_str())],
                    )
                );
            }
            if let Some(ref p) = pin {
                if spec_trim.is_some() {
                    println!(
                        "{}",
                        fmt_template(
                            &envr_core::i18n::tr_key(
                                "cli.why.pin_shadowed",
                                "（项目 pin 为 `{spec}`，已被 `--spec` 覆盖）",
                                "(project pin is `{spec}`, overridden by `--spec`)",
                            ),
                            &[("spec", p.as_str())],
                        )
                    );
                } else {
                    println!(
                        "{}",
                        fmt_template(
                            &envr_core::i18n::tr_key(
                                "cli.why.pin",
                                "项目 pin：`{spec}` → 使用 `versions` 下匹配该 spec 的目录。",
                                "Project pin: `{spec}` → pick matching directory under `versions`.",
                            ),
                            &[("spec", p.as_str())],
                        )
                    );
                }
            } else if spec_trim.is_none() {
                println!(
                    "{}",
                    fmt_template(
                        &envr_core::i18n::tr_key(
                            "cli.why.global_current",
                            "无项目 pin：使用全局 `runtimes/{lang}/current` 指向的安装目录。",
                            "No project pin: using global `runtimes/{lang}/current`.",
                        ),
                        &[("lang", lang.as_str())],
                    )
                );
            }
            println!(
                "{} {}",
                envr_core::i18n::tr_key("cli.why.resolved_home", "解析结果：", "Resolved home:"),
                home.display()
            );
        }
    }))
}
