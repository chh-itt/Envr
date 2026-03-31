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
            Self::Unknown(_) => ErrorCode::Unknown,
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
}
