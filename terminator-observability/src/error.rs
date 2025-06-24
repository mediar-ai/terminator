//! Error types for Terminator Observability

use thiserror::Error;

/// Result type alias for Terminator Observability operations
pub type Result<T> = std::result::Result<T, Error>;

/// Error types that can occur in Terminator Observability
#[derive(Debug, Error)]
pub enum Error {
    /// Error from the underlying Terminator SDK
    #[error("Terminator SDK error: {0}")]
    TerminatorError(String),

    /// Error initializing telemetry
    #[error("Telemetry initialization error: {0}")]
    TelemetryInit(String),

    /// Error exporting telemetry data
    #[error("Telemetry export error: {0}")]
    TelemetryExport(String),

    /// Error with metrics collection
    #[error("Metrics error: {0}")]
    MetricsError(String),

    /// Storage backend error
    #[cfg(feature = "sqlite")]
    #[error("Storage error: {0}")]
    StorageError(#[from] sqlx::Error),

    /// Configuration error
    #[error("Configuration error: {0}")]
    ConfigError(String),

    /// Session error
    #[error("Session error: {0}")]
    SessionError(String),

    /// IO error
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    /// Serialization error
    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    /// Generic error with context
    #[error("{context}: {source}")]
    WithContext {
        context: String,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    /// Other errors
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl Error {
    /// Create an error with additional context
    pub fn with_context<E>(context: impl Into<String>, source: E) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self::WithContext {
            context: context.into(),
            source: Box::new(source),
        }
    }

    /// Convert a Terminator SDK error
    pub fn from_terminator(err: terminator::AutomationError) -> Self {
        Self::TerminatorError(err.to_string())
    }
}

// Implement From for common error conversions
impl From<terminator::AutomationError> for Error {
    fn from(err: terminator::AutomationError) -> Self {
        Self::from_terminator(err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = Error::ConfigError("Invalid service name".to_string());
        assert_eq!(err.to_string(), "Configuration error: Invalid service name");
    }

    #[test]
    fn test_error_with_context() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let err = Error::with_context("Failed to load baseline", io_err);

        assert!(err.to_string().contains("Failed to load baseline"));
        assert!(err.to_string().contains("file not found"));
    }
}
