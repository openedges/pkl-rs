//! The Null constructor keeps its default lazy. This binding's Object guard
//! does not claim general abstract Object type-resolution support.
use super::*;
use crate::heap::CollectionKind;

pub(super) fn constructor_block(
    blocks: &mut Vec<Vec<Instruction>>,
    metadata: &mut Vec<BlockMetadata>,
) -> usize {
    let source = crate::stdlib_sources::base();
    let declaration = "external const function Null(defaultValue: Object | Function<Object>): Null";
    let start = source
        .text
        .find(declaration)
        .expect("pinned Null constructor");
    let parameter = start + declaration.find("Object |").unwrap();
    let span = Span::new(parameter, parameter + "Object | Function<Object>".len());
    let block = blocks.len();
    blocks.push(vec![
        Instruction::LoadLocal(0),
        Instruction::CheckNullDefault,
        Instruction::WrapNullableDefault { defined: true },
        Instruction::Return,
    ]);
    metadata.push(BlockMetadata {
        source: Some(source),
        member: "#Null".into(),
        declaration: Some(Span::new(start, start + declaration.len())),
        spans: vec![None, Some(span), None, None],
        ..Default::default()
    });
    block
}

impl Vm {
    pub(super) fn null_constructor(&mut self) -> Value {
        Value::Function(self.heap.allocate(Object {
            function: Some(Function {
                block: self.program.null_constructor_block,
                arity: 1,
                method: true,
                captured_this: None,
            }),
            ..Default::default()
        }))
    }

    #[cfg_attr(target_arch = "wasm32", inline(never))]
    pub(super) fn check_null_default(&self) -> Result<(), RuntimeError> {
        let value = self
            .stack
            .last()
            .ok_or_else(|| invalid("Null default operand"))?;
        let accepted = match value {
            Value::Function(_) => true,
            Value::Object(id) => {
                let object = self.heap.get(*id)?;
                object.pair.is_none()
                    && matches!(
                        object.collection_kind,
                        CollectionKind::Dynamic | CollectionKind::Listing | CollectionKind::Mapping
                    )
            }
            _ => false,
        };
        if accepted {
            return Ok(());
        }
        let frame = self.frames.last().expect("Null argument check");
        let span =
            self.program.metadata[frame.block].spans[frame.pc - 1].expect("Null argument span");
        let diagnostic = crate::diagnostic::TypeDiagnostic::Union {
            span,
            value: crate::diagnostic_values::render_string(
                value,
                &self.heap,
                &self.program.module_name,
                80,
            )?,
            actual_type: crate::diagnostic_values::type_name(
                value,
                &self.heap,
                &self.program.module_name,
            )?,
            is_null: value.is_null(),
            children: Vec::new(),
        };
        let mut error = RuntimeError::new(ErrorKind::Type, "Type constraint violated.");
        error.span = Some(span);
        error.type_diagnostic = Some(Box::new(diagnostic));
        Err(error)
    }
}
