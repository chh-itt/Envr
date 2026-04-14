use crate::cli::{Command, GlobalArgs, RuntimeHandlerGroup};
use crate::CommandOutcome;
use envr_core::runtime::service::RuntimeService;

pub(super) struct DispatchCtx<'a> {
    pub(super) global: &'a GlobalArgs,
}

pub(super) fn is_runtime_command(command: &Command) -> bool {
    command.is_runtime_command()
}

/// Route commands that require a live runtime service.
pub(super) fn route(command: Command, ctx: &DispatchCtx<'_>) -> CommandOutcome {
    debug_assert!(is_runtime_command(&command));
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
    F: FnOnce(&GlobalArgs, &RuntimeService) -> envr_error::EnvrResult<i32>,
{
    super::common::with_runtime_service(|service| f(ctx.global, service))
}
