//! Strict collection addition resumes at the authored `+` instruction.
//! Hash/equality continuations retry that instruction before publishing a member.
use super::*;
use crate::heap::{CollectionKind, CollectionMember};

#[derive(Clone, Copy)]
enum Phase {
    Read,
    AppendList,
    ContainsSet,
    InsertSet,
    ContainsMap,
    AssociateMap,
}

pub(super) struct Run {
    left: ObjectId,
    right: ObjectId,
    builder: ObjectId,
    kind: CollectionKind,
    members: Vec<CollectionMember>,
    next: usize,
    phase: Phase,
    current: Option<Value>,
    append_order: bool,
}

impl Run {
    pub(super) fn roots(&self, roots: &mut Vec<Value>) {
        roots.extend([self.left, self.right, self.builder].map(Value::Object));
        roots.extend(self.members.iter().map(|member| member.key.clone()));
        roots.extend(self.current.iter().cloned());
    }
}

impl Vm {
    #[cfg_attr(target_arch = "wasm32", inline(never))]
    pub(super) fn begin_collection_addition(
        &mut self,
        left: &Value,
        right: &Value,
    ) -> Result<bool, RuntimeError> {
        let (Value::Object(left), Value::Object(right)) = (left, right) else {
            return Ok(false);
        };
        let source = self.heap.get(*left)?;
        let kind = source.collection_kind;
        let right_kind = self.heap.get(*right)?.collection_kind;
        if !matches!(
            (kind, right_kind),
            (
                CollectionKind::List | CollectionKind::Set,
                CollectionKind::List | CollectionKind::Set
            ) | (CollectionKind::Map, CollectionKind::Map)
        ) {
            return Ok(false);
        }
        if self.heap.get(*right)?.length == 0 {
            self.stack.push(Value::Object(*left));
            return Ok(true);
        }
        if kind == CollectionKind::List {
            let right_obj = self.heap.get(*right)?;
            if let Some(left_elems) = &source.list_elements
                && let Some(right_elems) = &right_obj.list_elements
            {
                let total_len =
                    left_elems
                        .len()
                        .checked_add(right_elems.len())
                        .ok_or_else(|| {
                            RuntimeError::new(ErrorKind::Type, "List size exceeds maximum length.")
                        })?;
                if total_len > collections::MAX_MATERIALIZED_REPEAT_LIST {
                    return Err(RuntimeError::new(
                        ErrorKind::Type,
                        format!(
                            "List of length {} exceeds maximum materialized length ({}).",
                            total_len,
                            collections::MAX_MATERIALIZED_REPEAT_LIST
                        ),
                    ));
                }
                let mut combined = Vec::new();
                combined.try_reserve_exact(total_len).map_err(|_| {
                    RuntimeError::new(ErrorKind::Type, "List size exceeds available memory.")
                })?;
                combined.extend_from_slice(left_elems);
                combined.extend_from_slice(right_elems);
                let built = self.build_strict_list(combined);
                self.stack.push(Value::Object(built));
                return Ok(true);
            }
            // Virtual repeated list + List/Set: materialize left prefix (bounded) so
            // `repeat_base_len` is never silently dropped while preserving `length`.
            if let Some(base_len) = source.repeat_base_len
                && source.list_elements.is_none()
                && source.collection_members.len() < source.length
            {
                let right_len = right_obj.length;
                let total_len = source.length.checked_add(right_len).ok_or_else(|| {
                    RuntimeError::new(ErrorKind::Type, "List size exceeds maximum length.")
                })?;
                if total_len > collections::MAX_MATERIALIZED_REPEAT_LIST {
                    return Err(RuntimeError::new(
                        ErrorKind::Type,
                        format!(
                            "List of length {} exceeds maximum materialized length ({}).",
                            total_len,
                            collections::MAX_MATERIALIZED_REPEAT_LIST
                        ),
                    ));
                }
                let mut combined = Vec::new();
                combined.try_reserve_exact(total_len).map_err(|_| {
                    RuntimeError::new(ErrorKind::Type, "List size exceeds available memory.")
                })?;
                if base_len > 0 {
                    for idx in 0..source.length {
                        let val = source
                            .cache_slots
                            .get(idx % base_len)
                            .map(|s| s.value.clone())
                            .ok_or_else(|| invalid("missing virtual repeated list slot"))?;
                        combined.push(val);
                    }
                }
                if let Some(right_elems) = &right_obj.list_elements {
                    combined.extend_from_slice(right_elems);
                    let built = self.build_strict_list(combined);
                    self.stack.push(Value::Object(built));
                    return Ok(true);
                }
                let right_members = collections::ordered_members(&self.heap, *right)?;
                for member in &right_members {
                    let val = collections::cached_render_value(&self.heap, *right, member)?
                        .ok_or_else(|| invalid("addition strict element"))?;
                    combined.push(val);
                }
                let built = self.build_strict_list(combined);
                self.stack.push(Value::Object(built));
                return Ok(true);
            }
        }
        // Copy persistent strict storage, not the producer's environment,
        // executable owner, generic cast, default, or structural/hash caches.
        // Rebuilding the left prefix would rehash keys that Java does not visit.
        let builder = self.heap.allocate(Object {
            collection_kind: kind,
            length: source.length,
            list_elements: source.list_elements.clone(),
            members: source.members.clone(),
            collection_members: source.collection_members.clone(),
            member_values: source.member_values.clone(),
            cache_map: source.cache_map.clone(),
            cache_slots: source.cache_slots.clone(),
            has_object_collection_key: source.has_object_collection_key,
            cache_index: source.cache_index.clone(),
            map_index: source.map_index.clone(),
            map_key_order: source.map_key_order.clone(),
            set_entries: source.set_entries.clone(),
            ..Object::default()
        });
        let members = collections::ordered_members(&self.heap, *right)?;
        let frame = self.frames.last_mut().expect("addition frame");
        frame.builders.push(builder);
        frame.collection_addition = Some(Box::new(Run {
            left: *left,
            right: *right,
            builder,
            kind,
            members,
            next: 0,
            phase: Phase::Read,
            current: None,
            append_order: false,
        }));
        frame.pc -= 1;
        Ok(true)
    }

    #[cfg_attr(target_arch = "wasm32", inline(never))]
    pub(super) fn step_collection_addition(&mut self) -> Result<(), RuntimeError> {
        let run = self
            .frames
            .last()
            .unwrap()
            .collection_addition
            .as_ref()
            .unwrap();
        let (left, right, builder, kind, phase) =
            (run.left, run.right, run.builder, run.kind, run.phase);
        let Some(member) = run.members.get(run.next).cloned() else {
            let frame = self.frames.last_mut().unwrap();
            frame.collection_addition = None;
            assert_eq!(frame.builders.pop(), Some(builder));
            self.stack.push(Value::Object(builder));
            return Ok(());
        };
        let value = run.current.clone();
        match phase {
            Phase::Read => {
                let value = if kind == CollectionKind::Map {
                    if self.prepare_map_lookup(right, &member.key)? {
                        return Ok(());
                    }
                    self.map_value(right, &member.key)?
                        .ok_or_else(|| invalid("addition Map key"))?
                } else {
                    collections::cached_render_value(&self.heap, right, &member)?
                        .ok_or_else(|| invalid("addition strict element"))?
                };
                let run = self
                    .frames
                    .last_mut()
                    .unwrap()
                    .collection_addition
                    .as_mut()
                    .unwrap();
                run.current = Some(value);
                run.phase = match kind {
                    CollectionKind::List => Phase::AppendList,
                    CollectionKind::Set => Phase::ContainsSet,
                    CollectionKind::Map => Phase::ContainsMap,
                    _ => unreachable!(),
                };
            }
            Phase::AppendList => {
                self.stack.push(value.unwrap());
                self.append_strict_element()?;
                self.advance_collection_addition();
            }
            Phase::ContainsSet => {
                let value = value.unwrap();
                if self.prepare_set_lookup(builder, &value)? {
                    return Ok(());
                }
                if self.set_contains(builder, &value)? {
                    self.advance_collection_addition();
                } else {
                    self.frames
                        .last_mut()
                        .unwrap()
                        .collection_addition
                        .as_mut()
                        .unwrap()
                        .phase = Phase::InsertSet;
                }
            }
            Phase::InsertSet => {
                let value = value.unwrap();
                if self.put_set_key(&value)? {
                    return Ok(());
                }
                self.stack.push(value);
                self.append_strict_element()?;
                self.advance_collection_addition();
            }
            Phase::ContainsMap => {
                // VmMap.concatenate consults the original left map for key
                // order, separately from associating in the mutable result.
                if self.prepare_map_lookup(left, &member.key)? {
                    return Ok(());
                }
                let append = self.map_value(left, &member.key)?.is_none();
                let run = self
                    .frames
                    .last_mut()
                    .unwrap()
                    .collection_addition
                    .as_mut()
                    .unwrap();
                run.append_order = append;
                run.phase = Phase::AssociateMap;
            }
            Phase::AssociateMap => {
                if self.prepare_map_association(builder, &member.key)? {
                    return Ok(());
                }
                let append = self
                    .frames
                    .last()
                    .unwrap()
                    .collection_addition
                    .as_ref()
                    .unwrap()
                    .append_order;
                self.insert_map_entry_ordered(builder, member.key, value.unwrap(), Some(append))?;
                self.advance_collection_addition();
            }
        }
        self.frames.last_mut().unwrap().pc -= 1;
        Ok(())
    }

    fn advance_collection_addition(&mut self) {
        let run = self
            .frames
            .last_mut()
            .unwrap()
            .collection_addition
            .as_mut()
            .unwrap();
        run.next += 1;
        run.current = None;
        run.phase = Phase::Read;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn result(vm: &mut Vm, roots: &[Value]) -> Value {
        for _ in 0..100_000 {
            vm.collect(roots);
            match vm.poll(1) {
                VmPoll::Yielded => {}
                VmPoll::Ready(value) => return value.unwrap(),
                event => panic!("{event:?}"),
            }
        }
        panic!("addition poll bound")
    }

    fn source(kind: CollectionKind, empty: bool) -> Vec<Instruction> {
        use Instruction as I;
        let mut code = vec![I::BeginCollection {
            kind,
            has_parent: false,
            specialized: true,
        }];
        if !empty {
            if kind == CollectionKind::Map {
                code.push(I::Constant(Value::string("key")));
            }
            code.push(I::Constant(Value::Int(1.into())));
            match kind {
                CollectionKind::List => code.push(I::AppendStrictElement),
                CollectionKind::Set => code.extend([I::PrepareSetElement, I::AddSetElement]),
                CollectionKind::Map => code.extend([I::PrepareMapEntry, I::AddMapEntry]),
                _ => unreachable!(),
            }
        }
        code.extend([I::EndCollection, I::Return]);
        code
    }

    #[test]
    fn empty_right_keeps_handle_nonempty_right_allocates_without_producer_roots() {
        use Instruction as I;
        for (left_kind, right_kind) in [
            (CollectionKind::List, CollectionKind::List),
            (CollectionKind::List, CollectionKind::Set),
            (CollectionKind::Set, CollectionKind::List),
            (CollectionKind::Set, CollectionKind::Set),
            (CollectionKind::Map, CollectionKind::Map),
        ] {
            for empty in [false, true] {
                let mut vm = Vm::new(Arc::new(Program::new(vec![
                    source(left_kind, false),
                    source(right_kind, empty),
                    vec![I::Input(0), I::Input(1), I::Add, I::Return],
                    vec![I::Constant(Value::Null), I::Return],
                ])));
                vm.start(0, vec![]).unwrap();
                let left = result(&mut vm, &[]);
                vm.start(1, vec![]).unwrap();
                let right = result(&mut vm, std::slice::from_ref(&left));
                let left_id = left.as_object().unwrap();
                let producer = vm.heap.allocate(Object::default());
                vm.heap
                    .get_mut(left_id)
                    .unwrap()
                    .environment
                    .locals
                    .push(Value::Object(producer));
                vm.start(2, vec![left.clone(), right]).unwrap();
                let added = result(&mut vm, &[]);
                assert_eq!(added == left, empty, "{left_kind:?}+{right_kind:?}");
                if !empty {
                    let added_id = added.as_object().unwrap();
                    vm.start(3, vec![]).unwrap();
                    assert_eq!(result(&mut vm, std::slice::from_ref(&added)), Value::Null);
                    vm.collect(std::slice::from_ref(&added));
                    assert!(vm.heap.get(left_id).is_err());
                    assert!(vm.heap.get(producer).is_err());
                    let object = vm.heap.get(added_id).unwrap();
                    assert!(object.environment.locals.is_empty());
                    assert!(object.member_environments.is_empty());
                    assert!(object.parent.is_none());
                    assert_eq!(object.collection_kind, left_kind);
                }
                vm.close();
                assert_eq!(vm.heap_stats().slots, 0);
            }
        }
    }

    #[test]
    fn map_overwrite_releases_old_value_and_retains_right_value() {
        use Instruction as I;
        let mut map = source(CollectionKind::Map, false);
        map[2] = I::Input(0);
        let mut vm = Vm::new(Arc::new(Program::new(vec![
            map,
            vec![I::Input(0), I::Input(1), I::Add, I::Return],
            vec![I::Constant(Value::Null), I::Return],
        ])));
        let old = vm.heap.allocate(Object::default());
        let new = vm.heap.allocate(Object::default());
        vm.start(0, vec![Value::Object(old)]).unwrap();
        let left = result(&mut vm, &[Value::Object(new)]);
        vm.start(0, vec![Value::Object(new)]).unwrap();
        let right = result(&mut vm, std::slice::from_ref(&left));
        vm.start(1, vec![left, right]).unwrap();
        let added = result(&mut vm, &[]);
        vm.start(2, vec![]).unwrap();
        result(&mut vm, std::slice::from_ref(&added));
        vm.collect(std::slice::from_ref(&added));
        assert!(vm.heap.get(old).is_err());
        assert!(vm.heap.get(new).is_ok());
        assert_eq!(
            vm.map_value(added.as_object().unwrap(), &Value::string("key"))
                .unwrap(),
            Some(Value::Object(new))
        );
        vm.close();
        assert_eq!(vm.heap_stats().slots, 0);
    }
}
