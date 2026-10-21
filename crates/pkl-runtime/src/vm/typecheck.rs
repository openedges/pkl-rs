//! Resumable type checking uses the same bytecode frames, Host suspension, and heap.
use super::*;
use crate::diagnostic::{PowerAssertionValue, TypeDiagnostic, UnionTypeDiagnostic};

#[derive(Clone, Copy)]
pub(super) enum TypeDestination {
    Stack,
    Test,
    Return,
}

#[derive(Clone)]
struct Observation {
    value: Value,
    executions: usize,
}
type Trace = BTreeMap<Span, Observation>;

#[derive(Clone)]
enum Mismatch {
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
        use crate::diagnostic_values::{render, type_name as actual_type};
        Ok(match self {
            Self::InsertedFrame { inner, .. } => inner.diagnostic(heap, module, indent)?,
            Self::Simple { span, ty, value } => TypeDiagnostic::Simple { span: *span, message: type_error_at_width(ty, value, module, heap, 80usize.saturating_sub(indent))?.message },
            Self::Constraint { span, value, body_span, observations } => TypeDiagnostic::Constraint {
                span: *span, value: render(value, heap, module, 80usize.saturating_sub(indent))?, body_span: *body_span,
                observations: observations.as_ref().map(|trace| trace.iter().map(|(span, observation)| Ok(PowerAssertionValue {
                    span: *span, value: render(&observation.value, heap, module, 100)?, executions: observation.executions,
                    hidden: matches!(observation.value, Value::Function(_)),
                })).collect::<Result<Vec<_>, RuntimeError>>()).transpose()?,
            },
            Self::Union { span, value, children } => TypeDiagnostic::Union {
                span: *span, value: render(value, heap, module, 80usize.saturating_sub(indent))?, actual_type: actual_type(value, heap, module)?, is_null: matches!(value, Value::Null),
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
            Self::InsertedFrame { inner, .. } => inner.roots(roots),
            Self::Simple { value, .. } => roots.push(value.clone()),
            Self::Constraint {
                value,
                observations,
                ..
            } => {
                roots.push(value.clone());
                if let Some(trace) = observations {
                    roots.extend(trace.values().map(|observation| observation.value.clone()));
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
enum Task {
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
    },
    TraceResult {
        span: Span,
        body_span: Span,
    },
    UnionResult(UnionState),
}

pub(super) struct TypeRun {
    value: Value,
    checked: Value,
    pub(super) environment: Environment,
    pub(super) member: Arc<str>,
    tasks: Vec<Task>,
    outcome: Option<Result<Value, Mismatch>>,
    pub(super) returned: Option<Value>,
    destination: TypeDestination,
    frames: Vec<RuntimeFrame>,
    in_type_test: bool,
    eager: bool,
    constraint_active: bool,
    tracker: Option<Trace>,
}
pub(super) fn environment_roots(environment: &Environment, roots: &mut Vec<Value>) {
    roots.extend(environment.locals.iter().cloned());
    roots.extend(environment.this_value.iter().cloned());
    roots.extend(environment.outer_value.iter().cloned());
    roots.extend(environment.custom_this.iter().cloned());
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
        roots.extend([self.value.clone(), self.checked.clone()]);
        roots.extend(self.returned.iter().cloned());
        environment_roots(&self.environment, roots);
        if let Some(trace) = &self.tracker {
            roots.extend(trace.values().map(|observation| observation.value.clone()));
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
                Task::PredicateResult { function, .. } => roots.push(Value::Function(*function)),
                _ => {}
            }
        }
    }
    fn mismatch(&mut self, span: Span, body_span: Span) {
        self.outcome = Some(Err(Mismatch::Constraint {
            span,
            value: self.value.clone(),
            body_span,
            observations: self.tracker.take(),
        }));
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
            .unwrap_or_else(|| Arc::from(""));
        frame.type_run = Some(TypeRun {
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
        });
        Ok(())
    }
    fn launch_constraint(
        &mut self,
        mut run: TypeRun,
        constraint: ConstraintCode,
    ) -> Result<(), RuntimeError> {
        let mut environment = if constraint.module_environment {
            module_environment(&run.environment)?
        } else {
            run.environment.clone()
        };
        environment.custom_this = Some(run.value.clone());
        let member = self
            .program
            .metadata
            .get(constraint.block)
            .filter(|metadata| metadata.constraint_root.is_some())
            .map_or_else(|| run.member.clone(), |metadata| metadata.member.clone());
        run.constraint_active = true;
        self.frames.last_mut().expect("type frame").type_run = Some(run);
        self.frames.last_mut().expect("type frame").callsite_hidden = true;
        self.frames.push(Frame {
            block: constraint.block,
            pc: 0,
            base: self.stack.len(),
            environment,
            force: None,
            lambda: false,
            type_run: None,
            equality_run: None,
            prepared_hashes: None,
            key_search: None,
            return_to_type: true,
            diagnostic_member: Some(member),
            callsite_hidden: false,
            builders: Vec::new(),
        });
        Ok(())
    }
    fn launch_predicate(
        &mut self,
        mut run: TypeRun,
        function: ObjectId,
    ) -> Result<(), RuntimeError> {
        let argument = run.value.clone();
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
                            TypeDiagnostic::Simple { message, .. } => message.as_str(),
                            _ => "Type constraint violated.",
                        },
                    );
                    error.span = Some(diagnostic.span());
                    error.frames = run.frames;
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
        match task {
            Task::Check { ty, span } => match ty {
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
                    run.tasks.push(Task::Check { ty: domain, span });
                }
                TypeCheck::Alias(inner) | TypeCheck::GenericAlias(inner) => {
                    let environment = module_environment(&run.environment)?;
                    run.tasks.push(Task::RestoreEnvironment(std::mem::replace(
                        &mut run.environment,
                        environment,
                    )));
                    run.tasks.push(Task::Check { ty: *inner, span });
                }
                TypeCheck::Nullable(inner) if !matches!(run.value, Value::Null) => {
                    run.tasks.push(Task::Check { ty: *inner, span })
                }
                TypeCheck::Union {
                    types,
                    spans,
                    labels,
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
                    run.tasks.push(Task::Check {
                        ty: types[0].clone(),
                        span: spans[0],
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
                        && (kind == crate::heap::CollectionKind::List
                            || run.eager
                            || kind == crate::heap::CollectionKind::Mapping)
                    {
                        let mut current = Some(id);
                        while let Some(owner) = current {
                            let object = self.heap.get(owner)?;
                            for member in &object.collection_members {
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
                    run.tasks.push(Task::Check {
                        ty: *base,
                        span: base_span,
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
                let TypeCheck::Reference { referent, .. } = &ty else {
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
                } else if state.kind == crate::heap::CollectionKind::Mapping
                    && !state.arguments[0].0.noop()
                {
                    run.value = state.items[state.index].member.key.clone();
                    run.checked = run.value.clone();
                    run.eager = true;
                    let (ty, span) = state.arguments[0].clone();
                    run.tasks.push(Task::CollectionKeyResult(state));
                    run.tasks.push(Task::Check { ty, span });
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
                } else if state.eager {
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
                run.tasks.push(Task::CollectionValueResult(state));
                run.tasks.push(Task::Check { ty, span });
            }
            Task::CollectionValueResult(mut state) => {
                if let Some(Ok(value)) = &run.outcome {
                    if state.kind == crate::heap::CollectionKind::List && *value != run.value {
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
            Task::AfterBase { constraints } => {
                if let Some(Ok(value)) = &run.outcome {
                    run.checked = value.clone();
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
                        });
                        return self.launch_constraint(run, constraint);
                    }
                    Value::Bool(false) => run.mismatch(constraint.span, constraint.span),
                    Value::Function(function) => {
                        let body_span = self
                            .heap
                            .get(function)?
                            .function
                            .as_ref()
                            .and_then(|function| self.program.metadata.get(function.block))
                            .and_then(|metadata| metadata.declaration.or(metadata.value_span))
                            .unwrap_or(constraint.span);
                        run.tasks.push(Task::PredicateResult {
                            constraints,
                            index,
                            body_span,
                            function,
                        });
                        return self.launch_predicate(run, function);
                    }
                    value => {
                        let error = if matches!(value, Value::Null) {
                            Ok(RuntimeError::new(
                                ErrorKind::Type,
                                "Expected value of type `Boolean` or `Function`, but got `null`.",
                            ))
                        } else {
                            (|| {
                                Ok(RuntimeError::new(
                                    ErrorKind::Type,
                                    format!(
                                        "Expected value of type `Boolean` or `Function`, but got type `{}`.\nValue: {}",
                                        crate::diagnostic_values::type_name(
                                            &value,
                                            &self.heap,
                                            &self.program.module_name
                                        )?,
                                        crate::diagnostic_values::render(
                                            &value,
                                            &self.heap,
                                            &self.program.module_name,
                                            80
                                        )?,
                                    ),
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
                        });
                        return self.launch_predicate(run, function);
                    }
                    Value::Bool(false) => {
                        self.frames.last_mut().expect("type frame").callsite_hidden = false;
                        run.mismatch(constraints[index].span, body_span);
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
            Task::TraceResult { span, body_span } => {
                run.constraint_active = false;
                self.frames.last_mut().expect("type frame").callsite_hidden = false;
                run.returned
                    .take()
                    .ok_or_else(|| invalid("missing trace result"))?;
                run.mismatch(span, body_span);
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
                    run.tasks.push(Task::UnionResult(state));
                    run.tasks.push(Task::Check { ty, span });
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
    fn visit(ty: &TypeCheck, kinds: &mut Vec<crate::heap::CollectionKind>) -> bool {
        match ty {
            TypeCheck::ParameterizedCollection { kind, .. } => {
                if kinds.contains(kind) {
                    true
                } else {
                    kinds.push(*kind);
                    false
                }
            }
            TypeCheck::GenericAlias(inner) => {
                let kind = match inner.unaliased() {
                    TypeCheck::ParameterizedCollection { kind, .. } => Some(*kind),
                    _ => None,
                };
                if let Some(kind) = kind {
                    if kinds.contains(&kind) {
                        return true;
                    }
                    kinds.push(kind);
                }
                visit(inner, kinds)
            }
            TypeCheck::Alias(inner) | TypeCheck::Nullable(inner) => visit(inner, kinds),
            TypeCheck::Constrained { base, .. } => visit(base, kinds),
            TypeCheck::Union { types, .. } => types.iter().any(|ty| visit(ty, kinds)),
            _ => false,
        }
    }
    let mut kinds = Vec::new();
    types.iter().any(|ty| visit(ty, &mut kinds))
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
        run.eager = state.eager;
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
        let result = if state.eager {
            Value::Object(id)
        } else if state.kind == crate::heap::CollectionKind::List {
            if state.replacements.is_empty() {
                Value::Object(id)
            } else {
                let mut object = self.heap.get(id)?.clone();
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
            if ty.noop()
                || self
                    .heap
                    .get(id)?
                    .collection_cast
                    .as_ref()
                    .is_some_and(|cast| cast.ty.equivalent(&ty))
            {
                Value::Object(id)
            } else {
                let object = self.heap.get(id)?;
                let mut environment = run.environment.clone();
                environment.locals.clear();
                let wrapper = Object {
                    parent: Some(id),
                    environment: object.environment.clone(),
                    collection_kind: state.kind,
                    length: object.length,
                    collection_cast: Some(crate::heap::CollectionCast {
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
