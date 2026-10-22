//! Equality and key hashing share the ordinary VM scheduler and lazy caches.
//!
//! Expression equality coerces numeric operands; recursive comparisons use Java
//! boxed equality. Object equality forces left and then right depth first, while
//! strict List equality compares one element pair at a time. Hash preparation is
//! separate: small EconomicMaps compare keys without hashing until their fifth
//! distinct insertion. Canonical key IDs encode equality, not Java hash integers.
//! They are published only after successful preparation and reclaimed by the heap.
//!
//! Every operation that can evaluate Pkl is a task or an ordinary member frame.
//! Failed key searches use explicit task checkpoints, so nested equality can
//! suspend, reenter the Host, and fail without losing the outer search.
use super::*;
use crate::heap::{CollectionKind, CollectionMember, CollectionMemberKind};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Atom {
    Null,
    Bool(bool),
    Int(i64),
    Float(u64),
    String(PklString),
    Bytes(Arc<[u8]>),
    Structural(u64),
    Function(ObjectId),
    Class(ObjectId),
    TypeAlias(ObjectId),
    ReferenceType(String),
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct StructuralKey {
    kind: u8,
    class: Option<ObjectId>,
    members: Vec<(bool, Atom, Atom)>,
}
impl StructuralKey {
    pub(crate) fn children(&self) -> impl Iterator<Item = u64> + '_ {
        self.members
            .iter()
            .flat_map(|(_, k, v)| [k, v])
            .filter_map(|a| match a {
                Atom::Structural(id) => Some(*id),
                _ => None,
            })
    }
}
fn atom(heap: &Heap, value: &Value) -> Result<Atom, RuntimeError> {
    Ok(match value {
        Value::Null | Value::NullWithDefault(_) => Atom::Null,
        Value::Bool(v) => Atom::Bool(*v),
        Value::Int(v) => Atom::Int(*v),
        Value::Float(v) => Atom::Float(if v.is_nan() {
            f64::NAN.to_bits()
        } else {
            v.to_bits()
        }),
        Value::String(v) => Atom::String(v.clone()),
        Value::Bytes(v) => Atom::Bytes(v.clone()),
        Value::Function(v) => Atom::Function(*v),
        Value::Class(v) => Atom::Class(*v),
        Value::TypeAlias(v) => Atom::TypeAlias(*v),
        Value::Object(v) => Atom::Structural(
            heap.get(*v)?
                .structural_key
                .ok_or_else(|| invalid("unprepared structural key"))?,
        ),
    })
}
fn string_hash(value: &str) -> i32 {
    value.encode_utf16().fold(0i32, |hash, unit| {
        hash.wrapping_mul(31).wrapping_add(i32::from(unit))
    })
}

impl Vm {
    fn needs_key_hash(&self, id: ObjectId) -> Result<bool, RuntimeError> {
        let object = self.heap.get(id)?;
        Ok(matches!(
            object.collection_kind,
            CollectionKind::List | CollectionKind::Map | CollectionKind::Set
        ) || matches!(object.java_hash, None | Some(Some(0))))
    }

    pub(super) fn java_hash(&self, value: &Value) -> Result<Option<i32>, RuntimeError> {
        Ok(match value {
            Value::Null | Value::NullWithDefault(_) => Some(0),
            Value::Bool(value) => Some(if *value { 1231 } else { 1237 }),
            Value::Int(value) => Some((*value as u64 ^ ((*value as u64) >> 32)) as i32),
            Value::Float(value) => {
                let bits = if value.is_nan() {
                    0x7ff8000000000000
                } else {
                    value.to_bits()
                };
                Some((bits ^ (bits >> 32)) as i32)
            }
            Value::String(value) => Some(value.encode_utf16().fold(0i32, |hash, unit| {
                hash.wrapping_mul(31).wrapping_add(i32::from(unit))
            })),
            Value::Bytes(value) => Some(value.iter().fold(1i32, |hash, byte| {
                hash.wrapping_mul(31).wrapping_add(i32::from(*byte as i8))
            })),
            Value::Function(_) => None,
            Value::TypeAlias(id) => self.heap.get(*id)?.type_alias.as_ref().map(|name| {
                if name.contains('#') {
                    string_hash(name)
                } else {
                    string_hash(&format!("pkl.base#{name}"))
                }
            }),
            Value::Class(id) => {
                self.heap.get(*id)?.class_definition.as_ref().map(|class| {
                    string_hash(&format!("{}#{}", self.program.module_name, class.name))
                })
            }
            Value::Object(id) => self.heap.get(*id)?.java_hash.flatten(),
        })
    }
}

pub(super) fn identical_key(heap: &Heap, left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Object(a), Value::Object(b)) => a == b,
        _ => same_key(heap, left, right),
    }
}

pub(super) fn same_key(heap: &Heap, left: &Value, right: &Value) -> bool {
    if let (Value::Object(a), Value::Object(b)) = (left, right) {
        if a == b {
            return true;
        }
        return heap.key_equalities.contains(&(*a, *b));
    }
    matches!((atom(heap, left), atom(heap, right)), (Ok(a), Ok(b)) if a == b)
}
#[derive(Clone)]
enum Task {
    CountKey(ObjectId, Vec<Value>, usize, Vec<(Value, Option<i32>)>),
    CountReady(ObjectId, Vec<Value>, usize, Vec<(Value, Option<i32>)>),
    CountKeys(
        ObjectId,
        Vec<Value>,
        usize,
        Vec<(Value, Option<i32>)>,
        usize,
        Option<i32>,
    ),
    CountMatched(
        ObjectId,
        Vec<Value>,
        usize,
        Vec<(Value, Option<i32>)>,
        Value,
    ),
    PublishCache(ObjectId, CacheKey, Value),
    AppendCache(ObjectId, CacheKey, Value),
    CacheRebuild(ObjectId, usize),
    CacheLink(ObjectId, usize, bool),
    ResumeForce(ObjectId, ObjectId, Arc<str>, bool, bool, bool, bool, bool),
    Reject,
    Compare(Value, Value, bool),
    CompareMembers(ObjectId, ObjectId),
    MapNext(ObjectId, ObjectId, usize),
    SetNext(ObjectId, ObjectId, usize),
    SetLookup(ObjectId, Value),
    MapLookup(ObjectId, ObjectId, CollectionMember),
    Deep(Value),
    Match(Value, Value, Vec<(Value, Value)>, usize),
    Matched(Value, Value, Value, Value),
    Prepare(Value),
    HashMembers(ObjectId),
    Build(ObjectId),
    Force(ObjectId, CollectionMember, bool),
    Resolve(
        ObjectId,
        CollectionMember,
        Vec<CollectionMember>,
        usize,
        Option<bool>,
    ),
    Resolved(ObjectId, CollectionMember, Value, Option<bool>),
    Read(ObjectId, CollectionMember, Option<bool>),
    Await(bool),
}
pub(super) struct EqualityRun {
    tasks: Vec<Task>,
    // A failed nested key comparison tries the next candidate; a value mismatch
    // without a checkpoint ends the enclosing equality operation.
    on_false: Vec<(usize, Task)>,
    active: BTreeSet<ObjectId>,
    forced: BTreeSet<ObjectId>,
    roots: Vec<Value>,
    equal: bool,
    negate: bool,
    retry: bool,
    prepared_hashes: Option<Vec<Value>>,
    key_search: bool,
    pub(super) hide_callsite: bool,
    forward: bool,
    pub(super) forward_to_type: bool,
}
pub(super) struct KeySearch {
    key: Value,
    candidates: Vec<Value>,
    position: usize,
    returned: Option<bool>,
}
impl KeySearch {
    pub(super) fn roots(&self, roots: &mut Vec<Value>) {
        roots.push(self.key.clone());
        roots.extend(self.candidates.iter().cloned());
    }
}
impl EqualityRun {
    pub(super) fn retries_instruction(&self) -> bool {
        self.retry || self.key_search
    }

    fn reject(&mut self) {
        if let Some((depth, task)) = self.on_false.pop() {
            self.tasks.truncate(depth);
            self.tasks.push(task);
        } else {
            self.equal = false;
            self.tasks.clear();
        }
    }

    pub(super) fn roots(&self, roots: &mut Vec<Value>) {
        roots.extend(self.roots.iter().cloned());
        roots.extend(self.prepared_hashes.iter().flatten().cloned());
    }
}
impl Vm {
    fn schedule_tasks(&mut self, tasks: Vec<Task>, roots: Vec<Value>) {
        let frame = self.frames.last_mut().expect("active frame");
        let run = frame.equality_run.get_or_insert_with(|| EqualityRun {
            tasks: Vec::new(),
            on_false: Vec::new(),
            active: BTreeSet::new(),
            forced: BTreeSet::new(),
            roots: Vec::new(),
            equal: true,
            negate: false,
            retry: false,
            prepared_hashes: None,
            key_search: false,
            hide_callsite: true,
            forward: true,
            forward_to_type: false,
        });
        run.tasks.extend(tasks);
        run.roots.extend(roots);
    }

    pub(super) fn cache_value_key(
        &self,
        receiver: ObjectId,
        key: &CacheKey,
    ) -> Result<Option<Value>, RuntimeError> {
        if !key.entry || key.owner.is_some() {
            return Ok(None);
        }
        let mut owner = Some(receiver);
        while let Some(id) = owner {
            let object = self.heap.get(id)?;
            if let Some(member) = object.collection_members.iter().find(|member| {
                member.name == key.name && member.kind != CollectionMemberKind::Property
            }) {
                return Ok(Some(member.key.clone()));
            }
            owner = object.parent;
        }
        Err(invalid("missing cache entry key"))
    }

    fn cache_key_hash(&self, receiver: ObjectId, key: &CacheKey) -> Result<i32, RuntimeError> {
        // Property/local identity hashes are evaluator-local bucket assignments;
        // they are not claimed to reproduce JVM identity allocation.
        Ok(if let Some(value) = self.cache_value_key(receiver, key)? {
            self.java_hash(&value)?.unwrap_or(0)
        } else {
            string_hash(&key.name) ^ key.owner.map_or(0, |owner| owner.index as i32)
        })
    }

    pub(super) fn cached_slot(
        &self,
        receiver: ObjectId,
        key: &CacheKey,
    ) -> Result<Option<Value>, RuntimeError> {
        let object = self.heap.get(receiver)?;
        let indices = if object.cache_index.len() == object.cache_slots.len() {
            object
                .cache_index
                .candidates(Some(self.cache_key_hash(receiver, key)?))
        } else {
            (0..object.cache_slots.len()).collect()
        };
        Ok(indices.into_iter().find_map(|index| {
            let slot = &object.cache_slots[index];
            (slot.key == *key).then(|| slot.value.clone())
        }))
    }

    pub(super) fn complete_cache(
        &mut self,
        receiver: ObjectId,
        key: CacheKey,
        value: Value,
    ) -> Result<(), RuntimeError> {
        let mut tasks = vec![Task::PublishCache(receiver, key.clone(), value.clone())];
        if self.heap.get(receiver)?.cache_index.has_hash()
            && let Some(value_key) = self.cache_value_key(receiver, &key)?
        {
            tasks.push(Task::Prepare(value_key));
        }
        self.schedule_tasks(tasks, vec![Value::Object(receiver), value]);
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn prepare_cache_read(
        &mut self,
        receiver: ObjectId,
        owner: ObjectId,
        name: Arc<str>,
        local: bool,
        memoize: bool,
        method: bool,
        entry: bool,
        predicate: bool,
    ) -> Result<bool, RuntimeError> {
        let key = CacheKey {
            name: name.clone(),
            owner: local.then_some(owner),
            method,
            entry,
        };
        if self.heap.get(receiver)?.cache_index.has_hash()
            && let Some(value) = self.cache_value_key(receiver, &key)?
            && matches!(&value, Value::Object(id) if self.needs_key_hash(*id)?)
        {
            self.schedule_tasks(
                vec![
                    Task::ResumeForce(
                        receiver, owner, name, local, memoize, method, entry, predicate,
                    ),
                    Task::Prepare(value.clone()),
                ],
                vec![Value::Object(receiver), Value::Object(owner), value],
            );
            return Ok(true);
        }
        Ok(false)
    }

    pub(super) fn count_mapping_keys(&mut self, id: ObjectId, keys: Vec<Value>) {
        let mut roots = keys.clone();
        roots.push(Value::Object(id));
        self.schedule_tasks(vec![Task::CountKey(id, keys, 0, Vec::new())], roots);
        let run = self
            .frames
            .last_mut()
            .expect("active frame")
            .equality_run
            .as_mut()
            .expect("mapping count");
        run.retry = true;
        run.hide_callsite = false;
    }

    pub(super) fn begin_equality(&mut self, left: Value, right: Value, negate: bool) {
        self.frames.last_mut().expect("active frame").equality_run = Some(EqualityRun {
            tasks: vec![Task::Compare(left.clone(), right.clone(), true)],
            on_false: Vec::new(),
            active: BTreeSet::new(),
            forced: BTreeSet::new(),
            roots: vec![left, right],
            equal: true,
            negate,
            retry: false,
            prepared_hashes: None,
            key_search: false,
            hide_callsite: true,
            forward: false,
            forward_to_type: false,
        });
    }
    pub(super) fn force_collection_member(
        &mut self,
        id: ObjectId,
        member: CollectionMember,
    ) -> Result<(), RuntimeError> {
        let candidates = collections::forcing_members(&self.heap, id)?;
        self.frames.last_mut().expect("active frame").equality_run = Some(EqualityRun {
            tasks: vec![Task::Resolve(id, member, candidates, 0, None)],
            on_false: Vec::new(),
            active: BTreeSet::new(),
            forced: BTreeSet::new(),
            roots: vec![Value::Object(id)],
            equal: true,
            negate: false,
            retry: false,
            prepared_hashes: None,
            key_search: false,
            hide_callsite: true,
            forward: true,
            forward_to_type: false,
        });
        Ok(())
    }
    pub(super) fn search_key(
        &mut self,
        key: &Value,
        candidates: Vec<Value>,
    ) -> Result<bool, RuntimeError> {
        let frame = self.frames.last_mut().expect("active frame");
        let mut search = frame.key_search.take().unwrap_or(KeySearch {
            key: key.clone(),
            candidates,
            position: 0,
            returned: None,
        });
        if let Some(equal) = search.returned.take()
            && equal
        {
            if let (Value::Object(a), Value::Object(b)) =
                (&search.key, &search.candidates[search.position - 1])
            {
                self.heap.key_equalities.insert((*a, *b));
            }
            return Ok(false);
        }
        while let Some(candidate) = search.candidates.get(search.position).cloned() {
            search.position += 1;
            if same_key(&self.heap, key, &candidate) {
                return Ok(false);
            }
            if matches!((key, &candidate), (Value::Object(_), Value::Object(_))) {
                self.begin_equality(key.clone(), candidate, false);
                let frame = self.frames.last_mut().expect("active frame");
                let run = frame.equality_run.as_mut().expect("key equality");
                if let Some(Task::Compare(_, _, semantic)) = run.tasks.last_mut() {
                    *semantic = false;
                }
                run.key_search = true;
                frame.key_search = Some(search);
                return Ok(true);
            }
        }
        Ok(false)
    }
    pub(super) fn prepare_keys(&mut self, keys: Vec<Value>) -> Result<bool, RuntimeError> {
        // One bytecode instruction can prepare a probe and then a larger set
        // of insertion keys. Keep all completed preparations across its retries;
        // the scheduler clears them only when that instruction completes.
        let mut prepared = self
            .frames
            .last()
            .expect("active frame")
            .prepared_hashes
            .clone()
            .unwrap_or_default();
        let mut pending = Vec::new();
        for key in keys {
            if prepared
                .iter()
                .any(|old| identical_key(&self.heap, old, &key))
            {
                continue;
            }
            if matches!(&key, Value::Object(id) if self.needs_key_hash(*id)?) {
                pending.push(key.clone());
            }
            prepared.push(key);
        }
        if pending.is_empty() {
            self.frames
                .last_mut()
                .expect("active frame")
                .prepared_hashes = Some(prepared);
            return Ok(false);
        }
        self.frames.last_mut().expect("active frame").equality_run = Some(EqualityRun {
            tasks: pending.iter().rev().cloned().map(Task::Prepare).collect(),
            on_false: Vec::new(),
            active: BTreeSet::new(),
            forced: BTreeSet::new(),
            roots: pending,
            equal: true,
            negate: false,
            retry: true,
            prepared_hashes: Some(prepared),
            key_search: false,
            hide_callsite: true,
            forward: false,
            forward_to_type: false,
        });
        Ok(true)
    }
    fn class_identity(&self, mut id: ObjectId) -> Result<Option<ObjectId>, RuntimeError> {
        loop {
            let object = self.heap.get(id)?;
            if object.class_definition.is_some() {
                return Ok(Some(id));
            }
            let Some(parent) = object.parent else {
                return Ok(None);
            };
            id = parent;
        }
    }
    fn equality_members(&self, id: ObjectId) -> Result<Vec<CollectionMember>, RuntimeError> {
        if self.heap.get(id)?.collection_kind == CollectionKind::Map {
            return self.map_hash_members(id);
        }
        let mut members = collections::forcing_members(&self.heap, id)?;
        members.retain(|descriptor| {
            let mut owner = Some(id);
            while let Some(id) = owner {
                let Ok(object) = self.heap.get(id) else {
                    return false;
                };
                if object.members.iter().any(|m| {
                    !m.entry
                        && m.name == descriptor.name
                        && self.program.metadata.get(m.code).is_some_and(|m| m.hidden)
                }) {
                    return false;
                }
                owner = object.parent;
            }
            true
        });
        Ok(members)
    }
    fn cached_equality_member(
        &self,
        id: ObjectId,
        member: &CollectionMember,
    ) -> Result<Value, RuntimeError> {
        let key = CacheKey {
            owner: None,
            name: member.name.clone(),
            method: false,
            entry: member.kind != CollectionMemberKind::Property,
        };
        self.cached_slot(id, &key)?
            .ok_or_else(|| invalid("unforced equality member"))
    }
    /// Equality visits typed properties in class declaration order; collection
    /// equality visits values in successful cache insertion order. Neither order
    /// is the child-first declaration traversal used by VmObject.force.
    fn comparison_members(&self, id: ObjectId) -> Result<Vec<CollectionMember>, RuntimeError> {
        let members = self.equality_members(id)?;
        let mut result = Vec::with_capacity(members.len());
        if let Some(class) = self
            .class_identity(id)?
            .or(self.heap.get(id)?.is_module.then_some(id))
        {
            let mut owner = Some(class);
            while let Some(id) = owner {
                let object = self.heap.get(id)?;
                for member in &object.members {
                    if !member.local
                        && !member.method
                        && !member.entry
                        && let Some(descriptor) = members
                            .iter()
                            .find(|candidate| candidate.name == member.name)
                        && !result.iter().any(|old: &CollectionMember| {
                            old.name == descriptor.name && old.kind == descriptor.kind
                        })
                    {
                        result.push(descriptor.clone());
                    }
                }
                owner = object.parent;
            }
        } else {
            for key in &self.heap.get(id)?.completed_cache_order {
                if key.owner.is_none()
                    && !key.method
                    && let Some(member) = members.iter().find(|member| {
                        member.name == key.name
                            && (member.kind != CollectionMemberKind::Property) == key.entry
                    })
                {
                    result.push(member.clone());
                }
            }
        }
        Ok(result)
    }
    fn hash_entries(&self, id: ObjectId) -> Result<Vec<(Option<Value>, Value)>, RuntimeError> {
        let object = self.heap.get(id)?;
        let typed = self.class_identity(id)?.is_some() || object.is_module;
        if object.collection_kind == CollectionKind::Set {
            return Ok(self
                .set_hash_elements(id)?
                .into_iter()
                .map(|value| (None, value))
                .collect());
        }
        if object.collection_kind == CollectionKind::Map {
            return self
                .map_hash_members(id)?
                .iter()
                .map(|member| {
                    Ok((
                        Some(member.key.clone()),
                        self.cached_equality_member(id, member)?,
                    ))
                })
                .collect();
        }
        if typed || object.collection_kind == CollectionKind::List {
            let members = if object.collection_kind == CollectionKind::List {
                collections::ordered_members(&self.heap, id)?
            } else {
                self.comparison_members(id)?
            };
            return members
                .iter()
                .map(|member| Ok((None, self.cached_equality_member(id, member)?)))
                .collect();
        }
        let members = self.equality_members(id)?;
        let mut result = Vec::new();
        for slot in &object.cache_slots {
            let key = &slot.key;
            let value = &slot.value;
            if key.owner.is_some() {
                if matches!(
                    object.collection_kind,
                    CollectionKind::Listing | CollectionKind::Mapping
                ) {
                    result.push((None, value.clone()));
                }
            } else if let Some(member) = members.iter().find(|member| {
                member.name == key.name
                    && (member.kind != CollectionMemberKind::Property) == key.entry
            }) {
                result.push((
                    (member.kind != CollectionMemberKind::Property).then(|| member.key.clone()),
                    value.clone(),
                ));
            }
        }
        Ok(result)
    }

    pub(super) fn step_equality(&mut self) -> Result<(), RuntimeError> {
        let frame_index = self.frames.len() - 1;
        let mut run = self.frames[frame_index]
            .equality_run
            .take()
            .expect("equality run");
        let Some(task) = run.tasks.pop() else {
            self.frames[frame_index].callsite_hidden = false;
            if run.forward_to_type {
                let value = self.pop()?;
                self.frames[frame_index]
                    .type_run
                    .as_mut()
                    .ok_or_else(|| invalid("missing deferred cache type continuation"))?
                    .returned = Some(value);
            } else if run.key_search {
                self.frames[frame_index]
                    .key_search
                    .as_mut()
                    .expect("key search")
                    .returned = Some(run.equal);
                self.frames[frame_index].pc -= 1;
            } else if run.retry {
                self.frames[frame_index].prepared_hashes = run.prepared_hashes;
                self.frames[frame_index].pc -= 1;
            } else if !run.forward {
                self.stack.push(Value::Bool(run.equal ^ run.negate));
            }
            return Ok(());
        };
        match task {
            Task::CountKey(id, keys, position, seen) => {
                if let Some(key) = keys.get(position).cloned() {
                    run.tasks.push(Task::CountReady(id, keys, position, seen));
                    run.tasks.push(Task::Prepare(key));
                } else {
                    self.heap.get_mut(id)?.mapping_length = Some(seen.len());
                }
            }
            Task::CountReady(id, keys, position, seen) => {
                let hash = self.java_hash(&keys[position])?;
                run.tasks
                    .push(Task::CountKeys(id, keys, position, seen, 0, hash));
            }
            Task::CountKeys(id, keys, position, mut seen, candidate, hash) => {
                let key = &keys[position];
                if let Some((other, stored_hash)) = seen.get(candidate).cloned() {
                    let same_hash = match (hash, stored_hash) {
                        (Some(left), Some(right)) => left == right,
                        _ => true,
                    };
                    if same_hash && same_key(&self.heap, key, &other) {
                        run.tasks.push(Task::CountKey(id, keys, position + 1, seen));
                    } else if same_hash
                        && matches!((key, &other), (Value::Object(_), Value::Object(_)))
                    {
                        let depth = run.tasks.len();
                        run.on_false.push((
                            depth,
                            Task::CountKeys(
                                id,
                                keys.clone(),
                                position,
                                seen.clone(),
                                candidate + 1,
                                hash,
                            ),
                        ));
                        run.tasks.push(Task::CountMatched(
                            id,
                            keys.clone(),
                            position,
                            seen,
                            other.clone(),
                        ));
                        run.tasks.push(Task::Compare(key.clone(), other, false));
                    } else {
                        run.tasks.push(Task::CountKeys(
                            id,
                            keys,
                            position,
                            seen,
                            candidate + 1,
                            hash,
                        ));
                    }
                } else {
                    seen.push((key.clone(), hash));
                    run.tasks.push(Task::CountKey(id, keys, position + 1, seen));
                }
            }
            Task::CountMatched(id, keys, position, seen, other) => {
                run.on_false
                    .pop()
                    .ok_or_else(|| invalid("missing mapping count comparison"))?;
                if let (Value::Object(query), Value::Object(stored)) = (&keys[position], other) {
                    self.heap.key_equalities.insert((*query, stored));
                }
                run.tasks.push(Task::CountKey(id, keys, position + 1, seen));
            }
            Task::Reject => run.reject(),
            Task::ResumeForce(receiver, owner, name, local, memoize, method, entry, predicate) => {
                self.frames[frame_index].equality_run = Some(run);
                return self.force_key_mode_prepared(
                    receiver, owner, name, local, memoize, method, entry, predicate, true, None,
                );
            }
            Task::PublishCache(id, key, value) => {
                // Bootstrap strict collections can carry completed scalar slots
                // before their object is installed in the heap.
                while self.heap.get(id)?.cache_index.len() < self.heap.get(id)?.cache_slots.len() {
                    let object = self.heap.get_mut(id)?;
                    object.cache_index.grow_before_append();
                    object.cache_index.push();
                }
                let hash = self.cache_key_hash(id, &key)?;
                let existing = self
                    .heap
                    .get(id)?
                    .cache_index
                    .candidates(Some(hash))
                    .into_iter()
                    .find(|index| {
                        self.heap.get(id).expect("cache object").cache_slots[*index].key == key
                    });
                if let Some(index) = existing {
                    let object = self.heap.get_mut(id)?;
                    object.cache_slots[index].value = value.clone();
                    object.cache.insert(key, LazyState::Completed(value));
                } else {
                    run.tasks.push(Task::AppendCache(id, key, value));
                    if self.heap.get_mut(id)?.cache_index.grow_before_append() {
                        self.heap.get_mut(id)?.cache_index.start_rehash();
                        run.tasks.push(Task::CacheRebuild(id, 0));
                    }
                }
            }
            Task::AppendCache(id, key, value) => {
                // Warming local slots or publishing duplicate slots can change
                // directional object equality, including that of enclosing keys.
                if key.owner.is_some()
                    || self
                        .heap
                        .get(id)?
                        .cache_slots
                        .iter()
                        .any(|slot| slot.key == key)
                {
                    self.heap.key_equalities.clear();
                }
                let object = self.heap.get_mut(id)?;
                let index = object.cache_index.push();
                object.completed_cache_order.push(key.clone());
                object.cache_slots.push(crate::heap::CacheSlot {
                    key: key.clone(),
                    value: value.clone(),
                });
                object
                    .cache
                    .insert(key.clone(), LazyState::Completed(value));
                if object.cache_index.has_hash() {
                    run.tasks.push(Task::CacheLink(id, index, true));
                    if let Some(value) = self.cache_value_key(id, &key)? {
                        run.tasks.push(Task::Prepare(value));
                    }
                } else if object.cache_index.len() > 4 {
                    object.cache_index.start_rehash();
                    run.tasks.push(Task::CacheRebuild(id, 0));
                }
            }
            Task::CacheRebuild(id, index) => {
                if let Some(slot) = self.heap.get(id)?.cache_slots.get(index).cloned() {
                    run.tasks.push(Task::CacheRebuild(id, index + 1));
                    run.tasks.push(Task::CacheLink(id, index, false));
                    if let Some(value) = self.cache_value_key(id, &slot.key)? {
                        run.roots.push(value.clone());
                        run.tasks.push(Task::Prepare(value));
                    }
                }
            }
            Task::CacheLink(id, index, allow_rehash) => {
                let key = self.heap.get(id)?.cache_slots[index].key.clone();
                let hash = self.cache_key_hash(id, &key)?;
                let object = self.heap.get_mut(id)?;
                if object.cache_index.put_hash(
                    index,
                    hash,
                    allow_rehash && object.cache_index.should_rehash(),
                ) {
                    object.cache_index.start_rehash();
                    run.tasks.push(Task::CacheRebuild(id, 0));
                }
            }
            Task::Compare(left, right, semantic) => {
                let equal = match (&left, &right) {
                    (Value::Int(a), Value::Float(b)) if semantic => Some(*a as f64 == *b),
                    (Value::Float(a), Value::Int(b)) if semantic => Some(*a == *b as f64),
                    (Value::Float(a), Value::Float(b)) if semantic => Some(a == b),
                    (Value::Object(a), Value::Object(b)) if a == b => Some(true),
                    (Value::Object(a), Value::Object(b)) => {
                        let x = self.heap.get(*a)?;
                        let y = self.heap.get(*b)?;
                        if x.reference.is_some() || y.reference.is_some() {
                            match (&x.reference, &y.reference) {
                                (Some(x), Some(y)) => {
                                    if reference::metadata_key(&x.referent)
                                        != reference::metadata_key(&y.referent)
                                        || x.path.len() != y.path.len()
                                    {
                                        run.tasks.push(Task::Reject);
                                    } else {
                                        for (a, b) in x.path.iter().zip(&y.path).rev() {
                                            run.tasks.push(Task::Compare(
                                                a.clone(),
                                                b.clone(),
                                                false,
                                            ));
                                        }
                                    }
                                    run.tasks.push(Task::Compare(
                                        x.data.clone(),
                                        y.data.clone(),
                                        false,
                                    ));
                                    run.tasks.push(Task::Compare(
                                        x.domain.clone(),
                                        y.domain.clone(),
                                        false,
                                    ));
                                    None
                                }
                                _ => Some(false),
                            }
                        } else if x.collection_kind != y.collection_kind
                            || self.class_identity(*a)? != self.class_identity(*b)?
                            || x.is_module != y.is_module
                            || (matches!(
                                x.collection_kind,
                                CollectionKind::Listing | CollectionKind::List
                            ) && x.length != y.length)
                            || (matches!(
                                x.collection_kind,
                                CollectionKind::Map | CollectionKind::Set
                            ) && x.map_index().len() != y.map_index().len())
                        {
                            Some(false)
                        } else if x.collection_kind == CollectionKind::Set {
                            // AbstractUnmodSet.equals iterates the right table and
                            // performs contains on the left table.
                            run.tasks.push(Task::SetNext(*a, *b, 0));
                            None
                        } else if x.collection_kind == CollectionKind::Map {
                            run.tasks.push(Task::MapNext(*a, *b, 0));
                            None
                        } else if x.collection_kind == CollectionKind::List {
                            let lhs = collections::ordered_members(&self.heap, *a)?;
                            let rhs = collections::ordered_members(&self.heap, *b)?;
                            for (l, r) in lhs.iter().zip(&rhs).rev() {
                                let l = self.cached_equality_member(*a, l)?;
                                let r = self.cached_equality_member(*b, r)?;
                                run.roots.extend([l.clone(), r.clone()]);
                                run.tasks.push(Task::Compare(l, r, false));
                            }
                            None
                        } else {
                            run.tasks.push(Task::CompareMembers(*a, *b));
                            run.tasks.push(Task::Deep(right));
                            run.tasks.push(Task::Deep(left));
                            None
                        }
                    }
                    _ => Some(same_key(&self.heap, &left, &right)),
                };
                if equal == Some(false) {
                    run.reject();
                }
            }
            Task::SetNext(a, b, index) => {
                if let Some(value) = self.set_hash_elements(b)?.get(index).cloned() {
                    run.roots.push(value.clone());
                    run.tasks.push(Task::SetNext(a, b, index + 1));
                    run.tasks.push(Task::SetLookup(a, value.clone()));
                    run.tasks.push(Task::Prepare(value));
                }
            }
            Task::SetLookup(id, value) => {
                let object = self.heap.get(id)?;
                let candidates = object
                    .map_index()
                    .candidates(self.java_hash(&value)?)
                    .into_iter()
                    .map(|index| (object.set_entries[index].clone(), Value::Null))
                    .collect::<Vec<_>>();
                run.roots
                    .extend(candidates.iter().map(|(value, _)| value.clone()));
                run.tasks
                    .push(Task::Match(value, Value::Null, candidates, 0));
            }
            Task::MapNext(a, b, index) => {
                if let Some(member) = self.map_hash_members(a)?.get(index).cloned() {
                    let key = member.key.clone();
                    run.tasks.push(Task::MapNext(a, b, index + 1));
                    run.tasks.push(Task::MapLookup(a, b, member));
                    run.tasks.push(Task::Prepare(key));
                }
            }
            Task::MapLookup(a, b, member) => {
                let hash = self.java_hash(&member.key)?;
                let left = self.cached_equality_member(a, &member)?;
                let object = self.heap.get(b)?;
                let mut candidates = Vec::new();
                for index in object.map_index().candidates(hash) {
                    let other = &object.collection_members[index];
                    let right = self.cached_equality_member(b, other)?;
                    run.roots.extend([other.key.clone(), right.clone()]);
                    candidates.push((other.key.clone(), right));
                }
                run.roots.extend([member.key.clone(), left.clone()]);
                run.tasks.push(Task::Match(member.key, left, candidates, 0));
            }
            Task::CompareMembers(a, b) => {
                if self.heap.get(a)?.collection_kind == CollectionKind::Mapping {
                    let mut pending = Vec::new();
                    for id in [a, b] {
                        let mut chain = Vec::new();
                        let mut owner = Some(id);
                        while let Some(id) = owner {
                            chain.push(id);
                            owner = self.heap.get(id)?.parent;
                        }
                        for id in chain.into_iter().rev() {
                            for member in &self.heap.get(id)?.collection_members {
                                if matches!(&member.key, Value::Object(key) if self.heap.get(*key)?.structural_key.is_none())
                                {
                                    pending.push(member.key.clone());
                                }
                            }
                        }
                    }
                    if !pending.is_empty() {
                        run.tasks.push(Task::CompareMembers(a, b));
                        run.tasks
                            .extend(pending.into_iter().rev().map(Task::Prepare));
                        self.frames[frame_index].equality_run = Some(run);
                        return Ok(());
                    }
                }
                let lhs = self.comparison_members(a)?;
                let rhs = self.equality_members(b)?;
                let typed = self.class_identity(a)?.is_some() || self.heap.get(a)?.is_module;
                let mut entries = Vec::new();
                if typed {
                    for member in lhs {
                        let value = self.cached_equality_member(a, &member)?;
                        entries.push((Some(member), None, value));
                    }
                } else {
                    let members = self.equality_members(a)?;
                    for slot in &self.heap.get(a)?.cache_slots {
                        if slot.key.owner.is_some() {
                            if matches!(
                                self.heap.get(a)?.collection_kind,
                                CollectionKind::Listing | CollectionKind::Mapping
                            ) {
                                entries.push((None, Some(slot.key.clone()), slot.value.clone()));
                            }
                        } else if let Some(member) = members.iter().find(|member| {
                            member.name == slot.key.name
                                && (member.kind != CollectionMemberKind::Property) == slot.key.entry
                        }) {
                            entries.push((Some(member.clone()), None, slot.value.clone()));
                        }
                    }
                }
                if self.equality_members(a)?.len() != rhs.len() {
                    run.reject();
                } else {
                    for (member, local, left) in entries.into_iter().rev() {
                        if let Some(local) = local {
                            if let Some(right) = self.cached_slot(b, &local)? {
                                run.roots.extend([left.clone(), right.clone()]);
                                run.tasks.push(Task::Compare(left, right, false));
                            } else {
                                run.tasks.push(Task::Reject);
                            }
                            continue;
                        }
                        let member = member.expect("regular comparison member");
                        let object = self.heap.get(b)?;
                        let hash = if member.kind == CollectionMemberKind::Property {
                            string_hash(&member.name)
                        } else {
                            self.java_hash(&member.key)?.unwrap_or(0)
                        };
                        let indices = if object.cache_index.len() == object.cache_slots.len() {
                            object.cache_index.candidates(Some(hash))
                        } else {
                            (0..object.cache_slots.len()).collect()
                        };
                        let mut candidates = Vec::new();
                        for index in indices {
                            let slot = &object.cache_slots[index];
                            if slot.key.owner.is_none()
                                && let Some(other) = rhs.iter().find(|other| {
                                    other.name == slot.key.name
                                        && (other.kind != CollectionMemberKind::Property)
                                            == slot.key.entry
                                })
                                && (member.kind == CollectionMemberKind::Property)
                                    == (other.kind == CollectionMemberKind::Property)
                            {
                                run.roots.extend([other.key.clone(), slot.value.clone()]);
                                candidates.push((other.key.clone(), slot.value.clone()));
                            }
                        }
                        run.roots.extend([member.key.clone(), left.clone()]);
                        run.tasks.push(Task::Match(member.key, left, candidates, 0));
                    }
                }
            }
            Task::Match(key, value, candidates, index) => {
                if let Some((other_key, other_value)) = candidates.get(index).cloned() {
                    let depth = run.tasks.len();
                    run.on_false.push((
                        depth,
                        Task::Match(key.clone(), value.clone(), candidates, index + 1),
                    ));
                    run.tasks.push(Task::Matched(
                        key.clone(),
                        other_key.clone(),
                        value,
                        other_value,
                    ));
                    run.tasks.push(Task::Compare(key, other_key, false));
                } else {
                    run.reject();
                }
            }
            Task::Matched(key, other_key, value, other_value) => {
                run.on_false
                    .pop()
                    .ok_or_else(|| invalid("missing key comparison branch"))?;
                if let (Value::Object(a), Value::Object(b)) = (key, other_key) {
                    self.heap.key_equalities.insert((a, b));
                }
                run.tasks.push(Task::Compare(value, other_value, false));
            }
            Task::Deep(value) => {
                if let Value::Object(id) = value
                    && run.forced.insert(id)
                {
                    if let Some(reference) = self.heap.get_mut(id)?.reference.as_mut() {
                        if !reference.forced {
                            reference.forced = true;
                            run.tasks
                                .extend(reference.path.iter().rev().cloned().map(Task::Deep));
                            run.tasks.push(Task::Deep(reference.data.clone()));
                            run.tasks.push(Task::Deep(reference.domain.clone()));
                        }
                    } else if self.heap.get(id)?.collection_kind == CollectionKind::Map {
                        for member in self.map_hash_members(id)?.into_iter().rev() {
                            let value = self.cached_equality_member(id, &member)?;
                            run.roots.extend([member.key.clone(), value.clone()]);
                            run.tasks.push(Task::Deep(value));
                            run.tasks.push(Task::Deep(member.key));
                        }
                    } else {
                        for member in self.equality_members(id)?.into_iter().rev() {
                            run.tasks.push(Task::Force(id, member, false));
                        }
                    }
                }
            }
            Task::Prepare(value) => {
                if let Value::Object(id) = value
                    && self.needs_key_hash(id)?
                {
                    if !run.active.insert(id) {
                        return Err(RuntimeError::new(ErrorKind::Cycle, "cyclic structural key"));
                    }
                    run.tasks.push(Task::Build(id));
                    run.tasks.push(Task::HashMembers(id));
                    if !matches!(
                        self.heap.get(id)?.collection_kind,
                        CollectionKind::List | CollectionKind::Map | CollectionKind::Set
                    ) {
                        run.tasks.push(Task::Deep(Value::Object(id)));
                    }
                }
            }
            Task::HashMembers(id) => {
                // Java completes recursive value forcing before computing member
                // hashes. Nested Mapping keys can still be lazy at that point.
                // Hashing then follows cache order (or class property order).
                if let Some(reference) = self.heap.get(id)?.reference.as_ref() {
                    run.tasks
                        .extend(reference.path.iter().rev().cloned().map(Task::Prepare));
                    run.tasks.push(Task::Prepare(reference.data.clone()));
                    run.tasks.push(Task::Prepare(reference.domain.clone()));
                    self.frames[frame_index].equality_run = Some(run);
                    return Ok(());
                }
                for (key, value) in self.hash_entries(id)?.into_iter().rev() {
                    run.roots.push(value.clone());
                    run.tasks.push(Task::Prepare(value));
                    if let Some(key) = key {
                        run.roots.push(key.clone());
                        run.tasks.push(Task::Prepare(key));
                    }
                }
            }
            Task::Force(id, member, prepare) => {
                let candidates = collections::forcing_members(&self.heap, id)?;
                run.tasks
                    .push(Task::Resolve(id, member, candidates, 0, Some(prepare)));
            }
            Task::Resolve(id, member, candidates, index, prepare) => {
                if member.kind == CollectionMemberKind::Property {
                    run.tasks.push(Task::Read(id, member, prepare));
                } else if let Some(candidate) = candidates.get(index).cloned() {
                    if candidate.kind == CollectionMemberKind::Property {
                        run.tasks
                            .push(Task::Resolve(id, member, candidates, index + 1, prepare));
                    } else if same_key(&self.heap, &member.key, &candidate.key) {
                        run.tasks.push(Task::Read(id, candidate, prepare));
                    } else {
                        let depth = run.tasks.len();
                        run.on_false.push((
                            depth,
                            Task::Resolve(id, member.clone(), candidates, index + 1, prepare),
                        ));
                        run.tasks.push(Task::Resolved(
                            id,
                            candidate.clone(),
                            member.key.clone(),
                            prepare,
                        ));
                        run.tasks
                            .push(Task::Compare(member.key, candidate.key, false));
                    }
                } else {
                    return Err(invalid("missing effective collection member"));
                }
            }
            Task::Resolved(id, member, key, prepare) => {
                run.on_false
                    .pop()
                    .ok_or_else(|| invalid("missing member resolution branch"))?;
                if let (Value::Object(a), Value::Object(b)) = (key, &member.key) {
                    self.heap.key_equalities.insert((a, *b));
                }
                run.tasks.push(Task::Read(id, member, prepare));
            }
            Task::Read(id, member, prepare) => {
                if let Some(prepare) = prepare {
                    run.tasks.push(Task::Await(prepare));
                }
                let hide_callsite = run.hide_callsite;
                self.frames[frame_index].equality_run = Some(run);
                self.frames[frame_index].callsite_hidden = hide_callsite;
                return self.force_key(
                    id,
                    id,
                    member.name,
                    false,
                    true,
                    false,
                    member.kind != CollectionMemberKind::Property,
                );
            }
            Task::Await(prepare) => {
                let value = self.pop()?;
                run.roots.push(value.clone());
                run.tasks.push(if prepare {
                    Task::Prepare(value)
                } else {
                    Task::Deep(value)
                });
            }
            Task::Build(id) => {
                let object = self.heap.get(id)?;
                if let Some(reference) = &object.reference {
                    let mut members = vec![
                        (false, Atom::Int(0), atom(&self.heap, &reference.domain)?),
                        (false, Atom::Int(1), atom(&self.heap, &reference.data)?),
                        (
                            false,
                            Atom::Int(2),
                            Atom::ReferenceType(reference::metadata_key(&reference.referent)),
                        ),
                    ];
                    for (index, value) in reference.path.iter().enumerate() {
                        members.push((
                            false,
                            Atom::Int(index as i64 + 3),
                            atom(&self.heap, value)?,
                        ));
                    }
                    let key = StructuralKey {
                        kind: 4,
                        class: None,
                        members,
                    };
                    let next = self.heap.next_structural_key;
                    let value = *self.heap.structural_keys.entry(key).or_insert_with(|| {
                        self.heap.next_structural_key += 1;
                        next
                    });
                    let object = self.heap.get_mut(id)?;
                    object.structural_key = Some(value);
                    // PType hashes include JVM class/type identity; no portable Java numeric hash.
                    object.java_hash = Some(None);
                    run.active.remove(&id);
                    self.frames[frame_index].equality_run = Some(run);
                    return Ok(());
                }
                let kind = match object.collection_kind {
                    CollectionKind::Dynamic => 0,
                    CollectionKind::Listing => 1,
                    CollectionKind::Mapping => 2,
                    CollectionKind::List => 3,
                    CollectionKind::Map => 5,
                    CollectionKind::Set => 6,
                };
                let mut members = Vec::new();
                if self.heap.get(id)?.collection_kind == CollectionKind::Set {
                    for value in self.set_hash_elements(id)? {
                        members.push((false, Atom::Null, atom(&self.heap, &value)?));
                    }
                } else {
                    for member in self.equality_members(id)? {
                        let value = self.cached_equality_member(id, &member)?;
                        members.push((
                            member.kind == CollectionMemberKind::Property,
                            atom(&self.heap, &member.key)?,
                            atom(&self.heap, &value)?,
                        ));
                    }
                }
                members.sort();
                let key = StructuralKey {
                    kind,
                    class: self.class_identity(id)?,
                    members,
                };
                let typed = self.class_identity(id)?.is_some() || self.heap.get(id)?.is_module;
                let sequence = typed
                    || matches!(
                        self.heap.get(id)?.collection_kind,
                        CollectionKind::Listing | CollectionKind::List
                    );
                let mut hash = Some(
                    if self.heap.get(id)?.collection_kind == CollectionKind::List {
                        1i32
                    } else {
                        0i32
                    },
                );
                for (key, value) in self.hash_entries(id)? {
                    let value_hash = self.java_hash(&value)?;
                    let key_hash = key
                        .as_ref()
                        .map(|key| self.java_hash(key))
                        .transpose()?
                        .flatten();
                    hash = match (hash, value_hash) {
                        (Some(total), Some(value)) if sequence => {
                            Some(total.wrapping_mul(31).wrapping_add(value))
                        }
                        (Some(total), Some(value))
                            if self.heap.get(id)?.collection_kind == CollectionKind::Set =>
                        {
                            Some(total.wrapping_add(value))
                        }
                        (Some(total), Some(value)) => {
                            key_hash.map(|key| total.wrapping_add(key ^ value))
                        }
                        _ => None,
                    };
                }
                self.heap.get_mut(id)?.java_hash = Some(hash);
                let next = self.heap.next_structural_key;
                let value = *self.heap.structural_keys.entry(key).or_insert_with(|| {
                    self.heap.next_structural_key += 1;
                    next
                });
                self.heap.get_mut(id)?.structural_key = Some(value);
                run.active.remove(&id);
            }
        }
        self.frames[frame_index].equality_run = Some(run);
        Ok(())
    }
}
