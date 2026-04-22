//! `run` / `run --verbose` stack resolution and single-`exec` home resolution.
//!
//! Keeps language order and pin/skip rules in one place so `collect_run_env` and
//! `collect_run_verbose_lines` stay aligned.

use envr_config::project_config::ProjectConfig;
use envr_error::{EnvrError, EnvrResult};
use envr_resolver::{
    resolve_bun_home, resolve_deno_home, resolve_go_home, resolve_php_home, resolve_ruby_home,
};
use envr_shim_core::{ShimContext, resolve_runtime_home_for_lang};
use std::path::PathBuf;

/// Languages merged for `envr run` / `run --verbose` (fixed order; includes rust).
pub(crate) const RUN_STACK_LANG_ORDER: &[&str] = &[
    "node",
    "python",
    "java",
    "kotlin",
    "scala",
    "clojure",
    "groovy",
    "terraform",
    "v",
    "odin",
    "purescript",
    "elm",
    "gleam",
    "racket",
    "dart",
    "flutter",
    "go",
    "rust",
    "ruby",
    "elixir",
    "erlang",
    "php",
    "deno",
    "bun",
    "dotnet",
    "zig",
    "julia",
    "janet",
    "c3",
    "lua",
    "nim",
    "crystal",
    "perl",
    "r",
];

fn project_has_runtime_pin(cfg: Option<&ProjectConfig>, lang: &str) -> bool {
    cfg.and_then(|c| c.runtimes.get(lang))
        .and_then(|r| r.version.as_deref())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .is_some()
}

fn run_resolve_home_or_skip(
    install_if_missing: bool,
    cfg: Option<&ProjectConfig>,
    lang: &str,
    res: EnvrResult<PathBuf>,
) -> EnvrResult<Option<PathBuf>> {
    match res {
        Ok(home) => Ok(Some(home)),
        Err(e) => {
            if install_if_missing && project_has_runtime_pin(cfg, lang) {
                Err(e)
            } else {
                Ok(None)
            }
        }
    }
}

/// One layer of the `run` stack after resolution (canonicalized homes).
pub(crate) enum RunStackLang {
    RustBins(Vec<PathBuf>),
    Runtime { lang: String, home: PathBuf },
}

pub(crate) struct RunEnvStack<'a> {
    ctx: &'a ShimContext,
    cfg: Option<&'a ProjectConfig>,
    install_if_missing: bool,
}

impl<'a> RunEnvStack<'a> {
    pub fn new(
        ctx: &'a ShimContext,
        cfg: Option<&'a ProjectConfig>,
        install_if_missing: bool,
    ) -> Self {
        Self {
            ctx,
            cfg,
            install_if_missing,
        }
    }

    /// Resolve one language in [`RUN_STACK_LANG_ORDER`]. Returns `None` to skip (same as previous
    /// `continue` in the consumer loops).
    pub fn resolve_lang(&self, lang: &str) -> EnvrResult<Option<RunStackLang>> {
        if lang == "rust" {
            let bins = resolve_rust_bins(self.ctx);
            if bins.is_empty() {
                return Ok(None);
            }
            return Ok(Some(RunStackLang::RustBins(bins)));
        }
        if lang == "php" {
            let Some(home) = run_resolve_home_or_skip(
                self.install_if_missing,
                self.cfg,
                lang,
                resolve_php_home(self.ctx, self.cfg, None),
            )?
            else {
                return Ok(None);
            };
            let home = std::fs::canonicalize(&home).map_err(EnvrError::from)?;
            return Ok(Some(RunStackLang::Runtime {
                lang: lang.to_string(),
                home,
            }));
        }
        if lang == "deno" {
            let Some(home) = run_resolve_home_or_skip(
                self.install_if_missing,
                self.cfg,
                lang,
                resolve_deno_home(self.ctx, self.cfg, None),
            )?
            else {
                return Ok(None);
            };
            let home = std::fs::canonicalize(&home).map_err(EnvrError::from)?;
            return Ok(Some(RunStackLang::Runtime {
                lang: lang.to_string(),
                home,
            }));
        }
        if lang == "bun" {
            let Some(home) = run_resolve_home_or_skip(
                self.install_if_missing,
                self.cfg,
                lang,
                resolve_bun_home(self.ctx, self.cfg, None),
            )?
            else {
                return Ok(None);
            };
            let home = std::fs::canonicalize(&home).map_err(EnvrError::from)?;
            return Ok(Some(RunStackLang::Runtime {
                lang: lang.to_string(),
                home,
            }));
        }
        if lang == "ruby" {
            let Some(home) = run_resolve_home_or_skip(
                self.install_if_missing,
                self.cfg,
                lang,
                resolve_ruby_home(self.ctx, self.cfg, None),
            )?
            else {
                return Ok(None);
            };
            let home = std::fs::canonicalize(&home).map_err(EnvrError::from)?;
            return Ok(Some(RunStackLang::Runtime {
                lang: lang.to_string(),
                home,
            }));
        }
        let home_res = if lang == "go" {
            resolve_go_home(self.ctx, self.cfg, None)
        } else {
            resolve_runtime_home_for_lang(self.ctx, lang, None)
        };
        let Some(home) =
            run_resolve_home_or_skip(self.install_if_missing, self.cfg, lang, home_res)?
        else {
            return Ok(None);
        };
        let home = std::fs::canonicalize(&home).map_err(EnvrError::from)?;
        Ok(Some(RunStackLang::Runtime {
            lang: lang.to_string(),
            home,
        }))
    }
}

// --- rust (rustup under ENVR runtime root) ---------------------------------

pub(crate) fn rust_paths(ctx: &ShimContext) -> (PathBuf, PathBuf) {
    let rust_home = ctx.runtime_root.join("runtimes").join("rust");
    let rustup_home = rust_home.join("rustup");
    let cargo_home = rust_home.join("cargo");
    (rustup_home, cargo_home)
}

pub(crate) fn rustup_active_toolchain(ctx: &ShimContext) -> Option<String> {
    let (rustup_home, cargo_home) = rust_paths(ctx);
    // Skip `rustup` when envr has no rust data dir yet (typical for node-only projects); a cold
    // probe can stall CI / integration tests.
    if !rustup_home.is_dir() {
        return None;
    }
    let out = std::process::Command::new("rustup")
        .args(["show", "active-toolchain"])
        .env("RUSTUP_HOME", rustup_home.as_os_str())
        .env("CARGO_HOME", cargo_home.as_os_str())
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout);
    let first = s.split_whitespace().next()?.trim();
    if first.is_empty() {
        None
    } else {
        Some(first.to_string())
    }
}

pub(crate) fn resolve_rust_bins(ctx: &ShimContext) -> Vec<PathBuf> {
    let (rustup_home, cargo_home) = rust_paths(ctx);
    let mut bins = vec![cargo_home.join("bin")];
    if let Some(tc) = rustup_active_toolchain(ctx) {
        bins.push(rustup_home.join("toolchains").join(tc).join("bin"));
    }
    bins
}

// --- single `exec` language (optional --spec) ------------------------------

pub(crate) enum ExecLangResolution {
    RustPathOnly,
    Home(PathBuf),
}

pub(crate) fn resolve_exec_lang_layer(
    ctx: &ShimContext,
    cfg: Option<&ProjectConfig>,
    lang: &str,
    spec_override: Option<&str>,
) -> EnvrResult<ExecLangResolution> {
    match lang {
        "rust" => Ok(ExecLangResolution::RustPathOnly),
        "bun" => Ok(ExecLangResolution::Home(resolve_bun_home(
            ctx,
            cfg,
            spec_override,
        )?)),
        "deno" => Ok(ExecLangResolution::Home(resolve_deno_home(
            ctx,
            cfg,
            spec_override,
        )?)),
        "php" => Ok(ExecLangResolution::Home(resolve_php_home(
            ctx,
            cfg,
            spec_override,
        )?)),
        "go" => Ok(ExecLangResolution::Home(resolve_go_home(
            ctx,
            cfg,
            spec_override,
        )?)),
        "ruby" => Ok(ExecLangResolution::Home(resolve_ruby_home(
            ctx,
            cfg,
            spec_override,
        )?)),
        _ => Ok(ExecLangResolution::Home(resolve_runtime_home_for_lang(
            ctx,
            lang,
            spec_override,
        )?)),
    }
}
