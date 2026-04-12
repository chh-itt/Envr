//! Build merged process environment for `exec`, `run`, and `env` (PATH, JAVA_HOME, project `env`).

use super::run_env_builder::{
    resolve_exec_lang_layer, resolve_rust_bins, rust_paths, rustup_active_toolchain, ExecLangResolution,
    RunEnvStack, RunStackLang, RUN_STACK_LANG_ORDER,
};
use envr_config::project_config::load_project_config_profile;
use envr_config::settings::{
    Settings, bun_package_registry_env, deno_package_registry_env, settings_path_from_platform,
};
use envr_error::{EnvrError, EnvrResult};
use envr_platform::paths::current_platform_paths;
// Re-export merge helpers for callers that used `child_env::path_sep` / `prepend_path` / …
#[allow(unused_imports)]
pub use envr_resolver::{
    dedup_paths, path_sep, prepend_path, runtime_bin_dirs, version_label_from_runtime_home,
    runtime_error_might_install_fix,
};
use envr_resolver::{
    extend_env_with_tooling_settings, plan_missing_installable_pins, resolve_exec_lang_home,
    resolve_run_lang_home,
};
use envr_shim_core::ShimContext;
use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;

/// Human line for `exec --verbose` (language + version label + home path).
pub fn describe_exec_resolution(
    ctx: &ShimContext,
    lang: &str,
    spec_override: Option<&str>,
) -> EnvrResult<String> {
    if lang == "rust" {
        return Ok(describe_rust_exec_line(ctx));
    }
    let home = resolve_exec_home_for_lang(ctx, lang, spec_override)?;
    let canon = std::fs::canonicalize(&home).unwrap_or(home);
    let ver = version_label_from_runtime_home(&canon);
    Ok(format!("{} {} @ {}", lang, ver, canon.display()))
}

/// Rust `exec` environment (rustup + cargo homes under envr runtime root).
pub fn describe_rust_exec_line(ctx: &ShimContext) -> String {
    let (rustup_home, cargo_home) = rust_paths(ctx);
    let tc = rustup_active_toolchain(ctx).unwrap_or_else(|| "default".into());
    format!(
        "rust {} @ rustup={} cargo_home={}",
        tc,
        rustup_home.display(),
        cargo_home.display()
    )
}

/// One line per runtime that contributes to `collect_run_env` (for `run --verbose`).
pub fn collect_run_verbose_lines(
    ctx: &ShimContext,
    install_if_missing: bool,
) -> EnvrResult<Vec<String>> {
    let cfg =
        load_project_config_profile(&ctx.working_dir, ctx.profile.as_deref())?.map(|(c, _)| c);
    let stack = RunEnvStack::new(ctx, cfg.as_ref(), install_if_missing);
    let mut lines = Vec::new();
    for &lang in RUN_STACK_LANG_ORDER {
        let Some(piece) = stack.resolve_lang(lang)? else {
            continue;
        };
        match piece {
            RunStackLang::RustBins(bins) => {
                let tc = rustup_active_toolchain(ctx).unwrap_or_else(|| "default".into());
                lines.push(format!(
                    "rust {} @ {}",
                    tc,
                    bins.first()
                        .map(|p| p.display().to_string())
                        .unwrap_or_default()
                ));
            }
            RunStackLang::Runtime { lang, home } => {
                let ver = version_label_from_runtime_home(&home);
                lines.push(format!("{} {} @ {}", lang, ver, home.display()));
            }
        }
    }
    Ok(lines)
}

fn load_settings() -> EnvrResult<Settings> {
    let paths = current_platform_paths()?;
    let sp = settings_path_from_platform(&paths);
    Settings::load_or_default_from(&sp)
}

/// After `--spec` override, fall back to the project pin for `lang` (for install + retry).
pub fn effective_install_spec_for_exec(
    ctx: &ShimContext,
    lang: &str,
    spec_override: Option<&str>,
) -> EnvrResult<Option<String>> {
    if let Some(s) = spec_override.map(str::trim).filter(|s| !s.is_empty()) {
        return Ok(Some(s.to_string()));
    }
    let cfg =
        load_project_config_profile(&ctx.working_dir, ctx.profile.as_deref())?.map(|(c, _)| c);
    Ok(cfg.and_then(|c| {
        c.runtimes
            .get(lang)
            .and_then(|r| r.version.as_ref())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }))
}

/// Pairs `(lang, version_spec)` for pinned runtimes that are missing on disk but look installable.
pub fn plan_missing_pinned_runtimes_for_run(ctx: &ShimContext) -> EnvrResult<Vec<(String, String)>> {
    let cfg =
        load_project_config_profile(&ctx.working_dir, ctx.profile.as_deref())?.map(|(c, _)| c);
    Ok(plan_missing_installable_pins(cfg.as_ref(), |lang| {
        resolve_run_lang_home(ctx, cfg.as_ref(), lang).map(|_| ())
    }))
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
    resolve_exec_lang_home(ctx, cfg.as_ref(), lang, spec_override)
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
    match resolve_exec_lang_layer(ctx, cfg.as_ref(), lang, spec_override)? {
        ExecLangResolution::RustPathOnly => {
            let bins = dedup_paths(resolve_rust_bins(ctx));
            let old_path = env.get("PATH").map(|s| s.as_str()).unwrap_or("");
            env.insert("PATH".into(), prepend_path(&bins, old_path));
            Ok(env)
        }
        ExecLangResolution::Home(home) => {
            let home = std::fs::canonicalize(&home).map_err(EnvrError::from)?;
            let bins = dedup_paths(runtime_bin_dirs(&home, lang));
            let old_path = env.get("PATH").map(|s| s.as_str()).unwrap_or("");
            env.insert("PATH".into(), prepend_path(&bins, old_path));
            if lang == "java" {
                env.insert("JAVA_HOME".into(), home.display().to_string());
            }
            if lang == "go" {
                env.insert("GOROOT".into(), home.display().to_string());
            }
            let st = load_settings()?;
            extend_env_with_tooling_settings(
                &mut env,
                &st,
                lang == "go",
                lang == "deno",
                lang == "bun",
            );
            Ok(env)
        }
    }
}

/// Multi-runtime PATH (node, python, java, go) plus one `JAVA_HOME` when Java resolves.
///
/// When `install_if_missing` is true, resolution failures for languages pinned in `.envr.toml`
/// are propagated instead of silently omitting that runtime from `PATH`.
pub fn collect_run_env(
    ctx: &ShimContext,
    install_if_missing: bool,
) -> EnvrResult<HashMap<String, String>> {
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
    let mut deno_on_path = false;
    let mut bun_on_path = false;
    let stack = RunEnvStack::new(ctx, cfg.as_ref(), install_if_missing);
    for &lang in RUN_STACK_LANG_ORDER {
        let Some(piece) = stack.resolve_lang(lang)? else {
            continue;
        };
        match piece {
            RunStackLang::RustBins(bins) => {
                path_entries.extend(bins);
            }
            RunStackLang::Runtime { lang, home } => {
                if lang == "java" {
                    java_home = Some(home.clone());
                }
                if lang == "go" {
                    go_home = Some(home.clone());
                }
                if lang == "deno" {
                    deno_on_path = true;
                }
                if lang == "bun" {
                    bun_on_path = true;
                }
                path_entries.extend(runtime_bin_dirs(&home, &lang));
            }
        }
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
    let st = load_settings()?;
    extend_env_with_tooling_settings(&mut env, &st, true, deno_on_path, bun_on_path);
    Ok(env)
}

/// Extra `ENVR_*_VERSION` keys for `envr template` (same resolution rules as `run --verbose`).
pub fn template_extension_vars(ctx: &ShimContext) -> EnvrResult<HashMap<String, String>> {
    let mut m = HashMap::new();
    for line in collect_run_verbose_lines(ctx, false)? {
        let Some((left, _)) = line.split_once(" @ ") else {
            continue;
        };
        let (lang, ver) = match left.split_once(' ') {
            Some((a, b)) => (a, b.trim()),
            None => continue,
        };
        if ver.is_empty() {
            continue;
        }
        let key = match lang {
            "node" => "ENVR_NODE_VERSION",
            "python" => "ENVR_PYTHON_VERSION",
            "java" => "ENVR_JAVA_VERSION",
            "go" => "ENVR_GO_VERSION",
            "rust" => "ENVR_RUST_VERSION",
            "php" => "ENVR_PHP_VERSION",
            "deno" => "ENVR_DENO_VERSION",
            "bun" => "ENVR_BUN_VERSION",
            _ => continue,
        };
        m.insert(key.to_string(), ver.to_string());
    }
    Ok(m)
}

/// Variable names that `envr env` / `collect_run_env` may change for the given working directory,
/// used by `envr hook` to save/restore shell state when leaving a project directory.
pub fn hook_env_restore_keys(ctx: &ShimContext) -> EnvrResult<Vec<String>> {
    let mut keys: BTreeSet<String> = [
        "PATH",
        "JAVA_HOME",
        "GOROOT",
        "GOPROXY",
        "GOPRIVATE",
        "GOSUMDB",
        "GONOPROXY",
        "GONOSUMDB",
        "NPM_CONFIG_REGISTRY",
        "JSR_URL",
    ]
    .into_iter()
    .map(String::from)
    .collect();

    if let Some((cfg, _)) = load_project_config_profile(&ctx.working_dir, ctx.profile.as_deref())? {
        for k in cfg.env.keys() {
            keys.insert(k.clone());
        }
    }

    let st = load_settings()?;
    for (k, _) in deno_package_registry_env(&st) {
        keys.insert(k);
    }
    for (k, _) in bun_package_registry_env(&st) {
        keys.insert(k);
    }

    Ok(keys.into_iter().collect())
}
