//! Custom error types for thrust.
//!
//! This module provides a unified error type for all thrust operations,
//! replacing generic `Box<dyn std::error::Error>` with a concrete enum
//! that enables better error handling, logging, and debugging.

use std::fmt;

/// A unified error type for thrust operations.
///
/// This enum represents all possible error conditions that can occur
/// when parsing aviation data, querying databases, or performing I/O operations.
///
/// # Examples
///
/// ```rust
/// use thrust::ThrustError;
///
/// let err = ThrustError::ParseError("Invalid altitude format".to_string());
/// assert!(matches!(err, ThrustError::ParseError(_)));
/// ```
#[derive(Debug, Clone)]
pub enum ThrustError {
    /// Parsing error with descriptive message
    ParseError(String),

    /// File not found or inaccessible
    FileNotFound(String),

    /// Invalid or corrupted data
    InvalidData(String),

    /// I/O error (read, write, seek, etc.)
    Io(String),

    /// ZIP file processing error
    ZipError(String),

    /// XML parsing or validation error
    XmlError(String),

    /// CSV parsing error
    CsvError(String),

    /// HTTP/network error
    NetworkError(String),

    /// Database operation error
    DatabaseError(String),

    /// Missing required field or data
    MissingField(String),

    /// Generic error with context
    Other(String),
}

impl fmt::Display for ThrustError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ParseError(msg) => write!(f, "Parse error: {}", msg),
            Self::FileNotFound(msg) => write!(f, "File not found: {}", msg),
            Self::InvalidData(msg) => write!(f, "Invalid data: {}", msg),
            Self::Io(msg) => write!(f, "I/O error: {}", msg),
            Self::ZipError(msg) => write!(f, "ZIP error: {}", msg),
            Self::XmlError(msg) => write!(f, "XML error: {}", msg),
            Self::CsvError(msg) => write!(f, "CSV error: {}", msg),
            Self::NetworkError(msg) => write!(f, "Network error: {}", msg),
            Self::DatabaseError(msg) => write!(f, "Database error: {}", msg),
            Self::MissingField(msg) => write!(f, "Missing field: {}", msg),
            Self::Other(msg) => write!(f, "Error: {}", msg),
        }
    }
}

impl std::error::Error for ThrustError {}

// Conversion implementations for common error types

impl From<std::io::Error> for ThrustError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e.to_string())
    }
}

impl From<csv::Error> for ThrustError {
    fn from(e: csv::Error) -> Self {
        Self::CsvError(e.to_string())
    }
}

impl From<zip::result::ZipError> for ThrustError {
    fn from(e: zip::result::ZipError) -> Self {
        Self::ZipError(e.to_string())
    }
}

impl From<serde_json::Error> for ThrustError {
    fn from(e: serde_json::Error) -> Self {
        Self::InvalidData(e.to_string())
    }
}

impl From<chrono::ParseError> for ThrustError {
    fn from(e: chrono::ParseError) -> Self {
        Self::ParseError(e.to_string())
    }
}

impl From<quick_xml::Error> for ThrustError {
    fn from(e: quick_xml::Error) -> Self {
        Self::XmlError(e.to_string())
    }
}

impl From<std::str::Utf8Error> for ThrustError {
    fn from(e: std::str::Utf8Error) -> Self {
        Self::ParseError(e.to_string())
    }
}

impl From<std::num::ParseFloatError> for ThrustError {
    fn from(e: std::num::ParseFloatError) -> Self {
        Self::ParseError(e.to_string())
    }
}

// For quick_xml attribute errors
impl<'a> From<quick_xml::events::attributes::AttrError> for ThrustError {
    fn from(e: quick_xml::events::attributes::AttrError) -> Self {
        Self::XmlError(e.to_string())
    }
}

// For quick_xml encoding errors
#[cfg(feature = "encoding")]
impl From<encoding_rs::EncodingError> for ThrustError {
    fn from(_e: encoding_rs::EncodingError) -> Self {
        Self::XmlError("Encoding error".to_string())
    }
}

#[cfg(feature = "net")]
impl From<reqwest::Error> for ThrustError {
    fn from(e: reqwest::Error) -> Self {
        Self::NetworkError(e.to_string())
    }
}

impl From<&str> for ThrustError {
    fn from(e: &str) -> Self {
        Self::Other(e.to_string())
    }
}

impl From<String> for ThrustError {
    fn from(e: String) -> Self {
        Self::Other(e)
    }
}
