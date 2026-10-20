//! Values shared by native and hosted experiments. Object handles belong to one evaluator.

use std::fmt;
use std::sync::Arc;

use crate::source::Span;

/// A generation prevents a reclaimed slot from aliasing its next occupant.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjectId {
    pub(crate) heap: u64,
    pub(crate) index: usize,
    pub(crate) generation: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(Arc<str>),
    Bytes(Arc<[u8]>),
    Object(ObjectId),
    Function(ObjectId),
}

impl Value {
    pub fn string(value: impl Into<Arc<str>>) -> Self {
        Self::String(value.into())
    }

    /// Heap identity for every traced value variant.
    pub fn heap_id(&self) -> Option<ObjectId> {
        match self {
            Self::Object(id) | Self::Function(id) => Some(*id),
            _ => None,
        }
    }

    pub fn as_object(&self) -> Result<ObjectId, RuntimeError> {
        match self {
            Self::Object(id) => Ok(*id),
            _ => Err(RuntimeError::new(ErrorKind::Type, "expected an object")),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    Type,
    MissingMember,
    Cycle,
    Arithmetic,
    InvalidBytecode,
    StaleHandle,
    Busy,
    Cancelled,
    Timeout,
    Closed,
    Host,
    Unsupported,
}

/// A logical language frame, independent of Rust call-stack layout.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeFrame {
    pub span: Span,
    pub member: Arc<str>,
}

/// Runtime failures carry authored source locations separately from their message.
/// Rendering belongs to the evaluation boundary, which owns the source and URI.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeError {
    pub kind: ErrorKind,
    pub message: String,
    pub span: Option<Span>,
    pub frames: Vec<RuntimeFrame>,
    pub hint: Option<String>,
}

impl RuntimeError {
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            span: None,
            frames: Vec::new(),
            hint: None,
        }
    }

    pub fn at(mut self, span: Span) -> Self {
        if self.span.is_none() {
            self.span = Some(span);
        }
        self
    }
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for RuntimeError {}
