use super::dispatch_runtime::{dispatch_runtime_result, DispatchCtx};
use super::{project_cmd, prune};
use crate::cli::Command;
use crate::CommandOutcome;

pub(super) fn route(command: Command, ctx: &DispatchCtx<'_>) -> CommandOutcome {
    match command {
        Command::Project(sub) => dispatch_runtime_result(ctx, |g, service| {
            project_cmd::run_inner(g, service, sub)
        }),
        Command::Prune { lang, execute } => {
            dispatch_runtime_result(ctx, |g, service| prune::run_inner(g, service, lang, execute))
        }
        other => unreachable!(
            "project runtime route received unsupported command: {:?}",
            other
        ),
    }
}

