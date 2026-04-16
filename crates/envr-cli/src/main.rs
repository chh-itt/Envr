fn main() {
    let locale = envr_cli::bootstrap_locale();
    envr_core::i18n::with_locale(locale, || {
        let cli = match envr_cli::cli::parse_cli_from_env() {
            Ok(cli) => cli,
            Err(exit) => {
                envr_cli::flush_parse_metrics_on_early_exit();
                std::process::exit(exit.exit_code)
            }
        };
        envr_cli::cli::apply_global(&cli);

        let debug = cli.global.debug;
        let code = envr_cli::run_cli_with_logging(cli, debug);
        std::process::exit(code);
    });
}
