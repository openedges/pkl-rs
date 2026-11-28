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
        enum Work<'a> {
            Visit(&'a TypeCheck, usize),
            WrapShared {
                ptr: usize,
                is_parameter: bool,
                section: Option<crate::type_source::TypeSourceSection>,
            },
            RebuildDeclared {
                recipe: usize,
                count: usize,
            },
            RebuildNamedAlias {
                declaration: usize,
                name: PklString,
                default_origin: Option<Box<RuntimeFrame>>,
                arg_count: usize,
            },
            RebuildFunction {
                param_count: Option<usize>,
                syntax: bool,
            },
            RebuildReference,
            RebuildParameterized {
                kind: CollectionKind,
                spans: Vec<Span>,
            },
            RebuildPair {
                spans: Vec<Span>,
            },
            RebuildCollectionBase {
                spans: Vec<Span>,
            },
            RebuildVarArgs {
                span: Span,
            },
            RebuildNullable,
            RebuildAlias,
            RebuildGenericAlias,
            RebuildUnion {
                string_literals: bool,
                count: usize,
                spans: Vec<Span>,
                labels: Vec<Span>,
                default: Option<usize>,
            },
            RebuildConstrained {
                base_span: Span,
                constraints: Vec<ConstraintCode>,
                arg_counts: Vec<(usize, Vec<PklString>)>,
            },
        }

        let mut work = vec![Work::Visit(self, 0)];
        let mut values: Vec<TypeCheck> = Vec::new();
        let mut memo: crate::heap::FastMap<usize, TypeCheck> = crate::heap::FastMap::default();
        let mut expanded_nodes: usize = 0;

        while let Some(step) = work.pop() {
            match step {
                Work::Visit(cur, depth) => {
                    if depth > MAX_TYPE_COMPILE_DEPTH {
                        return Err(RuntimeError::new(
                            ErrorKind::Type,
                            "Type expression exceeds maximum nesting depth.",
                        ));
                    }
                    expanded_nodes += 1;
                    if expanded_nodes > MAX_TYPE_COMPILE_NODES {
                        return Err(RuntimeError::new(
                            ErrorKind::Type,
                            "Type expression exceeds maximum expansion complexity.",
                        ));
                    }
                    let next_depth = depth + 1;
                    match cur {
                        Shared(node) if !node.node().has_parameters => values.push(cur.clone()),
                        Shared(node) => {
                            let ptr = node.as_ptr_id();
                            if let Some(cached) = memo.get(&ptr) {
                                values.push(cached.clone());
                            } else {
                                work.push(Work::WrapShared {
                                    ptr,
                                    is_parameter: matches!(node.node().ty, Parameter { .. }),
                                    section: node.node().source_section.clone(),
                                });
                                work.push(Work::Visit(&node.node().ty, next_depth));
                            }
                        }
                        Declared { recipe, arguments } => {
                            work.push(Work::RebuildDeclared {
                                recipe: *recipe,
                                count: arguments.len(),
                            });
                            for arg in arguments.iter().rev() {
                                work.push(Work::Visit(arg, next_depth));
                            }
                        }
                        Parameter { block, name } => {
                            let resolved = environment
                                .type_arguments
                                .get(&(*block, name.clone()))
                                .cloned()
                                .ok_or_else(|| invalid("unbound generic constraint parameter"))?;
                            values.push(resolved);
                        }
                        NamedAlias {
                            declaration,
                            name,
                            arguments,
                            expanded,
                            default_origin,
                        } => {
                            work.push(Work::RebuildNamedAlias {
                                declaration: *declaration,
                                name: name.clone(),
                                default_origin: default_origin.clone(),
                                arg_count: arguments.len(),
                            });
                            work.push(Work::Visit(expanded, next_depth));
                            for arg in arguments.iter().rev() {
                                work.push(Work::Visit(arg, next_depth));
                            }
                        }
                        Function {
                            parameters,
                            result,
                            syntax,
                        } => {
                            work.push(Work::RebuildFunction {
                                param_count: parameters.as_ref().map(Vec::len),
                                syntax: *syntax,
                            });
                            work.push(Work::Visit(result, next_depth));
                            if let Some(params) = parameters {
                                for p in params.iter().rev() {
                                    work.push(Work::Visit(p, next_depth));
                                }
                            }
                        }
                        Reference { domain, referent } => {
                            work.push(Work::RebuildReference);
                            work.push(Work::Visit(referent, next_depth));
                            work.push(Work::Visit(domain, next_depth));
                        }
                        ParameterizedCollection { kind, arguments } => {
                            work.push(Work::RebuildParameterized {
                                kind: *kind,
                                spans: arguments.iter().map(|(_, s)| *s).collect(),
                            });
                            for (ty, _) in arguments.iter().rev() {
                                work.push(Work::Visit(ty, next_depth));
                            }
                        }
                        Pair { arguments } => {
                            work.push(Work::RebuildPair {
                                spans: arguments.iter().map(|(_, s)| *s).collect(),
                            });
                            for (ty, _) in arguments.iter().rev() {
                                work.push(Work::Visit(ty, next_depth));
                            }
                        }
                        CollectionBase { arguments } => {
                            work.push(Work::RebuildCollectionBase {
                                spans: arguments.iter().map(|(_, s)| *s).collect(),
                            });
                            for (ty, _) in arguments.iter().rev() {
                                work.push(Work::Visit(ty, next_depth));
                            }
                        }
                        VarArgs { element, span } => {
                            work.push(Work::RebuildVarArgs { span: *span });
                            work.push(Work::Visit(element, next_depth));
                        }
                        Nullable(inner) => {
                            work.push(Work::RebuildNullable);
                            work.push(Work::Visit(inner, next_depth));
                        }
                        Alias(inner) => {
                            work.push(Work::RebuildAlias);
                            work.push(Work::Visit(inner, next_depth));
                        }
                        GenericAlias(inner) => {
                            work.push(Work::RebuildGenericAlias);
                            work.push(Work::Visit(inner, next_depth));
                        }
                        Union {
                            string_literals,
                            types,
                            spans,
                            labels,
                            default,
                        } => {
                            work.push(Work::RebuildUnion {
                                string_literals: *string_literals,
                                count: types.len(),
                                spans: spans.clone(),
                                labels: labels.clone(),
                                default: *default,
                            });
                            for item in types.iter().rev() {
                                work.push(Work::Visit(item, next_depth));
                            }
                        }
                        Constrained {
                            base,
                            base_span,
                            constraints,
                        } => {
                            let mut cloned_constraints = Vec::with_capacity(constraints.len());
                            let mut arg_counts = Vec::with_capacity(constraints.len());
                            let mut to_visit: Vec<&TypeCheck> = Vec::new();
                            for (ci, constraint) in constraints.iter().enumerate() {
                                let mut result = constraint.clone();
                                let mut names = Vec::with_capacity(constraint.arguments.len());
                                for (name, argument) in &constraint.arguments {
                                    if let Some(source) = argument.argument_source(environment) {
                                        result.argument_sources.insert(name.clone(), source);
                                    }
                                    names.push(name.clone());
                                    to_visit.push(argument);
                                }
                                result.arguments.clear();
                                arg_counts.push((ci, names));
                                cloned_constraints.push(result);
                            }
                            work.push(Work::RebuildConstrained {
                                base_span: *base_span,
                                constraints: cloned_constraints,
                                arg_counts,
                            });
                            for arg in to_visit.into_iter().rev() {
                                work.push(Work::Visit(arg, next_depth));
                            }
                            work.push(Work::Visit(base, next_depth));
                        }
                        _ => values.push(cur.clone()),
                    }
                }
                Work::WrapShared {
                    ptr,
                    is_parameter,
                    section,
                } => {
                    let value = values.pop().expect("shared substituted value");
                    let wrapped = if is_parameter {
                        value
                    } else {
                        value.with_optional_source_section(section)
                    };
                    let shared = wrapped.shared();
                    memo.insert(ptr, shared.clone());
                    values.push(shared);
                }
                Work::RebuildDeclared { recipe, count } => {
                    let arguments = values.split_off(values.len() - count);
                    values.push(Declared { recipe, arguments });
                }
                Work::RebuildNamedAlias {
                    declaration,
                    name,
                    default_origin,
                    arg_count,
                } => {
                    let expanded = Box::new(values.pop().expect("named alias expanded"));
                    let arguments = values.split_off(values.len() - arg_count);
                    values.push(NamedAlias {
                        declaration,
                        default_origin,
                        name,
                        arguments,
                        expanded,
                    });
                }
                Work::RebuildFunction {
                    param_count,
                    syntax,
                } => {
                    let result = Box::new(values.pop().expect("function return"));
                    let parameters = param_count.map(|c| values.split_off(values.len() - c));
                    values.push(Function {
                        parameters,
                        result,
                        syntax,
                    });
                }
                Work::RebuildReference => {
                    let referent = Box::new(values.pop().expect("reference referent"));
                    let domain = Box::new(values.pop().expect("reference domain"));
                    values.push(Reference { domain, referent });
                }
                Work::RebuildParameterized { kind, spans } => {
                    let tys = values.split_off(values.len() - spans.len());
                    let arguments = tys.into_iter().zip(spans).collect();
                    values.push(ParameterizedCollection { kind, arguments });
                }
                Work::RebuildPair { spans } => {
                    let tys = values.split_off(values.len() - spans.len());
                    let arguments = tys.into_iter().zip(spans).collect();
                    values.push(Pair { arguments });
                }
                Work::RebuildCollectionBase { spans } => {
                    let tys = values.split_off(values.len() - spans.len());
                    let arguments = tys.into_iter().zip(spans).collect();
                    values.push(CollectionBase { arguments });
                }
                Work::RebuildVarArgs { span } => {
                    let element = Box::new(values.pop().expect("varargs element"));
                    values.push(VarArgs { element, span });
                }
                Work::RebuildNullable => {
                    let inner = Box::new(values.pop().expect("nullable inner"));
                    values.push(Nullable(inner));
                }
                Work::RebuildAlias => {
                    let inner = Box::new(values.pop().expect("alias inner"));
                    values.push(Alias(inner));
                }
                Work::RebuildGenericAlias => {
                    let inner = Box::new(values.pop().expect("generic alias inner"));
                    values.push(GenericAlias(inner));
                }
                Work::RebuildUnion {
                    string_literals,
                    count,
                    spans,
                    labels,
                    default,
                } => {
                    let types = values.split_off(values.len() - count);
                    values.push(Union {
                        string_literals,
                        types,
                        spans,
                        labels,
                        default,
                    });
                }
                Work::RebuildConstrained {
                    base_span,
                    mut constraints,
                    arg_counts,
                } => {
                    let total_args: usize = arg_counts.iter().map(|(_, ns)| ns.len()).sum();
                    let mut arg_iter = values.split_off(values.len() - total_args).into_iter();
                    let base = Box::new(values.pop().expect("constrained base"));
                    for (ci, names) in arg_counts {
                        for name in names {
                            let ty = arg_iter.next().expect("constraint argument");
                            constraints[ci].arguments.insert(name, ty);
                        }
                    }
                    values.push(Constrained {
                        base,
                        base_span,
                        constraints,
                    });
                }
            }
        }
        Ok(values.pop().expect("substituted type"))
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
                    if !matches!(
                        kind,
                        crate::heap::CollectionKind::List
                            | crate::heap::CollectionKind::Map
                            | crate::heap::CollectionKind::Set
                    ) =>
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
    Stack,
    Probe,
    Visit(TypeCheck),
    Nullable,
    Collection(ObjectId),
    Publish(ObjectId),
    Constructor(TypeCheck, TypeArgumentSource, Option<(usize, usize)>),
    InferredParent(Option<(usize, usize)>),
}
pub(super) struct GenericDefaultRun {
    tasks: Vec<DefaultTask>,
    value: Option<Value>,
    origin: Option<RuntimeFrame>,
}
impl GenericDefaultRun {
    /// DefaultPropertyBodyNode.isUndefined probes the same type-default logic
    /// without turning absence into an undefined-property exception.
    pub(super) fn probe(ty: TypeCheck) -> Self {
        Self {
            tasks: vec![DefaultTask::Probe, DefaultTask::Visit(ty)],
            value: None,
            origin: None,
        }
    }

    pub(super) fn stack(ty: TypeCheck, origin: Option<RuntimeFrame>) -> Self {
        Self {
            tasks: vec![DefaultTask::Stack, DefaultTask::Visit(ty)],
            value: None,
            origin,
        }
    }
    pub(super) fn inferred_parent(
        ty: TypeCheck,
        origin: Option<RuntimeFrame>,
        cache: Option<(usize, usize)>,
    ) -> Self {
        Self {
            tasks: vec![DefaultTask::InferredParent(cache), DefaultTask::Visit(ty)],
            value: None,
            origin,
        }
    }
    pub(super) fn constructor(
        ty: TypeCheck,
        source: TypeArgumentSource,
        origin: Option<RuntimeFrame>,
        cache: Option<(usize, usize)>,
    ) -> Self {
        Self {
            tasks: vec![
                DefaultTask::Constructor(ty.clone(), source, cache),
                DefaultTask::Visit(ty),
            ],
            value: None,
            origin,
        }
    }
    pub(super) fn new(target: ObjectId, ty: TypeCheck, origin: Option<RuntimeFrame>) -> Self {
        Self {
            tasks: vec![DefaultTask::Publish(target), DefaultTask::Visit(ty)],
            value: None,
            origin,
        }
    }
    pub(super) fn roots(&self, roots: &mut Vec<Value>) {
        roots.extend(self.value.iter().cloned());
        roots.extend(self.tasks.iter().filter_map(|task| match task {
            DefaultTask::Collection(id) | DefaultTask::Publish(id) => Some(Value::Object(*id)),
            DefaultTask::Stack
            | DefaultTask::Probe
            | DefaultTask::Visit(_)
            | DefaultTask::Nullable
            | DefaultTask::Constructor(_, _, _)
            | DefaultTask::InferredParent(_) => None,
        }));
    }
}
impl Vm {
    /// Default construction is VM work: each descent/allocation and each unwind
    /// consumes fuel, and every unpublished object remains traced between steps.
    #[cfg_attr(target_arch = "wasm32", inline(never))]
    pub(super) fn step_generic_default(&mut self) -> Result<(), RuntimeError> {
        use crate::heap::CollectionKind;
        let mut run = self
            .frames
            .last_mut()
            .expect("default frame")
            .generic_default
            .take()
            .expect("default run");
        let task = run
            .tasks
            .pop()
            .ok_or_else(|| invalid("empty default run"))?;
        let task = if let DefaultTask::Visit(ty) = task {
            if ty.has_declared() {
                let environment = self
                    .frames
                    .last()
                    .expect("default frame")
                    .environment
                    .clone();
                run.tasks.push(DefaultTask::Visit(ty.clone()));
                self.frames
                    .last_mut()
                    .expect("default frame")
                    .generic_default = Some(run);
                let Some(resolved) = self.resolve_runtime_type(&ty, &environment, true)? else {
                    return Ok(());
                };
                run = self
                    .frames
                    .last_mut()
                    .expect("default frame")
                    .generic_default
                    .take()
                    .expect("retained default run");
                run.tasks.pop();
                DefaultTask::Visit(resolved)
            } else {
                DefaultTask::Visit(ty)
            }
        } else {
            task
        };
        match task {
            DefaultTask::Probe => {
                self.stack.push(Value::Bool(run.value.is_some()));
                return Ok(());
            }
            DefaultTask::Stack => {
                let value = run.value.ok_or_else(|| {
                    let name = self
                        .frames
                        .last()
                        .and_then(|frame| frame.force.as_ref())
                        .map_or_else(PklString::default, |force| force.name.clone());
                    let mut error = RuntimeError::new(
                        ErrorKind::MissingMember,
                        PklString::from("Tried to read property `")
                            .concat(&name)
                            .concat(&"` but its value is undefined.".into()),
                    )
                    .undefined();
                    // DefaultPropertyBodyNode reports the property name after
                    // deferred alias resolution finds no default. The type's
                    // source remains the origin for failures resolving it.
                    error.span = self
                        .frames
                        .last()
                        .and_then(|frame| self.program.metadata[frame.block].value_span);
                    error
                })?;
                self.stack.push(value);
                return Ok(());
            }
            DefaultTask::InferredParent(cache) => {
                let value = run.value.ok_or_else(cannot_infer_parent)?;
                if let Some(key) = cache {
                    self.type_parent_defaults.insert(key, value.clone());
                    self.declaration_roots_dirty = true;
                }
                self.stack.push(value);
                return Ok(());
            }
            DefaultTask::Constructor(ty, source, cache) => {
                if let Some(value) = run.value {
                    if let Some(key) = cache {
                        self.type_parent_defaults.insert(key, value.clone());
                        self.declaration_roots_dirty = true;
                    }
                    self.stack.push(value);
                    return Ok(());
                }
                // TypeAliasTypeNode delegates getVmClass, while composite
                // nodes (including ConstrainedTypeNode) do not expose a class.
                let base = ty.unaliased();
                let class = if let Some(class) = classes::BuiltinClass::from_type(base) {
                    Some(self.canonical_builtin_class(class))
                } else if let TypeCheck::Class { id, .. } = base {
                    self.heap.find_class(*id)
                } else {
                    None
                };
                if let Some(class) = class {
                    let declaration = self.heap.class_definition(class)?;
                    if declaration.abstract_ {
                        let mut error = RuntimeError::new(
                            ErrorKind::Type,
                            PklString::from("Cannot instantiate abstract class `")
                                .concat(&self.heap.class_display_name(class)?)
                                .concat(&"`.".into()),
                        );
                        error.hint = Some("Abstract classes cannot be instantiated.\nInstead, instantiate a concrete subclass.".into());
                        return Err(error.at(source.span));
                    }
                    if self.class_external(class)? {
                        return Err(collections::external_amendment_error(
                            &self.heap.class_display_name(class)?,
                        )
                        .at(source.span));
                    }
                }
                return Err(RuntimeError::new(
                    ErrorKind::Type,
                    PklString::from("Cannot instantiate type `")
                        .concat(source.text.original())
                        .concat(&"`.".into()),
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
                TypeCheck::VarArgs { .. } => {
                    let mut error = internal_varargs_error();
                    error.span = run.origin.as_ref().map(|origin| origin.span);
                    return Err(error);
                }
                TypeCheck::Null => run.value = Some(Value::Null),
                TypeCheck::Nullable(inner) => {
                    run.tasks.push(DefaultTask::Nullable);
                    run.tasks.push(DefaultTask::Visit(*inner));
                }
                TypeCheck::StringLiteral(value) => run.value = Some(Value::String(value)),
                TypeCheck::NamedAlias {
                    declaration,
                    arguments,
                    default_origin,
                    ..
                } if amendment::is_base_mixin_alias(
                    declaration,
                    self.program.base_mixin_declaration,
                ) =>
                {
                    let mut origin = default_origin.map(|o| *o).or_else(|| run.origin.clone());
                    if let (Some(origin), Some(caller)) = (&mut origin, &run.origin) {
                        let caller_name = crate::source::SourceText::from(&caller.member);
                        origin.member = caller_name
                            .entire()
                            .strip_suffix(".default")
                            .unwrap_or(caller_name.entire())
                            .to_pkl_string();
                    }
                    let construction_origin = run.tasks.iter().rev().find_map(|task| {
                        let DefaultTask::Constructor(_, source, _) = task else {
                            return None;
                        };
                        origin.as_ref().map(|origin| RuntimeFrame {
                            anonymous: origin.anonymous,
                            source: origin.source.clone(),
                            span: source.span,
                            member: origin.member.clone(),
                        })
                    });
                    run.value = Some(self.identity_mixin(
                        arguments.into_iter().next().unwrap_or(TypeCheck::Unknown),
                        origin,
                        construction_origin,
                    ));
                }
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
                ty @ TypeCheck::Class { .. }
                    if matches!(
                        classes::BuiltinClass::from_type(&ty),
                        Some(classes::BuiltinClass::DataSize | classes::BuiltinClass::IntSeq)
                    ) =>
                {
                    run.value = None;
                }
                ty @ TypeCheck::Class { .. } => {
                    let TypeCheck::Class { id, .. } = &ty else {
                        unreachable!()
                    };
                    // Builtin type defaults share the canonical class graph;
                    // user classes have already been initialized by resolution.
                    let object = if let Some(class) = classes::BuiltinClass::from_type(&ty) {
                        self.canonical_builtin_class(class)
                    } else {
                        self.heap
                            .find_class(*id)
                            .ok_or_else(|| invalid("missing resolved default class"))?
                    };
                    let external = self.class_external(object)?;
                    let object = self.heap.get_class(object)?.prototype;
                    if external
                        || self
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
                TypeCheck::CollectionBase { .. } => {
                    run.value = Some(Value::Object(
                        self.empty_strict_collection(CollectionKind::List),
                    ));
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
                    if matches!(
                        kind,
                        CollectionKind::List | CollectionKind::Map | CollectionKind::Set
                    ) {
                        run.value = Some(Value::Object(self.empty_strict_collection(kind)));
                    } else {
                        let parent = self.empty_object(kind);
                        if let Some(argument) = argument
                            && !matches!(argument.resolved(), TypeCheck::Unknown)
                        {
                            let id = self.heap.allocate(Object {
                                parent: Some(parent),
                                collection_kind: kind,
                                collection_default_origin: run.origin.clone().map(Box::new),
                                environment: self
                                    .frames
                                    .last()
                                    .expect("default frame")
                                    .environment
                                    .clone(),
                                specialized_literal: true,
                                ..Object::default()
                            });
                            run.tasks.push(DefaultTask::Collection(id));
                            run.tasks.push(DefaultTask::Visit(argument));
                        } else {
                            run.value = Some(Value::Object(parent));
                        }
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
    pub(super) fn type_children(&self) -> Box<dyn Iterator<Item = &Self> + '_> {
        match self.resolved() {
            Self::Declared { arguments, .. } => Box::new(arguments.iter()),
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
            Self::VarArgs { element: inner, .. }
            | Self::Nullable(inner)
            | Self::Alias(inner)
            | Self::GenericAlias(inner)
            | Self::Constrained { base: inner, .. } => Box::new(std::iter::once(inner.as_ref())),
            Self::ParameterizedCollection { arguments, .. }
            | Self::Pair { arguments }
            | Self::CollectionBase { arguments } => Box::new(arguments.iter().map(|(ty, _)| ty)),
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
    needs_type_identity: bool,
    has_declared: bool,
    source_section: Option<crate::type_source::TypeSourceSection>,
    has_parameters: bool,
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
    pub(super) fn as_ptr_id(&self) -> usize {
        self.0.as_ref().map_or(0, |a| Arc::as_ptr(a) as usize)
    }
    pub(super) fn noop(&self) -> bool {
        self.node().noop
    }
    pub(super) fn has_declared(&self) -> bool {
        self.node().has_declared
    }
    pub(super) fn needs_custom_this(&self) -> bool {
        self.node().needs_custom_this
    }
    pub(super) fn needs_type_identity(&self) -> bool {
        self.node().needs_type_identity
    }
}
impl Drop for SharedTypeCheck {
    fn drop(&mut self) {
        let Some(node) = self.0.take().and_then(Arc::into_inner) else {
            return;
        };
        match &node.ty {
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
            | TypeCheck::Parameter { .. } => return,
            TypeCheck::Declared { arguments, .. } if arguments.is_empty() => return,
            _ => {}
        }
        let mut pending = vec![node.ty];
        while let Some(ty) = pending.pop() {
            match ty {
                TypeCheck::Shared(mut shared) => {
                    if let Some(node) = shared.0.take().and_then(Arc::into_inner) {
                        pending.push(node.ty);
                    }
                }
                TypeCheck::Declared { arguments, .. } => pending.extend(arguments),
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
                TypeCheck::VarArgs { element: inner, .. }
                | TypeCheck::Nullable(inner)
                | TypeCheck::Alias(inner)
                | TypeCheck::GenericAlias(inner) => pending.push(*inner),
                TypeCheck::ParameterizedCollection { arguments, .. }
                | TypeCheck::Pair { arguments }
                | TypeCheck::CollectionBase { arguments } => {
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
    pub(super) fn source_section(&self) -> Option<crate::type_source::TypeSourceSection> {
        match self {
            Self::Shared(node) => node.node().source_section.clone(),
            _ => None,
        }
    }
    pub(super) fn with_optional_source_section(
        self,
        section: Option<crate::type_source::TypeSourceSection>,
    ) -> Self {
        let Some(section) = section else {
            return self;
        };
        let ty = self.into_resolved();
        let noop = ty.noop();
        let needs_custom_this = typecheck::needs_custom_this(&ty);
        let needs_type_identity = needs_custom_this || typecheck::needs_type_identity(&ty);
        let has_declared = ty.has_declared();
        let has_parameters = ty.requires_substitution();
        Self::Shared(SharedTypeCheck(Some(Arc::new(SharedNode {
            ty,
            noop,
            needs_custom_this,
            needs_type_identity,
            has_declared,
            source_section: Some(section),
            has_parameters,
        }))))
    }
    /// Check lexical parameters in structural children and constraint actuals.
    /// Shared descendants cache this property so repeated DAG prefixes stay bounded.
    pub(super) fn requires_substitution(&self) -> bool {
        match self {
            Self::Any
            | Self::Unknown
            | Self::Nothing
            | Self::NonNull
            | Self::Null
            | Self::Boolean
            | Self::Int
            | Self::Float
            | Self::Number
            | Self::String
            | Self::Duration
            | Self::Bytes
            | Self::RefDomain
            | Self::StringLiteral(_)
            | Self::IntegerAlias { .. }
            | Self::MetaClass(_)
            | Self::Collection(_)
            | Self::Class { .. } => return false,
            Self::Declared { arguments, .. } if arguments.is_empty() => return false,
            Self::Parameter { .. } => return true,
            Self::Shared(node) => return node.node().has_parameters,
            _ => {}
        }
        let mut pending = vec![self];
        while let Some(value) = pending.pop() {
            if let Self::Shared(node) = value {
                if node.node().has_parameters {
                    return true;
                }
                continue;
            }
            if matches!(value, Self::Parameter { .. }) {
                return true;
            }
            pending.extend(value.type_children());
            if let Self::Constrained { constraints, .. } = value {
                pending.extend(constraints.iter().flat_map(|c| c.arguments.values()));
            }
        }
        false
    }
    pub(super) fn shared(self) -> Self {
        if matches!(self, Self::Shared(_)) {
            return self;
        }
        let noop = self.noop();
        let needs_custom_this = typecheck::needs_custom_this(&self);
        let needs_type_identity = needs_custom_this || typecheck::needs_type_identity(&self);
        let has_declared = self.has_declared();
        let has_parameters = self.requires_substitution();
        Self::Shared(SharedTypeCheck(Some(Arc::new(SharedNode {
            ty: self,
            noop,
            needs_custom_this,
            needs_type_identity,
            has_declared,
            source_section: None,
            has_parameters,
        }))))
    }
}

#[cfg(test)]
mod tests;
