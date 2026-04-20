//! Root `envr` clap subcommand enum ([`Command`]).

use super::nested::{
    AliasCmd, BundleCmd, CacheCmd, ConfigCmd, DebugCmd, DiagnosticsCmd, EnvShellKind, HelpCmd,
    HookCmd, ProfileCmd, ProjectCmd, RustCmd, ShimCmd,
};
use crate::cli::global::{ExecRunSharedArgs, ProjectPathProfileArgs};

use clap::Subcommand;
use clap_complete::Shell;
use std::path::PathBuf;

// Subcommands are ordered by topic (runtime → project → data → diagnostics) to make `--help` easier to scan.
// Clap 4 does not render multiple titled subcommand sections; see `cli::help_registry::apply_root_help` `after_long_help`.
#[derive(Subcommand, Debug)]
pub enum Command {
    /// Install a runtime version
    #[command(visible_alias = "i")]
    Install {
        #[arg(value_name = "RUNTIME")]
        runtime: String,
        #[arg(value_name = "VERSION")]
        runtime_version: String,
    },
    /// Set the **global default** runtime version (updates `current` under the runtime root; same idea as nvm/fnm).
    /// For a **temporary** shell-only environment, prefer `envr shell`, `envr exec`, or `eval "$(envr env …)"`.
    #[command(visible_alias = "sw")]
    Use {
        #[arg(value_name = "RUNTIME")]
        runtime: String,
        #[arg(value_name = "VERSION")]
        runtime_version: String,
    },
    /// List installed runtimes
    #[command(visible_alias = "ls")]
    List {
        #[arg(value_name = "RUNTIME")]
        runtime: Option<String>,
        /// Compare installed versions to remote latest-per-major index (network / cache)
        #[arg(long)]
        outdated: bool,
    },
    /// Show the active runtime version
    #[command(visible_alias = "cur")]
    Current {
        #[arg(value_name = "RUNTIME")]
        runtime: Option<String>,
    },
    /// Uninstall a runtime version
    #[command(visible_alias = "u")]
    Uninstall {
        #[arg(value_name = "RUNTIME")]
        runtime: String,
        #[arg(value_name = "VERSION")]
        runtime_version: String,
        /// Print what would be removed without deleting anything
        #[arg(long)]
        dry_run: bool,
        /// Allow uninstalling the globally active version (still confirms unless `--yes`)
        #[arg(long)]
        force: bool,
        /// Skip interactive confirmation
        #[arg(long, short = 'y')]
        yes: bool,
    },
    /// Locate a shim or executable
    Which {
        #[arg(value_name = "NAME")]
        name: Option<String>,
    },
    /// List available remote versions
    Remote {
        #[arg(value_name = "RUNTIME")]
        runtime: Option<String>,
        /// Limit remote versions to those whose labels start with this prefix
        #[arg(long, value_name = "PREFIX")]
        prefix: Option<String>,
        /// Force live refresh before rendering (ignore cached snapshot for this run)
        #[arg(long, short = 'u')]
        update: bool,
    },
    /// Rust / rustup helpers (e.g. managed rustup when no system rustup)
    #[command(subcommand)]
    Rust(RustCmd),
    /// Explain how a runtime version is resolved (project pin vs global current)
    Why {
        #[arg(value_name = "RUNTIME")]
        runtime: String,
        /// Version spec override (same as `resolve --spec`; wins over project pin)
        #[arg(long, value_name = "SPEC")]
        spec: Option<String>,
        #[command(flatten)]
        project: ProjectPathProfileArgs,
    },
    /// Print the runtime home directory shims would use (project pin, or global current)
    Resolve {
        /// Language key: `node`, `python`, `java`, `kotlin`, `scala`, `clojure`, `groovy`, …
        #[arg(value_name = "LANG")]
        lang: String,
        /// Version spec override (ignores project pin for this invocation)
        #[arg(long, value_name = "SPEC")]
        spec: Option<String>,
        #[command(flatten)]
        project: ProjectPathProfileArgs,
    },
    /// Run a subprocess with PATH and env for one language (project pins + `ENVR_PROFILE` / `--profile`)
    Exec {
        /// Language key: `node`, `python`, `java`, `kotlin`, `scala`, `clojure`, `groovy`, …
        #[arg(long, value_name = "LANG")]
        lang: String,
        #[arg(long, value_name = "SPEC")]
        spec: Option<String>,
        #[command(flatten)]
        shared: ExecRunSharedArgs,
        /// Append child stdout and stderr to this file (envr messages stay on stderr)
        #[arg(long, value_name = "FILE")]
        output: Option<PathBuf>,
        #[arg(value_name = "COMMAND", required = true)]
        command: String,
        #[arg(
            trailing_var_arg = true,
            allow_hyphen_values = true,
            value_name = "ARGS"
        )]
        args: Vec<String>,
    },
    /// Run a subprocess with merged PATH for configured runtimes (e.g. node, python, java, kotlin, scala, clojure, groovy) plus project `env`.
    /// If the first token matches `[scripts]` in `.envr.toml`, it is run as a shell one-liner.
    Run {
        #[command(flatten)]
        shared: ExecRunSharedArgs,
        #[arg(value_name = "COMMAND", required = true)]
        command: String,
        #[arg(
            trailing_var_arg = true,
            allow_hyphen_values = true,
            value_name = "ARGS"
        )]
        args: Vec<String>,
    },
    /// Print shell snippets setting PATH / runtime-home env / project env (merged runtimes)
    Env {
        #[command(flatten)]
        project: ProjectPathProfileArgs,
        #[arg(long, value_enum, default_value_t = EnvShellKind::Posix)]
        shell: EnvShellKind,
    },
    /// Render a template file with `${VAR}` placeholders using the merged `envr run` environment
    Template {
        #[arg(value_name = "FILE")]
        file: PathBuf,
        #[command(flatten)]
        project: ProjectPathProfileArgs,
        /// Set or override an environment variable for substitution (`KEY=VALUE`; repeatable)
        #[arg(long = "env", value_name = "KEY=VALUE", action = clap::ArgAction::Append)]
        env: Vec<String>,
        /// Load environment entries from a file (`KEY=VALUE` lines, `#` comments; repeatable)
        #[arg(long, value_name = "PATH", action = clap::ArgAction::Append)]
        env_file: Vec<PathBuf>,
    },
    /// Start an interactive subshell with the merged `envr env` environment
    Shell {
        #[command(flatten)]
        project: ProjectPathProfileArgs,
        /// Executable to run instead of `$SHELL` / `%ComSpec%` (or `ENVR_SHELL`)
        #[arg(long, value_name = "EXE")]
        shell: Option<PathBuf>,
    },
    /// Auto-apply project env when `cd` into a tree with `.envr.toml` (shell integration)
    #[command(subcommand)]
    Hook(HookCmd),
    /// Remove installed versions except the active `current` selection
    Prune {
        /// Limit to one language (`node`, `python`, `java`, `kotlin`, `scala`, `clojure`, `groovy`, …); default: all
        #[arg(value_name = "LANG")]
        lang: Option<String>,
        /// Actually uninstall (default is a dry-run plan only)
        #[arg(long)]
        execute: bool,
    },
    /// Create a starter `.envr.toml` in the given directory
    Init {
        /// Directory that will contain `.envr.toml`
        #[arg(long, value_name = "DIR", default_value = ".")]
        path: PathBuf,
        /// Overwrite an existing `.envr.toml`
        #[arg(long)]
        force: bool,
        /// Include commented `[env]` / `[profiles]` examples (tutorial-style)
        #[arg(long)]
        full: bool,
        /// Prompt for pinned runtimes (TTY only; implies text output, not `--format json`)
        #[arg(long)]
        interactive: bool,
    },
    /// Verify `.envr.toml` / pins resolve to installed runtimes (same rules as shims)
    Check {
        /// Directory or file to start config search from
        #[arg(long, value_name = "DIR", default_value = ".")]
        path: PathBuf,
    },
    /// Show project root (if any), pins, and active runtime versions for this directory
    #[command(visible_alias = "st")]
    Status {
        #[command(flatten)]
        project: ProjectPathProfileArgs,
    },
    /// Manage `.envr.toml` pins (add, install sync, validate)
    #[command(subcommand)]
    Project(ProjectCmd),
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
    #[command(visible_alias = "cfg", subcommand)]
    Config(ConfigCmd),
    /// Manage CLI aliases (`config/aliases.toml`)
    #[command(subcommand)]
    Alias(AliasCmd),
    /// Manage shims under `{runtime_root}/shims`
    #[command(visible_alias = "sh", subcommand)]
    Shim(ShimCmd),
    /// Manage envr download caches under `{runtime_root}/cache`
    #[command(visible_alias = "c", subcommand)]
    Cache(CacheCmd),
    /// Create/apply a portable bundle for offline deployment
    #[command(subcommand)]
    Bundle(BundleCmd),
    /// Run diagnostics and environment checks
    #[command(visible_alias = "doc")]
    Doctor {
        /// Apply safe automatic fixes (refresh empty shims, set or repair `current` when needed)
        #[arg(long)]
        fix: bool,
        /// Print reviewed copy/paste commands to add shims to PATH permanently (suggestions only)
        #[arg(long)]
        fix_path: bool,
        /// After `--fix-path`, on Windows prompt to run the User-scope PowerShell PATH snippet (requires `--fix-path`)
        #[arg(long, requires = "fix_path")]
        fix_path_apply: bool,
        /// Machine-readable output (same as `--format json`; for dashboards / CI)
        #[arg(long)]
        json: bool,
    },
    /// Restore variables saved by `envr hook` (run `envr deactivate` after `eval "$(envr hook …)"`)
    #[command(visible_alias = "off")]
    Deactivate,
    /// Troubleshooting helpers (paths, environment snapshot)
    #[command(subcommand)]
    Debug(DebugCmd),
    /// Export a diagnostics zip for bug reports (doctor JSON, env summary, recent logs)
    #[command(subcommand)]
    Diagnostics(DiagnosticsCmd),
    /// Print shell tab-completion script for `envr` (stdout)
    Completion {
        #[arg(value_enum)]
        shell: Shell,
    },
    /// Supplemental help (argv shorthands; see also completion script header comment)
    #[command(subcommand)]
    Help(HelpCmd),
    /// Show CLI version and update notes
    Update {
        /// Reserved for a future release check
        #[arg(long)]
        check: bool,
    },
}
