//! Arithmetic sequence transforms and collection folds share resumable cursors.
use super::*;
use crate::{
    heap::{CollectionKind, CollectionMemberKind},
    int_seq::IntSeq,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    Step,
    Map,
    Fold,
    FoldIndexed,
    ToList,
    ToListing,
    ToString,
    GetClass,
}

pub(super) fn blocks(
    blocks: &mut Vec<Vec<Instruction>>,
    metadata: &mut Vec<BlockMetadata>,
) -> BTreeMap<(Arc<str>, Arc<str>), usize> {
    let source = crate::stdlib_sources::base();
    let mut result = BTreeMap::new();
    let signature = "external const function IntSeq(start: Int, end: Int): IntSeq";
    let start = source.text.find(signature).expect("IntSeq constructor");
    let mut code = Vec::new();
    let mut spans = Vec::new();
    for (slot, parameter) in [(1, "start: Int"), (0, "end: Int")] {
        let position = start + signature.find(parameter).unwrap() + parameter.len() - 3;
        code.extend([
            Instruction::LoadLocal(slot),
            Instruction::CheckType(TypeCheck::Int),
        ]);
        spans.extend([None, Some(Span::new(position, position + 3))]);
    }
    code.extend([Instruction::MakeIntSeq, Instruction::Return]);
    spans.extend([None, None]);
    let block = blocks.len();
    blocks.push(code);
    metadata.push(BlockMetadata {
        source: Some(source.clone()),
        member: "#IntSeq".into(),
        declaration: Some(Span::new(start, start + signature.len())),
        spans,
        ..Default::default()
    });
    result.insert(("".into(), "IntSeq".into()), block);
    let class = source
        .text
        .find("external class IntSeq extends Any")
        .unwrap();
    for (class_name, name, signature, operation) in [
        (
            "IntSeq",
            "step",
            "external function step(newValue: Int(isNonZero)): IntSeq",
            Operation::Step,
        ),
        (
            "IntSeq",
            "map",
            "external function map<Result>(mapper: (Int) -> Result): List<Result>",
            Operation::Map,
        ),
        (
            "IntSeq",
            "fold",
            "external function fold<Result>(initial: Result, operator: (Result, Int) -> Result): Result",
            Operation::Fold,
        ),
        (
            "IntSeq",
            "toList",
            "external function toList(): List<Int>",
            Operation::ToList,
        ),
        (
            "IntSeq",
            "toListing",
            "external function toListing(): Listing<Int>",
            Operation::ToListing,
        ),
        (
            "IntSeq",
            "toString",
            "external function toString(): String",
            Operation::ToString,
        ),
        (
            "IntSeq",
            "getClass",
            "external function getClass(): Class",
            Operation::GetClass,
        ),
        (
            "List",
            "fold",
            "external function fold<Result>(initial: Result, operator: (Result, Element) -> Result): Result",
            Operation::Fold,
        ),
        (
            "Listing",
            "fold",
            "external function fold<Result>(initial: Result, operator: (Result, Element) -> Result): Result",
            Operation::Fold,
        ),
        (
            "Listing",
            "foldIndexed",
            "external function foldIndexed<Result>(\n    initial: Result,\n    operator: (Int, Result, Element) -> Result,\n  ): Result",
            Operation::FoldIndexed,
        ),
        (
            "Mapping",
            "fold",
            "external function fold<Result>(initial: Result, operator: (Result, Key, Value) -> Result): Result",
            Operation::Fold,
        ),
    ] {
        let class = if matches!(class_name, "List" | "Listing" | "Mapping") {
            source.text.find(&format!("class {class_name}<")).unwrap()
        } else {
            class
        };
        let inherited = matches!(operation, Operation::ToString | Operation::GetClass);
        let start = if inherited {
            source.text.find(signature).unwrap()
        } else {
            class + source.text[class..].find(signature).unwrap()
        };
        let mut code = Vec::new();
        let mut spans = Vec::new();
        if operation == Operation::Step {
            let parameter = start + signature.find("Int(").unwrap();
            code.extend([
                Instruction::LoadLocal(0),
                Instruction::CheckType(TypeCheck::Int),
                Instruction::CheckIntSeqStep,
            ]);
            spans.extend([
                None,
                Some(Span::new(parameter, parameter + 3)),
                Some(Span::new(parameter + 4, parameter + 4 + "isNonZero".len())),
            ]);
        }
        if matches!(
            operation,
            Operation::Map | Operation::Fold | Operation::FoldIndexed
        ) {
            if matches!(operation, Operation::Fold | Operation::FoldIndexed) {
                code.push(Instruction::LoadLocal(1));
                spans.push(None);
            }
            let function_type = if operation == Operation::Map {
                "(Int) -> Result"
            } else if operation == Operation::FoldIndexed {
                "(Int, Result, Element) -> Result"
            } else if class_name == "Mapping" {
                "(Result, Key, Value) -> Result"
            } else if matches!(class_name, "List" | "Listing") {
                "(Result, Element) -> Result"
            } else {
                "(Result, Int) -> Result"
            };
            let parameter = start + signature.find(function_type).unwrap();
            let arity = if operation == Operation::Map {
                1
            } else if class_name == "Mapping" || operation == Operation::FoldIndexed {
                3
            } else {
                2
            };
            let length = function_type.len();
            code.extend([
                Instruction::LoadLocal(0),
                Instruction::CheckType(TypeCheck::Function {
                    parameters: Some(vec![TypeCheck::Any; arity]),
                    result: Box::new(TypeCheck::Any),
                    syntax: true,
                }),
            ]);
            spans.extend([None, Some(Span::new(parameter, parameter + length))]);
        }
        if matches!(
            operation,
            Operation::Map
                | Operation::Fold
                | Operation::FoldIndexed
                | Operation::ToList
                | Operation::ToListing
        ) {
            code.push(Instruction::BeginIntSeq(operation));
            let next = code.len();
            code.push(Instruction::IntSeqNext { end: 0 });
            if matches!(
                operation,
                Operation::Map | Operation::Fold | Operation::FoldIndexed
            ) {
                code.push(Instruction::Call {
                    arity: if operation == Operation::Map {
                        1
                    } else if class_name == "Mapping" || operation == Operation::FoldIndexed {
                        3
                    } else {
                        2
                    },
                    method: false,
                });
            }
            code.extend([Instruction::IntSeqAccept, Instruction::Jump(next)]);
            code[next] = Instruction::IntSeqNext { end: code.len() };
            code.push(Instruction::EndIntSeq);
        } else {
            code.push(Instruction::Primitive(primitives::Operation::IntSeq(
                operation,
            )));
        }
        code.push(Instruction::Return);
        spans.resize(code.len(), None);
        let block = blocks.len();
        blocks.push(code);
        metadata.push(BlockMetadata {
            source: Some(source.clone()),
            member: format!("#{}.{name}", if inherited { "Any" } else { class_name }).into(),
            declaration: Some(Span::new(start, start + signature.len())),
            spans,
            ..Default::default()
        });
        result.insert((class_name.into(), name.into()), block);
    }
    result
}

#[derive(Default)]
struct VisitedKeys {
    values: Vec<Value>,
    index: collection_transforms::InsertionHashIndex,
}

struct MappingCursor {
    source: ObjectId,
    members: Vec<collections::RenderMember>,
    next: usize,
    // Identifier properties and entry keys occupy separate Java namespaces.
    properties: VisitedKeys,
    entries: VisitedKeys,
    // Some(None) is an already computed, unknown JVM identity hash. Preserve
    // the lookup hash while structural equality can suspend or reenter.
    lookup_hash: Option<Option<i32>>,
}

enum Cursor {
    IntSeq(crate::int_seq::Cursor),
    Mapping(Box<MappingCursor>),
    List {
        source: ObjectId,
        members: Vec<crate::heap::CollectionMember>,
        next: usize,
    },
}
pub(super) struct Run {
    operation: Operation,
    cursor: Cursor,
    callback: Option<Value>,
    accumulator: Option<Value>,
    shallow: bool,
}
impl Run {
    pub(super) fn roots(&self, roots: &mut Vec<Value>) {
        roots.extend(self.callback.iter().chain(&self.accumulator).cloned());
        match &self.cursor {
            Cursor::List { source, .. } => roots.push(Value::Object(*source)),
            Cursor::Mapping(cursor) => {
                roots.push(Value::Object(cursor.source));
                roots.extend(cursor.members.iter().map(|m| m.descriptor.key.clone()));
                roots.extend(cursor.properties.values.iter().cloned());
                roots.extend(cursor.entries.values.iter().cloned());
            }
            Cursor::IntSeq(_) => {}
        }
    }
}
impl Vm {
    pub(super) fn int_seq_constructor(&mut self) -> Value {
        Value::Function(self.heap.allocate(Object {
            function: Some(Function {
                block: self.program.primitive_methods[&("".into(), "IntSeq".into())],
                arity: 2,
                method: true,
                captured_this: None,
            }),
            ..Default::default()
        }))
    }
    pub(super) fn make_int_seq(&mut self) -> Result<(), RuntimeError> {
        let Value::Int(end) = self.pop()? else {
            return Err(invalid("checked IntSeq end"));
        };
        let Value::Int(start) = self.pop()? else {
            return Err(invalid("checked IntSeq start"));
        };
        self.stack
            .push(Value::IntSeq(IntSeq::new(start.get(), end.get(), 1)));
        Ok(())
    }
    pub(super) fn check_int_seq_step(&self) -> Result<(), RuntimeError> {
        if matches!(self.stack.last(), Some(Value::Int(value)) if value.get() != 0) {
            return Ok(());
        }
        Err(RuntimeError::new(
            ErrorKind::Type,
            "Type constraint `isNonZero` violated.\nValue: 0",
        ))
    }
    pub(super) fn int_seq_property(&mut self, sequence: &IntSeq, name: &str) -> bool {
        let value = match name {
            "start" => sequence.start(),
            "end" => sequence.end(),
            "step" => sequence.step(),
            _ => return false,
        };
        self.stack.push(Value::Int(value.into()));
        true
    }
    pub(super) fn int_seq_operation(
        &mut self,
        receiver: Value,
        operation: Operation,
    ) -> Result<(), RuntimeError> {
        let Value::IntSeq(sequence) = receiver else {
            return Err(invalid("IntSeq receiver"));
        };
        let value = match operation {
            Operation::Step => {
                let Value::Int(step) = self.pop()? else {
                    return Err(invalid("checked IntSeq step"));
                };
                Value::IntSeq(IntSeq::new(sequence.start(), sequence.end(), step.get()))
            }
            Operation::ToString => Value::String(sequence.to_string().into()),
            Operation::GetClass => self.reference_get_class(&Value::IntSeq(sequence))?,
            _ => return Err(invalid("IntSeq loop primitive")),
        };
        self.stack.push(value);
        Ok(())
    }
    pub(super) fn begin_int_seq(&mut self, operation: Operation) -> Result<(), RuntimeError> {
        let callback = if matches!(
            operation,
            Operation::Map | Operation::Fold | Operation::FoldIndexed
        ) {
            Some(self.pop()?)
        } else {
            None
        };
        let accumulator = if matches!(operation, Operation::Fold | Operation::FoldIndexed) {
            Some(self.pop()?)
        } else {
            None
        };
        let receiver = self
            .frames
            .last()
            .and_then(|frame| frame.environment.this_value.clone())
            .ok_or_else(|| invalid("sequence loop receiver"))?;
        let cursor = match receiver {
            Value::IntSeq(sequence) => Cursor::IntSeq(sequence.cursor(false)),
            Value::Object(source)
                if matches!(operation, Operation::Fold | Operation::FoldIndexed)
                    && matches!(
                        self.heap.get(source)?.collection_kind,
                        CollectionKind::List | CollectionKind::Listing
                    ) =>
            {
                Cursor::List {
                    source,
                    members: collections::ordered_members(&self.heap, source)?,
                    next: 0,
                }
            }
            Value::Object(source)
                if matches!(operation, Operation::Fold | Operation::FoldIndexed)
                    && self.heap.get(source)?.collection_kind == CollectionKind::Mapping =>
            {
                Cursor::Mapping(Box::new(MappingCursor {
                    source,
                    members: collections::render_members(&self.heap, source)?,
                    next: 0,
                    properties: VisitedKeys::default(),
                    entries: VisitedKeys::default(),
                    lookup_hash: None,
                }))
            }
            _ => return Err(invalid("sequence loop receiver")),
        };
        let shallow_source = match &cursor {
            Cursor::Mapping(cursor) => Some(cursor.source),
            Cursor::List { source, .. }
                if self.heap.get(*source)?.collection_kind == CollectionKind::Listing =>
            {
                Some(*source)
            }
            _ => None,
        };
        let frame = self.frames.last_mut().expect("sequence frame");
        frame.int_seq_run = Some(Box::new(Run {
            operation,
            cursor,
            callback,
            accumulator,
            shallow: shallow_source.is_some(),
        }));
        if !matches!(operation, Operation::Fold | Operation::FoldIndexed) {
            let builder = self.heap.allocate(Object {
                collection_kind: if operation == Operation::ToListing {
                    CollectionKind::Listing
                } else {
                    CollectionKind::List
                },
                ..Default::default()
            });
            frame.builders.push(builder);
        }
        if let Some(source) = shallow_source {
            // Java validates the callback first, then forces all receiver member
            // values before calling it. Nested member objects remain lazy.
            self.force_shallow_frame(Value::Object(source))?;
        }
        Ok(())
    }
    pub(super) fn int_seq_next(&mut self, end: usize) -> Result<(), RuntimeError> {
        if self
            .frames
            .last()
            .and_then(|frame| frame.int_seq_run.as_ref())
            .is_some_and(|run| run.shallow)
        {
            self.pop()?;
            self.frames
                .last_mut()
                .unwrap()
                .int_seq_run
                .as_mut()
                .unwrap()
                .shallow = false;
        }
        if self
            .frames
            .last()
            .and_then(|f| f.int_seq_run.as_ref())
            .is_some_and(|run| matches!(&run.cursor, Cursor::Mapping(_)))
        {
            return self.mapping_fold_next(end);
        }
        let run = self
            .frames
            .last_mut()
            .and_then(|frame| frame.int_seq_run.as_mut())
            .ok_or_else(|| invalid("IntSeq cursor"))?;
        let value = match &mut run.cursor {
            Cursor::Mapping(_) => unreachable!("Mapping fold uses its member cursor"),
            Cursor::IntSeq(cursor) => cursor.next().map(|(_, value)| Value::Int(value.into())),
            Cursor::List {
                source,
                members,
                next,
            } => {
                let value = if let Some(member) = members.get(*next) {
                    Some(
                        collections::cached_render_value(&self.heap, *source, member)?
                            .ok_or_else(|| invalid("uncached strict fold element"))?,
                    )
                } else {
                    None
                };
                *next += 1;
                value
            }
        };
        let Some(value) = value else {
            return self.jump(end);
        };
        if let Some(callback) = &run.callback {
            self.stack.push(callback.clone());
        }
        if run.operation == Operation::FoldIndexed {
            let Cursor::List { next, .. } = &run.cursor else {
                return Err(invalid("indexed fold cursor"));
            };
            self.stack.push(Value::Int((*next as i64 - 1).into()));
        }
        if let Some(accumulator) = &run.accumulator {
            self.stack.push(accumulator.clone());
        }
        self.stack.push(value);
        Ok(())
    }
    fn mapping_fold_next(&mut self, end: usize) -> Result<(), RuntimeError> {
        let run = self
            .frames
            .last()
            .and_then(|f| f.int_seq_run.as_ref())
            .ok_or_else(|| invalid("Mapping fold cursor"))?;
        let Cursor::Mapping(cursor) = &run.cursor else {
            return Err(invalid("Mapping fold receiver"));
        };
        let Some(item) = cursor.members.get(cursor.next).cloned() else {
            return self.jump(end);
        };
        let source = cursor.source;
        let key = item.descriptor.key.clone();
        let property = item.descriptor.kind == CollectionMemberKind::Property;
        let hash = if let Some(hash) = cursor.lookup_hash {
            hash
        } else {
            if self.prepare_keys(vec![key.clone()])? {
                return Ok(());
            }
            let hash = self.java_hash(&key)?;
            let run = self
                .frames
                .last_mut()
                .unwrap()
                .int_seq_run
                .as_mut()
                .unwrap();
            let Cursor::Mapping(cursor) = &mut run.cursor else {
                unreachable!()
            };
            cursor.lookup_hash = Some(hash);
            hash
        };
        let run = self.frames.last().unwrap().int_seq_run.as_ref().unwrap();
        let Cursor::Mapping(cursor) = &run.cursor else {
            unreachable!()
        };
        let seen = if property {
            &cursor.properties
        } else {
            &cursor.entries
        };
        let candidates = seen.index.candidates(hash, &seen.values);
        if self.search_key(&key, candidates.clone())? {
            return Ok(());
        }
        let duplicate = candidates
            .iter()
            .any(|old| equality::same_key(&self.heap, &key, old));
        let run = self
            .frames
            .last_mut()
            .unwrap()
            .int_seq_run
            .as_mut()
            .unwrap();
        let Cursor::Mapping(cursor) = &mut run.cursor else {
            unreachable!()
        };
        let seen = if property {
            &mut cursor.properties
        } else {
            &mut cursor.entries
        };
        if !duplicate {
            seen.index.insert(hash, seen.values.len());
            seen.values.push(key.clone());
        }
        cursor.next += 1;
        cursor.lookup_hash = None;
        // Java visits keys before filtering hidden/external properties. Do not
        // pre-deduplicate descriptors or skip an object's key hashing effects.
        if duplicate || item.skip {
            self.frames.last_mut().unwrap().pc -= 1;
            return Ok(());
        }
        let value = collections::cached_render_value(&self.heap, source, &item.descriptor)?
            .ok_or_else(|| invalid("uncached shallow Mapping fold value"))?;
        let run = self.frames.last().unwrap().int_seq_run.as_ref().unwrap();
        self.stack
            .push(run.callback.clone().expect("Mapping fold callback"));
        self.stack
            .push(run.accumulator.clone().expect("Mapping fold accumulator"));
        self.stack.push(key);
        self.stack.push(value);
        Ok(())
    }

    pub(super) fn int_seq_accept(&mut self) -> Result<(), RuntimeError> {
        let value = self.pop()?;
        let run = self
            .frames
            .last_mut()
            .and_then(|frame| frame.int_seq_run.as_mut())
            .ok_or_else(|| invalid("IntSeq result"))?;
        if matches!(run.operation, Operation::Fold | Operation::FoldIndexed) {
            run.accumulator = Some(value);
        } else {
            self.stack.push(value);
            self.append_strict_element()?;
        }
        Ok(())
    }
    pub(super) fn end_int_seq(&mut self) -> Result<(), RuntimeError> {
        let frame = self.frames.last_mut().expect("IntSeq end");
        let run = frame
            .int_seq_run
            .take()
            .ok_or_else(|| invalid("IntSeq end cursor"))?;
        let value = if let Some(accumulator) = run.accumulator {
            accumulator
        } else {
            let builder = frame
                .builders
                .pop()
                .ok_or_else(|| invalid("IntSeq result builder"))?;
            Value::Object(if run.operation == Operation::ToListing {
                builder
            } else {
                self.finish_strict_collection(builder)?
            })
        };
        self.stack.push(value);
        Ok(())
    }
}

impl Vm {
    pub(super) fn int_seq_member_error(&self, name: &str, method: bool) -> RuntimeError {
        let properties = ["start", "end", "step"].map(str::to_owned);
        let methods = [
            ("step", "step(newValue)", 1),
            ("fold", "fold(initial, operator)", 2),
            ("map", "map(mapper)", 1),
            ("toList", "toList()", 0),
            ("toListing", "toListing()", 0),
            ("getClass", "getClass()", 0),
            ("toString", "toString()", 0),
            ("ifNonNull", "ifNonNull(transform)", 1),
        ]
        .map(|(name, signature, arity)| (name.to_owned(), signature.to_owned(), arity));
        let arity = if method {
            self.frames
                .last()
                .and_then(|frame| {
                    self.program.metadata.get(frame.block).and_then(|metadata| {
                        metadata.method_arities.get(&frame.pc.saturating_sub(1))
                    })
                })
                .copied()
                .unwrap_or(0) as isize
        } else {
            -1
        };
        let mut error = RuntimeError::new(
            ErrorKind::MissingMember,
            if method {
                format!("Cannot find method `{name}` in class `IntSeq`.")
            } else {
                format!("Cannot find property `{name}` in object of type `IntSeq`.")
            },
        );
        error.hint = crate::diagnostic::member_suggestions(name, arity, &properties, &methods)
            .or_else(|| {
                let mut names = if method {
                    methods
                        .iter()
                        .map(|(name, _, _)| name.as_str())
                        .collect::<Vec<_>>()
                } else {
                    properties.iter().map(String::as_str).collect()
                };
                names.sort();
                Some(
                    format!(
                        "{}\n{}",
                        if method {
                            "Available methods in class `IntSeq`:"
                        } else {
                            "Available properties:"
                        },
                        names.join("\n")
                    )
                    .into(),
                )
            });
        error
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn listing_conversion_keeps_values_without_producer_environments() {
        let program = Arc::new(Program::new(vec![
            vec![
                Instruction::Constant(Value::IntSeq(IntSeq::new(1, 3, 1))),
                Instruction::GetMethod("toListing".into()),
                Instruction::Call {
                    arity: 0,
                    method: true,
                },
                Instruction::Return,
            ],
            vec![Instruction::Constant(Value::Null), Instruction::Return],
        ]));
        let mut vm = Vm::new(program);
        vm.start(0, vec![]).unwrap();
        let value = loop {
            vm.collect(&[]);
            match vm.poll(1) {
                VmPoll::Yielded => {}
                VmPoll::Ready(value) => break value.unwrap(),
                event => panic!("{event:?}"),
            }
        };
        let id = value.as_object().unwrap();
        assert!(vm.heap.get(id).unwrap().member_environments.is_empty());
        vm.start(1, vec![]).unwrap();
        while matches!(vm.poll(1), VmPoll::Yielded) {}
        vm.collect(std::slice::from_ref(&value));
        assert_eq!(vm.heap.get(id).unwrap().length, 3);
        vm.collect(&[]);
        assert!(vm.heap.get(id).is_err());
        vm.close();
        assert_eq!(vm.heap_stats().slots, 0);
    }
}
