//! Bounded, resumable bytecode candidate. This is an architecture experiment, not full Pkl.

use std::collections::BTreeSet;
use std::sync::Arc;

use crate::heap::{
    CacheKey, Environment, Function, Heap, HeapStats, LazyState, Member, Object, Scope,
};
use crate::source::Span;
use crate::value::{ErrorKind, ObjectId, RuntimeError, RuntimeFrame, Value};

#[derive(Clone, Debug)]
pub enum Instruction {
    Constant(Value),
    Input(usize),
    LoadLocal(usize),
    PushLocal,
    PopLocal,
    LoadThis,
    LoadOuter,
    LoadModule,
    NonNull,
    Throw,
    ToString,
    TestType(TypeCheck),
    JumpIfNull(usize),
    GetMember(Arc<str>),
    GetMethod(Arc<str>),
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
    MakeFunction {
        block: usize,
        arity: usize,
        method: bool,
    },
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

/// Type checks supported by the shared evaluator. Composite object/function types
/// need their own lazy wrappers and are rejected by the compiler for now.
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
    StringLiteral(Arc<str>),
    Nullable(Box<TypeCheck>),
    Union(Vec<TypeCheck>),
}

impl TypeCheck {
    fn accepts(&self, value: &Value) -> bool {
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
            Self::StringLiteral(expected) => {
                matches!(value, Value::String(actual) if actual == expected)
            }
            Self::Nullable(inner) => matches!(value, Value::Null) || inner.accepts(value),
            Self::Union(types) => types.iter().any(|ty| ty.accepts(value)),
        }
    }
}

/// Source metadata is parallel to bytecode, so diagnostics add no VM dispatches.
#[derive(Clone, Debug, Default)]
pub struct BlockMetadata {
    pub spans: Vec<Option<Span>>,
    pub member: Arc<str>,
    pub declaration: Option<Span>,
    pub value_span: Option<Span>,
}

#[derive(Clone, Debug)]
pub struct Program {
    pub blocks: Vec<Vec<Instruction>>,
    pub metadata: Vec<BlockMetadata>,
    pub objects: Vec<Vec<Member>>,
    pub entry: usize,
}

impl Program {
    pub fn new(blocks: Vec<Vec<Instruction>>) -> Self {
        Self {
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
    receiver: ObjectId,
    owner: ObjectId,
    name: Arc<str>,
    cache: Option<CacheKey>,
    method: bool,
}

struct Frame {
    block: usize,
    pc: usize,
    base: usize,
    environment: Environment,
    force: Option<Force>,
    lambda: bool,
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
        }
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
                roots.extend(frame.environment.locals.iter().cloned());
                for scope in &frame.environment.scopes {
                    roots.push(Value::Object(scope.receiver));
                    roots.push(Value::Object(scope.owner));
                }
                if let Some(force) = &frame.force {
                    roots.push(Value::Object(force.receiver));
                    roots.push(Value::Object(force.owner));
                }
            }
        }
        self.heap.collect(&roots)
    }

    fn fail(&mut self, mut error: RuntimeError) {
        if error.kind == ErrorKind::MissingMember
            && let Some(frame) = self.frames.last()
            && let Some(Instruction::GetProperty { name, .. }) = self
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
                error.frames.push(RuntimeFrame {
                    span: *span,
                    member: metadata.member.clone(),
                });
                if frame.force.is_some()
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
        let key = CacheKey {
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
                member.name == name && member.local == local && member.method == method
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
                    force.receiver == receiver
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
        if member.code >= self.program.blocks.len() {
            return Err(invalid("invalid member block"));
        }
        if self.frames.len() >= 4096 {
            return Err(RuntimeError::new(
                ErrorKind::Cycle,
                "experiment frame limit exceeded",
            ));
        }
        let mut environment = self.heap.get(owner)?.environment.clone();
        environment.scopes.insert(0, Scope { receiver, owner });
        if memoize {
            self.heap
                .get_mut(receiver)?
                .cache
                .insert(key.clone(), LazyState::Evaluating);
        }
        self.member_evaluations += 1;
        self.frames.push(Frame {
            block: member.code,
            pc: 0,
            base: self.stack.len(),
            environment,
            lambda: false,
            force: Some(Force {
                receiver,
                owner,
                name,
                cache: memoize.then_some(key),
                method,
            }),
        });
        Ok(())
    }

    fn step(&mut self) -> Result<(), RuntimeError> {
        let frame = self
            .frames
            .last_mut()
            .ok_or_else(|| invalid("missing execution frame"))?;
        let instruction = self
            .program
            .blocks
            .get(frame.block)
            .and_then(|block| block.get(frame.pc))
            .ok_or_else(|| invalid("instruction pointer outside block"))?
            .clone();
        frame.pc += 1;
        match instruction {
            Instruction::Constant(value) => self.stack.push(value),
            Instruction::Input(index) => self.stack.push(
                self.inputs
                    .get(index)
                    .cloned()
                    .ok_or_else(|| invalid("missing input"))?,
            ),
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
            Instruction::LoadThis => self.stack.push(Value::Object(self.scope(0)?.receiver)),
            Instruction::LoadOuter => {
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
                    return Err(type_error(&TypeCheck::String, &value));
                };
                return Err(RuntimeError::new(ErrorKind::Type, message.as_ref()));
            }
            Instruction::ToString => {
                let value = self.pop()?;
                self.stack.push(Value::string(scalar_string(&value)?));
            }
            Instruction::TestType(ty) => {
                let value = self.pop()?;
                self.stack.push(Value::Bool(ty.accepts(&value)));
            }
            Instruction::JumpIfNull(target) => {
                if matches!(self.pop()?, Value::Null) {
                    self.jump(target)?;
                }
            }
            Instruction::GetMember(ref name) | Instruction::GetMethod(ref name) => {
                let method = matches!(instruction, Instruction::GetMethod(_));
                let receiver = self.pop()?;
                if matches!(receiver, Value::Function(_)) && method && name.as_ref() == "apply" {
                    self.stack.push(receiver);
                } else {
                    let receiver = receiver.as_object()?;
                    self.force(receiver, receiver, name.clone(), false, true, method)?;
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
            Instruction::GetSuperMember(ref name) | Instruction::GetSuperMethod(ref name) => {
                let method = matches!(instruction, Instruction::GetSuperMethod(_));
                let scope = self.scope(0)?;
                let parent = self.heap.get(scope.owner)?.parent.ok_or_else(|| {
                    RuntimeError::new(ErrorKind::MissingMember, "super has no parent")
                })?;
                // Java ReadSuperPropertyNode deliberately does not memoize super calls.
                self.force(scope.receiver, parent, name.clone(), false, false, method)?;
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
                let environment = self
                    .frames
                    .last()
                    .expect("active frame")
                    .environment
                    .clone();
                let object = self.heap.allocate(Object {
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
                environment.locals.extend(arguments);
                self.frames.push(Frame {
                    block: function.block,
                    pc: 0,
                    base: self.stack.len(),
                    environment,
                    force: None,
                    lambda: !function.method,
                });
            }
            Instruction::CheckType(ty) => {
                let value = self.pop()?;
                if !ty.accepts(&value) {
                    return Err(type_error(&ty, &value));
                }
                self.stack.push(value);
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
            Instruction::Add
            | Instruction::Subtract
            | Instruction::Multiply
            | Instruction::Divide
            | Instruction::Less
            | Instruction::LessEqual
            | Instruction::Greater
            | Instruction::GreaterEqual
            | Instruction::Equal
            | Instruction::NotEqual
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
                value => return Err(type_error(&TypeCheck::Boolean, &value)),
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
                let frame = self.frames.pop().expect("active frame");
                if let Some(force) = frame.force
                    && let Some(key) = force.cache
                {
                    self.heap
                        .get_mut(force.receiver)?
                        .cache
                        .insert(key, LazyState::Completed(value.clone()));
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
            Instruction::Power => Value::Float(libm::pow(left as f64, right as f64)),
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
                    return Err(RuntimeError::new(
                        ErrorKind::Type,
                        "unsupported binary operand types",
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
        Instruction::Power => Value::Float(libm::pow(left, right)),
        Instruction::IntegerDivide => {
            let result = (left / right).trunc();
            if !result.is_finite()
                || !(-9223372036854775808.0..9223372036854775808.0).contains(&result)
            {
                return Err(RuntimeError::new(
                    ErrorKind::Arithmetic,
                    "cannot convert quotient to Int",
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
    let mut compiler = Compiler {
        resolved,
        program: Program::new(Vec::new()),
        options,
        member: String::new(),
    };
    let template = compiler.object(&resolved.module.members)?;
    compiler.program.entry = compiler.program.blocks.len();
    compiler.program.blocks.push(vec![
        Instruction::MakeObject {
            template,
            has_parent: false,
        },
        Instruction::Return,
    ]);
    compiler.program.metadata.push(BlockMetadata::default());
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
}

impl Compiler<'_> {
    fn object(&mut self, members: &[crate::syntax::Member]) -> Result<usize, RuntimeError> {
        let mut result = Vec::new();
        for member in members {
            let previous_member = self.member.clone();
            self.member = if previous_member.is_empty() {
                format!("#{}", member.name)
            } else {
                format!("{previous_member}.{}", member.name)
            };
            let mut instructions = Code::default();
            self.expression(&member.expr, &mut instructions)?;
            if let Some(ty) = &member.ty {
                instructions.push_at(Instruction::CheckType(compile_type(ty)?), ty.span);
            }
            instructions.push(Instruction::Return);
            let code = self.program.blocks.len();
            self.block(instructions, Some(member.span));
            self.program.metadata[code].value_span = Some(member.expr.span);
            self.member = previous_member;
            result.push(Member {
                name: Arc::from(member.name.as_str()),
                code,
                local: member.local,
                method: matches!(
                    member.expr.kind,
                    crate::syntax::ExprKind::Function { method: true, .. }
                ),
            });
        }
        let index = self.program.objects.len();
        self.program.objects.push(result);
        Ok(index)
    }

    fn block(&mut self, code: Code, declaration: Option<Span>) -> usize {
        let index = self.program.blocks.len();
        self.program.blocks.push(code.instructions);
        self.program.metadata.push(BlockMetadata {
            spans: code.spans,
            member: Arc::from(self.member.as_str()),
            declaration,
            value_span: None,
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
            ExprKind::Int(value) => output.push(Instruction::Constant(Value::Int(*value))),
            ExprKind::Float(value) => output.push(Instruction::Constant(Value::Float(*value))),
            ExprKind::Null => output.push(Instruction::Constant(Value::Null)),
            ExprKind::String(value) => {
                output.push(Instruction::Constant(Value::string(value.as_str())))
            }
            ExprKind::Bool(value) => output.push(Instruction::Constant(Value::Bool(*value))),
            ExprKind::This => output.push(Instruction::LoadThis),
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
                self.expression(value, output)?;
                output.push(Instruction::TestType(compile_type(ty)?));
            }
            ExprKind::TypeCast { value, ty } => {
                self.expression(value, output)?;
                output.push_at(Instruction::CheckType(compile_type(ty)?), ty.span);
            }
            ExprKind::NullableMember(receiver, name) => {
                self.expression(receiver, output)?;
                output.push(Instruction::Duplicate);
                let branch = output.len();
                output.push(Instruction::JumpIfNull(0));
                output.push(Instruction::GetMember(Arc::from(name.as_str())));
                output[branch] = Instruction::JumpIfNull(output.len());
            }
            ExprKind::Super => {
                return Err(RuntimeError::new(
                    ErrorKind::Unsupported,
                    "bare super is unsupported",
                ));
            }
            ExprKind::Name(name) => {
                let name = Arc::from(name.as_str());
                match self
                    .resolved
                    .names
                    .get(&expression.span)
                    .ok_or_else(|| invalid("missing static binding"))?
                {
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
                    Binding::ImplicitMethod => output.push(Instruction::GetMethodProperty {
                        name,
                        owner_depth: 0,
                        local: false,
                    }),
                    Binding::ImplicitThis => output.push(Instruction::GetProperty {
                        name,
                        owner_depth: 0,
                        local: false,
                    }),
                }
            }
            ExprKind::Member(receiver, name) => {
                if matches!(receiver.kind, ExprKind::Super) {
                    output.push(Instruction::GetSuperMember(Arc::from(name.as_str())));
                } else {
                    self.expression(receiver, output)?;
                    output.push(Instruction::GetMember(Arc::from(name.as_str())));
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
            } => {
                let mut instructions = Code::default();
                for (index, parameter) in parameters.iter().enumerate() {
                    if let Some(ty) = &parameter.ty {
                        instructions.push(Instruction::LoadLocal(parameters.len() - 1 - index));
                        instructions.push_at(Instruction::CheckType(compile_type(ty)?), ty.span);
                        instructions.push(Instruction::Pop);
                    }
                }
                self.expression(body, &mut instructions)?;
                if let Some(ty) = return_type {
                    instructions.push_at(Instruction::CheckType(compile_type(ty)?), ty.span);
                }
                instructions.push(Instruction::Return);
                let block = self.block(instructions, Some(*header_span));
                output.push(Instruction::MakeFunction {
                    block,
                    arity: parameters.len(),
                    method: *method,
                });
            }
            ExprKind::Call { callee, arguments } => {
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
                    output.push(Instruction::GetMethod(Arc::from(name.as_str())));
                    name != "apply"
                } else if let ExprKind::Member(receiver, name) = &callee.kind {
                    if matches!(receiver.kind, ExprKind::Super) {
                        output.push(Instruction::GetSuperMethod(Arc::from(name.as_str())));
                    } else {
                        self.expression(receiver, output)?;
                        output.push(Instruction::GetMethod(Arc::from(name.as_str())));
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
            ExprKind::Let { value, body, .. } => {
                self.expression(value, output)?;
                output.push(Instruction::PushLocal);
                self.expression(body, output)?;
                output.push(Instruction::PopLocal);
            }
            ExprKind::Object { parent, members } => {
                if let Some(parent) = parent {
                    self.expression(parent, output)?;
                }
                let template = self.object(members)?;
                output.push(Instruction::MakeObject {
                    template,
                    has_parent: parent.is_some(),
                });
            }
        }
        Ok(())
    }
}

fn compile_type(ty: &crate::syntax::Type) -> Result<TypeCheck, RuntimeError> {
    use crate::syntax::TypeKind;
    Ok(match &ty.kind {
        TypeKind::Unknown => TypeCheck::Any,
        TypeKind::Nothing => TypeCheck::Nothing,
        TypeKind::String(value) => TypeCheck::StringLiteral(Arc::from(value.as_str())),
        TypeKind::Nullable(inner) => TypeCheck::Nullable(Box::new(compile_type(inner)?)),
        TypeKind::Union { types, .. } => {
            TypeCheck::Union(types.iter().map(compile_type).collect::<Result<_, _>>()?)
        }
        TypeKind::Named { path, arguments } if path.len() == 1 && arguments.is_empty() => {
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
                _ => {
                    return Err(RuntimeError::new(
                        ErrorKind::Unsupported,
                        format!("type {} is not implemented", path[0]),
                    ));
                }
            }
        }
        _ => {
            return Err(RuntimeError::new(
                ErrorKind::Unsupported,
                "this type requires runtime support that is not implemented",
            ));
        }
    })
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

fn type_error(expected: &TypeCheck, actual: &Value) -> RuntimeError {
    let expected = type_name(expected);
    let actual_type = match actual {
        Value::Null => "Null",
        Value::Bool(_) => "Boolean",
        Value::Int(_) => "Int",
        Value::Float(_) => "Float",
        Value::String(_) => "String",
        Value::Bytes(_) => "Bytes",
        Value::Object(_) => "Dynamic",
        Value::Function(_) => "Function",
    };
    let value = match actual {
        Value::String(value) => format!("{value:?}"),
        _ => scalar_string(actual).unwrap_or_else(|_| actual_type.to_owned()),
    };
    RuntimeError::new(
        ErrorKind::Type,
        format!(
            "Expected value of type `{expected}`, but got type `{actual_type}`.\nValue: {value}"
        ),
    )
}

fn type_name(ty: &TypeCheck) -> String {
    match ty {
        TypeCheck::StringLiteral(value) => format!("{value:?}"),
        TypeCheck::Nullable(inner) => format!("{}?", type_name(inner)),
        TypeCheck::Union(types) => types.iter().map(type_name).collect::<Vec<_>>().join("|"),
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
