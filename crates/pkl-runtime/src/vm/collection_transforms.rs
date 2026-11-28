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
    DistinctBy,
    IsDistinctBy,
    Fold,
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
    ListIsDistinct,
    Contains,
    MapContainsValue,
    Entries,
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
            Self::DistinctBy => "distinctBy",
            Self::IsDistinctBy => "isDistinctBy",
            Self::Fold => "fold",
        }
    }

    fn indexed(self) -> bool {
        matches!(
            self,
            Self::Indexed | Self::NonNullIndexed | Self::FilterIndexed | Self::FlatMapIndexed
        )
    }
}

#[cfg_attr(target_arch = "wasm32", inline(never))]
pub(super) fn blocks(
    blocks: &mut Vec<Vec<Instruction>>,
    metadata: &mut Vec<BlockMetadata>,
) -> [usize; 37] {
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
        ("List", Any),
        ("List", FilterIndexed),
        ("List", FindOrNull),
        ("List", FindLast),
        ("List", FindLastOrNull),
        ("List", FlatMapIndexed),
        ("List", DistinctBy),
        ("List", IsDistinctBy),
        ("Map", Any),
        ("Map", Filter),
        ("Map", FlatMap),
        ("Map", Fold),
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
        let selector = matches!(operation, DistinctBy | IsDistinctBy);
        let delimiter = if predicate || selector { '(' } else { '<' };
        let start = class_start
            + source
                .text
                .entire()
                .range(class_start..)
                .find(&format!(
                    "external function {}{delimiter}",
                    operation.name()
                ))
                .expect("pinned transform");
        let parameter_name = if predicate {
            "predicate: "
        } else if selector {
            "selector: "
        } else if matches!(operation, Fold) {
            "operator: "
        } else {
            "transform: "
        };
        let parameter = start
            + source
                .text
                .entire()
                .range(start..)
                .find(parameter_name)
                .expect("callback parameter")
            + parameter_name.len();
        // The function type includes the argument tuple and the result type;
        // delimit it at the method parameter's closing delimiter, not the tuple.
        let arrow = parameter
            + source
                .text
                .entire()
                .range(parameter..)
                .find(" -> ")
                .expect("function arrow");
        let result_end = arrow
            + 4
            + source
                .text
                .entire()
                .range(arrow + 4..)
                .as_str()
                .expect("pinned builtin source is scalar")
                .find([')', '\n'])
                .expect("function result end");
        let type_end = source
            .text
            .entire()
            .range(arrow + 4..result_end)
            .as_str()
            .expect("pinned builtin source is scalar")
            .trim_end_matches(',')
            .len()
            + arrow
            + 4;
        let closing = arrow
            + source
                .text
                .entire()
                .range(arrow..)
                .find("):")
                .expect("method return type");
        let declaration_end = closing
            + source
                .text
                .entire()
                .range(closing..)
                .find("\n")
                .unwrap_or(0);
        let arity = if matches!(operation, Fold) {
            3
        } else if class == "Map" || operation.indexed() {
            2
        } else {
            1
        };
        let mut code = vec![
            Instruction::LoadLocal(0),
            Instruction::CheckType(Box::new(TypeCheck::Function {
                parameters: Some(vec![TypeCheck::Any; arity]),
                result: Box::new(TypeCheck::Any),
                syntax: true,
            })),
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
            Count | Fold => code.push(Instruction::Jump(3)),
            DistinctBy | IsDistinctBy => {
                code.push(Instruction::CollectionDistinctStep { next: 3, end: 0 });
                if matches!(operation, DistinctBy) {
                    code.push(Instruction::AppendStrictElement);
                }
                code.push(Instruction::Jump(3));
            }
            FlatMap | FlatMapIndexed => {
                code.push(Instruction::CollectionFlatMapNext { end: 3 });
                if class == "Set" {
                    code.extend([Instruction::PrepareSetElement, Instruction::AddSetElement]);
                } else if class == "Map" {
                    code.extend([Instruction::PrepareMapEntry, Instruction::AddMapEntry]);
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
        if selector {
            code[6] = Instruction::CollectionDistinctStep { next: 3, end };
        }
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

#[cfg_attr(target_arch = "wasm32", inline(never))]
pub(super) fn projection_blocks(
    blocks: &mut Vec<Vec<Instruction>>,
    metadata: &mut Vec<BlockMetadata>,
) -> [usize; 16] {
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
        ("List", "isDistinct", Some(Projection::ListIsDistinct)),
        ("Map", "entries", Some(Projection::Entries)),
        ("Map", "containsValue", Some(Projection::MapContainsValue)),
    ]
    .map(|(class, name, operation)| {
        let class_start = source
            .text
            .find(&format!("class {class}<"))
            .expect("collection class");
        let signature = if name == "add" {
            "external function add<Other>(element: Other):".to_owned()
        } else if name == "containsValue" {
            "external function containsValue(value: Any):".to_owned()
        } else if name == "contains" {
            "external function contains(element: Any):".to_owned()
        } else if class == "Map" || matches!(name, "distinct" | "isDistinct") {
            format!("external {name}:")
        } else {
            format!("external function {name}():")
        };
        let start = class_start
            + source
                .text
                .entire()
                .range(class_start..)
                .find(&signature)
                .expect("collection projection");
        let end = start + source.text.entire().range(start..).find("\n").unwrap();
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
            Some(operation @ (Projection::Contains | Projection::MapContainsValue)) => vec![
                Instruction::BeginCollectionProjection(operation),
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
            Some(operation @ (Projection::IsDistinct | Projection::ListIsDistinct)) => vec![
                Instruction::BeginCollectionProjection(operation),
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
    strict_list_len: Option<usize>,
    next: usize,
    current: Option<(Value, Value)>,
    matched: i64,
    accumulator: Option<Value>,
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
        roots.extend(self.accumulator.iter().cloned());
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

    /// General bulk fast-path for `StrictList.map` when the 1-arg lambda is a pure
    /// 1-or-2-hop cached member projection `(x) -> x.m1` or `(x) -> x.m1.m2` (e.g.
    /// `(c) -> c.properties.length` or `(u) -> u.name`). Stops cleanly at the first
    /// element that requires lazy evaluation or non-fast lookup so the normal VM
    /// finishes the remaining elements with exact semantics.
    #[cfg_attr(not(target_arch = "wasm32"), inline)]
    #[cfg_attr(target_arch = "wasm32", inline(never))]
    pub(super) fn try_fast_member_chain_bulk(
        &mut self,
        heap: &mut Heap,
        builder: Option<ObjectId>,
        program: &Program,
        executable_roots: &ExecutableRoots,
        max_steps: usize,
    ) -> Option<(usize, bool)> {
        let len = self.strict_list_len?;
        if !matches!(self.operation, Operation::Callback(Transform::Map))
            || self.lazy.is_some()
            || self.distinct.is_some()
            || self.next >= len
        {
            return None;
        }
        let target = builder?;
        let Some(Value::Function(fn_id)) = self.callback else {
            return None;
        };
        let cb_obj = heap.get(fn_id).ok()?;
        if cb_obj.native_function.is_some() || cb_obj.amendment.is_some() {
            return None;
        }
        let func = cb_obj.function.as_ref()?;
        if func.arity != 1 {
            return None;
        }
        let cb_block = func.block;
        let code = program.blocks.get(cb_block)?;
        #[inline(always)]
        fn skip_observe(code: &[Instruction], i: &mut usize) {
            while matches!(
                code.get(*i),
                Some(Instruction::Observe(_) | Instruction::ObserveNativeLiteral { .. })
            ) {
                *i += 1;
            }
        }
        let mut i = 0usize;
        skip_observe(code, &mut i);
        // Match the 1-arg lambda parameter prologue:
        // `LoadLocal(0); BindLocal { depth: Some(0), ty }` (standard compiler output)
        // or direct `BindLocal { depth: None, ty }`.
        let mut bind_pc = None;
        if let Some(Instruction::LoadLocal(0)) = code.get(i) {
            let mut j = i + 1;
            skip_observe(code, &mut j);
            if let Some(Instruction::BindLocal { depth: Some(0), ty }) = code.get(j) {
                if !matches!(ty.resolved(), TypeCheck::Any | TypeCheck::Unknown) {
                    return None;
                }
                bind_pc = Some(j);
                i = j + 1;
            }
        } else if let Some(Instruction::BindLocal { depth: None, ty }) = code.get(i) {
            if !matches!(ty.resolved(), TypeCheck::Any | TypeCheck::Unknown) {
                return None;
            }
            bind_pc = Some(i);
            i += 1;
        }
        skip_observe(code, &mut i);
        if !matches!(code.get(i), Some(Instruction::LoadLocal(0))) {
            return None;
        }
        i += 1;
        skip_observe(code, &mut i);
        let Some(Instruction::GetMember(m1)) = code.get(i) else {
            return None;
        };
        i += 1;
        skip_observe(code, &mut i);
        let m2 = if let Some(Instruction::GetMember(m2)) = code.get(i) {
            i += 1;
            Some(m2)
        } else {
            None
        };
        skip_observe(code, &mut i);
        if let Some(Instruction::CheckType(ty)) = code.get(i) {
            if !matches!(ty.resolved(), TypeCheck::Any | TypeCheck::Unknown) {
                return None;
            }
            i += 1;
            skip_observe(code, &mut i);
        }
        if !matches!(code.get(i), Some(Instruction::Return)) {
            return None;
        }
        if let Some(bpc) = bind_pc {
            let owner = cb_obj
                .environment
                .executable_owner
                .or_else(|| executable_roots.get(&cb_block).copied())?;
            if heap.local_kind(owner, (cb_block, bpc)).ok().flatten()
                != Some(identity::SlotKind::Object)
            {
                return None;
            }
        }
        #[inline(always)]
        fn fast_lookup_hint<'a>(
            obj: &'a Object,
            name: &PklString,
            hint: &mut usize,
        ) -> Option<&'a Value> {
            let slots = &obj.cache_slots;
            if let Some(slot) = slots.get(*hint)
                && !slot.key.entry
                && !slot.key.method
                && slot.key.owner.is_none()
                && slot.key.name == *name
            {
                return Some(&slot.value);
            }
            if slots.len() <= 16 {
                for (idx, slot) in slots.iter().enumerate() {
                    if !slot.key.entry
                        && !slot.key.method
                        && slot.key.owner.is_none()
                        && slot.key.name == *name
                    {
                        *hint = idx;
                        return Some(&slot.value);
                    }
                }
                return None;
            }
            super::fast_lookup_cached(obj, None, name, false)
        }
        #[inline(always)]
        fn fast_hop_val(
            heap: &Heap,
            oid: ObjectId,
            name: &PklString,
            is_length: bool,
            hint: &mut usize,
        ) -> Option<Value> {
            let obj = heap.get_fast(oid)?;
            if is_length
                && matches!(
                    obj.collection_kind,
                    CollectionKind::List | CollectionKind::Map | CollectionKind::Set
                )
                && obj.parent.is_none()
            {
                return Some(Value::Int(
                    crate::value_identity::Number::from(obj.length as i64).boxed(),
                ));
            }
            if obj.collection_kind == CollectionKind::Dynamic
                && obj.reference.is_none()
                && obj.pair.is_none()
                && obj.function.is_none()
                && obj.native_function.is_none()
                && (obj.class_definition.is_none() || obj.is_module)
                && obj.member_deprecations.is_empty()
            {
                return fast_lookup_hint(obj, name, hint).cloned();
            }
            None
        }
        #[inline(always)]
        fn fast_hop_obj(
            heap: &Heap,
            oid: ObjectId,
            name: &PklString,
            hint: &mut usize,
        ) -> Option<ObjectId> {
            let obj = heap.get_fast(oid)?;
            if obj.collection_kind == CollectionKind::Dynamic
                && obj.reference.is_none()
                && obj.pair.is_none()
                && obj.function.is_none()
                && obj.native_function.is_none()
                && (obj.class_definition.is_none() || obj.is_module)
                && obj.member_deprecations.is_empty()
                && let Value::Object(next) = *fast_lookup_hint(obj, name, hint)?
            {
                return Some(next);
            }
            None
        }
        let src_obj = heap.get_fast(self.source)?;
        let src_elems = src_obj.list_elements.as_ref()?;
        let end = len.min(src_elems.len());
        let remaining = end.saturating_sub(self.next);
        let step_cost = if m2.is_some() { 6usize } else { 5usize };
        let max_elems = (max_steps / step_cost).min(remaining);
        if max_elems == 0 {
            return None;
        }
        let m1_is_length = m1 == "length";
        let m2_is_length = m2.is_some_and(|m| m == "length");
        let mut hint1 = 0usize;
        let mut hint2 = 0usize;
        let mut extra_bytes = 0usize;
        let slice = &src_elems[self.next..self.next + max_elems];
        let done = if max_elems <= 64 {
            let mut buf = [const { Value::Null }; 64];
            let mut count = 0usize;
            for elem in slice {
                let Value::Object(oid1) = *elem else {
                    break;
                };
                let out = if let Some(m2) = m2 {
                    let Some(oid2) = fast_hop_obj(heap, oid1, m1, &mut hint1) else {
                        break;
                    };
                    let Some(v2) = fast_hop_val(heap, oid2, m2, m2_is_length, &mut hint2) else {
                        break;
                    };
                    v2
                } else {
                    let Some(v1) = fast_hop_val(heap, oid1, m1, m1_is_length, &mut hint1) else {
                        break;
                    };
                    v1
                };
                if let Value::String(s) = &out {
                    extra_bytes = extra_bytes.saturating_add(s.as_str().map_or(16, str::len));
                }
                buf[count] = out;
                count += 1;
            }
            if count == 0 {
                return None;
            }
            {
                let out_obj = heap.get_mut(target).ok()?;
                let elems = out_obj.list_elements.as_mut()?;
                elems.reserve(count);
                for slot in buf.iter_mut().take(count) {
                    elems.push(std::mem::replace(slot, Value::Null));
                }
                out_obj.length += count;
            }
            count
        } else {
            let mut results = Vec::with_capacity(max_elems);
            for elem in slice {
                let Value::Object(oid1) = *elem else {
                    break;
                };
                let out = if let Some(m2) = m2 {
                    let Some(oid2) = fast_hop_obj(heap, oid1, m1, &mut hint1) else {
                        break;
                    };
                    let Some(v2) = fast_hop_val(heap, oid2, m2, m2_is_length, &mut hint2) else {
                        break;
                    };
                    v2
                } else {
                    let Some(v1) = fast_hop_val(heap, oid1, m1, m1_is_length, &mut hint1) else {
                        break;
                    };
                    v1
                };
                if let Value::String(s) = &out {
                    extra_bytes = extra_bytes.saturating_add(s.as_str().map_or(16, str::len));
                }
                results.push(out);
            }
            let count = results.len();
            if count == 0 {
                return None;
            }
            {
                let out_obj = heap.get_mut(target).ok()?;
                let elems = out_obj.list_elements.as_mut()?;
                elems.extend(results);
                out_obj.length += count;
            }
            count
        };
        heap.note_external_allocation(
            done.saturating_mul(std::mem::size_of::<Value>())
                .saturating_add(extra_bytes),
        );
        self.next += done;
        let finished = self.next >= len;
        if finished {
            self.current = None;
        }
        Some((done.saturating_mul(step_cost).max(1), finished))
    }

    #[inline]
    pub(super) fn try_fast_map_next(
        &mut self,
        heap: &Heap,
        stack: &mut identity::OperandStack,
    ) -> Option<bool> {
        let len = self.strict_list_len?;
        if !matches!(self.operation, Operation::Callback(Transform::Map))
            || self.lazy.is_some()
            || self.distinct.is_some()
        {
            return None;
        }
        if self.next >= len {
            self.current = None;
            return Some(false);
        }
        let idx = self.next;
        let obj = heap.get(self.source).ok()?;
        let val = obj.list_elements.as_ref()?.get(idx)?.clone();
        let cb = self.callback.as_ref()?.clone();
        self.current = Some((Value::Int((idx as i64).into()), val.clone()));
        stack.push_raw(cb);
        stack.push_raw(val);
        self.next = idx + 1;
        Some(true)
    }

    #[cfg_attr(not(target_arch = "wasm32"), inline)]
    #[cfg_attr(target_arch = "wasm32", inline(never))]
    pub(super) fn try_inplace_map_accept_and_next(
        &mut self,
        heap: &mut Heap,
        builder: Option<ObjectId>,
        result: Value,
    ) -> Option<Option<Value>> {
        let len = self.strict_list_len?;
        if !matches!(self.operation, Operation::Callback(Transform::Map))
            || self.lazy.is_some()
            || self.distinct.is_some()
        {
            return None;
        }
        let target = builder?;
        let extra = match &result {
            Value::String(s) => s.as_str().map_or(16, str::len),
            _ => 0,
        };
        {
            let out_obj = heap.get_mut(target).ok()?;
            let elems = out_obj.list_elements.as_mut()?;
            elems.push(result);
            out_obj.length += 1;
        }
        heap.note_external_allocation(std::mem::size_of::<Value>().saturating_add(extra));
        if self.next >= len {
            self.current = None;
            return Some(None);
        }
        let idx = self.next;
        let src_obj = heap.get(self.source).ok()?;
        let next_val = src_obj.list_elements.as_ref()?.get(idx)?.clone();
        self.current = Some((Value::Int((idx as i64).into()), next_val.clone()));
        self.next = idx + 1;
        Some(Some(next_val))
    }
}

impl Vm {
    pub(super) fn collection_projection_member(
        &mut self,
        receiver: Value,
        name: &PklString,
        method: bool,
    ) -> Result<bool, RuntimeError> {
        let Some(name) = name.as_str() else {
            return Ok(false);
        };
        if !method
            && name == "values"
            && self.is_map(&receiver)
            && let Ok(id) = receiver.as_object()
        {
            let obj = self.heap.get(id)?;
            if obj.parent.is_none() {
                // Ultra-fast O(N) path when leading cache_slots align 1:1 with insertion_order
                // and all keys are pure scalars (zero ordered_members alloc, zero BTreeMap lookup).
                if obj.map_key_order.is_none()
                    && obj.cache_slots.len() >= obj.collection_members.len()
                {
                    let order = obj.map_index().insertion_order();
                    let mut values = Vec::with_capacity(order.len());
                    let mut fast_ok = true;
                    for &idx in order {
                        let member = &obj.collection_members[idx];
                        let slot = &obj.cache_slots[idx];
                        if matches!(
                            member.key,
                            Value::String(_) | Value::Int(_) | Value::Float(_) | Value::Bool(_)
                        ) && slot.key.entry
                            && slot.key.owner.is_none()
                            && !slot.key.method
                            && slot.key.name == member.name
                        {
                            values.push(slot.value.clone());
                        } else {
                            fast_ok = false;
                            break;
                        }
                    }
                    if fast_ok {
                        let list_id = self.build_strict_list(values);
                        self.stack.push(Value::Object(list_id));
                        return Ok(true);
                    }
                }
                let members = collections::ordered_members(&self.heap, id)?;
                let all_pure_scalars = members.iter().all(|m| {
                    matches!(
                        m.key,
                        Value::String(_) | Value::Int(_) | Value::Float(_) | Value::Bool(_)
                    )
                });
                if all_pure_scalars {
                    let mut values = Vec::with_capacity(members.len());
                    let mut ok = true;
                    for member in &members {
                        let key = CacheKey {
                            owner: None,
                            name: member.name.clone(),
                            method: false,
                            entry: true,
                        };
                        if let Some(val) = obj.cache_get_completed(&key) {
                            values.push(val.clone());
                        } else {
                            ok = false;
                            break;
                        }
                    }
                    if ok {
                        let list_id = self.build_strict_list(values);
                        self.stack.push(Value::Object(list_id));
                        return Ok(true);
                    }
                }
            }
        }
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
            (false, "entries") if self.is_map(&receiver) => 14,
            (true, "containsValue") if self.is_map(&receiver) => 15,
            _ => return Ok(false),
        };
        let id = self.heap.allocate(Object {
            function: Some(Function {
                block: self.program.collection_projection_blocks[index],
                arity: usize::from(matches!(index, 7 | 12 | 15)),
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

    pub(super) fn collection_transform_method(
        &mut self,
        receiver: Value,
        name: &PklString,
    ) -> bool {
        let Some(name) = name.as_str() else {
            return false;
        };
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
            (8, "any") => 25,
            (8, "filter") => 26,
            (8, "flatMap") => 27,
            (8, "fold") => 28,
            (0, "find") => 11,
            (0, "filter") => 12,
            (0, "flatMap") => 13,
            (0, "count") => 14,
            (0, "any") => 25,
            (0, "filterIndexed") => 26,
            (0, "findOrNull") => 27,
            (0, "findLast") => 28,
            (0, "findLastOrNull") => 29,
            (0, "flatMapIndexed") => 30,
            (0, "distinctBy") => 31,
            (0, "isDistinctBy") => 32,
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
        let id = self.heap.allocate_function(
            Function {
                block: self.program.collection_transform_blocks[offset + index],
                arity: if offset == 8 && name == "fold" { 2 } else { 1 },
                method: true,
                captured_this: Some(receiver),
            },
            None,
            Environment::default(),
        );
        self.stack.push(Value::Function(id));
        true
    }

    #[cfg_attr(target_arch = "wasm32", inline(never))]
    pub(super) fn begin_collection_transform(
        &mut self,
        operation: Transform,
    ) -> Result<(), RuntimeError> {
        let callback = self.pop()?;
        self.begin_collection_run(Operation::Callback(operation), Some(callback))
    }

    #[cfg_attr(target_arch = "wasm32", inline(never))]
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
        let reverse = matches!(
            operation,
            Operation::Projection(Projection::Reverse)
                | Operation::Callback(Transform::FindLast | Transform::FindLastOrNull)
        );
        let strict_list_len = {
            let src_obj = self.heap.get(source)?;
            if !reverse
                && src_obj.collection_kind == CollectionKind::List
                && src_obj.parent.is_none()
            {
                src_obj.list_elements.as_ref().map(Vec::len)
            } else {
                None
            }
        };
        // Fast synchronous strict-List projections for bounded non-object lists,
        // avoiding per-element TransformRun loops while keeping huge lists cancelable.
        if let Some(fast_result) = match operation {
            Operation::Projection(Projection::Contains)
                if strict_list_len.is_some_and(|n| n <= 512) =>
            {
                let query = self
                    .frames
                    .last()
                    .and_then(|f| f.environment.locals.first());
                if let Some(query) = query
                    && !matches!(query, Value::Object(_))
                    && let Some(elems) = self.heap.get(source)?.list_elements.as_deref()
                    && elems.iter().all(|e| !matches!(e, Value::Object(_)))
                {
                    let found = elems
                        .iter()
                        .any(|e| equality::same_key(&self.heap, e, query));
                    Some(Value::Bool(found))
                } else {
                    None
                }
            }
            Operation::Projection(Projection::Distinct)
                if strict_list_len.is_some_and(|n| n <= 512) =>
            {
                if let Some(elems) = self.heap.get(source)?.list_elements.as_deref()
                    && elems.iter().all(|e| !matches!(e, Value::Object(_)))
                {
                    let mut unique = Vec::with_capacity(elems.len());
                    if elems.len() > 16 && elems.iter().all(|e| matches!(e, Value::String(_))) {
                        let mut seen = crate::heap::FastMap::default();
                        seen.reserve(elems.len());
                        for e in elems {
                            let Value::String(s) = e else { unreachable!() };
                            if seen.insert(s.clone(), ()).is_none() {
                                unique.push(e.clone());
                            }
                        }
                    } else if elems.len() <= 64 {
                        for e in elems {
                            if !unique.iter().any(|u| equality::same_key(&self.heap, u, e)) {
                                unique.push(e.clone());
                            }
                        }
                    } else {
                        unique.clear();
                    }
                    if elems.is_empty() || !unique.is_empty() {
                        let list_id = self.build_strict_list(unique);
                        Some(Value::Object(list_id))
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            Operation::Projection(Projection::ListIsDistinct)
                if strict_list_len.is_some_and(|n| n <= 512) =>
            {
                if let Some(elems) = self.heap.get(source)?.list_elements.as_deref()
                    && elems.iter().all(|e| !matches!(e, Value::Object(_)))
                {
                    if elems.len() <= 1 {
                        Some(Value::Bool(true))
                    } else if elems.len() > 16
                        && elems.iter().all(|e| matches!(e, Value::String(_)))
                    {
                        let mut seen = crate::heap::FastMap::default();
                        seen.reserve(elems.len());
                        let mut distinct = true;
                        for e in elems {
                            let Value::String(s) = e else { unreachable!() };
                            if seen.insert(s.clone(), ()).is_some() {
                                distinct = false;
                                break;
                            }
                        }
                        Some(Value::Bool(distinct))
                    } else if elems.len() <= 64 {
                        let mut distinct = true;
                        'outer: for i in 1..elems.len() {
                            for j in 0..i {
                                if equality::same_key(&self.heap, &elems[i], &elems[j]) {
                                    distinct = false;
                                    break 'outer;
                                }
                            }
                        }
                        Some(Value::Bool(distinct))
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            Operation::Projection(Projection::Add) if strict_list_len.is_some_and(|n| n <= 512) => {
                let extra = self
                    .frames
                    .last()
                    .and_then(|f| f.environment.locals.last())
                    .cloned();
                if let Some(extra) = extra
                    && let Some(elems) = self.heap.get(source)?.list_elements.as_deref()
                {
                    let mut out = Vec::with_capacity(elems.len() + 1);
                    out.extend(elems.iter().cloned());
                    out.push(extra);
                    let approx = out.len().saturating_mul(std::mem::size_of::<Value>());
                    self.heap.note_external_allocation(approx);
                    let list_id = self.heap.allocate_strict_list(out);
                    Some(Value::Object(list_id))
                } else {
                    None
                }
            }
            _ => None,
        } {
            let ret_pc = self.program.blocks[self.frames.last().unwrap().block].len() - 1;
            self.stack.push(fast_result);
            return self.jump(ret_pc);
        }
        let mut members = if strict_list_len.is_some() {
            Vec::new()
        } else if matches!(
            operation,
            Operation::Projection(Projection::KeysHash | Projection::MapContainsValue)
        ) {
            self.map_hash_members(source)?
        } else {
            collections::ordered_members(&self.heap, source)?
        };
        if reverse {
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
                    | Transform::IsDistinctBy
                    | Transform::Fold
            ) | Operation::Projection(
                Projection::IsDistinct
                    | Projection::ListIsDistinct
                    | Projection::Contains
                    | Projection::MapContainsValue
            )
        ) {
            None
        } else {
            let out_kind = match operation {
                Operation::Every => unreachable!("every has no result builder"),
                Operation::Callback(_) => self.heap.get(source)?.collection_kind,
                Operation::Projection(Projection::ToSet | Projection::KeysHash) => {
                    CollectionKind::Set
                }
                Operation::Projection(_) => CollectionKind::List,
            };
            if out_kind == CollectionKind::List {
                let cap = strict_list_len.unwrap_or(members.len());
                let elems = if matches!(
                    operation,
                    Operation::Callback(Transform::Map | Transform::Indexed)
                        | Operation::Projection(Projection::ToList | Projection::Reverse)
                ) {
                    Vec::with_capacity(cap)
                } else {
                    Vec::new()
                };
                Some(self.heap.allocate_strict_list(elems))
            } else {
                Some(self.heap.allocate(Object {
                    collection_kind: out_kind,
                    list_elements: None,
                    ..Object::default()
                }))
            }
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
            strict_list_len,
            next: 0,
            current: None,
            matched: 0,
            accumulator: if matches!(operation, Operation::Callback(Transform::Fold)) {
                Some(frame.environment.locals[frame.environment.locals.len() - 2].clone())
            } else {
                None
            },
            lazy: None,
            inner: None,
            distinct: match operation {
                Operation::Projection(
                    Projection::Distinct | Projection::IsDistinct | Projection::ListIsDistinct,
                )
                | Operation::Callback(Transform::DistinctBy | Transform::IsDistinctBy) => {
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

    #[cfg_attr(target_arch = "wasm32", inline(never))]
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
        if let Some(len) = run.strict_list_len {
            if run.next >= len {
                return self.jump(end);
            }
            let idx = run.next;
            let source = run.source;
            let value = self
                .heap
                .get(source)?
                .list_elements
                .as_ref()
                .and_then(|elems| elems.get(idx))
                .cloned()
                .ok_or_else(|| invalid("uncached strict list transform element"))?;
            let run = self
                .frames
                .last_mut()
                .and_then(|frame| frame.collection_transform.as_mut())
                .expect("transform cursor");
            if matches!(run.operation, Operation::Every | Operation::Callback(_)) {
                self.stack
                    .push(run.callback.clone().expect("transform callback"));
                if let Some(accumulator) = &run.accumulator {
                    self.stack.push(accumulator.clone());
                }
                if matches!(run.operation, Operation::Callback(operation) if operation.indexed()) {
                    self.stack.push(Value::Int((idx as i64).into()));
                }
                run.current = Some((Value::Int((idx as i64).into()), value.clone()));
            }
            self.stack.push(value);
            run.next += 1;
            return Ok(());
        }
        let Some(member) = run.members.get(run.next).cloned() else {
            return self.jump(end);
        };
        let source = run.source;
        let entries = matches!(run.operation, Operation::Projection(Projection::Entries));
        let map = self.heap.get(source)?.collection_kind == CollectionKind::Map;
        let value = if matches!(
            run.operation,
            Operation::Projection(Projection::KeysHash | Projection::KeysOrder)
        ) {
            member.key.clone()
        } else if matches!(
            run.operation,
            Operation::Projection(Projection::MapContainsValue)
        ) {
            // Paguro UnmodMap.containsValue walks the backing trie directly:
            // read its stored value without rehashing/relooking up this key.
            match self.heap.get(source)?.cache_get_completed(&CacheKey {
                owner: None,
                name: member.name.clone(),
                method: false,
                entry: true,
            }) {
                Some(value) => value.clone(),
                None => return Err(invalid("uncached Map.containsValue entry")),
            }
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
        let value = if entries {
            // VmMap.entries allocates a fresh Pair for each insertion-order
            // entry after its ordinary resumable map.get(key) above completes.
            self.make_pair(member.key.clone(), value)
        } else {
            value
        };
        let run = self
            .frames
            .last_mut()
            .and_then(|frame| frame.collection_transform.as_mut())
            .expect("transform cursor");
        if matches!(run.operation, Operation::Every | Operation::Callback(_)) {
            self.stack
                .push(run.callback.clone().expect("transform callback"));
            if let Some(accumulator) = &run.accumulator {
                self.stack.push(accumulator.clone());
            }
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

    #[cfg_attr(target_arch = "wasm32", inline(never))]
    pub(super) fn collection_contains_step(&mut self, matched: usize) -> Result<(), RuntimeError> {
        let non_object_match = {
            let element = self
                .stack
                .last()
                .ok_or_else(|| invalid("contains element"))?;
            let query = self
                .frames
                .last()
                .and_then(|f| f.environment.locals.first())
                .ok_or_else(|| invalid("contains argument"))?;
            if !matches!(element, Value::Object(_)) && !matches!(query, Value::Object(_)) {
                Some(equality::same_key(&self.heap, element, query))
            } else {
                None
            }
        };
        if let Some(is_match) = non_object_match {
            self.pop()?;
            if is_match {
                self.frames.last_mut().unwrap().collection_transform = None;
                self.stack.push(Value::Bool(true));
                self.jump(matched)?;
            }
            return Ok(());
        }
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
        // Pinned Paguro UnmodList.contains and UnmodMap.containsValue both
        // call Objects.equals(stored element/value, query).
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
            Transform::Fold => {
                self.frames
                    .last_mut()
                    .unwrap()
                    .collection_transform
                    .as_mut()
                    .unwrap()
                    .accumulator = Some(value);
            }
            Transform::DistinctBy | Transform::IsDistinctBy => {
                self.frames
                    .last_mut()
                    .unwrap()
                    .collection_transform
                    .as_mut()
                    .unwrap()
                    .current = Some((key, input));
                self.stack.push(value);
            }
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
                    if matches!(operation, Transform::Filter)
                        && self.heap.get(source)?.collection_kind == CollectionKind::Map
                    {
                        self.stack.push(key);
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
                let map = self.heap.get(source)?.collection_kind == CollectionKind::Map;
                if if map {
                    !self.is_map(&value)
                } else {
                    !self.is_list(&value) && !self.is_set(&value)
                } {
                    return Err(type_error(
                        &if map {
                            TypeCheck::Collection(CollectionKind::Map)
                        } else {
                            TypeCheck::CollectionBase { arguments: vec![] }
                        },
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
    #[cfg_attr(target_arch = "wasm32", inline(never))]
    pub(super) fn collection_flat_map_next(&mut self, end: usize) -> Result<(), RuntimeError> {
        let (source, member) = {
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
            (inner.source, member)
        };
        let map = self.heap.get(source)?.collection_kind == CollectionKind::Map;
        let value = if map {
            if self.prepare_map_lookup(source, &member.key)? {
                return Ok(());
            }
            self.map_value(source, &member.key)?
                .ok_or_else(|| invalid("flatMap Map key"))?
        } else {
            collections::cached_render_value(&self.heap, source, &member)?
                .ok_or_else(|| invalid("uncached strict flatMap element"))?
        };
        self.frames
            .last_mut()
            .unwrap()
            .collection_transform
            .as_mut()
            .unwrap()
            .inner
            .as_mut()
            .unwrap()
            .next += 1;
        if map {
            self.stack.push(member.key);
        }
        self.stack.push(value);
        Ok(())
    }
}

#[inline]
pub(super) fn format_list_element_name(target_index: usize, idx: usize) -> PklString {
    use std::io::Write;
    let mut buf = [0u8; 48];
    let len = {
        let mut cursor = std::io::Cursor::new(&mut buf[..]);
        write!(cursor, "\0{target_index}:{idx}").expect("list element key fits 48 bytes");
        cursor.position() as usize
    };
    let s = std::str::from_utf8(&buf[..len]).expect("ASCII list element key");
    PklString::from(s)
}

impl Vm {
    /// Build a strict immutable `List` directly from precomputed elements using compact dense storage.
    pub(super) fn build_strict_list(&mut self, values: Vec<Value>) -> ObjectId {
        let len = values.len();
        if len == 0 {
            return self.empty_strict_collection(CollectionKind::List);
        }
        let mut approx_bytes = len.saturating_mul(std::mem::size_of::<Value>());
        for v in &values {
            if let Value::String(s) = v {
                approx_bytes = approx_bytes.saturating_add(s.as_str().map_or(16, str::len));
            }
        }
        self.heap.note_external_allocation(approx_bytes);
        self.heap.allocate(Object {
            collection_kind: CollectionKind::List,
            length: len,
            list_elements: Some(values),
            ..Default::default()
        })
    }

    pub(super) fn append_strict_element(&mut self) -> Result<(), RuntimeError> {
        let value = self.pop()?;
        let target = self.builder()?;
        let is_fast_list = {
            let object = self.heap.get(target)?;
            object.collection_kind == CollectionKind::List
                && object.parent.is_none()
                && (object.list_elements.is_some()
                    || (object.length == 0
                        && object.collection_members.is_empty()
                        && object.members.is_empty()
                        && object.cache_slots.is_empty()))
        };
        if is_fast_list {
            let extra = match &value {
                Value::String(s) => s.as_str().map_or(16, str::len),
                _ => 0,
            };
            self.heap
                .note_external_allocation(std::mem::size_of::<Value>().saturating_add(extra));
            let object = self.heap.get_mut(target)?;
            object
                .list_elements
                .get_or_insert_with(Vec::new)
                .push(value);
            object.length += 1;
            return Ok(());
        }
        self.add_collection_member(
            Member {
                flags: 0,
                entry: true,
                name: PklString::default(),
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
        if matches!(run.operation, Operation::Callback(Transform::Fold)) {
            self.stack.push(
                run.accumulator
                    .ok_or_else(|| invalid("Map.fold accumulator"))?,
            );
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
            let rendered = crate::diagnostic_values::render_string(
                &Value::Object(run.source),
                &self.heap,
                &self.program.module_name,
                80,
            )?;
            return Err(RuntimeError::with_program_values(
                ErrorKind::Type,
                "Cannot find matching element in collection.",
                &[("Collection", rendered)],
            ));
        }
        if matches!(run.operation, Operation::Callback(Transform::Count)) {
            self.stack.push(Value::Int(run.matched.into()));
            return Ok(());
        }
        if matches!(
            run.operation,
            Operation::Projection(Projection::Contains | Projection::MapContainsValue)
        ) {
            self.stack.push(Value::Bool(false));
            return Ok(());
        }
        if matches!(
            run.operation,
            Operation::Projection(Projection::IsDistinct | Projection::ListIsDistinct)
                | Operation::Callback(Transform::IsDistinctBy)
        ) {
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

#[cfg_attr(target_arch = "wasm32", inline(never))]
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
            let start = class
                + source
                    .text
                    .entire()
                    .range(class..)
                    .find(&signature)
                    .unwrap();
            let end = start + source.text.entire().range(start..).find("\n").unwrap();
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
    #[cfg_attr(target_arch = "wasm32", inline(never))]
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
        let (member, is_empty) = if self.heap.get(id)?.collection_kind == CollectionKind::List
            && self.heap.get(id)?.parent.is_none()
        {
            let obj = self.heap.get(id)?;
            if single && obj.length != 1 {
                if nullable {
                    self.stack.push(Value::Null);
                    return Ok(());
                }
                let rendered = crate::diagnostic_values::render_string(
                    &receiver,
                    &self.heap,
                    &self.program.module_name,
                    80,
                )?;
                return Err(RuntimeError::with_program_values(
                    ErrorKind::Type,
                    "Expected a single-element collection.",
                    &[("Collection", rendered)],
                ));
            }
            if let Some(elems) = &obj.list_elements {
                let picked = if last { elems.last() } else { elems.first() };
                if let Some(value) = picked {
                    self.stack.push(value.clone());
                    return Ok(());
                }
                (None, true)
            } else {
                let mem = if last {
                    if obj.length > obj.collection_members.len()
                        && !obj.collection_members.is_empty()
                    {
                        let base_len = obj
                            .repeat_base_len
                            .unwrap_or(obj.collection_members.len())
                            .max(1);
                        let resolved_idx = (obj.length - 1) % base_len;
                        Some(obj.collection_members[resolved_idx].clone())
                    } else {
                        obj.collection_members.last().cloned()
                    }
                } else {
                    obj.collection_members.first().cloned()
                };
                let empty = obj.length == 0;
                (mem, empty)
            }
        } else {
            let members = collections::ordered_members(&self.heap, id)?;
            if single && members.len() != 1 {
                if nullable {
                    self.stack.push(Value::Null);
                    return Ok(());
                }
                let rendered = crate::diagnostic_values::render_string(
                    &receiver,
                    &self.heap,
                    &self.program.module_name,
                    80,
                )?;
                return Err(RuntimeError::with_program_values(
                    ErrorKind::Type,
                    "Expected a single-element collection.",
                    &[("Collection", rendered)],
                ));
            }
            let empty = members.is_empty();
            let mem = if last {
                members.last().cloned()
            } else {
                members.first().cloned()
            };
            (mem, empty)
        };
        let value = if let Some(member) = &member {
            collections::cached_render_value(&self.heap, id, member)?
                .ok_or_else(|| invalid("uncached strict collection endpoint"))?
        } else if nullable || is_empty {
            if is_empty && !nullable {
                return Err(RuntimeError::with_program_values(
                    ErrorKind::Type,
                    "Expected a non-empty collection.",
                    &[(
                        "Collection",
                        format!("{:?}()", self.heap.get(id)?.collection_kind).into(),
                    )],
                ));
            }
            Value::Null
        } else {
            return Err(RuntimeError::with_program_values(
                ErrorKind::Type,
                "Expected a non-empty collection.",
                &[(
                    "Collection",
                    format!("{:?}()", self.heap.get(id)?.collection_kind).into(),
                )],
            ));
        };
        self.stack.push(value);
        Ok(())
    }
}

/// Predicate calls have no result builder and do not eagerly force lazy values.
#[cfg_attr(target_arch = "wasm32", inline(never))]
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
            + source
                .text
                .entire()
                .range(class_start..)
                .find("external function every(")
                .expect("pinned every");
        let end = start + source.text.entire().range(start..).find("\n").unwrap();
        let parameter = start + source.text.entire().range(start..end).find(": ").unwrap() + 2;
        let type_end = start
            + source
                .text
                .entire()
                .range(start..end)
                .rfind("): Boolean")
                .unwrap();
        let arity = if matches!(class, "Map" | "Mapping") {
            2
        } else {
            1
        };
        let code = vec![
            Instruction::LoadLocal(0),
            Instruction::CheckType(Box::new(TypeCheck::Function {
                parameters: Some(vec![TypeCheck::Any; arity]),
                result: Box::new(TypeCheck::Any),
                syntax: true,
            })),
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
            || collections::is_typed_class(&self.heap, source)?
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

    #[cfg_attr(target_arch = "wasm32", inline(never))]
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
            strict_list_len: None,
            next: 0,
            current: None,
            matched: 0,
            accumulator: None,
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

    #[cfg_attr(target_arch = "wasm32", inline(never))]
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

    #[cfg_attr(target_arch = "wasm32", inline(never))]
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
    known: crate::heap::FastMap<i32, Vec<usize>>,
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
        name: &PklString,
    ) -> Result<Option<Member>, RuntimeError> {
        let Some(name) = name.as_str() else {
            return Ok(None);
        };
        let code = match (self.heap.get(receiver)?.collection_kind, name) {
            (CollectionKind::List, "distinct") => self.program.collection_projection_blocks[10],
            (CollectionKind::List, "isDistinct") => self.program.collection_projection_blocks[13],
            (CollectionKind::Listing, "isDistinct") => {
                self.program.collection_projection_blocks[11]
            }
            (CollectionKind::Listing, name) if listing_endpoint(name) || name == "distinct" => {
                self.program.primitive_methods.expect("Listing", name)
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
    // Mapping.default uses this same cache path before an implicit invocation.
    pub(super) fn steal_collection_property(
        &mut self,
        receiver: ObjectId,
        name: &PklString,
    ) -> Result<bool, RuntimeError> {
        let Some(name) = name.as_str() else {
            return Ok(false);
        };
        let kind = self.heap.get(receiver)?.collection_kind;
        let listing_cached = kind == CollectionKind::Listing
            && (matches!(name, "isDistinct" | "distinct") || listing_endpoint(name));
        let mapping_default = kind == CollectionKind::Mapping && name == "default";
        if !listing_cached && !mapping_default {
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
            if !object.members.is_empty()
                || !object.collection_members.is_empty()
                || (mapping_default && object.collection_default.is_some())
            {
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
    #[cfg_attr(target_arch = "wasm32", inline(never))]
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
        let operation = self
            .frames
            .last()
            .unwrap()
            .collection_transform
            .as_ref()
            .unwrap()
            .operation;
        let stop_on_duplicate = matches!(
            operation,
            Operation::Projection(Projection::IsDistinct | Projection::ListIsDistinct)
                | Operation::Callback(Transform::IsDistinctBy)
        );
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
                    if stop_on_duplicate {
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
                if matches!(operation, Operation::Callback(Transform::DistinctBy)) {
                    self.pop()?;
                    let input = self
                        .frames
                        .last_mut()
                        .unwrap()
                        .collection_transform
                        .as_mut()
                        .unwrap()
                        .current
                        .take()
                        .unwrap()
                        .1;
                    self.stack.push(input);
                } else if stop_on_duplicate {
                    self.pop()?;
                }
                return Ok(());
            }
        }
        self.retry_distinct_step();
        Ok(())
    }
}
