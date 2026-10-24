//! Strict collection transforms use ordinary bytecode continuations. Each
//! callback and each hash/insertion finishes before advancing the input cursor.
use super::*;
use crate::heap::{CollectionKind, CollectionMember, CollectionMemberKind};

#[derive(Clone, Copy, Debug)]
pub enum Transform {
    Map,
    Indexed,
    NonNull,
    NonNullIndexed,
    Keys,
    Values,
}

#[derive(Clone, Copy, Debug)]
pub enum Projection {
    ToList,
    ToSet,
    ToBytes,
    Reverse,
    Add,
    KeysHash,
    KeysOrder,
    Values,
}

#[derive(Clone, Copy)]
enum Operation {
    Callback(Transform),
    Projection(Projection),
}

impl Transform {
    fn name(self) -> &'static str {
        match self {
            Self::Map => "map",
            Self::Indexed => "mapIndexed",
            Self::NonNull => "mapNonNull",
            Self::NonNullIndexed => "mapNonNullIndexed",
            Self::Keys => "mapKeys",
            Self::Values => "mapValues",
        }
    }

    fn indexed(self) -> bool {
        matches!(self, Self::Indexed | Self::NonNullIndexed)
    }
}

pub(super) fn blocks(
    blocks: &mut Vec<Vec<Instruction>>,
    metadata: &mut Vec<BlockMetadata>,
) -> [usize; 11] {
    use Transform::*;
    let source = crate::stdlib_sources::base();
    let methods = [
        ("List", Map),
        ("List", Indexed),
        ("List", NonNull),
        ("List", NonNullIndexed),
        ("Set", Map),
        ("Set", Indexed),
        ("Set", NonNull),
        ("Set", NonNullIndexed),
        ("Map", Map),
        ("Map", Keys),
        ("Map", Values),
    ];
    methods.map(|(class, operation)| {
        let class_start = source
            .text
            .find(&format!("external class {class}<"))
            .expect("pinned class");
        let start = class_start
            + source.text[class_start..]
                .find(&format!("external function {}<", operation.name()))
                .expect("pinned transform");
        let parameter = start
            + source.text[start..]
                .find("transform: ")
                .expect("transform parameter")
            + "transform: ".len();
        // The function type includes the argument tuple and the result type;
        // delimit it at the method parameter's closing delimiter, not the tuple.
        let arrow = parameter
            + source.text[parameter..]
                .find(" -> ")
                .expect("function arrow");
        let result_end = arrow
            + 4
            + source.text[arrow + 4..]
                .find([')', '\n'])
                .expect("function result end");
        let type_end = source.text[arrow + 4..result_end]
            .trim_end_matches(',')
            .len()
            + arrow
            + 4;
        let closing = arrow + source.text[arrow..].find("):").expect("method return type");
        let declaration_end = closing + source.text[closing..].find('\n').unwrap_or(0);
        let arity = if class == "Map" || operation.indexed() {
            2
        } else {
            1
        };
        let mut code = vec![
            Instruction::LoadLocal(0),
            Instruction::CheckType(TypeCheck::Function {
                parameters: Some(vec![TypeCheck::Any; arity]),
                result: Box::new(TypeCheck::Any),
                syntax: true,
            }),
            Instruction::BeginCollectionTransform(operation),
            Instruction::CollectionTransformNext { end: 0 },
            Instruction::Call {
                arity,
                method: false,
            },
            Instruction::CollectionTransformProject { next: 3 },
        ];
        match class {
            "List" => code.push(Instruction::AppendStrictElement),
            "Set" => code.extend([Instruction::PrepareSetElement, Instruction::AddSetElement]),
            "Map" => code.extend([Instruction::PrepareMapEntry, Instruction::AddMapEntry]),
            _ => unreachable!(),
        }
        code.push(Instruction::Jump(3));
        let end = code.len();
        code[3] = Instruction::CollectionTransformNext { end };
        code.extend([Instruction::EndCollectionTransform, Instruction::Return]);
        let mut spans = vec![None; code.len()];
        spans[1] = Some(Span::new(parameter, type_end));
        let block = blocks.len();
        blocks.push(code);
        metadata.push(BlockMetadata {
            source: Some(source.clone()),
            member: format!("#{class}.{}", operation.name()).into(),
            declaration: Some(Span::new(start, declaration_end)),
            spans,
            ..BlockMetadata::default()
        });
        block
    })
}

pub(super) fn projection_blocks(
    blocks: &mut Vec<Vec<Instruction>>,
    metadata: &mut Vec<BlockMetadata>,
) -> [usize; 10] {
    let source = crate::stdlib_sources::base();
    [
        ("List", "toList", None),
        ("List", "toSet", Some(Projection::ToSet)),
        ("Set", "toList", Some(Projection::ToList)),
        ("Set", "toSet", None),
        ("Map", "keys", Some(Projection::KeysHash)),
        ("Map", "values", Some(Projection::Values)),
        ("List", "toBytes", Some(Projection::ToBytes)),
        ("List", "add", Some(Projection::Add)),
        ("List", "reverse", Some(Projection::Reverse)),
        ("Set", "reverse", Some(Projection::Reverse)),
    ]
    .map(|(class, name, operation)| {
        let class_start = source
            .text
            .find(&format!("external class {class}<"))
            .expect("collection class");
        let signature = if name == "add" {
            "external function add<Other>(element: Other):".to_owned()
        } else if class == "Map" {
            format!("external {name}:")
        } else {
            format!("external function {name}():")
        };
        let start = class_start
            + source.text[class_start..]
                .find(&signature)
                .expect("collection projection");
        let end = start + source.text[start..].find('\n').unwrap();
        let mut code = match operation {
            None => vec![Instruction::LoadThis, Instruction::Return],
            Some(Projection::KeysHash) => vec![
                Instruction::BeginCollectionProjection(Projection::KeysHash),
                Instruction::CollectionTransformNext { end: 4 },
                Instruction::PutSetKey,
                Instruction::Jump(1),
                Instruction::CollectionKeyOrder,
                Instruction::CollectionTransformNext { end: 8 },
                Instruction::AppendStrictElement,
                Instruction::Jump(5),
            ],
            Some(operation) => {
                let mut code = vec![
                    Instruction::BeginCollectionProjection(operation),
                    Instruction::CollectionTransformNext { end: 0 },
                ];
                if matches!(operation, Projection::ToSet) {
                    code.extend([Instruction::PrepareSetElement, Instruction::AddSetElement]);
                } else {
                    if matches!(operation, Projection::ToBytes) {
                        code.push(Instruction::CheckByte);
                    }
                    code.push(Instruction::AppendStrictElement);
                }
                code.push(Instruction::Jump(1));
                code[1] = Instruction::CollectionTransformNext { end: code.len() };
                code
            }
        };
        if matches!(operation, Some(Projection::Add)) {
            code.extend([Instruction::LoadLocal(0), Instruction::AppendStrictElement]);
        }
        if operation.is_some() {
            code.extend([Instruction::EndCollectionTransform, Instruction::Return]);
        }
        let spans = vec![None; code.len()];
        let block = blocks.len();
        blocks.push(code);
        metadata.push(BlockMetadata {
            source: Some(source.clone()),
            member: format!("#{class}.{name}").into(),
            declaration: Some(Span::new(start, end)),
            spans,
            ..Default::default()
        });
        block
    })
}

pub(super) struct TransformRun {
    operation: Operation,
    source: ObjectId,
    callback: Option<Value>,
    members: Vec<CollectionMember>,
    next: usize,
    current: Option<(Value, Value)>,
}

impl TransformRun {
    pub(super) fn roots(&self, roots: &mut Vec<Value>) {
        roots.push(Value::Object(self.source));
        roots.extend(self.callback.iter().cloned());
        if let Some((key, value)) = &self.current {
            roots.extend([key.clone(), value.clone()]);
        }
    }
}

impl Vm {
    pub(super) fn collection_projection_member(
        &mut self,
        receiver: Value,
        name: &str,
        method: bool,
    ) -> Result<bool, RuntimeError> {
        let index = match (method, name) {
            (true, "toList") if self.is_list(&receiver) => 0,
            (true, "toSet") if self.is_list(&receiver) => 1,
            (true, "toList") if self.is_set(&receiver) => 2,
            (true, "toSet") if self.is_set(&receiver) => 3,
            (false, "keys") if self.is_map(&receiver) => 4,
            (false, "values") if self.is_map(&receiver) => 5,
            (true, "toBytes") if self.is_list(&receiver) => 6,
            (true, "add") if self.is_list(&receiver) => 7,
            (true, "reverse") if self.is_list(&receiver) => 8,
            (true, "reverse") if self.is_set(&receiver) => 9,
            _ => return Ok(false),
        };
        let id = self.heap.allocate(Object {
            function: Some(Function {
                block: self.program.collection_projection_blocks[index],
                arity: usize::from(index == 7),
                method: true,
                captured_this: Some(receiver),
            }),
            ..Default::default()
        });
        if method {
            self.stack.push(Value::Function(id));
        } else {
            self.invoke_function(id, Vec::new(), false)?;
        }
        Ok(true)
    }

    pub(super) fn collection_transform_method(&mut self, receiver: Value, name: &str) -> bool {
        let offset = if self.is_list(&receiver) {
            0
        } else if self.is_set(&receiver) {
            4
        } else if self.is_map(&receiver) {
            8
        } else {
            return false;
        };
        let index = match (offset, name) {
            (_, "map") => 0,
            (0 | 4, "mapIndexed") => 1,
            (0 | 4, "mapNonNull") => 2,
            (0 | 4, "mapNonNullIndexed") => 3,
            (8, "mapKeys") => 1,
            (8, "mapValues") => 2,
            _ => return false,
        };
        let id = self.heap.allocate(Object {
            function: Some(Function {
                block: self.program.collection_transform_blocks[offset + index],
                arity: 1,
                method: true,
                captured_this: Some(receiver),
            }),
            ..Object::default()
        });
        self.stack.push(Value::Function(id));
        true
    }

    pub(super) fn begin_collection_transform(
        &mut self,
        operation: Transform,
    ) -> Result<(), RuntimeError> {
        let callback = self.pop()?;
        self.begin_collection_run(Operation::Callback(operation), Some(callback))
    }

    pub(super) fn begin_collection_projection(
        &mut self,
        operation: Projection,
    ) -> Result<(), RuntimeError> {
        self.begin_collection_run(Operation::Projection(operation), None)
    }

    fn begin_collection_run(
        &mut self,
        operation: Operation,
        callback: Option<Value>,
    ) -> Result<(), RuntimeError> {
        let source = self
            .frames
            .last()
            .and_then(|frame| frame.environment.this_value.clone())
            .ok_or_else(|| invalid("transform receiver"))?
            .as_object()?;
        let mut members = if matches!(operation, Operation::Projection(Projection::KeysHash)) {
            self.map_hash_members(source)?
        } else {
            collections::ordered_members(&self.heap, source)?
        };
        if matches!(operation, Operation::Projection(Projection::Reverse)) {
            members.reverse();
        }
        // A strict result must not capture the method's source/callback scope.
        let builder = self.heap.allocate(Object {
            collection_kind: match operation {
                Operation::Callback(_) => self.heap.get(source)?.collection_kind,
                Operation::Projection(Projection::ToSet | Projection::KeysHash) => {
                    CollectionKind::Set
                }
                Operation::Projection(_) => CollectionKind::List,
            },
            ..Object::default()
        });
        let frame = self.frames.last_mut().expect("transform frame");
        frame.builders.push(builder);
        frame.collection_transform = Some(TransformRun {
            operation,
            source,
            callback,
            members,
            next: 0,
            current: None,
        });
        Ok(())
    }

    pub(super) fn collection_key_order(&mut self) -> Result<(), RuntimeError> {
        let source = self
            .frames
            .last()
            .and_then(|frame| frame.collection_transform.as_ref())
            .ok_or_else(|| invalid("keySet order"))?
            .source;
        let members = collections::ordered_members(&self.heap, source)?;
        let run = self
            .frames
            .last_mut()
            .and_then(|frame| frame.collection_transform.as_mut())
            .expect("keySet order");
        run.members = members;
        run.next = 0;
        run.operation = Operation::Projection(Projection::KeysOrder);
        Ok(())
    }

    pub(super) fn collection_transform_next(&mut self, end: usize) -> Result<(), RuntimeError> {
        let run = self
            .frames
            .last()
            .and_then(|frame| frame.collection_transform.as_ref())
            .ok_or_else(|| invalid("transform cursor"))?;
        let Some(member) = run.members.get(run.next).cloned() else {
            return self.jump(end);
        };
        let source = run.source;
        let map = self.heap.get(source)?.collection_kind == CollectionKind::Map;
        let value = if matches!(
            run.operation,
            Operation::Projection(Projection::KeysHash | Projection::KeysOrder)
        ) {
            member.key.clone()
        } else if map {
            if self.prepare_map_lookup(source, &member.key)? {
                return Ok(());
            }
            self.map_value(source, &member.key)?
                .ok_or_else(|| invalid("transform Map key"))?
        } else {
            collections::cached_render_value(&self.heap, source, &member)?
                .ok_or_else(|| invalid("uncached strict transform element"))?
        };
        let run = self
            .frames
            .last_mut()
            .and_then(|frame| frame.collection_transform.as_mut())
            .expect("transform cursor");
        if let Operation::Callback(operation) = run.operation {
            self.stack
                .push(run.callback.clone().expect("transform callback"));
            if map {
                self.stack.push(member.key.clone());
            } else if operation.indexed() {
                self.stack.push(Value::Int((run.next as i64).into()));
            }
            run.current = Some((member.key, value.clone()));
        }
        self.stack.push(value);
        run.next += 1;
        Ok(())
    }

    pub(super) fn collection_transform_project(&mut self, next: usize) -> Result<(), RuntimeError> {
        let value = self.pop()?;
        let run = self
            .frames
            .last_mut()
            .and_then(|frame| frame.collection_transform.as_mut())
            .ok_or_else(|| invalid("transform result"))?;
        let (key, input) = run
            .current
            .take()
            .ok_or_else(|| invalid("transform input"))?;
        let Operation::Callback(operation) = run.operation else {
            return Err(invalid("projection cannot receive callback result"));
        };
        match operation {
            Transform::NonNull | Transform::NonNullIndexed if value.is_null() => self.jump(next)?,
            Transform::Keys => self.stack.extend([value, input]),
            Transform::Values => self.stack.extend([key, value]),
            Transform::Map if self.heap.get(run.source)?.collection_kind == CollectionKind::Map => {
                let pair = value
                    .as_object()
                    .ok()
                    .and_then(|id| self.heap.get(id).ok())
                    .and_then(|object| object.pair.clone());
                let Some(pair) = pair else {
                    return Err(type_error(
                        &TypeCheck::Pair { arguments: vec![] },
                        &value,
                        &self.program.module_name,
                        &self.heap,
                    )?);
                };
                self.stack.extend(*pair);
            }
            _ => self.stack.push(value),
        }
        Ok(())
    }

    pub(super) fn append_strict_element(&mut self) -> Result<(), RuntimeError> {
        let value = self.pop()?;
        self.add_collection_member(
            Member {
                flags: 0,
                entry: true,
                name: Arc::from(""),
                code: usize::MAX,
                local: false,
                method: false,
                ty: None,
                type_span: None,
                constant: false,
                fixed: false,
            },
            None,
            CollectionMemberKind::Element,
            Some(value),
        )
    }

    pub(super) fn end_collection_transform(&mut self) -> Result<(), RuntimeError> {
        let frame = self.frames.last_mut().expect("transform frame");
        let run = frame
            .collection_transform
            .take()
            .ok_or_else(|| invalid("missing transform cursor"))?;
        let builder = frame
            .builders
            .pop()
            .ok_or_else(|| invalid("missing transform builder"))?;
        if matches!(run.operation, Operation::Projection(Projection::ToBytes)) {
            let data = collections::ordered_members(&self.heap, builder)?
                .iter()
                .map(|member| {
                    match collections::cached_render_value(&self.heap, builder, member)? {
                        Some(Value::Int(value)) => Ok(value.get() as u8),
                        _ => Err(invalid("checked List.toBytes element")),
                    }
                })
                .collect::<Result<Vec<_>, _>>()?;
            let bytes: crate::value::Bytes = data.into();
            self.heap
                .bytes_lists
                .insert(bytes.reference(), (bytes.lifetime(), run.source, false));
            self.stack.push(Value::Bytes(bytes));
            return Ok(());
        }
        let result = self.finish_strict_collection(builder)?;
        self.stack.push(Value::Object(result));
        Ok(())
    }
}

pub(super) fn endpoint_blocks(
    blocks: &mut Vec<Vec<Instruction>>,
    metadata: &mut Vec<BlockMetadata>,
) -> BTreeMap<(Arc<str>, Arc<str>), usize> {
    let source = crate::stdlib_sources::base();
    let mut result = BTreeMap::new();
    for class_name in ["List", "Set"] {
        let class = source
            .text
            .find(&format!("external class {class_name}<"))
            .unwrap();
        for name in ["first", "firstOrNull", "last", "lastOrNull"] {
            let signature = format!("external {name}:");
            let start = class + source.text[class..].find(&signature).unwrap();
            let end = start + source.text[start..].find('\n').unwrap();
            let block = blocks.len();
            blocks.push(vec![
                Instruction::Primitive(primitives::Operation::CollectionEndpoint {
                    last: name.starts_with("last"),
                    nullable: name.ends_with("OrNull"),
                }),
                Instruction::Return,
            ]);
            metadata.push(BlockMetadata {
                source: Some(source.clone()),
                member: format!("#{class_name}.{name}").into(),
                declaration: Some(Span::new(start, end)),
                spans: vec![None; 2],
                ..Default::default()
            });
            result.insert((class_name.into(), name.into()), block);
        }
    }
    result
}

impl Vm {
    pub(super) fn collection_endpoint(
        &mut self,
        receiver: Value,
        last: bool,
        nullable: bool,
    ) -> Result<(), RuntimeError> {
        let id = receiver.as_object()?;
        let members = collections::ordered_members(&self.heap, id)?;
        let member = if last {
            members.last()
        } else {
            members.first()
        };
        let value = if let Some(member) = member {
            collections::cached_render_value(&self.heap, id, member)?
                .ok_or_else(|| invalid("uncached strict collection endpoint"))?
        } else if nullable {
            Value::Null
        } else {
            return Err(RuntimeError::new(
                ErrorKind::Type,
                format!(
                    "Expected a non-empty collection.\nCollection: {:?}()",
                    self.heap.get(id)?.collection_kind
                ),
            ));
        };
        self.stack.push(value);
        Ok(())
    }
}

#[cfg(test)]
mod reversal_tests {
    use super::*;

    fn ready(vm: &mut Vm, roots: &[Value]) -> Value {
        for _ in 0..100_000 {
            vm.collect(roots);
            match vm.poll(1) {
                VmPoll::Yielded => {}
                VmPoll::Ready(value) => return value.unwrap(),
                event => panic!("{event:?}"),
            }
        }
        panic!("reversal poll bound")
    }

    fn source(kind: CollectionKind, empty: bool) -> Vec<Instruction> {
        use Instruction as I;
        let mut code = vec![I::BeginCollection {
            kind,
            has_parent: false,
            specialized: true,
        }];
        if !empty {
            for value in [I::Input(0), I::Constant(Value::Int(7.into()))] {
                code.push(value);
                if kind == CollectionKind::Set {
                    code.extend([I::PrepareSetElement, I::AddSetElement]);
                } else {
                    code.push(I::AppendStrictElement);
                }
            }
        }
        code.extend([I::EndCollection, I::Return]);
        code
    }

    #[test]
    fn reversed_values_keep_elements_but_release_source_and_producer_scope() {
        use Instruction as I;
        for kind in [CollectionKind::List, CollectionKind::Set] {
            for empty in [false, true] {
                let mut vm = Vm::new(Arc::new(Program::new(vec![
                    source(kind, empty),
                    vec![
                        I::Input(0),
                        I::GetMethod("reverse".into()),
                        I::Call {
                            arity: 0,
                            method: true,
                        },
                        I::Return,
                    ],
                    vec![I::Constant(Value::Null), I::Return],
                ])));
                let element = vm.heap.allocate(Object::default());
                vm.start(0, vec![Value::Object(element)]).unwrap();
                let source = ready(&mut vm, &[]);
                let source_id = source.as_object().unwrap();
                let producer = vm.heap.allocate(Object::default());
                if !empty {
                    vm.heap
                        .get_mut(source_id)
                        .unwrap()
                        .environment
                        .locals
                        .push(Value::Object(producer));
                }
                vm.start(1, vec![source.clone()]).unwrap();
                let reversed = ready(&mut vm, &[]);
                let reversed_id = reversed.as_object().unwrap();
                assert_eq!(
                    vm.heap.get(reversed_id).unwrap().collection_kind,
                    CollectionKind::List
                );
                if empty {
                    assert_eq!(
                        reversed_id,
                        vm.empty_strict_collection(CollectionKind::List)
                    );
                } else {
                    assert_ne!(reversed, source);
                    let members = collections::ordered_members(&vm.heap, reversed_id).unwrap();
                    assert_eq!(
                        collections::cached_render_value(&vm.heap, reversed_id, &members[0])
                            .unwrap(),
                        Some(Value::Int(7.into()))
                    );
                    assert_eq!(
                        collections::cached_render_value(&vm.heap, reversed_id, &members[1])
                            .unwrap(),
                        Some(Value::Object(element))
                    );
                }
                vm.start(1, vec![reversed.clone()]).unwrap();
                let twice = ready(&mut vm, std::slice::from_ref(&reversed));
                assert_eq!(twice == reversed, empty);
                vm.start(2, vec![]).unwrap();
                assert_eq!(ready(&mut vm, std::slice::from_ref(&reversed)), Value::Null);
                vm.collect(std::slice::from_ref(&reversed));
                if !empty {
                    assert!(vm.heap.get(source_id).is_err());
                    assert!(vm.heap.get(producer).is_err());
                    assert!(vm.heap.get(element).is_ok());
                    let object = vm.heap.get(reversed_id).unwrap();
                    assert!(object.parent.is_none());
                    assert!(object.environment.locals.is_empty());
                    assert!(object.member_environments.is_empty());
                }
                vm.close();
                assert_eq!(vm.heap_stats().slots, 0);
            }
        }
    }

    #[test]
    fn cancellation_reclaims_a_partially_reversed_builder() {
        use Instruction as I;
        let mut vm = Vm::new(Arc::new(Program::new(vec![
            source(CollectionKind::List, false),
            vec![
                I::Input(0),
                I::GetMethod("reverse".into()),
                I::Call {
                    arity: 0,
                    method: true,
                },
                I::Return,
            ],
        ])));
        vm.start(0, vec![Value::Int(1.into())]).unwrap();
        let source = ready(&mut vm, &[]);
        vm.start(1, vec![source.clone()]).unwrap();
        let builder = loop {
            vm.collect(std::slice::from_ref(&source));
            assert!(matches!(vm.poll(1), VmPoll::Yielded));
            if let Some(frame) = vm.frames.last()
                && let Some(run) = &frame.collection_transform
                && run.next == 1
                && vm.heap.get(*frame.builders.last().unwrap()).unwrap().length == 1
            {
                break *frame.builders.last().unwrap();
            }
        };
        vm.cancel();
        assert!(
            matches!(vm.poll(1), VmPoll::Ready(Err(error)) if error.kind == ErrorKind::Cancelled)
        );
        vm.collect(std::slice::from_ref(&source));
        assert!(vm.heap.get(builder).is_err());
        vm.start(1, vec![source]).unwrap();
        let reversed = ready(&mut vm, &[]);
        assert_eq!(
            vm.heap.get(reversed.as_object().unwrap()).unwrap().length,
            2
        );
        vm.close();
        assert_eq!(vm.heap_stats().slots, 0);
    }
}
