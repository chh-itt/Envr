use crate::cli::{GlobalArgs, OutputFormat, ProjectCmd};
use crate::commands::child_env;
use crate::commands::cli_install_progress;
use crate::commands::common;
use crate::output::{self, fmt_template};

use envr_config::project_config::{
    load_project_config_profile, reset_project_config_load_cache,
};
use envr_core::runtime::service::RuntimeService;
use envr_domain::runtime::{
    RemoteFilter, RuntimeKind, VersionSpec, parse_runtime_kind,
};
use envr_error::EnvrError;
use envr_resolver::{parse_runtime_pin_spec, runtime_kind_toml_key, upsert_runtime_pin};
use envr_shim_core::pick_version_home;
use serde_json::json;
use std::path::PathBuf;

pub fn run(g: &GlobalArgs, service: &RuntimeService, cmd: ProjectCmd) -> i32 {
    match cmd {
        ProjectCmd::Add { spec, path } => add(g, spec, path),
        ProjectCmd::Sync { path, install } => sync(g, service, path, install),
        ProjectCmd::Validate { path, check_remote } => validate(g, service, path, check_remote),
    }
}

fn add(g: &GlobalArgs, spec: String, path: PathBuf) -> i32 {
    let pin = match parse_runtime_pin_spec(&spec) {
        Ok(p) => p,
        Err(e) => return common::print_envr_error(g, e),
    };
    let dir = std::fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
    if !dir.is_dir() {
        return common::print_envr_error(
            g,
            EnvrError::Validation(format!("not a directory: {}", dir.display())),
        );
    }
    let written = match upsert_runtime_pin(&dir, &pin) {
        Ok(p) => p,
        Err(e) => return common::print_envr_error(g, e),
    };
    reset_project_config_load_cache();
    let kind_s = runtime_kind_toml_key(pin.kind);
    let version = pin.version.clone();
    let data = json!({
        "config_path": written.to_string_lossy(),
        "kind": kind_s,
        "version": version,
    });
    let path_s = written.display().to_string();
    output::emit_ok(g, "project_pin_added", data, || {
        if !g.quiet {
            println!(
                "{}",
                fmt_template(
                    &envr_core::i18n::tr_key(
                        "cli.project.add_ok",
                        "已写入 {path}：{kind} = {version}",
                        "wrote {path}: {kind} = {version}",
                    ),
                    &[
                        ("path", &path_s),
                        ("kind", kind_s),
                        ("version", &pin.version),
                    ],
                )
            );
        }
    })
}

fn sync(g: &GlobalArgs, service: &RuntimeService, path: PathBuf, install: bool) -> i32 {
    let ctx = match common::shim_context_for(path, None) {
        Ok(c) => c,
        Err(e) => return common::print_envr_error(g, e),
    };
    let pending = match child_env::plan_missing_pinned_runtimes_for_run(&ctx) {
        Ok(p) => p,
        Err(e) => return common::print_envr_error(g, e),
    };
    if pending.is_empty() {
        let data = json!({
            "missing": Vec::<serde_json::Value>::new(),
            "installed": Vec::<serde_json::Value>::new(),
        });
        return output::emit_ok(g, "project_synced", data, || {
            if !g.quiet {
                println!(
                    "{}",
                    envr_core::i18n::tr_key(
                        "cli.project.sync_nothing",
                        "所有已 pin 的运行时均已可用。",
                        "All pinned runtimes are already available.",
                    )
                );
            }
        });
    }
    if !install {
        let msg = envr_core::i18n::tr_key(
            "cli.project.sync_need_install",
            "以下 pin 尚未安装；使用 `envr project sync --install` 安装。",
            "Pinned runtimes are missing; run `envr project sync --install` to install them.",
        );
        match g.output_format.unwrap_or(OutputFormat::Text) {
            OutputFormat::Json => {
                let rows: Vec<_> = pending
                    .iter()
                    .map(|(k, v)| json!({ "kind": k, "version_spec": v }))
                    .collect();
                let data = json!({ "missing": rows, "installed": [] });
                output::write_envelope(false, Some("project_sync_pending"), &msg, data, &[]);
            }
            OutputFormat::Text => {
                output::print_error_text("project_sync_pending", &msg);
                for (k, v) in &pending {
                    eprintln!("envr:   - {k} {v}");
                }
            }
        }
        return 1;
    }

    let mut installed = Vec::new();
    for (lang, spec) in &pending {
        let kind = match parse_runtime_kind(lang) {
            Ok(k) => k,
            Err(e) => return common::print_envr_error(g, e),
        };
        if kind == RuntimeKind::Rust {
            return common::print_envr_error(
                g,
                EnvrError::Validation(
                    "rust pin sync is not automated here; use `envr rust` / rustup".into(),
                ),
            );
        }
        let headline = fmt_template(
            &envr_core::i18n::tr_key(
                "cli.project.sync_installing",
                "正在安装项目 pin：{kind} {version}…",
                "Installing pinned runtime {kind} {version}…",
            ),
            &[("kind", lang.as_str()), ("version", spec.as_str())],
        );
        let use_prog = cli_install_progress::wants_cli_download_progress(g);
        let (request, guard) =
            cli_install_progress::install_request_with_progress(g, VersionSpec(spec.clone()), headline.clone());
        if !use_prog
            && !g.quiet
            && matches!(
                g.output_format.unwrap_or(OutputFormat::Text),
                OutputFormat::Text
            )
        {
            eprintln!("{headline}");
        }
        let version = match service.install(kind, &request) {
            Ok(v) => v,
            Err(e) => {
                guard.finish();
                return common::print_envr_error(g, e);
            }
        };
        guard.finish();
        installed.push(json!({ "kind": lang, "version": version.0 }));
    }

    let data = json!({
        "missing_before": pending
            .iter()
            .map(|(a, b)| json!({ "kind": a, "version_spec": b }))
            .collect::<Vec<_>>(),
        "installed": installed,
    });
    output::emit_ok(g, "project_synced", data, || {
        if !g.quiet {
            println!(
                "{}",
                envr_core::i18n::tr_key(
                    "cli.project.sync_done",
                    "已安装缺失的 pin。",
                    "Installed missing pinned runtimes.",
                )
            );
        }
    })
}

fn validate(g: &GlobalArgs, service: &RuntimeService, path: PathBuf, check_remote: bool) -> i32 {
    let runtime_root = match common::session_runtime_root() {
        Ok(p) => p,
        Err(e) => return common::print_envr_error(g, e),
    };
    let loaded = match load_project_config_profile(&path, None) {
        Ok(l) => l,
        Err(e) => return common::print_envr_error(g, e),
    };
    let Some((cfg, loc)) = loaded else {
        return common::print_envr_error(
            g,
            EnvrError::Validation(fmt_template(
                &envr_core::i18n::tr_key(
                    "cli.err.no_project_config",
                    "自 {path} 向上未找到 `.envr.toml` 或 `.envr.local.toml`",
                    "no `.envr.toml` or `.envr.local.toml` found searching upward from {path}",
                ),
                &[("path", &path.display().to_string())],
            )),
        );
    };

    let mut issues = Vec::new();
    let mut remote_warnings = Vec::new();

    for (key, rt) in &cfg.runtimes {
        if parse_runtime_kind(key).is_err() {
            issues.push(fmt_template(
                &envr_core::i18n::tr_key(
                    "cli.err.unknown_runtime_key",
                    "未知的运行时键 `{key}`（应为 node、python 或 java）",
                    "unknown runtime key `{key}` (expected node, python, or java)",
                ),
                &[("key", key.as_str())],
            ));
            continue;
        }
        let Some(spec) = rt
            .version
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        else {
            continue;
        };
        let vd = runtime_root.join("runtimes").join(key).join("versions");
        if let Err(e) = pick_version_home(&vd, spec) {
            issues.push(format!("{key}: {e}"));
        }

        if check_remote {
            let kind = match parse_runtime_kind(key) {
                Ok(k) => k,
                Err(_) => continue,
            };
            if kind == RuntimeKind::Rust {
                remote_warnings.push(format!(
                    "{key}: remote validation skipped (use rustup for rust)"
                ));
                continue;
            }
            let prefix = spec.chars().take(32).collect::<String>();
            match service.list_remote(kind, &RemoteFilter {
                prefix: Some(prefix),
            }) {
                Ok(vers) if vers.is_empty() => {
                    remote_warnings.push(format!(
                        "{key}@{spec}: remote index returned no rows (offline or empty cache?)"
                    ));
                }
                Ok(vers) => {
                    let hit = vers.iter().any(|v| {
                        v.0 == spec
                            || v.0.starts_with(spec)
                            || spec.starts_with(&v.0)
                            || v.0.contains(spec)
                    });
                    if !hit {
                        remote_warnings.push(format!(
                            "{key}@{spec}: no matching entry in first remote page ({} versions)",
                            vers.len()
                        ));
                    }
                }
                Err(e) => remote_warnings.push(format!("{key}: remote: {e}")),
            }
        }
    }

    if !issues.is_empty() {
        let msg = envr_core::i18n::tr_key(
            "cli.project.validate_fail",
            "项目校验失败",
            "project validation failed",
        );
        match g.output_format.unwrap_or(OutputFormat::Text) {
            OutputFormat::Json => {
                let data = json!({
                    "config_dir": loc.dir.to_string_lossy(),
                    "issues": issues,
                    "remote_warnings": remote_warnings,
                });
                output::write_envelope(false, Some("project_validate_failed"), &msg, data, &[]);
            }
            OutputFormat::Text => {
                output::print_error_text("project_validate_failed", &msg);
                for p in &issues {
                    eprintln!("envr:   - {p}");
                }
            }
        }
        return 1;
    }

    let data = json!({
        "config_dir": loc.dir.to_string_lossy(),
        "issues": issues,
        "remote_warnings": remote_warnings,
        "check_remote": check_remote,
    });
    let root_s = loc.dir.display().to_string();
    output::emit_ok(g, "project_validated", data, || {
        if !g.quiet {
            println!(
                "{}",
                fmt_template(
                    &envr_core::i18n::tr_key(
                        "cli.project.validate_ok",
                        "项目配置校验通过（根 {path}）",
                        "project validation ok (root {path})",
                    ),
                    &[("path", &root_s)],
                )
            );
            for w in &remote_warnings {
                eprintln!("envr: warning: {w}");
            }
        }
    })
}
