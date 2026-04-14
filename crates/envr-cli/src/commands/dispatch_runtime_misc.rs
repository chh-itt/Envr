use super::diagnostics;
use super::dispatch_runtime::{dispatch_runtime_result, DispatchCtx};
use super::{doctor, remote};
use crate::cli::Command;
use crate::CommandOutcome;

pub(super) fn route(command: Command, ctx: &DispatchCtx<'_>) -> CommandOutcome {
    match command {
        Command::Doctor {
            fix,
            fix_path,
            fix_path_apply,
            json: _,
        } => dispatch_runtime_result(ctx, |g, service| {
            doctor::run_inner(g, service, fix, fix_path, fix_path_apply)
        }),
        Command::Remote { runtime, prefix } => {
            dispatch_runtime_result(ctx, |g, service| remote::run_inner(g, service, runtime, prefix))
        }
        Command::Diagnostics(sub) => dispatch_runtime_result(ctx, |g, service| match sub {
            crate::cli::DiagnosticsCmd::Export { output } => {
                diagnostics::export_zip_inner(g, service, output)
            }
        }),
        other => unreachable!(
            "misc runtime route received unsupported command: {:?}",
            other
        ),
    }
}

