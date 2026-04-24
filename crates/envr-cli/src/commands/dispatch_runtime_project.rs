use super::dispatch_macros::dispatch_match;
use super::dispatch_runtime::{DispatchCtx, dispatch_runtime_result};
use super::{project_cmd, prune};
use crate::CommandOutcome;
use crate::cli::Command;

pub(super) fn route(command: Command, ctx: &DispatchCtx<'_>) -> CommandOutcome {
    dispatch_match!(
        command,
        _ => unreachable!(
            "project runtime route received unsupported command: {:?}",
            command
        );
        Command::Project(sub) => {
            dispatch_runtime_result(ctx, |g, service| project_cmd::run_inner(g, service, sub))
        },
        Command::Prune { lang, execute } => dispatch_runtime_result(ctx, |g, service| {
            prune::run_inner(g, service, lang, execute)
        }),
    )
}
