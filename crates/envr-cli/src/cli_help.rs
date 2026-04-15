//! Localized `--help` for the clap tree (must match `Cli::command()` structure).
//!
//! Copy lives in [`crate::cli::help_registry`] as path-keyed static tables; this module only wires
//! `Cli::command()` + i18n application.

use clap::{Command, CommandFactory};

use crate::cli::Cli;
use crate::cli::help_registry;

/// Same as [`Cli::command()`] but with `settings.toml` locale applied to about/help text.
/// Call after [`envr_core::i18n::init_from_settings`].
pub fn localized_command() -> Command {
    let mut cmd = Cli::command();
    help_registry::apply_root_help(&mut cmd);
    help_registry::apply_subcommand_tree_help(&mut cmd);
    cmd
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn localized_command_matches_cli_structure() {
        let a = Cli::command();
        let b = localized_command();
        let na: Vec<_> = a
            .get_subcommands()
            .map(|c| c.get_name().to_string())
            .collect();
        let nb: Vec<_> = b
            .get_subcommands()
            .map(|c| c.get_name().to_string())
            .collect();
        assert_eq!(na, nb);
    }
}
