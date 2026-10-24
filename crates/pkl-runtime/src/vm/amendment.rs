//! Function amendments defer object construction until the wrapped call returns.
use super::*;
pub(crate) const MIXIN_ID: usize = usize::MAX - 300;
#[derive(Clone, Debug)]
pub enum ParameterTarget {
    Parent {
        kind: crate::heap::CollectionKind,
        has_parent: bool,
        typed: bool,
    },
    Builder {
        amendment_end: Option<usize>,
    },
    Value,
}
#[derive(Clone, Debug)]
pub struct Amendment {
    pub parent: Value,
    pub(super) parameter_count: usize,
    pub(super) initial: bool,
    function_block: usize,
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
    fn amended_function_member(&self, block: usize) -> PklString {
        let metadata = &self.program.metadata[block];
        let member: PklString = if metadata.member.is_empty() {
            metadata
                .source
                .as_ref()
                .map_or_else(
                    || self.program.module_name.to_string(),
                    |source| source.uri.to_string(),
                )
                .into()
        } else {
            metadata.member.clone()
        };
        member.concat(&".<function>".into())
    }

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
                block: if amendment.initial {
                    amendment.function_block
                } else {
                    self.program.amended_function_block
                },
                arity,
                method: false,
                captured_this: None,
            }),
            amendment: Some(amendment),
            environment,
            ..Default::default()
        })))
    }
    pub(super) fn check_object_parameters(
        &self,
        target: ParameterTarget,
    ) -> Result<(), RuntimeError> {
        use crate::heap::CollectionKind;
        let generator = matches!(target, ParameterTarget::Parent { .. });
        if let ParameterTarget::Builder {
            amendment_end: Some(end),
        } = &target
            && self
                .frames
                .last()
                .is_some_and(|frame| frame.amendment_end == Some(*end))
        {
            return Ok(());
        }
        let (id, default_kind, typed_parameter) = match target {
            ParameterTarget::Parent {
                kind,
                has_parent,
                typed,
            } => {
                let value = has_parent.then(|| self.stack.last()).flatten();
                if matches!(
                    value,
                    Some(
                        Value::Function(_)
                            | Value::Bool(_)
                            | Value::Int(_)
                            | Value::Float(_)
                            | Value::Duration(_)
                            | Value::DataSize(_)
                            | Value::Bytes(_)
                            | Value::IntSeq(_)
                            | Value::TypeAlias(_)
                            | Value::String(_)
                    )
                ) {
                    return Ok(());
                }
                let id = match value {
                    Some(Value::Class(class)) => {
                        if self.class_external(*class)? {
                            return Ok(());
                        }
                        Some(self.heap.get_class(*class)?.prototype)
                    }
                    _ => value.and_then(Value::heap_id),
                };
                (id, kind, typed)
            }
            ParameterTarget::Builder { .. } => (
                self.frames
                    .last()
                    .and_then(|frame| frame.builders.last())
                    .copied(),
                CollectionKind::Dynamic,
                true,
            ),
            ParameterTarget::Value => (
                self.stack.last().and_then(Value::heap_id),
                CollectionKind::Dynamic,
                true,
            ),
        };
        let (kind, typed_object) = if let Some(id) = id {
            if let Ok(object) = self.heap.get(id) {
                (
                    object.collection_kind,
                    collections::typed_class(&self.heap, id)?.is_some() || object.is_module,
                )
            } else {
                (default_kind, false)
            }
        } else {
            (default_kind, false)
        };
        if !generator && kind == CollectionKind::Dynamic && !typed_object {
            return Ok(());
        }
        if !typed_parameter {
            return Err(RuntimeError::new(
                ErrorKind::Unsupported,
                "Untyped parameterized object generators encounter an upstream internal error.",
            ));
        }
        let message = match kind {
            CollectionKind::Listing => {
                "Expected a function that returns a `Listing`, but got a `Listing`."
            }
            CollectionKind::Mapping => {
                "Expected a function that returns a `Mapping`, but got a `Mapping`."
            }
            _ => "Expected a function that returns an object, but got an object.",
        };
        Err(RuntimeError::new(ErrorKind::Type, message))
    }
    pub(super) fn prepare_function_amendment(
        &mut self,
        body: usize,
        arity: usize,
    ) -> Result<(), RuntimeError> {
        self.unwrap_amendment_default()?;
        let Some(Value::Function(parent)) = self.stack.last() else {
            self.frames.last_mut().expect("amendment frame").pc = body;
            return Ok(());
        };
        let object = self.heap.get(*parent)?;
        let count = object
            .function
            .as_ref()
            .map(|function| function.arity)
            .or_else(|| {
                object
                    .native_function
                    .as_ref()
                    .map(|function| match function.name.as_ref() {
                        "Reference" => 3,
                        "containsKey" | "getOrNull" | "contains" => 1,
                        _ => 0,
                    })
            })
            .ok_or_else(|| invalid("amended function arity"))?;
        if count != arity {
            return Err(RuntimeError::new(
                ErrorKind::Type,
                format!("Expected a function with {arity} parameters, but got {count}."),
            ));
        }
        Ok(())
    }
    pub(super) fn begin_function_amendment(
        &mut self,
        end: usize,
        function: Option<(usize, usize)>,
    ) -> Result<(), RuntimeError> {
        self.unwrap_amendment_default()?;
        if !matches!(self.stack.last(), Some(Value::Function(_))) {
            return Ok(());
        }
        let parent = self.pop()?;
        let frame = self.frames.last().unwrap();
        let amendment = Amendment {
            parent,
            parameter_count: function.map_or(0, |(_, arity)| arity),
            initial: true,
            function_block: function
                .map_or(self.program.amended_function_block, |(block, _)| block),
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
                        member: self.amended_function_member(frame.block),
                    })
            }),
        };
        let environment = frame.environment.clone();
        let value = self.amended_function(amendment, environment)?;
        self.stack.push(value);
        self.frames.last_mut().unwrap().pc = end + 1;
        Ok(())
    }
    // Literal nodes redispatch on VmNull.getDefaultValue before choosing the
    // object or function amendment specialization. Reading the default does
    // not execute it, including when a wrapped function returns another null.
    fn unwrap_amendment_default(&mut self) -> Result<(), RuntimeError> {
        while let Some(Value::NullWithDefault(id)) = self.stack.last() {
            let value = self.heap.null_default(*id)?;
            *self.stack.last_mut().expect("amendment parent") = value;
        }
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
        let captured = object.environment.locals.len();
        let arguments = if let Some(arguments) = self
            .frames
            .last_mut()
            .expect("amended frame")
            .amendment_arguments
            .take()
        {
            arguments
        } else {
            self.frames
                .last()
                .expect("amended frame")
                .environment
                .locals[captured..]
                .to_vec()
        };
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
        let mut environment = object.environment.clone();
        if amendment.initial && amendment.parameter_count > 0 {
            let frame = self.frames.last().expect("amendment invocation");
            environment.locals = frame.environment.locals.clone();
            environment.local_owners = frame.environment.local_owners.clone();
            environment.copy_locals = frame.environment.copy_locals.clone();
        }
        amendment.initial = false;
        self.unwrap_amendment_default()?;
        if matches!(self.stack.last(), Some(Value::Function(_))) {
            amendment.parent = self.pop()?;
            let value = self.amended_function(amendment, environment)?;
            self.stack.push(value);
            return Ok(());
        }
        let diagnostic_member = self.amended_function_member(amendment.block);
        let frame = self.frames.last_mut().unwrap();
        frame.block = amendment.block;
        frame.pc = amendment.start;
        frame.amendment_end = Some(amendment.end);
        frame.environment = environment;
        frame.diagnostic_member = Some(diagnostic_member);
        Ok(())
    }
}

impl Compiler<'_> {
    /// Resolve the first wrapper's parameter types only after selecting the
    /// function specialization. The emitted function uses the ordinary typed
    /// local continuation; subsequent curried wrappers use the forwarding block.
    pub(super) fn compile_function_amendment(
        &mut self,
        parameters: &[crate::syntax::Parameter],
        output: &mut Code,
    ) -> Result<usize, RuntimeError> {
        let prepare = if parameters.is_empty() {
            None
        } else {
            let index = output.len();
            output.push(Instruction::PrepareFunctionAmendment {
                body: 0,
                arity: parameters.len(),
            });
            Some(index)
        };
        let function = if parameters.is_empty() {
            None
        } else {
            let previous = self.member.clone();
            self.member = previous.concat(&".<function>".into());
            let mut code = Code::default();
            for (index, parameter) in parameters.iter().enumerate() {
                let depth = parameters.len() - 1 - index;
                let (check, span) = if let Some(ty) = &parameter.ty {
                    let check = self
                        .resolve_expression_type(ty, output)
                        .unwrap_or(TypeCheck::Unknown);
                    (check, self.type_source_span(ty))
                } else {
                    (TypeCheck::Unknown, parameter.span)
                };
                code.push(Instruction::LoadLocal(depth));
                code.push_at(
                    Instruction::BindLocal {
                        depth: Some(depth),
                        ty: check,
                    },
                    span,
                );
            }
            code.push(Instruction::InvokeAmendedParent);
            code.push(Instruction::ContinueFunctionAmendment);
            code.push(Instruction::Return);
            let block = self.block(code, None)?;
            self.member = previous;
            Some((block, parameters.len()))
        };
        let index = output.len();
        output.push(Instruction::BeginFunctionAmendment { end: 0, function });
        if let Some(prepare) = prepare {
            output[prepare] = Instruction::PrepareFunctionAmendment {
                body: output.len(),
                arity: parameters.len(),
            };
        }
        Ok(index)
    }
}
