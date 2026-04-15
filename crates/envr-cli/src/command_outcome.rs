//! Normalized CLI handler result before mapping to process exit code and user-visible errors.
//!
//! # Dispatch boundary (single entry)
//!
//! After parsing, every command path must produce a [`CommandOutcome`] via exactly one of:
//!
//! - [`CommandOutcome::from_result`] — handler returned [`envr_error::EnvrResult`]`<`[`CliExit`]`>` (includes
//!   `Ok(status)` after emitting stdout/stderr). [`CliExit::error_code`] carries the stable metrics token
//!   when the handler emitted a failure envelope (`emit_failure_envelope`, `emit_validation`, etc.);
//!   for success, use [`CliExit::ok`]. If `exit_code != 0` and `error_code` is [`None`], finish metrics
//!   use the fallback `nonzero_exit`.
//! - [`CommandOutcome::Err`] — only via [`CommandOutcome::from_result`] on `Err` (including
//!   [`crate::commands::common::with_runtime_service`], which ends in `from_result`).
//!
//! Do not construct [`CommandOutcome::Done`] manually outside this module; keep exit + metrics
//! wiring in one place.
//!
//! # Handler inventory
//!
//! | Style | Examples |
//! |-------|----------|
//! | `EnvrResult<CliExit>` + `CommandOutcome::from_result(..).finish(g)` inside handler | `resolve`, `which`, `check`, `run`, `exec`, … |
//! | `Ok(output::emit_validation(..))` / `Ok(output::emit_doctor(..))` | Missing-args and `doctor` paths return [`CliExit`] with explicit `error_code` |
//! | Runtime service → [`crate::commands::common::with_runtime_service`] → [`CommandOutcome::from_result`] | `install`, `list`, `doctor`, … |
//! | Thin wrapper only | [`finish_cli_cmd`] / [`crate::finish_cli_cmd`] |

use crate::cli::GlobalArgs;
use crate::commands::common;
use crate::presenter::CliPersona;
use envr_error::{EnvrError, EnvrResult};

/// Process exit status with an optional **business** metrics token (JSON envelope `code`, not [`ErrorCode`](envr_error::ErrorCode)).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliExit {
    pub exit_code: i32,
    pub error_code: Option<&'static str>,
}

impl CliExit {
    #[inline]
    pub fn ok() -> Self {
        Self {
            exit_code: 0,
            error_code: None,
        }
    }

    #[inline]
    pub fn failure(exit_code: i32, error_code: &'static str) -> Self {
        Self {
            exit_code,
            error_code: Some(error_code),
        }
    }
}

/// Result of a command body after business logic; user output may already be on stdout/stderr.
#[derive(Debug)]
pub enum CommandOutcome {
    /// Process exit code (may be non-zero when a failure envelope was already emitted).
    Done {
        exit_code: i32,
        error_code: Option<&'static str>,
    },
    /// Unhandled error: render with [`crate::output::emit_envr_error`].
    Err(EnvrError),
}

impl CommandOutcome {
    /// Handler returned an explicit exit + optional business metrics code.
    #[inline]
    pub fn from_cli_exit(status: CliExit) -> Self {
        CommandOutcome::Done {
            exit_code: status.exit_code,
            error_code: status.error_code,
        }
    }

    /// `Ok(status)` uses [`Self::from_cli_exit`]. `Err` becomes [`CommandOutcome::Err`];
    /// [`Self::finish`] renders it with [`crate::output::emit_envr_error`] and metrics use
    /// [`crate::output::error_code_token`] (no envelope `code`).
    #[inline]
    pub fn from_result(r: EnvrResult<CliExit>) -> Self {
        match r {
            Ok(status) => Self::from_cli_exit(status),
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
                error_code.or_else(|| crate::output::metrics_error_code_for_exit(exit_code)),
            ),
            CommandOutcome::Err(e) => {
                let code = crate::output::error_code_token(e.code());
                let exit = common::print_envr_error(g, e);
                (false, exit, Some(code))
            }
        };
        tracing::info!(
            target: "envr_cli_metrics",
            phase = "finish",
            output_mode = crate::output::output_mode_token(g.effective_output_format()),
            persona = CliPersona::from_env().token(),
            success = success,
            exit_code = exit_code,
            error_code = error_code.unwrap_or(""),
            "cli finish completed"
        );
        exit_code
    }
}

/// Maps [`EnvrResult<CliExit>`] to process exit code (same as [`CommandOutcome::from_result`] + [`CommandOutcome::finish`]).
#[inline]
pub fn finish_cli_cmd(g: &GlobalArgs, result: EnvrResult<CliExit>) -> i32 {
    CommandOutcome::from_result(result).finish(g)
}

#[cfg(test)]
mod tests {
    use super::{CliExit, CommandOutcome};
    use envr_error::EnvrError;

    #[test]
    fn from_cli_exit_zero_has_no_error_code() {
        let o = CommandOutcome::from_cli_exit(CliExit::ok());
        assert!(matches!(
            o,
            CommandOutcome::Done {
                exit_code: 0,
                error_code: None
            }
        ));
    }

    #[test]
    fn from_cli_exit_nonzero_keeps_explicit_code() {
        let o = CommandOutcome::from_cli_exit(CliExit::failure(1, crate::codes::err::CHILD_EXIT));
        match o {
            CommandOutcome::Done {
                exit_code,
                error_code,
            } => {
                assert_eq!(exit_code, 1);
                assert_eq!(error_code.as_deref(), Some(crate::codes::err::CHILD_EXIT));
            }
            CommandOutcome::Err(_) => panic!("expected Done"),
        }
    }

    #[test]
    fn from_result_ok_matches_from_cli_exit() {
        let from_res =
            CommandOutcome::from_result(Ok(CliExit::failure(1, crate::codes::err::VALIDATION)));
        let from_st = CommandOutcome::from_cli_exit(CliExit::failure(1, crate::codes::err::VALIDATION));
        assert!(matches!(
            (&from_res, &from_st),
            (
                CommandOutcome::Done {
                    exit_code: 1,
                    error_code: Some(ra),
                },
                CommandOutcome::Done {
                    exit_code: 1,
                    error_code: Some(rb),
                },
            ) if ra == rb && *ra == crate::codes::err::VALIDATION
        ));
    }

    #[test]
    fn from_result_err_is_not_done() {
        let o = CommandOutcome::from_result(Err(EnvrError::Validation("x".into())));
        assert!(matches!(o, CommandOutcome::Err(_)));
    }
}
