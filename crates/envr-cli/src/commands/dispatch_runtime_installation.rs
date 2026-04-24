use super::dispatch_runtime::{DispatchCtx, dispatch_runtime_result};
use super::dispatch_macros::dispatch_match;
use super::{current, install, list, uninstall, use_cmd};
use crate::CommandOutcome;
use crate::cli::Command;

pub(super) fn route(command: Command, ctx: &DispatchCtx<'_>) -> CommandOutcome {
    dispatch_match!(
        command,
        _ => unreachable!(
            "installation runtime route received unsupported command: {:?}",
            command
        );
        Command::Install {
            runtime,
            runtime_version,
        } => dispatch_runtime_result(ctx, |g, service| {
            install::run_inner(g, service, runtime, runtime_version)
        }),
        Command::Use {
            runtime,
            runtime_version,
        } => dispatch_runtime_result(ctx, |g, service| {
            use_cmd::run_inner(g, service, runtime, runtime_version)
        }),
        Command::List { runtime, outdated } => dispatch_runtime_result(ctx, |g, service| {
            list::run_inner(g, service, runtime, outdated)
        }),
        Command::Current { runtime } => {
            dispatch_runtime_result(ctx, |g, service| current::run_inner(g, service, runtime))
        },
        Command::Uninstall {
            runtime,
            runtime_version,
            dry_run,
            force,
            yes,
        } => dispatch_runtime_result(ctx, |g, service| {
            uninstall::run_inner(g, service, runtime, runtime_version, dry_run, force, yes)
        }),
    )
}
