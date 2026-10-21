//! Ordered lazy collections and bytecode construction/iteration continuations.
use super::*;
use crate::heap::{
    CollectionIterator, CollectionKind, CollectionMember, CollectionMemberKind, PredicateIterator,
};

pub(super) fn typed_class(heap: &Heap, mut id: ObjectId) -> Result<Option<Class>, RuntimeError> {
    let mut visited = BTreeSet::new();
    loop {
        if !visited.insert(id) {
            return Err(RuntimeError::new(ErrorKind::Cycle, "cyclic parent chain"));
        }
        let object = heap.get(id)?;
        if let Some(class) = &object.class_definition {
            return Ok(Some(class.clone()));
        }
        let Some(parent) = object.parent else {
            return Ok(None);
        };
        id = parent;
    }
}

use super::equality::same_key;

fn same_member(heap: &Heap, left: &CollectionMember, right: &CollectionMember) -> bool {
    (left.kind == CollectionMemberKind::Property) == (right.kind == CollectionMemberKind::Property)
        && same_key(heap, &left.key, &right.key)
}

/// Read declaration order without forcing member values. Overrides retain their
/// inherited position; properties and entries have distinct key namespaces.
pub fn ordered_members(
    heap: &Heap,
    receiver: ObjectId,
) -> Result<Vec<CollectionMember>, RuntimeError> {
    let typed = typed_class(heap, receiver)?.is_some() || heap.get(receiver)?.is_module;
    let mut chain = Vec::new();
    let mut next = Some(receiver);
    let mut visited = BTreeSet::new();
    while let Some(id) = next {
        if !visited.insert(id) {
            return Err(RuntimeError::new(ErrorKind::Cycle, "cyclic parent chain"));
        }
        chain.push(id);
        next = heap.get(id)?.parent;
    }
    let mut result: Vec<CollectionMember> = Vec::new();
    for id in chain.into_iter().rev() {
        let object = heap.get(id)?;
        let fallback;
        let members = if object.collection_members.is_empty() {
            fallback = object
                .members
                .iter()
                .filter(|m| !m.local && !m.method && !m.entry)
                .map(|m| CollectionMember {
                    key: Value::String(m.name.clone()),
                    name: m.name.clone(),
                    kind: CollectionMemberKind::Property,
                })
                .collect::<Vec<_>>();
            &fallback
        } else {
            &object.collection_members
        };
        for member in members {
            if !typed
                && member.kind == CollectionMemberKind::Property
                && member.name.as_ref() == "default"
            {
                continue;
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

pub(super) fn forcing_members(
    heap: &Heap,
    receiver: ObjectId,
) -> Result<Vec<CollectionMember>, RuntimeError> {
    let effective = ordered_members(heap, receiver)?;
    let mut result = Vec::new();
    let mut next = Some(receiver);
    let mut visited = BTreeSet::new();
    while let Some(id) = next {
        if !visited.insert(id) {
            return Err(RuntimeError::new(ErrorKind::Cycle, "cyclic parent chain"));
        }
        let object = heap.get(id)?;
        for member in &object.members {
            if member.local || member.method {
                continue;
            }
            if let Some(descriptor) = effective.iter().find(|d| {
                d.name == member.name && (d.kind != CollectionMemberKind::Property) == member.entry
            }) && !result.iter().any(|old| same_member(heap, descriptor, old))
            {
                result.push(descriptor.clone());
            }
        }
        next = object.parent;
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
    let mut visited = BTreeSet::new();
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

pub(super) fn external_amendment_error(name: &str) -> RuntimeError {
    let mut error = RuntimeError::new(
        ErrorKind::Type,
        format!("Cannot instantiate, or amend an instance of, external class `{name}`."),
    );
    error.hint = Some("To create an instance of an external class, use its literal syntax or constructor method.\nExamples:\n* `123` uses literal syntax to create an instance of class `Int`.\n* `Pair(1, 2)` uses the `Pair()` constructor method to create an instance of class `Pair`.".into());
    error
}

impl Vm {
    fn builder(&self) -> Result<ObjectId, RuntimeError> {
        self.frames
            .last()
            .and_then(|f| f.builders.last())
            .copied()
            .ok_or_else(|| invalid("missing collection builder"))
    }

    pub(super) fn collection_builtin(
        &mut self,
        receiver: ObjectId,
        name: &str,
    ) -> Result<bool, RuntimeError> {
        let object = self.heap.get(receiver)?;
        if object.collection_kind == CollectionKind::Dynamic
            || object.is_module
            || !matches!(name, "length" | "isEmpty")
        {
            return Ok(false);
        }
        if object.collection_kind == CollectionKind::Mapping
            && object.mapping_length.is_none()
            && matches!(name, "length" | "isEmpty")
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
                    Some(Instruction::GetMember(_) | Instruction::GetConstMember(_))
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
            "length" => Value::Int(length as i64),
            "isEmpty" => Value::Bool(length == 0),
            _ => return Ok(false),
        };
        self.stack.push(value);
        Ok(true)
    }

    /// Stable-hash declaration maps use the pinned bucket chain order. Maps
    /// containing JVM identity-dependent keys retain the explicit linear fallback.
    fn declaration_candidates(
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
        let keys = object
            .collection_members
            .iter()
            .map(|member| {
                if member.kind == CollectionMemberKind::Property {
                    Ok(None)
                } else {
                    self.java_hash(&member.key)
                }
            })
            .collect::<Result<Option<Vec<_>>, _>>()?;
        let keys = keys.filter(|_| object.collection_members.len() == object.members.len());
        let Some(keys) = keys else {
            let object = self.heap.get_mut(id)?;
            object.declaration_index = None;
            object.declaration_hashes.clear();
            return Ok(());
        };
        let object = self.heap.get_mut(id)?;
        let index = object
            .declaration_index
            .get_or_insert_with(|| crate::economic_map::EconomicMapIndex::with_capacity(0));
        while index.len() < keys.len() {
            if index.grow_before_append() {
                for old in index.start_rehash() {
                    object.declaration_hashes[old] = keys[old];
                    index.put_hash(old, keys[old], false);
                }
            }
            let slot = index.push();
            object.declaration_hashes.push(keys[slot]);
            if (!index.has_hash() && index.len() > 4)
                || (index.has_hash() && index.put_hash(slot, keys[slot], index.should_rehash()))
            {
                for old in index.start_rehash() {
                    object.declaration_hashes[old] = keys[old];
                    index.put_hash(old, keys[old], false);
                }
            }
        }
        Ok(())
    }

    fn prepare_insertion(
        &mut self,
        key: &Value,
        kind: CollectionMemberKind,
    ) -> Result<bool, RuntimeError> {
        let object = self.heap.get(self.builder()?)?;
        if object.members.len() < 4
            || object.collection_members.iter().any(|member| {
                (member.kind == CollectionMemberKind::Property)
                    == (kind == CollectionMemberKind::Property)
                    && same_key(&self.heap, key, &member.key)
            })
        {
            return Ok(false);
        }
        let mut keys: Vec<_> = object
            .collection_members
            .iter()
            .map(|member| member.key.clone())
            .collect();
        keys.push(key.clone());
        self.prepare_keys(keys)
    }

    fn add_collection_member(
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
        let key = key.unwrap_or(Value::Int(length as i64));
        if kind != CollectionMemberKind::Property
            && let Some(class) = typed_class(&self.heap, target)?
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
                format!("{}#{}", self.program.module_name, class.name)
            };
            let mut error = RuntimeError::new(
                ErrorKind::Type,
                format!("Object of type `{type_label}` cannot have an {label}."),
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
            && member.name.as_ref() != "default"
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
                    "Listing keys must have type `Int`.",
                ));
            };
            let parent_length = parent
                .map(|id| self.heap.get(id).map(|o| o.length))
                .transpose()?
                .unwrap_or(0);
            if index < 0 || index as usize >= parent_length {
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
                format!("Duplicate definition of member `{}`.", member.name),
            ));
        }
        let mut descriptor = CollectionMember {
            key,
            name: member.name.clone(),
            kind,
        };
        let candidates = self.declaration_candidates(target, &descriptor.key, false)?;
        let replacing = object.collection_members.iter().position(|old| {
            (descriptor.kind == CollectionMemberKind::Property
                || candidates
                    .iter()
                    .any(|candidate| equality::identical_key(&self.heap, candidate, &old.key)))
                && same_member(&self.heap, &descriptor, old)
        });
        if replacing.is_some_and(|index| {
            !(object.specialized_literal
                && kind == CollectionMemberKind::Element
                && object.collection_members[index].kind == CollectionMemberKind::Entry)
        }) {
            return Err(RuntimeError::new(
                ErrorKind::Type,
                format!(
                    "Duplicate definition of member `{}`.",
                    crate::diagnostic_values::render(
                        &descriptor.key,
                        &self.heap,
                        &self.program.module_name,
                        80
                    )?
                ),
            ));
        }
        if kind != CollectionMemberKind::Property {
            descriptor.name = replacing
                .map(|index| object.collection_members[index].name.clone())
                .or_else(|| {
                    parent
                        .and_then(|id| ordered_members(&self.heap, id).ok())
                        .and_then(|members| {
                            members
                                .into_iter()
                                .find(|old| same_member(&self.heap, &descriptor, old))
                        })
                        .map(|old| old.name)
                })
                .unwrap_or_else(|| {
                    Arc::from(format!("\0{}:{}", target.index, object.members.len()))
                });
            member.name = descriptor.name.clone();
            member.entry = true;
        }
        if let Some(parent) = parent {
            self.validate_object_members(parent, std::slice::from_ref(&member))?;
        }
        let environment = self
            .frames
            .last()
            .expect("active frame")
            .environment
            .clone();
        let cache_key = CacheKey {
            owner: None,
            name: member.name.clone(),
            method: member.method,
            entry: member.entry,
        };
        let object = self.heap.get_mut(target)?;
        object
            .member_environments
            .insert(cache_key.clone(), environment);
        if !member.local && !member.method {
            if let Some(index) = replacing {
                object.collection_members[index] = descriptor;
            } else {
                object.collection_members.push(descriptor);
            }
        }
        if let Some(index) = object.members.iter().position(|old| {
            old.name == member.name
                && old.entry == member.entry
                && old.local == member.local
                && old.method == member.method
        }) {
            object.members[index] = member;
        } else {
            object.members.push(member);
        }
        if kind == CollectionMemberKind::Element {
            object.length += 1;
        }
        if let Some(value) = value {
            object
                .member_values
                .insert(cache_key.clone(), value.clone());
            self.complete_cache(target, cache_key, value)?;
        }
        self.update_declaration_index(target)?;
        Ok(())
    }

    fn source_member(
        &self,
        mut source: ObjectId,
        descriptor: &CollectionMember,
    ) -> Result<Member, RuntimeError> {
        loop {
            let object = self.heap.get(source)?;
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

    fn spread_error(&mut self, message: String, member: &Member) -> RuntimeError {
        let mut error = RuntimeError::new(ErrorKind::Type, message);
        let frame = self.frames.last_mut().expect("active frame");
        if let Some(metadata) = self.program.metadata.get(frame.block)
            && let Some(Some(span)) = metadata.spans.get(frame.pc - 1)
        {
            error.frames.push(RuntimeFrame {
                source: None,
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
                source: None,
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
                        Value::Null => None,
                        value @ (Value::Bool(_)
                        | Value::Int(_)
                        | Value::Float(_)
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
                    if object.collection_kind == CollectionKind::List {
                        return Err(external_amendment_error("List"));
                    }
                    (object.collection_kind, object.length)
                } else {
                    (kind, 0)
                };
                let environment = self
                    .frames
                    .last()
                    .expect("active frame")
                    .environment
                    .clone();
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
            Instruction::SetCollectionDefault { defined } => {
                let value = if defined { Some(self.pop()?) } else { None };
                let target = self.builder()?;
                self.heap.get_mut(target)?.collection_default = Some(value);
            }
            Instruction::SetCollectionTypeDefault(ty) => {
                let environment = self
                    .frames
                    .last()
                    .expect("active frame")
                    .environment
                    .clone();
                let ty = ty.substituted(&environment)?;
                let target = self.builder()?;
                if ty.noop() {
                    self.heap.get_mut(target)?.collection_default = None;
                } else {
                    self.frames
                        .last_mut()
                        .expect("active frame")
                        .generic_default = Some(generics::GenericDefaultRun::new(target, ty));
                }
            }

            Instruction::EndCollection => self.end_collection()?,
            Instruction::AddCollectionProperty(member) => {
                let key = Value::String(member.name.clone());
                if self.prepare_insertion(&key, CollectionMemberKind::Property)? {
                    return Ok(());
                }
                self.add_collection_member(
                    member,
                    Some(key),
                    CollectionMemberKind::Property,
                    None,
                )?;
            }
            Instruction::AddCollectionElement(member) => {
                let object = self.heap.get(self.builder()?)?;
                if object.collection_kind != CollectionKind::Mapping
                    && typed_class(&self.heap, self.builder()?)?.is_none()
                {
                    let key = Value::Int(object.length as i64);
                    if self.prepare_insertion(&key, CollectionMemberKind::Element)? {
                        return Ok(());
                    }
                }
                self.add_collection_member(member, None, CollectionMemberKind::Element, None)?
            }
            Instruction::AddCollectionEntry {
                member,
                check_index,
                span,
            } => {
                let key = self.pop()?;
                let target = self.builder()?;
                if typed_class(&self.heap, target)?.is_none() && !check_index {
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
                    if index < 0 || index as usize >= length {
                        return Err(RuntimeError::new(
                            ErrorKind::Type,
                            format!(
                                "Element index `{index}` is out of range `0`..`{}`.",
                                length as i64 - 1
                            ),
                        ));
                    }
                }
                let target = self.builder()?;
                let generated_typed = !self.heap.get(target)?.specialized_literal
                    && typed_class(&self.heap, target)?.is_some();
                self.add_collection_member(member, Some(key), CollectionMemberKind::Entry, None)
                    .map_err(|error| {
                        if generated_typed {
                            error.at(span)
                        } else {
                            error
                        }
                    })?;
            }
            Instruction::Subscript | Instruction::SuperSubscript => {
                let key = self.pop()?;
                let is_super = matches!(instruction, Instruction::SuperSubscript);
                let receiver = if is_super {
                    self.scope(0)?.receiver
                } else {
                    self.pop()?.as_object()?
                };
                if !is_super && self.heap.get(receiver)?.reference.is_some() {
                    let result = self.reference_access(receiver, None, Some(key))?;
                    self.stack.push(result);
                    return Ok(());
                }
                let owner = if is_super {
                    self.heap.get(self.scope(0)?.owner)?.parent.ok_or_else(|| {
                        RuntimeError::new(ErrorKind::MissingMember, "super has no parent")
                    })?
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
                if matches!(
                    object.collection_kind,
                    CollectionKind::Listing | CollectionKind::List
                ) {
                    let Value::Int(index) = key else {
                        let name = format!("{:?}", object.collection_kind);
                        return Err(RuntimeError::new(
                            ErrorKind::Type,
                            format!(
                                "Operator `[]` is not defined for operand types `{name}` and `{}`.\nLeft operand : {}\nRight operand: {}",
                                value_type_name(&key),
                                crate::diagnostic_values::render(
                                    &Value::Object(receiver),
                                    &self.heap,
                                    &self.program.module_name,
                                    80
                                )?,
                                crate::diagnostic_values::render(
                                    &key,
                                    &self.heap,
                                    &self.program.module_name,
                                    80
                                )?
                            ),
                        ));
                    };
                    if index < 0 || index as usize >= object.length {
                        return Err(RuntimeError::new(
                            ErrorKind::Type,
                            format!(
                                "Element index `{index}` is out of range `0`..`{}`.",
                                object.length as i64 - 1
                            ),
                        ));
                    }
                }
                let candidates = self.declaration_candidates(owner, &key, true)?;
                let member = ordered_members(&self.heap, owner)?
                    .into_iter()
                    .rev()
                    .find(|m| {
                        m.kind != CollectionMemberKind::Property
                            && candidates.iter().any(|candidate| {
                                equality::identical_key(&self.heap, candidate, &m.key)
                            })
                            && same_key(&self.heap, &key, &m.key)
                    });
                if member.is_none() && self.prepare_keys(vec![key.clone()])? {
                    if !is_super {
                        self.stack.push(Value::Object(receiver));
                    }
                    self.stack.push(key);
                    return Ok(());
                }
                let member = member.ok_or_else(|| {
                    RuntimeError::new(
                        ErrorKind::MissingMember,
                        format!(
                            "Cannot find key `{}`.",
                            crate::diagnostic_values::render(
                                &key,
                                &self.heap,
                                &self.program.module_name,
                                80
                            )
                            .unwrap_or_else(|_| display_value(&key))
                        ),
                    )
                })?;
                self.force_key(receiver, owner, member.name, false, !is_super, false, true)?;
            }
            Instruction::EntryParent => {
                let force = self
                    .frames
                    .last()
                    .and_then(|f| f.force.as_ref())
                    .cloned()
                    .ok_or_else(|| invalid("entry parent outside member"))?;
                let parent = self.heap.get(force.owner)?.parent;
                let key = self
                    .heap
                    .get(force.owner)?
                    .collection_members
                    .iter()
                    .find(|m| m.name == force.name)
                    .map(|m| m.key.clone());
                if let (Some(parent), Some(key)) = (parent, &key) {
                    let candidates = forcing_members(&self.heap, parent)?
                        .into_iter()
                        .filter(|m| m.kind != CollectionMemberKind::Property)
                        .map(|m| m.key)
                        .collect();
                    if self.search_key(key, candidates)? {
                        return Ok(());
                    }
                }
                let inherited = parent
                    .map(|parent| ordered_members(&self.heap, parent))
                    .transpose()?
                    .unwrap_or_default()
                    .into_iter()
                    .find(|member| {
                        member.kind != CollectionMemberKind::Property
                            && (member.name == force.name
                                || key
                                    .as_ref()
                                    .is_some_and(|key| same_key(&self.heap, key, &member.key)))
                    });
                if let (Some(parent), Some(member)) = (parent, inherited) {
                    self.force_key(
                        force.receiver,
                        parent,
                        member.name,
                        false,
                        false,
                        false,
                        true,
                    )?;
                } else {
                    let mut owner = Some(force.owner);
                    while let Some(id) = owner {
                        let object = self.heap.get(id)?;
                        if let Some(value) = &object.collection_default {
                            if let Some(value) = value {
                                self.stack.push(value.clone());
                                return Ok(());
                            }
                            return Err(RuntimeError::new(
                                ErrorKind::MissingMember,
                                "Tried to read property `default` but its value is undefined.",
                            ));
                        }
                        if object.members.iter().any(|m| {
                            !m.entry && !m.method && !m.local && m.name.as_ref() == "default"
                        }) {
                            return Err(unsupported_type("custom collection default functions"));
                        }
                        owner = object.parent;
                    }
                    let object = self.heap.allocate(Object::default());
                    self.stack.push(Value::Object(object));
                }
            }
            Instruction::OpenIterator { nullable, spread } => {
                let value = self.pop()?;
                let iterator = if nullable && value == Value::Null {
                    let source = self.heap.allocate(Object::default());
                    Some(CollectionIterator {
                        source,
                        members: Vec::new(),
                        position: 0,
                        force_members: Vec::new(),
                        force_position: 0,
                        awaiting_value: false,
                    })
                } else {
                    let source = value.as_object().map_err(|_| {
                        let mut error = RuntimeError::new(
                            ErrorKind::Type,
                            format!(
                                "Cannot iterate over value of type `{}`.",
                                value_type_name(&value)
                            ),
                        );
                        if spread && value == Value::Null {
                            let frame = self.frames.last().expect("active frame");
                            error.spread_hint = self
                                .program
                                .metadata
                                .get(frame.block)
                                .and_then(|m| m.spans.get(frame.pc - 1))
                                .copied()
                                .flatten()
                                .map(Box::new);
                        }
                        error
                    })?;
                    if self.heap.get(source)?.reference.is_some() {
                        return Err(RuntimeError::new(
                            ErrorKind::Type,
                            format!(
                                "Cannot iterate over value of type `pkl.ref#Reference`.\nValue: {}",
                                crate::diagnostic_values::render(
                                    &value,
                                    &self.heap,
                                    &self.program.module_name,
                                    80
                                )?
                            ),
                        ));
                    }
                    if self.heap.get(source)?.is_module {
                        let mut error = RuntimeError::new(
                            ErrorKind::Type,
                            format!(
                                "Cannot iterate over value of type `{}`.\nValue: {}",
                                self.program.module_name,
                                crate::diagnostic_values::render(
                                    &value,
                                    &self.heap,
                                    &self.program.module_name,
                                    80
                                )?
                            ),
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
                        let mut error = RuntimeError::new(
                            ErrorKind::Type,
                            format!(
                                "Cannot iterate over value of type `{}#{}`.\nValue: {}",
                                self.program.module_name,
                                class.name,
                                crate::diagnostic_values::render(
                                    &value,
                                    &self.heap,
                                    &self.program.module_name,
                                    80
                                )?
                            ),
                        );
                        if spread {
                            error.hint = Some("`Typed` values are not iterable. If you mean to spread its members, convert it to `Dynamic` using `toDynamic()`.".into());
                        }
                        return Err(error);
                    }
                    let members = ordered_members(&self.heap, source)?;
                    Some(CollectionIterator {
                        source,
                        force_members: forcing_members(&self.heap, source)?,
                        members,
                        position: 0,
                        force_position: 0,
                        awaiting_value: false,
                    })
                };
                let id = self.heap.allocate(Object {
                    iterator,
                    ..Object::default()
                });
                self.stack.push(Value::Object(id));
            }
            Instruction::IteratorNext { end } => {
                let id = self
                    .stack
                    .iter()
                    .rev()
                    .filter_map(Value::heap_id)
                    .find(|id| {
                        self.heap
                            .get(*id)
                            .is_ok_and(|object| object.iterator.is_some())
                    })
                    .or_else(|| self.stack.last().and_then(Value::heap_id))
                    .ok_or_else(|| invalid("missing iterator"))?;
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
                let source = self
                    .heap
                    .get(id)?
                    .iterator
                    .as_ref()
                    .expect("active iterator")
                    .source;
                let effective = ordered_members(&self.heap, source)?;
                self.heap
                    .get_mut(id)?
                    .iterator
                    .as_mut()
                    .expect("active iterator")
                    .members = effective;
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
                let typed_target = typed_class(&self.heap, target)?;
                let forbidden = if typed_target.is_some() {
                    source_member.kind != CollectionMemberKind::Property
                } else {
                    match target_kind {
                        CollectionKind::Listing | CollectionKind::List => {
                            source_member.kind != CollectionMemberKind::Element
                        }
                        CollectionKind::Mapping => {
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
                    let type_label = typed_target.map_or_else(
                        || format!("{target_kind:?}"),
                        |class| format!("{}#{}", self.program.module_name, class.name),
                    );
                    return Err(self.spread_error(format!("Cannot spread object containing {kind} into object of type `{type_label}`."), &member));
                }
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
                    Value::Int(self.heap.get(target)?.length as i64)
                } else {
                    key.clone()
                };
                if self.prepare_insertion(&insertion_key, source_member.kind)? {
                    self.stack.push(key);
                    self.stack.push(value);
                    return Ok(());
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
                    return Err(self.spread_error(format!("Cannot spread object because the enclosing object already has a declaration of {label} `{key}`."), &member));
                }
                member.constant = false;
                let key = (source_member.kind != CollectionMemberKind::Element).then_some(key);
                self.add_collection_member(member, key, source_member.kind, Some(value))?;
            }
            Instruction::OpenPredicate => {
                let target = self.builder()?;
                let object = self.heap.get(target)?;
                let class = typed_class(&self.heap, target)?;
                if object.is_module
                    || class.is_some()
                    || object.collection_kind == CollectionKind::List
                {
                    let label = class.map_or_else(
                        || {
                            if object.is_module {
                                self.program.module_name.to_string()
                            } else {
                                format!("{:?}", object.collection_kind)
                            }
                        },
                        |class| format!("{}#{}", self.program.module_name, class.name),
                    );
                    return Err(RuntimeError::new(
                        ErrorKind::Type,
                        format!("Object of type `{label}` cannot have a predicate member."),
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
                    predicate_iterator: Some(PredicateIterator {
                        source,
                        members,
                        position: 0,
                        awaiting_value: false,
                        previous_this,
                    }),
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
                        member,
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
            Instruction::MakeList { length } => {
                let mut values = Vec::with_capacity(length);
                for _ in 0..length {
                    values.push(self.pop()?);
                }
                values.reverse();
                let id = self.heap.allocate(Object {
                    collection_kind: CollectionKind::List,
                    ..Object::default()
                });
                self.frames
                    .last_mut()
                    .expect("active frame")
                    .builders
                    .push(id);
                for value in values {
                    let member = Member {
                        entry: true,
                        name: Arc::from(""),
                        code: usize::MAX,
                        local: false,
                        method: false,
                        ty: None,
                        type_span: None,
                        constant: false,
                        fixed: false,
                    };
                    self.add_collection_member(
                        member,
                        None,
                        CollectionMemberKind::Element,
                        Some(value),
                    )?;
                }
                self.frames.last_mut().expect("active frame").builders.pop();
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
        let result = if object.specialized_literal && object.members.is_empty() {
            object.parent.unwrap_or(id)
        } else {
            id
        };
        self.stack.push(Value::Object(result));
        Ok(())
    }
}

impl Compiler<'_> {
    pub(super) fn instance_body(
        &mut self,
        members: &[crate::syntax::ObjectMember],
        span: Span,
        output: &mut Code,
    ) -> Result<(), RuntimeError> {
        if members
            .iter()
            .all(|member| matches!(member, crate::syntax::ObjectMember::Property(_)))
        {
            let properties = Self::body_properties_only(members)?;
            let template = self.object(&properties)?;
            output.push_at(Instruction::MakeInstance { template }, span);
            self.initialize_member_types(&properties, output)?;
        } else {
            // Construct the typed parent first, preserving class validation.
            // Generator clauses then execute in source order, including
            // predicates that reject typed parents only when reached.
            let template = self.object(&[])?;
            output.push_at(Instruction::MakeInstance { template }, span);
            self.collection_body(members, CollectionKind::Dynamic, true, output)?;
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
        self.type_scopes.push(properties.clone());
        let generators = members.iter().any(|m| {
            matches!(
                m,
                crate::syntax::ObjectMember::When { .. }
                    | crate::syntax::ObjectMember::For { .. }
                    | crate::syntax::ObjectMember::Spread { .. }
                    | crate::syntax::ObjectMember::Predicate { .. }
            )
        });
        output.push(Instruction::BeginCollection {
            kind,
            has_parent,
            specialized: !generators,
        });
        let mixed = members
            .iter()
            .any(|m| matches!(m, crate::syntax::ObjectMember::Element { .. }));
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
        self.initialize_member_types(&properties, output)?;
        self.type_scopes.pop();
        Ok(())
    }

    fn lazy_collection_member(
        &mut self,
        value: &crate::syntax::Expr,
        key: Option<&crate::syntax::Expr>,
        span: Span,
    ) -> Result<Member, RuntimeError> {
        let previous = self.member.clone();
        let literal = key.and_then(|key| match &key.kind {
            crate::syntax::ExprKind::String(value) => Some(format!("\"{value}\"")),
            crate::syntax::ExprKind::Int(value) => Some(value.to_string()),
            crate::syntax::ExprKind::Float(value) => Some(crate::numeric::double_to_string(*value)),
            crate::syntax::ExprKind::Bool(value) => Some(value.to_string()),
            _ => None,
        });
        let label = literal.unwrap_or_else(|| {
            let count = self.entry_counts.entry(previous.clone()).or_default();
            *count += 1;
            format!("#{count}")
        });
        self.member = format!("{previous}[{label}]");
        let mut code = Code::default();
        if let crate::syntax::ExprKind::Object {
            parent: None,
            members,
        } = &value.unparenthesized().kind
        {
            code.push(Instruction::EntryParent);
            self.collection_body(members, CollectionKind::Dynamic, true, &mut code)?;
        } else {
            self.expression(value, &mut code)?;
        }
        code.push(Instruction::Return);
        let block = self.block(code, Some(span));
        self.program.metadata[block].value_span = Some(value.span);
        self.member = previous;
        Ok(Member {
            entry: true,
            name: Arc::from(""),
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
                    output.push_at(Instruction::AddCollectionProperty(compiled), span);
                }
                ObjectMember::Element { value, span } => {
                    let member = self.lazy_collection_member(value, None, value.span)?;
                    output.push_at(Instruction::AddCollectionElement(member), *span);
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
                            member,
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
                        iterable.span,
                    );
                    let start = output.len();
                    output.push_at(Instruction::IteratorNext { end: 0 }, iterable.span);
                    if parameters.len() == 1 {
                        output.push(Instruction::Swap);
                        output.push(Instruction::Pop);
                        if let (Some(ty), Some(check)) = (&parameters[0].ty, &checks[0]) {
                            output.push_at(Instruction::CheckType(check.clone()), ty.span);
                        }
                        output.push(Instruction::PushLocal);
                    } else if parameters.len() == 2 {
                        output.push(Instruction::Swap);
                        if let (Some(ty), Some(check)) = (&parameters[0].ty, &checks[0]) {
                            output.push_at(Instruction::CheckType(check.clone()), ty.span);
                        }
                        output.push(Instruction::PushLocal);
                        if let (Some(ty), Some(check)) = (&parameters[1].ty, &checks[1]) {
                            output.push_at(Instruction::CheckType(check.clone()), ty.span);
                        }
                        output.push(Instruction::PushLocal);
                    } else {
                        return Err(unsupported_type("this generator parameter count").at(*span));
                    }
                    self.collection_members(members, false, output)?;
                    for _ in parameters {
                        output.push(Instruction::PopLocal);
                    }
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
                    output.push_at(
                        Instruction::OpenIterator {
                            nullable: *nullable,
                            spread: true,
                        },
                        value.span,
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
                    output.push_at(Instruction::AddPredicateMember(member), condition.span);
                    output.push(Instruction::Jump(start));
                    output[start] = Instruction::PredicateNext { end: output.len() };
                }
            }
        }
        Ok(())
    }
}
