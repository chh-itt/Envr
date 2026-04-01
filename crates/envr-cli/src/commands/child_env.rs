//! Build merged process environment for `exec`, `run`, and `env` (PATH, JAVA_HOME, project `env`).

use envr_config::project_config::load_project_config_profile;
use envr_error::{EnvrError, EnvrResult};
use envr_shim_core::{ShimContext, resolve_runtime_home_for_lang};
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
    let home = resolve_runtime_home_for_lang(ctx, lang, spec_override)?;
    let home = std::fs::canonicalize(&home).map_err(EnvrError::from)?;
    let bins = dedup_paths(runtime_bin_dirs(&home, lang));
    let old_path = env.get("PATH").map(|s| s.as_str()).unwrap_or("");
    env.insert("PATH".into(), prepend_path(&bins, old_path));
    if lang == "java" {
        env.insert("JAVA_HOME".into(), home.display().to_string());
    }
    Ok(env)
}

/// Multi-runtime PATH (node, python, java) plus one `JAVA_HOME` when Java resolves.
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
    for lang in ["node", "python", "java"] {
        let Ok(home) = resolve_runtime_home_for_lang(ctx, lang, None) else {
            continue;
        };
        let home = std::fs::canonicalize(&home).map_err(EnvrError::from)?;
        if lang == "java" {
            java_home = Some(home.clone());
        }
        path_entries.extend(runtime_bin_dirs(&home, lang));
    }
    path_entries = dedup_paths(path_entries);
    let old_path = env.get("PATH").map(|s| s.as_str()).unwrap_or("");
    env.insert("PATH".into(), prepend_path(&path_entries, old_path));
    if let Some(jh) = java_home {
        env.insert("JAVA_HOME".into(), jh.display().to_string());
    }
    Ok(env)
}
