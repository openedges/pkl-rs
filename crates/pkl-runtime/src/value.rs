//! Values shared by native and hosted experiments. Object handles belong to one evaluator.

use std::fmt;
use std::sync::Arc;

pub use crate::pkl_string::PklString;
use crate::source::Span;
pub use crate::value_identity::{Bytes, Number};

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
    Int(Number<i64>),
    Float(Number<f64>),
    Duration(crate::duration::Duration),
    DataSize(crate::data_size::DataSize),
    String(PklString),
    Bytes(Bytes),
    IntSeq(crate::int_seq::IntSeq),
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
            (Self::Duration(a), Self::Duration(b)) => a == b,
            (Self::DataSize(a), Self::DataSize(b)) => a == b,
            (Self::String(a), Self::String(b)) => a == b,
            (Self::Bytes(a), Self::Bytes(b)) => a == b,
            (Self::IntSeq(a), Self::IntSeq(b)) => a == b,
            (Self::Object(a), Self::Object(b))
            | (Self::Function(a), Self::Function(b))
            | (Self::Class(a), Self::Class(b))
            | (Self::TypeAlias(a), Self::TypeAlias(b)) => a == b,
            _ => false,
        }
    }
}

impl Value {
    pub(crate) fn boxed(self) -> Self {
        match self {
            Self::Int(value) => Self::Int(value.boxed()),
            Self::Float(value) => Self::Float(value.boxed()),
            value => value,
        }
    }

    pub(crate) fn reference(&self) -> Option<crate::value_identity::Reference> {
        use crate::value_identity::{Reference, Token};
        match self {
            Self::Int(value) => value.reference().map(Reference::Scalar),
            Self::Float(value) => value.reference().map(Reference::Scalar),
            Self::Duration(value) => Some(Reference::Scalar(value.reference())),
            Self::DataSize(value) => Some(Reference::Scalar(value.reference())),
            Self::Bytes(value) => Some(Reference::Scalar(value.reference())),
            Self::IntSeq(value) => Some(Reference::Scalar(value.reference())),
            Self::String(value) => Some(Reference::Scalar(value.reference())),
            Self::Bool(value) => Some(Reference::Scalar(Token::boolean(*value))),
            Self::Null => Some(Reference::Scalar(Token::null())),
            value => value.heap_id().map(Reference::Heap),
        }
    }

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
#[derive(Clone, Debug)]
pub struct RuntimeError {
    details: Option<Box<RuntimeErrorDetails>>,
    /// Internal exception provenance; unknown-member failures are never undefined.
    pub(crate) undefined_value: bool,
    pub kind: ErrorKind,
    pub message: PklString,
    pub span: Option<Span>,
    pub frames: Vec<RuntimeFrame>,
}

/// Rare structured provenance is detached so the common Result error stays
/// compact. Field access remains available through RuntimeError's dereference.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RuntimeErrorDetails {
    pub(crate) callback_origin: Option<crate::logging::CallbackOrigin>,
    /// Policy belongs to this throw, not to a cached declaration or source.
    pub(crate) frame_transformer: crate::logging::FrameTransformer,
    /// Completed Host diagnostic rendering, retaining the original UTF-16 units.
    pub(crate) rendered_diagnostic: Option<PklString>,
    /// Optional rendered hint; retained with rare provenance, outside the common error.
    pub hint: Option<PklString>,
    /// External ErrorMessages template paragraphs have priority over dynamic hints.
    pub(crate) hint_from_template: bool,
    /// A typed Host SPI mapping produced an ordinary guest exception.
    pub(crate) host_language_exception: bool,
    /// The throwing helper has neither a guest node nor a source section.
    /// Its innermost call-target frame is unavailable; caller frames remain.
    pub(crate) unavailable_origin: bool,
    pub frontend: Option<Arc<crate::modules::ModuleDiagnostic>>,
    pub(crate) cached_module_frames: bool,
    pub(crate) module_failure_id: Option<u64>,
    pub spread_hint: Option<Box<Span>>,
    pub type_diagnostic: Option<Box<crate::diagnostic::TypeDiagnostic>>,
}
const EMPTY_ERROR_DETAILS: RuntimeErrorDetails = RuntimeErrorDetails {
    callback_origin: None,
    frame_transformer: crate::logging::FrameTransformer::Default,
    rendered_diagnostic: None,
    hint: None,
    hint_from_template: false,
    host_language_exception: false,
    unavailable_origin: false,
    frontend: None,
    cached_module_frames: false,
    module_failure_id: None,
    spread_hint: None,
    type_diagnostic: None,
};
// Allocation of an empty details carrier is not part of an error's payload.
// In particular, assigning an absent dynamic hint must preserve equality.
impl PartialEq for RuntimeError {
    fn eq(&self, other: &Self) -> bool {
        self.undefined_value == other.undefined_value
            && self.kind == other.kind
            && self.message == other.message
            && self.span == other.span
            && self.frames == other.frames
            && **self == **other
    }
}
impl Eq for RuntimeError {}

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
    pub(crate) fn set_template_hint(&mut self, hint: impl Into<PklString>) {
        self.hint = Some(hint.into());
        self.hint_from_template = true;
    }

    pub fn new(kind: ErrorKind, message: impl Into<PklString>) -> Self {
        Self {
            details: None,
            undefined_value: false,
            kind,
            message: message.into(),
            span: None,
            frames: Vec::new(),
        }
    }

    /// The runtime callback stage that produced this failure, if any.
    pub fn callback_origin(&self) -> Option<crate::logging::CallbackOrigin> {
        self.callback_origin
    }

    pub(crate) fn callback_failure(
        origin: crate::logging::CallbackOrigin,
        message: impl Into<PklString>,
    ) -> Self {
        let mut error = Self::new(ErrorKind::Host, message);
        error.callback_origin = Some(origin);
        error
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

#[cfg(test)]
mod error_tests {
    use super::{ErrorKind, PklString, RuntimeError};

    #[test]
    fn absent_hint_assignment_preserves_error_payload_and_clones_detach_hints() {
        let plain = RuntimeError::new(ErrorKind::Type, "failed");
        let mut error = plain.clone();
        error.hint = None;
        assert_eq!(error, plain);
        error.set_template_hint(PklString::from_utf16(vec![0xD800, 0, 0xDC00]));
        let detached = error.clone();
        error.hint = Some("replacement".into());
        assert_ne!(error, detached);
        assert!(detached.hint_from_template);
        assert_eq!(
            detached
                .hint
                .as_ref()
                .unwrap()
                .encode_utf16()
                .collect::<Vec<_>>(),
            [0xD800, 0, 0xDC00]
        );
        assert_eq!(detached, detached.clone());
    }
}
