//! Declaration lookup is evaluator work. Recipes contain code identities, never heap handles.
use super::*;

#[derive(Clone, Debug)]
pub(super) struct TypeRecipe {
    pub unit: crate::modules::UnitId,
    pub path: Vec<PklString>,
    pub names: Vec<Span>,
    pub span: Span,
    pub argument_spans: Vec<Span>,
    pub builtin: Option<Result<TypeCheck, RuntimeError>>,
    /// Selected only after forcing the actual canonical ref-module receiver.
    pub reference: Option<Result<TypeCheck, RuntimeError>>,
    /// A compiled argument error is evaluated only after its actual base class
    /// admits that argument. In particular, Class<T> never evaluates this node.
    pub failure: Option<RuntimeError>,
}

#[derive(Clone, Debug)]
pub(super) struct AliasDefinition {
    pub parameters: Vec<PklString>,
    pub body: TypeCheck,
    pub origin: RuntimeFrame,
}

#[derive(Clone)]
pub(super) enum DeclarationValue {
    Value(Value),
    Builtin(TypeCheck),
    /// A module type first resolved in an open/abstract module remains a
    /// receiver-dependent self type. A closed first receiver is cached as Class.
    ModuleSelf,
}

#[derive(Clone, Copy)]
pub(super) struct Need {
    recipe: usize,
    owner: ObjectId,
    module_receiver: Option<ObjectId>,
}

enum ResolutionFailure {
    Need(Need),
    Error(RuntimeError),
}
impl From<RuntimeError> for ResolutionFailure {
    fn from(error: RuntimeError) -> Self {
        Self::Error(error)
    }
}

enum Phase {
    Import,
    Type,
}
pub(super) struct DeclaredRun {
    need: Need,
    phase: Phase,
    retry_instruction: bool,
    imported: Option<ObjectId>,
}
impl DeclaredRun {
    pub(super) fn span(&self, program: &Program) -> Span {
        program.type_recipes[self.need.recipe].span
    }
    pub(super) fn roots(&self, roots: &mut Vec<Value>) {
        roots.push(Value::Object(self.need.owner));
        roots.extend(self.need.module_receiver.map(Value::Object));
        roots.extend(self.imported.map(Value::Object));
    }
}

impl TypeCheck {
    pub(super) fn has_declared(&self) -> bool {
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
            | Self::Class { .. }
            | Self::Parameter { .. } => return false,
            Self::Declared { .. } => return true,
            Self::Shared(node) => return node.has_declared(),
            _ => {}
        }
        let mut pending = vec![self];
        let mut visited = BTreeSet::new();
        while let Some(ty) = pending.pop() {
            if let Self::Shared(node) = ty {
                if node.has_declared() {
                    return true;
                }
                continue;
            }
            if !visited.insert(ty as *const Self) {
                continue;
            }
            if matches!(ty, Self::Declared { .. }) {
                return true;
            }
            pending.extend(ty.type_children());
            if let Self::Constrained { constraints, .. } = ty {
                pending.extend(
                    constraints
                        .iter()
                        .flat_map(|constraint| constraint.arguments.values()),
                );
            }
        }
        false
    }

    /// Rebuild a type graph while preserving declaration and source metadata.
    fn map_children<E>(
        &self,
        mut map: impl FnMut(&TypeCheck) -> Result<TypeCheck, E>,
    ) -> Result<Self, E> {
        use TypeCheck::*;
        Ok(match self.resolved() {
            VarArgs { element, span } => VarArgs {
                element: Box::new(map(element)?),
                span: *span,
            },
            Nullable(inner) => Nullable(Box::new(map(inner)?)),
            Alias(inner) => Alias(Box::new(map(inner)?)),
            GenericAlias(inner) => GenericAlias(Box::new(map(inner)?)),
            NamedAlias {
                declaration,
                name,
                arguments,
                expanded,
                default_origin,
            } => NamedAlias {
                declaration: *declaration,
                name: name.clone(),
                arguments: arguments.iter().map(&mut map).collect::<Result<_, _>>()?,
                expanded: Box::new(map(expanded)?),
                default_origin: default_origin.clone(),
            },
            Constrained {
                base,
                base_span,
                constraints,
            } => Constrained {
                base: Box::new(map(base)?),
                base_span: *base_span,
                constraints: constraints
                    .iter()
                    .map(|constraint| {
                        let mut result = constraint.clone();
                        result.arguments = constraint
                            .arguments
                            .iter()
                            .map(|(name, ty)| Ok((name.clone(), map(ty)?)))
                            .collect::<Result<_, E>>()?;
                        Ok(result)
                    })
                    .collect::<Result<_, E>>()?,
            },
            Function {
                parameters,
                result,
                syntax,
            } => Function {
                parameters: parameters
                    .as_ref()
                    .map(|values| values.iter().map(&mut map).collect::<Result<_, _>>())
                    .transpose()?,
                result: Box::new(map(result)?),
                syntax: *syntax,
            },
            Union {
                string_literals,
                types,
                spans,
                labels,
                default,
            } => Union {
                string_literals: *string_literals,
                types: types.iter().map(&mut map).collect::<Result<_, _>>()?,
                spans: spans.clone(),
                labels: labels.clone(),
                default: *default,
            },
            Reference { domain, referent } => Reference {
                domain: Box::new(map(domain)?),
                referent: Box::new(map(referent)?),
            },
            ParameterizedCollection { kind, arguments } => ParameterizedCollection {
                kind: *kind,
                arguments: arguments
                    .iter()
                    .map(|(ty, span)| Ok((map(ty)?, *span)))
                    .collect::<Result<_, E>>()?,
            },
            Pair { arguments } => Pair {
                arguments: arguments
                    .iter()
                    .map(|(ty, span)| Ok((map(ty)?, *span)))
                    .collect::<Result<_, E>>()?,
            },
            CollectionBase { arguments } => CollectionBase {
                arguments: arguments
                    .iter()
                    .map(|(ty, span)| Ok((map(ty)?, *span)))
                    .collect::<Result<_, E>>()?,
            },
            other => other.clone(),
        })
    }
}

impl Vm {
    pub(super) fn declaring_module(
        &self,
        environment: &Environment,
        self_type: bool,
    ) -> Result<ObjectId, RuntimeError> {
        let mut scope = environment
            .scopes
            .last()
            .cloned()
            .ok_or_else(|| invalid("type resolution has no lexical module"))?;
        let mut seen = SmallObjectIdSet::new();
        while seen.insert(scope.owner) {
            let object = self.heap.get(scope.owner)?;
            if object.is_module {
                // `module` is a receiver-dependent self type. Ordinary named
                // declarations retain the lexical owner's lookup/cache key.
                return Ok(if self_type {
                    scope.receiver
                } else {
                    scope.owner
                });
            }
            scope = object
                .environment
                .scopes
                .last()
                .cloned()
                .ok_or_else(|| invalid("type owner has no enclosing module"))?;
        }
        Err(invalid("cyclic lexical type owner"))
    }

    pub(super) fn alias_check_environment(
        &self,
        declaration: usize,
        environment: &Environment,
    ) -> Result<Option<Environment>, RuntimeError> {
        if let Some(alias) = self.alias_singletons.get(&declaration) {
            return Ok(Some(self.heap.get(*alias)?.environment.clone()));
        }
        // Inlined aliases need not have had their value forced. Locate the
        // published declaration owner without evaluating the alias or its body.
        let mut pending: Vec<_> = environment
            .scopes
            .iter()
            .rev()
            .map(|scope| scope.owner)
            .collect();
        let mut visited = SmallObjectIdSet::new();
        while let Some(owner) = pending.pop() {
            if !visited.insert(owner) {
                continue;
            }
            let object = self.heap.get(owner)?;
            if object.members.iter().any(|member| matches!(
                self.program.blocks.get(member.code).and_then(|block| block.first()),
                Some(Instruction::BeginTypeAlias { declaration: found, .. }) if *found == declaration
            )) {
                let mut lexical = object.environment.clone();
                lexical.scopes.insert(0, Scope { receiver: owner, owner });
                return Ok(Some(lexical));
            }
            pending.extend(
                object
                    .environment
                    .scopes
                    .iter()
                    .rev()
                    .map(|scope| scope.owner),
            );
            pending.extend(object.parent);
        }
        Ok(None)
    }

    fn materialize_type(
        &self,
        ty: &TypeCheck,
        environment: &Environment,
        for_default: bool,
        final_type: &mut bool,
    ) -> Result<TypeCheck, ResolutionFailure> {
        // Postorder work avoids Rust recursion even when a deep shared generic
        // graph acquires a declaration leaf. Context ids distinguish alias
        // instantiations whose formal arguments differ.
        type Context = (usize, Arc<Environment>);
        enum Work {
            Visit(TypeCheck, Context),
            Rebuild(TypeCheck, usize),
            Changed,
            Shared(TypeCheck, (usize, *const TypeCheck)),
            Alias(ObjectId, usize, Context, Span, Option<RuntimeFrame>),
            Parameterized(classes::BuiltinClass, Vec<Span>, Span),
            ExpandedAlias {
                declaration: usize,
                name: PklString,
                arguments: Vec<TypeCheck>,
                origin: RuntimeFrame,
                use_origin: Option<RuntimeFrame>,
            },
        }
        let mut work = vec![Work::Visit(ty.clone(), (0, Arc::new(environment.clone())))];
        let mut values: Vec<(TypeCheck, bool)> = Vec::new();
        // Retaining the original node makes pointer keys stable for this run.
        let mut memo: BTreeMap<(usize, *const TypeCheck), (TypeCheck, TypeCheck, bool)> =
            BTreeMap::new();
        let mut next_context = 1;
        while let Some(task) = work.pop() {
            match task {
                Work::Visit(ty, context) => {
                    if let TypeCheck::Shared(node) = &ty {
                        if !node.has_declared() {
                            values.push((ty, false));
                            continue;
                        }
                        let key = (context.0, ty.resolved() as *const TypeCheck);
                        if let Some((_, value, changed)) = memo.get(&key) {
                            values.push((value.clone(), *changed));
                        } else {
                            let inner = ty.resolved().clone();
                            work.push(Work::Shared(ty, key));
                            work.push(Work::Visit(inner, context));
                        }
                        continue;
                    }
                    if let TypeCheck::Declared { recipe, arguments } = ty {
                        let self_type = self.program.type_recipes[recipe].path.is_empty()
                            && self.program.type_recipes[recipe].failure.is_none();
                        let owner = self.declaring_module(&context.1, false)?;
                        let module_receiver = self_type
                            .then(|| self.declaring_module(&context.1, true))
                            .transpose()?;
                        let value = self.resolved_declarations.get(&(recipe, owner)).ok_or(
                            ResolutionFailure::Need(Need {
                                recipe,
                                owner,
                                module_receiver,
                            }),
                        )?;
                        let definition = &self.program.type_recipes[recipe];
                        match value {
                            DeclarationValue::ModuleSelf => {
                                *final_type = false;
                                // The declaration established a non-final module
                                // type; defaults/checks now use the invocation's
                                // module even when an alias supplies lexical names.
                                // Alias checking swaps in its declaration frame;
                                // createDefaultValue deliberately keeps the caller.
                                let receiver = self.declaring_module(
                                    if for_default { environment } else { &context.1 },
                                    true,
                                )?;
                                let class = self
                                    .heap
                                    .object_class(receiver)?
                                    .ok_or_else(|| invalid("module self type has no class"))?;
                                let class = self.heap.class_definition(class)?;
                                values.push((
                                    TypeCheck::Class {
                                        module: class.module.clone(),
                                        id: class.id,
                                        name: class.name.clone(),
                                    },
                                    true,
                                ));
                            }
                            DeclarationValue::Builtin(ty) => {
                                work.push(Work::Changed);
                                work.push(Work::Visit(ty.substituted(&context.1)?, context));
                            }
                            DeclarationValue::Value(Value::Class(id)) => {
                                if !arguments.is_empty() {
                                    let builtin = self.check_class_type_arity(
                                        *id,
                                        arguments.len(),
                                        definition.span,
                                    )?;
                                    let builtin = builtin.ok_or_else(|| {
                                        invalid("non-generic class passed nonempty arity")
                                    })?;
                                    if builtin == classes::BuiltinClass::Class {
                                        values.push((TypeCheck::MetaClass("Class".into()), true));
                                    } else {
                                        work.push(Work::Parameterized(
                                            builtin,
                                            definition.argument_spans.clone(),
                                            definition.span,
                                        ));
                                        for argument in arguments.into_iter().rev() {
                                            work.push(Work::Visit(argument, context.clone()));
                                        }
                                    }
                                    continue;
                                }
                                let class = self
                                    .heap
                                    .get(self.heap.get_class(*id)?.prototype)?
                                    .class_definition
                                    .as_ref()
                                    .ok_or_else(|| invalid("resolved class metadata missing"))?;
                                values.push((
                                    classes::specialized_type(class.module.as_ref(), &class.name)
                                        .unwrap_or_else(|| TypeCheck::Class {
                                            module: class.module.clone(),
                                            id: class.id,
                                            name: class.name.clone(),
                                        }),
                                    true,
                                ));
                            }
                            DeclarationValue::Value(Value::Object(id)) => {
                                let class = collections::typed_class(&self.heap, *id)?
                                    .ok_or_else(|| invalid("module type has no class"))?;
                                if !arguments.is_empty() {
                                    return Err(RuntimeError::new(
                                        ErrorKind::Type,
                                        PklString::from("Class `")
                                            .concat(&class.name)
                                            .concat(&"` does not have type parameters.".into()),
                                    )
                                    .at(definition.argument_spans[0])
                                    .into());
                                }
                                self.declared_module_class(*id, definition.span)?;
                                values.push((
                                    TypeCheck::Class {
                                        module: class.module.clone(),
                                        id: class.id,
                                        name: class.name.clone(),
                                    },
                                    true,
                                ));
                            }
                            DeclarationValue::Value(Value::TypeAlias(id)) => {
                                let use_origin =
                                    definition.argument_spans.first().and_then(|span| {
                                        self.program
                                            .units
                                            .get(definition.unit.0 as usize)
                                            .and_then(|unit| unit.source.clone())
                                            .map(|source| RuntimeFrame {
                                                anonymous: false,
                                                source: Some(source),
                                                span: *span,
                                                member: PklString::default(),
                                            })
                                    });
                                work.push(Work::Alias(
                                    *id,
                                    arguments.len(),
                                    context.clone(),
                                    definition.span,
                                    use_origin,
                                ));
                                for argument in arguments.into_iter().rev() {
                                    work.push(Work::Visit(argument, context.clone()));
                                }
                            }
                            _ => {
                                return Err(invalid(
                                    "resolved declaration is not a class or alias",
                                )
                                .into());
                            }
                        }
                        continue;
                    }
                    let mut children: Vec<_> = ty.type_children().cloned().collect();
                    if let TypeCheck::Constrained { constraints, .. } = &ty {
                        children.extend(
                            constraints
                                .iter()
                                .flat_map(|constraint| constraint.arguments.values().cloned()),
                        );
                    }
                    if children.is_empty() {
                        values.push((ty, false));
                        continue;
                    }
                    // Locally compiled aliases already carry their expanded
                    // body rather than a Declared lookup. Preserve the same
                    // lexical receiver swap as the runtime alias path above.
                    let alias_context = if !for_default
                        && let TypeCheck::NamedAlias { declaration, .. } = &ty
                        && let Some(lexical) =
                            self.alias_check_environment(*declaration, &context.1)?
                    {
                        let id = next_context;
                        next_context += 1;
                        Some((id, Arc::new(lexical)))
                    } else {
                        None
                    };
                    let expanded_index = children.len() - 1;
                    work.push(Work::Rebuild(ty, children.len()));
                    for (index, child) in children.into_iter().enumerate().rev() {
                        let child_context = if index == expanded_index {
                            alias_context.as_ref().unwrap_or(&context)
                        } else {
                            &context
                        };
                        work.push(Work::Visit(child, child_context.clone()));
                    }
                }
                Work::Changed => values.last_mut().expect("builtin type result").1 = true,
                Work::Parameterized(builtin, spans, span) => {
                    let count = spans.len();
                    let arguments = values
                        .split_off(values.len() - count)
                        .into_iter()
                        .zip(spans)
                        .map(|((ty, _), span)| (ty, span))
                        .collect();
                    values.push((
                        builtin
                            .parameterized_type(arguments, span)
                            .map_err(|error| error.at(span))?,
                        true,
                    ));
                }
                Work::Rebuild(original, count) => {
                    let children = values.split_off(values.len() - count);
                    let changed = children.iter().any(|(_, changed)| *changed);
                    if changed {
                        let mut children = children.into_iter().map(|(ty, _)| ty);
                        let result = original.map_children(|_| {
                            Ok::<_, ResolutionFailure>(children.next().expect("type child"))
                        })?;
                        values.push((result, true));
                    } else {
                        values.push((original, false));
                    }
                }
                Work::Shared(original, key) => {
                    let (value, changed) = values.pop().expect("shared type result");
                    let value = if changed {
                        value
                            .with_optional_source_section(original.source_section())
                            .shared()
                    } else {
                        original.clone()
                    };
                    memo.insert(key, (original, value.clone(), changed));
                    values.push((value, changed));
                }
                Work::Alias(id, count, _caller, span, use_origin) => {
                    let arguments: Vec<_> = values
                        .split_off(values.len() - count)
                        .into_iter()
                        .map(|(ty, _)| ty)
                        .collect();
                    let alias = self.heap.get(id)?;
                    if !alias.type_alias_initialized {
                        return Err(RuntimeError::new(
                            ErrorKind::Cycle,
                            "Type alias definitions must not be cyclic.",
                        )
                        .at(span)
                        .into());
                    }
                    // Only BuiltinTypeAlias creates initialized alias values
                    // without an authored declaration. Declared resolution in
                    // the pinned VM selects intrinsic nodes for these base
                    // aliases; user aliases with the same name retain their
                    // declaration/recipe and never enter this branch.
                    if alias.type_alias_declaration.is_none() {
                        let name = alias
                            .type_alias
                            .as_ref()
                            .and_then(PklString::as_str)
                            .ok_or_else(|| invalid("builtin alias name missing or not scalar"))?;
                        let ty = match name {
                            "NonNull" => TypeCheck::NonNull,
                            "Int8" | "Int16" | "Int32" | "UInt8" | "UInt16" | "UInt32" | "UInt" => {
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
                                    name: name.into(),
                                    min,
                                    max,
                                }
                            }
                            _ => return Err(invalid("unrecognized builtin alias").into()),
                        };
                        // Work::Alias runs after its argument visits, matching
                        // TypeAliasTypeNode's check after resolving arguments.
                        if !arguments.is_empty() {
                            return Err(RuntimeError::new(
                                ErrorKind::Type,
                                format!("Expected 0 type argument(s) but got {}.", arguments.len()),
                            )
                            .at(span)
                            .into());
                        }
                        values.push((ty, true));
                        continue;
                    }
                    let declaration = alias
                        .type_alias_declaration
                        .expect("authored alias declaration");
                    let metadata = self
                        .program
                        .alias_definition(declaration)
                        .ok_or_else(|| invalid("resolved alias recipe missing"))?;
                    if !arguments.is_empty() && arguments.len() != metadata.parameters.len() {
                        return Err(RuntimeError::new(
                            ErrorKind::Type,
                            format!(
                                "Expected {} type argument(s) but got {}.",
                                metadata.parameters.len(),
                                arguments.len()
                            ),
                        )
                        .at(span)
                        .into());
                    }
                    let mut lexical = alias.environment.clone();
                    for (index, name) in metadata.parameters.iter().enumerate() {
                        lexical.type_arguments.insert(
                            (declaration, name.clone()),
                            arguments.get(index).cloned().unwrap_or(TypeCheck::Unknown),
                        );
                    }
                    let body = metadata.body.substituted(&lexical)?;
                    let context = (next_context, Arc::new(lexical));
                    next_context += 1;
                    work.push(Work::ExpandedAlias {
                        declaration,
                        name: alias.type_alias.clone().expect("alias name"),
                        arguments,
                        origin: metadata.origin.clone(),
                        use_origin,
                    });
                    work.push(Work::Visit(body, context));
                }
                Work::ExpandedAlias {
                    declaration,
                    name,
                    arguments,
                    origin,
                    use_origin,
                } => {
                    let (expanded, _) = values.pop().expect("alias expansion");
                    values.push((
                        TypeCheck::NamedAlias {
                            declaration,
                            name,
                            arguments,
                            expanded: Box::new(expanded),
                            default_origin: if self.program.base_mixin_declaration
                                == Some(declaration)
                            {
                                use_origin.or(Some(origin)).map(Box::new)
                            } else {
                                Some(Box::new(origin))
                            },
                        },
                        true,
                    ));
                }
            }
        }
        Ok(values.pop().expect("materialized type").0)
    }

    /// Resolve an inherited property's declared type against its declaring
    /// owner's environment, caching final non-generic results as `Shared`.
    pub(super) fn resolve_property_type(
        &mut self,
        declaration: usize,
        owner: ObjectId,
        ty: &TypeCheck,
    ) -> Option<TypeCheck> {
        if !ty.has_declared() {
            return None;
        }
        if let Some(cached) = self.resolved_property_types.get(&(declaration, owner)) {
            return Some(cached.clone());
        }
        let owner_obj = self.heap.get(owner).ok()?;
        if !owner_obj.environment.type_arguments.is_empty() {
            return None;
        }
        let fallback_env;
        let env = if owner_obj.environment.scopes.is_empty() {
            let mut env = owner_obj.environment.clone();
            env.scopes = crate::heap::ScopeList::from_single(Scope {
                receiver: owner,
                owner,
            });
            fallback_env = env;
            &fallback_env
        } else {
            &owner_obj.environment
        };
        let mut final_type = true;
        match self.materialize_type(ty, env, false, &mut final_type) {
            Ok(resolved) if final_type => {
                let shared = resolved.shared();
                self.resolved_property_types
                    .insert((declaration, owner), shared.clone());
                Some(shared)
            }
            Ok(_) | Err(_) => None,
        }
    }

    /// A caller retains its own work item while the ordinary member/import
    /// continuation resolves the next declaration.
    pub(super) fn resolve_runtime_type(
        &mut self,
        ty: &TypeCheck,
        environment: &Environment,
        for_default: bool,
    ) -> Result<Option<TypeCheck>, RuntimeError> {
        self.resolve_runtime_type_retry(ty, environment, for_default, false)
    }

    /// Resolve exported declaration metadata while preserving the read site's
    /// operands. Rewind only after successful resolution, keeping a failing
    /// declaration's caller at its original source instruction.
    pub(super) fn resolve_metadata_type(
        &mut self,
        ty: &TypeCheck,
        environment: &Environment,
    ) -> Result<Option<TypeCheck>, RuntimeError> {
        self.resolve_runtime_type_retry(ty, environment, false, true)
    }

    fn resolve_runtime_type_retry(
        &mut self,
        ty: &TypeCheck,
        environment: &Environment,
        for_default: bool,
        retry_instruction: bool,
    ) -> Result<Option<TypeCheck>, RuntimeError> {
        if !ty.has_declared() {
            return Ok(Some(ty.clone()));
        }
        match self.materialize_type(ty, environment, for_default, &mut true) {
            Ok(resolved) => Ok(Some(resolved)),
            Err(ResolutionFailure::Error(error)) => Err(error),
            Err(ResolutionFailure::Need(need)) => {
                self.begin_declared_type(need, retry_instruction)?;
                Ok(None)
            }
        }
    }

    pub(super) fn resolve_instruction_types(
        &mut self,
        mut instruction: Instruction,
    ) -> Result<Option<Instruction>, RuntimeError> {
        if matches!(
            instruction,
            Instruction::ResolvedInferredParent { cache: true, .. }
                | Instruction::GenericConstructorParent { cache: true, .. }
        ) {
            let frame = self.frames.last().expect("inferred parent frame");
            if let Some(value) = self.type_parent_defaults.get(&(frame.block, frame.pc - 1)) {
                return Ok(Some(Instruction::Constant(value.clone())));
            }
        }
        let for_default = matches!(
            instruction,
            Instruction::ResolvedDefault { .. }
                | Instruction::ResolvedInferredParent { .. }
                | Instruction::GenericConstructorParent { .. }
                | Instruction::SetCollectionTypeDefault { .. }
        );
        let mut final_type = true;
        let ty = match &mut instruction {
            Instruction::ResolveType(ty)
            | Instruction::ResolvedClass(ty)
            | Instruction::ResolvedSupertype { ty, .. }
            | Instruction::BuiltinClass(ty)
            | Instruction::BindLocal { ty, .. }
            | Instruction::CheckType(ty)
            | Instruction::TestType(ty)
            | Instruction::ResolvedDefault { ty, .. }
            | Instruction::ResolvedInferredParent { ty, .. }
            | Instruction::ValidateTypeArguments { ty, .. }
            | Instruction::GenericConstructorParent { ty, .. }
            | Instruction::SetCollectionTypeDefault { ty, .. } => ty,
            _ => return Ok(Some(instruction)),
        };
        if !ty.has_declared() {
            return Ok(Some(instruction));
        }
        let (block, pc, env_type_args_empty) = {
            let frame = self.frames.last().expect("type instruction");
            (
                frame.block,
                frame.pc.saturating_sub(1),
                frame.environment.type_arguments.is_empty(),
            )
        };
        let cache_key = if env_type_args_empty {
            let environment = &self.frames.last().expect("type instruction").environment;
            self.declaring_module(environment, false)
                .ok()
                .map(|owner| (block, pc, owner, for_default))
        } else {
            None
        };
        if let Some(key) = cache_key
            && let Some(cached) = self.resolved_instruction_types.get(&key)
        {
            **ty = cached.clone();
            return Ok(Some(instruction));
        }
        let environment = &self.frames.last().expect("type instruction").environment;
        match self.materialize_type(ty, environment, for_default, &mut final_type) {
            Ok(resolved) => {
                if final_type && let Some(key) = cache_key {
                    let shared = resolved.shared();
                    self.resolved_instruction_types.insert(key, shared.clone());
                    **ty = shared;
                } else {
                    **ty = resolved;
                }
            }
            Err(ResolutionFailure::Error(error)) => return Err(error),
            Err(ResolutionFailure::Need(need)) => {
                self.begin_declared_type(need, true)?;
                return Ok(None);
            }
        }
        if !final_type && let Instruction::GenericConstructorParent { cache, .. } = &mut instruction
        {
            *cache = false;
        }
        Ok(Some(instruction))
    }

    /// Select an authored declaration in one module, without virtual property
    /// lookup or evaluation. Both executable types and mirror recipes use this
    /// boundary; class value properties are never part of this namespace.
    pub(super) fn declared_type_member(
        &self,
        owner: ObjectId,
        name: &PklString,
        local: bool,
        allow_import: bool,
        span: Span,
    ) -> Result<Option<Member>, RuntimeError> {
        let member = self
            .heap
            .get(owner)?
            .members
            .iter()
            .find(|member| member.local == local && !member.method && member.name == *name);
        if let Some(member) = member
            && !member.definition()
            && !(allow_import && member.import() && !member.glob())
        {
            return Err(RuntimeError::new(
                ErrorKind::Type,
                PklString::from("Expected `")
                    .concat(name)
                    .concat(&"` to be a type, but it is not.".into()),
            )
            .at(span));
        }
        Ok(member.cloned())
    }

    pub(super) fn declared_type_import(
        &self,
        owner: ObjectId,
        name: &PklString,
        span: Span,
    ) -> Result<Member, RuntimeError> {
        let member = self
            .heap
            .get(owner)?
            .members
            .iter()
            .find(|member| member.local && !member.method && member.name == *name)
            .cloned()
            .ok_or_else(|| {
                RuntimeError::new(
                    ErrorKind::Type,
                    PklString::from("Cannot find module import `")
                        .concat(name)
                        .concat(&"`.".into()),
                )
                .at(span)
            })?;
        if !member.import() {
            return Err(RuntimeError::new(
                ErrorKind::Type,
                PklString::from("Expected `")
                    .concat(name)
                    .concat(&"` to be a module import, but it is not.".into()),
            )
            .at(span));
        }
        if member.glob() {
            return Err(RuntimeError::new(
                ErrorKind::Type,
                PklString::from("Expected `")
                    .concat(name)
                    .concat(&"` to be a type, but it is not.".into()),
            )
            .at(span));
        }
        Ok(member)
    }

    /// Named module declarations denote their prototype class. Amending module
    /// values and arbitrary objects are not named module types (keyword `module`
    /// has its separate receiver-dependent resolution path).
    pub(super) fn declared_module_class(
        &self,
        id: ObjectId,
        span: Span,
    ) -> Result<ObjectId, RuntimeError> {
        let object = self.heap.get(id)?;
        let class = self
            .heap
            .object_class(id)?
            .ok_or_else(|| invalid("module type has no class"))?;
        if !object.is_module || object.class_definition.is_none() {
            let name = &self.heap.class_definition(class)?.name;
            return Err(RuntimeError::new(
                ErrorKind::Type,
                PklString::from("Module `").concat(name).concat(
                    &"` cannot be extended or used as type because it amends another module."
                        .into(),
                ),
            )
            .at(span));
        }
        Ok(class)
    }

    fn begin_declared_type(
        &mut self,
        need: Need,
        retry_instruction: bool,
    ) -> Result<(), RuntimeError> {
        let recipe = self.program.type_recipes[need.recipe].clone();
        if let Some(error) = recipe.failure {
            return Err(error);
        }
        if recipe.path.is_empty() {
            // UnresolvedTypeNode.Module chooses finality once, using the
            // receiver at first resolution. Inherited call sites first resolved
            // in a closed child retain that child's class, matching Java's
            // observable order-dependent direct `new module` behavior.
            let receiver = need
                .module_receiver
                .ok_or_else(|| invalid("module type receiver"))?;
            let value = self.reference_get_class(&Value::Object(receiver))?;
            let class = self
                .heap
                .class_definition(value.heap_id().expect("module class"))?;
            let value = if class.open || class.abstract_ {
                DeclarationValue::ModuleSelf
            } else {
                DeclarationValue::Value(value)
            };
            self.resolved_declarations
                .insert((need.recipe, need.owner), value);
            self.declaration_roots_dirty = true;
            if retry_instruction {
                self.frames.last_mut().expect("type frame").pc -= 1;
            }
            return Ok(());
        }
        if recipe.path.len() == 1 {
            let name = recipe.path[0].clone();
            let mut owner = Some(need.owner);
            let mut first = true;
            while let Some(id) = owner {
                let local_member = if first {
                    self.declared_type_member(id, &name, true, true, recipe.span)?
                } else {
                    None
                };
                let member = if local_member.is_some() {
                    local_member
                } else {
                    self.declared_type_member(id, &name, false, true, recipe.span)?
                };
                if let Some(member) = member {
                    let local = member.local;
                    self.frames
                        .last_mut()
                        .expect("type resolution frame")
                        .type_resolution = Some(Box::new(DeclaredRun {
                        need,
                        phase: Phase::Type,
                        retry_instruction,
                        imported: None,
                    }));
                    return self.force(id, id, name, local, true, false);
                }
                if first && let Some(fallback) = &recipe.builtin {
                    self.resolved_declarations.insert(
                        (need.recipe, need.owner),
                        DeclarationValue::Builtin(
                            fallback.clone().map_err(|error| error.at(recipe.span))?,
                        ),
                    );
                    self.declaration_roots_dirty = true;
                    if retry_instruction {
                        self.frames.last_mut().expect("type frame").pc -= 1;
                    }
                    return Ok(());
                }
                first = false;
                owner = self.heap.get(id)?.parent;
            }
            // Unit types are authored base aliases. Resolve their retained
            // declaration after lexical/user parents, preserving its union's
            // source section instead of constructing a source-less substitute.
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
            ) && let Some(modules::CachedModule::Value(base)) =
                self.modules_stdlib.get("pkl:base")
            {
                let base = *base;
                self.frames
                    .last_mut()
                    .expect("type resolution frame")
                    .type_resolution = Some(Box::new(DeclaredRun {
                    need,
                    phase: Phase::Type,
                    retry_instruction,
                    imported: None,
                }));
                return self.force(base, base, name, false, true, false);
            }
            return Err(RuntimeError::new(
                ErrorKind::MissingType,
                PklString::from("Cannot find type `")
                    .concat(&name)
                    .concat(&"`.".into()),
            )
            .at(recipe.span));
        }
        let import = recipe
            .path
            .first()
            .ok_or_else(|| invalid("empty declared type path"))?;
        self.declared_type_import(need.owner, import, recipe.names[0])?;
        self.frames
            .last_mut()
            .expect("type resolution frame")
            .type_resolution = Some(Box::new(DeclaredRun {
            need,
            phase: Phase::Import,
            retry_instruction,
            imported: None,
        }));
        self.force(need.owner, need.owner, import.clone(), true, true, false)
    }

    pub(super) fn step_declared_type(&mut self) -> Result<(), RuntimeError> {
        let mut run = self
            .frames
            .last_mut()
            .expect("type resolution frame")
            .type_resolution
            .take()
            .expect("declared type run");
        match run.phase {
            Phase::Import => {
                let module = self.pop()?.as_object()?;
                run.imported = Some(module);
                let recipe = &self.program.type_recipes[run.need.recipe];
                if self.heap.get(module)?.ref_module
                    && let Some(reference) = &recipe.reference
                {
                    self.resolved_declarations.insert(
                        (run.need.recipe, run.need.owner),
                        DeclarationValue::Builtin(reference.clone().map_err(|mut error| {
                            if !error.frames.is_empty() {
                                error.unavailable_origin = true;
                            }
                            error.at(recipe.span)
                        })?),
                    );
                    self.declaration_roots_dirty = true;
                    if run.retry_instruction {
                        self.frames.last_mut().expect("type frame").pc -= 1;
                    }
                    return Ok(());
                }
                let name = recipe
                    .path
                    .get(1)
                    .ok_or_else(|| invalid("qualified type suffix missing"))?
                    .clone();
                let mut owner = Some(module);
                while let Some(id) = owner {
                    let object = self.heap.get(id)?;
                    if self
                        .declared_type_member(
                            id,
                            &name,
                            false,
                            false,
                            Span::new(
                                recipe.names[0].start,
                                recipe.names.last().expect("qualified type name").end,
                            ),
                        )?
                        .is_some()
                    {
                        run.phase = Phase::Type;
                        self.frames.last_mut().unwrap().type_resolution = Some(run);
                        return self.force(id, id, name, false, true, false);
                    }
                    owner = object.parent;
                }
                let module_name = self
                    .heap
                    .get(module)?
                    .module_unit
                    .and_then(|id| self.program.units.get(id.0 as usize))
                    .map_or(&self.program.module_name, |unit| &unit.name);
                return Err(RuntimeError::new(
                    ErrorKind::Type,
                    PklString::from("Cannot find type `")
                        .concat(&name)
                        .concat(&"` in module `".into())
                        .concat(module_name)
                        .concat(&"`.".into()),
                )
                .at(recipe.names[1]));
            }
            Phase::Type => {
                let value = self.pop()?;
                self.resolved_declarations.insert(
                    (run.need.recipe, run.need.owner),
                    DeclarationValue::Value(value),
                );
                self.declaration_roots_dirty = true;
                if run.retry_instruction {
                    self.frames.last_mut().unwrap().pc -= 1;
                }
            }
        }
        Ok(())
    }
}

impl Compiler<'_> {
    pub(super) fn seed_recipe_caches(
        program: &Program,
        unit: crate::modules::UnitId,
    ) -> (BTreeMap<Span, usize>, BTreeMap<Span, usize>) {
        let mut declared = BTreeMap::new();
        let mut module = BTreeMap::new();
        for (id, recipe) in program.type_recipes.iter_with_ids() {
            if recipe.unit == unit {
                if recipe.path.is_empty() && recipe.failure.is_none() {
                    module.entry(recipe.span).or_insert(id);
                } else {
                    declared.entry(recipe.span).or_insert(id);
                }
            }
        }
        (declared, module)
    }

    pub(super) fn deferred_type_error(
        &mut self,
        error: RuntimeError,
        span: Span,
    ) -> Result<TypeCheck, RuntimeError> {
        let recipe = self.program.type_recipes.try_push(TypeRecipe {
            unit: self.unit,
            path: Vec::new(),
            names: Vec::new(),
            span,
            argument_spans: Vec::new(),
            builtin: None,
            reference: None,
            failure: Some(error.at(span)),
        })?;
        Ok(TypeCheck::Declared {
            recipe,
            arguments: Vec::new(),
        })
    }

    pub(super) fn module_type(&mut self, span: Span) -> Result<TypeCheck, RuntimeError> {
        let recipe = if let Some(&id) = self.module_recipes.get(&span) {
            id
        } else {
            let id = self.program.type_recipes.try_push(TypeRecipe {
                unit: self.unit,
                path: Vec::new(),
                names: Vec::new(),
                span,
                argument_spans: Vec::new(),
                builtin: None,
                reference: None,
                failure: None,
            })?;
            self.module_recipes.insert(span, id);
            id
        };
        Ok(TypeCheck::Declared {
            recipe,
            arguments: Vec::new(),
        })
    }

    pub(super) fn declared_type(
        &mut self,
        ty: &crate::syntax::Type,
        arguments: Vec<TypeCheck>,
    ) -> Result<TypeCheck, RuntimeError> {
        let crate::syntax::TypeKind::Named {
            path,
            name_spans,
            arguments: actuals,
        } = &ty.kind
        else {
            unreachable!()
        };
        let recipe = match self.declared_recipes.get(&ty.span).copied() {
            Some(id) if self.program.type_recipes[id].path.iter().eq(path.iter()) => Some(id),
            Some(_) => self
                .program
                .type_recipes
                .iter_with_ids()
                .find(|(_, recipe)| {
                    recipe.unit == self.unit
                        && recipe.span == ty.span
                        && recipe.path.iter().eq(path.iter())
                })
                .map(|(id, _)| id),
            None => None,
        };
        let recipe = match recipe {
            Some(id) => id,
            None => {
                let id = self.program.type_recipes.try_push(TypeRecipe {
                    unit: self.unit,
                    path: path.clone(),
                    names: name_spans.clone(),
                    span: ty.span,
                    argument_spans: actuals.iter().map(|argument| argument.span).collect(),
                    builtin: None,
                    reference: None,
                    failure: None,
                })?;
                self.declared_recipes.entry(ty.span).or_insert(id);
                id
            }
        };
        Ok(TypeCheck::Declared { recipe, arguments })
    }
}

impl Vm {
    fn check_class_type_arity(
        &self,
        class: ObjectId,
        actual: usize,
        span: Span,
    ) -> Result<Option<classes::BuiltinClass>, RuntimeError> {
        let builtin = self.canonical_type_class(class)?;
        let expected = builtin.map_or(0, classes::BuiltinClass::type_parameter_count);
        if actual != expected {
            return Err(RuntimeError::new(
                ErrorKind::Type,
                format!("Expected {expected} type argument(s) but got {actual}."),
            )
            .at(span));
        }
        Ok(builtin)
    }

    fn prepare_type_arguments(&mut self, recipe: usize, end: usize) -> Result<(), RuntimeError> {
        let owner = self.declaring_module(
            &self.frames.last().expect("type argument frame").environment,
            false,
        )?;
        let Some(value) = self.resolved_declarations.get(&(recipe, owner)) else {
            return self.begin_declared_type(
                Need {
                    recipe,
                    owner,
                    module_receiver: None,
                },
                true,
            );
        };
        let definition = &self.program.type_recipes[recipe];
        match value {
            DeclarationValue::Value(Value::Class(class)) => {
                if self.check_class_type_arity(
                    *class,
                    definition.argument_spans.len(),
                    definition.span,
                )? == Some(classes::BuiltinClass::Class)
                {
                    self.frames.last_mut().expect("type argument frame").pc = end;
                }
            }
            DeclarationValue::Value(Value::Object(module)) => {
                let class = collections::typed_class(&self.heap, *module)?
                    .ok_or_else(|| invalid("module type has no class"))?;
                return Err(RuntimeError::new(
                    ErrorKind::Type,
                    PklString::from("Class `")
                        .concat(&class.name)
                        .concat(&"` does not have type parameters.".into()),
                )
                .at(definition.argument_spans[0]));
            }
            // Alias arguments resolve before alias arity/initialization checks.
            // Ref bindings have already admitted the actual imported module.
            DeclarationValue::Value(Value::TypeAlias(_)) | DeclarationValue::Builtin(_) => {}
            _ => return Err(invalid("invalid parameterized type declaration")),
        }
        Ok(())
    }

    /// Type substitution and validation have their own bounded native frame.
    /// Keeping them outside the general opcode match avoids retaining that
    /// match's debug-build temporaries during nested type metadata operations.
    pub(super) fn type_instruction(
        &mut self,
        instruction: Instruction,
    ) -> Result<(), RuntimeError> {
        match instruction {
            Instruction::PrepareTypeArguments { recipe, end } => {
                self.prepare_type_arguments(recipe, end)?;
            }
            Instruction::ValidateTypeArguments { ty, aliases } => {
                let frame = self.frames.last().expect("active frame");
                let ty = ty.substituted(&frame.environment)?;
                let span = self
                    .program
                    .metadata
                    .get(frame.block)
                    .and_then(|metadata| metadata.spans.get(frame.pc.saturating_sub(1)))
                    .copied()
                    .flatten()
                    .unwrap_or(Span::new(0, 0));
                ty.validate_reference_arguments(span, &aliases, &self.program.constraint_origins)?;
            }
            Instruction::BuiltinClass(ty) => {
                let ty = ty.substituted(&self.frames.last().expect("active frame").environment)?;
                let value = self.builtin_class(ty);
                self.stack.push(value);
            }
            Instruction::TestType(ty) => {
                let value = self.pop()?;
                self.begin_type(*ty, value, TypeDestination::Test, Vec::new())?;
            }
            Instruction::ResolveType(_) => {}
            Instruction::ResolvedSupertype { ty, label } => {
                if classes::BuiltinClass::from_type(&ty).is_some() {
                    let value = self.builtin_class(*ty);
                    self.stack.push(value);
                    return Ok(());
                }
                let TypeCheck::Class { id, .. } = ty.unaliased() else {
                    return Err(invalid_supertype(&label));
                };
                let class = self
                    .heap
                    .find_class(*id)
                    .ok_or_else(|| invalid("missing resolved class"))?;
                self.stack.push(Value::Class(class));
            }
            Instruction::ResolvedClass(ty) => {
                if classes::BuiltinClass::from_type(&ty).is_some() {
                    let value = self.builtin_class(*ty);
                    self.stack.push(value);
                    return Ok(());
                }
                let TypeCheck::Class { id, .. } = ty.unaliased() else {
                    return Err(RuntimeError::new(ErrorKind::Type, "Expected a class type."));
                };
                let class = self
                    .heap
                    .find_class(*id)
                    .ok_or_else(|| invalid("missing resolved class"))?;
                self.stack.push(Value::Class(class));
            }
            Instruction::ResolvedDefault { ty, origin } => {
                self.frames
                    .last_mut()
                    .expect("default frame")
                    .generic_default = Some(Box::new(generics::GenericDefaultRun::stack(
                    *ty,
                    origin.map(|o| *o),
                )));
            }
            Instruction::ResolvedInferredParent { ty, origin, cache } => {
                let frame = self.frames.last_mut().expect("inferred parent frame");
                let ty = ty.substituted(&frame.environment)?;
                frame.generic_default =
                    Some(Box::new(generics::GenericDefaultRun::inferred_parent(
                        ty,
                        origin.map(|o| *o),
                        cache.then_some((frame.block, frame.pc - 1)),
                    )));
            }
            Instruction::ResolveClassType { name, local } => {
                let frame = self.frames.last().expect("active frame");
                let module = match frame.environment.scopes.last() {
                    // ResolveSimpleDeclaredTypeNode starts at the enclosing
                    // lexical module, even when this member is inherited by a
                    // different receiver. Local declarations are not overrides.
                    Some(_) => self.declaring_module(&frame.environment, false)?,
                    None => self
                        .stack
                        .first()
                        .ok_or_else(|| invalid("missing module for type resolution"))?
                        .as_object()?,
                };
                self.force(module, module, name, local, true, false)?;
            }
            Instruction::BindLocal { depth, ty } => {
                let policy = identity::SlotPolicy::from_type(&ty);
                let value = self.pop()?;
                self.begin_type(
                    *ty,
                    value,
                    TypeDestination::Local { depth, policy },
                    Vec::new(),
                )?;
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
                self.begin_type(*ty, value, TypeDestination::Stack, Vec::new())?;
            }
            _ => return Err(invalid("not a type instruction")),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changed_shared_type_dags_materialize_and_drop_on_a_small_stack() {
        std::thread::Builder::new()
            .stack_size(256 * 1024)
            .spawn(|| {
                let mut program = Program::new(Vec::new());
                program.type_recipes.push(TypeRecipe {
                    unit: crate::modules::UnitId(0),
                    path: vec!["C".into()],
                    names: vec![Span::new(0, 1)],
                    span: Span::new(0, 1),
                    argument_spans: Vec::new(),
                    builtin: None,
                    reference: None,
                    failure: None,
                });
                let mut vm = Vm::new(Arc::new(program));
                let owner = vm.heap.allocate(Object {
                    is_module: true,
                    ..Object::default()
                });
                let class = vm.heap.allocate(Object {
                    class_definition: Some(Box::new(Class {
                        id: 42,
                        module: Some("owner".into()),
                        origin_uri: None,
                        name: "C".into(),
                        open: false,
                        abstract_: false,
                    })),
                    ..Object::default()
                });
                let class = vm.heap.allocate_class(class);
                vm.resolved_declarations
                    .insert((0, owner), DeclarationValue::Value(Value::Class(class)));
                let environment = Environment {
                    scopes: crate::heap::ScopeList::from_single(Scope {
                        receiver: owner,
                        owner,
                    }),
                    ..Environment::default()
                };
                for shape in [0, 1, 2] {
                    let mut ty = TypeCheck::Declared {
                        recipe: 0,
                        arguments: Vec::new(),
                    }
                    .shared();
                    for _ in 0..20_000 {
                        ty = if shape == 1 {
                            TypeCheck::ParameterizedCollection {
                                kind: crate::heap::CollectionKind::Mapping,
                                arguments: vec![
                                    (ty.clone(), Span::new(0, 0)),
                                    (ty, Span::new(0, 0)),
                                ],
                            }
                        } else if shape == 2 {
                            TypeCheck::VarArgs {
                                element: Box::new(ty),
                                span: Span::new(0, 0),
                            }
                        } else {
                            TypeCheck::Nullable(Box::new(ty))
                        }
                        .shared();
                    }
                    assert!(ty.has_declared());
                    let result = match vm.materialize_type(&ty, &environment, false, &mut true) {
                        Ok(result) => result,
                        Err(_) => panic!("cached declaration should resolve"),
                    };
                    assert!(!result.has_declared());
                    let mut node = &result;
                    for _ in 0..20_000 {
                        node = if shape == 1 {
                            let TypeCheck::ParameterizedCollection { arguments, .. } =
                                node.resolved()
                            else {
                                panic!("Mapping")
                            };
                            assert!(std::ptr::eq(
                                arguments[0].0.resolved(),
                                arguments[1].0.resolved()
                            ));
                            &arguments[0].0
                        } else if shape == 2 {
                            let TypeCheck::VarArgs { element: inner, .. } = node.resolved() else {
                                panic!("VarArgs");
                            };
                            inner
                        } else {
                            let TypeCheck::Nullable(inner) = node.resolved() else {
                                panic!("Nullable")
                            };
                            inner
                        };
                    }
                    assert!(matches!(node.resolved(), TypeCheck::Class { id: 42, .. }));
                    drop(result);
                    drop(ty);
                }
            })
            .unwrap()
            .join()
            .unwrap();
    }
}
