//! Generic operand values and Java frame-slot representation have distinct lifetimes.
use super::*;
use std::ops::{Deref, DerefMut};

/// Bytecode operands are generic Object results. Physical locals may instead
/// contain an unboxed number; every load onto this stack performs valueOf.
#[derive(Default)]
pub(super) struct OperandStack(Vec<Value>);
impl OperandStack {
    pub(super) fn split_off(&mut self, at: usize) -> Vec<Value> {
        self.0.split_off(at)
    }
    pub(super) fn push(&mut self, value: Value) {
        self.0.push(value.boxed());
    }
    pub(super) fn extend(&mut self, values: impl IntoIterator<Item = Value>) {
        self.0.extend(values.into_iter().map(Value::boxed));
    }
}
impl Deref for OperandStack {
    type Target = Vec<Value>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl DerefMut for OperandStack {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SlotKind {
    Long,
    Double,
    Boolean,
    Object,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum SlotPolicy {
    Adaptive,
    Fixed(SlotKind),
}

/// Each iteration has new physical storage but shares the generator AST's
/// descriptor. Saved outer activations remain roots while a child suspends.
pub(super) struct GeneratorScope {
    locals: Vec<Value>,
    local_owners: BTreeMap<usize, (ExecutableOwnerId, (usize, usize))>,
    copy_locals: BTreeSet<usize>,
    descriptor: ExecutableOwnerId,
    bindings: std::ops::Range<usize>,
}
impl GeneratorScope {
    pub(super) fn roots(&self, roots: &mut Vec<Value>) {
        roots.extend(self.locals.iter().cloned());
        roots.extend(self.local_owners.values().map(|(owner, _)| owner.root()));
        roots.push(self.descriptor.root());
    }
}
impl SlotPolicy {
    pub(super) fn from_type(mut ty: &TypeCheck) -> Self {
        loop {
            ty = match ty.unaliased() {
                TypeCheck::Constrained { base, .. } => base,
                TypeCheck::Int | TypeCheck::IntegerAlias { .. } => {
                    return Self::Fixed(SlotKind::Long);
                }
                TypeCheck::Float => return Self::Fixed(SlotKind::Double),
                TypeCheck::Boolean => return Self::Fixed(SlotKind::Boolean),
                TypeCheck::Unknown
                | TypeCheck::Any
                | TypeCheck::NonNull
                | TypeCheck::Number
                | TypeCheck::Nullable(_)
                | TypeCheck::Union {
                    string_literals: false,
                    ..
                }
                | TypeCheck::Parameter { .. } => return Self::Adaptive,
                _ => return Self::Fixed(SlotKind::Object),
            };
        }
    }
}

impl Vm {
    pub(super) fn bind_local(
        &mut self,
        depth: Option<usize>,
        policy: SlotPolicy,
        value: Value,
    ) -> Result<(), RuntimeError> {
        let mut owner = self.current_executable()?;
        let frame = self.frames.last().expect("local frame");
        let index = match depth {
            Some(depth) => frame
                .environment
                .locals
                .len()
                .checked_sub(depth + 1)
                .ok_or_else(|| invalid("invalid parameter slot"))?,
            None => frame.environment.locals.len(),
        };
        let mut site = (frame.block, frame.pc - 1);
        if let Some(scope) = frame.generator_scopes.last()
            && scope.bindings.contains(&index)
        {
            owner = scope.descriptor;
            site = (usize::MAX, index);
        }
        let kind = match policy {
            SlotPolicy::Fixed(kind) => kind,
            SlotPolicy::Adaptive => {
                let actual = match value {
                    Value::Int(_) => SlotKind::Long,
                    Value::Float(_) => SlotKind::Double,
                    Value::Bool(_) => SlotKind::Boolean,
                    _ => SlotKind::Object,
                };
                match self.heap.local_kind(owner, site)? {
                    None => actual,
                    Some(previous) if previous == actual => actual,
                    Some(_) => SlotKind::Object,
                }
            }
        };
        self.heap.set_local_kind(owner, site, kind)?;
        let value = match (kind, value) {
            (SlotKind::Long, Value::Int(value)) => Value::Int(value.unboxed()),
            (SlotKind::Double, Value::Float(value)) => Value::Float(value.unboxed()),
            (_, value) => value.boxed(),
        };
        let environment = &mut self.frames.last_mut().expect("local frame").environment;
        if depth.is_some() {
            environment.locals[index] = value;
        } else {
            environment.locals.push(value);
        }
        environment.local_owners.insert(index, (owner, site));
        environment.copy_locals.insert(index);
        Ok(())
    }

    /// A materialized activation can still contain a primitive after the shared
    /// descriptor has become Object. copyLocals boxes that physical value then;
    /// it cannot recover the reference that was discarded when the slot was set.
    pub(super) fn copy_captured_locals(
        &mut self,
        environment: &mut Environment,
        target: ExecutableOwnerId,
    ) -> Result<(), RuntimeError> {
        for &index in &environment.copy_locals {
            let &(owner, slot) = environment
                .local_owners
                .get(&index)
                .ok_or_else(|| invalid("missing copied local descriptor"))?;
            let kind = self
                .heap
                .local_kind(owner, slot)?
                .ok_or_else(|| invalid("missing copied local kind"))?;
            let value = environment
                .locals
                .get_mut(index)
                .ok_or_else(|| invalid("invalid captured local slot"))?;
            if kind == SlotKind::Object {
                *value = value.clone().boxed();
            }
            let slot = (usize::MAX, index);
            self.heap.set_local_kind(target, slot, kind)?;
            environment.local_owners.insert(index, (target, slot));
        }
        Ok(())
    }

    pub(super) fn enter_for_scope(&mut self, bindings: usize) -> Result<(), RuntimeError> {
        let owner = self.current_executable()?;
        let frame = self.frames.last().expect("generator frame");
        let site = ExecutionSite::Generator {
            block: frame.block,
            instruction: frame.pc - 1,
        };
        let start = frame.environment.locals.len();
        let mut environment = frame.environment.clone();
        let descriptor = self.heap.executable_child(owner, site)?;
        self.copy_captured_locals(&mut environment, descriptor)?;
        let frame = self.frames.last_mut().expect("generator frame");
        let scope = GeneratorScope {
            locals: std::mem::replace(&mut frame.environment.locals, environment.locals),
            local_owners: std::mem::replace(
                &mut frame.environment.local_owners,
                environment.local_owners,
            ),
            copy_locals: frame.environment.copy_locals.clone(),
            descriptor,
            bindings: start..start + bindings,
        };
        frame.generator_scopes.push(scope);
        Ok(())
    }

    pub(super) fn leave_for_scope(&mut self) -> Result<(), RuntimeError> {
        let frame = self.frames.last_mut().expect("generator frame");
        let scope = frame
            .generator_scopes
            .pop()
            .ok_or_else(|| invalid("missing generator activation"))?;
        frame.environment.locals = scope.locals;
        frame.environment.local_owners = scope.local_owners;
        frame.environment.copy_locals = scope.copy_locals;
        Ok(())
    }
}
