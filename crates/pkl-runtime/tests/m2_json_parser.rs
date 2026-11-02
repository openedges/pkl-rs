//! Source-derived runtime invariants; separate pinned Java acquisition is required.
use pkl_runtime::{
    evaluation::{
        EvaluationOptions,
        session::{SourceSession, SourceSessionOptions},
    },
    host::HostPoll,
    modules::{ModuleRequestKind, default_completion},
    resources::{ReadResult, Resource},
    value::{ErrorKind, PklString, RuntimeError, Value},
};

fn session(source: &str, paths: &[&str], power: bool) -> SourceSession {
    SourceSession::compile(
        source,
        "file:///stage139-json/main.pkl",
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
const ORDINARY_BUDGET: usize = 10_000;
const PARSER_LIFECYCLE_BUDGET: usize = 8;

fn run(
    session: &mut SourceSession,
    index: usize,
    reads: &mut Vec<PklString>,
) -> Result<Value, RuntimeError> {
    drive(session, index, reads, false)
}

// Bootstrap and ordinary semantic checks use the normal budget without forced
// GC. Only the dedicated lifecycle test switches to small-budget collection,
// after the resource callback proves that authored module setup is complete.
fn drive(
    session: &mut SourceSession,
    index: usize,
    reads: &mut Vec<PklString>,
    lifecycle_gc: bool,
) -> Result<Value, RuntimeError> {
    session.start_path(index, 0, None).unwrap();
    let mut parser_progress = false;
    let mut progress_yields = 0;
    for _ in 0..100_000 {
        let budget = if parser_progress {
            PARSER_LIFECYCLE_BUDGET
        } else {
            ORDINARY_BUDGET
        };
        let event = session.poll(0, budget);
        match event {
            HostPoll::Yielded => {
                if parser_progress {
                    progress_yields += 1;
                    session.collect(&[]);
                }
            }
            HostPoll::Module { token, request } => {
                assert!(!matches!(
                    request.kind,
                    ModuleRequestKind::LoadSource { .. }
                ));
                let completion = default_completion(&request).expect("authored bundled module");
                session.complete_module(&token, completion, 0).unwrap();
            }
            HostPoll::Read { token, uri } => {
                reads.push(uri.clone());
                if lifecycle_gc {
                    // Check roots while the Host owns a pending request.
                    session.collect(&[]);
                }
                let value = match uri.as_str() {
                    Some("parse:source") => ReadResult::Resource(Resource {
                        uri: "parse:returned".into(),
                        bytes: b"{\"value\":2}".as_slice().into(),
                    }),
                    Some("parse:value") => ReadResult::Text("40".into()),
                    _ => panic!("unexpected JSON Host read: {uri:?}"),
                };
                session.complete_read(&token, Ok(value), 0).unwrap();
                if lifecycle_gc {
                    // Also check the resumed Resource/callback value before
                    // it is consumed by the next evaluator poll.
                    session.collect(&[]);
                    parser_progress = true;
                }
            }
            HostPoll::Ready(result) => {
                if lifecycle_gc {
                    assert!(parser_progress, "reached the parser resource boundary");
                    assert!(
                        progress_yields > 0,
                        "exercised small-budget parser/callback progress"
                    );
                }
                return result;
            }
            other => panic!("unexpected JSON effect: {other:?}"),
        }
    }
    panic!("JSON evaluation did not settle");
}
fn text(session: &mut SourceSession, index: usize) -> PklString {
    let Value::String(value) = run(session, index, &mut Vec::new()).unwrap() else {
        panic!("expression-string API");
    };
    value
}
fn close(mut session: SourceSession) {
    session.close();
    assert_eq!(session.collect(&[]).slots, 0);
}

#[test]
fn authored_parser_keeps_shapes_numeric_kinds_and_duplicate_key_position() {
    let source = r##"import "pkl:json"
local parser = new json.Parser {}
local mapped = new json.Parser { useMapping=true }
a=parser.parse(#"{"x":1,"a":[true,null,"z"],"x":2}"#)
b=mapped.parse(#"{"x":1,"a":2,"x":3}"#)
shape = a is Dynamic && a.a is Listing && a.x == 2 && a.a[0] == true && a.a[1] == null && a.a[2] == "z" && b is Mapping && b["x"] == 3
order = b.toMap().keys.toList().join(",")
numberKinds = parser.parse("9223372036854775807") is Int && parser.parse("9223372036854775808") is Float && parser.parse("-0") == 0 && parser.parse("1e999").isInfinite
"##;
    for power in [false, true] {
        let mut s = session(source, &["shape", "order", "numberKinds"], power);
        assert_eq!(text(&mut s, 0), "true");
        assert_eq!(text(&mut s, 1), "x,a");
        assert_eq!(text(&mut s, 2), "true");
        close(s);
    }
}

#[test]
fn mapping_paths_remain_properties_and_array_paths_are_wildcards_not_indices() {
    let source = r##"import "pkl:json"
local parser = new json.Parser {
  useMapping=true
  converters {
    ["^a[0]"] = (_) -> throw("an index is not the parser wildcard")
    ["^a[*].x"] = (v) -> v + 10
    ["^a[*]"] = (v) -> v["x"]
    [Int] = (v) -> v + 100
  }
}
result=parser.parse(#"{"a":[{"x":1},{"x":2}]}"#)["a"].toList().join(",")
"##;
    let mut s = session(source, &["result"], true);
    assert_eq!(text(&mut s, 0), "11,12");
    close(s);
}

#[test]
fn converters_run_before_later_syntax_and_for_overwritten_values_but_root_waits() {
    let source = r##"import "pkl:json"
local child = new json.Parser { converters { [Int] = (v) -> if (v == 1) throw("prefix converter") else v } }
local root = new json.Parser { converters { ["^"] = (_) -> throw("root converter") } }
invalidAfterChild=child.parse("[1,]")
overwritten=child.parse(#"{"x":1,"x":2}"#)
invalidBeforeRoot=root.parse("1 x")
valid=child.parse("2")
"##;
    let mut s = session(
        source,
        &[
            "invalidAfterChild",
            "overwritten",
            "invalidBeforeRoot",
            "valid",
        ],
        false,
    );
    for index in [0, 1, 0, 1] {
        assert_eq!(
            run(&mut s, index, &mut Vec::new()).unwrap_err().message,
            "prefix converter"
        );
        assert_eq!(text(&mut s, 3), "2");
    }
    let error = run(&mut s, 2, &mut Vec::new()).unwrap_err();
    assert_eq!(error.message, "Error parsing JSON document.");
    assert_eq!(error.hint, Some("Unexpected character at 1:3".into()));
    close(s);
}

#[test]
fn resource_input_and_converter_host_suspension_keep_partial_values_rooted() {
    let source = r#"import "pkl:json"
local parser = new json.Parser { converters { [Int]=(v) -> read("parse:value").toInt() + v } }
result=parser.parse(read("parse:source")).value
"#;
    let mut s = session(source, &["result"], false);
    let mut reads = Vec::new();
    assert_eq!(
        drive(&mut s, 0, &mut reads, true).unwrap(),
        Value::String("42".into())
    );
    assert_eq!(
        reads,
        vec![
            PklString::from("parse:source"),
            PklString::from("parse:value")
        ]
    );
    reads.clear();
    assert_eq!(
        run(&mut s, 0, &mut reads).unwrap(),
        Value::String("42".into())
    );
    assert!(reads.is_empty());
    close(s);
}

#[test]
fn parser_preserves_unpaired_utf16_units_and_counts_utf16_error_columns() {
    let source = r##"import "pkl:json"
local parser=new json.Parser {}
units=parser.parse(#""\uD800x\uDFFF""#)
keyUnits=parser.parse(#"{"\uD800x\uDFFF":1}"#).toMap().keys.toList()[0]
invalid=parser.parse(#""😀" x"#)
"##;
    let mut s = session(source, &["units", "invalid", "keyUnits"], false);
    assert_eq!(
        text(&mut s, 0),
        PklString::from_utf16(vec![0xd800, 120, 0xdfff])
    );
    assert_eq!(
        text(&mut s, 2),
        PklString::from_utf16(vec![0xd800, 120, 0xdfff])
    );
    let error = run(&mut s, 1, &mut Vec::new()).unwrap_err();
    assert_eq!(error.hint, Some("Unexpected character at 1:6".into()));
    assert!(
        !error
            .frames
            .iter()
            .any(|frame| frame.member == "#Parser.parse")
    );
    close(s);
}

#[test]
fn line_locations_distinguish_final_lf_eof_and_the_next_character() {
    let source = r#"import "pkl:json"
local parser=new json.Parser {}
eof=parser.parse("[1,\n")
next=parser.parse("[1,\n]")
valid=parser.parse("[]").length
"#;
    let mut s = session(source, &["eof", "next", "valid"], false);
    for (index, hint) in [
        (0, "Unexpected end of input at 1:5"),
        (1, "Expected value at 2:1"),
    ] {
        let error = run(&mut s, index, &mut Vec::new()).unwrap_err();
        assert_eq!(error.hint, Some(hint.into()));
        assert_eq!(text(&mut s, 2), "0");
    }
    close(s);
}

#[test]
fn cancellation_at_converter_setup_and_partial_child_discards_parser_roots() {
    let source = r##"import "pkl:json"
local parser = new json.Parser {
  converters = if (read("parse:options") == "go") new Mapping {
    [Int] = (v) -> read("parse:child").toInt() + v
  } else throw("option")
}
result = parser.parse(#"{"a":[1,2]}"#)
"##;
    for stop in ["parse:options", "parse:child"] {
        let mut s = session(source, &["result"], false);
        s.start_path(0, 0, None).unwrap();
        let mut pending = None;
        let mut after_options = false;
        for _ in 0..100_000 {
            let budget = if after_options {
                PARSER_LIFECYCLE_BUDGET
            } else {
                ORDINARY_BUDGET
            };
            let event = s.poll(0, budget);
            match event {
                HostPoll::Yielded => {
                    if after_options {
                        s.collect(&[]);
                    }
                }
                HostPoll::Module { token, request } => {
                    assert!(!matches!(
                        request.kind,
                        ModuleRequestKind::LoadSource { .. }
                    ));
                    s.complete_module(&token, default_completion(&request).unwrap(), 0)
                        .unwrap();
                }
                HostPoll::Read { token, uri } if uri == stop => {
                    s.collect(&[]);
                    pending = Some(token);
                    break;
                }
                HostPoll::Read { token, uri } => {
                    assert_eq!(uri, "parse:options");
                    s.collect(&[]);
                    s.complete_read(&token, Ok(ReadResult::Text("go".into())), 0)
                        .unwrap();
                    s.collect(&[]);
                    after_options = true;
                }
                other => panic!("unexpected parser cancellation setup: {other:?}"),
            }
        }
        let token = pending.expect("reached selected parser boundary");
        s.cancel();
        s.collect(&[]);
        let HostPoll::Ready(Err(error)) = s.poll(0, ORDINARY_BUDGET) else {
            panic!("cancelled parser terminal");
        };
        assert_eq!(error.kind, ErrorKind::Cancelled);
        assert!(
            s.complete_read(&token, Ok(ReadResult::Text("0".into())), 0)
                .is_err()
        );
        close(s);
    }
}

#[test]
fn root_converter_result_is_not_rechecked_against_stdlib_return_annotation() {
    let source = r#"import "pkl:json"
local parser = new json.Parser { converters { ["^"] = (_) -> (x) -> x } }
result = parser.parse("1")
function checkedUser(): json.Value = result
checked = checkedUser()
"#;
    for power in [false, true] {
        let mut s = session(source, &["result", "checked"], power);
        // Actual pinned Java root-wrong-return on/off observation; the full
        // raw oracle harness additionally compares its complete channels.
        assert_eq!(text(&mut s, 0), "new Function1 {}");
        assert_eq!(
            run(&mut s, 1, &mut Vec::new()).unwrap_err().kind,
            ErrorKind::Type
        );
        assert_eq!(text(&mut s, 0), "new Function1 {}");
        close(s);
    }
}

#[test]
fn dynamic_default_is_checked_on_access_but_hidden_from_rendering() {
    let source = r##"import "pkl:json"
local parser = new json.Parser {}
local mappingParser = new json.Parser { useMapping = true }
value = parser.parse(#"{"default":1,"other":2}"#)
direct = value.default
rendered = value.toString()
mapped = mappingParser.parse(#"{"default":1}"#)["default"]
"##;
    for power in [false, true] {
        let mut s = session(source, &["direct", "rendered", "mapped"], power);
        // Actual Java dynamic-default-render observation, not a bypass of
        // Dynamic.default's inherited Function1 constraint on direct access.
        assert_eq!(text(&mut s, 1), "new Dynamic { other = 2 }");
        for _ in 0..2 {
            assert_eq!(
                run(&mut s, 0, &mut Vec::new()).unwrap_err().kind,
                ErrorKind::Type
            );
            assert_eq!(text(&mut s, 1), "new Dynamic { other = 2 }");
            assert_eq!(text(&mut s, 2), "1");
        }
        close(s);
    }
}
