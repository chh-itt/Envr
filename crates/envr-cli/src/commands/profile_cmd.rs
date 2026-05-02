use crate::CliExit;
use crate::CliUxPolicy;
use crate::cli::GlobalArgs;
use crate::output::{self, fmt_template};

use envr_config::project_config::load_project_config_disk_only;
use envr_error::{EnvrError, EnvrResult};
use serde_json::json;
use std::path::PathBuf;

/// Body for [`crate::commands::dispatch`]; errors are finished at the dispatch boundary.
pub(crate) fn list_inner(g: &GlobalArgs, path: PathBuf) -> EnvrResult<CliExit> {
    let loaded = load_project_config_disk_only(&path)?;
    let Some((cfg, loc)) = loaded else {
        return Err(EnvrError::Validation(fmt_template(
            &envr_core::i18n::tr_key(
                "cli.err.no_project_config",
                "自 {path} 向上未找到 `.envr.toml` 或 `.envr.local.toml`",
                "no `.envr.toml` or `.envr.local.toml` found searching upward from {path}",
            ),
            &[("path", &path.display().to_string())],
        )));
    };

    let mut names: Vec<_> = cfg.profiles.keys().cloned().collect();
    names.sort();
    let data = json!({
        "config_dir": loc.dir.to_string_lossy(),
        "profiles": names,
        "profile_count": cfg.profiles.len(),
    });
    Ok(output::emit_ok(
        g,
        crate::codes::ok::PROFILES_LIST,
        data,
        || {
            if CliUxPolicy::from_global(g).human_text_primary() {
                if names.is_empty() {
                    println!(
                        "{}",
                        fmt_template(
                            &envr_core::i18n::tr_key(
                                "cli.profile.none_in_dir",
                                "（在 {path} 中无 profile）",
                                "(no profiles in {path})",
                            ),
                            &[("path", &loc.dir.display().to_string())],
                        )
                    );
                } else {
                    println!(
                        "{}",
                        fmt_template(
                            &envr_core::i18n::tr_key(
                                "cli.profile.list_header",
                                "可用 profile：",
                                "Available profiles:"
                            ),
                            &[]
                        )
                    );
                    for n in names {
                        println!("  {n}");
                    }
                    println!(
                        "{}",
                        envr_core::i18n::tr_key(
                            "cli.profile.list_hint",
                            "使用 `ENVR_PROFILE=<name>` 或 `envr run --profile <name>` 激活。",
                            "Activate with `ENVR_PROFILE=<name>` or `envr run --profile <name>`.",
                        )
                    );
                }
            }
        },
    ))
}

/// Body for [`crate::commands::dispatch`]; errors are finished at the dispatch boundary.
pub(crate) fn show_inner(g: &GlobalArgs, path: PathBuf, name: String) -> EnvrResult<CliExit> {
    let loaded = load_project_config_disk_only(&path)?;
    let Some((cfg, loc)) = loaded else {
        return Err(EnvrError::Validation(fmt_template(
            &envr_core::i18n::tr_key(
                "cli.err.no_project_config",
                "自 {path} 向上未找到 `.envr.toml` 或 `.envr.local.toml`",
                "no `.envr.toml` or `.envr.local.toml` found searching upward from {path}",
            ),
            &[("path", &path.display().to_string())],
        )));
    };

    let Some(p) = cfg.profiles.get(&name) else {
        return Err(EnvrError::Validation(fmt_template(
            &envr_core::i18n::tr_key(
                "cli.err.no_profile",
                "在 {path} 中不存在 profile `{name}`",
                "no profile `{name}` in {path}",
            ),
            &[
                ("name", name.as_str()),
                ("path", &loc.dir.display().to_string()),
            ],
        )));
    };

    let data = json!({
        "config_dir": loc.dir.to_string_lossy(),
        "name": name,
        "runtimes": p.runtimes,
        "env": p.env,
        "scripts": p.scripts,
    });
    Ok(output::emit_ok(
        g,
        crate::codes::ok::PROFILE_SHOW,
        data,
        || {
            if CliUxPolicy::from_global(g).human_text_primary() {
                println!(
                    "{}",
                    fmt_template(
                        &envr_core::i18n::tr_key(
                            "cli.profile.show_header",
                            "profile `{name}`（根 {path}）",
                            "profile `{name}` (root {path})",
                        ),
                        &[
                            ("name", name.as_str()),
                            ("path", &loc.dir.display().to_string()),
                        ],
                    )
                );
                if !p.runtimes.is_empty() {
                    println!(
                        "{}",
                        envr_core::i18n::tr_key(
                            "cli.profile.show_runtimes",
                            "运行时：",
                            "Runtimes:"
                        )
                    );
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&p.runtimes).unwrap_or_default()
                    );
                }
                if !p.env.is_empty() {
                    println!(
                        "{}",
                        envr_core::i18n::tr_key(
                            "cli.profile.show_env",
                            "环境变量：",
                            "Environment:"
                        )
                    );
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&p.env).unwrap_or_default()
                    );
                }
                if !p.scripts.is_empty() {
                    println!(
                        "{}",
                        envr_core::i18n::tr_key("cli.profile.show_scripts", "脚本：", "Scripts:")
                    );
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&p.scripts).unwrap_or_default()
                    );
                }
            }
        },
    ))
}
