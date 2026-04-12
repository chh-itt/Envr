//! CLI command handlers wired to `envr_core::runtime::RuntimeService`.

mod alias_cmd;
mod bundle_cmd;
mod cache_cmd;
mod check;
mod child_env;
mod cli_install_progress;
mod run_env_builder;
mod common;
mod completion_cmd;
mod config_cmd;
mod current;
mod debug_cmd;
mod deactivate_cmd;
mod diagnostics;
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

pub fn dispatch(cli: crate::cli::Cli) -> i32 {
    use crate::cli::Command;

    match cli.command {
        Command::Completion { shell } => completion_cmd::run(shell),
        Command::Help(sub) => help_cmd::run(&cli.global, sub),
        Command::Init {
            path,
            force,
            full,
            interactive,
        } => init::run(&cli.global, path, force, full, interactive),
        Command::Check { path } => check::run(&cli.global, path),
        Command::Status { path, profile } => status_cmd::run(&cli.global, path, profile),
        Command::Why {
            runtime,
            spec,
            path,
            profile,
        } => why_cmd::run(&cli.global, runtime, spec, path, profile),
        Command::Resolve {
            lang,
            spec,
            path,
            profile,
        } => resolve_cmd::run(&cli.global, lang, spec, path, profile),
        Command::Exec {
            lang,
            spec,
            install_if_missing,
            dry_run,
            dry_run_diff,
            verbose,
            path,
            profile,
            env,
            env_file,
            output,
            command,
            args,
        } => exec::run(
            &cli.global,
            lang,
            spec,
            install_if_missing,
            dry_run,
            dry_run_diff,
            verbose,
            path,
            profile,
            env,
            env_file,
            output,
            command,
            args,
        ),
        Command::Run {
            install_if_missing,
            dry_run,
            dry_run_diff,
            verbose,
            path,
            profile,
            env,
            env_file,
            command,
            args,
        } => run_cmd::run(
            &cli.global,
            install_if_missing,
            dry_run,
            dry_run_diff,
            verbose,
            path,
            profile,
            env,
            env_file,
            command,
            args,
        ),
        Command::Env {
            path,
            profile,
            shell,
        } => env_cmd::run(&cli.global, path, profile, shell),
        Command::Template {
            file,
            path,
            profile,
            env,
            env_file,
        } => template_cmd::run(&cli.global, file, path, profile, env_file, env),
        Command::Shell {
            path,
            profile,
            shell,
        } => shell_cmd::run(&cli.global, path, profile, shell),
        Command::Hook(sub) => hook_cmd::run(&cli.global, sub),
        Command::Import { file, path } => import_export::import_run(&cli.global, file, path),
        Command::Export { path, output } => import_export::export_run(&cli.global, path, output),
        Command::Profile(sub) => match sub {
            crate::cli::ProfileCmd::List { path } => profile_cmd::list(&cli.global, path),
            crate::cli::ProfileCmd::Show { name, path } => {
                profile_cmd::show(&cli.global, path, name)
            }
        },
        Command::Config(sub) => config_cmd::run(&cli.global, sub),
        Command::Alias(sub) => alias_cmd::run(&cli.global, sub),
        Command::Update { check } => update::run(&cli.global, check),
        Command::Shim(sub) => match sub {
            crate::cli::ShimCmd::Sync { globals } => shim_cmd::sync(&cli.global, globals),
        },
        Command::Cache(sub) => match sub {
            crate::cli::CacheCmd::Clean {
                kind,
                all,
                older_than,
                newer_than,
                dry_run,
            } => cache_cmd::clean(
                &cli.global,
                kind,
                all,
                older_than,
                newer_than,
                dry_run,
            ),
            crate::cli::CacheCmd::Index(sub) => cache_cmd::index(&cli.global, sub),
        },
        Command::Bundle(sub) => bundle_cmd::run(&cli.global, sub),
        Command::Rust(sub) => rust_cmd::run(&cli.global, sub),
        Command::Deactivate => deactivate_cmd::run(&cli.global),
        Command::Debug(sub) => debug_cmd::run(&cli.global, sub),
        Command::Which { name } => which::run(&cli.global, name),

        Command::Project(sub) => common::with_runtime_service(&cli.global, |service| {
            project_cmd::run(&cli.global, service, sub)
        }),
        Command::Prune { lang, execute } => common::with_runtime_service(&cli.global, |service| {
            prune::run(&cli.global, service, lang, execute)
        }),
        Command::Doctor {
            fix,
            fix_path,
            fix_path_apply,
            json,
        } => common::with_runtime_service(&cli.global, |service| {
            let mut g = cli.global.clone();
            if json {
                g.output_format = Some(crate::cli::OutputFormat::Json);
            }
            doctor::run(&g, service, fix, fix_path, fix_path_apply)
        }),
        Command::Install {
            runtime,
            runtime_version,
        } => common::with_runtime_service(&cli.global, |service| {
            install::run(&cli.global, service, runtime, runtime_version)
        }),
        Command::Use {
            runtime,
            runtime_version,
        } => common::with_runtime_service(&cli.global, |service| {
            use_cmd::run(&cli.global, service, runtime, runtime_version)
        }),
        Command::List { runtime, outdated } => common::with_runtime_service(&cli.global, |service| {
            list::run(&cli.global, service, runtime, outdated)
        }),
        Command::Current { runtime } => common::with_runtime_service(&cli.global, |service| {
            current::run(&cli.global, service, runtime)
        }),
        Command::Uninstall {
            runtime,
            runtime_version,
            dry_run,
            force,
            yes,
        } => common::with_runtime_service(&cli.global, |service| {
            uninstall::run(
                &cli.global,
                service,
                runtime,
                runtime_version,
                dry_run,
                force,
                yes,
            )
        }),
        Command::Remote { runtime, prefix } => common::with_runtime_service(&cli.global, |service| {
            remote::run(&cli.global, service, runtime, prefix)
        }),
        Command::Diagnostics(sub) => common::with_runtime_service(&cli.global, |service| {
            diagnostics::run(&cli.global, service, sub)
        }),
    }
}
