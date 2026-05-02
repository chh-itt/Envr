//! Explain runtime resolution for the current project directory (`envr why <runtime>`).
use crate::CliExit;
use crate::CliUxPolicy;

use crate::CliPathProfile;
use crate::cli::{GlobalArgs, ProjectPathProfileArgs};
use crate::output::{self, fmt_template};

use envr_domain::runtime::{RuntimeKind, parse_runtime_kind};
use envr_config::project_config::load_project_lock;
use envr_error::EnvrError;
use envr_shim_core::{resolve_runtime_home_for_lang_with_project, resolve_version_home};
use serde_json::json;

use super::version_request::{classify_request, explain_request};

fn project_lock_notice(lock_file_present: bool) -> Option<&'static str> {
    if lock_file_present {
        Some("No project pin: a lockfile is present; run `envr project sync --locked`.")
    } else {
        None
    }
}

fn request_source_label(source: &str) -> &'static str {
    match source {
        "cli" => "cli",
        "project" => "project",
        "tool_versions_compat" => ".tool-versions",
        _ => "global",
    }
}

fn project_lock_state_json(
    session: &crate::runtime_session::RuntimeSession,
) -> Option<serde_json::Value> {
    let (_, loc) = session.project.as_ref()?;
    let lock_path = loc.lock_file.as_ref()?;
    let fresh = load_project_lock(lock_path)
        .ok()
        .flatten()
        .is_some_and(|lock_cfg| session.project_config() == Some(&lock_cfg));
    Some(json!({
        "path": lock_path.to_string_lossy(),
        "fresh": fresh,
    }))
}

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

    let request = classify_request(spec_deref, pin.is_some());
    let lock_state = project_lock_state_json(&session);
    let has_lock = lock_state.is_some();
    let compat_name = loaded.as_ref().and_then(|(c, _)| {
        c.compat
            .asdf
            .names
            .iter()
            .find_map(|(asdf_name, envr_name)| (envr_name == &lang).then_some(asdf_name.clone()))
    });
    let resolution = if spec_deref.is_some() {
        "spec_override"
    } else if pin.is_some() {
        "project_pin"
    } else if compat_name.is_some() {
        "tool_versions_compat"
    } else {
        "global_current"
    };
    let request_source = if spec_deref.is_some() {
        "cli"
    } else if pin.is_some() {
        "project"
    } else if compat_name.is_some() {
        "tool_versions_compat"
    } else {
        "global"
    };

    let resolution = if let Some(spec) = spec_deref {
        let versions_dir = session
            .ctx
            .runtime_root
            .join("runtimes")
            .join(&lang)
            .join("versions");
        resolve_version_home(&versions_dir, spec).ok()
    } else {
        None
    };
    let home = resolve_runtime_home_for_lang_with_project(&session.ctx, &lang, spec_deref, cfg)?;
    let home = std::fs::canonicalize(&home).unwrap_or(home);
    let resolved_version = resolution
        .as_ref()
        .and_then(|r| r.resolved_version.clone())
        .or_else(|| home.file_name().and_then(|s| s.to_str()).map(|s| s.to_string()))
        .unwrap_or_default();
    let candidate_note = if spec_deref.is_some() {
        Some("candidate selection was handled by the runtime-specific resolver")
    } else if pin.is_some() {
        Some("project pin selected the resolved runtime directory")
    } else if compat_name.is_some() {
        Some(".tool-versions compatibility mapping selected the resolved runtime directory")
    } else {
        Some("global current selected the resolved runtime directory")
    };

    let project_json = loaded.as_ref().map(|(cfg, loc)| {
        json!({
            "config_dir": loc.dir.to_string_lossy(),
            "base_file": loc.base_file.as_ref().map(|p| p.to_string_lossy().to_string()),
            "local_file": loc.local_file.as_ref().map(|p| p.to_string_lossy().to_string()),
            "compat_file": loc.compat_file.as_ref().map(|p| p.to_string_lossy().to_string()),
            "lock_file": loc.lock_file.as_ref().map(|p| p.to_string_lossy().to_string()),
            "lock": lock_state,
            "pin": pin.clone(),
            "runtimes": cfg.runtimes.keys().cloned().collect::<Vec<_>>(),
            "compat_asdf_names": cfg.compat.asdf.names.clone(),
        })
    });

    let data = json!({
        "lang": lang,
        "working_dir": session.ctx.working_dir.to_string_lossy(),
        "profile": session.ctx.profile,
        "spec_override": spec_trim.clone(),
        "project": project_json,
        "compat_source": compat_name,
        "resolution": resolution,
        "request_source": request_source,
        "request_kind": request.kind_str(),
        "request_value": request.raw,
        "request_normalized": request.normalized,
        "request_alias": request.alias,
        "resolution_reason": if spec_deref.is_some() {
            explain_request(&request)
        } else if pin.is_some() {
            "resolved from project runtime pin"
        } else if compat_name.is_some() {
            "resolved via .tool-versions compatibility mapping"
        } else {
            "resolved from global current runtime"
        },
        "resolved_home": home.to_string_lossy(),
        "resolved_version": resolved_version,
        "candidate_count": resolution.as_ref().map(|r| r.candidate_count),
        "selection_reason": resolution.as_ref().map(|r| r.selection_reason()),
        "candidate_note": candidate_note,
    });

    Ok(output::emit_ok(
        g,
        crate::codes::ok::WHY_RUNTIME,
        data,
        || {
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
                    if let Some(p) = &loc.compat_file {
                        println!(
                            "{} {}",
                            envr_core::i18n::tr_key("cli.why.compat_file", "  compat", "  compat"),
                            p.display()
                        );
                    }
                    if let Some(p) = &loc.lock_file {
                        println!(
                            "{} {}",
                            envr_core::i18n::tr_key("cli.why.lock_file", "  lock", "  lock"),
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
                    if loaded.is_some() {
                        if let Some(msg) = project_lock_notice(has_lock) {
                            println!(
                                "{}",
                                envr_core::i18n::tr_key(
                                    "cli.why.lock_present",
                                    "未找到项目 pin：当前目录已发现 lockfile，可执行 `envr project sync --locked`。",
                                    if lock_state.as_ref().and_then(|v| v.get("fresh")).and_then(|v| v.as_bool()).unwrap_or(false) {
                                        "No project pin: a fresh lockfile is present; run `envr project sync --locked`."
                                    } else {
                                        msg
                                    },
                                )
                            );
                        } else if compat_name.is_some() {
                            println!(
                                "{}",
                                fmt_template(
                                    &envr_core::i18n::tr_key(
                                        "cli.why.compat_source",
                                        "未找到项目 pin：使用 `.tool-versions` 兼容映射。",
                                        "No project pin: using `.tool-versions` compatibility mapping.",
                                    ),
                                    &[("lang", lang.as_str())],
                                )
                            );
                        } else {
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
                    } else {
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
                }
                println!(
                    "{} {}",
                    envr_core::i18n::tr_key(
                        "cli.why.resolved_home",
                        "解析结果：",
                        "Resolved home:"
                    ),
                    home.display()
                );
                println!(
                    "{} {}",
                    envr_core::i18n::tr_key(
                        "cli.why.resolved_version",
                        "解析版本：",
                        "Resolved version:"
                    ),
                    resolved_version
                );
                if let Some(note) = candidate_note {
                    println!(
                        "{} {}",
                        envr_core::i18n::tr_key(
                            "cli.why.candidate_note",
                            "候选说明：",
                            "Candidate note:"
                        ),
                        note
                    );
                }
                println!(
                    "{} {}",
                    envr_core::i18n::tr_key("cli.why.request_kind", "请求类型：", "Request kind:",),
                    request.kind_str()
                );
                if let Some(count) = resolution.as_ref().map(|r| r.candidate_count) {
                    println!(
                        "{} {}",
                        envr_core::i18n::tr_key(
                            "cli.why.candidate_count",
                            "候选数量：",
                            "Candidate count:"
                        ),
                        count
                    );
                }
                if let Some(reason) = resolution.as_ref().map(|r| r.selection_reason()) {
                    println!(
                        "{} {}",
                        envr_core::i18n::tr_key(
                            "cli.why.selection_reason",
                            "选择理由：",
                            "Selection reason:"
                        ),
                        reason
                    );
                }
                println!(
                    "{} {}",
                    envr_core::i18n::tr_key(
                        "cli.why.request_source",
                        "请求来源：",
                        "Request source:",
                    ),
                    request_source_label(request_source)
                );
                if let Some(alias) = request.alias.as_deref() {
                    println!(
                        "{} {}",
                        envr_core::i18n::tr_key(
                            "cli.why.request_alias",
                            "请求别名：",
                            "Request alias:",
                        ),
                        alias
                    );
                }
                if let Some(normalized) = request.normalized.as_deref() {
                    println!(
                        "{} {}",
                        envr_core::i18n::tr_key(
                            "cli.why.request_normalized",
                            "规范化请求：",
                            "Normalized request:",
                        ),
                        normalized
                    );
                }
                if let Some(alias) = request.alias.as_deref() {
                    println!(
                        "{} {}",
                        envr_core::i18n::tr_key(
                            "cli.why.request_alias",
                            "请求别名：",
                            "Request alias:",
                        ),
                        alias
                    );
                }
                if let Some(project) = &project_json {
                    if let Some(runtimes) = project.get("runtimes").and_then(|v| v.as_array()) {
                        if !runtimes.is_empty() {
                            println!(
                                "{} {}",
                                envr_core::i18n::tr_key(
                                    "cli.why.project_runtimes",
                                    "项目运行时键：",
                                    "Project runtime keys:",
                                ),
                                runtimes
                                    .iter()
                                    .filter_map(|v| v.as_str())
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            );
                        }
                    }
                }
            }
        },
    ))
}
