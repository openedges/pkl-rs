//! Resumable type checking uses the same bytecode frames, Host suspension, and heap.
use super::*;
use crate::diagnostic::{PowerAssertionValue, TypeDiagnostic, UnionTypeDiagnostic};

#[derive(Clone, Copy)]
pub(super) enum TypeDestination {
    Stack,
    Test,
    Return,
    Local {
        depth: Option<usize>,
        policy: identity::SlotPolicy,
    },
}

#[derive(Clone)]
enum ObservedValue {
    Runtime(Value),
    // Native constant folding can retain a value's canonical representation
    // without allocating a guest object (the pinned ModuleInfo regex binding).
    NativeLiteral(PklString),
}
impl ObservedValue {
    fn roots(&self) -> Option<Value> {
        match self {
            Self::Runtime(value) => Some(value.clone()),
            Self::NativeLiteral(_) => None,
        }
    }
    fn render(&self, heap: &Heap, module: &str) -> Result<PklString, RuntimeError> {
        match self {
            Self::Runtime(value) => {
                crate::diagnostic_values::render_string(value, heap, module, 100)
            }
            Self::NativeLiteral(text) => Ok(text.clone()),
        }
    }
}

#[derive(Clone)]
struct Observation {
    value: ObservedValue,
    executions: usize,
}
type Trace = BTreeMap<Span, Observation>;

#[derive(Clone)]
enum Mismatch {
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
        children: Vec<(Span, Mismatch)>,
    },
}
impl Mismatch {
    fn diagnostic(
        &self,
        heap: &Heap,
        module: &str,
        indent: usize,
    ) -> Result<TypeDiagnostic, RuntimeError> {
        use crate::diagnostic_values::{render_string as render, type_name as actual_type};
        Ok(match self {
            Self::InsertedFrame { inner, .. } | Self::AliasSource { inner, .. } => inner.diagnostic(heap, module, indent)?,
            Self::Simple { span, ty, value } => TypeDiagnostic::Simple { span: *span, message: type_error_at_width(ty, value, module, heap, 80usize.saturating_sub(indent))?.message },
            Self::Constraint { span, value, body_span, observations, .. } => TypeDiagnostic::Constraint {
                span: *span, value: render(value, heap, module, 80usize.saturating_sub(indent))?, body_span: *body_span,
                observations: observations.as_ref().map(|trace| trace.iter().map(|(span, observation)| Ok(PowerAssertionValue {
                    span: *span, value: observation.value.render(heap, module)?, executions: observation.executions,
                    hidden: matches!(observation.value, ObservedValue::Runtime(Value::Function(_))),
                })).collect::<Result<Vec<_>, RuntimeError>>()).transpose()?,
            },
            Self::Union { span, value, children } => TypeDiagnostic::Union {
                span: *span, value: render(value, heap, module, 80usize.saturating_sub(indent))?, actual_type: actual_type(value, heap, module)?, is_null: value.is_null(),
                children: children.iter().map(|(type_span, mismatch)| Ok(UnionTypeDiagnostic {
                    type_span: *type_span,
                    nontrivial: !matches!(mismatch, Mismatch::Simple { span, ty, .. } if span == type_span && !matches!(ty.unaliased(), TypeCheck::Nothing)),
                    diagnostic: Box::new(mismatch.diagnostic(heap, module, indent + 2)?),
                })).collect::<Result<Vec<_>, RuntimeError>>()?,
            },
        })
    }
    fn roots(&self, roots: &mut Vec<Value>) {
        match self {
            Self::InsertedFrame { inner, .. } | Self::AliasSource { inner, .. } => {
                inner.roots(roots)
            }
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
                for (_, mismatch) in children {
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
    span: Span,
    index: usize,
    failures: Vec<(Span, Mismatch)>,
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
    replacements: Vec<(Arc<str>, Value)>,
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
    RestoreEnvironment(Environment),
    RestoreTypeNode(Option<TypeNodeId>),
    AfterBase {
        constraints: Vec<ConstraintCode>,
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
                    for (_, mismatch) in &state.failures {
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
    fn mismatch(&mut self, span: Span, body_span: Span, value: Value) {
        self.outcome = Some(Err(Mismatch::Constraint {
            source: self.constraint_source.clone(),
            span,
            value,
            body_span,
            observations: self.tracker.take(),
        }));
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
    result.scopes = vec![scope];
    // TypeAliasTypeNode swaps receiver and owner in the original frame, preserving locals.
    result.this_value = Some(Value::Object(scope.receiver));
    result.outer_value = None;
    Ok(result)
}

impl Vm {
    pub(super) fn observe(&mut self, span: Span, value: Value) {
        self.observe_value(span, ObservedValue::Runtime(value));
    }
    pub(super) fn observe_native_literal(&mut self, span: Span, rendered: PklString) {
        self.observe_value(span, ObservedValue::NativeLiteral(rendered));
    }
    fn observe_value(&mut self, span: Span, value: ObservedValue) {
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
        self.frames.iter().any(|frame| {
            frame
                .type_run
                .as_ref()
                .is_some_and(|run| run.tracker.is_some())
        })
    }
    pub(super) fn begin_type(
        &mut self,
        ty: TypeCheck,
        value: Value,
        destination: TypeDestination,
        frames: Vec<RuntimeFrame>,
    ) -> Result<(), RuntimeError> {
        let ty = ty.substituted(
            &self
                .frames
                .last()
                .ok_or_else(|| invalid("missing type-check frame"))?
                .environment,
        )?;
        let node = if needs_custom_this(&ty) {
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
        frame.type_run = Some(TypeRun {
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
        });
        Ok(())
    }
    fn push_type_child(
        &mut self,
        run: &mut TypeRun,
        ty: TypeCheck,
        span: Span,
        index: usize,
    ) -> Result<(), RuntimeError> {
        if needs_custom_this(&ty) {
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
        mut run: TypeRun,
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
                environment.scopes = vec![Scope {
                    receiver: module,
                    owner: module,
                }];
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
                        .insert((constraint.block, Arc::from(name.as_str())), source);
                } else {
                    environment
                        .type_argument_sources
                        .remove(&(constraint.block, Arc::from(name.as_str())));
                }
            } else {
                environment
                    .type_argument_sources
                    .remove(&(constraint.block, Arc::from(name.as_str())));
            }
            // VmTypeAlias.instantiate visits the cloned child tree, but a let
            // expression's call target is outside that tree. Its type variables
            // retain the alias declaration's unbound (unknown) interpretation.
            let argument = if shared_root {
                TypeCheck::Unknown
            } else {
                argument.substituted(&run.environment)?
            };
            environment.type_arguments.insert(
                (constraint.block, Arc::from(name.as_str())),
                argument.shared(),
            );
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
        run.constraint_source = self
            .program
            .metadata
            .get(constraint.block)
            .and_then(|metadata| metadata.source.clone());
        self.frames.last_mut().expect("type frame").type_run = Some(run);
        self.frames.last_mut().expect("type frame").callsite_hidden = true;
        self.frames.push(Frame {
            guest_catch: None,
            block: constraint.block,
            pc: 0,
            base: self.stack.len(),
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
            int_seq_run: None,
            function_owner: None,
            amendment_arguments: None,
            amendment_end: None,
            prepared_hashes: None,
            key_search: None,
            return_to_type: true,
            diagnostic_member: Some(member),
            callsite_hidden: false,
            builders: Vec::new(),
            generator_scopes: Vec::new(),
            module_load: None,
            uri_resolution: None,
            module_receiver: None,
        });
        Ok(())
    }
    fn launch_predicate(
        &mut self,
        mut run: TypeRun,
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
        run: TypeRun,
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
            let outcome = run
                .outcome
                .take()
                .ok_or_else(|| invalid("missing type result"))?;
            match (outcome, run.destination) {
                (Ok(_), TypeDestination::Test) => self.stack.push(Value::Bool(true)),
                (Err(_), TypeDestination::Test) => self.stack.push(Value::Bool(false)),
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
                    let mut error = RuntimeError::new(
                        ErrorKind::Type,
                        match &diagnostic {
                            TypeDiagnostic::Simple { message, .. } => message.clone(),
                            _ => "Type constraint violated.".into(),
                        },
                    );
                    error.span = Some(diagnostic.span());
                    error.frames = run.frames;
                    if let Mismatch::Constraint {
                        source: Some(source),
                        ..
                    }
                    | Mismatch::AliasSource { source, .. } = &mismatch
                    {
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
                            source.clone()
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
                                source: Some(source),
                                span: diagnostic.span(),
                                member: run.member.clone(),
                            });
                        }
                        caller.callsite_hidden = true;
                    }
                    if let Mismatch::InsertedFrame { frame, .. } = &mismatch {
                        if error.frames.is_empty() {
                            error.frames.push(RuntimeFrame {
                                source: None,
                                span: diagnostic.span(),
                                member: run.member.clone(),
                            });
                        }
                        error.frames.insert(1, frame.clone());
                        self.frames.last_mut().expect("type frame").callsite_hidden = true;
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
                let Some(resolved) = self.resolve_runtime_type(&ty, &environment)? else {
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
        match task {
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
                    let span = if matches!(
                        expanded.unaliased(),
                        TypeCheck::Union {
                            string_literals: true,
                            ..
                        }
                    ) && let Some(origin) = default_origin
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
                    run.tasks.push(Task::RestoreEnvironment(std::mem::replace(
                        &mut run.environment,
                        environment,
                    )));
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
                            let members = if kind == crate::heap::CollectionKind::Map {
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
                    run.tasks.push(Task::AfterBase { constraints });
                    self.push_type_child(&mut run, *base, base_span, 0)?;
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
            Task::RestoreEnvironment(environment) => run.environment = environment,
            Task::RestoreTypeNode(node) => run.node = node,
            Task::AfterBase { constraints } => {
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
                        run.mismatch(constraint.span, constraint.span, value);
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
                                Ok(RuntimeError::new(
                                    ErrorKind::Type,
                                    PklString::join(&["Expected value of type `Boolean` or `Function`, but got type `".into(),
(crate::diagnostic_values::type_name(
                                            &value,
                                            &self.heap,
                                            &self.program.module_name
                                        )?).into(),
"`.\nValue: ".into(),
crate::diagnostic_values::render(
                                            &value,
                                            &self.heap,
                                            &self.program.module_name,
                                            80
                                        )?], ""),
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
                        run.mismatch(constraints[index].span, body_span, predicate_value);
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
                run.mismatch(span, body_span, value);
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
                            state.failures[state.index] = (state.labels[state.index], mismatch);
                        } else {
                            state.failures.push((state.labels[state.index], mismatch));
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
        run: TypeRun,
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
                Value::Object(self.heap.allocate(object))
            }
        } else {
            let (ty, span) = state.arguments.last().expect("value type").clone();
            let node = if needs_custom_this(&ty) {
                let parent = run
                    .node
                    .ok_or_else(|| invalid("missing collection type node"))?;
                Some(TypeNodeId(self.heap.executable_child(
                    parent.0,
                    ExecutionSite::TypeChild(state.arguments.len() - 1),
                )?))
            } else {
                None
            };
            if ty.noop()
                || self
                    .heap
                    .get(id)?
                    .collection_cast
                    .as_ref()
                    .is_some_and(|cast| {
                        // ConstrainedTypeNode is equivalent only to itself. Shared
                        // bytecode (especially let roots) is not type-node identity.
                        cast.ty.equivalent(&ty) && (node.is_none() || cast.node == node)
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
