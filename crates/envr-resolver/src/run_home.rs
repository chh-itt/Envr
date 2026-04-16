//! Resolve on-disk runtime home directories for managed languages (pin vs `current` symlink).
//!
//! Low-level matching of version specs lives in [`envr_shim_core::pick_version_home`] and
//! [`envr_shim_core::resolve_runtime_home_for_lang`]; this module applies **project** rules for
//! go/deno/bun/php and dispatches `exec` vs `run` language sets.

use envr_config::project_config::ProjectConfig;
use envr_error::{EnvrError, EnvrResult};
use envr_shim_core::{ShimContext, pick_version_home, resolve_runtime_home_for_lang};
use std::path::PathBuf;

fn pinned_or_override<'a>(
    cfg: Option<&'a ProjectConfig>,
    lang: &str,
    spec_override: Option<&'a str>,
) -> Option<&'a str> {
    spec_override
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .or_else(|| {
            cfg.and_then(|c| c.runtimes.get(lang))
                .and_then(|r| r.version.as_deref())
        })
}

/// Go: pin → `versions/<spec>`, else global `current` symlink.
pub fn resolve_go_home(
    ctx: &ShimContext,
    cfg: Option<&ProjectConfig>,
    spec_override: Option<&str>,
) -> EnvrResult<PathBuf> {
    let versions_dir = ctx
        .runtime_root
        .join("runtimes")
        .join("go")
        .join("versions");
    let current_link = ctx.runtime_root.join("runtimes").join("go").join("current");

    if let Some(spec) = pinned_or_override(cfg, "go", spec_override) {
        pick_version_home(&versions_dir, spec)
    } else if !current_link.exists() {
        Err(EnvrError::Runtime(format!(
            "no global current for go at {}; install and select a version",
            current_link.display()
        )))
    } else {
        std::fs::canonicalize(&current_link).map_err(EnvrError::from)
    }
}

/// Deno: pin → `versions/<spec>`, else global `current` symlink.
pub fn resolve_deno_home(
    ctx: &ShimContext,
    cfg: Option<&ProjectConfig>,
    spec_override: Option<&str>,
) -> EnvrResult<PathBuf> {
    let versions_dir = ctx
        .runtime_root
        .join("runtimes")
        .join("deno")
        .join("versions");
    let current_link = ctx
        .runtime_root
        .join("runtimes")
        .join("deno")
        .join("current");

    if let Some(spec) = pinned_or_override(cfg, "deno", spec_override) {
        pick_version_home(&versions_dir, spec)
    } else if !current_link.exists() {
        Err(EnvrError::Runtime(format!(
            "no global current for deno at {}; install and select a version",
            current_link.display()
        )))
    } else {
        std::fs::canonicalize(&current_link).map_err(EnvrError::from)
    }
}

/// Bun: pin → `versions/<spec>`, else global `current` symlink.
pub fn resolve_bun_home(
    ctx: &ShimContext,
    cfg: Option<&ProjectConfig>,
    spec_override: Option<&str>,
) -> EnvrResult<PathBuf> {
    let versions_dir = ctx
        .runtime_root
        .join("runtimes")
        .join("bun")
        .join("versions");
    let current_link = ctx
        .runtime_root
        .join("runtimes")
        .join("bun")
        .join("current");

    if let Some(spec) = pinned_or_override(cfg, "bun", spec_override) {
        pick_version_home(&versions_dir, spec)
    } else if !current_link.exists() {
        Err(EnvrError::Runtime(format!(
            "no global current for bun at {}; install and select a version",
            current_link.display()
        )))
    } else {
        std::fs::canonicalize(&current_link).map_err(EnvrError::from)
    }
}

/// PHP uses the same pin/current rules as other shims via [`resolve_runtime_home_for_lang`].
pub fn resolve_php_home(
    ctx: &ShimContext,
    _cfg: Option<&ProjectConfig>,
    spec_override: Option<&str>,
) -> EnvrResult<PathBuf> {
    resolve_runtime_home_for_lang(ctx, "php", spec_override)
}

/// Dotnet uses the same pin/current rules as other shims via [`resolve_runtime_home_for_lang`].
pub fn resolve_dotnet_home(
    ctx: &ShimContext,
    _cfg: Option<&ProjectConfig>,
    spec_override: Option<&str>,
) -> EnvrResult<PathBuf> {
    resolve_runtime_home_for_lang(ctx, "dotnet", spec_override)
}

/// Languages considered by `envr run` / missing-pin planning (no spec override).
pub fn resolve_run_lang_home(
    ctx: &ShimContext,
    cfg: Option<&ProjectConfig>,
    lang: &str,
) -> EnvrResult<PathBuf> {
    match lang {
        "php" => resolve_php_home(ctx, cfg, None),
        "deno" => resolve_deno_home(ctx, cfg, None),
        "bun" => resolve_bun_home(ctx, cfg, None),
        "dotnet" => resolve_dotnet_home(ctx, cfg, None),
        "go" => resolve_go_home(ctx, cfg, None),
        "node" | "python" | "java" => resolve_runtime_home_for_lang(ctx, lang, None),
        _ => Err(EnvrError::Validation(format!(
            "internal: unknown run language {lang}"
        ))),
    }
}

/// Same resolution family as [`resolve_run_lang_home`], but supports CLI `--spec` override and
/// delegates unknown `lang` values to [`resolve_runtime_home_for_lang`] (used by `exec`).
pub fn resolve_exec_lang_home(
    ctx: &ShimContext,
    cfg: Option<&ProjectConfig>,
    lang: &str,
    spec_override: Option<&str>,
) -> EnvrResult<PathBuf> {
    if lang == "bun" {
        resolve_bun_home(ctx, cfg, spec_override)
    } else if lang == "deno" {
        resolve_deno_home(ctx, cfg, spec_override)
    } else if lang == "php" {
        resolve_php_home(ctx, cfg, spec_override)
    } else if lang == "go" {
        resolve_go_home(ctx, cfg, spec_override)
    } else if lang == "dotnet" {
        resolve_dotnet_home(ctx, cfg, spec_override)
    } else {
        resolve_runtime_home_for_lang(ctx, lang, spec_override)
    }
}
