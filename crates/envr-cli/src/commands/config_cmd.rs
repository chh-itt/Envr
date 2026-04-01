//! `envr config` — path and show for `settings.toml`.

use crate::cli::GlobalArgs;
use crate::output;

use envr_config::settings::Settings;
use envr_error::EnvrError;
use envr_platform::paths::current_platform_paths;

pub fn run(g: &GlobalArgs, sub: crate::cli::ConfigCmd) -> i32 {
    let paths = match current_platform_paths() {
        Ok(p) => p,
        Err(e) => return crate::commands::common::print_envr_error(g, e),
    };
    let settings_path = envr_config::settings::settings_path_from_platform(&paths);

    match sub {
        crate::cli::ConfigCmd::Path => {
            let data = serde_json::json!({ "path": settings_path.to_string_lossy() });
            output::emit_ok(g, "config_path", data, || {
                println!("{}", settings_path.display());
            })
        }
        crate::cli::ConfigCmd::Show => match Settings::load_or_default_from(&settings_path) {
            Ok(st) => {
                let pretty = match toml::to_string_pretty(&st) {
                    Ok(s) => s,
                    Err(e) => {
                        return crate::commands::common::print_envr_error(
                            g,
                            EnvrError::Runtime(format!("toml encode: {e}")),
                        );
                    }
                };
                let data = serde_json::json!({
                    "path": settings_path.to_string_lossy(),
                    "settings": serde_json::to_value(&st).unwrap_or(serde_json::Value::Null),
                });
                output::emit_ok(g, "config_show", data, || {
                    println!("{}", settings_path.display());
                    println!();
                    print!("{pretty}");
                })
            }
            Err(e) => crate::commands::common::print_envr_error(g, e),
        },
    }
}
