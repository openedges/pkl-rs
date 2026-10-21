//! Lexical type substitutions for reusable constraint bytecode.
use super::*;

impl TypeCheck {
    pub(super) fn argument_source(&self, environment: &Environment) -> Option<TypeArgumentSource> {
        let TypeCheck::Parameter { block, name } = self.resolved() else {
            return None;
        };
        environment
            .type_argument_sources
            .get(&(*block, name.clone()))
            .cloned()
    }
    /// Resolve a finite type expression against the invocation that created it.
    /// Constraint bodies stay in Program; only their argument expressions are
    /// substituted, so changing recursive arguments never expand bytecode.
    pub(super) fn substituted(&self, environment: &Environment) -> Result<Self, RuntimeError> {
        use TypeCheck::*;
        let sub = |ty: &TypeCheck| ty.substituted(environment);
        Ok(match self {
            Shared(_) => self.clone(),
            Parameter { block, name } => environment
                .type_arguments
                .get(&(*block, name.clone()))
                .cloned()
                .ok_or_else(|| invalid("unbound generic constraint parameter"))?,
            NamedAlias {
                declaration,
                name,
                arguments,
                expanded,
            } => NamedAlias {
                declaration: *declaration,
                name: name.clone(),
                arguments: arguments.iter().map(sub).collect::<Result<_, _>>()?,
                expanded: Box::new(sub(expanded)?),
            },
            Function {
                parameters,
                result,
                syntax,
            } => Function {
                parameters: parameters
                    .as_ref()
                    .map(|args| args.iter().map(sub).collect::<Result<_, _>>())
                    .transpose()?,
                result: Box::new(sub(result)?),
                syntax: *syntax,
            },
            Reference { domain, referent } => Reference {
                domain: Box::new(sub(domain)?),
                referent: Box::new(sub(referent)?),
            },
            ParameterizedCollection { kind, arguments } => ParameterizedCollection {
                kind: *kind,
                arguments: arguments
                    .iter()
                    .map(|(ty, span)| Ok((sub(ty)?, *span)))
                    .collect::<Result<_, RuntimeError>>()?,
            },
            Nullable(inner) => Nullable(Box::new(sub(inner)?)),
            Alias(inner) => Alias(Box::new(sub(inner)?)),
            GenericAlias(inner) => GenericAlias(Box::new(sub(inner)?)),
            Union {
                types,
                spans,
                labels,
                default,
            } => Union {
                types: types.iter().map(sub).collect::<Result<_, _>>()?,
                spans: spans.clone(),
                labels: labels.clone(),
                default: *default,
            },
            Constrained {
                base,
                base_span,
                constraints,
            } => Constrained {
                base: Box::new(sub(base)?),
                base_span: *base_span,
                constraints: constraints
                    .iter()
                    .map(|constraint| {
                        let mut result = constraint.clone();
                        for (name, argument) in &constraint.arguments {
                            if let Some(source) = argument.argument_source(environment) {
                                result.argument_sources.insert(name.clone(), source);
                            }
                        }
                        result.arguments = constraint
                            .arguments
                            .iter()
                            .map(|(name, ty)| Ok((name.clone(), sub(ty)?)))
                            .collect::<Result<_, RuntimeError>>()?;
                        Ok(result)
                    })
                    .collect::<Result<_, RuntimeError>>()?,
            },
            _ => self.clone(),
        })
    }
}

impl TypeCheck {
    /// Whether selecting a default requires this invocation's actual argument.
    /// Nullable defaults retain their inner default for amendment. Constraints
    /// do not run while obtaining the base type's default.
    pub(super) fn default_depends_on_parameters(&self) -> bool {
        let mut ty = self;
        loop {
            ty = match ty.resolved() {
                Self::Parameter { .. } => return true,
                Self::NamedAlias { expanded, .. }
                | Self::Nullable(expanded)
                | Self::Alias(expanded)
                | Self::GenericAlias(expanded)
                | Self::Constrained { base: expanded, .. } => expanded,
                Self::Union {
                    types,
                    default: Some(index),
                    ..
                } => &types[*index],
                Self::ParameterizedCollection { kind, arguments }
                    if *kind != crate::heap::CollectionKind::List =>
                {
                    let Some((argument, _)) = arguments.last() else {
                        return false;
                    };
                    argument
                }
                _ => return false,
            };
        }
    }
}

enum DefaultTask {
    Visit(TypeCheck),
    Nullable,
    Collection(ObjectId),
    Publish(ObjectId),
    Constructor(TypeCheck, TypeArgumentSource),
}
pub(super) struct GenericDefaultRun {
    tasks: Vec<DefaultTask>,
    value: Option<Value>,
}
impl GenericDefaultRun {
    pub(super) fn constructor(ty: TypeCheck, source: TypeArgumentSource) -> Self {
        Self {
            tasks: vec![
                DefaultTask::Constructor(ty.clone(), source),
                DefaultTask::Visit(ty),
            ],
            value: None,
        }
    }
    pub(super) fn new(target: ObjectId, ty: TypeCheck) -> Self {
        Self {
            tasks: vec![DefaultTask::Publish(target), DefaultTask::Visit(ty)],
            value: None,
        }
    }
    pub(super) fn roots(&self, roots: &mut Vec<Value>) {
        roots.extend(self.value.iter().cloned());
        roots.extend(self.tasks.iter().filter_map(|task| match task {
            DefaultTask::Collection(id) | DefaultTask::Publish(id) => Some(Value::Object(*id)),
            DefaultTask::Visit(_) | DefaultTask::Nullable | DefaultTask::Constructor(_, _) => None,
        }));
    }
}
impl Vm {
    /// Default construction is VM work: each descent/allocation and each unwind
    /// consumes fuel, and every unpublished object remains traced between steps.
    pub(super) fn step_generic_default(&mut self) -> Result<(), RuntimeError> {
        use crate::heap::CollectionKind;
        let mut run = self
            .frames
            .last_mut()
            .expect("default frame")
            .generic_default
            .take()
            .expect("default run");
        match run
            .tasks
            .pop()
            .ok_or_else(|| invalid("empty default run"))?
        {
            DefaultTask::Constructor(ty, source) => {
                if let Some(value) = run.value {
                    self.stack.push(value);
                    return Ok(());
                }
                let mut base = ty.unaliased();
                while let TypeCheck::Constrained { base: inner, .. } = base {
                    base = inner.unaliased();
                }
                if let TypeCheck::Class { id, .. } = base
                    && let Some(class) = self.heap.find_class(*id)
                    && let Some(class) = &self.heap.get(class)?.class_definition
                    && class.abstract_
                {
                    let mut error = RuntimeError::new(
                        ErrorKind::Type,
                        format!(
                            "Cannot instantiate abstract class `{}#{}`.",
                            self.program.module_name, class.name
                        ),
                    );
                    error.hint = Some("Abstract classes cannot be instantiated.\nInstead, instantiate a concrete subclass.".into());
                    return Err(error.at(source.span));
                }
                if matches!(
                    base,
                    TypeCheck::Boolean
                        | TypeCheck::Int
                        | TypeCheck::Float
                        | TypeCheck::String
                        | TypeCheck::Bytes
                        | TypeCheck::Reference { .. }
                ) {
                    return Err(collections::external_amendment_error(&type_name(
                        base,
                        &self.program.module_name,
                    ))
                    .at(source.span));
                }
                return Err(RuntimeError::new(
                    ErrorKind::Type,
                    format!("Cannot instantiate type `{}`.", source.text),
                ));
            }
            DefaultTask::Nullable => {
                run.value = Some(self.heap.allocate_null(run.value.take()));
            }
            DefaultTask::Publish(target) => {
                self.heap.get_mut(target)?.collection_default = Some(run.value);
                return Ok(());
            }
            DefaultTask::Collection(id) => {
                self.heap.get_mut(id)?.collection_default = Some(run.value.take());
                run.value = Some(Value::Object(id));
            }
            DefaultTask::Visit(ty) => match ty.into_resolved() {
                TypeCheck::Null => run.value = Some(Value::Null),
                TypeCheck::Nullable(inner) => {
                    run.tasks.push(DefaultTask::Nullable);
                    run.tasks.push(DefaultTask::Visit(*inner));
                }
                TypeCheck::StringLiteral(value) => run.value = Some(Value::String(value)),
                TypeCheck::NamedAlias { expanded, .. }
                | TypeCheck::Alias(expanded)
                | TypeCheck::GenericAlias(expanded)
                | TypeCheck::Constrained { base: expanded, .. } => {
                    run.tasks.push(DefaultTask::Visit(*expanded))
                }
                TypeCheck::Union {
                    mut types,
                    default: Some(index),
                    ..
                } => run.tasks.push(DefaultTask::Visit(types.swap_remove(index))),
                TypeCheck::Class { id, .. } => {
                    let object = self
                        .heap
                        .find_class(id)
                        .ok_or_else(|| invalid("missing resolved default class"))?;
                    if self
                        .heap
                        .get(object)?
                        .class_definition
                        .as_ref()
                        .is_none_or(|class| class.abstract_)
                    {
                        run.value = None;
                    } else {
                        run.value = Some(Value::Object(object));
                    }
                }
                ty @ (TypeCheck::Collection(_) | TypeCheck::ParameterizedCollection { .. }) => {
                    let (kind, argument) = match ty {
                        TypeCheck::Collection(kind) => (kind, None),
                        TypeCheck::ParameterizedCollection {
                            kind,
                            mut arguments,
                        } => (kind, arguments.pop().map(|(ty, _)| ty)),
                        _ => unreachable!(),
                    };
                    let mut object = Object {
                        collection_kind: kind,
                        ..Object::default()
                    };
                    if kind != CollectionKind::List {
                        object.environment = self
                            .frames
                            .last()
                            .expect("default frame")
                            .environment
                            .clone();
                        object.specialized_literal = true;
                    }
                    let id = self.heap.allocate(object);
                    if kind != CollectionKind::List
                        && let Some(argument) = argument
                        && !argument.noop()
                    {
                        run.tasks.push(DefaultTask::Collection(id));
                        run.tasks.push(DefaultTask::Visit(argument));
                    } else {
                        run.value = Some(Value::Object(id));
                    }
                }
                TypeCheck::Parameter { .. } => {
                    return Err(invalid("unbound generic default parameter"));
                }
                _ => run.value = None,
            },
        }
        self.frames
            .last_mut()
            .expect("default frame")
            .generic_default = Some(run);
        Ok(())
    }
}

impl TypeCheck {
    pub(super) fn contains_parameter(&self) -> bool {
        let mut pending = vec![self];
        let mut visited = BTreeSet::new();
        while let Some(ty) = pending.pop() {
            let ty = ty.resolved();
            if !visited.insert(ty as *const Self) {
                continue;
            }
            if matches!(ty, Self::Parameter { .. }) {
                return true;
            }
            pending.extend(ty.type_children());
        }
        false
    }

    /// Structural metadata children, excluding executable constraint bodies.
    fn type_children(&self) -> Box<dyn Iterator<Item = &Self> + '_> {
        match self.resolved() {
            Self::NamedAlias {
                arguments,
                expanded,
                ..
            } => Box::new(arguments.iter().chain(std::iter::once(expanded.as_ref()))),
            Self::Function {
                parameters, result, ..
            } => Box::new(
                parameters
                    .iter()
                    .flatten()
                    .chain(std::iter::once(result.as_ref())),
            ),
            Self::Reference { domain, referent } => {
                Box::new([domain.as_ref(), referent.as_ref()].into_iter())
            }
            Self::Nullable(inner)
            | Self::Alias(inner)
            | Self::GenericAlias(inner)
            | Self::Constrained { base: inner, .. } => Box::new(std::iter::once(inner.as_ref())),
            Self::ParameterizedCollection { arguments, .. } => {
                Box::new(arguments.iter().map(|(ty, _)| ty))
            }
            Self::Union { types, .. } => Box::new(types.iter()),
            _ => Box::new(std::iter::empty()),
        }
    }

    pub(super) fn has_deferred_reference_validation(&self) -> bool {
        let mut pending = vec![self];
        let mut visited = BTreeSet::new();
        while let Some(ty) = pending.pop() {
            let ty = ty.resolved();
            if !visited.insert(ty as *const Self) {
                continue;
            }
            if matches!(ty, Self::Reference { referent, .. } if referent.contains_parameter()) {
                return true;
            }
            pending.extend(ty.type_children());
        }
        false
    }

    pub(super) fn validate_reference_arguments(
        &self,
        span: Span,
        aliases: &[RuntimeFrame],
        origins: &BTreeMap<usize, (Vec<RuntimeFrame>, Vec<RuntimeFrame>)>,
    ) -> Result<(), RuntimeError> {
        let mut pending = vec![(self, false)];
        let mut visited = BTreeSet::new();
        while let Some((ty, complete)) = pending.pop() {
            let ty = ty.resolved();
            if !complete {
                if !visited.insert(ty as *const Self) {
                    continue;
                }
                pending.push((ty, true));
                let children: Vec<_> = ty.type_children().collect();
                pending.extend(children.into_iter().rev().map(|child| (child, false)));
                continue;
            }
            if let Self::Reference { referent, .. } = ty
                && let Some(Self::Constrained { constraints, .. }) =
                    reference_constrained_type(referent)
            {
                let mut error = RuntimeError::new(
                    ErrorKind::Type,
                    "`Reference` referent type argument may not include type constraints.",
                )
                .at(span);
                // Keep the compiler's existing origin-frame policy for actual
                // arguments; do not fabricate source locations from their values.
                if let Some((mut frames, origin_aliases)) = constraints
                    .first()
                    .and_then(|constraint| origins.get(&constraint.block))
                    .cloned()
                {
                    if origin_aliases.starts_with(aliases) {
                        frames.truncate(origin_aliases.len().saturating_sub(aliases.len() + 1));
                    }
                    frames.retain(|frame| {
                        std::iter::once(span)
                            .chain(aliases.iter().map(|alias| alias.span))
                            .all(|usage| {
                                frame.span.start < usage.start || frame.span.end > usage.end
                            })
                    });
                    error.frames = frames;
                }
                return Err(error);
            }
        }
        Ok(())
    }
}

/// Runtime metadata has no value/heap edges and is immutable after substitution.
/// The optional Arc permits iterative last-owner reclamation, including concurrent
/// releases: Arc::into_inner guarantees exactly one releasing owner receives a node.
#[derive(Clone)]
pub struct SharedTypeCheck(Option<Arc<SharedNode>>);
struct SharedNode {
    ty: TypeCheck,
    noop: bool,
    needs_custom_this: bool,
}
impl std::fmt::Debug for SharedTypeCheck {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Debugging an Environment must not recursively print its argument DAG.
        f.debug_struct("SharedTypeCheck").finish_non_exhaustive()
    }
}
impl SharedTypeCheck {
    fn node(&self) -> &SharedNode {
        self.0.as_deref().expect("live shared type")
    }
    pub(super) fn noop(&self) -> bool {
        self.node().noop
    }
    pub(super) fn needs_custom_this(&self) -> bool {
        self.node().needs_custom_this
    }
}
impl Drop for SharedTypeCheck {
    fn drop(&mut self) {
        let Some(node) = self.0.take().and_then(Arc::into_inner) else {
            return;
        };
        let mut pending = vec![node.ty];
        while let Some(ty) = pending.pop() {
            match ty {
                TypeCheck::Shared(mut shared) => {
                    if let Some(node) = shared.0.take().and_then(Arc::into_inner) {
                        pending.push(node.ty);
                    }
                }
                TypeCheck::NamedAlias {
                    arguments,
                    expanded,
                    ..
                } => {
                    pending.extend(arguments);
                    pending.push(*expanded);
                }
                TypeCheck::Function {
                    parameters, result, ..
                } => {
                    pending.extend(parameters.into_iter().flatten());
                    pending.push(*result);
                }
                TypeCheck::Reference { domain, referent } => {
                    pending.push(*domain);
                    pending.push(*referent);
                }
                TypeCheck::Nullable(inner)
                | TypeCheck::Alias(inner)
                | TypeCheck::GenericAlias(inner) => pending.push(*inner),
                TypeCheck::ParameterizedCollection { arguments, .. } => {
                    pending.extend(arguments.into_iter().map(|(ty, _)| ty));
                }
                TypeCheck::Union { types, .. } => pending.extend(types),
                TypeCheck::Constrained {
                    base, constraints, ..
                } => {
                    pending.push(*base);
                    for constraint in constraints {
                        pending.extend(constraint.arguments.into_values());
                    }
                }
                _ => {}
            }
        }
    }
}
impl TypeCheck {
    pub(super) fn resolved(&self) -> &Self {
        let mut ty = self;
        while let Self::Shared(node) = ty {
            ty = &node.node().ty;
        }
        ty
    }
    pub(super) fn into_resolved(self) -> Self {
        match self {
            Self::Shared(node) => node.node().ty.clone(),
            ty => ty,
        }
    }
    pub(super) fn shared(self) -> Self {
        if matches!(self, Self::Shared(_)) {
            return self;
        }
        let noop = self.noop();
        let needs_custom_this = typecheck::needs_custom_this(&self);
        Self::Shared(SharedTypeCheck(Some(Arc::new(SharedNode {
            ty: self,
            noop,
            needs_custom_this,
        }))))
    }
}

#[cfg(test)]
mod tests;
