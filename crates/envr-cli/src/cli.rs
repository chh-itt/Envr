//! Command-line interface for `envr` (clap tree and global flags).

use clap::{Parser, Subcommand, ValueEnum};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, ValueEnum)]
pub enum OutputFormat {
    /// Human-readable text (default).
    #[default]
    Text,
    /// JSON for scripts and automation.
    Json,
}

#[derive(Parser, Debug)]
#[command(
    name = "envr",
    version,
    about = "Language runtime version manager",
    propagate_version = true
)]
pub struct Cli {
    #[command(flatten)]
    pub global: GlobalArgs,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Parser, Debug)]
pub struct GlobalArgs {
    /// Output format (`text` or `json`). Default: `text`.
    #[arg(long = "format", value_enum, global = true)]
    pub output_format: Option<OutputFormat>,

    /// Suppress non-error output.
    #[arg(long, global = true)]
    pub quiet: bool,

    /// Disable ANSI color in terminal output.
    #[arg(long, global = true)]
    pub no_color: bool,

    /// Override runtime root directory (sets `ENVR_RUNTIME_ROOT`).
    #[arg(long, global = true, value_name = "PATH")]
    pub runtime_root: Option<String>,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Install a runtime version
    Install {
        #[arg(value_name = "LANG")]
        lang: Option<String>,
        #[arg(value_name = "VERSION")]
        runtime_version: Option<String>,
    },
    /// Select a runtime for the current shell
    Use {
        #[arg(value_name = "LANG")]
        lang: Option<String>,
        #[arg(value_name = "VERSION")]
        runtime_version: Option<String>,
    },
    /// List installed runtimes
    List {
        #[arg(value_name = "LANG")]
        lang: Option<String>,
    },
    /// Show the active runtime version
    Current {
        #[arg(value_name = "LANG")]
        lang: Option<String>,
    },
    /// Uninstall a runtime version
    Uninstall {
        #[arg(value_name = "LANG")]
        lang: Option<String>,
        #[arg(value_name = "VERSION")]
        runtime_version: Option<String>,
    },
    /// Locate a shim or executable
    Which {
        #[arg(value_name = "NAME")]
        name: Option<String>,
    },
    /// List available remote versions
    Remote {
        #[arg(value_name = "LANG")]
        lang: Option<String>,
        /// Limit remote versions to those whose labels start with this prefix
        #[arg(long, value_name = "PREFIX")]
        prefix: Option<String>,
    },
    /// Run diagnostics and environment checks
    Doctor,
}

/// Apply global flags to the process environment before logging and core calls.
///
/// # Safety
///
/// Mutates process environment during single-threaded startup before any other
/// threads read these variables (see `std::env::set_var` safety contract in Rust 2024).
pub fn apply_global(args: &GlobalArgs) {
    // SAFETY: CLI entry point runs before worker threads; env is read by logging/core after this.
    unsafe {
        if args.no_color {
            std::env::set_var("NO_COLOR", "1");
        }
        if let Some(ref p) = args.runtime_root {
            std::env::set_var("ENVR_RUNTIME_ROOT", p);
        }
        match args.output_format.unwrap_or(OutputFormat::Text) {
            OutputFormat::Json => {
                std::env::set_var("ENVR_OUTPUT_FORMAT", "json");
            }
            OutputFormat::Text => {
                std::env::remove_var("ENVR_OUTPUT_FORMAT");
            }
        }
        if args.quiet {
            std::env::set_var("RUST_LOG", "error");
        }
    }
}

pub fn run(cli: Cli) -> i32 {
    crate::commands::dispatch(cli)
}
