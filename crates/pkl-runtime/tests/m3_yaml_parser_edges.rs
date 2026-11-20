//! M3 YAML parser regression tests covering closed scalar/diagnostic/backend
//! `ErrorKind::Unsupported` branches (verified against live pinned Java 0.32.0)
//! and cooperative mid-document / mid-merge yielding and cancellation.

use pkl_runtime::{
    evaluation::session::{SourceSession, SourceSessionOptions},
    host::HostPoll,
    modules::{ModuleRequestKind, default_completion},
    resources::{ReadResult, Resource},
    value::{ErrorKind, PklString, RuntimeError, Value},
};

fn compile(source: &str, paths: &[&str]) -> SourceSession {
    SourceSession::compile(
        source,
        "file:///m3-yaml-edges/main.pkl",
        "main",
        &paths.iter().map(|p| (*p).to_owned()).collect::<Vec<_>>(),
        SourceSessionOptions::default(),
    )
    .unwrap()
}

fn eval_path(session: &mut SourceSession, index: usize) -> Result<Value, RuntimeError> {
    session.start_path(index, 0, None).unwrap();
    for _ in 0..200_000 {
        match session.poll(0, 10_000) {
            HostPoll::Yielded => (),
            HostPoll::Module { token, request } => {
                assert!(!matches!(
                    request.kind,
                    ModuleRequestKind::LoadSource { .. }
                ));
                let completion = default_completion(&request).expect("bundled stdlib module");
                session.complete_module(&token, completion, 0).unwrap();
            }
            HostPoll::Read { token, uri } => {
                let res = match uri.as_str() {
                    Some("bad:uri") => ReadResult::Resource(Resource {
                        uri: "a b".into(),
                        bytes: b"x: 1\n".as_slice().into(),
                    }),
                    other => panic!("unexpected read: {other:?}"),
                };
                session.complete_read(&token, Ok(res), 0).unwrap();
            }
            HostPoll::Ready(result) => return result,
            other => panic!("unexpected event: {other:?}"),
        }
    }
    panic!("evaluation did not settle")
}

fn close(mut session: SourceSession) {
    session.close();
    assert_eq!(session.collect(&[]).slots, 0);
}

fn assert_parse_hint_contains(source: &str, expected_hint: &str) {
    let mut s = compile(source, &["res"]);
    let err = eval_path(&mut s, 0).unwrap_err();
    assert_eq!(
        err.kind,
        ErrorKind::Type,
        "expected Type error, got {err:?}"
    );
    assert_eq!(err.message, "Error parsing YAML document.");
    let hint = err.hint.as_ref().expect("expected hint").to_java_utf8();
    assert!(
        hint.contains(expected_hint),
        "expected hint to contain {expected_hint:?}, got {hint:?}"
    );
    close(s);
}

#[test]
fn scalars_empty_sign_only_and_base60_errors_match_java_0_32_0() {
    let p = "import \"pkl:yaml\"\n";
    let error_cases = [
        (
            r#"res = new yaml.Parser {}.parse("!!int ''")"#,
            "java.lang.StringIndexOutOfBoundsException: Index 0 out of bounds for length 0",
        ),
        (
            r#"res = new yaml.Parser {}.parse("!!int '_'")"#,
            "java.lang.StringIndexOutOfBoundsException: Index 0 out of bounds for length 0",
        ),
        (
            r#"res = new yaml.Parser {}.parse("!!int '+'")"#,
            "java.lang.StringIndexOutOfBoundsException: Index 1 out of bounds for length 1",
        ),
        (
            r#"res = new yaml.Parser {}.parse("!!int '-'")"#,
            "java.lang.StringIndexOutOfBoundsException: Index 1 out of bounds for length 1",
        ),
        (
            r#"res = new yaml.Parser {}.parse("!!int '-_'")"#,
            "java.lang.StringIndexOutOfBoundsException: Index 1 out of bounds for length 1",
        ),
        (
            r#"res = new yaml.Parser {}.parse("!!int '1:abc'")"#,
            r#"java.lang.NumberFormatException: For input string: "abc""#,
        ),
        (
            r#"res = new yaml.Parser {}.parse("!!int ':1'")"#,
            r#"java.lang.NumberFormatException: For input string: """#,
        ),
        (
            r#"res = new yaml.Parser {}.parse("!!int '1::2'")"#,
            r#"java.lang.NumberFormatException: For input string: """#,
        ),
        (
            r#"res = new yaml.Parser {}.parse("!!int '1:99999999999999999999'")"#,
            r#"java.lang.NumberFormatException: For input string: "99999999999999999999""#,
        ),
        (
            r#"res = new yaml.Parser {}.parse("!!int '1: '")"#,
            r#"java.lang.NumberFormatException: For input string: " ""#,
        ),
        (
            r#"res = new yaml.Parser {}.parse("!!float '1:abc'")"#,
            r#"java.lang.NumberFormatException: For input string: "abc""#,
        ),
        (
            r#"res = new yaml.Parser {}.parse("!!float ':1'")"#,
            "java.lang.NumberFormatException: empty String",
        ),
        (
            r#"res = new yaml.Parser {}.parse("!!float '1::2'")"#,
            "java.lang.NumberFormatException: empty String",
        ),
        (
            r#"res = new yaml.Parser {}.parse("!!float '1: '")"#,
            "java.lang.NumberFormatException: empty String",
        ),
    ];
    for (expr, expected) in error_cases {
        assert_parse_hint_contains(&format!("{p}{expr}\n"), expected);
    }

    // Valid base60 edge cases
    let ok_src = r#"import "pkl:yaml"
local y = new yaml.Parser {}
a = y.parse("!!int '1:+2'")
b = y.parse("!!int '1:-2'")
c = y.parse("!!int '1:'")
d = y.parse("!!int ':'")
e = y.parse("!!int '1:\u{0662}'")
f = y.parse("!!float ':'")
"#;
    let mut s = compile(ok_src, &["a", "b", "c", "d", "e", "f"]);
    assert_eq!(eval_path(&mut s, 0).unwrap(), Value::String("62".into()));
    assert_eq!(eval_path(&mut s, 1).unwrap(), Value::String("58".into()));
    assert_eq!(eval_path(&mut s, 2).unwrap(), Value::String("1".into()));
    assert_eq!(eval_path(&mut s, 3).unwrap(), Value::String("0".into()));
    assert_eq!(eval_path(&mut s, 4).unwrap(), Value::String("62".into()));
    assert_eq!(eval_path(&mut s, 5).unwrap(), Value::String("0.0".into()));
    close(s);
}

#[test]
fn duplicate_non_scalar_keys_format_java_to_string_matching_java_0_32_0() {
    let p = "import \"pkl:yaml\"\n";
    let cases = [
        (
            r#"res = new yaml.Parser {}.parse("1.5: a\n1.5: b")"#,
            "found duplicate key 1.5",
        ),
        (
            r#"res = new yaml.Parser {}.parse("0.0: a\n0.0: b")"#,
            "found duplicate key 0.0",
        ),
        (
            r#"res = new yaml.Parser {}.parse("-0.0: a\n-0.0: b")"#,
            "found duplicate key -0.0",
        ),
        (
            r#"res = new yaml.Parser {}.parse(".nan: a\n.nan: b")"#,
            "found duplicate key NaN",
        ),
        (
            r#"res = new yaml.Parser {}.parse(".inf: a\n.inf: b")"#,
            "found duplicate key Infinity",
        ),
        (
            r#"res = new yaml.Parser {}.parse("!!binary AA==: a\n!!binary AA==: b")"#,
            "found duplicate key Bytes(0)",
        ),
        (
            r#"res = new yaml.Parser {}.parse("[1, 2]: a\n[1, 2]: b")"#,
            "found duplicate key new Listing { 1; 2 }",
        ),
        (
            r#"res = new yaml.Parser {}.parse("{x: 1}: a\n{x: 1}: b")"#,
            "found duplicate key new Dynamic { x = 1 }",
        ),
        (
            r#"res = new yaml.Parser { useMapping = true }.parse("{x: 1}: a\n{x: 1}: b")"#,
            r#"found duplicate key new Mapping { ["x"] = 1 }"#,
        ),
    ];
    for (expr, expected) in cases {
        assert_parse_hint_contains(&format!("{p}{expr}\n"), expected);
    }
}

#[test]
fn backend_unicode_digits_uri_escapes_and_utf8_malformed_lengths_match_java_0_32_0() {
    let ok_src = r#"import "pkl:yaml"
local y = new yaml.Parser {}
v1 = y.parse("%YAML 1.\u{0662}\n---\na: 1").a
v2 = y.parse("%YAML 1.\u{ff12}\n---\na: 2").a
ind1 = y.parse("|\u{0662}\n  hi\n")
ind2 = y.parse("|\u{ff12}\n  ok\n")
"#;
    let mut s = compile(ok_src, &["v1", "v2", "ind1", "ind2"]);
    assert_eq!(eval_path(&mut s, 0).unwrap(), Value::String("1".into()));
    assert_eq!(eval_path(&mut s, 1).unwrap(), Value::String("2".into()));
    assert_eq!(eval_path(&mut s, 2).unwrap(), Value::String("hi\n".into()));
    assert_eq!(eval_path(&mut s, 3).unwrap(), Value::String("ok\n".into()));
    close(s);

    let p = "import \"pkl:yaml\"\n";
    let err_cases = [
        (
            r#"res = new yaml.Parser {}.parse("|\u{0660}\n  hi\n")"#,
            "expected indentation indicator in the range 1-9, but found 0",
        ),
        (
            r#"res = new yaml.Parser {}.parse("!a%\u{0664}\u{0661} x")"#,
            "could not determine a constructor for the tag !aA",
        ),
        (
            r#"res = new yaml.Parser {}.parse("!a%80 x")"#,
            "expected URI in UTF-8: Input length = 1",
        ),
        (
            r#"res = new yaml.Parser {}.parse("!a%E0%A0 x")"#,
            "expected URI in UTF-8: Input length = 2",
        ),
        (
            r#"res = new yaml.Parser {}.parse("!a%ED%A0%80 x")"#,
            "expected URI in UTF-8: Input length = 3",
        ),
        (
            r#"res = new yaml.Parser {}.parse("!a%F0%90%80 x")"#,
            "expected URI in UTF-8: Input length = 3",
        ),
        (
            r#"res = new yaml.Parser {}.parse("!a%F4%90%80%80 x")"#,
            "expected URI in UTF-8: Input length = 1",
        ),
    ];
    for (expr, expected) in err_cases {
        assert_parse_hint_contains(&format!("{p}{expr}\n"), expected);
    }
}

#[test]
fn invalid_resource_uri_retains_uri_create_cause_provenance() {
    let source = r#"import "pkl:yaml"
res = new yaml.Parser {}.parse(read("bad:uri"))
"#;
    let mut s = compile(source, &["res"]);
    let err = eval_path(&mut s, 0).unwrap_err();
    assert_eq!(err.kind, ErrorKind::Unsupported);
    let cause = err.uri_create_cause.as_ref().expect("UriCreateCause");
    assert_eq!(cause.input, PklString::from("a b"));
    close(s);
}

#[test]
fn backend_yields_cooperatively_and_cancels_cleanly_mid_document_and_mid_merge() {
    // 1. Large document with comments + 250 items + explicit merges
    let mut yaml = String::new();
    for i in 0..80 {
        yaml.push_str(&format!("# comment line {i}\n"));
    }
    yaml.push_str("base: &b\n  k0: 0\n  k1: 1\n");
    yaml.push_str("merged:\n  !!merge <<: *b\n  extra: 2\n");
    yaml.push_str("items:\n");
    for i in 0..250 {
        yaml.push_str(&format!("  - {i}\n"));
    }

    let source = format!(
        "import \"pkl:yaml\"\nlocal doc = new yaml.Parser {{}}.parse(\"\"\"\n{yaml}\"\"\")\nres = \"\\(doc.items.length),\\(doc.merged.k0),\\(doc.merged.extra)\"\n"
    );

    // Run to completion under a tiny VM budget with GC on every yield.
    let mut s = compile(&source, &["res"]);
    s.start_path(0, 0, None).unwrap();
    let mut yields = 0usize;
    let mut settled = None;
    for _ in 0..500_000 {
        match s.poll(0, 4) {
            HostPoll::Yielded => {
                yields += 1;
                if yields.is_multiple_of(8) {
                    s.collect(&[]);
                }
            }
            HostPoll::Module { token, request } => {
                s.complete_module(&token, default_completion(&request).unwrap(), 0)
                    .unwrap();
            }
            HostPoll::Ready(res) => {
                settled = Some(res);
                break;
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }
    assert!(yields > 10, "expected cooperative yields, got {yields}");
    assert_eq!(
        settled.expect("settled").unwrap(),
        Value::String("250,0,2".into())
    );
    close(s);

    // 2. Cancel mid-parse after a few small-budget yields and verify zero leaked slots.
    let mut s = compile(&source, &["res"]);
    s.start_path(0, 0, None).unwrap();
    let mut cancelled = false;
    let mut seen_yields = 0usize;
    for _ in 0..100_000 {
        match s.poll(0, 4) {
            HostPoll::Yielded => {
                seen_yields += 1;
                if seen_yields == 15 {
                    s.cancel();
                    s.collect(&[]);
                    cancelled = true;
                }
            }
            HostPoll::Module { token, request } => {
                s.complete_module(&token, default_completion(&request).unwrap(), 0)
                    .unwrap();
            }
            HostPoll::Ready(Err(err)) => {
                assert!(cancelled, "cancelled before ready");
                assert_eq!(err.kind, ErrorKind::Cancelled);
                break;
            }
            other => panic!("unexpected cancel event: {other:?}"),
        }
    }
    assert!(cancelled, "reached mid-parse cancel point");
    close(s);
}
