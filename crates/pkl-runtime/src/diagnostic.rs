//! Structured diagnostics before product-specific rendering.
use crate::source::{SourceId, Span};
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticKind {
    Syntax,
    Unsupported,
    DuplicateMember,
    IntegerRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LogicalFrame {
    pub source: SourceId,
    pub span: Span,
    pub member: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Diagnostic {
    pub kind: DiagnosticKind,
    pub span: Span,
    pub message: String,
    /// Synthetic JVM underline width (not a source byte length), e.g. at EOF.
    pub highlight_length_utf16: Option<usize>,
    pub frames: Vec<LogicalFrame>,
}

impl Diagnostic {
    pub fn new(kind: DiagnosticKind, span: Span, message: impl Into<String>) -> Self {
        Self {
            kind,
            span,
            message: message.into(),
            highlight_length_utf16: None,
            frames: Vec::new(),
        }
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for Diagnostic {}
