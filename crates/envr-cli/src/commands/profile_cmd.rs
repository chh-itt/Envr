use crate::cli::GlobalArgs;
use crate::CommandOutcome;
use crate::output::{self, fmt_template};

use envr_config::project_config::load_project_config_disk_only;
use envr_error::{EnvrError, EnvrResult};
use serde_json::json;
use std::path::PathBuf;

pub fn list(g: &GlobalArgs, path: PathBuf) -> i32 {
    CommandOutcome::from_result(list_inner(g, path)).finish(g)
}

fn list_inner(g: &GlobalArgs, path: PathBuf) -> EnvrResult<i32> {
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
    });
    Ok(output::emit_ok(g, "profiles_list", data, || {
        if !g.quiet {
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
                for n in names {
                    println!("{n}");
                }
            }
        }
    }))
}

pub fn show(g: &GlobalArgs, path: PathBuf, name: String) -> i32 {
    CommandOutcome::from_result(show_inner(g, path, name)).finish(g)
}

fn show_inner(g: &GlobalArgs, path: PathBuf, name: String) -> EnvrResult<i32> {
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
    });
    Ok(output::emit_ok(g, "profile_show", data, || {
        if !g.quiet {
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
            println!("{}", serde_json::to_string_pretty(p).unwrap_or_default());
        }
    }))
}
