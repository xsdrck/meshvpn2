//! Error handling for the MeshVPN crate

use thiserror::Error;

/// The result type for MeshVPN operations
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// The main error type for MeshVPN
#[derive(Debug, Error)]
pub enum Error {
    /// An I/O error occurred
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// A configuration error
    #[error("Configuration error: {0}")]
    Config(String),

    /// A cryptography error
    #[error("Cryptography error: {0}")]
    Crypto(String),

    /// A network error
    #[error("Network error: {0}")]
    Network(String),

    /// A NAT traversal error
    #[error("NAT traversal error: {0}")]
    NatTraversal(String),

    /// A tunnel error
    #[error("Tunnel error: {0}")]
    Tunnel(String),

    /// A protocol error
    #[error("Protocol error: {0}")]
    Protocol(String),

    /// A serializing/deserializing error
    #[error("Serialization error: {0}")]
    Serialization(#[from] bincode::Error),

    /// A JSON serialization error
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// An invalid argument
    #[error("Invalid argument: {0}")]
    InvalidArgument(String),

    /// The operation timed out
    #[error("Operation timed out")]
    Timeout,

    /// The operation was cancelled
    #[error("Operation cancelled")]
    Cancelled,

    /// Permission denied
    #[error("Permission denied")]
    PermissionDenied,

    /// Resource not found
    #[error("Resource not found: {0}")]
    NotFound(String),

    /// Feature not implemented
    #[error("Feature not implemented: {0}")]
    NotImplemented(String),

    /// Internal error
    #[error("Internal error: {0}")]
    Internal(String),

    /// Other error
    #[error("{0}")]
    Other(#[from] anyhow::Error),
}

impl From<chacha20poly1305::Error> for Error {
    fn from(err: chacha20poly1305::Error) -> Self {
        Error::Crypto(format!("Encryption/decryption failed: {}", err))
    }
}

impl From<tokio::time::error::Elapsed> for Error {
    fn from(_: tokio::time::error::Elapsed) -> Self {
        Error::Timeout
    }
}

impl Error {
    /// Create a new config error
    pub fn config(msg: impl Into<String>) -> Self {
        Error::Config(msg.into())
    }

    /// Create a new crypto error
    pub fn crypto(msg: impl Into<String>) -> Self {
        Error::Crypto(msg.into())
    }

    /// Create a new network error
    pub fn network(msg: impl Into<String>) -> Self {
        Error::Network(msg.into())
    }

    /// Create a new NAT traversal error
    pub fn nat(msg: impl Into<String>) -> Self {
        Error::NatTraversal(msg.into())
    }

    /// Create a new tunnel error
    pub fn tunnel(msg: impl Into<String>) -> Self {
        Error::Tunnel(msg.into())
    }

    /// Create a new protocol error
    pub fn protocol(msg: impl Into<String>) -> Self {
        Error::Protocol(msg.into())
    }

    /// Create a new invalid argument error
    pub fn invalid_argument(msg: impl Into<String>) -> Self {
        Error::InvalidArgument(msg.into())
    }

    /// Create a new not found error
    pub fn not_found(msg: impl Into<String>) -> Self {
        Error::NotFound(msg.into())
    }

    /// Create a new internal error
    pub fn internal(msg: impl Into<String>) -> Self {
        Error::Internal(msg.into())
    }

    /// Check if the error is a timeout
    pub fn is_timeout(&self) -> bool {
        matches!(self, Error::Timeout)
    }

    /// Check if the error is due to permission denied
    pub fn is_permission_denied(&self) -> bool {
        matches!(self, Error::PermissionDenied)
    }

    /// Check if the error is a connection error
    pub fn is_connection_error(&self) -> bool {
        matches!(
            self,
            Error::Network(_) | Error::Timeout | Error::NatTraversal(_)
        )
    }
}
