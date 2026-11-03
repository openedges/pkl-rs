//! Source-derived runtime invariants; separate pinned Java acquisition is required.
use pkl_runtime::{
    evaluation::{
        EvaluationOptions,
        session::{SourceSession, SourceSessionOptions},
    },
    host::HostPoll,
    logging::{LogDelivery, LogLevel},
    modules::{ModuleRequestKind, default_completion},
    resources::{ReadResult, Resource},
    value::{ErrorKind, PklString, RuntimeError, Value},
};

fn session(source: &str, paths: &[&str], power: bool) -> SourceSession {
    SourceSession::compile(
        source,
        "file:///stage142-yaml/main.pkl",
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
                        bytes: b"value: 2\n".as_slice().into(),
                    }),
                    Some("parse:value") => ReadResult::Text("40".into()),
                    _ => panic!("unexpected YAML Host read: {uri:?}"),
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
            other => panic!("unexpected YAML effect: {other:?}"),
        }
    }
    panic!("YAML evaluation did not settle");
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
fn modes_tags_and_parse_all_keep_authored_types() {
    let source = r#"import "pkl:yaml"
local compat=new yaml.Parser {}
local core=new yaml.Parser {mode="1.2"}
shape=compat.parse("yes") is Boolean && core.parse("yes") is String && compat.parse("012")==10 && core.parse("012")==12
values=compat.parseAll("--- 1\n--- 2\n").join(",")
empty=compat.parseAll("").isEmpty && compat.parse("")==null
binary=compat.parse("!!binary SGVsbG8=") is Bytes
"#;
    for power in [false, true] {
        let mut s = session(source, &["shape", "values", "empty", "binary"], power);
        assert_eq!(text(&mut s, 0), "true");
        assert_eq!(text(&mut s, 1), "1,2");
        assert_eq!(text(&mut s, 2), "true");
        assert_eq!(text(&mut s, 3), "true");
        close(s);
    }
}

#[test]
fn root_paths_and_original_keys_determine_conversion_edges() {
    let source = r#"import "pkl:yaml"
local root=new yaml.Parser {converters { ["^"]=(_) -> "root";[Int]=(_) -> 9 }}
local keyed=new yaml.Parser {converters {[String]=(v) -> v + "x";["^a"]=(_) -> 9}}
single=root.parse("1")
all=root.parseAll("1")[0]
keyedValue=keyed.parse("a: 1").ax
"#;
    let mut s = session(source, &["single", "all", "keyedValue"], false);
    assert_eq!(text(&mut s, 0), "9");
    assert_eq!(text(&mut s, 1), "root");
    assert_eq!(text(&mut s, 2), "9");
    close(s);
}

#[test]
fn syntax_precedes_children_but_parse_all_constructs_before_next_document() {
    let source = r#"import "pkl:yaml"
local parser=new yaml.Parser {converters {[Int]=(_) -> throw("child")}}
syntax=parser.parse("[1,")
child=parser.parse("[1]")
root=parser.parse("1")
first=parser.parseAll("--- [1]\n--- [\n")
good=(new yaml.Parser {}).parse("2")
"#;
    let mut s = session(source, &["syntax", "child", "root", "first", "good"], false);
    let syntax = run(&mut s, 0, &mut Vec::new()).unwrap_err();
    assert_eq!(syntax.message, "Error parsing YAML document.");
    assert_ne!(
        syntax.hint,
        Some("org.pkl.core.runtime.VmEvalException: child".into())
    );
    for index in [1, 3, 1] {
        let e = run(&mut s, index, &mut Vec::new()).unwrap_err();
        assert_eq!(e.message, "Error parsing YAML document.");
        assert_eq!(
            e.hint,
            Some("org.pkl.core.runtime.VmEvalException: child".into())
        );
    }
    assert_eq!(
        run(&mut s, 2, &mut Vec::new()).unwrap_err().message,
        "child"
    );
    assert_eq!(text(&mut s, 4), "2");
    close(s);
}

#[test]
fn resource_and_child_host_suspension_keep_partial_values_rooted() {
    let source = r#"import "pkl:yaml"
local parser=new yaml.Parser {converters {[Int]=(v) -> read("parse:value").toInt()+v}}
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
fn alias_graphs_keep_shell_length_and_key_preflight_precedes_pair_conversion() {
    let source = r#"import "pkl:yaml"
local parser=new yaml.Parser {}
recursive=parser.parse("&a [*a]").length
shared=parser.parse("a: &a [1]\nb: *a\n")
shape=shared.a.length==1 && shared.b[0]==1
local duplicate=new yaml.Parser {converters {[String]=(_) -> throw("key converter must wait")}}
invalid=duplicate.parse("a: 1\na: 2\n")
"#;
    let mut s = session(source, &["recursive", "shape", "invalid"], true);
    assert_eq!(text(&mut s, 0), "1");
    assert_eq!(text(&mut s, 1), "true");
    let e = run(&mut s, 2, &mut Vec::new()).unwrap_err();
    assert_eq!(e.message, "Error parsing YAML document.");
    assert!(
        e.hint
            .as_ref()
            .unwrap()
            .as_str()
            .unwrap()
            .contains("found duplicate key a")
    );
    close(s);
}

#[test]
fn cancellation_at_child_callback_rejects_late_completion_and_reclaims_roots() {
    let source = r#"import "pkl:yaml"
local parser=new yaml.Parser {converters {[Int]=(v) -> read("parse:child").toInt()+v}}
result=parser.parse("a: [1,2]")
"#;
    let mut s = session(source, &["result"], false);
    s.start_path(0, 0, None).unwrap();
    let mut pending = None;
    for _ in 0..100_000 {
        match s.poll(0, ORDINARY_BUDGET) {
            HostPoll::Yielded => (),
            HostPoll::Module { token, request } => {
                s.complete_module(&token, default_completion(&request).unwrap(), 0)
                    .unwrap();
            }
            HostPoll::Read { token, uri } => {
                assert_eq!(uri, "parse:child");
                s.collect(&[]);
                pending = Some(token);
                break;
            }
            other => panic!("unexpected YAML cancellation event: {other:?}"),
        }
    }
    let token = pending.expect("converter Host boundary");
    s.cancel();
    s.collect(&[]);
    let HostPoll::Ready(Err(error)) = s.poll(0, ORDINARY_BUDGET) else {
        panic!("cancel terminal")
    };
    assert_eq!(error.kind, ErrorKind::Cancelled);
    assert!(
        s.complete_read(&token, Ok(ReadResult::Text("0".into())), 0)
            .is_err()
    );
    close(s);
}

#[test]
fn each_alias_edge_converts_the_cached_raw_node_again() {
    let source = r#"import "pkl:yaml"
local parser=new yaml.Parser {converters {[Listing]=(_) -> trace(read("parse:value").toInt())}}
result=let (v=parser.parse("a: &shared [1]\nb: *shared\n")) v.a+v.b
"#;
    let mut s = SourceSession::compile(
        source,
        "file:///stage142-yaml/main.pkl",
        "main",
        &["result".to_owned()],
        SourceSessionOptions {
            log_delivery: LogDelivery::Host,
            ..Default::default()
        },
    )
    .unwrap();
    let mut reads = Vec::new();
    let mut logs = Vec::new();
    // A second query also proves that the successful result property is cached;
    // it must not execute either converter or the resource reader again.
    for _ in 0..2 {
        s.start_path(0, 0, None).unwrap();
        let mut result = None;
        for _ in 0..100_000 {
            match s.poll(0, ORDINARY_BUDGET) {
                HostPoll::Yielded => (),
                HostPoll::Module { token, request } => {
                    s.complete_module(&token, default_completion(&request).unwrap(), 0)
                        .unwrap();
                }
                HostPoll::Read { token, uri } => {
                    assert_eq!(uri, "parse:value");
                    reads.push(uri);
                    s.collect(&[]);
                    s.complete_read(&token, Ok(ReadResult::Text("40".into())), 0)
                        .unwrap();
                    s.collect(&[]);
                }
                HostPoll::Log { token, event } => {
                    logs.push((*event).clone());
                    s.collect(&[]);
                    s.acknowledge_log(&token, Ok(()), 0).unwrap();
                    s.collect(&[]);
                }
                HostPoll::Ready(value) => {
                    result = Some(value);
                    break;
                }
                other => panic!("unexpected aliased-converter effect: {other:?}"),
            }
        }
        assert_eq!(
            result
                .expect("aliased-converter evaluation settled")
                .unwrap(),
            Value::String("80".into())
        );
        assert_eq!(reads, vec![PklString::from("parse:value")]);
        assert_eq!(logs.len(), 2, "each alias edge invokes its converter");
        for event in &logs {
            assert_eq!(event.level, LogLevel::Trace);
            assert_eq!(event.message, r#"read("parse:value").toInt() = 40"#);
            assert_eq!(event.frame.module_uri, "file:///stage142-yaml/main.pkl");
        }
    }
    close(s);
}

#[test]
fn mapping_hash_threshold_and_converted_key_replacement_preserve_values() {
    let source = r#"import "pkl:yaml"
local p=new yaml.Parser {useMapping=true}
result=let (v=p.parse("0: a\n1: b\n2: c\n3: d\n4: e\n5: f\n6: g\n7: h\n")) v.length==8 && v[7]=="h" && v[0]=="a"
local collapse=new yaml.Parser {useMapping=true;converters {[Int]=(_) -> 0}}
last=collapse.parse("1: first\n2: last\n")[0]
"#;
    let mut s = session(source, &["result", "last"], false);
    assert_eq!(text(&mut s, 0), "true");
    assert_eq!(text(&mut s, 1), "last");
    close(s);
}
