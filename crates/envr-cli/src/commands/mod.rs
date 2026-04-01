//! CLI command handlers wired to `envr_core::runtime::RuntimeService`.

mod alias_cmd;
mod check;
mod common;
mod config_cmd;
mod current;
mod doctor;
mod init;
mod install;
mod list;
mod prune;
mod remote;
mod resolve_cmd;
mod uninstall;
mod update;
mod use_cmd;
mod which;

pub fn dispatch(cli: crate::cli::Cli) -> i32 {
    match cli.command {
        crate::cli::Command::Init { path, force } => init::run(&cli.global, path, force),
        crate::cli::Command::Check { path } => check::run(&cli.global, path),
        crate::cli::Command::Resolve { lang, spec, path } => {
            resolve_cmd::run(&cli.global, lang, spec, path)
        }
        crate::cli::Command::Config(sub) => config_cmd::run(&cli.global, sub),
        crate::cli::Command::Alias(sub) => alias_cmd::run(&cli.global, sub),
        crate::cli::Command::Update { check } => update::run(&cli.global, check),
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
                crate::cli::Command::Init { .. }
                | crate::cli::Command::Check { .. }
                | crate::cli::Command::Resolve { .. }
                | crate::cli::Command::Config { .. }
                | crate::cli::Command::Alias { .. }
                | crate::cli::Command::Update { .. }
                | crate::cli::Command::Prune { .. } => unreachable!("handled above"),
            }
        }
    }
}
