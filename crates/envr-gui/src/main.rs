#![allow(
    clippy::collapsible_else_if,
    clippy::field_reassign_with_default,
    clippy::large_enum_variant,
    clippy::obfuscated_if_else,
    clippy::too_many_arguments,
    clippy::uninlined_format_args,
    clippy::unnecessary_sort_by,
    clippy::useless_conversion
)]

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
