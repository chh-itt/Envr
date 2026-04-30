use super::dispatch_macros::dispatch_match;
use super::{
    alias_cmd, bundle_cmd, cache_cmd, check, completion_cmd, config_cmd, deactivate_cmd, debug_cmd,
    env_cmd, exec, help_cmd, hook_cmd, import_export, init, profile_cmd, resolve_cmd, run_cmd,
    rust_cmd, shell_cmd, shim_cmd, status_cmd, template_cmd, update, which, why_cmd,
};
use crate::CommandOutcome;
use crate::cli::{Command, GlobalArgs};

macro_rules! ok {
    ($expr:expr) => {
        CommandOutcome::from_result($expr)
    };
}

macro_rules! cli {
    ($expr:expr) => {
        CommandOutcome::from_cli_exit($expr)
    };
}

/// Route commands that do not require a live runtime service.
pub(super) fn route(command: Command, global: &GlobalArgs) -> CommandOutcome {
    debug_assert!(command.runtime_handler_group().is_none());
    dispatch_match!(
        command,
        _ => unreachable!(
            "non-runtime dispatch received runtime-classified command: {:?}",
            command
        );
        Command::Completion { shell } => ok!(completion_cmd::run(shell)),
        Command::Help(sub) => route_help(sub, global),
        Command::Init {
            path,
            force,
            full,
            interactive,
        } => ok!(init::run_inner(global, path, force, full, interactive)),
        Command::Check { path } => ok!(check::run_inner(global, path)),
        Command::Status { project } => {
            ok!(status_cmd::run_inner(global, project))
        },
        Command::Why {
            runtime,
            spec,
            project,
        } => ok!(why_cmd::run_inner(global, runtime, spec, project)),
        Command::Resolve {
            lang,
            spec,
            project,
        } => ok!(resolve_cmd::run_inner(global, lang, spec, project)),
        Command::Exec {
            lang,
            spec,
            shared,
            output,
            command,
            args,
        } => ok!(exec::run_inner(
            global, lang, spec, shared, output, command, args,
        )),
        Command::Run {
            shared,
            list,
            command,
            args,
        } => ok!(run_cmd::run_inner(global, shared, list, command, args)),
        Command::Env { project, shell } => {
            ok!(env_cmd::run_inner(global, project, shell))
        },
        Command::Template {
            file,
            project,
            env,
            env_file,
        } => ok!(template_cmd::run_inner(
            global, file, project, env_file, env,
        )),
        Command::Shell { project, shell } => {
            ok!(shell_cmd::run_inner(global, project, shell))
        },
        Command::Hook(sub) => route_hook(sub, global),
        Command::Import {
            file,
            path,
            format,
            dry_run,
        } => ok!(import_export::import_run_inner(
            global, file, path, format, dry_run,
        )),
        Command::Export {
            path,
            output,
            format,
        } => ok!(import_export::export_run_inner(
            global, path, output, format,
        )),
        Command::Profile(sub) => route_profile(sub, global),
        Command::Config(sub) => ok!(config_cmd::run_inner(global, sub)),
        Command::Alias(sub) => ok!(alias_cmd::run_inner(global, sub)),
        Command::Update { check } => ok!(update::run_inner(global, check)),
        Command::Shim(sub) => route_shim(sub, global),
        Command::Cache(sub) => route_cache(sub, global),
        Command::Bundle(sub) => ok!(bundle_cmd::run_inner(global, sub)),
        Command::Rust(sub) => route_rust(sub, global),
        Command::Deactivate => ok!(deactivate_cmd::run_inner(global)),
        Command::Debug(sub) => route_debug(sub, global),
        Command::Which { name } => ok!(which::run_inner(global, name)),
    )
}

fn route_help(sub: crate::cli::HelpCmd, global: &GlobalArgs) -> CommandOutcome {
    dispatch_match!(
        sub;
        crate::cli::HelpCmd::Shortcuts => {
            CommandOutcome::from_result(help_cmd::shortcuts_inner(global))
        },
    )
}

fn route_hook(sub: crate::cli::HookCmd, global: &GlobalArgs) -> CommandOutcome {
    dispatch_match!(
        sub;
        crate::cli::HookCmd::Bash => cli!(hook_cmd::emit_hook_script(
            global,
            "bash",
            hook_cmd::HOOK_BASH,
        )),
        crate::cli::HookCmd::Zsh => cli!(hook_cmd::emit_hook_script(
            global,
            "zsh",
            hook_cmd::HOOK_ZSH,
        )),
        crate::cli::HookCmd::Keys { path } => {
            ok!(hook_cmd::run_keys_inner(global, path))
        },
        crate::cli::HookCmd::Prompt { project } => {
            ok!(status_cmd::run_hook_prompt_inner(global, project))
        },
    )
}

fn route_profile(sub: crate::cli::ProfileCmd, global: &GlobalArgs) -> CommandOutcome {
    dispatch_match!(
        sub;
        crate::cli::ProfileCmd::List { path } => {
            ok!(profile_cmd::list_inner(global, path))
        },
        crate::cli::ProfileCmd::Show { name, path } => {
            ok!(profile_cmd::show_inner(global, path, name))
        },
    )
}

fn route_shim(sub: crate::cli::ShimCmd, global: &GlobalArgs) -> CommandOutcome {
    dispatch_match!(
        sub;
        crate::cli::ShimCmd::Sync { globals } => {
            ok!(shim_cmd::sync_inner(global, globals))
        },
    )
}

fn route_cache(sub: crate::cli::CacheCmd, global: &GlobalArgs) -> CommandOutcome {
    dispatch_match!(
        sub;
        crate::cli::CacheCmd::Clean {
            kind,
            all,
            older_than,
            newer_than,
            dry_run,
        } => ok!(cache_cmd::clean_inner(
            global, kind, all, older_than, newer_than, dry_run,
        )),
        crate::cli::CacheCmd::Index(sub) => {
            ok!(cache_cmd::index_inner(global, sub))
        },
        crate::cli::CacheCmd::Runtime(sub) => {
            ok!(cache_cmd::runtime_inner(global, sub))
        },
    )
}

fn route_rust(sub: crate::cli::RustCmd, global: &GlobalArgs) -> CommandOutcome {
    dispatch_match!(
        sub;
        crate::cli::RustCmd::InstallManaged => {
            ok!(rust_cmd::install_managed_inner(global))
        },
    )
}

fn route_debug(sub: crate::cli::DebugCmd, global: &GlobalArgs) -> CommandOutcome {
    dispatch_match!(
        sub;
        crate::cli::DebugCmd::Info => ok!(debug_cmd::info_inner(global)),
    )
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
