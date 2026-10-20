//! Bounded, resumable bytecode candidate. This is an architecture experiment, not full Pkl.

use std::collections::BTreeSet;
use std::sync::Arc;

use crate::heap::{CacheKey, Environment, Heap, HeapStats, LazyState, Member, Object, Scope};
use crate::value::{ErrorKind, ObjectId, RuntimeError, Value};

#[derive(Clone, Debug)]
pub enum Instruction {
    Constant(Value),
    Input(usize),
    LoadLocal(usize),
    PushLocal,
    PopLocal,
    LoadThis,
    GetMember(Arc<str>),
    GetProperty {
        name: Arc<str>,
        owner_depth: usize,
        local: bool,
    },
    GetSuperMember(Arc<str>),
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
    Jump(usize),
    JumpIfFalse(usize),
    ReadResource,
    Return,
}

#[derive(Clone, Debug)]
pub struct Program {
    pub blocks: Vec<Vec<Instruction>>,
    pub objects: Vec<Vec<Member>>,
    pub entry: usize,
}

impl Program {
    pub fn new(blocks: Vec<Vec<Instruction>>) -> Self {
        Self {
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
}

struct Frame {
    block: usize,
    pc: usize,
    base: usize,
    environment: Environment,
    force: Option<Force>,
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
            .any(|instruction| matches!(instruction, Instruction::Constant(Value::Object(_))))
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
            if let Value::Object(id) = input {
                self.heap.get(*id)?;
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
        if let Err(error) = self.force(receiver, receiver, Arc::from(name), false, true) {
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
                if let Value::Object(object) = &value {
                    self.heap.get(*object)?;
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

    fn fail(&mut self, error: RuntimeError) {
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
    ) -> Result<(), RuntimeError> {
        let key = CacheKey {
            owner: local.then_some(initial_owner),
            name: name.clone(),
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
            if let Some(member) = object
                .members
                .iter()
                .find(|member| member.name == name && member.local == local)
            {
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
                    force.receiver == receiver && force.owner == owner && force.name == name
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
            force: Some(Force {
                receiver,
                owner,
                name,
                cache: memoize.then_some(key),
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
            Instruction::GetMember(name) => {
                let receiver = self.pop()?.as_object()?;
                self.force(receiver, receiver, name, false, true)?;
            }
            Instruction::GetProperty {
                name,
                owner_depth,
                local,
            } => {
                let scope = self.scope(owner_depth)?;
                self.force(
                    scope.receiver,
                    if local { scope.owner } else { scope.receiver },
                    name,
                    local,
                    true,
                )?;
            }
            Instruction::GetSuperMember(name) => {
                let scope = self.scope(0)?;
                let parent = self.heap.get(scope.owner)?.parent.ok_or_else(|| {
                    RuntimeError::new(ErrorKind::MissingMember, "super has no parent")
                })?;
                // Java ReadSuperPropertyNode deliberately does not memoize super calls.
                self.force(scope.receiver, parent, name, false, false)?;
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
            Instruction::AddIntConstant(right) => {
                let left = self.pop()?;
                let value = match left {
                    Value::Int(left) => {
                        Value::Int(crate::numeric::add(left, right).map_err(numeric_error)?)
                    }
                    Value::Float(left) => Value::Float(left + right as f64),
                    _ => {
                        return Err(RuntimeError::new(
                            ErrorKind::Type,
                            "unsupported binary operand types",
                        ));
                    }
                };
                self.stack.push(value);
            }
            Instruction::Add
            | Instruction::Subtract
            | Instruction::Multiply
            | Instruction::Divide
            | Instruction::Less => {
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
                _ => {
                    return Err(RuntimeError::new(
                        ErrorKind::Type,
                        "condition must be Boolean",
                    ));
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
            _ => return Err(invalid("invalid binary instruction")),
        }),
        (Value::String(left), Value::String(right)) if matches!(instruction, Instruction::Add) => {
            Ok(Value::string(format!("{left}{right}")))
        }
        (Value::Int(left), Value::Float(right)) => float_binary(instruction, left as f64, right),
        (Value::Float(left), Value::Int(right)) => float_binary(instruction, left, right as f64),
        (Value::Float(left), Value::Float(right)) => float_binary(instruction, left, right),
        _ => Err(RuntimeError::new(
            ErrorKind::Type,
            "unsupported binary operand types",
        )),
    }
}

fn float_binary(instruction: &Instruction, left: f64, right: f64) -> Result<Value, RuntimeError> {
    Ok(match instruction {
        Instruction::Add => Value::Float(left + right),
        Instruction::Subtract => Value::Float(left - right),
        Instruction::Multiply => Value::Float(left * right),
        Instruction::Divide => Value::Float(left / right),
        Instruction::Less => Value::Bool(left < right),
        _ => return Err(invalid("invalid numeric instruction")),
    })
}

fn numeric_error(error: crate::numeric::NumericError) -> RuntimeError {
    RuntimeError::new(ErrorKind::Arithmetic, format!("{error:?}"))
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
    Ok(Arc::new(compiler.program))
}

struct Compiler<'a> {
    resolved: &'a crate::resolver::ResolvedModule,
    program: Program,
    options: CompileOptions,
}

impl Compiler<'_> {
    fn object(&mut self, members: &[crate::syntax::Member]) -> Result<usize, RuntimeError> {
        let mut result = Vec::new();
        for member in members {
            let mut instructions = Vec::new();
            self.expression(&member.expr, &mut instructions)?;
            instructions.push(Instruction::Return);
            let code = self.program.blocks.len();
            self.program.blocks.push(instructions);
            result.push(Member {
                name: Arc::from(member.name.as_str()),
                code,
                local: member.local,
            });
        }
        let index = self.program.objects.len();
        self.program.objects.push(result);
        Ok(index)
    }

    fn expression(
        &mut self,
        expression: &crate::syntax::Expr,
        output: &mut Vec<Instruction>,
    ) -> Result<(), RuntimeError> {
        use crate::resolver::Binding;
        use crate::syntax::{BinaryOp, ExprKind};
        match &expression.kind {
            ExprKind::Int(value) => output.push(Instruction::Constant(Value::Int(*value))),
            ExprKind::String(value) => {
                output.push(Instruction::Constant(Value::string(value.as_str())))
            }
            ExprKind::Bool(value) => output.push(Instruction::Constant(Value::Bool(*value))),
            ExprKind::This => output.push(Instruction::LoadThis),
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
            ExprKind::Binary { op, left, right } => {
                self.expression(left, output)?;
                if self.options.fuse_constant_add
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
