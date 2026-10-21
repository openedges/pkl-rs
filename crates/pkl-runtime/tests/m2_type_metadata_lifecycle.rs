//! Evaluator ownership, GC roots, and Host cleanup for alias/function metadata.
//! These scheduler and heap assertions are separate from raw Java comparisons.
use std::sync::Arc;

use pkl_runtime::heap::LazyState;
use pkl_runtime::host::{HostPoll, HostSession};
use pkl_runtime::parser::parse_module;
use pkl_runtime::resolver::resolve_module;
use pkl_runtime::value::{ErrorKind, RuntimeError, Value};
use pkl_runtime::vm::{Instruction as I, Program, Vm, VmPoll, compile_module};

fn program(source: &str) -> Program {
    let parsed = parse_module(source).unwrap();
    (*compile_module(&resolve_module(&parsed).unwrap()).unwrap()).clone()
}

fn vm_ready(vm: &mut Vm) -> Result<Value, RuntimeError> {
    for _ in 0..50_000 {
        match vm.poll(1) {
            VmPoll::Ready(result) => return result,
            VmPoll::Yielded => {
                vm.collect(&[]);
            }
            event => panic!("unexpected VM event {event:?}"),
        }
    }
    panic!("evaluation failed to settle")
}

fn member(vm: &mut Vm, module: &Value, name: &str) -> Result<Value, RuntimeError> {
    vm.start_member(module.as_object().unwrap(), name).unwrap();
    vm_ready(vm)
}

#[test]
fn alias_singleton_roots_its_declaration_graph_and_releases_temporary_values() {
    let mut program = program(
        r#"
        const captured = new Dynamic { value = 42 }
        typealias Answer = Int(this == captured.value)
        result: Answer = 42
        temporary = new Dynamic { value = 1 }
        "#,
    );
    let scalar = program.blocks.len();
    program
        .blocks
        .push(vec![I::Constant(Value::Int(7)), I::Return]);
    let template = program.objects.len();
    program.objects.push(vec![]);
    let transient_entry = program.blocks.len();
    program.blocks.push(vec![
        I::MakeObject {
            template,
            has_parent: false,
        },
        I::Return,
    ]);
    let entry = program.entry;
    let mut vm = Vm::new(Arc::new(program));
    vm.start(entry, vec![]).unwrap();
    let module = vm_ready(&mut vm).unwrap();
    let alias = member(&mut vm, &module, "Answer").unwrap();
    assert!(matches!(alias, Value::TypeAlias(_)));
    assert_eq!(vm.collect(&[]).live, 2);
    assert_eq!(member(&mut vm, &module, "result"), Ok(Value::Int(42)));
    // The singleton's declaration module now also retains the captured object.
    assert_eq!(vm.collect(&[]).live, 3);
    vm.start(scalar, vec![]).unwrap();
    assert_eq!(vm_ready(&mut vm), Ok(Value::Int(7)));
    assert_eq!(vm.collect(&[]).live, 3);
    assert_eq!(member(&mut vm, &module, "Answer"), Ok(alias.clone()));
    vm.start_member(module.as_object().unwrap(), "temporary")
        .unwrap();
    let temporary = vm_ready(&mut vm).unwrap();
    // A completed module property deliberately remains in the declaration graph.
    assert_eq!(vm.collect(&[]).live, 4);
    assert_eq!(member(&mut vm, &module, "Answer"), Ok(alias));
    vm.start(transient_entry, vec![]).unwrap();
    let transient = vm_ready(&mut vm).unwrap();
    assert_eq!(vm.collect(&[]).live, 5);
    vm.start(scalar, vec![]).unwrap();
    assert_eq!(vm_ready(&mut vm), Ok(Value::Int(7)));
    let collected = vm.collect(&[]);
    assert_eq!(collected.live, 4);
    assert!(collected.reclaimed > 0);
    assert_eq!(
        vm.start(scalar, vec![transient]).unwrap_err().kind,
        ErrorKind::StaleHandle
    );
    vm.close();
    assert_eq!(vm.heap_stats().live, 0);
    assert_eq!(vm.heap_stats().slots, 0);
    assert_eq!(
        vm.start(entry, vec![temporary]).unwrap_err().kind,
        ErrorKind::Closed
    );
}

#[test]
fn immutable_program_shares_no_alias_handles_between_evaluators() {
    let program = Arc::new(program("typealias Callback<T> = (T) -> T"));
    let mut left = Vm::new(program.clone());
    let mut right = Vm::new(program.clone());
    left.start(program.entry, vec![]).unwrap();
    let left_module = vm_ready(&mut left).unwrap();
    right.start(program.entry, vec![]).unwrap();
    let right_module = vm_ready(&mut right).unwrap();
    let left_alias = member(&mut left, &left_module, "Callback").unwrap();
    let right_alias = member(&mut right, &right_module, "Callback").unwrap();
    assert!(matches!(left_alias, Value::TypeAlias(_)));
    assert!(matches!(right_alias, Value::TypeAlias(_)));
    assert_ne!(left_alias, right_alias);
    assert_eq!(
        right
            .start(program.entry, vec![left_alias.clone()])
            .unwrap_err()
            .kind,
        ErrorKind::StaleHandle
    );
    left.start(program.entry, vec![]).unwrap();
    let second_module = vm_ready(&mut left).unwrap();
    // Program/template identity is a VM contract, not a module-loader claim.
    assert_eq!(
        member(&mut left, &second_module, "Callback"),
        Ok(left_alias)
    );
    assert_eq!(left.collect(&[]).live, 2);
    assert_eq!(right.collect(&[]).live, 2);
    left.close();
    right.close();
    assert_eq!(left.heap_stats().slots, 0);
    assert_eq!(right.heap_stats().slots, 0);
}

const HOST_SOURCE: &str = r#"
typealias Callback<T> = (T) -> T
typealias Nested<T> = List<Mapping<String, Listing<T>>>
outerResult = let (captured = new Dynamic { value = 40 })
  let (callback: Callback<Int>(read("mem:outer") == "ok") = (x) -> captured.value + x)
    callback.apply(2)
innerResult = let (values: Nested<String(read("mem:inner") == "ok")> =
  List(new Mapping { ["key"] = new Listing { "inner" } })) values[0]["key"][0]
aliasIdentity = Callback
badArity: Callback<Int> = (x, y) -> x + y
"#;

fn setup() -> (HostSession, Value, [usize; 4]) {
    let mut program = program(HOST_SOURCE);
    let module_entry = program.entry;
    let entries = ["outerResult", "innerResult", "aliasIdentity", "badArity"].map(|name| {
        let entry = program.blocks.len();
        program
            .blocks
            .push(vec![I::Input(0), I::GetMember(name.into()), I::Return]);
        entry
    });
    let mut session = HostSession::new(Arc::new(program));
    session.start(module_entry, vec![], 0, None).unwrap();
    let module = ready(&mut session, 0).unwrap();
    (session, module, entries)
}

fn event(session: &mut HostSession, now: u64) -> HostPoll {
    for _ in 0..50_000 {
        match session.poll(now, 1) {
            HostPoll::Yielded => {
                session.collect(&[]);
            }
            event => return event,
        }
    }
    panic!("Host evaluation failed to settle")
}

fn ready(session: &mut HostSession, now: u64) -> Result<Value, RuntimeError> {
    match event(session, now) {
        HostPoll::Ready(result) => result,
        event => panic!("expected completed context, got {event:?}"),
    }
}

fn request(session: &mut HostSession, now: u64, expected_uri: &str) -> String {
    match event(session, now) {
        HostPoll::Read { token, uri } => {
            assert_eq!(&*uri, expected_uri);
            token
        }
        event => panic!("expected {expected_uri}, got {event:?}"),
    }
}

#[test]
fn generic_function_and_nested_alias_checks_keep_captures_across_gc_and_host_reentry() {
    let (mut session, module, [outer_entry, inner_entry, alias_entry, bad_arity]) = setup();
    session
        .start(alias_entry, vec![module.clone()], 0, None)
        .unwrap();
    let alias = ready(&mut session, 0).unwrap();
    session
        .start(outer_entry, vec![module.clone()], 1, None)
        .unwrap();
    let outer = request(&mut session, 1, "mem:outer");
    let suspended = session.collect(&[]).live;
    session
        .reenter(&outer, inner_entry, vec![module.clone()], 2, None)
        .unwrap();
    let inner = request(&mut session, 2, "mem:inner");
    assert!(session.collect(&[]).live > suspended);
    session
        .reenter(&inner, alias_entry, vec![module.clone()], 2, None)
        .unwrap();
    assert_eq!(ready(&mut session, 2), Ok(alias));
    session
        .reenter(&inner, bad_arity, vec![module], 2, None)
        .unwrap();
    assert_eq!(ready(&mut session, 2).unwrap_err().kind, ErrorKind::Type);
    assert_eq!(request(&mut session, 2, "mem:inner"), inner);
    session.resume(&inner, Ok(Value::string("ok")), 3).unwrap();
    assert_eq!(ready(&mut session, 3), Ok(Value::string("inner")));
    assert_eq!(session.context_depth(), 1);
    assert_eq!(request(&mut session, 3, "mem:outer"), outer);
    session.collect(&[]);
    session.resume(&outer, Ok(Value::string("ok")), 4).unwrap();
    assert_eq!(ready(&mut session, 4), Ok(Value::Int(42)));
    assert_eq!(session.context_depth(), 0);
    assert!(session.collect(&[]).live < suspended);
    session.close();
    assert_eq!(session.heap_stats().slots, 0);
    assert!(session.drain_cancelled_reads().is_empty());
}

#[test]
fn ordinary_alias_check_failures_retry_without_losing_identity_or_parent_cursor() {
    let (mut session, module, [outer_entry, inner_entry, alias_entry, _]) = setup();
    session
        .start(outer_entry, vec![module.clone()], 1, None)
        .unwrap();
    let outer = request(&mut session, 1, "mem:outer");
    let mut previous: Option<String> = None;
    for (now, response) in [
        (
            2,
            Err(RuntimeError::new(ErrorKind::Host, "metadata retry\0\r\n🙂")),
        ),
        (3, Ok(Value::string("wrong"))),
        (4, Ok(Value::string("ok"))),
    ] {
        session
            .reenter(&outer, inner_entry, vec![module.clone()], now, None)
            .unwrap();
        let inner = request(&mut session, now, "mem:inner");
        if let Some(previous) = &previous {
            assert_ne!(&inner, previous);
            assert_eq!(
                session
                    .resume(previous, Ok(Value::Null), now)
                    .unwrap_err()
                    .kind,
                ErrorKind::Host
            );
        }
        session.collect(&[]);
        session.resume(&inner, response, now).unwrap();
        let result = ready(&mut session, now);
        match now {
            2 => {
                let error = result.unwrap_err();
                assert_eq!(error.kind, ErrorKind::Host);
                assert_eq!(error.message, "metadata retry\0\r\n🙂");
            }
            3 => assert_eq!(result.unwrap_err().kind, ErrorKind::Type),
            4 => assert_eq!(result, Ok(Value::string("inner"))),
            _ => unreachable!(),
        }
        assert_eq!(request(&mut session, now, "mem:outer"), outer);
        previous = Some(inner);
    }
    session
        .reenter(&outer, alias_entry, vec![module], 5, None)
        .unwrap();
    assert!(matches!(ready(&mut session, 5), Ok(Value::TypeAlias(_))));
    session.resume(&outer, Ok(Value::string("ok")), 5).unwrap();
    assert_eq!(ready(&mut session, 5), Ok(Value::Int(42)));
    session.close();
    assert_eq!(session.heap_stats().slots, 0);
}

#[test]
fn terminal_actions_release_aliases_function_captures_and_all_pending_metadata_reads() {
    for termination in [ErrorKind::Cancelled, ErrorKind::Closed, ErrorKind::Timeout] {
        let (mut session, module, [outer_entry, inner_entry, _, _]) = setup();
        session
            .start(
                outer_entry,
                vec![module.clone()],
                1,
                (termination == ErrorKind::Timeout).then_some(5),
            )
            .unwrap();
        let outer = request(&mut session, 1, "mem:outer");
        session
            .reenter(&outer, inner_entry, vec![module], 2, None)
            .unwrap();
        let inner = request(&mut session, 2, "mem:inner");
        assert_eq!(session.context_depth(), 2);
        assert!(session.collect(&[]).live > 0);
        match termination {
            ErrorKind::Cancelled => session.cancel(),
            ErrorKind::Closed => session.close(),
            ErrorKind::Timeout => {}
            _ => unreachable!(),
        }
        let now = if termination == ErrorKind::Timeout {
            6
        } else {
            3
        };
        assert_eq!(ready(&mut session, now).unwrap_err().kind, termination);
        assert!(session.is_closed());
        assert_eq!(session.context_depth(), 0);
        assert_eq!(session.heap_stats().live, 0);
        assert_eq!(session.heap_stats().slots, 0);
        assert_eq!(
            session.drain_cancelled_reads(),
            [outer.clone(), inner.clone()]
        );
        for token in [outer, inner] {
            assert_eq!(
                session
                    .resume(&token, Ok(Value::Null), now)
                    .unwrap_err()
                    .kind,
                ErrorKind::Closed
            );
        }
        session.close();
        assert!(session.drain_cancelled_reads().is_empty());
    }
}

#[test]
fn development_cancel_retries_function_type_checks_and_preserves_alias_identity() {
    let mut program = program(HOST_SOURCE);
    let scalar = program.blocks.len();
    program
        .blocks
        .push(vec![I::Constant(Value::Int(0)), I::Return]);
    let entry = program.entry;
    let mut vm = Vm::new(Arc::new(program));
    vm.start(entry, vec![]).unwrap();
    let module = vm_ready(&mut vm).unwrap();
    let alias = member(&mut vm, &module, "Callback").unwrap();
    vm.start_member(module.as_object().unwrap(), "outerResult")
        .unwrap();
    let pending = loop {
        match vm.poll(1) {
            VmPoll::Pending(read) => break read.id,
            VmPoll::Yielded => {
                vm.collect(&[]);
            }
            event => panic!("expected function constraint read, got {event:?}"),
        }
    };
    let suspended = vm.collect(&[]).live;
    vm.cancel();
    assert_eq!(vm_ready(&mut vm).unwrap_err().kind, ErrorKind::Cancelled);
    assert!(!vm.is_closed());
    assert_eq!(
        vm.member_state(module.as_object().unwrap(), "outerResult")
            .unwrap(),
        LazyState::Unevaluated
    );
    assert_eq!(
        vm.resume(pending, Ok(Value::Null)).unwrap_err().kind,
        ErrorKind::Host
    );
    vm.start(scalar, vec![]).unwrap();
    assert_eq!(vm_ready(&mut vm), Ok(Value::Int(0)));
    assert!(vm.collect(&[]).live < suspended);
    assert_eq!(member(&mut vm, &module, "Callback"), Ok(alias));
    vm.close();
    assert_eq!(vm.heap_stats().slots, 0);
}
