//! Evaluator-owned tracing arena. No object edge uses reference counting.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::value::{ErrorKind, ObjectId, RuntimeError, Value};

static NEXT_HEAP: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Scope {
    pub receiver: ObjectId,
    pub owner: ObjectId,
}

#[derive(Clone, Debug, Default)]
pub struct Environment {
    pub locals: Vec<Value>,
    pub this_value: Option<Value>,
    pub outer_value: Option<Value>,
    pub custom_this: Option<CustomThisSlot>,
    /// Innermost first; immutable captured lexical object scopes.
    pub scopes: Vec<Scope>,
}

#[derive(Clone, Debug)]
pub struct Member {
    pub entry: bool,
    pub name: Arc<str>,
    pub code: usize,
    pub local: bool,
    pub method: bool,
    pub ty: Option<crate::vm::TypeCheck>,
    pub type_span: Option<crate::source::Span>,
    pub constant: bool,
    /// A class property may be overridden only by another fixed declaration.
    pub fixed: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum LazyState {
    Unevaluated,
    Evaluating,
    Completed(Value),
}

/// Local definitions use owner identity; ordinary virtual properties use `None`.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct CacheKey {
    pub entry: bool,
    pub owner: Option<ObjectId>,
    pub name: Arc<str>,
    pub method: bool,
}

#[derive(Clone, Debug)]
pub struct Function {
    pub block: usize,
    pub arity: usize,
    pub method: bool,
    /// Custom-scope receiver captured when the literal is evaluated.
    pub captured_this: Option<Value>,
}

#[derive(Clone, Debug)]
pub struct Class {
    pub id: usize,
    pub name: Arc<str>,
    pub open: bool,
    pub abstract_: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CollectionKind {
    #[default]
    Dynamic,
    Listing,
    Mapping,
    List,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CollectionMemberKind {
    Property,
    Entry,
    Element,
}

#[derive(Clone, Debug)]
pub struct CollectionMember {
    pub key: Value,
    pub name: Arc<str>,
    pub kind: CollectionMemberKind,
}

#[derive(Clone, Debug)]
pub struct CollectionIterator {
    pub source: ObjectId,
    pub members: Vec<CollectionMember>,
    pub position: usize,
    pub force_members: Vec<CollectionMember>,
    pub force_position: usize,
    pub awaiting_value: bool,
}

#[derive(Clone, Debug)]
pub struct PredicateIterator {
    pub source: ObjectId,
    pub members: Vec<(ObjectId, CollectionMember)>,
    pub position: usize,
    pub awaiting_value: bool,
    pub previous_this: Option<Value>,
}

#[derive(Clone, Debug)]
pub struct CollectionCast {
    pub ty: crate::vm::TypeCheck,
    pub span: crate::source::Span,
    pub member: Arc<str>,
    pub environment: Environment,
}

#[derive(Clone, Debug)]
pub struct CacheSlot {
    pub key: CacheKey,
    pub value: Value,
}

#[derive(Clone, Debug, Default)]
pub struct Object {
    /// Evaluator-owned canonical identity, populated only after key hashing succeeds.
    pub structural_key: Option<u64>,
    /// Numeric Java hash, independent of canonical equality identity.
    /// Inner None marks a hash depending on JVM identity allocation.
    pub java_hash: Option<Option<i32>>,
    pub function: Option<Function>,
    pub reference: Option<crate::vm::reference::Reference>,
    pub native_function: Option<crate::vm::reference::NativeFunction>,
    pub type_alias: Option<Arc<str>>,
    pub type_alias_initialized: bool,
    pub builtin_type: Option<crate::vm::TypeCheck>,
    pub ref_module: bool,
    pub is_module: bool,
    pub class_definition: Option<Class>,
    /// Published class whose initialization has not completed (including failure).
    /// Such classes accept new property names, but expose only published metadata.
    pub class_initializing: bool,
    /// Includes successful resolution of the implicit Typed superclass, which
    /// has no arena prototype in the bounded runtime.
    pub class_superclass_initialized: bool,
    pub parent: Option<ObjectId>,
    pub members: Vec<Member>,
    pub environment: Environment,
    pub cache: BTreeMap<CacheKey, LazyState>,
    /// Successful cache publication order, matching VmObject.cachedValues.
    /// Evaluating markers do not occupy an insertion position.
    pub completed_cache_order: Vec<CacheKey>,
    pub cache_slots: Vec<CacheSlot>,
    pub cache_index: crate::economic_map::EconomicMapIndex,
    pub collection_kind: CollectionKind,
    pub collection_members: Vec<CollectionMember>,
    pub declaration_index: Option<crate::economic_map::EconomicMapIndex>,
    pub declaration_hashes: Vec<i32>,
    pub length: usize,
    pub mapping_length: Option<usize>,
    pub specialized_literal: bool,
    pub member_environments: BTreeMap<CacheKey, Environment>,
    pub member_values: BTreeMap<CacheKey, Value>,
    pub iterator: Option<CollectionIterator>,
    pub predicate_iterator: Option<PredicateIterator>,
    pub collection_cast: Option<CollectionCast>,
    /// Inherited default value for inferred element/entry objects. `Some(None)`
    /// means the annotated value type has no default; `None` uses Dynamic's default.
    pub collection_default: Option<Option<Value>>,
}

impl Object {
    pub(crate) fn complete_cache(&mut self, key: CacheKey, value: Value) {
        // Completed entries persist for the object's lifetime. Failure cleanup
        // removes only pending entries, so retries acquire their position here.
        if !matches!(self.cache.get(&key), Some(LazyState::Completed(_))) {
            self.completed_cache_order.push(key.clone());
            self.cache_slots.push(CacheSlot {
                key: key.clone(),
                value: value.clone(),
            });
        }
        if let Some(slot) = self.cache_slots.iter_mut().find(|slot| slot.key == key) {
            slot.value = value.clone();
        }
        self.cache.insert(key, LazyState::Completed(value));
    }
}

/// Identity of one logical call frame's auxiliary receiver slot. This is never
/// exposed as a Pkl value; its arena entry participates in ordinary tracing.
#[derive(Clone, Copy, Debug)]
pub struct CustomThisSlot(ObjectId);

impl CustomThisSlot {
    pub(crate) fn root(self) -> Value {
        Value::Object(self.0)
    }
}

enum Entry {
    Object(Box<Object>),
    CustomThis(Option<Value>),
}

struct Slot {
    generation: u64,
    entry: Option<Entry>,
}

impl Slot {
    fn object(&self) -> Option<&Object> {
        match &self.entry {
            Some(Entry::Object(object)) => Some(object),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HeapStats {
    pub live: usize,
    pub slots: usize,
    pub collections: usize,
    pub reclaimed: usize,
}

pub struct Heap {
    identity: u64,
    slots: Vec<Slot>,
    free: Vec<usize>,
    stats: HeapStats,
    pub(crate) structural_keys: BTreeMap<crate::vm::equality::StructuralKey, u64>,
    pub(crate) next_structural_key: u64,
    pub(crate) key_equalities: std::collections::BTreeSet<(ObjectId, ObjectId)>,
}

impl Default for Heap {
    fn default() -> Self {
        Self::new()
    }
}

impl Heap {
    pub fn new() -> Self {
        Self {
            identity: NEXT_HEAP.fetch_add(1, Ordering::Relaxed),
            slots: Vec::new(),
            free: Vec::new(),
            stats: HeapStats::default(),
            structural_keys: BTreeMap::new(),
            next_structural_key: 1,
            key_equalities: std::collections::BTreeSet::new(),
        }
    }

    pub fn allocate(&mut self, object: Object) -> ObjectId {
        self.allocate_entry(Entry::Object(Box::new(object)))
    }

    pub(crate) fn allocate_custom_this(&mut self, value: Option<Value>) -> CustomThisSlot {
        CustomThisSlot(self.allocate_entry(Entry::CustomThis(value)))
    }

    pub(crate) fn custom_this(&self, slot: CustomThisSlot) -> Result<Option<Value>, RuntimeError> {
        match self.entry(slot.0)? {
            Entry::CustomThis(value) => Ok(value.clone()),
            _ => Err(RuntimeError::new(
                ErrorKind::StaleHandle,
                "invalid custom receiver slot",
            )),
        }
    }

    pub(crate) fn set_custom_this(
        &mut self,
        slot: CustomThisSlot,
        value: Option<Value>,
    ) -> Result<(), RuntimeError> {
        self.custom_this(slot)?;
        self.slots[slot.0.index].entry = Some(Entry::CustomThis(value));
        Ok(())
    }

    fn allocate_entry(&mut self, entry: Entry) -> ObjectId {
        let index = if let Some(index) = self.free.pop() {
            self.slots[index].entry = Some(entry);
            index
        } else {
            self.slots.push(Slot {
                generation: 0,
                entry: Some(entry),
            });
            self.slots.len() - 1
        };
        self.stats.live += 1;
        self.stats.slots = self.slots.len();
        ObjectId {
            heap: self.identity,
            index,
            generation: self.slots[index].generation,
        }
    }

    pub(crate) fn find_type_alias(&self, name: &str) -> Option<ObjectId> {
        self.slots.iter().enumerate().find_map(|(index, slot)| {
            slot.object()
                .filter(|o| o.type_alias.as_deref() == Some(name))
                .map(|_| ObjectId {
                    heap: self.identity,
                    index,
                    generation: slot.generation,
                })
        })
    }

    pub(crate) fn find_builtin_class(&self, ty: &crate::vm::TypeCheck) -> Option<ObjectId> {
        self.slots.iter().enumerate().find_map(|(index, slot)| {
            slot.object()
                .filter(|o| o.builtin_type.as_ref().is_some_and(|t| t.equivalent(ty)))
                .map(|_| ObjectId {
                    heap: self.identity,
                    index,
                    generation: slot.generation,
                })
        })
    }

    pub(crate) fn find_class(&self, class_id: usize) -> Option<ObjectId> {
        self.slots.iter().enumerate().find_map(|(index, slot)| {
            slot.object()
                .filter(|object| {
                    object
                        .class_definition
                        .as_ref()
                        .is_some_and(|class| class.id == class_id)
                })
                .map(|_| ObjectId {
                    heap: self.identity,
                    index,
                    generation: slot.generation,
                })
        })
    }

    fn entry(&self, id: ObjectId) -> Result<&Entry, RuntimeError> {
        if id.heap == self.identity
            && let Some(slot) = self.slots.get(id.index)
            && slot.generation == id.generation
            && let Some(entry) = &slot.entry
        {
            return Ok(entry);
        }
        Err(RuntimeError::new(
            ErrorKind::StaleHandle,
            "object does not belong to this live heap",
        ))
    }

    pub fn get(&self, id: ObjectId) -> Result<&Object, RuntimeError> {
        match self.entry(id)? {
            Entry::Object(object) => Ok(object),
            Entry::CustomThis(_) => Err(RuntimeError::new(
                ErrorKind::StaleHandle,
                "internal slot is not a Pkl object",
            )),
        }
    }

    pub fn get_mut(&mut self, id: ObjectId) -> Result<&mut Object, RuntimeError> {
        self.get(id)?;
        match self.slots[id.index].entry.as_mut() {
            Some(Entry::Object(object)) => Ok(object),
            _ => unreachable!("validated object slot"),
        }
    }

    pub fn stats(&self) -> HeapStats {
        self.stats
    }

    /// Trace all object edges, including lazy captures and completed values, iteratively.
    /// The caller must include every externally retained handle in `roots`.
    pub fn collect(&mut self, roots: &[Value]) -> HeapStats {
        let mut marked = vec![false; self.slots.len()];
        let mut pending: Vec<ObjectId> = roots.iter().filter_map(Value::heap_id).collect();
        while let Some(id) = pending.pop() {
            let Ok(entry) = self.entry(id) else { continue };
            if marked[id.index] {
                continue;
            }
            marked[id.index] = true;
            let object = match entry {
                Entry::CustomThis(value) => {
                    pending.extend(value.iter().filter_map(Value::heap_id));
                    continue;
                }
                Entry::Object(object) => object,
            };
            pending.extend(object.parent);
            if let Some(function) = &object.function {
                pending.extend(function.captured_this.iter().filter_map(Value::heap_id));
            }
            if let Some(reference) = &object.reference {
                pending.extend(reference.domain.heap_id());
                pending.extend(reference.data.heap_id());
                pending.extend(reference.path.iter().filter_map(Value::heap_id));
                pending.extend(reference.classes.iter().copied());
            }
            if let Some(native) = &object.native_function {
                pending.extend(native.receiver.heap_id());
            }
            for member in &object.collection_members {
                pending.extend(member.key.heap_id());
            }
            if let Some(iterator) = &object.iterator {
                pending.push(iterator.source);
                for member in iterator.members.iter().chain(&iterator.force_members) {
                    pending.extend(member.key.heap_id());
                }
            }
            if let Some(iterator) = &object.predicate_iterator {
                pending.push(iterator.source);
                pending.extend(iterator.previous_this.iter().filter_map(Value::heap_id));
                for (owner, member) in &iterator.members {
                    pending.push(*owner);
                    pending.extend(member.key.heap_id());
                }
            }
            for environment in object
                .member_environments
                .values()
                .chain(object.collection_cast.iter().map(|cast| &cast.environment))
            {
                pending.extend(environment.custom_this.map(|slot| slot.0));
                for scope in &environment.scopes {
                    pending.push(scope.receiver);
                    pending.push(scope.owner);
                }
                pending.extend(
                    environment
                        .locals
                        .iter()
                        .chain(environment.this_value.iter())
                        .chain(environment.outer_value.iter())
                        .filter_map(Value::heap_id),
                );
            }
            pending.extend(object.environment.custom_this.map(|slot| slot.0));
            for scope in &object.environment.scopes {
                pending.push(scope.receiver);
                pending.push(scope.owner);
            }
            for value in object
                .environment
                .locals
                .iter()
                .chain(object.environment.this_value.iter())
                .chain(object.environment.outer_value.iter())
                .chain(object.collection_default.iter().flatten())
                .chain(object.member_values.values())
                .chain(object.cache_slots.iter().map(|slot| &slot.value))
                .chain(object.cache.values().filter_map(|state| match state {
                    LazyState::Completed(value) => Some(value),
                    _ => None,
                }))
            {
                if let Some(id) = value.heap_id() {
                    pending.push(id);
                }
            }
            for key in object
                .cache
                .keys()
                .chain(object.cache_slots.iter().map(|slot| &slot.key))
            {
                pending.extend(key.owner);
            }
        }
        for (index, slot) in self.slots.iter_mut().enumerate() {
            if !marked[index] && slot.entry.take_if(|_| true).is_some() {
                self.stats.live -= 1;
                self.stats.reclaimed += 1;
                // Exhausted generations are retired rather than wrapped.
                if let Some(generation) = slot.generation.checked_add(1) {
                    slot.generation = generation;
                    self.free.push(index);
                }
            }
        }
        let live: std::collections::BTreeSet<_> = self
            .slots
            .iter()
            .enumerate()
            .filter_map(|(index, slot)| {
                slot.object().map(|_| ObjectId {
                    heap: self.identity,
                    index,
                    generation: slot.generation,
                })
            })
            .collect();
        self.key_equalities
            .retain(|(a, b)| live.contains(a) && live.contains(b));
        // Canonical keys are metadata, not roots. Retain only keys referenced by
        // live objects and their structural components.
        let by_id: BTreeMap<_, _> = self
            .structural_keys
            .iter()
            .map(|(key, id)| (*id, key))
            .collect();
        let mut pending: Vec<_> = self
            .slots
            .iter()
            .filter_map(|slot| slot.object().and_then(|o| o.structural_key))
            .collect();
        let mut retained = std::collections::BTreeSet::new();
        while let Some(id) = pending.pop() {
            if retained.insert(id)
                && let Some(key) = by_id.get(&id)
            {
                pending.extend(key.children());
            }
        }
        self.structural_keys.retain(|_, id| retained.contains(id));
        self.stats.collections += 1;
        self.stats
    }

    pub fn clear(&mut self) {
        self.collect(&[]);
    }

    /// Permanently discard this arena's backing storage; all old IDs become invalid.
    pub fn release(&mut self) {
        self.stats.reclaimed += self.stats.live;
        self.stats.live = 0;
        self.stats.slots = 0;
        self.stats.collections += 1;
        self.slots = Vec::new();
        self.free = Vec::new();
        self.structural_keys = BTreeMap::new();
        self.key_equalities.clear();
        self.identity = NEXT_HEAP.fetch_add(1, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_receiver_cycles_are_traced_and_reclaimed_in_the_object_arena() {
        let mut heap = Heap::new();
        let slot = heap.allocate_custom_this(None);
        let object = heap.allocate(Object {
            environment: Environment {
                custom_this: Some(slot),
                ..Environment::default()
            },
            ..Object::default()
        });
        heap.set_custom_this(slot, Some(Value::Object(object)))
            .unwrap();
        assert_eq!(heap.collect(&[slot.root()]).live, 2);
        assert_eq!(heap.custom_this(slot).unwrap(), Some(Value::Object(object)));
        assert!(heap.get(object).is_ok());
        assert!(
            heap.get(slot.0).is_err(),
            "internal slots are not Pkl objects"
        );
        assert_eq!(heap.collect(&[Value::Object(object)]).live, 2);
        assert_eq!(heap.collect(&[]).live, 0);
        assert!(heap.custom_this(slot).is_err());
        let replacement = heap.allocate_custom_this(Some(Value::Null));
        assert!(
            heap.custom_this(slot).is_err(),
            "reused slots reject old generations"
        );
        assert_eq!(heap.custom_this(replacement).unwrap(), Some(Value::Null));
        assert!(Heap::new().custom_this(replacement).is_err());
        heap.release();
        assert!(heap.custom_this(replacement).is_err());
        assert_eq!(heap.stats().slots, 0);
    }

    #[test]
    fn function_snapshot_survives_an_independent_receiver_slot_mutation() {
        let mut heap = Heap::new();
        let snapshot = heap.allocate(Object::default());
        let slot = heap.allocate_custom_this(Some(Value::Object(snapshot)));
        let function = heap.allocate(Object {
            function: Some(Function {
                block: 0,
                arity: 0,
                method: false,
                captured_this: Some(Value::Object(snapshot)),
            }),
            environment: Environment {
                custom_this: Some(slot),
                ..Environment::default()
            },
            ..Object::default()
        });
        heap.set_custom_this(slot, Some(Value::Function(function)))
            .unwrap();
        assert_eq!(heap.collect(&[Value::Function(function)]).live, 3);
        assert!(heap.get(snapshot).is_ok());
        assert_eq!(
            heap.custom_this(slot).unwrap(),
            Some(Value::Function(function))
        );
        assert_eq!(heap.collect(&[]).live, 0);
    }
}
