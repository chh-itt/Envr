use crate::CliExit;
use crate::cli::GlobalArgs;
use crate::commands::doctor_analyzer;
use crate::commands::doctor_fixer;
use crate::commands::doctor_presenter;
use crate::output;

use envr_core::runtime::service::RuntimeService;
use envr_domain::runtime::{RuntimeKind, runtime_kinds_all};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

pub(crate) fn all_kinds() -> impl Iterator<Item = RuntimeKind> {
    runtime_kinds_all()
}

/// When a tool on PATH resolves outside envr shims (machine-readable detail for JSON).
#[derive(Debug, Clone)]
pub(crate) struct PathShadowingInfo {
    pub tool: String,
    pub executable: String,
    pub first_path_directory: String,
    pub shims_directory: String,
}

/// PATH directory order vs shims placement (for monitoring / IDE).
#[derive(Debug, Clone)]
pub(crate) struct PathAnalysis {
    pub path_directory_order: Vec<String>,
    pub shims_path_index: Option<usize>,
    pub shims_directory: String,
}

/// Severity-classified finding for dashboards (`--format json`).
#[derive(Debug, Clone)]
pub(crate) struct DoctorFinding {
    pub severity: String,
    pub code: String,
    pub message: String,
}

/// Same payload as `envr doctor` JSON `data` field (for `diagnostics export` and tests).
#[derive(Debug, Clone)]
pub(crate) struct DoctorReport {
    pub root: PathBuf,
    pub env_override: Option<String>,
    /// Hard failures (exit `doctor_issues` when non-empty).
    pub issues: Vec<String>,
    /// Actionable problems that do not alone fail the command.
    pub warnings: Vec<String>,
    /// Contextual hints.
    pub notes: Vec<String>,
    /// First PATH conflict (legacy field; same as `path_conflicts.get(0)` when non-empty).
    pub path_shadowing: Option<PathShadowingInfo>,
    /// All detected conflicts (same shape as `path_shadowing`).
    pub path_conflicts: Vec<PathShadowingInfo>,
    pub path_analysis: Option<PathAnalysis>,
    pub findings: Vec<DoctorFinding>,
    /// `None` if `shims` does not exist; else probe result.
    pub shims_dir_writable: Option<bool>,
    /// `(kind_label, installed_count, current_version)`
    pub kinds: Vec<(String, usize, Option<String>)>,
}

impl DoctorReport {
    pub fn ok(&self) -> bool {
        self.issues.is_empty()
    }

    fn merge_recommendations(warnings: &[String], notes: &[String]) -> Vec<String> {
        warnings.iter().chain(notes.iter()).cloned().collect()
    }

    pub fn to_json(&self) -> Value {
        let kinds_json: Vec<_> = self
            .kinds
            .iter()
            .map(|(k, n, cur)| {
                serde_json::json!({
                    "kind": k,
                    "installed_count": n,
                    "current_version": cur,
                })
            })
            .collect();

        let rec = Self::merge_recommendations(&self.warnings, &self.notes);
        let path_shadowing = match &self.path_shadowing {
            None => Value::Null,
            Some(p) => json!({
                "tool": p.tool,
                "executable": p.executable,
                "first_path_directory": p.first_path_directory,
                "shims_directory": p.shims_directory,
            }),
        };
        let path_conflicts: Vec<Value> = self
            .path_conflicts
            .iter()
            .map(|p| {
                json!({
                    "tool": p.tool,
                    "executable": p.executable,
                    "first_path_directory": p.first_path_directory,
                    "shims_directory": p.shims_directory,
                })
            })
            .collect();
        let path_analysis = self
            .path_analysis
            .as_ref()
            .map(|a| {
                json!({
                    "path_directory_order": a.path_directory_order,
                    "shims_path_index": a.shims_path_index,
                    "shims_directory": a.shims_directory,
                })
            })
            .unwrap_or(Value::Null);
        let findings: Vec<Value> = self
            .findings
            .iter()
            .map(|f| {
                json!({
                    "severity": f.severity,
                    "code": f.code,
                    "message": f.message,
                })
            })
            .collect();

        serde_json::json!({
            "runtime_root": self.root.to_string_lossy(),
            "envr_runtime_root_env": self.env_override,
            "kinds": kinds_json,
            "issues": self.issues,
            "warnings": self.warnings,
            "notes": self.notes,
            "recommendations": rec,
            "next_steps": next_steps_for_doctor(self),
            "onboarding_checklist": onboarding_checklist_lines(),
            "path_shadowing": path_shadowing,
            "path_conflicts": path_conflicts,
            "path_analysis": path_analysis,
            "findings": findings,
            "shims_dir_writable": self.shims_dir_writable,
        })
    }
}

pub(crate) fn onboarding_checklist_lines() -> Vec<String> {
    vec![
        envr_core::i18n::tr_key(
            "cli.doctor.onboarding.1",
            "在新目录中运行 `envr doctor`，确认运行时根目录、shims 与 PATH。",
            "Run `envr doctor` in a fresh checkout to verify the runtime root, shims, and PATH.",
        ),
        envr_core::i18n::tr_key(
            "cli.doctor.onboarding.2",
            "使用 `envr init` 或手写 `.envr.toml`，并用 `envr project sync` / `envr install` 对齐固定版本。",
            "Add a project config with `envr init` or edit `.envr.toml`, then align installs via `envr project sync` / `envr install`.",
        ),
        envr_core::i18n::tr_key(
            "cli.doctor.onboarding.3",
            "用 `envr use <种类> <版本>` 设置全局默认（`current`），需要时执行 `envr shim sync`。",
            "Set global defaults with `envr use <kind> <version>` (`current`); run `envr shim sync` when integrating shims.",
        ),
        envr_core::i18n::tr_key(
            "cli.doctor.onboarding.4",
            "用 `envr status` 查看当前目录解析到的运行时来源（项目 / 全局 / 系统 PATH）。",
            "Use `envr status` to see which runtimes resolve for the current directory (project pin / global / system PATH).",
        ),
    ]
}

#[cfg(windows)]
fn escape_ps_single_quoted(s: &str) -> String {
    s.replace('\'', "''")
}

#[cfg(windows)]
pub(crate) fn powershell_append_user_path_snippet(shims: &Path) -> String {
    let esc = escape_ps_single_quoted(&shims.display().to_string());
    format!(
        "$shims = '{esc}'; $u = [Environment]::GetEnvironmentVariable('Path','User'); if ($null -eq $u) {{ $u = '' }}; if ($u -notlike \"*$shims*\") {{ [Environment]::SetEnvironmentVariable('Path', \"$u;$shims\", 'User') }}"
    )
}

fn path_fix_suggestions_value(shims: &Path) -> Value {
    let shims_s = shims.display().to_string();
    let session = json!({
        "posix": format!("export PATH=\"{shims_s}:$PATH\""),
        "powershell": format!("$env:PATH = \"{shims_s};\" + $env:PATH"),
        "cmd": format!("set PATH={shims_s};%PATH%"),
    });
    let mut persistent = serde_json::Map::new();
    persistent.insert(
        "posix_profile_snippet".into(),
        json!(format!("export PATH=\"{shims_s}:$PATH\"")),
    );
    #[cfg(windows)]
    persistent.insert(
        "windows_powershell_user".into(),
        json!(powershell_append_user_path_snippet(shims)),
    );
    json!({
        "shims_dir": shims_s,
        "session_commands": session,
        "persistent_user": Value::Object(persistent),
        "cautions": [
            "Review every command before running; broken PATH can lock you out of tools or sessions.",
            "On Windows, `setx` can truncate PATH; prefer User-scope PowerShell or System Properties.",
        ],
    })
}

fn merge_path_fix_json(data: &mut Value, fix_path: bool, report: &DoctorReport) {
    if !fix_path {
        return;
    }
    let Some(shims) = doctor_presenter::shims_path_needs_attention(report) else {
        return;
    };
    if let Some(obj) = data.as_object_mut() {
        obj.insert(
            "path_fix_suggestions".into(),
            path_fix_suggestions_value(&shims),
        );
    }
}

fn next_steps_for_doctor(report: &DoctorReport) -> Vec<(&'static str, String)> {
    let mut steps: Vec<(&'static str, String)> = Vec::new();
    if report.ok() {
        steps.push((
            "verify_project_status",
            envr_core::i18n::tr_key(
                "cli.next_step.doctor.verify_project_status",
                "可执行 `envr status` 验证当前目录运行时解析来源。",
                "Run `envr status` to verify runtime resolution source in current directory.",
            ),
        ));
    } else {
        steps.push((
            "run_doctor_fix",
            envr_core::i18n::tr_key(
                "cli.next_step.doctor.run_fix",
                "可执行 `envr doctor --fix` 自动修复可修复问题。",
                "Run `envr doctor --fix` to auto-fix recoverable issues.",
            ),
        ));
        steps.push((
            "show_path_fix_suggestions",
            envr_core::i18n::tr_key(
                "cli.next_step.doctor.show_path_fix",
                "可执行 `envr doctor --fix-path` 查看 PATH 修复建议。",
                "Run `envr doctor --fix-path` to view PATH repair suggestions.",
            ),
        ));
    }
    steps
}

/// Body for [`crate::commands::dispatch`]; errors are finished at the dispatch boundary.
pub(crate) fn run_inner(
    g: &GlobalArgs,
    service: &RuntimeService,
    fix: bool,
    fix_path: bool,
    fix_path_apply: bool,
) -> envr_error::EnvrResult<CliExit> {
    let mut report = doctor_analyzer::build_doctor_report(service)?;

    let fixes_applied = if fix {
        doctor_fixer::apply_doctor_fixes(g, service, &report)
    } else {
        Vec::new()
    };

    if fix {
        report = doctor_analyzer::build_doctor_report(service)?;
    }

    let mut data = report.to_json();
    if fix && let Some(obj) = data.as_object_mut() {
        obj.insert(
            "fixes_applied".into(),
            serde_json::to_value(&fixes_applied).unwrap_or_else(|_| json!([])),
        );
    }
    data = output::with_next_steps(data, next_steps_for_doctor(&report));
    merge_path_fix_json(&mut data, fix_path, &report);
    let ok = report.ok();
    let fixes_for_text = fixes_applied.clone();

    if ok {
        let ok_msg = envr_core::i18n::tr_key(
            "cli.ok.doctor_ok",
            "环境检查通过",
            "environment checks passed",
        );
        Ok(output::emit_doctor(
            g,
            ok,
            crate::codes::ok::DOCTOR_OK,
            &ok_msg,
            data,
            || {
                doctor_presenter::print_doctor_human_sections(g, &report, &fixes_for_text);
                doctor_presenter::doctor_path_followup(g, &report, fix_path, fix_path_apply);
            },
        ))
    } else {
        let fail_msg = envr_core::i18n::tr_key(
            "cli.doctor.json_fail_message",
            "环境检查发现问题",
            "environment checks found problems",
        );
        Ok(output::emit_doctor(
            g,
            ok,
            crate::codes::ok::DOCTOR_ISSUES,
            &fail_msg,
            data,
            || {
                doctor_presenter::print_doctor_human_sections(g, &report, &fixes_for_text);
                doctor_presenter::doctor_path_followup(g, &report, fix_path, fix_path_apply);
            },
        ))
    }
}
