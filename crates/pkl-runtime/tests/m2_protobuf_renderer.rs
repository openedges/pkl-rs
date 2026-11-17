//! Source-derived development assertions, NOT Java oracle observations.
//! Authority: pinned RendererNodes.java / AbstractRenderer.java / TypeNode.java.
//! This draft has not been compiled or executed; see its provenance manifest.
use pkl_runtime::{
    evaluation::{
        frontend::{ModuleEvalOptions, ModuleSourceInput, evaluate_module_output},
        session::{SourceSession, SourceSessionOptions},
    },
    host::{HostPoll, ReadFailure},
    modules::{ModuleRequestKind, default_completion},
    resources::ReadResult,
    value::ErrorKind,
};
fn output(source: &str, options: ModuleEvalOptions) -> Vec<u8> {
    let result = evaluate_module_output(
        ModuleSourceInput {
            text: source,
            uri: "file:///stage145-protobuf/main.pkl",
            inferred_name: "main",
        },
        &options,
    );
    assert_eq!(
        result.exit_code,
        0,
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(result.stderr, b"");
    result.stdout
}
#[test]
fn default_format_and_authored_document_share_exact_layout() {
    let options = ModuleEvalOptions {
        properties: vec![("pkl.outputFormat".into(), "textproto".into())],
        ..Default::default()
    };
    assert_eq!(
        output("first=1\nsecond=\"x\"", options),
        b"first: 1\nsecond: \"x\"\n"
    );
    assert_eq!(
        output(
            r#"import "pkl:protobuf"
class Input { first=1; second="x" }
output { value=new Input {}; renderer=new protobuf.Renderer {} }
"#,
            Default::default()
        ),
        b"first: 1\nsecond: \"x\"\n"
    );
}
#[test]
fn empty_indent_and_directive_final_lf_do_not_add_document_newline() {
    assert_eq!(
        output(
            r#"import "pkl:protobuf"
class Input { first=1; second="x" }
output { value=new Input {}; renderer=new protobuf.Renderer { indent="" } }
"#,
            Default::default()
        ),
        b"first: 1second: \"x\""
    );
    assert_eq!(
        output(
            r#"import "pkl:protobuf"
output { value=new RenderDirective { text="already\n"; bytes=throw("unused") }; renderer=new protobuf.Renderer {} }
"#,
            Default::default()
        ),
        b"already\n"
    );
}
#[test]
fn normalized_union_wrappers_and_string_literals_use_declared_metadata() {
    assert_eq!(
        output(
            r#"import "pkl:protobuf"
typealias Alias = "a" | "b" | Int8
class Input {
 scalar: Alias = 4
 literal: "a" | "b" = "b"
 values: Listing<Alias> = new { "a"; 5 }
 nullable: (Int | String)? = null
}
output { value=new Input {}; renderer=new protobuf.Renderer { indent="" } }
"#,
            Default::default()
        ),
        b"scalar: {it_Int: 4}literal: \"b\"values: {it_String: \"a\"}values: {it_Int: 5}"
    );
}
#[test]
fn detached_listing_alternative_is_explicitly_unqualified_before_elements() {
    // Java04 constraint-lazy-list observes an upstream internal failure.
    // This is an honest unsupported boundary, not byte-equivalent diagnostics.
    let result = evaluate_module_output(
        ModuleSourceInput {
            text: include_str!(
                "../../../tests/fixtures/m2-protobuf-java04/constraint-lazy-list/module.pkl"
            ),
            uri: "file:///stage145-protobuf/constraint-lazy-list/main.pkl",
            inferred_name: "main",
        },
        &ModuleEvalOptions::default(),
    );
    assert_ne!(result.exit_code, 0);
    assert!(result.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        stderr.contains("TypeNode$ListingOrMappingTypeNode.getRootNode()")
            || stderr.contains("detached Listing alternative")
    );
}
#[test]
fn detached_mapping_alternative_is_explicitly_unqualified_before_elements() {
    let text = r#"import "pkl:protobuf"
class Input {
  value: Mapping<String, Int> | String = "seed"
}
output {
  value = new Input {}
  renderer = new protobuf.Renderer {
    converters {
      ["value"] = (_) -> new Mapping {}
    }
  }
}
"#;
    let result = evaluate_module_output(
        ModuleSourceInput {
            text,
            uri: "file:///stage159-protobuf/detached-mapping/main.pkl",
            inferred_name: "main",
        },
        &ModuleEvalOptions::default(),
    );
    assert_ne!(result.exit_code, 0);
    assert!(result.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        stderr.contains("TypeNode$ListingOrMappingTypeNode.getRootNode()")
            || stderr.contains("detached Mapping alternative")
    );

    // Verify foreign exception provenance frames for detached mapping (1642) vs listing (1572)
    let prov = pkl_runtime::java_exception::provenance::ForeignExceptionProvenance::new(
        pkl_runtime::java_exception::provenance::ForeignOperation::ProtobufListingOrMappingRootNullPointer {
            is_mapping: true,
        },
    );
    let bug = prov
        .project_pkl_bug(&pkl_runtime::value::PklString::from("ordinary\n"))
        .to_java_utf8();
    assert!(
        bug.contains("TypeNode$ListingOrMappingTypeNode.getRootNode()")
            && bug.contains("TypeNode$MappingTypeNode.executeLazily")
            && bug.contains("TypeNode.java:1642")
    );

    let prov_listing = pkl_runtime::java_exception::provenance::ForeignExceptionProvenance::new(
        pkl_runtime::java_exception::provenance::ForeignOperation::ProtobufListingOrMappingRootNullPointer {
            is_mapping: false,
        },
    );
    let bug_listing = prov_listing
        .project_pkl_bug(&pkl_runtime::value::PklString::from("ordinary\n"))
        .to_java_utf8();
    assert!(
        bug_listing.contains("TypeNode$ListingTypeNode.executeLazily")
            && bug_listing.contains("TypeNode.java:1572")
    );
}
#[test]
fn inherited_type_owner_and_final_renamed_name_are_distinct_from_converter_path() {
    assert_eq!(
        output(
            r#"import "pkl:protobuf"
 local typealias Choice=Int | String
open class Parent {
 inherited: Choice=1
 @protobuf.Property { name="plain" }
 original: Int | String=2
 hidden plain: Int=0
}
class Child extends Parent { inherited="child" }
output { value=new Child {}; renderer=new protobuf.Renderer {
 indent=""
 converters { ["original"]=(v)->v+1; ["plain"]=(_)->throw("renamed converter path") }
} }
"#,
            Default::default()
        ),
        b"inherited: {it_String: \"child\"}plain: 3"
    );
}
#[test]
fn nested_collection_wrappers_and_converted_null_omission() {
    assert_eq!(output(r#"import "pkl:protobuf"
class Input {
 nested: Listing<Listing<String>> = new { new { "a"; "b" }; new {} }
 map: Map<String, List<Int>> = Map("a", List(1,2), "empty", List())
 omitted: String? = "omit"
}
output { value=new Input {}; renderer=new protobuf.Renderer {
 indent=""; converters { ["omitted"]=(_)->null }
} }
"#, Default::default()), b"nested: {it: \"a\"it: \"b\"}nested: {}map: {key: \"a\"value: {it: 1it: 2}}map: {key: \"empty\"value: {}}");
}
#[test]
fn duration_floor_and_negative_nanos_are_not_normalized() {
    assert_eq!(
        output(
            r#"import "pkl:protobuf"
class Input { negative=(-1.5).s; positive=42.ns }
output { value=new Input {}; renderer=new protobuf.Renderer { indent="" } }
"#,
            Default::default()
        ),
        b"negative: {seconds: -2nanos: -500000000}positive: {seconds: 0nanos: 42}"
    );
}
#[test]
fn more_specific_class_union_is_selected_before_its_ancestor() {
    assert_eq!(
        output(
            r#"import "pkl:protobuf"
open class Parent { x=1 }
class Child extends Parent {}
class Input { choice: Parent | Child = new Child {} }
output { value=new Input {}; renderer=new protobuf.Renderer { indent="" } }
"#,
            Default::default()
        ),
        b"choice: {it_Child: {x: 1}}"
    );
}
fn session(source: &str) -> SourceSession {
    SourceSession::compile(
        source,
        "file:///stage145-protobuf/main.pkl",
        "main",
        &["result".to_owned(), "nested".to_owned()],
        SourceSessionOptions::default(),
    )
    .unwrap()
}
fn event(s: &mut SourceSession, collect: bool) -> HostPoll {
    // The qualified same-source poll1/GC-every-poll diagnostic reaches the
    // intended boundary at 237679 polls. This is an evidence-based stress-work
    // allowance, not a performance claim; retain the noncollecting condition.
    let polls = if collect { 400_000 } else { 200_000 };
    for _ in 0..polls {
        if collect {
            s.collect(&[]);
        }
        match s.poll(0, if collect { 1 } else { 10_000 }) {
            HostPoll::Yielded => (),
            HostPoll::Module { token, request } => {
                assert!(!matches!(
                    request.kind,
                    ModuleRequestKind::LoadSource { .. }
                ));
                s.complete_module(
                    &token,
                    default_completion(&request).expect("bundled module"),
                    0,
                )
                .unwrap();
            }
            result => return result,
        }
    }
    panic!("protobuf lifecycle did not settle")
}
#[test]
fn detached_listing_failure_precedes_host_element_and_tail_and_releases_roots() {
    let mut s = session(
        r#"import "pkl:protobuf"
class Input { choice: Listing<Int> | String="seed"; tail=new RenderDirective { text=read("proto:tail"); bytes=throw("unused") } }
local renderer=new protobuf.Renderer { converters { ["choice"]=(_)->new Listing { read("proto:element") } } }
result=renderer.renderValue(new Input {})
nested=null
"#,
    );
    s.start_path(0, 0, None).unwrap();
    let HostPoll::Ready(Err(error)) = event(&mut s, true) else {
        panic!("detached Listing must stop before element/tail Host reads")
    };
    assert_eq!(error.kind, ErrorKind::Unsupported);
    assert!(
        error
            .message
            .contains("TypeNode$ListingOrMappingTypeNode.getRootNode()")
            || error.message.contains("detached Listing alternative")
    );
    s.close();
    assert_eq!(s.collect(&[]).slots, 0);
}
#[test]
fn alternative_constraint_host_failure_is_not_a_mismatch() {
    let mut s = session(
        r#"import "pkl:protobuf"
class Input { choice: Int(read("proto:constraint") == "yes") | String = "seed" }
result=new protobuf.Renderer { converters { ["choice"]=(_)->7 } }.renderValue(new Input {})
nested=null
"#,
    );
    s.start_path(0, 0, None).unwrap();
    let HostPoll::Read { token, uri } = event(&mut s, false) else {
        panic!("constraint read")
    };
    assert_eq!(uri, "proto:constraint");
    s.collect(&[]);
    s.complete_read(
        &token,
        Err(ReadFailure::Io("original host failure".into())),
        0,
    )
    .unwrap();
    let HostPoll::Ready(Err(error)) = event(&mut s, true) else {
        panic!("fatal Host result")
    };
    assert_eq!(
        error.message,
        "I/O error reading resource `proto:constraint`.\nIOException: original host failure"
    );
    s.close();
    assert_eq!(s.collect(&[]).slots, 0);
}
#[test]
fn pending_alternative_cancellation_invalidates_token_and_releases_roots() {
    let mut s = session(
        r#"import "pkl:protobuf"
class Input { choice: Int(read("proto:constraint") == "yes") | String = "seed" }
result=new protobuf.Renderer { converters { ["choice"]=(_)->7 } }.renderValue(new Input {})
nested=null
"#,
    );
    s.start_path(0, 0, None).unwrap();
    let HostPoll::Read { token, .. } = event(&mut s, false) else {
        panic!("constraint read")
    };
    s.collect(&[]);
    s.cancel();
    let HostPoll::Ready(Err(error)) = event(&mut s, true) else {
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
#[test]
fn render_type_uses_mirror_class_and_value_entrypoint_has_no_terminator() {
    assert_eq!(
        output(
            r#"import "pkl:protobuf"
import "pkl:reflect"
output { text=new protobuf.Renderer {}.renderType(reflect.intType) }
"#,
            Default::default()
        ),
        b"DeclaredType"
    );
    assert_eq!(
        output(
            r#"import "pkl:protobuf"
output { text=new protobuf.Renderer {}.renderValue("x") }
"#,
            Default::default()
        ),
        b"\"x\""
    );
}
#[test]
fn typed_builtin_node_does_not_apply_nonfinal_class_subtype_rejection() {
    assert_eq!(
        output(
            r#"import "pkl:protobuf"
class Child { x=1 }
class Input { value: Typed=new Child {} }
output { value=new Input {}; renderer=new protobuf.Renderer { indent="" } }
"#,
            Default::default()
        ),
        b"value: {x: 1}"
    );
}
