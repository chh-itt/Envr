//! Global output flags and shared clap argument groups.

use clap::{Args, Parser, ValueEnum};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, ValueEnum)]
pub enum OutputFormat {
    /// Human-readable text (default).
    #[default]
    Text,
    /// JSON for scripts and automation.
    Json,
}

#[derive(Parser, Debug, Clone)]
pub struct GlobalArgs {
    /// Output format (`text` or `json`). Default: `text`.
    #[arg(long = "format", value_enum, global = true)]
    pub output_format: Option<OutputFormat>,

    /// Script-friendly plain text output (no labels/decorations).
    #[arg(long, alias = "plain", global = true)]
    pub porcelain: bool,

    /// Suppress non-error output.
    #[arg(long, global = true)]
    pub quiet: bool,

    /// Disable ANSI color in terminal output.
    #[arg(long, global = true)]
    pub no_color: bool,

    /// When set, default `RUST_LOG=debug` if unset; tracing always goes to **stderr** (stdout stays for command / JSON output).
    #[arg(long, global = true)]
    pub debug: bool,

    /// Emit detailed step logs for write operations to stderr.
    #[arg(long, global = true)]
    pub verbose: bool,

    /// Override runtime root directory for this process.
    #[arg(long, global = true, value_name = "PATH")]
    pub runtime_root: Option<String>,
}

/// Project config search directory and optional `[profiles.*]` overlay.
/// Shared by `why`, `resolve`, `status`, `env`, `template`, and `shell`.
#[derive(Args, Clone, Debug)]
pub struct ProjectPathProfileArgs {
    /// Working directory for upward `.envr.toml` search
    #[arg(long, value_name = "DIR", default_value = ".")]
    pub path: PathBuf,
    /// Profile overlay (`[profiles.<name>]`), overrides `ENVR_PROFILE` for this invocation
    #[arg(long, value_name = "NAME")]
    pub profile: Option<String>,
}

/// Shared flags for `exec` and `run`: working directory, profile, env files,
/// install-if-missing, and dry-run modes. Keep this struct in sync for both subcommands.
#[derive(Args, Clone, Debug)]
pub struct ExecRunSharedArgs {
    /// Install missing pinned (or specified) runtimes before executing
    #[arg(long, alias = "install")]
    pub install_if_missing: bool,
    /// Print merged env and command, then exit without running
    #[arg(long, conflicts_with = "dry_run_diff")]
    pub dry_run: bool,
    /// Like `--dry-run` but print only **changes** vs the current process env (PATH entries split)
    #[arg(long, conflicts_with = "dry_run")]
    pub dry_run_diff: bool,
    /// Log resolved runtime paths before executing
    #[arg(long, short = 'v')]
    pub verbose: bool,
    #[arg(long, value_name = "DIR", default_value = ".")]
    pub path: PathBuf,
    /// Profile overlay (`[profiles.<name>]`), overrides `ENVR_PROFILE` for this invocation
    #[arg(long, value_name = "NAME")]
    pub profile: Option<String>,
    /// Set or override an environment variable for the child (`KEY=VALUE`; repeatable)
    #[arg(long = "env", value_name = "KEY=VALUE", action = clap::ArgAction::Append)]
    pub env: Vec<String>,
    /// Load environment entries from a file before applying `--env` (repeatable)
    #[arg(long, value_name = "PATH", action = clap::ArgAction::Append)]
    pub env_file: Vec<PathBuf>,
}

impl GlobalArgs {
    /// Output format from global `--format` (default: [`OutputFormat::Text`]).
    ///
    /// Prefer this over `output_format.unwrap_or(OutputFormat::Text)` so behavior stays consistent
    /// with [`crate::cli::Cli::resolved_output_format`] and subcommand `--json` shorthands (see
    /// [`crate::cli::Command::legacy_json_shorthand`] and [`GlobalArgs::cloned_with_legacy_json`]).
    #[inline]
    pub fn effective_output_format(&self) -> OutputFormat {
        self.output_format.unwrap_or(OutputFormat::Text)
    }

    /// Clone `self`, forcing [`OutputFormat::Json`] when the subcommand sets a legacy `--json` flag
    /// (e.g. `doctor --json`), equivalent to `--format json` for that invocation.
    #[inline]
    pub fn cloned_with_legacy_json(&self, legacy_json: bool) -> Self {
        if legacy_json {
            Self {
                output_format: Some(OutputFormat::Json),
                ..self.clone()
            }
        } else {
            self.clone()
        }
    }
}
