use crate::commands::common::{self, kind_label};
use crate::commands::doctor::{
    DoctorFinding, DoctorReport, PathAnalysis, PathShadowingInfo, all_kinds,
};
use crate::commands::doctor_presenter;
use crate::output::fmt_template;

use envr_core::runtime::service::RuntimeService;
use envr_domain::runtime::RuntimeVersion;
use envr_error::EnvrError;
use std::path::{Path, PathBuf};

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
            if !doctor_presenter::path_contains_dir(&shims) {
                let (shell, cmd) = doctor_presenter::shell_add_path_command(&shims);
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
    for kind in all_kinds() {
        let label = kind_label(kind).to_string();
        let index = match service.index_port(kind) {
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
        let installed = match index.list_installed() {
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
        let current = match index.current() {
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

pub(crate) fn current_is_broken(
    current: &Option<RuntimeVersion>,
    installed: &[RuntimeVersion],
) -> bool {
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

fn detect_tool_path_shadowing(
    shims: &Path,
    tool: &str,
    names: &[&str],
) -> Option<PathShadowingInfo> {
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

pub(crate) fn runtime_root_writable(root: &Path) -> bool {
    let probe = root.join(".envr-doctor-probe");
    match std::fs::write(&probe, b"ok") {
        Ok(()) => {
            let _ = std::fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}
