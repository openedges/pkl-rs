//! Strict Set builders and hash/equality work retain roots across VM and Host transitions.
//! These native lifetime checks are separate from pinned Java byte comparisons.
//! Retained module graphs include their one builtin Module parent; executable
//! nodes, transient-object deltas and terminal-zero assertions remain separate.
use std::process::{Command, Stdio};
use std::sync::Arc;
#[path = "support/eager_base.rs"]
mod eager_base;
use eager_base::Baseline;
use std::time::{Duration, Instant};

use pkl_runtime::heap::LazyState;
use pkl_runtime::host::{HostPoll, HostSession};
use pkl_runtime::parser::parse_module;
use pkl_runtime::resolver::resolve_module;
use pkl_runtime::value::{ErrorKind, RuntimeError, Value};
use pkl_runtime::vm::{Instruction as I, Program, Vm, VmPoll, compile_module};

const SOURCE: &str = r#"
parent = read("mem:outer")
set = Set(new Dynamic { p = 3 }, Set(new Listing { read("mem:key") }), read("mem:next"))
child = set.contains(Set(new Listing { 7 }))
inspect = set.length
"#;

fn program(source: &str) -> (Arc<Program>, [usize; 3]) {
    let mut program = (*compile_module(&resolve_module(&parse_module(source).unwrap()).unwrap())
        .unwrap())
    .clone();
    let entries = ["parent", "child", "inspect"].map(|name| {
        let block = program.blocks.len();
        program
            .blocks
            .push(vec![I::Input(0), I::GetMember(name.into()), I::Return]);
        block
    });
    (Arc::new(program), entries)
}

fn vm_event(vm: &mut Vm) -> VmPoll {
    for _ in 0..200_000 {
        match vm.poll(1) {
            VmPoll::Yielded => {
                vm.collect(&[]);
            }
            event => return event,
        }
    }
    panic!("strict Set did not settle")
}

fn vm_ready(vm: &mut Vm) -> Result<Value, RuntimeError> {
    match vm_event(vm) {
        VmPoll::Ready(result) => result,
        event => panic!("expected VM result, got {event:?}"),
    }
}

fn vm_read(vm: &mut Vm, expected: &str) -> u64 {
    match vm_event(vm) {
        VmPoll::Pending(read) => {
            assert_eq!(&*read.uri, expected);
            vm.collect(&[]);
            read.id
        }
        event => panic!("expected {expected}, got {event:?}"),
    }
}

fn host_event(session: &mut HostSession, now: u64) -> HostPoll {
    for _ in 0..200_000 {
        match session.poll(now, 1) {
            HostPoll::Yielded => {
                session.collect(&[]);
            }
            event => return event,
        }
    }
    panic!("strict Set Host did not settle")
}

fn host_ready(session: &mut HostSession, now: u64) -> Result<Value, RuntimeError> {
    match host_event(session, now) {
        HostPoll::Ready(result) => result,
        event => panic!("expected Host result, got {event:?}"),
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

fn setup() -> (HostSession, Value, [usize; 3]) {
    let (program, entries) = program(SOURCE);
    let entry = program.entry;
    let mut session = HostSession::new(program);
    session.start(entry, vec![], 0, None).unwrap();
    let module = host_ready(&mut session, 0).unwrap();
    (session, module, entries)
}

#[test]
fn item_hash_suspends_before_next_argument_and_duplicate_keeps_first_position() {
    let source = r#"
result = Set(let (key = read("mem:item-evaluation")) new Listing { key; read("mem:item-hash") },
  read("mem:next-item")).length
ordered = Set(new Listing { 1 }, new Listing { 2 }, new Listing { 1 })
entries = new Listing { for (item in ordered) { item[0] } }
check = entries[0] * 10 + entries[1]
hashOrdered = Map(Set(new Dynamic { p = read("mem:first") },
  new Dynamic { p = read("mem:second") }), 42).length
"#;
    let (program, _) = program(source);
    let mut vm = Vm::new(program.clone());
    vm.start(program.entry, vec![]).unwrap();
    let module = vm_ready(&mut vm).unwrap();
    let object = module.as_object().unwrap();
    vm.start_member(object, "result").unwrap();
    for (uri, response) in [
        ("mem:item-evaluation", Value::Int(1.into())),
        ("mem:item-hash", Value::Int(2.into())),
        ("mem:next-item", Value::string("retained next item")),
    ] {
        let token = vm_read(&mut vm, uri);
        vm.resume(token, Ok(response)).unwrap();
    }
    assert_eq!(vm_ready(&mut vm), Ok(Value::Int(2.into())));
    vm.start_member(object, "check").unwrap();
    assert_eq!(vm_ready(&mut vm), Ok(Value::Int(12.into())));
    vm.start_member(object, "hashOrdered").unwrap();
    for uri in ["mem:first", "mem:second"] {
        let token = vm_read(&mut vm, uri);
        vm.resume(token, Ok(Value::Int(7.into()))).unwrap();
    }
    assert_eq!(vm_ready(&mut vm), Ok(Value::Int(1.into())));
    vm.close();
    assert_eq!(vm.heap_stats().slots, 0);
}

#[test]
fn contains_query_and_comparison_operands_survive_failed_reentry_and_gc() {
    // Equality's left Set must remain rooted while the right constructor hashes
    // its lazy element. Membership retains its completed Set during query hash.
    for expression in [
        "set.contains(query)",
        "Set(new Listing { 7 }) == Set(query)",
        "Set(new Listing { 7 }) != Set(query)",
    ] {
        let source = format!(
            "parent = read(\"mem:outer\")\nset = Set(new Listing {{ 7 }})\nquery = new Listing {{ read(\"mem:query\") }}\nchild = {expression}\n"
        );
        let (program, [parent, child, _]) = program(&source);
        let entry = program.entry;
        let mut session = HostSession::new(program);
        session.start(entry, vec![], 0, None).unwrap();
        let module = host_ready(&mut session, 0).unwrap();
        let initialized = Baseline::new(session.collect(&[]));
        let stale_module = module.clone();
        session
            .start(parent, vec![module.clone()], 1, None)
            .unwrap();
        let outer = host_read(&mut session, 1, "mem:outer");
        session
            .reenter(&outer, child, vec![module.clone()], 2, None)
            .unwrap();
        let failed = host_read(&mut session, 2, "mem:query");
        let error = RuntimeError::new(ErrorKind::Host, "Set query retry\0\r\n🙂");
        session.resume(&failed, Err(error.clone()), 2).unwrap();
        let actual = host_ready(&mut session, 2).unwrap_err();
        assert_eq!((actual.kind, actual.message), (error.kind, error.message));
        assert_eq!(session.context_depth(), 1);
        assert_eq!(host_read(&mut session, 2, "mem:outer"), outer);
        session
            .reenter(&outer, child, vec![module], 3, None)
            .unwrap();
        let token = host_read(&mut session, 3, "mem:query");
        assert_ne!(token, failed);
        assert_eq!(
            session
                .resume(&failed, Ok(Value::Null), 3)
                .unwrap_err()
                .kind,
            ErrorKind::Host
        );
        session.resume(&token, Ok(Value::Int(7.into())), 3).unwrap();
        assert_eq!(
            host_ready(&mut session, 3),
            Ok(Value::Bool(!expression.contains("!=")))
        );
        session.resume(&outer, Ok(Value::Null), 3).unwrap();
        assert_eq!(host_ready(&mut session, 3), Ok(Value::Null));
        // User values are reclaimed; the private base and exact executable graph remain.
        assert_eq!(initialized.added_values(session.collect(&[]), 0), -1);
        assert_eq!(
            session
                .start(parent, vec![stale_module], 10, None)
                .unwrap_err()
                .kind,
            ErrorKind::StaleHandle
        );
        session.close();
        assert_eq!(session.heap_stats().slots, 0);
    }
}

#[test]
fn nested_set_hash_failure_retries_unpublished_builder_during_parent_reentry() {
    let (mut session, module, [parent, child, inspect]) = setup();
    let initialized = Baseline::new(session.collect(&[]));
    let stale_module = module.clone();
    session
        .start(parent, vec![module.clone()], 1, None)
        .unwrap();
    let outer = host_read(&mut session, 1, "mem:outer");
    let mut failed: Option<String> = None;
    for success in [false, true] {
        session
            .reenter(&outer, child, vec![module.clone()], 2, None)
            .unwrap();
        let key = host_read(&mut session, 2, "mem:key");
        if let Some(stale) = &failed {
            assert_ne!(&key, stale);
            assert_eq!(
                session.resume(stale, Ok(Value::Null), 2).unwrap_err().kind,
                ErrorKind::Host
            );
        }
        if success {
            session.resume(&key, Ok(Value::Int(7.into())), 2).unwrap();
            let next = host_read(&mut session, 2, "mem:next");
            session.resume(&next, Ok(Value::Int(42.into())), 2).unwrap();
            assert_eq!(host_ready(&mut session, 2), Ok(Value::Bool(true)));
        } else {
            let error = RuntimeError::new(ErrorKind::Host, "strict Set hash retry\0\r\n🙂");
            session.resume(&key, Err(error.clone()), 2).unwrap();
            let actual = host_ready(&mut session, 2).unwrap_err();
            assert_eq!((actual.kind, actual.message), (error.kind, error.message));
            failed = Some(key);
        }
        assert_eq!(session.context_depth(), 1);
        assert_eq!(host_read(&mut session, 2, "mem:outer"), outer);
    }
    session
        .reenter(&outer, inspect, vec![module], 3, None)
        .unwrap();
    assert_eq!(host_ready(&mut session, 3), Ok(Value::Int(3.into())));
    session.resume(&outer, Ok(Value::Null), 3).unwrap();
    assert_eq!(host_ready(&mut session, 3), Ok(Value::Null));
    // User values are reclaimed; the private base and exact executable graph remain.
    assert_eq!(initialized.added_values(session.collect(&[]), 0), -1);
    assert_eq!(
        session
            .start(parent, vec![stale_module], 10, None)
            .unwrap_err()
            .kind,
        ErrorKind::StaleHandle
    );
    session.close();
    assert_eq!(session.heap_stats().slots, 0);
}

#[test]
fn repeated_nonterminal_cancel_reclaims_set_builder_and_hash_operands() {
    let (program, _) = program(SOURCE);
    let mut vm = Vm::new(program.clone());
    vm.start(program.entry, vec![]).unwrap();
    let module = vm_ready(&mut vm).unwrap();
    let initialized = Baseline::new(vm.collect(std::slice::from_ref(&module)));
    let object = module.as_object().unwrap();
    let mut retained = None;
    for _ in 0..24 {
        vm.start_member(object, "child").unwrap();
        let key = vm_read(&mut vm, "mem:key");
        vm.cancel();
        assert_eq!(vm_ready(&mut vm).unwrap_err().kind, ErrorKind::Cancelled);
        assert!(!vm.is_closed());
        for name in ["child", "set"] {
            assert_eq!(
                vm.member_state(object, name).unwrap(),
                LazyState::Unevaluated
            );
        }
        assert_eq!(
            vm.resume(key, Ok(Value::Null)).unwrap_err().kind,
            ErrorKind::Host
        );
        let stats = vm.collect(std::slice::from_ref(&module));
        let live = (stats.live - stats.executable_live, stats.executable_live);
        assert_eq!(initialized.added_values(stats, 0), 0);
        if let Some(expected) = retained {
            assert_eq!(live, expected);
        } else {
            retained = Some(live);
        }
    }
    vm.start_member(object, "child").unwrap();
    let key = vm_read(&mut vm, "mem:key");
    vm.resume(key, Ok(Value::Int(7.into()))).unwrap();
    let next = vm_read(&mut vm, "mem:next");
    vm.resume(next, Ok(Value::Int(42.into()))).unwrap();
    assert_eq!(vm_ready(&mut vm), Ok(Value::Bool(true)));
    vm.close();
    assert_eq!(vm.heap_stats().slots, 0);
}

#[test]
fn sets_and_pending_hashes_are_evaluator_local_and_reclaimed_handles_stay_stale() {
    let source = "parent = Set(new Listing { 7 })\nchild = Set(new Listing { read(\"mem:leaf\") }).length\ninspect = parent.length";
    let (program, [_, child, inspect]) = program(source);
    let mut left = Vm::new(program.clone());
    let mut right = Vm::new(program.clone());
    left.start(program.entry, vec![]).unwrap();
    let left_module = vm_ready(&mut left).unwrap();
    let initialized = Baseline::new(left.collect(&[]));
    let stale_module = left_module.clone();
    right.start(program.entry, vec![]).unwrap();
    let right_module = vm_ready(&mut right).unwrap();
    left.start_member(left_module.as_object().unwrap(), "parent")
        .unwrap();
    let set = vm_ready(&mut left).unwrap();
    assert_eq!(
        right.start(inspect, vec![set.clone()]).unwrap_err().kind,
        ErrorKind::StaleHandle
    );
    left.start(child, vec![left_module.clone()]).unwrap();
    let left_read = vm_read(&mut left, "mem:leaf");
    right.start(child, vec![right_module]).unwrap();
    let right_read = vm_read(&mut right, "mem:leaf");
    right.resume(right_read, Ok(Value::Int(8.into()))).unwrap();
    assert_eq!(vm_ready(&mut right), Ok(Value::Int(1.into())));
    right.close();
    assert_eq!(right.heap_stats().slots, 0);
    left.collect(std::slice::from_ref(&left_module));
    left.resume(left_read, Ok(Value::Int(7.into()))).unwrap();
    assert_eq!(vm_ready(&mut left), Ok(Value::Int(1.into())));
    // User values are reclaimed; the private base and exact executable graph remain.
    assert_eq!(initialized.added_values(left.collect(&[]), 0), -1);
    assert_eq!(
        left.start(inspect, vec![stale_module]).unwrap_err().kind,
        ErrorKind::StaleHandle
    );
    assert_eq!(
        left.start(inspect, vec![set]).unwrap_err().kind,
        ErrorKind::StaleHandle
    );
    left.close();
    assert_eq!(left.heap_stats().slots, 0);
}

#[test]
fn terminal_events_release_parent_child_and_pending_set_builder() {
    for kind in [ErrorKind::Cancelled, ErrorKind::Closed, ErrorKind::Timeout] {
        let (mut session, module, [parent, child, _]) = setup();
        session
            .start(
                parent,
                vec![module.clone()],
                1,
                (kind == ErrorKind::Timeout).then_some(5),
            )
            .unwrap();
        let outer = host_read(&mut session, 1, "mem:outer");
        session
            .reenter(&outer, child, vec![module], 2, None)
            .unwrap();
        let key = host_read(&mut session, 2, "mem:key");
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
        assert_eq!(session.heap_stats().slots, 0);
        assert_eq!(session.context_depth(), 0);
        assert_eq!(
            session.drain_cancelled_reads(),
            [outer.clone(), key.clone()]
        );
        for token in [outer, key] {
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
fn eager_alias_type_checks_retry_cached_set_and_keep_captured_metadata_through_gc() {
    for check_type in [
        "Items<Int(read(\"mem:check\") == \"ok\")>",
        "Set<Listing<Int(read(uri) == \"ok\")>>",
    ] {
        let source = r#"
typealias Items<T> = Set<Listing<T>>
class Check {
  uri = "mem:check"
  function check(values: CHECK_TYPE) = values.length
}
set = Set(new Listing { 7 })
parent = read("mem:outer")
child = new Check {}.check(set)
inspect = set.length
function again() = new Check {}.check(set)
"#
        .replace("CHECK_TYPE", check_type);
        let (mut program, [parent, child, inspect]) = program(&source);
        let again = program.blocks.len();
        Arc::make_mut(&mut program).blocks.push(vec![
            I::Input(0),
            I::GetMethod("again".into()),
            I::Call {
                arity: 0,
                method: true,
            },
            I::Return,
        ]);
        let entry = program.entry;
        let mut session = HostSession::new(program);
        session.start(entry, vec![], 0, None).unwrap();
        let module = host_ready(&mut session, 0).unwrap();
        let initialized = Baseline::new(session.collect(&[]));
        session
            .start(parent, vec![module.clone()], 1, None)
            .unwrap();
        let outer = host_read(&mut session, 1, "mem:outer");
        session
            .reenter(&outer, child, vec![module.clone()], 2, None)
            .unwrap();
        let failed = host_read(&mut session, 2, "mem:check");
        session
            .resume(
                &failed,
                Err(RuntimeError::new(ErrorKind::Host, "retry Set alias check")),
                2,
            )
            .unwrap();
        assert_eq!(
            host_ready(&mut session, 2).unwrap_err().kind,
            ErrorKind::Host
        );
        session
            .reenter(&outer, inspect, vec![module.clone()], 3, None)
            .unwrap();
        assert_eq!(host_ready(&mut session, 3), Ok(Value::Int(1.into())));
        session
            .reenter(&outer, child, vec![module.clone()], 3, None)
            .unwrap();
        let token = host_read(&mut session, 3, "mem:check");
        assert_ne!(token, failed);
        session.resume(&token, Ok(Value::string("ok")), 3).unwrap();
        assert_eq!(host_ready(&mut session, 3), Ok(Value::Int(1.into())));
        session
            .reenter(&outer, child, vec![module.clone()], 4, None)
            .unwrap();
        assert_eq!(host_ready(&mut session, 4), Ok(Value::Int(1.into())));
        session.resume(&outer, Ok(Value::Null), 4).unwrap();
        assert_eq!(host_ready(&mut session, 4), Ok(Value::Null));
        let stats = session.collect(&[]);
        println!("Set eager check retained heap for {check_type}: {stats:?}");
        // Initialized declarations retain their module and successful caches.
        // Ordinary values and persistent executable entries have separate bounds.
        // The initial module has not forced Check or Items. Both paths retain
        // Check's class/prototype and the cached Set/Listing; only the Items
        // path also initializes its alias. Keep the exact executable bound
        // separate from these five or four additional user values.
        let (added_values, added_executables) = if check_type.starts_with("Items") {
            (5, 7)
        } else {
            (4, 5)
        };
        assert_eq!(
            initialized.added_values(stats, added_executables),
            added_values
        );
        let mut retained = None;
        for _ in 0..12 {
            session.start(again, vec![module.clone()], 5, None).unwrap();
            let token = host_read(&mut session, 5, "mem:check");
            session.resume(&token, Ok(Value::string("ok")), 5).unwrap();
            assert_eq!(host_ready(&mut session, 5), Ok(Value::Int(1.into())));
            let stats = session.collect(&[]);
            let live = (stats.live - stats.executable_live, stats.executable_live);
            if let Some(expected) = retained {
                assert_eq!(live, expected);
            } else {
                retained = Some(live);
            }
        }
        println!("Set eager check repeated-call plateau for {check_type}: {retained:?}");
        session.close();
        assert_eq!(session.heap_stats().slots, 0);
    }
}

#[test]
fn deep_nested_set_hash_yields_suspends_and_releases_on_small_stack() {
    const NAME: &str = "deep_nested_set_hash_yields_suspends_and_releases_on_small_stack";
    const DEPTH: usize = 768;
    const GC_CADENCE: usize = 128;
    const MAX_POLLS_PER_EVENT: usize = 4_000_000;
    if std::env::var("PKL_STRICT_SET_CHILD").as_deref() == Ok(NAME) {
        let source = format!(
            "function nest(n) = if (n == 0) new Listing {{ read(\"mem:leaf\") }} else Set(nest(n - 1))\nresult = Map(nest({DEPTH}), 42).length\n"
        );
        let (program, _) = program(&source);
        std::thread::Builder::new()
            .name("strict-set-256k".into())
            .stack_size(256 * 1024)
            .spawn(move || {
                // A distinct experiment condition from the ordinary tests:
                // nested Set constructors rehash their descendants repeatedly.
                // Collect periodically while retaining one-instruction polling.
                let mut polls = 0;
                let mut collections = 0;
                let mut event = |vm: &mut Vm| {
                    for _ in 0..MAX_POLLS_PER_EVENT {
                        polls += 1;
                        match vm.poll(1) {
                            VmPoll::Yielded => {
                                if polls % GC_CADENCE == 0 {
                                    vm.collect(&[]);
                                    collections += 1;
                                }
                            }
                            result => {
                                if matches!(result, VmPoll::Pending(_)) {
                                    vm.collect(&[]);
                                    collections += 1;
                                }
                                println!("Set stress event: polls={polls}, collections={collections}, gc_cadence={GC_CADENCE}");
                                return result;
                            }
                        }
                    }
                    panic!("Set stress event budget exceeded: polls={polls}, collections={collections}, gc_cadence={GC_CADENCE}, max_polls_per_event={MAX_POLLS_PER_EVENT}");
                };
                let mut vm = Vm::new(program.clone());
                vm.start(program.entry, vec![]).unwrap();
                let module = match event(&mut vm) {
                    VmPoll::Ready(Ok(module)) => module,
                    result => panic!("expected stress module, got {result:?}"),
                };
                vm.start_member(module.as_object().unwrap(), "result")
                    .unwrap();
                let token = match event(&mut vm) {
                    VmPoll::Pending(read) => {
                        assert_eq!(&*read.uri, "mem:leaf");
                        read.id
                    }
                    result => panic!("expected stress read, got {result:?}"),
                };
                assert!(vm.instructions_executed() > DEPTH as u64);
                vm.resume(token, Ok(Value::Int(7.into()))).unwrap();
                match event(&mut vm) {
                    VmPoll::Ready(value) => assert_eq!(value, Ok(Value::Int(1.into()))),
                    result => panic!("expected stress result, got {result:?}"),
                }
                vm.collect(std::slice::from_ref(&module));
                println!("Set stress final rooted collection: 1 additional collection");
                vm.close();
                assert_eq!(vm.heap_stats().slots, 0);
            })
            .unwrap()
            .join()
            .unwrap();
        println!("isolated strict Set stress completed: {NAME}");
        std::process::exit(0);
    }
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", NAME, "--nocapture"])
        .env("PKL_STRICT_SET_CHILD", NAME)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success(), "isolated strict Set stress: {status}");
            break;
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            let status = child.wait().unwrap();
            panic!("strict Set stress exceeded 60 seconds: {status}");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}
