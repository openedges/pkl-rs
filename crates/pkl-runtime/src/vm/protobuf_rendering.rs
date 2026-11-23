// Included in rendering.rs: all forcing, converters, type checks and pending
// Host work remain in the shared RenderRun. No platform or Java evaluation path.
#[derive(Default)]
struct ProtoState {
    names: Vec<PklString>,
    wrappers: Vec<bool>,
    collection: Option<ProtoType>,
    depth: usize,
    last: Option<u16>,
}
impl ProtoState {
    fn roots(&self, roots: &mut Vec<Value>) {
        if let Some(ty) = &self.collection {
            ty.roots(roots);
        }
    }
    fn wrapper(&self) -> Result<bool, RuntimeError> {
        self.wrapper_for(false)
    }
    fn wrapper_for(&self, is_map: bool) -> Result<bool, RuntimeError> {
        self.wrappers.last().copied().ok_or_else(|| {
            RuntimeError::foreign(
                crate::java_exception::provenance::ForeignOperation::ProtobufEmptyWrapperNullPointer { is_map },
                "Cannot invoke \"java.lang.Boolean.booleanValue()\" because \"result\" is null",
            )
        })
    }
}
struct ProtoField {
    value: Value,
    path: Vec<PathPart>,
    // Some(name) denotes a property; None denotes a collection element.
    name: Option<PklString>,
    ty: Option<ProtoType>,
    phase: usize,
    alternatives: Vec<TypeCheck>,
    next: usize,
    accepted: Option<bool>,
    selected: Option<PklString>,
    previous_collection: Option<ProtoType>,
    pushed_name: bool,
}
enum ProtoTask {
    Field(Box<ProtoField>),
    EndField {
        previous_collection: Option<Box<ProtoType>>,
        name: bool,
        union: bool,
        element: bool,
    },
    Key(Value),
    EndEntry,
}
impl ProtoTask {
    fn roots(&self, roots: &mut Vec<Value>) {
        match self {
            Self::Field(field) => {
                roots.push(field.value.clone());
                path_roots(&field.path, roots);
                if let Some(ty) = &field.ty {
                    ty.roots(roots);
                }
                if let Some(ty) = &field.previous_collection {
                    ty.roots(roots);
                }
            }
            Self::EndField {
                previous_collection: Some(ty),
                ..
            } => ty.roots(roots),
            Self::Key(value) => roots.push(value.clone()),
            Self::EndField { .. } | Self::EndEntry => {}
        }
    }
}
impl RenderRun {
    fn proto_newline(&mut self) {
        if self.indent.is_empty() || self.output.is_empty() || self.protobuf.last == Some(10) {
            return;
        }
        self.append("\n");
        self.append_value(&self.indentation(self.protobuf.depth));
    }
    fn proto_name(&mut self) {
        if let Some(name) = self.protobuf.names.last().cloned() {
            self.proto_newline();
            self.append_value(&name);
            self.append(": ");
        }
    }
    fn proto_start_message(&mut self) {
        self.append("{");
        self.protobuf.depth += 1;
    }
    fn proto_end_message(&mut self) -> Result<(), RuntimeError> {
        self.protobuf.depth = self
            .protobuf
            .depth
            .checked_sub(1)
            .ok_or_else(|| invalid("protobuf message depth"))?;
        if self.protobuf.last != Some(b'{' as u16) {
            self.proto_newline();
        }
        self.append("}");
        Ok(())
    }
}
fn protobuf_escape_name(name: &PklString) -> PklString {
    PklString::from_utf16(
        name.encode_utf16()
            .map(|unit| {
                if unit == b'.' as u16 || unit == b'#' as u16 {
                    b'_' as u16
                } else {
                    unit
                }
            })
            .collect::<Vec<_>>(),
    )
}
impl Vm {
    pub(super) fn begin_protobuf_render(&mut self, document: bool) -> Result<(), RuntimeError> {
        let value = self
            .frames
            .last()
            .and_then(|frame| frame.environment.locals.last())
            .cloned()
            .ok_or_else(|| invalid("protobuf render argument"))?;
        self.stack.push(value);
        self.begin_render(document, Format::Protobuf)
    }
    pub(super) fn protobuf_render_type(&mut self) -> Result<(), RuntimeError> {
        // The authored reflect.Type parameter check is preserved. Java asks for
        // the mirror's runtime class (e.g. ClassType), not its reflectee class.
        let value = self
            .frames
            .last()
            .and_then(|frame| frame.environment.locals.last())
            .cloned()
            .ok_or_else(|| invalid("protobuf renderType argument"))?;
        let name = self.protobuf_value_class_name(&value)?;
        self.stack.push(Value::String(protobuf_escape_name(&name)));
        Ok(())
    }
    fn protobuf_value_class(&mut self, value: &Value) -> Result<ObjectId, RuntimeError> {
        Ok(match classes::runtime_class(value, &self.heap)? {
            classes::RuntimeClass::Declared(class) => class,
            classes::RuntimeClass::Builtin(class) => self.canonical_builtin_class(class),
        })
    }
    fn protobuf_value_class_name(&mut self, value: &Value) -> Result<PklString, RuntimeError> {
        let class = self.protobuf_value_class(value)?;
        self.heap.class_simple_name(class)
    }
    fn protobuf_type_name(&mut self, ty: &TypeCheck) -> Result<PklString, RuntimeError> {
        let Some(class) = self.protobuf_type_class(ty)? else {
            return Err(RuntimeError::new(
                ErrorKind::Unsupported,
                "Failed to resolve type required for rendering protobuf.\n\nConsider adding a type annotation.\nInternal TypeNode diagnostic value projection remains unresolved.",
            ));
        };
        Ok(protobuf_escape_name(&self.heap.class_simple_name(class)?))
    }
    fn protobuf_visit(
        &mut self,
        run: &mut RenderRun,
        value: Value,
        root: bool,
        path: Vec<PathPart>,
    ) -> Result<bool, RuntimeError> {
        if root && !run.document {
            let allowed = matches!(
                &value,
                Value::String(_)
                    | Value::Bool(_)
                    | Value::Int(_)
                    | Value::Float(_)
                    | Value::Duration(_)
            ) || matches!(&value, Value::Object(id) if classes::typed_instance(&self.heap, *id)?);
            if !allowed {
                // Java rejects the host VmFunction before visiting it, using
                // getClass().getSimpleName() rather than its FunctionN VmClass.
                // Nested/document function errors still use the runtime class.
                let name = if matches!(&value, Value::Function(_)) {
                    PklString::from("Function")
                } else {
                    self.protobuf_value_class_name(&value)?
                };
                return Err(RuntimeError::new(ErrorKind::Type, PklString::from("The top-level value of a protobuf file must have type `Typed`, but got type `").concat(&name).concat(&"`.".into())));
            }
            run.protobuf.wrappers.push(false);
        }
        match value {
            Value::String(text) => {
                run.proto_name();
                run.append_value(&json_string(&text));
            }
            Value::Bool(value) => {
                run.proto_name();
                run.append(if value { "true" } else { "false" });
            }
            Value::Int(value) => {
                run.proto_name();
                run.append(&value.to_string());
            }
            Value::Float(value) => {
                run.proto_name();
                run.append(&crate::numeric::double_to_string(*value));
            }
            Value::Duration(value) => {
                use crate::duration::DurationUnit;
                let seconds = value.value_in(DurationUnit::Seconds).floor() as i64;
                let nanos = (value.value_in(DurationUnit::Nanoseconds) as i64) % 1_000_000_000;
                run.proto_name();
                run.proto_start_message();
                for (name, value) in [("seconds", seconds), ("nanos", nanos)] {
                    run.protobuf.names.push(name.into());
                    run.proto_name();
                    run.append(&value.to_string());
                    run.protobuf.names.pop();
                }
                run.proto_end_message()?;
            }
            // These Protobuf visitors build their own VmException without
            // currSourceSection. Inherited Class/TypeAlias/Function visitors
            // still use AbstractRenderer's source-bearing error below.
            Value::DataSize(_) | Value::Bytes(_) => {
                return self.render_unrenderable_at(run, &value, None);
            }
            Value::Null | Value::NullWithDefault(_) | Value::IntSeq(_) | Value::Regex(_) => {
                run.proto_name();
                run.await_ = Some(Await::ProtobufString);
                self.java_value_string(value)?;
            }
            Value::Object(id) => {
                if self.is_render_directive(id)? {
                    run.proto_name();
                    run.await_ = Some(Await::Directive { receiver: id, path });
                    self.force(id, id, "text".into(), false, true, false)?;
                    return Ok(false);
                }
                let object = self.heap.get(id)?;
                if object.pair.is_some() || object.reference.is_some() {
                    run.proto_name();
                    run.await_ = Some(Await::ProtobufString);
                    self.java_value_string(Value::Object(id))?;
                    return Ok(false);
                }
                if object.foreign_map_entry.is_some() {
                    return self.pcf_unrenderable(run, &Value::Object(id));
                }
                if !run.active.insert(id) {
                    return Err(RuntimeError::new(
                        ErrorKind::Cycle,
                        "Cannot render a cyclic value.",
                    ));
                }
                let strict = matches!(
                    object.collection_kind,
                    CollectionKind::List | CollectionKind::Set | CollectionKind::Map
                );
                run.tasks.push(Task::OpenObject {
                    receiver: id,
                    depth: 0,
                    standalone: false,
                    root,
                    path,
                    stream: false,
                });
                if !strict {
                    run.await_ = Some(Await::Shallow);
                    self.force_shallow_frame(Value::Object(id))?;
                }
            }
            value => return self.pcf_unrenderable(run, &value),
        }
        Ok(false)
    }
    fn protobuf_start(&self, run: &mut RenderRun, receiver: ObjectId) -> Result<(), RuntimeError> {
        let kind = self.heap.get(receiver)?.collection_kind;
        if classes::typed_instance(&self.heap, receiver)? {
            run.protobuf.wrappers.push(false);
            if !run.protobuf.names.is_empty() {
                run.proto_name();
                run.proto_start_message();
            }
        } else {
            match kind {
                CollectionKind::Dynamic => {
                    // Protobuf.startDynamic also omits currSourceSection.
                    return self
                        .render_unrenderable_at(run, &Value::Object(receiver), None)
                        .map(|_| ());
                }
                CollectionKind::Listing | CollectionKind::List | CollectionKind::Set => {
                    if run.protobuf.wrapper_for(false)? {
                        run.proto_name();
                        run.proto_start_message();
                    }
                }
                CollectionKind::Mapping | CollectionKind::Map => {
                    if run.protobuf.wrapper_for(true)? {
                        run.proto_name();
                        run.protobuf.names.push("it".into());
                        run.proto_start_message();
                    }
                    run.protobuf.wrappers.push(true);
                }
            }
        }
        Ok(())
    }
    fn protobuf_close(&self, run: &mut RenderRun, receiver: ObjectId) -> Result<(), RuntimeError> {
        if classes::typed_instance(&self.heap, receiver)? {
            run.protobuf.wrappers.pop();
            if !run.protobuf.names.is_empty() {
                run.proto_end_message()?;
            }
        } else {
            let kind = self.heap.get(receiver)?.collection_kind;
            if matches!(kind, CollectionKind::Map | CollectionKind::Mapping) {
                run.protobuf.wrappers.pop();
                if run.protobuf.wrapper_for(true)? {
                    run.proto_end_message()?;
                    run.protobuf.names.pop();
                }
            } else if run.protobuf.wrapper_for(false)? {
                run.proto_end_message()?;
            }
        }
        Ok(())
    }
    #[allow(clippy::too_many_arguments)]
    fn protobuf_member(
        &mut self,
        run: &mut RenderRun,
        members: Members,
        descriptor: CollectionMember,
        key: Value,
        value: Value,
        output_name: Option<PklString>,
        source: Option<RuntimeFrame>,
        path: Vec<PathPart>,
    ) -> Result<(), RuntimeError> {
        let receiver = members.receiver;
        let (name, ty) = if descriptor.kind == CollectionMemberKind::Property {
            let name = output_name.unwrap_or_else(|| match descriptor.key {
                Value::String(name) => name,
                _ => unreachable!("property name"),
            });
            let ty = self.protobuf_property_type(receiver, &name)?;
            (Some(name), ty)
        } else {
            (None, run.protobuf.collection.clone())
        };
        run.tasks.push(Task::Members(members));
        run.tasks.push(Task::RestoreSource(source));
        if descriptor.kind == CollectionMemberKind::Entry {
            run.tasks.push(Task::Protobuf(ProtoTask::EndEntry));
            run.value(value, 0, false, false, false, path);
            run.tasks.push(Task::Protobuf(ProtoTask::Key(key)));
        } else {
            run.tasks
                .push(Task::Protobuf(ProtoTask::Field(Box::new(ProtoField {
                    value,
                    path,
                    name,
                    ty,
                    phase: 0,
                    alternatives: Vec::new(),
                    next: 0,
                    accepted: None,
                    selected: None,
                    previous_collection: run.protobuf.collection.clone(),
                    pushed_name: false,
                }))));
        }
        Ok(())
    }
    fn advance_protobuf(
        &mut self,
        run: &mut RenderRun,
        task: ProtoTask,
    ) -> Result<bool, RuntimeError> {
        match task {
            ProtoTask::Key(key) => {
                let directive = matches!(&key, Value::Object(id) if self.is_render_directive(*id)?);
                if !directive && !matches!(&key, Value::String(_) | Value::Int(_) | Value::Bool(_))
                {
                    let rendered = crate::diagnostic_values::render(
                        &key,
                        &self.heap,
                        &self.program.module_name,
                        80,
                    )?;
                    return Err(RuntimeError::with_program_values(
                        ErrorKind::Type,
                        "Cannot render map with a non-scalar, floating point or byte array key.",
                        &[("Key", rendered)],
                    ));
                }
                run.proto_name();
                run.proto_start_message();
                run.proto_newline();
                run.append("key: ");
                run.protobuf.names.push("value".into());
                match key {
                    Value::String(text) => run.append_value(&json_string(&text)),
                    Value::Object(id) => {
                        run.await_ = Some(Await::Directive {
                            receiver: id,
                            path: Vec::new(),
                        });
                        self.force(id, id, "text".into(), false, true, false)?;
                    }
                    key => run.append_value(&scalar_string(&key)?),
                }
            }
            ProtoTask::EndEntry => {
                run.protobuf.names.pop();
                run.proto_end_message()?;
            }
            ProtoTask::EndField {
                previous_collection,
                name,
                union,
                element,
            } => {
                if element {
                    run.protobuf.wrappers.pop();
                }
                if union {
                    run.protobuf.names.pop();
                    run.proto_end_message()?;
                }
                if name {
                    run.protobuf.names.pop();
                }
                run.protobuf.collection = previous_collection.map(|ty| *ty);
            }
            ProtoTask::Field(mut field) => {
                let element = field.name.is_none();
                if field.phase == 0 {
                    if let Some(name) = &field.name {
                        run.protobuf.names.push(name.clone());
                        field.pushed_name = true;
                    } else if run.protobuf.wrapper()? {
                        run.protobuf.names.push("it".into());
                        field.pushed_name = true;
                    }
                    field.phase = 1;
                }
                if field.phase == 1 {
                    if !element && let Some(context) = &mut field.ty {
                        let env = context.environment.clone();
                        let Some(resolved) = self.resolve_runtime_type(&context.ty, &env, false)?
                        else {
                            run.tasks.push(Task::Protobuf(ProtoTask::Field(field)));
                            return Ok(false);
                        };
                        let Some(normalized) =
                            self.protobuf_normalize_type(&resolved, &env, run.protobuf.wrapper()?)?
                        else {
                            run.tasks.push(Task::Protobuf(ProtoTask::Field(field)));
                            return Ok(false);
                        };
                        self.protobuf_retain_type(&resolved, &mut context.retained);
                        if let Some(ty) = &normalized {
                            self.protobuf_retain_type(ty, &mut context.retained);
                        }
                        if let Some(ty) = normalized {
                            context.ty = ty;
                        } else {
                            field.ty = None;
                        }
                    }
                    if let Some(context) = &field.ty {
                        Self::protobuf_alternatives(&context.ty, &mut field.alternatives);
                    }
                    field.phase = 2;
                }
                if field.phase == 2 {
                    if field.accepted.take() == Some(true) {
                        field.selected =
                            Some(self.protobuf_type_name(&field.alternatives[field.next - 1])?);
                        field.phase = 3;
                    }
                    if field.phase == 2 && field.next < field.alternatives.len() {
                        let ty = field.alternatives[field.next].clone();
                        field.next += 1;
                        if let Some(expected) = self.protobuf_open_class(&ty)? {
                            let actual = self.protobuf_value_class(&field.value)?;
                            if expected == actual {
                                // The NonFinalClass exact-identity fast path omits
                                // dot/hash escaping, unlike renderType(type).
                                field.selected = Some(self.heap.class_simple_name(expected)?);
                                field.phase = 3;
                            }
                        }
                        if field.phase == 2 {
                            let context = field
                                .ty
                                .clone()
                                .ok_or_else(|| invalid("protobuf alternative context"))?;
                            let value = field.value.clone();
                            run.tasks.push(Task::Protobuf(ProtoTask::Field(field)));
                            run.await_ = Some(Await::ProtobufAlternative);
                            self.protobuf_begin_alternative(ty, value, &context)?;
                            return Ok(false);
                        }
                    }
                    field.phase = 3;
                }
                let union = field.selected.is_some();
                if let Some(name) = field.selected {
                    run.proto_name();
                    if element {
                        run.proto_start_message();
                    }
                    run.protobuf
                        .names
                        .push(PklString::from("it_").concat(&name));
                    if !element {
                        run.proto_start_message();
                    }
                } else if !element && let Some(context) = &field.ty {
                    match context.ty.resolved() {
                        TypeCheck::ParameterizedCollection { kind, arguments } => {
                            let index = usize::from(matches!(
                                kind,
                                CollectionKind::Map | CollectionKind::Mapping
                            ));
                            if let Some((ty, _)) = arguments.get(index) {
                                run.protobuf.collection = Some(context.with_type(ty.clone()));
                            }
                        }
                        TypeCheck::Class { .. } => {
                            if let Some(expected) = self.protobuf_open_class(&context.ty)? {
                                let actual = self.protobuf_value_class(&field.value)?;
                                if expected != actual {
                                    let expected = self.heap.class_display_name(expected)?;
                                    let actual = self.heap.class_display_name(actual)?;
                                    return Err(RuntimeError::new(
                                        ErrorKind::Type,
                                        PklString::from("Cannot render subtype ")
                                            .concat(&expected)
                                            .concat(&" of specified type ".into())
                                            .concat(&actual)
                                            .concat(&" in protobuf.".into()),
                                    ));
                                }
                            }
                        }
                        _ => {}
                    }
                }
                if element {
                    run.protobuf.wrappers.push(true);
                }
                run.tasks.push(Task::Protobuf(ProtoTask::EndField {
                    previous_collection: field.previous_collection.map(Box::new),
                    name: field.pushed_name,
                    union,
                    element,
                }));
                run.value(field.value, 0, false, false, false, field.path);
            }
        }
        Ok(false)
    }
}
