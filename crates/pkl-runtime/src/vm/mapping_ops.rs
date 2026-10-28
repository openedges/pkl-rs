//! Mapping keys and cache-first lookups remain lazy across Host/GC suspension.
use super::*;
use crate::heap::{CollectionKind, CollectionMemberKind};

#[derive(Clone, Copy, Debug)]
pub enum Operation {
    KeysStart,
    KeysStep,
    Contains,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum LookupMode {
    Contains,
    Null,
    Default,
    Subscript,
    ForcedVisitor,
    LazyVisitor,
    Super(usize),
    Parent(usize),
}
impl LookupMode {
    fn direct(self) -> bool {
        matches!(self, Self::Subscript | Self::Super(_) | Self::Parent(_))
    }
}
#[derive(Clone, Copy)]
enum Phase {
    Cache(ObjectId),
    Declaration(ObjectId),
    Steal,
}
pub(super) struct LookupRun {
    receiver: ObjectId,
    query: Value,
    mode: LookupMode,
    phase: Phase,
    empty_wrappers: Vec<ObjectId>,
    seen: BTreeSet<ObjectId>,
    steal: Option<(Arc<str>, Vec<ObjectId>, usize, bool)>,
    value: Option<Value>,
    repeated_cache: bool,
}
impl LookupRun {
    pub(super) fn roots(&self, roots: &mut Vec<Value>) {
        roots.extend([Value::Object(self.receiver), self.query.clone()]);
        roots.extend(self.empty_wrappers.iter().copied().map(Value::Object));
        roots.extend(self.value.iter().cloned());
        if let Some((_, chain, _, _)) = &self.steal {
            roots.extend(chain.iter().copied().map(Value::Object));
        }
    }
}
pub(super) struct KeysRun {
    chain: Vec<ObjectId>,
    owner: usize,
    parent_keys: ObjectId,
    members: Vec<Value>,
    next: usize,
    building: bool,
}
impl KeysRun {
    pub(super) fn roots(&self, roots: &mut Vec<Value>) {
        roots.extend(self.chain.iter().copied().map(Value::Object));
        roots.push(Value::Object(self.parent_keys));
        roots.extend(self.members.iter().cloned());
    }
}

pub(super) fn blocks(
    blocks: &mut Vec<Vec<Instruction>>,
    metadata: &mut Vec<BlockMetadata>,
) -> BTreeMap<(Arc<str>, Arc<str>), usize> {
    let source = crate::stdlib_sources::base();
    let class = source.text.find("class Mapping<").unwrap();
    let mut result = BTreeMap::new();
    for (name, signature, code) in [
        (
            "containsKey",
            "external function containsKey(key: Any): Boolean",
            vec![
                Instruction::Primitive(primitives::Operation::MappingOps(Operation::Contains)),
                Instruction::Return,
            ],
        ),
        (
            "keys",
            "external keys: Set<Key>",
            vec![
                Instruction::Primitive(primitives::Operation::MappingOps(Operation::KeysStart)),
                Instruction::Primitive(primitives::Operation::MappingOps(Operation::KeysStep)),
                Instruction::PrepareSetElement,
                Instruction::AddSetElement,
                Instruction::Jump(1),
                Instruction::Return,
            ],
        ),
    ] {
        let start = class + source.text[class..].find(signature).unwrap();
        let block = blocks.len();
        let len = code.len();
        blocks.push(code);
        metadata.push(BlockMetadata {
            source: Some(source.clone()),
            member: format!("#Mapping.{name}").into(),
            declaration: Some(Span::new(start, start + signature.len())),
            spans: vec![None; len],
            ..Default::default()
        });
        result.insert(("Mapping".into(), name.into()), block);
    }
    result
}

impl Vm {
    pub(super) fn mapping_ops(
        &mut self,
        receiver: Value,
        operation: Operation,
    ) -> Result<(), RuntimeError> {
        let source = receiver.as_object()?;
        match operation {
            Operation::Contains => {
                self.mapping_lookup(source, self.conversion_argument(0)?, LookupMode::Contains)
            }
            Operation::KeysStart => {
                let mut chain = Vec::new();
                let mut current = source;
                let mut seen = BTreeSet::new();
                loop {
                    if !seen.insert(current) {
                        return Err(invalid("cyclic Mapping keys parent chain"));
                    }
                    let object = self.heap.get(current)?;
                    chain.push(current);
                    if object.mapping_keys.is_some() {
                        break;
                    }
                    let Some(parent) = object.parent else {
                        break;
                    };
                    if self.heap.get(parent)?.collection_kind != CollectionKind::Mapping {
                        break;
                    }
                    current = parent;
                }
                chain.reverse();
                let parent_keys = self.empty_strict_collection(CollectionKind::Set);
                self.frames.last_mut().unwrap().mapping_keys = Some(Box::new(KeysRun {
                    chain,
                    owner: 0,
                    parent_keys,
                    members: Vec::new(),
                    next: 0,
                    building: false,
                }));
                Ok(())
            }
            Operation::KeysStep => self.mapping_keys_step(),
        }
    }

    fn mapping_keys_step(&mut self) -> Result<(), RuntimeError> {
        let mut run = *self
            .frames
            .last_mut()
            .unwrap()
            .mapping_keys
            .take()
            .ok_or_else(|| invalid("Mapping.keys cursor"))?;
        if run.owner == run.chain.len() {
            self.stack.push(Value::Object(run.parent_keys));
            return self.jump(5);
        }
        let owner = run.chain[run.owner];
        if !run.building {
            if let Some(keys) = self.heap.get(owner)?.mapping_keys {
                run.parent_keys = keys;
                run.owner += 1;
                self.frames.last_mut().unwrap().mapping_keys = Some(Box::new(run));
                return self.jump(1);
            }
            // VmSet.builder(parentKeys) copies persistent storage without
            // rehashing the inherited prefix, then adds only this owner's keys.
            let old = self.heap.get(run.parent_keys)?;
            let builder = self.heap.allocate(Object {
                collection_kind: CollectionKind::Set,
                length: old.length,
                members: old.members.clone(),
                collection_members: old.collection_members.clone(),
                map_index: old.map_index.clone(),
                set_entries: old.set_entries.clone(),
                member_values: old.member_values.clone(),
                cache: old.cache.clone(),
                completed_cache_order: old.completed_cache_order.clone(),
                cache_slots: old.cache_slots.clone(),
                cache_index: old.cache_index.clone(),
                ..Default::default()
            });
            self.frames.last_mut().unwrap().builders.push(builder);
            run.members = self
                .heap
                .get(owner)?
                .collection_members
                .iter()
                .filter(|m| m.kind == CollectionMemberKind::Entry)
                .map(|m| m.key.clone())
                .collect();
            run.next = 0;
            run.building = true;
        }
        if let Some(key) = run.members.get(run.next).cloned() {
            run.next += 1;
            self.frames.last_mut().unwrap().mapping_keys = Some(Box::new(run));
            self.stack.push(key);
            return Ok(());
        }
        let builder = self
            .frames
            .last_mut()
            .unwrap()
            .builders
            .pop()
            .ok_or_else(|| invalid("Mapping.keys builder"))?;
        let result = self.finish_strict_collection(builder)?;
        self.heap.get_mut(owner)?.mapping_keys = Some(result);
        run.parent_keys = result;
        run.owner += 1;
        run.building = false;
        run.members.clear();
        self.frames.last_mut().unwrap().mapping_keys = Some(Box::new(run));
        self.jump(1)
    }

    /// Subscript retains its evaluated operands in the run across suspension.
    pub(super) fn resume_entry_lookup(&mut self) -> Result<bool, RuntimeError> {
        let args = self
            .frames
            .last()
            .and_then(|f| f.mapping_lookup.as_ref())
            .filter(|run| run.mode.direct())
            .map(|run| (run.receiver, run.query.clone(), run.mode));
        if let Some((receiver, query, mode)) = args {
            self.mapping_lookup(receiver, query, mode)?;
            return Ok(true);
        }
        Ok(false)
    }

    pub(super) fn resume_visitor_lookup(&mut self) -> Result<bool, RuntimeError> {
        let args = self
            .frames
            .last()
            .and_then(|f| f.mapping_lookup.as_ref())
            .filter(|run| {
                matches!(
                    run.mode,
                    LookupMode::ForcedVisitor | LookupMode::LazyVisitor
                )
            })
            .map(|run| (run.receiver, run.query.clone(), run.mode));
        if let Some((receiver, query, mode)) = args {
            self.mapping_lookup(receiver, query, mode)?;
            return Ok(true);
        }
        Ok(false)
    }

    pub(super) fn super_entry_lookup(
        &mut self,
        receiver: ObjectId,
        owner: Option<ObjectId>,
        query: Value,
        end: usize,
        implicit: bool,
    ) -> Result<(), RuntimeError> {
        let mode = if implicit {
            LookupMode::Parent(end)
        } else {
            LookupMode::Super(end)
        };
        let Some(owner) = owner else {
            if !implicit {
                self.stack.push(query);
            }
            return self.force(receiver, receiver, "default".into(), false, true, false);
        };
        self.frames.last_mut().unwrap().mapping_lookup = Some(Box::new(LookupRun {
            receiver,
            query: query.clone(),
            mode,
            phase: Phase::Declaration(owner),
            empty_wrappers: Vec::new(),
            seen: BTreeSet::new(),
            steal: None,
            value: None,
            repeated_cache: false,
        }));
        self.mapping_lookup(receiver, query, mode)
    }

    pub(super) fn mapping_lookup(
        &mut self,
        receiver: ObjectId,
        query: Value,
        mode: LookupMode,
    ) -> Result<(), RuntimeError> {
        if self.frames.last().unwrap().mapping_lookup.is_none() {
            self.frames.last_mut().unwrap().mapping_lookup = Some(Box::new(LookupRun {
                receiver,
                query: query.clone(),
                mode,
                phase: Phase::Cache(receiver),
                empty_wrappers: Vec::new(),
                seen: BTreeSet::new(),
                steal: None,
                value: None,
                repeated_cache: false,
            }));
        }
        let phase = self
            .frames
            .last()
            .unwrap()
            .mapping_lookup
            .as_ref()
            .unwrap()
            .phase;
        if matches!(phase, Phase::Steal) {
            return self.mapping_steal_step();
        }
        let owner = match phase {
            Phase::Cache(owner) | Phase::Declaration(owner) => owner,
            Phase::Steal => unreachable!(),
        };
        let object = self.heap.get(owner)?;
        let hash = match phase {
            Phase::Cache(_) => object.cache_index.has_hash(),
            Phase::Declaration(_) => object.members.len() > 4,
            Phase::Steal => false,
        };
        if hash && self.prepare_keys(vec![query.clone()])? {
            return Ok(());
        }
        let object = self.heap.get(owner)?;
        let candidates: Vec<(Value, Arc<str>, Option<Value>)> = match phase {
            Phase::Cache(_) => {
                let indices = if object.cache_index.has_hash() {
                    object
                        .cache_index
                        .candidates(Some(self.java_hash(&query)?.unwrap_or(0)))
                } else {
                    (0..object.cache_slots.len()).collect()
                };
                let mut values = Vec::new();
                for i in indices {
                    let slot = &object.cache_slots[i];
                    if slot.key.entry
                        && slot.key.owner.is_none()
                        && !slot.key.method
                        && let Some(key) =
                            collections::cache_slot_entry_key(&self.heap, owner, slot)?
                    {
                        values.push((key, slot.key.name.clone(), Some(slot.value.clone())));
                    }
                }
                values
            }
            Phase::Declaration(_) => {
                let indices = if let (Some(index), Some(hash)) =
                    (&object.declaration_index, self.java_hash(&query)?)
                {
                    index.candidates(Some(hash))
                } else {
                    (0..object.collection_members.len()).collect()
                };
                indices
                    .into_iter()
                    .filter_map(|i| {
                        let m = &object.collection_members[i];
                        (m.kind != CollectionMemberKind::Property)
                            .then(|| (m.key.clone(), m.name.clone(), None))
                    })
                    .collect()
            }
            Phase::Steal => unreachable!(),
        };
        if self.search_key(
            &query,
            candidates.iter().map(|(key, _, _)| key.clone()).collect(),
        )? {
            return Ok(());
        }
        if let Some((_, name, cached)) = candidates
            .into_iter()
            .find(|(key, _, _)| equality::same_key(&self.heap, &query, key))
        {
            if mode == LookupMode::Contains {
                return self.finish_mapping_lookup(Value::Bool(true));
            }
            if let Some(value) = cached {
                if owner == receiver {
                    return self.finish_mapping_lookup(value);
                }
                let run = self
                    .frames
                    .last_mut()
                    .unwrap()
                    .mapping_lookup
                    .as_mut()
                    .unwrap();
                let mut chain = run.empty_wrappers.clone();
                chain.reverse();
                run.steal = Some((name, chain, 0, false));
                run.value = Some(value);
                run.phase = Phase::Steal;
                return self.mapping_steal_step();
            }
            self.frames.last_mut().unwrap().mapping_lookup = None;
            let frame = self.frames.last_mut().unwrap();
            let memoize = match mode {
                LookupMode::Super(end) | LookupMode::Parent(end) => {
                    frame.pc = end;
                    false
                }
                LookupMode::Subscript | LookupMode::LazyVisitor => {
                    if mode == LookupMode::LazyVisitor {
                        frame.pc -= 1;
                    }
                    true
                }
                _ => {
                    frame.pc = self.program.blocks[frame.block].len() - 1;
                    true
                }
            };
            return self.force_key_mode_probe(
                receiver,
                owner,
                name,
                false,
                memoize,
                false,
                true,
                false,
                true,
                None,
                Some(query),
            );
        }
        let object = self.heap.get(owner)?;
        let parent = object.parent;
        let steal = mode != LookupMode::Contains
            && matches!(phase, Phase::Cache(_))
            && object.members.is_empty()
            && object.collection_default.is_none()
            && matches!(
                object.collection_kind,
                CollectionKind::Mapping | CollectionKind::Listing
            );
        // An unmatched explicit super entry probes the inherited declaration
        // chain before invoking the receiver's default. Builtin declarations
        // are represented separately here, so preserve their final key-hash
        // effect even when every materialized owner table was small. A hit in
        // an earlier small table still avoids hashing its query.
        // Keep the run and its roots intact before suspension, and do not mark
        // this terminal owner visited until the probe has completed.
        if matches!(mode, LookupMode::Super(_))
            && matches!(phase, Phase::Declaration(_))
            && parent.is_none()
            && self.prepare_keys(vec![query.clone()])?
        {
            return Ok(());
        }
        let run = self
            .frames
            .last_mut()
            .unwrap()
            .mapping_lookup
            .as_mut()
            .unwrap();
        if !run.seen.insert(owner) {
            return Err(invalid("cyclic Mapping lookup parent chain"));
        }
        match phase {
            Phase::Cache(_) if steal && parent.is_some() => {
                run.empty_wrappers.push(owner);
                run.phase = Phase::Cache(parent.unwrap());
            }
            Phase::Cache(_) => {
                if mode == LookupMode::ForcedVisitor {
                    return Err(RuntimeError::new(
                        ErrorKind::Unsupported,
                        "The pinned forced-iteration missing-cache internal-error protocol is not implemented.",
                    ));
                }
                if mode == LookupMode::LazyVisitor && !run.repeated_cache {
                    // iterateMemberValues supplies a cache miss; the callback's
                    // VmUtils.readMember then performs its own cache lookup.
                    run.repeated_cache = true;
                    run.empty_wrappers.clear();
                    run.phase = Phase::Cache(receiver);
                } else {
                    run.phase = Phase::Declaration(receiver);
                }
                run.seen.clear();
            }
            Phase::Declaration(_) if parent.is_some() => {
                run.phase = Phase::Declaration(parent.unwrap());
            }
            Phase::Declaration(_) => {
                self.frames.last_mut().unwrap().mapping_lookup = None;
                if matches!(mode, LookupMode::Subscript | LookupMode::LazyVisitor) {
                    return self.fail_missing_key(receiver, query);
                }
                if matches!(mode, LookupMode::Super(_) | LookupMode::Parent(_)) {
                    if matches!(mode, LookupMode::Super(_)) {
                        self.stack.push(query);
                    }
                    return self.force(receiver, receiver, "default".into(), false, true, false);
                }
                if mode == LookupMode::Default {
                    return self.force(receiver, receiver, "default".into(), false, true, false);
                }
                return self.finish_mapping_lookup(if mode == LookupMode::Contains {
                    Value::Bool(false)
                } else {
                    Value::Null
                });
            }
            Phase::Steal => unreachable!(),
        }
        let frame = self.frames.last_mut().unwrap();
        // Each Java getCachedValue/getMember call hashes its probe independently,
        // including a zero hash whose lazy contents can change on reentry.
        frame.prepared_hashes = None;
        frame.pc -= 1;
        Ok(())
    }

    fn finish_mapping_lookup(&mut self, value: Value) -> Result<(), RuntimeError> {
        let frame = self.frames.last_mut().unwrap();
        let subscript = frame
            .mapping_lookup
            .as_ref()
            .is_some_and(|run| run.mode == LookupMode::Subscript);
        let visitor = frame.mapping_lookup.as_ref().is_some_and(|run| {
            matches!(
                run.mode,
                LookupMode::ForcedVisitor | LookupMode::LazyVisitor
            )
        });
        frame.mapping_lookup = None;
        if visitor {
            frame.pc -= 1;
        } else if !subscript {
            frame.pc = self.program.blocks[frame.block].len() - 1;
        }
        self.stack.push(value);
        Ok(())
    }

    fn mapping_steal_step(&mut self) -> Result<(), RuntimeError> {
        let awaiting = self
            .frames
            .last()
            .unwrap()
            .mapping_lookup
            .as_ref()
            .unwrap()
            .steal
            .as_ref()
            .unwrap()
            .3;
        if awaiting {
            let value = self.pop()?;
            let run = self
                .frames
                .last_mut()
                .unwrap()
                .mapping_lookup
                .as_mut()
                .unwrap();
            run.value = Some(value);
            let (_, _, next, waiting) = run.steal.as_mut().unwrap();
            *next += 1;
            *waiting = false;
        }
        let run = self
            .frames
            .last_mut()
            .unwrap()
            .mapping_lookup
            .as_mut()
            .unwrap();
        let (name, chain, next, awaiting) = run.steal.as_mut().unwrap();
        if *next == chain.len() {
            let value = run
                .value
                .take()
                .ok_or_else(|| invalid("stolen Mapping value"))?;
            return self.finish_mapping_lookup(value);
        }
        let query = run.query.clone();
        let value = run
            .value
            .clone()
            .ok_or_else(|| invalid("stolen Mapping value"))?;
        let receiver = chain[*next];
        let name = name.clone();
        *awaiting = true;
        self.frames.last_mut().unwrap().pc -= 1;
        // Each empty wrapper gets its own cast and cache publication, deepest
        // first. Its earlier raw cache lookup has already prepared key hashing.
        self.force_key_mode_value(
            receiver,
            receiver,
            name,
            false,
            true,
            false,
            true,
            false,
            true,
            None,
            Some(query),
            Some(value),
        )
    }
}

/// A wrapper catches only its own cast. The retained error contains immutable
/// diagnostic data and cast-target identities; all retained heap edges are rooted.
#[derive(Clone)]
pub(super) struct CastFailure {
    target: crate::heap::TypeNodeId,
    known_member: Option<(Option<RuntimeFrame>, bool)>,
    owner: ObjectId,
    probe: Value,
    stack: usize,
    pc: usize,
    error: Option<RuntimeError>,
    seen: BTreeSet<ObjectId>,
}
impl CastFailure {
    pub(super) fn new(
        owner: ObjectId,
        probe: Value,
        stack: usize,
        pc: usize,
        target: crate::heap::TypeNodeId,
    ) -> Self {
        Self {
            target,
            known_member: None,
            owner,
            probe,
            stack,
            pc,
            error: None,
            seen: BTreeSet::new(),
        }
    }
    pub(super) fn known_member(
        receiver: ObjectId,
        stack: usize,
        pc: usize,
        target: crate::heap::TypeNodeId,
        member: Option<RuntimeFrame>,
        constant: bool,
    ) -> Self {
        let mut result = Self::new(receiver, Value::Null, stack, pc, target);
        result.known_member = Some((member, constant));
        result
    }
    pub(super) fn active_target(&self) -> Option<crate::heap::TypeNodeId> {
        self.error.is_none().then_some(self.target)
    }
    pub(super) fn roots(&self, roots: &mut Vec<Value>) {
        roots.extend([Value::Object(self.owner), self.probe.clone()]);
        roots.extend(self.seen.iter().copied().map(Value::Object));
        roots.push(self.target.0.root());
        if let Some(error) = &self.error {
            error.diagnostic_roots(roots);
        }
    }
}
impl Vm {
    pub(super) fn mapping_cast_boundary(&self, error: &RuntimeError) -> Option<usize> {
        if !guest_exceptions::is_guest_exception(error) {
            return None;
        }
        self.frames.iter().rposition(|frame| {
            frame
                .force
                .as_ref()
                .and_then(|force| force.stolen_cast.as_ref())
                .is_some_and(|handler| handler.error.is_none())
        })
    }
    pub(super) fn catch_mapping_cast(&mut self, index: usize, mut error: RuntimeError) {
        self.frames.truncate(index + 1);
        let frame = &mut self.frames[index];
        let handler = frame
            .force
            .as_mut()
            .expect("cast force")
            .stolen_cast
            .as_mut()
            .expect("cast handler");
        error.reserve_cast_target(handler.target);
        handler.error = Some(error);
        self.stack.truncate(handler.stack);
        frame.type_run = None;
        frame.type_resolution = None;
        frame.generic_default = None;
        frame.module_load = None;
        frame.uri_resolution = None;
        frame.deep_force = None;
        frame.prepared_hashes = None;
        frame.key_search = None;
        frame.equality_run = None;
        self.state = State::Running;
    }
    pub(super) fn mapping_cast_failure_pending(&self) -> bool {
        self.frames
            .last()
            .and_then(|frame| frame.force.as_ref())
            .and_then(|force| force.stolen_cast.as_ref())
            .is_some_and(|handler| handler.error.is_some())
    }
    pub(super) fn step_mapping_cast_failure(&mut self) -> Result<(), RuntimeError> {
        let frame = self.frames.last_mut().expect("cast failure frame");
        let handler = frame
            .force
            .as_ref()
            .expect("cast force")
            .stolen_cast
            .as_ref()
            .expect("cast failure");
        if handler.known_member.is_some() {
            let mut handler = frame
                .force
                .as_mut()
                .expect("cast force")
                .stolen_cast
                .take()
                .expect("cast failure");
            let mut error = handler.error.take().expect("retained cast error");
            let (member, constant) = handler.known_member.take().expect("known member");
            if let Some(member) = member {
                if constant {
                    error.insert_cast_frame(handler.target, member)?;
                } else if !error.cached_module_frames {
                    // An executed ElementOrEntryNode already has an ordinary
                    // call frame; executeTypeCasts receives member=null here.
                    error.frames.push(member);
                }
            }
            frame.callsite_hidden = true;
            return Err(error);
        }
        // Key preparation retries an instruction. This continuation has no new
        // instruction fetch, so restore its saved PC before each routed lookup.
        frame.pc = handler.pc;
        let owner = handler.owner;
        let probe = handler.probe.clone();
        let object = self.heap.get(owner)?;
        if object.members.len() > 4 && self.prepare_keys(vec![probe.clone()])? {
            return Ok(());
        }
        let object = self.heap.get(owner)?;
        let indices = if let (Some(index), Some(hash)) =
            (&object.declaration_index, self.java_hash(&probe)?)
        {
            index.candidates(Some(hash))
        } else {
            (0..object.collection_members.len()).collect()
        };
        let candidates: Vec<_> = indices
            .into_iter()
            .filter_map(|index| {
                let member = &object.collection_members[index];
                (member.kind != CollectionMemberKind::Property)
                    .then(|| (member.key.clone(), member.name.clone()))
            })
            .collect();
        if self.search_key(
            &probe,
            candidates.iter().map(|(key, _)| key.clone()).collect(),
        )? {
            return Ok(());
        }
        if let Some((_, name)) = candidates
            .into_iter()
            .find(|(key, _)| equality::same_key(&self.heap, &probe, key))
        {
            let member = self
                .heap
                .get(owner)?
                .members
                .iter()
                .find(|member| {
                    member.entry && !member.local && !member.method && member.name == name
                })
                .ok_or_else(|| invalid("cast diagnostic declaration missing"))?;
            let inserted = self.program.metadata.get(member.code).and_then(|metadata| {
                metadata
                    .value_span
                    .or(metadata.declaration)
                    .map(|span| RuntimeFrame {
                        source: metadata.source.clone(),
                        span,
                        member: metadata.member.clone(),
                    })
            });
            let mut handler = self
                .frames
                .last_mut()
                .expect("cast failure frame")
                .force
                .as_mut()
                .expect("cast force")
                .stolen_cast
                .take()
                .expect("cast failure");
            let mut error = handler.error.take().expect("retained cast error");
            // Same-target outer catches replace the first captured insertion;
            // distinct targets retain their own positions in the same error.
            if let Some(inserted) = inserted {
                error.insert_cast_frame(handler.target, inserted)?;
            }
            self.frames
                .last_mut()
                .expect("cast failure frame")
                .callsite_hidden = true;
            return Err(error);
        }
        let parent = self.heap.get(owner)?.parent;
        let frame = self.frames.last_mut().expect("cast failure frame");
        let handler = frame
            .force
            .as_mut()
            .expect("cast force")
            .stolen_cast
            .as_mut()
            .expect("cast failure");
        if !handler.seen.insert(owner) {
            return Err(RuntimeError::new(ErrorKind::Cycle, "cyclic parent chain"));
        }
        let Some(parent) = parent else {
            // Java asserts that a cached member has a declaration. Preserve the
            // unresolved internal protocol instead of reporting the cast error.
            return Err(RuntimeError::new(
                ErrorKind::Unsupported,
                "The pinned cached-member diagnostic lookup missing-member internal-error protocol is not implemented.",
            ));
        };
        handler.owner = parent;
        frame.prepared_hashes = None;
        frame.key_search = None;
        Ok(())
    }
}
