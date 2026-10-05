use std::path::Path;

use serde::Serialize;

/// Stable error codes. The UI translates known codes; MCP returns them as-is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    NotFound,
    InvalidInput,
    Conflict,
    Validation,
    Toolchain,
    CommandFailed,
    NotAllowed,
    Io,
    Internal,
}

impl ErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotFound => "not_found",
            Self::InvalidInput => "invalid_input",
            Self::Conflict => "conflict",
            Self::Validation => "validation",
            Self::Toolchain => "toolchain",
            Self::CommandFailed => "command_failed",
            Self::NotAllowed => "not_allowed",
            Self::Io => "io",
            Self::Internal => "internal",
        }
    }
}

#[derive(Debug, Clone, thiserror::Error, Serialize)]
#[error("{message}")]
pub struct Error {
    pub code: ErrorCode,
    pub message: String,
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

impl Error {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self { code, message: message.into() }
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::NotFound, message)
    }

    pub fn invalid(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::InvalidInput, message)
    }

    pub fn conflict(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Conflict, message)
    }

    pub fn validation(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Validation, message)
    }

    pub fn command(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::CommandFailed, message)
    }

    pub fn not_allowed(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::NotAllowed, message)
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Internal, message)
    }

    pub fn io(path: &Path, err: std::io::Error) -> Self {
        Self::new(ErrorCode::Io, format!("{}: {err}", path.display()))
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Self::new(ErrorCode::Io, err.to_string())
    }
}

impl From<rusqlite::Error> for Error {
    fn from(err: rusqlite::Error) -> Self {
        Self::internal(format!("database: {err}"))
    }
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Self::invalid(err.to_string())
    }
}

/// Adds the path to I/O errors: `fs::read(&p).at(&p)?`.
pub trait IoContext<T> {
    fn at(self, path: &Path) -> Result<T>;
}

impl<T> IoContext<T> for std::io::Result<T> {
    fn at(self, path: &Path) -> Result<T> {
        self.map_err(|err| Error::io(path, err))
    }
}
