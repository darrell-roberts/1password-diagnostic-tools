//! Error types for the diagnostic parser.

use std::path::PathBuf;

/// Errors that can occur when parsing a `.1pdiagnostics` file.
#[derive(Debug, thiserror::Error)]
pub enum DiagnosticError {
    /// An I/O error occurred while reading the file.
    #[error("failed to read diagnostic file {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },

    /// The JSON content could not be deserialized.
    #[error("failed to parse diagnostic JSON: {0}")]
    Json(#[from] serde_json::Error),

    /// A log line could not be parsed into a structured log entry.
    #[error("failed to parse log line: {line}")]
    LogParse { line: String },

    /// A timestamp value could not be interpreted.
    #[error("failed to parse timestamp: {value}")]
    TimestampParse { value: String },
}

/// Convenience type alias for results returned by this crate.
pub type Result<T> = std::result::Result<T, DiagnosticError>;
