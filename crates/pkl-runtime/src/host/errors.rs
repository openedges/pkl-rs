//! Typed Host failures retain Java exception-message units until rendering.
use crate::value::{PklString, RuntimeError};
use std::sync::Arc;

/// Supported Java `IOException` subclass simple names recognized by [`IoFailure::with_class`].
const KNOWN_IO_SUBCLASS_PREFIXES: &[&str] = &[
    "ConnectException: ",
    "SSLHandshakeException: ",
    "SSLException: ",
    "ZipException: ",
    "FileNotFoundException: ",
    "SocketTimeoutException: ",
];

/// The supported base `java.io.IOException` failure (and subclasses created via
/// [`IoFailure::with_class`]). A missing message and an empty message are distinct.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IoFailure {
    pub message: Option<PklString>,
}

impl IoFailure {
    pub fn new(message: Option<PklString>) -> Self {
        Self { message }
    }

    /// Creates an `IoFailure` retaining a specific Java `IOException` subclass simple name
    /// (such as `"SSLHandshakeException"`, `"ZipException"`, or `"ConnectException"`), so
    /// `VmExceptionRenderer` (`cause.getClass().getSimpleName() + ": " + causeMessage`)
    /// preserves the exact exception class name instead of relabeling it as `IOException`.
    pub fn with_class(class_name: &str, message: impl Into<PklString>) -> Self {
        let cause = message.into();
        let prefixed = PklString::from(format!("{class_name}: ")).concat(&cause);
        Self {
            message: Some(prefixed),
        }
    }

    pub(crate) fn into_runtime(self, operation: &str, uri: impl Into<PklString>) -> RuntimeError {
        let headline = PklString::from(format!("I/O error {operation} `"))
            .concat(&uri.into())
            .concat(&"`.".into());
        let message = match self.message {
            Some(cause) if cause != headline => {
                let has_known_class = cause.as_str().is_some_and(|s| {
                    KNOWN_IO_SUBCLASS_PREFIXES
                        .iter()
                        .any(|prefix| s.starts_with(prefix))
                });
                if has_known_class {
                    headline.concat(&"\n".into()).concat(&cause)
                } else {
                    headline.concat(&"\nIOException: ".into()).concat(&cause)
                }
            }
            _ => headline,
        };
        RuntimeError::host_language(message)
    }
}

impl From<PklString> for IoFailure {
    fn from(message: PklString) -> Self {
        Self::new(Some(message))
    }
}
impl From<&str> for IoFailure {
    fn from(message: &str) -> Self {
        PklString::from(message).into()
    }
}
impl From<String> for IoFailure {
    fn from(message: String) -> Self {
        PklString::from(message).into()
    }
}
impl From<Arc<str>> for IoFailure {
    fn from(message: Arc<str>) -> Self {
        PklString::from(message).into()
    }
}

/// Resource SPI failures with an implemented shared language mapping.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReadFailure {
    Io(IoFailure),
    /// Typed policy exception from ResourceManager's resource security check.
    /// This does not grant guest provenance to an arbitrary Host rejection.
    SecurityManagerException {
        message: Option<PklString>,
    },
    /// Typed `org.pkl.core.http.HttpClientException` rendered directly via `withCause(e)`.
    HttpClientException {
        message: Option<PklString>,
    },
    /// Typed `org.pkl.core.externalreader.ExternalReaderProcessException` rendered directly
    /// via `withCause(e)` (`ResourceManager.java:91-92`).
    ExternalReaderProcess {
        message: Option<PklString>,
    },
    UriSyntax {
        reason: PklString,
    },
    EvalError {
        message: PklString,
        hint: Option<PklString>,
    },
}

impl ReadFailure {
    pub(crate) fn into_runtime(self, uri: impl Into<PklString>) -> RuntimeError {
        let uri = uri.into();
        match self {
            Self::Io(failure) => failure.into_runtime("reading resource", uri),
            Self::SecurityManagerException { message }
            | Self::HttpClientException { message }
            | Self::ExternalReaderProcess { message } => RuntimeError::host_language(
                message.unwrap_or_else(|| "None (cause has no message)".into()),
            ),
            Self::UriSyntax { reason } => {
                let mut error = RuntimeError::host_language(
                    PklString::from("Resource URI `")
                        .concat(&uri)
                        .concat(&"` has invalid syntax.".into()),
                );
                error.hint = Some(reason);
                error
            }
            Self::EvalError { message, hint } => {
                let mut error = RuntimeError::host_language(message);
                error.hint = hint;
                error
            }
        }
    }
}
