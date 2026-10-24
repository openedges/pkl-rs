//! Strict Map builders and hash/equality work retain roots across VM and Host transitions.
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
map = Map(Map("entry", new Listing { read("mem:key") }), read("mem:value"))
child = map[Map("entry", new Listing { 7 })]
inspect = map.length
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
    panic!("strict Map did not settle")
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
    panic!("strict Map Host did not settle")
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
fn first_pair_hash_suspends_before_next_pair_and_duplicate_keeps_original_order() {
    let source = r#"
result = Map(
  let (key = read("mem:key-evaluation")) new Listing { key; read("mem:key-hash") },
  read("mem:value"), read("mem:next-key"), 9).length
ordered = Map(new Listing { 1 }, "old", new Listing { 2 }, "other", new Listing { 1 }, "new")
entries = new Listing { for (key, value in ordered) { "\(key[0]):\(value)" } }
check = entries[0] + "|" + entries[1]
hashOrdered = Map(Map(1, new Listing { read("mem:one") },
  0, new Listing { read("mem:zero") }), 42).length
"#;
    let (program, _) = program(source);
    let mut vm = Vm::new(program.clone());
    vm.start(program.entry, vec![]).unwrap();
    let module = vm_ready(&mut vm).unwrap();
    let object = module.as_object().unwrap();
    vm.start_member(object, "result").unwrap();
    for (uri, response) in [
        ("mem:key-evaluation", Value::Int(1.into())),
        ("mem:value", Value::string("retained value")),
        ("mem:key-hash", Value::Int(2.into())),
        ("mem:next-key", Value::Int(3.into())),
    ] {
        let token = vm_read(&mut vm, uri);
        vm.resume(token, Ok(response)).unwrap();
    }
    assert_eq!(vm_ready(&mut vm), Ok(Value::Int(2.into())));
    vm.start_member(object, "check").unwrap();
    assert_eq!(vm_ready(&mut vm), Ok(Value::string("1:new|2:other")));
    vm.start_member(object, "hashOrdered").unwrap();
    for uri in ["mem:zero", "mem:one"] {
        let token = vm_read(&mut vm, uri);
        vm.resume(token, Ok(Value::Int(7.into()))).unwrap();
    }
    assert_eq!(vm_ready(&mut vm), Ok(Value::Int(1.into())));
    vm.close();
    assert_eq!(vm.heap_stats().slots, 0);
}

#[test]
fn query_hash_and_map_equality_resume_after_failed_reentry_without_losing_operands() {
    for (expression, expected) in [
        ("map.containsKey(query)", Value::Bool(true)),
        ("map.getOrNull(query)", Value::Int(42.into())),
        ("map[query]", Value::Int(42.into())),
        (
            "Map(\"entry\", new Listing { read(\"mem:lookup\") }) == Map(\"entry\", new Listing { 7 })",
            Value::Bool(true),
        ),
    ] {
        let source = format!(
            "parent = read(\"mem:outer\")\nmap = Map(new Listing {{ 7 }}, 42)\nquery = new Listing {{ read(\"mem:lookup\") }}\nchild = {expression}\n"
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
        let failed = host_read(&mut session, 2, "mem:lookup");
        session
            .resume(
                &failed,
                Err(RuntimeError::new(ErrorKind::Host, "query retry")),
                2,
            )
            .unwrap();
        assert_eq!(
            host_ready(&mut session, 2).unwrap_err().kind,
            ErrorKind::Host
        );
        assert_eq!(host_read(&mut session, 2, "mem:outer"), outer);
        session
            .reenter(&outer, child, vec![module], 3, None)
            .unwrap();
        let token = host_read(&mut session, 3, "mem:lookup");
        assert_ne!(token, failed);
        session.resume(&token, Ok(Value::Int(7.into())), 3).unwrap();
        assert_eq!(host_ready(&mut session, 3), Ok(expected));
        session.resume(&outer, Ok(Value::Null), 3).unwrap();
        assert_eq!(host_ready(&mut session, 3), Ok(Value::Null));
        // User values are reclaimed; the private base and exact executable graph remain.
        assert_eq!(
            initialized.added_values(
                session.collect(&[]),
                usize::from(expression.starts_with("map."))
            ),
            -1
        );
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
fn nested_map_key_failure_retries_unpublished_builder_during_parent_reentry() {
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
        let value = host_read(&mut session, 2, "mem:value");
        session
            .resume(&value, Ok(Value::Int(42.into())), 2)
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
            assert_eq!(host_ready(&mut session, 2), Ok(Value::Int(42.into())));
        } else {
            let error = RuntimeError::new(ErrorKind::Host, "strict Map hash retry\0\r\n🙂");
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
    assert_eq!(host_ready(&mut session, 3), Ok(Value::Int(1.into())));
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
fn repeated_nonterminal_cancel_reclaims_strict_map_builder_and_hash_operands() {
    let (program, _) = program(SOURCE);
    let mut vm = Vm::new(program.clone());
    vm.start(program.entry, vec![]).unwrap();
    let module = vm_ready(&mut vm).unwrap();
    let initialized = Baseline::new(vm.collect(std::slice::from_ref(&module)));
    let object = module.as_object().unwrap();
    let mut retained = None;
    for _ in 0..24 {
        vm.start_member(object, "child").unwrap();
        let value = vm_read(&mut vm, "mem:value");
        vm.resume(value, Ok(Value::Int(42.into()))).unwrap();
        let key = vm_read(&mut vm, "mem:key");
        vm.cancel();
        assert_eq!(vm_ready(&mut vm).unwrap_err().kind, ErrorKind::Cancelled);
        assert!(!vm.is_closed());
        for name in ["child", "map"] {
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
    let value = vm_read(&mut vm, "mem:value");
    vm.resume(value, Ok(Value::Int(42.into()))).unwrap();
    let key = vm_read(&mut vm, "mem:key");
    vm.resume(key, Ok(Value::Int(7.into()))).unwrap();
    assert_eq!(vm_ready(&mut vm), Ok(Value::Int(42.into())));
    vm.close();
    assert_eq!(vm.heap_stats().slots, 0);
}

#[test]
fn warmed_listing_prefix_and_array_node_routes_survive_query_failure_and_gc() {
    // Pinned Java witnesses retain the cache-order-sensitive Listing hash:
    // warming element 1 gives a a different full hash from equal b. A bitmap
    // leaf compares them after five bits; expansion at17 occupied root slots
    // routes through an additional hash chunk and therefore misses b.
    for extra_slots in [0, 15, 16] {
        let entries = (0..extra_slots)
            .map(|key| format!(", {key}, 9"))
            .collect::<String>();
        let source = format!(
            r#"
parent = read("mem:outer")
a = new Listing {{ read("mem:first"); read("mem:second") }}
b = new Listing {{ read("mem:query-first"); read("mem:query-second") }}
child = let (warm = a[1]) Map(a, 7{entries}).getOrNull(b)
"#
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
        for (uri, value) in [("mem:second", 17), ("mem:first", 1), ("mem:query-first", 1)] {
            let token = host_read(&mut session, 2, uri);
            session
                .resume(&token, Ok(Value::Int(value.into())), 2)
                .unwrap();
        }
        let failed = host_read(&mut session, 2, "mem:query-second");
        session
            .resume(
                &failed,
                Err(RuntimeError::new(ErrorKind::Host, "prefix query retry")),
                2,
            )
            .unwrap();
        assert_eq!(
            host_ready(&mut session, 2).unwrap_err().kind,
            ErrorKind::Host
        );
        assert_eq!(host_read(&mut session, 2, "mem:outer"), outer);
        session
            .reenter(&outer, child, vec![module], 3, None)
            .unwrap();
        let token = host_read(&mut session, 3, "mem:query-second");
        assert_ne!(token, failed);
        session
            .resume(&token, Ok(Value::Int(17.into())), 3)
            .unwrap();
        assert_eq!(
            host_ready(&mut session, 3),
            Ok(if extra_slots < 16 {
                Value::Int(7.into())
            } else {
                Value::Null
            })
        );
        session.resume(&outer, Ok(Value::Null), 3).unwrap();
        assert_eq!(host_ready(&mut session, 3), Ok(Value::Null));
        // User values are reclaimed; the private base and exact executable graph remain.
        assert_eq!(initialized.added_values(session.collect(&[]), 1), -1);
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
fn strict_maps_and_pending_hashes_are_evaluator_local_and_reclaimed_handles_stay_stale() {
    let source = "parent = Map(\"entry\", new Listing { read(\"mem:leaf\") })\nchild = Map(parent, 42).length\ninspect = parent.length";
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
    let map = vm_ready(&mut left).unwrap();
    assert_eq!(
        right.start(inspect, vec![map.clone()]).unwrap_err().kind,
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
        left.start(inspect, vec![map]).unwrap_err().kind,
        ErrorKind::StaleHandle
    );
    left.close();
    assert_eq!(left.heap_stats().slots, 0);
}

#[test]
fn terminal_events_release_parent_child_and_pending_strict_map_builder() {
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
        let value = host_read(&mut session, 2, "mem:value");
        session
            .resume(&value, Ok(Value::Int(42.into())), 2)
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
        for token in [outer, key, value] {
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
fn collision_table_value_absent_from_key_order_survives_gc_and_terminal_release() {
    // Paguro's collision lookup ignores the supplied full hash, while assoc
    // checks it. With the warmed Listing hash, a is omitted from keyOrder but
    // remains a distinct internal table entry owning this lazy Dynamic value.
    let source = r#"
parent = read("mem:outer")
a = new Listing { 1; 17 }
b = new Listing { 1; 17 }
map = let (warm = a[1]) Map(b, 7, 48, 9, a, new Dynamic { p = read("mem:extra") })
inspect = if (map.length == 2 && map[b] == 7 && map[48] == 9
  && map != Map(b, 8, 48, 9)) 2 else -1
child = map.getOrNull(a).p
"#;
    for complete in [true, false] {
        let (program, [parent, child, inspect]) = program(source);
        let entry = program.entry;
        let mut session = HostSession::new(program);
        session.start(entry, vec![], 0, None).unwrap();
        let module = host_ready(&mut session, 0).unwrap();
        let initialized = Baseline::new(session.collect(&[]));
        let stale_module = module.clone();
        session
            .start(inspect, vec![module.clone()], 0, None)
            .unwrap();
        assert_eq!(host_ready(&mut session, 0), Ok(Value::Int(2.into())));
        assert!(session.collect(std::slice::from_ref(&module)).live > 0);
        session
            .start(parent, vec![module.clone()], 1, None)
            .unwrap();
        let outer = host_read(&mut session, 1, "mem:outer");
        session
            .reenter(&outer, child, vec![module], 2, None)
            .unwrap();
        let extra = host_read(&mut session, 2, "mem:extra");
        assert!(session.collect(&[]).live > 0);
        if complete {
            session
                .resume(&extra, Ok(Value::Int(42.into())), 2)
                .unwrap();
            assert_eq!(host_ready(&mut session, 2), Ok(Value::Int(42.into())));
            session.resume(&outer, Ok(Value::Null), 3).unwrap();
            assert_eq!(host_ready(&mut session, 3), Ok(Value::Null));
            // User values are reclaimed; the private base and exact executable graph remain.
            assert_eq!(initialized.added_values(session.collect(&[]), 2), -1);
            assert_eq!(
                session
                    .start(parent, vec![stale_module], 10, None)
                    .unwrap_err()
                    .kind,
                ErrorKind::StaleHandle
            );
            assert!(session.drain_cancelled_reads().is_empty());
            session.close();
        } else {
            session.close();
            assert_eq!(
                host_ready(&mut session, 3).unwrap_err().kind,
                ErrorKind::Closed
            );
            assert_eq!(
                session.drain_cancelled_reads(),
                [outer.clone(), extra.clone()]
            );
            for token in [outer, extra] {
                assert_eq!(
                    session.resume(&token, Ok(Value::Null), 3).unwrap_err().kind,
                    ErrorKind::Closed
                );
            }
        }
        assert_eq!(session.context_depth(), 0);
        assert_eq!(session.heap_stats().slots, 0);
    }
}

#[test]
fn deep_nested_map_hash_yields_suspends_and_releases_on_small_stack() {
    const NAME: &str = "deep_nested_map_hash_yields_suspends_and_releases_on_small_stack";
    const DEPTH: usize = 768;
    if std::env::var("PKL_STRICT_MAP_CHILD").as_deref() == Ok(NAME) {
        let source = format!(
            "function nest(n) = if (n == 0) new Listing {{ read(\"mem:leaf\") }} else Map(\"entry\", nest(n - 1))\nresult = Map(nest({DEPTH}), 42).length\n"
        );
        let (program, _) = program(&source);
        std::thread::Builder::new()
            .name("strict-map-256k".into())
            .stack_size(256 * 1024)
            .spawn(move || {
                let mut vm = Vm::new(program.clone());
                vm.start(program.entry, vec![]).unwrap();
                let module = vm_ready(&mut vm).unwrap();
                vm.start_member(module.as_object().unwrap(), "result")
                    .unwrap();
                let token = vm_read(&mut vm, "mem:leaf");
                assert!(vm.instructions_executed() > DEPTH as u64);
                vm.resume(token, Ok(Value::Int(7.into()))).unwrap();
                assert_eq!(vm_ready(&mut vm), Ok(Value::Int(1.into())));
                vm.collect(std::slice::from_ref(&module));
                vm.close();
                assert_eq!(vm.heap_stats().slots, 0);
            })
            .unwrap()
            .join()
            .unwrap();
        println!("isolated strict Map stress completed: {NAME}");
        std::process::exit(0);
    }
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", NAME, "--nocapture"])
        .env("PKL_STRICT_MAP_CHILD", NAME)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success(), "isolated strict Map stress: {status}");
            break;
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            let status = child.wait().unwrap();
            panic!("strict Map stress exceeded 60 seconds: {status}");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}
