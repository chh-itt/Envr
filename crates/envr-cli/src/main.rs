fn main() {
    let _logging_guard = match envr_core::logging::init_logging("envr-cli") {
        Ok(guard) => guard,
        Err(err) => {
            eprintln!(
                "failed to init logging: {}",
                envr_core::logging::format_error_chain(&err)
            );
            return;
        }
    };

    tracing::info!("envr-cli started");
}
