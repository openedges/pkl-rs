// Included in rendering.rs so binary traversal shares its private continuation,
// converter paths, source frames and GC roots with the text renderers.
impl Vm {
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
    pub(super) fn begin_pkl_binary_encode_value(&mut self) -> Result<(), RuntimeError> {
        let root = self.pop()?.boxed();
        if let Some(bytes) = self.try_fast_encode_pkl_binary(&root) {
            self.stack.push(Value::Bytes(bytes.into()));
            return Ok(());
        }
        let receiver = self.canonical_builtin_class(classes::BuiltinClass::Any);
        self.frames.last_mut().expect("renderer frame").pcf_render = Some(RenderRun {
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
            converters: Converters::default(),
            source: None,
        });
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
                run.tasks.push(Task::OpenObject {
                    receiver,
                    depth,
                    standalone,
                    root,
                    path,
                    stream: false,
                });
                if !strict {
                    run.await_ = Some(Await::Shallow);
                    self.force_shallow_frame(Value::Object(receiver))?;
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
        mut members: Members,
        descriptor: CollectionMember,
        key: Value,
        value: Value,
        output_name: Option<PklString>,
        previous_source: Option<RuntimeFrame>,
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

    /// Direct zero-task `pkl-binary` encoder for already-forced acyclic scalar / strict-List /
    /// plain-Dynamic / top-level-Module values. Returns `None` whenever any member needs
    /// evaluation, inheritance traversal, cycle diagnostics, or special collection handling,
    /// falling back to the full resumable `RenderRun` with zero side effects.
    fn try_fast_encode_pkl_binary(&self, root: &Value) -> Option<Vec<u8>> {
        let mut wire = super::pkl_binary_wire::BinaryWire::with_capacity(4096);
        let mut active = Vec::with_capacity(8);
        let mut scratch: Vec<(&PklString, &Value)> = Vec::with_capacity(16);
        if self.try_fast_encode_binary_value(root, &mut wire, &mut active, &mut scratch) {
            Some(wire.into_bytes())
        } else {
            None
        }
    }

    fn try_fast_encode_binary_value<'a>(
        &'a self,
        value: &'a Value,
        wire: &mut super::pkl_binary_wire::BinaryWire,
        active: &mut Vec<ObjectId>,
        scratch: &mut Vec<(&'a PklString, &'a Value)>,
    ) -> bool {
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
                if active.len() >= 32 || active.contains(id) {
                    return false;
                }
                let Some(obj) = self.heap.get_fast(*id) else {
                    return false;
                };
                if obj.pair.is_some()
                    || obj.reference.is_some()
                    || obj.foreign_map_entry.is_some()
                    || obj.function.is_some()
                    || obj.native_function.is_some()
                {
                    return false;
                }
                if obj.collection_kind == CollectionKind::List && obj.parent.is_none() {
                    let Some(elems) = &obj.list_elements else {
                        return false;
                    };
                    if wire.array(2).is_err()
                        || wire.integer(0x04).is_err()
                        || wire.array(elems.len()).is_err()
                    {
                        return false;
                    }
                    active.push(*id);
                    for elem in elems {
                        if !self.try_fast_encode_binary_value(elem, wire, active, scratch) {
                            active.pop();
                            return false;
                        }
                    }
                    active.pop();
                    return true;
                }
                if obj.collection_kind != CollectionKind::Dynamic
                    || !obj.member_annotations.is_empty()
                    || obj
                        .collection_members
                        .iter()
                        .any(|cm| cm.kind != CollectionMemberKind::Property)
                {
                    return false;
                }
                // Case 1: plain un-amended `new Dynamic { prop = val; ... }`
                let instance_class_is_none_or_dynamic = match obj.instance_class {
                    None => true,
                    Some(cls) => self
                        .heap
                        .get_class(cls)
                        .ok()
                        .and_then(|c| self.heap.get_fast(c.prototype))
                        .and_then(|p| p.builtin_type.as_ref())
                        .and_then(classes::BuiltinClass::from_type)
                        == Some(classes::BuiltinClass::Dynamic),
                };
                let parent_is_empty_dynamic = {
                    let mut cur = obj.parent;
                    let mut hops = 0u8;
                    let mut ok = true;
                    while let Some(p) = cur {
                        hops += 1;
                        if hops > 4 {
                            ok = false;
                            break;
                        }
                        let Some(pobj) = self.heap.get_fast(p) else {
                            ok = false;
                            break;
                        };
                        if pobj.collection_kind != CollectionKind::Dynamic
                            || pobj.is_module
                            || !pobj.collection_members.is_empty()
                        {
                            ok = false;
                            break;
                        }
                        if pobj
                            .builtin_type
                            .as_ref()
                            .and_then(classes::BuiltinClass::from_type)
                            == Some(classes::BuiltinClass::Dynamic)
                        {
                            break;
                        }
                        if pobj.instance_class.is_some() || !pobj.members.is_empty() {
                            ok = false;
                            break;
                        }
                        cur = pobj.parent;
                    }
                    ok
                };
                if !obj.is_module && instance_class_is_none_or_dynamic && parent_is_empty_dynamic {
                    if obj.members.iter().any(|m| {
                        m.entry
                            || m.hidden()
                            || (!m.local
                                && !m.method
                                && !m.definition()
                                && !m.import()
                                && !m.external()
                                && m.name == "default")
                    }) {
                        return false;
                    }
                    let base = scratch.len();
                    if !obj.collection_members.is_empty() {
                        let slots_aligned = obj.cache_slots.len() == obj.collection_members.len();
                        for (idx, cm) in obj.collection_members.iter().enumerate() {
                            if cm.name == "default" {
                                scratch.truncate(base);
                                return false;
                            }
                            if scratch[base..].iter().any(|(prev, _)| *prev == &cm.name) {
                                scratch.truncate(base);
                                return false;
                            }
                            let val = if slots_aligned
                                && !obj.cache_slots[idx].key.entry
                                && !obj.cache_slots[idx].key.method
                                && obj.cache_slots[idx].key.owner.is_none()
                                && obj.cache_slots[idx].key.name == cm.name
                            {
                                &obj.cache_slots[idx].value
                            } else if let Some(val) =
                                super::fast_lookup_cached(obj, None, &cm.name, false)
                            {
                                val
                            } else {
                                scratch.truncate(base);
                                return false;
                            };
                            scratch.push((&cm.name, val));
                        }
                    } else {
                        let slots_aligned = obj.cache_slots.len() == obj.members.len();
                        for (idx, m) in obj.members.iter().enumerate() {
                            if m.local
                                || m.method
                                || m.entry
                                || m.definition()
                                || m.import()
                                || m.external()
                                || m.hidden()
                            {
                                continue;
                            }
                            if scratch[base..].iter().any(|(prev, _)| *prev == &m.name) {
                                scratch.truncate(base);
                                return false;
                            }
                            let val = if slots_aligned
                                && !obj.cache_slots[idx].key.entry
                                && !obj.cache_slots[idx].key.method
                                && obj.cache_slots[idx].key.owner.is_none()
                                && obj.cache_slots[idx].key.name == m.name
                            {
                                &obj.cache_slots[idx].value
                            } else if let Some(val) =
                                super::fast_lookup_cached(obj, None, &m.name, false)
                            {
                                val
                            } else {
                                scratch.truncate(base);
                                return false;
                            };
                            scratch.push((&m.name, val));
                        }
                    }
                    let count = scratch.len() - base;
                    if let Some(expected_count) = obj.dynamic_regular_member_count
                        && expected_count != count
                    {
                        scratch.truncate(base);
                        return false;
                    }
                    if wire.array(4).is_err()
                        || wire.integer(0x01).is_err()
                        || wire.str_bytes("Dynamic").is_err()
                        || wire.str_bytes("pkl:base").is_err()
                        || wire.array(count).is_err()
                    {
                        scratch.truncate(base);
                        return false;
                    }
                    active.push(*id);
                    let end = base + count;
                    for i in base..end {
                        let (name, val) = scratch[i];
                        if wire.array(3).is_err()
                            || wire.integer(0x10).is_err()
                            || wire.string(name).is_err()
                            || !self.try_fast_encode_binary_value(val, wire, active, scratch)
                        {
                            active.pop();
                            scratch.truncate(base);
                            return false;
                        }
                    }
                    active.pop();
                    scratch.truncate(base);
                    return true;
                }
                // Case 2: plain top-level module whose parent is None, `pkl.base#Module`
                // prototype, or `pkl:base`
                if obj.is_module && self.is_render_directive(*id).ok() == Some(false) {
                    let parent_is_none_or_base = match obj.parent {
                        None => true,
                        Some(p) => self.heap.get_fast(p).is_some_and(|pobj| {
                            pobj.builtin_type
                                .as_ref()
                                .and_then(classes::BuiltinClass::from_type)
                                == Some(classes::BuiltinClass::Ordinary(output::MODULE))
                                || pobj
                                    .module_unit
                                    .and_then(|u| self.program.units.get(u.0 as usize))
                                    .and_then(|u| u.identity.as_ref())
                                    .is_some_and(|ident| ident.original_uri == "pkl:base")
                        }),
                    };
                    if !parent_is_none_or_base {
                        return false;
                    }
                    let Ok(Some(class)) = self.heap.object_class(*id) else {
                        return false;
                    };
                    let Ok((class_name, class_uri)) = self.binary_declared_class_identity(class)
                    else {
                        return false;
                    };
                    let base = scratch.len();
                    if !obj.collection_members.is_empty() {
                        for cm in &obj.collection_members {
                            match collections::hidden_property(&self.heap, *id, &cm.name, false) {
                                Ok(true) => continue,
                                Ok(false) => {}
                                Err(_) => {
                                    scratch.truncate(base);
                                    return false;
                                }
                            }
                            if scratch[base..].iter().any(|(prev, _)| *prev == &cm.name) {
                                scratch.truncate(base);
                                return false;
                            }
                            let Some(val) = super::fast_lookup_cached(obj, None, &cm.name, false)
                            else {
                                scratch.truncate(base);
                                return false;
                            };
                            scratch.push((&cm.name, val));
                        }
                    } else {
                        for m in &obj.members {
                            if m.local
                                || m.method
                                || m.entry
                                || m.definition()
                                || m.import()
                                || m.external()
                                || m.hidden()
                            {
                                continue;
                            }
                            match collections::hidden_property(&self.heap, *id, &m.name, false) {
                                Ok(true) => continue,
                                Ok(false) => {}
                                Err(_) => {
                                    scratch.truncate(base);
                                    return false;
                                }
                            }
                            if scratch[base..].iter().any(|(prev, _)| *prev == &m.name) {
                                scratch.truncate(base);
                                return false;
                            }
                            let Some(val) = super::fast_lookup_cached(obj, None, &m.name, false)
                            else {
                                scratch.truncate(base);
                                return false;
                            };
                            scratch.push((&m.name, val));
                        }
                    }
                    let count = scratch.len() - base;
                    if self.binary_regular_property_count(*id).ok() != Some(count) {
                        scratch.truncate(base);
                        return false;
                    }
                    if wire.array(4).is_err()
                        || wire.integer(0x01).is_err()
                        || wire.string(&class_name).is_err()
                        || wire.string(&class_uri).is_err()
                        || wire.array(count).is_err()
                    {
                        scratch.truncate(base);
                        return false;
                    }
                    active.push(*id);
                    let end = base + count;
                    for i in base..end {
                        let (name, val) = scratch[i];
                        if wire.array(3).is_err()
                            || wire.integer(0x10).is_err()
                            || wire.string(name).is_err()
                            || !self.try_fast_encode_binary_value(val, wire, active, scratch)
                        {
                            active.pop();
                            scratch.truncate(base);
                            return false;
                        }
                    }
                    active.pop();
                    scratch.truncate(base);
                    return true;
                }
                false
            }
        }
    }
}
