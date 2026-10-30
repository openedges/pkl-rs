//! Shared VM regressions. Pinned Java raw comparison is separate evidence.
use pkl_runtime::{
    evaluation::frontend::{ModuleEvalOptions, ModuleSourceInput, evaluate_module_output},
    heap::LazyState,
    host::{HostPoll, HostSession},
    parser::parse_module,
    resolver::resolve_module,
    value::{ErrorKind, RuntimeError, Value},
    vm::{Instruction as I, Program, Vm, VmPoll, compile_module},
};
use std::sync::Arc;
#[path = "support/module_keys.rs"]
mod module_keys;

fn output(source: &str) -> Vec<u8> {
    let result = evaluate_module_output(
        ModuleSourceInput {
            text: source,
            uri: "file:///transforms.pkl",
            inferred_name: "transforms",
        },
        &ModuleEvalOptions::default(),
    );
    assert_eq!(
        result.exit_code,
        0,
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    result.stdout
}

#[test]
fn all_transforms_preserve_order_indices_nulls_and_duplicate_rules() {
    for (expression, expected) in [
        ("List(3,1,2).map((x)->x*10)", "List(30, 10, 20)"),
        (
            "List(4,5,6).mapIndexed((i,x)->Pair(i,x))",
            "List(Pair(0, 4), Pair(1, 5), Pair(2, 6))",
        ),
        (
            "List(1,2,3).mapNonNull((x)->if(x==2)null else x)",
            "List(1, 3)",
        ),
        (
            "List(4,5,6).mapNonNullIndexed((i,x)->if(i==1)null else Pair(i,x))",
            "List(Pair(0, 4), Pair(2, 6))",
        ),
        (
            "List(1,2).mapNonNull((x)->Null(new Dynamic { marker=throw(\"unforced\") }))",
            "List()",
        ),
        ("Set(3,1,2).map((x)->x%2)", "Set(1, 0)"),
        (
            "Set(4,5,6).mapIndexed((i,x)->Pair(i,x))",
            "Set(Pair(0, 4), Pair(1, 5), Pair(2, 6))",
        ),
        ("Set(1,2,3).mapNonNull((x)->if(x==2)null else 1)", "Set(1)"),
        (
            "Set(4,5,6).mapNonNullIndexed((i,x)->if(i==1)null else i)",
            "Set(0, 2)",
        ),
        (
            "Map(\"b\",1,\"a\",2).map((k,v)->Pair(v,k))",
            "Map(1, \"b\", 2, \"a\")",
        ),
        (
            "Map(\"a\",1,\"b\",2,\"c\",3).mapKeys((k,v)->\"same\")",
            "Map(\"same\", 3)",
        ),
        (
            "Map(\"b\",1,\"a\",2).mapValues((k,v)->v*10)",
            "Map(\"b\", 10, \"a\", 20)",
        ),
        ("Set(1,2).map((x)->if(x==1)1 else 1.0)", "Set(1, 1.0)"),
    ] {
        assert_eq!(
            output(&format!("output {{ text = ({expression}).toString() }}")),
            expected.as_bytes(),
            "{expression}"
        );
    }
}

#[test]
fn conversions_and_map_projections_preserve_key_representatives_and_order() {
    assert_eq!(
        output(
            r#"
class Key { id:Int; hidden tag:Int }
first = new Key { id=1; tag=10 }
other = new Key { id=2; tag=20 }
duplicate = new Key { id=1; tag=30 }
keys = List(first,other,duplicate).toSet().toList().map((x)->x.tag)
table = Map(first,1,other,2,duplicate,3)
output { text = List(keys, table.keys.toList().map((x)->x.tag), table.values, table.keys.contains(duplicate)).toString() }
"#
        ),
        b"List(List(10, 20), List(10, 20), List(3, 2), true)"
    );
    for expression in [
        "List().toSet().toList()",
        "Set().toList().toSet().toList()",
        "Map().keys.toList()",
        "Map().values",
    ] {
        assert_eq!(
            output(&format!("output {{ text = ({expression}).toString() }}")),
            b"List()"
        );
    }
    let numbers = (0..40)
        .rev()
        .map(|n| n.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let entries = (0..40)
        .rev()
        .map(|n| format!("{n}, {}", n * 10))
        .collect::<Vec<_>>()
        .join(",");
    assert_eq!(
        output(&format!(
            "output {{ text = List({numbers}).toSet().toList().toString() }}"
        )),
        format!(
            "List({})",
            (0..40)
                .rev()
                .map(|n| n.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        )
        .as_bytes()
    );
    assert_eq!(
        output(&format!(
            "output {{ text = Map({entries}).keys.toList().toString() }}"
        )),
        format!(
            "List({})",
            (0..40)
                .rev()
                .map(|n| n.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        )
        .as_bytes()
    );
}

#[test]
fn identity_conversions_return_the_actual_empty_and_nonempty_receiver() {
    let mut compiled =
        (*program("emptyList=List()\noneList=List(1)\nemptySet=Set()\noneSet=Set(1)")).clone();
    let calls = ["toList", "toSet"].map(|name| {
        let block = compiled.blocks.len();
        compiled.blocks.push(vec![
            I::Input(0),
            I::GetMethod(name.into()),
            I::Call {
                arity: 0,
                method: true,
            },
            I::Return,
        ]);
        block
    });
    let entry = compiled.entry;
    let mut vm = Vm::new(Arc::new(compiled));
    vm.start(entry, vec![]).unwrap();
    let module = ready(&mut vm).unwrap();
    for (property, call) in [
        ("emptyList", calls[0]),
        ("oneList", calls[0]),
        ("emptySet", calls[1]),
        ("oneSet", calls[1]),
    ] {
        vm.start_member(module.as_object().unwrap(), property)
            .unwrap();
        let receiver = ready_rooted(&mut vm, std::slice::from_ref(&module)).unwrap();
        vm.start(call, vec![receiver.clone()]).unwrap();
        let actual = ready_rooted(&mut vm, &[module.clone(), receiver.clone()]).unwrap();
        assert_eq!(actual.as_object().unwrap(), receiver.as_object().unwrap());
    }
    vm.close();
    assert_eq!(vm.heap_stats().slots, 0);
}

#[test]
fn identity_converters_preserve_complete_java_pcf_diagnostics() {
    use pkl_runtime::probe::SourceProbe;
    let cases = [
        (
            "empty-list-reference-matrix",
            include_str!("../../../tests/fixtures/m2-collection-transforms/provenance/java06/workspace/empty-list-reference-matrix/input.pkl"),
            [
                include_bytes!("../../../tests/fixtures/m2-collection-transforms/provenance/java06/cases/empty-list-reference-matrix/0/raw/session-0/call-2.stderr.utf16be.bin").as_slice(),
                include_bytes!("../../../tests/fixtures/m2-collection-transforms/provenance/java06/cases/empty-list-reference-matrix/0/raw/session-0/call-14.stderr.utf16be.bin").as_slice(),
            ],
        ),
        (
            "empty-set-reference-matrix",
            include_str!("../../../tests/fixtures/m2-collection-transforms/provenance/java06/workspace/empty-set-reference-matrix/input.pkl"),
            [
                include_bytes!("../../../tests/fixtures/m2-collection-transforms/provenance/java06/cases/empty-set-reference-matrix/0/raw/session-0/call-2.stderr.utf16be.bin").as_slice(),
                include_bytes!("../../../tests/fixtures/m2-collection-transforms/provenance/java06/cases/empty-set-reference-matrix/0/raw/session-0/call-14.stderr.utf16be.bin").as_slice(),
            ],
        ),
    ];
    for (name, source, expected) in cases {
        let uri = format!(
            "file:///Users/cody.ij.hwang/Projects/pkl-rs.stage51/target/m2/stage51-collection-oracle-06/workspace/{name}/input.pkl"
        );
        let mut probe = SourceProbe::new_module_evaluator("compact", false, false, false).unwrap();
        for _ in 0..2 {
            for (expression, expected) in [
                "r_receiver.renderDocument(empty)",
                "r_receiver.renderDocument(one)",
            ]
            .into_iter()
            .zip(expected)
            {
                probe
                    .evaluate_module_source(&uri, Some(source), expression, 0, None)
                    .unwrap();
                let error = public_event(&mut probe);
                assert_eq!(error.kind, "error", "{error:?}");
                let (pairs, remainder) = expected.as_chunks::<2>();
                assert!(remainder.is_empty());
                assert_eq!(
                    error.error.unwrap(),
                    pairs
                        .iter()
                        .map(|pair| u16::from_be_bytes(*pair))
                        .collect::<Vec<_>>(),
                    "{name}: {expression}"
                );
            }
            probe.collect();
        }
        probe.close();
        assert_eq!(probe.collect().slots, 0);
    }
}

#[test]
fn list_to_set_hashing_reenters_retries_and_cancels_without_losing_source_values() {
    let mut compiled = (*program(r#"
values = List(1,2,3).map((x)->let (value=read("make:\(x)")) new Listing { value; read("hash:\(x)") })
converted = values.toSet().toList().map((x)->x[0]).join("|")
side = Map(2,20,1,10).values.toSet().toList().toString()
"#)).clone();
    let [side, converted] = ["side", "converted"].map(|name| {
        let block = compiled.blocks.len();
        compiled
            .blocks
            .push(vec![I::Input(0), I::GetMember(name.into()), I::Return]);
        block
    });
    let entry = compiled.entry;
    let compiled = Arc::new(compiled);
    let mut vm = HostSession::new(compiled.clone());
    vm.start(entry, vec![], 0, None).unwrap();
    let module = host_ready(&mut vm).unwrap();
    vm.start(converted, vec![module.clone()], 0, None).unwrap();
    for x in 1..=3 {
        let token = host_read(&mut vm, &format!("make:{x}"));
        vm.resume(&token, Ok(Value::Int(x.into())), 0).unwrap();
    }
    let first = host_read(&mut vm, "hash:1");
    vm.resume(&first, Ok(Value::Int(1.into())), 0).unwrap();
    let second = host_read(&mut vm, "hash:2");
    vm.reenter(&second, side, vec![module.clone()], 0, None)
        .unwrap();
    assert_eq!(host_ready(&mut vm).unwrap(), Value::string("List(20, 10)"));
    assert_eq!(host_read(&mut vm, "hash:2"), second);
    vm.resume(
        &second,
        Err(RuntimeError::new(ErrorKind::Host, "conversion retry")),
        0,
    )
    .unwrap();
    assert_eq!(
        host_ready_rooted(&mut vm, std::slice::from_ref(&module))
            .unwrap_err()
            .message,
        "conversion retry"
    );
    vm.start(converted, vec![module], 0, None).unwrap();
    for x in 2..=3 {
        let token = host_read(&mut vm, &format!("hash:{x}"));
        vm.resume(&token, Ok(Value::Int(x.into())), 0).unwrap();
    }
    assert_eq!(host_ready(&mut vm).unwrap(), Value::string("1|2|3"));
    vm.close();
    assert_eq!(vm.heap_stats().slots, 0);

    // Vm cancellation is nonterminal; HostSession cancellation is terminal.
    // Exercise repeated builder abandonment through the nonterminal boundary.
    let mut vm = Vm::new(compiled);
    vm.start(entry, vec![]).unwrap();
    let module = ready(&mut vm).unwrap();
    let object = module.as_object().unwrap();
    let mut plateau = None;
    for iteration in 0..40 {
        vm.start_member(object, "converted").unwrap();
        if iteration == 0 {
            for x in 1..=3 {
                let token = read(&mut vm, &format!("make:{x}"));
                vm.resume(token, Ok(Value::Int(x.into()))).unwrap();
            }
            for x in 1..=2 {
                let token = read(&mut vm, &format!("hash:{x}"));
                vm.resume(token, Ok(Value::Int(x.into()))).unwrap();
            }
        }
        let pending = read(&mut vm, "hash:3");
        vm.cancel();
        assert_eq!(
            ready_rooted(&mut vm, std::slice::from_ref(&module))
                .unwrap_err()
                .kind,
            ErrorKind::Cancelled
        );
        assert_eq!(
            vm.member_state(object, "converted").unwrap(),
            LazyState::Unevaluated
        );
        assert_eq!(
            vm.resume(pending, Ok(Value::Null)).unwrap_err().kind,
            ErrorKind::Host
        );
        let stats = vm.collect(std::slice::from_ref(&module));
        let retained = (stats.live, stats.executable_live, stats.slots);
        if iteration == 3 {
            plateau = Some(retained)
        }
        if iteration >= 3 {
            assert_eq!(Some(retained), plateau)
        }
    }
    vm.start_member(object, "converted").unwrap();
    let last = read(&mut vm, "hash:3");
    vm.resume(last, Ok(Value::Int(3.into()))).unwrap();
    assert_eq!(ready(&mut vm).unwrap(), Value::string("1|2|3"));
    vm.close();
    assert_eq!(vm.heap_stats().slots, 0);
}

#[test]
fn original_source_code_union_fixture_runs_unchanged() {
    let source = include_str!(
        "../../../tests/fixtures/m2/annotation-stage35-builtin-source-code-union-and-string.pkl"
    );
    let program = compile_module(&resolve_module(&parse_module(source).unwrap()).unwrap()).unwrap();
    let mut vm = Vm::new(program.clone());
    vm.start(program.entry, vec![]).unwrap();
    let module = ready(&mut vm).unwrap();
    vm.start_member(module.as_object().unwrap(), "result")
        .unwrap();
    assert_eq!(
        ready(&mut vm).unwrap(),
        Value::string(
            "List(\"Go\", \"HTML\", \"Java\", \"JavaScript\", \"Markdown\", \"Pkl\", \"PklExpr\", \"Python\", \"Ruby\", \"SQL\", \"Swift\", \"Rust\")"
        )
    );
}

fn program(source: &str) -> Arc<Program> {
    compile_module(&resolve_module(&parse_module(source).unwrap()).unwrap()).unwrap()
}
fn event(vm: &mut Vm) -> VmPoll {
    event_rooted(vm, &[])
}
fn event_rooted(vm: &mut Vm, external_roots: &[Value]) -> VmPoll {
    for _ in 0..200_000 {
        vm.collect(external_roots);
        let event = vm.poll(1);
        // A returned value belongs to the caller, including when a nested
        // evaluation has restored the suspended parent before returning it.
        let mut roots = external_roots.to_vec();
        if let VmPoll::Ready(Ok(value)) = &event {
            roots.push(value.clone());
        }
        vm.collect(&roots);
        match event {
            VmPoll::Yielded => {}
            event => return event,
        }
    }
    panic!("transform poll bound")
}
fn ready(vm: &mut Vm) -> Result<Value, RuntimeError> {
    ready_rooted(vm, &[])
}
fn ready_rooted(vm: &mut Vm, external_roots: &[Value]) -> Result<Value, RuntimeError> {
    match event_rooted(vm, external_roots) {
        VmPoll::Ready(value) => value,
        other => panic!("expected result: {other:?}"),
    }
}
fn read(vm: &mut Vm, uri: &str) -> u64 {
    match event(vm) {
        VmPoll::Pending(request) => {
            assert_eq!(request.uri, uri);
            request.id
        }
        other => panic!("expected {uri}: {other:?}"),
    }
}

#[test]
fn function_validation_runs_on_empty_inputs_and_pair_check_runs_per_result() {
    for expression in [
        "List().map(42)",
        "List().map(()->1)",
        "Set().mapIndexed((x)->x)",
        "Map().mapValues((x)->x)",
        "Map(1,2).map((k,v)->new Dynamic { first=k; second=v })",
    ] {
        let program = program(&format!("result = {expression}"));
        let mut vm = Vm::new(program.clone());
        vm.start(program.entry, vec![]).unwrap();
        let module = ready(&mut vm).unwrap();
        vm.start_member(module.as_object().unwrap(), "result")
            .unwrap();
        assert_eq!(
            ready(&mut vm).unwrap_err().kind,
            ErrorKind::Type,
            "{expression}"
        );
    }
    assert_eq!(
        output("output { text = List().map((x)->throw(\"unreachable\")).toString() }"),
        b"List()"
    );
}

#[test]
fn set_callback_then_hash_finishes_before_next_callback_under_gc() {
    let program = program(
        r#"result = Set(1,2,3).map((x)->let (value=read("callback:\(x)")) new Listing { value; read("hash:\(x)") }).length"#,
    );
    let mut vm = Vm::new(program.clone());
    vm.start(program.entry, vec![]).unwrap();
    let module = ready(&mut vm).unwrap();
    vm.start_member(module.as_object().unwrap(), "result")
        .unwrap();
    for x in 1..=3 {
        for uri in [format!("callback:{x}"), format!("hash:{x}")] {
            let token = read(&mut vm, &uri);
            vm.resume(token, Ok(Value::Int(x.into()))).unwrap();
        }
    }
    assert_eq!(ready(&mut vm).unwrap(), Value::Int(3.into()));
    vm.close();
    assert_eq!(vm.heap_stats().slots, 0);
}

fn host_event(session: &mut HostSession) -> HostPoll {
    host_event_rooted(session, &[])
}
fn host_event_rooted(session: &mut HostSession, external_roots: &[Value]) -> HostPoll {
    for _ in 0..200_000 {
        session.collect(external_roots);
        let event = session.poll(0, 1);
        let mut roots = external_roots.to_vec();
        if let HostPoll::Ready(Ok(value)) = &event {
            roots.push(value.clone());
        }
        session.collect(&roots);
        match event {
            HostPoll::Yielded => {}
            event => return event,
        }
    }
    panic!("Host transform poll bound")
}
fn host_ready(session: &mut HostSession) -> Result<Value, RuntimeError> {
    host_ready_rooted(session, &[])
}
fn host_ready_rooted(
    session: &mut HostSession,
    external_roots: &[Value],
) -> Result<Value, RuntimeError> {
    match host_event_rooted(session, external_roots) {
        HostPoll::Ready(value) => value,
        other => panic!("expected Host result: {other:?}"),
    }
}
fn host_read(session: &mut HostSession, uri: &str) -> String {
    match host_event(session) {
        HostPoll::Read { token, uri: actual } => {
            assert_eq!(actual, uri);
            token
        }
        other => panic!("expected {uri}: {other:?}"),
    }
}

#[test]
fn nested_transform_failure_and_retry_preserve_suspended_outer_builder() {
    let source = r#"
parent = List(1,2).map((x)->read("outer:\(x)")).join("|")
child = Map(1,10,2,20).mapValues((k,v)->read("child:\(k)")).toString()
"#;
    let mut program = (*program(source)).clone();
    let entries = ["parent", "child"].map(|name| {
        let block = program.blocks.len();
        program
            .blocks
            .push(vec![I::Input(0), I::GetMember(name.into()), I::Return]);
        block
    });
    let entry = program.entry;
    let mut host = HostSession::new(Arc::new(program));
    host.start(entry, vec![], 0, None).unwrap();
    let module = host_ready(&mut host).unwrap();
    host.start(entries[0], vec![module.clone()], 0, None)
        .unwrap();
    let first = host_read(&mut host, "outer:1");
    host.resume(&first, Ok(Value::string("one")), 0).unwrap();
    let outer = host_read(&mut host, "outer:2");
    host.reenter(&outer, entries[1], vec![module.clone()], 0, None)
        .unwrap();
    let bad = host_read(&mut host, "child:1");
    host.resume(
        &bad,
        Err(RuntimeError::new(ErrorKind::Host, "retry transform")),
        0,
    )
    .unwrap();
    assert_eq!(
        host_ready(&mut host).unwrap_err().message,
        "retry transform"
    );
    assert_eq!(host_read(&mut host, "outer:2"), outer);
    host.reenter(&outer, entries[1], vec![module], 0, None)
        .unwrap();
    for x in 1..=2 {
        let token = host_read(&mut host, &format!("child:{x}"));
        assert_ne!(token, bad);
        host.resume(&token, Ok(Value::Int((x * 100).into())), 0)
            .unwrap();
    }
    assert_eq!(
        host_ready(&mut host).unwrap(),
        Value::string("Map(1, 100, 2, 200)")
    );
    assert_eq!(host_read(&mut host, "outer:2"), outer);
    host.resume(&outer, Ok(Value::string("two")), 0).unwrap();
    assert_eq!(host_ready(&mut host).unwrap(), Value::string("one|two"));
    host.close();
    assert_eq!(host.heap_stats().slots, 0);
}

#[test]
fn resumed_callback_objects_survive_every_poll_and_nested_result_boundary() {
    let mut compiled = (*program(
        r#"
result = List(1,2).map((x)->read("payload:\(x)")).map((value)->value.tag).toString()
function fresh() = new Dynamic { tag=7; junk=new Listing { 1; 2; 3 } }
"#,
    ))
    .clone();
    let result = compiled.blocks.len();
    compiled
        .blocks
        .push(vec![I::Input(0), I::GetMember("result".into()), I::Return]);
    let fresh = compiled.blocks.len();
    compiled.blocks.push(vec![
        I::Input(0),
        I::GetMethod("fresh".into()),
        I::Call {
            arity: 0,
            method: true,
        },
        I::Return,
    ]);
    let entry = compiled.entry;
    let mut host = HostSession::new(Arc::new(compiled));
    host.start(entry, vec![], 0, None).unwrap();
    let module = host_ready(&mut host).unwrap();
    host.start(result, vec![module.clone()], 0, None).unwrap();
    for x in 1..=2 {
        let token = host_read(&mut host, &format!("payload:{x}"));
        host.reenter(&token, fresh, vec![module.clone()], 0, None)
            .unwrap();
        let object = host_ready(&mut host).unwrap();
        assert!(object.as_object().is_ok());
        // The result now belongs to this caller. While checking the restored
        // parent token, keep it alive explicitly; resume transfers ownership
        // into the VM. Every subsequent pre/post-poll collection has no roots.
        let restored = host_event_rooted(&mut host, std::slice::from_ref(&object));
        assert!(matches!(restored, HostPoll::Read { token: ref actual, .. } if actual == &token));
        host.resume(&token, Ok(object), 0).unwrap();
    }
    assert_eq!(host_ready(&mut host).unwrap(), Value::string("List(7, 7)"));
    host.close();
    assert_eq!(host.heap_stats().slots, 0);
}

#[test]
fn cancellation_releases_partial_builders_and_callback_cursors_at_a_plateau() {
    let program = program(
        r#"child = Set(1,2,3).map((x)->let (value=read("callback:\(x)")) new Listing { value; read("hash:\(x)") }).length"#,
    );
    let mut vm = Vm::new(program.clone());
    vm.start(program.entry, vec![]).unwrap();
    let module = ready(&mut vm).unwrap();
    let object = module.as_object().unwrap();
    let mut plateau = None;
    for iteration in 0..40 {
        vm.start_member(object, "child").unwrap();
        for uri in ["callback:1", "hash:1"] {
            let token = read(&mut vm, uri);
            vm.resume(token, Ok(Value::Int(1.into()))).unwrap();
        }
        let callback = read(&mut vm, "callback:2");
        let token = if iteration % 2 == 0 {
            callback
        } else {
            vm.resume(callback, Ok(Value::Int(2.into()))).unwrap();
            read(&mut vm, "hash:2")
        };
        vm.cancel();
        assert_eq!(
            ready_rooted(&mut vm, std::slice::from_ref(&module))
                .unwrap_err()
                .kind,
            ErrorKind::Cancelled
        );
        assert_eq!(
            vm.member_state(object, "child").unwrap(),
            LazyState::Unevaluated
        );
        assert_eq!(
            vm.resume(token, Ok(Value::Null)).unwrap_err().kind,
            ErrorKind::Host
        );
        let stats = vm.collect(std::slice::from_ref(&module));
        let retained = (stats.live, stats.executable_live, stats.slots);
        if iteration == 3 {
            plateau = Some(retained)
        }
        if iteration >= 3 {
            assert_eq!(Some(retained), plateau)
        }
    }
    vm.start_member(object, "child").unwrap();
    for x in 1..=3 {
        for uri in [format!("callback:{x}"), format!("hash:{x}")] {
            let token = read(&mut vm, &uri);
            vm.resume(token, Ok(Value::Int(x.into()))).unwrap();
        }
    }
    assert_eq!(ready(&mut vm).unwrap(), Value::Int(3.into()));
    vm.close();
    assert_eq!(vm.heap_stats().slots, 0);
}

#[test]
fn scalar_result_drops_callback_captures_while_escaped_closures_keep_them() {
    let source = r#"
function payload() = new Dynamic { tag=5; junk=new Listing { 1; 2; 3; 4; 5 } }
function scalarList(payload) = List(1,2).map((x)->x + payload.tag)
function scalarSet(payload) = Set(1,2).map((x)->x + payload.tag)
function scalarMap(payload) = Map(1,10,2,20).mapValues((k,v)->v + payload.tag)
function escaped() = List(4,7).map((x)->(y)->x+y)
"#;
    let mut compiled = (*program(source)).clone();
    let [payload, scalar_list, scalar_set, scalar_map, escaped] =
        ["payload", "scalarList", "scalarSet", "scalarMap", "escaped"].map(|name| {
            let block = compiled.blocks.len();
            let mut code = vec![I::Input(0), I::GetMethod(name.into())];
            if name.starts_with("scalar") {
                code.push(I::Input(1));
            }
            code.extend([
                I::Call {
                    arity: usize::from(name.starts_with("scalar")),
                    method: true,
                },
                I::Return,
            ]);
            compiled.blocks.push(code);
            block
        });
    let use_escaped = compiled.blocks.len();
    compiled.blocks.push(vec![
        I::Input(0),
        I::Constant(Value::Int(1.into())),
        I::Subscript {
            key_span: pkl_runtime::source::Span::new(0, 0),
        },
        I::Constant(Value::Int(10.into())),
        I::Call {
            arity: 1,
            method: false,
        },
        I::Return,
    ]);
    let entry = compiled.entry;
    let mut vm = Vm::new(Arc::new(compiled));
    vm.start(entry, vec![]).unwrap();
    let module = ready(&mut vm).unwrap();
    let mut plateau = [None; 3];
    for iteration in 0..40 {
        for (index, scalar) in [scalar_list, scalar_set, scalar_map]
            .into_iter()
            .enumerate()
        {
            vm.start(payload, vec![module.clone()]).unwrap();
            let capture = ready_rooted(&mut vm, std::slice::from_ref(&module)).unwrap();
            let weak_capture = capture.as_object().unwrap();
            vm.start(scalar, vec![module.clone(), capture]).unwrap();
            let result = ready_rooted(&mut vm, std::slice::from_ref(&module)).unwrap();
            let stats = vm.collect(&[module.clone(), result]);
            assert_eq!(
                vm.member_state(weak_capture, "tag").unwrap_err().kind,
                ErrorKind::StaleHandle
            );
            let retained = (stats.live, stats.executable_live, stats.slots);
            if iteration == 3 {
                plateau[index] = Some(retained)
            }
            if iteration >= 3 {
                assert_eq!(Some(retained), plateau[index])
            }
        }
    }
    vm.start(escaped, vec![module.clone()]).unwrap();
    let functions = ready_rooted(&mut vm, std::slice::from_ref(&module)).unwrap();
    vm.collect(&[module, functions.clone()]);
    vm.start(use_escaped, vec![functions]).unwrap();
    assert_eq!(ready(&mut vm).unwrap(), Value::Int(17.into()));
    vm.close();
    assert_eq!(vm.heap_stats().slots, 0);
}

#[test]
fn empty_results_share_identity_without_retaining_their_producers() {
    let mut compiled = (*program(
        r#"
function payload() = new Dynamic { tag=1; junk=new Listing { 1; 2; 3 } }
function emptyList(payload) = List(1).mapNonNull((x)->if(payload.tag==1)null else x)
function emptySet(payload) = Set(1).mapNonNull((x)->if(payload.tag==1)null else x)
function emptyMap(payload) = Map().mapValues((k,v)->payload.tag)
function literalList(payload) = List()
function literalSet(payload) = Set()
function literalMap(payload) = Map()
"#,
    ))
    .clone();
    let entries = [
        "payload",
        "emptyList",
        "emptySet",
        "emptyMap",
        "literalList",
        "literalSet",
        "literalMap",
    ]
    .map(|name| {
        let block = compiled.blocks.len();
        let arity = usize::from(name != "payload");
        let mut code = vec![I::Input(0), I::GetMethod(name.into())];
        if arity == 1 {
            code.push(I::Input(1));
        }
        code.extend([
            I::Call {
                arity,
                method: true,
            },
            I::Return,
        ]);
        compiled.blocks.push(code);
        block
    });
    let clear = compiled.blocks.len();
    compiled
        .blocks
        .push(vec![I::Constant(Value::Null), I::Return]);
    let entry = compiled.entry;
    let mut vm = Vm::new(Arc::new(compiled));
    vm.start(entry, vec![]).unwrap();
    let module = ready(&mut vm).unwrap();
    let mut roots = vec![module.clone()];
    for iteration in 0..40 {
        for kind in 0..3 {
            vm.start(entries[0], vec![module.clone()]).unwrap();
            let payload = ready_rooted(&mut vm, &roots).unwrap();
            let weak = payload.as_object().unwrap();
            let operation = entries[1 + kind + if iteration % 2 == 0 { 0 } else { 3 }];
            vm.start(operation, vec![module.clone(), payload]).unwrap();
            let empty = ready_rooted(&mut vm, &roots).unwrap();
            if iteration == 0 {
                roots.push(empty);
            } else {
                assert_eq!(empty, roots[1 + kind]);
            }
            vm.collect(&roots);
            assert_eq!(
                vm.member_state(weak, "tag").unwrap_err().kind,
                ErrorKind::StaleHandle
            );
        }
    }
    let empty_handles = roots[1..]
        .iter()
        .map(|value| value.as_object().unwrap())
        .collect::<Vec<_>>();
    vm.start(clear, vec![]).unwrap();
    assert_eq!(ready(&mut vm).unwrap(), Value::Null);
    vm.collect(&[]);
    for id in empty_handles {
        assert_eq!(
            vm.member_state(id, "unused").unwrap_err().kind,
            ErrorKind::StaleHandle
        );
    }
    vm.close();
    assert_eq!(vm.heap_stats().slots, 0);
}

#[test]
fn null_constructor_preserves_defaults_without_forcing_and_respects_shadowing() {
    assert_eq!(
        output(
            r#"value = Null(new Dynamic { marker=throw("unforced") })
output { text = List(1,2).mapNonNull((x)->value).toString() }"#
        ),
        b"List()"
    );
    assert_eq!(
        output(
            r#"function Null(x) = x + 10
output { text = List(1,2).map((x)->Null(x)).toString() }"#
        ),
        b"List(11, 12)"
    );
    for value in [
        "42",
        "null",
        "List()",
        "Set()",
        "Map()",
        "Pair(1,2)",
        "String",
    ] {
        let compiled = program(&format!("result = Null({value})"));
        let mut vm = Vm::new(compiled.clone());
        vm.start(compiled.entry, vec![]).unwrap();
        let module = ready(&mut vm).unwrap();
        vm.start_member(module.as_object().unwrap(), "result")
            .unwrap();
        assert_eq!(ready(&mut vm).unwrap_err().kind, ErrorKind::Type, "{value}");
    }
}

#[test]
fn null_defaults_keep_object_and_function_amendment_semantics() {
    // Java05 null-amendment-parenthesized supplies the independent outcomes.
    assert_eq!(
        output(
            r#"
defaultObject = Null(new Dynamic { a=1 })
amended = (defaultObject) { a=2 }
defaultFunction = Null(()->new Dynamic { a=3 })
amendedFunction = (defaultFunction) { a=4 }
defaultFunction1 = Null((x)->new Dynamic { a=x })
amendedFunction1 = (defaultFunction1) { a=5 }
unforced = Null(new Dynamic { boom=throw("default-not-forced") })
amendedBoom = (unforced) { a=1 }
output { text = List(amended.a, amendedFunction.apply().a, amendedFunction1.apply(9).a, amendedBoom.a).toString() }
"#
        ),
        b"List(2, 4, 5, 1)"
    );
    assert_eq!(
        output(
            r#"output { text = Set(1,2).mapNonNull((x)->Null(()->throw("default-not-forced"))).toString() }"#
        ),
        b"Set()"
    );
}

#[test]
fn amended_function_failures_match_complete_java_diagnostics() {
    use pkl_runtime::probe::SourceProbe;
    let source = include_str!(
        "../../../tests/fixtures/m2-collection-transforms/provenance/java05/workspace/null-amendment-parenthesized/input.pkl"
    );
    let uri = "file:///Users/cody.ij.hwang/Projects/pkl-rs.stage50/target/m2/stage51-collection-oracle-05/workspace/null-amendment-parenthesized/input.pkl";
    let mut probe = SourceProbe::new_module_evaluator("compact", false, false, false).unwrap();
    for (expression, expected) in [
        (
            "amendedFunction.a",
            include_bytes!("../../../tests/fixtures/m2-collection-transforms/provenance/java05/cases/null-amendment-parenthesized/0/raw/session-0/call-5.stderr.bin").as_slice(),
        ),
        (
            "amendedFunction1.apply().a",
            include_bytes!("../../../tests/fixtures/m2-collection-transforms/provenance/java05/cases/null-amendment-parenthesized/0/raw/session-0/call-8.stderr.bin").as_slice(),
        ),
    ] {
        probe.evaluate_module_source(uri, Some(source), expression, 0, None).unwrap();
        let failure = public_event(&mut probe);
        assert_eq!(failure.kind, "error", "{failure:?}");
        assert_eq!(
            failure.error.unwrap(),
            std::str::from_utf8(expected)
                .unwrap()
                .encode_utf16()
                .collect::<Vec<_>>()
        );
    }
    probe.close();
    assert_eq!(probe.collect().slots, 0);
}

#[test]
fn null_rejects_an_actual_duration_with_the_complete_java_union_diagnostic() {
    use pkl_runtime::probe::SourceProbe;
    let source = include_str!(
        "../../../tests/fixtures/m2-collection-transforms/provenance/java04/workspace/null-argument-guard/input.pkl"
    );
    let uri = "file:///Users/cody.ij.hwang/Projects/pkl-rs.stage50/target/m2/stage51-collection-oracle-04/workspace/null-argument-guard/input.pkl";
    let expected = include_bytes!(
        "../../../tests/fixtures/m2-collection-transforms/provenance/java04/cases/null-argument-guard/0/raw/session-0/call-18.stderr.utf16be.bin"
    );
    let mut probe = SourceProbe::new_module_evaluator("compact", false, false, false).unwrap();
    probe
        .evaluate_module_source(uri, Some(source), "Null(1.s).toString()", 0, None)
        .unwrap();
    let failure = public_event(&mut probe);
    assert_eq!(failure.kind, "error", "{failure:?}");
    let (pairs, remainder) = expected.as_chunks::<2>();
    assert!(remainder.is_empty());
    assert_eq!(
        failure.error.unwrap(),
        pairs
            .iter()
            .map(|pair| u16::from_be_bytes(*pair))
            .collect::<Vec<_>>()
    );
    probe.close();
    assert_eq!(probe.collect().slots, 0);
}

fn public_event(probe: &mut pkl_runtime::probe::SourceProbe) -> pkl_runtime::probe::SourceEvent {
    use pkl_runtime::modules::*;
    for _ in 0..200_000 {
        probe.collect();
        let event = probe.poll(0, 1).unwrap();
        probe.collect();
        if event.kind == "yield" {
            continue;
        }
        if event.kind == "module-request" {
            let token = event.token.as_deref().unwrap();
            let request = probe.module_request(token).unwrap();
            let completion = match &request.kind {
                ModuleRequestKind::QueryCachePolicy { key } if key.uri == "memo:/child.pkl" => {
                    Some(ModuleCompletion::CachePolicy(Ok(false)))
                }
                ModuleRequestKind::Resolve {
                    key,
                    resolved_key_id,
                    ..
                }
                | ModuleRequestKind::ResolveKey {
                    key,
                    resolved_key_id,
                    ..
                } if key.uri.starts_with("memo:") => {
                    Some(ModuleCompletion::KeyResolved(Ok(ResolvedModuleKey {
                        id: *resolved_key_id,
                        key: key.clone(),
                        handle: "resolved".into(),
                        module: ResolvedModule {
                            original_uri: key.uri.clone(),
                            resolved_uri: key.uri.clone(),
                            source_uri: key.uri.clone(),
                        },
                    })))
                }
                _ => module_keys::defaults(&request),
            };
            if let Some(completion) = completion {
                probe.complete_module(token, completion, 0).unwrap();
                continue;
            }
        }
        return event;
    }
    panic!("public transform poll bound")
}

#[test]
fn callback_import_and_escaped_typed_closures_survive_reentry_and_module_replacement() {
    use pkl_runtime::modules::{ModuleCompletion, ModuleRequestKind, ModuleSource};
    use pkl_runtime::probe::SourceProbe;
    let main =
        r#"kept = List(1,2).map((x)->let (child=import("memo:/child.pkl")) (y)->child.make(x+y))"#;
    let mut probe = SourceProbe::new_module_evaluator("compact", false, false, false).unwrap();
    probe
        .evaluate_module_source(
            "memo:/main.pkl",
            Some(main),
            "kept[0].apply(10).tag",
            0,
            None,
        )
        .unwrap();
    let suspended = public_event(&mut probe);
    assert_eq!(suspended.kind, "module-request", "{suspended:?}");
    let token = suspended.token.unwrap();
    assert!(matches!(
        probe.module_request(&token).unwrap().kind,
        ModuleRequestKind::LoadSource { .. }
    ));
    probe
        .reenter_module_source(
            &token,
            "memo:/main.pkl",
            Some(main),
            "List(7,8).mapIndexed((i,x)->i+x).toString()",
            0,
            None,
        )
        .unwrap();
    let nested = public_event(&mut probe);
    assert_eq!(nested.kind, "value", "{nested:?}");
    assert_eq!(
        String::from_utf16(&nested.value.unwrap()).unwrap(),
        "List(7, 9)"
    );
    let suspended_again = public_event(&mut probe);
    assert_eq!(suspended_again.token.as_deref(), Some(token.as_str()));
    probe.complete_module(&token, ModuleCompletion::Source(Ok(ModuleSource {
        text: "class C { tag:Int; version=1 }\ntypealias A=C\nfunction make(x:Int):A = new C { tag=x }\n".into(), inferred_name: "child".into(),
    })), 0).unwrap();
    let value = public_event(&mut probe);
    assert_eq!(value.kind, "value", "{value:?}");
    assert_eq!(String::from_utf16(&value.value.unwrap()).unwrap(), "11");
    probe
        .evaluate_module_source(
            "memo:/main.pkl",
            Some(main),
            "import(\"memo:/child.pkl\").make(99).version",
            0,
            None,
        )
        .unwrap();
    let replacement = public_event(&mut probe);
    assert_eq!(replacement.kind, "module-request", "{replacement:?}");
    probe.complete_module(replacement.token.as_deref().unwrap(), ModuleCompletion::Source(Ok(ModuleSource {
        text: "class C { tag:Int; version=2 }\ntypealias A=C\nfunction make(x:Int):A = new C { tag=x }\n".into(), inferred_name: "child".into(),
    })), 0).unwrap();
    let value = public_event(&mut probe);
    assert_eq!(value.kind, "value", "{value:?}");
    assert_eq!(String::from_utf16(&value.value.unwrap()).unwrap(), "2");
    for (expression, expected) in [
        ("kept[1].apply(20).tag", "22"),
        ("kept[1].apply(20).version", "1"),
    ] {
        probe
            .evaluate_module_source("memo:/main.pkl", Some(main), expression, 0, None)
            .unwrap();
        let value = public_event(&mut probe);
        assert_eq!(value.kind, "value", "{value:?}");
        assert_eq!(String::from_utf16(&value.value.unwrap()).unwrap(), expected);
    }
    probe.close();
    assert_eq!(probe.collect().slots, 0);
}
