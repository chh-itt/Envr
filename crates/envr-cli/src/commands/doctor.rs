use crate::cli::{GlobalArgs, OutputFormat};
use crate::commands::common::{self, kind_label};

use envr_core::runtime::service::RuntimeService;
use envr_domain::runtime::RuntimeKind;
use std::path::Path;

const ALL_KINDS: [RuntimeKind; 3] = [RuntimeKind::Node, RuntimeKind::Python, RuntimeKind::Java];

pub fn run(g: &GlobalArgs, service: &RuntimeService) -> i32 {
    let root = match common::effective_runtime_root() {
        Ok(r) => r,
        Err(e) => return common::print_envr_error(g, e),
    };

    let env_override = std::env::var("ENVR_RUNTIME_ROOT")
        .ok()
        .filter(|s| !s.is_empty());

    let mut issues = Vec::new();
    let mut recommendations = Vec::new();

    if !root.exists() {
        issues.push("runtime data root does not exist".to_string());
        recommendations.push(format!(
            "create `{}` or set ENVR_RUNTIME_ROOT to a writable directory",
            root.display()
        ));
    } else if !runtime_root_writable(&root) {
        issues.push(format!(
            "runtime data root is not writable: {}",
            root.display()
        ));
        recommendations
            .push("fix directory permissions or choose another ENVR_RUNTIME_ROOT".to_string());
    }

    let shims = root.join("shims");
    if shims.is_dir() {
        let empty = std::fs::read_dir(&shims)
            .map(|mut d| d.next().is_none())
            .unwrap_or(true);
        if empty {
            recommendations.push(
                "`shims` directory is empty; after installing runtimes, add `shims` to PATH or refresh shims when integrated"
                    .to_string(),
            );
        } else {
            recommendations.push(format!(
                "ensure `{}` is on your PATH ahead of other tool copies",
                shims.display()
            ));
        }
    }

    let mut rows: Vec<(&'static str, usize, Option<String>)> = Vec::new();

    for kind in ALL_KINDS {
        let installed = match service.list_installed(kind) {
            Ok(v) => v,
            Err(e) => {
                issues.push(format!("{}: list_installed failed: {e}", kind_label(kind)));
                rows.push((kind_label(kind), 0, None));
                continue;
            }
        };
        let current = match service.current(kind) {
            Ok(c) => c,
            Err(e) => {
                issues.push(format!("{}: current failed: {e}", kind_label(kind)));
                rows.push((kind_label(kind), installed.len(), None));
                continue;
            }
        };

        if !installed.is_empty() && current.is_none() {
            recommendations.push(format!(
                "{} has installed versions but no `current` symlink; run `envr use {} <version>`",
                kind_label(kind),
                kind_label(kind)
            ));
        }

        rows.push((kind_label(kind), installed.len(), current.map(|v| v.0)));
    }

    match g.output_format.unwrap_or(OutputFormat::Text) {
        OutputFormat::Json => {
            let kinds: Vec<_> = rows
                .iter()
                .map(|(k, n, cur)| {
                    serde_json::json!({
                        "kind": k,
                        "installed_count": n,
                        "current": cur,
                    })
                })
                .collect();
            println!(
                "{}",
                serde_json::json!({
                    "success": issues.is_empty(),
                    "data": {
                        "runtime_root": root.to_string_lossy(),
                        "envr_runtime_root_env": env_override,
                        "kinds": kinds,
                        "issues": issues,
                        "recommendations": recommendations,
                    },
                    "diagnostics": [],
                })
            );
        }
        OutputFormat::Text => {
            println!("runtime root: {}", root.display());
            if let Some(ref e) = env_override {
                println!("ENVR_RUNTIME_ROOT: {e}");
            }
            println!();
            for (kind, ic, cur) in &rows {
                match cur {
                    Some(v) => println!("{kind}: {ic} installed, current = {v}"),
                    None => println!("{kind}: {ic} installed, current = (none)"),
                }
            }
            if !issues.is_empty() {
                println!("\nIssues:");
                for i in &issues {
                    println!("  - {i}");
                }
            }
            if !recommendations.is_empty() && !g.quiet {
                println!("\nSuggestions:");
                for r in &recommendations {
                    println!("  - {r}");
                }
            }
        }
    }

    if issues.is_empty() { 0 } else { 1 }
}

fn runtime_root_writable(root: &Path) -> bool {
    let probe = root.join(".envr-doctor-probe");
    match std::fs::write(&probe, b"ok") {
        Ok(()) => {
            let _ = std::fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}
