use crate::cli::GlobalArgs;
use crate::commands::common;
use crate::output;

use envr_core::shim_service::ShimService;
use envr_domain::runtime::RuntimeKind;

pub fn sync(g: &GlobalArgs, include_globals: bool) -> i32 {
    let root = match common::effective_runtime_root() {
        Ok(r) => r,
        Err(e) => return common::print_envr_error(g, e),
    };

    let shim_exe = match find_envr_shim_executable() {
        Ok(p) => p,
        Err(e) => return common::print_envr_error(g, e),
    };

    let svc = ShimService::new(root.clone(), shim_exe);
    let mut ensured: Vec<&'static str> = Vec::new();

    for kind in [
        RuntimeKind::Node,
        RuntimeKind::Python,
        RuntimeKind::Java,
        RuntimeKind::Go,
        RuntimeKind::Rust,
        RuntimeKind::Php,
        RuntimeKind::Deno,
        RuntimeKind::Bun,
    ] {
        if svc.ensure_shims(kind).is_ok() {
            ensured.push(common::kind_label(kind));
        }
    }

    if include_globals {
        let _ = svc.sync_all_global_package_shims();
    }

    let data = serde_json::json!({
        "runtime_root": root.to_string_lossy(),
        "ensured_core_kinds": ensured,
        "globals_synced": include_globals,
    });

    output::emit_ok(g, "shims_synced", data, || {
        if !g.quiet {
            println!("shims refreshed under {}", root.join("shims").display());
        }
    })
}

fn find_envr_shim_executable() -> envr_error::EnvrResult<std::path::PathBuf> {
    use envr_error::EnvrError;
    let exe = std::env::current_exe().map_err(EnvrError::from)?;
    let dir = exe
        .parent()
        .ok_or_else(|| EnvrError::Runtime("current_exe has no parent directory".into()))?;

    #[cfg(windows)]
    let candidates = ["envr-shim.exe", "envr-shim"];
    #[cfg(not(windows))]
    let candidates = ["envr-shim"];

    for name in candidates {
        let p = dir.join(name);
        if p.is_file() {
            return Ok(p);
        }
    }

    Err(EnvrError::Runtime(format!(
        "envr-shim executable not found next to {}",
        exe.display()
    )))
}
