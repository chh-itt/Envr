use crate::cli::GlobalArgs;
use crate::commands::common;
use crate::output;

use envr_config::project_config::load_project_config_profile;
use envr_domain::runtime::parse_runtime_kind;
use envr_shim_core::{ShimContext, resolve_runtime_home_for_lang};
use std::path::PathBuf;

pub fn run(
    g: &GlobalArgs,
    lang: String,
    spec: Option<String>,
    path: PathBuf,
    profile: Option<String>,
) -> i32 {
    let lang = lang.trim().to_ascii_lowercase();
    if let Err(e) = parse_runtime_kind(&lang) {
        return common::print_envr_error(g, e);
    }

    let mut ctx = match ShimContext::from_process_env() {
        Ok(c) => c,
        Err(e) => return common::print_envr_error(g, e),
    };
    ctx.working_dir = std::fs::canonicalize(&path).unwrap_or(path);
    if let Some(p) = profile.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()) {
        ctx.profile = Some(p.to_string());
    }

    let cfg = match load_project_config_profile(&ctx.working_dir, ctx.profile.as_deref()) {
        Ok(l) => l.map(|(c, _)| c),
        Err(e) => return common::print_envr_error(g, e),
    };
    let has_pin = cfg
        .as_ref()
        .and_then(|c| c.runtimes.get(&lang))
        .and_then(|r| r.version.as_deref())
        .is_some();
    let override_nonempty = spec.as_ref().is_some_and(|s| !s.trim().is_empty());
    let source = if override_nonempty {
        "cli_override"
    } else if has_pin {
        "project"
    } else {
        "global_current"
    };

    let trimmed = spec.as_deref().map(str::trim).filter(|s| !s.is_empty());
    let home = match resolve_runtime_home_for_lang(&ctx, &lang, trimmed) {
        Ok(h) => h,
        Err(e) => return common::print_envr_error(g, e),
    };
    let home = match std::fs::canonicalize(&home) {
        Ok(h) => h,
        Err(e) => return common::print_envr_error(g, e.into()),
    };
    let version_label = home
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();

    let data = serde_json::json!({
        "kind": lang,
        "resolution_source": source,
        "home": home.to_string_lossy(),
        "version_dir": version_label,
    });
    output::emit_ok(g, "runtime_resolved", data, || {
        if !g.quiet {
            println!("{}: {} (from {source})", lang, home.display());
        }
    })
}
