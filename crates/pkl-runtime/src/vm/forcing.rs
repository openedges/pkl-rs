//! VmObject.force(true, true) as evaluator-owned, interruptible continuations.
use super::*;
use crate::heap::CollectionKind;

#[derive(Clone)]
enum Task {
    Value(Value),
    Members {
        receiver: ObjectId,
        owner: ObjectId,
        index: usize,
    },
    Finish(ObjectId),
}
struct ReadBoundary {
    receiver: ObjectId,
    owner: ObjectId,
    name: Arc<str>,
    entry: bool,
    stack: usize,
}
pub(super) enum FinishAction {
    ReturnValue,
    ObjectString,
    Trace(super::tracing::TraceRun),
}
pub(super) struct ForceRun {
    root: Value,
    pending_render_key: Option<Value>,
    pending_render_comparison: Option<(Value, Value)>,
    allow_undefined: bool,
    recurse: bool,
    finish: FinishAction,
    renderer: Option<crate::diagnostic_values::RenderRun>,
    tasks: Vec<Task>,
    /// Only unfinished calls own rollback of a published force marker.
    active: Vec<ObjectId>,
    /// Missing standard-library bindings remain deferred capability gaps.
    incomplete: BTreeSet<ObjectId>,
    awaiting: Option<ReadBoundary>,
}
impl ForceRun {
    pub(super) fn is_trace(&self) -> bool {
        matches!(self.finish, FinishAction::Trace(_))
    }
    pub(super) fn acknowledge_log(&mut self) -> Result<(), RuntimeError> {
        let FinishAction::Trace(trace) = &mut self.finish else {
            return Err(invalid("log outside trace continuation"));
        };
        trace.acknowledge()
    }

    pub(super) fn roots(&self, roots: &mut Vec<Value>) {
        roots.push(self.root.clone());
        roots.extend(self.pending_render_key.iter().cloned());
        if let Some((left, right)) = &self.pending_render_comparison {
            roots.extend([left.clone(), right.clone()]);
        }
        if let Some(renderer) = &self.renderer {
            renderer.roots(roots);
        }
        roots.extend(self.active.iter().copied().map(Value::Object));
        for task in &self.tasks {
            match task {
                Task::Value(value) => roots.push(value.clone()),
                Task::Members {
                    receiver, owner, ..
                } => roots.extend([Value::Object(*receiver), Value::Object(*owner)]),
                Task::Finish(id) => roots.push(Value::Object(*id)),
            }
        }
        if let Some(read) = &self.awaiting {
            roots.extend([Value::Object(read.receiver), Value::Object(read.owner)]);
        }
    }
    pub(super) fn reads(
        &self,
        receiver: ObjectId,
        owner: ObjectId,
        name: &str,
        entry: bool,
    ) -> bool {
        self.awaiting.as_ref().is_some_and(|read| {
            read.receiver == receiver
                && read.owner == owner
                && read.name.as_ref() == name
                && read.entry == entry
        })
    }
}

impl Vm {
    pub(super) fn begin_object_string(&mut self, root: Value) -> Result<(), RuntimeError> {
        self.begin_forcing(root, FinishAction::ObjectString)
    }
    pub(super) fn begin_forcing(
        &mut self,
        root: Value,
        finish: FinishAction,
    ) -> Result<(), RuntimeError> {
        let frame = self
            .frames
            .last_mut()
            .ok_or_else(|| invalid("object string outside frame"))?;
        if frame.deep_force.is_some() {
            return Err(invalid("overlapping object force continuation"));
        }
        frame.deep_force = Some(ForceRun {
            pending_render_key: None,
            pending_render_comparison: None,
            allow_undefined: true,
            recurse: true,
            finish,
            renderer: None,
            tasks: vec![Task::Value(root.clone())],
            root,
            active: Vec::new(),
            incomplete: BTreeSet::new(),
            awaiting: None,
        });
        Ok(())
    }
    /// Hashing/equality use the same persistent force operation with undefined forbidden.
    pub(super) fn force_value_frame(&mut self, root: Value) -> Result<(), RuntimeError> {
        self.force_frame(root, true)
    }
    pub(super) fn force_stdlib_frame(&mut self, root: Value) -> Result<(), RuntimeError> {
        let annotations = root
            .heap_id()
            .map(|id| self.heap.get(id).map(|object| object.annotations.clone()))
            .transpose()?
            .unwrap_or_default();
        self.force_value_frame(root)?;
        self.frames
            .last_mut()
            .expect("stdlib force")
            .deep_force
            .as_mut()
            .expect("force continuation")
            .tasks
            .extend(annotations.into_iter().rev().map(Task::Value));
        Ok(())
    }
    pub(super) fn force_shallow_frame(&mut self, root: Value) -> Result<(), RuntimeError> {
        self.force_frame(root, false)
    }
    fn force_frame(&mut self, root: Value, recurse: bool) -> Result<(), RuntimeError> {
        if self.frames.len() >= 4096 {
            return Err(RuntimeError::new(
                ErrorKind::Cycle,
                "experiment frame limit exceeded",
            ));
        }
        let environment = self
            .frames
            .last()
            .ok_or_else(|| invalid("force outside frame"))?
            .environment
            .clone();
        self.frames.push(Frame {
            guest_catch: None,
            block: usize::MAX,
            pc: 0,
            base: self.stack.len(),
            environment,
            force: None,
            lambda: false,
            type_run: None,
            type_resolution: None,
            generic_default: None,
            equality_run: None,
            pcf_render: None,
            collection_join: None,
            function_owner: None,
            amendment_end: None,
            deep_force: Some(ForceRun {
                pending_render_key: None,
                pending_render_comparison: None,
                root: root.clone(),
                allow_undefined: false,
                recurse,
                finish: FinishAction::ReturnValue,
                renderer: None,
                tasks: vec![Task::Value(root)],
                active: Vec::new(),
                incomplete: BTreeSet::new(),
                awaiting: None,
            }),
            prepared_hashes: None,
            key_search: None,
            return_to_type: false,
            diagnostic_member: None,
            callsite_hidden: false,
            builders: Vec::new(),
            generator_scopes: Vec::new(),
            module_load: None,
            uri_resolution: None,
            module_receiver: None,
        });
        Ok(())
    }
    pub(super) fn step_deep_force(&mut self) -> Result<(), RuntimeError> {
        let index = self.frames.len() - 1;
        let mut run = self.frames[index]
            .deep_force
            .take()
            .expect("force continuation");
        // Always restore the continuation on failure so fail/cancel can reset its marks.
        let result = self.advance_deep_force(&mut run);
        match result {
            Ok(true) => {}
            Ok(false) | Err(_) => self.frames[index].deep_force = Some(run),
        }
        result.map(|_| ())
    }
    fn advance_deep_force(&mut self, run: &mut ForceRun) -> Result<bool, RuntimeError> {
        if run.awaiting.take().is_some() {
            let value = self.pop()?;
            if run.recurse {
                run.tasks.push(Task::Value(value));
            }
        }
        let Some(task) = run.tasks.pop() else {
            if matches!(run.finish, FinishAction::ReturnValue)
                || matches!(&run.finish, FinishAction::Trace(trace) if trace.acknowledged())
            {
                self.stack.push(run.root.clone());
                return Ok(true);
            }
            let (limit, multiline) = match &run.finish {
                FinishAction::Trace(_) => match self.context.trace_mode {
                    crate::logging::TraceMode::Compact => (1_000_000, false),
                    crate::logging::TraceMode::Pretty => (i32::MAX as usize, true),
                },
                _ => (i32::MAX as usize, false),
            };
            let renderer = run.renderer.get_or_insert_with(|| {
                crate::diagnostic_values::RenderRun::new(run.root.clone(), limit, multiline)
            });
            if let Some(key) = run.pending_render_key.take() {
                renderer.record_key_hash(self.java_hash(&key)?);
            }
            if let Some((left, right)) = run.pending_render_comparison.take() {
                let Value::Bool(equal) = self.pop()? else {
                    return Err(invalid("render key equality result"));
                };
                renderer.record_key_comparison(equal);
                if equal && let (Value::Object(left), Value::Object(right)) = (left, right) {
                    self.heap.key_equalities.insert((left, right));
                }
            }
            if let Some(key) = renderer.take_key_to_prepare() {
                run.pending_render_key = Some(key.clone());
                self.prepare_render_key(key);
                return Ok(false);
            }
            if let Some((left, right)) = renderer.take_key_comparison(&self.heap) {
                run.pending_render_comparison = Some((left.clone(), right.clone()));
                self.begin_equality(left, right, false);
                self.frames
                    .last_mut()
                    .expect("render frame")
                    .equality_run
                    .as_mut()
                    .expect("key comparison")
                    .hide_callsite = false;
                return Ok(false);
            }
            if renderer.step(&self.heap, &self.program.module_name)? {
                let text = run.renderer.take().expect("completed renderer").finish();
                if let FinishAction::Trace(trace) = &mut run.finish {
                    if self.emit_trace(trace, text)? {
                        self.stack.push(run.root.clone());
                        return Ok(true);
                    }
                    return Ok(false);
                }
                self.stack.push(Value::string(text));
                return Ok(true);
            }
            return Ok(false);
        };
        match task {
            Task::Value(Value::Object(id)) => {
                if let Some(pair) = self.heap.get(id)?.pair.clone() {
                    if !self.heap.get(id)?.forced {
                        self.heap.get_mut(id)?.forced = true;
                        run.active.push(id);
                        run.tasks.push(Task::Finish(id));
                        run.tasks.extend(pair.into_iter().rev().map(Task::Value));
                    }
                } else if let Some(reference) = self.heap.get_mut(id)?.reference.as_mut() {
                    if !reference.forced {
                        reference.forced = true;
                        run.tasks
                            .extend(reference.path.iter().rev().cloned().map(Task::Value));
                        run.tasks.push(Task::Value(reference.data.clone()));
                        run.tasks.push(Task::Value(reference.domain.clone()));
                    }
                } else if !self.heap.get(id)?.forced {
                    if run.recurse {
                        self.heap.get_mut(id)?.forced = true;
                    }
                    run.active.push(id);
                    run.tasks.push(Task::Finish(id));
                    let kind = self.heap.get(id)?.collection_kind;
                    if matches!(
                        kind,
                        CollectionKind::List | CollectionKind::Map | CollectionKind::Set
                    ) {
                        for member in collections::ordered_members(&self.heap, id)?
                            .into_iter()
                            .rev()
                        {
                            let key = CacheKey {
                                entry: true,
                                owner: None,
                                name: member.name,
                                method: false,
                            };
                            let value = self
                                .cached_slot(id, &key)?
                                .ok_or_else(|| invalid("strict collection value is not cached"))?;
                            run.tasks.push(Task::Value(value));
                            if kind == CollectionKind::Map {
                                run.tasks.push(Task::Value(member.key));
                            }
                        }
                    } else {
                        run.tasks.push(Task::Members {
                            receiver: id,
                            owner: id,
                            index: 0,
                        });
                    }
                }
            }
            Task::Value(_) => {}
            Task::Finish(id) => {
                if run.incomplete.remove(&id) {
                    self.heap.get_mut(id)?.forced = false;
                }
                if run.active.pop() != Some(id) {
                    return Err(invalid("unbalanced object force markers"));
                }
            }
            Task::Members {
                receiver,
                owner,
                index,
            } => {
                let object = self.heap.get(owner)?;
                if let Some(member) = object.members.get(index).cloned() {
                    run.tasks.push(Task::Members {
                        receiver,
                        owner,
                        index: index + 1,
                    });
                    let deferred_binding = self.in_stdlib_bootstrap() && member.definition()
                        && self.program.blocks.get(member.code).is_some_and(|code|
                            matches!(code.first(), Some(Instruction::Fail(error)) if error.kind == ErrorKind::Unsupported));
                    if deferred_binding {
                        run.incomplete.insert(receiver);
                    }
                    if !deferred_binding
                        && !member.local
                        && !member.method
                        && !member.external()
                        && !member.abstract_member()
                        && !collections::hidden_property(
                            &self.heap,
                            owner,
                            &member.name,
                            member.entry,
                        )?
                    {
                        run.awaiting = Some(ReadBoundary {
                            receiver,
                            owner,
                            name: member.name.clone(),
                            entry: member.entry,
                            stack: self.stack.len(),
                        });
                        // force_key may defer key hashing, so install the boundary first.
                        let frame = self.frames.len() - 1;
                        self.frames[frame].deep_force = Some(std::mem::replace(
                            run,
                            ForceRun {
                                pending_render_key: None,
                                pending_render_comparison: None,
                                allow_undefined: false,
                                recurse: true,
                                finish: FinishAction::ReturnValue,
                                renderer: None,
                                root: Value::Null,
                                tasks: Vec::new(),
                                active: Vec::new(),
                                incomplete: BTreeSet::new(),
                                awaiting: None,
                            },
                        ));
                        let result = self.force_declaration_member(receiver, owner, member);
                        *run = self.frames[frame]
                            .deep_force
                            .take()
                            .expect("force read boundary");
                        result?;
                    }
                } else if let Some(parent) = object.parent {
                    run.tasks.push(Task::Members {
                        receiver,
                        owner: parent,
                        index: 0,
                    });
                }
            }
        }
        Ok(false)
    }

    pub(super) fn reset_force_marks(&mut self, start: usize) {
        for frame in &mut self.frames[start..] {
            if let Some(run) = &mut frame.deep_force {
                for id in run.active.drain(..) {
                    if let Ok(object) = self.heap.get_mut(id) {
                        object.forced = false;
                    }
                }
            }
        }
    }
    pub(super) fn catch_force_undefined(&mut self, innermost_guest: Option<usize>) -> bool {
        let Some(index) = self.frames.iter().rposition(|frame| {
            frame
                .deep_force
                .as_ref()
                .is_some_and(|run| run.allow_undefined && run.awaiting.is_some())
        }) else {
            return false;
        };
        if innermost_guest.is_some_and(|guest| guest > index) {
            return false;
        }
        let read = self.frames[index]
            .deep_force
            .as_ref()
            .expect("force boundary")
            .awaiting
            .as_ref()
            .expect("read");
        // getCachedValue is outside Java's undefined catch. Only an entered
        // doReadMember call owns this boundary, not deferred cache-key hashing.
        if !self.frames[index + 1..].iter().any(|frame| {
            frame.force.as_ref().is_some_and(|force| {
                force.receiver == read.receiver
                    && force.owner == read.owner
                    && force.name == read.name
                    && force.entry == read.entry
            })
        }) {
            return false;
        }
        // Inferred-parent reads convert undefined before it reaches object.force.
        if self.frames[index + 1..].iter().any(|frame| {
            self.program
                .blocks
                .get(frame.block)
                .and_then(|code| code.get(frame.pc.saturating_sub(1)))
                .is_some_and(|instruction| matches!(instruction, Instruction::InferPropertyParent))
        }) {
            return false;
        }
        self.reset_force_marks(index + 1);
        let read = self.frames[index]
            .deep_force
            .as_mut()
            .expect("force boundary")
            .awaiting
            .take()
            .expect("member read");
        let removed = self.frames.split_off(index + 1);
        for frame in removed {
            if let Some(force) = frame.force
                && let Some(key) = force.cache
            {
                let retained = self
                    .frames
                    .iter()
                    .chain(self.suspended.iter().flat_map(|context| &context.frames))
                    .any(|frame| {
                        frame.force.as_ref().is_some_and(|active| {
                            active.receiver == force.receiver && active.cache.as_ref() == Some(&key)
                        })
                    });
                if !retained
                    && let Ok(object) = self.heap.get_mut(force.receiver)
                    && matches!(object.cache.get(&key), Some(LazyState::Evaluating))
                {
                    object.cache.remove(&key);
                }
            }
        }
        self.stack.truncate(read.stack);
        self.frames[index].callsite_hidden = false;
        self.state = State::Running;
        true
    }
}

#[cfg(test)]
mod tests;
