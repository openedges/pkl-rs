//! Object materialization, strict collection bridges and lazy object adapters.
//! Shallow forcing completes before first-definition iteration or result hashing.
use super::*;
use crate::heap::{CollectionKind, CollectionMember, CollectionMemberKind};

#[derive(Clone, Debug)]
pub struct Delegate {
    pub source: ObjectId,
    map: bool,
    fallback: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    ToMap,
    ToList,
    ToSet,
    SelectMap,
    ToListing,
    ToDynamic,
    ToMapping,
    ToTyped,
    TypedToDynamic,
    SetAdd,
    Length,
    Identity,
}

impl Operation {
    fn target(self) -> CollectionKind {
        match self {
            Self::ToMap | Self::SelectMap => CollectionKind::Map,
            Self::ToList => CollectionKind::List,
            Self::ToSet => CollectionKind::Set,
            Self::ToListing => CollectionKind::Listing,
            Self::ToMapping => CollectionKind::Mapping,
            Self::ToDynamic => CollectionKind::Dynamic,
            _ => unreachable!(),
        }
    }
}

pub(super) fn blocks(
    blocks: &mut Vec<Vec<Instruction>>,
    metadata: &mut Vec<BlockMetadata>,
) -> BTreeMap<(Arc<str>, Arc<str>), (usize, usize)> {
    use Operation::*;
    let source = crate::stdlib_sources::base();
    let mut result = BTreeMap::new();
    for (class, name, operation, arity) in [
        ("Typed", "toMap", ToMap, 0),
        ("Dynamic", "toMap", ToMap, 0),
        ("Mapping", "toMap", ToMap, 0),
        ("Dynamic", "toList", ToList, 0),
        ("Listing", "toList", ToList, 0),
        ("Listing", "toSet", ToSet, 0),
        ("Map", "toMap", Identity, 0),
        ("List", "toMap", SelectMap, 2),
        ("Set", "toMap", SelectMap, 2),
        ("List", "toListing", ToListing, 0),
        ("Set", "toListing", ToListing, 0),
        ("List", "toDynamic", ToDynamic, 0),
        ("Set", "toDynamic", ToDynamic, 0),
        ("Map", "toDynamic", ToDynamic, 0),
        ("Map", "toMapping", ToMapping, 0),
        ("Typed", "toDynamic", TypedToDynamic, 0),
        ("Dynamic", "toTyped", ToTyped, 1),
        ("Map", "toTyped", ToTyped, 1),
        ("Set", "add", SetAdd, 1),
        ("Dynamic", "length", Length, 0),
    ] {
        let class_marker = match class {
            "Typed" | "Dynamic" => format!("class {class} extends"),
            _ => format!("class {class}<"),
        };
        let class_start = source.text.find(&class_marker).unwrap();
        let class_end = class_start + source.text[class_start..].find("\n}").unwrap();
        let start = class_start
            + source.text[class_start..class_end]
                .find(&format!("external function {name}"))
                .unwrap();
        let end = start + source.text[start..].find('\n').unwrap();
        let mut code = Vec::new();
        let mut spans = Vec::new();
        if operation == SelectMap {
            for (local, label, ty) in [
                (1, "keyExtractor: ", "(Element) -> Key"),
                (0, "valueExtractor: ", "(Element) -> Value"),
            ] {
                let position = start + source.text[start..].find(label).unwrap() + label.len();
                code.extend([
                    Instruction::LoadLocal(local),
                    Instruction::CheckType(TypeCheck::Function {
                        parameters: Some(vec![TypeCheck::Any]),
                        result: Box::new(TypeCheck::Any),
                        syntax: true,
                    }),
                    Instruction::Pop,
                ]);
                spans.extend([None, Some(Span::new(position, position + ty.len())), None]);
            }
        }
        if operation == ToTyped {
            let position = start + source.text[start..].find("Class<Type>").unwrap();
            code.extend([
                Instruction::LoadLocal(0),
                Instruction::CheckType(TypeCheck::MetaClass("Class".into())),
                Instruction::Pop,
            ]);
            spans.extend([
                None,
                Some(Span::new(position, position + "Class<Type>".len())),
                None,
            ]);
        }
        if matches!(
            operation,
            ToTyped | TypedToDynamic | SetAdd | Length | Identity
        ) {
            code.push(Instruction::ConvertObject(operation));
            if operation == SetAdd {
                // VmSet.add has already performed contains in ConvertObject.
                // Persistent put must not repeat that lookup on the cloned Set.
                code.extend([
                    Instruction::PutSetKey,
                    Instruction::LoadLocal(0),
                    Instruction::AppendStrictElement,
                    Instruction::EndConversion,
                ]);
            }
        } else {
            code.push(Instruction::BeginConversion(operation));
            let next = code.len();
            code.push(Instruction::ConversionNext { end: 0 });
            if operation == SelectMap {
                code.extend([
                    Instruction::Call {
                        arity: 1,
                        method: false,
                    },
                    Instruction::ConversionValue,
                    Instruction::Call {
                        arity: 1,
                        method: false,
                    },
                    Instruction::ConversionPair,
                ]);
            }
            match operation.target() {
                CollectionKind::Map => {
                    code.extend([Instruction::PrepareMapEntry, Instruction::AddMapEntry])
                }
                CollectionKind::Set => {
                    code.extend([Instruction::PrepareSetElement, Instruction::AddSetElement])
                }
                CollectionKind::Mapping | CollectionKind::Dynamic => {
                    code.push(Instruction::AppendConvertedMember)
                }
                _ => code.push(Instruction::AppendStrictElement),
            }
            code.push(Instruction::Jump(next));
            code[next] = Instruction::ConversionNext { end: code.len() };
            code.push(Instruction::EndConversion);
        }
        code.push(Instruction::Return);
        spans.resize(code.len(), None);
        let block = blocks.len();
        blocks.push(code);
        metadata.push(BlockMetadata {
            source: Some(source.clone()),
            member: format!("#{class}.{name}").into(),
            declaration: Some(Span::new(start, end)),
            spans,
            ..Default::default()
        });
        result.insert((class.into(), name.into()), (block, arity));
    }
    result
}

pub(super) struct Run {
    operation: Operation,
    source: ObjectId,
    members: Vec<collections::RenderMember>,
    next: usize,
    shallow: bool,
    strict: bool,
    seen: Vec<CollectionMember>,
    current: Option<Value>,
    key: Option<Value>,
    kind: CollectionMemberKind,
}
impl Run {
    pub(super) fn roots(&self, roots: &mut Vec<Value>) {
        roots.push(Value::Object(self.source));
        roots.extend(self.members.iter().map(|m| m.descriptor.key.clone()));
        roots.extend(self.seen.iter().map(|m| m.key.clone()));
        roots.extend(self.current.iter().chain(self.key.iter()).cloned());
    }
}

impl Vm {
    pub(super) fn conversion_method(
        &mut self,
        receiver: Value,
        name: &str,
    ) -> Result<bool, RuntimeError> {
        let Value::Object(source) = receiver else {
            return Ok(false);
        };
        let object = self.heap.get(source)?;
        // Pair's heap prototype has a class definition, but VmPair extends Any,
        // not Typed. Reject its conversion/lookup dispatch before arguments run.
        if object.pair.is_some() {
            return Ok(false);
        }
        let class: Arc<str> =
            if object.is_module || collections::typed_class(&self.heap, source)?.is_some() {
                "Typed".into()
            } else if object.reference.is_none() && object.pair.is_none() {
                format!("{:?}", object.collection_kind).into()
            } else {
                return Ok(false);
            };
        if class.as_ref() == "Typed"
            && matches!(name, "hasProperty" | "getProperty" | "getPropertyOrNull")
            && !self.typed_has_property_available(source)?
        {
            return Ok(false);
        }
        let Some(&(block, arity)) = self.program.conversion_blocks.get(&(class, name.into()))
        else {
            return Ok(false);
        };
        let callable = self.heap.allocate(Object {
            function: Some(Function {
                block,
                arity,
                method: true,
                captured_this: Some(receiver),
            }),
            ..Default::default()
        });
        self.stack.push(Value::Function(callable));
        Ok(true)
    }

    pub(super) fn conversion_receiver(&self) -> Result<ObjectId, RuntimeError> {
        self.frames
            .last()
            .and_then(|f| f.environment.this_value.as_ref())
            .ok_or_else(|| invalid("conversion receiver"))?
            .as_object()
    }
    pub(super) fn conversion_argument(&self, depth: usize) -> Result<Value, RuntimeError> {
        let locals = &self
            .frames
            .last()
            .ok_or_else(|| invalid("conversion frame"))?
            .environment
            .locals;
        locals
            .get(locals.len().wrapping_sub(depth + 1))
            .cloned()
            .ok_or_else(|| invalid("conversion argument"))
    }
    pub(super) fn begin_conversion(&mut self, operation: Operation) -> Result<(), RuntimeError> {
        let source = self.conversion_receiver()?;
        let kind = self.heap.get(source)?.collection_kind;
        let strict = matches!(
            kind,
            CollectionKind::List | CollectionKind::Set | CollectionKind::Map
        );
        let members = if strict {
            collections::ordered_members(&self.heap, source)?
                .into_iter()
                .map(|descriptor| collections::RenderMember {
                    descriptor,
                    skip: false,
                    code: None,
                })
                .collect()
        } else {
            collections::render_members(&self.heap, source)?
        };
        let builder = self.heap.allocate(Object {
            collection_kind: operation.target(),
            ..Default::default()
        });
        let frame = self.frames.last_mut().unwrap();
        frame.builders.push(builder);
        frame.conversion_run = Some(Box::new(Run {
            operation,
            source,
            members,
            next: 0,
            shallow: !strict,
            strict,
            seen: Vec::new(),
            current: None,
            key: None,
            kind: CollectionMemberKind::Element,
        }));
        if !strict {
            self.force_shallow_frame(Value::Object(source))?;
        }
        Ok(())
    }

    pub(super) fn conversion_next(&mut self, end: usize) -> Result<(), RuntimeError> {
        if self
            .frames
            .last()
            .and_then(|f| f.conversion_run.as_ref())
            .is_some_and(|r| r.shallow)
        {
            self.pop()?;
            self.frames
                .last_mut()
                .unwrap()
                .conversion_run
                .as_mut()
                .unwrap()
                .shallow = false;
        }
        let run = self
            .frames
            .last()
            .and_then(|f| f.conversion_run.as_ref())
            .ok_or_else(|| invalid("conversion cursor"))?;
        let Some(item) = run.members.get(run.next).cloned() else {
            return self.jump(end);
        };
        let source = run.source;
        let operation = run.operation;
        let member = item.descriptor;
        if !run.strict {
            let candidates: Vec<_> = run
                .seen
                .iter()
                .filter(|m| {
                    (m.kind == CollectionMemberKind::Property)
                        == (member.kind == CollectionMemberKind::Property)
                })
                .map(|m| m.key.clone())
                .collect();
            if self.prepare_keys(vec![member.key.clone()])? {
                return Ok(());
            }
            let hash = self.java_hash(&member.key)?;
            let candidates = candidates
                .into_iter()
                .filter(|value| {
                    self.java_hash(value)
                        .is_ok_and(|other| !hash.zip(other).is_some_and(|(a, b)| a != b))
                })
                .collect::<Vec<_>>();
            if self.search_key(&member.key, candidates.clone())? {
                return Ok(());
            }
            let seen = candidates
                .iter()
                .any(|key| equality::same_key(&self.heap, key, &member.key));
            let run = self
                .frames
                .last_mut()
                .unwrap()
                .conversion_run
                .as_mut()
                .unwrap();
            run.next += 1;
            if !seen {
                run.seen.push(member.clone());
            }
            let include = match operation {
                Operation::ToMap => member.kind != CollectionMemberKind::Element,
                Operation::ToList | Operation::ToSet => {
                    member.kind == CollectionMemberKind::Element
                }
                _ => true,
            };
            if seen || item.skip || !include {
                self.frames.last_mut().unwrap().pc -= 1;
                return Ok(());
            }
        } else {
            self.frames
                .last_mut()
                .unwrap()
                .conversion_run
                .as_mut()
                .unwrap()
                .next += 1;
        }
        let value = collections::cached_render_value(&self.heap, source, &member)?
            .ok_or_else(|| invalid("uncached conversion value"))?;
        if operation == Operation::SelectMap {
            let callback = self.conversion_argument(1)?;
            self.frames
                .last_mut()
                .unwrap()
                .conversion_run
                .as_mut()
                .unwrap()
                .current = Some(value.clone());
            self.stack.extend([callback, value]);
        } else if operation == Operation::ToMap {
            self.stack.extend([member.key, value]);
        } else {
            let map_source = self.heap.get(source)?.collection_kind == CollectionKind::Map;
            let run = self
                .frames
                .last_mut()
                .unwrap()
                .conversion_run
                .as_mut()
                .unwrap();
            run.kind = if map_source {
                if operation == Operation::ToDynamic && matches!(member.key, Value::String(_)) {
                    CollectionMemberKind::Property
                } else {
                    CollectionMemberKind::Entry
                }
            } else {
                CollectionMemberKind::Element
            };
            run.key = Some(member.key);
            self.stack.push(value);
        }
        Ok(())
    }

    pub(super) fn conversion_value(&mut self) -> Result<(), RuntimeError> {
        let key = self.pop()?;
        let callback = self.conversion_argument(0)?;
        let run = self
            .frames
            .last_mut()
            .unwrap()
            .conversion_run
            .as_mut()
            .unwrap();
        run.key = Some(key);
        self.stack.extend([callback, run.current.clone().unwrap()]);
        Ok(())
    }
    pub(super) fn conversion_pair(&mut self) -> Result<(), RuntimeError> {
        let value = self.pop()?;
        let run = self
            .frames
            .last_mut()
            .unwrap()
            .conversion_run
            .as_mut()
            .unwrap();
        run.current = None;
        self.stack.extend([run.key.take().unwrap(), value]);
        Ok(())
    }
    pub(super) fn append_converted_member(&mut self) -> Result<(), RuntimeError> {
        let value = self.pop()?;
        let run = self.frames.last().unwrap().conversion_run.as_ref().unwrap();
        let kind = run.kind;
        let key = run.key.clone().unwrap();
        let name = if kind == CollectionMemberKind::Property {
            let Value::String(name) = &key else {
                unreachable!()
            };
            property_identifier(name)
        } else {
            Arc::from("")
        };
        let member = Member {
            flags: 0,
            name,
            code: usize::MAX,
            local: false,
            method: false,
            entry: kind != CollectionMemberKind::Property,
            ty: None,
            type_span: None,
            constant: false,
            fixed: false,
        };
        if kind != CollectionMemberKind::Element && self.prepare_insertion(&key, kind)? {
            self.stack.push(value);
            return Ok(());
        }
        self.add_collection_member(
            member,
            (kind != CollectionMemberKind::Element).then_some(key),
            kind,
            Some(value),
        )
    }
    pub(super) fn end_conversion(&mut self) -> Result<(), RuntimeError> {
        let frame = self.frames.last_mut().unwrap();
        frame.conversion_run.take();
        let builder = frame.builders.pop().unwrap();
        let id = self.finish_strict_collection(builder)?;
        self.stack.push(Value::Object(id));
        Ok(())
    }
    pub(super) fn convert_object(&mut self, operation: Operation) -> Result<(), RuntimeError> {
        let source = self.conversion_receiver()?;
        match operation {
            Operation::Identity => self.stack.push(Value::Object(source)),
            Operation::Length => self
                .stack
                .push(Value::Int((self.heap.get(source)?.length as i64).into())),
            Operation::SetAdd => {
                let value = self.conversion_argument(0)?;
                if self.prepare_set_lookup(source, &value)? {
                    return Ok(());
                }
                if self.set_contains(source, &value)? {
                    self.stack.push(Value::Object(source));
                    let frame = self.frames.last_mut().unwrap();
                    frame.pc = self.program.blocks[frame.block].len() - 1;
                } else {
                    let mut copy = self.heap.get(source)?.clone();
                    copy.structural_key = None;
                    copy.java_hash = None;
                    copy.forced = false;
                    let builder = self.heap.allocate(copy);
                    self.frames.last_mut().unwrap().builders.push(builder);
                    self.stack.push(value);
                }
            }
            Operation::TypedToDynamic | Operation::ToTyped => {
                self.convert_delegating_object(source, operation)?
            }
            _ => unreachable!(),
        }
        Ok(())
    }

    fn convert_delegating_object(
        &mut self,
        source: ObjectId,
        operation: Operation,
    ) -> Result<(), RuntimeError> {
        let typed = operation == Operation::ToTyped;
        let (parent, class, properties) = if typed {
            let Value::Class(class) = self.conversion_argument(0)? else {
                return Err(invalid("checked conversion class"));
            };
            let base = self.canonical_builtin_class(classes::BuiltinClass::Typed);
            if !self.heap.is_subclass(class, base) {
                return Err(RuntimeError::new(
                    ErrorKind::Type,
                    format!(
                        "Class `{}` is not a subtype of `Typed`.",
                        self.heap.class_display_name(class)?
                    ),
                ));
            }
            let parent = self.heap.get_class(class)?.prototype;
            let declaration = self
                .heap
                .get(parent)?
                .class_definition
                .as_ref()
                .ok_or_else(|| invalid("conversion class prototype"))?;
            self.check_class_instantiable(declaration)?;
            (parent, Some(class), parent)
        } else {
            let dynamic = self.canonical_builtin_class(classes::BuiltinClass::Dynamic);
            let parent = self.heap.get_class(dynamic)?.prototype;
            let class = self
                .heap
                .object_class(source)?
                .ok_or_else(|| invalid("Typed conversion class"))?;
            (parent, None, self.heap.get_class(class)?.prototype)
        };
        let mut chain = Vec::new();
        let mut current = Some(properties);
        while let Some(owner) = current {
            chain.push(owner);
            current = self.heap.get(owner)?.parent;
        }
        let mut names = BTreeSet::new();
        let mut members = Vec::new();
        for owner in chain.into_iter().rev() {
            for member in &self.heap.get(owner)?.members {
                if member.entry
                    || member.local
                    || member.method
                    || member.definition()
                    || !names.insert(member.name.clone())
                    || collections::hidden_property(&self.heap, properties, &member.name, false)?
                {
                    continue;
                }
                members.push(Member {
                    name: member.name.clone(),
                    code: self.program.conversion_delegate_block,
                    flags: 0,
                    entry: false,
                    local: false,
                    method: false,
                    ty: None,
                    type_span: None,
                    constant: false,
                    fixed: false,
                });
            }
        }
        let map = self.heap.get(source)?.collection_kind == CollectionKind::Map;
        let result = self.heap.allocate(Object {
            parent: Some(parent),
            instance_class: class,
            members,
            conversion_delegate: Some(Box::new(Delegate {
                source,
                map,
                fallback: typed,
            })),
            ..Default::default()
        });
        self.stack.push(Value::Object(result));
        Ok(())
    }

    pub(super) fn read_conversion_property(&mut self) -> Result<(), RuntimeError> {
        let force = self
            .frames
            .last()
            .and_then(|f| f.force.as_ref())
            .ok_or_else(|| invalid("delegating property frame"))?;
        let owner = force.owner;
        let name = force.name.clone();
        let delegate = self
            .heap
            .get(owner)?
            .conversion_delegate
            .as_ref()
            .ok_or_else(|| invalid("delegating property owner"))?
            .clone();
        if delegate.map {
            let key = Value::string(name.clone());
            if self.prepare_map_lookup(delegate.source, &key)? {
                return Ok(());
            }
            if let Some(value) = self.map_value(delegate.source, &key)? {
                self.stack.push(value);
                return Ok(());
            }
        } else {
            let mut current = Some(delegate.source);
            while let Some(id) = current {
                let object = self.heap.get(id)?;
                if object
                    .members
                    .iter()
                    .any(|m| m.name == name && !m.local && !m.entry && !m.method)
                {
                    return self.force(delegate.source, delegate.source, name, false, true, false);
                }
                current = object.parent;
            }
        }
        if delegate.fallback {
            let parent = self
                .heap
                .get(owner)?
                .parent
                .ok_or_else(|| invalid("delegation default"))?;
            self.force(parent, parent, name, false, true, false)
        } else {
            self.force(delegate.source, delegate.source, name, false, true, false)
        }
    }
}

/// Quoted authored identifiers allow NUL, but their lexer forbids LF. Reserve
/// an LF-prefixed namespace for arbitrary UTF-16 strings, escaping LF-bearing
/// strings too. The original descriptor key remains the public property name.
pub(super) fn property_identifier(name: &PklString) -> Arc<str> {
    if let Some(name) = name.as_str().filter(|name| !name.contains('\n')) {
        return Arc::from(name);
    }
    format!("\nproperty:{:?}", name.encode_utf16().collect::<Vec<_>>()).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn result(vm: &mut Vm) -> Value {
        result_with_roots(vm, &[])
    }
    fn result_with_roots(vm: &mut Vm, roots: &[Value]) -> Value {
        loop {
            vm.collect(roots);
            match vm.poll(1) {
                VmPoll::Yielded => {}
                VmPoll::Ready(value) => return value.unwrap(),
                event => panic!("{event:?}"),
            }
        }
    }

    fn source(kind: CollectionKind) -> Vec<Instruction> {
        use Instruction as I;
        if kind == CollectionKind::List {
            return vec![
                I::Constant(Value::Int(1.into())),
                I::MakeList { length: 1 },
                I::Return,
            ];
        }
        let mut code = vec![I::BeginCollection {
            kind,
            has_parent: false,
            specialized: true,
        }];
        if kind == CollectionKind::Map {
            code.push(I::Constant(Value::string("key")));
        }
        code.push(I::Constant(Value::Int(1.into())));
        code.extend(if kind == CollectionKind::Map {
            vec![I::PrepareMapEntry, I::AddMapEntry]
        } else {
            vec![I::PrepareSetElement, I::AddSetElement]
        });
        code.extend([I::EndCollection, I::Return]);
        code
    }

    #[test]
    fn synthetic_results_keep_values_without_retaining_source_or_environment() {
        use Instruction as I;
        for (kind, method) in [
            (CollectionKind::List, "toListing"),
            (CollectionKind::Set, "toListing"),
            (CollectionKind::List, "toDynamic"),
            (CollectionKind::Set, "toDynamic"),
            (CollectionKind::Map, "toMapping"),
            (CollectionKind::Map, "toDynamic"),
        ] {
            let program = Arc::new(Program::new(vec![
                source(kind),
                vec![
                    I::Input(0),
                    I::GetMethod(method.into()),
                    I::Call {
                        arity: 0,
                        method: true,
                    },
                    I::Return,
                ],
                vec![I::Constant(Value::Null), I::Return],
            ]));
            let mut vm = Vm::new(program);
            vm.start(0, vec![]).unwrap();
            let original = result(&mut vm);
            let original_id = original.as_object().unwrap();
            vm.start(1, vec![original]).unwrap();
            let converted = result(&mut vm);
            let converted_id = converted.as_object().unwrap();
            assert_ne!(original_id, converted_id);
            vm.start(2, vec![]).unwrap();
            assert_eq!(
                result_with_roots(&mut vm, std::slice::from_ref(&converted)),
                Value::Null
            );
            vm.collect(std::slice::from_ref(&converted));
            assert!(vm.heap.get(original_id).is_err(), "{kind:?}.{method}");
            let object = vm.heap.get(converted_id).unwrap();
            assert!(object.member_environments.is_empty());
            assert!(object.conversion_delegate.is_none());
            assert_eq!(object.member_values.len(), 1);
            assert_eq!(
                object.member_values.values().next(),
                Some(&Value::Int(1.into()))
            );
            vm.collect(&[]);
            assert!(vm.heap.get(converted_id).is_err());
            vm.close();
            assert_eq!(vm.heap_stats().slots, 0);
        }
    }

    #[test]
    fn map_identity_and_set_duplicate_add_preserve_original_handles() {
        use Instruction as I;
        for (kind, method, argument) in [
            (CollectionKind::Map, "toMap", None),
            (CollectionKind::Set, "add", Some(Value::Int(1.into()))),
        ] {
            let mut call = vec![I::Input(0), I::GetMethod(method.into())];
            if let Some(value) = &argument {
                call.push(I::Constant(value.clone()));
            }
            call.extend([
                I::Call {
                    arity: usize::from(argument.is_some()),
                    method: true,
                },
                I::Return,
            ]);
            let mut vm = Vm::new(Arc::new(Program::new(vec![source(kind), call])));
            vm.start(0, vec![]).unwrap();
            let original = result(&mut vm);
            vm.start(1, vec![original.clone()]).unwrap();
            assert_eq!(result(&mut vm), original);
            vm.close();
            assert_eq!(vm.heap_stats().slots, 0);
        }
    }
}
