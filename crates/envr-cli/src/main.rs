mod cli;
mod cli_help;
mod commands;
mod output;

use clap::FromArgMatches;

fn main() {
    bootstrap_i18n();
    let matches = cli_help::localized_command().get_matches();
    let cli = cli::Cli::from_arg_matches(&matches).unwrap_or_else(|e| e.exit());
    cli::apply_global(&cli.global);

    let _logging_guard = match envr_core::logging::init_logging("envr-cli") {
        Ok(guard) => guard,
        Err(err) => {
            let prefix = envr_core::i18n::tr_key(
                "cli.bootstrap.logging_failed",
                "初始化日志失败",
                "failed to init logging",
            );
            eprintln!(
                "{}: {}",
                prefix,
                envr_core::logging::format_error_chain(&err)
            );
            std::process::exit(2);
        }
    };

    tracing::info!("envr-cli started");
    let code = cli::run(cli);
    std::process::exit(code);
}

fn bootstrap_i18n() {
    if let Ok(paths) = envr_platform::paths::current_platform_paths() {
        let settings_path = envr_config::settings::settings_path_from_platform(&paths);
        let st = envr_config::settings::Settings::load_or_default_from(&settings_path)
            .unwrap_or_default();
        envr_core::i18n::init_from_settings(&st);
    }
}
