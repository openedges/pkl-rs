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

    fn protobuf_type_class(&mut self, ty: &TypeCheck) -> Result<Option<ObjectId>, RuntimeError> {
        // TypeNode.getVmClass is deliberately null for constrained/union/unknown
        // nodes. Do not infer their class from the successful value.
        let ty = ty.resolved();
        match ty {
            TypeCheck::Alias(inner)
            | TypeCheck::GenericAlias(inner)
            | TypeCheck::NamedAlias {
                expanded: inner, ..
            } => return self.protobuf_type_class(inner),
            TypeCheck::IntegerAlias { .. } => {
                return Ok(Some(
                    self.canonical_builtin_class(classes::BuiltinClass::Int),
                ));
            }
            _ => {}
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
        wrapper: bool,
    ) -> Result<Option<TypeCheck>, RuntimeError> {
        use TypeCheck as T;
        let normalized = match ty.resolved() {
            T::IntegerAlias { .. } => T::Int,
            T::StringLiteral(_)
            | T::Union {
                string_literals: true,
                ..
            } => T::String,
            T::Nullable(inner)
            | T::Alias(inner)
            | T::GenericAlias(inner)
            | T::NamedAlias {
                expanded: inner, ..
            } => return self.protobuf_normalize_type(inner, wrapper),
            T::ParameterizedCollection {
                kind: CollectionKind::Listing,
                arguments,
            } => {
                let value = self
                    .protobuf_normalize_type(&arguments[0].0, wrapper)?
                    .ok_or_else(|| invalid("protobuf normalized Listing element"))?;
                if wrapper {
                    return Ok(None);
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
                let key = self.protobuf_normalize_type(&arguments[0].0, wrapper)?;
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
                let value = self
                    .protobuf_normalize_type(&arguments[1].0, wrapper)?
                    .ok_or_else(|| invalid("protobuf normalized Mapping value"))?;
                if wrapper {
                    return Ok(None);
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
                    let resolved = self.protobuf_normalize_type(element, wrapper)?.ok_or_else(|| RuntimeError::new(ErrorKind::Unsupported, "Pinned Protobuf union contains a null normalized collection node in wrapper context; Java internal-error projection remains unresolved."))?;
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
                    return Ok(types.into_iter().next());
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
            _ => return Ok(Some(ty.clone())),
        };
        Ok(Some(
            normalized.with_optional_source_section(ty.source_section()),
        ))
    }

    fn protobuf_alternatives(ty: &TypeCheck, output: &mut Vec<TypeCheck>) {
        if let TypeCheck::Union { types, .. } = ty.resolved() {
            for ty in types {
                if matches!(ty.resolved(), TypeCheck::Union { .. }) {
                    Self::protobuf_alternatives(ty, output);
                }
                // A nested union with no successful child still executes its own
                // TypeNode before the next sibling, exactly as computeName does.
                output.push(ty.clone());
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
        // resolveType creates a detached ListingTypeNode. Java04 confirms
        // that a matching converted Listing reaches getRootNode().getName()
        // and fails before forcing any elements. Do not claim a successful
        // wrapper while the internal error and known-subtype fast path are
        // unqualified. Nonmatching values still take the normal mismatch path.
        let collection_kind = match ty.resolved() {
            TypeCheck::ParameterizedCollection { kind, .. } | TypeCheck::Collection(kind)
                if matches!(kind, CollectionKind::Listing | CollectionKind::Mapping) =>
            {
                Some(*kind)
            }
            _ => None,
        };
        if let Some(kind) = collection_kind
            && TypeCheck::Collection(kind).accepts(&value, &self.heap)
        {
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
