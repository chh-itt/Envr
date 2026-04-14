//! CLI command handlers wired to `envr_core::runtime::RuntimeService`.

mod alias_cmd;
mod bundle_cmd;
mod cache_cmd;
mod check;
mod child_env;
mod cli_install_progress;
mod run_env_builder;
pub mod common;
mod completion_cmd;
mod config_cmd;
mod current;
mod debug_cmd;
mod deactivate_cmd;
mod diagnostics;
mod dispatch;
mod dispatch_non_runtime;
mod dispatch_runtime;
mod dispatch_runtime_installation;
mod dispatch_runtime_misc;
mod dispatch_runtime_project;
mod doctor;
mod dry_run_env;
mod env_cmd;
mod env_overrides;
mod exec;
mod help_cmd;
mod hook_cmd;
mod import_export;
mod init;
mod install;
mod list;
mod profile_cmd;
mod project_cmd;
mod project_status;
mod prune;
mod remote;
mod resolve_cmd;
mod status_cmd;
mod run_cmd;
mod rust_cmd;
mod shell_cmd;
mod shim_cmd;
mod template_cmd;
mod uninstall;
mod update;
mod use_cmd;
mod which;
mod why_cmd;

pub fn dispatch(cli: crate::cli::Cli) -> (crate::CommandOutcome, crate::cli::GlobalArgs) {
    dispatch::dispatch(cli)
}
