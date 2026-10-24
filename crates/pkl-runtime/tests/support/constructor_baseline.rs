//! Initialize canonical constructor parents before measuring user-graph deltas.
//! This is a test-only baseline, not production eager initialization or a change
//! to authored member evaluation. Successful constructor sites retain their
//! empty parents for the lifetime of the code, as Java GetParentForTypeNode does.
use std::sync::Arc;

use pkl_runtime::heap::CollectionKind;
use pkl_runtime::resolver::ResolvedModule;
use pkl_runtime::source::Span;
use pkl_runtime::value::RuntimeError;
use pkl_runtime::vm::{Instruction, Program, TypeArgumentSource, TypeCheck};

pub fn compile_module_with_constructor_baseline(
    resolved: &ResolvedModule,
) -> Result<Arc<Program>, RuntimeError> {
    let mut program = pkl_runtime::vm::compile_module(resolved)?;
    let program_mut = Arc::make_mut(&mut program);
    let block = &mut program_mut.blocks[program_mut.entry];
    let terminal = block.len() - 1;
    assert!(matches!(block[terminal], Instruction::Return));
    // Preserve every authored instruction index and jump target. The completed
    // module remains on the stack while three independent constructor sites
    // publish their canonical parents; none of its lazy members are evaluated.
    block[terminal] = Instruction::Jump(terminal + 1);
    for (kind, name) in [
        (CollectionKind::Dynamic, "Dynamic"),
        (CollectionKind::Listing, "Listing"),
        (CollectionKind::Mapping, "Mapping"),
    ] {
        block.push(Instruction::GenericConstructorParent {
            ty: TypeCheck::Collection(kind),
            source: TypeArgumentSource {
                span: Span::default(),
                text: name.into(),
            },
            cache: true,
        });
        block.push(Instruction::Pop);
    }
    block.push(Instruction::Return);
    program_mut.metadata[program_mut.entry]
        .spans
        .resize(block.len(), None);
    Ok(program)
}
