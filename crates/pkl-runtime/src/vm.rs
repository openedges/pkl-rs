//! Bounded, resumable bytecode candidate. This is an architecture experiment, not full Pkl.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use crate::heap::{
    CacheKey, Class, Environment, Function, Heap, HeapStats, LazyState, Member, Object, Scope,
};
use crate::source::Span;
use crate::value::{ErrorKind, ObjectId, RuntimeError, RuntimeFrame, Value};
pub mod collections;
pub(crate) mod equality;
mod typecheck;
use typecheck::{TypeDestination, TypeRun};

#[derive(Clone, Debug)]
pub enum Instruction {
    Constant(Value),
    Input(usize),
    LoadLocal(usize),
    StoreLocal(usize),
    PushLocal,
    PopLocal,
    Swap,
    BeginCollection {
        kind: crate::heap::CollectionKind,
        has_parent: bool,
        specialized: bool,
    },
    EndCollection,
    SetCollectionDefault {
        defined: bool,
    },
    AddCollectionProperty(Member),
    AddCollectionElement(Member),
    AddCollectionEntry {
        member: Member,
        check_index: bool,
        span: Span,
    },
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
    Subscript,
    SuperSubscript,
    EntryParent,
    MakeList {
        length: usize,
    },
    LoadThis,
    LoadCustomThis,
    LoadOuter,
    LoadModule,
    NonNull,
    Throw,
    ToString,
    Observe(Span),
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
    MakeClass {
        template: usize,
        class: Class,
        has_parent: bool,
    },
    MakeInstance {
        template: usize,
    },
    DefaultClassInstance,
    Undefined(Arc<str>),
    AbstractMember {
        name: Arc<str>,
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
    Return,
}

/// Type checks supported by the shared evaluator. Parametric collections use
/// resumable checks and lazy wrappers; structural/function types remain unsupported.
#[derive(Clone, Debug)]
pub enum TypeCheck {
    Any,
    Nothing,
    NonNull,
    Null,
    Boolean,
    Int,
    Float,
    Number,
    String,
    Bytes,
    Collection(crate::heap::CollectionKind),
    ParameterizedCollection {
        kind: crate::heap::CollectionKind,
        arguments: Vec<(TypeCheck, Span)>,
    },
    StringLiteral(Arc<str>),
    Class {
        id: usize,
        name: Arc<str>,
    },
    Nullable(Box<TypeCheck>),
    Alias(Box<TypeCheck>),
    /// Generic aliases themselves participate in upstream union eager classification.
    GenericAlias(Box<TypeCheck>),
    Union {
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
    span: Span,
    module_environment: bool,
}

impl TypeCheck {
    fn unaliased(&self) -> &Self {
        match self {
            Self::Alias(inner) | Self::GenericAlias(inner) => inner.unaliased(),
            _ => self,
        }
    }
    fn equivalent(&self, other: &Self) -> bool {
        let left = self.unaliased();
        let right = other.unaliased();
        match (left, right) {
            (Self::Nullable(a), Self::Nullable(b)) => a.equivalent(b),
            (Self::StringLiteral(a), Self::StringLiteral(b)) => a == b,
            (Self::Class { id: a, .. }, Self::Class { id: b, .. }) => a == b,
            (Self::Collection(a), Self::Collection(b)) => a == b,
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
                a == b
                    && aa.len() == ba.len()
                    && aa.iter().zip(ba).all(|((a, _), (b, _))| a.equivalent(b))
            }
            (
                Self::Union {
                    types: a,
                    default: ad,
                    ..
                },
                Self::Union {
                    types: b,
                    default: bd,
                    ..
                },
            ) => ad == bd && a.len() == b.len() && a.iter().zip(b).all(|(a, b)| a.equivalent(b)),
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
                a.equivalent(b)
                    && ac.len() == bc.len()
                    && ac.iter().zip(bc).all(|(a, b)| a.block == b.block)
            }
            (
                Self::Any
                | Self::Nothing
                | Self::NonNull
                | Self::Null
                | Self::Boolean
                | Self::Int
                | Self::Float
                | Self::Number
                | Self::String
                | Self::Bytes,
                _,
            ) => std::mem::discriminant(left) == std::mem::discriminant(right),
            _ => false,
        }
    }
    fn noop(&self) -> bool {
        match self {
            Self::Any => true,
            Self::Alias(inner) | Self::GenericAlias(inner) => inner.noop(),
            Self::Union { types, .. } => types.iter().all(Self::noop),
            _ => false,
        }
    }
    fn accepts(&self, value: &Value, heap: &Heap) -> bool {
        match self {
            Self::Any => true,
            Self::Nothing => false,
            Self::NonNull => !matches!(value, Value::Null),
            Self::Null => matches!(value, Value::Null),
            Self::Boolean => matches!(value, Value::Bool(_)),
            Self::Int => matches!(value, Value::Int(_)),
            Self::Float => matches!(value, Value::Float(_)),
            Self::Number => matches!(value, Value::Int(_) | Value::Float(_)),
            Self::String => matches!(value, Value::String(_)),
            Self::Bytes => matches!(value, Value::Bytes(_)),
            Self::Collection(kind) => {
                matches!(value, Value::Object(id) if heap.get(*id).is_ok_and(|object| object.collection_kind == *kind && !object.is_module) && collections::typed_class(heap, *id).is_ok_and(|class| class.is_none()))
            }
            Self::ParameterizedCollection { kind, .. } => {
                Self::Collection(*kind).accepts(value, heap)
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
            Self::Nullable(inner) => matches!(value, Value::Null) || inner.accepts(value, heap),
            Self::Alias(inner) | Self::GenericAlias(inner) => inner.accepts(value, heap),
            Self::Union { types, .. } => types.iter().any(|ty| ty.accepts(value, heap)),
            Self::Constrained { .. } => false,
        }
    }
}

/// Source metadata is parallel to bytecode, so diagnostics add no VM dispatches.
#[derive(Clone, Debug, Default)]
pub struct BlockMetadata {
    pub hidden: bool,
    pub spans: Vec<Option<Span>>,
    pub member: Arc<str>,
    pub declaration: Option<Span>,
    pub value_span: Option<Span>,
    pub property_typecheck: Option<usize>,
}

#[derive(Clone, Debug)]
pub struct Program {
    pub module_name: Arc<str>,
    pub module_template: Option<usize>,
    pub blocks: Vec<Vec<Instruction>>,
    pub metadata: Vec<BlockMetadata>,
    pub objects: Vec<Vec<Member>>,
    pub entry: usize,
}

impl Program {
    pub fn new(blocks: Vec<Vec<Instruction>>) -> Self {
        Self {
            module_name: Arc::from(""),
            module_template: None,
            metadata: vec![BlockMetadata::default(); blocks.len()],
            blocks,
            objects: Vec::new(),
            entry: 0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadRequest {
    pub id: u64,
    pub uri: Arc<str>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum VmPoll {
    Ready(Result<Value, RuntimeError>),
    Pending(ReadRequest),
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
    inherited_types: Vec<(TypeCheck, Span, Arc<str>, ObjectId)>,
    collection_casts: Vec<crate::heap::CollectionCast>,
    skip_typecheck: bool,
}

struct Frame {
    block: usize,
    pc: usize,
    base: usize,
    environment: Environment,
    force: Option<Force>,
    lambda: bool,
    type_run: Option<TypeRun>,
    equality_run: Option<equality::EqualityRun>,
    key_search: Option<equality::KeySearch>,
    return_to_type: bool,
    diagnostic_member: Option<Arc<str>>,
    callsite_hidden: bool,
    builders: Vec<ObjectId>,
}

enum State {
    Idle,
    Running,
    Waiting(ReadRequest),
    Finished(Result<Value, RuntimeError>),
    Closed,
}

struct SuspendedContext {
    frames: Vec<Frame>,
    stack: Vec<Value>,
    inputs: Vec<Value>,
    state: State,
}

pub struct Vm {
    program: Arc<Program>,
    heap: Heap,
    frames: Vec<Frame>,
    stack: Vec<Value>,
    inputs: Vec<Value>,
    state: State,
    suspended: Vec<SuspendedContext>,
    next_request: u64,
    instructions: u64,
    member_evaluations: u64,
    code_validation: Result<(), RuntimeError>,
    power_assertions: bool,
}

impl Vm {
    pub fn new(program: Arc<Program>) -> Self {
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
            heap: Heap::new(),
            frames: Vec::new(),
            stack: Vec::new(),
            inputs: Vec::new(),
            state: State::Idle,
            suspended: Vec::new(),
            next_request: 1,
            instructions: 0,
            member_evaluations: 0,
            code_validation,
            power_assertions: false,
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
        self.available()?;
        self.code_validation.clone()?;
        if entry >= self.program.blocks.len() {
            return Err(invalid("invalid entry block"));
        }
        for input in &inputs {
            if let Some(id) = input.heap_id() {
                self.heap.get(id)?;
            }
        }
        self.stack.clear();
        self.inputs = inputs;
        self.frames.push(Frame {
            block: entry,
            pc: 0,
            base: 0,
            environment: Environment::default(),
            force: None,
            lambda: false,
            type_run: None,
            equality_run: None,
            key_search: None,
            return_to_type: false,
            diagnostic_member: None,
            callsite_hidden: false,
            builders: Vec::new(),
        });
        self.state = State::Running;
        Ok(())
    }

    /// Enter a callback evaluation while the current context waits for Host I/O.
    /// The program, heap and successful member cache belong to the evaluator.
    pub(crate) fn reenter(
        &mut self,
        request: u64,
        entry: usize,
        inputs: Vec<Value>,
    ) -> Result<(), RuntimeError> {
        if !matches!(&self.state, State::Waiting(read) if read.id == request) {
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
        self.suspended.push(SuspendedContext {
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
            self.frames = context.frames;
            self.stack = context.stack;
            self.inputs = context.inputs;
            self.state = context.state;
        }
    }

    pub(crate) fn pending_requests(&self) -> Vec<u64> {
        self.suspended
            .iter()
            .map(|context| &context.state)
            .chain(std::iter::once(&self.state))
            .filter_map(|state| match state {
                State::Waiting(request) => Some(request.id),
                _ => None,
            })
            .collect()
    }

    pub fn start_member(&mut self, receiver: ObjectId, name: &str) -> Result<(), RuntimeError> {
        self.available()?;
        self.heap.get(receiver)?;
        self.inputs.clear();
        self.stack.clear();
        self.state = State::Running;
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
                State::Waiting(request) => return VmPoll::Pending(request.clone()),
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
            State::Waiting(request) => VmPoll::Pending(request.clone()),
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
            State::Waiting(request) if request.id == id => {}
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
                if let Some(object) = value.heap_id() {
                    self.heap.get(object)?;
                }
                self.stack.push(value);
                self.state = State::Running;
            }
            Err(error) => self.fail(error),
        }
        Ok(())
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
        self.stack = Vec::new();
        self.inputs = Vec::new();
        self.state = State::Closed;
        self.heap.release();
        self.program = Arc::new(Program::new(Vec::new()));
    }

    pub fn is_closed(&self) -> bool {
        matches!(self.state, State::Closed)
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
                roots.extend(frame.builders.iter().copied().map(Value::Object));
                roots.extend(frame.environment.locals.iter().cloned());
                roots.extend(frame.environment.this_value.iter().cloned());
                roots.extend(frame.environment.outer_value.iter().cloned());
                roots.extend(frame.environment.custom_this.iter().cloned());
                if let Some(run) = &frame.key_search {
                    run.roots(&mut roots);
                }
                if let Some(run) = &frame.equality_run {
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
        self.heap.collect(&roots)
    }

    fn fail(&mut self, mut error: RuntimeError) {
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
            let mut names = Vec::new();
            for scope in &frame.environment.scopes {
                if let Ok(owner) = self.heap.get(scope.owner) {
                    names.extend(
                        owner
                            .members
                            .iter()
                            .filter(|member| !member.method)
                            .map(|member| member.name.to_string()),
                    );
                }
                let mut current = Some(scope.receiver);
                let mut visited = BTreeSet::new();
                while let Some(id) = current {
                    if !visited.insert(id) {
                        break;
                    }
                    let Ok(object) = self.heap.get(id) else {
                        break;
                    };
                    names.extend(
                        object
                            .members
                            .iter()
                            .filter(|member| !member.method && !member.local)
                            .map(|member| member.name.to_string()),
                    );
                    current = object.parent;
                }
            }
            error.message = format!("Cannot find property `{name}`.");
            error.hint = crate::diagnostic::property_suggestions(name, names, true);
        }
        for frame in self.frames.iter().rev() {
            if let Some(metadata) = self.program.metadata.get(frame.block)
                && let Some(Some(span)) = metadata.spans.get(frame.pc.saturating_sub(1))
            {
                if !frame.callsite_hidden {
                    error.frames.push(RuntimeFrame {
                        span: if error.frames.is_empty() {
                            error.span.unwrap_or(*span)
                        } else {
                            *span
                        },
                        member: frame
                            .diagnostic_member
                            .clone()
                            .unwrap_or_else(|| metadata.member.clone()),
                    });
                }
                if frame.force.is_some()
                    && metadata.property_typecheck == Some(frame.pc.saturating_sub(1))
                    && matches!(
                        self.program.blocks[frame.block].get(frame.pc.saturating_sub(1)),
                        Some(Instruction::CheckType(_))
                    )
                    && let Some(value_span) = metadata.value_span
                {
                    error.frames.push(RuntimeFrame {
                        span: value_span,
                        member: metadata.member.clone(),
                    });
                }
            }
            if let Some(run) = &frame.type_run {
                error.frames.extend(run.fatal_value_frames().cloned());
            }
        }
        if error.span.is_none() {
            error.span = error.frames.first().map(|frame| frame.span);
        }
        for frame in &self.frames {
            if let Some(force) = &frame.force
                && let Some(key) = &force.cache
                && let Ok(object) = self.heap.get_mut(force.receiver)
            {
                // VmUtils.doReadMember and ReadLocalPropertyNode publish only successful
                // return values. A failed attempt must be retryable in this evaluator.
                object.cache.remove(key);
            }
        }
        self.frames.clear();
        self.stack.clear();
        self.inputs.clear();
        self.state = State::Finished(Err(error));
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
        let key = CacheKey {
            entry,
            owner: local.then_some(initial_owner),
            name: name.clone(),
            method,
        };
        if memoize {
            match self.heap.get(receiver)?.cache.get(&key) {
                Some(LazyState::Completed(value)) => {
                    self.stack.push(value.clone());
                    return Ok(());
                }
                Some(LazyState::Evaluating) => {
                    return Err(RuntimeError::new(
                        ErrorKind::Cycle,
                        format!("cyclic forcing of {name}"),
                    ));
                }
                _ => {}
            }
        }
        let mut owner = initial_owner;
        let mut visited = BTreeSet::new();
        let member = loop {
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
            owner = object.parent.ok_or_else(|| {
                RuntimeError::new(ErrorKind::MissingMember, format!("missing member {name}"))
            })?;
        };
        if self
            .frames
            .iter()
            .chain(self.suspended.iter().flat_map(|context| &context.frames))
            .any(|frame| {
                frame.force.as_ref().is_some_and(|force| {
                    force.entry == entry
                        && force.receiver == receiver
                        && force.owner == owner
                        && force.name == name
                        && force.method == method
                })
            })
        {
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
                    && let Some(LazyState::Completed(value)) = object.cache.get(&key)
                {
                    cached = Some(value.clone());
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
                later.span == earlier.span && later.ty.equivalent(&earlier.ty)
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
                    self.heap
                        .get_mut(receiver)?
                        .complete_cache(key, value.clone());
                }
                self.stack.push(value.clone());
                return Ok(());
            }
        } else if member.code >= self.program.blocks.len() {
            return Err(invalid("invalid member block"));
        }
        if self.frames.len() >= 4096 {
            return Err(RuntimeError::new(
                ErrorKind::Cycle,
                "experiment frame limit exceeded",
            ));
        }
        let mut inherited_types = Vec::new();
        if !local && !method {
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
            block,
            pc,
            base,
            environment,
            lambda: false,
            type_run: None,
            equality_run: None,
            key_search: None,
            return_to_type: false,
            diagnostic_member: None,
            callsite_hidden: false,
            builders: Vec::new(),
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
                ));
                return Err(error);
            }
            if local {
                break;
            }
            owner = object.parent;
        }
        Ok(())
    }

    fn validate_const_overrides(
        &self,
        parent: ObjectId,
        members: &[Member],
    ) -> Result<(), RuntimeError> {
        for member in members.iter().filter(|member| !member.local) {
            let mut current = Some(parent);
            let mut visited = BTreeSet::new();
            while let Some(id) = current {
                if !visited.insert(id) {
                    return Err(RuntimeError::new(ErrorKind::Cycle, "cyclic parent chain"));
                }
                let object = self.heap.get(id)?;
                if let Some(inherited) = object.members.iter().find(|inherited| {
                    !inherited.local
                        && inherited.method == member.method
                        && inherited.name == member.name
                }) {
                    if inherited.constant && !member.method {
                        return Err(RuntimeError::new(
                            ErrorKind::Type,
                            format!("Cannot assign to const property `{}`.", member.name),
                        ));
                    }
                    if inherited.constant && member.method && !member.constant {
                        return Err(RuntimeError::new(
                            ErrorKind::Type,
                            format!(
                                "Method `{}` must be const because it overrides a const method.",
                                member.name
                            ),
                        ));
                    }
                    break;
                }
                current = object.parent;
            }
        }
        Ok(())
    }

    fn validate_object_members(
        &self,
        parent: ObjectId,
        members: &[Member],
    ) -> Result<(), RuntimeError> {
        self.validate_const_overrides(parent, members)?;
        let mut ancestor = Some(parent);
        let mut declared = BTreeSet::new();
        let mut property_names = Vec::new();
        let mut class_name = None;
        let mut visited = BTreeSet::new();
        while let Some(id) = ancestor {
            if !visited.insert(id) {
                return Err(RuntimeError::new(ErrorKind::Cycle, "cyclic parent chain"));
            }
            let object = self.heap.get(id)?;
            if class_name.is_none() {
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
            for member in members.iter().filter(|member| !member.local) {
                if !declared.contains(&(member.name.clone(), member.method)) {
                    let mut error = RuntimeError::new(
                        ErrorKind::MissingMember,
                        format!(
                            "Cannot find property `{}` in object of type `{}#{class_name}`.",
                            member.name, self.program.module_name
                        ),
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
                        Some(format!(
                            "Available properties:\n{}",
                            if names.is_empty() {
                                "(none)".into()
                            } else {
                                names.join("\n")
                            }
                        ))
                    });
                    return Err(error);
                }
            }
        }
        Ok(())
    }

    fn invoke_function(
        &mut self,
        id: ObjectId,
        arguments: Vec<Value>,
        method: bool,
    ) -> Result<(), RuntimeError> {
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
        if function.arity != arity {
            let mut error = RuntimeError::new(
                ErrorKind::Type,
                format!(
                    "Expected {} function arguments but got {arity}.",
                    function.arity
                ),
            );
            if let Some(metadata) = self.program.metadata.get(function.block)
                && let Some(span) = metadata.declaration
            {
                error.frames.push(RuntimeFrame {
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
        let mut environment = object.environment.clone();
        environment.outer_value = environment.this_value.clone();
        if function.capture_custom_this {
            environment.this_value = environment.custom_this.clone();
        }
        environment.locals.extend(arguments);
        self.frames.push(Frame {
            block: function.block,
            pc: 0,
            base: self.stack.len(),
            environment,
            force: None,
            lambda: !function.method,
            type_run: None,
            equality_run: None,
            key_search: None,
            return_to_type: false,
            diagnostic_member: None,
            callsite_hidden: false,
            builders: Vec::new(),
        });
        Ok(())
    }

    fn step(&mut self) -> Result<(), RuntimeError> {
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
        match instruction {
            Instruction::Swap
            | Instruction::BeginCollection { .. }
            | Instruction::EndCollection
            | Instruction::SetCollectionDefault { .. }
            | Instruction::AddCollectionProperty(_)
            | Instruction::AddCollectionElement(_)
            | Instruction::AddCollectionEntry { .. }
            | Instruction::OpenIterator { .. }
            | Instruction::IteratorNext { .. }
            | Instruction::SpreadMember
            | Instruction::OpenPredicate
            | Instruction::PredicateNext { .. }
            | Instruction::AddPredicateMember(_)
            | Instruction::Subscript
            | Instruction::SuperSubscript
            | Instruction::EntryParent
            | Instruction::MakeList { .. } => self.collection_instruction(instruction)?,
            Instruction::Constant(value) => self.stack.push(value),
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
                self.frames
                    .last_mut()
                    .expect("active frame")
                    .environment
                    .locals
                    .push(value);
            }
            Instruction::PopLocal => {
                if frame.environment.locals.pop().is_none() {
                    return Err(invalid("lexical stack underflow"));
                }
            }
            Instruction::LoadThis => self.stack.push(
                frame
                    .environment
                    .this_value
                    .clone()
                    .unwrap_or(Value::Object(self.scope(0)?.receiver)),
            ),
            Instruction::LoadCustomThis => self.stack.push(
                frame
                    .environment
                    .custom_this
                    .clone()
                    .ok_or_else(|| invalid("missing custom receiver"))?,
            ),
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
                if matches!(value, Value::Null) {
                    return Err(RuntimeError::new(
                        ErrorKind::Type,
                        "Expected a non-null value, but got `null`.",
                    ));
                }
                self.stack.push(value);
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
                return Err(RuntimeError::new(ErrorKind::Type, message.as_ref()));
            }
            Instruction::ToString => {
                let value = self.pop()?;
                self.stack.push(Value::string(scalar_string(&value)?));
            }
            Instruction::TestType(ty) => {
                let value = self.pop()?;
                self.begin_type(ty, value, TypeDestination::Test, Vec::new())?;
            }
            Instruction::JumpIfNull(target) => {
                if matches!(self.pop()?, Value::Null) {
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
                self.require_const(
                    if local { scope.owner } else { scope.receiver },
                    &name,
                    local,
                    method,
                )?;
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
                if matches!(receiver, Value::Function(_)) && method && name.as_ref() == "apply" {
                    self.stack.push(receiver);
                } else {
                    let receiver = receiver.as_object()?;
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
                self.force(
                    scope.receiver,
                    if local { scope.owner } else { scope.receiver },
                    name.clone(),
                    local,
                    true,
                    method,
                )?;
            }
            Instruction::GetImplicitProperty(ref name)
            | Instruction::GetImplicitMethod(ref name) => {
                let method = matches!(instruction, Instruction::GetImplicitMethod(_));
                let receiver = self
                    .frames
                    .last()
                    .expect("active frame")
                    .environment
                    .this_value
                    .clone()
                    .unwrap_or(Value::Object(self.scope(0)?.receiver));
                let receiver = receiver.as_object()?;
                if method || !self.collection_builtin(receiver, name)? {
                    self.force(receiver, receiver, name.clone(), false, true, method)?;
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
                let parent = self.heap.get(scope.owner)?.parent.ok_or_else(|| {
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
            Instruction::ResolveClassType { name, local } => {
                let frame = self.frames.last().expect("active frame");
                let module = match frame.environment.scopes.last() {
                    Some(scope) => scope.receiver,
                    None => self
                        .stack
                        .last()
                        .ok_or_else(|| invalid("missing module for type resolution"))?
                        .as_object()?,
                };
                // ClassNode publishes a partially initialized singleton before resolving
                // property types. A type-only back edge is therefore valid.
                if self.frames.iter().any(|frame| {
                    frame.force.as_ref().is_some_and(|force| {
                        force.receiver == module && force.name == name && !force.method
                    })
                }) {
                    self.stack.push(Value::Null);
                } else {
                    self.force(module, module, name, local, true, false)?;
                }
            }
            Instruction::Undefined(name) => {
                return Err(RuntimeError::new(
                    ErrorKind::MissingMember,
                    format!("Tried to read property `{name}` but its value is undefined."),
                ));
            }
            Instruction::AbstractMember { name, method } => {
                let kind = if method { "method" } else { "property" };
                return Err(RuntimeError::new(
                    ErrorKind::Type,
                    format!(
                        "Cannot invoke abstract {kind} `{}{name}`.",
                        self.program.module_name
                    ),
                ));
            }
            Instruction::Unsupported(message) => {
                return Err(RuntimeError::new(ErrorKind::Unsupported, message.as_ref()));
            }
            Instruction::MakeClass {
                template,
                class,
                has_parent,
            } => {
                let parent = if has_parent {
                    let Value::Class(id) = self.pop()? else {
                        return Err(RuntimeError::new(
                            ErrorKind::Type,
                            "Superclass must be a class.",
                        ));
                    };
                    let parent_class = self
                        .heap
                        .get(id)?
                        .class_definition
                        .as_ref()
                        .ok_or_else(|| invalid("invalid class handle"))?;
                    if !parent_class.open && !parent_class.abstract_ {
                        let mut error = RuntimeError::new(
                            ErrorKind::Type,
                            format!(
                                "Cannot extend non-open class `{}#{}`.",
                                self.program.module_name, parent_class.name
                            ),
                        );
                        error.hint = Some("By default, classes are closed for extension.\nTo make a class extensible, add an `open` modifier: `open class MyClass { ... }`".into());
                        return Err(error);
                    }
                    Some(id)
                } else {
                    None
                };
                let members = self
                    .program
                    .objects
                    .get(template)
                    .ok_or_else(|| invalid("invalid class template"))?
                    .clone();
                if let Some(parent) = parent {
                    self.validate_const_overrides(parent, &members)?;
                }
                let environment = self
                    .frames
                    .last()
                    .expect("active frame")
                    .environment
                    .clone();
                let id = self.heap.allocate(Object {
                    class_definition: Some(class),
                    parent,
                    members,
                    environment,
                    ..Object::default()
                });
                self.stack.push(Value::Class(id));
            }
            Instruction::MakeInstance { template } => {
                let Value::Class(parent) = self.pop()? else {
                    return Err(RuntimeError::new(ErrorKind::Type, "Expected a class."));
                };
                let class = self
                    .heap
                    .get(parent)?
                    .class_definition
                    .as_ref()
                    .ok_or_else(|| invalid("invalid class handle"))?;
                if class.abstract_ {
                    let mut error = RuntimeError::new(
                        ErrorKind::Type,
                        format!(
                            "Cannot instantiate abstract class `{}#{}`.",
                            self.program.module_name, class.name
                        ),
                    );
                    error.hint = Some("Abstract classes cannot be instantiated.\nInstead, instantiate a concrete subclass.".into());
                    return Err(error);
                }
                let members = self
                    .program
                    .objects
                    .get(template)
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
                let parent = if has_parent {
                    Some(self.pop()?.as_object()?)
                } else {
                    None
                };
                if let Some(id) = parent {
                    self.heap.get(id)?;
                }
                let members = self
                    .program
                    .objects
                    .get(template)
                    .ok_or_else(|| invalid("invalid object template"))?
                    .clone();
                if let Some(parent) = parent {
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
                let object = self.heap.allocate(Object {
                    is_module: self.program.module_template == Some(template),
                    parent,
                    members,
                    environment,
                    ..Object::default()
                });
                self.stack.push(Value::Object(object));
            }
            Instruction::MakeFunction {
                block,
                arity,
                method,
                capture_custom_this,
            } => {
                if block >= self.program.blocks.len() {
                    return Err(invalid("invalid function block"));
                }
                let environment = self
                    .frames
                    .last()
                    .expect("active frame")
                    .environment
                    .clone();
                let id = self.heap.allocate(Object {
                    function: Some(Function {
                        block,
                        arity,
                        method,
                        capture_custom_this,
                    }),
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
                        format!(
                            "Operator `|>` is not defined for operand types `{}` and `{}`.\nLeft operand : {}\nRight operand: {}",
                            crate::diagnostic_values::type_name(
                                &left,
                                &self.heap,
                                &self.program.module_name
                            )?,
                            crate::diagnostic_values::type_name(
                                &right,
                                &self.heap,
                                &self.program.module_name
                            )?,
                            crate::diagnostic_values::render(
                                &left,
                                &self.heap,
                                &self.program.module_name,
                                80
                            )?,
                            crate::diagnostic_values::render(
                                &right,
                                &self.heap,
                                &self.program.module_name,
                                80
                            )?,
                        ),
                    ));
                };
                self.frames
                    .last_mut()
                    .expect("active frame")
                    .callsite_hidden = true;
                self.invoke_function(id, vec![left], false)?;
            }
            Instruction::CheckType(ty) => {
                let frame = self.frames.last().expect("active frame");
                if self
                    .program
                    .metadata
                    .get(frame.block)
                    .is_some_and(|metadata| {
                        metadata.property_typecheck == Some(frame.pc.saturating_sub(1))
                    })
                    && frame
                        .force
                        .as_ref()
                        .is_some_and(|force| force.skip_typecheck)
                {
                    return Ok(());
                }
                let value = self.pop()?;
                self.begin_type(ty, value, TypeDestination::Stack, Vec::new())?;
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
                    Value::Int(value) => {
                        Value::Int(crate::numeric::subtract(0, value).map_err(numeric_error)?)
                    }
                    Value::Float(value) => Value::Float(-value),
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
                    return Err(
                        if matches!(instruction, Instruction::CheckLogicalLeft { .. }) {
                            RuntimeError::new(
                                ErrorKind::Type,
                                format!(
                                    "Operator `{operator}` is not defined for left operand type `{}`.\nLeft operand: {}",
                                    value_type_name(&value),
                                    display_value(&value)
                                ),
                            )
                        } else {
                            binary_error(operator, &Value::Bool(!or), &value)
                        },
                    );
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
                if !matches!(self.pop()?, Value::Null) {
                    self.jump(target)?;
                }
            }
            Instruction::AddIntConstant(right) => {
                let left = self.pop()?;
                let value = match left {
                    Value::Int(left) => {
                        Value::Int(crate::numeric::add(left, right).map_err(numeric_error)?)
                    }
                    Value::Float(left) => Value::Float(left + right as f64),
                    left => return Err(binary_error("+", &left, &Value::Int(right))),
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
            Instruction::ReadResource => {
                let Value::String(uri) = self.pop()? else {
                    return Err(RuntimeError::new(
                        ErrorKind::Type,
                        "resource URI must be a String",
                    ));
                };
                let id = self.next_request;
                self.next_request = id
                    .checked_add(1)
                    .ok_or_else(|| invalid("Host request IDs exhausted"))?;
                self.state = State::Waiting(ReadRequest { id, uri });
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
                    let mut frames = vec![RuntimeFrame { span, member }];
                    let frame = self.frames.last().expect("active frame");
                    if let Some(metadata) = self.program.metadata.get(frame.block)
                        && let Some(span) = metadata.value_span
                    {
                        frames.push(RuntimeFrame {
                            span,
                            member: metadata.member.clone(),
                        });
                    }
                    self.begin_type(ty, value, TypeDestination::Return, frames)?;
                    let mut environment = self.heap.get(owner)?.environment.clone();
                    environment.outer_value = environment.this_value.clone();
                    environment.this_value = Some(Value::Object(receiver));
                    environment.scopes.insert(0, Scope { receiver, owner });
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
                        span: cast.span,
                        member: cast.member,
                    }];
                    let frame = self.frames.last().expect("active frame");
                    if let Some(metadata) = self.program.metadata.get(frame.block)
                        && let Some(span) = metadata.value_span
                    {
                        frames.push(RuntimeFrame {
                            span,
                            member: metadata.member.clone(),
                        });
                    }
                    self.begin_type(cast.ty, value, TypeDestination::Return, frames)?;
                    self.frames
                        .last_mut()
                        .expect("type frame")
                        .type_run
                        .as_mut()
                        .expect("type run")
                        .environment = cast.environment;
                    return Ok(());
                }
                let frame = self.frames.pop().expect("active frame");
                if let Some(force) = frame.force
                    && let Some(key) = force.cache
                {
                    self.heap
                        .get_mut(force.receiver)?
                        .complete_cache(key, value.clone());
                }
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
            (Value::Int(a), Value::Float(b)) => *a as f64 == *b,
            (Value::Float(a), Value::Int(b)) => *a == *b as f64,
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
            Instruction::Add => {
                Value::Int(crate::numeric::add(left, right).map_err(numeric_error)?)
            }
            Instruction::Subtract => {
                Value::Int(crate::numeric::subtract(left, right).map_err(numeric_error)?)
            }
            Instruction::Multiply => {
                Value::Int(crate::numeric::multiply(left, right).map_err(numeric_error)?)
            }
            Instruction::Divide => Value::Float(left as f64 / right as f64),
            Instruction::Less => Value::Bool(left < right),
            Instruction::LessEqual => Value::Bool(left <= right),
            Instruction::Greater => Value::Bool(left > right),
            Instruction::GreaterEqual => Value::Bool(left >= right),
            Instruction::IntegerDivide => {
                Value::Int(crate::numeric::divide(left, right).map_err(numeric_error)?)
            }
            Instruction::Remainder => {
                Value::Int(crate::numeric::remainder(left, right).map_err(numeric_error)?)
            }
            Instruction::Power if right >= 0 => Value::Int(integer_power(left, right as u64)?),
            Instruction::Power => Value::Float(float_power(left as f64, right as f64)),
            _ => return Err(invalid("invalid binary instruction")),
        }),
        (Value::String(left), Value::String(right)) if matches!(instruction, Instruction::Add) => {
            Ok(Value::string(format!("{left}{right}")))
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
        (Value::Int(left), Value::Float(right)) => float_binary(instruction, left as f64, right),
        (Value::Float(left), Value::Int(right)) => float_binary(instruction, left, right as f64),
        (Value::Float(left), Value::Float(right)) => float_binary(instruction, left, right),
        (left, right) => Err(binary_error(operator_name(instruction), &left, &right)),
    }
}

fn float_binary(instruction: &Instruction, left: f64, right: f64) -> Result<Value, RuntimeError> {
    Ok(match instruction {
        Instruction::Add => Value::Float(left + right),
        Instruction::Subtract => Value::Float(left - right),
        Instruction::Multiply => Value::Float(left * right),
        Instruction::Divide => Value::Float(left / right),
        Instruction::Less => Value::Bool(left < right),
        Instruction::LessEqual => Value::Bool(left <= right),
        Instruction::Greater => Value::Bool(left > right),
        Instruction::GreaterEqual => Value::Bool(left >= right),
        Instruction::Remainder => Value::Float(left % right),
        Instruction::Power => Value::Float(float_power(left, right)),
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
            Value::Int(result as i64)
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
    if let Some(declaration) = &resolved.module.declaration
        && (declaration.parent.is_some() || !declaration.modifiers.is_empty())
    {
        return Err(unsupported_type("module inheritance and modifiers").at(declaration.span));
    }
    let mut compiler = Compiler {
        resolved,
        program: Program::new(Vec::new()),
        options,
        member: String::new(),
        type_scopes: Vec::new(),
        modifiers: Vec::new(),
        constraint_module: false,
        active_type_parameters: BTreeMap::new(),
        parameter_spans: BTreeMap::new(),
        function_counts: BTreeMap::new(),
        entry_counts: BTreeMap::new(),
        function_names: BTreeMap::new(),
        instrument,
    };
    compiler.program.module_name = Arc::from(
        resolved
            .module
            .declaration
            .as_ref()
            .and_then(|declaration| declaration.name.as_ref())
            .map(|parts| parts.join("."))
            .unwrap_or_default(),
    );
    let template = compiler.object(&resolved.module.members)?;
    compiler.program.module_template = Some(template);
    let mut entry = Code::default();
    entry.push(Instruction::MakeObject {
        template,
        has_parent: false,
    });
    compiler.type_scopes.push(resolved.module.members.clone());
    compiler.initialize_member_types(&resolved.module.members, &mut entry)?;
    compiler.type_scopes.pop();
    entry.push(Instruction::Return);
    compiler.program.entry = compiler.block(entry, None);
    Ok(Arc::new(compiler.program))
}

#[derive(Default)]
struct Code {
    instructions: Vec<Instruction>,
    spans: Vec<Option<Span>>,
}
impl Code {
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
    program: Program,
    options: CompileOptions,
    member: String,
    type_scopes: Vec<Vec<crate::syntax::Member>>,
    modifiers: Vec<crate::syntax::Modifier>,
    constraint_module: bool,
    active_type_parameters: BTreeMap<String, TypeCheck>,
    parameter_spans: BTreeMap<String, Span>,
    function_counts: BTreeMap<String, usize>,
    entry_counts: BTreeMap<String, usize>,
    function_names: BTreeMap<Span, String>,
    instrument: bool,
}

impl Compiler<'_> {
    fn object(&mut self, members: &[crate::syntax::Member]) -> Result<usize, RuntimeError> {
        self.type_scopes.push(members.to_vec());
        let mut result = Vec::new();
        for member in members {
            let previous_modifiers =
                std::mem::replace(&mut self.modifiers, member.modifiers.clone());
            self.validate_modifiers(member)?;
            let previous_member = self.member.clone();
            self.member = if previous_member.is_empty() {
                format!("#{}", member.name)
            } else {
                format!("{previous_member}.{}", member.name)
            };
            let mut instructions = Code::default();
            if matches!(member.expr.kind, crate::syntax::ExprKind::Uninitialized) {
                self.default_property(member, &mut instructions)?;
            } else if let crate::syntax::ExprKind::Object {
                parent: None,
                members,
            } = &member.expr.kind
                && let Some(ty) = &member.ty
                && matches!(self.compile_type(ty)?.unaliased(), TypeCheck::Class { .. })
            {
                self.class_expression(
                    ty,
                    &mut instructions,
                    self.type_scopes.len(),
                    &mut BTreeSet::new(),
                )?;
                self.instance_body(members, ty.span, &mut instructions)?;
            } else if let crate::syntax::ExprKind::Object {
                parent: None,
                members,
            } = &member.expr.kind
                && let Some(ty) = &member.ty
                && let TypeCheck::Collection(kind) | TypeCheck::ParameterizedCollection { kind, .. } =
                    self.compile_type(ty)?.unaliased()
            {
                let kind = *kind;
                let check = self.compile_type(ty)?;
                let has_default =
                    matches!(check.unaliased(), TypeCheck::ParameterizedCollection { .. });
                if has_default {
                    self.default_type(&check, &mut instructions, ty.span)?;
                }
                self.collection_body(members, kind, has_default, &mut instructions)?;
            } else {
                self.expression(&member.expr, &mut instructions)?;
            }
            let property_typecheck = member.ty.as_ref().map(|_| instructions.len());
            let compiled_type = member
                .ty
                .as_ref()
                .map(|ty| self.compile_type(ty))
                .transpose()?;
            if let Some(ty) = &member.ty {
                instructions.push_at(
                    Instruction::CheckType(compiled_type.clone().expect("member annotation")),
                    self.type_source_span(ty),
                );
            }
            instructions.push(Instruction::Return);
            let code = self.program.blocks.len();
            let header_span = Span::new(
                member.span.start,
                member
                    .ty
                    .as_ref()
                    .map_or(member.name_span.end, |ty| ty.span.end),
            );
            self.block(instructions, Some(header_span));
            self.program.metadata[code].hidden = member
                .modifiers
                .iter()
                .any(|m| m.kind == crate::syntax::ModifierKind::Hidden);
            self.program.metadata[code].value_span = Some(member.expr.span);
            self.program.metadata[code].property_typecheck = property_typecheck;
            self.member = previous_member;
            self.modifiers = previous_modifiers;
            result.push(Member {
                entry: false,
                name: Arc::from(member.name.as_str()),
                code,
                local: crate::syntax::modifier_flags(&member.modifiers)
                    & crate::syntax::ModifierKind::Local.flag()
                    != 0,
                ty: compiled_type,
                type_span: member.ty.as_ref().map(|ty| self.type_source_span(ty)),
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
            });
        }
        let index = self.program.objects.len();
        self.program.objects.push(result);
        self.type_scopes.pop();
        Ok(index)
    }

    fn block(&mut self, code: Code, declaration: Option<Span>) -> usize {
        let index = self.program.blocks.len();
        self.program.blocks.push(code.instructions);
        self.program.metadata.push(BlockMetadata {
            hidden: false,
            spans: code.spans,
            member: Arc::from(self.member.as_str()),
            declaration,
            value_span: None,
            property_typecheck: None,
        });
        index
    }

    fn expression(
        &mut self,
        expression: &crate::syntax::Expr,
        output: &mut Code,
    ) -> Result<(), RuntimeError> {
        let start = output.len();
        self.inner_expression(expression, output)
            .map_err(|error| error.at(expression.span))?;
        for span in &mut output.spans[start..] {
            if span.is_none() {
                *span = Some(expression.span);
            }
        }
        if self.instrument && !matches!(expression.kind, crate::syntax::ExprKind::Parenthesized(_))
        {
            output.push_at(Instruction::Observe(expression.span), expression.span);
        }
        Ok(())
    }

    fn inner_expression(
        &mut self,
        expression: &crate::syntax::Expr,
        output: &mut Code,
    ) -> Result<(), RuntimeError> {
        use crate::resolver::Binding;
        use crate::syntax::{BinaryOp, ExprKind};
        match &expression.kind {
            ExprKind::Parenthesized(inner) => self.expression(inner, output)?,
            ExprKind::Class {
                type_parameters,
                superclass,
                members,
                ..
            } => {
                if !type_parameters.is_empty() {
                    return Err(unsupported_type("generic classes"));
                }
                if let Some(parent) = superclass
                    && let Err(error) = self.class_expression(
                        parent,
                        output,
                        self.type_scopes.len(),
                        &mut BTreeSet::new(),
                    )
                {
                    output.push_at(Instruction::Fail(error.at(parent.span)), parent.span);
                    return Ok(());
                }
                let class = Class {
                    id: expression.span.start,
                    name: Arc::from(self.member.trim_start_matches('#')),
                    open: crate::syntax::modifier_flags(&self.modifiers)
                        & crate::syntax::ModifierKind::Open.flag()
                        != 0,
                    abstract_: crate::syntax::modifier_flags(&self.modifiers)
                        & crate::syntax::ModifierKind::Abstract.flag()
                        != 0,
                };
                // Class definitions are forced lazily, including their member type resolution.
                let checkpoint = (
                    self.program.blocks.len(),
                    self.program.metadata.len(),
                    self.program.objects.len(),
                    self.type_scopes.len(),
                    self.member.clone(),
                    self.modifiers.clone(),
                );
                let template = match self.object(members) {
                    Ok(template) => template,
                    Err(error) => {
                        self.program.blocks.truncate(checkpoint.0);
                        self.program.metadata.truncate(checkpoint.1);
                        self.program.objects.truncate(checkpoint.2);
                        self.type_scopes.truncate(checkpoint.3);
                        self.member = checkpoint.4;
                        self.modifiers = checkpoint.5;
                        let span = error.span.unwrap_or(expression.span);
                        output.push_at(Instruction::Fail(error.at(span)), span);
                        return Ok(());
                    }
                };
                output.push_at(
                    Instruction::MakeClass {
                        template,
                        class,
                        has_parent: superclass.is_some(),
                    },
                    superclass.as_ref().map_or(expression.span, |ty| ty.span),
                );
                self.initialize_member_types(members, output)?;
            }
            ExprKind::TypeAlias { .. } => {
                // Declarations are lazy. Expand only when a type annotation or constructor uses the alias.
                output.push(Instruction::Unsupported(Arc::from(
                    "type alias values and reflection are not implemented",
                )));
            }
            ExprKind::New { ty, members } => {
                let check = self.compile_type(ty)?;
                if let TypeCheck::Collection(kind)
                | TypeCheck::ParameterizedCollection { kind, .. } = check.unaliased()
                {
                    let kind = *kind;
                    let has_default =
                        matches!(check.unaliased(), TypeCheck::ParameterizedCollection { .. });
                    if has_default {
                        self.default_type(&check, output, ty.span)?;
                    }
                    self.collection_body(members, kind, has_default, output)?;
                    if matches!(check.unaliased(), TypeCheck::ParameterizedCollection { .. }) {
                        output.push_at(Instruction::CheckType(check), self.type_source_span(ty));
                    }
                } else {
                    self.class_expression(
                        ty,
                        output,
                        self.type_scopes.len(),
                        &mut BTreeSet::new(),
                    )?;
                    self.instance_body(members, ty.span, output)?;
                }
            }
            ExprKind::Subscript { receiver, key } => {
                if matches!(receiver.unparenthesized().kind, ExprKind::Super) {
                    self.expression(key, output)?;
                    output.push(Instruction::SuperSubscript);
                } else {
                    self.expression(receiver, output)?;
                    self.expression(key, output)?;
                    output.push(Instruction::Subscript);
                }
            }
            ExprKind::EntryParent => output.push(Instruction::EntryParent),
            ExprKind::Uninitialized => output.push(Instruction::AbstractMember {
                name: Arc::from(self.member.as_str()),
                method: true,
            }),
            ExprKind::Int(value) => output.push(Instruction::Constant(Value::Int(*value))),
            ExprKind::Float(value) => output.push(Instruction::Constant(Value::Float(*value))),
            ExprKind::Null => output.push(Instruction::Constant(Value::Null)),
            ExprKind::String(value) => {
                output.push(Instruction::Constant(Value::string(value.as_str())))
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
            ExprKind::InterpolatedString(parts) => {
                output.push(Instruction::Constant(Value::string("")));
                for part in parts {
                    self.expression(part, output)?;
                    output.push(Instruction::ToString);
                    output.push(Instruction::Add);
                }
            }
            ExprKind::NonNull(value) => {
                self.expression(value, output)?;
                output.push_at(Instruction::NonNull, value.span);
            }
            ExprKind::Throw(value) => {
                self.expression(value, output)?;
                output.push(Instruction::Throw);
            }
            ExprKind::TypeCheck { value, ty } => {
                let Some(check) = self.resolve_expression_type(ty, output) else {
                    return Ok(());
                };
                self.expression(value, output)?;
                output.push(Instruction::TestType(check));
            }
            ExprKind::TypeCast { value, ty } => {
                let Some(check) = self.resolve_expression_type(ty, output) else {
                    return Ok(());
                };
                self.expression(value, output)?;
                output.push_at(Instruction::CheckType(check), self.type_source_span(ty));
            }
            ExprKind::NullableMember(receiver, name) => {
                self.expression(receiver, output)?;
                output.push(Instruction::Duplicate);
                let branch = output.len();
                output.push(Instruction::JumpIfNull(0));
                output.push(if self.resolved.const_names.contains(&expression.span) {
                    Instruction::GetConstMember(Arc::from(name.as_str()))
                } else {
                    Instruction::GetMember(Arc::from(name.as_str()))
                });
                output[branch] = Instruction::JumpIfNull(output.len());
            }
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
                    Binding::BaseConstant => output.push(Instruction::Constant(Value::Float(
                        if name.as_ref() == "NaN" {
                            f64::NAN
                        } else {
                            f64::INFINITY
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
            ExprKind::Member(receiver, name) => {
                if matches!(receiver.unparenthesized().kind, ExprKind::Super) {
                    output.push(if self.resolved.const_names.contains(&expression.span) {
                        Instruction::GetConstSuperMember(Arc::from(name.as_str()))
                    } else {
                        Instruction::GetSuperMember(Arc::from(name.as_str()))
                    });
                } else {
                    self.expression(receiver, output)?;
                    output.push(if self.resolved.const_names.contains(&expression.span) {
                        Instruction::GetConstMember(Arc::from(name.as_str()))
                    } else {
                        Instruction::GetMember(Arc::from(name.as_str()))
                    });
                }
            }
            ExprKind::Unary { op, operand } => {
                self.expression(operand, output)?;
                output.push(match op {
                    crate::syntax::UnaryOp::Negate => Instruction::Negate,
                    crate::syntax::UnaryOp::Not => Instruction::Not,
                });
            }
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.expression(condition, output)?;
                let branch = output.len();
                output.push_at(Instruction::JumpIfFalse(0), condition.span);
                self.expression(then_branch, output)?;
                let end = output.len();
                output.push(Instruction::Jump(0));
                output[branch] = Instruction::JumpIfFalse(output.len());
                self.expression(else_branch, output)?;
                output[end] = Instruction::Jump(output.len());
            }
            ExprKind::Function {
                parameters,
                return_type,
                body,
                method,
                header_span,
                type_parameters,
            } => {
                if !type_parameters.is_empty() {
                    return Err(unsupported_type("generic methods"));
                }
                let previous_member = self.member.clone();
                if !method {
                    self.member = self
                        .function_names
                        .entry(expression.span)
                        .or_insert_with(|| {
                            let count = self
                                .function_counts
                                .entry(previous_member.clone())
                                .or_default();
                            *count += 1;
                            format!("{previous_member}.<function#{count}>")
                        })
                        .clone();
                }
                if !method {
                    for ty in parameters
                        .iter()
                        .filter_map(|parameter| parameter.ty.as_ref())
                        .chain(return_type.iter())
                    {
                        if self.resolve_expression_type(ty, output).is_none() {
                            self.member = previous_member;
                            return Ok(());
                        }
                    }
                }
                let mut instructions = Code::default();
                for (index, parameter) in parameters.iter().enumerate() {
                    if let Some(ty) = &parameter.ty {
                        instructions.push(Instruction::LoadLocal(parameters.len() - 1 - index));
                        instructions.push_at(
                            Instruction::CheckType(self.compile_type(ty)?),
                            self.type_source_span(ty),
                        );
                        instructions.push(Instruction::StoreLocal(parameters.len() - 1 - index));
                    }
                }
                self.expression(body, &mut instructions)?;
                if let Some(ty) = return_type {
                    instructions.push_at(
                        Instruction::CheckType(self.compile_type(ty)?),
                        self.type_source_span(ty),
                    );
                }
                instructions.push(Instruction::Return);
                let block = self.block(
                    instructions,
                    Some(if *method {
                        *header_span
                    } else {
                        expression.span
                    }),
                );
                self.program.metadata[block].value_span = Some(body.span);
                self.member = previous_member;
                output.push(Instruction::MakeFunction {
                    block,
                    arity: parameters.len(),
                    method: *method,
                    capture_custom_this: self.resolved.custom_functions.contains(&expression.span),
                });
            }
            ExprKind::Call { callee, arguments } => {
                if matches!(&callee.kind, ExprKind::Name(name) if name == "List")
                    && self.resolved.names.get(&callee.span) == Some(&Binding::ImplicitMethod)
                {
                    for argument in arguments {
                        self.expression(argument, output)?;
                    }
                    output.push(Instruction::MakeList {
                        length: arguments.len(),
                    });
                    return Ok(());
                }
                let callee = callee.unparenthesized();
                if matches!(&callee.kind, ExprKind::Name(name) if name == "read")
                    && self.resolved.names.get(&callee.span) == Some(&Binding::ImplicitMethod)
                {
                    if arguments.len() != 1 {
                        return Err(RuntimeError::new(
                            ErrorKind::Type,
                            "read expects one argument",
                        ));
                    }
                    self.expression(&arguments[0], output)?;
                    output.push(Instruction::ReadResource);
                    return Ok(());
                }
                let mut nullable_branch = None;
                let method = if let ExprKind::NullableMember(receiver, name) = &callee.kind {
                    self.expression(receiver, output)?;
                    output.push(Instruction::Duplicate);
                    nullable_branch = Some(output.len());
                    output.push(Instruction::JumpIfNull(0));
                    output.push(if self.resolved.const_names.contains(&callee.span) {
                        Instruction::GetConstMethod(Arc::from(name.as_str()))
                    } else {
                        Instruction::GetMethod(Arc::from(name.as_str()))
                    });
                    name != "apply"
                } else if let ExprKind::Member(receiver, name) = &callee.kind {
                    if matches!(receiver.unparenthesized().kind, ExprKind::Super) {
                        output.push(if self.resolved.const_names.contains(&callee.span) {
                            Instruction::GetConstSuperMethod(Arc::from(name.as_str()))
                        } else {
                            Instruction::GetSuperMethod(Arc::from(name.as_str()))
                        });
                    } else {
                        self.expression(receiver, output)?;
                        output.push(if self.resolved.const_names.contains(&callee.span) {
                            Instruction::GetConstMethod(Arc::from(name.as_str()))
                        } else {
                            Instruction::GetMethod(Arc::from(name.as_str()))
                        });
                    }
                    name != "apply"
                } else {
                    self.expression(callee, output)?;
                    true
                };
                for argument in arguments {
                    self.expression(argument, output)?;
                }
                output.push(Instruction::Call {
                    arity: arguments.len(),
                    method,
                });
                if let Some(branch) = nullable_branch {
                    output[branch] = Instruction::JumpIfNull(output.len());
                }
            }
            ExprKind::Binary { op, left, right } => {
                self.expression(left, output)?;
                if matches!(op, BinaryOp::And | BinaryOp::Or | BinaryOp::NullCoalesce) {
                    if *op != BinaryOp::NullCoalesce {
                        output.push(Instruction::CheckLogicalLeft {
                            or: *op == BinaryOp::Or,
                        });
                    }
                    output.push(Instruction::Duplicate);
                    if *op == BinaryOp::Or {
                        output.push(Instruction::Not);
                    }
                    let branch = output.len();
                    output.push(if *op == BinaryOp::NullCoalesce {
                        Instruction::JumpIfNotNull(0)
                    } else {
                        Instruction::JumpIfFalse(0)
                    });
                    output.push(Instruction::Pop);
                    self.expression(right, output)?;
                    if *op != BinaryOp::NullCoalesce {
                        output.push(Instruction::CheckLogicalRight {
                            or: *op == BinaryOp::Or,
                        });
                    }
                    output[branch] = if *op == BinaryOp::NullCoalesce {
                        Instruction::JumpIfNotNull(output.len())
                    } else {
                        Instruction::JumpIfFalse(output.len())
                    };
                } else if self.options.fuse_constant_add
                    && *op == BinaryOp::Add
                    && let ExprKind::Int(value) = right.kind
                {
                    output.push(Instruction::AddIntConstant(value));
                } else {
                    self.expression(right, output)?;
                    output.push(match op {
                        BinaryOp::Pipe => Instruction::PipeApply,
                        BinaryOp::Add => Instruction::Add,
                        BinaryOp::Subtract => Instruction::Subtract,
                        BinaryOp::Multiply => Instruction::Multiply,
                        BinaryOp::Divide => Instruction::Divide,
                        BinaryOp::IntegerDivide => Instruction::IntegerDivide,
                        BinaryOp::Remainder => Instruction::Remainder,
                        BinaryOp::Power => Instruction::Power,
                        BinaryOp::Equal => Instruction::Equal,
                        BinaryOp::NotEqual => Instruction::NotEqual,
                        BinaryOp::Less => Instruction::Less,
                        BinaryOp::LessEqual => Instruction::LessEqual,
                        BinaryOp::Greater => Instruction::Greater,
                        BinaryOp::GreaterEqual => Instruction::GreaterEqual,
                        BinaryOp::And | BinaryOp::Or | BinaryOp::NullCoalesce => unreachable!(),
                    });
                }
            }
            ExprKind::Let {
                value, body, ty, ..
            } => {
                self.expression(value, output)?;
                if let Some(ty) = ty {
                    let Some(check) = self.resolve_expression_type(ty, output) else {
                        return Ok(());
                    };
                    output.push_at(Instruction::CheckType(check), self.type_source_span(ty));
                }
                output.push(Instruction::PushLocal);
                self.expression(body, output)?;
                output.push(Instruction::PopLocal);
            }
            ExprKind::Object { parent, members } => {
                if let Some(parent) = parent {
                    self.expression(parent, output)?;
                }
                let begin = output.len();
                self.collection_body(
                    members,
                    crate::heap::CollectionKind::Dynamic,
                    parent.is_some(),
                    output,
                )?;
                if let Some(parent) = parent {
                    output.spans[begin] = Some(parent.unparenthesized().span);
                }
            }
        }
        Ok(())
    }
}

fn scalar_string(value: &Value) -> Result<String, RuntimeError> {
    Ok(match value {
        Value::Null => "null".to_owned(),
        Value::Bool(value) => value.to_string(),
        Value::Int(value) => value.to_string(),
        Value::Float(value) => crate::numeric::double_to_string(*value),
        Value::String(value) => value.to_string(),
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
    let actual_type = crate::diagnostic_values::type_name(actual, heap, module)?;
    let value = crate::diagnostic_values::render(actual, heap, module, limit)?;
    if matches!(expected.unaliased(), TypeCheck::Nothing) {
        return Ok(RuntimeError::new(
            ErrorKind::Type,
            format!(
                "Expected `nothing`, but got type `{actual_type}`.\n\n\
                 `nothing` is typically used as the return type of functions that throw errors.\n\
                 Value: {value}"
            ),
        ));
    }
    let expected = type_name(expected, module);
    Ok(RuntimeError::new(
        ErrorKind::Type,
        format!(
            "Expected value of type `{expected}`, but got type `{actual_type}`.\nValue: {value}"
        ),
    ))
}

fn type_name(ty: &TypeCheck, module: &str) -> String {
    match ty {
        TypeCheck::Class { name, .. } => format!("{module}#{name}"),
        TypeCheck::Collection(kind) | TypeCheck::ParameterizedCollection { kind, .. } => {
            format!("{kind:?}")
        }
        TypeCheck::StringLiteral(value) => format!("{value:?}"),
        TypeCheck::Nullable(inner) => format!("{}?", type_name(inner, module)),
        TypeCheck::Alias(inner) | TypeCheck::GenericAlias(inner) => type_name(inner, module),
        TypeCheck::Constrained { base, .. } => type_name(base, module),
        TypeCheck::Union { types, .. } => types
            .iter()
            .map(|ty| type_name(ty, module))
            .collect::<Vec<_>>()
            .join("|"),
        _ => format!("{ty:?}"),
    }
}

fn value_type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "Null",
        Value::Bool(_) => "Boolean",
        Value::Int(_) => "Int",
        Value::Float(_) => "Float",
        Value::String(_) => "String",
        Value::Bytes(_) => "Bytes",
        Value::Object(_) => "Dynamic",
        Value::Function(_) => "Function",
        Value::Class(_) => "Class",
    }
}

fn display_value(value: &Value) -> String {
    match value {
        Value::String(value) => format!("{value:?}"),
        _ => scalar_string(value).unwrap_or_else(|_| value_type_name(value).to_owned()),
    }
}

fn unary_error(operator: &str, operand: &Value) -> RuntimeError {
    RuntimeError::new(
        ErrorKind::Type,
        format!(
            "Operator `{operator}` is not defined for operand type `{}`.\nOperand: {}",
            value_type_name(operand),
            display_value(operand)
        ),
    )
}

fn binary_error(operator: &str, left: &Value, right: &Value) -> RuntimeError {
    RuntimeError::new(
        ErrorKind::Type,
        format!(
            "Operator `{operator}` is not defined for operand types `{}` and `{}`.\nLeft operand : {}\nRight operand: {}",
            value_type_name(left),
            value_type_name(right),
            display_value(left),
            display_value(right)
        ),
    )
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
                let check = self.compile_type(ty)?;
                self.initialize_type(&check, self.type_source_span(ty), output);
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
                    let check = self.compile_type(ty)?;
                    self.initialize_type(&check, self.type_source_span(ty), output);
                }
            }
        }
        Ok(())
    }

    fn initialize_type(&mut self, check: &TypeCheck, span: Span, output: &mut Code) {
        match check {
            TypeCheck::Class { id, .. } => {
                if let Some(member) = self
                    .resolved
                    .module
                    .members
                    .iter()
                    .find(|member| member.expr.span.start == *id)
                {
                    output.push_at(
                        Instruction::ResolveClassType {
                            name: Arc::from(member.name.as_str()),
                            local: crate::syntax::modifier_flags(&member.modifiers)
                                & crate::syntax::ModifierKind::Local.flag()
                                != 0,
                        },
                        span,
                    );
                    output.push(Instruction::Pop);
                }
            }
            TypeCheck::ParameterizedCollection { arguments, .. } => {
                for (ty, span) in arguments {
                    self.initialize_type(ty, *span, output);
                }
            }
            TypeCheck::Nullable(inner) => self.initialize_type(inner, span, output),
            TypeCheck::Alias(inner) | TypeCheck::GenericAlias(inner) => {
                self.initialize_type(inner, span, output)
            }
            TypeCheck::Constrained {
                base, base_span, ..
            } => self.initialize_type(base, *base_span, output),
            TypeCheck::Union { types, .. } => {
                for ty in types {
                    self.initialize_type(ty, span, output);
                }
            }
            _ => {}
        }
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
                self.initialize_type(&check, self.type_source_span(ty), output);
                Some(check)
            }
            Err(error) => {
                output.push_at(Instruction::Fail(error.at(ty.span)), ty.span);
                None
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
                error.frames.push(RuntimeFrame {
                    span: ty.span,
                    member: Arc::from(""),
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

    fn type_source_span_at(
        &self,
        ty: &crate::syntax::Type,
        parameters: &BTreeMap<String, Span>,
        aliases: &mut BTreeSet<Span>,
    ) -> Span {
        use crate::syntax::{ExprKind, TypeKind};
        match &ty.kind {
            TypeKind::Nullable(inner) => self.type_source_span_at(inner, parameters, aliases),
            TypeKind::Named { path, arguments } if path.len() == 1 => {
                if let Some(span) = parameters.get(&path[0]) {
                    return *span;
                }
                if let Some((member, _)) = self.find_type(&path[0], self.type_scopes.len())
                    && let ExprKind::TypeAlias {
                        type_parameters,
                        ty: aliased,
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
        Ok(match &ty.kind {
            TypeKind::Unknown => TypeCheck::Any,
            TypeKind::Nothing => TypeCheck::Nothing,
            TypeKind::String(value) => TypeCheck::StringLiteral(Arc::from(value.as_str())),
            TypeKind::Nullable(inner) => TypeCheck::Nullable(Box::new(
                self.compile_type_at(inner, limit, aliases, parameters)?,
            )),
            TypeKind::Constrained { base, constraints } => {
                let base_check = self.compile_type_at(base, limit, aliases, parameters)?;
                let mut compiled = Vec::new();
                for expression in constraints {
                    let expression = expression.unparenthesized();
                    let previous =
                        std::mem::replace(&mut self.active_type_parameters, parameters.clone());
                    let mut code = Code::default();
                    let result = self.expression(expression, &mut code);
                    self.active_type_parameters = previous;
                    result?;
                    code.push(Instruction::Return);
                    let block = self.block(code, Some(expression.span));
                    compiled.push(ConstraintCode {
                        block,
                        span: expression.span,
                        module_environment: self.constraint_module,
                    });
                }
                TypeCheck::Constrained {
                    base: Box::new(base_check),
                    base_span: self.type_source_span(base),
                    constraints: compiled,
                }
            }
            TypeKind::Union { types, default } => TypeCheck::Union {
                spans: types.iter().map(|ty| self.type_source_span(ty)).collect(),
                // Alternatives retain their use-site labels even when checking an
                // alias expands to a different source section. Type parameters
                // alone inherit the actual argument's source section.
                labels: types
                    .iter()
                    .map(|ty| match &ty.kind {
                        TypeKind::Named { path, .. } if path.len() == 1 => self
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
            TypeKind::Named { path, arguments } if path.len() == 1 => {
                if let Some(parameter) = parameters.get(&path[0]) {
                    if !arguments.is_empty() {
                        return Err(RuntimeError::new(
                            ErrorKind::Type,
                            "Type parameters cannot have type arguments.",
                        ));
                    }
                    parameter.clone()
                } else if let Some((member, index)) = self.find_type(&path[0], limit) {
                    match &member.expr.kind {
                        ExprKind::Class { .. } => {
                            if !arguments.is_empty() {
                                return Err(RuntimeError::new(
                                    ErrorKind::Type,
                                    format!("Class `{}` does not have type parameters.", path[0]),
                                ));
                            }
                            TypeCheck::Class {
                                id: member.expr.span.start,
                                name: Arc::from(member.name),
                            }
                        }
                        ExprKind::TypeAlias {
                            type_parameters,
                            ty: aliased,
                        } => {
                            if type_parameters.len() != arguments.len() {
                                return Err(RuntimeError::new(
                                    ErrorKind::Type,
                                    format!(
                                        "Expected {} type arguments but got {}.",
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
                            let previous_module =
                                std::mem::replace(&mut self.constraint_module, true);
                            let result = self
                                .compile_type_at(aliased, index + 1, aliases, &substitutions)
                                .map_err(|mut error| {
                                    error.frames.push(RuntimeFrame {
                                        span: aliased.span,
                                        member: Arc::from(format!("#{}", member.name)),
                                    });
                                    error
                                });
                            self.constraint_module = previous_module;
                            self.parameter_spans = previous_spans;
                            let result = result?;
                            aliases.remove(&member.expr.span);
                            if type_parameters.is_empty() {
                                TypeCheck::Alias(Box::new(result))
                            } else {
                                TypeCheck::GenericAlias(Box::new(result))
                            }
                        }
                        _ => {
                            return Err(RuntimeError::new(
                                ErrorKind::Type,
                                format!("`{}` is not a type.", path[0]),
                            ));
                        }
                    }
                } else {
                    if !arguments.is_empty() {
                        let kind = match path[0].as_str() {
                            "Listing" => crate::heap::CollectionKind::Listing,
                            "Mapping" => crate::heap::CollectionKind::Mapping,
                            "List" => crate::heap::CollectionKind::List,
                            _ => return Err(unsupported_type("generic builtin types")),
                        };
                        let arity = if kind == crate::heap::CollectionKind::Mapping {
                            2
                        } else {
                            1
                        };
                        if arguments.len() != arity {
                            return Err(RuntimeError::new(
                                ErrorKind::Type,
                                format!(
                                    "Expected {arity} type arguments but got {}.",
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
                        "Any" => TypeCheck::Any,
                        "NonNull" => TypeCheck::NonNull,
                        "Null" => TypeCheck::Null,
                        "Boolean" => TypeCheck::Boolean,
                        "Int" => TypeCheck::Int,
                        "Float" => TypeCheck::Float,
                        "Number" => TypeCheck::Number,
                        "String" => TypeCheck::String,
                        "Bytes" => TypeCheck::Bytes,
                        "Dynamic" => TypeCheck::Collection(crate::heap::CollectionKind::Dynamic),
                        "Listing" => TypeCheck::Collection(crate::heap::CollectionKind::Listing),
                        "Mapping" => TypeCheck::Collection(crate::heap::CollectionKind::Mapping),
                        "List" => TypeCheck::Collection(crate::heap::CollectionKind::List),
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
        let TypeCheck::Class { id, .. } = check.unaliased() else {
            return Err(RuntimeError::new(ErrorKind::Type, "Expected a class type."));
        };
        for (index, members) in self.type_scopes.iter().enumerate().rev() {
            if let Some(member) = members.iter().find(|member| {
                member.expr.span.start == *id
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
                    name: Arc::from(self.member.as_str()),
                    method: false,
                },
                member.span,
            );
            return Ok(());
        }
        if let Some(ty) = &member.ty {
            let check = self.compile_type(ty)?;
            if self.default_type(&check, output, ty.span)? {
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
        match check {
            TypeCheck::Null | TypeCheck::Nullable(_) => {
                output.push_at(Instruction::Constant(Value::Null), span)
            }
            TypeCheck::StringLiteral(value) => {
                output.push_at(Instruction::Constant(Value::String(value.clone())), span)
            }
            TypeCheck::Union {
                types,
                default: Some(index),
                ..
            } => return self.default_type(&types[*index], output, span),
            TypeCheck::Constrained { base, .. } => return self.default_type(base, output, span),
            TypeCheck::Alias(inner) | TypeCheck::GenericAlias(inner) => {
                return self.default_type(inner, output, span);
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
                    if let TypeCheck::ParameterizedCollection { arguments, .. } = check {
                        let (ty, span) = arguments.last().expect("collection value type");
                        if !ty.noop() {
                            let defined = self.default_type(ty, output, *span)?;
                            output.push(Instruction::SetCollectionDefault { defined });
                        }
                    }
                    output.push(Instruction::EndCollection);
                }
            }
            TypeCheck::Class { .. } => {
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
                ModifierKind::Local | ModifierKind::Const | ModifierKind::Hidden => true,
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
        let flags = crate::syntax::modifier_flags(&member.modifiers);
        for kind in [ModifierKind::External, ModifierKind::Fixed] {
            if flags & kind.flag() != 0 {
                return Err(
                    unsupported_type(&format!("{} modifiers", kind.spelling())).at(member.span)
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
