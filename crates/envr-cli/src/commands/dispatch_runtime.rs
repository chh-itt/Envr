use crate::CommandOutcome;
use crate::cli::{Command, GlobalArgs, RuntimeHandlerGroup};
use crate::command_outcome::CliExit;
use envr_core::runtime::service::RuntimeService;

pub(super) struct DispatchCtx<'a> {
    pub(super) global: &'a GlobalArgs,
}

/// Route commands that require a live runtime service.
pub(super) fn route(command: Command, ctx: &DispatchCtx<'_>) -> CommandOutcome {
    debug_assert!(command.runtime_handler_group().is_some());
    match command.runtime_handler_group() {
        Some(RuntimeHandlerGroup::Installation) => {
            super::dispatch_runtime_installation::route(command, ctx)
        }
        Some(RuntimeHandlerGroup::Project) => super::dispatch_runtime_project::route(command, ctx),
        Some(RuntimeHandlerGroup::Misc) => super::dispatch_runtime_misc::route(command, ctx),
        None => unreachable!(
            "runtime dispatch received non-runtime command key: {:?}",
            command.key()
        ),
    }
}

pub(super) fn dispatch_runtime_result<F>(ctx: &DispatchCtx<'_>, f: F) -> CommandOutcome
where
    F: FnOnce(&GlobalArgs, &RuntimeService) -> envr_error::EnvrResult<CliExit>,
{
    super::common::with_runtime_service(|service| f(ctx.global, service))
}
