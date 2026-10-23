//! Typed Host failures retain Java exception-message units until rendering.
use crate::value::{PklString, RuntimeError};
use std::sync::Arc;

/// The supported base `java.io.IOException` failure. A missing message and an
/// empty message are distinct. Other Java exception classes are not relabeled
/// as this type; they require their own supported Host failure variant.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IoFailure {
    pub message: Option<PklString>,
}

impl IoFailure {
    pub fn new(message: Option<PklString>) -> Self {
        Self { message }
    }

    pub(crate) fn into_runtime(self, operation: &str, uri: &str) -> RuntimeError {
        let headline: PklString = format!("I/O error {operation} `{uri}`.").into();
        let message = match self.message {
            Some(cause) if cause != headline => {
                headline.concat(&"\nIOException: ".into()).concat(&cause)
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
}

impl ReadFailure {
    pub(crate) fn into_runtime(self, uri: &str) -> RuntimeError {
        match self {
            Self::Io(failure) => failure.into_runtime("reading resource", uri),
            Self::SecurityManagerException { message } => RuntimeError::host_language(
                message.unwrap_or_else(|| "None (cause has no message)".into()),
            ),
        }
    }
}
