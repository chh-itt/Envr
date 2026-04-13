use clap::FromArgMatches;

fn main() {
    envr_cli::bootstrap_i18n();
    let argv = envr_cli::cli::expand_user_cli_aliases(std::env::args_os().collect());
    let argv = envr_cli::cli::preprocess_cli_args(argv);
    let matches = envr_cli::cli_help::localized_command().get_matches_from(argv);
    let cli = envr_cli::cli::Cli::from_arg_matches(&matches).unwrap_or_else(|e| e.exit());
    envr_cli::cli::apply_global(&cli);

    let debug = cli.global.debug;
    let code = envr_cli::run_cli_with_logging(cli, debug);
    std::process::exit(code);
}
