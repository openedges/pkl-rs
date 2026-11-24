//! Ordered lazy collections and bytecode construction/iteration continuations.
use super::*;
use crate::heap::{
    CollectionIterator, CollectionKind, CollectionMember, CollectionMemberKind, PredicateIterator,
};

#[inline]
pub(super) fn typed_class_ref(heap: &Heap, id: ObjectId) -> Result<Option<&Class>, RuntimeError> {
    // Fast unroll for depth 0 and 1 (covers >95% of collections and builtin prototypes
    // without initializing the cycle-detection stack array).
    let first = heap.get(id)?;
    if let Some(class) = &first.class_definition {
        if matches!(
            first.builtin_type.as_deref(),
            Some(TypeCheck::Collection(_))
        ) {
            return Ok(None);
        }
        return Ok(Some(class));
    }
    let Some(parent1) = first.parent else {
        return Ok(None);
    };
    if parent1 == id {
        return Err(RuntimeError::new(ErrorKind::Cycle, "cyclic parent chain"));
    }
    let second = heap.get(parent1)?;
    if let Some(class) = &second.class_definition {
        if matches!(
            second.builtin_type.as_deref(),
            Some(TypeCheck::Collection(_))
        ) {
            return Ok(None);
        }
        return Ok(Some(class));
    }
    let Some(mut cur) = second.parent else {
        return Ok(None);
    };
    let mut visited = [id, parent1, id, id, id, id, id, id];
    let mut depth = 2usize;
    let mut overflow: Option<BTreeSet<ObjectId>> = None;
    loop {
        if let Some(set) = &mut overflow {
            if !set.insert(cur) {
                return Err(RuntimeError::new(ErrorKind::Cycle, "cyclic parent chain"));
            }
        } else if visited[..depth].contains(&cur) {
            return Err(RuntimeError::new(ErrorKind::Cycle, "cyclic parent chain"));
        } else if depth < visited.len() {
            visited[depth] = cur;
            depth += 1;
        } else {
            let mut set: BTreeSet<ObjectId> = visited.into_iter().collect();
            set.insert(cur);
            overflow = Some(set);
        }
        let object = heap.get(cur)?;
        if let Some(class) = &object.class_definition {
            if matches!(
                object.builtin_type.as_deref(),
                Some(TypeCheck::Collection(_))
            ) {
                return Ok(None);
            }
            return Ok(Some(class));
        }
        let Some(parent) = object.parent else {
            return Ok(None);
        };
        cur = parent;
    }
}

#[inline]
pub(super) fn is_typed_class(heap: &Heap, id: ObjectId) -> Result<bool, RuntimeError> {
    Ok(typed_class_ref(heap, id)?.is_some())
}

#[inline]
pub(super) fn typed_class(heap: &Heap, id: ObjectId) -> Result<Option<Class>, RuntimeError> {
    Ok(typed_class_ref(heap, id)?.cloned())
}

#[inline]
pub(super) fn format_entry_internal_name(object_index: usize, member_index: usize) -> PklString {
    let mut buf = [0u8; 48];
    let mut b1 = [0u8; 24];
    let mut b2 = [0u8; 24];
    let s1 = super::format_i64_buf(object_index as i64, &mut b1);
    let s2 = super::format_i64_buf(member_index as i64, &mut b2);
    buf[0] = 0;
    let l1 = s1.len();
    let l2 = s2.len();
    buf[1..1 + l1].copy_from_slice(s1.as_bytes());
    buf[1 + l1] = b':';
    buf[2 + l1..2 + l1 + l2].copy_from_slice(s2.as_bytes());
    let text = std::str::from_utf8(&buf[..2 + l1 + l2]).expect("ASCII entry name");
    PklString::from(text)
}

use super::equality::same_key;

#[cfg(test)]
#[path = "collections_tests.rs"]
mod tests;

#[cfg_attr(target_arch = "wasm32", inline(never))]
pub(super) fn join_blocks(
    blocks: &mut Vec<Vec<Instruction>>,
    metadata: &mut Vec<BlockMetadata>,
) -> [usize; 4] {
    let source = crate::stdlib_sources::base();
    ["List", "Set", "Listing", "ForeignMapEntry"].map(|class| {
        if class == "ForeignMapEntry" {
            let block = blocks.len();
            blocks.push(vec![
                Instruction::LoadLocal(1),
                Instruction::JavaValueString,
                Instruction::Constant(Value::string("=")),
                Instruction::Add,
                Instruction::LoadLocal(0),
                Instruction::JavaValueString,
                Instruction::Add,
                Instruction::Return,
            ]);
            metadata.push(BlockMetadata {
                spans: vec![None; 8],
                ..Default::default()
            });
            return block;
        }
        let class_start = source
            .text
            .find(&format!("class {class}<"))
            .expect("pinned collection class");
        let declaration = "external function join(separator: String): String";
        let start = class_start
            + source
                .text
                .entire()
                .range(class_start..)
                .find(declaration)
                .expect("pinned collection join");
        let parameter = start + "external function join(separator: ".len();
        let block = blocks.len();
        blocks.push(vec![
            Instruction::LoadLocal(0),
            Instruction::CheckType(Box::new(TypeCheck::String)),
            Instruction::CollectionJoin,
            Instruction::Return,
        ]);
        metadata.push(BlockMetadata {
            source: Some(source.clone()),
            member: format!("#{class}.join").into(),
            declaration: Some(Span::new(start, start + declaration.len())),
            spans: vec![None, Some(Span::new(parameter, parameter + 6)), None, None],
            ..BlockMetadata::default()
        });
        block
    })
}

pub(super) struct JoinRun {
    source: ObjectId,
    shallow: bool,
    values: Vec<Value>,
    separator: PklString,
    next: usize,
    awaiting: bool,
    output: crate::pkl_string::PklStringBuilder,
}
impl JoinRun {
    pub(super) fn roots(&self, roots: &mut Vec<Value>) {
        roots.push(Value::Object(self.source));
        roots.extend(self.values.iter().cloned());
    }
}

impl Vm {
    pub(super) fn empty_object(&mut self, kind: CollectionKind) -> ObjectId {
        let (index, class) = match kind {
            CollectionKind::Dynamic => (0, classes::BuiltinClass::Dynamic),
            CollectionKind::Listing => (1, classes::BuiltinClass::Listing),
            CollectionKind::Mapping => (2, classes::BuiltinClass::Mapping),
            _ => unreachable!("lazy empty object"),
        };
        // Like the strict empty cache, this is weak: a retained value preserves
        // its identity, while an unreferenced singleton does not keep a heap
        // alive after a query. Generational handles prevent stale-slot reuse.
        if let Some(id) = self.empty_objects[index]
            && self.heap.get(id).is_ok()
        {
            return id;
        }
        let class = self.canonical_builtin_class(class);
        let parent = self.heap.get_class(class).expect("builtin class").prototype;
        let id = self.heap.allocate(Object {
            parent: Some(parent),
            collection_kind: kind,
            ..Object::default()
        });
        self.empty_objects[index] = Some(id);
        id
    }

    fn amendment_class_parent(&self, class: ObjectId) -> Result<ObjectId, RuntimeError> {
        let prototype = self.heap.get_class(class)?.prototype;
        self.check_class_instantiable(self.heap.class_definition(class)?)?;
        if self.class_external(class)? {
            return Err(external_amendment_error(
                &self.heap.class_display_name(class)?,
            ));
        }
        Ok(prototype)
    }

    pub(super) fn empty_strict_collection(&mut self, kind: CollectionKind) -> ObjectId {
        let index = match kind {
            CollectionKind::List => 0,
            CollectionKind::Set => 1,
            CollectionKind::Map => 2,
            _ => unreachable!("strict empty collection"),
        };
        if let Some(id) = self.empty_collections[index]
            && self.heap.get(id).is_ok()
        {
            return id;
        }
        let id = self.heap.allocate(Object {
            collection_kind: kind,
            ..Object::default()
        });
        self.empty_collections[index] = Some(id);
        id
    }

    pub(super) fn finish_strict_collection(
        &mut self,
        id: ObjectId,
    ) -> Result<ObjectId, RuntimeError> {
        let object = self.heap.get(id)?;
        if matches!(
            object.collection_kind,
            CollectionKind::List | CollectionKind::Set | CollectionKind::Map
        ) && object.length == 0
            && object.members.is_empty()
            && object.collection_members.is_empty()
            && object.list_elements.as_ref().is_none_or(Vec::is_empty)
            && object.set_entries.is_empty()
            && object.parent.is_none()
            && object.collection_cast.is_none()
        {
            Ok(self.empty_strict_collection(object.collection_kind))
        } else {
            Ok(id)
        }
    }

    pub(super) fn collection_join_method(&mut self, receiver: Value) {
        let index = if self.is_list(&receiver) {
            0
        } else if self.is_set(&receiver) {
            1
        } else {
            2
        };
        let block = self.program.collection_join_blocks[index];
        let id = self.heap.allocate(Object {
            function: Some(Function {
                block,
                arity: 1,
                method: true,
                captured_this: Some(receiver),
            }),
            ..Object::default()
        });
        self.stack.push(Value::Function(id));
    }

    #[cfg_attr(target_arch = "wasm32", inline(never))]
    pub(super) fn begin_collection_join(&mut self) -> Result<(), RuntimeError> {
        let Value::String(separator) = self.pop()? else {
            return Err(invalid("unchecked collection join separator"));
        };
        let receiver = self
            .frames
            .last()
            .and_then(|f| f.environment.this_value.clone())
            .ok_or_else(|| invalid("collection join without receiver"))?
            .as_object()?;
        let shallow = self.heap.get(receiver)?.collection_kind == CollectionKind::Listing;
        // ListingNodes.join returns before force/iteration for an empty receiver.
        // The separator's String check has already run in the synthetic block.
        if shallow && self.heap.get(receiver)?.length == 0 {
            self.stack.push(Value::String("".into()));
            return Ok(());
        }
        if !shallow {
            let object = self.heap.get(receiver)?;
            if let Some(elems) = &object.list_elements {
                if elems.is_empty() {
                    self.stack.push(Value::String("".into()));
                    return Ok(());
                }
                if let Some(sep_str) = separator.as_str()
                    && elems
                        .iter()
                        .all(|val| matches!(val, Value::String(s) if s.as_str().is_some()))
                {
                    let total_bytes: usize = elems
                        .iter()
                        .map(|val| match val {
                            Value::String(s) => s.as_str().unwrap().len(),
                            _ => unreachable!(),
                        })
                        .sum::<usize>()
                        + sep_str.len().saturating_mul(elems.len() - 1);
                    let mut out = std::mem::take(&mut self.scratch_utf8);
                    out.clear();
                    if out.capacity() < total_bytes {
                        out.reserve(total_bytes - out.capacity());
                    }
                    for (idx, val) in elems.iter().enumerate() {
                        if idx != 0 {
                            out.push_str(sep_str);
                        }
                        if let Value::String(s) = val {
                            out.push_str(s.as_str().unwrap());
                        }
                    }
                    let joined = PklString::from(out.as_str());
                    out.clear();
                    if out.capacity() <= 512 * 1024 {
                        self.scratch_utf8 = out;
                    }
                    self.stack.push(Value::String(joined));
                    return Ok(());
                }
                if elems.iter().all(|val| {
                    matches!(
                        val,
                        Value::String(_)
                            | Value::Int(_)
                            | Value::Float(_)
                            | Value::Bool(_)
                            | Value::Null
                            | Value::NullWithDefault(_)
                    )
                }) {
                    let mut out = crate::pkl_string::PklStringBuilder::default();
                    for (idx, val) in elems.iter().enumerate() {
                        if idx != 0 {
                            out.push_value(&separator);
                        }
                        if let Value::String(s) = val {
                            out.push_value(s);
                        } else {
                            out.push_value(&scalar_string(val)?);
                        }
                    }
                    self.stack.push(Value::String(out.finish()));
                    return Ok(());
                }
            }
        }
        let values = if shallow {
            Vec::new()
        } else {
            ordered_members(&self.heap, receiver)?
                .iter()
                .map(|member| {
                    cached_render_value(&self.heap, receiver, member)?
                        .ok_or_else(|| invalid("uncached strict collection element"))
                })
                .collect::<Result<Vec<_>, _>>()?
        };
        self.frames
            .last_mut()
            .expect("active join frame")
            .collection_join = Some(Box::new(JoinRun {
            source: receiver,
            shallow,
            values,
            separator,
            next: 0,
            awaiting: false,
            output: crate::pkl_string::PklStringBuilder::default(),
        }));
        if shallow {
            self.force_shallow_frame(Value::Object(receiver))?;
        }
        Ok(())
    }

    #[cfg_attr(target_arch = "wasm32", inline(never))]
    pub(super) fn step_collection_join(&mut self) -> Result<(), RuntimeError> {
        let frame = self.frames.len() - 1;
        let mut run = self.frames[frame]
            .collection_join
            .take()
            .expect("active join");
        if run.shallow {
            self.pop()?;
            run.shallow = false;
            run.values = ordered_members(&self.heap, run.source)?
                .iter()
                .map(|member| {
                    cached_render_value(&self.heap, run.source, member)?
                        .ok_or_else(|| invalid("uncached shallow Listing join element"))
                })
                .collect::<Result<Vec<_>, _>>()?;
        }
        if run.awaiting {
            let Value::String(text) = self.pop()? else {
                return Err(invalid("non-string collection element rendering"));
            };
            run.output.push_value(&text);
        }
        let Some(value) = run.values.get(run.next).cloned() else {
            self.stack.push(Value::String(run.output.finish()));
            return Ok(());
        };
        if run.next != 0 {
            run.output.push_value(&run.separator);
        }
        run.next += 1;
        run.awaiting = true;
        self.frames[frame].collection_join = Some(run);

        self.java_value_string(value)
    }

    // Java Object.toString is also used for PropertiesRenderer path components.
    // Preserve the same forcing and reference behavior as collection join.
    #[cfg_attr(target_arch = "wasm32", inline(never))]
    pub(super) fn java_value_string(&mut self, value: Value) -> Result<(), RuntimeError> {
        if let Value::Object(id) = &value
            && let Some(entry) = self.heap.get(*id)?.foreign_map_entry.clone()
        {
            // Map.entry's Java toString concatenates key + "=" + value. This is
            // distinct from Pkl method dispatch, which hits an upstream internal
            // error for these foreign objects. Each child can suspend independently.
            let function = self.heap.allocate(Object {
                function: Some(Function {
                    block: self.program.collection_join_blocks[3],
                    arity: 2,
                    method: false,
                    captured_this: None,
                }),
                ..Default::default()
            });
            return self.invoke_function(function, entry.to_vec(), false);
        }
        // StringBuilder.append(Object) invokes Java value stringification, not
        // Pkl's virtual Typed.toString. Objects force deeply; strict collections
        // render their cached contents without forcing nested objects.
        if let Value::Object(id) = value
            && self.heap.get(id)?.reference.is_some()
        {
            self.render_reference(id)?;
        } else if self.is_list(&value) || self.is_set(&value) {
            let text = crate::diagnostic_values::render_multiline(
                &value,
                &self.heap,
                &self.program.module_name,
            )?;
            self.stack.push(Value::String(text));
        } else if let Value::Bytes(bytes) = &value {
            // VmBytes.toString is not the diagnostic renderer's eight-byte preview.
            let mut text = crate::pkl_string::PklStringBuilder::default();
            text.push_str("Bytes(");
            for (index, byte) in bytes.iter().enumerate() {
                if index != 0 {
                    text.push_str(", ");
                }
                text.push_str(&byte.to_string());
            }
            text.push(')');
            self.stack.push(Value::String(text.finish()));
        } else if self.is_map(&value)
            || matches!(
                value,
                Value::Class(_) | Value::TypeAlias(_) | Value::Function(_)
            )
        {
            let text = crate::diagnostic_values::render_string(
                &value,
                &self.heap,
                &self.program.module_name,
                usize::MAX,
            )?;
            self.stack.push(Value::String(text));
        } else if matches!(value, Value::Object(_)) {
            self.begin_object_string(value)?;
        } else {
            self.stack.push(Value::String(scalar_string(&value)?));
        }
        Ok(())
    }
}

fn same_member(heap: &Heap, left: &CollectionMember, right: &CollectionMember) -> bool {
    (left.kind == CollectionMemberKind::Property) == (right.kind == CollectionMemberKind::Property)
        && same_key(heap, &left.key, &right.key)
}

#[inline]
pub(super) fn is_single_level_leaf_collection(heap: &Heap, object: &Object) -> bool {
    if object.is_module || object.class_definition.is_some() || !heap.key_equalities.is_empty() {
        return false;
    }
    match object.parent {
        None => true,
        Some(parent_id) => heap.get_fast(parent_id).is_some_and(|parent| {
            parent.parent.is_none()
                && matches!(
                    parent.builtin_type.as_deref(),
                    Some(TypeCheck::Collection(_))
                )
                && parent.collection_members.is_empty()
                && parent
                    .members
                    .iter()
                    .all(|m| m.local || m.method || m.entry || m.name == "default")
        }),
    }
}

/// Read declaration order without forcing member values. Overrides retain their
/// inherited position; properties and entries have distinct key namespaces.
pub fn ordered_members(
    heap: &Heap,
    receiver: ObjectId,
) -> Result<Vec<CollectionMember>, RuntimeError> {
    let first = heap.get(receiver)?;
    if first.collection_kind == CollectionKind::List && first.parent.is_none() {
        if let Some(elems) = &first.list_elements {
            return Ok((0..elems.len())
                .map(|idx| CollectionMember {
                    key: Value::Int((idx as i64).into()),
                    name: super::collection_transforms::format_list_element_name(
                        receiver.index,
                        idx,
                    ),
                    kind: CollectionMemberKind::Element,
                })
                .collect());
        }
        return Ok(first.collection_members.clone());
    }
    if first.collection_kind == CollectionKind::Map {
        if let Some(order) = first.map_key_order.as_deref() {
            return Ok(order.clone());
        }
        return Ok(first
            .map_index()
            .insertion_order()
            .iter()
            .map(|index| first.collection_members[*index].clone())
            .collect());
    }
    if !first.collection_members.is_empty() && is_single_level_leaf_collection(heap, first) {
        let has_default_prop = first
            .collection_members
            .iter()
            .any(|m| m.kind == CollectionMemberKind::Property && m.name == "default");
        if !has_default_prop {
            return Ok(first.collection_members.clone());
        }
        return Ok(first
            .collection_members
            .iter()
            .filter(|m| !(m.kind == CollectionMemberKind::Property && m.name == "default"))
            .cloned()
            .collect());
    }
    let typed = is_typed_class(heap, receiver)? || first.is_module;
    let mut chain = Vec::new();
    let mut next = Some(receiver);
    let mut visited = SmallObjectIdSet::new();
    while let Some(id) = next {
        if !visited.insert(id) {
            return Err(RuntimeError::new(ErrorKind::Cycle, "cyclic parent chain"));
        }
        chain.push(id);
        next = heap.get(id)?.parent;
    }
    let mut result: Vec<CollectionMember> = Vec::new();
    let mut prop_index: Option<crate::heap::FastMap<PklString, usize>> = None;
    let mut str_entry_index: Option<crate::heap::FastMap<PklString, usize>> = None;
    let mut int_entry_index: Option<crate::heap::FastMap<i64, usize>> = None;
    let mut has_other_entry = false;
    for id in chain.into_iter().rev() {
        let object = heap.get(id)?;
        let fallback;
        let members = if object.collection_members.is_empty() {
            fallback = object
                .members
                .iter()
                .filter(|m| !m.local && !m.method && !m.entry)
                .map(|m| CollectionMember {
                    key: Value::string(m.name.clone()),
                    name: m.name.clone(),
                    kind: CollectionMemberKind::Property,
                })
                .collect::<Vec<_>>();
            &fallback
        } else {
            &object.collection_members
        };
        if result.is_empty() && members.len() > 16 {
            let mut pi = crate::heap::FastMap::default();
            pi.reserve(members.len());
            let mut si = crate::heap::FastMap::default();
            si.reserve(members.len());
            prop_index = Some(pi);
            str_entry_index = Some(si);
            int_entry_index = Some(crate::heap::FastMap::default());
        }
        for member in members {
            if !typed && member.kind == CollectionMemberKind::Property && member.name == "default" {
                continue;
            }
            if prop_index.is_none() && result.len() >= 16 {
                let mut pi = crate::heap::FastMap::default();
                pi.reserve(result.len() * 2);
                let mut si = crate::heap::FastMap::default();
                si.reserve(result.len() * 2);
                let mut ii = crate::heap::FastMap::default();
                for (idx, existing) in result.iter().enumerate() {
                    if existing.kind == CollectionMemberKind::Property {
                        pi.insert(existing.name.clone(), idx);
                    } else {
                        match &existing.key {
                            Value::String(s) => {
                                si.insert(s.clone(), idx);
                            }
                            Value::Int(i) => {
                                ii.insert(i.get(), idx);
                            }
                            _ => has_other_entry = true,
                        }
                    }
                }
                prop_index = Some(pi);
                str_entry_index = Some(si);
                int_entry_index = Some(ii);
            }
            if let (Some(pi), Some(si), Some(ii)) = (
                prop_index.as_mut(),
                str_entry_index.as_mut(),
                int_entry_index.as_mut(),
            ) {
                if member.kind == CollectionMemberKind::Property {
                    if let Some(&idx) = pi.get(&member.name) {
                        result[idx] = member.clone();
                    } else {
                        let idx = result.len();
                        pi.insert(member.name.clone(), idx);
                        result.push(member.clone());
                    }
                    continue;
                }
                match &member.key {
                    Value::String(s) => {
                        if let Some(&idx) = si.get(s) {
                            result[idx] = member.clone();
                        } else {
                            let idx = result.len();
                            si.insert(s.clone(), idx);
                            result.push(member.clone());
                        }
                        continue;
                    }
                    Value::Int(i) => {
                        if let Some(&idx) = ii.get(&i.get()) {
                            result[idx] = member.clone();
                        } else {
                            let idx = result.len();
                            ii.insert(i.get(), idx);
                            result.push(member.clone());
                        }
                        continue;
                    }
                    _ => {
                        if has_other_entry
                            && let Some(old) = result.iter_mut().find(|old| {
                                old.kind != CollectionMemberKind::Property
                                    && same_member(heap, old, member)
                            })
                        {
                            *old = member.clone();
                        } else {
                            has_other_entry = true;
                            result.push(member.clone());
                        }
                        continue;
                    }
                }
            }
            if let Some(old) = result.iter_mut().find(|old| same_member(heap, old, member)) {
                *old = member.clone();
            } else {
                result.push(member.clone());
            }
        }
    }
    Ok(result)
}

#[inline]
fn environments_match(a: &Environment, b: &Environment) -> bool {
    a.locals.len() == b.locals.len()
        && a.scopes.len() == b.scopes.len()
        && a.restore_for_bindings == b.restore_for_bindings
        && a.executable_owner == b.executable_owner
        && a.code_owner == b.code_owner
        && a.type_arguments.is_empty()
        && b.type_arguments.is_empty()
        && a.type_argument_sources.is_empty()
        && b.type_argument_sources.is_empty()
        && a.custom_this.map(|s| s.root()) == b.custom_this.map(|s| s.root())
        && a.this_value == b.this_value
        && a.outer_value == b.outer_value
        && a.scopes.as_slice() == b.scopes.as_slice()
        && a.locals == b.locals
        && a.copy_locals.fast_eq(&b.copy_locals)
        && a.local_owners.fast_eq(&b.local_owners)
}

/// Recover an entry's authored key from the internal cache name.
pub(crate) fn cache_entry_key(
    heap: &Heap,
    receiver: ObjectId,
    key: &CacheKey,
) -> Result<Option<Value>, RuntimeError> {
    if !key.entry || key.owner.is_some() {
        return Ok(None);
    }
    let parsed_idx = key
        .name
        .as_str()
        .and_then(|s| s.strip_prefix('\0'))
        .and_then(|rest| rest.split_once(':'))
        .and_then(|(_, idx_str)| idx_str.parse::<usize>().ok());
    let mut owner = Some(receiver);
    let mut visited = [receiver; 8];
    let mut visited_len = 0usize;
    let mut overflow: Option<BTreeSet<ObjectId>> = None;
    while let Some(id) = owner {
        if let Some(set) = &mut overflow {
            if !set.insert(id) {
                return Err(RuntimeError::new(ErrorKind::Cycle, "cyclic parent chain"));
            }
        } else if visited[..visited_len].contains(&id) {
            return Err(RuntimeError::new(ErrorKind::Cycle, "cyclic parent chain"));
        } else if visited_len < visited.len() {
            visited[visited_len] = id;
            visited_len += 1;
        } else {
            let mut set: BTreeSet<ObjectId> = visited.into_iter().collect();
            if !set.insert(id) {
                return Err(RuntimeError::new(ErrorKind::Cycle, "cyclic parent chain"));
            }
            overflow = Some(set);
        }
        let object = heap.get(id)?;
        if let Some(idx) = parsed_idx {
            if let Some(elems) = &object.list_elements
                && idx < elems.len()
            {
                return Ok(Some(Value::Int((idx as i64).into())));
            }
            let limit = (idx + 1).min(object.collection_members.len());
            if let Some(member) = object.collection_members[..limit]
                .iter()
                .rev()
                .find(|member| {
                    member.name == key.name && member.kind != CollectionMemberKind::Property
                })
            {
                return Ok(Some(member.key.clone()));
            }
            if let Some(member) = object.collection_members[limit..].iter().find(|member| {
                member.name == key.name && member.kind != CollectionMemberKind::Property
            }) {
                return Ok(Some(member.key.clone()));
            }
        } else if let Some(member) = object
            .collection_members
            .iter()
            .find(|member| member.name == key.name && member.kind != CollectionMemberKind::Property)
        {
            return Ok(Some(member.key.clone()));
        }
        owner = object.parent;
    }
    Err(invalid("missing cache entry key"))
}

/// Recover the key physically retained by a completed cache slot.
pub(crate) fn cache_slot_entry_key(
    heap: &Heap,
    receiver: ObjectId,
    slot: &crate::heap::CacheSlot,
) -> Result<Option<Value>, RuntimeError> {
    if let Some(key) = &slot.entry_key {
        return Ok(Some(key.clone()));
    }
    cache_entry_key(heap, receiver, &slot.key)
}

/// Snapshot an already cached receiver value. Runtime rendering prepares key
/// equality before this lookup; this helper itself never evaluates a member.
pub(crate) fn cached_render_value(
    heap: &Heap,
    receiver: ObjectId,
    member: &CollectionMember,
) -> Result<Option<Value>, RuntimeError> {
    let entry = member.kind != CollectionMemberKind::Property;
    let object = heap.get(receiver)?;
    if entry {
        if let Some(elems) = &object.list_elements
            && let Value::Int(index) = &member.key
            && index.get() >= 0
        {
            return Ok(elems.get(index.get() as usize).cloned());
        }
        if object.collection_kind == CollectionKind::List
            && object.parent.is_none()
            && let Value::Int(index) = &member.key
            && index.get() >= 0
            && let Some(slot) = object.cache_slots.get(index.get() as usize)
            && slot.key.entry
            && slot.key.owner.is_none()
            && !slot.key.method
            && slot
                .entry_key
                .as_ref()
                .is_some_and(|key| matches!(key, Value::Int(stored) if stored.get() == index.get()))
        {
            return Ok(Some(slot.value.clone()));
        }
        if object.cache_index.len() == object.cache_slots.len() && object.cache_index.has_hash() {
            let hash = equality::java_hash_on_heap(heap, &member.key)?.unwrap_or(0);
            for index in object.cache_index.candidates(Some(hash)) {
                let slot = &object.cache_slots[index];
                if slot.key.entry
                    && slot.key.owner.is_none()
                    && !slot.key.method
                    && let Some(key) = cache_slot_entry_key(heap, receiver, slot)?
                    && same_key(heap, &member.key, &key)
                {
                    return Ok(Some(slot.value.clone()));
                }
            }
        } else {
            for slot in &object.cache_slots {
                if slot.key.entry
                    && slot.key.owner.is_none()
                    && !slot.key.method
                    && let Some(key) = cache_slot_entry_key(heap, receiver, slot)?
                    && same_key(heap, &member.key, &key)
                {
                    return Ok(Some(slot.value.clone()));
                }
            }
        }
    }
    if !entry && object.cache_map.is_empty() && !object.has_evaluating_keys() {
        for slot in object.cache_slots.iter().rev() {
            if !slot.key.entry
                && slot.key.owner.is_none()
                && !slot.key.method
                && slot.key.name == member.name
            {
                return Ok(Some(slot.value.clone()));
            }
        }
        return Ok(None);
    }
    Ok(object
        .cache_get_completed(&CacheKey {
            entry,
            owner: None,
            name: member.name.clone(),
            method: false,
        })
        .cloned())
}

pub(super) fn forcing_members(
    heap: &Heap,
    receiver: ObjectId,
) -> Result<Vec<CollectionMember>, RuntimeError> {
    let receiver_obj = heap.get(receiver)?;
    if receiver_obj.collection_kind == CollectionKind::Map
        || (receiver_obj.collection_kind == CollectionKind::List
            && receiver_obj.list_elements.is_some())
    {
        return ordered_members(heap, receiver);
    }
    if !receiver_obj.collection_members.is_empty()
        && is_single_level_leaf_collection(heap, receiver_obj)
        && receiver_obj.collection_members.len() == receiver_obj.members.len()
        && !receiver_obj
            .collection_members
            .iter()
            .any(|m| m.kind == CollectionMemberKind::Property && m.name == "default")
    {
        return Ok(receiver_obj.collection_members.clone());
    }
    let effective = ordered_members(heap, receiver)?;
    if effective.is_empty() {
        return Ok(effective);
    }
    let lookup: Option<crate::heap::FastMap<(&PklString, bool), usize>> = if effective.len() > 12 {
        let mut map = crate::heap::FastMap::default();
        map.reserve(effective.len());
        for (idx, d) in effective.iter().enumerate() {
            map.entry((&d.name, d.kind != CollectionMemberKind::Property))
                .or_insert(idx);
        }
        Some(map)
    } else {
        None
    };
    let mut emitted = vec![false; effective.len()];
    let mut result = Vec::with_capacity(effective.len());
    let mut next = Some(receiver);
    let mut visited = SmallObjectIdSet::new();
    while let Some(id) = next {
        if !visited.insert(id) {
            return Err(RuntimeError::new(ErrorKind::Cycle, "cyclic parent chain"));
        }
        let object = heap.get(id)?;
        for member in &object.members {
            if member.local || member.method {
                continue;
            }
            let idx = if let Some(map) = &lookup {
                map.get(&(&member.name, member.entry)).copied()
            } else {
                effective.iter().position(|d| {
                    d.name == member.name
                        && (d.kind != CollectionMemberKind::Property) == member.entry
                })
            };
            if let Some(idx) = idx
                && !emitted[idx]
            {
                emitted[idx] = true;
                result.push(effective[idx].clone());
            }
        }
        next = object.parent;
    }
    Ok(result)
}

/// Class-hidden names stay hidden even when an override lacks the modifier.
pub(crate) fn hidden_property(
    heap: &Heap,
    receiver: ObjectId,
    name: &PklString,
    entry: bool,
) -> Result<bool, RuntimeError> {
    if entry {
        return Ok(false);
    }
    let is_default = name == "default";
    // Dynamic, Listing and Mapping inherit the hidden standard-library default.
    if is_default && !is_typed_class(heap, receiver)? && !heap.get(receiver)?.is_module {
        return Ok(true);
    }
    let mut owner = Some(receiver);
    let mut visited = [receiver; 8];
    let mut depth = 0usize;
    let mut overflow: Option<BTreeSet<ObjectId>> = None;
    while let Some(id) = owner {
        if let Some(set) = &mut overflow {
            if !set.insert(id) {
                return Err(RuntimeError::new(ErrorKind::Cycle, "cyclic parent chain"));
            }
        } else if visited[..depth].contains(&id) {
            return Err(RuntimeError::new(ErrorKind::Cycle, "cyclic parent chain"));
        } else if depth < visited.len() {
            visited[depth] = id;
            depth += 1;
        } else {
            let mut set: BTreeSet<ObjectId> = visited.into_iter().collect();
            set.insert(id);
            overflow = Some(set);
        }
        let object = heap.get(id)?;
        // Builtin collection prototypes (Dynamic/Listing/Mapping) only declare hidden `default`,
        // and Typed/Object/Any declare no hidden properties at all.
        if !is_default
            && (matches!(
                object.builtin_type.as_deref(),
                Some(TypeCheck::Collection(_))
            ) || object.class_definition.as_ref().is_some_and(|c| {
                c.module.as_ref().and_then(PklString::as_str) == Some("pkl.base")
                    && matches!(c.name.as_str(), Some("Typed" | "Object" | "Any"))
            }))
        {
            return Ok(false);
        }
        if object
            .members
            .iter()
            .any(|member| member.hidden() && !member.entry && !member.local && &member.name == name)
        {
            return Ok(true);
        }
        owner = object.parent;
    }
    Ok(false)
}

/// Returns true if `owner` or any non-builtin ancestor declares any `hidden` non-entry property.
pub(crate) fn owner_has_explicit_hidden(
    heap: &Heap,
    receiver: ObjectId,
) -> Result<bool, RuntimeError> {
    let mut owner = Some(receiver);
    let mut visited = [receiver; 8];
    let mut depth = 0usize;
    let mut overflow: Option<BTreeSet<ObjectId>> = None;
    while let Some(id) = owner {
        if let Some(set) = &mut overflow {
            if !set.insert(id) {
                return Err(RuntimeError::new(ErrorKind::Cycle, "cyclic parent chain"));
            }
        } else if visited[..depth].contains(&id) {
            return Err(RuntimeError::new(ErrorKind::Cycle, "cyclic parent chain"));
        } else if depth < visited.len() {
            visited[depth] = id;
            depth += 1;
        } else {
            let mut set: BTreeSet<ObjectId> = visited.into_iter().collect();
            set.insert(id);
            overflow = Some(set);
        }
        let object = heap.get(id)?;
        if matches!(
            object.builtin_type.as_deref(),
            Some(TypeCheck::Collection(_))
        ) || object.class_definition.as_ref().is_some_and(|c| {
            c.module.as_ref().and_then(PklString::as_str) == Some("pkl.base")
                && matches!(c.name.as_str(), Some("Typed" | "Object" | "Any"))
        }) {
            return Ok(false);
        }
        if object
            .members
            .iter()
            .any(|member| member.hidden() && !member.entry && !member.local)
        {
            return Ok(true);
        }
        owner = object.parent;
    }
    Ok(false)
}

#[derive(Clone)]
pub(crate) struct RenderMember {
    pub descriptor: CollectionMember,
    pub skip: bool,
    pub code: Option<usize>,
}

/// VmObject.iterateMembers visits every nonlocal declaration parent first.
/// Deduplication happens later, after the renderer hashes each visited key.
pub(crate) fn render_members(
    heap: &Heap,
    receiver: ObjectId,
) -> Result<Vec<RenderMember>, RuntimeError> {
    let first = heap.get(receiver)?;
    if !first.members.is_empty()
        && is_single_level_leaf_collection(heap, first)
        && first.collection_members.len() == first.members.len()
    {
        let mut result = Vec::with_capacity(first.members.len());
        for (idx, member) in first.members.iter().enumerate() {
            if member.local || member.method {
                continue;
            }
            let descriptor = {
                let direct = &first.collection_members[idx];
                if direct.name == member.name
                    && (direct.kind != CollectionMemberKind::Property) == member.entry
                {
                    direct.clone()
                } else if let Some(found) = first.collection_members.iter().find(|d| {
                    d.name == member.name
                        && (d.kind != CollectionMemberKind::Property) == member.entry
                }) {
                    found.clone()
                } else if member.entry {
                    continue;
                } else {
                    CollectionMember {
                        key: Value::string(member.name.clone()),
                        name: member.name.clone(),
                        kind: CollectionMemberKind::Property,
                    }
                }
            };
            result.push(RenderMember {
                code: Some(member.code),
                descriptor,
                skip: member.hidden()
                    || member.external()
                    || member.definition()
                    || (!member.entry && member.name == "default"),
            });
        }
        return Ok(result);
    }
    let mut chain = Vec::new();
    let mut owner = Some(receiver);
    let mut seen = SmallObjectIdSet::new();
    while let Some(id) = owner {
        if !seen.insert(id) {
            return Err(RuntimeError::new(ErrorKind::Cycle, "cyclic parent chain"));
        }
        chain.push(id);
        owner = heap.get(id)?.parent;
    }
    let typed = is_typed_class(heap, receiver)? || first.is_module;
    let mut result = Vec::new();
    for id in chain.into_iter().rev() {
        let object = heap.get(id)?;
        for (idx, member) in object.members.iter().enumerate() {
            if member.local || member.method {
                continue;
            }
            let descriptor = if let Some(direct) = object.collection_members.get(idx)
                && direct.name == member.name
                && (direct.kind != CollectionMemberKind::Property) == member.entry
            {
                direct.clone()
            } else if let Some(descriptor) = object.collection_members.iter().find(|descriptor| {
                descriptor.name == member.name
                    && (descriptor.kind != CollectionMemberKind::Property) == member.entry
            }) {
                descriptor.clone()
            } else if member.entry {
                continue;
            } else {
                CollectionMember {
                    key: Value::string(member.name.clone()),
                    name: member.name.clone(),
                    kind: CollectionMemberKind::Property,
                }
            };
            result.push(RenderMember {
                code: Some(member.code),
                descriptor,
                skip: member.hidden()
                    || member.external()
                    || member.definition()
                    || (!typed && !member.entry && member.name == "default"),
            });
        }
    }
    Ok(result)
}

/// Predicates visit declarations child first and test each value immediately.
/// Their cache belongs to the declaration owner, even when the member executes
/// against a more derived source receiver.
fn predicate_members(
    heap: &Heap,
    source: ObjectId,
) -> Result<Vec<(ObjectId, CollectionMember)>, RuntimeError> {
    let mut result = Vec::new();
    let mut seen = Vec::new();
    let mut next = Some(source);
    let mut visited = SmallObjectIdSet::new();
    while let Some(owner) = next {
        if !visited.insert(owner) {
            return Err(RuntimeError::new(ErrorKind::Cycle, "cyclic parent chain"));
        }
        let object = heap.get(owner)?;
        for descriptor in &object.collection_members {
            if seen.iter().any(|old| same_member(heap, descriptor, old)) {
                continue;
            }
            seen.push(descriptor.clone());
            if descriptor.kind != CollectionMemberKind::Property {
                result.push((owner, descriptor.clone()));
            }
        }
        next = object.parent;
    }
    Ok(result)
}

pub(super) fn external_amendment_error(name: impl Into<PklString>) -> RuntimeError {
    let name = name.into();
    let mut error = RuntimeError::new(
        ErrorKind::Type,
        PklString::from("Cannot instantiate, or amend an instance of, external class `")
            .concat(&name)
            .concat(&"`.".into()),
    );
    error.hint = Some("To create an instance of an external class, use its literal syntax or constructor method.\nExamples:\n* `123` uses literal syntax to create an instance of class `Int`.\n* `Pair(1, 2)` uses the `Pair()` constructor method to create an instance of class `Pair`.".into());
    error
}

impl Vm {
    fn set_candidates(
        &self,
        id: ObjectId,
        value: &Value,
        assoc: bool,
    ) -> Result<Vec<Value>, RuntimeError> {
        let object = self.heap.get(id)?;
        let hash = self.java_hash(value)?;
        let indices = if assoc {
            object.map_index().assoc_candidates(hash)
        } else {
            object.map_index().candidates(hash)
        };
        Ok(indices
            .into_iter()
            .map(|index| object.set_entries[index].clone())
            .collect())
    }

    pub(super) fn set_contains(&self, id: ObjectId, value: &Value) -> Result<bool, RuntimeError> {
        // Paguro's empty root returns before hashing even a primitive probe.
        let object = self.heap.get(id)?;
        if object.set_entries.is_empty() {
            return Ok(false);
        }
        let hash = self.java_hash(value)?;
        let indices = object.map_index().candidates(hash);
        Ok(indices
            .into_iter()
            .any(|index| same_key(&self.heap, value, &object.set_entries[index])))
    }

    pub(super) fn prepare_set_lookup(
        &mut self,
        id: ObjectId,
        value: &Value,
    ) -> Result<bool, RuntimeError> {
        if self.heap.get(id)?.set_entries.is_empty() {
            return Ok(false);
        }
        if !matches!(value, Value::Object(_)) {
            return Ok(false);
        }
        if self.prepare_keys(vec![value.clone()])? {
            return Ok(true);
        }
        self.search_key(value, self.set_candidates(id, value, false)?)
    }

    pub(super) fn invoke_set_contains(
        &mut self,
        callable: ObjectId,
        native: reference::NativeFunction,
        arguments: Vec<Value>,
    ) -> Result<(), RuntimeError> {
        if arguments.len() != 1 {
            let source = crate::stdlib_sources::base();
            let declaration = source
                .text
                .find("external class Set<")
                .expect("pinned Set class");
            let fragment = "external function contains(element: Any): Boolean";
            let start = declaration
                + source
                    .text
                    .entire()
                    .range(declaration..)
                    .find(fragment)
                    .expect("pinned Set.contains");
            let span = Span::new(start, start + fragment.len());
            let mut error = RuntimeError::new(
                ErrorKind::Type,
                format!("Expected 1 function arguments but got {}.", arguments.len()),
            );
            error.frames.push(RuntimeFrame {
                anonymous: false,
                span,
                member: "#Set.contains".into(),
                source: Some(source),
            });
            return Err(error);
        }
        let id = native.receiver.as_object()?;
        if self.prepare_set_lookup(id, &arguments[0])? {
            self.frames
                .last_mut()
                .expect("active frame")
                .equality_run
                .as_mut()
                .expect("Set lookup")
                .hide_callsite = false;
            self.stack.push(Value::Function(callable));
            self.stack.extend(arguments);
            return Ok(());
        }
        self.stack
            .push(Value::Bool(self.set_contains(id, &arguments[0])?));
        Ok(())
    }

    fn prepare_set_element(&mut self) -> Result<(), RuntimeError> {
        let value = self.pop()?;
        let id = self.builder()?;
        if self.prepare_set_lookup(id, &value)? {
            self.stack.push(value);
            return Ok(());
        }
        let present = self.set_contains(id, &value)?;
        self.stack.extend([value, Value::Bool(present)]);
        Ok(())
    }

    pub(super) fn add_set_element(&mut self) -> Result<(), RuntimeError> {
        let present = self.pop()?;
        let value = self.pop()?;
        if present == Value::Bool(true) {
            return Ok(());
        }
        if self.put_set_key(&value)? {
            self.stack.extend([value, present]);
            return Ok(());
        }
        let target = self.builder()?;
        let fast_set = self
            .heap
            .get_fast(target)
            .is_some_and(|obj| obj.collection_kind == CollectionKind::Set && obj.parent.is_none());
        if fast_set {
            let (index, member_count) = {
                let obj = self.heap.get(target)?;
                (obj.length, obj.members.len())
            };
            let name = PklString::from(format!("\0{}:{}", target.index, member_count));
            let descriptor = CollectionMember {
                key: Value::Int((index as i64).into()),
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
            let need_index = {
                let object = self.heap.get_mut(target)?;
                object.collection_members.push(descriptor);
                object.members.push(member);
                object.length += 1;
                object
                    .member_values
                    .insert(cache_key.clone(), value.clone());
                object.members.len() > 4
            };
            self.complete_cache(target, cache_key, value)?;
            if need_index {
                self.update_declaration_index(target)?;
            }
            return Ok(());
        }
        self.stack.push(value);
        self.append_strict_element()
    }

    /// Return true only when hashing/equality must retry this instruction.
    pub(super) fn put_set_key(&mut self, value: &Value) -> Result<bool, RuntimeError> {
        let id = self.builder()?;
        if matches!(value, Value::Object(_))
            && (self.prepare_keys(vec![value.clone()])?
                || self.search_key(value, self.set_candidates(id, value, true)?)?)
        {
            return Ok(true);
        }
        let value_hash = self.java_hash(value)?;
        let replacing = {
            let object = self.heap.get(id)?;
            object
                .map_index()
                .assoc_candidates(value_hash)
                .into_iter()
                .any(|index| same_key(&self.heap, value, &object.set_entries[index]))
        };
        if !replacing {
            let old: Vec<Value> = {
                let object = self.heap.get(id)?;
                object
                    .map_index()
                    .rehash(value_hash)
                    .into_iter()
                    .filter_map(|index| {
                        let entry = &object.set_entries[index];
                        matches!(entry, Value::Object(_)).then(|| entry.clone())
                    })
                    .collect()
            };
            if !old.is_empty() && self.prepare_keys(old)? {
                return Ok(true);
            }
            let mut map_index = self.heap.get_mut(id)?.map_index.take().unwrap_or_default();
            map_index.insert_lazy(value_hash, true, |slot| {
                self.java_hash(&self.heap.get(id)?.set_entries[slot])
            })?;
            let object = self.heap.get_mut(id)?;
            object.map_index = Some(map_index);
            object.set_entries.push(value.clone());
        }
        Ok(false)
    }

    pub(super) fn set_hash_elements(&self, id: ObjectId) -> Result<Vec<Value>, RuntimeError> {
        let object = self.heap.get(id)?;
        Ok(object
            .map_index()
            .ordered()
            .into_iter()
            .map(|index| object.set_entries[index].clone())
            .collect())
    }

    fn map_candidates(&self, id: ObjectId, key: &Value) -> Result<Vec<Value>, RuntimeError> {
        let object = self.heap.get(id)?;
        Ok(object
            .map_index()
            .candidates(self.java_hash(key)?)
            .into_iter()
            .map(|index| object.collection_members[index].key.clone())
            .collect())
    }

    /// Paguro skips hashing only when its root is empty. Every nonempty lookup
    /// hashes the probe before comparing candidates, including small Maps.
    pub(super) fn prepare_map_lookup(
        &mut self,
        id: ObjectId,
        key: &Value,
    ) -> Result<bool, RuntimeError> {
        if self.heap.get(id)?.map_index().len() == 0 {
            return Ok(false);
        }
        if !matches!(key, Value::Object(_)) {
            return Ok(false);
        }
        if self.prepare_keys(vec![key.clone()])? {
            return Ok(true);
        }
        self.search_key(key, self.map_candidates(id, key)?)
    }

    pub(super) fn map_value(
        &self,
        id: ObjectId,
        key: &Value,
    ) -> Result<Option<Value>, RuntimeError> {
        let object = self.heap.get(id)?;
        if object.map_index().len() == 0 {
            return Ok(None);
        }
        let member = object
            .map_index()
            .candidates(self.java_hash(key)?)
            .into_iter()
            .map(|slot| &object.collection_members[slot])
            .find(|member| same_key(&self.heap, key, &member.key));
        Ok(member.and_then(|member| {
            object
                .member_values
                .get(&CacheKey {
                    owner: None,
                    name: member.name.clone(),
                    method: false,
                    entry: true,
                })
                .cloned()
        }))
    }

    /// Construct the diagnostic only after the shared hash/equality continuation
    /// has visited all candidate keys. Member values are never forced here.
    pub(super) fn key_suggestions_error(
        &self,
        key: &Value,
        candidates: Vec<PklString>,
    ) -> RuntimeError {
        let rendered =
            crate::diagnostic_values::render(key, &self.heap, &self.program.module_name, 80)
                .unwrap_or_else(|_| display_value(key));
        let mut error = RuntimeError::new(
            ErrorKind::MissingMember,
            PklString::join(&["Cannot find key `".into(), rendered, "`.".into()], ""),
        );
        if let Value::String(query) = key {
            let suggestions = crate::diagnostic::key_suggestions(query, candidates);
            if !suggestions.is_empty() {
                let mut lines = vec![PklString::from("Did you mean any of the following?")];
                lines.extend(suggestions.into_iter().map(|key| key.quoted()));
                error.hint = Some(PklString::join(&lines, "\n"));
            }
        }
        error
    }

    pub(super) fn prepare_map_association(
        &mut self,
        id: ObjectId,
        key: &Value,
    ) -> Result<bool, RuntimeError> {
        if matches!(key, Value::Object(_)) {
            if self.prepare_keys(vec![key.clone()])? {
                return Ok(true);
            }
            let object = self.heap.get(id)?;
            let candidates: Vec<_> = object
                .map_index()
                .assoc_candidates(self.java_hash(key)?)
                .into_iter()
                .map(|index| object.collection_members[index].key.clone())
                .collect();
            if self.search_key(key, candidates.clone())? {
                return Ok(true);
            }
            if !candidates
                .iter()
                .any(|candidate| same_key(&self.heap, key, candidate))
            {
                let object = self.heap.get(id)?;
                let keys: Vec<_> = object
                    .map_index()
                    .rehash(self.java_hash(key)?)
                    .into_iter()
                    .filter_map(|index| {
                        let k = &object.collection_members[index].key;
                        matches!(k, Value::Object(_)).then(|| k.clone())
                    })
                    .collect();
                if !keys.is_empty() && self.prepare_keys(keys)? {
                    return Ok(true);
                }
            }
            return Ok(false);
        }
        let key_hash = self.java_hash(key)?;
        let keys: Vec<Value> = {
            let object = self.heap.get(id)?;
            let found = object
                .map_index()
                .assoc_candidates(key_hash)
                .into_iter()
                .any(|index| same_key(&self.heap, key, &object.collection_members[index].key));
            if found {
                return Ok(false);
            }
            object
                .map_index()
                .rehash(key_hash)
                .into_iter()
                .filter_map(|index| {
                    let k = &object.collection_members[index].key;
                    matches!(k, Value::Object(_)).then(|| k.clone())
                })
                .collect()
        };
        if !keys.is_empty() && self.prepare_keys(keys)? {
            return Ok(true);
        }
        Ok(false)
    }

    #[cfg_attr(target_arch = "wasm32", inline(never))]
    pub(super) fn insert_map_entry_ordered(
        &mut self,
        id: ObjectId,
        key: Value,
        value: Value,
        append_order: Option<bool>,
    ) -> Result<(), RuntimeError> {
        let key_hash = self.java_hash(&key)?;
        let object = self.heap.get(id)?;
        let candidates = object.map_index().assoc_candidates(key_hash);
        let replacing = candidates
            .into_iter()
            .find(|slot| same_key(&self.heap, &key, &object.collection_members[*slot].key));
        let append_order = append_order.unwrap_or(replacing.is_none());
        let preserve_implicit_order =
            object.map_key_order.is_none() && replacing.is_none() && append_order;
        let name = replacing
            .map(|index| object.collection_members[index].name.clone())
            .unwrap_or_else(|| {
                format_entry_internal_name(id.index, object.collection_members.len())
            });
        let explicit_order = if preserve_implicit_order {
            None
        } else {
            let mut order = ordered_members(&self.heap, id)?;
            if append_order {
                order.push(CollectionMember {
                    key: key.clone(),
                    name: name.clone(),
                    kind: CollectionMemberKind::Entry,
                });
            }
            Some(order)
        };
        let key_for_cache = key.clone();
        let object = if replacing.is_none() {
            let mut map_index = self.heap.get_mut(id)?.map_index.take().unwrap_or_default();
            map_index.insert_lazy(key_hash, append_order, |slot| {
                self.java_hash(&self.heap.get(id)?.collection_members[slot].key)
            })?;
            let object = self.heap.get_mut(id)?;
            object.map_index = Some(map_index);
            if matches!(key, Value::Object(_)) {
                object.has_object_collection_key = true;
            }
            object.collection_members.push(CollectionMember {
                key,
                name: name.clone(),
                kind: CollectionMemberKind::Entry,
            });
            object.members.push(Member {
                flags: 0,
                name: name.clone(),
                code: usize::MAX,
                local: false,
                method: false,
                entry: true,
                ty: None,
                type_span: None,
                constant: false,
                fixed: false,
            });
            object.length = object.map_index().insertion_order().len();
            object
        } else {
            self.heap.get_mut(id)?
        };
        if let Some(order) = explicit_order {
            object.length = order.len();
            object.map_key_order = Some(Box::new(order));
        }
        let cache = CacheKey {
            owner: None,
            name,
            method: false,
            entry: true,
        };
        object.member_values.insert(cache.clone(), value.clone());
        object.complete_cache_with_key(cache, Some(key_for_cache), value);
        Ok(())
    }

    pub(super) fn invoke_map_lookup(
        &mut self,
        callable: ObjectId,
        native: reference::NativeFunction,
        arguments: Vec<Value>,
    ) -> Result<(), RuntimeError> {
        if arguments.len() != 1 {
            return Err(RuntimeError::new(
                ErrorKind::Type,
                format!("Expected 1 function arguments but got {}.", arguments.len()),
            ));
        }
        let id = native.receiver.as_object()?;
        let key = &arguments[0];
        if self.prepare_map_lookup(id, key)? {
            self.frames
                .last_mut()
                .expect("active frame")
                .equality_run
                .as_mut()
                .expect("Map lookup")
                .hide_callsite = false;
            self.stack.push(Value::Function(callable));
            self.stack.extend(arguments);
            return Ok(());
        }
        let value = self.map_value(id, key)?;
        self.stack.push(if native.name.as_ref() == "containsKey" {
            Value::Bool(value.is_some())
        } else {
            value.unwrap_or(Value::Null)
        });
        Ok(())
    }

    pub(super) fn map_hash_members(
        &self,
        id: ObjectId,
    ) -> Result<Vec<CollectionMember>, RuntimeError> {
        let object = self.heap.get(id)?;
        Ok(object
            .map_index()
            .ordered()
            .into_iter()
            .map(|index| object.collection_members[index].clone())
            .collect())
    }

    fn prepare_map_entry(&mut self) -> Result<(), RuntimeError> {
        let value = self.pop()?;
        let key = self.pop()?;
        let id = self.builder()?;
        if self.prepare_map_lookup(id, &key)? {
            self.stack.extend([key, value]);
            return Ok(());
        }
        let append_order = self.map_value(id, &key)?.is_none();
        self.stack.extend([key, value, Value::Bool(append_order)]);
        Ok(())
    }

    fn add_map_entry(&mut self) -> Result<(), RuntimeError> {
        let Value::Bool(append_order) = self.pop()? else {
            return Err(invalid("Map builder contains result"));
        };
        let value = self.pop()?;
        let key = self.pop()?;
        let id = self.builder()?;
        // VmMap.Builder.add repeats hashing/equality for assoc after contains.
        // A distinct instruction owns its own prepared-hash retry state.
        if self.prepare_map_association(id, &key)? {
            self.stack.extend([key, value, Value::Bool(append_order)]);
            return Ok(());
        }
        self.insert_map_entry_ordered(id, key, value, Some(append_order))
    }

    fn next_map_entry(&mut self, iterator: ObjectId, end: usize) -> Result<(), RuntimeError> {
        let state = self
            .heap
            .get(iterator)?
            .iterator
            .as_ref()
            .expect("Map iterator");
        let source = state.source;
        let Some(member) = state.members.get(state.position).cloned() else {
            return self.jump(end);
        };
        if self.prepare_map_lookup(source, &member.key)? {
            return Ok(());
        }
        let value = self
            .map_value(source, &member.key)?
            .ok_or_else(|| invalid("Map iterator key is no longer reachable"))?;
        self.heap
            .get_mut(iterator)?
            .iterator
            .as_mut()
            .expect("Map iterator")
            .position += 1;
        self.stack.extend([member.key, value]);
        Ok(())
    }

    #[cfg_attr(target_arch = "wasm32", inline(never))]
    fn check_strict_spread(&self) -> Result<(), RuntimeError> {
        let value = self
            .stack
            .last()
            .ok_or_else(|| invalid("missing spread source"))?;
        let (source_label, allowed, hint) = match value {
            Value::Bytes(_) => (
                "Bytes",
                CollectionKind::Listing,
                "`Bytes` can only be spread into objects of type `Dynamic` and `Listing`.",
            ),
            Value::IntSeq(_) => (
                "IntSeq",
                CollectionKind::Listing,
                "`IntSeq` can only be spread into objects of type `Dynamic` and `Listing`.",
            ),
            Value::Object(source) => match self.heap.get(*source)?.collection_kind {
                CollectionKind::Map => (
                    "Map",
                    CollectionKind::Mapping,
                    "`Map` can only be spread into objects of type `Dynamic` and `Mapping`.",
                ),
                CollectionKind::List => (
                    "List",
                    CollectionKind::Listing,
                    "`List` and `Set` can only be spread into objects of type `Dynamic` and `Listing`.",
                ),
                CollectionKind::Set => (
                    "Set",
                    CollectionKind::Listing,
                    "`List` and `Set` can only be spread into objects of type `Dynamic` and `Listing`.",
                ),
                _ => return Ok(()),
            },
            _ => return Ok(()),
        };
        let target = self.builder()?;
        let kind = self.heap.get(target)?.collection_kind;
        if (kind == CollectionKind::Dynamic || kind == allowed)
            && !is_typed_class(&self.heap, target)?
        {
            return Ok(());
        }
        let class = typed_class_ref(&self.heap, target)?;
        let label = class.map_or_else(
            || PklString::from(format!("{kind:?}")),
            |class| {
                class
                    .module
                    .as_ref()
                    .unwrap_or(&self.program.module_name)
                    .concat(&"#".into())
                    .concat(&class.name)
            },
        );
        let headline = PklString::from(format!(
            "Cannot spread value of type `{source_label}` into object of type `"
        ))
        .concat(&label)
        .concat(&"`.".into());
        let rendered =
            crate::diagnostic_values::render(value, &self.heap, &self.program.module_name, 80)?;
        let mut error =
            RuntimeError::with_program_values(ErrorKind::Type, headline, &[("Value", rendered)]);
        error.hint = Some(hint.into());
        Err(error)
    }

    pub(super) fn builder(&self) -> Result<ObjectId, RuntimeError> {
        self.frames
            .last()
            .and_then(|f| f.builders.last())
            .copied()
            .ok_or_else(|| invalid("missing collection builder"))
    }

    #[cfg_attr(target_arch = "wasm32", inline(never))]
    pub(super) fn collection_builtin(
        &mut self,
        receiver: ObjectId,
        name: &PklString,
    ) -> Result<bool, RuntimeError> {
        let Some(name) = name.as_str() else {
            return Ok(false);
        };
        let object = self.heap.get(receiver)?;
        if let Some(pair) = &object.pair
            && matches!(name, "first" | "key" | "second" | "value")
        {
            let value = pair[usize::from(matches!(name, "second" | "value"))].clone();
            self.stack.push(value);
            return Ok(true);
        }
        if object.collection_kind == CollectionKind::Dynamic
            || object.is_module
            || !(matches!(name, "length" | "isEmpty" | "isNotEmpty")
                || (name == "lastIndex"
                    && matches!(
                        object.collection_kind,
                        CollectionKind::List | CollectionKind::Listing
                    )))
        {
            return Ok(false);
        }
        if object.collection_kind == CollectionKind::Mapping
            && object.mapping_length.is_none()
            && matches!(name, "length" | "isEmpty" | "isNotEmpty")
        {
            let mut chain = Vec::new();
            let mut owner = Some(receiver);
            while let Some(id) = owner {
                chain.push(id);
                owner = self.heap.get(id)?.parent;
            }
            let mut keys = Vec::new();
            for id in chain.into_iter().rev() {
                keys.extend(
                    self.heap
                        .get(id)?
                        .collection_members
                        .iter()
                        .filter(|m| m.kind == CollectionMemberKind::Entry)
                        .map(|m| m.key.clone()),
                );
            }
            self.count_mapping_keys(receiver, keys);
            {
                self.frames
                    .last_mut()
                    .expect("active frame")
                    .equality_run
                    .as_mut()
                    .expect("key hash")
                    .hide_callsite = false;
                let frame = self.frames.last().expect("active frame");
                if matches!(
                    self.program
                        .blocks
                        .get(frame.block)
                        .and_then(|b| b.get(frame.pc - 1)),
                    Some(
                        Instruction::GetMember(_)
                            | Instruction::GetConstMember(_)
                            | Instruction::GetImplicitCustomProperty(_),
                    )
                ) {
                    self.stack.push(Value::Object(receiver));
                }
                return Ok(true);
            }
        }
        let object = self.heap.get(receiver)?;
        let length = if object.collection_kind == CollectionKind::Mapping {
            object.mapping_length.expect("completed Mapping length")
        } else {
            object.length
        };
        let value = match name {
            "length" => Value::Int((length as i64).into()),
            "lastIndex" => Value::Int((length as i64 - 1).into()),
            "isEmpty" => Value::Bool(length == 0),
            "isNotEmpty" => Value::Bool(length != 0),
            _ => return Ok(false),
        };
        self.stack.push(value);
        Ok(true)
    }

    /// Stable-hash declaration maps use the pinned bucket chain order. Maps
    /// containing JVM identity-dependent keys retain the explicit linear fallback.
    pub(super) fn declaration_candidates(
        &self,
        owner: ObjectId,
        key: &Value,
        inherited: bool,
    ) -> Result<Vec<Value>, RuntimeError> {
        let mut result = Vec::new();
        let mut current = Some(owner);
        while let Some(id) = current {
            let object = self.heap.get(id)?;
            let members = &object.collection_members;
            let indices = if let (Some(index), Some(hash)) =
                (&object.declaration_index, self.java_hash(key)?)
            {
                index.candidates(Some(hash))
            } else {
                (0..members.len()).collect()
            };
            result.extend(indices.into_iter().filter_map(|index| {
                let member = &members[index];
                (member.kind != CollectionMemberKind::Property).then(|| member.key.clone())
            }));
            current = if inherited { object.parent } else { None };
        }
        Ok(result)
    }

    fn update_declaration_index(&mut self, id: ObjectId) -> Result<(), RuntimeError> {
        let object = self.heap.get(id)?;
        if object.members.len() <= 4 {
            return Ok(());
        }
        if object.collection_members.len() != object.members.len() {
            let object = self.heap.get_mut(id)?;
            object.declaration_index = None;
            object.declaration_hashes.clear();
            return Ok(());
        }
        let start = if object.declaration_index.is_some()
            && object.declaration_hashes.len() <= object.collection_members.len()
        {
            object.declaration_hashes.len()
        } else {
            0
        };
        let remaining = object.collection_members.len().saturating_sub(start);
        if remaining == 1 {
            let member = &object.collection_members[start];
            if member.kind == CollectionMemberKind::Property {
                let object = self.heap.get_mut(id)?;
                object.declaration_index = None;
                object.declaration_hashes.clear();
                return Ok(());
            }
            let Some(hash) = self.java_hash(&member.key)? else {
                let object = self.heap.get_mut(id)?;
                object.declaration_index = None;
                object.declaration_hashes.clear();
                return Ok(());
            };
            let object = self.heap.get_mut(id)?;
            let index = object.declaration_index.get_or_insert_with(|| {
                Box::new(crate::economic_map::EconomicMapIndex::with_capacity(0))
            });
            if index.len() < object.collection_members.len() {
                if index.grow_before_append() {
                    for old in index.start_rehash() {
                        let old_hash = object.declaration_hashes[old];
                        index.put_hash(old, old_hash, false);
                    }
                }
                let slot = index.push();
                object.declaration_hashes.push(hash);
                if (!index.has_hash() && index.len() > 4)
                    || (index.has_hash() && index.put_hash(slot, hash, index.should_rehash()))
                {
                    for old in index.start_rehash() {
                        let old_hash = object.declaration_hashes[old];
                        index.put_hash(old, old_hash, false);
                    }
                }
            }
            return Ok(());
        }
        let mut new_hashes = Vec::with_capacity(remaining);
        for member in &object.collection_members[start..] {
            if member.kind == CollectionMemberKind::Property {
                let object = self.heap.get_mut(id)?;
                object.declaration_index = None;
                object.declaration_hashes.clear();
                return Ok(());
            }
            let Some(hash) = self.java_hash(&member.key)? else {
                let object = self.heap.get_mut(id)?;
                object.declaration_index = None;
                object.declaration_hashes.clear();
                return Ok(());
            };
            new_hashes.push(hash);
        }
        let object = self.heap.get_mut(id)?;
        if start == 0 {
            object.declaration_hashes.clear();
        }
        let index = object.declaration_index.get_or_insert_with(|| {
            Box::new(crate::economic_map::EconomicMapIndex::with_capacity(0))
        });
        let mut new_iter = new_hashes.into_iter();
        while index.len() < object.collection_members.len() {
            if index.grow_before_append() {
                for old in index.start_rehash() {
                    let hash = object.declaration_hashes[old];
                    index.put_hash(old, hash, false);
                }
            }
            let slot = index.push();
            let hash = new_iter.next().expect("incremental declaration hash");
            object.declaration_hashes.push(hash);
            if (!index.has_hash() && index.len() > 4)
                || (index.has_hash() && index.put_hash(slot, hash, index.should_rehash()))
            {
                for old in index.start_rehash() {
                    let hash = object.declaration_hashes[old];
                    index.put_hash(old, hash, false);
                }
            }
        }
        Ok(())
    }

    pub(super) fn prepare_insertion(
        &mut self,
        key: &Value,
        kind: CollectionMemberKind,
    ) -> Result<bool, RuntimeError> {
        let object = self.heap.get(self.builder()?)?;
        if object.members.len() < 4 {
            return Ok(false);
        }
        // Only `Value::Object` keys can ever require lazy key-hash preparation.
        if !matches!(key, Value::Object(_)) && !object.has_object_collection_key {
            return Ok(false);
        }
        let duplicate = if kind != CollectionMemberKind::Property
            && let (Some(index), Some(hash)) = (&object.declaration_index, self.java_hash(key)?)
            && index.len() == object.collection_members.len()
        {
            index.candidates(Some(hash)).into_iter().any(|pos| {
                let member = &object.collection_members[pos];
                member.kind != CollectionMemberKind::Property
                    && same_key(&self.heap, key, &member.key)
            })
        } else {
            object.collection_members.iter().any(|member| {
                (member.kind == CollectionMemberKind::Property)
                    == (kind == CollectionMemberKind::Property)
                    && same_key(&self.heap, key, &member.key)
            })
        };
        if duplicate {
            return Ok(false);
        }
        let mut keys: Vec<_> = object
            .collection_members
            .iter()
            .filter(|member| matches!(member.key, Value::Object(_)))
            .map(|member| member.key.clone())
            .collect();
        if matches!(key, Value::Object(_)) {
            keys.push(key.clone());
        }
        self.prepare_keys(keys)
    }

    pub(super) fn add_collection_member(
        &mut self,
        mut member: Member,
        key: Option<Value>,
        kind: CollectionMemberKind,
        value: Option<Value>,
    ) -> Result<(), RuntimeError> {
        let target = self.builder()?;
        let object = self.heap.get(target)?;
        let collection_kind = object.collection_kind;
        let parent = object.parent;
        let length = object.length;
        let key = key.unwrap_or(Value::Int((length as i64).into()));
        if kind != CollectionMemberKind::Property
            && collection_kind == CollectionKind::Dynamic
            && let Some(class) = typed_class_ref(&self.heap, target)?
        {
            let element = kind == CollectionMemberKind::Element;
            let label = if element { "element" } else { "entry" };
            let type_label = if !element && !object.specialized_literal {
                crate::diagnostic_values::render(
                    &Value::Object(target),
                    &self.heap,
                    &self.program.module_name,
                    80,
                )?
            } else {
                class
                    .module
                    .as_ref()
                    .unwrap_or(&self.program.module_name)
                    .concat(&"#".into())
                    .concat(&class.name)
            };
            let mut error = RuntimeError::new(
                ErrorKind::Type,
                PklString::join(
                    &[
                        "Object of type `".into(),
                        type_label,
                        format!("` cannot have an {label}.").into(),
                    ],
                    "",
                ),
            );
            error.hint = Some(if element {
                "Only objects of type `Listing` and `Dynamic` can have elements. If you did not mean to add an element, you probably got the syntax wrong. For example, `obj { name }` adds an element, whereas `obj { name = \"Pigeon\" }` sets a property."
            } else {
                "Only objects of type `Mapping` and `Dynamic` can have entries. If you did not mean to add an entry, you probably got the syntax wrong. For example, `obj { [\"name\"] = \"Pigeon\" }` adds an entry, whereas `obj { name = \"Pigeon\" }` sets a property."
            }.into());
            return Err(error);
        }
        if kind == CollectionMemberKind::Element && collection_kind == CollectionKind::Mapping {
            let mut error = RuntimeError::new(
                ErrorKind::Type,
                "Object of type `Mapping` cannot have an element.",
            );
            error.hint = Some("Only objects of type `Listing` and `Dynamic` can have elements. If you did not mean to add an element, you probably got the syntax wrong. For example, `obj { name }` adds an element, whereas `obj { name = \"Pigeon\" }` sets a property.".into());
            return Err(error);
        }
        if kind == CollectionMemberKind::Property
            && !member.local
            && member.name != "default"
            && collection_kind != CollectionKind::Dynamic
        {
            let mut error = RuntimeError::new(
                ErrorKind::Type,
                format!(
                    "Object of type `{collection_kind:?}` cannot have a property (other than `default`)."
                ),
            );
            error.hint = Some("Objects of type `Listing` and `Mapping` can only have a `default` property (and `local` properties). If you did not mean to set a property, you probably got the syntax wrong. For example, `obj { name = \"Pigeon\" }` sets a property, whereas `obj { [\"name\"] = \"Pigeon\" }` adds an entry.".into());
            return Err(error);
        }
        if kind == CollectionMemberKind::Entry && collection_kind == CollectionKind::Listing {
            let Value::Int(index) = key else {
                return Err(RuntimeError::new(
                    ErrorKind::Type,
                    format!(
                        "Expected key of type `Int`, but got type `{}`.",
                        crate::diagnostic_values::type_name(
                            &key,
                            &self.heap,
                            &self.program.module_name
                        )?
                    ),
                ));
            };
            let parent_length = parent
                .map(|id| self.heap.get(id).map(|o| o.length))
                .transpose()?
                .unwrap_or(0);
            if index < 0 || index.get() as usize >= parent_length {
                return Err(RuntimeError::new(
                    ErrorKind::Type,
                    format!(
                        "Element index `{index}` is out of range `0`..`{}`.",
                        parent_length as i64 - 1
                    ),
                ));
            }
        }
        if kind == CollectionMemberKind::Property
            && object.members.iter().any(|old| {
                !old.entry
                    && old.name == member.name
                    && old.local == member.local
                    && old.method == member.method
            })
        {
            return Err(RuntimeError::new(
                ErrorKind::Type,
                PklString::from("Duplicate definition of member `")
                    .concat(&member.name)
                    .concat(&"`.".into()),
            ));
        }
        let mut descriptor = CollectionMember {
            key,
            name: member.name.clone(),
            kind,
        };
        let replacing = if descriptor.kind == CollectionMemberKind::Property {
            // Line 1905 already verified no duplicate property with this name/local/method exists.
            None
        } else if descriptor.kind == CollectionMemberKind::Element
            && collection_kind == CollectionKind::Listing
            && !object.specialized_literal
            && object.collection_members.len() == length
        {
            // Sequential Listing element append (key = length): cannot collide with 0..length.
            None
        } else if let (Some(index), Some(hash)) =
            (&object.declaration_index, self.java_hash(&descriptor.key)?)
            && index.len() == object.collection_members.len()
        {
            if index.has_hash() {
                index.hash_candidates(hash).find(|&pos| {
                    let old = &object.collection_members[pos];
                    same_member(&self.heap, &descriptor, old)
                })
            } else {
                (0..index.len()).find(|&pos| {
                    let old = &object.collection_members[pos];
                    same_member(&self.heap, &descriptor, old)
                })
            }
        } else if !matches!(descriptor.key, Value::Object(_)) && !object.has_object_collection_key {
            object
                .collection_members
                .iter()
                .position(|old| same_member(&self.heap, &descriptor, old))
        } else {
            let candidates = self.declaration_candidates(target, &descriptor.key, false)?;
            object.collection_members.iter().position(|old| {
                candidates
                    .iter()
                    .any(|candidate| equality::identical_key(&self.heap, candidate, &old.key))
                    && same_member(&self.heap, &descriptor, old)
            })
        };
        if replacing.is_some_and(|index| {
            !(object.specialized_literal
                && kind == CollectionMemberKind::Element
                && object.collection_members[index].kind == CollectionMemberKind::Entry)
        }) {
            return Err(RuntimeError::new(
                ErrorKind::Type,
                PklString::join(
                    &[
                        "Duplicate definition of member `".into(),
                        crate::diagnostic_values::render(
                            &descriptor.key,
                            &self.heap,
                            &self.program.module_name,
                            80,
                        )?,
                        "`.".into(),
                    ],
                    "",
                ),
            ));
        }
        let mut reused_parent_name = false;
        let mut parent_has_entries = false;
        if kind != CollectionMemberKind::Property {
            let parent_entry_name = if replacing.is_none()
                && let Some(parent_id) = parent
            {
                let mut found_name = None;
                let mut cur = Some(parent_id);
                let mut hops = 0u32;
                while let Some(pid) = cur {
                    hops += 1;
                    if hops > 1024 {
                        parent_has_entries = true;
                        found_name = ordered_members(&self.heap, parent_id)
                            .ok()
                            .and_then(|members| {
                                members
                                    .into_iter()
                                    .find(|old| same_member(&self.heap, &descriptor, old))
                            })
                            .map(|old| old.name);
                        break;
                    }
                    let Ok(pobj) = self.heap.get(pid) else {
                        break;
                    };
                    if pobj.builtin_type.is_some() && pobj.collection_members.is_empty() {
                        break;
                    }
                    if !pobj.collection_members.is_empty() {
                        parent_has_entries = true;
                        if found_name.is_none()
                            && let Some(old) = pobj
                                .collection_members
                                .iter()
                                .find(|old| same_member(&self.heap, &descriptor, old))
                        {
                            found_name = Some(old.name.clone());
                        }
                    }
                    if !parent_has_entries
                        && pobj.builtin_type.is_none()
                        && pobj.members.iter().any(|m| m.entry)
                    {
                        parent_has_entries = true;
                    }
                    cur = pobj.parent;
                }
                found_name
            } else {
                parent_has_entries = parent.is_some();
                None
            };
            reused_parent_name = parent_entry_name.is_some();
            descriptor.name = replacing
                .map(|index| object.collection_members[index].name.clone())
                .or(parent_entry_name)
                .unwrap_or_else(|| format_entry_internal_name(target.index, object.members.len()));
            member.name = descriptor.name.clone();
            member.entry = true;
        }
        if let Some(parent) = parent
            && (kind == CollectionMemberKind::Property || parent_has_entries)
        {
            let skip_plain_dynamic_prop = kind == CollectionMemberKind::Property
                && !member.local
                && !member.method
                && member.name.as_str() != Some("default")
                && self
                    .heap
                    .get_fast(parent)
                    .and_then(|pobj| pobj.builtin_type.as_deref())
                    .and_then(classes::BuiltinClass::from_type)
                    == Some(classes::BuiltinClass::Dynamic);
            if !skip_plain_dynamic_prop {
                self.validate_object_members(parent, std::slice::from_ref(&member))?;
            }
        }
        // Synthetic members with no body are complete values. Retaining their
        // producer's environment would keep callback inputs and lexical scopes
        // alive even when the result contains only scalars. Likewise, when a
        // member's lexical environment is identical to `object.environment`,
        // `member_environments` lookup falls back to `object.environment`
        // directly without duplicating it per member.
        let environment = if value.is_some() && member.code == usize::MAX && member.ty.is_none() {
            None
        } else {
            let frame = self.frames.last().expect("active frame");
            if frame.generator_scopes.is_empty()
                && environments_match(&frame.environment, &self.heap.get(target)?.environment)
            {
                None
            } else {
                let mut environment = frame.environment.clone();
                environment.restore_for_bindings = !frame.generator_scopes.is_empty();
                Some(environment)
            }
        };
        let probe = if value.is_some() && kind != CollectionMemberKind::Property {
            Some(descriptor.key.clone())
        } else {
            None
        };
        let object = self.heap.get_mut(target)?;
        let cache_key =
            if environment.is_some() || !object.member_environments.is_empty() || value.is_some() {
                Some(CacheKey {
                    owner: None,
                    name: member.name.clone(),
                    method: member.method,
                    entry: member.entry,
                })
            } else {
                None
            };
        if let Some(environment) = environment {
            object
                .member_environments
                .insert(cache_key.as_ref().unwrap().clone(), environment);
        } else if !object.member_environments.is_empty() {
            object
                .member_environments
                .remove(cache_key.as_ref().unwrap());
        }
        if !member.local && !member.method {
            if matches!(descriptor.key, Value::Object(_)) {
                object.has_object_collection_key = true;
            }
            if let Some(index) = replacing {
                object.collection_members[index] = descriptor;
            } else {
                object.collection_members.push(descriptor);
            }
        }
        if kind == CollectionMemberKind::Property || (replacing.is_none() && !reused_parent_name) {
            object.members.push(member);
        } else if let Some(index) = object.members.iter().position(|old| {
            old.name == member.name
                && old.entry == member.entry
                && old.local == member.local
                && old.method == member.method
        }) {
            object.members.set(index, member);
        } else {
            object.members.push(member);
        }
        if kind == CollectionMemberKind::Element {
            object.length += 1;
        }
        let need_decl_index =
            kind != CollectionMemberKind::Property || object.declaration_index.is_some();
        let mut decl_index_done = false;
        if need_decl_index && value.is_none() {
            if object.members.len() <= 4 {
                decl_index_done = true;
            } else if object.collection_members.len() != object.members.len() {
                object.declaration_index = None;
                object.declaration_hashes.clear();
                decl_index_done = true;
            } else if object.declaration_index.is_some()
                && object.declaration_hashes.len() + 1 == object.collection_members.len()
            {
                let last_member = &object.collection_members[object.declaration_hashes.len()];
                if last_member.kind == CollectionMemberKind::Property {
                    object.declaration_index = None;
                    object.declaration_hashes.clear();
                    decl_index_done = true;
                } else {
                    let fast_hash = match &last_member.key {
                        Value::String(s) => Some(equality::string_hash(s)),
                        Value::Int(i) => {
                            let v = i.get();
                            Some((v ^ (v >> 32)) as i32)
                        }
                        Value::Bool(b) => Some(if *b { 1231 } else { 1237 }),
                        _ => None,
                    };
                    if let Some(hash) = fast_hash {
                        let index = object.declaration_index.as_mut().unwrap();
                        if index.len() < object.collection_members.len() {
                            if index.grow_before_append() {
                                for old in index.start_rehash() {
                                    let old_hash = object.declaration_hashes[old];
                                    index.put_hash(old, old_hash, false);
                                }
                            }
                            let slot = index.push();
                            object.declaration_hashes.push(hash);
                            if (!index.has_hash() && index.len() > 4)
                                || (index.has_hash()
                                    && index.put_hash(slot, hash, index.should_rehash()))
                            {
                                for old in index.start_rehash() {
                                    let old_hash = object.declaration_hashes[old];
                                    index.put_hash(old, old_hash, false);
                                }
                            }
                        }
                        decl_index_done = true;
                    }
                }
            }
        }
        if let Some(value) = value {
            let cache_key = cache_key.unwrap();
            // Defer caching synthetic `default` properties on Dynamic/Listing/Mapping
            // so the first read runs `VmUtils.doReadMember`'s `default: (unknown) -> Any`
            // type check (e.g. `Map("default", null).toDynamic().default`), while
            // rendering the object without reading `default` stays unaffected.
            let defer_default_cache = kind == CollectionMemberKind::Property
                && cache_key.name == "default"
                && matches!(
                    collection_kind,
                    CollectionKind::Dynamic | CollectionKind::Listing | CollectionKind::Mapping
                );
            object
                .member_values
                .insert(cache_key.clone(), value.clone());
            if !defer_default_cache {
                self.complete_cache_probe_known_new(
                    target,
                    cache_key,
                    value,
                    probe,
                    replacing.is_none(),
                )?;
            }
        }
        if need_decl_index && !decl_index_done {
            self.update_declaration_index(target)?;
        }
        Ok(())
    }

    fn source_member(
        &self,
        mut source: ObjectId,
        descriptor: &CollectionMember,
    ) -> Result<Member, RuntimeError> {
        loop {
            let object = self.heap.get(source)?;
            if let Some(elems) = &object.list_elements
                && descriptor.kind == CollectionMemberKind::Element
                && let Value::Int(idx) = &descriptor.key
                && idx.get() >= 0
                && (idx.get() as usize) < elems.len()
            {
                return Ok(Member {
                    flags: 0,
                    name: descriptor.name.clone(),
                    code: usize::MAX,
                    local: false,
                    method: false,
                    entry: true,
                    ty: None,
                    type_span: None,
                    constant: false,
                    fixed: false,
                });
            }
            if let Some(member) = object.members.iter().find(|m| {
                m.name == descriptor.name
                    && m.entry == (descriptor.kind != CollectionMemberKind::Property)
                    && !m.local
                    && !m.method
            }) {
                return Ok(member.clone());
            }
            source = object
                .parent
                .ok_or_else(|| invalid("missing iterable member"))?;
        }
    }

    fn spread_error(&mut self, message: impl Into<PklString>, member: &Member) -> RuntimeError {
        self.spread_member_error(RuntimeError::new(ErrorKind::Type, message), member)
    }

    fn spread_member_error(&mut self, mut error: RuntimeError, member: &Member) -> RuntimeError {
        let frame = self.frames.last_mut().expect("active frame");
        if let Some(metadata) = self.program.metadata.get(frame.block)
            && let Some(Some(span)) = metadata.spans.get(frame.pc - 1)
        {
            error.frames.push(RuntimeFrame {
                anonymous: false,
                source: metadata.source.clone(),
                span: *span,
                member: frame
                    .diagnostic_member
                    .clone()
                    .unwrap_or_else(|| metadata.member.clone()),
            });
        }
        if let Some(metadata) = self.program.metadata.get(member.code)
            && let Some(span) = metadata.declaration
        {
            error.frames.push(RuntimeFrame {
                anonymous: false,
                source: metadata.source.clone(),
                span,
                member: metadata.member.clone(),
            });
        }
        frame.callsite_hidden = true;
        error
    }

    pub(super) fn collection_instruction(
        &mut self,
        instruction: Instruction,
    ) -> Result<(), RuntimeError> {
        let empty_amendment = matches!(instruction, Instruction::AmendEmpty);
        // Reuse construction without nesting this large dispatcher's native frame.
        let instruction = if empty_amendment {
            if matches!(self.stack.last(), Some(Value::Function(_))) {
                return Ok(());
            }
            // References use the object heap but are not Pkl VmObjectLike values.
            if let Some(Value::Object(id)) = self.stack.last()
                && self.heap.get(*id)?.reference.is_none()
                && !matches!(
                    self.heap.get(*id)?.collection_kind,
                    CollectionKind::List | CollectionKind::Set | CollectionKind::Map
                )
            {
                return Ok(());
            }
            if let Some(Value::NullWithDefault(id)) = self.stack.last() {
                let value = self.heap.null_default(*id)?;
                *self.stack.last_mut().expect("null parent") = value;
                return Ok(());
            }
            Instruction::BeginCollection {
                kind: CollectionKind::Dynamic,
                has_parent: true,
                specialized: true,
            }
        } else {
            instruction
        };
        match instruction {
            Instruction::Swap => {
                let right = self.pop()?;
                let left = self.pop()?;
                self.stack.push(right);
                self.stack.push(left);
            }
            Instruction::BeginCollection {
                kind,
                has_parent,
                specialized,
            } => {
                let parent = if has_parent {
                    let mut value = self.pop()?;
                    while let Value::NullWithDefault(id) = value {
                        value = self.heap.null_default(id)?;
                    }
                    match value {
                        Value::Null => Some(self.empty_object(CollectionKind::Dynamic)),
                        Value::Class(class) => Some(self.amendment_class_parent(class)?),
                        value @ (Value::Bool(_)
                        | Value::Int(_)
                        | Value::Float(_)
                        | Value::Duration(_)
                        | Value::IntSeq(_)
                        | Value::Regex(_)
                        | Value::Bytes(_)
                        | Value::TypeAlias(_)
                        | Value::DataSize(_)
                        | Value::String(_)) => {
                            return Err(external_amendment_error(value_type_name(&value)));
                        }
                        value => Some(value.as_object()?),
                    }
                } else {
                    None
                };
                let (kind, length) = if let Some(id) = parent {
                    let object = self.heap.get(id)?;
                    if object.reference.is_some() {
                        return Err(external_amendment_error("pkl.ref#Reference"));
                    }
                    if object.collection_kind == CollectionKind::Dynamic
                        && object.builtin_type.is_none()
                        && let Some(class) = typed_class_ref(&self.heap, id)?
                    {
                        self.check_class_instantiable(class)?;
                    }
                    if matches!(
                        object.collection_kind,
                        CollectionKind::List | CollectionKind::Map | CollectionKind::Set
                    ) {
                        return Err(external_amendment_error(format!(
                            "{:?}",
                            object.collection_kind
                        )));
                    }
                    (object.collection_kind, object.length)
                } else {
                    (kind, 0)
                };
                let environment = if parent.is_none()
                    && matches!(
                        kind,
                        CollectionKind::List | CollectionKind::Set | CollectionKind::Map
                    ) {
                    Environment::default()
                } else {
                    self.frames
                        .last()
                        .expect("active frame")
                        .environment
                        .clone()
                };
                let is_module = parent
                    .map(|id| self.heap.get(id).map(|o| o.is_module))
                    .transpose()?
                    .unwrap_or(false);
                let id = self.heap.allocate(Object {
                    is_module,
                    parent,
                    collection_kind: kind,
                    length,
                    environment,
                    specialized_literal: specialized,
                    ..Object::default()
                });
                self.frames
                    .last_mut()
                    .expect("active frame")
                    .builders
                    .push(id);
            }
            Instruction::SetCollectionDefault { defined, origin } => {
                let value = if defined { Some(self.pop()?) } else { None };
                let target = self.builder()?;
                self.heap.get_mut(target)?.collection_default = Some(value);
                self.heap.get_mut(target)?.collection_default_origin = origin;
            }
            Instruction::SetCollectionTypeDefault { ty, origin } => {
                let environment = self
                    .frames
                    .last()
                    .expect("active frame")
                    .environment
                    .clone();
                let ty = ty.substituted(&environment)?;
                let target = self.builder()?;
                self.heap.get_mut(target)?.collection_default_origin = origin.clone();
                if matches!(ty.resolved(), TypeCheck::Unknown) {
                    self.heap.get_mut(target)?.collection_default = None;
                } else {
                    self.frames
                        .last_mut()
                        .expect("active frame")
                        .generic_default = Some(Box::new(generics::GenericDefaultRun::new(
                        target,
                        ty,
                        origin.map(|o| *o),
                    )));
                }
            }
            Instruction::GenericConstructorParent { ty, source, cache } => {
                let environment = &self.frames.last().expect("active frame").environment;
                let source = ty.argument_source(environment).unwrap_or(*source);
                let ty = ty.substituted(environment)?;
                let frame = self.frames.last().expect("active frame");
                let metadata = &self.program.metadata[frame.block];
                let origin = metadata
                    .spans
                    .get(frame.pc.saturating_sub(1))
                    .copied()
                    .flatten()
                    .map(|span| RuntimeFrame {
                        anonymous: false,
                        source: metadata.source.clone(),
                        span,
                        member: if metadata.member.is_empty() {
                            metadata
                                .source
                                .as_ref()
                                .map(|source| source.uri.clone())
                                .unwrap_or_default()
                        } else {
                            metadata.member.clone()
                        }
                        .concat(&".default".into()),
                    });
                let cache = cache.then_some((frame.block, frame.pc - 1));
                self.frames
                    .last_mut()
                    .expect("active frame")
                    .generic_default = Some(Box::new(generics::GenericDefaultRun::constructor(
                    ty, source, origin, cache,
                )));
            }

            Instruction::EndCollection => self.end_collection()?,
            Instruction::AddCollectionProperty(member) => {
                let key = Value::string(member.name.clone());
                if self.heap.get(self.builder()?)?.has_object_collection_key
                    && self.prepare_insertion(&key, CollectionMemberKind::Property)?
                {
                    return Ok(());
                }
                self.add_collection_member(
                    *member,
                    Some(key),
                    CollectionMemberKind::Property,
                    None,
                )?;
            }
            Instruction::AddCollectionElement(member) => {
                let target = self.builder()?;
                let object = self.heap.get(target)?;
                if object.has_object_collection_key
                    && object.collection_kind != CollectionKind::Mapping
                    && !is_typed_class(&self.heap, target)?
                {
                    let key = Value::Int((object.length as i64).into());
                    if self.prepare_insertion(&key, CollectionMemberKind::Element)? {
                        return Ok(());
                    }
                }
                self.add_collection_member(*member, None, CollectionMemberKind::Element, None)?
            }
            Instruction::AddCollectionEntry {
                member,
                check_index,
                span,
            } => {
                let key = self.pop()?;
                let target = self.builder()?;
                let (is_typed, specialized_literal, needs_obj_prep) = {
                    let obj = self.heap.get(target)?;
                    let is_typed = obj.collection_kind == CollectionKind::Dynamic
                        && is_typed_class(&self.heap, target)?;
                    let needs_obj_prep =
                        matches!(key, Value::Object(_)) || obj.has_object_collection_key;
                    (is_typed, obj.specialized_literal, needs_obj_prep)
                };
                if !is_typed && !check_index && needs_obj_prep {
                    if self.heap.get(target)?.members.len() > 4
                        && self.prepare_keys(vec![key.clone()])?
                    {
                        self.stack.push(key);
                        return Ok(());
                    }
                    let candidates = self.declaration_candidates(target, &key, false)?;
                    if self.search_key(&key, candidates)? {
                        self.stack.push(key);
                        return Ok(());
                    }
                    if self.prepare_insertion(&key, CollectionMemberKind::Entry)? {
                        self.stack.push(key);
                        return Ok(());
                    }
                }
                if check_index {
                    let parent = self.heap.get(self.builder()?)?.parent;
                    let length = parent
                        .map(|id| self.heap.get(id).map(|o| o.length))
                        .transpose()?
                        .unwrap_or(0);
                    let Value::Int(index) = key else {
                        return Err(RuntimeError::new(
                            ErrorKind::Type,
                            format!(
                                "Expected value of type `Int`, but got type `{}`.",
                                value_type_name(&key)
                            ),
                        ));
                    };
                    if index < 0 || index.get() as usize >= length {
                        return Err(RuntimeError::new(
                            ErrorKind::Type,
                            format!(
                                "Element index `{index}` is out of range `0`..`{}`.",
                                length as i64 - 1
                            ),
                        ));
                    }
                }
                let generated_typed = !specialized_literal && is_typed;
                self.add_collection_member(*member, Some(key), CollectionMemberKind::Entry, None)
                    .map_err(|error| {
                        if generated_typed {
                            error.at(span)
                        } else {
                            error
                        }
                    })?;
            }
            Instruction::Subscript { .. } | Instruction::SuperSubscript { .. } => {
                if self.resume_entry_lookup()? {
                    return Ok(());
                }
                let key = self.pop()?;
                let is_super = matches!(instruction, Instruction::SuperSubscript { .. });
                let receiver = if is_super {
                    self.scope(0)?.receiver
                } else {
                    let value = self.pop()?;
                    if let Value::String(text) = &value {
                        let Instruction::Subscript { key_span } = instruction else {
                            unreachable!("ordinary subscript instruction")
                        };
                        let value = self.subscript_string(text, key, key_span)?;
                        self.stack.push(value);
                        return Ok(());
                    }
                    if let Value::Bytes(bytes) = &value {
                        let value = self.subscript_bytes(bytes, key)?;
                        self.stack.push(value);
                        return Ok(());
                    }
                    if matches!(value, Value::IntSeq(_)) {
                        let headline = format!(
                            "Operator `[]` is not defined for operand types `IntSeq` and `{}`.",
                            crate::diagnostic_values::type_name(
                                &key,
                                &self.heap,
                                &self.program.module_name,
                            )?
                        );
                        let left_val = crate::diagnostic_values::render(
                            &value,
                            &self.heap,
                            &self.program.module_name,
                            80,
                        )?;
                        let right_val = crate::diagnostic_values::render(
                            &key,
                            &self.heap,
                            &self.program.module_name,
                            80,
                        )?;
                        return Err(RuntimeError::with_program_values(
                            ErrorKind::Type,
                            headline,
                            &[("Left operand", left_val), ("Right operand", right_val)],
                        ));
                    }
                    value.as_object()?
                };
                if !is_super && self.heap.get(receiver)?.collection_kind == CollectionKind::Set {
                    let headline = PklString::join(
                        &[
                            "Operator `[]` is not defined for operand types `Set` and `".into(),
                            crate::diagnostic_values::type_name(
                                &key,
                                &self.heap,
                                &self.program.module_name,
                            )?,
                            "`.".into(),
                        ],
                        "",
                    );
                    let left_val = crate::diagnostic_values::render(
                        &Value::Object(receiver),
                        &self.heap,
                        &self.program.module_name,
                        80,
                    )?;
                    let right_val = crate::diagnostic_values::render(
                        &key,
                        &self.heap,
                        &self.program.module_name,
                        80,
                    )?;
                    return Err(RuntimeError::with_program_values(
                        ErrorKind::Type,
                        headline,
                        &[("Left operand", left_val), ("Right operand", right_val)],
                    ));
                }
                if !is_super && self.heap.get(receiver)?.collection_kind == CollectionKind::Map {
                    if self.prepare_map_lookup(receiver, &key)? {
                        self.stack.extend([Value::Object(receiver), key]);
                        return Ok(());
                    }
                    let Some(value) = self.map_value(receiver, &key)? else {
                        return self.fail_missing_key(receiver, key);
                    };
                    self.stack.push(value);
                    return Ok(());
                }
                if !is_super && self.heap.get(receiver)?.reference.is_some() {
                    self.stack.push(Value::Object(receiver));
                    self.stack.push(key.clone());
                    if let Some(result) = self.reference_access(receiver, None, Some(key))? {
                        self.pop()?;
                        self.pop()?;
                        self.stack.push(result);
                    }
                    return Ok(());
                }
                if let Instruction::SuperSubscript { end } = instruction {
                    let owner = self.heap.get(self.scope(0)?.owner)?.parent;
                    return self.super_entry_lookup(receiver, owner, key, end, false);
                }
                if !is_super
                    && matches!(
                        self.heap.get(receiver)?.collection_kind,
                        CollectionKind::Mapping | CollectionKind::Dynamic
                    )
                {
                    return self.mapping_lookup(receiver, key, mapping_ops::LookupMode::Subscript);
                }
                let owner = if is_super {
                    let Some(parent) = self.heap.get(self.scope(0)?.owner)?.parent else {
                        self.stack.push(key);
                        return self.force(
                            receiver,
                            receiver,
                            "default".into(),
                            false,
                            true,
                            false,
                        );
                    };
                    parent
                } else {
                    receiver
                };
                if self.heap.get(owner)?.members.len() > 4
                    && self.prepare_keys(vec![key.clone()])?
                {
                    if !is_super {
                        self.stack.push(Value::Object(receiver));
                    }
                    self.stack.push(key);
                    return Ok(());
                }
                let candidates = self.declaration_candidates(owner, &key, true)?;
                if self.search_key(&key, candidates)? {
                    if !is_super {
                        self.stack.push(Value::Object(receiver));
                    }
                    self.stack.push(key);
                    return Ok(());
                }
                let object = self.heap.get(receiver)?;
                if !is_super
                    && matches!(
                        object.collection_kind,
                        CollectionKind::Listing | CollectionKind::List
                    )
                {
                    let Value::Int(index) = key else {
                        let name = format!("{:?}", object.collection_kind);
                        let headline = format!(
                            "Operator `[]` is not defined for operand types `{name}` and `{}`.",
                            value_type_name(&key)
                        );
                        let left_val = crate::diagnostic_values::render(
                            &Value::Object(receiver),
                            &self.heap,
                            &self.program.module_name,
                            80,
                        )?;
                        let right_val = crate::diagnostic_values::render(
                            &key,
                            &self.heap,
                            &self.program.module_name,
                            80,
                        )?;
                        return Err(RuntimeError::with_program_values(
                            ErrorKind::Type,
                            headline,
                            &[("Left operand", left_val), ("Right operand", right_val)],
                        ));
                    };
                    if index < 0 || index.get() as usize >= object.length {
                        let headline: PklString = format!(
                            "Element index `{}` is out of range `0`..`{}`.",
                            strings::diagnostic_integer(index.get()),
                            strings::diagnostic_integer(object.length as i64 - 1)
                        )
                        .into();
                        if object.collection_kind == CollectionKind::List {
                            let rendered = crate::diagnostic_values::render_string(
                                &Value::Object(receiver),
                                &self.heap,
                                &self.program.module_name,
                                80,
                            )?;
                            return Err(RuntimeError::with_program_values(
                                ErrorKind::Type,
                                headline,
                                &[("Collection", rendered)],
                            ));
                        }
                        return Err(RuntimeError::new(ErrorKind::Type, headline));
                    }
                }
                let member = if self.heap.get(owner)?.collection_kind == CollectionKind::List
                    && self.heap.get(owner)?.parent.is_none()
                {
                    if let Value::Int(index) = &key
                        && index.get() >= 0
                    {
                        let obj = self.heap.get(owner)?;
                        let idx = index.get() as usize;
                        if let Some(elems) = &obj.list_elements {
                            if let Some(val) = elems.get(idx) {
                                self.stack.push(val.clone());
                                return Ok(());
                            }
                            None
                        } else if idx < obj.collection_members.len() {
                            Some(obj.collection_members[idx].clone())
                        } else if idx < obj.length && !obj.collection_members.is_empty() {
                            let base_len = obj
                                .repeat_base_len
                                .unwrap_or(obj.collection_members.len())
                                .max(1);
                            let resolved_idx = idx % base_len;
                            Some(obj.collection_members[resolved_idx].clone())
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    let candidates = self.declaration_candidates(owner, &key, true)?;
                    ordered_members(&self.heap, owner)?
                        .into_iter()
                        .rev()
                        .find(|m| {
                            m.kind != CollectionMemberKind::Property
                                && candidates.iter().any(|candidate| {
                                    equality::identical_key(&self.heap, candidate, &m.key)
                                })
                                && same_key(&self.heap, &key, &m.key)
                        })
                };
                if member.is_none() && self.prepare_keys(vec![key.clone()])? {
                    if !is_super {
                        self.stack.push(Value::Object(receiver));
                    }
                    self.stack.push(key);
                    return Ok(());
                }
                if let Instruction::SuperSubscript { end } = instruction {
                    if member.is_none() {
                        // ReadSuperEntryNode uses the dynamic receiver's default,
                        // not the lexical owner's. Keep the evaluated key rooted
                        // while property/type resolution or the callback suspends.
                        self.stack.push(key);
                        return self.force(
                            receiver,
                            receiver,
                            "default".into(),
                            false,
                            true,
                            false,
                        );
                    }
                    self.frames.last_mut().expect("super entry frame").pc = end;
                }
                let Some(member) = member else {
                    return self.fail_missing_key(receiver, key);
                };
                self.force_key_probe(receiver, owner, member.name, !is_super, key)?;
            }
            Instruction::EntryKey => {
                let force = self
                    .frames
                    .last()
                    .and_then(|frame| frame.force.as_ref())
                    .ok_or_else(|| invalid("entry key outside member"))?;
                let key = force
                    .entry_probe
                    .clone()
                    .ok_or_else(|| invalid("entry key is missing"))?;
                self.stack.push(key);
            }
            Instruction::EntryDefault => {
                let receiver = self
                    .frames
                    .last()
                    .and_then(|frame| frame.force.as_ref())
                    .ok_or_else(|| invalid("entry default outside member"))?
                    .receiver;
                self.force(receiver, receiver, "default".into(), false, true, false)?;
            }
            Instruction::EntryParent { end } => {
                if self.resume_entry_lookup()? {
                    return Ok(());
                }
                let force = self
                    .frames
                    .last()
                    .and_then(|frame| frame.force.as_ref())
                    .cloned()
                    .ok_or_else(|| invalid("entry parent outside member"))?;
                let owner = self.heap.get(force.owner)?.parent;
                let key = force
                    .entry_probe
                    .ok_or_else(|| invalid("entry parent key"))?;
                self.super_entry_lookup(force.receiver, owner, key, end, true)?;
            }
            Instruction::OpenIterator { nullable, spread } => {
                let value = self.pop()?;
                if let Value::Bytes(bytes) = value {
                    let id = self.heap.allocate(Object {
                        bytes_iterator: Some(Box::new((bytes, 0, spread))),
                        ..Default::default()
                    });
                    self.stack.push(Value::Object(id));
                    return Ok(());
                }
                if let Value::IntSeq(sequence) = value {
                    let id = self.heap.allocate(Object {
                        int_seq_iterator: Some(Box::new(sequence.cursor(!spread))),
                        ..Default::default()
                    });
                    self.stack.push(Value::Object(id));
                    return Ok(());
                }
                let iterator = if nullable && value == Value::Null {
                    let source = self.heap.allocate(Object::default());
                    Some(CollectionIterator {
                        source,
                        members: Vec::new(),
                        position: 0,
                        force_members: Vec::new(),
                        force_position: 0,
                        awaiting_value: false,
                        members_refreshed: true,
                    })
                } else {
                    let source = match value.as_object() {
                        Ok(source) => source,
                        Err(_) => {
                            let mut error = RuntimeError::new(
                                ErrorKind::Type,
                                format!(
                                    "Cannot iterate over value of type `{}`.",
                                    value_type_name(&value)
                                ),
                            );
                            if !(spread && value.is_null()) {
                                let rendered = crate::diagnostic_values::render(
                                    &value,
                                    &self.heap,
                                    &self.program.module_name,
                                    80,
                                )?;
                                error.push_program_value("Value", rendered);
                            }
                            if spread && value.is_null() {
                                let frame = self.frames.last().expect("active frame");
                                error.spread_hint = self
                                    .program
                                    .metadata
                                    .get(frame.block)
                                    .and_then(|metadata| metadata.spans.get(frame.pc - 1))
                                    .copied()
                                    .flatten()
                                    .map(Box::new);
                            }
                            return Err(error);
                        }
                    };
                    if self.heap.get(source)?.reference.is_some() {
                        let rendered = crate::diagnostic_values::render(
                            &value,
                            &self.heap,
                            &self.program.module_name,
                            80,
                        )?;
                        return Err(RuntimeError::with_program_values(
                            ErrorKind::Type,
                            "Cannot iterate over value of type `pkl.ref#Reference`.",
                            &[("Value", rendered)],
                        ));
                    }
                    if self.heap.get(source)?.is_module {
                        let headline = PklString::from("Cannot iterate over value of type `")
                            .concat(&self.program.module_name)
                            .concat(&"`.".into());
                        let rendered = crate::diagnostic_values::render(
                            &value,
                            &self.heap,
                            &self.program.module_name,
                            80,
                        )?;
                        let mut error = RuntimeError::with_program_values(
                            ErrorKind::Type,
                            headline,
                            &[("Value", rendered)],
                        );
                        if spread {
                            error.hint = Some("`Typed` values are not iterable. If you mean to spread its members, convert it to `Dynamic` using `toDynamic()`.".into());
                        }
                        self.frames
                            .last_mut()
                            .expect("active frame")
                            .callsite_hidden = true;
                        return Err(error);
                    }
                    if let Some(class) = typed_class(&self.heap, source)? {
                        let headline = PklString::from("Cannot iterate over value of type `")
                            .concat(&reference::type_name(
                                &TypeCheck::Class {
                                    module: class.module.clone(),
                                    id: class.id,
                                    name: class.name,
                                },
                                &self.program.module_name,
                            ))
                            .concat(&"`.".into());
                        let rendered = crate::diagnostic_values::render(
                            &value,
                            &self.heap,
                            &self.program.module_name,
                            80,
                        )?;
                        let mut error = RuntimeError::with_program_values(
                            ErrorKind::Type,
                            headline,
                            &[("Value", rendered)],
                        );
                        if spread {
                            error.hint = Some("`Typed` values are not iterable. If you mean to spread its members, convert it to `Dynamic` using `toDynamic()`.".into());
                        }
                        return Err(error);
                    }
                    let members = ordered_members(&self.heap, source)?;
                    let src_obj = self.heap.get(source)?;
                    let same_force_order = src_obj.collection_kind == CollectionKind::Map
                        || (src_obj.collection_kind == CollectionKind::List
                            && src_obj.list_elements.is_some())
                        || (!src_obj.collection_members.is_empty()
                            && is_single_level_leaf_collection(&self.heap, src_obj)
                            && src_obj.collection_members.len() == src_obj.members.len()
                            && members.len() == src_obj.collection_members.len());
                    let force_members = if same_force_order {
                        members.clone()
                    } else {
                        forcing_members(&self.heap, source)?
                    };
                    Some(CollectionIterator {
                        source,
                        force_members,
                        members,
                        position: 0,
                        force_position: 0,
                        awaiting_value: false,
                        members_refreshed: false,
                    })
                };
                let id = self.heap.allocate(Object {
                    iterator: iterator.map(Box::new),
                    ..Object::default()
                });
                self.stack.push(Value::Object(id));
            }
            Instruction::EnterForScope { bindings } => self.enter_for_scope(bindings)?,
            Instruction::LeaveForScope => self.leave_for_scope()?,
            Instruction::IteratorNext { end } => {
                let id = self
                    .stack
                    .iter()
                    .rev()
                    .filter_map(Value::heap_id)
                    .find(|id| {
                        self.heap.get(*id).is_ok_and(|object| {
                            object.iterator.is_some()
                                || object.int_seq_iterator.is_some()
                                || object.bytes_iterator.is_some()
                        })
                    })
                    .or_else(|| self.stack.last().and_then(Value::heap_id))
                    .ok_or_else(|| invalid("missing iterator"))?;
                if let Some(cursor) = self.heap.get_mut(id)?.bytes_iterator.as_mut() {
                    if let Some(byte) = cursor.0.get(cursor.1).copied() {
                        let value = if cursor.2 {
                            i64::from(byte)
                        } else {
                            i64::from(byte as i8)
                        };
                        self.stack.extend([
                            Value::Int((cursor.1 as i64).into()),
                            Value::Int(value.into()),
                        ]);
                        cursor.1 += 1;
                    } else {
                        self.jump(end)?;
                    }
                    return Ok(());
                }
                if let Some(cursor) = self.heap.get_mut(id)?.int_seq_iterator.as_mut() {
                    if let Some((index, value)) = cursor.next() {
                        self.stack
                            .extend([Value::Int(index.into()), Value::Int(value.into())]);
                    } else {
                        self.jump(end)?;
                    }
                    return Ok(());
                }
                let source = self
                    .heap
                    .get(id)?
                    .iterator
                    .as_ref()
                    .ok_or_else(|| invalid("missing iterator"))?
                    .source;
                if self.heap.get(source)?.collection_kind == CollectionKind::Map {
                    self.next_map_entry(id, end)?;
                    return Ok(());
                }
                let awaiting = self
                    .heap
                    .get(id)?
                    .iterator
                    .as_ref()
                    .is_some_and(|iterator| iterator.awaiting_value);
                if awaiting {
                    self.pop()?;
                    self.heap
                        .get_mut(id)?
                        .iterator
                        .as_mut()
                        .expect("active iterator")
                        .awaiting_value = false;
                }
                let forcing = self
                    .heap
                    .get_mut(id)?
                    .iterator
                    .as_mut()
                    .and_then(|iterator| {
                        let member = iterator.force_members.get(iterator.force_position)?.clone();
                        iterator.force_position += 1;
                        iterator.awaiting_value = true;
                        Some((iterator.source, member))
                    });
                if let Some((source, member)) = forcing {
                    let caller = self.frames.len() - 1;
                    self.frames[caller].pc -= 1;
                    self.force_collection_member(source, member)?;
                    if self.frames.len() > caller + 1 {
                        self.frames[caller].callsite_hidden = true;
                    }
                    return Ok(());
                }
                let (source, needs_refresh) = {
                    let iter = self
                        .heap
                        .get(id)?
                        .iterator
                        .as_ref()
                        .expect("active iterator");
                    (iter.source, !iter.members_refreshed)
                };
                if needs_refresh {
                    if !self.heap.key_equalities.is_empty() {
                        let effective = ordered_members(&self.heap, source)?;
                        let iter = self
                            .heap
                            .get_mut(id)?
                            .iterator
                            .as_mut()
                            .expect("active iterator");
                        iter.members = effective;
                        iter.members_refreshed = true;
                    } else {
                        self.heap
                            .get_mut(id)?
                            .iterator
                            .as_mut()
                            .expect("active iterator")
                            .members_refreshed = true;
                    }
                }
                let next = self
                    .heap
                    .get_mut(id)?
                    .iterator
                    .as_mut()
                    .and_then(|iterator| {
                        let member = iterator.members.get(iterator.position)?.clone();
                        iterator.position += 1;
                        Some((iterator.source, member))
                    });
                if let Some((source, member)) = next {
                    self.stack.push(member.key.clone());
                    let caller = self.frames.len() - 1;
                    self.force_collection_member(source, member)?;
                    if self.frames.len() > caller + 1 {
                        self.frames[caller].callsite_hidden = true;
                    }
                } else {
                    self.jump(end)?;
                }
            }
            Instruction::SpreadMember => {
                let value = self.pop()?;
                let key = self.pop()?;
                let iterator = self
                    .stack
                    .last()
                    .ok_or_else(|| invalid("missing spread iterator"))?
                    .as_object()?;
                if self.heap.get(iterator)?.int_seq_iterator.is_some()
                    || self.heap.get(iterator)?.bytes_iterator.is_some()
                {
                    self.stack.push(value);
                    self.append_strict_element()?;
                    return Ok(());
                }
                let state = self
                    .heap
                    .get(iterator)?
                    .iterator
                    .as_ref()
                    .ok_or_else(|| invalid("empty spread iterator"))?;
                let source_member = state.members[state.position - 1].clone();
                let mut member = self.source_member(state.source, &source_member)?;
                let target = self.builder()?;
                let object = self.heap.get(target)?;
                let target_kind = object.collection_kind;
                let needs_obj_prep =
                    matches!(key, Value::Object(_)) || object.has_object_collection_key;
                let is_typed = is_typed_class(&self.heap, target)?;
                let forbidden = if is_typed {
                    source_member.kind != CollectionMemberKind::Property
                } else {
                    match target_kind {
                        CollectionKind::Listing | CollectionKind::List | CollectionKind::Set => {
                            source_member.kind != CollectionMemberKind::Element
                        }
                        CollectionKind::Mapping | CollectionKind::Map => {
                            source_member.kind != CollectionMemberKind::Entry
                        }
                        CollectionKind::Dynamic => false,
                    }
                };
                if forbidden {
                    let kind = match source_member.kind {
                        CollectionMemberKind::Property => "properties",
                        CollectionMemberKind::Entry => "entries",
                        CollectionMemberKind::Element => "elements",
                    };
                    let type_label = typed_class_ref(&self.heap, target)?.map_or_else(
                        || PklString::from(format!("{target_kind:?}")),
                        |class| {
                            class
                                .module
                                .as_ref()
                                .unwrap_or(&self.program.module_name)
                                .concat(&"#".into())
                                .concat(&class.name)
                        },
                    );
                    return Err(self.spread_error(
                        PklString::from(format!(
                            "Cannot spread object containing {kind} into object of type `"
                        ))
                        .concat(&type_label)
                        .concat(&"`.".into()),
                        &member,
                    ));
                }
                if needs_obj_prep {
                    if source_member.kind == CollectionMemberKind::Entry {
                        if self.heap.get(target)?.members.len() > 4
                            && self.prepare_keys(vec![key.clone()])?
                        {
                            self.stack.push(key);
                            return Ok(());
                        }
                        let candidates = self.declaration_candidates(target, &key, false)?;
                        if self.search_key(&key, candidates)? {
                            self.stack.push(key);
                            self.stack.push(value);
                            return Ok(());
                        }
                    }
                    let insertion_key = if source_member.kind == CollectionMemberKind::Element {
                        Value::Int((self.heap.get(target)?.length as i64).into())
                    } else {
                        key.clone()
                    };
                    if self.prepare_insertion(&insertion_key, source_member.kind)? {
                        self.stack.push(key);
                        self.stack.push(value);
                        return Ok(());
                    }
                }
                let object = self.heap.get(target)?;
                if source_member.kind != CollectionMemberKind::Element
                    && object
                        .collection_members
                        .iter()
                        .any(|m| same_member(&self.heap, &source_member, m))
                {
                    let label = if source_member.kind == CollectionMemberKind::Property {
                        "property"
                    } else {
                        "entry key"
                    };
                    let key = if source_member.kind == CollectionMemberKind::Property {
                        scalar_string(&key)?
                    } else {
                        display_value(&key)
                    };
                    return Err(self.spread_error(PklString::join(&[format!("Cannot spread object because the enclosing object already has a declaration of {label} `").into(),key,"`.".into()], ""), &member));
                }
                member.constant = false;
                let key = (source_member.kind != CollectionMemberKind::Element).then_some(key);
                // GeneratorSpreadNode retains the throwing spread node and
                // inserts the source member's header under its qualified name.
                // Keep the validation error's message, hint and provenance.
                if let Err(error) =
                    self.add_collection_member(member.clone(), key, source_member.kind, Some(value))
                {
                    return Err(self.spread_member_error(error, &member));
                }
            }
            Instruction::OpenPredicate => {
                let target = self.builder()?;
                let object = self.heap.get(target)?;
                let is_typed = is_typed_class(&self.heap, target)?;
                if object.is_module || is_typed || object.collection_kind == CollectionKind::List {
                    let label = typed_class_ref(&self.heap, target)?.map_or_else(
                        || {
                            if object.is_module {
                                self.program.module_name.clone()
                            } else {
                                PklString::from(format!("{:?}", object.collection_kind))
                            }
                        },
                        |class| {
                            class
                                .module
                                .as_ref()
                                .unwrap_or(&self.program.module_name)
                                .concat(&"#".into())
                                .concat(&class.name)
                        },
                    );
                    return Err(RuntimeError::new(
                        ErrorKind::Type,
                        PklString::from("Object of type `")
                            .concat(&label)
                            .concat(&"` cannot have a predicate member.".into()),
                    ));
                }
                let source = object.parent.unwrap_or(target);
                let members = if object.parent.is_some() {
                    predicate_members(&self.heap, source)?
                } else {
                    Vec::new()
                };
                let previous_this = self.current_custom_this()?;
                let id = self.heap.allocate(Object {
                    predicate_iterator: Some(Box::new(PredicateIterator {
                        source,
                        members,
                        position: 0,
                        awaiting_value: false,
                        previous_this,
                    })),
                    ..Object::default()
                });
                self.stack.push(Value::Object(id));
            }
            Instruction::PredicateNext { end } => {
                let id = self
                    .stack
                    .last()
                    .and_then(Value::heap_id)
                    .filter(|id| {
                        self.heap
                            .get(*id)
                            .is_ok_and(|object| object.predicate_iterator.is_some())
                    })
                    .or_else(|| {
                        self.stack
                            .iter()
                            .rev()
                            .filter_map(Value::heap_id)
                            .find(|id| {
                                self.heap
                                    .get(*id)
                                    .is_ok_and(|object| object.predicate_iterator.is_some())
                            })
                    })
                    .ok_or_else(|| invalid("missing predicate iterator"))?;
                let state = self
                    .heap
                    .get(id)?
                    .predicate_iterator
                    .as_ref()
                    .expect("predicate iterator");
                let source = state.source;
                let awaiting_value = state.awaiting_value;
                let previous_this = state.previous_this.clone();
                let next = state
                    .members
                    .get(state.position - usize::from(awaiting_value))
                    .cloned();
                if awaiting_value {
                    let value = self.pop()?;
                    let (owner, member) =
                        next.ok_or_else(|| invalid("missing pending predicate member"))?;
                    self.complete_cache(
                        owner,
                        CacheKey {
                            owner: None,
                            name: member.name.clone(),
                            method: false,
                            entry: true,
                        },
                        value.clone(),
                    )?;
                    self.heap
                        .get_mut(id)?
                        .predicate_iterator
                        .as_mut()
                        .expect("predicate iterator")
                        .awaiting_value = false;
                    self.set_current_custom_this(Some(value))?;
                    return Ok(());
                }
                let Some((owner, member)) = next else {
                    self.set_current_custom_this(previous_this)?;
                    self.pop()?;
                    self.jump(end)?;
                    return Ok(());
                };
                self.heap
                    .get_mut(id)?
                    .predicate_iterator
                    .as_mut()
                    .expect("predicate iterator")
                    .position += 1;
                let key = CacheKey {
                    owner: None,
                    name: member.name.clone(),
                    method: false,
                    entry: true,
                };
                if let Some(value) = self.cached_slot(owner, &key)? {
                    self.set_current_custom_this(Some(value))?;
                } else {
                    self.heap
                        .get_mut(id)?
                        .predicate_iterator
                        .as_mut()
                        .expect("predicate iterator")
                        .awaiting_value = true;
                    let caller = self.frames.len() - 1;
                    self.frames[caller].pc -= 1;
                    self.force_key_mode(
                        source,
                        owner,
                        member.name.clone(),
                        false,
                        false,
                        false,
                        true,
                        true,
                    )?;
                    if self.frames.len() > caller + 1 {
                        self.frames[caller].callsite_hidden = true;
                    }
                }
            }
            Instruction::AddPredicateMember(member) => {
                let applicable = self.pop()?;
                let id = self
                    .stack
                    .last()
                    .ok_or_else(|| invalid("missing predicate iterator"))?
                    .as_object()?;
                let state = self
                    .heap
                    .get(id)?
                    .predicate_iterator
                    .as_ref()
                    .ok_or_else(|| invalid("missing predicate iterator"))?;
                let key = state.members[state.position - 1].1.key.clone();
                let kind = state.members[state.position - 1].1.kind;
                let previous_this = state.previous_this.clone();
                self.set_current_custom_this(previous_this)?;
                let Value::Bool(applicable) = applicable else {
                    return Err(type_error(
                        &TypeCheck::Boolean,
                        &applicable,
                        &self.program.module_name,
                        &self.heap,
                    )?);
                };
                if applicable {
                    // Selected elements replace existing indices; they do not
                    // append or increase the collection's length.
                    self.add_collection_member(
                        *member,
                        Some(key),
                        CollectionMemberKind::Entry,
                        None,
                    )?;
                    self.heap
                        .get_mut(self.builder()?)?
                        .collection_members
                        .last_mut()
                        .ok_or_else(|| invalid("missing selected predicate member"))?
                        .kind = kind;
                }
            }
            Instruction::PrepareSetElement => self.prepare_set_element()?,
            Instruction::AddSetElement => self.add_set_element()?,
            Instruction::PutSetKey => {
                let value = self.pop()?;
                if self.put_set_key(&value)? {
                    self.stack.push(value);
                }
            }
            Instruction::PrepareMapEntry => self.prepare_map_entry()?,
            Instruction::AddMapEntry => self.add_map_entry()?,
            Instruction::CheckMapSpread => self.check_strict_spread()?,
            Instruction::MakeList { length } => {
                let mut values = Vec::with_capacity(length);
                for _ in 0..length {
                    values.push(self.pop()?);
                }
                values.reverse();
                let id = self.build_strict_list(values);
                self.stack.push(Value::Object(id));
            }
            _ => return Err(invalid("unexpected collection instruction")),
        }
        if empty_amendment {
            self.end_collection()?;
        }
        Ok(())
    }

    fn end_collection(&mut self) -> Result<(), RuntimeError> {
        let id = self
            .frames
            .last_mut()
            .expect("active frame")
            .builders
            .pop()
            .ok_or_else(|| invalid("missing collection builder"))?;
        let object = self.heap.get(id)?;
        let result = if object.members.is_empty() {
            if let Some(parent) = object.parent {
                let prototype = self.heap.get(parent)?;
                if prototype.class_definition.is_some()
                    && matches!(
                        prototype.builtin_type.as_deref(),
                        Some(TypeCheck::Collection(_))
                    )
                {
                    self.empty_object(object.collection_kind)
                } else {
                    parent
                }
            } else if matches!(
                object.collection_kind,
                CollectionKind::Dynamic | CollectionKind::Listing | CollectionKind::Mapping
            ) && object.collection_default.is_none()
            {
                self.empty_object(object.collection_kind)
            } else {
                id
            }
        } else {
            id
        };
        let result = self.finish_strict_collection(result)?;
        self.stack.push(Value::Object(result));
        Ok(())
    }
}

impl Compiler<'_> {
    pub(super) fn instance_body(
        &mut self,
        members: &[crate::syntax::ObjectMember],
        parameters: &[crate::syntax::Parameter],
        span: Span,
        parameter_span: Span,
        output: &mut Code,
    ) -> Result<(), RuntimeError> {
        if members
            .iter()
            .all(|member| matches!(member, crate::syntax::ObjectMember::Property(_)))
        {
            let properties = Self::body_properties_only(members)?;
            let template = self.object(&properties)?;
            output.push_at(Instruction::MakeInstance { template }, span);
            if !parameters.is_empty() && !members.is_empty() {
                output.push_at(
                    Instruction::CheckObjectParameters {
                        target: amendment::ParameterTarget::Value,
                    },
                    parameter_span,
                );
            }
            self.initialize_member_types(&properties, false, output)?;
        } else {
            // Construct the typed parent first, preserving class validation.
            // Generator clauses then execute in source order, including
            // predicates that reject typed parents only when reached.
            if let Some(parameter) = parameters.first() {
                output.push_at(
                    Instruction::CheckObjectParameters {
                        target: amendment::ParameterTarget::Parent {
                            kind: CollectionKind::Dynamic,
                            has_parent: true,
                            typed: parameter.ty.is_some(),
                        },
                    },
                    parameter.ty.as_ref().map_or(parameter.span, |ty| ty.span),
                );
            }
            let template = self.object(&[])?;
            output.push_at(Instruction::MakeInstance { template }, span);
            self.collection_body(members, parameters, CollectionKind::Dynamic, true, output)?;
        }
        Ok(())
    }

    pub(super) fn body_properties_only(
        members: &[crate::syntax::ObjectMember],
    ) -> Result<Vec<crate::syntax::Member>, RuntimeError> {
        members
            .iter()
            .map(|member| match member {
                crate::syntax::ObjectMember::Property(member) => Ok(member.clone()),
                member => {
                    Err(unsupported_type("non-property typed object members").at(member.span()))
                }
            })
            .collect()
    }

    fn body_properties(
        members: &[crate::syntax::ObjectMember],
        result: &mut Vec<crate::syntax::Member>,
    ) {
        use crate::syntax::ObjectMember;
        for member in members {
            match member {
                ObjectMember::Property(member) => result.push(member.clone()),
                ObjectMember::When {
                    then_members,
                    else_members,
                    ..
                } => {
                    Self::body_properties(then_members, result);
                    Self::body_properties(else_members, result);
                }
                ObjectMember::For { members, .. } => Self::body_properties(members, result),
                _ => {}
            }
        }
    }

    pub(super) fn collection_body(
        &mut self,
        members: &[crate::syntax::ObjectMember],
        parameters: &[crate::syntax::Parameter],
        kind: CollectionKind,
        has_parent: bool,
        output: &mut Code,
    ) -> Result<(), RuntimeError> {
        let previous = std::mem::replace(&mut self.super_method_allowed, false);
        // Java ObjectScope owns entry numbering even when nested object bodies
        // share the same displayed qualified name (for example an entry key).
        let previous_entry_counts = std::mem::take(&mut self.entry_counts);
        let result = self.collection_body_scoped(members, parameters, kind, has_parent, output);
        self.entry_counts = previous_entry_counts;
        self.super_method_allowed = previous;
        result
    }

    fn collection_body_scoped(
        &mut self,
        members: &[crate::syntax::ObjectMember],
        parameters: &[crate::syntax::Parameter],
        kind: CollectionKind,
        has_parent: bool,
        output: &mut Code,
    ) -> Result<(), RuntimeError> {
        if has_parent && members.is_empty() {
            output.push(Instruction::AmendEmpty);
            return Ok(());
        }
        let mut properties = Vec::new();
        Self::body_properties(members, &mut properties);
        let generators = members.iter().any(|m| {
            matches!(
                m,
                crate::syntax::ObjectMember::When { .. }
                    | crate::syntax::ObjectMember::For { .. }
                    | crate::syntax::ObjectMember::Spread { .. }
                    | crate::syntax::ObjectMember::Predicate { .. }
            )
        });
        if let Some(parameter) = parameters.first().filter(|_| generators) {
            output.push_at(
                Instruction::CheckObjectParameters {
                    target: amendment::ParameterTarget::Parent {
                        kind,
                        has_parent,
                        typed: parameter.ty.is_some(),
                    },
                },
                parameter.ty.as_ref().map_or(parameter.span, |ty| ty.span),
            );
        }
        let amendment = if has_parent {
            Some(self.compile_function_amendment(parameters, output)?)
        } else {
            None
        };
        self.type_scopes.push(extract_type_scope(&properties));
        if let Some(first_generator) = members.iter().position(|member| {
            matches!(
                member,
                crate::syntax::ObjectMember::When { .. }
                    | crate::syntax::ObjectMember::For { .. }
                    | crate::syntax::ObjectMember::Spread { .. }
                    | crate::syntax::ObjectMember::Predicate { .. }
            )
        }) {
            self.project_generator_prefix(&members[..first_generator]);
        }
        output.push(Instruction::BeginCollection {
            kind,
            has_parent,
            specialized: !generators,
        });
        let mixed = members
            .iter()
            .any(|m| matches!(m, crate::syntax::ObjectMember::Element { .. }));
        let mut parameter_check = None;
        if generators {
            self.collection_members(members, false, output)?;
        } else {
            // Pinned specialized literals install keyed members before elements.
            for member in members
                .iter()
                .filter(|m| matches!(m, crate::syntax::ObjectMember::Property(_)))
            {
                self.collection_members(std::slice::from_ref(member), mixed, output)?;
            }
            if !parameters.is_empty() {
                parameter_check = Some(output.len());
                output.push(Instruction::CheckObjectParameters {
                    target: amendment::ParameterTarget::Builder {
                        amendment_end: None,
                    },
                });
            }
            for member in members
                .iter()
                .filter(|m| matches!(m, crate::syntax::ObjectMember::Entry { .. }))
            {
                self.collection_members(std::slice::from_ref(member), mixed, output)?;
            }
            for member in members
                .iter()
                .filter(|m| matches!(m, crate::syntax::ObjectMember::Element { .. }))
            {
                self.collection_members(std::slice::from_ref(member), mixed, output)?;
            }
        }
        output.push(Instruction::EndCollection);
        self.initialize_member_types(&properties, false, output)?;
        self.type_scopes.pop();
        if let Some(index) = amendment {
            let end = output.len();
            if let Instruction::BeginFunctionAmendment { end: target, .. } = &mut output[index] {
                *target = end;
            }
            if let Some(check) = parameter_check {
                output[check] = Instruction::CheckObjectParameters {
                    target: amendment::ParameterTarget::Builder {
                        amendment_end: Some(end),
                    },
                };
            }
            output.push(Instruction::EndFunctionAmendment);
        }
        Ok(())
    }

    /// AstBuilder first constructs ordinary members, then revisits the whole
    /// body when it encounters a generator. Only direct unlabeled elements and
    /// entries in the prefix advance this object scope's `entry_counts`, since
    /// nested object bodies save and restore `entry_counts` around their own scope.
    fn project_generator_prefix(&mut self, prefix: &[crate::syntax::ObjectMember]) {
        let mut unlabeled = 0usize;
        for member in prefix {
            match member {
                crate::syntax::ObjectMember::Element { .. } => unlabeled += 1,
                crate::syntax::ObjectMember::Entry { key, .. }
                    if Self::entry_key_node_label(key).is_none() =>
                {
                    unlabeled += 1;
                }
                _ => {}
            }
        }
        if unlabeled > 0 {
            *self.entry_counts.entry(self.member.clone()).or_default() += unlabeled;
        }
    }

    /// Match the scalar ConstantNode values that Java uses for entry names.
    /// Parentheses lower to their child; arbitrary constant folding does not.
    fn entry_key_node_label(key: &crate::syntax::Expr) -> Option<PklString> {
        use crate::syntax::{ExprKind, UnaryOp};

        fn numeric_literal(expr: &crate::syntax::Expr) -> Option<f64> {
            match &expr.unparenthesized().kind {
                ExprKind::Int(value) => Some(*value as f64),
                ExprKind::Float(value) => Some(*value),
                ExprKind::Unary {
                    op: UnaryOp::Negate,
                    operand,
                } => match &operand.kind {
                    ExprKind::Float(value) => Some(-value),
                    _ => None,
                },
                _ => None,
            }
        }

        match &key.unparenthesized().kind {
            ExprKind::String(value) => {
                Some(PklString::from("\"").concat(value).concat(&"\"".into()))
            }
            ExprKind::Int(value) => Some(value.to_string().into()),
            ExprKind::Float(value) => Some(crate::numeric::double_to_string(*value).into()),
            ExprKind::Bool(value) => Some(value.to_string().into()),
            ExprKind::Unary {
                op: UnaryOp::Negate,
                operand,
            } => match &operand.kind {
                ExprKind::Float(value) => Some(crate::numeric::double_to_string(-value).into()),
                _ => None,
            },
            ExprKind::Member(receiver, name) | ExprKind::NullableMember(receiver, name) => {
                let value = numeric_literal(receiver)?;
                let unit = name.as_str()?;
                if let Some(unit) = crate::duration::DurationUnit::parse(unit) {
                    Some(
                        crate::duration::Duration::new(value, unit)
                            .to_string()
                            .into(),
                    )
                } else {
                    crate::data_size::DataSizeUnit::parse(unit).map(|unit| {
                        crate::data_size::DataSize::new(value, unit)
                            .to_string()
                            .into()
                    })
                }
            }
            _ => None,
        }
    }

    fn lazy_collection_member(
        &mut self,
        value: &crate::syntax::Expr,
        key: Option<&crate::syntax::Expr>,
        span: Span,
    ) -> Result<Member, RuntimeError> {
        let previous = std::mem::replace(&mut self.constructor_parameters_bound, false);
        let result = self.lazy_collection_member_body(value, key, span);
        self.constructor_parameters_bound = previous;
        result
    }

    fn lazy_collection_member_body(
        &mut self,
        value: &crate::syntax::Expr,
        key: Option<&crate::syntax::Expr>,
        span: Span,
    ) -> Result<Member, RuntimeError> {
        let previous = self.member.clone();
        let literal = key.and_then(Self::entry_key_node_label);
        let label = literal.unwrap_or_else(|| {
            let count = self.entry_counts.entry(previous.clone()).or_default();
            *count += 1;
            format!("#{count}").into()
        });
        self.member = if let (Some(p), Some(l)) = (previous.as_str(), label.as_str()) {
            let mut s = String::with_capacity(p.len() + l.len() + 2);
            s.push_str(p);
            s.push('[');
            s.push_str(l);
            s.push(']');
            PklString::from(s)
        } else {
            previous
                .concat(&"[".into())
                .concat(&label)
                .concat(&"]".into())
        };
        let mut code = Code::default();
        self.expression_with_entry_parent(value, &mut code)?;
        let is_constant = self.constant_member_body(value);
        if is_constant {
            for instruction in &mut code.instructions {
                if let Instruction::Constant(value) = instruction {
                    *value = value.clone().boxed();
                }
            }
        }
        code.push(Instruction::Return);
        let block = self.block(code, Some(span))?;
        self.program.metadata[block].member_span = Some(span);
        self.program.metadata[block].value_span = Some(if is_constant {
            self.constant_member_value_span(
                value,
                self.executable_value_span(key.unwrap_or(value)).end,
                key.is_some(),
            )
        } else {
            self.executable_value_span(value)
        });
        self.member = previous;
        Ok(Member {
            flags: 0,
            entry: true,
            name: PklString::default(),
            code: block,
            local: false,
            method: false,
            ty: None,
            type_span: None,
            constant: false,
            fixed: false,
        })
    }

    fn collection_eager_expression(
        &mut self,
        expression: &crate::syntax::Expr,
        output: &mut Code,
    ) -> Result<(), RuntimeError> {
        let scope = self.type_scopes.pop().expect("collection scope");
        let result = self.expression(expression, output);
        self.type_scopes.push(scope);
        result
    }

    fn collection_members(
        &mut self,
        members: &[crate::syntax::ObjectMember],
        mixed: bool,
        output: &mut Code,
    ) -> Result<(), RuntimeError> {
        use crate::syntax::ObjectMember;
        for member in members {
            match member {
                ObjectMember::Property(member) => {
                    let scope = self.type_scopes.pop().expect("collection scope");
                    let template = self.object(std::slice::from_ref(member));
                    self.type_scopes.push(scope);
                    let template = template?;
                    let compiled = self.program.objects[template][0].clone();
                    let span = if let crate::syntax::ExprKind::Function {
                        method: true,
                        header_span,
                        ..
                    } = &member.expr.kind
                    {
                        Span::new(member.span.start, header_span.end)
                    } else {
                        member.name_span
                    };
                    output.push_at(Instruction::AddCollectionProperty(Box::new(compiled)), span);
                }
                ObjectMember::Element { value, span } => {
                    let member = self.lazy_collection_member(value, None, value.span)?;
                    output.push_at(Instruction::AddCollectionElement(Box::new(member)), *span);
                }
                ObjectMember::Entry { key, value, span } => {
                    self.collection_eager_expression(key, output)?;
                    let member = self.lazy_collection_member(value, Some(key), key.span)?;
                    let literal = matches!(
                        key.kind,
                        crate::syntax::ExprKind::Int(_)
                            | crate::syntax::ExprKind::Float(_)
                            | crate::syntax::ExprKind::String(_)
                            | crate::syntax::ExprKind::Bool(_)
                            | crate::syntax::ExprKind::Null
                    );
                    output.push_at(
                        Instruction::AddCollectionEntry {
                            member: Box::new(member),
                            check_index: mixed && !literal,
                            span: *span,
                        },
                        key.span,
                    );
                }
                ObjectMember::When {
                    condition,
                    then_members,
                    else_members,
                    ..
                } => {
                    self.collection_eager_expression(condition, output)?;
                    let branch = output.len();
                    output.push_at(Instruction::JumpIfFalse(0), condition.span);
                    self.collection_members(then_members, false, output)?;
                    let end = output.len();
                    output.push(Instruction::Jump(0));
                    output[branch] = Instruction::JumpIfFalse(output.len());
                    self.collection_members(else_members, false, output)?;
                    output[end] = Instruction::Jump(output.len());
                }
                ObjectMember::For {
                    parameters,
                    iterable,
                    members,
                    span,
                    ..
                } => {
                    let scope = self.type_scopes.pop().expect("collection scope");
                    let checks = parameters
                        .iter()
                        .map(|parameter| {
                            parameter
                                .ty
                                .as_ref()
                                .and_then(|ty| self.resolve_expression_type(ty, output))
                        })
                        .collect::<Vec<_>>();
                    self.type_scopes.push(scope);
                    self.collection_eager_expression(iterable, output)?;
                    output.push_at(
                        Instruction::OpenIterator {
                            nullable: false,
                            spread: false,
                        },
                        iterable.unparenthesized().span,
                    );
                    let start = output.len();
                    output.push_at(Instruction::IteratorNext { end: 0 }, iterable.span);
                    output.push(Instruction::EnterForScope {
                        bindings: parameters.len(),
                    });
                    if parameters.len() == 1 {
                        output.push(Instruction::Swap);
                        output.push(Instruction::Pop);
                        let ty = checks[0].clone().unwrap_or(TypeCheck::Unknown);
                        let span = parameters[0]
                            .ty
                            .as_ref()
                            .map_or(parameters[0].span, |ty| ty.span);
                        output.push_at(
                            Instruction::BindLocal {
                                depth: None,
                                ty: Box::new(ty),
                            },
                            span,
                        );
                    } else if parameters.len() == 2 {
                        output.push(Instruction::Swap);
                        let ty = checks[0].clone().unwrap_or(TypeCheck::Unknown);
                        let span = parameters[0]
                            .ty
                            .as_ref()
                            .map_or(parameters[0].span, |ty| ty.span);
                        output.push_at(
                            Instruction::BindLocal {
                                depth: None,
                                ty: Box::new(ty),
                            },
                            span,
                        );
                        let ty = checks[1].clone().unwrap_or(TypeCheck::Unknown);
                        let span = parameters[1]
                            .ty
                            .as_ref()
                            .map_or(parameters[1].span, |ty| ty.span);
                        output.push_at(
                            Instruction::BindLocal {
                                depth: None,
                                ty: Box::new(ty),
                            },
                            span,
                        );
                    } else {
                        return Err(unsupported_type("this generator parameter count").at(*span));
                    }
                    self.collection_members(members, false, output)?;
                    output.push(Instruction::LeaveForScope);
                    output.push(Instruction::Jump(start));
                    output[start] = Instruction::IteratorNext { end: output.len() };
                    output.push(Instruction::Pop);
                }
                ObjectMember::Spread {
                    value,
                    nullable,
                    span,
                } => {
                    self.collection_eager_expression(value, output)?;
                    output.push_at(Instruction::CheckMapSpread, *span);
                    output.push_at(
                        Instruction::OpenIterator {
                            nullable: *nullable,
                            spread: true,
                        },
                        value.unparenthesized().span,
                    );
                    let start = output.len();
                    output.push_at(Instruction::IteratorNext { end: 0 }, value.span);
                    output.push_at(Instruction::SpreadMember, *span);
                    output.push(Instruction::Jump(start));
                    output[start] = Instruction::IteratorNext { end: output.len() };
                    output.push(Instruction::Pop);
                }
                ObjectMember::Predicate {
                    condition, value, ..
                } => {
                    output.push_at(Instruction::OpenPredicate, condition.span);
                    let start = output.len();
                    output.push_at(Instruction::PredicateNext { end: 0 }, condition.span);
                    self.collection_eager_expression(condition, output)?;
                    let member =
                        self.lazy_collection_member(value, Some(condition), condition.span)?;
                    output.push_at(
                        Instruction::AddPredicateMember(Box::new(member)),
                        condition.span,
                    );
                    output.push(Instruction::Jump(start));
                    output[start] = Instruction::PredicateNext { end: output.len() };
                }
            }
        }
        Ok(())
    }
}
