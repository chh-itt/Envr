//! CLI command handlers wired to `envr_core::runtime::RuntimeService`.

mod common;
mod current;
mod install;
mod list;
mod use_cmd;

use crate::cli::{Command, GlobalArgs, OutputFormat};

pub fn dispatch(cli: crate::cli::Cli) -> i32 {
    let service = match common::runtime_service() {
        Ok(s) => s,
        Err(e) => return common::print_envr_error(&cli.global, e),
    };

    match cli.command {
        Command::Install {
            lang,
            runtime_version,
        } => install::run(&cli.global, &service, lang, runtime_version),
        Command::Use {
            lang,
            runtime_version,
        } => use_cmd::run(&cli.global, &service, lang, runtime_version),
        Command::List { lang } => list::run(&cli.global, &service, lang),
        Command::Current { lang } => current::run(&cli.global, &service, lang),
        Command::Uninstall { .. } => stub_not_implemented(&cli.global, "uninstall"),
        Command::Which { .. } => stub_not_implemented(&cli.global, "which"),
        Command::Remote { .. } => stub_not_implemented(&cli.global, "remote"),
        Command::Doctor => stub_not_implemented(&cli.global, "doctor"),
    }
}

fn stub_not_implemented(g: &GlobalArgs, cmd: &str) -> i32 {
    match g.output_format.unwrap_or(OutputFormat::Text) {
        OutputFormat::Json => {
            println!(
                r#"{{"success":false,"code":"NOT_IMPLEMENTED","message":"command `{cmd}` is not yet implemented","data":null,"diagnostics":[]}}"#
            );
        }
        OutputFormat::Text => {
            if !g.quiet {
                eprintln!("envr: `{cmd}` is not yet implemented (see T027+)");
            }
        }
    }
    1
}
