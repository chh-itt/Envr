//! Plan which pinned runtimes are missing on disk but may be installable.
//!
//! Resolution of actual paths stays in `envr-cli` / `envr-shim-core`; this module owns the
//! **iteration order**, **pin extraction** from [`ProjectConfig`], and the **install-fix** heuristic.

use envr_config::project_config::ProjectConfig;
use envr_error::EnvrError;

/// Languages considered by `envr project sync` / missing-pin planning when checking pins (fixed order).
pub const RUNTIME_PLAN_ORDER: &[&str] = &[
    "node",
    "python",
    "java",
    "kotlin",
    "scala",
    "clojure",
    "groovy",
    "terraform",
    "v",
    "dart",
    "flutter",
    "go",
    "ruby",
    "elixir",
    "erlang",
    "php",
    "deno",
    "bun",
    "dotnet",
    "zig",
    "julia",
    "lua",
    "nim",
    "crystal",
    "r",
];

/// True when a failed resolution likely means "nothing installed for this spec yet" and
/// [`envr_core::runtime::RuntimeService::install`] may create the missing tree.
pub fn runtime_error_might_install_fix(err: &EnvrError) -> bool {
    match err {
        EnvrError::Runtime(msg) => {
            msg.contains("no installed version matches")
                || msg.contains("no installed php matches")
                || msg.contains("no versions directory at")
        }
        _ => false,
    }
}

fn trimmed_pin(cfg: Option<&ProjectConfig>, lang: &str) -> Option<String> {
    cfg.and_then(|c| c.runtimes.get(lang))
        .and_then(|r| r.version.as_deref())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

/// Lists `(lang, version_spec)` for every pinned runtime in [`RUNTIME_PLAN_ORDER`] (stable order).
pub fn list_pinned_runtime_specs(cfg: Option<&ProjectConfig>) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for &lang in RUNTIME_PLAN_ORDER {
        if let Some(spec) = trimmed_pin(cfg, lang) {
            out.push((lang.to_string(), spec));
        }
    }
    out
}

/// For each pinned language, calls `try_resolve(lang)`. Collects `(lang, spec)` when resolution
/// fails with an error that [`runtime_error_might_install_fix`] treats as installable.
pub fn plan_missing_installable_pins(
    cfg: Option<&ProjectConfig>,
    mut try_resolve: impl FnMut(&str) -> Result<(), EnvrError>,
) -> Vec<(String, String)> {
    let mut pending = Vec::new();
    for &lang in RUNTIME_PLAN_ORDER {
        let Some(spec) = trimmed_pin(cfg, lang) else {
            continue;
        };
        match try_resolve(lang) {
            Ok(()) => {}
            Err(e) if runtime_error_might_install_fix(&e) => {
                pending.push((lang.to_string(), spec));
            }
            Err(_) => {}
        }
    }
    pending
}

#[cfg(test)]
mod tests {
    use super::*;
    use envr_config::project_config::RuntimeConfig;
    use std::collections::HashMap;

    fn cfg_with_node_pin(version: &str) -> ProjectConfig {
        let mut runtimes = HashMap::new();
        runtimes.insert(
            "node".to_string(),
            RuntimeConfig {
                version: Some(version.to_string()),
                channel: None,
                version_prefix: None,
                enforce: None,
            },
        );
        ProjectConfig {
            env: HashMap::new(),
            runtimes,
            profiles: HashMap::new(),
            ..Default::default()
        }
    }

    #[test]
    fn list_pinned_empty_cfg() {
        assert!(list_pinned_runtime_specs(None).is_empty());
    }

    #[test]
    fn list_pinned_respects_plan_order() {
        let mut runtimes = HashMap::new();
        for (k, v) in [("bun", "1.0"), ("node", "20"), ("python", "3.12")] {
            runtimes.insert(
                k.to_string(),
                RuntimeConfig {
                    version: Some(v.to_string()),
                    channel: None,
                    version_prefix: None,
                    enforce: None,
                },
            );
        }
        let cfg = ProjectConfig {
            env: HashMap::new(),
            runtimes,
            profiles: HashMap::new(),
            ..Default::default()
        };
        let langs: Vec<_> = list_pinned_runtime_specs(Some(&cfg))
            .into_iter()
            .map(|(l, _)| l)
            .collect();
        assert_eq!(langs, vec!["node", "python", "bun"]);
    }

    #[test]
    fn plan_collects_only_install_fix_errors() {
        let cfg = cfg_with_node_pin("20");
        let pending = plan_missing_installable_pins(Some(&cfg), |lang| {
            assert_eq!(lang, "node");
            Err(EnvrError::Runtime("no installed version matches 20".into()))
        });
        assert_eq!(pending, vec![("node".to_string(), "20".to_string())]);
    }

    #[test]
    fn plan_skips_validation_like_errors() {
        let cfg = cfg_with_node_pin("20");
        let pending =
            plan_missing_installable_pins(Some(&cfg), |_| Err(EnvrError::Validation("bad".into())));
        assert!(pending.is_empty());
    }

    #[test]
    fn plan_ok_means_satisfied() {
        let cfg = cfg_with_node_pin("20");
        let pending = plan_missing_installable_pins(Some(&cfg), |_| Ok(()));
        assert!(pending.is_empty());
    }
}
