use crate::cli::GlobalArgs;
use crate::commands::common;
use crate::output;

use envr_config::project_config::load_project_config_disk_only;
use envr_error::EnvrError;
use serde_json::json;
use std::path::PathBuf;

pub fn list(g: &GlobalArgs, path: PathBuf) -> i32 {
    let loaded = match load_project_config_disk_only(&path) {
        Ok(l) => l,
        Err(e) => return common::print_envr_error(g, e),
    };
    let Some((cfg, loc)) = loaded else {
        return common::print_envr_error(
            g,
            EnvrError::Validation(format!(
                "no `.envr.toml` or `.envr.local.toml` found searching upward from {}",
                path.display()
            )),
        );
    };

    let mut names: Vec<_> = cfg.profiles.keys().cloned().collect();
    names.sort();
    let data = json!({
        "config_dir": loc.dir.to_string_lossy(),
        "profiles": names,
    });
    output::emit_ok(g, "profiles_list", data, || {
        if !g.quiet {
            if names.is_empty() {
                println!("(no profiles in {})", loc.dir.display());
            } else {
                for n in names {
                    println!("{n}");
                }
            }
        }
    })
}

pub fn show(g: &GlobalArgs, path: PathBuf, name: String) -> i32 {
    let loaded = match load_project_config_disk_only(&path) {
        Ok(l) => l,
        Err(e) => return common::print_envr_error(g, e),
    };
    let Some((cfg, loc)) = loaded else {
        return common::print_envr_error(
            g,
            EnvrError::Validation(format!(
                "no `.envr.toml` or `.envr.local.toml` found searching upward from {}",
                path.display()
            )),
        );
    };

    let Some(p) = cfg.profiles.get(&name) else {
        return common::print_envr_error(
            g,
            EnvrError::Validation(format!("no profile `{name}` in {}", loc.dir.display())),
        );
    };

    let data = json!({
        "config_dir": loc.dir.to_string_lossy(),
        "name": name,
        "runtimes": p.runtimes,
        "env": p.env,
    });
    output::emit_ok(g, "profile_show", data, || {
        if !g.quiet {
            println!("profile `{name}` (root {})", loc.dir.display());
            println!("{}", serde_json::to_string_pretty(p).unwrap_or_default());
        }
    })
}
