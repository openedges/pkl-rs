//! Bounded native stack and lifecycle stress, separate from Java compatibility.
//! Potential aborts run in a child process with a fixed timeout.
#[path = "support/module_policy.rs"]
mod module_policy;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use pkl_runtime::heap::LazyState;
use pkl_runtime::host::{HostPoll, HostSession};
use pkl_runtime::parser::parse_module;
use pkl_runtime::resolver::resolve_module;
use pkl_runtime::value::{ErrorKind, RuntimeError, Value};
use pkl_runtime::vm::{Instruction as I, Program, Vm, VmPoll, compile_module};

const STACK_BYTES: usize = 256 * 1024;
const DEPTH: usize = 768;
const ORIGINAL: &str = "typealias A<T> = Int(this < 0 || this is A<List<T>>)\nunused = 42\nidentity = A == A\nshort: A<Int> = -1\nunsupported: A<Int> = 1";

fn isolated(name: &str, run: impl FnOnce() + Send + 'static) {
    if std::env::var("PKL_GENERIC_METADATA_CHILD").as_deref() == Ok(name) {
        std::thread::Builder::new()
            .name("generic-metadata-256k".into())
            .stack_size(STACK_BYTES)
            .spawn(run)
            .unwrap()
            .join()
            .unwrap();
        // The joined thread has dropped its evaluator and metadata. Suppress
        // the child's harness summary so workspace totals count each test once.
        println!("isolated metadata stress completed: {name}");
        std::process::exit(0);
    }
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", name, "--nocapture"])
        .env("PKL_GENERIC_METADATA_CHILD", name)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success(), "isolated metadata stress: {status}");
            return;
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            let status = child.wait().unwrap();
            panic!("metadata stress exceeded 60 seconds: {status}");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn program(source: &str) -> Arc<Program> {
    let mut program =
        compile_module(&resolve_module(&parse_module(source).unwrap()).unwrap()).unwrap();
    module_policy::register(Arc::make_mut(&mut program), source);
    program
}

fn event(vm: &mut Vm) -> VmPoll {
    for index in 0..2_000_000 {
        match vm.poll(1) {
            VmPoll::Yielded => {
                if index % 256 == 0 {
                    vm.collect(&[]);
                }
            }
            VmPoll::Module(request) => vm
                .complete_module(
                    request.id,
                    pkl_runtime::modules::default_completion(&request)
                        .expect("default module policy"),
                )
                .unwrap(),
            event => return event,
        }
    }
    panic!("bounded instruction allowance exhausted");
}

fn ready(vm: &mut Vm) -> Result<Value, RuntimeError> {
    match event(vm) {
        VmPoll::Ready(result) => result,
        event => panic!("expected completion, got {event:?}"),
    }
}

fn start_module(vm: &mut Vm, entry: usize) -> Value {
    vm.start(entry, vec![]).unwrap();
    ready(vm).unwrap()
}

fn member(vm: &mut Vm, module: &Value, name: &str) -> Result<Value, RuntimeError> {
    vm.start_member(module.as_object().unwrap(), name).unwrap();
    ready(vm)
}

fn read(vm: &mut Vm, uri: &str) -> u64 {
    match event(vm) {
        VmPoll::Pending(request) => {
            assert_eq!(request.uri, uri);
            vm.collect(&[]);
            request.id
        }
        event => panic!("expected {uri}, got {event:?}"),
    }
}

#[test]
fn original_infinite_growth_yields_cancels_retries_and_drops_on_small_stack() {
    // Compilation is outside the deliberately small evaluator thread.
    let program = program(ORIGINAL);
    isolated(
        "original_infinite_growth_yields_cancels_retries_and_drops_on_small_stack",
        move || {
            let mut vm = Vm::new(program.clone());
            let module = start_module(&mut vm, program.entry);
            let mut retained = None;
            for _ in 0..3 {
                vm.start_member(module.as_object().unwrap(), "unsupported")
                    .unwrap();
                // 150,000 guest instructions, versus the historical 300.
                for index in 0..1_500 {
                    assert!(matches!(vm.poll(100), VmPoll::Yielded));
                    if index % 100 == 0 {
                        vm.collect(&[]);
                    }
                }
                vm.cancel();
                assert_eq!(ready(&mut vm).unwrap_err().kind, ErrorKind::Cancelled);
                assert!(!vm.is_closed());
                assert_eq!(
                    vm.member_state(module.as_object().unwrap(), "unsupported")
                        .unwrap(),
                    LazyState::Unevaluated
                );
                assert_eq!(
                    member(&mut vm, &module, "unused"),
                    Ok(Value::Int(42.into()))
                );
                assert_eq!(member(&mut vm, &module, "identity"), Ok(Value::Bool(true)));
                assert_eq!(
                    member(&mut vm, &module, "short"),
                    Ok(Value::Int((-1).into()))
                );
                let stats = vm.collect(std::slice::from_ref(&module));
                let counts = (stats.live, stats.executable_live);
                if let Some(previous) = retained {
                    assert_eq!(counts, previous);
                }
                retained = Some(counts);
            }
            // Drop a running graph as well as an explicitly closed graph.
            vm.start_member(module.as_object().unwrap(), "unsupported")
                .unwrap();
            for _ in 0..1_500 {
                assert!(matches!(vm.poll(100), VmPoll::Yielded));
            }
            drop(vm);
            let mut fresh = Vm::new(program.clone());
            let module = start_module(&mut fresh, program.entry);
            assert_eq!(
                member(&mut fresh, &module, "short"),
                Ok(Value::Int((-1).into()))
            );
            fresh.close();
            assert_eq!(fresh.heap_stats().slots, 0);
        },
    );
}

#[test]
fn deep_arguments_survive_captured_closures_gc_failure_retry_and_evaluator_isolation() {
    let source = format!(
        "typealias A<T> = Int(if (this < 0) ((f) -> read(\"mem:leaf\") == \"ok\" && !f.apply(null)).apply((x) -> x is T) else (this - 1) is A<List<T>>)\nresult: A<Int> = {DEPTH}\nshort: A<String> = -1\n"
    );
    let program = program(&source);
    isolated(
        "deep_arguments_survive_captured_closures_gc_failure_retry_and_evaluator_isolation",
        move || {
            let mut left = Vm::new(program.clone());
            let mut right = Vm::new(program.clone());
            let left_module = start_module(&mut left, program.entry);
            let right_module = start_module(&mut right, program.entry);
            left.start_member(left_module.as_object().unwrap(), "result")
                .unwrap();
            let failed = read(&mut left, "mem:leaf");
            assert_eq!(
                right
                    .start_member(left_module.as_object().unwrap(), "result")
                    .unwrap_err()
                    .kind,
                ErrorKind::StaleHandle
            );
            right
                .start_member(right_module.as_object().unwrap(), "short")
                .unwrap();
            let right_read = read(&mut right, "mem:leaf");
            right.resume(right_read, Ok(Value::string("ok"))).unwrap();
            assert_eq!(ready(&mut right), Ok(Value::Int((-1).into())));
            right.close();
            assert_eq!(right.heap_stats().slots, 0);
            left.resume(failed, Err(RuntimeError::new(ErrorKind::Host, "deep\0🙂")))
                .unwrap();
            let error = ready(&mut left).unwrap_err();
            assert_eq!(error.kind, ErrorKind::Host);
            assert_eq!(error.message, "deep\0🙂");
            left.collect(std::slice::from_ref(&left_module));
            left.start_member(left_module.as_object().unwrap(), "result")
                .unwrap();
            let cancelled = read(&mut left, "mem:leaf");
            left.cancel();
            assert_eq!(ready(&mut left).unwrap_err().kind, ErrorKind::Cancelled);
            assert_eq!(
                left.resume(cancelled, Ok(Value::Null)).unwrap_err().kind,
                ErrorKind::Host
            );
            left.collect(std::slice::from_ref(&left_module));
            left.start_member(left_module.as_object().unwrap(), "result")
                .unwrap();
            let token = read(&mut left, "mem:leaf");
            assert_ne!(token, failed);
            left.resume(token, Ok(Value::string("ok"))).unwrap();
            assert_eq!(ready(&mut left), Ok(Value::Int((DEPTH as i64).into())));
            left.close();
            assert_eq!(left.heap_stats().slots, 0);
        },
    );
}

#[test]
fn deep_default_and_reference_consumers_are_stack_safe() {
    let source = format!(
        "import \"pkl:ref\"\nclass D extends ref.Domain {{}}\nclass C {{ marker = 7 }}\ntypealias Defaults<T> = Int(if (this < 0) (new Mapping<String,T> {{[\"k\"] {{}}}})[\"k\"] is T else (this - 1) is Defaults<Mapping<String,T>>)\ntypealias References<T> = Int(if (this < 0) !(this is ref.Reference<D,T>) else (this - 1) is References<List<T>>)\ndefaults: Defaults<C> = {DEPTH}\nreferences: References<Int> = {DEPTH}\n"
    );
    let program = program(&source);
    isolated(
        "deep_default_and_reference_consumers_are_stack_safe",
        move || {
            let mut vm = Vm::new(program.clone());
            let module = start_module(&mut vm, program.entry);
            assert_eq!(
                member(&mut vm, &module, "defaults"),
                Ok(Value::Int((DEPTH as i64).into()))
            );
            vm.collect(std::slice::from_ref(&module));
            assert_eq!(
                member(&mut vm, &module, "references"),
                Ok(Value::Int((DEPTH as i64).into()))
            );
            vm.collect(std::slice::from_ref(&module));
            vm.close();
            assert_eq!(vm.heap_stats().slots, 0);
        },
    );
}

#[test]
fn host_terminal_actions_release_deep_parent_and_child_graphs() {
    let source = format!(
        "typealias A<T> = Int(this < 0 || (read(\"mem:step\") == \"ok\" && (this - 1) is A<List<T>>))\nresult: A<Int> = {DEPTH}\ninner: A<String> = {DEPTH}\n"
    );
    let mut program = (*program(&source)).clone();
    let entry = program.entry;
    let result = program.blocks.len();
    program
        .blocks
        .push(vec![I::Input(0), I::GetMember("result".into()), I::Return]);
    let inner = program.blocks.len();
    program
        .blocks
        .push(vec![I::Input(0), I::GetMember("inner".into()), I::Return]);
    let program = Arc::new(program);
    isolated(
        "host_terminal_actions_release_deep_parent_and_child_graphs",
        move || {
            for kind in [ErrorKind::Cancelled, ErrorKind::Closed, ErrorKind::Timeout] {
                let mut session = HostSession::new(program.clone());
                session.start(entry, vec![], 0, None).unwrap();
                let module = match host_event(&mut session, 0) {
                    HostPoll::Ready(Ok(module)) => module,
                    event => panic!("expected module: {event:?}"),
                };
                session
                    .start(result, vec![module.clone()], 1, Some(10))
                    .unwrap();
                let parent = advance(&mut session, 1, DEPTH / 2);
                session
                    .reenter(&parent, inner, vec![module], 2, None)
                    .unwrap();
                let child = advance(&mut session, 2, DEPTH / 2);
                assert_eq!(session.context_depth(), 2);
                match kind {
                    ErrorKind::Cancelled => session.cancel(),
                    ErrorKind::Closed => session.close(),
                    ErrorKind::Timeout => {}
                    _ => unreachable!(),
                }
                let now = if kind == ErrorKind::Timeout { 11 } else { 3 };
                match host_event(&mut session, now) {
                    HostPoll::Ready(Err(error)) => assert_eq!(error.kind, kind),
                    event => panic!("expected terminal result: {event:?}"),
                }
                assert_eq!(session.heap_stats().slots, 0);
                assert_eq!(session.context_depth(), 0);
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
            }
        },
    );
}

fn host_event(session: &mut HostSession, now: u64) -> HostPoll {
    for _ in 0..100_000 {
        match session.poll(now, 1) {
            HostPoll::Yielded => {}
            event => {
                session.collect(&[]);
                return event;
            }
        }
    }
    panic!("bounded Host instruction allowance exhausted");
}

fn advance(session: &mut HostSession, now: u64, count: usize) -> String {
    for index in 0..=count {
        match host_event(session, now) {
            HostPoll::Read { token, uri } => {
                assert_eq!(uri, "mem:step");
                if index == count {
                    return token;
                }
                session
                    .resume(&token, Ok(Value::string("ok")), now)
                    .unwrap();
            }
            event => panic!("expected deep read: {event:?}"),
        }
    }
    unreachable!()
}
