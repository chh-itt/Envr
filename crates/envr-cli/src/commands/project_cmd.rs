use crate::CliExit;
use crate::CliPathProfile;
use crate::CliUxPolicy;
use crate::cli::{GlobalArgs, ProjectCmd};
use crate::commands::child_env;
use crate::commands::common;
use crate::output::{self, fmt_template};

use envr_config::env_context::load_settings_cached;
use envr_config::project_config::{
    EnvLockFile, ProjectConfigLocation, RuntimeLockEntry, load_project_lock,
    project_lock_candidates, project_lock_exists, project_lock_is_fresh,
    reset_project_config_load_cache,
};
use envr_core::runtime::service::RuntimeService;
use envr_domain::runtime::{RemoteFilter, RuntimeKind, VersionSpec, parse_runtime_kind};
use envr_error::{EnvrError, EnvrResult};
use envr_resolver::{parse_runtime_pin_spec, runtime_kind_toml_key, upsert_runtime_pin};
use envr_shim_core::{pick_version_home, resolve_version_home};
use serde_json::json;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

fn next_steps_for_project_validate_ok(check_remote: bool) -> Vec<(&'static str, String)> {
    let mut steps = Vec::new();
    if !check_remote {
        steps.push((
            "validate_with_remote_check",
            envr_core::i18n::tr_key(
                "cli.next_step.project_validate.remote_check",
                "可执行 `envr project validate --check-remote` 增加远端可用性校验。",
                "Run `envr project validate --check-remote` for extra remote availability checks.",
            ),
        ));
    }
    steps.push((
        "sync_project_pins",
        envr_core::i18n::tr_key(
            "cli.next_step.project_validate.sync_pins",
            "可执行 `envr project sync --install` 对齐并安装项目 pin。",
            "Run `envr project sync --install` to align and install pinned runtimes.",
        ),
    ));
    steps
}

fn next_steps_for_project_validate_failure() -> Vec<(&'static str, String)> {
    vec![
        (
            "fix_project_config",
            envr_core::i18n::tr_key(
                "cli.next_step.project_validate.fix_config",
                "请修复 `.envr.toml` 中无效版本或运行时键后重试。",
                "Fix invalid runtime keys/versions in `.envr.toml`, then retry.",
            ),
        ),
        (
            "run_project_sync",
            envr_core::i18n::tr_key(
                "cli.next_step.project_validate.run_sync",
                "可执行 `envr project sync --install` 自动安装缺失版本。",
                "Run `envr project sync --install` to install missing versions.",
            ),
        ),
    ]
}

/// Body for [`crate::commands::dispatch`]; errors are finished at the dispatch boundary.
pub(crate) fn run_inner(
    g: &GlobalArgs,
    service: &RuntimeService,
    cmd: ProjectCmd,
) -> EnvrResult<CliExit> {
    match cmd {
        ProjectCmd::Add { spec, path } => add_inner(g, spec, path),
        ProjectCmd::Lock { path, dry_run } => lock_inner(g, path, dry_run),
        ProjectCmd::Sync {
            path,
            install,
            locked,
        } => sync_inner(g, service, path, install, locked),
        ProjectCmd::Validate {
            path,
            check_remote,
            locked,
        } => validate_inner(g, service, path, check_remote, locked),
    }
}

fn add_inner(g: &GlobalArgs, spec: String, path: PathBuf) -> EnvrResult<CliExit> {
    let pin = parse_runtime_pin_spec(&spec)?;
    common::emit_verbose_step(
        g,
        &fmt_template(
            &envr_core::i18n::tr_key(
                "cli.verbose.project.add",
                "[verbose] 正在写入项目 pin：{kind} {version}",
                "[verbose] writing project pin: {kind} {version}",
            ),
            &[
                ("kind", runtime_kind_toml_key(pin.kind)),
                ("version", &pin.version),
            ],
        ),
    );
    let dir = std::fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
    if !dir.is_dir() {
        return Err(EnvrError::Validation(format!(
            "not a directory: {}",
            dir.display()
        )));
    }
    let written = upsert_runtime_pin(&dir, &pin)?;
    reset_project_config_load_cache();
    let kind_s = runtime_kind_toml_key(pin.kind);
    let version = pin.version.clone();
    let data = json!({
        "config_path": written.to_string_lossy(),
        "kind": kind_s,
        "version": version,
    });
    let path_s = written.display().to_string();
    Ok(output::emit_ok(
        g,
        crate::codes::ok::PROJECT_PIN_ADDED,
        data,
        || {
            if CliUxPolicy::from_global(g).human_text_primary() {
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
        },
    ))
}

fn lock_inner(g: &GlobalArgs, path: PathBuf, dry_run: bool) -> EnvrResult<CliExit> {
    let session = CliPathProfile::new(path.clone(), None).load_project()?;
    let Some((cfg, loc)) = session.project.as_ref() else {
        return Err(EnvrError::Validation("no project config found".into()));
    };
    let [lock_path, lock_alt_path] = project_lock_candidates(&loc.dir);
    let mut runtime = Vec::new();
    for (name, rt) in &cfg.runtimes {
        let Some(request) = rt.version.as_deref().map(str::trim).filter(|s| !s.is_empty()) else {
            continue;
        };
        let versions_dir = session
            .ctx
            .runtime_root
            .join("runtimes")
            .join(name)
            .join("versions");
        let resolved = resolve_version_home(&versions_dir, request)?;
        let resolved_version = resolved.resolved_version.unwrap_or_default();
        runtime.push(RuntimeLockEntry {
            name: name.clone(),
            request: request.to_string(),
            resolved: resolved_version,
            source: if resolved.candidate_count > 0 { "resolved".into() } else { "direct".into() },
            candidate_count: resolved.candidate_count,
        });
    }
    let lock_file = EnvLockFile { version: 1, runtime };
    let rendered = toml::to_string_pretty(&lock_file).map_err(|e| {
        EnvrError::with_source(envr_error::ErrorCode::Runtime, "serialize env lock", e)
    })?;
    if !dry_run {
        let lock_content = toml::to_string_pretty(&lock_file).map_err(|e| {
            EnvrError::with_source(envr_error::ErrorCode::Runtime, "serialize env lock", e)
        })?;
        std::fs::write(&lock_path, &lock_content)?;
        if lock_alt_path != lock_path {
            std::fs::write(&lock_alt_path, &lock_content)?;
        }
        reset_project_config_load_cache();
    }
    let data = json!({
        "lock_path": lock_path.to_string_lossy(),
        "lock_path_alt": lock_alt_path.to_string_lossy(),
        "lock_exists": lock_path.is_file() || lock_alt_path.is_file(),
        "lock_version": lock_file.version,
        "dry_run": dry_run,
        "content": rendered,
        "config_dir": loc.dir.to_string_lossy(),
        "project_runtimes": cfg.runtimes.keys().cloned().collect::<Vec<_>>(),
        "compat_asdf_names": cfg.compat.asdf.names.clone(),
    });
    Ok(output::emit_ok(
        g,
        crate::codes::ok::PROJECT_SYNCED,
        data,
        || {
            if CliUxPolicy::from_global(g).human_text_primary() {
                println!(
                    "{}",
                    fmt_template(
                        &envr_core::i18n::tr_key(
                            "cli.project.lock_ok",
                            "已写入 lockfile：{path}",
                            "Wrote lockfile: {path}",
                        ),
                        &[("path", &lock_path.display().to_string())],
                    )
                );
                if lock_alt_path != lock_path {
                    println!(
                        "{}",
                        fmt_template(
                            &envr_core::i18n::tr_key(
                                "cli.project.lock_alt",
                                "兼容 lockfile：{path}",
                                "Compatibility lockfile: {path}",
                            ),
                            &[("path", &lock_alt_path.display().to_string())],
                        )
                    );
                }
                println!(
                    "{}",
                    fmt_template(
                        &envr_core::i18n::tr_key(
                            "cli.project.lock_config_dir",
                            "项目目录：{path}",
                            "Project dir: {path}",
                        ),
                        &[("path", &loc.dir.display().to_string())],
                    )
                );
                if dry_run {
                    println!(
                        "{}",
                        envr_core::i18n::tr_key(
                            "cli.project.lock_dry_run",
                            "dry run：未写入任何文件",
                            "dry run: no files written",
                        )
                    );
                }
            }
        },
    ))
}

fn lock_status_json(
    session: &crate::runtime_session::RuntimeSession,
    locked: bool,
) -> EnvrResult<Option<serde_json::Value>> {
    if !locked {
        return Ok(session.project.as_ref().and_then(|(_, loc)| {
            loc.lock_file.as_ref().map(|p| json!({
                "path": p.to_string_lossy(),
                "version": 1,
                "matched": false,
                "fresh": false,
            }))
        }));
    }

    let Some((_, loc)) = session.project.as_ref() else {
        return Err(EnvrError::Validation("no project config found".into()));
    };

    let lock_result = loc
        .lock_file
        .clone()
        .or_else(|| project_lock_candidates(&session.ctx.working_dir).into_iter().find(|p| p.is_file()));
    let Some(lock_path) = lock_result else {
        return Err(EnvrError::Validation(format!(
            "no lockfile found under {}; run `envr project lock`",
            session.ctx.working_dir.display()
        )));
    };

    let fresh = project_lock_is_fresh(session.project_config(), &lock_path)?;
    if !fresh {
        return Err(EnvrError::Validation(format!(
            "lockfile {} is stale; run `envr project lock`",
            lock_path.display()
        )));
    }

    let lock_entries = match std::fs::read_to_string(&lock_path)
        .ok()
        .and_then(|content| toml::from_str::<EnvLockFile>(&content).ok()) {
        Some(lock) => lock.runtime,
        None => Vec::new(),
    };
    Ok(Some(json!({
        "path": lock_path.to_string_lossy(),
        "matched": true,
        "version": 1,
        "fresh": fresh,
        "entries": lock_entries.iter().map(|entry| json!({
            "name": entry.name,
            "request": entry.request,
            "resolved": entry.resolved,
            "source": entry.source,
            "candidate_count": entry.candidate_count,
        })).collect::<Vec<_>>(),
    })))
}

fn sync_inner(
    g: &GlobalArgs,
    _service: &RuntimeService,
    path: PathBuf,
    install: bool,
    locked: bool,
) -> EnvrResult<CliExit> {
    let session = CliPathProfile::new(path.clone(), None).load_project()?;
    let ctx = &session.ctx;
    let mut lock_status = lock_status_json(&session, locked)?;
    let pending = child_env::plan_missing_pinned_runtimes_for_run(ctx, session.project_config())?;
    if pending.is_empty() {
        let data = json!({
            "missing": Vec::<serde_json::Value>::new(),
            "installed": Vec::<serde_json::Value>::new(),
            "lock_status": lock_status,
        });
        return Ok(output::emit_ok(
            g,
            crate::codes::ok::PROJECT_SYNCED,
            data,
            || {
                if CliUxPolicy::from_global(g).human_text_primary() {
                    println!(
                        "{}",
                        envr_core::i18n::tr_key(
                            "cli.project.sync_nothing",
                            "所有已 pin 的运行时均已可用。",
                            "All pinned runtimes are already available.",
                        )
                    );
                    if let Some(path) = lock_status
                        .as_ref()
                        .and_then(|v| v.get("path"))
                        .and_then(|v| v.as_str())
                    {
                        println!("lockfile: {path}");
                    }
                }
            },
        ));
    }
    if !install {
        let msg = envr_core::i18n::tr_key(
            "cli.project.sync_need_install",
            "以下 pin 尚未安装；使用 `envr project sync --install` 安装。",
            "Pinned runtimes are missing; run `envr project sync --install` to install them.",
        );
        let rows: Vec<_> = pending
            .iter()
            .map(|(k, v)| json!({ "kind": k, "version_spec": v }))
            .collect();
        let data = json!({
            "missing": rows,
            "installed": [],
            "lock_status": lock_status,
            "config_dir": session.ctx.working_dir.to_string_lossy(),
            "project_runtimes": session
                .project_config()
                .map(|cfg| cfg.runtimes.keys().cloned().collect::<Vec<_>>())
                .unwrap_or_default(),
        });
        let code = output::emit_failure_envelope(
            g,
            crate::codes::err::PROJECT_SYNC_PENDING,
            &msg,
            data,
            &[],
            1,
        );
        if CliUxPolicy::from_global(g).human_text_primary() {
            for (k, v) in &pending {
                eprintln!("envr:   - {k} {v}");
            }
            println!(
                "{}",
                fmt_template(
                    &envr_core::i18n::tr_key(
                        "cli.project.sync_pending_hint",
                        "提示：可运行 `envr project sync --install` 自动安装这些 pin。",
                        "Tip: run `envr project sync --install` to install these pins automatically.",
                    ),
                    &[],
                )
            );
        }
        return Ok(code);
    }

    for (lang, spec) in &pending {
        let kind = parse_runtime_kind(lang)?;
        common::emit_verbose_step(
            g,
            &fmt_template(
                &envr_core::i18n::tr_key(
                    "cli.verbose.project.sync.install",
                    "[verbose] 正在安装项目 pin：{kind} {version}",
                    "[verbose] installing pinned runtime: {kind} {version}",
                ),
                &[("kind", lang.as_str()), ("version", spec.as_str())],
            ),
        );
        if kind == RuntimeKind::Rust {
            return Err(EnvrError::Validation(
                "rust pin sync is not automated here; use `envr rust` / rustup".into(),
            ));
        }
    }
    let installed_pairs = install_pending_parallel(pending.clone())?;
    let installed: Vec<_> = installed_pairs
        .into_iter()
        .map(|(lang, version)| json!({ "kind": lang, "version": version }))
        .collect();

    let data = json!({
        "missing_before": pending
            .iter()
            .map(|(a, b)| json!({ "kind": a, "version_spec": b }))
            .collect::<Vec<_>>(),
        "installed": installed,
        "lock_status": lock_status,
        "project_runtimes": session
            .project_config()
            .map(|cfg| cfg.runtimes.keys().cloned().collect::<Vec<_>>())
            .unwrap_or_default(),
    });
    Ok(output::emit_ok(
        g,
        crate::codes::ok::PROJECT_SYNCED,
        data,
        || {
            if CliUxPolicy::from_global(g).human_text_primary() {
                println!(
                    "{}",
                    envr_core::i18n::tr_key(
                        "cli.project.sync_done",
                        "已安装缺失的 pin。",
                        "Installed missing pinned runtimes.",
                    )
                );
            }
        },
    ))
}

fn install_pending_parallel(pending: Vec<(String, String)>) -> EnvrResult<Vec<(String, String)>> {
    let max_workers = read_max_download_workers().max(1) as usize;
    let queue = Arc::new(Mutex::new(pending));
    let results = Arc::new(Mutex::new(Vec::<(String, String)>::new()));
    let first_err = Arc::new(Mutex::new(None::<EnvrError>));
    let workers = max_workers.min(queue.lock().map(|q| q.len()).unwrap_or(1).max(1));
    let mut joins = Vec::new();
    for _ in 0..workers {
        let queue = Arc::clone(&queue);
        let results = Arc::clone(&results);
        let first_err = Arc::clone(&first_err);
        joins.push(std::thread::spawn(move || {
            loop {
                if first_err.lock().map(|e| e.is_some()).unwrap_or(false) {
                    break;
                }
                let next = queue.lock().ok().and_then(|mut q| q.pop());
                let Some((lang, spec)) = next else {
                    break;
                };
                let kind = match parse_runtime_kind(&lang) {
                    Ok(v) => v,
                    Err(e) => {
                        if let Ok(mut slot) = first_err.lock() {
                            *slot = Some(e);
                        }
                        break;
                    }
                };
                let service = match common::runtime_service() {
                    Ok(v) => v,
                    Err(e) => {
                        if let Ok(mut slot) = first_err.lock() {
                            *slot = Some(e);
                        }
                        break;
                    }
                };
                let request = envr_domain::runtime::InstallRequest {
                    spec: VersionSpec(spec),
                    progress_downloaded: None,
                    progress_total: None,
                    cancel: None,
                };
                match service
                    .installer_port(kind)
                    .and_then(|installer| installer.install(&request))
                {
                    Ok(v) => {
                        if let Ok(mut out) = results.lock() {
                            out.push((lang, v.0));
                        }
                    }
                    Err(e) => {
                        if let Ok(mut slot) = first_err.lock() {
                            *slot = Some(e);
                        }
                        break;
                    }
                }
            }
        }));
    }
    for join in joins {
        let _ = join.join();
    }
    if let Ok(mut slot) = first_err.lock()
        && let Some(err) = slot.take()
    {
        return Err(err);
    }
    Ok(results.lock().map(|v| v.clone()).unwrap_or_default())
}

fn read_max_download_workers() -> u32 {
    load_settings_cached()
        .map(|s| s.download.max_concurrent_downloads)
        .unwrap_or(4)
}

fn validate_inner(
    g: &GlobalArgs,
    service: &RuntimeService,
    path: PathBuf,
    check_remote: bool,
    locked: bool,
) -> EnvrResult<CliExit> {
    let session = CliPathProfile::new(path.clone(), None).load_project()?;
    let Some((cfg, loc)) = session.project.as_ref() else {
        return Err(EnvrError::Validation(fmt_template(
            &envr_core::i18n::tr_key(
                "cli.err.no_project_config",
                "自 {path} 向上未找到 `.envr.toml` 或 `.envr.local.toml`",
                "no `.envr.toml` or `.envr.local.toml` found searching upward from {path}",
            ),
            &[("path", &path.display().to_string())],
        )));
    };
    let lock_status = if locked {
        Some(lock_status_json(&session, true)?)
    } else {
        lock_status_json(&session, false)?
    };

    let runtime_root = common::session_runtime_root()?;

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
            match service.index_port(kind).and_then(|index| {
                index.list_remote_installable(&RemoteFilter {
                    prefix: Some(prefix),
                    ..Default::default()
                })
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
        let mut data = json!({
            "config_dir": loc.dir.to_string_lossy(),
            "issues": issues,
            "remote_warnings": remote_warnings,
            "project_runtimes": cfg.runtimes.keys().cloned().collect::<Vec<_>>(),
            "compat_asdf_names": cfg.compat.asdf.names.clone(),
            "lock_status": lock_status,
        });
        data = output::with_next_steps(data, next_steps_for_project_validate_failure());
        let code = output::emit_failure_envelope(
            g,
            crate::codes::err::PROJECT_VALIDATE_FAILED,
            &msg,
            data,
            &[],
            1,
        );
        if CliUxPolicy::from_global(g).human_text_primary() {
            for p in &issues {
                eprintln!("envr:   - {p}");
            }
            if !remote_warnings.is_empty() {
                println!(
                    "{}",
                    envr_core::i18n::tr_key(
                        "cli.project.validate_remote_warn",
                        "远端校验有警告，建议检查网络或远端索引。",
                        "Remote validation reported warnings; check network or remote index availability.",
                    )
                );
            }
        }
        return Ok(code);
    }

    let mut data = json!({
        "config_dir": loc.dir.to_string_lossy(),
        "issues": issues,
        "remote_warnings": remote_warnings,
        "check_remote": check_remote,
        "project_runtimes": cfg.runtimes.keys().cloned().collect::<Vec<_>>(),
        "compat_asdf_names": cfg.compat.asdf.names.clone(),
        "lock_status": lock_status,
    });
    data = output::with_next_steps(data, next_steps_for_project_validate_ok(check_remote));
    let root_s = loc.dir.display().to_string();
    Ok(output::emit_ok(
        g,
        crate::codes::ok::PROJECT_VALIDATED,
        data,
        || {
            if CliUxPolicy::from_global(g).human_text_primary() {
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
        },
    ))
}
