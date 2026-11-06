//! Evaluator-owned pkl:json Parser.parse continuation. JSON scanning pauses at
//! each completed child while ordinary Pkl converter functions execute.
use super::converters::{Converters, PathPart};
use super::*;
use crate::heap::{CollectionKind, CollectionMember, CollectionMemberKind};
pub(crate) mod syntax;
use syntax::{Event, Reader, Scalar};

enum Container {
    Array(Vec<Value>),
    Object {
        members: Vec<(PklString, Value)>,
        key: Option<PklString>,
    },
}
#[derive(Clone, Copy)]
enum Await {
    Text,
    Converters,
    ForcedConverters,
    UseMapping,
    ConvertedChild,
    ConvertedRoot,
}
pub(super) struct Run {
    receiver: ObjectId,
    source: Value,
    mapping: Option<Value>,
    converters: Converters,
    phase: u8,
    await_: Option<Await>,
    use_mapping: bool,
    reader: Option<Reader>,
    containers: Vec<Container>,
    path: Vec<PathPart>,
    root: Option<Value>,
    pending: Option<Value>,
}
impl Run {
    pub(super) fn roots(&self, roots: &mut Vec<Value>) {
        roots.push(Value::Object(self.receiver));
        roots.push(self.source.clone());
        roots.extend(self.mapping.iter().cloned());
        roots.extend(self.root.iter().cloned());
        roots.extend(self.pending.iter().cloned());
        self.converters.roots(roots);
        for container in &self.containers {
            match container {
                Container::Array(values) => roots.extend(values.iter().cloned()),
                Container::Object { members, .. } => {
                    roots.extend(members.iter().map(|(_, value)| value.clone()))
                }
            }
        }
    }
    fn insert_child(&mut self, value: Value) -> Result<(), RuntimeError> {
        match self.containers.last_mut() {
            Some(Container::Array(values)) => values.push(value),
            Some(Container::Object { members, key }) => {
                let key = key.take().ok_or_else(|| invalid("JSON object key"))?;
                // EconomicMap.put replaces the value in its original position.
                // The converter has already run even for an overwritten key.
                if let Some((_, old)) = members.iter_mut().find(|(name, _)| name == &key) {
                    *old = value;
                } else {
                    members.push((key, value));
                }
                self.path.pop();
            }
            None => return Err(invalid("JSON child without container")),
        }
        Ok(())
    }
}
fn parse_error(hint: PklString) -> RuntimeError {
    let mut error = RuntimeError::new(ErrorKind::Type, "Error parsing JSON document.");
    error.hint = Some(hint);
    // ParserNodes' external member body has an unavailable source section.
    error.unavailable_origin = true;
    error
}
impl Vm {
    pub(super) fn begin_json_parse(&mut self) -> Result<(), RuntimeError> {
        let receiver = self.this_receiver()?.as_object()?;
        let source = self
            .frames
            .last()
            .and_then(|frame| frame.environment.locals.last())
            .cloned()
            .ok_or_else(|| invalid("JSON parse argument"))?;
        self.frames.last_mut().unwrap().json_parse = Some(Box::new(Run {
            receiver,
            source,
            mapping: None,
            converters: Converters::default(),
            phase: 0,
            await_: None,
            use_mapping: false,
            reader: None,
            containers: Vec::new(),
            path: vec![PathPart::Root],
            root: None,
            pending: None,
        }));
        Ok(())
    }
    pub(super) fn step_json_parse(&mut self) -> Result<(), RuntimeError> {
        let frame = self.frames.len() - 1;
        let mut run = self.frames[frame]
            .json_parse
            .take()
            .expect("JSON continuation");
        let result = self.advance_json_parse(&mut run);
        match result {
            Ok(true) => self
                .stack
                .push(run.root.take().ok_or_else(|| invalid("JSON root value"))?),
            Ok(false) | Err(_) => self.frames[frame].json_parse = Some(run),
        }
        result.map(|_| ())
    }
    fn advance_json_parse(&mut self, run: &mut Run) -> Result<bool, RuntimeError> {
        if let Some(await_) = run.await_.take() {
            let value = self.pop()?;
            match await_ {
                Await::Text => run.source = value,
                Await::Converters => run.mapping = Some(value),
                Await::ForcedConverters => {}
                Await::UseMapping => {
                    let Value::Bool(value) = value else {
                        return Err(invalid("checked JSON useMapping"));
                    };
                    run.use_mapping = value;
                }
                Await::ConvertedChild => {
                    run.pending.take();
                    run.insert_child(value)?;
                }
                Await::ConvertedRoot => {
                    run.root = Some(value);
                    return Ok(true);
                }
            }
        }
        match run.phase {
            0 => {
                run.phase = 1;
                if let Value::Object(resource) = run.source {
                    // Resource.text is evaluated before converters or useMapping.
                    run.await_ = Some(Await::Text);
                    self.force(resource, resource, "text".into(), false, true, false)?;
                }
                return Ok(false);
            }
            1 => {
                if !matches!(run.source, Value::String(_)) {
                    return Err(invalid("checked JSON source text"));
                }
                run.phase = 2;
                run.await_ = Some(Await::Converters);
                self.force(
                    run.receiver,
                    run.receiver,
                    "converters".into(),
                    false,
                    true,
                    false,
                )?;
                return Ok(false);
            }
            2 => {
                run.phase = 3;
                run.await_ = Some(Await::ForcedConverters);
                self.force_shallow_frame(
                    run.mapping
                        .clone()
                        .ok_or_else(|| invalid("JSON converters"))?,
                )?;
                return Ok(false);
            }
            3 => {
                let mapping = run
                    .mapping
                    .as_ref()
                    .ok_or_else(|| invalid("JSON converter mapping"))?
                    .as_object()?;
                run.converters = Converters::load_parser(&self.heap, mapping)?;
                run.phase = 4;
                run.await_ = Some(Await::UseMapping);
                self.force(
                    run.receiver,
                    run.receiver,
                    "useMapping".into(),
                    false,
                    true,
                    false,
                )?;
                return Ok(false);
            }
            4 => {
                let Value::String(text) = &run.source else {
                    return Err(invalid("JSON text"));
                };
                run.reader = Some(Reader::new(text));
                run.phase = 5;
                return Ok(false);
            }
            _ => {}
        }
        let event = run
            .reader
            .as_mut()
            .ok_or_else(|| invalid("JSON reader"))?
            .next()
            .map_err(parse_error)?;
        let value = match event {
            Event::StartArray => {
                run.containers.push(Container::Array(Vec::new()));
                run.path.push(PathPart::WildcardElement);
                return Ok(false);
            }
            Event::StartObject => {
                run.containers.push(Container::Object {
                    members: Vec::new(),
                    key: None,
                });
                return Ok(false);
            }
            Event::Key(name) => {
                let Some(Container::Object { key, .. }) = run.containers.last_mut() else {
                    return Err(invalid("JSON object name outside object"));
                };
                *key = Some(name.clone());
                // Java uses Identifier even for Mapping output: parser object
                // paths remain properties, unlike renderer Mapping entries.
                run.path.push(PathPart::Property(name));
                return Ok(false);
            }
            Event::EndArray | Event::EndObject => {
                let container = run
                    .containers
                    .pop()
                    .ok_or_else(|| invalid("JSON container end"))?;
                if matches!(container, Container::Array(_)) {
                    run.path.pop();
                }
                self.json_container_value(container, run.use_mapping)?
            }
            Event::Value(scalar) => match scalar {
                Scalar::Null => Value::Null,
                Scalar::Boolean(value) => Value::Bool(value),
                Scalar::String(value) => Value::String(value),
                Scalar::Number(text) => {
                    if let Ok(value) = text.parse::<i64>() {
                        Value::Int(value.into())
                    } else if let Ok(value) = text.parse::<f64>() {
                        Value::Float(value.into())
                    } else {
                        return Err(parse_error(
                            run.reader
                                .as_ref()
                                .unwrap()
                                .error(&format!("Cannot parse `{text}` as number.")),
                        ));
                    }
                }
            },
            Event::EndDocument => {
                let value = run
                    .root
                    .as_ref()
                    .ok_or_else(|| invalid("missing parsed root"))?
                    .clone();
                if let Some(function) = run.converters.select(self, &value, &[PathPart::Root])? {
                    run.await_ = Some(Await::ConvertedRoot);
                    self.invoke_json_converter(function, value)?;
                    return Ok(false);
                }
                return Ok(true);
            }
        };
        if run.containers.is_empty() {
            // The root converter waits until trailing input has been checked.
            run.root = Some(value);
        } else if let Some(function) = run.converters.select(self, &value, &run.path)? {
            run.pending = Some(value.clone());
            run.await_ = Some(Await::ConvertedChild);
            self.invoke_json_converter(function, value)?;
        } else {
            run.insert_child(value)?;
        }
        Ok(false)
    }
    fn invoke_json_converter(&mut self, function: Value, value: Value) -> Result<(), RuntimeError> {
        let Value::Function(function) = function else {
            return Err(invalid("checked JSON converter function"));
        };
        self.invoke_function(function, vec![value], false)
    }
    fn json_container_value(
        &mut self,
        container: Container,
        use_mapping: bool,
    ) -> Result<Value, RuntimeError> {
        let (kind, class, values): (_, _, Vec<(Value, Value)>) = match container {
            Container::Array(values) => (
                CollectionKind::Listing,
                classes::BuiltinClass::Listing,
                values
                    .into_iter()
                    .enumerate()
                    .map(|(index, value)| (Value::Int((index as i64).into()), value))
                    .collect(),
            ),
            Container::Object { members, .. } => (
                if use_mapping {
                    CollectionKind::Mapping
                } else {
                    CollectionKind::Dynamic
                },
                if use_mapping {
                    classes::BuiltinClass::Mapping
                } else {
                    classes::BuiltinClass::Dynamic
                },
                members
                    .into_iter()
                    .map(|(name, value)| (Value::String(name), value))
                    .collect(),
            ),
        };
        let class = self.canonical_builtin_class(class);
        let parent = self.heap.get_class(class)?.prototype;
        let mut object = Object {
            parent: Some(parent),
            collection_kind: kind,
            length: if kind == CollectionKind::Listing {
                values.len()
            } else {
                0
            },
            ..Object::default()
        };
        for (index, (key, value)) in values.into_iter().enumerate() {
            let member_kind = match kind {
                CollectionKind::Listing => CollectionMemberKind::Element,
                CollectionKind::Mapping => CollectionMemberKind::Entry,
                _ => CollectionMemberKind::Property,
            };
            let entry = member_kind != CollectionMemberKind::Property;
            let name = if entry {
                format!("\0json:{index}").into()
            } else {
                let Value::String(name) = &key else {
                    unreachable!()
                };
                name.clone()
            };
            object.collection_members.push(CollectionMember {
                key,
                name: name.clone(),
                kind: member_kind,
            });
            object.members.push(Member {
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
            // Java initConstantValue is not an already-forced cache entry:
            // inherited property/collection types still check on first access.
            object.member_values.insert(
                CacheKey {
                    owner: None,
                    name,
                    method: false,
                    entry,
                },
                value,
            );
        }
        Ok(Value::Object(self.heap.allocate(object)))
    }
}
