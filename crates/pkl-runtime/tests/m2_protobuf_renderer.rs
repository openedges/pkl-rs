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
#[test]
fn parameterized_type_alias_union_uses_unparameterized_alias_body_and_rejects_type_variable() {
    use pkl_runtime::modules::{ModuleCompletion, ModuleKey, ModuleKeyKind};

    let inline_result = evaluate_module_output(
        ModuleSourceInput {
            text: r#"import "pkl:protobuf"
typealias Choice<T> = T | String
open class Base { value: Choice<Int> = 1 }
class Child extends Base { value = "child" }
output { value = new Child {}; renderer = new protobuf.Renderer { indent = "" } }
"#,
            uri: "file:///stage145-protobuf/inline-alias/main.pkl",
            inferred_name: "main",
        },
        &ModuleEvalOptions::default(),
    );
    assert_ne!(inline_result.exit_code, 0);
    assert!(inline_result.stdout.is_empty());
    let inline_stderr = String::from_utf8_lossy(&inline_result.stderr);
    assert!(
        inline_stderr.contains("Failed to resolve type required for rendering protobuf."),
        "{inline_stderr}"
    );

    let types_source = r#"module types
typealias Choice<T> = T | String
open class Parent { value: Choice<Int> = 1 }
class Child extends Parent { value = "child" }
"#;
    let types_uri = "file:///stage145-protobuf/types.pkl";
    let mut s = session(
        r#"import "pkl:protobuf"
import "types.pkl"
result = new protobuf.Renderer { indent = "" }.renderDocument(new types.Child {})
nested = null
"#,
    );
    s.start_path(0, 0, None).unwrap();
    let outcome = loop {
        match s.poll(0, 10_000) {
            HostPoll::Yielded => {}
            HostPoll::Module { token, request } => {
                let completion = if let Some(default) = default_completion(&request) {
                    default
                } else {
                    match &request.kind {
                        ModuleRequestKind::SelectKey { id, uri, .. } if uri == types_uri => {
                            ModuleCompletion::KeySelected(Ok(ModuleKey {
                                id: *id,
                                uri: uri.clone(),
                                kind: ModuleKeyKind::Synthetic,
                                handle: "types".into(),
                            }))
                        }
                        ModuleRequestKind::QueryCachePolicy { .. } => {
                            ModuleCompletion::CachePolicy(Ok(true))
                        }
                        ModuleRequestKind::Resolve { original_uri, .. }
                            if original_uri == types_uri =>
                        {
                            ModuleCompletion::Resolved(Ok(pkl_runtime::modules::ResolvedModule {
                                original_uri: original_uri.clone(),
                                resolved_uri: original_uri.clone(),
                                source_uri: original_uri.clone(),
                            }))
                        }
                        ModuleRequestKind::LoadSource { .. } => {
                            ModuleCompletion::Source(Ok(pkl_runtime::modules::ModuleSource {
                                text: types_source.into(),
                                inferred_name: "types".into(),
                            }))
                        }
                        other => panic!("unexpected module request {other:?}"),
                    }
                };
                s.complete_module(&token, completion, 0).unwrap();
            }
            other => break other,
        }
    };
    let HostPoll::Ready(Err(error)) = outcome else {
        panic!("imported parameterized alias TypeVariable must be rejected: {outcome:?}");
    };
    assert_eq!(error.kind, ErrorKind::Unsupported);
    assert!(
        error
            .message
            .contains("Failed to resolve type required for rendering protobuf."),
        "{error:?}"
    );
    s.close();
    assert_eq!(s.collect(&[]).slots, 0);

    assert_eq!(
        output(
            r#"import "pkl:protobuf"
typealias SafeChoice<T> = String | T
open class Base { value: SafeChoice<Int> = 1 }
class Child extends Base { value = "child" }
output { value = new Child {}; renderer = new protobuf.Renderer { indent = "" } }
"#,
            Default::default()
        ),
        b"value: {it_String: \"child\"}"
    );
}
#[test]
fn constrained_union_alternative_evaluates_constraint_before_rejecting_null_type_class() {
    let mut s = session(
        r#"import "pkl:protobuf"
class Input { choice: Int(read("proto:constraint") == "yes") | String = "seed" }
result = new protobuf.Renderer { converters { ["choice"] = (_) -> 7 } }.renderValue(new Input {})
nested = null
"#,
    );
    s.start_path(0, 0, None).unwrap();
    let HostPoll::Read { token, uri } = event(&mut s, false) else {
        panic!("constraint read")
    };
    assert_eq!(uri, "proto:constraint");
    s.complete_read(&token, Ok(ReadResult::Text("yes".into())), 0)
        .unwrap();
    let HostPoll::Ready(Err(error)) = event(&mut s, true) else {
        panic!("constrained union alternative must fail after constraint passes")
    };
    assert_eq!(error.kind, ErrorKind::Unsupported);
    assert!(
        error
            .message
            .contains("Failed to resolve type required for rendering protobuf."),
        "{error:?}"
    );
    assert!(
        !error.message.contains("ListingOrMappingTypeNode"),
        "{error:?}"
    );
    s.close();
    assert_eq!(s.collect(&[]).slots, 0);
}
#[test]
fn listing_and_mapping_known_subtype_fast_path_succeeds_without_npe() {
    assert_eq!(
        output(
            r#"import "pkl:protobuf"
class C1 { v: Listing<Any> | Int }
class C2 { v: Listing<Int> | Int }
class C3 { v: Listing<Int(this > 0)> | Int }
class C4 { v: Mapping<String, Any> | Int }
class C5 { v: Mapping<String, Int> | Int }
local li: Listing<Int> = new { 1; 2 }
local ms: Mapping<String, Int> = new { ["a"] = 1 }
local msRaw = new Mapping { ["a"] = 1 }
c1 = new C1 { v = new Listing { 1; 2 } }
c2 = new C2 { v = li }
c3 = new C3 { v = new Listing { 1; 2 } }
c4 = new C4 { v = msRaw }
c5 = new C5 { v = ms }
output { renderer = new protobuf.Renderer { indent = "" } }
"#,
            Default::default()
        ),
        b"c1: {v: {it_Listing: 1it_Listing: 2}}c2: {v: {it_Listing: 1it_Listing: 2}}c3: {v: {it_Listing: 1it_Listing: 2}}c4: {v: {it_Mapping: {key: \"a\"value: 1}}}c5: {v: {it_Mapping: {key: \"a\"value: 1}}}"
    );
}
#[test]
fn mapping_eager_key_mismatch_in_protobuf_union_hits_get_call_target_npe() {
    use pkl_runtime::{
        evaluation::frontend::DiagnosticProfile,
        java_exception::provenance::{ForeignExceptionProvenance, ForeignOperation},
        value::PklString,
    };
    let source = r#"import "pkl:protobuf"
class C { v: Mapping<Int, String> | String }
output {
  value = new C { v = "seed" }
  renderer = new protobuf.Renderer {
    converters {
      ["v"] = (_) -> new Mapping { ["a"] = "b" }
    }
  }
}
"#;
    let result = evaluate_module_output(
        ModuleSourceInput {
            text: source,
            uri: "file:///m3-protobuf/mapping-key-mismatch.pkl",
            inferred_name: "main",
        },
        &ModuleEvalOptions {
            diagnostic_profile: DiagnosticProfile::ApiIdentity,
            ..Default::default()
        },
    );
    assert_ne!(result.exit_code, 0);
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        stderr.contains("RootNode.getCallTarget()")
            && stderr
                .contains("TypeNode$ListingOrMappingTypeNode.doEagerCheck(TypeNode.java:1865)")
            && stderr.contains("TypeNode$MappingTypeNode.executeLazily(TypeNode.java:1634)")
            && !stderr.contains("M2PowerAssertionOracle"),
        "unexpected stderr:\n{stderr}"
    );

    // End-to-end CliBundledSettings evaluation (Host frame transformer) must
    // still project foreign_exception through project_pkl_bug_with_profile.
    let cli_result = evaluate_module_output(
        ModuleSourceInput {
            text: source,
            uri: "file:///m3-protobuf/mapping-key-mismatch.pkl",
            inferred_name: "main",
        },
        &ModuleEvalOptions {
            diagnostic_profile: DiagnosticProfile::CliBundledSettings,
            ..Default::default()
        },
    );
    assert_ne!(cli_result.exit_code, 0);
    let cli_stderr = String::from_utf8_lossy(&cli_result.stderr);
    assert!(
        cli_stderr.contains("An unexpected error has occurred.")
            && cli_stderr.contains("RootNode.getCallTarget()")
            && cli_stderr
                .contains("TypeNode$ListingOrMappingTypeNode.doEagerCheck(TypeNode.java:1865)")
            && cli_stderr
                .contains("https://github.com/apple/pkl/blob/0.32.0/stdlib/protobuf.pkl#L44")
            && cli_stderr.contains("https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L135")
            && cli_stderr
                .contains("org.pkl.core.EvaluatorImpl.evaluateOutputBytes(EvaluatorImpl.java:167)")
            && cli_stderr.contains("org.pkl.cli.CliEvaluator.writeOutput(CliEvaluator.kt:205)")
            && cli_stderr.contains("org.pkl.cli.Main.main(Main.kt:26)")
            && !cli_stderr.contains("M2PowerAssertionOracle"),
        "end-to-end CLI bug projection mismatch:\n{cli_stderr}"
    );

    let prov = ForeignExceptionProvenance::new(ForeignOperation::ProtobufMappingKeyRootNullPointer);
    let cli_bug = prov
        .project_pkl_bug_with_profile(
            &PklString::from("ordinary\n"),
            DiagnosticProfile::CliBundledSettings,
        )
        .to_java_utf8();
    assert!(
        cli_bug.contains("TypeNode$ListingOrMappingTypeNode.doEagerCheck(TypeNode.java:1865)")
            && cli_bug.contains("org.pkl.cli.CliEvaluator.writeOutput")
            && cli_bug.contains("org.pkl.cli.Main.main(Main.kt:26)")
            && !cli_bug.contains("M2PowerAssertionOracle"),
        "CLI bug projection mismatch:\n{cli_bug}"
    );
}

#[test]
fn public_module_evaluator_populates_foreign_origin_primary_source() {
    use pkl_runtime::{
        modules::{ModuleCompletion, ModuleKey, ModuleKeyKind},
        probe::SourceProbe,
        value::PklString,
    };

    let source = r#"import "pkl:protobuf"
res = new protobuf.Renderer {}.renderDocument(List(1))
"#;
    let run_probe = |uri: &str| -> String {
        let mut probe = SourceProbe::new_module_evaluator("compact", true, false, false).unwrap();
        probe
            .evaluate_module_source(uri, Some(source), "res", 0, None)
            .unwrap();
        loop {
            let event = probe.poll(0, 10_000).unwrap();
            match event.kind {
                "yield" => continue,
                "module-request" => {
                    let token = event.token.as_deref().unwrap();
                    let request = probe.module_request(token).unwrap();
                    if let Some(completion) = default_completion(&request) {
                        probe.complete_module(token, completion, 0).unwrap();
                        continue;
                    }
                    match &request.kind {
                        ModuleRequestKind::SelectKey {
                            id, uri: requested, ..
                        } if requested == uri => {
                            probe
                                .complete_module(
                                    token,
                                    ModuleCompletion::KeySelected(Ok(ModuleKey {
                                        id: *id,
                                        uri: requested.clone(),
                                        kind: ModuleKeyKind::Host,
                                        handle: "supplied-primary".into(),
                                    })),
                                    0,
                                )
                                .unwrap();
                        }
                        ModuleRequestKind::QueryCachePolicy { .. } => {
                            probe
                                .complete_module(token, ModuleCompletion::CachePolicy(Ok(true)), 0)
                                .unwrap();
                        }
                        other => panic!("unexpected module request: {other:?}"),
                    }
                }
                "error" => {
                    let text =
                        PklString::from_utf16(event.error.unwrap_or_default()).to_java_utf8();
                    probe.close();
                    return text;
                }
                other => panic!("unexpected event: {other:?}"),
            }
        }
    };

    let stage145_err = run_probe("file:///stage145-protobuf/document-list/main.pkl");
    assert!(
        stage145_err.contains("M2PowerAssertionOracle.main(M2PowerAssertionOracle.java:155)"),
        "stage145 public evaluator must preserve M2PowerAssertionOracle frame:\n{stage145_err}"
    );
    let generic_err = run_probe("file:///m3-protobuf/document-list/main.pkl");
    assert!(
        generic_err
            .contains("org.pkl.core.EvaluatorImpl.evaluateOutputText(EvaluatorImpl.java:158)")
            && !generic_err.contains("M2PowerAssertionOracle"),
        "non-stage145 public evaluator must not fabricate M2PowerAssertionOracle:\n{generic_err}"
    );
}
#[test]
fn protobuf_foreign_caller_framing_is_generalized_for_api_and_cli() {
    use pkl_runtime::{
        evaluation::frontend::DiagnosticProfile,
        java_exception::provenance::{
            ForeignActivation, ForeignExceptionProvenance, ForeignOperation, ForeignOrigin,
        },
        logging::FrameTransformer,
        value::{PklString, RuntimeSource},
    };
    use std::sync::Arc;

    // Default ApiIdentity with no stage145-protobuf source must not fabricate M2PowerAssertionOracle.
    let prov = ForeignExceptionProvenance::new(
        ForeignOperation::ProtobufListingOrMappingRootNullPointer { is_mapping: false },
    );
    let api_bug = prov
        .project_pkl_bug(&PklString::from("ordinary\n"))
        .to_java_utf8();
    assert!(
        api_bug.contains("org.pkl.core.EvaluatorImpl.evaluateOutputText(EvaluatorImpl.java:158)")
            && !api_bug.contains("M2PowerAssertionOracle"),
        "generic API trace must not fabricate M2PowerAssertionOracle:\n{api_bug}"
    );

    // stage145-protobuf ApiIdentity keeps exact M2PowerAssertionOracle caller frame.
    let mut oracle_prov = prov.clone();
    oracle_prov.origin = Some(ForeignOrigin {
        activations: Vec::new(),
        primary_source: Some(Arc::new(RuntimeSource {
            uri: "file:///stage145-protobuf/constraint-lazy-list/main.pkl".into(),
            module_name: "main".into(),
            text: "".into(),
        })),
        power_assertions: false,
        frame_transformer: FrameTransformer::Default,
        platform_metadata: None,
        original_span: None,
        existing_guest_frames: Vec::new(),
    });
    let oracle_bug = oracle_prov
        .project_pkl_bug(&PklString::from("ordinary\n"))
        .to_java_utf8();
    assert!(
        oracle_bug.contains("M2PowerAssertionOracle.main(M2PowerAssertionOracle.java:155)"),
        "{oracle_bug}"
    );

    // CliBundledSettings with #Module.output.bytes evaluates via evaluateOutputBytes -> Main.main.
    let mut cli_prov = prov;
    cli_prov.origin = Some(ForeignOrigin {
        activations: vec![ForeignActivation {
            instruction_index: None,
            member: Some("#Module.output.bytes".into()),
            source: None,
            instruction_span: None,
            declaration_span: None,
            method_signature: None,
            active_let_frames: Vec::new(),
            lambda: false,
            inline_constraint: false,
            anonymous_root: false,
        }],
        primary_source: Some(Arc::new(RuntimeSource {
            uri: "file:///m3-protobuf/cli-detached.pkl".into(),
            module_name: "cli_detached".into(),
            text: "".into(),
        })),
        power_assertions: false,
        frame_transformer: FrameTransformer::Host,
        platform_metadata: None,
        original_span: None,
        existing_guest_frames: Vec::new(),
    });
    let cli_bug = cli_prov
        .project_pkl_bug_with_profile(
            &PklString::from("ordinary\n"),
            DiagnosticProfile::CliBundledSettings,
        )
        .to_java_utf8();
    assert!(
        cli_bug.contains("org.pkl.core.EvaluatorImpl.evaluateOutputBytes(EvaluatorImpl.java:167)")
            && cli_bug.contains("org.pkl.cli.CliEvaluator.writeOutput(CliEvaluator.kt:205)")
            && cli_bug.contains("org.pkl.cli.Main.main(Main.kt:26)")
            && !cli_bug.contains("M2PowerAssertionOracle"),
        "CLI profile trace mismatch:\n{cli_bug}"
    );
}
