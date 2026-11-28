mod protobuf_spark_sort {
    include!("protobuf_spark_sort.rs");
}
// Included in rendering.rs. Projection of pinned RendererNodes.resolveType;
// this metadata is not a replacement evaluator or a structural `is` test.
#[derive(Clone)]
struct ProtoType {
    ty: TypeCheck,
    environment: Environment,
    retained: Vec<Value>,
    declaration: Option<RuntimeFrame>,
}
impl ProtoType {
    fn roots(&self, roots: &mut Vec<Value>) {
        roots.extend(self.retained.iter().cloned());
        typecheck::environment_roots(&self.environment, roots);
    }
    fn with_type(&self, ty: TypeCheck) -> Self {
        Self { ty, ..self.clone() }
    }
}
impl Vm {
    /// A detached Protobuf type node keeps its authored source section but
    /// executes under the active renderer method's logical root. Source URI,
    /// text and span are independent from that root's display name.
    pub(super) fn protobuf_type_origin(
        &self,
        mut authored: RuntimeFrame,
        renderer: &BlockMetadata,
    ) -> RuntimeFrame {
        authored.source = authored
            .source
            .or_else(|| self.context.primary_source.clone())
            .map(|source| {
                Arc::new(crate::value::RuntimeSource {
                    module_name: renderer.source.as_ref().map_or_else(
                        || self.program.module_name.clone(),
                        |owner| owner.module_name.clone(),
                    ),
                    ..source.as_ref().clone()
                })
            });
        authored.member = renderer.member.clone();
        authored
    }

    fn protobuf_property_type(
        &mut self,
        receiver: ObjectId,
        name: &PklString,
    ) -> Result<Option<ProtoType>, RuntimeError> {
        if !classes::typed_instance(&self.heap, receiver)? {
            return Ok(None);
        }
        let Some(class) = self.heap.object_class(receiver)? else {
            return Ok(None);
        };
        let mut owner = Some(self.heap.get_class(class)?.prototype);
        let mut seen = BTreeSet::new();
        while let Some(id) = owner {
            if !seen.insert(id) {
                return Err(invalid("cyclic protobuf declaration hierarchy"));
            }
            let object = self.heap.get(id)?;
            let parent = object.parent;
            // Class.getProperty uses the transformed name, retaining an inherited
            // typed declaration when an override changes only the initializer.
            if let Some(member) = object
                .members
                .iter()
                .find(|member| {
                    !member.local
                        && !member.entry
                        && !member.method
                        && member.name == *name
                        && member.ty.is_some()
                })
                .cloned()
            {
                let mut environment = object
                    .member_environments
                    .get(&CacheKey {
                        owner: None,
                        name: name.clone(),
                        method: false,
                        entry: false,
                    })
                    .unwrap_or(&object.environment)
                    .clone();
                environment.outer_value = environment.this_value.clone();
                environment.this_value = Some(Value::Object(id));
                environment.custom_this = None;
                environment.executable_owner = environment
                    .executable_owner
                    .map(|parent| {
                        self.heap
                            .executable_child(parent, ExecutionSite::Member(member.code))
                    })
                    .transpose()?;
                if environment.restore_for_bindings {
                    let target = environment
                        .executable_owner
                        .unwrap_or_else(|| self.executable_root(member.code));
                    self.copy_captured_locals(&mut environment, target)?;
                    environment.restore_for_bindings = false;
                }
                environment.scopes.insert(
                    0,
                    Scope {
                        receiver: id,
                        owner: id,
                    },
                );
                let metadata = self.program.metadata.get(member.code);
                let declaration = member.type_span.map(|span| RuntimeFrame {
                    source: metadata.and_then(|metadata| metadata.source.clone()),
                    span,
                    member: metadata
                        .map(|metadata| metadata.member.clone())
                        .unwrap_or_default(),
                    anonymous: false,
                });
                return Ok(Some(ProtoType {
                    ty: member
                        .ty
                        .clone()
                        .expect("selected property type")
                        .substituted(&environment)?,
                    environment,
                    retained: vec![Value::Object(id), Value::Class(class)],
                    declaration,
                }));
            }
            owner = parent;
        }
        Ok(None)
    }

    fn protobuf_retain_type(&self, ty: &TypeCheck, retained: &mut Vec<Value>) {
        let mut pending = vec![ty];
        let mut seen = BTreeSet::new();
        while let Some(ty) = pending.pop() {
            let ty = ty.resolved();
            if !seen.insert(ty as *const TypeCheck) {
                continue;
            }
            if let TypeCheck::Class { id, .. } = ty {
                retained.extend(self.heap.find_class(*id).map(Value::Class));
            }
            if let TypeCheck::NamedAlias { declaration, .. } = ty {
                retained.extend(
                    self.alias_singletons
                        .get(declaration)
                        .copied()
                        .map(Value::TypeAlias),
                );
            }
            pending.extend(ty.type_children());
        }
    }

    fn protobuf_type_class(&mut self, mut ty: &TypeCheck) -> Result<Option<ObjectId>, RuntimeError> {
        // TypeNode.getVmClass is deliberately null for constrained/union/unknown
        // nodes. Do not infer their class from the successful value.
        let mut depth = 0usize;
        loop {
            ty = ty.resolved();
            match ty {
                TypeCheck::Alias(inner)
                | TypeCheck::GenericAlias(inner)
                | TypeCheck::NamedAlias {
                    expanded: inner, ..
                } => {
                    depth += 1;
                    if depth > MAX_TYPE_COMPILE_DEPTH {
                        return Err(RuntimeError::new(
                            ErrorKind::Type,
                            "Type expression exceeds maximum nesting depth.",
                        ));
                    }
                    ty = inner;
                }
                TypeCheck::IntegerAlias { .. } => {
                    return Ok(Some(
                        self.canonical_builtin_class(classes::BuiltinClass::Int),
                    ));
                }
                _ => break,
            }
        }
        if matches!(
            ty,
            TypeCheck::Constrained { .. }
                | TypeCheck::Union { .. }
                | TypeCheck::Unknown
                | TypeCheck::Nothing
                | TypeCheck::NonNull
        ) {
            return Ok(None);
        }
        if let Some(builtin) = classes::BuiltinClass::from_type(ty) {
            return Ok(Some(self.canonical_builtin_class(builtin)));
        }
        Ok(match ty {
            TypeCheck::Class { id, .. } => self.heap.find_class(*id),
            _ => None,
        })
    }

    fn protobuf_normalize_type(
        &mut self,
        ty: &TypeCheck,
        property_env: &Environment,
        wrapper: bool,
    ) -> Result<Option<Option<TypeCheck>>, RuntimeError> {
        let mut nodes = 0usize;
        self.protobuf_normalize_type_bounded(ty, property_env, wrapper, 0, &mut nodes)
    }

    fn protobuf_normalize_type_bounded(
        &mut self,
        mut ty: &TypeCheck,
        property_env: &Environment,
        wrapper: bool,
        mut depth: usize,
        nodes: &mut usize,
    ) -> Result<Option<Option<TypeCheck>>, RuntimeError> {
        use TypeCheck as T;
        loop {
            *nodes += 1;
            if *nodes > MAX_TYPE_COMPILE_NODES || depth > MAX_TYPE_COMPILE_DEPTH {
                return Err(RuntimeError::new(
                    ErrorKind::Type,
                    "Type expression exceeds maximum expansion complexity.",
                ));
            }
            match ty.resolved() {
                T::Nullable(inner) | T::Alias(inner) | T::GenericAlias(inner) => {
                    ty = inner;
                    depth += 1;
                }
                _ => break,
            }
        }
        let normalized = match ty.resolved() {
            T::IntegerAlias { .. } => T::Int,
            T::StringLiteral(_)
            | T::Union {
                string_literals: true,
                ..
            } => T::String,
            T::NamedAlias {
                declaration,
                expanded: inner,
                ..
            } => {
                // Upstream RendererNodes.resolveType calls getVmTypeAlias().getTypeNode(),
                // normalizing the unparameterized alias body where TypeVariableNode leaves
                // unbound type parameters as Unknown (getVmClass() == null).
                if let Some(metadata) = self.program.alias_definition(*declaration).cloned()
                    && !metadata.parameters.is_empty()
                {
                    let mut lexical = self
                        .alias_check_environment(*declaration, property_env)?
                        .ok_or_else(|| invalid("protobuf type alias lexical owner missing"))?;
                    for name in &metadata.parameters {
                        lexical
                            .type_arguments
                            .insert((*declaration, name.clone()), T::Unknown);
                    }
                    let body = metadata.body.substituted(&lexical)?;
                    let Some(unparameterized) =
                        self.resolve_runtime_type(&body, &lexical, false)?
                    else {
                        return Ok(None);
                    };
                    return self.protobuf_normalize_type_bounded(
                        &unparameterized,
                        &lexical,
                        wrapper,
                        depth + 1,
                        nodes,
                    );
                }
                let next_env = self
                    .alias_check_environment(*declaration, property_env)?
                    .unwrap_or_else(|| property_env.clone());
                return self.protobuf_normalize_type_bounded(
                    inner,
                    &next_env,
                    wrapper,
                    depth + 1,
                    nodes,
                );
            }
            T::ParameterizedCollection {
                kind: CollectionKind::Listing,
                arguments,
            } => {
                let Some(inner) = self.protobuf_normalize_type_bounded(
                    &arguments[0].0,
                    property_env,
                    wrapper,
                    depth + 1,
                    nodes,
                )?
                else {
                    return Ok(None);
                };
                let value = inner.ok_or_else(|| invalid("protobuf normalized Listing element"))?;
                if wrapper {
                    return Ok(Some(None));
                }
                T::ParameterizedCollection {
                    kind: CollectionKind::Listing,
                    arguments: vec![(value, arguments[0].1)],
                }
            }
            T::ParameterizedCollection {
                kind: CollectionKind::Mapping,
                arguments,
            } => {
                let Some(key) = self.protobuf_normalize_type_bounded(
                    &arguments[0].0,
                    property_env,
                    wrapper,
                    depth + 1,
                    nodes,
                )?
                else {
                    return Ok(None);
                };
                if !matches!(
                    key.as_ref().map(TypeCheck::resolved),
                    Some(T::Int | T::String | T::Boolean)
                ) {
                    let mut error = RuntimeError::new(
                        ErrorKind::Type,
                        "Cannot render map with a non-scalar, floating point or byte array key type.",
                    );
                    if let Some(section) = ty.source_section() {
                        error.span = Some(section.span());
                        let authored = RuntimeFrame {
                            source: section.external_source(),
                            span: section.span(),
                            member: "".into(),
                            anonymous: false,
                        };
                        let authored = self
                            .frames
                            .last()
                            .and_then(|frame| self.program.metadata.get(frame.block))
                            .map_or_else(
                                || authored.clone(),
                                |renderer| self.protobuf_type_origin(authored.clone(), renderer),
                            );
                        error.frames.push(authored);
                    }
                    return Err(error);
                }
                let Some(inner) = self.protobuf_normalize_type_bounded(
                    &arguments[1].0,
                    property_env,
                    wrapper,
                    depth + 1,
                    nodes,
                )?
                else {
                    return Ok(None);
                };
                let value = inner.ok_or_else(|| invalid("protobuf normalized Mapping value"))?;
                if wrapper {
                    return Ok(Some(None));
                }
                T::ParameterizedCollection {
                    kind: CollectionKind::Mapping,
                    arguments: vec![(key.unwrap(), arguments[0].1), (value, arguments[1].1)],
                }
            }
            T::Union { types, .. } => {
                let mut elements = Vec::new();
                let mut has_string = false;
                for element in types {
                    let Some(resolved) = self.protobuf_normalize_type_bounded(
                        element,
                        property_env,
                        wrapper,
                        depth + 1,
                        nodes,
                    )?
                    else {
                        return Ok(None);
                    };
                    let resolved = resolved.ok_or_else(|| RuntimeError::new(ErrorKind::Unsupported, "Pinned Protobuf union contains a null normalized collection node in wrapper context; Java internal-error projection remains unresolved."))?;
                    if matches!(resolved.resolved(), T::String) {
                        if has_string {
                            continue;
                        }
                        has_string = true;
                    }
                    elements.push(resolved);
                }
                // Match the comparator and TimSort control flow used by the
                // pinned JDK. Even identical classes compare Less; this is not
                // a total order accepted by Rust's general-purpose sorting API.
                let mut keyed = Vec::new();
                for element in elements {
                    let object_slot = matches!(
                        element.resolved(),
                        T::String
                            | T::Bytes
                            | T::Duration
                            | T::Class { .. }
                            | T::Collection(_)
                            | T::ParameterizedCollection { .. }
                            | T::CollectionBase { .. }
                            | T::Pair { .. }
                            | T::Reference { .. }
                            | T::Function { .. }
                            | T::MetaClass(_)
                            | T::RefDomain
                    );
                    let class = if object_slot {
                        self.protobuf_type_class(&element)?
                    } else {
                        None
                    };
                    keyed.push((element, class));
                }
                let order = protobuf_spark_sort::order(keyed.len(), |left, right| {
                    match (keyed[left].1, keyed[right].1) {
                        (Some(a), Some(b)) if self.heap.is_subclass(a, b) => -1,
                        (Some(a), Some(b)) if self.heap.is_subclass(b, a) => 1,
                        _ => 0,
                    }
                }).map_err(|message| RuntimeError::new(ErrorKind::Unsupported,
                    format!("Pinned Java Protobuf comparator failure; exact Java exception projection remains unqualified: {message}")))?;
                let types: Vec<_> = order
                    .into_iter()
                    .map(|index| keyed[index].0.clone())
                    .collect();
                if has_string && types.len() == 1 {
                    return Ok(Some(types.into_iter().next()));
                }
                T::Union {
                    spans: vec![Span::new(0, 0); types.len()],
                    labels: vec![Span::new(0, 0); types.len()],
                    types,
                    string_literals: false,
                    default: None,
                }
            }
            // Unlike Listing/Mapping, List/Map/Set arguments are NOT normalized.
            _ => return Ok(Some(Some(ty.clone()))),
        };
        Ok(Some(Some(
            normalized.with_optional_source_section(ty.source_section()),
        )))
    }

    fn protobuf_alternatives(ty: &TypeCheck, output: &mut Vec<TypeCheck>) {
        enum AltWork<'a> {
            Expand(&'a TypeCheck),
            Emit(&'a TypeCheck),
        }
        let mut stack = vec![AltWork::Expand(ty)];
        let mut emitted = 0usize;
        while let Some(step) = stack.pop() {
            match step {
                AltWork::Expand(cur) => {
                    if let TypeCheck::Union { types, .. } = cur.resolved() {
                        for item in types.iter().rev() {
                            // A nested union with no successful child still executes its own
                            // TypeNode before the next sibling, exactly as computeName does.
                            stack.push(AltWork::Emit(item));
                            if matches!(item.resolved(), TypeCheck::Union { .. }) {
                                stack.push(AltWork::Expand(item));
                            }
                        }
                    }
                }
                AltWork::Emit(item) => {
                    emitted += 1;
                    if emitted > MAX_TYPE_COMPILE_NODES {
                        break;
                    }
                    output.push(item.clone());
                }
            }
        }
    }

    fn protobuf_open_class(&mut self, ty: &TypeCheck) -> Result<Option<ObjectId>, RuntimeError> {
        if !matches!(ty.resolved(), TypeCheck::Class { .. }) {
            return Ok(None);
        }
        let Some(class) = self.protobuf_type_class(ty)? else {
            return Ok(None);
        };
        // TypedTypeNode and module-specific TypeNodes are ObjectSlot nodes,
        // not NonFinalClassTypeNode, even when their runtime class is open.
        if matches!(
            ty.resolved(),
            TypeCheck::Class {
                id: classes::TYPED,
                ..
            }
        ) || self
            .heap
            .get(self.heap.get_class(class)?.prototype)?
            .is_module
        {
            return Ok(None);
        }
        let definition = self.heap.class_definition(class)?;
        Ok((definition.open || definition.abstract_).then_some(class))
    }

    fn protobuf_begin_alternative(
        &mut self,
        ty: TypeCheck,
        value: Value,
        context: &ProtoType,
    ) -> Result<(), RuntimeError> {
        let collection = match ty.resolved() {
            TypeCheck::Collection(kind @ (CollectionKind::Listing | CollectionKind::Mapping)) => {
                Some((*kind, None, None))
            }
            TypeCheck::ParameterizedCollection {
                kind: CollectionKind::Listing,
                arguments,
            } => Some((CollectionKind::Listing, None, Some(arguments[0].0.clone()))),
            TypeCheck::ParameterizedCollection {
                kind: CollectionKind::Mapping,
                arguments,
            } => Some((
                CollectionKind::Mapping,
                Some(arguments[0].0.clone()),
                Some(arguments[1].0.clone()),
            )),
            _ => None,
        };
        if let Some((kind, key_ty, value_ty)) = collection
            && TypeCheck::Collection(kind).accepts(&value, &self.heap)
        {
            let Value::Object(id) = value else {
                unreachable!("collection value is an object")
            };
            // MappingTypeNode.executeLazily eagerly checks all mapping keys
            // before checking isValueTypeKnownSubtypeOf (TypeNode.java:1634).
            if kind == CollectionKind::Mapping
                && let Some(key_ty) = &key_ty
                && !key_ty.noop()
            {
                let mut current = Some(id);
                let mut seen = BTreeSet::new();
                while let Some(owner) = current {
                    if !seen.insert(owner) {
                        return Err(RuntimeError::new(
                            ErrorKind::Cycle,
                            "cyclic mapping parent chain",
                        ));
                    }
                    let object = self.heap.get(owner)?;
                    for member in &object.collection_members {
                        if member.kind != CollectionMemberKind::Property
                            && !key_ty.accepts(&member.key, &self.heap)
                        {
                            // Detached MappingTypeNode fails inside doEagerCheck's
                            // catch block at getRootNode().getCallTarget() (TypeNode.java:1865).
                            return Err(RuntimeError::foreign(
                                crate::java_exception::provenance::ForeignOperation::ProtobufMappingKeyRootNullPointer,
                                "Cannot invoke \"com.oracle.truffle.api.nodes.RootNode.getCallTarget()\" because the return value of \"org.pkl.core.ast.type.TypeNode$ListingOrMappingTypeNode.getRootNode()\" is null",
                            ));
                        }
                    }
                    current = object.parent;
                }
            }
            // VmListingOrMapping.isValueTypeKnownSubtypeOf (VmListingOrMapping.java:131):
            // elides getValueTypeCastNode() when valueTypeNode is a noop check or
            // matches this object's own reified typeCastNode.
            let known_subtype = match &value_ty {
                None => true,
                Some(value_ty) => {
                    value_ty.noop()
                        || self
                            .heap
                            .get(id)?
                            .collection_cast
                            .as_ref()
                            .is_some_and(|cast| cast.ty.equivalent(value_ty))
                }
            };
            if known_subtype {
                self.stack.push(Value::Bool(true));
                return Ok(());
            }
            let is_mapping = kind == CollectionKind::Mapping;
            return Err(RuntimeError::foreign(
                crate::java_exception::provenance::ForeignOperation::ProtobufListingOrMappingRootNullPointer {
                    is_mapping,
                },
                "Cannot invoke \"com.oracle.truffle.api.nodes.RootNode.getName()\" because the return value of \"org.pkl.core.ast.type.TypeNode$ListingOrMappingTypeNode.getRootNode()\" is null",
            ));
        }
        // Use the declaration's captured lexical context, not the renderer's
        // method locals. TypeRun owns a clone across constraint/Host suspension.
        let index = self.frames.len() - 1;
        let previous = std::mem::replace(
            &mut self.frames[index].environment,
            context.environment.clone(),
        );
        let result = self.begin_type(
            ty,
            value,
            TypeDestination::ProtobufAlternative,
            context.declaration.iter().cloned().collect(),
        );
        self.frames[index].environment = previous;
        result
    }
}
