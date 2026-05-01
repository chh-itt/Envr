//! Nested clap subcommand enums used by [`super::Command`].

use crate::cli::global::ProjectPathProfileArgs;

use clap::{Subcommand, ValueEnum};
use std::path::PathBuf;

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
    /// Inspect runtime remote/unified cache files under `{runtime_root}/cache`
    #[command(subcommand)]
    Runtime(CacheRuntimeCmd),
}

#[derive(Subcommand, Debug)]
pub enum CacheRuntimeCmd {
    /// Show per-runtime remote/unified cache status (unified list + provider snapshots)
    Status {
        /// Limit to one runtime (e.g. `zig`, `node`). Default: all.
        #[arg(value_name = "RUNTIME")]
        runtime: Option<String>,
        /// Use all runtimes (same as omitting RUNTIME).
        #[arg(long)]
        all: bool,
    },
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
    /// powershell: add to your PowerShell profile as `Invoke-Expression (& envr hook powershell)`
    Powershell,
    /// Print whether a shell hook is currently active and which profile files are relevant
    Status {
        /// Profile directory to inspect (defaults to current directory)
        #[arg(long, value_name = "DIR", default_value = ".")]
        path: PathBuf,
    },
    /// Print hook diagnostics and next-step guidance for the selected shell
    Doctor {
        /// Shell to diagnose
        #[arg(value_enum)]
        shell: HookShell,
        /// Profile directory to inspect (defaults to current directory)
        #[arg(long, value_name = "DIR", default_value = ".")]
        path: PathBuf,
    },
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

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum HookShell {
    Bash,
    Zsh,
    Powershell,
}

#[derive(Subcommand, Debug)]
pub enum DebugCmd {
    /// Print config paths, `ENVR_*` env, and a short runtime-root directory listing (for issues)
    Info,
}

#[derive(Subcommand, Debug)]
pub enum ToolCmd {
    /// List managed tool placeholders available to envr (read-only discovery)
    List,
    /// Show where a managed tool would resolve from (currently runtime-key based)
    Which {
        #[arg(value_name = "NAME")]
        name: String,
    },
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
    /// Built-in argv expansions before clap (`add`, `diag`, …).
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
    /// Read or write the lockfile snapshot for the project config
    Lock {
        #[arg(long, value_name = "DIR", default_value = ".")]
        path: PathBuf,
        /// Print the lockfile content instead of writing it
        #[arg(long)]
        dry_run: bool,
    },
    /// Ensure pinned runtimes exist; with `--install`, download missing pins
    Sync {
        #[arg(long, value_name = "DIR", default_value = ".")]
        path: PathBuf,
        #[arg(long)]
        install: bool,
        /// Respect `.envr.lock.toml` and refuse to install when it is stale
        #[arg(long)]
        locked: bool,
    },
    /// Verify pins resolve under the runtime root; optional remote index check
    Validate {
        #[arg(long, value_name = "DIR", default_value = ".")]
        path: PathBuf,
        /// Compare pins against remote indexes (may use network / local cache)
        #[arg(long)]
        check_remote: bool,
        /// Respect `.envr.lock.toml` and verify it matches `.envr.toml`
        #[arg(long)]
        locked: bool,
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
