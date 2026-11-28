//! Persistent Map updates retain their stored keys and insertion order.
use super::*;
use crate::heap::CollectionKind;

#[derive(Clone, Copy, Debug)]
pub enum Operation {
    PutContains,
    PutClone,
    PutAssociate,
    Remove,
}

#[cfg_attr(target_arch = "wasm32", inline(never))]
pub(super) fn blocks(
    blocks: &mut Vec<Vec<Instruction>>,
    metadata: &mut Vec<BlockMetadata>,
) -> BTreeMap<(Arc<str>, Arc<str>), usize> {
    let source = crate::stdlib_sources::base();
    let class = source.text.find("external class Map<").unwrap();
    let start = class
        + source
            .text
            .entire()
            .range(class..)
            .find("external function put<")
            .unwrap();
    let close = start + source.text.entire().range(start..).find("): Map<").unwrap();
    let end = close + source.text.entire().range(close..).find("\n").unwrap();
    let block = blocks.len();
    blocks.push(vec![
        Instruction::Primitive(primitives::Operation::MapOps(Operation::PutContains)),
        Instruction::Primitive(primitives::Operation::MapOps(Operation::PutClone)),
        Instruction::Primitive(primitives::Operation::MapOps(Operation::PutAssociate)),
        Instruction::Return,
    ]);
    metadata.push(BlockMetadata {
        source: Some(source),
        member: "#Map.put".into(),
        declaration: Some(Span::new(start, end)),
        spans: vec![None; 4],
        ..Default::default()
    });
    let remove_start = class
        + metadata[block]
            .source
            .as_ref()
            .unwrap()
            .text
            .entire()
            .range(class..)
            .find("external function remove(")
            .unwrap();
    let source = metadata[block].source.as_ref().unwrap().clone();
    let remove_end = remove_start
        + source
            .text
            .entire()
            .range(remove_start..)
            .find("\n")
            .unwrap();
    let remove_block = blocks.len();
    blocks.push(vec![
        Instruction::Primitive(primitives::Operation::MapOps(Operation::Remove)),
        Instruction::Return,
    ]);
    metadata.push(BlockMetadata {
        source: Some(source),
        member: "#Map.remove".into(),
        declaration: Some(Span::new(remove_start, remove_end)),
        spans: vec![None; 2],
        ..Default::default()
    });
    BTreeMap::from([
        (("Map".into(), "put".into()), block),
        (("Map".into(), "remove".into()), remove_block),
    ])
}

impl Vm {
    #[cfg_attr(target_arch = "wasm32", inline(never))]
    pub(super) fn map_ops(
        &mut self,
        receiver: Value,
        operation: Operation,
    ) -> Result<(), RuntimeError> {
        let source = receiver.as_object()?;
        let locals = &self
            .frames
            .last()
            .ok_or_else(|| invalid("Map.put frame"))?
            .environment
            .locals;
        if matches!(operation, Operation::Remove) {
            let key = locals[locals.len() - 1].clone();
            if self.heap.get(source)?.map_index().len() == 0 {
                self.stack.push(receiver);
            } else {
                self.begin_map_remove(source, key);
            }
            return Ok(());
        }
        let key = locals[locals.len() - 2].clone();
        let value = locals[locals.len() - 1].clone();
        match operation {
            Operation::Remove => unreachable!(),
            Operation::PutContains => {
                // VmMap.put checks containsKey before associating, including
                // replacement. Separate instructions repeat uncached hashing.
                if self.prepare_map_lookup(source, &key)? {
                    return Ok(());
                }
                self.stack
                    .push(Value::Bool(self.map_value(source, &key)?.is_none()));
            }
            Operation::PutClone => {
                let old = self.heap.get(source)?;
                let target = self.heap.allocate(Object {
                    collection_kind: CollectionKind::Map,
                    length: old.length,
                    members: old.members.clone(),
                    collection_members: old.collection_members.clone(),
                    member_values: old.member_values.clone(),
                    cache_map: old.cache_map.clone(),
                    cache_slots: old.cache_slots.clone(),
                    has_object_collection_key: old.has_object_collection_key,
                    cache_index: old.cache_index.clone(),
                    map_index: old.map_index.clone(),
                    map_key_order: old.map_key_order.clone(),
                    ..Default::default()
                });
                self.frames.last_mut().unwrap().builders.push(target);
            }
            Operation::PutAssociate => {
                let target = *self
                    .frames
                    .last()
                    .unwrap()
                    .builders
                    .last()
                    .ok_or_else(|| invalid("Map.put builder"))?;
                if self.prepare_map_association(target, &key)? {
                    return Ok(());
                }
                let Value::Bool(append_order) = self.pop()? else {
                    return Err(invalid("Map.put contains result"));
                };
                self.insert_map_entry_ordered(target, key, value, Some(append_order))?;
                self.frames.last_mut().unwrap().builders.pop();
                // VmMap.put always returns a fresh wrapper, even when an
                // association leaves the same stored key and value identities.
                self.stack.push(Value::Object(target));
            }
        }
        Ok(())
    }
}

impl Vm {
    /// Apply independently completed persistent-trie and captured-order removals.
    pub(super) fn finish_map_remove(
        &mut self,
        source: ObjectId,
        removed: Option<(usize, Option<i32>)>,
        order_position: usize,
    ) -> Result<ObjectId, RuntimeError> {
        let mut order = collections::ordered_members(&self.heap, source)?;
        order.remove(order_position);
        let old = self.heap.get(source)?;
        let removed_name = removed.map(|(slot, _)| old.collection_members[slot].name.clone());
        let keep = |name: &PklString| removed_name.as_ref() != Some(name);
        let map_index = match removed {
            Some((slot, hash)) => old.map_index().without_matched(hash, slot),
            None => old.map_index().clone(),
        };
        let collection_members: Vec<_> = old
            .collection_members
            .iter()
            .filter(|member| keep(&member.name))
            .cloned()
            .collect();
        let has_object_collection_key = old.has_object_collection_key
            && collection_members
                .iter()
                .any(|member| matches!(member.key, Value::Object(_)));
        let target = self.heap.allocate(Object {
            collection_kind: CollectionKind::Map,
            length: order.len(),
            members: old
                .members
                .iter()
                .filter(|member| keep(&member.name))
                .cloned()
                .collect(),
            collection_members,
            has_object_collection_key,
            map_index: Some(Box::new(map_index)),
            map_key_order: Some(Box::new(order)),
            member_values: old
                .member_values
                .iter()
                .filter(|(key, _)| keep(&key.name))
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect(),
            cache_slots: old
                .cache_slots
                .iter()
                .filter(|slot| keep(&slot.key.name))
                .cloned()
                .collect(),
            ..Default::default()
        });
        self.heap.get_mut(target)?.rebuild_cache_map();
        // VmMap.remove calls its constructor directly, including empty results.
        Ok(target)
    }
}
