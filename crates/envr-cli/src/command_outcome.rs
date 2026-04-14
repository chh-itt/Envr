//! Normalized CLI handler result before mapping to process exit code and user-visible errors.
//!
//! # Dispatch boundary (single entry)
//!
//! After parsing, every command path must produce a [`CommandOutcome`] via exactly one of:
//!
//! - [`CommandOutcome::from_result`] — handler returned [`envr_error::EnvrResult`]`<i32>` (includes
//!   `Ok(n)` after emitting stdout/stderr). For `Ok` with non-zero `n`, metrics `error_code` comes from
//!   [`crate::output::take_recorded_failure_code_for_exit`] when the handler used
//!   [`crate::output::emit_failure_envelope`], [`crate::output::write_envelope`] (failure),
//!   [`crate::output::emit_validation`], etc.; otherwise the fallback token `nonzero_exit` applies.
//! - [`CommandOutcome::from_exit_code`] — handler returned a bare `i32` (e.g. shell completion,
//!   hook script emission). Same thread-local rule as `from_result(Ok(code))`.
//! - [`CommandOutcome::Err`] — only via [`CommandOutcome::from_result`] on `Err` (including
//!   [`crate::commands::common::with_runtime_service`], which ends in `from_result`).
//!
//! Do not construct [`CommandOutcome::Done`] manually outside this module; keep exit + metrics
//! wiring in one place.
//!
//! # Migration (Phase C)
//!
//! - [`finish_cli_cmd`] is a thin alias for `CommandOutcome::from_result(..).finish(g)` (re-exported
//!   from the crate root as [`crate::finish_cli_cmd`] for embedders).
//! - Most `EnvrResult<i32>` handlers call [`CommandOutcome::from_result`] + [`CommandOutcome::finish`]
//!   directly.
//! - [`crate::commands::dispatch`] returns [`CommandOutcome`] and [`crate::cli::GlobalArgs`]; [`crate::cli::run`]
//!   calls [`CommandOutcome::finish`] once at the boundary (no extra clone of globals).
//! - Individual handlers still return `i32` (or `EnvrResult<i32>` + `finish` inside the handler body).
//!
//! # Handler inventory
//!
//! | Style | Examples |
//! |-------|----------|
//! | `EnvrResult<i32>` + `CommandOutcome::from_result(..).finish(g)` inside handler | `resolve`, `which`, `check`, `run`, `exec`, … |
//! | `Ok(output::emit_validation(..))` / `Ok(output::emit_doctor(.., false, ..))` | Missing-args and `doctor` hard-fail paths record stable codes for metrics (same mechanism as [`crate::output::emit_failure_envelope`]; JSON `doctor` failures also go through [`crate::output::write_envelope`]) |
//! | `i32` at handler boundary → [`CommandOutcome::from_exit_code`] in dispatch | `completion`, hook script emit |
//! | Runtime service → [`crate::commands::common::with_runtime_service`] → [`CommandOutcome::from_result`] | `install`, `list`, `doctor`, … |
//! | Thin wrapper only | [`finish_cli_cmd`] / [`crate::finish_cli_cmd`] |

use crate::cli::GlobalArgs;
use crate::commands::common;
use envr_error::{EnvrError, EnvrResult};

/// Result of a command body after business logic; user output may already be on stdout/stderr.
#[derive(Debug)]
pub enum CommandOutcome {
    /// Process exit code (may be non-zero when a failure envelope was already emitted).
    Done {
        exit_code: i32,
        error_code: Option<String>,
    },
    /// Unhandled error: render with [`crate::output::emit_envr_error`].
    Err(EnvrError),
}

impl CommandOutcome {
    /// Wrap a process exit code after the handler has written any user-visible output.
    ///
    /// Pairs with [`crate::output::take_recorded_failure_code_for_exit`]: stable failure `code`s
    /// emitted through [`crate::output::emit_failure_envelope`], [`crate::output::write_envelope`],
    /// etc. are picked up here for dispatch/finish metrics.
    #[inline]
    pub fn from_exit_code(exit_code: i32) -> Self {
        CommandOutcome::Done {
            exit_code,
            error_code: crate::output::take_recorded_failure_code_for_exit(exit_code),
        }
    }

    /// `Ok(code)` is equivalent to [`Self::from_exit_code`]. `Err` becomes [`CommandOutcome::Err`];
    /// [`Self::finish`] renders it with [`crate::output::emit_envr_error`] and metrics use
    /// [`crate::output::error_code_token`] (no thread-local recording).
    #[inline]
    pub fn from_result(r: EnvrResult<i32>) -> Self {
        match r {
            Ok(code) => Self::from_exit_code(code),
            Err(e) => CommandOutcome::Err(e),
        }
    }

    #[inline]
    pub fn finish(self, g: &GlobalArgs) -> i32 {
        let (success, exit_code, error_code) = match self {
            CommandOutcome::Done {
                exit_code,
                error_code,
            } => (
                exit_code == 0,
                exit_code,
                error_code
                    .or_else(|| crate::output::metrics_error_code_for_exit(exit_code).map(str::to_string)),
            ),
            CommandOutcome::Err(e) => {
                let code = crate::output::error_code_token(e.code());
                let exit = common::print_envr_error(g, e);
                (false, exit, Some(code.to_string()))
            }
        };
        tracing::info!(
            target: "envr_cli_metrics",
            phase = "finish",
            output_mode = crate::output::output_mode_token(g.effective_output_format()),
            success = success,
            exit_code = exit_code,
            error_code = error_code.as_deref().unwrap_or(""),
            "cli finish completed"
        );
        exit_code
    }
}

/// Maps [`EnvrResult`]`<i32>` to process exit code (same as [`CommandOutcome::from_result`] + [`CommandOutcome::finish`]).
#[inline]
pub fn finish_cli_cmd(g: &GlobalArgs, result: EnvrResult<i32>) -> i32 {
    CommandOutcome::from_result(result).finish(g)
}

#[cfg(test)]
mod tests {
    use super::CommandOutcome;
    use envr_error::EnvrError;

    #[test]
    fn from_exit_code_zero_clears_recorded_metrics_code() {
        crate::output::record_failure_code_for_metrics("should_not_leak");
        let o = CommandOutcome::from_exit_code(0);
        assert!(matches!(
            o,
            CommandOutcome::Done {
                exit_code: 0,
                error_code: None
            }
        ));
    }

    #[test]
    fn from_exit_code_nonzero_picks_up_recorded_failure_code() {
        crate::output::record_failure_code_for_metrics("child_exit");
        let o = CommandOutcome::from_exit_code(1);
        match o {
            CommandOutcome::Done {
                exit_code,
                error_code,
            } => {
                assert_eq!(exit_code, 1);
                assert_eq!(error_code.as_deref(), Some("child_exit"));
            }
            CommandOutcome::Err(_) => panic!("expected Done"),
        }
    }

    #[test]
    fn from_result_ok_matches_from_exit_code() {
        crate::output::record_failure_code_for_metrics("validation");
        let from_res = CommandOutcome::from_result(Ok(1));
        crate::output::record_failure_code_for_metrics("validation");
        let from_code = CommandOutcome::from_exit_code(1);
        assert!(matches!(
            (&from_res, &from_code),
            (
                CommandOutcome::Done {
                    exit_code: 1,
                    error_code: Some(ra),
                },
                CommandOutcome::Done {
                    exit_code: 1,
                    error_code: Some(rb),
                },
            ) if ra == rb && ra == "validation"
        ));
    }

    #[test]
    fn from_result_err_is_not_done() {
        let o = CommandOutcome::from_result(Err(EnvrError::Validation("x".into())));
        assert!(matches!(o, CommandOutcome::Err(_)));
    }
}
