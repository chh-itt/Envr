use crate::cli::GlobalArgs;
use crate::commands::common;
use crate::output::{self, fmt_template};

use envr_config::project_config::PROJECT_CONFIG_FILE;
use envr_error::EnvrError;
use std::fs;
use std::path::PathBuf;

const INIT_TEMPLATE: &str = r#"# envr project configuration
# See `refactor docs/04-shim-设计.md`. Uncomment a version to pin this repo.

[env]

[runtimes.node]
# version = "20"

[runtimes.python]
# version = "3.12"

[runtimes.java]
# version = "21"
"#;

pub fn run(g: &GlobalArgs, path: PathBuf, force: bool) -> i32 {
    if !path.is_dir() {
        return common::print_envr_error(
            g,
            EnvrError::Validation(fmt_template(
                &envr_core::i18n::tr_key(
                    "cli.err.not_a_directory",
                    "不是目录：{path}",
                    "not a directory: {path}",
                ),
                &[("path", &path.display().to_string())],
            )),
        );
    }
    let target = path.join(PROJECT_CONFIG_FILE);
    if target.exists() && !force {
        return common::print_envr_error(
            g,
            EnvrError::Config(fmt_template(
                &envr_core::i18n::tr_key(
                    "cli.err.init_exists",
                    "{path} 已存在（使用 --force 覆盖）",
                    "{path} already exists (use --force to overwrite)",
                ),
                &[("path", &target.display().to_string())],
            )),
        );
    }
    if let Err(e) = fs::write(&target, INIT_TEMPLATE) {
        return common::print_envr_error(g, EnvrError::from(e));
    }
    let data = serde_json::json!({
        "path": target.to_string_lossy(),
    });
    output::emit_ok(g, "project_config_init", data, || {
        if !g.quiet {
            println!(
                "{}",
                fmt_template(
                    &envr_core::i18n::tr_key(
                        "cli.init.wrote",
                        "已写入 {path}",
                        "wrote {path}",
                    ),
                    &[("path", &target.display().to_string())],
                )
            );
        }
    })
}
