use crate::cli::GlobalArgs;
use crate::commands::common;
use crate::output;

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
            EnvrError::Validation(format!("not a directory: {}", path.display())),
        );
    }
    let target = path.join(PROJECT_CONFIG_FILE);
    if target.exists() && !force {
        return common::print_envr_error(
            g,
            EnvrError::Config(format!(
                "{} already exists (use --force to overwrite)",
                target.display()
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
            println!("wrote {}", target.display());
        }
    })
}
