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
    match cli.command {
        crate::cli::Command::Completion { shell } => completion_cmd::run(shell),
        crate::cli::Command::Init {
            path,
            force,
            full,
            interactive,
        } => init::run(&cli.global, path, force, full, interactive),
        crate::cli::Command::Check { path } => check::run(&cli.global, path),
        crate::cli::Command::Status { path, profile } => {
            status_cmd::run(&cli.global, path, profile)
        }
        crate::cli::Command::Project(sub) => {
            let service = match common::runtime_service() {
                Ok(s) => s,
                Err(e) => return common::print_envr_error(&cli.global, e),
            };
            project_cmd::run(&cli.global, &service, sub)
        },
        crate::cli::Command::Why {
            runtime,
            spec,
            path,
            profile,
        } => why_cmd::run(&cli.global, runtime, spec, path, profile),
        crate::cli::Command::Resolve {
            lang,
            spec,
            path,
            profile,
        } => resolve_cmd::run(&cli.global, lang, spec, path, profile),
        crate::cli::Command::Exec {
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
        crate::cli::Command::Run {
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
        crate::cli::Command::Env {
            path,
            profile,
            shell,
        } => env_cmd::run(&cli.global, path, profile, shell),
        crate::cli::Command::Template {
            file,
            path,
            profile,
            env,
            env_file,
        } => template_cmd::run(&cli.global, file, path, profile, env_file, env),
        crate::cli::Command::Shell {
            path,
            profile,
            shell,
        } => shell_cmd::run(&cli.global, path, profile, shell),
        crate::cli::Command::Hook(sub) => hook_cmd::run(&cli.global, sub),
        crate::cli::Command::Import { file, path } => {
            import_export::import_run(&cli.global, file, path)
        }
        crate::cli::Command::Export { path, output } => {
            import_export::export_run(&cli.global, path, output)
        }
        crate::cli::Command::Profile(sub) => match sub {
            crate::cli::ProfileCmd::List { path } => profile_cmd::list(&cli.global, path),
            crate::cli::ProfileCmd::Show { name, path } => {
                profile_cmd::show(&cli.global, path, name)
            }
        },
        crate::cli::Command::Config(sub) => config_cmd::run(&cli.global, sub),
        crate::cli::Command::Alias(sub) => alias_cmd::run(&cli.global, sub),
        crate::cli::Command::Update { check } => update::run(&cli.global, check),
        crate::cli::Command::Shim(sub) => match sub {
            crate::cli::ShimCmd::Sync { globals } => shim_cmd::sync(&cli.global, globals),
        },
        crate::cli::Command::Cache(sub) => match sub {
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
        crate::cli::Command::Bundle(sub) => bundle_cmd::run(&cli.global, sub),
        crate::cli::Command::Rust(sub) => rust_cmd::run(&cli.global, sub),
        crate::cli::Command::Prune { lang, execute } => {
            let service = match common::runtime_service() {
                Ok(s) => s,
                Err(e) => return common::print_envr_error(&cli.global, e),
            };
            prune::run(&cli.global, &service, lang, execute)
        }
        crate::cli::Command::Doctor {
            fix,
            fix_path,
            fix_path_apply,
            json,
        } => {
            let service = match common::runtime_service() {
                Ok(s) => s,
                Err(e) => return common::print_envr_error(&cli.global, e),
            };
            let mut g = cli.global.clone();
            if json {
                g.output_format = Some(crate::cli::OutputFormat::Json);
            }
            doctor::run(&g, &service, fix, fix_path, fix_path_apply)
        }
        crate::cli::Command::Deactivate => deactivate_cmd::run(&cli.global),
        crate::cli::Command::Debug(sub) => debug_cmd::run(&cli.global, sub),
        other => {
            let service = match common::runtime_service() {
                Ok(s) => s,
                Err(e) => return common::print_envr_error(&cli.global, e),
            };
            match other {
                crate::cli::Command::Install {
                    runtime,
                    runtime_version,
                } => install::run(&cli.global, &service, runtime, runtime_version),
                crate::cli::Command::Use {
                    runtime,
                    runtime_version,
                } => use_cmd::run(&cli.global, &service, runtime, runtime_version),
                crate::cli::Command::List { runtime, outdated } => {
                    list::run(&cli.global, &service, runtime, outdated)
                }
                crate::cli::Command::Current { runtime } => {
                    current::run(&cli.global, &service, runtime)
                }
                crate::cli::Command::Uninstall {
                    runtime,
                    runtime_version,
                    dry_run,
                    force,
                    yes,
                } => uninstall::run(
                    &cli.global,
                    &service,
                    runtime,
                    runtime_version,
                    dry_run,
                    force,
                    yes,
                ),
                crate::cli::Command::Which { name } => which::run(&cli.global, name),
                crate::cli::Command::Remote { runtime, prefix } => {
                    remote::run(&cli.global, &service, runtime, prefix)
                }
                crate::cli::Command::Diagnostics(sub) => {
                    diagnostics::run(&cli.global, &service, sub)
                }
                crate::cli::Command::Init { .. }
                | crate::cli::Command::Check { .. }
                | crate::cli::Command::Status { .. }
                | crate::cli::Command::Project { .. }
                | crate::cli::Command::Why { .. }
                | crate::cli::Command::Resolve { .. }
                | crate::cli::Command::Exec { .. }
                | crate::cli::Command::Run { .. }
                | crate::cli::Command::Env { .. }
                | crate::cli::Command::Template { .. }
                | crate::cli::Command::Shell { .. }
                | crate::cli::Command::Hook(_)
                | crate::cli::Command::Import { .. }
                | crate::cli::Command::Export { .. }
                | crate::cli::Command::Profile { .. }
                | crate::cli::Command::Config { .. }
                | crate::cli::Command::Alias { .. }
                | crate::cli::Command::Update { .. }
                | crate::cli::Command::Shim { .. }
                | crate::cli::Command::Cache { .. }
                | crate::cli::Command::Bundle { .. }
                | crate::cli::Command::Rust(_)
                | crate::cli::Command::Prune { .. }
                | crate::cli::Command::Doctor { .. }
                | crate::cli::Command::Deactivate
                | crate::cli::Command::Debug { .. }
                | crate::cli::Command::Completion { .. } => unreachable!("handled above"),
            }
        }
    }
}
