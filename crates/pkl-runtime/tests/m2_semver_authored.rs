//! Authored stdlib loading and runtime invariants. Not Java differential evidence.
use pkl_runtime::{
    evaluation::{
        EvaluationOptions,
        session::{SourceSession, SourceSessionOptions},
    },
    host::HostPoll,
    modules::{ModuleRequestKind, default_completion},
    value::{RuntimeError, Value},
};

fn session(source: &str, paths: &[&str], power: bool) -> SourceSession {
    SourceSession::compile(
        source,
        "file:///stage138-semver/main.pkl",
        "main",
        &paths
            .iter()
            .map(|path| (*path).to_owned())
            .collect::<Vec<_>>(),
        SourceSessionOptions {
            evaluation: EvaluationOptions {
                power_assertions: power,
            },
            ..Default::default()
        },
    )
    .unwrap()
}
fn run(session: &mut SourceSession, index: usize) -> Result<Value, RuntimeError> {
    // SourceSession paths return the rendered expression value.
    session.start_path(index, 0, None).unwrap();
    for _ in 0..1_000_000 {
        let event = session.poll(0, 1);
        session.collect(&[]);
        match event {
            HostPoll::Yielded => {}
            HostPoll::Module { token, request } => {
                // Authored bundled semver must not be replaced by Host source.
                assert!(!matches!(
                    request.kind,
                    ModuleRequestKind::LoadSource { .. }
                ));
                let completion = default_completion(&request).expect("bundled module effect");
                session.complete_module(&token, completion, 0).unwrap();
            }
            HostPoll::Ready(result) => return result,
            other => panic!("unexpected effect: {other:?}"),
        }
    }
    panic!("bounded semver evaluation did not terminate");
}

#[test]
fn version_class_and_function_share_the_name_without_sharing_the_namespace() {
    let source = r#"import "pkl:semver"
parsed: semver.Version = semver.Version("1.2.3-alpha.1+build")
constructed: semver.Version = new semver.Version { major=1; minor=2; patch=3; preRelease="alpha.1"; build="build" }
result = parsed == constructed && parsed.getClass() == semver.Version && parsed.toString() == "1.2.3-alpha.1+build"
defaults = new semver.Version {}.toString()
"#;
    for power in [false, true] {
        let mut s = session(source, &["result", "defaults"], power);
        assert_eq!(run(&mut s, 0).unwrap(), Value::String("true".into()));
        for _ in 0..2 {
            let error = run(&mut s, 1).unwrap_err();
            assert_eq!(
                error.message,
                "Tried to read property `major` but its value is undefined."
            );
            assert!(error.frames.iter().any(|frame| {
                frame
                    .source
                    .as_ref()
                    .is_some_and(|source| source.uri == "pkl:semver")
            }));
            assert_eq!(run(&mut s, 0).unwrap(), Value::String("true".into()));
        }
        s.close();
        assert_eq!(s.collect(&[]).slots, 0);
    }
}

#[test]
fn canonical_singleton_class_and_comparator_survive_repeated_imports_and_collection() {
    let source = r#"a=import("pkl:semver")
b=import("pkl:semver#alias")
function fresh()=import("pkl:semver")
identity = a == b && a == fresh() && a.Version == b.Version && a.comparator == b.comparator
result = a.Version("1.2.3").isLessThan(b.Version("2.0.0"))
"#;
    let mut s = session(source, &["identity", "result"], false);
    for index in [0, 1, 0, 1] {
        assert_eq!(run(&mut s, index).unwrap(), Value::String("true".into()));
    }
    s.close();
    assert_eq!(s.collect(&[]).slots, 0);
}

#[test]
fn authored_parse_failure_keeps_stdlib_source_and_does_not_poison_later_queries() {
    let source = r#"import "pkl:semver"
invalid = semver.Version("1.02.3")
valid = semver.Version("1.2.3").toString()
"#;
    for power in [false, true] {
        let mut s = session(source, &["invalid", "valid"], power);
        for index in [0, 1, 0, 1] {
            if index == 0 {
                let error = run(&mut s, index).unwrap_err();
                assert_eq!(
                    error.message,
                    "`1.02.3` is not a valid semantic version number."
                );
                assert!(error.frames.iter().any(|frame| {
                    frame.member == "#Version"
                        && frame.source.as_ref().is_some_and(|source| {
                            source.uri == "pkl:semver" && source.module_name == "pkl.semver"
                        })
                }));
            } else {
                assert_eq!(run(&mut s, index).unwrap(), Value::String("1.2.3".into()));
            }
        }
        s.close();
        assert_eq!(s.collect(&[]).slots, 0);
    }
}

#[test]
fn comparator_precedence_and_semantic_equality_execute_authored_method_bodies() {
    let source = r#"import "pkl:semver"
ordered = List("1.0.0", "1.0.0-beta.11", "1.0.0-alpha.1", "1.0.0-beta.2", "1.0.0-alpha").map((it) -> semver.Version(it)).sortWith(semver.comparator).map((it) -> it.toString()).join(",")
equality = semver.Version("1.0.0+abc").equals(semver.Version("1.0.0+def")) && semver.Version("1.0.0+abc") != semver.Version("1.0.0+def")
"#;
    let mut s = session(source, &["ordered", "equality"], false);
    assert_eq!(
        run(&mut s, 0).unwrap(),
        Value::String("1.0.0-alpha,1.0.0-alpha.1,1.0.0-beta.2,1.0.0-beta.11,1.0.0".into())
    );
    assert_eq!(run(&mut s, 1).unwrap(), Value::String("true".into()));
    s.close();
}
