mod cli;

use clap::Parser;

fn main() {
    let cli = cli::Cli::parse();
    cli::apply_global(&cli.global);

    let _logging_guard = match envr_core::logging::init_logging("envr-cli") {
        Ok(guard) => guard,
        Err(err) => {
            eprintln!(
                "failed to init logging: {}",
                envr_core::logging::format_error_chain(&err)
            );
            std::process::exit(2);
        }
    };

    tracing::info!("envr-cli started");
    let code = cli::run(cli);
    std::process::exit(code);
}
