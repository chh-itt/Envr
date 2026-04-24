use crate::CliUxPolicy;
use crate::CommandOutcome;
use crate::cli::GlobalArgs;
use crate::command_outcome::CliExit;
use crate::runtime_session::CliRuntimeSession;

use envr_config::project_config::{ProjectConfig, RustEnforceMode};
use envr_config::env_context::load_settings_cached;
use envr_config::settings::resolve_runtime_root;
use envr_core::runtime::service::RuntimeService;
use envr_domain::runtime::{RuntimeKind, runtime_descriptor};
use envr_error::{EnvrError, EnvrResult};
use envr_shim_core::ShimContext;
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, SystemTime};

/// Resolve the effective runtime root for this process (same rules as [`resolve_runtime_root`]:
/// CLI `--runtime-root` override, then `ENVR_RUNTIME_ROOT`, then `settings.toml` `paths.runtime_root`,
/// then the platform default; `settings.toml` is re-read when its mtime changes).
pub fn session_runtime_root() -> EnvrResult<PathBuf> {
    effective_runtime_root()
}

/// [`ShimContext`] for CLI commands: cached `runtime_root`, merged `profile` (`--profile` wins over `ENVR_PROFILE`).
pub fn shim_context_for(path: PathBuf, profile_cli: Option<String>) -> EnvrResult<ShimContext> {
    let runtime_root = session_runtime_root()?;
    let working_dir = std::fs::canonicalize(&path).unwrap_or(path);
    let profile = profile_cli
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| {
            std::env::var("ENVR_PROFILE")
                .ok()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        });
    Ok(ShimContext::with_runtime_root(
        runtime_root,
        working_dir,
        profile,
    ))
}

pub fn kind_label(kind: RuntimeKind) -> &'static str {
    runtime_descriptor(kind).key
}

/// [`RuntimeService`] for the **process default** runtime root (see [`CliRuntimeSession::connect`]).
/// For an explicit root (e.g. bundle apply target), use [`RuntimeService::with_runtime_root`].
pub fn runtime_service() -> Result<RuntimeService, EnvrError> {
    maybe_prune_artifact_cache_on_start();
    Ok(CliRuntimeSession::connect()?.into_service())
}

/// Run `f` with a resolved [`RuntimeService`]; connection and handler errors become [`CommandOutcome::Err`].
/// Handler success uses [`CommandOutcome::from_result`] (same exit-code + metrics rules as dispatch).
/// The process exit path is always [`CommandOutcome::finish`].
pub fn with_runtime_service<F>(f: F) -> CommandOutcome
where
    F: FnOnce(&RuntimeService) -> EnvrResult<CliExit>,
{
    let result = (|| {
        let session = CliRuntimeSession::connect()?;
        f(&session)
    })();
    CommandOutcome::from_result(result)
}

/// Data directory for envr runtimes (`ENVR_RUNTIME_ROOT`, then `settings.toml`, then platform default).
pub(crate) fn effective_runtime_root() -> Result<std::path::PathBuf, EnvrError> {
    resolve_runtime_root()
}

pub fn print_envr_error(g: &GlobalArgs, err: EnvrError) -> i32 {
    crate::output::emit_envr_error(g, err)
}

pub fn missing_positional(g: &GlobalArgs, cmd: &str, example: &str) -> CliExit {
    crate::output::emit_validation(g, cmd, example)
}

pub fn emit_child_process_outcome(g: &GlobalArgs, data: Value, exit: i32) -> CliExit {
    if exit == 0 {
        crate::output::emit_ok(g, crate::codes::ok::CHILD_COMPLETED, data, || {})
    } else {
        let msg = crate::output::fmt_template(
            &envr_core::i18n::tr_key(
                "cli.child.exit_nonzero",
                "子进程退出，代码 {exit}",
                "child process exited with code {exit}",
            ),
            &[("exit", &exit.to_string())],
        );
        crate::output::emit_failure_envelope(
            g,
            crate::codes::err::CHILD_EXIT,
            &msg,
            data,
            &[],
            exit,
        )
    }
}

pub fn emit_verbose_lines(g: &GlobalArgs, text_out: bool, lines: &[String], i18n_key: &str) {
    if CliUxPolicy::from_global(g).quiet || !text_out {
        return;
    }
    for line in lines {
        let msg = crate::output::fmt_template(
            &envr_core::i18n::tr_key(i18n_key, "Using {detail}", "Using {detail}"),
            &[("detail", line)],
        );
        eprintln!("envr: {msg}");
    }
}

/// Emit one verbose progress line for mutating commands (`--verbose`).
pub fn emit_verbose_step(g: &GlobalArgs, detail: &str) {
    let p = CliUxPolicy::from_global(g);
    if p.verbose_stderr(g.verbose) {
        eprintln!("envr: {detail}");
    }
}

pub fn build_child_command(
    executable: &str,
    args: &[String],
    env_map: &HashMap<String, String>,
    working_dir: &Path,
) -> Command {
    let mut child = Command::new(executable);
    child.args(args);
    child.env_clear();
    for (k, v) in env_map {
        child.env(k, v);
    }
    child.current_dir(working_dir);
    child
}

fn parse_rust_channel_from_toolchain(toolchain: &str) -> Option<String> {
    let t = toolchain.trim();
    if t.is_empty() {
        return None;
    }
    let first = t.split_whitespace().next().unwrap_or("").trim();
    let chan = first
        .split('-')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    match chan.as_str() {
        "stable" | "beta" | "nightly" => Some(chan),
        _ => None,
    }
}

fn rustc_version_from_output(out: &str) -> Option<String> {
    let s = out.trim();
    let mut it = s.split_whitespace();
    let _ = it.next()?; // "rustc"
    let v = it.next()?.trim();
    if v.is_empty() {
        None
    } else {
        Some(v.to_string())
    }
}

fn maybe_prune_artifact_cache_on_start() {
    let Ok(settings) = load_settings_cached() else {
        return;
    };
    // Best-effort: configure global download bandwidth cap for this CLI process.
    let _ = envr_download::set_global_download_limit(Some(settings.download.max_bytes_per_sec));
    if !settings.behavior.cache_auto_prune_on_start {
        return;
    }
    let root = resolve_runtime_root().ok();
    let Some(root) = root else {
        return;
    };
    let cache_dir = root.join("cache");
    let ttl_days = settings.behavior.cache_artifact_ttl_days.max(1) as u64;
    let cutoff = SystemTime::now()
        .checked_sub(Duration::from_secs(ttl_days * 86_400))
        .unwrap_or(SystemTime::UNIX_EPOCH);
    let _ = prune_dir_by_mtime(&cache_dir, cutoff);
}

fn prune_dir_by_mtime(path: &Path, cutoff: SystemTime) -> EnvrResult<()> {
    if !path.is_dir() {
        return Ok(());
    }
    for ent in fs::read_dir(path).map_err(EnvrError::from)? {
        let ent = ent.map_err(EnvrError::from)?;
        let p = ent.path();
        if p.is_dir() {
            let _ = prune_dir_by_mtime(&p, cutoff);
            if fs::read_dir(&p)
                .map_err(EnvrError::from)?
                .next()
                .is_none()
            {
                let _ = fs::remove_dir(&p);
            }
            continue;
        }
        if let Ok(meta) = fs::metadata(&p)
            && let Ok(m) = meta.modified()
            && m < cutoff
        {
            let _ = fs::remove_file(&p);
        }
    }
    Ok(())
}

pub fn enforce_rust_constraints(
    env_map: &HashMap<String, String>,
    cfg: &ProjectConfig,
    working_dir: &Path,
) -> EnvrResult<()> {
    let Some(r) = cfg.runtimes.get("rust") else {
        return Ok(());
    };
    let want_channel = r
        .channel
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let want_prefix = r
        .version_prefix
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    if want_channel.is_none() && want_prefix.is_none() {
        return Ok(());
    }
    let mode = r.enforce.unwrap_or(RustEnforceMode::Warn);

    let current_channel = (|| -> Option<String> {
        let o = std::process::Command::new("rustup")
            .args(["show", "active-toolchain"])
            .env_clear()
            .envs(env_map)
            .current_dir(working_dir)
            .output()
            .ok()?;
        if !o.status.success() {
            return None;
        }
        let s = String::from_utf8_lossy(&o.stdout);
        parse_rust_channel_from_toolchain(&s)
    })();

    let current_rustc = (|| -> Option<String> {
        let o = std::process::Command::new("rustc")
            .arg("-V")
            .env_clear()
            .envs(env_map)
            .current_dir(working_dir)
            .output()
            .ok()?;
        if !o.status.success() {
            return None;
        }
        let s = String::from_utf8_lossy(&o.stdout);
        rustc_version_from_output(&s)
    })();

    let mut problems = Vec::new();
    if let Some(want) = want_channel
        && current_channel.as_deref() != Some(&want.to_ascii_lowercase())
    {
        problems.push(format!(
            "rust channel mismatch: want {want}, got {}",
            current_channel.as_deref().unwrap_or("(unknown)")
        ));
    }
    if let Some(pref) = want_prefix
        && !current_rustc
            .as_deref()
            .is_some_and(|v| v.starts_with(pref))
    {
        problems.push(format!(
            "rustc version mismatch: want prefix {pref}, got {}",
            current_rustc.as_deref().unwrap_or("(unknown)")
        ));
    }
    if problems.is_empty() {
        return Ok(());
    }

    let msg = format!("Rust constraints not satisfied: {}", problems.join("; "));
    match mode {
        RustEnforceMode::Warn => {
            eprintln!("envr: warning: {msg}");
            Ok(())
        }
        RustEnforceMode::Error => Err(EnvrError::Validation(msg)),
    }
}
