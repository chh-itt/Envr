use super::diagnostics;
use super::dispatch_macros::dispatch_match;
use super::dispatch_runtime::{DispatchCtx, dispatch_runtime_result};
use super::{doctor, remote};
use crate::CommandOutcome;
use crate::cli::Command;

pub(super) fn route(command: Command, ctx: &DispatchCtx<'_>) -> CommandOutcome {
    dispatch_match!(
        command,
        _ => unreachable!(
            "misc runtime route received unsupported command: {:?}",
            command
        );
        Command::Doctor {
            fix,
            fix_path,
            fix_path_apply,
            json: _,
        } => dispatch_runtime_result(ctx, |g, service| {
            doctor::run_inner(g, service, fix, fix_path, fix_path_apply)
        }),
        Command::Remote {
            runtime,
            prefix,
            update,
        } => dispatch_runtime_result(ctx, |g, service| {
            remote::run_inner(g, service, runtime, prefix, update)
        }),
        Command::Diagnostics(sub) => dispatch_runtime_result(ctx, |g, service| match sub {
            crate::cli::DiagnosticsCmd::Export { output } => {
                diagnostics::export_zip_inner(g, service, output)
            }
        }),
    )
}
