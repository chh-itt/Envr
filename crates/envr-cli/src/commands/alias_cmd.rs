//! `envr alias` — persist name → target strings in `config/aliases.toml`.
use crate::CliExit;

use crate::cli::GlobalArgs;
use crate::output::{self, fmt_template};

use envr_config::aliases::AliasesFile;
use envr_error::EnvrResult;
use envr_platform::paths::current_platform_paths;

/// Body for [`crate::commands::dispatch`]; errors are finished at the dispatch boundary.
pub(crate) fn run_inner(g: &GlobalArgs, sub: crate::cli::AliasCmd) -> EnvrResult<CliExit> {
    let paths = current_platform_paths()?;
    let path = AliasesFile::path_from(&paths);

    match sub {
        crate::cli::AliasCmd::List => {
            let file = AliasesFile::load_or_default(&path)?;
            let entries: Vec<_> = file
                .aliases
                .iter()
                .map(|(k, v)| serde_json::json!({ "name": k, "target": v }))
                .collect();
            let data = serde_json::json!({ "aliases": entries });
            Ok(output::emit_ok(
                g,
                crate::codes::ok::ALIAS_LIST,
                data,
                || {
                    if file.aliases.is_empty() {
                        println!(
                            "{}",
                            envr_core::i18n::tr_key("cli.alias.none", "（无别名）", "(no aliases)",)
                        );
                        return;
                    }
                    for (name, target) in &file.aliases {
                        println!("{name} -> {target}");
                    }
                },
            ))
        }
        crate::cli::AliasCmd::Add { name, target } => {
            let name = name.trim().to_string();
            let target = target.trim().to_string();
            if name.is_empty() || target.is_empty() {
                return Ok(crate::output::emit_validation(
                    g,
                    "alias add",
                    r#"envr alias add mynode node
envr alias add mydiag "diagnostics export""#,
                ));
            }
            let mut file = AliasesFile::load_or_default(&path)?;
            file.aliases.insert(name.clone(), target.clone());
            file.save_to(&path)?;
            let data = serde_json::json!({ "name": name, "target": target });
            Ok(output::emit_ok(
                g,
                crate::codes::ok::ALIAS_ADDED,
                data,
                || {
                    println!(
                        "{}",
                        fmt_template(
                            &envr_core::i18n::tr_key(
                                "cli.alias.added",
                                "别名 `{name}` -> `{target}`",
                                "alias `{name}` -> `{target}`",
                            ),
                            &[("name", &name), ("target", &target)],
                        )
                    );
                },
            ))
        }
        crate::cli::AliasCmd::Remove { name } => {
            let name = name.trim().to_string();
            if name.is_empty() {
                return Ok(crate::output::emit_validation(
                    g,
                    "alias remove",
                    "envr alias remove mynode",
                ));
            }
            let mut file = AliasesFile::load_or_default(&path)?;
            let removed = file.aliases.remove(&name);
            file.save_to(&path)?;
            let data = serde_json::json!({
                "name": name,
                "removed": removed.is_some(),
            });
            Ok(output::emit_ok(
                g,
                crate::codes::ok::ALIAS_REMOVED,
                data,
                || {
                    if removed.is_some() {
                        println!(
                            "{}",
                            fmt_template(
                                &envr_core::i18n::tr_key(
                                    "cli.alias.removed",
                                    "已移除别名 `{name}`",
                                    "removed alias `{name}`",
                                ),
                                &[("name", &name)],
                            )
                        );
                    } else {
                        println!(
                            "{}",
                            fmt_template(
                                &envr_core::i18n::tr_key(
                                    "cli.alias.not_found",
                                    "没有名为 `{name}` 的别名",
                                    "no alias named `{name}`",
                                ),
                                &[("name", &name)],
                            )
                        );
                    }
                },
            ))
        }
    }
}
