//! Command-line interface for `envr` (clap tree and global flags).

use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

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
    /// Create a starter `.envr.toml` in the given directory
    Init {
        /// Directory that will contain `.envr.toml`
        #[arg(long, value_name = "DIR", default_value = ".")]
        path: PathBuf,
        /// Overwrite an existing `.envr.toml`
        #[arg(long)]
        force: bool,
    },
    /// Verify `.envr.toml` / pins resolve to installed runtimes (same rules as shims)
    Check {
        /// Directory or file to start config search from
        #[arg(long, value_name = "DIR", default_value = ".")]
        path: PathBuf,
    },
    /// Print the runtime home directory shims would use (project pin, or global current)
    Resolve {
        /// Language key: `node`, `python`, or `java`
        #[arg(value_name = "LANG")]
        lang: String,
        /// Version spec override (ignores project pin for this invocation)
        #[arg(long, value_name = "SPEC")]
        spec: Option<String>,
        /// Working directory for upward `.envr.toml` search
        #[arg(long, value_name = "DIR", default_value = ".")]
        path: PathBuf,
        /// Profile overlay (`[profiles.<name>]`), overrides `ENVR_PROFILE` for this invocation
        #[arg(long, value_name = "NAME")]
        profile: Option<String>,
    },
    /// Run a subprocess with PATH and env for one language (project pins + `ENVR_PROFILE` / `--profile`)
    Exec {
        /// Language key: `node`, `python`, or `java`
        #[arg(long, value_name = "LANG")]
        lang: String,
        #[arg(long, value_name = "SPEC")]
        spec: Option<String>,
        #[arg(long, value_name = "DIR", default_value = ".")]
        path: PathBuf,
        #[arg(long, value_name = "NAME")]
        profile: Option<String>,
        #[arg(value_name = "COMMAND", required = true)]
        command: String,
        #[arg(
            trailing_var_arg = true,
            allow_hyphen_values = true,
            value_name = "ARGS"
        )]
        args: Vec<String>,
    },
    /// Run a subprocess with merged PATH for node, python, and java (plus project `env`)
    Run {
        #[arg(long, value_name = "DIR", default_value = ".")]
        path: PathBuf,
        #[arg(long, value_name = "NAME")]
        profile: Option<String>,
        #[arg(value_name = "COMMAND", required = true)]
        command: String,
        #[arg(
            trailing_var_arg = true,
            allow_hyphen_values = true,
            value_name = "ARGS"
        )]
        args: Vec<String>,
    },
    /// Print shell snippets setting PATH / JAVA_HOME / project env (merged runtimes)
    Env {
        #[arg(long, value_name = "DIR", default_value = ".")]
        path: PathBuf,
        #[arg(long, value_name = "NAME")]
        profile: Option<String>,
        #[arg(long, value_enum, default_value_t = EnvShellKind::Posix)]
        shell: EnvShellKind,
    },
    /// Merge a TOML file into the project `.envr.toml` (imported keys win on conflict)
    Import {
        #[arg(value_name = "FILE")]
        file: PathBuf,
        #[arg(long, value_name = "DIR", default_value = ".")]
        path: PathBuf,
    },
    /// Print merged on-disk project config (base + local, no profile overlay) as TOML
    Export {
        #[arg(long, value_name = "DIR", default_value = ".")]
        path: PathBuf,
        #[arg(long, value_name = "FILE")]
        output: Option<PathBuf>,
    },
    /// Inspect `[profiles.*]` blocks (use `ENVR_PROFILE` or `exec`/`run` `--profile` to activate)
    #[command(subcommand)]
    Profile(ProfileCmd),
    /// Inspect user settings (`settings.toml`)
    #[command(subcommand)]
    Config(ConfigCmd),
    /// Manage CLI aliases (`config/aliases.toml`)
    #[command(subcommand)]
    Alias(AliasCmd),
    /// Remove installed versions except the active `current` selection
    Prune {
        /// Limit to one language (`node`, `python`, `java`); default: all
        #[arg(value_name = "LANG")]
        lang: Option<String>,
        /// Actually uninstall (default is a dry-run plan only)
        #[arg(long)]
        execute: bool,
    },
    /// Show CLI version and update notes
    Update {
        /// Reserved for a future release check
        #[arg(long)]
        check: bool,
    },
    /// Manage shims under `{runtime_root}/shims`
    #[command(subcommand)]
    Shim(ShimCmd),
    /// Manage envr download caches under `{runtime_root}/cache`
    #[command(subcommand)]
    Cache(CacheCmd),
}

#[derive(Subcommand, Debug)]
pub enum ShimCmd {
    /// Refresh core shims (and optionally global package forwards)
    Sync {
        /// Also sync global package executables (npm global bin, bun global bin)
        #[arg(long)]
        globals: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum CacheCmd {
    /// Remove download/extract caches
    Clean {
        /// Limit to one cache kind (e.g. `bun`, `node`). Default: remove all cache.
        #[arg(value_name = "KIND")]
        kind: Option<String>,
        /// Alias for removing all cache (same as no KIND).
        #[arg(long)]
        all: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum ConfigCmd {
    /// Print absolute path to `settings.toml`
    Path,
    /// Print merged settings (defaults + file)
    Show,
}

#[derive(Subcommand, Debug)]
pub enum ProfileCmd {
    /// List profile names defined in merged project config
    List {
        #[arg(long, value_name = "DIR", default_value = ".")]
        path: PathBuf,
    },
    /// Show runtimes and env for a named profile
    Show {
        #[arg(value_name = "NAME")]
        name: String,
        #[arg(long, value_name = "DIR", default_value = ".")]
        path: PathBuf,
    },
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, ValueEnum)]
pub enum EnvShellKind {
    #[default]
    Posix,
    Cmd,
    Powershell,
}

#[derive(Subcommand, Debug)]
pub enum AliasCmd {
    /// List aliases
    List,
    /// Add or replace an alias (`name` expands to `target`, e.g. `n` → `node`)
    Add {
        #[arg(value_name = "NAME")]
        name: String,
        #[arg(value_name = "TARGET")]
        target: String,
    },
    /// Remove an alias
    Remove {
        #[arg(value_name = "NAME")]
        name: String,
    },
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
