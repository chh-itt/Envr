use crate::cli::GlobalArgs;
use crate::commands::common::{self, kind_label};
use crate::output::{self, fmt_template};

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
        issues.push(envr_core::i18n::tr_key(
            "cli.doctor.issue.root_missing",
            "运行时数据根目录不存在",
            "runtime data root does not exist",
        ));
        recommendations.push(fmt_template(
            &envr_core::i18n::tr_key(
                "cli.doctor.rec.root_create",
                "请创建 `{path}`，或将 ENVR_RUNTIME_ROOT 设为可写目录",
                "create `{path}` or set ENVR_RUNTIME_ROOT to a writable directory",
            ),
            &[("path", &root.display().to_string())],
        ));
    } else if !runtime_root_writable(&root) {
        issues.push(fmt_template(
            &envr_core::i18n::tr_key(
                "cli.doctor.issue.root_not_writable",
                "运行时数据根目录不可写：{path}",
                "runtime data root is not writable: {path}",
            ),
            &[("path", &root.display().to_string())],
        ));
        recommendations.push(envr_core::i18n::tr_key(
            "cli.doctor.rec.root_permissions",
            "请修复目录权限或更换 ENVR_RUNTIME_ROOT",
            "fix directory permissions or choose another ENVR_RUNTIME_ROOT",
        ));
    }

    let shims = root.join("shims");
    if shims.is_dir() {
        let empty = std::fs::read_dir(&shims)
            .map(|mut d| d.next().is_none())
            .unwrap_or(true);
        if empty {
            recommendations.push(envr_core::i18n::tr_key(
                "cli.doctor.rec.shims_empty",
                "`shims` 目录为空；安装运行时后请将 `shims` 加入 PATH，或在集成环境中刷新 shims",
                "`shims` directory is empty; after installing runtimes, add `shims` to PATH or refresh shims when integrated",
            ));
        } else {
            recommendations.push(fmt_template(
                &envr_core::i18n::tr_key(
                    "cli.doctor.rec.shims_path",
                    "请确保 `{path}` 在 PATH 中且优先于其他同名工具",
                    "ensure `{path}` is on your PATH ahead of other tool copies",
                ),
                &[("path", &shims.display().to_string())],
            ));
        }
    }

    let mut kinds: Vec<(String, usize, Option<String>)> = Vec::new();

    for kind in ALL_KINDS {
        let label = kind_label(kind).to_string();
        let installed = match service.list_installed(kind) {
            Ok(v) => v,
            Err(e) => {
                issues.push(fmt_template(
                    &envr_core::i18n::tr_key(
                        "cli.doctor.issue.list_failed",
                        "{kind}：list_installed 失败：{detail}",
                        "{kind}: list_installed failed: {detail}",
                    ),
                    &[("kind", kind_label(kind)), ("detail", &e.to_string())],
                ));
                kinds.push((label, 0, None));
                continue;
            }
        };
        let current = match service.current(kind) {
            Ok(c) => c,
            Err(e) => {
                issues.push(fmt_template(
                    &envr_core::i18n::tr_key(
                        "cli.doctor.issue.current_failed",
                        "{kind}：current 失败：{detail}",
                        "{kind}: current failed: {detail}",
                    ),
                    &[("kind", kind_label(kind)), ("detail", &e.to_string())],
                ));
                kinds.push((label, installed.len(), None));
                continue;
            }
        };

        if !installed.is_empty() && current.is_none() {
            recommendations.push(fmt_template(
                &envr_core::i18n::tr_key(
                    "cli.doctor.rec.no_current",
                    "{kind} 已安装版本但未设置 `current` 符号链接；请运行 `envr use {kind} <version>`",
                    "{kind} has installed versions but no `current` symlink; run `envr use {kind} <version>`",
                ),
                &[("kind", kind_label(kind))],
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
    let none_label = envr_core::i18n::tr_key("cli.common.none", "（无）", "(none)");

    if ok {
        output::emit_doctor(g, ok, "doctor_ok", None, data, || {
            println!(
                "{} {}",
                envr_core::i18n::tr_key(
                    "cli.doctor.runtime_root_label",
                    "运行时根目录：",
                    "runtime root:",
                ),
                report.root.display()
            );
            if let Some(ref e) = report.env_override {
                println!(
                    "{} {e}",
                    envr_core::i18n::tr_key(
                        "cli.doctor.env_override_label",
                        "ENVR_RUNTIME_ROOT：",
                        "ENVR_RUNTIME_ROOT:",
                    )
                );
            }
            println!();
            for (kind, ic, cur) in &report.kinds {
                match cur {
                    Some(v) => println!(
                        "{}",
                        fmt_template(
                            &envr_core::i18n::tr_key(
                                "cli.doctor.line_installed_current",
                                "{kind}：已安装 {count} 个版本，当前 = {current}",
                                "{kind}: {count} installed, current = {current}",
                            ),
                            &[("kind", kind), ("count", &ic.to_string()), ("current", v),],
                        )
                    ),
                    None => println!(
                        "{}",
                        fmt_template(
                            &envr_core::i18n::tr_key(
                                "cli.doctor.line_installed_none",
                                "{kind}：已安装 {count} 个版本，当前 = {none}",
                                "{kind}: {count} installed, current = {none}",
                            ),
                            &[
                                ("kind", kind),
                                ("count", &ic.to_string()),
                                ("none", &none_label),
                            ],
                        )
                    ),
                }
            }
            if !report.issues.is_empty() {
                println!(
                    "\n{}",
                    envr_core::i18n::tr_key("cli.doctor.issues_heading", "问题：", "Issues:",)
                );
                for i in &report.issues {
                    println!("  - {i}");
                }
            }
            if !report.recommendations.is_empty() && !g.quiet {
                println!(
                    "\n{}",
                    envr_core::i18n::tr_key(
                        "cli.doctor.suggestions_heading",
                        "建议：",
                        "Suggestions:",
                    )
                );
                for r in &report.recommendations {
                    println!("  - {r}");
                }
            }
        })
    } else {
        let fail_msg = envr_core::i18n::tr_key(
            "cli.doctor.json_fail_message",
            "环境检查发现问题",
            "environment checks found problems",
        );
        output::emit_doctor(g, ok, &fail_msg, Some("doctor_issues"), data, || {
            println!(
                "{} {}",
                envr_core::i18n::tr_key(
                    "cli.doctor.runtime_root_label",
                    "运行时根目录：",
                    "runtime root:",
                ),
                report.root.display()
            );
            if let Some(ref e) = report.env_override {
                println!(
                    "{} {e}",
                    envr_core::i18n::tr_key(
                        "cli.doctor.env_override_label",
                        "ENVR_RUNTIME_ROOT：",
                        "ENVR_RUNTIME_ROOT:",
                    )
                );
            }
            println!();
            for (kind, ic, cur) in &report.kinds {
                match cur {
                    Some(v) => println!(
                        "{}",
                        fmt_template(
                            &envr_core::i18n::tr_key(
                                "cli.doctor.line_installed_current",
                                "{kind}：已安装 {count} 个版本，当前 = {current}",
                                "{kind}: {count} installed, current = {current}",
                            ),
                            &[("kind", kind), ("count", &ic.to_string()), ("current", v),],
                        )
                    ),
                    None => println!(
                        "{}",
                        fmt_template(
                            &envr_core::i18n::tr_key(
                                "cli.doctor.line_installed_none",
                                "{kind}：已安装 {count} 个版本，当前 = {none}",
                                "{kind}: {count} installed, current = {none}",
                            ),
                            &[
                                ("kind", kind),
                                ("count", &ic.to_string()),
                                ("none", &none_label),
                            ],
                        )
                    ),
                }
            }
            if !report.issues.is_empty() {
                println!(
                    "\n{}",
                    envr_core::i18n::tr_key("cli.doctor.issues_heading", "问题：", "Issues:",)
                );
                for i in &report.issues {
                    println!("  - {i}");
                }
            }
            if !report.recommendations.is_empty() && !g.quiet {
                println!(
                    "\n{}",
                    envr_core::i18n::tr_key(
                        "cli.doctor.suggestions_heading",
                        "建议：",
                        "Suggestions:",
                    )
                );
                for r in &report.recommendations {
                    println!("  - {r}");
                }
            }
        })
    }
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
