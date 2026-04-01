//! `envr update` — CLI version info (self-update TBD).

use crate::cli::GlobalArgs;
use crate::output;

pub fn run(g: &GlobalArgs, check: bool) -> i32 {
    let version = env!("CARGO_PKG_VERSION");
    let data = serde_json::json!({
        "version": version,
        "check_requested": check,
        "self_update": "not_implemented",
    });
    output::emit_ok(g, "update_info", data, || {
        println!("envr {version}");
        if check {
            println!("(release check is not implemented yet)");
        }
        println!(
            "Self-update is not implemented; reinstall from your package source when upgrades are available."
        );
    })
}
