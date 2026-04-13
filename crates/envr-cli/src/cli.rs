//! Command-line interface for `envr` (clap tree and global flags).

use clap::{Args, Parser, Subcommand, ValueEnum};
use clap_complete::Shell;
use envr_config::aliases::AliasesFile;
use envr_platform::paths::current_platform_paths;
use std::ffi::OsString;
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
    propagate_version = true,
    disable_help_subcommand = true
)]
pub struct Cli {
    #[command(flatten)]
    pub global: GlobalArgs,

    #[command(subcommand)]
    pub command: Command,
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

    /// Override runtime root directory (sets `ENVR_RUNTIME_ROOT`).
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

/// Shared flags for [`Command::Exec`] and [`Command::Run`]: working directory, profile, env files,
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
    /// with [`Cli::resolved_output_format`] and subcommand `--json` shorthands (see
    /// [`GlobalArgs::cloned_with_legacy_json`]).
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

impl Cli {
    /// Effective output format for this argv after global flags and known subcommand shorthands.
    ///
    /// Used by [`apply_global`] so `ENVR_OUTPUT_FORMAT` matches what handlers will use (e.g.
    /// `doctor --json` implies JSON the same way as `--format json`).
    #[inline]
    pub fn resolved_output_format(&self) -> OutputFormat {
        match &self.command {
            Command::Doctor { json: true, .. } => OutputFormat::Json,
            _ => self.global.effective_output_format(),
        }
    }
}

// Subcommands are ordered by topic (runtime → project → data → diagnostics) to make `--help` easier to scan.
// Clap 4 does not render multiple titled subcommand sections; see `cli_help::patch_root` `after_long_help`.
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
        /// Language key: `node`, `python`, or `java`
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
        /// Language key: `node`, `python`, or `java`
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
    /// Run a subprocess with merged PATH for node, python, and java (plus project `env`).
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
    /// Print shell snippets setting PATH / JAVA_HOME / project env (merged runtimes)
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
        /// Limit to one language (`node`, `python`, `java`); default: all
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
        /// Remove only files under the cache tree older than this age (e.g. `30d`, `7d`, `24h`, `90m`, `3600s`, `1w`). Units: `s`, `m`, `h`, `d`, `w` (and common long forms like `days`). Keeps the cache root directory; prunes empty subdirectories after deletes.
        #[arg(long, value_name = "DURATION")]
        older_than: Option<String>,
        /// With `--older-than`, only delete files also **newer** than this longer age (further in the past), i.e. keep files outside the window. Example: `--newer-than 90d --older-than 30d` targets files last modified between 30 and 90 days ago. Requires `--older-than`.
        #[arg(long, value_name = "DURATION")]
        newer_than: Option<String>,
        /// Show what would be removed (file counts / sizes, or whole-tree intent) without deleting anything.
        #[arg(long)]
        dry_run: bool,
    },
    /// Manage offline remote index cache
    #[command(subcommand)]
    Index(CacheIndexCmd),
}

#[derive(Subcommand, Debug)]
pub enum CacheIndexCmd {
    /// Download remote indexes/tags into the index cache directory
    Sync {
        /// Limit to one runtime (e.g. `node`, `deno`, `bun`). Default: all.
        #[arg(value_name = "RUNTIME")]
        runtime: Option<String>,
        /// Use all runtimes (same as omitting RUNTIME).
        #[arg(long)]
        all: bool,
        /// Override index cache directory (default: ENVR_INDEX_CACHE_DIR or `{runtime_root}/cache/indexes`)
        #[arg(long, value_name = "DIR")]
        dir: Option<PathBuf>,
    },
    /// Show cached index files and freshness
    Status {
        /// Override index cache directory (default: ENVR_INDEX_CACHE_DIR or `{runtime_root}/cache/indexes`)
        #[arg(long, value_name = "DIR")]
        dir: Option<PathBuf>,
    },
}

#[derive(Subcommand, Debug)]
pub enum BundleCmd {
    /// Create a portable bundle (runtimes + indexes + project config)
    Create {
        /// Output `.zip` path (default: `envr-bundle-<unix_secs>.zip` in cwd)
        #[arg(long, value_name = "PATH")]
        output: Option<PathBuf>,
        /// Working directory for upward `.envr.toml` search
        #[arg(long, value_name = "DIR", default_value = ".")]
        path: PathBuf,
        /// Profile overlay (`[profiles.<name>]`), overrides `ENVR_PROFILE` for this invocation
        #[arg(long, value_name = "NAME")]
        profile: Option<String>,
        /// Include offline remote index cache (`cache/indexes`)
        #[arg(long)]
        include_indexes: bool,
        /// Include shims (`{runtime_root}/shims`)
        #[arg(long)]
        include_shims: bool,
        /// Include all runtimes under `{runtime_root}/runtimes` (larger, but simplest)
        #[arg(long)]
        full: bool,
        /// Do not include global `current` selections (project pins only)
        #[arg(long)]
        no_current: bool,
    },
    /// Apply a bundle to the current machine
    Apply {
        /// Bundle `.zip` file
        #[arg(value_name = "FILE")]
        file: PathBuf,
        /// Override runtime root directory for apply (sets `ENVR_RUNTIME_ROOT` for this command)
        #[arg(long, value_name = "PATH")]
        runtime_root: Option<String>,
        /// Override index cache directory for apply (default: ENVR_INDEX_CACHE_DIR or `{runtime_root}/cache/indexes`)
        #[arg(long, value_name = "DIR")]
        index_cache_dir: Option<PathBuf>,
    },
}

#[derive(Subcommand, Debug)]
pub enum ConfigCmd {
    /// Print a commented `settings.toml` template (Chinese descriptions + defaults)
    Schema,
    /// Validate `settings.toml` (parse + semantic rules); prints OK or human-readable errors
    Validate,
    /// Open `settings.toml` in `$EDITOR` / `VISUAL` and validate after save
    Edit,
    /// Print absolute path to `settings.toml`
    Path,
    /// Print merged settings (defaults + file)
    Show,
    /// List writable settings keys
    Keys,
    /// Read one settings key by dotted path
    Get {
        #[arg(value_name = "KEY")]
        key: String,
    },
    /// Set one settings key by dotted path
    Set {
        #[arg(value_name = "KEY")]
        key: String,
        #[arg(value_name = "VALUE")]
        value: String,
        /// Parse VALUE as this exact type (overrides auto-parse).
        #[arg(long = "type", value_enum)]
        value_type: Option<ConfigValueType>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum ConfigValueType {
    String,
    Bool,
    Int,
    Float,
    Json,
}

#[derive(Subcommand, Debug)]
pub enum RustCmd {
    /// Install envr-managed rustup (downloads rustup-init; stable default toolchain)
    #[command(name = "install-managed")]
    InstallManaged,
}

#[derive(Subcommand, Debug)]
pub enum HookCmd {
    /// bash: add to `~/.bashrc` as `eval "$(envr hook bash)"` (bash 4+; uses `PROMPT_COMMAND`)
    Bash,
    /// zsh: add to `~/.zshrc` as `eval "$(envr hook zsh)"` (uses `chpwd`)
    Zsh,
    /// Print env var names that hooks save/restore (one per line; internal / debugging)
    Keys {
        #[arg(long, value_name = "DIR", default_value = ".")]
        path: PathBuf,
    },
    /// One-line runtime summary for shell prompts (use after `eval "$(envr hook …)"`; see `PS1` examples in hook output)
    Prompt {
        #[command(flatten)]
        project: ProjectPathProfileArgs,
    },
}

#[derive(Subcommand, Debug)]
pub enum DebugCmd {
    /// Print config paths, `ENVR_*` env, and a short runtime-root directory listing (for issues)
    Info,
}

#[derive(Subcommand, Debug)]
pub enum DiagnosticsCmd {
    /// Write `doctor.json`, `system.txt`, `environment.txt`, and recent `*.log` files into a zip
    Export {
        /// Output `.zip` path (default: `envr-diagnostics-<unix_secs>.zip` in cwd)
        #[arg(long, value_name = "PATH")]
        output: Option<PathBuf>,
    },
}

#[derive(Subcommand, Debug)]
pub enum HelpCmd {
    /// Built-in argv expansions before clap (`add`, `diag`, …)
    Shortcuts,
}

#[derive(Subcommand, Debug)]
pub enum ProjectCmd {
    /// Add or update a `[runtimes.<kind>]` pin (`node@20`, `python@3.12`)
    Add {
        #[arg(value_name = "SPEC")]
        spec: String,
        #[arg(long, value_name = "DIR", default_value = ".")]
        path: PathBuf,
    },
    /// Ensure pinned runtimes exist; with `--install`, download missing pins
    Sync {
        #[arg(long, value_name = "DIR", default_value = ".")]
        path: PathBuf,
        #[arg(long)]
        install: bool,
    },
    /// Verify pins resolve under the runtime root; optional remote index check
    Validate {
        #[arg(long, value_name = "DIR", default_value = ".")]
        path: PathBuf,
        /// Compare pins against remote indexes (may use network / local cache)
        #[arg(long)]
        check_remote: bool,
    },
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

impl Command {
    /// Stable snake_case label for tracing and structured logs (not localized).
    pub fn trace_name(&self) -> &'static str {
        match self {
            Command::Install { .. } => "install",
            Command::Use { .. } => "use",
            Command::List { .. } => "list",
            Command::Current { .. } => "current",
            Command::Uninstall { .. } => "uninstall",
            Command::Which { .. } => "which",
            Command::Remote { .. } => "remote",
            Command::Rust(sub) => match sub {
                RustCmd::InstallManaged => "rust_install_managed",
            },
            Command::Why { .. } => "why",
            Command::Resolve { .. } => "resolve",
            Command::Exec { .. } => "exec",
            Command::Run { .. } => "run",
            Command::Env { .. } => "env",
            Command::Template { .. } => "template",
            Command::Shell { .. } => "shell",
            Command::Hook(sub) => match sub {
                HookCmd::Bash => "hook_bash",
                HookCmd::Zsh => "hook_zsh",
                HookCmd::Keys { .. } => "hook_keys",
                HookCmd::Prompt { .. } => "hook_prompt",
            },
            Command::Prune { .. } => "prune",
            Command::Init { .. } => "init",
            Command::Check { .. } => "check",
            Command::Status { .. } => "status",
            Command::Project(sub) => match sub {
                ProjectCmd::Add { .. } => "project_add",
                ProjectCmd::Sync { .. } => "project_sync",
                ProjectCmd::Validate { .. } => "project_validate",
            },
            Command::Import { .. } => "import",
            Command::Export { .. } => "export",
            Command::Profile(sub) => match sub {
                ProfileCmd::List { .. } => "profile_list",
                ProfileCmd::Show { .. } => "profile_show",
            },
            Command::Config(sub) => match sub {
                ConfigCmd::Schema => "config_schema",
                ConfigCmd::Validate => "config_validate",
                ConfigCmd::Edit => "config_edit",
                ConfigCmd::Path => "config_path",
                ConfigCmd::Show => "config_show",
                ConfigCmd::Keys => "config_keys",
                ConfigCmd::Get { .. } => "config_get",
                ConfigCmd::Set { .. } => "config_set",
            },
            Command::Alias(sub) => match sub {
                AliasCmd::List => "alias_list",
                AliasCmd::Add { .. } => "alias_add",
                AliasCmd::Remove { .. } => "alias_remove",
            },
            Command::Shim(sub) => match sub {
                ShimCmd::Sync { .. } => "shim_sync",
            },
            Command::Cache(sub) => match sub {
                CacheCmd::Clean { .. } => "cache_clean",
                CacheCmd::Index(sub) => match sub {
                    CacheIndexCmd::Sync { .. } => "cache_index_sync",
                    CacheIndexCmd::Status { .. } => "cache_index_status",
                },
            },
            Command::Bundle(sub) => match sub {
                BundleCmd::Create { .. } => "bundle_create",
                BundleCmd::Apply { .. } => "bundle_apply",
            },
            Command::Doctor { .. } => "doctor",
            Command::Deactivate => "deactivate",
            Command::Debug(sub) => match sub {
                DebugCmd::Info => "debug_info",
            },
            Command::Diagnostics(sub) => match sub {
                DiagnosticsCmd::Export { .. } => "diagnostics_export",
            },
            Command::Completion { .. } => "completion",
            Command::Help(sub) => match sub {
                HelpCmd::Shortcuts => "help_shortcuts",
            },
            Command::Update { .. } => "update",
        }
    }
}

/// Apply global flags to the process environment before logging and core calls.
///
/// Uses [`Cli::resolved_output_format`] so `ENVR_OUTPUT_FORMAT` matches subcommand shorthands such as
/// `doctor --json`.
///
/// # Safety
///
/// Mutates process environment during single-threaded startup before any other
/// threads read these variables (see `std::env::set_var` safety contract in Rust 2024).
pub fn apply_global(cli: &Cli) {
    let args = &cli.global;
    // SAFETY: CLI entry point runs before worker threads; env is read by logging/core after this.
    unsafe {
        if args.no_color {
            std::env::set_var("NO_COLOR", "1");
        }
        if let Some(ref p) = args.runtime_root {
            std::env::set_var("ENVR_RUNTIME_ROOT", p);
        }
        match cli.resolved_output_format() {
            OutputFormat::Json => {
                std::env::set_var("ENVR_OUTPUT_FORMAT", "json");
            }
            OutputFormat::Text => {
                std::env::remove_var("ENVR_OUTPUT_FORMAT");
            }
        }
        if args.quiet {
            std::env::set_var("RUST_LOG", "error");
        } else if args.debug && std::env::var("RUST_LOG").is_err() {
            std::env::set_var("RUST_LOG", "debug");
        }
    }
}

/// Split an `alias add` target string into argv tokens (whitespace; supports `"` / `'` quotes).
pub fn split_alias_target(s: &str) -> Vec<String> {
    let s = s.trim();
    if s.is_empty() {
        return Vec::new();
    }
    let mut tokens = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    for ch in s.chars() {
        match quote {
            Some(q) if ch == q => {
                tokens.push(std::mem::take(&mut cur));
                quote = None;
            }
            Some(_) => cur.push(ch),
            None if ch == '"' || ch == '\'' => {
                if !cur.is_empty() {
                    tokens.push(std::mem::take(&mut cur));
                }
                quote = Some(ch);
            }
            None if ch.is_whitespace() => {
                if !cur.is_empty() {
                    tokens.push(std::mem::take(&mut cur));
                }
            }
            None => cur.push(ch),
        }
    }
    if !cur.is_empty() {
        tokens.push(cur);
    }
    tokens
}

/// Index of the first argv token after the program name that is not a known global flag
/// (nor its value). `args[0]` is the binary; scans `args[1..]`.
///
/// Stops at the first token that is not consumed as a global, at a lone `--` (returns the
/// following token), or at end of argv. Returns `args.len()` when there is no command token.
pub fn first_command_token_index(args: &[OsString]) -> usize {
    if args.len() <= 1 {
        return args.len();
    }
    let mut i = 1;
    while i < args.len() {
        let Some(s) = args[i].to_str() else {
            return i;
        };
        if s == "--" {
            return if i + 1 < args.len() { i + 1 } else { args.len() };
        }
        if !s.starts_with('-') {
            return i;
        }
        let lower = s.to_ascii_lowercase();
        if lower == "--porcelain"
            || lower == "--plain"
            || lower == "--quiet"
            || lower == "--no-color"
            || lower == "--debug"
        {
            i += 1;
            continue;
        }
        if lower.contains('=') && lower.starts_with("--format=") {
            i += 1;
            continue;
        }
        if lower == "--format" {
            if i + 1 < args.len() {
                i += 2;
            } else {
                i += 1;
            }
            continue;
        }
        if lower.contains('=') && lower.starts_with("--runtime-root=") {
            i += 1;
            continue;
        }
        if lower == "--runtime-root" {
            if i + 1 < args.len() {
                i += 2;
            } else {
                i += 1;
            }
            continue;
        }
        return i;
    }
    args.len()
}

/// Expand the first command-like argv token using `config/aliases.toml` (multi-token targets allowed).
///
/// Runs **before** [`preprocess_cli_args`] so user-defined names can override built-in shorthands
/// like `ci` / `diag` when desired.
pub fn expand_user_cli_aliases(mut args: Vec<OsString>) -> Vec<OsString> {
    const MAX_CHAIN: usize = 8;
    let Some(paths) = current_platform_paths().ok() else {
        return args;
    };
    let path = AliasesFile::path_from(&paths);
    let Ok(file) = AliasesFile::load_or_default(&path) else {
        return args;
    };
    for _ in 0..MAX_CHAIN {
        if args.len() < 2 {
            break;
        }
        let idx = first_command_token_index(&args);
        if idx >= args.len() {
            break;
        }
        let Some(key) = args[idx].to_str() else {
            break;
        };
        if key.starts_with('-') || key.contains('/') || key.contains('\\') {
            break;
        }
        let Some(target) = file.aliases.get(key) else {
            break;
        };
        let parts = split_alias_target(target);
        if parts.is_empty() {
            break;
        }
        args.remove(idx);
        for (i, p) in parts.into_iter().enumerate() {
            args.insert(idx + i, OsString::from(p));
        }
    }
    args
}

/// Built-in argv rewrites applied by [`preprocess_cli_args`]; listed for `envr help shortcuts`.
pub const BUILTIN_ARGV_SHORTHANDS: &[(&str, &str)] = &[
    ("add <lang> <version>", "project add <lang>@<version>"),
    ("add <spec>", "project add <spec>"),
    ("diag", "diagnostics export"),
    ("dx", "diagnostics export"),
    ("ci", "cache index sync"),
    ("cis", "cache index status"),
];

/// Expand argv shorthands before clap parsing (e.g. `diag` → `diagnostics export`).
///
/// Applies to the first command-like token (after global flags), same rule as
/// [`expand_user_cli_aliases`].
pub fn preprocess_cli_args(mut args: Vec<OsString>) -> Vec<OsString> {
    if args.is_empty() {
        return args;
    }
    let idx = first_command_token_index(&args);
    if idx >= args.len() {
        return args;
    }
    let key = args[idx].to_string_lossy().to_ascii_lowercase();
    // `envr add node 20` → `envr project add node@20` (shorthand for frequent pins)
    if key == "add" {
        if args.len() <= idx + 1 {
            args.remove(idx);
            args.insert(idx, OsString::from("project"));
            args.insert(idx + 1, OsString::from("add"));
            return args;
        }
        let t1 = args[idx + 1].to_string_lossy();
        if t1.starts_with('-') {
            args.remove(idx);
            args.insert(idx, OsString::from("project"));
            args.insert(idx + 1, OsString::from("add"));
            return args;
        }
        if args.len() > idx + 2 {
            let t2 = args[idx + 2].to_string_lossy();
            if !t2.starts_with('-') {
                let spec = format!("{t1}@{t2}");
                args.remove(idx + 2);
                args.remove(idx + 1);
                args.remove(idx);
                args.insert(idx, OsString::from("project"));
                args.insert(idx + 1, OsString::from("add"));
                args.insert(idx + 2, OsString::from(spec));
                return args;
            }
        }
        args.remove(idx);
        args.insert(idx, OsString::from("project"));
        args.insert(idx + 1, OsString::from("add"));
        return args;
    }
    let rep: Option<&'static [&'static str]> = match key.as_str() {
        "diag" | "dx" => Some(&["diagnostics", "export"]),
        "ci" => Some(&["cache", "index", "sync"]),
        "cis" => Some(&["cache", "index", "status"]),
        _ => None,
    };
    let Some(rep) = rep else {
        return args;
    };
    args.remove(idx);
    for (i, p) in rep.iter().enumerate() {
        args.insert(idx + i, OsString::from(*p));
    }
    args
}

pub fn run(cli: Cli) -> i32 {
    let global = cli.global.clone();
    crate::commands::dispatch(cli).finish(&global)
}

#[cfg(test)]
mod command_trace_tests {
    use super::{Cli, Command, HookCmd, Parser};

    #[test]
    fn trace_name_matches_subcommand() {
        let cli = Cli::try_parse_from(["envr", "doctor"]).expect("parse");
        assert_eq!(cli.command.trace_name(), "doctor");
        let cli = Cli::try_parse_from(["envr", "config", "path"]).expect("parse");
        assert_eq!(cli.command.trace_name(), "config_path");
        let cli = Cli::try_parse_from(["envr", "hook", "bash"]).expect("parse");
        assert!(matches!(cli.command, Command::Hook(HookCmd::Bash)));
        assert_eq!(cli.command.trace_name(), "hook_bash");
    }
}

#[cfg(test)]
mod output_format_resolution_tests {
    use super::{Cli, OutputFormat, Parser};

    #[test]
    fn resolved_output_format_doctor_json_shorthand() {
        let cli = Cli::try_parse_from(["envr", "doctor", "--json"]).expect("parse");
        assert_eq!(cli.resolved_output_format(), OutputFormat::Json);
    }

    #[test]
    fn resolved_output_format_global_json() {
        let cli = Cli::try_parse_from(["envr", "--format", "json", "doctor"]).expect("parse");
        assert_eq!(cli.resolved_output_format(), OutputFormat::Json);
    }

    #[test]
    fn resolved_output_format_doctor_default_text() {
        let cli = Cli::try_parse_from(["envr", "doctor"]).expect("parse");
        assert_eq!(cli.resolved_output_format(), OutputFormat::Text);
    }

    #[test]
    fn legacy_json_overrides_global_format_text_for_doctor() {
        let cli =
            Cli::try_parse_from(["envr", "--format", "text", "doctor", "--json"]).expect("parse");
        assert_eq!(cli.resolved_output_format(), OutputFormat::Json);
    }

    #[test]
    fn cloned_with_legacy_json_sets_json() {
        let cli = Cli::try_parse_from(["envr", "doctor", "--json"]).expect("parse");
        let g = cli.global.cloned_with_legacy_json(true);
        assert_eq!(g.effective_output_format(), OutputFormat::Json);
        let g2 = cli.global.cloned_with_legacy_json(false);
        assert_eq!(g2.effective_output_format(), OutputFormat::Text);
    }
}

#[cfg(test)]
mod preprocess_tests {
    use super::{first_command_token_index, preprocess_cli_args, split_alias_target};
    use std::ffi::OsString;

    #[test]
    fn diag_expands_to_diagnostics_export() {
        let out = preprocess_cli_args(vec![
            OsString::from("envr"),
            OsString::from("diag"),
            OsString::from("--help"),
        ]);
        assert_eq!(
            out,
            vec![
                OsString::from("envr"),
                OsString::from("diagnostics"),
                OsString::from("export"),
                OsString::from("--help"),
            ]
        );
    }

    #[test]
    fn ci_expands_to_cache_index_sync() {
        let out = preprocess_cli_args(vec![
            OsString::from("envr"),
            OsString::from("ci"),
            OsString::from("node"),
        ]);
        assert_eq!(
            out,
            vec![
                OsString::from("envr"),
                OsString::from("cache"),
                OsString::from("index"),
                OsString::from("sync"),
                OsString::from("node"),
            ]
        );
    }

    #[test]
    fn cis_expands_to_cache_index_status() {
        let out = preprocess_cli_args(vec![OsString::from("envr"), OsString::from("cis")]);
        assert_eq!(
            out,
            vec![
                OsString::from("envr"),
                OsString::from("cache"),
                OsString::from("index"),
                OsString::from("status"),
            ]
        );
    }

    #[test]
    fn add_two_tokens_expands_to_project_add_at_spec() {
        let out = preprocess_cli_args(vec![
            OsString::from("envr"),
            OsString::from("add"),
            OsString::from("node"),
            OsString::from("20"),
        ]);
        assert_eq!(
            out,
            vec![
                OsString::from("envr"),
                OsString::from("project"),
                OsString::from("add"),
                OsString::from("node@20"),
            ]
        );
    }

    #[test]
    fn add_one_token_expands_to_project_add_passthrough_spec() {
        let out = preprocess_cli_args(vec![
            OsString::from("envr"),
            OsString::from("add"),
            OsString::from("python@3.12"),
        ]);
        assert_eq!(
            out,
            vec![
                OsString::from("envr"),
                OsString::from("project"),
                OsString::from("add"),
                OsString::from("python@3.12"),
            ]
        );
    }

    #[test]
    fn first_command_token_skips_known_globals() {
        let args = vec![
            OsString::from("envr"),
            OsString::from("--format"),
            OsString::from("json"),
            OsString::from("--quiet"),
            OsString::from("--runtime-root"),
            OsString::from("C:\\rt"),
            OsString::from("list"),
        ];
        assert_eq!(first_command_token_index(&args), 6);
    }

    #[test]
    fn first_command_token_after_double_dash() {
        let args = vec![
            OsString::from("envr"),
            OsString::from("--format"),
            OsString::from("json"),
            OsString::from("--"),
            OsString::from("diag"),
        ];
        assert_eq!(first_command_token_index(&args), 4);
    }

    #[test]
    fn diag_after_globals_expands() {
        let out = preprocess_cli_args(vec![
            OsString::from("envr"),
            OsString::from("--porcelain"),
            OsString::from("--format"),
            OsString::from("json"),
            OsString::from("diag"),
            OsString::from("--help"),
        ]);
        assert_eq!(
            out,
            vec![
                OsString::from("envr"),
                OsString::from("--porcelain"),
                OsString::from("--format"),
                OsString::from("json"),
                OsString::from("diagnostics"),
                OsString::from("export"),
                OsString::from("--help"),
            ]
        );
    }

    #[test]
    fn split_alias_target_respects_quotes() {
        assert_eq!(
            split_alias_target(r#"cache index sync node"#),
            vec!["cache", "index", "sync", "node"]
        );
        assert_eq!(
            split_alias_target(r#"diagnostics export --output "a b.zip""#),
            vec!["diagnostics", "export", "--output", "a b.zip"]
        );
    }
}
