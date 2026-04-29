use crate::CliExit;
use crate::CliUxPolicy;
use crate::cli::GlobalArgs;
use crate::commands::common;
use crate::output;

use envr_error::EnvrResult;

use envr_core::shim_service::ShimService;
use envr_domain::runtime::RuntimeKind;
#[cfg(windows)]
use std::fs;

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
        RuntimeKind::Purescript,
        RuntimeKind::Elm,
        RuntimeKind::Gleam,
        RuntimeKind::Racket,
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
        RuntimeKind::Janet,
        RuntimeKind::C3,
        RuntimeKind::Babashka,
        RuntimeKind::Sbcl,
        RuntimeKind::Haxe,
        RuntimeKind::Lua,
        RuntimeKind::Nim,
        RuntimeKind::Crystal,
        RuntimeKind::Perl,
        RuntimeKind::Unison,
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
        RuntimeKind::Purescript,
        RuntimeKind::Elm,
        RuntimeKind::Gleam,
        RuntimeKind::Racket,
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
        RuntimeKind::Janet,
        RuntimeKind::C3,
        RuntimeKind::Babashka,
        RuntimeKind::Sbcl,
        RuntimeKind::Haxe,
        RuntimeKind::Lua,
        RuntimeKind::Nim,
        RuntimeKind::Crystal,
        RuntimeKind::Perl,
        RuntimeKind::Unison,
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

pub(crate) fn sync_kind_after_use(
    kind: RuntimeKind,
    include_globals: bool,
    include_windows_path_mirror: bool,
) -> EnvrResult<()> {
    let root = common::effective_runtime_root()?;
    let shim_exe = find_envr_shim_executable()?;
    let svc = ShimService::new(root.clone(), shim_exe.clone());
    svc.ensure_shims(kind)?;
    if include_globals {
        match kind {
            RuntimeKind::Python => {
                let _ = svc.sync_python_global_package_shims_fast();
            }
            RuntimeKind::Java => {
                let _ = svc.sync_java_global_package_shims_fast();
            }
            RuntimeKind::Node | RuntimeKind::Bun => {
                let _ = svc.sync_all_global_package_shims();
            }
            _ => {}
        }
    }
    #[cfg(windows)]
    if include_windows_path_mirror {
        ensure_windows_path_shim_mirror(kind, &root, &shim_exe)?;
    }
    #[cfg(not(windows))]
    let _ = include_windows_path_mirror;
    Ok(())
}

#[cfg(windows)]
fn ensure_windows_path_shim_mirror(
    kind: RuntimeKind,
    runtime_root: &std::path::Path,
    shim_exe: &std::path::Path,
) -> EnvrResult<()> {
    let mut roots = windows_path_shim_roots();
    roots.push(runtime_root.to_path_buf());
    roots.sort();
    roots.dedup();
    for root in &roots {
        let svc = ShimService::new(root.clone(), shim_exe.to_path_buf());
        svc.ensure_shims(kind)?;
    }
    if matches!(kind, RuntimeKind::Node) {
        let from_dir = runtime_root.join("shims");
        let to_core_stems = [
            "node", "npm", "npx", "python", "python3", "pip", "pip3", "java", "javac", "bun",
            "bunx", "crystal", "perl", "ucm", "lua", "luac", "r", "rscript", "janet", "jpm", "c3c",
            "bb", "sbcl", "haxe", "haxelib",
        ];
        for to_root in roots {
            if to_root == runtime_root {
                continue;
            }
            let to_dir = to_root.join("shims");
            if fs::create_dir_all(&to_dir).is_err() {
                continue;
            }
            let Ok(entries) = fs::read_dir(&from_dir) else {
                continue;
            };
            for e in entries.flatten() {
                let path = e.path();
                if !path.is_file() {
                    continue;
                }
                let stem = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_ascii_lowercase();
                if to_core_stems.contains(&stem.as_str()) {
                    continue;
                }
                let dst = to_dir.join(path.file_name().unwrap_or_default());
                let _ = fs::copy(&path, &dst);
            }
        }
    }
    Ok(())
}

#[cfg(windows)]
fn windows_path_shim_roots() -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    let p = std::env::var_os("Path").unwrap_or_default();
    let s = p.to_string_lossy();
    for seg in s.split(';') {
        let mut seg = seg.trim();
        if seg.is_empty() {
            continue;
        }
        while seg.ends_with('\\') || seg.ends_with('/') {
            seg = seg.trim_end_matches(['\\', '/']).trim();
        }
        let lower = seg.to_ascii_lowercase();
        if lower.contains("envr") && lower.ends_with(r"\shims") {
            let shims_dir = std::path::Path::new(seg);
            if !shims_dir.is_dir() {
                continue;
            }
            if let Some(root) = shims_dir.parent() {
                out.push(root.to_path_buf());
            }
        }
    }
    out
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
