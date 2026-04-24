//! Library surface for `envr` CLI logic (binary stays a thin `main` + logging bootstrap).
//!
//! Downstream tests and tools can depend on this crate without linking the full binary.
//!
//! # API stability (semver)
//!
//! Treat as **stable** for embedding and follow semver when changing:
//!
//! | Area | Symbols |
//! |------|---------|
//! | Process entry | [`bootstrap_i18n`], [`run_cli_with_logging`] |
//! | Parsed argv / globals | [`cli::Cli`], [`cli::GlobalArgs`], [`cli::apply_global`], [`cli::run`], [`commands::dispatch`] → `(CommandOutcome, GlobalArgs)` |
//! | Command results | [`CommandOutcome`], [`CliExit`], [`finish_cli_cmd`] |
//! | Output UX | [`presenter::CliUxPolicy`], [`presenter::CliPresenter`] |
//! | Runtime / project helpers | [`CliRuntimeSession`], [`CliPathProfile`], [`CliProjectContext`], [`RunExecContext`] |
//!
//! The [`commands`] module is a **large, mostly internal** handler surface: prefer the items above.
//! New subcommands can land under `commands` without a major version if stable entry types and JSON
//! contracts are unchanged; changing [`cli::Cli`] shape or documented output contracts is **breaking**.

mod app;
pub mod cli;
pub mod cli_help;
pub mod codes;
pub mod command_outcome;
pub mod commands;
pub mod output;
pub mod presenter;
pub mod run_context;
mod runtime_session;

pub use command_outcome::{CliExit, CommandOutcome, finish_cli_cmd};
use envr_config::env_context::load_settings_cached;
pub use presenter::{CliPresenter, CliUxPolicy};
pub use run_context::{CliPathProfile, CliProjectContext, RunExecContext};
pub use runtime_session::CliRuntimeSession;

/// Resolve the effective locale for this process from `settings.toml` (no global mutation).
pub fn bootstrap_locale() -> envr_core::i18n::Locale {
    if let Ok(st) = load_settings_cached() {
        return envr_core::i18n::locale_from_settings(&st);
    }
    envr_core::i18n::Locale::EnUs
}

/// Match `settings.toml` / platform locale before parsing argv (same as the `envr` binary).
pub fn bootstrap_i18n() {
    if let Ok(st) = load_settings_cached() {
        envr_core::i18n::init_from_settings(&st);
    }
}

/// Initialize tracing (rolling log file + console on stderr), then dispatch CLI commands.
///
/// Console tracing always uses **stderr** so stdout stays reserved for normal command output and
/// `--format json` envelopes even when `RUST_LOG` is set. The `debug` flag only affects default
/// filter setup (used only when `RUST_LOG` is unset or invalid).
pub fn run_cli_with_logging(cli: cli::Cli, debug_enabled: bool) -> i32 {
    let globals = cli.effective_global_args();
    let default_filter = if globals.quiet {
        Some("error")
    } else if debug_enabled {
        Some("debug")
    } else {
        None
    };
    let _logging_guard = match envr_core::logging::init_logging_with(
        "envr-cli",
        envr_core::logging::LoggingInitOptions {
            log_to_stderr: true,
            default_filter,
        },
    ) {
        Ok(guard) => guard,
        Err(err) => {
            let prefix = envr_core::i18n::tr_key(
                "cli.bootstrap.logging_failed",
                "初始化日志失败",
                "failed to init logging",
            );
            eprintln!(
                "{}: {}",
                prefix,
                envr_core::logging::format_error_chain(&err)
            );
            return 2;
        }
    };

    cli::emit_pending_parse_metrics();
    tracing::info!(debug_enabled, "envr-cli started");
    cli::run(cli)
}

/// Best-effort flush of parse-phase metrics when argv parsing failed before normal CLI run.
///
/// This initializes logging with default options, emits pending parse metrics if present,
/// and ignores logging-init failures so process exit behavior remains unchanged.
pub fn flush_parse_metrics_on_early_exit() {
    if let Ok(_guard) = envr_core::logging::init_logging_with(
        "envr-cli",
        envr_core::logging::LoggingInitOptions {
            log_to_stderr: true,
            default_filter: None,
        },
    ) {
        cli::emit_pending_parse_metrics();
    }
}
