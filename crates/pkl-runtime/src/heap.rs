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
    pub custom_this: Option<Value>,
    /// Innermost first; immutable captured lexical object scopes.
    pub scopes: Vec<Scope>,
}

#[derive(Clone, Debug)]
pub struct Member {
    pub name: Arc<str>,
    pub code: usize,
    pub local: bool,
    pub method: bool,
    pub ty: Option<crate::vm::TypeCheck>,
    pub type_span: Option<crate::source::Span>,
    pub constant: bool,
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
    pub owner: Option<ObjectId>,
    pub name: Arc<str>,
    pub method: bool,
}

#[derive(Clone, Debug)]
pub struct Function {
    pub block: usize,
    pub arity: usize,
    pub method: bool,
    pub capture_custom_this: bool,
}

#[derive(Clone, Debug)]
pub struct Class {
    pub id: usize,
    pub name: Arc<str>,
    pub open: bool,
    pub abstract_: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Object {
    pub function: Option<Function>,
    pub class_definition: Option<Class>,
    pub parent: Option<ObjectId>,
    pub members: Vec<Member>,
    pub environment: Environment,
    pub cache: BTreeMap<CacheKey, LazyState>,
}

struct Slot {
    generation: u64,
    object: Option<Object>,
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
        }
    }

    pub fn allocate(&mut self, object: Object) -> ObjectId {
        let index = if let Some(index) = self.free.pop() {
            self.slots[index].object = Some(object);
            index
        } else {
            self.slots.push(Slot {
                generation: 0,
                object: Some(object),
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

    pub fn get(&self, id: ObjectId) -> Result<&Object, RuntimeError> {
        if id.heap == self.identity
            && let Some(slot) = self.slots.get(id.index)
            && slot.generation == id.generation
            && let Some(object) = &slot.object
        {
            return Ok(object);
        }
        Err(RuntimeError::new(
            ErrorKind::StaleHandle,
            "object does not belong to this live heap",
        ))
    }

    pub fn get_mut(&mut self, id: ObjectId) -> Result<&mut Object, RuntimeError> {
        self.get(id)?;
        Ok(self.slots[id.index]
            .object
            .as_mut()
            .expect("validated live slot"))
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
            let Ok(object) = self.get(id) else { continue };
            if marked[id.index] {
                continue;
            }
            marked[id.index] = true;
            pending.extend(object.parent);
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
                .chain(object.environment.custom_this.iter())
                .chain(object.cache.values().filter_map(|state| match state {
                    LazyState::Completed(value) => Some(value),
                    _ => None,
                }))
            {
                if let Some(id) = value.heap_id() {
                    pending.push(id);
                }
            }
            for key in object.cache.keys() {
                pending.extend(key.owner);
            }
        }
        for (index, slot) in self.slots.iter_mut().enumerate() {
            if !marked[index] && slot.object.take_if(|_| true).is_some() {
                self.stats.live -= 1;
                self.stats.reclaimed += 1;
                // Exhausted generations are retired rather than wrapped.
                if let Some(generation) = slot.generation.checked_add(1) {
                    slot.generation = generation;
                    self.free.push(index);
                }
            }
        }
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
        self.identity = NEXT_HEAP.fetch_add(1, Ordering::Relaxed);
    }
}
