//! Values shared by native and hosted experiments. Object handles belong to one evaluator.

use std::fmt;
use std::sync::Arc;

pub use crate::pkl_string::PklString;
use crate::source::Span;

/// A generation prevents a reclaimed slot from aliasing its next occupant.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjectId {
    pub(crate) heap: u64,
    pub(crate) index: usize,
    pub(crate) generation: u64,
}

#[derive(Clone, Debug)]
pub enum Value {
    Null,
    /// A language null retaining the default used when it is amended.
    NullWithDefault(ObjectId),
    Bool(bool),
    Int(i64),
    Float(f64),
    String(PklString),
    Bytes(Arc<[u8]>),
    Object(ObjectId),
    Function(ObjectId),
    Class(ObjectId),
    TypeAlias(ObjectId),
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (a, b) if a.is_null() && b.is_null() => true,
            (Self::Bool(a), Self::Bool(b)) => a == b,
            (Self::Int(a), Self::Int(b)) => a == b,
            (Self::Float(a), Self::Float(b)) => a == b,
            (Self::String(a), Self::String(b)) => a == b,
            (Self::Bytes(a), Self::Bytes(b)) => a == b,
            (Self::Object(a), Self::Object(b))
            | (Self::Function(a), Self::Function(b))
            | (Self::Class(a), Self::Class(b))
            | (Self::TypeAlias(a), Self::TypeAlias(b)) => a == b,
            _ => false,
        }
    }
}

impl Value {
    pub fn is_null(&self) -> bool {
        matches!(self, Self::Null | Self::NullWithDefault(_))
    }

    pub fn string(value: impl Into<PklString>) -> Self {
        Self::String(value.into())
    }

    /// Heap identity for every traced value variant.
    pub fn heap_id(&self) -> Option<ObjectId> {
        match self {
            Self::Object(id)
            | Self::Function(id)
            | Self::Class(id)
            | Self::TypeAlias(id)
            | Self::NullWithDefault(id) => Some(*id),
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
    MissingType,
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

/// Detached source provenance for frames outside the primary module.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeSource {
    pub text: Arc<str>,
    pub uri: Arc<str>,
    pub module_name: Arc<str>,
    /// Standard-library links use the displayed source line as a URL fragment.
    pub line_anchors: bool,
}

/// A logical language frame, independent of Rust call-stack layout.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeFrame {
    pub span: Span,
    pub member: PklString,
    /// None denotes the caller-provided module source at the public boundary.
    pub source: Option<Arc<RuntimeSource>>,
}

/// Runtime failures carry authored source locations separately from their message.
/// Rendering belongs to the evaluation boundary, which owns the source and URI.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeError {
    details: Option<Box<RuntimeErrorDetails>>,
    /// Internal exception provenance; unknown-member failures are never undefined.
    pub(crate) undefined_value: bool,
    pub kind: ErrorKind,
    pub message: PklString,
    pub span: Option<Span>,
    pub frames: Vec<RuntimeFrame>,
    pub hint: Option<PklString>,
}

/// Rare structured provenance is detached so the common Result error stays
/// compact. Field access remains available through RuntimeError's dereference.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RuntimeErrorDetails {
    /// A typed Host SPI mapping produced an ordinary guest exception.
    pub(crate) host_language_exception: bool,
    pub frontend: Option<Arc<crate::modules::ModuleDiagnostic>>,
    pub(crate) cached_module_frames: bool,
    pub(crate) module_failure_key: Option<Arc<str>>,
    pub spread_hint: Option<Box<Span>>,
    pub type_diagnostic: Option<Box<crate::diagnostic::TypeDiagnostic>>,
}
const EMPTY_ERROR_DETAILS: RuntimeErrorDetails = RuntimeErrorDetails {
    host_language_exception: false,
    frontend: None,
    cached_module_frames: false,
    module_failure_key: None,
    spread_hint: None,
    type_diagnostic: None,
};
impl std::ops::Deref for RuntimeError {
    type Target = RuntimeErrorDetails;
    fn deref(&self) -> &Self::Target {
        self.details.as_deref().unwrap_or(&EMPTY_ERROR_DETAILS)
    }
}
impl std::ops::DerefMut for RuntimeError {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.details.get_or_insert_with(Default::default)
    }
}

impl RuntimeError {
    pub fn new(kind: ErrorKind, message: impl Into<PklString>) -> Self {
        Self {
            details: None,
            undefined_value: false,
            kind,
            message: message.into(),
            span: None,
            frames: Vec::new(),
            hint: None,
        }
    }

    /// A shared, typed Host mapping has constructed an ordinary guest error.
    /// Raw Host/protocol failures do not acquire this provenance by default.
    pub(crate) fn host_language(message: impl Into<PklString>) -> Self {
        let mut error = Self::new(ErrorKind::Host, message);
        error.host_language_exception = true;
        error
    }

    pub(crate) fn undefined(mut self) -> Self {
        self.undefined_value = true;
        self
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
        formatter.write_str(&self.message.to_java_utf8())
    }
}

impl std::error::Error for RuntimeError {}
