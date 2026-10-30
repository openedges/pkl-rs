//! Full Java error bytes retain the logical declaration owner and source section.
use std::sync::Arc;

use pkl_runtime::parser::parse_module;
use pkl_runtime::resolver::resolve_module;
use pkl_runtime::value::{RuntimeError, RuntimeSource, Value};
use pkl_runtime::vm::{Vm, VmPoll, compile_module};

#[test]
fn builtin_constraint_keeps_its_logical_module_owner() {
    let actual = pkl_runtime::evaluation::scalar_expression(
        include_str!("../../../tests/fixtures/m2-reference-frame-owner/input.pkl"),
        "file:///Users/cody.ij.hwang/Projects/pkl-rs.stage42/tests/fixtures/m2/reference-stage10-access-invalid.pkl",
        "reference-stage10-access-invalid",
        "result",
    )
    .unwrap_err();
    assert_eq!(
        actual.as_bytes(),
        include_bytes!("../../../tests/fixtures/m2-reference-frame-owner/java.stderr.bin")
    );
}

fn finish(vm: &mut Vm) -> Result<Value, RuntimeError> {
    for _ in 0..10_000 {
        match vm.poll(1) {
            VmPoll::Yielded => {}
            VmPoll::Ready(result) => return result,
            event => panic!("unexpected effect: {event:?}"),
        }
    }
    panic!("evaluation did not terminate")
}

#[test]
fn inherited_primary_declaration_remains_primary_with_a_foreign_value_body() {
    // This checks the source-ownership contract of compiled metadata. Detached
    // constraint source sections and value roots can differ from the logical
    // property declaration; None on that declaration still means primary.
    let text = "module primary\nopen class Base { value: Int(this > 0) = 1 }\nclass Child extends Base { value = -1 }\nresult = (new Child {}).value\n";
    let parsed = parse_module(text).unwrap();
    let mut program = (*compile_module(&resolve_module(&parsed).unwrap()).unwrap()).clone();
    let constraint_source = Arc::new(RuntimeSource {
        text: text.into(),
        uri: "file:///declarations.pkl".into(),
        module_name: "primary".into(),
    });
    let value_source = Arc::new(RuntimeSource {
        uri: "file:///value.pkl".into(),
        module_name: "foreign".into(),
        ..constraint_source.as_ref().clone()
    });
    let mut primary_declarations = 0;
    let mut foreign_bodies = 0;
    for metadata in &mut program.metadata {
        if metadata.source.is_some() {
            continue;
        }
        metadata.source = Some(constraint_source.clone());
        let name = metadata.member.as_str();
        if name == Some("#Base.value") && metadata.property_typecheck.is_some() {
            metadata.source = None;
            primary_declarations += 1;
        }
        if name == Some("#Child.value") && metadata.value_span.is_some() {
            metadata.source = Some(value_source.clone());
            foreign_bodies += 1;
        }
    }
    assert_eq!(primary_declarations, 1);
    assert_eq!(foreign_bodies, 1);
    let entry = program.entry;
    let mut vm = Vm::new(Arc::new(program));
    vm.start(entry, vec![]).unwrap();
    let module = finish(&mut vm).unwrap().as_object().unwrap();
    for _ in 0..2 {
        vm.start_member(module, "result").unwrap();
        let error = finish(&mut vm).unwrap_err();
        let first = &error.frames[0];
        assert_eq!(first.member.as_str(), Some("#Base.value"));
        let source = first.source.as_ref().unwrap();
        assert_eq!(source.module_name, "primary");
        assert_eq!(source.uri, "file:///declarations.pkl");
        assert!(error.frames.iter().any(|frame| {
            frame
                .source
                .as_ref()
                .is_some_and(|source| source.module_name == "foreign")
        }));
    }
    vm.close();
    assert_eq!(vm.collect(&[]).live, 0);
}
