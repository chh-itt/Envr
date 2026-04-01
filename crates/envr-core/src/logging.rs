use envr_error::{EnvrError, EnvrResult};
use std::{env, error::Error, fs, path::PathBuf};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

pub struct LoggingGuard {
    _file_guard: WorkerGuard,
}

fn default_log_dir() -> EnvrResult<PathBuf> {
    let cwd = env::current_dir().map_err(EnvrError::from)?;
    Ok(cwd.join(".envr").join("logs"))
}

/// Directory used by [`init_logging`] for rolling files (`ENVR_LOG_DIR` or `<cwd>/.envr/logs`).
pub fn resolve_log_dir() -> EnvrResult<PathBuf> {
    match env::var("ENVR_LOG_DIR") {
        Ok(path) => Ok(PathBuf::from(path)),
        Err(_) => default_log_dir(),
    }
}

pub fn init_logging(app_name: &str) -> EnvrResult<LoggingGuard> {
    let log_dir = resolve_log_dir()?;

    fs::create_dir_all(&log_dir).map_err(EnvrError::from)?;

    let file_appender = tracing_appender::rolling::daily(&log_dir, format!("{app_name}.log"));
    let (file_writer, file_guard) = tracing_appender::non_blocking(file_appender);

    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let stdout_layer = tracing_subscriber::fmt::layer().with_target(false);
    let file_layer = tracing_subscriber::fmt::layer()
        .with_ansi(false)
        .with_target(true)
        .with_writer(file_writer);

    tracing_subscriber::registry()
        .with(env_filter)
        .with(stdout_layer)
        .with(file_layer)
        .try_init()
        .map_err(|err| EnvrError::Runtime(format!("failed to initialize logging: {err}")))?;

    tracing::info!(app = app_name, log_dir = %log_dir.display(), "logging initialized");

    Ok(LoggingGuard {
        _file_guard: file_guard,
    })
}

pub fn format_error_chain(err: &(dyn Error + 'static)) -> String {
    let mut parts = vec![err.to_string()];
    let mut current = err.source();

    while let Some(source) = current {
        parts.push(source.to_string());
        current = source.source();
    }

    parts.join(" | caused by: ")
}

#[cfg(test)]
mod tests {
    use super::{format_error_chain, init_logging};
    use std::{error::Error, fmt};
    use tempfile::TempDir;

    #[derive(Debug)]
    struct LeafError;

    impl fmt::Display for LeafError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "leaf")
        }
    }

    impl Error for LeafError {}

    #[derive(Debug)]
    struct ParentError {
        source: LeafError,
    }

    impl fmt::Display for ParentError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "parent")
        }
    }

    impl Error for ParentError {
        fn source(&self) -> Option<&(dyn Error + 'static)> {
            Some(&self.source)
        }
    }

    #[test]
    fn format_error_chain_contains_all_levels() {
        let err = ParentError { source: LeafError };
        let chain = format_error_chain(&err);
        assert!(chain.contains("parent"));
        assert!(chain.contains("leaf"));
    }

    #[test]
    fn format_error_chain_single_level() {
        let err = LeafError;
        let chain = format_error_chain(&err);
        assert_eq!(chain, "leaf");
    }

    #[test]
    fn init_logging_second_init_returns_error() {
        let tmp = TempDir::new().expect("tmp");
        let old = std::env::current_dir().expect("cwd");
        std::env::set_current_dir(tmp.path()).expect("chdir");

        let _guard = init_logging("envr-core-test").expect("first init");
        let err = match init_logging("envr-core-test") {
            Ok(_) => panic!("second init should fail"),
            Err(e) => e,
        };
        assert!(err.to_string().contains("failed to initialize logging"));
        std::env::set_current_dir(old).expect("restore cwd");
    }
}
