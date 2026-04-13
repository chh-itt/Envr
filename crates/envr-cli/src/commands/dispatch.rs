//! Top-level command routing (keeps [`crate::commands`] `mod.rs` as module declarations only).

use super::common;
use super::{
    alias_cmd, bundle_cmd, cache_cmd, check, completion_cmd, config_cmd, current, deactivate_cmd,
    debug_cmd, diagnostics, doctor, env_cmd, exec, help_cmd, hook_cmd, import_export, init,
    install, list, profile_cmd, project_cmd, prune, remote, resolve_cmd, run_cmd, rust_cmd,
    shell_cmd, shim_cmd, status_cmd, template_cmd, uninstall, update, use_cmd, which, why_cmd,
};

use crate::cli::{Command, GlobalArgs};
use crate::CommandOutcome;
use envr_core::runtime::service::RuntimeService;

/// Route a parsed [`crate::cli::Cli`] to the appropriate handler.
///
/// Returns a single [`CommandOutcome`]; map to process exit with [`CommandOutcome::finish`]
/// (see [`crate::cli::run`]).
pub fn dispatch(cli: crate::cli::Cli) -> CommandOutcome {
    let command = cli.command.trace_name();
    let span = tracing::info_span!("envr.cli.command", command);
    let _guard = span.enter();
    match cli.command {
        Command::Completion { shell } => CommandOutcome::Done(completion_cmd::run(shell)),
        Command::Help(sub) => CommandOutcome::Done(help_cmd::run(&cli.global, sub)),
        Command::Init {
            path,
            force,
            full,
            interactive,
        } => CommandOutcome::Done(init::run(&cli.global, path, force, full, interactive)),
        Command::Check { path } => CommandOutcome::Done(check::run(&cli.global, path)),
        Command::Status { project } => CommandOutcome::Done(status_cmd::run(&cli.global, project)),
        Command::Why {
            runtime,
            spec,
            project,
        } => CommandOutcome::Done(why_cmd::run(&cli.global, runtime, spec, project)),
        Command::Resolve {
            lang,
            spec,
            project,
        } => CommandOutcome::Done(resolve_cmd::run(&cli.global, lang, spec, project)),
        Command::Exec {
            lang,
            spec,
            shared,
            output,
            command,
            args,
        } => CommandOutcome::Done(exec::run(
            &cli.global,
            lang,
            spec,
            shared,
            output,
            command,
            args,
        )),
        Command::Run {
            shared,
            command,
            args,
        } => CommandOutcome::Done(run_cmd::run(&cli.global, shared, command, args)),
        Command::Env { project, shell } => CommandOutcome::Done(env_cmd::run(&cli.global, project, shell)),
        Command::Template {
            file,
            project,
            env,
            env_file,
        } => CommandOutcome::Done(template_cmd::run(
            &cli.global,
            file,
            project,
            env_file,
            env,
        )),
        Command::Shell { project, shell } => {
            CommandOutcome::Done(shell_cmd::run(&cli.global, project, shell))
        }
        Command::Hook(sub) => CommandOutcome::Done(hook_cmd::run(&cli.global, sub)),
        Command::Import { file, path } => {
            CommandOutcome::Done(import_export::import_run(&cli.global, file, path))
        }
        Command::Export { path, output } => {
            CommandOutcome::Done(import_export::export_run(&cli.global, path, output))
        }
        Command::Profile(sub) => match sub {
            crate::cli::ProfileCmd::List { path } => {
                CommandOutcome::Done(profile_cmd::list(&cli.global, path))
            }
            crate::cli::ProfileCmd::Show { name, path } => {
                CommandOutcome::Done(profile_cmd::show(&cli.global, path, name))
            }
        },
        Command::Config(sub) => CommandOutcome::Done(config_cmd::run(&cli.global, sub)),
        Command::Alias(sub) => CommandOutcome::Done(alias_cmd::run(&cli.global, sub)),
        Command::Update { check } => CommandOutcome::Done(update::run(&cli.global, check)),
        Command::Shim(sub) => match sub {
            crate::cli::ShimCmd::Sync { globals } => {
                CommandOutcome::Done(shim_cmd::sync(&cli.global, globals))
            }
        },
        Command::Cache(sub) => match sub {
            crate::cli::CacheCmd::Clean {
                kind,
                all,
                older_than,
                newer_than,
                dry_run,
            } => CommandOutcome::Done(cache_cmd::clean(
                &cli.global,
                kind,
                all,
                older_than,
                newer_than,
                dry_run,
            )),
            crate::cli::CacheCmd::Index(sub) => CommandOutcome::Done(cache_cmd::index(&cli.global, sub)),
        },
        Command::Bundle(sub) => CommandOutcome::Done(bundle_cmd::run(&cli.global, sub)),
        Command::Rust(sub) => CommandOutcome::Done(rust_cmd::run(&cli.global, sub)),
        Command::Deactivate => CommandOutcome::Done(deactivate_cmd::run(&cli.global)),
        Command::Debug(sub) => CommandOutcome::Done(debug_cmd::run(&cli.global, sub)),
        Command::Which { name } => CommandOutcome::Done(which::run(&cli.global, name)),

        Command::Project(sub) => {
            dispatch_runtime(&cli.global, |g, service| project_cmd::run(g, service, sub))
        }
        Command::Prune { lang, execute } => {
            dispatch_runtime(&cli.global, |g, service| prune::run(g, service, lang, execute))
        }
        Command::Doctor {
            fix,
            fix_path,
            fix_path_apply,
            json,
        } => dispatch_runtime(&cli.global, |g, service| {
            doctor::run(
                &g.cloned_with_legacy_json(json),
                service,
                fix,
                fix_path,
                fix_path_apply,
            )
        }),
        Command::Install {
            runtime,
            runtime_version,
        } => dispatch_runtime(&cli.global, |g, service| {
            install::run(g, service, runtime, runtime_version)
        }),
        Command::Use {
            runtime,
            runtime_version,
        } => dispatch_runtime(&cli.global, |g, service| {
            use_cmd::run(g, service, runtime, runtime_version)
        }),
        Command::List { runtime, outdated } => {
            dispatch_runtime(&cli.global, |g, service| list::run(g, service, runtime, outdated))
        }
        Command::Current { runtime } => {
            dispatch_runtime(&cli.global, |g, service| current::run(g, service, runtime))
        }
        Command::Uninstall {
            runtime,
            runtime_version,
            dry_run,
            force,
            yes,
        } => dispatch_runtime(&cli.global, |g, service| {
            uninstall::run(
                g,
                service,
                runtime,
                runtime_version,
                dry_run,
                force,
                yes,
            )
        }),
        Command::Remote { runtime, prefix } => {
            dispatch_runtime(&cli.global, |g, service| remote::run(g, service, runtime, prefix))
        }
        Command::Diagnostics(sub) => {
            dispatch_runtime(&cli.global, |g, service| diagnostics::run(g, service, sub))
        }
    }
}

/// Commands that need a live [`envr_core::runtime::service::RuntimeService`].
fn dispatch_runtime<F>(g: &GlobalArgs, f: F) -> CommandOutcome
where
    F: FnOnce(&GlobalArgs, &RuntimeService) -> i32,
{
    common::with_runtime_service(|service| Ok(f(g, service)))
}
