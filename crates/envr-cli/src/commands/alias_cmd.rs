//! `envr alias` — persist name → target strings in `config/aliases.toml`.

use crate::cli::GlobalArgs;
use crate::output;

use envr_config::aliases::AliasesFile;
use envr_platform::paths::current_platform_paths;

pub fn run(g: &GlobalArgs, sub: crate::cli::AliasCmd) -> i32 {
    let paths = match current_platform_paths() {
        Ok(p) => p,
        Err(e) => return crate::commands::common::print_envr_error(g, e),
    };
    let path = AliasesFile::path_from(&paths);

    match sub {
        crate::cli::AliasCmd::List => match AliasesFile::load_or_default(&path) {
            Ok(file) => {
                let entries: Vec<_> = file
                    .aliases
                    .iter()
                    .map(|(k, v)| serde_json::json!({ "name": k, "target": v }))
                    .collect();
                let data = serde_json::json!({ "aliases": entries });
                output::emit_ok(g, "alias_list", data, || {
                    if file.aliases.is_empty() {
                        println!("(no aliases)");
                        return;
                    }
                    for (name, target) in &file.aliases {
                        println!("{name} -> {target}");
                    }
                })
            }
            Err(e) => crate::commands::common::print_envr_error(g, e),
        },
        crate::cli::AliasCmd::Add { name, target } => {
            let name = name.trim().to_string();
            let target = target.trim().to_string();
            if name.is_empty() || target.is_empty() {
                return crate::output::emit_validation(
                    g,
                    "alias add",
                    "envr alias add mynode node",
                );
            }
            let mut file = match AliasesFile::load_or_default(&path) {
                Ok(f) => f,
                Err(e) => return crate::commands::common::print_envr_error(g, e),
            };
            file.aliases.insert(name.clone(), target.clone());
            match file.save_to(&path) {
                Ok(()) => {
                    let data = serde_json::json!({ "name": name, "target": target });
                    output::emit_ok(g, "alias_added", data, || {
                        println!("alias `{name}` -> `{target}`");
                    })
                }
                Err(e) => crate::commands::common::print_envr_error(g, e),
            }
        }
        crate::cli::AliasCmd::Remove { name } => {
            let name = name.trim().to_string();
            if name.is_empty() {
                return crate::output::emit_validation(
                    g,
                    "alias remove",
                    "envr alias remove mynode",
                );
            }
            let mut file = match AliasesFile::load_or_default(&path) {
                Ok(f) => f,
                Err(e) => return crate::commands::common::print_envr_error(g, e),
            };
            let removed = file.aliases.remove(&name);
            match file.save_to(&path) {
                Ok(()) => {
                    let data = serde_json::json!({
                        "name": name,
                        "removed": removed.is_some(),
                    });
                    output::emit_ok(g, "alias_removed", data, || {
                        if removed.is_some() {
                            println!("removed alias `{name}`");
                        } else {
                            println!("no alias named `{name}`");
                        }
                    })
                }
                Err(e) => crate::commands::common::print_envr_error(g, e),
            }
        }
    }
}
