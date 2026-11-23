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
    Duration(u64, Option<crate::value_identity::Token>),
    DataSize(u64, Option<crate::value_identity::Token>),
    String(PklString),
    Bytes(Arc<[u8]>),
    IntSeq(Option<[i64; 3]>),
    Regex(PklString),
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
        Value::Int(v) => Atom::Int(**v),
        Value::Float(v) => Atom::Float(if v.is_nan() {
            f64::NAN.to_bits()
        } else {
            v.to_bits()
        }),
        Value::DataSize(v) => {
            let bytes = v.value_in(crate::data_size::DataSizeUnit::Bytes);
            Atom::DataSize(
                if bytes == 0.0 {
                    0
                } else if bytes.is_nan() {
                    f64::NAN.to_bits()
                } else {
                    bytes.to_bits()
                },
                bytes.is_nan().then_some(v.reference()),
            )
        }
        Value::Duration(v) => {
            let nanos = v.nanos();
            // VmDuration.equals compares doubles numerically after an identity
            // shortcut: signed zeros compare equal, distinct NaNs do not.
            Atom::Duration(
                if nanos == 0.0 {
                    0
                } else if nanos.is_nan() {
                    f64::NAN.to_bits()
                } else {
                    nanos.to_bits()
                },
                nanos.is_nan().then_some(v.reference()),
            )
        }
        Value::String(v) => Atom::String(v.clone()),
        Value::Bytes(v) => Atom::Bytes(v.clone().into_data()),
        Value::IntSeq(v) => Atom::IntSeq(v.key()),
        Value::Regex(v) => Atom::Regex(v.pattern().clone()),
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
#[inline]
fn string_hash(value: &PklString) -> i32 {
    if let Some(s) = value.as_str()
        && s.is_ascii()
    {
        return s.bytes().fold(0i32, |hash, byte| {
            hash.wrapping_mul(31).wrapping_add(i32::from(byte))
        });
    }
    value.encode_utf16().fold(0i32, |hash, unit| {
        hash.wrapping_mul(31).wrapping_add(i32::from(unit))
    })
}

pub(crate) const fn int_java_hash(value: i64) -> i32 {
    ((value as u64) ^ ((value as u64) >> 32)) as i32
}

pub(crate) fn java_hash_on_heap(heap: &Heap, value: &Value) -> Result<Option<i32>, RuntimeError> {
    Ok(match value {
        Value::Null | Value::NullWithDefault(_) => Some(0),
        Value::Bool(value) => Some(if *value { 1231 } else { 1237 }),
        Value::Int(value) => Some(int_java_hash(value.get())),
        Value::Float(value) => {
            let bits = if value.is_nan() {
                0x7ff8000000000000
            } else {
                value.to_bits()
            };
            Some((bits ^ (bits >> 32)) as i32)
        }
        Value::Duration(value) => Some(value.java_hash()),
        Value::IntSeq(value) => Some(value.java_hash()),
        Value::Regex(value) => Some(value.java_hash()),
        Value::DataSize(value) => Some(value.java_hash()),
        Value::String(value) => Some(string_hash(value)),
        Value::Bytes(value) => Some(value.iter().fold(1i32, |hash, byte| {
            hash.wrapping_mul(31).wrapping_add(i32::from(*byte as i8))
        })),
        Value::Function(_) => None,
        Value::TypeAlias(id) => heap.get(*id)?.type_alias.as_ref().map(|name| {
            if name.contains("#") {
                string_hash(name)
            } else {
                string_hash(&PklString::from("pkl.base#").concat(name))
            }
        }),
        Value::Class(id) => Some(string_hash(&heap.class_qualified_name(*id)?)),
        Value::Object(id) => heap.get(*id)?.java_hash.flatten(),
    })
}

impl Vm {
    fn needs_key_hash(&self, id: ObjectId) -> Result<bool, RuntimeError> {
        let object = self.heap.get(id)?;
        Ok(matches!(
            object.collection_kind,
            CollectionKind::List | CollectionKind::Map | CollectionKind::Set
        ) || object.foreign_map_entry.is_some()
            || matches!(object.java_hash, None | Some(Some(0))))
    }

    pub(super) fn java_hash(&self, value: &Value) -> Result<Option<i32>, RuntimeError> {
        java_hash_on_heap(&self.heap, value)
    }
}

pub(super) fn identical_key(heap: &Heap, left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Object(a), Value::Object(b)) => a == b,
        _ => same_key(heap, left, right),
    }
}

#[inline]
pub(crate) fn same_key(heap: &Heap, left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::String(a), Value::String(b)) => a == b,
        (Value::Int(a), Value::Int(b)) => a.get() == b.get(),
        (Value::Bool(a), Value::Bool(b)) => a == b,
        (Value::Null | Value::NullWithDefault(_), Value::Null | Value::NullWithDefault(_)) => true,
        (Value::Float(a), Value::Float(b)) => {
            let ab = if a.is_nan() {
                f64::NAN.to_bits()
            } else {
                a.to_bits()
            };
            let bb = if b.is_nan() {
                f64::NAN.to_bits()
            } else {
                b.to_bits()
            };
            ab == bb
        }
        (Value::Object(a), Value::Object(b)) => a == b || heap.key_equalities.contains(&(*a, *b)),
        (
            Value::String(_)
            | Value::Int(_)
            | Value::Bool(_)
            | Value::Null
            | Value::NullWithDefault(_)
            | Value::Float(_)
            | Value::Object(_),
            Value::String(_)
            | Value::Int(_)
            | Value::Bool(_)
            | Value::Null
            | Value::NullWithDefault(_)
            | Value::Float(_)
            | Value::Object(_),
        ) => false,
        _ => matches!((atom(heap, left), atom(heap, right)), (Ok(a), Ok(b)) if a == b),
    }
}
/// Parent-first member iteration for a failed String-key lookup. The visited
/// set observes key hashing and boxed equality before String filtering.
#[derive(Clone)]
struct KeySuggestions {
    receiver: ObjectId,
    query: Value,
    members: Arc<[collections::RenderMember]>,
    position: usize,
    seen: Vec<(CollectionMember, Option<i32>)>,
    candidates: Vec<PklString>,
    map: bool,
    hash: Option<i32>,
}
#[derive(Clone, Copy)]
enum SuggestionLookup {
    Visited,
    Cached,
    Map,
}

/// Two separate guest hash/equality operations implement Paguro Set.without:
/// contains(first), then map.without(first). Neither operation can assume that
/// the first visible value still has the hash or equality it had at insertion.
#[derive(Clone)]
struct SetRestLookup {
    source: ObjectId,
    key: Value,
    removing: bool,
    hash: Option<i32>,
    candidates: Vec<usize>,
    repeat_equality: bool,
}

#[derive(Clone)]
struct CacheRead {
    receiver: ObjectId,
    owner: ObjectId,
    name: PklString,
    local: bool,
    memoize: bool,
    method: bool,
    entry: bool,
    predicate: bool,
    probe: Option<Value>,
}

#[derive(Clone)]
enum Task {
    MapRemoveHash(ObjectId, Value, bool),
    MapRemoveLookup(ObjectId, Value, bool),
    MapRemoveCompare(Box<SetRestLookup>, usize),
    MapRemoveMatched(Box<SetRestLookup>, usize),
    MapRemoveConfirmed(Box<SetRestLookup>),
    MapRemoveAbsent(ObjectId),
    MapRemoveOrder(ObjectId, Value, Option<(usize, Option<i32>)>, usize),
    MapRemoveFinish(ObjectId, Option<(usize, Option<i32>)>, usize),
    SetRestHash(ObjectId, Value, bool),
    SetRestLookup(ObjectId, Value, bool),
    SetRestCompare(Box<SetRestLookup>, usize),
    SetRestMatched(Box<SetRestLookup>, usize),
    SetRestConfirmed(Box<SetRestLookup>),
    SetRestFinish(ObjectId, Option<(usize, Option<i32>)>),
    SetRestAppend(ObjectId, Arc<[Value]>, usize),
    SuggestionVisit(Box<KeySuggestions>),
    SuggestionHashed(Box<KeySuggestions>),
    SuggestionCache(Box<KeySuggestions>),
    SuggestionLookup(Box<KeySuggestions>, SuggestionLookup, Vec<Value>, usize),
    SuggestionMatched(Box<KeySuggestions>, SuggestionLookup, Value),
    SuggestionCollect(Box<KeySuggestions>),
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
    ForceCached(ObjectId, ObjectId, Member, Option<Vec<usize>>, usize),
    ForceCachedMatch(Value, Value, Value),
    PublishCache(ObjectId, CacheKey, Value, Option<Value>),
    PublishProbe(ObjectId, CacheKey, Value, Value, Vec<usize>, usize),
    PublishMatch(ObjectId, CacheKey, Value, usize),
    AppendCache(ObjectId, CacheKey, Value, Option<Value>),
    CacheRebuild(ObjectId, usize),
    CacheLink(ObjectId, usize, bool),
    ResumeForce(Box<CacheRead>),
    ReadCache(Box<CacheRead>, Vec<usize>, usize),
    ReadCacheMatch(Value),
    Reject,
    Compare(Value, Value, bool),
    CompareMembers(ObjectId, ObjectId),
    CompareTypedMember(ObjectId, ObjectId, CollectionMember),
    MapNext(ObjectId, ObjectId, usize),
    SetNext(ObjectId, ObjectId, usize),
    SetLookup(ObjectId, Value),
    MapLookup(ObjectId, ObjectId, CollectionMember),
    Deep(Value),
    DiscardForcedValue,
    Match(Value, Value, Vec<(Value, Value)>, usize),
    Matched(Value, Value, Value, Value),
    Prepare(Value),
    HashMembers(ObjectId),
    Build(ObjectId),
    Resolve(
        ObjectId,
        CollectionMember,
        Vec<CollectionMember>,
        usize,
        Option<bool>,
    ),
    Resolved(ObjectId, CollectionMember, Value, Option<bool>),
    Read(ObjectId, CollectionMember, Option<bool>, Option<Value>),
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
        let run = frame.equality_run.get_or_insert_with(|| Box::new(EqualityRun {
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
        }));
        run.tasks.extend(tasks);
        run.roots.extend(roots);
    }

    pub(super) fn cache_value_key(
        &self,
        receiver: ObjectId,
        key: &CacheKey,
    ) -> Result<Option<Value>, RuntimeError> {
        collections::cache_entry_key(&self.heap, receiver, key)
    }

    fn cache_slot_value_key(
        &self,
        receiver: ObjectId,
        slot: &crate::heap::CacheSlot,
    ) -> Result<Option<Value>, RuntimeError> {
        collections::cache_slot_entry_key(&self.heap, receiver, slot)
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
        if !key.entry {
            let object = self.heap.get(receiver)?;
            return Ok(match object.cache.get(key) {
                Some(LazyState::Completed(value)) => Some(value.clone()),
                _ => None,
            });
        }
        if !key.method
            && key.owner.is_none()
            && let Some(elems) = &self.heap.get(receiver)?.list_elements
            && let Some(name_str) = key.name.as_str()
            && let Some(rest) = name_str.strip_prefix('\0')
            && let Some((_, idx_str)) = rest.split_once(':')
            && let Ok(idx) = idx_str.parse::<usize>()
            && let Some(val) = elems.get(idx)
        {
            return Ok(Some(val.clone()));
        }
        let probe = self.cache_value_key(receiver, key)?;
        self.cached_slot_probe(receiver, key, probe.as_ref())
    }

    pub(super) fn cached_slot_probe(
        &self,
        receiver: ObjectId,
        key: &CacheKey,
        probe: Option<&Value>,
    ) -> Result<Option<Value>, RuntimeError> {
        let object = self.heap.get(receiver)?;
        if key.entry
            && !key.method
            && key.owner.is_none()
            && let Some(elems) = &object.list_elements
            && let Some(Value::Int(index)) = probe
            && index.get() >= 0
            && let Some(val) = elems.get(index.get() as usize)
        {
            return Ok(Some(val.clone()));
        }
        let hash = if let Some(probe) = probe {
            self.java_hash(probe)?.unwrap_or(0)
        } else {
            self.cache_key_hash(receiver, key)?
        };
        let indices = if object.cache_index.len() == object.cache_slots.len() {
            object.cache_index.candidates(Some(hash))
        } else {
            (0..object.cache_slots.len()).collect()
        };
        for index in indices {
            let slot = &object.cache_slots[index];
            let matched = if let Some(probe) = probe {
                slot.key.entry
                    && slot.key.owner.is_none()
                    && !slot.key.method
                    && collections::cache_slot_entry_key(&self.heap, receiver, slot)?
                        .is_some_and(|stored| same_key(&self.heap, probe, &stored))
            } else {
                slot.key == *key
            };
            if matched {
                return Ok(Some(slot.value.clone()));
            }
        }
        Ok(None)
    }

    pub(super) fn complete_cache(
        &mut self,
        receiver: ObjectId,
        key: CacheKey,
        value: Value,
    ) -> Result<(), RuntimeError> {
        let probe = self.cache_value_key(receiver, &key)?;
        self.complete_cache_probe(receiver, key, value, probe)
    }

    fn all_cache_slots_non_object(&self, id: ObjectId) -> Result<bool, RuntimeError> {
        let object = self.heap.get(id)?;
        for slot in &object.cache_slots {
            if let Some(entry_key) = &slot.entry_key {
                if matches!(entry_key, Value::Object(_)) {
                    return Ok(false);
                }
            } else if slot.key.entry
                && slot.key.owner.is_none()
                && matches!(
                    collections::cache_entry_key(&self.heap, id, &slot.key)?,
                    Some(Value::Object(_))
                )
            {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn rebuild_cache_index_sync(&mut self, id: ObjectId) -> Result<(), RuntimeError> {
        let len = self.heap.get(id)?.cache_slots.len();
        let mut hashes = Vec::with_capacity(len);
        {
            let object = self.heap.get(id)?;
            for slot in &object.cache_slots {
                let hash = if let Some(entry_key) = &slot.entry_key {
                    java_hash_on_heap(&self.heap, entry_key)?.unwrap_or(0)
                } else if slot.key.entry && slot.key.owner.is_none() {
                    if let Some(probe) = collections::cache_entry_key(&self.heap, id, &slot.key)? {
                        java_hash_on_heap(&self.heap, &probe)?.unwrap_or(0)
                    } else {
                        string_hash(&slot.key.name)
                    }
                } else {
                    string_hash(&slot.key.name)
                        ^ slot.key.owner.map_or(0, |owner| owner.index as i32)
                };
                hashes.push(hash);
            }
        }
        let object = self.heap.get_mut(id)?;
        let range = object.cache_index.start_rehash();
        for index in range {
            let _ = object.cache_index.put_hash(index, hashes[index], false);
        }
        Ok(())
    }

    pub(super) fn lookup_non_object_cache_entry(
        &self,
        receiver: ObjectId,
        probe: &Value,
    ) -> Result<Option<Value>, RuntimeError> {
        let object = self.heap.get(receiver)?;
        if object.parent.is_none() {
            if object.collection_kind == CollectionKind::Map
                && let Some(map_index) = object.map_index.as_deref()
            {
                let hash = java_hash_on_heap(&self.heap, probe)?;
                for slot in map_index.candidates(hash) {
                    if let Some(member) = object.collection_members.get(slot)
                        && identical_key(&self.heap, probe, &member.key)
                    {
                        let key = CacheKey {
                            owner: None,
                            name: member.name.clone(),
                            method: false,
                            entry: true,
                        };
                        if let Some(LazyState::Completed(value)) = object.cache.get(&key) {
                            return Ok(Some(value.clone()));
                        }
                    }
                }
                return Ok(None);
            }
            if object.collection_kind == CollectionKind::List
                && let Value::Int(idx) = probe
                && idx.get() >= 0
            {
                let pos = idx.get() as usize;
                if let Some(elems) = &object.list_elements {
                    return Ok(elems.get(pos).cloned());
                }
                if let Some(slot) = object.cache_slots.get(pos)
                    && slot.key.entry
                    && slot.key.owner.is_none()
                    && !slot.key.method
                    && slot
                        .entry_key
                        .as_ref()
                        .is_some_and(|stored| identical_key(&self.heap, probe, stored))
                {
                    return Ok(Some(slot.value.clone()));
                }
            }
        }
        let check_slot = |index: usize| -> Result<Option<Value>, RuntimeError> {
            let Some(slot) = object.cache_slots.get(index) else {
                return Ok(None);
            };
            if !slot.key.entry || slot.key.owner.is_some() || slot.key.method {
                return Ok(None);
            }
            let matched = if let Some(stored) = &slot.entry_key {
                identical_key(&self.heap, probe, stored)
            } else if let Some(stored) =
                collections::cache_entry_key(&self.heap, receiver, &slot.key)?
            {
                identical_key(&self.heap, probe, &stored)
            } else {
                false
            };
            Ok(matched.then(|| slot.value.clone()))
        };
        if object.cache_index.has_hash() {
            let hash = java_hash_on_heap(&self.heap, probe)?.unwrap_or(0);
            for index in object.cache_index.hash_candidates(hash) {
                if let Some(value) = check_slot(index)? {
                    return Ok(Some(value));
                }
            }
        } else {
            for index in 0..object.cache_slots.len() {
                if let Some(value) = check_slot(index)? {
                    return Ok(Some(value));
                }
            }
        }
        Ok(None)
    }

    pub(super) fn complete_cache_probe(
        &mut self,
        receiver: ObjectId,
        key: CacheKey,
        value: Value,
        probe: Option<Value>,
    ) -> Result<(), RuntimeError> {
        let sync_probe = match &probe {
            Some(Value::Object(_)) => false,
            Some(_) => true,
            None => !key.entry || key.owner.is_some(),
        };
        if sync_probe {
            {
                let object = self.heap.get_mut(receiver)?;
                while object.cache_index.len() < object.cache_slots.len() {
                    object.cache_index.grow_before_append();
                    object.cache_index.push();
                }
            }
            let hash = if let Some(ref probe_val) = probe {
                java_hash_on_heap(&self.heap, probe_val)?.unwrap_or(0)
            } else {
                string_hash(&key.name) ^ key.owner.map_or(0, |owner| owner.index as i32)
            };
            let existing = {
                let object = self.heap.get(receiver)?;
                let check = |index: usize| -> Result<bool, RuntimeError> {
                    let slot = &object.cache_slots[index];
                    if let Some(ref probe_val) = probe {
                        if slot.key.entry && slot.key.owner.is_none() && !slot.key.method {
                            if let Some(stored) = &slot.entry_key {
                                return Ok(identical_key(&self.heap, probe_val, stored));
                            }
                            if let Some(stored) =
                                collections::cache_entry_key(&self.heap, receiver, &slot.key)?
                            {
                                return Ok(identical_key(&self.heap, probe_val, &stored));
                            }
                        }
                        Ok(false)
                    } else {
                        Ok(slot.key == key)
                    }
                };
                let mut found = None;
                if object.cache_index.has_hash() {
                    for index in object.cache_index.hash_candidates(hash) {
                        if check(index)? {
                            found = Some(index);
                            break;
                        }
                    }
                } else {
                    for index in 0..object.cache_slots.len() {
                        if check(index)? {
                            found = Some(index);
                            break;
                        }
                    }
                }
                found
            };
            if let Some(index) = existing {
                let object = self.heap.get_mut(receiver)?;
                object.cache_slots[index].value = value.clone();
                object.cache.insert(key, LazyState::Completed(value));
                return Ok(());
            }
            if self
                .heap
                .get_mut(receiver)?
                .cache_index
                .grow_before_append()
            {
                if self.all_cache_slots_non_object(receiver)? {
                    self.rebuild_cache_index_sync(receiver)?;
                } else {
                    self.heap.get_mut(receiver)?.cache_index.start_rehash();
                    let mut roots = vec![Value::Object(receiver), value.clone()];
                    roots.extend(probe.clone());
                    self.schedule_tasks(
                        vec![
                            Task::AppendCache(receiver, key, value, probe),
                            Task::CacheRebuild(receiver, 0),
                        ],
                        roots,
                    );
                    return Ok(());
                }
            }
            if key.owner.is_some()
                || matches!(
                    self.heap.get(receiver)?.cache.get(&key),
                    Some(LazyState::Completed(_))
                )
            {
                self.heap.key_equalities.clear();
            }
            let need_rehash = {
                let object = self.heap.get_mut(receiver)?;
                let index = object.cache_index.push();
                object.completed_cache_order.push(key.clone());
                object.cache_slots.push(crate::heap::CacheSlot {
                    key: key.clone(),
                    entry_key: probe,
                    value: value.clone(),
                });
                object.cache.insert(key, LazyState::Completed(value));
                if object.cache_index.has_hash() {
                    let should = object.cache_index.should_rehash();
                    object.cache_index.put_hash(index, hash, should)
                } else {
                    object.cache_index.len() > 4
                }
            };
            if need_rehash {
                if self.all_cache_slots_non_object(receiver)? {
                    self.rebuild_cache_index_sync(receiver)?;
                } else {
                    self.heap.get_mut(receiver)?.cache_index.start_rehash();
                    self.schedule_tasks(
                        vec![Task::CacheRebuild(receiver, 0)],
                        vec![Value::Object(receiver)],
                    );
                }
            }
            return Ok(());
        }
        let mut tasks = vec![Task::PublishCache(
            receiver,
            key,
            value.clone(),
            probe.clone(),
        )];
        if self.heap.get(receiver)?.cache_index.has_hash()
            && let Some(probe) = &probe
        {
            tasks.push(Task::Prepare(probe.clone()));
        }
        let mut roots = vec![Value::Object(receiver), value];
        roots.extend(probe);
        self.schedule_tasks(tasks, roots);
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn prepare_cache_read(
        &mut self,
        receiver: ObjectId,
        owner: ObjectId,
        name: PklString,
        local: bool,
        memoize: bool,
        method: bool,
        entry: bool,
        predicate: bool,
        probe: Option<Value>,
    ) -> Result<bool, RuntimeError> {
        if !memoize {
            return Ok(false);
        }
        let object = self.heap.get(receiver)?;
        if object.cache_slots.is_empty() {
            return Ok(false);
        }
        if let Some(ref probe_val) = probe
            && !matches!(probe_val, Value::Object(_))
        {
            if let Some(value) = self.lookup_non_object_cache_entry(receiver, probe_val)? {
                self.stack.push(value);
                return Ok(true);
            }
            return Ok(false);
        }
        let has_hash = object.cache_index.has_hash();
        let request = Box::new(CacheRead {
            receiver,
            owner,
            name,
            local,
            memoize,
            method,
            entry,
            predicate,
            probe: probe.clone(),
        });
        if let Some(probe) = probe {
            let mut tasks = vec![Task::ResumeForce(request)];
            if has_hash {
                tasks.push(Task::Prepare(probe.clone()));
            }
            self.schedule_tasks(
                tasks,
                vec![Value::Object(receiver), Value::Object(owner), probe],
            );
            return Ok(true);
        }
        Ok(false)
    }

    /// VmObject.force queries the receiver cache before reading the current
    /// owner's declaration. Small EconomicMaps compare keys without hashing.
    pub(super) fn force_declaration_member(
        &mut self,
        receiver: ObjectId,
        owner: ObjectId,
        member: Member,
    ) -> Result<(), RuntimeError> {
        if !member.entry || self.heap.get(receiver)?.cache_slots.is_empty() {
            return self.force_key(
                receiver,
                owner,
                member.name,
                false,
                true,
                false,
                member.entry,
            );
        }
        let key = CacheKey {
            entry: true,
            owner: None,
            method: false,
            name: member.name.clone(),
        };
        let query = self
            .cache_value_key(receiver, &key)?
            .ok_or_else(|| invalid("force entry key"))?;
        if !matches!(query, Value::Object(_)) {
            if let Some(value) = self.lookup_non_object_cache_entry(receiver, &query)? {
                self.stack.push(value);
                return Ok(());
            }
            return self.force_key_mode_prepared(
                receiver,
                owner,
                member.name.clone(),
                false,
                true,
                false,
                true,
                false,
                true,
                Some(member),
            );
        }
        let mut tasks = vec![Task::ForceCached(receiver, owner, member, None, 0)];
        if self.heap.get(receiver)?.cache_index.has_hash() {
            tasks.push(Task::Prepare(query.clone()));
        }
        self.schedule_tasks(
            tasks,
            vec![Value::Object(receiver), Value::Object(owner), query],
        );
        self.frames
            .last_mut()
            .expect("force frame")
            .equality_run
            .as_mut()
            .expect("cache lookup")
            .hide_callsite = false;
        Ok(())
    }

    /// A missing non-String key has no suggestion traversal. String misses use
    /// the same scheduler as key hashing, so errors, reads and logs can suspend.
    pub(super) fn fail_missing_key(
        &mut self,
        receiver: ObjectId,
        query: Value,
    ) -> Result<(), RuntimeError> {
        if !matches!(&query, Value::String(_)) {
            return Err(self.key_suggestions_error(&query, Vec::new()));
        }
        let map = self.heap.get(receiver)?.collection_kind == CollectionKind::Map;
        let members = if map {
            collections::ordered_members(&self.heap, receiver)?
                .into_iter()
                .map(|descriptor| collections::RenderMember {
                    descriptor,
                    skip: false,
                    code: None,
                })
                .collect::<Vec<_>>()
        } else {
            // Do not deduplicate with ordered_members: Java hashes each raw
            // declaration before its visited-set and visibility checks.
            collections::render_members(&self.heap, receiver)?
        };
        let mut roots = vec![Value::Object(receiver), query.clone()];
        roots.extend(members.iter().map(|item| item.descriptor.key.clone()));
        self.schedule_tasks(
            vec![Task::SuggestionVisit(Box::new(KeySuggestions {
                receiver,
                query,
                members: members.into(),
                position: 0,
                seen: Vec::new(),
                candidates: Vec::new(),
                map,
                hash: None,
            }))],
            roots,
        );
        // VmExceptionBuilder's key hashing is outside the failing subscript's
        // guest frame. The final lookup diagnostic restores that frame below.
        self.frames
            .last_mut()
            .expect("lookup frame")
            .equality_run
            .as_mut()
            .expect("suggestion traversal")
            .hide_callsite = true;
        Ok(())
    }

    /// Renderer continuation receives no stack result and does not retry its
    /// external Render instruction after the length cache is published.
    pub(super) fn count_render_mapping_keys(&mut self, id: ObjectId, keys: Vec<Value>) {
        let mut roots = keys.clone();
        roots.push(Value::Object(id));
        self.schedule_tasks(vec![Task::CountKey(id, keys, 0, Vec::new())], roots);
        self.frames
            .last_mut()
            .expect("renderer frame")
            .equality_run
            .as_mut()
            .expect("mapping count")
            .hide_callsite = false;
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

    pub(super) fn begin_map_remove(&mut self, source: ObjectId, key: Value) {
        self.schedule_tasks(
            vec![Task::MapRemoveHash(source, key.clone(), false)],
            vec![Value::Object(source), key],
        );
    }

    pub(super) fn begin_set_rest(&mut self, source: ObjectId, first: Value) {
        self.schedule_tasks(
            vec![Task::SetRestHash(source, first.clone(), false)],
            vec![Value::Object(source), first],
        );
    }

    /// Java boxed key equality without bytecode-PC retries.
    pub(super) fn begin_boxed_equality(&mut self, left: Value, right: Value) {
        self.begin_equality(left, right, false);
        if let Some(Task::Compare(_, _, semantic)) = self
            .frames
            .last_mut()
            .unwrap()
            .equality_run
            .as_mut()
            .unwrap()
            .tasks
            .last_mut()
        {
            *semantic = false;
        }
    }

    pub(super) fn begin_equality(&mut self, left: Value, right: Value, negate: bool) {
        self.frames.last_mut().expect("active frame").equality_run = Some(Box::new(EqualityRun {
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
        }));
    }
    pub(super) fn force_collection_member(
        &mut self,
        id: ObjectId,
        member: CollectionMember,
    ) -> Result<(), RuntimeError> {
        if let Some(elems) = &self.heap.get(id)?.list_elements
            && member.kind == CollectionMemberKind::Element
            && let Value::Int(idx) = &member.key
            && idx.get() >= 0
        {
            let value = elems
                .get(idx.get() as usize)
                .cloned()
                .ok_or_else(|| invalid("missing effective collection member"))?;
            self.stack.push(value);
            return Ok(());
        }
        let candidates = collections::forcing_members(&self.heap, id)?;
        let probe = member.key.clone();
        self.frames.last_mut().expect("active frame").equality_run = Some(Box::new(EqualityRun {
            tasks: vec![Task::Resolve(id, member, candidates, 0, None)],
            on_false: Vec::new(),
            active: BTreeSet::new(),
            forced: BTreeSet::new(),
            roots: vec![Value::Object(id), probe],
            equal: true,
            negate: false,
            retry: false,
            prepared_hashes: None,
            key_search: false,
            hide_callsite: true,
            forward: true,
            forward_to_type: false,
        }));
        Ok(())
    }
    pub(super) fn search_key(
        &mut self,
        key: &Value,
        candidates: Vec<Value>,
    ) -> Result<bool, RuntimeError> {
        let frame = self.frames.last_mut().expect("active frame");
        if frame.key_search.is_none() && !matches!(key, Value::Object(_)) {
            return Ok(false);
        }
        let mut search = frame.key_search.take().unwrap_or_else(|| {
            Box::new(KeySearch {
                key: key.clone(),
                candidates,
                position: 0,
                returned: None,
            })
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
    /// VmObject.iterateMemberValues hashes each visited key before rendering it.
    pub(super) fn prepare_render_key(&mut self, key: Value) {
        self.schedule_tasks(vec![Task::Prepare(key.clone())], vec![key]);
        self.frames
            .last_mut()
            .expect("render frame")
            .equality_run
            .as_mut()
            .expect("key preparation")
            .hide_callsite = false;
    }
    pub(super) fn prepare_keys(&mut self, keys: Vec<Value>) -> Result<bool, RuntimeError> {
        if !keys.iter().any(|key| matches!(key, Value::Object(_))) {
            return Ok(false);
        }
        // One bytecode instruction can prepare a probe and then a larger set
        // of insertion keys. Keep all completed preparations across its retries;
        // the scheduler clears them only when that instruction completes.
        // Only `Value::Object` keys can ever require lazy key-hash preparation.
        let mut prepared = self
            .frames
            .last()
            .expect("active frame")
            .prepared_hashes
            .clone()
            .unwrap_or_default();
        let mut pending = Vec::new();
        for key in keys {
            let Value::Object(id) = key else {
                continue;
            };
            if prepared
                .iter()
                .any(|old| matches!(old, Value::Object(old_id) if *old_id == id))
            {
                continue;
            }
            if self.needs_key_hash(id)? {
                pending.push(Value::Object(id));
            }
            prepared.push(Value::Object(id));
        }
        if pending.is_empty() {
            self.frames
                .last_mut()
                .expect("active frame")
                .prepared_hashes = Some(prepared);
            return Ok(false);
        }
        self.frames.last_mut().expect("active frame").equality_run = Some(Box::new(EqualityRun {
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
        }));
        Ok(true)
    }
    fn class_identity(&self, id: ObjectId) -> Result<Option<ObjectId>, RuntimeError> {
        let Some(class) = self.heap.object_class(id)? else {
            return Ok(None);
        };
        let prototype = self.heap.get(self.heap.get_class(class)?.prototype)?;
        // Builtin collection prototypes supply inherited members and getClass,
        // but collection equality/hash use the object's own cached members.
        // Only Typed/module instances use class-declared property ordering.
        Ok((!matches!(prototype.builtin_type, Some(TypeCheck::Collection(_)))).then_some(class))
    }
    pub(super) fn equality_members(
        &self,
        id: ObjectId,
    ) -> Result<Vec<CollectionMember>, RuntimeError> {
        if self.heap.get(id)?.collection_kind == CollectionKind::Map {
            return self.map_hash_members(id);
        }
        let mut members = collections::forcing_members(&self.heap, id)?;
        if let Some(prototype) = self
            .class_identity(id)?
            .map(|class| self.heap.get_class(class).map(|class| class.prototype))
            .transpose()?
            .or(self.heap.get(id)?.is_module.then_some(id))
        {
            // VmClass.getAllRegularPropertyNames excludes external declarations
            // as well as class/typealias prototype definitions.
            // Their native getters need not have a cached value after force().
            // Union each class's own regular names with its inherited names, so
            // a regular superclass property survives an external override too.
            let mut regular = BTreeSet::new();
            let mut owner = Some(prototype);
            while let Some(id) = owner {
                let object = self.heap.get(id)?;
                for member in &object.members {
                    if !member.local
                        && !member.method
                        && !member.entry
                        && !member.external()
                        && !member.definition()
                    {
                        regular.insert(member.name.clone());
                    }
                }
                owner = object.parent;
            }
            members.retain(|member| regular.contains(&member.name));
        }
        members.retain(|descriptor| {
            let mut owner = Some(id);
            while let Some(id) = owner {
                let Ok(object) = self.heap.get(id) else {
                    return false;
                };
                if object.members.iter().any(|m| {
                    !m.entry
                        && !m.local
                        && m.name == descriptor.name
                        && (m.hidden()
                            || self.program.metadata.get(m.code).is_some_and(|m| m.hidden))
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
        self.cached_slot_probe(
            id,
            &key,
            (member.kind != CollectionMemberKind::Property).then_some(&member.key),
        )?
        .ok_or_else(|| invalid("unforced equality member"))
    }
    fn cached_comparison_entries(
        &self,
        id: ObjectId,
    ) -> Result<Vec<(CollectionMember, Value)>, RuntimeError> {
        let members = self.equality_members(id)?;
        let mut result = Vec::new();
        for slot in &self.heap.get(id)?.cache_slots {
            if slot.key.owner.is_some() || slot.key.method {
                continue;
            }
            if let Some(descriptor) = members.iter().find(|member| {
                member.name == slot.key.name
                    && (member.kind != CollectionMemberKind::Property) == slot.key.entry
            }) {
                let mut member = descriptor.clone();
                if slot.key.entry {
                    member.key = self
                        .cache_slot_value_key(id, slot)?
                        .ok_or_else(|| invalid("physical cache entry key"))?;
                }
                result.push((member, slot.value.clone()));
            }
        }
        Ok(result)
    }

    pub(super) fn dynamic_regular_member_count(
        &mut self,
        id: ObjectId,
    ) -> Result<usize, RuntimeError> {
        if let Some(count) = self.heap.get(id)?.dynamic_regular_member_count {
            return Ok(count);
        }
        // VmDynamic.getRegularMemberCount observes cachedValues after force,
        // which can still be incomplete during recursive equality. Its cached
        // result survives publication of the currently evaluating property.
        let count = self.cached_comparison_entries(id)?.len();
        self.heap.get_mut(id)?.dynamic_regular_member_count = Some(count);
        Ok(count)
    }

    /// Equality visits typed properties in class declaration order; collection
    /// equality visits values in successful cache insertion order. Neither order
    /// is the child-first declaration traversal used by VmObject.force.
    fn comparison_members(&self, id: ObjectId) -> Result<Vec<CollectionMember>, RuntimeError> {
        let members = self.equality_members(id)?;
        let mut result = Vec::with_capacity(members.len());
        if let Some(prototype) = self
            .class_identity(id)?
            .map(|class| self.heap.get_class(class).map(|class| class.prototype))
            .transpose()?
            .or(self.heap.get(id)?.is_module.then_some(id))
        {
            // Class identity belongs to the descriptor; declarations and their
            // child-first property order belong to its instance prototype.
            let mut owner = Some(prototype);
            while let Some(id) = owner {
                let object = self.heap.get(id)?;
                for member in &object.members {
                    if !member.local
                        && !member.method
                        && !member.entry
                        && !member.definition()
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
            result.extend(
                self.cached_comparison_entries(id)?
                    .into_iter()
                    .map(|(member, _)| member),
            );
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
            if let Some(elems) = &object.list_elements {
                return Ok(elems.iter().map(|v| (None, v.clone())).collect());
            }
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
                    if member.kind != CollectionMemberKind::Property {
                        self.cache_slot_value_key(id, slot)?
                    } else {
                        None
                    },
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
            Task::Reject => {
                run.reject();
                self.frames[frame_index].equality_run = Some(run);
                Ok(())
            }
            Task::MapRemoveHash(..)
            | Task::MapRemoveLookup(..)
            | Task::MapRemoveCompare(..)
            | Task::MapRemoveMatched(..)
            | Task::MapRemoveConfirmed(..)
            | Task::MapRemoveAbsent(..)
            | Task::MapRemoveOrder(..)
            | Task::MapRemoveFinish(..)
            | Task::SetRestHash(..)
            | Task::SetRestLookup(..)
            | Task::SetRestCompare(..)
            | Task::SetRestMatched(..)
            | Task::SetRestConfirmed(..)
            | Task::SetRestFinish(..)
            | Task::SetRestAppend(..)
            | Task::SuggestionVisit(..)
            | Task::SuggestionHashed(..)
            | Task::SuggestionLookup(..)
            | Task::SuggestionMatched(..)
            | Task::SuggestionCache(..)
            | Task::SuggestionCollect(..)
            | Task::CountKey(..)
            | Task::CountReady(..)
            | Task::CountKeys(..)
            | Task::CountMatched(..) => self.step_equality_collection_ops(frame_index, run, task),
            Task::ResumeForce(..)
            | Task::ReadCache(..)
            | Task::ReadCacheMatch(..)
            | Task::ForceCached(..)
            | Task::ForceCachedMatch(..)
            | Task::PublishCache(..)
            | Task::PublishProbe(..)
            | Task::PublishMatch(..)
            | Task::AppendCache(..)
            | Task::CacheRebuild(..)
            | Task::CacheLink(..) => self.step_equality_cache_ops(frame_index, run, task),
            Task::Compare(..)
            | Task::SetNext(..)
            | Task::SetLookup(..)
            | Task::MapNext(..)
            | Task::MapLookup(..)
            | Task::CompareTypedMember(..)
            | Task::CompareMembers(..)
            | Task::Match(..)
            | Task::Matched(..) => self.step_equality_compare(frame_index, run, task),
            Task::Deep(..)
            | Task::DiscardForcedValue
            | Task::Prepare(..)
            | Task::HashMembers(..)
            | Task::Resolve(..)
            | Task::Resolved(..)
            | Task::Read(..)
            | Task::Await(..)
            | Task::Build(..) => self.step_equality_hash_and_build(frame_index, run, task),
        }
    }

    #[cfg_attr(target_arch = "wasm32", inline(never))]
    fn step_equality_collection_ops(
        &mut self,
        frame_index: usize,
        mut run: Box<EqualityRun>,
        task: Task,
    ) -> Result<(), RuntimeError> {
        match task {
            Task::MapRemoveHash(source, key, removing) => {
                // A second hash must recompute zero/uncached structural hashes
                // and collection hashes. Do not reuse prepare_keys' per-bytecode
                // completion set or suppress recursive forcing from the prior lookup.
                run.forced.clear();
                run.tasks
                    .push(Task::MapRemoveLookup(source, key.clone(), removing));
                run.tasks.push(Task::Prepare(key));
            }
            Task::MapRemoveLookup(source, key, removing) => {
                let hash = self.java_hash(&key)?;
                let index = self.heap.get(source)?.map_index();
                let candidates = index.candidates(hash);
                let repeat_equality = !removing && index.repeats_lookup_equality(hash);
                run.tasks.push(Task::MapRemoveCompare(
                    Box::new(SetRestLookup {
                        source,
                        key,
                        removing,
                        hash,
                        candidates,
                        repeat_equality,
                    }),
                    0,
                ));
            }
            Task::MapRemoveCompare(state, position) => {
                if let Some(slot) = state.candidates.get(position).copied() {
                    let candidate = self.heap.get(state.source)?.collection_members[slot]
                        .key
                        .clone();
                    let depth = run.tasks.len();
                    run.on_false
                        .push((depth, Task::MapRemoveCompare(state.clone(), position + 1)));
                    run.tasks.push(Task::MapRemoveMatched(state.clone(), slot));
                    run.forced.clear();
                    run.tasks
                        .push(Task::Compare(state.key.clone(), candidate, false));
                } else if state.removing {
                    run.tasks
                        .push(Task::MapRemoveOrder(state.source, state.key, None, 0));
                } else {
                    self.stack.push(Value::Object(state.source));
                }
            }
            Task::MapRemoveMatched(state, slot) => {
                run.on_false
                    .pop()
                    .ok_or_else(|| invalid("Map removal comparison"))?;
                if state.repeat_equality {
                    let candidate = self.heap.get(state.source)?.collection_members[slot]
                        .key
                        .clone();
                    let depth = run.tasks.len();
                    run.on_false
                        .push((depth, Task::MapRemoveAbsent(state.source)));
                    run.tasks.push(Task::MapRemoveConfirmed(state.clone()));
                    run.forced.clear();
                    run.tasks
                        .push(Task::Compare(state.key.clone(), candidate, false));
                } else if state.removing {
                    run.tasks.push(Task::MapRemoveOrder(
                        state.source,
                        state.key.clone(),
                        Some((slot, state.hash)),
                        0,
                    ));
                } else {
                    run.tasks
                        .push(Task::MapRemoveHash(state.source, state.key.clone(), true));
                }
            }
            Task::MapRemoveConfirmed(state) => {
                run.on_false
                    .pop()
                    .ok_or_else(|| invalid("Map collision confirmation"))?;
                run.tasks
                    .push(Task::MapRemoveHash(state.source, state.key.clone(), true));
            }
            Task::MapRemoveAbsent(source) => {
                self.stack.push(Value::Object(source));
            }
            Task::MapRemoveOrder(source, key, removed, position) => {
                let members = collections::ordered_members(&self.heap, source)?;
                let Some(member) = members.get(position) else {
                    // Upstream invokes RrbTree.without(-1), an internal exception.
                    // Its external diagnostic protocol remains an explicit gap.
                    return Err(invalid("Map.remove keyOrder.indexOf returned -1"));
                };
                let depth = run.tasks.len();
                run.on_false.push((
                    depth,
                    Task::MapRemoveOrder(source, key.clone(), removed, position + 1),
                ));
                run.tasks
                    .push(Task::MapRemoveFinish(source, removed, position));
                run.forced.clear();
                // UnmodList.indexOf uses stored.equals(query), unlike trie lookup.
                run.tasks
                    .push(Task::Compare(member.key.clone(), key, false));
            }
            Task::MapRemoveFinish(source, removed, position) => {
                run.on_false
                    .pop()
                    .ok_or_else(|| invalid("Map.remove order comparison"))?;
                let result = self.finish_map_remove(source, removed, position)?;
                self.stack.push(Value::Object(result));
            }
            Task::SetRestHash(source, key, removing) => {
                // A second hash must recompute zero/uncached structural hashes
                // and collection hashes. Do not reuse prepare_keys' per-bytecode
                // completion set or suppress recursive forcing from the prior lookup.
                run.forced.clear();
                run.tasks
                    .push(Task::SetRestLookup(source, key.clone(), removing));
                run.tasks.push(Task::Prepare(key));
            }
            Task::SetRestLookup(source, key, removing) => {
                let hash = self.java_hash(&key)?;
                let index = self.heap.get(source)?.map_index();
                let candidates = index.candidates(hash);
                let repeat_equality = !removing && index.repeats_lookup_equality(hash);
                run.tasks.push(Task::SetRestCompare(
                    Box::new(SetRestLookup {
                        source,
                        key,
                        removing,
                        hash,
                        candidates,
                        repeat_equality,
                    }),
                    0,
                ));
            }
            Task::SetRestCompare(state, position) => {
                if let Some(slot) = state.candidates.get(position).copied() {
                    let candidate = self.heap.get(state.source)?.set_entries[slot].clone();
                    let depth = run.tasks.len();
                    run.on_false
                        .push((depth, Task::SetRestCompare(state.clone(), position + 1)));
                    run.tasks.push(Task::SetRestMatched(state.clone(), slot));
                    run.forced.clear();
                    run.tasks
                        .push(Task::Compare(state.key.clone(), candidate, false));
                } else {
                    run.tasks.push(Task::SetRestFinish(state.source, None));
                }
            }
            Task::SetRestMatched(state, slot) => {
                run.on_false
                    .pop()
                    .ok_or_else(|| invalid("Set removal comparison"))?;
                if state.repeat_equality {
                    let candidate = self.heap.get(state.source)?.set_entries[slot].clone();
                    let depth = run.tasks.len();
                    run.on_false
                        .push((depth, Task::SetRestFinish(state.source, None)));
                    run.tasks.push(Task::SetRestConfirmed(state.clone()));
                    run.forced.clear();
                    run.tasks
                        .push(Task::Compare(state.key.clone(), candidate, false));
                } else if state.removing {
                    run.tasks
                        .push(Task::SetRestFinish(state.source, Some((slot, state.hash))));
                } else {
                    run.tasks
                        .push(Task::SetRestHash(state.source, state.key.clone(), true));
                }
            }
            Task::SetRestConfirmed(state) => {
                run.on_false
                    .pop()
                    .ok_or_else(|| invalid("Set collision confirmation"))?;
                run.tasks
                    .push(Task::SetRestHash(state.source, state.key.clone(), true));
            }
            Task::SetRestFinish(source, removed) => {
                let (target, values) = self.prepare_set_rest_result(source, removed)?;
                if values.is_empty() {
                    self.stack.push(Value::Object(target));
                } else {
                    run.roots.push(Value::Object(target));
                    run.tasks
                        .push(Task::SetRestAppend(target, values.into(), 0));
                }
            }
            Task::SetRestAppend(target, values, position) => {
                if let Some(value) = values.get(position).cloned() {
                    run.tasks
                        .push(Task::SetRestAppend(target, values, position + 1));
                    // Element publication uses the shared cache scheduler. Keep
                    // its continuation installed while appending, and complete
                    // each cache insertion before appending the next element.
                    self.frames[frame_index].equality_run = Some(run);
                    self.stack.push(value);
                    self.append_strict_element()?;
                    return Ok(());
                }
                let builder = self.frames[frame_index].builders.pop();
                if builder != Some(target) {
                    return Err(invalid("Set.restOrNull result builder"));
                }
                let result = self.finish_strict_collection(target)?;
                self.stack.push(Value::Object(result));
            }
            Task::SuggestionVisit(state) => {
                if let Some(item) = state.members.get(state.position) {
                    let key = item.descriptor.key.clone();
                    run.tasks.push(Task::SuggestionHashed(state));
                    run.tasks.push(Task::Prepare(key));
                } else {
                    self.frames[frame_index].callsite_hidden = false;
                    return Err(self.key_suggestions_error(&state.query, state.candidates));
                }
            }
            Task::SuggestionHashed(mut state) => {
                let item = &state.members[state.position];
                state.hash = self.java_hash(&item.descriptor.key)?;
                let candidates = if state.map {
                    // VmMap.iterator.next calls map.get(key) before its consumer.
                    let object = self.heap.get(state.receiver)?;
                    object
                        .map_index()
                        .candidates(state.hash)
                        .into_iter()
                        .map(|index| object.collection_members[index].key.clone())
                        .collect::<Vec<_>>()
                } else {
                    state
                        .seen
                        .iter()
                        .filter(|(other, hash)| {
                            (other.kind == CollectionMemberKind::Property)
                                == (item.descriptor.kind == CollectionMemberKind::Property)
                                && !hash
                                    .zip(state.hash)
                                    .is_some_and(|(left, right)| left != right)
                        })
                        .map(|(other, _)| other.key.clone())
                        .collect::<Vec<_>>()
                };
                run.roots.extend(candidates.iter().cloned());
                let lookup = if state.map {
                    SuggestionLookup::Map
                } else {
                    SuggestionLookup::Visited
                };
                run.tasks
                    .push(Task::SuggestionLookup(state, lookup, candidates, 0));
            }
            Task::SuggestionLookup(mut state, lookup, candidates, index) => {
                let item = state.members[state.position].clone();
                let key = item.descriptor.key.clone();
                if let Some(other) = candidates.get(index).cloned() {
                    if same_key(&self.heap, &key, &other) {
                        if matches!(lookup, SuggestionLookup::Visited) {
                            state.position += 1;
                            run.tasks.push(Task::SuggestionVisit(state));
                        } else {
                            run.tasks.push(Task::SuggestionCollect(state));
                        }
                    } else if matches!((&key, &other), (Value::Object(_), Value::Object(_))) {
                        let depth = run.tasks.len();
                        run.on_false.push((
                            depth,
                            Task::SuggestionLookup(state.clone(), lookup, candidates, index + 1),
                        ));
                        run.tasks
                            .push(Task::SuggestionMatched(state, lookup, other.clone()));
                        // HashSet.add and cache/map lookup call query.equals(stored).
                        run.tasks.push(Task::Compare(key, other, false));
                    } else {
                        run.tasks.push(Task::SuggestionLookup(
                            state,
                            lookup,
                            candidates,
                            index + 1,
                        ));
                    }
                } else {
                    match lookup {
                        SuggestionLookup::Visited => {
                            state.seen.push((item.descriptor.clone(), state.hash));
                            if item.skip {
                                state.position += 1;
                                run.tasks.push(Task::SuggestionVisit(state));
                            } else {
                                // iterateMemberValues reads cached values before the
                                // consumer filters keys, but never forces a value.
                                let hashed = self.heap.get(state.receiver)?.cache_index.has_hash();
                                run.tasks.push(Task::SuggestionCache(state));
                                if hashed {
                                    run.tasks.push(Task::Prepare(key));
                                }
                            }
                        }
                        SuggestionLookup::Cached => run.tasks.push(Task::SuggestionCollect(state)),
                        SuggestionLookup::Map => {
                            return Err(invalid("Map iterator lost its stored key"));
                        }
                    }
                }
            }
            Task::SuggestionMatched(mut state, lookup, other) => {
                run.on_false
                    .pop()
                    .ok_or_else(|| invalid("missing suggestion comparison branch"))?;
                if let (Value::Object(query), Value::Object(stored)) =
                    (&state.members[state.position].descriptor.key, other)
                {
                    self.heap.key_equalities.insert((*query, stored));
                }
                if matches!(lookup, SuggestionLookup::Visited) {
                    state.position += 1;
                    run.tasks.push(Task::SuggestionVisit(state));
                } else {
                    run.tasks.push(Task::SuggestionCollect(state));
                }
            }
            Task::SuggestionCache(state) => {
                let descriptor = &state.members[state.position].descriptor;
                let entry = descriptor.kind != CollectionMemberKind::Property;
                let object = self.heap.get(state.receiver)?;
                let indices = if object.cache_index.has_hash() {
                    let key = CacheKey {
                        owner: None,
                        name: descriptor.name.clone(),
                        method: false,
                        entry,
                    };
                    object
                        .cache_index
                        .candidates(Some(self.cache_key_hash(state.receiver, &key)?))
                } else {
                    (0..object.cache_slots.len()).collect()
                };
                let mut candidates = Vec::new();
                for index in indices {
                    let slot = &object.cache_slots[index];
                    if slot.key.owner.is_some() || slot.key.method || slot.key.entry != entry {
                        continue;
                    }
                    if entry {
                        if let Some(key) =
                            collections::cache_slot_entry_key(&self.heap, state.receiver, slot)?
                        {
                            candidates.push(key);
                        }
                    } else {
                        candidates.push(Value::string(slot.key.name.clone()));
                    }
                }
                run.roots.extend(candidates.iter().cloned());
                run.tasks.push(Task::SuggestionLookup(
                    state,
                    SuggestionLookup::Cached,
                    candidates,
                    0,
                ));
            }
            Task::SuggestionCollect(mut state) => {
                let descriptor = &state.members[state.position].descriptor;
                if descriptor.kind != CollectionMemberKind::Property
                    && let Value::String(key) = &descriptor.key
                {
                    state.candidates.push(key.clone());
                }
                state.position += 1;
                run.tasks.push(Task::SuggestionVisit(state));
            }
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
            _ => unreachable!("equality collection op"),
        }
        self.frames[frame_index].equality_run = Some(run);
        Ok(())
    }

    #[cfg_attr(target_arch = "wasm32", inline(never))]
    fn step_equality_cache_ops(
        &mut self,
        frame_index: usize,
        mut run: Box<EqualityRun>,
        task: Task,
    ) -> Result<(), RuntimeError> {
        match task {
            Task::ResumeForce(request) => {
                let object = self.heap.get(request.receiver)?;
                let indices = if object.cache_index.has_hash() {
                    object.cache_index.candidates(Some(
                        self.java_hash(
                            request
                                .probe
                                .as_ref()
                                .ok_or_else(|| invalid("cache lookup probe"))?,
                        )?
                        .unwrap_or(0),
                    ))
                } else {
                    (0..object.cache_slots.len()).collect()
                };
                run.tasks.push(Task::ReadCache(request, indices, 0));
            }
            Task::ReadCache(request, indices, position) => {
                if let Some(slot) = indices
                    .get(position)
                    .and_then(|index| {
                        self.heap
                            .get(request.receiver)
                            .ok()?
                            .cache_slots
                            .get(*index)
                    })
                    .cloned()
                {
                    let stored =
                        collections::cache_slot_entry_key(&self.heap, request.receiver, &slot)?;
                    let query = request
                        .probe
                        .clone()
                        .ok_or_else(|| invalid("cache lookup probe"))?;
                    let retry = Task::ReadCache(request, indices, position + 1);
                    if slot.key.entry
                        && slot.key.owner.is_none()
                        && !slot.key.method
                        && let Some(stored) = stored
                    {
                        if identical_key(&self.heap, &query, &stored) {
                            self.stack.push(slot.value);
                        } else {
                            let depth = run.tasks.len();
                            run.on_false.push((depth, retry));
                            run.roots
                                .extend([query.clone(), stored.clone(), slot.value.clone()]);
                            run.tasks.push(Task::ReadCacheMatch(slot.value));
                            run.tasks.push(Task::Compare(query, stored, false));
                        }
                    } else {
                        run.tasks.push(retry);
                    }
                } else {
                    self.frames[frame_index].equality_run = Some(run);
                    return self.force_key_mode_probe(
                        request.receiver,
                        request.owner,
                        request.name,
                        request.local,
                        request.memoize,
                        request.method,
                        request.entry,
                        request.predicate,
                        true,
                        None,
                        request.probe,
                    );
                }
            }
            Task::ReadCacheMatch(value) => {
                run.on_false
                    .pop()
                    .ok_or_else(|| invalid("cache lookup branch"))?;
                self.stack.push(value);
            }
            Task::ForceCached(receiver, owner, member, candidates, index) => {
                let key = CacheKey {
                    entry: true,
                    owner: None,
                    method: false,
                    name: member.name.clone(),
                };
                let query = self
                    .cache_value_key(receiver, &key)?
                    .ok_or_else(|| invalid("force entry key"))?;
                let candidates = match candidates {
                    Some(candidates) => candidates,
                    None => {
                        let object = self.heap.get(receiver)?;
                        if object.cache_index.has_hash() {
                            object
                                .cache_index
                                .candidates(Some(self.cache_key_hash(receiver, &key)?))
                        } else {
                            (0..object.cache_slots.len()).collect()
                        }
                    }
                };
                if let Some(slot) = candidates
                    .get(index)
                    .and_then(|index| self.heap.get(receiver).ok()?.cache_slots.get(*index))
                    .cloned()
                {
                    let retry =
                        Task::ForceCached(receiver, owner, member, Some(candidates), index + 1);
                    if slot.key.owner.is_some() || !slot.key.entry || slot.key.method {
                        run.tasks.push(retry);
                    } else {
                        let stored = self
                            .cache_slot_value_key(receiver, &slot)?
                            .ok_or_else(|| invalid("cached entry key"))?;
                        if same_key(&self.heap, &query, &stored) {
                            self.stack.push(slot.value);
                        } else {
                            let depth = run.tasks.len();
                            run.on_false.push((depth, retry));
                            run.roots
                                .extend([query.clone(), stored.clone(), slot.value.clone()]);
                            run.tasks.push(Task::ForceCachedMatch(
                                query.clone(),
                                stored.clone(),
                                slot.value,
                            ));
                            run.tasks.push(Task::Compare(query, stored, false));
                        }
                    }
                } else {
                    self.frames[frame_index].equality_run = Some(run);
                    return self.force_key_mode_prepared(
                        receiver,
                        owner,
                        member.name.clone(),
                        false,
                        true,
                        false,
                        true,
                        false,
                        true,
                        Some(member),
                    );
                }
            }
            Task::ForceCachedMatch(query, stored, value) => {
                run.on_false
                    .pop()
                    .ok_or_else(|| invalid("missing force cache branch"))?;
                if let (Value::Object(query), Value::Object(stored)) = (query, stored) {
                    self.heap.key_equalities.insert((query, stored));
                }
                self.stack.push(value);
            }
            Task::PublishCache(id, key, value, probe) => {
                // Bootstrap strict collections can carry completed scalar slots
                // before their object is installed in the heap.
                while self.heap.get(id)?.cache_index.len() < self.heap.get(id)?.cache_slots.len() {
                    let object = self.heap.get_mut(id)?;
                    object.cache_index.grow_before_append();
                    object.cache_index.push();
                }
                if let Some(probe) = probe {
                    let object = self.heap.get(id)?;
                    let indices = if object.cache_index.has_hash() {
                        object
                            .cache_index
                            .candidates(Some(self.java_hash(&probe)?.unwrap_or(0)))
                    } else {
                        (0..object.cache_slots.len()).collect()
                    };
                    run.tasks
                        .push(Task::PublishProbe(id, key, value, probe, indices, 0));
                    self.frames[frame_index].equality_run = Some(run);
                    return Ok(());
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
                    run.tasks.push(Task::AppendCache(id, key, value, None));
                    if self.heap.get_mut(id)?.cache_index.grow_before_append() {
                        self.heap.get_mut(id)?.cache_index.start_rehash();
                        run.tasks.push(Task::CacheRebuild(id, 0));
                    }
                }
            }
            Task::PublishProbe(id, key, value, probe, indices, position) => {
                if let Some(index) = indices.get(position).copied() {
                    let slot = self.heap.get(id)?.cache_slots[index].clone();
                    let stored = self.cache_slot_value_key(id, &slot)?;
                    let retry = Task::PublishProbe(
                        id,
                        key.clone(),
                        value.clone(),
                        probe.clone(),
                        indices,
                        position + 1,
                    );
                    if slot.key.entry
                        && slot.key.owner.is_none()
                        && !slot.key.method
                        && let Some(stored) = stored
                    {
                        if identical_key(&self.heap, &probe, &stored) {
                            let object = self.heap.get_mut(id)?;
                            object.cache_slots[index].value = value.clone();
                            object.cache.insert(key, LazyState::Completed(value));
                        } else {
                            let depth = run.tasks.len();
                            run.on_false.push((depth, retry));
                            run.roots
                                .extend([probe.clone(), stored.clone(), value.clone()]);
                            run.tasks.push(Task::PublishMatch(id, key, value, index));
                            run.tasks.push(Task::Compare(probe, stored, false));
                        }
                    } else {
                        run.tasks.push(retry);
                    }
                } else {
                    run.tasks
                        .push(Task::AppendCache(id, key, value, Some(probe)));
                    if self.heap.get_mut(id)?.cache_index.grow_before_append() {
                        self.heap.get_mut(id)?.cache_index.start_rehash();
                        run.tasks.push(Task::CacheRebuild(id, 0));
                    }
                }
            }
            Task::PublishMatch(id, key, value, index) => {
                run.on_false
                    .pop()
                    .ok_or_else(|| invalid("cache publication branch"))?;
                let object = self.heap.get_mut(id)?;
                // EconomicMap replaces the value, retaining the original key object.
                object.cache_slots[index].value = value.clone();
                object.cache.insert(key, LazyState::Completed(value));
            }
            Task::AppendCache(id, key, value, probe) => {
                // Warming local slots or publishing duplicate slots can change
                // directional object equality, including that of enclosing keys.
                if key.owner.is_some()
                    || matches!(
                        self.heap.get(id)?.cache.get(&key),
                        Some(LazyState::Completed(_))
                    )
                {
                    self.heap.key_equalities.clear();
                }
                let object = self.heap.get_mut(id)?;
                let index = object.cache_index.push();
                object.completed_cache_order.push(key.clone());
                object.cache_slots.push(crate::heap::CacheSlot {
                    key: key.clone(),
                    entry_key: probe.clone(),
                    value: value.clone(),
                });
                object
                    .cache
                    .insert(key.clone(), LazyState::Completed(value));
                if object.cache_index.has_hash() {
                    run.tasks.push(Task::CacheLink(id, index, true));
                    if let Some(value) = probe {
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
                    if let Some(value) = self.cache_slot_value_key(id, &slot)? {
                        run.roots.push(value.clone());
                        run.tasks.push(Task::Prepare(value));
                    }
                }
            }
            Task::CacheLink(id, index, allow_rehash) => {
                let slot = &self.heap.get(id)?.cache_slots[index];
                let hash = if let Some(probe) = self.cache_slot_value_key(id, slot)? {
                    self.java_hash(&probe)?.unwrap_or(0)
                } else {
                    self.cache_key_hash(id, &slot.key)?
                };
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
            _ => unreachable!("equality cache op"),
        }
        self.frames[frame_index].equality_run = Some(run);
        Ok(())
    }

    #[cfg_attr(target_arch = "wasm32", inline(never))]
    fn step_equality_compare(
        &mut self,
        frame_index: usize,
        mut run: Box<EqualityRun>,
        task: Task,
    ) -> Result<(), RuntimeError> {
        match task {
            Task::Compare(left, right, semantic) => {
                // EqualNode only dispatches same-class objects through VmValue.
                // Two foreign Map.Entry values therefore reach its unsupported
                // specialization, even when they are the same Java object.
                // Boxed collection equality still uses Map.Entry.equals below.
                if semantic
                    && let (Value::Object(a), Value::Object(b)) = (&left, &right)
                    && self.heap.get(*a)?.foreign_map_entry.is_some()
                    && self.heap.get(*b)?.foreign_map_entry.is_some()
                {
                    let frame = &self.frames[frame_index];
                    let metadata = self.program.metadata.get(frame.block);
                    let source = metadata
                        .and_then(|m| m.source.as_ref())
                        .or(self.context.primary_source.as_ref());
                    let span = metadata.and_then(|m| {
                        frame
                            .pc
                            .checked_sub(1)
                            .and_then(|i| m.spans.get(i).copied().flatten())
                    });
                    let location = match (source, span) {
                        (Some(src), Some(sp)) => {
                            let line = src
                                .text
                                .index()
                                .position(sp.start)
                                .map_or(1, |pos| pos.line);
                            let expr = src
                                .text
                                .slice(sp)
                                .map(|s| s.to_pkl_string())
                                .unwrap_or_default();
                            PklString::from("(")
                                .concat(&src.uri)
                                .concat(&format!(":{line}) ").into())
                                .concat(&expr)
                        }
                        _ => PklString::from("(unknown)"),
                    };
                    let render_operand = |val: &Value| -> Result<PklString, RuntimeError> {
                        if let Value::Object(id) = val
                            && let Some(entry) = &self.heap.get(*id)?.foreign_map_entry
                        {
                            let key = crate::diagnostic_values::render_string(
                                &entry[0],
                                &self.heap,
                                &self.program.module_name,
                                usize::MAX,
                            )?;
                            let value = crate::diagnostic_values::render_string(
                                &entry[1],
                                &self.heap,
                                &self.program.module_name,
                                usize::MAX,
                            )?;
                            return Ok(key.concat(&"=".into()).concat(&value));
                        }
                        crate::diagnostic_values::render_string(
                            val,
                            &self.heap,
                            &self.program.module_name,
                            usize::MAX,
                        )
                    };
                    let left_str = render_operand(&left)?;
                    let right_str = render_operand(&right)?;
                    let detail = PklString::from("Unexpected values provided for ")
                        .concat(&location)
                        .concat(&": [".into())
                        .concat(&left_str)
                        .concat(&", ".into())
                        .concat(&right_str)
                        .concat(&"], [KeyValueHolder,KeyValueHolder]".into());
                    return Err(RuntimeError::foreign(
                        crate::java_exception::provenance::ForeignOperation::KeyValueHolderEqualitySpecialization {
                            detail: detail.clone(),
                        },
                        detail,
                    ));
                }
                let equal = match (&left, &right) {
                    (Value::Int(a), Value::Float(b)) if semantic => Some(a.get() as f64 == **b),
                    (Value::Float(a), Value::Int(b)) if semantic => Some(*a == b.get() as f64),
                    (Value::Float(a), Value::Float(b)) if semantic => Some(a == b),
                    (Value::Object(a), Value::Object(b)) if a == b => Some(true),
                    (Value::Object(a), Value::Object(b)) => {
                        let x = self.heap.get(*a)?;
                        let y = self.heap.get(*b)?;
                        if x.foreign_map_entry.is_some() || y.foreign_map_entry.is_some() {
                            match (&x.foreign_map_entry, &y.foreign_map_entry) {
                                (Some(x), Some(y)) => {
                                    run.tasks.push(Task::Compare(
                                        x[1].clone(),
                                        y[1].clone(),
                                        false,
                                    ));
                                    run.tasks.push(Task::Compare(
                                        x[0].clone(),
                                        y[0].clone(),
                                        false,
                                    ));
                                    None
                                }
                                _ => Some(false),
                            }
                        } else if x.pair.is_some() || y.pair.is_some() {
                            match (&x.pair, &y.pair) {
                                (Some(x), Some(y)) => {
                                    run.tasks.push(Task::Compare(
                                        x[1].clone(),
                                        y[1].clone(),
                                        false,
                                    ));
                                    run.tasks.push(Task::Compare(
                                        x[0].clone(),
                                        y[0].clone(),
                                        false,
                                    ));
                                    None
                                }
                                _ => Some(false),
                            }
                        } else if x.reference.is_some() || y.reference.is_some() {
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
                        } else {
                            // `VmTyped.equals` compares `clazz == other.clazz` without
                            // requiring matching `isModuleObject()`, so e.g.
                            // `Map(...).toTyped(OpenMod.getClass()) == new OpenMod {}`
                            // succeeds when both share the same `class_identity`.
                            let left_class = self.class_identity(*a)?;
                            let right_class = self.class_identity(*b)?;
                            if x.collection_kind != y.collection_kind
                                || left_class != right_class
                                || (left_class.is_none() && x.is_module != y.is_module)
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
                                if let (Some(lx), Some(ry)) = (&x.list_elements, &y.list_elements) {
                                    for (l, r) in lx.iter().zip(ry.iter()).rev() {
                                        run.roots.extend([l.clone(), r.clone()]);
                                        run.tasks.push(Task::Compare(l.clone(), r.clone(), false));
                                    }
                                } else {
                                    let lhs = collections::ordered_members(&self.heap, *a)?;
                                    let rhs = collections::ordered_members(&self.heap, *b)?;
                                    for (l, r) in lhs.iter().zip(&rhs).rev() {
                                        let l = self.cached_equality_member(*a, l)?;
                                        let r = self.cached_equality_member(*b, r)?;
                                        run.roots.extend([l.clone(), r.clone()]);
                                        run.tasks.push(Task::Compare(l, r, false));
                                    }
                                }
                                None
                            } else {
                                run.tasks.push(Task::CompareMembers(*a, *b));
                                run.tasks.push(Task::Deep(right));
                                run.tasks.push(Task::Deep(left));
                                None
                            }
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
            Task::CompareTypedMember(a, b, member) => {
                // VmTyped.equals reads the next property only after the previous
                // comparison succeeds. A recursively forced receiver can still
                // have an in-progress later property with no cached value.
                let left = self.cached_equality_member(a, &member)?;
                let key = CacheKey {
                    owner: None,
                    name: member.name.clone(),
                    method: false,
                    entry: member.kind != CollectionMemberKind::Property,
                };
                if let Some(right) = self.cached_slot(b, &key)? {
                    run.roots.extend([left.clone(), right.clone()]);
                    run.tasks.push(Task::Compare(left, right, false));
                } else {
                    // Java compares a present left value with its missing-right
                    // null sentinel. That sentinel is not a Pkl null value.
                    run.reject();
                }
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
                if typed {
                    if self.equality_members(a)?.len() != rhs.len() {
                        run.reject();
                    } else {
                        for member in lhs.into_iter().rev() {
                            run.tasks.push(Task::CompareTypedMember(a, b, member));
                        }
                    }
                    self.frames[frame_index].equality_run = Some(run);
                    return Ok(());
                }
                let mut entries = Vec::new();
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
                        let mut member = member.clone();
                        if slot.key.entry {
                            member.key = self
                                .cache_slot_value_key(a, slot)?
                                .ok_or_else(|| invalid("left equality cache key"))?;
                        }
                        entries.push((Some(member), None, slot.value.clone()));
                    }
                }
                let different_length = if self.heap.get(a)?.collection_kind
                    == CollectionKind::Dynamic
                {
                    self.dynamic_regular_member_count(a)? != self.dynamic_regular_member_count(b)?
                } else {
                    self.equality_members(a)?.len() != rhs.len()
                };
                if different_length {
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
                                let key = if slot.key.entry {
                                    self.cache_slot_value_key(b, slot)?
                                        .ok_or_else(|| invalid("right equality cache key"))?
                                } else {
                                    other.key.clone()
                                };
                                run.roots.extend([key.clone(), slot.value.clone()]);
                                candidates.push((key, slot.value.clone()));
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
            _ => unreachable!("equality compare op"),
        }
        self.frames[frame_index].equality_run = Some(run);
        Ok(())
    }

    #[cfg_attr(target_arch = "wasm32", inline(never))]
    fn step_equality_hash_and_build(
        &mut self,
        frame_index: usize,
        mut run: Box<EqualityRun>,
        task: Task,
    ) -> Result<(), RuntimeError> {
        match task {
            Task::Deep(value) => {
                if let Value::Object(id) = value
                    && run.forced.insert(id)
                {
                    run.tasks.push(Task::DiscardForcedValue);
                    self.frames[frame_index].callsite_hidden = run.hide_callsite;
                    self.frames[frame_index].equality_run = Some(run);
                    self.force_value_frame(Value::Object(id))?;
                    return Ok(());
                }
            }
            Task::DiscardForcedValue => {
                self.pop()?;
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
                    if self.heap.get(id)?.pair.is_none()
                        && self.heap.get(id)?.foreign_map_entry.is_none()
                        && !matches!(
                            self.heap.get(id)?.collection_kind,
                            CollectionKind::List | CollectionKind::Map | CollectionKind::Set
                        )
                    {
                        run.tasks.push(Task::Deep(Value::Object(id)));
                    }
                }
            }
            Task::HashMembers(id) => {
                // Java completes recursive value forcing before computing member
                // hashes. Nested Mapping keys can still be lazy at that point.
                // Hashing then follows cache order (or class property order).
                if let Some(entry) = &self.heap.get(id)?.foreign_map_entry {
                    run.tasks
                        .extend(entry.iter().rev().cloned().map(Task::Prepare));
                    self.frames[frame_index].equality_run = Some(run);
                    return Ok(());
                }
                if let Some(pair) = &self.heap.get(id)?.pair {
                    run.tasks
                        .extend(pair.iter().rev().cloned().map(Task::Prepare));
                    self.frames[frame_index].equality_run = Some(run);
                    return Ok(());
                }
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
            Task::Resolve(id, member, candidates, index, prepare) => {
                if member.kind == CollectionMemberKind::Property {
                    run.tasks.push(Task::Read(id, member, prepare, None));
                } else if let Some(candidate) = candidates.get(index).cloned() {
                    if candidate.kind == CollectionMemberKind::Property {
                        run.tasks
                            .push(Task::Resolve(id, member, candidates, index + 1, prepare));
                    } else if same_key(&self.heap, &member.key, &candidate.key) {
                        run.tasks
                            .push(Task::Read(id, candidate, prepare, Some(member.key)));
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
                if let (Value::Object(a), Value::Object(b)) = (&key, &member.key) {
                    self.heap.key_equalities.insert((*a, *b));
                }
                run.tasks.push(Task::Read(id, member, prepare, Some(key)));
            }
            Task::Read(id, member, prepare, probe) => {
                if let Some(prepare) = prepare {
                    run.tasks.push(Task::Await(prepare));
                }
                if let Some(elems) = &self.heap.get(id)?.list_elements
                    && member.kind == CollectionMemberKind::Element
                    && let Value::Int(idx) = &member.key
                    && idx.get() >= 0
                {
                    let value = elems
                        .get(idx.get() as usize)
                        .cloned()
                        .ok_or_else(|| invalid("missing effective collection member"))?;
                    self.stack.push(value);
                    self.frames[frame_index].equality_run = Some(run);
                    return Ok(());
                }
                let hide_callsite = run.hide_callsite;
                self.frames[frame_index].equality_run = Some(run);
                self.frames[frame_index].callsite_hidden = hide_callsite;
                return self.force_key_mode_probe(
                    id,
                    id,
                    member.name,
                    false,
                    true,
                    false,
                    member.kind != CollectionMemberKind::Property,
                    false,
                    false,
                    None,
                    probe,
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
                if let Some(entry) = object.foreign_map_entry.clone() {
                    let key = StructuralKey {
                        kind: 8,
                        class: None,
                        members: vec![
                            (false, Atom::Int(0), atom(&self.heap, &entry[0])?),
                            (false, Atom::Int(1), atom(&self.heap, &entry[1])?),
                        ],
                    };
                    let hash = self
                        .java_hash(&entry[0])?
                        .zip(self.java_hash(&entry[1])?)
                        .map(|(key, value)| key ^ value);
                    let next = self.heap.next_structural_key;
                    let value = *self.heap.structural_keys.entry(key).or_insert_with(|| {
                        self.heap.next_structural_key += 1;
                        next
                    });
                    let object = self.heap.get_mut(id)?;
                    object.structural_key = Some(value);
                    object.java_hash = Some(hash);
                    run.active.remove(&id);
                    self.frames[frame_index].equality_run = Some(run);
                    return Ok(());
                }
                if let Some(pair) = object.pair.clone() {
                    let key = StructuralKey {
                        kind: 7,
                        class: None,
                        members: vec![
                            (false, Atom::Int(0), atom(&self.heap, &pair[0])?),
                            (false, Atom::Int(1), atom(&self.heap, &pair[1])?),
                        ],
                    };
                    let hash = self
                        .java_hash(&pair[0])?
                        .zip(self.java_hash(&pair[1])?)
                        .map(|(a, b)| a.wrapping_mul(31).wrapping_add(b));
                    let next = self.heap.next_structural_key;
                    let value = *self.heap.structural_keys.entry(key).or_insert_with(|| {
                        self.heap.next_structural_key += 1;
                        next
                    });
                    let object = self.heap.get_mut(id)?;
                    object.structural_key = Some(value);
                    object.java_hash = Some(hash);
                    run.active.remove(&id);
                    self.frames[frame_index].equality_run = Some(run);
                    return Ok(());
                }
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
                    // Reentrant force can return with only a cache prefix ready.
                    // VmDynamic/VmListing/VmMapping hash that current cache;
                    // constructing our internal key must not demand later values
                    // which Java's hash traversal never reads.
                    let entries = if self.class_identity(id)?.is_none()
                        && !self.heap.get(id)?.is_module
                        && matches!(
                            self.heap.get(id)?.collection_kind,
                            CollectionKind::Dynamic
                                | CollectionKind::Listing
                                | CollectionKind::Mapping
                        ) {
                        self.cached_comparison_entries(id)?
                    } else {
                        self.equality_members(id)?
                            .into_iter()
                            .map(|member| {
                                let value = self.cached_equality_member(id, &member)?;
                                Ok((member, value))
                            })
                            .collect::<Result<Vec<_>, RuntimeError>>()?
                    };
                    for (member, value) in entries {
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
            _ => unreachable!("equality hash/build op"),
        }
        self.frames[frame_index].equality_run = Some(run);
        Ok(())
    }
}
