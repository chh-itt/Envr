mod app;
mod download_runner;
mod gui_ops;
mod icons;
mod runtime_exec;
mod service;
mod theme;
mod view;
mod widget_styles;

fn main() {
    let _logging_guard = match envr_core::logging::init_logging("envr-gui") {
        Ok(guard) => guard,
        Err(err) => {
            eprintln!(
                "failed to init logging: {}",
                envr_core::logging::format_error_chain(&err)
            );
            return;
        }
    };

    tracing::info!("envr-gui started");
    if let Err(err) = app::run() {
        eprintln!("envr-gui exited with error: {err}");
        std::process::exit(1);
    }
}
