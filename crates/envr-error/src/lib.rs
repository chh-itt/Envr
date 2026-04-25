use serde::{Deserialize, Serialize};
use std::{error::Error, io};
use thiserror::Error;

pub type EnvrResult<T> = Result<T, EnvrError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    Unknown,
    Io,
    Config,
    Validation,
    Runtime,
    Platform,
    Download,
    Mirror,
    RuntimeVersionSpecInvalid,
    RuntimeVersionNotFound,
    RemoteIndexFetchFailed,
    RemoteIndexParseFailed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorPayload {
    pub code: ErrorCode,
    pub message: String,
    pub chain: Vec<String>,
}

#[derive(Debug, Error)]
pub enum EnvrError {
    #[error("i/o error: {0}")]
    Io(#[from] io::Error),
    #[error("config error: {0}")]
    Config(String),
    #[error("validation error: {0}")]
    Validation(String),
    #[error("runtime error: {0}")]
    Runtime(String),
    #[error("platform error: {0}")]
    Platform(String),
    #[error("download error: {0}")]
    Download(String),
    #[error("mirror error: {0}")]
    Mirror(String),
    #[error("{message}")]
    Context {
        code: ErrorCode,
        message: String,
        #[source]
        source: Box<dyn Error + Send + Sync>,
    },
    #[error("unknown error: {0}")]
    Unknown(String),
}

impl EnvrError {
    pub fn code(&self) -> ErrorCode {
        match self {
            Self::Io(_) => ErrorCode::Io,
            Self::Config(_) => ErrorCode::Config,
            Self::Validation(_) => ErrorCode::Validation,
            Self::Runtime(_) => ErrorCode::Runtime,
            Self::Platform(_) => ErrorCode::Platform,
            Self::Download(_) => ErrorCode::Download,
            Self::Mirror(_) => ErrorCode::Mirror,
            Self::Context { code, .. } => *code,
            Self::Unknown(_) => ErrorCode::Unknown,
        }
    }

    /// Attach context while preserving the original error as `source`.
    pub fn context(self, message: impl Into<String>) -> Self {
        let code = self.code();
        Self::Context {
            code,
            message: message.into(),
            source: Box::new(self),
        }
    }

    /// Create an error with explicit `code` and external source chain.
    pub fn with_source(
        code: ErrorCode,
        message: impl Into<String>,
        source: impl Error + Send + Sync + 'static,
    ) -> Self {
        Self::Context {
            code,
            message: message.into(),
            source: Box::new(source),
        }
    }

    pub fn to_payload(&self) -> ErrorPayload {
        let mut chain = Vec::new();
        let mut current = self.source();

        while let Some(source) = current {
            chain.push(source.to_string());
            current = source.source();
        }

        ErrorPayload {
            code: self.code(),
            message: self.to_string(),
            chain,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_code_is_serializable() {
        let code = ErrorCode::Validation;
        let serialized = serde_json::to_string(&code).expect("serialize error code");
        assert_eq!(serialized, "\"validation\"");
    }

    #[test]
    fn io_error_converts_to_envr_error() {
        let io_error = io::Error::new(io::ErrorKind::NotFound, "missing");
        let err: EnvrError = io_error.into();
        assert_eq!(err.code(), ErrorCode::Io);
    }

    #[test]
    fn all_error_codes_map_and_serialize() {
        let cases = [
            (
                EnvrError::Config("c".into()),
                ErrorCode::Config,
                "\"config\"",
            ),
            (
                EnvrError::Validation("v".into()),
                ErrorCode::Validation,
                "\"validation\"",
            ),
            (
                EnvrError::Runtime("r".into()),
                ErrorCode::Runtime,
                "\"runtime\"",
            ),
            (
                EnvrError::Platform("p".into()),
                ErrorCode::Platform,
                "\"platform\"",
            ),
            (
                EnvrError::Download("d".into()),
                ErrorCode::Download,
                "\"download\"",
            ),
            (
                EnvrError::Mirror("m".into()),
                ErrorCode::Mirror,
                "\"mirror\"",
            ),
            (
                EnvrError::Unknown("u".into()),
                ErrorCode::Unknown,
                "\"unknown\"",
            ),
        ];
        for (err, code, json) in cases {
            assert_eq!(err.code(), code);
            assert_eq!(serde_json::to_string(&code).expect("ser"), json);
            let payload = err.to_payload();
            assert_eq!(payload.code, code);
            let needle = match &err {
                EnvrError::Config(_) => "config error",
                EnvrError::Validation(_) => "validation error",
                EnvrError::Runtime(_) => "runtime error",
                EnvrError::Platform(_) => "platform error",
                EnvrError::Download(_) => "download error",
                EnvrError::Mirror(_) => "mirror error",
                EnvrError::Context { .. } => "context",
                EnvrError::Unknown(_) => "unknown error",
                EnvrError::Io(_) => "i/o error",
            };
            if !matches!(err, EnvrError::Context { .. }) {
                assert!(payload.message.contains(needle));
                assert!(payload.chain.is_empty());
            }
        }
    }

    #[test]
    fn context_preserves_error_chain() {
        let base = EnvrError::Download("request failed".into());
        let err = base.context("downloading runtime archive");
        let payload = err.to_payload();
        assert_eq!(payload.code, ErrorCode::Download);
        assert!(payload.message.contains("downloading runtime archive"));
        assert!(payload.chain.iter().any(|x| x.contains("download error")));
    }
}
