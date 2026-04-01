//! Shim routing: map `node` / `python` / `java` / … to a concrete executable using project config, then global `current`.

mod resolve;

pub use resolve::{
    CoreCommand, ResolvedShim, ShimContext, core_tool_executable, normalize_invoked_basename,
    parse_shim_invocation, pick_version_home, resolve_core_shim, resolve_core_shim_command,
};
