//! Function.applyToList uses the ordinary function call frame and argument checks.
use super::*;
use crate::heap::CollectionKind;

#[cfg_attr(target_arch = "wasm32", inline(never))]
pub(super) fn blocks(
    blocks: &mut Vec<Vec<Instruction>>,
    metadata: &mut Vec<BlockMetadata>,
) -> BTreeMap<(Arc<str>, Arc<str>), usize> {
    let source = crate::stdlib_sources::base();
    let declaration = "external function applyToList(argumentList: List<Any>): Result";
    let start = source
        .text
        .find(declaration)
        .expect("pinned Function.applyToList");
    let list_start = start + declaration.find("List<Any>").unwrap();
    let any_start = list_start + "List<".len();
    let block = blocks.len();
    blocks.push(vec![
        Instruction::LoadLocal(0),
        Instruction::CheckType(Box::new(TypeCheck::ParameterizedCollection {
            kind: CollectionKind::List,
            arguments: vec![(TypeCheck::Any, Span::new(any_start, any_start + 3))],
        })),
        Instruction::Primitive(Operation::FunctionApplyToList),
        Instruction::Return,
    ]);
    metadata.push(BlockMetadata {
        source: Some(source),
        member: "#Function.applyToList".into(),
        declaration: Some(Span::new(start, start + declaration.len())),
        spans: vec![
            None,
            Some(Span::new(list_start, list_start + 9)),
            None,
            None,
        ],
        ..Default::default()
    });
    BTreeMap::from([(("Function".into(), "applyToList".into()), block)])
}

impl Vm {
    #[cfg_attr(target_arch = "wasm32", inline(never))]
    pub(super) fn function_apply_to_list(&mut self, receiver: Value) -> Result<(), RuntimeError> {
        let Value::Function(function) = receiver else {
            return Err(invalid("Function.applyToList receiver"));
        };
        let list = self.pop()?.as_object()?;
        let arguments = collections::ordered_members(&self.heap, list)?
            .iter()
            .map(|member| {
                collections::cached_render_value(&self.heap, list, member)?
                    .ok_or_else(|| invalid("uncached strict function argument"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        // Java dispatches to the original call target. It owns arity checks,
        // captured this/locals, amended forwarding and parameter diagnostics.
        // Its ordinary frame also roots the arguments across Host suspension.
        self.invoke_function(function, arguments, false)
    }
}
