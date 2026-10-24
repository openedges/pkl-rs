//! Declaration lookup is evaluator work. Recipes contain code identities, never heap handles.
use super::*;

#[derive(Clone, Debug)]
pub(super) struct TypeRecipe {
    pub unit: crate::modules::UnitId,
    pub path: Vec<Arc<str>>,
    pub names: Vec<Span>,
    pub span: Span,
    pub argument_spans: Vec<Span>,
    pub builtin: Option<Result<TypeCheck, RuntimeError>>,
    /// Selected only after forcing the actual canonical ref-module receiver.
    pub reference: Option<Result<TypeCheck, RuntimeError>>,
}

#[derive(Clone, Debug)]
pub(super) struct AliasDefinition {
    pub parameters: Vec<Arc<str>>,
    pub body: TypeCheck,
    pub origin: RuntimeFrame,
}

#[derive(Clone)]
pub(super) enum DeclarationValue {
    Value(Value),
    Builtin(TypeCheck),
}

#[derive(Clone, Copy)]
pub(super) struct Need {
    recipe: usize,
    owner: ObjectId,
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
        roots.extend(self.imported.map(Value::Object));
    }
}

impl TypeCheck {
    pub(super) fn has_declared(&self) -> bool {
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
    fn declaring_module(&self, environment: &Environment) -> Result<ObjectId, RuntimeError> {
        let mut owner = environment
            .scopes
            .last()
            .map(|scope| scope.owner)
            .ok_or_else(|| invalid("type resolution has no lexical module"))?;
        let mut seen = BTreeSet::new();
        while seen.insert(owner) {
            let object = self.heap.get(owner)?;
            if object.is_module {
                return Ok(owner);
            }
            owner = object
                .environment
                .scopes
                .last()
                .map(|scope| scope.owner)
                .ok_or_else(|| invalid("type owner has no enclosing module"))?;
        }
        Err(invalid("cyclic lexical type owner"))
    }

    fn materialize_type(
        &self,
        ty: &TypeCheck,
        environment: &Environment,
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
            Alias(ObjectId, usize, Context, Span),
            ExpandedAlias {
                declaration: usize,
                name: Arc<str>,
                arguments: Vec<TypeCheck>,
                origin: RuntimeFrame,
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
                        let owner = self.declaring_module(&context.1)?;
                        let value = self
                            .resolved_declarations
                            .get(&(recipe, owner))
                            .ok_or(ResolutionFailure::Need(Need { recipe, owner }))?;
                        let definition = &self.program.type_recipes[recipe];
                        match value {
                            DeclarationValue::Builtin(ty) => {
                                work.push(Work::Changed);
                                work.push(Work::Visit(ty.substituted(&context.1)?, context));
                            }
                            DeclarationValue::Value(Value::Class(id)) => {
                                if !arguments.is_empty() {
                                    return Err(RuntimeError::new(
                                        ErrorKind::Type,
                                        format!(
                                            "Expected 0 type argument(s) but got {}.",
                                            arguments.len()
                                        ),
                                    )
                                    .at(definition.span)
                                    .into());
                                }
                                let class = self
                                    .heap
                                    .get(self.heap.get_class(*id)?.prototype)?
                                    .class_definition
                                    .as_ref()
                                    .ok_or_else(|| invalid("resolved class metadata missing"))?;
                                values.push((
                                    classes::specialized_type(class.module.as_deref(), &class.name)
                                        .unwrap_or_else(|| TypeCheck::Class {
                                            module: class.module.clone(),
                                            id: class.id,
                                            name: class.name.clone(),
                                        }),
                                    true,
                                ));
                            }
                            DeclarationValue::Value(Value::Object(id)) => {
                                let object = self.heap.get(*id)?;
                                let class = collections::typed_class(&self.heap, *id)?
                                    .ok_or_else(|| invalid("module type has no class"))?;
                                if !arguments.is_empty() {
                                    return Err(RuntimeError::new(
                                        ErrorKind::Type,
                                        format!(
                                            "Class `{}` does not have type parameters.",
                                            class.name
                                        ),
                                    )
                                    .at(definition.argument_spans[0])
                                    .into());
                                }
                                if !object.is_module || object.class_definition.is_none() {
                                    return Err(RuntimeError::new(ErrorKind::Type, format!("Module `{}` cannot be extended or used as type because it amends another module.", class.name)).at(definition.span).into());
                                }
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
                                work.push(Work::Alias(
                                    *id,
                                    arguments.len(),
                                    context.clone(),
                                    definition.span,
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
                    work.push(Work::Rebuild(ty, children.len()));
                    for child in children.into_iter().rev() {
                        work.push(Work::Visit(child, context.clone()));
                    }
                }
                Work::Changed => values.last_mut().expect("builtin type result").1 = true,
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
                        value.shared()
                    } else {
                        original.clone()
                    };
                    memo.insert(key, (original, value.clone(), changed));
                    values.push((value, changed));
                }
                Work::Alias(id, count, _caller, span) => {
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
                    let declaration = alias
                        .type_alias_declaration
                        .ok_or_else(|| invalid("resolved alias declaration missing"))?;
                    let metadata = self
                        .program
                        .alias_definitions
                        .get(&declaration)
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
                    });
                    work.push(Work::Visit(body, context));
                }
                Work::ExpandedAlias {
                    declaration,
                    name,
                    arguments,
                    origin,
                } => {
                    let (expanded, _) = values.pop().expect("alias expansion");
                    values.push((
                        TypeCheck::NamedAlias {
                            declaration,
                            name,
                            arguments,
                            expanded: Box::new(expanded),
                            default_origin: Some(origin),
                        },
                        true,
                    ));
                }
            }
        }
        Ok(values.pop().expect("materialized type").0)
    }

    /// A caller retains its own work item while the ordinary member/import
    /// continuation resolves the next declaration.
    pub(super) fn resolve_runtime_type(
        &mut self,
        ty: &TypeCheck,
        environment: &Environment,
    ) -> Result<Option<TypeCheck>, RuntimeError> {
        if !ty.has_declared() {
            return Ok(Some(ty.clone()));
        }
        match self.materialize_type(ty, environment) {
            Ok(resolved) => Ok(Some(resolved)),
            Err(ResolutionFailure::Error(error)) => Err(error),
            Err(ResolutionFailure::Need(need)) => {
                self.begin_declared_type(need, false)?;
                Ok(None)
            }
        }
    }

    pub(super) fn resolve_instruction_types(
        &mut self,
        mut instruction: Instruction,
    ) -> Result<Option<Instruction>, RuntimeError> {
        let ty = match &mut instruction {
            Instruction::ResolveType(ty)
            | Instruction::ResolvedClass(ty)
            | Instruction::BuiltinClass(ty)
            | Instruction::BindLocal { ty, .. }
            | Instruction::CheckType(ty)
            | Instruction::TestType(ty)
            | Instruction::ResolvedDefault { ty, .. }
            | Instruction::ValidateTypeArguments { ty, .. }
            | Instruction::GenericConstructorParent { ty, .. }
            | Instruction::SetCollectionTypeDefault { ty, .. } => ty,
            _ => return Ok(Some(instruction)),
        };
        if !ty.has_declared() {
            return Ok(Some(instruction));
        }
        let environment = &self.frames.last().expect("type instruction").environment;
        match self.materialize_type(ty, environment) {
            Ok(resolved) => *ty = resolved,
            Err(ResolutionFailure::Error(error)) => return Err(error),
            Err(ResolutionFailure::Need(need)) => {
                self.begin_declared_type(need, true)?;
                return Ok(None);
            }
        }
        Ok(Some(instruction))
    }

    fn begin_declared_type(
        &mut self,
        need: Need,
        retry_instruction: bool,
    ) -> Result<(), RuntimeError> {
        let recipe = self.program.type_recipes[need.recipe].clone();
        if recipe.path.is_empty() {
            // The `module` type uses the current module's class, including an
            // adopted parent class. This is distinct from importing a prototype.
            let value = self.reference_get_class(&Value::Object(need.owner))?;
            self.resolved_declarations
                .insert((need.recipe, need.owner), DeclarationValue::Value(value));
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
                let object = self.heap.get(id)?;
                let member = if first {
                    object
                        .members
                        .iter()
                        .find(|member| member.local && !member.method && member.name == name)
                        .or_else(|| {
                            object.members.iter().find(|member| {
                                !member.local && !member.method && member.name == name
                            })
                        })
                } else {
                    object
                        .members
                        .iter()
                        .find(|member| !member.local && !member.method && member.name == name)
                };
                if let Some(member) = member {
                    if !member.definition() && !member.import() {
                        return Err(RuntimeError::new(
                            ErrorKind::Type,
                            format!("Expected `{name}` to be a type, but it is not."),
                        )
                        .at(recipe.span));
                    }
                    let local = member.local;
                    self.frames
                        .last_mut()
                        .expect("type resolution frame")
                        .type_resolution = Some(DeclaredRun {
                        need,
                        phase: Phase::Type,
                        retry_instruction,
                        imported: None,
                    });
                    return self.force(id, id, name, local, true, false);
                }
                if first && let Some(fallback) = &recipe.builtin {
                    self.resolved_declarations.insert(
                        (need.recipe, need.owner),
                        DeclarationValue::Builtin(
                            fallback.clone().map_err(|error| error.at(recipe.span))?,
                        ),
                    );
                    if retry_instruction {
                        self.frames.last_mut().expect("type frame").pc -= 1;
                    }
                    return Ok(());
                }
                first = false;
                owner = object.parent;
            }
            return Err(RuntimeError::new(
                ErrorKind::MissingType,
                format!("Cannot find type `{name}`."),
            )
            .at(recipe.span));
        }
        let import = recipe
            .path
            .first()
            .ok_or_else(|| invalid("empty declared type path"))?;
        let member = self
            .heap
            .get(need.owner)?
            .members
            .iter()
            .find(|member| member.local && !member.method && member.name == *import)
            .cloned()
            .ok_or_else(|| {
                RuntimeError::new(
                    ErrorKind::Type,
                    format!("Cannot find module import `{import}`."),
                )
                .at(recipe.names[0])
            })?;
        if !member.import() {
            return Err(RuntimeError::new(
                ErrorKind::Type,
                format!("Expected `{import}` to be a module import, but it is not."),
            )
            .at(recipe.names[0]));
        }
        self.frames
            .last_mut()
            .expect("type resolution frame")
            .type_resolution = Some(DeclaredRun {
            need,
            phase: Phase::Import,
            retry_instruction,
            imported: None,
        });
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
                    if let Some(member) = object
                        .members
                        .iter()
                        .find(|member| !member.local && !member.method && member.name == name)
                    {
                        if !member.definition() {
                            return Err(RuntimeError::new(
                                ErrorKind::Type,
                                format!("Expected `{name}` to be a type, but it is not."),
                            )
                            .at(recipe.span));
                        }
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
                    .map_or(self.program.module_name.as_ref(), |unit| unit.name.as_ref());
                return Err(RuntimeError::new(
                    ErrorKind::Type,
                    format!("Cannot find type `{name}` in module `{module_name}`."),
                )
                .at(recipe.names[1]));
            }
            Phase::Type => {
                let value = self.pop()?;
                self.resolved_declarations.insert(
                    (run.need.recipe, run.need.owner),
                    DeclarationValue::Value(value),
                );
                if run.retry_instruction {
                    self.frames.last_mut().unwrap().pc -= 1;
                }
            }
        }
        Ok(())
    }
}

impl Compiler<'_> {
    pub(super) fn module_type(&mut self, span: Span) -> Result<TypeCheck, RuntimeError> {
        let recipe = self
            .program
            .type_recipes
            .iter_with_ids()
            .find(|(_, recipe)| {
                recipe.unit == self.unit && recipe.span == span && recipe.path.is_empty()
            })
            .map(|(id, _)| id);
        let recipe = match recipe {
            Some(id) => id,
            None => self.program.type_recipes.try_push(TypeRecipe {
                unit: self.unit,
                path: Vec::new(),
                names: Vec::new(),
                span,
                argument_spans: Vec::new(),
                builtin: None,
                reference: None,
            })?,
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
        let recipe = self
            .program
            .type_recipes
            .iter_with_ids()
            .find(|(_, recipe)| {
                recipe.unit == self.unit
                    && recipe.span == ty.span
                    && recipe
                        .path
                        .iter()
                        .map(AsRef::as_ref)
                        .eq(path.iter().map(String::as_str))
            })
            .map(|(id, _)| id);
        let recipe = match recipe {
            Some(id) => id,
            None => self.program.type_recipes.try_push(TypeRecipe {
                unit: self.unit,
                path: path.iter().map(|name| Arc::from(name.as_str())).collect(),
                names: name_spans.clone(),
                span: ty.span,
                argument_spans: actuals.iter().map(|argument| argument.span).collect(),
                builtin: None,
                reference: None,
            })?,
        };
        Ok(TypeCheck::Declared { recipe, arguments })
    }
}

impl Vm {
    /// Type substitution and validation have their own bounded native frame.
    /// Keeping them outside the general opcode match avoids retaining that
    /// match's debug-build temporaries during nested type metadata operations.
    pub(super) fn type_instruction(
        &mut self,
        instruction: Instruction,
    ) -> Result<(), RuntimeError> {
        match instruction {
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
                self.begin_type(ty, value, TypeDestination::Test, Vec::new())?;
            }
            Instruction::ResolveType(_) => {}
            Instruction::ResolvedClass(ty) => {
                if classes::BuiltinClass::from_type(&ty).is_some() {
                    let value = self.builtin_class(ty);
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
                    .generic_default = Some(generics::GenericDefaultRun::stack(ty, origin));
            }
            Instruction::ResolveClassType { name, local } => {
                let frame = self.frames.last().expect("active frame");
                let module = match frame.environment.scopes.last() {
                    Some(scope) => scope.receiver,
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
                    ty,
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
                self.begin_type(ty, value, TypeDestination::Stack, Vec::new())?;
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
                });
                let mut vm = Vm::new(Arc::new(program));
                let owner = vm.heap.allocate(Object {
                    is_module: true,
                    ..Object::default()
                });
                let class = vm.heap.allocate(Object {
                    class_definition: Some(Class {
                        id: 42,
                        module: Some("owner".into()),
                        name: "C".into(),
                        open: false,
                        abstract_: false,
                    }),
                    ..Object::default()
                });
                let class = vm.heap.allocate_class(class);
                vm.resolved_declarations
                    .insert((0, owner), DeclarationValue::Value(Value::Class(class)));
                let environment = Environment {
                    scopes: vec![Scope {
                        receiver: owner,
                        owner,
                    }],
                    ..Environment::default()
                };
                for branching in [false, true] {
                    let mut ty = TypeCheck::Declared {
                        recipe: 0,
                        arguments: Vec::new(),
                    }
                    .shared();
                    for _ in 0..20_000 {
                        ty = if branching {
                            TypeCheck::ParameterizedCollection {
                                kind: crate::heap::CollectionKind::Mapping,
                                arguments: vec![
                                    (ty.clone(), Span::new(0, 0)),
                                    (ty, Span::new(0, 0)),
                                ],
                            }
                        } else {
                            TypeCheck::Nullable(Box::new(ty))
                        }
                        .shared();
                    }
                    assert!(ty.has_declared());
                    let result = match vm.materialize_type(&ty, &environment) {
                        Ok(result) => result,
                        Err(_) => panic!("cached declaration should resolve"),
                    };
                    assert!(!result.has_declared());
                    let mut node = &result;
                    for _ in 0..20_000 {
                        node = if branching {
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
