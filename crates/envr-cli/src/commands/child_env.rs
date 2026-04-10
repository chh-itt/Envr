//! Build merged process environment for `exec`, `run`, and `env` (PATH, JAVA_HOME, project `env`).

use envr_config::project_config::{ProjectConfig, load_project_config_profile};
use envr_config::settings::{Settings, settings_path_from_platform};
use envr_error::{EnvrError, EnvrResult};
use envr_platform::paths::current_platform_paths;
use envr_shim_core::{ShimContext, pick_version_home, resolve_runtime_home_for_lang};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

pub fn path_sep() -> char {
    if cfg!(windows) { ';' } else { ':' }
}

pub fn prepend_path(entries: &[PathBuf], existing: &str) -> String {
    let sep = path_sep();
    let mut parts: Vec<String> = entries.iter().map(|p| p.display().to_string()).collect();
    if !existing.is_empty() {
        parts.push(existing.to_string());
    }
    parts.join(&sep.to_string())
}

pub fn runtime_bin_dirs(home: &Path, lang: &str) -> Vec<PathBuf> {
    match lang {
        "node" => vec![home.join("bin"), home.to_path_buf()],
        "python" => vec![home.join("Scripts"), home.join("bin")],
        "java" => vec![home.join("bin")],
        "go" => vec![home.join("bin")],
        "rust" => vec![home.to_path_buf()],
        "php" => vec![home.to_path_buf(), home.join("bin")],
        "deno" => vec![home.to_path_buf(), home.join("bin")],
        "bun" => vec![home.to_path_buf(), home.join("bin")],
        _ => vec![],
    }
}

fn dedup_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut seen = HashSet::<String>::new();
    let mut out = Vec::new();
    for p in paths {
        let key = p.display().to_string();
        if seen.insert(key) {
            out.push(p);
        }
    }
    out
}

fn load_settings() -> EnvrResult<Settings> {
    let paths = current_platform_paths()?;
    let sp = settings_path_from_platform(&paths);
    Settings::load_or_default_from(&sp)
}

fn maybe_inject_go_settings(env: &mut HashMap<String, String>) -> EnvrResult<()> {
    let st = load_settings()?;

    // GOPROXY injection.
    let legacy = st.runtime.go.goproxy.as_deref().unwrap_or("").trim();
    let custom = st.runtime.go.proxy_custom.as_deref().unwrap_or("").trim();
    let gp = match st.runtime.go.proxy_mode {
        envr_config::settings::GoProxyMode::Auto => {
            if !legacy.is_empty() {
                legacy.to_string()
            } else if envr_config::settings::prefer_china_mirror_locale(&st) {
                "https://goproxy.cn,direct".to_string()
            } else {
                "https://proxy.golang.org,direct".to_string()
            }
        }
        envr_config::settings::GoProxyMode::Domestic => "https://goproxy.cn,direct".to_string(),
        envr_config::settings::GoProxyMode::Official => {
            "https://proxy.golang.org,direct".to_string()
        }
        envr_config::settings::GoProxyMode::Direct => "direct".to_string(),
        envr_config::settings::GoProxyMode::Custom => {
            if !custom.is_empty() {
                custom.to_string()
            } else {
                legacy.to_string()
            }
        }
    };
    if !gp.trim().is_empty() {
        env.insert("GOPROXY".into(), gp);
    }

    // Private module patterns: keep these in sync for common corporate setups.
    if let Some(p) = st.runtime.go.private_patterns.as_deref() {
        let v = p.trim();
        if !v.is_empty() {
            env.insert("GOPRIVATE".into(), v.to_string());
            env.insert("GONOSUMDB".into(), v.to_string());
            env.insert("GONOPROXY".into(), v.to_string());
        }
    }
    Ok(())
}

fn resolve_go_home(
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

    let pinned = spec_override
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .or_else(|| {
            cfg.and_then(|c| c.runtimes.get("go"))
                .and_then(|r| r.version.as_deref())
        });

    if let Some(spec) = pinned {
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

fn rust_paths(ctx: &ShimContext) -> (PathBuf, PathBuf) {
    let rust_home = ctx.runtime_root.join("runtimes").join("rust");
    let rustup_home = rust_home.join("rustup");
    let cargo_home = rust_home.join("cargo");
    (rustup_home, cargo_home)
}

fn rustup_active_toolchain(ctx: &ShimContext) -> Option<String> {
    let (rustup_home, cargo_home) = rust_paths(ctx);
    let out = std::process::Command::new("rustup")
        .args(["show", "active-toolchain"])
        .env("RUSTUP_HOME", rustup_home)
        .env("CARGO_HOME", cargo_home)
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

fn resolve_rust_bins(ctx: &ShimContext) -> Vec<PathBuf> {
    let (rustup_home, cargo_home) = rust_paths(ctx);
    let mut bins = vec![cargo_home.join("bin")];
    if let Some(tc) = rustup_active_toolchain(ctx) {
        bins.push(rustup_home.join("toolchains").join(tc).join("bin"));
    }
    bins
}

fn resolve_php_home(
    ctx: &ShimContext,
    _cfg: Option<&ProjectConfig>,
    spec_override: Option<&str>,
) -> EnvrResult<PathBuf> {
    resolve_runtime_home_for_lang(ctx, "php", spec_override)
}

fn resolve_deno_home(
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

    let pinned = spec_override
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .or_else(|| {
            cfg.and_then(|c| c.runtimes.get("deno"))
                .and_then(|r| r.version.as_deref())
        });

    if let Some(spec) = pinned {
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

fn resolve_bun_home(
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

    let pinned = spec_override
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .or_else(|| {
            cfg.and_then(|c| c.runtimes.get("bun"))
                .and_then(|r| r.version.as_deref())
        });

    if let Some(spec) = pinned {
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

/// Resolve the runtime home directory for `exec` (same pin/current rules as `collect_exec_env`).
///
/// Note: On Windows, process spawning may resolve the executable before applying the child's
/// environment block, so callers may prefer using the returned home to build an absolute tool path.
pub fn resolve_exec_home_for_lang(
    ctx: &ShimContext,
    lang: &str,
    spec_override: Option<&str>,
) -> EnvrResult<PathBuf> {
    let cfg =
        load_project_config_profile(&ctx.working_dir, ctx.profile.as_deref())?.map(|(c, _)| c);
    if lang == "bun" {
        resolve_bun_home(ctx, cfg.as_ref(), spec_override)
    } else if lang == "deno" {
        resolve_deno_home(ctx, cfg.as_ref(), spec_override)
    } else if lang == "php" {
        resolve_php_home(ctx, cfg.as_ref(), spec_override)
    } else if lang == "go" {
        resolve_go_home(ctx, cfg.as_ref(), spec_override)
    } else {
        resolve_runtime_home_for_lang(ctx, lang, spec_override)
    }
}

/// Single-language resolution: prepend that runtime's bin dirs to `PATH`, set `JAVA_HOME` for Java.
pub fn collect_exec_env(
    ctx: &ShimContext,
    lang: &str,
    spec_override: Option<&str>,
) -> EnvrResult<HashMap<String, String>> {
    let cfg =
        load_project_config_profile(&ctx.working_dir, ctx.profile.as_deref())?.map(|(c, _)| c);
    let mut env: HashMap<String, String> = std::env::vars().collect();
    if let Some(ref c) = cfg {
        for (k, v) in &c.env {
            env.insert(k.clone(), v.clone());
        }
    }
    let home = if lang == "bun" {
        resolve_bun_home(ctx, cfg.as_ref(), spec_override)?
    } else if lang == "deno" {
        resolve_deno_home(ctx, cfg.as_ref(), spec_override)?
    } else if lang == "rust" {
        // For Rust, prefer cargo bin; also add active toolchain bin when available.
        let bins = dedup_paths(resolve_rust_bins(ctx));
        let old_path = env.get("PATH").map(|s| s.as_str()).unwrap_or("");
        env.insert("PATH".into(), prepend_path(&bins, old_path));
        return Ok(env);
    } else if lang == "php" {
        resolve_php_home(ctx, cfg.as_ref(), spec_override)?
    } else if lang == "go" {
        resolve_go_home(ctx, cfg.as_ref(), spec_override)?
    } else {
        resolve_runtime_home_for_lang(ctx, lang, spec_override)?
    };
    let home = std::fs::canonicalize(&home).map_err(EnvrError::from)?;
    let bins = dedup_paths(runtime_bin_dirs(&home, lang));
    let old_path = env.get("PATH").map(|s| s.as_str()).unwrap_or("");
    env.insert("PATH".into(), prepend_path(&bins, old_path));
    if lang == "java" {
        env.insert("JAVA_HOME".into(), home.display().to_string());
    }
    if lang == "go" {
        env.insert("GOROOT".into(), home.display().to_string());
        maybe_inject_go_settings(&mut env)?;
    }
    Ok(env)
}

/// Multi-runtime PATH (node, python, java, go) plus one `JAVA_HOME` when Java resolves.
pub fn collect_run_env(ctx: &ShimContext) -> EnvrResult<HashMap<String, String>> {
    let cfg =
        load_project_config_profile(&ctx.working_dir, ctx.profile.as_deref())?.map(|(c, _)| c);
    let mut env: HashMap<String, String> = std::env::vars().collect();
    if let Some(ref c) = cfg {
        for (k, v) in &c.env {
            env.insert(k.clone(), v.clone());
        }
    }
    let mut path_entries: Vec<PathBuf> = Vec::new();
    let mut java_home: Option<PathBuf> = None;
    let mut go_home: Option<PathBuf> = None;
    for lang in ["node", "python", "java", "go", "rust", "php", "deno", "bun"] {
        if lang == "rust" {
            path_entries.extend(resolve_rust_bins(ctx));
            continue;
        }
        if lang == "php" {
            let home = match resolve_php_home(ctx, cfg.as_ref(), None) {
                Ok(h) => h,
                Err(_) => continue,
            };
            let home = std::fs::canonicalize(&home).map_err(EnvrError::from)?;
            path_entries.extend(runtime_bin_dirs(&home, "php"));
            continue;
        }
        if lang == "deno" {
            let home = match resolve_deno_home(ctx, cfg.as_ref(), None) {
                Ok(h) => h,
                Err(_) => continue,
            };
            let home = std::fs::canonicalize(&home).map_err(EnvrError::from)?;
            path_entries.extend(runtime_bin_dirs(&home, "deno"));
            continue;
        }
        if lang == "bun" {
            let home = match resolve_bun_home(ctx, cfg.as_ref(), None) {
                Ok(h) => h,
                Err(_) => continue,
            };
            let home = std::fs::canonicalize(&home).map_err(EnvrError::from)?;
            path_entries.extend(runtime_bin_dirs(&home, "bun"));
            continue;
        }
        let home = if lang == "go" {
            match resolve_go_home(ctx, cfg.as_ref(), None) {
                Ok(h) => h,
                Err(_) => continue,
            }
        } else {
            let Ok(h) = resolve_runtime_home_for_lang(ctx, lang, None) else {
                continue;
            };
            h
        };
        let home = std::fs::canonicalize(&home).map_err(EnvrError::from)?;
        if lang == "java" {
            java_home = Some(home.clone());
        }
        if lang == "go" {
            go_home = Some(home.clone());
        }
        path_entries.extend(runtime_bin_dirs(&home, lang));
    }
    path_entries = dedup_paths(path_entries);
    let old_path = env.get("PATH").map(|s| s.as_str()).unwrap_or("");
    env.insert("PATH".into(), prepend_path(&path_entries, old_path));
    if let Some(jh) = java_home {
        env.insert("JAVA_HOME".into(), jh.display().to_string());
    }
    if let Some(gh) = go_home {
        env.insert("GOROOT".into(), gh.display().to_string());
    }
    maybe_inject_go_settings(&mut env)?;
    Ok(env)
}
