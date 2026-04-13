//! Normalized CLI handler result before mapping to process exit code and user-visible errors.
//!
//! # Migration (Phase C)
//!
//! - [`finish_cli_cmd`] is a thin alias for `CommandOutcome::from_result(..).finish(g)` (re-exported
//!   from the crate root as [`crate::finish_cli_cmd`] for embedders).
//! - Most `EnvrResult<i32>` handlers call [`CommandOutcome::from_result`] + [`CommandOutcome::finish`]
//!   directly.
//! - [`crate::commands::dispatch`] returns [`CommandOutcome`] for every subcommand; [`crate::cli::run`]
//!   calls [`CommandOutcome::finish`] once at the boundary.
//! - Individual handlers still return `i32` (or `EnvrResult<i32>` + `finish` inside the handler body).
//!
//! # Handler inventory
//!
//! | Style | Examples |
//! |-------|----------|
//! | `EnvrResult<i32>` + `CommandOutcome::from_result(..).finish(g)` inside handler | `resolve`, `which`, `check`, `run`, `exec`, … |
//! | `i32` at handler boundary, wrapped as [`CommandOutcome::Done`] in dispatch | `completion`, `help`, runtime commands via [`crate::commands::common::with_runtime_service`] |
//! | Thin wrapper only | [`finish_cli_cmd`] / [`crate::finish_cli_cmd`] |

use crate::cli::GlobalArgs;
use crate::commands::common;
use envr_error::{EnvrError, EnvrResult};

/// Result of a command body after business logic; user output may already be on stdout/stderr.
#[derive(Debug)]
pub enum CommandOutcome {
    /// Process exit code (may be non-zero when a failure envelope was already emitted).
    Done(i32),
    /// Unhandled error: render with [`crate::output::emit_envr_error`].
    Err(EnvrError),
}

impl CommandOutcome {
    #[inline]
    pub fn from_result(r: EnvrResult<i32>) -> Self {
        match r {
            Ok(code) => CommandOutcome::Done(code),
            Err(e) => CommandOutcome::Err(e),
        }
    }

    #[inline]
    pub fn finish(self, g: &GlobalArgs) -> i32 {
        match self {
            CommandOutcome::Done(code) => code,
            CommandOutcome::Err(e) => common::print_envr_error(g, e),
        }
    }
}

/// Maps [`EnvrResult`]`<i32>` to process exit code (same as [`CommandOutcome::from_result`] + [`CommandOutcome::finish`]).
#[inline]
pub fn finish_cli_cmd(g: &GlobalArgs, result: EnvrResult<i32>) -> i32 {
    CommandOutcome::from_result(result).finish(g)
}
