use envr_error::{EnvrError, EnvrResult, ErrorCode};
use std::{env, error::Error, fs, path::PathBuf};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::Layer;
use tracing_subscriber::filter::Targets;
use tracing_subscriber::fmt::writer::BoxMakeWriter;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

pub struct LoggingGuard {
    _file_guard: WorkerGuard,
    _metrics_guard: Option<WorkerGuard>,
}

/// Options for [`init_logging_with`].
#[derive(Debug, Clone, Copy, Default)]
pub struct LoggingInitOptions {
    /// When `true`, emit the console tracing layer to **stderr**; when `false`, to **stdout**.
    /// The `envr` CLI uses `true` so machine-readable stdout (`--format json`, porcelain) is never mixed with logs.
    pub log_to_stderr: bool,
    /// Default filter used when `RUST_LOG` is not set or invalid.
    ///
    /// When `None`, falls back to `"info"`.
    pub default_filter: Option<&'static str>,
}

fn default_log_dir() -> EnvrResult<PathBuf> {
    if let Ok(paths) = envr_platform::paths::current_platform_paths() {
        return Ok(paths.log_dir);
    }
    let cwd = env::current_dir().map_err(EnvrError::from)?;
    Ok(cwd.join(".envr").join("logs"))
}

/// Directory used by [`init_logging`] for rolling files.
///
/// Resolution:
/// - `ENVR_LOG_DIR` (when set)
/// - platform default log dir (for example `%APPDATA%\envr\logs` on Windows)
/// - fallback: `<cwd>/.envr/logs` when platform path discovery fails
pub fn resolve_log_dir() -> EnvrResult<PathBuf> {
    match env::var("ENVR_LOG_DIR") {
        Ok(path) => Ok(PathBuf::from(path)),
        Err(_) => default_log_dir(),
    }
}

pub fn init_logging(app_name: &str) -> EnvrResult<LoggingGuard> {
    init_logging_with(app_name, LoggingInitOptions::default())
}

pub fn init_logging_with(app_name: &str, opts: LoggingInitOptions) -> EnvrResult<LoggingGuard> {
    let log_dir = resolve_log_dir()?;

    fs::create_dir_all(&log_dir).map_err(EnvrError::from)?;

    let file_appender = tracing_appender::rolling::daily(&log_dir, format!("{app_name}.log"));
    let (non_blocking, file_guard) = tracing_appender::non_blocking(file_appender);

    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(opts.default_filter.unwrap_or("info")));

    // Optional: emit `envr_cli_metrics` events as JSONL for CI / observed metrics aggregation.
    //
    // When `ENVR_CLI_METRICS_JSONL` is set, we write JSON objects (one per line) to that path.
    // This is intentionally separate from normal app logs so automation can consume it directly.
    //
    // Note: we build the layer inside each branch so type inference stays consistent for the
    // chosen console writer (stdout vs stderr).
    let metrics_path = env::var("ENVR_CLI_METRICS_JSONL")
        .ok()
        .filter(|p| !p.trim().is_empty());

    if opts.log_to_stderr {
        let (metrics_layer, metrics_guard) = if let Some(ref p) = metrics_path {
            let path = PathBuf::from(p);
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let file = fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
                .map_err(EnvrError::from)?;
            let (writer, guard) = tracing_appender::non_blocking(file);
            let layer = tracing_subscriber::fmt::layer()
                .json()
                .with_target(false)
                .with_current_span(false)
                .with_writer(BoxMakeWriter::new(writer))
                .with_filter(Targets::new().with_target("envr_cli_metrics", tracing::Level::INFO));
            (layer, Some(guard))
        } else {
            let (writer, guard) = tracing_appender::non_blocking(std::io::sink());
            let layer = tracing_subscriber::fmt::layer()
                .json()
                .with_target(false)
                .with_current_span(false)
                .with_writer(BoxMakeWriter::new(writer))
                .with_filter(Targets::new());
            (layer, Some(guard))
        };
        let file_layer = tracing_subscriber::fmt::layer()
            .with_ansi(false)
            .with_target(true)
            .with_writer(non_blocking.clone());
        tracing_subscriber::registry()
            .with(env_filter)
            .with(
                tracing_subscriber::fmt::layer()
                    .with_target(true)
                    .with_writer(std::io::stderr),
            )
            .with(file_layer)
            .with(metrics_layer)
            .try_init()
            .map_err(|err| {
                EnvrError::with_source(ErrorCode::Runtime, "failed to initialize logging", err)
            })?;
        Ok(LoggingGuard {
            _file_guard: file_guard,
            _metrics_guard: metrics_guard,
        })
    } else {
        let (metrics_layer, metrics_guard) = if let Some(ref p) = metrics_path {
            let path = PathBuf::from(p);
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let file = fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
                .map_err(EnvrError::from)?;
            let (writer, guard) = tracing_appender::non_blocking(file);
            let layer = tracing_subscriber::fmt::layer()
                .json()
                .with_target(false)
                .with_current_span(false)
                .with_writer(BoxMakeWriter::new(writer))
                .with_filter(Targets::new().with_target("envr_cli_metrics", tracing::Level::INFO));
            (layer, Some(guard))
        } else {
            let (writer, guard) = tracing_appender::non_blocking(std::io::sink());
            let layer = tracing_subscriber::fmt::layer()
                .json()
                .with_target(false)
                .with_current_span(false)
                .with_writer(BoxMakeWriter::new(writer))
                .with_filter(Targets::new());
            (layer, Some(guard))
        };
        let file_layer = tracing_subscriber::fmt::layer()
            .with_ansi(false)
            .with_target(true)
            .with_writer(non_blocking.clone());
        tracing_subscriber::registry()
            .with(env_filter)
            .with(
                tracing_subscriber::fmt::layer()
                    .with_target(false)
                    .with_writer(std::io::stdout),
            )
            .with(file_layer)
            .with(metrics_layer)
            .try_init()
            .map_err(|err| {
                EnvrError::with_source(ErrorCode::Runtime, "failed to initialize logging", err)
            })?;
        Ok(LoggingGuard {
            _file_guard: file_guard,
            _metrics_guard: metrics_guard,
        })
    }
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
    use super::{LoggingInitOptions, format_error_chain, init_logging, init_logging_with};
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

        let _guard =
            init_logging_with("envr-core-test", LoggingInitOptions::default()).expect("first init");
        let err = match init_logging("envr-core-test") {
            Ok(_) => panic!("second init should fail"),
            Err(e) => e,
        };
        assert!(err.to_string().contains("failed to initialize logging"));
        std::env::set_current_dir(old).expect("restore cwd");
    }
}
