//! Native ownership/scheduler assertions complement the pinned Java raw cases.
use super::*;
use crate::host::{HostPoll, HostSession};
use crate::parser::parse_module;
use crate::resolver::resolve_module;

fn program(source: &str) -> Arc<Program> {
    compile_module(&resolve_module(&parse_module(source).unwrap()).unwrap()).unwrap()
}

fn ready(vm: &mut Vm) -> Result<Value, RuntimeError> {
    for _ in 0..20_000 {
        match vm.poll(1) {
            VmPoll::Ready(result) => return result,
            VmPoll::Yielded => {
                vm.collect(&[]);
            }
            event => panic!("unexpected event: {event:?}"),
        }
    }
    panic!("evaluation failed to settle")
}

fn setup(source: &str) -> (Vm, Value) {
    let program = program(source);
    let mut vm = Vm::new(program.clone());
    vm.start(program.entry, vec![]).unwrap();
    let module = ready(&mut vm).unwrap();
    (vm, module)
}

fn member(vm: &mut Vm, receiver: &Value, name: &str) -> Result<Value, RuntimeError> {
    vm.start_member(receiver.as_object().unwrap(), name)
        .unwrap();
    ready(vm)
}

#[test]
fn class_and_alias_keep_annotation_prefix_and_identity_after_failure_and_gc() {
    for declaration in ["class Target { value = 1 }", "typealias Target = Int"] {
        let source = format!(
            r#"
class Marker extends Annotation {{ captured: Dynamic }}
const payload = new Dynamic {{ number = 42 }}
@Marker {{ captured = payload }}
@Int
{declaration}
scalar = 0
"#
        );
        let (mut vm, module) = setup(&source);
        assert_eq!(
            member(&mut vm, &module, "Target").unwrap_err().message,
            "Expected an annotation class."
        );
        let target = member(&mut vm, &module, "Target").unwrap();
        let id = target.heap_id().unwrap();
        let annotations = vm.heap.get(id).unwrap().annotations.clone();
        assert_eq!(annotations.len(), 1);
        let annotation = annotations[0].as_object().unwrap();
        assert!(vm.heap.get(annotation).unwrap().cache.is_empty());
        assert_eq!(member(&mut vm, &module, "scalar"), Ok(Value::Int(0)));
        vm.collect(&[]);
        assert!(vm.heap.get(annotation).is_ok());
        let captured = member(&mut vm, &annotations[0], "captured").unwrap();
        assert_eq!(member(&mut vm, &captured, "number"), Ok(Value::Int(42)));
        assert_eq!(member(&mut vm, &module, "Target"), Ok(target));
        if declaration.starts_with("typealias") {
            assert!(!vm.heap.get(id).unwrap().type_alias_initialized);
        } else {
            assert!(vm.heap.get(id).unwrap().class_initializing);
            assert!(vm.heap.get(id).unwrap().members.is_empty());
        }
        vm.close();
        assert_eq!(vm.heap_stats().slots, 0);
    }
}

#[test]
fn failed_member_annotation_lists_do_not_publish_or_retain_temporary_objects() {
    let (mut vm, module) = setup(
        r#"
class Marker extends Annotation { captured: Dynamic }
const payload = new Dynamic { number = 41 }
class Owner {
  @Marker { captured = payload }
  early = 1
  @Marker { captured = new Dynamic { number = 52 } }
  @Int
  late = 2
}
scalar = 0
"#,
    );
    vm.start_member(module.as_object().unwrap(), "Owner")
        .unwrap();
    let mut observed = BTreeSet::new();
    let failure = loop {
        for value in &vm.stack {
            if let Some(id) = value.heap_id() {
                observed.extend(
                    vm.heap
                        .get(id)
                        .unwrap()
                        .annotations
                        .iter()
                        .filter_map(Value::heap_id),
                );
            }
        }
        match vm.poll(1) {
            VmPoll::Ready(result) => break result.unwrap_err(),
            VmPoll::Yielded => {
                vm.collect(&[]);
            }
            event => panic!("unexpected event {event:?}"),
        }
    };
    assert_eq!(failure.message, "Expected an annotation class.");
    let owner = member(&mut vm, &module, "Owner")
        .unwrap()
        .heap_id()
        .unwrap();
    let object = vm.heap.get(owner).unwrap();
    assert_eq!(
        object
            .members
            .iter()
            .map(|member| member.name.as_ref())
            .collect::<Vec<_>>(),
        ["early"]
    );
    assert_eq!(object.member_annotations.len(), 1);
    let retained = object.member_annotations.values().next().unwrap()[0]
        .heap_id()
        .unwrap();
    assert!(observed.contains(&retained));
    assert!(observed.len() >= 2);
    vm.collect(&[]);
    assert!(vm.heap.get(retained).is_ok());
    assert!(
        observed
            .into_iter()
            .filter(|id| *id != retained)
            .all(|id| vm.heap.get(id).is_err())
    );
    assert_eq!(member(&mut vm, &module, "scalar"), Ok(Value::Int(0)));
}

#[test]
fn annotation_property_parent_inference_and_lexical_capture_remain_lazy() {
    let (mut vm, module) = setup(
        r#"
class Payload { number: Int }
class Marker extends Annotation { child: Payload; unused: String }
const captured = 71
@Marker { child = new { number = captured }; unused = throw("unforced") }
class Owner {}
"#,
    );
    let owner = member(&mut vm, &module, "Owner")
        .unwrap()
        .heap_id()
        .unwrap();
    let annotation = vm.heap.get(owner).unwrap().annotations[0].clone();
    let child = member(&mut vm, &annotation, "child").unwrap();
    assert_eq!(member(&mut vm, &child, "number"), Ok(Value::Int(71)));
    assert_eq!(
        vm.member_state(annotation.as_object().unwrap(), "unused")
            .unwrap(),
        LazyState::Unevaluated
    );
    vm.collect(&[]);
    assert_eq!(
        member(&mut vm, &annotation, "unused").unwrap_err().message,
        "unforced"
    );
}

#[test]
fn class_member_eager_annotations_use_declaration_frame_and_module_const_barrier() {
    for qualifier in ["const ", ""] {
        let source = format!(
            r#"
{qualifier}flag = true
class Owner {{
  @Deprecated {{ when (flag) {{ message = "ok" }} }}
  function method() = 1
}}
"#
        );
        let (mut vm, module) = setup(&source);
        let result = member(&mut vm, &module, "Owner");
        if qualifier.is_empty() {
            assert!(result.unwrap_err().message.contains("not `const`"));
        } else {
            let owner = result.unwrap().heap_id().unwrap();
            let annotation = vm
                .heap
                .get(owner)
                .unwrap()
                .member_annotations
                .values()
                .next()
                .unwrap()[0]
                .clone();
            assert_eq!(
                vm.member_state(annotation.as_object().unwrap(), "message")
                    .unwrap(),
                LazyState::Completed(Value::string("ok"))
            );
            assert_eq!(
                vm.member_state(annotation.as_object().unwrap(), "since")
                    .unwrap(),
                LazyState::Unevaluated
            );
        }
    }
}

const PENDING_SOURCE: &str = r#"
class Marker extends Annotation { value: String }
@Marker { value = "retained" }
@Marker { when (read("mem:annotation") == "go") { value = "pending" } }
class Owner { value = 9 }
trigger = Owner == Owner
nested = read("mem:nested")
"#;

#[test]
fn nonterminal_cancel_reclaims_pending_builder_but_preserves_annotation_prefix() {
    let (mut vm, module) = setup(PENDING_SOURCE);
    vm.start_member(module.as_object().unwrap(), "Owner")
        .unwrap();
    let request = loop {
        match vm.poll(1) {
            VmPoll::Pending(request) => break request,
            VmPoll::Yielded => {
                vm.collect(&[]);
            }
            event => panic!("unexpected event {event:?}"),
        }
    };
    let pending = *vm
        .frames
        .iter()
        .flat_map(|frame| &frame.builders)
        .next_back()
        .unwrap();
    let owner = vm
        .class_singletons
        .values()
        .copied()
        .find(|id| {
            vm.heap
                .get(*id)
                .unwrap()
                .class_definition
                .as_ref()
                .unwrap()
                .name
                .as_ref()
                == "Owner"
        })
        .unwrap();
    let retained = vm.heap.get(owner).unwrap().annotations[0]
        .heap_id()
        .unwrap();
    vm.collect(&[]);
    assert!(vm.heap.get(pending).is_ok());
    vm.cancel();
    assert_eq!(ready(&mut vm).unwrap_err().kind, ErrorKind::Cancelled);
    vm.collect(&[]);
    assert!(vm.heap.get(pending).is_err());
    assert!(vm.heap.get(retained).is_ok());
    assert_eq!(member(&mut vm, &module, "Owner"), Ok(Value::Class(owner)));
    assert_eq!(
        vm.resume(request.id, Ok(Value::Null)).unwrap_err().kind,
        ErrorKind::Host
    );
    vm.close();
    assert_eq!(vm.heap_stats().slots, 0);
}

#[test]
fn terminal_actions_release_actual_annotation_and_reentrant_host_roots() {
    for terminal in [ErrorKind::Cancelled, ErrorKind::Closed, ErrorKind::Timeout] {
        let mut program = (*program(PENDING_SOURCE)).clone();
        let entries = ["trigger", "nested"].map(|name| {
            let entry = program.blocks.len();
            program.blocks.push(vec![
                Instruction::Input(0),
                Instruction::GetMember(name.into()),
                Instruction::Return,
            ]);
            entry
        });
        let module_entry = program.entry;
        let mut session = HostSession::new(Arc::new(program));
        session.start(module_entry, vec![], 0, None).unwrap();
        let module = loop {
            match session.poll(0, 1) {
                HostPoll::Ready(result) => break result.unwrap(),
                HostPoll::Yielded => {
                    session.collect(&[]);
                }
                event => panic!("unexpected event {event:?}"),
            }
        };
        session
            .start(entries[0], vec![module.clone()], 1, Some(10))
            .unwrap();
        let outer = loop {
            match session.poll(1, 1) {
                HostPoll::Read { token, .. } => break token,
                HostPoll::Yielded => {
                    session.collect(&[]);
                }
                event => panic!("unexpected event {event:?}"),
            }
        };
        session
            .reenter(&outer, entries[1], vec![module], 2, None)
            .unwrap();
        let inner = loop {
            match session.poll(2, 1) {
                HostPoll::Read { token, .. } => break token,
                HostPoll::Yielded => {
                    session.collect(&[]);
                }
                event => panic!("unexpected event {event:?}"),
            }
        };
        assert!(session.collect(&[]).live > 0);
        match terminal {
            ErrorKind::Cancelled => session.cancel(),
            ErrorKind::Closed => session.close(),
            ErrorKind::Timeout => {}
            _ => unreachable!(),
        }
        let HostPoll::Ready(Err(error)) = session.poll(12, 1) else {
            panic!("expected terminal state")
        };
        assert_eq!(error.kind, terminal);
        assert_eq!(session.heap_stats().slots, 0);
        assert_eq!(
            session.drain_cancelled_reads(),
            [outer.clone(), inner.clone()]
        );
        for token in [outer, inner] {
            assert_eq!(
                session
                    .resume(&token, Ok(Value::Null), 12)
                    .unwrap_err()
                    .kind,
                ErrorKind::Closed
            );
        }
        assert_eq!(session.context_depth(), 0);
    }
}

#[test]
fn reused_program_keeps_annotation_metadata_and_captured_values_evaluator_local() {
    let program = program(
        r#"
@Deprecated { message = "value" }
class Owner {}
"#,
    );
    let mut first = Vm::new(program.clone());
    let mut second = Vm::new(program.clone());
    first.start(program.entry, vec![]).unwrap();
    let first_module = ready(&mut first).unwrap();
    second.start(program.entry, vec![]).unwrap();
    let second_module = ready(&mut second).unwrap();
    let first_owner = member(&mut first, &first_module, "Owner")
        .unwrap()
        .heap_id()
        .unwrap();
    let second_owner = member(&mut second, &second_module, "Owner")
        .unwrap()
        .heap_id()
        .unwrap();
    let annotation = first.heap.get(first_owner).unwrap().annotations[0].clone();
    assert_ne!(
        annotation,
        second.heap.get(second_owner).unwrap().annotations[0]
    );
    assert_eq!(
        second
            .start(program.entry, vec![annotation.clone()])
            .unwrap_err()
            .kind,
        ErrorKind::StaleHandle
    );
    second.close();
    first.collect(&[]);
    assert_eq!(
        member(&mut first, &annotation, "message"),
        Ok(Value::string("value"))
    );
    first.close();
    assert_eq!(first.heap_stats().live, 0);
}

#[test]
fn builtin_annotation_fields_keep_pinned_defaults_and_lazy_collection_checks() {
    let (mut vm, module) = setup(
        r#"
deprecated = new Deprecated {}
source = new SourceCode {}
known = new AlsoKnownAs { names { "correct"; 1 } }
defaults = deprecated.message == null && deprecated.since == null && deprecated.replaceWith == null
prefix = source.prefix
language = source.language
size = known.names.length
invalid = known.names[1]
"#,
    );
    assert_eq!(member(&mut vm, &module, "defaults"), Ok(Value::Bool(true)));
    assert_eq!(member(&mut vm, &module, "prefix"), Ok(Value::Null));
    assert_eq!(
        member(&mut vm, &module, "language").unwrap_err().message,
        "Tried to read property `language` but its value is undefined."
    );
    assert_eq!(member(&mut vm, &module, "size"), Ok(Value::Int(2)));
    let error = member(&mut vm, &module, "invalid").unwrap_err();
    assert!(error.message.contains("Expected value of type `String`"));
    let origin = &error.frames[0];
    let source = origin.source.as_ref().unwrap();
    assert_eq!(source.module_name.as_ref(), "pkl.base");
    assert_eq!(&source.text[origin.span.start..origin.span.end], "String");
}

#[test]
fn inherited_collection_checks_preserve_primary_origin_in_foreign_value_frames() {
    let source = "class Base { value: Listing<Listing<String>> }\nobj = new Base { value = new Listing { new Listing { 1 } } }\nlisting = obj.value\ninvalid = listing[0][0]\n";
    let mut program = (*program(source)).clone();
    let value_start = source.find("value =").unwrap();
    let metadata = program
        .metadata
        .iter_mut()
        .find(|metadata| {
            metadata
                .declaration
                .is_some_and(|span| span.start == value_start)
        })
        .unwrap();
    // Exercise the VM's cross-source contract without claiming module loading:
    // the override is foreign, but its inherited collection type is primary.
    metadata.source = Some(Arc::new(crate::value::RuntimeSource {
        text: Arc::from(source),
        uri: Arc::from("file:///foreign-value.pkl"),
        module_name: Arc::from("foreign"),
        line_anchors: false,
    }));
    let mut vm = Vm::new(Arc::new(program));
    vm.start(vm.program.entry, vec![]).unwrap();
    let module = ready(&mut vm).unwrap();
    let listing = member(&mut vm, &module, "listing").unwrap();
    let cast = vm
        .heap
        .get(listing.as_object().unwrap())
        .unwrap()
        .collection_cast
        .as_ref()
        .unwrap();
    assert!(cast.source.is_none());
    let error = member(&mut vm, &module, "invalid").unwrap_err();
    assert!(error.frames[0].source.is_none());
    assert_eq!(
        &source[error.frames[0].span.start..error.frames[0].span.end],
        "String"
    );
}

#[test]
fn annotation_parameters_preserve_empty_body_and_generator_error_order() {
    for (body, expected) in [
        ("x: Missing ->", None),
        (
            "x -> value = 1",
            Some("Expected a function that returns an object, but got an object."),
        ),
        (
            "x: Int -> when (read(\"mem:must-not-run\") == \"go\") {}",
            Some("Expected a function that returns an object, but got an object."),
        ),
    ] {
        let source = format!(
            "class Marker extends Annotation {{ value: Int }}\n@Marker {{ {body} }}\nclass Owner {{}}\n"
        );
        let (mut vm, module) = setup(&source);
        let result = member(&mut vm, &module, "Owner");
        if let Some(expected) = expected {
            assert_eq!(result.unwrap_err().message, expected);
        } else {
            assert!(result.is_ok());
        }
    }
}
