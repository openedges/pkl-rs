//! Authored syntax traverses the complete frontend and releases its trees on the
//! same bounded native stack. These are safety/lifetime checks, not Java output
//! comparisons or guarantees for arbitrary syntax nesting.
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use pkl_runtime::diagnostic::DiagnosticKind;
use pkl_runtime::host::{HostPoll, HostSession};
use pkl_runtime::parser::{parse_expression, parse_module};
use pkl_runtime::resolver::resolve_module;
use pkl_runtime::syntax::ExprKind;
use pkl_runtime::value::{ErrorKind, RuntimeError, Value};
use pkl_runtime::vm::{
    Instruction as I, Program, compile_module, compile_module_with_power_assertions,
};

const DEPTH: usize = 768;

fn isolated(name: &str, work: fn()) {
    if std::env::var("PKL_FRONTEND_CHILD").as_deref() == Ok(name) {
        std::thread::Builder::new()
            .name("frontend-256k".into())
            .stack_size(256 * 1024)
            .spawn(work)
            .unwrap()
            .join()
            .unwrap();
        println!("authored frontend completed: {name}; stack=262144; depth={DEPTH}");
        return;
    }
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", name, "--nocapture"])
        .env("PKL_FRONTEND_CHILD", name)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(
                status.success(),
                "isolated authored frontend {name}: {status}"
            );
            return;
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            let status = child.wait().unwrap();
            panic!("authored frontend {name} exceeded 60 seconds: {status}");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn nested(prefix: &str, leaf: &str, suffix: &str, depth: usize) -> String {
    format!("{}{leaf}{}", prefix.repeat(depth), suffix.repeat(depth))
}

fn compile_and_release(source: &str, instrument: bool) -> Arc<Program> {
    println!("frontend phase=parse");
    let parsed = parse_module(source).unwrap();
    println!("frontend phase=Module-clone/drop");
    let parsed_copy = parsed.clone();
    drop(parsed);
    println!("frontend phase=resolve");
    let resolved = resolve_module(&parsed_copy).unwrap();
    drop(parsed_copy);
    println!("frontend phase=ResolvedModule-clone/drop");
    let resolved_copy = resolved.clone();
    drop(resolved);
    println!("frontend phase=compile");
    let program = if instrument {
        compile_module_with_power_assertions(&resolved_copy)
    } else {
        compile_module(&resolved_copy)
    }
    .unwrap();
    println!("frontend phase=ResolvedModule-drop");
    drop(resolved_copy);
    program
}

#[test]
fn authored_chains_parse_clone_resolve_compile_and_drop_on_small_stack() {
    isolated(
        "authored_chains_parse_clone_resolve_compile_and_drop_on_small_stack",
        || {
            let expressions = [
                ("group", nested("(", "42", ")", DEPTH)),
                ("unary", format!("{}true", "! ".repeat(DEPTH))),
                ("binary", format!("{}1", "1 + ".repeat(DEPTH))),
                ("member", format!("this{}", ".result".repeat(DEPTH))),
                ("nullable", format!("this{}", "?.result".repeat(DEPTH))),
                ("nonnull", format!("this{}", "!!".repeat(DEPTH))),
                ("subscript", format!("List(1){}", "[0]".repeat(DEPTH))),
                ("calls", nested("List(", "42", ")", DEPTH)),
                ("set-calls", nested("Set(", "42", ")", DEPTH)),
                ("map-calls", nested("Map(0, ", "42", ")", DEPTH)),
                ("custom-calls", nested("identity(", "42", ")", DEPTH)),
                ("read-calls", nested("read(", "\"mem:leaf\"", ")", DEPTH)),
                (
                    "shallow-interpolation",
                    format!("\"\\({})\"", nested("(", "42", ")", DEPTH)),
                ),
                (
                    "if-boundary",
                    format!("if (true) {} else 0", nested("(", "42", ")", DEPTH)),
                ),
                (
                    "let-boundary",
                    format!("let (x = 42) {}", nested("(", "x", ")", DEPTH)),
                ),
                (
                    "lambda-boundary",
                    format!("(x) -> {}", nested("(", "x", ")", DEPTH)),
                ),
                (
                    "object-boundary",
                    format!("new {{ value = {} }}", nested("(", "42", ")", DEPTH)),
                ),
            ];
            for (shape, expression) in expressions {
                println!("authored frontend shape={shape}");
                let source = format!("function identity(value) = value\nresult = {expression}\n");
                drop(compile_and_release(&source, false));
            }
            drop(compile_and_release(
                &format!("result: Int = {}\n", nested("(", "42", ")", DEPTH)),
                false,
            ));
            // A standalone exported expression also owns its tree independently.
            let expression = parse_expression(&nested("(", "42", ")", DEPTH)).unwrap();
            assert!(matches!(
                expression.unparenthesized().kind,
                ExprKind::Int(42)
            ));
            let copy = expression.clone();
            drop(expression);
            drop(copy);
            // Missing initializers remain explicit default-value syntax, not
            // an unreachable resolver branch after continuation scheduling.
            drop(compile_and_release("value: Int\n", false));
            // Independent shallow properties exceeded the former global 512 count.
            let wide = (0..1024)
                .map(|i| format!("p{i} = {i}\n"))
                .collect::<String>();
            drop(compile_and_release(&wide, false));
        },
    );
}

#[test]
fn authored_power_assertion_compiler_and_cleanup_use_small_stack() {
    isolated(
        "authored_power_assertion_compiler_and_cleanup_use_small_stack",
        || {
            let predicate = format!("{}true", "true && ".repeat(DEPTH));
            let source = format!("result: Int({predicate}) = 42\n");
            drop(compile_and_release(&source, true));
        },
    );
}

#[test]
fn authored_partial_parse_resolve_and_compile_errors_release_on_small_stack() {
    isolated(
        "authored_partial_parse_resolve_and_compile_errors_release_on_small_stack",
        || {
            let deep = nested("List(", "42", ")", DEPTH);
            for source in [
                format!("good = {deep}\nbroken = (\n"),
                format!("good = {deep}\nbroken = 1 +\n"),
            ] {
                let error = parse_module(&source).unwrap_err();
                assert_eq!(error.kind, DiagnosticKind::Syntax);
                drop(error);
            }
            // The first completed deep argument is released when a later argument fails.
            let error = parse_module(&format!("result = List({deep}, ) +\n")).unwrap_err();
            assert_eq!(error.kind, DiagnosticKind::Syntax);
            drop(error);
            let parsed = parse_module(&format!("result = {deep}\nresult = 0\n")).unwrap();
            let error = resolve_module(&parsed).unwrap_err();
            assert_eq!(error.kind, DiagnosticKind::DuplicateMember);
            drop(error);
            drop(parsed);
            // Constant-key resolution, comparison and duplicate rendering also
            // consume authored calls, rather than a runtime-generated value.
            let parsed = parse_module(&format!(
                "result = new Mapping {{ [{deep}] = 1; [{deep}] = 2 }}\n"
            ))
            .unwrap();
            let error = resolve_module(&parsed).unwrap_err();
            assert_eq!(error.kind, DiagnosticKind::DuplicateMember);
            drop(error);
            drop(parsed);
            // A later compiler error unwinds queued work after a completed deep
            // first argument has already emitted its instructions.
            let parsed = parse_module(&format!("result = List({deep}, Map(1))\n")).unwrap();
            let resolved = resolve_module(&parsed).unwrap();
            drop(parsed);
            let error = compile_module(&resolved).unwrap_err();
            assert_eq!(error.kind, ErrorKind::Type);
            drop(error);
            drop(resolved);
            // The original inheritance source now compiles. Keep its complete
            // deep tree and release the resulting program on the same stack.
            let parsed = parse_module(&format!("amends \"mem:base\"\nresult = {deep}\n")).unwrap();
            let resolved = resolve_module(&parsed).unwrap();
            drop(parsed);
            let program = compile_module(&resolved).unwrap();
            drop(program);
            drop(resolved);
            // A malformed import URI still fails during compilation, after
            // the completed deep first argument has emitted its instructions.
            let source =
                format!("amends \"mem:base\"\nresult = List({deep}, import(\"life:%GG\"))\n");
            let parsed = parse_module(&source).unwrap();
            let resolved = resolve_module(&parsed).unwrap();
            drop(parsed);
            let error = match compile_module(&resolved) {
                Err(error) => error,
                Ok(program) => {
                    drop(program);
                    panic!("malformed import URI unexpectedly compiled");
                }
            };
            assert_eq!(error.kind, ErrorKind::Host);
            assert!(error.span.unwrap().start > source.find(&deep).unwrap() + deep.len());
            drop(error);
            drop(resolved);
        },
    );
}

#[test]
fn authored_scope_type_and_shared_work_guards_release_on_small_stack() {
    isolated(
        "authored_scope_type_and_shared_work_guards_release_on_small_stack",
        || {
            // Rejection beyond a guard is insufficient: accepted trees close
            // to that guard must also survive all downstream phases and drops.
            for depth in [24, 31] {
                for (shape, prefix, suffix) in [
                    ("if", "if (true) (", ") else 0"),
                    ("let", "let (x = 0) (", ")"),
                    ("lambda", "(() -> (", "))"),
                    ("object", "new Dynamic { value = (", ") }"),
                ] {
                    println!("authored accepted control={shape}; depth={depth}");
                    let expression = nested(prefix, "42", suffix, depth);
                    drop(compile_and_release(
                        &format!("result = {expression}\n"),
                        false,
                    ));
                }
                println!("authored accepted type arguments; depth={depth}");
                let ty = nested("List<", "Int", ">", depth);
                let parsed = parse_module(&format!("result: {ty} = List()\n")).unwrap();
                let copy = parsed.clone();
                drop(parsed);
                drop(copy);
            }
            // A composed type below the structural bound remains accepted.
            let mixed = nested("List<", "Int? ? ", ">? ", 8);
            let parsed = parse_module(&format!("result: {mixed} = List()\n")).unwrap();
            let copy = parsed.clone();
            drop(parsed);
            drop(copy);
            for (prefix, suffix) in [
                ("if (true) (", ") else 0"),
                ("let (x = 0) (", ")"),
                ("(() -> (", "))"),
                ("new Dynamic { value = (", ") }"),
            ] {
                println!("authored guard control={prefix}");
                let expression = nested(
                    &format!("{}{prefix}", "(".repeat(8)),
                    "42",
                    &format!("{suffix}{}", ")".repeat(8)),
                    64,
                );
                let error = parse_module(&format!("result = {expression}\n")).unwrap_err();
                assert_eq!(error.kind, DiagnosticKind::Unsupported);
                drop(error);
            }
            println!("authored guard type arguments");
            let ty = nested("List<", "Int", ">", 64);
            let error = parse_module(&format!("result: {ty} = List()\n")).unwrap_err();
            assert_eq!(error.kind, DiagnosticKind::Unsupported);
            assert_eq!(error.message, "Type nesting limit exceeded.");
            drop(error);
            for suffix in ["? ", "(true)"] {
                println!("authored guard type suffix={suffix}");
                let error =
                    parse_module(&format!("result: Int{} = 42\n", suffix.repeat(64))).unwrap_err();
                assert_eq!(error.kind, DiagnosticKind::Unsupported);
                assert_eq!(error.message, "Type nesting limit exceeded.");
                drop(error);
            }
            // Recursive wrappers compose across type atoms and amendment edges.
            println!("authored guard mixed type");
            let mut mixed_type = "Int".to_owned();
            for _ in 0..16 {
                mixed_type = format!("List<{mixed_type}>{}", "? ".repeat(16));
            }
            let error = parse_module(&format!("result: {mixed_type} = List()\n")).unwrap_err();
            assert_eq!(error.kind, DiagnosticKind::Unsupported);
            drop(error);
            println!("authored accepted amendment parents; depth=64");
            let amendment = nested("(", "new Dynamic { x = 1 }", ") {}", 64);
            drop(compile_and_release(
                &format!("result = {amendment}\n"),
                false,
            ));
            for header in ["when (true) { ", "for (x in List(1)) { "] {
                println!("authored guard generator={header}");
                let generator = nested(header, "1", " }", 64);
                let error =
                    parse_module(&format!("result = new Listing {{ {generator} }}\n")).unwrap_err();
                assert_eq!(error.kind, DiagnosticKind::Unsupported);
                drop(error);
            }
            println!("authored guard work budget");
            let error = parse_module(&format!(
                "good = {}\nbudget = {}true\n",
                nested("List(", "42", ")", DEPTH),
                "! ".repeat(70_000)
            ))
            .unwrap_err();
            assert_eq!(error.kind, DiagnosticKind::Unsupported);
            drop(error);
            let half = format!("{}true", "! ".repeat(20_000));
            let error =
                parse_module(&format!("result = List({half}, \"\\({half})\")\n")).unwrap_err();
            assert_eq!(error.kind, DiagnosticKind::Unsupported);
            assert_eq!(error.message, "Expression work limit exceeded.");
            drop(error);
            drop(compile_and_release("result = 42\n", false));
        },
    );
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
    panic!("authored frontend Host event budget exceeded");
}

fn host_read(session: &mut HostSession, now: u64, uri: &str) -> String {
    match host_event(session, now) {
        HostPoll::Read { token, uri: actual } => {
            assert_eq!(&*actual, uri);
            session.collect(&[]);
            token
        }
        event => panic!("expected read, got {event:?}"),
    }
}

fn host_ready(session: &mut HostSession, now: u64) -> Result<Value, RuntimeError> {
    match host_event(session, now) {
        HostPoll::Ready(result) => result,
        event => panic!("expected result, got {event:?}"),
    }
}

#[test]
fn authored_frontend_program_survives_host_reentry_retry_cancel_and_terminal_release() {
    isolated(
        "authored_frontend_program_survives_host_reentry_retry_cancel_and_terminal_release",
        || {
            // Compilation and every AST clone/drop occur here, on the same 256 KiB stack.
            let deep = nested("(", "read(\"mem:leaf\")", ")", DEPTH);
            let source = format!("parent = read(\"mem:outer\")\nchild = {deep}\n");
            let mut program = compile_and_release(&source, false);
            let entries = ["parent", "child"].map(|name| {
                let block = program.blocks.len();
                Arc::make_mut(&mut program).blocks.push(vec![
                    I::Input(0),
                    I::GetMember(name.into()),
                    I::Return,
                ]);
                block
            });
            let entry = program.entry;
            let mut session = HostSession::new(program);
            session.start(entry, vec![], 0, None).unwrap();
            let module = host_ready(&mut session, 0).unwrap();
            session
                .start(entries[0], vec![module.clone()], 1, None)
                .unwrap();
            let outer = host_read(&mut session, 1, "mem:outer");
            session
                .reenter(&outer, entries[1], vec![module.clone()], 2, None)
                .unwrap();
            let failed = host_read(&mut session, 2, "mem:leaf");
            session
                .resume(
                    &failed,
                    Err(RuntimeError::new(ErrorKind::Host, "authored retry")),
                    2,
                )
                .unwrap();
            assert_eq!(
                host_ready(&mut session, 2).unwrap_err().kind,
                ErrorKind::Host
            );
            session
                .reenter(&outer, entries[1], vec![module.clone()], 3, None)
                .unwrap();
            let retry = host_read(&mut session, 3, "mem:leaf");
            session.resume(&retry, Ok(Value::Int(42)), 3).unwrap();
            assert_eq!(host_ready(&mut session, 3), Ok(Value::Int(42)));
            assert_eq!(host_read(&mut session, 3, "mem:outer"), outer);
            session.cancel();
            assert_eq!(
                host_ready(&mut session, 4).unwrap_err().kind,
                ErrorKind::Cancelled
            );
            assert_eq!(
                session.drain_cancelled_reads().as_slice(),
                std::slice::from_ref(&outer)
            );
            assert_eq!(
                session.resume(&outer, Ok(Value::Null), 4).unwrap_err().kind,
                ErrorKind::Closed
            );
            assert_eq!(session.heap_stats().slots, 0);
            assert_eq!(session.context_depth(), 0);
            drop(module);
            drop(session);
        },
    );
}
