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
    /// Persistent cloned executable identity, independent of the receiver slot.
    /// None keeps the canonical code template implicit across every branch.
    pub executable_owner: Option<ExecutableOwnerId>,
    /// Lexical substitutions captured by reusable generic constraint bodies.
    /// Runtime values are immutable shared type DAGs; environment clones copy handles.
    pub type_arguments: BTreeMap<(usize, Arc<str>), crate::vm::TypeCheck>,
    /// Source sections follow actual arguments without participating in type identity.
    pub type_argument_sources: BTreeMap<(usize, Arc<str>), crate::vm::TypeArgumentSource>,
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
    Map,
    Set,
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
    pub source: Option<Arc<crate::value::RuntimeSource>>,
    pub(crate) node: Option<TypeNodeId>,
    pub ty: crate::vm::TypeCheck,
    pub span: crate::source::Span,
    pub member: crate::value::PklString,
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
    /// Declaration-owned annotation values; fields remain ordinary lazy members.
    pub annotations: Vec<Value>,
    /// Published property/method annotation metadata, keyed by declaration code.
    pub member_annotations: BTreeMap<usize, Vec<Value>>,
    pub builtin_type: Option<crate::vm::TypeCheck>,
    pub ref_module: bool,
    pub is_module: bool,
    pub module_initializing: bool,
    pub published_module_members: std::collections::BTreeSet<usize>,
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
    pub map_index: Option<Box<crate::strict_map::MapIndex>>,
    /// Strict Set hash-table keys; element order lives in collection_members.
    pub set_entries: Vec<Value>,
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
    /// Authored declaration of a synthesized undefined collection default.
    pub collection_default_origin: Option<crate::value::RuntimeFrame>,
}

impl Object {
    pub(crate) fn map_index(&self) -> &crate::strict_map::MapIndex {
        self.map_index
            .as_deref()
            .unwrap_or(&crate::strict_map::EMPTY)
    }

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

/// Persistent evaluator-local executable state. Handles are generation checked
/// and traced, but are never exposed as language values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExecutableOwnerId(ObjectId);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TypeNodeId(pub(crate) ExecutableOwnerId);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ExecutionSite {
    TypeUse { block: usize, instruction: usize },
    PropertyType(usize),
    TypeChild(usize),
    Constraint(usize),
    Function { block: usize, instruction: usize },
    Member(usize),
}

#[derive(Default)]
struct ExecutableState {
    children: BTreeMap<ExecutionSite, ExecutableOwnerId>,
    // ObjectMember belongs to the declaration, whereas owner/receiver are read
    // from the current frame on every invocation. Never retain the first receiver.
    local_properties: BTreeMap<(usize, usize), Member>,
}

impl ExecutableOwnerId {
    pub(crate) fn root(self) -> Value {
        Value::Object(self.0)
    }
}

enum Entry {
    Object(Box<Object>),
    NullDefault(Value),
    CustomThis(Option<Value>),
    Executable(Box<ExecutableState>),
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
    /// Persistent executable/type nodes included in `live`, excluding objects and receiver slots.
    pub executable_live: usize,
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

    pub(crate) fn allocate_null(&mut self, default: Option<Value>) -> Value {
        default.map_or(Value::Null, |value| {
            Value::NullWithDefault(self.allocate_entry(Entry::NullDefault(value)))
        })
    }

    pub(crate) fn null_default(&self, id: ObjectId) -> Result<Value, RuntimeError> {
        match self.entry(id)? {
            Entry::NullDefault(value) => Ok(value.clone()),
            _ => Err(RuntimeError::new(
                ErrorKind::StaleHandle,
                "invalid null default slot",
            )),
        }
    }

    pub(crate) fn validate_value(&self, value: &Value) -> Result<(), RuntimeError> {
        if let Value::NullWithDefault(id) = value {
            self.null_default(*id)?;
        } else if let Some(id) = value.heap_id() {
            self.get(id)?;
        }
        Ok(())
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

    pub(crate) fn allocate_executable(&mut self) -> ExecutableOwnerId {
        ExecutableOwnerId(self.allocate_entry(Entry::Executable(Box::default())))
    }

    fn executable(&self, owner: ExecutableOwnerId) -> Result<&ExecutableState, RuntimeError> {
        match self.entry(owner.0)? {
            Entry::Executable(state) => Ok(state),
            _ => Err(RuntimeError::new(
                ErrorKind::StaleHandle,
                "invalid executable owner",
            )),
        }
    }

    fn executable_mut(
        &mut self,
        owner: ExecutableOwnerId,
    ) -> Result<&mut ExecutableState, RuntimeError> {
        self.executable(owner)?;
        match self.slots[owner.0.index].entry.as_mut() {
            Some(Entry::Executable(state)) => Ok(state),
            _ => unreachable!("validated executable owner"),
        }
    }

    pub(crate) fn executable_child(
        &mut self,
        owner: ExecutableOwnerId,
        site: ExecutionSite,
    ) -> Result<ExecutableOwnerId, RuntimeError> {
        if let Some(child) = self.executable(owner)?.children.get(&site) {
            return Ok(*child);
        }
        let child = self.allocate_executable();
        self.executable_mut(owner)?.children.insert(site, child);
        Ok(child)
    }

    pub(crate) fn resolved_local(
        &self,
        owner: ExecutableOwnerId,
        site: (usize, usize),
    ) -> Result<Option<Member>, RuntimeError> {
        Ok(self.executable(owner)?.local_properties.get(&site).cloned())
    }

    pub(crate) fn publish_local(
        &mut self,
        owner: ExecutableOwnerId,
        site: (usize, usize),
        member: Member,
    ) -> Result<(), RuntimeError> {
        self.executable_mut(owner)?
            .local_properties
            .insert(site, member);
        Ok(())
    }

    fn allocate_entry(&mut self, entry: Entry) -> ObjectId {
        self.stats.executable_live += usize::from(matches!(entry, Entry::Executable(_)));
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
            Entry::CustomThis(_) | Entry::Executable(_) | Entry::NullDefault(_) => Err(
                RuntimeError::new(ErrorKind::StaleHandle, "internal slot is not a Pkl object"),
            ),
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
                Entry::NullDefault(value) => {
                    pending.extend(value.heap_id());
                    continue;
                }
                Entry::Executable(state) => {
                    pending.extend(state.children.values().map(|owner| owner.0));
                    continue;
                }
                Entry::CustomThis(value) => {
                    pending.extend(value.iter().filter_map(Value::heap_id));
                    continue;
                }
                Entry::Object(object) => object,
            };
            pending.extend(object.parent);
            pending.extend(object.annotations.iter().filter_map(Value::heap_id));
            pending.extend(
                object
                    .member_annotations
                    .values()
                    .flatten()
                    .filter_map(Value::heap_id),
            );
            pending.extend(
                object
                    .collection_cast
                    .as_ref()
                    .and_then(|cast| cast.node)
                    .map(|node| (node.0).0),
            );
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
                pending.extend(environment.executable_owner.map(|owner| owner.0));
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
            pending.extend(object.environment.executable_owner.map(|owner| owner.0));
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
                .chain(object.set_entries.iter())
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
            if !marked[index]
                && let Some(entry) = slot.entry.take()
            {
                self.stats.executable_live -= usize::from(matches!(entry, Entry::Executable(_)));
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
        self.stats.executable_live = 0;
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
    fn nullable_defaults_are_null_values_with_checked_private_heap_edges() {
        let mut heap = Heap::new();
        let value = heap.allocate_null(Some(Value::Int(7)));
        assert_eq!(value, Value::Null);
        let id = value.heap_id().expect("default payload is traced");
        assert_eq!(heap.null_default(id), Ok(Value::Int(7)));
        assert_eq!(heap.get(id).unwrap_err().kind, ErrorKind::StaleHandle);
        assert_eq!(
            Heap::new().null_default(id).unwrap_err().kind,
            ErrorKind::StaleHandle
        );
        assert_eq!(heap.collect(std::slice::from_ref(&value)).live, 1);
        assert_eq!(heap.collect(&[]).live, 0);
        let replacement = heap.allocate_null(Some(Value::Int(8)));
        assert_ne!(replacement.heap_id(), Some(id));
        assert_eq!(
            heap.null_default(id).unwrap_err().kind,
            ErrorKind::StaleHandle
        );
        assert_eq!(heap.allocate_null(None), Value::Null);
    }

    #[test]
    fn deep_nullable_defaults_trace_and_release_iteratively_on_small_stack() {
        std::thread::Builder::new()
            .stack_size(256 * 1024)
            .spawn(|| {
                let mut heap = Heap::new();
                let mut value = Value::Object(heap.allocate(Object::default()));
                for _ in 0..20_000 {
                    value = heap.allocate_null(Some(value));
                }
                assert_eq!(heap.collect(std::slice::from_ref(&value)).live, 20_001);
                assert_eq!(heap.collect(&[]).live, 0);
                value = Value::Object(heap.allocate(Object::default()));
                for _ in 0..20_000 {
                    value = heap.allocate_null(Some(value));
                }
                // Arena destruction releases edges as handles, not Rust recursion.
                drop(heap);
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn captured_executable_graphs_trace_children_without_retaining_receivers() {
        let mut heap = Heap::new();
        let owner = heap.allocate_executable();
        let child = heap
            .executable_child(owner, ExecutionSite::Member(7))
            .unwrap();
        let capture = heap.allocate(Object {
            environment: Environment {
                executable_owner: Some(owner),
                ..Environment::default()
            },
            ..Object::default()
        });
        let retained = heap.collect(&[Value::Object(capture)]);
        assert_eq!(retained.live, 3);
        assert_eq!(retained.executable_live, 2);
        assert_eq!(
            heap.executable_child(owner, ExecutionSite::Member(7))
                .unwrap(),
            child
        );
        assert!(heap.get(owner.0).is_err());
        // A static owner may persist, but must not keep a prior lexical receiver.
        let retained = heap.collect(&[owner.root()]);
        assert_eq!(retained.live, 2);
        assert_eq!(retained.executable_live, 2);
        assert!(heap.get(capture).is_err());
        assert_eq!(heap.collect(&[]).live, 0);
        assert_eq!(heap.stats().executable_live, 0);
        let replacement = heap.allocate_executable();
        assert!(
            heap.executable_child(owner, ExecutionSite::Member(7))
                .is_err()
        );
        assert!(
            Heap::new()
                .executable_child(replacement, ExecutionSite::Member(7))
                .is_err()
        );
        heap.release();
        assert!(
            heap.executable_child(replacement, ExecutionSite::Member(7))
                .is_err()
        );
    }

    #[test]
    fn lazy_cast_retains_its_type_node_without_a_live_execution_frame() {
        let mut heap = Heap::new();
        let owner = heap.allocate_executable();
        let node = TypeNodeId(
            heap.executable_child(owner, ExecutionSite::TypeChild(0))
                .unwrap(),
        );
        let wrapper = heap.allocate(Object {
            collection_cast: Some(CollectionCast {
                source: None,
                node: Some(node),
                ty: crate::vm::TypeCheck::Int,
                span: crate::source::Span::new(0, 1),
                member: "#values".into(),
                environment: Environment::default(),
            }),
            ..Object::default()
        });
        let retained = heap.collect(&[Value::Object(wrapper)]);
        assert_eq!(retained.live, 2);
        assert_eq!(retained.executable_live, 1);
        assert!(heap.executable(owner).is_err());
        assert!(heap.executable(node.0).is_ok());
        assert_eq!(heap.collect(&[]).live, 0);
        assert!(heap.executable(node.0).is_err());
    }

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
