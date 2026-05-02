//! Shim routing: map `node` / `python` / `java` / … to a concrete executable using project config, then global `current`.

mod resolve;

pub use resolve::{
    CoreCommand, ResolvedShim, ShimContext, ShimSettingsSnapshot, WhichRuntimeDetail,
    WhichRuntimeSource, core_command_uses_path_proxy_bypass, core_tool_executable,
    load_shim_settings_snapshot, normalize_invoked_basename, parse_core_command,
    parse_shim_invocation, pick_php_version_home, pick_version_home, resolve_core_shim,
    resolve_core_shim_command, resolve_core_shim_command_with_settings,
    resolve_runtime_home_for_lang, resolve_runtime_home_for_lang_with_project,
    resolve_runtime_home_for_lang_with_project_and_settings, resolve_version_home,
    runtime_bin_dirs_for_key, runtime_home_env_for_key, runtime_version_label_from_executable,
    which_runtime_detail,
};
