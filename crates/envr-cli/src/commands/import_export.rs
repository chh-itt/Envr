use crate::cli::GlobalArgs;
use crate::CommandOutcome;
use crate::output::{self, fmt_template};

use envr_config::project_config::{
    PROJECT_CONFIG_FILE, ProjectConfig, load_project_config_disk_only, parse_project_config,
    save_project_config,
};
use envr_error::{EnvrError, EnvrResult};
use serde_json::json;
use std::fs;
use std::path::PathBuf;

pub fn import_run(g: &GlobalArgs, file: PathBuf, path: PathBuf) -> i32 {
    CommandOutcome::from_result(import_run_inner(g, file, path)).finish(g)
}

fn import_run_inner(g: &GlobalArgs, file: PathBuf, path: PathBuf) -> EnvrResult<i32> {
    if !file.is_file() {
        return Err(EnvrError::Validation(fmt_template(
            &envr_core::i18n::tr_key(
                "cli.err.not_a_file",
                "不是文件：{path}",
                "not a file: {path}",
            ),
            &[("path", &file.display().to_string())],
        )));
    }
    let dest = path.join(PROJECT_CONFIG_FILE);
    let mut merged = if dest.is_file() {
        parse_project_config(&dest)?
    } else {
        ProjectConfig::default()
    };
    let imported = parse_project_config(&file)?;
    merged.runtimes.extend(imported.runtimes);
    merged.env.extend(imported.env);
    merged.profiles.extend(imported.profiles);

    save_project_config(&dest, &merged)?;

    let data = json!({
        "dest": dest.to_string_lossy(),
        "source": file.to_string_lossy(),
    });
    Ok(output::emit_ok(g, "config_imported", data, || {
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
    }))
}

pub fn export_run(g: &GlobalArgs, path: PathBuf, output: Option<PathBuf>) -> i32 {
    CommandOutcome::from_result(export_run_inner(g, path, output)).finish(g)
}

fn export_run_inner(
    g: &GlobalArgs,
    path: PathBuf,
    output: Option<PathBuf>,
) -> EnvrResult<i32> {
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

    let toml = toml::to_string_pretty(&cfg).map_err(|e| EnvrError::Config(e.to_string()))?;

    if let Some(out_path) = output {
        fs::write(&out_path, &toml)?;
        let data = json!({
            "config_dir": loc.dir.to_string_lossy(),
            "written": out_path.to_string_lossy(),
            "toml": toml,
        });
        Ok(output::emit_ok(g, "config_exported", data, || {
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
        }))
    } else {
        let data = json!({
            "config_dir": loc.dir.to_string_lossy(),
            "toml": toml,
        });
        Ok(output::emit_ok(g, "config_exported", data, || {
            if !g.quiet {
                print!("{toml}");
                if !toml.ends_with('\n') {
                    println!();
                }
            }
        }))
    }
}
