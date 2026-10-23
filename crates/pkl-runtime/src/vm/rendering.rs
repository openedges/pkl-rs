//! PCF output is an evaluator-owned continuation, separate from diagnostic rendering.
use super::converters::{Converters, PathPart};
use super::*;
use crate::heap::{CollectionKind, CollectionMember, CollectionMemberKind};

struct Members {
    path: Vec<PathPart>,
    receiver: ObjectId,
    members: Vec<collections::RenderMember>,
    next: usize,
    seen: Vec<(CollectionMember, Option<i32>)>,
    compare: usize,
    prepared: bool,
    hash: Option<i32>,
    depth: usize,
    document_root: bool,
    emitted: bool,
    strict: bool,
}
struct MemberRun {
    members: Members,
    descriptor: CollectionMember,
    key: Value,
    value: Value,
    step: usize,
    output_name: Option<PklString>,
    previous_source: Option<RuntimeFrame>,
}
struct PropertyRun {
    member: MemberRun,
    annotations: Vec<ObjectId>,
    next: usize,
    annotation: Value,
    property: Value,
    function: Option<Value>,
    step: usize,
}
enum Task {
    Visit {
        value: Value,
        depth: usize,
        standalone: bool,
        root: bool,
        string_element: bool,
        path: Vec<PathPart>,
        converted: bool,
    },
    OpenObject {
        receiver: ObjectId,
        depth: usize,
        standalone: bool,
        root: bool,
        path: Vec<PathPart>,
    },
    Member(MemberRun),
    Property(PropertyRun),
    Members(Members),
    Text(PklString),
    RestoreSource(Option<RuntimeFrame>),
    Close {
        receiver: ObjectId,
        depth: usize,
        document_root: bool,
        emitted: bool,
        strict: bool,
    },
}
enum Await {
    Option(usize),
    Mapping,
    Shallow,
    KeyEquality,
    ConverterVisit,
    PropertyTransformer,
    PropertyFunction,
    PropertyResult,
    ConverterMember { key: bool },
}
pub(super) struct RenderRun {
    receiver: ObjectId,
    root: Value,
    document: bool,
    option: usize,
    options: Vec<Value>,
    await_: Option<Await>,
    tasks: Vec<Task>,
    active: BTreeSet<ObjectId>,
    output: crate::pkl_string::PklStringBuilder,
    indent: PklString,
    omit_null: bool,
    custom: bool,
    started: bool,
    converters: Converters,
    source: Option<RuntimeFrame>,
}
impl RenderRun {
    pub(super) fn roots(&self, roots: &mut Vec<Value>) {
        roots.push(Value::Object(self.receiver));
        roots.push(self.root.clone());
        roots.extend(self.options.iter().cloned());
        roots.extend(self.active.iter().copied().map(Value::Object));
        self.converters.roots(roots);
        for task in &self.tasks {
            match task {
                Task::Visit { value, path, .. } => {
                    roots.push(value.clone());
                    path_roots(path, roots);
                }
                Task::OpenObject { receiver, path, .. } => {
                    roots.push(Value::Object(*receiver));
                    path_roots(path, roots);
                }
                Task::Close { receiver, .. } => roots.push(Value::Object(*receiver)),
                Task::Members(m) => {
                    path_roots(&m.path, roots);
                    roots.push(Value::Object(m.receiver));
                    roots.extend(m.members.iter().map(|m| m.descriptor.key.clone()));
                    roots.extend(m.seen.iter().map(|(m, _)| m.key.clone()));
                }
                Task::Member(m) => {
                    path_roots(&m.members.path, roots);
                    roots.extend(m.members.seen.iter().map(|(m, _)| m.key.clone()));
                    roots.extend([
                        Value::Object(m.members.receiver),
                        m.key.clone(),
                        m.value.clone(),
                    ]);
                    roots.extend(m.members.members.iter().map(|m| m.descriptor.key.clone()));
                }
                Task::Property(p) => {
                    roots.extend(p.annotations.iter().copied().map(Value::Object));
                    roots.extend([
                        p.annotation.clone(),
                        p.property.clone(),
                        Value::Object(p.member.members.receiver),
                        p.member.key.clone(),
                        p.member.value.clone(),
                    ]);
                    roots.extend(p.function.iter().cloned());
                    path_roots(&p.member.members.path, roots);
                    roots.extend(
                        p.member
                            .members
                            .members
                            .iter()
                            .map(|m| m.descriptor.key.clone()),
                    );
                    roots.extend(p.member.members.seen.iter().map(|(m, _)| m.key.clone()));
                }
                Task::Text(_) | Task::RestoreSource(_) => {}
            }
        }
    }
    fn indentation(&self, depth: usize) -> PklString {
        let mut text = crate::pkl_string::PklStringBuilder::default();
        for _ in 0..depth {
            text.push_value(&self.indent);
        }
        text.finish()
    }
    fn append(&mut self, text: &str) {
        self.output.push_str(text);
    }
    fn value(
        &mut self,
        value: Value,
        depth: usize,
        standalone: bool,
        root: bool,
        string_element: bool,
        path: Vec<PathPart>,
    ) {
        self.tasks.push(Task::Visit {
            value,
            depth,
            standalone,
            root,
            string_element,
            path,
            converted: true,
        });
    }
}
impl Vm {
    pub(super) fn pcf_undefined_hint(&self) -> Option<PklString> {
        let force = self
            .frames
            .iter()
            .rev()
            .find_map(|frame| frame.force.as_ref())?;
        for frame in self.frames.iter().rev() {
            let Some(run) = &frame.pcf_render else {
                continue;
            };
            if !matches!(run.await_, Some(Await::Shallow)) {
                continue;
            }
            let Some(Task::OpenObject { receiver, path, .. }) = run.tasks.last() else {
                continue;
            };
            if *receiver != force.receiver {
                continue;
            }
            let mut path = path.clone();
            if force.entry {
                let object = self.heap.get(force.owner).ok()?;
                let descriptor = object
                    .collection_members
                    .iter()
                    .find(|member| member.name == force.name)?;
                path.push(PathPart::Entry(descriptor.key.clone()));
            } else {
                path.push(PathPart::Property(force.name.as_ref().into()));
            }
            let mut text = crate::pkl_string::PklStringBuilder::default();
            let mut trailing = false;
            for part in path {
                match part {
                    PathPart::Root => {}
                    PathPart::Property(name) => {
                        if trailing {
                            text.push('.');
                        }
                        trailing = true;
                        if name
                            .as_str()
                            .is_some_and(crate::parser::is_regular_identifier)
                        {
                            text.push_value(&name);
                        } else {
                            text.push('`');
                            text.push_value(&name);
                            text.push('`');
                        }
                    }
                    PathPart::Entry(value) => {
                        text.push('[');
                        match value {
                            Value::String(value) => text.push_value(&value),
                            value => text.push_value(
                                &crate::diagnostic_values::render_string(
                                    &value,
                                    &self.heap,
                                    &self.program.module_name,
                                    usize::MAX,
                                )
                                .ok()?,
                            ),
                        }
                        text.push(']');
                    }
                }
            }
            let mut hint = crate::pkl_string::PklStringBuilder::default();
            hint.push_str("The above error occurred when rendering path `");
            hint.push_value(&text.finish());
            hint.push('`');
            if matches!(&run.root, Value::Object(id) if self.heap.get(*id).is_ok_and(|object| object.is_module))
            {
                let source = self.context.primary_source.as_ref()?;
                hint.push_str(&format!(" of module `{}`", source.uri));
            }
            hint.push('.');
            return Some(hint.finish());
        }
        None
    }
    pub(super) fn begin_pcf(&mut self, document: bool) -> Result<(), RuntimeError> {
        let root = self.pop()?;
        let receiver = self.scope(0)?.receiver;
        self.frames.last_mut().expect("renderer frame").pcf_render = Some(RenderRun {
            receiver,
            root,
            document,
            option: 0,
            options: Vec::new(),
            await_: None,
            tasks: Vec::new(),
            active: BTreeSet::new(),
            output: Default::default(),
            indent: Default::default(),
            omit_null: false,
            custom: false,
            started: false,
            converters: Converters::default(),
            source: None,
        });
        Ok(())
    }
    pub(super) fn step_pcf(&mut self) -> Result<(), RuntimeError> {
        let index = self.frames.len() - 1;
        let mut run = self.frames[index]
            .pcf_render
            .take()
            .expect("renderer continuation");
        let result = self.advance_pcf(&mut run);
        match result {
            Ok(true) => {
                self.stack.push(Value::String(run.output.finish()));
            }
            Ok(false) | Err(_) => self.frames[index].pcf_render = Some(run),
        }
        result.map(|_| ())
    }
    fn advance_pcf(&mut self, run: &mut RenderRun) -> Result<bool, RuntimeError> {
        if let Some(await_) = run.await_.take() {
            match await_ {
                Await::Option(index) => {
                    let value = self.pop()?;
                    assert_eq!(index, run.options.len());
                    run.options.push(value);
                }
                Await::Mapping => {
                    self.pop()?;
                }
                Await::PropertyTransformer | Await::PropertyFunction | Await::PropertyResult => {
                    let value = self.pop()?;
                    let Some(Task::Property(p)) = run.tasks.last_mut() else {
                        return Err(invalid("property converter continuation"));
                    };
                    match await_ {
                        Await::PropertyTransformer => p.annotation = value,
                        Await::PropertyFunction => p.function = Some(value),
                        Await::PropertyResult => p.property = value,
                        _ => unreachable!(),
                    }
                }
                Await::ConverterVisit => {
                    let converted = self.pop()?;
                    let Some(Task::Visit { value, .. }) = run.tasks.last_mut() else {
                        return Err(invalid("converted visit"));
                    };
                    *value = converted;
                }
                Await::ConverterMember { key } => {
                    let converted = self.pop()?;
                    let Some(Task::Member(member)) = run.tasks.last_mut() else {
                        return Err(invalid("converted member"));
                    };
                    if key {
                        member.key = converted;
                    } else {
                        member.value = converted;
                    }
                }
                Await::Shallow => {
                    self.pop()?;
                }
                Await::KeyEquality => {
                    let Value::Bool(equal) = self.pop()? else {
                        return Err(invalid("renderer key equality"));
                    };
                    let Some(Task::Members(m)) = run.tasks.last_mut() else {
                        return Err(invalid("renderer member state"));
                    };
                    if equal {
                        let left = &m.seen[m.compare - 1].0.key;
                        let right = &m.members[m.next].descriptor.key;
                        if let (Value::Object(left), Value::Object(right)) = (left, right) {
                            self.heap.key_equalities.insert((*left, *right));
                        }
                        m.next += 1;
                        m.compare = 0;
                        m.prepared = false;
                    }
                }
            }
        }
        if run.option < 5 {
            let names = [
                "indent",
                "omitNullProperties",
                "useCustomStringDelimiters",
                "converters",
                "convertPropertyTransformers",
            ];
            let index = run.option;
            run.option += 1;
            run.await_ = Some(Await::Option(index));
            self.force(
                run.receiver,
                run.receiver,
                names[index].into(),
                false,
                true,
                false,
            )?;
            return Ok(false);
        }
        if run.option < 7 {
            let index = run.option - 2;
            run.option += 1;
            run.await_ = Some(Await::Mapping);
            self.force_shallow_frame(run.options[index].clone())?;
            return Ok(false);
        }
        if !run.started {
            run.started = true;
            let Value::String(indent) = &run.options[0] else {
                return Err(invalid("renderer indent type"));
            };
            run.indent = indent.clone();
            let Value::Bool(omit) = run.options[1] else {
                return Err(invalid("renderer omit type"));
            };
            run.omit_null = omit;
            let Value::Bool(custom) = run.options[2] else {
                return Err(invalid("renderer delimiter type"));
            };
            run.custom = custom;
            let (Value::Object(mapping), Value::Object(transformers)) =
                (&run.options[3], &run.options[4])
            else {
                return Err(invalid("renderer mappings"));
            };
            run.converters = Converters::load(&self.heap, *mapping, *transformers)?;
            run.tasks.push(Task::Visit {
                value: run.root.clone(),
                depth: 0,
                standalone: false,
                root: true,
                string_element: false,
                path: vec![PathPart::Root],
                converted: false,
            });
            return Ok(false);
        }
        let Some(task) = run.tasks.pop() else {
            if run.document && !run.output.is_empty() {
                run.append("\n");
            }
            return Ok(true);
        };
        match task {
            Task::RestoreSource(source) => {
                run.source = source;
            }
            Task::Text(text) => run.output.push_value(&text),
            Task::Visit {
                value,
                depth,
                standalone,
                root,
                string_element,
                path,
                converted,
            } => {
                if !converted && let Some(function) = run.converters.select(self, &value, &path)? {
                    run.tasks.push(Task::Visit {
                        value: value.clone(),
                        depth,
                        standalone,
                        root,
                        string_element,
                        path,
                        converted: true,
                    });
                    run.await_ = Some(Await::ConverterVisit);
                    self.invoke_converter(function, value)?;
                    return Ok(false);
                }
                if root {
                    run.root = value.clone();
                    if run.document && !self.pcf_document_value(&value)? {
                        let ty = crate::diagnostic_values::type_name(
                            &value,
                            &self.heap,
                            &self.program.module_name,
                        )?;
                        let text = crate::diagnostic_values::render(
                            &value,
                            &self.heap,
                            &self.program.module_name,
                            80,
                        )?;
                        return Err(RuntimeError::new(
                            ErrorKind::Type,
                            PklString::from(format!("The top-level value of a Pcf document must have type `Typed` or `Dynamic`, but got type `{ty}`.\nValue: ")).concat(&text),
                        ));
                    }
                }
                match value {
                    Value::Null | Value::NullWithDefault(_) => run.append("null"),
                    Value::Bool(value) => run.append(if value { "true" } else { "false" }),
                    Value::Int(value) => run.append(&value.to_string()),
                    Value::Float(value) => run.append(&crate::numeric::double_to_string(value)),
                    Value::String(value) => run.output.push_value(&super::pcf_format::string(
                        &value,
                        &run.indentation(depth + usize::from(!string_element)),
                        run.custom,
                    )),
                    Value::Bytes(value) => {
                        run.append("Bytes(");
                        for (index, byte) in value.iter().enumerate() {
                            if index != 0 {
                                run.append(", ");
                            }
                            run.append(&byte.to_string());
                        }
                        run.append(")");
                    }
                    Value::Object(id) => {
                        if let Some(pair) = self.heap.get(id)?.pair.clone() {
                            run.append("Pair(");
                            run.tasks.push(Task::Text(")".into()));
                            run.value(pair[1].clone(), depth, true, false, false, path.clone());
                            run.tasks.push(Task::Text(", ".into()));
                            run.value(pair[0].clone(), depth, true, false, false, path);
                            return Ok(false);
                        }
                        if self.heap.get(id)?.reference.is_some() {
                            return self.pcf_unrenderable(run, &Value::Object(id));
                        }
                        if !run.active.insert(id) {
                            return Err(RuntimeError::new(
                                ErrorKind::Cycle,
                                "cyclic value rendering",
                            ));
                        }
                        let kind = self.heap.get(id)?.collection_kind;
                        let strict = matches!(
                            kind,
                            CollectionKind::List | CollectionKind::Map | CollectionKind::Set
                        );
                        if strict {
                            run.append(match kind {
                                CollectionKind::List => "List(",
                                CollectionKind::Map => "Map(",
                                _ => "Set(",
                            });
                            let members = collections::ordered_members(&self.heap, id)?
                                .into_iter()
                                .map(|descriptor| collections::RenderMember {
                                    descriptor,
                                    skip: false,
                                    code: None,
                                })
                                .collect();
                            run.tasks.push(Task::Members(Members {
                                receiver: id,
                                members,
                                next: 0,
                                seen: Vec::new(),
                                compare: 0,
                                prepared: false,
                                hash: None,
                                depth,
                                document_root: false,
                                emitted: false,
                                strict: true,
                                path,
                            }));
                        } else {
                            run.tasks.push(Task::OpenObject {
                                receiver: id,
                                depth,
                                standalone,
                                root,
                                path,
                            });
                            run.await_ = Some(Await::Shallow);
                            self.force_shallow_frame(Value::Object(id))?;
                        }
                    }
                    value => return self.pcf_unrenderable(run, &value),
                }
            }
            Task::OpenObject {
                receiver,
                depth,
                standalone,
                root,
                path,
            } => {
                let doc = run.document && root;
                if !doc {
                    if standalone {
                        run.append("new ");
                    }
                    run.append("{");
                }
                run.tasks.push(Task::Members(Members {
                    receiver,
                    members: collections::render_members(&self.heap, receiver)?,
                    next: 0,
                    seen: Vec::new(),
                    compare: 0,
                    prepared: false,
                    hash: None,
                    depth: depth + usize::from(!doc),
                    document_root: doc,
                    emitted: false,
                    strict: false,
                    path,
                }));
            }
            Task::Members(mut m) => {
                if m.next == m.members.len() {
                    run.tasks.push(Task::Close {
                        receiver: m.receiver,
                        depth: m.depth,
                        document_root: m.document_root,
                        emitted: m.emitted,
                        strict: m.strict,
                    });
                    return Ok(false);
                }
                let item = m.members[m.next].clone();
                let descriptor = &item.descriptor;
                if !m.prepared {
                    m.prepared = true;
                    let key = descriptor.key.clone();
                    run.tasks.push(Task::Members(m));
                    self.prepare_render_key(key);
                    return Ok(false);
                }
                m.hash = self.java_hash(&descriptor.key)?;
                while m.compare < m.seen.len() {
                    let (seen, hash) = &m.seen[m.compare];
                    m.compare += 1;
                    if (seen.kind == CollectionMemberKind::Property)
                        != (descriptor.kind == CollectionMemberKind::Property)
                        || hash.zip(m.hash).is_some_and(|(a, b)| a != b)
                    {
                        continue;
                    }
                    let left = seen.key.clone();
                    let right = descriptor.key.clone();
                    run.tasks.push(Task::Members(m));
                    run.await_ = Some(Await::KeyEquality);
                    self.begin_equality(left, right, false);
                    return Ok(false);
                }
                m.seen.push((descriptor.clone(), m.hash));
                m.next += 1;
                m.compare = 0;
                m.prepared = false;
                if item.skip {
                    run.tasks.push(Task::Members(m));
                    return Ok(false);
                }
                let value = collections::cached_render_value(&self.heap, m.receiver, descriptor)?
                    .ok_or_else(|| invalid("shallow-forced member is uncached"))?;
                let descriptor = descriptor.clone();
                let key = descriptor.key.clone();
                let previous_source = run.source.clone();
                if let Some(metadata) = item.code.and_then(|code| self.program.metadata.get(code))
                    && let Some(span) = metadata.member_span.or(metadata.declaration)
                {
                    let renderer = &self.program.metadata[self.frames.last().unwrap().block];
                    let source = metadata
                        .source
                        .as_ref()
                        .or(self.context.primary_source.as_ref())
                        .map(|source| {
                            Arc::new(crate::value::RuntimeSource {
                                module_name: renderer.source.as_ref().map_or_else(
                                    || self.program.module_name.clone(),
                                    |source| source.module_name.clone(),
                                ),
                                ..(**source).clone()
                            })
                        });
                    run.source = Some(RuntimeFrame {
                        source,
                        span,
                        member: renderer.member.clone(),
                    });
                }
                run.tasks.push(Task::Member(MemberRun {
                    members: m,
                    descriptor,
                    key,
                    value,
                    step: 0,
                    output_name: None,
                    previous_source,
                }));
            }
            Task::Member(mut member) => {
                if member.step == 0 {
                    member.step = 1;
                    if member.descriptor.kind == CollectionMemberKind::Property {
                        let annotations = self.pcf_property_annotations(
                            member.members.receiver,
                            &member.descriptor.name,
                        )?;
                        if !annotations.is_empty() {
                            let property = self.make_pair(
                                Value::String(member.descriptor.name.clone().into()),
                                member.value.clone(),
                            );
                            run.tasks.push(Task::Property(PropertyRun {
                                member,
                                annotations,
                                next: 0,
                                annotation: Value::Null,
                                property,
                                function: None,
                                step: 0,
                            }));
                            return Ok(false);
                        }
                    }
                    if member.descriptor.kind == CollectionMemberKind::Entry
                        && let Some(function) = run.converters.select(self, &member.key, &[])?
                    {
                        let argument = member.key.clone();
                        run.tasks.push(Task::Member(member));
                        run.await_ = Some(Await::ConverterMember { key: true });
                        self.invoke_converter(function, argument)?;
                        return Ok(false);
                    }
                }
                let mut path = member.members.path.clone();
                path.push(
                    if member.descriptor.kind == CollectionMemberKind::Property {
                        PathPart::Property(member.descriptor.name.clone().into())
                    } else {
                        PathPart::Entry(member.key.clone())
                    },
                );
                if member.step == 1 {
                    member.step = 2;
                    if let Some(function) = run.converters.select(self, &member.value, &path)? {
                        let argument = member.value.clone();
                        run.tasks.push(Task::Member(member));
                        run.await_ = Some(Await::ConverterMember { key: false });
                        self.invoke_converter(function, argument)?;
                        return Ok(false);
                    }
                }
                let MemberRun {
                    members: mut m,
                    descriptor,
                    key,
                    value,
                    output_name,
                    previous_source,
                    ..
                } = member;
                if descriptor.kind == CollectionMemberKind::Property
                    && run.omit_null
                    && value.is_null()
                {
                    run.tasks.push(Task::Members(m));
                    run.tasks.push(Task::RestoreSource(previous_source));
                    return Ok(false);
                }
                let depth = m.depth;
                if m.strict {
                    if m.emitted {
                        run.append(", ");
                    }
                    m.emitted = true;
                    let map = self.heap.get(m.receiver)?.collection_kind == CollectionKind::Map;
                    run.tasks.push(Task::Members(m));
                    run.tasks.push(Task::RestoreSource(previous_source));
                    run.value(value, depth, true, false, false, path);
                    if map {
                        run.tasks.push(Task::Text(", ".into()));
                        run.value(key, depth, true, false, false, Vec::new());
                    }
                } else {
                    match descriptor.kind {
                        CollectionMemberKind::Property => {
                            if !run.output.is_empty() {
                                run.append("\n");
                                run.output.push_value(&run.indentation(depth));
                            }
                            let name =
                                output_name.unwrap_or_else(|| descriptor.name.clone().into());
                            if name
                                .as_str()
                                .is_some_and(crate::parser::is_regular_identifier)
                            {
                                run.output.push_value(&name);
                            } else {
                                run.append("`");
                                run.output.push_value(&name);
                                run.append("`");
                            }
                            run.append(if self.pcf_object_like(&value)? {
                                " "
                            } else {
                                " = "
                            });
                            m.emitted = true;
                            run.tasks.push(Task::Members(m));
                            run.tasks.push(Task::RestoreSource(previous_source));
                            run.value(value, depth, false, false, false, path);
                        }
                        CollectionMemberKind::Element => {
                            run.append("\n");
                            run.output.push_value(&run.indentation(depth));
                            m.emitted = true;
                            run.tasks.push(Task::Members(m));
                            run.tasks.push(Task::RestoreSource(previous_source));
                            run.value(value, depth, true, false, true, path);
                        }
                        CollectionMemberKind::Entry => {
                            run.append("\n");
                            run.output.push_value(&run.indentation(depth));
                            run.append("[");
                            let separator = if self.pcf_object_like(&value)? {
                                "] "
                            } else {
                                "] = "
                            };
                            m.emitted = true;
                            run.tasks.push(Task::Members(m));
                            run.tasks.push(Task::RestoreSource(previous_source));
                            run.value(value, depth, false, false, false, path);
                            run.tasks.push(Task::Text(separator.into()));
                            run.value(key, depth, true, false, false, Vec::new());
                        }
                    }
                }
            }
            Task::Property(mut p) => {
                if p.step == 0 {
                    if p.next == p.annotations.len() {
                        let Value::Object(id) = p.property else {
                            return Err(RuntimeError::new(
                                ErrorKind::Unsupported,
                                "Property conversion returned a non-Pair value (pinned Java internal error).",
                            ));
                        };
                        let pair=self.heap.get(id)?.pair.as_ref().ok_or_else(||RuntimeError::new(ErrorKind::Unsupported,"Property conversion returned a non-Pair value (pinned Java internal error)."))?;
                        let Value::String(name) = &pair[0] else {
                            return Err(RuntimeError::new(
                                ErrorKind::Unsupported,
                                "Property conversion returned a non-String name (pinned Java internal error).",
                            ));
                        };
                        p.member.output_name = Some(name.clone());
                        p.member.value = pair[1].clone();
                        run.tasks.push(Task::Member(p.member));
                        return Ok(false);
                    }
                    let annotation = p.annotations[p.next];
                    p.annotation = Value::Object(annotation);
                    p.step = 1;
                    if let Some(function) = run.converters.transformer(self, annotation)? {
                        let value = p.annotation.clone();
                        run.tasks.push(Task::Property(p));
                        run.await_ = Some(Await::PropertyTransformer);
                        self.invoke_converter(function, value)?;
                        return Ok(false);
                    }
                }
                if p.step == 1 {
                    p.step = 2;
                    let annotation = p.annotation.as_object()?;
                    run.tasks.push(Task::Property(p));
                    run.await_ = Some(Await::PropertyFunction);
                    self.force(annotation, annotation, "render".into(), false, true, false)?;
                    return Ok(false);
                }
                if p.step == 2 {
                    p.step = 3;
                    let function = p
                        .function
                        .take()
                        .ok_or_else(|| invalid("annotation render function"))?;
                    let Value::Function(function) = function else {
                        return Err(invalid("checked annotation function"));
                    };
                    let property = p.property.clone();
                    run.tasks.push(Task::Property(p));
                    run.await_ = Some(Await::PropertyResult);
                    self.invoke_function(
                        function,
                        vec![property, Value::Object(run.receiver)],
                        false,
                    )?;
                    return Ok(false);
                }
                p.step = 0;
                p.next += 1;
                run.tasks.push(Task::Property(p));
            }
            Task::Close {
                receiver,
                depth,
                document_root,
                emitted,
                strict,
            } => {
                if strict {
                    run.append(")");
                } else if !document_root {
                    if emitted {
                        run.append("\n");
                        run.output.push_value(&run.indentation(depth - 1));
                    }
                    run.append("}");
                }
                run.active.remove(&receiver);
            }
        }
        Ok(false)
    }
    fn pcf_property_annotations(
        &self,
        receiver: ObjectId,
        name: &str,
    ) -> Result<Vec<ObjectId>, RuntimeError> {
        let mut definitions = Vec::new();
        let mut owner = Some(receiver);
        let mut seen = BTreeSet::new();
        while let Some(id) = owner {
            if !seen.insert(id) {
                return Err(RuntimeError::new(
                    ErrorKind::Cycle,
                    "cyclic class hierarchy",
                ));
            }
            let object = self.heap.get(id)?;
            if object.class_definition.is_some()
                && let Some(member) = object
                    .members
                    .iter()
                    .find(|m| !m.local && !m.method && !m.entry && m.name.as_ref() == name)
            {
                definitions.push((id, member));
            }
            owner = object.parent;
        }
        let Some(selected) = definitions
            .iter()
            .position(|(_, m)| m.ty.is_some())
            .or_else(|| definitions.len().checked_sub(1))
        else {
            return Ok(Vec::new());
        };
        let mut result = Vec::new();
        for (id, member) in definitions[selected..].iter().rev() {
            for value in self
                .heap
                .get(*id)?
                .member_annotations
                .get(&member.code)
                .into_iter()
                .flatten()
            {
                if builtins::ty(output::CONVERT_PROPERTY).accepts(value, &self.heap) {
                    result.push(value.as_object()?);
                }
            }
        }
        Ok(result)
    }
    fn invoke_converter(&mut self, function: Value, value: Value) -> Result<(), RuntimeError> {
        let Value::Function(id) = function else {
            return Err(RuntimeError::new(
                ErrorKind::Type,
                "Renderer converter must be a function.",
            ));
        };
        self.invoke_function(id, vec![value], false)
    }
    fn pcf_object_like(&self, value: &Value) -> Result<bool, RuntimeError> {
        Ok(
            matches!(value,Value::Object(id) if self.heap.get(*id)?.reference.is_none() && self.heap.get(*id)?.pair.is_none() && !matches!(self.heap.get(*id)?.collection_kind,CollectionKind::List|CollectionKind::Map|CollectionKind::Set)),
        )
    }
    fn pcf_document_value(&self, value: &Value) -> Result<bool, RuntimeError> {
        Ok(
            matches!(value,Value::Object(id) if self.heap.get(*id)?.reference.is_none() && self.heap.get(*id)?.pair.is_none() && (collections::typed_class(&self.heap,*id)?.is_some() || self.heap.get(*id)?.collection_kind==CollectionKind::Dynamic)),
        )
    }
    fn pcf_unrenderable(&self, run: &RenderRun, value: &Value) -> Result<bool, RuntimeError> {
        let ty = crate::diagnostic_values::type_name(value, &self.heap, &self.program.module_name)?;
        let rendered =
            crate::diagnostic_values::render(value, &self.heap, &self.program.module_name, 80)?;
        let mut error = RuntimeError::new(
            ErrorKind::Type,
            PklString::from(format!(
                "Cannot render value of type `{ty}` as Pcf.\nValue: "
            ))
            .concat(&rendered),
        );
        error.hint = Some("Consider adding a converter to `output.converters`.".into());
        error.frames.extend(run.source.clone());
        Err(error)
    }
}

fn path_roots(path: &[PathPart], roots: &mut Vec<Value>) {
    roots.extend(path.iter().filter_map(|part| {
        if let PathPart::Entry(value) = part {
            Some(value.clone())
        } else {
            None
        }
    }));
}
