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
    Find,
    Filter,
    FlatMap,
    Count,
    Any,
    FilterIndexed,
    FindOrNull,
    FindLast,
    FindLastOrNull,
    FlatMapIndexed,
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
    Distinct,
    IsDistinct,
    Contains,
}

#[derive(Clone, Copy)]
enum Operation {
    Every,
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
            Self::Find => "find",
            Self::Filter => "filter",
            Self::FlatMap => "flatMap",
            Self::Count => "count",
            Self::Any => "any",
            Self::FilterIndexed => "filterIndexed",
            Self::FindOrNull => "findOrNull",
            Self::FindLast => "findLast",
            Self::FindLastOrNull => "findLastOrNull",
            Self::FlatMapIndexed => "flatMapIndexed",
        }
    }

    fn indexed(self) -> bool {
        matches!(
            self,
            Self::Indexed | Self::NonNullIndexed | Self::FilterIndexed | Self::FlatMapIndexed
        )
    }
}

pub(super) fn blocks(
    blocks: &mut Vec<Vec<Instruction>>,
    metadata: &mut Vec<BlockMetadata>,
) -> [usize; 25] {
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
        ("List", Find),
        ("List", Filter),
        ("List", FlatMap),
        ("List", Count),
        ("Set", Find),
        ("Set", Filter),
        ("Set", FlatMap),
        ("Set", Count),
        ("Set", Any),
        ("Set", FilterIndexed),
        ("Set", FindOrNull),
        ("Set", FindLast),
        ("Set", FindLastOrNull),
        ("Set", FlatMapIndexed),
    ];
    methods.map(|(class, operation)| {
        let class_start = source
            .text
            .find(&format!("external class {class}<"))
            .expect("pinned class");
        let predicate = matches!(
            operation,
            Find | FindOrNull | FindLast | FindLastOrNull | Filter | FilterIndexed | Count | Any
        );
        let delimiter = if predicate { '(' } else { '<' };
        let start = class_start
            + source.text[class_start..]
                .find(&format!(
                    "external function {}{delimiter}",
                    operation.name()
                ))
                .expect("pinned transform");
        let parameter_name = if predicate {
            "predicate: "
        } else {
            "transform: "
        };
        let parameter = start
            + source.text[start..]
                .find(parameter_name)
                .expect("callback parameter")
            + parameter_name.len();
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
        match operation {
            // A successful find leaves the original element on the stack and
            // releases its cursor. A false predicate jumps back to instruction 3.
            Find | FindOrNull | FindLast | FindLastOrNull | Any => code.push(Instruction::Return),
            Count => code.push(Instruction::Jump(3)),
            FlatMap | FlatMapIndexed => {
                code.push(Instruction::CollectionFlatMapNext { end: 3 });
                if class == "Set" {
                    code.extend([Instruction::PrepareSetElement, Instruction::AddSetElement]);
                } else {
                    code.push(Instruction::AppendStrictElement);
                }
                code.push(Instruction::Jump(6));
            }
            _ => {
                match class {
                    "List" => code.push(Instruction::AppendStrictElement),
                    "Set" => {
                        code.extend([Instruction::PrepareSetElement, Instruction::AddSetElement])
                    }
                    "Map" => code.extend([Instruction::PrepareMapEntry, Instruction::AddMapEntry]),
                    _ => unreachable!(),
                }
                code.push(Instruction::Jump(3));
            }
        }
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
) -> [usize; 13] {
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
        ("List", "distinct", Some(Projection::Distinct)),
        ("Listing", "isDistinct", Some(Projection::IsDistinct)),
        ("List", "contains", Some(Projection::Contains)),
    ]
    .map(|(class, name, operation)| {
        let class_start = source
            .text
            .find(&format!("class {class}<"))
            .expect("collection class");
        let signature = if name == "add" {
            "external function add<Other>(element: Other):".to_owned()
        } else if name == "contains" {
            "external function contains(element: Any):".to_owned()
        } else if class == "Map" || matches!(name, "distinct" | "isDistinct") {
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
            Some(Projection::Contains) => vec![
                Instruction::BeginCollectionProjection(Projection::Contains),
                Instruction::CollectionTransformNext { end: 4 },
                Instruction::CollectionContainsStep { matched: 5 },
                Instruction::Jump(1),
            ],
            Some(Projection::Distinct) => vec![
                Instruction::BeginCollectionProjection(Projection::Distinct),
                Instruction::CollectionTransformNext { end: 5 },
                Instruction::CollectionDistinctStep { next: 1, end: 5 },
                Instruction::AppendStrictElement,
                Instruction::Jump(1),
            ],
            Some(Projection::IsDistinct) => vec![
                Instruction::BeginCollectionProjection(Projection::IsDistinct),
                Instruction::CollectionTransformNext { end: 4 },
                Instruction::CollectionDistinctStep { next: 1, end: 4 },
                Instruction::Jump(1),
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
    matched: i64,
    lazy: Option<LazyEvery>,
    inner: Option<FlatMapRun>,
    distinct: Option<DistinctRun>,
}

struct FlatMapRun {
    source: ObjectId,
    members: Vec<CollectionMember>,
    next: usize,
}

struct LazyEvery {
    members: Vec<collections::RenderMember>,
    seen: Vec<CollectionMember>,
    awaiting: Option<CollectionMember>,
}

impl TransformRun {
    pub(super) fn roots(&self, roots: &mut Vec<Value>) {
        roots.push(Value::Object(self.source));
        if let Some(distinct) = &self.distinct {
            roots.extend(distinct.values.iter().cloned());
        }
        roots.extend(self.callback.iter().cloned());
        if let Some(inner) = &self.inner {
            roots.push(Value::Object(inner.source));
        }
        if let Some(lazy) = &self.lazy {
            roots.extend(lazy.members.iter().map(|m| m.descriptor.key.clone()));
            roots.extend(lazy.seen.iter().map(|m| m.key.clone()));
            roots.extend(lazy.awaiting.iter().map(|m| m.key.clone()));
        }
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
            (true, "contains") if self.is_list(&receiver) => 12,
            _ => return Ok(false),
        };
        let id = self.heap.allocate(Object {
            function: Some(Function {
                block: self.program.collection_projection_blocks[index],
                arity: usize::from(matches!(index, 7 | 12)),
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
            (0, "find") => 11,
            (0, "filter") => 12,
            (0, "flatMap") => 13,
            (0, "count") => 14,
            (4, "find") => 11,
            (4, "filter") => 12,
            (4, "flatMap") => 13,
            (4, "count") => 14,
            (4, "any") => 15,
            (4, "filterIndexed") => 16,
            (4, "findOrNull") => 17,
            (4, "findLast") => 18,
            (4, "findLastOrNull") => 19,
            (4, "flatMapIndexed") => 20,
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
        if matches!(
            operation,
            Operation::Projection(Projection::Reverse)
                | Operation::Callback(Transform::FindLast | Transform::FindLastOrNull)
        ) {
            members.reverse();
        }
        // A strict result must not capture the method's source/callback scope.
        let builder = if matches!(
            operation,
            Operation::Callback(
                Transform::Find
                    | Transform::FindOrNull
                    | Transform::FindLast
                    | Transform::FindLastOrNull
                    | Transform::Any
                    | Transform::Count
            ) | Operation::Projection(Projection::IsDistinct | Projection::Contains)
        ) {
            None
        } else {
            Some(self.heap.allocate(Object {
                collection_kind: match operation {
                    Operation::Every => unreachable!("every has no result builder"),
                    Operation::Callback(_) => self.heap.get(source)?.collection_kind,
                    Operation::Projection(Projection::ToSet | Projection::KeysHash) => {
                        CollectionKind::Set
                    }
                    Operation::Projection(_) => CollectionKind::List,
                },
                ..Object::default()
            }))
        };
        let frame = self.frames.last_mut().expect("transform frame");
        if let Some(builder) = builder {
            frame.builders.push(builder);
        }
        frame.collection_transform = Some(Box::new(TransformRun {
            operation,
            source,
            callback,
            members,
            next: 0,
            current: None,
            matched: 0,
            lazy: None,
            inner: None,
            distinct: match operation {
                Operation::Projection(Projection::Distinct | Projection::IsDistinct) => {
                    Some(DistinctRun::default())
                }
                _ => None,
            },
        }));
        if matches!(operation, Operation::Projection(Projection::IsDistinct)) {
            // VmObject.forceAndIterateMemberValues forces all direct values first.
            frame
                .collection_transform
                .as_mut()
                .unwrap()
                .distinct
                .as_mut()
                .unwrap()
                .shallow = true;
            self.force_shallow_frame(Value::Object(source))?;
        }
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
        if self
            .frames
            .last()
            .and_then(|f| f.collection_transform.as_ref())
            .and_then(|r| r.distinct.as_ref())
            .is_some_and(|r| r.shallow)
        {
            self.pop()?;
            self.frames
                .last_mut()
                .unwrap()
                .collection_transform
                .as_mut()
                .unwrap()
                .distinct
                .as_mut()
                .unwrap()
                .shallow = false;
        }
        if self
            .frames
            .last()
            .and_then(|f| f.collection_transform.as_ref())
            .is_some_and(|r| r.lazy.is_some())
        {
            return self.collection_every_lazy_next(end);
        }
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
        if matches!(run.operation, Operation::Every | Operation::Callback(_)) {
            self.stack
                .push(run.callback.clone().expect("transform callback"));
            if map {
                self.stack.push(member.key.clone());
            } else if matches!(run.operation, Operation::Callback(operation) if operation.indexed())
            {
                self.stack.push(Value::Int((run.next as i64).into()));
            }
            run.current = Some((member.key, value.clone()));
        }
        self.stack.push(value);
        run.next += 1;
        Ok(())
    }

    pub(super) fn collection_contains_step(&mut self, matched: usize) -> Result<(), RuntimeError> {
        let element = self
            .stack
            .last()
            .cloned()
            .ok_or_else(|| invalid("contains element"))?;
        let query = self
            .frames
            .last()
            .and_then(|f| f.environment.locals.first())
            .cloned()
            .ok_or_else(|| invalid("contains argument"))?;
        // Pinned Paguro UnmodList.contains calls Objects.equals(element, query).
        // No key hashes are requested; boxed equality preserves numeric kinds.
        // The element stays on the value stack while equality suspends/retries,
        // and the query stays in the ordinary method environment.
        if self.search_key(&element, vec![query.clone()])? {
            self.frames
                .last_mut()
                .unwrap()
                .equality_run
                .as_mut()
                .unwrap()
                .hide_callsite = false;
            return Ok(());
        }
        self.pop()?;
        if equality::same_key(&self.heap, &element, &query) {
            self.frames.last_mut().unwrap().collection_transform = None;
            self.stack.push(Value::Bool(true));
            self.jump(matched)?;
        }
        Ok(())
    }

    pub(super) fn collection_transform_project(&mut self, next: usize) -> Result<(), RuntimeError> {
        let value = self.pop()?;
        let (operation, source, key, input) = {
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
            (operation, run.source, key, input)
        };
        match operation {
            Transform::Find
            | Transform::FindOrNull
            | Transform::FindLast
            | Transform::FindLastOrNull
            | Transform::Any
            | Transform::Filter
            | Transform::FilterIndexed
            | Transform::Count => {
                // ApplyVmFunction1Node.executeBoolean checks the callback's
                // actual result; there is no authored return type-node span.
                let Value::Bool(accepted) = value else {
                    return Err(type_error(
                        &TypeCheck::Boolean,
                        &value,
                        &self.program.module_name,
                        &self.heap,
                    )?);
                };
                if matches!(operation, Transform::Count) {
                    if accepted {
                        self.frames
                            .last_mut()
                            .unwrap()
                            .collection_transform
                            .as_mut()
                            .unwrap()
                            .matched += 1;
                    }
                } else if accepted {
                    if matches!(
                        operation,
                        Transform::Find
                            | Transform::FindOrNull
                            | Transform::FindLast
                            | Transform::FindLastOrNull
                            | Transform::Any
                    ) {
                        self.frames.last_mut().unwrap().collection_transform = None;
                    }
                    self.stack.push(if matches!(operation, Transform::Any) {
                        Value::Bool(true)
                    } else {
                        input
                    });
                } else {
                    self.jump(next)?;
                }
            }
            Transform::FlatMap | Transform::FlatMapIndexed => {
                if !self.is_list(&value) && !self.is_set(&value) {
                    return Err(type_error(
                        &TypeCheck::CollectionBase { arguments: vec![] },
                        &value,
                        &self.program.module_name,
                        &self.heap,
                    )?);
                }
                let source = value.as_object()?;
                let members = collections::ordered_members(&self.heap, source)?;
                self.frames
                    .last_mut()
                    .unwrap()
                    .collection_transform
                    .as_mut()
                    .unwrap()
                    .inner = Some(FlatMapRun {
                    source,
                    members,
                    next: 0,
                });
            }
            Transform::NonNull | Transform::NonNullIndexed if value.is_null() => self.jump(next)?,
            Transform::Keys => self.stack.extend([value, input]),
            Transform::Values => self.stack.extend([key, value]),
            Transform::Map if self.heap.get(source)?.collection_kind == CollectionKind::Map => {
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

    /// Drain a strict callback result in ordinary VM steps. The inner source
    /// remains rooted across yields, and is exhausted before the next callback.
    pub(super) fn collection_flat_map_next(&mut self, end: usize) -> Result<(), RuntimeError> {
        let run = self
            .frames
            .last_mut()
            .and_then(|frame| frame.collection_transform.as_mut())
            .ok_or_else(|| invalid("flatMap cursor"))?;
        let inner = run
            .inner
            .as_mut()
            .ok_or_else(|| invalid("flatMap result"))?;
        let Some(member) = inner.members.get(inner.next).cloned() else {
            run.inner = None;
            return self.jump(end);
        };
        let value = collections::cached_render_value(&self.heap, inner.source, &member)?
            .ok_or_else(|| invalid("uncached strict flatMap element"))?;
        inner.next += 1;
        self.stack.push(value);
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
        if matches!(
            run.operation,
            Operation::Callback(Transform::FindOrNull | Transform::FindLastOrNull)
        ) {
            self.stack.push(Value::Null);
            return Ok(());
        }
        if matches!(run.operation, Operation::Callback(Transform::Any)) {
            self.stack.push(Value::Bool(false));
            return Ok(());
        }
        if matches!(
            run.operation,
            Operation::Callback(Transform::Find | Transform::FindLast)
        ) {
            return Err(RuntimeError::new(
                ErrorKind::Type,
                PklString::join(
                    &[
                        "Cannot find matching element in collection.\nCollection: ".into(),
                        crate::diagnostic_values::render_string(
                            &Value::Object(run.source),
                            &self.heap,
                            &self.program.module_name,
                            80,
                        )?,
                    ],
                    "",
                ),
            ));
        }
        if matches!(run.operation, Operation::Callback(Transform::Count)) {
            self.stack.push(Value::Int(run.matched.into()));
            return Ok(());
        }
        if matches!(run.operation, Operation::Projection(Projection::Contains)) {
            self.stack.push(Value::Bool(false));
            return Ok(());
        }
        if matches!(run.operation, Operation::Projection(Projection::IsDistinct)) {
            self.stack.push(Value::Bool(run.distinct.unwrap().result));
            return Ok(());
        }
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

fn listing_endpoint(name: &str) -> bool {
    matches!(
        name,
        "first" | "firstOrNull" | "last" | "lastOrNull" | "single" | "singleOrNull"
    )
}

pub(super) fn endpoint_blocks(
    blocks: &mut Vec<Vec<Instruction>>,
    metadata: &mut Vec<BlockMetadata>,
) -> BTreeMap<(Arc<str>, Arc<str>), usize> {
    let source = crate::stdlib_sources::base();
    let mut result = BTreeMap::new();
    for class_name in ["List", "Set", "Listing"] {
        let class = source.text.find(&format!("class {class_name}<")).unwrap();
        for name in [
            "first",
            "firstOrNull",
            "last",
            "lastOrNull",
            "single",
            "singleOrNull",
        ] {
            let signature = format!("external {name}:");
            let start = class + source.text[class..].find(&signature).unwrap();
            let end = start + source.text[start..].find('\n').unwrap();
            let block = blocks.len();
            blocks.push(vec![
                Instruction::Primitive(primitives::Operation::CollectionEndpoint {
                    last: name.starts_with("last"),
                    nullable: name.ends_with("OrNull"),
                    single: name.starts_with("single"),
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
        single: bool,
    ) -> Result<(), RuntimeError> {
        let id = receiver.as_object()?;
        if self.heap.get(id)?.collection_kind == CollectionKind::Listing {
            let length = self.heap.get(id)?.length;
            if (single && length != 1) || length == 0 {
                if nullable {
                    self.stack.push(Value::Null);
                    return Ok(());
                }
                return Err(RuntimeError::new(
                    ErrorKind::Type,
                    if single {
                        "Expected a single-element Listing."
                    } else {
                        "Expected a non-empty Listing."
                    },
                ));
            }
            // ListingNodes reads exactly one numeric member. Do not convert to
            // a strict List, render the receiver, or force other element values.
            let index = if last { length - 1 } else { 0 };
            let mut owner = Some(id);
            while let Some(current) = owner {
                let object = self.heap.get(current)?;
                if let Some(member) = object.collection_members.iter().rev().find(|member| {
                    member.kind != CollectionMemberKind::Property
                        && matches!(&member.key, Value::Int(value) if value.get() == index as i64)
                }) {
                    let name = member.name.clone();
                    return self.force_key(id, id, name, false, true, false, true);
                }
                owner = object.parent;
            }
            return Err(invalid("Listing endpoint missing numeric member"));
        }
        let members = collections::ordered_members(&self.heap, id)?;
        if single && members.len() != 1 {
            if nullable {
                self.stack.push(Value::Null);
                return Ok(());
            }
            return Err(RuntimeError::new(
                ErrorKind::Type,
                PklString::join(
                    &[
                        "Expected a single-element collection.\nCollection: ".into(),
                        crate::diagnostic_values::render_string(
                            &receiver,
                            &self.heap,
                            &self.program.module_name,
                            80,
                        )?,
                    ],
                    "",
                ),
            ));
        }
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

/// Predicate calls have no result builder and do not eagerly force lazy values.
pub(super) fn every_blocks(
    blocks: &mut Vec<Vec<Instruction>>,
    metadata: &mut Vec<BlockMetadata>,
) -> [usize; 5] {
    let source = crate::stdlib_sources::base();
    ["List", "Set", "Map", "Listing", "Mapping"].map(|class| {
        let class_start = source
            .text
            .find(&format!("class {class}<"))
            .expect("pinned collection class");
        let start = class_start
            + source.text[class_start..]
                .find("external function every(")
                .expect("pinned every");
        let end = start + source.text[start..].find('\n').unwrap();
        let parameter = start + source.text[start..end].find(": ").unwrap() + 2;
        let type_end = start + source.text[start..end].rfind("): Boolean").unwrap();
        let arity = if matches!(class, "Map" | "Mapping") {
            2
        } else {
            1
        };
        let code = vec![
            Instruction::LoadLocal(0),
            Instruction::CheckType(TypeCheck::Function {
                parameters: Some(vec![TypeCheck::Any; arity]),
                result: Box::new(TypeCheck::Any),
                syntax: true,
            }),
            Instruction::BeginCollectionEvery,
            Instruction::CollectionTransformNext { end: 7 },
            Instruction::Call {
                arity,
                method: false,
            },
            Instruction::CollectionEveryAccept { next: 3 },
            Instruction::Return,
            Instruction::EndCollectionEvery,
            Instruction::Return,
        ];
        let mut spans = vec![None; code.len()];
        spans[1] = Some(Span::new(parameter, type_end));
        let block = blocks.len();
        blocks.push(code);
        metadata.push(BlockMetadata {
            source: Some(source.clone()),
            member: format!("#{class}.every").into(),
            declaration: Some(Span::new(start, end)),
            spans,
            ..Default::default()
        });
        block
    })
}

impl Vm {
    pub(super) fn collection_every_method(
        &mut self,
        receiver: Value,
    ) -> Result<bool, RuntimeError> {
        let Value::Object(source) = receiver else {
            return Ok(false);
        };
        let object = self.heap.get(source)?;
        if object.reference.is_some()
            || object.pair.is_some()
            || object.is_module
            || collections::typed_class(&self.heap, source)?.is_some()
        {
            return Ok(false);
        }
        let index = match object.collection_kind {
            CollectionKind::List => 0,
            CollectionKind::Set => 1,
            CollectionKind::Map => 2,
            CollectionKind::Listing => 3,
            CollectionKind::Mapping => 4,
            _ => return Ok(false),
        };
        let id = self.heap.allocate(Object {
            function: Some(Function {
                block: self.program.collection_every_blocks[index],
                arity: 1,
                method: true,
                captured_this: Some(receiver),
            }),
            ..Default::default()
        });
        self.stack.push(Value::Function(id));
        Ok(true)
    }

    pub(super) fn begin_collection_every(&mut self) -> Result<(), RuntimeError> {
        let callback = self.pop()?;
        let source = self.conversion_receiver()?;
        let lazy = matches!(
            self.heap.get(source)?.collection_kind,
            CollectionKind::Listing | CollectionKind::Mapping
        );
        let (members, lazy) = if lazy {
            (
                Vec::new(),
                Some(LazyEvery {
                    members: collections::render_members(&self.heap, source)?,
                    seen: Vec::new(),
                    awaiting: None,
                }),
            )
        } else {
            (collections::ordered_members(&self.heap, source)?, None)
        };
        self.frames.last_mut().unwrap().collection_transform = Some(Box::new(TransformRun {
            operation: Operation::Every,
            source,
            callback: Some(callback),
            members,
            next: 0,
            current: None,
            matched: 0,
            lazy,
            inner: None,
            distinct: None,
        }));
        Ok(())
    }

    fn collection_every_lazy_next(&mut self, end: usize) -> Result<(), RuntimeError> {
        let run = self
            .frames
            .last()
            .unwrap()
            .collection_transform
            .as_ref()
            .unwrap();
        let source = run.source;
        let lazy = run.lazy.as_ref().unwrap();
        if let Some(member) = lazy.awaiting.clone() {
            let value = self.pop()?;
            self.frames
                .last_mut()
                .unwrap()
                .collection_transform
                .as_mut()
                .unwrap()
                .lazy
                .as_mut()
                .unwrap()
                .awaiting = None;
            return self.collection_every_input(member, value);
        }
        let Some(item) = lazy.members.get(run.next).cloned() else {
            return self.jump(end);
        };
        let member = item.descriptor;
        let candidates = lazy
            .seen
            .iter()
            .filter(|m| {
                (m.kind == CollectionMemberKind::Property)
                    == (member.kind == CollectionMemberKind::Property)
            })
            .map(|m| m.key.clone())
            .collect::<Vec<_>>();
        // These equality continuations rewind their bytecode instruction themselves.
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
        let frame = self.frames.last_mut().unwrap();
        let run = frame.collection_transform.as_mut().unwrap();
        run.next += 1;
        if !seen {
            run.lazy.as_mut().unwrap().seen.push(member.clone());
        }
        // Java marks keys visited before excluding hidden/external declarations.
        if seen || item.skip {
            frame.pc -= 1;
            return Ok(());
        }
        if let Some(value) = collections::cached_render_value(&self.heap, source, &member)? {
            return self.collection_every_input(member, value);
        }
        let frame = self.frames.last_mut().unwrap();
        frame
            .collection_transform
            .as_mut()
            .unwrap()
            .lazy
            .as_mut()
            .unwrap()
            .awaiting = Some(member.clone());
        frame.pc -= 1;
        self.force_key_mode(
            source,
            source,
            member.name,
            false,
            true,
            false,
            member.kind != CollectionMemberKind::Property,
            false,
        )
    }

    fn collection_every_input(
        &mut self,
        member: CollectionMember,
        value: Value,
    ) -> Result<(), RuntimeError> {
        let run = self
            .frames
            .last_mut()
            .unwrap()
            .collection_transform
            .as_mut()
            .unwrap();
        self.stack
            .push(run.callback.clone().expect("every callback"));
        if self.heap.get(run.source)?.collection_kind == CollectionKind::Mapping {
            self.stack.push(member.key.clone());
        }
        run.current = Some((member.key, value.clone()));
        self.stack.push(value);
        Ok(())
    }

    pub(super) fn collection_every_accept(&mut self, next: usize) -> Result<(), RuntimeError> {
        let value = self.pop()?;
        let Value::Bool(accepted) = value else {
            // ApplyVmFunction.executeBoolean has no authored type-node span.
            return Err(type_error(
                &TypeCheck::Boolean,
                &value,
                &self.program.module_name,
                &self.heap,
            )?);
        };
        if accepted {
            self.frames
                .last_mut()
                .unwrap()
                .collection_transform
                .as_mut()
                .unwrap()
                .current = None;
            self.jump(next)
        } else {
            self.frames.last_mut().unwrap().collection_transform = None;
            self.stack.push(Value::Bool(false));
            Ok(())
        }
    }

    pub(super) fn end_collection_every(&mut self) -> Result<(), RuntimeError> {
        self.frames
            .last_mut()
            .unwrap()
            .collection_transform
            .take()
            .ok_or_else(|| invalid("missing every cursor"))?;
        self.stack.push(Value::Bool(true));
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

// List uses the pinned Graal EconomicSet; Listing uses JDK HashSet after a
// complete shallow force. Both compare query.equals(stored), not Pkl numeric ==.
#[derive(Clone, Copy, Default)]
enum DistinctStage {
    #[default]
    Lookup,
    Grow,
    Append,
    Link,
    Rehash {
        slot: usize,
        append_after: bool,
    },
    Complete,
}
// Listing's HashSet remembers each entry's insertion-time hash. The integer
// index has no Pkl callbacks and is never iterated to choose comparison order.
#[derive(Default)]
pub(super) struct InsertionHashIndex {
    known: std::collections::HashMap<i32, Vec<usize>>,
    unknown: Vec<usize>,
}
impl InsertionHashIndex {
    pub(super) fn candidates(&self, hash: Option<i32>, values: &[Value]) -> Vec<Value> {
        let Some(hash) = hash else {
            // Unknown JVM identity hashes retain the conservative full search.
            return values.to_vec();
        };
        let known = self.known.get(&hash).map(Vec::as_slice).unwrap_or(&[]);
        let mut result = Vec::with_capacity(known.len() + self.unknown.len());
        let (mut left, mut right) = (0, 0);
        loop {
            let index = match (known.get(left), self.unknown.get(right)) {
                (Some(&a), Some(&b)) if a < b => {
                    left += 1;
                    a
                }
                (Some(_), Some(&b)) | (None, Some(&b)) => {
                    right += 1;
                    b
                }
                (Some(&a), None) => {
                    left += 1;
                    a
                }
                (None, None) => break,
            };
            result.push(values[index].clone());
        }
        result
    }

    pub(super) fn insert(&mut self, hash: Option<i32>, index: usize) {
        match hash {
            Some(hash) => self.known.entry(hash).or_default().push(index),
            None => self.unknown.push(index),
        }
    }
}
#[derive(Clone, Copy)]
struct ListingDistinctLookup {
    // Capture once before equality. A suspended comparison can reenter hashing
    // and change an object's cache, but HashMap.putVal keeps its hash argument.
    hash: Option<i32>,
}
struct DistinctRun {
    values: Vec<Value>,
    economic: crate::economic_map::EconomicMapIndex,
    listing_index: InsertionHashIndex,
    listing_lookup: Option<ListingDistinctLookup>,
    stage: DistinctStage,
    shallow: bool,
    result: bool,
}
impl Default for DistinctRun {
    fn default() -> Self {
        Self {
            values: Vec::new(),
            economic: Default::default(),
            listing_index: Default::default(),
            listing_lookup: None,
            stage: DistinctStage::Lookup,
            shallow: false,
            result: true,
        }
    }
}
impl Vm {
    // These are ordinary cached properties. In particular, implicit reads in a
    // Listing constraint must take the same path as an explicit `.isDistinct`.
    pub(super) fn collection_cached_property(
        &self,
        receiver: ObjectId,
        name: &str,
    ) -> Result<Option<Member>, RuntimeError> {
        let code = match (self.heap.get(receiver)?.collection_kind, name) {
            (CollectionKind::List, "distinct") => self.program.collection_projection_blocks[10],
            (CollectionKind::Listing, "isDistinct") => {
                self.program.collection_projection_blocks[11]
            }
            (CollectionKind::Listing, name) if listing_endpoint(name) || name == "distinct" => {
                self.program.primitive_methods[&("Listing".into(), name.into())]
            }
            _ => return Ok(None),
        };
        Ok(Some(Member {
            name: name.into(),
            code,
            flags: crate::syntax::ModifierKind::External.flag(),
            entry: false,
            local: false,
            method: false,
            ty: None,
            type_span: None,
            constant: false,
            fixed: false,
        }))
    }

    // VmListingOrMapping.getCachedValue steals an Identifier-valued property
    // only through empty-member wrappers, without applying the element type.
    pub(super) fn steal_collection_property(
        &mut self,
        receiver: ObjectId,
        name: &str,
    ) -> Result<bool, RuntimeError> {
        if (!matches!(name, "isDistinct" | "distinct") && !listing_endpoint(name))
            || self.heap.get(receiver)?.collection_kind != CollectionKind::Listing
        {
            return Ok(false);
        }
        let key = CacheKey {
            name: name.into(),
            owner: None,
            method: false,
            entry: false,
        };
        let mut chain = Vec::new();
        let mut current = receiver;
        loop {
            let object = self.heap.get(current)?;
            if !object.members.is_empty() || !object.collection_members.is_empty() {
                return Ok(false);
            }
            let Some(parent) = object.parent else {
                return Ok(false);
            };
            chain.push(current);
            if let Some(value) = self.cached_slot(parent, &key)? {
                // Tasks run in reverse insertion order, hence deepest first.
                for id in chain {
                    self.complete_cache(id, key.clone(), value.clone())?;
                }
                self.stack.push(value);
                return Ok(true);
            }
            current = parent;
        }
    }

    fn distinct_run_mut(&mut self) -> &mut DistinctRun {
        self.frames
            .last_mut()
            .unwrap()
            .collection_transform
            .as_mut()
            .unwrap()
            .distinct
            .as_mut()
            .unwrap()
    }
    fn distinct_run(&self) -> &DistinctRun {
        self.frames
            .last()
            .unwrap()
            .collection_transform
            .as_ref()
            .unwrap()
            .distinct
            .as_ref()
            .unwrap()
    }
    fn retry_distinct_step(&mut self) {
        self.frames.last_mut().unwrap().pc -= 1;
    }
    pub(super) fn collection_distinct_step(
        &mut self,
        next: usize,
        end: usize,
    ) -> Result<(), RuntimeError> {
        let value = self
            .stack
            .last()
            .cloned()
            .ok_or_else(|| invalid("distinct input"))?;
        let listing = matches!(
            self.frames
                .last()
                .unwrap()
                .collection_transform
                .as_ref()
                .unwrap()
                .operation,
            Operation::Projection(Projection::IsDistinct)
        );
        match self.distinct_run().stage {
            DistinctStage::Lookup => {
                // EconomicMapImpl.findLinear does not request a hash. Keep
                // both preparation and lookup inside the actual hashed path;
                // the fifth distinct insertion starts hashing only after append.
                let hashed = listing || self.distinct_run().economic.has_hash();
                let hash = match (listing, self.distinct_run().listing_lookup) {
                    (true, Some(lookup)) => lookup.hash,
                    _ => {
                        let hash = if hashed {
                            if self.prepare_keys(vec![value.clone()])? {
                                return Ok(());
                            }
                            self.java_hash(&value)?
                        } else {
                            None
                        };
                        if listing {
                            self.distinct_run_mut().listing_lookup =
                                Some(ListingDistinctLookup { hash });
                        }
                        hash
                    }
                };
                let candidates = if listing {
                    // JDK's spread function is bijective, so equal full spread
                    // hashes are exactly equal raw hashes. Same-hash and unknown
                    // entries remain in insertion order; collision-tree ordering
                    // retains the existing bounded audit scope.
                    let run = self.distinct_run();
                    run.listing_index.candidates(hash, &run.values)
                } else {
                    self.distinct_run()
                        .economic
                        .candidates(if hashed {
                            Some(hash.unwrap_or(0))
                        } else {
                            None
                        })
                        .into_iter()
                        .map(|i| self.distinct_run().values[i].clone())
                        .collect()
                };
                if self.search_key(&value, candidates.clone())? {
                    return Ok(());
                }
                if candidates
                    .iter()
                    .any(|old| equality::same_key(&self.heap, &value, old))
                {
                    self.pop()?;
                    if listing {
                        let run = self.distinct_run_mut();
                        run.listing_lookup = None;
                        run.result = false;
                        self.jump(end)?;
                    } else {
                        self.jump(next)?;
                    }
                    return Ok(());
                }
                if listing {
                    let run = self.distinct_run_mut();
                    run.listing_index.insert(hash, run.values.len());
                    run.values.push(value);
                    run.listing_lookup = None;
                    self.pop()?;
                    return Ok(());
                }
                self.distinct_run_mut().stage = DistinctStage::Grow;
            }
            DistinctStage::Grow => {
                let run = self.distinct_run_mut();
                if run.economic.grow_before_append() {
                    run.economic.start_rehash();
                    run.stage = DistinctStage::Rehash {
                        slot: 0,
                        append_after: true,
                    };
                } else {
                    run.stage = DistinctStage::Append;
                }
            }
            DistinctStage::Append => {
                let run = self.distinct_run_mut();
                run.economic.push();
                run.values.push(value);
                if run.economic.has_hash() {
                    run.stage = DistinctStage::Link;
                } else if run.values.len() > 4 {
                    run.economic.start_rehash();
                    run.stage = DistinctStage::Rehash {
                        slot: 0,
                        append_after: false,
                    };
                } else {
                    run.stage = DistinctStage::Complete;
                }
            }
            DistinctStage::Link => {
                if self.prepare_keys(vec![value.clone()])? {
                    return Ok(());
                }
                let hash = self.java_hash(&value)?.unwrap_or(0);
                let run = self.distinct_run_mut();
                if run
                    .economic
                    .put_hash(run.values.len() - 1, hash, run.economic.should_rehash())
                {
                    run.economic.start_rehash();
                    run.stage = DistinctStage::Rehash {
                        slot: 0,
                        append_after: false,
                    };
                } else {
                    run.stage = DistinctStage::Complete;
                }
            }
            DistinctStage::Rehash { slot, append_after } => {
                if let Some(old) = self.distinct_run().values.get(slot).cloned() {
                    if self.prepare_keys(vec![old.clone()])? {
                        return Ok(());
                    }
                    let hash = self.java_hash(&old)?.unwrap_or(0);
                    let run = self.distinct_run_mut();
                    run.economic.put_hash(slot, hash, false);
                    run.stage = DistinctStage::Rehash {
                        slot: slot + 1,
                        append_after,
                    };
                } else {
                    self.distinct_run_mut().stage = if append_after {
                        DistinctStage::Append
                    } else {
                        DistinctStage::Complete
                    };
                }
            }
            DistinctStage::Complete => {
                self.distinct_run_mut().stage = DistinctStage::Lookup;
                return Ok(());
            }
        }
        self.retry_distinct_step();
        Ok(())
    }
}
