use crate::CliExit;
use crate::CliUxPolicy;
use crate::cli::GlobalArgs;
use crate::commands::common;
use crate::output;

use envr_error::EnvrResult;

use envr_core::shim_service::ShimService;
use envr_domain::runtime::RuntimeKind;

/// Ensure all core shims exist (strict). Used by `doctor --fix`.
pub fn sync_core_shims_strict(_g: &GlobalArgs) -> envr_error::EnvrResult<Vec<String>> {
    let root = common::effective_runtime_root()?;
    let shim_exe = find_envr_shim_executable()?;
    let svc = ShimService::new(root, shim_exe);
    let mut ensured: Vec<String> = Vec::new();
    for kind in [
        RuntimeKind::Node,
        RuntimeKind::Python,
        RuntimeKind::Java,
        RuntimeKind::Kotlin,
        RuntimeKind::Scala,
        RuntimeKind::Clojure,
        RuntimeKind::Groovy,
        RuntimeKind::Terraform,
        RuntimeKind::V,
        RuntimeKind::Odin,
        RuntimeKind::Dart,
        RuntimeKind::Flutter,
        RuntimeKind::Go,
        RuntimeKind::Rust,
        RuntimeKind::Ruby,
        RuntimeKind::Elixir,
        RuntimeKind::Erlang,
        RuntimeKind::Php,
        RuntimeKind::Deno,
        RuntimeKind::Bun,
        RuntimeKind::Dotnet,
        RuntimeKind::Zig,
        RuntimeKind::Julia,
        RuntimeKind::Lua,
        RuntimeKind::Nim,
        RuntimeKind::Crystal,
        RuntimeKind::Perl,
        RuntimeKind::RLang,
    ] {
        svc.ensure_shims(kind)?;
        ensured.push(common::kind_label(kind).to_string());
    }
    Ok(ensured)
}

/// Body for [`crate::commands::dispatch`]; errors are finished at the dispatch boundary.
pub(crate) fn sync_inner(g: &GlobalArgs, include_globals: bool) -> EnvrResult<CliExit> {
    let root = common::effective_runtime_root()?;
    let shim_exe = find_envr_shim_executable()?;

    let svc = ShimService::new(root.clone(), shim_exe);
    let mut ensured: Vec<&'static str> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();

    for kind in [
        RuntimeKind::Node,
        RuntimeKind::Python,
        RuntimeKind::Java,
        RuntimeKind::Kotlin,
        RuntimeKind::Scala,
        RuntimeKind::Clojure,
        RuntimeKind::Groovy,
        RuntimeKind::Terraform,
        RuntimeKind::V,
        RuntimeKind::Odin,
        RuntimeKind::Dart,
        RuntimeKind::Flutter,
        RuntimeKind::Go,
        RuntimeKind::Rust,
        RuntimeKind::Ruby,
        RuntimeKind::Elixir,
        RuntimeKind::Erlang,
        RuntimeKind::Php,
        RuntimeKind::Deno,
        RuntimeKind::Bun,
        RuntimeKind::Dotnet,
        RuntimeKind::Zig,
        RuntimeKind::Julia,
        RuntimeKind::Lua,
        RuntimeKind::Nim,
        RuntimeKind::Crystal,
        RuntimeKind::Perl,
        RuntimeKind::RLang,
    ] {
        match svc.ensure_shims(kind) {
            Ok(()) => ensured.push(common::kind_label(kind)),
            Err(err) => warnings.push(format!(
                "failed to ensure {} shims: {}",
                common::kind_label(kind),
                err
            )),
        }
    }

    if include_globals {
        if let Err(err) = svc.sync_all_global_package_shims() {
            warnings.push(format!("failed to sync global package shims: {err}"));
        }
    }

    let data = serde_json::json!({
        "runtime_root": root.to_string_lossy(),
        "ensured_core_kinds": ensured,
        "globals_synced": include_globals,
        "warnings": warnings,
    });

    Ok(output::emit_ok(
        g,
        crate::codes::ok::SHIMS_SYNCED,
        data,
        || {
            if CliUxPolicy::from_global(g).human_text_primary() {
                println!(
                    "{}",
                    crate::output::fmt_template(
                        &envr_core::i18n::tr_key(
                            "cli.shim.sync_ok",
                            "已在 {path} 下刷新 shims",
                            "shims refreshed under {path}",
                        ),
                        &[("path", &root.join("shims").display().to_string())],
                    )
                );
                for warning in &warnings {
                    eprintln!("warning: {warning}");
                }
            }
        },
    ))
}

fn find_envr_shim_executable() -> envr_error::EnvrResult<std::path::PathBuf> {
    use envr_error::EnvrError;
    let exe = std::env::current_exe().map_err(EnvrError::from)?;
    let dir = exe.parent().ok_or_else(|| {
        EnvrError::Runtime(envr_core::i18n::tr_key(
            "cli.err.shim_exe_no_parent",
            "current_exe 没有父目录",
            "current_exe has no parent directory",
        ))
    })?;

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

    Err(EnvrError::Runtime(crate::output::fmt_template(
        &envr_core::i18n::tr_key(
            "cli.err.shim_exe_not_found",
            "在 {path} 旁未找到 envr-shim 可执行文件",
            "envr-shim executable not found next to {path}",
        ),
        &[("path", &exe.display().to_string())],
    )))
}
