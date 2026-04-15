//! Backward-compatible module name: command identity lives in [`super::command_spec`] (`CommandSpec`).

pub(crate) use super::command_spec::*;

/// Historical name for [`CommandSpec`] (dispatch / tracing / help path / JSON contract hints).
pub(crate) type CommandMetadata = CommandSpec;

#[inline]
pub(crate) fn metadata_for_key(key: CommandKey) -> CommandSpec {
    spec_for_key(key)
}

#[cfg(test)]
#[inline]
pub(crate) fn metadata_registry_entries() -> &'static [(CommandKey, CommandSpec)] {
    spec_registry_entries()
}
