use super::{current, install, list, uninstall, use_cmd};
use super::dispatch_runtime::{dispatch_runtime_result, DispatchCtx};
use crate::cli::Command;
use crate::CommandOutcome;

pub(super) fn route(command: Command, ctx: &DispatchCtx<'_>) -> CommandOutcome {
    match command {
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
        Command::List { runtime, outdated } => {
            dispatch_runtime_result(ctx, |g, service| list::run_inner(g, service, runtime, outdated))
        }
        Command::Current { runtime } => {
            dispatch_runtime_result(ctx, |g, service| current::run_inner(g, service, runtime))
        }
        Command::Uninstall {
            runtime,
            runtime_version,
            dry_run,
            force,
            yes,
        } => dispatch_runtime_result(ctx, |g, service| {
            uninstall::run_inner(g, service, runtime, runtime_version, dry_run, force, yes)
        }),
        other => unreachable!(
            "installation runtime route received unsupported command: {:?}",
            other
        ),
    }
}

