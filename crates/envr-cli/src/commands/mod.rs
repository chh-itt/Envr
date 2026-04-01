//! CLI command handlers wired to `envr_core::runtime::RuntimeService`.

mod common;
mod current;
mod doctor;
mod install;
mod list;
mod remote;
mod uninstall;
mod use_cmd;
mod which;

pub fn dispatch(cli: crate::cli::Cli) -> i32 {
    let service = match common::runtime_service() {
        Ok(s) => s,
        Err(e) => return common::print_envr_error(&cli.global, e),
    };

    match cli.command {
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
    }
}
