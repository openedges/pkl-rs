//! Explicit ModuleKey for lower-level VM tests that supply parsed source.
//! This leaves their existing instruction diagnostic metadata unchanged.
use pkl_runtime::{modules::ModuleIdentity, value::RuntimeSource, vm::Program};
use std::sync::Arc;

pub fn register(program: &mut Program, source: &str) {
    if !program
        .blocks
        .iter()
        .flatten()
        .any(|instruction| matches!(instruction, pkl_runtime::vm::Instruction::ImportModule(_)))
    {
        return;
    }
    program
        .register_primary_source(
            Arc::new(RuntimeSource {
                text: source.into(),
                uri: "file:///runtime-test/main.pkl".into(),
                module_name: program.module_name.clone(),
                line_anchors: false,
            }),
            ModuleIdentity {
                original_uri: "file:///runtime-test/main.pkl".into(),
                resolved_uri: "file:///runtime-test/main.pkl".into(),
            },
        )
        .unwrap();
}
