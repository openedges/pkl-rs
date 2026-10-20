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
    String(Arc<str>),
    Bytes(Arc<[u8]>),
    Structural(u64),
    Function(ObjectId),
    Class(ObjectId),
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
        Value::Null => Atom::Null,
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
        Value::Object(v) => Atom::Structural(
            heap.get(*v)?
                .structural_key
                .ok_or_else(|| invalid("unprepared structural key"))?,
        ),
    })
}
pub(super) fn same_key(heap: &Heap, left: &Value, right: &Value) -> bool {
    if let (Value::Object(a), Value::Object(b)) = (left, right) {
        return a == b
            || heap.key_equalities.contains(&(*a, *b))
            || heap
                .get(*a)
                .ok()
                .and_then(|o| o.structural_key)
                .is_some_and(|key| heap.get(*b).is_ok_and(|o| o.structural_key == Some(key)));
    }
    matches!((atom(heap, left), atom(heap, right)), (Ok(a), Ok(b)) if a == b)
}
#[derive(Clone)]
enum Task {
    Compare(Value, Value, bool),
    CompareMembers(ObjectId, ObjectId),
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
    key_search: bool,
    pub(super) hide_callsite: bool,
    forward: bool,
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
    }
}
impl Vm {
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
            key_search: false,
            hide_callsite: true,
            forward: false,
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
            key_search: false,
            hide_callsite: true,
            forward: true,
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
                self.heap.key_equalities.insert((*b, *a));
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
        let mut pending = Vec::new();
        for key in keys {
            if matches!(&key, Value::Object(id) if self.heap.get(*id)?.structural_key.is_none()) {
                pending.push(key);
            }
        }
        if pending.is_empty() {
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
            key_search: false,
            hide_callsite: true,
            forward: false,
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
        match self.heap.get(id)?.cache.get(&key) {
            Some(LazyState::Completed(value)) => Ok(value.clone()),
            _ => Err(invalid("unforced equality member")),
        }
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
    pub(super) fn step_equality(&mut self) -> Result<(), RuntimeError> {
        let frame_index = self.frames.len() - 1;
        let mut run = self.frames[frame_index]
            .equality_run
            .take()
            .expect("equality run");
        let Some(task) = run.tasks.pop() else {
            self.frames[frame_index].callsite_hidden = false;
            if run.key_search {
                self.frames[frame_index]
                    .key_search
                    .as_mut()
                    .expect("key search")
                    .returned = Some(run.equal);
                self.frames[frame_index].pc -= 1;
            } else if run.retry {
                self.frames[frame_index].pc -= 1;
            } else if !run.forward {
                self.stack.push(Value::Bool(run.equal ^ run.negate));
            }
            return Ok(());
        };
        match task {
            Task::Compare(left, right, semantic) => {
                let equal = match (&left, &right) {
                    (Value::Int(a), Value::Float(b)) if semantic => Some(*a as f64 == *b),
                    (Value::Float(a), Value::Int(b)) if semantic => Some(*a == *b as f64),
                    (Value::Float(a), Value::Float(b)) if semantic => Some(a == b),
                    (Value::Object(a), Value::Object(b)) if a == b => Some(true),
                    (Value::Object(a), Value::Object(b)) => {
                        let x = self.heap.get(*a)?;
                        let y = self.heap.get(*b)?;
                        if x.collection_kind != y.collection_kind
                            || self.class_identity(*a)? != self.class_identity(*b)?
                            || x.is_module != y.is_module
                            || (matches!(
                                x.collection_kind,
                                CollectionKind::Listing | CollectionKind::List
                            ) && x.length != y.length)
                        {
                            Some(false)
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
                let rhs = self.comparison_members(b)?;
                if lhs.len() != rhs.len() {
                    run.reject();
                } else {
                    for l in lhs.iter().rev() {
                        let left = self.cached_equality_member(a, l)?;
                        let mut candidates = Vec::new();
                        for r in &rhs {
                            if (l.kind == CollectionMemberKind::Property)
                                == (r.kind == CollectionMemberKind::Property)
                            {
                                let value = self.cached_equality_member(b, r)?;
                                run.roots.extend([r.key.clone(), value.clone()]);
                                candidates.push((r.key.clone(), value));
                            }
                        }
                        run.roots.extend([l.key.clone(), left.clone()]);
                        run.tasks
                            .push(Task::Match(l.key.clone(), left, candidates, 0));
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
                    self.heap.key_equalities.insert((b, a));
                }
                run.tasks.push(Task::Compare(value, other_value, false));
            }
            Task::Deep(value) => {
                if let Value::Object(id) = value
                    && run.forced.insert(id)
                {
                    for member in self.equality_members(id)?.into_iter().rev() {
                        run.tasks.push(Task::Force(id, member, false));
                    }
                }
            }
            Task::Prepare(value) => {
                if let Value::Object(id) = value
                    && self.heap.get(id)?.structural_key.is_none()
                {
                    if !run.active.insert(id) {
                        return Err(RuntimeError::new(ErrorKind::Cycle, "cyclic structural key"));
                    }
                    run.tasks.push(Task::Build(id));
                    run.tasks.push(Task::HashMembers(id));
                    if self.heap.get(id)?.collection_kind != CollectionKind::List {
                        run.tasks.push(Task::Deep(Value::Object(id)));
                    }
                }
            }
            Task::HashMembers(id) => {
                // Java completes recursive value forcing before computing member
                // hashes. Nested Mapping keys can still be lazy at that point.
                // Hashing then follows cache order (or class property order).
                let members = if self.heap.get(id)?.collection_kind == CollectionKind::List {
                    collections::ordered_members(&self.heap, id)?
                } else {
                    self.comparison_members(id)?
                };
                for member in members.into_iter().rev() {
                    let value = self.cached_equality_member(id, &member)?;
                    run.roots.extend([member.key.clone(), value.clone()]);
                    run.tasks.push(Task::Prepare(value));
                    run.tasks.push(Task::Prepare(member.key));
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
                    self.heap.key_equalities.insert((*b, a));
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
                let kind = match object.collection_kind {
                    CollectionKind::Dynamic => 0,
                    CollectionKind::Listing => 1,
                    CollectionKind::Mapping => 2,
                    CollectionKind::List => 3,
                };
                let mut members = Vec::new();
                for member in self.equality_members(id)? {
                    let value = self.cached_equality_member(id, &member)?;
                    members.push((
                        member.kind == CollectionMemberKind::Property,
                        atom(&self.heap, &member.key)?,
                        atom(&self.heap, &value)?,
                    ));
                }
                members.sort();
                let key = StructuralKey {
                    kind,
                    class: self.class_identity(id)?,
                    members,
                };
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
