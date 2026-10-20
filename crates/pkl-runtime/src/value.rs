//! Values shared by native and hosted experiments. Object handles belong to one evaluator.

use std::fmt;
use std::sync::Arc;

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
}

impl Value {
    pub fn string(value: impl Into<Arc<str>>) -> Self {
        Self::String(value.into())
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

/// Structured experiment error, deliberately not a claim of Java diagnostic formatting.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeError {
    pub kind: ErrorKind,
    pub message: String,
}

impl RuntimeError {
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for RuntimeError {}
