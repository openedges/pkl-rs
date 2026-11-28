//! Error types for the public `pkl` Rust evaluator API.

use std::fmt;

/// Categorizes errors returned by the `pkl` API.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
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

impl fmt::Display for PklErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Evaluation => "evaluation",
            Self::Security => "security",
            Self::Timeout => "timeout",
            Self::Closed => "closed",
            Self::Cancelled => "cancelled",
            Self::Configuration => "configuration",
            Self::Decode => "decode",
            Self::Io => "io",
            Self::InvalidArgument => "invalid-argument",
        };
        f.write_str(name)
    }
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

    pub fn is_cancelled(&self) -> bool {
        self.kind == PklErrorKind::Cancelled
    }

    pub fn is_security(&self) -> bool {
        self.kind == PklErrorKind::Security
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

pub(crate) fn format_module_host_error(err: &pkl_runtime::modules::ModuleHostError) -> String {
    use pkl_runtime::modules::ModuleHostError;
    match err {
        ModuleHostError::NotFound => "Module not found.".to_string(),
        ModuleHostError::NoLoader => "No module loader is registered for this URI.".to_string(),
        ModuleHostError::UriSyntax { reason } => {
            format!("Invalid module URI syntax: {}", reason.to_java_utf8())
        }
        ModuleHostError::ExternalReaderProcess { message } => match message {
            Some(msg) => format!(
                "Failed to communicate with external reader process: {}",
                msg.to_java_utf8()
            ),
            None => "Failed to communicate with external reader process.".to_string(),
        },
        ModuleHostError::Io(io) => match &io.message {
            Some(msg) => msg.to_java_utf8(),
            None => "I/O error loading module.".to_string(),
        },
        ModuleHostError::Denied(msg) | ModuleHostError::Unsupported(msg) => msg.to_java_utf8(),
        ModuleHostError::SecurityManagerException { message } => message
            .as_ref()
            .map(pkl_runtime::value::PklString::to_java_utf8)
            .unwrap_or_else(|| "Module access denied by security manager.".to_string()),
        ModuleHostError::HttpClientException { message } => message
            .as_ref()
            .map(pkl_runtime::value::PklString::to_java_utf8)
            .unwrap_or_else(|| "HTTP client error loading module.".to_string()),
        ModuleHostError::EvalError { message, hint } => match hint {
            Some(hint) => format!("{}\n{}", message.to_java_utf8(), hint.to_java_utf8()),
            None => message.to_java_utf8(),
        },
    }
}

pub(crate) fn format_read_failure(err: &pkl_runtime::host::ReadFailure) -> String {
    use pkl_runtime::host::ReadFailure;
    match err {
        ReadFailure::Io(io) => match &io.message {
            Some(msg) => msg.to_java_utf8(),
            None => "I/O error reading resource.".to_string(),
        },
        ReadFailure::SecurityManagerException { message } => message
            .as_ref()
            .map(pkl_runtime::value::PklString::to_java_utf8)
            .unwrap_or_else(|| "Resource access denied by security manager.".to_string()),
        ReadFailure::HttpClientException { message } => message
            .as_ref()
            .map(pkl_runtime::value::PklString::to_java_utf8)
            .unwrap_or_else(|| "HTTP client error reading resource.".to_string()),
        ReadFailure::ExternalReaderProcess { message } => message
            .as_ref()
            .map(pkl_runtime::value::PklString::to_java_utf8)
            .unwrap_or_else(|| {
                "Failed to communicate with external resource reader process.".to_string()
            }),
        ReadFailure::UriSyntax { reason } => {
            format!("Invalid resource URI syntax: {}", reason.to_java_utf8())
        }
        ReadFailure::EvalError { message, hint } => match hint {
            Some(hint) => format!("{}\n{}", message.to_java_utf8(), hint.to_java_utf8()),
            None => message.to_java_utf8(),
        },
    }
}

pub(crate) fn format_analysis_failure(
    failure: &pkl_runtime::modules::analyze::AnalysisFailure,
) -> String {
    use pkl_runtime::modules::analyze::{FailureCause, UriFailure};
    let uri = failure.module_uri.to_java_utf8();
    match &failure.cause {
        FailureCause::Host(err) => {
            format!(
                "Failed to analyze module `{uri}`: {}",
                format_module_host_error(err)
            )
        }
        FailureCause::Syntax(diag) => {
            format!(
                "Syntax error while analyzing module `{uri}`: {}",
                diag.message.to_java_utf8()
            )
        }
        FailureCause::Uri { input, reason } => {
            let input_str = input.to_java_utf8();
            let detail = match reason {
                UriFailure::Relative => "URI must be absolute.".to_string(),
                UriFailure::Syntax { reason, .. } => reason.to_java_utf8(),
                UriFailure::Unsupported(msg) => msg.to_java_utf8(),
            };
            format!("Invalid URI `{input_str}` while analyzing `{uri}`: {detail}")
        }
        FailureCause::Unsupported(msg) => {
            format!(
                "Unsupported import construct while analyzing `{uri}`: {}",
                msg.to_java_utf8()
            )
        }
    }
}
