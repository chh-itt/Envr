use crate::cli::GlobalArgs;
use crate::commands::common::{self, kind_label};
use crate::commands::shim_cmd;
use crate::output::{self, fmt_template};

use std::io::{self, Write};

use envr_core::runtime::service::RuntimeService;
use envr_domain::runtime::{RuntimeKind, RuntimeVersion};
use envr_error::EnvrError;
use serde_json::{Value, json};
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
            "path_shadowing": path_shadowing,
            "path_conflicts": path_conflicts,
            "path_analysis": path_analysis,
            "findings": findings,
            "shims_dir_writable": self.shims_dir_writable,
        })
    }
}

fn build_findings_parts(
    issues: &[String],
    warnings: &[String],
    notes: &[String],
    shims_dir_writable: Option<bool>,
) -> Vec<DoctorFinding> {
    let mut out = Vec::new();
    for (i, s) in issues.iter().enumerate() {
        out.push(DoctorFinding {
            severity: "critical".into(),
            code: format!("issue_{i}"),
            message: s.clone(),
        });
    }
    for (i, s) in warnings.iter().enumerate() {
        out.push(DoctorFinding {
            severity: "warning".into(),
            code: format!("warning_{i}"),
            message: s.clone(),
        });
    }
    for (i, s) in notes.iter().enumerate() {
        out.push(DoctorFinding {
            severity: "info".into(),
            code: format!("note_{i}"),
            message: s.clone(),
        });
    }
    if let Some(false) = shims_dir_writable {
        out.push(DoctorFinding {
            severity: "critical".into(),
            code: "shims_dir_not_writable".into(),
            message: "envr shims directory is not writable; shim sync may fail".into(),
        });
    }
    out
}

pub(crate) fn build_doctor_report(service: &RuntimeService) -> Result<DoctorReport, EnvrError> {
    let root = common::effective_runtime_root()?;

    let env_override = std::env::var("ENVR_RUNTIME_ROOT")
        .ok()
        .filter(|s| !s.is_empty());

    let mut issues = Vec::new();
    let mut warnings = Vec::new();
    let mut notes = Vec::new();
    let mut path_shadowing = None;
    let mut path_conflicts = Vec::new();
    let mut path_analysis = None;
    let mut shims_dir_writable = None;

    if !root.exists() {
        issues.push(envr_core::i18n::tr_key(
            "cli.doctor.issue.root_missing",
            "运行时数据根目录不存在",
            "runtime data root does not exist",
        ));
        warnings.push(fmt_template(
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
        warnings.push(envr_core::i18n::tr_key(
            "cli.doctor.rec.root_permissions",
            "请修复目录权限或更换 ENVR_RUNTIME_ROOT",
            "fix directory permissions or choose another ENVR_RUNTIME_ROOT",
        ));
    }

    let shims = root.join("shims");
    if shims.is_dir() {
        shims_dir_writable = Some(shims_writable_probe(&shims));
        path_analysis = Some(analyze_path_precedence(&shims));
        let empty = std::fs::read_dir(&shims)
            .map(|mut d| d.next().is_none())
            .unwrap_or(true);
        if empty {
            notes.push(envr_core::i18n::tr_key(
                "cli.doctor.rec.shims_empty",
                "`shims` 目录为空；安装运行时后请将 `shims` 加入 PATH，或在集成环境中刷新 shims",
                "`shims` directory is empty; after installing runtimes, add `shims` to PATH or refresh shims when integrated",
            ));
        } else {
            notes.push(fmt_template(
                &envr_core::i18n::tr_key(
                    "cli.doctor.rec.shims_path",
                    "请确保 `{path}` 在 PATH 中且优先于其他同名工具",
                    "ensure `{path}` is on your PATH ahead of other tool copies",
                ),
                &[("path", &shims.display().to_string())],
            ));
            path_conflicts = all_path_shadowings(&shims);
            path_shadowing = path_conflicts.first().cloned();
            for info in &path_conflicts {
                warnings.push(fmt_template(
                    &envr_core::i18n::tr_key(
                        "cli.doctor.warn.path_shadow_tool",
                        "PATH 上先于 envr shims 解析到 `{tool}`（`{exe}`）；请调整 PATH 顺序或移除冲突安装",
                        "`{tool}` resolves from `{exe}` before envr shims; reorder PATH or remove the conflicting install",
                    ),
                    &[("tool", &info.tool), ("exe", &info.executable)],
                ));
            }
            if !path_contains_dir(&shims) {
                let (shell, cmd) = shell_add_path_command(&shims);
                warnings.push(fmt_template(
                    &envr_core::i18n::tr_key(
                        "cli.doctor.rec.shims_path_add_cmd",
                        "检测到 shims 目录未在 PATH（shell={shell}）。可执行：{cmd}",
                        "shims directory is not on PATH (shell={shell}). Run: {cmd}",
                    ),
                    &[("shell", shell), ("cmd", &cmd)],
                ));
            }
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

        if current_is_broken(&current, &installed) {
            let ver = current.as_ref().map(|v| v.0.as_str()).unwrap_or("");
            warnings.push(fmt_template(
                &envr_core::i18n::tr_key(
                    "cli.doctor.warn.broken_current",
                    "{kind}：current 指向 `{version}`，但该版本未安装或已缺失",
                    "{kind}: current points at `{version}`, which is not among installed versions",
                ),
                &[("kind", kind_label(kind)), ("version", ver)],
            ));
        }

        if !installed.is_empty() && current.is_none() {
            warnings.push(fmt_template(
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

    let findings = build_findings_parts(&issues, &warnings, &notes, shims_dir_writable);

    Ok(DoctorReport {
        root,
        env_override,
        issues,
        warnings,
        notes,
        path_shadowing,
        path_conflicts,
        path_analysis,
        findings,
        shims_dir_writable,
        kinds,
    })
}

fn current_is_broken(current: &Option<RuntimeVersion>, installed: &[RuntimeVersion]) -> bool {
    current
        .as_ref()
        .is_some_and(|c| !installed.iter().any(|i| i.0 == c.0))
}

fn node_path_candidates() -> &'static [&'static str] {
    if cfg!(windows) {
        &["node.exe", "node.cmd", "node"]
    } else {
        &["node"]
    }
}

fn python_path_candidates() -> &'static [&'static str] {
    if cfg!(windows) {
        &["python.exe", "python3.exe", "py.exe"]
    } else {
        &["python3", "python"]
    }
}

fn java_path_candidates() -> &'static [&'static str] {
    if cfg!(windows) {
        &["java.exe", "java"]
    } else {
        &["java"]
    }
}

fn go_path_candidates() -> &'static [&'static str] {
    if cfg!(windows) {
        &["go.exe", "go"]
    } else {
        &["go"]
    }
}

fn php_path_candidates() -> &'static [&'static str] {
    if cfg!(windows) {
        &["php.exe", "php"]
    } else {
        &["php"]
    }
}

fn deno_path_candidates() -> &'static [&'static str] {
    if cfg!(windows) {
        &["deno.exe", "deno"]
    } else {
        &["deno"]
    }
}

fn bun_path_candidates() -> &'static [&'static str] {
    if cfg!(windows) {
        &["bun.exe", "bun"]
    } else {
        &["bun"]
    }
}

fn shims_writable_probe(shims: &Path) -> bool {
    let probe = shims.join(".envr-doctor-write-probe");
    match std::fs::write(&probe, b"ok") {
        Ok(()) => {
            let _ = std::fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

fn analyze_path_precedence(shims: &Path) -> PathAnalysis {
    let shims_canon = std::fs::canonicalize(shims).unwrap_or_else(|_| shims.to_path_buf());
    let dirs: Vec<String> = std::env::var_os("PATH")
        .map(|p| {
            std::env::split_paths(&p)
                .map(|x| x.display().to_string())
                .collect()
        })
        .unwrap_or_default();
    let idx = dirs.iter().position(|d| {
        let p = Path::new(d);
        std::fs::canonicalize(p).ok().as_ref() == Some(&shims_canon)
    });
    PathAnalysis {
        path_directory_order: dirs,
        shims_path_index: idx,
        shims_directory: shims_canon.display().to_string(),
    }
}

fn first_executable_on_path(names: &[&str]) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        if dir.as_os_str().is_empty() {
            continue;
        }
        for name in names {
            let p = dir.join(name);
            if p.is_file() {
                return Some(std::fs::canonicalize(&p).unwrap_or(p));
            }
        }
    }
    None
}

fn detect_tool_path_shadowing(shims: &Path, tool: &str, names: &[&str]) -> Option<PathShadowingInfo> {
    let shims_canon = std::fs::canonicalize(shims).ok()?;
    let exe = first_executable_on_path(names)?;
    let parent = exe.parent()?;
    let parent_canon = std::fs::canonicalize(parent).unwrap_or_else(|_| parent.to_path_buf());
    if parent_canon == shims_canon {
        return None;
    }
    Some(PathShadowingInfo {
        tool: tool.to_string(),
        executable: exe.display().to_string(),
        first_path_directory: parent_canon.display().to_string(),
        shims_directory: shims_canon.display().to_string(),
    })
}

fn all_path_shadowings(shims: &Path) -> Vec<PathShadowingInfo> {
    let checks: &[(&str, &[&str])] = &[
        ("node", node_path_candidates()),
        ("python", python_path_candidates()),
        ("java", java_path_candidates()),
        ("go", go_path_candidates()),
        ("php", php_path_candidates()),
        ("deno", deno_path_candidates()),
        ("bun", bun_path_candidates()),
    ];
    let mut out = Vec::new();
    for (tool, names) in checks {
        if let Some(info) = detect_tool_path_shadowing(shims, tool, names) {
            out.push(info);
        }
    }
    out
}

/// Compare version-like labels for picking a reasonable "latest" `current` in `--fix`.
fn cmp_version_labels(a: &str, b: &str) -> std::cmp::Ordering {
    fn tokens(s: &str) -> Vec<&str> {
        s.split(|c: char| !c.is_ascii_alphanumeric())
            .filter(|t| !t.is_empty())
            .collect()
    }
    let ta = tokens(a);
    let tb = tokens(b);
    let n = ta.len().max(tb.len());
    for i in 0..n {
        let va = ta.get(i).copied().unwrap_or("");
        let vb = tb.get(i).copied().unwrap_or("");
        let ord = match (va.parse::<u64>(), vb.parse::<u64>()) {
            (Ok(na), Ok(nb)) => na.cmp(&nb),
            _ => va.cmp(vb),
        };
        if ord != std::cmp::Ordering::Equal {
            return ord;
        }
    }
    std::cmp::Ordering::Equal
}

fn pick_latest_installed(installed: &[RuntimeVersion]) -> Option<RuntimeVersion> {
    installed
        .iter()
        .max_by(|x, y| cmp_version_labels(&x.0, &y.0))
        .cloned()
}

fn apply_doctor_fixes(g: &GlobalArgs, service: &RuntimeService, report: &DoctorReport) -> Vec<String> {
    let mut applied = Vec::new();
    let shims = report.root.join("shims");
    let empty_shims = shims.is_dir()
        && std::fs::read_dir(&shims)
            .map(|mut d| d.next().is_none())
            .unwrap_or(true);

    if empty_shims && report.root.exists() && runtime_root_writable(&report.root) {
        match shim_cmd::sync_core_shims_strict(g) {
            Ok(kinds) => {
                applied.push(fmt_template(
                    &envr_core::i18n::tr_key(
                        "cli.doctor.fix.shims_ok",
                        "已刷新核心 shims：{kinds}",
                        "refreshed core shims: {kinds}",
                    ),
                    &[("kinds", &kinds.join(", "))],
                ));
            }
            Err(e) => {
                applied.push(fmt_template(
                    &envr_core::i18n::tr_key(
                        "cli.doctor.fix.shims_err",
                        "刷新 shims 失败：{detail}",
                        "failed to refresh shims: {detail}",
                    ),
                    &[("detail", &e.to_string())],
                ));
            }
        }
    }

    for kind in ALL_KINDS {
        let Ok(installed) = service.list_installed(kind) else {
            continue;
        };
        let Ok(current) = service.current(kind) else {
            continue;
        };
        if installed.is_empty() {
            continue;
        }

        let was_broken = current_is_broken(&current, &installed);
        let need_set = current.is_none() || was_broken;
        if !need_set {
            continue;
        }
        let Some(best) = pick_latest_installed(&installed) else {
            continue;
        };
        if let Err(e) = service.set_current(kind, &best) {
            applied.push(fmt_template(
                &envr_core::i18n::tr_key(
                    "cli.doctor.fix.current_err",
                    "{kind}：无法设置 current：{detail}",
                    "{kind}: could not set current: {detail}",
                ),
                &[
                    ("kind", kind_label(kind)),
                    ("detail", &e.to_string()),
                ],
            ));
            continue;
        }
        let tmpl = if was_broken {
            envr_core::i18n::tr_key(
                "cli.doctor.fix.broken_current_ok",
                "{kind}：current 已从不存在的版本重定向到 {version}",
                "{kind}: repointed current from a missing version to {version}",
            )
        } else {
            envr_core::i18n::tr_key(
                "cli.doctor.fix.current_ok",
                "{kind}：已将 current 设为 {version}",
                "{kind}: set current to {version}",
            )
        };
        applied.push(fmt_template(
            &tmpl,
            &[
                ("kind", kind_label(kind)),
                ("version", &best.0),
            ],
        ));
    }

    applied
}

fn shims_path_needs_attention(report: &DoctorReport) -> Option<PathBuf> {
    let shims = report.root.join("shims");
    if !shims.is_dir() {
        return None;
    }
    let empty = std::fs::read_dir(&shims)
        .map(|mut d| d.next().is_none())
        .unwrap_or(true);
    if empty {
        return None;
    }
    if path_contains_dir(&shims) {
        return None;
    }
    Some(shims)
}

fn escape_ps_single_quoted(s: &str) -> String {
    s.replace('\'', "''")
}

fn powershell_append_user_path_snippet(shims: &Path) -> String {
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
    let Some(shims) = shims_path_needs_attention(report) else {
        return;
    };
    if let Some(obj) = data.as_object_mut() {
        obj.insert(
            "path_fix_suggestions".into(),
            path_fix_suggestions_value(&shims),
        );
    }
}

fn doctor_use_color(g: &GlobalArgs) -> bool {
    output::use_terminal_styles(g) && !g.porcelain
}

fn doctor_style_line(g: &GlobalArgs, tone: u8, line: &str) -> String {
    if !doctor_use_color(g) {
        return line.to_string();
    }
    const RESET: &str = "\x1b[0m";
    const RED: &str = "\x1b[31m";
    const YELLOW: &str = "\x1b[33m";
    const DIM: &str = "\x1b[2m";
    match tone {
        1 => format!("{RED}{line}{RESET}"),
        2 => format!("{YELLOW}{line}{RESET}"),
        3 => format!("{DIM}{line}{RESET}"),
        _ => line.to_string(),
    }
}

#[cfg(windows)]
fn run_powershell_user_path_snippet(shims: &Path) -> Result<(), String> {
    use std::process::Command;
    let script = powershell_append_user_path_snippet(shims);
    let status = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .status()
        .map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("powershell exited with {status}"))
    }
}

fn doctor_path_followup(
    g: &GlobalArgs,
    report: &DoctorReport,
    fix_path: bool,
    fix_path_apply: bool,
) {
    if output::wants_porcelain(g) {
        return;
    }
    let Some(shims) = shims_path_needs_attention(report) else {
        if fix_path_apply && !g.quiet {
            println!(
                "{}",
                envr_core::i18n::tr_key(
                    "cli.doctor.path_fix.apply_noop",
                    "无需应用 PATH 修复（shims 已在 PATH 或 shims 目录不可用）。",
                    "Nothing to apply for PATH (shims already on PATH or shims dir unavailable).",
                )
            );
        }
        return;
    };
    if g.quiet {
        return;
    }

    if fix_path {
        println!();
        println!(
            "{}",
            doctor_style_line(
                g,
                2,
                &envr_core::i18n::tr_key(
                    "cli.doctor.path_fix.heading",
                    "永久加入 PATH（请自行审核后复制执行）：",
                    "Add shims to PATH permanently (review carefully, then copy/paste):",
                ),
            )
        );
        println!(
            "{}",
            doctor_style_line(
                g,
                2,
                &envr_core::i18n::tr_key(
                    "cli.doctor.path_fix.warn",
                    "错误修改 PATH 可能影响登录与程序查找；避免盲目使用 setx 或直接改注册表。",
                    "Incorrect PATH edits can break sessions. Avoid blind `setx` / registry edits.",
                ),
            )
        );
        #[cfg(windows)]
        {
            println!(
                "{}",
                envr_core::i18n::tr_key(
                    "cli.doctor.path_fix.ps_user",
                    "PowerShell（当前用户，若 PATH 中尚无 shims 则追加）：",
                    "PowerShell (User scope; append shims if missing):",
                )
            );
            println!("{}", powershell_append_user_path_snippet(&shims));
        }
        #[cfg(not(windows))]
        {
            let p = shims.display().to_string();
            println!(
                "{}",
                envr_core::i18n::tr_key(
                    "cli.doctor.path_fix.posix_profile",
                    "在 ~/.profile 或 ~/.bashrc 中加入一行（示例）：",
                    "Append one line to `~/.profile` or `~/.bashrc` (example):",
                )
            );
            println!("export PATH=\"{p}:$PATH\"");
        }

        if fix_path_apply {
            #[cfg(windows)]
            {
                use std::io::IsTerminal;
                let is_tty = io::stdin().is_terminal() && io::stdout().is_terminal();
                if is_tty {
                    let prompt = envr_core::i18n::tr_key(
                        "cli.doctor.path_fix.apply_prompt",
                        "是否立即执行上述 PowerShell 以写入当前用户 PATH？[y/N] ",
                        "Run that PowerShell now to update User PATH? [y/N] ",
                    );
                    print!("{prompt}");
                    let _ = io::stdout().flush();
                    let mut line = String::new();
                    if io::stdin().read_line(&mut line).is_ok() {
                        let yes =
                            matches!(line.trim().to_ascii_lowercase().as_str(), "y" | "yes");
                        if yes {
                            match run_powershell_user_path_snippet(&shims) {
                                Ok(()) => println!(
                                    "{}",
                                    doctor_style_line(
                                        g,
                                        2,
                                        &envr_core::i18n::tr_key(
                                            "cli.doctor.path_fix.apply_ok",
                                            "已更新用户 PATH；请打开新终端以生效。",
                                            "Updated User PATH; open a new terminal to pick it up.",
                                        ),
                                    )
                                ),
                                Err(e) => println!(
                                    "{}",
                                    doctor_style_line(
                                        g,
                                        1,
                                        &fmt_template(
                                            &envr_core::i18n::tr_key(
                                                "cli.doctor.path_fix.apply_err",
                                                "写入用户 PATH 失败：{detail}",
                                                "Failed to update User PATH: {detail}",
                                            ),
                                            &[("detail", &e)],
                                        ),
                                    )
                                ),
                            }
                        }
                    }
                } else {
                    println!(
                        "{}",
                        envr_core::i18n::tr_key(
                            "cli.doctor.path_fix.apply_need_tty",
                            "非交互终端：跳过自动写入 PATH；请手动复制执行上述命令。",
                            "Not a TTY: skipped automatic PATH update; copy/paste the command above.",
                        )
                    );
                }
            }
            #[cfg(not(windows))]
            {
                println!(
                    "{}",
                    envr_core::i18n::tr_key(
                        "cli.doctor.path_fix.apply_windows_only",
                        "`--fix-path-apply` 仅在 Windows 上可用。",
                        "`--fix-path-apply` is only available on Windows.",
                    )
                );
            }
        }
    }

    use std::io::IsTerminal;
    let (_, session_cmd) = shell_add_path_command(&shims);
    let is_tty = io::stdin().is_terminal() && io::stdout().is_terminal();

    println!();
    if is_tty {
        let prompt = envr_core::i18n::tr_key(
            "cli.doctor.path_session.prompt",
            "是否打印仅作用于当前终端会话的 PATH 命令？[y/N] ",
            "Print a command to prepend shims to PATH for this session only? [y/N] ",
        );
        print!("{prompt}");
        let _ = io::stdout().flush();
        let mut line = String::new();
        if io::stdin().read_line(&mut line).is_ok() {
            let yes = matches!(line.trim().to_ascii_lowercase().as_str(), "y" | "yes");
            if yes {
                println!(
                    "\n{}",
                    envr_core::i18n::tr_key(
                        "cli.doctor.path_session.copy",
                        "在当前 shell 中执行：",
                        "Run in this shell:",
                    )
                );
                println!("{session_cmd}");
            }
        }
    } else {
        println!(
            "{}",
            envr_core::i18n::tr_key(
                "cli.doctor.path_session.non_tty",
                "当前会话 PATH（复制执行，仅本次终端有效）：",
                "Session PATH (copy & run; applies to this terminal only):",
            )
        );
        println!("{session_cmd}");
    }
}

fn print_doctor_human_sections(g: &GlobalArgs, report: &DoctorReport, fixes_for_text: &[String]) {
    let none_label = envr_core::i18n::tr_key("cli.common.none", "（无）", "(none)");
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
                    &[("kind", kind), ("count", &ic.to_string()), ("current", v)],
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
            doctor_style_line(
                g,
                1,
                &envr_core::i18n::tr_key("cli.doctor.issues_heading", "问题：", "Issues:",),
            )
        );
        for i in &report.issues {
            println!("  - {}", doctor_style_line(g, 1, i));
        }
    }
    if !report.warnings.is_empty() && !g.quiet {
        println!(
            "\n{}",
            doctor_style_line(
                g,
                2,
                &envr_core::i18n::tr_key("cli.doctor.warnings_heading", "警告：", "Warnings:",),
            )
        );
        for w in &report.warnings {
            println!("  - {}", doctor_style_line(g, 2, w));
        }
    }
    if !report.notes.is_empty() && !g.quiet {
        println!(
            "\n{}",
            doctor_style_line(
                g,
                3,
                &envr_core::i18n::tr_key("cli.doctor.notes_heading", "提示：", "Notes:",),
            )
        );
        for n in &report.notes {
            println!("  - {}", doctor_style_line(g, 3, n));
        }
    }
    if !fixes_for_text.is_empty() && !g.quiet {
        println!(
            "\n{}",
            envr_core::i18n::tr_key(
                "cli.doctor.fixes_heading",
                "已执行的修复：",
                "Fixes applied:",
            )
        );
        for f in fixes_for_text {
            println!("  - {f}");
        }
    }
}

pub fn run(
    g: &GlobalArgs,
    service: &RuntimeService,
    fix: bool,
    fix_path: bool,
    fix_path_apply: bool,
) -> i32 {
    let mut report = match build_doctor_report(service) {
        Ok(r) => r,
        Err(e) => return common::print_envr_error(g, e),
    };

    let fixes_applied = if fix {
        apply_doctor_fixes(g, service, &report)
    } else {
        Vec::new()
    };

    if fix {
        report = match build_doctor_report(service) {
            Ok(r) => r,
            Err(e) => return common::print_envr_error(g, e),
        };
    }

    let mut data = report.to_json();
    if fix {
        if let Some(obj) = data.as_object_mut() {
            obj.insert(
                "fixes_applied".into(),
                serde_json::to_value(&fixes_applied).unwrap_or_else(|_| json!([])),
            );
        }
    }
    merge_path_fix_json(&mut data, fix_path, &report);
    let ok = report.ok();
    let fixes_for_text = fixes_applied.clone();

    if ok {
        output::emit_doctor(g, ok, "doctor_ok", None, data, || {
            print_doctor_human_sections(g, &report, &fixes_for_text);
            doctor_path_followup(g, &report, fix_path, fix_path_apply);
        })
    } else {
        let fail_msg = envr_core::i18n::tr_key(
            "cli.doctor.json_fail_message",
            "环境检查发现问题",
            "environment checks found problems",
        );
        output::emit_doctor(g, ok, &fail_msg, Some("doctor_issues"), data, || {
            print_doctor_human_sections(g, &report, &fixes_for_text);
            doctor_path_followup(g, &report, fix_path, fix_path_apply);
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

fn path_contains_dir(dir: &Path) -> bool {
    let Ok(path_var) = std::env::var("PATH") else {
        return false;
    };
    let want = std::fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
    std::env::split_paths(&path_var).any(|p| std::fs::canonicalize(&p).unwrap_or(p) == want)
}

fn shell_add_path_command(shims: &Path) -> (&'static str, String) {
    let shell = detect_shell_kind();
    let p = shims.display().to_string();
    match shell {
        "powershell" => (
            "powershell",
            format!("$env:PATH = \"{p};\" + $env:PATH"),
        ),
        "cmd" => ("cmd", format!("set PATH={p};%PATH%")),
        _ => ("posix", format!("export PATH=\"{p}:$PATH\"")),
    }
}

fn detect_shell_kind() -> &'static str {
    if std::env::var("PSModulePath").is_ok() {
        return "powershell";
    }
    if let Ok(comspec) = std::env::var("ComSpec") {
        if comspec.to_ascii_lowercase().contains("cmd.exe") {
            return "cmd";
        }
    }
    "posix"
}
