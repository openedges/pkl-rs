//! Bounded, resumable bytecode candidate. This is an architecture experiment, not full Pkl.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use crate::heap::{
    CacheKey, Class, CollectionKind, Environment, ExecutableOwnerId, ExecutionSite, Function, Heap,
    HeapStats, LazyState, Member, Object, Scope, TypeNodeId,
};
use crate::logging::{EvaluationContext, FrameTransformRequest, LogRequest, LoggerFailure};
use crate::source::Span;
use crate::value::{ErrorKind, ObjectId, PklString, RuntimeError, RuntimeFrame, Value};
pub(crate) mod amendment;
mod analyze;
pub(crate) mod annotations;
mod any;
mod base_snapshot;
pub(crate) mod builtins;
mod bytes;
pub(crate) mod classes;
mod code_storage;
mod collection_addition;
pub(crate) mod collection_conversions;
mod collection_transforms;
pub mod collections;
pub mod columnar_value;
mod data_size;
pub(crate) mod deprecation;
mod duration;
mod fast_int;
mod int_seq;
mod lazy_collections;
mod list_ops;
mod map_ops;
mod mapping_ops;
pub(crate) mod reflection;
mod regex;
pub(crate) mod repl;
pub(crate) mod schema;
mod set_order;
mod set_sequence;
mod string_transforms;
pub use code_storage::CodeStorage;
mod compiler_expressions;
mod converters;
pub(crate) mod equality;
mod forcing;
mod generics;
pub(crate) mod glob;
mod glob_pattern;
mod glob_traversal;
mod guest_exceptions;
pub(crate) mod json_parser;
mod jsonnet;
mod modules;
mod null;
mod platform;
mod query_lifecycle;
mod release;
mod session_expressions;
mod settings_paths;
mod stdlib_modules;
mod xml;
mod yaml_parser;
pub use session_expressions::{SessionExpressionError, compile_session_expression};
mod frame_transformation;
pub(crate) mod identity;
mod object_lookups;
mod output;
mod pair;
mod pcf_format;
mod pkl_binary_wire;
mod plist;
mod primitives;
#[cfg(test)]
mod receiver_tests;
pub(crate) mod reference;
mod rendering;
mod resources;
mod strings;
mod tracing;
#[cfg(test)]
mod type_equivalence_tests;
mod type_resolution;
mod typecheck;
mod uri_resolution;
mod yaml;
use typecheck::{TypeDestination, TypeRun};

#[derive(Clone, Debug)]
pub enum Instruction {
    LoadRefModule,
    LoadJsonnetModule,
    LoadXmlModule,
    LoadProtobufModule,
    LoadPklBinaryModule,
    ReflectCall(reflection::Operation),
    /// Execute the authored pkl:json Parser.parse external body.
    JsonParse,
    /// Pure lexical path resolution for authored EvaluatorSettings.resolveForOs.
    ResolveEvaluatorSettingsPath,
    PlatformCurrent,
    ReleaseCurrent,
    BenchmarkRun,
    AnalyzeImportGraph,
    YamlParse {
        all: bool,
    },
    MirrorProperty(PklString),
    LoadBaseModule,
    /// Invoke a checked Function0 under a guest exception handler.
    GuestCatch {
        or_null: bool,
    },
    ImportModule(modules::ImportSpec),
    LoadPrimary(Arc<crate::modules::ModuleSourceSpec>),
    EvaluateQuery(crate::source::SourceText),
    EvaluateRawQuery(crate::source::SourceText),
    PrepareModuleImports(crate::modules::UnitId),
    CheckModuleOutput,
    /// Eager initialization in the private standard-library profile.
    BootstrapStdlib,
    MatchModuleVersion,
    ReadModuleVersion,
    CheckModuleVersion,
    EncodeUtf8,
    EncodePklBinary,
    OutputRenderer,
    RenderPcf {
        document: bool,
    },
    RenderPklBinary {
        document: bool,
    },
    RenderProtobuf {
        document: bool,
    },
    ProtobufRenderType,
    RenderXml {
        document: bool,
    },
    RenderJsonnet {
        document: bool,
    },
    RenderJson {
        document: bool,
    },
    RenderProperties {
        document: bool,
    },
    RenderPlist {
        document: bool,
    },
    RenderYaml {
        document: bool,
    },
    CheckAnnotationClass,
    PublishModuleMember {
        code: usize,
    },
    FinishModule,
    InitializeModuleParent,
    AppendAnnotation,
    BeginMemberAnnotations,
    InitializeDeprecation,
    StoreDeprecation,
    /// ObjectMethodNode initializes types via a raw call with (owner, owner).
    BeginObjectMethodSignature,
    EndObjectMethodSignature,
    PrepareMethodCall {
        lookup: usize,
        arity: usize,
        after_arguments: bool,
    },
    PublishMemberAnnotations {
        code: usize,
    },
    PublishAnnotatedClassMember {
        template: usize,
        index: usize,
    },
    BuiltinTypeAlias(PklString),
    BeginTypeAlias {
        declaration: usize,
        name: PklString,
        end: usize,
    },
    FinishTypeAlias,
    CheckTypeAliasInitialized,
    ValidateTypeArguments {
        ty: TypeCheck,
        aliases: Vec<RuntimeFrame>,
    },
    BuiltinClass(TypeCheck),
    ReferenceRenderMethod,
    Constant(Value),
    WrapNullableDefault {
        defined: bool,
    },
    StripNullDefault,
    AmendEmpty,
    Input(usize),
    LoadLocal(usize),
    StoreLocal(usize),
    /// Check first, then store using the declaration's frame-slot representation.
    BindLocal {
        depth: Option<usize>,
        ty: TypeCheck,
    },
    PushLocal,
    PopLocal,
    EnterForScope {
        bindings: usize,
    },
    LeaveForScope,
    Swap,
    /// Place a late-resolved lexical method beneath its already evaluated arguments.
    RotateStackRight(usize),
    BeginCollection {
        kind: crate::heap::CollectionKind,
        has_parent: bool,
        specialized: bool,
    },
    EndCollection,
    SetCollectionDefault {
        defined: bool,
        origin: Option<RuntimeFrame>,
    },
    /// Instantiate a collection element default after lexical type substitution.
    SetCollectionTypeDefault {
        ty: TypeCheck,
        origin: Option<RuntimeFrame>,
    },
    /// Resolve a generic constructor's parent using this invocation's actual type.
    GenericConstructorParent {
        ty: TypeCheck,
        source: TypeArgumentSource,
        cache: bool,
    },
    AddCollectionProperty(Member),
    AddCollectionElement(Member),
    AddCollectionEntry {
        member: Member,
        check_index: bool,
        span: Span,
    },
    CheckMapSpread,
    OpenIterator {
        nullable: bool,
        spread: bool,
    },
    IteratorNext {
        end: usize,
    },
    SpreadMember,
    OpenPredicate,
    PredicateNext {
        end: usize,
    },
    AddPredicateMember(Member),
    Subscript {
        key_span: Span,
    },
    SuperSubscript {
        end: usize,
    },
    EntryParent {
        end: usize,
    },
    EntryDefault,
    EntryKey,
    ApplyEntryDefault,
    EmptyDynamic,
    InferPropertyParent,
    MakeList {
        length: usize,
    },
    /// Hash and insert one strict Map pair before evaluating the next pair.
    PrepareMapEntry,
    AddMapEntry,
    /// Hash and deduplicate one strict Set element before the next argument.
    PrepareSetElement,
    AddSetElement,
    /// Paguro keySet builds its hash table with put, without a contains probe.
    PutSetKey,
    LoadThis,
    LoadCustomThis,
    LoadOuter,
    LoadModule,
    NonNull,
    Throw,
    Trace {
        operand_span: Span,
    },
    ToString,
    /// Validates/stringifies an interpolated expression left-to-right, deferring
    /// pure `String`/`Int` formatting to `FinishInterpolation` for zero-temp concat.
    CheckInterpolate,
    /// Concatenates the top `count` checked interpolation parts into one `PklString`.
    FinishInterpolation(usize),
    /// Internal Java Object.toString conversion used by collection/foreign values.
    JavaValueString,
    StringRepeat,
    CollectionJoin,
    BeginConversion(collection_conversions::Operation),
    ObjectLookup(object_lookups::Lookup),
    ModuleRelativePathTo,
    ObjectDefault,
    ObjectDefaultProperty,
    ConversionNext {
        end: usize,
    },
    ConversionValue,
    ConversionPair,
    AppendConvertedMember,
    EndConversion,
    ConvertObject(collection_conversions::Operation),
    ReadConversionProperty,
    BeginLazyCollection(lazy_collections::Operation),
    LazyCollectionNext {
        end: usize,
    },
    LazyCollectionAccept {
        next: usize,
        end: usize,
    },
    EndLazyCollection,
    BeginCollectionEvery,
    CollectionEveryAccept {
        next: usize,
    },
    EndCollectionEvery,
    BeginCollectionTransform(collection_transforms::Transform),
    BeginCollectionProjection(collection_transforms::Projection),
    CollectionKeyOrder,
    CollectionTransformNext {
        end: usize,
    },
    CollectionTransformProject {
        next: usize,
    },
    CollectionFlatMapNext {
        end: usize,
    },
    AppendStrictElement,
    EndCollectionTransform,
    CollectionDistinctStep {
        next: usize,
        end: usize,
    },
    CollectionContainsStep {
        matched: usize,
    },
    StringEncode,
    Primitive(primitives::Operation),
    MakePair,
    PairConstructor,
    IntSeqConstructor,
    RegexConstructor,
    MakeRegex,
    RegexMatchProperty(regex::Property),
    MakeIntSeq,
    CheckIntSeqStep,
    BeginIntSeq(int_seq::Operation),
    IntSeqNext {
        end: usize,
    },
    IntSeqAccept,
    EndIntSeq,
    NullConstructor,
    CheckNullDefault,
    CheckObjectParameters {
        target: amendment::ParameterTarget,
    },
    PrepareFunctionAmendment {
        body: usize,
        arity: usize,
    },
    BeginFunctionAmendment {
        end: usize,
        function: Option<(usize, usize)>,
    },
    EndFunctionAmendment,
    InvokeAmendedParent,
    ContinueFunctionAmendment,
    CheckMixinArgument,
    Observe(Span),
    ObserveNativeLiteral {
        span: Span,
        rendered: PklString,
    },
    TestType(TypeCheck),
    JumpIfNull(usize),
    GetMember(PklString),
    GetMethod(PklString),
    GetImplicitProperty(PklString),
    GetImplicitMethod(PklString),
    GetImplicitCustomProperty(PklString),
    GetImplicitCustomMethod(PklString),
    GetConstMember(PklString),
    GetConstMethod(PklString),
    RequireConstProperty {
        name: PklString,
        owner_depth: usize,
        local: bool,
        method: bool,
    },
    ResolveType(TypeCheck),
    /// Resolve the actual declaration and class arity before argument nodes.
    /// Canonical Class<T> skips that argument region without resolving T.
    PrepareTypeArguments {
        recipe: usize,
        end: usize,
    },
    ResolvedClass(TypeCheck),
    /// A superclass use retains its authored label through deferred alias lookup.
    /// Annotation and default-instance class resolution use ResolvedClass instead.
    ResolvedSupertype {
        ty: TypeCheck,
        label: PklString,
    },
    ResolvedDefault {
        ty: TypeCheck,
        origin: Option<RuntimeFrame>,
    },
    ResolvedInferredParent {
        ty: TypeCheck,
        origin: Option<RuntimeFrame>,
        cache: bool,
    },
    ResolveClassType {
        name: PklString,
        local: bool,
    },
    GetProperty {
        name: PklString,
        owner_depth: usize,
        local: bool,
    },
    GetAmbiguousProperty {
        name: PklString,
        owner_depth: usize,
        needs_const: bool,
    },
    GetMethodProperty {
        name: PklString,
        owner_depth: usize,
        local: bool,
    },
    GetSuperMember(PklString),
    GetSuperMethod(PklString),
    GetConstSuperMember(PklString),
    GetConstSuperMethod(PklString),
    MakeFunction {
        block: usize,
        arity: usize,
        method: bool,
        capture_custom_this: bool,
    },
    PipeApply,
    Call {
        arity: usize,
        method: bool,
    },
    CheckType(TypeCheck),
    Duplicate,
    Pop,
    Negate,
    Not,
    CheckBoolean,
    CheckLogicalLeft {
        or: bool,
    },
    CheckLogicalRight {
        or: bool,
    },
    JumpIfNotNull(usize),
    /// Publish the evaluator-owned singleton before any initialization can fail.
    BeginClass {
        template: usize,
        class: Class,
        end: usize,
    },
    SetClassParent,
    SetDefaultClassParent,
    PublishClassMember {
        template: usize,
        index: usize,
    },
    FinishClass,
    MakeInstance {
        template: usize,
    },
    MakeBuiltinInstance {
        template: usize,
    },
    DefaultClassInstance,
    CheckByte,
    MakeBytes {
        length: usize,
    },
    Undefined(PklString),
    AbstractMember {
        name: PklString,
        method: bool,
    },
    Unsupported(PklString),
    Fail(RuntimeError),
    MakeObject {
        template: usize,
        has_parent: bool,
    },
    Add,
    /// Fused literal-right addition; preserves checked integer overflow and numeric types.
    AddIntConstant(i64),
    Subtract,
    Multiply,
    Divide,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    Equal,
    NotEqual,
    IntegerDivide,
    Remainder,
    Power,
    Jump(usize),
    JumpIfFalse(usize),
    ReadResource,
    /// Operand validation precedes glob URI preparation and Host expansion.
    ReadResourceGlob(glob::Spec),
    ImportGlob(glob::Spec),
    GlobMember,
    ResourceProperty(resources::Property),
    ReadResourceOrNull,
    Return,
}

/// Type checks supported by the shared evaluator. Parametric collections use
/// resumable checks and lazy wrappers; function types preserve metadata and check arity.
#[derive(Clone, Debug)]
pub enum TypeCheck {
    /// Immutable instantiated argument graph; cloning never copies its descendants.
    Shared(generics::SharedTypeCheck),
    /// A unit-owned declaration lookup, resolved by the evaluator at its use site.
    Declared {
        recipe: usize,
        arguments: Vec<TypeCheck>,
    },
    Unknown,
    /// A lexical parameter in a reusable constraint template. Resolved before checking.
    Parameter {
        block: usize,
        name: PklString,
    },
    /// Declaration identity and arguments are retained independently of validation.
    NamedAlias {
        declaration: usize,
        name: PklString,
        arguments: Vec<TypeCheck>,
        expanded: Box<TypeCheck>,
        default_origin: Option<RuntimeFrame>,
    },
    /// Function annotations validate arity; their parameter/result types are metadata.
    Function {
        parameters: Option<Vec<TypeCheck>>,
        result: Box<TypeCheck>,
        syntax: bool,
    },
    /// Public parameterized VarArgs preserves its argument metadata but rejects
    /// value checking/default acquisition at the corresponding runtime boundary.
    VarArgs {
        element: Box<TypeCheck>,
        span: Span,
    },
    MetaClass(PklString),
    IntegerAlias {
        name: PklString,
        min: i64,
        max: i64,
    },
    RefDomain,
    Reference {
        domain: Box<TypeCheck>,
        referent: Box<TypeCheck>,
    },
    Any,
    Nothing,
    NonNull,
    Null,
    Boolean,
    Int,
    Float,
    Duration,
    Number,
    String,
    Bytes,
    Collection(crate::heap::CollectionKind),
    /// Abstract `Collection` type, distinct from every concrete heap kind.
    /// An empty argument list is the bare class; a parameterized use has one.
    CollectionBase {
        arguments: Vec<(TypeCheck, Span)>,
    },
    Pair {
        arguments: Vec<(TypeCheck, Span)>,
    },
    ParameterizedCollection {
        kind: crate::heap::CollectionKind,
        arguments: Vec<(TypeCheck, Span)>,
    },
    StringLiteral(PklString),
    Class {
        id: usize,
        name: PklString,
        module: Option<PklString>,
    },
    Nullable(Box<TypeCheck>),
    Alias(Box<TypeCheck>),
    /// Generic aliases themselves participate in upstream union eager classification.
    GenericAlias(Box<TypeCheck>),
    Union {
        /// Pinned AST distinguishes an authored union of string constants.
        string_literals: bool,
        types: Vec<TypeCheck>,
        spans: Vec<Span>,
        labels: Vec<Span>,
        default: Option<usize>,
    },
    Constrained {
        base: Box<TypeCheck>,
        base_span: Span,
        constraints: Vec<ConstraintCode>,
    },
}

#[derive(Clone, Debug)]
pub struct ConstraintCode {
    section: Option<crate::type_source::TypeSourceSection>,
    block: usize,
    /// Actual arguments for this constraint template, possibly referring to an outer template.
    arguments: BTreeMap<PklString, TypeCheck>,
    argument_sources: BTreeMap<PklString, TypeArgumentSource>,
    span: Span,
    module_environment: bool,
    source: crate::source::SourceText,
}

/// Authored source of an instantiated type node, kept separate from type identity.
#[derive(Clone, Debug)]
pub struct TypeArgumentSource {
    pub span: Span,
    pub text: crate::source::SourceText,
}

impl TypeCheck {
    fn unaliased(&self) -> &Self {
        let mut current = self;
        loop {
            match current.resolved() {
                Self::Alias(inner)
                | Self::GenericAlias(inner)
                | Self::NamedAlias {
                    expanded: inner, ..
                } => current = inner,
                ty => return ty,
            }
        }
    }
    pub(crate) fn equivalent(&self, other: &Self) -> bool {
        let mut pending = vec![(self, other)];
        let mut visited = BTreeSet::new();
        while let Some((left, right)) = pending.pop() {
            let left = left.unaliased();
            let right = right.unaliased();
            if std::ptr::eq(left, right) {
                continue;
            }
            if !visited.insert((left as *const Self, right as *const Self)) {
                continue;
            }
            let equal = match (left, right) {
                (Self::MetaClass(a), Self::MetaClass(b)) => a == b,
                (Self::VarArgs { element: a, .. }, Self::VarArgs { element: b, .. }) => {
                    pending.push((a, b));
                    true
                }
                (
                    Self::Function {
                        parameters: a,
                        result: ar,
                        syntax: ac,
                    },
                    Self::Function {
                        parameters: b,
                        result: br,
                        syntax: bc,
                    },
                ) => {
                    pending.push((ar, br));
                    ac == bc
                        && match (a, b) {
                            (None, None) => true,
                            (Some(a), Some(b)) if a.len() == b.len() => {
                                pending.extend(a.iter().zip(b));
                                true
                            }
                            _ => false,
                        }
                }
                (
                    Self::Reference {
                        domain: ad,
                        referent: ar,
                    },
                    Self::Reference {
                        domain: bd,
                        referent: br,
                    },
                ) => {
                    pending.extend([(ad.as_ref(), bd.as_ref()), (ar.as_ref(), br.as_ref())]);
                    true
                }
                (Self::IntegerAlias { name: a, .. }, Self::IntegerAlias { name: b, .. }) => a == b,
                (Self::Nullable(a), Self::Nullable(b)) => {
                    pending.push((a, b));
                    true
                }
                (Self::StringLiteral(a), Self::StringLiteral(b)) => a == b,
                (Self::Class { id: a, .. }, Self::Class { id: b, .. }) => a == b,
                (Self::Collection(a), Self::Collection(b)) => a == b,
                (Self::Pair { arguments: a }, Self::Pair { arguments: b }) => {
                    pending.extend(a.iter().zip(b).map(|((a, _), (b, _))| (a, b)));
                    a.len() == b.len()
                }
                (Self::CollectionBase { arguments: a }, Self::CollectionBase { arguments: b }) => {
                    // CollectionTypeNode is equivalent only to the same node.
                    a.is_empty() && b.is_empty()
                }
                (
                    Self::ParameterizedCollection {
                        kind: a,
                        arguments: aa,
                    },
                    Self::ParameterizedCollection {
                        kind: b,
                        arguments: ba,
                    },
                ) => {
                    pending.extend(aa.iter().zip(ba).map(|((a, _), (b, _))| (a, b)));
                    a == b && aa.len() == ba.len()
                }
                (
                    Self::Union {
                        types: a,
                        string_literals: al,
                        ..
                    },
                    Self::Union {
                        types: b,
                        string_literals: bl,
                        ..
                    },
                ) => match (al, bl) {
                    (true, true) => {
                        // Java's optimized literal union compares its set of
                        // strings. Order, duplicates and defaults do not affect
                        // whether a lazy collection can reuse this type check.
                        let literals = |types: &[Self]| {
                            types
                                .iter()
                                .map(|ty| match ty.resolved() {
                                    Self::StringLiteral(value) => value.clone(),
                                    _ => unreachable!("literal union metadata"),
                                })
                                .collect::<BTreeSet<_>>()
                        };
                        literals(a) == literals(b)
                    }
                    (false, false) => {
                        // General unions compare ordered child checks; their
                        // default alternatives do not change check equivalence.
                        pending.extend(a.iter().zip(b));
                        a.len() == b.len()
                    }
                    _ => false,
                },
                (
                    Self::Constrained {
                        base: a,
                        constraints: ac,
                        ..
                    },
                    Self::Constrained {
                        base: b,
                        constraints: bc,
                        ..
                    },
                ) => {
                    pending.push((a, b));
                    ac.len() == bc.len() && ac.iter().zip(bc).all(|(a, b)| a.block == b.block)
                }
                (
                    Self::Unknown
                    | Self::RefDomain
                    | Self::Any
                    | Self::Nothing
                    | Self::NonNull
                    | Self::Null
                    | Self::Boolean
                    | Self::Int
                    | Self::Float
                    | Self::Duration
                    | Self::Number
                    | Self::String
                    | Self::Bytes,
                    _,
                ) => std::mem::discriminant(left) == std::mem::discriminant(right),
                _ => false,
            };
            if !equal {
                return false;
            }
        }
        true
    }
    fn noop(&self) -> bool {
        match self {
            Self::Shared(node) => node.noop(),
            Self::Any | Self::Unknown => true,
            Self::Alias(inner)
            | Self::GenericAlias(inner)
            | Self::NamedAlias {
                expanded: inner, ..
            } => inner.noop(),
            Self::Union { types, .. } => types.iter().all(Self::noop),
            _ => false,
        }
    }
    fn accepts(&self, value: &Value, heap: &Heap) -> bool {
        match self.resolved() {
            Self::Any | Self::Unknown => return true,
            Self::Int => return matches!(value, Value::Int(_)),
            Self::Float => return matches!(value, Value::Float(_)),
            Self::Number => return matches!(value, Value::Int(_) | Value::Float(_)),
            Self::Boolean => return matches!(value, Value::Bool(_)),
            Self::String => return matches!(value, Value::String(_)),
            Self::NonNull => return !value.is_null(),
            Self::Null => return value.is_null(),
            Self::Duration => return matches!(value, Value::Duration(_)),
            Self::Bytes => return matches!(value, Value::Bytes(_)),
            Self::IntegerAlias { min, max, .. } => {
                return matches!(value, Value::Int(i) if i >= min && i <= max);
            }
            Self::StringLiteral(expected) => {
                return matches!(value, Value::String(actual) if actual == expected);
            }
            Self::Nothing | Self::VarArgs { .. } => return false,
            _ => {}
        }
        enum Work<'a> {
            Check(&'a TypeCheck, &'a Value),
            Union(&'a [TypeCheck], &'a Value),
            Reference(&'a TypeCheck, &'a reference::Reference),
        }
        let mut pending = vec![Work::Check(self, value)];
        let mut accepted = false;
        while let Some(work) = pending.pop() {
            let (ty, value) = match work {
                Work::Check(ty, value) => (ty.resolved(), value),
                Work::Union(rest, value) => {
                    if !accepted && let Some((next, rest)) = rest.split_first() {
                        pending.push(Work::Union(rest, value));
                        pending.push(Work::Check(next, value));
                    }
                    continue;
                }
                Work::Reference(referent, reference) => {
                    if accepted {
                        accepted = reference::is_subtype(
                            &reference.referent,
                            &reference::normalize(referent, &PklString::default()),
                            heap,
                        );
                    }
                    continue;
                }
            };
            match ty {
                Self::Nullable(_) if value.is_null() => accepted = true,
                Self::Alias(inner)
                | Self::GenericAlias(inner)
                | Self::Nullable(inner)
                | Self::NamedAlias {
                    expanded: inner, ..
                } => pending.push(Work::Check(inner, value)),
                Self::Union { types, .. } => {
                    accepted = false;
                    pending.push(Work::Union(types, value));
                }
                Self::Reference { domain, referent } => {
                    accepted = false;
                    if let Value::Object(id) = value
                        && let Ok(object) = heap.get(*id)
                        && let Some(reference) = &object.reference
                    {
                        pending.push(Work::Reference(referent, reference));
                        pending.push(Work::Check(domain, &reference.domain));
                    }
                }
                _ => accepted = ty.accepts_leaf(value, heap),
            }
        }
        accepted
    }
    fn accepts_leaf(&self, value: &Value, heap: &Heap) -> bool {
        match self.resolved() {
            Self::Shared(_) => unreachable!(),
            Self::Declared { .. } => {
                unreachable!("declaration must be resolved before checking a value")
            }
            Self::Any | Self::Unknown => true,
            Self::MetaClass(name) => matches!(
                (name.as_str(), value),
                (Some("TypeAlias"), Value::TypeAlias(_)) | (Some("Class"), Value::Class(_))
            ),
            Self::Function { parameters, .. } => {
                matches!(value, Value::Function(id) if heap.get(*id).is_ok_and(|object| parameters.as_ref().is_none_or(|parameters| object.function.as_ref().is_some_and(|function| function.arity == parameters.len()) || object.native_function.as_ref().is_some_and(|function| parameters.len() == match function.name.as_ref() { "Reference" => 3, "containsKey" | "getOrNull" | "contains" => 1, _ => 0 }))))
            }
            Self::RefDomain => Self::Class {
                module: None,
                id: reference::DOMAIN_ID,
                name: "Domain".into(),
            }
            .accepts(value, heap),
            Self::IntegerAlias { min, max, .. } => {
                matches!(value,Value::Int(i) if i>=min && i<=max)
            }
            Self::Nothing | Self::VarArgs { .. } => false,
            Self::NonNull => !value.is_null(),
            Self::Null => value.is_null(),
            Self::Boolean => matches!(value, Value::Bool(_)),
            Self::Int => matches!(value, Value::Int(_)),
            Self::Float => matches!(value, Value::Float(_)),
            Self::Duration => matches!(value, Value::Duration(_)),
            Self::Number => matches!(value, Value::Int(_) | Value::Float(_)),
            Self::String => matches!(value, Value::String(_)),
            Self::Bytes => matches!(value, Value::Bytes(_)),
            Self::Pair { .. } => {
                matches!(value, Value::Object(id) if heap.get(*id).is_ok_and(|o|o.pair.is_some()))
            }
            Self::Collection(kind) => {
                matches!(value, Value::Object(id) if heap.get(*id).is_ok_and(|object| object.collection_kind == *kind && object.foreign_map_entry.is_none() && !object.is_module && object.reference.is_none() && !object.ref_module) && collections::typed_class(heap, *id).is_ok_and(|class| class.is_none()))
            }
            Self::ParameterizedCollection { kind, .. } => {
                Self::Collection(*kind).accepts(value, heap)
            }
            Self::CollectionBase { .. } => {
                Self::Collection(crate::heap::CollectionKind::List).accepts(value, heap)
                    || Self::Collection(crate::heap::CollectionKind::Set).accepts(value, heap)
            }
            Self::Class { id, .. } => classes::accepts_nominal(*id, value, heap),
            Self::StringLiteral(expected) => {
                matches!(value, Value::String(actual) if actual == expected)
            }
            Self::Nullable(_)
            | Self::Alias(_)
            | Self::GenericAlias(_)
            | Self::NamedAlias { .. }
            | Self::Union { .. }
            | Self::Reference { .. } => unreachable!("structural accepts task"),
            Self::Constrained { .. } | Self::Parameter { .. } => false,
        }
    }
}

/// Source metadata is parallel to bytecode, so diagnostics add no VM dispatches.
#[derive(Clone, Debug, Default)]
pub struct BlockMetadata {
    /// Compiling unit. An explicit source replacement may detach declaration
    /// provenance from this unit while preserving its executable code index.
    pub unit: Option<crate::modules::UnitId>,
    /// Bind the actual module input as the lexical receiver of a source entry.
    pub module_receiver_input: Option<usize>,
    pub source: Option<Arc<crate::value::RuntimeSource>>,
    pub hidden: bool,
    /// The Java shared glob member root has an empty qualified diagnostic name.
    pub(crate) anonymous_root: bool,
    pub(crate) stdlib_bootstrap: bool,
    pub(crate) constant_member: bool,
    pub spans: Vec<Option<Span>>,
    pub member: PklString,
    pub declaration: Option<Span>,
    pub member_span: Option<Span>,
    pub value_span: Option<Span>,
    pub property_typecheck: Option<usize>,
    /// Local properties authored in object bodies check in their member root.
    /// Module/class properties use a separate PropertyTypeNode call, even local ones.
    pub inline_property_typecheck: bool,
    pub(crate) amendment: bool,
    /// The synthetic super read in `name { ... }` has no Java source section.
    pub(crate) amendment_parent_span: Option<Span>,
    /// Inferred element/entry parents use a source-less ApplyVmFunction1Node.
    /// Keep authored `new` spans for evaluation; parent-validation errors
    /// at these sites have an unavailable innermost Java source section.
    pub(crate) inferred_entry_parent_spans: Vec<Span>,
    pub(crate) fixed_span: Option<Span>,
    pub(crate) const_span: Option<Span>,
    pub(crate) initialization_error: Option<RuntimeError>,
    pub(crate) method_signature: Option<(PklString, usize)>,
    pub(crate) method_arities: BTreeMap<usize, usize>,
    pub(crate) let_scopes: Vec<LetScope>,
    pub(crate) constraint_root: Option<Span>,
    pub(crate) inline_constraint: bool,
    pub(crate) type_casts: Vec<usize>,
}

#[derive(Clone, Debug)]
pub(crate) struct LetScope {
    body: std::ops::Range<usize>,
    frame: RuntimeFrame,
    writes_slot: bool,
}

type ExtraDeclarationSources = Vec<(
    std::ops::Range<usize>,
    Arc<[(usize, crate::syntax::DeclarationSource)]>,
)>;
type ConversionBlocksMap = BTreeMap<(Arc<str>, Arc<str>), (usize, usize)>;
type PrimitiveMethodsMap = BTreeMap<(Arc<str>, Arc<str>), usize>;

#[derive(Clone, Debug)]
pub struct Program {
    /// Immutable declaration source descriptors keyed by compiled member body.
    /// ModuleUnit.syntax retains type/annotation recipes without forcing values.
    pub declaration_sources: BTreeMap<usize, crate::syntax::DeclarationSource>,
    base_declaration_sources: Option<Arc<BTreeMap<usize, crate::syntax::DeclarationSource>>>,
    extra_declaration_sources: ExtraDeclarationSources,
    reflection_annotations: BTreeMap<(PklString, Span), usize>,
    base_reflection_annotations: Option<Arc<BTreeMap<(PklString, Span), usize>>>,
    pub(crate) units: CodeStorage<modules::ModuleUnit>,
    next_declaration: usize,
    type_recipes: CodeStorage<type_resolution::TypeRecipe>,
    alias_definitions: BTreeMap<usize, type_resolution::AliasDefinition>,
    base_alias_definitions: Option<Arc<BTreeMap<usize, type_resolution::AliasDefinition>>>,
    pub module_name: PklString,
    pub module_template: Option<usize>,
    /// Imports, classes and aliases exist before module annotation evaluation.
    pub(crate) initial_module_members: Vec<Member>,
    pub blocks: CodeStorage<Vec<Instruction>>,
    pub metadata: CodeStorage<BlockMetadata>,
    pub objects: CodeStorage<Vec<Member>>,
    builtin_objects: Arc<Vec<Vec<Member>>>,
    builtin_block_range: std::ops::Range<usize>,
    pub entry: usize,
    pub(crate) reference_render_block: usize,
    pub(crate) typed_string_block: usize,
    pub(crate) string_repeat_block: usize,
    pub(crate) collection_join_blocks: [usize; 4],
    collection_every_blocks: [usize; 5],
    collection_transform_blocks: [usize; 37],
    collection_projection_blocks: [usize; 16],
    conversion_blocks: Arc<ConversionBlocksMap>,
    conversion_delegate_block: usize,
    object_default_block: usize,
    object_default_property_blocks: [usize; 3],
    null_constructor_block: usize,
    pub(crate) string_encode_block: usize,
    pub(crate) pair_constructor_block: usize,
    primitive_methods: Arc<PrimitiveMethodsMap>,
    pub(crate) list_fold_block: usize,
    pub(crate) amended_function_block: usize,
    pub(crate) identity_mixin_block: usize,
    pub(crate) public_warm_blocks: [usize; 8],
    pub(crate) base_mixin_declaration: Option<usize>,
    pub(crate) access_members: Arc<Vec<Member>>,
    pub(crate) builtin_members: Arc<BTreeMap<usize, Vec<Member>>>,
    pub(crate) constraint_origins: BTreeMap<usize, (Vec<RuntimeFrame>, Vec<RuntimeFrame>)>,
}

impl Program {
    #[inline]
    pub(crate) fn declaration_source(
        &self,
        code: usize,
    ) -> Option<&crate::syntax::DeclarationSource> {
        if let Some(found) = self.declaration_sources.get(&code) {
            return Some(found);
        }
        for (range, entries) in &self.extra_declaration_sources {
            if range.contains(&code)
                && let Ok(idx) = entries.binary_search_by_key(&code, |(k, _)| *k)
            {
                return Some(&entries[idx].1);
            }
        }
        self.base_declaration_sources
            .as_ref()
            .and_then(|base| base.get(&code))
    }

    #[inline]
    pub(crate) fn reflection_annotation(&self, key: &(PklString, Span)) -> Option<usize> {
        self.reflection_annotations.get(key).copied().or_else(|| {
            self.base_reflection_annotations
                .as_ref()
                .and_then(|base| base.get(key).copied())
        })
    }

    #[inline]
    fn alias_definition(&self, id: usize) -> Option<&type_resolution::AliasDefinition> {
        self.alias_definitions.get(&id).or_else(|| {
            self.base_alias_definitions
                .as_ref()
                .and_then(|base| base.get(&id))
        })
    }

    /// Cached immutable baseline `Program` built with no initial user blocks.
    pub(crate) fn empty_shared() -> Arc<Self> {
        static EMPTY: std::sync::OnceLock<Arc<Program>> = std::sync::OnceLock::new();
        EMPTY
            .get_or_init(|| Arc::new(Self::build_uncached(Vec::new())))
            .clone()
    }

    /// Cached baseline `Program` for `SourceSession::module_evaluator` and `ReplSession::new`
    /// (`vec![vec![Instruction::Return]]`).
    pub(crate) fn session_shared() -> Arc<Self> {
        static SESSION: std::sync::OnceLock<Arc<Program>> = std::sync::OnceLock::new();
        SESSION
            .get_or_init(|| Arc::new(Self::build_uncached(vec![vec![Instruction::Return]])))
            .clone()
    }

    pub fn new(blocks: Vec<Vec<Instruction>>) -> Self {
        if blocks.is_empty() {
            return (*Self::empty_shared()).clone();
        }
        Self::build_uncached(blocks)
    }

    fn build_uncached(mut blocks: Vec<Vec<Instruction>>) -> Self {
        let builtin_start = blocks.len();
        let reference_render_block = blocks.len();
        blocks.push(vec![
            Instruction::LoadLocal(0),
            Instruction::ReferenceRenderMethod,
            Instruction::PrepareMethodCall {
                lookup: 1,
                arity: 0,
                after_arguments: false,
            },
            Instruction::LoadLocal(0),
            Instruction::Call {
                arity: 1,
                method: true,
            },
            Instruction::CheckType(TypeCheck::String),
            Instruction::Return,
        ]);
        let typed_string_block = blocks.len();
        blocks.push(vec![
            Instruction::LoadLocal(0),
            Instruction::GetMethod("toString".into()),
            Instruction::PrepareMethodCall {
                lookup: 1,
                arity: 0,
                after_arguments: false,
            },
            Instruction::Call {
                arity: 0,
                method: true,
            },
            Instruction::Return,
        ]);
        let mut metadata = vec![BlockMetadata::default(); blocks.len()];
        let source = crate::stdlib_sources::reference();
        let expression = "getDomain().renderReference(this)";
        let start = source
            .text
            .find(expression)
            .expect("pinned reference render expression");
        let span = Span::new(start, start + expression.len());
        let return_start = source
            .text
            .find("function toString(): String")
            .expect("reference method")
            + "function toString(): ".len();
        metadata[reference_render_block] = BlockMetadata {
            source: Some(source),
            spans: vec![
                Some(span),
                Some(span),
                Some(span),
                Some(span),
                Some(span),
                Some(Span::new(return_start, return_start + 6)),
                None,
            ],
            member: "#Reference.toString".into(),
            declaration: Some(Span::new(
                return_start - "function toString(): ".len(),
                return_start + 6,
            )),
            ..BlockMetadata::default()
        };
        let access_members = reference::access_members(&mut blocks, &mut metadata);
        let mut builtin_members = annotations::builtin_members(&mut blocks, &mut metadata);
        builtin_members.extend(resources::builtin_members(&mut blocks, &mut metadata));
        let mut objects = Vec::new();
        builtin_members.extend(output::builtin_members(
            &mut blocks,
            &mut metadata,
            &mut objects,
        ));
        builtin_members.extend(classes::ordinary_members(&mut blocks, &mut metadata));
        builtin_members.extend(regex::builtin_members(&mut blocks, &mut metadata));
        builtin_members.extend(reflection::builtin_members(&mut blocks, &mut metadata));
        let string_repeat_block = strings::repeat_block(&mut blocks, &mut metadata);
        let collection_join_blocks = collections::join_blocks(&mut blocks, &mut metadata);
        let collection_transform_blocks = collection_transforms::blocks(&mut blocks, &mut metadata);
        let collection_projection_blocks =
            collection_transforms::projection_blocks(&mut blocks, &mut metadata);
        let mut conversion_blocks = collection_conversions::blocks(&mut blocks, &mut metadata);
        conversion_blocks.extend(object_lookups::blocks(&mut blocks, &mut metadata));
        let object_default_property_blocks =
            object_lookups::default_blocks(&mut blocks, &mut metadata);
        let object_default_block = blocks.len();
        blocks.push(vec![Instruction::ObjectDefault, Instruction::Return]);
        metadata.push(BlockMetadata {
            spans: vec![None; 2],
            ..Default::default()
        });
        let conversion_delegate_block = blocks.len();
        blocks.push(vec![
            Instruction::ReadConversionProperty,
            Instruction::Return,
        ]);
        metadata.push(BlockMetadata {
            spans: vec![None; 2],
            ..Default::default()
        });
        let null_constructor_block = null::constructor_block(&mut blocks, &mut metadata);
        let string_encode_block = strings::encode_block(&mut blocks, &mut metadata);
        let pair_constructor_block = pair::constructor_block(&mut blocks, &mut metadata);
        let primitive_methods = primitives::blocks(&mut blocks, &mut metadata);
        let (amended_function_block, identity_mixin_block) =
            amendment::blocks(&mut blocks, &mut metadata);
        let collection_every_blocks =
            collection_transforms::every_blocks(&mut blocks, &mut metadata);
        let mut push_warm_block = |code: Vec<Instruction>| {
            let id = blocks.len();
            let len = code.len();
            blocks.push(code);
            metadata.push(BlockMetadata {
                spans: vec![None; len],
                ..Default::default()
            });
            id
        };
        let public_warm_blocks = [
            // 0: output.text
            push_warm_block(vec![
                Instruction::Input(0),
                Instruction::GetMember("output".into()),
                Instruction::CheckModuleOutput,
                Instruction::GetMember("text".into()),
                Instruction::Return,
            ]),
            // 1: output.bytes
            push_warm_block(vec![
                Instruction::Input(0),
                Instruction::GetMember("output".into()),
                Instruction::CheckModuleOutput,
                Instruction::GetMember("bytes".into()),
                Instruction::Return,
            ]),
            // 2: load module
            push_warm_block(vec![Instruction::Input(0), Instruction::Return]),
            // 3: pkl-binary module
            push_warm_block(vec![
                Instruction::Input(0),
                Instruction::EncodePklBinary,
                Instruction::Return,
            ]),
            // 4: pkl-binary output.text
            push_warm_block(vec![
                Instruction::Input(0),
                Instruction::GetMember("output".into()),
                Instruction::CheckModuleOutput,
                Instruction::GetMember("text".into()),
                Instruction::EncodePklBinary,
                Instruction::Return,
            ]),
            // 5: pkl-binary output.value
            push_warm_block(vec![
                Instruction::Input(0),
                Instruction::GetMember("output".into()),
                Instruction::CheckModuleOutput,
                Instruction::GetMember("value".into()),
                Instruction::EncodePklBinary,
                Instruction::Return,
            ]),
            // 6: pkl-binary output.bytes
            push_warm_block(vec![
                Instruction::Input(0),
                Instruction::GetMember("output".into()),
                Instruction::CheckModuleOutput,
                Instruction::GetMember("bytes".into()),
                Instruction::EncodePklBinary,
                Instruction::Return,
            ]),
            // 7: encode already-pushed query value
            push_warm_block(vec![Instruction::EncodePklBinary, Instruction::Return]),
        ];
        let builtin_end = blocks.len();
        Self {
            units: CodeStorage::default(),
            declaration_sources: BTreeMap::new(),
            base_declaration_sources: None,
            extra_declaration_sources: Vec::new(),
            reflection_annotations: BTreeMap::new(),
            base_reflection_annotations: None,
            next_declaration: 0,
            type_recipes: CodeStorage::default(),
            alias_definitions: BTreeMap::new(),
            base_alias_definitions: None,
            module_name: PklString::default(),
            module_template: None,
            initial_module_members: Vec::new(),
            metadata: metadata.into(),
            blocks: blocks.into(),
            objects: CodeStorage::default(),
            builtin_objects: Arc::new(objects),
            builtin_block_range: builtin_start..builtin_end,
            entry: 0,
            reference_render_block,
            typed_string_block,
            string_repeat_block,
            collection_join_blocks,
            collection_every_blocks,
            collection_transform_blocks,
            collection_projection_blocks,
            conversion_blocks: Arc::new(conversion_blocks),
            conversion_delegate_block,
            object_default_block,
            object_default_property_blocks,
            null_constructor_block,
            string_encode_block,
            pair_constructor_block,
            list_fold_block: *primitive_methods
                .get(&(Arc::from("List"), Arc::from("fold")))
                .expect("List.fold primitive block"),
            primitive_methods: Arc::new(primitive_methods),
            amended_function_block,
            identity_mixin_block,
            public_warm_blocks,
            base_mixin_declaration: None,
            access_members: Arc::new(access_members),
            builtin_members: Arc::new(builtin_members),
            constraint_origins: BTreeMap::new(),
        }
    }

    fn seal_code_segment(&mut self) {
        self.blocks.seal();
        self.metadata.seal();
        self.objects.seal();
        self.units.seal();
        self.type_recipes.seal();
    }

    fn align_metadata(&mut self) -> Result<(), RuntimeError> {
        let missing = self.metadata.next_id() < self.blocks.next_id();
        while self.metadata.next_id() < self.blocks.next_id() {
            self.metadata.try_push(BlockMetadata::default())?;
        }
        if self.metadata.next_id() != self.blocks.next_id() {
            return Err(invalid("misaligned executable metadata"));
        }
        if missing {
            // Missing raw-adapter provenance precedes the current compiler
            // transaction and must not share its retirement segment.
            self.metadata.seal();
        }
        Ok(())
    }

    pub(crate) fn alloc_block(
        &mut self,
        code: Vec<Instruction>,
        metadata: BlockMetadata,
    ) -> Result<usize, RuntimeError> {
        self.align_metadata()?;
        self.blocks.check_allocation()?;
        self.metadata.check_allocation()?;
        let id = self.blocks.try_push(code)?;
        let metadata_id = self.metadata.try_push(metadata)?;
        debug_assert_eq!(id, metadata_id);
        Ok(id)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadRequest {
    pub id: u64,
    /// Authored operand, before a Host profile's URI parsing and resolution.
    pub uri: PklString,
    /// The read declaration's original ModuleKey URI when associated with a
    /// registered unit; otherwise its explicit source URI. This is independent
    /// of the caller and is never replaced by the canonical resolved key.
    pub module_uri: Option<PklString>,
    /// Prepared URI from a successful declaration-ModuleKey resolution.
    pub resolved_uri: Option<PklString>,
    /// Only an absent resource becomes null; parse, policy and reader errors propagate.
    pub nullable: bool,
}

impl ReadRequest {
    /// Resolved URI delivered to the reader. URI semantics remain in the engine.
    pub fn resolved_uri(&self) -> Result<PklString, RuntimeError> {
        crate::host::resources::resolved_uri(self)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum VmPoll {
    Ready(Result<Value, RuntimeError>),
    Pending(ReadRequest),
    Log(LogRequest),
    TransformFrame(FrameTransformRequest),
    Module(crate::modules::ModuleRequest),
    Glob(crate::glob::GlobRequest),
    Yielded,
}

#[derive(Clone)]
struct Force {
    entry: bool,
    /// The actual argument of the member call, not its declaration key.
    entry_probe: Option<Value>,
    publishing_cache: bool,
    stolen_cache_value: bool,
    stolen_cast: Option<Box<mapping_ops::CastFailure>>,
    receiver: ObjectId,
    owner: ObjectId,
    name: PklString,
    cache: Option<CacheKey>,
    /// A nonlocal const read acquires and checks its value at the prototype owner.
    /// Only its successful result is copied into the original receiver cache.
    const_receiver: Option<Box<(ObjectId, CacheKey)>>,
    method: bool,
    inherited_types: Vec<(TypeCheck, Span, PklString, ObjectId)>,
    collection_casts: Vec<crate::heap::CollectionCast>,
    skip_typecheck: bool,
}

struct Frame {
    guest_catch: Option<guest_exceptions::GuestCatch>,
    block: usize,
    pc: usize,
    base: usize,
    environment: Environment,
    force: Option<Box<Force>>,
    lambda: bool,
    type_run: Option<TypeRun>,
    type_resolution: Option<type_resolution::DeclaredRun>,
    generic_default: Option<generics::GenericDefaultRun>,
    equality_run: Option<equality::EqualityRun>,
    deep_force: Option<forcing::ForceRun>,
    warning: Option<deprecation::PendingWarning>,
    method_call_site: Option<Box<deprecation::CallSite>>,
    pcf_render: Option<rendering::RenderRun>,
    json_parse: Option<Box<json_parser::Run>>,
    yaml_parse: Option<Box<yaml_parser::Run>>,
    collection_join: Option<collections::JoinRun>,
    collection_transform: Option<Box<collection_transforms::TransformRun>>,
    set_order: Option<Box<set_order::Run>>,
    set_sequence: Option<Box<set_sequence::Run>>,
    list_ops: Option<Box<list_ops::Run>>,
    string_transform: Option<Box<string_transforms::Run>>,
    int_seq_run: Option<Box<int_seq::Run>>,
    lazy_collection_run: Option<Box<lazy_collections::Run>>,
    mapping_lookup: Option<Box<mapping_ops::LookupRun>>,
    mapping_keys: Option<Box<mapping_ops::KeysRun>>,
    reflection: Option<Box<reflection::Run>>,
    analysis: Option<Box<analyze::Run>>,
    conversion_run: Option<Box<collection_conversions::Run>>,
    collection_addition: Option<Box<collection_addition::Run>>,
    function_owner: Option<ObjectId>,
    amendment_arguments: Option<Vec<Value>>,
    amendment_end: Option<usize>,
    prepared_hashes: Option<Vec<Value>>,
    key_search: Option<equality::KeySearch>,
    return_to_type: bool,
    diagnostic_member: Option<PklString>,
    callsite_hidden: bool,
    fast_call_frame: bool,
    captured_locals_len: usize,
    builders: Vec<ObjectId>,
    generator_scopes: Vec<identity::GeneratorScope>,
    module_load: Option<modules::ModuleRun>,
    uri_resolution: Option<uri_resolution::Resolution>,
    glob: Option<Box<glob::Run>>,
    module_receiver: Option<ObjectId>,
}

impl Frame {
    fn new_empty(block: usize, pc: usize, base: usize, environment: Environment) -> Box<Self> {
        Box::new(Self {
            guest_catch: None,
            block,
            pc,
            base,
            environment,
            force: None,
            lambda: false,
            type_run: None,
            type_resolution: None,
            generic_default: None,
            equality_run: None,
            deep_force: None,
            warning: None,
            method_call_site: None,
            pcf_render: None,
            json_parse: None,
            yaml_parse: None,
            collection_join: None,
            collection_transform: None,
            set_order: None,
            set_sequence: None,
            list_ops: None,
            string_transform: None,
            int_seq_run: None,
            lazy_collection_run: None,
            mapping_lookup: None,
            mapping_keys: None,
            reflection: None,
            analysis: None,
            conversion_run: None,
            collection_addition: None,
            function_owner: None,
            amendment_arguments: None,
            amendment_end: None,
            prepared_hashes: None,
            key_search: None,
            return_to_type: false,
            diagnostic_member: None,
            callsite_hidden: false,
            fast_call_frame: false,
            captured_locals_len: 0,
            builders: Vec::new(),
            generator_scopes: Vec::new(),
            module_load: None,
            uri_resolution: None,
            glob: None,
            module_receiver: None,
        })
    }

    #[inline]
    fn has_pending_run(&self) -> bool {
        self.glob.is_some()
            || self.guest_catch.is_some()
            || self.type_resolution.is_some()
            || self.module_load.is_some()
            || self.generic_default.is_some()
            || self.equality_run.is_some()
            || self.type_run.is_some()
            || self.reflection.is_some()
            || self.analysis.is_some()
            || self.deep_force.is_some()
            || self.yaml_parse.is_some()
            || self.json_parse.is_some()
            || self.pcf_render.is_some()
            || self.collection_join.is_some()
            || self
                .force
                .as_ref()
                .is_some_and(|force| force.stolen_cast.is_some())
    }

    #[inline]
    fn is_fast_returnable(&self) -> bool {
        self.force.is_none()
            && !self.return_to_type
            && self.amendment_arguments.is_none()
            && self.builders.is_empty()
            && self.generator_scopes.is_empty()
            && self.warning.is_none()
            && self.method_call_site.is_none()
            && self.diagnostic_member.is_none()
            && self.uri_resolution.is_none()
            && self.module_receiver.is_none()
            && self.key_search.is_none()
            && self.collection_transform.is_none()
            && self.set_order.is_none()
            && self.set_sequence.is_none()
            && self.list_ops.is_none()
            && self.string_transform.is_none()
            && self.int_seq_run.is_none()
            && self.lazy_collection_run.is_none()
            && self.mapping_lookup.is_none()
            && self.mapping_keys.is_none()
            && self.conversion_run.is_none()
            && self.collection_addition.is_none()
    }

    #[inline]
    fn fast_reset_for_pool(&mut self) {
        self.environment.locals.clear();
        self.environment.local_owners.clear();
        self.environment.copy_locals.clear();
        self.environment.restore_for_bindings = false;
        self.environment.this_value = None;
        self.environment.outer_value = None;
        self.environment.custom_this = None;
        self.environment.executable_owner = None;
        self.environment.code_owner = None;
        if !self.environment.type_arguments.is_empty() {
            self.environment.type_arguments.clear();
        }
        if !self.environment.type_argument_sources.is_empty() {
            self.environment.type_argument_sources.clear();
        }
        self.environment.scopes = crate::heap::ScopeList::default();
        self.lambda = false;
        self.function_owner = None;
        self.amendment_end = None;
        self.callsite_hidden = false;
        self.fast_call_frame = false;
        self.captured_locals_len = 0;
        debug_assert!(
            !self.has_pending_run() && self.is_fast_returnable() && self.prepared_hashes.is_none()
        );
    }

    fn reset_for_pool(&mut self) {
        self.guest_catch = None;
        self.block = 0;
        self.pc = 0;
        self.base = 0;
        self.environment.locals.clear();
        self.environment.local_owners.clear();
        self.environment.copy_locals.clear();
        self.environment.restore_for_bindings = false;
        self.environment.this_value = None;
        self.environment.outer_value = None;
        self.environment.custom_this = None;
        self.environment.executable_owner = None;
        self.environment.code_owner = None;
        self.environment.type_arguments.clear();
        self.environment.type_argument_sources.clear();
        self.environment.scopes = crate::heap::ScopeList::default();
        self.force = None;
        self.lambda = false;
        self.type_run = None;
        self.type_resolution = None;
        self.generic_default = None;
        self.equality_run = None;
        self.deep_force = None;
        self.warning = None;
        self.method_call_site = None;
        self.pcf_render = None;
        self.json_parse = None;
        self.yaml_parse = None;
        self.collection_join = None;
        self.collection_transform = None;
        self.set_order = None;
        self.set_sequence = None;
        self.list_ops = None;
        self.string_transform = None;
        self.int_seq_run = None;
        self.lazy_collection_run = None;
        self.mapping_lookup = None;
        self.mapping_keys = None;
        self.reflection = None;
        self.analysis = None;
        self.conversion_run = None;
        self.collection_addition = None;
        self.function_owner = None;
        self.amendment_arguments = None;
        self.amendment_end = None;
        self.prepared_hashes = None;
        self.key_search = None;
        self.return_to_type = false;
        self.diagnostic_member = None;
        self.callsite_hidden = false;
        self.fast_call_frame = false;
        self.captured_locals_len = 0;
        self.builders.clear();
        self.generator_scopes.clear();
        self.module_load = None;
        self.uri_resolution = None;
        self.glob = None;
        self.module_receiver = None;
    }

    fn hides_callsite(&self) -> bool {
        self.callsite_hidden
            || self.force.as_ref().is_some_and(|force| force.stolen_cache_value)
            // Module initialization is invoked from ModuleCache without a guest
            // call node. Resolve failures still originate at the ImportNode.
            || self.module_load.as_ref().is_some_and(|run| run.initializing)
            || self
                .deep_force
                .as_ref()
                .is_some_and(forcing::ForceRun::is_trace)
    }
}

#[derive(Clone)]
pub(crate) enum PendingRequest {
    Read(ReadRequest),
    Log(LogRequest),
    TransformFrame(Box<frame_transformation::PendingTransform>),
    Module(crate::modules::ModuleRequest),
    Glob(crate::glob::GlobRequest),
}
impl PendingRequest {
    fn id(&self) -> u64 {
        match self {
            Self::Read(request) => request.id,
            Self::Log(request) => request.id,
            Self::TransformFrame(pending) => pending.request.id,
            Self::Module(request) => request.id,
            Self::Glob(request) => request.id,
        }
    }
    fn poll(&self) -> VmPoll {
        match self {
            Self::Read(request) => VmPoll::Pending(request.clone()),
            Self::Log(request) => VmPoll::Log(request.clone()),
            Self::TransformFrame(pending) => VmPoll::TransformFrame(pending.request.clone()),
            Self::Module(request) => VmPoll::Module(request.clone()),
            Self::Glob(request) => VmPoll::Glob(request.clone()),
        }
    }
}

// Keep Host wait/resume transitions allocation-free. The inline footprint also
// applies to suspended contexts; measure that tradeoff before changing layout.
#[allow(clippy::large_enum_variant)]
enum State {
    Idle,
    Running,
    Waiting(PendingRequest),
    Finished(Result<Value, RuntimeError>),
    Closed,
}

#[derive(Clone)]
struct CachedQuery {
    unit: crate::modules::UnitId,
    expression: PklString,
    raw_value: bool,
    power_assertions: bool,
    entry: usize,
    owner: ExecutableOwnerId,
}

pub(crate) enum WarmPublicOp<'a> {
    OutputText,
    OutputBytes,
    LoadModule,
    PklBinaryModule,
    PklBinaryOutputText,
    PklBinaryOutputValue,
    PklBinaryOutputBytes,
    Query {
        expression: &'a crate::source::SourceText,
        raw_value: bool,
        encode_pkl_binary: bool,
    },
}

struct SuspendedContext {
    public_primary: Option<ObjectId>,
    /// Caller locations captured at the Host boundary, innermost first. The
    /// pending reader has no language call node for a callback evaluation.
    diagnostic_callers: Vec<RuntimeFrame>,
    frames: Vec<Box<Frame>>,
    stack: identity::OperandStack,
    inputs: Vec<Value>,
    state: State,
}

#[inline]
fn block_hash_slot(block: usize, shift: u32) -> usize {
    #[cfg(target_pointer_width = "64")]
    const MULT: usize = 0x9E37_79B9_7F4A_7C15;
    #[cfg(not(target_pointer_width = "64"))]
    const MULT: usize = 0x9E37_79B9;
    block.wrapping_mul(MULT) >> shift
}

/// Block-ID-indexed table of static executable owners (`O(1)` lookup on hot paths),
/// with a copy-on-write frozen dense `pkl:base` prefix shared across fresh evaluators
/// and a compact Fibonacci-hashed open-addressed local table whose memory stays `O(live)`
/// even when dynamic block IDs advance monotonically across long-lived sessions.
#[derive(Clone, Default)]
pub(super) struct ExecutableRoots {
    base: Option<Arc<[Option<ExecutableOwnerId>]>>,
    base_len: usize,
    base_count: usize,
    base_overrides: Vec<(usize, ExecutableOwnerId)>,
    local_slots: Vec<Option<(usize, ExecutableOwnerId)>>,
    local_count: usize,
}

impl ExecutableRoots {
    #[inline]
    pub(super) fn new() -> Self {
        Self::default()
    }
    pub(super) fn freeze_base(&mut self) {
        if self.base.is_none() && self.local_count > 0 {
            let max_block = self
                .local_slots
                .iter()
                .flatten()
                .map(|(block, _)| *block)
                .max()
                .unwrap_or(0);
            let mut dense = vec![None; max_block + 1];
            for &(block, owner) in self.local_slots.iter().flatten() {
                dense[block] = Some(owner);
            }
            self.base_len = dense.len();
            self.base_count = self.local_count;
            self.base = Some(Arc::from(dense));
            self.base_overrides.clear();
            self.local_slots = Vec::new();
            self.local_count = 0;
        }
    }
    #[inline]
    pub(super) fn get(&self, block: &usize) -> Option<&ExecutableOwnerId> {
        let block = *block;
        if block < self.base_len {
            if let Some(owner) = self.base.as_deref()?.get(block).and_then(Option::as_ref) {
                return Some(owner);
            }
            for (b, owner) in &self.base_overrides {
                if *b == block {
                    return Some(owner);
                }
            }
            None
        } else {
            let cap = self.local_slots.len();
            if cap == 0 {
                return None;
            }
            let mask = cap - 1;
            let shift = usize::BITS - cap.trailing_zeros();
            let mut idx = block_hash_slot(block, shift);
            loop {
                match &self.local_slots[idx] {
                    Some((k, owner)) if *k == block => return Some(owner),
                    None => return None,
                    Some(_) => idx = (idx + 1) & mask,
                }
            }
        }
    }
    fn rehash_local(&mut self, new_cap: usize) {
        if new_cap == 0 {
            self.local_slots = Vec::new();
            self.local_count = 0;
            return;
        }
        let cap = new_cap.next_power_of_two().max(16);
        let old = std::mem::replace(&mut self.local_slots, vec![None; cap]);
        self.local_count = 0;
        let mask = cap - 1;
        let shift = usize::BITS - cap.trailing_zeros();
        for (block, owner) in old.into_iter().flatten() {
            let mut idx = block_hash_slot(block, shift);
            loop {
                if self.local_slots[idx].is_none() {
                    self.local_slots[idx] = Some((block, owner));
                    self.local_count += 1;
                    break;
                }
                idx = (idx + 1) & mask;
            }
        }
    }
    fn insert_local(&mut self, block: usize, owner: ExecutableOwnerId) {
        if (self.local_count + 1).saturating_mul(2) > self.local_slots.len() {
            let new_cap = self.local_slots.len().saturating_mul(2).max(16);
            self.rehash_local(new_cap);
        }
        let cap = self.local_slots.len();
        let mask = cap - 1;
        let shift = usize::BITS - cap.trailing_zeros();
        let mut idx = block_hash_slot(block, shift);
        loop {
            match &mut self.local_slots[idx] {
                Some((k, existing)) if *k == block => {
                    *existing = owner;
                    return;
                }
                None => {
                    self.local_slots[idx] = Some((block, owner));
                    self.local_count += 1;
                    return;
                }
                Some(_) => idx = (idx + 1) & mask,
            }
        }
    }
    fn unfreeze_base(&mut self) {
        let base = self.base.take();
        let overrides = std::mem::take(&mut self.base_overrides);
        let total_needed = self.base_count + overrides.len() + self.local_count;
        self.base_len = 0;
        self.base_count = 0;
        if total_needed.saturating_mul(2) > self.local_slots.len() {
            self.rehash_local(total_needed.saturating_mul(2).max(16));
        }
        if let Some(base) = base {
            for (block, slot) in base.iter().enumerate() {
                if let Some(owner) = *slot {
                    self.insert_local(block, owner);
                }
            }
        }
        for (block, owner) in overrides {
            self.insert_local(block, owner);
        }
    }
    #[inline]
    pub(super) fn insert(&mut self, block: usize, owner: ExecutableOwnerId) {
        if block < self.base_len {
            match self
                .base
                .as_deref()
                .and_then(|b| b.get(block))
                .copied()
                .flatten()
            {
                None => {
                    for (b, existing) in &mut self.base_overrides {
                        if *b == block {
                            *existing = owner;
                            return;
                        }
                    }
                    self.base_overrides.push((block, owner));
                    return;
                }
                Some(existing) if existing == owner => return,
                Some(_) => self.unfreeze_base(),
            }
        }
        self.insert_local(block, owner);
    }
    pub(super) fn compact(&mut self) {
        if self.local_count == 0 {
            if !self.local_slots.is_empty() {
                self.local_slots = Vec::new();
            }
        } else if self.local_slots.len() > 64
            && self.local_count.saturating_mul(4) < self.local_slots.len()
        {
            let target = self
                .local_count
                .saturating_mul(2)
                .next_power_of_two()
                .max(16);
            self.rehash_local(target);
        }
    }
    #[inline]
    pub(super) fn clear(&mut self) {
        self.base = None;
        self.base_len = 0;
        self.base_count = 0;
        self.base_overrides = Vec::new();
        self.local_slots = Vec::new();
        self.local_count = 0;
    }
    #[inline]
    pub(super) fn len(&self) -> usize {
        self.base_count + self.base_overrides.len() + self.local_count
    }
    #[inline]
    pub(super) fn is_empty(&self) -> bool {
        self.len() == 0
    }
    #[cfg(test)]
    pub(super) fn local_capacity(&self) -> usize {
        self.local_slots.capacity()
    }
    pub(super) fn retain(&mut self, mut f: impl FnMut(&usize, &mut ExecutableOwnerId) -> bool) {
        if let Some(base) = self.base.as_deref() {
            let mut needs_unfreeze = false;
            for (block, slot) in base.iter().enumerate() {
                if let Some(mut copy) = *slot
                    && (!f(&block, &mut copy) || copy != slot.unwrap())
                {
                    needs_unfreeze = true;
                    break;
                }
            }
            if needs_unfreeze {
                self.unfreeze_base();
            }
        }
        if !self.base_overrides.is_empty() {
            self.base_overrides
                .retain_mut(|(block, owner)| f(block, owner));
        }
        if self.local_count > 0 {
            let mut removed = false;
            let mut kept = 0usize;
            for slot in &mut self.local_slots {
                if let Some((block, owner)) = slot {
                    if f(block, owner) {
                        kept += 1;
                    } else {
                        *slot = None;
                        removed = true;
                    }
                }
            }
            if removed {
                if kept == 0 {
                    self.rehash_local(0);
                } else {
                    let target = kept.saturating_mul(2).next_power_of_two().max(16);
                    self.rehash_local(target);
                }
            }
        }
        self.compact();
    }
    pub(super) fn iter(&self) -> impl Iterator<Item = (usize, ExecutableOwnerId)> + '_ {
        let base_slice: &[Option<ExecutableOwnerId>] = self.base.as_deref().unwrap_or(&[]);
        base_slice
            .iter()
            .enumerate()
            .filter_map(|(block, slot)| slot.map(|owner| (block, owner)))
            .chain(self.base_overrides.iter().copied())
            .chain(self.local_slots.iter().flatten().copied())
    }
}

/// Compact Fibonacci-hashed open-addressed cache of compiled `FastIntBlock` entries keyed
/// by bytecode block ID. Memory stays `O(live)` regardless of monotonic block IDs.
#[derive(Default)]
struct FastIntCache {
    slots: Vec<Option<(usize, Option<Box<fast_int::FastIntBlock>>)>>,
    count: usize,
}

impl FastIntCache {
    #[inline]
    fn is_empty(&self) -> bool {
        self.count == 0
    }
    #[cfg(test)]
    fn capacity(&self) -> usize {
        self.slots.capacity()
    }
    #[inline]
    fn clear(&mut self) {
        self.slots = Vec::new();
        self.count = 0;
    }
    #[inline]
    fn get_or_compile(
        &mut self,
        block_id: usize,
        builtin_end: usize,
        compile: impl FnOnce() -> Option<Box<fast_int::FastIntBlock>>,
    ) -> Option<&fast_int::FastIntBlock> {
        if block_id < builtin_end {
            return None;
        }
        let cap = self.slots.len();
        if cap > 0 {
            let mask = cap - 1;
            let shift = usize::BITS - cap.trailing_zeros();
            let mut idx = block_hash_slot(block_id, shift);
            let mut hit = None;
            loop {
                match self.slots[idx] {
                    Some((k, _)) if k == block_id => {
                        hit = Some(idx);
                        break;
                    }
                    None => break,
                    Some(_) => idx = (idx + 1) & mask,
                }
            }
            if let Some(found) = hit {
                return self.slots[found].as_ref().and_then(|(_, v)| v.as_deref());
            }
        }
        self.insert_slow(block_id, compile())
    }
    #[cold]
    fn insert_slow(
        &mut self,
        block_id: usize,
        compiled: Option<Box<fast_int::FastIntBlock>>,
    ) -> Option<&fast_int::FastIntBlock> {
        if (self.count + 1).saturating_mul(2) > self.slots.len() {
            let new_cap = self.slots.len().saturating_mul(2).max(16);
            self.rehash(new_cap);
        }
        let cap = self.slots.len();
        let mask = cap - 1;
        let shift = usize::BITS - cap.trailing_zeros();
        let mut idx = block_hash_slot(block_id, shift);
        loop {
            match &mut self.slots[idx] {
                Some((k, existing)) if *k == block_id => {
                    *existing = compiled;
                    break;
                }
                None => {
                    self.slots[idx] = Some((block_id, compiled));
                    self.count += 1;
                    break;
                }
                Some(_) => idx = (idx + 1) & mask,
            }
        }
        self.slots[idx].as_ref().and_then(|(_, v)| v.as_deref())
    }
    fn rehash(&mut self, new_cap: usize) {
        if new_cap == 0 {
            self.slots = Vec::new();
            self.count = 0;
            return;
        }
        let cap = new_cap.next_power_of_two().max(16);
        let old = std::mem::replace(&mut self.slots, vec![None; cap]);
        self.count = 0;
        let mask = cap - 1;
        let shift = usize::BITS - cap.trailing_zeros();
        for (block_id, entry) in old.into_iter().flatten() {
            let mut idx = block_hash_slot(block_id, shift);
            loop {
                if self.slots[idx].is_none() {
                    self.slots[idx] = Some((block_id, entry));
                    self.count += 1;
                    break;
                }
                idx = (idx + 1) & mask;
            }
        }
    }
    fn retain(&mut self, mut keep: impl FnMut(usize) -> bool) {
        if self.count == 0 {
            if !self.slots.is_empty() {
                self.slots = Vec::new();
            }
            return;
        }
        let mut removed = false;
        let mut kept = 0usize;
        for slot in &mut self.slots {
            if let Some((block_id, _)) = slot {
                if keep(*block_id) {
                    kept += 1;
                } else {
                    *slot = None;
                    removed = true;
                }
            }
        }
        if removed {
            if kept == 0 {
                self.rehash(0);
            } else {
                let target = kept.saturating_mul(2).next_power_of_two().max(16);
                self.rehash(target);
            }
        } else if self.slots.len() > 64 && self.count.saturating_mul(4) < self.slots.len() {
            let target = self.count.saturating_mul(2).next_power_of_two().max(16);
            self.rehash(target);
        }
    }
}

/// Static call targets own code state, never captured receivers/environments.
pub struct Vm {
    program: Arc<Program>,
    context: EvaluationContext,
    callback_closing: bool,
    log_buffer: Vec<u16>,
    heap: Heap,
    frames: Vec<Box<Frame>>,
    frame_pool: Vec<Box<Frame>>,
    stack: identity::OperandStack,
    inputs: Vec<Value>,
    state: State,
    suspended: Vec<SuspendedContext>,
    next_request: u64,
    instructions: u64,
    member_evaluations: u64,
    code_validation: Result<(), RuntimeError>,
    power_assertions: bool,
    pure_standard_host: bool,
    inline_module_depth: u8,
    active_type_trackers: usize,
    /// Keys belong to this immutable Program; values belong only to this evaluator.
    /// Reentering or restarting a declaration retains its first captured environment.
    public_primary: Option<ObjectId>,
    modules_original: modules::ModuleCache,
    modules_stdlib: modules::ModuleCache,
    modules_resolved: modules::ModuleCache,
    resources: BTreeMap<crate::host::resources::CacheKey, Option<Value>>,
    glob_list_threshold: u32,
    class_singletons: BTreeMap<usize, ObjectId>,
    builtin_classes: BTreeMap<classes::BuiltinClass, ObjectId>,
    alias_singletons: BTreeMap<usize, ObjectId>,
    /// Weak handles: live empty strict values share Java's EMPTY identity, but
    /// this lookup does not keep discarded values or producer scopes alive.
    empty_collections: [Option<ObjectId>; 3],
    empty_objects: [Option<ObjectId>; 3],
    resolved_declarations: BTreeMap<(usize, ObjectId), type_resolution::DeclarationValue>,
    type_parent_defaults: BTreeMap<(usize, usize), Value>,
    /// Static call targets own code state, never captured receivers/environments.
    executable_roots: ExecutableRoots,
    code_arenas: Vec<query_lifecycle::CodeArena>,
    declaration_roots_dirty: bool,
    declaration_roots_lens: (usize, usize, usize, usize, usize, usize),
    unowned_declaration_roots: Vec<Value>,
    public_operation_entries: BTreeSet<usize>,
    query_cache: Vec<CachedQuery>,
    auto_gc: bool,
    gc_threshold: usize,
    last_gc_arenas: usize,
    gc_epoch: u64,
    session_root: Option<Value>,
    uncached_module_uris: std::collections::VecDeque<(PklString, PklString)>,
    cached_module_uris: BTreeSet<PklString>,
    /// 1-entry MRU cache for warm persistent public operations (bypasses BTreeMap URI compares).
    last_warm_module: Option<(PklString, ObjectId)>,
    fast_int_cache: FastIntCache,
    fast_int_captured: Vec<i64>,
    fast_int_vals: Vec<i64>,
    fast_int_locals: Vec<i64>,
    fast_int_calls: Vec<(u32, u32)>,
    scratch_utf8: String,
    pub(crate) platform_metadata: Option<crate::logging::PlatformMetadata>,
    pub(crate) monotonic_clock: Option<crate::logging::MonotonicClock>,
}

impl Vm {
    pub fn new(program: Arc<Program>) -> Self {
        Self::with_context(program, EvaluationContext::default())
    }

    pub fn with_context(mut program: Arc<Program>, context: EvaluationContext) -> Self {
        let registration = if let Some(source) = &context.primary_source {
            if program
                .units
                .get(0)
                .is_some_and(|unit| unit.identity.is_none())
            {
                Arc::make_mut(&mut program).register_primary_source(
                    source.clone(),
                    crate::modules::ModuleIdentity {
                        original_uri: source.uri.clone(),
                        resolved_uri: source.uri.clone(),
                    },
                )
            } else {
                Ok(())
            }
        } else {
            Ok(())
        };
        // Compiled caches cannot retain evaluator-owned handles. Validate once,
        // independently of repeated evaluation state creation. Builtin blocks
        // emitted by Program::new contain no heap object constants.
        let code_validation = if program
            .blocks
            .iter_with_ids()
            .filter(|(id, _)| !program.builtin_block_range.contains(id))
            .flat_map(|(_, block)| block.iter())
            .any(|instruction| matches!(instruction, Instruction::Constant(value) if value.heap_id().is_some()))
        {
            Err(invalid(
                "object constants cannot be stored in compiled code",
            ))
        } else {
            Ok(())
        };
        let platform_metadata = context.platform_metadata.clone();
        Self {
            program,
            context,
            callback_closing: false,
            log_buffer: Vec::new(),
            heap: Heap::new(),
            frames: Vec::new(),
            frame_pool: Vec::new(),
            stack: identity::OperandStack::default(),
            inputs: Vec::new(),
            state: State::Idle,
            suspended: Vec::new(),
            next_request: 1,
            instructions: 0,
            member_evaluations: 0,
            code_validation: registration.and(code_validation),
            power_assertions: false,
            pure_standard_host: false,
            inline_module_depth: 0,
            active_type_trackers: 0,
            public_primary: None,
            modules_original: modules::ModuleCache::default(),
            modules_stdlib: modules::ModuleCache::default(),
            modules_resolved: modules::ModuleCache::default(),
            resources: BTreeMap::new(),
            glob_list_threshold: 16384,
            class_singletons: BTreeMap::new(),
            builtin_classes: BTreeMap::new(),
            alias_singletons: BTreeMap::new(),
            empty_collections: [None; 3],
            empty_objects: [None; 3],
            resolved_declarations: BTreeMap::new(),
            type_parent_defaults: BTreeMap::new(),
            executable_roots: ExecutableRoots::new(),
            code_arenas: Vec::new(),
            declaration_roots_dirty: true,
            declaration_roots_lens: (0, 0, 0, 0, 0, 0),
            unowned_declaration_roots: Vec::new(),
            public_operation_entries: BTreeSet::new(),
            query_cache: Vec::new(),
            auto_gc: false,
            gc_threshold: 8192,
            last_gc_arenas: 0,
            gc_epoch: 1,
            session_root: None,
            uncached_module_uris: std::collections::VecDeque::new(),
            cached_module_uris: BTreeSet::new(),
            last_warm_module: None,
            fast_int_cache: FastIntCache::default(),
            fast_int_captured: Vec::new(),
            fast_int_vals: Vec::new(),
            fast_int_locals: Vec::new(),
            fast_int_calls: Vec::new(),
            scratch_utf8: String::with_capacity(256),
            platform_metadata,
            monotonic_clock: None,
        }
    }

    pub fn set_platform_metadata(&mut self, metadata: Option<crate::logging::PlatformMetadata>) {
        self.platform_metadata = metadata;
    }

    pub fn set_monotonic_clock(&mut self, clock: crate::logging::MonotonicClock) {
        self.monotonic_clock = Some(clock);
    }

    pub fn set_power_assertions(&mut self, enabled: bool) {
        self.power_assertions = enabled;
    }

    pub(crate) fn set_pure_standard_host(&mut self, enabled: bool) {
        self.pure_standard_host = enabled;
    }

    #[inline]
    pub(crate) fn public_primary_id(&self) -> Option<ObjectId> {
        self.public_primary
    }

    fn available(&self) -> Result<(), RuntimeError> {
        match self.state {
            State::Closed => Err(RuntimeError::new(ErrorKind::Closed, "evaluator is closed")),
            State::Running | State::Waiting(_) => Err(RuntimeError::new(
                ErrorKind::Busy,
                "evaluation is already active",
            )),
            _ => Ok(()),
        }
    }

    pub fn start(&mut self, entry: usize, inputs: Vec<Value>) -> Result<(), RuntimeError> {
        self.start_entry(entry, inputs, true)
    }

    /// Continue an already admitted evaluation after its preceding entry finished.
    /// This is not a new evaluator invocation, so its buffered logs survive.
    pub(crate) fn continue_entry(
        &mut self,
        entry: usize,
        inputs: Vec<Value>,
    ) -> Result<(), RuntimeError> {
        if !matches!(self.state, State::Finished(_)) {
            return Err(invalid("continuation requires a completed entry"));
        }
        let result = self.start_entry(entry, inputs, false);
        if let Err(error) = &result {
            self.state = State::Finished(Err(error.clone()));
        }
        result
    }

    #[inline]
    fn alloc_frame(
        &mut self,
        block: usize,
        pc: usize,
        base: usize,
        environment: Environment,
    ) -> Box<Frame> {
        if let Some(mut frame) = self.frame_pool.pop() {
            frame.block = block;
            frame.pc = pc;
            frame.base = base;
            frame.environment = environment;
            frame
        } else {
            Frame::new_empty(block, pc, base, environment)
        }
    }

    #[inline]
    fn recycle_frame(&mut self, mut frame: Box<Frame>) {
        if self.frame_pool.len() < 64 {
            frame.reset_for_pool();
            self.frame_pool.push(frame);
        }
    }

    fn start_entry(
        &mut self,
        entry: usize,
        inputs: Vec<Value>,
        clear_logs: bool,
    ) -> Result<(), RuntimeError> {
        self.available()?;
        self.code_validation.clone()?;
        if self.program.blocks.get(entry).is_none() {
            return Err(invalid("invalid entry block"));
        }
        for input in &inputs {
            self.heap.validate_value(input)?;
        }
        let mut environment = Environment::default();
        if let Some(index) = self
            .program
            .metadata
            .get(entry)
            .and_then(|m| m.module_receiver_input)
        {
            let receiver = inputs
                .get(index)
                .ok_or_else(|| invalid("missing module input"))?
                .as_object()?;
            environment.scopes.push(Scope {
                receiver,
                owner: receiver,
            });
        }
        if clear_logs {
            self.log_buffer.clear();
        }
        self.stack.clear();
        self.inputs = inputs;
        let frame = self.alloc_frame(entry, 0, 0, environment);
        self.frames.push(frame);
        self.state = State::Running;
        Ok(())
    }

    #[inline]
    pub(crate) fn has_warm_cached_module(&self, uri: &PklString) -> bool {
        if self.code_validation.is_err() || self.available().is_err() {
            return false;
        }
        if let Some((last_uri, _)) = &self.last_warm_module
            && last_uri == uri
        {
            return true;
        }
        self.cached_module_uris.contains(uri) && self.modules_original.get_ref(uri).is_some()
    }

    pub(crate) fn try_start_warm_cached_public_op(
        &mut self,
        uri: &PklString,
        op: WarmPublicOp<'_>,
    ) -> Result<bool, RuntimeError> {
        self.available()?;
        if let Err(err) = &self.code_validation {
            return Err(err.clone());
        }
        let receiver = if let Some((last_uri, receiver)) = &self.last_warm_module
            && last_uri == uri
        {
            *receiver
        } else {
            if !self.cached_module_uris.contains(uri) {
                return Ok(false);
            }
            let Some(cached) = self.modules_original.get_ref(uri) else {
                return Ok(false);
            };
            match cached {
                modules::CachedModule::Failed(error) => {
                    let error = error.clone();
                    self.start_failed(error)?;
                    return Ok(true);
                }
                modules::CachedModule::Value(receiver) => {
                    let receiver = *receiver;
                    self.last_warm_module = Some((uri.clone(), receiver));
                    receiver
                }
            }
        };
        match op {
            WarmPublicOp::OutputText
            | WarmPublicOp::OutputBytes
            | WarmPublicOp::LoadModule
            | WarmPublicOp::PklBinaryModule
            | WarmPublicOp::PklBinaryOutputText
            | WarmPublicOp::PklBinaryOutputValue
            | WarmPublicOp::PklBinaryOutputBytes => {
                let slot = match op {
                    WarmPublicOp::OutputText => 0,
                    WarmPublicOp::OutputBytes => 1,
                    WarmPublicOp::LoadModule => 2,
                    WarmPublicOp::PklBinaryModule => 3,
                    WarmPublicOp::PklBinaryOutputText => 4,
                    WarmPublicOp::PklBinaryOutputValue => 5,
                    WarmPublicOp::PklBinaryOutputBytes => 6,
                    WarmPublicOp::Query { .. } => unreachable!(),
                };
                let entry = self.program.public_warm_blocks[slot];
                self.public_primary = Some(receiver);
                self.start_entry(entry, vec![Value::Object(receiver)], true)?;
                Ok(true)
            }
            WarmPublicOp::Query {
                expression,
                raw_value,
                encode_pkl_binary,
            } => {
                let Some(unit) = self.heap.get(receiver)?.module_unit else {
                    return Ok(false);
                };
                let power_assertions = self.power_assertions;
                let expr_orig = expression.original();
                let Some((owner, entry)) = self.query_cache.iter().find_map(|cached| {
                    (cached.unit == unit
                        && cached.raw_value == raw_value
                        && cached.power_assertions == power_assertions
                        && cached.expression == *expr_orig
                        && self.heap.executable_is_live(cached.owner))
                    .then_some((cached.owner, cached.entry))
                }) else {
                    return Ok(false);
                };
                self.public_primary = Some(receiver);
                self.log_buffer.clear();
                self.stack.clear();
                self.inputs.clear();
                if encode_pkl_binary {
                    let encode_block = self.program.public_warm_blocks[7];
                    let wrapper = self.alloc_frame(encode_block, 0, 0, Environment::default());
                    self.frames.push(wrapper);
                }
                let mut frame = if let Some(mut pooled) = self.frame_pool.pop() {
                    pooled.block = entry;
                    pooled.pc = 0;
                    pooled.base = 0;
                    pooled
                } else {
                    Frame::new_empty(entry, 0, 0, Environment::default())
                };
                frame.environment.executable_owner = Some(owner);
                frame.environment.code_owner = Some(owner);
                frame.environment.scopes = crate::heap::ScopeList::from_single(Scope {
                    receiver,
                    owner: receiver,
                });
                self.frames.push(frame);
                self.state = State::Running;
                Ok(true)
            }
        }
    }

    pub(crate) fn start_failed(&mut self, error: RuntimeError) -> Result<(), RuntimeError> {
        self.available()?;
        self.log_buffer.clear();
        self.stack.clear();
        self.inputs.clear();
        self.frames.clear();
        self.finish_error(error);
        Ok(())
    }

    /// Enter a callback evaluation while the current context waits for Host I/O.
    /// The program, heap and successful member cache belong to the evaluator.
    pub(crate) fn validate_pending_request(&self, id: u64) -> Result<(), RuntimeError> {
        if matches!(&self.state, State::Waiting(request) if request.id() == id) {
            Ok(())
        } else {
            Err(RuntimeError::new(
                ErrorKind::Host,
                "stale or unexpected Host completion",
            ))
        }
    }
    pub(crate) fn reenter(
        &mut self,
        request: u64,
        entry: usize,
        inputs: Vec<Value>,
    ) -> Result<(), RuntimeError> {
        if !matches!(&self.state, State::Waiting(pending) if pending.id() == request) {
            return Err(RuntimeError::new(
                ErrorKind::Host,
                "stale or unexpected Host completion",
            ));
        }
        if self.suspended.len() >= 63 {
            return Err(RuntimeError::new(
                ErrorKind::Busy,
                "experiment context limit exceeded",
            ));
        }
        let diagnostic_callers = self
            .frames
            .iter()
            .enumerate()
            .rev()
            .skip(1)
            .filter_map(|(index, frame)| {
                if frame.hides_callsite()
                    || self
                        .frames
                        .get(index + 1)
                        .is_some_and(|child| self.object_method_signature_frame(child))
                {
                    return None;
                }
                let metadata = self.program.metadata.get(frame.block)?;
                let span = (*metadata.spans.get(frame.pc.saturating_sub(1))?)?;
                Some(RuntimeFrame {
                    anonymous: false,
                    source: metadata.source.clone(),
                    span,
                    member: frame
                        .diagnostic_member
                        .clone()
                        .unwrap_or_else(|| metadata.member.clone()),
                })
            })
            .collect();
        self.suspended.push(SuspendedContext {
            public_primary: self.public_primary.take(),
            diagnostic_callers,
            frames: std::mem::take(&mut self.frames),
            stack: std::mem::take(&mut self.stack),
            inputs: std::mem::take(&mut self.inputs),
            state: std::mem::replace(&mut self.state, State::Idle),
        });
        if let Err(error) = self.start(entry, inputs) {
            self.restore_context();
            return Err(error);
        }
        self.active_type_trackers = 0;
        Ok(())
    }

    /// Called only after the Host has captured the completed nested result.
    pub(crate) fn finish_reentry(&mut self) {
        if matches!(self.state, State::Finished(_)) {
            self.restore_context();
        }
    }

    fn recompute_active_type_trackers(&mut self) {
        self.active_type_trackers = self
            .frames
            .iter()
            .filter(|frame| frame.type_run.as_ref().is_some_and(TypeRun::has_tracker))
            .count();
    }

    fn restore_context(&mut self) {
        if let Some(context) = self.suspended.pop() {
            self.public_primary = context.public_primary;
            self.frames = context.frames;
            self.stack = context.stack;
            self.inputs = context.inputs;
            self.state = context.state;
            self.recompute_active_type_trackers();
        }
    }

    pub(crate) fn pending_requests(&self) -> Vec<Option<PendingRequest>> {
        self.suspended
            .iter()
            .map(|context| &context.state)
            .chain(std::iter::once(&self.state))
            .map(|state| match state {
                State::Waiting(request) => Some(request.clone()),
                _ => None,
            })
            .collect()
    }

    pub fn start_member(
        &mut self,
        receiver: ObjectId,
        name: impl Into<PklString>,
    ) -> Result<(), RuntimeError> {
        let name = name.into();
        self.available()?;
        self.heap.get(receiver)?;
        self.log_buffer.clear();
        self.inputs.clear();
        self.stack.clear();
        self.state = State::Running;
        if matches!(
            self.heap.get(receiver)?.collection_kind,
            crate::heap::CollectionKind::Map | crate::heap::CollectionKind::Set
        ) && self.collection_builtin(receiver, &name)?
        {
            self.finish_top()?;
            return Ok(());
        }
        if let Err(error) = self.force(receiver, receiver, name, false, true, false) {
            self.fail(error);
        } else if self.frames.is_empty() {
            self.finish_top()?;
        }
        Ok(())
    }

    pub fn poll(&mut self, budget: usize) -> VmPoll {
        let mut remaining = budget;
        while remaining > 0 {
            match &self.state {
                State::Waiting(request) => return request.poll(),
                State::Finished(result) => return VmPoll::Ready(result.clone()),
                State::Closed => {
                    return VmPoll::Ready(Err(RuntimeError::new(
                        ErrorKind::Closed,
                        "evaluator is closed",
                    )));
                }
                State::Idle => {
                    return VmPoll::Ready(Err(invalid("evaluation has not been started")));
                }
                State::Running => {}
            }
            if self.auto_gc && self.heap.allocations_since_gc() >= self.gc_threshold {
                let stats = self.collect(&[]);
                self.gc_threshold = stats.live.saturating_mul(2).max(8192);
                self.last_gc_arenas = self.code_arenas.len();
            }
            let fast = self.poll_fast_run(remaining);
            if fast > 0 {
                remaining = remaining.saturating_sub(fast);
                continue;
            }
            self.instructions += 1;
            remaining -= 1;
            if let Err(error) = self.step() {
                self.fail(error);
            }
        }
        match &self.state {
            State::Waiting(request) => request.poll(),
            State::Finished(result) => VmPoll::Ready(result.clone()),
            State::Closed => VmPoll::Ready(Err(RuntimeError::new(
                ErrorKind::Closed,
                "evaluator is closed",
            ))),
            _ => VmPoll::Yielded,
        }
    }

    fn poll_fast_run(&mut self, max_steps: usize) -> usize {
        let Some(top) = self.frames.last() else {
            return 0;
        };
        if top.has_pending_run() || top.prepared_hashes.is_some() {
            return 0;
        }
        let mut block = top.block;
        let Some(mut code) = self.program.blocks.get(block).map(Vec::as_slice) else {
            return 0;
        };
        let mut pc = top.pc;
        let mut base = top.base;

        let program = &*self.program;
        let frames = &mut self.frames;
        let frame_pool = &mut self.frame_pool;
        let stack = &mut self.stack;
        let heap = &mut self.heap;
        let executable_roots = &self.executable_roots;
        let resolved_declarations = &self.resolved_declarations;
        let active_type_trackers = self.active_type_trackers;
        let fast_int_cache = &mut self.fast_int_cache;
        let fast_int_captured = &mut self.fast_int_captured;
        let fast_int_vals = &mut self.fast_int_vals;
        let fast_int_locals = &mut self.fast_int_locals;
        let fast_int_calls = &mut self.fast_int_calls;
        let scratch_utf8 = &mut self.scratch_utf8;
        let mut finished_top_result: Option<Value> = None;
        // Cached in-place loop shape: (callee_block, caller_block, caller_pc, end_target, is_transform_map)
        let mut inplace_loop_cache: (usize, usize, usize, usize, bool) =
            (usize::MAX, usize::MAX, usize::MAX, 0, false);

        // Per-run inline caches: nothing inside `poll_fast_run` mutates
        // `executable_roots`, `local_kind`, `obj.cache`, or `method_call_caches`.
        let mut cur_exec_block: usize = usize::MAX;
        let mut cur_exec_root: Option<ExecutableOwnerId> = None;
        let mut last_bind_ok: Option<(ExecutableOwnerId, usize, usize, identity::SlotKind)> = None;
        let mut method_cache: [(usize, usize, ObjectId, ObjectId); 2] = [(
            usize::MAX,
            usize::MAX,
            ObjectId {
                heap: 0,
                index: 0,
                generation: 0,
            },
            ObjectId {
                heap: 0,
                index: 0,
                generation: 0,
            },
        ); 2];
        let mut method_cache_next: usize = 0;
        let mut prep_method_cache: [Option<(ExecutableOwnerId, usize, usize, ObjectId)>; 4] =
            [None; 4];
        let mut prep_method_next: usize = 0;

        let mut consumed = 0usize;
        while consumed < max_steps {
            let Some(instruction) = code.get(pc) else {
                break;
            };
            match instruction {
                Instruction::Observe(_) | Instruction::ObserveNativeLiteral { .. }
                    if active_type_trackers == 0 =>
                {
                    pc += 1;
                }
                Instruction::PrepareModuleImports(unit) => {
                    if program
                        .units
                        .get(unit.0 as usize)
                        .is_some_and(|u| u.import_order.is_empty())
                    {
                        pc += 1;
                    } else {
                        break;
                    }
                }
                Instruction::LoadLocal(depth) => {
                    let locals = &frames.last().unwrap().environment.locals;
                    let Some(offset) = depth.checked_add(1) else {
                        break;
                    };
                    let Some(index) = locals.len().checked_sub(offset) else {
                        break;
                    };
                    match &locals[index] {
                        Value::Int(n) => stack.push_raw(Value::Int(n.boxed())),
                        Value::Bool(b) => stack.push_raw(Value::Bool(*b)),
                        Value::Float(f) => stack.push_raw(Value::Float(f.boxed())),
                        Value::Null => stack.push_raw(Value::Null),
                        other => stack.push_raw(other.clone()),
                    }
                    pc += 1;
                }
                Instruction::StoreLocal(depth) => {
                    if stack.len() <= base {
                        break;
                    }
                    let locals = &mut frames.last_mut().unwrap().environment.locals;
                    let Some(offset) = depth.checked_add(1) else {
                        break;
                    };
                    let Some(index) = locals.len().checked_sub(offset) else {
                        break;
                    };
                    locals[index] = stack.pop().unwrap();
                    pc += 1;
                }
                Instruction::Constant(value) => {
                    match value {
                        Value::Int(n) => stack.push_raw(Value::Int(n.boxed())),
                        Value::Bool(b) => stack.push_raw(Value::Bool(*b)),
                        Value::Float(f) => stack.push_raw(Value::Float(f.boxed())),
                        Value::Null => stack.push_raw(Value::Null),
                        other => stack.push_raw(other.clone()),
                    }
                    pc += 1;
                }
                Instruction::Pop => {
                    if stack.len() <= base {
                        break;
                    }
                    stack.pop();
                    pc += 1;
                }
                Instruction::Duplicate => {
                    if stack.len() <= base {
                        break;
                    }
                    let value = stack.last().unwrap().clone();
                    stack.push_raw(value);
                    pc += 1;
                }
                Instruction::Swap => {
                    if stack.len() < base + 2 {
                        break;
                    }
                    let len = stack.len();
                    stack.swap(len - 1, len - 2);
                    pc += 1;
                }
                Instruction::Jump(target) => {
                    if *target >= code.len() {
                        break;
                    }
                    pc = *target;
                }
                Instruction::JumpIfFalse(target) => {
                    if stack.len() <= base || *target >= code.len() {
                        break;
                    }
                    if matches!(stack.last(), Some(Value::Bool(_))) {
                        let Some(Value::Bool(cond)) = stack.pop() else {
                            unreachable!()
                        };
                        pc = if cond { pc + 1 } else { *target };
                    } else {
                        break;
                    }
                }
                Instruction::JumpIfNull(target) => {
                    if stack.len() <= base || *target >= code.len() {
                        break;
                    }
                    let is_null = stack.pop().unwrap().is_null();
                    pc = if is_null { *target } else { pc + 1 };
                }
                Instruction::JumpIfNotNull(target) => {
                    if stack.len() <= base || *target >= code.len() {
                        break;
                    }
                    let is_null = stack.pop().unwrap().is_null();
                    pc = if !is_null { *target } else { pc + 1 };
                }
                Instruction::AddIntConstant(right) => {
                    if stack.len() <= base {
                        break;
                    }
                    if let Some(Value::Int(left)) = stack.last_mut() {
                        let Some(sum) = left.get().checked_add(*right) else {
                            break;
                        };
                        *left = crate::value_identity::Number::from(sum).boxed();
                        pc += 1;
                    } else {
                        break;
                    }
                }
                Instruction::Add => {
                    if stack.len() < base + 2
                        || frames.last().unwrap().collection_addition.is_some()
                    {
                        break;
                    }
                    let len = stack.len();
                    match (&stack[len - 2], &stack[len - 1]) {
                        (Value::Int(l), Value::Int(r)) => {
                            let Some(sum) = l.get().checked_add(r.get()) else {
                                break;
                            };
                            let Some(Value::Int(_)) = stack.pop() else {
                                unreachable!()
                            };
                            if let Some(Value::Int(slot)) = stack.last_mut() {
                                *slot = crate::value_identity::Number::from(sum).boxed();
                            }
                            pc += 1;
                        }
                        (Value::String(_), Value::String(_)) => {
                            let Some(Value::String(r)) = stack.pop() else {
                                unreachable!()
                            };
                            let Some(Value::String(l)) = stack.pop() else {
                                unreachable!()
                            };
                            stack.push_raw(Value::string(l.concat(&r)));
                            pc += 1;
                        }
                        _ => break,
                    }
                }
                Instruction::Subtract => {
                    if stack.len() < base + 2 {
                        break;
                    }
                    let len = stack.len();
                    if let (Value::Int(l), Value::Int(r)) = (&stack[len - 2], &stack[len - 1]) {
                        let Some(diff) = l.get().checked_sub(r.get()) else {
                            break;
                        };
                        let Some(Value::Int(_)) = stack.pop() else {
                            unreachable!()
                        };
                        if let Some(Value::Int(slot)) = stack.last_mut() {
                            *slot = crate::value_identity::Number::from(diff).boxed();
                        }
                        pc += 1;
                    } else {
                        break;
                    }
                }
                Instruction::Multiply => {
                    if stack.len() < base + 2 {
                        break;
                    }
                    let len = stack.len();
                    if let (Value::Int(l), Value::Int(r)) = (&stack[len - 2], &stack[len - 1]) {
                        let Some(prod) = l.get().checked_mul(r.get()) else {
                            break;
                        };
                        let Some(Value::Int(_)) = stack.pop() else {
                            unreachable!()
                        };
                        if let Some(Value::Int(slot)) = stack.last_mut() {
                            *slot = crate::value_identity::Number::from(prod).boxed();
                        }
                        pc += 1;
                    } else {
                        break;
                    }
                }
                Instruction::Less
                | Instruction::LessEqual
                | Instruction::Greater
                | Instruction::GreaterEqual
                | Instruction::Equal
                | Instruction::NotEqual => {
                    if stack.len() < base + 2 {
                        break;
                    }
                    let len = stack.len();
                    if let (Value::Int(l), Value::Int(r)) = (&stack[len - 2], &stack[len - 1]) {
                        let (lv, rv) = (l.get(), r.get());
                        let res = match instruction {
                            Instruction::Less => lv < rv,
                            Instruction::LessEqual => lv <= rv,
                            Instruction::Greater => lv > rv,
                            Instruction::GreaterEqual => lv >= rv,
                            Instruction::Equal => lv == rv,
                            Instruction::NotEqual => lv != rv,
                            _ => unreachable!(),
                        };
                        let Some(Value::Int(_)) = stack.pop() else {
                            unreachable!()
                        };
                        let Some(Value::Int(_)) = stack.pop() else {
                            unreachable!()
                        };
                        stack.push_raw(Value::Bool(res));
                        pc += 1;
                    } else {
                        break;
                    }
                }
                Instruction::CheckInterpolate => {
                    if stack.len() <= base {
                        break;
                    }
                    if matches!(stack.last(), Some(Value::String(_) | Value::Int(_))) {
                        pc += 1;
                    } else {
                        break;
                    }
                }
                Instruction::FinishInterpolation(count) => {
                    if stack.len() < base + *count {
                        break;
                    }
                    if finish_interpolation_on_stack(stack, *count, scratch_utf8).is_ok() {
                        pc += 1;
                    } else {
                        break;
                    }
                }
                Instruction::ToString => {
                    if stack.len() <= base {
                        break;
                    }
                    match stack.last() {
                        Some(Value::Int(n)) => {
                            let s = format_i64_pkl_string(n.get());
                            let Some(Value::Int(_)) = stack.pop() else {
                                unreachable!()
                            };
                            stack.push_raw(Value::String(s));
                            pc += 1;
                        }
                        Some(Value::String(_)) => {
                            pc += 1;
                        }
                        _ => break,
                    }
                }
                Instruction::GetMember(name) => {
                    if stack.len() <= base {
                        break;
                    }
                    let Some(&Value::Object(recv_id)) = stack.last() else {
                        break;
                    };
                    let Ok(obj) = heap.get(recv_id) else {
                        break;
                    };
                    if name == "length"
                        && matches!(
                            obj.collection_kind,
                            CollectionKind::List | CollectionKind::Map | CollectionKind::Set
                        )
                        && obj.parent.is_none()
                    {
                        let len = obj.length as i64;
                        let Some(Value::Object(_)) = stack.pop() else {
                            unreachable!()
                        };
                        stack
                            .push_raw(Value::Int(crate::value_identity::Number::from(len).boxed()));
                        pc += 1;
                    } else if name == "values"
                        && obj.collection_kind == CollectionKind::Map
                        && obj.parent.is_none()
                        && obj.map_key_order.is_none()
                        && obj.cache_slots.len() >= obj.collection_members.len()
                        && !obj.collection_members.is_empty()
                    {
                        let order = obj.map_index().insertion_order();
                        if order.is_empty() {
                            break;
                        }
                        let mut values = Vec::with_capacity(order.len());
                        let mut approx_bytes =
                            order.len().saturating_mul(std::mem::size_of::<Value>());
                        let mut fast_ok = true;
                        for &idx in order {
                            let member = &obj.collection_members[idx];
                            let slot = &obj.cache_slots[idx];
                            if matches!(
                                member.key,
                                Value::String(_) | Value::Int(_) | Value::Float(_) | Value::Bool(_)
                            ) && slot.key.entry
                                && slot.key.owner.is_none()
                                && !slot.key.method
                                && slot.key.name == member.name
                            {
                                if let Value::String(s) = &slot.value {
                                    approx_bytes = approx_bytes
                                        .saturating_add(s.as_str().map_or(16, str::len));
                                }
                                values.push(slot.value.clone());
                            } else {
                                fast_ok = false;
                                break;
                            }
                        }
                        if !fast_ok {
                            break;
                        }
                        heap.note_external_allocation(approx_bytes);
                        let list_id = heap.allocate_strict_list(values);
                        let Some(Value::Object(_)) = stack.pop() else {
                            unreachable!()
                        };
                        stack.push_raw(Value::Object(list_id));
                        pc += 1;
                    } else if obj.collection_kind == CollectionKind::Dynamic
                        && obj.reference.is_none()
                        && obj.pair.is_none()
                        && obj.function.is_none()
                        && obj.native_function.is_none()
                        && (obj.class_definition.is_none() || obj.is_module)
                        && obj.member_deprecations.is_empty()
                    {
                        let Some(val) = fast_lookup_cached(obj, None, name, false) else {
                            break;
                        };
                        let cloned = val.clone();
                        let Some(Value::Object(_)) = stack.pop() else {
                            unreachable!()
                        };
                        stack.push_raw(cloned);
                        pc += 1;
                    } else {
                        break;
                    }
                }
                Instruction::GetProperty {
                    name,
                    owner_depth,
                    local,
                } => {
                    let frame = frames.last().unwrap();
                    let Some(&scope) = frame.environment.scopes.get(*owner_depth) else {
                        break;
                    };
                    let Ok(obj) = heap.get(scope.receiver) else {
                        break;
                    };
                    if obj.collection_kind == CollectionKind::Dynamic
                        && obj.reference.is_none()
                        && obj.pair.is_none()
                        && obj.function.is_none()
                        && obj.native_function.is_none()
                        && (obj.class_definition.is_none() || obj.is_module)
                        && obj.member_deprecations.is_empty()
                    {
                        let Some(val) =
                            fast_lookup_cached(obj, local.then_some(scope.owner), name, false)
                        else {
                            break;
                        };
                        stack.push_raw(val.clone());
                        pc += 1;
                    } else {
                        break;
                    }
                }
                Instruction::GetImplicitMethod(name) => {
                    let frame = frames.last().unwrap();
                    if frame.method_call_site.is_some() {
                        break;
                    }
                    let Some(&scope) = frame.environment.scopes.first() else {
                        break;
                    };
                    let Ok(obj) = heap.get(scope.receiver) else {
                        break;
                    };
                    if obj.collection_kind == CollectionKind::Dynamic
                        && obj.reference.is_none()
                        && obj.pair.is_none()
                        && obj.function.is_none()
                        && obj.native_function.is_none()
                        && (obj.class_definition.is_none() || obj.is_module)
                        && obj.member_deprecations.is_empty()
                    {
                        let Some(val) = fast_lookup_cached(obj, None, name, true) else {
                            break;
                        };
                        stack.push_raw(val.clone());
                        pc += 1;
                    } else {
                        break;
                    }
                }
                Instruction::GetMethod(name) => {
                    if stack.len() <= base {
                        break;
                    }
                    if name == "toString"
                        && active_type_trackers == 0
                        && let Some(Value::Int(n)) = stack.last()
                    {
                        let skip = match (code.get(pc + 1), code.get(pc + 2), code.get(pc + 3)) {
                            (
                                Some(Instruction::PrepareMethodCall { arity: 0, .. }),
                                Some(Instruction::PrepareMethodCall { arity: 0, .. }),
                                Some(Instruction::Call { arity: 0, .. }),
                            ) => 4,
                            (
                                Some(Instruction::PrepareMethodCall { arity: 0, .. }),
                                Some(Instruction::Call { arity: 0, .. }),
                                _,
                            ) => 3,
                            (Some(Instruction::Call { arity: 0, .. }), _, _) => 2,
                            _ => 0,
                        };
                        if skip > 0 && consumed + skip <= max_steps {
                            let s = if let (
                                Some(Instruction::Constant(Value::String(suffix))),
                                Some(Instruction::Add),
                            ) = (code.get(pc + skip), code.get(pc + skip + 1))
                                && consumed + skip + 2 <= max_steps
                                && let Some(sfx) = suffix.as_str()
                                && sfx.is_ascii()
                                && sfx.len() <= 16
                            {
                                let mut ibuf = [0u8; 24];
                                let num_str = format_i64_buf(n.get(), &mut ibuf);
                                let mut out = [0u8; 40];
                                let nlen = num_str.len();
                                let slen = sfx.len();
                                out[..nlen].copy_from_slice(num_str.as_bytes());
                                out[nlen..nlen + slen].copy_from_slice(sfx.as_bytes());
                                let combined = PklString::from(
                                    std::str::from_utf8(&out[..nlen + slen])
                                        .expect("ASCII integer + suffix"),
                                );
                                let Some(Value::Int(_)) = stack.pop() else {
                                    unreachable!()
                                };
                                stack.push_raw(Value::String(combined));
                                pc += skip + 2;
                                consumed += skip + 2;
                                continue;
                            } else {
                                format_i64_pkl_string(n.get())
                            };
                            let Some(Value::Int(_)) = stack.pop() else {
                                unreachable!()
                            };
                            stack.push_raw(Value::String(s));
                            pc += skip;
                            consumed += skip;
                            continue;
                        }
                        break;
                    }
                    let Some(&Value::Object(recv_id)) = stack.last() else {
                        break;
                    };
                    let Ok(obj) = heap.get(recv_id) else {
                        break;
                    };
                    if obj.collection_kind == CollectionKind::Dynamic
                        && obj.reference.is_none()
                        && obj.pair.is_none()
                        && obj.function.is_none()
                        && obj.native_function.is_none()
                        && (obj.class_definition.is_none() || obj.is_module)
                        && obj.member_deprecations.is_empty()
                    {
                        let Some(val) = fast_lookup_cached(obj, None, name, true) else {
                            break;
                        };
                        let cloned = val.clone();
                        let Some(Value::Object(_)) = stack.pop() else {
                            unreachable!()
                        };
                        stack.push_raw(cloned);
                        pc += 1;
                    } else if obj.collection_kind == CollectionKind::List
                        && obj.parent.is_none()
                        && obj.instance_class.is_none()
                        && obj.pair.is_none()
                        && obj.reference.is_none()
                        && obj.foreign_map_entry.is_none()
                        && (name == "map" || name == "fold")
                    {
                        let frame = frames.last().unwrap();
                        let exec_owner = if let Some(o) = frame.environment.executable_owner {
                            Some(o)
                        } else {
                            if cur_exec_block != block {
                                cur_exec_block = block;
                                cur_exec_root = executable_roots.get(&block).copied();
                            }
                            cur_exec_root
                        };
                        if let Some(owner) = exec_owner
                            && heap
                                .existing_method_call_cache(owner, (block, pc))
                                .ok()
                                .flatten()
                                .is_some_and(|c| {
                                    c.contains_builtin_class(classes::BuiltinClass::List)
                                })
                        {
                            let (fn_block, arity) = if name == "map" {
                                (program.collection_transform_blocks[0], 1)
                            } else {
                                (program.list_fold_block, 2)
                            };
                            let id = heap.allocate_function(
                                Function {
                                    block: fn_block,
                                    arity,
                                    method: true,
                                    captured_this: Some(Value::Object(recv_id)),
                                },
                                None,
                                Environment::default(),
                            );
                            let Some(Value::Object(_)) = stack.pop() else {
                                unreachable!()
                            };
                            stack.push_raw(Value::Function(id));
                            pc += 1;
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                }
                Instruction::MakeFunction {
                    block: fn_block,
                    arity,
                    method,
                    capture_custom_this,
                } => {
                    if *method || *capture_custom_this {
                        break;
                    }
                    let Some(fn_code) = program.blocks.get(*fn_block) else {
                        break;
                    };
                    let frame = frames.last().unwrap();
                    if frame.environment.executable_owner.is_some() {
                        break;
                    }
                    let environment = if is_non_capturing_lambda(fn_code, *arity) {
                        Environment {
                            code_owner: frame.environment.code_owner,
                            ..Environment::default()
                        }
                    } else {
                        frame.environment.clone()
                    };
                    let id = heap.allocate_function(
                        Function {
                            block: *fn_block,
                            arity: *arity,
                            method: false,
                            captured_this: None,
                        },
                        None,
                        environment,
                    );
                    stack.push_raw(Value::Function(id));
                    pc += 1;
                }
                Instruction::CheckType(ty) => {
                    if stack.len() <= base {
                        break;
                    }
                    let val = stack.last().unwrap();
                    if fast_check_type_in_poll(
                        ty,
                        val,
                        frames.last().unwrap(),
                        heap,
                        resolved_declarations,
                    ) {
                        pc += 1;
                    } else {
                        break;
                    }
                }
                Instruction::ReflectCall(operation) => {
                    let locals = &frames.last().unwrap().environment.locals;
                    let arity = operation.arity();
                    let Some(arg) = locals.len().checked_sub(arity).and_then(|s| locals.get(s))
                    else {
                        break;
                    };
                    let cached = match operation {
                        reflection::Operation::Module => arg
                            .heap_id()
                            .and_then(|id| heap.get(id).ok())
                            .filter(|obj| obj.module_unit.is_some())
                            .and_then(|obj| obj.module_mirror),
                        reflection::Operation::Class => match arg {
                            Value::Class(id) => heap
                                .get_class(*id)
                                .ok()
                                .and_then(|cls| heap.get(cls.prototype).ok())
                                .and_then(|proto| proto.declaration_mirror),
                            _ => None,
                        },
                        reflection::Operation::TypeAlias => match arg {
                            Value::TypeAlias(id) => {
                                heap.get(*id).ok().and_then(|obj| obj.declaration_mirror)
                            }
                            _ => None,
                        },
                        _ => None,
                    };
                    if let Some(mirror_id) = cached {
                        stack.push_raw(Value::Object(mirror_id));
                        pc += 1;
                    } else {
                        break;
                    }
                }
                Instruction::BindLocal { depth, ty } => {
                    if stack.len() <= base {
                        break;
                    }
                    let frame = frames.last().unwrap();
                    if !frame.generator_scopes.is_empty() {
                        break;
                    }
                    let owner = if let Some(o) = frame.environment.executable_owner {
                        o
                    } else {
                        if cur_exec_block != block {
                            cur_exec_block = block;
                            cur_exec_root = executable_roots.get(&block).copied();
                        }
                        let Some(o) = cur_exec_root else {
                            break;
                        };
                        o
                    };
                    let site = (block, pc);
                    // Ultra-fast repeat hit for hot loop/lambda Int bindings.
                    if let Some((co, cb, cp, identity::SlotKind::Long)) = last_bind_ok
                        && co == owner
                        && cb == block
                        && cp == pc
                        && let Some(Value::Int(v)) = stack.last()
                    {
                        let unboxed = v.unboxed();
                        let index = match depth {
                            Some(d) => match frame.environment.locals.len().checked_sub(d + 1) {
                                Some(i) => i,
                                None => break,
                            },
                            None => frame.environment.locals.len(),
                        };
                        let Some(Value::Int(_)) = stack.pop() else {
                            unreachable!()
                        };
                        let env = &mut frames.last_mut().unwrap().environment;
                        if depth.is_some() {
                            if let Value::Int(slot) = &mut env.locals[index] {
                                *slot = unboxed;
                            } else {
                                env.locals[index] = Value::Int(unboxed);
                            }
                        } else {
                            env.locals.push(Value::Int(unboxed));
                        }
                        env.local_owners.insert(index, (owner, site));
                        env.copy_locals.insert(index);
                        pc += 1;
                        consumed += 1;
                        continue;
                    }
                    let top_val = stack.last().unwrap();
                    if !fast_check_type_in_poll(ty, top_val, frame, heap, resolved_declarations) {
                        break;
                    }
                    let index = match depth {
                        Some(d) => match frame.environment.locals.len().checked_sub(d + 1) {
                            Some(i) => i,
                            None => break,
                        },
                        None => frame.environment.locals.len(),
                    };
                    let policy = identity::SlotPolicy::from_type(ty);
                    let kind = match policy {
                        identity::SlotPolicy::Fixed(k) => k,
                        identity::SlotPolicy::Adaptive => {
                            let actual = match top_val {
                                Value::Int(_) => identity::SlotKind::Long,
                                Value::Float(_) => identity::SlotKind::Double,
                                Value::Bool(_) => identity::SlotKind::Boolean,
                                _ => identity::SlotKind::Object,
                            };
                            if let Some((co, cb, cp, ck)) = last_bind_ok
                                && co == owner
                                && cb == block
                                && cp == pc
                                && (ck == actual || ck == identity::SlotKind::Object)
                            {
                                ck
                            } else {
                                match heap.local_kind(owner, site) {
                                    Ok(Some(prev)) if prev == actual => actual,
                                    Ok(Some(identity::SlotKind::Object)) => {
                                        identity::SlotKind::Object
                                    }
                                    _ => break,
                                }
                            }
                        }
                    };
                    if last_bind_ok != Some((owner, block, pc, kind)) {
                        if !matches!(heap.local_kind(owner, site), Ok(Some(existing)) if existing == kind)
                        {
                            break;
                        }
                        last_bind_ok = Some((owner, block, pc, kind));
                    }
                    let stored = match (kind, stack.last().unwrap()) {
                        (identity::SlotKind::Long, Value::Int(v)) => {
                            let unboxed = v.unboxed();
                            let Some(Value::Int(_)) = stack.pop() else {
                                unreachable!()
                            };
                            Value::Int(unboxed)
                        }
                        (identity::SlotKind::Double, Value::Float(v)) => {
                            let unboxed = v.unboxed();
                            let Some(Value::Float(_)) = stack.pop() else {
                                unreachable!()
                            };
                            Value::Float(unboxed)
                        }
                        _ => stack.pop().unwrap().boxed(),
                    };
                    let env = &mut frames.last_mut().unwrap().environment;
                    if depth.is_some() {
                        env.locals[index] = stored;
                    } else {
                        env.locals.push(stored);
                    }
                    env.local_owners.insert(index, (owner, site));
                    env.copy_locals.insert(index);
                    pc += 1;
                }
                Instruction::GetMethodProperty {
                    name,
                    owner_depth,
                    local,
                } => {
                    let frame = frames.last().unwrap();
                    if frame.method_call_site.is_some() {
                        break;
                    }
                    let Some(&scope) = frame.environment.scopes.get(*owner_depth) else {
                        break;
                    };
                    if !*local {
                        if method_cache[0].0 == block
                            && method_cache[0].1 == pc
                            && method_cache[0].2 == scope.receiver
                        {
                            stack.push_raw(Value::Function(method_cache[0].3));
                            pc += 1;
                            consumed += 1;
                            continue;
                        }
                        if method_cache[1].0 == block
                            && method_cache[1].1 == pc
                            && method_cache[1].2 == scope.receiver
                        {
                            stack.push_raw(Value::Function(method_cache[1].3));
                            pc += 1;
                            consumed += 1;
                            continue;
                        }
                    }
                    let exec_owner = if let Some(o) = frame.environment.executable_owner {
                        Some(o)
                    } else {
                        if cur_exec_block != block {
                            cur_exec_block = block;
                            cur_exec_root = executable_roots.get(&block).copied();
                        }
                        cur_exec_root
                    };
                    if let Some(owner) = exec_owner {
                        match heap.existing_method_call_cache(owner, (block, pc)) {
                            Ok(Some(cache)) if cache.is_saturated() => {
                                break;
                            }
                            Err(_) => break,
                            _ => {}
                        }
                    }
                    let Ok(obj) = heap.get(scope.receiver) else {
                        break;
                    };
                    let Some(val) =
                        fast_lookup_cached(obj, local.then_some(scope.owner), name, true)
                    else {
                        break;
                    };
                    if !*local && let Value::Function(fn_id) = val {
                        method_cache[method_cache_next] = (block, pc, scope.receiver, *fn_id);
                        method_cache_next ^= 1;
                    }
                    stack.push_raw(val.clone());
                    pc += 1;
                }
                Instruction::PrepareMethodCall {
                    lookup,
                    arity,
                    after_arguments,
                } => {
                    if stack.len() <= *arity {
                        break;
                    }
                    let Some(Value::Function(function_id)) = stack.get(stack.len() - arity - 1)
                    else {
                        break;
                    };
                    let Ok(obj) = heap.get(*function_id) else {
                        break;
                    };
                    let has_method = obj.function.as_ref().is_none_or(|f| f.method);
                    if !has_method {
                        pc += 1;
                        consumed += 1;
                        continue;
                    }
                    let has_receiver = obj.native_function.is_some()
                        || obj
                            .function
                            .as_ref()
                            .is_some_and(|f| f.captured_this.is_some())
                        || obj.environment.this_value.is_some();
                    if !has_receiver {
                        pc += 1;
                        consumed += 1;
                        continue;
                    }
                    match code.get(*lookup) {
                        Some(Instruction::GetMethodProperty {
                            owner_depth, local, ..
                        }) => {
                            let Some(&scope) =
                                frames.last().unwrap().environment.scopes.get(*owner_depth)
                            else {
                                break;
                            };
                            let Ok(owner_obj) = heap.get(scope.owner) else {
                                break;
                            };
                            let Some(class) = owner_obj.class_definition.as_ref() else {
                                pc += 1;
                                consumed += 1;
                                continue;
                            };
                            let direct = *local || !(class.open || class.abstract_);
                            if !direct {
                                break;
                            }
                            if !*after_arguments {
                                pc += 1;
                                consumed += 1;
                                continue;
                            }
                            if let Some((decl_owner, decl_code)) = obj.method_declaration
                                && heap
                                    .get(decl_owner)
                                    .is_ok_and(|o| o.member_deprecations.contains_key(&decl_code))
                            {
                                break;
                            }
                            pc += 1;
                        }
                        Some(Instruction::GetMethod(_) | Instruction::GetImplicitMethod(_)) => {
                            // Virtual calls have `(direct, lexical) == (false, false)`:
                            // `after_arguments == true` is an unconditional no-op.
                            if *after_arguments {
                                pc += 1;
                                consumed += 1;
                                continue;
                            }
                            let frame = frames.last().unwrap();
                            if frame.method_call_site.is_some() {
                                break;
                            }
                            if let Some((decl_owner, decl_code)) = obj.method_declaration
                                && heap
                                    .get(decl_owner)
                                    .is_ok_and(|o| o.member_deprecations.contains_key(&decl_code))
                            {
                                break;
                            }
                            let recv_val = obj
                                .function
                                .as_ref()
                                .and_then(|f| f.captured_this.as_ref())
                                .or(obj.environment.this_value.as_ref());
                            let Some(Value::Object(recv_id)) = recv_val else {
                                break;
                            };
                            let Ok(recv_obj) = heap.get(*recv_id) else {
                                break;
                            };
                            let exec_owner = if let Some(o) = frame.environment.executable_owner {
                                Some(o)
                            } else {
                                if cur_exec_block != block {
                                    cur_exec_block = block;
                                    cur_exec_root = executable_roots.get(&block).copied();
                                }
                                cur_exec_root
                            };
                            let Some(owner) = exec_owner else {
                                break;
                            };
                            if let Some(recv_cls) = recv_obj.instance_class {
                                let key = Some((owner, block, *lookup, recv_cls));
                                if prep_method_cache[0] == key
                                    || prep_method_cache[1] == key
                                    || prep_method_cache[2] == key
                                    || prep_method_cache[3] == key
                                {
                                    pc += 1;
                                    consumed += 1;
                                    continue;
                                }
                                if heap
                                    .existing_method_call_cache(owner, (block, *lookup))
                                    .ok()
                                    .flatten()
                                    .is_some_and(|c| c.contains_declared_class(recv_cls))
                                {
                                    prep_method_cache[prep_method_next] = key;
                                    prep_method_next = (prep_method_next + 1) & 3;
                                    pc += 1;
                                } else {
                                    break;
                                }
                            } else if recv_obj.parent.is_none()
                                && recv_obj.pair.is_none()
                                && recv_obj.reference.is_none()
                                && recv_obj.foreign_map_entry.is_none()
                                && let Some(builtin_cls) = match recv_obj.collection_kind {
                                    CollectionKind::List => Some(classes::BuiltinClass::List),
                                    CollectionKind::Map => Some(classes::BuiltinClass::Map),
                                    CollectionKind::Set => Some(classes::BuiltinClass::Set),
                                    _ => None,
                                }
                            {
                                if heap
                                    .existing_method_call_cache(owner, (block, *lookup))
                                    .ok()
                                    .flatten()
                                    .is_some_and(|c| c.contains_builtin_class(builtin_cls))
                                {
                                    pc += 1;
                                } else {
                                    break;
                                }
                            } else {
                                break;
                            }
                        }
                        _ => break,
                    }
                }
                Instruction::CollectionTransformNext { end } => {
                    if *end >= code.len() {
                        break;
                    }
                    if active_type_trackers == 0 {
                        let frame = frames.last_mut().unwrap();
                        let builder = frame.builders.last().copied();
                        if let Some(run) = frame.collection_transform.as_mut()
                            && let Some((steps, finished)) = run.try_fast_member_chain_bulk(
                                heap,
                                builder,
                                program,
                                executable_roots,
                                max_steps.saturating_sub(consumed),
                            )
                        {
                            consumed = consumed.saturating_add(steps);
                            if finished {
                                pc = *end;
                                continue;
                            }
                            if consumed >= max_steps {
                                break;
                            }
                        }
                    }
                    let Some(run) = frames.last_mut().unwrap().collection_transform.as_mut() else {
                        break;
                    };
                    match run.try_fast_map_next(heap, stack) {
                        Some(true) => pc += 1,
                        Some(false) => pc = *end,
                        None => break,
                    }
                }
                Instruction::IntSeqNext { end } => {
                    if *end >= code.len() {
                        break;
                    }
                    // Fast bulk pure-i64 loop for IntSeq.map / List.fold when callback is a FastIntBlock.
                    if active_type_trackers == 0 {
                        let cb_fn_id = frames
                            .last()
                            .and_then(|f| f.int_seq_run.as_ref())
                            .and_then(|r| r.callback_function_id());
                        if let Some(fn_id) = cb_fn_id
                            && let Ok(cb_obj) = heap.get(fn_id)
                            && cb_obj.native_function.is_none()
                            && cb_obj.amendment.is_none()
                            && let Some(func) = cb_obj.function.as_ref()
                        {
                            let cb_block = func.block;
                            let cb_arity = func.arity;
                            if let Some(fast_block) = fast_int_cache.get_or_compile(
                                cb_block,
                                program.builtin_block_range.end,
                                || {
                                    program
                                        .blocks
                                        .get(cb_block)
                                        .and_then(|c| fast_int::FastIntBlock::compile(c, cb_arity))
                                        .map(Box::new)
                                },
                            ) && fast_int::verify_fast_int_invocation(
                                fast_block,
                                cb_block,
                                fn_id,
                                cb_obj,
                                heap,
                                executable_roots,
                                fast_int_captured,
                            ) {
                                let frame = frames.last_mut().unwrap();
                                let builder = frame.builders.last().copied();
                                let run = frame.int_seq_run.as_mut().unwrap();
                                let budget = max_steps.saturating_sub(consumed);
                                if let Some(steps) = run.try_fast_int_bulk(
                                    heap,
                                    builder,
                                    fast_block,
                                    fast_int_captured,
                                    budget,
                                    fast_int_vals,
                                    fast_int_locals,
                                    fast_int_calls,
                                ) {
                                    consumed = consumed.saturating_add(steps);
                                    pc = *end;
                                    continue;
                                }
                            }
                        }
                    }
                    let Some(run) = frames.last_mut().unwrap().int_seq_run.as_mut() else {
                        break;
                    };
                    match run.try_fast_next(heap, stack) {
                        Some(true) => pc += 1,
                        Some(false) => pc = *end,
                        None => break,
                    }
                }
                Instruction::IntSeqAccept => {
                    if stack.len() <= base {
                        break;
                    }
                    let frame = frames.last_mut().unwrap();
                    let builder = frame.builders.last().copied();
                    let Some(run) = frame.int_seq_run.as_mut() else {
                        break;
                    };
                    if run.try_fast_accept(heap, builder, stack) {
                        pc += 1;
                    } else {
                        break;
                    }
                }
                Instruction::Call { arity, method } => {
                    if stack.len() < base + arity + 1 || frames.len() >= 4096 {
                        break;
                    }
                    let fn_idx = stack.len() - arity - 1;
                    let Value::Function(id) = stack[fn_idx] else {
                        break;
                    };
                    let Ok(object) = heap.get(id) else {
                        break;
                    };
                    if object.native_function.is_some() || object.amendment.is_some() {
                        break;
                    }
                    let Some(function) = object.function.as_ref() else {
                        break;
                    };
                    if (!function.method && *method) || function.arity != *arity {
                        break;
                    }
                    let target_block = function.block;
                    // Speculative pure-i64 FastIntBlock path for hot recursive / arithmetic functions.
                    if active_type_trackers == 0
                        && *arity <= 4
                        && stack[fn_idx + 1..]
                            .iter()
                            .all(|v| matches!(v, Value::Int(_)))
                        && let Some(fast_block) = fast_int_cache.get_or_compile(
                            target_block,
                            program.builtin_block_range.end,
                            || {
                                program
                                    .blocks
                                    .get(target_block)
                                    .and_then(|c| fast_int::FastIntBlock::compile(c, *arity))
                                    .map(Box::new)
                            },
                        )
                        && fast_int::verify_fast_int_invocation(
                            fast_block,
                            target_block,
                            id,
                            object,
                            heap,
                            executable_roots,
                            fast_int_captured,
                        )
                    {
                        let mut int_args = [0i64; 4];
                        for (i, val) in stack[fn_idx + 1..].iter().enumerate() {
                            if let Value::Int(n) = val {
                                int_args[i] = n.get();
                            }
                        }
                        let budget = max_steps.saturating_sub(consumed);
                        if let Some((res, steps)) = fast_block.execute(
                            fast_int_captured,
                            &int_args[..*arity],
                            budget,
                            fast_int_vals,
                            fast_int_locals,
                            fast_int_calls,
                        ) {
                            stack.truncate(fn_idx);
                            let out = if fast_block.returns_int {
                                Value::Int(crate::value_identity::Number::from(res).boxed())
                            } else {
                                Value::Bool(res != 0)
                            };
                            stack.push_raw(out);
                            pc += 1;
                            consumed = consumed.saturating_add(steps.max(1));
                            continue;
                        }
                    }
                    let target_code = if target_block == block {
                        code
                    } else {
                        let Some(c) = program.blocks.get(target_block).map(Vec::as_slice) else {
                            break;
                        };
                        c
                    };
                    frames.last_mut().unwrap().pc = pc + 1;
                    let mut next_frame = frame_pool
                        .pop()
                        .unwrap_or_else(|| Frame::new_empty(0, 0, 0, Environment::default()));
                    let env = &mut next_frame.environment;
                    env.locals.clear();
                    let captured_len = object.environment.locals.len();
                    if captured_len > 0 {
                        env.locals.extend_from_slice(&object.environment.locals);
                    }
                    match *arity {
                        0 => {
                            let Some(Value::Function(_)) = stack.pop() else {
                                unreachable!()
                            };
                        }
                        1 => {
                            let a0 = stack.pop().unwrap();
                            let Some(Value::Function(_)) = stack.pop() else {
                                unreachable!()
                            };
                            env.locals.push(a0);
                        }
                        2 => {
                            let a1 = stack.pop().unwrap();
                            let a0 = stack.pop().unwrap();
                            let Some(Value::Function(_)) = stack.pop() else {
                                unreachable!()
                            };
                            env.locals.push(a0);
                            env.locals.push(a1);
                        }
                        _ => {
                            env.locals.extend(stack.drain(fn_idx + 1..));
                            let Some(Value::Function(_)) = stack.pop() else {
                                unreachable!()
                            };
                        }
                    }
                    if !object.environment.local_owners.is_empty() {
                        env.local_owners = object.environment.local_owners.clone();
                    }
                    env.copy_locals.clear();
                    env.restore_for_bindings = false;
                    env.outer_value = object.environment.this_value.clone();
                    env.this_value = function
                        .captured_this
                        .clone()
                        .or_else(|| object.environment.this_value.clone());
                    env.custom_this = None;
                    env.executable_owner = object.environment.executable_owner;
                    env.code_owner = object.environment.code_owner;
                    if !object.environment.type_arguments.is_empty() {
                        env.type_arguments = object.environment.type_arguments.clone();
                    }
                    if !object.environment.type_argument_sources.is_empty() {
                        env.type_argument_sources =
                            object.environment.type_argument_sources.clone();
                    }
                    env.scopes = object.environment.scopes.clone();
                    let next_base = stack.len();
                    next_frame.block = target_block;
                    next_frame.pc = 0;
                    next_frame.base = next_base;
                    next_frame.lambda = !function.method;
                    next_frame.function_owner = Some(id);
                    next_frame.fast_call_frame = true;
                    next_frame.captured_locals_len = captured_len;
                    frames.push(next_frame);
                    block = target_block;
                    code = target_code;
                    pc = 0;
                    base = next_base;
                }
                Instruction::Return => {
                    if stack.len() != base + 1 {
                        break;
                    }
                    let top_frame = frames.last().unwrap();
                    if !top_frame.fast_call_frame && !top_frame.is_fast_returnable() {
                        break;
                    }
                    // Fast top-level completion (eliminates Vm::step + finish_return on warm queries).
                    if frames.len() == 1 && base == 0 {
                        let result = stack.pop().unwrap();
                        let mut finished = frames.pop().unwrap();
                        if frame_pool.len() < 64 {
                            finished.fast_reset_for_pool();
                            frame_pool.push(finished);
                        }
                        finished_top_result = Some(result);
                        consumed += 1;
                        break;
                    }
                    // In-place lambda frame reuse for hot IntSeq.map / List.fold / List.map loops!
                    let n_frames = frames.len();
                    if top_frame.fast_call_frame && consumed + 5 <= max_steps {
                        let caller_block = frames[n_frames - 2].block;
                        let caller_pc = frames[n_frames - 2].pc;
                        let cached_hit = inplace_loop_cache.0 == block
                            && inplace_loop_cache.1 == caller_block
                            && inplace_loop_cache.2 == caller_pc;
                        let loop_info = if cached_hit {
                            Some((inplace_loop_cache.3, inplace_loop_cache.4))
                        } else if !frames[n_frames - 2].has_pending_run()
                            && frames[n_frames - 2].prepared_hashes.is_none()
                        {
                            if frames[n_frames - 2].int_seq_run.is_some()
                                && let Some(caller_code) = program.blocks.get(caller_block)
                                && matches!(
                                    caller_code.get(caller_pc),
                                    Some(Instruction::IntSeqAccept)
                                )
                                && let Some(Instruction::Jump(next_pc)) =
                                    caller_code.get(caller_pc + 1)
                                && let Some(Instruction::IntSeqNext { end }) =
                                    caller_code.get(*next_pc)
                                && matches!(
                                    caller_code.get(next_pc + 1),
                                    Some(Instruction::Call { method: false, .. })
                                )
                            {
                                inplace_loop_cache = (block, caller_block, caller_pc, *end, false);
                                Some((*end, false))
                            } else if frames[n_frames - 2].collection_transform.is_some()
                                && let Some(caller_code) = program.blocks.get(caller_block)
                                && let Some(Instruction::CollectionTransformProject { next }) =
                                    caller_code.get(caller_pc)
                                && matches!(
                                    caller_code.get(caller_pc + 1),
                                    Some(Instruction::AppendStrictElement)
                                )
                                && matches!(
                                    caller_code.get(caller_pc + 2),
                                    Some(Instruction::Jump(j)) if j == next
                                )
                                && let Some(Instruction::CollectionTransformNext { end }) =
                                    caller_code.get(*next)
                                && matches!(
                                    caller_code.get(*next + 1),
                                    Some(Instruction::Call {
                                        arity: 1,
                                        method: false
                                    })
                                )
                            {
                                inplace_loop_cache = (block, caller_block, caller_pc, *end, true);
                                Some((*end, true))
                            } else {
                                None
                            }
                        } else {
                            None
                        };
                        if let Some((end_target, is_transform_map)) = loop_info {
                            let result = stack.pop().unwrap();
                            let builder = frames[n_frames - 2].builders.last().copied();
                            if !is_transform_map {
                                let inplace = frames[n_frames - 2]
                                    .int_seq_run
                                    .as_mut()
                                    .unwrap()
                                    .try_inplace_accept_and_next(heap, builder, result.clone());
                                match inplace {
                                    Some(Some((a0, opt_a1))) => {
                                        let f = frames.last_mut().unwrap();
                                        let cap = f.captured_locals_len;
                                        if opt_a1.is_none() && f.environment.locals.len() == cap + 1
                                        {
                                            if let (Value::Int(dst), Value::Int(src)) =
                                                (&mut f.environment.locals[cap], &a0)
                                            {
                                                *dst = *src;
                                            } else {
                                                f.environment.locals[cap] = a0;
                                            }
                                        } else {
                                            f.environment.locals.truncate(cap);
                                            f.environment.locals.push(a0);
                                            if let Some(a1) = opt_a1 {
                                                f.environment.locals.push(a1);
                                            }
                                        }
                                        f.environment.local_owners.truncate_from(cap);
                                        f.environment.copy_locals.clear();
                                        pc = 0;
                                        consumed += 4;
                                        continue;
                                    }
                                    Some(None) => {
                                        let mut finished = frames.pop().unwrap();
                                        if frame_pool.len() < 64 {
                                            finished.fast_reset_for_pool();
                                            frame_pool.push(finished);
                                        }
                                        let caller = frames.last_mut().unwrap();
                                        caller.callsite_hidden = false;
                                        caller.pc = end_target;
                                        pc = end_target;
                                        base = caller.base;
                                        block = caller_block;
                                        code = program.blocks.get(caller_block).unwrap().as_slice();
                                        consumed += 4;
                                        continue;
                                    }
                                    None => {
                                        stack.push_raw(result);
                                    }
                                }
                            } else {
                                let inplace = frames[n_frames - 2]
                                    .collection_transform
                                    .as_mut()
                                    .unwrap()
                                    .try_inplace_map_accept_and_next(heap, builder, result.clone());
                                match inplace {
                                    Some(Some(a0)) => {
                                        let f = frames.last_mut().unwrap();
                                        let cap = f.captured_locals_len;
                                        if f.environment.locals.len() == cap + 1 {
                                            f.environment.locals[cap] = a0;
                                        } else {
                                            f.environment.locals.truncate(cap);
                                            f.environment.locals.push(a0);
                                        }
                                        f.environment.local_owners.truncate_from(cap);
                                        f.environment.copy_locals.clear();
                                        pc = 0;
                                        consumed += 5;
                                        continue;
                                    }
                                    Some(None) => {
                                        let mut finished = frames.pop().unwrap();
                                        if frame_pool.len() < 64 {
                                            finished.fast_reset_for_pool();
                                            frame_pool.push(finished);
                                        }
                                        let caller = frames.last_mut().unwrap();
                                        caller.callsite_hidden = false;
                                        caller.pc = end_target;
                                        pc = end_target;
                                        base = caller.base;
                                        block = caller_block;
                                        code = program.blocks.get(caller_block).unwrap().as_slice();
                                        consumed += 5;
                                        continue;
                                    }
                                    None => {
                                        stack.push_raw(result);
                                    }
                                }
                            }
                        }
                    }
                    let mut finished = frames.pop().unwrap();
                    if frame_pool.len() < 64 {
                        finished.fast_reset_for_pool();
                        frame_pool.push(finished);
                    }
                    let caller = frames.last_mut().unwrap();
                    caller.callsite_hidden = false;
                    let caller_block = caller.block;
                    pc = caller.pc;
                    base = caller.base;
                    consumed += 1;
                    if !caller.fast_call_frame
                        && (caller.has_pending_run() || caller.prepared_hashes.is_some())
                    {
                        break;
                    }
                    if caller_block != block {
                        let Some(caller_code) = program.blocks.get(caller_block).map(Vec::as_slice)
                        else {
                            break;
                        };
                        block = caller_block;
                        code = caller_code;
                    }
                    continue;
                }
                _ => break,
            }
            consumed += 1;
        }
        if let Some(frame) = frames.last_mut() {
            frame.pc = pc;
        }
        self.instructions += consumed as u64;
        if let Some(result) = finished_top_result {
            self.inputs.clear();
            self.state = State::Finished(Ok(result));
        }
        consumed
    }

    pub fn resume(
        &mut self,
        id: u64,
        result: Result<Value, RuntimeError>,
    ) -> Result<(), RuntimeError> {
        match &self.state {
            State::Waiting(PendingRequest::Read(request)) if request.id == id => {}
            State::Closed => {
                return Err(RuntimeError::new(ErrorKind::Closed, "evaluator is closed"));
            }
            _ => {
                return Err(RuntimeError::new(
                    ErrorKind::Host,
                    "stale or unexpected Host completion",
                ));
            }
        }
        match result {
            Ok(value) => {
                self.heap.validate_value(&value)?;
                self.stack.push(value);
                self.state = State::Running;
            }
            Err(error) => self.fail(error),
        }
        Ok(())
    }

    pub fn acknowledge_log(
        &mut self,
        id: u64,
        completion: Result<(), LoggerFailure>,
    ) -> Result<(), RuntimeError> {
        if !matches!(&self.state, State::Waiting(PendingRequest::Log(request)) if request.id == id)
        {
            return Err(RuntimeError::new(
                ErrorKind::Host,
                "stale or unexpected Host completion",
            ));
        }
        match completion {
            Ok(()) => {
                self.finish_log_delivery()?;
            }
            Err(error) => self.fail(RuntimeError::callback_failure(
                crate::logging::CallbackOrigin::LogDelegate,
                error.message,
            )),
        }
        Ok(())
    }

    pub(crate) fn close_from_callback(&mut self, id: u64) -> Result<(), RuntimeError> {
        if !matches!(&self.state, State::Waiting(request @ (PendingRequest::Read(_) | PendingRequest::Log(_) | PendingRequest::TransformFrame(_) | PendingRequest::Module(_) | PendingRequest::Glob(_))) if request.id() == id)
        {
            return Err(RuntimeError::new(
                ErrorKind::Host,
                "stale or unexpected Host completion",
            ));
        }
        self.callback_closing = true;
        Ok(())
    }

    pub(crate) fn is_closing(&self) -> bool {
        self.callback_closing
    }

    /// The actual module receiver, including one whose initialization is suspended.
    pub fn primary_module(&self) -> Option<Value> {
        let is_primary_unit = |unit: crate::modules::UnitId| {
            self.program.units.get(unit.0 as usize).is_some_and(|info| {
                !info.standard_library
                    && match self.program.module_template {
                        Some(template) => info.template == template,
                        None => unit == crate::modules::UnitId(0),
                    }
            })
        };
        self.suspended
            .iter()
            .flat_map(|context| &context.frames)
            .chain(self.frames.iter())
            .flat_map(|frame| &frame.environment.scopes)
            .map(|scope| scope.receiver)
            .find(|id| {
                self.heap
                    .get(*id)
                    .is_ok_and(|object| object.module_unit.is_some_and(is_primary_unit))
            })
            .map(Value::Object)
            .or_else(|| match &self.state {
                State::Finished(Ok(Value::Object(id)))
                    if self
                        .heap
                        .get(*id)
                        .is_ok_and(|object| object.module_unit.is_some_and(is_primary_unit)) =>
                {
                    Some(Value::Object(*id))
                }
                _ => None,
            })
    }

    pub fn expression_string_needs_conversion(&self, value: &Value) -> bool {
        self.is_reference(value)
            || self.is_map(value)
            || self.is_set(value)
            || self.is_list(value)
            || matches!(value, Value::TypeAlias(_))
    }

    /// Cancellation terminates the active computation; a later start has fresh stack state.
    pub fn cancel(&mut self) {
        if matches!(self.state, State::Running | State::Waiting(_)) {
            loop {
                self.fail(RuntimeError::new(
                    ErrorKind::Cancelled,
                    "evaluation cancelled",
                ));
                if self.suspended.is_empty() {
                    break;
                }
                self.restore_context();
            }
        }
    }

    pub fn close(&mut self) {
        self.suspended = Vec::new();
        self.frames = Vec::new();
        self.frame_pool = Vec::new();
        self.stack = identity::OperandStack::default();
        self.inputs = Vec::new();
        self.state = State::Closed;
        self.log_buffer = Vec::new();
        self.context = EvaluationContext::default();
        self.public_primary = None;
        self.modules_original.clear();
        self.modules_stdlib.clear();
        self.modules_resolved.clear();
        self.resources.clear();
        self.class_singletons.clear();
        self.builtin_classes.clear();
        self.alias_singletons.clear();
        self.empty_collections = [None; 3];
        self.empty_objects = [None; 3];
        self.resolved_declarations.clear();
        self.type_parent_defaults.clear();
        self.executable_roots.clear();
        self.code_arenas = Vec::new();
        self.declaration_roots_dirty = true;
        self.declaration_roots_lens = (0, 0, 0, 0, 0, 0);
        self.unowned_declaration_roots = Vec::new();
        self.public_operation_entries.clear();
        self.query_cache = Vec::new();
        self.active_type_trackers = 0;
        self.session_root = None;
        self.uncached_module_uris.clear();
        self.cached_module_uris.clear();
        self.last_warm_module = None;
        self.fast_int_cache.clear();
        self.fast_int_captured = Vec::new();
        self.fast_int_vals = Vec::new();
        self.fast_int_locals = Vec::new();
        self.fast_int_calls = Vec::new();
        self.scratch_utf8 = String::new();
        self.gc_threshold = 8192;
        self.last_gc_arenas = 0;
        self.gc_epoch = self.gc_epoch.wrapping_add(1);
        self.heap.release();
        self.program = Program::empty_shared();
    }

    pub fn is_closed(&self) -> bool {
        matches!(self.state, State::Closed)
    }
    pub(crate) fn is_set(&self, value: &Value) -> bool {
        matches!(value, Value::Object(id) if self.heap.get(*id).is_ok_and(|object| object.collection_kind == crate::heap::CollectionKind::Set))
    }

    pub(crate) fn is_list(&self, value: &Value) -> bool {
        matches!(value, Value::Object(id) if self.heap.get(*id).is_ok_and(|object| object.collection_kind == crate::heap::CollectionKind::List))
    }

    pub(crate) fn is_map(&self, value: &Value) -> bool {
        matches!(value, Value::Object(id) if self.heap.get(*id).is_ok_and(|object| object.collection_kind == crate::heap::CollectionKind::Map))
    }

    pub(crate) fn is_reference(&self, value: &Value) -> bool {
        matches!(value,Value::Object(id) if self.heap.get(*id).is_ok_and(|object| object.reference.is_some()))
    }

    pub fn heap_stats(&self) -> HeapStats {
        self.heap.stats()
    }

    pub(crate) fn enable_auto_gc(&mut self) {
        self.auto_gc = true;
    }

    pub(crate) fn set_session_root(&mut self, root: Option<Value>) {
        self.session_root = root;
    }

    pub(crate) fn maybe_auto_gc_between_operations(&mut self) {
        if !self.auto_gc || !self.frames.is_empty() || !self.suspended.is_empty() {
            return;
        }
        const EXTERNAL_GC_THRESHOLD_BYTES: usize = 8 * 1024 * 1024;
        if self.heap.allocations_since_gc() < self.gc_threshold
            && self.heap.external_bytes_since_gc() < EXTERNAL_GC_THRESHOLD_BYTES
            && self.code_arenas.len() < self.last_gc_arenas.saturating_add(64)
            && self.public_operation_entries.len() < 64
        {
            return;
        }
        self.public_primary = None;
        if matches!(self.state, State::Finished(_)) {
            self.state = State::Idle;
        }
        // Keep up to 8 warm query executables for currently cached modules alive across
        // automatic inter-operation GC so hot persistent queries don't constantly recompile.
        let mut keep_roots: Vec<Value> = Vec::new();
        if !self.query_cache.is_empty() && !self.cached_module_uris.is_empty() {
            let mut cached_units: BTreeSet<crate::modules::UnitId> = BTreeSet::new();
            for uri in &self.cached_module_uris {
                if let Some(modules::CachedModule::Value(id)) = self.modules_original.get_ref(uri)
                    && let Ok(obj) = self.heap.get(*id)
                    && let Some(unit) = obj.module_unit
                {
                    cached_units.insert(unit);
                }
            }
            for cached in self.query_cache.iter().rev() {
                if cached_units.contains(&cached.unit) && self.heap.executable_is_live(cached.owner)
                {
                    keep_roots.push(cached.owner.root());
                    if keep_roots.len() >= 8 {
                        break;
                    }
                }
            }
        }
        let stats = self.collect(&keep_roots);
        self.gc_threshold = stats.live.saturating_mul(2).max(8192);
        self.last_gc_arenas = self.code_arenas.len();
    }

    pub(super) fn record_cached_module_uris(&mut self, orig: &PklString, res: &PklString) {
        self.cached_module_uris.insert(orig.clone());
        self.cached_module_uris.insert(res.clone());
    }

    pub(super) fn record_uncached_module_uris(&mut self, orig: &PklString, res: &PklString) {
        const MAX_UNCACHED_ALIASES: usize = 8;
        if !self
            .uncached_module_uris
            .iter()
            .any(|(o, r)| o == orig && r == res)
        {
            self.uncached_module_uris
                .push_back((orig.clone(), res.clone()));
        }
        while self.uncached_module_uris.len() > MAX_UNCACHED_ALIASES {
            if let Some((old_orig, old_res)) = self.uncached_module_uris.pop_front() {
                if !self.cached_module_uris.contains(&old_orig) {
                    self.modules_original.remove(old_orig);
                    self.last_warm_module = None;
                }
                if !self.cached_module_uris.contains(&old_res) {
                    self.modules_resolved.remove(old_res);
                }
            }
        }
    }
    pub fn instructions_executed(&self) -> u64 {
        self.instructions
    }
    pub fn member_evaluations(&self) -> u64 {
        self.member_evaluations
    }

    pub fn member_state(
        &self,
        receiver: ObjectId,
        name: impl Into<PklString>,
    ) -> Result<LazyState, RuntimeError> {
        let name = name.into();
        let key = CacheKey {
            entry: false,
            owner: None,
            name,
            method: false,
        };
        Ok(self
            .heap
            .get(receiver)?
            .cache
            .get(&key)
            .cloned()
            .unwrap_or(LazyState::Unevaluated))
    }

    /// Safepoint collection includes suspended frames and externally pinned results.
    pub fn collect(&mut self, external_roots: &[Value]) -> HeapStats {
        self.gc_epoch = self.gc_epoch.wrapping_add(1);
        let mut roots = Vec::with_capacity(external_roots.len() + 64);
        roots.extend_from_slice(external_roots);
        roots.extend(self.session_root.iter().cloned());
        roots.extend(self.resources.values().filter_map(Clone::clone));
        roots.extend(self.public_primary.map(Value::Object));
        roots.extend(
            self.suspended
                .iter()
                .filter_map(|context| context.public_primary)
                .map(Value::Object),
        );
        roots.extend(
            self.modules_original
                .values()
                .chain(self.modules_resolved.values())
                .chain(self.modules_stdlib.values())
                .filter_map(|module| match module {
                    modules::CachedModule::Value(id) => Some(Value::Object(*id)),
                    _ => None,
                }),
        );
        for module in self
            .modules_original
            .values()
            .chain(self.modules_resolved.values())
            .chain(self.modules_stdlib.values())
        {
            if let modules::CachedModule::Failed(error) = module {
                error.diagnostic_roots(&mut roots);
            }
        }
        roots.extend(self.builtin_classes.values().copied().map(Value::Class));
        if self.declaration_roots_dirty
            || self.declaration_root_counts() != self.declaration_roots_lens
        {
            self.declaration_code_roots(&mut roots);
        } else {
            roots.extend(self.unowned_declaration_roots.iter().cloned());
        }
        for (stack, inputs, frames, state) in self
            .suspended
            .iter()
            .map(|context| {
                (
                    &context.stack,
                    &context.inputs,
                    &context.frames,
                    &context.state,
                )
            })
            .chain(std::iter::once((
                &self.stack,
                &self.inputs,
                &self.frames,
                &self.state,
            )))
        {
            roots.extend(
                stack
                    .iter()
                    .filter(|value| value.heap_id().is_some() || matches!(value, Value::Bytes(_)))
                    .cloned(),
            );
            roots.extend(
                inputs
                    .iter()
                    .filter(|value| value.heap_id().is_some() || matches!(value, Value::Bytes(_)))
                    .cloned(),
            );
            match state {
                State::Finished(Ok(value)) => roots.push(value.clone()),
                State::Finished(Err(error)) => error.diagnostic_roots(&mut roots),
                State::Waiting(PendingRequest::TransformFrame(pending)) => {
                    pending.roots(&mut roots)
                }
                _ => (),
            }
            for frame in frames {
                Self::collect_frame_roots(frame, &mut roots);
            }
        }
        let stats = self.heap.collect(&roots);
        self.retire_unreachable_code();
        stats
    }

    #[cfg_attr(target_arch = "wasm32", inline(never))]
    fn collect_frame_roots(frame: &Frame, roots: &mut Vec<Value>) {
        roots.extend(frame.module_receiver.map(Value::Object));
        roots.extend(
            frame
                .module_load
                .as_ref()
                .and_then(|run| run.receiver)
                .map(Value::Object),
        );
        if let Some(run) = &frame.glob {
            run.roots(roots);
        }
        roots.extend(frame.builders.iter().copied().map(Value::Object));
        for scope in &frame.generator_scopes {
            scope.roots(roots);
        }
        roots.extend(frame.prepared_hashes.iter().flatten().cloned());
        roots.extend(
            frame
                .environment
                .locals
                .iter()
                .filter(|value| value.heap_id().is_some() || matches!(value, Value::Bytes(_)))
                .cloned(),
        );
        roots.extend(frame.environment.this_value.iter().cloned());
        roots.extend(frame.environment.outer_value.iter().cloned());
        roots.extend(frame.environment.custom_this.map(|slot| slot.root()));
        roots.extend(frame.environment.executable_owner.map(|owner| owner.root()));
        roots.extend(frame.environment.code_owner.map(|owner| owner.root()));
        roots.extend(
            frame
                .environment
                .local_owners
                .values()
                .map(|(owner, _)| owner.root()),
        );
        if let Some(run) = &frame.key_search {
            run.roots(roots);
        }
        roots.extend(frame.function_owner.map(Value::Function));
        roots.extend(frame.amendment_arguments.iter().flatten().cloned());
        if let Some(run) = &frame.collection_join {
            run.roots(roots);
        }
        if let Some(run) = &frame.mapping_lookup {
            run.roots(roots);
        }
        if let Some(run) = &frame.analysis {
            run.roots(roots);
        }
        if let Some(run) = &frame.reflection {
            run.roots(roots);
        }
        if let Some(run) = &frame.mapping_keys {
            run.roots(roots);
        }
        if let Some(run) = &frame.lazy_collection_run {
            run.roots(roots);
        }
        if let Some(run) = &frame.int_seq_run {
            run.roots(roots);
        }
        if let Some(run) = &frame.collection_addition {
            run.roots(roots);
        }
        if let Some(run) = &frame.conversion_run {
            run.roots(roots);
        }
        if let Some(run) = &frame.collection_transform {
            run.roots(roots);
        }
        if let Some(run) = &frame.set_order {
            run.roots(roots);
        }
        if let Some(run) = &frame.set_sequence {
            run.roots(roots);
        }
        if let Some(run) = &frame.list_ops {
            run.roots(roots);
        }
        if let Some(run) = &frame.string_transform {
            run.roots(roots);
        }
        if let Some(run) = &frame.yaml_parse {
            run.roots(roots);
        }
        if let Some(run) = &frame.json_parse {
            run.roots(roots);
        }
        if let Some(run) = &frame.pcf_render {
            run.roots(roots);
        }
        if let Some(run) = &frame.deep_force {
            run.roots(roots);
        }
        roots.extend(
            frame
                .method_call_site
                .as_ref()
                .map(|site| site.executable.root()),
        );
        if let Some(warning) = &frame.warning {
            warning.roots(roots);
        }
        if let Some(run) = &frame.equality_run {
            run.roots(roots);
        }
        if let Some(run) = &frame.type_resolution {
            run.roots(roots);
        }
        if let Some(run) = &frame.generic_default {
            run.roots(roots);
        }
        if let Some(run) = &frame.type_run {
            run.roots(roots);
        }
        for scope in &frame.environment.scopes {
            roots.push(Value::Object(scope.receiver));
            roots.push(Value::Object(scope.owner));
        }
        if let Some(force) = &frame.force {
            roots.push(Value::Object(force.receiver));
            roots.push(Value::Object(force.owner));
            roots.extend(force.entry_probe.iter().cloned());
            if let Some(handler) = &force.stolen_cast {
                handler.roots(roots);
            }
            if let Some(copy) = &force.const_receiver {
                roots.push(Value::Object(copy.0));
            }
            for cast in &force.collection_casts {
                typecheck::environment_roots(&cast.environment, roots);
                roots.extend(cast.node.map(|node| node.0.root()));
            }
            roots.extend(
                force
                    .inherited_types
                    .iter()
                    .map(|(_, _, _, owner)| Value::Object(*owner)),
            );
        }
    }

    fn fail(&mut self, mut error: RuntimeError) {
        // Capture the original activations before catches, header relocation or
        // diagnostic filtering. A cached/rethrown cause retains its first origin.
        if error
            .foreign_exception
            .as_ref()
            .is_some_and(|cause| cause.origin.is_none())
        {
            use crate::java_exception::provenance::{ForeignActivation, ForeignOrigin};
            let mut activations: Vec<ForeignActivation> = self
                .frames
                .iter()
                .map(|frame| {
                    let metadata = self.program.metadata.get(frame.block);
                    let instruction_index = frame.pc.checked_sub(1);
                    ForeignActivation {
                        instruction_index,
                        member: metadata.map(|metadata| metadata.member.clone()),
                        source: metadata.and_then(|metadata| metadata.source.clone()),
                        instruction_span: metadata.and_then(|metadata| {
                            instruction_index
                                .and_then(|index| metadata.spans.get(index).copied().flatten())
                        }),
                        declaration_span: metadata.and_then(|metadata| metadata.declaration),
                        method_signature: metadata
                            .and_then(|metadata| metadata.method_signature.clone()),
                        active_let_frames: metadata.map_or_else(Vec::new, |metadata| {
                            metadata
                                .let_scopes
                                .iter()
                                .filter(|scope| {
                                    instruction_index
                                        .is_some_and(|index| scope.body.contains(&index))
                                })
                                .map(|scope| scope.frame.clone())
                                .collect()
                        }),
                        lambda: frame.lambda,
                        inline_constraint: metadata
                            .is_some_and(|metadata| metadata.inline_constraint),
                        anonymous_root: metadata.is_some_and(|metadata| metadata.anonymous_root),
                    }
                })
                .collect();
            activations.extend(error.frames.iter().rev().map(|frame| ForeignActivation {
                instruction_index: None,
                member: Some(frame.member.clone()),
                source: frame.source.clone(),
                instruction_span: None,
                declaration_span: Some(frame.span),
                method_signature: None,
                active_let_frames: Vec::new(),
                lambda: false,
                inline_constraint: false,
                anonymous_root: frame.anonymous,
            }));
            let origin = ForeignOrigin {
                activations,
                primary_source: self
                    .public_primary_source()
                    .or_else(|| self.context.primary_source.clone()),
                power_assertions: self.power_assertions,
                frame_transformer: self.context.frame_transformer,
                platform_metadata: self.platform_metadata.clone(),
                original_span: error.span,
                existing_guest_frames: error.frames.clone(),
            };
            Arc::make_mut(error.foreign_exception.as_mut().expect("foreign cause")).origin =
                Some(origin);
        }
        if error.kind == ErrorKind::MissingType
            && error.type_lookup_module.is_none()
            && let Some(frame) = self.frames.last()
            && let Ok(owner) = self.declaring_module(&frame.environment, false)
            && let Ok(module) = self.heap.get(owner)
            && let Some(unit) = module
                .module_unit
                .and_then(|id| self.program.units.get(id.0 as usize))
        {
            error.type_lookup_module = Some(unit.name.clone());
        }
        if error.span.is_none()
            && let Some(frame) = self.frames.last()
            && let Some(metadata) = self.program.metadata.get(frame.block)
            && let Some(parent_span) = metadata
                .spans
                .get(frame.pc.saturating_sub(1))
                .copied()
                .flatten()
            && (metadata.amendment_parent_span == Some(parent_span)
                || metadata.inferred_entry_parent_spans.contains(&parent_span))
            && matches!(
                self.program.blocks[frame.block].get(frame.pc.saturating_sub(1)),
                Some(Instruction::BeginCollection { .. } | Instruction::AmendEmpty)
            )
        {
            error.unavailable_origin = true;
        }
        let cast_boundary = self.mapping_cast_boundary(&error);
        let yaml_boundary = self.yaml_constructor_boundary(&error);
        let catch_boundary = self
            .guest_catch_boundary(&error)
            .max(cast_boundary)
            .max(yaml_boundary);
        if error.undefined_value && self.catch_force_undefined(catch_boundary) {
            return;
        }
        // Undefined() has no property receiver in VmUndefinedValueException;
        // only undefined-property failures can acquire a renderer leaf hint.
        if error.undefined_value && !error.undefined_receiverless && error.hint.is_none() {
            error.hint = self.pcf_undefined_hint();
        }
        // Logger callbacks throw outside the language exception hierarchy.
        // PklRootNode catches that foreign exception; MemberNode's override
        // attaches its header section before ordinary guest stack unwinding.
        // Truffle helper continuations (including trace forcing) are not roots.
        let callback_root = if error.frames.is_empty()
            && matches!(
                error.callback_origin(),
                Some(
                    crate::logging::CallbackOrigin::LogTransformer
                        | crate::logging::CallbackOrigin::LogDelegate
                )
            ) {
            self.frames
                .iter()
                .enumerate()
                .rev()
                .find_map(|(index, frame)| {
                    let metadata = self.program.metadata.get(frame.block)?;
                    (!metadata.inline_constraint)
                        .then_some(metadata.declaration)
                        .flatten()
                        .map(|span| (index, span))
                })
        } else {
            None
        };
        if let Some((_, span)) = callback_root {
            error.span = Some(span);
        }
        // Source absence belongs to the original throwing frame. If an
        // inferred-parent boundary replaces this error and shortens the visible
        // stack, that boundary's authored origin must remain visible.
        let unavailable_origin = error.unavailable_origin;
        if unavailable_origin {
            // Consume this origin once: cached exceptions already contain the
            // visible stack. Unwinding and inserted let frames are unaffected.
            error.unavailable_origin = false;
        }
        let unwind_start = catch_boundary.map_or(0, |index| index + 1);
        // The internal boundary retains its frame for findMember, but abandons
        // that frame's cast work, including any module/force subcontinuation.
        let cleanup_start = catch_boundary
            .filter(|index| Some(*index) == cast_boundary || Some(*index) == yaml_boundary)
            .unwrap_or(unwind_start);
        if guest_exceptions::is_guest_exception(&error) && !error.hint_from_template {
            match self.document_conversion_hint(unwind_start) {
                Ok(Some(hint)) => error.hint = Some(hint),
                Ok(None) => (),
                // Guest catch renders message only; reference-sensitive hints
                // are observed only at the uncaught diagnostic boundary.
                Err(boundary) if catch_boundary.is_none() => {
                    error.kind = boundary.kind;
                    error.message = boundary
                        .message
                        .concat(&"\nOriginal failure: ".into())
                        .concat(&error.message);
                    error.hint = None;
                    error.type_diagnostic = None;
                    error.frontend = None;
                }
                Err(_) => (),
            }
        }
        self.reset_force_marks(cleanup_start);
        // Inferred properties read their class prototype without type checks.
        // An undefined prototype value is caught by the inference node, whose
        // authored new expression becomes the error origin. Keep all frames for
        // pending-cache cleanup even though the caught read is not diagnostic.
        let instruction = |frame: &Frame| {
            self.program
                .blocks
                .get(frame.block)
                .and_then(|code| code.get(frame.pc.saturating_sub(1)))
        };
        let diagnostic_count = if let Some((index, _)) = callback_root {
            index + 1
        } else if error.undefined_value {
            // InferParentWithinPropertyNode catches every undefined-value
            // exception from its prototype read, including Undefined().
            self.frames[unwind_start..]
                .iter()
                .rposition(|frame| {
                    matches!(instruction(frame), Some(Instruction::InferPropertyParent))
                })
                .map(|index| {
                    error = cannot_infer_parent();
                    unwind_start + index + 1
                })
                .unwrap_or(self.frames.len())
        } else {
            self.frames.len()
        };
        let diagnostic_frames = if error.cached_module_frames {
            &self.frames[..0]
        } else {
            &self.frames[..diagnostic_count]
        };
        // LetExprNode inserts one frame per call target when a body throws.
        // Unwinding replaces inner scopes with the outermost active scope,
        // including across recursive calls. Initializers and type checks are
        // outside the body range and therefore do not insert a frame.
        let logical_frames = &self.frames[..diagnostic_count];
        let mut let_frames = BTreeMap::new();
        let mut cast_let_frames: Vec<(TypeNodeId, RuntimeFrame)> = Vec::new();
        let mut cast_targets = Vec::with_capacity(self.frames.len());
        let mut logical_targets = Vec::with_capacity(self.frames.len());
        for frame in logical_frames {
            let target = if frame.return_to_type
                && self
                    .program
                    .metadata
                    .get(frame.block)
                    .is_some_and(|m| m.inline_constraint)
            {
                logical_targets.last().copied().unwrap_or(frame.block)
            } else {
                frame.block
            };
            let cast_target = if frame.return_to_type
                && self
                    .program
                    .metadata
                    .get(frame.block)
                    .is_some_and(|m| m.inline_constraint)
            {
                cast_targets.last().copied().flatten()
            } else {
                frame
                    .force
                    .as_ref()
                    .and_then(|force| force.stolen_cast.as_ref())
                    .and_then(|handler| handler.active_target())
            };
            cast_targets.push(cast_target);
            logical_targets.push(target);
        }
        for (index, frame) in logical_frames[unwind_start.min(logical_frames.len())..]
            .iter()
            .rev()
            .enumerate()
        {
            // A recoverable mismatch becomes VmException only at PklRootNode.
            // BytesLiteralNode converts its element mismatch before returning,
            // so an enclosing let does catch that exception, like a type cast.
            if index == 0
                && (callback_root.is_some()
                    || error.type_diagnostic.is_some()
                        && !matches!(instruction(frame), Some(Instruction::CheckByte))
                        && !self
                            .program
                            .metadata
                            .get(frame.block)
                            .is_some_and(|metadata| {
                                metadata.type_casts.contains(&frame.pc.saturating_sub(1))
                            }))
            {
                continue;
            }
            if let Some(scope) = self
                .program
                .metadata
                .get(frame.block)
                .filter(|metadata| {
                    !metadata.inline_constraint
                        || cast_targets[logical_frames.len() - 1 - index].is_some()
                })
                .and_then(|metadata| {
                    metadata
                        .let_scopes
                        .iter()
                        .rev()
                        .find(|scope| scope.body.contains(&frame.pc.saturating_sub(1)))
                })
            {
                if let Some(target) = cast_targets[logical_frames.len() - 1 - index] {
                    if let Some((_, previous)) =
                        cast_let_frames.iter_mut().find(|(old, _)| *old == target)
                    {
                        *previous = scope.frame.clone();
                    } else {
                        cast_let_frames.push((target, scope.frame.clone()));
                    }
                } else {
                    let_frames.insert(
                        logical_targets[logical_frames.len() - 1 - index],
                        scope.frame.clone(),
                    );
                }
            }
        }
        if error.kind == ErrorKind::MissingMember
            && let Some(frame) = self.frames.last()
            && let Some(
                Instruction::GetProperty { name, .. }
                | Instruction::GetAmbiguousProperty { name, .. }
                | Instruction::GetImplicitProperty(name)
                | Instruction::GetImplicitCustomProperty(name),
            ) = self
                .program
                .blocks
                .get(frame.block)
                .and_then(|code| code.get(frame.pc.saturating_sub(1)))
        {
            error.message = PklString::from("Cannot find property `")
                .concat(name)
                .concat(&"`.".into());
            error.hint = self.implicit_member_suggestions(frame, name, -1);
        }
        if error.kind == ErrorKind::MissingMember
            && let Some(frame) = self.frames.last()
            && let Some(
                Instruction::GetMethodProperty { name, .. }
                | Instruction::GetImplicitMethod(name)
                | Instruction::GetImplicitCustomMethod(name),
            ) = self
                .program
                .blocks
                .get(frame.block)
                .and_then(|code| code.get(frame.pc.saturating_sub(1)))
        {
            error.message = PklString::from("Cannot find method `")
                .concat(name)
                .concat(&"`.".into());
            let arity = self
                .program
                .metadata
                .get(frame.block)
                .and_then(|metadata| metadata.method_arities.get(&frame.pc.saturating_sub(1)))
                .copied()
                .unwrap_or(0);
            error.hint = self.implicit_member_suggestions(frame, name, arity as isize);
        }
        for (frame_index, frame) in diagnostic_frames
            .iter()
            .enumerate()
            .skip(unwind_start)
            .rev()
        {
            if matches!(
                instruction(frame),
                Some(Instruction::InvokeAmendedParent | Instruction::ContinueFunctionAmendment)
            ) && let Some(owner) = frame.function_owner
                && let Ok(object) = self.heap.get(owner)
                && let Some(origin) = object
                    .amendment
                    .as_ref()
                    .and_then(|amendment| amendment.origin.clone())
            {
                error.frames.push(origin);
            }
            if let Some(metadata) = self.program.metadata.get(frame.block)
                && let Some(span) = callback_root
                    .as_ref()
                    .filter(|(index, _)| *index == frame_index)
                    .map(|(_, span)| span)
                    .or_else(|| {
                        metadata
                            .spans
                            .get(frame.pc.saturating_sub(1))
                            .and_then(Option::as_ref)
                    })
            {
                let reading_inferred_parent = frame_index + 1 < diagnostic_frames.len()
                    && matches!(instruction(frame), Some(Instruction::InferPropertyParent));
                // Both BytesLiteralNode and LetExprNode insert a frame under
                // the same call target; the enclosing let replaces the byte
                // argument's insertion while retaining the UInt8 type frame.
                let replaced_byte_argument =
                    matches!(instruction(frame), Some(Instruction::CheckByte))
                        && (let_frames.contains_key(&logical_targets[frame_index])
                            || cast_targets[frame_index].is_some_and(|target| {
                                cast_let_frames.iter().any(|(old, _)| *old == target)
                            }));
                if (!frame.hides_callsite()
                    || callback_root.is_some_and(|(index, _)| index == frame_index))
                    && !diagnostic_frames
                        .get(frame_index + 1)
                        .is_some_and(|child| self.object_method_signature_frame(child))
                    && !reading_inferred_parent
                    && !replaced_byte_argument
                    && !(unavailable_origin && frame_index + 1 == self.frames.len())
                {
                    let mut displayed = RuntimeFrame {
                        anonymous: metadata.anonymous_root,
                        source: metadata.source.clone(),
                        span: if error.frames.is_empty() {
                            error.span.unwrap_or(*span)
                        } else {
                            frame
                                .type_resolution
                                .as_ref()
                                .map_or(*span, |run| run.span(&self.program))
                        },
                        member: frame
                            .diagnostic_member
                            .clone()
                            .unwrap_or_else(|| metadata.member.clone()),
                    };
                    // Only the inline constraint body executed by the nearest
                    // Protobuf alternative shares the renderer's logical root.
                    // Ordinary checks and called functions retain their roots.
                    if frame.return_to_type
                        && metadata.inline_constraint
                        && metadata.constraint_root.is_none()
                        && let Some(renderer) = diagnostic_frames[..frame_index]
                            .iter()
                            .rev()
                            .find(|parent| parent.type_run.is_some())
                            .filter(|parent| {
                                parent
                                    .type_run
                                    .as_ref()
                                    .is_some_and(|run| run.is_protobuf_alternative())
                            })
                            .and_then(|parent| self.program.metadata.get(parent.block))
                    {
                        displayed = self.protobuf_type_origin(displayed, renderer);
                    }
                    if matches!(instruction(frame), Some(Instruction::CheckByte))
                        && let Some(target) = cast_targets[frame_index]
                    {
                        if let Err(invalid) = error.insert_cast_frame(target, displayed) {
                            error = invalid;
                        }
                    } else {
                        error.frames.push(displayed);
                    }
                }
                // Object-local checks run inside LocalTypedPropertyNode.
                // Module/class declarations use a separate PropertyTypeNode
                // call even when local, adding the authored value frame.
                if frame.force.is_some()
                    && !metadata.inline_property_typecheck
                    && metadata.property_typecheck == Some(frame.pc.saturating_sub(1))
                    && matches!(
                        self.program.blocks[frame.block].get(frame.pc.saturating_sub(1)),
                        Some(Instruction::CheckType(_))
                    )
                    && let Some(value_span) = metadata.value_span
                {
                    error.frames.push(RuntimeFrame {
                        anonymous: false,
                        source: metadata.source.clone(),
                        span: value_span,
                        member: metadata.member.clone(),
                    });
                }
            }
            if let Some(run) = &frame.type_run {
                error.frames.extend(run.fatal_value_frames().cloned());
            }
            if let Some(target) = cast_targets[frame_index]
                && let Some(index) = cast_let_frames.iter().position(|(old, _)| *old == target)
            {
                let (_, inserted) = cast_let_frames.remove(index);
                if let Err(invalid) = error.insert_cast_frame(target, inserted) {
                    error = invalid;
                }
            }
            // A slot-writing constraint has its own shared Java call target.
            // Mapping cast catches unwind it before an outer invocation of the
            // same root catches the exception. Reserve its first stack position
            // even when this invocation has no active let body; a later LetExpr
            // insertion belongs to that first occurrence, not the outer frame.
            let constraint_target = self
                .program
                .metadata
                .get(frame.block)
                .filter(|metadata| metadata.constraint_root.is_some())
                .and(frame.environment.executable_owner)
                .map(TypeNodeId);
            if let Some(target) = constraint_target {
                error.reserve_cast_target(target);
            }
            if let Some(inserted) = let_frames.remove(&logical_targets[frame_index]) {
                if let Some(target) = constraint_target {
                    if let Err(invalid) = error.insert_cast_frame(target, inserted) {
                        error = invalid;
                    }
                } else {
                    error.frames.push(inserted);
                }
            }
            if frame.return_to_type
                && let Some(span) = self
                    .program
                    .metadata
                    .get(frame.block)
                    .and_then(|m| m.constraint_root)
                && let Some(caller) = frame_index
                    .checked_sub(1)
                    .and_then(|index| self.frames.get(index))
                && let Some(run) = &caller.type_run
            {
                error.frames.push(RuntimeFrame {
                    anonymous: false,
                    source: None,
                    span,
                    member: run.member.clone(),
                });
            }
        }
        if error.cached_module_frames {
            // Java rethrows the cached exception without recapturing its stack,
            // but active LetExpr catches still mutate the same insertion map.
            for (target, inserted) in cast_let_frames {
                if let Err(invalid) = error.insert_cast_frame(target, inserted) {
                    error = invalid;
                }
            }
        }
        // Callback failures expose callers in suspended contexts without
        // unwinding those contexts or changing their force/cache ownership.
        if !error.cached_module_frames && catch_boundary.is_none() {
            for context in self.suspended.iter().rev() {
                error
                    .frames
                    .extend(context.diagnostic_callers.iter().cloned());
            }
        }
        if error.span.is_none() {
            error.span = error.frames.first().map(|frame| frame.span);
        }
        for frame in &self.frames[cleanup_start..] {
            if let Some(force) = &frame.force
                && let Some(key) = &force.cache
                && !self
                    .suspended
                    .iter()
                    .flat_map(|context| &context.frames)
                    .chain(self.frames[..cleanup_start].iter())
                    .any(|frame| {
                        frame.force.as_ref().is_some_and(|other| {
                            other.receiver == force.receiver && other.cache.as_ref() == Some(key)
                        })
                    })
                && let Ok(object) = self.heap.get_mut(force.receiver)
            {
                // VmUtils.doReadMember and ReadLocalPropertyNode publish only successful
                // return values. A failed attempt must be retryable in this evaluator.
                if matches!(object.cache.get(key), Some(LazyState::Evaluating)) {
                    object.cache.remove(key);
                }
            }
        }
        // Truffle retains the same source/initialization exception in every
        // module-cache alias. Its lazy stack keeps unwinding across guest catches;
        // the first uncaught rendering freezes the completed stack.
        if error.module_failure_id.is_some() {
            if !error.cached_module_frames {
                error.cached_module_frames = catch_boundary.is_none();
            }
            // A rethrow may replace an insertion on an already captured cast
            // target. Propagate that mutation to every alias without recapturing
            // caller frames or changing the exception's failure identity.
            if let Some(key) = &error.module_failure_id {
                for cached in self
                    .modules_original
                    .values_mut()
                    .chain(self.modules_resolved.values_mut())
                    .chain(self.modules_stdlib.values_mut())
                {
                    if matches!(cached, modules::CachedModule::Failed(previous) if previous.module_failure_id.as_ref() == Some(key))
                    {
                        *cached = modules::CachedModule::Failed(error.clone());
                    }
                }
            }
        }
        let mut failed_uncached = Vec::new();
        for frame in &self.frames[cleanup_start..] {
            if let Some(receiver) = frame.module_receiver
                && self
                    .heap
                    .get(receiver)
                    .is_ok_and(|module| module.module_initializing)
                && let Some(unit) = self
                    .program
                    .metadata
                    .get(frame.block)
                    .and_then(|metadata| metadata.unit)
                    .and_then(|unit| self.program.units.get(unit.0 as usize))
                && unit.entry == frame.block
                && let Some(identity) = &unit.identity
            {
                if error.module_failure_id.is_none() {
                    error.module_failure_id = Some(modules::next_failure_id());
                    error.cached_module_frames = catch_boundary.is_none();
                }
                if let Some(key) = unit.singleton_cache_key() {
                    self.modules_stdlib
                        .insert(key.into(), modules::CachedModule::Failed(error.clone()));
                } else {
                    self.modules_original.insert(
                        identity.original_uri.clone(),
                        modules::CachedModule::Failed(error.clone()),
                    );
                    self.last_warm_module = None;
                    self.modules_resolved.insert(
                        identity.resolved_uri.clone(),
                        modules::CachedModule::Failed(error.clone()),
                    );
                }
            }
            if let Some(run) = &frame.module_load
                && run.initializing
                && let Some(resolved) = &run.resolved
            {
                if error.module_failure_id.is_none() {
                    error.module_failure_id = Some(modules::next_failure_id());
                    error.cached_module_frames = catch_boundary.is_none();
                }
                if run.is_singleton() {
                    self.modules_stdlib.insert(
                        resolved.original_uri.clone(),
                        modules::CachedModule::Failed(error.clone()),
                    );
                } else {
                    self.modules_original.insert(
                        resolved.original_uri.clone(),
                        modules::CachedModule::Failed(error.clone()),
                    );
                    self.last_warm_module = None;
                    self.modules_resolved.insert(
                        resolved.resolved_uri.clone(),
                        modules::CachedModule::Failed(error.clone()),
                    );
                    if !run.cached {
                        failed_uncached
                            .push((resolved.original_uri.clone(), resolved.resolved_uri.clone()));
                    }
                }
            }
        }
        for (orig, res) in failed_uncached {
            self.record_uncached_module_uris(&orig, &res);
        }
        if let Some(index) = catch_boundary.filter(|index| Some(*index) == yaml_boundary) {
            self.catch_yaml_constructor(index, error);
            self.recompute_active_type_trackers();
        } else if let Some(index) = catch_boundary.filter(|index| Some(*index) == cast_boundary) {
            self.catch_mapping_cast(index, error);
            self.recompute_active_type_trackers();
        } else if let Some(index) = catch_boundary {
            let value = self.render_caught_exception(&error);
            let handler = self.frames[index]
                .guest_catch
                .take()
                .expect("catch boundary");
            self.frames.truncate(index + 1);
            self.stack.truncate(handler.stack);
            self.stack.push(Value::String(value));
            self.recompute_active_type_trackers();
            self.state = State::Running;
        } else {
            self.frames.clear();
            self.active_type_trackers = 0;
            self.stack.clear();
            self.inputs.clear();
            self.finish_error(error);
        }
    }

    fn finish_top(&mut self) -> Result<(), RuntimeError> {
        let value = self.pop()?;
        if !self.stack.is_empty() {
            return Err(invalid("unbalanced top-level stack"));
        }
        self.inputs.clear();
        self.state = State::Finished(Ok(value));
        Ok(())
    }

    fn pop(&mut self) -> Result<Value, RuntimeError> {
        let base = self.frames.last().map_or(0, |frame| frame.base);
        if self.stack.len() <= base {
            return Err(invalid("operand stack underflow"));
        }
        self.stack
            .pop()
            .ok_or_else(|| invalid("operand stack underflow"))
    }

    fn scope(&self, depth: usize) -> Result<Scope, RuntimeError> {
        self.frames
            .last()
            .and_then(|frame| frame.environment.scopes.get(depth))
            .copied()
            .ok_or_else(|| invalid("invalid object scope"))
    }

    fn this_receiver(&self) -> Result<Value, RuntimeError> {
        match self
            .frames
            .last()
            .and_then(|frame| frame.environment.this_value.clone())
        {
            Some(receiver) => Ok(receiver),
            None => Ok(Value::Object(self.scope(0)?.receiver)),
        }
    }

    fn force(
        &mut self,
        receiver: ObjectId,
        initial_owner: ObjectId,
        name: PklString,
        local: bool,
        memoize: bool,
        method: bool,
    ) -> Result<(), RuntimeError> {
        self.force_key(receiver, initial_owner, name, local, memoize, method, false)
    }

    #[allow(clippy::too_many_arguments)]
    fn force_key(
        &mut self,
        receiver: ObjectId,
        initial_owner: ObjectId,
        name: PklString,
        local: bool,
        memoize: bool,
        method: bool,
        entry: bool,
    ) -> Result<(), RuntimeError> {
        self.force_key_mode(
            receiver,
            initial_owner,
            name,
            local,
            memoize,
            method,
            entry,
            false,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn force_key_mode(
        &mut self,
        receiver: ObjectId,
        initial_owner: ObjectId,
        name: PklString,
        local: bool,
        memoize: bool,
        method: bool,
        entry: bool,
        predicate: bool,
    ) -> Result<(), RuntimeError> {
        self.force_key_mode_prepared(
            receiver,
            initial_owner,
            name,
            local,
            memoize,
            method,
            entry,
            predicate,
            false,
            None,
        )
    }

    /// Publish a completed owner result without enlarging the ordinary VM
    /// instruction dispatch stack frame with the rare receiver-copy payload.
    #[inline(never)]
    fn publish_const_receiver(&mut self, value: &Value) -> Result<bool, RuntimeError> {
        let copy = self
            .frames
            .last_mut()
            .and_then(|frame| frame.force.as_mut())
            .and_then(|force| force.const_receiver.take());
        let Some(copy) = copy else {
            return Ok(false);
        };
        let (receiver, key) = *copy;
        self.complete_cache(receiver, key, value.clone())?;
        self.stack.push(value.clone());
        self.frames.last_mut().expect("active frame").pc -= 1;
        Ok(true)
    }

    // Keep the rare const forwarding locals out of the ordinary member-force
    // stack frame: deep source recovery is required to fit a 256 KiB stack.
    #[inline(never)]
    fn force_const_owner(
        &mut self,
        receiver: ObjectId,
        owner: ObjectId,
        name: PklString,
        key: CacheKey,
        member: Member,
        predicate: bool,
    ) -> Result<(), RuntimeError> {
        let previous_frames = self.frames.len();
        self.force_key_mode_prepared(
            owner,
            owner,
            name,
            false,
            true,
            false,
            false,
            predicate,
            true,
            Some(member),
        )?;
        if self.frames.len() == previous_frames {
            let value = self
                .stack
                .last()
                .cloned()
                .ok_or_else(|| invalid("missing const owner value"))?;
            self.complete_cache(receiver, key, value)?;
        } else {
            let force = self
                .frames
                .last_mut()
                .and_then(|frame| frame.force.as_mut())
                .ok_or_else(|| invalid("missing const owner force"))?;
            // The owner is the sole pending cache attempt. In particular,
            // failure/cancellation must not poison a child's cache, and a
            // reentrant callback must still use the shared owner attempt.
            force.const_receiver = Some(Box::new((receiver, key)));
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn force_key_mode_prepared(
        &mut self,
        receiver: ObjectId,
        initial_owner: ObjectId,
        name: PklString,
        local: bool,
        memoize: bool,
        method: bool,
        entry: bool,
        predicate: bool,
        hash_prepared: bool,
        resolved_member: Option<Member>,
    ) -> Result<(), RuntimeError> {
        self.force_key_mode_probe(
            receiver,
            initial_owner,
            name,
            local,
            memoize,
            method,
            entry,
            predicate,
            hash_prepared,
            resolved_member,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn force_key_probe(
        &mut self,
        receiver: ObjectId,
        owner: ObjectId,
        name: PklString,
        memoize: bool,
        probe: Value,
    ) -> Result<(), RuntimeError> {
        self.force_key_mode_probe(
            receiver,
            owner,
            name,
            false,
            memoize,
            false,
            true,
            false,
            false,
            None,
            Some(probe),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn force_key_mode_probe(
        &mut self,
        receiver: ObjectId,
        initial_owner: ObjectId,
        name: PklString,
        local: bool,
        memoize: bool,
        method: bool,
        entry: bool,
        predicate: bool,
        hash_prepared: bool,
        resolved_member: Option<Member>,
        entry_probe: Option<Value>,
    ) -> Result<(), RuntimeError> {
        self.force_key_mode_value(
            receiver,
            initial_owner,
            name,
            local,
            memoize,
            method,
            entry,
            predicate,
            hash_prepared,
            resolved_member,
            entry_probe,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn force_key_mode_value(
        &mut self,
        receiver: ObjectId,
        initial_owner: ObjectId,
        name: PklString,
        local: bool,
        mut memoize: bool,
        method: bool,
        entry: bool,
        predicate: bool,
        hash_prepared: bool,
        resolved_member: Option<Member>,
        entry_probe: Option<Value>,
        stolen_value: Option<Value>,
    ) -> Result<(), RuntimeError> {
        if entry
            && !method
            && !local
            && let Some(elems) = &self.heap.get(receiver)?.list_elements
        {
            let idx = if let Some(Value::Int(index)) = &entry_probe {
                (index.get() >= 0).then_some(index.get() as usize)
            } else if let Some(name_str) = name.as_str()
                && let Some(rest) = name_str.strip_prefix('\0')
                && let Some((_, idx_str)) = rest.split_once(':')
            {
                idx_str.parse::<usize>().ok()
            } else {
                None
            };
            if let Some(idx) = idx
                && let Some(val) = elems.get(idx)
            {
                self.stack.push(val.clone());
                return Ok(());
            }
        }
        let entry_probe = if entry && entry_probe.is_none() {
            collections::cache_entry_key(
                &self.heap,
                initial_owner,
                &CacheKey {
                    entry,
                    owner: local.then_some(initial_owner),
                    name: name.clone(),
                    method,
                },
            )?
        } else {
            entry_probe
        };
        if !hash_prepared
            && self.prepare_cache_read(
                receiver,
                initial_owner,
                name.clone(),
                local,
                memoize,
                method,
                entry,
                predicate,
                entry_probe.clone(),
            )?
        {
            return Ok(());
        }
        let key = CacheKey {
            entry,
            owner: local.then_some(initial_owner),
            name: name.clone(),
            method,
        };
        if memoize
            && !entry
            && let Some(value) = self.cached_slot(receiver, &key)?
        {
            self.stack.push(value);
            return Ok(());
        }
        if memoize
            && !local
            && !method
            && !entry
            && self.steal_collection_property(receiver, &name)?
        {
            return Ok(());
        }
        let mut owner = initial_owner;
        let mut visited = BTreeSet::new();
        let member = if let Some(member) = resolved_member {
            member
        } else {
            loop {
                if !visited.insert(owner) {
                    return Err(RuntimeError::new(ErrorKind::Cycle, "cyclic parent chain"));
                }
                let object = self.heap.get(owner)?;
                if let Some(member) = object.members.iter().find(|member| {
                    member.name == name
                        && member.local == local
                        && member.method == method
                        && member.entry == entry
                }) {
                    break member.clone();
                }
                if local {
                    return Err(RuntimeError::new(
                        ErrorKind::MissingMember,
                        PklString::from("missing local member ")
                            .concat(&name)
                            .concat(&"".into()),
                    ));
                }
                owner = match object.parent {
                    Some(parent) => parent,
                    None if method
                        && name == "ifNonNull"
                        && (object.class_definition.is_none()
                            || object.class_superclass_initialized) =>
                    {
                        self.any_method(Value::Object(receiver));
                        return Ok(());
                    }
                    None if method && name == "getClass" && object.class_superclass_initialized => {
                        let function =
                            self.native_reference_method(Value::Object(receiver), "getClass");
                        self.stack.push(function);
                        return Ok(());
                    }
                    None if method
                        && name == "toString"
                        && (object.class_definition.is_none()
                            || object.class_superclass_initialized) =>
                    {
                        let function =
                            self.native_reference_method(Value::Object(receiver), "objectToString");
                        self.stack.push(function);
                        return Ok(());
                    }
                    None => {
                        if !method
                            && !entry
                            && let Some(member) =
                                self.collection_cached_property(receiver, &name)?
                        {
                            owner = receiver;
                            break member;
                        }
                        if method && self.conversion_method(Value::Object(receiver), &name)? {
                            return Ok(());
                        }
                        if !method
                            && !entry
                            && name == "default"
                            && self.object_default_member(receiver)?
                        {
                            return Ok(());
                        }
                        // A super method searches from its owner's parent while
                        // retaining the actual receiver for successful invocation.
                        // The failed lookup describes that search origin too.
                        let diagnostic_receiver = if method { initial_owner } else { receiver };
                        return Err(self.missing_member(diagnostic_receiver, &name, method)?);
                    }
                };
            }
        };
        // VmUtils.doReadMember acquires inherited const properties from their
        // prototype owner, including the owner's type check. Local properties
        // and uncached super-property reads have separate upstream semantics.
        if memoize && !local && !method && !entry && member.constant && owner != receiver {
            return self.force_const_owner(receiver, owner, name, key, member, predicate);
        }
        // Only direct class declaration call targets bypass ordinary lazy cycles.
        // Other reads execute BeginClass, whose early return still publishes the
        // successful ordinary member cache through the normal return machinery.
        let same_attempt = |frame: &Frame| {
            frame.force.as_ref().is_some_and(|force| {
                force.entry == entry
                    && force.receiver == receiver
                    && force.owner == owner
                    && force.name == name
                    && force.method == method
            })
        };
        let active_here = self.frames.iter().any(|f| same_attempt(f));
        let suspended_attempt = self
            .suspended
            .iter()
            .flat_map(|context| &context.frames)
            .any(|f| same_attempt(f));
        let active = active_here || suspended_attempt;
        // A Host callback owns a distinct evaluation attempt. It can publish a
        // value before its suspended caller returns; ordinary cycles in this
        // context still fail, and no cache snapshot is restored on callback exit.
        let callback_attempt = !active_here && suspended_attempt;
        if active
            && let Some(Instruction::BeginClass { template, .. }) = self
                .program
                .blocks
                .get(member.code)
                .and_then(|block| block.first())
            && let Some(id) = self.class_singletons.get(template).copied()
        {
            self.stack.push(Value::Class(id));
            return Ok(());
        }
        if active
            && let Some(Instruction::BeginTypeAlias { declaration, .. }) = self
                .program
                .blocks
                .get(member.code)
                .and_then(|block| block.first())
            && let Some(id) = self.alias_singletons.get(declaration).copied()
        {
            self.stack.push(Value::TypeAlias(id));
            return Ok(());
        }
        if active_here
            && local
            && member.ty.is_none()
            && matches!(
                self.program.blocks.get(member.code).map(Vec::as_slice),
                Some(
                    [Instruction::ImportModule(_), Instruction::Return]
                        | [
                            Instruction::ImportModule(_),
                            Instruction::Observe(_),
                            Instruction::Return,
                        ]
                )
            )
            && let Some(id) = self.active_cached_import_receiver(receiver, owner, &name)
        {
            self.stack.push(Value::Object(id));
            return Ok(());
        }
        let direct_force_read = self
            .frames
            .last()
            .and_then(|frame| frame.deep_force.as_ref())
            .is_some_and(|run| run.reads(receiver, owner, &name, entry));
        if memoize
            && stolen_value.is_none()
            && !direct_force_read
            && !callback_attempt
            && matches!(
                self.heap.get(receiver)?.cache.get(&key),
                Some(LazyState::Evaluating)
            )
        {
            return Err(RuntimeError::new(
                ErrorKind::Cycle,
                "A stack overflow occurred.",
            ));
        }
        if local
            && self
                .program
                .metadata
                .get(member.code)
                .is_some_and(|metadata| metadata.constant_member)
        {
            memoize = false;
        }
        if stolen_value.is_none() && active && !direct_force_read && !callback_attempt {
            return Err(RuntimeError::new(
                ErrorKind::Cycle,
                "A stack overflow occurred.",
            ));
        }
        let mut collection_casts = Vec::new();
        let mut constant_value = stolen_value.clone();
        if stolen_value.is_some() {
            // getCachedValue already selected this exact parent value. An empty
            // wrapper applies only its own cast and never relooks up/reexecutes it.
            collection_casts.extend(self.heap.get(receiver)?.collection_cast.clone());
        } else if memoize && entry && !local && !method && !predicate {
            let mut current = receiver;
            let mut cached = None;
            loop {
                let object = self.heap.get(current)?;
                if current != receiver
                    && let Some(value) =
                        self.cached_slot_probe(current, &key, entry_probe.as_ref())?
                {
                    cached = Some(value);
                    break;
                }
                if let Some(cast) = &object.collection_cast {
                    collection_casts.push(cast.clone());
                }
                if current == owner {
                    break;
                }
                let Some(parent) = object.parent else {
                    break;
                };
                if !object.members.is_empty() {
                    // Amendments change the receiver. Cached ancestor values cannot be reused.
                    current = parent;
                    while current != owner {
                        let object = self.heap.get(current)?;
                        if let Some(cast) = &object.collection_cast {
                            collection_casts.push(cast.clone());
                        }
                        current = object
                            .parent
                            .ok_or_else(|| invalid("missing collection owner"))?;
                    }
                    break;
                }
                current = parent;
            }
            collection_casts.reverse();
            collection_casts
                .dedup_by(|later, earlier| later.node.is_some() && later.node == earlier.node);
            if cached.is_some()
                && !collection_casts.is_empty()
                && let Some(frame) = self.frames.last_mut()
            {
                frame.callsite_hidden = true;
            }
            constant_value = cached;
        }
        if constant_value.is_none() {
            constant_value = self
                .heap
                .get(owner)?
                .member_values
                .get(&CacheKey {
                    owner: None,
                    name: name.clone(),
                    method,
                    entry,
                })
                .cloned();
        }
        let owner_object = self.heap.get(owner)?;
        let owner_is_dynamic = !owner_object.is_module
            && owner_object.reference.is_none()
            && owner_object.pair.is_none()
            && owner_object.collection_kind == crate::heap::CollectionKind::Dynamic
            && collections::typed_class(&self.heap, owner)?.is_none();
        let is_constant_member = constant_value.is_some()
            || self
                .program
                .metadata
                .get(member.code)
                .is_some_and(|metadata| metadata.constant_member);
        // Java `TypeCheckedPropertyNode` (`@Specialization eval(VirtualFrame, VmDynamic)`)
        // and `VmClass.getTypedToDynamicMembers()` (`UntypedObjectMemberNode`) skip
        // inherited property type checks when `owner` is a `VmDynamic`, while
        // `VmUtils.doReadMember` still checks `ConstantNode` properties against
        // `Dynamic.default: (unknown) -> Any`.
        let skip_dynamic_property_typecheck = owner_is_dynamic && !is_constant_member;
        let implicit_default_type =
            if !skip_dynamic_property_typecheck && !local && !method && !entry && name == "default"
            {
                self.object_default_type(receiver)?
            } else {
                None
            };
        if let Some(value) = &constant_value {
            if collection_casts.is_empty() && implicit_default_type.is_none() {
                if memoize {
                    self.complete_cache_probe(receiver, key, value.clone(), entry_probe.clone())?;
                }
                self.stack.push(value.clone());
                return Ok(());
            }
        } else if self.program.blocks.get(member.code).is_none() {
            return Err(invalid("invalid member block"));
        }
        if self.frames.len() >= 4096 {
            return Err(RuntimeError::new(
                ErrorKind::Cycle,
                "experiment frame limit exceeded",
            ));
        }
        let mut inherited_types = Vec::new();
        if stolen_value.is_none()
            && !skip_dynamic_property_typecheck
            && !local
            && !method
            && member.ty.is_none()
            && self.heap.get(owner)?.mirror.is_none()
        {
            let mut parent = self.heap.get(owner)?.parent;
            let mut visited = BTreeSet::new();
            while let Some(id) = parent {
                if !visited.insert(id) {
                    return Err(RuntimeError::new(ErrorKind::Cycle, "cyclic parent chain"));
                }
                let object = self.heap.get(id)?;
                for inherited in object.members.iter().filter(|member| {
                    member.name == name && !member.local && !member.method && member.entry == entry
                }) {
                    if let (Some(ty), Some(span)) = (&inherited.ty, inherited.type_span) {
                        let name = self.program.metadata.get(inherited.code).map_or_else(
                            || inherited.name.clone(),
                            |metadata| metadata.member.clone(),
                        );
                        inherited_types.push((ty.clone(), span, name, id));
                    }
                }
                if !inherited_types.is_empty() {
                    break;
                }
                parent = object.parent;
            }
        }
        if stolen_value.is_none() && inherited_types.is_empty() && member.ty.is_none() {
            inherited_types.extend(implicit_default_type);
        }
        let object = self.heap.get(owner)?;
        let mut environment = object
            .member_environments
            .get(&CacheKey {
                owner: None,
                name: name.clone(),
                method,
                entry,
            })
            .unwrap_or(&object.environment)
            .clone();
        environment.outer_value = environment.this_value.clone();
        environment.this_value = Some(Value::Object(receiver));
        environment.custom_this = None;
        environment.executable_owner = environment
            .executable_owner
            .map(|parent| {
                self.heap
                    .executable_child(parent, ExecutionSite::Member(member.code))
            })
            .transpose()?;
        if environment.restore_for_bindings {
            let target = environment
                .executable_owner
                .unwrap_or_else(|| self.executable_root(member.code));
            self.copy_captured_locals(&mut environment, target)?;
            environment.restore_for_bindings = false;
        }
        environment.scopes.insert(0, Scope { receiver, owner });
        if memoize && stolen_value.is_none() {
            self.heap
                .get_mut(receiver)?
                .cache
                .insert(key.clone(), LazyState::Evaluating);
        }
        self.member_evaluations += usize::from(constant_value.is_none()) as u64;
        let base = self.stack.len();
        let (block, pc) = if let Some(value) = constant_value {
            self.stack.push(value);
            if let Some(block) = self.program.blocks.get(member.code) {
                (member.code, block.len() - 1)
            } else {
                (usize::MAX, 0)
            }
        } else {
            (member.code, 0)
        };
        let mut frame = self.alloc_frame(block, pc, base, environment);
        frame.force = Some(Box::new(Force {
            entry,
            entry_probe,
            publishing_cache: false,
            stolen_cache_value: stolen_value.is_some(),
            stolen_cast: None,
            receiver,
            owner,
            name,
            cache: memoize.then_some(key),
            const_receiver: None,
            method,
            inherited_types,
            collection_casts,
            skip_typecheck: !memoize && !method,
        }));
        self.frames.push(frame);
        Ok(())
    }

    fn require_const(
        &self,
        initial: ObjectId,
        name: &PklString,
        local: bool,
        method: bool,
    ) -> Result<(), RuntimeError> {
        let mut owner = Some(initial);
        let mut visited = BTreeSet::new();
        while let Some(id) = owner {
            if !visited.insert(id) {
                return Err(RuntimeError::new(ErrorKind::Cycle, "cyclic parent chain"));
            }
            let object = self.heap.get(id)?;
            if let Some(member) = object.members.iter().find(|member| {
                &member.name == name && member.local == local && member.method == method
            }) {
                if member.constant {
                    return Ok(());
                }
                let (verb, kind, reference) = if method {
                    ("call", "method", "reference this method")
                } else {
                    ("reference", "property", "reference this property")
                };
                let mut error = RuntimeError::new(
                    ErrorKind::Type,
                    PklString::from(format!("Cannot {verb} {kind} `"))
                        .concat(name)
                        .concat(&"` from here because it is not `const`.".into()),
                );
                error.hint = Some(
                    PklString::from(format!(
                        "To fix, do either of:\n1. Add modifier `const` to {kind} `"
                    ))
                    .concat(name)
                    .concat(
                        &format!("`\n2. Self-import this module, and {reference} from the import.")
                            .into(),
                    ),
                );
                return Err(error);
            }
            if local {
                break;
            }
            owner = object.parent;
        }
        if method
            && (matches!(name.as_str(), Some("toString" | "ifNonNull"))
                || matches!(
                    name.as_str(),
                    Some("hasProperty" | "getProperty" | "getPropertyOrNull")
                ) && self.typed_has_property_available(initial)?)
        {
            return Err(Self::non_const_method_error(
                name.as_str().expect("known builtin method"),
            ));
        }
        Ok(())
    }

    /// Match lexical candidate visibility without evaluating candidate members.
    fn implicit_member_suggestions(
        &self,
        frame: &Frame,
        name: &PklString,
        arity: isize,
    ) -> Option<PklString> {
        use crate::diagnostic::MemberSuggestion;

        let mut candidates = Vec::new();
        // Lexical owners include local declarations; receiver ancestry below does not.
        for scope in &frame.environment.scopes {
            let Ok(object) = self.heap.get(scope.owner) else {
                continue;
            };
            let mut declared_methods = Vec::new();
            for member in object.members.iter().filter(|member| {
                !member.entry
                    && (!object.module_initializing
                        || object.published_module_members.contains(&member.code))
            }) {
                if !member.method
                    || (member.local && !object.is_module && object.class_definition.is_none())
                {
                    candidates.push(MemberSuggestion::Property(member.name.clone()));
                } else if let Some((signature, arity)) = self
                    .program
                    .metadata
                    .get(member.code)
                    .and_then(|metadata| metadata.method_signature.as_ref())
                {
                    declared_methods.push(MemberSuggestion::Method {
                        name: member.name.clone(),
                        signature: signature.clone(),
                        arity: *arity,
                    });
                }
            }
            if object.is_module {
                candidates.push(MemberSuggestion::Property("output".into()));
            }
            // Each lexical owner's properties precede its declared methods; both
            // precede candidates from enclosing owners and the base module.
            candidates.extend(declared_methods);
        }
        candidates.extend(
            crate::diagnostic_names::BASE_PROPERTIES
                .iter()
                .map(|name| MemberSuggestion::Property((*name).into())),
        );
        candidates.extend(
            crate::diagnostic_names::MODULE_METHODS
                .iter()
                .chain(crate::diagnostic_names::BASE_METHODS)
                .map(|(name, signature, arity)| MemberSuggestion::Method {
                    name: (*name).into(),
                    signature: (*signature).into(),
                    arity: *arity,
                }),
        );
        let is_custom_this = matches!(
            self.program
                .blocks
                .get(frame.block)
                .and_then(|code| code.get(frame.pc.saturating_sub(1))),
            Some(
                Instruction::GetImplicitCustomProperty(_) | Instruction::GetImplicitCustomMethod(_)
            )
        );
        let receiver = if is_custom_this {
            frame
                .environment
                .custom_this
                .and_then(|slot| self.heap.custom_this(slot).ok().flatten())
                .and_then(|value| value.as_object().ok())
        } else {
            frame
                .environment
                .this_value
                .as_ref()
                .and_then(|value| value.as_object().ok())
                .or_else(|| frame.environment.scopes.first().map(|scope| scope.receiver))
        };
        let mut current = receiver;
        let mut visited = BTreeSet::new();
        let mut receiver_methods = Vec::new();
        while let Some(id) = current {
            if !visited.insert(id) {
                break;
            }
            let Ok(object) = self.heap.get(id) else {
                break;
            };
            for member in object.members.iter().filter(|member| {
                !member.entry
                    && !member.local
                    && (!object.module_initializing
                        || object.published_module_members.contains(&member.code))
            }) {
                if !member.method {
                    candidates.push(MemberSuggestion::Property(member.name.clone()));
                } else if let Some((signature, arity)) = self
                    .program
                    .metadata
                    .get(member.code)
                    .and_then(|metadata| metadata.method_signature.as_ref())
                {
                    receiver_methods.push(MemberSuggestion::Method {
                        name: member.name.clone(),
                        signature: signature.clone(),
                        arity: *arity,
                    });
                }
            }
            current = object.parent;
        }
        candidates.extend(receiver_methods);
        crate::diagnostic::ordered_member_suggestions(name, arity, candidates)
    }

    /// VmExceptionBuilder enumerates the live prototype, never the full template.
    fn missing_member(
        &self,
        receiver: ObjectId,
        name: &PklString,
        method: bool,
    ) -> Result<RuntimeError, RuntimeError> {
        if self.heap.get(receiver)?.is_module {
            let module_name = self
                .heap
                .get(receiver)?
                .class_definition
                .as_ref()
                .map_or(&self.program.module_name, |class| &class.name);
            let kind = if method { "method" } else { "property" };
            let mut error = RuntimeError::new(
                ErrorKind::MissingMember,
                PklString::from(format!("Cannot find {kind} `"))
                    .concat(name)
                    .concat(&"` in module `".into())
                    .concat(module_name)
                    .concat(&"`.".into()),
            );
            {
                let mut properties = vec![PklString::from("output")];
                let mut methods = Vec::new();
                let mut current = Some(receiver);
                let mut seen = BTreeSet::new();
                while let Some(id) = current {
                    if !seen.insert(id) {
                        return Err(RuntimeError::new(ErrorKind::Cycle, "cyclic parent chain"));
                    }
                    let object = self.heap.get(id)?;
                    for member in object.members.iter().filter(|member| {
                        !member.local
                            && !member.entry
                            && (!object.module_initializing
                                || object.published_module_members.contains(&member.code))
                    }) {
                        if !member.method {
                            properties.push(member.name.clone());
                        } else if let Some((signature, arity)) = self
                            .program
                            .metadata
                            .get(member.code)
                            .and_then(|metadata| metadata.method_signature.as_ref())
                        {
                            methods.push((member.name.clone(), signature.clone(), *arity));
                        }
                    }
                    current = object.parent;
                }
                methods.extend(crate::diagnostic_names::MODULE_METHODS.iter().map(
                    |(name, signature, arity)| {
                        (PklString::from(*name), PklString::from(*signature), *arity)
                    },
                ));
                let arity = if method {
                    self.frames
                        .last()
                        .and_then(|frame| {
                            self.program.metadata.get(frame.block).and_then(|metadata| {
                                metadata.method_arities.get(&frame.pc.saturating_sub(1))
                            })
                        })
                        .copied()
                        .unwrap_or(0) as isize
                } else {
                    -1
                };
                error.hint =
                    crate::diagnostic::member_suggestions(name, arity, &properties, &methods)
                        .or_else(|| {
                            if !method {
                                return None;
                            }
                            let mut names: Vec<_> =
                                methods.into_iter().map(|method| method.0).collect();
                            names.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
                            names.dedup();
                            Some(
                                PklString::from("Available methods in module `")
                                    .concat(module_name)
                                    .concat(&"`:\n".into())
                                    .concat(&PklString::join(&names, "\n")),
                            )
                        });
                if method || error.hint.is_some() {
                    return Ok(error);
                }
            }
            let mut names = self
                .heap
                .get(receiver)?
                .members
                .iter()
                .filter(|member| {
                    !member.local
                        && member.method == method
                        && (!self.heap.get(receiver).expect("module").module_initializing
                            || self
                                .heap
                                .get(receiver)
                                .expect("module")
                                .published_module_members
                                .contains(&member.code))
                })
                .map(|member| member.name.clone())
                .collect::<Vec<_>>();
            if !method {
                names.push("output".into());
            }
            names.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
            names.dedup();
            error.hint = Some(
                PklString::from(if method {
                    "Available methods in module `"
                } else {
                    "Available properties in module `"
                })
                .concat(module_name)
                .concat(&"`:\n".into())
                .concat(&PklString::join(&names, "\n")),
            );
            return Ok(error);
        }
        let class = collections::typed_class(&self.heap, receiver)?;
        let object = self.heap.get(receiver)?;
        if object.pair.is_some() {
            // Pair stores its fields outside ordinary object members. Diagnostic
            // candidates still come from its pinned class declarations.
            return Ok(self.builtin_member_error("Pair", name, method));
        }
        let dynamic = class.is_none()
            && object.collection_kind == crate::heap::CollectionKind::Dynamic
            && object.reference.is_none()
            && !object.ref_module;
        if class.is_none() && !dynamic {
            if object.pair.is_none() && object.reference.is_none() && !object.ref_module {
                let class_name = format!("{:?}", object.collection_kind);
                let (properties, methods) =
                    crate::diagnostic_names::collection_members(&class_name);
                let properties = properties
                    .into_iter()
                    .map(PklString::from)
                    .collect::<Vec<_>>();
                let methods = methods
                    .into_iter()
                    .map(|(name, signature, arity)| (name.into(), signature.into(), arity))
                    .collect::<Vec<_>>();
                let arity = if method {
                    self.frames
                        .last()
                        .and_then(|frame| {
                            self.program.metadata.get(frame.block).and_then(|meta| {
                                meta.method_arities.get(&frame.pc.saturating_sub(1))
                            })
                        })
                        .copied()
                        .unwrap_or(0) as isize
                } else {
                    -1
                };
                let mut error = RuntimeError::new(
                    ErrorKind::MissingMember,
                    if method {
                        PklString::from("Cannot find method `")
                            .concat(name)
                            .concat(&"` in class `".into())
                            .concat(&PklString::from(&class_name))
                            .concat(&"`.".into())
                    } else {
                        PklString::from("Cannot find property `")
                            .concat(name)
                            .concat(&"` in object of type `".into())
                            .concat(&PklString::from(&class_name))
                            .concat(&"`.".into())
                    },
                );
                error.hint =
                    crate::diagnostic::member_suggestions(name, arity, &properties, &methods)
                        .or_else(|| {
                            let mut names = if method {
                                methods.into_iter().map(|m| m.0).collect()
                            } else {
                                properties
                            };
                            names.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
                            names.dedup();
                            Some(if method {
                                PklString::from("Available methods in class `")
                                    .concat(&PklString::from(&class_name))
                                    .concat(&"`:\n".into())
                                    .concat(&PklString::join(&names, "\n"))
                            } else {
                                PklString::from("Available properties:\n").concat(&if names
                                    .is_empty()
                                {
                                    "(none)".into()
                                } else {
                                    PklString::join(&names, "\n")
                                })
                            })
                        });
                return Ok(error);
            }
            return Ok(RuntimeError::new(
                ErrorKind::MissingMember,
                PklString::from("missing member ").concat(name),
            ));
        }
        let mut properties = Vec::new();
        let mut method_groups = Vec::new();
        let mut current = Some(receiver);
        let mut visited = BTreeSet::new();
        while let Some(id) = current {
            if !visited.insert(id) {
                return Err(RuntimeError::new(ErrorKind::Cycle, "cyclic parent chain"));
            }
            let object = self.heap.get(id)?;
            let mut methods = Vec::new();
            for member in object.members.iter().filter(|m| !m.local && !m.entry) {
                if member.method {
                    if let Some(signature) = self
                        .program
                        .metadata
                        .get(member.code)
                        .and_then(|m| m.method_signature.clone())
                    {
                        methods.push((member.name.clone(), signature.0, signature.1));
                    }
                } else {
                    properties.push(member.name.clone());
                }
            }
            method_groups.push(methods);
            // Diagnostic metadata follows the same explicit builtin ancestry.
            if let Some(ty) = &object.builtin_type {
                let inherited = match classes::BuiltinClass::from_type(ty) {
                    Some(classes::BuiltinClass::Any) => {
                        &crate::diagnostic_names::TYPED_METHODS[..3]
                    }
                    Some(classes::BuiltinClass::Typed) => {
                        &crate::diagnostic_names::TYPED_METHODS[3..]
                    }
                    Some(classes::BuiltinClass::Class) => {
                        properties.push(PklString::from("simpleName"));
                        &[]
                    }
                    _ => &[],
                };
                method_groups.push(
                    inherited
                        .iter()
                        .map(|(name, signature, arity)| {
                            (PklString::from(*name), PklString::from(*signature), *arity)
                        })
                        .collect(),
                );
            }
            current = object.parent;
        }
        if dynamic {
            // Dynamic's inherited hidden default is a real readable builtin
            // property (base.pkl:2015), hence also an available-name candidate.
            properties.push("default".into());
            method_groups.push(
                crate::diagnostic_names::DYNAMIC_METHODS
                    .iter()
                    .map(|(name, signature, arity)| {
                        (PklString::from(*name), PklString::from(*signature), *arity)
                    })
                    .collect(),
            );
        }
        // VmClass.getAllMethods merges ancestors first; overriding a method
        // replaces its definition without moving its insertion position.
        let mut methods: Vec<(PklString, PklString, usize)> = Vec::new();
        for candidate in method_groups.into_iter().rev().flatten() {
            if let Some(existing) = methods.iter_mut().find(|m| m.0 == candidate.0) {
                *existing = candidate;
            } else {
                methods.push(candidate);
            }
        }
        let arity = if method {
            self.frames
                .last()
                .and_then(|frame| {
                    self.program.metadata.get(frame.block).and_then(|metadata| {
                        metadata.method_arities.get(&frame.pc.saturating_sub(1))
                    })
                })
                .copied()
                .map_or(0, |n| n as isize)
        } else {
            -1
        };
        let class_name = class.map_or_else(
            || PklString::from("Dynamic"),
            |class| {
                reference::type_name(
                    &TypeCheck::Class {
                        module: class.module.clone(),
                        id: class.id,
                        name: class.name,
                    },
                    &self.program.module_name,
                )
            },
        );
        let mut error = RuntimeError::new(
            ErrorKind::MissingMember,
            if method {
                PklString::from("Cannot find method `")
                    .concat(name)
                    .concat(&"` in class `".into())
                    .concat(&PklString::from(&class_name))
                    .concat(&"`.".into())
            } else {
                PklString::from("Cannot find property `")
                    .concat(name)
                    .concat(&"` in object of type `".into())
                    .concat(&PklString::from(&class_name))
                    .concat(&"`.".into())
            },
        );
        error.hint = crate::diagnostic::member_suggestions(name, arity, &properties, &methods)
            .or_else(|| {
                let mut names = if method {
                    methods.into_iter().map(|m| m.0).collect()
                } else {
                    properties
                };
                names.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
                names.dedup();
                Some(if method {
                    PklString::from("Available methods in class `")
                        .concat(&PklString::from(&class_name))
                        .concat(&"`:\n".into())
                        .concat(&PklString::join(&names, "\n"))
                } else {
                    PklString::from("Available properties:\n").concat(&if names.is_empty() {
                        "(none)".into()
                    } else {
                        PklString::join(&names, "\n")
                    })
                })
            });
        Ok(error)
    }

    /// Find an inherited declaration without forcing its initializer. Typed object
    /// validation starts at the class prototype, not an instance's cached value.
    fn inherited_member(
        &self,
        parent: ObjectId,
        member: &Member,
    ) -> Result<Option<&Member>, RuntimeError> {
        let mut current = Some(parent);
        let mut visited = BTreeSet::new();
        let mut definition = None;
        while let Some(id) = current {
            if !visited.insert(id) {
                return Err(RuntimeError::new(ErrorKind::Cycle, "cyclic parent chain"));
            }
            let object = self.heap.get(id)?;
            if let Some(inherited) = object.members.iter().find(|inherited| {
                !inherited.local
                    && inherited.entry == member.entry
                    && inherited.method == member.method
                    && inherited.name == member.name
            }) {
                // VmClass.collectAllProperties keeps an inherited declaration
                // when a prototype override has no type annotation. Value dispatch
                // still uses that closest override in force_key_mode_prepared.
                if inherited.method || inherited.ty.is_some() {
                    return Ok(Some(inherited));
                }
                definition = Some(inherited);
            }
            current = object.parent;
        }
        Ok(definition)
    }

    fn validate_class_override(
        &self,
        parent: ObjectId,
        member: &Member,
    ) -> Result<(), RuntimeError> {
        if member.local {
            return Ok(());
        }
        let Some(inherited) = self.inherited_member(parent, member)? else {
            return Ok(());
        };
        if member.method {
            if inherited.constant && !member.constant {
                return Err(RuntimeError::new(
                    ErrorKind::Type,
                    PklString::from("Method `")
                        .concat(&member.name)
                        .concat(&"` must be const because it overrides a const method.".into()),
                ));
            }
            return Ok(());
        }
        let parent_class = self
            .heap
            .get(parent)?
            .class_definition
            .as_ref()
            .ok_or_else(|| invalid("missing parent class"))?;
        let parent_name = if self.heap.get(parent)?.is_module {
            parent_class.name.clone()
        } else if parent_class.id == reference::ACCESS_ID {
            PklString::from("pkl.ref#Access")
        } else {
            parent_class
                .module
                .as_ref()
                .unwrap_or(&self.program.module_name)
                .concat(&"#".into())
                .concat(&parent_class.name)
        };
        // UnresolvedPropertyNode resolves the type, then checks fixed, then const.
        for (modifier, actual, expected) in [
            ("fixed", member.fixed, inherited.fixed),
            ("const", member.constant, inherited.constant),
        ] {
            if actual == expected {
                continue;
            }
            let metadata = self.program.metadata.get(member.code);
            let mut error = if expected {
                let mut error =
                    RuntimeError::new(ErrorKind::Type, format!("Missing modifier `{modifier}`."));
                error.hint = Some(PklString::from("Property `").concat(&member.name).concat(&format!("` must be declared {modifier}, because it overrides a {modifier} property on parent class `").into()).concat(&parent_name).concat(&"`.".into()));
                error.span = metadata.and_then(|m| m.declaration);
                error
            } else {
                let mut error = RuntimeError::new(
                    ErrorKind::Type,
                    PklString::from(format!(
                        "Modifier `{modifier}` cannot be applied to property `"
                    ))
                    .concat(&member.name)
                    .concat(&"`.".into()),
                );
                error.hint = Some(PklString::from("Property `").concat(&member.name).concat(&format!("` cannot be declared {modifier}, because it overrides a non-{modifier} property on parent class `").into()).concat(&parent_name).concat(&"`.".into()));
                error.span = metadata.and_then(|m| {
                    if modifier == "fixed" {
                        m.fixed_span
                    } else {
                        m.const_span
                    }
                });
                error
            };
            // Class initialization errors belong to the class's call target.
            if error.span.is_none() {
                error.span = metadata.and_then(|m| m.declaration);
            }
            return Err(error);
        }
        Ok(())
    }

    fn validate_object_override(
        &self,
        parent: ObjectId,
        member: &Member,
    ) -> Result<(), RuntimeError> {
        let Some(inherited) = self.inherited_member(parent, member)? else {
            return Ok(());
        };
        if member.method {
            if inherited.constant && !member.constant {
                return Err(RuntimeError::new(
                    ErrorKind::Type,
                    PklString::from("Method `")
                        .concat(&member.name)
                        .concat(&"` must be const because it overrides a const method.".into()),
                ));
            }
        } else if inherited.constant || inherited.fixed {
            let metadata = self.program.metadata.get(member.code);
            let action = if metadata.is_some_and(|m| m.amendment) {
                "amend"
            } else {
                "assign to"
            };
            let modifier = if inherited.constant { "const" } else { "fixed" };
            let mut error = RuntimeError::new(
                ErrorKind::Type,
                PklString::from(format!("Cannot {action} {modifier} property `"))
                    .concat(&member.name)
                    .concat(&"`.".into()),
            );
            error.span = metadata.and_then(|m| m.declaration);
            return Err(error);
        }
        Ok(())
    }

    fn validate_object_members(
        &self,
        parent: ObjectId,
        members: &[Member],
    ) -> Result<(), RuntimeError> {
        if collections::typed_class(&self.heap, parent)?.is_none() {
            for member in members.iter().filter(|member| !member.local) {
                self.validate_object_override(parent, member)?;
            }
            return Ok(());
        }
        let mut ancestor = Some(parent);
        let mut declared = BTreeSet::new();
        let mut property_names = Vec::new();
        let mut class_name = None;
        let mut class_parent = None;
        let mut class_initializing = false;
        let mut visited = BTreeSet::new();
        while let Some(id) = ancestor {
            if !visited.insert(id) {
                return Err(RuntimeError::new(ErrorKind::Cycle, "cyclic parent chain"));
            }
            let object = self.heap.get(id)?;
            if class_name.is_none() {
                if object.class_definition.is_some() {
                    class_parent = Some(id);
                    class_initializing = object.class_initializing;
                }
                class_name = object
                    .class_definition
                    .as_ref()
                    .map(|class| class.name.clone());
            }
            property_names.extend(
                object
                    .members
                    .iter()
                    .filter(|member| !member.local && !member.method)
                    .map(|member| member.name.clone()),
            );
            declared.extend(
                object
                    .members
                    .iter()
                    .filter(|member| !member.local)
                    .map(|member| (member.name.clone(), member.method)),
            );
            ancestor = object.parent;
        }
        if let Some(class_name) = class_name {
            // Typed amendments diagnose the class prototype. A module
            // prototype retains its own ModuleInfo name, including when the
            // receiver being amended adopted that class from another module.
            let module_name = class_parent
                .filter(|id| self.heap.get(*id).is_ok_and(|object| object.is_module))
                .map(|_| class_name.clone());
            let class_label = if let Some(id) = class_parent {
                let class = self
                    .heap
                    .get(id)?
                    .class_definition
                    .as_ref()
                    .expect("class metadata");
                reference::type_name(
                    &TypeCheck::Class {
                        module: class.module.clone(),
                        id: class.id,
                        name: class_name,
                    },
                    &self.program.module_name,
                )
            } else {
                self.program
                    .module_name
                    .concat(&"#".into())
                    .concat(&class_name)
            };
            for member in members.iter().filter(|member| !member.local) {
                if !(declared.contains(&(member.name.clone(), member.method))
                    || class_initializing && !member.method)
                {
                    let mut error = RuntimeError::new(
                        ErrorKind::MissingMember,
                        if let Some(module_name) = &module_name {
                            PklString::from("Cannot find property `")
                                .concat(&member.name)
                                .concat(&"` in module `".into())
                                .concat(module_name)
                                .concat(&"`.".into())
                        } else {
                            PklString::from("Cannot find property `")
                                .concat(&member.name)
                                .concat(&"` in object of type `".into())
                                .concat(&class_label)
                                .concat(&"`.".into())
                        },
                    );
                    error.span = self
                        .program
                        .metadata
                        .get(member.code)
                        .and_then(|metadata| metadata.declaration);
                    error.hint = crate::diagnostic::property_suggestions(
                        &member.name,
                        property_names.clone(),
                        false,
                    )
                    .or_else(|| {
                        let mut names = property_names.clone();
                        names.sort_by(|left, right| left.encode_utf16().cmp(right.encode_utf16()));
                        names.dedup();
                        Some(if let Some(module_name) = &module_name {
                            PklString::from("Available properties in module `")
                                .concat(module_name)
                                .concat(&"`:\n".into())
                                .concat(&PklString::join(&names, "\n"))
                        } else {
                            PklString::from("Available properties:\n").concat(
                                &if names.is_empty() {
                                    "(none)".into()
                                } else {
                                    PklString::join(&names, "\n")
                                },
                            )
                        })
                    });
                    return Err(error);
                }
                self.validate_object_override(class_parent.unwrap_or(parent), member)?;
            }
        } else {
            for member in members.iter().filter(|m| !m.local) {
                self.validate_object_override(parent, member)?;
            }
        }
        Ok(())
    }

    fn object_method_signature_frame(&self, frame: &Frame) -> bool {
        // InvokeObjectMethodNode requests this lazy call target with raw
        // CallTarget.call(owner, owner), before creating a DirectCallNode.
        // Derive the unavailable caller from the active initialization frame,
        // so caught failures cannot leave a hidden flag on a retained caller.
        frame.force.as_ref().is_some_and(|force| force.method)
            && matches!(
                self.program
                    .blocks
                    .get(frame.block)
                    .and_then(|code| code.first()),
                Some(Instruction::BeginObjectMethodSignature)
            )
    }

    fn executable_root(&mut self, block: usize) -> ExecutableOwnerId {
        if let Some(&owner) = self.executable_roots.get(&block) {
            return owner;
        }
        let owner = self.heap.allocate_executable();
        self.executable_roots.insert(block, owner);
        self.declaration_roots_dirty = true;
        owner
    }

    fn current_executable(&mut self) -> Result<ExecutableOwnerId, RuntimeError> {
        let frame = self
            .frames
            .last()
            .ok_or_else(|| invalid("missing executable frame"))?;
        if let Some(owner) = frame.environment.executable_owner {
            return Ok(owner);
        }
        // None denotes the canonical static template, not an uninitialized
        // captured owner. A prior check must not change how later function or
        // object literals clone their code state on this invocation's path.
        Ok(self.executable_root(frame.block))
    }

    /// The pinned Java node replaces itself before evaluating either selected
    /// read. Keep both that choice and the local declaration across failed or
    /// suspended values, while each invocation still supplies owner/receiver.
    #[inline(never)]
    fn read_ambiguous_property(
        &mut self,
        name: PklString,
        owner_depth: usize,
        needs_const: bool,
    ) -> Result<(), RuntimeError> {
        let scope = self.scope(owner_depth)?;
        let executable = self.current_executable()?;
        let frame = self.frames.last().expect("ambiguous read frame");
        let site = (frame.block, frame.pc - 1);
        let local = match self.heap.ambiguous_locality(executable, site)? {
            Some(local) => local,
            None => {
                // VmObject.hasMember checks only this receiver's declarations,
                // with local identifiers distinct from public identifiers.
                let local = !self.heap.get(scope.receiver)?.members.iter().any(|member| {
                    member.name == name && !member.local && !member.method && !member.entry
                });
                self.heap.specialize_locality(executable, site, local)?;
                local
            }
        };
        let resolved = if local {
            match self.heap.resolved_local(executable, site)? {
                Some(member) => Some(member),
                None => {
                    let member = self
                        .heap
                        .get(scope.owner)?
                        .members
                        .iter()
                        .find(|member| {
                            member.name == name && member.local && !member.method && !member.entry
                        })
                        .cloned()
                        .ok_or_else(|| {
                            RuntimeError::new(
                                ErrorKind::MissingMember,
                                PklString::from("missing local member ")
                                    .concat(&name)
                                    .concat(&"".into()),
                            )
                        })?;
                    // ReadLocalPropertyNode publishes its declaration before
                    // checking constness; a failed value must not re-resolve it.
                    self.heap.publish_local(executable, site, member.clone())?;
                    if needs_const {
                        self.require_const(scope.owner, &name, true, false)?;
                    }
                    Some(member)
                }
            }
        } else {
            if needs_const {
                self.require_const(scope.receiver, &name, false, false)?;
            }
            None
        };
        self.force_key_mode_prepared(
            scope.receiver,
            if local { scope.owner } else { scope.receiver },
            name,
            local,
            true,
            false,
            false,
            false,
            false,
            resolved,
        )
    }

    fn current_custom_this(&self) -> Result<Option<Value>, RuntimeError> {
        match self
            .frames
            .last()
            .expect("active frame")
            .environment
            .custom_this
        {
            Some(slot) => self.heap.custom_this(slot),
            None => Ok(None),
        }
    }

    fn set_current_custom_this(&mut self, value: Option<Value>) -> Result<(), RuntimeError> {
        let frame = self.frames.last_mut().expect("active frame");
        match frame.environment.custom_this {
            Some(slot) => self.heap.set_custom_this(slot, value),
            None => {
                if value.is_some() {
                    frame.environment.custom_this = Some(self.heap.allocate_custom_this(value));
                }
                Ok(())
            }
        }
    }

    fn invoke_function(
        &mut self,
        id: ObjectId,
        arguments: Vec<Value>,
        method: bool,
    ) -> Result<(), RuntimeError> {
        if let Some(native) = self.heap.get(id)?.native_function.clone() {
            if native.name.as_ref() == "contains" {
                return self.invoke_set_contains(id, native, arguments);
            }
            if matches!(native.name.as_ref(), "containsKey" | "getOrNull") {
                return self.invoke_map_lookup(id, native, arguments);
            }
            return self.invoke_reference_native(native, arguments);
        }
        let arity = arguments.len();
        let object = self.heap.get(id)?;
        let function = object
            .function
            .as_ref()
            .ok_or_else(|| invalid("invalid function handle"))?;
        if !function.method && method {
            return Err(RuntimeError::new(
                ErrorKind::Type,
                "function values must be invoked with apply",
            ));
        }
        // An amendment without object parameters is a forwarding call target
        // (AmendFunctionNode's SimpleRootNode). The original function checks
        // the actual arguments, retaining both its declaration and the wrapper
        // frame in diagnostics.
        if function.arity != arity
            && object
                .amendment
                .as_ref()
                .is_none_or(|amendment| amendment.parameter_count > 0 && amendment.initial)
        {
            let mut error = RuntimeError::new(
                ErrorKind::Type,
                format!(
                    "Expected {} function arguments but got {arity}.",
                    function.arity
                ),
            );
            if let Some(origin) = object
                .amendment
                .as_ref()
                .and_then(|amendment| amendment.origin.clone())
                .or_else(|| {
                    object
                        .mixin
                        .as_ref()
                        .and_then(|(_, _, origin)| origin.clone())
                })
            {
                error.frames.push(origin);
            } else if let Some(metadata) = self.program.metadata.get(function.block)
                && let Some(span) = metadata.declaration
            {
                error.frames.push(RuntimeFrame {
                    anonymous: false,
                    source: metadata.source.clone(),
                    span,
                    member: metadata.member.clone(),
                });
            }
            return Err(error);
        }
        if self.frames.len() >= 4096 {
            return Err(RuntimeError::new(
                ErrorKind::Cycle,
                "evaluation frame limit exceeded",
            ));
        }
        let amendment_arguments = object
            .amendment
            .as_ref()
            .filter(|amendment| amendment.initial && amendment.parameter_count > 0)
            .map(|_| arguments.clone());
        let mut environment = object.environment.clone();
        environment.outer_value = environment.this_value.clone();
        if let Some(receiver) = &function.captured_this {
            environment.this_value = Some(receiver.clone());
        }
        // A function call creates a logical frame, unlike a constraint bytecode
        // continuation. Its lexical capture must not supply the mutable slot.
        environment.custom_this = None;
        environment.copy_locals.clear();
        environment.restore_for_bindings = false;
        environment
            .locals
            .extend(arguments.into_iter().map(Value::boxed));
        let block = function.block;
        let lambda = !function.method;
        let base = self.stack.len();
        let mut frame = self.alloc_frame(block, 0, base, environment);
        frame.lambda = lambda;
        frame.function_owner = Some(id);
        frame.amendment_arguments = amendment_arguments;
        self.frames.push(frame);
        Ok(())
    }

    fn step(&mut self) -> Result<(), RuntimeError> {
        if let Some(frame) = self.frames.last_mut() {
            frame.fast_call_frame = false;
        }
        if self.frames.last().is_some_and(|frame| frame.glob.is_some()) {
            return self.step_glob();
        }
        if self
            .frames
            .last()
            .is_some_and(|frame| frame.guest_catch.is_some())
        {
            return self.finish_guest_catch();
        }
        if self
            .frames
            .last()
            .is_some_and(|frame| frame.type_resolution.is_some() && frame.module_load.is_none())
        {
            return self.step_declared_type();
        }
        if self
            .frames
            .last()
            .is_some_and(|frame| frame.module_load.is_some())
        {
            return self.step_module();
        }
        if self
            .frames
            .last()
            .is_some_and(|f| f.generic_default.is_some())
        {
            return self.step_generic_default();
        }
        if self.frames.last().is_some_and(|f| f.equality_run.is_some()) {
            return self.step_equality();
        }
        if self.mapping_cast_failure_pending() {
            return self.step_mapping_cast_failure();
        }
        if self
            .frames
            .last()
            .is_some_and(|frame| frame.type_run.is_some())
        {
            return self.step_type();
        }
        if self
            .frames
            .last()
            .is_some_and(|frame| frame.reflection.is_some())
        {
            return self.step_reflection();
        }
        if self
            .frames
            .last()
            .is_some_and(|frame| frame.analysis.is_some())
        {
            return self.step_analysis();
        }
        if self
            .frames
            .last()
            .is_some_and(|frame| frame.deep_force.is_some())
        {
            return self.step_deep_force();
        }
        if self
            .frames
            .last()
            .is_some_and(|frame| frame.yaml_parse.is_some())
        {
            return self.step_yaml_parse();
        }
        if self
            .frames
            .last()
            .is_some_and(|frame| frame.json_parse.is_some())
        {
            return self.step_json_parse();
        }
        if self
            .frames
            .last()
            .is_some_and(|frame| frame.pcf_render.is_some())
        {
            return self.step_pcf();
        }
        if self
            .frames
            .last()
            .is_some_and(|frame| frame.collection_join.is_some())
        {
            return self.step_collection_join();
        }
        let frame = self
            .frames
            .last_mut()
            .ok_or_else(|| invalid("missing execution frame"))?;
        let instruction = if frame.block == usize::MAX {
            Instruction::Return
        } else {
            self.program
                .blocks
                .get(frame.block)
                .and_then(|block| block.get(frame.pc))
                .ok_or_else(|| invalid("instruction pointer outside block"))?
                .clone()
        };
        frame.pc += 1;
        let instruction = if matches!(instruction, Instruction::EndFunctionAmendment)
            && frame.amendment_end == Some(frame.pc - 1)
        {
            Instruction::Return
        } else {
            instruction
        };
        let Some(instruction) = self.resolve_instruction_types(instruction)? else {
            return Ok(());
        };
        let frame_index = self.frames.len() - 1;
        // Compiling pinned bundled declarations must not retain the large
        // general instruction-dispatch frame on the native stack. Evaluation
        // still starts in the ordinary module initializer and can yield there.
        let result = if matches!(instruction, Instruction::Return) {
            self.finish_return()
        } else if matches!(instruction, Instruction::LoadBaseModule) {
            self.import_stdlib("pkl:base").map(|_| ())
        } else if matches!(instruction, Instruction::LoadProtobufModule) {
            self.import_stdlib("pkl:protobuf").map(|_| ())
        } else if matches!(instruction, Instruction::LoadPklBinaryModule) {
            self.import_stdlib("pkl:pklbinary").map(|_| ())
        } else if matches!(instruction, Instruction::LoadXmlModule) {
            self.import_stdlib("pkl:xml").map(|_| ())
        } else if matches!(instruction, Instruction::LoadJsonnetModule) {
            self.import_stdlib("pkl:jsonnet").map(|_| ())
        } else if matches!(instruction, Instruction::LoadRefModule) {
            self.import_stdlib("pkl:ref").map(|_| ())
        } else if matches!(instruction, Instruction::ReadModuleVersion) {
            self.read_module_version()
        } else if matches!(instruction, Instruction::CheckModuleVersion) {
            self.check_module_version()
        } else if matches!(
            &instruction,
            Instruction::Swap
                | Instruction::BeginCollection { .. }
                | Instruction::EndCollection
                | Instruction::SetCollectionDefault { .. }
                | Instruction::SetCollectionTypeDefault { .. }
                | Instruction::GenericConstructorParent { .. }
                | Instruction::AmendEmpty
                | Instruction::AddCollectionProperty(_)
                | Instruction::AddCollectionElement(_)
                | Instruction::AddCollectionEntry { .. }
                | Instruction::CheckMapSpread
                | Instruction::OpenIterator { .. }
                | Instruction::IteratorNext { .. }
                | Instruction::EnterForScope { .. }
                | Instruction::LeaveForScope
                | Instruction::SpreadMember
                | Instruction::OpenPredicate
                | Instruction::PredicateNext { .. }
                | Instruction::AddPredicateMember(_)
                | Instruction::Subscript { .. }
                | Instruction::SuperSubscript { .. }
                | Instruction::EntryParent { .. }
                | Instruction::EntryDefault
                | Instruction::EntryKey
                | Instruction::MakeList { .. }
                | Instruction::PrepareMapEntry
                | Instruction::PrepareSetElement
                | Instruction::AddSetElement
                | Instruction::PutSetKey
                | Instruction::AddMapEntry
        ) {
            self.collection_instruction(instruction)
        } else if matches!(
            &instruction,
            Instruction::ValidateTypeArguments { .. }
                | Instruction::BuiltinClass(_)
                | Instruction::TestType(_)
                | Instruction::ResolveType(_)
                | Instruction::PrepareTypeArguments { .. }
                | Instruction::ResolvedClass(_)
                | Instruction::ResolvedSupertype { .. }
                | Instruction::ResolvedDefault { .. }
                | Instruction::ResolvedInferredParent { .. }
                | Instruction::ResolveClassType { .. }
                | Instruction::BindLocal { .. }
                | Instruction::CheckType(_)
        ) {
            self.type_instruction(instruction)
        } else if matches!(
            &instruction,
            Instruction::GetMember(_)
                | Instruction::GetMethod(_)
                | Instruction::GetImplicitCustomProperty(_)
                | Instruction::GetImplicitCustomMethod(_)
                | Instruction::GetConstMember(_)
                | Instruction::GetConstMethod(_)
                | Instruction::GetAmbiguousProperty { .. }
                | Instruction::GetProperty { .. }
                | Instruction::GetMethodProperty { .. }
                | Instruction::GetImplicitProperty(_)
                | Instruction::GetImplicitMethod(_)
                | Instruction::InferPropertyParent
                | Instruction::GetSuperMember(_)
                | Instruction::GetSuperMethod(_)
                | Instruction::GetConstSuperMember(_)
                | Instruction::GetConstSuperMethod(_)
        ) {
            self.member_access_instruction(instruction)
        } else {
            self.step_instruction(instruction)
        };
        if let Some(frame) = self.frames.get_mut(frame_index)
            && frame
                .equality_run
                .as_ref()
                .is_none_or(|run| !run.retries_instruction())
        {
            frame.prepared_hashes = None;
        }
        result
    }

    #[inline(never)]
    fn member_access_instruction(&mut self, instruction: Instruction) -> Result<(), RuntimeError> {
        match instruction {
            Instruction::GetMember(ref name)
            | Instruction::GetMethod(ref name)
            | Instruction::GetImplicitCustomProperty(ref name)
            | Instruction::GetImplicitCustomMethod(ref name)
            | Instruction::GetConstMember(ref name)
            | Instruction::GetConstMethod(ref name) => {
                let method = matches!(
                    instruction,
                    Instruction::GetMethod(_)
                        | Instruction::GetImplicitCustomMethod(_)
                        | Instruction::GetConstMethod(_)
                );
                let needs_const = matches!(
                    instruction,
                    Instruction::GetConstMember(_) | Instruction::GetConstMethod(_)
                );
                let receiver = self.pop()?;
                if method {
                    self.retire_saturated_method_cache(&receiver, name)?;
                }
                if needs_const
                    && method
                    && name == "ifNonNull"
                    && !matches!(receiver, Value::Object(_))
                {
                    return Err(Self::non_const_method_error(name));
                }
                if self.any_member(&receiver, name, method)
                    || self.class_member(&receiver, name, method)?
                    || self.primitive_member(&receiver, name, method)?
                {
                    return Ok(());
                }
                if method && name == "encodeToBytes" && matches!(receiver, Value::String(_)) {
                    self.string_encode_method(receiver);
                    return Ok(());
                }
                if method
                    && name == "join"
                    && (self.is_list(&receiver)
                        || self.is_set(&receiver)
                        || matches!(&receiver, Value::Object(id) if self.heap.get(*id)?.collection_kind == crate::heap::CollectionKind::Listing))
                {
                    self.collection_join_method(receiver);
                    return Ok(());
                }
                if method && self.collection_transform_method(receiver.clone(), name) {
                    return Ok(());
                }
                if self.collection_projection_member(receiver.clone(), name, method)? {
                    return Ok(());
                }
                if method && name == "repeat" && matches!(receiver, Value::String(_)) {
                    self.string_repeat_method(receiver);
                } else if method
                    && name == "getClass"
                    && (!matches!(receiver, Value::Object(id) if self.heap.object_class(id)?.is_some()))
                {
                    let function = self.native_reference_method(receiver, "getClass");
                    self.stack.push(function);
                } else if matches!(receiver, Value::Function(_)) && method && name == "apply" {
                    self.stack.push(receiver);
                } else if method
                    && name == "toString"
                    && (receiver.as_object().is_err()
                        || self.is_map(&receiver)
                        || self.is_set(&receiver)
                        || self.is_list(&receiver))
                {
                    let function = self.native_reference_method(receiver, "scalarToString");
                    self.stack.push(function);
                } else if matches!(receiver, Value::Function(_)) {
                    return Err(self.function_member_error(&receiver, name, method)?);
                } else if matches!(receiver, Value::DataSize(_)) {
                    return Err(self.data_size_member_error(name, method));
                } else if matches!(receiver, Value::Duration(_)) {
                    return Err(self.duration_member_error(name, method));
                } else if matches!(receiver, Value::IntSeq(_)) {
                    return Err(self.int_seq_member_error(name, method));
                } else if matches!(receiver, Value::Bytes(_)) {
                    return Err(self.bytes_member_error(name, method));
                } else if let Some(error) = self.scalar_member_error(&receiver, name, method) {
                    return Err(error);
                } else {
                    let receiver = receiver.as_object()?;
                    if method
                        && self.heap.get(receiver)?.collection_kind
                            == crate::heap::CollectionKind::Set
                        && name == "contains"
                    {
                        let function = self.native_reference_method(
                            Value::Object(receiver),
                            name.as_str().expect("recognized builtin method"),
                        );
                        self.stack.push(function);
                        return Ok(());
                    }
                    if method
                        && self.heap.get(receiver)?.collection_kind
                            == crate::heap::CollectionKind::Map
                        && matches!(name.as_str(), Some("containsKey" | "getOrNull"))
                    {
                        let function = self.native_reference_method(
                            Value::Object(receiver),
                            name.as_str().expect("recognized builtin method"),
                        );
                        self.stack.push(function);
                        return Ok(());
                    }
                    if self.reference_member(receiver, name, method)? {
                        return Ok(());
                    }
                    if needs_const {
                        self.require_const(receiver, name, false, method)?;
                    }
                    if !method && self.collection_builtin(receiver, name)? {
                        // Builtin collection properties do not force values.
                    } else {
                        self.force(receiver, receiver, name.clone(), false, true, method)?;
                    }
                }
            }
            Instruction::GetAmbiguousProperty {
                ref name,
                owner_depth,
                needs_const,
            } => {
                self.read_ambiguous_property(name.clone(), owner_depth, needs_const)?;
            }
            Instruction::GetProperty {
                ref name,
                owner_depth,
                local,
            }
            | Instruction::GetMethodProperty {
                ref name,
                owner_depth,
                local,
            } => {
                let method = matches!(instruction, Instruction::GetMethodProperty { .. });
                let scope = self.scope(owner_depth)?;
                if method {
                    self.retire_saturated_method_cache(&Value::Object(scope.receiver), name)?;
                }
                let frame = self.frames.last().expect("active frame");
                let resolved = if local && !method {
                    frame
                        .environment
                        .executable_owner
                        .or_else(|| self.executable_roots.get(&frame.block).copied())
                        .map(|owner| self.heap.resolved_local(owner, (frame.block, frame.pc - 1)))
                        .transpose()?
                        .flatten()
                } else {
                    None
                };
                self.force_key_mode_prepared(
                    scope.receiver,
                    if local { scope.owner } else { scope.receiver },
                    name.clone(),
                    local,
                    true,
                    method,
                    false,
                    false,
                    false,
                    resolved,
                )?;
            }
            Instruction::GetImplicitProperty(ref name)
            | Instruction::GetImplicitMethod(ref name) => {
                let method = matches!(instruction, Instruction::GetImplicitMethod(_));
                let receiver = self.this_receiver()?;
                if method {
                    self.retire_saturated_method_cache(&receiver, name)?;
                }
                if self.any_member(&receiver, name, method)
                    || self.class_member(&receiver, name, method)?
                    || self.primitive_member(&receiver, name, method)?
                {
                    return Ok(());
                }
                if method
                    && name == "join"
                    && (self.is_list(&receiver)
                        || self.is_set(&receiver)
                        || matches!(&receiver, Value::Object(id) if self.heap.get(*id)?.collection_kind == crate::heap::CollectionKind::Listing))
                {
                    self.collection_join_method(receiver);
                    return Ok(());
                }
                if matches!(receiver, Value::DataSize(_)) {
                    return Err(self.data_size_member_error(name, method));
                }
                if matches!(receiver, Value::Duration(_)) {
                    return Err(self.duration_member_error(name, method));
                }
                if matches!(receiver, Value::IntSeq(_)) {
                    return Err(self.int_seq_member_error(name, method));
                }
                if matches!(receiver, Value::Bytes(_)) {
                    return Err(self.bytes_member_error(name, method));
                }
                if let Some(error) = self.scalar_member_error(&receiver, name, method) {
                    return Err(error);
                }
                let receiver = receiver.as_object()?;
                if method || !self.collection_builtin(receiver, name)? {
                    self.force(receiver, receiver, name.clone(), false, true, method)?;
                }
            }
            Instruction::InferPropertyParent => {
                let scope = self.scope(0)?;
                let name = self
                    .frames
                    .last()
                    .and_then(|frame| frame.force.as_ref())
                    .ok_or_else(|| invalid("property parent outside member"))?
                    .name
                    .clone();
                if !self.heap.get(scope.owner)?.is_module
                    && collections::typed_class(&self.heap, scope.owner)?.is_none()
                {
                    let kind = self.heap.get(scope.owner)?.collection_kind;
                    if name == "default"
                        && matches!(
                            kind,
                            crate::heap::CollectionKind::Listing
                                | crate::heap::CollectionKind::Mapping
                        )
                    {
                        let class = self.canonical_builtin_class(
                            if kind == crate::heap::CollectionKind::Listing {
                                classes::BuiltinClass::Listing
                            } else {
                                classes::BuiltinClass::Mapping
                            },
                        );
                        let prototype = self.heap.get_class(class)?.prototype;
                        self.force(prototype, prototype, name, false, true, false)?;
                    } else {
                        let empty = self.empty_object(crate::heap::CollectionKind::Dynamic);
                        self.stack.push(Value::Object(empty));
                    }
                    return Ok(());
                }
                let mut prototype = Some(scope.owner);
                while let Some(owner) = prototype {
                    let object = self.heap.get(owner)?;
                    if object.class_definition.is_some() {
                        break;
                    }
                    prototype = object.parent;
                }
                if let Some(prototype) = prototype
                    && prototype != scope.owner
                {
                    let depth = self.frames.len();
                    self.force(prototype, prototype, name, false, true, false)?;
                    if self.frames.len() > depth {
                        self.frames
                            .last_mut()
                            .expect("prototype read")
                            .force
                            .as_mut()
                            .expect("property force")
                            .skip_typecheck = true;
                    }
                } else if prototype.is_some() {
                    // A prototype uses the aggregated declaration type, not a
                    // superclass initializer. Ordinary instances above read
                    // their prototype's value instead.
                    let mut owner = prototype;
                    let mut annotation = None;
                    while let Some(id) = owner {
                        let object = self.heap.get(id)?;
                        if let Some(member) = object.members.iter().find(|member| {
                            !member.entry
                                && !member.local
                                && !member.method
                                && member.name == name
                                && member.ty.is_some()
                        }) {
                            let origin =
                                self.program.metadata.get(member.code).and_then(|metadata| {
                                    member.type_span.map(|span| RuntimeFrame {
                                        anonymous: false,
                                        source: metadata.source.clone(),
                                        span,
                                        member: metadata.member.concat(&".default".into()),
                                    })
                                });
                            annotation = Some((member.ty.clone().expect("property type"), origin));
                            break;
                        }
                        owner = object.parent;
                    }
                    if let Some((ty, origin)) = annotation
                        && !matches!(ty.resolved(), TypeCheck::Unknown)
                    {
                        let frame = self.frames.last_mut().expect("property default frame");
                        let ty = ty.substituted(&frame.environment)?;
                        frame.generic_default = Some(generics::GenericDefaultRun::inferred_parent(
                            ty, origin, None,
                        ));
                    } else {
                        let empty = self.empty_object(crate::heap::CollectionKind::Dynamic);
                        self.stack.push(Value::Object(empty));
                    }
                } else {
                    let empty = self.empty_object(crate::heap::CollectionKind::Dynamic);
                    self.stack.push(Value::Object(empty));
                }
            }
            Instruction::GetSuperMember(ref name)
            | Instruction::GetSuperMethod(ref name)
            | Instruction::GetConstSuperMember(ref name)
            | Instruction::GetConstSuperMethod(ref name) => {
                let method = matches!(
                    instruction,
                    Instruction::GetSuperMethod(_) | Instruction::GetConstSuperMethod(_)
                );
                let scope = self.scope(0)?;
                let parent = self.heap.get(scope.owner)?.parent;
                if !method {
                    let mut owner = parent;
                    let mut found = false;
                    while let Some(id) = owner {
                        let object = self.heap.get(id)?;
                        if object.members.iter().any(|member| {
                            !member.entry && !member.local && !member.method && member.name == *name
                        }) {
                            found = true;
                            break;
                        }
                        owner = object.parent;
                    }
                    if !found {
                        self.stack
                            .push(Value::Object(self.heap.allocate(Object::default())));
                        return Ok(());
                    }
                }
                let parent = parent.ok_or_else(|| {
                    RuntimeError::new(ErrorKind::MissingMember, "super has no parent")
                })?;
                if matches!(
                    instruction,
                    Instruction::GetConstSuperMember(_) | Instruction::GetConstSuperMethod(_)
                ) {
                    self.require_const(parent, name, false, method)?;
                }
                // Java ReadSuperPropertyNode deliberately does not memoize super calls.
                self.force(scope.receiver, parent, name.clone(), false, false, method)?;
            }
            _ => unreachable!("member access instruction called with non-member instruction"),
        }
        Ok(())
    }

    /// Keep the large general dispatch frame out of collection and continuation
    /// calls; their independent bounded stacks are exercised at 256 KiB.
    fn step_instruction(&mut self, instruction: Instruction) -> Result<(), RuntimeError> {
        let frame = self.frames.last_mut().expect("active frame");
        match instruction {
            Instruction::ValidateTypeArguments { .. }
            | Instruction::BuiltinClass(_)
            | Instruction::TestType(_)
            | Instruction::ResolveType(_)
            | Instruction::PrepareTypeArguments { .. }
            | Instruction::ResolvedClass(_)
            | Instruction::ResolvedSupertype { .. }
            | Instruction::ResolvedDefault { .. }
            | Instruction::ResolvedInferredParent { .. }
            | Instruction::ResolveClassType { .. }
            | Instruction::BindLocal { .. }
            | Instruction::CheckType(_) => {
                unreachable!("type instruction uses its bounded dispatcher")
            }
            Instruction::ImportModule(spec) => self.begin_import(spec)?,
            Instruction::ImportGlob(spec) => {
                let authored = spec
                    .import
                    .as_ref()
                    .ok_or_else(|| invalid("glob import spec"))?
                    .authored
                    .clone();
                self.begin_glob(spec, authored)?;
            }
            Instruction::GlobMember => self.load_glob_member()?,
            Instruction::PrepareModuleImports(unit) => self.prepare_module_imports(unit)?,
            Instruction::LoadPrimary(source) => self.begin_primary(source)?,
            Instruction::EvaluateQuery(expression) => self.begin_query(expression)?,
            Instruction::EvaluateRawQuery(expression) => self.begin_raw_query(expression)?,
            Instruction::InitializeModuleParent => self.initialize_module_parent()?,
            Instruction::CheckModuleOutput => self.check_module_output()?,
            Instruction::BootstrapStdlib => self.bootstrap_stdlib()?,
            Instruction::ReadModuleVersion | Instruction::CheckModuleVersion => {
                unreachable!("module version instruction uses its bounded dispatcher")
            }
            Instruction::MatchModuleVersion => {
                let Value::String(value) = self.pop()? else {
                    return Err(invalid("checked version string"));
                };
                let valid = annotations::matches_module_version(&value);
                self.stack.push(Value::Bool(valid));
            }
            Instruction::EncodeUtf8 => {
                let Value::String(value) = self.pop()? else {
                    return Err(invalid("UTF-8 encoding operand"));
                };
                self.stack
                    .push(Value::Bytes(value.to_java_utf8().into_bytes().into()));
            }
            Instruction::EncodePklBinary => {
                self.begin_pkl_binary_encode_value()?;
            }
            Instruction::ResourceProperty(property) => self.resource_property(property)?,
            Instruction::ResolveEvaluatorSettingsPath => self.resolve_evaluator_settings_path()?,
            Instruction::PlatformCurrent => self.platform_current()?,
            Instruction::ReleaseCurrent => self.release_current()?,
            Instruction::BenchmarkRun => self.benchmark_run()?,
            Instruction::AnalyzeImportGraph => self.begin_analysis()?,
            Instruction::OutputRenderer => self.output_renderer()?,
            Instruction::RenderPcf { document } => {
                self.begin_render(document, rendering::Format::Pcf)?
            }
            Instruction::RenderPklBinary { document } => {
                self.begin_pkl_binary_render(document)?;
            }
            Instruction::RenderProtobuf { document } => self.begin_protobuf_render(document)?,
            Instruction::ProtobufRenderType => self.protobuf_render_type()?,
            Instruction::RenderXml { document } => {
                self.begin_xml_render(document)?;
            }
            Instruction::RenderJsonnet { document } => {
                self.begin_jsonnet_render(document)?;
            }
            Instruction::RenderJson { document } => {
                self.begin_render(document, rendering::Format::Json)?
            }
            Instruction::RenderPlist { document } => {
                self.begin_render(document, rendering::Format::Plist)?;
            }
            Instruction::RenderYaml { document } => {
                self.begin_render(document, rendering::Format::Yaml)?;
            }
            Instruction::RenderProperties { document } => {
                self.begin_render(document, rendering::Format::Properties)?
            }
            Instruction::PublishModuleMember { code } => {
                let owner = self
                    .stack
                    .last()
                    .ok_or_else(|| invalid("missing module"))?
                    .as_object()?;
                let unit = self
                    .heap
                    .get(owner)?
                    .module_unit
                    .ok_or_else(|| invalid("missing module unit"))?;
                let template = self.program.units[unit.0 as usize].template;
                let member = self.program.objects[template]
                    .iter()
                    .find(|m| m.code == code)
                    .ok_or_else(|| invalid("missing module declaration"))?
                    .clone();
                if self.program.units[unit.0 as usize].mode == modules::ModuleMode::Extends
                    && let Some(parent) = self.heap.get(owner)?.parent
                {
                    self.validate_class_override(parent, &member)?;
                }
                let object = self.heap.get_mut(owner)?;
                if object.published_module_members.insert(code) {
                    object.members.push(member);
                }
            }
            Instruction::FinishModule => {
                let owner = self
                    .stack
                    .last()
                    .ok_or_else(|| invalid("missing module"))?
                    .as_object()?;
                let object = self.heap.get_mut(owner)?;
                object.module_initializing = false;
                object.class_initializing = false;
            }
            Instruction::CheckAnnotationClass => {
                let Some(Value::Class(id)) = self.stack.last().cloned() else {
                    return Err(RuntimeError::new(
                        ErrorKind::Type,
                        "Expected an annotation class.",
                    ));
                };
                let expected = self.canonical_builtin_class(classes::BuiltinClass::Ordinary(
                    annotations::ANNOTATION_ID,
                ));
                let valid = self.heap.is_subclass(id, expected);
                if !valid {
                    return Err(RuntimeError::new(
                        ErrorKind::Type,
                        "Expected an annotation class.",
                    ));
                }
            }
            Instruction::AppendAnnotation => self.append_annotation()?,
            Instruction::BeginMemberAnnotations => {
                let id = self.heap.allocate(Object::default());
                self.stack.push(Value::Object(id));
            }
            Instruction::InitializeDeprecation => self.initialize_deprecation()?,
            Instruction::StoreDeprecation => self.store_deprecation()?,
            Instruction::PrepareMethodCall {
                lookup,
                arity,
                after_arguments,
            } => {
                self.prepare_method_call(lookup, arity, after_arguments)?;
            }
            Instruction::PublishMemberAnnotations { code } => {
                self.publish_member_annotations(code)?
            }
            Instruction::PublishAnnotatedClassMember { template, index } => {
                let annotations = self.pop()?.as_object()?;
                let owner = self.heap.declaration_object(
                    self.stack
                        .last()
                        .ok_or_else(|| invalid("missing annotation owner"))?,
                )?;
                let member = self
                    .program
                    .objects
                    .get(template)
                    .and_then(|members| members.get(index))
                    .ok_or_else(|| invalid("missing annotated class member"))?
                    .clone();
                if let Some(parent) = self.heap.get(owner)?.parent {
                    self.validate_class_override(parent, &member)?;
                }
                self.publish_annotations(owner, annotations, member.code)?;
                self.heap.get_mut(owner)?.members.push(member);
            }
            Instruction::BeginTypeAlias {
                declaration,
                name,
                end,
            } => {
                if let Some(id) = self.alias_singletons.get(&declaration).copied() {
                    self.stack.push(Value::TypeAlias(id));
                    self.frames.last_mut().expect("active frame").pc = end;
                } else {
                    let environment = self
                        .frames
                        .last()
                        .expect("active frame")
                        .environment
                        .clone();
                    let name = self.current_module_name().concat(&name);
                    let id = self.heap.allocate(Object {
                        type_alias: Some(name),
                        type_alias_declaration: Some(declaration),
                        declaration_code: self.frames.last().map(|frame| frame.block),
                        environment,
                        ..Object::default()
                    });
                    self.alias_singletons.insert(declaration, id);
                    self.declaration_roots_dirty = true;
                    self.stack.push(Value::TypeAlias(id));
                }
            }
            Instruction::FinishTypeAlias => {
                let Some(Value::TypeAlias(id)) = self.stack.last().cloned() else {
                    return Err(invalid("missing alias initialization target"));
                };
                self.heap.get_mut(id)?.type_alias_initialized = true;
            }
            Instruction::CheckTypeAliasInitialized => {
                let Value::TypeAlias(id) = self.pop()? else {
                    return Err(invalid("missing alias type"));
                };
                if !self.heap.get(id)?.type_alias_initialized {
                    return Err(RuntimeError::new(
                        ErrorKind::Cycle,
                        "Type alias definitions must not be cyclic.",
                    ));
                }
            }

            Instruction::BuiltinTypeAlias(name) => {
                let id = self.heap.find_type_alias(&name).unwrap_or_else(|| {
                    self.heap.allocate(Object {
                        type_alias: Some(name),
                        type_alias_initialized: true,
                        ..Object::default()
                    })
                });
                self.stack.push(Value::TypeAlias(id));
            }
            Instruction::GuestCatch { or_null } => {
                self.begin_guest_catch(or_null)?;
            }
            Instruction::LoadBaseModule
            | Instruction::LoadRefModule
            | Instruction::LoadJsonnetModule
            | Instruction::LoadXmlModule
            | Instruction::LoadProtobufModule
            | Instruction::LoadPklBinaryModule => {
                unreachable!("bundled module preparation is dispatched before this frame")
            }

            Instruction::ReferenceRenderMethod => {
                let id = self.pop()?.as_object()?;
                let domain = self
                    .heap
                    .get(id)?
                    .reference
                    .as_ref()
                    .ok_or_else(|| invalid("reference render receiver"))?
                    .domain
                    .as_object()?;
                self.retire_saturated_method_cache(
                    &Value::Object(domain),
                    &"renderReference".into(),
                )?;
                self.force(
                    domain,
                    domain,
                    PklString::from("renderReference"),
                    false,
                    true,
                    true,
                )?;
            }
            Instruction::Swap
            | Instruction::BeginCollection { .. }
            | Instruction::EndCollection
            | Instruction::SetCollectionDefault { .. }
            | Instruction::SetCollectionTypeDefault { .. }
            | Instruction::GenericConstructorParent { .. }
            | Instruction::AmendEmpty
            | Instruction::AddCollectionProperty(_)
            | Instruction::AddCollectionElement(_)
            | Instruction::AddCollectionEntry { .. }
            | Instruction::CheckMapSpread
            | Instruction::OpenIterator { .. }
            | Instruction::IteratorNext { .. }
            | Instruction::EnterForScope { .. }
            | Instruction::LeaveForScope
            | Instruction::SpreadMember
            | Instruction::OpenPredicate
            | Instruction::PredicateNext { .. }
            | Instruction::AddPredicateMember(_)
            | Instruction::Subscript { .. }
            | Instruction::SuperSubscript { .. }
            | Instruction::EntryParent { .. }
            | Instruction::EntryDefault
            | Instruction::EntryKey
            | Instruction::MakeList { .. }
            | Instruction::PrepareMapEntry
            | Instruction::PrepareSetElement
            | Instruction::AddSetElement
            | Instruction::PutSetKey
            | Instruction::AddMapEntry => {
                return Err(invalid("collection instruction bypassed scheduler"));
            }
            Instruction::Constant(value) => self.stack.push(value),
            Instruction::WrapNullableDefault { defined } => {
                let default = if defined { Some(self.pop()?) } else { None };
                self.stack.push(self.heap.allocate_null(default));
            }
            Instruction::StripNullDefault => {
                if self.stack.last().is_some_and(Value::is_null) {
                    *self.stack.last_mut().expect("null value") = Value::Null;
                }
            }
            Instruction::Input(index) => self.stack.push(
                self.inputs
                    .get(index)
                    .cloned()
                    .ok_or_else(|| invalid("missing input"))?,
            ),
            Instruction::StoreLocal(depth) => {
                let value = self.pop()?;
                let locals = &mut self
                    .frames
                    .last_mut()
                    .expect("active frame")
                    .environment
                    .locals;
                let index = locals
                    .len()
                    .checked_sub(depth + 1)
                    .ok_or_else(|| invalid("invalid local slot"))?;
                locals[index] = value;
            }
            Instruction::LoadLocal(depth) => {
                let locals = &frame.environment.locals;
                let offset = depth
                    .checked_add(1)
                    .ok_or_else(|| invalid("invalid lexical binding"))?;
                let index = locals
                    .len()
                    .checked_sub(offset)
                    .ok_or_else(|| invalid("invalid lexical binding"))?;
                self.stack.push(locals[index].clone());
            }
            Instruction::PushLocal => {
                let value = self.pop()?;
                self.bind_local(None, identity::SlotPolicy::Adaptive, value)?;
            }
            Instruction::PopLocal => {
                frame
                    .environment
                    .copy_locals
                    .remove(&frame.environment.locals.len().saturating_sub(1));
                frame
                    .environment
                    .local_owners
                    .remove(&frame.environment.locals.len().saturating_sub(1));
                if frame.environment.locals.pop().is_none() {
                    return Err(invalid("lexical stack underflow"));
                }
            }
            Instruction::LoadThis => {
                let receiver = self.this_receiver()?;
                self.stack.push(receiver);
            }
            Instruction::LoadCustomThis => {
                let slot = frame
                    .environment
                    .custom_this
                    .ok_or_else(|| invalid("missing custom receiver slot"))?;
                self.stack.push(
                    self.heap
                        .custom_this(slot)?
                        .ok_or_else(|| invalid("missing custom receiver"))?,
                );
            }
            Instruction::LoadOuter => {
                if let Some(value) = frame.environment.outer_value.clone() {
                    self.stack.push(value);
                    return Ok(());
                }
                let depth = usize::from(!frame.lambda);
                let scope = frame.environment.scopes.get(depth).ok_or_else(|| {
                    RuntimeError::new(
                        ErrorKind::MissingMember,
                        "Top-level scope does not have an outer scope.",
                    )
                })?;
                self.stack.push(Value::Object(scope.receiver));
            }
            Instruction::LoadModule => {
                let scope = frame
                    .environment
                    .scopes
                    .last()
                    .ok_or_else(|| invalid("missing module scope"))?;
                self.stack.push(Value::Object(scope.receiver));
            }
            Instruction::ObserveNativeLiteral { span, rendered } => {
                if self.active_type_trackers != 0 {
                    self.observe_native_literal(span, rendered);
                }
            }
            Instruction::Observe(span) => {
                if self.active_type_trackers == 0 {
                    return Ok(());
                }
                let value = self
                    .stack
                    .last()
                    .cloned()
                    .ok_or_else(|| invalid("missing observed value"))?;
                self.observe(span, value);
            }
            Instruction::NonNull => {
                let value = self.pop()?;
                if value.is_null() {
                    return Err(RuntimeError::new(
                        ErrorKind::Type,
                        "Expected a non-null value, but got `null`.",
                    ));
                }
                self.stack.push(value);
            }
            Instruction::Trace { operand_span } => {
                self.begin_trace(operand_span)?;
            }
            Instruction::Throw => {
                let value = self.pop()?;
                let Value::String(message) = value else {
                    return Err(type_error(
                        &TypeCheck::String,
                        &value,
                        &self.program.module_name,
                        &self.heap,
                    )?);
                };
                let mut error = RuntimeError::new(ErrorKind::Type, message.clone());
                error.java_exception_message =
                    Some(("org.pkl.core.runtime.VmEvalException", message));
                return Err(error);
            }
            Instruction::StringRepeat => self.repeat_string()?,
            Instruction::CollectionJoin => self.begin_collection_join()?,
            Instruction::ObjectLookup(operation) => self.object_lookup(operation)?,
            Instruction::ModuleRelativePathTo => self.module_relative_path_to()?,
            Instruction::ObjectDefault => self.object_default()?,
            Instruction::EmptyDynamic => {
                let empty = self.empty_object(crate::heap::CollectionKind::Dynamic);
                self.stack.push(Value::Object(empty));
            }
            Instruction::ObjectDefaultProperty => {
                let receiver = self.conversion_receiver()?;
                if !self.object_default_member(receiver)? {
                    return Err(invalid("object default receiver"));
                }
            }
            Instruction::BeginConversion(operation) => self.begin_conversion(operation)?,
            Instruction::ConversionNext { end } => self.conversion_next(end)?,
            Instruction::ConversionValue => self.conversion_value()?,
            Instruction::ConversionPair => self.conversion_pair()?,
            Instruction::AppendConvertedMember => self.append_converted_member()?,
            Instruction::EndConversion => self.end_conversion()?,
            Instruction::ConvertObject(operation) => self.convert_object(operation)?,
            Instruction::ReadConversionProperty => self.read_conversion_property()?,
            Instruction::BeginLazyCollection(operation) => self.begin_lazy_collection(operation)?,
            Instruction::LazyCollectionNext { end } => self.lazy_collection_next(end)?,
            Instruction::LazyCollectionAccept { next, end } => {
                self.lazy_collection_accept(next, end)?
            }
            Instruction::EndLazyCollection => self.end_lazy_collection()?,
            Instruction::BeginCollectionEvery => self.begin_collection_every()?,
            Instruction::CollectionEveryAccept { next } => self.collection_every_accept(next)?,
            Instruction::EndCollectionEvery => self.end_collection_every()?,
            Instruction::BeginCollectionTransform(operation) => {
                self.begin_collection_transform(operation)?
            }
            Instruction::BeginCollectionProjection(operation) => {
                self.begin_collection_projection(operation)?
            }
            Instruction::CollectionKeyOrder => self.collection_key_order()?,
            Instruction::CollectionTransformNext { end } => self.collection_transform_next(end)?,
            Instruction::CollectionTransformProject { next } => {
                self.collection_transform_project(next)?
            }
            Instruction::CollectionFlatMapNext { end } => self.collection_flat_map_next(end)?,
            Instruction::AppendStrictElement => self.append_strict_element()?,
            Instruction::EndCollectionTransform => self.end_collection_transform()?,
            Instruction::CollectionDistinctStep { next, end } => {
                self.collection_distinct_step(next, end)?
            }
            Instruction::CollectionContainsStep { matched } => {
                self.collection_contains_step(matched)?
            }
            Instruction::StringEncode => self.encode_string()?,
            Instruction::Primitive(operation) => self.primitive(operation)?,
            Instruction::CheckObjectParameters { target } => {
                self.check_object_parameters(target)?
            }
            Instruction::PrepareFunctionAmendment { body, arity } => {
                self.prepare_function_amendment(body, arity)?
            }
            Instruction::BeginFunctionAmendment { end, function } => {
                self.begin_function_amendment(end, function)?
            }
            Instruction::EndFunctionAmendment => {}
            Instruction::InvokeAmendedParent => self.invoke_amended_parent()?,
            Instruction::ContinueFunctionAmendment => self.continue_function_amendment()?,
            Instruction::CheckMixinArgument => self.check_mixin_argument()?,
            Instruction::RegexConstructor => {
                let value = self.regex_constructor()?;
                self.stack.push(value);
            }
            Instruction::MakeRegex => self.make_regex()?,
            Instruction::RegexMatchProperty(property) => self.regex_match_property(property)?,
            Instruction::ReflectCall(operation) => self.begin_reflection(operation)?,
            Instruction::JsonParse => self.begin_json_parse()?,
            Instruction::YamlParse { all } => self.begin_yaml_parse(all)?,
            Instruction::MirrorProperty(name) => self.begin_mirror_property(name)?,
            Instruction::IntSeqConstructor => {
                let value = self.int_seq_constructor();
                self.stack.push(value);
            }
            Instruction::MakeIntSeq => self.make_int_seq()?,
            Instruction::CheckIntSeqStep => self.check_int_seq_step()?,
            Instruction::BeginIntSeq(operation) => self.begin_int_seq(operation)?,
            Instruction::IntSeqNext { end } => self.int_seq_next(end)?,
            Instruction::IntSeqAccept => self.int_seq_accept()?,
            Instruction::EndIntSeq => self.end_int_seq()?,
            Instruction::PairConstructor => {
                let value = self.pair_constructor();
                self.stack.push(value);
            }
            Instruction::NullConstructor => {
                let value = self.null_constructor();
                self.stack.push(value);
            }
            Instruction::CheckNullDefault => self.check_null_default()?,
            Instruction::MakePair => {
                let second = self.pop()?;
                let first = self.pop()?;
                let value = self.make_pair(first, second);
                self.stack.push(value);
            }
            Instruction::JavaValueString => {
                let value = self.pop()?;
                self.java_value_string(value)?;
            }
            Instruction::CheckInterpolate
                if matches!(self.stack.last(), Some(Value::String(_) | Value::Int(_))) => {}
            Instruction::FinishInterpolation(count) => {
                finish_interpolation_on_stack(&mut self.stack, count, &mut self.scratch_utf8)?;
            }
            Instruction::CheckInterpolate | Instruction::ToString => {
                let value = self.pop()?;
                if let Value::Object(id) = value
                    && self.heap.get(id)?.reference.is_some()
                {
                    self.render_reference(id)?;
                } else if self.is_set(&value) || self.is_list(&value) {
                    let text = crate::diagnostic_values::render_multiline(
                        &value,
                        &self.heap,
                        &self.program.module_name,
                    )?;
                    self.stack.push(Value::string(text));
                } else if matches!(
                    value,
                    Value::TypeAlias(_) | Value::Class(_) | Value::Function(_)
                ) || self.is_map(&value)
                {
                    let text = crate::diagnostic_values::render_string(
                        &value,
                        &self.heap,
                        &self.program.module_name,
                        usize::MAX,
                    )?;
                    self.stack.push(Value::string(text));
                } else if let Value::Object(id) = value {
                    if collections::typed_class(&self.heap, id)?.is_some()
                        || self.heap.get(id)?.is_module
                    {
                        // Pkl interpolation virtually invokes Typed.toString; user
                        // overrides can avoid forcing the receiver altogether.
                        let site = self.current_method_call_site()?;
                        let code_owner = self
                            .frames
                            .last()
                            .expect("ToString frame")
                            .environment
                            .code_owner;
                        let function = self.heap.allocate(Object {
                            function: Some(Function {
                                block: self.program.typed_string_block,
                                arity: 0,
                                method: true,
                                captured_this: None,
                            }),
                            environment: Environment {
                                locals: vec![Value::Object(id)],
                                code_owner,
                                ..Environment::default()
                            },
                            ..Object::default()
                        });
                        self.invoke_function(function, Vec::new(), true)?;
                        self.frames
                            .last_mut()
                            .expect("implicit ToString call")
                            .method_call_site = Some(Box::new(site));
                    } else {
                        self.begin_object_string(Value::Object(id))?;
                    }
                } else {
                    self.stack.push(Value::string(scalar_string(&value)?));
                }
            }

            Instruction::JumpIfNull(target) => {
                if self.pop()?.is_null() {
                    self.jump(target)?;
                }
            }
            Instruction::RequireConstProperty {
                name,
                owner_depth,
                local,
                method,
            } => {
                let scope = self.scope(owner_depth)?;
                if local && !method {
                    let owner = self.current_executable()?;
                    let frame = self.frames.last().expect("active frame");
                    // The following GetProperty is the read node's execution site.
                    let site = (frame.block, frame.pc);
                    if self.heap.resolved_local(owner, site)?.is_none() {
                        let member = self
                            .heap
                            .get(scope.owner)?
                            .members
                            .iter()
                            .find(|member| {
                                member.name == name
                                    && member.local
                                    && !member.method
                                    && !member.entry
                            })
                            .cloned();
                        if let Some(member) = member {
                            // Java publishes ObjectMember before its const check can fail.
                            self.heap.publish_local(owner, site, member)?;
                            self.require_const(scope.owner, &name, true, false)?;
                        }
                    }
                } else {
                    self.require_const(
                        if local { scope.owner } else { scope.receiver },
                        &name,
                        local,
                        method,
                    )?;
                }
            }
            Instruction::GetMember(_)
            | Instruction::GetMethod(_)
            | Instruction::GetImplicitCustomProperty(_)
            | Instruction::GetImplicitCustomMethod(_)
            | Instruction::GetConstMember(_)
            | Instruction::GetConstMethod(_)
            | Instruction::GetAmbiguousProperty { .. }
            | Instruction::GetProperty { .. }
            | Instruction::GetMethodProperty { .. }
            | Instruction::GetImplicitProperty(_)
            | Instruction::GetImplicitMethod(_)
            | Instruction::InferPropertyParent
            | Instruction::GetSuperMember(_)
            | Instruction::GetSuperMethod(_)
            | Instruction::GetConstSuperMember(_)
            | Instruction::GetConstSuperMethod(_) => {
                unreachable!("member access instruction uses its bounded dispatcher")
            }

            Instruction::Fail(error) => return Err(error),

            Instruction::Undefined(name) => {
                return Err(RuntimeError::new(
                    ErrorKind::MissingMember,
                    PklString::from("Tried to read property `")
                        .concat(&name)
                        .concat(&"` but its value is undefined.".into()),
                )
                .undefined());
            }
            Instruction::AbstractMember { name, method } => {
                let kind = if method { "method" } else { "property" };
                return Err(RuntimeError::new(
                    ErrorKind::Type,
                    PklString::from(format!("Cannot invoke abstract {kind} `"))
                        .concat(self.current_module_name())
                        .concat(&name)
                        .concat(&"`.".into()),
                ));
            }
            Instruction::Unsupported(message) => {
                return Err(RuntimeError::new(ErrorKind::Unsupported, message));
            }
            Instruction::BeginClass {
                template,
                class,
                end,
            } => {
                if let Some(id) = self.class_singletons.get(&template).copied() {
                    self.stack.push(Value::Class(id));
                    self.frames.last_mut().expect("active frame").pc = end;
                } else {
                    let environment = self
                        .frames
                        .last()
                        .expect("active frame")
                        .environment
                        .clone();
                    let declaration_code = self.frames.last().map(|frame| frame.block);
                    // Primary Programs can acquire their source after bytecode
                    // compilation. Resolve the missing URI from this class's
                    // declaring block, never from the renderer or its caller.
                    let origin_uri = class.origin_uri.clone().or_else(|| {
                        let metadata =
                            declaration_code.and_then(|code| self.program.metadata.get(code))?;
                        metadata
                            .unit
                            .and_then(|unit| self.program.units.get(unit.0 as usize))
                            .and_then(|unit| {
                                unit.identity
                                    .as_ref()
                                    .map(|identity| identity.original_uri.clone())
                                    .or_else(|| {
                                        unit.source.as_ref().map(|source| source.uri.clone())
                                    })
                            })
                            .or_else(|| metadata.source.as_ref().map(|source| source.uri.clone()))
                    });
                    let id = self.heap.allocate(Object {
                        class_definition: Some(Class {
                            origin_uri,
                            module: class
                                .module
                                .or_else(|| Some(self.current_module_name().into())),
                            ..class
                        }),
                        class_initializing: true,
                        declaration_code,
                        environment,
                        ..Object::default()
                    });
                    let class = self.heap.allocate_class(id);
                    self.class_singletons.insert(template, class);
                    self.declaration_roots_dirty = true;
                    self.stack.push(Value::Class(class));
                }
            }
            Instruction::SetClassParent => {
                let Value::Class(parent) = self.pop()? else {
                    return Err(RuntimeError::new(
                        ErrorKind::Type,
                        "Superclass must be a class.",
                    ));
                };
                let Some(Value::Class(id)) = self.stack.last().cloned() else {
                    return Err(invalid("missing class initialization target"));
                };
                let parent_class = self.heap.class_definition(parent)?;
                if id == parent {
                    return Err(RuntimeError::new(
                        ErrorKind::Type,
                        PklString::from("Class `")
                            .concat(
                                parent_class
                                    .module
                                    .as_ref()
                                    .unwrap_or(&self.program.module_name),
                            )
                            .concat(&"#".into())
                            .concat(&parent_class.name)
                            .concat(&"` cannot extend itself.".into()),
                    ));
                }
                if !parent_class.open && !parent_class.abstract_ {
                    let mut error = RuntimeError::new(
                        ErrorKind::Type,
                        PklString::from("Cannot extend non-open class `")
                            .concat(&reference::type_name(
                                &TypeCheck::Class {
                                    module: parent_class.module.clone(),
                                    id: parent_class.id,
                                    name: parent_class.name.clone(),
                                },
                                &self.program.module_name,
                            ))
                            .concat(&"`.".into()),
                    );
                    error.hint = Some("By default, classes are closed for extension.\nTo make a class extensible, add an `open` modifier: `open class MyClass { ... }`".into());
                    return Err(error);
                }
                // Upstream ClassNode uses PClassInfo's module-name predicate here.
                if self.class_external(parent)?
                    && !self
                        .heap
                        .class_definition(id)?
                        .module
                        .as_ref()
                        .is_some_and(|name| name.starts_with("pkl."))
                {
                    let mut error = RuntimeError::new(
                        ErrorKind::Type,
                        PklString::from("Cannot extend external class `")
                            .concat(&self.heap.class_display_name(parent)?)
                            .concat(&"`.".into()),
                    );
                    error.hint = Some(
                        "External classes can only be extended by standard library classes.".into(),
                    );
                    return Err(error);
                }
                self.heap.link_superclass(id, Some(parent))?;
            }
            Instruction::SetDefaultClassParent => {
                let Some(Value::Class(id)) = self.stack.last().cloned() else {
                    return Err(invalid("missing class initialization target"));
                };
                let parent = self.canonical_builtin_class(classes::BuiltinClass::Typed);
                self.heap.link_superclass(id, Some(parent))?;
            }
            Instruction::PublishClassMember { template, index } => {
                let Some(Value::Class(id)) = self.stack.last().cloned() else {
                    return Err(invalid("missing class initialization target"));
                };
                let member = self
                    .program
                    .objects
                    .get(template)
                    .and_then(|members| members.get(index))
                    .ok_or_else(|| invalid("missing class member"))?
                    .clone();
                let prototype = self.heap.get_class(id)?.prototype;
                if let Some(parent) = self.heap.get(prototype)?.parent {
                    self.validate_class_override(parent, &member)?;
                }
                self.heap.get_mut(prototype)?.members.push(member);
            }
            Instruction::FinishClass => {
                let Some(Value::Class(id)) = self.stack.last().cloned() else {
                    return Err(invalid("missing class initialization target"));
                };
                let prototype = self.heap.get_class(id)?.prototype;
                self.heap.get_mut(prototype)?.class_initializing = false;
            }
            Instruction::MakeInstance { template }
            | Instruction::MakeBuiltinInstance { template } => {
                let builtin = matches!(instruction, Instruction::MakeBuiltinInstance { .. });
                let Value::Class(parent) = self.pop()? else {
                    return Err(RuntimeError::new(ErrorKind::Type, "Expected a class."));
                };
                if self.class_external(parent)? {
                    let name = self.heap.class_display_name(parent)?;
                    let mut error = RuntimeError::new(
                        ErrorKind::Type,
                        format!(
                            "Cannot instantiate, or amend an instance of, external class `{name}`."
                        ),
                    );
                    error.hint = Some("To create an instance of an external class, use its literal syntax or constructor method.\nExamples:\n* `123` uses literal syntax to create an instance of class `Int`.\n* `Pair(1, 2)` uses the `Pair()` constructor method to create an instance of class `Pair`.".into());
                    return Err(error);
                }
                let class_id = parent;
                let parent = self.heap.get_class(class_id)?.prototype;
                let class = self
                    .heap
                    .get(parent)?
                    .class_definition
                    .as_ref()
                    .ok_or_else(|| invalid("invalid class handle"))?;
                self.check_class_instantiable(class)?;
                let members = if builtin {
                    self.program.builtin_objects.get(template)
                } else {
                    self.program.objects.get(template)
                }
                .ok_or_else(|| invalid("invalid instance template"))?
                .clone();
                self.validate_object_members(parent, &members)?;
                if members.is_empty() {
                    self.stack.push(Value::Object(parent));
                    return Ok(());
                }
                let environment = self
                    .frames
                    .last()
                    .expect("active frame")
                    .environment
                    .clone();
                let id = self.heap.allocate(Object {
                    parent: Some(parent),
                    instance_class: Some(class_id),
                    members: members.into(),
                    environment,
                    ..Object::default()
                });
                self.stack.push(Value::Object(id));
            }
            Instruction::CheckByte => self.check_byte()?,
            Instruction::MakeBytes { length } => {
                if self.stack.len() < length {
                    return Err(invalid("Bytes literal stack"));
                }
                let values = self.stack.split_off(self.stack.len() - length);
                let bytes = values
                    .into_iter()
                    .map(|value| match value {
                        Value::Int(value) => Ok(value.get() as u8),
                        _ => Err(invalid("checked Bytes argument")),
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                self.stack.push(Value::Bytes(bytes.into()));
            }
            Instruction::DefaultClassInstance => {
                let Value::Class(id) = self.pop()? else {
                    return Err(RuntimeError::new(ErrorKind::Type, "Expected a class."));
                };
                let id = self.heap.get_class(id)?.prototype;
                if self
                    .heap
                    .get(id)?
                    .class_definition
                    .as_ref()
                    .is_none_or(|class| class.abstract_)
                {
                    return Err(RuntimeError::new(
                        ErrorKind::Type,
                        "Abstract class has no default value.",
                    ));
                }
                self.stack.push(Value::Object(id));
            }
            Instruction::MakeObject {
                template,
                has_parent,
            } => {
                let module = self
                    .program
                    .units
                    .iter()
                    .find(|unit| unit.template == template)
                    .cloned();
                let parent = if has_parent {
                    Some(self.pop()?.as_object()?)
                } else if module
                    .as_ref()
                    .is_some_and(|unit| unit.mode == modules::ModuleMode::Plain)
                {
                    let class = self
                        .canonical_builtin_class(classes::BuiltinClass::Ordinary(output::MODULE));
                    Some(self.heap.get_class(class)?.prototype)
                } else {
                    None
                };
                if let Some(id) = parent
                    && self.heap.get(id)?.reference.is_some()
                {
                    return Err(collections::external_amendment_error("pkl.ref#Reference"));
                }
                let members = if let Some(module) = &module {
                    if module.mode == modules::ModuleMode::Amends {
                        Vec::new()
                    } else {
                        module.initial_members.clone()
                    }
                } else {
                    self.program
                        .objects
                        .get(template)
                        .ok_or_else(|| invalid("invalid object template"))?
                        .clone()
                };
                if let Some(parent) = parent
                    && module.is_none()
                {
                    self.validate_object_members(parent, &members)?;
                    if members.is_empty() {
                        self.stack.push(Value::Object(parent));
                        return Ok(());
                    }
                }
                let environment = self
                    .frames
                    .last()
                    .expect("active frame")
                    .environment
                    .clone();
                let definition = Object {
                    ref_module: module.as_ref().is_some_and(|unit| {
                        unit.standard_library
                            && unit
                                .identity
                                .as_ref()
                                .is_some_and(|identity| identity.original_uri == "pkl:ref")
                    }),
                    is_module: module
                        .as_ref()
                        .is_some_and(|unit| unit.mode != modules::ModuleMode::Amends),
                    module_unit: module.as_ref().map(|unit| unit.id),
                    class_definition: module
                        .as_ref()
                        .filter(|unit| unit.mode != modules::ModuleMode::Amends)
                        .map(|unit| Class {
                            module: None,
                            origin_uri: unit
                                .identity
                                .as_ref()
                                .map(|identity| identity.original_uri.clone())
                                .or_else(|| unit.source.as_ref().map(|source| source.uri.clone())),
                            id: unit.class_id,
                            name: if unit.id.0 == 0 {
                                self.program.module_name.clone()
                            } else {
                                unit.name.clone()
                            },
                            open: unit.modifiers & crate::syntax::ModifierKind::Open.flag() != 0,
                            abstract_: unit.modifiers
                                & crate::syntax::ModifierKind::Abstract.flag()
                                != 0,
                        }),
                    class_initializing: module
                        .as_ref()
                        .is_some_and(|unit| unit.mode == modules::ModuleMode::Extends),
                    class_superclass_initialized: module
                        .as_ref()
                        .is_some_and(|unit| unit.mode == modules::ModuleMode::Plain),
                    module_initializing: module.is_some(),
                    published_module_members: if module.is_some() {
                        members.iter().map(|m| m.code).collect()
                    } else {
                        BTreeSet::new()
                    },
                    parent,
                    members: members.into(),
                    environment,
                    ..Object::default()
                };
                let existing = self
                    .frames
                    .last()
                    .and_then(|frame| frame.module_receiver)
                    .filter(|_| module.is_some());
                let object = if let Some(receiver) = existing {
                    *self.heap.get_mut(receiver)? = definition;
                    receiver
                } else {
                    self.heap.allocate(definition)
                };
                if self.heap.get(object)?.class_definition.is_some() {
                    let class = self.heap.allocate_class(object);
                    if let Some(parent) = parent {
                        let superclass = self
                            .heap
                            .object_class(parent)?
                            .ok_or_else(|| invalid("module superclass"))?;
                        self.heap.link_superclass(class, Some(superclass))?;
                    }
                } else if let Some(parent) = parent {
                    let class = self.heap.object_class(parent)?;
                    self.heap.get_mut(object)?.instance_class = class;
                }
                if let Some(unit) = &module {
                    self.frames.last_mut().unwrap().module_receiver = Some(object);
                    if let Some(identity) = &unit.identity {
                        if let Some(key) = unit.singleton_cache_key() {
                            self.modules_stdlib
                                .insert(key.into(), modules::CachedModule::Value(object));
                        } else {
                            self.modules_original.insert(
                                identity.original_uri.clone(),
                                modules::CachedModule::Value(object),
                            );
                            self.last_warm_module = None;
                            self.modules_resolved.insert(
                                identity.resolved_uri.clone(),
                                modules::CachedModule::Value(object),
                            );
                        }
                    }
                }
                self.stack.push(Value::Object(object));
                if module.is_some() {
                    self.frames
                        .last_mut()
                        .expect("module frame")
                        .environment
                        .scopes
                        .push(Scope {
                            receiver: object,
                            owner: object,
                        });
                }
            }
            Instruction::BeginObjectMethodSignature => {
                let frame = self.frames.last_mut().expect("method member frame");
                let owner = frame
                    .force
                    .as_ref()
                    .ok_or_else(|| invalid("method signature owner"))?
                    .owner;
                frame.environment.this_value = Some(Value::Object(owner));
                if !frame.environment.scopes.set_first_receiver(owner) {
                    return Err(invalid("method signature scope"));
                }
            }
            Instruction::EndObjectMethodSignature => {
                let frame = self.frames.last_mut().expect("method member frame");
                let receiver = frame
                    .force
                    .as_ref()
                    .ok_or_else(|| invalid("method signature receiver"))?
                    .receiver;
                frame.environment.this_value = Some(Value::Object(receiver));
                if !frame.environment.scopes.set_first_receiver(receiver) {
                    return Err(invalid("method signature scope"));
                }
            }
            Instruction::MakeFunction {
                block,
                arity,
                method,
                capture_custom_this,
            } => {
                if self.program.blocks.get(block).is_none() {
                    return Err(invalid("invalid function block"));
                }
                let environment = self
                    .frames
                    .last()
                    .expect("active frame")
                    .environment
                    .clone();
                let mut environment = environment;
                if let Some(parent) = environment.executable_owner {
                    let frame = self.frames.last().expect("active frame");
                    environment.executable_owner = Some(self.heap.executable_child(
                        parent,
                        ExecutionSite::Function {
                            block: frame.block,
                            instruction: frame.pc - 1,
                        },
                    )?);
                }
                let captured_this = if capture_custom_this {
                    let slot = environment
                        .custom_this
                        .ok_or_else(|| invalid("missing captured receiver slot"))?;
                    self.heap.custom_this(slot)?
                } else {
                    None
                };
                let method_declaration = method
                    .then(|| {
                        self.frames.last().and_then(|frame| {
                            frame.force.as_ref().map(|force| (force.owner, frame.block))
                        })
                    })
                    .flatten();
                let id = self.heap.allocate_function(
                    Function {
                        block,
                        arity,
                        method,
                        captured_this,
                    },
                    method_declaration,
                    environment,
                );
                self.stack.push(Value::Function(id));
            }
            Instruction::ApplyEntryDefault => {
                let argument = self.pop()?;
                let Value::Function(id) = self.pop()? else {
                    return Err(invalid("entry default is not a function"));
                };
                // ApplyVmFunction1Node has no guest source section. Keep the
                // enclosing member alive, but omit this synthetic call site.
                self.frames
                    .last_mut()
                    .expect("active frame")
                    .callsite_hidden = true;
                self.invoke_function(id, vec![argument], false)?;
            }
            Instruction::Call { arity, method } => {
                let base = self.frames.last().expect("active frame").base;
                if self.stack.len().saturating_sub(base) < arity.saturating_add(1) {
                    return Err(invalid("call operand stack underflow"));
                }
                let arguments = self.stack.split_off(self.stack.len() - arity);
                let Value::Function(id) = self.pop()? else {
                    return Err(RuntimeError::new(ErrorKind::Type, "value is not callable"));
                };
                self.invoke_function(id, arguments, method)?;
            }
            Instruction::PipeApply => {
                let right = self.pop()?;
                let left = self.pop()?;
                let Value::Function(id) = right else {
                    return Err(RuntimeError::new(
                        ErrorKind::Type,
                        PklString::join(
                            &[
                                "Operator `|>` is not defined for operand types `".into(),
                                (crate::diagnostic_values::type_name(
                                    &left,
                                    &self.heap,
                                    &self.program.module_name,
                                )?),
                                "` and `".into(),
                                (crate::diagnostic_values::type_name(
                                    &right,
                                    &self.heap,
                                    &self.program.module_name,
                                )?),
                                "`.\nLeft operand : ".into(),
                                crate::diagnostic_values::render(
                                    &left,
                                    &self.heap,
                                    &self.program.module_name,
                                    80,
                                )?,
                                "\nRight operand: ".into(),
                                crate::diagnostic_values::render(
                                    &right,
                                    &self.heap,
                                    &self.program.module_name,
                                    80,
                                )?,
                            ],
                            "",
                        ),
                    ));
                };
                self.frames
                    .last_mut()
                    .expect("active frame")
                    .callsite_hidden = true;
                self.invoke_function(id, vec![left], false)?;
            }

            Instruction::Duplicate => {
                let value = self.pop()?;
                self.stack.push(value.clone());
                self.stack.push(value);
            }
            Instruction::RotateStackRight(length) => {
                let start = self
                    .stack
                    .len()
                    .checked_sub(length)
                    .ok_or_else(|| invalid("lexical call stack operands"))?;
                self.stack[start..].rotate_right(1);
            }
            Instruction::Pop => {
                self.pop()?;
            }
            Instruction::Negate => {
                let value = match self.pop()? {
                    Value::Int(value) => Value::Int(
                        crate::numeric::subtract(0, *value)
                            .map_err(numeric_error)?
                            .into(),
                    ),
                    Value::Float(value) => Value::Float((-value.get()).into()),
                    Value::Duration(value) => Value::Duration(crate::duration::Duration::new(
                        -value.value(),
                        value.unit(),
                    )),
                    Value::DataSize(value) => Value::DataSize(crate::data_size::DataSize::new(
                        -value.value(),
                        value.unit(),
                    )),
                    value => return Err(unary_error("-", &value)),
                };
                self.stack.push(value);
            }
            Instruction::Not => {
                let operand = self.pop()?;
                let Value::Bool(value) = operand else {
                    return Err(unary_error("!", &operand));
                };
                self.stack.push(Value::Bool(!value));
            }
            Instruction::CheckLogicalLeft { or } | Instruction::CheckLogicalRight { or } => {
                let value = self.pop()?;
                if !matches!(value, Value::Bool(_)) {
                    let operator = if or { "||" } else { "&&" };
                    let name = crate::diagnostic_values::type_name(
                        &value,
                        &self.heap,
                        &self.program.module_name,
                    )?;
                    let rendered = crate::diagnostic_values::render(
                        &value,
                        &self.heap,
                        &self.program.module_name,
                        80,
                    )?;
                    let message = if matches!(instruction, Instruction::CheckLogicalLeft { .. }) {
                        PklString::join(&[format!("Operator `{operator}` is not defined for left operand type `{name}`.\nLeft operand: ").into(), rendered], "")
                    } else {
                        PklString::join(&[format!("Operator `{operator}` is not defined for operand types `Boolean` and `{name}`.\nLeft operand : {}\nRight operand: ", !or).into(), rendered], "")
                    };
                    return Err(RuntimeError::new(ErrorKind::Type, message));
                }
                self.stack.push(value);
            }
            Instruction::CheckBoolean => {
                let value = self.pop()?;
                if !matches!(value, Value::Bool(_)) {
                    return Err(RuntimeError::new(
                        ErrorKind::Type,
                        "logical operand must be Boolean",
                    ));
                }
                self.stack.push(value);
            }
            Instruction::JumpIfNotNull(target) => {
                if !self.pop()?.is_null() {
                    self.jump(target)?;
                }
            }
            Instruction::AddIntConstant(right) => {
                let left = self.pop()?;
                let value = match left {
                    Value::Int(left) => Value::Int(
                        crate::numeric::add(*left, right)
                            .map_err(numeric_error)?
                            .into(),
                    ),
                    Value::Float(left) => Value::Float((left.get() + right as f64).into()),
                    left => {
                        return Err(binary_error(
                            "+",
                            &left,
                            &Value::Int(right.into()),
                            &self.heap,
                            &self.program.module_name,
                        )?);
                    }
                };
                self.stack.push(value);
            }
            Instruction::Equal | Instruction::NotEqual => {
                let right = self.pop()?;
                let left = self.pop()?;
                if matches!((&left, &right), (Value::Object(_), Value::Object(_))) {
                    self.begin_equality(left, right, matches!(instruction, Instruction::NotEqual));
                } else {
                    self.stack.push(binary(
                        &instruction,
                        left,
                        right,
                        &self.heap,
                        &self.program.module_name,
                    )?);
                }
            }
            Instruction::Add
            | Instruction::Subtract
            | Instruction::Multiply
            | Instruction::Divide
            | Instruction::Less
            | Instruction::LessEqual
            | Instruction::Greater
            | Instruction::GreaterEqual
            | Instruction::IntegerDivide
            | Instruction::Remainder
            | Instruction::Power => {
                if matches!(instruction, Instruction::Add)
                    && self
                        .frames
                        .last()
                        .is_some_and(|frame| frame.collection_addition.is_some())
                {
                    return self.step_collection_addition();
                }
                let right = self.pop()?;
                let left = self.pop()?;
                if matches!(instruction, Instruction::Add)
                    && self.begin_collection_addition(&left, &right)?
                {
                    return Ok(());
                }
                self.stack.push(binary(
                    &instruction,
                    left,
                    right,
                    &self.heap,
                    &self.program.module_name,
                )?);
            }
            Instruction::Jump(target) => {
                self.jump(target)?;
            }
            Instruction::JumpIfFalse(target) => match self.pop()? {
                Value::Bool(false) => self.jump(target)?,
                Value::Bool(true) => {}
                value => {
                    return Err(type_error(
                        &TypeCheck::Boolean,
                        &value,
                        &self.program.module_name,
                        &self.heap,
                    )?);
                }
            },
            Instruction::ReadResource
            | Instruction::ReadResourceOrNull
            | Instruction::ReadResourceGlob(_) => {
                let nullable = matches!(instruction, Instruction::ReadResourceOrNull);
                let glob_spec = match instruction {
                    Instruction::ReadResourceGlob(spec) => Some(spec),
                    _ => None,
                };
                let glob = glob_spec.is_some();
                if self.callback_closing {
                    return Err(RuntimeError::new(ErrorKind::Closed, "evaluator is closing"));
                }
                let uri = match self.pop()? {
                    Value::String(uri) => uri,
                    operand => {
                        let operator = if glob {
                            "read*"
                        } else if nullable {
                            "read?"
                        } else {
                            "read"
                        };
                        return Err(RuntimeError::new(
                            ErrorKind::Type,
                            PklString::join(
                                &[
                                    format!(
                                        "Operator `{operator}` is not defined for operand type `"
                                    )
                                    .into(),
                                    (crate::diagnostic_values::type_name(
                                        &operand,
                                        &self.heap,
                                        &self.program.module_name,
                                    )?),
                                    "`.\nOperand: ".into(),
                                    crate::diagnostic_values::render(
                                        &operand,
                                        &self.heap,
                                        &self.program.module_name,
                                        80,
                                    )?,
                                ],
                                "",
                            ),
                        ));
                    }
                };
                if let Some(spec) = glob_spec {
                    return self.begin_glob(spec, uri);
                }
                if self.in_stdlib_bootstrap() {
                    // StdLibModule's private context contains no caller properties
                    // or environment. Only base's nullable format read is required.
                    if nullable && uri == "prop:pkl.outputFormat" {
                        self.stack.push(Value::Null);
                        return Ok(());
                    }
                    return Err(RuntimeError::new(
                        ErrorKind::Unsupported,
                        "Resource reader is unavailable in standard-library initialization.",
                    ));
                }
                let id = self.next_request;
                self.next_request = id
                    .checked_add(1)
                    .ok_or_else(|| invalid("Host request IDs exhausted"))?;
                let module_uri = self
                    .frames
                    .last()
                    .and_then(|frame| self.program.metadata.get(frame.block))
                    .and_then(|metadata| {
                        metadata
                            .unit
                            .and_then(|unit| self.program.units.get(unit.0 as usize))
                            // Explicit foreign source metadata can replace a
                            // primary compiled block's declaration provenance.
                            // Its old unit ID must not override that source.
                            .filter(|unit| match (&metadata.source, &unit.source) {
                                (None, _) => {
                                    unit.id == crate::modules::UnitId(0) && !unit.standard_library
                                }
                                (Some(source), Some(unit_source)) => {
                                    Arc::ptr_eq(source, unit_source)
                                }
                                _ => false,
                            })
                    })
                    .and_then(|unit| unit.identity.as_ref())
                    .map(|identity| identity.original_uri.clone())
                    .or_else(|| {
                        self.frames
                            .last()
                            .and_then(|frame| self.program.metadata.get(frame.block))
                            .and_then(|metadata| metadata.source.as_ref())
                            .or(self.context.primary_source.as_ref())
                            .map(|source| source.uri.clone())
                    });
                let request = ReadRequest {
                    id,
                    uri,
                    module_uri,
                    resolved_uri: None,
                    nullable,
                };
                self.begin_read_resolution(request)?;
            }
            Instruction::Return => {
                unreachable!("Return instruction uses its bounded dispatcher")
            }
        }
        Ok(())
    }

    // A returned Frame and type-check continuations are large cold locals.
    // Keep them off the stack of every non-Return VM instruction.
    #[inline(never)]
    fn finish_return(&mut self) -> Result<(), RuntimeError> {
        let base = self.frames.last().expect("active frame").base;
        if self.stack.len() != base + 1 {
            return Err(invalid("unbalanced return stack"));
        }
        let value = self.pop()?;
        if let Some(force) = &mut self.frames.last_mut().expect("active frame").force {
            // A successful cast exits its Java try block before cache publication.
            force.stolen_cast = None;
        }
        if let Some(force) = &mut self.frames.last_mut().expect("active frame").force
            && !force.skip_typecheck
            && !force.inherited_types.is_empty()
        {
            let receiver = force.receiver;
            let (ty, span, member, owner) = force.inherited_types.remove(0);
            let declaration = self
                .heap
                .get(owner)?
                .members
                .iter()
                .find(|m| m.name == force.name && !m.method)
                .ok_or_else(|| invalid("missing inherited declaration"))?
                .code;
            let ty = ty.substituted(&self.heap.get(owner)?.environment)?;
            let source = self
                .program
                .metadata
                .get(declaration)
                .and_then(|m| m.source.clone());
            let node = if typecheck::needs_type_identity(&ty) {
                let executable =
                    if let Some(parent) = self.heap.get(owner)?.environment.executable_owner {
                        self.heap
                            .executable_child(parent, ExecutionSite::Member(declaration))?
                    } else {
                        self.executable_root(declaration)
                    };
                // Synthetic stdlib members carry annotation metadata without
                // an authored CheckType instruction. Their declaration still
                // owns a distinct annotation site, shared with direct checks.
                Some(TypeNodeId(self.heap.executable_child(
                    executable,
                    ExecutionSite::PropertyType(declaration),
                )?))
            } else {
                None
            };
            let mut frames = vec![RuntimeFrame {
                anonymous: false,
                source,
                span,
                member,
            }];
            let frame = self.frames.last().expect("active frame");
            if let Some(metadata) = self.program.metadata.get(frame.block)
                && let Some(span) = metadata.value_span
            {
                frames.push(RuntimeFrame {
                    anonymous: false,
                    source: metadata.source.clone(),
                    span,
                    member: metadata.member.clone(),
                });
            }
            self.begin_type_node(ty, value, TypeDestination::Return, frames, node)?;
            let mut environment = self.heap.get(owner)?.environment.clone();
            environment.outer_value = environment.this_value.clone();
            environment.this_value = Some(Value::Object(receiver));
            environment.scopes.insert(0, Scope { receiver, owner });
            environment.custom_this = self
                .frames
                .last()
                .expect("type frame")
                .environment
                .custom_this;
            self.frames
                .last_mut()
                .expect("type frame")
                .type_run
                .as_mut()
                .expect("type run")
                .environment = environment;
            return Ok(());
        }
        if let Some(force) = &mut self.frames.last_mut().expect("active frame").force
            && !force.skip_typecheck
            && !force.collection_casts.is_empty()
        {
            let stolen = force.stolen_cache_value;
            let receiver = force.receiver;
            let probe = force.entry_probe.clone();
            let cast = force.collection_casts.remove(0);
            let frames = vec![RuntimeFrame {
                anonymous: false,
                source: cast.source,
                span: cast.span,
                member: cast.member,
            }];
            let target = cast
                .node
                .ok_or_else(|| invalid("missing wrapper cast target identity"))?;
            let frame = self.frames.last().expect("cast frame");
            let handler = if stolen {
                let parent = self
                    .heap
                    .get(receiver)?
                    .parent
                    .ok_or_else(|| invalid("stolen Mapping cast without parent"))?;
                let probe = probe.ok_or_else(|| invalid("stolen Mapping cast without probe"))?;
                mapping_ops::CastFailure::new(parent, probe, self.stack.len(), frame.pc, target)
            } else {
                let member = self.program.metadata.get(frame.block).and_then(|metadata| {
                    metadata
                        .value_span
                        .or(metadata.declaration)
                        .map(|span| RuntimeFrame {
                            anonymous: false,
                            source: metadata.source.clone(),
                            span,
                            member: metadata.member.clone(),
                        })
                });
                let constant = self
                    .program
                    .metadata
                    .get(frame.block)
                    .is_some_and(|metadata| metadata.constant_member);
                mapping_ops::CastFailure::known_member(
                    receiver,
                    self.stack.len(),
                    frame.pc,
                    target,
                    member,
                    constant,
                )
            };
            self.frames
                .last_mut()
                .expect("cast frame")
                .force
                .as_mut()
                .expect("cast force")
                .stolen_cast = Some(Box::new(handler));
            self.begin_type_node(cast.ty, value, TypeDestination::Return, frames, cast.node)?;
            let mut environment = cast.environment;
            environment.custom_this = self
                .frames
                .last()
                .expect("type frame")
                .environment
                .custom_this;
            self.frames
                .last_mut()
                .expect("type frame")
                .type_run
                .as_mut()
                .expect("type run")
                .environment = environment;
            return Ok(());
        }
        if let Some(force) = self.frames.last_mut().expect("active frame").force.as_mut()
            && !force.publishing_cache
            && let Some(key) = force.cache.clone()
        {
            // Publication can itself hash/compare keys and suspend. Keep ownership
            // of the pending slot until it succeeds, so failed publication retries.
            force.publishing_cache = true;
            let receiver = force.receiver;
            let probe = force.entry_probe.clone();
            self.complete_cache_probe(receiver, key, value.clone(), probe)?;
            if self.frames.len() > 1 {
                let caller = self.frames.len() - 2;
                self.frames[caller].callsite_hidden = true;
            }
            self.stack.push(value);
            self.frames.last_mut().expect("active frame").pc -= 1;
            return Ok(());
        }
        if let Some(force) = self.frames.last_mut().expect("active frame").force.as_mut()
            && force.publishing_cache
        {
            force.cache = None;
            force.publishing_cache = false;
        }
        if self.publish_const_receiver(&value)? {
            return Ok(());
        }
        let frame = self.frames.pop().expect("active frame");
        let return_to_type = frame.return_to_type;
        self.recycle_frame(frame);
        if return_to_type {
            self.frames
                .last_mut()
                .and_then(|frame| frame.type_run.as_mut())
                .ok_or_else(|| invalid("missing constraint continuation"))?
                .returned = Some(value);
            return Ok(());
        }
        if let Some(caller) = self.frames.last_mut() {
            caller.callsite_hidden = false;
        }
        self.stack.push(value);
        if self.frames.is_empty() {
            self.finish_top()?;
        }
        Ok(())
    }

    fn jump(&mut self, target: usize) -> Result<(), RuntimeError> {
        let frame = self
            .frames
            .last_mut()
            .ok_or_else(|| invalid("missing frame"))?;
        if target >= self.program.blocks[frame.block].len() {
            return Err(invalid("invalid jump target"));
        }
        frame.pc = target;
        Ok(())
    }
}

#[inline(always)]
fn fast_check_type_in_poll(
    ty: &TypeCheck,
    val: &Value,
    frame: &Frame,
    heap: &Heap,
    resolved_declarations: &BTreeMap<(usize, ObjectId), type_resolution::DeclarationValue>,
) -> bool {
    if typecheck::try_fast_leaf_type(ty, val) == Some(true) {
        return true;
    }
    match ty.resolved() {
        TypeCheck::MetaClass(name) => {
            if name == "Class" {
                matches!(val, Value::Class(_))
            } else if name == "TypeAlias" {
                matches!(val, Value::TypeAlias(_))
            } else {
                false
            }
        }
        TypeCheck::Class { id, .. } if *id != classes::TYPED => {
            if let Value::Object(val_id) = val
                && let Ok(obj) = heap.get(*val_id)
                && obj.foreign_map_entry.is_none()
                && obj.pair.is_none()
                && obj.reference.is_none()
                && let Some(cls) = obj.instance_class
            {
                heap.class_has_ancestor_id(cls, *id)
            } else {
                false
            }
        }
        TypeCheck::Function {
            parameters, result, ..
        } if matches!(result.resolved(), TypeCheck::Any | TypeCheck::Unknown)
            && parameters.as_ref().is_none_or(|ps| {
                ps.iter()
                    .all(|p| matches!(p.resolved(), TypeCheck::Any | TypeCheck::Unknown))
            }) =>
        {
            if let Value::Function(fid) = val
                && let Ok(obj) = heap.get(*fid)
                && obj.amendment.is_none()
                && obj.native_function.is_none()
                && let Some(f) = obj.function.as_ref()
            {
                parameters.as_ref().is_none_or(|ps| f.arity == ps.len())
            } else {
                false
            }
        }
        TypeCheck::Declared { recipe, arguments } if arguments.is_empty() => {
            let Some(scope) = frame.environment.scopes.last() else {
                return false;
            };
            if !heap.get(scope.owner).is_ok_and(|o| o.is_module) {
                return false;
            }
            match resolved_declarations.get(&(*recipe, scope.owner)) {
                Some(type_resolution::DeclarationValue::Value(Value::Class(target_cls))) => {
                    match val {
                        Value::Object(val_id) => {
                            if let Ok(obj) = heap.get(*val_id)
                                && obj.foreign_map_entry.is_none()
                                && obj.pair.is_none()
                                && obj.reference.is_none()
                                && let Some(mut cur_cls) = obj.instance_class
                            {
                                if cur_cls == *target_cls {
                                    return true;
                                }
                                for _ in 0..4 {
                                    let Some(sup) =
                                        heap.get_class(cur_cls).ok().and_then(|c| c.superclass)
                                    else {
                                        break;
                                    };
                                    if sup == *target_cls {
                                        return true;
                                    }
                                    cur_cls = sup;
                                }
                            }
                            false
                        }
                        Value::Class(_) => heap
                            .get_class(*target_cls)
                            .ok()
                            .and_then(|c| heap.get(c.prototype).ok())
                            .and_then(|p| p.builtin_type.as_ref())
                            .is_some_and(
                                |bt| matches!(bt.resolved(), TypeCheck::MetaClass(n) if n == "Class"),
                            ),
                        Value::TypeAlias(_) => heap
                            .get_class(*target_cls)
                            .ok()
                            .and_then(|c| heap.get(c.prototype).ok())
                            .and_then(|p| p.builtin_type.as_ref())
                            .is_some_and(
                                |bt| matches!(bt.resolved(), TypeCheck::MetaClass(n) if n == "TypeAlias"),
                            ),
                        _ => false,
                    }
                }
                Some(type_resolution::DeclarationValue::Builtin(bty)) => {
                    fast_check_type_in_poll(bty, val, frame, heap, resolved_declarations)
                }
                _ => false,
            }
        }
        _ => false,
    }
}

#[inline]
fn is_non_capturing_lambda(code: &[Instruction], arity: usize) -> bool {
    if code.is_empty() || code.len() > 32 {
        return false;
    }
    for ins in code {
        match ins {
            Instruction::Observe(_) | Instruction::ObserveNativeLiteral { .. } => {}
            Instruction::LoadLocal(d) if *d < arity => {}
            Instruction::BindLocal { depth: Some(d), ty }
                if *d < arity
                    && matches!(
                        ty.resolved(),
                        TypeCheck::Any
                            | TypeCheck::Unknown
                            | TypeCheck::Int
                            | TypeCheck::Float
                            | TypeCheck::Boolean
                            | TypeCheck::String
                            | TypeCheck::Number
                            | TypeCheck::NonNull
                    ) => {}
            Instruction::Constant(
                Value::Null | Value::Bool(_) | Value::Int(_) | Value::Float(_) | Value::String(_),
            )
            | Instruction::GetMember(_)
            | Instruction::Add
            | Instruction::AddIntConstant(_)
            | Instruction::Subtract
            | Instruction::Multiply
            | Instruction::Divide
            | Instruction::IntegerDivide
            | Instruction::Remainder
            | Instruction::Less
            | Instruction::LessEqual
            | Instruction::Greater
            | Instruction::GreaterEqual
            | Instruction::Equal
            | Instruction::NotEqual
            | Instruction::Negate
            | Instruction::Not
            | Instruction::Return => {}
            Instruction::CheckType(ty)
                if matches!(
                    ty.resolved(),
                    TypeCheck::Any
                        | TypeCheck::Unknown
                        | TypeCheck::Int
                        | TypeCheck::Float
                        | TypeCheck::Boolean
                        | TypeCheck::String
                        | TypeCheck::Number
                        | TypeCheck::NonNull
                ) => {}
            _ => return false,
        }
    }
    true
}

#[inline(always)]
fn fast_lookup_cached<'a>(
    obj: &'a Object,
    owner: Option<ObjectId>,
    name: &PklString,
    method: bool,
) -> Option<&'a Value> {
    if obj.cache_slots.len() <= 16 {
        for slot in &obj.cache_slots {
            if !slot.key.entry
                && slot.key.method == method
                && slot.key.owner == owner
                && slot.key.name == *name
            {
                return Some(&slot.value);
            }
        }
        return None;
    }
    let key = CacheKey {
        entry: false,
        owner,
        name: name.clone(),
        method,
    };
    match obj.cache.get(&key) {
        Some(LazyState::Completed(val)) => Some(val),
        _ => None,
    }
}

#[inline]
fn format_i64_buf(n: i64, buf: &mut [u8; 24]) -> &str {
    if n == 0 {
        return "0";
    }
    let mut pos = buf.len();
    let neg = n < 0;
    let mut mag: u64 = if neg {
        n.wrapping_neg() as u64
    } else {
        n as u64
    };
    while mag > 0 {
        pos -= 1;
        buf[pos] = b'0' + (mag % 10) as u8;
        mag /= 10;
    }
    if neg {
        pos -= 1;
        buf[pos] = b'-';
    }
    std::str::from_utf8(&buf[pos..]).expect("ASCII decimal integer")
}

#[inline]
fn format_i64_pkl_string(n: i64) -> PklString {
    let mut buf = [0u8; 24];
    PklString::from(format_i64_buf(n, &mut buf))
}

#[inline]
fn finish_interpolation_on_stack(
    stack: &mut identity::OperandStack,
    count: usize,
    scratch: &mut String,
) -> Result<(), RuntimeError> {
    if count == 0 {
        stack.push_raw(Value::string(""));
        return Ok(());
    }
    let start = stack
        .len()
        .checked_sub(count)
        .ok_or_else(|| invalid("interpolation stack underflow"))?;
    let all_utf8 = stack[start..].iter().all(|v| match v {
        Value::String(s) => s.as_str().is_some(),
        Value::Int(_) => true,
        _ => false,
    });
    if all_utf8 {
        scratch.clear();
        let mut num_buf = [0u8; 24];
        for v in &stack[start..] {
            match v {
                Value::String(s) => scratch.push_str(s.as_str().unwrap()),
                Value::Int(n) => scratch.push_str(format_i64_buf(n.get(), &mut num_buf)),
                _ => unreachable!(),
            }
        }
        let result = PklString::from(scratch.as_str());
        stack.truncate(start);
        stack.push_raw(Value::String(result));
        return Ok(());
    }
    let mut builder = crate::pkl_string::PklStringBuilder::default();
    for v in &stack[start..] {
        match v {
            Value::String(s) => builder.push_value(s),
            Value::Int(n) => builder.push_value(&format_i64_pkl_string(n.get())),
            _ => return Err(invalid("unchecked interpolation part")),
        }
    }
    stack.truncate(start);
    stack.push_raw(Value::String(builder.finish()));
    Ok(())
}

fn invalid_supertype(label: &PklString) -> RuntimeError {
    RuntimeError::new(
        ErrorKind::Type,
        PklString::from("`")
            .concat(label)
            .concat(&"` is not a valid supertype.".into()),
    )
}

fn invalid(message: &str) -> RuntimeError {
    RuntimeError::new(ErrorKind::InvalidBytecode, message)
}

fn binary(
    instruction: &Instruction,
    left: Value,
    right: Value,
    heap: &Heap,
    module: &PklString,
) -> Result<Value, RuntimeError> {
    if matches!(instruction, Instruction::Equal | Instruction::NotEqual) {
        let equal = match (&left, &right) {
            (Value::Int(a), Value::Float(b)) => a.get() as f64 == **b,
            (Value::Float(a), Value::Int(b)) => *a == b.get() as f64,
            (Value::Object(_), Value::Object(_)) => {
                return Err(RuntimeError::new(
                    ErrorKind::Unsupported,
                    "structural object equality is not implemented",
                ));
            }
            _ => left == right,
        };
        return Ok(Value::Bool(
            if matches!(instruction, Instruction::NotEqual) {
                !equal
            } else {
                equal
            },
        ));
    }
    if let Some(value) = duration::binary(instruction, &left, &right)? {
        return Ok(value);
    }
    if let Some(value) = data_size::binary(instruction, &left, &right)? {
        return Ok(value);
    }
    match (left, right) {
        (Value::Int(left), Value::Int(right)) => Ok(match instruction {
            Instruction::Add => Value::Int(
                crate::numeric::add(*left, *right)
                    .map_err(numeric_error)?
                    .into(),
            ),
            Instruction::Subtract => Value::Int(
                crate::numeric::subtract(*left, *right)
                    .map_err(numeric_error)?
                    .into(),
            ),
            Instruction::Multiply => Value::Int(
                crate::numeric::multiply(*left, *right)
                    .map_err(numeric_error)?
                    .into(),
            ),
            Instruction::Divide => Value::Float((left.get() as f64 / right.get() as f64).into()),
            Instruction::Less => Value::Bool(left < right),
            Instruction::LessEqual => Value::Bool(left <= right),
            Instruction::Greater => Value::Bool(left > right),
            Instruction::GreaterEqual => Value::Bool(left >= right),
            Instruction::IntegerDivide => Value::Int(
                crate::numeric::divide(*left, *right)
                    .map_err(numeric_error)?
                    .into(),
            ),
            Instruction::Remainder if right.get() == 0 => {
                return Err(RuntimeError::foreign(
                    crate::java_exception::provenance::ForeignOperation::IntegerRemainderByZero {
                        dividend: left.get(),
                    },
                    "Integer remainder by zero requires foreign-exception projection.",
                ));
            }
            Instruction::Remainder => Value::Int(
                crate::numeric::remainder(*left, *right)
                    .map_err(numeric_error)?
                    .into(),
            ),
            Instruction::Power if right >= 0 => {
                Value::Int(integer_power(*left, right.get() as u64)?.into())
            }
            Instruction::Power => {
                Value::Float(float_power(left.get() as f64, right.get() as f64).into())
            }
            _ => return Err(invalid("invalid binary instruction")),
        }),
        (Value::Bytes(left), Value::Bytes(right)) if matches!(instruction, Instruction::Add) => {
            Ok(Value::Bytes(left.concatenate(&right)))
        }
        (Value::String(left), Value::String(right)) if matches!(instruction, Instruction::Add) => {
            Ok(Value::string(left.concat(&right)))
        }
        (Value::String(left), Value::String(right)) => {
            let order = left.encode_utf16().cmp(right.encode_utf16());
            Ok(Value::Bool(match instruction {
                Instruction::Less => order.is_lt(),
                Instruction::LessEqual => order.is_le(),
                Instruction::Greater => order.is_gt(),
                Instruction::GreaterEqual => order.is_ge(),
                _ => {
                    return Err(binary_error(
                        operator_name(instruction),
                        &Value::String(left),
                        &Value::String(right),
                        heap,
                        module,
                    )?);
                }
            }))
        }
        (Value::Int(left), Value::Float(right)) => {
            float_binary(instruction, left.get() as f64, *right)
        }
        (Value::Float(left), Value::Int(right)) => {
            float_binary(instruction, *left, right.get() as f64)
        }
        (Value::Float(left), Value::Float(right)) => float_binary(instruction, *left, *right),
        (left, right) => Err(binary_error(
            operator_name(instruction),
            &left,
            &right,
            heap,
            module,
        )?),
    }
}

fn float_binary(instruction: &Instruction, left: f64, right: f64) -> Result<Value, RuntimeError> {
    Ok(match instruction {
        Instruction::Add => Value::Float((left + right).into()),
        Instruction::Subtract => Value::Float((left - right).into()),
        Instruction::Multiply => Value::Float((left * right).into()),
        Instruction::Divide => Value::Float((left / right).into()),
        Instruction::Less => Value::Bool(left < right),
        Instruction::LessEqual => Value::Bool(left <= right),
        Instruction::Greater => Value::Bool(left > right),
        Instruction::GreaterEqual => Value::Bool(left >= right),
        Instruction::Remainder => Value::Float((left % right).into()),
        Instruction::Power => Value::Float(float_power(left, right).into()),
        Instruction::IntegerDivide => {
            let result = (left / right).trunc();
            if !result.is_finite()
                || !(-9223372036854775808.0..9223372036854775808.0).contains(&result)
            {
                return Err(RuntimeError::new(
                    ErrorKind::Arithmetic,
                    if left.is_finite() {
                        format!(
                            "Cannot convert Float `{}` to Int because it is too large.",
                            crate::numeric::double_to_string(left)
                        )
                    } else {
                        format!(
                            "Cannot convert non-finite Float `{}` to Int.",
                            crate::numeric::double_to_string(left)
                        )
                    },
                ));
            }
            Value::Int((result as i64).into())
        }
        _ => return Err(invalid("invalid numeric instruction")),
    })
}

fn integer_power(mut base: i64, mut exponent: u64) -> Result<i64, RuntimeError> {
    let mut result = 1;
    while exponent != 0 {
        if exponent & 1 != 0 {
            result = crate::numeric::multiply(result, base).map_err(numeric_error)?;
        }
        exponent >>= 1;
        if exponent != 0 {
            base = crate::numeric::multiply(base, base).map_err(numeric_error)?;
        }
    }
    Ok(result)
}

fn numeric_error(error: crate::numeric::NumericError) -> RuntimeError {
    RuntimeError::new(
        ErrorKind::Arithmetic,
        match error {
            crate::numeric::NumericError::IntegerOverflow => "Integer overflow.",
            crate::numeric::NumericError::DivisionByZero => "Division by zero.",
        },
    )
}

/// Compile the bounded frontend. Static name decisions come exclusively from its resolver.
pub fn compile_module(
    resolved: &crate::resolver::ResolvedModule,
) -> Result<Arc<Program>, RuntimeError> {
    compile_module_with_options(
        resolved,
        CompileOptions {
            fuse_constant_add: true,
        },
    )
}

#[derive(Clone, Copy, Debug, Default)]
pub struct CompileOptions {
    pub fuse_constant_add: bool,
}

pub fn compile_module_with_options(
    resolved: &crate::resolver::ResolvedModule,
    options: CompileOptions,
) -> Result<Arc<Program>, RuntimeError> {
    compile_module_configured(resolved, options, false)
}

pub fn compile_module_with_power_assertions(
    resolved: &crate::resolver::ResolvedModule,
) -> Result<Arc<Program>, RuntimeError> {
    compile_module_configured(
        resolved,
        CompileOptions {
            fuse_constant_add: true,
        },
        true,
    )
}

fn compile_module_configured(
    resolved: &crate::resolver::ResolvedModule,
    options: CompileOptions,
    instrument: bool,
) -> Result<Arc<Program>, RuntimeError> {
    let mut program = Program::new(Vec::new());
    compile_unit_into(&mut program, resolved, options, instrument, None, None)?;
    program.blocks.freeze_prepared_prefix();
    program.metadata.freeze_prepared_prefix();
    program.objects.freeze_prepared_prefix();
    program.units.freeze_prepared_prefix();
    program.type_recipes.freeze_prepared_prefix();
    Ok(Arc::new(program))
}

fn compile_unit_into(
    program: &mut Program,
    resolved: &crate::resolver::ResolvedModule,
    options: CompileOptions,
    instrument: bool,
    source: Option<Arc<crate::value::RuntimeSource>>,
    identity: Option<crate::modules::ModuleIdentity>,
) -> Result<crate::modules::UnitId, RuntimeError> {
    compile_unit_into_with_syntax(
        program, resolved, None, None, options, instrument, source, identity, false,
    )
}

#[allow(clippy::too_many_arguments)]
fn compile_unit_into_with_syntax(
    program: &mut Program,
    resolved: &crate::resolver::ResolvedModule,
    cached_resolved: Option<Arc<crate::resolver::ResolvedModule>>,
    cached_syntax: Option<Arc<crate::syntax::Module>>,
    options: CompileOptions,
    instrument: bool,
    source: Option<Arc<crate::value::RuntimeSource>>,
    identity: Option<crate::modules::ModuleIdentity>,
    pure_standard_host: bool,
) -> Result<crate::modules::UnitId, RuntimeError> {
    // Fast single-pass path for pure standard Hosts when every import step in
    // `resolved.preparation` is a clean, non-glob, non-triple-dot, non-package
    // `file:///` or `pkl:` URI whose inferred clause name matches the parsed member name.
    if pure_standard_host
        && resolved.preparation_is_evaluation
        && resolved.preparation_failure.is_none()
        && !resolved.preparation.is_empty()
        && let Some(id) = identity.as_ref()
        && id
            .original_uri
            .as_str()
            .is_some_and(crate::host::resources::is_clean_file_uri)
    {
        let mut prepared: BTreeMap<Span, (PklString, PklString)> = BTreeMap::new();
        let mut all_clean = true;
        for step in &resolved.preparation {
            let crate::resolver::AstPreparation::Import {
                uri,
                span,
                clause,
                glob,
                ..
            } = step;
            if *glob || uri.starts_with("@") || uri.starts_with("...") {
                all_clean = false;
                break;
            }
            if !matches!(crate::host::resources::import_triple_path(uri), Ok(None)) {
                all_clean = false;
                break;
            }
            let Ok(resolved_uri) =
                crate::host::resources::resolve_module_key_uri(uri, &id.original_uri)
            else {
                all_clean = false;
                break;
            };
            let is_clean_target = resolved_uri
                .as_str()
                .is_some_and(crate::host::resources::is_clean_file_uri)
                || (resolved_uri.starts_with("pkl:")
                    && crate::host::resources::validate_module_key_uri(&resolved_uri).is_ok());
            if !is_clean_target {
                all_clean = false;
                break;
            }
            let Ok((abs, orig)) = crate::host::resources::prepare_module_uri(&resolved_uri, None)
            else {
                all_clean = false;
                break;
            };
            if *clause {
                // On `pure_standard_host`, `is_clean_file_uri` (`StandardFile`) and `pkl:`
                // (`StandardLibrary`) always have `hasFragmentPaths == false`.
                let Some(member) = resolved.module.members.iter().find(|m| {
                    matches!(
                        &m.expr.kind,
                        crate::syntax::ExprKind::Import {
                            uri_span,
                            clause_alias: Some(_),
                            ..
                        } if *uri_span == *span
                    )
                }) else {
                    all_clean = false;
                    break;
                };
                if let crate::syntax::ExprKind::Import {
                    clause_alias: Some(None),
                    ..
                } = &member.expr.kind
                {
                    let Ok(inferred) = crate::host::resources::infer_module_name(&orig, false)
                    else {
                        all_clean = false;
                        break;
                    };
                    if member.name != inferred {
                        all_clean = false;
                        break;
                    }
                }
            }
            prepared.insert(*span, (abs, orig));
        }
        if all_clean {
            let unit = compile_unit_body_at_with_syntax(
                None,
                program,
                resolved,
                cached_syntax,
                true,
                options,
                instrument,
                source,
                identity,
            )?;
            program.update_import_uris(unit, |span| prepared.get(&span));
            program.units[unit.0 as usize].import_order.clear();
            return Ok(unit);
        }
    }
    if resolved.preparation_is_evaluation
        && resolved.preparation.iter().any(|step| {
            matches!(
                step,
                crate::resolver::AstPreparation::Import { clause: true, .. }
            )
        })
    {
        let shell_decl = resolved.module.declaration.as_ref().map(|d| {
            let mut decl = d.clone();
            decl.annotations.clear();
            decl.parent = None;
            decl.modifiers.clear();
            decl
        });
        let shell = crate::resolver::ResolvedModule {
            module: crate::syntax::Module {
                source: resolved.module.source.clone(),
                declaration: shell_decl,
                members: Vec::new(),
                deferred_diagnostics: Vec::new(),
                folded_integer_spans: BTreeMap::new(),
                interpolation_spans: BTreeMap::new(),
            },
            standard_library: resolved.standard_library,
            preparation: resolved
                .preparation
                .iter()
                .filter(|step| {
                    matches!(
                        step,
                        crate::resolver::AstPreparation::Import { clause: true, .. }
                    )
                })
                .cloned()
                .collect(),
            preparation_is_evaluation: resolved.preparation_is_evaluation,
            function_owners: BTreeMap::new(),
            preparation_failure: None,
            names: BTreeMap::new(),
            const_names: BTreeSet::new(),
            custom_this: BTreeSet::new(),
            custom_functions: BTreeSet::new(),
        };
        let unit = compile_unit_body(program, &shell, options, instrument, source, identity)?;
        program.units[unit.0 as usize].build = Some(modules::ModuleBuild {
            module: cached_syntax.unwrap_or_else(|| Arc::new(resolved.module.clone())),
            resolved: cached_resolved,
            name_changed: false,
            options,
            instrument,
            prepared: BTreeMap::new(),
        });
        return Ok(unit);
    }
    if resolved.preparation_failure.is_none() {
        return compile_unit_body_at_with_syntax(
            None,
            program,
            resolved,
            cached_syntax,
            false,
            options,
            instrument,
            source,
            identity,
        );
    }
    // Rejected ASTs never publish or execute partially resolved guest members.
    // Their initializer only replays the observed construction prefix and fails.
    let mut rejected = resolved.clone();
    rejected.module.members.clear();
    rejected.module.deferred_diagnostics.clear();
    if let Some(declaration) = &mut rejected.module.declaration {
        declaration.annotations.clear();
        declaration.parent = None;
        declaration.modifiers.clear();
    }
    rejected.preparation.clear();
    rejected.preparation_failure = None;
    let unit = compile_unit_body(program, &rejected, options, instrument, source, identity)?;
    program.units[unit.0 as usize].import_order = uri_resolution::module_import_order(resolved);
    Ok(unit)
}

fn compile_unit_body(
    program: &mut Program,
    resolved: &crate::resolver::ResolvedModule,
    options: CompileOptions,
    instrument: bool,
    source: Option<Arc<crate::value::RuntimeSource>>,
    identity: Option<crate::modules::ModuleIdentity>,
) -> Result<crate::modules::UnitId, RuntimeError> {
    compile_unit_body_at(
        None, program, resolved, options, instrument, source, identity,
    )
}

fn compile_unit_body_at(
    replacement: Option<crate::modules::UnitId>,
    program: &mut Program,
    resolved: &crate::resolver::ResolvedModule,
    options: CompileOptions,
    instrument: bool,
    source: Option<Arc<crate::value::RuntimeSource>>,
    identity: Option<crate::modules::ModuleIdentity>,
) -> Result<crate::modules::UnitId, RuntimeError> {
    compile_unit_body_at_with_syntax(
        replacement,
        program,
        resolved,
        None,
        false,
        options,
        instrument,
        source,
        identity,
    )
}

#[allow(clippy::too_many_arguments)]
fn compile_unit_body_at_with_syntax(
    replacement: Option<crate::modules::UnitId>,
    program: &mut Program,
    resolved: &crate::resolver::ResolvedModule,
    cached_syntax: Option<Arc<crate::syntax::Module>>,
    skip_import_clauses: bool,
    options: CompileOptions,
    instrument: bool,
    source: Option<Arc<crate::value::RuntimeSource>>,
    identity: Option<crate::modules::ModuleIdentity>,
) -> Result<crate::modules::UnitId, RuntimeError> {
    // Raw development blocks may omit provenance, but their logical IDs
    // still share the same address space as compiler-created metadata.
    program.align_metadata()?;
    let blocks_start = program.blocks.next_id();
    let unit = replacement.unwrap_or(crate::modules::UnitId(
        program
            .units
            .next_id()
            .try_into()
            .map_err(|_| invalid("module unit limit"))?,
    ));
    let declaration_base = program.next_declaration;
    program.next_declaration = declaration_base
        .checked_add(resolved.module.source.len_bytes() + 2)
        .ok_or_else(|| invalid("declaration identity limit"))?;
    let mut compiler = Compiler {
        resolved,
        program,
        unit,
        declaration_base,
        source,
        identity,
        options,
        member: PklString::default(),
        super_method_allowed: false,
        module_annotation: false,
        let_members: Vec::new(),
        type_scopes: Vec::new(),
        modifiers: Vec::new(),
        constraint_module: false,
        runtime_type_owner: resolved
            .module
            .declaration
            .as_ref()
            .is_some_and(|declaration| declaration.parent.is_some()),
        runtime_type_fallback: None,
        alias_frames: Vec::new(),
        constraint_origins: BTreeMap::new(),
        active_constraints: BTreeMap::new(),
        shared_constraint_roots: BTreeMap::new(),
        active_type_parameters: BTreeMap::new(),
        constructor_parameters_bound: false,
        parameter_spans: BTreeMap::new(),
        parameter_label_spans: BTreeMap::new(),
        argument_spans: BTreeMap::new(),
        function_counts: BTreeMap::new(),
        entry_counts: BTreeMap::new(),
        function_names: BTreeMap::new(),
        instrument,
    };
    let module_name = resolved
        .module
        .declaration
        .as_ref()
        .and_then(|declaration| declaration.name.as_ref())
        .map(|parts| PklString::join(parts, "."))
        .unwrap_or_else(|| {
            compiler
                .source
                .as_ref()
                .map_or_else(PklString::default, |source| source.module_name.clone())
        });
    let is_base_module = compiler.identity.as_ref().is_some_and(|identity| {
        identity.original_uri == "pkl:base" || identity.resolved_uri == "pkl:base"
    });
    let is_primary_root = !is_base_module
        && (replacement == Some(crate::modules::UnitId(0))
            || (replacement.is_none() && compiler.program.module_template.is_none()));
    let template = compiler.object_template(&resolved.module.members, false, false)?;
    if is_primary_root {
        compiler.program.module_template = Some(template);
        compiler.program.module_name = module_name.clone();
    }
    let mut initial_module_members = Vec::new();
    // AstBuilder.doVisitModuleProperties installs declarations before ClassNode
    // executes module annotations, then publishes ordinary properties and methods.
    for kind in 0..3 {
        for (index, member) in resolved.module.members.iter().enumerate() {
            let declaration_kind = match member.expr.kind {
                crate::syntax::ExprKind::Import { .. } => Some(0),
                crate::syntax::ExprKind::Class { .. } => Some(1),
                crate::syntax::ExprKind::TypeAlias { .. } => Some(2),
                _ => None,
            };
            if declaration_kind == Some(kind) {
                initial_module_members.push(compiler.program.objects[template][index].clone());
            }
        }
    }
    let mode = match resolved
        .module
        .declaration
        .as_ref()
        .and_then(|declaration| declaration.parent.as_ref())
        .map(|parent| parent.kind)
    {
        Some(crate::syntax::ModuleParentKind::Extends) => modules::ModuleMode::Extends,
        Some(crate::syntax::ModuleParentKind::Amends) => modules::ModuleMode::Amends,
        None => modules::ModuleMode::Plain,
    };
    let modifiers = resolved
        .module
        .declaration
        .as_ref()
        .map_or(0, |declaration| {
            crate::syntax::modifier_flags(&declaration.modifiers)
        });
    let mut entry = Code::default();
    if is_primary_root {
        // Java initializes and eagerly forces the private base module before a
        // user module runs. Its output also initializes PcfRenderer's shared
        // method slots through an ordinary Module-valued renderDocument call.
        entry.push(Instruction::LoadBaseModule);
        entry.push(Instruction::Pop);
    }

    entry.push(Instruction::PrepareModuleImports(unit));
    entry.push(Instruction::MakeObject {
        template,
        has_parent: false,
    });
    if let Some(parent) = resolved
        .module
        .declaration
        .as_ref()
        .and_then(|declaration| declaration.parent.as_ref())
    {
        let spec = if resolved.preparation_is_evaluation {
            modules::ImportSpec::authored(parent.uri.clone(), parent.span, unit)
        } else {
            modules::ImportSpec::new(
                parent.uri.clone(),
                parent.span,
                unit,
                compiler.identity.as_ref(),
            )?
        };
        entry.push_at(Instruction::ImportModule(spec), parent.span);
        entry.push_at(Instruction::InitializeModuleParent, parent.span);
    }
    compiler
        .type_scopes
        .push(extract_type_scope(&resolved.module.members));
    if let Some(declaration) = &resolved.module.declaration {
        compiler.module_annotation = true;
        let annotations = compiler.annotations(&declaration.annotations, &mut entry);
        compiler.module_annotation = false;
        annotations?;
    }
    for method in [false, true] {
        if mode == modules::ModuleMode::Amends {
            break;
        }
        for (index, member) in resolved.module.members.iter().enumerate() {
            let compiled = compiler.program.objects[template][index].clone();
            if compiled.method != method {
                continue;
            }
            let annotated = mode != modules::ModuleMode::Amends
                && !member.annotations.is_empty()
                && !matches!(
                    member.expr.kind,
                    crate::syntax::ExprKind::Class { .. }
                        | crate::syntax::ExprKind::TypeAlias { .. }
                );
            if annotated {
                compiler.member_annotations(member, &mut entry, false)?;
            }
            compiler.initialize_member_types(std::slice::from_ref(member), true, &mut entry)?;
            if annotated {
                if method {
                    entry.push(Instruction::InitializeDeprecation);
                    entry.push(Instruction::StoreDeprecation);
                }
                entry.push(Instruction::PublishMemberAnnotations {
                    code: compiled.code,
                });
            }
            entry.push(Instruction::PublishModuleMember {
                code: compiled.code,
            });
        }
    }
    entry.push(Instruction::ReadModuleVersion);
    entry.push(Instruction::CheckModuleVersion);
    entry.push(Instruction::FinishModule);
    compiler.type_scopes.pop();
    entry.push(Instruction::Return);
    let entry = compiler.block(entry, None)?;
    if is_primary_root {
        compiler.program.entry = entry;
        compiler.program.initial_module_members = initial_module_members.clone();
    }
    compiler
        .program
        .constraint_origins
        .extend(compiler.constraint_origins);
    let built_unit = modules::ModuleUnit {
        id: unit,
        identity: compiler.identity,
        standard_library: false,
        singleton_cache_key: None,
        key: None,
        delegate_key: None,
        build: None,
        syntax: Some(cached_syntax.unwrap_or_else(|| Arc::new(resolved.module.clone()))),
        source: compiler.source,
        name: module_name,
        template,
        entry,
        blocks_start,
        initial_members: initial_module_members,
        import_order: uri_resolution::module_import_order_filtered(resolved, skip_import_clauses),
        mode,
        modifiers,
        class_id: if is_primary_root {
            reference::MODULE_ID
        } else {
            declaration_base + resolved.module.source.len_bytes() + 1
        },
    };
    if let Some(id) = replacement {
        compiler.program.units[id.0 as usize] = built_unit;
    } else {
        compiler.program.units.try_push(built_unit)?;
    }
    Ok(unit)
}

#[derive(Default)]
struct Code {
    instructions: Vec<Instruction>,
    spans: Vec<Option<Span>>,
    inferred_entry_parent_spans: Vec<Span>,
    let_scopes: Vec<LetScope>,
    type_casts: Vec<usize>,
    method_arities: BTreeMap<usize, usize>,
}
impl Code {
    /// Instrumentation observes a literal without changing its ConstantNode
    /// allocation. Box that declaration result once, not once per receiver.
    fn box_constant_result(&mut self) -> bool {
        let mut body = self
            .instructions
            .iter_mut()
            .filter(|instruction| !matches!(instruction, Instruction::Observe(_)));
        let (Some(Instruction::Constant(value)), None) = (body.next(), body.next()) else {
            return false;
        };
        *value = value.clone().boxed();
        true
    }
    fn push(&mut self, instruction: Instruction) {
        self.instructions.push(instruction);
        self.spans.push(None);
    }
    fn push_at(&mut self, instruction: Instruction, span: Span) {
        self.instructions.push(instruction);
        self.spans.push(Some(span));
    }
}
impl std::ops::Deref for Code {
    type Target = [Instruction];
    fn deref(&self) -> &Self::Target {
        &self.instructions
    }
}
impl std::ops::DerefMut for Code {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.instructions
    }
}

#[derive(Clone)]
struct TypeScopeClass {
    name: PklString,
    expr_span: Span,
    local: bool,
    abstract_: bool,
}

fn extract_type_scope(members: &[crate::syntax::Member]) -> Vec<TypeScopeClass> {
    let mut out = Vec::new();
    for m in members {
        if matches!(m.expr.kind, crate::syntax::ExprKind::Class { .. }) {
            let flags = crate::syntax::modifier_flags(&m.modifiers);
            out.push(TypeScopeClass {
                name: m.name.clone(),
                expr_span: m.expr.span,
                local: flags & crate::syntax::ModifierKind::Local.flag() != 0,
                abstract_: flags & crate::syntax::ModifierKind::Abstract.flag() != 0,
            });
        }
    }
    out
}

struct Compiler<'a> {
    resolved: &'a crate::resolver::ResolvedModule,
    program: &'a mut Program,
    unit: crate::modules::UnitId,
    declaration_base: usize,
    source: Option<Arc<crate::value::RuntimeSource>>,
    identity: Option<crate::modules::ModuleIdentity>,
    options: CompileOptions,
    member: PklString,
    /// Whether the current member has a class or non-amend module scope.
    super_method_allowed: bool,
    module_annotation: bool,
    let_members: Vec<PklString>,
    type_scopes: Vec<Vec<TypeScopeClass>>,
    modifiers: Vec<crate::syntax::Modifier>,
    constraint_module: bool,
    runtime_type_owner: bool,
    runtime_type_fallback: Option<Span>,
    /// Alias use sites retained separately from executable constraint blocks.
    alias_frames: Vec<RuntimeFrame>,
    constraint_origins: BTreeMap<usize, (Vec<RuntimeFrame>, Vec<RuntimeFrame>)>,
    /// Constraint bytecode is a graph: a recursive type use can point back to
    /// a reserved block without recursively compiling that expression again.
    active_constraints: BTreeMap<Span, usize>,
    /// Local-writing constraints retain their declaration call target across substitutions.
    shared_constraint_roots: BTreeMap<Span, usize>,
    active_type_parameters: BTreeMap<PklString, TypeCheck>,
    /// Alias cloning visits inline/lambda nodes, but not lazy member call targets.
    constructor_parameters_bound: bool,
    parameter_spans: BTreeMap<PklString, Span>,
    /// Union labels retain the actual type node rather than its failing child.
    parameter_label_spans: BTreeMap<PklString, Span>,
    /// Constructor parent nodes retain the authored actual, including alias syntax.
    argument_spans: BTreeMap<PklString, Span>,
    function_counts: BTreeMap<PklString, usize>,
    entry_counts: BTreeMap<PklString, usize>,
    function_names: BTreeMap<Span, PklString>,
    instrument: bool,
}

fn authored_type_constraints(ty: &crate::syntax::Type) -> bool {
    use crate::syntax::TypeKind;
    match &ty.kind {
        TypeKind::Constrained { .. } => true,
        TypeKind::Named { arguments, .. } => arguments.iter().any(authored_type_constraints),
        TypeKind::Nullable(inner) => authored_type_constraints(inner),
        TypeKind::Union { types, .. } => types.iter().any(authored_type_constraints),
        TypeKind::Function {
            parameters,
            return_type,
        } => {
            parameters.iter().any(authored_type_constraints)
                || authored_type_constraints(return_type)
        }
        _ => false,
    }
}

// Only an authored `as`/`is` expression supplies the current member as the
// trailing Reference-use frame. Declaration annotations keep their own owner.
#[derive(Clone, Copy)]
enum ReferenceUseSite {
    Declaration,
    ExpressionCastOrTest,
}

fn reference_constrained_type(check: &TypeCheck) -> Option<&TypeCheck> {
    let mut pending = vec![check];
    let mut visited = BTreeSet::new();
    while let Some(check) = pending.pop() {
        let check = check.resolved();
        if !visited.insert(check as *const TypeCheck) {
            continue;
        }
        match check {
            TypeCheck::Constrained { .. } => return Some(check),
            TypeCheck::Nullable(inner)
            | TypeCheck::Alias(inner)
            | TypeCheck::GenericAlias(inner)
            | TypeCheck::NamedAlias {
                expanded: inner, ..
            } => pending.push(inner),
            TypeCheck::ParameterizedCollection { arguments, .. }
            | TypeCheck::Pair { arguments }
            | TypeCheck::CollectionBase { arguments } => {
                pending.extend(arguments.iter().rev().map(|(ty, _)| ty))
            }
            TypeCheck::Union { types, .. } => pending.extend(types.iter().rev()),
            TypeCheck::Reference { domain, referent } => {
                pending.push(referent);
                pending.push(domain);
            }
            _ => {}
        }
    }
    None
}

impl Compiler<'_> {
    /// Source owned by the executable value after Java's grouping removal and
    /// direct numeric negation folding. Authored spans remain the identity for
    /// resolution and assertion observations; runtime value locations use this.
    fn executable_value_span(&self, expression: &crate::syntax::Expr) -> Span {
        use crate::syntax::{ExprKind, UnaryOp};

        let expression = expression.unparenthesized();
        if let Some(span) = self
            .resolved
            .module
            .folded_integer_spans
            .get(&expression.span)
        {
            return *span;
        }
        if let ExprKind::Unary {
            op: UnaryOp::Negate,
            operand,
        } = &expression.kind
            && matches!(operand.kind, ExprKind::Float(_))
        {
            return operand.span;
        }
        expression.span
    }

    /// Constant members have no Java MemberNode. Their inserted value frame
    /// comes from ObjectMember.getBodySection's authored header suffix instead.
    fn constant_member_body(&self, expression: &crate::syntax::Expr) -> bool {
        use crate::resolver::Binding;
        use crate::syntax::{ExprKind, UnaryOp};

        let mut pending = vec![expression];
        while let Some(expression) = pending.pop() {
            match &expression.unparenthesized().kind {
                ExprKind::Null
                | ExprKind::Bool(_)
                | ExprKind::Int(_)
                | ExprKind::Float(_)
                | ExprKind::String(_) => {}
                ExprKind::Unary {
                    op: UnaryOp::Negate,
                    operand,
                } if matches!(operand.kind, ExprKind::Float(_)) => {}
                ExprKind::Call { callee, arguments }
                    if matches!(
                        &callee.kind,
                        ExprKind::Name(name) if matches!(name.as_str(), Some("List" | "Set" | "Map"))
                    ) && self.resolved.names.get(&callee.span)
                        == Some(&Binding::ImplicitMethod) =>
                {
                    pending.extend(arguments);
                }
                _ => return false,
            }
        }
        true
    }

    fn constant_member_value_span(
        &self,
        expression: &crate::syntax::Expr,
        header_end: usize,
        entry: bool,
    ) -> Span {
        let suffix = self
            .resolved
            .module
            .source
            .entire()
            .range(header_end..expression.span.end);
        if suffix.is_empty() {
            return expression.span;
        }
        let skip = suffix
            .characters()
            .take_while(|(_, ch)| {
                ch.scalar().is_some_and(|ch| {
                    (entry && ch == ']') || ch == '=' || crate::diagnostic::java_whitespace(ch)
                })
            })
            .map(|(_, ch)| ch.len_bytes())
            .sum::<usize>();
        Span::new(header_end + skip, expression.span.end)
    }

    fn property_value_span(&self, member: &crate::syntax::Member) -> Span {
        if matches!(member.expr.kind, crate::syntax::ExprKind::Uninitialized) {
            return member.name_span;
        }
        // VmUtils.createObjectProperty retains a member node for local typed
        // properties even when the body is constant, so their lowered body owns
        // the frame. This distinction applies to module, class and object scopes.
        if !(member.local && member.ty.is_some()) && self.constant_member_body(&member.expr) {
            let header_end = member
                .ty
                .as_ref()
                .map_or(member.name_span.end, |ty| ty.span.end);
            self.constant_member_value_span(&member.expr, header_end, false)
        } else {
            self.executable_value_span(&member.expr)
        }
    }

    fn object(&mut self, members: &[crate::syntax::Member]) -> Result<usize, RuntimeError> {
        self.object_template(members, false, true)
    }

    fn object_template(
        &mut self,
        members: &[crate::syntax::Member],
        class: bool,
        object_body: bool,
    ) -> Result<usize, RuntimeError> {
        self.type_scopes.push(extract_type_scope(members));
        let mut result = Vec::new();
        for member in members {
            let scope_count = self.type_scopes.len();
            let previous_member = self.member.clone();
            let previous_modifiers = self.modifiers.clone();
            let object_method = object_body
                || (!class
                    && self
                        .resolved
                        .module
                        .declaration
                        .as_ref()
                        .and_then(|declaration| declaration.parent.as_ref())
                        .is_some_and(|parent| {
                            parent.kind == crate::syntax::ModuleParentKind::Amends
                        }));
            match self.object_member(member, object_method) {
                Ok(compiled) => {
                    self.program.metadata[compiled.code].inline_property_typecheck =
                        object_body && member.local;
                    result.push(compiled);
                }
                Err(error) if class => {
                    // A later declaration's unresolved type must not outrun an earlier
                    // declaration's override error. Replay failures during class
                    // initialization, without running any property initializer.
                    self.type_scopes.truncate(scope_count);
                    self.member = previous_member.concat(
                        &format!(
                            ".{}",
                            crate::diagnostic_names::quote_identifier(&member.name)
                        )
                        .into(),
                    );
                    let mut code = Code::default();
                    code.push_at(
                        Instruction::Fail(error.clone()),
                        error.span.unwrap_or(member.span),
                    );
                    let block = self.block(code, Some(member.name_span))?;
                    self.program.metadata[block].initialization_error = Some(error);
                    self.program
                        .declaration_sources
                        .insert(block, member.declaration_source());
                    result.push(Member {
                        flags: Member::declaration_flags(member),
                        entry: false,
                        name: member.name.clone(),
                        code: block,
                        local: member.local,
                        method: matches!(
                            member.expr.kind,
                            crate::syntax::ExprKind::Function { method: true, .. }
                        ),
                        ty: None,
                        type_span: None,
                        constant: false,
                        fixed: false,
                    });
                    self.member = previous_member;
                    self.modifiers = previous_modifiers;
                }
                Err(error) => return Err(error),
            }
        }
        let index = self.program.objects.try_push(result)?;
        self.type_scopes.pop();
        Ok(index)
    }

    fn object_member(
        &mut self,
        member: &crate::syntax::Member,
        object_method: bool,
    ) -> Result<Member, RuntimeError> {
        let previous = std::mem::replace(&mut self.constructor_parameters_bound, false);
        let previous_super_method_allowed =
            std::mem::replace(&mut self.super_method_allowed, !object_method);
        let result = self.object_member_body(member, object_method);
        if let Ok(compiled) = &result {
            self.program
                .declaration_sources
                .insert(compiled.code, member.declaration_source());
        }
        self.super_method_allowed = previous_super_method_allowed;
        self.constructor_parameters_bound = previous;
        result
    }

    fn object_member_body(
        &mut self,
        member: &crate::syntax::Member,
        object_method: bool,
    ) -> Result<Member, RuntimeError> {
        let previous_modifiers = std::mem::replace(&mut self.modifiers, member.modifiers.clone());
        self.validate_modifiers(member)?;
        let previous_member = self.member.clone();
        let label = crate::diagnostic_names::quote_identifier(&member.name);
        self.member = if previous_member.is_empty() {
            format!(
                "{}{}",
                if self.module_annotation { "." } else { "#" },
                label
            )
            .into()
        } else {
            previous_member.concat(&format!(".{label}").into())
        };
        let mut initialization_error = None;
        let compiled_type = member.ty.as_ref().map(|ty| {
            self.compile_type(ty).unwrap_or_else(|error| {
                initialization_error = Some(error);
                TypeCheck::Unknown
            })
        });
        if initialization_error.is_none()
            && let crate::syntax::ExprKind::Function {
                parameters,
                return_type,
                method: true,
                ..
            } = &member.expr.kind
        {
            for ty in parameters
                .iter()
                .filter_map(|p| p.ty.as_ref())
                .chain(return_type.iter())
            {
                if let Err(error) = self.compile_type(ty) {
                    initialization_error = Some(error);
                    break;
                }
            }
        }
        let mut instructions = Code::default();
        if object_method
            && matches!(
                member.expr.kind,
                crate::syntax::ExprKind::Function { method: true, .. }
            )
        {
            // ObjectMethodNode resolves its signature when the lazy member's
            // call target is requested, not when its enclosing object is built.
            instructions.push(Instruction::BeginObjectMethodSignature);
            self.initialize_member_types(std::slice::from_ref(member), true, &mut instructions)?;
            instructions.push(Instruction::EndObjectMethodSignature);
        }
        if let Some(error) = &initialization_error {
            // Annotation resolution runs when the owner is initialized. Keep a
            // stub body so compilation cannot bypass earlier type dependencies.
            instructions.push_at(
                Instruction::Fail(error.clone()),
                error.span.unwrap_or(member.span),
            );
        } else if matches!(member.expr.kind, crate::syntax::ExprKind::Uninitialized) {
            if self
                .source
                .as_ref()
                .is_some_and(|source| source.uri == "pkl:reflect")
                && let Some(operation) = reflection::Operation::binding(&self.member)
            {
                instructions.push_at(Instruction::ReflectCall(operation), member.name_span);
            } else if self.resolved.standard_library
                && self.source.as_ref().is_some_and(|source| {
                    matches!(source.uri.as_str(), Some("pkl:platform" | "pkl:release"))
                })
                && self.member == "#current"
                && self
                    .modifiers
                    .iter()
                    .any(|modifier| modifier.kind == crate::syntax::ModifierKind::External)
            {
                if self.source.as_ref().unwrap().uri == "pkl:platform" {
                    instructions.push_at(Instruction::PlatformCurrent, member.name_span);
                } else {
                    instructions.push_at(Instruction::ReleaseCurrent, member.name_span);
                }
            } else {
                self.default_property(member, &mut instructions)?;
            }
        } else if matches!(member.expr.kind, crate::syntax::ExprKind::TypeAlias { .. }) {
            self.type_alias_expression(&member.expr, &mut instructions)?;
        } else if matches!(member.expr.kind, crate::syntax::ExprKind::Class { .. }) {
            self.class_declaration_expression(&member.expr, &mut instructions)?;
        } else {
            self.expression_with_property_parent(&member.expr, &mut instructions, Some(member))?;
        }
        let constant_member = !(member.local && member.ty.is_some())
            && self.constant_member_body(&member.expr)
            && instructions.box_constant_result();
        let property_typecheck = member.ty.as_ref().map(|_| instructions.len());
        if let Some(ty) = &member.ty {
            instructions.push_at(
                Instruction::CheckType(compiled_type.clone().expect("member annotation")),
                self.type_source_span(ty),
            );
        }
        instructions.push(Instruction::Return);
        let code = self.program.blocks.next_id();
        let header_span = Span::new(
            member.span.start,
            member
                .ty
                .as_ref()
                .map_or(member.name_span.end, |ty| ty.span.end),
        );
        self.block(instructions, Some(header_span))?;
        self.program.metadata[code].hidden = member
            .modifiers
            .iter()
            .any(|m| m.kind == crate::syntax::ModifierKind::Hidden);
        self.program.metadata[code].constant_member = constant_member;
        self.program.metadata[code].value_span = Some(self.property_value_span(member));
        self.program.metadata[code].member_span = Some(member.span);
        self.program.metadata[code].initialization_error = initialization_error;
        self.program.metadata[code].property_typecheck = property_typecheck;
        self.program.metadata[code].amendment = member.is_amendment();
        self.program.metadata[code].amendment_parent_span =
            member.is_amendment().then_some(member.name_span);
        self.program.metadata[code].fixed_span = member.override_fixed_span;
        self.program.metadata[code].const_span = member.override_const_span;
        if let crate::syntax::ExprKind::Function {
            parameters,
            method: true,
            ..
        } = &member.expr.kind
        {
            self.program.metadata[code].method_signature = Some((
                member
                    .name
                    .concat(&"(".into())
                    .concat(&PklString::join(
                        &parameters
                            .iter()
                            .map(|p| p.name.clone())
                            .collect::<Vec<_>>(),
                        ", ",
                    ))
                    .concat(&")".into()),
                parameters.len(),
            ));
        }
        self.member = previous_member;
        self.modifiers = previous_modifiers;
        Ok(Member {
            flags: Member::declaration_flags(member),
            entry: false,
            name: member.name.clone(),
            code,
            local: crate::syntax::modifier_flags(&member.modifiers)
                & crate::syntax::ModifierKind::Local.flag()
                != 0,
            ty: compiled_type,
            type_span: member.ty.as_ref().map(|ty| self.type_source_span(ty)),
            fixed: crate::syntax::modifier_flags(&member.modifiers)
                & crate::syntax::ModifierKind::Fixed.flag()
                != 0,
            constant: crate::syntax::modifier_flags(&member.modifiers)
                & crate::syntax::ModifierKind::Const.flag()
                != 0
                || matches!(
                    member.expr.kind,
                    crate::syntax::ExprKind::Class { .. }
                        | crate::syntax::ExprKind::TypeAlias { .. }
                ),
            method: matches!(
                member.expr.kind,
                crate::syntax::ExprKind::Function { method: true, .. }
            ),
        })
    }

    fn block(&mut self, code: Code, declaration: Option<Span>) -> Result<usize, RuntimeError> {
        self.program.alloc_block(
            code.instructions,
            BlockMetadata {
                unit: Some(self.unit),
                module_receiver_input: None,
                source: self.source.clone(),
                hidden: false,
                anonymous_root: false,
                stdlib_bootstrap: false,
                constant_member: false,
                spans: code.spans,
                member: self.member.clone(),
                declaration,
                member_span: None,
                value_span: None,
                property_typecheck: None,
                inline_property_typecheck: false,
                amendment: false,
                amendment_parent_span: None,
                inferred_entry_parent_spans: code.inferred_entry_parent_spans,
                fixed_span: None,
                const_span: None,
                initialization_error: None,
                method_signature: None,
                method_arities: code.method_arities,
                let_scopes: code.let_scopes,
                constraint_root: None,
                inline_constraint: false,
                type_casts: code.type_casts,
            },
        )
    }

    // Alias annotations compile nested object bodies. Keep this path out of
    // inner_expression's large general-dispatch frame during stdlib bootstrap.
    fn type_alias_expression(
        &mut self,
        expression: &crate::syntax::Expr,
        output: &mut Code,
    ) -> Result<(), RuntimeError> {
        let crate::syntax::ExprKind::TypeAlias {
            type_parameters,
            ty,
            ..
        } = &expression.kind
        else {
            return Err(invalid("type alias expression"));
        };
        let begin = output.len();
        let declaration_name = self
            .resolved
            .module
            .members
            .iter()
            .find(|member| member.expr.span == expression.span)
            .map(|member| {
                PklString::from("#").concat(
                    &self
                        .resolved
                        .module
                        .source
                        .slice(member.name_span)
                        .expect("declaration source span")
                        .to_pkl_string(),
                )
            })
            .unwrap_or_else(|| self.member.clone());
        let canonical_base_mixin = self
            .source
            .as_ref()
            .is_some_and(|source| source.uri == "pkl:base")
            && declaration_name == "#Mixin";
        output.push(Instruction::BeginTypeAlias {
            declaration: self.declaration_base + expression.span.start,
            name: declaration_name,
            end: 0,
        });
        self.declaration_annotations(expression, output)?;
        let declaration = self.declaration_base + expression.span.start;
        let formals = type_parameters
            .iter()
            .map(|parameter| {
                (
                    parameter.name.clone(),
                    TypeCheck::Parameter {
                        block: declaration,
                        name: parameter.name.clone(),
                    },
                )
            })
            .collect();
        let previous_module = std::mem::replace(&mut self.constraint_module, true);
        let alias_body =
            self.compile_type_at(ty, self.type_scopes.len(), &mut BTreeSet::new(), &formals);
        self.constraint_module = previous_module;
        if let Ok(body) = alias_body {
            if canonical_base_mixin {
                self.program.base_mixin_declaration = Some(declaration);
            }
            self.program.alias_definitions.insert(
                declaration,
                type_resolution::AliasDefinition {
                    parameters: type_parameters
                        .iter()
                        .map(|parameter| parameter.name.clone())
                        .collect(),
                    body,
                    origin: RuntimeFrame {
                        anonymous: false,
                        source: self.source.clone(),
                        span: ty.span,
                        member: self.member.clone(),
                    },
                },
            );
        }
        let parameters = type_parameters
            .iter()
            .map(|parameter| (parameter.name.clone(), TypeCheck::Unknown))
            .collect();
        match self.compile_type_at(
            ty,
            self.type_scopes.len(),
            &mut BTreeSet::new(),
            &parameters,
        ) {
            Ok(_) => self.initialize_type_resolution_with_parameters(
                ty,
                &parameters,
                output,
                ReferenceUseSite::Declaration,
            ),
            Err(error) => {
                self.initialize_type_resolution_with_parameters(
                    ty,
                    &parameters,
                    output,
                    ReferenceUseSite::Declaration,
                );
                let span = error.span.unwrap_or(ty.span);
                output.push_at(Instruction::Fail(error), span);
            }
        }
        output.push(Instruction::FinishTypeAlias);
        let end = output.len();
        if let Instruction::BeginTypeAlias { end: target, .. } = &mut output[begin] {
            *target = end;
        }
        Ok(())
    }

    // Class member compilation can reenter the expression compiler.
    // Keep this path out of inner_expression's large general-dispatch frame.
    fn class_declaration_expression(
        &mut self,
        expression: &crate::syntax::Expr,
        output: &mut Code,
    ) -> Result<(), RuntimeError> {
        let crate::syntax::ExprKind::Class {
            type_parameters,
            superclass,
            members,
            ..
        } = &expression.kind
        else {
            return Err(invalid("class expression"));
        };
        if !type_parameters.is_empty() {
            return Err(unsupported_type("generic classes"));
        }
        let class = Class {
            module: self
                .source
                .as_ref()
                .map(|source| source.module_name.clone()),
            origin_uri: self
                .identity
                .as_ref()
                .map(|identity| identity.original_uri.clone())
                .or_else(|| self.source.as_ref().map(|source| source.uri.clone())),
            id: self.declaration_base + expression.span.start,
            // PClassInfo receives the raw declaration name; quoted
            // SymbolTable scope labels belong only to diagnostics.
            name: self
                .type_scopes
                .iter()
                .rev()
                .flatten()
                .find(|member| member.expr_span == expression.span)
                .map(|member| member.name.clone())
                .ok_or_else(|| invalid("missing class declaration name"))?,
            open: crate::syntax::modifier_flags(&self.modifiers)
                & crate::syntax::ModifierKind::Open.flag()
                != 0,
            abstract_: crate::syntax::modifier_flags(&self.modifiers)
                & crate::syntax::ModifierKind::Abstract.flag()
                != 0,
        };
        // Class definitions are forced lazily, including their member type resolution.
        let arena = modules::ArenaCheckpoint::new(self.program);
        let checkpoint = (
            self.type_scopes.len(),
            self.member.clone(),
            self.modifiers.clone(),
        );
        let template = match self.object_template(members, true, false) {
            Ok(template) => template,
            Err(error) => {
                arena.rollback(self.program);
                self.type_scopes.truncate(checkpoint.0);
                self.member = checkpoint.1;
                self.modifiers = checkpoint.2;
                let span = error.span.unwrap_or(expression.span);
                output.push_at(Instruction::Fail(error.at(span)), span);
                return Ok(());
            }
        };
        let begin = output.len();
        output.push_at(
            Instruction::BeginClass {
                template,
                class,
                end: 0,
            },
            expression.span,
        );
        if let Some(parent) = superclass {
            if let Err(error) =
                self.class_expression(parent, output, self.type_scopes.len(), &mut BTreeSet::new())
            {
                output.push_at(Instruction::Fail(error.at(parent.span)), parent.span);
            } else {
                output.push_at(Instruction::SetClassParent, parent.span);
            }
        } else {
            output.push(Instruction::SetDefaultClassParent);
        }
        self.declaration_annotations(expression, output)?;
        self.type_scopes.push(extract_type_scope(members));
        // ClassNode initializes properties in declaration order, then methods.
        let compiled = self.program.objects[template].clone();
        for method in [false, true] {
            for (index, (member, compiled)) in members.iter().zip(&compiled).enumerate() {
                if compiled.method != method {
                    continue;
                }
                let annotated = !member.annotations.is_empty();
                if annotated {
                    self.member_annotations(member, output, true)?;
                }
                if let Some(error) = self.program.metadata[compiled.code]
                    .initialization_error
                    .clone()
                {
                    // Expansion failures still resolve declaration identities at
                    // runtime, including a previously published partial alias.
                    if let Some(ty) = &member.ty {
                        self.initialize_type_resolution(ty, output);
                    }
                    if let crate::syntax::ExprKind::Function {
                        parameters,
                        return_type,
                        ..
                    } = &member.expr.kind
                    {
                        for ty in parameters
                            .iter()
                            .filter_map(|p| p.ty.as_ref())
                            .chain(return_type.iter())
                        {
                            self.initialize_type_resolution(ty, output);
                        }
                    }
                    output.push_at(
                        Instruction::Fail(error.clone()),
                        error.span.unwrap_or(member.span),
                    );
                    continue;
                }
                self.initialize_member_types(std::slice::from_ref(member), true, output)?;
                if annotated && method {
                    output.push(Instruction::InitializeDeprecation);
                    output.push(Instruction::StoreDeprecation);
                }
                output.push_at(
                    if annotated {
                        Instruction::PublishAnnotatedClassMember { template, index }
                    } else {
                        Instruction::PublishClassMember { template, index }
                    },
                    member.name_span,
                );
            }
        }
        self.type_scopes.pop();
        output.push(Instruction::FinishClass);
        let end = output.len();
        if let Instruction::BeginClass { end: target, .. } = &mut output[begin] {
            *target = end;
        }
        Ok(())
    }

    fn inner_expression(
        &mut self,
        expression: &crate::syntax::Expr,
        output: &mut Code,
    ) -> Result<(), RuntimeError> {
        use crate::resolver::Binding;
        use crate::syntax::ExprKind;
        match &expression.kind {
            ExprKind::Import {
                uri,
                uri_span,
                glob,
                ..
            } => {
                if *glob && uri.starts_with("...") && !self.resolved.preparation_is_evaluation {
                    return Err(RuntimeError::new(
                        ErrorKind::Type,
                        "Cannot combine glob imports with triple-dot module URIs.",
                    )
                    .at(expression.span));
                }
                let spec = if self.resolved.preparation_is_evaluation {
                    modules::ImportSpec::authored(uri.clone(), *uri_span, self.unit)
                } else {
                    modules::ImportSpec::new(
                        uri.clone(),
                        *uri_span,
                        self.unit,
                        self.identity.as_ref(),
                    )?
                };
                if *glob {
                    let spec = self.glob_spec(expression, Some(spec))?;
                    output.push_at(Instruction::ImportGlob(spec), expression.span);
                } else {
                    output.push_at(Instruction::ImportModule(spec), expression.span);
                }
            }
            ExprKind::Class { .. } => self.class_declaration_expression(expression, output)?,
            ExprKind::TypeAlias { .. } => self.type_alias_expression(expression, output)?,
            ExprKind::EntryParent => {
                output.push(Instruction::EntryParent {
                    end: output.len() + 3,
                });
                output.push(Instruction::EntryKey);
                output.push(Instruction::ApplyEntryDefault);
            }
            ExprKind::Uninitialized => {
                if self.resolved.standard_library
                    && self
                        .source
                        .as_ref()
                        .is_some_and(|source| source.uri == "pkl:analyze")
                    && self.member == "#importGraph"
                    && self
                        .modifiers
                        .iter()
                        .any(|modifier| modifier.kind == crate::syntax::ModifierKind::External)
                {
                    output.push(Instruction::AnalyzeImportGraph);
                } else if self.resolved.standard_library
                    && self
                        .source
                        .as_ref()
                        .is_some_and(|source| source.uri == "pkl:EvaluatorSettings")
                    && self.member == "#resolvePath"
                    && self
                        .modifiers
                        .iter()
                        .any(|modifier| modifier.kind == crate::syntax::ModifierKind::External)
                {
                    output.push(Instruction::ResolveEvaluatorSettingsPath);
                } else if self
                    .source
                    .as_ref()
                    .is_some_and(|source| source.uri == "pkl:base")
                    && self.member.as_str() == Some("#Undefined")
                {
                    // BaseNodes.Undefined uses undefinedValue(), without the
                    // property receiver carried by undefinedPropertyValue().
                    let mut error = RuntimeError::new(ErrorKind::Type, "Undefined value.")
                        .undefined_without_receiver();
                    // ExternalMemberNode's body has an unavailable source
                    // section. Its declaration remains available to reflection,
                    // but StackTraceGenerator omits this throwing body frame.
                    error.unavailable_origin = true;
                    output.push(Instruction::Fail(error));
                } else if self
                    .source
                    .as_ref()
                    .is_some_and(|source| source.uri == "pkl:reflect")
                    && let Some(operation) = reflection::Operation::binding(&self.member)
                {
                    output.push(Instruction::ReflectCall(operation));
                } else if self
                    .source
                    .as_ref()
                    .is_some_and(|source| source.uri == "pkl:json")
                    && self.member == "#Parser.parse"
                {
                    output.push(Instruction::JsonParse);
                } else if self.resolved.standard_library
                    && self
                        .source
                        .as_ref()
                        .is_some_and(|source| source.uri == "pkl:yaml")
                    && matches!(
                        self.member.as_str(),
                        Some("#Parser.parse" | "#Parser.parseAll")
                    )
                {
                    output.push(Instruction::YamlParse {
                        all: self.member == "#Parser.parseAll",
                    });
                } else if self.resolved.standard_library
                    && self
                        .source
                        .as_ref()
                        .is_some_and(|source| source.uri == "pkl:Benchmark")
                    && self.member.ends_with(".run")
                {
                    output.push(Instruction::BenchmarkRun);
                } else if self
                    .source
                    .as_ref()
                    .is_some_and(|source| source.uri == "pkl:jsonnet")
                    && self.resolved.standard_library
                    && matches!(
                        self.member.as_str(),
                        Some("#Renderer.renderDocument" | "#Renderer.renderValue")
                    )
                {
                    output.push(Instruction::RenderJsonnet {
                        document: self.member == "#Renderer.renderDocument",
                    });
                } else if self.resolved.standard_library
                    && self
                        .source
                        .as_ref()
                        .is_some_and(|source| source.uri == "pkl:protobuf")
                    && matches!(
                        self.member.as_str(),
                        Some(
                            "#Renderer.renderValue"
                                | "#Renderer.renderDocument"
                                | "#Renderer.renderType"
                        )
                    )
                {
                    output.push(if self.member == "#Renderer.renderType" {
                        Instruction::ProtobufRenderType
                    } else {
                        Instruction::RenderProtobuf {
                            document: self.member == "#Renderer.renderDocument",
                        }
                    });
                } else if self
                    .source
                    .as_ref()
                    .is_some_and(|source| source.uri == "pkl:pklbinary")
                    && self.resolved.standard_library
                    && matches!(
                        self.member.as_str(),
                        Some("#Renderer.renderDocument" | "#Renderer.renderValue")
                    )
                {
                    output.push(Instruction::RenderPklBinary {
                        document: self.member == "#Renderer.renderDocument",
                    });
                } else if self
                    .source
                    .as_ref()
                    .is_some_and(|source| source.uri == "pkl:xml")
                    && self.resolved.standard_library
                    && matches!(
                        self.member.as_str(),
                        Some("#Renderer.renderDocument" | "#Renderer.renderValue")
                    )
                {
                    output.push(Instruction::RenderXml {
                        document: self.member == "#Renderer.renderDocument",
                    });
                } else {
                    output.push(Instruction::AbstractMember {
                        name: self.member.clone(),
                        method: true,
                    });
                }
            }
            ExprKind::Int(value) => output.push(Instruction::Constant(Value::Int((*value).into()))),
            ExprKind::Float(value) => {
                output.push(Instruction::Constant(Value::Float((*value).into())))
            }
            ExprKind::Null => output.push(Instruction::Constant(Value::Null)),
            ExprKind::String(value) => {
                output.push(Instruction::Constant(Value::string(value.source_literal())))
            }
            ExprKind::Bool(value) => output.push(Instruction::Constant(Value::Bool(*value))),
            ExprKind::This => {
                output.push(if self.resolved.custom_this.contains(&expression.span) {
                    Instruction::LoadCustomThis
                } else {
                    Instruction::LoadThis
                })
            }
            ExprKind::Outer => output.push(Instruction::LoadOuter),
            ExprKind::Module => output.push(Instruction::LoadModule),
            ExprKind::Super => {
                return Err(RuntimeError::new(
                    ErrorKind::Unsupported,
                    "bare super is unsupported",
                ));
            }
            ExprKind::Name(name) => {
                if self.resolved.const_names.contains(&expression.span)
                    && !matches!(
                        self.resolved.names.get(&expression.span),
                        Some(Binding::AmbiguousProperty { .. })
                    )
                {
                    let (owner_depth, local, method) =
                        match self.resolved.names.get(&expression.span) {
                            Some(Binding::Property { owner_depth, local }) => {
                                (*owner_depth, *local, false)
                            }
                            Some(Binding::Method { owner_depth, local }) => {
                                (*owner_depth, *local, true)
                            }
                            Some(Binding::ImplicitThis) => (0, false, false),
                            Some(Binding::ImplicitMethod) => (0, false, true),
                            _ => return Err(invalid("invalid const member binding")),
                        };
                    output.push(Instruction::RequireConstProperty {
                        name: name.clone(),
                        owner_depth,
                        local,
                        method,
                    });
                }
                let name: PklString = name.clone();
                match self
                    .resolved
                    .names
                    .get(&expression.span)
                    .ok_or_else(|| invalid("missing static binding"))?
                {
                    Binding::BaseTypeAlias
                        if matches!(
                            name.as_str(),
                            Some(
                                "DataSizeUnit"
                                    | "DurationUnit"
                                    | "Charset"
                                    | "Comparable"
                                    | "Char"
                                    | "Uri"
                                    | "Mixin"
                            )
                        ) =>
                    {
                        output.push(Instruction::LoadBaseModule);
                        output.push(Instruction::GetMember(name));
                    }
                    Binding::BaseTypeAlias => output.push(Instruction::BuiltinTypeAlias(name)),
                    Binding::BaseClass => {
                        let ty = name
                            .as_str()
                            .and_then(reference::base_type)
                            .or_else(|| {
                                name.as_str()
                                    .and_then(builtins::id)
                                    .map(|id| TypeCheck::Class {
                                        module: None,
                                        id,
                                        name: name.clone(),
                                    })
                            })
                            .ok_or_else(|| unsupported_type("builtin class value"))?;
                        output.push(Instruction::BuiltinClass(ty));
                    }
                    Binding::BaseConstant => {
                        output.push(Instruction::Constant(Value::Float(if name == "NaN" {
                            f64::NAN.into()
                        } else {
                            f64::INFINITY.into()
                        })))
                    }
                    Binding::Lexical { depth, slot: 0 } => {
                        output.push(Instruction::LoadLocal(*depth))
                    }
                    Binding::Lexical { .. } => return Err(invalid("unsupported lexical slot")),
                    Binding::AmbiguousProperty { owner_depth } => {
                        output.push(Instruction::GetAmbiguousProperty {
                            name,
                            owner_depth: *owner_depth,
                            needs_const: self.resolved.const_names.contains(&expression.span),
                        })
                    }
                    Binding::Property { owner_depth, local } => {
                        output.push(Instruction::GetProperty {
                            name,
                            owner_depth: *owner_depth,
                            local: *local,
                        })
                    }
                    Binding::Method { owner_depth, local } => {
                        output.push(Instruction::GetMethodProperty {
                            name,
                            owner_depth: *owner_depth,
                            local: *local,
                        })
                    }
                    Binding::ImplicitMethod
                        if matches!(name.as_str(), Some("TODO" | "Undefined")) =>
                    {
                        output.push(Instruction::LoadBaseModule);
                        output.push(Instruction::GetMethod(name));
                    }
                    Binding::ImplicitMethod if name == "Regex" => {
                        output.push(Instruction::RegexConstructor)
                    }
                    Binding::ImplicitMethod if name == "IntSeq" => {
                        output.push(Instruction::IntSeqConstructor)
                    }
                    Binding::ImplicitMethod if name == "Pair" => {
                        output.push(Instruction::PairConstructor)
                    }
                    Binding::ImplicitMethod if name == "Null" => {
                        output.push(Instruction::NullConstructor)
                    }
                    Binding::ImplicitMethod => output.push(Instruction::GetImplicitMethod(name)),
                    Binding::ImplicitThis => output.push(Instruction::GetImplicitProperty(name)),
                    Binding::CustomThis => {
                        output.push(Instruction::LoadCustomThis);
                        output.push(Instruction::GetImplicitCustomProperty(name));
                    }
                    Binding::CustomMethod => {
                        output.push(Instruction::LoadCustomThis);
                        output.push(Instruction::GetImplicitCustomMethod(name));
                    }
                }
            }
            ExprKind::Object {
                parent,
                members,
                parameters,
            } => {
                if let Some(parent) = parent {
                    self.expression(parent, output)?;
                }
                let begin = output.len();
                self.collection_body(
                    members,
                    parameters,
                    crate::heap::CollectionKind::Dynamic,
                    parent.is_some(),
                    output,
                )?;
                if let Some(parent) = parent {
                    for index in begin..output.len() {
                        if matches!(
                            output[index],
                            Instruction::PrepareFunctionAmendment { .. }
                                | Instruction::BeginFunctionAmendment { .. }
                                | Instruction::BeginCollection { .. }
                                | Instruction::AmendEmpty
                        ) {
                            output.spans[index] = Some(parent.unparenthesized().span);
                        }
                        if matches!(
                            output[index],
                            Instruction::BeginCollection { .. } | Instruction::AmendEmpty
                        ) {
                            break;
                        }
                    }
                }
            }
            _ => unreachable!("expression continuation handles this syntax"),
        }
        Ok(())
    }
}

pub(crate) fn regex_string(value: &crate::pkl_regex::PklRegex) -> PklString {
    regex::string(value)
}

fn scalar_string(value: &Value) -> Result<PklString, RuntimeError> {
    Ok(match value {
        Value::Null | Value::NullWithDefault(_) => "null".into(),
        Value::Bool(value) => value.to_string().into(),
        Value::Int(value) => value.to_string().into(),
        Value::Float(value) => crate::numeric::double_to_string(**value).into(),
        Value::Duration(value) => value.to_string().into(),
        Value::DataSize(value) => value.to_string().into(),
        Value::Bytes(value) => value.to_string().into(),
        Value::IntSeq(value) => value.to_string().into(),
        Value::Regex(value) => regex::string(value),
        Value::String(value) => value.clone(),
        _ => {
            return Err(RuntimeError::new(
                ErrorKind::Unsupported,
                "object and function string conversion is not implemented",
            ));
        }
    })
}

fn type_error(
    expected: &TypeCheck,
    actual: &Value,
    module: &PklString,
    heap: &Heap,
) -> Result<RuntimeError, RuntimeError> {
    type_error_at_width(expected, actual, module, heap, 80)
}
fn type_error_at_width(
    expected: &TypeCheck,
    actual: &Value,
    module: &PklString,
    heap: &Heap,
    limit: usize,
) -> Result<RuntimeError, RuntimeError> {
    // Java's Simple mismatch compares the complete qualified-name string, not
    // separate module/name fields or class IDs. The failed nominal check has
    // already established that this value does not have the expected type.
    let version_conflict = (|| {
        let (TypeCheck::Class { id, .. }, Value::Object(actual_id)) =
            (expected.unaliased(), actual)
        else {
            return None;
        };
        if !classes::typed_instance(heap, *actual_id).ok()? {
            return None;
        }
        let expected_id = heap.find_class(*id)?;
        // Use the value's actual class handle, as VmTyped.getVmClass() does.
        let actual_class_id = heap.object_class(*actual_id).ok()??;
        let actual_class = heap.class_definition(actual_class_id).ok()?;
        if heap.class_qualified_name(expected_id).ok()?
            != heap.class_qualified_name(actual_class_id).ok()?
        {
            return None;
        }
        let expected_uri = heap
            .class_definition(expected_id)
            .ok()?
            .origin_uri
            .clone()?;
        let actual_uri = actual_class.origin_uri.clone()?;
        // PClassInfo.isModuleClass tests the simple class name, including its
        // unusual behavior for a user class literally named ModuleClass.
        let module_class = heap.class_simple_name(actual_class_id).ok()? == "ModuleClass";
        Some((expected_uri, actual_uri, module_class))
    })();
    if let Some((expected_uri, actual_uri, module_class)) = version_conflict {
        let rendered = type_name_value_with_heap(expected, module, Some(heap));
        let qualifier = if module_class {
            "` defined by module `"
        } else {
            "` defined in module `"
        };
        return Ok(RuntimeError::new(
            ErrorKind::Type,
            PklString::join(
                &[
                    "Module version conflict: Expected value of type `".into(),
                    rendered.clone(),
                    qualifier.into(),
                    expected_uri.clone(),
                    "`, but got type `".into(),
                    rendered,
                    qualifier.into(),
                    actual_uri.clone(),
                    "`.\n".into(),
                ],
                "",
            ),
        ));
    }
    let actual_reference = if let Value::Object(id) = actual {
        heap.get(*id)?.reference.as_ref()
    } else {
        None
    };
    let actual_type = if matches!(expected.unaliased(), TypeCheck::Reference { .. })
        && let Some(reference) = actual_reference
    {
        reference::export_type_name_value(reference, heap, module)?
    } else {
        crate::diagnostic_values::type_name(actual, heap, module)?
    };
    let value = crate::diagnostic_values::render_string(actual, heap, module, limit)?;
    if matches!(expected.unaliased(), TypeCheck::Nothing) {
        return Ok(RuntimeError::new(ErrorKind::Type, PklString::join(&[
            "Expected `nothing`, but got type `".into(), actual_type,
            "`.\n\n`nothing` is typically used as the return type of functions that throw errors.\nValue: ".into(), value
        ], "")));
    }
    let inline_value = matches!(
        expected.resolved(),
        TypeCheck::Union {
            string_literals: true,
            ..
        }
    ) && matches!(actual, Value::String(_));
    let expected = if matches!(expected.unaliased(), TypeCheck::Reference { .. })
        && actual_reference.is_some()
    {
        reference::type_name_value(expected, module)
    } else {
        type_name_value_with_heap(expected, module, Some(heap))
    };
    Ok(RuntimeError::new(
        ErrorKind::Type,
        if actual.is_null() {
            PklString::join(
                &[
                    "Expected value of type `".into(),
                    expected,
                    "`, but got `null`.".into(),
                ],
                "",
            )
        } else if inline_value {
            PklString::join(
                &[
                    "Expected value of type `".into(),
                    expected,
                    "`, but got `".into(),
                    value,
                    "`.".into(),
                ],
                "",
            )
        } else {
            PklString::join(
                &[
                    "Expected value of type `".into(),
                    expected,
                    "`, but got type `".into(),
                    actual_type,
                    "`.\nValue: ".into(),
                    value,
                ],
                "",
            )
        },
    ))
}

fn type_name_value(ty: &TypeCheck, module: &PklString) -> PklString {
    type_name_value_with_heap(ty, module, None)
}

fn type_name_value_with_heap(ty: &TypeCheck, module: &PklString, heap: Option<&Heap>) -> PklString {
    enum Part<'a> {
        Type(&'a TypeCheck),
        Text(PklString),
    }
    let mut pending = vec![Part::Type(ty)];
    let mut result = crate::pkl_string::PklStringBuilder::default();
    while let Some(part) = pending.pop() {
        let ty = match part {
            Part::Text(text) => {
                result.push_value(&text);
                continue;
            }
            Part::Type(ty) => ty.resolved(),
        };
        let text: PklString = match ty {
            TypeCheck::Unknown => "unknown".into(),
            TypeCheck::VarArgs { .. } => "VarArgs".into(),
            TypeCheck::MetaClass(name) => name.clone(),
            TypeCheck::Function { parameters, .. } => parameters.as_ref().map_or_else(
                || "Function".into(),
                |p| format!("Function{}", p.len()).into(),
            ),
            TypeCheck::NamedAlias { expanded, .. }
            | TypeCheck::Alias(expanded)
            | TypeCheck::GenericAlias(expanded)
            | TypeCheck::Constrained { base: expanded, .. } => {
                pending.push(Part::Type(expanded));
                continue;
            }
            TypeCheck::IntegerAlias { name, .. } => name.clone(),
            TypeCheck::RefDomain => "pkl.ref#Domain".into(),
            TypeCheck::Reference { .. } => "pkl.ref#Reference".into(),
            TypeCheck::Class {
                id,
                name,
                module: declaring_module,
            } => {
                if builtins::name(*id).is_some() || classes::base_class_name(*id).is_some() {
                    name.clone()
                } else if heap
                    .and_then(|heap| {
                        heap.find_class(*id).and_then(|class| {
                            heap.get_class(class)
                                .ok()
                                .and_then(|class| heap.get(class.prototype).ok())
                        })
                    })
                    .is_some_and(|prototype| prototype.is_module)
                {
                    crate::heap::display_class_name(name, "ModuleClass")
                } else {
                    crate::heap::display_class_name(
                        declaring_module.as_ref().unwrap_or(module),
                        name,
                    )
                }
            }
            TypeCheck::Collection(kind) | TypeCheck::ParameterizedCollection { kind, .. } => {
                format!("{kind:?}").into()
            }
            TypeCheck::Pair { .. } => "Pair".into(),
            TypeCheck::CollectionBase { .. } => "Collection".into(),
            TypeCheck::StringLiteral(value) => {
                result.push_value(&value.quoted());
                continue;
            }
            TypeCheck::Nullable(inner) => {
                pending.push(Part::Text("?".into()));
                pending.push(Part::Type(inner));
                continue;
            }
            TypeCheck::Union {
                types,
                string_literals: true,
                ..
            } => {
                let mut seen = std::collections::BTreeSet::new();
                for ty in types {
                    let TypeCheck::StringLiteral(value) = ty.resolved() else {
                        unreachable!("literal union metadata")
                    };
                    if seen.insert(value.clone()) {
                        if seen.len() > 1 {
                            result.push('|');
                        }
                        result.push_value(&value.quoted());
                    }
                }
                continue;
            }
            TypeCheck::Union { types, .. } => {
                for (index, ty) in types.iter().enumerate().rev() {
                    pending.push(Part::Type(ty));
                    if index > 0 {
                        pending.push(Part::Text("|".into()));
                    }
                }
                continue;
            }
            _ => format!("{ty:?}").into(),
        };
        result.push_value(&text);
    }
    result.finish()
}

fn value_type_name(value: &Value) -> &'static str {
    match value {
        Value::Null | Value::NullWithDefault(_) => "Null",
        Value::Bool(_) => "Boolean",
        Value::Int(_) => "Int",
        Value::Float(_) => "Float",
        Value::Duration(_) => "Duration",
        Value::DataSize(_) => "DataSize",
        Value::String(_) => "String",
        Value::Bytes(_) => "Bytes",
        Value::IntSeq(_) => "IntSeq",
        Value::Regex(_) => "Regex",
        Value::Object(_) => "Dynamic",
        Value::Function(_) => "Function",
        Value::Class(_) => "Class",
        Value::TypeAlias(_) => "TypeAlias",
    }
}

fn display_value(value: &Value) -> PklString {
    match value {
        Value::String(value) => value.quoted(),
        _ => scalar_string(value).unwrap_or_else(|_| value_type_name(value).into()),
    }
}

fn unary_error(operator: &str, operand: &Value) -> RuntimeError {
    RuntimeError::new(
        ErrorKind::Type,
        PklString::from(format!(
            "Operator `{operator}` is not defined for operand type `{}`.\nOperand: ",
            value_type_name(operand)
        ))
        .concat(&display_value(operand)),
    )
}

fn binary_error(
    operator: &str,
    left: &Value,
    right: &Value,
    heap: &Heap,
    module: &PklString,
) -> Result<RuntimeError, RuntimeError> {
    Ok(RuntimeError::new(
        ErrorKind::Type,
        PklString::join(
            &[
                PklString::from(format!(
                    "Operator `{operator}` is not defined for operand types `"
                ))
                .concat(&crate::diagnostic_values::type_name(left, heap, module)?)
                .concat(&"` and `".into())
                .concat(&crate::diagnostic_values::type_name(right, heap, module)?)
                .concat(&"`.\nLeft operand : ".into()),
                crate::diagnostic_values::render_string(left, heap, module, 80)?,
                "\nRight operand: ".into(),
                crate::diagnostic_values::render_string(right, heap, module, 80)?,
            ],
            "",
        ),
    ))
}

fn operator_name(instruction: &Instruction) -> &'static str {
    match instruction {
        Instruction::Add | Instruction::AddIntConstant(_) => "+",
        Instruction::Subtract => "-",
        Instruction::Multiply => "*",
        Instruction::Divide => "/",
        Instruction::IntegerDivide => "~/",
        Instruction::Remainder => "%",
        Instruction::Power => "**",
        Instruction::Less => "<",
        Instruction::LessEqual => "<=",
        Instruction::Greater => ">",
        Instruction::GreaterEqual => ">=",
        _ => "unknown",
    }
}

fn internal_varargs_error() -> RuntimeError {
    RuntimeError::new(
        ErrorKind::Type,
        "Class `VarArgs` can only be used by the standard library.",
    )
}

fn unsupported_type(feature: &str) -> RuntimeError {
    RuntimeError::new(
        ErrorKind::Unsupported,
        format!("{feature} are not implemented"),
    )
}

impl Compiler<'_> {
    fn compile_reference_type(
        &self,
        ty: &crate::syntax::Type,
        arguments: &[TypeCheck],
    ) -> Result<TypeCheck, RuntimeError> {
        let crate::syntax::TypeKind::Named {
            path, name_spans, ..
        } = &ty.kind
        else {
            unreachable!()
        };
        Ok(match path[1].as_str() {
            Some("Domain") if arguments.is_empty() => TypeCheck::RefDomain,
            Some("Access") if arguments.is_empty() => TypeCheck::Class {
                module: None,
                id: reference::ACCESS_ID,
                name: "Access".into(),
            },
            Some("Reference") if arguments.is_empty() => TypeCheck::Reference {
                domain: Box::new(TypeCheck::Unknown),
                referent: Box::new(TypeCheck::Unknown),
            },
            Some("Reference") if arguments.len() == 2 => {
                let domain = arguments[0].clone();
                let referent = arguments[1].clone();
                if let Some(TypeCheck::Constrained { constraints, .. }) =
                    reference_constrained_type(&referent)
                {
                    let mut error = RuntimeError::new(
                        ErrorKind::Type,
                        "`Reference` referent type argument may not include type constraints.",
                    )
                    .at(ty.span);
                    if let Some((mut frames, origin_aliases)) = constraints
                        .first()
                        .and_then(|constraint| self.constraint_origins.get(&constraint.block))
                        .cloned()
                    {
                        // Aliases first expanded inside this Reference have
                        // not yet adopted their outermost node when initial
                        // validation runs. Already substituted type arguments
                        // retain the alias tree from their use site instead.
                        if origin_aliases.starts_with(&self.alias_frames) {
                            frames.truncate(
                                origin_aliases
                                    .len()
                                    .saturating_sub(self.alias_frames.len() + 1),
                            );
                        }
                        frames.retain(|frame| {
                            std::iter::once(ty.span)
                                .chain(self.alias_frames.iter().map(|alias| alias.span))
                                .all(|usage| {
                                    frame.span.start < usage.start || frame.span.end > usage.end
                                })
                        });
                        error.frames = frames;
                    }
                    return Err(error);
                }
                TypeCheck::Reference {
                    domain: Box::new(domain),
                    referent: Box::new(referent),
                }
            }
            Some("Domain" | "Reference" | "Access") => {
                return Err(RuntimeError::new(
                    ErrorKind::Type,
                    format!(
                        "Expected {} type argument(s) but got {}.",
                        if path[1] == "Reference" { 2 } else { 0 },
                        arguments.len()
                    ),
                ));
            }
            _ => {
                return Err(RuntimeError::new(
                    ErrorKind::Type,
                    format!("Cannot find type `{}` in module `pkl.ref`.", path[1]),
                )
                .at(name_spans[1]));
            }
        })
    }

    fn find_type(&self, name: &PklString, _limit: usize) -> Option<(crate::syntax::Member, usize)> {
        // ResolveSimpleDeclaredTypeNode starts at the enclosing module. Ordinary
        // module properties can shadow types, but locals/parameters/class members cannot.
        self.resolved
            .module
            .members
            .iter()
            .find(|member| {
                &member.name == name
                    && !matches!(
                        member.expr.kind,
                        crate::syntax::ExprKind::Function { method: true, .. }
                    )
            })
            .cloned()
            .map(|member| (member, 0))
    }

    fn initialize_member_types(
        &mut self,
        members: &[crate::syntax::Member],
        methods: bool,
        output: &mut Code,
    ) -> Result<(), RuntimeError> {
        for member in members {
            if let Some(ty) = &member.ty {
                self.initialize_type_resolution(ty, output);
            }
            if methods
                && let crate::syntax::ExprKind::Function {
                    parameters,
                    return_type,
                    method: true,
                    ..
                } = &member.expr.kind
            {
                for ty in parameters
                    .iter()
                    .filter_map(|parameter| parameter.ty.as_ref())
                    .chain(return_type.iter())
                {
                    self.initialize_type_resolution(ty, output);
                }
            }
        }
        Ok(())
    }

    /// Compile the authored resolution order separately from the expanded check.
    /// A declaration executes at its position in this plan, before later names
    /// are resolved. Failures remain VM instructions, so published declarations
    /// survive and ordinary lazy members can retry through the same scheduler.
    fn initialize_type_resolution(&mut self, ty: &crate::syntax::Type, output: &mut Code) {
        self.initialize_type_resolution_with_parameters(
            ty,
            &self.active_type_parameters.clone(),
            output,
            ReferenceUseSite::Declaration,
        );
    }

    fn initialize_type_resolution_with_parameters(
        &mut self,
        ty: &crate::syntax::Type,
        parameters: &BTreeMap<PklString, TypeCheck>,
        output: &mut Code,
        use_site: ReferenceUseSite,
    ) {
        if let Err(failure) = self.type_resolution_plan(ty, parameters, output, use_site) {
            let (error, span) = *failure;
            output.push_at(Instruction::Fail(error.at(span)), span);
        }
    }

    fn type_resolution_plan(
        &mut self,
        ty: &crate::syntax::Type,
        parameters: &BTreeMap<PklString, TypeCheck>,
        output: &mut Code,
        use_site: ReferenceUseSite,
    ) -> Result<(), Box<(RuntimeError, Span)>> {
        use crate::syntax::{ExprKind, TypeKind};
        let reference_start = self.program.type_recipes.next_id();
        let check =
            self.compile_type_at(ty, self.type_scopes.len(), &mut BTreeSet::new(), parameters);
        self.reference_usage_context(reference_start, ty, use_site);
        if let Ok(check) = check
            && check.has_declared()
        {
            // An expanded alias can contain a runtime-qualified declaration.
            // Initialize its authored declaration before materializing that
            // expansion, just as in the ordinary named-alias path below. The
            // alias call target owns resolution errors and partial publication.
            if let TypeKind::Named {
                path,
                arguments,
                name_spans,
            } = &ty.kind
                && path.len() == 1
                && !(arguments.is_empty() && parameters.contains_key(&path[0]))
                && let Some((member, _)) = self.find_type(&path[0], self.type_scopes.len())
                && matches!(member.expr.kind, ExprKind::TypeAlias { .. })
            {
                output.push_at(
                    Instruction::ResolveClassType {
                        name: member.name.clone(),
                        local: crate::syntax::modifier_flags(&member.modifiers)
                            & crate::syntax::ModifierKind::Local.flag()
                            != 0,
                    },
                    name_spans[0],
                );
                for argument in arguments {
                    self.type_resolution_plan(argument, parameters, output, use_site)?;
                }
                output.push_at(Instruction::CheckTypeAliasInitialized, ty.span);
            }
            // Class resolution and arity precede argument resolution. Unlike
            // aliases, canonical Class<T> erases the argument entirely.
            if let TypeKind::Named {
                path, arguments, ..
            } = &ty.kind
                && path.len() > 1
                && !arguments.is_empty()
            {
                let TypeCheck::Declared { recipe, .. } = check.resolved() else {
                    unreachable!("qualified type is a declaration recipe")
                };
                let recipe = *recipe;
                let gate = output.len();
                output.push_at(
                    Instruction::PrepareTypeArguments { recipe, end: 0 },
                    ty.span,
                );
                for argument in arguments {
                    if let Err(failure) =
                        self.type_resolution_plan(argument, parameters, output, use_site)
                    {
                        let (error, span) = *failure;
                        output.push_at(Instruction::Fail(error.at(span)), span);
                        break;
                    }
                }
                output[gate] = Instruction::PrepareTypeArguments {
                    recipe,
                    end: output.len(),
                };
            }
            output.push_at(Instruction::ResolveType(check.clone()), ty.span);
            // The actual receiver determines whether this is Reference. Its
            // substituted arguments must be validated at the authored type
            // use, before evaluating the value being tested or cast.
            output.push_at(
                Instruction::ValidateTypeArguments {
                    ty: check,
                    aliases: self.alias_frames.clone(),
                },
                ty.span,
            );
            return Ok(());
        }
        match &ty.kind {
            TypeKind::Named {
                path,
                arguments,
                name_spans,
            } => {
                let parameter =
                    path.len() == 1 && arguments.is_empty() && parameters.contains_key(&path[0]);
                let declaration = (!parameter && path.len() == 1)
                    .then(|| self.find_type(&path[0], self.type_scopes.len()))
                    .flatten()
                    .map(|(member, _)| member);
                let alias = declaration
                    .as_ref()
                    .is_some_and(|member| matches!(member.expr.kind, ExprKind::TypeAlias { .. }));
                if let Some(member) = &declaration
                    && matches!(
                        member.expr.kind,
                        ExprKind::Class { .. } | ExprKind::TypeAlias { .. }
                    )
                {
                    output.push_at(
                        Instruction::ResolveClassType {
                            name: member.name.clone(),
                            local: crate::syntax::modifier_flags(&member.modifiers)
                                & crate::syntax::ModifierKind::Local.flag()
                                != 0,
                        },
                        name_spans[0],
                    );
                    // Parameterized aliases resolve actual arguments before testing
                    // initialization or arity. Keep the alias value on the VM stack
                    // across those child plans; suspension and GC already trace it.
                    if !alias {
                        output.push(Instruction::Pop);
                    }
                }
                if !alias {
                    // Class arity and missing base names precede argument resolution.
                    // Unknown placeholders validate only this node, without resolving
                    // its authored arguments ahead of their position in the plan.
                    let mut base = ty.clone();
                    if let TypeKind::Named { arguments, .. } = &mut base.kind {
                        for argument in arguments {
                            argument.kind = TypeKind::Unknown;
                        }
                    }
                    self.compile_type_at(
                        &base,
                        self.type_scopes.len(),
                        &mut BTreeSet::new(),
                        parameters,
                    )
                    .map_err(|error| Box::new((error, ty.span)))?;
                }
                // The builtin Class erases its argument without resolving it.
                // A declaration or type parameter named Class retains its own policy.
                let erases_arguments =
                    !parameter && declaration.is_none() && path.len() == 1 && path[0] == "Class";
                if !erases_arguments {
                    for argument in arguments {
                        self.type_resolution_plan(argument, parameters, output, use_site)?;
                    }
                }
                if alias {
                    output.push_at(Instruction::CheckTypeAliasInitialized, ty.span);
                }
            }
            TypeKind::Nullable(inner) | TypeKind::Constrained { base: inner, .. } => {
                self.type_resolution_plan(inner, parameters, output, use_site)?;
            }
            TypeKind::Union { types, .. } => {
                for ty in types {
                    self.type_resolution_plan(ty, parameters, output, use_site)?;
                }
            }
            TypeKind::Function {
                parameters: arguments,
                return_type,
            } => {
                for ty in arguments {
                    self.type_resolution_plan(ty, parameters, output, use_site)?;
                }
                self.type_resolution_plan(return_type, parameters, output, use_site)?;
            }
            _ => {}
        }
        // Validation that depends on resolved arguments (including alias
        // instantiation and Reference constraints) occurs after child resolution.
        let check = self
            .compile_type_at(ty, self.type_scopes.len(), &mut BTreeSet::new(), parameters)
            .map_err(|error| Box::new((error, ty.span)))?;
        if check.has_deferred_reference_validation() {
            output.push_at(
                Instruction::ValidateTypeArguments {
                    ty: check,
                    aliases: self.alias_frames.clone(),
                },
                ty.span,
            );
        }
        Ok(())
    }

    fn resolve_expression_type(
        &mut self,
        ty: &crate::syntax::Type,
        output: &mut Code,
    ) -> Option<TypeCheck> {
        // Let/function/collection/amendment parameter annotations retain the
        // declaration usage frame. Keep the established two-argument entrypoint
        // for callers outside compiler_expressions.rs.
        self.resolve_expression_type_for_use_site(ty, output, ReferenceUseSite::Declaration)
    }

    fn resolve_expression_type_for_use_site(
        &mut self,
        ty: &crate::syntax::Type,
        output: &mut Code,
        use_site: ReferenceUseSite,
    ) -> Option<TypeCheck> {
        match self.compile_type_at(
            ty,
            self.type_scopes.len(),
            &mut BTreeSet::new(),
            &self.active_type_parameters.clone(),
        ) {
            Ok(check) => {
                self.initialize_type_resolution_with_parameters(
                    ty,
                    &self.active_type_parameters.clone(),
                    output,
                    use_site,
                );
                Some(check)
            }
            Err(error) => {
                self.initialize_type_resolution_with_parameters(
                    ty,
                    &self.active_type_parameters.clone(),
                    output,
                    use_site,
                );
                output.push_at(Instruction::Fail(error.at(ty.span)), ty.span);
                None
            }
        }
    }

    fn reference_usage_context(
        &mut self,
        start: usize,
        ty: &crate::syntax::Type,
        use_site: ReferenceUseSite,
    ) {
        let expression_member = if matches!(use_site, ReferenceUseSite::ExpressionCastOrTest)
            && self.program.type_recipes.iter_from(start).any(
                |recipe| matches!(&recipe.reference, Some(Err(error)) if !error.frames.is_empty()),
            ) {
            Some(self.member.clone())
        } else {
            None
        };
        for recipe in self.program.type_recipes.iter_from_mut(start) {
            if let Some(Err(error)) = &mut recipe.reference
                && !error.frames.is_empty()
            {
                let span = if expression_member.is_some() {
                    ty.span
                } else if error.message
                    == "`Reference` referent type argument may not include type constraints."
                {
                    match &ty.kind {
                        crate::syntax::TypeKind::Named {
                            path,
                            name_spans,
                            arguments,
                        } if path.len() == 1
                            && !arguments.iter().any(authored_type_constraints) =>
                        {
                            name_spans[0]
                        }
                        _ => ty.span,
                    }
                } else {
                    ty.span
                };
                // Alias initialization can validate a qualified Reference before the
                // alias is expanded at its caller. That authored use still belongs to
                // the enclosing alias body (TypeNode.ownerAlias), not the module frame.
                let member = self
                    .resolved
                    .module
                    .members
                    .iter()
                    .find_map(|member| match &member.expr.kind {
                        crate::syntax::ExprKind::TypeAlias { ty: body, .. }
                            if body.span.start <= ty.span.start && ty.span.end <= body.span.end =>
                        {
                            Some(
                                PklString::from("#").concat(
                                    &self
                                        .resolved
                                        .module
                                        .source
                                        .entire()
                                        .range(member.name_span.start..member.name_span.end)
                                        .to_pkl_string(),
                                ),
                            )
                        }
                        _ => None,
                    })
                    // Declaration types resolve in their class/module initializer;
                    // expression types resolve in the current executable member.
                    // Both use the active lexical owner after alias ownership wins.
                    .unwrap_or_else(|| self.member.clone());
                error.frames.push(RuntimeFrame {
                    anonymous: false,
                    span,
                    member: member.clone(),
                    source: self.source.clone(),
                });
                error.span = Some(ty.span);
            }
        }
    }

    fn compile_type(&mut self, ty: &crate::syntax::Type) -> Result<TypeCheck, RuntimeError> {
        self.compile_type_at(
            ty,
            self.type_scopes.len(),
            &mut BTreeSet::new(),
            &self.active_type_parameters.clone(),
        )
        .map_err(|mut error| {
            if !error.frames.is_empty() {
                let span = if error.message
                    == "`Reference` referent type argument may not include type constraints."
                {
                    match &ty.kind {
                        crate::syntax::TypeKind::Named {
                            path,
                            name_spans,
                            arguments,
                        } if path.len() == 1
                            && !arguments.iter().any(authored_type_constraints) =>
                        {
                            name_spans[0]
                        }
                        _ => ty.span,
                    }
                } else {
                    ty.span
                };
                error.frames.push(RuntimeFrame {
                    anonymous: false,
                    span,
                    member: "".into(),
                    source: self.source.clone(),
                });
            }
            error.at(ty.span)
        })
    }

    /// Alias expansion retains the source section of the instantiated type node.
    /// Type parameters inherit the actual argument's source section.
    fn type_source_span(&self, ty: &crate::syntax::Type) -> Span {
        self.type_source_span_at(ty, &self.parameter_spans, &mut BTreeSet::new())
    }

    // Union labels use the actual node's source, preserving nullable wrappers
    // and alias use sites while forwarding already-bound type parameters.
    fn type_label_span(&self, ty: &crate::syntax::Type) -> Span {
        if let crate::syntax::TypeKind::Named {
            path, arguments, ..
        } = &ty.kind
            && path.len() == 1
            && arguments.is_empty()
            && let Some(span) = self.parameter_label_spans.get(&path[0])
        {
            return *span;
        }
        ty.unparenthesized_span.unwrap_or(ty.span)
    }

    fn type_argument_sources(&self) -> BTreeMap<PklString, TypeArgumentSource> {
        self.argument_spans
            .iter()
            .map(|(name, span)| {
                (
                    name.clone(),
                    TypeArgumentSource {
                        span: *span,
                        text: self
                            .resolved
                            .module
                            .source
                            .slice(*span)
                            .expect("type source span")
                            .to_pkl_string()
                            .into(),
                    },
                )
            })
            .collect()
    }

    fn type_source_span_at(
        &self,
        ty: &crate::syntax::Type,
        parameters: &BTreeMap<PklString, Span>,
        aliases: &mut BTreeSet<Span>,
    ) -> Span {
        use crate::syntax::{ExprKind, TypeKind};
        match &ty.kind {
            TypeKind::Nullable(inner) => self.type_source_span_at(inner, parameters, aliases),
            // ParenthesizedType resolves to its inner Java type node. Alias
            // use-site labels remain distinct from expanded failure spans.
            TypeKind::Union { .. } => ty.unparenthesized_span.unwrap_or(ty.span),
            TypeKind::Named {
                path, arguments, ..
            } if path.len() == 1 => {
                if let Some(span) = parameters.get(&path[0]).filter(|_| arguments.is_empty()) {
                    return *span;
                }
                if let Some((member, _)) = self.find_type(&path[0], self.type_scopes.len())
                    && let ExprKind::TypeAlias {
                        type_parameters,
                        ty: aliased,
                        ..
                    } = &member.expr.kind
                {
                    let substitutions = type_parameters
                        .iter()
                        .zip(arguments)
                        .map(|(parameter, argument)| {
                            (
                                parameter.name.clone(),
                                self.type_source_span_at(argument, parameters, aliases),
                            )
                        })
                        .collect();
                    if !aliases.insert(member.expr.span) {
                        return ty.span;
                    }
                    let span = self.type_source_span_at(aliased, &substitutions, aliases);
                    aliases.remove(&member.expr.span);
                    return span;
                }
                ty.unparenthesized_span.unwrap_or(ty.span)
            }
            _ => ty.unparenthesized_span.unwrap_or(ty.span),
        }
    }

    fn type_source_section(&self, span: Span) -> crate::type_source::TypeSourceSection {
        let owner = match &self.source {
            Some(source) => crate::type_source::TypeSourceOwner::External(source.clone()),
            None => {
                crate::type_source::TypeSourceOwner::Primary(self.resolved.module.source.clone())
            }
        };
        crate::type_source::TypeSourceSection::new(owner, span)
    }

    fn compile_type_at(
        &mut self,
        ty: &crate::syntax::Type,
        limit: usize,
        aliases: &mut BTreeSet<Span>,
        parameters: &BTreeMap<PklString, TypeCheck>,
    ) -> Result<TypeCheck, RuntimeError> {
        let check = self.compile_type_at_unlocated(ty, limit, aliases, parameters)?;
        if matches!(&ty.kind, crate::syntax::TypeKind::Named { path, arguments, .. }
            if path.len() == 1 && arguments.is_empty() && parameters.contains_key(&path[0]))
        {
            return Ok(check);
        }
        let section = self.type_source_section(ty.unparenthesized_span.unwrap_or(ty.span));
        Ok(check.with_optional_source_section(Some(section)))
    }

    fn compile_type_at_unlocated(
        &mut self,
        ty: &crate::syntax::Type,
        limit: usize,
        aliases: &mut BTreeSet<Span>,
        parameters: &BTreeMap<PklString, TypeCheck>,
    ) -> Result<TypeCheck, RuntimeError> {
        use crate::syntax::{ExprKind, TypeKind};
        if self.runtime_type_owner
            && self.runtime_type_fallback != Some(ty.span)
            && let TypeKind::Named {
                path, arguments, ..
            } = &ty.kind
            && path.len() == 1
            && !parameters.contains_key(&path[0])
        {
            let actuals = arguments
                .iter()
                .map(|argument| self.compile_type_at(argument, limit, aliases, parameters))
                .collect::<Result<Vec<_>, _>>()?;
            let fallback = if !matches!(
                path[0].as_str(),
                Some(
                    "DataSizeUnit"
                        | "DurationUnit"
                        | "Charset"
                        | "Comparable"
                        | "Char"
                        | "Uri"
                        | "Mixin"
                )
            ) && crate::diagnostic_names::BASE_PROPERTIES
                .iter()
                .any(|candidate| path[0] == *candidate)
            {
                let previous = self.runtime_type_fallback.replace(ty.span);
                let result = self.compile_type_at(ty, limit, aliases, parameters);
                self.runtime_type_fallback = previous;
                Some(result)
            } else {
                None
            };
            let result = self.declared_type(ty, actuals)?;
            if let TypeCheck::Declared { recipe, .. } = &result {
                self.program.type_recipes[*recipe].builtin = fallback;
            }
            return Ok(result);
        }
        Ok(match &ty.kind {
            TypeKind::Unknown => TypeCheck::Unknown,
            TypeKind::Nothing => TypeCheck::Nothing,
            TypeKind::Module => self.module_type(ty.span)?,
            TypeKind::Function {
                parameters: args,
                return_type,
            } => TypeCheck::Function {
                parameters: Some(
                    args.iter()
                        .map(|ty| self.compile_type_at(ty, limit, aliases, parameters))
                        .collect::<Result<_, _>>()?,
                ),
                result: Box::new(self.compile_type_at(return_type, limit, aliases, parameters)?),
                syntax: true,
            },
            TypeKind::String(value) => TypeCheck::StringLiteral(value.clone()),
            TypeKind::Nullable(inner) => TypeCheck::Nullable(Box::new(
                self.compile_type_at(inner, limit, aliases, parameters)?,
            )),
            TypeKind::Constrained { base, constraints } => {
                let base_check = self.compile_type_at(base, limit, aliases, parameters)?;
                let mut compiled = Vec::new();
                for expression in constraints {
                    let source: crate::source::SourceText = self
                        .resolved
                        .module
                        .source
                        .slice(expression.span)
                        .expect("constraint source span")
                        .to_pkl_string()
                        .into();
                    let expression = expression.unparenthesized();
                    if let Some(&block) = self.active_constraints.get(&expression.span) {
                        compiled.push(ConstraintCode {
                            section: Some(self.type_source_section(expression.span)),
                            block,
                            arguments: parameters.clone(),
                            argument_sources: self.type_argument_sources(),
                            span: expression.span,
                            module_environment: self.constraint_module,
                            source,
                        });
                        continue;
                    }
                    let block = self.block(Code::default(), Some(expression.span))?;
                    self.active_constraints.insert(expression.span, block);
                    // Compile one finite template. Recursive uses retain substitutions
                    // instead of expanding an unbounded family of specialized bodies.
                    let symbolic = parameters
                        .keys()
                        .map(|name| {
                            (
                                name.clone(),
                                TypeCheck::Parameter {
                                    block,
                                    name: name.clone(),
                                },
                            )
                        })
                        .collect();
                    let previous = std::mem::replace(&mut self.active_type_parameters, symbolic);
                    let previous_constructor_binding =
                        std::mem::replace(&mut self.constructor_parameters_bound, true);
                    let mut code = Code::default();
                    // CustomThisScope interrupts consecutive let qualification.
                    let enclosing_lets = std::mem::take(&mut self.let_members);
                    // Types inside a constraint retain the declaration's lexical scope,
                    // including when its alias is instantiated inside another owner.
                    let inner_scopes = self.type_scopes.split_off(limit);
                    let result = self.expression(expression, &mut code);
                    self.type_scopes.extend(inner_scopes);
                    self.let_members = enclosing_lets;
                    self.active_type_parameters = previous;
                    self.constructor_parameters_bound = previous_constructor_binding;
                    self.active_constraints.remove(&expression.span);
                    result?;
                    let writes_slots = code.let_scopes.iter().any(|scope| scope.writes_slot);
                    code.push(Instruction::Return);
                    self.program.blocks[block] = code.instructions;
                    let metadata = &mut self.program.metadata[block];
                    metadata.spans = code.spans;
                    metadata.inferred_entry_parent_spans = code.inferred_entry_parent_spans;
                    metadata.let_scopes = code.let_scopes;
                    metadata.type_casts = code.type_casts;
                    metadata.method_arities = code.method_arities;
                    metadata.constraint_root = writes_slots.then_some(expression.span);
                    metadata.inline_constraint = !writes_slots;
                    let mut origin = vec![RuntimeFrame {
                        anonymous: false,
                        span: ty.span,
                        member: self.member.clone(),
                        source: self.source.clone(),
                    }];
                    origin.extend(self.alias_frames.iter().rev().cloned());
                    self.constraint_origins
                        .insert(block, (origin, self.alias_frames.clone()));
                    let target = if writes_slots {
                        *self
                            .shared_constraint_roots
                            .entry(expression.span)
                            .or_insert(block)
                    } else {
                        block
                    };
                    compiled.push(ConstraintCode {
                        section: Some(self.type_source_section(expression.span)),
                        block: target,
                        arguments: parameters.clone(),
                        argument_sources: self.type_argument_sources(),
                        span: expression.span,
                        module_environment: self.constraint_module,
                        source,
                    });
                }
                TypeCheck::Constrained {
                    base: Box::new(base_check),
                    base_span: self.type_source_span(base),
                    constraints: compiled,
                }
            }
            TypeKind::Union { types, default } => TypeCheck::Union {
                string_literals: types
                    .iter()
                    .all(|ty| !ty.parenthesized && matches!(ty.kind, TypeKind::String(_))),
                spans: types.iter().map(|ty| self.type_source_span(ty)).collect(),
                // Alternatives retain their use-site labels even when checking an
                // alias expands to a different source section. Type parameters
                // alone inherit the actual argument's source section.
                labels: types.iter().map(|ty| self.type_label_span(ty)).collect(),
                types: types
                    .iter()
                    .map(|ty| self.compile_type_at(ty, limit, aliases, parameters))
                    .collect::<Result<_, _>>()?,
                default: *default,
            },
            TypeKind::Named {
                path, arguments, ..
            } if path.len() == 2 => {
                let actuals = arguments
                    .iter()
                    .map(|argument| {
                        let mut argument_aliases = aliases.clone();
                        self.compile_type_at(argument, limit, &mut argument_aliases, parameters)
                            .or_else(|error| self.deferred_type_error(error, argument.span))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let reference = self.compile_reference_type(ty, &actuals);
                let declared = self.declared_type(ty, actuals)?;
                let TypeCheck::Declared { recipe, arguments } = declared else {
                    unreachable!()
                };
                let mut specialized = self.program.type_recipes[recipe].clone();
                specialized.reference = Some(reference);
                let recipe = self.program.type_recipes.try_push(specialized)?;
                TypeCheck::Declared { recipe, arguments }
            }

            TypeKind::Named {
                path,
                arguments,
                name_spans,
            } if path.len() == 1 => {
                if let Some(parameter) = parameters.get(&path[0]).filter(|_| arguments.is_empty()) {
                    parameter.clone()
                } else if let Some((member, index)) = self.find_type(&path[0], limit) {
                    match &member.expr.kind {
                        ExprKind::Import { .. } => {
                            let actuals = arguments
                                .iter()
                                .map(|argument| {
                                    self.compile_type_at(argument, limit, aliases, parameters)
                                })
                                .collect::<Result<Vec<_>, _>>()?;
                            self.declared_type(ty, actuals)?
                        }
                        ExprKind::Class { .. } => {
                            if !arguments.is_empty() {
                                return Err(RuntimeError::new(
                                    ErrorKind::Type,
                                    format!(
                                        "Expected 0 type argument(s) but got {}.",
                                        arguments.len()
                                    ),
                                ));
                            }
                            let module = self
                                .source
                                .as_ref()
                                .map(|source| source.module_name.clone())
                                .or_else(|| {
                                    self.resolved
                                        .module
                                        .declaration
                                        .as_ref()
                                        .and_then(|declaration| declaration.name.as_ref())
                                        .map(|parts| PklString::join(parts, "."))
                                });
                            classes::specialized_type(module.as_ref(), &member.name).unwrap_or_else(
                                || TypeCheck::Class {
                                    module,
                                    id: self.declaration_base + member.expr.span.start,
                                    name: member.name,
                                },
                            )
                        }
                        ExprKind::TypeAlias {
                            type_parameters,
                            ty: aliased,
                            ..
                        } => {
                            if !arguments.is_empty() && type_parameters.len() != arguments.len() {
                                return Err(RuntimeError::new(
                                    ErrorKind::Type,
                                    format!(
                                        "Expected {} type argument(s) but got {}.",
                                        type_parameters.len(),
                                        arguments.len()
                                    ),
                                ));
                            }
                            // Resolve actual arguments at the use site before entering the alias's lexical scope.
                            let actual = arguments
                                .iter()
                                .map(|argument| {
                                    self.compile_type_at(argument, limit, aliases, parameters)
                                })
                                .collect::<Result<Vec<_>, _>>()?;
                            let metadata_arguments = actual.clone();
                            let actual = if actual.is_empty() {
                                vec![TypeCheck::Unknown; type_parameters.len()]
                            } else {
                                actual
                            };
                            let substitutions = type_parameters
                                .iter()
                                .zip(actual)
                                .map(|(parameter, actual)| (parameter.name.clone(), actual))
                                .collect();
                            if !aliases.insert(member.expr.span) {
                                return Err(RuntimeError::new(
                                    ErrorKind::Cycle,
                                    "Type alias definitions must not be cyclic.",
                                )
                                .at(ty.span));
                            }
                            let spans = type_parameters
                                .iter()
                                .zip(arguments)
                                .map(|(parameter, argument)| {
                                    (parameter.name.clone(), self.type_source_span(argument))
                                })
                                .collect();
                            let label_spans = type_parameters
                                .iter()
                                .zip(arguments)
                                .map(|(parameter, argument)| {
                                    (parameter.name.clone(), self.type_label_span(argument))
                                })
                                .collect();
                            let previous_spans =
                                std::mem::replace(&mut self.parameter_spans, spans);
                            let previous_label_spans =
                                std::mem::replace(&mut self.parameter_label_spans, label_spans);
                            let argument_spans = type_parameters
                                .iter()
                                .zip(arguments)
                                .map(|(parameter, argument)| {
                                    let span = match &argument.kind {
                                        TypeKind::Named {
                                            path, arguments, ..
                                        } if path.len() == 1
                                            && arguments.is_empty()
                                            && parameters.contains_key(&path[0]) =>
                                        {
                                            self.argument_spans
                                                .get(&path[0])
                                                .copied()
                                                .unwrap_or(argument.span)
                                        }
                                        _ => argument.span,
                                    };
                                    (parameter.name.clone(), span)
                                })
                                .collect();
                            let previous_argument_spans =
                                std::mem::replace(&mut self.argument_spans, argument_spans);
                            let previous_module =
                                std::mem::replace(&mut self.constraint_module, true);
                            let alias_owner: PklString = format!(
                                "#{}",
                                crate::diagnostic_names::quote_identifier(&member.name)
                            )
                            .into();
                            let previous_member =
                                std::mem::replace(&mut self.member, alias_owner.clone());
                            self.alias_frames.push(RuntimeFrame {
                                anonymous: false,
                                span: ty.span,
                                member: previous_member.clone(),
                                source: self.source.clone(),
                            });
                            let reference_start = self.program.type_recipes.next_id();
                            let result = self
                                .compile_type_at(aliased, index + 1, aliases, &substitutions)
                                .map_err(|mut error| {
                                    error.frames.push(RuntimeFrame {
                                        anonymous: false,
                                        span: aliased.span,
                                        member: alias_owner.clone(),
                                        source: self.source.clone(),
                                    });
                                    error
                                });
                            for recipe in self.program.type_recipes.iter_from_mut(reference_start) {
                                if let Some(Err(error)) = &mut recipe.reference {
                                    error.frames.push(RuntimeFrame {
                                        anonymous: false,
                                        span: aliased.span,
                                        member: alias_owner.clone(),
                                        source: self.source.clone(),
                                    });
                                }
                            }
                            self.alias_frames.pop();
                            self.constraint_module = previous_module;
                            self.member = previous_member;
                            self.parameter_spans = previous_spans;
                            self.parameter_label_spans = previous_label_spans;
                            self.argument_spans = previous_argument_spans;
                            let result = result?;
                            aliases.remove(&member.expr.span);
                            TypeCheck::NamedAlias {
                                declaration: self.declaration_base + member.expr.span.start,
                                name: self
                                    .resolved
                                    .module
                                    .source
                                    .entire()
                                    .range(member.name_span.start..member.name_span.end)
                                    .to_pkl_string(),
                                arguments: metadata_arguments,
                                default_origin: None,
                                expanded: Box::new(if arguments.is_empty() {
                                    TypeCheck::Alias(Box::new(result))
                                } else {
                                    TypeCheck::GenericAlias(Box::new(result))
                                }),
                            }
                        }
                        _ => {
                            return Err(RuntimeError::new(
                                ErrorKind::Type,
                                format!("Expected `{}` to be a type, but it is not.", path[0]),
                            )
                            .at(name_spans[0]));
                        }
                    }
                } else {
                    // User declarations were resolved above. The remaining
                    // candidates are scalar builtin identifiers; raw names
                    // retain their exact units in the ordinary missing-type error.
                    let Some(builtin_name) = path[0].as_str() else {
                        return Err(RuntimeError::new(
                            ErrorKind::MissingType,
                            PklString::from("Cannot find type `")
                                .concat(&path[0])
                                .concat(&"`.".into()),
                        )
                        .at(ty.span));
                    };
                    if path[0] == "Mixin" {
                        // Java resolves type arguments before alias arity validation.
                        let actuals = arguments
                            .iter()
                            .map(|argument| {
                                self.compile_type_at(argument, limit, aliases, parameters)
                            })
                            .collect::<Result<Vec<_>, _>>()?;
                        return self.declared_type(ty, actuals);
                    }
                    if path[0] == "Pair" {
                        if !arguments.is_empty() && arguments.len() != 2 {
                            return Err(RuntimeError::new(
                                ErrorKind::Type,
                                format!("Expected 2 type argument(s) but got {}.", arguments.len()),
                            ));
                        }
                        let arguments = arguments
                            .iter()
                            .map(|argument| {
                                Ok((
                                    self.compile_type_at(argument, limit, aliases, parameters)?,
                                    self.type_source_span(argument),
                                ))
                            })
                            .collect::<Result<Vec<_>, RuntimeError>>()?;
                        return Ok(TypeCheck::Pair { arguments });
                    }
                    if path[0] == "VarArgs" && !arguments.is_empty() {
                        if arguments.len() != 1 {
                            return Err(RuntimeError::new(
                                ErrorKind::Type,
                                format!("Expected 1 type argument(s) but got {}.", arguments.len()),
                            ));
                        }
                        return Ok(TypeCheck::VarArgs {
                            element: Box::new(self.compile_type_at(
                                &arguments[0],
                                limit,
                                aliases,
                                parameters,
                            )?),
                            span: self.type_source_span(ty),
                        });
                    }
                    if path[0] == "Class" {
                        if arguments.len() > 1 {
                            return Err(RuntimeError::new(
                                ErrorKind::Type,
                                format!("Expected 1 type argument(s) but got {}.", arguments.len()),
                            ));
                        }
                        // Pkl 0.32.0 erases Class<T> before resolving T.
                        return Ok(TypeCheck::MetaClass("Class".into()));
                    }
                    if path[0] == "Function"
                        || builtin_name
                            .strip_prefix("Function")
                            .is_some_and(|n| matches!(n, "0" | "1" | "2" | "3" | "4" | "5"))
                    {
                        let arity = builtin_name
                            .strip_prefix("Function")
                            .and_then(|n| n.parse::<usize>().ok());
                        let expected = arity.map_or(1, |n| n + 1);
                        if !arguments.is_empty() && arguments.len() != expected {
                            return Err(RuntimeError::new(
                                ErrorKind::Type,
                                format!(
                                    "Expected {expected} type argument(s) but got {}.",
                                    arguments.len()
                                ),
                            ));
                        }
                        let mut args = if arguments.is_empty() {
                            vec![TypeCheck::Unknown; expected]
                        } else {
                            arguments
                                .iter()
                                .map(|ty| self.compile_type_at(ty, limit, aliases, parameters))
                                .collect::<Result<Vec<_>, _>>()?
                        };
                        let result = Box::new(args.pop().expect("return type"));
                        return Ok(TypeCheck::Function {
                            parameters: arity.map(|_| args),
                            result,
                            syntax: false,
                        });
                    }
                    if path[0] == "Collection" {
                        if arguments.len() > 1 {
                            return Err(RuntimeError::new(
                                ErrorKind::Type,
                                format!("Expected 1 type argument(s) but got {}.", arguments.len()),
                            ));
                        }
                        let arguments = arguments
                            .iter()
                            .map(|argument| {
                                Ok((
                                    self.compile_type_at(argument, limit, aliases, parameters)?,
                                    self.type_source_span(argument),
                                ))
                            })
                            .collect::<Result<_, RuntimeError>>()?;
                        return Ok(TypeCheck::CollectionBase { arguments });
                    }
                    if !arguments.is_empty() {
                        if matches!(
                            builtin_name,
                            "Comparable"
                                | "Char"
                                | "Uri"
                                | "DataSizeUnit"
                                | "DurationUnit"
                                | "Charset"
                        ) {
                            let actuals = arguments
                                .iter()
                                .map(|argument| {
                                    self.compile_type_at(argument, limit, aliases, parameters)
                                })
                                .collect::<Result<Vec<_>, _>>()?;
                            return self.declared_type(ty, actuals);
                        }
                        let kind = match builtin_name {
                            "Listing" => crate::heap::CollectionKind::Listing,
                            "Mapping" => crate::heap::CollectionKind::Mapping,
                            "List" => crate::heap::CollectionKind::List,
                            "Map" => crate::heap::CollectionKind::Map,
                            "Set" => crate::heap::CollectionKind::Set,
                            _ if crate::diagnostic_names::BASE_PROPERTIES
                                .contains(&builtin_name) =>
                            {
                                return Err(unsupported_type("generic builtin types"));
                            }
                            _ => {
                                return Err(RuntimeError::new(
                                    ErrorKind::MissingType,
                                    format!("Cannot find type `{}`.", path[0]),
                                )
                                .at(name_spans[0]));
                            }
                        };
                        let arity = if matches!(
                            kind,
                            crate::heap::CollectionKind::Mapping | crate::heap::CollectionKind::Map
                        ) {
                            2
                        } else {
                            1
                        };
                        if arguments.len() != arity {
                            return Err(RuntimeError::new(
                                ErrorKind::Type,
                                format!(
                                    "Expected {arity} type argument(s) but got {}.",
                                    arguments.len()
                                ),
                            ));
                        }
                        let arguments = arguments
                            .iter()
                            .map(|argument| {
                                Ok((
                                    self.compile_type_at(argument, limit, aliases, parameters)?,
                                    self.type_source_span(argument),
                                ))
                            })
                            .collect::<Result<Vec<_>, RuntimeError>>()?;
                        return Ok(TypeCheck::ParameterizedCollection { kind, arguments });
                    }
                    match builtin_name {
                        name if classes::named_type(name).is_some() => {
                            classes::named_type(name).expect("builtin nominal type")
                        }
                        name if builtins::id(name).is_some() => TypeCheck::Class {
                            module: None,
                            id: builtins::id(name).expect("annotation class"),
                            name: PklString::from(name),
                        },
                        "Class" | "TypeAlias" => TypeCheck::MetaClass(builtin_name.into()),
                        "Any" => TypeCheck::Any,
                        "NonNull" => TypeCheck::NonNull,
                        "Null" => TypeCheck::Null,
                        "Boolean" => TypeCheck::Boolean,
                        "Int" => TypeCheck::Int,
                        "Float" => TypeCheck::Float,
                        "Duration" => TypeCheck::Duration,
                        "DataSizeUnit" | "DurationUnit" | "Charset" | "Comparable" | "Char"
                        | "Uri" => {
                            let actuals = arguments
                                .iter()
                                .map(|argument| {
                                    self.compile_type_at(argument, limit, aliases, parameters)
                                })
                                .collect::<Result<Vec<_>, _>>()?;
                            self.declared_type(ty, actuals)?
                        }
                        "Number" => TypeCheck::Number,
                        "String" => TypeCheck::String,
                        "Bytes" => TypeCheck::Bytes,
                        name @ ("Int8" | "Int16" | "Int32" | "UInt8" | "UInt16" | "UInt32"
                        | "UInt") => {
                            let (min, max) = match name {
                                "Int8" => (i8::MIN as i64, i8::MAX as i64),
                                "Int16" => (i16::MIN as i64, i16::MAX as i64),
                                "Int32" => (i32::MIN as i64, i32::MAX as i64),
                                "UInt8" => (0, u8::MAX as i64),
                                "UInt16" => (0, u16::MAX as i64),
                                "UInt32" => (0, u32::MAX as i64),
                                _ => (0, i64::MAX),
                            };
                            TypeCheck::IntegerAlias {
                                name: PklString::from(name),
                                min,
                                max,
                            }
                        }
                        "Dynamic" => TypeCheck::Collection(crate::heap::CollectionKind::Dynamic),
                        "Listing" => TypeCheck::Collection(crate::heap::CollectionKind::Listing),
                        "Mapping" => TypeCheck::Collection(crate::heap::CollectionKind::Mapping),
                        "List" => TypeCheck::Collection(crate::heap::CollectionKind::List),
                        "Map" => TypeCheck::Collection(crate::heap::CollectionKind::Map),
                        "Set" => TypeCheck::Collection(crate::heap::CollectionKind::Set),
                        _ if crate::diagnostic_names::BASE_PROPERTIES.contains(&builtin_name) => {
                            return Err(unsupported_type(&format!("type {}", path[0])));
                        }
                        _ => {
                            return Err(RuntimeError::new(
                                ErrorKind::MissingType,
                                format!("Cannot find type `{}`.", path[0]),
                            )
                            .at(ty.span));
                        }
                    }
                }
            }
            _ => return Err(unsupported_type("this composite type")),
        })
    }

    fn class_expression(
        &mut self,
        ty: &crate::syntax::Type,
        output: &mut Code,
        limit: usize,
        aliases: &mut BTreeSet<Span>,
    ) -> Result<(), RuntimeError> {
        self.initialize_type_resolution(ty, output);
        let check = self
            .compile_type_at(ty, limit, aliases, &BTreeMap::new())
            .map_err(|error| error.at(ty.span))?;
        let span = ty.unparenthesized_span.unwrap_or(ty.span);
        let label = self
            .resolved
            .module
            .source
            .slice(span)
            .expect("authored supertype span")
            .to_pkl_string();
        if check.has_declared() {
            output.push_at(Instruction::ResolvedSupertype { ty: check, label }, span);
            return Ok(());
        }
        if classes::BuiltinClass::from_type(&check).is_none()
            && !matches!(check.unaliased(), TypeCheck::Class { .. })
        {
            // Keep this failure inside the lazy class body, at the superclass
            // use site rather than an alias declaration or annotation context.
            output.push_at(Instruction::Fail(invalid_supertype(&label).at(span)), span);
            return Ok(());
        }
        self.class_value(&check, output, span)
    }

    fn class_value(
        &self,
        check: &TypeCheck,
        output: &mut Code,
        span: Span,
    ) -> Result<(), RuntimeError> {
        if check.has_declared() {
            output.push_at(Instruction::ResolvedClass(check.clone()), span);
            return Ok(());
        }
        if classes::BuiltinClass::from_type(check).is_some() {
            output.push_at(Instruction::BuiltinClass(check.clone()), span);
            return Ok(());
        }
        let TypeCheck::Class { id, .. } = check.unaliased() else {
            return Err(RuntimeError::new(ErrorKind::Type, "Expected a class type."));
        };
        for (index, members) in self.type_scopes.iter().enumerate().rev() {
            if let Some(member) = members
                .iter()
                .find(|member| self.declaration_base + member.expr_span.start == *id)
            {
                output.push_at(
                    Instruction::GetProperty {
                        name: member.name.clone(),
                        owner_depth: self.type_scopes.len() - 1 - index,
                        local: member.local,
                    },
                    span,
                );
                return Ok(());
            }
        }
        Err(invalid(
            "class declaration is outside the compiled lexical scopes",
        ))
    }

    fn default_property(
        &mut self,
        member: &crate::syntax::Member,
        output: &mut Code,
    ) -> Result<(), RuntimeError> {
        if crate::syntax::modifier_flags(&member.modifiers)
            & crate::syntax::ModifierKind::Abstract.flag()
            != 0
        {
            output.push_at(
                Instruction::AbstractMember {
                    name: self.member.clone(),
                    method: false,
                },
                member.span,
            );
            return Ok(());
        }
        if let Some(ty) = &member.ty {
            let check = self.compile_type(ty)?;
            let origin = RuntimeFrame {
                anonymous: false,
                source: self.source.clone(),
                span: self.type_source_span(ty),
                member: self.member.concat(&".default".into()),
            };
            if self.default_type_with_origin(&check, output, ty.span, Some(&origin))? {
                return Ok(());
            }
        }
        output.push_at(
            Instruction::Undefined(member.name.clone()),
            member.name_span,
        );
        Ok(())
    }

    fn default_type_with_origin(
        &mut self,
        check: &TypeCheck,
        output: &mut Code,
        span: Span,
        origin: Option<&RuntimeFrame>,
    ) -> Result<bool, RuntimeError> {
        if check.has_declared() {
            output.push_at(
                Instruction::ResolvedDefault {
                    ty: check.clone(),
                    origin: origin.cloned(),
                },
                span,
            );
            return Ok(true);
        }
        match check.resolved() {
            TypeCheck::VarArgs { .. } => output.push_at(
                Instruction::ResolvedDefault {
                    ty: check.clone(),
                    origin: origin.cloned(),
                },
                span,
            ),
            TypeCheck::Null => output.push_at(Instruction::Constant(Value::Null), span),
            TypeCheck::Nullable(inner) => {
                let defined = self.default_type_with_origin(inner, output, span, origin)?;
                output.push_at(Instruction::WrapNullableDefault { defined }, span);
            }
            TypeCheck::StringLiteral(value) => {
                output.push_at(Instruction::Constant(Value::String(value.clone())), span)
            }
            TypeCheck::Union {
                types,
                default: Some(index),
                ..
            } => return self.default_type_with_origin(&types[*index], output, span, origin),
            TypeCheck::Constrained { base, .. } => {
                return self.default_type_with_origin(base, output, span, origin);
            }
            TypeCheck::NamedAlias { declaration, .. }
                if amendment::is_base_mixin_alias(
                    *declaration,
                    self.program.base_mixin_declaration,
                ) =>
            {
                output.push_at(
                    Instruction::GenericConstructorParent {
                        cache: !check.contains_parameter(),
                        ty: check.clone(),
                        source: TypeArgumentSource {
                            span,
                            text: "Mixin".into(),
                        },
                    },
                    span,
                );
            }
            TypeCheck::Alias(inner)
            | TypeCheck::GenericAlias(inner)
            | TypeCheck::NamedAlias {
                expanded: inner, ..
            } => {
                return self.default_type_with_origin(inner, output, span, origin);
            }
            TypeCheck::CollectionBase { .. } => {
                output.push_at(Instruction::MakeList { length: 0 }, span);
            }
            TypeCheck::Collection(kind) | TypeCheck::ParameterizedCollection { kind, .. } => {
                if *kind == crate::heap::CollectionKind::List {
                    output.push_at(Instruction::MakeList { length: 0 }, span);
                } else {
                    output.push(Instruction::BeginCollection {
                        kind: *kind,
                        has_parent: false,
                        specialized: true,
                    });
                    if let TypeCheck::ParameterizedCollection { arguments, .. } = check.resolved()
                        && !matches!(
                            kind,
                            crate::heap::CollectionKind::Map | crate::heap::CollectionKind::Set
                        )
                    {
                        let (ty, span) = arguments.last().expect("collection value type");
                        if ty.default_depends_on_parameters() {
                            output.push_at(
                                Instruction::SetCollectionTypeDefault {
                                    ty: ty.clone(),
                                    origin: origin.cloned(),
                                },
                                *span,
                            );
                        } else if !matches!(ty.resolved(), TypeCheck::Unknown) {
                            let defined =
                                self.default_type_with_origin(ty, output, *span, origin)?;
                            output.push(Instruction::SetCollectionDefault {
                                defined,
                                origin: origin.cloned(),
                            });
                        }
                    }
                    output.push(Instruction::EndCollection);
                }
            }
            TypeCheck::Class { id, .. } => {
                if matches!(
                    classes::BuiltinClass::from_type(check),
                    Some(classes::BuiltinClass::DataSize | classes::BuiltinClass::IntSeq)
                ) {
                    return Ok(false);
                }
                if self.type_scopes.iter().flatten().any(|member| {
                    self.declaration_base + member.expr_span.start == *id && member.abstract_
                }) {
                    return Ok(false);
                }
                self.class_value(check, output, span)?;
                output.push_at(Instruction::DefaultClassInstance, span);
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    fn validate_modifiers(&self, member: &crate::syntax::Member) -> Result<(), RuntimeError> {
        use crate::syntax::{ExprKind, ModifierKind};
        for modifier in &member.modifiers {
            let supported = match modifier.kind {
                ModifierKind::Local
                | ModifierKind::Const
                | ModifierKind::Hidden
                | ModifierKind::Fixed => true,
                ModifierKind::Abstract => true,
                ModifierKind::Open => matches!(member.expr.kind, ExprKind::Class { .. }),
                ModifierKind::External => self.resolved.standard_library,
            };
            if !supported {
                return Err(
                    unsupported_type(&format!("{} modifiers", modifier.kind.spelling()))
                        .at(modifier.span),
                );
            }
        }
        Ok(())
    }
}

// StrictMath.pow specifies NaN for |base| == 1 with an infinite exponent,
// and for every NaN exponent. Generic libm follows C's different 1**NaN rule.
fn float_power(base: f64, exponent: f64) -> f64 {
    if exponent.is_nan() || (base.abs() == 1.0 && exponent.is_infinite()) {
        f64::NAN
    } else {
        libm::pow(base, exponent)
    }
}

fn cannot_infer_parent() -> RuntimeError {
    let mut error = RuntimeError::new(ErrorKind::Type, "Cannot tell which parent to amend.");
    error.hint = Some("Please specify a parent explicitly, for example `new Person { ... }` instead of `new { ... }`.".into());
    error
}
