//! Build merged process environment for `exec`, `run`, and `env` (PATH, JAVA_HOME, project `env`).

use super::run_env_builder::{
    ExecLangResolution, RUN_STACK_LANG_ORDER, RunEnvStack, RunStackLang, resolve_exec_lang_layer,
    resolve_rust_bins, rust_paths, rustup_active_toolchain,
};
use envr_config::project_config::{ProjectConfig, load_project_config_profile};
use envr_config::settings::{
    Settings, bun_package_registry_env, deno_package_registry_env, settings_path_from_platform,
};
use envr_domain::jvm_hosted;
use envr_error::{EnvrError, EnvrResult};
use envr_platform::paths::current_platform_paths;
// Re-export merge helpers for callers that used `child_env::path_sep` / `prepend_path` / …
#[allow(unused_imports)]
pub use envr_resolver::{
    dedup_paths, path_sep, prepend_path, runtime_bin_dirs, runtime_error_might_install_fix,
    version_label_from_runtime_home,
};
use envr_resolver::{
    extend_env_with_tooling_settings, plan_missing_installable_pins, resolve_exec_lang_home,
    resolve_run_lang_home,
};
use envr_shim_core::{
    ShimContext, resolve_runtime_home_for_lang_with_project, runtime_home_env_for_key,
};
use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;

fn with_project_config_ref<R>(
    ctx: &ShimContext,
    cached: Option<&ProjectConfig>,
    f: impl FnOnce(Option<&ProjectConfig>) -> EnvrResult<R>,
) -> EnvrResult<R> {
    if let Some(c) = cached {
        return f(Some(c));
    }
    let loaded = load_project_config_profile(&ctx.working_dir, ctx.profile.as_deref())?;
    f(loaded.as_ref().map(|(cfg, _)| cfg))
}

/// Human line for `exec --verbose` (language + version label + home path).
pub fn describe_exec_resolution(
    ctx: &ShimContext,
    lang: &str,
    spec_override: Option<&str>,
    project: Option<&ProjectConfig>,
) -> EnvrResult<String> {
    if lang == "rust" {
        return Ok(describe_rust_exec_line(ctx));
    }
    let home = resolve_exec_home_for_lang(ctx, lang, spec_override, project)?;
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
    project: Option<&ProjectConfig>,
) -> EnvrResult<Vec<String>> {
    with_project_config_ref(ctx, project, |cfg| {
        let stack = RunEnvStack::new(ctx, cfg, install_if_missing);
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
    })
}

fn load_settings() -> EnvrResult<Settings> {
    let paths = current_platform_paths()?;
    let sp = settings_path_from_platform(&paths);
    Settings::load_or_default_from(&sp)
}

pub(crate) fn base_env_with_project_env(cfg: Option<&ProjectConfig>) -> HashMap<String, String> {
    let mut env: HashMap<String, String> = std::env::vars().collect();
    if let Some(c) = cfg {
        for (k, v) in &c.env {
            env.insert(k.clone(), v.clone());
        }
    }
    env
}

/// After `--spec` override, fall back to the project pin for `lang` (for install + retry).
pub fn effective_install_spec_for_exec(
    ctx: &ShimContext,
    lang: &str,
    spec_override: Option<&str>,
    project: Option<&ProjectConfig>,
) -> EnvrResult<Option<String>> {
    if let Some(s) = spec_override.map(str::trim).filter(|s| !s.is_empty()) {
        return Ok(Some(s.to_string()));
    }
    with_project_config_ref(ctx, project, |cfg| {
        Ok(cfg.and_then(|c| {
            c.runtimes
                .get(lang)
                .and_then(|r| r.version.as_ref())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        }))
    })
}

/// Pairs `(lang, version_spec)` for pinned runtimes that are missing on disk but look installable.
pub fn plan_missing_pinned_runtimes_for_run(
    ctx: &ShimContext,
    project: Option<&ProjectConfig>,
) -> EnvrResult<Vec<(String, String)>> {
    with_project_config_ref(ctx, project, |cfg| {
        Ok(plan_missing_installable_pins(cfg, |lang| {
            resolve_run_lang_home(ctx, cfg, lang).map(|_| ())
        }))
    })
}

/// Resolve the runtime home directory for `exec` (same pin/current rules as `collect_exec_env`).
///
/// Note: On Windows, process spawning may resolve the executable before applying the child's
/// environment block, so callers may prefer using the returned home to build an absolute tool path.
pub fn resolve_exec_home_for_lang(
    ctx: &ShimContext,
    lang: &str,
    spec_override: Option<&str>,
    project: Option<&ProjectConfig>,
) -> EnvrResult<PathBuf> {
    with_project_config_ref(ctx, project, |cfg| {
        resolve_exec_lang_home(ctx, cfg, lang, spec_override)
    })
}

/// Single-language resolution: prepend that runtime's bin dirs to `PATH`, then apply runtime-home
/// env keys from shared policy hooks.
pub fn collect_exec_env(
    ctx: &ShimContext,
    lang: &str,
    spec_override: Option<&str>,
    project: Option<&ProjectConfig>,
) -> EnvrResult<HashMap<String, String>> {
    with_project_config_ref(ctx, project, |cfg| {
        let mut env: HashMap<String, String> = std::env::vars().collect();
        if let Some(c) = cfg {
            for (k, v) in &c.env {
                env.insert(k.clone(), v.clone());
            }
        }
        match resolve_exec_lang_layer(ctx, cfg, lang, spec_override)? {
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
                for (k, v) in runtime_home_env_for_key(&home, lang) {
                    env.insert(k, v);
                }
                if jvm_hosted::is_jvm_hosted_runtime(&lang) {
                    let java_home = resolve_runtime_home_for_lang_with_project(
                        ctx,
                        "java",
                        spec_override,
                        cfg,
                    )?;
                    let runtime_label = home.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    let java_label = java_home.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    if !runtime_label.is_empty()
                        && let Some(msg) = jvm_hosted::hosted_runtime_jdk_mismatch_message(
                            lang,
                            runtime_label,
                            java_label,
                        )
                    {
                        return Err(EnvrError::Validation(msg));
                    }
                    let java_home = std::fs::canonicalize(&java_home).map_err(EnvrError::from)?;
                    for (k, v) in runtime_home_env_for_key(&java_home, "java") {
                        env.insert(k, v);
                    }
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
    })
}

fn template_version_key_for_lang(lang: &str) -> Option<&'static str> {
    match lang {
        "node" => Some("ENVR_NODE_VERSION"),
        "python" => Some("ENVR_PYTHON_VERSION"),
        "java" => Some("ENVR_JAVA_VERSION"),
        "go" => Some("ENVR_GO_VERSION"),
        "ruby" => Some("ENVR_RUBY_VERSION"),
        "elixir" => Some("ENVR_ELIXIR_VERSION"),
        "erlang" => Some("ENVR_ERLANG_VERSION"),
        "php" => Some("ENVR_PHP_VERSION"),
        "deno" => Some("ENVR_DENO_VERSION"),
        "bun" => Some("ENVR_BUN_VERSION"),
        "dotnet" => Some("ENVR_DOTNET_VERSION"),
        "zig" => Some("ENVR_ZIG_VERSION"),
        "julia" => Some("ENVR_JULIA_VERSION"),
        "lua" => Some("ENVR_LUA_VERSION"),
        "nim" => Some("ENVR_NIM_VERSION"),
        "crystal" => Some("ENVR_CRYSTAL_VERSION"),
        "perl" => Some("ENVR_PERL_VERSION"),
        "r" => Some("ENVR_R_VERSION"),
        "kotlin" => Some("ENVR_KOTLIN_VERSION"),
        "scala" => Some("ENVR_SCALA_VERSION"),
        "clojure" => Some("ENVR_CLOJURE_VERSION"),
        "groovy" => Some("ENVR_GROOVY_VERSION"),
        "terraform" => Some("ENVR_TERRAFORM_VERSION"),
        "v" => Some("ENVR_V_VERSION"),
        "dart" => Some("ENVR_DART_VERSION"),
        "flutter" => Some("ENVR_FLUTTER_VERSION"),
        _ => None,
    }
}

fn join_path_entries(entries: &[PathBuf]) -> String {
    let sep = path_sep().to_string();
    entries
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join(&sep)
}

#[cfg(windows)]
fn compose_run_path(
    front_entries: &[PathBuf],
    old_path: &str,
    java_suffix_entries: &[PathBuf],
) -> String {
    let mut merged = prepend_path(front_entries, old_path);
    if !java_suffix_entries.is_empty() {
        if !merged.is_empty() {
            merged.push(path_sep());
        }
        merged.push_str(&join_path_entries(java_suffix_entries));
    }
    merged
}

#[cfg(not(windows))]
fn compose_run_path(
    front_entries: &[PathBuf],
    old_path: &str,
    _java_suffix_entries: &[PathBuf],
) -> String {
    prepend_path(front_entries, old_path)
}

fn collect_run_env_impl(
    ctx: &ShimContext,
    install_if_missing: bool,
    collect_template_keys: bool,
    template_keys: &mut HashMap<String, String>,
    cfg: Option<&ProjectConfig>,
) -> EnvrResult<HashMap<String, String>> {
    let mut env = base_env_with_project_env(cfg);
    let mut path_entries: Vec<PathBuf> = Vec::new();
    let mut runtime_home_env: HashMap<String, String> = HashMap::new();
    let mut java_suffix_entries: Vec<PathBuf> = Vec::new();
    let mut deno_on_path = false;
    let mut bun_on_path = false;
    let stack = RunEnvStack::new(ctx, cfg, install_if_missing);
    for &lang in RUN_STACK_LANG_ORDER {
        let Some(piece) = stack.resolve_lang(lang)? else {
            continue;
        };
        match piece {
            RunStackLang::RustBins(bins) => {
                path_entries.extend(bins);
                if collect_template_keys {
                    let tc = rustup_active_toolchain(ctx).unwrap_or_else(|| "default".into());
                    template_keys.insert("ENVR_RUST_VERSION".into(), tc);
                }
            }
            RunStackLang::Runtime { lang, home } => {
                if collect_template_keys && let Some(key) = template_version_key_for_lang(&lang) {
                    let ver = version_label_from_runtime_home(&home);
                    template_keys.insert(key.to_string(), ver);
                }
                for (k, v) in runtime_home_env_for_key(&home, &lang) {
                    runtime_home_env.insert(k, v);
                }
                if jvm_hosted::is_jvm_hosted_runtime(&lang) {
                    let java_home = resolve_run_lang_home(ctx, cfg, "java")?;
                    let java_home = std::fs::canonicalize(&java_home).unwrap_or(java_home);
                    let runtime_label = home.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    let java_label = java_home.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    if let Some(msg) = jvm_hosted::hosted_runtime_jdk_mismatch_message(
                        &lang,
                        runtime_label,
                        java_label,
                    ) {
                        return Err(EnvrError::Validation(msg));
                    }
                    for (k, v) in runtime_home_env_for_key(&java_home, "java") {
                        runtime_home_env.insert(k, v);
                    }
                }
                if lang == "deno" {
                    deno_on_path = true;
                }
                if lang == "bun" {
                    bun_on_path = true;
                }
                let dirs = runtime_bin_dirs(&home, &lang);
                #[cfg(windows)]
                if lang == "java" {
                    java_suffix_entries.extend(dirs);
                } else {
                    path_entries.extend(dirs);
                }
                #[cfg(not(windows))]
                {
                    path_entries.extend(dirs);
                }
            }
        }
    }
    path_entries = dedup_paths(path_entries);
    java_suffix_entries = dedup_paths(java_suffix_entries);
    let old_path = env.get("PATH").map(|s| s.as_str()).unwrap_or("");
    env.insert(
        "PATH".into(),
        compose_run_path(&path_entries, old_path, &java_suffix_entries),
    );
    for (k, v) in runtime_home_env {
        env.insert(k, v);
    }
    let st = load_settings()?;
    extend_env_with_tooling_settings(&mut env, &st, true, deno_on_path, bun_on_path);
    Ok(env)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn compose_run_path_puts_java_dirs_at_tail_on_windows() {
        let front = vec![
            PathBuf::from(r"D:\envr\shims"),
            PathBuf::from(r"D:\envr\node\bin"),
        ];
        let old = r"C:\Windows\System32;C:\Windows";
        let java_tail = vec![PathBuf::from(r"D:\envr\java\bin")];
        let merged = compose_run_path(&front, old, &java_tail);
        assert!(merged.starts_with(r"D:\envr\shims;D:\envr\node\bin;"));
        assert!(merged.ends_with(r";D:\envr\java\bin"));
    }
}

/// Multi-runtime PATH plus runtime-home environment keys such as `JAVA_HOME`, `GOROOT`, and
/// `.NET` root variables when the corresponding runtimes resolve.
///
/// When `install_if_missing` is true, resolution failures for languages pinned in `.envr.toml`
/// are propagated instead of silently omitting that runtime from `PATH`.
pub fn collect_run_env(
    ctx: &ShimContext,
    install_if_missing: bool,
    project: Option<&ProjectConfig>,
) -> EnvrResult<HashMap<String, String>> {
    with_project_config_ref(ctx, project, |cfg_ref| {
        let mut discard = HashMap::new();
        collect_run_env_impl(ctx, install_if_missing, false, &mut discard, cfg_ref)
    })
}

/// Same as [`collect_run_env`] plus `ENVR_*_VERSION` keys (same semantics as `run --verbose` lines).
pub fn collect_run_env_for_template(
    ctx: &ShimContext,
    project: Option<&ProjectConfig>,
) -> EnvrResult<HashMap<String, String>> {
    with_project_config_ref(ctx, project, |cfg_ref| {
        let mut tmpl = HashMap::new();
        let mut env = collect_run_env_impl(ctx, false, true, &mut tmpl, cfg_ref)?;
        for (k, v) in tmpl {
            env.insert(k, v);
        }
        Ok(env)
    })
}

/// Variable names that `envr env` / `collect_run_env` may change for the given working directory,
/// used by `envr hook` to save/restore shell state when leaving a project directory.
pub fn hook_env_restore_keys(ctx: &ShimContext) -> EnvrResult<Vec<String>> {
    let mut keys: BTreeSet<String> = [
        "PATH",
        "JAVA_HOME",
        "ERLANG_HOME",
        "GOROOT",
        "DOTNET_ROOT",
        "DOTNET_MULTILEVEL_LOOKUP",
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
