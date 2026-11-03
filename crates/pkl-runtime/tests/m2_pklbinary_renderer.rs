//! Source-derived development regressions. These assertions are not Java
//! differential evidence; the historical binary cases remain unpassed until
//! executed against the pinned oracle with complete raw channel comparisons.
use pkl_runtime::{
    evaluation::{
        frontend::{ModuleSourceInput, evaluate_module_output},
        session::{SourceSession, SourceSessionOptions},
    },
    host::HostPoll,
    modules::{ModuleRequestKind, default_completion},
    resources::ReadResult,
    value::{ErrorKind, PklString, Value},
};

const URI: &str = "file:///stage144-pklbinary/main.pkl";

fn output(source: &str) -> Vec<u8> {
    let result = evaluate_module_output(
        ModuleSourceInput {
            text: source,
            uri: URI,
            inferred_name: "main",
        },
        &Default::default(),
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

fn render(value: &str) -> Vec<u8> {
    output(&format!(
        "import \"pkl:pklbinary\"\noutput {{ renderer=new pklbinary.Renderer {{}}; value={value} }}\n"
    ))
}

#[test]
fn binary_output_preserves_nil_negative_zero_and_arbitrary_byte_payload() {
    assert_eq!(render("null"), [0xc0]);
    assert_eq!(render("-0.0"), [0xcb, 0x80, 0, 0, 0, 0, 0, 0, 0]);
    assert_eq!(
        render("Bytes(0,127,128,255)"),
        [0x92, 0x0f, 0xc4, 4, 0, 127, 128, 255]
    );
}

#[test]
fn value_and_document_methods_return_the_same_bytes_without_a_newline() {
    let document = render("List(1,null)");
    assert_eq!(document, [0x92, 0x04, 0x92, 0x01, 0xc0]);
    assert_eq!(
        output(
            r#"import "pkl:pklbinary"
output { bytes = new pklbinary.Renderer {}.renderValue(List(1,null)) }
"#
        ),
        document
    );
}

#[test]
fn strict_and_lazy_collection_tags_and_map_key_types_remain_distinct() {
    for (source, expected) in [
        ("List(1,2)", vec![0x92, 0x04, 0x92, 1, 2]),
        ("new Listing { 1; 2 }", vec![0x92, 0x05, 0x92, 1, 2]),
        ("Set(1,2)", vec![0x92, 0x06, 0x92, 1, 2]),
        ("Map(1,2)", vec![0x92, 0x02, 0x81, 1, 2]),
        ("new Mapping { [1]=2 }", vec![0x92, 0x03, 0x81, 1, 2]),
        ("new Mapping { [1]=null }", vec![0x92, 0x03, 0x81, 1, 0xc0]),
    ] {
        assert_eq!(render(source), expected, "{source}");
    }
}

#[test]
fn dynamic_property_entry_and_element_records_keep_declaration_order() {
    assert_eq!(
        render(r#"new Dynamic { x=1; ["k"]=2; 3 }"#),
        b"\x94\x01\xa7Dynamic\xa8pkl:base\x93\x93\x10\xa1x\x01\x93\x11\xa1k\x02\x93\x12\x00\x03"
    );
}

#[test]
fn inherited_entries_are_counted_once_and_local_properties_are_omitted() {
    assert_eq!(
        render(r#"(new Dynamic { x=1; local l=throw("local"); ["k"]=2 }) { x=3; ["k"]=4 }"#),
        b"\x94\x01\xa7Dynamic\xa8pkl:base\x92\x93\x10\xa1x\x03\x93\x11\xa1k\x04"
    );
    assert_eq!(
        render(r#"(new Mapping { ["k"]=1 }) { ["k"]=2; ["j"]=3 }"#),
        b"\x92\x03\x82\xa1k\x02\xa1j\x03"
    );
}

#[test]
fn typed_hidden_properties_are_omitted_without_forcing() {
    let actual = output(
        r#"import "pkl:pklbinary"
class Holder {
  x: Int=3
  hidden h: Int=throw("hidden")
}
output { renderer=new pklbinary.Renderer {}; value=new Holder {} }
"#,
    );
    let mut expected = b"\x94\x01\xabmain#Holder\xd9\x23".to_vec();
    expected.extend_from_slice(URI.as_bytes());
    expected.extend_from_slice(b"\x91\x93\x10\xa1x\x03");
    assert_eq!(actual, expected);
}

#[test]
fn pair_children_bypass_converters_but_nested_collection_members_do_not() {
    assert_eq!(
        output(
            r#"import "pkl:pklbinary"
output { renderer=new pklbinary.Renderer { converters { [Int]=(v)->v+10 } }; value=Pair(1,List(2)) }
"#
        ),
        [0x93, 0x09, 1, 0x92, 0x04, 0x91, 12]
    );
    assert_eq!(render("(x)->x"), [0x91, 0x0e]);
}

#[test]
fn render_directive_inserts_raw_bytes_after_map_key_construction() {
    assert_eq!(
        render(r#"new RenderDirective { bytes=Bytes(0,255,128); text=throw("unused") }"#),
        [0, 255, 128]
    );
    assert_eq!(
        render(
            r#"Map(new RenderDirective { bytes=Bytes(1); text="key" },new RenderDirective { bytes=Bytes(2); text=throw("unused") })"#
        ),
        [0x92, 0x02, 0x81, 1, 2]
    );
}

#[test]
fn class_and_alias_values_keep_declaring_source_uri() {
    assert_eq!(render("Int"), b"\x93\x0c\xa3Int\xa8pkl:base");
    assert_eq!(render("UInt8"), b"\x93\x0d\xa5UInt8\xa8pkl:base");
    let class = output(
        r#"import "pkl:pklbinary"
class Box {}
output { renderer=new pklbinary.Renderer {}; value=Box }
"#,
    );
    let mut expected = b"\x93\x0c\xa8main#Box\xd9\x23".to_vec();
    expected.extend_from_slice(URI.as_bytes());
    assert_eq!(class, expected);
}

#[test]
fn property_annotations_are_disabled_by_the_authored_binary_default() {
    assert_eq!(
        output(
            r#"import "pkl:pklbinary"
class Holder {
  @ConvertProperty { render=(property,renderer)->throw("disabled") }
  x: Int=7
}

output { renderer=new pklbinary.Renderer {}; value=new Holder {} }
"#
        ),
        {
            let mut expected = b"\x94\x01\xabmain#Holder\xd9\x23".to_vec();
            expected.extend_from_slice(URI.as_bytes());
            expected.extend_from_slice(b"\x91\x93\x10\xa1x\x07");
            expected
        }
    );
}

#[test]
fn references_encode_domain_data_and_path_without_rendering_or_forcing_the_referent() {
    let actual = output(
        r#"import "pkl:pklbinary"
import "pkl:ref"
class D extends ref.Domain {
 function renderReference(r): String = throw("not a text renderer")
}
class Holder { value: Int=throw("referent body") }
output {
 renderer=new pklbinary.Renderer { converters { [Int]=(v)->v+10 } }
 value=ref.Reference(new D {}, Holder, 2)
}
"#,
    );
    let mut expected = b"\x94\x20\x94\x01\xa6main#D\xd9\x23".to_vec();
    expected.extend_from_slice(URI.as_bytes());
    expected.extend_from_slice(b"\x90\x02\x90");
    assert_eq!(actual, expected);
}

fn session(source: &str) -> SourceSession {
    SourceSession::compile_output_text(
        ModuleSourceInput {
            text: source,
            uri: URI,
            inferred_name: "main",
        },
        &[],
        &Default::default(),
        SourceSessionOptions::default(),
    )
    .unwrap()
}

fn module(s: &mut SourceSession, token: &str, request: &pkl_runtime::modules::ModuleRequest) {
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
fn key_value_conversion_and_directive_forcing_order_survives_gc_at_suspension() {
    let mut s = session(
        r#"import "pkl:pklbinary"
local binaryRenderer=new pklbinary.Renderer {
 converters {
  [String]=(v)->new RenderDirective {
   bytes=if (read("render:key") == "go") Bytes(1) else throw("key")
   text=throw("unused")
  }
  [Int]=(v)->if (read("render:value") == "go") v else throw("value")
 }
}
output { renderer=binaryRenderer; value=new Mapping { ["k"]=if (read("render:force") == "go") 2 else throw("force") } }
"#,
    );
    s.start_output_bytes(0, None).unwrap();
    let mut reads = Vec::new();
    let mut result = None;
    for _ in 0..100_000 {
        match s.poll(0, 16) {
            HostPoll::Yielded => {
                s.collect(&[]);
            }
            HostPoll::Module { token, request } => module(&mut s, &token, &request),
            HostPoll::Read { token, uri } => {
                reads.push(uri);
                s.collect(&[]);
                s.complete_read(&token, Ok(ReadResult::Text("go".into())), 0)
                    .unwrap();
                s.collect(&[]);
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
        ["render:force", "render:value", "render:key"].map(PklString::from)
    );
    let Value::Bytes(bytes) = result.expect("binary render completed") else {
        panic!("binary result")
    };
    assert_eq!(bytes.as_ref(), [0x92, 0x03, 0x81, 1, 2]);
    s.close();
    assert_eq!(s.collect(&[]).slots, 0);
    assert_eq!(bytes.as_ref(), [0x92, 0x03, 0x81, 1, 2]);
}

#[test]
fn pending_directive_cancellation_releases_the_continuation_and_rejects_stale_completion() {
    let mut s = session(
        r#"import "pkl:pklbinary"
output { renderer=new pklbinary.Renderer {}; value=new RenderDirective {
 bytes=if (read("render:pending") == "go") Bytes(1) else Bytes(2)
 text=throw("unused")
} }
"#,
    );
    s.start_output_bytes(0, None).unwrap();
    let mut pending = None;
    for _ in 0..100_000 {
        match s.poll(0, 64) {
            HostPoll::Yielded => (),
            HostPoll::Module { token, request } => module(&mut s, &token, &request),
            HostPoll::Read { token, uri } => {
                assert_eq!(uri, "render:pending");
                s.collect(&[]);
                pending = Some(token);
                break;
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }
    let token = pending.expect("directive suspended");
    s.cancel();
    let HostPoll::Ready(Err(error)) = s.poll(0, 64) else {
        panic!("cancelled result")
    };
    assert_eq!(error.kind, ErrorKind::Cancelled);
    assert!(
        s.complete_read(&token, Ok(ReadResult::Text("late".into())), 0)
            .is_err()
    );
    s.close();
    assert_eq!(s.collect(&[]).slots, 0);
}

// Exercise the same late source registration used by the original-module
// evaluator. The earlier `output` helper prepares source-aware bytecode first.
fn late_registered_binary(source: &str, name: &str, imported: Option<&str>) -> Vec<u8> {
    use pkl_runtime::{
        modules::{
            ModuleCompletion, ModuleIdentity, ModuleKey, ModuleKeyKind, ModuleRequestKind,
            ModuleSource, ResolvedModule,
        },
        parser::parse_module,
        resolver::resolve_module,
        value::RuntimeSource,
        vm::{Vm, VmPoll, compile_module},
    };
    use std::sync::Arc;

    const IMPORTED_URI: &str = "file:///stage144-pklbinary/other.pkl";
    fn finish(vm: &mut Vm, source: &str, imported: Option<&str>) -> Value {
        for _ in 0..100_000 {
            match vm.poll(32) {
                VmPoll::Ready(result) => return result.unwrap(),
                VmPoll::Yielded => (),
                VmPoll::Module(request) => {
                    let completion =
                        default_completion(&request).unwrap_or_else(|| match &request.kind {
                            ModuleRequestKind::SelectKey { id, uri, .. } => {
                                assert!(uri == URI || uri == IMPORTED_URI);
                                ModuleCompletion::KeySelected(Ok(ModuleKey {
                                    id: *id,
                                    uri: uri.clone(),
                                    kind: ModuleKeyKind::Host,
                                    handle: "binary-class-fixture".into(),
                                }))
                            }
                            ModuleRequestKind::QueryCachePolicy { .. } => {
                                ModuleCompletion::CachePolicy(Ok(true))
                            }
                            ModuleRequestKind::ResolveKey { key, .. }
                            | ModuleRequestKind::Resolve { key, .. } => {
                                assert!(key.uri == URI || key.uri == IMPORTED_URI);
                                ModuleCompletion::Resolved(Ok(ResolvedModule {
                                    original_uri: key.uri.clone(),
                                    resolved_uri: "file:///physical/class-copy.pkl".into(),
                                    source_uri: "file:///diagnostics/class-source.pkl".into(),
                                }))
                            }
                            ModuleRequestKind::LoadSource { resolved, .. } => {
                                let (text, inferred_name) = if resolved.original_uri == URI {
                                    (source, "main")
                                } else {
                                    assert_eq!(resolved.original_uri, IMPORTED_URI);
                                    (imported.expect("declared fixture source"), "other")
                                };
                                ModuleCompletion::Source(Ok(ModuleSource {
                                    text: text.into(),
                                    inferred_name: inferred_name.into(),
                                }))
                            }
                            event => panic!("unexpected module request: {event:?}"),
                        });
                    vm.complete_module(request.id, completion).unwrap();
                }
                event => panic!("unexpected event: {event:?}"),
            }
        }
        panic!("late-registered binary poll bound");
    }

    let parsed = parse_module(source).unwrap();
    let mut program = compile_module(&resolve_module(&parsed).unwrap()).unwrap();
    Arc::make_mut(&mut program)
        .register_primary_source(
            Arc::new(RuntimeSource {
                text: source.into(),
                uri: URI.into(),
                module_name: name.into(),
            }),
            ModuleIdentity {
                original_uri: URI.into(),
                resolved_uri: "file:///physical/primary-copy.pkl".into(),
            },
        )
        .unwrap();
    let mut vm = Vm::new(program.clone());
    vm.start(program.entry, Vec::new()).unwrap();
    let module = finish(&mut vm, source, imported).as_object().unwrap();
    vm.start_member(module, "result").unwrap();
    let Value::Bytes(bytes) = finish(&mut vm, source, imported) else {
        panic!("binary renderer must return Bytes")
    };
    let result = bytes.into_data().to_vec();
    vm.close();
    result
}

#[test]
fn module_root_count_uses_regular_declarations_and_inherited_hidden_names() {
    let actual = late_registered_binary(
        r#"import "pkl:pklbinary"
x = 7
local ignored = throw("local")
hidden secret = throw("hidden")
class Unused {}
typealias Alias = Int
output { renderer = new pklbinary.Renderer {} }
hidden result = new pklbinary.Renderer {}.renderDocument(module)
"#,
        "main",
        None,
    );
    let mut expected = b"\x94\x01\xa4main\xd9\x23".to_vec();
    expected.extend_from_slice(URI.as_bytes());
    expected.extend_from_slice(b"\x91\x93\x10\xa1x\x07");
    assert_eq!(actual, expected);
}

#[test]
fn explicit_primary_classes_and_instances_keep_the_late_registered_module_uri() {
    let actual = late_registered_binary(
        r#"module com.foo.bar.MyModule
import "pkl:pklbinary"
class Person { name: String = "bird" }
hidden result = new pklbinary.Renderer {}.renderValue(List(Person, new Person {}))
"#,
        "com.foo.bar.MyModule",
        None,
    );
    let mut expected = b"\x92\x04\x92\x93\x0c\xbbcom.foo.bar.MyModule#Person\xd9\x23".to_vec();
    expected.extend_from_slice(URI.as_bytes());
    expected.extend_from_slice(b"\x94\x01\xbbcom.foo.bar.MyModule#Person\xd9\x23");
    expected.extend_from_slice(URI.as_bytes());
    expected.extend_from_slice(b"\x91\x93\x10\xa4name\xa4bird");
    assert_eq!(actual, expected);
}

#[test]
fn imported_class_identity_keeps_its_owner_when_the_caller_declares_the_same_name() {
    let actual = late_registered_binary(
        r#"module caller.Name
import "pkl:pklbinary"
import "file:///stage144-pklbinary/other.pkl"
class Person { name: String = "caller" }
hidden result = new pklbinary.Renderer {}.renderValue(List(other.Person, new other.Person {}))
"#,
        "caller.Name",
        Some("module imported.Name\nclass Person { name: String = \"bird\" }\n"),
    );
    const IMPORTED_URI: &[u8] = b"file:///stage144-pklbinary/other.pkl";
    let mut expected = b"\x92\x04\x92\x93\x0c\xb4imported.Name#Person\xd9\x24".to_vec();
    expected.extend_from_slice(IMPORTED_URI);
    expected.extend_from_slice(b"\x94\x01\xb4imported.Name#Person\xd9\x24");
    expected.extend_from_slice(IMPORTED_URI);
    expected.extend_from_slice(b"\x91\x93\x10\xa4name\xa4bird");
    assert_eq!(actual, expected);
}

#[test]
fn locally_bound_class_instances_in_collections_keep_the_primary_declaration_owner() {
    let actual = late_registered_binary(
        r#"import "pkl:pklbinary"
class Test {}
hidden result = new pklbinary.Renderer {}.renderValue(
    new Listing { local value = new Test {}; value }
)
"#,
        "main",
        None,
    );
    let mut expected = b"\x92\x05\x91\x94\x01\xa9main#Test\xd9\x23".to_vec();
    expected.extend_from_slice(URI.as_bytes());
    expected.push(0x90);
    assert_eq!(actual, expected);
}
