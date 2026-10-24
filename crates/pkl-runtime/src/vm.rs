//! Bounded, resumable bytecode candidate. This is an architecture experiment, not full Pkl.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use crate::heap::{
    CacheKey, Class, Environment, ExecutableOwnerId, ExecutionSite, Function, Heap, HeapStats,
    LazyState, Member, Object, Scope, TypeNodeId,
};
use crate::logging::{EvaluationContext, LogRequest, LoggerFailure};
use crate::source::Span;
use crate::value::{ErrorKind, ObjectId, PklString, RuntimeError, RuntimeFrame, Value};
pub(crate) mod amendment;
pub(crate) mod annotations;
pub(crate) mod builtins;
mod code_storage;
mod collection_transforms;
pub mod collections;
pub(crate) mod deprecation;
pub use code_storage::CodeStorage;
mod compiler_expressions;
mod converters;
pub(crate) mod equality;
mod forcing;
mod generics;
mod guest_exceptions;
mod modules;
mod null;
mod query_lifecycle;
mod session_expressions;
mod stdlib_modules;
pub use session_expressions::{SessionExpressionError, compile_session_expression};
pub(crate) mod identity;
mod output;
mod pair;
mod pcf_format;
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
use typecheck::{TypeDestination, TypeRun};

#[derive(Clone, Debug)]
pub enum Instruction {
    LoadRefModule,
    LoadBaseModule,
    /// Invoke a checked Function0 under a guest exception handler.
    GuestCatch {
        or_null: bool,
    },
    ImportModule(modules::ImportSpec),
    LoadPrimary(Arc<crate::modules::ModuleSourceSpec>),
    EvaluateQuery(Arc<str>),
    PrepareModuleImports(crate::modules::UnitId),
    CheckModuleOutput,
    /// Eager initialization in the private standard-library profile.
    BootstrapStdlib,
    MatchModuleVersion,
    ReadModuleVersion,
    CheckModuleVersion,
    EncodeUtf8,
    OutputRenderer,
    RenderPcf {
        document: bool,
    },
    RenderJson {
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
    BuiltinTypeAlias(Arc<str>),
    BeginTypeAlias {
        declaration: usize,
        name: Arc<str>,
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
    SuperSubscript,
    EntryParent,
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
    StringRepeat,
    CollectionJoin,
    BeginCollectionTransform(collection_transforms::Transform),
    BeginCollectionProjection(collection_transforms::Projection),
    CollectionKeyOrder,
    CollectionTransformNext {
        end: usize,
    },
    CollectionTransformProject {
        next: usize,
    },
    AppendStrictElement,
    EndCollectionTransform,
    StringEncode,
    Primitive(primitives::Operation),
    MakePair,
    PairConstructor,
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
    GetMember(Arc<str>),
    GetMethod(Arc<str>),
    GetImplicitProperty(Arc<str>),
    GetImplicitMethod(Arc<str>),
    GetConstMember(Arc<str>),
    GetConstMethod(Arc<str>),
    RequireConstProperty {
        name: Arc<str>,
        owner_depth: usize,
        local: bool,
        method: bool,
    },
    ResolveType(TypeCheck),
    ResolvedClass(TypeCheck),
    ResolvedDefault {
        ty: TypeCheck,
        origin: Option<RuntimeFrame>,
    },
    ResolveClassType {
        name: Arc<str>,
        local: bool,
    },
    GetProperty {
        name: Arc<str>,
        owner_depth: usize,
        local: bool,
    },
    GetMethodProperty {
        name: Arc<str>,
        owner_depth: usize,
        local: bool,
    },
    GetSuperMember(Arc<str>),
    GetSuperMethod(Arc<str>),
    GetConstSuperMember(Arc<str>),
    GetConstSuperMethod(Arc<str>),
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
    Undefined(Arc<str>),
    AbstractMember {
        name: PklString,
        method: bool,
    },
    Unsupported(Arc<str>),
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
        name: Arc<str>,
    },
    /// Declaration identity and arguments are retained independently of validation.
    NamedAlias {
        declaration: usize,
        name: Arc<str>,
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
    MetaClass(Arc<str>),
    IntegerAlias {
        name: Arc<str>,
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
        name: Arc<str>,
        module: Option<Arc<str>>,
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
    block: usize,
    /// Actual arguments for this constraint template, possibly referring to an outer template.
    arguments: BTreeMap<String, TypeCheck>,
    argument_sources: BTreeMap<String, TypeArgumentSource>,
    span: Span,
    module_environment: bool,
    source: Arc<str>,
}

/// Authored source of an instantiated type node, kept separate from type identity.
#[derive(Clone, Debug)]
pub struct TypeArgumentSource {
    pub span: Span,
    pub text: Arc<str>,
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
    /// Mixin's identity default belongs to its declaration, not to an
    /// arbitrary structurally equivalent function type. Aliases and selected
    /// default wrappers delegate to that declaration without losing its identity.
    fn mixin_default(&self) -> bool {
        let mut ty = self;
        loop {
            ty = match ty.resolved() {
                Self::NamedAlias { declaration, .. } if *declaration == amendment::MIXIN_ID => {
                    return true;
                }
                Self::Alias(inner)
                | Self::GenericAlias(inner)
                | Self::NamedAlias {
                    expanded: inner, ..
                }
                | Self::Nullable(inner)
                | Self::Constrained { base: inner, .. } => inner,
                Self::Union {
                    types,
                    default: Some(index),
                    ..
                } => &types[*index],
                _ => return false,
            };
        }
    }
    /// Whether this type's selected default ultimately comes from Collection.
    /// Nullable defaults retain the empty List for empty amendments.
    fn collection_base_default(&self) -> bool {
        let mut ty = self;
        loop {
            ty = match ty.unaliased() {
                Self::CollectionBase { .. } => return true,
                Self::Nullable(inner) | Self::Constrained { base: inner, .. } => inner,
                Self::Union {
                    types,
                    default: Some(index),
                    ..
                } => &types[*index],
                _ => return false,
            };
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
                            &reference::normalize(referent, ""),
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
                (name.as_ref(), value),
                ("TypeAlias", Value::TypeAlias(_)) | ("Class", Value::Class(_))
            ),
            Self::Function { parameters, .. } => {
                matches!(value, Value::Function(id) if heap.get(*id).is_ok_and(|object| parameters.as_ref().is_none_or(|parameters| object.function.as_ref().is_some_and(|function| function.arity == parameters.len()) || object.native_function.as_ref().is_some_and(|function| parameters.len() == match function.name.as_ref() { "Reference" => 3, "containsKey" | "getOrNull" | "contains" => 1, _ => 0 }))))
            }
            Self::RefDomain => Self::Class {
                module: None,
                id: reference::DOMAIN_ID,
                name: Arc::from("Domain"),
            }
            .accepts(value, heap),
            Self::IntegerAlias { min, max, .. } => {
                matches!(value,Value::Int(i) if i>=min && i<=max)
            }
            Self::Nothing => false,
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
                matches!(value, Value::Object(id) if heap.get(*id).is_ok_and(|object| object.collection_kind == *kind && !object.is_module && object.reference.is_none() && !object.ref_module) && collections::typed_class(heap, *id).is_ok_and(|class| class.is_none()))
            }
            Self::ParameterizedCollection { kind, .. } => {
                Self::Collection(*kind).accepts(value, heap)
            }
            Self::CollectionBase { .. } => {
                Self::Collection(crate::heap::CollectionKind::List).accepts(value, heap)
                    || Self::Collection(crate::heap::CollectionKind::Set).accepts(value, heap)
            }
            Self::Class { id, .. } => {
                let &Value::Object(mut object) = value else {
                    return false;
                };
                let mut visited = BTreeSet::new();
                loop {
                    if !visited.insert(object) {
                        return false;
                    }
                    let Ok(value) = heap.get(object) else {
                        return false;
                    };
                    if value
                        .class_definition
                        .as_ref()
                        .is_some_and(|class| class.id == *id)
                    {
                        return true;
                    }
                    let Some(parent) = value.parent else {
                        return false;
                    };
                    object = parent;
                }
            }
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
    pub(crate) fixed_span: Option<Span>,
    pub(crate) const_span: Option<Span>,
    pub(crate) initialization_error: Option<RuntimeError>,
    pub(crate) method_signature: Option<(String, usize)>,
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

#[derive(Clone, Debug)]
pub struct Program {
    pub(crate) units: CodeStorage<modules::ModuleUnit>,
    next_declaration: usize,
    type_recipes: CodeStorage<type_resolution::TypeRecipe>,
    alias_definitions: BTreeMap<usize, type_resolution::AliasDefinition>,
    pub module_name: Arc<str>,
    pub module_template: Option<usize>,
    /// Imports, classes and aliases exist before module annotation evaluation.
    pub(crate) initial_module_members: Vec<Member>,
    pub blocks: CodeStorage<Vec<Instruction>>,
    pub metadata: CodeStorage<BlockMetadata>,
    pub objects: CodeStorage<Vec<Member>>,
    builtin_objects: Vec<Vec<Member>>,
    pub entry: usize,
    pub(crate) reference_render_block: usize,
    pub(crate) typed_string_block: usize,
    pub(crate) string_repeat_block: usize,
    pub(crate) collection_join_blocks: [usize; 2],
    collection_transform_blocks: [usize; 11],
    collection_projection_blocks: [usize; 6],
    null_constructor_block: usize,
    pub(crate) string_encode_block: usize,
    pub(crate) pair_constructor_block: usize,
    primitive_methods: BTreeMap<(Arc<str>, Arc<str>), usize>,
    pub(crate) amended_function_block: usize,
    pub(crate) identity_mixin_block: usize,
    pub(crate) access_members: Vec<Member>,
    pub(crate) builtin_members: BTreeMap<usize, Vec<Member>>,
    pub(crate) constraint_origins: BTreeMap<usize, (Vec<RuntimeFrame>, Vec<RuntimeFrame>)>,
}

impl Program {
    pub fn new(mut blocks: Vec<Vec<Instruction>>) -> Self {
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
        let string_repeat_block = strings::repeat_block(&mut blocks, &mut metadata);
        let collection_join_blocks = collections::join_blocks(&mut blocks, &mut metadata);
        let collection_transform_blocks = collection_transforms::blocks(&mut blocks, &mut metadata);
        let collection_projection_blocks =
            collection_transforms::projection_blocks(&mut blocks, &mut metadata);
        let null_constructor_block = null::constructor_block(&mut blocks, &mut metadata);
        let string_encode_block = strings::encode_block(&mut blocks, &mut metadata);
        let pair_constructor_block = pair::constructor_block(&mut blocks, &mut metadata);
        let primitive_methods = primitives::blocks(&mut blocks, &mut metadata);
        let (amended_function_block, identity_mixin_block) =
            amendment::blocks(&mut blocks, &mut metadata);
        Self {
            units: CodeStorage::default(),
            next_declaration: 0,
            type_recipes: CodeStorage::default(),
            alias_definitions: BTreeMap::new(),
            module_name: Arc::from(""),
            module_template: None,
            initial_module_members: Vec::new(),
            metadata: metadata.into(),
            blocks: blocks.into(),
            objects: CodeStorage::default(),
            builtin_objects: objects,
            entry: 0,
            reference_render_block,
            typed_string_block,
            string_repeat_block,
            collection_join_blocks,
            collection_transform_blocks,
            collection_projection_blocks,
            null_constructor_block,
            string_encode_block,
            pair_constructor_block,
            primitive_methods,
            amended_function_block,
            identity_mixin_block,
            access_members,
            builtin_members,
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
    pub uri: Arc<str>,
    /// The read declaration's original ModuleKey URI when associated with a
    /// registered unit; otherwise its explicit source URI. This is independent
    /// of the caller and is never replaced by the canonical resolved key.
    pub module_uri: Option<Arc<str>>,
    /// Prepared URI from a successful declaration-ModuleKey resolution.
    pub resolved_uri: Option<Arc<str>>,
    /// Only an absent resource becomes null; parse, policy and reader errors propagate.
    pub nullable: bool,
}

impl ReadRequest {
    /// Resolved URI delivered to the reader. URI semantics remain in the engine.
    pub fn resolved_uri(&self) -> Result<String, RuntimeError> {
        crate::host::resources::resolved_uri(self)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum VmPoll {
    Ready(Result<Value, RuntimeError>),
    Pending(ReadRequest),
    Log(LogRequest),
    Module(crate::modules::ModuleRequest),
    Yielded,
}

#[derive(Clone)]
struct Force {
    entry: bool,
    receiver: ObjectId,
    owner: ObjectId,
    name: Arc<str>,
    cache: Option<CacheKey>,
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
    force: Option<Force>,
    lambda: bool,
    type_run: Option<TypeRun>,
    type_resolution: Option<type_resolution::DeclaredRun>,
    generic_default: Option<generics::GenericDefaultRun>,
    equality_run: Option<equality::EqualityRun>,
    deep_force: Option<forcing::ForceRun>,
    warning: Option<deprecation::PendingWarning>,
    method_call_site: Option<Box<deprecation::CallSite>>,
    pcf_render: Option<rendering::RenderRun>,
    collection_join: Option<collections::JoinRun>,
    collection_transform: Option<collection_transforms::TransformRun>,
    function_owner: Option<ObjectId>,
    amendment_arguments: Option<Vec<Value>>,
    amendment_end: Option<usize>,
    prepared_hashes: Option<Vec<Value>>,
    key_search: Option<equality::KeySearch>,
    return_to_type: bool,
    diagnostic_member: Option<PklString>,
    callsite_hidden: bool,
    builders: Vec<ObjectId>,
    generator_scopes: Vec<identity::GeneratorScope>,
    module_load: Option<modules::ModuleRun>,
    uri_resolution: Option<uri_resolution::Resolution>,
    module_receiver: Option<ObjectId>,
}

impl Frame {
    fn hides_callsite(&self) -> bool {
        self.callsite_hidden
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
    Module(crate::modules::ModuleRequest),
}
impl PendingRequest {
    fn id(&self) -> u64 {
        match self {
            Self::Read(request) => request.id,
            Self::Log(request) => request.id,
            Self::Module(request) => request.id,
        }
    }
    fn poll(&self) -> VmPoll {
        match self {
            Self::Read(request) => VmPoll::Pending(request.clone()),
            Self::Log(request) => VmPoll::Log(request.clone()),
            Self::Module(request) => VmPoll::Module(request.clone()),
        }
    }
}

enum State {
    Idle,
    Running,
    Waiting(PendingRequest),
    Finished(Result<Value, RuntimeError>),
    Closed,
}

struct SuspendedContext {
    public_primary: Option<ObjectId>,
    /// Caller locations captured at the Host boundary, innermost first. The
    /// pending reader has no language call node for a callback evaluation.
    diagnostic_callers: Vec<RuntimeFrame>,
    frames: Vec<Frame>,
    stack: identity::OperandStack,
    inputs: Vec<Value>,
    state: State,
}

pub struct Vm {
    program: Arc<Program>,
    context: EvaluationContext,
    callback_closing: bool,
    log_buffer: Vec<u16>,
    heap: Heap,
    frames: Vec<Frame>,
    stack: identity::OperandStack,
    inputs: Vec<Value>,
    state: State,
    suspended: Vec<SuspendedContext>,
    next_request: u64,
    instructions: u64,
    member_evaluations: u64,
    code_validation: Result<(), RuntimeError>,
    power_assertions: bool,
    /// Keys belong to this immutable Program; values belong only to this evaluator.
    /// Reentering or restarting a declaration retains its first captured environment.
    public_primary: Option<ObjectId>,
    modules_original: modules::ModuleCache,
    modules_stdlib: modules::ModuleCache,
    modules_resolved: modules::ModuleCache,
    resources: BTreeMap<crate::host::resources::CacheKey, Option<Value>>,
    class_singletons: BTreeMap<usize, ObjectId>,
    alias_singletons: BTreeMap<usize, ObjectId>,
    /// Weak handles: live empty strict values share Java's EMPTY identity, but
    /// this lookup does not keep discarded values or producer scopes alive.
    empty_collections: [Option<ObjectId>; 3],
    resolved_declarations: BTreeMap<(usize, ObjectId), type_resolution::DeclarationValue>,
    /// Static call targets own code state, never captured receivers/environments.
    executable_roots: BTreeMap<usize, ExecutableOwnerId>,
    code_arenas: Vec<query_lifecycle::CodeArena>,
    public_operation_entries: BTreeSet<usize>,
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
        // independently of repeated evaluation state creation.
        let code_validation = if program
            .blocks
            .iter()
            .flatten()
            .any(|instruction| matches!(instruction, Instruction::Constant(value) if value.heap_id().is_some()))
        {
            Err(invalid(
                "object constants cannot be stored in compiled code",
            ))
        } else {
            Ok(())
        };
        Self {
            program,
            context,
            callback_closing: false,
            log_buffer: Vec::new(),
            heap: Heap::new(),
            frames: Vec::new(),
            stack: identity::OperandStack::default(),
            inputs: Vec::new(),
            state: State::Idle,
            suspended: Vec::new(),
            next_request: 1,
            instructions: 0,
            member_evaluations: 0,
            code_validation: registration.and(code_validation),
            power_assertions: false,
            public_primary: None,
            modules_original: modules::ModuleCache::default(),
            modules_stdlib: modules::ModuleCache::default(),
            modules_resolved: modules::ModuleCache::default(),
            resources: BTreeMap::new(),
            class_singletons: BTreeMap::new(),
            alias_singletons: BTreeMap::new(),
            empty_collections: [None; 3],
            resolved_declarations: BTreeMap::new(),
            executable_roots: BTreeMap::new(),
            code_arenas: Vec::new(),
            public_operation_entries: BTreeSet::new(),
        }
    }

    pub fn set_power_assertions(&mut self, enabled: bool) {
        self.power_assertions = enabled;
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
        self.frames.push(Frame {
            guest_catch: None,
            block: entry,
            pc: 0,
            base: 0,
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
            collection_join: None,
            collection_transform: None,
            function_owner: None,
            amendment_arguments: None,
            amendment_end: None,
            prepared_hashes: None,
            key_search: None,
            return_to_type: false,
            diagnostic_member: None,
            callsite_hidden: false,
            builders: Vec::new(),
            generator_scopes: Vec::new(),
            module_load: None,
            uri_resolution: None,
            module_receiver: None,
        });
        self.state = State::Running;
        Ok(())
    }

    pub(crate) fn start_failed(&mut self, error: RuntimeError) -> Result<(), RuntimeError> {
        self.available()?;
        self.log_buffer.clear();
        self.stack.clear();
        self.inputs.clear();
        self.frames.clear();
        self.state = State::Finished(Err(error));
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
            .rev()
            .skip(1)
            .filter_map(|frame| {
                if frame.hides_callsite() {
                    return None;
                }
                let metadata = self.program.metadata.get(frame.block)?;
                let span = (*metadata.spans.get(frame.pc.saturating_sub(1))?)?;
                Some(RuntimeFrame {
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
        Ok(())
    }

    /// Called only after the Host has captured the completed nested result.
    pub(crate) fn finish_reentry(&mut self) {
        if matches!(self.state, State::Finished(_)) {
            self.restore_context();
        }
    }

    fn restore_context(&mut self) {
        if let Some(context) = self.suspended.pop() {
            self.public_primary = context.public_primary;
            self.frames = context.frames;
            self.stack = context.stack;
            self.inputs = context.inputs;
            self.state = context.state;
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

    pub fn start_member(&mut self, receiver: ObjectId, name: &str) -> Result<(), RuntimeError> {
        self.available()?;
        self.heap.get(receiver)?;
        self.log_buffer.clear();
        self.inputs.clear();
        self.stack.clear();
        self.state = State::Running;
        if matches!(
            self.heap.get(receiver)?.collection_kind,
            crate::heap::CollectionKind::Map | crate::heap::CollectionKind::Set
        ) && self.collection_builtin(receiver, name)?
        {
            self.finish_top()?;
            return Ok(());
        }
        if let Err(error) = self.force(receiver, receiver, Arc::from(name), false, true, false) {
            self.fail(error);
        } else if self.frames.is_empty() {
            self.finish_top()?;
        }
        Ok(())
    }

    pub fn poll(&mut self, budget: usize) -> VmPoll {
        for _ in 0..budget {
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
            self.instructions += 1;
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
                self.state = State::Running;
                if let Some(warning) = self
                    .frames
                    .last_mut()
                    .and_then(|frame| frame.warning.take())
                {
                    self.publish_method_call(warning)?;
                } else {
                    self.frames
                        .last_mut()
                        .and_then(|frame| frame.deep_force.as_mut())
                        .ok_or_else(|| invalid("log outside logger continuation"))?
                        .acknowledge_log()?;
                }
            }
            Err(error) => self.fail(RuntimeError::new(ErrorKind::Host, error.message)),
        }
        Ok(())
    }

    pub(crate) fn close_from_callback(&mut self, id: u64) -> Result<(), RuntimeError> {
        if !matches!(&self.state, State::Waiting(request @ (PendingRequest::Read(_) | PendingRequest::Log(_) | PendingRequest::Module(_))) if request.id() == id)
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
        self.suspended
            .iter()
            .flat_map(|context| &context.frames)
            .chain(self.frames.iter())
            .flat_map(|frame| &frame.environment.scopes)
            .map(|scope| scope.receiver)
            .find(|id| {
                self.heap
                    .get(*id)
                    .is_ok_and(|object| object.module_unit == Some(crate::modules::UnitId(0)))
            })
            .map(Value::Object)
            .or_else(|| match &self.state {
                State::Finished(Ok(Value::Object(id)))
                    if self.heap.get(*id).is_ok_and(|object| {
                        object.module_unit == Some(crate::modules::UnitId(0))
                    }) =>
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
        self.alias_singletons.clear();
        self.empty_collections = [None; 3];
        self.resolved_declarations.clear();
        self.executable_roots.clear();
        self.code_arenas.clear();
        self.public_operation_entries.clear();
        self.heap.release();
        self.program = Arc::new(Program::new(Vec::new()));
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
    pub fn instructions_executed(&self) -> u64 {
        self.instructions
    }
    pub fn member_evaluations(&self) -> u64 {
        self.member_evaluations
    }

    pub fn member_state(&self, receiver: ObjectId, name: &str) -> Result<LazyState, RuntimeError> {
        let key = CacheKey {
            entry: false,
            owner: None,
            name: Arc::from(name),
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
        let mut roots = external_roots.to_vec();
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
        self.declaration_code_roots(&mut roots);
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
            roots.extend(stack.iter().cloned());
            roots.extend(inputs.iter().cloned());
            if let State::Finished(Ok(value)) = state {
                roots.push(value.clone());
            }
            for frame in frames {
                roots.extend(frame.module_receiver.map(Value::Object));
                roots.extend(
                    frame
                        .module_load
                        .as_ref()
                        .and_then(|run| run.receiver)
                        .map(Value::Object),
                );
                roots.extend(frame.builders.iter().copied().map(Value::Object));
                for scope in &frame.generator_scopes {
                    scope.roots(&mut roots);
                }
                roots.extend(frame.prepared_hashes.iter().flatten().cloned());
                roots.extend(frame.environment.locals.iter().cloned());
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
                    run.roots(&mut roots);
                }
                roots.extend(frame.function_owner.map(Value::Function));
                roots.extend(frame.amendment_arguments.iter().flatten().cloned());
                if let Some(run) = &frame.collection_join {
                    run.roots(&mut roots);
                }
                if let Some(run) = &frame.collection_transform {
                    run.roots(&mut roots);
                }
                if let Some(run) = &frame.pcf_render {
                    run.roots(&mut roots);
                }
                if let Some(run) = &frame.deep_force {
                    run.roots(&mut roots);
                }
                roots.extend(
                    frame
                        .method_call_site
                        .as_ref()
                        .map(|site| site.executable.root()),
                );
                if let Some(warning) = &frame.warning {
                    warning.roots(&mut roots);
                }
                if let Some(run) = &frame.equality_run {
                    run.roots(&mut roots);
                }
                if let Some(run) = &frame.type_resolution {
                    run.roots(&mut roots);
                }
                if let Some(run) = &frame.generic_default {
                    run.roots(&mut roots);
                }
                if let Some(run) = &frame.type_run {
                    run.roots(&mut roots);
                }
                for scope in &frame.environment.scopes {
                    roots.push(Value::Object(scope.receiver));
                    roots.push(Value::Object(scope.owner));
                }
                if let Some(force) = &frame.force {
                    roots.push(Value::Object(force.receiver));
                    roots.push(Value::Object(force.owner));
                    for cast in &force.collection_casts {
                        typecheck::environment_roots(&cast.environment, &mut roots);
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
        }
        let stats = self.heap.collect(&roots);
        self.retire_unreachable_code();
        stats
    }

    fn fail(&mut self, mut error: RuntimeError) {
        if error.undefined_value && self.catch_force_undefined(self.guest_catch_boundary(&error)) {
            return;
        }
        if error.undefined_value && error.hint.is_none() {
            error.hint = self.pcf_undefined_hint();
        }
        let catch_boundary = self.guest_catch_boundary(&error);
        let unavailable_origin = error.unavailable_origin;
        if unavailable_origin {
            // Consume this origin once: cached exceptions already contain the
            // visible stack. Unwinding and inserted let frames are unaffected.
            error.unavailable_origin = false;
        }
        let unwind_start = catch_boundary.map_or(0, |index| index + 1);
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
        self.reset_force_marks(unwind_start);
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
        let caught_undefined = error.undefined_value
            && error.kind == ErrorKind::MissingMember
            && self.frames.last().is_some_and(|frame| {
                matches!(
                    instruction(frame),
                    Some(Instruction::Undefined(_) | Instruction::EntryParent)
                )
            });
        let diagnostic_count = if caught_undefined {
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
        let mut let_frames = BTreeMap::new();
        let mut logical_targets = Vec::with_capacity(self.frames.len());
        for frame in diagnostic_frames {
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
            logical_targets.push(target);
        }
        for (index, frame) in diagnostic_frames[unwind_start.min(diagnostic_frames.len())..]
            .iter()
            .rev()
            .enumerate()
        {
            // A recoverable mismatch becomes VmException only at PklRootNode.
            // Let bodies within that root do not catch the original mismatch.
            if index == 0
                && error.type_diagnostic.is_some()
                && !self
                    .program
                    .metadata
                    .get(frame.block)
                    .is_some_and(|metadata| {
                        metadata.type_casts.contains(&frame.pc.saturating_sub(1))
                    })
            {
                continue;
            }
            if let Some(scope) = self
                .program
                .metadata
                .get(frame.block)
                .filter(|metadata| !metadata.inline_constraint)
                .and_then(|metadata| {
                    metadata
                        .let_scopes
                        .iter()
                        .rev()
                        .find(|scope| scope.body.contains(&frame.pc.saturating_sub(1)))
                })
            {
                let_frames.insert(
                    logical_targets[diagnostic_frames.len() - 1 - index],
                    scope.frame.clone(),
                );
            }
        }
        if error.kind == ErrorKind::MissingMember
            && let Some(frame) = self.frames.last()
            && let Some(
                Instruction::GetProperty { name, .. } | Instruction::GetImplicitProperty(name),
            ) = self
                .program
                .blocks
                .get(frame.block)
                .and_then(|code| code.get(frame.pc.saturating_sub(1)))
        {
            error.message = format!("Cannot find property `{name}`.").into();
            error.hint = self.implicit_member_suggestions(frame, name, -1);
        }
        if error.kind == ErrorKind::MissingMember
            && let Some(frame) = self.frames.last()
            && let Some(
                Instruction::GetMethodProperty { name, .. } | Instruction::GetImplicitMethod(name),
            ) = self
                .program
                .blocks
                .get(frame.block)
                .and_then(|code| code.get(frame.pc.saturating_sub(1)))
        {
            error.message = format!("Cannot find method `{name}`.").into();
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
                && let Some(Some(span)) = metadata.spans.get(frame.pc.saturating_sub(1))
            {
                let reading_inferred_parent = frame_index + 1 < diagnostic_frames.len()
                    && matches!(instruction(frame), Some(Instruction::InferPropertyParent));
                if !frame.hides_callsite()
                    && !reading_inferred_parent
                    && !(unavailable_origin && frame_index + 1 == diagnostic_frames.len())
                {
                    error.frames.push(RuntimeFrame {
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
                    });
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
                        source: metadata.source.clone(),
                        span: value_span,
                        member: metadata.member.clone(),
                    });
                }
            }
            if let Some(run) = &frame.type_run {
                error.frames.extend(run.fatal_value_frames().cloned());
            }
            if let Some(inserted) = let_frames.remove(&logical_targets[frame_index]) {
                error.frames.push(inserted);
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
                    source: None,
                    span,
                    member: run.member.clone(),
                });
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
        for frame in &self.frames[unwind_start..] {
            if let Some(force) = &frame.force
                && let Some(key) = &force.cache
                && !self
                    .suspended
                    .iter()
                    .flat_map(|context| &context.frames)
                    .chain(self.frames[..unwind_start].iter())
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
        if !error.cached_module_frames && error.module_failure_id.is_some() {
            error.cached_module_frames = catch_boundary.is_none();
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
        for frame in &self.frames[unwind_start..] {
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
                if unit.is_singleton() {
                    self.modules_stdlib.insert(
                        identity.original_uri.clone(),
                        modules::CachedModule::Failed(error.clone()),
                    );
                } else {
                    self.modules_original.insert(
                        identity.original_uri.clone(),
                        modules::CachedModule::Failed(error.clone()),
                    );
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
                    self.modules_resolved.insert(
                        resolved.resolved_uri.clone(),
                        modules::CachedModule::Failed(error.clone()),
                    );
                }
            }
        }
        if let Some(index) = catch_boundary {
            let value = self.render_caught_exception(&error);
            let handler = self.frames[index]
                .guest_catch
                .take()
                .expect("catch boundary");
            self.frames.truncate(index + 1);
            self.stack.truncate(handler.stack);
            self.stack.push(Value::String(value));
            self.state = State::Running;
        } else {
            self.frames.clear();
            self.stack.clear();
            self.inputs.clear();
            self.state = State::Finished(Err(error));
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
        name: Arc<str>,
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
        name: Arc<str>,
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
        name: Arc<str>,
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

    #[allow(clippy::too_many_arguments)]
    fn force_key_mode_prepared(
        &mut self,
        receiver: ObjectId,
        initial_owner: ObjectId,
        name: Arc<str>,
        local: bool,
        mut memoize: bool,
        method: bool,
        entry: bool,
        predicate: bool,
        hash_prepared: bool,
        resolved_member: Option<Member>,
    ) -> Result<(), RuntimeError> {
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
        if memoize && let Some(value) = self.cached_slot(receiver, &key)? {
            self.stack.push(value);
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
                        format!("missing local member {name}"),
                    ));
                }
                owner = match object.parent {
                    Some(parent) => parent,
                    None if method
                        && name.as_ref() == "toString"
                        && (object.class_definition.is_none()
                            || object.class_superclass_initialized) =>
                    {
                        let function =
                            self.native_reference_method(Value::Object(receiver), "objectToString");
                        self.stack.push(function);
                        return Ok(());
                    }
                    None => {
                        // A super method searches from its owner's parent while
                        // retaining the actual receiver for successful invocation.
                        // The failed lookup describes that search origin too.
                        let diagnostic_receiver = if method { initial_owner } else { receiver };
                        return Err(self.missing_member(diagnostic_receiver, &name, method)?);
                    }
                };
            }
        };
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
        let active_here = self.frames.iter().any(same_attempt);
        let suspended_attempt = self
            .suspended
            .iter()
            .flat_map(|context| &context.frames)
            .any(same_attempt);
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
        let direct_force_read = self
            .frames
            .last()
            .and_then(|frame| frame.deep_force.as_ref())
            .is_some_and(|run| run.reads(receiver, owner, &name, entry));
        if memoize
            && !direct_force_read
            && !callback_attempt
            && matches!(
                self.heap.get(receiver)?.cache.get(&key),
                Some(LazyState::Evaluating)
            )
        {
            return Err(RuntimeError::new(
                ErrorKind::Cycle,
                format!("cyclic forcing of {name}"),
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
        if active && !direct_force_read && !callback_attempt {
            return Err(RuntimeError::new(
                ErrorKind::Cycle,
                format!("cyclic forcing of {name}"),
            ));
        }
        let mut collection_casts = Vec::new();
        let mut constant_value = None;
        if entry && !local && !method && !predicate {
            let mut current = receiver;
            let mut cached = None;
            loop {
                let object = self.heap.get(current)?;
                if current != receiver
                    && let Some(value) = self.cached_slot(current, &key)?
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
            collection_casts.dedup_by(|later, earlier| {
                later.source == earlier.source
                    && later.span == earlier.span
                    && later.ty.equivalent(&earlier.ty)
            });
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
        if let Some(value) = &constant_value {
            if collection_casts.is_empty() {
                if memoize {
                    self.complete_cache(receiver, key, value.clone())?;
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
        if !local && !method && member.ty.is_none() {
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
                            || inherited.name.clone().into(),
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
        if memoize {
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
        self.frames.push(Frame {
            guest_catch: None,
            block,
            pc,
            base,
            environment,
            lambda: false,
            type_run: None,
            type_resolution: None,
            generic_default: None,
            equality_run: None,
            deep_force: None,
            warning: None,
            method_call_site: None,
            pcf_render: None,
            collection_join: None,
            collection_transform: None,
            function_owner: None,
            amendment_arguments: None,
            amendment_end: None,
            prepared_hashes: None,
            key_search: None,
            return_to_type: false,
            diagnostic_member: None,
            callsite_hidden: false,
            builders: Vec::new(),
            generator_scopes: Vec::new(),
            module_load: None,
            uri_resolution: None,
            module_receiver: None,
            force: Some(Force {
                entry,
                receiver,
                owner,
                name,
                cache: memoize.then_some(key),
                method,
                inherited_types,
                collection_casts,
                skip_typecheck: !memoize && !method,
            }),
        });
        Ok(())
    }

    fn require_const(
        &self,
        initial: ObjectId,
        name: &str,
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
                member.name.as_ref() == name && member.local == local && member.method == method
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
                    format!("Cannot {verb} {kind} `{name}` from here because it is not `const`."),
                );
                error.hint = Some(format!(
                    "To fix, do either of:\n1. Add modifier `const` to {kind} `{name}`\n2. Self-import this module, and {reference} from the import."
                ).into());
                return Err(error);
            }
            if local {
                break;
            }
            owner = object.parent;
        }
        if method && name == "toString" {
            let mut error = RuntimeError::new(
                ErrorKind::Type,
                "Cannot call method `toString` from here because it is not `const`.",
            );
            error.hint = Some("To fix, do either of:\n1. Add modifier `const` to method `toString`\n2. Self-import this module, and reference this method from the import.".into());
            return Err(error);
        }
        Ok(())
    }

    /// Match lexical candidate visibility without evaluating candidate members.
    fn implicit_member_suggestions(
        &self,
        frame: &Frame,
        name: &str,
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
                    candidates.push(MemberSuggestion::Property(&member.name));
                } else if let Some((signature, arity)) = self
                    .program
                    .metadata
                    .get(member.code)
                    .and_then(|metadata| metadata.method_signature.as_ref())
                {
                    declared_methods.push(MemberSuggestion::Method {
                        name: &member.name,
                        signature,
                        arity: *arity,
                    });
                }
            }
            if object.is_module {
                candidates.push(MemberSuggestion::Property("output"));
            }
            // Each lexical owner's properties precede its declared methods; both
            // precede candidates from enclosing owners and the base module.
            candidates.extend(declared_methods);
        }
        candidates.extend(
            crate::diagnostic_names::BASE_PROPERTIES
                .iter()
                .map(|name| MemberSuggestion::Property(name)),
        );
        candidates.extend(
            crate::diagnostic_names::MODULE_METHODS
                .iter()
                .chain(crate::diagnostic_names::BASE_METHODS)
                .map(|(name, signature, arity)| MemberSuggestion::Method {
                    name,
                    signature,
                    arity: *arity,
                }),
        );
        let receiver = frame
            .environment
            .this_value
            .as_ref()
            .and_then(|value| value.as_object().ok())
            .or_else(|| frame.environment.scopes.first().map(|scope| scope.receiver));
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
                    candidates.push(MemberSuggestion::Property(&member.name));
                } else if let Some((signature, arity)) = self
                    .program
                    .metadata
                    .get(member.code)
                    .and_then(|metadata| metadata.method_signature.as_ref())
                {
                    receiver_methods.push(MemberSuggestion::Method {
                        name: &member.name,
                        signature,
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
        name: &str,
        method: bool,
    ) -> Result<RuntimeError, RuntimeError> {
        if self.heap.get(receiver)?.is_module {
            let module_name = self
                .heap
                .get(receiver)?
                .class_definition
                .as_ref()
                .map_or(self.program.module_name.as_ref(), |class| {
                    class.name.as_ref()
                });
            let kind = if method { "method" } else { "property" };
            let mut error = RuntimeError::new(
                ErrorKind::MissingMember,
                format!(
                    "Cannot find {kind} `{name}` in module `{}`.",
                    self.heap
                        .get(receiver)?
                        .class_definition
                        .as_ref()
                        .map_or(self.program.module_name.as_ref(), |class| class
                            .name
                            .as_ref())
                ),
            );
            {
                let mut properties = vec!["output".to_owned()];
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
                            properties.push(member.name.to_string());
                        } else if let Some((signature, arity)) = self
                            .program
                            .metadata
                            .get(member.code)
                            .and_then(|metadata| metadata.method_signature.as_ref())
                        {
                            methods.push((member.name.to_string(), signature.clone(), *arity));
                        }
                    }
                    current = object.parent;
                }
                methods.extend(crate::diagnostic_names::MODULE_METHODS.iter().map(
                    |(name, signature, arity)| {
                        ((*name).to_owned(), (*signature).to_owned(), *arity)
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
                                format!(
                                    "Available methods in module `{}`:\n{}",
                                    module_name,
                                    names.join("\n")
                                )
                                .into(),
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
                .map(|member| member.name.to_string())
                .collect::<Vec<_>>();
            if !method {
                names.push("output".into());
            }
            names.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
            names.dedup();
            error.hint = Some(
                format!(
                    "Available {} in module `{}`:\n{}",
                    if method { "methods" } else { "properties" },
                    module_name,
                    names.join("\n")
                )
                .into(),
            );
            return Ok(error);
        }
        let class = collections::typed_class(&self.heap, receiver)?;
        let object = self.heap.get(receiver)?;
        let dynamic = class.is_none()
            && object.collection_kind == crate::heap::CollectionKind::Dynamic
            && object.reference.is_none()
            && !object.ref_module;
        if class.is_none() && !dynamic {
            return Ok(RuntimeError::new(
                ErrorKind::MissingMember,
                format!("missing member {name}"),
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
                        methods.push((member.name.to_string(), signature.0, signature.1));
                    }
                } else {
                    properties.push(member.name.to_string());
                }
            }
            method_groups.push(methods);
            // A failed superclass resolution has no Typed/Any ancestry yet.
            if object.class_definition.is_some()
                && object.class_superclass_initialized
                && object.parent.is_none()
            {
                method_groups.push(
                    crate::diagnostic_names::TYPED_METHODS
                        .iter()
                        .map(|(name, signature, arity)| {
                            ((*name).to_owned(), (*signature).to_owned(), *arity)
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
                        ((*name).to_owned(), (*signature).to_owned(), *arity)
                    })
                    .collect(),
            );
        }
        // VmClass.getAllMethods merges ancestors first; overriding a method
        // replaces its definition without moving its insertion position.
        let mut methods: Vec<(String, String, usize)> = Vec::new();
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
            || "Dynamic".to_owned(),
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
                format!("Cannot find method `{name}` in class `{class_name}`.")
            } else {
                format!("Cannot find property `{name}` in object of type `{class_name}`.")
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
                    format!(
                        "Available methods in class `{class_name}`:\n{}",
                        names.join("\n")
                    )
                    .into()
                } else {
                    format!(
                        "Available properties:\n{}",
                        if names.is_empty() {
                            "(none)".to_owned()
                        } else {
                            names.join("\n")
                        }
                    )
                    .into()
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
                    format!(
                        "Method `{}` must be const because it overrides a const method.",
                        member.name
                    ),
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
            parent_class.name.to_string()
        } else if parent_class.id == reference::ACCESS_ID {
            "pkl.ref#Access".to_owned()
        } else {
            format!(
                "{}#{}",
                parent_class
                    .module
                    .as_deref()
                    .unwrap_or(&self.program.module_name),
                parent_class.name
            )
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
                error.hint = Some(format!(
                    "Property `{}` must be declared {modifier}, because it overrides a {modifier} property on parent class `{parent_name}`.",
                    member.name
                ).into());
                error.span = metadata.and_then(|m| m.declaration);
                error
            } else {
                let mut error = RuntimeError::new(
                    ErrorKind::Type,
                    format!(
                        "Modifier `{modifier}` cannot be applied to property `{}`.",
                        member.name
                    ),
                );
                error.hint = Some(format!(
                    "Property `{}` cannot be declared {modifier}, because it overrides a non-{modifier} property on parent class `{parent_name}`.",
                    member.name
                ).into());
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
                    format!(
                        "Method `{}` must be const because it overrides a const method.",
                        member.name
                    ),
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
                format!("Cannot {action} {modifier} property `{}`.", member.name),
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
                    .map(|member| member.name.to_string()),
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
                format!("{}#{class_name}", self.program.module_name)
            };
            for member in members.iter().filter(|member| !member.local) {
                if !(declared.contains(&(member.name.clone(), member.method))
                    || class_initializing && !member.method)
                {
                    let mut error = RuntimeError::new(
                        ErrorKind::MissingMember,
                        if let Some(module_name) = &module_name {
                            format!(
                                "Cannot find property `{}` in module `{module_name}`.",
                                member.name
                            )
                        } else {
                            format!(
                                "Cannot find property `{}` in object of type `{class_label}`.",
                                member.name
                            )
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
                            format!(
                                "Available properties in module `{module_name}`:\n{}",
                                names.join("\n")
                            )
                            .into()
                        } else {
                            format!(
                                "Available properties:\n{}",
                                if names.is_empty() {
                                    "(none)".into()
                                } else {
                                    names.join("\n")
                                }
                            )
                            .into()
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

    fn executable_root(&mut self, block: usize) -> ExecutableOwnerId {
        *self
            .executable_roots
            .entry(block)
            .or_insert_with(|| self.heap.allocate_executable())
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
            {
                error.frames.push(origin);
            } else if let Some(metadata) = self.program.metadata.get(function.block)
                && let Some(span) = metadata.declaration
            {
                error.frames.push(RuntimeFrame {
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
        self.frames.push(Frame {
            guest_catch: None,
            block: function.block,
            pc: 0,
            base: self.stack.len(),
            environment,
            force: None,
            lambda: !function.method,
            type_run: None,
            type_resolution: None,
            generic_default: None,
            equality_run: None,
            deep_force: None,
            warning: None,
            method_call_site: None,
            pcf_render: None,
            collection_join: None,
            collection_transform: None,
            function_owner: Some(id),
            amendment_arguments,
            amendment_end: None,
            prepared_hashes: None,
            key_search: None,
            return_to_type: false,
            diagnostic_member: None,
            callsite_hidden: false,
            builders: Vec::new(),
            generator_scopes: Vec::new(),
            module_load: None,
            uri_resolution: None,
            module_receiver: None,
        });
        Ok(())
    }

    fn step(&mut self) -> Result<(), RuntimeError> {
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
            .is_some_and(|frame| frame.deep_force.is_some())
        {
            return self.step_deep_force();
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
        let result = if matches!(instruction, Instruction::LoadBaseModule) {
            self.import_stdlib("pkl:base").map(|_| ())
        } else if matches!(instruction, Instruction::LoadRefModule) {
            self.import_stdlib("pkl:ref").map(|_| ())
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
                | Instruction::SuperSubscript
                | Instruction::EntryParent
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
                | Instruction::ResolvedClass(_)
                | Instruction::ResolvedDefault { .. }
                | Instruction::ResolveClassType { .. }
                | Instruction::BindLocal { .. }
                | Instruction::CheckType(_)
        ) {
            self.type_instruction(instruction)
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

    /// Keep the large general dispatch frame out of collection and continuation
    /// calls; their independent bounded stacks are exercised at 256 KiB.
    fn step_instruction(&mut self, instruction: Instruction) -> Result<(), RuntimeError> {
        let frame = self.frames.last_mut().expect("active frame");
        match instruction {
            Instruction::ValidateTypeArguments { .. }
            | Instruction::BuiltinClass(_)
            | Instruction::TestType(_)
            | Instruction::ResolveType(_)
            | Instruction::ResolvedClass(_)
            | Instruction::ResolvedDefault { .. }
            | Instruction::ResolveClassType { .. }
            | Instruction::BindLocal { .. }
            | Instruction::CheckType(_) => {
                unreachable!("type instruction uses its bounded dispatcher")
            }
            Instruction::ImportModule(spec) => self.begin_import(spec)?,
            Instruction::PrepareModuleImports(unit) => self.prepare_module_imports(unit)?,
            Instruction::LoadPrimary(source) => self.begin_primary(source)?,
            Instruction::EvaluateQuery(expression) => self.begin_query(expression)?,
            Instruction::InitializeModuleParent => self.initialize_module_parent()?,
            Instruction::CheckModuleOutput => self.check_module_output()?,
            Instruction::BootstrapStdlib => self.bootstrap_stdlib()?,
            Instruction::ReadModuleVersion => self.read_module_version()?,
            Instruction::CheckModuleVersion => self.check_module_version()?,
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
            Instruction::ResourceProperty(property) => self.resource_property(property)?,
            Instruction::OutputRenderer => self.output_renderer()?,
            Instruction::RenderPcf { document } => {
                self.begin_render(document, rendering::Format::Pcf)?
            }
            Instruction::RenderJson { document } => {
                self.begin_render(document, rendering::Format::Json)?
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
                let Some(Value::Class(id)) = self.stack.last() else {
                    return Err(RuntimeError::new(
                        ErrorKind::Type,
                        "Expected an annotation class.",
                    ));
                };
                let mut current = Some(*id);
                let mut valid = false;
                let mut visited = BTreeSet::new();
                while let Some(id) = current {
                    if !visited.insert(id) {
                        break;
                    }
                    let object = self.heap.get(id)?;
                    if object
                        .class_definition
                        .as_ref()
                        .is_some_and(|class| class.id == annotations::ANNOTATION_ID)
                    {
                        valid = true;
                        break;
                    }
                    current = object.parent;
                }
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
                let owner = self
                    .stack
                    .last()
                    .ok_or_else(|| invalid("missing annotation owner"))?
                    .heap_id()
                    .ok_or_else(|| invalid("annotation owner is not a declaration"))?;
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
                    let name = Arc::from(format!("{}{}", self.current_module_name(), name));
                    let id = self.heap.allocate(Object {
                        type_alias: Some(name),
                        type_alias_declaration: Some(declaration),
                        environment,
                        ..Object::default()
                    });
                    self.alias_singletons.insert(declaration, id);
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
            Instruction::LoadBaseModule | Instruction::LoadRefModule => {
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
                self.retire_saturated_method_cache(&Value::Object(domain), "renderReference")?;
                self.force(
                    domain,
                    domain,
                    Arc::from("renderReference"),
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
            | Instruction::SuperSubscript
            | Instruction::EntryParent
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
                    RuntimeError::new(ErrorKind::MissingMember, "No outer scope is available.")
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
                self.observe_native_literal(span, rendered)
            }
            Instruction::Observe(span) => {
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
                return Err(RuntimeError::new(ErrorKind::Type, message));
            }
            Instruction::StringRepeat => self.repeat_string()?,
            Instruction::CollectionJoin => self.begin_collection_join()?,
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
            Instruction::AppendStrictElement => self.append_strict_element()?,
            Instruction::EndCollectionTransform => self.end_collection_transform()?,
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
            Instruction::ToString => {
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
            Instruction::GetMember(ref name)
            | Instruction::GetMethod(ref name)
            | Instruction::GetConstMember(ref name)
            | Instruction::GetConstMethod(ref name) => {
                let method = matches!(
                    instruction,
                    Instruction::GetMethod(_) | Instruction::GetConstMethod(_)
                );
                let needs_const = matches!(
                    instruction,
                    Instruction::GetConstMember(_) | Instruction::GetConstMethod(_)
                );
                let receiver = self.pop()?;
                if method {
                    self.retire_saturated_method_cache(&receiver, name)?;
                }
                if self.primitive_member(&receiver, name, method)? {
                    return Ok(());
                }
                if method
                    && name.as_ref() == "encodeToBytes"
                    && matches!(receiver, Value::String(_))
                {
                    self.string_encode_method(receiver);
                    return Ok(());
                }
                if method
                    && name.as_ref() == "join"
                    && (self.is_list(&receiver) || self.is_set(&receiver))
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
                if method && name.as_ref() == "repeat" && matches!(receiver, Value::String(_)) {
                    self.string_repeat_method(receiver);
                } else if !method
                    && let Value::String(text) = &receiver
                    && matches!(name.as_ref(), "length" | "isEmpty")
                {
                    self.stack.push(if name.as_ref() == "length" {
                        Value::Int((text.code_point_len() as i64).into())
                    } else {
                        Value::Bool(text.is_empty())
                    });
                } else if method && name.as_ref() == "getClass" {
                    let function = self.native_reference_method(receiver, "getClass");
                    self.stack.push(function);
                } else if matches!(receiver, Value::Function(_))
                    && method
                    && name.as_ref() == "apply"
                {
                    self.stack.push(receiver);
                } else if method
                    && name.as_ref() == "toString"
                    && (receiver.as_object().is_err()
                        || self.is_map(&receiver)
                        || self.is_set(&receiver)
                        || self.is_list(&receiver))
                {
                    let function = self.native_reference_method(receiver, "scalarToString");
                    self.stack.push(function);
                } else if matches!(receiver, Value::Function(_)) {
                    return Err(self.function_member_error(&receiver, name, method)?);
                } else {
                    let receiver = receiver.as_object()?;
                    if method
                        && self.heap.get(receiver)?.collection_kind
                            == crate::heap::CollectionKind::Set
                        && name.as_ref() == "contains"
                    {
                        let function = self.native_reference_method(Value::Object(receiver), name);
                        self.stack.push(function);
                        return Ok(());
                    }
                    if method
                        && self.heap.get(receiver)?.collection_kind
                            == crate::heap::CollectionKind::Map
                        && matches!(name.as_ref(), "containsKey" | "getOrNull")
                    {
                        let function = self.native_reference_method(Value::Object(receiver), name);
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
                if self.primitive_member(&receiver, name, method)? {
                    return Ok(());
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
                        frame.generic_default =
                            Some(generics::GenericDefaultRun::inferred_parent(ty, origin));
                    } else {
                        self.stack
                            .push(Value::Object(self.heap.allocate(Object::default())));
                    }
                } else {
                    self.stack
                        .push(Value::Object(self.heap.allocate(Object::default())));
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
            Instruction::Fail(error) => return Err(error),

            Instruction::Undefined(name) => {
                return Err(RuntimeError::new(
                    ErrorKind::MissingMember,
                    format!("Tried to read property `{name}` but its value is undefined."),
                )
                .undefined());
            }
            Instruction::AbstractMember { name, method } => {
                let kind = if method { "method" } else { "property" };
                return Err(RuntimeError::new(
                    ErrorKind::Type,
                    PklString::from(format!(
                        "Cannot invoke abstract {kind} `{}",
                        self.current_module_name()
                    ))
                    .concat(&name)
                    .concat(&"`.".into()),
                ));
            }
            Instruction::Unsupported(message) => {
                return Err(RuntimeError::new(ErrorKind::Unsupported, message.as_ref()));
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
                    let id = self.heap.allocate(Object {
                        class_definition: Some(Class {
                            module: class
                                .module
                                .or_else(|| Some(self.current_module_name().into())),
                            ..class
                        }),
                        class_initializing: true,
                        environment,
                        ..Object::default()
                    });
                    self.class_singletons.insert(template, id);
                    self.stack.push(Value::Class(id));
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
                let parent_class = self
                    .heap
                    .get(parent)?
                    .class_definition
                    .as_ref()
                    .ok_or_else(|| invalid("invalid class handle"))?;
                if id == parent {
                    return Err(RuntimeError::new(
                        ErrorKind::Type,
                        format!(
                            "Class `{}#{}` cannot extend itself.",
                            parent_class
                                .module
                                .as_deref()
                                .unwrap_or(&self.program.module_name),
                            parent_class.name
                        ),
                    ));
                }
                if !parent_class.open && !parent_class.abstract_ {
                    let mut error = RuntimeError::new(
                        ErrorKind::Type,
                        format!(
                            "Cannot extend non-open class `{}`.",
                            reference::type_name(
                                &TypeCheck::Class {
                                    module: parent_class.module.clone(),
                                    id: parent_class.id,
                                    name: parent_class.name.clone()
                                },
                                &self.program.module_name
                            )
                        ),
                    );
                    error.hint = Some("By default, classes are closed for extension.\nTo make a class extensible, add an `open` modifier: `open class MyClass { ... }`".into());
                    return Err(error);
                }
                let object = self.heap.get_mut(id)?;
                object.parent = Some(parent);
                object.class_superclass_initialized = true;
            }
            Instruction::SetDefaultClassParent => {
                let Some(Value::Class(id)) = self.stack.last().cloned() else {
                    return Err(invalid("missing class initialization target"));
                };
                // Typed/Any are implicit in the bounded heap representation.
                self.heap.get_mut(id)?.class_superclass_initialized = true;
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
                if let Some(parent) = self.heap.get(id)?.parent {
                    self.validate_class_override(parent, &member)?;
                }
                self.heap.get_mut(id)?.members.push(member);
            }
            Instruction::FinishClass => {
                let Some(Value::Class(id)) = self.stack.last().cloned() else {
                    return Err(invalid("missing class initialization target"));
                };
                self.heap.get_mut(id)?.class_initializing = false;
            }
            Instruction::MakeInstance { template }
            | Instruction::MakeBuiltinInstance { template } => {
                let builtin = matches!(instruction, Instruction::MakeBuiltinInstance { .. });
                let Value::Class(parent) = self.pop()? else {
                    return Err(RuntimeError::new(ErrorKind::Type, "Expected a class."));
                };
                if let Some(ty) = &self.heap.get(parent)?.builtin_type
                    && !matches!(
                        ty.resolved(),
                        TypeCheck::RefDomain
                            | TypeCheck::Class {
                                id: reference::ACCESS_ID,
                                ..
                            }
                    )
                    && !matches!(ty.resolved(), TypeCheck::Class { id, .. } if builtins::name(*id).is_some())
                {
                    let name = reference::type_name(ty, &self.program.module_name);
                    let name = if matches!(ty.resolved(), TypeCheck::Reference { .. }) {
                        "pkl.ref#Reference"
                    } else {
                        &name
                    };
                    let mut error = RuntimeError::new(
                        ErrorKind::Type,
                        format!(
                            "Cannot instantiate, or amend an instance of, external class `{name}`."
                        ),
                    );
                    error.hint=Some("To create an instance of an external class, use its literal syntax or constructor method.\nExamples:\n* `123` uses literal syntax to create an instance of class `Int`.\n* `Pair(1, 2)` uses the `Pair()` constructor method to create an instance of class `Pair`.".into());
                    return Err(error);
                }
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
                    members,
                    environment,
                    ..Object::default()
                });
                self.stack.push(Value::Object(id));
            }
            Instruction::DefaultClassInstance => {
                let Value::Class(id) = self.pop()? else {
                    return Err(RuntimeError::new(ErrorKind::Type, "Expected a class."));
                };
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
                    self.builtin_class(builtins::ty(output::MODULE)).heap_id()
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
                                .is_some_and(|identity| identity.original_uri.as_ref() == "pkl:ref")
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
                    members,
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
                if let Some(unit) = &module {
                    self.frames.last_mut().unwrap().module_receiver = Some(object);
                    if let Some(identity) = &unit.identity {
                        if unit.is_singleton() {
                            self.modules_stdlib.insert(
                                identity.original_uri.clone(),
                                modules::CachedModule::Value(object),
                            );
                        } else {
                            self.modules_original.insert(
                                identity.original_uri.clone(),
                                modules::CachedModule::Value(object),
                            );
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
                let id = self.heap.allocate(Object {
                    function: Some(Function {
                        block,
                        arity,
                        method,
                        captured_this,
                    }),
                    method_declaration,
                    environment,
                    ..Object::default()
                });
                self.stack.push(Value::Function(id));
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
                                )?)
                                .into(),
                                "` and `".into(),
                                (crate::diagnostic_values::type_name(
                                    &right,
                                    &self.heap,
                                    &self.program.module_name,
                                )?)
                                .into(),
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
                    left => return Err(binary_error("+", &left, &Value::Int(right.into()))),
                };
                self.stack.push(value);
            }
            Instruction::Equal | Instruction::NotEqual => {
                let right = self.pop()?;
                let left = self.pop()?;
                if matches!((&left, &right), (Value::Object(_), Value::Object(_))) {
                    self.begin_equality(left, right, matches!(instruction, Instruction::NotEqual));
                } else {
                    self.stack.push(binary(&instruction, left, right)?);
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
                let right = self.pop()?;
                let left = self.pop()?;
                self.stack.push(binary(&instruction, left, right)?);
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
            Instruction::ReadResource | Instruction::ReadResourceOrNull => {
                let nullable = matches!(instruction, Instruction::ReadResourceOrNull);
                if self.callback_closing {
                    return Err(RuntimeError::new(ErrorKind::Closed, "evaluator is closing"));
                }
                let uri = match self.pop()? {
                    Value::String(uri) => uri,
                    operand => {
                        let operator = if nullable { "read?" } else { "read" };
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
                                    )?)
                                    .into(),
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
                let Some(uri) = uri.as_str() else {
                    return Err(RuntimeError::new(
                        ErrorKind::Unsupported,
                        "Resource URIs containing unmatched UTF-16 surrogates are not implemented.",
                    ));
                };
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
                let uri = Arc::from(uri);
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
                                (None, _) => unit.id == crate::modules::UnitId(0),
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
                let base = self.frames.last().expect("active frame").base;
                if self.stack.len() != base + 1 {
                    return Err(invalid("unbalanced return stack"));
                }
                let value = self.pop()?;
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
                    let node = if typecheck::needs_custom_this(&ty) {
                        let executable = if let Some(parent) =
                            self.heap.get(owner)?.environment.executable_owner
                        {
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
                        source,
                        span,
                        member,
                    }];
                    let frame = self.frames.last().expect("active frame");
                    if let Some(metadata) = self.program.metadata.get(frame.block)
                        && let Some(span) = metadata.value_span
                    {
                        frames.push(RuntimeFrame {
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
                    let cast = force.collection_casts.remove(0);
                    let mut frames = vec![RuntimeFrame {
                        source: cast.source,
                        span: cast.span,
                        member: cast.member,
                    }];
                    let frame = self.frames.last().expect("active frame");
                    if let Some(metadata) = self.program.metadata.get(frame.block)
                        && let Some(span) = metadata.value_span
                    {
                        frames.push(RuntimeFrame {
                            source: metadata.source.clone(),
                            span,
                            member: metadata.member.clone(),
                        });
                    }
                    self.begin_type_node(
                        cast.ty,
                        value,
                        TypeDestination::Return,
                        frames,
                        cast.node,
                    )?;
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
                    && let Some(key) = force.cache.take()
                {
                    let receiver = force.receiver;
                    self.complete_cache(receiver, key, value.clone())?;
                    if self.frames.len() > 1 {
                        let caller = self.frames.len() - 2;
                        self.frames[caller].callsite_hidden = true;
                    }
                    self.stack.push(value);
                    self.frames.last_mut().expect("active frame").pc -= 1;
                    return Ok(());
                }
                let frame = self.frames.pop().expect("active frame");
                if frame.return_to_type {
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
            }
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

fn invalid(message: &str) -> RuntimeError {
    RuntimeError::new(ErrorKind::InvalidBytecode, message)
}

fn binary(instruction: &Instruction, left: Value, right: Value) -> Result<Value, RuntimeError> {
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
                    ));
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
        (left, right) => Err(binary_error(operator_name(instruction), &left, &right)),
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
    if resolved.preparation_is_evaluation
        && resolved.preparation.iter().any(|step| {
            matches!(
                step,
                crate::resolver::AstPreparation::Import { clause: true, .. }
            )
        })
    {
        let mut shell = resolved.clone();
        shell.module.members.clear();
        shell.module.deferred_diagnostics.clear();
        if let Some(declaration) = &mut shell.module.declaration {
            declaration.annotations.clear();
            declaration.parent = None;
            declaration.modifiers.clear();
        }
        shell.preparation.retain(|step| {
            matches!(
                step,
                crate::resolver::AstPreparation::Import { clause: true, .. }
            )
        });
        shell.preparation_failure = None;
        let unit = compile_unit_body(program, &shell, options, instrument, source, identity)?;
        program.units[unit.0 as usize].build = Some(modules::ModuleBuild {
            module: resolved.module.clone(),
            options,
            instrument,
            prepared: BTreeMap::new(),
        });
        return Ok(unit);
    }
    if resolved.preparation_failure.is_none() {
        return compile_unit_body(program, resolved, options, instrument, source, identity);
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
    // Raw development blocks may omit provenance, but their logical IDs
    // still share the same address space as compiler-created metadata.
    program.align_metadata()?;
    let unit = replacement.unwrap_or(crate::modules::UnitId(
        program
            .units
            .next_id()
            .try_into()
            .map_err(|_| invalid("module unit limit"))?,
    ));
    let declaration_base = program.next_declaration;
    program.next_declaration = declaration_base
        .checked_add(resolved.module.source.len() + 2)
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
        argument_spans: BTreeMap::new(),
        function_counts: BTreeMap::new(),
        entry_counts: BTreeMap::new(),
        function_names: BTreeMap::new(),
        instrument,
    };
    let module_name: Arc<str> = Arc::from(
        resolved
            .module
            .declaration
            .as_ref()
            .and_then(|declaration| declaration.name.as_ref())
            .map(|parts| parts.join("."))
            .unwrap_or_else(|| {
                compiler
                    .source
                    .as_ref()
                    .map_or_else(String::new, |source| source.module_name.to_string())
            }),
    );
    let template = compiler.object_template(&resolved.module.members, false, false)?;
    if unit.0 == 0 {
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
    if unit.0 == 0 {
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
            modules::ImportSpec::authored(parent.uri.clone().into(), parent.span, unit)
        } else {
            modules::ImportSpec::new(
                parent.uri.clone().into(),
                parent.span,
                unit,
                compiler.identity.as_ref(),
            )?
        };
        entry.push_at(Instruction::ImportModule(spec), parent.span);
        entry.push_at(Instruction::InitializeModuleParent, parent.span);
    }
    compiler.type_scopes.push(resolved.module.members.clone());
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
            compiler.initialize_member_types(std::slice::from_ref(member), &mut entry)?;
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
    if unit.0 == 0 {
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
        key: None,
        delegate_key: None,
        build: None,
        syntax: Some(Arc::new(resolved.module.clone())),
        source: compiler.source,
        name: module_name,
        template,
        entry,
        initial_members: initial_module_members,
        import_order: uri_resolution::module_import_order(resolved),
        mode,
        modifiers,
        class_id: if unit.0 == 0 {
            reference::MODULE_ID
        } else {
            declaration_base + resolved.module.source.len() + 1
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

struct Compiler<'a> {
    resolved: &'a crate::resolver::ResolvedModule,
    program: &'a mut Program,
    unit: crate::modules::UnitId,
    declaration_base: usize,
    source: Option<Arc<crate::value::RuntimeSource>>,
    identity: Option<crate::modules::ModuleIdentity>,
    options: CompileOptions,
    member: PklString,
    module_annotation: bool,
    let_members: Vec<PklString>,
    type_scopes: Vec<Vec<crate::syntax::Member>>,
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
    active_type_parameters: BTreeMap<String, TypeCheck>,
    /// Alias cloning visits inline/lambda nodes, but not lazy member call targets.
    constructor_parameters_bound: bool,
    parameter_spans: BTreeMap<String, Span>,
    /// Constructor parent nodes retain the authored actual, including alias syntax.
    argument_spans: BTreeMap<String, Span>,
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
                        ExprKind::Name(name) if matches!(name.as_str(), "List" | "Set" | "Map")
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
        let suffix = &self.resolved.module.source[header_end..expression.span.end];
        if suffix.is_empty() {
            return expression.span;
        }
        let skip = suffix
            .chars()
            .take_while(|&ch| {
                (entry && ch == ']') || ch == '=' || crate::diagnostic::java_whitespace(ch)
            })
            .map(char::len_utf8)
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
        self.type_scopes.push(members.to_vec());
        let mut result = Vec::new();
        for member in members {
            let scope_count = self.type_scopes.len();
            let previous_member = self.member.clone();
            let previous_modifiers = self.modifiers.clone();
            match self.object_member(member) {
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
                    self.member = previous_member.concat(&format!(".{}", member.name).into());
                    let mut code = Code::default();
                    code.push_at(
                        Instruction::Fail(error.clone()),
                        error.span.unwrap_or(member.span),
                    );
                    let block = self.block(code, Some(member.name_span))?;
                    self.program.metadata[block].initialization_error = Some(error);
                    result.push(Member {
                        flags: Member::declaration_flags(member),
                        entry: false,
                        name: Arc::from(member.name.as_str()),
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

    fn object_member(&mut self, member: &crate::syntax::Member) -> Result<Member, RuntimeError> {
        let previous = std::mem::replace(&mut self.constructor_parameters_bound, false);
        let result = self.object_member_body(member);
        self.constructor_parameters_bound = previous;
        result
    }

    fn object_member_body(
        &mut self,
        member: &crate::syntax::Member,
    ) -> Result<Member, RuntimeError> {
        let previous_modifiers = std::mem::replace(&mut self.modifiers, member.modifiers.clone());
        self.validate_modifiers(member)?;
        let previous_member = self.member.clone();
        self.member = if previous_member.is_empty() {
            format!(
                "{}{}",
                if self.module_annotation { "." } else { "#" },
                member.name
            )
            .into()
        } else {
            previous_member.concat(&format!(".{}", member.name).into())
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
        if let Some(error) = &initialization_error {
            // Annotation resolution runs when the owner is initialized. Keep a
            // stub body so compilation cannot bypass earlier type dependencies.
            instructions.push_at(
                Instruction::Fail(error.clone()),
                error.span.unwrap_or(member.span),
            );
        } else if matches!(member.expr.kind, crate::syntax::ExprKind::Uninitialized) {
            self.default_property(member, &mut instructions)?;
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
        self.program.metadata[code].fixed_span = member.override_fixed_span;
        self.program.metadata[code].const_span = member.override_const_span;
        if let crate::syntax::ExprKind::Function {
            parameters,
            method: true,
            ..
        } = &member.expr.kind
        {
            self.program.metadata[code].method_signature = Some((
                format!(
                    "{}({})",
                    member.name,
                    parameters
                        .iter()
                        .map(|p| p.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                parameters.len(),
            ));
        }
        self.member = previous_member;
        self.modifiers = previous_modifiers;
        Ok(Member {
            flags: Member::declaration_flags(member),
            entry: false,
            name: Arc::from(member.name.as_str()),
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

    fn inner_expression(
        &mut self,
        expression: &crate::syntax::Expr,
        output: &mut Code,
    ) -> Result<(), RuntimeError> {
        use crate::resolver::Binding;
        use crate::syntax::ExprKind;
        match &expression.kind {
            ExprKind::Import { uri, uri_span, .. } => {
                let spec = if self.resolved.preparation_is_evaluation {
                    modules::ImportSpec::authored(uri.clone().into(), *uri_span, self.unit)
                } else {
                    modules::ImportSpec::new(
                        uri.clone().into(),
                        *uri_span,
                        self.unit,
                        self.identity.as_ref(),
                    )?
                };
                output.push(Instruction::ImportModule(spec));
            }
            ExprKind::Class {
                type_parameters,
                superclass,
                members,
                ..
            } => {
                if !type_parameters.is_empty() {
                    return Err(unsupported_type("generic classes"));
                }
                let class = Class {
                    module: self
                        .source
                        .as_ref()
                        .map(|source| source.module_name.clone()),
                    id: self.declaration_base + expression.span.start,
                    name: Arc::from(
                        self.member
                            .as_str()
                            .expect("classes are declared in module scope")
                            .trim_start_matches('#'),
                    ),
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
                    if let Err(error) = self.class_expression(
                        parent,
                        output,
                        self.type_scopes.len(),
                        &mut BTreeSet::new(),
                    ) {
                        output.push_at(Instruction::Fail(error.at(parent.span)), parent.span);
                    } else {
                        output.push_at(Instruction::SetClassParent, parent.span);
                    }
                } else {
                    output.push(Instruction::SetDefaultClassParent);
                }
                self.declaration_annotations(expression, output)?;
                self.type_scopes.push(members.to_vec());
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
                            if let ExprKind::Function {
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
                        self.initialize_member_types(std::slice::from_ref(member), output)?;
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
            }
            ExprKind::TypeAlias {
                type_parameters,
                ty,
                ..
            } => {
                let begin = output.len();
                let declaration_name = self
                    .resolved
                    .module
                    .members
                    .iter()
                    .find(|member| member.expr.span == expression.span)
                    .map(|member| {
                        format!(
                            "#{}",
                            &self.resolved.module.source
                                [member.name_span.start..member.name_span.end]
                        )
                    })
                    .unwrap_or_else(|| {
                        self.member
                            .as_str()
                            .expect("type aliases are declared in module scope")
                            .to_owned()
                    });
                output.push(Instruction::BeginTypeAlias {
                    declaration: self.declaration_base + expression.span.start,
                    name: Arc::from(declaration_name),
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
                                name: parameter.name.clone().into(),
                            },
                        )
                    })
                    .collect();
                let previous_module = std::mem::replace(&mut self.constraint_module, true);
                let alias_body = self.compile_type_at(
                    ty,
                    self.type_scopes.len(),
                    &mut BTreeSet::new(),
                    &formals,
                );
                self.constraint_module = previous_module;
                if let Ok(body) = alias_body {
                    self.program.alias_definitions.insert(
                        declaration,
                        type_resolution::AliasDefinition {
                            parameters: type_parameters
                                .iter()
                                .map(|parameter| Arc::from(parameter.name.as_str()))
                                .collect(),
                            body,
                            origin: RuntimeFrame {
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
                    Ok(_) => {
                        self.initialize_type_resolution_with_parameters(ty, &parameters, output)
                    }
                    Err(error) => {
                        self.initialize_type_resolution_with_parameters(ty, &parameters, output);
                        let span = error.span.unwrap_or(ty.span);
                        output.push_at(Instruction::Fail(error), span);
                    }
                }
                output.push(Instruction::FinishTypeAlias);
                let end = output.len();
                if let Instruction::BeginTypeAlias { end: target, .. } = &mut output[begin] {
                    *target = end;
                }
            }
            ExprKind::EntryParent => output.push(Instruction::EntryParent),
            ExprKind::Uninitialized => output.push(Instruction::AbstractMember {
                name: self.member.clone(),
                method: true,
            }),
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
                if self.resolved.const_names.contains(&expression.span) {
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
                        name: Arc::from(name.as_str()),
                        owner_depth,
                        local,
                        method,
                    });
                }
                let name: Arc<str> = Arc::from(name.as_str());
                match self
                    .resolved
                    .names
                    .get(&expression.span)
                    .ok_or_else(|| invalid("missing static binding"))?
                {
                    Binding::BaseTypeAlias => output.push(Instruction::BuiltinTypeAlias(name)),
                    Binding::BaseClass => {
                        let ty = reference::base_type(&name)
                            .or_else(|| {
                                builtins::id(&name).map(|id| TypeCheck::Class {
                                    module: None,
                                    id,
                                    name: name.clone(),
                                })
                            })
                            .ok_or_else(|| unsupported_type("builtin class value"))?;
                        output.push(Instruction::BuiltinClass(ty));
                    }
                    Binding::BaseConstant => output.push(Instruction::Constant(Value::Float(
                        if name.as_ref() == "NaN" {
                            f64::NAN.into()
                        } else {
                            f64::INFINITY.into()
                        },
                    ))),
                    Binding::Lexical { depth, slot: 0 } => {
                        output.push(Instruction::LoadLocal(*depth))
                    }
                    Binding::Lexical { .. } => return Err(invalid("unsupported lexical slot")),
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
                    Binding::ImplicitMethod if name.as_ref() == "Pair" => {
                        output.push(Instruction::PairConstructor)
                    }
                    Binding::ImplicitMethod if name.as_ref() == "Null" => {
                        output.push(Instruction::NullConstructor)
                    }
                    Binding::ImplicitMethod => output.push(Instruction::GetImplicitMethod(name)),
                    Binding::ImplicitThis => output.push(Instruction::GetImplicitProperty(name)),
                    Binding::CustomThis => {
                        output.push(Instruction::LoadCustomThis);
                        output.push(Instruction::GetMember(name));
                    }
                    Binding::CustomMethod => {
                        output.push(Instruction::LoadCustomThis);
                        output.push(Instruction::GetMethod(name));
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

fn scalar_string(value: &Value) -> Result<PklString, RuntimeError> {
    Ok(match value {
        Value::Null | Value::NullWithDefault(_) => "null".into(),
        Value::Bool(value) => value.to_string().into(),
        Value::Int(value) => value.to_string().into(),
        Value::Float(value) => crate::numeric::double_to_string(**value).into(),
        Value::Duration(value) => value.to_string().into(),
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
    module: &str,
    heap: &Heap,
) -> Result<RuntimeError, RuntimeError> {
    type_error_at_width(expected, actual, module, heap, 80)
}
fn type_error_at_width(
    expected: &TypeCheck,
    actual: &Value,
    module: &str,
    heap: &Heap,
    limit: usize,
) -> Result<RuntimeError, RuntimeError> {
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
        crate::diagnostic_values::type_name(actual, heap, module)?.into()
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
        type_name_value(expected, module)
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

fn type_name(ty: &TypeCheck, module: &str) -> String {
    type_name_value(ty, module).to_java_utf8()
}

fn type_name_value(ty: &TypeCheck, module: &str) -> PklString {
    enum Part<'a> {
        Type(&'a TypeCheck),
        Text(String),
    }
    let mut pending = vec![Part::Type(ty)];
    let mut result = crate::pkl_string::PklStringBuilder::default();
    while let Some(part) = pending.pop() {
        let ty = match part {
            Part::Text(text) => {
                result.push_str(&text);
                continue;
            }
            Part::Type(ty) => ty.resolved(),
        };
        let text = match ty {
            TypeCheck::Unknown => "unknown".into(),
            TypeCheck::MetaClass(name) => name.to_string(),
            TypeCheck::Function { parameters, .. } => parameters
                .as_ref()
                .map_or_else(|| "Function".into(), |p| format!("Function{}", p.len())),
            TypeCheck::NamedAlias { expanded, .. }
            | TypeCheck::Alias(expanded)
            | TypeCheck::GenericAlias(expanded)
            | TypeCheck::Constrained { base: expanded, .. } => {
                pending.push(Part::Type(expanded));
                continue;
            }
            TypeCheck::IntegerAlias { name, .. } => name.to_string(),
            TypeCheck::RefDomain => "pkl.ref#Domain".into(),
            TypeCheck::Reference { .. } => "pkl.ref#Reference".into(),
            TypeCheck::Class {
                id,
                name,
                module: declaring_module,
            } => {
                if builtins::name(*id).is_some() {
                    name.to_string()
                } else {
                    format!("{}#{name}", declaring_module.as_deref().unwrap_or(module))
                }
            }
            TypeCheck::Collection(kind) | TypeCheck::ParameterizedCollection { kind, .. } => {
                format!("{kind:?}")
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
            _ => format!("{ty:?}"),
        };
        result.push_str(&text);
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
        Value::String(_) => "String",
        Value::Bytes(_) => "Bytes",
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

fn binary_error(operator: &str, left: &Value, right: &Value) -> RuntimeError {
    RuntimeError::new(ErrorKind::Type, PklString::join(&[
        format!("Operator `{operator}` is not defined for operand types `{}` and `{}`.\nLeft operand : ", value_type_name(left), value_type_name(right)).into(),
        display_value(left), "\nRight operand: ".into(), display_value(right),
    ], ""))
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
            "Domain" if arguments.is_empty() => TypeCheck::RefDomain,
            "Access" if arguments.is_empty() => TypeCheck::Class {
                module: None,
                id: reference::ACCESS_ID,
                name: Arc::from("Access"),
            },
            "Reference" if arguments.is_empty() => TypeCheck::Reference {
                domain: Box::new(TypeCheck::Unknown),
                referent: Box::new(TypeCheck::Unknown),
            },
            "Reference" if arguments.len() == 2 => {
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
            "Domain" | "Reference" | "Access" => {
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

    fn find_type(&self, name: &str, _limit: usize) -> Option<(crate::syntax::Member, usize)> {
        // ResolveSimpleDeclaredTypeNode starts at the enclosing module. Ordinary
        // module properties can shadow types, but locals/parameters/class members cannot.
        self.resolved
            .module
            .members
            .iter()
            .find(|member| {
                member.name == name
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
        output: &mut Code,
    ) -> Result<(), RuntimeError> {
        for member in members {
            if let Some(ty) = &member.ty {
                self.initialize_type_resolution(ty, output);
            }
            if let crate::syntax::ExprKind::Function {
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
        );
    }

    fn initialize_type_resolution_with_parameters(
        &mut self,
        ty: &crate::syntax::Type,
        parameters: &BTreeMap<String, TypeCheck>,
        output: &mut Code,
    ) {
        if let Err(failure) = self.type_resolution_plan(ty, parameters, output) {
            let (error, span) = *failure;
            output.push_at(Instruction::Fail(error.at(span)), span);
        }
    }

    fn type_resolution_plan(
        &mut self,
        ty: &crate::syntax::Type,
        parameters: &BTreeMap<String, TypeCheck>,
        output: &mut Code,
    ) -> Result<(), Box<(RuntimeError, Span)>> {
        use crate::syntax::{ExprKind, TypeKind};
        let reference_start = self.program.type_recipes.next_id();
        let check =
            self.compile_type_at(ty, self.type_scopes.len(), &mut BTreeSet::new(), parameters);
        self.reference_usage_context(reference_start, ty);
        if let Ok(check) = check
            && check.has_declared()
        {
            // Resolve a parameterized qualified class before its arguments,
            // then execute the authored argument plans. A statically known
            // local class still needs initialization; materializing its
            // TypeCheck alone does not force that declaration.
            if let TypeKind::Named {
                path, arguments, ..
            } = &ty.kind
                && path.len() > 1
                && !arguments.is_empty()
            {
                let mut base = ty.clone();
                let TypeKind::Named {
                    arguments: placeholders,
                    ..
                } = &mut base.kind
                else {
                    unreachable!()
                };
                for argument in placeholders {
                    argument.kind = TypeKind::Unknown;
                }
                let base = self
                    .compile_type_at(
                        &base,
                        self.type_scopes.len(),
                        &mut BTreeSet::new(),
                        parameters,
                    )
                    .map_err(|error| Box::new((error, ty.span)))?;
                output.push_at(Instruction::ResolveType(base), ty.span);
                for argument in arguments {
                    self.type_resolution_plan(argument, parameters, output)?;
                }
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
                            name: Arc::from(member.name.as_str()),
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
                        self.type_resolution_plan(argument, parameters, output)?;
                    }
                }
                if alias {
                    output.push_at(Instruction::CheckTypeAliasInitialized, ty.span);
                }
            }
            TypeKind::Nullable(inner) | TypeKind::Constrained { base: inner, .. } => {
                self.type_resolution_plan(inner, parameters, output)?;
            }
            TypeKind::Union { types, .. } => {
                for ty in types {
                    self.type_resolution_plan(ty, parameters, output)?;
                }
            }
            TypeKind::Function {
                parameters: arguments,
                return_type,
            } => {
                for ty in arguments {
                    self.type_resolution_plan(ty, parameters, output)?;
                }
                self.type_resolution_plan(return_type, parameters, output)?;
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
        match self.compile_type_at(
            ty,
            self.type_scopes.len(),
            &mut BTreeSet::new(),
            &self.active_type_parameters.clone(),
        ) {
            Ok(check) => {
                self.initialize_type_resolution(ty, output);
                Some(check)
            }
            Err(error) => {
                self.initialize_type_resolution(ty, output);
                output.push_at(Instruction::Fail(error.at(ty.span)), ty.span);
                None
            }
        }
    }

    fn reference_usage_context(&mut self, start: usize, ty: &crate::syntax::Type) {
        for recipe in self.program.type_recipes.iter_from_mut(start) {
            if let Some(Err(error)) = &mut recipe.reference
                && !error.frames.is_empty()
            {
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
                    span,
                    member: "".into(),
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

    fn type_argument_sources(&self) -> BTreeMap<String, TypeArgumentSource> {
        self.argument_spans
            .iter()
            .map(|(name, span)| {
                (
                    name.clone(),
                    TypeArgumentSource {
                        span: *span,
                        text: Arc::from(&self.resolved.module.source[span.start..span.end]),
                    },
                )
            })
            .collect()
    }

    fn type_source_span_at(
        &self,
        ty: &crate::syntax::Type,
        parameters: &BTreeMap<String, Span>,
        aliases: &mut BTreeSet<Span>,
    ) -> Span {
        use crate::syntax::{ExprKind, TypeKind};
        match &ty.kind {
            TypeKind::Nullable(inner) => self.type_source_span_at(inner, parameters, aliases),
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
                ty.span
            }
            _ => ty.span,
        }
    }

    fn compile_type_at(
        &mut self,
        ty: &crate::syntax::Type,
        limit: usize,
        aliases: &mut BTreeSet<Span>,
        parameters: &BTreeMap<String, TypeCheck>,
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
            let fallback = if crate::diagnostic_names::BASE_PROPERTIES.contains(&path[0].as_str()) {
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
                    let source: Arc<str> = Arc::from(
                        &self.resolved.module.source[expression.span.start..expression.span.end],
                    );
                    let expression = expression.unparenthesized();
                    if let Some(&block) = self.active_constraints.get(&expression.span) {
                        compiled.push(ConstraintCode {
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
                                    name: Arc::from(name.as_str()),
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
                    metadata.let_scopes = code.let_scopes;
                    metadata.type_casts = code.type_casts;
                    metadata.method_arities = code.method_arities;
                    metadata.constraint_root = writes_slots.then_some(expression.span);
                    metadata.inline_constraint = !writes_slots;
                    let mut origin = vec![RuntimeFrame {
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
                labels: types
                    .iter()
                    .map(|ty| match &ty.kind {
                        TypeKind::Named {
                            path, arguments, ..
                        } if path.len() == 1 && arguments.is_empty() => self
                            .parameter_spans
                            .get(&path[0])
                            .copied()
                            .unwrap_or(ty.span),
                        _ => ty.span,
                    })
                    .collect(),
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
                    .map(|argument| self.compile_type_at(argument, limit, aliases, parameters))
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
                            TypeCheck::Class {
                                module: self
                                    .source
                                    .as_ref()
                                    .map(|source| source.module_name.clone()),
                                id: self.declaration_base + member.expr.span.start,
                                name: Arc::from(member.name),
                            }
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
                            let previous_spans =
                                std::mem::replace(&mut self.parameter_spans, spans);
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
                            let previous_member = std::mem::replace(
                                &mut self.member,
                                format!("#{}", member.name).into(),
                            );
                            self.alias_frames.push(RuntimeFrame {
                                span: ty.span,
                                member: previous_member.clone(),
                                source: self.source.clone(),
                            });
                            let reference_start = self.program.type_recipes.next_id();
                            let result = self
                                .compile_type_at(aliased, index + 1, aliases, &substitutions)
                                .map_err(|mut error| {
                                    error.frames.push(RuntimeFrame {
                                        span: aliased.span,
                                        member: format!("#{}", member.name).into(),
                                        source: self.source.clone(),
                                    });
                                    error
                                });
                            for recipe in self.program.type_recipes.iter_from_mut(reference_start) {
                                if let Some(Err(error)) = &mut recipe.reference {
                                    error.frames.push(RuntimeFrame {
                                        span: aliased.span,
                                        member: format!("#{}", member.name).into(),
                                        source: self.source.clone(),
                                    });
                                }
                            }
                            self.alias_frames.pop();
                            self.constraint_module = previous_module;
                            self.member = previous_member;
                            self.parameter_spans = previous_spans;
                            self.argument_spans = previous_argument_spans;
                            let result = result?;
                            aliases.remove(&member.expr.span);
                            TypeCheck::NamedAlias {
                                declaration: self.declaration_base + member.expr.span.start,
                                name: Arc::from(
                                    self.resolved.module.source
                                        [member.name_span.start..member.name_span.end]
                                        .to_owned(),
                                ),
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
                    if path[0] == "Mixin" {
                        if arguments.len() > 1 {
                            return Err(RuntimeError::new(
                                ErrorKind::Type,
                                format!("Expected 1 type argument(s) but got {}.", arguments.len()),
                            ));
                        }
                        let argument = arguments
                            .first()
                            .map(|t| self.compile_type_at(t, limit, aliases, parameters))
                            .transpose()?
                            .unwrap_or(TypeCheck::Unknown);
                        let mut mixin = amendment::mixin_type(argument);
                        if let TypeCheck::NamedAlias { default_origin, .. } = &mut mixin {
                            *default_origin = arguments.first().map(|argument| RuntimeFrame {
                                source: self.source.clone(),
                                span: self.type_source_span(argument),
                                member: self.member.clone(),
                            });
                        }
                        return Ok(mixin);
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
                    if path[0] == "Class" {
                        if arguments.len() > 1 {
                            return Err(RuntimeError::new(
                                ErrorKind::Type,
                                format!("Expected 1 type argument(s) but got {}.", arguments.len()),
                            ));
                        }
                        // Pkl 0.32.0 erases Class<T> before resolving T.
                        return Ok(TypeCheck::MetaClass(Arc::from("Class")));
                    }
                    if path[0] == "Function"
                        || path[0]
                            .strip_prefix("Function")
                            .is_some_and(|n| matches!(n, "0" | "1" | "2" | "3" | "4" | "5"))
                    {
                        let arity = path[0]
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
                        let kind = match path[0].as_str() {
                            "Listing" => crate::heap::CollectionKind::Listing,
                            "Mapping" => crate::heap::CollectionKind::Mapping,
                            "List" => crate::heap::CollectionKind::List,
                            "Map" => crate::heap::CollectionKind::Map,
                            "Set" => crate::heap::CollectionKind::Set,
                            _ if crate::diagnostic_names::BASE_PROPERTIES
                                .contains(&path[0].as_str()) =>
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
                    match path[0].as_str() {
                        name if builtins::id(name).is_some() => TypeCheck::Class {
                            module: None,
                            id: builtins::id(name).expect("annotation class"),
                            name: Arc::from(name),
                        },
                        "Class" | "TypeAlias" => TypeCheck::MetaClass(Arc::from(path[0].as_str())),
                        "Any" => TypeCheck::Any,
                        "NonNull" => TypeCheck::NonNull,
                        "Null" => TypeCheck::Null,
                        "Boolean" => TypeCheck::Boolean,
                        "Int" => TypeCheck::Int,
                        "Float" => TypeCheck::Float,
                        "Duration" => TypeCheck::Duration,
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
                                name: Arc::from(name),
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
                        _ if crate::diagnostic_names::BASE_PROPERTIES
                            .contains(&path[0].as_str()) =>
                        {
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
        self.class_value(&check, output, ty.span)
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
        if matches!(
            check.unaliased(),
            TypeCheck::RefDomain | TypeCheck::Reference { .. }
        ) || matches!(check.unaliased(), TypeCheck::Class { id, .. } if *id == reference::ACCESS_ID || builtins::name(*id).is_some())
        {
            output.push_at(Instruction::BuiltinClass(check.clone()), span);
            return Ok(());
        }
        let TypeCheck::Class { id, .. } = check.unaliased() else {
            return Err(RuntimeError::new(ErrorKind::Type, "Expected a class type."));
        };
        for (index, members) in self.type_scopes.iter().enumerate().rev() {
            if let Some(member) = members.iter().find(|member| {
                self.declaration_base + member.expr.span.start == *id
                    && matches!(member.expr.kind, crate::syntax::ExprKind::Class { .. })
            }) {
                output.push_at(
                    Instruction::GetProperty {
                        name: Arc::from(member.name.as_str()),
                        owner_depth: self.type_scopes.len() - 1 - index,
                        local: crate::syntax::modifier_flags(&member.modifiers)
                            & crate::syntax::ModifierKind::Local.flag()
                            != 0,
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
                source: self.source.clone(),
                span: self.type_source_span(ty),
                member: self.member.concat(&".default".into()),
            };
            if self.default_type_with_origin(&check, output, ty.span, Some(&origin))? {
                return Ok(());
            }
        }
        output.push_at(
            Instruction::Undefined(Arc::from(member.name.as_str())),
            member.name_span,
        );
        Ok(())
    }

    fn default_type(
        &mut self,
        check: &TypeCheck,
        output: &mut Code,
        span: Span,
    ) -> Result<bool, RuntimeError> {
        self.default_type_with_origin(check, output, span, None)
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
        match check {
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
            TypeCheck::NamedAlias { declaration, .. } if *declaration == amendment::MIXIN_ID => {
                output.push_at(
                    Instruction::GenericConstructorParent {
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
                    if let TypeCheck::ParameterizedCollection { arguments, .. } = check
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
                if self.type_scopes.iter().flatten().any(|member| {
                    self.declaration_base + member.expr.span.start == *id
                        && matches!(member.expr.kind, crate::syntax::ExprKind::Class { .. })
                        && crate::syntax::modifier_flags(&member.modifiers)
                            & crate::syntax::ModifierKind::Abstract.flag()
                            != 0
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
                _ => false,
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
