use super::{
    alias_cmd, bundle_cmd, cache_cmd, check, completion_cmd, config_cmd, debug_cmd, deactivate_cmd,
    env_cmd, exec, help_cmd, hook_cmd, import_export, init, profile_cmd, resolve_cmd, run_cmd,
    rust_cmd, shell_cmd, shim_cmd, status_cmd, template_cmd, update, which, why_cmd,
};
use crate::cli::{Command, GlobalArgs};
use crate::CommandOutcome;

/// Route commands that do not require a live runtime service.
pub(super) fn route(command: Command, global: &GlobalArgs) -> CommandOutcome {
    debug_assert!(command.runtime_handler_group().is_none());
    match command {
        Command::Completion { shell } => CommandOutcome::from_result(completion_cmd::run(shell)),
        Command::Help(sub) => route_help(sub, global),
        Command::Init {
            path,
            force,
            full,
            interactive,
        } => CommandOutcome::from_result(init::run_inner(global, path, force, full, interactive)),
        Command::Check { path } => CommandOutcome::from_result(check::run_inner(global, path)),
        Command::Status { project } => {
            CommandOutcome::from_result(status_cmd::run_inner(global, project))
        }
        Command::Why {
            runtime,
            spec,
            project,
        } => CommandOutcome::from_result(why_cmd::run_inner(global, runtime, spec, project)),
        Command::Resolve {
            lang,
            spec,
            project,
        } => CommandOutcome::from_result(resolve_cmd::run_inner(global, lang, spec, project)),
        Command::Exec {
            lang,
            spec,
            shared,
            output,
            command,
            args,
        } => CommandOutcome::from_result(exec::run_inner(
            global, lang, spec, shared, output, command, args,
        )),
        Command::Run {
            shared,
            command,
            args,
        } => CommandOutcome::from_result(run_cmd::run_inner(global, shared, command, args)),
        Command::Env { project, shell } => {
            CommandOutcome::from_result(env_cmd::run_inner(global, project, shell))
        }
        Command::Template {
            file,
            project,
            env,
            env_file,
        } => CommandOutcome::from_result(template_cmd::run_inner(
            global, file, project, env_file, env,
        )),
        Command::Shell { project, shell } => {
            CommandOutcome::from_result(shell_cmd::run_inner(global, project, shell))
        }
        Command::Hook(sub) => route_hook(sub, global),
        Command::Import { file, path } => {
            CommandOutcome::from_result(import_export::import_run_inner(global, file, path))
        }
        Command::Export { path, output } => {
            CommandOutcome::from_result(import_export::export_run_inner(global, path, output))
        }
        Command::Profile(sub) => route_profile(sub, global),
        Command::Config(sub) => CommandOutcome::from_result(config_cmd::run_inner(global, sub)),
        Command::Alias(sub) => CommandOutcome::from_result(alias_cmd::run_inner(global, sub)),
        Command::Update { check } => CommandOutcome::from_result(update::run_inner(global, check)),
        Command::Shim(sub) => route_shim(sub, global),
        Command::Cache(sub) => route_cache(sub, global),
        Command::Bundle(sub) => CommandOutcome::from_result(bundle_cmd::run_inner(global, sub)),
        Command::Rust(sub) => route_rust(sub, global),
        Command::Deactivate => CommandOutcome::from_result(deactivate_cmd::run_inner(global)),
        Command::Debug(sub) => route_debug(sub, global),
        Command::Which { name } => CommandOutcome::from_result(which::run_inner(global, name)),
        other => unreachable!(
            "non-runtime dispatch received runtime-classified command: {:?}",
            other
        ),
    }
}

fn route_help(sub: crate::cli::HelpCmd, global: &GlobalArgs) -> CommandOutcome {
    match sub {
        crate::cli::HelpCmd::Shortcuts => {
            CommandOutcome::from_result(help_cmd::shortcuts_inner(global))
        }
    }
}

fn route_hook(sub: crate::cli::HookCmd, global: &GlobalArgs) -> CommandOutcome {
    match sub {
        crate::cli::HookCmd::Bash => {
            CommandOutcome::from_cli_exit(hook_cmd::emit_hook_script(
                global,
                "bash",
                hook_cmd::HOOK_BASH,
            ))
        }
        crate::cli::HookCmd::Zsh => {
            CommandOutcome::from_cli_exit(hook_cmd::emit_hook_script(
                global,
                "zsh",
                hook_cmd::HOOK_ZSH,
            ))
        }
        crate::cli::HookCmd::Keys { path } => {
            CommandOutcome::from_result(hook_cmd::run_keys_inner(global, path))
        }
        crate::cli::HookCmd::Prompt { project } => {
            CommandOutcome::from_result(status_cmd::run_hook_prompt_inner(global, project))
        }
    }
}

fn route_profile(sub: crate::cli::ProfileCmd, global: &GlobalArgs) -> CommandOutcome {
    match sub {
        crate::cli::ProfileCmd::List { path } => {
            CommandOutcome::from_result(profile_cmd::list_inner(global, path))
        }
        crate::cli::ProfileCmd::Show { name, path } => {
            CommandOutcome::from_result(profile_cmd::show_inner(global, path, name))
        }
    }
}

fn route_shim(sub: crate::cli::ShimCmd, global: &GlobalArgs) -> CommandOutcome {
    match sub {
        crate::cli::ShimCmd::Sync { globals } => {
            CommandOutcome::from_result(shim_cmd::sync_inner(global, globals))
        }
    }
}

fn route_cache(sub: crate::cli::CacheCmd, global: &GlobalArgs) -> CommandOutcome {
    match sub {
        crate::cli::CacheCmd::Clean {
            kind,
            all,
            older_than,
            newer_than,
            dry_run,
        } => CommandOutcome::from_result(cache_cmd::clean_inner(
            global, kind, all, older_than, newer_than, dry_run,
        )),
        crate::cli::CacheCmd::Index(sub) => {
            CommandOutcome::from_result(cache_cmd::index_inner(global, sub))
        }
    }
}

fn route_rust(sub: crate::cli::RustCmd, global: &GlobalArgs) -> CommandOutcome {
    match sub {
        crate::cli::RustCmd::InstallManaged => {
            CommandOutcome::from_result(rust_cmd::install_managed_inner(global))
        }
    }
}

fn route_debug(sub: crate::cli::DebugCmd, global: &GlobalArgs) -> CommandOutcome {
    match sub {
        crate::cli::DebugCmd::Info => CommandOutcome::from_result(debug_cmd::info_inner(global)),
    }
}

#[cfg(test)]
mod tests {
    use super::route;
    use crate::cli::{self, GlobalArgs};
    use std::ffi::OsString;

    #[test]
    fn non_runtime_command_routes_directly() {
        let cmd = parse_command(&["envr", "help", "shortcuts"]);
        let g = GlobalArgs {
            output_format: None,
            porcelain: false,
            quiet: false,
            no_color: false,
            debug: false,
            verbose: false,
            runtime_root: None,
        };
        let out = route(cmd, &g);
        assert!(matches!(
            out,
            crate::CommandOutcome::Done {
                exit_code: 0,
                error_code: None
            }
        ));
    }

    fn parse_command(argv: &[&str]) -> crate::cli::Command {
        let argv: Vec<OsString> = argv.iter().map(OsString::from).collect();
        cli::parse_cli_from_argv(argv).expect("parse").command
    }
}
