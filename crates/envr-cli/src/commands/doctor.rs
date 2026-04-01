use crate::cli::GlobalArgs;
use crate::commands::common::{self, kind_label};
use crate::output;

use envr_core::runtime::service::RuntimeService;
use envr_domain::runtime::RuntimeKind;
use envr_error::EnvrError;
use serde_json::Value;
use std::path::{Path, PathBuf};

const ALL_KINDS: [RuntimeKind; 8] = [
    RuntimeKind::Node,
    RuntimeKind::Python,
    RuntimeKind::Java,
    RuntimeKind::Go,
    RuntimeKind::Rust,
    RuntimeKind::Php,
    RuntimeKind::Deno,
    RuntimeKind::Bun,
];

/// Same payload as `envr doctor` JSON `data` field (for `diagnostics export` and tests).
#[derive(Debug, Clone)]
pub(crate) struct DoctorReport {
    pub root: PathBuf,
    pub env_override: Option<String>,
    pub issues: Vec<String>,
    pub recommendations: Vec<String>,
    /// `(kind_label, installed_count, current_version)`
    pub kinds: Vec<(String, usize, Option<String>)>,
}

impl DoctorReport {
    pub fn ok(&self) -> bool {
        self.issues.is_empty()
    }

    pub fn to_json(&self) -> Value {
        let kinds_json: Vec<_> = self
            .kinds
            .iter()
            .map(|(k, n, cur)| {
                serde_json::json!({
                    "kind": k,
                    "installed_count": n,
                    "current": cur,
                })
            })
            .collect();

        serde_json::json!({
            "runtime_root": self.root.to_string_lossy(),
            "envr_runtime_root_env": self.env_override,
            "kinds": kinds_json,
            "issues": self.issues,
            "recommendations": self.recommendations,
        })
    }
}

pub(crate) fn build_doctor_report(service: &RuntimeService) -> Result<DoctorReport, EnvrError> {
    let root = common::effective_runtime_root()?;

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

    let mut kinds: Vec<(String, usize, Option<String>)> = Vec::new();

    for kind in ALL_KINDS {
        let label = kind_label(kind).to_string();
        let installed = match service.list_installed(kind) {
            Ok(v) => v,
            Err(e) => {
                issues.push(format!("{}: list_installed failed: {e}", kind_label(kind)));
                kinds.push((label, 0, None));
                continue;
            }
        };
        let current = match service.current(kind) {
            Ok(c) => c,
            Err(e) => {
                issues.push(format!("{}: current failed: {e}", kind_label(kind)));
                kinds.push((label, installed.len(), None));
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

        kinds.push((label, installed.len(), current.map(|v| v.0)));
    }

    Ok(DoctorReport {
        root,
        env_override,
        issues,
        recommendations,
        kinds,
    })
}

pub fn run(g: &GlobalArgs, service: &RuntimeService) -> i32 {
    let report = match build_doctor_report(service) {
        Ok(r) => r,
        Err(e) => return common::print_envr_error(g, e),
    };

    let data = report.to_json();
    let ok = report.ok();
    let (message, code_if_fail) = if ok {
        ("doctor_ok", None)
    } else {
        ("environment checks found problems", Some("doctor_issues"))
    };

    output::emit_doctor(g, ok, message, code_if_fail, data, || {
        println!("runtime root: {}", report.root.display());
        if let Some(ref e) = report.env_override {
            println!("ENVR_RUNTIME_ROOT: {e}");
        }
        println!();
        for (kind, ic, cur) in &report.kinds {
            match cur {
                Some(v) => println!("{kind}: {ic} installed, current = {v}"),
                None => println!("{kind}: {ic} installed, current = (none)"),
            }
        }
        if !report.issues.is_empty() {
            println!("\nIssues:");
            for i in &report.issues {
                println!("  - {i}");
            }
        }
        if !report.recommendations.is_empty() && !g.quiet {
            println!("\nSuggestions:");
            for r in &report.recommendations {
                println!("  - {r}");
            }
        }
    })
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
