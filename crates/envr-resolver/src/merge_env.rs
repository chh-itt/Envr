//! Pure PATH merging, runtime bin layout, and settings-derived toolchain env extensions.
//!
//! Callers in `envr-cli` perform I/O (load settings, resolve homes); this module applies
//! deterministic rules so GUI / future hosts can reuse the same behavior.

use envr_config::settings::{
    GoProxyMode, Settings, bun_package_registry_env, deno_package_registry_env,
    prefer_china_mirrors,
};
use envr_shim_core::runtime_bin_dirs_for_key;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// `PATH` separator for the current platform (`;` on Windows, `:` elsewhere).
pub fn path_sep() -> char {
    if cfg!(windows) { ';' } else { ':' }
}

/// Directory name under `.../versions/<name>` when applicable; otherwise the last path component.
pub fn version_label_from_runtime_home(home: &Path) -> String {
    if let Some(parent) = home.parent()
        && parent
            .file_name()
            .and_then(|s| s.to_str())
            .is_some_and(|n| n == "versions")
        && let Some(leaf) = home.file_name().and_then(|s| s.to_str())
    {
        return leaf.to_string();
    }
    home.file_name()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| home.display().to_string())
}

/// Prepend `entries` to `existing` using the platform `PATH` separator.
pub fn prepend_path(entries: &[PathBuf], existing: &str) -> String {
    let sep = path_sep();
    let mut parts: Vec<String> = entries.iter().map(|p| p.display().to_string()).collect();
    if !existing.is_empty() {
        parts.push(existing.to_string());
    }
    parts.join(&sep.to_string())
}

/// Bin / root dirs to put on `PATH` for a resolved runtime home (order matters).
pub fn runtime_bin_dirs(home: &Path, lang: &str) -> Vec<PathBuf> {
    runtime_bin_dirs_for_key(home, lang)
}

/// First-seen wins; stable order preserved.
pub fn dedup_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
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

/// `GOPROXY` / `GOPRIVATE`-style keys derived from settings (same rules as `envr run` / `exec`).
pub fn go_env_from_settings(settings: &Settings) -> Vec<(String, String)> {
    let mut out = Vec::new();

    let legacy = settings.runtime.go.goproxy.as_deref().unwrap_or("").trim();
    let custom = settings
        .runtime
        .go
        .proxy_custom
        .as_deref()
        .unwrap_or("")
        .trim();
    let gp = match settings.runtime.go.proxy_mode {
        GoProxyMode::Auto => {
            if !legacy.is_empty() {
                legacy.to_string()
            } else if prefer_china_mirrors(settings) {
                "https://goproxy.cn,direct".to_string()
            } else {
                "https://proxy.golang.org,direct".to_string()
            }
        }
        GoProxyMode::Domestic => "https://goproxy.cn,direct".to_string(),
        GoProxyMode::Official => "https://proxy.golang.org,direct".to_string(),
        GoProxyMode::Direct => "direct".to_string(),
        GoProxyMode::Custom => {
            if !custom.is_empty() {
                custom.to_string()
            } else {
                legacy.to_string()
            }
        }
    };
    if !gp.trim().is_empty() {
        out.push(("GOPROXY".into(), gp));
    }

    if let Some(p) = settings.runtime.go.private_patterns.as_deref() {
        let v = p.trim();
        if !v.is_empty() {
            let s = v.to_string();
            out.push(("GOPRIVATE".into(), s.clone()));
            out.push(("GONOSUMDB".into(), s.clone()));
            out.push(("GONOPROXY".into(), s));
        }
    }
    out
}

/// Inject Go / Deno / Bun registry-related keys according to flags (matches `collect_run_env` /
/// `collect_exec_env` behavior).
pub fn extend_env_with_tooling_settings(
    env: &mut HashMap<String, String>,
    settings: &Settings,
    inject_go: bool,
    inject_deno: bool,
    inject_bun: bool,
) {
    if inject_go {
        for (k, v) in go_env_from_settings(settings) {
            env.insert(k, v);
        }
    }
    if inject_deno {
        for (k, v) in deno_package_registry_env(settings) {
            env.insert(k, v);
        }
    }
    if inject_bun {
        for (k, v) in bun_package_registry_env(settings) {
            env.insert(k, v);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prepend_path_preserves_order() {
        let p = vec![PathBuf::from("/a"), PathBuf::from("/b")];
        let s = prepend_path(&p, "/old");
        assert!(s.starts_with("/a"));
        assert!(s.contains("/b"));
        assert!(s.ends_with("/old"));
    }

    #[test]
    fn dedup_drops_second_identical_display() {
        let d = dedup_paths(vec![
            PathBuf::from("/x"),
            PathBuf::from("/y"),
            PathBuf::from("/x"),
        ]);
        assert_eq!(d.len(), 2);
    }

    #[test]
    fn version_label_versions_dir_leaf() {
        let h = PathBuf::from("/r/runtimes/node/versions/20.1.0");
        assert_eq!(version_label_from_runtime_home(&h), "20.1.0");
    }
}
