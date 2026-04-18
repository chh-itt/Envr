use crate::CliExit;
use crate::CliUxPolicy;
use crate::cli::GlobalArgs;
use crate::commands::common;
use crate::output;

use envr_error::{EnvrError, EnvrResult};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// Body for [`crate::commands::dispatch`]; errors are finished at the dispatch boundary.
pub(crate) fn clean_inner(
    g: &GlobalArgs,
    kind: Option<String>,
    all: bool,
    older_than: Option<String>,
    newer_than: Option<String>,
    dry_run: bool,
) -> EnvrResult<CliExit> {
    clean_impl(g, kind, all, older_than, newer_than, dry_run)
}

fn clean_impl(
    g: &GlobalArgs,
    kind: Option<String>,
    all: bool,
    older_than: Option<String>,
    newer_than: Option<String>,
    dry_run: bool,
) -> EnvrResult<CliExit> {
    let root = common::effective_runtime_root()?;

    let target = match (
        all,
        kind.as_deref().map(|s| s.trim()).filter(|s| !s.is_empty()),
    ) {
        (true, _) => root.join("cache"),
        (false, None) => root.join("cache"),
        (false, Some(k)) => root.join("cache").join(k.to_ascii_lowercase()),
    };

    let older_raw = older_than
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    let newer_raw = newer_than
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    if newer_raw.is_some() && older_raw.is_none() {
        return Err(EnvrError::Validation(envr_core::i18n::tr_key(
            "cli.err.cache_newer_than_requires_older",
            "`--newer-than` 必须与 `--older-than` 同时使用。",
            "`--newer-than` requires `--older-than`.",
        )));
    }

    if let Some(ref spec) = older_raw {
        let age_old = parse_duration_spec(spec)?;
        let age_new = if let Some(ref ns) = newer_raw {
            Some(parse_duration_spec(ns)?)
        } else {
            None
        };
        if let Some(an) = age_new
            && an <= age_old
        {
            return Err(EnvrError::Validation(envr_core::i18n::tr_key(
                "cli.err.cache_age_window_invalid",
                "`--newer-than` 必须比 `--older-than` 更长（更久以前），例如 `--newer-than 90d --older-than 30d`。",
                "`--newer-than` must be a longer age than `--older-than` (further in the past), e.g. `--newer-than 90d --older-than 30d`.",
            )));
        }

        let now = SystemTime::now();
        let cutoff_old = now.checked_sub(age_old).unwrap_or(SystemTime::UNIX_EPOCH);
        let cutoff_new = age_new.map(|d| now.checked_sub(d).unwrap_or(SystemTime::UNIX_EPOCH));

        if !target.exists() {
            let data = prune_missing_json(&target, spec, newer_raw.as_deref(), dry_run);
            return Ok(output::emit_ok(
                g,
                crate::codes::ok::CACHE_CLEANED,
                data,
                || {
                    if CliUxPolicy::from_global(g).human_text_primary() {
                        println!(
                            "{}",
                            envr_core::i18n::tr_key(
                                "cli.cache.prune_none_missing",
                                "缓存路径不存在，无需清理。",
                                "cache path does not exist; nothing to do.",
                            )
                        );
                    }
                },
            ));
        }

        if target.is_file() {
            return Err(EnvrError::Validation(crate::output::fmt_template(
                &envr_core::i18n::tr_key(
                    "cli.err.cache_path_is_file",
                    "缓存路径是文件，应为目录：{path}",
                    "cache path is a file, expected directory: {path}",
                ),
                &[("path", &target.display().to_string())],
            )));
        }

        let (n, b) = prune_cache_by_age(&target, cutoff_old, cutoff_new, dry_run)?;
        let data = prune_result_json(&target, spec, newer_raw.as_deref(), dry_run, n, b);
        Ok(output::emit_ok(
            g,
            crate::codes::ok::CACHE_CLEANED,
            data,
            || {
                if CliUxPolicy::from_global(g).human_text_primary() {
                    print_prune_human(&target, spec, newer_raw.as_deref(), dry_run, n, b);
                }
            },
        ))
    } else if dry_run {
        if !target.exists() {
            let data = serde_json::json!({
                "removed": target.to_string_lossy(),
                "mode": "remove_tree",
                "dry_run": true,
                "files_would_remove": 0u64,
                "bytes_would_free": 0u64,
            });
            return Ok(output::emit_ok(
                g,
                crate::codes::ok::CACHE_CLEANED,
                data,
                || {
                    if CliUxPolicy::from_global(g).human_text_primary() {
                        println!(
                            "{}",
                            envr_core::i18n::tr_key(
                                "cli.cache.remove_tree_dry_none",
                                "[dry-run] 缓存路径不存在，无需操作。",
                                "[dry-run] cache path does not exist; nothing to do.",
                            )
                        );
                    }
                },
            ));
        }
        if target.is_file() {
            return Err(EnvrError::Validation(crate::output::fmt_template(
                &envr_core::i18n::tr_key(
                    "cli.err.cache_path_is_file",
                    "缓存路径是文件，应为目录：{path}",
                    "cache path is a file, expected directory: {path}",
                ),
                &[("path", &target.display().to_string())],
            )));
        }
        let (files, bytes) = count_tree_stats(&target)?;
        let data = serde_json::json!({
            "removed": target.to_string_lossy(),
            "mode": "remove_tree",
            "dry_run": true,
            "files_would_remove": files,
            "bytes_would_free": bytes,
        });
        Ok(output::emit_ok(
            g,
            crate::codes::ok::CACHE_CLEANED,
            data,
            || {
                if CliUxPolicy::from_global(g).human_text_primary() {
                    println!(
                        "{}",
                        crate::output::fmt_template(
                            &envr_core::i18n::tr_key(
                                "cli.cache.remove_tree_dry",
                                "[dry-run] 将整棵删除缓存目录 {path}（约 {files} 个文件，{bytes} 字节）",
                                "[dry-run] would remove entire cache tree {path} (~{files} file(s), {bytes} byte(s))",
                            ),
                            &[
                                ("path", &target.display().to_string()),
                                ("files", &files.to_string()),
                                ("bytes", &bytes.to_string()),
                            ],
                        )
                    );
                }
            },
        ))
    } else {
        remove_dir_if_exists(&target)?;
        let data = serde_json::json!({
            "removed": target.to_string_lossy(),
            "mode": "remove_tree",
        });
        Ok(output::emit_ok(
            g,
            crate::codes::ok::CACHE_CLEANED,
            data,
            || {
                if CliUxPolicy::from_global(g).human_text_primary() {
                    println!(
                        "{}",
                        crate::output::fmt_template(
                            &envr_core::i18n::tr_key(
                                "cli.cache.removed",
                                "已移除缓存：{path}",
                                "cache removed: {path}",
                            ),
                            &[("path", &target.display().to_string())],
                        )
                    );
                }
            },
        ))
    }
}

fn prune_missing_json(
    target: &Path,
    older_spec: &str,
    newer_spec: Option<&str>,
    dry_run: bool,
) -> serde_json::Value {
    let mut data = serde_json::json!({
        "removed": target.to_string_lossy(),
        "mode": "prune_by_age",
        "older_than": older_spec,
        "newer_than": newer_spec,
    });
    if dry_run {
        data["dry_run"] = serde_json::json!(true);
        data["files_would_remove"] = serde_json::json!(0u64);
        data["bytes_would_free"] = serde_json::json!(0u64);
    } else {
        data["files_removed"] = serde_json::json!(0u64);
        data["bytes_freed"] = serde_json::json!(0u64);
    }
    data
}

fn prune_result_json(
    target: &Path,
    older_spec: &str,
    newer_spec: Option<&str>,
    dry_run: bool,
    files: u64,
    bytes: u64,
) -> serde_json::Value {
    let mut data = serde_json::json!({
        "removed": target.to_string_lossy(),
        "mode": "prune_by_age",
        "older_than": older_spec,
        "newer_than": newer_spec,
    });
    if dry_run {
        data["dry_run"] = serde_json::json!(true);
        data["files_would_remove"] = serde_json::json!(files);
        data["bytes_would_free"] = serde_json::json!(bytes);
    } else {
        data["files_removed"] = serde_json::json!(files);
        data["bytes_freed"] = serde_json::json!(bytes);
    }
    data
}

fn print_prune_human(
    target: &Path,
    older_spec: &str,
    newer_spec: Option<&str>,
    dry_run: bool,
    files: u64,
    bytes: u64,
) {
    if dry_run {
        if let Some(ns) = newer_spec {
            println!(
                "{}",
                crate::output::fmt_template(
                    &envr_core::i18n::tr_key(
                        "cli.cache.prune_dry_window",
                        "[dry-run] 缓存 {path}：将删除 {files} 个文件，约 {bytes} 字节（早于 {older} 且晚于 {newer}）",
                        "[dry-run] cache {path}: would remove {files} file(s), ~{bytes} byte(s) (older than {older} but newer than {newer})",
                    ),
                    &[
                        ("path", &target.display().to_string()),
                        ("files", &files.to_string()),
                        ("bytes", &bytes.to_string()),
                        ("older", older_spec),
                        ("newer", ns),
                    ],
                )
            );
        } else {
            println!(
                "{}",
                crate::output::fmt_template(
                    &envr_core::i18n::tr_key(
                        "cli.cache.prune_dry",
                        "[dry-run] 缓存 {path}：将删除 {files} 个文件，约 {bytes} 字节（早于 {spec}）",
                        "[dry-run] cache {path}: would remove {files} file(s), ~{bytes} byte(s) older than {spec}",
                    ),
                    &[
                        ("path", &target.display().to_string()),
                        ("files", &files.to_string()),
                        ("bytes", &bytes.to_string()),
                        ("spec", older_spec),
                    ],
                )
            );
        }
        return;
    }
    if let Some(ns) = newer_spec {
        println!(
            "{}",
            crate::output::fmt_template(
                &envr_core::i18n::tr_key(
                    "cli.cache.prune_done_window",
                    "已按时间窗清理缓存 {path}：删除 {files} 个文件，释放约 {bytes} 字节（早于 {older} 且晚于 {newer}）",
                    "Pruned cache under {path}: removed {files} file(s), ~{bytes} byte(s) (older than {older} but newer than {newer})",
                ),
                &[
                    ("path", &target.display().to_string()),
                    ("files", &files.to_string()),
                    ("bytes", &bytes.to_string()),
                    ("older", older_spec),
                    ("newer", ns),
                ],
            )
        );
    } else {
        println!(
            "{}",
            crate::output::fmt_template(
                &envr_core::i18n::tr_key(
                    "cli.cache.prune_done",
                    "已按时间清理缓存 {path}：删除 {files} 个文件，释放约 {bytes} 字节（早于 {spec}）",
                    "Pruned cache under {path}: removed {files} file(s), ~{bytes} byte(s) older than {spec}",
                ),
                &[
                    ("path", &target.display().to_string()),
                    ("files", &files.to_string()),
                    ("bytes", &bytes.to_string()),
                    ("spec", older_spec),
                ],
            )
        );
    }
}

/// Parse `<n><unit>` with units `s|m|h|d|w` (ASCII, case-insensitive) plus common long forms (`days`, `hours`, …).
fn parse_duration_spec(raw: &str) -> Result<Duration, EnvrError> {
    let s = raw.trim();
    let pos = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
    if pos == 0 || pos == s.len() {
        return Err(EnvrError::Validation(format!(
            "invalid duration {s:?}: expected e.g. 30d, 24h, 90m, 3600s, 1w (see help)"
        )));
    }
    let (num_s, rest) = s.split_at(pos);
    let num: u64 = num_s
        .parse()
        .map_err(|_| EnvrError::Validation(format!("invalid number in duration {s:?}")))?;
    if num == 0 {
        return Err(EnvrError::Validation(
            "duration must be positive (non-zero)".into(),
        ));
    }
    let unit = rest.trim().to_ascii_lowercase();
    const DAY: u64 = 86_400;
    let secs: u64 = match unit.as_str() {
        "s" | "sec" | "secs" | "second" | "seconds" => num,
        "m" | "min" | "mins" | "minute" | "minutes" => num
            .checked_mul(60)
            .ok_or_else(|| EnvrError::Validation("duration too large".into()))?,
        "h" | "hr" | "hrs" | "hour" | "hours" => num
            .checked_mul(3_600)
            .ok_or_else(|| EnvrError::Validation("duration too large".into()))?,
        "d" | "day" | "days" => num
            .checked_mul(DAY)
            .ok_or_else(|| EnvrError::Validation("duration too large".into()))?,
        "w" | "wk" | "wks" | "week" | "weeks" => num
            .checked_mul(DAY)
            .and_then(|x| x.checked_mul(7))
            .ok_or_else(|| EnvrError::Validation("duration too large".into()))?,
        _ => {
            return Err(EnvrError::Validation(format!(
                "unknown unit in duration {s:?}: use s, m, h, d, or w (e.g. 30d, 1w)"
            )));
        }
    };
    Ok(Duration::from_secs(secs))
}

/// Deletes regular files under `root` whose mtime is older than `cutoff_old` and (if set) newer than `cutoff_new`
/// (`cutoff_new` < mtime < `cutoff_old`). When `dry_run` is false, removes empty subdirectories afterward (never `root`).
fn prune_cache_by_age(
    root: &Path,
    cutoff_old: SystemTime,
    cutoff_new: Option<SystemTime>,
    dry_run: bool,
) -> EnvrResult<(u64, u64)> {
    let mut files_removed = 0u64;
    let mut bytes_freed = 0u64;
    prune_files_recursive(
        root,
        cutoff_old,
        cutoff_new,
        dry_run,
        &mut files_removed,
        &mut bytes_freed,
    )?;
    if !dry_run {
        prune_empty_descendants(root, root)?;
    }
    Ok((files_removed, bytes_freed))
}

fn prune_files_recursive(
    path: &Path,
    cutoff_old: SystemTime,
    cutoff_new: Option<SystemTime>,
    dry_run: bool,
    files_removed: &mut u64,
    bytes_freed: &mut u64,
) -> EnvrResult<()> {
    if path.is_file() {
        let meta = fs::metadata(path).map_err(EnvrError::from)?;
        if let Ok(mtime) = meta.modified()
            && mtime < cutoff_old
            && cutoff_new.map(|cn| mtime > cn).unwrap_or(true)
        {
            let len = meta.len();
            if dry_run {
                *files_removed = files_removed.saturating_add(1);
                *bytes_freed = bytes_freed.saturating_add(len);
            } else {
                fs::remove_file(path).map_err(EnvrError::from)?;
                *files_removed = files_removed.saturating_add(1);
                *bytes_freed = bytes_freed.saturating_add(len);
            }
        }
        return Ok(());
    }
    if path.is_dir() {
        for ent in fs::read_dir(path).map_err(EnvrError::from)? {
            let ent = ent.map_err(EnvrError::from)?;
            prune_files_recursive(
                &ent.path(),
                cutoff_old,
                cutoff_new,
                dry_run,
                files_removed,
                bytes_freed,
            )?;
        }
    }
    Ok(())
}

fn count_tree_stats(path: &Path) -> EnvrResult<(u64, u64)> {
    let mut files = 0u64;
    let mut bytes = 0u64;
    count_tree_recursive(path, &mut files, &mut bytes)?;
    Ok((files, bytes))
}

fn count_tree_recursive(path: &Path, files: &mut u64, bytes: &mut u64) -> EnvrResult<()> {
    if path.is_file() {
        let meta = fs::metadata(path).map_err(EnvrError::from)?;
        *files = files.saturating_add(1);
        *bytes = bytes.saturating_add(meta.len());
        return Ok(());
    }
    if path.is_dir() {
        for ent in fs::read_dir(path).map_err(EnvrError::from)? {
            let ent = ent.map_err(EnvrError::from)?;
            count_tree_recursive(&ent.path(), files, bytes)?;
        }
    }
    Ok(())
}

fn prune_empty_descendants(base: &Path, current: &Path) -> EnvrResult<()> {
    if !current.is_dir() {
        return Ok(());
    }
    for ent in fs::read_dir(current).map_err(EnvrError::from)? {
        let ent = ent.map_err(EnvrError::from)?;
        let p = ent.path();
        if p.is_dir() {
            prune_empty_descendants(base, &p)?;
        }
    }
    if current != base && dir_is_empty(current)? {
        let _ = fs::remove_dir(current);
    }
    Ok(())
}

fn dir_is_empty(path: &Path) -> EnvrResult<bool> {
    Ok(fs::read_dir(path)
        .map_err(EnvrError::from)?
        .next()
        .is_none())
}

/// Body for [`crate::commands::dispatch`]; errors are finished at the dispatch boundary.
pub(crate) fn index_inner(g: &GlobalArgs, sub: crate::cli::CacheIndexCmd) -> EnvrResult<CliExit> {
    match sub {
        crate::cli::CacheIndexCmd::Sync { runtime, all, dir } => {
            index_sync_inner(g, runtime, all, dir)
        }
        crate::cli::CacheIndexCmd::Status { dir } => index_status_inner(g, dir),
    }
}

pub(crate) fn runtime_inner(
    g: &GlobalArgs,
    sub: crate::cli::CacheRuntimeCmd,
) -> EnvrResult<CliExit> {
    match sub {
        crate::cli::CacheRuntimeCmd::Status { runtime, all } => runtime_status_inner(g, runtime, all),
    }
}

#[derive(Clone)]
struct RuntimeCacheStatusRow {
    runtime: String,
    unified_files: usize,
    unified_newest_mtime_unix_secs: Option<u64>,
    unified_major_rows_mtime_unix_secs: Option<u64>,
    unified_full_installable_mtime_unix_secs: Option<u64>,
    unified_children_files: usize,
    provider_files: usize,
    provider_newest_mtime_unix_secs: Option<u64>,
    provider_index_json_mtime_unix_secs: Option<u64>,
    provider_remote_latest_files: usize,
    provider_remote_latest_newest_mtime_unix_secs: Option<u64>,
    unified_ready: bool,
    provider_ready: bool,
    remote_may_paint_empty: bool,
}

fn runtime_status_inner(
    g: &GlobalArgs,
    runtime: Option<String>,
    all: bool,
) -> EnvrResult<CliExit> {
    use envr_domain::runtime::{parse_runtime_kind, runtime_descriptor, runtime_kinds_all};
    use std::time::SystemTime;

    let root = common::effective_runtime_root()?;
    let cache_root = root.join("cache");

    let target = runtime
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());
    if target.is_some() && all {
        return Err(EnvrError::Validation(
            "cannot combine RUNTIME with --all".to_string(),
        ));
    }

    let kinds: Vec<envr_domain::runtime::RuntimeKind> = match target {
        None => runtime_kinds_all().collect(),
        Some(r) => vec![parse_runtime_kind(&r)?],
    };

    let svc = common::runtime_service()?;

    fn newest_file_mtime_secs(dir: &Path) -> (usize, Option<u64>) {
        let mut files = 0usize;
        let mut newest: Option<SystemTime> = None;
        if let Ok(rd) = fs::read_dir(dir) {
            for ent in rd.flatten() {
                let p = ent.path();
                if p.is_file() {
                    files += 1;
                    if let Ok(m) = ent.metadata().and_then(|m| m.modified()) {
                        newest = Some(newest.map(|cur| cur.max(m)).unwrap_or(m));
                    }
                } else if p.is_dir() {
                    let (c_files, c_newest) = newest_file_mtime_secs(&p);
                    files += c_files;
                    if let Some(secs) = c_newest {
                        let t = SystemTime::UNIX_EPOCH + Duration::from_secs(secs);
                        newest = Some(newest.map(|cur| cur.max(t)).unwrap_or(t));
                    }
                }
            }
        }
        let newest_secs = newest
            .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
            .map(|d| d.as_secs());
        (files, newest_secs)
    }

    fn file_mtime_secs(path: &Path) -> Option<u64> {
        let meta = fs::metadata(path).ok()?;
        if !meta.is_file() {
            return None;
        }
        let m = meta.modified().ok()?;
        m.duration_since(SystemTime::UNIX_EPOCH).ok().map(|d| d.as_secs())
    }

    let mut rows: Vec<RuntimeCacheStatusRow> = Vec::new();
    for kind in kinds {
        let key = runtime_descriptor(kind).key.to_string();
        let unified_dir = cache_root.join(&key).join("unified_version_list");
        let provider_dir = cache_root.join(&key);
        let (unified_files, unified_newest) = newest_file_mtime_secs(&unified_dir);
        let (provider_files, provider_newest) = newest_file_mtime_secs(&provider_dir);

        let unified_major_rows_mtime_unix_secs =
            file_mtime_secs(&unified_dir.join("major_rows.json"));
        let unified_full_installable_mtime_unix_secs =
            file_mtime_secs(&unified_dir.join("full_installable_versions.json"));
        let (unified_children_files, _) = newest_file_mtime_secs(&unified_dir.join("children"));

        let provider_index_json_mtime_unix_secs = file_mtime_secs(&provider_dir.join("index.json"));
        let mut provider_remote_latest_files = 0usize;
        let mut provider_remote_latest_newest_mtime_unix_secs: Option<u64> = None;
        if let Ok(rd) = fs::read_dir(&provider_dir) {
            for ent in rd.flatten() {
                let p = ent.path();
                if !p.is_file() {
                    continue;
                }
                let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
                if !name.starts_with("remote_latest_per_major") || !name.ends_with(".json") {
                    continue;
                }
                provider_remote_latest_files += 1;
                if let Some(m) = file_mtime_secs(&p) {
                    provider_remote_latest_newest_mtime_unix_secs = Some(
                        provider_remote_latest_newest_mtime_unix_secs
                            .map(|cur| cur.max(m))
                            .unwrap_or(m),
                    );
                }
            }
        }

        // Best-effort warm: if unified has no snapshot but provider has disk cache, keep as-is.
        // If both are empty, a background refresh would be needed; do not network in status.
        let _ = svc.try_load_full_remote_installable_from_disk(kind);

        rows.push(RuntimeCacheStatusRow {
            runtime: key,
            unified_files,
            unified_newest_mtime_unix_secs: unified_newest,
            unified_major_rows_mtime_unix_secs,
            unified_full_installable_mtime_unix_secs,
            unified_children_files,
            provider_files,
            provider_newest_mtime_unix_secs: provider_newest,
            provider_index_json_mtime_unix_secs,
            provider_remote_latest_files,
            provider_remote_latest_newest_mtime_unix_secs,
            unified_ready: unified_full_installable_mtime_unix_secs.is_some()
                || unified_major_rows_mtime_unix_secs.is_some(),
            provider_ready: provider_index_json_mtime_unix_secs.is_some()
                || provider_remote_latest_files > 0,
            remote_may_paint_empty: unified_files == 0 && provider_remote_latest_files == 0,
        });
    }
    rows.sort_by(|a, b| a.runtime.cmp(&b.runtime));

    let entries_json: Vec<serde_json::Value> = rows
        .iter()
        .map(|r| {
            serde_json::json!({
                "runtime": r.runtime,
                "unified_files": r.unified_files,
                "unified_newest_mtime_unix_secs": r.unified_newest_mtime_unix_secs,
                "unified_major_rows_mtime_unix_secs": r.unified_major_rows_mtime_unix_secs,
                "unified_full_installable_mtime_unix_secs": r.unified_full_installable_mtime_unix_secs,
                "unified_children_files": r.unified_children_files,
                "provider_files": r.provider_files,
                "provider_newest_mtime_unix_secs": r.provider_newest_mtime_unix_secs,
                "provider_index_json_mtime_unix_secs": r.provider_index_json_mtime_unix_secs,
                "provider_remote_latest_files": r.provider_remote_latest_files,
                "provider_remote_latest_newest_mtime_unix_secs": r.provider_remote_latest_newest_mtime_unix_secs,
                "unified_ready": r.unified_ready,
                "provider_ready": r.provider_ready,
                "remote_may_paint_empty": r.remote_may_paint_empty,
            })
        })
        .collect();

    let data = serde_json::json!({
        "runtime_root": root.to_string_lossy(),
        "cache_root": cache_root.to_string_lossy(),
        "entries": entries_json,
    });

    Ok(output::emit_ok(
        g,
        crate::codes::ok::CACHE_RUNTIME_STATUS,
        data,
        || {
            if CliUxPolicy::from_global(g).human_text_primary() {
                println!("{}", cache_root.display());
                for r in &rows {
                    println!(
                        "{}\tunified_files={}\tunified_mtime={}\tmajor_rows_mtime={}\tfull_installable_mtime={}\tchildren_files={}\tprovider_files={}\tprovider_mtime={}\tprovider_index_mtime={}\tremote_latest_files={}\tremote_latest_mtime={}\tunified_ready={}\tprovider_ready={}\tremote_may_paint_empty={}",
                        r.runtime,
                        r.unified_files,
                        r.unified_newest_mtime_unix_secs
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| "-".to_string()),
                        r.unified_major_rows_mtime_unix_secs
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| "-".to_string()),
                        r.unified_full_installable_mtime_unix_secs
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| "-".to_string()),
                        r.unified_children_files,
                        r.provider_files,
                        r.provider_newest_mtime_unix_secs
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| "-".to_string()),
                        r.provider_index_json_mtime_unix_secs
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| "-".to_string()),
                        r.provider_remote_latest_files,
                        r.provider_remote_latest_newest_mtime_unix_secs
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| "-".to_string()),
                        r.unified_ready,
                        r.provider_ready,
                        r.remote_may_paint_empty,
                    );
                }
            }
        },
    ))
}

fn index_cache_dir(dir_override: Option<PathBuf>) -> Result<PathBuf, EnvrError> {
    if let Some(d) = dir_override {
        return Ok(d);
    }
    let platform = envr_platform::paths::current_platform_paths()?;
    Ok(envr_platform::paths::index_cache_dir_from_platform(
        &platform,
    ))
}

fn index_sync_inner(
    g: &GlobalArgs,
    runtime: Option<String>,
    all: bool,
    dir: Option<PathBuf>,
) -> EnvrResult<CliExit> {
    let cache_dir = index_cache_dir(dir)?;

    // Temporarily override index cache dir for this process.
    // This keeps runtime providers consistent without adding new plumbing.
    let _guard = ScopedEnvVar::set("ENVR_INDEX_CACHE_DIR", cache_dir.to_string_lossy().as_ref());

    let service = common::runtime_service()?;

    let target = runtime
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());
    if target.is_some() && all {
        return Err(EnvrError::Validation(
            "cannot combine RUNTIME with --all".to_string(),
        ));
    }

    let kinds = match target {
        None => vec![
            envr_domain::runtime::RuntimeKind::Node,
            envr_domain::runtime::RuntimeKind::Deno,
            envr_domain::runtime::RuntimeKind::Bun,
        ],
        Some(runtime_s) => {
            let kind = envr_domain::runtime::parse_runtime_kind(runtime_s)?;
            let supported = matches!(
                kind,
                envr_domain::runtime::RuntimeKind::Node
                    | envr_domain::runtime::RuntimeKind::Deno
                    | envr_domain::runtime::RuntimeKind::Bun
            );
            if !supported {
                return Err(EnvrError::Validation(format!(
                    "index cache sync supports only node/deno/bun, got: {runtime_s}"
                )));
            }
            vec![kind]
        }
    };

    let mut synced: Vec<String> = Vec::new();
    let mut errors: Vec<String> = Vec::new();
    let use_mock_sync = cli_test_mock_index_sync_enabled();
    for k in kinds {
        if use_mock_sync {
            if let Err(e) = write_mock_index_cache_entry(&cache_dir, k) {
                errors.push(format!("{k:?}: {e}"));
            } else {
                synced.push(format!("{k:?}"));
            }
            continue;
        }
        // Prefer smaller indexes; ensure Node index.json is populated.
        let res: Result<(), envr_error::EnvrError> = (|| {
            if k == envr_domain::runtime::RuntimeKind::Node {
                let _ = service.list_remote_majors(k)?;
                let _ = service.list_remote_latest_per_major(k);
                return Ok(());
            }
            let _ = service.list_remote_latest_per_major(k)?;
            Ok(())
        })();
        if let Err(e) = res {
            errors.push(format!("{k:?}: {e}"));
        } else {
            synced.push(format!("{k:?}"));
        }
    }

    if !errors.is_empty() {
        return Err(EnvrError::Download(format!(
            "index sync failed for some runtimes: {}",
            errors.join("; ")
        )));
    }

    let mut data = serde_json::json!({
        "dir": cache_dir.to_string_lossy(),
        "synced": synced,
    });
    data = output::with_next_steps(
        data,
        vec![(
            "check_index_status",
            envr_core::i18n::tr_key(
                "cli.next_step.cache_index.check_status",
                "可执行 `envr cache index status` 查看各运行时索引文件状态。",
                "Run `envr cache index status` to inspect per-runtime index cache status.",
            ),
        )],
    );
    Ok(output::emit_ok(
        g,
        crate::codes::ok::CACHE_INDEX_SYNCED,
        data,
        || {
            if CliUxPolicy::from_global(g).human_text_primary() {
                println!(
                    "{}",
                    crate::output::fmt_template(
                        &envr_core::i18n::tr_key(
                            "cli.cache.index.synced",
                            "已预缓存远程索引：{dir}",
                            "remote indexes cached: {dir}",
                        ),
                        &[("dir", &cache_dir.display().to_string())],
                    )
                );
            }
        },
    ))
}

fn cli_test_mock_index_sync_enabled() -> bool {
    matches!(
        std::env::var("ENVR_CLI_TEST_MOCK_INDEX_SYNC")
            .ok()
            .map(|v| v.to_ascii_lowercase())
            .as_deref(),
        Some("1" | "true" | "yes")
    )
}

fn write_mock_index_cache_entry(
    dir: &Path,
    kind: envr_domain::runtime::RuntimeKind,
) -> EnvrResult<()> {
    let kind_dir_name = match kind {
        envr_domain::runtime::RuntimeKind::Node => "node",
        envr_domain::runtime::RuntimeKind::Deno => "deno",
        envr_domain::runtime::RuntimeKind::Bun => "bun",
        _ => {
            return Err(EnvrError::Validation(format!(
                "mock index sync unsupported runtime: {kind:?}"
            )));
        }
    };
    let sub = dir.join(kind_dir_name);
    fs::create_dir_all(&sub).map_err(EnvrError::from)?;
    let file = sub.join("index.json");
    fs::write(file, b"{\"mock\":true}\n").map_err(EnvrError::from)?;
    Ok(())
}

fn index_status_inner(g: &GlobalArgs, dir: Option<PathBuf>) -> EnvrResult<CliExit> {
    let cache_dir = index_cache_dir(dir)?;
    let report = build_index_status(cache_dir.as_path());
    let entries_json: Vec<serde_json::Value> = report
        .iter()
        .map(|r| {
            serde_json::json!({
                "runtime": r.runtime,
                "files": r.files,
                "newest_mtime_unix_secs": r.newest_mtime_unix_secs,
            })
        })
        .collect();
    let data = serde_json::json!({
        "dir": cache_dir.to_string_lossy(),
        "entries": entries_json,
    });
    Ok(output::emit_ok(
        g,
        crate::codes::ok::CACHE_INDEX_STATUS,
        data,
        || {
            println!("{}", cache_dir.display());
            for r in report {
                println!(
                    "{}\t{}\t{}",
                    r.runtime,
                    r.files,
                    r.newest_mtime_unix_secs
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| "-".to_string())
                );
            }
        },
    ))
}

#[derive(Clone)]
struct IndexStatusRow {
    runtime: String,
    files: usize,
    newest_mtime_unix_secs: Option<u64>,
}

fn build_index_status(dir: &Path) -> Vec<IndexStatusRow> {
    let runtimes = ["node", "deno", "bun"];
    let mut out = Vec::new();
    for r in runtimes {
        let sub = dir.join(r);
        let mut files = 0usize;
        let mut newest: Option<SystemTime> = None;
        if let Ok(rd) = fs::read_dir(&sub) {
            for ent in rd.flatten() {
                let p = ent.path();
                if p.is_file() {
                    files += 1;
                    if let Ok(m) = ent.metadata().and_then(|m| m.modified()) {
                        newest = Some(newest.map(|cur| cur.max(m)).unwrap_or(m));
                    }
                }
            }
        }
        let newest_secs = newest
            .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
            .map(|d| d.as_secs());
        out.push(IndexStatusRow {
            runtime: r.to_string(),
            files,
            newest_mtime_unix_secs: newest_secs,
        });
    }
    out
}

struct ScopedEnvVar {
    key: &'static str,
    prev: Option<String>,
}

impl ScopedEnvVar {
    fn set(key: &'static str, value: &str) -> Self {
        let prev = std::env::var(key).ok();
        // SAFETY: CLI entry point mutates env during startup; this is local to current thread.
        unsafe {
            std::env::set_var(key, value);
        }
        Self { key, prev }
    }
}

impl Drop for ScopedEnvVar {
    fn drop(&mut self) {
        // SAFETY: single-threaded CLI command execution.
        unsafe {
            match &self.prev {
                Some(v) => std::env::set_var(self.key, v),
                None => std::env::remove_var(self.key),
            }
        }
    }
}

fn remove_dir_if_exists(path: &PathBuf) -> EnvrResult<()> {
    if !path.exists() {
        return Ok(());
    }
    if path.is_file() {
        return Err(EnvrError::Validation(crate::output::fmt_template(
            &envr_core::i18n::tr_key(
                "cli.err.cache_path_is_file",
                "缓存路径是文件，应为目录：{path}",
                "cache path is a file, expected directory: {path}",
            ),
            &[("path", &path.display().to_string())],
        )));
    }
    fs::remove_dir_all(path).map_err(EnvrError::from)?;
    Ok(())
}

#[cfg(test)]
mod parse_duration_tests {
    use super::parse_duration_spec;
    use std::time::Duration;

    #[test]
    fn parses_days_hours_minutes_seconds_weeks() {
        assert_eq!(
            parse_duration_spec("30d").unwrap(),
            Duration::from_secs(30 * 86_400)
        );
        assert_eq!(
            parse_duration_spec("24h").unwrap(),
            Duration::from_secs(24 * 3600)
        );
        assert_eq!(
            parse_duration_spec("90m").unwrap(),
            Duration::from_secs(90 * 60)
        );
        assert_eq!(
            parse_duration_spec("3600s").unwrap(),
            Duration::from_secs(3600)
        );
        assert_eq!(
            parse_duration_spec("1w").unwrap(),
            Duration::from_secs(7 * 86_400)
        );
        assert_eq!(
            parse_duration_spec("2days").unwrap(),
            Duration::from_secs(2 * 86_400)
        );
    }

    #[test]
    fn rejects_zero_and_garbage() {
        assert!(parse_duration_spec("0d").is_err());
        assert!(parse_duration_spec("30").is_err());
        assert!(parse_duration_spec("xyz").is_err());
        assert!(parse_duration_spec("30x").is_err());
    }

    #[test]
    fn prune_window_uses_longer_newer_than_bound() {
        let older = parse_duration_spec("30d").unwrap();
        let newer = parse_duration_spec("90d").unwrap();
        assert!(newer > older, "mtime window needs newer span > older span");
    }
}

#[cfg(test)]
mod duration_proptest {
    use super::parse_duration_spec;
    use proptest::prelude::*;
    use std::time::Duration;

    proptest! {
        #[test]
        fn ndays_parses_to_seconds(n in 1u64..500u64) {
            let spec = format!("{n}d");
            let d = parse_duration_spec(&spec).expect("parse");
            prop_assert_eq!(d, Duration::from_secs(n * 86_400));
        }

        #[test]
        fn nweeks_parses_to_seconds(n in 1u64..52u64) {
            let spec = format!("{n}w");
            let d = parse_duration_spec(&spec).expect("parse");
            prop_assert_eq!(d, Duration::from_secs(n * 7 * 86_400));
        }
    }
}
