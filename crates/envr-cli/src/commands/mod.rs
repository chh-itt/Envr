//! CLI command handlers wired to `envr_core::runtime::RuntimeService`.

mod alias_cmd;
mod cache_cmd;
mod check;
mod child_env;
mod common;
mod config_cmd;
mod current;
mod diagnostics;
mod doctor;
mod env_cmd;
mod exec;
mod import_export;
mod init;
mod install;
mod list;
mod profile_cmd;
mod prune;
mod remote;
mod resolve_cmd;
mod run_cmd;
mod shim_cmd;
mod uninstall;
mod update;
mod use_cmd;
mod which;

pub fn dispatch(cli: crate::cli::Cli) -> i32 {
    match cli.command {
        crate::cli::Command::Init { path, force } => init::run(&cli.global, path, force),
        crate::cli::Command::Check { path } => check::run(&cli.global, path),
        crate::cli::Command::Resolve {
            lang,
            spec,
            path,
            profile,
        } => resolve_cmd::run(&cli.global, lang, spec, path, profile),
        crate::cli::Command::Exec {
            lang,
            spec,
            path,
            profile,
            command,
            args,
        } => exec::run(&cli.global, lang, spec, path, profile, command, args),
        crate::cli::Command::Run {
            path,
            profile,
            command,
            args,
        } => run_cmd::run(&cli.global, path, profile, command, args),
        crate::cli::Command::Env {
            path,
            profile,
            shell,
        } => env_cmd::run(&cli.global, path, profile, shell),
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
            crate::cli::CacheCmd::Clean { kind, all } => cache_cmd::clean(&cli.global, kind, all),
        },
        crate::cli::Command::Prune { lang, execute } => {
            let service = match common::runtime_service() {
                Ok(s) => s,
                Err(e) => return common::print_envr_error(&cli.global, e),
            };
            prune::run(&cli.global, &service, lang, execute)
        }
        other => {
            let service = match common::runtime_service() {
                Ok(s) => s,
                Err(e) => return common::print_envr_error(&cli.global, e),
            };
            match other {
                crate::cli::Command::Install {
                    lang,
                    runtime_version,
                } => install::run(&cli.global, &service, lang, runtime_version),
                crate::cli::Command::Use {
                    lang,
                    runtime_version,
                } => use_cmd::run(&cli.global, &service, lang, runtime_version),
                crate::cli::Command::List { lang } => list::run(&cli.global, &service, lang),
                crate::cli::Command::Current { lang } => current::run(&cli.global, &service, lang),
                crate::cli::Command::Uninstall {
                    lang,
                    runtime_version,
                } => uninstall::run(&cli.global, &service, lang, runtime_version),
                crate::cli::Command::Which { name } => which::run(&cli.global, name),
                crate::cli::Command::Remote { lang, prefix } => {
                    remote::run(&cli.global, &service, lang, prefix)
                }
                crate::cli::Command::Doctor => doctor::run(&cli.global, &service),
                crate::cli::Command::Diagnostics(sub) => {
                    diagnostics::run(&cli.global, &service, sub)
                }
                crate::cli::Command::Init { .. }
                | crate::cli::Command::Check { .. }
                | crate::cli::Command::Resolve { .. }
                | crate::cli::Command::Exec { .. }
                | crate::cli::Command::Run { .. }
                | crate::cli::Command::Env { .. }
                | crate::cli::Command::Import { .. }
                | crate::cli::Command::Export { .. }
                | crate::cli::Command::Profile { .. }
                | crate::cli::Command::Config { .. }
                | crate::cli::Command::Alias { .. }
                | crate::cli::Command::Update { .. }
                | crate::cli::Command::Shim { .. }
                | crate::cli::Command::Cache { .. }
                | crate::cli::Command::Prune { .. } => unreachable!("handled above"),
            }
        }
    }
}
