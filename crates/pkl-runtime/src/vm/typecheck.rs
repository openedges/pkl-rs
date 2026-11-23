//! Resumable type checking uses the same bytecode frames, Host suspension, and heap.
use super::*;
use crate::diagnostic::{PowerAssertionValue, TypeDiagnostic, UnionTypeDiagnostic};

#[derive(Clone, Copy)]
pub(super) enum TypeDestination {
    Stack,
    Test,
    /// Renderer TypeNode.execute: mismatch is false, ordinary checks stay lazy.
    ProtobufAlternative,
    Return,
    Local {
        depth: Option<usize>,
        policy: identity::SlotPolicy,
    },
}

#[derive(Clone)]
pub(super) enum ObservedValue {
    Runtime(Value),
    // Native constant folding can retain a value's canonical representation
    // without allocating a guest object (the pinned ModuleInfo regex binding).
    NativeLiteral(PklString),
}
impl ObservedValue {
    pub(super) fn roots(&self) -> Option<Value> {
        match self {
            Self::Runtime(value) => Some(value.clone()),
            Self::NativeLiteral(_) => None,
        }
    }
    pub(super) fn render(
        &self,
        heap: &Heap,
        module: &PklString,
    ) -> Result<PklString, RuntimeError> {
        match self {
            Self::Runtime(value) => {
                crate::diagnostic_values::render_string(value, heap, module, 100)
            }
            Self::NativeLiteral(text) => Ok(text.clone()),
        }
    }
}

#[derive(Clone)]
pub(super) struct Observation {
    pub(super) value: ObservedValue,
    pub(super) executions: usize,
}
pub(super) type Trace = BTreeMap<Span, Observation>;

#[derive(Clone)]
enum Mismatch {
    SourceSection {
        inner: Box<Mismatch>,
        section: crate::type_source::TypeSourceSection,
    },
    AliasSource {
        inner: Box<Mismatch>,
        source: Arc<crate::value::RuntimeSource>,
    },
    InsertedFrame {
        inner: Box<Mismatch>,
        frame: RuntimeFrame,
    },
    Simple {
        span: Span,
        ty: TypeCheck,
        value: Value,
    },
    Constraint {
        source: Option<Arc<crate::value::RuntimeSource>>,
        span: Span,
        value: Value,
        body_span: Span,
        observations: Option<Trace>,
    },
    Union {
        span: Span,
        value: Value,
        children: Vec<(
            Span,
            Option<crate::type_source::TypeSourceSection>,
            Mismatch,
        )>,
    },
}
impl Mismatch {
    fn diagnostic(
        &self,
        heap: &Heap,
        module: &PklString,
        indent: usize,
    ) -> Result<TypeDiagnostic, RuntimeError> {
        use crate::diagnostic_values::{render_string as render, type_name as actual_type};
        Ok(match self {
            Self::SourceSection { inner, section } => inner
                .diagnostic(heap, module, indent)?
                .with_source(section.clone()),
            Self::InsertedFrame { inner, .. } => inner.diagnostic(heap, module, indent)?,
            Self::AliasSource { inner, source } => {
                let diagnostic = inner.diagnostic(heap, module, indent)?;
                let section = crate::type_source::TypeSourceSection::new(
                    crate::type_source::TypeSourceOwner::External(source.clone()),
                    diagnostic.span(),
                );
                diagnostic.with_source(section)
            }
            Self::Simple { span, ty, value } => TypeDiagnostic::Simple {
                span: *span,
                message: type_error_at_width(
                    ty,
                    value,
                    module,
                    heap,
                    80usize.saturating_sub(indent),
                )?
                .message,
            },
            Self::Constraint {
                span,
                value,
                body_span,
                observations,
                ..
            } => TypeDiagnostic::Constraint {
                span: *span,
                value: render(value, heap, module, 80usize.saturating_sub(indent))?,
                body_span: *body_span,
                observations: observations
                    .as_ref()
                    .map(|trace| {
                        trace
                            .iter()
                            .map(|(span, observation)| {
                                Ok(PowerAssertionValue {
                                    span: *span,
                                    value: observation.value.render(heap, module)?,
                                    executions: observation.executions,
                                    hidden: matches!(
                                        observation.value,
                                        ObservedValue::Runtime(Value::Function(_))
                                    ),
                                })
                            })
                            .collect::<Result<Vec<_>, RuntimeError>>()
                    })
                    .transpose()?,
            },
            Self::Union {
                span,
                value,
                children,
            } => TypeDiagnostic::Union {
                span: *span,
                value: render(value, heap, module, 80usize.saturating_sub(indent))?,
                actual_type: actual_type(value, heap, module)?,
                is_null: value.is_null(),
                children: children
                    .iter()
                    .map(|(type_span, type_source, mismatch)| {
                        Ok(UnionTypeDiagnostic {
                            type_span: *type_span,
                            type_source: type_source.clone(),
                            nontrivial: !mismatch.is_trivial(type_source.as_ref(), *type_span),
                            diagnostic: Box::new(mismatch.diagnostic(heap, module, indent + 2)?),
                        })
                    })
                    .collect::<Result<Vec<_>, RuntimeError>>()?,
            },
        })
    }
    fn section(&self) -> Option<&crate::type_source::TypeSourceSection> {
        match self {
            Self::SourceSection { inner, section } => inner.section().or(Some(section)),
            Self::AliasSource { inner, .. } | Self::InsertedFrame { inner, .. } => inner.section(),
            _ => None,
        }
    }
    fn is_trivial(
        &self,
        expected: Option<&crate::type_source::TypeSourceSection>,
        expected_span: Span,
    ) -> bool {
        let section = self.section();
        let mut inner = self;
        let mut alias_without_section = false;
        loop {
            match inner {
                Self::AliasSource { inner: child, .. } => {
                    alias_without_section |= child.section().is_none();
                    inner = child;
                }
                Self::SourceSection { inner: child, .. }
                | Self::InsertedFrame { inner: child, .. } => inner = child,
                _ => break,
            }
        }
        if alias_without_section {
            return false;
        }
        match inner {
            Self::Simple { span, ty, .. } if !matches!(ty.unaliased(), TypeCheck::Nothing) => {
                match (section, expected) {
                    (Some(actual), Some(expected)) => actual.same_identity(expected),
                    (None, None) => *span == expected_span,
                    _ => false,
                }
            }
            _ => false,
        }
    }
    fn roots(&self, roots: &mut Vec<Value>) {
        match self {
            Self::SourceSection { inner, .. }
            | Self::InsertedFrame { inner, .. }
            | Self::AliasSource { inner, .. } => inner.roots(roots),
            Self::Simple { value, .. } => roots.push(value.clone()),
            Self::Constraint {
                value,
                observations,
                ..
            } => {
                roots.push(value.clone());
                if let Some(trace) = observations {
                    roots.extend(
                        trace
                            .values()
                            .filter_map(|observation| observation.value.roots()),
                    );
                }
            }
            Self::Union {
                value, children, ..
            } => {
                roots.push(value.clone());
                for (_, _, mismatch) in children {
                    mismatch.roots(roots);
                }
            }
        }
    }
}

#[derive(Clone)]
struct UnionState {
    types: Vec<TypeCheck>,
    spans: Vec<Span>,
    labels: Vec<Span>,
    label_sources: Vec<Option<crate::type_source::TypeSourceSection>>,
    span: Span,
    index: usize,
    failures: Vec<(
        Span,
        Option<crate::type_source::TypeSourceSection>,
        Mismatch,
    )>,
    parent_test: bool,
    parent_eager: bool,
    replay: bool,
}
#[derive(Clone)]
struct CollectionItem {
    owner: ObjectId,
    member: crate::heap::CollectionMember,
}
#[derive(Clone)]
struct CollectionState {
    value: Value,
    checked: Value,
    kind: crate::heap::CollectionKind,
    arguments: Vec<(TypeCheck, Span)>,
    items: Vec<CollectionItem>,
    index: usize,
    eager: bool,
    replacements: Vec<(PklString, Value)>,
}
#[derive(Clone)]
struct PairState {
    value: Value,
    checked: Value,
    components: [Value; 2],
    arguments: Vec<(TypeCheck, Span)>,
    index: usize,
}
#[derive(Clone)]
enum Task {
    SourceSection(crate::type_source::TypeSourceSection),
    AliasSource(Arc<crate::value::RuntimeSource>),
    PairResult(PairState),
    ReferenceDomainResult {
        value: Value,
        ty: TypeCheck,
        span: Span,
    },
    Check {
        ty: TypeCheck,
        span: Span,
    },
    CollectionNext(CollectionState),
    CollectionKeyResult(CollectionState),
    CollectionValueReady(CollectionState),
    CollectionValueResult(CollectionState),
    RestoreEnvironment(Box<Environment>),
    RestoreTypeNode(Option<TypeNodeId>),
    AfterBase {
        constraints: Vec<ConstraintCode>,
        parent_eager: bool,
    },
    Constraints {
        constraints: Vec<ConstraintCode>,
        index: usize,
    },
    ConstraintResult {
        constraints: Vec<ConstraintCode>,
        index: usize,
    },
    PredicateResult {
        constraints: Vec<ConstraintCode>,
        index: usize,
        body_span: Span,
        function: ObjectId,
        value: Value,
    },
    TraceResult {
        span: Span,
        body_span: Span,
        predicate_value: Option<Value>,
    },
    UnionResult(UnionState),
}

pub(super) struct TypeRun {
    pub(super) node: Option<TypeNodeId>,
    value: Value,
    checked: Value,
    pub(super) environment: Environment,
    pub(super) member: PklString,
    tasks: Vec<Task>,
    outcome: Option<Result<Value, Mismatch>>,
    pub(super) returned: Option<Value>,
    destination: TypeDestination,
    frames: Vec<RuntimeFrame>,
    in_type_test: bool,
    eager: bool,
    constraint_active: bool,
    tracker: Option<Trace>,
    constraint_source: Option<Arc<crate::value::RuntimeSource>>,
    constraint_section: Option<crate::type_source::TypeSourceSection>,
}
pub(super) fn environment_roots(environment: &Environment, roots: &mut Vec<Value>) {
    roots.extend(environment.locals.iter().cloned());
    roots.extend(environment.this_value.iter().cloned());
    roots.extend(environment.outer_value.iter().cloned());
    roots.extend(environment.custom_this.map(|slot| slot.root()));
    roots.extend(environment.executable_owner.map(|owner| owner.root()));
    roots.extend(environment.code_owner.map(|owner| owner.root()));
    roots.extend(
        environment
            .local_owners
            .values()
            .map(|(owner, _)| owner.root()),
    );
    for scope in &environment.scopes {
        roots.extend([Value::Object(scope.receiver), Value::Object(scope.owner)]);
    }
}
impl TypeRun {
    pub(super) fn has_tracker(&self) -> bool {
        self.tracker.is_some()
    }

    pub(super) fn is_protobuf_alternative(&self) -> bool {
        matches!(self.destination, TypeDestination::ProtobufAlternative)
    }

    /// Inherited annotation continuations have no authored Return instruction.
    /// Fatal child evaluation still needs the overriding property's value frame.
    pub(super) fn fatal_value_frames(&self) -> impl Iterator<Item = &RuntimeFrame> {
        self.frames.iter().skip(1)
    }
    pub(super) fn roots(&self, roots: &mut Vec<Value>) {
        roots.extend(self.node.map(|node| node.0.root()));
        roots.extend([self.value.clone(), self.checked.clone()]);
        roots.extend(self.returned.iter().cloned());
        environment_roots(&self.environment, roots);
        if let Some(trace) = &self.tracker {
            roots.extend(
                trace
                    .values()
                    .filter_map(|observation| observation.value.roots()),
            );
        }
        match &self.outcome {
            Some(Err(mismatch)) => mismatch.roots(roots),
            Some(Ok(value)) => roots.push(value.clone()),
            None => {}
        }
        for task in &self.tasks {
            match task {
                Task::CollectionNext(state)
                | Task::CollectionKeyResult(state)
                | Task::CollectionValueReady(state)
                | Task::CollectionValueResult(state) => {
                    roots.extend([state.value.clone(), state.checked.clone()]);
                    for item in &state.items {
                        roots.extend([Value::Object(item.owner), item.member.key.clone()]);
                    }
                    roots.extend(state.replacements.iter().map(|(_, value)| value.clone()));
                }
                Task::UnionResult(state) => {
                    for (_, _, mismatch) in &state.failures {
                        mismatch.roots(roots);
                    }
                }
                Task::ReferenceDomainResult { value, .. } => roots.push(value.clone()),
                Task::RestoreEnvironment(environment) => environment_roots(environment, roots),
                Task::RestoreTypeNode(node) => roots.extend(node.map(|node| node.0.root())),
                Task::PredicateResult {
                    function, value, ..
                } => {
                    roots.push(Value::Function(*function));
                    roots.push(value.clone());
                }
                Task::PairResult(state) => {
                    roots.extend([state.value.clone(), state.checked.clone()]);
                    roots.extend(state.components.iter().cloned());
                }
                Task::TraceResult {
                    predicate_value, ..
                } => roots.extend(predicate_value.iter().cloned()),
                _ => {}
            }
        }
    }
    fn mismatch(&mut self, span: Span, body_span: Span, value: Value) -> bool {
        let observations = self.tracker.take();
        let had_tracker = observations.is_some();
        let mismatch = Mismatch::Constraint {
            source: self.constraint_source.clone(),
            span,
            value,
            body_span,
            observations,
        };
        self.outcome = Some(Err(match &self.constraint_section {
            Some(section) => Mismatch::SourceSection {
                inner: Box::new(mismatch),
                section: section.clone(),
            },
            None => mismatch,
        }));
        had_tracker
    }
}
// Function parameter/result and Reference referent types are metadata; they do
// not evaluate constraints in this frame. The remaining wrappers can do so.
pub(super) fn needs_custom_this(ty: &TypeCheck) -> bool {
    match ty {
        TypeCheck::Shared(node) => node.needs_custom_this(),
        TypeCheck::Constrained { .. } | TypeCheck::Declared { .. } => true,
        TypeCheck::NamedAlias { expanded, .. }
        | TypeCheck::Nullable(expanded)
        | TypeCheck::Alias(expanded)
        | TypeCheck::GenericAlias(expanded) => needs_custom_this(expanded),
        TypeCheck::Reference { domain, .. } => needs_custom_this(domain),
        TypeCheck::ParameterizedCollection { arguments, .. }
        | TypeCheck::Pair { arguments }
        | TypeCheck::CollectionBase { arguments } => {
            arguments.iter().any(|(ty, _)| needs_custom_this(ty))
        }
        TypeCheck::Union { types, .. } => types.iter().any(needs_custom_this),
        _ => false,
    }
}

fn module_environment(environment: &Environment) -> Result<Environment, RuntimeError> {
    let mut result = environment.clone();
    let scope = *result
        .scopes
        .last()
        .ok_or_else(|| invalid("missing alias module"))?;
    result.scopes = crate::heap::ScopeList::from_single(scope);
    // TypeAliasTypeNode swaps receiver and owner in the original frame, preserving locals.
    result.this_value = Some(Value::Object(scope.receiver));
    result.outer_value = None;
    Ok(result)
}

/// Lazy collection cast roots have identity even when their value type does
/// not need a custom-this cell. Keep these two allocation policies separate.
pub(super) fn needs_type_identity(ty: &TypeCheck) -> bool {
    needs_type_identity_depth(ty, 0)
}

fn needs_type_identity_depth(ty: &TypeCheck, depth: u8) -> bool {
    if let TypeCheck::Shared(node) = ty {
        return node.needs_type_identity();
    }
    match ty {
        TypeCheck::Any
        | TypeCheck::Unknown
        | TypeCheck::Nothing
        | TypeCheck::NonNull
        | TypeCheck::Null
        | TypeCheck::Boolean
        | TypeCheck::Int
        | TypeCheck::Float
        | TypeCheck::Number
        | TypeCheck::String
        | TypeCheck::Duration
        | TypeCheck::Bytes
        | TypeCheck::RefDomain
        | TypeCheck::StringLiteral(_)
        | TypeCheck::IntegerAlias { .. }
        | TypeCheck::MetaClass(_)
        | TypeCheck::Collection(_)
        | TypeCheck::Class { .. }
        | TypeCheck::Parameter { .. }
        | TypeCheck::Function { .. } => return false,
        TypeCheck::Constrained { .. }
        | TypeCheck::Declared { .. }
        | TypeCheck::ParameterizedCollection {
            kind: crate::heap::CollectionKind::Listing | crate::heap::CollectionKind::Mapping,
            ..
        } => return true,
        _ => {}
    }
    if depth < 16 {
        let next = depth + 1;
        return match ty {
            TypeCheck::NamedAlias { expanded, .. }
            | TypeCheck::Nullable(expanded)
            | TypeCheck::Alias(expanded)
            | TypeCheck::GenericAlias(expanded) => needs_type_identity_depth(expanded, next),
            TypeCheck::Reference { domain, .. } => needs_type_identity_depth(domain, next),
            TypeCheck::ParameterizedCollection { arguments, .. }
            | TypeCheck::Pair { arguments }
            | TypeCheck::CollectionBase { arguments } => arguments
                .iter()
                .any(|(arg, _)| needs_type_identity_depth(arg, next)),
            TypeCheck::Union { types, .. } => {
                types.iter().any(|alt| needs_type_identity_depth(alt, next))
            }
            TypeCheck::VarArgs { element, .. } => needs_type_identity_depth(element, next),
            _ => false,
        };
    }
    if needs_custom_this(ty) {
        return true;
    }
    let mut pending = vec![ty];
    let mut seen = BTreeSet::new();
    while let Some(ty) = pending.pop() {
        if let TypeCheck::Shared(node) = ty {
            if node.needs_type_identity() {
                return true;
            }
            continue;
        }
        let ty = ty.resolved();
        if !seen.insert(ty as *const TypeCheck) {
            continue;
        }
        match ty {
            TypeCheck::ParameterizedCollection {
                kind: crate::heap::CollectionKind::Listing | crate::heap::CollectionKind::Mapping,
                ..
            } => return true,
            // These types are metadata; their nested annotations do not execute
            // at this type-check site.
            TypeCheck::Function { .. } => (),
            TypeCheck::Reference { domain, .. } => pending.push(domain),
            _ => pending.extend(ty.type_children()),
        }
    }
    false
}

/// Fast synchronous check for leaf primitive types that never substitute type
/// variables, never allocate type-node identity or custom-this cells, never
/// run constraints, and never transform/cast the checked value.
#[inline]
pub(super) fn try_fast_leaf_type(ty: &TypeCheck, value: &Value) -> Option<bool> {
    match ty.resolved() {
        TypeCheck::Any | TypeCheck::Unknown => Some(true),
        TypeCheck::Int => Some(matches!(value, Value::Int(_))),
        TypeCheck::Float => Some(matches!(value, Value::Float(_))),
        TypeCheck::Number => Some(matches!(value, Value::Int(_) | Value::Float(_))),
        TypeCheck::Boolean => Some(matches!(value, Value::Bool(_))),
        TypeCheck::String => Some(matches!(value, Value::String(_))),
        TypeCheck::NonNull => Some(!value.is_null()),
        TypeCheck::Null => Some(value.is_null()),
        TypeCheck::Duration => Some(matches!(value, Value::Duration(_))),
        TypeCheck::Bytes => Some(matches!(value, Value::Bytes(_))),
        TypeCheck::IntegerAlias { min, max, .. } => {
            Some(matches!(value, Value::Int(i) if i.get() >= *min && i.get() <= *max))
        }
        TypeCheck::StringLiteral(expected) => {
            Some(matches!(value, Value::String(actual) if actual == expected))
        }
        TypeCheck::Alias(inner) | TypeCheck::GenericAlias(inner) => {
            try_fast_leaf_type(inner, value)
        }
        TypeCheck::NamedAlias {
            arguments,
            expanded: inner,
            ..
        } if arguments.iter().all(|a| !a.has_declared()) => try_fast_leaf_type(inner, value),
        TypeCheck::Nullable(inner) => {
            if value.is_null() {
                try_fast_leaf_type(inner, value).map(|_| true)
            } else {
                try_fast_leaf_type(inner, value)
            }
        }
        _ => None,
    }
}

#[inline]
fn is_fast_heap_type(ty: &TypeCheck, depth: u8) -> bool {
    if depth >= 8 {
        return false;
    }
    match ty.resolved() {
        TypeCheck::Any
        | TypeCheck::Unknown
        | TypeCheck::Nothing
        | TypeCheck::Int
        | TypeCheck::Float
        | TypeCheck::Number
        | TypeCheck::Boolean
        | TypeCheck::String
        | TypeCheck::NonNull
        | TypeCheck::Null
        | TypeCheck::Duration
        | TypeCheck::Bytes
        | TypeCheck::IntegerAlias { .. }
        | TypeCheck::StringLiteral(_)
        | TypeCheck::Class { .. }
        | TypeCheck::Collection(_) => true,
        TypeCheck::MetaClass(name) => name == "Class" || name == "TypeAlias",
        TypeCheck::Alias(inner) | TypeCheck::GenericAlias(inner) | TypeCheck::Nullable(inner) => {
            is_fast_heap_type(inner, depth + 1)
        }
        TypeCheck::NamedAlias {
            arguments,
            expanded: inner,
            ..
        } => arguments.iter().all(|a| !a.has_declared()) && is_fast_heap_type(inner, depth + 1),
        TypeCheck::Union {
            string_literals: true,
            types,
            ..
        } => !types.is_empty(),
        TypeCheck::Union {
            string_literals: false,
            types,
            ..
        } => !types.is_empty() && types.iter().all(|alt| is_fast_heap_type(alt, depth + 1)),
        TypeCheck::ParameterizedCollection {
            kind: crate::heap::CollectionKind::List | crate::heap::CollectionKind::Set,
            arguments,
        } if arguments.len() == 1 => {
            (arguments[0].0.noop() && !arguments[0].0.has_declared())
                || is_fast_heap_type(&arguments[0].0, depth + 1)
        }
        TypeCheck::ParameterizedCollection {
            kind: crate::heap::CollectionKind::Map,
            arguments,
        } if arguments.len() == 2 => {
            ((arguments[0].0.noop() && !arguments[0].0.has_declared())
                || is_fast_heap_type(&arguments[0].0, depth + 1))
                && ((arguments[1].0.noop() && !arguments[1].0.has_declared())
                    || is_fast_heap_type(&arguments[1].0, depth + 1))
        }
        TypeCheck::CollectionBase { arguments } if arguments.len() <= 1 => {
            arguments.first().is_none_or(|(ty, _)| {
                (ty.noop() && !ty.has_declared()) || is_fast_heap_type(ty, depth + 1)
            })
        }
        _ => false,
    }
}

#[inline]
fn try_fast_strict_list_check(
    elem_ty: &TypeCheck,
    value: &Value,
    heap: &Heap,
    depth: u8,
) -> Option<bool> {
    let elem_noop = elem_ty.noop() && !elem_ty.has_declared();
    if !elem_noop && !is_fast_heap_type(elem_ty, depth) {
        return None;
    }
    let Value::Object(id) = value else {
        return Some(false);
    };
    let object = heap.get(*id).ok()?;
    if object.collection_kind != crate::heap::CollectionKind::List
        || object.foreign_map_entry.is_some()
        || object.is_module
        || object.reference.is_some()
        || object.ref_module
    {
        return Some(false);
    }
    if object.parent.is_some()
        || object.instance_class.is_some()
        || object.class_definition.is_some()
    {
        return None;
    }
    let Some(elems) = &object.list_elements else {
        return None;
    };
    if elems.len() != object.length {
        return None;
    }
    if elem_noop {
        return Some(true);
    }
    for val in elems {
        match try_fast_type_with_heap_depth(elem_ty, val, heap, depth + 1) {
            Some(true) => continue,
            Some(false) => return Some(false),
            None => return None,
        }
    }
    Some(true)
}

#[inline]
fn try_fast_strict_set_check(
    elem_ty: &TypeCheck,
    value: &Value,
    heap: &Heap,
    depth: u8,
) -> Option<bool> {
    let elem_noop = elem_ty.noop() && !elem_ty.has_declared();
    if !elem_noop && !is_fast_heap_type(elem_ty, depth) {
        return None;
    }
    let Value::Object(id) = value else {
        return Some(false);
    };
    let object = heap.get(*id).ok()?;
    if object.collection_kind != crate::heap::CollectionKind::Set
        || object.foreign_map_entry.is_some()
        || object.is_module
        || object.reference.is_some()
        || object.ref_module
    {
        return Some(false);
    }
    if object.parent.is_some()
        || object.instance_class.is_some()
        || object.class_definition.is_some()
    {
        return None;
    }
    if elem_noop {
        return Some(true);
    }
    let order = object.map_index().insertion_order();
    if order.len() != object.length {
        return None;
    }
    for &idx in order {
        let val = object.set_entries.get(idx)?;
        match try_fast_type_with_heap_depth(elem_ty, val, heap, depth + 1) {
            Some(true) => continue,
            Some(false) => return Some(false),
            None => return None,
        }
    }
    Some(true)
}

#[inline]
fn try_fast_strict_map_check(
    key_ty: &TypeCheck,
    val_ty: &TypeCheck,
    value: &Value,
    heap: &Heap,
    depth: u8,
) -> Option<bool> {
    let key_noop = key_ty.noop() && !key_ty.has_declared();
    let val_noop = val_ty.noop() && !val_ty.has_declared();
    if (!key_noop && !is_fast_heap_type(key_ty, depth))
        || (!val_noop && !is_fast_heap_type(val_ty, depth))
    {
        return None;
    }
    let Value::Object(id) = value else {
        return Some(false);
    };
    let object = heap.get(*id).ok()?;
    if object.collection_kind != crate::heap::CollectionKind::Map
        || object.foreign_map_entry.is_some()
        || object.is_module
        || object.reference.is_some()
        || object.ref_module
    {
        return Some(false);
    }
    if object.parent.is_some()
        || object.instance_class.is_some()
        || object.class_definition.is_some()
    {
        return None;
    }
    if key_noop && val_noop {
        return Some(true);
    }
    let check_member =
        |member: &crate::heap::CollectionMember, slot_hint: Option<usize>| -> Option<bool> {
            if member.kind == crate::heap::CollectionMemberKind::Property {
                return Some(true);
            }
            if !key_noop {
                match try_fast_type_with_heap_depth(key_ty, &member.key, heap, depth + 1) {
                    Some(true) => {}
                    other => return other,
                }
            }
            if !val_noop {
                if matches!(member.key, Value::Object(_)) {
                    return None;
                }
                let val = if let Some(hint) = slot_hint
                    && let Some(slot) = object.cache_slots.get(hint)
                    && slot.key.entry
                    && slot.key.owner.is_none()
                    && !slot.key.method
                    && slot.key.name == member.name
                {
                    &slot.value
                } else if let Some(LazyState::Completed(v)) = object.cache.get(&CacheKey {
                    owner: None,
                    name: member.name.clone(),
                    method: false,
                    entry: true,
                }) {
                    v
                } else {
                    return None;
                };
                match try_fast_type_with_heap_depth(val_ty, val, heap, depth + 1) {
                    Some(true) => {}
                    other => return other,
                }
            }
            Some(true)
        };
    if let Some(order) = &object.map_key_order {
        for member in order {
            match check_member(member, None) {
                Some(true) => continue,
                other => return other,
            }
        }
    } else {
        let order = object.map_index().insertion_order();
        if order.len() != object.length {
            return None;
        }
        for &idx in order {
            let member = object.collection_members.get(idx)?;
            match check_member(member, Some(idx)) {
                Some(true) => continue,
                other => return other,
            }
        }
    }
    Some(true)
}

fn try_fast_type_with_heap(ty: &TypeCheck, value: &Value, heap: &Heap) -> Option<bool> {
    try_fast_type_with_heap_depth(ty, value, heap, 0)
}

fn try_fast_type_with_heap_depth(
    ty: &TypeCheck,
    value: &Value,
    heap: &Heap,
    depth: u8,
) -> Option<bool> {
    if let Some(matched) = try_fast_leaf_type(ty, value) {
        return Some(matched);
    }
    if depth >= 8 {
        return None;
    }
    match ty.resolved() {
        TypeCheck::Nothing => Some(false),
        TypeCheck::MetaClass(name) => {
            if name == "Class" {
                Some(matches!(value, Value::Class(_)))
            } else if name == "TypeAlias" {
                Some(matches!(value, Value::TypeAlias(_)))
            } else {
                None
            }
        }
        TypeCheck::Collection(kind) => Some(TypeCheck::Collection(*kind).accepts(value, heap)),
        TypeCheck::Class { id, .. } => Some(classes::accepts_nominal(*id, value, heap)),
        TypeCheck::ParameterizedCollection {
            kind: crate::heap::CollectionKind::List,
            arguments,
        } if arguments.len() == 1 => {
            try_fast_strict_list_check(&arguments[0].0, value, heap, depth + 1)
        }
        TypeCheck::ParameterizedCollection {
            kind: crate::heap::CollectionKind::Set,
            arguments,
        } if arguments.len() == 1 => {
            try_fast_strict_set_check(&arguments[0].0, value, heap, depth + 1)
        }
        TypeCheck::ParameterizedCollection {
            kind: crate::heap::CollectionKind::Map,
            arguments,
        } if arguments.len() == 2 => {
            try_fast_strict_map_check(&arguments[0].0, &arguments[1].0, value, heap, depth + 1)
        }
        TypeCheck::CollectionBase { arguments } if arguments.is_empty() => Some(
            TypeCheck::CollectionBase {
                arguments: Vec::new(),
            }
            .accepts(value, heap),
        ),
        TypeCheck::CollectionBase { arguments } if arguments.len() == 1 => {
            let elem_ty = &arguments[0].0;
            let elem_fast = (elem_ty.noop() && !elem_ty.has_declared())
                || is_fast_heap_type(elem_ty, depth + 1);
            let Value::Object(id) = value else {
                return elem_fast.then_some(false);
            };
            let object = heap.get(*id).ok()?;
            match object.collection_kind {
                crate::heap::CollectionKind::List => {
                    try_fast_strict_list_check(elem_ty, value, heap, depth + 1)
                }
                crate::heap::CollectionKind::Set => {
                    try_fast_strict_set_check(elem_ty, value, heap, depth + 1)
                }
                _ => elem_fast.then_some(false),
            }
        }
        TypeCheck::Nullable(inner) => {
            if value.is_null() {
                try_fast_type_with_heap_depth(inner, value, heap, depth + 1).map(|_| true)
            } else {
                try_fast_type_with_heap_depth(inner, value, heap, depth + 1)
            }
        }
        TypeCheck::Alias(inner) | TypeCheck::GenericAlias(inner) => {
            try_fast_type_with_heap_depth(inner, value, heap, depth + 1)
        }
        TypeCheck::NamedAlias {
            arguments,
            expanded: inner,
            ..
        } if arguments.iter().all(|a| !a.has_declared()) => {
            try_fast_type_with_heap_depth(inner, value, heap, depth + 1)
        }
        TypeCheck::Union {
            types,
            string_literals: true,
            ..
        } if !types.is_empty() => {
            match value {
                Value::String(actual) => Some(types.iter().any(
                    |alt| matches!(alt.resolved(), TypeCheck::StringLiteral(s) if s == actual),
                )),
                _ => Some(false),
            }
        }
        TypeCheck::Union {
            types,
            string_literals: false,
            ..
        } if !types.is_empty() => {
            for alt in types {
                match try_fast_type_with_heap_depth(alt, value, heap, depth + 1) {
                    Some(true) => return Some(true),
                    Some(false) => continue,
                    None => return None,
                }
            }
            Some(false)
        }
        _ => None,
    }
}

impl Vm {
    pub(super) fn observe(&mut self, span: Span, value: Value) {
        if self.active_type_trackers == 0 || !self.power_assertions {
            return;
        }
        self.observe_value(span, ObservedValue::Runtime(value));
    }
    pub(super) fn observe_native_literal(&mut self, span: Span, rendered: PklString) {
        if self.active_type_trackers == 0 || !self.power_assertions {
            return;
        }
        self.observe_value(span, ObservedValue::NativeLiteral(rendered));
    }
    fn observe_value(&mut self, span: Span, value: ObservedValue) {
        if self.active_type_trackers == 0 || !self.power_assertions {
            return;
        }
        if let Some((target_uri, fact_span, trace)) = &mut self.fact_tracker {
            let in_target_module = self
                .frames
                .last()
                .and_then(|frame| self.program.metadata.get(frame.block))
                .and_then(|meta| meta.source.as_ref())
                .is_some_and(|src| src.uri == *target_uri);
            if in_target_module && span.start >= fact_span.start && span.end <= fact_span.end {
                trace
                    .entry(span)
                    .and_modify(|observation| {
                        observation.executions += 1;
                        observation.value = value.clone();
                    })
                    .or_insert_with(|| Observation {
                        value: value.clone(),
                        executions: 1,
                    });
            }
        }
        for frame in &mut self.frames {
            if let Some(run) = &mut frame.type_run
                && let Some(trace) = &mut run.tracker
            {
                trace
                    .entry(span)
                    .and_modify(|observation| {
                        observation.executions += 1;
                        observation.value = value.clone();
                    })
                    .or_insert_with(|| Observation {
                        value: value.clone(),
                        executions: 1,
                    });
            }
        }
    }
    fn active_tracker(&self) -> bool {
        self.active_type_trackers > 0
    }
    pub(super) fn try_fast_lazy_collection_check(
        &mut self,
        ty: &TypeCheck,
        value: &Value,
        parent_node: impl FnOnce(&mut Self) -> Result<TypeNodeId, RuntimeError>,
        source: impl FnOnce(&Self) -> Option<Arc<crate::value::RuntimeSource>>,
        member: impl FnOnce(&Self) -> PklString,
        cast_env: impl FnOnce(&mut Self) -> Result<Environment, RuntimeError>,
    ) -> Result<Option<Value>, RuntimeError> {
        if !self.suspended.is_empty()
            || self.frames.iter().any(|frame| frame.type_run.is_some())
            || ty.has_declared()
            || ty.requires_substitution()
        {
            return Ok(None);
        }
        let (inner, nullable) = match ty.resolved() {
            TypeCheck::Nullable(inner) if !value.is_null() => (inner.resolved(), true),
            other => (other, false),
        };
        let TypeCheck::ParameterizedCollection { kind, arguments } = inner else {
            return Ok(None);
        };
        if !matches!(
            kind,
            crate::heap::CollectionKind::Listing | crate::heap::CollectionKind::Mapping
        ) || arguments.is_empty()
        {
            return Ok(None);
        }
        if !TypeCheck::Collection(*kind).accepts(value, &self.heap) {
            return Ok(None);
        }
        let Value::Object(id) = *value else {
            unreachable!()
        };
        if *kind == crate::heap::CollectionKind::Mapping && !arguments[0].0.noop() {
            let key_ty = &arguments[0].0;
            let mut current = Some(id);
            while let Some(owner) = current {
                let object = self.heap.get(owner)?;
                for m in &object.collection_members {
                    if m.kind != crate::heap::CollectionMemberKind::Property
                        && try_fast_type_with_heap(key_ty, &m.key, &self.heap) != Some(true)
                    {
                        return Ok(None);
                    }
                }
                current = object.parent;
            }
        }
        if needs_custom_this(ty)
            && self
                .frames
                .last()
                .expect("type frame")
                .environment
                .custom_this
                .is_none()
        {
            let slot = self.heap.allocate_custom_this(None);
            self.frames
                .last_mut()
                .expect("type frame")
                .environment
                .custom_this = Some(slot);
        }
        let mut parent = parent_node(self)?;
        if nullable {
            parent = TypeNodeId(
                self.heap
                    .executable_child(parent.0, ExecutionSite::TypeChild(0))?,
            );
        }
        let (val_ty, val_span) = arguments.last().expect("collection value type");
        let child = Some(TypeNodeId(self.heap.executable_child(
            parent.0,
            ExecutionSite::TypeChild(arguments.len() - 1),
        )?));
        if val_ty.noop()
            || self
                .heap
                .get(id)?
                .collection_cast
                .as_ref()
                .is_some_and(|cast| {
                    cast.ty.equivalent(val_ty) && (!needs_custom_this(val_ty) || cast.node == child)
                })
        {
            return Ok(Some(Value::Object(id)));
        }
        let mut environment = cast_env(self)?;
        environment.locals.clear();
        environment.local_owners.clear();
        let cast_source = source(self);
        let cast_member = member(self);
        let object = self.heap.get(id)?;
        let wrapper = Object {
            parent: Some(id),
            environment: object.environment.clone(),
            collection_kind: *kind,
            length: object.length,
            collection_cast: Some(crate::heap::CollectionCast {
                source: cast_source,
                node: child,
                ty: val_ty.clone(),
                span: *val_span,
                member: cast_member,
                environment,
            }),
            ..Object::default()
        };
        Ok(Some(Value::Object(self.heap.allocate(wrapper))))
    }
    pub(super) fn begin_type(
        &mut self,
        ty: TypeCheck,
        value: Value,
        destination: TypeDestination,
        frames: Vec<RuntimeFrame>,
    ) -> Result<(), RuntimeError> {
        if let Some(matched) = try_fast_type_with_heap(&ty, &value, &self.heap) {
            match (matched, destination) {
                (true, TypeDestination::Stack) => {
                    self.stack.push(value);
                    return Ok(());
                }
                (true, TypeDestination::Local { depth, policy }) => {
                    return self.bind_local(depth, policy, value);
                }
                (true, TypeDestination::Return) => {
                    self.stack.push(value);
                    self.frames.last_mut().expect("type frame").pc -= 1;
                    return Ok(());
                }
                (matched, TypeDestination::Test | TypeDestination::ProtobufAlternative) => {
                    self.stack.push(Value::Bool(matched));
                    return Ok(());
                }
                (false, _) => {}
            }
        }
        if !matches!(
            destination,
            TypeDestination::Test | TypeDestination::ProtobufAlternative
        ) && let Some(result) = self.try_fast_lazy_collection_check(
            &ty,
            &value,
            |vm| {
                let owner = vm.current_executable()?;
                let frame = vm.frames.last().expect("type-check frame");
                let instruction = frame.pc.saturating_sub(1);
                let site = if vm
                    .program
                    .metadata
                    .get(frame.block)
                    .is_some_and(|metadata| metadata.property_typecheck == Some(instruction))
                {
                    ExecutionSite::PropertyType(frame.block)
                } else {
                    ExecutionSite::TypeUse {
                        block: frame.block,
                        instruction,
                    }
                };
                Ok(TypeNodeId(vm.heap.executable_child(owner, site)?))
            },
            |vm| match frames.first() {
                Some(frame) => frame.source.clone(),
                None => vm
                    .frames
                    .last()
                    .and_then(|frame| vm.program.metadata.get(frame.block))
                    .and_then(|metadata| metadata.source.clone()),
            },
            |vm| {
                let frame = vm.frames.last().expect("type-check frame");
                frames
                    .first()
                    .map(|f| f.member.clone())
                    .or_else(|| frame.diagnostic_member.clone())
                    .or_else(|| {
                        vm.program
                            .metadata
                            .get(frame.block)
                            .map(|metadata| metadata.member.clone())
                    })
                    .unwrap_or_default()
            },
            |vm| {
                Ok(vm
                    .frames
                    .last()
                    .expect("type-check frame")
                    .environment
                    .clone())
            },
        )? {
            match destination {
                TypeDestination::Stack => {
                    self.stack.push(result);
                    return Ok(());
                }
                TypeDestination::Local { depth, policy } => {
                    return self.bind_local(depth, policy, result);
                }
                TypeDestination::Return => {
                    self.stack.push(result);
                    self.frames.last_mut().expect("type frame").pc -= 1;
                    return Ok(());
                }
                TypeDestination::Test | TypeDestination::ProtobufAlternative => unreachable!(),
            }
        }
        let ty = ty.substituted(
            &self
                .frames
                .last()
                .ok_or_else(|| invalid("missing type-check frame"))?
                .environment,
        )?;
        let node = if needs_type_identity(&ty) {
            let owner = self.current_executable()?;
            let frame = self.frames.last().expect("type-check frame");
            let instruction = frame.pc.saturating_sub(1);
            let site = if self
                .program
                .metadata
                .get(frame.block)
                .is_some_and(|metadata| metadata.property_typecheck == Some(instruction))
            {
                ExecutionSite::PropertyType(frame.block)
            } else {
                ExecutionSite::TypeUse {
                    block: frame.block,
                    instruction,
                }
            };
            Some(TypeNodeId(self.heap.executable_child(owner, site)?))
        } else {
            None
        };
        self.begin_type_node(ty, value, destination, frames, node)
    }

    pub(super) fn begin_type_node(
        &mut self,
        ty: TypeCheck,
        value: Value,
        destination: TypeDestination,
        frames: Vec<RuntimeFrame>,
        node: Option<TypeNodeId>,
    ) -> Result<(), RuntimeError> {
        let in_type_test = matches!(destination, TypeDestination::Test)
            || self
                .frames
                .iter()
                .rev()
                .find_map(|frame| frame.type_run.as_ref())
                .is_some_and(|run| run.in_type_test);
        let eager = matches!(destination, TypeDestination::Test)
            || self
                .frames
                .iter()
                .chain(self.suspended.iter().flat_map(|context| &context.frames))
                .any(|frame| {
                    frame
                        .type_run
                        .as_ref()
                        .is_some_and(|run| run.constraint_active)
                });
        let frame = self
            .frames
            .last_mut()
            .ok_or_else(|| invalid("missing type-check frame"))?;
        if frame.environment.custom_this.is_none() && needs_custom_this(&ty) {
            frame.environment.custom_this = Some(self.heap.allocate_custom_this(None));
        }
        let metadata = self.program.metadata.get(frame.block);
        let span = frames
            .first()
            .map(|frame| frame.span)
            .or_else(|| {
                metadata
                    .and_then(|metadata| metadata.spans.get(frame.pc.saturating_sub(1)))
                    .copied()
                    .flatten()
            })
            .unwrap_or(Span::new(0, 0));
        let member = frames
            .first()
            .map(|frame| frame.member.clone())
            .or_else(|| frame.diagnostic_member.clone())
            .or_else(|| metadata.map(|metadata| metadata.member.clone()))
            .unwrap_or_default();
        frame.type_run = Some(Box::new(TypeRun {
            node,
            value: value.clone(),
            checked: value,
            environment: frame.environment.clone(),
            member,
            tasks: vec![Task::Check { ty, span }],
            outcome: None,
            returned: None,
            destination,
            frames,
            in_type_test,
            eager,
            constraint_active: false,
            tracker: None,
            constraint_source: None,
            constraint_section: None,
        }));
        Ok(())
    }
    fn push_type_child(
        &mut self,
        run: &mut TypeRun,
        ty: TypeCheck,
        span: Span,
        index: usize,
    ) -> Result<(), RuntimeError> {
        if needs_type_identity(&ty) {
            let parent = run
                .node
                .ok_or_else(|| invalid("missing parent type node"))?;
            let child = TypeNodeId(
                self.heap
                    .executable_child(parent.0, ExecutionSite::TypeChild(index))?,
            );
            run.tasks.push(Task::RestoreTypeNode(run.node));
            run.node = Some(child);
        }
        run.tasks.push(Task::Check { ty, span });
        Ok(())
    }

    fn constraint_value(&self, environment: &Environment) -> Result<Value, RuntimeError> {
        let slot = environment
            .custom_this
            .ok_or_else(|| invalid("missing constraint receiver slot"))?;
        self.heap
            .custom_this(slot)?
            .ok_or_else(|| invalid("missing constraint receiver"))
    }
    fn launch_constraint(
        &mut self,
        mut run: Box<TypeRun>,
        constraint: ConstraintCode,
    ) -> Result<(), RuntimeError> {
        let mut environment = if constraint.module_environment {
            let mut environment = run.environment.clone();
            if let Some(unit) = self
                .program
                .metadata
                .get(constraint.block)
                .and_then(|metadata| metadata.unit)
                .and_then(|unit| self.program.units.get(unit.0 as usize))
                && unit.entry != usize::MAX
                && let Some(module) = self.heap.find_module(unit.id)
            {
                environment.scopes = crate::heap::ScopeList::from_single(Scope {
                    receiver: module,
                    owner: module,
                });
            }
            module_environment(&environment)?
        } else {
            run.environment.clone()
        };
        let shared_root = self
            .program
            .metadata
            .get(constraint.block)
            .is_some_and(|metadata| metadata.constraint_root.is_some());
        for (name, argument) in &constraint.arguments {
            if !shared_root {
                if let Some(source) = argument
                    .argument_source(&run.environment)
                    .or_else(|| constraint.argument_sources.get(name).cloned())
                {
                    environment
                        .type_argument_sources
                        .insert((constraint.block, name.clone()), source);
                } else {
                    environment
                        .type_argument_sources
                        .remove(&(constraint.block, name.clone()));
                }
            } else {
                environment
                    .type_argument_sources
                    .remove(&(constraint.block, name.clone()));
            }
            // VmTypeAlias.instantiate visits the cloned child tree, but a let
            // expression's call target is outside that tree. Its type variables
            // retain the alias declaration's unbound (unknown) interpretation.
            let argument = if shared_root {
                TypeCheck::Unknown
            } else {
                argument.substituted(&run.environment)?
            };
            environment
                .type_arguments
                .insert((constraint.block, name.clone()), argument.shared());
        }
        // Local-writing constraints execute in a fresh Java root frame. Inline
        // constraints use the checking frame's slot, including during replay.
        if self
            .program
            .metadata
            .get(constraint.block)
            .is_some_and(|metadata| metadata.constraint_root.is_some())
        {
            let value = self.constraint_value(&environment)?;
            environment.custom_this = Some(self.heap.allocate_custom_this(Some(value)));
        }
        let owner = if self
            .program
            .metadata
            .get(constraint.block)
            .is_some_and(|metadata| metadata.constraint_root.is_some())
        {
            self.executable_root(constraint.block)
        } else {
            let node = run
                .node
                .ok_or_else(|| invalid("missing constraint type node"))?;
            self.heap
                .executable_child(node.0, ExecutionSite::Constraint(constraint.block))?
        };
        if self
            .program
            .metadata
            .get(constraint.block)
            .is_some_and(|metadata| metadata.constraint_root.is_some())
        {
            self.copy_captured_locals(&mut environment, owner)?;
        }
        environment.executable_owner = Some(owner);
        let member = self
            .program
            .metadata
            .get(constraint.block)
            .filter(|metadata| metadata.constraint_root.is_some())
            .map_or_else(|| run.member.clone(), |metadata| metadata.member.clone());
        run.constraint_active = true;
        run.constraint_section = constraint.section.clone();
        run.constraint_source = self
            .program
            .metadata
            .get(constraint.block)
            .and_then(|metadata| metadata.source.clone());
        self.frames.last_mut().expect("type frame").type_run = Some(run);
        self.frames.last_mut().expect("type frame").callsite_hidden = true;
        let mut frame = self.alloc_frame(constraint.block, 0, self.stack.len(), environment);
        frame.return_to_type = true;
        frame.diagnostic_member = Some(member);
        self.frames.push(frame);
        Ok(())
    }
    fn launch_predicate(
        &mut self,
        mut run: Box<TypeRun>,
        function: ObjectId,
        argument: Value,
    ) -> Result<(), RuntimeError> {
        run.constraint_active = true;
        self.frames.last_mut().expect("type frame").type_run = Some(run);
        self.frames.last_mut().expect("type frame").callsite_hidden = true;
        self.invoke_function(function, vec![argument], false)?;
        self.frames
            .last_mut()
            .expect("predicate frame")
            .return_to_type = true;
        Ok(())
    }
    /// Result validation happens after the child frame has returned. Keep the
    /// checking continuation available to `fail`, including inherited value sites.
    fn fail_type_result(
        &mut self,
        run: Box<TypeRun>,
        error: Result<RuntimeError, RuntimeError>,
        authored_span: Option<Span>,
    ) -> Result<(), RuntimeError> {
        // Rendering failures are fatal too; propagate them with the same frames.
        let mut error = match error {
            Ok(error) | Err(error) => error,
        };
        if let Some(span) = authored_span {
            error = error.at(span);
            if let Some(declaration) = run.frames.first() {
                // Inherited checks execute at a source-less Return instruction.
                // Ordinary property checks already have this authored VM frame.
                error.frames.push(RuntimeFrame {
                    anonymous: false,
                    source: declaration.source.clone(),
                    span,
                    member: declaration.member.clone(),
                });
            }
        }
        self.frames.last_mut().expect("type frame").type_run = Some(run);
        Err(error)
    }
    pub(super) fn step_type(&mut self) -> Result<(), RuntimeError> {
        let mut run = self
            .frames
            .last_mut()
            .expect("type frame")
            .type_run
            .take()
            .expect("type run");
        let should_trace = self.power_assertions
            && (!run.in_type_test || self.active_tracker() || run.tracker.is_some());
        let Some(task) = run.tasks.pop() else {
            if run.tracker.take().is_some() {
                self.active_type_trackers = self.active_type_trackers.saturating_sub(1);
            }
            let outcome = run
                .outcome
                .take()
                .ok_or_else(|| invalid("missing type result"))?;
            match (outcome, run.destination) {
                (Ok(_), TypeDestination::Test | TypeDestination::ProtobufAlternative) => {
                    self.stack.push(Value::Bool(true))
                }
                (Err(_), TypeDestination::Test | TypeDestination::ProtobufAlternative) => {
                    self.stack.push(Value::Bool(false))
                }
                (Ok(value), TypeDestination::Stack) => self.stack.push(value),
                (Ok(value), TypeDestination::Local { depth, policy }) => {
                    self.bind_local(depth, policy, value)?
                }
                (Ok(value), TypeDestination::Return) => {
                    self.stack.push(value);
                    self.frames.last_mut().expect("type frame").pc -= 1;
                }
                (Err(mismatch), _) => {
                    let diagnostic =
                        mismatch.diagnostic(&self.heap, &self.program.module_name, 0)?;
                    let mut error = RuntimeError::new(ErrorKind::Type, {
                        let mut plain = &diagnostic;
                        while let TypeDiagnostic::AtSource { diagnostic, .. } = plain {
                            plain = diagnostic;
                        }
                        match plain {
                            TypeDiagnostic::Simple { message, .. } => message.clone(),
                            _ => "Type constraint violated.".into(),
                        }
                    });
                    error.span = Some(diagnostic.span());
                    error.frames = run.frames;
                    let mut alias_source = None;
                    let mut inner_mismatch = &mismatch;
                    let mut inserted_frames = Vec::new();
                    loop {
                        match inner_mismatch {
                            Mismatch::AliasSource { inner, source } => {
                                alias_source = Some(source.clone());
                                inner_mismatch = inner;
                            }
                            Mismatch::SourceSection { inner, .. } => inner_mismatch = inner,
                            Mismatch::InsertedFrame { inner, frame } => {
                                inserted_frames.push(frame.clone());
                                inner_mismatch = inner;
                            }
                            _ => break,
                        }
                    }
                    let constraint_source = match inner_mismatch {
                        Mismatch::Constraint { source, .. } => source.clone(),
                        _ => None,
                    };
                    if let Some(source) = alias_source.or(constraint_source.clone()) {
                        // Keep the constraint's source section, but qualify its
                        // member with the same logical root that supplied
                        // TypeRun.member. An explicit frame with no source
                        // belongs to the primary module, even when the active
                        // value body belongs to another source.
                        let caller = self.frames.last_mut().expect("type frame");
                        let owner_source = match error.frames.first() {
                            Some(frame) => frame.source.as_ref(),
                            None => self
                                .program
                                .metadata
                                .get(caller.block)
                                .and_then(|metadata| metadata.source.as_ref()),
                        };
                        let module = owner_source
                            .map_or(&self.program.module_name, |source| &source.module_name);
                        let source = if &source.module_name == module {
                            source
                        } else {
                            Arc::new(crate::value::RuntimeSource {
                                module_name: module.clone(),
                                ..source.as_ref().clone()
                            })
                        };
                        if let Some(frame) = error.frames.first_mut() {
                            frame.source = Some(source);
                        } else {
                            error.frames.push(RuntimeFrame {
                                anonymous: false,
                                source: Some(source),
                                span: diagnostic.span(),
                                member: run.member.clone(),
                            });
                        }
                        caller.callsite_hidden = true;
                    }
                    for frame in inserted_frames {
                        if error.frames.is_empty() {
                            error.frames.push(RuntimeFrame {
                                anonymous: false,
                                source: self
                                    .program
                                    .metadata
                                    .get(self.frames.last().expect("type frame").block)
                                    .and_then(|m| m.source.clone()),
                                span: diagnostic.span(),
                                member: run.member.clone(),
                            });
                        }
                        error.insert_frame(1, frame);
                        self.frames.last_mut().expect("type frame").callsite_hidden = true;
                    }
                    if let Some(section) = diagnostic.source_section() {
                        let caller = self.frames.last().expect("type frame");
                        let logical_source = match error.frames.first() {
                            Some(frame) => frame.source.as_ref(),
                            None => self
                                .program
                                .metadata
                                .get(caller.block)
                                .and_then(|metadata| metadata.source.as_ref()),
                        };
                        let module = logical_source
                            .map_or(&self.program.module_name, |source| &source.module_name);
                        // A primary section owns immutable diagnostic text, but
                        // its constraint block can carry a relocated frame URI.
                        // External argument sections take precedence; an outer
                        // alias source must not replace a primary argument.
                        let source = section
                            .external_source()
                            .or_else(|| constraint_source.clone())
                            .map(|source| {
                                if &source.module_name == module {
                                    source
                                } else {
                                    Arc::new(crate::value::RuntimeSource {
                                        module_name: module.clone(),
                                        ..source.as_ref().clone()
                                    })
                                }
                            });
                        if let Some(frame) = error.frames.first_mut() {
                            frame.source = source;
                        } else if let Some(source) = source {
                            error.frames.push(RuntimeFrame {
                                anonymous: false,
                                source: Some(source),
                                span: diagnostic.span(),
                                member: run.member.clone(),
                            });
                            self.frames.last_mut().expect("type frame").callsite_hidden = true;
                        }
                    }
                    if let Some(frame) = error.frames.first_mut() {
                        frame.span = diagnostic.span();
                    }
                    error.type_diagnostic = Some(Box::new(diagnostic));
                    return Err(error);
                }
            }
            return Ok(());
        };
        let task = if let Task::Check { ty, span } = task {
            if ty.has_declared() {
                let environment = run.environment.clone();
                run.tasks.push(Task::Check {
                    ty: ty.clone(),
                    span,
                });
                self.frames.last_mut().expect("type frame").type_run = Some(run);
                let Some(resolved) = self.resolve_runtime_type(&ty, &environment, false)? else {
                    return Ok(());
                };
                run = self
                    .frames
                    .last_mut()
                    .expect("type frame")
                    .type_run
                    .take()
                    .expect("retained type run");
                run.tasks.pop();
                Task::Check { ty: resolved, span }
            } else {
                Task::Check { ty, span }
            }
        } else {
            task
        };
        let task = if let Task::Check { ty, span } = task {
            if let Some(section) = ty.source_section() {
                let span = section.span();
                run.tasks.push(Task::SourceSection(section));
                Task::Check {
                    ty: ty.into_resolved(),
                    span,
                }
            } else {
                Task::Check { ty, span }
            }
        } else {
            task
        };
        match task {
            Task::SourceSection(section) => {
                let outcome = run
                    .outcome
                    .take()
                    .ok_or_else(|| invalid("missing source section outcome"))?;
                run.outcome = Some(outcome.map_err(|inner| Mismatch::SourceSection {
                    inner: Box::new(inner),
                    section,
                }));
            }
            Task::AliasSource(source) => {
                let outcome = run
                    .outcome
                    .take()
                    .ok_or_else(|| invalid("alias type outcome"))?;
                run.outcome = Some(outcome.map_err(|inner| Mismatch::AliasSource {
                    inner: Box::new(inner),
                    source,
                }));
            }
            Task::PairResult(mut state) => {
                let outcome = run
                    .outcome
                    .take()
                    .ok_or_else(|| invalid("Pair type child outcome"))?;
                match outcome {
                    Err(error) => {
                        run.value = state.value;
                        run.checked = state.checked;
                        run.outcome = Some(Err(error));
                    }
                    Ok(value) => {
                        state.components[state.index] = value;
                        if state.index == 0 {
                            state.index = 1;
                            run.value = state.components[1].clone();
                            run.checked = run.value.clone();
                            let (ty, span) = state.arguments[1].clone();
                            run.tasks.push(Task::PairResult(state));
                            self.push_type_child(&mut run, ty, span, 1)?;
                        } else {
                            let Value::Object(id) = state.value else {
                                return Err(invalid("Pair original value"));
                            };
                            let result = if run.eager
                                || self.heap.get(id)?.pair.as_deref() == Some(&state.components)
                            {
                                state.value.clone()
                            } else {
                                self.make_pair(
                                    state.components[0].clone(),
                                    state.components[1].clone(),
                                )
                            };
                            run.value = state.value;
                            run.checked = state.checked;
                            run.outcome = Some(Ok(result));
                        }
                    }
                }
            }
            Task::Check { ty, span } => match ty.into_resolved() {
                TypeCheck::VarArgs { span, .. } => {
                    let mut error = internal_varargs_error().at(span);
                    error.frames = run.frames;
                    if let Some(frame) = error.frames.first_mut() {
                        frame.span = span;
                    }
                    return Err(error);
                }
                TypeCheck::Pair { arguments }
                    if !arguments.is_empty()
                        && matches!(&run.value,Value::Object(id) if self.heap.get(*id)?.pair.is_some()) =>
                {
                    let Value::Object(id) = run.value else {
                        unreachable!()
                    };
                    let components = *self.heap.get(id)?.pair.clone().unwrap();
                    let state = PairState {
                        value: run.value.clone(),
                        checked: run.checked.clone(),
                        components,
                        arguments,
                        index: 0,
                    };
                    run.value = state.components[0].clone();
                    run.checked = run.value.clone();
                    let (ty, span) = state.arguments[0].clone();
                    run.tasks.push(Task::PairResult(state));
                    self.push_type_child(&mut run, ty, span, 0)?;
                }
                ty @ TypeCheck::Reference { .. } if matches!(&run.value,Value::Object(id) if self.heap.get(*id)?.reference.is_some()) =>
                {
                    let TypeCheck::Reference { domain, .. } = &ty else {
                        unreachable!()
                    };
                    let domain = *domain.clone();
                    let Value::Object(id) = run.value else {
                        unreachable!()
                    };
                    let reference = self.heap.get(id)?.reference.as_ref().expect("reference");
                    let value = std::mem::replace(&mut run.value, reference.domain.clone());
                    run.tasks
                        .push(Task::ReferenceDomainResult { value, ty, span });
                    self.push_type_child(&mut run, domain, span, 0)?;
                }
                TypeCheck::NamedAlias {
                    expanded,
                    default_origin,
                    ..
                } => {
                    let span = if let Some(origin) = default_origin
                        && let Some(source) = origin.source
                    {
                        run.tasks.push(Task::AliasSource(source));
                        origin.span
                    } else {
                        span
                    };
                    self.push_type_child(&mut run, *expanded, span, 0)?
                }
                TypeCheck::Alias(inner) | TypeCheck::GenericAlias(inner) => {
                    let environment = module_environment(&run.environment)?;
                    run.tasks
                        .push(Task::RestoreEnvironment(Box::new(std::mem::replace(
                            &mut run.environment,
                            environment,
                        ))));
                    self.push_type_child(&mut run, *inner, span, 0)?;
                }
                TypeCheck::Nullable(inner) if !run.value.is_null() => {
                    self.push_type_child(&mut run, *inner, span, 0)?
                }
                TypeCheck::Union {
                    types,
                    spans,
                    labels,
                    string_literals: false,
                    ..
                } if !types.is_empty() => {
                    let parent_test = run.in_type_test;
                    let parent_eager = run.eager;
                    run.eager |= union_eager(&types);
                    run.in_type_test = true;
                    run.tasks.push(Task::UnionResult(UnionState {
                        label_sources: types.iter().map(TypeCheck::source_section).collect(),
                        types: types.clone(),
                        spans: spans.clone(),
                        labels,
                        span,
                        index: 0,
                        failures: Vec::new(),
                        parent_test,
                        parent_eager,
                        replay: false,
                    }));
                    self.push_type_child(&mut run, types[0].clone(), spans[0], 0)?;
                }
                TypeCheck::CollectionBase { arguments }
                    if !arguments.is_empty()
                        && (TypeCheck::CollectionBase {
                            arguments: Vec::new(),
                        })
                        .accepts(&run.value, &self.heap) =>
                {
                    let Value::Object(id) = run.value else {
                        unreachable!()
                    };
                    let kind = self.heap.get(id)?.collection_kind;
                    // Keep the abstract type's executable node: its element is
                    // child zero regardless of the concrete value's kind.
                    run.tasks.push(Task::Check {
                        ty: TypeCheck::ParameterizedCollection { kind, arguments },
                        span,
                    });
                }
                TypeCheck::ParameterizedCollection { kind, arguments }
                    if TypeCheck::Collection(kind).accepts(&run.value, &self.heap) =>
                {
                    if kind == crate::heap::CollectionKind::List
                        && arguments.len() == 1
                        && try_fast_strict_list_check(&arguments[0].0, &run.value, &self.heap, 0)
                            == Some(true)
                    {
                        run.outcome = Some(Ok(run.value.clone()));
                        self.frames.last_mut().expect("type frame").type_run = Some(run);
                        return Ok(());
                    }
                    let Value::Object(id) = run.value else {
                        unreachable!()
                    };
                    let mut items = Vec::new();
                    if !arguments.iter().all(|(ty, _)| ty.noop())
                        && (matches!(
                            kind,
                            crate::heap::CollectionKind::List
                                | crate::heap::CollectionKind::Map
                                | crate::heap::CollectionKind::Set
                        ) || run.eager
                            || kind == crate::heap::CollectionKind::Mapping)
                    {
                        let mut current = Some(id);
                        while let Some(owner) = current {
                            let object = self.heap.get(owner)?;
                            let ordered;
                            let members = if kind == crate::heap::CollectionKind::Map
                                || object.list_elements.is_some()
                            {
                                ordered = collections::ordered_members(&self.heap, owner)?;
                                &ordered
                            } else {
                                &object.collection_members
                            };
                            for member in members {
                                if member.kind != crate::heap::CollectionMemberKind::Property {
                                    items.push(CollectionItem {
                                        owner,
                                        member: member.clone(),
                                    });
                                }
                            }
                            current = object.parent;
                        }
                    }
                    run.tasks.push(Task::CollectionNext(CollectionState {
                        value: run.value.clone(),
                        checked: run.checked.clone(),
                        kind,
                        arguments,
                        items,
                        index: 0,
                        eager: run.eager,
                        replacements: Vec::new(),
                    }));
                }
                TypeCheck::Constrained {
                    base,
                    base_span,
                    constraints,
                } => {
                    let parent_eager = std::mem::replace(&mut run.eager, false);
                    run.tasks.push(Task::AfterBase {
                        constraints,
                        parent_eager,
                    });
                    self.push_type_child(&mut run, *base, base_span, 0)?;
                }
                TypeCheck::NonNull => {
                    run.outcome = Some(if !run.value.is_null() {
                        Ok(run.value.clone())
                    } else {
                        // The pinned NonNullTypeAliasTypeNode reports the base
                        // alias constraint, with one synthetic false observation.
                        // Preserve that source inside any enclosing user alias.
                        let source = crate::stdlib_sources::base();
                        let declaration = "typealias NonNull = Any(";
                        let start = source.text.find(declaration).expect("pinned NonNull alias")
                            + declaration.len();
                        let end = start
                            + source
                                .text
                                .entire()
                                .range(start..)
                                .find("\n")
                                .expect("alias line")
                            - 1;
                        let predicate = Span::new(start, end);
                        let mut observations = Trace::new();
                        observations.insert(
                            predicate,
                            Observation {
                                value: ObservedValue::NativeLiteral("false".into()),
                                executions: 1,
                            },
                        );
                        Err(Mismatch::AliasSource {
                            source: source.clone(),
                            inner: Box::new(Mismatch::Constraint {
                                source: Some(source),
                                span: predicate,
                                value: run.value.clone(),
                                body_span: predicate,
                                observations: Some(observations),
                            }),
                        })
                    });
                }
                TypeCheck::IntegerAlias { name, min, max } => {
                    let accepted = matches!(&run.value, Value::Int(value) if (min..=max).contains(&value.get()));
                    run.outcome = Some(if accepted {
                        Ok(run.value.clone())
                    } else {
                        let source = crate::stdlib_sources::base();
                        let declaration = source
                            .text
                            .find(&format!("typealias {name} = Int("))
                            .expect("pinned integer alias");
                        let start = declaration + format!("typealias {name} = ").len();
                        let end = start + source.text.entire().range(start..).find("\n").unwrap();
                        let mismatch = if matches!(&run.value, Value::Int(_)) {
                            let predicate = Span::new(start + 4, end - 1);
                            let mut observations = Trace::new();
                            observations.insert(
                                predicate,
                                Observation {
                                    value: ObservedValue::NativeLiteral("false".into()),
                                    executions: 1,
                                },
                            );
                            Mismatch::Constraint {
                                source: Some(source.clone()),
                                span: predicate,
                                value: run.value.clone(),
                                body_span: predicate,
                                observations: Some(observations),
                            }
                        } else {
                            Mismatch::Simple {
                                span: Span::new(start, start + 3),
                                ty: TypeCheck::Int,
                                value: run.value.clone(),
                            }
                        };
                        Err(Mismatch::AliasSource {
                            source,
                            inner: Box::new(mismatch),
                        })
                    });
                }
                ty => {
                    run.outcome = Some(if ty.accepts(&run.value, &self.heap) {
                        Ok(run.value.clone())
                    } else {
                        Err(Mismatch::Simple {
                            span,
                            ty,
                            value: run.value.clone(),
                        })
                    });
                }
            },
            Task::ReferenceDomainResult { value, ty, span } => {
                let TypeCheck::Reference { referent, .. } = ty.resolved() else {
                    unreachable!()
                };
                let Value::Object(id) = value else {
                    unreachable!()
                };
                let reference = self.heap.get(id)?.reference.as_ref().expect("reference");
                let accepted = run.outcome.as_ref().is_some_and(Result::is_ok)
                    && reference::is_subtype(
                        &reference.referent,
                        &reference::normalize(referent, &self.program.module_name),
                        &self.heap,
                    );
                run.value = value.clone();
                run.checked = value.clone();
                run.outcome = Some(if accepted {
                    Ok(value)
                } else {
                    Err(Mismatch::Simple { span, ty, value })
                });
            }
            Task::CollectionNext(state) => {
                if state.index == state.items.len() {
                    self.finish_collection(&mut run, state)?;
                } else if matches!(
                    state.kind,
                    crate::heap::CollectionKind::Mapping | crate::heap::CollectionKind::Map
                ) && !state.arguments[0].0.noop()
                {
                    run.value = state.items[state.index].member.key.clone();
                    run.checked = run.value.clone();
                    run.eager = true;
                    let (ty, span) = state.arguments[0].clone();
                    run.tasks.push(Task::CollectionKeyResult(state));
                    self.push_type_child(&mut run, ty, span, 0)?;
                } else if state.kind != crate::heap::CollectionKind::Mapping || state.eager {
                    return self.collection_value(run, state);
                } else {
                    let mut state = state;
                    state.index += 1;
                    run.tasks.push(Task::CollectionNext(state));
                }
            }
            Task::CollectionKeyResult(mut state) => {
                run.eager = state.eager;
                if let Some(Err(mismatch)) = run.outcome.take() {
                    let item = &state.items[state.index];
                    let member = self
                        .heap
                        .get(item.owner)?
                        .members
                        .iter()
                        .find(|member| member.entry && member.name == item.member.name);
                    let frame = member
                        .and_then(|member| self.program.metadata.get(member.code))
                        .and_then(|metadata| {
                            metadata.declaration.map(|span| RuntimeFrame {
                                anonymous: false,
                                source: metadata.source.clone(),
                                span,
                                member: metadata.member.clone(),
                            })
                        });
                    run.outcome = Some(Err(if let Some(frame) = frame {
                        Mismatch::InsertedFrame {
                            inner: Box::new(mismatch),
                            frame,
                        }
                    } else {
                        mismatch
                    }));
                    restore_collection(&mut run, &state);
                } else if state.eager || state.kind == crate::heap::CollectionKind::Map {
                    return self.collection_value(run, state);
                } else {
                    state.index += 1;
                    run.tasks.push(Task::CollectionNext(state));
                }
            }
            Task::CollectionValueReady(state) => {
                self.frames.last_mut().expect("type frame").callsite_hidden = false;
                run.value = run
                    .returned
                    .take()
                    .ok_or_else(|| invalid("missing collection value"))?;
                run.checked = run.value.clone();
                let (ty, span) = state
                    .arguments
                    .last()
                    .expect("collection value type")
                    .clone();
                let argument = state.arguments.len() - 1;
                run.tasks.push(Task::CollectionValueResult(state));
                self.push_type_child(&mut run, ty, span, argument)?;
            }
            Task::CollectionValueResult(mut state) => {
                if let Some(Ok(value)) = &run.outcome {
                    if matches!(
                        state.kind,
                        crate::heap::CollectionKind::List | crate::heap::CollectionKind::Map
                    ) && *value != run.value
                    {
                        state
                            .replacements
                            .push((state.items[state.index].member.name.clone(), value.clone()));
                    }
                    state.index += 1;
                    restore_collection(&mut run, &state);
                    run.tasks.push(Task::CollectionNext(state));
                } else {
                    restore_collection(&mut run, &state);
                }
            }
            Task::RestoreEnvironment(environment) => run.environment = *environment,
            Task::RestoreTypeNode(node) => run.node = node,
            Task::AfterBase {
                constraints,
                parent_eager,
            } => {
                run.eager = parent_eager;
                if let Some(Ok(value)) = &run.outcome {
                    run.checked = value.clone();
                    let slot = run
                        .environment
                        .custom_this
                        .ok_or_else(|| invalid("missing constraint receiver slot"))?;
                    self.heap.set_custom_this(slot, Some(run.value.clone()))?;
                    run.tasks.push(Task::Constraints {
                        constraints,
                        index: 0,
                    });
                }
            }
            Task::Constraints { constraints, index } => {
                if index == constraints.len() {
                    run.outcome = Some(Ok(run.checked.clone()));
                } else {
                    let constraint = constraints[index].clone();
                    run.tasks
                        .push(Task::ConstraintResult { constraints, index });
                    return self.launch_constraint(run, constraint);
                }
            }
            Task::ConstraintResult { constraints, index } => {
                run.constraint_active = false;
                self.frames.last_mut().expect("type frame").callsite_hidden = false;
                let result = run
                    .returned
                    .take()
                    .ok_or_else(|| invalid("missing constraint return"))?;
                let constraint = constraints[index].clone();
                match result {
                    Value::Bool(true) => run.tasks.push(Task::Constraints {
                        constraints,
                        index: index + 1,
                    }),
                    Value::Bool(false) if should_trace => {
                        if run.tracker.is_none() {
                            self.active_type_trackers += 1;
                        }
                        run.tracker = Some(BTreeMap::new());
                        run.tasks.push(Task::TraceResult {
                            span: constraint.span,
                            body_span: constraint.span,
                            predicate_value: None,
                        });
                        return self.launch_constraint(run, constraint);
                    }
                    Value::Bool(false) => {
                        let value = self.constraint_value(&run.environment)?;
                        if run.mismatch(constraint.span, constraint.span, value) {
                            self.active_type_trackers = self.active_type_trackers.saturating_sub(1);
                        }
                    }
                    Value::Function(function) => {
                        let body_span = self
                            .heap
                            .get(function)?
                            .function
                            .as_ref()
                            .and_then(|function| self.program.metadata.get(function.block))
                            .and_then(|metadata| metadata.declaration.or(metadata.value_span))
                            .unwrap_or(constraint.span);
                        let value = self.constraint_value(&run.environment)?;
                        run.tasks.push(Task::PredicateResult {
                            constraints,
                            index,
                            body_span,
                            function,
                            value: value.clone(),
                        });
                        return self.launch_predicate(run, function, value);
                    }
                    value => {
                        let error = if value.is_null() {
                            Ok(RuntimeError::new(
                                ErrorKind::Type,
                                "Expected value of type `Boolean` or `Function`, but got `null`.",
                            ))
                        } else {
                            (|| {
                                let headline = PklString::join(
                                    &[
                                        "Expected value of type `Boolean` or `Function`, but got type `"
                                            .into(),
                                        crate::diagnostic_values::type_name(
                                            &value,
                                            &self.heap,
                                            &self.program.module_name,
                                        )?,
                                        "`.".into(),
                                    ],
                                    "",
                                );
                                let rendered = crate::diagnostic_values::render(
                                    &value,
                                    &self.heap,
                                    &self.program.module_name,
                                    80,
                                )?;
                                Ok(RuntimeError::with_program_values(
                                    ErrorKind::Type,
                                    headline,
                                    &[("Value", rendered)],
                                ))
                            })()
                        };
                        return self.fail_type_result(run, error, Some(constraint.span));
                    }
                }
            }
            Task::PredicateResult {
                constraints,
                index,
                body_span,
                function,
                value: predicate_value,
            } => {
                run.constraint_active = false;
                let result = run
                    .returned
                    .take()
                    .ok_or_else(|| invalid("missing predicate return"))?;
                match result {
                    Value::Bool(true) => {
                        self.frames.last_mut().expect("type frame").callsite_hidden = false;
                        run.tasks.push(Task::Constraints {
                            constraints,
                            index: index + 1,
                        });
                    }
                    Value::Bool(false) if should_trace => {
                        if run.tracker.is_none() {
                            self.active_type_trackers += 1;
                        }
                        run.tracker = Some(BTreeMap::new());
                        run.tasks.push(Task::TraceResult {
                            span: constraints[index].span,
                            body_span,
                            predicate_value: Some(predicate_value.clone()),
                        });
                        return self.launch_predicate(run, function, predicate_value);
                    }
                    Value::Bool(false) => {
                        self.frames.last_mut().expect("type frame").callsite_hidden = false;
                        if run.mismatch(constraints[index].span, body_span, predicate_value) {
                            self.active_type_trackers = self.active_type_trackers.saturating_sub(1);
                        }
                    }
                    value => {
                        let error = type_error(
                            &TypeCheck::Boolean,
                            &value,
                            &self.program.module_name,
                            &self.heap,
                        );
                        return self.fail_type_result(run, error, None);
                    }
                }
            }
            Task::TraceResult {
                span,
                body_span,
                predicate_value,
            } => {
                run.constraint_active = false;
                self.frames.last_mut().expect("type frame").callsite_hidden = false;
                run.returned
                    .take()
                    .ok_or_else(|| invalid("missing trace result"))?;
                let value = match predicate_value {
                    Some(value) => value,
                    None => self.constraint_value(&run.environment)?,
                };
                if run.mismatch(span, body_span, value) {
                    self.active_type_trackers = self.active_type_trackers.saturating_sub(1);
                }
            }
            Task::UnionResult(mut state) => {
                let outcome = run
                    .outcome
                    .take()
                    .ok_or_else(|| invalid("missing union alternative result"))?;
                if outcome.is_ok() && !state.replay {
                    run.in_type_test = state.parent_test;
                    run.eager = state.parent_eager;
                    run.outcome = Some(outcome);
                } else {
                    if let Err(mismatch) = outcome {
                        if state.replay {
                            state.failures[state.index] = (
                                state.labels[state.index],
                                state.label_sources[state.index].clone(),
                                mismatch,
                            );
                        } else {
                            state.failures.push((
                                state.labels[state.index],
                                state.label_sources[state.index].clone(),
                                mismatch,
                            ));
                        }
                    }
                    state.index += 1;
                    if state.index == state.types.len() {
                        run.in_type_test = state.parent_test;
                        if !state.replay
                            && self.power_assertions
                            && (!state.parent_test
                                || self.active_tracker()
                                || run.tracker.is_some())
                        {
                            state.index = 0;
                            state.replay = true;
                        } else {
                            run.eager = state.parent_eager;
                            run.outcome = Some(Err(Mismatch::Union {
                                span: state.span,
                                value: run.value.clone(),
                                children: state.failures,
                            }));
                            self.frames.last_mut().expect("type frame").type_run = Some(run);
                            return Ok(());
                        }
                    }
                    let ty = state.types[state.index].clone();
                    let span = state.spans[state.index];
                    let index = state.index;
                    run.tasks.push(Task::UnionResult(state));
                    self.push_type_child(&mut run, ty, span, index)?;
                }
            }
        }
        self.frames.last_mut().expect("type frame").type_run = Some(run);
        Ok(())
    }
}

fn restore_collection(run: &mut TypeRun, state: &CollectionState) {
    run.value = state.value.clone();
    run.checked = state.checked.clone();
    run.eager = state.eager;
}
fn union_eager(types: &[TypeCheck]) -> bool {
    let mut pending: Vec<_> = types.iter().rev().collect();
    let mut kinds = Vec::new();
    while let Some(ty) = pending.pop() {
        match ty.resolved() {
            TypeCheck::ParameterizedCollection { kind, .. } => {
                if kinds.contains(&Some(*kind)) {
                    return true;
                }
                kinds.push(Some(*kind));
            }
            TypeCheck::CollectionBase { arguments } if !arguments.is_empty() => {
                if kinds.contains(&None) {
                    return true;
                }
                kinds.push(None);
            }
            TypeCheck::GenericAlias(inner) => {
                if matches!(inner.unaliased(), TypeCheck::CollectionBase { arguments } if !arguments.is_empty())
                {
                    if kinds.contains(&None) {
                        return true;
                    }
                    kinds.push(None);
                }
                if let TypeCheck::ParameterizedCollection { kind, .. } = inner.unaliased() {
                    if kinds.contains(&Some(*kind)) {
                        return true;
                    }
                    kinds.push(Some(*kind));
                }
                pending.push(inner);
            }
            TypeCheck::Alias(inner)
            | TypeCheck::Nullable(inner)
            | TypeCheck::NamedAlias {
                expanded: inner, ..
            }
            | TypeCheck::Constrained { base: inner, .. } => pending.push(inner),
            TypeCheck::Union { types, .. } => pending.extend(types.iter().rev()),
            _ => {}
        }
    }
    false
}

impl Vm {
    fn collection_value(
        &mut self,
        run: Box<TypeRun>,
        state: CollectionState,
    ) -> Result<(), RuntimeError> {
        if state.arguments.last().expect("value type").0.noop() {
            let mut run = run;
            let mut state = state;
            state.index += 1;
            run.tasks.push(Task::CollectionNext(state));
            self.frames.last_mut().expect("type frame").type_run = Some(run);
            return Ok(());
        }
        let Value::Object(receiver) = state.value else {
            unreachable!()
        };
        let item = state.items[state.index].clone();
        let receiver_obj = self.heap.get(receiver)?;
        if let Some(elems) = &receiver_obj.list_elements {
            let idx = match &item.member.key {
                Value::Int(i) if i.get() >= 0 => i.get() as usize,
                _ => return Err(invalid("compact List element key")),
            };
            let value = elems
                .get(idx)
                .cloned()
                .ok_or_else(|| invalid("compact List element out of bounds"))?;
            let mut run = run;
            run.eager = state.eager;
            run.tasks.push(Task::CollectionValueReady(state));
            run.returned = Some(value);
            self.frames.last_mut().expect("type frame").type_run = Some(run);
            return Ok(());
        }
        if receiver_obj.parent.is_none()
            && item.owner == receiver
            && matches!(
                state.kind,
                crate::heap::CollectionKind::Map | crate::heap::CollectionKind::List
            )
            && !matches!(item.member.key, Value::Object(_))
            && let Some(LazyState::Completed(value)) = receiver_obj.cache.get(&CacheKey {
                owner: None,
                name: item.member.name.clone(),
                method: false,
                entry: true,
            })
        {
            let value = value.clone();
            let mut run = run;
            run.eager = state.eager;
            run.tasks.push(Task::CollectionValueReady(state));
            run.returned = Some(value);
            self.frames.last_mut().expect("type frame").type_run = Some(run);
            return Ok(());
        }
        let mut run = run;
        run.eager = state.eager || state.kind == crate::heap::CollectionKind::Set;
        run.tasks.push(Task::CollectionValueReady(state));
        self.frames.last_mut().expect("type frame").type_run = Some(run);
        let depth = self.frames.len();
        let stack_depth = self.stack.len();
        self.force_key(
            receiver,
            item.owner,
            item.member.name,
            false,
            true,
            false,
            true,
        )?;
        if self.frames.len() > depth {
            self.frames[depth - 1].callsite_hidden = true;
            self.frames
                .last_mut()
                .expect("collection value frame")
                .return_to_type = true;
        } else if self.stack.len() == stack_depth {
            self.frames
                .last_mut()
                .expect("type frame")
                .equality_run
                .as_mut()
                .ok_or_else(|| invalid("missing deferred cache read"))?
                .forward_to_type = true;
        } else {
            let value = self.pop()?;
            self.frames
                .last_mut()
                .expect("type frame")
                .type_run
                .as_mut()
                .expect("type run")
                .returned = Some(value);
        }
        Ok(())
    }
    fn finish_collection(
        &mut self,
        run: &mut TypeRun,
        state: CollectionState,
    ) -> Result<(), RuntimeError> {
        restore_collection(run, &state);
        let Value::Object(id) = state.value else {
            unreachable!()
        };
        let result = if state.eager || state.kind == crate::heap::CollectionKind::Set {
            Value::Object(id)
        } else if matches!(
            state.kind,
            crate::heap::CollectionKind::List | crate::heap::CollectionKind::Map
        ) {
            if state.replacements.is_empty() {
                Value::Object(id)
            } else {
                let mut object = self.heap.get(id)?.clone();
                object.structural_key = None;
                object.java_hash = None;
                if let Some(elems) = &mut object.list_elements {
                    for (name, value) in state.replacements {
                        if let Some(idx) = name
                            .as_str()
                            .and_then(|s| s.rsplit_once(':'))
                            .and_then(|(_, i)| i.parse::<usize>().ok())
                            && idx < elems.len()
                        {
                            elems[idx] = value;
                        }
                    }
                } else {
                    for (name, value) in state.replacements {
                        let key = CacheKey {
                            entry: true,
                            owner: None,
                            name,
                            method: false,
                        };
                        object.member_values.insert(key.clone(), value.clone());
                        object.complete_cache(key, value);
                    }
                }
                Value::Object(self.heap.allocate(object))
            }
        } else {
            let (ty, span) = state.arguments.last().expect("value type").clone();
            let parent = run
                .node
                .ok_or_else(|| invalid("missing collection type node"))?;
            let node = Some(TypeNodeId(self.heap.executable_child(
                parent.0,
                ExecutionSite::TypeChild(state.arguments.len() - 1),
            )?));
            if ty.noop()
                || self
                    .heap
                    .get(id)?
                    .collection_cast
                    .as_ref()
                    .is_some_and(|cast| {
                        // ConstrainedTypeNode is equivalent only to itself. Shared
                        // bytecode (especially let roots) is not type-node identity.
                        cast.ty.equivalent(&ty) && (!needs_custom_this(&ty) || cast.node == node)
                    })
            {
                Value::Object(id)
            } else {
                let object = self.heap.get(id)?;
                let mut environment = run.environment.clone();
                environment.locals.clear();
                environment.local_owners.clear();
                let wrapper = Object {
                    parent: Some(id),
                    environment: object.environment.clone(),
                    collection_kind: state.kind,
                    length: object.length,
                    collection_cast: Some(crate::heap::CollectionCast {
                        source: match run.frames.first() {
                            // An explicit None denotes the primary module source.
                            Some(frame) => frame.source.clone(),
                            None => self
                                .frames
                                .last()
                                .and_then(|frame| self.program.metadata.get(frame.block))
                                .and_then(|metadata| metadata.source.clone()),
                        },
                        node,
                        ty,
                        span,
                        member: run.member.clone(),
                        environment,
                    }),
                    ..Object::default()
                };
                Value::Object(self.heap.allocate(wrapper))
            }
        };
        run.outcome = Some(Ok(result));
        Ok(())
    }
}

#[cfg(test)]
mod source_identity_tests {
    use super::*;
    use crate::type_source::{TypeSourceOwner, TypeSourceSection};

    #[test]
    fn union_triviality_uses_section_identity_not_equal_source_text() {
        let section =
            || TypeSourceSection::new(TypeSourceOwner::Primary("Int".into()), Span::new(0, 3));
        let actual = section();
        let separate = section();
        let mismatch = Mismatch::SourceSection {
            section: actual.clone(),
            inner: Box::new(Mismatch::Simple {
                span: Span::new(0, 3),
                ty: TypeCheck::Int,
                value: Value::Bool(false),
            }),
        };
        assert_eq!(actual, separate);
        assert!(mismatch.is_trivial(Some(&actual), Span::new(0, 3)));
        assert!(!mismatch.is_trivial(Some(&separate), Span::new(0, 3)));
    }

    #[test]
    fn constrained_base_clears_eager_flag_so_nested_constraint_frame_survives() {
        let source = concat!(
            "class Box {\n",
            "  items: Listing<Int>(!toList().isEmpty)\n",
            "  value: Int(items.length > 0)\n",
            "}\n",
            "local function makeBox(xs: Listing<Int(this > 0)>): Box = new Box {\n",
            "  items = xs\n",
            "  value = 1\n",
            "}\n",
            "res = makeBox(new Listing { -1 }).value\n",
        );
        let parsed = crate::parser::parse_module(source).unwrap();
        let program = compile_module(&crate::resolver::resolve_module(&parsed).unwrap()).unwrap();
        let mut vm = Vm::new(program.clone());
        vm.start(program.entry, vec![]).unwrap();
        let module = loop {
            match vm.poll(100) {
                VmPoll::Ready(result) => break result.unwrap().as_object().unwrap(),
                VmPoll::Yielded => {}
                other => panic!("unexpected poll initializing module: {other:?}"),
            }
        };
        vm.start_member(module, "res").unwrap();
        let err = loop {
            match vm.poll(100) {
                VmPoll::Ready(Err(err)) => break err,
                VmPoll::Ready(Ok(val)) => panic!("expected constraint failure, got {val:?}"),
                VmPoll::Yielded => {}
                other => panic!("unexpected poll evaluating member: {other:?}"),
            }
        };
        assert_eq!(err.kind, ErrorKind::Type);
        let frames: Vec<_> = err
            .frames
            .iter()
            .map(|frame| source.get(frame.span.start..frame.span.end))
            .collect();
        assert!(
            frames.contains(&Some("toList()")),
            "expected toList() constraint frame in error stack, got {frames:?}"
        );
    }
}
