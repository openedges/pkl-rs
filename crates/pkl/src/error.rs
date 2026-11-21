//! Error types for the public `pkl` Rust evaluator API.

use std::fmt;

/// Categorizes errors returned by the `pkl` API.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PklErrorKind {
    /// Pkl parse, resolution, type, or runtime evaluation error (already formatted with
    /// stack frames and optional ANSI color codes).
    Evaluation,
    /// Security manager rejection (`SecurityManagerException`).
    Security,
    /// Evaluation exceeded its configured timeout duration.
    Timeout,
    /// Evaluator or HTTP client was already closed.
    Closed,
    /// Evaluation was cancelled via [`crate::CancelHandle`] or [`crate::Evaluator::close`].
    Cancelled,
    /// Invalid builder, settings, project, or URL configuration.
    Configuration,
    /// Binary or typed value decoding error.
    Decode,
    /// Filesystem or network I/O error.
    Io,
    /// Invalid argument passed to a Rust API function.
    InvalidArgument,
}

/// Public error type mirroring `org.pkl.core.PklException` and `SecurityManagerException`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PklError {
    kind: PklErrorKind,
    message: String,
}

impl PklError {
    pub fn new(kind: PklErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    pub fn evaluation(message: impl Into<String>) -> Self {
        Self::new(PklErrorKind::Evaluation, message)
    }

    pub fn security(message: impl Into<String>) -> Self {
        Self::new(PklErrorKind::Security, message)
    }

    pub fn timeout(message: impl Into<String>) -> Self {
        Self::new(PklErrorKind::Timeout, message)
    }

    pub fn closed(message: impl Into<String>) -> Self {
        Self::new(PklErrorKind::Closed, message)
    }

    pub fn cancelled(message: impl Into<String>) -> Self {
        Self::new(PklErrorKind::Cancelled, message)
    }

    pub fn configuration(message: impl Into<String>) -> Self {
        Self::new(PklErrorKind::Configuration, message)
    }

    pub fn decode(message: impl Into<String>) -> Self {
        Self::new(PklErrorKind::Decode, message)
    }

    pub fn io(message: impl Into<String>) -> Self {
        Self::new(PklErrorKind::Io, message)
    }

    pub fn invalid_argument(message: impl Into<String>) -> Self {
        Self::new(PklErrorKind::InvalidArgument, message)
    }

    pub fn kind(&self) -> PklErrorKind {
        self.kind
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn is_timeout(&self) -> bool {
        self.kind == PklErrorKind::Timeout
    }

    pub fn is_closed(&self) -> bool {
        self.kind == PklErrorKind::Closed
    }
}

impl fmt::Display for PklError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for PklError {}

impl From<std::io::Error> for PklError {
    fn from(err: std::io::Error) -> Self {
        Self::io(err.to_string())
    }
}

/// Convenience result alias for the `pkl` crate.
pub type Result<T> = std::result::Result<T, PklError>;
