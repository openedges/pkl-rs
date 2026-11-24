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
    members: Box<Members>,
    descriptor: CollectionMember,
    key: Value,
    value: Value,
    step: usize,
    output_name: Option<PklString>,
    previous_source: Option<Box<RuntimeFrame>>,
}
struct PropertyRun {
    member: Box<MemberRun>,
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
    Xml(Box<XmlTask>),
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
    PcfDirective {
        receiver: ObjectId,
        depth: usize,
        path: Vec<PathPart>,
        stage: u8,
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
    Member(Box<MemberRun>),
    Property(Box<PropertyRun>),
    Members(Box<Members>),
    FastList {
        receiver: ObjectId,
        index: usize,
        depth: usize,
        path: Vec<PathPart>,
    },
    ForcePlainObject {
        receiver: ObjectId,
        index: usize,
    },
    Text(PklString),
    RestoreSource(Option<Box<RuntimeFrame>>),
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
    PcfDirective {
        receiver: ObjectId,
        path: Vec<PathPart>,
        stage: u8,
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
    has_value_converters: bool,
    converters: Converters,
    source: Option<RuntimeFrame>,
    source_overrides: crate::heap::FastMap<
        usize,
        (
            Arc<crate::value::RuntimeSource>,
            Arc<crate::value::RuntimeSource>,
        ),
    >,
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
            | Await::PcfDirective { receiver, path, .. }
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
                Task::PropertiesDirective { receiver, path }
                | Task::PcfDirective { receiver, path, .. } => {
                    roots.push(Value::Object(*receiver));
                    path_roots(path, roots);
                }
                Task::Visit { value, path, .. } => {
                    roots.push(value.clone());
                    path_roots(path, roots);
                }
                Task::OpenObject { receiver, path, .. } | Task::FastList { receiver, path, .. } => {
                    roots.push(Value::Object(*receiver));
                    path_roots(path, roots);
                }
                Task::Close { receiver, .. } => roots.push(Value::Object(*receiver)),
                Task::ForcePlainObject { receiver, .. } => roots.push(Value::Object(*receiver)),
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
            self.output.push_str("\n");
            for _ in 0..depth {
                self.output.push_value(&self.indent);
            }
        }
    }
    fn append_json_string(&mut self, value: &PklString) {
        if let Some(s) = value.as_str() {
            self.output.push('"');
            let bytes = s.as_bytes();
            let mut start = 0usize;
            let mut i = 0usize;
            while i < bytes.len() {
                let b = bytes[i];
                if b < 0x20 || b == b'"' || b == b'\\' {
                    if i > start {
                        self.output.push_str(&s[start..i]);
                    }
                    match b {
                        b'"' => self.output.push_str("\\\""),
                        b'\\' => self.output.push_str("\\\\"),
                        8 => self.output.push_str("\\b"),
                        9 => self.output.push_str("\\t"),
                        10 => self.output.push_str("\\n"),
                        12 => self.output.push_str("\\f"),
                        13 => self.output.push_str("\\r"),
                        _ => {
                            const HEX: &[u8; 16] = b"0123456789abcdef";
                            let esc = [
                                b'\\',
                                b'u',
                                b'0',
                                b'0',
                                HEX[(b >> 4) as usize],
                                HEX[(b & 0xf) as usize],
                            ];
                            self.output.push_str(std::str::from_utf8(&esc).unwrap());
                        }
                    }
                    i += 1;
                    start = i;
                } else if b == 0xe2
                    && i + 2 < bytes.len()
                    && bytes[i + 1] == 0x80
                    && (bytes[i + 2] == 0xa8 || bytes[i + 2] == 0xa9)
                {
                    if i > start {
                        self.output.push_str(&s[start..i]);
                    }
                    self.output.push_str(if bytes[i + 2] == 0xa8 {
                        "\\u2028"
                    } else {
                        "\\u2029"
                    });
                    i += 3;
                    start = i;
                } else {
                    i += 1;
                }
            }
            if start < bytes.len() {
                self.output.push_str(&s[start..]);
            }
            self.output.push('"');
        } else {
            let rendered = json_string(value);
            self.output.push_value(&rendered);
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
            if same_render_reference(original, &run.root) {
                continue;
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
                    | Await::PcfDirective { receiver, path, .. }
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
                    let target_task = match run.tasks.last() {
                        Some(Task::ForcePlainObject { .. }) => run
                            .tasks
                            .len()
                            .checked_sub(2)
                            .and_then(|i| run.tasks.get(i)),
                        other => other,
                    };
                    let (receiver, path) = match target_task {
                        Some(Task::OpenObject { receiver, path, .. }) => (receiver, path),
                        Some(Task::Xml(task)) => match task.as_ref() {
                            XmlTask {
                                value: Value::Object(receiver),
                                path,
                                action: XmlAction::CheckedObject { .. },
                                ..
                            } => (receiver, path),
                            _ => continue,
                        },
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
        let root = self.pop()?.boxed();
        let receiver = if matches!(
            format,
            Format::Jsonnet | Format::Xml | Format::PklBinary | Format::Protobuf
        ) {
            self.this_receiver()?.as_object()?
        } else {
            self.scope(0)?.receiver
        };
        if format != Format::Protobuf && self.is_unmodified_default_renderer(receiver, format) {
            if let Value::Object(root_id) = root
                && (format != Format::Properties || document)
            {
                if self.heap.get_fast(root_id).is_some_and(|o| {
                    o.collection_kind == CollectionKind::Dynamic
                        && !o.forced
                        && o.cache_slots.len() < o.members.len()
                }) && self.is_plain_shallow_forceable(root_id)
                {
                    let _ = self.try_force_plain_object_sync(root_id)?;
                } else {
                    let _ = self.try_force_plain_typed_sync(root_id, 0)?;
                }
            }
            if let Some(rendered) = self.try_fast_render_default_document(format, document, &root) {
                self.stack.push(rendered);
                return Ok(());
            }
        }
        self.frames.last_mut().expect("renderer frame").pcf_render = Some(Box::new(RenderRun {
            receiver,
            root,
            document_original: None,
            document,
            format,
            option: 0,
            options: Vec::new(),
            await_: None,
            tasks: Vec::with_capacity(64),
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
            has_value_converters: true,
            converters: Converters::default(),
            source: None,
            source_overrides: crate::heap::FastMap::default(),
        }));
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
            self.advance_pcf_await(run, await_)?;
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
            if self.advance_pcf_start(run, names.len())? {
                return Ok(true);
            }
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
            Task::Protobuf(task) => self.advance_protobuf(run, task),
            Task::BinaryArray(length) => {
                run.binary.array(length)?;
                Ok(false)
            }
            Task::Xml(task) => self.advance_xml(run, *task),
            Task::RestoreSource(source) => {
                run.source = source.map(|s| *s);
                Ok(false)
            }
            Task::Text(text) => {
                run.append_value(&text);
                Ok(false)
            }
            Task::JsonnetKey { .. }
            | Task::PlistKey { .. }
            | Task::YamlKey(..)
            | Task::PropertiesKey { .. }
            | Task::PropertiesDirective { .. }
            | Task::PcfDirective { .. } => self.advance_pcf_keys_and_directives(run, task),
            Task::Visit { .. } => self.advance_pcf_visit(run, task),
            Task::OpenObject { .. } | Task::Close { .. } => {
                self.advance_pcf_open_or_close(run, task)
            }
            Task::Members(m) => self.advance_pcf_members(run, m),
            Task::FastList {
                receiver,
                index,
                depth,
                path,
            } => self.advance_fast_list(run, receiver, index, depth, path),
            Task::ForcePlainObject { receiver, index } => {
                self.advance_force_plain_object(run, receiver, index)
            }
            Task::Member(member) => self.advance_pcf_member(run, member),
            Task::Property(p) => self.advance_pcf_property(run, p),
        }
    }

    #[cfg_attr(target_arch = "wasm32", inline(never))]
    fn advance_pcf_await(
        &mut self,
        run: &mut RenderRun,
        await_: Await,
    ) -> Result<(), RuntimeError> {
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
                let converted = self.pop()?.boxed();
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
            Await::PcfDirective {
                receiver, stage, ..
            } => {
                let value = self.pop()?;
                match stage {
                    0 => {
                        if let Value::String(text) = value {
                            run.append_value(&text);
                        }
                    }
                    1 => {
                        let Some(Task::PcfDirective { depth, path, .. }) = run.tasks.last() else {
                            return Err(invalid("pcf directive continuation"));
                        };
                        let (depth, path) = (*depth, path.clone());
                        run.value(value, depth, false, false, false, path);
                    }
                    2 => {
                        if let Value::String(text) = value {
                            run.append_value(&text);
                        }
                        run.active.remove(&receiver);
                    }
                    _ => return Err(invalid("pcf directive stage")),
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
        Ok(())
    }

    #[cfg_attr(target_arch = "wasm32", inline(never))]
    fn advance_pcf_start(
        &mut self,
        run: &mut RenderRun,
        names_len: usize,
    ) -> Result<bool, RuntimeError> {
        run.started = true;
        if run.format == Format::PklBinary {
            // BytesRenderer has no text formatting or omission options.
        } else if run.format == Format::Yaml {
            let (Value::String(mode), Value::Int(width), Value::Bool(omit), Value::Bool(stream)) = (
                &run.options[0],
                &run.options[1],
                &run.options[2],
                &run.options[3],
            ) else {
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
            (&run.options[names_len - 2], &run.options[names_len - 1])
        else {
            return Err(invalid("renderer mappings"));
        };
        run.has_value_converters = !collections::ordered_members(&self.heap, *mapping)?.is_empty();
        run.converters = Converters::load(&self.heap, *mapping, *transformers)?;
        if !run.has_value_converters && self.try_fast_render_document(run) {
            return Ok(true);
        }
        run.tasks.push(Task::Visit {
            value: run.root.clone(),
            depth: 0,
            standalone: false,
            root: true,
            string_element: false,
            path: vec![PathPart::Root],
            converted: false,
        });
        Ok(false)
    }

    #[cfg_attr(target_arch = "wasm32", inline(never))]
    fn advance_pcf_keys_and_directives(
        &mut self,
        run: &mut RenderRun,
        task: Task,
    ) -> Result<bool, RuntimeError> {
        match task {
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
            Task::PcfDirective {
                receiver,
                depth,
                path,
                stage,
            } => match stage {
                1 => {
                    run.tasks.push(Task::PcfDirective {
                        receiver,
                        depth,
                        path: path.clone(),
                        stage: 2,
                    });
                    run.await_ = Some(Await::PcfDirective {
                        receiver,
                        path,
                        stage: 1,
                    });
                    self.force(receiver, receiver, "value".into(), false, true, false)?;
                }
                2 => {
                    run.await_ = Some(Await::PcfDirective {
                        receiver,
                        path,
                        stage: 2,
                    });
                    self.force(receiver, receiver, "after".into(), false, true, false)?;
                }
                _ => return Err(invalid("pcf directive task stage")),
            },
            _ => unreachable!("key/directive task"),
        }
        Ok(false)
    }

    #[cfg_attr(target_arch = "wasm32", inline(never))]
    fn advance_pcf_visit(&mut self, run: &mut RenderRun, task: Task) -> Result<bool, RuntimeError> {
        let Task::Visit {
            value,
            depth,
            standalone,
            root,
            string_element,
            path,
            converted,
        } = task
        else {
            unreachable!("visit task")
        };
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
                            CollectionKind::Listing | CollectionKind::List | CollectionKind::Set
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
                        return Err(RuntimeError::with_program_values(
                            ErrorKind::Type,
                            format!(
                                "The top-level value of a YAML stream must have type `Listing`, `List`, or `Set`, but got type `{ty}`."
                            ),
                            &[("Value", text)],
                        ));
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
                    self.begin_shallow_force_for_render(run, id)?;
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
                    return Err(RuntimeError::with_program_values(
                        ErrorKind::Type,
                        format!(
                            "The top-level value of an XML property list must have type `Typed`, `Dynamic`, `Listing`, `Mapping`, `List`, `Set`, or `Map`, but got type `{ty}`."
                        ),
                        &[("Value", text)],
                    ));
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
                    return Err(RuntimeError::with_program_values(
                        ErrorKind::Type,
                        format!(
                            "The top-level value of a Java properties file must have type `Typed`, `Dynamic`, `Mapping`, or `Map`, but got type `{ty}`."
                        ),
                        &[("Value", text)],
                    ));
                }
                if !run.document && valid && !directive {
                    return self.pcf_unrenderable(run, &value);
                }
                run.properties_document = run.document && !directive;
            }
            if run.format == Format::Pcf && run.document && !self.pcf_document_value(&value)? {
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
                return Err(RuntimeError::with_program_values(
                    ErrorKind::Type,
                    format!(
                        "The top-level value of a Pcf document must have type `Typed` or `Dynamic`, but got type `{ty}`."
                    ),
                    &[("Value", text)],
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
            Value::String(value) => {
                if run.format == Format::Json {
                    run.append_json_string(&value);
                } else {
                    run.append_value(&if run.format == Format::Jsonnet {
                        super::jsonnet::string(&value, &run.indent, &run.indentation(depth))
                    } else {
                        super::pcf_format::string(
                            &value,
                            &run.indentation(depth + usize::from(!string_element)),
                            run.custom,
                        )
                    });
                }
            }
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
                if run.format == Format::Pcf && self.is_pcf_render_directive(id)? {
                    if !run.active.insert(id) {
                        return Err(RuntimeError::new(
                            ErrorKind::Cycle,
                            "cyclic value rendering",
                        ));
                    }
                    if standalone {
                        run.append("new ");
                    }
                    run.tasks.push(Task::PcfDirective {
                        receiver: id,
                        depth,
                        path: path.clone(),
                        stage: 1,
                    });
                    run.await_ = Some(Await::PcfDirective {
                        receiver: id,
                        path,
                        stage: 0,
                    });
                    self.force(id, id, "before".into(), false, true, false)?;
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
                    if !run.has_value_converters
                        && kind == CollectionKind::List
                        && matches!(
                            run.format,
                            Format::Pcf
                                | Format::Json
                                | Format::Jsonnet
                                | Format::Yaml
                                | Format::Plist
                        )
                        && self
                            .heap
                            .get_fast(id)
                            .is_some_and(|obj| obj.parent.is_none() && obj.list_elements.is_some())
                        && (run.format == Format::Yaml
                            && run.yaml.as_ref().is_some_and(super::yaml::Emitter::is_utf8)
                            || run.format != Format::Yaml && run.indent.as_str().is_some())
                    {
                        if self.try_fast_render_open_object(run, id, depth, standalone, root, &path)
                        {
                            run.active.remove(&id);
                            return Ok(false);
                        }
                        if run.format == Format::Yaml {
                            run.yaml.as_mut().unwrap().start(true);
                        } else if run.format == Format::Plist {
                            run.append("<array>\n");
                        } else if matches!(run.format, Format::Json | Format::Jsonnet) {
                            run.append("[");
                        } else {
                            run.append("List(");
                        }
                        run.tasks.push(Task::FastList {
                            receiver: id,
                            index: 0,
                            depth,
                            path,
                        });
                        return Ok(false);
                    }
                    if run.format == Format::Yaml {
                        run.yaml.as_mut().unwrap().start(array);
                    } else if !matches!(
                        run.format,
                        Format::Properties | Format::Plist | Format::Xml
                    ) {
                        run.append(if matches!(run.format, Format::Json | Format::Jsonnet) {
                            if array { "[" } else { "{" }
                        } else {
                            match kind {
                                CollectionKind::List => "List(",
                                CollectionKind::Map => "Map(",
                                _ => "Set(",
                            }
                        });
                    }
                    let members = collections::ordered_members(&self.heap, id)?
                        .into_iter()
                        .map(|descriptor| collections::RenderMember {
                            descriptor,
                            skip: false,
                            code: None,
                        })
                        .collect();
                    run.tasks.push(Task::Members(Box::new(Members {
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
                    })));
                } else {
                    if !run.has_value_converters
                        && run.format != Format::Protobuf
                        && self.try_fast_render_open_object(run, id, depth, standalone, root, &path)
                    {
                        run.active.remove(&id);
                        return Ok(false);
                    }
                    run.tasks.push(Task::OpenObject {
                        receiver: id,
                        depth,
                        standalone,
                        root,
                        path,
                        stream: false,
                    });
                    self.begin_shallow_force_for_render(run, id)?;
                }
            }
            value => return self.pcf_unrenderable(run, &value),
        }
        Ok(false)
    }

    #[cfg_attr(target_arch = "wasm32", inline(never))]
    fn advance_pcf_open_or_close(
        &mut self,
        run: &mut RenderRun,
        task: Task,
    ) -> Result<bool, RuntimeError> {
        match task {
            Task::OpenObject {
                receiver,
                depth,
                standalone,
                root,
                path,
                stream,
            } => {
                if !run.has_value_converters
                    && !stream
                    && run.format != Format::Protobuf
                    && self
                        .try_fast_render_open_object(run, receiver, depth, standalone, root, &path)
                {
                    run.active.remove(&receiver);
                    return Ok(false);
                }
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
                run.tasks.push(Task::Members(Box::new(Members {
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
                })));
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
            _ => unreachable!("open/close task"),
        }
        Ok(false)
    }

    #[cfg_attr(target_arch = "wasm32", inline(never))]
    fn advance_pcf_members(
        &mut self,
        run: &mut RenderRun,
        mut m: Box<Members>,
    ) -> Result<bool, RuntimeError> {
        let needs_key_dedup = !(matches!(
            run.format,
            Format::Properties
                | Format::Jsonnet
                | Format::Yaml
                | Format::Plist
                | Format::Xml
                | Format::PklBinary
                | Format::Protobuf
        ) && m.strict);
        loop {
            let item = loop {
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
                let item = &m.members[m.next];
                let descriptor = &item.descriptor;
                if needs_key_dedup && !m.prepared && matches!(descriptor.key, Value::Object(_)) {
                    m.prepared = true;
                    let key = descriptor.key.clone();
                    run.tasks.push(Task::Members(m));
                    self.prepare_render_key(key);
                    return Ok(false);
                }
                if needs_key_dedup {
                    m.hash = self.java_hash(&descriptor.key)?;
                    let mut duplicate = false;
                    while m.compare < m.seen.len() {
                        let (seen, hash) = &m.seen[m.compare];
                        m.compare += 1;
                        if (seen.kind == CollectionMemberKind::Property)
                            != (descriptor.kind == CollectionMemberKind::Property)
                            || hash.zip(m.hash).is_some_and(|(a, b)| a != b)
                        {
                            continue;
                        }
                        if !matches!(
                            (&seen.key, &descriptor.key),
                            (Value::Object(_), _) | (_, Value::Object(_))
                        ) {
                            if equality::same_key(&self.heap, &seen.key, &descriptor.key) {
                                duplicate = true;
                                break;
                            }
                            continue;
                        }
                        let left = seen.key.clone();
                        let right = descriptor.key.clone();
                        run.tasks.push(Task::Members(m));
                        run.await_ = Some(Await::KeyEquality);
                        self.begin_equality(left, right, false);
                        return Ok(false);
                    }
                    if duplicate {
                        m.next += 1;
                        m.compare = 0;
                        m.prepared = false;
                        continue;
                    }
                    m.seen.push((descriptor.clone(), m.hash));
                }
                let skip = item.skip;
                let item = m.members[m.next].clone();
                m.next += 1;
                m.compare = 0;
                m.prepared = false;
                if skip {
                    continue;
                }
                break item;
            };
            let descriptor = &item.descriptor;
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
                return Err(RuntimeError::with_program_values(
                    ErrorKind::Type,
                    format!(
                        "Cannot render object with both elements and properties/entries as {}.",
                        run.format.name()
                    ),
                    &[("Object", rendered)],
                ));
            }
            let value = collections::cached_render_value(&self.heap, m.receiver, descriptor)?
                .ok_or_else(|| invalid("shallow-forced member is uncached"))?;
            if let Some(mode) = &m.xml {
                let action = match mode {
                    XmlMembers::Attributes { name } => XmlAction::Attribute {
                        name: name.clone(),
                        key: descriptor.key.clone(),
                    },
                    XmlMembers::Elements if descriptor.kind == CollectionMemberKind::Element => {
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
                run.tasks.push(Task::Xml(Box::new(task)));
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
            if !run.has_value_converters
                && (descriptor.kind != CollectionMemberKind::Property
                    || self
                        .pcf_property_annotations(m.receiver, &descriptor.name)?
                        .is_empty())
            {
                if (descriptor.kind == CollectionMemberKind::Property
                    || (run.format != Format::Pcf
                        && descriptor.kind == CollectionMemberKind::Entry))
                    && run.omit_null
                    && value.is_null()
                {
                    continue;
                }
                if run.format == Format::Json
                    && matches!(
                        &value,
                        Value::Null
                            | Value::NullWithDefault(_)
                            | Value::Bool(_)
                            | Value::Int(_)
                            | Value::String(_)
                    )
                    && (descriptor.kind != CollectionMemberKind::Entry
                        || matches!(descriptor.key, Value::String(_)))
                {
                    if m.emitted {
                        run.append(",");
                    }
                    run.json_newline(m.depth);
                    m.emitted = true;
                    match descriptor.kind {
                        CollectionMemberKind::Property | CollectionMemberKind::Entry => {
                            if let Value::String(ref k) = descriptor.key {
                                run.append_json_string(k);
                                run.output
                                    .push_str(if run.indent.is_empty() { ":" } else { ": " });
                            }
                        }
                        CollectionMemberKind::Element => {}
                    }
                    match &value {
                        Value::Null | Value::NullWithDefault(_) => run.append("null"),
                        Value::Bool(b) => run.append(if *b { "true" } else { "false" }),
                        Value::Int(n) => {
                            let mut buf = [0u8; 24];
                            run.output
                                .push_str(super::format_i64_buf(n.get(), &mut buf));
                        }
                        Value::String(s) => run.append_json_string(s),
                        _ => unreachable!(),
                    }
                    continue;
                }
            }
            let descriptor = descriptor.clone();
            let key = descriptor.key.clone();
            let previous_source = run.source.clone().map(Box::new);
            if let Some(metadata) = item.code.and_then(|code| self.program.metadata.get(code))
                && let Some(span) = metadata.member_span.or(metadata.declaration)
            {
                let renderer = &self.program.metadata[self.frames.last().unwrap().block];
                let target_module_name = renderer
                    .source
                    .as_ref()
                    .map_or(&self.program.module_name, |source| &source.module_name);
                let source = metadata
                    .source
                    .as_ref()
                    .or(self.context.primary_source.as_ref())
                    .map(|source| {
                        let key = Arc::as_ptr(source) as usize;
                        if source.module_name == *target_module_name {
                            source.clone()
                        } else if let Some((orig, cached)) = run.source_overrides.get(&key)
                            && Arc::ptr_eq(orig, source)
                            && cached.module_name == *target_module_name
                        {
                            cached.clone()
                        } else {
                            let overridden = Arc::new(crate::value::RuntimeSource {
                                module_name: target_module_name.clone(),
                                ..(**source).clone()
                            });
                            run.source_overrides
                                .insert(key, (source.clone(), overridden.clone()));
                            overridden
                        }
                    });
                run.source = Some(RuntimeFrame {
                    anonymous: false,
                    source,
                    span,
                    member: renderer.member.clone(),
                });
            }
            run.tasks.push(Task::Member(Box::new(MemberRun {
                members: m,
                descriptor,
                key,
                value,
                step: 0,
                output_name: None,
                previous_source,
            })));
            return Ok(false);
        }
    }

    #[cfg_attr(target_arch = "wasm32", inline(never))]
    fn advance_pcf_member(
        &mut self,
        run: &mut RenderRun,
        mut member: Box<MemberRun>,
    ) -> Result<bool, RuntimeError> {
        if member.step == 0 {
            member.step = 1;
            if member.descriptor.kind == CollectionMemberKind::Property {
                let annotations = self
                    .pcf_property_annotations(member.members.receiver, &member.descriptor.name)?;
                if !annotations.is_empty() {
                    let property =
                        self.make_pair(member.descriptor.key.clone(), member.value.clone());
                    run.tasks.push(Task::Property(Box::new(PropertyRun {
                        member,
                        annotations,
                        next: 0,
                        annotation: Value::Null,
                        property,
                        function: None,
                        step: 0,
                    })));
                    return Ok(false);
                }
            }
            if run.has_value_converters
                && member.descriptor.kind == CollectionMemberKind::Entry
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
            if run.has_value_converters
                && let Some(function) = run.converters.select(self, &member.value, &path)?
            {
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
        } = *member;
        if (descriptor.kind == CollectionMemberKind::Property
            || (run.format != Format::Pcf && descriptor.kind == CollectionMemberKind::Entry))
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
            if descriptor.kind == CollectionMemberKind::Entry && !self.yaml_scalar_key(&key)? {
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
                && !collections::is_typed_class(&self.heap, *id)?
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
            if kind == CollectionMemberKind::Property {
                let name = output_name.unwrap_or_else(|| {
                    if let Value::String(name) = &descriptor.key {
                        name.clone()
                    } else {
                        unreachable!("property string key")
                    }
                });
                run.append_json_string(&name);
                run.output
                    .push_str(if run.indent.is_empty() { ":" } else { ": " });
            } else if kind == CollectionMemberKind::Entry {
                if let Value::String(ref k) = key {
                    run.append_json_string(k);
                    run.output
                        .push_str(if run.indent.is_empty() { ":" } else { ": " });
                } else {
                    run.tasks.push(Task::Text(
                        if run.indent.is_empty() { ":" } else { ": " }.into(),
                    ));
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
        Ok(false)
    }

    #[cfg_attr(target_arch = "wasm32", inline(never))]
    fn advance_pcf_property(
        &mut self,
        run: &mut RenderRun,
        mut p: Box<PropertyRun>,
    ) -> Result<bool, RuntimeError> {
        if p.step == 0 {
            if p.next == p.annotations.len() {
                let Value::Object(id) = p.property else {
                    return Err(RuntimeError::new(
                        ErrorKind::Unsupported,
                        "Property conversion returned a non-Pair value (pinned Java internal error).",
                    ));
                };
                let pair = self.heap.get(id)?.pair.as_ref().ok_or_else(|| {
                    RuntimeError::new(
                        ErrorKind::Unsupported,
                        "Property conversion returned a non-Pair value (pinned Java internal error).",
                    )
                })?;
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
            self.invoke_function(function, vec![property, Value::Object(run.receiver)], false)?;
            return Ok(false);
        }
        p.step = 0;
        p.next += 1;
        run.tasks.push(Task::Property(p));
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
        let (headline, container_label) = if map {
            ("Cannot render map with non-scalar key as YAML.", "Map")
        } else {
            (
                "Cannot render object with non-scalar key as YAML.",
                "Object",
            )
        };
        Err(RuntimeError::with_program_values(
            ErrorKind::Type,
            headline,
            &[
                (container_label, render(&Value::Object(receiver))?),
                ("Key", render(key)?),
            ],
        ))
    }
    #[cfg_attr(target_arch = "wasm32", inline(never))]
    fn pcf_property_annotations(
        &self,
        receiver: ObjectId,
        name: &PklString,
    ) -> Result<Vec<ObjectId>, RuntimeError> {
        let mut has_any_annotations = false;
        let mut cur = Some(receiver);
        for _ in 0..64 {
            let Some(id) = cur else {
                break;
            };
            let object = self.heap.get(id)?;
            if object.class_definition.is_some() && !object.member_annotations.is_empty() {
                has_any_annotations = true;
                break;
            }
            cur = object.parent;
        }
        if cur.is_none() && !has_any_annotations {
            return Ok(Vec::new());
        }
        let mut definitions = Vec::new();
        let mut owner = Some(receiver);
        let mut seen = SmallObjectIdSet::new();
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
            && (collections::is_typed_class(&self.heap, *id)?
                || matches!(self.heap.get(*id)?.collection_kind,
                    CollectionKind::Dynamic | CollectionKind::Mapping | CollectionKind::Map))),
        )
    }
    fn pcf_document_value(&self, value: &Value) -> Result<bool, RuntimeError> {
        Ok(
            matches!(value,Value::Object(id) if self.heap.get(*id)?.reference.is_none() && self.heap.get(*id)?.pair.is_none() && (collections::is_typed_class(&self.heap,*id)? || self.heap.get(*id)?.collection_kind==CollectionKind::Dynamic)),
        )
    }
    fn pcf_unrenderable(&self, run: &RenderRun, value: &Value) -> Result<bool, RuntimeError> {
        self.render_unrenderable_at(run, value, run.source.as_ref())
    }
    #[cfg_attr(target_arch = "wasm32", inline(never))]
    fn render_unrenderable_at(
        &self,
        run: &RenderRun,
        value: &Value,
        source: Option<&RuntimeFrame>,
    ) -> Result<bool, RuntimeError> {
        let ty = crate::diagnostic_values::type_name(value, &self.heap, &self.program.module_name)?;
        let rendered =
            crate::diagnostic_values::render(value, &self.heap, &self.program.module_name, 80)?;
        let mut error = RuntimeError::with_program_values(
            ErrorKind::Type,
            format!(
                "Cannot render value of type `{ty}` as {}.",
                run.format.name()
            ),
            &[("Value", rendered)],
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
    pub(super) fn is_render_directive(&self, id: ObjectId) -> Result<bool, RuntimeError> {
        Ok(collections::typed_class_ref(&self.heap, id)?
            .is_some_and(|class| class.id == output::RENDER_DIRECTIVE))
    }
    fn is_pcf_render_directive(&self, id: ObjectId) -> Result<bool, RuntimeError> {
        Ok(collections::typed_class_ref(&self.heap, id)?
            .is_some_and(|class| class.id == output::PCF_RENDER_DIRECTIVE))
    }
    fn plist_non_string_key(&self, receiver: ObjectId, key: &Value) -> Result<bool, RuntimeError> {
        let map = self.heap.get(receiver)?.collection_kind == CollectionKind::Map;
        let render = |value: &Value| {
            crate::diagnostic_values::render(value, &self.heap, &self.program.module_name, 80)
        };
        let (headline, container_label) = if map {
            (
                "Cannot render map with non-string key as XML property list.",
                "Map",
            )
        } else {
            (
                "Cannot render object with non-string key as XML property list.",
                "Object",
            )
        };
        Err(RuntimeError::with_program_values(
            ErrorKind::Type,
            headline,
            &[
                (container_label, render(&Value::Object(receiver))?),
                ("Key", render(key)?),
            ],
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
            let Some(Value::Class(expected)) = module.cache_get_completed(&key) else {
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
        let (headline, container_label) = if map {
            ("Cannot render map with non-string key as Jsonnet.", "Map")
        } else {
            (
                "Cannot render object with non-string key as Jsonnet.",
                "Object",
            )
        };
        Err(RuntimeError::with_program_values(
            ErrorKind::Type,
            headline,
            &[
                (container_label, render(&Value::Object(receiver))?),
                ("Key", render(key)?),
            ],
        ))
    }
    fn json_non_string_key(&self, receiver: ObjectId, key: &Value) -> Result<bool, RuntimeError> {
        let map = self.heap.get(receiver)?.collection_kind == CollectionKind::Map;
        let render = |value: &Value| {
            crate::diagnostic_values::render(value, &self.heap, &self.program.module_name, 80)
        };
        let (headline, container_label) = if map {
            ("Cannot render map with non-string key as JSON.", "Map")
        } else {
            (
                "Cannot render object with non-string key as JSON.",
                "Object",
            )
        };
        Err(RuntimeError::with_program_values(
            ErrorKind::Type,
            headline,
            &[
                (container_label, render(&Value::Object(receiver))?),
                ("Key", render(key)?),
            ],
        ))
    }
}

/// Reference identity is independent of Pkl payload equality and backing buffers.
fn same_render_reference(left: &Value, right: &Value) -> bool {
    match (left.reference(), right.reference()) {
        (Some(left), Some(right)) => left == right,
        _ => left.clone().boxed().reference() == right.clone().boxed().reference(),
    }
}

/// The pinned JsonEscaper operates on Java chars, including unpaired surrogates.
#[cfg_attr(target_arch = "wasm32", inline(never))]
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
#[cfg_attr(target_arch = "wasm32", inline(never))]
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

#[cfg_attr(target_arch = "wasm32", inline(never))]
fn yaml_scalar(value: &Value) -> Result<PklString, RuntimeError> {
    Ok(match value {
        Value::Null | Value::NullWithDefault(_) => "null".into(),
        Value::Float(value) if value.is_nan() => ".NaN".into(),
        Value::Float(value) if **value == f64::INFINITY => ".Inf".into(),
        Value::Float(value) if **value == f64::NEG_INFINITY => "-.Inf".into(),
        _ => scalar_string(value)?,
    })
}

#[derive(Clone, Copy)]
enum FastMember<'a> {
    Property(&'a PklString, &'a Value),
    Entry(&'a Value, &'a Value),
    Element(i64, &'a Value),
}

enum FastObjectKind<'a> {
    StrictList(&'a [Value]),
    EmptyList,
    StrictSet(usize),
    StrictMap(usize),
    PlainDynamic {
        count: usize,
        has_elements: bool,
        has_keyed: bool,
    },
    PlainListing(usize),
    PlainMapping(usize),
    PlainModule {
        class: ObjectId,
        count: usize,
    },
    PlainTyped {
        count: usize,
    },
}

#[inline(always)]
fn fast_lookup_member_cached<'a>(
    obj: &'a Object,
    name: &PklString,
    entry: bool,
) -> Option<&'a Value> {
    if obj.cache_index.is_rehash_incomplete() {
        return None;
    }
    if obj.cache_map.is_empty() && !obj.has_evaluating_keys() {
        for slot in obj.cache_slots.iter().rev() {
            if slot.key.entry == entry
                && !slot.key.method
                && slot.key.owner.is_none()
                && slot.key.name == *name
            {
                return Some(&slot.value);
            }
        }
        return None;
    }
    let key = CacheKey {
        entry,
        owner: None,
        name: name.clone(),
        method: false,
    };
    obj.cache_get_completed(&key)
}

#[inline]
fn push_indent_str(buf: &mut String, indent: &str, depth: usize) {
    if depth == 0 || indent.is_empty() {
        return;
    }
    if indent == "  " {
        const SPACES: &str = "                                ";
        let mut n = depth * 2;
        while n > 0 {
            let chunk = n.min(SPACES.len());
            buf.push_str(&SPACES[..chunk]);
            n -= chunk;
        }
        return;
    }
    for _ in 0..depth {
        buf.push_str(indent);
    }
}

#[inline]
fn write_json_str(out: &mut String, s: &str) {
    out.push('"');
    let bytes = s.as_bytes();
    let mut start = 0;
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b >= 0x20 && b != b'"' && b != b'\\' && b != 0xe2 {
            i += 1;
            continue;
        }
        if b == 0xe2 {
            if i + 2 < bytes.len() && bytes[i + 1] == 0x80 && matches!(bytes[i + 2], 0xa8 | 0xa9) {
                out.push_str(&s[start..i]);
                out.push_str(if bytes[i + 2] == 0xa8 {
                    "\\u2028"
                } else {
                    "\\u2029"
                });
                i += 3;
                start = i;
            } else {
                i += 1;
            }
            continue;
        }
        out.push_str(&s[start..i]);
        match b {
            b'"' => out.push_str("\\\""),
            b'\\' => out.push_str("\\\\"),
            8 => out.push_str("\\b"),
            9 => out.push_str("\\t"),
            10 => out.push_str("\\n"),
            12 => out.push_str("\\f"),
            13 => out.push_str("\\r"),
            _ => {
                const HEX: &[u8; 16] = b"0123456789abcdef";
                out.push_str("\\u00");
                out.push(HEX[(b >> 4) as usize] as char);
                out.push(HEX[(b & 0xf) as usize] as char);
            }
        }
        i += 1;
        start = i;
    }
    out.push_str(&s[start..]);
    out.push('"');
}

fn write_properties_str(out: &mut String, s: &str, key: bool, restrict: bool) {
    let bytes = s.as_bytes();
    let fast_clean = if restrict {
        bytes.iter().enumerate().all(|(i, &b)| {
            (32..=126).contains(&b)
                && !matches!(b, b'!' | b'#' | b':' | b'=' | b'\\')
                && (b != b' ' || (!key && i > 0))
        })
    } else {
        bytes.iter().enumerate().all(|(i, &b)| {
            !matches!(b, 9 | 10 | 12 | 13 | b'!' | b'#' | b':' | b'=' | b'\\')
                && (b != b' ' || (!key && i > 0))
        })
    };
    if fast_clean {
        out.push_str(s);
        return;
    }
    if !restrict {
        for (i, ch) in s.chars().enumerate() {
            let esc = match ch {
                '\t' => Some('t'),
                '\n' => Some('n'),
                '\x0c' => Some('f'),
                '\r' => Some('r'),
                ' ' if key || i == 0 => Some(' '),
                '!' | '#' | ':' | '=' | '\\' => Some(ch),
                _ => None,
            };
            if let Some(e) = esc {
                out.push('\\');
                out.push(e);
            } else {
                out.push(ch);
            }
        }
    } else {
        for (index, unit) in s.encode_utf16().enumerate() {
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
                out.push('\\');
                out.push(escaped as char);
            } else if !(32..=126).contains(&unit) {
                const HEX: &[u8; 16] = b"0123456789ABCDEF";
                out.push_str("\\u");
                for shift in [12, 8, 4, 0] {
                    out.push(HEX[((unit >> shift) & 15) as usize] as char);
                }
            } else {
                out.push(unit as u8 as char);
            }
        }
    }
}

#[inline]
fn write_fast_scalar_text(out: &mut String, value: &Value) -> bool {
    match value {
        Value::Bool(v) => out.push_str(if *v { "true" } else { "false" }),
        Value::Int(v) => {
            let mut buf = [0u8; 24];
            out.push_str(super::format_i64_buf(v.get(), &mut buf));
        }
        Value::Float(v) => out.push_str(&crate::numeric::double_to_string(**v)),
        Value::String(s) => {
            let Some(text) = s.as_str() else {
                return false;
            };
            out.push_str(text);
        }
        _ => return false,
    }
    true
}

impl Vm {
    fn instance_class_is_none_or_builtin(
        &self,
        instance_class: Option<ObjectId>,
        expected: classes::BuiltinClass,
    ) -> bool {
        match instance_class {
            None => true,
            Some(cls) => {
                self.heap
                    .get_class(cls)
                    .ok()
                    .and_then(|c| self.heap.get_fast(c.prototype))
                    .and_then(|p| p.builtin_type.as_deref())
                    .and_then(classes::BuiltinClass::from_type)
                    == Some(expected)
            }
        }
    }

    fn parent_is_empty_builtin(
        &self,
        parent: Option<ObjectId>,
        kind: CollectionKind,
        expected: classes::BuiltinClass,
    ) -> bool {
        let mut cur = parent;
        let mut hops = 0u8;
        while let Some(p) = cur {
            hops += 1;
            if hops > 4 {
                return false;
            }
            let Some(pobj) = self.heap.get_fast(p) else {
                return false;
            };
            if pobj.collection_kind != kind
                || pobj.is_module
                || !pobj.collection_members.is_empty()
                || !pobj.member_annotations.is_empty()
            {
                return false;
            }
            if pobj
                .builtin_type
                .as_deref()
                .and_then(classes::BuiltinClass::from_type)
                == Some(expected)
            {
                return true;
            }
            if pobj.instance_class.is_some() || !pobj.members.is_empty() {
                return false;
            }
            cur = pobj.parent;
        }
        true
    }

    fn is_pure_shallow_member_block(&self, obj: &Object, member: &Member) -> bool {
        if member.code == usize::MAX {
            return true;
        }
        let Some(instructions) = self.program.blocks.get(member.code) else {
            return false;
        };
        let env = if obj.member_environments.is_empty() {
            &obj.environment
        } else {
            let cache_key = CacheKey {
                owner: None,
                name: member.name.clone(),
                method: false,
                entry: member.entry,
            };
            obj.member_environments
                .get(&cache_key)
                .unwrap_or(&obj.environment)
        };
        let is_scalar = |val: &Value| {
            matches!(
                val,
                Value::Null
                    | Value::Bool(_)
                    | Value::Int(_)
                    | Value::Float(_)
                    | Value::String(_)
                    | Value::Duration(_)
                    | Value::DataSize(_)
                    | Value::Bytes(_)
            )
        };
        let mut produced_object = false;
        for instr in instructions {
            match instr {
                Instruction::Constant(val) if is_scalar(val) => {}
                Instruction::LoadLocal(depth)
                    if depth
                        .checked_add(1)
                        .and_then(|offset| env.locals.len().checked_sub(offset))
                        .and_then(|index| env.locals.get(index))
                        .is_some_and(is_scalar) => {}
                Instruction::Observe(_)
                | Instruction::ObserveNativeLiteral { .. }
                | Instruction::Duplicate
                | Instruction::Pop
                | Instruction::Swap
                | Instruction::Return
                | Instruction::Add
                | Instruction::AddIntConstant(_)
                | Instruction::Subtract
                | Instruction::Multiply
                | Instruction::Divide
                | Instruction::IntegerDivide
                | Instruction::Remainder
                | Instruction::Power
                | Instruction::Negate
                | Instruction::Not
                | Instruction::Less
                | Instruction::LessEqual
                | Instruction::Greater
                | Instruction::GreaterEqual
                | Instruction::CheckBoolean
                | Instruction::CheckLogicalLeft { .. }
                | Instruction::CheckLogicalRight { .. }
                | Instruction::Jump(_)
                | Instruction::JumpIfFalse(_)
                | Instruction::JumpIfNull(_)
                | Instruction::JumpIfNotNull(_)
                | Instruction::NonNull
                | Instruction::CheckByte
                | Instruction::MakeBytes { .. } => {}
                Instruction::Equal
                | Instruction::NotEqual
                | Instruction::CheckInterpolate
                | Instruction::FinishInterpolation(_)
                | Instruction::PrepareSetElement
                | Instruction::AddSetElement
                | Instruction::PrepareMapEntry
                | Instruction::AddMapEntry
                | Instruction::BeginCollection {
                    kind: CollectionKind::List | CollectionKind::Set | CollectionKind::Map,
                    has_parent: false,
                    ..
                } if !produced_object => {}
                Instruction::GetMember(_) if !produced_object => {
                    produced_object = true;
                }
                Instruction::MakeList { .. }
                | Instruction::MakePair
                | Instruction::EndCollection => {
                    produced_object = true;
                }
                _ => return false,
            }
        }
        true
    }

    fn is_plain_shallow_forceable(&self, id: ObjectId) -> bool {
        if self.in_stdlib_bootstrap() {
            return false;
        }
        let Some(obj) = self.heap.get_fast(id) else {
            return false;
        };
        let expected = match obj.collection_kind {
            CollectionKind::Dynamic => classes::BuiltinClass::Dynamic,
            CollectionKind::Listing => classes::BuiltinClass::Listing,
            CollectionKind::Mapping => classes::BuiltinClass::Mapping,
            _ => return false,
        };
        !obj.is_module
            && obj.pair.is_none()
            && obj.reference.is_none()
            && obj.foreign_map_entry.is_none()
            && obj.function.is_none()
            && obj.native_function.is_none()
            && self.instance_class_is_none_or_builtin(obj.instance_class, expected)
            && self.parent_is_empty_builtin(obj.parent, obj.collection_kind, expected)
            && obj.members.iter().all(|m| {
                m.local
                    || m.method
                    || m.external()
                    || m.abstract_member()
                    || m.hidden()
                    || (!m.entry && m.name.as_str() == Some("default"))
                    || self.is_pure_shallow_member_block(obj, m)
            })
    }

    fn begin_shallow_force_for_render(
        &mut self,
        run: &mut RenderRun,
        receiver: ObjectId,
    ) -> Result<(), RuntimeError> {
        if self.heap.get(receiver)?.forced {
            return Ok(());
        }
        if self.is_plain_shallow_forceable(receiver) {
            self.advance_force_plain_object(run, receiver, 0)?;
            return Ok(());
        }
        if self.try_force_plain_typed_sync(receiver, 0)? {
            return Ok(());
        }
        run.await_ = Some(Await::Shallow);
        self.force_shallow_frame(Value::Object(receiver))
    }

    fn try_force_plain_typed_sync(
        &mut self,
        id: ObjectId,
        depth: u8,
    ) -> Result<bool, RuntimeError> {
        if self.in_stdlib_bootstrap() || depth >= 4 {
            return Ok(false);
        }
        let Some(obj) = self.heap.get_fast(id) else {
            return Ok(false);
        };
        if obj.pair.is_some()
            || obj.reference.is_some()
            || obj.foreign_map_entry.is_some()
            || obj.function.is_some()
            || obj.native_function.is_some()
        {
            return Ok(false);
        }
        if obj.collection_kind == CollectionKind::List && obj.parent.is_none() {
            let Some(len) = obj.list_elements.as_ref().map(Vec::len) else {
                return Ok(false);
            };
            let mut all_ok = true;
            for i in 0..len {
                let child = self
                    .heap
                    .get_fast(id)
                    .and_then(|o| o.list_elements.as_ref())
                    .and_then(|e| e.get(i))
                    .and_then(|v| match v {
                        Value::Object(cid) => Some(*cid),
                        _ => None,
                    });
                if let Some(child_id) = child
                    && child_id != id
                    && !self.try_force_plain_typed_sync(child_id, depth + 1)?
                {
                    all_ok = false;
                }
            }
            return Ok(all_ok);
        }
        if self.is_plain_shallow_forceable(id) {
            return self.try_force_plain_object_sync(id);
        }
        if obj.collection_kind != CollectionKind::Dynamic
            || obj.is_module
            || obj.collection_cast.is_some()
            || obj.class_definition.is_some()
            || obj.instance_class.is_none()
            || collections::is_typed_class(&self.heap, id).ok() != Some(true)
            || self.is_render_directive(id).ok() != Some(false)
            || self.is_pcf_render_directive(id).ok() != Some(false)
        {
            return Ok(false);
        }
        let mut chain = [id; 16];
        let mut chain_len = 0usize;
        let mut cur = Some(id);
        while let Some(anc_id) = cur {
            if chain_len >= chain.len() || chain[..chain_len].contains(&anc_id) {
                return Ok(false);
            }
            let Some(anc) = self.heap.get_fast(anc_id) else {
                return Ok(false);
            };
            if !anc.member_annotations.is_empty()
                || anc.collection_cast.is_some()
                || anc.members.iter().any(|m| !m.local && !m.method && m.entry)
            {
                return Ok(false);
            }
            chain[chain_len] = anc_id;
            chain_len += 1;
            cur = anc.parent;
        }
        let mut prefix_len = 0usize;
        for &anc_id in &chain[..chain_len] {
            let Some(anc) = self.heap.get_fast(anc_id) else {
                return Ok(false);
            };
            if collections::is_canonical_object_root(anc) {
                break;
            }
            prefix_len += 1;
        }
        // Walk parent-to-child so a subclass `hidden` property hides only its own
        // declaration and descendants without erasing a superclass's visible property.
        let mut ancestor_hidden: Vec<PklString> = Vec::new();
        let mut visible: Vec<(PklString, ObjectId, Member)> = Vec::new();
        for &anc_id in chain[..prefix_len].iter().rev() {
            let Some(anc) = self.heap.get_fast(anc_id) else {
                return Ok(false);
            };
            for m in anc.members.iter() {
                if m.local || m.method || m.entry {
                    continue;
                }
                if m.hidden() {
                    if !ancestor_hidden.contains(&m.name) {
                        ancestor_hidden.push(m.name.clone());
                    }
                    continue;
                }
                if m.external() || m.abstract_member() || m.definition() || m.import() {
                    continue;
                }
                if ancestor_hidden.contains(&m.name) {
                    continue;
                }
                if let Some(entry) = visible.iter_mut().find(|(name, _, _)| *name == m.name) {
                    entry.1 = anc_id;
                    entry.2 = m.clone();
                } else {
                    visible.push((m.name.clone(), anc_id, m.clone()));
                }
            }
        }
        let mut to_force: Vec<(ObjectId, Member)> = Vec::new();
        let mut nested_children: Vec<ObjectId> = Vec::new();
        {
            let Some(recv_obj) = self.heap.get_fast(id) else {
                return Ok(false);
            };
            for (name, owner_id, member) in visible {
                if let Some(cached) = fast_lookup_member_cached(recv_obj, &name, false) {
                    if let Value::Object(child_id) = cached
                        && *child_id != id
                    {
                        nested_children.push(*child_id);
                    }
                } else {
                    to_force.push((owner_id, member));
                }
            }
        }
        for (owner_id, member) in to_force {
            let Some(val) = self.try_force_trivial_property_sync(id, owner_id, &member)? else {
                return Ok(false);
            };
            if let Value::Object(child_id) = val
                && child_id != id
            {
                nested_children.push(child_id);
            }
        }
        for child_id in nested_children {
            let _ = self.try_force_plain_typed_sync(child_id, depth + 1)?;
        }
        Ok(true)
    }

    fn build_pure_scalar_set(&mut self, values: Vec<Value>) -> Option<ObjectId> {
        let len = values.len();
        if len == 0 {
            return Some(self.empty_strict_collection(CollectionKind::Set));
        }
        if len > 4 {
            return None;
        }
        let mut hashes = Vec::with_capacity(len);
        for v in &values {
            hashes.push(self.java_hash(v).ok()?);
        }
        let id = self.heap.allocate(Object {
            collection_kind: CollectionKind::Set,
            length: len,
            set_entries: values.clone().into(),
            map_index: Some(Box::default()),
            ..Object::default()
        });
        {
            let map_index = self.heap.get_mut(id).ok()?.map_index.as_mut()?;
            for &hash in &hashes {
                let Ok(()) =
                    map_index.insert_lazy::<std::convert::Infallible>(hash, true, |slot| {
                        Ok(hashes[slot])
                    });
            }
        }
        let mut cache_index = crate::economic_map::EconomicMapIndex::with_capacity(0);
        let mut collection_members = Vec::with_capacity(len);
        let mut members = Vec::with_capacity(len);
        let mut cache_slots = Vec::with_capacity(len);
        let mut member_values = crate::heap::FastMap::default();
        for (idx, val) in values.into_iter().enumerate() {
            let name = collections::format_entry_internal_name(id.index, idx);
            let key_int = Value::Int((idx as i64).into());
            collection_members.push(CollectionMember {
                key: key_int.clone(),
                name: name.clone(),
                kind: CollectionMemberKind::Element,
            });
            members.push(Member {
                flags: 0,
                entry: true,
                name: name.clone(),
                code: usize::MAX,
                local: false,
                method: false,
                ty: None,
                type_span: None,
                constant: false,
                fixed: false,
            });
            let cache_key = CacheKey {
                owner: None,
                name,
                method: false,
                entry: true,
            };
            cache_index.grow_before_append();
            cache_index.push();
            cache_slots.push(crate::heap::CacheSlot {
                key: cache_key.clone(),
                entry_key: Some(key_int),
                value: val.clone(),
            });
            member_values.insert(cache_key, val);
        }
        let obj = self.heap.get_mut(id).ok()?;
        obj.collection_members = collection_members;
        obj.members = members.into();
        obj.cache_slots = cache_slots;
        obj.cache_index = cache_index;
        obj.member_values = member_values.into();
        obj.rebuild_cache_map();
        Some(id)
    }

    fn try_eval_pure_member(
        &mut self,
        receiver: ObjectId,
        member: &Member,
        cache_key: &CacheKey,
    ) -> Option<Value> {
        if self.active_type_trackers != 0 || member.ty.is_some() || member.code == usize::MAX {
            return None;
        }
        let instructions_len = self.program.blocks.get(member.code)?.len();
        if instructions_len == 0 || instructions_len > 64 {
            return None;
        }
        let (locals_len, restore_for_bindings) = {
            let object = self.heap.get_fast(receiver)?;
            if object.collection_cast.is_some() || !object.member_values.is_empty() {
                return None;
            }
            if object.cache_contains_key(cache_key) {
                return None;
            }
            let env = object
                .member_environments
                .get(cache_key)
                .unwrap_or(&object.environment);
            (env.locals.len(), env.restore_for_bindings)
        };
        let trailing_is_return = |instructions: &[Instruction], mut j: usize| -> bool {
            while j < instructions.len() {
                match &instructions[j] {
                    Instruction::Observe(_) | Instruction::ObserveNativeLiteral { .. } => j += 1,
                    Instruction::Return => return j + 1 == instructions.len(),
                    _ => return false,
                }
            }
            false
        };
        let mut stack: Vec<Value> = Vec::with_capacity(8);
        let mut building_set: Option<Vec<Value>> = None;
        for idx in 0..instructions_len {
            let instr = &self.program.blocks[member.code][idx];
            match instr {
                Instruction::Observe(_) | Instruction::ObserveNativeLiteral { .. } => {}
                Instruction::Constant(val) => stack.push(val.clone()),
                Instruction::LoadLocal(depth) => {
                    let index = depth
                        .checked_add(1)
                        .and_then(|offset| locals_len.checked_sub(offset))?;
                    let val = {
                        let object = self.heap.get_fast(receiver)?;
                        let env = object
                            .member_environments
                            .get(cache_key)
                            .unwrap_or(&object.environment);
                        env.locals.get(index)?.clone()
                    };
                    stack.push(val);
                }
                Instruction::Duplicate => {
                    let top = stack.last()?.clone();
                    stack.push(top);
                }
                Instruction::Pop => {
                    stack.pop()?;
                }
                Instruction::Swap => {
                    let len = stack.len();
                    if len < 2 {
                        return None;
                    }
                    stack.swap(len - 2, len - 1);
                }
                Instruction::AddIntConstant(k) => {
                    let Value::Int(a) = stack.pop()? else {
                        return None;
                    };
                    let sum = crate::numeric::add(a.get(), *k).ok()?;
                    stack.push(Value::Int(sum.into()));
                }
                Instruction::Add => {
                    let right = stack.pop()?;
                    let left = stack.pop()?;
                    let res = match (left, right) {
                        (Value::Int(a), Value::Int(b)) => {
                            Value::Int(crate::numeric::add(a.get(), b.get()).ok()?.into())
                        }
                        (Value::Int(a), Value::Float(b)) => {
                            Value::Float((a.get() as f64 + b.get()).into())
                        }
                        (Value::Float(a), Value::Int(b)) => {
                            Value::Float((a.get() + b.get() as f64).into())
                        }
                        (Value::Float(a), Value::Float(b)) => {
                            Value::Float((a.get() + b.get()).into())
                        }
                        (Value::String(a), Value::String(b)) => Value::string(a.concat(&b)),
                        _ => return None,
                    };
                    stack.push(res);
                }
                Instruction::Subtract => {
                    let right = stack.pop()?;
                    let left = stack.pop()?;
                    let res = match (left, right) {
                        (Value::Int(a), Value::Int(b)) => {
                            Value::Int(crate::numeric::subtract(a.get(), b.get()).ok()?.into())
                        }
                        (Value::Int(a), Value::Float(b)) => {
                            Value::Float((a.get() as f64 - b.get()).into())
                        }
                        (Value::Float(a), Value::Int(b)) => {
                            Value::Float((a.get() - b.get() as f64).into())
                        }
                        (Value::Float(a), Value::Float(b)) => {
                            Value::Float((a.get() - b.get()).into())
                        }
                        _ => return None,
                    };
                    stack.push(res);
                }
                Instruction::Multiply => {
                    let right = stack.pop()?;
                    let left = stack.pop()?;
                    let res = match (left, right) {
                        (Value::Int(a), Value::Int(b)) => {
                            Value::Int(crate::numeric::multiply(a.get(), b.get()).ok()?.into())
                        }
                        (Value::Int(a), Value::Float(b)) => {
                            Value::Float((a.get() as f64 * b.get()).into())
                        }
                        (Value::Float(a), Value::Int(b)) => {
                            Value::Float((a.get() * b.get() as f64).into())
                        }
                        (Value::Float(a), Value::Float(b)) => {
                            Value::Float((a.get() * b.get()).into())
                        }
                        _ => return None,
                    };
                    stack.push(res);
                }
                Instruction::Divide => {
                    let right = stack.pop()?;
                    let left = stack.pop()?;
                    let (a, b) = match (left, right) {
                        (Value::Int(a), Value::Int(b)) => (a.get() as f64, b.get() as f64),
                        (Value::Int(a), Value::Float(b)) => (a.get() as f64, b.get()),
                        (Value::Float(a), Value::Int(b)) => (a.get(), b.get() as f64),
                        (Value::Float(a), Value::Float(b)) => (a.get(), b.get()),
                        _ => return None,
                    };
                    if b == 0.0 {
                        return None;
                    }
                    stack.push(Value::Float((a / b).into()));
                }
                Instruction::Remainder => {
                    let right = stack.pop()?;
                    let left = stack.pop()?;
                    let Value::Int(a) = left else {
                        return None;
                    };
                    let Value::Int(b) = right else {
                        return None;
                    };
                    let rem = crate::numeric::remainder(a.get(), b.get()).ok()?;
                    stack.push(Value::Int(rem.into()));
                }
                Instruction::Equal | Instruction::NotEqual => {
                    let neg = matches!(instr, Instruction::NotEqual);
                    let right = stack.pop()?;
                    let left = stack.pop()?;
                    let eq = match (&left, &right) {
                        (Value::Int(a), Value::Int(b)) => a.get() == b.get(),
                        (Value::Bool(a), Value::Bool(b)) => a == b,
                        (Value::String(a), Value::String(b)) => a == b,
                        _ => return None,
                    };
                    stack.push(Value::Bool(eq ^ neg));
                }
                Instruction::CheckInterpolate => {
                    if !matches!(stack.last(), Some(Value::String(_) | Value::Int(_))) {
                        return None;
                    }
                }
                Instruction::FinishInterpolation(count) => {
                    let count = *count;
                    if stack.len() < count {
                        return None;
                    }
                    let start = stack.len() - count;
                    self.scratch_utf8.clear();
                    for part in &stack[start..] {
                        match part {
                            Value::String(s) => {
                                let text = s.as_str()?;
                                self.scratch_utf8.push_str(text);
                            }
                            Value::Int(n) => {
                                let mut buf = [0u8; 24];
                                self.scratch_utf8
                                    .push_str(super::format_i64_buf(n.get(), &mut buf));
                            }
                            _ => return None,
                        }
                    }
                    stack.truncate(start);
                    let result = PklString::from(self.scratch_utf8.as_str());
                    stack.push(Value::string(result));
                }
                Instruction::CheckByte => {
                    let Some(Value::Int(n)) = stack.last() else {
                        return None;
                    };
                    if !(0..=255).contains(&n.get()) {
                        return None;
                    }
                }
                Instruction::MakeBytes { length } => {
                    let length = *length;
                    if stack.len() < length {
                        return None;
                    }
                    let start = stack.len() - length;
                    let mut bytes = Vec::with_capacity(length);
                    for v in stack.drain(start..) {
                        let Value::Int(n) = v else {
                            return None;
                        };
                        bytes.push(n.get() as u8);
                    }
                    stack.push(Value::Bytes(bytes.into()));
                }
                Instruction::GetMember(name) => {
                    let name_str = name.as_str()?;
                    let val = stack.pop()?;
                    let num = match val {
                        Value::Int(n) => n.get() as f64,
                        Value::Float(f) => f.get(),
                        _ => return None,
                    };
                    if let Some(unit) = crate::duration::DurationUnit::parse(name_str) {
                        stack.push(Value::Duration(crate::duration::Duration::new(num, unit)));
                    } else {
                        let unit = crate::data_size::DataSizeUnit::parse(name_str)?;
                        stack.push(Value::DataSize(crate::data_size::DataSize::new(num, unit)));
                    }
                }
                Instruction::BeginCollection {
                    kind: CollectionKind::Set,
                    has_parent: false,
                    ..
                } if !restore_for_bindings && building_set.is_none() => {
                    building_set = Some(Vec::with_capacity(4));
                }
                Instruction::PrepareSetElement if building_set.is_some() => {
                    if !matches!(
                        stack.last(),
                        Some(Value::String(_) | Value::Int(_) | Value::Bool(_) | Value::Float(_))
                    ) {
                        return None;
                    }
                }
                Instruction::AddSetElement if building_set.is_some() => {
                    let val = stack.pop()?;
                    let set = building_set.as_mut()?;
                    if !set
                        .iter()
                        .any(|old| equality::same_key(&self.heap, &val, old))
                    {
                        if set.len() >= 4 {
                            return None;
                        }
                        set.push(val);
                    }
                }
                Instruction::EndCollection
                    if building_set.is_some()
                        && trailing_is_return(&self.program.blocks[member.code], idx + 1) =>
                {
                    let elems = building_set.take()?;
                    let id = self.build_pure_scalar_set(elems)?;
                    stack.push(Value::Object(id));
                }
                Instruction::MakeList { length }
                    if !restore_for_bindings
                        && building_set.is_none()
                        && trailing_is_return(&self.program.blocks[member.code], idx + 1) =>
                {
                    let length = *length;
                    if stack.len() < length {
                        return None;
                    }
                    let start = stack.len() - length;
                    let values: Vec<Value> = stack.drain(start..).collect();
                    let id = self.build_strict_list(values);
                    stack.push(Value::Object(id));
                }
                Instruction::Return => {
                    if building_set.is_none() && stack.len() == 1 {
                        return stack.pop();
                    }
                    return None;
                }
                _ => return None,
            }
        }
        None
    }

    fn try_force_plain_object_sync(&mut self, receiver: ObjectId) -> Result<bool, RuntimeError> {
        if self.frames.last().is_some_and(|f| f.equality_run.is_some()) {
            return Ok(false);
        }
        let len = {
            let object = self.heap.get(receiver)?;
            if object.forced {
                return Ok(true);
            }
            if object.cache_index.is_rehash_incomplete() {
                return Ok(false);
            }
            object.members.len()
        };
        if !self.all_cache_slots_non_object(receiver)? {
            return Ok(false);
        }
        for idx in 0..len {
            let next_member = {
                let object = self.heap.get(receiver)?;
                if object.forced {
                    return Ok(true);
                }
                let member = &object.members[idx];
                if member.local
                    || member.method
                    || member.external()
                    || member.abstract_member()
                    || member.hidden()
                    || (!member.entry && member.name.as_str() == Some("default"))
                {
                    continue;
                }
                if !member.entry && fast_lookup_member_cached(object, &member.name, false).is_some()
                {
                    continue;
                }
                member.clone()
            };
            let cache_key = CacheKey {
                owner: None,
                name: next_member.name.clone(),
                method: false,
                entry: next_member.entry,
            };
            let entry_probe = if next_member.entry {
                let obj = self.heap.get(receiver)?;
                if let Some(cm) = obj.collection_members.get(idx)
                    && cm.name == next_member.name
                    && cm.kind != CollectionMemberKind::Property
                {
                    Some(cm.key.clone())
                } else {
                    self.cache_value_key(receiver, &cache_key)?
                }
            } else {
                None
            };
            if let Some(ref probe) = entry_probe
                && !matches!(probe, Value::Object(_))
            {
                let obj = self.heap.get(receiver)?;
                if obj.cache_get_completed(&cache_key).is_some()
                    || (!obj.cache_slots.is_empty()
                        && self
                            .lookup_non_object_cache_entry(receiver, probe)?
                            .is_some())
                {
                    continue;
                }
            }
            if entry_probe
                .as_ref()
                .is_none_or(|p| !matches!(p, Value::Object(_)))
                && let Some(val) = self.try_eval_pure_member(receiver, &next_member, &cache_key)
            {
                self.complete_cache_probe(receiver, cache_key, val, entry_probe)?;
                continue;
            }
            return Ok(false);
        }
        Ok(true)
    }

    fn advance_force_plain_object(
        &mut self,
        run: &mut RenderRun,
        receiver: ObjectId,
        mut index: usize,
    ) -> Result<bool, RuntimeError> {
        let len = self.heap.get(receiver)?.members.len();
        let mut slots_all_non_object: Option<bool> = None;
        while index < len {
            let next_member = {
                let object = self.heap.get(receiver)?;
                if object.forced {
                    return Ok(false);
                }
                let member = &object.members[index];
                index += 1;
                if member.local
                    || member.method
                    || member.external()
                    || member.abstract_member()
                    || member.hidden()
                    || (!member.entry && member.name.as_str() == Some("default"))
                {
                    continue;
                }
                if !member.entry && fast_lookup_member_cached(object, &member.name, false).is_some()
                {
                    continue;
                }
                member.clone()
            };
            let cache_key = CacheKey {
                owner: None,
                name: next_member.name.clone(),
                method: false,
                entry: next_member.entry,
            };
            let entry_probe = if next_member.entry {
                let obj = self.heap.get(receiver)?;
                if let Some(cm) = obj.collection_members.get(index - 1)
                    && cm.name == next_member.name
                    && cm.kind != CollectionMemberKind::Property
                {
                    Some(cm.key.clone())
                } else {
                    self.cache_value_key(receiver, &cache_key)?
                }
            } else {
                None
            };
            let rehash_incomplete = self.heap.get(receiver)?.cache_index.is_rehash_incomplete();
            if !rehash_incomplete
                && let Some(ref probe) = entry_probe
                && !matches!(probe, Value::Object(_))
            {
                let obj = self.heap.get(receiver)?;
                if obj.cache_get_completed(&cache_key).is_some()
                    || (!obj.cache_slots.is_empty()
                        && self
                            .lookup_non_object_cache_entry(receiver, probe)?
                            .is_some())
                {
                    continue;
                }
            }
            let can_complete_sync = !rehash_incomplete
                && self.frames.last().is_none_or(|f| f.equality_run.is_none())
                && entry_probe
                    .as_ref()
                    .is_none_or(|p| !matches!(p, Value::Object(_)))
                && {
                    let obj = self.heap.get(receiver)?;
                    let fast_append = obj.cache_index.len() == obj.cache_slots.len()
                        && obj.cache_index.can_fast_append_no_rehash();
                    if fast_append {
                        true
                    } else {
                        match slots_all_non_object {
                            Some(flag) => flag,
                            None => {
                                let flag = self.all_cache_slots_non_object(receiver)?;
                                slots_all_non_object = Some(flag);
                                flag
                            }
                        }
                    }
                };
            if can_complete_sync
                && let Some(val) = self.try_eval_pure_member(receiver, &next_member, &cache_key)
            {
                self.complete_cache_probe(receiver, cache_key, val, entry_probe)?;
                continue;
            }
            let pushed = index < len;
            if pushed {
                run.tasks.push(Task::ForcePlainObject { receiver, index });
            }
            run.await_ = Some(Await::Shallow);
            let frames_before = self.frames.len();
            let stack_before = self.stack.len();
            self.force_declaration_member(receiver, receiver, next_member)?;
            if self.frames.len() == frames_before
                && self.stack.len() == stack_before + 1
                && self.frames[frames_before - 1].equality_run.is_none()
                && self.frames[frames_before - 1].type_run.is_none()
            {
                self.stack.pop();
                run.await_ = None;
                if pushed {
                    run.tasks.pop();
                }
                slots_all_non_object = None;
                continue;
            }
            return Ok(false);
        }
        Ok(false)
    }

    fn is_plain_unforced_dynamic(&self, id: ObjectId) -> bool {
        let Some(obj) = self.heap.get_fast(id) else {
            return false;
        };
        obj.collection_kind == CollectionKind::Dynamic
            && !obj.is_module
            && obj.pair.is_none()
            && obj.reference.is_none()
            && obj.foreign_map_entry.is_none()
            && obj.function.is_none()
            && obj.native_function.is_none()
            && self.instance_class_is_none_or_builtin(
                obj.instance_class,
                classes::BuiltinClass::Dynamic,
            )
            && self.parent_is_empty_builtin(
                obj.parent,
                CollectionKind::Dynamic,
                classes::BuiltinClass::Dynamic,
            )
    }

    fn extract_fast_object<'a>(
        &'a self,
        id: ObjectId,
        scratch: &mut Vec<FastMember<'a>>,
        check_xml_marker: bool,
    ) -> Option<FastObjectKind<'a>> {
        let obj = self.heap.get_fast(id)?;
        if obj.pair.is_some()
            || obj.reference.is_some()
            || obj.foreign_map_entry.is_some()
            || obj.function.is_some()
            || obj.native_function.is_some()
        {
            return None;
        }
        match obj.collection_kind {
            CollectionKind::List if obj.parent.is_none() => {
                if let Some(elems) = &obj.list_elements {
                    return Some(FastObjectKind::StrictList(elems.as_slice()));
                }
                if obj.length == 0 && obj.collection_members.is_empty() {
                    return Some(FastObjectKind::EmptyList);
                }
                return None;
            }
            CollectionKind::Set if obj.parent.is_none() => {
                if obj.length != obj.collection_members.len() {
                    return None;
                }
                let base = scratch.len();
                let slots_aligned = obj.cache_slots.len() == obj.collection_members.len();
                for (idx, cm) in obj.collection_members.iter().enumerate() {
                    if cm.kind != CollectionMemberKind::Element {
                        scratch.truncate(base);
                        return None;
                    }
                    let val = if slots_aligned
                        && obj.cache_slots[idx].key.entry
                        && !obj.cache_slots[idx].key.method
                        && obj.cache_slots[idx].key.owner.is_none()
                        && obj.cache_slots[idx].key.name == cm.name
                    {
                        &obj.cache_slots[idx].value
                    } else if let Some(val) = fast_lookup_member_cached(obj, &cm.name, true) {
                        val
                    } else {
                        scratch.truncate(base);
                        return None;
                    };
                    scratch.push(FastMember::Element(idx as i64, val));
                }
                return Some(FastObjectKind::StrictSet(obj.collection_members.len()));
            }
            CollectionKind::Map if obj.parent.is_none() => {
                let base = scratch.len();
                if let Some(order) = obj.map_key_order.as_deref() {
                    if order.len() != obj.length {
                        return None;
                    }
                    for cm in order {
                        let Some(val) = fast_lookup_member_cached(obj, &cm.name, true) else {
                            scratch.truncate(base);
                            return None;
                        };
                        scratch.push(FastMember::Entry(&cm.key, val));
                    }
                    return Some(FastObjectKind::StrictMap(order.len()));
                }
                let order = obj.map_index().insertion_order();
                if order.len() != obj.length {
                    return None;
                }
                let slots_aligned = obj.cache_slots.len() == obj.collection_members.len();
                for &slot_idx in order {
                    let Some(cm) = obj.collection_members.get(slot_idx) else {
                        scratch.truncate(base);
                        return None;
                    };
                    let val = if slots_aligned
                        && obj.cache_slots[slot_idx].key.entry
                        && !obj.cache_slots[slot_idx].key.method
                        && obj.cache_slots[slot_idx].key.owner.is_none()
                        && obj.cache_slots[slot_idx].key.name == cm.name
                    {
                        &obj.cache_slots[slot_idx].value
                    } else if let Some(val) = fast_lookup_member_cached(obj, &cm.name, true) {
                        val
                    } else {
                        scratch.truncate(base);
                        return None;
                    };
                    scratch.push(FastMember::Entry(&cm.key, val));
                }
                return Some(FastObjectKind::StrictMap(order.len()));
            }
            CollectionKind::List | CollectionKind::Set | CollectionKind::Map => return None,
            CollectionKind::Dynamic | CollectionKind::Listing | CollectionKind::Mapping => {}
        }

        if !obj.member_annotations.is_empty() {
            return None;
        }

        if !obj.is_module {
            let expected_builtin = match obj.collection_kind {
                CollectionKind::Dynamic => classes::BuiltinClass::Dynamic,
                CollectionKind::Listing => classes::BuiltinClass::Listing,
                CollectionKind::Mapping => classes::BuiltinClass::Mapping,
                _ => return None,
            };
            if !self.instance_class_is_none_or_builtin(obj.instance_class, expected_builtin)
                || !self.parent_is_empty_builtin(obj.parent, obj.collection_kind, expected_builtin)
            {
                // Fast path for non-module Typed class instances with all visible
                // properties already cached on the receiver and no property annotations.
                if obj.collection_kind == CollectionKind::Dynamic
                    && obj.collection_cast.is_none()
                    && obj.class_definition.is_none()
                    && obj.instance_class.is_some()
                    && collections::is_typed_class(&self.heap, id).ok() == Some(true)
                    && self.is_render_directive(id).ok() == Some(false)
                    && self.is_pcf_render_directive(id).ok() == Some(false)
                    && (self.modules_stdlib.get("pkl:jsonnet").is_none()
                        || self.jsonnet_wrapper(id).ok() == Some(None))
                    && (!check_xml_marker
                        || self.modules_stdlib.get("pkl:xml").is_none()
                        || self.xml_wrapper(id).ok() == Some(None))
                {
                    let mut chain = [id; 16];
                    let mut chain_len = 0usize;
                    let mut cur = Some(id);
                    while let Some(anc_id) = cur {
                        if chain_len >= chain.len() || chain[..chain_len].contains(&anc_id) {
                            return None;
                        }
                        let anc = self.heap.get_fast(anc_id)?;
                        if !anc.member_annotations.is_empty()
                            || anc.collection_cast.is_some()
                            || anc.members.iter().any(|m| !m.local && !m.method && m.entry)
                        {
                            return None;
                        }
                        chain[chain_len] = anc_id;
                        chain_len += 1;
                        cur = anc.parent;
                    }
                    let base = scratch.len();
                    let mut seen_hidden: [Option<&PklString>; 64] = [None; 64];
                    let mut hidden_len = 0usize;
                    for &anc_id in chain[..chain_len].iter().rev() {
                        let anc = self.heap.get_fast(anc_id)?;
                        for m in anc.members.iter() {
                            if m.local || m.method {
                                continue;
                            }
                            if seen_hidden[..hidden_len].contains(&Some(&m.name))
                                || scratch[base..].iter().any(
                                    |prev| matches!(prev, FastMember::Property(n, _) if *n == &m.name),
                                )
                            {
                                continue;
                            }
                            if check_xml_marker && m.name == "_isXmlElement" {
                                scratch.truncate(base);
                                return None;
                            }
                            if m.hidden() || m.external() || m.definition() || m.import() {
                                if hidden_len >= seen_hidden.len() {
                                    scratch.truncate(base);
                                    return None;
                                }
                                seen_hidden[hidden_len] = Some(&m.name);
                                hidden_len += 1;
                                continue;
                            }
                            if scratch.len() - base >= 128 {
                                scratch.truncate(base);
                                return None;
                            }
                            let Some(val) = fast_lookup_member_cached(obj, &m.name, false) else {
                                scratch.truncate(base);
                                return None;
                            };
                            scratch.push(FastMember::Property(&m.name, val));
                        }
                    }
                    let count = scratch.len() - base;
                    return Some(FastObjectKind::PlainTyped { count });
                }
                return None;
            }
            let mut non_local_non_method = 0usize;
            for m in obj.members.iter() {
                if m.local || m.method {
                    continue;
                }
                if m.hidden()
                    || m.external()
                    || m.definition()
                    || m.import()
                    || (!m.entry && m.name == "default")
                    || (check_xml_marker && !m.entry && m.name == "_isXmlElement")
                {
                    return None;
                }
                non_local_non_method += 1;
            }
            let base = scratch.len();
            let mut has_elements = false;
            let mut has_keyed = false;
            if !obj.collection_members.is_empty() {
                if non_local_non_method != obj.collection_members.len() {
                    return None;
                }
                let slots_aligned = obj.cache_slots.len() == obj.collection_members.len();
                for (idx, cm) in obj.collection_members.iter().enumerate() {
                    let is_entry = cm.kind != CollectionMemberKind::Property;
                    let aligned_hit = slots_aligned
                        && obj.cache_slots[idx].key.entry == is_entry
                        && !obj.cache_slots[idx].key.method
                        && obj.cache_slots[idx].key.owner.is_none()
                        && obj.cache_slots[idx].key.name == cm.name;
                    let val = if aligned_hit {
                        &obj.cache_slots[idx].value
                    } else if let Some(val) = fast_lookup_member_cached(obj, &cm.name, is_entry) {
                        val
                    } else {
                        scratch.truncate(base);
                        return None;
                    };
                    match cm.kind {
                        CollectionMemberKind::Property => {
                            if cm.name == "default"
                                || (check_xml_marker && cm.name == "_isXmlElement")
                            {
                                scratch.truncate(base);
                                return None;
                            }
                            if !aligned_hit
                                && (scratch.len() - base > 32
                                    || scratch[base..].iter().any(|prev| {
                                        matches!(prev, FastMember::Property(n, _) if *n == &cm.name)
                                    }))
                            {
                                scratch.truncate(base);
                                return None;
                            }
                            has_keyed = true;
                            scratch.push(FastMember::Property(&cm.name, val));
                        }
                        CollectionMemberKind::Entry => {
                            if !matches!(
                                &cm.key,
                                Value::String(_) | Value::Int(_) | Value::Bool(_) | Value::Float(_)
                            ) {
                                scratch.truncate(base);
                                return None;
                            }
                            has_keyed = true;
                            scratch.push(FastMember::Entry(&cm.key, val));
                        }
                        CollectionMemberKind::Element => {
                            let Value::Int(i) = &cm.key else {
                                scratch.truncate(base);
                                return None;
                            };
                            has_elements = true;
                            scratch.push(FastMember::Element(i.get(), val));
                        }
                    }
                }
            } else {
                let slots_aligned = obj.cache_slots.len() == obj.members.len();
                for (idx, m) in obj.members.iter().enumerate() {
                    if m.local || m.method {
                        continue;
                    }
                    if m.entry {
                        scratch.truncate(base);
                        return None;
                    }
                    let aligned_hit = slots_aligned
                        && !obj.cache_slots[idx].key.entry
                        && !obj.cache_slots[idx].key.method
                        && obj.cache_slots[idx].key.owner.is_none()
                        && obj.cache_slots[idx].key.name == m.name;
                    let val = if aligned_hit {
                        &obj.cache_slots[idx].value
                    } else if let Some(val) = fast_lookup_member_cached(obj, &m.name, false) {
                        val
                    } else {
                        scratch.truncate(base);
                        return None;
                    };
                    if !aligned_hit
                        && (scratch.len() - base > 32
                            || scratch[base..].iter().any(
                                |prev| matches!(prev, FastMember::Property(n, _) if *n == &m.name),
                            ))
                    {
                        scratch.truncate(base);
                        return None;
                    }
                    has_keyed = true;
                    scratch.push(FastMember::Property(&m.name, val));
                }
            }
            let count = scratch.len() - base;
            return match obj.collection_kind {
                CollectionKind::Dynamic => {
                    if let Some(expected) = obj.dynamic_regular_member_count
                        && expected != count
                    {
                        scratch.truncate(base);
                        return None;
                    }
                    Some(FastObjectKind::PlainDynamic {
                        count,
                        has_elements,
                        has_keyed,
                    })
                }
                CollectionKind::Listing => {
                    if has_keyed || count != obj.length {
                        scratch.truncate(base);
                        return None;
                    }
                    Some(FastObjectKind::PlainListing(count))
                }
                CollectionKind::Mapping => {
                    if has_elements {
                        scratch.truncate(base);
                        return None;
                    }
                    Some(FastObjectKind::PlainMapping(count))
                }
                _ => unreachable!(),
            };
        }

        // Plain top-level module whose parent is None, `pkl.base#Module` prototype, or `pkl:base`.
        if obj.collection_kind != CollectionKind::Dynamic
            || self.is_render_directive(id).ok() != Some(false)
        {
            return None;
        }
        let parent_ok = match obj.parent {
            None => true,
            Some(p) => self.heap.get_fast(p).is_some_and(|pobj| {
                pobj.member_annotations.is_empty()
                    && (pobj
                        .builtin_type
                        .as_deref()
                        .and_then(classes::BuiltinClass::from_type)
                        == Some(classes::BuiltinClass::Ordinary(output::MODULE))
                        || pobj
                            .module_unit
                            .and_then(|u| self.program.units.get(u.0 as usize))
                            .and_then(|u| u.identity.as_ref())
                            .is_some_and(|ident| ident.original_uri == "pkl:base"))
            }),
        };
        if !parent_ok {
            return None;
        }
        let Ok(Some(class)) = self.heap.object_class(id) else {
            return None;
        };
        let base = scratch.len();
        if !obj.collection_members.is_empty() {
            let mut non_local_non_method = 0usize;
            for m in obj.members.iter() {
                if !m.local && !m.method {
                    if m.entry || m.external() || m.definition() || m.import() {
                        return None;
                    }
                    non_local_non_method += 1;
                }
            }
            if non_local_non_method != obj.collection_members.len() {
                return None;
            }
            for cm in &obj.collection_members {
                if cm.kind != CollectionMemberKind::Property {
                    scratch.truncate(base);
                    return None;
                }
                match collections::hidden_property(&self.heap, id, &cm.name, false) {
                    Ok(true) => continue,
                    Ok(false) => {}
                    Err(_) => {
                        scratch.truncate(base);
                        return None;
                    }
                }
                if scratch[base..]
                    .iter()
                    .any(|prev| matches!(prev, FastMember::Property(n, _) if *n == &cm.name))
                {
                    scratch.truncate(base);
                    return None;
                }
                let Some(val) = fast_lookup_member_cached(obj, &cm.name, false) else {
                    scratch.truncate(base);
                    return None;
                };
                scratch.push(FastMember::Property(&cm.name, val));
            }
        } else {
            for m in obj.members.iter() {
                if m.local || m.method {
                    continue;
                }
                if m.entry || m.external() || m.definition() || m.import() {
                    scratch.truncate(base);
                    return None;
                }
                if m.hidden() {
                    continue;
                }
                match collections::hidden_property(&self.heap, id, &m.name, false) {
                    Ok(true) => continue,
                    Ok(false) => {}
                    Err(_) => {
                        scratch.truncate(base);
                        return None;
                    }
                }
                if scratch[base..]
                    .iter()
                    .any(|prev| matches!(prev, FastMember::Property(n, _) if *n == &m.name))
                {
                    scratch.truncate(base);
                    return None;
                }
                let Some(val) = fast_lookup_member_cached(obj, &m.name, false) else {
                    scratch.truncate(base);
                    return None;
                };
                scratch.push(FastMember::Property(&m.name, val));
            }
        }
        let count = scratch.len() - base;
        Some(FastObjectKind::PlainModule { class, count })
    }

    fn is_unmodified_default_renderer(&self, receiver: ObjectId, format: Format) -> bool {
        let Some(obj) = self.heap.get_fast(receiver) else {
            return false;
        };
        if !obj.members.is_empty() || !obj.collection_members.is_empty() {
            return false;
        }
        let Some(parent) = obj.parent else {
            return false;
        };
        let Some(pobj) = self.heap.get_fast(parent) else {
            return false;
        };
        match format {
            Format::Pcf => pobj
                .class_definition
                .as_ref()
                .is_some_and(|c| c.id == output::PCF_RENDERER),
            Format::Json => pobj
                .class_definition
                .as_ref()
                .is_some_and(|c| c.id == output::JSON_RENDERER),
            Format::Yaml => pobj
                .class_definition
                .as_ref()
                .is_some_and(|c| c.id == output::YAML_RENDERER),
            Format::Plist => pobj
                .class_definition
                .as_ref()
                .is_some_and(|c| c.id == output::PLIST_RENDERER),
            Format::Properties => pobj
                .class_definition
                .as_ref()
                .is_some_and(|c| c.id == output::PROPERTIES_RENDERER),
            Format::Xml | Format::Jsonnet | Format::PklBinary => {
                let uri = match format {
                    Format::Xml => "pkl:xml",
                    Format::Jsonnet => "pkl:jsonnet",
                    Format::PklBinary => "pkl:pklbinary",
                    _ => unreachable!(),
                };
                let Some(modules::CachedModule::Value(module_id)) = self.modules_stdlib.get(uri)
                else {
                    return false;
                };
                let Some(module) = self.heap.get_fast(*module_id) else {
                    return false;
                };
                let Some(Value::Class(expected_class)) =
                    fast_lookup_member_cached(module, &PklString::from("Renderer"), false)
                else {
                    return false;
                };
                self.heap
                    .get_class(*expected_class)
                    .ok()
                    .is_some_and(|cls| cls.prototype == parent)
            }
            Format::Protobuf => false,
        }
    }

    fn try_fast_render_default_document(
        &self,
        format: Format,
        document: bool,
        root: &Value,
    ) -> Option<Value> {
        let mut active = Vec::with_capacity(8);
        let mut scratch = Vec::with_capacity(32);
        match format {
            Format::PklBinary => self
                .try_fast_encode_pkl_binary(root)
                .map(|bytes| Value::Bytes(bytes.into())),
            Format::Pcf => {
                if document && self.pcf_document_value(root).ok() != Some(true) {
                    return None;
                }
                let mut buf = String::with_capacity(4096);
                let ok = self.write_fast_pcf_value(
                    root,
                    &mut buf,
                    "  ",
                    false,
                    false,
                    document,
                    0,
                    false,
                    true,
                    false,
                    false,
                    None,
                    &mut active,
                    &mut scratch,
                );
                if !ok {
                    return None;
                }
                if document && !buf.is_empty() {
                    buf.push('\n');
                }
                Some(Value::String(PklString::from(buf)))
            }
            Format::Json => {
                let mut buf = String::with_capacity(4096);
                let ok = self.write_fast_json_value(
                    root,
                    &mut buf,
                    "  ",
                    true,
                    0,
                    None,
                    &mut active,
                    &mut scratch,
                );
                if !ok {
                    return None;
                }
                if document {
                    buf.push('\n');
                }
                Some(Value::String(PklString::from(buf)))
            }
            Format::Jsonnet => {
                let mut buf = String::with_capacity(4096);
                let ok = self.write_fast_jsonnet_value(
                    root,
                    &mut buf,
                    "  ",
                    true,
                    0,
                    None,
                    &mut active,
                    &mut scratch,
                );
                if !ok {
                    return None;
                }
                if document {
                    buf.push('\n');
                }
                Some(Value::String(PklString::from(buf)))
            }
            Format::Yaml => {
                let mut yaml = super::yaml::Emitter::new(super::yaml::Mode::Compat, 2);
                let ok = self.write_fast_yaml_value(
                    root,
                    &mut yaml,
                    true,
                    None,
                    &mut active,
                    &mut scratch,
                );
                if !ok {
                    return None;
                }
                if document {
                    yaml.newline();
                }
                Some(Value::String(yaml.finish()))
            }
            Format::Plist => {
                if document {
                    let valid = matches!(root, Value::Object(id) if self.heap.get_fast(*id).is_some_and(|o| o.pair.is_none() && o.reference.is_none()));
                    if !valid {
                        return None;
                    }
                }
                let mut buf = String::with_capacity(4096);
                if document {
                    buf.push_str(super::plist::DOCUMENT_HEADER);
                }
                let ok = self.write_fast_plist_value(
                    root,
                    &mut buf,
                    "  ",
                    0,
                    None,
                    &mut active,
                    &mut scratch,
                );
                if !ok {
                    return None;
                }
                if document {
                    buf.push_str("\n</plist>\n");
                }
                Some(Value::String(PklString::from(buf)))
            }
            Format::Xml => {
                let mut buf = String::with_capacity(4096);
                let mut xml_line = 0usize;
                if document {
                    buf.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<root>");
                    xml_line = 1;
                    if !self.write_fast_xml_visit(
                        root,
                        &mut buf,
                        "  ",
                        false,
                        1,
                        &mut xml_line,
                        true,
                        None,
                        &mut active,
                        &mut scratch,
                    ) {
                        return None;
                    }
                    if 1 < xml_line {
                        buf.push('\n');
                    }
                    buf.push_str("</root>\n");
                } else if !self.write_fast_xml_visit(
                    root,
                    &mut buf,
                    "  ",
                    false,
                    0,
                    &mut xml_line,
                    false,
                    None,
                    &mut active,
                    &mut scratch,
                ) {
                    return None;
                }
                Some(Value::String(PklString::from(buf)))
            }
            Format::Properties => {
                if !document {
                    return None;
                }
                let Value::Object(root_id) = *root else {
                    return None;
                };
                if self.properties_document_value(root).ok() != Some(true)
                    || self.is_render_directive(root_id).ok() != Some(false)
                {
                    return None;
                }
                let mut buf = String::with_capacity(4096);
                let mut prefix = String::with_capacity(64);
                active.push(root_id);
                let ok = self.write_fast_properties_object(
                    root_id,
                    &mut buf,
                    &mut prefix,
                    false,
                    true,
                    false,
                    None,
                    &mut active,
                    &mut scratch,
                );
                active.pop();
                if !ok {
                    return None;
                }
                Some(Value::String(PklString::from(buf)))
            }
            Format::Protobuf => None,
        }
    }

    fn try_fast_render_document(&self, run: &mut RenderRun) -> bool {
        match run.format {
            Format::PklBinary => {
                if let Some(bytes) = self.try_fast_encode_pkl_binary(&run.root) {
                    run.binary = super::pkl_binary_wire::BinaryWire::default();
                    return run.binary.raw(&bytes).is_ok();
                }
                false
            }
            Format::Pcf => {
                let Some(indent) = run.indent.as_str() else {
                    return false;
                };
                if run.document && self.pcf_document_value(&run.root).ok() != Some(true) {
                    return false;
                }
                let mut buf = String::with_capacity(4096);
                let mut active = Vec::with_capacity(8);
                let mut scratch = Vec::with_capacity(16);
                let ok = self.write_fast_pcf_value(
                    &run.root,
                    &mut buf,
                    indent,
                    run.omit_null,
                    run.custom,
                    run.document,
                    0,
                    false,
                    true,
                    false,
                    false,
                    None,
                    &mut active,
                    &mut scratch,
                );
                if !ok {
                    return false;
                }
                if run.document && !buf.is_empty() {
                    buf.push('\n');
                }
                run.output.push_str(&buf);
                true
            }
            Format::Json => {
                let Some(indent) = run.indent.as_str() else {
                    return false;
                };
                let mut buf = String::with_capacity(4096);
                let mut active = Vec::with_capacity(8);
                let mut scratch = Vec::with_capacity(16);
                let ok = self.write_fast_json_value(
                    &run.root,
                    &mut buf,
                    indent,
                    run.omit_null,
                    0,
                    None,
                    &mut active,
                    &mut scratch,
                );
                if !ok {
                    return false;
                }
                if run.document {
                    buf.push('\n');
                }
                run.output.push_str(&buf);
                true
            }
            Format::Jsonnet => {
                let Some(indent) = run.indent.as_str() else {
                    return false;
                };
                let mut buf = String::with_capacity(4096);
                let mut active = Vec::with_capacity(8);
                let mut scratch = Vec::with_capacity(16);
                let ok = self.write_fast_jsonnet_value(
                    &run.root,
                    &mut buf,
                    indent,
                    run.omit_null,
                    0,
                    None,
                    &mut active,
                    &mut scratch,
                );
                if !ok {
                    return false;
                }
                if run.document {
                    buf.push('\n');
                }
                run.output.push_str(&buf);
                true
            }
            Format::Yaml => {
                if run.document && run.yaml_stream {
                    return false;
                }
                let Some(yaml) = run.yaml.as_mut() else {
                    return false;
                };
                if !yaml.is_utf8() {
                    return false;
                }
                let cp = yaml.checkpoint();
                let mut active = Vec::with_capacity(8);
                let mut scratch = Vec::with_capacity(16);
                let ok = self.write_fast_yaml_value(
                    &run.root,
                    yaml,
                    run.omit_null,
                    None,
                    &mut active,
                    &mut scratch,
                );
                if !ok {
                    yaml.rollback(cp);
                    return false;
                }
                if run.document {
                    yaml.newline();
                }
                true
            }
            Format::Plist => {
                let Some(indent) = run.indent.as_str() else {
                    return false;
                };
                if run.document {
                    let valid = matches!(&run.root, Value::Object(id) if self.heap.get_fast(*id).is_some_and(|o| o.pair.is_none() && o.reference.is_none()));
                    if !valid {
                        return false;
                    }
                }
                let mut buf = String::with_capacity(4096);
                if run.document {
                    buf.push_str(super::plist::DOCUMENT_HEADER);
                }
                let mut active = Vec::with_capacity(8);
                let mut scratch = Vec::with_capacity(16);
                let ok = self.write_fast_plist_value(
                    &run.root,
                    &mut buf,
                    indent,
                    0,
                    None,
                    &mut active,
                    &mut scratch,
                );
                if !ok {
                    return false;
                }
                if run.document {
                    buf.push_str("\n</plist>\n");
                }
                run.output.push_str(&buf);
                true
            }
            Format::Xml => self.try_fast_render_xml_document(run),
            Format::Properties => {
                if !run.document {
                    return false;
                }
                let Value::Object(root_id) = run.root else {
                    return false;
                };
                if self.properties_document_value(&run.root).ok() != Some(true)
                    || self.is_render_directive(root_id).ok() != Some(false)
                {
                    return false;
                }
                let mut buf = String::with_capacity(4096);
                let mut prefix = String::with_capacity(64);
                let mut active = Vec::with_capacity(8);
                let mut scratch = Vec::with_capacity(64);
                active.push(root_id);
                let ok = self.write_fast_properties_object(
                    root_id,
                    &mut buf,
                    &mut prefix,
                    false,
                    run.omit_null,
                    run.restrict_charset,
                    None,
                    &mut active,
                    &mut scratch,
                );
                active.pop();
                if !ok {
                    return false;
                }
                run.output.push_str(&buf);
                true
            }
            Format::Protobuf => false,
        }
    }

    fn try_fast_render_xml_document(&self, run: &mut RenderRun) -> bool {
        let Some(indent) = run.indent.as_str() else {
            return false;
        };
        let Some(Value::String(version)) = run.options.get(1) else {
            return false;
        };
        let Some(version_str) = version.as_str() else {
            return false;
        };
        let version11 = version_str == "1.1";
        let mut buf = String::with_capacity(4096);
        let mut active = Vec::with_capacity(8);
        let mut scratch = Vec::with_capacity(16);
        let mut xml_line = 0usize;
        if run.document {
            let (Some(Value::String(root_name)), Some(Value::Object(attrs_id))) =
                (run.options.get(2), run.options.get(3))
            else {
                return false;
            };
            let Some(root_name_str) = root_name.as_str() else {
                return false;
            };
            if !super::xml::valid_name_str(root_name_str, version11) {
                return false;
            }
            // Only take fast path when rootElementAttributes is an empty Mapping (default).
            let Some(attrs_obj) = self.heap.get_fast(*attrs_id) else {
                return false;
            };
            if attrs_obj.collection_kind != CollectionKind::Mapping
                || !attrs_obj.collection_members.is_empty()
                || !attrs_obj.members.is_empty()
                || !self.parent_is_empty_builtin(
                    attrs_obj.parent,
                    CollectionKind::Mapping,
                    classes::BuiltinClass::Mapping,
                )
            {
                return false;
            }
            buf.push_str("<?xml version=\"");
            super::plist::write_escaped_str(&mut buf, version_str);
            buf.push_str("\" encoding=\"UTF-8\"?>\n<");
            buf.push_str(root_name_str);
            buf.push('>');
            xml_line = 1;
            let open_line = xml_line;
            if !self.write_fast_xml_visit(
                &run.root,
                &mut buf,
                indent,
                version11,
                1,
                &mut xml_line,
                true,
                None,
                &mut active,
                &mut scratch,
            ) {
                return false;
            }
            if open_line < xml_line {
                buf.push('\n');
            }
            buf.push_str("</");
            buf.push_str(root_name_str);
            buf.push_str(">\n");
        } else if !self.write_fast_xml_visit(
            &run.root,
            &mut buf,
            indent,
            version11,
            0,
            &mut xml_line,
            false,
            None,
            &mut active,
            &mut scratch,
        ) {
            return false;
        }
        run.output.push_str(&buf);
        true
    }

    #[allow(clippy::too_many_arguments)]
    fn try_fast_render_open_object(
        &self,
        run: &mut RenderRun,
        receiver: ObjectId,
        depth: usize,
        standalone: bool,
        root: bool,
        path: &[PathPart],
    ) -> bool {
        let mut active = Vec::with_capacity(8);
        let mut scratch = Vec::with_capacity(32);
        match run.format {
            Format::PklBinary => {
                let cp = run.binary.len();
                if self.try_fast_encode_binary_object(
                    receiver,
                    &mut run.binary,
                    Some(&run.active),
                    &mut active,
                    &mut scratch,
                ) {
                    true
                } else {
                    run.binary.truncate(cp);
                    false
                }
            }
            Format::Pcf => {
                let Some(indent) = run.indent.as_str() else {
                    return false;
                };
                let mut buf = String::with_capacity(256);
                let ok = self.write_fast_pcf_object(
                    receiver,
                    &mut buf,
                    indent,
                    run.omit_null,
                    run.custom,
                    run.document,
                    depth,
                    standalone,
                    root,
                    !run.output_empty(),
                    Some(&run.active),
                    &mut active,
                    &mut scratch,
                );
                if ok {
                    run.output.push_str(&buf);
                }
                ok
            }
            Format::Json => {
                let Some(indent) = run.indent.as_str() else {
                    return false;
                };
                let mut buf = String::with_capacity(256);
                let ok = self.write_fast_json_object(
                    receiver,
                    &mut buf,
                    indent,
                    run.omit_null,
                    depth,
                    Some(&run.active),
                    &mut active,
                    &mut scratch,
                );
                if ok {
                    run.output.push_str(&buf);
                }
                ok
            }
            Format::Jsonnet => {
                let Some(indent) = run.indent.as_str() else {
                    return false;
                };
                let mut buf = String::with_capacity(256);
                let ok = self.write_fast_jsonnet_object(
                    receiver,
                    &mut buf,
                    indent,
                    run.omit_null,
                    depth,
                    Some(&run.active),
                    &mut active,
                    &mut scratch,
                );
                if ok {
                    run.output.push_str(&buf);
                }
                ok
            }
            Format::Yaml => {
                let Some(yaml) = run.yaml.as_mut() else {
                    return false;
                };
                if !yaml.is_utf8() {
                    return false;
                }
                let cp = yaml.checkpoint();
                let ok = self.write_fast_yaml_object(
                    receiver,
                    yaml,
                    run.omit_null,
                    Some(&run.active),
                    &mut active,
                    &mut scratch,
                );
                if !ok {
                    yaml.rollback(cp);
                }
                ok
            }
            Format::Plist => {
                let Some(indent) = run.indent.as_str() else {
                    return false;
                };
                let mut buf = String::with_capacity(256);
                let ok = self.write_fast_plist_object(
                    receiver,
                    &mut buf,
                    indent,
                    depth,
                    Some(&run.active),
                    &mut active,
                    &mut scratch,
                );
                if ok {
                    run.output.push_str(&buf);
                }
                ok
            }
            Format::Xml => {
                let Some(indent) = run.indent.as_str() else {
                    return false;
                };
                let Some(Value::String(version)) = run.options.get(1) else {
                    return false;
                };
                let Some(version_str) = version.as_str() else {
                    return false;
                };
                let version11 = version_str == "1.1";
                let mut buf = String::with_capacity(256);
                let mut xml_line = run.xml_line;
                let ok = self.write_fast_xml_object(
                    receiver,
                    &mut buf,
                    indent,
                    version11,
                    depth,
                    &mut xml_line,
                    !run.output_empty(),
                    Some(&run.active),
                    &mut active,
                    &mut scratch,
                );
                if ok {
                    run.output.push_str(&buf);
                    run.xml_line = xml_line;
                }
                ok
            }
            Format::Properties => {
                if !run.properties_document {
                    return false;
                }
                let mut prefix = String::with_capacity(64);
                let mut has_prefix = false;
                for part in path {
                    match part {
                        PathPart::Root => {}
                        PathPart::Property(name) => {
                            let Some(s) = name.as_str() else {
                                return false;
                            };
                            if has_prefix {
                                prefix.push('.');
                            }
                            has_prefix = true;
                            write_properties_str(&mut prefix, s, true, run.restrict_charset);
                        }
                        PathPart::Entry(Value::String(name)) => {
                            let Some(s) = name.as_str() else {
                                return false;
                            };
                            if has_prefix {
                                prefix.push('.');
                            }
                            has_prefix = true;
                            write_properties_str(&mut prefix, s, true, run.restrict_charset);
                        }
                        _ => return false,
                    }
                }
                let mut buf = String::with_capacity(1024);
                let ok = self.write_fast_properties_object(
                    receiver,
                    &mut buf,
                    &mut prefix,
                    has_prefix,
                    run.omit_null,
                    run.restrict_charset,
                    Some(&run.active),
                    &mut active,
                    &mut scratch,
                );
                if ok {
                    run.output.push_str(&buf);
                }
                ok
            }
            Format::Protobuf => false,
        }
    }

    fn advance_fast_list(
        &mut self,
        run: &mut RenderRun,
        receiver: ObjectId,
        mut index: usize,
        depth: usize,
        path: Vec<PathPart>,
    ) -> Result<bool, RuntimeError> {
        let len = self
            .heap
            .get_fast(receiver)
            .and_then(|o| o.list_elements.as_ref())
            .map_or(0, Vec::len);
        let format = run.format;
        let child_depth = depth
            + usize::from(matches!(
                format,
                Format::Json | Format::Jsonnet | Format::Plist
            ));
        let standalone = format == Format::Pcf;
        let mut local_active = Vec::with_capacity(8);
        let mut text_buf = String::new();
        let mut pending: Option<(usize, Value)> = None;

        while index < len {
            if let Some(child_id) = self
                .heap
                .get_fast(receiver)
                .and_then(|o| o.list_elements.as_ref())
                .and_then(|e| e.get(index))
                .and_then(|v| match v {
                    Value::Object(id) => Some(*id),
                    _ => None,
                })
                && !run.active.contains(&child_id)
            {
                if self
                    .heap
                    .get_fast(child_id)
                    .is_some_and(|o| !o.forced && o.cache_slots.len() < o.members.len())
                    && self.is_plain_shallow_forceable(child_id)
                {
                    let _ = self.try_force_plain_object_sync(child_id)?;
                } else if self.heap.get_fast(child_id).is_some_and(|o| !o.forced) {
                    let _ = self.try_force_plain_typed_sync(child_id, 0)?;
                }
            }
            let mut scratch = Vec::with_capacity(16);
            let Some(elem) = self
                .heap
                .get_fast(receiver)
                .and_then(|o| o.list_elements.as_ref())
                .and_then(|e| e.get(index))
            else {
                return Err(invalid("fast list element"));
            };
            match format {
                Format::PklBinary => {
                    let cp = run.binary.len();
                    local_active.clear();
                    scratch.clear();
                    if self.try_fast_encode_binary_value(
                        elem,
                        &mut run.binary,
                        Some(&run.active),
                        &mut local_active,
                        &mut scratch,
                    ) {
                        index += 1;
                        continue;
                    }
                    run.binary.truncate(cp);
                    pending = Some((index, elem.clone()));
                    break;
                }
                Format::Yaml => {
                    let yaml = run.yaml.as_mut().unwrap();
                    yaml.member(true, index == 0);
                    let cp = yaml.checkpoint();
                    local_active.clear();
                    scratch.clear();
                    if yaml.is_utf8()
                        && self.write_fast_yaml_value(
                            elem,
                            yaml,
                            run.omit_null,
                            Some(&run.active),
                            &mut local_active,
                            &mut scratch,
                        )
                    {
                        index += 1;
                        continue;
                    }
                    yaml.rollback(cp);
                    pending = Some((index, elem.clone()));
                    break;
                }
                Format::Pcf | Format::Json | Format::Jsonnet | Format::Plist => {
                    let indent = run.indent.as_str().unwrap_or("");
                    match format {
                        Format::Pcf => {
                            if index > 0 {
                                text_buf.push_str(", ");
                            }
                        }
                        Format::Json => {
                            if index > 0 {
                                text_buf.push(',');
                            }
                            if !indent.is_empty() {
                                text_buf.push('\n');
                                push_indent_str(&mut text_buf, indent, child_depth);
                            }
                        }
                        Format::Jsonnet => {
                            if index > 0 {
                                text_buf.push(',');
                            }
                            if !indent.is_empty() {
                                text_buf.push('\n');
                                push_indent_str(&mut text_buf, indent, child_depth);
                            } else if index > 0 {
                                text_buf.push(' ');
                            }
                        }
                        Format::Plist => {
                            push_indent_str(&mut text_buf, indent, child_depth);
                        }
                        _ => unreachable!(),
                    }
                    let cp = text_buf.len();
                    local_active.clear();
                    scratch.clear();
                    let ok = match format {
                        Format::Pcf => self.write_fast_pcf_value(
                            elem,
                            &mut text_buf,
                            indent,
                            run.omit_null,
                            run.custom,
                            run.document,
                            child_depth,
                            true,
                            false,
                            false,
                            true,
                            Some(&run.active),
                            &mut local_active,
                            &mut scratch,
                        ),
                        Format::Json => self.write_fast_json_value(
                            elem,
                            &mut text_buf,
                            indent,
                            run.omit_null,
                            child_depth,
                            Some(&run.active),
                            &mut local_active,
                            &mut scratch,
                        ),
                        Format::Jsonnet => self.write_fast_jsonnet_value(
                            elem,
                            &mut text_buf,
                            indent,
                            run.omit_null,
                            child_depth,
                            Some(&run.active),
                            &mut local_active,
                            &mut scratch,
                        ),
                        Format::Plist => {
                            let ok = self.write_fast_plist_value(
                                elem,
                                &mut text_buf,
                                indent,
                                child_depth,
                                Some(&run.active),
                                &mut local_active,
                                &mut scratch,
                            );
                            if ok {
                                text_buf.push('\n');
                            }
                            ok
                        }
                        _ => unreachable!(),
                    };
                    if ok {
                        index += 1;
                        continue;
                    }
                    text_buf.truncate(cp);
                    pending = Some((index, elem.clone()));
                    break;
                }
                _ => unreachable!("fast list format"),
            }
        }

        if !text_buf.is_empty() {
            run.output.push_str(&text_buf);
        }

        if let Some((elem_idx, elem)) = pending {
            let mut elem_path = path.clone();
            elem_path.push(PathPart::Entry(Value::Int((elem_idx as i64).into())));
            run.tasks.push(Task::FastList {
                receiver,
                index: elem_idx + 1,
                depth,
                path,
            });
            if format == Format::Plist {
                run.tasks.push(Task::Text("\n".into()));
            }
            if let Value::Object(child_id) = elem
                && !run.active.contains(&child_id)
                && self.is_plain_unforced_dynamic(child_id)
            {
                run.active.insert(child_id);
                run.tasks.push(Task::OpenObject {
                    receiver: child_id,
                    depth: child_depth,
                    standalone,
                    root: false,
                    path: elem_path,
                    stream: false,
                });
                self.begin_shallow_force_for_render(run, child_id)?;
            } else {
                run.value(elem, child_depth, standalone, false, false, elem_path);
            }
            return Ok(false);
        }

        match format {
            Format::PklBinary => {}
            Format::Yaml => {
                run.yaml.as_mut().unwrap().close(len == 0);
            }
            Format::Pcf => run.append(")"),
            Format::Json => {
                if len > 0 {
                    run.json_newline(depth);
                }
                run.append("]");
            }
            Format::Jsonnet => {
                if len > 0 && !run.indent.is_empty() {
                    run.append(",");
                    run.json_newline(depth);
                }
                run.append("]");
            }
            Format::Plist => {
                let indent = run.indent.as_str().unwrap_or("");
                let mut close = String::with_capacity(indent.len() * depth + 8);
                push_indent_str(&mut close, indent, depth);
                close.push_str("</array>");
                run.output.push_str(&close);
            }
            _ => unreachable!(),
        }
        run.active.remove(&receiver);
        Ok(false)
    }
}

impl Vm {
    #[allow(clippy::too_many_arguments)]
    fn write_fast_pcf_value<'a>(
        &'a self,
        value: &'a Value,
        buf: &mut String,
        indent: &str,
        omit_null: bool,
        custom: bool,
        document: bool,
        depth: usize,
        standalone: bool,
        root: bool,
        string_element: bool,
        output_was_non_empty: bool,
        outer_active: Option<&BTreeSet<ObjectId>>,
        active: &mut Vec<ObjectId>,
        scratch: &mut Vec<FastMember<'a>>,
    ) -> bool {
        match value {
            Value::Null | Value::NullWithDefault(_) => buf.push_str("null"),
            Value::Bool(v) => buf.push_str(if *v { "true" } else { "false" }),
            Value::Int(v) => {
                let mut num = [0u8; 24];
                buf.push_str(super::format_i64_buf(v.get(), &mut num));
            }
            Value::Float(v) => buf.push_str(&crate::numeric::double_to_string(**v)),
            Value::Duration(v) => {
                use std::fmt::Write;
                let _ = write!(buf, "{v}");
            }
            Value::DataSize(v) => {
                use std::fmt::Write;
                let _ = write!(buf, "{v}");
            }
            Value::IntSeq(v) => {
                use std::fmt::Write;
                let _ = write!(buf, "{v}");
            }
            Value::String(s) => {
                return super::pcf_format::write_string(
                    buf,
                    s,
                    indent,
                    depth + usize::from(!string_element),
                    custom,
                );
            }
            Value::Bytes(bytes) => {
                buf.push_str("Bytes(");
                let mut num = [0u8; 24];
                for (i, &b) in bytes.iter().enumerate() {
                    if i != 0 {
                        buf.push_str(", ");
                    }
                    buf.push_str(super::format_i64_buf(i64::from(b), &mut num));
                }
                buf.push(')');
            }
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
                    buf.push_str("Pair(");
                    if self.write_fast_pcf_value(
                        &pair[0],
                        buf,
                        indent,
                        omit_null,
                        custom,
                        document,
                        depth,
                        true,
                        false,
                        false,
                        true,
                        outer_active,
                        active,
                        scratch,
                    ) {
                        buf.push_str(", ");
                        if self.write_fast_pcf_value(
                            &pair[1],
                            buf,
                            indent,
                            omit_null,
                            custom,
                            document,
                            depth,
                            true,
                            false,
                            false,
                            true,
                            outer_active,
                            active,
                            scratch,
                        ) {
                            buf.push(')');
                            true
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                } else {
                    self.write_fast_pcf_object(
                        *id,
                        buf,
                        indent,
                        omit_null,
                        custom,
                        document,
                        depth,
                        standalone,
                        root,
                        output_was_non_empty,
                        outer_active,
                        active,
                        scratch,
                    )
                };
                active.pop();
                return ok;
            }
            _ => return false,
        }
        true
    }

    #[allow(clippy::too_many_arguments)]
    fn write_fast_pcf_object<'a>(
        &'a self,
        id: ObjectId,
        buf: &mut String,
        indent: &str,
        omit_null: bool,
        custom: bool,
        document: bool,
        depth: usize,
        standalone: bool,
        root: bool,
        output_was_non_empty: bool,
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
                buf.push_str("List(");
                let mut ok = true;
                for (i, elem) in elems.iter().enumerate() {
                    if i > 0 {
                        buf.push_str(", ");
                    }
                    if !self.write_fast_pcf_value(
                        elem,
                        buf,
                        indent,
                        omit_null,
                        custom,
                        document,
                        depth,
                        true,
                        false,
                        false,
                        true,
                        outer_active,
                        active,
                        scratch,
                    ) {
                        ok = false;
                        break;
                    }
                }
                buf.push(')');
                ok
            }
            FastObjectKind::EmptyList => {
                buf.push_str("List()");
                true
            }
            FastObjectKind::StrictSet(count) => {
                let end = base + count;
                buf.push_str("Set(");
                let mut ok = true;
                for i in base..end {
                    if i > base {
                        buf.push_str(", ");
                    }
                    let FastMember::Element(_, elem) = scratch[i] else {
                        ok = false;
                        break;
                    };
                    if !self.write_fast_pcf_value(
                        elem,
                        buf,
                        indent,
                        omit_null,
                        custom,
                        document,
                        depth,
                        true,
                        false,
                        false,
                        true,
                        outer_active,
                        active,
                        scratch,
                    ) {
                        ok = false;
                        break;
                    }
                }
                buf.push(')');
                ok
            }
            FastObjectKind::StrictMap(count) => {
                let end = base + count;
                buf.push_str("Map(");
                let mut ok = true;
                for i in base..end {
                    if i > base {
                        buf.push_str(", ");
                    }
                    let FastMember::Entry(k, v) = scratch[i] else {
                        ok = false;
                        break;
                    };
                    if !self.write_fast_pcf_value(
                        k,
                        buf,
                        indent,
                        omit_null,
                        custom,
                        document,
                        depth,
                        true,
                        false,
                        false,
                        true,
                        outer_active,
                        active,
                        scratch,
                    ) {
                        ok = false;
                        break;
                    }
                    buf.push_str(", ");
                    if !self.write_fast_pcf_value(
                        v,
                        buf,
                        indent,
                        omit_null,
                        custom,
                        document,
                        depth,
                        true,
                        false,
                        false,
                        true,
                        outer_active,
                        active,
                        scratch,
                    ) {
                        ok = false;
                        break;
                    }
                }
                buf.push(')');
                ok
            }
            FastObjectKind::PlainDynamic { count, .. }
            | FastObjectKind::PlainListing(count)
            | FastObjectKind::PlainMapping(count)
            | FastObjectKind::PlainModule { count, .. }
            | FastObjectKind::PlainTyped { count } => {
                let end = base + count;
                let doc = document && root;
                if !doc {
                    if standalone {
                        buf.push_str("new ");
                    }
                    buf.push('{');
                }
                let child_depth = depth + usize::from(!doc);
                let mut emitted = false;
                let mut ok = true;
                for i in base..end {
                    match scratch[i] {
                        FastMember::Property(name, val) => {
                            if omit_null && val.is_null() {
                                continue;
                            }
                            let Some(name_str) = name.as_str() else {
                                ok = false;
                                break;
                            };
                            if output_was_non_empty || !buf.is_empty() {
                                buf.push('\n');
                                push_indent_str(buf, indent, child_depth);
                            }
                            if crate::parser::is_regular_identifier(name_str) {
                                buf.push_str(name_str);
                            } else {
                                buf.push('`');
                                buf.push_str(name_str);
                                buf.push('`');
                            }
                            let Ok(obj_like) = self.pcf_object_like(val) else {
                                ok = false;
                                break;
                            };
                            buf.push_str(if obj_like { " " } else { " = " });
                            emitted = true;
                            if !self.write_fast_pcf_value(
                                val,
                                buf,
                                indent,
                                omit_null,
                                custom,
                                document,
                                child_depth,
                                false,
                                false,
                                false,
                                true,
                                outer_active,
                                active,
                                scratch,
                            ) {
                                ok = false;
                                break;
                            }
                        }
                        FastMember::Element(_, val) => {
                            buf.push('\n');
                            push_indent_str(buf, indent, child_depth);
                            emitted = true;
                            if !self.write_fast_pcf_value(
                                val,
                                buf,
                                indent,
                                omit_null,
                                custom,
                                document,
                                child_depth,
                                true,
                                false,
                                true,
                                true,
                                outer_active,
                                active,
                                scratch,
                            ) {
                                ok = false;
                                break;
                            }
                        }
                        FastMember::Entry(key, val) => {
                            buf.push('\n');
                            push_indent_str(buf, indent, child_depth);
                            buf.push('[');
                            if !self.write_fast_pcf_value(
                                key,
                                buf,
                                indent,
                                omit_null,
                                custom,
                                document,
                                child_depth,
                                true,
                                false,
                                false,
                                true,
                                outer_active,
                                active,
                                scratch,
                            ) {
                                ok = false;
                                break;
                            }
                            let Ok(obj_like) = self.pcf_object_like(val) else {
                                ok = false;
                                break;
                            };
                            buf.push_str(if obj_like { "] " } else { "] = " });
                            emitted = true;
                            if !self.write_fast_pcf_value(
                                val,
                                buf,
                                indent,
                                omit_null,
                                custom,
                                document,
                                child_depth,
                                false,
                                false,
                                false,
                                true,
                                outer_active,
                                active,
                                scratch,
                            ) {
                                ok = false;
                                break;
                            }
                        }
                    }
                }
                if ok && !doc {
                    if emitted {
                        buf.push('\n');
                        push_indent_str(buf, indent, depth);
                    }
                    buf.push('}');
                }
                ok
            }
        };
        scratch.truncate(base);
        ok
    }

    #[allow(clippy::too_many_arguments)]
    fn write_fast_json_value<'a>(
        &'a self,
        value: &'a Value,
        buf: &mut String,
        indent: &str,
        omit_null: bool,
        depth: usize,
        outer_active: Option<&BTreeSet<ObjectId>>,
        active: &mut Vec<ObjectId>,
        scratch: &mut Vec<FastMember<'a>>,
    ) -> bool {
        match value {
            Value::Null | Value::NullWithDefault(_) => buf.push_str("null"),
            Value::Bool(v) => buf.push_str(if *v { "true" } else { "false" }),
            Value::Int(v) => {
                let mut num = [0u8; 24];
                buf.push_str(super::format_i64_buf(v.get(), &mut num));
            }
            Value::Float(v) => {
                if !v.is_finite() {
                    return false;
                }
                buf.push_str(&crate::numeric::double_to_string(**v));
            }
            Value::String(s) => {
                let Some(text) = s.as_str() else {
                    return false;
                };
                write_json_str(buf, text);
            }
            Value::Object(id) => {
                if active.len() >= 32
                    || active.contains(id)
                    || outer_active.is_some_and(|set| set.contains(id))
                {
                    return false;
                }
                active.push(*id);
                let ok = self.write_fast_json_object(
                    *id,
                    buf,
                    indent,
                    omit_null,
                    depth,
                    outer_active,
                    active,
                    scratch,
                );
                active.pop();
                return ok;
            }
            _ => return false,
        }
        true
    }

    #[allow(clippy::too_many_arguments)]
    fn write_fast_json_object<'a>(
        &'a self,
        id: ObjectId,
        buf: &mut String,
        indent: &str,
        omit_null: bool,
        depth: usize,
        outer_active: Option<&BTreeSet<ObjectId>>,
        active: &mut Vec<ObjectId>,
        scratch: &mut Vec<FastMember<'a>>,
    ) -> bool {
        let base = scratch.len();
        let Some(kind) = self.extract_fast_object(id, scratch, false) else {
            return false;
        };
        let child_depth = depth + 1;
        let colon = if indent.is_empty() { ":" } else { ": " };
        let ok = match kind {
            FastObjectKind::StrictList(elems) => {
                buf.push('[');
                let mut ok = true;
                for (i, elem) in elems.iter().enumerate() {
                    if i > 0 {
                        buf.push(',');
                    }
                    if !indent.is_empty() {
                        buf.push('\n');
                        push_indent_str(buf, indent, child_depth);
                    }
                    if !self.write_fast_json_value(
                        elem,
                        buf,
                        indent,
                        omit_null,
                        child_depth,
                        outer_active,
                        active,
                        scratch,
                    ) {
                        ok = false;
                        break;
                    }
                }
                if ok && !elems.is_empty() && !indent.is_empty() {
                    buf.push('\n');
                    push_indent_str(buf, indent, depth);
                }
                buf.push(']');
                ok
            }
            FastObjectKind::EmptyList => {
                buf.push_str("[]");
                true
            }
            FastObjectKind::StrictSet(count) | FastObjectKind::PlainListing(count) => {
                let end = base + count;
                buf.push('[');
                let mut ok = true;
                for i in base..end {
                    if i > base {
                        buf.push(',');
                    }
                    if !indent.is_empty() {
                        buf.push('\n');
                        push_indent_str(buf, indent, child_depth);
                    }
                    let FastMember::Element(_, elem) = scratch[i] else {
                        ok = false;
                        break;
                    };
                    if !self.write_fast_json_value(
                        elem,
                        buf,
                        indent,
                        omit_null,
                        child_depth,
                        outer_active,
                        active,
                        scratch,
                    ) {
                        ok = false;
                        break;
                    }
                }
                if ok && count > 0 && !indent.is_empty() {
                    buf.push('\n');
                    push_indent_str(buf, indent, depth);
                }
                buf.push(']');
                ok
            }
            FastObjectKind::PlainDynamic {
                count,
                has_elements: true,
                has_keyed: false,
            } => {
                let end = base + count;
                buf.push('[');
                let mut ok = true;
                for i in base..end {
                    if i > base {
                        buf.push(',');
                    }
                    if !indent.is_empty() {
                        buf.push('\n');
                        push_indent_str(buf, indent, child_depth);
                    }
                    let FastMember::Element(_, elem) = scratch[i] else {
                        ok = false;
                        break;
                    };
                    if !self.write_fast_json_value(
                        elem,
                        buf,
                        indent,
                        omit_null,
                        child_depth,
                        outer_active,
                        active,
                        scratch,
                    ) {
                        ok = false;
                        break;
                    }
                }
                if ok && count > 0 && !indent.is_empty() {
                    buf.push('\n');
                    push_indent_str(buf, indent, depth);
                }
                buf.push(']');
                ok
            }
            FastObjectKind::PlainDynamic {
                has_elements: true,
                has_keyed: true,
                ..
            } => false,
            FastObjectKind::StrictMap(count)
            | FastObjectKind::PlainMapping(count)
            | FastObjectKind::PlainDynamic {
                count,
                has_elements: false,
                ..
            }
            | FastObjectKind::PlainModule { count, .. }
            | FastObjectKind::PlainTyped { count } => {
                let is_strict_map = matches!(kind, FastObjectKind::StrictMap(_));
                let end = base + count;
                buf.push('{');
                let mut emitted = false;
                let mut ok = true;
                for i in base..end {
                    let (key_str, val) = match scratch[i] {
                        FastMember::Property(name, val) => (name.as_str(), val),
                        FastMember::Entry(Value::String(name), val) => (name.as_str(), val),
                        _ => {
                            ok = false;
                            break;
                        }
                    };
                    if omit_null && val.is_null() {
                        continue;
                    }
                    let Some(key_str) = key_str else {
                        ok = false;
                        break;
                    };
                    if emitted {
                        buf.push(',');
                    }
                    if !indent.is_empty() {
                        buf.push('\n');
                        push_indent_str(buf, indent, child_depth);
                    }
                    emitted = true;
                    write_json_str(buf, key_str);
                    buf.push_str(colon);
                    if !self.write_fast_json_value(
                        val,
                        buf,
                        indent,
                        omit_null,
                        child_depth,
                        outer_active,
                        active,
                        scratch,
                    ) {
                        ok = false;
                        break;
                    }
                }
                if ok && is_strict_map && count > 0 && !emitted {
                    ok = false;
                }
                if ok && emitted && !indent.is_empty() {
                    buf.push('\n');
                    push_indent_str(buf, indent, depth);
                }
                buf.push('}');
                ok
            }
        };
        scratch.truncate(base);
        ok
    }

    #[allow(clippy::too_many_arguments)]
    fn write_fast_jsonnet_value<'a>(
        &'a self,
        value: &'a Value,
        buf: &mut String,
        indent: &str,
        omit_null: bool,
        depth: usize,
        outer_active: Option<&BTreeSet<ObjectId>>,
        active: &mut Vec<ObjectId>,
        scratch: &mut Vec<FastMember<'a>>,
    ) -> bool {
        match value {
            Value::Null | Value::NullWithDefault(_) => buf.push_str("null"),
            Value::Bool(v) => buf.push_str(if *v { "true" } else { "false" }),
            Value::Int(v) => {
                let mut num = [0u8; 24];
                buf.push_str(super::format_i64_buf(v.get(), &mut num));
            }
            Value::Float(v) => buf.push_str(&crate::numeric::double_to_string(**v)),
            Value::String(s) => return super::jsonnet::write_string(buf, s, indent, depth),
            Value::Object(id) => {
                if active.len() >= 32
                    || active.contains(id)
                    || outer_active.is_some_and(|set| set.contains(id))
                {
                    return false;
                }
                active.push(*id);
                let ok = self.write_fast_jsonnet_object(
                    *id,
                    buf,
                    indent,
                    omit_null,
                    depth,
                    outer_active,
                    active,
                    scratch,
                );
                active.pop();
                return ok;
            }
            _ => return false,
        }
        true
    }

    #[allow(clippy::too_many_arguments)]
    fn write_fast_jsonnet_object<'a>(
        &'a self,
        id: ObjectId,
        buf: &mut String,
        indent: &str,
        omit_null: bool,
        depth: usize,
        outer_active: Option<&BTreeSet<ObjectId>>,
        active: &mut Vec<ObjectId>,
        scratch: &mut Vec<FastMember<'a>>,
    ) -> bool {
        let base = scratch.len();
        let Some(kind) = self.extract_fast_object(id, scratch, false) else {
            return false;
        };
        let child_depth = depth + 1;
        let ok = match kind {
            FastObjectKind::StrictList(elems) => {
                buf.push('[');
                let mut ok = true;
                for (i, elem) in elems.iter().enumerate() {
                    if i > 0 {
                        buf.push(',');
                    }
                    if !indent.is_empty() {
                        buf.push('\n');
                        push_indent_str(buf, indent, child_depth);
                    } else if i > 0 {
                        buf.push(' ');
                    }
                    if !self.write_fast_jsonnet_value(
                        elem,
                        buf,
                        indent,
                        omit_null,
                        child_depth,
                        outer_active,
                        active,
                        scratch,
                    ) {
                        ok = false;
                        break;
                    }
                }
                if ok && !elems.is_empty() && !indent.is_empty() {
                    buf.push(',');
                    buf.push('\n');
                    push_indent_str(buf, indent, depth);
                }
                buf.push(']');
                ok
            }
            FastObjectKind::EmptyList => {
                buf.push_str("[]");
                true
            }
            FastObjectKind::StrictSet(count)
            | FastObjectKind::PlainListing(count)
            | FastObjectKind::PlainDynamic {
                count,
                has_elements: true,
                has_keyed: false,
            } => {
                let end = base + count;
                buf.push('[');
                let mut ok = true;
                for i in base..end {
                    if i > base {
                        buf.push(',');
                    }
                    if !indent.is_empty() {
                        buf.push('\n');
                        push_indent_str(buf, indent, child_depth);
                    } else if i > base {
                        buf.push(' ');
                    }
                    let FastMember::Element(_, elem) = scratch[i] else {
                        ok = false;
                        break;
                    };
                    if !self.write_fast_jsonnet_value(
                        elem,
                        buf,
                        indent,
                        omit_null,
                        child_depth,
                        outer_active,
                        active,
                        scratch,
                    ) {
                        ok = false;
                        break;
                    }
                }
                if ok && count > 0 && !indent.is_empty() {
                    buf.push(',');
                    buf.push('\n');
                    push_indent_str(buf, indent, depth);
                }
                buf.push(']');
                ok
            }
            FastObjectKind::PlainDynamic {
                has_elements: true,
                has_keyed: true,
                ..
            } => false,
            FastObjectKind::StrictMap(count)
            | FastObjectKind::PlainMapping(count)
            | FastObjectKind::PlainDynamic {
                count,
                has_elements: false,
                ..
            }
            | FastObjectKind::PlainModule { count, .. }
            | FastObjectKind::PlainTyped { count } => {
                let is_strict_map = matches!(kind, FastObjectKind::StrictMap(_));
                let end = base + count;
                buf.push('{');
                let mut emitted = false;
                let mut ok = true;
                for i in base..end {
                    let (key, val) = match scratch[i] {
                        FastMember::Property(name, val) => (name, val),
                        FastMember::Entry(Value::String(name), val) => (name, val),
                        _ => {
                            ok = false;
                            break;
                        }
                    };
                    if omit_null && val.is_null() {
                        continue;
                    }
                    if emitted {
                        buf.push(',');
                    }
                    if !indent.is_empty() {
                        buf.push('\n');
                        push_indent_str(buf, indent, child_depth);
                    } else {
                        buf.push(' ');
                    }
                    emitted = true;
                    if !super::jsonnet::write_field_name(buf, key, indent, child_depth) {
                        ok = false;
                        break;
                    }
                    buf.push_str(": ");
                    if !self.write_fast_jsonnet_value(
                        val,
                        buf,
                        indent,
                        omit_null,
                        child_depth,
                        outer_active,
                        active,
                        scratch,
                    ) {
                        ok = false;
                        break;
                    }
                }
                if ok && is_strict_map && count > 0 && !emitted {
                    ok = false;
                }
                if ok && emitted {
                    if !indent.is_empty() {
                        buf.push(',');
                        buf.push('\n');
                        push_indent_str(buf, indent, depth);
                    } else {
                        buf.push(' ');
                    }
                }
                buf.push('}');
                ok
            }
        };
        scratch.truncate(base);
        ok
    }

    fn write_fast_yaml_value<'a>(
        &'a self,
        value: &'a Value,
        yaml: &mut super::yaml::Emitter,
        omit_null: bool,
        outer_active: Option<&BTreeSet<ObjectId>>,
        active: &mut Vec<ObjectId>,
        scratch: &mut Vec<FastMember<'a>>,
    ) -> bool {
        match value {
            Value::Null | Value::NullWithDefault(_) => yaml.scalar_str("null"),
            Value::Bool(v) => yaml.scalar_str(if *v { "true" } else { "false" }),
            Value::Int(v) => {
                let mut num = [0u8; 24];
                yaml.scalar_str(super::format_i64_buf(v.get(), &mut num));
            }
            Value::Float(v) => {
                if v.is_nan() {
                    yaml.scalar_str(".NaN");
                } else if **v == f64::INFINITY {
                    yaml.scalar_str(".Inf");
                } else if **v == f64::NEG_INFINITY {
                    yaml.scalar_str("-.Inf");
                } else {
                    yaml.scalar_str(&crate::numeric::double_to_string(**v));
                }
            }
            Value::String(s) => {
                if s.as_str().is_none() {
                    return false;
                }
                yaml.string_value(s);
            }
            Value::Bytes(bytes) => yaml.bytes(bytes),
            Value::Object(id) => {
                if active.len() >= 32
                    || active.contains(id)
                    || outer_active.is_some_and(|set| set.contains(id))
                {
                    return false;
                }
                active.push(*id);
                let ok = self.write_fast_yaml_object(
                    *id,
                    yaml,
                    omit_null,
                    outer_active,
                    active,
                    scratch,
                );
                active.pop();
                return ok;
            }
            _ => return false,
        }
        true
    }

    fn write_fast_yaml_object<'a>(
        &'a self,
        id: ObjectId,
        yaml: &mut super::yaml::Emitter,
        omit_null: bool,
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
                yaml.start(true);
                let mut ok = true;
                for (i, elem) in elems.iter().enumerate() {
                    yaml.member(true, i == 0);
                    if !self.write_fast_yaml_value(
                        elem,
                        yaml,
                        omit_null,
                        outer_active,
                        active,
                        scratch,
                    ) {
                        ok = false;
                        break;
                    }
                }
                if ok {
                    yaml.close(elems.is_empty());
                }
                ok
            }
            FastObjectKind::EmptyList => {
                yaml.start(true);
                yaml.close(true);
                true
            }
            FastObjectKind::StrictSet(count)
            | FastObjectKind::PlainListing(count)
            | FastObjectKind::PlainDynamic {
                count,
                has_elements: true,
                has_keyed: false,
            } => {
                let end = base + count;
                yaml.start(true);
                let mut ok = true;
                for i in base..end {
                    yaml.member(true, i == base);
                    let FastMember::Element(_, elem) = scratch[i] else {
                        ok = false;
                        break;
                    };
                    if !self.write_fast_yaml_value(
                        elem,
                        yaml,
                        omit_null,
                        outer_active,
                        active,
                        scratch,
                    ) {
                        ok = false;
                        break;
                    }
                }
                if ok {
                    yaml.close(count == 0);
                }
                ok
            }
            FastObjectKind::PlainDynamic {
                has_elements: true,
                has_keyed: true,
                ..
            } => false,
            FastObjectKind::StrictMap(count)
            | FastObjectKind::PlainMapping(count)
            | FastObjectKind::PlainDynamic {
                count,
                has_elements: false,
                ..
            }
            | FastObjectKind::PlainModule { count, .. }
            | FastObjectKind::PlainTyped { count } => {
                let is_strict_map = matches!(kind, FastObjectKind::StrictMap(_));
                let end = base + count;
                yaml.start(false);
                let mut emitted = false;
                let mut ok = true;
                for i in base..end {
                    match scratch[i] {
                        FastMember::Property(name, val) => {
                            if omit_null && val.is_null() {
                                continue;
                            }
                            if name.as_str().is_none() {
                                ok = false;
                                break;
                            }
                            yaml.member(false, !emitted);
                            emitted = true;
                            yaml.string(name, true);
                            yaml.text(":");
                            if !self.write_fast_yaml_value(
                                val,
                                yaml,
                                omit_null,
                                outer_active,
                                active,
                                scratch,
                            ) {
                                ok = false;
                                break;
                            }
                        }
                        FastMember::Entry(key, val) => {
                            if !matches!(
                                key,
                                Value::Null
                                    | Value::NullWithDefault(_)
                                    | Value::String(_)
                                    | Value::Bool(_)
                                    | Value::Int(_)
                                    | Value::Float(_)
                            ) {
                                ok = false;
                                break;
                            }
                            if omit_null && val.is_null() {
                                continue;
                            }
                            yaml.member(false, !emitted);
                            emitted = true;
                            match key {
                                Value::String(s) => {
                                    if s.as_str().is_none() {
                                        ok = false;
                                        break;
                                    }
                                    yaml.string(s, true);
                                }
                                Value::Null | Value::NullWithDefault(_) => yaml.text("null"),
                                Value::Bool(v) => yaml.text(if *v { "true" } else { "false" }),
                                Value::Int(v) => {
                                    let mut num = [0u8; 24];
                                    yaml.text(super::format_i64_buf(v.get(), &mut num));
                                }
                                Value::Float(v) => {
                                    if v.is_nan() {
                                        yaml.text(".NaN");
                                    } else if **v == f64::INFINITY {
                                        yaml.text(".Inf");
                                    } else if **v == f64::NEG_INFINITY {
                                        yaml.text("-.Inf");
                                    } else {
                                        yaml.text(&crate::numeric::double_to_string(**v));
                                    }
                                }
                                _ => unreachable!(),
                            }
                            yaml.text(":");
                            if !self.write_fast_yaml_value(
                                val,
                                yaml,
                                omit_null,
                                outer_active,
                                active,
                                scratch,
                            ) {
                                ok = false;
                                break;
                            }
                        }
                        FastMember::Element(..) => {
                            ok = false;
                            break;
                        }
                    }
                }
                if ok && is_strict_map && count > 0 && !emitted {
                    ok = false;
                }
                if ok {
                    yaml.close(!emitted);
                }
                ok
            }
        };
        scratch.truncate(base);
        ok
    }

    #[allow(clippy::too_many_arguments)]
    fn write_fast_plist_value<'a>(
        &'a self,
        value: &'a Value,
        buf: &mut String,
        indent: &str,
        depth: usize,
        outer_active: Option<&BTreeSet<ObjectId>>,
        active: &mut Vec<ObjectId>,
        scratch: &mut Vec<FastMember<'a>>,
    ) -> bool {
        if let Value::Object(id) = value {
            if active.len() >= 32
                || active.contains(id)
                || outer_active.is_some_and(|set| set.contains(id))
            {
                return false;
            }
            active.push(*id);
            let ok = self.write_fast_plist_object(
                *id,
                buf,
                indent,
                depth,
                outer_active,
                active,
                scratch,
            );
            active.pop();
            return ok;
        }
        super::plist::write_scalar(buf, value)
    }

    #[allow(clippy::too_many_arguments)]
    fn write_fast_plist_object<'a>(
        &'a self,
        id: ObjectId,
        buf: &mut String,
        indent: &str,
        depth: usize,
        outer_active: Option<&BTreeSet<ObjectId>>,
        active: &mut Vec<ObjectId>,
        scratch: &mut Vec<FastMember<'a>>,
    ) -> bool {
        let base = scratch.len();
        let Some(kind) = self.extract_fast_object(id, scratch, false) else {
            return false;
        };
        let child_depth = depth + 1;
        let ok = match kind {
            FastObjectKind::StrictList(elems) => {
                if elems.is_empty() {
                    push_indent_str(buf, indent, depth);
                    buf.push_str("<array/>");
                    true
                } else {
                    buf.push_str("<array>\n");
                    let mut ok = true;
                    for elem in elems {
                        push_indent_str(buf, indent, child_depth);
                        if !self.write_fast_plist_value(
                            elem,
                            buf,
                            indent,
                            child_depth,
                            outer_active,
                            active,
                            scratch,
                        ) {
                            ok = false;
                            break;
                        }
                        buf.push('\n');
                    }
                    if ok {
                        push_indent_str(buf, indent, depth);
                        buf.push_str("</array>");
                    }
                    ok
                }
            }
            FastObjectKind::EmptyList => {
                push_indent_str(buf, indent, depth);
                buf.push_str("<array/>");
                true
            }
            FastObjectKind::StrictSet(count)
            | FastObjectKind::PlainListing(count)
            | FastObjectKind::PlainDynamic {
                count,
                has_elements: true,
                has_keyed: false,
            } => {
                if count == 0 {
                    push_indent_str(buf, indent, depth);
                    buf.push_str("<array/>");
                    true
                } else {
                    let end = base + count;
                    buf.push_str("<array>\n");
                    let mut ok = true;
                    for i in base..end {
                        let FastMember::Element(_, elem) = scratch[i] else {
                            ok = false;
                            break;
                        };
                        push_indent_str(buf, indent, child_depth);
                        if !self.write_fast_plist_value(
                            elem,
                            buf,
                            indent,
                            child_depth,
                            outer_active,
                            active,
                            scratch,
                        ) {
                            ok = false;
                            break;
                        }
                        buf.push('\n');
                    }
                    if ok {
                        push_indent_str(buf, indent, depth);
                        buf.push_str("</array>");
                    }
                    ok
                }
            }
            FastObjectKind::PlainDynamic {
                has_elements: true,
                has_keyed: true,
                ..
            } => false,
            FastObjectKind::StrictMap(count)
            | FastObjectKind::PlainMapping(count)
            | FastObjectKind::PlainDynamic {
                count,
                has_elements: false,
                ..
            }
            | FastObjectKind::PlainModule { count, .. }
            | FastObjectKind::PlainTyped { count } => {
                let is_strict_map = matches!(kind, FastObjectKind::StrictMap(_));
                let end = base + count;
                let mut emitted = false;
                let mut ok = true;
                for i in base..end {
                    let (key_str, val) = match scratch[i] {
                        FastMember::Property(name, val) => (name.as_str(), val),
                        FastMember::Entry(Value::String(name), val) => (name.as_str(), val),
                        _ => {
                            ok = false;
                            break;
                        }
                    };
                    if val.is_null() {
                        continue;
                    }
                    let Some(key_str) = key_str else {
                        ok = false;
                        break;
                    };
                    if !emitted {
                        buf.push_str("<dict>\n");
                        emitted = true;
                    }
                    push_indent_str(buf, indent, child_depth);
                    buf.push_str("<key>");
                    super::plist::write_escaped_str(buf, key_str);
                    buf.push_str("</key>\n");
                    push_indent_str(buf, indent, child_depth);
                    if !self.write_fast_plist_value(
                        val,
                        buf,
                        indent,
                        child_depth,
                        outer_active,
                        active,
                        scratch,
                    ) {
                        ok = false;
                        break;
                    }
                    buf.push('\n');
                }
                if ok && is_strict_map && count > 0 && !emitted {
                    ok = false;
                }
                if ok {
                    push_indent_str(buf, indent, depth);
                    buf.push_str(if emitted { "</dict>" } else { "<dict/>" });
                }
                ok
            }
        };
        scratch.truncate(base);
        ok
    }

    #[allow(clippy::too_many_arguments)]
    fn write_fast_xml_visit<'a>(
        &'a self,
        value: &'a Value,
        buf: &mut String,
        indent: &str,
        version11: bool,
        depth: usize,
        xml_line: &mut usize,
        output_non_empty: bool,
        outer_active: Option<&BTreeSet<ObjectId>>,
        active: &mut Vec<ObjectId>,
        scratch: &mut Vec<FastMember<'a>>,
    ) -> bool {
        match value {
            Value::String(s) => {
                let Some(text) = s.as_str() else {
                    return false;
                };
                super::plist::write_escaped_str(buf, text);
                true
            }
            Value::Bool(_) | Value::Int(_) | Value::Float(_) => write_fast_scalar_text(buf, value),
            Value::Object(id) => {
                if active.len() >= 32
                    || active.contains(id)
                    || outer_active.is_some_and(|set| set.contains(id))
                {
                    return false;
                }
                active.push(*id);
                let ok = self.write_fast_xml_object(
                    *id,
                    buf,
                    indent,
                    version11,
                    depth,
                    xml_line,
                    output_non_empty,
                    outer_active,
                    active,
                    scratch,
                );
                active.pop();
                ok
            }
            _ => false,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn write_fast_xml_element<'a>(
        &'a self,
        value: &'a Value,
        buf: &mut String,
        indent: &str,
        version11: bool,
        depth: usize,
        xml_line: &mut usize,
        output_non_empty: bool,
        outer_active: Option<&BTreeSet<ObjectId>>,
        active: &mut Vec<ObjectId>,
        scratch: &mut Vec<FastMember<'a>>,
    ) -> bool {
        if value.is_null() {
            return true;
        }
        match value {
            Value::String(s) => {
                let Some(text) = s.as_str() else {
                    return false;
                };
                super::plist::write_escaped_str(buf, text);
                true
            }
            Value::Bool(_) | Value::Int(_) | Value::Float(_) => write_fast_scalar_text(buf, value),
            Value::Object(id) => {
                if active.len() >= 32
                    || active.contains(id)
                    || outer_active.is_some_and(|set| set.contains(id))
                {
                    return false;
                }
                let base = scratch.len();
                let Some(kind) = self.extract_fast_object(*id, scratch, true) else {
                    return false;
                };
                let tag: PklString;
                let tag_str = match &kind {
                    FastObjectKind::StrictList(_) | FastObjectKind::EmptyList => "List",
                    FastObjectKind::StrictSet(_) => "Set",
                    FastObjectKind::StrictMap(_) => "Map",
                    FastObjectKind::PlainDynamic { .. } => "Dynamic",
                    FastObjectKind::PlainListing(_) => "Listing",
                    FastObjectKind::PlainMapping(_) => "Mapping",
                    FastObjectKind::PlainModule { class, .. } => {
                        let Ok(name) = self.heap.class_simple_name(*class) else {
                            scratch.truncate(base);
                            return false;
                        };
                        tag = name;
                        let Some(s) = tag.as_str() else {
                            scratch.truncate(base);
                            return false;
                        };
                        if !super::xml::valid_name_str(s, version11) {
                            scratch.truncate(base);
                            return false;
                        }
                        s
                    }
                    FastObjectKind::PlainTyped { .. } => {
                        scratch.truncate(base);
                        return false;
                    }
                };
                if output_non_empty || !buf.is_empty() {
                    *xml_line += 1;
                    buf.push('\n');
                    push_indent_str(buf, indent, depth);
                }
                buf.push('<');
                buf.push_str(tag_str);
                buf.push('>');
                let open_line = *xml_line;
                active.push(*id);
                let ok = self.write_fast_xml_object_members(
                    kind,
                    base,
                    buf,
                    indent,
                    version11,
                    depth + 1,
                    xml_line,
                    true,
                    outer_active,
                    active,
                    scratch,
                );
                active.pop();
                scratch.truncate(base);
                if !ok {
                    return false;
                }
                if open_line < *xml_line {
                    *xml_line += 1;
                    buf.push('\n');
                    push_indent_str(buf, indent, depth);
                }
                buf.push_str("</");
                buf.push_str(tag_str);
                buf.push('>');
                true
            }
            _ => false,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn write_fast_xml_object<'a>(
        &'a self,
        id: ObjectId,
        buf: &mut String,
        indent: &str,
        version11: bool,
        depth: usize,
        xml_line: &mut usize,
        output_non_empty: bool,
        outer_active: Option<&BTreeSet<ObjectId>>,
        active: &mut Vec<ObjectId>,
        scratch: &mut Vec<FastMember<'a>>,
    ) -> bool {
        let base = scratch.len();
        let Some(kind) = self.extract_fast_object(id, scratch, true) else {
            return false;
        };
        let ok = self.write_fast_xml_object_members(
            kind,
            base,
            buf,
            indent,
            version11,
            depth,
            xml_line,
            output_non_empty,
            outer_active,
            active,
            scratch,
        );
        scratch.truncate(base);
        ok
    }

    #[allow(clippy::too_many_arguments)]
    fn write_fast_xml_object_members<'a>(
        &'a self,
        kind: FastObjectKind<'a>,
        base: usize,
        buf: &mut String,
        indent: &str,
        version11: bool,
        depth: usize,
        xml_line: &mut usize,
        output_non_empty: bool,
        outer_active: Option<&BTreeSet<ObjectId>>,
        active: &mut Vec<ObjectId>,
        scratch: &mut Vec<FastMember<'a>>,
    ) -> bool {
        match kind {
            FastObjectKind::StrictList(elems) => elems.iter().all(|elem| {
                self.write_fast_xml_element(
                    elem,
                    buf,
                    indent,
                    version11,
                    depth,
                    xml_line,
                    output_non_empty,
                    outer_active,
                    active,
                    scratch,
                )
            }),
            FastObjectKind::EmptyList => true,
            FastObjectKind::StrictSet(count)
            | FastObjectKind::PlainListing(count)
            | FastObjectKind::PlainDynamic {
                count,
                has_elements: true,
                has_keyed: false,
            } => {
                let end = base + count;
                for i in base..end {
                    let FastMember::Element(_, elem) = scratch[i] else {
                        return false;
                    };
                    if !self.write_fast_xml_element(
                        elem,
                        buf,
                        indent,
                        version11,
                        depth,
                        xml_line,
                        output_non_empty,
                        outer_active,
                        active,
                        scratch,
                    ) {
                        return false;
                    }
                }
                true
            }
            FastObjectKind::PlainDynamic {
                has_elements: true,
                has_keyed: true,
                ..
            } => false,
            FastObjectKind::StrictMap(count)
            | FastObjectKind::PlainMapping(count)
            | FastObjectKind::PlainDynamic {
                count,
                has_elements: false,
                ..
            }
            | FastObjectKind::PlainModule { count, .. }
            | FastObjectKind::PlainTyped { count } => {
                let end = base + count;
                for i in base..end {
                    let (key_str, val) = match scratch[i] {
                        FastMember::Property(name, val) => (name.as_str(), val),
                        FastMember::Entry(Value::String(name), val) => (name.as_str(), val),
                        _ => return false,
                    };
                    if val.is_null() {
                        continue;
                    }
                    let Some(key_str) = key_str else {
                        return false;
                    };
                    if !super::xml::valid_name_str(key_str, version11) {
                        return false;
                    }
                    if output_non_empty || !buf.is_empty() {
                        *xml_line += 1;
                        buf.push('\n');
                        push_indent_str(buf, indent, depth);
                    }
                    buf.push('<');
                    buf.push_str(key_str);
                    buf.push('>');
                    let open_line = *xml_line;
                    if !self.write_fast_xml_visit(
                        val,
                        buf,
                        indent,
                        version11,
                        depth + 1,
                        xml_line,
                        true,
                        outer_active,
                        active,
                        scratch,
                    ) {
                        return false;
                    }
                    if open_line < *xml_line {
                        *xml_line += 1;
                        buf.push('\n');
                        push_indent_str(buf, indent, depth);
                    }
                    buf.push_str("</");
                    buf.push_str(key_str);
                    buf.push('>');
                }
                true
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn write_fast_properties_object<'a>(
        &'a self,
        id: ObjectId,
        buf: &mut String,
        prefix: &mut String,
        has_prefix: bool,
        omit_null: bool,
        restrict_charset: bool,
        outer_active: Option<&BTreeSet<ObjectId>>,
        active: &mut Vec<ObjectId>,
        scratch: &mut Vec<FastMember<'a>>,
    ) -> bool {
        let base = scratch.len();
        let Some(kind) = self.extract_fast_object(id, scratch, false) else {
            return false;
        };
        let count = match kind {
            FastObjectKind::StrictMap(count)
            | FastObjectKind::PlainMapping(count)
            | FastObjectKind::PlainModule { count, .. }
            | FastObjectKind::PlainTyped { count } => count,
            FastObjectKind::PlainDynamic {
                count,
                has_elements: false,
                ..
            } => {
                if self.heap.get_fast(id).is_none_or(|o| o.length != 0) {
                    scratch.truncate(base);
                    return false;
                }
                count
            }
            _ => {
                scratch.truncate(base);
                return false;
            }
        };
        let end = base + count;
        let mut ok = true;
        for i in base..end {
            let (key_is_prop, key_val, val) = match scratch[i] {
                FastMember::Property(name, val) => (true, Ok(name), val),
                FastMember::Entry(key, val) => (false, Err(key), val),
                FastMember::Element(..) => {
                    ok = false;
                    break;
                }
            };
            if omit_null && val.is_null() {
                continue;
            }
            let saved_prefix = prefix.len();
            if has_prefix {
                prefix.push('.');
            }
            if key_is_prop {
                let Ok(name) = key_val else { unreachable!() };
                let Some(s) = name.as_str() else {
                    prefix.truncate(saved_prefix);
                    ok = false;
                    break;
                };
                write_properties_str(prefix, s, true, restrict_charset);
            } else {
                let Err(key) = key_val else { unreachable!() };
                match key {
                    Value::String(s) => {
                        let Some(s) = s.as_str() else {
                            prefix.truncate(saved_prefix);
                            ok = false;
                            break;
                        };
                        write_properties_str(prefix, s, true, restrict_charset);
                    }
                    Value::Bool(_) | Value::Int(_) | Value::Float(_) => {
                        let mut tmp = String::new();
                        if !write_fast_scalar_text(&mut tmp, key) {
                            prefix.truncate(saved_prefix);
                            ok = false;
                            break;
                        }
                        write_properties_str(prefix, &tmp, true, restrict_charset);
                    }
                    _ => {
                        prefix.truncate(saved_prefix);
                        ok = false;
                        break;
                    }
                }
            }
            match val {
                Value::Null | Value::NullWithDefault(_) => {
                    buf.push_str(prefix);
                    buf.push_str(" = \n");
                }
                Value::String(s) => {
                    let Some(s) = s.as_str() else {
                        prefix.truncate(saved_prefix);
                        ok = false;
                        break;
                    };
                    buf.push_str(prefix);
                    buf.push_str(" = ");
                    write_properties_str(buf, s, false, restrict_charset);
                    buf.push('\n');
                }
                Value::Bool(_) | Value::Int(_) | Value::Float(_) => {
                    buf.push_str(prefix);
                    buf.push_str(" = ");
                    let mut tmp = String::new();
                    if !write_fast_scalar_text(&mut tmp, val) {
                        prefix.truncate(saved_prefix);
                        ok = false;
                        break;
                    }
                    write_properties_str(buf, &tmp, false, restrict_charset);
                    buf.push('\n');
                }
                Value::Object(child_id) => {
                    if active.len() >= 32
                        || active.contains(child_id)
                        || outer_active.is_some_and(|set| set.contains(child_id))
                    {
                        prefix.truncate(saved_prefix);
                        ok = false;
                        break;
                    }
                    active.push(*child_id);
                    let child_ok = self.write_fast_properties_object(
                        *child_id,
                        buf,
                        prefix,
                        true,
                        omit_null,
                        restrict_charset,
                        outer_active,
                        active,
                        scratch,
                    );
                    active.pop();
                    if !child_ok {
                        prefix.truncate(saved_prefix);
                        ok = false;
                        break;
                    }
                }
                _ => {
                    prefix.truncate(saved_prefix);
                    ok = false;
                    break;
                }
            }
            prefix.truncate(saved_prefix);
        }
        scratch.truncate(base);
        ok
    }
}

include!("xml_rendering.rs");

include!("pkl_binary_render.rs");

include!("protobuf_types.rs");
include!("protobuf_rendering.rs");
