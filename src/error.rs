//! Error types for the AiSEG2 to InfluxDB2 forwarder.
//!
//! This module defines typed errors for different components of the application,
//! providing better error categorization and enabling specific error handling strategies.

use thiserror::Error;

/// Result type alias using our custom error types.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Top-level error type that encompasses all application errors.
#[derive(Error, Debug)]
pub enum Error {
    /// Configuration-related errors
    #[error("configuration error")]
    Config(#[from] ConfigError),

    /// AiSEG2 communication and parsing errors
    #[error("AiSEG2 error")]
    Aiseg(#[from] AisegError),

    /// Metric collection errors
    #[error("collector error")]
    Collector(#[from] CollectorError),

    /// InfluxDB storage errors
    #[error("storage error")]
    Storage(#[from] StorageError),

    /// Generic errors that don't fit other categories
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// Configuration-related errors.
#[derive(Error, Debug)]
pub enum ConfigError {
    /// Environment variable parsing failed
    #[error("failed to parse environment variables: {0}")]
    EnvParse(String),

    /// Required configuration value is missing
    #[error("missing required configuration: {0}")]
    Missing(String),

    /// Configuration value is invalid
    #[error("invalid configuration value for {field}: {message}")]
    Invalid { field: String, message: String },
}

/// AiSEG2 communication and parsing errors.
#[derive(Error, Debug)]
pub enum AisegError {
    /// HTTP request failed
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    /// Diqwest (digest auth) request failed
    #[error("Digest auth request failed: {0}")]
    DigestAuth(String),

    /// Authentication failed (401)
    #[error("authentication failed: invalid credentials")]
    AuthFailed,

    /// Server returned an error status
    #[error("server error (status {status}): {message}")]
    ServerError { status: u16, message: String },

    /// HTML parsing failed
    #[error("HTML parsing error")]
    Parse(#[from] ParseError),

    /// Network timeout
    #[error("request timed out after {0} seconds")]
    Timeout(u64),

    /// Rate limit exceeded
    #[error("rate limit exceeded, retry after {0} seconds")]
    RateLimit(u64),
}

/// HTML parsing errors.
#[derive(Error, Debug)]
pub enum ParseError {
    /// Element not found in HTML
    #[error("element not found: {selector}")]
    ElementNotFound { selector: String },

    /// Invalid CSS selector
    #[error("invalid selector '{selector}': {message}")]
    InvalidSelector { selector: String, message: String },

    /// Failed to parse numeric value
    #[error("failed to parse number from '{text}': {message}")]
    NumberParse { text: String, message: String },

    /// Failed to parse date/time
    #[error("failed to parse date/time from '{text}': {message}")]
    DateTimeParse { text: String, message: String },

    /// Expected element has no content
    #[error("element '{selector}' has no content")]
    EmptyElement { selector: String },

    /// Unexpected HTML structure
    #[error("unexpected HTML structure: {0}")]
    UnexpectedStructure(String),
}

/// Metric collection errors.
#[derive(Error, Debug)]
pub enum CollectorError {
    /// Collector task timed out
    #[error("collector '{name}' timed out after {timeout} seconds")]
    Timeout { name: String, timeout: u64 },

    /// Circuit breaker is open
    #[error("circuit breaker open for collector '{name}'")]
    CircuitOpen { name: String },

    /// Data source error
    #[error("failed to collect from source")]
    Source(#[from] AisegError),

    /// Data validation failed
    #[error("invalid metric data: {0}")]
    ValidationFailed(String),

    /// Collector is temporarily unavailable
    #[error("collector '{name}' is temporarily unavailable")]
    Unavailable { name: String },
}

/// InfluxDB storage errors.
#[derive(Error, Debug)]
pub enum StorageError {
    /// InfluxDB client error
    #[error("InfluxDB error: {0}")]
    Client(#[from] influxdb2::RequestError),

    /// Authentication failed
    #[error("InfluxDB authentication failed")]
    AuthFailed,

    /// Write operation failed
    #[error("failed to write {count} data points: {message}")]
    WriteFailed { count: usize, message: String },

    /// Connection failed
    #[error("failed to connect to InfluxDB at {url}")]
    ConnectionFailed { url: String },

    /// Invalid data point
    #[error("invalid data point: {0}")]
    InvalidDataPoint(String),
}

// Note: Our Error types automatically work with anyhow due to implementing std::error::Error

impl ConfigError {
    /// Creates a new environment parse error.
    pub fn env_parse(err: impl std::fmt::Display) -> Self {
        Self::EnvParse(err.to_string())
    }

    /// Creates a new missing configuration error.
    pub fn missing(field: impl Into<String>) -> Self {
        Self::Missing(field.into())
    }

    /// Creates a new invalid configuration error.
    pub fn invalid(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Invalid {
            field: field.into(),
            message: message.into(),
        }
    }
}

impl AisegError {
    /// Creates a server error from HTTP status and response body.
    pub fn server_error(status: reqwest::StatusCode, body: String) -> Self {
        if status.as_u16() == 401 {
            Self::AuthFailed
        } else if status.as_u16() == 429 {
            // Try to parse retry-after header value, default to 60 seconds
            Self::RateLimit(60)
        } else {
            Self::ServerError {
                status: status.as_u16(),
                message: body,
            }
        }
    }
}

impl ParseError {
    /// Creates an element not found error.
    pub fn element_not_found(selector: impl Into<String>) -> Self {
        Self::ElementNotFound {
            selector: selector.into(),
        }
    }

    /// Creates an invalid selector error.
    pub fn invalid_selector(selector: impl Into<String>, err: impl std::fmt::Display) -> Self {
        Self::InvalidSelector {
            selector: selector.into(),
            message: err.to_string(),
        }
    }

    /// Creates a number parse error.
    pub fn number_parse(text: impl Into<String>, err: impl std::fmt::Display) -> Self {
        Self::NumberParse {
            text: text.into(),
            message: err.to_string(),
        }
    }

    /// Creates a datetime parse error.
    pub fn datetime_parse(text: impl Into<String>, err: impl std::fmt::Display) -> Self {
        Self::DateTimeParse {
            text: text.into(),
            message: err.to_string(),
        }
    }
}

impl CollectorError {
    /// Creates a timeout error.
    pub fn timeout(name: impl Into<String>, timeout: u64) -> Self {
        Self::Timeout {
            name: name.into(),
            timeout,
        }
    }

    /// Creates a circuit open error.
    pub fn circuit_open(name: impl Into<String>) -> Self {
        Self::CircuitOpen { name: name.into() }
    }

    /// Creates an unavailable error.
    pub fn unavailable(name: impl Into<String>) -> Self {
        Self::Unavailable { name: name.into() }
    }
}

impl StorageError {
    /// Creates a write failed error.
    pub fn write_failed(count: usize, err: impl std::fmt::Display) -> Self {
        Self::WriteFailed {
            count,
            message: err.to_string(),
        }
    }

    /// Creates a connection failed error.
    pub fn connection_failed(url: impl Into<String>) -> Self {
        Self::ConnectionFailed { url: url.into() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod config_error {
        use super::*;

        #[test]
        fn test_env_parse_error() {
            let err = ConfigError::env_parse("invalid format");
            assert_eq!(err.to_string(), "failed to parse environment variables: invalid format");
        }

        #[test]
        fn test_missing_error() {
            let err = ConfigError::missing("DATABASE_URL");
            assert_eq!(err.to_string(), "missing required configuration: DATABASE_URL");
        }

        #[test]
        fn test_invalid_error() {
            let err = ConfigError::invalid("port", "must be a number");
            assert_eq!(err.to_string(), "invalid configuration value for port: must be a number");
        }
    }

    mod parse_error {
        use super::*;

        #[test]
        fn test_element_not_found() {
            let err = ParseError::element_not_found("#missing");
            assert_eq!(err.to_string(), "element not found: #missing");
        }

        #[test]
        fn test_number_parse() {
            let err = ParseError::number_parse("abc", "invalid digit");
            assert_eq!(err.to_string(), "failed to parse number from 'abc': invalid digit");
        }
    }

    mod collector_error {
        use super::*;

        #[test]
        fn test_timeout() {
            let err = CollectorError::timeout("PowerCollector", 30);
            assert_eq!(err.to_string(), "collector 'PowerCollector' timed out after 30 seconds");
        }

        #[test]
        fn test_circuit_open() {
            let err = CollectorError::circuit_open("ClimateCollector");
            assert_eq!(err.to_string(), "circuit breaker open for collector 'ClimateCollector'");
        }
    }

    mod storage_error {
        use super::*;

        #[test]
        fn test_write_failed() {
            let err = StorageError::write_failed(100, "network error");
            assert_eq!(err.to_string(), "failed to write 100 data points: network error");
        }

        #[test]
        fn test_connection_failed() {
            let err = StorageError::connection_failed("http://localhost:8086");
            assert_eq!(err.to_string(), "failed to connect to InfluxDB at http://localhost:8086");
        }
    }

    mod error_conversion {
        use super::*;

        #[test]
        fn test_config_error_conversion() {
            let config_err = ConfigError::missing("test");
            let err: Error = config_err.into();
            assert!(matches!(err, Error::Config(_)));
        }

        #[test]
        fn test_anyhow_conversion() {
            let err = Error::Config(ConfigError::missing("test"));
            let anyhow_err: anyhow::Error = err.into();
            assert!(anyhow_err.to_string().contains("configuration error"));
        }
    }
}