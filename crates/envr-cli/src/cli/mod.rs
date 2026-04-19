//! Command-line interface for `envr` (clap tree and global flags).
//!
//! Layout: `global` (`GlobalArgs`, shared arg structs), `command` (`Command`, `trace_name`),
//! `command_spec` (SSOT: trace, routing hints, help path, success JSON messages), `metadata`
//! (re-exports `command_spec` + `CommandMetadata` alias), and this module (`Cli`, argv preprocess, `run`).

mod command;
mod command_spec;
mod global;
pub(crate) mod help_registry;
mod metadata;

pub use command::{
    AliasCmd, BundleCmd, CacheCmd, CacheIndexCmd, CacheRuntimeCmd, Command, ConfigCmd,
    ConfigValueType, DebugCmd, DiagnosticsCmd, EnvShellKind, HelpCmd, HookCmd, ProfileCmd,
    ProjectCmd, RustCmd, ShimCmd,
};
pub use global::{ExecRunSharedArgs, GlobalArgs, OutputFormat, ProjectPathProfileArgs};

#[allow(unused_imports)]
pub(crate) use metadata::metadata_for_key;
#[allow(unused_imports)]
pub(crate) use metadata::{
    CommandCapabilities, CommandKey, ContractSurface, RuntimeHandlerGroup, command_specs,
};

#[cfg(test)]
#[allow(unused_imports)]
pub(crate) use metadata::{all_command_keys, metadata_registry_entries};

use crate::presenter::CliPersona;
use clap::{FromArgMatches, Parser};
use envr_config::aliases::AliasesFile;
use envr_platform::paths::current_platform_paths;
use std::ffi::OsString;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

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

/// Parsed CLI invocation after applying legacy shorthands and other dispatch-boundary normalization.
///
/// This keeps `dispatch` as the single boundary that decides effective globals, while preserving
/// `Cli` as the clap-derived argv model.
pub(crate) struct CliContext {
    pub global: GlobalArgs,
    pub command: Command,
    pub trace_name: &'static str,
    pub output_format: OutputFormat,
    pub legacy_json_applied: bool,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ParseMetricsEvent {
    pub wants_json: bool,
    pub quiet: bool,
    pub success: bool,
    pub exit_code: i32,
}

static PENDING_PARSE_METRICS: OnceLock<Mutex<Option<ParseMetricsEvent>>> = OnceLock::new();
static METRICS_INVOCATION_ID: OnceLock<String> = OnceLock::new();

fn pending_parse_metrics() -> &'static Mutex<Option<ParseMetricsEvent>> {
    PENDING_PARSE_METRICS.get_or_init(|| Mutex::new(None))
}

fn build_metrics_invocation_id() -> String {
    let pid = std::process::id();
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    format!("p{pid}-t{now_ms}")
}

pub(crate) fn metrics_invocation_id() -> &'static str {
    METRICS_INVOCATION_ID
        .get_or_init(build_metrics_invocation_id)
        .as_str()
}

fn now_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

impl Cli {
    /// Effective output format for this argv after global flags and known subcommand shorthands.
    ///
    /// Used by [`apply_global`] so `ENVR_OUTPUT_FORMAT` matches what handlers will use (e.g.
    /// `doctor --json` implies JSON the same way as `--format json`).
    #[inline]
    pub fn resolved_output_format(&self) -> OutputFormat {
        if self.command.legacy_json_shorthand() {
            OutputFormat::Json
        } else {
            self.global.effective_output_format()
        }
    }

    /// Effective globals after applying subcommand-local shorthands at dispatch boundary.
    #[inline]
    pub(crate) fn effective_global_args(&self) -> GlobalArgs {
        self.global
            .cloned_with_legacy_json(self.command.legacy_json_shorthand())
    }

    /// Convert the raw clap-derived argv model into a normalized context for dispatch.
    #[inline]
    pub(crate) fn into_context(self) -> CliContext {
        let Cli { global, command } = self;
        let legacy_json_applied = command.legacy_json_shorthand();
        let trace_name = command.trace_name();
        let global = global.cloned_with_legacy_json(legacy_json_applied);
        let output_format = global.effective_output_format();
        CliContext {
            global,
            command,
            trace_name,
            output_format,
            legacy_json_applied,
        }
    }
}

/// Parse argv into [`Cli`] after alias/shorthand preprocessing.
///
/// On clap failures (token parsing **or** model binding into [`Cli`]), this function returns
/// `Err(`[`CliExit`][crate::CliExit]`)` with the process exit code. After emitting a JSON parse
/// envelope, [`CliExit::error_code`] is `Some("argv_parse_error")`; in text mode it is [`None`].
///
/// - emits one JSON failure envelope to stdout when machine-readable mode is requested
///   ([`crate::output::emit_failure_envelope`] with code `argv_parse_error`);
/// - otherwise prints clap's formatted error to stderr (same family of message as `clap::Error::exit`).
///
/// Parse-phase metrics are recorded before emitting output; the process entrypoint should call
/// [`crate::flush_parse_metrics_on_early_exit`] when returning [`Err`].
pub fn parse_cli_from_argv(argv: Vec<OsString>) -> Result<Cli, crate::CliExit> {
    let (wants_json, quiet) = argv_requests_json_and_quiet(&argv);
    let matches = match crate::cli_help::localized_command().try_get_matches_from(argv) {
        Ok(m) => m,
        Err(e) => {
            let exit_code = e.exit_code();
            record_parse_metrics(ParseMetricsEvent {
                wants_json,
                quiet,
                success: false,
                exit_code,
            });
            if wants_json {
                return Err(emit_clap_error_as_json_envelope(&e, quiet));
            }
            let _ = e.print();
            return Err(crate::CliExit {
                exit_code,
                error_code: None,
            });
        }
    };
    match Cli::from_arg_matches(&matches) {
        Ok(cli) => {
            record_parse_metrics(ParseMetricsEvent {
                wants_json,
                quiet,
                success: true,
                exit_code: 0,
            });
            Ok(cli)
        }
        Err(e) => {
            let exit_code = e.exit_code();
            record_parse_metrics(ParseMetricsEvent {
                wants_json,
                quiet,
                success: false,
                exit_code,
            });
            if wants_json {
                return Err(emit_clap_error_as_json_envelope(&e, quiet));
            }
            let _ = e.print();
            Err(crate::CliExit {
                exit_code,
                error_code: None,
            })
        }
    }
}

/// Expand user aliases and built-in shorthands from process argv, then parse into [`Cli`].
pub fn parse_cli_from_env() -> Result<Cli, crate::CliExit> {
    let argv = expand_user_cli_aliases(std::env::args_os().collect());
    let argv = preprocess_cli_args(argv);
    parse_cli_from_argv(argv)
}

fn emit_clap_error_as_json_envelope(e: &clap::Error, quiet: bool) -> crate::CliExit {
    let exit_code = e.exit_code();
    let kind = format!("{:?}", e.kind());
    let g = GlobalArgs {
        output_format: Some(OutputFormat::Json),
        porcelain: false,
        quiet,
        no_color: false,
        debug: false,
        verbose: false,
        runtime_root: None,
    };
    let msg = envr_core::i18n::tr_key(
        "cli.err.argv_parse_error",
        "命令行参数解析失败。",
        "failed to parse command-line arguments.",
    );
    let details = e.to_string();
    let data = serde_json::json!({
        "source": "clap",
        "kind": kind,
        "error": details,
        "exit_code": exit_code
    });
    let diagnostics = vec![e.to_string()];
    crate::output::emit_failure_envelope(
        &g,
        crate::codes::err::ARGV_PARSE_ERROR,
        &msg,
        data,
        &diagnostics,
        exit_code,
    )
}

fn argv_requests_json_and_quiet(argv: &[OsString]) -> (bool, bool) {
    if argv.is_empty() {
        return (false, false);
    }
    let command_idx = first_command_token_index(argv);

    let mut wants_json = false;
    let mut quiet = false;
    let mut i = 1usize;
    while i < argv.len() {
        let s = argv[i].to_string_lossy();
        if s == "--" {
            break;
        }
        if s == "--quiet" {
            quiet = true;
        }
        if s == "--format" {
            if let Some(v) = argv.get(i + 1).map(|x| x.to_string_lossy())
                && v == "json"
            {
                wants_json = true;
            }
            i += 1;
        }
        if let Some(rest) = s.strip_prefix("--format=")
            && rest == "json"
        {
            wants_json = true;
        }
        i += 1;
    }
    if argv_requests_legacy_json_shorthand(argv, command_idx) {
        wants_json = true;
    }
    (wants_json, quiet)
}

fn argv_requests_legacy_json_shorthand(argv: &[OsString], command_idx: usize) -> bool {
    let Some((matched_path_len, legacy_flag)) =
        legacy_json_path_and_flag_for_argv(argv, command_idx)
    else {
        return false;
    };
    let mut i = command_idx.saturating_add(matched_path_len);
    while i < argv.len() {
        let token = argv[i].to_string_lossy();
        if token == "--" {
            break;
        }
        if token == legacy_flag {
            return true;
        }
        i += 1;
    }
    false
}

fn legacy_json_path_and_flag_for_argv(
    argv: &[OsString],
    command_idx: usize,
) -> Option<(usize, &'static str)> {
    if command_idx >= argv.len() {
        return None;
    }
    let lower_tokens: Vec<String> = argv[command_idx..]
        .iter()
        .map(|s| s.to_string_lossy().to_ascii_lowercase())
        .collect();
    for (_, spec) in command_specs() {
        let Some(flag) = spec.legacy_json_flag else {
            continue;
        };
        if spec.help_path.len() > lower_tokens.len() {
            continue;
        }
        let path_matches = spec
            .help_path
            .iter()
            .zip(lower_tokens.iter())
            .all(|(expected, actual)| actual == expected);
        if path_matches {
            return Some((spec.help_path.len(), flag));
        }
    }
    None
}

fn record_parse_metrics(event: ParseMetricsEvent) {
    if let Ok(mut slot) = pending_parse_metrics().lock() {
        *slot = Some(event);
    }
}

pub(crate) fn take_parse_metrics_event() -> Option<ParseMetricsEvent> {
    pending_parse_metrics().lock().ok()?.take()
}

pub fn emit_pending_parse_metrics() {
    let Some(event) = take_parse_metrics_event() else {
        return;
    };
    let output_mode = if event.wants_json { "json" } else { "text" };
    tracing::info!(
        target: "envr_cli_metrics",
        phase = "parse",
        invocation_id = metrics_invocation_id(),
        timestamp_ms = now_timestamp_ms(),
        output_mode = output_mode,
        persona = CliPersona::from_env().token(),
        quiet = event.quiet,
        success = event.success,
        exit_code = event.exit_code,
        error_code = if event.success { "" } else { "argv_parse_error" },
        "cli parse completed"
    );
}
/// Apply global flags to process-wide runtime configuration.
///
/// This function intentionally avoids mutating the process environment (and therefore avoids
/// `unsafe` in Rust 2024). Environment variables remain supported as **inputs** (e.g.
/// `ENVR_RUNTIME_ROOT`, `RUST_LOG`) when set by the user or parent process.
pub fn apply_global(cli: &Cli) {
    let args = cli.effective_global_args();
    if let Some(ref p) = args.runtime_root {
        let _ = envr_config::settings::set_process_runtime_root_override(p.trim().into());
    }
}

/// Split an `alias add` target string into argv tokens (whitespace; supports quoted words).
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
            return if i + 1 < args.len() {
                i + 1
            } else {
                args.len()
            };
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
            || lower == "--verbose"
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

/// Expand argv shorthands before clap parsing (e.g. `diag` 鈫?`diagnostics export`).
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
    // `envr add node 20` 鈫?`envr project add node@20` (shorthand for frequent pins)
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
    let (outcome, global) = crate::commands::dispatch(cli);
    outcome.finish(&global)
}

#[cfg(test)]
mod command_trace_tests {
    use super::{
        Cli, Command, CommandKey, ContractSurface, HookCmd, Parser, all_command_keys,
        command_specs, metadata_for_key, metadata_registry_entries,
    };
    use std::collections::HashSet;

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

    #[test]
    fn trace_name_is_unique_across_command_samples() {
        let samples: [&[&str]; 18] = [
            &["envr", "install", "node", "20.0.0"],
            &["envr", "use", "node", "20.0.0"],
            &["envr", "list"],
            &["envr", "current"],
            &["envr", "which"],
            &["envr", "remote"],
            &["envr", "resolve", "node"],
            &["envr", "run", "echo", "ok"],
            &["envr", "env"],
            &["envr", "shell"],
            &["envr", "project", "validate"],
            &["envr", "config", "show"],
            &["envr", "alias", "list"],
            &["envr", "cache", "index", "status"],
            &["envr", "bundle", "apply", "x.zip"],
            &["envr", "doctor"],
            &["envr", "debug", "info"],
            &["envr", "diagnostics", "export"],
        ];
        let mut seen = HashSet::new();
        for argv in samples {
            let cli = Cli::try_parse_from(argv).expect("parse");
            let trace = cli.command.trace_name();
            assert!(
                seen.insert(trace),
                "duplicate trace_name in sample set: {trace}"
            );
        }
    }

    #[test]
    fn metadata_runtime_and_legacy_json_flags_are_consistent() {
        let doctor = Cli::try_parse_from(["envr", "doctor", "--json"]).expect("parse");
        assert!(doctor.command.legacy_json_shorthand());
        assert!(doctor.command.runtime_handler_group().is_some());

        let status = Cli::try_parse_from(["envr", "status"]).expect("parse");
        assert!(!status.command.legacy_json_shorthand());
        assert!(status.command.runtime_handler_group().is_none());

        let remote = Cli::try_parse_from(["envr", "remote"]).expect("parse");
        assert!(remote.command.runtime_handler_group().is_some());
    }

    #[test]
    fn legacy_json_spec_support_matches_runtime_behavior_for_doctor() {
        let doctor_spec = command_specs()
            .iter()
            .find_map(|(key, spec)| (*key == CommandKey::Doctor).then_some(*spec))
            .expect("doctor command spec");
        assert_eq!(doctor_spec.legacy_json_flag, Some("--json"));

        let without = Cli::try_parse_from(["envr", "doctor"]).expect("parse");
        assert!(!without.command.legacy_json_shorthand());

        let with = Cli::try_parse_from(["envr", "doctor", "--json"]).expect("parse");
        assert!(with.command.legacy_json_shorthand());
    }

    #[test]
    fn command_registry_has_unique_trace_names() {
        let mut seen = HashSet::new();
        for key in all_command_keys() {
            let trace = metadata_for_key(key).trace_name;
            assert!(
                seen.insert(trace),
                "duplicate trace name in registry: {trace}"
            );
        }
    }

    #[test]
    fn command_registry_has_unique_keys_and_round_trips() {
        let mut seen = HashSet::new();
        for (key, expected) in metadata_registry_entries() {
            assert!(seen.insert(*key), "duplicate key in registry: {key:?}");
            let got = metadata_for_key(*key);
            assert_eq!(got.trace_name, expected.trace_name);
            assert_eq!(got.legacy_json_shorthand, expected.legacy_json_shorthand);
            assert_eq!(got.runtime_required, expected.runtime_required);
            assert_eq!(got.runtime_group, expected.runtime_group);
            assert_eq!(got.capabilities, expected.capabilities);
        }
    }

    #[test]
    fn command_key_mapping_round_trips_against_registry() {
        let samples: &[(CommandKey, &[&str])] = &[
            (CommandKey::Install, &["envr", "install", "node", "20.0.0"]),
            (CommandKey::Use, &["envr", "use", "node", "20.0.0"]),
            (CommandKey::List, &["envr", "list"]),
            (CommandKey::Current, &["envr", "current"]),
            (
                CommandKey::Uninstall,
                &["envr", "uninstall", "node", "20.0.0", "--dry-run", "-y"],
            ),
            (CommandKey::Which, &["envr", "which"]),
            (CommandKey::Remote, &["envr", "remote"]),
            (
                CommandKey::RustInstallManaged,
                &["envr", "rust", "install-managed"],
            ),
            (CommandKey::Why, &["envr", "why", "node"]),
            (CommandKey::Resolve, &["envr", "resolve", "node"]),
            (
                CommandKey::Exec,
                &["envr", "exec", "--lang", "node", "echo", "ok"],
            ),
            (CommandKey::Run, &["envr", "run", "echo", "ok"]),
            (CommandKey::Env, &["envr", "env"]),
            (CommandKey::Template, &["envr", "template", "Cargo.toml"]),
            (CommandKey::Shell, &["envr", "shell"]),
            (CommandKey::HookBash, &["envr", "hook", "bash"]),
            (CommandKey::HookZsh, &["envr", "hook", "zsh"]),
            (CommandKey::HookKeys, &["envr", "hook", "keys"]),
            (CommandKey::HookPrompt, &["envr", "hook", "prompt"]),
            (CommandKey::Prune, &["envr", "prune"]),
            (CommandKey::Init, &["envr", "init"]),
            (CommandKey::Check, &["envr", "check"]),
            (CommandKey::Status, &["envr", "status"]),
            (
                CommandKey::ProjectAdd,
                &["envr", "project", "add", "node@20"],
            ),
            (CommandKey::ProjectSync, &["envr", "project", "sync"]),
            (
                CommandKey::ProjectValidate,
                &["envr", "project", "validate"],
            ),
            (CommandKey::Import, &["envr", "import", "Cargo.toml"]),
            (CommandKey::Export, &["envr", "export"]),
            (CommandKey::ProfileList, &["envr", "profile", "list"]),
            (CommandKey::ProfileShow, &["envr", "profile", "show", "dev"]),
            (CommandKey::ConfigSchema, &["envr", "config", "schema"]),
            (CommandKey::ConfigValidate, &["envr", "config", "validate"]),
            (CommandKey::ConfigEdit, &["envr", "config", "edit"]),
            (CommandKey::ConfigPath, &["envr", "config", "path"]),
            (CommandKey::ConfigShow, &["envr", "config", "show"]),
            (CommandKey::ConfigKeys, &["envr", "config", "keys"]),
            (
                CommandKey::ConfigGet,
                &["envr", "config", "get", "mirror.mode"],
            ),
            (
                CommandKey::ConfigSet,
                &["envr", "config", "set", "mirror.mode", "auto"],
            ),
            (CommandKey::AliasList, &["envr", "alias", "list"]),
            (CommandKey::AliasAdd, &["envr", "alias", "add", "n", "node"]),
            (CommandKey::AliasRemove, &["envr", "alias", "remove", "n"]),
            (CommandKey::ShimSync, &["envr", "shim", "sync"]),
            (CommandKey::CacheClean, &["envr", "cache", "clean"]),
            (
                CommandKey::CacheIndexSync,
                &["envr", "cache", "index", "sync"],
            ),
            (
                CommandKey::CacheIndexStatus,
                &["envr", "cache", "index", "status"],
            ),
            (
                CommandKey::CacheRuntimeStatus,
                &["envr", "cache", "runtime", "status"],
            ),
            (CommandKey::BundleCreate, &["envr", "bundle", "create"]),
            (
                CommandKey::BundleApply,
                &["envr", "bundle", "apply", "x.zip"],
            ),
            (CommandKey::Doctor, &["envr", "doctor"]),
            (CommandKey::Deactivate, &["envr", "deactivate"]),
            (CommandKey::DebugInfo, &["envr", "debug", "info"]),
            (
                CommandKey::DiagnosticsExport,
                &["envr", "diagnostics", "export"],
            ),
            (CommandKey::Completion, &["envr", "completion", "bash"]),
            (CommandKey::HelpShortcuts, &["envr", "help", "shortcuts"]),
            (CommandKey::Update, &["envr", "update"]),
        ];

        assert_eq!(
            samples.len(),
            metadata_registry_entries().len(),
            "argv sample table must include exactly one entry per COMMAND_SPEC_REGISTRY row"
        );

        let mut seen = HashSet::new();
        for (expected, argv) in samples {
            let cli = Cli::try_parse_from(*argv).expect("parse");
            let key = cli.command.key();
            assert_eq!(key, *expected, "command key mismatch for argv={argv:?}");
            let m = metadata_for_key(key);
            assert!(!m.trace_name.is_empty());
            seen.insert(key);
        }

        let registry_keys: HashSet<CommandKey> = all_command_keys().collect();
        assert_eq!(registry_keys.len(), metadata_registry_entries().len());
        assert_eq!(
            seen, registry_keys,
            "sample set must cover all command keys in registry"
        );
    }

    #[test]
    fn runtime_handler_group_is_consistent_with_runtime_required() {
        for key in all_command_keys() {
            let m = metadata_for_key(key);
            assert_eq!(
                m.runtime_required,
                m.runtime_group.is_some(),
                "runtime_group must be Some(..) iff runtime_required=true for key={key:?}"
            );
        }
    }

    #[test]
    fn capabilities_offline_safe_never_claims_network() {
        for key in all_command_keys() {
            let c = metadata_for_key(key).capabilities;
            assert!(
                !(c.offline_safe && c.may_network),
                "offline_safe implies !may_network for key={key:?}"
            );
        }
    }

    #[test]
    fn porcelain_contract_surface_is_limited_to_expected_commands() {
        let allowed = ["list", "current", "which", "resolve"];
        for key in all_command_keys() {
            let m = metadata_for_key(key);
            let surf = m.capabilities.contract_surface;
            if matches!(surf, ContractSurface::Both) {
                assert!(
                    allowed.contains(&m.trace_name),
                    "unexpected porcelain contract surface for key={key:?} trace={}",
                    m.trace_name
                );
            }
        }
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

    #[test]
    fn legacy_json_shorthand_centralizes_subcommand_json_flags() {
        let cli = Cli::try_parse_from(["envr", "list"]).expect("parse");
        assert!(
            !cli.command.legacy_json_shorthand(),
            "list has no legacy --json"
        );
        let cli = Cli::try_parse_from(["envr", "doctor"]).expect("parse");
        assert!(!cli.command.legacy_json_shorthand());
        let cli = Cli::try_parse_from(["envr", "doctor", "--json"]).expect("parse");
        assert!(cli.command.legacy_json_shorthand());
        assert_eq!(cli.resolved_output_format(), OutputFormat::Json);
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

#[cfg(test)]
mod preparse_request_tests {
    use super::{argv_requests_json_and_quiet, command_specs, legacy_json_path_and_flag_for_argv};
    use std::ffi::OsString;

    fn os_args(xs: &[&str]) -> Vec<OsString> {
        xs.iter().map(OsString::from).collect()
    }

    #[test]
    fn detects_global_json_and_quiet() {
        let args = os_args(&["envr", "--quiet", "--format", "json", "list"]);
        let (json, quiet) = argv_requests_json_and_quiet(&args);
        assert!(json);
        assert!(quiet);
    }

    #[test]
    fn detects_doctor_legacy_json_shorthand() {
        let args = os_args(&["envr", "doctor", "--json"]);
        let (json, quiet) = argv_requests_json_and_quiet(&args);
        assert!(json);
        assert!(!quiet);
    }

    #[test]
    fn ignores_double_dash_tail_tokens() {
        let args = os_args(&["envr", "doctor", "--", "--json"]);
        let (json, _) = argv_requests_json_and_quiet(&args);
        assert!(!json);
    }

    #[test]
    fn resolves_legacy_json_path_and_flag_from_command_spec_registry() {
        let args = os_args(&["envr", "doctor", "--json"]);
        let matched = legacy_json_path_and_flag_for_argv(&args, 1);
        assert_eq!(matched, Some((1, "--json")));
    }

    #[test]
    fn every_spec_legacy_json_flag_is_detected_by_preparse_scan() {
        for (_, spec) in command_specs() {
            let Some(flag) = spec.legacy_json_flag else {
                continue;
            };
            let mut argv = vec![OsString::from("envr")];
            argv.extend(spec.help_path.iter().map(OsString::from));
            argv.push(OsString::from(flag));

            let (json, quiet) = argv_requests_json_and_quiet(&argv);
            assert!(
                json,
                "preparse scan must detect legacy json flag `{flag}` for command path {:?}",
                spec.help_path
            );
            assert!(!quiet);
        }
    }
}

#[cfg(test)]
mod argv_parse_stage_tests {
    use super::emit_clap_error_as_json_envelope;
    use crate::cli_help::localized_command;
    use std::ffi::OsString;

    #[test]
    fn emit_clap_error_json_envelope_sets_argv_parse_failure_metrics_code() {
        let cmd = localized_command();
        let err = cmd
            .try_get_matches_from(vec![
                OsString::from("envr"),
                OsString::from("--format"),
                OsString::from("json"),
                OsString::from("--not-a-real-flag"),
            ])
            .unwrap_err();
        let exit = err.exit_code();
        assert_ne!(exit, 0);
        let ret = emit_clap_error_as_json_envelope(&err, false);
        assert_eq!(ret.exit_code, exit);
        assert_eq!(ret.error_code.as_deref(), Some("argv_parse_error"));
    }

    #[test]
    fn parse_cli_from_argv_json_unknown_global_returns_err() {
        let argv = vec![
            OsString::from("envr"),
            OsString::from("--format"),
            OsString::from("json"),
            OsString::from("--bogus-global"),
        ];
        assert!(super::parse_cli_from_argv(argv).is_err());
    }
}
