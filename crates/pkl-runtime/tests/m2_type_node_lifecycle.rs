//! Persistent executable-node ownership under GC and Host lifecycle transitions.
//! These native lifecycle checks are separate from raw Java compatibility evidence.
use std::sync::Arc;

use pkl_runtime::heap::LazyState;
use pkl_runtime::host::{HostPoll, HostSession};
use pkl_runtime::parser::parse_module;
use pkl_runtime::resolver::resolve_module;
use pkl_runtime::value::{ErrorKind, RuntimeError, Value};
use pkl_runtime::vm::{Instruction as I, Program, Vm, VmPoll, compile_module};

const SOURCE: &str = r#"
local threshold = 0
typealias A = Int(read("mem:gate") == "ok" && this > threshold)
typealias L = Int((n) -> read("mem:lambda") == "ok" && n > threshold)
typealias T = Int(let (n = this) read("mem:let") == "ok" && n > threshold)
same = (n) -> n is A
other = (n) -> n is A
lambdaLeft = (n) -> n is L
lambdaRight = (n) -> n is L
letLeft = (n) -> n is T
letRight = (n) -> n is T
identity = A
wrappedLeft: Listing<A> = new Listing { 2; 3 }
wrappedRight: Listing<A> = new Listing { 4; 5 }
wrapperFirst = wrappedLeft[0]
wrapperSecond = wrappedLeft[1]
wrapperOther = wrappedRight[0]
make = (warm) -> let (ignored = if (warm) 0 is Int(true) else true) (n) -> n is A
warmed = make.apply(true)
cold = make.apply(false)
"#;

fn compile(source: &str) -> Program {
    (*compile_module(&resolve_module(&parse_module(source).unwrap()).unwrap()).unwrap()).clone()
}

fn program() -> (Arc<Program>, [usize; 6]) {
    let mut program = compile(SOURCE);
    // These adapter blocks call source-compiled closures. They do not implement
    // checks, retries, const lookup, or resource reads themselves.
    let entries = [
        "same",
        "other",
        "lambdaLeft",
        "lambdaRight",
        "letLeft",
        "letRight",
    ]
    .map(|name| {
        let entry = program.blocks.len();
        program.blocks.push(vec![
            I::Input(0),
            I::GetMember(name.into()),
            I::Input(1),
            I::Call {
                arity: 1,
                method: false,
            },
            I::Return,
        ]);
        entry
    });
    (Arc::new(program), entries)
}

fn vm_event(vm: &mut Vm) -> VmPoll {
    for _ in 0..50_000 {
        match vm.poll(1) {
            VmPoll::Yielded => {
                vm.collect(&[]);
            }
            event => return event,
        }
    }
    panic!("type-node VM evaluation did not settle")
}

fn vm_ready(vm: &mut Vm) -> Result<Value, RuntimeError> {
    match vm_event(vm) {
        VmPoll::Ready(result) => result,
        event => panic!("expected completed VM context, got {event:?}"),
    }
}

fn vm_read(vm: &mut Vm, uri: &str) -> u64 {
    match vm_event(vm) {
        VmPoll::Pending(read) => {
            assert_eq!(&*read.uri, uri);
            vm.collect(&[]);
            read.id
        }
        event => panic!("expected {uri}, got {event:?}"),
    }
}

fn host_event(session: &mut HostSession, now: u64) -> HostPoll {
    for _ in 0..50_000 {
        match session.poll(now, 1) {
            HostPoll::Yielded => {
                session.collect(&[]);
            }
            event => return event,
        }
    }
    panic!("type-node Host evaluation did not settle")
}

fn host_ready(session: &mut HostSession, now: u64) -> Result<Value, RuntimeError> {
    match host_event(session, now) {
        HostPoll::Ready(result) => result,
        event => panic!("expected completed Host context, got {event:?}"),
    }
}

fn host_read(session: &mut HostSession, now: u64, expected: &str) -> String {
    match host_event(session, now) {
        HostPoll::Read { token, uri } => {
            assert_eq!(&*uri, expected);
            session.collect(&[]);
            token
        }
        event => panic!("expected {expected}, got {event:?}"),
    }
}

fn assert_const_error(result: Result<Value, RuntimeError>) {
    let error = result.unwrap_err();
    assert_eq!(error.kind, ErrorKind::Type);
    assert_eq!(
        error.message,
        "Cannot reference property `threshold` from here because it is not `const`."
    );
}

fn host_setup() -> (HostSession, Value, [usize; 6]) {
    let (program, entries) = program();
    let entry = program.entry;
    let mut session = HostSession::new(program);
    session.start(entry, vec![], 0, None).unwrap();
    let module = host_ready(&mut session, 0).unwrap();
    (session, module, entries)
}

#[test]
fn same_site_reentry_publishes_local_resolution_without_completing_parent_value() {
    let (mut session, module, [same, _, _, _, _, _]) = host_setup();
    session
        .start(same, vec![module.clone(), Value::Int(2.into())], 1, None)
        .unwrap();
    let parent = host_read(&mut session, 1, "mem:gate");
    session
        .reenter(
            &parent,
            same,
            vec![module.clone(), Value::Int(3.into())],
            2,
            None,
        )
        .unwrap();
    let child = host_read(&mut session, 2, "mem:gate");
    session.resume(&child, Ok(Value::string("ok")), 2).unwrap();
    assert_const_error(host_ready(&mut session, 2));
    assert_eq!(host_read(&mut session, 2, "mem:gate"), parent);
    // The nested failure published this executable site's local member resolution.
    session.resume(&parent, Ok(Value::string("ok")), 3).unwrap();
    assert_eq!(host_ready(&mut session, 3), Ok(Value::Bool(true)));
    assert_eq!(session.context_depth(), 0);
    assert_eq!(
        session.resume(&child, Ok(Value::Null), 3).unwrap_err().kind,
        ErrorKind::Host
    );
    session.close();
    assert_eq!(session.heap_stats().slots, 0);
}

#[test]
fn different_inline_and_lambda_sites_keep_resolution_independent_but_let_root_shares() {
    let (mut session, module, [same, other, lambda_left, lambda_right, let_left, let_right]) =
        host_setup();
    for (left, right, uri, shared) in [
        (same, other, "mem:gate", false),
        (lambda_left, lambda_right, "mem:lambda", false),
        (let_left, let_right, "mem:let", true),
    ] {
        for (entry, must_fail) in [
            (left, true),
            (left, false),
            (right, !shared),
            (right, false),
        ] {
            session
                .start(entry, vec![module.clone(), Value::Int(2.into())], 1, None)
                .unwrap();
            let token = host_read(&mut session, 1, uri);
            session.resume(&token, Ok(Value::string("ok")), 1).unwrap();
            let result = host_ready(&mut session, 1);
            if must_fail {
                assert_const_error(result);
            } else {
                assert_eq!(result, Ok(Value::Bool(true)));
            }
        }
    }
    session.close();
    assert_eq!(session.heap_stats().slots, 0);
}

#[test]
fn failed_different_site_reentry_does_not_publish_parent_site_resolution() {
    let (mut session, module, [same, other, _, _, _, _]) = host_setup();
    session
        .start(same, vec![module.clone(), Value::Int(2.into())], 1, None)
        .unwrap();
    let parent = host_read(&mut session, 1, "mem:gate");
    for must_fail in [true, false] {
        session
            .reenter(
                &parent,
                other,
                vec![module.clone(), Value::Int(3.into())],
                2,
                None,
            )
            .unwrap();
        let child = host_read(&mut session, 2, "mem:gate");
        session.resume(&child, Ok(Value::string("ok")), 2).unwrap();
        let result = host_ready(&mut session, 2);
        if must_fail {
            assert_const_error(result);
        } else {
            assert_eq!(result, Ok(Value::Bool(true)));
        }
        assert_eq!(host_read(&mut session, 2, "mem:gate"), parent);
    }
    session.resume(&parent, Ok(Value::string("ok")), 3).unwrap();
    assert_const_error(host_ready(&mut session, 3));
    session.close();
    assert_eq!(session.heap_stats().slots, 0);
}

#[test]
fn host_failure_and_nonterminal_cancel_preserve_only_published_node_state() {
    let (program, [same, _, _, _, _, _]) = program();
    let mut vm = Vm::new(program.clone());
    vm.start(program.entry, vec![]).unwrap();
    let module = vm_ready(&mut vm).unwrap();
    let mut old = Vec::new();
    // Reads fail or cancel before reaching local lookup, so neither should skip
    // the first const error when the source finally reaches that lookup.
    for cancel in [false, true] {
        vm.start(same, vec![module.clone(), Value::Int(2.into())])
            .unwrap();
        let token = vm_read(&mut vm, "mem:gate");
        if cancel {
            vm.cancel();
            assert_eq!(vm_ready(&mut vm).unwrap_err().kind, ErrorKind::Cancelled);
            assert!(!vm.is_closed());
        } else {
            vm.resume(
                token,
                Err(RuntimeError::new(ErrorKind::Host, "node retry\0\r\n🙂")),
            )
            .unwrap();
            let error = vm_ready(&mut vm).unwrap_err();
            assert_eq!(error.kind, ErrorKind::Host);
            assert_eq!(error.message, "node retry\0\r\n🙂");
        }
        old.push(token);
    }
    vm.start(same, vec![module.clone(), Value::Int(2.into())])
        .unwrap();
    let token = vm_read(&mut vm, "mem:gate");
    vm.resume(token, Ok(Value::string("ok"))).unwrap();
    assert_const_error(vm_ready(&mut vm));
    // Published nodes are persistent; canceled activations and their receiver
    // slots are temporary. Warm this reached prefix once, then require exactly
    // the same retained graph after each canceled call, including GC at yields.
    let retained = vm.collect(std::slice::from_ref(&module)).live;
    for _ in 0..32 {
        vm.start(same, vec![module.clone(), Value::Int(2.into())])
            .unwrap();
        let token = vm_read(&mut vm, "mem:gate");
        vm.cancel();
        assert_eq!(vm_ready(&mut vm).unwrap_err().kind, ErrorKind::Cancelled);
        assert_eq!(vm.collect(std::slice::from_ref(&module)).live, retained);
        old.push(token);
    }
    vm.start(same, vec![module, Value::Int(2.into())]).unwrap();
    let token = vm_read(&mut vm, "mem:gate");
    for stale in old {
        assert_ne!(stale, token);
        assert_eq!(
            vm.resume(stale, Ok(Value::Null)).unwrap_err().kind,
            ErrorKind::Host
        );
    }
    vm.resume(token, Ok(Value::string("ok"))).unwrap();
    assert_eq!(vm_ready(&mut vm), Ok(Value::Bool(true)));
    vm.close();
    assert_eq!(vm.heap_stats().slots, 0);
}

#[test]
fn recursive_failed_prefix_is_reused_and_fixed_depth_retries_do_not_grow_nodes() {
    let mut program = compile(
        r#"
local threshold = 0
typealias A = Int(this < 0 || ((this - 1) is A && this < threshold))
value: A = 2
check = (n) -> n is A
stable = 42
"#,
    );
    let call = program.blocks.len();
    program.blocks.push(vec![
        I::Input(0),
        I::GetMember("check".into()),
        I::Input(1),
        I::Call {
            arity: 1,
            method: false,
        },
        I::Return,
    ]);
    let program = Arc::new(program);
    let mut vm = Vm::new(program.clone());
    vm.start(program.entry, vec![]).unwrap();
    let module = vm_ready(&mut vm).unwrap();
    let object = module.as_object().unwrap();
    vm.start_member(object, "stable").unwrap();
    assert_eq!(vm_ready(&mut vm), Ok(Value::Int(42.into())));
    for attempt in 0..6 {
        vm.start_member(object, "value").unwrap();
        let result = vm_ready(&mut vm);
        if attempt < 3 {
            assert_const_error(result);
            assert_eq!(
                vm.member_state(object, "value").unwrap(),
                LazyState::Unevaluated
            );
        } else {
            assert_eq!(result, Ok(Value::Int(2.into())));
        }
    }
    // A separate function type use owns its own recursively discovered prefix.
    for _ in 0..3 {
        vm.start(call, vec![module.clone(), Value::Int(2.into())])
            .unwrap();
        assert_const_error(vm_ready(&mut vm));
    }
    vm.start(call, vec![module.clone(), Value::Int(2.into())])
        .unwrap();
    assert_eq!(vm_ready(&mut vm), Ok(Value::Bool(true)));
    let retained = vm.collect(std::slice::from_ref(&module)).live;
    for _ in 0..32 {
        vm.start(call, vec![module.clone(), Value::Int(2.into())])
            .unwrap();
        assert_eq!(vm_ready(&mut vm), Ok(Value::Bool(true)));
        assert_eq!(vm.collect(std::slice::from_ref(&module)).live, retained);
    }
    vm.start_member(object, "stable").unwrap();
    assert_eq!(vm_ready(&mut vm), Ok(Value::Int(42.into())));
    vm.close();
    assert_eq!(vm.heap_stats().slots, 0);
}

#[test]
fn lazy_wrapper_argument_node_survives_failure_and_is_shared_across_members() {
    let (program, _) = program();
    let mut vm = Vm::new(program.clone());
    vm.start(program.entry, vec![]).unwrap();
    let module = vm_ready(&mut vm).unwrap();
    let object = module.as_object().unwrap();
    for (name, expected) in [
        ("wrapperFirst", None),
        ("wrapperSecond", Some(3)),
        ("wrapperOther", None),
        ("wrapperFirst", Some(2)),
        ("wrapperOther", Some(4)),
    ] {
        vm.start_member(object, name).unwrap();
        let read = vm_read(&mut vm, "mem:gate");
        vm.resume(read, Ok(Value::string("ok"))).unwrap();
        let result = vm_ready(&mut vm);
        if let Some(value) = expected {
            assert_eq!(result, Ok(Value::Int(value.into())));
        } else {
            assert_const_error(result);
            assert_eq!(
                vm.member_state(object, name).unwrap(),
                LazyState::Unevaluated
            );
        }
    }
    vm.close();
    assert_eq!(vm.heap_stats().slots, 0);
}

#[test]
fn shared_program_local_resolution_and_heap_handles_are_evaluator_local() {
    let (program, [same, _, _, _, _, _]) = program();
    let mut left = Vm::new(program.clone());
    let mut right = Vm::new(program.clone());
    left.start(program.entry, vec![]).unwrap();
    let left_module = vm_ready(&mut left).unwrap();
    right.start(program.entry, vec![]).unwrap();
    let right_module = vm_ready(&mut right).unwrap();
    for (vm, module) in [(&mut left, &left_module), (&mut right, &right_module)] {
        vm.start(same, vec![module.clone(), Value::Int(2.into())])
            .unwrap();
        let token = vm_read(vm, "mem:gate");
        vm.resume(token, Ok(Value::string("ok"))).unwrap();
        assert_const_error(vm_ready(vm));
    }
    assert_eq!(
        right
            .start(same, vec![left_module.clone(), Value::Int(2.into())])
            .unwrap_err()
            .kind,
        ErrorKind::StaleHandle
    );
    right.close();
    assert_eq!(right.heap_stats().slots, 0);
    left.start(same, vec![left_module, Value::Int(2.into())])
        .unwrap();
    let token = vm_read(&mut left, "mem:gate");
    left.resume(token, Ok(Value::string("ok"))).unwrap();
    assert_eq!(vm_ready(&mut left), Ok(Value::Bool(true)));
    left.close();
    assert_eq!(left.heap_stats().slots, 0);
}

fn branch_program() -> (Arc<Program>, [usize; 2]) {
    let mut program = compile(SOURCE);
    let entries = ["warmed", "cold"].map(|name| {
        let entry = program.blocks.len();
        program.blocks.push(vec![
            I::Input(0),
            I::GetMember(name.into()),
            I::Input(1),
            I::Call {
                arity: 1,
                method: false,
            },
            I::Return,
        ]);
        entry
    });
    (Arc::new(program), entries)
}

#[test]
fn earlier_branch_checks_do_not_split_later_lambda_owners_during_host_reentry() {
    for reverse in [false, true] {
        let (program, mut entries) = branch_program();
        if reverse {
            entries.reverse();
        }
        let [first, second] = entries;
        let entry = program.entry;
        let mut session = HostSession::new(program);
        session.start(entry, vec![], 0, None).unwrap();
        let module = host_ready(&mut session, 0).unwrap();
        session
            .start(first, vec![module.clone(), Value::Int(2.into())], 1, None)
            .unwrap();
        let initial = host_read(&mut session, 1, "mem:gate");
        session
            .resume(&initial, Ok(Value::string("ok")), 1)
            .unwrap();
        assert_const_error(host_ready(&mut session, 1));
        session
            .start(first, vec![module.clone(), Value::Int(2.into())], 2, None)
            .unwrap();
        let parent = host_read(&mut session, 2, "mem:gate");
        session
            .reenter(
                &parent,
                second,
                vec![module.clone(), Value::Int(3.into())],
                3,
                None,
            )
            .unwrap();
        let failed = host_read(&mut session, 3, "mem:gate");
        session
            .resume(
                &failed,
                Err(RuntimeError::new(ErrorKind::Host, "branch retry")),
                3,
            )
            .unwrap();
        assert_eq!(
            host_ready(&mut session, 3).unwrap_err().message,
            "branch retry"
        );
        assert_eq!(host_read(&mut session, 3, "mem:gate"), parent);
        session
            .reenter(&parent, second, vec![module, Value::Int(3.into())], 4, None)
            .unwrap();
        let retry = host_read(&mut session, 4, "mem:gate");
        assert_eq!(
            session
                .resume(&failed, Ok(Value::Null), 4)
                .unwrap_err()
                .kind,
            ErrorKind::Host
        );
        session.resume(&retry, Ok(Value::string("ok")), 4).unwrap();
        // Both closures come from one source lambda, irrespective of whether
        // the earlier branch in their factory reached an unrelated type check.
        assert_eq!(host_ready(&mut session, 4), Ok(Value::Bool(true)));
        assert_eq!(host_read(&mut session, 4, "mem:gate"), parent);
        session.resume(&parent, Ok(Value::string("ok")), 5).unwrap();
        assert_eq!(host_ready(&mut session, 5), Ok(Value::Bool(true)));
        session.close();
        assert_eq!(session.heap_stats().slots, 0);
    }
}

#[test]
fn branch_independent_lambda_cache_survives_cancellation_without_graph_growth() {
    let (program, [first, second]) = branch_program();
    let mut vm = Vm::new(program.clone());
    vm.start(program.entry, vec![]).unwrap();
    let module = vm_ready(&mut vm).unwrap();
    vm.start(first, vec![module.clone(), Value::Int(2.into())])
        .unwrap();
    let read = vm_read(&mut vm, "mem:gate");
    vm.resume(read, Ok(Value::string("ok"))).unwrap();
    assert_const_error(vm_ready(&mut vm));
    let mut retained = None;
    for _ in 0..16 {
        vm.start(second, vec![module.clone(), Value::Int(3.into())])
            .unwrap();
        let read = vm_read(&mut vm, "mem:gate");
        vm.cancel();
        assert_eq!(vm_ready(&mut vm).unwrap_err().kind, ErrorKind::Cancelled);
        assert!(!vm.is_closed());
        let live = vm.collect(std::slice::from_ref(&module)).live;
        if let Some(expected) = retained {
            assert_eq!(live, expected);
        } else {
            retained = Some(live);
        }
        assert_eq!(
            vm.resume(read, Ok(Value::Null)).unwrap_err().kind,
            ErrorKind::Host
        );
    }
    vm.start(second, vec![module, Value::Int(3.into())])
        .unwrap();
    let read = vm_read(&mut vm, "mem:gate");
    vm.resume(read, Ok(Value::string("ok"))).unwrap();
    assert_eq!(vm_ready(&mut vm), Ok(Value::Bool(true)));
    vm.close();
    assert_eq!(vm.heap_stats().slots, 0);
}

#[test]
fn terminal_events_release_node_graphs_and_both_pending_contexts() {
    for kind in [ErrorKind::Cancelled, ErrorKind::Closed, ErrorKind::Timeout] {
        let (mut session, module, [same, other, _, _, _, _]) = host_setup();
        session
            .start(
                same,
                vec![module.clone(), Value::Int(2.into())],
                1,
                (kind == ErrorKind::Timeout).then_some(5),
            )
            .unwrap();
        let parent = host_read(&mut session, 1, "mem:gate");
        session
            .reenter(&parent, other, vec![module, Value::Int(3.into())], 2, None)
            .unwrap();
        let child = host_read(&mut session, 2, "mem:gate");
        assert_eq!(session.context_depth(), 2);
        match kind {
            ErrorKind::Cancelled => session.cancel(),
            ErrorKind::Closed => session.close(),
            ErrorKind::Timeout => {}
            _ => unreachable!(),
        }
        let now = if kind == ErrorKind::Timeout { 6 } else { 3 };
        assert_eq!(host_ready(&mut session, now).unwrap_err().kind, kind);
        assert!(session.is_closed());
        assert_eq!(session.context_depth(), 0);
        assert_eq!(session.heap_stats().live, 0);
        assert_eq!(session.heap_stats().slots, 0);
        assert_eq!(
            session.drain_cancelled_reads(),
            [parent.clone(), child.clone()]
        );
        for token in [parent, child] {
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
