// Included in rendering.rs so binary traversal shares its private continuation,
// converter paths, source frames and GC roots with the text renderers.
impl Vm {
    #[cfg_attr(target_arch = "wasm32", inline(never))]
    pub(super) fn begin_pkl_binary_render(&mut self, document: bool) -> Result<(), RuntimeError> {
        let root = self
            .frames
            .last()
            .and_then(|frame| frame.environment.locals.last())
            .cloned()
            .ok_or_else(|| invalid("pkl-binary renderer argument"))?;
        self.stack.push(root);
        self.begin_render(document, Format::PklBinary)
    }

    /// Encodes the top-of-stack `Value` directly as a `pkl-binary` document
    /// (`Value::Bytes`) with no converters, matching `EvaluatorImpl.evaluateExpressionPklBinary`.
    #[cfg_attr(target_arch = "wasm32", inline(never))]
    pub(super) fn begin_pkl_binary_encode_value(&mut self) -> Result<(), RuntimeError> {
        let root = self.pop()?.boxed();
        if let Some(bytes) = self.try_fast_encode_pkl_binary(&root) {
            self.stack.push(Value::Bytes(bytes.into()));
            return Ok(());
        }
        let receiver = self.canonical_builtin_class(classes::BuiltinClass::Any);
        self.frames.last_mut().expect("renderer frame").pcf_render = Some(Box::new(RenderRun {
            receiver,
            root: root.clone(),
            document_original: None,
            document: true,
            format: Format::PklBinary,
            // Skip `converters` / `convertPropertyTransformers` reads on `receiver`.
            option: 4,
            options: Vec::new(),
            await_: None,
            tasks: vec![Task::Visit {
                value: root,
                depth: 0,
                standalone: false,
                root: true,
                string_element: false,
                path: vec![PathPart::Root],
                converted: false,
            }],
            active: BTreeSet::new(),
            output: Default::default(),
            binary: Default::default(),
            protobuf: Default::default(),
            indent: Default::default(),
            omit_null: false,
            restrict_charset: false,
            properties_document: false,
            yaml: None,
            yaml_stream: false,
            xml_line: 0,
            custom: false,
            started: true,
            has_value_converters: false,
            converters: Converters::default(),
            source: None,
            source_overrides: crate::heap::FastMap::default(),
        }));
        Ok(())
    }

    fn binary_class_identity(
        &mut self,
        value: &Value,
    ) -> Result<(PklString, PklString), RuntimeError> {
        let class = match classes::runtime_class(value, &self.heap)? {
            classes::RuntimeClass::Declared(class) => class,
            classes::RuntimeClass::Builtin(class) => self.canonical_builtin_class(class),
        };
        self.binary_declared_class_identity(class)
    }

    pub(super) fn binary_declared_class_identity(
        &self,
        class: ObjectId,
    ) -> Result<(PklString, PklString), RuntimeError> {
        let name = self.heap.class_display_name(class)?;
        let definition = self.heap.class_definition(class)?;
        let uri = definition
            .origin_uri
            .clone()
            .ok_or_else(|| invalid("pkl-binary class has no declaration URI"))?;
        Ok((name, uri))
    }

    #[cfg_attr(target_arch = "wasm32", inline(never))]
    fn binary_alias_identity(
        &self,
        alias: ObjectId,
    ) -> Result<(PklString, PklString), RuntimeError> {
        let object = self.heap.get(alias)?;
        let stored_name = object
            .type_alias
            .clone()
            .ok_or_else(|| invalid("pkl-binary type alias name"))?;
        let metadata = object
            .declaration_code
            .and_then(|code| self.program.metadata.get(code));
        let qualified = object
            .declaration_code
            .and_then(|code| self.program.declaration_source(code))
            .zip(metadata.and_then(|metadata| metadata.source.as_ref()))
            .map(|(declaration, source)| {
                source
                    .module_name
                    .concat(&"#".into())
                    .concat(&declaration.name)
            })
            .unwrap_or(stored_name);
        let uri = metadata
            .and_then(|metadata| metadata.unit)
            .and_then(|unit| self.program.units.get(unit.0 as usize))
            .and_then(|unit| {
                unit.key.as_ref().map(|key| key.uri.clone()).or_else(|| {
                    unit.identity
                        .as_ref()
                        .map(|identity| identity.original_uri.clone())
                })
            })
            .or_else(|| {
                metadata
                    .and_then(|metadata| metadata.source.as_ref())
                    .map(|source| source.uri.clone())
            })
            .or_else(|| {
                (object.declaration_code.is_none() && object.type_alias_declaration.is_none())
                    .then(|| "pkl:base".into())
            })
            .ok_or_else(|| invalid("pkl-binary type alias has no declaration URI"))?;
        let name = if uri == "pkl:base" {
            crate::source::SourceText::from(&qualified)
                .entire()
                .strip_prefix("pkl.base#")
                .map(|name| name.to_pkl_string())
                .unwrap_or(qualified)
        } else {
            qualified
        };
        Ok((name, uri))
    }

    #[allow(clippy::too_many_arguments)]
    #[cfg_attr(target_arch = "wasm32", inline(never))]
    fn binary_visit(
        &mut self,
        run: &mut RenderRun,
        value: Value,
        depth: usize,
        standalone: bool,
        root: bool,
        path: Vec<PathPart>,
    ) -> Result<bool, RuntimeError> {
        match value {
            Value::Class(class) => {
                let (name, uri) = self.binary_declared_class_identity(class)?;
                run.binary.array(3)?;
                run.binary.integer(0x0c)?;
                run.binary.string(&name)?;
                run.binary.string(&uri)?;
            }
            Value::TypeAlias(alias) => {
                let (name, uri) = self.binary_alias_identity(alias)?;
                run.binary.array(3)?;
                run.binary.integer(0x0d)?;
                run.binary.string(&name)?;
                run.binary.string(&uri)?;
            }
            Value::Object(receiver) => {
                if self.is_render_directive(receiver)? {
                    run.await_ = Some(Await::Directive { receiver, path });
                    self.force(receiver, receiver, "bytes".into(), false, true, false)?;
                    return Ok(false);
                }
                let object = self.heap.get(receiver)?;
                if let Some(pair) = object.pair.clone() {
                    run.binary.array(3)?;
                    run.binary.integer(0x09)?;
                    // VmPklBinaryEncoder.visitPair calls visit directly, without
                    // converter selection or an added member path component.
                    run.value(pair[1].clone(), depth, false, false, false, path.clone());
                    run.value(pair[0].clone(), depth, false, false, false, path);
                    return Ok(false);
                }
                if let Some(reference) = object.reference.clone() {
                    run.binary.array(4)?;
                    run.binary.integer(0x20)?;
                    for access in reference.path.iter().rev() {
                        run.value(access.clone(), depth, false, false, false, path.clone());
                    }
                    run.tasks.push(Task::BinaryArray(reference.path.len()));
                    run.value(reference.data, depth, false, false, false, path.clone());
                    run.value(reference.domain, depth, false, false, false, path);
                    return Ok(false);
                }
                if object.foreign_map_entry.is_some() {
                    return Err(RuntimeError::foreign(
                        crate::java_exception::provenance::ForeignOperation::KeyValueHolderTypeVisitor { is_set: false },
                        "Unknown VM value type: java.util.KeyValueHolder",
                    ));
                }
                let strict = matches!(
                    object.collection_kind,
                    CollectionKind::List | CollectionKind::Set | CollectionKind::Map
                );
                if !run.active.insert(receiver) {
                    return Err(RuntimeError::new(
                        ErrorKind::Cycle,
                        "cyclic value rendering",
                    ));
                }
                if !run.has_value_converters
                    && object.collection_kind == CollectionKind::List
                    && object.parent.is_none()
                    && let Some(elems) = &object.list_elements
                {
                    let len = elems.len();
                    let checkpoint = run.binary.len();
                    let mut local_active = Vec::with_capacity(8);
                    let mut scratch = Vec::with_capacity(16);
                    if self.try_fast_encode_binary_object(
                        receiver,
                        &mut run.binary,
                        Some(&run.active),
                        &mut local_active,
                        &mut scratch,
                    ) {
                        run.active.remove(&receiver);
                        return Ok(false);
                    }
                    run.binary.truncate(checkpoint);
                    run.binary.array(2)?;
                    run.binary.integer(0x04)?;
                    run.binary.array(len)?;
                    run.tasks.push(Task::FastList {
                        receiver,
                        index: 0,
                        depth,
                        path,
                    });
                    return Ok(false);
                }
                run.tasks.push(Task::OpenObject {
                    receiver,
                    depth,
                    standalone,
                    root,
                    path,
                    stream: false,
                });
                if !strict {
                    self.begin_shallow_force_for_render(run, receiver)?;
                }
            }
            value => run.binary.scalar(&value)?,
        }
        Ok(false)
    }

    pub(super) fn binary_regular_property_count(&self, receiver: ObjectId) -> Result<usize, RuntimeError> {
        let class = self
            .heap
            .object_class(receiver)?
            .ok_or_else(|| invalid("pkl-binary Typed class"))?;
        let mut owner = Some(self.heap.get_class(class)?.prototype);
        let mut visited = BTreeSet::new();
        let mut names = BTreeSet::new();
        while let Some(id) = owner {
            if !visited.insert(id) {
                return Err(RuntimeError::new(
                    ErrorKind::Cycle,
                    "cyclic class parent chain",
                ));
            }
            let prototype = self.heap.get(id)?;
            for member in &prototype.members {
                // VmClass collects each class's declared regular names, then
                // unions its superclass's independently collected names. An
                // external override must not remove a regular ancestor name.
                if !member.local
                    && !member.method
                    && !member.entry
                    && !member.definition()
                    && !member.import()
                    && !member.external()
                    && !collections::hidden_property(&self.heap, id, &member.name, false)?
                {
                    names.insert(member.name.clone());
                }
            }
            owner = prototype.parent;
        }
        Ok(names.len())
    }

    #[cfg_attr(target_arch = "wasm32", inline(never))]
    fn binary_start(
        &mut self,
        run: &mut RenderRun,
        receiver: ObjectId,
    ) -> Result<(), RuntimeError> {
        let object = self.heap.get(receiver)?;
        let kind = object.collection_kind;
        let length = object.length;
        if kind == CollectionKind::Dynamic {
            let typed = classes::typed_instance(&self.heap, receiver)? || object.is_module;
            let count = if typed {
                // Count class declarations, independently of cached equality
                // slots and the renderer's emitted-member traversal.
                self.binary_regular_property_count(receiver)?
            } else {
                // Preserve VmDynamic's cached count even for a reentrant force
                // prefix; counting emitted members would silently repair bytes.
                self.dynamic_regular_member_count(receiver)?
            };
            let (name, uri) = self.binary_class_identity(&Value::Object(receiver))?;
            run.binary.array(4)?;
            run.binary.integer(0x01)?;
            run.binary.string(&name)?;
            run.binary.string(&uri)?;
            run.binary.array(count)?;
        } else {
            let tag = match kind {
                CollectionKind::Map => 0x02,
                CollectionKind::Mapping => 0x03,
                CollectionKind::List => 0x04,
                CollectionKind::Listing => 0x05,
                CollectionKind::Set => 0x06,
                CollectionKind::Dynamic => unreachable!(),
            };
            run.binary.array(2)?;
            run.binary.integer(tag)?;
            if matches!(kind, CollectionKind::Map | CollectionKind::Mapping) {
                let length = if kind == CollectionKind::Mapping {
                    self.heap
                        .get(receiver)?
                        .mapping_length
                        .ok_or_else(|| invalid("pkl-binary Mapping length"))?
                } else {
                    length
                };
                run.binary.map(length)?;
            } else {
                run.binary.array(length)?;
            }
        }
        Ok(())
    }

    fn binary_prepare_mapping_length(&mut self, receiver: ObjectId) -> Result<(), RuntimeError> {
        // Use the same parent-first key sequence as collection_builtin's
        // Mapping.length. CountKey retains keys/receiver across Host and GC;
        // unlike the property instruction, this continuation must not retry PC.
        let mut chain = Vec::new();
        let mut owner = Some(receiver);
        while let Some(id) = owner {
            chain.push(id);
            owner = self.heap.get(id)?.parent;
        }
        let mut keys = Vec::new();
        for id in chain.into_iter().rev() {
            keys.extend(
                self.heap
                    .get(id)?
                    .collection_members
                    .iter()
                    .filter(|member| member.kind == CollectionMemberKind::Entry)
                    .map(|member| member.key.clone()),
            );
        }
        self.count_render_mapping_keys(receiver, keys);
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    #[cfg_attr(target_arch = "wasm32", inline(never))]
    fn binary_member(
        &self,
        run: &mut RenderRun,
        mut members: Box<Members>,
        descriptor: CollectionMember,
        key: Value,
        value: Value,
        output_name: Option<PklString>,
        previous_source: Option<Box<RuntimeFrame>>,
        path: Vec<PathPart>,
    ) -> Result<(), RuntimeError> {
        let object = self.heap.get(members.receiver)?;
        let dynamic = object.collection_kind == CollectionKind::Dynamic
            && !object.is_module
            && !classes::typed_instance(&self.heap, members.receiver)?;
        let depth = members.depth;
        members.emitted = true;
        match descriptor.kind {
            CollectionMemberKind::Property => {
                let name = output_name.unwrap_or_else(|| match descriptor.key {
                    Value::String(name) => name,
                    _ => unreachable!("property String key"),
                });
                run.binary.array(3)?;
                run.binary.integer(0x10)?;
                run.binary.string(&name)?;
            }
            CollectionMemberKind::Entry if dynamic => {
                run.binary.array(3)?;
                run.binary.integer(0x11)?;
            }
            CollectionMemberKind::Element if dynamic => {
                run.binary.array(3)?;
                run.binary.integer(0x12)?;
                let Value::Int(index) = descriptor.key else {
                    return Err(invalid("pkl-binary element index"));
                };
                run.binary.integer(*index)?;
            }
            _ => (),
        }
        run.tasks.push(Task::Members(members));
        run.tasks.push(Task::RestoreSource(previous_source));
        run.value(value, depth, false, false, false, path);
        if descriptor.kind == CollectionMemberKind::Entry {
            run.value(key, depth, false, false, false, Vec::new());
        }
        Ok(())
    }

    /// Direct zero-task `pkl-binary` encoder for already-forced acyclic scalars,
    /// Pairs, strict Lists/Sets/Maps, plain Dynamics/Listings/Mappings, and
    /// top-level Modules. Returns `None` whenever any member needs evaluation,
    /// inheritance traversal, cycle diagnostics, converters, or special handling,
    /// falling back to the full resumable `RenderRun` with zero side effects.
    fn try_fast_encode_pkl_binary(&self, root: &Value) -> Option<Vec<u8>> {
        let mut wire = super::pkl_binary_wire::BinaryWire::with_capacity(4096);
        let mut active = Vec::with_capacity(8);
        let mut scratch: Vec<FastMember<'_>> = Vec::with_capacity(16);
        if self.try_fast_encode_binary_value(root, &mut wire, None, &mut active, &mut scratch) {
            Some(wire.into_bytes())
        } else {
            None
        }
    }

    fn try_fast_encode_binary_value<'a>(
        &'a self,
        value: &'a Value,
        wire: &mut super::pkl_binary_wire::BinaryWire,
        outer_active: Option<&BTreeSet<ObjectId>>,
        active: &mut Vec<ObjectId>,
        scratch: &mut Vec<FastMember<'a>>,
    ) -> bool {
        // Abort the synchronous fast path on huge DAG expansions so the cooperative
        // `RenderRun` loop can enforce step/timeout/cancellation limits cleanly.
        const MAX_FAST_BINARY_BYTES: usize = 32 * 1024 * 1024;
        if wire.len() > MAX_FAST_BINARY_BYTES {
            return false;
        }
        match value {
            Value::Null
            | Value::NullWithDefault(_)
            | Value::Bool(_)
            | Value::Int(_)
            | Value::Float(_)
            | Value::String(_)
            | Value::Duration(_)
            | Value::DataSize(_)
            | Value::Bytes(_)
            | Value::IntSeq(_)
            | Value::Regex(_)
            | Value::Function(_) => wire.scalar(value).is_ok(),
            Value::Class(_) | Value::TypeAlias(_) => false,
            Value::Object(id) => {
                if active.len() >= 32
                    || active.contains(id)
                    || outer_active.is_some_and(|set| set.contains(id))
                {
                    return false;
                }
                let Some(obj) = self.heap.get_fast(*id) else {
                    return false;
                };
                active.push(*id);
                let ok = if let Some(pair) = obj.pair.as_deref() {
                    wire.array(3).is_ok()
                        && wire.integer(0x09).is_ok()
                        && self.try_fast_encode_binary_value(
                            &pair[0],
                            wire,
                            outer_active,
                            active,
                            scratch,
                        )
                        && self.try_fast_encode_binary_value(
                            &pair[1],
                            wire,
                            outer_active,
                            active,
                            scratch,
                        )
                } else {
                    self.try_fast_encode_binary_object(*id, wire, outer_active, active, scratch)
                };
                active.pop();
                ok
            }
        }
    }

    /// Encodes an already-open object (already tracked in `outer_active` or `active`)
    /// directly into `wire`. On `false`, callers must truncate `wire` to their saved checkpoint.
    fn try_fast_encode_binary_object<'a>(
        &'a self,
        id: ObjectId,
        wire: &mut super::pkl_binary_wire::BinaryWire,
        outer_active: Option<&BTreeSet<ObjectId>>,
        active: &mut Vec<ObjectId>,
        scratch: &mut Vec<FastMember<'a>>,
    ) -> bool {
        let base = scratch.len();
        let Some(kind) = self.extract_fast_object(id, scratch, false) else {
            return false;
        };
        let ok = match kind {
            FastObjectKind::StrictList(elems) => {
                wire.array(2).is_ok()
                    && wire.integer(0x04).is_ok()
                    && wire.array(elems.len()).is_ok()
                    && elems.iter().all(|elem| {
                        self.try_fast_encode_binary_value(elem, wire, outer_active, active, scratch)
                    })
            }
            FastObjectKind::EmptyList => {
                wire.array(2).is_ok() && wire.integer(0x04).is_ok() && wire.array(0).is_ok()
            }
            FastObjectKind::StrictSet(count) => {
                let end = base + count;
                wire.array(2).is_ok()
                    && wire.integer(0x06).is_ok()
                    && wire.array(count).is_ok()
                    && (base..end).all(|i| match scratch[i] {
                        FastMember::Element(_, val) => self.try_fast_encode_binary_value(
                            val,
                            wire,
                            outer_active,
                            active,
                            scratch,
                        ),
                        _ => false,
                    })
            }
            FastObjectKind::StrictMap(count) => {
                let end = base + count;
                wire.array(2).is_ok()
                    && wire.integer(0x02).is_ok()
                    && wire.map(count).is_ok()
                    && (base..end).all(|i| match scratch[i] {
                        FastMember::Entry(k, v) => {
                            self.try_fast_encode_binary_value(
                                k,
                                wire,
                                outer_active,
                                active,
                                scratch,
                            ) && self.try_fast_encode_binary_value(
                                v,
                                wire,
                                outer_active,
                                active,
                                scratch,
                            )
                        }
                        _ => false,
                    })
            }
            FastObjectKind::PlainListing(count) => {
                let end = base + count;
                wire.array(2).is_ok()
                    && wire.integer(0x05).is_ok()
                    && wire.array(count).is_ok()
                    && (base..end).all(|i| match scratch[i] {
                        FastMember::Element(_, val) => self.try_fast_encode_binary_value(
                            val,
                            wire,
                            outer_active,
                            active,
                            scratch,
                        ),
                        _ => false,
                    })
            }
            FastObjectKind::PlainMapping(count) => {
                let end = base + count;
                self.heap
                    .get_fast(id)
                    .is_some_and(|obj| obj.mapping_length == Some(count))
                    && wire.array(2).is_ok()
                    && wire.integer(0x03).is_ok()
                    && wire.map(count).is_ok()
                    && (base..end).all(|i| match scratch[i] {
                        FastMember::Entry(k, v) => {
                            self.try_fast_encode_binary_value(
                                k,
                                wire,
                                outer_active,
                                active,
                                scratch,
                            ) && self.try_fast_encode_binary_value(
                                v,
                                wire,
                                outer_active,
                                active,
                                scratch,
                            )
                        }
                        _ => false,
                    })
            }
            FastObjectKind::PlainDynamic { count, .. } => {
                let end = base + count;
                wire.array(4).is_ok()
                    && wire.integer(0x01).is_ok()
                    && wire.str_bytes("Dynamic").is_ok()
                    && wire.str_bytes("pkl:base").is_ok()
                    && wire.array(count).is_ok()
                    && (base..end).all(|i| match scratch[i] {
                        FastMember::Property(name, val) => {
                            wire.array(3).is_ok()
                                && wire.integer(0x10).is_ok()
                                && wire.string(name).is_ok()
                                && self.try_fast_encode_binary_value(
                                    val,
                                    wire,
                                    outer_active,
                                    active,
                                    scratch,
                                )
                        }
                        FastMember::Entry(key, val) => {
                            wire.array(3).is_ok()
                                && wire.integer(0x11).is_ok()
                                && self.try_fast_encode_binary_value(
                                    key,
                                    wire,
                                    outer_active,
                                    active,
                                    scratch,
                                )
                                && self.try_fast_encode_binary_value(
                                    val,
                                    wire,
                                    outer_active,
                                    active,
                                    scratch,
                                )
                        }
                        FastMember::Element(idx, val) => {
                            wire.array(3).is_ok()
                                && wire.integer(0x12).is_ok()
                                && wire.integer(idx).is_ok()
                                && self.try_fast_encode_binary_value(
                                    val,
                                    wire,
                                    outer_active,
                                    active,
                                    scratch,
                                )
                        }
                    })
            }
            FastObjectKind::PlainTyped { .. } => false,
            FastObjectKind::PlainModule { class, count } => {
                let end = base + count;
                if self.binary_regular_property_count(id).ok() != Some(count) {
                    false
                } else if let Ok((class_name, class_uri)) =
                    self.binary_declared_class_identity(class)
                {
                    wire.array(4).is_ok()
                        && wire.integer(0x01).is_ok()
                        && wire.string(&class_name).is_ok()
                        && wire.string(&class_uri).is_ok()
                        && wire.array(count).is_ok()
                        && (base..end).all(|i| match scratch[i] {
                            FastMember::Property(name, val) => {
                                wire.array(3).is_ok()
                                    && wire.integer(0x10).is_ok()
                                    && wire.string(name).is_ok()
                                    && self.try_fast_encode_binary_value(
                                        val,
                                        wire,
                                        outer_active,
                                        active,
                                        scratch,
                                    )
                            }
                            _ => false,
                        })
                } else {
                    false
                }
            }
        };
        scratch.truncate(base);
        ok
    }
}
