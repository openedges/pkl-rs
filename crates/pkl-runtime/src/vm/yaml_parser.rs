//! YAML document construction is independent of JSON's streaming callbacks.
//! The pure backend composes a document first; this continuation constructs the
//! identity graph and executes every edge converter through ordinary VM frames.
use super::converters::{Converters, PathPart};
use super::*;
use crate::heap::{CollectionKind, CollectionMember, CollectionMemberKind};
#[path = "yaml_parser/backend/mod.rs"]
mod backend;
#[path = "yaml_parser/scalars.rs"]
mod scalars;
use backend::{Backend, Document, Mode, NodeId, NodeKind, Settings, Text};

#[derive(Clone)]
struct MapWork {
    node: NodeId,
    object: ObjectId,
    path: Vec<PathPart>,
    index: usize,
    keys: Vec<(Value, i32)>,
}
enum Task {
    Construct(NodeId, Vec<PathPart>),
    Finish(NodeId, ObjectId),
    Sequence {
        node: NodeId,
        object: ObjectId,
        index: usize,
        path: Vec<PathPart>,
        set: bool,
    },
    Element(ObjectId, usize),
    Convert(Vec<PathPart>),
    Preflight(MapWork),
    PreflightKey(MapWork),
    Duplicate {
        work: MapWork,
        key: Value,
        scan: usize,
    },
    DuplicateResult {
        work: MapWork,
        key: Value,
        scan: usize,
    },
    Pairs(MapWork),
    Key {
        work: MapWork,
        original: Value,
    },
    Value {
        work: MapWork,
        converted: Value,
    },
    Put {
        object: ObjectId,
        key: Value,
        value: Value,
        scan: usize,
        candidates: Option<Vec<usize>>,
    },
    PutResult {
        object: ObjectId,
        key: Value,
        value: Value,
        scan: usize,
        candidates: Option<Vec<usize>>,
    },
    Append(ObjectId, Value, Value),
    Rebuild(ObjectId, usize),
    Link(ObjectId, usize, bool),
    Hash(Value),
    CheckKeyHash(Value),
    Documents(ObjectId, usize),
    Root,
}
impl Task {
    fn roots(&self, roots: &mut Vec<Value>) {
        use Task::*;
        match self {
            Construct(_, _) | Convert(_) | Root => (),
            Finish(_, id) | Element(id, _) | Sequence { object: id, .. } => {
                roots.push(crate::value::Value::Object(*id))
            }
            Preflight(w) | PreflightKey(w) | Pairs(w) => w.roots(roots),
            Duplicate { work, key, .. } | DuplicateResult { work, key, .. } => {
                work.roots(roots);
                roots.push(key.clone());
            }
            Key { work, original } => {
                work.roots(roots);
                roots.push(original.clone());
            }
            Value { work, converted } => {
                work.roots(roots);
                roots.push(converted.clone());
            }
            Put {
                object, key, value, ..
            }
            | PutResult {
                object, key, value, ..
            }
            | Append(object, key, value) => roots.extend([
                crate::value::Value::Object(*object),
                key.clone(),
                value.clone(),
            ]),
            Rebuild(id, _) | Link(id, _, _) | Documents(id, _) => {
                roots.push(crate::value::Value::Object(*id))
            }
            Hash(value) | CheckKeyHash(value) => roots.push(value.clone()),
        }
    }
}
impl MapWork {
    fn roots(&self, roots: &mut Vec<Value>) {
        roots.push(Value::Object(self.object));
        roots.extend(self.keys.iter().map(|(key, _)| key.clone()));
    }
}
enum Await {
    Text,
    Uri,
    Converters,
    ForcedConverters,
    Mode,
    UseMapping,
    MaxAliases,
    Converted,
    Compared,
}
pub(super) struct Run {
    receiver: ObjectId,
    source: Value,
    resource: Option<ObjectId>,
    uri: PklString,
    mapping: Option<Value>,
    converters: Converters,
    phase: u8,
    awaiting: Option<Await>,
    mode: Mode,
    use_mapping: bool,
    max_aliases: usize,
    all: bool,
    backend: Option<Backend>,
    document: Option<Document>,
    cache: Vec<Option<Value>>,
    active: BTreeSet<NodeId>,
    declaration_maps: BTreeMap<ObjectId, Option<crate::economic_map::EconomicMapIndex>>,
    tasks: Vec<Task>,
    values: Vec<Value>,
    documents: Vec<Value>,
    returned: Option<bool>,
    pending: Option<Value>,
    result: Option<Value>,
    constructing: bool,
    checking_key_hash: bool,
    stack_base: usize,
    wrapped_failure: Option<RuntimeError>,
}
impl Run {
    pub(super) fn roots(&self, roots: &mut Vec<Value>) {
        roots.extend([Value::Object(self.receiver), self.source.clone()]);
        roots.extend(self.resource.map(Value::Object));
        roots.extend(self.mapping.iter().cloned());
        roots.extend(self.cache.iter().flatten().cloned());
        roots.extend(self.declaration_maps.keys().copied().map(Value::Object));
        roots.extend(self.values.iter().cloned());
        roots.extend(self.documents.iter().cloned());
        roots.extend(self.pending.iter().cloned());
        roots.extend(self.result.iter().cloned());
        self.converters.roots(roots);
        for task in &self.tasks {
            task.roots(roots);
        }
        if let Some(error) = &self.wrapped_failure {
            error.diagnostic_roots(roots);
        }
    }
    fn document(&self) -> Result<&Document, RuntimeError> {
        self.document
            .as_ref()
            .ok_or_else(|| invalid("YAML document"))
    }
    fn pop(&mut self) -> Result<Value, RuntimeError> {
        self.values
            .pop()
            .ok_or_else(|| invalid("YAML construction value stack"))
    }
}
fn text(value: &Text) -> PklString {
    PklString::from_utf16(value.units().to_vec())
}
fn backend_text(value: &PklString) -> Text {
    Text::from_units(value.encode_utf16().collect::<Vec<_>>())
}
fn unsupported(message: &str) -> RuntimeError {
    RuntimeError::new(
        ErrorKind::Unsupported,
        format!("YAML constructor: {message}"),
    )
}
fn parse_error(hint: PklString) -> RuntimeError {
    let mut error = RuntimeError::new(ErrorKind::Type, "Error parsing YAML document.");
    error.hint = Some(hint);
    error.unavailable_origin = true;
    error.yaml_constructor_error_ready = true;
    error.java_exception_message = Some((
        "org.pkl.core.runtime.VmEvalException",
        "yamlParseError".into(),
    ));
    error
}
// Pkl 0.32.0's shaded Engine node classes share the unnamed application
// module. Derive both class names from the actual graph kind and selected tag
// constructor; this covers every scalar/sequence/mapping cast direction.
fn constructor_node_cast(actual: &str, expected: &str) -> RuntimeError {
    let package = "org.pkl.thirdparty.snakeyaml.engine.v2.nodes";
    let actual = format!("{package}.{actual}");
    let expected = format!("{package}.{expected}");
    parse_error(
        format!(
            "java.lang.ClassCastException: class {actual} cannot be cast to class {expected} ({actual} and {expected} are in unnamed module of loader 'app')"
        )
        .into(),
    )
}
fn engine_error(error: backend::YamlError, max: usize) -> RuntimeError {
    if error.kind == backend::diagnostic::ErrorKind::Foreign {
        return unsupported("non-YamlEngine backend throwable requires Java failure projection");
    }
    if error.kind == backend::diagnostic::ErrorKind::Unsupported {
        return RuntimeError::new(
            ErrorKind::Unsupported,
            PklString::from("YAML backend: ").concat(&text(&error.problem)),
        );
    }
    if error.problem.units().starts_with(
        &"Number of aliases for non-scalar nodes exceeds the specified"
            .encode_utf16()
            .collect::<Vec<_>>(),
    ) {
        let mut error = RuntimeError::new(
            ErrorKind::Type,
            format!(
                "Error parsing YAML document: The number of aliases for collection nodes exceeds the allowed maximum of {max}."
            ),
        );
        error.set_template_hint(
            "To increase the allowed maximum, set `YamlRenderer.maxCollectionAliases`.",
        );
        error.unavailable_origin = true;
        error.yaml_constructor_error_ready = true;
        error.java_exception_message = Some((
            "org.pkl.core.runtime.VmEvalException",
            "yamlParseErrorTooManyAliases".into(),
        ));
        return error;
    }
    parse_error(text(&error.message()))
}
impl Vm {
    pub(super) fn begin_yaml_parse(&mut self, all: bool) -> Result<(), RuntimeError> {
        let receiver = self.this_receiver()?.as_object()?;
        let source = self
            .frames
            .last()
            .and_then(|f| f.environment.locals.last())
            .cloned()
            .ok_or_else(|| invalid("YAML argument"))?;
        self.frames.last_mut().unwrap().yaml_parse = Some(Box::new(Run {
            receiver,
            source,
            resource: None,
            uri: "input_string".into(),
            mapping: None,
            converters: Converters::default(),
            phase: 0,
            awaiting: None,
            mode: Mode::Compat,
            use_mapping: false,
            max_aliases: 50,
            all,
            backend: None,
            document: None,
            cache: Vec::new(),
            active: BTreeSet::new(),
            declaration_maps: BTreeMap::new(),
            tasks: Vec::new(),
            values: Vec::new(),
            documents: Vec::new(),
            returned: None,
            pending: None,
            result: None,
            constructing: false,
            checking_key_hash: false,
            stack_base: self.stack.len(),
            wrapped_failure: None,
        }));
        Ok(())
    }
    pub(super) fn step_yaml_parse(&mut self) -> Result<(), RuntimeError> {
        let index = self.frames.len() - 1;
        let mut run = self.frames[index]
            .yaml_parse
            .take()
            .expect("YAML continuation");
        let mut result = self.advance_yaml_parse(&mut run);
        if let Err(error) = &mut result
            && error.yaml_constructor_error_ready
        {
            error.yaml_constructor_error_ready = false;
            run.constructing = false;
        }
        match result {
            Ok(true) => self
                .stack
                .push(run.result.take().ok_or_else(|| invalid("YAML result"))?),
            Ok(false) | Err(_) => self.frames[index].yaml_parse = Some(run),
        }
        result.map(|_| ())
    }
    /// Innermost guest catches still win over the constructor wrapper. Host
    /// protocol errors, cancellation, limits and unsupported backends never do.
    pub(super) fn yaml_constructor_boundary(&self, error: &RuntimeError) -> Option<usize> {
        if !guest_exceptions::is_guest_exception(error) {
            return None;
        }
        self.frames.iter().rposition(|frame| {
            frame
                .yaml_parse
                .as_ref()
                .is_some_and(|run| run.constructing && run.wrapped_failure.is_none())
        })
    }
    pub(super) fn catch_yaml_constructor(&mut self, index: usize, error: RuntimeError) {
        self.frames.truncate(index + 1);
        let frame = &mut self.frames[index];
        let run = frame.yaml_parse.as_mut().expect("YAML boundary");
        run.constructing = false;
        run.wrapped_failure = Some(if run.checking_key_hash {
            unsupported(
                "constructKey caught an exception: unacceptable-key Java toString/cause projection is not modeled",
            )
        } else if let Some((class, message)) = &error.java_exception_message {
            parse_error(PklString::from(*class).concat(&": ".into()).concat(message))
        } else {
            unsupported(
                "escaping guest error has no source-proven Java Throwable message provenance",
            )
        });
        self.stack.truncate(run.stack_base);
        frame.type_run = None;
        frame.type_resolution = None;
        frame.generic_default = None;
        frame.reflection = None;
        frame.module_load = None;
        frame.uri_resolution = None;
        frame.deep_force = None;
        frame.prepared_hashes = None;
        frame.key_search = None;
        frame.equality_run = None;
        self.state = State::Running;
    }
    fn yaml_read(
        &mut self,
        run: &mut Run,
        name: &str,
        awaiting: Await,
    ) -> Result<(), RuntimeError> {
        run.awaiting = Some(awaiting);
        self.force(run.receiver, run.receiver, name.into(), false, true, false)
    }
    fn advance_yaml_parse(&mut self, run: &mut Run) -> Result<bool, RuntimeError> {
        if let Some(error) = run.wrapped_failure.take() {
            return Err(error);
        }
        if let Some(awaiting) = run.awaiting.take() {
            let value = self.pop()?;
            match awaiting {
                Await::Text => run.source = value,
                Await::Uri => {
                    let Value::String(uri) = value else {
                        return Err(invalid("checked Resource.uri"));
                    };
                    run.uri = uri;
                }
                Await::Converters => run.mapping = Some(value),
                Await::ForcedConverters => (),
                Await::Mode => {
                    let Value::String(mode) = value else {
                        return Err(invalid("checked YAML mode"));
                    };
                    run.mode = match mode.as_str() {
                        Some("compat") => Mode::Compat,
                        Some("1.1") => Mode::Yaml11,
                        Some("1.2") => Mode::Yaml12,
                        _ => return Err(invalid("checked YAML mode constraint")),
                    };
                }
                Await::UseMapping => {
                    let Value::Bool(flag) = value else {
                        return Err(invalid("checked YAML useMapping"));
                    };
                    run.use_mapping = flag;
                }
                Await::MaxAliases => {
                    let Value::Int(max) = value else {
                        return Err(invalid("checked YAML alias limit"));
                    };
                    run.max_aliases =
                        usize::try_from(max.get()).map_err(|_| invalid("checked alias limit"))?;
                }
                Await::Converted => {
                    run.pending.take();
                    run.values.push(value);
                }
                Await::Compared => {
                    let Value::Bool(equal) = value else {
                        return Err(invalid("YAML key equality"));
                    };
                    run.returned = Some(equal);
                }
            }
        }
        match run.phase {
            0 => {
                run.phase = 1;
                if let Value::Object(id) = run.source {
                    run.resource = Some(id);
                    run.awaiting = Some(Await::Text);
                    self.force(id, id, "text".into(), false, true, false)?;
                }
                return Ok(false);
            }
            1 => {
                run.phase = 2;
                if let Some(id) = run.resource {
                    run.awaiting = Some(Await::Uri);
                    self.force(id, id, "uri".into(), false, true, false)?;
                }
                return Ok(false);
            }
            2 => {
                if !matches!(run.source, Value::String(_)) {
                    return Err(invalid("checked YAML text"));
                }
                run.phase = 3;
                self.yaml_read(run, "converters", Await::Converters)?;
                return Ok(false);
            }
            3 => {
                run.phase = 4;
                run.awaiting = Some(Await::ForcedConverters);
                self.force_shallow_frame(
                    run.mapping
                        .clone()
                        .ok_or_else(|| invalid("YAML converters"))?,
                )?;
                return Ok(false);
            }
            4 => {
                run.converters = Converters::load_parser(
                    &self.heap,
                    run.mapping
                        .as_ref()
                        .ok_or_else(|| invalid("YAML converter mapping"))?
                        .as_object()?,
                )?;
                run.phase = 5;
                self.yaml_read(run, "mode", Await::Mode)?;
                return Ok(false);
            }
            5 => {
                run.phase = 6;
                self.yaml_read(run, "useMapping", Await::UseMapping)?;
                return Ok(false);
            }
            6 => {
                run.phase = 7;
                self.yaml_read(run, "maxCollectionAliases", Await::MaxAliases)?;
                return Ok(false);
            }
            7 => {
                let Value::String(source) = &run.source else {
                    return Err(invalid("YAML text"));
                };
                // Java source creation calls URI.create outside its parse catch.
                // Nonstandard invalid labels need typed Java cause projection.
                crate::host::resources::glob::parse_base(&run.uri).map_err(|_| {
                    unsupported("Resource URI.create failure projection or unsupported URI syntax")
                })?;
                run.backend = Some(Backend::new(
                    backend_text(source),
                    Settings {
                        mode: run.mode,
                        max_collection_aliases: run.max_aliases,
                        label: backend_text(&run.uri),
                    },
                ));
                run.phase = 8;
                return Ok(false);
            }
            _ => (),
        }
        if let Some(task) = run.tasks.pop() {
            self.yaml_task(run, task)?;
            return Ok(false);
        }
        if run.phase == 10 {
            let object = run
                .result
                .take()
                .ok_or_else(|| invalid("YAML document list"))?
                .as_object()?;
            run.result = Some(Value::Object(self.finish_strict_collection(object)?));
            run.documents.clear();
            return Ok(true);
        }
        if run.phase == 9 {
            let result = run.pop()?;
            run.document = None;
            run.cache.clear();
            run.active.clear();
            if !run.all {
                run.result = Some(result);
                return Ok(true);
            }
            run.documents.push(result);
            run.phase = 8;
        }
        let backend = run
            .backend
            .as_mut()
            .ok_or_else(|| invalid("YAML backend"))?;
        let document = if run.all {
            backend.next_document()
        } else {
            backend.single_document()
        }
        .map_err(|e| engine_error(e, run.max_aliases))?;
        let Some(document) = document else {
            if run.all {
                let object = self.heap.allocate(Object {
                    collection_kind: CollectionKind::List,
                    length: run.documents.len(),
                    ..Object::default()
                });
                run.result = Some(Value::Object(object));
                run.tasks.push(Task::Documents(object, 0));
                run.phase = 10;
                return Ok(false);
            }
            run.values.push(Value::Null);
            run.tasks.push(Task::Root);
            run.phase = 9;
            return Ok(false);
        };
        let node = document.root();
        run.cache = vec![None; document.nodes().len()];
        run.document = Some(document);
        run.constructing = true;
        run.phase = 9;
        run.tasks.push(Task::Root);
        // BaseConstructor.constructSingleDocument bypasses a root tagged NULL.
        if !run.all
            && run
                .document()?
                .node(node)
                .tag
                .is_ascii("tag:yaml.org,2002:null")
        {
            run.values.push(Value::Null);
        } else {
            run.tasks.push(Task::Construct(node, vec![PathPart::Root]));
        }
        Ok(false)
    }
    fn yaml_task(&mut self, run: &mut Run, task: Task) -> Result<(), RuntimeError> {
        run.checking_key_hash = false;
        match task {
            Task::Construct(id, path) => {
                if let Some(value) = run.cache[id.0].clone() {
                    run.values.push(value);
                    return Ok(());
                }
                if !run.active.insert(id) {
                    return Err(unsupported("unpublished recursive node construction"));
                }
                let node = run.document()?.node(id).clone();
                if ![
                    "null",
                    "bool",
                    "int",
                    "float",
                    "binary",
                    "set",
                    "str",
                    "seq",
                    "map",
                    "timestamp",
                ]
                .iter()
                .any(|tag| node.tag.is_ascii(&format!("tag:yaml.org,2002:{tag}")))
                {
                    let problem = PklString::from("could not determine a constructor for the tag ")
                        .concat(&text(&node.tag));
                    return Err(engine_error(
                        backend::YamlError::marked(
                            backend::diagnostic::ErrorKind::Engine,
                            None,
                            None,
                            backend_text(&problem),
                            Some(node.start.clone()),
                        ),
                        run.max_aliases,
                    ));
                }
                if node.tag.is_ascii("tag:yaml.org,2002:null") {
                    run.cache[id.0] = Some(Value::Null);
                    run.active.remove(&id);
                    run.values.push(Value::Null);
                    return Ok(());
                }
                // Each pinned tag constructor casts its node before reading
                // values or allocating a guest container. NULL is the exception
                // above: its constructor deliberately ignores the node kind.
                let expected_class = if node.tag.is_ascii("tag:yaml.org,2002:seq") {
                    "SequenceNode"
                } else if node.tag.is_ascii("tag:yaml.org,2002:map")
                    || node.tag.is_ascii("tag:yaml.org,2002:set")
                {
                    "MappingNode"
                } else {
                    "ScalarNode"
                };
                let actual_class = match &node.kind {
                    NodeKind::Scalar { .. } => "ScalarNode",
                    NodeKind::Sequence { .. } => "SequenceNode",
                    NodeKind::Mapping { .. } => "MappingNode",
                };
                if actual_class != expected_class {
                    return Err(constructor_node_cast(actual_class, expected_class));
                }
                if let NodeKind::Scalar { value, .. } = &node.kind {
                    let value = scalars::construct(&node.tag, value, run.mode)?;
                    run.cache[id.0] = Some(value.clone());
                    run.active.remove(&id);
                    run.values.push(value);
                    return Ok(());
                }
                let set = node.tag.is_ascii("tag:yaml.org,2002:set");
                let mapping = node.tag.is_ascii("tag:yaml.org,2002:map");
                let sequence = node.tag.is_ascii("tag:yaml.org,2002:seq");
                let count = match &node.kind {
                    NodeKind::Sequence { children, .. } if sequence => children.len(),
                    NodeKind::Mapping { entries, .. } if set => entries.len(),
                    NodeKind::Mapping { .. } if mapping => 0,
                    _ => unreachable!("tag constructor node kind was checked before allocation"),
                };
                let kind = if sequence || set {
                    CollectionKind::Listing
                } else if run.use_mapping {
                    CollectionKind::Mapping
                } else {
                    CollectionKind::Dynamic
                };
                let object = self.yaml_shell(kind, count);
                let capacity = match &node.kind {
                    NodeKind::Mapping { entries, .. } => entries.len(),
                    NodeKind::Sequence { children, .. } => children.len(),
                    _ => 0,
                };
                run.declaration_maps.insert(
                    object,
                    Some(crate::economic_map::EconomicMapIndex::with_capacity(
                        capacity,
                    )),
                );
                if node.recursive {
                    run.cache[id.0] = Some(Value::Object(object));
                }
                run.tasks.push(Task::Finish(id, object));
                if sequence {
                    run.tasks.push(Task::Sequence {
                        node: id,
                        object,
                        index: 0,
                        path,
                        set: false,
                    });
                } else {
                    run.tasks.push(Task::Preflight(MapWork {
                        node: id,
                        object,
                        path,
                        index: 0,
                        keys: Vec::new(),
                    }));
                }
            }
            Task::Finish(id, object) => {
                let value = Value::Object(object);
                run.cache[id.0] = Some(value.clone());
                run.active.remove(&id);
                run.values.push(value);
            }
            Task::Sequence {
                node,
                object,
                index,
                path,
                set,
            } => {
                let child = match &run.document()?.node(node).kind {
                    NodeKind::Sequence { children, .. } => children.get(index).copied(),
                    NodeKind::Mapping { entries, .. } if set => {
                        entries.get(index).map(|(key, _)| *key)
                    }
                    _ => return Err(invalid("YAML sequence node")),
                };
                if let Some(child) = child {
                    let mut child_path = path.clone();
                    child_path.push(PathPart::WildcardElement);
                    run.tasks.push(Task::Sequence {
                        node,
                        object,
                        index: index + 1,
                        path,
                        set,
                    });
                    run.tasks.push(Task::Element(object, index));
                    run.tasks.push(Task::Convert(child_path.clone()));
                    run.tasks.push(Task::Construct(child, child_path));
                }
            }
            Task::Element(object, index) => {
                let value = run.pop()?;
                self.yaml_insert(object, Value::Int((index as i64).into()), value, None)?;
            }
            Task::Convert(path) => {
                let value = run.pop()?;
                if let Some(function) = run.converters.select(self, &value, &path)? {
                    let Value::Function(f) = function else {
                        return Err(invalid("checked YAML converter"));
                    };
                    run.pending = Some(value.clone());
                    run.awaiting = Some(Await::Converted);
                    self.invoke_function(f, vec![value], false)?;
                } else {
                    run.values.push(value);
                }
            }
            Task::Preflight(work) => {
                let node = run.document()?.node(work.node);
                let NodeKind::Mapping { entries, .. } = &node.kind else {
                    return Err(invalid("YAML mapping preflight"));
                };
                if let Some((key, _)) = entries.get(work.index).copied() {
                    let path = work.path.clone();
                    run.tasks.push(Task::PreflightKey(work));
                    run.tasks.push(Task::Construct(key, path));
                } else if node.tag.is_ascii("tag:yaml.org,2002:set") {
                    run.tasks.push(Task::Sequence {
                        node: work.node,
                        object: work.object,
                        index: 0,
                        path: work.path,
                        set: true,
                    });
                } else {
                    run.tasks.push(Task::Pairs(MapWork { index: 0, ..work }));
                }
            }
            Task::PreflightKey(work) => {
                let key = run.pop()?;
                run.tasks.push(Task::Duplicate {
                    work,
                    key: key.clone(),
                    scan: 0,
                });
                run.tasks.push(Task::Hash(key.clone()));
                run.tasks.push(Task::CheckKeyHash(key));
            }
            Task::CheckKeyHash(key) => {
                run.checking_key_hash = true;
                if matches!(&key,Value::Object(id) if run.active.iter().any(|n|run.cache[n.0].as_ref()==Some(&Value::Object(*id))))
                {
                    return Err(unsupported(
                        "recursive original-key hash requires partial-shell Java hashing proof",
                    ));
                }
                self.prepare_render_key(key);
            }
            Task::Hash(key) => {
                if matches!(&key,Value::Object(id) if run.active.iter().any(|n| run.cache[n.0].as_ref()==Some(&Value::Object(*id))))
                {
                    return Err(unsupported(
                        "Java recursive structural key hash overflow behavior",
                    ));
                }
                self.prepare_render_key(key);
            }
            Task::Duplicate {
                mut work,
                key,
                scan,
            } => {
                let hash = self.java_hash(&key)?.ok_or_else(|| {
                    unsupported("identity-based Java hash for converter-produced key")
                })?;
                if scan == 0
                    && work.keys.iter().filter(|(_, h)| *h == hash).count() >= 7
                    && (matches!(key, Value::Object(_))
                        || work
                            .keys
                            .iter()
                            .any(|(k, h)| *h == hash && matches!(k, Value::Object(_))))
                {
                    return Err(unsupported(
                        "HashMap tree-bin duplicate-key comparison order for colliding guest objects",
                    ));
                }
                if let Some((candidate, candidate_hash)) = work.keys.get(scan).cloned() {
                    run.tasks.push(Task::DuplicateResult {
                        work,
                        key: key.clone(),
                        scan,
                    });
                    if hash != candidate_hash {
                        run.returned = Some(false);
                    } else {
                        run.awaiting = Some(Await::Compared);
                        self.begin_boxed_equality(key, candidate);
                    }
                } else {
                    work.keys.push((key, hash));
                    work.index += 1;
                    run.tasks.push(Task::Preflight(work));
                }
            }
            Task::DuplicateResult { work, key, scan } => {
                if run
                    .returned
                    .take()
                    .ok_or_else(|| invalid("YAML duplicate comparison"))?
                {
                    let node = run.document()?.node(work.node);
                    let NodeKind::Mapping { entries, .. } = &node.kind else {
                        unreachable!()
                    };
                    let key_node = run.document()?.node(entries[work.index].0);
                    let name = match &key {
                        Value::String(s) => s.clone(),
                        Value::Int(n) => n.get().to_string().into(),
                        Value::Bool(b) => b.to_string().into(),
                        Value::Null => "null".into(),
                        _ => {
                            return Err(unsupported(
                                "duplicate-key Java toString for non-scalar key",
                            ));
                        }
                    };
                    let problem = PklString::from("found duplicate key ").concat(&name);
                    let error = backend::YamlError::marked(
                        backend::diagnostic::ErrorKind::Engine,
                        Some("while constructing a mapping"),
                        Some(node.start.clone()),
                        backend_text(&problem),
                        Some(key_node.start.clone()),
                    );
                    return Err(engine_error(error, run.max_aliases));
                }
                run.tasks.push(Task::Duplicate {
                    work,
                    key,
                    scan: scan + 1,
                });
            }
            Task::Pairs(work) => {
                let NodeKind::Mapping { entries, .. } = &run.document()?.node(work.node).kind
                else {
                    return Err(invalid("YAML map pairs"));
                };
                if let Some((key, _)) = entries.get(work.index).copied() {
                    // flattenMapping already constructed the raw key at the outer
                    // path; only the key edge converter uses the empty path.
                    let original = run.cache[key.0]
                        .clone()
                        .ok_or_else(|| invalid("preflight key identity cache"))?;
                    run.tasks.push(Task::Key {
                        work,
                        original: original.clone(),
                    });
                    run.tasks.push(Task::Convert(Vec::new()));
                    run.values.push(original);
                }
            }
            Task::Key { work, original } => {
                let converted = run.pop()?;
                let NodeKind::Mapping { entries, .. } = &run.document()?.node(work.node).kind
                else {
                    unreachable!()
                };
                let value = entries[work.index].1;
                let mut path = work.path.clone();
                path.push(match original {
                    Value::String(key) => PathPart::Property(key),
                    _ => PathPart::WildcardProperty,
                });
                run.tasks.push(Task::Value { work, converted });
                run.tasks.push(Task::Convert(path.clone()));
                run.tasks.push(Task::Construct(value, path));
            }
            Task::Value {
                mut work,
                converted,
            } => {
                let value = run.pop()?;
                let object = work.object;
                work.index += 1;
                run.tasks.push(Task::Pairs(work));
                let property = self.heap.get(object)?.collection_kind == CollectionKind::Dynamic
                    && matches!(&converted, Value::String(_));
                // Java Identifier hashes are identity based. Property-only or
                // scalar mixed dictionaries can use a side-effect-free scan;
                // do not guess bucket order when a guest object key is present.
                if property {
                    run.declaration_maps.insert(object, None);
                }
                let indexed = run.declaration_maps.get(&object).and_then(Option::as_ref);
                if indexed.is_none() && self.heap.get(object)?.collection_members.len() >= 4 {
                    let safe = |v: &Value| {
                        matches!(
                            v,
                            Value::Null
                                | Value::Bool(_)
                                | Value::Int(_)
                                | Value::Float(_)
                                | Value::String(_)
                        )
                    };
                    if !safe(&converted)
                        || self
                            .heap
                            .get(object)?
                            .collection_members
                            .iter()
                            .any(|m| !safe(&m.key))
                    {
                        return Err(unsupported(
                            "hashed declaration map mixing Identifier and guest object keys",
                        ));
                    }
                }
                let needs_hash = indexed.is_some_and(|m| m.has_hash());
                run.tasks.push(Task::Put {
                    object,
                    key: converted.clone(),
                    value,
                    scan: 0,
                    candidates: None,
                });
                if needs_hash {
                    run.tasks.push(Task::Hash(converted));
                }
            }
            Task::Put {
                object,
                key,
                value,
                scan,
                candidates,
            } => {
                let candidates = match candidates {
                    Some(c) => c,
                    None => match run.declaration_maps.get(&object).and_then(Option::as_ref) {
                        Some(index) => {
                            let hash = if index.has_hash() {
                                Some(self.java_hash(&key)?.ok_or_else(|| {
                                    unsupported("converted key Java identity hash")
                                })?)
                            } else {
                                None
                            };
                            index.candidates(hash)
                        }
                        None => (0..self.heap.get(object)?.collection_members.len()).collect(),
                    },
                };
                let property = self.heap.get(object)?.collection_kind == CollectionKind::Dynamic
                    && matches!(&key, Value::String(_));
                if let Some(slot) = candidates.get(scan).copied() {
                    let candidate = self.heap.get(object)?.collection_members[slot].clone();
                    if property != (candidate.kind == CollectionMemberKind::Property) {
                        run.tasks.push(Task::Put {
                            object,
                            key,
                            value,
                            scan: scan + 1,
                            candidates: Some(candidates),
                        });
                    } else {
                        run.tasks.push(Task::PutResult {
                            object,
                            key: key.clone(),
                            value,
                            scan,
                            candidates: Some(candidates),
                        });
                        run.awaiting = Some(Await::Compared);
                        self.begin_boxed_equality(key, candidate.key);
                    }
                } else {
                    run.tasks.push(Task::Append(object, key, value));
                    if let Some(index) = run
                        .declaration_maps
                        .get_mut(&object)
                        .and_then(Option::as_mut)
                        && index.grow_before_append()
                    {
                        index.start_rehash();
                        run.tasks.push(Task::Rebuild(object, 0));
                    }
                }
            }
            Task::PutResult {
                object,
                key,
                value,
                scan,
                candidates,
            } => {
                if run
                    .returned
                    .take()
                    .ok_or_else(|| invalid("YAML insertion equality"))?
                {
                    let slot = candidates
                        .as_ref()
                        .and_then(|c| c.get(scan))
                        .copied()
                        .ok_or_else(|| invalid("YAML member replacement slot"))?;
                    self.yaml_insert(object, key, value, Some(slot))?;
                } else {
                    run.tasks.push(Task::Put {
                        object,
                        key,
                        value,
                        scan: scan + 1,
                        candidates,
                    });
                }
            }
            Task::Append(object, key, value) => {
                // EconomicMap publishes the member before the fifth-key or
                // collision-triggered hash work, including if hashing throws.
                self.yaml_insert(object, key.clone(), value, None)?;
                if let Some(index) = run
                    .declaration_maps
                    .get_mut(&object)
                    .and_then(Option::as_mut)
                {
                    let slot = index.push();
                    if index.has_hash() {
                        run.tasks.push(Task::Link(object, slot, true));
                        run.tasks.push(Task::Hash(key));
                    } else if index.len() > 4 {
                        index.start_rehash();
                        run.tasks.push(Task::Rebuild(object, 0));
                    }
                }
            }
            Task::Rebuild(object, slot) => {
                if let Some(member) = self.heap.get(object)?.collection_members.get(slot) {
                    let key = member.key.clone();
                    run.tasks.push(Task::Rebuild(object, slot + 1));
                    run.tasks.push(Task::Link(object, slot, false));
                    run.tasks.push(Task::Hash(key));
                }
            }
            Task::Link(object, slot, allow_rehash) => {
                let key = &self.heap.get(object)?.collection_members[slot].key;
                let hash = self
                    .java_hash(key)?
                    .ok_or_else(|| unsupported("converted key Java identity hash"))?;
                let index = run
                    .declaration_maps
                    .get_mut(&object)
                    .and_then(Option::as_mut)
                    .ok_or_else(|| invalid("YAML declaration index"))?;
                if index.put_hash(slot, hash, allow_rehash && index.should_rehash()) {
                    index.start_rehash();
                    run.tasks.push(Task::Rebuild(object, 0));
                }
            }
            Task::Documents(object, index) => {
                if let Some(value) = run.documents.get(index).cloned() {
                    self.yaml_insert(object, Value::Int((index as i64).into()), value, None)?;
                    run.tasks.push(Task::Documents(object, index + 1));
                }
            }
            Task::Root => {
                run.constructing = false;
                run.cache.clear();
                run.active.clear();
                run.declaration_maps.clear();
                run.document = None;
                run.tasks.push(Task::Convert(if run.all {
                    vec![PathPart::Root]
                } else {
                    Vec::new()
                }));
            }
        }
        Ok(())
    }
    fn yaml_shell(&mut self, kind: CollectionKind, length: usize) -> ObjectId {
        let class = self.canonical_builtin_class(match kind {
            CollectionKind::Listing => classes::BuiltinClass::Listing,
            CollectionKind::Mapping => classes::BuiltinClass::Mapping,
            _ => classes::BuiltinClass::Dynamic,
        });
        let parent = self
            .heap
            .get_class(class)
            .expect("canonical YAML collection class")
            .prototype;
        self.heap.allocate(Object {
            parent: Some(parent),
            collection_kind: kind,
            length,
            ..Object::default()
        })
    }
    fn yaml_insert(
        &mut self,
        object: ObjectId,
        key: Value,
        value: Value,
        replace: Option<usize>,
    ) -> Result<(), RuntimeError> {
        let obj = self.heap.get_mut(object)?;
        if let Some(index) = replace {
            let member = &obj.collection_members[index];
            let entry = member.kind != CollectionMemberKind::Property;
            obj.member_values.insert(
                CacheKey {
                    owner: None,
                    name: member.name.clone(),
                    method: false,
                    entry,
                },
                value,
            );
            return Ok(());
        }
        let kind = match obj.collection_kind {
            CollectionKind::Listing | CollectionKind::List => CollectionMemberKind::Element,
            CollectionKind::Dynamic if matches!(&key, Value::String(_)) => {
                CollectionMemberKind::Property
            }
            _ => CollectionMemberKind::Entry,
        };
        let entry = kind != CollectionMemberKind::Property;
        let name = if kind == CollectionMemberKind::Property {
            let Value::String(s) = &key else {
                unreachable!()
            };
            s.clone()
        } else {
            format!("\0yaml:{}", obj.collection_members.len()).into()
        };
        obj.collection_members.push(CollectionMember {
            key,
            name: name.clone(),
            kind,
        });
        obj.members.push(Member {
            flags: 0,
            name: name.clone(),
            code: usize::MAX,
            local: false,
            method: false,
            entry,
            ty: None,
            type_span: None,
            constant: false,
            fixed: false,
        });
        // Constant source values remain subject to inherited member type checks.
        let cache_key = CacheKey {
            owner: None,
            name,
            method: false,
            entry,
        };
        if obj.collection_kind == CollectionKind::List {
            obj.complete_cache(cache_key, value);
        } else {
            obj.member_values.insert(cache_key, value);
        }
        obj.structural_key = None;
        obj.java_hash = None;
        Ok(())
    }
}
