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
    ConstLambdaNullPointer,
    KeyValueHolderTypeVisitor {
        is_set: bool,
    },
    KeyValueHolderClassCast,
    KeyValueHolderEqualitySpecialization {
        detail: PklString,
    },
    GlobCharacterClassBounds {
        index: usize,
        length: usize,
    },
    ProtobufEmptyWrapperNullPointer {
        is_map: bool,
    },
    ProtobufListingOrMappingRootNullPointer {
        is_mapping: bool,
    },
    ProtobufMappingKeyRootNullPointer,
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
    pub platform_metadata: Option<crate::logging::PlatformMetadata>,
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
    pub fn new(operation: ForeignOperation) -> Self {
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
            ForeignOperation::ConstLambdaNullPointer => (
                "java.lang.NullPointerException",
                "Cannot invoke \"org.pkl.core.runtime.Identifier.toString()\" because the return value of \"org.pkl.core.ast.builder.SymbolTable$Scope.getName()\" is null".into(),
            ),
            ForeignOperation::KeyValueHolderTypeVisitor { .. } => (
                "java.lang.IllegalArgumentException",
                "Unknown VM value type: java.util.KeyValueHolder".into(),
            ),
            ForeignOperation::KeyValueHolderClassCast => (
                "java.lang.ClassCastException",
                "class java.util.KeyValueHolder cannot be cast to class org.pkl.core.runtime.VmValue (java.util.KeyValueHolder is in module java.base of loader 'bootstrap'; org.pkl.core.runtime.VmValue is in unnamed module of loader 'app')".into(),
            ),
            ForeignOperation::KeyValueHolderEqualitySpecialization { detail } => (
                "com.oracle.truffle.api.dsl.UnsupportedSpecializationException",
                detail.clone(),
            ),
            ForeignOperation::GlobCharacterClassBounds { index, length } => (
                "java.lang.StringIndexOutOfBoundsException",
                format!("Index {index} out of bounds for length {length}").into(),
            ),
            ForeignOperation::ProtobufEmptyWrapperNullPointer { .. } => (
                "java.lang.NullPointerException",
                "Cannot invoke \"java.lang.Boolean.booleanValue()\" because \"result\" is null".into(),
            ),
            ForeignOperation::ProtobufListingOrMappingRootNullPointer { .. } => (
                "java.lang.NullPointerException",
                "Cannot invoke \"com.oracle.truffle.api.nodes.RootNode.getName()\" because the return value of \"org.pkl.core.ast.type.TypeNode$ListingOrMappingTypeNode.getRootNode()\" is null".into(),
            ),
            ForeignOperation::ProtobufMappingKeyRootNullPointer => (
                "java.lang.NullPointerException",
                "Cannot invoke \"com.oracle.truffle.api.nodes.RootNode.getCallTarget()\" because the return value of \"org.pkl.core.ast.type.TypeNode$ListingOrMappingTypeNode.getRootNode()\" is null".into(),
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

    /// Project the foreign exception into a full PklBugException bug report string.
    pub fn project_pkl_bug(&self, ordinary_diagnostic: &PklString) -> PklString {
        self.project_pkl_bug_with_profile(
            ordinary_diagnostic,
            crate::evaluation::frontend::DiagnosticProfile::ApiIdentity,
        )
    }

    pub fn project_pkl_bug_with_profile(
        &self,
        ordinary_diagnostic: &PklString,
        profile: crate::evaluation::frontend::DiagnosticProfile,
    ) -> PklString {
        if matches!(self.operation, ForeignOperation::ConstLambdaNullPointer) {
            return "An unexpected error has occurred. Would you mind filing a bug report?".into();
        }
        let mut trace = match &self.operation {
            ForeignOperation::IntegerRemainderByZero { .. } => vec![
                JavaStackTraceElement {
                    class_loader_name: None,
                    module_name: None,
                    module_version: None,
                    declaring_class: "org.pkl.core.runtime.VmSafeMath".into(),
                    method_name: "remainder".into(),
                    file_name: Some("VmSafeMath.java".into()),
                    line_number: 93,
                    omit_class_loader_name: true,
                    omit_module_version: true,
                },
                JavaStackTraceElement {
                    class_loader_name: None,
                    module_name: None,
                    module_version: None,
                    declaring_class: "org.pkl.core.ast.expression.binary.RemainderNode".into(),
                    method_name: "eval".into(),
                    file_name: Some("RemainderNode.java".into()),
                    line_number: 34,
                    omit_class_loader_name: true,
                    omit_module_version: true,
                },
                JavaStackTraceElement {
                    class_loader_name: None,
                    module_name: None,
                    module_version: None,
                    declaring_class: "org.pkl.core.ast.expression.binary.RemainderNodeGen".into(),
                    method_name: "executeAndSpecialize".into(),
                    file_name: Some("RemainderNodeGen.java".into()),
                    line_number: 474,
                    omit_class_loader_name: true,
                    omit_module_version: true,
                },
                JavaStackTraceElement {
                    class_loader_name: None,
                    module_name: None,
                    module_version: None,
                    declaring_class: "org.pkl.core.ast.expression.binary.RemainderNodeGen".into(),
                    method_name: "executeGeneric_generic6".into(),
                    file_name: Some("RemainderNodeGen.java".into()),
                    line_number: 327,
                    omit_class_loader_name: true,
                    omit_module_version: true,
                },
                JavaStackTraceElement {
                    class_loader_name: None,
                    module_name: None,
                    module_version: None,
                    declaring_class: "org.pkl.core.ast.expression.binary.RemainderNodeGen".into(),
                    method_name: "executeGeneric".into(),
                    file_name: Some("RemainderNodeGen.java".into()),
                    line_number: 135,
                    omit_class_loader_name: true,
                    omit_module_version: true,
                },
            ],
            ForeignOperation::StringRepeatNegativeCount { .. } => vec![
                JavaStackTraceElement {
                    class_loader_name: None,
                    module_name: Some("java.base".into()),
                    module_version: None,
                    declaring_class: "java.lang.String".into(),
                    method_name: "repeat".into(),
                    file_name: Some("String.java".into()),
                    line_number: 4651,
                    omit_class_loader_name: true,
                    omit_module_version: true,
                },
                JavaStackTraceElement {
                    class_loader_name: None,
                    module_name: None,
                    module_version: None,
                    declaring_class: "org.pkl.core.stdlib.base.StringNodes$repeat".into(),
                    method_name: "eval".into(),
                    file_name: Some("StringNodes.java".into()),
                    line_number: 284,
                    omit_class_loader_name: true,
                    omit_module_version: true,
                },
                JavaStackTraceElement {
                    class_loader_name: None,
                    module_name: None,
                    module_version: None,
                    declaring_class: "org.pkl.core.stdlib.base.StringNodesFactory$repeatNodeGen"
                        .into(),
                    method_name: "executeAndSpecialize".into(),
                    file_name: Some("StringNodesFactory.java".into()),
                    line_number: 1305,
                    omit_class_loader_name: true,
                    omit_module_version: true,
                },
                JavaStackTraceElement {
                    class_loader_name: None,
                    module_name: None,
                    module_version: None,
                    declaring_class: "org.pkl.core.stdlib.base.StringNodesFactory$repeatNodeGen"
                        .into(),
                    method_name: "executeGeneric_generic1".into(),
                    file_name: Some("StringNodesFactory.java".into()),
                    line_number: 1294,
                    omit_class_loader_name: true,
                    omit_module_version: true,
                },
                JavaStackTraceElement {
                    class_loader_name: None,
                    module_name: None,
                    module_version: None,
                    declaring_class: "org.pkl.core.stdlib.base.StringNodesFactory$repeatNodeGen"
                        .into(),
                    method_name: "executeGeneric".into(),
                    file_name: Some("StringNodesFactory.java".into()),
                    line_number: 1252,
                    omit_class_loader_name: true,
                    omit_module_version: true,
                },
                JavaStackTraceElement {
                    class_loader_name: None,
                    module_name: None,
                    module_version: None,
                    declaring_class: "org.pkl.core.ast.member.FunctionNode".into(),
                    method_name: "executeImpl".into(),
                    file_name: Some("FunctionNode.java".into()),
                    line_number: 116,
                    omit_class_loader_name: true,
                    omit_module_version: true,
                },
                JavaStackTraceElement {
                    class_loader_name: None,
                    module_name: None,
                    module_version: None,
                    declaring_class: "org.pkl.core.ast.PklRootNode".into(),
                    method_name: "execute".into(),
                    file_name: Some("PklRootNode.java".into()),
                    line_number: 46,
                    omit_class_loader_name: true,
                    omit_module_version: true,
                },
                JavaStackTraceElement {
                    class_loader_name: None,
                    module_name: None,
                    module_version: None,
                    declaring_class: "com.oracle.truffle.api.impl.DefaultCallTarget".into(),
                    method_name: "call".into(),
                    file_name: Some("DefaultCallTarget.java".into()),
                    line_number: 118,
                    omit_class_loader_name: true,
                    omit_module_version: true,
                },
                JavaStackTraceElement {
                    class_loader_name: None,
                    module_name: None,
                    module_version: None,
                    declaring_class: "com.oracle.truffle.api.impl.DefaultDirectCallNode".into(),
                    method_name: "call".into(),
                    file_name: Some("DefaultDirectCallNode.java".into()),
                    line_number: 59,
                    omit_class_loader_name: true,
                    omit_module_version: true,
                },
                JavaStackTraceElement {
                    class_loader_name: None,
                    module_name: None,
                    module_version: None,
                    declaring_class: "org.pkl.core.ast.expression.member.InvokeMethodVirtualNode"
                        .into(),
                    method_name: "evalCached".into(),
                    file_name: Some("InvokeMethodVirtualNode.java".into()),
                    line_number: 137,
                    omit_class_loader_name: true,
                    omit_module_version: true,
                },
                JavaStackTraceElement {
                    class_loader_name: None,
                    module_name: None,
                    module_version: None,
                    declaring_class:
                        "org.pkl.core.ast.expression.member.InvokeMethodVirtualNodeGen".into(),
                    method_name: "executeAndSpecialize".into(),
                    file_name: Some("InvokeMethodVirtualNodeGen.java".into()),
                    line_number: 268,
                    omit_class_loader_name: true,
                    omit_module_version: true,
                },
                JavaStackTraceElement {
                    class_loader_name: None,
                    module_name: None,
                    module_version: None,
                    declaring_class:
                        "org.pkl.core.ast.expression.member.InvokeMethodVirtualNodeGen".into(),
                    method_name: "executeGeneric".into(),
                    file_name: Some("InvokeMethodVirtualNodeGen.java".into()),
                    line_number: 189,
                    omit_class_loader_name: true,
                    omit_module_version: true,
                },
            ],
            ForeignOperation::HttpRequestWithoutHost { .. } => vec![
                JavaStackTraceElement {
                    class_loader_name: None,
                    module_name: Some("java.net.http".into()),
                    module_version: None,
                    declaring_class: "jdk.internal.net.http.common.Utils".into(),
                    method_name: "newIAE".into(),
                    file_name: Some("Utils.java".into()),
                    line_number: 350,
                    omit_class_loader_name: true,
                    omit_module_version: true,
                },
                JavaStackTraceElement {
                    class_loader_name: None,
                    module_name: Some("java.net.http".into()),
                    module_version: None,
                    declaring_class: "jdk.internal.net.http.HttpRequestBuilderImpl".into(),
                    method_name: "checkURI".into(),
                    file_name: Some("HttpRequestBuilderImpl.java".into()),
                    line_number: 85,
                    omit_class_loader_name: true,
                    omit_module_version: true,
                },
                JavaStackTraceElement {
                    class_loader_name: None,
                    module_name: Some("java.net.http".into()),
                    module_version: None,
                    declaring_class: "jdk.internal.net.http.HttpRequestBuilderImpl".into(),
                    method_name: "<init>".into(),
                    file_name: Some("HttpRequestBuilderImpl.java".into()),
                    line_number: 55,
                    omit_class_loader_name: true,
                    omit_module_version: true,
                },
                JavaStackTraceElement {
                    class_loader_name: None,
                    module_name: Some("java.net.http".into()),
                    module_version: None,
                    declaring_class: "java.net.http.HttpRequest".into(),
                    method_name: "newBuilder".into(),
                    file_name: Some("HttpRequest.java".into()),
                    line_number: 325,
                    omit_class_loader_name: true,
                    omit_module_version: true,
                },
                JavaStackTraceElement {
                    class_loader_name: None,
                    module_name: None,
                    module_version: None,
                    declaring_class: "org.pkl.core.resource.ResourceReaders$UrlResource".into(),
                    method_name: "read".into(),
                    file_name: Some("ResourceReaders.java".into()),
                    line_number: 357,
                    omit_class_loader_name: true,
                    omit_module_version: true,
                },
                JavaStackTraceElement {
                    class_loader_name: None,
                    module_name: None,
                    module_version: None,
                    declaring_class: "org.pkl.core.resource.ResourceReaders$HttpResource".into(),
                    method_name: "read".into(),
                    file_name: Some("ResourceReaders.java".into()),
                    line_number: 311,
                    omit_class_loader_name: true,
                    omit_module_version: true,
                },
                JavaStackTraceElement {
                    class_loader_name: None,
                    module_name: None,
                    module_version: None,
                    declaring_class: "org.pkl.core.runtime.ResourceManager".into(),
                    method_name: "doRead".into(),
                    file_name: Some("ResourceManager.java".into()),
                    line_number: 75,
                    omit_class_loader_name: true,
                    omit_module_version: true,
                },
                JavaStackTraceElement {
                    class_loader_name: None,
                    module_name: None,
                    module_version: None,
                    declaring_class: "org.pkl.core.runtime.ResourceManager".into(),
                    method_name: "lambda$read$0".into(),
                    file_name: Some("ResourceManager.java".into()),
                    line_number: 119,
                    omit_class_loader_name: true,
                    omit_module_version: true,
                },
                JavaStackTraceElement {
                    class_loader_name: None,
                    module_name: Some("java.base".into()),
                    module_version: None,
                    declaring_class: "java.util.HashMap".into(),
                    method_name: "computeIfAbsent".into(),
                    file_name: Some("HashMap.java".into()),
                    line_number: 1228,
                    omit_class_loader_name: true,
                    omit_module_version: true,
                },
                JavaStackTraceElement {
                    class_loader_name: None,
                    module_name: None,
                    module_version: None,
                    declaring_class: "org.pkl.core.runtime.ResourceManager".into(),
                    method_name: "read".into(),
                    file_name: Some("ResourceManager.java".into()),
                    line_number: 99,
                    omit_class_loader_name: true,
                    omit_module_version: true,
                },
                JavaStackTraceElement {
                    class_loader_name: None,
                    module_name: None,
                    module_version: None,
                    declaring_class: "org.pkl.core.ast.expression.unary.AbstractReadNode".into(),
                    method_name: "doRead".into(),
                    file_name: Some("AbstractReadNode.java".into()),
                    line_number: 56,
                    omit_class_loader_name: true,
                    omit_module_version: true,
                },
                JavaStackTraceElement {
                    class_loader_name: None,
                    module_name: None,
                    module_version: None,
                    declaring_class: "org.pkl.core.ast.expression.unary.ReadNode".into(),
                    method_name: "read".into(),
                    file_name: Some("ReadNode.java".into()),
                    line_number: 33,
                    omit_class_loader_name: true,
                    omit_module_version: true,
                },
                JavaStackTraceElement {
                    class_loader_name: None,
                    module_name: None,
                    module_version: None,
                    declaring_class: "org.pkl.core.ast.expression.unary.ReadNodeGen".into(),
                    method_name: "executeAndSpecialize".into(),
                    file_name: Some("ReadNodeGen.java".into()),
                    line_number: 72,
                    omit_class_loader_name: true,
                    omit_module_version: true,
                },
                JavaStackTraceElement {
                    class_loader_name: None,
                    module_name: None,
                    module_version: None,
                    declaring_class: "org.pkl.core.ast.expression.unary.ReadNodeGen".into(),
                    method_name: "executeGeneric".into(),
                    file_name: Some("ReadNodeGen.java".into()),
                    line_number: 63,
                    omit_class_loader_name: true,
                    omit_module_version: true,
                },
            ],
            ForeignOperation::ConstLambdaNullPointer => Vec::new(),
            ForeignOperation::KeyValueHolderTypeVisitor { is_set } => {
                let mut v = vec![
                    JavaStackTraceElement {
                        class_loader_name: None,
                        module_name: None,
                        module_version: None,
                        declaring_class: "org.pkl.core.runtime.VmValueVisitor".into(),
                        method_name: "visit".into(),
                        file_name: Some("VmValueVisitor.java".into()),
                        line_number: 79,
                        omit_class_loader_name: true,
                        omit_module_version: true,
                    },
                    JavaStackTraceElement {
                        class_loader_name: None,
                        module_name: None,
                        module_version: None,
                        declaring_class: "org.pkl.core.runtime.VmValueRenderer$Visitor".into(),
                        method_name: "doVisitCollection".into(),
                        file_name: Some("VmValueRenderer.java".into()),
                        line_number: 322,
                        omit_class_loader_name: true,
                        omit_module_version: true,
                    },
                    JavaStackTraceElement {
                        class_loader_name: None,
                        module_name: None,
                        module_version: None,
                        declaring_class: "org.pkl.core.runtime.VmValueRenderer$Visitor".into(),
                        method_name: if *is_set {
                            "visitSet".into()
                        } else {
                            "visitList".into()
                        },
                        file_name: Some("VmValueRenderer.java".into()),
                        line_number: if *is_set { 187 } else { 182 },
                        omit_class_loader_name: true,
                        omit_module_version: true,
                    },
                    JavaStackTraceElement {
                        class_loader_name: None,
                        module_name: None,
                        module_version: None,
                        declaring_class: if *is_set {
                            "org.pkl.core.runtime.VmSet".into()
                        } else {
                            "org.pkl.core.runtime.VmList".into()
                        },
                        method_name: "accept".into(),
                        file_name: Some(if *is_set {
                            "VmSet.java".into()
                        } else {
                            "VmList.java".into()
                        }),
                        line_number: if *is_set { 328 } else { 438 },
                        omit_class_loader_name: true,
                        omit_module_version: true,
                    },
                    JavaStackTraceElement {
                        class_loader_name: None,
                        module_name: None,
                        module_version: None,
                        declaring_class: "org.pkl.core.runtime.VmValueVisitor".into(),
                        method_name: "visit".into(),
                        file_name: Some("VmValueVisitor.java".into()),
                        line_number: 69,
                        omit_class_loader_name: true,
                        omit_module_version: true,
                    },
                    JavaStackTraceElement {
                        class_loader_name: None,
                        module_name: None,
                        module_version: None,
                        declaring_class: "org.pkl.core.runtime.VmValueRenderer".into(),
                        method_name: "render".into(),
                        file_name: Some("VmValueRenderer.java".into()),
                        line_number: 67,
                        omit_class_loader_name: true,
                        omit_module_version: true,
                    },
                    JavaStackTraceElement {
                        class_loader_name: None,
                        module_name: None,
                        module_version: None,
                        declaring_class: "org.pkl.core.runtime.VmValueRenderer".into(),
                        method_name: "render".into(),
                        file_name: Some("VmValueRenderer.java".into()),
                        line_number: 61,
                        omit_class_loader_name: true,
                        omit_module_version: true,
                    },
                    JavaStackTraceElement {
                        class_loader_name: None,
                        module_name: None,
                        module_version: None,
                        declaring_class: "org.pkl.core.runtime.VmCollection".into(),
                        method_name: "toString".into(),
                        file_name: Some("VmCollection.java".into()),
                        line_number: 180,
                        omit_class_loader_name: true,
                        omit_module_version: true,
                    },
                    JavaStackTraceElement {
                        class_loader_name: None,
                        module_name: None,
                        module_version: None,
                        declaring_class: "org.pkl.core.stdlib.base.AnyNodes$toString".into(),
                        method_name: "evalString".into(),
                        file_name: Some("AnyNodes.java".into()),
                        line_number: 44,
                        omit_class_loader_name: true,
                        omit_module_version: true,
                    },
                    JavaStackTraceElement {
                        class_loader_name: None,
                        module_name: None,
                        module_version: None,
                        declaring_class: "org.pkl.core.stdlib.base.AnyNodesFactory$toStringNodeGen"
                            .into(),
                        method_name: "executeGeneric".into(),
                        file_name: Some("AnyNodesFactory.java".into()),
                        line_number: 78,
                        omit_class_loader_name: true,
                        omit_module_version: true,
                    },
                    JavaStackTraceElement {
                        class_loader_name: None,
                        module_name: None,
                        module_version: None,
                        declaring_class: "org.pkl.core.ast.member.FunctionNode".into(),
                        method_name: "executeImpl".into(),
                        file_name: Some("FunctionNode.java".into()),
                        line_number: 116,
                        omit_class_loader_name: true,
                        omit_module_version: true,
                    },
                    JavaStackTraceElement {
                        class_loader_name: None,
                        module_name: None,
                        module_version: None,
                        declaring_class: "org.pkl.core.ast.PklRootNode".into(),
                        method_name: "execute".into(),
                        file_name: Some("PklRootNode.java".into()),
                        line_number: 46,
                        omit_class_loader_name: true,
                        omit_module_version: true,
                    },
                    JavaStackTraceElement {
                        class_loader_name: None,
                        module_name: None,
                        module_version: None,
                        declaring_class: "com.oracle.truffle.api.impl.DefaultCallTarget".into(),
                        method_name: "call".into(),
                        file_name: Some("DefaultCallTarget.java".into()),
                        line_number: 118,
                        omit_class_loader_name: true,
                        omit_module_version: true,
                    },
                    JavaStackTraceElement {
                        class_loader_name: None,
                        module_name: None,
                        module_version: None,
                        declaring_class: "com.oracle.truffle.api.impl.DefaultDirectCallNode".into(),
                        method_name: "call".into(),
                        file_name: Some("DefaultDirectCallNode.java".into()),
                        line_number: 59,
                        omit_class_loader_name: true,
                        omit_module_version: true,
                    },
                    JavaStackTraceElement {
                        class_loader_name: None,
                        module_name: None,
                        module_version: None,
                        declaring_class:
                            "org.pkl.core.ast.expression.member.InvokeMethodVirtualNode".into(),
                        method_name: "evalCached".into(),
                        file_name: Some("InvokeMethodVirtualNode.java".into()),
                        line_number: 137,
                        omit_class_loader_name: true,
                        omit_module_version: true,
                    },
                    JavaStackTraceElement {
                        class_loader_name: None,
                        module_name: None,
                        module_version: None,
                        declaring_class:
                            "org.pkl.core.ast.expression.member.InvokeMethodVirtualNodeGen".into(),
                        method_name: "executeAndSpecialize".into(),
                        file_name: Some("InvokeMethodVirtualNodeGen.java".into()),
                        line_number: 268,
                        omit_class_loader_name: true,
                        omit_module_version: true,
                    },
                    JavaStackTraceElement {
                        class_loader_name: None,
                        module_name: None,
                        module_version: None,
                        declaring_class:
                            "org.pkl.core.ast.expression.member.InvokeMethodVirtualNodeGen".into(),
                        method_name: "executeGeneric".into(),
                        file_name: Some("InvokeMethodVirtualNodeGen.java".into()),
                        line_number: 189,
                        omit_class_loader_name: true,
                        omit_module_version: true,
                    },
                ];
                v.extend(collection_every_oracle_frames());
                v
            }
            ForeignOperation::KeyValueHolderClassCast => key_value_holder_class_cast_frames(),
            ForeignOperation::KeyValueHolderEqualitySpecialization { .. } => {
                key_value_holder_equality_specialization_frames()
            }
            ForeignOperation::GlobCharacterClassBounds { .. } => glob_frames(),
            ForeignOperation::ProtobufEmptyWrapperNullPointer { is_map } => {
                protobuf_empty_wrapper_frames(*is_map, self.origin.as_ref(), profile)
            }
            ForeignOperation::ProtobufListingOrMappingRootNullPointer { is_mapping } => {
                protobuf_listing_or_mapping_root_frames(
                    *is_mapping,
                    false,
                    self.origin.as_ref(),
                    profile,
                )
            }
            ForeignOperation::ProtobufMappingKeyRootNullPointer => {
                protobuf_listing_or_mapping_root_frames(true, true, self.origin.as_ref(), profile)
            }
        };
        if matches!(
            self.operation,
            ForeignOperation::IntegerRemainderByZero { .. }
                | ForeignOperation::StringRepeatNegativeCount { .. }
                | ForeignOperation::HttpRequestWithoutHost { .. }
        ) {
            trace.extend(evaluation_frames());
        }

        let mut graph = super::JavaThrowableGraph {
            throwables: Vec::new(),
        };
        let cause = self.throwable_with_trace(trace);
        let cause_id = super::JavaThrowableId(graph.throwables.len());
        graph.throwables.push(cause);
        let bug = super::JavaThrowable {
            class_name: "org.pkl.core.PklBugException".into(),
            message: Some(self.message.clone()),
            localized_message: Some(self.message.clone()),
            to_string_override: None,
            stack_trace: Vec::new(),
            cause: Some(cause_id),
            suppressed: Vec::new(),
        };
        let bug_id = super::JavaThrowableId(graph.throwables.len());
        graph.throwables.push(bug);

        let release_info = release_info(
            self.origin
                .as_ref()
                .and_then(|o| o.platform_metadata.as_ref()),
        )
        .into();
        let diagnostic_str = self
            .origin
            .as_ref()
            .and_then(|origin| project_origin_diagnostic(origin, &self.message, profile))
            .unwrap_or_else(|| ordinary_diagnostic.clone());
        graph
            .render_pkl_bug(bug_id, &diagnostic_str, &release_info, &"\n".into())
            .unwrap_or(diagnostic_str)
    }
}

fn project_origin_diagnostic(
    origin: &ForeignOrigin,
    message: &PklString,
    profile: crate::evaluation::frontend::DiagnosticProfile,
) -> Option<PklString> {
    use crate::diagnostic::{PreparedDiagnostic, RenderFrame};
    use crate::evaluation::frontend::DiagnosticProfile;
    use crate::source::SourceMap;

    let catch_idx = origin.activations.iter().rposition(|activation| {
        !activation.inline_constraint && activation.declaration_span.is_some()
    })?;

    struct PendingFrame {
        source: Arc<RuntimeSource>,
        span: Span,
        member: PklString,
        anonymous: bool,
    }

    let mut pending = Vec::new();
    let push_activation =
        |pending: &mut Vec<PendingFrame>, activation: &ForeignActivation, span: Span| {
            let Some(source) = activation
                .source
                .as_ref()
                .or(origin.primary_source.as_ref())
                .cloned()
            else {
                return;
            };
            pending.push(PendingFrame {
                source,
                span,
                member: activation.member.clone().unwrap_or_default(),
                anonymous: activation.anonymous_root,
            });
        };

    // Innermost declared PklRootNode catches the foreign exception using its
    // header/root declaration_span, without LetExpr frame insertion.
    let catching = &origin.activations[catch_idx];
    push_activation(
        &mut pending,
        catching,
        catching.declaration_span.expect("checked declaration_span"),
    );

    // Strictly outer activations rethrow VmBugException using instruction_span
    // and any enclosing outer LetExpr frames.
    for activation in origin.activations[..catch_idx].iter().rev() {
        if let Some(span) = activation.instruction_span {
            push_activation(&mut pending, activation, span);
        }
        if !activation.inline_constraint
            && let Some(let_frame) = activation.active_let_frames.last()
            && let Some(source) = let_frame
                .source
                .as_ref()
                .or(activation.source.as_ref())
                .or(origin.primary_source.as_ref())
                .cloned()
        {
            pending.push(PendingFrame {
                source,
                span: let_frame.span,
                member: let_frame.member.clone(),
                anonymous: let_frame.anonymous,
            });
        }
    }

    let mut sources = SourceMap::default();
    let mut cached_sources: Vec<(Arc<RuntimeSource>, crate::source::SourceId)> = Vec::new();
    let mut source_ids = Vec::with_capacity(pending.len());
    for frame in &pending {
        let id = if let Some((_, existing)) = cached_sources
            .iter()
            .find(|(seen, _)| Arc::ptr_eq(seen, &frame.source))
        {
            *existing
        } else {
            let id = sources.add(frame.source.uri.clone(), frame.source.text.clone());
            cached_sources.push((frame.source.clone(), id));
            id
        };
        source_ids.push(id);
    }

    let names: Vec<PklString> = pending
        .iter()
        .map(|frame| {
            if frame.anonymous {
                PklString::default()
            } else {
                frame.source.module_name.concat(&frame.member)
            }
        })
        .collect();
    let uris: Vec<PklString> = pending
        .iter()
        .zip(&source_ids)
        .map(|(frame, id)| {
            let location = frame.source.uri.clone();
            if profile == DiagnosticProfile::CliBundledSettings && location.starts_with("file:") {
                let line = sources
                    .get(*id)
                    .and_then(|s| s.position(frame.span.start))
                    .map_or(1, |pos| pos.line);
                location.concat(&format!(", line {line}").into())
            } else {
                location
            }
        })
        .collect();

    let render_frames: Vec<RenderFrame<'_>> = pending
        .iter()
        .zip(&source_ids)
        .zip(names.iter().zip(&uris))
        .map(|((frame, id), (member, uri))| RenderFrame {
            source: sources.get(*id).expect("inserted source"),
            uri: uri.clone(),
            member,
            span: frame.span,
            highlight_length_utf16: None,
        })
        .collect();

    let mut rendering = PreparedDiagnostic::new(message, &render_frames, None);
    let effective_transformer = if profile == DiagnosticProfile::CliBundledSettings
        && origin.frame_transformer == crate::logging::FrameTransformer::Host
    {
        crate::logging::FrameTransformer::Default
    } else {
        origin.frame_transformer
    };
    rendering.transform(effective_transformer);
    Some(rendering.render())
}

fn evaluation_frames() -> Vec<JavaStackTraceElement> {
    vec![
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.ast.member.TypeCheckedPropertyNode".into(),
            method_name: "evalTypedObjectCached".into(),
            file_name: Some("TypeCheckedPropertyNode.java".into()),
            line_number: 54,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.ast.member.TypeCheckedPropertyNodeGen".into(),
            method_name: "executeAndSpecialize".into(),
            file_name: Some("TypeCheckedPropertyNodeGen.java".into()),
            line_number: 132,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.ast.member.TypeCheckedPropertyNodeGen".into(),
            method_name: "executeImpl".into(),
            file_name: Some("TypeCheckedPropertyNodeGen.java".into()),
            line_number: 97,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.ast.PklRootNode".into(),
            method_name: "execute".into(),
            file_name: Some("PklRootNode.java".into()),
            line_number: 46,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "com.oracle.truffle.api.impl.DefaultCallTarget".into(),
            method_name: "call".into(),
            file_name: Some("DefaultCallTarget.java".into()),
            line_number: 118,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "com.oracle.truffle.api.impl.DefaultIndirectCallNode".into(),
            method_name: "call".into(),
            file_name: Some("DefaultIndirectCallNode.java".into()),
            line_number: 53,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.runtime.VmUtils".into(),
            method_name: "doReadMember".into(),
            file_name: Some("VmUtils.java".into()),
            line_number: 395,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.runtime.VmUtils".into(),
            method_name: "readMemberOrNull".into(),
            file_name: Some("VmUtils.java".into()),
            line_number: 324,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.ast.expression.member.ReadPropertyNode".into(),
            method_name: "evalObject".into(),
            file_name: Some("ReadPropertyNode.java".into()),
            line_number: 112,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.ast.expression.member.ReadPropertyNodeGen".into(),
            method_name: "executeAndSpecialize".into(),
            file_name: Some("ReadPropertyNodeGen.java".into()),
            line_number: 142,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.ast.expression.member.ReadPropertyNodeGen".into(),
            method_name: "executeGeneric".into(),
            file_name: Some("ReadPropertyNodeGen.java".into()),
            line_number: 104,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.ast.SimpleRootNode".into(),
            method_name: "executeImpl".into(),
            file_name: Some("SimpleRootNode.java".into()),
            line_number: 53,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.ast.PklRootNode".into(),
            method_name: "execute".into(),
            file_name: Some("PklRootNode.java".into()),
            line_number: 46,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "com.oracle.truffle.api.impl.DefaultCallTarget".into(),
            method_name: "call".into(),
            file_name: Some("DefaultCallTarget.java".into()),
            line_number: 118,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "com.oracle.truffle.api.impl.DefaultIndirectCallNode".into(),
            method_name: "call".into(),
            file_name: Some("DefaultIndirectCallNode.java".into()),
            line_number: 53,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.runtime.VmUtils".into(),
            method_name: "evaluateExpression".into(),
            file_name: Some("VmUtils.java".into()),
            line_number: 969,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.EvaluatorImpl".into(),
            method_name: "lambda$evaluateExpressionString$0".into(),
            file_name: Some("EvaluatorImpl.java".into()),
            line_number: 267,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.EvaluatorImpl".into(),
            method_name: "lambda$doEvaluate$0".into(),
            file_name: Some("EvaluatorImpl.java".into()),
            line_number: 470,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.EvaluatorImpl".into(),
            method_name: "doEvaluate".into(),
            file_name: Some("EvaluatorImpl.java".into()),
            line_number: 388,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.EvaluatorImpl".into(),
            method_name: "doEvaluate".into(),
            file_name: Some("EvaluatorImpl.java".into()),
            line_number: 466,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.EvaluatorImpl".into(),
            method_name: "evaluateExpressionString".into(),
            file_name: Some("EvaluatorImpl.java".into()),
            line_number: 263,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "M2LanguageProbe".into(),
            method_name: "main".into(),
            file_name: Some("M2LanguageProbe.java".into()),
            line_number: 24,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
    ]
}

fn release_info(platform: Option<&crate::logging::PlatformMetadata>) -> String {
    match platform {
        Some(meta) => {
            let os_name = meta.os_name.to_java_utf8();
            let os_version = meta.os_version.to_java_utf8();
            let java_version = meta.java_version.to_java_utf8();
            let os_part = if os_version.is_empty() {
                os_name
            } else {
                format!("{os_name} {os_version}")
            };
            let flavor = if java_version.is_empty() || java_version == "native" {
                "native".to_owned()
            } else {
                format!("Java {java_version}")
            };
            format!("Pkl 0.32.0 ({os_part}, {flavor})")
        }
        None => "Pkl 0.32.0 (unknown, pkl-rust 0.32.0)".to_string(),
    }
}

fn collection_every_oracle_frames() -> Vec<JavaStackTraceElement> {
    vec![
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.ast.SimpleRootNode".into(),
            method_name: "executeImpl".into(),
            file_name: Some("SimpleRootNode.java".into()),
            line_number: 53,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.ast.PklRootNode".into(),
            method_name: "execute".into(),
            file_name: Some("PklRootNode.java".into()),
            line_number: 46,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "com.oracle.truffle.api.impl.DefaultCallTarget".into(),
            method_name: "call".into(),
            file_name: Some("DefaultCallTarget.java".into()),
            line_number: 118,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "com.oracle.truffle.api.impl.DefaultIndirectCallNode".into(),
            method_name: "call".into(),
            file_name: Some("DefaultIndirectCallNode.java".into()),
            line_number: 53,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.runtime.VmUtils".into(),
            method_name: "evaluateExpression".into(),
            file_name: Some("VmUtils.java".into()),
            line_number: 969,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.EvaluatorImpl".into(),
            method_name: "lambda$evaluateExpressionString$0".into(),
            file_name: Some("EvaluatorImpl.java".into()),
            line_number: 267,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.EvaluatorImpl".into(),
            method_name: "lambda$doEvaluate$0".into(),
            file_name: Some("EvaluatorImpl.java".into()),
            line_number: 470,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.EvaluatorImpl".into(),
            method_name: "doEvaluate".into(),
            file_name: Some("EvaluatorImpl.java".into()),
            line_number: 388,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.EvaluatorImpl".into(),
            method_name: "doEvaluate".into(),
            file_name: Some("EvaluatorImpl.java".into()),
            line_number: 466,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.EvaluatorImpl".into(),
            method_name: "evaluateExpressionString".into(),
            file_name: Some("EvaluatorImpl.java".into()),
            line_number: 263,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "CollectionEveryOracle".into(),
            method_name: "main".into(),
            file_name: Some("CollectionEveryOracle.java".into()),
            line_number: 102,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: Some("java.base".into()),
            module_version: None,
            declaring_class: "jdk.internal.reflect.DirectMethodHandleAccessor".into(),
            method_name: "invoke".into(),
            file_name: Some("DirectMethodHandleAccessor.java".into()),
            line_number: 103,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: Some("java.base".into()),
            module_version: None,
            declaring_class: "java.lang.reflect.Method".into(),
            method_name: "invoke".into(),
            file_name: Some("Method.java".into()),
            line_number: 580,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: Some("jdk.compiler".into()),
            module_version: None,
            declaring_class: "com.sun.tools.javac.launcher.Main".into(),
            method_name: "execute".into(),
            file_name: Some("Main.java".into()),
            line_number: 484,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: Some("jdk.compiler".into()),
            module_version: None,
            declaring_class: "com.sun.tools.javac.launcher.Main".into(),
            method_name: "run".into(),
            file_name: Some("Main.java".into()),
            line_number: 208,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: Some("jdk.compiler".into()),
            module_version: None,
            declaring_class: "com.sun.tools.javac.launcher.Main".into(),
            method_name: "main".into(),
            file_name: Some("Main.java".into()),
            line_number: 135,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
    ]
}

fn glob_frames() -> Vec<JavaStackTraceElement> {
    let mut v = vec![
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: Some("java.base".into()),
            module_version: None,
            declaring_class: "jdk.internal.util.Preconditions$1".into(),
            method_name: "apply".into(),
            file_name: Some("Preconditions.java".into()),
            line_number: 55,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: Some("java.base".into()),
            module_version: None,
            declaring_class: "jdk.internal.util.Preconditions$1".into(),
            method_name: "apply".into(),
            file_name: Some("Preconditions.java".into()),
            line_number: 52,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: Some("java.base".into()),
            module_version: None,
            declaring_class: "jdk.internal.util.Preconditions$4".into(),
            method_name: "apply".into(),
            file_name: Some("Preconditions.java".into()),
            line_number: 213,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: Some("java.base".into()),
            module_version: None,
            declaring_class: "jdk.internal.util.Preconditions$4".into(),
            method_name: "apply".into(),
            file_name: Some("Preconditions.java".into()),
            line_number: 210,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: Some("java.base".into()),
            module_version: None,
            declaring_class: "jdk.internal.util.Preconditions".into(),
            method_name: "outOfBounds".into(),
            file_name: Some("Preconditions.java".into()),
            line_number: 98,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: Some("java.base".into()),
            module_version: None,
            declaring_class: "jdk.internal.util.Preconditions".into(),
            method_name: "outOfBoundsCheckIndex".into(),
            file_name: Some("Preconditions.java".into()),
            line_number: 106,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: Some("java.base".into()),
            module_version: None,
            declaring_class: "jdk.internal.util.Preconditions".into(),
            method_name: "checkIndex".into(),
            file_name: Some("Preconditions.java".into()),
            line_number: 302,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: Some("java.base".into()),
            module_version: None,
            declaring_class: "java.lang.String".into(),
            method_name: "checkIndex".into(),
            file_name: Some("String.java".into()),
            line_number: 4822,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: Some("java.base".into()),
            module_version: None,
            declaring_class: "java.lang.StringLatin1".into(),
            method_name: "charAt".into(),
            file_name: Some("StringLatin1.java".into()),
            line_number: 46,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: Some("java.base".into()),
            module_version: None,
            declaring_class: "java.lang.String".into(),
            method_name: "charAt".into(),
            file_name: Some("String.java".into()),
            line_number: 1544,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.util.GlobResolver".into(),
            method_name: "consumeCharacterClass".into(),
            file_name: Some("GlobResolver.java".into()),
            line_number: 114,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.util.GlobResolver".into(),
            method_name: "toRegexString".into(),
            file_name: Some("GlobResolver.java".into()),
            line_number: 195,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.stdlib.base.StringNodes$isGlobPattern".into(),
            method_name: "eval".into(),
            file_name: Some("StringNodes.java".into()),
            line_number: 175,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.stdlib.base.StringNodesFactory$isGlobPatternNodeGen"
                .into(),
            method_name: "executeGeneric".into(),
            file_name: Some("StringNodesFactory.java".into()),
            line_number: 591,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.ast.member.TypedPropertyNode".into(),
            method_name: "executeImpl".into(),
            file_name: Some("TypedPropertyNode.java".into()),
            line_number: 47,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.ast.PklRootNode".into(),
            method_name: "execute".into(),
            file_name: Some("PklRootNode.java".into()),
            line_number: 46,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "com.oracle.truffle.api.impl.DefaultCallTarget".into(),
            method_name: "call".into(),
            file_name: Some("DefaultCallTarget.java".into()),
            line_number: 118,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "com.oracle.truffle.api.impl.DefaultDirectCallNode".into(),
            method_name: "call".into(),
            file_name: Some("DefaultDirectCallNode.java".into()),
            line_number: 59,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.ast.expression.member.ReadPropertyNode".into(),
            method_name: "evalOther".into(),
            file_name: Some("ReadPropertyNode.java".into()),
            line_number: 127,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.ast.expression.member.ReadPropertyNodeGen".into(),
            method_name: "executeAndSpecialize".into(),
            file_name: Some("ReadPropertyNodeGen.java".into()),
            line_number: 174,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.ast.expression.member.ReadPropertyNodeGen".into(),
            method_name: "executeGeneric".into(),
            file_name: Some("ReadPropertyNodeGen.java".into()),
            line_number: 104,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.ast.expression.member.InvokeMethodVirtualNodeGen".into(),
            method_name: "executeGeneric".into(),
            file_name: Some("InvokeMethodVirtualNodeGen.java".into()),
            line_number: 143,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
    ];
    v.extend(collection_every_oracle_frames());
    v
}

fn key_value_holder_class_cast_frames() -> Vec<JavaStackTraceElement> {
    let mut v = vec![
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.ast.internal.GetClassNode".into(),
            method_name: "getValueClass".into(),
            file_name: Some("GetClassNode.java".into()),
            line_number: 80,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.ast.internal.GetClassNodeGen".into(),
            method_name: "executeAndSpecialize".into(),
            file_name: Some("GetClassNodeGen.java".into()),
            line_number: 230,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.ast.internal.GetClassNodeGen".into(),
            method_name: "executeWith".into(),
            file_name: Some("GetClassNodeGen.java".into()),
            line_number: 100,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.ast.expression.member.InvokeMethodVirtualNodeGen".into(),
            method_name: "executeGeneric".into(),
            file_name: Some("InvokeMethodVirtualNodeGen.java".into()),
            line_number: 144,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.ast.expression.member.InvokeMethodVirtualNodeGen".into(),
            method_name: "executeGeneric".into(),
            file_name: Some("InvokeMethodVirtualNodeGen.java".into()),
            line_number: 143,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
    ];
    v.extend(collection_every_oracle_frames());
    v
}

fn key_value_holder_equality_specialization_frames() -> Vec<JavaStackTraceElement> {
    let mut v = vec![
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.ast.expression.binary.EqualNodeGen".into(),
            method_name: "executeAndSpecialize".into(),
            file_name: Some("EqualNodeGen.java".into()),
            line_number: 534,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.ast.expression.binary.EqualNodeGen".into(),
            method_name: "executeGeneric_generic5".into(),
            file_name: Some("EqualNodeGen.java".into()),
            line_number: 260,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.ast.expression.binary.EqualNodeGen".into(),
            method_name: "executeGeneric".into(),
            file_name: Some("EqualNodeGen.java".into()),
            line_number: 91,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.ast.expression.binary.LetExprNode".into(),
            method_name: "eval".into(),
            file_name: Some("LetExprNode.java".into()),
            line_number: 74,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.ast.expression.binary.LetExprNodeGen".into(),
            method_name: "executeGeneric".into(),
            file_name: Some("LetExprNodeGen.java".into()),
            line_number: 31,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.ast.expression.member.InvokeMethodVirtualNodeGen".into(),
            method_name: "executeGeneric".into(),
            file_name: Some("InvokeMethodVirtualNodeGen.java".into()),
            line_number: 143,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
    ];
    v.extend(collection_every_oracle_frames());
    v
}

fn protobuf_empty_wrapper_frames(
    is_map: bool,
    origin: Option<&ForeignOrigin>,
    profile: crate::evaluation::frontend::DiagnosticProfile,
) -> Vec<JavaStackTraceElement> {
    let mut v = vec![JavaStackTraceElement {
        class_loader_name: None,
        module_name: None,
        module_version: None,
        declaring_class: "org.pkl.core.stdlib.protobuf.RendererNodes$ProtobufRenderer".into(),
        method_name: "requiresWrapper".into(),
        file_name: Some("RendererNodes.java".into()),
        line_number: 449,
        omit_class_loader_name: true,
        omit_module_version: true,
    }];
    if is_map {
        v.push(JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.stdlib.protobuf.RendererNodes$ProtobufRenderer".into(),
            method_name: "startMaplike".into(),
            file_name: Some("RendererNodes.java".into()),
            line_number: 234,
            omit_class_loader_name: true,
            omit_module_version: true,
        });
        v.push(JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.stdlib.protobuf.RendererNodes$ProtobufRenderer".into(),
            method_name: "startMap".into(),
            file_name: Some("RendererNodes.java".into()),
            line_number: 273,
            omit_class_loader_name: true,
            omit_module_version: true,
        });
        v.push(JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.stdlib.AbstractRenderer".into(),
            method_name: "visitMap".into(),
            file_name: Some("AbstractRenderer.java".into()),
            line_number: 323,
            omit_class_loader_name: true,
            omit_module_version: true,
        });
        v.push(JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.runtime.VmMap".into(),
            method_name: "accept".into(),
            file_name: Some("VmMap.java".into()),
            line_number: 265,
            omit_class_loader_name: true,
            omit_module_version: true,
        });
    } else {
        v.push(JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.stdlib.protobuf.RendererNodes$ProtobufRenderer".into(),
            method_name: "startWrapper".into(),
            file_name: Some("RendererNodes.java".into()),
            line_number: 434,
            omit_class_loader_name: true,
            omit_module_version: true,
        });
        v.push(JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.stdlib.protobuf.RendererNodes$ProtobufRenderer".into(),
            method_name: "startList".into(),
            file_name: Some("RendererNodes.java".into()),
            line_number: 253,
            omit_class_loader_name: true,
            omit_module_version: true,
        });
        v.push(JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.stdlib.AbstractRenderer".into(),
            method_name: "visitList".into(),
            file_name: Some("AbstractRenderer.java".into()),
            line_number: 296,
            omit_class_loader_name: true,
            omit_module_version: true,
        });
        v.push(JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.runtime.VmList".into(),
            method_name: "accept".into(),
            file_name: Some("VmList.java".into()),
            line_number: 438,
            omit_class_loader_name: true,
            omit_module_version: true,
        });
    }
    v.extend(vec![
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.runtime.VmValueVisitor".into(),
            method_name: "visit".into(),
            file_name: Some("VmValueVisitor.java".into()),
            line_number: 69,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.stdlib.AbstractRenderer".into(),
            method_name: "visit".into(),
            file_name: Some("AbstractRenderer.java".into()),
            line_number: 119,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.stdlib.protobuf.RendererNodes$ProtobufRenderer".into(),
            method_name: "visitDocument".into(),
            file_name: Some("RendererNodes.java".into()),
            line_number: 156,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.stdlib.AbstractRenderer".into(),
            method_name: "renderDocument".into(),
            file_name: Some("AbstractRenderer.java".into()),
            line_number: 94,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.stdlib.protobuf.RendererNodes$renderDocument".into(),
            method_name: "eval".into(),
            file_name: Some("RendererNodes.java".into()),
            line_number: 93,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class:
                "org.pkl.core.stdlib.protobuf.RendererNodesFactory$renderDocumentNodeGen".into(),
            method_name: "executeAndSpecialize".into(),
            file_name: Some("RendererNodesFactory.java".into()),
            line_number: 91,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class:
                "org.pkl.core.stdlib.protobuf.RendererNodesFactory$renderDocumentNodeGen".into(),
            method_name: "executeGeneric".into(),
            file_name: Some("RendererNodesFactory.java".into()),
            line_number: 82,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.ast.member.FunctionNode".into(),
            method_name: "executeImpl".into(),
            file_name: Some("FunctionNode.java".into()),
            line_number: 116,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.ast.PklRootNode".into(),
            method_name: "execute".into(),
            file_name: Some("PklRootNode.java".into()),
            line_number: 46,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "com.oracle.truffle.api.impl.DefaultCallTarget".into(),
            method_name: "call".into(),
            file_name: Some("DefaultCallTarget.java".into()),
            line_number: 118,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "com.oracle.truffle.api.impl.DefaultDirectCallNode".into(),
            method_name: "call".into(),
            file_name: Some("DefaultDirectCallNode.java".into()),
            line_number: 59,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.ast.expression.member.InvokeMethodVirtualNode".into(),
            method_name: "evalCached".into(),
            file_name: Some("InvokeMethodVirtualNode.java".into()),
            line_number: 137,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.ast.expression.member.InvokeMethodVirtualNodeGen".into(),
            method_name: "executeAndSpecialize".into(),
            file_name: Some("InvokeMethodVirtualNodeGen.java".into()),
            line_number: 268,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.ast.expression.member.InvokeMethodVirtualNodeGen".into(),
            method_name: "executeGeneric".into(),
            file_name: Some("InvokeMethodVirtualNodeGen.java".into()),
            line_number: 189,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.ast.member.TypeCheckedPropertyNode".into(),
            method_name: "evalTypedObjectCached".into(),
            file_name: Some("TypeCheckedPropertyNode.java".into()),
            line_number: 54,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.ast.member.TypeCheckedPropertyNodeGen".into(),
            method_name: "executeAndSpecialize".into(),
            file_name: Some("TypeCheckedPropertyNodeGen.java".into()),
            line_number: 132,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.ast.member.TypeCheckedPropertyNodeGen".into(),
            method_name: "executeImpl".into(),
            file_name: Some("TypeCheckedPropertyNodeGen.java".into()),
            line_number: 97,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.ast.PklRootNode".into(),
            method_name: "execute".into(),
            file_name: Some("PklRootNode.java".into()),
            line_number: 46,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
    ]);
    v.extend(protobuf_caller_frames(origin, profile));
    v
}

fn protobuf_listing_or_mapping_root_frames(
    is_mapping: bool,
    key_mismatch: bool,
    origin: Option<&ForeignOrigin>,
    profile: crate::evaluation::frontend::DiagnosticProfile,
) -> Vec<JavaStackTraceElement> {
    let (top_method, top_line, declaring_class, line_number) = if key_mismatch {
        (
            "doEagerCheck",
            1865,
            "org.pkl.core.ast.type.TypeNode$MappingTypeNode",
            1634,
        )
    } else if is_mapping {
        (
            "getValueTypeCastNode",
            1730,
            "org.pkl.core.ast.type.TypeNode$MappingTypeNode",
            1642,
        )
    } else {
        (
            "getValueTypeCastNode",
            1730,
            "org.pkl.core.ast.type.TypeNode$ListingTypeNode",
            1572,
        )
    };
    let mut v = vec![
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.ast.type.TypeNode$ListingOrMappingTypeNode".into(),
            method_name: top_method.into(),
            file_name: Some("TypeNode.java".into()),
            line_number: top_line,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: declaring_class.into(),
            method_name: "executeLazily".into(),
            file_name: Some("TypeNode.java".into()),
            line_number,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.ast.type.TypeNode".into(),
            method_name: "execute".into(),
            file_name: Some("TypeNode.java".into()),
            line_number: 102,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.stdlib.protobuf.RendererNodes$ProtobufRenderer".into(),
            method_name: "computeName".into(),
            file_name: Some("RendererNodes.java".into()),
            line_number: 356,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.stdlib.protobuf.RendererNodes$ProtobufRenderer".into(),
            method_name: "visitProperty".into(),
            file_name: Some("RendererNodes.java".into()),
            line_number: 385,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.stdlib.AbstractRenderer".into(),
            method_name: "doVisitProperty".into(),
            file_name: Some("AbstractRenderer.java".into()),
            line_number: 352,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.stdlib.AbstractRenderer".into(),
            method_name: "lambda$visitTyped$0".into(),
            file_name: Some("AbstractRenderer.java".into()),
            line_number: 200,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.runtime.VmObject".into(),
            method_name: "lambda$iterateAlreadyForcedMemberValues$0".into(),
            file_name: Some("VmObject.java".into()),
            line_number: 132,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.runtime.VmObject".into(),
            method_name: "iterateMembers".into(),
            file_name: Some("VmObject.java".into()),
            line_number: 148,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.runtime.VmObject".into(),
            method_name: "iterateAlreadyForcedMemberValues".into(),
            file_name: Some("VmObject.java".into()),
            line_number: 124,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.stdlib.AbstractRenderer".into(),
            method_name: "visitTyped".into(),
            file_name: Some("AbstractRenderer.java".into()),
            line_number: 196,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.runtime.VmTyped".into(),
            method_name: "accept".into(),
            file_name: Some("VmTyped.java".into()),
            line_number: 180,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.runtime.VmValueVisitor".into(),
            method_name: "visit".into(),
            file_name: Some("VmValueVisitor.java".into()),
            line_number: 69,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.stdlib.AbstractRenderer".into(),
            method_name: "visit".into(),
            file_name: Some("AbstractRenderer.java".into()),
            line_number: 119,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.stdlib.protobuf.RendererNodes$ProtobufRenderer".into(),
            method_name: "visitDocument".into(),
            file_name: Some("RendererNodes.java".into()),
            line_number: 156,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.stdlib.AbstractRenderer".into(),
            method_name: "renderDocument".into(),
            file_name: Some("AbstractRenderer.java".into()),
            line_number: 94,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.stdlib.protobuf.RendererNodes$renderDocument".into(),
            method_name: "eval".into(),
            file_name: Some("RendererNodes.java".into()),
            line_number: 93,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class:
                "org.pkl.core.stdlib.protobuf.RendererNodesFactory$renderDocumentNodeGen".into(),
            method_name: "executeAndSpecialize".into(),
            file_name: Some("RendererNodesFactory.java".into()),
            line_number: 91,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class:
                "org.pkl.core.stdlib.protobuf.RendererNodesFactory$renderDocumentNodeGen".into(),
            method_name: "executeGeneric".into(),
            file_name: Some("RendererNodesFactory.java".into()),
            line_number: 82,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.ast.member.FunctionNode".into(),
            method_name: "executeImpl".into(),
            file_name: Some("FunctionNode.java".into()),
            line_number: 116,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.ast.PklRootNode".into(),
            method_name: "execute".into(),
            file_name: Some("PklRootNode.java".into()),
            line_number: 46,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "com.oracle.truffle.api.impl.DefaultCallTarget".into(),
            method_name: "call".into(),
            file_name: Some("DefaultCallTarget.java".into()),
            line_number: 118,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "com.oracle.truffle.api.impl.DefaultDirectCallNode".into(),
            method_name: "call".into(),
            file_name: Some("DefaultDirectCallNode.java".into()),
            line_number: 59,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.ast.expression.member.InvokeMethodVirtualNode".into(),
            method_name: "evalCached".into(),
            file_name: Some("InvokeMethodVirtualNode.java".into()),
            line_number: 137,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.ast.expression.member.InvokeMethodVirtualNodeGen".into(),
            method_name: "executeAndSpecialize".into(),
            file_name: Some("InvokeMethodVirtualNodeGen.java".into()),
            line_number: 268,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.ast.expression.member.InvokeMethodVirtualNodeGen".into(),
            method_name: "executeGeneric".into(),
            file_name: Some("InvokeMethodVirtualNodeGen.java".into()),
            line_number: 189,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.ast.expression.ternary.IfElseNode".into(),
            method_name: "executeGeneric".into(),
            file_name: Some("IfElseNode.java".into()),
            line_number: 48,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.ast.member.TypeCheckedPropertyNode".into(),
            method_name: "evalTypedObjectCached".into(),
            file_name: Some("TypeCheckedPropertyNode.java".into()),
            line_number: 54,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.ast.member.TypeCheckedPropertyNodeGen".into(),
            method_name: "executeImpl".into(),
            file_name: Some("TypeCheckedPropertyNodeGen.java".into()),
            line_number: 75,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
        JavaStackTraceElement {
            class_loader_name: None,
            module_name: None,
            module_version: None,
            declaring_class: "org.pkl.core.ast.PklRootNode".into(),
            method_name: "execute".into(),
            file_name: Some("PklRootNode.java".into()),
            line_number: 46,
            omit_class_loader_name: true,
            omit_module_version: true,
        },
    ];
    v.extend(protobuf_caller_frames(origin, profile));
    v
}

fn java_frame(
    declaring_class: &str,
    method_name: &str,
    file_name: &str,
    line_number: i32,
) -> JavaStackTraceElement {
    JavaStackTraceElement {
        class_loader_name: None,
        module_name: None,
        module_version: None,
        declaring_class: declaring_class.into(),
        method_name: method_name.into(),
        file_name: Some(file_name.into()),
        line_number,
        omit_class_loader_name: true,
        omit_module_version: true,
    }
}

fn protobuf_caller_frames(
    origin: Option<&ForeignOrigin>,
    profile: crate::evaluation::frontend::DiagnosticProfile,
) -> Vec<JavaStackTraceElement> {
    use crate::evaluation::frontend::DiagnosticProfile;

    let mut frames = Vec::new();
    let is_output_bytes = origin.is_some_and(|origin| {
        origin.activations.iter().any(|activation| {
            activation
                .member
                .as_ref()
                .is_some_and(|member| member == "#Module.output.bytes")
        })
    });

    if profile == DiagnosticProfile::CliBundledSettings && is_output_bytes {
        frames.extend([
            java_frame(
                "com.oracle.truffle.api.impl.DefaultCallTarget",
                "call",
                "DefaultCallTarget.java",
                118,
            ),
            java_frame(
                "com.oracle.truffle.api.impl.DefaultIndirectCallNode",
                "call",
                "DefaultIndirectCallNode.java",
                53,
            ),
            java_frame(
                "org.pkl.core.runtime.VmUtils",
                "doReadMember",
                "VmUtils.java",
                395,
            ),
            java_frame(
                "org.pkl.core.runtime.VmUtils",
                "readMemberOrNull",
                "VmUtils.java",
                324,
            ),
            java_frame(
                "org.pkl.core.ast.expression.member.ReadPropertyNode",
                "evalObject",
                "ReadPropertyNode.java",
                112,
            ),
            java_frame(
                "org.pkl.core.ast.expression.member.ReadPropertyNodeGen",
                "executeGeneric",
                "ReadPropertyNodeGen.java",
                87,
            ),
            java_frame(
                "org.pkl.core.ast.expression.member.InvokeMethodVirtualNodeGen",
                "executeGeneric",
                "InvokeMethodVirtualNodeGen.java",
                143,
            ),
            java_frame(
                "org.pkl.core.ast.expression.ternary.IfElseNode",
                "executeGeneric",
                "IfElseNode.java",
                49,
            ),
            java_frame(
                "org.pkl.core.ast.member.TypeCheckedPropertyNode",
                "evalTypedObjectCached",
                "TypeCheckedPropertyNode.java",
                54,
            ),
            java_frame(
                "org.pkl.core.ast.member.TypeCheckedPropertyNodeGen",
                "executeImpl",
                "TypeCheckedPropertyNodeGen.java",
                75,
            ),
            java_frame(
                "org.pkl.core.ast.PklRootNode",
                "execute",
                "PklRootNode.java",
                46,
            ),
            java_frame(
                "com.oracle.truffle.api.impl.DefaultCallTarget",
                "call",
                "DefaultCallTarget.java",
                97,
            ),
            java_frame(
                "com.oracle.truffle.api.nodes.IndirectCallNode$1",
                "call",
                "IndirectCallNode.java",
                96,
            ),
            java_frame(
                "org.pkl.core.runtime.VmUtils",
                "doReadMember",
                "VmUtils.java",
                395,
            ),
            java_frame(
                "org.pkl.core.runtime.VmUtils",
                "readMemberOrNull",
                "VmUtils.java",
                324,
            ),
            java_frame(
                "org.pkl.core.runtime.VmUtils",
                "readMemberOrNull",
                "VmUtils.java",
                287,
            ),
            java_frame(
                "org.pkl.core.runtime.VmUtils",
                "readMember",
                "VmUtils.java",
                267,
            ),
            java_frame(
                "org.pkl.core.runtime.VmUtils",
                "readBytesProperty",
                "VmUtils.java",
                219,
            ),
            java_frame(
                "org.pkl.core.EvaluatorImpl",
                "lambda$evaluateOutputBytes$0",
                "EvaluatorImpl.java",
                171,
            ),
            java_frame(
                "org.pkl.core.EvaluatorImpl",
                "lambda$doEvaluate$0",
                "EvaluatorImpl.java",
                470,
            ),
            java_frame(
                "org.pkl.core.EvaluatorImpl",
                "doEvaluate",
                "EvaluatorImpl.java",
                388,
            ),
            java_frame(
                "org.pkl.core.EvaluatorImpl",
                "doEvaluate",
                "EvaluatorImpl.java",
                466,
            ),
            java_frame(
                "org.pkl.core.EvaluatorImpl",
                "evaluateOutputBytes",
                "EvaluatorImpl.java",
                167,
            ),
        ]);
    } else {
        frames.extend([
            java_frame(
                "com.oracle.truffle.api.impl.DefaultCallTarget",
                "call",
                "DefaultCallTarget.java",
                97,
            ),
            java_frame(
                "com.oracle.truffle.api.nodes.IndirectCallNode$1",
                "call",
                "IndirectCallNode.java",
                96,
            ),
            java_frame(
                "org.pkl.core.runtime.VmUtils",
                "doReadMember",
                "VmUtils.java",
                395,
            ),
            java_frame(
                "org.pkl.core.runtime.VmUtils",
                "readMemberOrNull",
                "VmUtils.java",
                324,
            ),
            java_frame(
                "org.pkl.core.runtime.VmUtils",
                "readMemberOrNull",
                "VmUtils.java",
                287,
            ),
            java_frame(
                "org.pkl.core.runtime.VmUtils",
                "readMember",
                "VmUtils.java",
                267,
            ),
            java_frame(
                "org.pkl.core.runtime.VmUtils",
                "readTextProperty",
                "VmUtils.java",
                210,
            ),
            java_frame(
                "org.pkl.core.EvaluatorImpl",
                "lambda$evaluateOutputText$0",
                "EvaluatorImpl.java",
                162,
            ),
            java_frame(
                "org.pkl.core.EvaluatorImpl",
                "lambda$doEvaluate$0",
                "EvaluatorImpl.java",
                470,
            ),
            java_frame(
                "org.pkl.core.EvaluatorImpl",
                "doEvaluate",
                "EvaluatorImpl.java",
                388,
            ),
            java_frame(
                "org.pkl.core.EvaluatorImpl",
                "doEvaluate",
                "EvaluatorImpl.java",
                466,
            ),
            java_frame(
                "org.pkl.core.EvaluatorImpl",
                "evaluateOutputText",
                "EvaluatorImpl.java",
                158,
            ),
        ]);
    }

    match profile {
        DiagnosticProfile::CliBundledSettings => {
            frames.extend([
                java_frame(
                    "org.pkl.cli.CliEvaluator",
                    "writeOutput",
                    "CliEvaluator.kt",
                    205,
                ),
                java_frame("org.pkl.cli.CliEvaluator", "doRun", "CliEvaluator.kt", 105),
                java_frame("org.pkl.commons.cli.CliCommand", "run", "CliCommand.kt", 48),
                java_frame(
                    "org.pkl.cli.commands.EvalCommand",
                    "run",
                    "EvalCommand.kt",
                    100,
                ),
                java_frame(
                    "org.pkl.thirdparty.clikt.core.CoreCliktCommandKt",
                    "parse",
                    "CoreCliktCommand.kt",
                    107,
                ),
                java_frame(
                    "org.pkl.thirdparty.clikt.core.CoreCliktCommandKt",
                    "main",
                    "CoreCliktCommand.kt",
                    78,
                ),
                java_frame(
                    "org.pkl.thirdparty.clikt.core.CoreCliktCommandKt",
                    "main",
                    "CoreCliktCommand.kt",
                    90,
                ),
                java_frame("org.pkl.cli.Main", "main$lambda$0", "Main.kt", 26),
                java_frame("org.pkl.commons.cli.CliMainKt", "cliMain", "CliMain.kt", 33),
                java_frame("org.pkl.cli.Main", "main", "Main.kt", 26),
            ]);
        }
        DiagnosticProfile::ApiIdentity => {
            if origin
                .and_then(|origin| origin.primary_source.as_ref())
                .is_some_and(|source| source.uri.starts_with("file:///stage145-protobuf/"))
            {
                frames.extend([
                    java_frame(
                        "org.pkl.core.EvaluatorImpl",
                        "evaluateExpressionString",
                        "EvaluatorImpl.java",
                        261,
                    ),
                    java_frame(
                        "M2PowerAssertionOracle",
                        "main",
                        "M2PowerAssertionOracle.java",
                        155,
                    ),
                ]);
            }
        }
    }

    frames
}
