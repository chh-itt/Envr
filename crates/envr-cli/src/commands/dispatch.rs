//! Top-level command routing (keeps [`crate::commands`] `mod.rs` as module declarations only).

use super::{dispatch_non_runtime, dispatch_runtime};

use crate::cli::{CliContext, GlobalArgs};
use crate::CommandOutcome;
use std::time::{Duration, Instant};

/// Route a parsed [`crate::cli::Cli`] to the appropriate handler.
///
/// Returns [`CommandOutcome`] and the parsed [`GlobalArgs`] so the caller can call
/// [`CommandOutcome::finish`] once without cloning globals (see [`crate::cli::run`]).
pub fn dispatch(cli: crate::cli::Cli) -> (CommandOutcome, GlobalArgs) {
    let ctx = cli.into_context();
    dispatch_ctx(ctx)
}

fn dispatch_ctx(ctx: CliContext) -> (CommandOutcome, GlobalArgs) {
    let CliContext {
        global,
        command,
        trace_name,
        output_format,
        legacy_json_applied,
    } = ctx;
    let dctx = DispatchCtx { global: &global };
    let span = tracing::info_span!(
        "envr.cli.command",
        command = trace_name,
        output_format = ?output_format,
        legacy_json_applied = legacy_json_applied,
        may_network = command.capabilities().may_network
    );
    let _guard = span.enter();
    let started_at = Instant::now();
    let outcome = match command.runtime_handler_group() {
        Some(_) => dispatch_runtime::route(command, &dctx),
        None => dispatch_non_runtime::route(command, &global),
    };
    emit_dispatch_metrics(trace_name, output_format, &outcome, started_at.elapsed());
    (outcome, global)
}

pub(super) type DispatchCtx<'a> = dispatch_runtime::DispatchCtx<'a>;

fn emit_dispatch_metrics(
    command: &str,
    output_format: crate::cli::OutputFormat,
    outcome: &CommandOutcome,
    elapsed: Duration,
) {
    let (success, exit_code, error_code) = dispatch_metrics_fields(outcome);
    tracing::info!(
        target: "envr_cli_metrics",
        phase = "dispatch",
        command = command,
        output_mode = crate::output::output_mode_token(output_format),
        success = success,
        exit_code = exit_code,
        error_code = error_code.unwrap_or(""),
        elapsed_ms = elapsed.as_millis() as u64,
        "cli dispatch finished"
    );
}

fn dispatch_metrics_fields(outcome: &CommandOutcome) -> (bool, i32, Option<&str>) {
    match outcome {
        CommandOutcome::Done {
            exit_code,
            error_code,
        } => (
            *exit_code == 0,
            *exit_code,
            error_code
                .as_deref()
                .or_else(|| crate::output::metrics_error_code_for_exit(*exit_code)),
        ),
        CommandOutcome::Err(err) => (
            false,
            crate::output::exit_code_for_error(err),
            Some(crate::output::error_code_token(err.code())),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::dispatch_metrics_fields;
    use crate::cli::{self, Command, RuntimeHandlerGroup, metadata_for_key};
    use crate::CommandOutcome;
    use envr_error::EnvrError;
    use std::ffi::OsString;

    #[test]
    fn metrics_fields_for_done_and_err() {
        assert_eq!(
            dispatch_metrics_fields(&CommandOutcome::Done {
                exit_code: 0,
                error_code: None
            }),
            (true, 0, None)
        );
        assert_eq!(
            dispatch_metrics_fields(&CommandOutcome::Done {
                exit_code: 3,
                error_code: None
            }),
            (false, 3, Some("nonzero_exit"))
        );
        assert_eq!(
            dispatch_metrics_fields(&CommandOutcome::Done {
                exit_code: 1,
                error_code: Some(crate::codes::err::PROJECT_CHECK_FAILED)
            }),
            (false, 1, Some(crate::codes::err::PROJECT_CHECK_FAILED))
        );

        let err = CommandOutcome::Err(EnvrError::Validation("bad".into()));
        let (success, exit, code) = dispatch_metrics_fields(&err);
        assert!(!success);
        assert_eq!(exit, 1);
        assert_eq!(code, Some("validation"));
    }

    #[test]
    fn metrics_fields_use_explicit_business_failure_code_over_fallback() {
        use crate::CliExit;
        let out = CommandOutcome::from_result(Ok(CliExit::failure(
            1,
            crate::codes::err::PROJECT_CHECK_FAILED,
        )));
        assert_eq!(
            dispatch_metrics_fields(&out),
            (false, 1, Some(crate::codes::err::PROJECT_CHECK_FAILED))
        );
    }

    #[test]
    fn runtime_classifier_matches_runtime_group_presence() {
        let runtime_argv: [&[&str]; 10] = [
            &["envr", "install", "node", "20.0.0"],
            &["envr", "use", "node", "20.0.0"],
            &["envr", "list"],
            &["envr", "current"],
            &["envr", "uninstall", "node", "20.0.0", "--dry-run", "-y"],
            &["envr", "remote"],
            &["envr", "project", "validate"],
            &["envr", "prune"],
            &["envr", "doctor"],
            &["envr", "diagnostics", "export"],
        ];
        for argv in runtime_argv {
            let (cmd, _) = parse_command_and_global(argv);
            assert!(cmd.runtime_handler_group().is_some());
        }
    }

    #[test]
    fn runtime_classifier_rejects_commands_without_runtime_group() {
        let non_runtime_argv: [&[&str]; 27] = [
            &["envr", "completion", "bash"],
            &["envr", "help", "shortcuts"],
            &["envr", "init", "--path", "."],
            &["envr", "check"],
            &["envr", "status"],
            &["envr", "why", "node"],
            &["envr", "resolve", "node"],
            &["envr", "exec", "--lang", "node", "echo", "ok"],
            &["envr", "run", "echo", "ok"],
            &["envr", "env"],
            &["envr", "template", "Cargo.toml"],
            &["envr", "shell"],
            &["envr", "hook", "prompt"],
            &["envr", "import", "Cargo.toml"],
            &["envr", "export"],
            &["envr", "profile", "list"],
            &["envr", "config", "schema"],
            &["envr", "alias", "list"],
            &["envr", "update"],
            &["envr", "shim", "sync"],
            &["envr", "cache", "index", "status"],
            &["envr", "bundle", "apply", "x.zip"],
            &["envr", "rust", "install-managed"],
            &["envr", "deactivate"],
            &["envr", "debug", "info"],
            &["envr", "which"],
            &["envr", "hook", "keys"],
        ];
        for argv in non_runtime_argv {
            let (cmd, _) = parse_command_and_global(argv);
            assert!(cmd.runtime_handler_group().is_none());
        }
    }

    #[test]
    fn runtime_group_values_match_expected_subsystems() {
        let installation = parse_command_and_global(&["envr", "install", "node", "20.0.0"]).0;
        assert_eq!(
            installation.runtime_handler_group(),
            Some(RuntimeHandlerGroup::Installation)
        );

        let project = parse_command_and_global(&["envr", "project", "validate"]).0;
        assert_eq!(project.runtime_handler_group(), Some(RuntimeHandlerGroup::Project));

        let misc = parse_command_and_global(&["envr", "doctor"]).0;
        assert_eq!(misc.runtime_handler_group(), Some(RuntimeHandlerGroup::Misc));

        let non_runtime = parse_command_and_global(&["envr", "status"]).0;
        assert_eq!(non_runtime.runtime_handler_group(), None);
    }

    #[test]
    fn runtime_group_presence_is_runtime_classifier() {
        let samples: [&[&str]; 6] = [
            &["envr", "doctor"],
            &["envr", "remote"],
            &["envr", "prune"],
            &["envr", "status"],
            &["envr", "which"],
            &["envr", "help", "shortcuts"],
        ];
        for argv in samples {
            let (cmd, _) = parse_command_and_global(argv);
            assert_eq!(
                metadata_for_key(cmd.key()).runtime_required,
                cmd.runtime_handler_group().is_some(),
                "runtime_required and runtime_group must stay aligned for argv={argv:?}"
            );
        }
    }

    fn parse_command_and_global(argv: &[&str]) -> (Command, cli::GlobalArgs) {
        let argv: Vec<OsString> = argv.iter().map(OsString::from).collect();
        let parsed = cli::parse_cli_from_argv(argv).expect("parse argv");
        (parsed.command, parsed.global)
    }
}
