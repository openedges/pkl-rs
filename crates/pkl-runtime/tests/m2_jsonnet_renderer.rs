//! Source-derived development controls; raw Java differential evidence is separate.
use pkl_runtime::{
    evaluation::{
        frontend::{ModuleEvalOptions, ModuleSourceInput, evaluate_module_output},
        session::{SourceSession, SourceSessionOptions},
    },
    host::HostPoll,
    modules::{ModuleRequestKind, default_completion},
    resources::ReadResult,
    value::{ErrorKind, PklString, Value},
};

fn output(source: &str, options: &ModuleEvalOptions) -> Vec<u8> {
    let result = evaluate_module_output(
        ModuleSourceInput {
            text: source,
            uri: "file:///stage140-jsonnet/main.pkl",
            inferred_name: "main",
        },
        options,
    );
    assert_eq!(
        result.exit_code,
        0,
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(result.stderr.is_empty());
    result.stdout
}

#[test]
fn inline_and_indented_layout_omit_entries_but_preserve_null_elements() {
    let source = r#"import "pkl:jsonnet"
output { renderer = new jsonnet.Renderer { indent="" }; value = new Dynamic {
  first=List(1,null,"x")
  omitted=null
  second=Map("gone", null, "keep", 2)
  empty=new Dynamic {}
} }
"#;
    assert_eq!(
        output(source, &Default::default()),
        b"{ first: [1, null, 'x'], second: { keep: 2 }, empty: {} }\n"
    );
    assert_eq!(
        output(
            r#"import "pkl:jsonnet"
output { renderer=new jsonnet.Renderer {}; value=List(1,new Dynamic { a=2 }) }
"#,
            &Default::default()
        ),
        b"[\n  1,\n  {\n    a: 2,\n  },\n]\n"
    );
    assert_eq!(
        output(
            r#"import "pkl:jsonnet"
output { renderer=new jsonnet.Renderer { indent=null }; value=List(1,2) }
"#,
            &Default::default()
        ),
        b"[1, 2]\n"
    );
}

#[test]
fn wrappers_are_authored_values_not_host_reads_or_lookalike_class_names() {
    let source = r#"import "pkl:jsonnet"
class ImportStr { path: String }
output { renderer=new jsonnet.Renderer { indent="" }; value=new Dynamic {
  imported=jsonnet.ImportStr("a\nb")
  `external`=jsonnet.ExtVar("x'y")
  ordinary=new ImportStr { path="ordinary" }
} }
"#;
    assert_eq!(output(source, &Default::default()), b"{ imported: importstr 'a\\nb', external: std.extVar(\"x'y\"), ordinary: { path: 'ordinary' } }\n");
}

#[test]
fn annotation_conversion_preserves_original_value_path_and_renderer_identity() {
    let source = r#"import "pkl:jsonnet"
class Holder {
 @jsonnet.Property { name="local" } original: Int=7
 omitted: String?=null
}
output { value=new Holder {}; renderer=new jsonnet.Renderer {
 indent=""
 converters { ["original"]=(v)->v+1; ["local"]=(_)->throw("renamed path") }
} }
"#;
    assert_eq!(output(source, &Default::default()), b"{ 'local': 8 }\n");
    let pcf = source.replace("renderer=new jsonnet.Renderer", "renderer=new PcfRenderer");
    assert_eq!(
        output(&pcf, &Default::default()),
        b"original = 8\nomitted = null\n"
    );
}

#[test]
fn default_format_uses_same_authored_renderer_and_document_terminator() {
    let options = ModuleEvalOptions {
        properties: vec![("pkl.outputFormat".into(), "jsonnet".into())],
        ..Default::default()
    };
    assert_eq!(output("value=1", &options), b"{\n  value: 1,\n}\n");
    assert_eq!(
        output(
            r#"import "pkl:jsonnet"
output { renderer=new jsonnet.Renderer {}; value=new RenderDirective { text=""; bytes=throw("unused") } }
"#,
            &Default::default()
        ),
        b"\n"
    );
}

fn session(source: &str) -> SourceSession {
    SourceSession::compile(
        source,
        "file:///stage140-jsonnet/main.pkl",
        "main",
        &["result".to_owned()],
        SourceSessionOptions::default(),
    )
    .unwrap()
}
fn complete_module(
    s: &mut SourceSession,
    token: &str,
    request: &pkl_runtime::modules::ModuleRequest,
) {
    assert!(!matches!(
        request.kind,
        ModuleRequestKind::LoadSource { .. }
    ));
    s.complete_module(
        token,
        default_completion(request).expect("bundled module"),
        0,
    )
    .unwrap();
}

#[test]
fn converter_wrapper_and_directive_roots_survive_host_gc_in_order() {
    let source = r#"import "pkl:jsonnet"
local renderer=new jsonnet.Renderer {
 indent=""
 converters { [Int]=(v)->if (read("render:convert") == "go") jsonnet.ExtVar(read("render:name")) else throw("bad") }
}
result=renderer.renderValue(new Dynamic {
 a=7
 b=new RenderDirective { text=read("render:directive"); bytes=throw("unused") }
})
"#;
    let mut s = session(source);
    s.start_path(0, 0, None).unwrap();
    let mut reads = Vec::new();
    let mut result = None;
    let mut lifecycle = false;
    for _ in 0..100_000 {
        match s.poll(0, if lifecycle { 16 } else { 10_000 }) {
            HostPoll::Yielded => {
                if lifecycle {
                    s.collect(&[]);
                }
            }
            HostPoll::Module { token, request } => complete_module(&mut s, &token, &request),
            HostPoll::Read { token, uri } => {
                s.collect(&[]);
                reads.push(uri.clone());
                let text = match uri.as_str() {
                    Some("render:convert") => PklString::from("go"),
                    Some("render:name") => PklString::from_utf16(vec![0xd800]),
                    Some("render:directive") => PklString::from_utf16(vec![0xdc00]),
                    _ => panic!("unexpected read: {uri:?}"),
                };
                s.complete_read(&token, Ok(ReadResult::Text(text)), 0)
                    .unwrap();
                s.collect(&[]);
                lifecycle = true;
            }
            HostPoll::Ready(value) => {
                result = Some(value.unwrap());
                break;
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }
    assert_eq!(
        reads,
        ["render:convert", "render:name", "render:directive"].map(PklString::from)
    );
    let mut expected = "{ a: std.extVar('".encode_utf16().collect::<Vec<_>>();
    expected.push(0xd800);
    expected.extend("'), b: ".encode_utf16());
    expected.push(0xdc00);
    expected.extend(" }".encode_utf16());
    let Value::String(owned) = result.expect("render settled") else {
        panic!("expression-string contract")
    };
    assert_eq!(owned.encode_utf16().collect::<Vec<_>>(), expected);
    s.close();
    assert_eq!(s.collect(&[]).slots, 0);
    assert_eq!(owned.encode_utf16().collect::<Vec<_>>(), expected);
}

#[test]
fn pending_special_wrapper_getter_cancellation_releases_owned_render_state() {
    let mut s = session(
        r#"import "pkl:jsonnet"
result=new jsonnet.Renderer {}.renderValue(new jsonnet.ImportStr { path=read("render:pending") })
"#,
    );
    s.start_path(0, 0, None).unwrap();
    let mut pending = None;
    for _ in 0..100_000 {
        match s.poll(0, 10_000) {
            HostPoll::Yielded => (),
            HostPoll::Module { token, request } => complete_module(&mut s, &token, &request),
            HostPoll::Read { token, uri } => {
                assert_eq!(uri, "render:pending");
                s.collect(&[]);
                pending = Some(token);
                break;
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }
    let token = pending.expect("wrapper getter reached");
    s.cancel();
    s.collect(&[]);
    let HostPoll::Ready(Err(error)) = s.poll(0, 10_000) else {
        panic!("cancelled render")
    };
    assert_eq!(error.kind, ErrorKind::Cancelled);
    assert!(
        s.complete_read(&token, Ok(ReadResult::Text("late".into())), 0)
            .is_err()
    );
    s.close();
    assert_eq!(s.collect(&[]).slots, 0);
}
