//! Authored shell module/VM invariants; unexecuted and not Java differential evidence.
use pkl_runtime::{
    evaluation::{
        EvaluationOptions,
        session::{SourceSession, SourceSessionOptions},
    },
    host::HostPoll,
    modules::{ModuleRequestKind, default_completion},
    value::{ErrorKind, RuntimeError, Value},
};

fn session(source: &str, paths: &[&str], power: bool) -> SourceSession {
    SourceSession::compile(
        source,
        "file:///stage143-shell/main.pkl",
        "main",
        &paths.iter().map(|p| (*p).to_owned()).collect::<Vec<_>>(),
        SourceSessionOptions {
            evaluation: EvaluationOptions {
                power_assertions: power,
            },
            ..Default::default()
        },
    )
    .unwrap()
}
fn run(
    session: &mut SourceSession,
    index: usize,
    collect_at_boundaries: bool,
) -> Result<Value, RuntimeError> {
    if collect_at_boundaries {
        session.collect(&[]);
    }
    session.start_path(index, 0, None).unwrap();
    for _ in 0..1_000_000 {
        let event = session.poll(0, 128);
        match event {
            HostPoll::Yielded => {}
            HostPoll::Module { token, request } => {
                if collect_at_boundaries {
                    session.collect(&[]);
                }
                assert!(!matches!(
                    request.kind,
                    ModuleRequestKind::LoadSource { .. }
                ));
                let completion = default_completion(&request).expect("bundled module effect");
                session.complete_module(&token, completion, 0).unwrap();
                if collect_at_boundaries {
                    session.collect(&[]);
                }
            }
            HostPoll::Ready(result) => return result,
            other => panic!("unexpected effect: {other:?}"),
        }
    }
    panic!("shell evaluation did not terminate within the control budget");
}
fn check_units(value: Value, expected: &[u16]) {
    let Value::String(value) = value else {
        panic!("rendered expression string");
    };
    assert_eq!(value.encode_utf16().collect::<Vec<_>>(), expected);
}

#[test]
fn exact_authored_grouping_retains_empty_quotes_controls_and_utf16() {
    let source = r#"import "pkl:shell"
empty = shell.escapeWithSingleQuotes("")
quotes = shell.escapeWithSingleQuotes("a''b'")
controls = shell.escapeWithSingleQuotes("\u{0}\n\r\t")
surrogates = shell.escapeWithSingleQuotes("\u{d800}'\u{dc00}")
supplementary = shell.escapeWithSingleQuotes("\u{1f600}")
"#;
    for power in [false, true] {
        let mut s = session(
            source,
            &["empty", "quotes", "controls", "surrogates", "supplementary"],
            power,
        );
        check_units(run(&mut s, 0, false).unwrap(), &[]);
        check_units(
            run(&mut s, 1, false).unwrap(),
            &"'a'\\'\\''b'\\'".encode_utf16().collect::<Vec<_>>(),
        );
        check_units(run(&mut s, 2, false).unwrap(), &[39, 0, 10, 13, 9, 39]);
        check_units(
            run(&mut s, 3, false).unwrap(),
            &[39, 0xd800, 39, 92, 39, 39, 0xdc00, 39],
        );
        check_units(run(&mut s, 4, false).unwrap(), &[39, 0xd83d, 0xde00, 39]);
        s.close();
        assert_eq!(s.collect(&[]).slots, 0);
    }
}

#[test]
fn cached_module_and_type_failure_preserve_later_queries_with_collection() {
    let source = r#"a = import("pkl:shell")
b = import("pkl:shell")
identity = a == b
bad = a.escapeWithSingleQuotes(1)
good = b.escapeWithSingleQuotes("ok")
"#;
    let mut s = session(source, &["identity", "bad", "good"], false);
    for index in [0, 1, 2, 1, 2] {
        if index == 1 {
            assert_eq!(run(&mut s, index, true).unwrap_err().kind, ErrorKind::Type);
        } else {
            check_units(
                run(&mut s, index, true).unwrap(),
                &if index == 0 { "true" } else { "'ok'" }
                    .encode_utf16()
                    .collect::<Vec<_>>(),
            );
        }
    }
    s.close();
    assert_eq!(s.collect(&[]).slots, 0);
}
