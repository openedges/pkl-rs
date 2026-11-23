//! Set sequence operations preserve separate membership and visible-order paths.
use super::*;
use crate::heap::{CollectionKind, CollectionMember, CollectionMemberKind};
#[derive(Clone, Copy, Debug)]
pub enum Kind {
    FilterNonNull,
    FilterIsInstance,
    GroupBy,
    Flatten,
    StartsWith,
    EndsWith,
    Split,
    SplitOrNull,
    Partition,
    Take,
    TakeWhile,
    TakeLast,
    TakeLastWhile,
    Drop,
    DropWhile,
    DropLast,
    DropLastWhile,
    Repeat,
    Zip,
    Intersect,
    Difference,
}
impl Kind {
    fn name(self) -> &'static str {
        match self {
            Self::FilterNonNull => "filterNonNull",
            Self::FilterIsInstance => "filterIsInstance",
            Self::GroupBy => "groupBy",
            Self::Flatten => "flatten",
            Self::StartsWith => "startsWith",
            Self::EndsWith => "endsWith",
            Self::Split => "split",
            Self::SplitOrNull => "splitOrNull",
            Self::Partition => "partition",
            Self::Take => "take",
            Self::TakeWhile => "takeWhile",
            Self::TakeLast => "takeLast",
            Self::TakeLastWhile => "takeLastWhile",
            Self::Drop => "drop",
            Self::DropWhile => "dropWhile",
            Self::DropLast => "dropLast",
            Self::DropLastWhile => "dropLastWhile",
            Self::Repeat => "repeat",
            Self::Zip => "zip",
            Self::Intersect => "intersect",
            Self::Difference => "difference",
        }
    }
    fn predicate(self) -> bool {
        matches!(
            self,
            Self::Partition
                | Self::TakeWhile
                | Self::TakeLastWhile
                | Self::DropWhile
                | Self::DropLastWhile
        )
    }
    fn split(self) -> bool {
        matches!(self, Self::Split | Self::SplitOrNull)
    }
}
#[derive(Clone, Copy, Debug)]
pub enum Operation {
    Start(Kind),
    Step,
    AppendVisible,
    AfterInsert,
}
pub(super) fn method_arity(name: &str) -> Option<usize> {
    Some(match name {
        "filterNonNull" | "flatten" => 0,
        "groupBy" | "filterIsInstance" | "startsWith" | "endsWith" | "split" | "splitOrNull"
        | "partition" | "take" | "takeWhile" | "takeLast" | "takeLastWhile" | "drop"
        | "dropWhile" | "dropLast" | "dropLastWhile" | "repeat" | "zip" | "intersect"
        | "difference" => 1,
        _ => return None,
    })
}
#[cfg_attr(target_arch = "wasm32", inline(never))]
pub(super) fn blocks(
    blocks: &mut Vec<Vec<Instruction>>,
    metadata: &mut Vec<BlockMetadata>,
) -> BTreeMap<(Arc<str>, Arc<str>), usize> {
    use Kind::*;
    let source = crate::stdlib_sources::base();
    let mut result = BTreeMap::new();
    for class_name in ["Set", "List"] {
        let class = source
            .text
            .find(&format!("external class {class_name}<"))
            .unwrap();
        for kind in [
            FilterNonNull,
            FilterIsInstance,
            GroupBy,
            Flatten,
            StartsWith,
            EndsWith,
            Split,
            SplitOrNull,
            Partition,
            Take,
            TakeWhile,
            TakeLast,
            TakeLastWhile,
            Drop,
            DropWhile,
            DropLast,
            DropLastWhile,
            Repeat,
            Zip,
            Intersect,
            Difference,
        ] {
            if class_name == "List" && matches!(kind, Intersect | Difference) {
                continue;
            }
            let name = kind.name();
            let start = class
                + source
                    .text
                    .entire()
                    .range(class..)
                    .find(&format!("external function {name}"))
                    .unwrap();
            let end = start + source.text.entire().range(start..).find("\n").unwrap();
            let declaration = &source.text.entire().range(start..end);
            let mut code = vec![];
            let mut spans = vec![];
            if !matches!(kind, FilterNonNull | Flatten) {
                let (label, ty) = if matches!(kind, FilterIsInstance) {
                    ("clazz: ", TypeCheck::MetaClass("Class".into()))
                } else if kind.predicate() || matches!(kind, GroupBy) {
                    (
                        if matches!(kind, GroupBy) {
                            "selector: "
                        } else {
                            "predicate: "
                        },
                        TypeCheck::Function {
                            parameters: Some(vec![TypeCheck::Any]),
                            result: Box::new(TypeCheck::Any),
                            syntax: true,
                        },
                    )
                } else if matches!(kind, StartsWith | EndsWith | Zip) {
                    ("coll: ", TypeCheck::CollectionBase { arguments: vec![] })
                } else if matches!(kind, Intersect | Difference) {
                    ("other: ", TypeCheck::Collection(CollectionKind::Set))
                } else {
                    (if kind.split() { "index: " } else { "n: " }, TypeCheck::Int)
                };
                let p = start + declaration.find(label).unwrap() + label.len();
                let type_end = if kind.predicate() || matches!(kind, GroupBy) {
                    let arrow = p + source.text.entire().range(p..end).find(" -> ").unwrap();
                    arrow
                        + 4
                        + source
                            .text
                            .entire()
                            .range(arrow + 4..end)
                            .find(")")
                            .unwrap()
                } else if matches!(
                    kind,
                    FilterIsInstance | StartsWith | EndsWith | Zip | Intersect | Difference
                ) {
                    p + source.text.entire().range(p..end).find(")").unwrap()
                } else {
                    p + 3
                };
                code.extend([
                    Instruction::LoadLocal(0),
                    Instruction::CheckType(Box::new(ty)),
                ]);
                spans.extend([None, Some(Span::new(p, type_end))]);
            }
            code.push(Instruction::Primitive(primitives::Operation::SetSequence(
                Operation::Start(kind),
            )));
            spans.push(None);
            let step = code.len();
            code.extend([
                Instruction::Primitive(primitives::Operation::SetSequence(Operation::Step)),
                Instruction::Jump(step),
                Instruction::Return,
                Instruction::PrepareSetElement,
                Instruction::AddSetElement,
                Instruction::Primitive(primitives::Operation::SetSequence(Operation::AfterInsert)),
                Instruction::Jump(step),
                Instruction::PutSetKey,
                Instruction::Primitive(primitives::Operation::SetSequence(
                    Operation::AppendVisible,
                )),
                Instruction::Primitive(primitives::Operation::SetSequence(Operation::AfterInsert)),
                Instruction::Jump(step),
                Instruction::AppendStrictElement,
                Instruction::Primitive(primitives::Operation::SetSequence(Operation::AfterInsert)),
                Instruction::Jump(step),
            ]);
            spans.resize(code.len(), None);
            let block = blocks.len();
            blocks.push(code);
            metadata.push(BlockMetadata {
                source: Some(source.clone()),
                member: format!("#{class_name}.{name}").into(),
                declaration: Some(Span::new(start, end)),
                spans,
                ..Default::default()
            });
            result.insert((class_name.into(), name.into()), block);
        }
    }
    result
}
#[derive(Clone, Copy)]
enum Stage {
    FlattenMap,
    FlattenForced,
    FlattenInner,
    GroupKey,
    GroupGet,
    GroupSetContains,
    GroupMapContains,
    GroupAssociate,
    Instance,
    Scan,
    Predicate,
    Emit,
}
#[derive(Clone)]
pub(super) struct Run {
    kind: Kind,
    source: ObjectId,
    collection_kind: CollectionKind,
    values: Vec<Value>,
    other: Option<ObjectId>,
    other_values: Vec<Value>,
    argument: Value,
    builders: Vec<ObjectId>,
    cursor: usize,
    matched: usize,
    split: usize,
    selection: Vec<Value>,
    current: Value,
    stage: Stage,
    step: usize,
    #[allow(dead_code)]
    cycle: i32,
    repeats: i64,
    group_key: Value,
    group_value: Value,
    append_order: bool,
    inner: Option<ObjectId>,
    inner_values: Vec<Value>,
    inner_cursor: usize,
    int_cursor: Option<crate::int_seq::Cursor>,
}
impl Run {
    pub(super) fn roots(&self, roots: &mut Vec<Value>) {
        roots.push(Value::Object(self.source));
        roots.extend(self.other.map(Value::Object));
        roots.extend(self.builders.iter().copied().map(Value::Object));
        roots.extend(self.values.iter().cloned());
        roots.extend(self.other_values.iter().cloned());
        roots.extend(self.selection.iter().cloned());
        roots.extend(self.inner.map(Value::Object));
        roots.extend(self.inner_values.iter().cloned());
        roots.extend([
            self.argument.clone(),
            self.current.clone(),
            self.group_key.clone(),
            self.group_value.clone(),
        ]);
    }
}
fn values(vm: &Vm, id: ObjectId) -> Result<Vec<Value>, RuntimeError> {
    let object = vm.heap.get(id)?;
    if let Some(elems) = &object.list_elements {
        return Ok(elems.clone());
    }
    collections::ordered_members(&vm.heap, id)?
        .iter()
        .map(|m| {
            collections::cached_render_value(&vm.heap, id, m)?
                .ok_or_else(|| invalid("strict Set sequence value"))
        })
        .collect()
}
impl Vm {
    fn finish_set_sequence(&mut self, run: &Run) -> Result<Value, RuntimeError> {
        let first = Value::Object(self.finish_strict_collection(run.builders[0])?);
        if run.builders.len() == 2 {
            let second = Value::Object(self.finish_strict_collection(run.builders[1])?);
            Ok(self.make_pair(first, second))
        } else {
            Ok(first)
        }
    }
    fn set_sequence_return(&mut self, step: usize, value: Value) -> Result<(), RuntimeError> {
        self.stack.push(value);
        self.jump(step + 2)
    }
    fn set_sequence_insert(
        &mut self,
        mut run: Run,
        value: Value,
        side: usize,
        direct: bool,
    ) -> Result<(), RuntimeError> {
        let builder = run.builders[side];
        let step = run.step;
        let list = self.heap.get(builder)?.collection_kind == CollectionKind::List;
        run.current = value.clone();
        self.frames.last_mut().unwrap().set_sequence = Some(Box::new(run));
        self.frames.last_mut().unwrap().builders.push(builder);
        self.stack.push(value);
        self.jump(
            step + if list {
                11
            } else if direct {
                7
            } else {
                3
            },
        )
    }
    pub(super) fn set_sequence(
        &mut self,
        receiver: Value,
        operation: Operation,
    ) -> Result<(), RuntimeError> {
        use Kind::*;
        if matches!(operation, Operation::AfterInsert) {
            self.frames
                .last_mut()
                .unwrap()
                .builders
                .pop()
                .ok_or_else(|| invalid("Set insertion builder"))?;
            let run = self
                .frames
                .last_mut()
                .unwrap()
                .set_sequence
                .as_mut()
                .unwrap();
            if matches!(run.kind, GroupBy) {
                run.builders.pop();
                run.stage = Stage::GroupMapContains;
            }
            return Ok(());
        }
        if matches!(operation, Operation::AppendVisible) {
            let value = self
                .frames
                .last()
                .unwrap()
                .set_sequence
                .as_ref()
                .unwrap()
                .current
                .clone();
            self.stack.push(value);
            return self.append_strict_element();
        }
        if let Operation::Start(kind) = operation {
            let argument = if matches!(kind, FilterNonNull | Flatten) {
                Value::Null
            } else {
                self.pop()?
            };
            let source = receiver.as_object()?;
            let collection_kind = self.heap.get(source)?.collection_kind;
            let mut input = values(self, source)?;
            let step = self.frames.last().unwrap().pc;
            let mut other = None;
            let mut other_values = vec![];
            if matches!(kind, StartsWith | EndsWith | Zip | Intersect | Difference) {
                let id = argument.as_object()?;
                other = Some(id);
                other_values = values(self, id)?;
            }
            if matches!(kind, Intersect) && input.len() > other_values.len() {
                std::mem::swap(&mut input, &mut other_values);
                other = Some(source);
            }
            if matches!(kind, StartsWith | EndsWith) && input.len() < other_values.len() {
                return self.set_sequence_return(step, Value::Bool(false));
            }
            let mut selection = vec![];
            let mut stage = Stage::Scan;
            let mut split = 0;
            let mut repeats = 0;
            if matches!(
                kind,
                Take | TakeLast | Drop | DropLast | Split | SplitOrNull | Repeat
            ) {
                let Value::Int(n) = argument else {
                    return Err(invalid("checked Set integer"));
                };
                let n = n.get();
                let len = input.len() as i64;
                if kind.split() {
                    if n < 0 || n > len {
                        if matches!(kind, SplitOrNull) {
                            return self.set_sequence_return(step, Value::Null);
                        }
                        let rendered = crate::diagnostic_values::render_string(
                            &receiver,
                            &self.heap,
                            &self.program.module_name,
                            80,
                        )?;
                        return Err(RuntimeError::with_program_values(
                            ErrorKind::Arithmetic,
                            format!(
                                "Element index `{}` is out of range `0`..`{}`.",
                                strings::diagnostic_integer(n),
                                strings::diagnostic_integer(len)
                            ),
                            &[("Collection", rendered)],
                        ));
                    }
                    split = n as usize;
                    if collection_kind == CollectionKind::List {
                        let left = self.build_strict_list(input[..split].to_vec());
                        let right = self.build_strict_list(input[split..].to_vec());
                        let pair = self.make_pair(Value::Object(left), Value::Object(right));
                        return self.set_sequence_return(step, pair);
                    }
                    selection = input.clone();
                    stage = Stage::Emit;
                } else if matches!(kind, Repeat) {
                    if n < 0 {
                        return Err(RuntimeError::new(
                            ErrorKind::Arithmetic,
                            format!(
                                "Expected a positive number, but got `{}`.",
                                strings::diagnostic_integer(n)
                            ),
                        ));
                    }
                    if n == 0 || input.is_empty() {
                        let value =
                            Value::Object(self.empty_strict_collection(CollectionKind::List));
                        return self.set_sequence_return(step, value);
                    }
                    if n == 1 && collection_kind == CollectionKind::List {
                        return self.set_sequence_return(step, receiver);
                    }
                    repeats = n;
                } else {
                    if n == 0 {
                        let value = if matches!(kind, Drop | DropLast) {
                            receiver
                        } else {
                            Value::Object(self.empty_strict_collection(collection_kind))
                        };
                        return self.set_sequence_return(step, value);
                    }
                    if n >= len {
                        let value = if matches!(kind, Take | TakeLast) {
                            receiver
                        } else {
                            Value::Object(self.empty_strict_collection(collection_kind))
                        };
                        return self.set_sequence_return(step, value);
                    }
                    if n < 0 {
                        return Err(RuntimeError::new(
                            ErrorKind::Arithmetic,
                            format!(
                                "Expected a positive number, but got `{}`.",
                                strings::diagnostic_integer(n)
                            ),
                        ));
                    }
                    let n = n as usize;
                    selection = match kind {
                        Take => input[..n].to_vec(),
                        TakeLast => input[input.len() - n..].to_vec(),
                        Drop => input[n..].to_vec(),
                        DropLast => input[..input.len() - n].to_vec(),
                        _ => unreachable!(),
                    };
                    if collection_kind == CollectionKind::List {
                        let built = self.build_strict_list(selection);
                        return self.set_sequence_return(step, Value::Object(built));
                    }
                    stage = Stage::Emit;
                }
            }
            let count = if kind.split() || matches!(kind, Partition) {
                2
            } else {
                1
            };
            let builders = (0..count)
                .map(|_| {
                    self.heap.allocate(Object {
                        collection_kind: if matches!(kind, Repeat) {
                            CollectionKind::List
                        } else if matches!(kind, GroupBy) {
                            CollectionKind::Map
                        } else {
                            collection_kind
                        },
                        ..Default::default()
                    })
                })
                .collect();
            self.frames.last_mut().unwrap().set_sequence = Some(Box::new(Run {
                kind,
                source,
                collection_kind,
                values: input,
                other,
                other_values,
                argument,
                builders,
                cursor: 0,
                matched: 0,
                split,
                selection,
                current: Value::Null,
                stage,
                step,
                cycle: 0,
                repeats,
                group_key: Value::Null,
                group_value: Value::Null,
                append_order: false,
                inner: None,
                inner_values: Vec::new(),
                inner_cursor: 0,
                int_cursor: None,
            }));
            return Ok(());
        }
        let mut run = *self
            .frames
            .last_mut()
            .unwrap()
            .set_sequence
            .take()
            .ok_or_else(|| invalid("Set sequence cursor"))?;
        if matches!(run.stage, Stage::FlattenMap) {
            if let Some(key) = run.inner_values.get(run.inner_cursor).cloned() {
                let source = run.inner.unwrap();
                self.frames.last_mut().unwrap().set_sequence = Some(Box::new(run));
                if self.prepare_map_lookup(source, &key)? {
                    return Ok(());
                }
                run = *self.frames.last_mut().unwrap().set_sequence.take().unwrap();
                let value = self.map_value(source, &key)?.ok_or_else(||RuntimeError::new(ErrorKind::Unsupported,"Map iterator lost its key; pinned Java Map.entry null/internal-error protocol remains unresolved."))?;
                run.inner_cursor += 1;
                let entry = Value::Object(self.heap.allocate(Object {
                    foreign_map_entry: Some(Box::new([key, value])),
                    ..Default::default()
                }));
                return self.set_sequence_insert(run, entry, 0, false);
            }
            run.inner = None;
            run.inner_values.clear();
            run.stage = Stage::Scan;
        }
        if matches!(run.stage, Stage::FlattenForced) {
            self.pop()?;
            let source = run.inner.unwrap();
            let mut seen = Vec::new();
            for member in collections::render_members(&self.heap, source)? {
                if seen
                    .iter()
                    .any(|key| equality::same_key(&self.heap, key, &member.descriptor.key))
                {
                    continue;
                }
                seen.push(member.descriptor.key.clone());
                if member.skip {
                    continue;
                }
                run.inner_values.push(
                    collections::cached_render_value(&self.heap, source, &member.descriptor)?
                        .ok_or_else(|| invalid("forced Listing value"))?,
                );
            }
            run.stage = Stage::FlattenInner;
        }
        if matches!(run.stage, Stage::FlattenInner) {
            let value = if let Some(cursor) = &mut run.int_cursor {
                cursor.next().map(|(_, value)| Value::Int(value.into()))
            } else {
                let value = run.inner_values.get(run.inner_cursor).cloned();
                run.inner_cursor += usize::from(value.is_some());
                value
            };
            if let Some(value) = value {
                return self.set_sequence_insert(run, value, 0, false);
            }
            run.inner = None;
            run.inner_values.clear();
            run.int_cursor = None;
            run.stage = Stage::Scan;
        }
        if matches!(run.stage, Stage::GroupKey) {
            run.group_key = self.pop()?;
            run.stage = Stage::GroupGet;
        }
        if matches!(
            run.stage,
            Stage::GroupGet
                | Stage::GroupMapContains
                | Stage::GroupAssociate
                | Stage::GroupSetContains
        ) {
            let stage = run.stage;
            let map = run.builders[0];
            let key = run.group_key.clone();
            let input = run.current.clone();
            let group = run.group_value.clone();
            self.frames.last_mut().unwrap().set_sequence = Some(Box::new(run));
            let pending = match stage {
                Stage::GroupGet | Stage::GroupMapContains => self.prepare_map_lookup(map, &key)?,
                Stage::GroupAssociate => self.prepare_map_association(map, &key)?,
                Stage::GroupSetContains => self.prepare_set_lookup(group.as_object()?, &input)?,
                _ => unreachable!(),
            };
            if pending {
                return Ok(());
            }
            run = *self.frames.last_mut().unwrap().set_sequence.take().unwrap();
            match stage {
                Stage::GroupGet => {
                    if run.collection_kind == CollectionKind::List {
                        let builder = if let Some(group) = self.map_value(map, &key)? {
                            let source = self.heap.get(group.as_object()?)?;
                            self.heap.allocate(Object {
                                collection_kind: CollectionKind::List,
                                length: source.length,
                                list_elements: source.list_elements.clone(),
                                members: source.members.clone(),
                                collection_members: source.collection_members.clone(),
                                member_values: source.member_values.clone(),
                                cache_map: source.cache_map.clone(),
                                cache_slots: source.cache_slots.clone(),
                                cache_index: source.cache_index.clone(),
                                ..Default::default()
                            })
                        } else {
                            self.heap.allocate(Object {
                                collection_kind: CollectionKind::List,
                                ..Default::default()
                            })
                        };
                        run.group_value = Value::Object(builder);
                        run.builders.push(builder);
                        return self.set_sequence_insert(run, input, 1, false);
                    }
                    if let Some(group) = self.map_value(map, &key)? {
                        run.group_value = group;
                        run.stage = Stage::GroupSetContains;
                    } else {
                        let builder = self.heap.allocate(Object {
                            collection_kind: CollectionKind::Set,
                            ..Default::default()
                        });
                        run.group_value = Value::Object(builder);
                        run.builders.push(builder);
                        return self.set_sequence_insert(run, input, 1, true);
                    }
                }
                Stage::GroupSetContains => {
                    let group = group.as_object()?;
                    if self.set_contains(group, &input)? {
                        run.stage = Stage::GroupMapContains;
                    } else {
                        let source = self.heap.get(group)?;
                        let builder = self.heap.allocate(Object {
                            collection_kind: CollectionKind::Set,
                            length: source.length,
                            members: source.members.clone(),
                            collection_members: source.collection_members.clone(),
                            member_values: source.member_values.clone(),
                            cache_map: source.cache_map.clone(),
                            cache_slots: source.cache_slots.clone(),
                            cache_index: source.cache_index.clone(),
                            map_index: source.map_index.clone(),
                            set_entries: source.set_entries.clone(),
                            ..Default::default()
                        });
                        run.group_value = Value::Object(builder);
                        run.builders.push(builder);
                        return self.set_sequence_insert(run, input, 1, true);
                    }
                }
                Stage::GroupMapContains => {
                    run.append_order = self.map_value(map, &key)?.is_none();
                    run.stage = Stage::GroupAssociate;
                }
                Stage::GroupAssociate => {
                    self.insert_map_entry_ordered(
                        map,
                        key,
                        run.group_value.clone(),
                        Some(run.append_order),
                    )?;
                    run.stage = Stage::Scan;
                }
                _ => unreachable!(),
            }
            self.frames.last_mut().unwrap().set_sequence = Some(Box::new(run));
            return Ok(());
        }
        if matches!(run.stage, Stage::Instance) {
            let Value::Bool(accepted) = self.pop()? else {
                return Err(invalid("instance test result"));
            };
            run.stage = Stage::Scan;
            if accepted {
                let value = run.current.clone();
                return self.set_sequence_insert(run, value, 0, false);
            }
        }
        if matches!(run.stage, Stage::Predicate) {
            let result = self.pop()?;
            let Value::Bool(accepted) = result else {
                return Err(type_error(
                    &TypeCheck::Boolean,
                    &result,
                    &self.program.module_name,
                    &self.heap,
                )?);
            };
            run.stage = Stage::Scan;
            if matches!(run.kind, Partition) {
                let value = run.current.clone();
                return self.set_sequence_insert(run, value, usize::from(!accepted), false);
            }
            if matches!(run.kind, TakeWhile) {
                if accepted {
                    let value = run.current.clone();
                    return self.set_sequence_insert(run, value, 0, false);
                }
                let value = self.finish_set_sequence(&run)?;
                return self.set_sequence_return(run.step, value);
            }
            if accepted {
                run.matched += 1;
            } else {
                run.selection = match run.kind {
                    DropWhile => run.values[run.matched..].to_vec(),
                    TakeLastWhile => run.values[run.values.len() - run.matched..].to_vec(),
                    DropLastWhile => run.values[..run.values.len() - run.matched].to_vec(),
                    _ => unreachable!(),
                };
                let identity = matches!(run.kind, DropWhile | DropLastWhile) && run.matched == 0;
                if identity {
                    return self.set_sequence_return(run.step, Value::Object(run.source));
                }
                run.cursor = 0;
                run.stage = Stage::Emit;
            }
        }
        if matches!(run.stage, Stage::Emit) {
            if run.cursor == run.selection.len() {
                let value = self.finish_set_sequence(&run)?;
                return self.set_sequence_return(run.step, value);
            }
            let side = usize::from(run.kind.split() && run.cursor >= run.split);
            let value = run.selection[run.cursor].clone();
            run.cursor += 1;
            return self.set_sequence_insert(run, value, side, true);
        }
        if matches!(run.kind, StartsWith | EndsWith) {
            if run.cursor == run.other_values.len() {
                return self.set_sequence_return(run.step, Value::Bool(true));
            }
            let (left, right) = if matches!(run.kind, EndsWith) {
                (
                    run.values[run.values.len() - 1 - run.cursor].clone(),
                    run.other_values[run.other_values.len() - 1 - run.cursor].clone(),
                )
            } else {
                (
                    run.values[run.cursor].clone(),
                    run.other_values[run.cursor].clone(),
                )
            };
            self.frames.last_mut().unwrap().set_sequence = Some(Box::new(run));
            if self.search_key(&left, vec![right.clone()])? {
                return Ok(());
            }
            run = *self.frames.last_mut().unwrap().set_sequence.take().unwrap();
            if !equality::same_key(&self.heap, &left, &right) {
                return self.set_sequence_return(run.step, Value::Bool(false));
            }
            run.cursor += 1;
        } else if matches!(run.kind, Repeat) {
            let builder = run.builders[0];
            if run.values.is_empty() || run.repeats <= 0 {
                let object = self.heap.get_mut(builder)?;
                object.length = 0;
                object.repeat_base_len = Some(run.values.len());
                let value = self.finish_set_sequence(&run)?;
                return self.set_sequence_return(run.step, value);
            }
            let total = (run.values.len() as u64).saturating_mul(run.repeats as u64);
            let total_count = total as usize;
            let count_to_populate = if run.repeats == 0 {
                0
            } else if total_count <= 65_536 {
                total_count
            } else {
                run.values.len()
            };
            let object = self.heap.get_mut(builder)?;
            object.length = total_count;
            object.repeat_base_len = Some(run.values.len());
            object.collection_members.reserve(count_to_populate);
            object.members.reserve(count_to_populate);
            object.cache_slots.reserve(count_to_populate);
            let m_len = run.values.len().max(1);
            for i in 0..count_to_populate {
                let val = run.values[i % m_len].clone();
                let name = PklString::from(format!("\0{}:{}", builder.index, i));
                let entry_key = Value::Int((i as i64).into());
                let descriptor = CollectionMember {
                    key: entry_key.clone(),
                    name: name.clone(),
                    kind: CollectionMemberKind::Element,
                };
                let member = Member {
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
                };
                let cache_key = CacheKey {
                    owner: None,
                    name,
                    method: false,
                    entry: true,
                };
                object.collection_members.push(descriptor);
                object.members.push(member);
                object.member_values.insert(cache_key.clone(), val.clone());
                object.push_completed_slot(cache_key, Some(entry_key), val);
            }
            let value = self.finish_set_sequence(&run)?;
            return self.set_sequence_return(run.step, value);
        } else if run.cursor == run.values.len()
            || (matches!(run.kind, Zip) && run.cursor == run.other_values.len())
        {
            let value = if matches!(run.kind, TakeWhile | TakeLastWhile) {
                Value::Object(run.source)
            } else if matches!(run.kind, DropWhile | DropLastWhile) {
                Value::Object(self.empty_strict_collection(run.collection_kind))
            } else {
                self.finish_set_sequence(&run)?
            };
            return self.set_sequence_return(run.step, value);
        } else {
            let reverse = matches!(run.kind, TakeLastWhile | DropLastWhile);
            let value = run.values[if reverse {
                run.values.len() - 1 - run.cursor
            } else {
                run.cursor
            }]
            .clone();
            if matches!(run.kind, Intersect | Difference) {
                let other = run.other.unwrap();
                self.frames.last_mut().unwrap().set_sequence = Some(Box::new(run));
                if self.prepare_set_lookup(other, &value)? {
                    return Ok(());
                }
                run = *self.frames.last_mut().unwrap().set_sequence.take().unwrap();
                let present = self.set_contains(other, &value)?;
                run.cursor += 1;
                if present == matches!(run.kind, Intersect) {
                    return self.set_sequence_insert(run, value, 0, false);
                }
            } else if matches!(run.kind, Flatten) {
                run.cursor += 1;
                run.inner_cursor = 0;
                if let Value::IntSeq(sequence) = &value {
                    run.int_cursor = Some(sequence.cursor(false));
                    run.stage = Stage::FlattenInner;
                } else if let Value::Object(id) = value {
                    match self.heap.get(id)?.collection_kind {
                        CollectionKind::List | CollectionKind::Set => {
                            run.inner = Some(id);
                            run.inner_values = values(self, id)?;
                            run.stage = Stage::FlattenInner;
                        }
                        CollectionKind::Listing => {
                            run.inner = Some(id);
                            run.stage = Stage::FlattenForced;
                            self.frames.last_mut().unwrap().set_sequence = Some(Box::new(run));
                            return self.force_shallow_frame(Value::Object(id));
                        }
                        CollectionKind::Map => {
                            run.inner = Some(id);
                            run.inner_values = collections::ordered_members(&self.heap, id)?
                                .into_iter()
                                .map(|member| member.key)
                                .collect();
                            run.stage = Stage::FlattenMap;
                        }
                        _ => {
                            return Err(flatten_error(
                                &Value::Object(id),
                                &self.heap,
                                &self.program.module_name,
                            )?);
                        }
                    }
                } else {
                    return Err(flatten_error(
                        &value,
                        &self.heap,
                        &self.program.module_name,
                    )?);
                }
            } else if matches!(run.kind, GroupBy) {
                let Value::Function(callback) = run.argument else {
                    return Err(invalid("checked group selector"));
                };
                run.current = value.clone();
                run.cursor += 1;
                run.stage = Stage::GroupKey;
                self.frames.last_mut().unwrap().set_sequence = Some(Box::new(run));
                return self.invoke_function(callback, vec![value], false);
            } else if matches!(run.kind, FilterIsInstance) {
                if matches!(&value, Value::Object(id) if self.heap.get(*id)?.foreign_map_entry.is_some())
                {
                    return Err(RuntimeError::new(
                        ErrorKind::Unsupported,
                        "Foreign Map.Entry IsInstanceOfNode dispatch has a pinned Java internal-error protocol that remains unresolved.",
                    ));
                }
                let Value::Class(class) = run.argument else {
                    return Err(invalid("checked filter class"));
                };
                let prototype = self.heap.get(self.heap.get_class(class)?.prototype)?;
                let class = prototype
                    .class_definition
                    .as_ref()
                    .ok_or_else(|| invalid("filter class metadata"))?;
                let ty = prototype
                    .builtin_type
                    .as_deref()
                    .cloned()
                    .unwrap_or_else(|| TypeCheck::Class {
                        module: class.module.clone(),
                        id: class.id,
                        name: class.name.clone(),
                    });
                run.current = value.clone();
                run.cursor += 1;
                run.stage = Stage::Instance;
                self.frames.last_mut().unwrap().set_sequence = Some(Box::new(run));
                return self.begin_type(ty, value, TypeDestination::Test, Vec::new());
            } else if run.kind.predicate() {
                let Value::Function(callback) = run.argument else {
                    return Err(invalid("checked Set predicate"));
                };
                run.current = value.clone();
                run.cursor += 1;
                run.stage = Stage::Predicate;
                self.frames.last_mut().unwrap().set_sequence = Some(Box::new(run));
                return self.invoke_function(callback, vec![value], false);
            } else if matches!(run.kind, Zip) {
                let other = run.other_values[run.cursor].clone();
                run.cursor += 1;
                let pair = self.make_pair(value, other);
                return self.set_sequence_insert(run, pair, 0, false);
            } else if matches!(run.kind, FilterNonNull) {
                run.cursor += 1;
                if !value.is_null() {
                    return self.set_sequence_insert(run, value, 0, false);
                }
            }
        }
        self.frames.last_mut().unwrap().set_sequence = Some(Box::new(run));
        Ok(())
    }
}

fn flatten_error(
    value: &Value,
    heap: &Heap,
    module: &PklString,
) -> Result<RuntimeError, RuntimeError> {
    let rendered = crate::diagnostic_values::render_string(value, heap, module, 80)?;
    Ok(RuntimeError::with_program_values(
        ErrorKind::Type,
        "Cannot flatten a collection containing a non-collection element.",
        &[("Element", rendered)],
    ))
}
