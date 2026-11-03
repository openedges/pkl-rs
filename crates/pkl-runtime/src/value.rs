//! Values shared by native and hosted experiments. Object handles belong to one evaluator.

use std::fmt;
use std::sync::Arc;

pub use crate::pkl_regex::PklRegex;
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
    Regex(PklRegex),
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
            (Self::Regex(a), Self::Regex(b)) => a == b,
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
            Self::Regex(value) => Some(Reference::Scalar(value.reference())),
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
    pub text: crate::source::SourceText,
    pub uri: PklString,
    pub module_name: PklString,
}

/// A logical language frame, independent of Rust call-stack layout.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeFrame {
    /// Anonymous shared member root: render no module/member name, while retaining
    /// the original source, module identity and authored span.
    pub anonymous: bool,
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
#[derive(Clone, Debug, Default)]
pub struct RuntimeErrorDetails {
    /// VM-only insertion identities and their first position. Excluded from
    /// observable error equality; cloned with cached exceptions and never
    /// consumed by diagnostic rendering.
    pub(crate) cast_insertions: Vec<(crate::heap::TypeNodeId, Option<usize>, bool)>,
    pub(crate) callback_origin: Option<crate::logging::CallbackOrigin>,
    /// Policy belongs to this throw, not to a cached declaration or source.
    pub(crate) frame_transformer: crate::logging::FrameTransformer,
    /// Completed Host diagnostic rendering, retaining the original UTF-16 units.
    pub(crate) rendered_diagnostic: Option<PklString>,
    /// An uncaught Java URI.create failure whose JVM rendering is not implemented.
    pub uri_create_cause: Option<UriCreateCause>,
    /// Exact Java Throwable class/message when established at the throw site.
    /// Internal constructor provenance; excluded from ordinary error equality.
    pub(crate) java_exception_message: Option<(&'static str, PklString)>,
    /// One-shot continuation marker consumed before a YAML error escapes.
    pub(crate) yaml_constructor_error_ready: bool,
    /// Optional rendered hint; retained with rare provenance, outside the common error.
    pub hint: Option<PklString>,
    /// External ErrorMessages template paragraphs have priority over dynamic hints.
    pub(crate) hint_from_template: bool,
    /// The lexical module searched for a type can differ from its diagnostic
    /// source (for example an external expression uses the synthetic repl URI).
    pub(crate) type_lookup_module: Option<PklString>,
    /// A typed Host SPI mapping produced an ordinary guest exception.
    pub(crate) host_language_exception: bool,
    /// Undefined() has no receiver; renderer paths belong only to undefined properties.
    pub(crate) undefined_receiverless: bool,
    /// The throwing helper has neither a guest node nor a source section.
    /// Its innermost call-target frame is unavailable; caller frames remain.
    pub(crate) unavailable_origin: bool,
    pub frontend: Option<Arc<crate::modules::ModuleDiagnostic>>,
    pub(crate) cached_module_frames: bool,
    pub(crate) module_failure_id: Option<u64>,
    pub spread_hint: Option<Box<Span>>,
    pub type_diagnostic: Option<Box<crate::diagnostic::TypeDiagnostic>>,
}

/// Lossless provenance for URI.create's IllegalArgumentException and its
/// URISyntaxException cause; the index is in Java UTF-16 code units.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UriCreateCause {
    pub input: PklString,
    pub reason: &'static str,
    pub index_utf16: Option<usize>,
}
// Executable identities are evaluator-local bookkeeping, not error payload.
// Complete diagnostic frames and all existing provenance still compare exactly.
impl PartialEq for RuntimeErrorDetails {
    fn eq(&self, other: &Self) -> bool {
        self.callback_origin == other.callback_origin
            && self.frame_transformer == other.frame_transformer
            && self.rendered_diagnostic == other.rendered_diagnostic
            && self.uri_create_cause == other.uri_create_cause
            && self.hint == other.hint
            && self.hint_from_template == other.hint_from_template
            && self.type_lookup_module == other.type_lookup_module
            && self.host_language_exception == other.host_language_exception
            && self.undefined_receiverless == other.undefined_receiverless
            && self.unavailable_origin == other.unavailable_origin
            && self.frontend == other.frontend
            && self.cached_module_frames == other.cached_module_frames
            && self.module_failure_id == other.module_failure_id
            && self.spread_hint == other.spread_hint
            && self.type_diagnostic == other.type_diagnostic
    }
}
impl Eq for RuntimeErrorDetails {}

static EMPTY_ERROR_DETAILS: RuntimeErrorDetails = RuntimeErrorDetails {
    cast_insertions: Vec::new(),
    callback_origin: None,
    frame_transformer: crate::logging::FrameTransformer::Default,
    rendered_diagnostic: None,
    uri_create_cause: None,
    java_exception_message: None,
    yaml_constructor_error_ready: false,
    hint: None,
    hint_from_template: false,
    type_lookup_module: None,
    host_language_exception: false,
    undefined_receiverless: false,
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
    pub(crate) fn diagnostic_roots(&self, roots: &mut Vec<Value>) {
        roots.extend(
            self.cast_insertions
                .iter()
                .map(|(target, _, _)| target.0.root()),
        );
    }

    /// Insert a leading/intermediate frame while retaining keyed insertion slots.
    pub(crate) fn insert_frame(&mut self, index: usize, frame: RuntimeFrame) {
        for (_, position, _) in &mut self.cast_insertions {
            if let Some(position) = position
                && *position >= index
            {
                *position += 1;
            }
        }
        self.frames.insert(index, frame);
    }

    /// Reserve the innermost cast-target position even when its declaration
    /// has no displayable section. A later outer insertion still belongs here.
    pub(crate) fn reserve_cast_target(&mut self, target: crate::heap::TypeNodeId) {
        if !self
            .cast_insertions
            .iter()
            .any(|(old, _, _)| *old == target)
        {
            // A frozen module exception cannot acquire a new captured target.
            // Keep its put-only identity, but do not display a new stack frame.
            let position = (!self.cached_module_frames).then_some(self.frames.len());
            self.cast_insertions.push((target, position, false));
        }
    }

    /// Java replaces a target's frame and displays it after its first captured
    /// occurrence. Distinct targets at an unavailable source retain stack order.
    pub(crate) fn insert_cast_frame(
        &mut self,
        target: crate::heap::TypeNodeId,
        frame: RuntimeFrame,
    ) -> Result<(), RuntimeError> {
        self.reserve_cast_target(target);
        let entry = self
            .cast_insertions
            .iter()
            .position(|(old, _, _)| *old == target)
            .expect("reserved cast target");
        let (_, position, occupied) = self.cast_insertions[entry];
        let Some(position) = position else {
            return Ok(());
        };
        if occupied {
            let slot = self.frames.get_mut(position).ok_or_else(|| {
                RuntimeError::new(
                    ErrorKind::InvalidBytecode,
                    "invalid cast insertion position",
                )
            })?;
            *slot = frame;
        } else {
            if position > self.frames.len() {
                return Err(RuntimeError::new(
                    ErrorKind::InvalidBytecode,
                    "invalid cast insertion anchor",
                ));
            }
            for (other, (_, slot, _)) in self.cast_insertions.iter_mut().enumerate() {
                if let Some(slot) = slot
                    && (*slot > position || (*slot == position && other > entry))
                {
                    *slot += 1;
                }
            }
            self.frames.insert(position, frame);
            self.cast_insertions[entry].2 = true;
        }
        Ok(())
    }

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

    pub(crate) fn undefined_without_receiver(mut self) -> Self {
        self.undefined_value = true;
        self.undefined_receiverless = true;
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
