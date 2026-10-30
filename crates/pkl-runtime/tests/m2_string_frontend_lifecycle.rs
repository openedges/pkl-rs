//! Authored nested lexical interpolation stays on the Stage 25 native stack and
//! deadline. Java byte comparisons and platform execution are separate checks.
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use pkl_runtime::diagnostic::DiagnosticKind;
use pkl_runtime::host::{HostPoll, HostSession};
use pkl_runtime::parser::{parse_expression, parse_module};
use pkl_runtime::resolver::resolve_module;
use pkl_runtime::value::{ErrorKind, RuntimeError, Value};
use pkl_runtime::vm::{Instruction as I, Program, compile_module};

fn isolated(name: &str, work: fn()) {
    if std::env::var("PKL_STRING_FRONTEND_CHILD").as_deref() == Ok(name) {
        std::thread::Builder::new()
            .name("string-frontend-256k".into())
            .stack_size(256 * 1024)
            .spawn(work)
            .unwrap()
            .join()
            .unwrap();
        println!("authored string frontend completed: {name}; stack=262144");
        return;
    }
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", name, "--nocapture"])
        .env("PKL_STRING_FRONTEND_CHILD", name)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success(), "isolated nested lexical {name}: {status}");
            return;
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            let status = child.wait().unwrap();
            panic!("nested lexical {name} exceeded 60 seconds: {status}");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn nested(prefix: &str, leaf: &str, suffix: &str, depth: usize) -> String {
    format!("{}{leaf}{}", prefix.repeat(depth), suffix.repeat(depth))
}

fn lexical(leaf: &str, depth: usize) -> String {
    nested("\"\\(", leaf, ")\"", depth)
}

fn compile_and_release(source: &str) -> Arc<Program> {
    println!("nested lexical phase=parse");
    let parsed = parse_module(source).unwrap();
    println!("nested lexical phase=Module-clone/drop");
    let parsed_copy = parsed.clone();
    drop(parsed);
    println!("nested lexical phase=resolve");
    let resolved = resolve_module(&parsed_copy).unwrap();
    drop(parsed_copy);
    println!("nested lexical phase=ResolvedModule-clone/drop");
    let resolved_copy = resolved.clone();
    drop(resolved);
    println!("nested lexical phase=compile");
    let program = compile_module(&resolved_copy).unwrap();
    println!("nested lexical phase=ResolvedModule-drop");
    drop(resolved_copy);
    program
}

#[test]
fn nested_lexical_accepted_near_guard_traverses_frontend_on_small_stack() {
    isolated(
        "nested_lexical_accepted_near_guard_traverses_frontend_on_small_stack",
        || {
            for depth in [24, 31] {
                for (shape, prefix, suffix) in [
                    ("ordinary", "\"\\(", ")\""),
                    ("hash", "#\"\\#(", ")\"#"),
                    ("multiline", "\"\"\"\n\\(", ")\n\"\"\""),
                ] {
                    println!("nested lexical accepted={shape}; depth={depth}");
                    let expression = nested(prefix, "42", suffix, depth);
                    drop(compile_and_release(&format!("result = {expression}\n")));
                    let expression = parse_expression(&expression).unwrap();
                    let copy = expression.clone();
                    drop(expression);
                    drop(copy);
                }
            }
            // Separate strings must not consume a cumulative nesting allowance.
            let wide = (0..64)
                .map(|i| format!("p{i} = {}\n", lexical("42", 31)))
                .collect::<String>();
            drop(compile_and_release(&wide));
        },
    );
}

#[test]
fn original_nested_lexical_64_rejects_without_aborting_small_stack() {
    isolated(
        "original_nested_lexical_64_rejects_without_aborting_small_stack",
        || {
            // Preserve the original Stage 25 SIGABRT shape and depth. A bounded
            // rejection is safety evidence, not Java diagnostic compatibility.
            let original = lexical("42", 64);
            for source in [original.clone(), format!("result = {original}\n")] {
                let error = if source.starts_with("result") {
                    parse_module(&source).unwrap_err()
                } else {
                    parse_expression(&source).unwrap_err()
                };
                assert_eq!(error.kind, DiagnosticKind::Unsupported);
                assert_eq!(
                    error.message,
                    "String interpolation nesting limit exceeded."
                );
                drop(error);
            }
            // Alternating delimiters must use the same bounded lexical depth.
            let mut mixed = "42".to_string();
            for i in 0..64 {
                mixed = match i % 3 {
                    0 => format!("\"\\({mixed})\""),
                    1 => format!("#\"\\#({mixed})\"#"),
                    _ => format!("\"\"\"\n\\({mixed})\n\"\"\""),
                };
            }
            let error = parse_module(&format!("result = {mixed}\n")).unwrap_err();
            assert_eq!(error.kind, DiagnosticKind::Unsupported);
            assert_eq!(
                error.message,
                "String interpolation nesting limit exceeded."
            );
            drop(error);
            drop(compile_and_release(&format!(
                "result = {}\n",
                lexical("42", 31)
            )));
        },
    );
}

#[test]
fn nested_lexical_partial_errors_and_shared_work_guard_release_small_stack() {
    isolated(
        "nested_lexical_partial_errors_and_shared_work_guard_release_small_stack",
        || {
            let deep = lexical("42", 31);
            for source in [
                format!("good = {deep}\nbroken = (\n"),
                format!("result = List({deep}, \"\\(1 + )\")\n"),
                format!("result = \"\\({})\\(1 + )\"\n", lexical("42", 24)),
            ] {
                let error = parse_module(&source).unwrap_err();
                assert_eq!(error.kind, DiagnosticKind::Syntax);
                drop(error);
            }
            // Missing delimiters at accepted lexical depth must not reenter a
            // recursive lexer -> EOF parser -> lexer error-validation chain.
            for source in [
                format!("result = {}42", "\"\\(".repeat(31)),
                format!("result = {}", &deep[..deep.len() - 2]),
                format!("result = {}", &deep[..deep.len() - 1]),
            ] {
                let error = parse_module(&source).unwrap_err();
                assert_eq!(error.kind, DiagnosticKind::Syntax);
                drop(error);
            }
            // Preserve all six malformed multiline inputs that previously
            // aborted this stack in lexer error-validation replay. Exercise
            // each unchanged expression and the module entry point as well.
            for depth in [24, 31] {
                for (shape, prefix, suffix) in [
                    ("opening", "\"\"\"x\\(", ")\n\"\"\""),
                    ("closing", "\"\"\"\n\\(", ")x\"\"\""),
                    ("indent", "\"\"\"\nx\\(", ")\n  \"\"\""),
                ] {
                    println!("nested lexical malformed-multiline={shape}; depth={depth}");
                    let expression = nested(prefix, "42", suffix, depth);
                    let error = parse_expression(&expression).unwrap_err();
                    assert_eq!(error.kind, DiagnosticKind::Syntax);
                    drop(error);
                    let error = parse_module(&format!("result = {expression}\n")).unwrap_err();
                    assert_eq!(error.kind, DiagnosticKind::Syntax);
                    drop(error);
                }
            }
            // The first malformed expression wins over the later lexical
            // escape error. Outer interpolation plus30 inner strings uses31
            // lexical levels, independently of the work and scope guards.
            let source = format!("result = \"\\({})\\(\"bad\\q\")\"\n", lexical("1 +", 30));
            let error = parse_module(&source).unwrap_err();
            assert_eq!(error.kind, DiagnosticKind::Syntax);
            assert_eq!(error.span.start, source.find("1 +").unwrap() + 3);
            drop(error);
            let parsed = parse_module(&format!("result = {deep}\nresult = 0\n")).unwrap();
            let error = resolve_module(&parsed).unwrap_err();
            assert_eq!(error.kind, DiagnosticKind::DuplicateMember);
            drop(error);
            drop(parsed);
            let parsed = parse_module(&format!("result = List({deep}, Map(1))\n")).unwrap();
            let resolved = resolve_module(&parsed).unwrap();
            drop(parsed);
            let error = compile_module(&resolved).unwrap_err();
            assert_eq!(error.kind, ErrorKind::Type);
            drop(error);
            drop(resolved);
            // Each interpolation inherits the same expression-work allowance;
            // the second half must not get a fresh allowance after the first.
            let half = format!("{}true", "! ".repeat(20_000));
            let error = parse_module(&format!(
                "good = {deep}\nresult = List(\"\\({half})\", \"\\({half})\")\n"
            ))
            .unwrap_err();
            assert_eq!(error.kind, DiagnosticKind::Unsupported);
            assert_eq!(error.message, "Expression work limit exceeded.");
            drop(error);
            drop(compile_and_release(&format!("result = {deep}\n")));
        },
    );
}

fn event(session: &mut HostSession, now: u64) -> HostPoll {
    for _ in 0..200_000 {
        match session.poll(now, 1) {
            HostPoll::Yielded => {
                session.collect(&[]);
            }
            event => return event,
        }
    }
    panic!("nested lexical Host event budget exceeded");
}

fn read(session: &mut HostSession, now: u64, uri: &str) -> String {
    match event(session, now) {
        HostPoll::Read { token, uri: actual } => {
            assert_eq!(actual, uri);
            session.collect(&[]);
            token
        }
        other => panic!("expected read, got {other:?}"),
    }
}

fn ready(session: &mut HostSession, now: u64) -> Result<Value, RuntimeError> {
    match event(session, now) {
        HostPoll::Ready(result) => result,
        other => panic!("expected result, got {other:?}"),
    }
}

#[test]
fn nested_lexical_host_reentry_read_failure_retry_cancel_and_terminal_cleanup() {
    isolated(
        "nested_lexical_host_reentry_read_failure_retry_cancel_and_terminal_cleanup",
        || {
            let source = format!(
                "parent = read(\"mem:outer\")\nchild = {}\npending = {}\n",
                lexical("read(\"mem:leaf\")", 31),
                lexical("read(\"mem:cancel\")", 31),
            );
            let mut program = compile_and_release(&source);
            let entries = ["parent", "child", "pending"].map(|name| {
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
            let module = ready(&mut session, 0).unwrap();
            session
                .start(entries[0], vec![module.clone()], 1, None)
                .unwrap();
            let outer = read(&mut session, 1, "mem:outer");
            session
                .reenter(&outer, entries[1], vec![module.clone()], 2, None)
                .unwrap();
            let failed = read(&mut session, 2, "mem:leaf");
            session
                .resume(
                    &failed,
                    Err(RuntimeError::new(ErrorKind::Host, "nested lexical retry")),
                    2,
                )
                .unwrap();
            assert_eq!(ready(&mut session, 2).unwrap_err().kind, ErrorKind::Host);
            session
                .reenter(&outer, entries[1], vec![module.clone()], 3, None)
                .unwrap();
            let retry = read(&mut session, 3, "mem:leaf");
            assert_ne!(failed, retry);
            session
                .resume(&retry, Ok(Value::Int(42.into())), 3)
                .unwrap();
            assert_eq!(ready(&mut session, 3), Ok(Value::String("42".into())));
            assert_eq!(read(&mut session, 3, "mem:outer"), outer);
            session
                .reenter(&outer, entries[2], vec![module.clone()], 4, None)
                .unwrap();
            let cancelled = read(&mut session, 4, "mem:cancel");
            session.cancel();
            assert_eq!(
                ready(&mut session, 5).unwrap_err().kind,
                ErrorKind::Cancelled
            );
            assert_eq!(
                session.drain_cancelled_reads(),
                vec![outer.clone(), cancelled.clone()]
            );
            for token in [outer, cancelled] {
                assert_eq!(
                    session.resume(&token, Ok(Value::Null), 5).unwrap_err().kind,
                    ErrorKind::Closed
                );
            }
            assert_eq!(session.heap_stats().slots, 0);
            assert_eq!(session.context_depth(), 0);
            drop(module);
            drop(session);
        },
    );
}
