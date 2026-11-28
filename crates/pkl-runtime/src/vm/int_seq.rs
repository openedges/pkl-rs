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

#[cfg_attr(target_arch = "wasm32", inline(never))]
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
            Instruction::CheckType(Box::new(TypeCheck::Int)),
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
            class + source.text.entire().range(class..).find(signature).unwrap()
        };
        let mut code = Vec::new();
        let mut spans = Vec::new();
        if operation == Operation::Step {
            let parameter = start + signature.find("Int(").unwrap();
            code.extend([
                Instruction::LoadLocal(0),
                Instruction::CheckType(Box::new(TypeCheck::Int)),
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
                Instruction::CheckType(Box::new(TypeCheck::Function {
                    parameters: Some(vec![TypeCheck::Any; arity]),
                    result: Box::new(TypeCheck::Any),
                    syntax: true,
                })),
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
    StrictList {
        source: ObjectId,
        len: usize,
        next: usize,
    },
    List {
        source: ObjectId,
        members: Vec<crate::heap::CollectionMember>,
        next: usize,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FastIntState {
    Unchecked,
    RetryOnce,
    Done,
}
pub(super) struct Run {
    operation: Operation,
    cursor: Cursor,
    callback: Option<Value>,
    accumulator: Option<Value>,
    shallow: bool,
    fast_int_state: FastIntState,
}
impl Run {
    pub(super) fn roots(&self, roots: &mut Vec<Value>) {
        roots.extend(self.callback.iter().chain(&self.accumulator).cloned());
        match &self.cursor {
            Cursor::List { source, .. } | Cursor::StrictList { source, .. } => {
                roots.push(Value::Object(*source))
            }
            Cursor::Mapping(cursor) => {
                roots.push(Value::Object(cursor.source));
                roots.extend(cursor.members.iter().map(|m| m.descriptor.key.clone()));
                roots.extend(cursor.properties.values.iter().cloned());
                roots.extend(cursor.entries.values.iter().cloned());
            }
            Cursor::IntSeq(_) => {}
        }
    }

    #[inline]
    pub(super) fn try_fast_next(
        &mut self,
        heap: &Heap,
        stack: &mut identity::OperandStack,
    ) -> Option<bool> {
        if self.shallow || matches!(self.cursor, Cursor::Mapping(_)) {
            return None;
        }
        let value = match &mut self.cursor {
            Cursor::IntSeq(cursor) => cursor.next().map(|(_, value)| Value::Int(value.into())),
            Cursor::StrictList { source, len, next } => {
                let value = if *next < *len {
                    let obj = heap.get(*source).ok()?;
                    obj.list_elements.as_ref()?.get(*next).cloned()
                } else {
                    None
                };
                *next += 1;
                value
            }
            _ => return None,
        };
        let Some(value) = value else {
            return Some(false);
        };
        if let Some(callback) = &self.callback {
            stack.push(callback.clone());
        }
        if self.operation == Operation::FoldIndexed {
            let Cursor::StrictList { next, .. } = &self.cursor else {
                return None;
            };
            stack.push(Value::Int((*next as i64 - 1).into()));
        }
        if let Some(accumulator) = &self.accumulator {
            stack.push(accumulator.clone());
        }
        stack.push(value);
        Some(true)
    }

    #[inline]
    pub(super) fn try_fast_accept(
        &mut self,
        heap: &mut Heap,
        builder: Option<ObjectId>,
        stack: &mut identity::OperandStack,
    ) -> bool {
        if matches!(self.operation, Operation::Fold | Operation::FoldIndexed) {
            if let Some(value) = stack.pop() {
                self.accumulator = Some(value);
                return true;
            }
            return false;
        }
        if let Some(target) = builder
            && let Ok(object) = heap.get_mut(target)
            && let Some(elems) = &mut object.list_elements
            && let Some(value) = stack.pop()
        {
            let extra = match &value {
                Value::String(s) => s.as_str().map_or(16, str::len),
                _ => 0,
            };
            elems.push(value);
            object.length += 1;
            heap.note_external_allocation(std::mem::size_of::<Value>().saturating_add(extra));
            return true;
        }
        false
    }

    /// In-place accept + next for `poll_fast_run` lambda frame reuse:
    /// Accepts `result`, advances the cursor, and returns:
    /// - `Some(Some((arg0, opt_arg1)))` if there is a next iteration (for arity 1 or 2)
    /// - `Some(None)` if the cursor is exhausted
    /// - `None` if this run is not eligible for in-place reuse
    #[cfg_attr(not(target_arch = "wasm32"), inline)]
    #[cfg_attr(target_arch = "wasm32", inline(never))]
    pub(super) fn try_inplace_accept_and_next(
        &mut self,
        heap: &mut Heap,
        builder: Option<ObjectId>,
        result: Value,
    ) -> Option<Option<(Value, Option<Value>)>> {
        if self.shallow
            || matches!(self.cursor, Cursor::Mapping(_) | Cursor::List { .. })
            || !matches!(self.operation, Operation::Map | Operation::Fold)
        {
            return None;
        }
        // Accept previous result
        if self.operation == Operation::Fold {
            self.accumulator = Some(result);
        } else {
            let target = builder?;
            let extra = match &result {
                Value::String(s) => s.as_str().map_or(16, str::len),
                _ => 0,
            };
            let object = heap.get_mut(target).ok()?;
            let elems = object.list_elements.as_mut()?;
            elems.push(result);
            object.length += 1;
            heap.note_external_allocation(std::mem::size_of::<Value>().saturating_add(extra));
        }
        // Fetch next element
        let next_elem = match &mut self.cursor {
            Cursor::IntSeq(cursor) => cursor.next().map(|(_, v)| Value::Int(v.into())),
            Cursor::StrictList { source, len, next } => {
                if *next < *len {
                    let obj = heap.get(*source).ok()?;
                    let v = obj.list_elements.as_ref()?.get(*next).cloned()?;
                    *next += 1;
                    Some(v)
                } else {
                    *next += 1;
                    None
                }
            }
            _ => return None,
        };
        let Some(elem) = next_elem else {
            return Some(None);
        };
        if self.operation == Operation::Fold {
            let acc = self.accumulator.clone()?;
            Some(Some((acc, Some(elem))))
        } else {
            Some(Some((elem, None)))
        }
    }

    #[inline]
    pub(super) fn fast_int_callback(&self) -> Option<ObjectId> {
        if self.fast_int_state == FastIntState::Done {
            return None;
        }
        match self.callback {
            Some(Value::Function(id)) => Some(id),
            _ => None,
        }
    }

    #[inline]
    pub(super) fn note_fast_int_unverified(&mut self) {
        self.fast_int_state = match self.fast_int_state {
            FastIntState::Unchecked => FastIntState::RetryOnce,
            FastIntState::RetryOnce | FastIntState::Done => FastIntState::Done,
        };
    }

    #[inline]
    pub(super) fn mark_fast_int_done(&mut self) {
        self.fast_int_state = FastIntState::Done;
    }

    /// Executes an entire `IntSeq.map` or `IntSeq`/`StrictList.fold` directly against a verified
    /// pure-`i64` `FastIntBlock`, respecting `max_steps`.
    /// Returns `Ok(steps_consumed)` if the entire loop completed (cursor exhausted and builder/accumulator updated),
    /// or `Err(steps_consumed)` if ineligible or bailed out mid-way so the caller still bills consumed steps.
    #[allow(clippy::too_many_arguments)]
    #[cfg_attr(target_arch = "wasm32", inline(never))]
    pub(super) fn try_fast_int_bulk(
        &mut self,
        heap: &mut Heap,
        builder: Option<ObjectId>,
        fast_block: &fast_int::FastIntBlock,
        captured: &[i64],
        max_steps: usize,
        val_stack: &mut Vec<i64>,
        locals_buf: &mut Vec<i64>,
        call_stack: &mut Vec<(u32, u32)>,
    ) -> Result<usize, usize> {
        if self.shallow || !fast_block.returns_int {
            return Err(0);
        }
        let mut total_steps = 0usize;
        match (&mut self.cursor, self.operation) {
            (Cursor::IntSeq(cursor), Operation::Map) => {
                let Some(target) = builder else {
                    return Err(0);
                };
                let mut probe = cursor.clone();
                let mut results = Vec::new();
                while let Some((_, v)) = probe.next() {
                    let rem = max_steps.saturating_sub(total_steps);
                    if rem == 0 {
                        return Err(total_steps);
                    }
                    let (out, steps) = match fast_block.execute(
                        captured,
                        &[v],
                        rem,
                        val_stack,
                        locals_buf,
                        call_stack,
                    ) {
                        Ok(res) => res,
                        Err(steps) => return Err(total_steps.saturating_add(steps)),
                    };
                    total_steps = total_steps.saturating_add(steps);
                    if results.len() == results.capacity() && results.try_reserve(1).is_err() {
                        return Err(total_steps);
                    }
                    results.push(Value::Int(crate::value_identity::Number::from(out).boxed()));
                }
                let Ok(object) = heap.get_mut(target) else {
                    return Err(total_steps);
                };
                let Some(elems) = object.list_elements.as_mut() else {
                    return Err(total_steps);
                };
                let added = results.len();
                if elems.try_reserve(added).is_err() {
                    return Err(total_steps);
                }
                elems.extend(results);
                object.length += added;
                *cursor = probe;
                heap.note_external_allocation(added.saturating_mul(std::mem::size_of::<Value>()));
                Ok(total_steps.max(1))
            }
            (Cursor::IntSeq(cursor), Operation::Fold) => {
                let Some(Value::Int(init_acc)) = self.accumulator.as_ref() else {
                    return Err(0);
                };
                let mut acc = init_acc.get();
                let mut probe = cursor.clone();
                while let Some((_, v)) = probe.next() {
                    let rem = max_steps.saturating_sub(total_steps);
                    if rem == 0 {
                        return Err(total_steps);
                    }
                    let (out, steps) = match fast_block.execute(
                        captured,
                        &[acc, v],
                        rem,
                        val_stack,
                        locals_buf,
                        call_stack,
                    ) {
                        Ok(res) => res,
                        Err(steps) => return Err(total_steps.saturating_add(steps)),
                    };
                    total_steps = total_steps.saturating_add(steps);
                    acc = out;
                }
                *cursor = probe;
                self.accumulator =
                    Some(Value::Int(crate::value_identity::Number::from(acc).boxed()));
                Ok(total_steps.max(1))
            }
            (Cursor::StrictList { source, len, next }, Operation::Fold) => {
                let Some(Value::Int(init_acc)) = self.accumulator.as_ref() else {
                    return Err(0);
                };
                let mut acc = init_acc.get();
                let Ok(obj) = heap.get(*source) else {
                    return Err(0);
                };
                let Some(elems) = obj.list_elements.as_ref() else {
                    return Err(0);
                };
                let end = (*len).min(elems.len());
                if let Some(op_code) = fast_block.try_fold_add_sub_mul(captured) {
                    let count = end.saturating_sub(*next);
                    let needed = count.saturating_mul(4);
                    if needed > max_steps {
                        return Err(0);
                    }
                    let mut scanned = 0usize;
                    match op_code {
                        0 => {
                            for elem in &elems[*next..end] {
                                scanned += 4;
                                let Value::Int(v) = elem else {
                                    return Err(scanned);
                                };
                                let Some(next_acc) = acc.checked_add(v.get()) else {
                                    return Err(scanned);
                                };
                                acc = next_acc;
                            }
                        }
                        1 => {
                            for elem in &elems[*next..end] {
                                scanned += 4;
                                let Value::Int(v) = elem else {
                                    return Err(scanned);
                                };
                                let Some(next_acc) = acc.checked_sub(v.get()) else {
                                    return Err(scanned);
                                };
                                acc = next_acc;
                            }
                        }
                        2 => {
                            for elem in &elems[*next..end] {
                                scanned += 4;
                                let Value::Int(v) = elem else {
                                    return Err(scanned);
                                };
                                let Some(next_acc) = acc.checked_mul(v.get()) else {
                                    return Err(scanned);
                                };
                                acc = next_acc;
                            }
                        }
                        _ => unreachable!(),
                    }
                    *next = end;
                    self.accumulator =
                        Some(Value::Int(crate::value_identity::Number::from(acc).boxed()));
                    return Ok(needed.max(1));
                }
                for elem in &elems[*next..end] {
                    let Value::Int(v) = elem else {
                        return Err(total_steps);
                    };
                    let rem = max_steps.saturating_sub(total_steps);
                    if rem == 0 {
                        return Err(total_steps);
                    }
                    let (out, steps) = match fast_block.execute(
                        captured,
                        &[acc, v.get()],
                        rem,
                        val_stack,
                        locals_buf,
                        call_stack,
                    ) {
                        Ok(res) => res,
                        Err(steps) => return Err(total_steps.saturating_add(steps)),
                    };
                    total_steps = total_steps.saturating_add(steps);
                    acc = out;
                }
                *next = end;
                self.accumulator =
                    Some(Value::Int(crate::value_identity::Number::from(acc).boxed()));
                Ok(total_steps.max(1))
            }
            _ => Err(0),
        }
    }
}
impl Vm {
    pub(super) fn int_seq_constructor(&mut self) -> Value {
        Value::Function(self.heap.allocate(Object {
            function: Some(Function {
                block: self.program.primitive_methods.expect("", "IntSeq"),
                arity: 2,
                method: true,
                captured_this: None,
            }),
            ..Default::default()
        }))
    }
    #[cfg_attr(target_arch = "wasm32", inline(never))]
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
    #[cfg_attr(target_arch = "wasm32", inline(never))]
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
    #[cfg_attr(target_arch = "wasm32", inline(never))]
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
    #[cfg_attr(target_arch = "wasm32", inline(never))]
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
        let mut list_capacity = 0usize;
        let cursor = match receiver {
            Value::IntSeq(sequence) => {
                list_capacity = sequence.len_hint();
                Cursor::IntSeq(sequence.cursor(false))
            }
            Value::Object(source)
                if matches!(operation, Operation::Fold | Operation::FoldIndexed)
                    && matches!(
                        self.heap.get(source)?.collection_kind,
                        CollectionKind::List | CollectionKind::Listing
                    ) =>
            {
                let obj = self.heap.get(source)?;
                if let Some(elems) = &obj.list_elements {
                    Cursor::StrictList {
                        source,
                        len: elems.len(),
                        next: 0,
                    }
                } else {
                    Cursor::List {
                        source,
                        members: collections::ordered_members(&self.heap, source)?,
                        next: 0,
                    }
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
            fast_int_state: FastIntState::Unchecked,
        }));
        if !matches!(operation, Operation::Fold | Operation::FoldIndexed) {
            let is_listing = operation == Operation::ToListing;
            let initial_cap = list_capacity.min(4096);
            let list_elements = if is_listing {
                None
            } else {
                let mut v = Vec::new();
                if initial_cap > 0 && v.try_reserve(initial_cap).is_ok() {
                    self.heap.note_external_allocation(
                        initial_cap.saturating_mul(std::mem::size_of::<Value>()),
                    );
                }
                Some(v)
            };
            let builder = self.heap.allocate(Object {
                collection_kind: if is_listing {
                    CollectionKind::Listing
                } else {
                    CollectionKind::List
                },
                list_elements,
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
    #[cfg_attr(target_arch = "wasm32", inline(never))]
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
            Cursor::StrictList { source, len, next } => {
                let value = if *next < *len {
                    self.heap
                        .get(*source)?
                        .list_elements
                        .as_ref()
                        .and_then(|elems| elems.get(*next))
                        .cloned()
                } else {
                    None
                };
                *next += 1;
                value
            }
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
            let (Cursor::List { next, .. } | Cursor::StrictList { next, .. }) = &run.cursor else {
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

    #[cfg_attr(target_arch = "wasm32", inline(never))]
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
    #[cfg_attr(target_arch = "wasm32", inline(never))]
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
    pub(super) fn int_seq_member_error(
        &self,
        name: impl Into<PklString>,
        method: bool,
    ) -> RuntimeError {
        let name = name.into();
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
                PklString::from("Cannot find method `")
                    .concat(&name)
                    .concat(&"` in class `IntSeq`.".into())
            } else {
                PklString::from("Cannot find property `")
                    .concat(&name)
                    .concat(&"` in object of type `IntSeq`.".into())
            },
        );
        error.hint = crate::diagnostic::member_suggestions(&name, arity, &properties, &methods)
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
