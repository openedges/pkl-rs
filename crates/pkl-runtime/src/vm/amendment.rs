//! Function amendments defer object construction until the wrapped call returns.
use super::*;
pub(crate) const MIXIN_ID: usize = usize::MAX - 300;
#[derive(Clone, Debug)]
pub struct Amendment {
    pub parent: Value,
    block: usize,
    start: usize,
    end: usize,
    pub(super) origin: Option<RuntimeFrame>,
}
pub(super) fn mixin_type(argument: TypeCheck) -> TypeCheck {
    TypeCheck::NamedAlias {
        declaration: MIXIN_ID,
        default_origin: None,
        name: "Mixin".into(),
        arguments: vec![argument.clone()],
        expanded: Box::new(TypeCheck::Function {
            parameters: Some(vec![argument.clone()]),
            result: Box::new(argument),
            syntax: true,
        }),
    }
}
pub(super) fn blocks(
    blocks: &mut Vec<Vec<Instruction>>,
    metadata: &mut Vec<BlockMetadata>,
) -> (usize, usize) {
    let amend = blocks.len();
    blocks.push(vec![
        Instruction::InvokeAmendedParent,
        Instruction::ContinueFunctionAmendment,
        Instruction::Return,
    ]);
    metadata.push(BlockMetadata::default());
    let identity = blocks.len();
    blocks.push(vec![
        Instruction::LoadLocal(0),
        Instruction::CheckMixinArgument,
        Instruction::Return,
    ]);
    metadata.push(BlockMetadata::default());
    (amend, identity)
}
impl Vm {
    pub(super) fn identity_mixin(&mut self, ty: TypeCheck, origin: Option<RuntimeFrame>) -> Value {
        Value::Function(
            self.heap.allocate(Object {
                function: Some(Function {
                    block: self.program.identity_mixin_block,
                    arity: 1,
                    method: false,
                    captured_this: None,
                }),
                mixin: Some((ty, origin)),
                environment: self
                    .frames
                    .last()
                    .map(|f| f.environment.clone())
                    .unwrap_or_default(),
                ..Default::default()
            }),
        )
    }
    pub(super) fn check_mixin_argument(&mut self) -> Result<(), RuntimeError> {
        let id = self
            .frames
            .last()
            .and_then(|f| f.function_owner)
            .ok_or_else(|| invalid("mixin invocation"))?;
        let (ty, origin) = self
            .heap
            .get(id)?
            .mixin
            .clone()
            .ok_or_else(|| invalid("mixin metadata"))?;
        let value = self.pop()?;
        self.begin_type(
            ty,
            value,
            TypeDestination::Stack,
            origin.into_iter().collect(),
        )
    }
    fn amended_function(
        &mut self,
        amendment: Amendment,
        environment: Environment,
    ) -> Result<Value, RuntimeError> {
        let Value::Function(parent) = amendment.parent else {
            return Err(invalid("amended parent function"));
        };
        let object = self.heap.get(parent)?;
        let arity = object
            .function
            .as_ref()
            .map(|f| f.arity)
            .or_else(|| {
                object
                    .native_function
                    .as_ref()
                    .map(|f| match f.name.as_ref() {
                        "Reference" => 3,
                        "containsKey" | "getOrNull" | "contains" => 1,
                        _ => 0,
                    })
            })
            .ok_or_else(|| invalid("amended function arity"))?;
        Ok(Value::Function(self.heap.allocate(Object {
            function: Some(Function {
                block: self.program.amended_function_block,
                arity,
                method: false,
                captured_this: None,
            }),
            amendment: Some(amendment),
            environment,
            ..Default::default()
        })))
    }
    pub(super) fn begin_function_amendment(&mut self, end: usize) -> Result<(), RuntimeError> {
        if !matches!(self.stack.last(), Some(Value::Function(_))) {
            return Ok(());
        }
        let parent = self.pop()?;
        let frame = self.frames.last().unwrap();
        let amendment = Amendment {
            parent,
            block: frame.block,
            start: frame.pc,
            end,
            origin: self.program.metadata.get(frame.block).and_then(|metadata| {
                metadata
                    .spans
                    .get(end)
                    .copied()
                    .flatten()
                    .map(|span| RuntimeFrame {
                        source: metadata.source.clone(),
                        span,
                        member: metadata.member.concat(&".<function>".into()),
                    })
            }),
        };
        let environment = frame.environment.clone();
        let value = self.amended_function(amendment, environment)?;
        self.stack.push(value);
        self.frames.last_mut().unwrap().pc = end + 1;
        Ok(())
    }
    pub(super) fn invoke_amended_parent(&mut self) -> Result<(), RuntimeError> {
        let frame = self.frames.last().unwrap();
        let owner = frame
            .function_owner
            .ok_or_else(|| invalid("amended function owner"))?;
        let object = self.heap.get(owner)?;
        let parent = object
            .amendment
            .as_ref()
            .ok_or_else(|| invalid("amended function metadata"))?
            .parent
            .clone();
        let Value::Function(parent) = parent else {
            return Err(invalid("amended parent function"));
        };
        let arity = object.function.as_ref().unwrap().arity;
        let arguments = frame.environment.locals[frame.environment.locals.len() - arity..].to_vec();
        self.invoke_function(parent, arguments, false)
    }
    pub(super) fn continue_function_amendment(&mut self) -> Result<(), RuntimeError> {
        let owner = self
            .frames
            .last()
            .and_then(|f| f.function_owner)
            .ok_or_else(|| invalid("amended function owner"))?;
        let object = self.heap.get(owner)?;
        let mut amendment = object
            .amendment
            .clone()
            .ok_or_else(|| invalid("amended function metadata"))?;
        let environment = object.environment.clone();
        if matches!(self.stack.last(), Some(Value::Function(_))) {
            amendment.parent = self.pop()?;
            let value = self.amended_function(amendment, environment)?;
            self.stack.push(value);
            return Ok(());
        }
        let frame = self.frames.last_mut().unwrap();
        frame.block = amendment.block;
        frame.pc = amendment.start;
        frame.amendment_end = Some(amendment.end);
        frame.environment = environment;
        frame.diagnostic_member = Some(
            self.program.metadata[amendment.block]
                .member
                .concat(&".<function>".into()),
        );
        Ok(())
    }
}
