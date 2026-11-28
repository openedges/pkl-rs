// Included in rendering.rs so XML uses the same converter, forcing and source state.
#[derive(Clone)]
enum XmlPlace {
    Top,
    Visit,
    Property {
        key: Value,
        enclosing: ObjectId,
        validate: bool,
    },
    Element,
}
enum XmlAction {
    Dispatch {
        place: XmlPlace,
        marker_read: bool,
    },
    Explicit {
        stage: usize,
        values: Vec<Value>,
    },
    Wrapper {
        kind: u8,
        stage: usize,
    },
    Key {
        enclosing: ObjectId,
    },
    Raw {
        hint: bool,
    },
    Write {
        name: PklString,
        attributes: Option<ObjectId>,
        block: bool,
        validate: bool,
        stage: u8,
        explicit: bool,
    },
    Attributes {
        receiver: ObjectId,
        name: PklString,
    },
    Close {
        name: PklString,
        line: usize,
        release: Option<ObjectId>,
    },
    Attribute {
        name: PklString,
        key: Value,
    },
    Elements {
        receiver: ObjectId,
    },
    CheckedObject {
        marker_read: bool,
    },
}
struct XmlTask {
    value: Value,
    depth: usize,
    path: Vec<PathPart>,
    action: XmlAction,
    input: Option<Value>,
}
impl XmlTask {
    fn roots(&self, roots: &mut Vec<Value>) {
        roots.push(self.value.clone());
        roots.extend(self.input.iter().cloned());
        path_roots(&self.path, roots);
        match &self.action {
            XmlAction::Dispatch {
                place: XmlPlace::Property { key, enclosing, .. },
                ..
            } => {
                roots.push(key.clone());
                roots.push(Value::Object(*enclosing));
            }
            XmlAction::Explicit { values, .. } => roots.extend(values.iter().cloned()),
            XmlAction::Write { attributes, .. } => {
                roots.extend(attributes.iter().copied().map(Value::Object))
            }
            XmlAction::Attributes { receiver, .. } | XmlAction::Elements { receiver } => {
                roots.push(Value::Object(*receiver))
            }
            XmlAction::Key { enclosing } => roots.push(Value::Object(*enclosing)),
            XmlAction::Attribute { key, .. } => roots.push(key.clone()),
            XmlAction::Close { release, .. } => {
                roots.extend(release.iter().copied().map(Value::Object))
            }
            _ => (),
        }
    }
}
impl RenderRun {
    fn xml_dispatch(&mut self, value: Value, depth: usize, path: Vec<PathPart>, place: XmlPlace) {
        self.tasks.push(Task::Xml(Box::new(XmlTask {
            value,
            depth,
            path,
            action: XmlAction::Dispatch {
                place,
                marker_read: false,
            },
            input: None,
        })));
    }
    fn xml_newline(&mut self, depth: usize) {
        if !self.output_empty() {
            self.xml_line += 1;
            self.append("\n");
            self.append_value(&self.indentation(depth));
        }
    }
    fn xml_validate(&self, name: &PklString, kind: &str) -> Result<(), RuntimeError> {
        let Value::String(version) = &self.options[1] else {
            return Err(invalid("XML version"));
        };
        if super::xml::valid_name(name, version == "1.1") {
            return Ok(());
        }
        Err(RuntimeError::new(
            ErrorKind::Type,
            PklString::join(
                &[
                    "Invalid XML ".into(),
                    version.clone(),
                    format!(" {kind} name: `").into(),
                    name.clone(),
                    "`".into(),
                ],
                "",
            ),
        ))
    }
}
impl Vm {
    #[cfg_attr(target_arch = "wasm32", inline(never))]
    pub(super) fn begin_xml_render(&mut self, document: bool) -> Result<(), RuntimeError> {
        let root = self
            .frames
            .last()
            .and_then(|f| f.environment.locals.last())
            .cloned()
            .ok_or_else(|| invalid("XML argument"))?;
        self.stack.push(root);
        self.begin_render(document, Format::Xml)
    }
    fn xml_wrapper(&self, id: ObjectId) -> Result<Option<u8>, RuntimeError> {
        let Some(actual) = self.heap.object_class(id)? else {
            return Ok(None);
        };
        let Some(modules::CachedModule::Value(module)) = self.modules_stdlib.get("pkl:xml") else {
            return Err(invalid("XML canonical module"));
        };
        for (name, kind) in [("Inline", 0), ("Comment", 1), ("CData", 2)] {
            let key = CacheKey {
                owner: None,
                entry: false,
                method: false,
                name: name.into(),
            };
            let Some(Value::Class(class)) = self.heap.get(*module)?.cache_get_completed(&key)
            else {
                return Err(invalid("XML canonical class"));
            };
            if *class == actual {
                return Ok(Some(kind));
            }
        }
        Ok(None)
    }
    fn xml_dynamic(&self, value: &Value) -> Result<Option<ObjectId>, RuntimeError> {
        if let Value::Object(id) = value
            && self.heap.get(*id)?.collection_kind == CollectionKind::Dynamic
            && !self.heap.get(*id)?.is_module
            // A Dynamic produced by inferred/explicit `new` can retain its
            // canonical built-in class. Nominal class presence does not imply
            // Java VmTyped storage; use the shared representation classifier.
            && !classes::typed_instance(&self.heap, *id)?
            && self.heap.get(*id)?.pair.is_none()
            && self.heap.get(*id)?.reference.is_none()
        {
            return Ok(Some(*id));
        }
        Ok(None)
    }
    fn xml_read(
        &mut self,
        run: &mut RenderRun,
        task: XmlTask,
        receiver: ObjectId,
        name: &str,
        required: bool,
    ) -> Result<bool, RuntimeError> {
        if !required {
            let key = CacheKey {
                owner: None,
                entry: false,
                method: false,
                name: name.into(),
            };
            let mut owner = Some(receiver);
            let mut seen = BTreeSet::new();
            let mut present = self.heap.get(receiver)?.cache_contains_key(&key);
            while let Some(id) = owner {
                if !seen.insert(id) {
                    return Err(RuntimeError::new(ErrorKind::Cycle, "cyclic parent chain"));
                }
                let object = self.heap.get(id)?;
                present |= object
                    .members
                    .iter()
                    .any(|m| !m.local && !m.entry && !m.method && m.name == name);
                owner = object.parent;
            }
            if !present {
                let mut task = task;
                task.input = Some(Value::Null);
                run.tasks.push(Task::Xml(Box::new(task)));
                return Ok(false);
            }
        }
        let hint = matches!(
            &task.action,
            XmlAction::Wrapper { kind: 1 | 2, .. }
                | XmlAction::CheckedObject { .. }
                | XmlAction::Raw { hint: true }
        );
        run.await_ = Some(Await::XmlRead {
            receiver,
            path: task.path.clone(),
            hint,
        });
        run.tasks.push(Task::Xml(Box::new(task)));
        self.force(receiver, receiver, name.into(), false, true, false)?;
        Ok(false)
    }
    fn xml_type_error(&self, value: &Value, expected: &str) -> Result<bool, RuntimeError> {
        if value.is_null() {
            Err(RuntimeError::new(
                ErrorKind::Type,
                format!("Expected value of type {expected}, but got `null`."),
            ))
        } else {
            let headline = PklString::join(
                &[
                    format!("Expected value of type {expected}, but got type `").into(),
                    crate::diagnostic_values::type_name(
                        value,
                        &self.heap,
                        &self.program.module_name,
                    )?,
                    "`.".into(),
                ],
                "",
            );
            let rendered = crate::diagnostic_values::render(
                value,
                &self.heap,
                &self.program.module_name,
                80,
            )?;
            Err(RuntimeError::with_program_values(
                ErrorKind::Type,
                headline,
                &[("Value", rendered)],
            ))
        }
    }
    fn xml_bad_key(&self, receiver: ObjectId, key: &Value) -> Result<bool, RuntimeError> {
        let map = self.heap.get(receiver)?.collection_kind == CollectionKind::Map;
        let (headline, container_label) = if map {
            ("Cannot render map with non-string key as XML.", "Map")
        } else {
            ("Cannot render object with non-string key as XML.", "Object")
        };
        let container_val = crate::diagnostic_values::render(
            &Value::Object(receiver),
            &self.heap,
            &self.program.module_name,
            80,
        )?;
        let key_val = crate::diagnostic_values::render(
            key,
            &self.heap,
            &self.program.module_name,
            80,
        )?;
        Err(RuntimeError::with_program_values(
            ErrorKind::Type,
            headline,
            &[(container_label, container_val), ("Key", key_val)],
        ))
    }
    fn xml_disallowed(&self, value: &Value, kind: &str) -> Result<bool, RuntimeError> {
        let rendered = crate::diagnostic_values::render(
            value,
            &self.heap,
            &self.program.module_name,
            80,
        )?;
        Err(RuntimeError::with_program_values(
            ErrorKind::Type,
            format!("`xml.{kind}` is not supported here."),
            &[("Value", rendered)],
        ))
    }
    fn xml_members(
        &self,
        run: &mut RenderRun,
        receiver: ObjectId,
        depth: usize,
        path: Vec<PathPart>,
        mode: XmlMembers,
    ) -> Result<(), RuntimeError> {
        let members = collections::render_members(&self.heap, receiver)?;
        run.tasks.push(Task::Members(Box::new(Members {
            xml: Some(mode),
            receiver,
            members,
            next: 0,
            seen: Vec::new(),
            seen_buckets: crate::heap::FastMap::default(),
            compare: 0,
            prepared: false,
            hash: None,
            depth,
            document_root: false,
            emitted: false,
            strict: false,
            array: false,
            stream: false,
            path,
        })));
        Ok(())
    }
    fn advance_xml(
        &mut self,
        run: &mut RenderRun,
        mut task: XmlTask,
    ) -> Result<bool, RuntimeError> {
        let XmlTask {
            value,
            depth,
            path,
            action,
            input,
        } = task;
        match action {
            XmlAction::Dispatch { place, marker_read } => {
                // XML's special marker is a lazy property on Dynamic, not a class/name test.
                if !matches!(place, XmlPlace::Visit)
                    && !marker_read
                    && let Some(id) = self.xml_dynamic(&value)?
                {
                    task = XmlTask {
                        value,
                        depth,
                        path,
                        input: None,
                        action: XmlAction::Dispatch {
                            place,
                            marker_read: true,
                        },
                    };
                    return self.xml_read(run, task, id, "_isXmlElement", false);
                }
                let element =
                    matches!(input, Some(Value::Bool(true))) && !matches!(place, XmlPlace::Visit);
                if matches!(place, XmlPlace::Top) && run.document {
                    let Value::String(version) = &run.options[1] else {
                        return Err(invalid("XML version"));
                    };
                    let version = version.clone();
                    run.append("<?xml version=\"");
                    run.append_value(&super::plist::escape(&version));
                    run.append("\" encoding=\"UTF-8\"?>");
                }
                if element {
                    if !run.active.insert(value.as_object()?) {
                        return Err(RuntimeError::new(
                            ErrorKind::Cycle,
                            "cyclic value rendering",
                        ));
                    }
                    run.tasks.push(Task::Xml(Box::new(XmlTask {
                        value,
                        depth,
                        path,
                        input: None,
                        action: XmlAction::Explicit {
                            stage: 0,
                            values: Vec::new(),
                        },
                    })));
                    return Ok(false);
                }
                if matches!(place, XmlPlace::Top) && run.document {
                    let (Value::String(name), Value::Object(attributes)) =
                        (&run.options[2], &run.options[3])
                    else {
                        return Err(invalid("XML root options"));
                    };
                    run.tasks.push(Task::Xml(Box::new(XmlTask {
                        value,
                        depth,
                        path,
                        input: None,
                        action: XmlAction::Write {
                            name: name.clone(),
                            attributes: Some(*attributes),
                            block: true,
                            validate: true,
                            stage: 0,
                            explicit: false,
                        },
                    })));
                    return Ok(false);
                }
                let wrapper = if let Value::Object(id) = &value {
                    self.xml_wrapper(*id)?
                } else {
                    None
                };
                if wrapper == Some(0)
                    && matches!(place, XmlPlace::Property { .. } | XmlPlace::Element)
                {
                    run.tasks.push(Task::Xml(Box::new(XmlTask {
                        value,
                        depth,
                        path,
                        input: None,
                        action: XmlAction::Wrapper { kind: 0, stage: 0 },
                    })));
                    return Ok(false);
                }
                if let XmlPlace::Property {
                    key,
                    enclosing,
                    validate,
                } = place
                {
                    match key {
                        Value::String(name) => run.tasks.push(Task::Xml(Box::new(XmlTask {
                            value,
                            depth,
                            path,
                            input: None,
                            action: XmlAction::Write {
                                name,
                                attributes: None,
                                block: true,
                                validate,
                                stage: 0,
                                explicit: false,
                            },
                        }))),
                        Value::Object(id) if self.is_render_directive(id)? => {
                            task = XmlTask {
                                value,
                                depth,
                                path,
                                input: None,
                                action: XmlAction::Key { enclosing },
                            };
                            return self.xml_read(run, task, id, "text", true);
                        }
                        key => return self.xml_bad_key(enclosing, &key),
                    }
                    return Ok(false);
                }
                if matches!(place, XmlPlace::Element) {
                    if value.is_null() {
                        return Ok(false);
                    }
                    let content = matches!(
                        value,
                        Value::String(_) | Value::Bool(_) | Value::Int(_) | Value::Float(_)
                    ) || matches!(wrapper, Some(1 | 2));
                    let directive =
                        matches!(value,Value::Object(id) if self.is_render_directive(id)?);
                    if !content && !directive {
                        let name = if let Value::Object(id) = &value
                            && let Some(class) = self.heap.object_class(*id)?
                        {
                            self.heap.class_simple_name(class)?
                        } else {
                            crate::diagnostic_values::type_name(
                                &value,
                                &self.heap,
                                &self.program.module_name,
                            )?
                        };
                        run.tasks.push(Task::Xml(Box::new(XmlTask {
                            value,
                            depth,
                            path,
                            input: None,
                            action: XmlAction::Write {
                                name,
                                attributes: None,
                                block: true,
                                validate: true,
                                stage: 0,
                                explicit: false,
                            },
                        })));
                        return Ok(false);
                    }
                }
                match value {
                    Value::String(text) => run.append_value(&super::plist::escape(&text)),
                    Value::Bool(_) | Value::Int(_) | Value::Float(_) => {
                        run.append_value(&scalar_string(&value)?)
                    }
                    Value::Object(id) => {
                        if matches!(wrapper, Some(1 | 2)) {
                            run.tasks.push(Task::Xml(Box::new(XmlTask {
                                value: Value::Object(id),
                                depth,
                                path,
                                input: None,
                                action: XmlAction::Wrapper {
                                    kind: wrapper.unwrap(),
                                    stage: 0,
                                },
                            })));
                            return Ok(false);
                        }
                        if self.is_render_directive(id)? {
                            task = XmlTask {
                                value: Value::Object(id),
                                depth,
                                path,
                                input: None,
                                action: XmlAction::Raw {
                                    hint: !matches!(place, XmlPlace::Element),
                                },
                            };
                            return self.xml_read(run, task, id, "text", true);
                        }
                        if self.heap.get(id)?.pair.is_some()
                            || self.heap.get(id)?.reference.is_some()
                        {
                            return self.pcf_unrenderable(run, &Value::Object(id));
                        }
                        if self.heap.get(id)?.foreign_map_entry.is_some() {
                            return Err(RuntimeError::foreign(
                                crate::java_exception::provenance::ForeignOperation::KeyValueHolderTypeVisitor { is_set: false },
                                "Unknown VM value type: java.util.KeyValueHolder",
                            ));
                        }
                        if !run.active.insert(id) {
                            return Err(RuntimeError::new(
                                ErrorKind::Cycle,
                                "cyclic value rendering",
                            ));
                        }
                        run.tasks.push(Task::Xml(Box::new(XmlTask {
                            value: Value::Object(id),
                            depth,
                            path,
                            input: None,
                            action: XmlAction::CheckedObject { marker_read: false },
                        })));
                        // Strict collections already contain their values. Java's
                        // visitList/Set/Map must not force child objects before converters.
                        if !matches!(
                            self.heap.get(id)?.collection_kind,
                            CollectionKind::List | CollectionKind::Set | CollectionKind::Map
                        ) {
                            run.await_ = Some(Await::Shallow);
                            self.force_shallow_frame(Value::Object(id))?;
                        }
                    }
                    _ => return self.pcf_unrenderable(run, &value),
                }
            }
            XmlAction::CheckedObject { marker_read } => {
                let id = value.as_object()?;
                if self.xml_dynamic(&value)?.is_some() {
                    if !marker_read {
                        task = XmlTask {
                            value,
                            depth,
                            path,
                            input: None,
                            action: XmlAction::CheckedObject { marker_read: true },
                        };
                        return self.xml_read(run, task, id, "_isXmlElement", false);
                    }
                    if matches!(input, Some(Value::Bool(true))) {
                        return self.xml_disallowed(&value, "Element");
                    }
                }
                if self.xml_wrapper(id)? == Some(0) {
                    return self.xml_disallowed(&value, "Inline");
                }
                run.tasks.push(Task::OpenObject {
                    receiver: id,
                    depth,
                    standalone: false,
                    root: false,
                    path,
                    stream: false,
                });
            }
            XmlAction::Explicit {
                mut stage,
                mut values,
            } => {
                let id = value.as_object()?;
                if let Some(input) = input {
                    match stage {
                        0 if !matches!(input, Value::String(_)) => {
                            return self.xml_type_error(&value, "`String`");
                        }
                        1 if !matches!(input,Value::Object(a) if self.heap.get(a)?.collection_kind==CollectionKind::Mapping) =>
                        {
                            return self.xml_type_error(&value, "`Mapping`");
                        }
                        2 if !matches!(input, Value::Bool(_)) => {
                            return self.xml_type_error(&value, "`Boolean`");
                        }
                        _ => (),
                    }
                    values.push(input);
                    stage += 1;
                }
                if stage < 3 {
                    task = XmlTask {
                        value,
                        depth,
                        path,
                        input: None,
                        action: XmlAction::Explicit { stage, values },
                    };
                    return self.xml_read(
                        run,
                        task,
                        id,
                        ["name", "attributes", "isBlockFormat"][stage],
                        true,
                    );
                }
                // Pinned Java reports the Element receiver itself for these structural type errors.
                let Value::String(name) = &values[0] else {
                    return self.xml_type_error(&value, "`String`");
                };
                let Value::Object(attributes) = values[1] else {
                    return self.xml_type_error(&value, "`Mapping`");
                };
                if self.heap.get(attributes)?.collection_kind != CollectionKind::Mapping {
                    return self.xml_type_error(&value, "`Mapping`");
                }
                let Value::Bool(block) = values[2] else {
                    return self.xml_type_error(&value, "`Boolean`");
                };
                run.tasks.push(Task::Xml(Box::new(XmlTask {
                    value,
                    depth,
                    path,
                    input: None,
                    action: XmlAction::Write {
                        name: name.clone(),
                        attributes: Some(attributes),
                        block,
                        validate: true,
                        stage: 0,
                        explicit: true,
                    },
                })));
            }
            XmlAction::Wrapper { kind, stage } => {
                let id = value.as_object()?;
                if stage == 0 {
                    task = XmlTask {
                        value,
                        depth,
                        path,
                        input: None,
                        action: XmlAction::Wrapper { kind, stage: 1 },
                    };
                    return self.xml_read(
                        run,
                        task,
                        id,
                        if kind == 0 {
                            "value"
                        } else if kind == 1 {
                            "isBlockFormat"
                        } else {
                            "text"
                        },
                        true,
                    );
                }
                if kind == 0 {
                    run.xml_dispatch(
                        input.ok_or_else(|| invalid("XML inline result"))?,
                        depth,
                        path,
                        XmlPlace::Visit,
                    );
                } else if kind == 1 && stage == 1 {
                    let Some(Value::Bool(block)) = input else {
                        return Err(invalid("XML comment block"));
                    };
                    if block {
                        run.xml_newline(depth);
                    }
                    task = XmlTask {
                        value,
                        depth,
                        path,
                        input: None,
                        action: XmlAction::Wrapper { kind, stage: 2 },
                    };
                    return self.xml_read(run, task, id, "text", true);
                } else {
                    let Some(Value::String(text)) = input else {
                        return Err(invalid("XML directive text"));
                    };
                    if kind == 2 {
                        run.append_value(&super::xml::cdata(&text));
                    } else {
                        run.append("<!--");
                        run.append_value(&text);
                        run.append("-->");
                    }
                }
            }
            XmlAction::Raw { .. } => {
                let Some(Value::String(text)) = input else {
                    return Err(invalid("XML raw directive text"));
                };
                run.append_value(&text);
            }
            XmlAction::Key { enclosing: _ } => {
                let Some(Value::String(name)) = input else {
                    return Err(invalid("XML directive key"));
                };
                run.tasks.push(Task::Xml(Box::new(XmlTask {
                    value,
                    depth,
                    path,
                    input: None,
                    action: XmlAction::Write {
                        name,
                        attributes: None,
                        block: true,
                        validate: false,
                        stage: 0,
                        explicit: false,
                    },
                })));
            }
            XmlAction::Write {
                name,
                attributes,
                block,
                validate,
                stage,
                explicit,
            } => {
                if stage == 0 {
                    if block {
                        run.xml_newline(depth);
                    }
                    if validate {
                        run.xml_validate(&name, "element")?;
                    }
                    run.append("<");
                    run.append_value(&name);
                    run.tasks.push(Task::Xml(Box::new(XmlTask {
                        value,
                        depth,
                        path: path.clone(),
                        input: None,
                        action: XmlAction::Write {
                            name: name.clone(),
                            attributes,
                            block,
                            validate,
                            stage: 1,
                            explicit,
                        },
                    })));
                    if let Some(receiver) = attributes {
                        run.tasks.push(Task::Xml(Box::new(XmlTask {
                            value: Value::Object(receiver),
                            depth,
                            path,
                            input: None,
                            action: XmlAction::Attributes { receiver, name },
                        })));
                        run.await_ = Some(Await::Shallow);
                        self.force_shallow_frame(Value::Object(receiver))?;
                    }
                } else {
                    run.append(">");
                    run.tasks.push(Task::Xml(Box::new(XmlTask {
                        value: value.clone(),
                        depth,
                        path: path.clone(),
                        input: None,
                        action: XmlAction::Close {
                            name,
                            line: run.xml_line,
                            release: if explicit {
                                Some(value.as_object()?)
                            } else {
                                None
                            },
                        },
                    })));
                    if explicit {
                        let receiver = value.as_object()?;
                        run.tasks.push(Task::Xml(Box::new(XmlTask {
                            value,
                            depth: depth + 1,
                            path,
                            input: None,
                            action: XmlAction::Elements { receiver },
                        })));
                        run.await_ = Some(Await::Shallow);
                        self.force_shallow_frame(Value::Object(receiver))?;
                    } else {
                        run.xml_dispatch(value, depth + 1, path, XmlPlace::Visit);
                    }
                }
            }
            XmlAction::Attributes { receiver, name } => {
                self.xml_members(run, receiver, depth, path, XmlMembers::Attributes { name })?;
            }
            XmlAction::Attribute { name, key } => {
                run.append(" ");
                let Value::String(key) = key else {
                    return self.xml_type_error(&Value::String(name), "`String`");
                };
                run.xml_validate(&key, "attribute")?;
                run.append_value(&key);
                run.append("=\"");
                if !matches!(
                    value,
                    Value::String(_) | Value::Bool(_) | Value::Int(_) | Value::Float(_)
                ) {
                    return self.xml_type_error(&Value::String(name), "`String` or `Boolean`");
                }
                run.append_value(&super::plist::escape(&scalar_string(&value)?));
                run.append("\"");
            }
            XmlAction::Close {
                name,
                line,
                release,
            } => {
                if line < run.xml_line {
                    run.xml_newline(depth);
                }
                run.append("</");
                run.append_value(&name);
                run.append(">");
                if let Some(id) = release {
                    run.active.remove(&id);
                }
            }
            XmlAction::Elements { receiver } => {
                // forceAndIterateMemberValues forces every visible property before
                // discarding non-elements. It does not run PklConverter on children.
                self.xml_members(run, receiver, depth, path, XmlMembers::Elements)?;
            }
        }
        Ok(false)
    }
}
