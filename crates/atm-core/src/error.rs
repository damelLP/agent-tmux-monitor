//! Domain-specific error types following panic-free policy.

use crate::SessionId;
use thiserror::Error;

/// Errors that can occur in domain operations.
#[derive(Error, Debug, Clone)]
pub enum DomainError {
    /// Session not found in registry
    #[error("Session not found: {session_id}")]
    SessionNotFound { session_id: SessionId },

    /// Session already exists
    #[error("Session already exists: {session_id}")]
    SessionAlreadyExists { session_id: SessionId },

    /// Invalid field value
    #[error("Invalid {field}: {value} (expected {expected})")]
    InvalidFieldValue {
        field: String,
        value: String,
        expected: String,
    },

    /// Parse error for incoming data
    #[error("Failed to parse {field}: {reason}")]
    ParseError { field: String, reason: String },
}

/// Result type for domain operations.
pub type DomainResult<T> = Result<T, DomainError>;
