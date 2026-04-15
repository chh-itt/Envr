//! Shim routing: map `node` / `python` / `java` / … to a concrete executable using project config, then global `current`.

mod resolve;

pub use resolve::{
    CoreCommand, ResolvedShim, ShimContext, WhichRuntimeDetail, WhichRuntimeSource,
    core_command_uses_path_proxy_bypass, core_tool_executable, normalize_invoked_basename,
    parse_core_command, parse_shim_invocation, pick_php_version_home, pick_version_home,
    resolve_core_shim, resolve_core_shim_command, resolve_runtime_home_for_lang,
    resolve_runtime_home_for_lang_with_project, which_runtime_detail,
};
