use crate::cli::GlobalArgs;
use crate::commands::common;
use crate::output::{self, fmt_template};

use envr_config::project_config::{
    PROJECT_CONFIG_FILE, ProjectConfig, load_project_config_disk_only, parse_project_config,
    save_project_config,
};
use envr_error::EnvrError;
use serde_json::json;
use std::fs;
use std::path::PathBuf;

pub fn import_run(g: &GlobalArgs, file: PathBuf, path: PathBuf) -> i32 {
    if !file.is_file() {
        return common::print_envr_error(
            g,
            EnvrError::Validation(fmt_template(
                &envr_core::i18n::tr_key(
                    "cli.err.not_a_file",
                    "不是文件：{path}",
                    "not a file: {path}",
                ),
                &[("path", &file.display().to_string())],
            )),
        );
    }
    let dest = path.join(PROJECT_CONFIG_FILE);
    let mut merged = if dest.is_file() {
        match parse_project_config(&dest) {
            Ok(c) => c,
            Err(e) => return common::print_envr_error(g, e),
        }
    } else {
        ProjectConfig::default()
    };
    let imported = match parse_project_config(&file) {
        Ok(c) => c,
        Err(e) => return common::print_envr_error(g, e),
    };
    merged.runtimes.extend(imported.runtimes);
    merged.env.extend(imported.env);
    merged.profiles.extend(imported.profiles);

    if let Err(e) = save_project_config(&dest, &merged) {
        return common::print_envr_error(g, e);
    }

    let data = json!({
        "dest": dest.to_string_lossy(),
        "source": file.to_string_lossy(),
    });
    output::emit_ok(g, "config_imported", data, || {
        if !g.quiet {
            println!(
                "{}",
                fmt_template(
                    &envr_core::i18n::tr_key(
                        "cli.import.merged",
                        "已合并到 {path}",
                        "merged into {path}",
                    ),
                    &[("path", &dest.display().to_string())],
                )
            );
        }
    })
}

pub fn export_run(g: &GlobalArgs, path: PathBuf, output: Option<PathBuf>) -> i32 {
    let loaded = match load_project_config_disk_only(&path) {
        Ok(l) => l,
        Err(e) => return common::print_envr_error(g, e),
    };
    let Some((cfg, loc)) = loaded else {
        return common::print_envr_error(
            g,
            EnvrError::Validation(fmt_template(
                &envr_core::i18n::tr_key(
                    "cli.err.no_project_config",
                    "自 {path} 向上未找到 `.envr.toml` 或 `.envr.local.toml`",
                    "no `.envr.toml` or `.envr.local.toml` found searching upward from {path}",
                ),
                &[("path", &path.display().to_string())],
            )),
        );
    };

    let toml = match toml::to_string_pretty(&cfg) {
        Ok(s) => s,
        Err(e) => return common::print_envr_error(g, EnvrError::Config(e.to_string())),
    };

    if let Some(out_path) = output {
        if let Err(e) = fs::write(&out_path, &toml) {
            return common::print_envr_error(g, e.into());
        }
        let data = json!({
            "config_dir": loc.dir.to_string_lossy(),
            "written": out_path.to_string_lossy(),
            "toml": toml,
        });
        output::emit_ok(g, "config_exported", data, || {
            if !g.quiet {
                println!(
                    "{}",
                    fmt_template(
                        &envr_core::i18n::tr_key(
                            "cli.export.wrote",
                            "已写入 {path}",
                            "wrote {path}",
                        ),
                        &[("path", &out_path.display().to_string())],
                    )
                );
            }
        })
    } else {
        let data = json!({
            "config_dir": loc.dir.to_string_lossy(),
            "toml": toml,
        });
        output::emit_ok(g, "config_exported", data, || {
            if !g.quiet {
                print!("{toml}");
                if !toml.ends_with('\n') {
                    println!();
                }
            }
        })
    }
}
