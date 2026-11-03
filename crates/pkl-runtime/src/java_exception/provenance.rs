//! Foreign causes established by shared operations, before guest unwinding.
//!
//! These records contain no JVM stack trace. Declared VM boundaries are only
//! candidates for PklRootNode/MemberNode projection, not generated Java frames.
//! Until that projection and the embedding API profile exist, these failures
//! remain Unsupported. This intentionally leaves a catch-semantics gap: Java
//! wraps them in VmBugException, a VmException caught by TestNodes.catch and
//! catchOrNull. Unsupported must not be mistaken for Java uncatchability.

use std::sync::Arc;

use super::{JavaStackTraceElement, JavaThrowable};
use crate::source::Span;
use crate::value::{PklString, RuntimeFrame, RuntimeSource};

/// Semantic operation and original inputs; no fixture or Java caller names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ForeignOperation {
    IntegerRemainderByZero {
        dividend: i64,
    },
    StringRepeatNegativeCount {
        length_utf16: i64,
        requested_count: i64,
        narrowed_count: i32,
    },
    HttpRequestWithoutHost {
        uri: PklString,
    },
}

/// One original VM activation. All source data outlives executable retirement.
/// An absent metadata/source/section is preserved, never replaced with a frame
/// from a later rethrow. No heap handles or live code-arena references are held.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForeignActivation {
    pub instruction_index: Option<usize>,
    pub member: Option<PklString>,
    pub source: Option<Arc<RuntimeSource>>,
    pub instruction_span: Option<Span>,
    pub declaration_span: Option<Span>,
    pub method_signature: Option<(PklString, usize)>,
    /// Let roots active at the throwing instruction, in metadata nesting order.
    pub active_let_frames: Vec<RuntimeFrame>,
    pub lambda: bool,
    pub inline_constraint: bool,
    pub anonymous_root: bool,
}

/// Captured once before any catch selection, root relocation or frame unwind.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForeignOrigin {
    /// Outermost first, including activations without diagnostic metadata.
    pub activations: Vec<ForeignActivation>,
    /// Supplied evaluation source, separate from each activation's declaration.
    /// This does not identify the embedding Java API or its caller stack.
    pub primary_source: Option<Arc<RuntimeSource>>,
    pub power_assertions: bool,
    pub frame_transformer: crate::logging::FrameTransformer,
    pub original_span: Option<Span>,
    pub existing_guest_frames: Vec<RuntimeFrame>,
}

impl ForeignOrigin {
    /// Actual VM entry, not an inferred Java Evaluator/embedding API frame.
    pub fn vm_entry(&self) -> Option<&ForeignActivation> {
        self.activations.first()
    }

    /// Innermost declared candidates first. Inline checks do not claim a root.
    /// A later source-backed projector must decide the Java root kind.
    pub fn declared_root_candidates(&self) -> impl Iterator<Item = &ForeignActivation> {
        self.activations.iter().rev().filter(|activation| {
            !activation.inline_constraint && activation.declaration_span.is_some()
        })
    }
}

/// A cause of a Java PklRootNode foreign-exception wrapper, not that wrapper's
/// finished diagnostic. It stays attached to cloned and rethrown RuntimeErrors.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForeignExceptionProvenance {
    pub operation: ForeignOperation,
    pub class_name: &'static str,
    pub message: PklString,
    pub origin: Option<ForeignOrigin>,
}

impl ForeignExceptionProvenance {
    pub(crate) fn new(operation: ForeignOperation) -> Self {
        let (class_name, message) = match &operation {
            ForeignOperation::IntegerRemainderByZero { .. } => {
                ("java.lang.ArithmeticException", "/ by zero".into())
            }
            ForeignOperation::StringRepeatNegativeCount { narrowed_count, .. } => (
                "java.lang.IllegalArgumentException",
                format!("count is negative: {narrowed_count}").into(),
            ),
            ForeignOperation::HttpRequestWithoutHost { uri } => (
                "java.lang.IllegalArgumentException",
                PklString::from("unsupported URI ").concat(uri),
            ),
        };
        Self {
            operation,
            class_name,
            message,
            origin: None,
        }
    }

    /// Feed established cause fields to the existing throwable-graph renderer.
    /// The caller must separately supply a complete, qualified JVM trace. This
    /// method does not derive a JVM trace from `origin` or authorize projection.
    pub fn throwable_with_trace(&self, stack_trace: Vec<JavaStackTraceElement>) -> JavaThrowable {
        let mut throwable = JavaThrowable::new(self.class_name, Some(self.message.clone()));
        throwable.stack_trace = stack_trace;
        throwable
    }
}
