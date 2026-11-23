//! Method target installation owns deprecation effects, independently of invocation values.
use super::*;
use crate::logging::{LogEvent, LogLevel};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ReceiverClass {
    Declared(ObjectId),
    Builtin(classes::BuiltinClass),
}
impl ReceiverClass {
    fn prototype(self) -> Option<ObjectId> {
        match self {
            Self::Declared(id) => Some(id),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
struct Target {
    class: Option<ReceiverClass>,
    declaration: Option<ObjectId>,
}

#[derive(Clone, Default)]
pub(crate) struct CallCache {
    direct: bool,
    megamorphic: bool,
    targets: Vec<Target>,
    head: Option<u64>,
    next_head: u64,
}
impl CallCache {
    #[inline]
    pub(super) fn is_saturated(&self) -> bool {
        !self.direct && !self.megamorphic && self.targets.len() == 3
    }

    #[inline]
    pub(super) fn is_megamorphic(&self) -> bool {
        self.megamorphic
    }

    #[inline]
    pub(super) fn contains_declared_class(&self, class: ObjectId) -> bool {
        !self.direct
            && !self.megamorphic
            && self
                .targets
                .iter()
                .any(|t| t.class == Some(ReceiverClass::Declared(class)))
    }

    #[inline]
    pub(super) fn contains_builtin_class(&self, class: classes::BuiltinClass) -> bool {
        !self.direct
            && !self.megamorphic
            && self
                .targets
                .iter()
                .any(|t| t.class == Some(ReceiverClass::Builtin(class)))
    }

    pub(crate) fn trace(&self, pending: &mut Vec<ObjectId>) {
        for target in &self.targets {
            pending.extend(target.class.and_then(ReceiverClass::prototype));
            pending.extend(target.declaration);
        }
    }
}

/// An implicit invocation keeps the authored caller's cache and source location.
#[derive(Clone)]
pub(super) struct CallSite {
    pub(super) executable: ExecutableOwnerId,
    site: (usize, usize),
    source: Option<Arc<crate::value::RuntimeSource>>,
    span: Option<Span>,
}

pub(super) struct PendingWarning {
    executable: ExecutableOwnerId,
    site: (usize, usize),
    direct: bool,
    target: Target,
    expected_head: Option<u64>,
    new_head: u64,
    previous_targets: Vec<Target>,
    event: Option<Arc<LogEvent>>,
}
impl PendingWarning {
    pub(super) fn roots(&self, roots: &mut Vec<Value>) {
        roots.push(self.executable.root());
        roots.extend(
            self.target
                .class
                .and_then(ReceiverClass::prototype)
                .map(Value::Class),
        );
        roots.extend(self.target.declaration.map(Value::Object));
        for target in &self.previous_targets {
            roots.extend(
                target
                    .class
                    .and_then(ReceiverClass::prototype)
                    .map(Value::Class),
            );
            roots.extend(target.declaration.map(Value::Object));
        }
    }
}

impl Vm {
    fn method_receiver_class(&self, value: &Value) -> Result<ReceiverClass, RuntimeError> {
        Ok(match classes::runtime_class(value, &self.heap)? {
            classes::RuntimeClass::Declared(class) => ReceiverClass::Declared(class),
            classes::RuntimeClass::Builtin(class) => ReceiverClass::Builtin(class),
        })
    }

    /// The replacing Truffle specialization retires its direct-cache chain
    /// before resolveMethod can report a missing method or const violation.
    pub(super) fn retire_saturated_method_cache(
        &mut self,
        receiver: &Value,
        name: &PklString,
    ) -> Result<(), RuntimeError> {
        if name == "apply" && matches!(receiver, Value::Function(_)) {
            return Ok(());
        }
        let frame = self
            .frames
            .last()
            .ok_or_else(|| invalid("method lookup outside frame"))?;
        let site = frame
            .method_call_site
            .as_ref()
            .map_or((frame.block, frame.pc - 1), |site| site.site);
        let owner = frame
            .method_call_site
            .as_ref()
            .map(|site| site.executable)
            .or(frame.environment.executable_owner)
            .or_else(|| self.executable_roots.get(&frame.block).copied());
        let Some(owner) = owner else {
            return Ok(());
        };
        let full = self
            .heap
            .existing_method_call_cache(owner, site)?
            .is_some_and(|cache| !cache.direct && !cache.megamorphic && cache.targets.len() == 3);
        if !full {
            return Ok(());
        }
        let class = self.method_receiver_class(receiver)?;
        let cache = self.heap.method_call_cache(owner, site)?;
        if !cache
            .targets
            .iter()
            .any(|target| target.class == Some(class))
        {
            cache.megamorphic = true;
            cache.head = None;
            cache.targets.clear();
        }
        Ok(())
    }

    pub(super) fn publish_method_call(
        &mut self,
        mut pending: PendingWarning,
    ) -> Result<(), RuntimeError> {
        let cache = self
            .heap
            .method_call_cache(pending.executable, pending.site)?;
        if pending.direct {
            cache.direct = true;
            cache.targets = vec![pending.target];
        } else if cache.head == pending.expected_head {
            // A logger callback can retire the chain and restore the null head.
            // This deliberately compares head identity, not a monotonic revision.
            cache.head = Some(pending.new_head);
            cache.targets = pending.previous_targets;
            cache.targets.push(pending.target);
            cache.megamorphic = false;
        } else if cache
            .targets
            .iter()
            .any(|target| target.class == pending.target.class)
        {
            // Reentrant installation of this same class wins without another warning.
        } else if cache.targets.len() == 3 {
            cache.megamorphic = true;
            cache.head = None;
            cache.targets.clear();
        } else {
            // Truffle retries initialization after failed cache-head publication.
            // Its retry loop does not revisit the indirect-specialization guard.
            pending.expected_head = cache.head;
            pending.previous_targets = cache.targets.clone();
            pending.new_head = cache.next_head;
            cache.next_head = cache
                .next_head
                .checked_add(1)
                .ok_or_else(|| invalid("method cache identities exhausted"))?;
            return self.emit_method_warning(pending);
        }
        Ok(())
    }

    pub(super) fn prepare_method_call(
        &mut self,
        lookup: usize,
        arity: usize,
        after_arguments: bool,
    ) -> Result<(), RuntimeError> {
        let frame = self
            .frames
            .last()
            .ok_or_else(|| invalid("method call outside frame"))?;
        let frame_block = frame.block;
        let function_index = self
            .stack
            .len()
            .checked_sub(arity + 1)
            .ok_or_else(|| invalid("method call operand"))?;
        let Some(Value::Function(function_id)) = self.stack.get(function_index) else {
            return Ok(());
        };
        let object = self.heap.get(*function_id)?;
        if object
            .function
            .as_ref()
            .is_some_and(|function| !function.method)
        {
            // Function.apply has a distinct intrinsic specialization and no ClassMethod.
            return Ok(());
        }
        let declaration = object.method_declaration;
        let has_receiver = object.native_function.is_some()
            || object
                .function
                .as_ref()
                .is_some_and(|function| function.captured_this.is_some())
            || object.environment.this_value.is_some();
        if !has_receiver {
            return Ok(());
        }
        let lookup_instruction = &self.program.blocks[frame_block][lookup];
        let (direct, lexical) = match lookup_instruction {
            Instruction::GetMethodProperty {
                owner_depth, local, ..
            } => {
                let scope = self.scope(*owner_depth)?;
                let owner = self.heap.get(scope.owner)?;
                let Some(class) = owner.class_definition.as_ref() else {
                    // Object local methods resolve an ObjectMember, not ClassMethod.
                    return Ok(());
                };
                let direct = *local
                    || !(class.open || class.abstract_)
                    || declaration
                        .and_then(|(owner, code)| {
                            self.heap
                                .get(owner)
                                .ok()?
                                .members
                                .iter()
                                .find(|member| member.code == code)
                        })
                        .is_some_and(Member::external);
                (direct, direct)
            }
            Instruction::GetSuperMethod(_) | Instruction::GetConstSuperMethod(_) => (true, false),
            _ => (false, false),
        };
        if after_arguments != lexical {
            return Ok(());
        }
        let message = declaration.and_then(|(owner, code)| {
            self.heap
                .get(owner)
                .ok()?
                .member_deprecations
                .get(&code)
                .cloned()
                .map(|message| (code, message))
        });
        // A nondeprecated direct target needs no additional warning state.
        // Virtual calls still count nondeprecated classes toward specialization.
        if direct && message.is_none() {
            return Ok(());
        }
        let object = self.heap.get(*function_id)?;
        let receiver = object
            .native_function
            .as_ref()
            .map(|native| native.receiver.clone())
            .or_else(|| {
                object
                    .function
                    .as_ref()
                    .and_then(|function| function.captured_this.clone())
            })
            .or_else(|| object.environment.this_value.clone())
            .expect("checked method receiver");
        let frame = self.frames.last().expect("method call frame");
        let call_site = frame.method_call_site.as_deref().cloned();
        let site = call_site
            .as_ref()
            .map_or((frame_block, lookup), |site| site.site);
        let class = if direct {
            None
        } else {
            Some(self.method_receiver_class(&receiver)?)
        };
        let executable = match &call_site {
            Some(site) => site.executable,
            None => self.current_executable()?,
        };
        let cache = self.heap.method_call_cache(executable, site)?;
        if cache.direct
            || cache.megamorphic
            || cache.targets.iter().any(|target| target.class == class)
        {
            return Ok(());
        }
        if !direct && cache.targets.len() == 3 {
            // Truffle's default polymorphic cache limit is three receiver classes.
            // The replacing indirect specialization never reports deprecation.
            cache.megamorphic = true;
            cache.head = None;
            cache.targets.clear();
            return Ok(());
        }
        let new_head = cache.next_head;
        cache.next_head = cache
            .next_head
            .checked_add(1)
            .ok_or_else(|| invalid("method cache identities exhausted"))?;
        let mut pending = PendingWarning {
            executable,
            site,
            direct,
            expected_head: cache.head,
            new_head,
            previous_targets: cache.targets.clone(),
            event: None,
            target: Target {
                class,
                declaration: declaration.map(|(owner, _)| owner),
            },
        };
        let Some((code, message)) = message else {
            return self.publish_method_call(pending);
        };
        if self.callback_closing {
            return Err(RuntimeError::new(ErrorKind::Closed, "evaluator is closing"));
        }
        self.query_cache.clear();
        let metadata = &self.program.metadata[code];
        let module = metadata
            .source
            .as_ref()
            .map(|source| &source.module_name)
            .or_else(|| {
                metadata
                    .unit
                    .and_then(|unit| self.program.units.get(unit.0 as usize))
                    .map(|unit| &unit.name)
            })
            .unwrap_or(&self.program.module_name);
        let message = PklString::from("Method `")
            .concat(&PklString::from(module))
            .concat(&metadata.member)
            .concat(&"` is deprecated".into())
            .concat(&if message.is_empty() {
                PklString::default()
            } else {
                PklString::from(": ").concat(&message)
            });
        let frame = if let Some(site) = call_site {
            self.method_log_frame(&site)?
        } else {
            let site = self.current_method_call_site()?;
            self.method_log_frame(&site)?
        };
        pending.event = Some(Arc::new(LogEvent {
            level: LogLevel::Warn,
            message,
            frame,
        }));
        self.emit_method_warning(pending)
    }

    pub(super) fn current_method_call_site(&mut self) -> Result<CallSite, RuntimeError> {
        let executable = self.current_executable()?;
        let frame = self
            .frames
            .last()
            .ok_or_else(|| invalid("missing method source frame"))?;
        let metadata = self.program.metadata.get(frame.block);
        Ok(CallSite {
            executable,
            site: (frame.block, frame.pc - 1),
            source: metadata
                .and_then(|metadata| metadata.source.clone())
                .or_else(|| self.context.primary_source.clone()),
            span: metadata
                .and_then(|metadata| metadata.spans.get(frame.pc - 1))
                .copied()
                .flatten(),
        })
    }

    fn method_log_frame(&self, site: &CallSite) -> Result<crate::logging::LogFrame, RuntimeError> {
        match (&site.source, site.span) {
            (Some(source), Some(span)) => tracing::source_frame(source, span),
            _ => Ok(crate::logging::LogFrame {
                module_uri: "source:unavailable".into(),
                member_name: None,
                source_lines: vec![PklString::default()].into(),
                start_line: 1,
                start_column: 1,
                end_line: 1,
                end_column: 1,
            }),
        }
    }

    fn emit_method_warning(&mut self, pending: PendingWarning) -> Result<(), RuntimeError> {
        let event = pending
            .event
            .clone()
            .ok_or_else(|| invalid("missing deprecation event"))?;
        // BufferedLogger runs before the transformer and delegate, including failures.
        self.log_buffer.extend(event.message.encode_utf16());
        self.log_buffer.push(u16::from(b'\n'));
        if self.begin_log_delivery(event)? {
            return self.publish_method_call(pending);
        }
        self.frames.last_mut().expect("active call frame").warning = Some(pending);
        Ok(())
    }
}

#[cfg(test)]
mod tests;
