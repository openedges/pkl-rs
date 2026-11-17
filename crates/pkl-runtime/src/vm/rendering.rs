//! Value renderers share evaluator-owned forcing, conversion and Host continuations.
use super::converters::{Converters, PathPart};
use super::*;
use crate::heap::{CollectionKind, CollectionMember, CollectionMemberKind};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Format {
    Pcf,
    PklBinary,
    Protobuf,
    Json,
    Jsonnet,
    Xml,
    Properties,
    Plist,
    Yaml,
}
impl Format {
    fn name(self) -> &'static str {
        match self {
            Self::Pcf => "Pcf",
            Self::Protobuf => "Protobuf",
            Self::PklBinary => "pkl-binary",
            Self::Json => "JSON",
            Self::Jsonnet => "Jsonnet",
            Self::Xml => "XML",
            Self::Properties => "Properties",
            Self::Plist => "XML property list",
            Self::Yaml => "YAML",
        }
    }
}

enum XmlMembers {
    Attributes { name: PklString },
    Elements,
}
struct Members {
    xml: Option<XmlMembers>,
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
    array: bool,
    stream: bool,
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
    Protobuf(ProtoTask),
    BinaryArray(usize),
    Xml(XmlTask),
    JsonnetKey {
        key: Value,
        depth: usize,
    },
    PlistKey {
        key: Value,
        depth: usize,
    },
    YamlKey(Value),
    PropertiesKey {
        path: Vec<PathPart>,
        next: usize,
        following: bool,
    },
    PropertiesDirective {
        receiver: ObjectId,
        path: Vec<PathPart>,
    },
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
        stream: bool,
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
        array: bool,
        stream: bool,
    },
}
enum Await {
    ProtobufAlternative,
    ProtobufString,
    XmlRead {
        receiver: ObjectId,
        path: Vec<PathPart>,
        hint: bool,
    },
    JsonnetWrapper {
        receiver: ObjectId,
        path: Vec<PathPart>,
        depth: usize,
        import_str: bool,
    },
    PlistKey {
        receiver: ObjectId,
        depth: usize,
    },
    PropertiesKey {
        verbatim: bool,
    },
    Option(usize),
    Mapping,
    Shallow,
    KeyEquality,
    ConverterVisit,
    PropertyTransformer,
    PropertyFunction,
    PropertyResult,
    ConverterMember {
        key: bool,
    },
    Directive {
        receiver: ObjectId,
        path: Vec<PathPart>,
    },
}
pub(super) struct RenderRun {
    receiver: ObjectId,
    root: Value,
    document_original: Option<Value>,
    document: bool,
    format: Format,
    option: usize,
    options: Vec<Value>,
    await_: Option<Await>,
    tasks: Vec<Task>,
    active: BTreeSet<ObjectId>,
    output: crate::pkl_string::PklStringBuilder,
    binary: super::pkl_binary_wire::BinaryWire,
    protobuf: ProtoState,
    indent: PklString,
    omit_null: bool,
    restrict_charset: bool,
    properties_document: bool,
    yaml: Option<super::yaml::Emitter>,
    yaml_stream: bool,
    xml_line: usize,
    custom: bool,
    started: bool,
    converters: Converters,
    source: Option<RuntimeFrame>,
}
impl RenderRun {
    pub(super) fn roots(&self, roots: &mut Vec<Value>) {
        roots.push(Value::Object(self.receiver));
        roots.push(self.root.clone());
        roots.extend(self.document_original.iter().cloned());
        roots.extend(self.options.iter().cloned());
        roots.extend(self.active.iter().copied().map(Value::Object));
        self.converters.roots(roots);
        self.protobuf.roots(roots);
        if let Some(
            Await::Directive { receiver, path }
            | Await::JsonnetWrapper { receiver, path, .. }
            | Await::XmlRead { receiver, path, .. },
        ) = &self.await_
        {
            roots.push(Value::Object(*receiver));
            path_roots(path, roots);
        }
        if let Some(Await::PlistKey { receiver, .. }) = &self.await_ {
            roots.push(Value::Object(*receiver));
        }
        for task in &self.tasks {
            match task {
                Task::Protobuf(task) => task.roots(roots),
                Task::Xml(task) => task.roots(roots),
                Task::YamlKey(key) | Task::PlistKey { key, .. } | Task::JsonnetKey { key, .. } => {
                    roots.push(key.clone())
                }
                Task::PropertiesKey { path, .. } => path_roots(path, roots),
                Task::PropertiesDirective { receiver, path } => {
                    roots.push(Value::Object(*receiver));
                    path_roots(path, roots);
                }
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
                Task::Text(_) | Task::RestoreSource(_) | Task::BinaryArray(_) => {}
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
        if self.format == Format::Protobuf
            && let Some(last) = text.encode_utf16().last()
        {
            self.protobuf.last = Some(last);
        }
        if let Some(yaml) = &mut self.yaml {
            yaml.text(text);
        } else {
            self.output.push_str(text);
        }
    }
    fn append_value(&mut self, text: &PklString) {
        if self.format == Format::Protobuf
            && let Some(last) = text.encode_utf16().last()
        {
            self.protobuf.last = Some(last);
        }
        if let Some(yaml) = &mut self.yaml {
            yaml.raw(text);
        } else {
            self.output.push_value(text);
        }
    }
    fn output_empty(&self) -> bool {
        self.yaml
            .as_ref()
            .map_or_else(|| self.output.is_empty(), super::yaml::Emitter::is_empty)
    }
    fn json_newline(&mut self, depth: usize) {
        if !self.indent.is_empty() {
            self.append("\n");
            self.output.push_value(&self.indentation(depth));
        }
    }
    fn plist_key(&mut self, text: &PklString, depth: usize) {
        self.append_value(&self.indentation(depth));
        self.append("<key>");
        self.append_value(&super::plist::escape(text));
        self.append("</key>\n");
        self.append_value(&self.indentation(depth));
    }
    fn properties_value(&mut self, value: PklString, path: Vec<PathPart>) {
        if self.properties_document {
            self.tasks.push(Task::Text("\n".into()));
            self.tasks.push(Task::Text(properties_string(
                &value,
                false,
                self.restrict_charset,
            )));
            self.tasks.push(Task::Text(" = ".into()));
            self.tasks.push(Task::PropertiesKey {
                path,
                next: 0,
                following: false,
            });
        } else {
            self.output
                .push_value(&properties_string(&value, false, self.restrict_charset));
        }
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
    /// An outer renderDocument overwrites inner hints while unwinding. The
    /// top-level converter call itself is outside Java's render try/catch.
    pub(super) fn document_conversion_hint(
        &self,
        unwind_start: usize,
    ) -> Result<Option<PklString>, RuntimeError> {
        for frame in &self.frames[unwind_start..] {
            let Some(run) = &frame.pcf_render else {
                continue;
            };
            let Some(original) = &run.document_original else {
                continue;
            };
            match same_render_reference(original, &run.root) {
                Some(true) => continue,
                None => {
                    return Err(RuntimeError::new(
                        ErrorKind::Unsupported,
                        "Java reference identity for this unchanged scalar or byte-payload conversion is not preserved; renderDocument conversion hints are not implemented for this boundary.",
                    ));
                }
                Some(false) => (),
            }
            let render = |value: &Value| {
                crate::diagnostic_values::render_string(
                    value,
                    &self.heap,
                    &self.program.module_name,
                    80,
                )
            };
            let mut hint = crate::pkl_string::PklStringBuilder::default();
            hint.push_str("This value was converted during rendering. Previous: ");
            hint.push_value(&render(original)?);
            hint.push_str(". After: ");
            hint.push_value(&render(&run.root)?);
            hint.push('.');
            return Ok(Some(hint.finish()));
        }
        Ok(None)
    }

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
            let path = match &run.await_ {
                Some(
                    Await::Directive { receiver, path }
                    | Await::JsonnetWrapper { receiver, path, .. }
                    | Await::XmlRead {
                        receiver,
                        path,
                        hint: true,
                    },
                ) if *receiver == force.receiver => {
                    // VmUndefinedValueException.fillInHint appends the missing
                    // member key even when the receiver is a RenderDirective.
                    let mut path = path.clone();
                    path.push(PathPart::Property(force.name.clone()));
                    path
                }
                Some(Await::PlistKey { receiver, .. }) if *receiver == force.receiver => {
                    vec![PathPart::Property(force.name.clone())]
                }
                Some(Await::Shallow) => {
                    let (receiver, path) = match run.tasks.last() {
                        Some(Task::OpenObject { receiver, path, .. }) => (receiver, path),
                        Some(Task::Xml(XmlTask {
                            value: Value::Object(receiver),
                            path,
                            action: XmlAction::CheckedObject { .. },
                            ..
                        })) => (receiver, path),
                        _ => continue,
                    };
                    if *receiver != force.receiver {
                        continue;
                    }
                    let mut path = path.clone();
                    if force.entry {
                        path.push(PathPart::Entry(force.entry_probe.clone()?));
                    } else {
                        path.push(PathPart::Property(force.name.clone()));
                    }
                    path
                }
                _ => continue,
            };
            let mut text = crate::pkl_string::PklStringBuilder::default();
            let mut trailing = false;
            for part in path {
                match part {
                    PathPart::Root => {}
                    PathPart::WildcardProperty => {
                        if trailing {
                            text.push('.');
                        }
                        text.push('*');
                        trailing = true;
                    }
                    PathPart::WildcardElement => {
                        text.push_str("[*]");
                        trailing = true;
                    }
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
            if let Value::Object(id) = &run.root
                && let Ok(module) = self.heap.get(*id)
                && module.is_module
            {
                // Java uses the rendered module's ModuleInfo.getModuleKey(),
                // including when a public evaluator has no global primary source
                // or the rendered module differs from the evaluator's caller.
                let unit = self.program.units.get(module.module_unit?.0 as usize)?;
                let uri = unit.key.as_ref().map(|key| &key.uri).or_else(|| {
                    // Statically compiled modules register their original identity
                    // before they have a selected Host key.
                    unit.identity
                        .as_ref()
                        .map(|identity| &identity.original_uri)
                })?;
                hint.push_str(" of module `");
                hint.push_value(uri);
                hint.push('`');
            }
            hint.push('.');
            return Some(hint.finish());
        }
        None
    }
    pub(super) fn begin_jsonnet_render(&mut self, document: bool) -> Result<(), RuntimeError> {
        // Authored methods may capture lexical locals before their argument.
        let root = self
            .frames
            .last()
            .and_then(|frame| frame.environment.locals.last())
            .cloned()
            .ok_or_else(|| invalid("Jsonnet renderer argument"))?;
        self.stack.push(root);
        self.begin_render(document, Format::Jsonnet)
    }
    pub(super) fn begin_render(
        &mut self,
        document: bool,
        format: Format,
    ) -> Result<(), RuntimeError> {
        let root = self.pop()?;
        let receiver = if matches!(
            format,
            Format::Jsonnet | Format::Xml | Format::PklBinary | Format::Protobuf
        ) {
            self.this_receiver()?.as_object()?
        } else {
            self.scope(0)?.receiver
        };
        self.frames.last_mut().expect("renderer frame").pcf_render = Some(RenderRun {
            receiver,
            root,
            document_original: None,
            document,
            format,
            option: 0,
            options: Vec::new(),
            await_: None,
            tasks: Vec::new(),
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
            Ok(true) if run.format == Format::PklBinary => {
                self.stack
                    .push(Value::Bytes(run.binary.into_bytes().into()));
            }
            Ok(true) => {
                let text = match run.yaml {
                    Some(yaml) => yaml.finish(),
                    None => run.output.finish(),
                };
                self.stack.push(Value::String(text));
            }
            Ok(false) | Err(_) => self.frames[index].pcf_render = Some(run),
        }
        result.map(|_| ())
    }
    fn advance_pcf(&mut self, run: &mut RenderRun) -> Result<bool, RuntimeError> {
        if let Some(await_) = run.await_.take() {
            match await_ {
                Await::ProtobufAlternative => {
                    let Value::Bool(accepted) = self.pop()? else {
                        return Err(invalid("protobuf alternative result"));
                    };
                    let Some(Task::Protobuf(ProtoTask::Field(field))) = run.tasks.last_mut() else {
                        return Err(invalid("protobuf alternative continuation"));
                    };
                    field.accepted = Some(accepted);
                }
                Await::ProtobufString => {
                    let Value::String(text) = self.pop()? else {
                        return Err(invalid("protobuf scalar string"));
                    };
                    run.append_value(&text);
                }
                Await::XmlRead { .. } => {
                    let value = self.pop()?;
                    let Some(Task::Xml(task)) = run.tasks.last_mut() else {
                        return Err(invalid("XML read continuation"));
                    };
                    task.input = Some(value);
                }
                Await::JsonnetWrapper {
                    depth, import_str, ..
                } => {
                    let Value::String(text) = self.pop()? else {
                        return Err(invalid("checked Jsonnet wrapper String"));
                    };
                    if import_str {
                        run.append("importstr ");
                        run.append_value(&super::jsonnet::quoted(&text));
                    } else {
                        run.append("std.extVar(");
                        run.append_value(&super::jsonnet::string(
                            &text,
                            &run.indent,
                            &run.indentation(depth),
                        ));
                        run.append(")");
                    }
                }
                Await::PlistKey { depth, .. } => {
                    let Value::String(text) = self.pop()? else {
                        return Err(invalid("checked Plist directive key text"));
                    };
                    run.plist_key(&text, depth);
                }
                Await::PropertiesKey { verbatim } => {
                    let Value::String(text) = self.pop()? else {
                        return Err(invalid("Properties key string"));
                    };
                    if verbatim {
                        run.append_value(&text);
                    } else {
                        run.append_value(&properties_string(&text, true, run.restrict_charset));
                    }
                }
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
                Await::Directive { .. } if run.format == Format::PklBinary => {
                    let Value::Bytes(bytes) = self.pop()? else {
                        return Err(invalid("checked render directive bytes"));
                    };
                    run.binary.raw(&bytes)?;
                }
                Await::Directive { .. } => {
                    let Value::String(text) = self.pop()? else {
                        return Err(invalid("checked render directive text"));
                    };
                    run.append_value(&text);
                    if run.format == Format::Properties && run.properties_document {
                        run.append("\n");
                    }
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
        let names: &[&str] = if run.format == Format::PklBinary {
            &["converters", "convertPropertyTransformers"]
        } else if run.format == Format::Pcf {
            &[
                "indent",
                "omitNullProperties",
                "useCustomStringDelimiters",
                "converters",
                "convertPropertyTransformers",
            ]
        } else if run.format == Format::Yaml {
            &[
                "mode",
                "indentWidth",
                "omitNullProperties",
                "isStream",
                "converters",
                "convertPropertyTransformers",
            ]
        } else if run.format == Format::Xml {
            &[
                "indent",
                "xmlVersion",
                "rootElementName",
                "rootElementAttributes",
                "converters",
                "convertPropertyTransformers",
            ]
        } else if matches!(run.format, Format::Plist | Format::Protobuf) {
            &["indent", "converters", "convertPropertyTransformers"]
        } else if matches!(run.format, Format::Json | Format::Jsonnet) {
            &[
                "indent",
                "omitNullProperties",
                "converters",
                "convertPropertyTransformers",
            ]
        } else {
            &[
                "omitNullProperties",
                "restrictCharset",
                "converters",
                "convertPropertyTransformers",
            ]
        };
        // Java constructs the narrowed indent before evaluating converter getters.
        if run.format == Format::Yaml && run.option == 4 {
            let Value::Int(width) = &run.options[1] else {
                return Err(invalid("YAML indent"));
            };
            if (width.get() as i32) < 0 {
                return Err(RuntimeError::new(
                    ErrorKind::Unsupported,
                    "Pinned Java YAML negative narrowed indent raises an oracle-internal error; this boundary remains unresolved.",
                ));
            }
        }
        if run.option < names.len() {
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
        if run.option < names.len() + 2 {
            let index = run.option - 2;
            run.option += 1;
            run.await_ = Some(Await::Mapping);
            self.force_shallow_frame(run.options[index].clone())?;
            return Ok(false);
        }
        if !run.started {
            run.started = true;
            if run.format == Format::PklBinary {
                // BytesRenderer has no text formatting or omission options.
            } else if run.format == Format::Yaml {
                let (
                    Value::String(mode),
                    Value::Int(width),
                    Value::Bool(omit),
                    Value::Bool(stream),
                ) = (
                    &run.options[0],
                    &run.options[1],
                    &run.options[2],
                    &run.options[3],
                )
                else {
                    return Err(invalid("YAML options"));
                };
                let width = width.get() as i32;
                if width == 0 {
                    return Err(RuntimeError::new(
                        ErrorKind::Unsupported,
                        "Pinned Java YAML zero narrowed indent raises an oracle-internal error; this boundary remains unresolved.",
                    ));
                }
                let mode = match mode.as_str() {
                    Some("compat") => super::yaml::Mode::Compat,
                    Some("1.1") => super::yaml::Mode::Yaml11,
                    Some("1.2") => super::yaml::Mode::Yaml12,
                    _ => return Err(invalid("checked YAML mode")),
                };
                run.yaml = Some(super::yaml::Emitter::new(mode, width as usize));
                run.omit_null = *omit;
                run.yaml_stream = *stream;
            } else if run.format == Format::Xml {
                let Value::String(indent) = &run.options[0] else {
                    return Err(invalid("XML indent"));
                };
                run.indent = indent.clone();
                run.omit_null = true;
            } else if matches!(run.format, Format::Plist | Format::Protobuf) {
                let Value::String(indent) = &run.options[0] else {
                    return Err(invalid("Plist indent type"));
                };
                run.indent = indent.clone();
                // Pinned PListRenderer always omits null properties and entries.
                run.omit_null = true;
            } else if run.format == Format::Properties {
                let (Value::Bool(omit), Value::Bool(restrict)) = (&run.options[0], &run.options[1])
                else {
                    return Err(invalid("Properties renderer options"));
                };
                run.omit_null = *omit;
                run.restrict_charset = *restrict;
            } else if run.format == Format::Jsonnet {
                run.indent = match &run.options[0] {
                    Value::String(indent) => indent.clone(),
                    value if value.is_null() => PklString::default(),
                    _ => return Err(invalid("checked Jsonnet indent")),
                };
                let Value::Bool(omit) = run.options[1] else {
                    return Err(invalid("checked Jsonnet omit"));
                };
                run.omit_null = omit;
            } else {
                let Value::String(indent) = &run.options[0] else {
                    return Err(invalid("renderer indent type"));
                };
                run.indent = indent.clone();
                let Value::Bool(omit) = run.options[1] else {
                    return Err(invalid("renderer omit type"));
                };
                run.omit_null = omit;
            }
            if run.format == Format::Pcf {
                let Value::Bool(custom) = run.options[2] else {
                    return Err(invalid("renderer delimiter type"));
                };
                run.custom = custom;
            }
            let (Value::Object(mapping), Value::Object(transformers)) =
                (&run.options[names.len() - 2], &run.options[names.len() - 1])
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
            if run.format == Format::Protobuf {
                if !run.document && run.protobuf.wrappers.pop() != Some(false) {
                    return Err(invalid("protobuf root wrapper"));
                }
                if !run.protobuf.names.is_empty()
                    || !run.protobuf.wrappers.is_empty()
                    || run.protobuf.depth != 0
                {
                    return Err(invalid("protobuf traversal stack"));
                }
            }
            if run.document && run.format == Format::Protobuf {
                run.proto_newline();
            } else if run.document && run.format == Format::Plist {
                run.append("\n</plist>\n");
            } else if run.document && run.format == Format::Yaml {
                run.yaml.as_mut().unwrap().newline();
            } else if run.document
                && !matches!(run.format, Format::Properties | Format::PklBinary)
                && (matches!(run.format, Format::Json | Format::Jsonnet) || !run.output_empty())
            {
                run.append("\n");
            }
            return Ok(true);
        };
        match task {
            Task::Protobuf(task) => return self.advance_protobuf(run, task),
            Task::BinaryArray(length) => run.binary.array(length)?,
            Task::Xml(task) => return self.advance_xml(run, task),
            Task::JsonnetKey { key, depth } => match key {
                Value::String(text) => run.append_value(&super::jsonnet::field_name(
                    &text,
                    &run.indent,
                    &run.indentation(depth),
                )),
                Value::Object(receiver) => {
                    run.await_ = Some(Await::Directive {
                        receiver,
                        path: Vec::new(),
                    });
                    self.force(receiver, receiver, "text".into(), false, true, false)?;
                }
                _ => return Err(invalid("checked Jsonnet key")),
            },
            Task::PlistKey { key, depth } => match key {
                Value::String(text) => run.plist_key(&text, depth),
                Value::Object(receiver) => {
                    run.await_ = Some(Await::PlistKey { receiver, depth });
                    self.force(receiver, receiver, "text".into(), false, true, false)?;
                }
                _ => return Err(invalid("checked Plist key")),
            },
            Task::YamlKey(key) => {
                if let Value::Object(id) = key {
                    run.await_ = Some(Await::Directive {
                        receiver: id,
                        path: Vec::new(),
                    });
                    self.force(id, id, "text".into(), false, true, false)?;
                } else if let Value::String(text) = key {
                    run.yaml.as_mut().unwrap().string(&text, true);
                } else {
                    run.append_value(&yaml_scalar(&key)?);
                }
            }
            Task::PropertiesKey {
                path,
                mut next,
                mut following,
            } => {
                let Some(part) = path.get(next).cloned() else {
                    return Ok(false);
                };
                next += 1;
                if !matches!(part, PathPart::Root) {
                    if following {
                        run.append(".");
                    }
                    following = true;
                }
                run.tasks.push(Task::PropertiesKey {
                    path,
                    next,
                    following,
                });
                match part {
                    PathPart::Root => {}
                    PathPart::WildcardElement => run.append("[*]"),
                    PathPart::WildcardProperty => run.append("*"),
                    PathPart::Property(name) => {
                        run.append_value(&properties_string(&name, true, run.restrict_charset))
                    }
                    PathPart::Entry(value) => {
                        if let Value::Object(id) = value
                            && self.is_render_directive(id)?
                        {
                            run.await_ = Some(Await::PropertiesKey { verbatim: true });
                            self.force(id, id, "text".into(), false, true, false)?;
                        } else {
                            run.await_ = Some(Await::PropertiesKey { verbatim: false });
                            self.java_value_string(value)?;
                        }
                    }
                }
            }
            Task::PropertiesDirective { receiver, path } => {
                run.await_ = Some(Await::Directive { receiver, path });
                self.force(receiver, receiver, "text".into(), false, true, false)?;
            }
            Task::RestoreSource(source) => {
                run.source = source;
            }
            Task::Text(text) => run.append_value(&text),
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
                    if run.document && converted {
                        run.document_original = Some(run.root.clone());
                    }
                    run.root = value.clone();
                    if run.format == Format::Xml {
                        run.xml_dispatch(value, depth, path, XmlPlace::Top);
                        return Ok(false);
                    }
                    if run.format == Format::Yaml && run.document && run.yaml_stream {
                        let id = match value {
                            Value::Object(id)
                                if matches!(
                                    self.heap.get(id)?.collection_kind,
                                    CollectionKind::Listing
                                        | CollectionKind::List
                                        | CollectionKind::Set
                                ) =>
                            {
                                id
                            }
                            _ => {
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
                                return Err(RuntimeError::new(ErrorKind::Type, PklString::from(format!("The top-level value of a YAML stream must have type `Listing`, `List`, or `Set`, but got type `{ty}`.\nValue: ")).concat(&text)));
                            }
                        };
                        run.active.insert(id);
                        run.tasks.push(Task::OpenObject {
                            receiver: id,
                            depth,
                            standalone,
                            root,
                            path,
                            stream: true,
                        });
                        if self.heap.get(id)?.collection_kind == CollectionKind::Listing {
                            run.await_ = Some(Await::Shallow);
                            self.force_shallow_frame(Value::Object(id))?;
                        }
                        return Ok(false);
                    }
                    if run.format == Format::Plist && run.document {
                        // VmFunction is an ObjectLike in the pinned Java root gate,
                        // but subsequently fails its ordinary unsupported-type visit.
                        let valid = matches!(&value, Value::Function(_))
                            || matches!(&value, Value::Object(id) if self.heap.get(*id)?.pair.is_none() && self.heap.get(*id)?.reference.is_none());
                        if !valid {
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
                            return Err(RuntimeError::new(ErrorKind::Type, PklString::from(format!("The top-level value of an XML property list must have type `Typed`, `Dynamic`, `Listing`, `Mapping`, `List`, `Set`, or `Map`, but got type `{ty}`.\nValue: ")).concat(&text)));
                        }
                        run.append(super::plist::DOCUMENT_HEADER);
                    }
                    if run.format == Format::Properties {
                        let valid = self.properties_document_value(&value)?;
                        let directive =
                            matches!(&value, Value::Object(id) if self.is_render_directive(*id)?);
                        if run.document && !valid {
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
                            return Err(RuntimeError::new(ErrorKind::Type,
                                PklString::from(format!("The top-level value of a Java properties file must have type `Typed`, `Dynamic`, `Mapping`, or `Map`, but got type `{ty}`.\nValue: ")).concat(&text)));
                        }
                        if !run.document && valid && !directive {
                            return self.pcf_unrenderable(run, &value);
                        }
                        run.properties_document = run.document && !directive;
                    }
                    if run.format == Format::Pcf
                        && run.document
                        && !self.pcf_document_value(&value)?
                    {
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
                if run.format == Format::Protobuf {
                    return self.protobuf_visit(run, value, root, path);
                }
                if run.format == Format::PklBinary {
                    return self.binary_visit(run, value, depth, standalone, root, path);
                }
                if run.format == Format::Properties {
                    match &value {
                        Value::Null | Value::NullWithDefault(_) => {
                            if run.properties_document {
                                run.properties_value("".into(), path);
                            }
                            return Ok(false);
                        }
                        Value::String(text) => {
                            run.properties_value(text.clone(), path);
                            return Ok(false);
                        }
                        Value::Bool(_) | Value::Int(_) | Value::Float(_) => {
                            run.properties_value(scalar_string(&value)?, path);
                            return Ok(false);
                        }
                        Value::Object(_) => {}
                        _ => return self.pcf_unrenderable(run, &value),
                    }
                }
                if run.format == Format::Xml {
                    run.xml_dispatch(value, depth, path, XmlPlace::Visit);
                    return Ok(false);
                }
                if run.format == Format::Plist {
                    if let Some(text) = super::plist::scalar(&value) {
                        run.append_value(&text);
                        return Ok(false);
                    }
                    if !matches!(value, Value::Object(_)) {
                        return self.pcf_unrenderable(run, &value);
                    }
                }
                if run.format == Format::Yaml {
                    match &value {
                        Value::Null
                        | Value::NullWithDefault(_)
                        | Value::Bool(_)
                        | Value::Int(_)
                        | Value::Float(_) => {
                            run.yaml.as_mut().unwrap().scalar(&yaml_scalar(&value)?);
                            return Ok(false);
                        }
                        Value::String(text) => {
                            run.yaml.as_mut().unwrap().string_value(text);
                            return Ok(false);
                        }
                        Value::Bytes(bytes) => {
                            run.yaml.as_mut().unwrap().bytes(bytes);
                            return Ok(false);
                        }
                        Value::Object(_) => (),
                        _ => return self.pcf_unrenderable(run, &value),
                    }
                }
                match value {
                    Value::Null | Value::NullWithDefault(_) => run.append("null"),
                    Value::Bool(value) => run.append(if value { "true" } else { "false" }),
                    Value::Int(value) => run.append(&value.to_string()),
                    Value::Float(value) => {
                        let text = crate::numeric::double_to_string(*value);
                        if run.format == Format::Json && !value.is_finite() {
                            // ErrorMessages uses Java MessageFormat for the
                            // numeric argument, unlike the rendered value form.
                            let diagnostic = if value.is_nan() {
                                "NaN"
                            } else if value.is_sign_negative() {
                                "-∞"
                            } else {
                                "∞"
                            };
                            return Err(RuntimeError::new(
                                ErrorKind::Type,
                                format!("Cannot render value `{diagnostic}` as JSON."),
                            ));
                        }
                        run.append(&text);
                    }
                    Value::DataSize(value) => {
                        if matches!(run.format, Format::Json | Format::Jsonnet) {
                            return self.pcf_unrenderable(run, &Value::DataSize(value));
                        }
                        run.append(&value.to_string());
                    }
                    Value::IntSeq(value) => {
                        if matches!(run.format, Format::Json | Format::Jsonnet) {
                            return self.pcf_unrenderable(run, &Value::IntSeq(value));
                        }
                        run.append(&value.to_string());
                    }
                    Value::Regex(value) => {
                        if matches!(run.format, Format::Json | Format::Jsonnet) {
                            return self.pcf_unrenderable(run, &Value::Regex(value));
                        }
                        // The language PcfRenderer delegates to VmRegex.toString;
                        // renderer string settings do not apply to Regex patterns.
                        run.append_value(&super::regex::string(&value));
                    }
                    Value::Duration(value) => {
                        if matches!(run.format, Format::Json | Format::Jsonnet) {
                            return self.pcf_unrenderable(run, &Value::Duration(value));
                        }
                        run.append(&value.to_string());
                    }
                    Value::String(value) => run.append_value(&if run.format == Format::Jsonnet {
                        super::jsonnet::string(&value, &run.indent, &run.indentation(depth))
                    } else if run.format == Format::Json {
                        json_string(&value)
                    } else {
                        super::pcf_format::string(
                            &value,
                            &run.indentation(depth + usize::from(!string_element)),
                            run.custom,
                        )
                    }),
                    Value::Bytes(value) => {
                        if matches!(run.format, Format::Json | Format::Jsonnet) {
                            return self.pcf_unrenderable(run, &Value::Bytes(value));
                        }
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
                        if run.format == Format::Jsonnet
                            && let Some(import_str) = self.jsonnet_wrapper(id)?
                        {
                            run.await_ = Some(Await::JsonnetWrapper {
                                receiver: id,
                                path,
                                depth,
                                import_str,
                            });
                            self.force(
                                id,
                                id,
                                if import_str { "path" } else { "name" }.into(),
                                false,
                                true,
                                false,
                            )?;
                            return Ok(false);
                        }
                        if self.is_render_directive(id)? {
                            if run.format == Format::Properties && run.properties_document {
                                run.tasks.push(Task::PropertiesDirective {
                                    receiver: id,
                                    path: path.clone(),
                                });
                                run.tasks.push(Task::Text(" = ".into()));
                                run.tasks.push(Task::PropertiesKey {
                                    path,
                                    next: 0,
                                    following: false,
                                });
                                return Ok(false);
                            }
                            run.await_ = Some(Await::Directive { receiver: id, path });
                            self.force(id, id, "text".into(), false, true, false)?;
                            return Ok(false);
                        }
                        if self.heap.get(id)?.foreign_map_entry.is_some() {
                            return Err(RuntimeError::foreign(
                                crate::java_exception::provenance::ForeignOperation::KeyValueHolderTypeVisitor { is_set: false },
                                "Unknown VM value type: java.util.KeyValueHolder",
                            ));
                        }
                        if let Some(pair) = self.heap.get(id)?.pair.clone() {
                            if run.format != Format::Pcf {
                                return self.pcf_unrenderable(run, &Value::Object(id));
                            }
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
                            let array = kind != CollectionKind::Map;
                            if run.format == Format::Properties && array {
                                return self.pcf_unrenderable(run, &Value::Object(id));
                            }
                            if run.format == Format::Yaml {
                                run.yaml.as_mut().unwrap().start(array);
                            } else if !matches!(
                                run.format,
                                Format::Properties | Format::Plist | Format::Xml
                            ) {
                                run.append(
                                    if matches!(run.format, Format::Json | Format::Jsonnet) {
                                        if array { "[" } else { "{" }
                                    } else {
                                        match kind {
                                            CollectionKind::List => "List(",
                                            CollectionKind::Map => "Map(",
                                            _ => "Set(",
                                        }
                                    },
                                );
                            }
                            let members = collections::ordered_members(&self.heap, id)?
                                .into_iter()
                                .map(|descriptor| collections::RenderMember {
                                    descriptor,
                                    skip: false,
                                    code: None,
                                })
                                .collect();
                            run.tasks.push(Task::Members(Members {
                                xml: None,
                                receiver: id,
                                members,
                                next: 0,
                                seen: Vec::new(),
                                compare: 0,
                                prepared: false,
                                hash: None,
                                depth: depth
                                    + usize::from(matches!(
                                        run.format,
                                        Format::Json | Format::Jsonnet | Format::Plist
                                    )),
                                document_root: false,
                                emitted: false,
                                strict: true,
                                array,
                                stream: false,
                                path,
                            }));
                        } else {
                            run.tasks.push(Task::OpenObject {
                                receiver: id,
                                depth,
                                standalone,
                                root,
                                path,
                                stream: false,
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
                stream,
            } => {
                if run.format == Format::Properties
                    && self.heap.get(receiver)?.collection_kind == CollectionKind::Listing
                {
                    return self.pcf_unrenderable(run, &Value::Object(receiver));
                }
                let kind = self.heap.get(receiver)?.collection_kind;
                if run.format == Format::PklBinary
                    && kind == CollectionKind::Mapping
                    && self.heap.get(receiver)?.mapping_length.is_none()
                {
                    // VmMapping.getLength hashes keys before the first member
                    // converter; reuse the ordinary resumable length operation.
                    run.tasks.push(Task::OpenObject {
                        receiver,
                        depth,
                        standalone,
                        root,
                        path,
                        stream,
                    });
                    self.binary_prepare_mapping_length(receiver)?;
                    return Ok(false);
                }
                let members = if (stream
                    && matches!(kind, CollectionKind::List | CollectionKind::Set))
                    || (matches!(
                        run.format,
                        Format::Xml | Format::PklBinary | Format::Protobuf
                    ) && matches!(
                        kind,
                        CollectionKind::List | CollectionKind::Set | CollectionKind::Map
                    )) {
                    collections::ordered_members(&self.heap, receiver)?
                        .into_iter()
                        .map(|descriptor| collections::RenderMember {
                            descriptor,
                            skip: false,
                            code: None,
                        })
                        .collect()
                } else {
                    collections::render_members(&self.heap, receiver)?
                };
                let array = matches!(
                    kind,
                    CollectionKind::Listing | CollectionKind::List | CollectionKind::Set
                ) || members
                    .iter()
                    .any(|m| !m.skip && m.descriptor.kind == CollectionMemberKind::Element);
                let doc = run.format == Format::Pcf && run.document && root;
                if run.format == Format::Protobuf {
                    self.protobuf_start(run, receiver)?;
                } else if run.format == Format::PklBinary {
                    self.binary_start(run, receiver)?;
                } else if run.format == Format::Yaml && !stream {
                    run.yaml.as_mut().unwrap().start(array);
                } else if !doc
                    && !matches!(run.format, Format::Properties | Format::Plist | Format::Xml)
                    && !stream
                {
                    if run.format == Format::Pcf && standalone {
                        run.append("new ");
                    }
                    run.append(
                        if matches!(run.format, Format::Json | Format::Jsonnet) && array {
                            "["
                        } else {
                            "{"
                        },
                    );
                }
                run.tasks.push(Task::Members(Members {
                    xml: None,
                    receiver,
                    members,
                    next: 0,
                    seen: Vec::new(),
                    compare: 0,
                    prepared: false,
                    hash: None,
                    depth: depth + usize::from(!doc && run.format != Format::Xml),
                    document_root: doc,
                    emitted: false,
                    strict: matches!(
                        self.heap.get(receiver)?.collection_kind,
                        CollectionKind::List | CollectionKind::Set | CollectionKind::Map
                    ),
                    array,
                    stream,
                    path,
                }));
            }
            Task::Members(mut m) => {
                if m.next == m.members.len() {
                    if m.xml.is_some() {
                        return Ok(false);
                    }
                    run.tasks.push(Task::Close {
                        receiver: m.receiver,
                        depth: m.depth,
                        document_root: m.document_root,
                        emitted: m.emitted
                            || (matches!(
                                run.format,
                                Format::Json | Format::Jsonnet | Format::Yaml | Format::Plist
                            ) && m.strict
                                && !m.array
                                && !m.members.is_empty()),
                        strict: m.strict,
                        array: m.array,
                        stream: m.stream,
                    });
                    return Ok(false);
                }
                let item = m.members[m.next].clone();
                let descriptor = &item.descriptor;
                if !(matches!(
                    run.format,
                    Format::Properties
                        | Format::Jsonnet
                        | Format::Yaml
                        | Format::Plist
                        | Format::Xml
                        | Format::PklBinary
                        | Format::Protobuf
                ) && m.strict)
                    && !m.prepared
                {
                    m.prepared = true;
                    let key = descriptor.key.clone();
                    run.tasks.push(Task::Members(m));
                    self.prepare_render_key(key);
                    return Ok(false);
                }
                if !(matches!(
                    run.format,
                    Format::Properties
                        | Format::Jsonnet
                        | Format::Yaml
                        | Format::Plist
                        | Format::Xml
                        | Format::PklBinary
                        | Format::Protobuf
                ) && m.strict)
                {
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
                }
                m.next += 1;
                m.compare = 0;
                m.prepared = false;
                if item.skip {
                    run.tasks.push(Task::Members(m));
                    return Ok(false);
                }
                if matches!(
                    run.format,
                    Format::Json
                        | Format::Properties
                        | Format::Jsonnet
                        | Format::Yaml
                        | Format::Plist
                        | Format::Xml
                ) && m.array
                    && m.xml.is_none()
                    && descriptor.kind != CollectionMemberKind::Element
                {
                    let rendered = crate::diagnostic_values::render(
                        &Value::Object(m.receiver),
                        &self.heap,
                        &self.program.module_name,
                        80,
                    )?;
                    return Err(RuntimeError::new(ErrorKind::Type,
                        PklString::from(format!("Cannot render object with both elements and properties/entries as {}.\nObject: ", run.format.name())).concat(&rendered)));
                }
                let value = collections::cached_render_value(&self.heap, m.receiver, descriptor)?
                    .ok_or_else(|| invalid("shallow-forced member is uncached"))?;
                if let Some(mode) = &m.xml {
                    let action = match mode {
                        XmlMembers::Attributes { name } => XmlAction::Attribute {
                            name: name.clone(),
                            key: descriptor.key.clone(),
                        },
                        XmlMembers::Elements
                            if descriptor.kind == CollectionMemberKind::Element =>
                        {
                            XmlAction::Dispatch {
                                place: XmlPlace::Element,
                                marker_read: false,
                            }
                        }
                        XmlMembers::Elements => {
                            run.tasks.push(Task::Members(m));
                            return Ok(false);
                        }
                    };
                    let task = XmlTask {
                        value,
                        depth: m.depth,
                        path: m.path.clone(),
                        input: None,
                        action,
                    };
                    run.tasks.push(Task::Members(m));
                    run.tasks.push(Task::Xml(task));
                    return Ok(false);
                }
                if m.stream {
                    if m.emitted {
                        run.yaml.as_mut().unwrap().separator();
                    }
                    m.emitted = true;
                    let path = m.path.clone();
                    run.tasks.push(Task::Members(m));
                    run.value(value, 0, false, false, false, path);
                    return Ok(false);
                }
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
                        anonymous: false,
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
                            let property =
                                self.make_pair(member.descriptor.key.clone(), member.value.clone());
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
                        PathPart::Property(if let Value::String(name) = &member.descriptor.key {
                            name.clone()
                        } else {
                            unreachable!("property string key")
                        })
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
                if (descriptor.kind == CollectionMemberKind::Property
                    || (run.format != Format::Pcf
                        && descriptor.kind == CollectionMemberKind::Entry))
                    && run.omit_null
                    && value.is_null()
                {
                    run.tasks.push(Task::Members(m));
                    run.tasks.push(Task::RestoreSource(previous_source));
                    return Ok(false);
                }
                let depth = m.depth;
                if run.format == Format::Protobuf {
                    self.protobuf_member(
                        run,
                        m,
                        descriptor,
                        key,
                        value,
                        output_name,
                        previous_source,
                        path,
                    )?;
                } else if run.format == Format::PklBinary {
                    self.binary_member(
                        run,
                        m,
                        descriptor,
                        key,
                        value,
                        output_name,
                        previous_source,
                        path,
                    )?;
                } else if run.format == Format::Xml {
                    let place = if descriptor.kind == CollectionMemberKind::Element {
                        XmlPlace::Element
                    } else {
                        XmlPlace::Property {
                            key: if descriptor.kind == CollectionMemberKind::Property {
                                Value::String(output_name.unwrap_or_else(|| match descriptor.key {
                                    Value::String(name) => name,
                                    _ => unreachable!(),
                                }))
                            } else {
                                key
                            },
                            enclosing: m.receiver,
                            validate: true,
                        }
                    };
                    run.tasks.push(Task::Members(m));
                    run.tasks.push(Task::RestoreSource(previous_source));
                    run.xml_dispatch(value, depth, path, place);
                } else if run.format == Format::Plist {
                    let element = descriptor.kind == CollectionMemberKind::Element;
                    if descriptor.kind == CollectionMemberKind::Entry
                        && !matches!(&key, Value::String(_))
                        && !matches!(&key, Value::Object(id) if self.is_render_directive(*id)?)
                    {
                        return self.plist_non_string_key(m.receiver, &key);
                    }
                    if !m.emitted {
                        run.append(if element { "<array>\n" } else { "<dict>\n" });
                    }
                    m.emitted = true;
                    run.tasks.push(Task::Members(m));
                    run.tasks.push(Task::RestoreSource(previous_source));
                    run.tasks.push(Task::Text("\n".into()));
                    run.value(value, depth, false, false, false, path);
                    if element {
                        run.append_value(&run.indentation(depth));
                    } else {
                        let key = if descriptor.kind == CollectionMemberKind::Property {
                            Value::String(output_name.unwrap_or_else(|| match descriptor.key {
                                Value::String(name) => name,
                                _ => unreachable!("property string key"),
                            }))
                        } else {
                            key
                        };
                        run.tasks.push(Task::PlistKey { key, depth });
                    }
                } else if run.format == Format::Yaml {
                    let element = descriptor.kind == CollectionMemberKind::Element;
                    if descriptor.kind == CollectionMemberKind::Entry
                        && !self.yaml_scalar_key(&key)?
                    {
                        return self.yaml_non_scalar_key(m.receiver, &key);
                    }
                    run.yaml.as_mut().unwrap().member(element, !m.emitted);
                    m.emitted = true;
                    run.tasks.push(Task::Members(m));
                    run.tasks.push(Task::RestoreSource(previous_source));
                    run.value(value, depth, false, false, false, path);
                    if !element {
                        run.tasks.push(Task::Text(":".into()));
                        let key = if descriptor.kind == CollectionMemberKind::Property {
                            Value::String(output_name.unwrap_or_else(|| match descriptor.key {
                                Value::String(name) => name,
                                _ => unreachable!("property string key"),
                            }))
                        } else {
                            key
                        };
                        run.tasks.push(Task::YamlKey(key));
                    }
                } else if run.format == Format::Properties {
                    run.tasks.push(Task::Members(m));
                    run.tasks.push(Task::RestoreSource(previous_source));
                    // Root Dynamic elements are converted, then deliberately ignored.
                    if descriptor.kind == CollectionMemberKind::Element {
                        return Ok(false);
                    }
                    // A keyed Dynamic with elements is wrapped as a Listing and
                    // converted once more at this same path, as in pinned Java.
                    if let Value::Object(id) = &value
                        && self.heap.get(*id)?.collection_kind == CollectionKind::Dynamic
                        && collections::typed_class(&self.heap, *id)?.is_none()
                        && !self.heap.get(*id)?.is_module
                        && self.heap.get(*id)?.length != 0
                    {
                        // A VmListing view keeps the Dynamic as its member parent,
                        // but has Listing identity for converter/class dispatch.
                        let class = self.canonical_builtin_class(classes::BuiltinClass::Listing);
                        let object = Object {
                            parent: Some(*id),
                            instance_class: Some(class),
                            collection_kind: CollectionKind::Listing,
                            length: self.heap.get(*id)?.length,
                            ..Default::default()
                        };
                        let wrapper = self.heap.allocate(object);
                        run.tasks.push(Task::Visit {
                            value: Value::Object(wrapper),
                            depth,
                            standalone: false,
                            root: false,
                            string_element: false,
                            path,
                            converted: false,
                        });
                    } else {
                        run.value(value, depth, false, false, false, path);
                    }
                } else if run.format == Format::Jsonnet {
                    let element = descriptor.kind == CollectionMemberKind::Element;
                    if m.emitted {
                        run.append(",");
                    }
                    if !run.indent.is_empty() {
                        run.json_newline(depth);
                    } else if m.emitted || !element {
                        run.append(" ");
                    }
                    if descriptor.kind == CollectionMemberKind::Entry
                        && !matches!(&key, Value::String(_))
                        && !matches!(&key, Value::Object(id) if self.is_render_directive(*id)?)
                    {
                        return self.jsonnet_non_string_key(m.receiver, &key);
                    }
                    m.emitted = true;
                    run.tasks.push(Task::Members(m));
                    run.tasks.push(Task::RestoreSource(previous_source));
                    run.value(value, depth, false, false, false, path);
                    if !element {
                        run.tasks.push(Task::Text(": ".into()));
                        let key = if descriptor.kind == CollectionMemberKind::Property {
                            Value::String(output_name.unwrap_or_else(|| match descriptor.key {
                                Value::String(name) => name,
                                _ => unreachable!("property string key"),
                            }))
                        } else {
                            key
                        };
                        run.tasks.push(Task::JsonnetKey { key, depth });
                    }
                } else if run.format == Format::Json {
                    if m.emitted {
                        run.append(",");
                    }
                    run.json_newline(depth);
                    m.emitted = true;
                    let kind = descriptor.kind;
                    if kind == CollectionMemberKind::Entry
                        && !matches!(key, Value::String(_))
                        && !matches!(key, Value::Object(id) if self.is_render_directive(id)?)
                    {
                        return self.json_non_string_key(m.receiver, &key);
                    }
                    run.tasks.push(Task::Members(m));
                    run.tasks.push(Task::RestoreSource(previous_source));
                    run.value(value, depth, false, false, false, path);
                    if kind != CollectionMemberKind::Element {
                        run.tasks.push(Task::Text(
                            if run.indent.is_empty() { ":" } else { ": " }.into(),
                        ));
                        if kind == CollectionMemberKind::Property {
                            run.tasks
                                .push(Task::Text(json_string(&output_name.unwrap_or_else(|| {
                                    if let Value::String(name) = &descriptor.key {
                                        name.clone()
                                    } else {
                                        unreachable!("property string key")
                                    }
                                }))));
                        } else {
                            run.value(key, depth, false, false, false, Vec::new());
                        }
                    }
                } else if m.strict {
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
                            if !run.output_empty() {
                                run.append("\n");
                                run.append_value(&run.indentation(depth));
                            }
                            let name = output_name.unwrap_or_else(|| {
                                if let Value::String(name) = &descriptor.key {
                                    name.clone()
                                } else {
                                    unreachable!("property string key")
                                }
                            });
                            if name
                                .as_str()
                                .is_some_and(crate::parser::is_regular_identifier)
                            {
                                run.append_value(&name);
                            } else {
                                run.append("`");
                                run.append_value(&name);
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
                            run.append_value(&run.indentation(depth));
                            m.emitted = true;
                            run.tasks.push(Task::Members(m));
                            run.tasks.push(Task::RestoreSource(previous_source));
                            run.value(value, depth, true, false, true, path);
                        }
                        CollectionMemberKind::Entry => {
                            run.append("\n");
                            run.append_value(&run.indentation(depth));
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
                array,
                stream,
            } => {
                if run.format == Format::Protobuf {
                    self.protobuf_close(run, receiver)?;
                } else if run.format == Format::Yaml {
                    if !stream {
                        run.yaml.as_mut().unwrap().close(!emitted);
                    }
                } else if run.format == Format::Plist {
                    // Java always appends the parent indent, including an empty
                    // nested container whose element/key prefix is already present.
                    run.append_value(&run.indentation(depth - 1));
                    run.append(match (array, emitted) {
                        (true, false) => "<array/>",
                        (true, true) => "</array>",
                        (false, false) => "<dict/>",
                        (false, true) => "</dict>",
                    });
                } else if matches!(
                    run.format,
                    Format::Properties | Format::Xml | Format::PklBinary
                ) {
                    // Flattened output has no container delimiters.
                } else if run.format == Format::Jsonnet {
                    if emitted && !run.indent.is_empty() {
                        run.append(",");
                        run.json_newline(depth - 1);
                    } else if emitted && !array {
                        run.append(" ");
                    }
                    run.append(if array { "]" } else { "}" });
                } else if run.format == Format::Json {
                    if emitted {
                        run.json_newline(depth - 1);
                    }
                    run.append(if array { "]" } else { "}" });
                } else if strict {
                    run.append(")");
                } else if !document_root {
                    if emitted {
                        run.append("\n");
                        run.append_value(&run.indentation(depth - 1));
                    }
                    run.append("}");
                }
                run.active.remove(&receiver);
            }
        }
        Ok(false)
    }
    fn yaml_scalar_key(&self, key: &Value) -> Result<bool, RuntimeError> {
        Ok(match key {
            Value::Null
            | Value::NullWithDefault(_)
            | Value::String(_)
            | Value::Bool(_)
            | Value::Int(_)
            | Value::Float(_) => true,
            Value::Object(id) => self.is_render_directive(*id)?,
            _ => false,
        })
    }
    fn yaml_non_scalar_key(&self, receiver: ObjectId, key: &Value) -> Result<bool, RuntimeError> {
        let map = self.heap.get(receiver)?.collection_kind == CollectionKind::Map;
        let render = |value: &Value| {
            crate::diagnostic_values::render(value, &self.heap, &self.program.module_name, 80)
        };
        Err(RuntimeError::new(
            ErrorKind::Type,
            PklString::from(if map {
                "Cannot render map with non-scalar key as YAML.\nMap: "
            } else {
                "Cannot render object with non-scalar key as YAML.\nObject: "
            })
            .concat(&render(&Value::Object(receiver))?)
            .concat(&if map { "\nKey: " } else { "\nKey   : " }.into())
            .concat(&render(key)?),
        ))
    }
    fn pcf_property_annotations(
        &self,
        receiver: ObjectId,
        name: &PklString,
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
                    .find(|m| !m.local && !m.method && !m.entry && &m.name == name)
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
    fn properties_document_value(&self, value: &Value) -> Result<bool, RuntimeError> {
        Ok(
            matches!(value, Value::Object(id) if self.heap.get(*id)?.reference.is_none()
            && self.heap.get(*id)?.pair.is_none()
            && (collections::typed_class(&self.heap, *id)?.is_some()
                || matches!(self.heap.get(*id)?.collection_kind,
                    CollectionKind::Dynamic | CollectionKind::Mapping | CollectionKind::Map))),
        )
    }
    fn pcf_document_value(&self, value: &Value) -> Result<bool, RuntimeError> {
        Ok(
            matches!(value,Value::Object(id) if self.heap.get(*id)?.reference.is_none() && self.heap.get(*id)?.pair.is_none() && (collections::typed_class(&self.heap,*id)?.is_some() || self.heap.get(*id)?.collection_kind==CollectionKind::Dynamic)),
        )
    }
    fn pcf_unrenderable(&self, run: &RenderRun, value: &Value) -> Result<bool, RuntimeError> {
        self.render_unrenderable_at(run, value, run.source.as_ref())
    }
    fn render_unrenderable_at(
        &self,
        run: &RenderRun,
        value: &Value,
        source: Option<&RuntimeFrame>,
    ) -> Result<bool, RuntimeError> {
        let ty = crate::diagnostic_values::type_name(value, &self.heap, &self.program.module_name)?;
        let rendered =
            crate::diagnostic_values::render(value, &self.heap, &self.program.module_name, 80)?;
        let mut error = RuntimeError::new(
            ErrorKind::Type,
            PklString::from(format!(
                "Cannot render value of type `{ty}` as {}.\nValue: ",
                run.format.name()
            ))
            .concat(&rendered),
        );
        // PList primitive visitors use cannotRenderType directly. Object,
        // Null, Class, TypeAlias and Function errors retain the shared hint.
        if !(run.format == Format::Plist
            && matches!(
                value,
                Value::Duration(_) | Value::DataSize(_) | Value::Regex(_) | Value::IntSeq(_)
            ))
        {
            error.set_template_hint("Consider adding a converter to `output.converters`.");
            error.frames.extend(source.cloned());
        }
        Err(error)
    }
    fn is_render_directive(&self, id: ObjectId) -> Result<bool, RuntimeError> {
        Ok(collections::typed_class(&self.heap, id)?
            .is_some_and(|class| class.id == output::RENDER_DIRECTIVE))
    }
    fn plist_non_string_key(&self, receiver: ObjectId, key: &Value) -> Result<bool, RuntimeError> {
        let map = self.heap.get(receiver)?.collection_kind == CollectionKind::Map;
        let render = |value: &Value| {
            crate::diagnostic_values::render(value, &self.heap, &self.program.module_name, 80)
        };
        Err(RuntimeError::new(
            ErrorKind::Type,
            PklString::from(if map {
                "Cannot render map with non-string key as XML property list.\nMap: "
            } else {
                "Cannot render object with non-string key as XML property list.\nObject: "
            })
            .concat(&render(&Value::Object(receiver))?)
            .concat(&if map { "\nKey: " } else { "\nKey   : " }.into())
            .concat(&render(key)?),
        ))
    }
    fn jsonnet_wrapper(&self, receiver: ObjectId) -> Result<Option<bool>, RuntimeError> {
        let Some(actual) = self.heap.object_class(receiver)? else {
            return Ok(None);
        };
        let Some(modules::CachedModule::Value(module)) = self.modules_stdlib.get("pkl:jsonnet")
        else {
            return Err(invalid("Jsonnet renderer without its canonical module"));
        };
        let module = self.heap.get(*module)?;
        for (name, import_str) in [("ImportStr", true), ("ExtVar", false)] {
            let key = CacheKey {
                owner: None,
                entry: false,
                method: false,
                name: name.into(),
            };
            let Some(LazyState::Completed(Value::Class(expected))) = module.cache.get(&key) else {
                return Err(invalid("Jsonnet module class not initialized"));
            };
            if actual == *expected {
                return Ok(Some(import_str));
            }
        }
        Ok(None)
    }
    fn jsonnet_non_string_key(
        &self,
        receiver: ObjectId,
        key: &Value,
    ) -> Result<bool, RuntimeError> {
        let map = self.heap.get(receiver)?.collection_kind == CollectionKind::Map;
        let render = |value: &Value| {
            crate::diagnostic_values::render(value, &self.heap, &self.program.module_name, 80)
        };
        Err(RuntimeError::new(
            ErrorKind::Type,
            PklString::from(if map {
                "Cannot render map with non-string key as Jsonnet.\nMap: "
            } else {
                "Cannot render object with non-string key as Jsonnet.\nObject: "
            })
            .concat(&render(&Value::Object(receiver))?)
            .concat(&if map { "\nKey: " } else { "\nKey   : " }.into())
            .concat(&render(key)?),
        ))
    }
    fn json_non_string_key(&self, receiver: ObjectId, key: &Value) -> Result<bool, RuntimeError> {
        let map = self.heap.get(receiver)?.collection_kind == CollectionKind::Map;
        let render = |value: &Value| {
            crate::diagnostic_values::render(value, &self.heap, &self.program.module_name, 80)
        };
        Err(RuntimeError::new(
            ErrorKind::Type,
            PklString::from(if map {
                "Cannot render map with non-string key as JSON.\nMap: "
            } else {
                "Cannot render object with non-string key as JSON.\nObject: "
            })
            .concat(&render(&Value::Object(receiver))?)
            .concat(&if map { "\nKey: " } else { "\nKey   : " }.into())
            .concat(&render(key)?),
        ))
    }
}

/// Reference identity is independent of Pkl payload equality and backing buffers.
fn same_render_reference(left: &Value, right: &Value) -> Option<bool> {
    Some(left.reference()? == right.reference()?)
}

/// The pinned JsonEscaper operates on Java chars, including unpaired surrogates.
fn json_string(value: &PklString) -> PklString {
    let mut units = vec![b'"' as u16];
    for unit in value.encode_utf16() {
        let replacement = match unit {
            0x22 => Some("\\\""),
            0x5c => Some("\\\\"),
            8 => Some("\\b"),
            9 => Some("\\t"),
            10 => Some("\\n"),
            12 => Some("\\f"),
            13 => Some("\\r"),
            0x2028 => Some("\\u2028"),
            0x2029 => Some("\\u2029"),
            _ => None,
        };
        if let Some(text) = replacement {
            units.extend(text.encode_utf16());
        } else if unit < 0x20 {
            units.extend(format!("\\u{unit:04x}").encode_utf16());
        } else {
            units.push(unit);
        }
    }
    units.push(b'"' as u16);
    PklString::from_utf16(units)
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

/// Pinned PropertiesUtils operates on UTF-16 chars, including lone surrogates.
fn properties_string(value: &PklString, key: bool, restrict: bool) -> PklString {
    let mut out = Vec::new();
    for (index, unit) in value.encode_utf16().enumerate() {
        let escape = match unit {
            9 => Some(b't'),
            10 => Some(b'n'),
            12 => Some(b'f'),
            13 => Some(b'r'),
            32 if key || index == 0 => Some(b' '),
            33 | 35 | 58 | 61 | 92 => Some(unit as u8),
            _ => None,
        };
        if let Some(escaped) = escape {
            out.extend([92, escaped as u16]);
        } else if restrict && !(32..=126).contains(&unit) {
            out.extend([92, b'u' as u16]);
            for shift in [12, 8, 4, 0] {
                out.push(b"0123456789ABCDEF"[((unit >> shift) & 15) as usize] as u16);
            }
        } else {
            out.push(unit);
        }
    }
    PklString::from_utf16(out)
}

fn yaml_scalar(value: &Value) -> Result<PklString, RuntimeError> {
    Ok(match value {
        Value::Null | Value::NullWithDefault(_) => "null".into(),
        Value::Float(value) if value.is_nan() => ".NaN".into(),
        Value::Float(value) if **value == f64::INFINITY => ".Inf".into(),
        Value::Float(value) if **value == f64::NEG_INFINITY => "-.Inf".into(),
        _ => scalar_string(value)?,
    })
}

include!("xml_rendering.rs");

include!("pkl_binary_render.rs");

include!("protobuf_types.rs");
include!("protobuf_rendering.rs");
