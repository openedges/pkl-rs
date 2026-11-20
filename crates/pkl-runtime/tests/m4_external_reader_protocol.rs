//! Wasm-pure unit and `HostSession` integration tests for the Pkl 0.32.0
//! external module & resource reader protocol (`0x26`–`0x32`).
//!
//! Zero OS subprocesses, zero filesystem I/O, zero `unsafe`.

use pkl_runtime::evaluation::frontend::{
    ModuleEvalOptions, ModuleSourceInput, evaluate_module_output_with_glob_host,
};
use pkl_runtime::external_reader::{
    DirectTransport, ExternalReaderConfig, ExternalReaderError, ExternalReaderMessage,
    ExternalReaderProcessHandle, ExternalReaderRegistry, ExternalReaderTransport, MessageType,
    MockExternalModuleReader, MockExternalReaderPeer, MockExternalResourceReader, ModuleReaderSpec,
    RawFrameTransport, ResourceReaderSpec, WirePathElement, codec, shlex,
};
use pkl_runtime::glob::{GlobCompletion, GlobHostError, configured_or_unsupported_completion};
use pkl_runtime::host::ReadFailure;
use pkl_runtime::modules::{ModuleCompletion, ModuleHostError, default_or_unsupported_completion};
use pkl_runtime::value::PklString;
use std::collections::BTreeMap;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

// ============================================================================
// 1. Wire Protocol Codec Round-Trip Tests (`0x26`–`0x32`)
// ============================================================================

#[test]
fn roundtrip_all_13_external_reader_messages() {
    let messages = vec![
        ExternalReaderMessage::ReadResourceRequest {
            request_id: 123,
            evaluator_id: 456,
            uri: "some/resource.json".into(),
        },
        ExternalReaderMessage::ReadResourceResponse {
            request_id: 123,
            evaluator_id: 456,
            contents: Some(vec![0, 1, 2, 127, 128, 255]),
            error: None,
        },
        ExternalReaderMessage::ReadResourceResponse {
            request_id: -42,
            evaluator_id: i64::MAX,
            contents: None,
            error: None,
        },
        ExternalReaderMessage::ReadResourceResponse {
            request_id: i64::MIN,
            evaluator_id: 0,
            contents: Some(Vec::new()),
            error: Some("read failed".into()),
        },
        ExternalReaderMessage::ReadModuleRequest {
            request_id: 123,
            evaluator_id: 456,
            uri: "some/module.pkl".into(),
        },
        ExternalReaderMessage::ReadModuleResponse {
            request_id: 123,
            evaluator_id: 456,
            contents: Some("x = 42\n".into()),
            error: None,
        },
        ExternalReaderMessage::ReadModuleResponse {
            request_id: 124,
            evaluator_id: 0,
            contents: None,
            error: None,
        },
        ExternalReaderMessage::ReadModuleResponse {
            request_id: 125,
            evaluator_id: 0,
            contents: None,
            error: Some("cannot read module".into()),
        },
        ExternalReaderMessage::ListResourcesRequest {
            request_id: 987,
            evaluator_id: 1359,
            uri: "bar:/bazzy".into(),
        },
        ExternalReaderMessage::ListResourcesResponse {
            request_id: 3851,
            evaluator_id: 3019,
            path_elements: Some(vec![
                WirePathElement::directory("foo"),
                WirePathElement::file("bar"),
            ]),
            error: None,
        },
        ExternalReaderMessage::ListResourcesResponse {
            request_id: 3851,
            evaluator_id: 3019,
            path_elements: None,
            error: Some("something went wrong".into()),
        },
        ExternalReaderMessage::ListModulesRequest {
            request_id: 135,
            evaluator_id: 246,
            uri: "foo:/bar/baz/biz".into(),
        },
        ExternalReaderMessage::ListModulesResponse {
            request_id: 123,
            evaluator_id: 234,
            path_elements: Some(vec![
                WirePathElement::directory("foo"),
                WirePathElement::file("bar"),
            ]),
            error: None,
        },
        ExternalReaderMessage::ListModulesResponse {
            request_id: 123,
            evaluator_id: 234,
            path_elements: None,
            error: Some("Something dun went wrong".into()),
        },
        ExternalReaderMessage::InitializeModuleReaderRequest {
            request_id: 123,
            scheme: "my-scheme".into(),
        },
        ExternalReaderMessage::InitializeModuleReaderResponse {
            request_id: 123,
            spec: None,
        },
        ExternalReaderMessage::InitializeModuleReaderResponse {
            request_id: 123,
            spec: Some(ModuleReaderSpec {
                scheme: "my-scheme".into(),
                has_hierarchical_uris: true,
                is_local: true,
                is_globbable: true,
            }),
        },
        ExternalReaderMessage::InitializeResourceReaderRequest {
            request_id: 123,
            scheme: "my-scheme".into(),
        },
        ExternalReaderMessage::InitializeResourceReaderResponse {
            request_id: 123,
            spec: None,
        },
        ExternalReaderMessage::InitializeResourceReaderResponse {
            request_id: 123,
            spec: Some(ResourceReaderSpec {
                scheme: "my-scheme".into(),
                has_hierarchical_uris: true,
                is_globbable: false,
            }),
        },
        ExternalReaderMessage::CloseExternalProcess,
    ];

    let mut stream = Vec::new();
    for msg in &messages {
        let encoded = codec::encode_to_vec(msg).expect("encode");
        let decoded = codec::decode_from_slice(&encoded)
            .expect("decode")
            .expect("present");
        assert_eq!(&decoded, msg);
        stream.extend_from_slice(&encoded);
    }

    let decoded_all = codec::decode_all_from_slice(&stream).expect("decode concatenated stream");
    assert_eq!(decoded_all, messages);
}

#[test]
fn wire_codes_and_key_omission_match_pinned_java_exact_bytes() {
    // CloseExternalProcess: [0x32, {}] -> 0x92, 0x32, 0x80
    assert_eq!(
        codec::encode_to_vec(&ExternalReaderMessage::CloseExternalProcess).unwrap(),
        vec![0x92, 0x32, 0x80]
    );

    // InitializeModuleReaderResponse(123, null): [0x2f, {"requestId": 123}]
    // Must omit "spec" key (map header = 0x81, NOT 0x82 with nil).
    let bytes = codec::encode_to_vec(&ExternalReaderMessage::InitializeModuleReaderResponse {
        request_id: 123,
        spec: None,
    })
    .unwrap();
    assert_eq!(bytes[0], 0x92);
    assert_eq!(bytes[1], 0x2f);
    assert_eq!(bytes[2], 0x81); // 1-entry map

    // Clean EOF on empty buffer returns Ok(None)
    assert_eq!(codec::decode_from_slice(&[]).unwrap(), None);

    // MessageType codes match Message.java:22-41
    assert_eq!(MessageType::ReadResourceRequest.code(), 0x26);
    assert_eq!(MessageType::ReadResourceResponse.code(), 0x27);
    assert_eq!(MessageType::ReadModuleRequest.code(), 0x28);
    assert_eq!(MessageType::ReadModuleResponse.code(), 0x29);
    assert_eq!(MessageType::ListResourcesRequest.code(), 0x2a);
    assert_eq!(MessageType::ListResourcesResponse.code(), 0x2b);
    assert_eq!(MessageType::ListModulesRequest.code(), 0x2c);
    assert_eq!(MessageType::ListModulesResponse.code(), 0x2d);
    assert_eq!(MessageType::InitializeModuleReaderRequest.code(), 0x2e);
    assert_eq!(MessageType::InitializeModuleReaderResponse.code(), 0x2f);
    assert_eq!(MessageType::InitializeResourceReaderRequest.code(), 0x30);
    assert_eq!(MessageType::InitializeResourceReaderResponse.code(), 0x31);
    assert_eq!(MessageType::CloseExternalProcess.code(), 0x32);
}

#[test]
fn decode_errors_match_pinned_java_error_messages_properties() {
    // 1. Invalid header type (BaseMessagePackCodecTest.kt:120-127): integer 0x02 instead of Array
    let err = codec::decode_from_slice(&[0x02]).unwrap_err();
    assert_eq!(
        err,
        ExternalReaderError::Decode {
            message: "Malformed message header.".into(),
            cause: Some("Expected Array, but got Integer (02)".into()),
        }
    );

    // 2. Wrong header array length: [0x32] (size 1 instead of 2)
    let err = codec::decode_from_slice(&[0x91, 0x32]).unwrap_err();
    assert_eq!(
        err,
        ExternalReaderError::Decode {
            message: "Malformed message header (expected size 2, but got 1).".into(),
            cause: None,
        }
    );

    // 3. Unrecognized code (lowercase hex): [0x7f, {}]
    let err = codec::decode_from_slice(&[0x92, 0x7f, 0x80]).unwrap_err();
    assert_eq!(
        err,
        ExternalReaderError::Decode {
            message: "Malformed message header (unrecognized code `7f`).".into(),
            cause: None,
        }
    );

    // 4. Recognized server-only code (0x20 CREATE_EVALUATOR_REQUEST) -> Unhandled decoding code
    let err = codec::decode_from_slice(&[0x92, 0x20, 0x80]).unwrap_err();
    assert_eq!(
        err,
        ExternalReaderError::Decode {
            message: "Unhandled decoding message code `20`.".into(),
            cause: None,
        }
    );

    // 5. Missing required parameter (BaseMessagePackCodecTest.kt:102-117):
    // [0x2a, {"uri": "file:/test"}] -> Missing message parameter `requestId`
    let mut missing_req_id = vec![0x92, 0x2a, 0x81, 0xa3];
    missing_req_id.extend_from_slice(b"uri");
    missing_req_id.push(0xaa);
    missing_req_id.extend_from_slice(b"file:/test");
    let err = codec::decode_from_slice(&missing_req_id).unwrap_err();
    assert_eq!(
        err,
        ExternalReaderError::Decode {
            message: "Missing message parameter `requestId`".into(),
            cause: None,
        }
    );

    // 6. Explicit MessagePack `nil` (0xc0) on optional field `spec` -> Malformed message body
    // [0x2f, {"requestId": 1, "spec": nil}] -> decimal code 47
    let mut explicit_nil = vec![0x92, 0x2f, 0x82, 0xa9];
    explicit_nil.extend_from_slice(b"requestId");
    explicit_nil.push(0x01);
    explicit_nil.push(0xa4);
    explicit_nil.extend_from_slice(b"spec");
    explicit_nil.push(0xc0);
    let err = codec::decode_from_slice(&explicit_nil).unwrap_err();
    assert!(matches!(
        err,
        ExternalReaderError::Decode { ref message, .. }
            if message == "Malformed message body for message with code `47`."
    ));

    // 7. Invalid URI syntax inside ReadModuleRequest -> Malformed message body (code 40)
    let mut bad_uri = vec![0x92, 0x28, 0x83, 0xa9];
    bad_uri.extend_from_slice(b"requestId");
    bad_uri.push(0x01);
    bad_uri.push(0xab);
    bad_uri.extend_from_slice(b"evaluatorId");
    bad_uri.push(0x00);
    bad_uri.push(0xa3);
    bad_uri.extend_from_slice(b"uri");
    bad_uri.push(0xa7);
    bad_uri.extend_from_slice(b"bad uri");
    let err = codec::decode_from_slice(&bad_uri).unwrap_err();
    assert!(matches!(
        err,
        ExternalReaderError::Decode { ref message, .. }
            if message == "Malformed message body for message with code `40`."
    ));
}

// ============================================================================
// 2. `shlex` Parity Tests (all 14 cases from upstream `ShlexTest.kt`)
// ============================================================================

#[test]
fn shlex_matches_all_14_upstream_shlex_test_cases() {
    assert_eq!(shlex(""), Vec::<String>::new());
    assert_eq!(shlex("  \n \t  "), Vec::<String>::new());
    assert_eq!(shlex("\nabc  def\tghi "), vec!["abc", "def", "ghi"]);
    assert_eq!(
        shlex("'this is a single token'"),
        vec!["this is a single token"]
    );
    assert_eq!(
        shlex("\"this is a single token\""),
        vec!["this is a single token"]
    );
    assert_eq!(
        shlex("\"\\\"this is a single double quoted token\\\"\""),
        vec!["\"this is a single double quoted token\""]
    );
    assert_eq!(
        shlex("'this is a single \\\" token'"),
        vec!["this is a single \\\" token"]
    );
    assert_eq!(
        shlex("\"single\"' joined '\"token\""),
        vec!["single joined token"]
    );
    assert_eq!(shlex("\"single\"' '\"token\""), vec!["single token"]);
    assert_eq!(shlex("single\\ token"), vec!["single token"]);
    assert_eq!(shlex("\"\""), vec![""]);
    assert_eq!(shlex("''"), vec![""]);
    assert_eq!(shlex("'' ''"), vec!["", ""]);
    assert_eq!(shlex("''''"), vec![""]);

    // CLI option parsing (`BaseOptions.kt:86-97`)
    let (scheme, cfg) =
        ExternalReaderConfig::parse_cli_option("scheme+ext=reader5 'with arg' --flag").unwrap();
    assert_eq!(scheme, "scheme+ext");
    assert_eq!(cfg.executable, "reader5");
    assert_eq!(cfg.arguments, vec!["with arg", "--flag"]);
    assert_eq!(cfg.working_dir, None);
    assert!(!cfg.executable_has_path_separator(false));
    assert!(
        ExternalReaderConfig::new("./bin/reader", vec![], None)
            .executable_has_path_separator(false)
    );
    assert!(
        ExternalReaderConfig::new(r"bin\reader.exe", vec![], None)
            .executable_has_path_separator(true)
    );
}

// ============================================================================
// 3. Mock Peer + `HostSession` End-to-End Tests (100% Wasm-Pure)
// ============================================================================

struct MapModuleReader {
    spec: ModuleReaderSpec,
    files: BTreeMap<String, Option<String>>,
    dirs: BTreeMap<String, Vec<WirePathElement>>,
    errors: BTreeMap<String, String>,
    read_calls: Arc<AtomicUsize>,
    list_calls: Arc<AtomicUsize>,
}

impl MockExternalModuleReader for MapModuleReader {
    fn spec(&self) -> ModuleReaderSpec {
        self.spec.clone()
    }

    fn read(&self, uri: &str) -> Result<Option<String>, String> {
        self.read_calls.fetch_add(1, Ordering::SeqCst);
        if let Some(err) = self.errors.get(uri) {
            return Err(err.clone());
        }
        match self.files.get(uri) {
            Some(content) => Ok(content.clone()),
            None => Err(format!("not found: {uri}")),
        }
    }

    fn list_elements(&self, base_uri: &str) -> Result<Option<Vec<WirePathElement>>, String> {
        self.list_calls.fetch_add(1, Ordering::SeqCst);
        if let Some(err) = self.errors.get(base_uri) {
            return Err(err.clone());
        }
        Ok(Some(self.dirs.get(base_uri).cloned().unwrap_or_default()))
    }
}

struct MapResourceReader {
    spec: ResourceReaderSpec,
    files: BTreeMap<String, Option<Vec<u8>>>,
    dirs: BTreeMap<String, Vec<WirePathElement>>,
    errors: BTreeMap<String, String>,
    read_calls: Arc<AtomicUsize>,
}

impl MockExternalResourceReader for MapResourceReader {
    fn spec(&self) -> ResourceReaderSpec {
        self.spec.clone()
    }

    fn read(&self, uri: &str) -> Result<Option<Vec<u8>>, String> {
        self.read_calls.fetch_add(1, Ordering::SeqCst);
        if let Some(err) = self.errors.get(uri) {
            return Err(err.clone());
        }
        Ok(self.files.get(uri).cloned().flatten())
    }

    fn list_elements(&self, base_uri: &str) -> Result<Option<Vec<WirePathElement>>, String> {
        if let Some(err) = self.errors.get(base_uri) {
            return Err(err.clone());
        }
        Ok(Some(self.dirs.get(base_uri).cloned().unwrap_or_default()))
    }
}

fn eval_with_registry(
    source: &str,
    uri: &str,
    registry: &ExternalReaderRegistry,
    allowed_schemes: &[&str],
) -> Result<String, String> {
    let allowed: Vec<String> = allowed_schemes.iter().map(|s| format!("{s}:")).collect();
    let check_mod = |target: &PklString| -> Result<(), ModuleHostError> {
        let s = target.as_str().unwrap_or("");
        if allowed.iter().any(|p| s.starts_with(p))
            || s.starts_with("file:")
            || s.starts_with("pkl:")
        {
            Ok(())
        } else {
            Err(ModuleHostError::SecurityManagerException {
                message: Some(
                    format!(
                        "Refusing to load module `{s}` because it does not match any entry in the module allowlist (`--allowed-modules`)."
                    )
                    .into(),
                ),
            })
        }
    };
    let check_res_mod = |target: &PklString| -> Result<(), ModuleHostError> {
        let s = target.as_str().unwrap_or("");
        if allowed.iter().any(|p| s.starts_with(p)) || s.starts_with("file:") {
            Ok(())
        } else {
            Err(ModuleHostError::SecurityManagerException {
                message: Some(
                    format!(
                        "Refusing to read resource `{s}` because it does not match any entry in the resource allowlist (`--allowed-resources`)."
                    )
                    .into(),
                ),
            })
        }
    };
    let check_res = |target: &PklString| -> Result<(), ReadFailure> {
        let s = target.as_str().unwrap_or("");
        if allowed.iter().any(|p| s.starts_with(p)) || s.starts_with("file:") {
            Ok(())
        } else {
            Err(ReadFailure::SecurityManagerException {
                message: Some(
                    format!(
                        "Refusing to read resource `{s}` because it does not match any entry in the resource allowlist (`--allowed-resources`)."
                    )
                    .into(),
                ),
            })
        }
    };

    let options = ModuleEvalOptions::default();
    let input = ModuleSourceInput {
        text: source,
        uri,
        inferred_name: "main",
    };

    let outcome = evaluate_module_output_with_glob_host(
        input,
        &options,
        |req| -> ModuleCompletion {
            if let Some(comp) = registry.try_complete_module(req, check_mod) {
                return comp;
            }
            if let Some(comp) = pkl_runtime::modules::default_completion(req) {
                return comp;
            }
            if let Some(comp) = pkl_runtime::modules::standard_key_completion(req) {
                return comp;
            }
            default_or_unsupported_completion(req)
        },
        |req| {
            let resolved = req.resolved_uri().ok()?;
            registry.try_complete_read(&resolved, check_res)
        },
        |req| -> Option<GlobCompletion> {
            if let Some(comp) = registry.try_complete_glob(req, check_mod, check_res_mod) {
                return Some(comp);
            }
            let fallback = configured_or_unsupported_completion(req, &[], &[]);
            if matches!(
                fallback,
                GlobCompletion::Resource(Err(GlobHostError::ReaderNotFound))
            ) {
                return Some(fallback);
            }
            Some(fallback)
        },
    );

    if outcome.exit_code == 0 {
        Ok(String::from_utf8(outcome.stdout).expect("utf-8 pcf"))
    } else {
        Err(String::from_utf8(outcome.stderr).expect("utf-8 stderr"))
    }
}

#[test]
fn mock_external_module_and_resource_import_read_glob_caching_and_dedup() {
    let mod_reads = Arc::new(AtomicUsize::new(0));
    let mod_lists = Arc::new(AtomicUsize::new(0));
    let res_reads = Arc::new(AtomicUsize::new(0));

    let mod_reader = MapModuleReader {
        spec: ModuleReaderSpec {
            scheme: "ext".into(),
            has_hierarchical_uris: true,
            is_local: true,
            is_globbable: true,
        },
        files: BTreeMap::from([
            ("ext:/a.pkl".into(), Some("val = \"a\"\n".into())),
            ("ext:/b.pkl".into(), Some("val = \"b\"\n".into())),
            ("ext:/empty.pkl".into(), None), // None contents -> "" empty module
            (
                "ext:/dir/c.pkl".into(),
                Some("import \".../parent.pkl\"\nval = parent.p\n".into()),
            ),
            (
                "ext:/parent.pkl".into(),
                Some("p = \"fromParent\"\n".into()),
            ),
        ]),
        dirs: BTreeMap::from([
            (
                "ext:/".into(),
                vec![
                    WirePathElement::directory("dir"),
                    WirePathElement::file("b.pkl"),
                    WirePathElement::file("a.pkl"),
                ],
            ),
            ("ext:/dir/".into(), vec![WirePathElement::file("c.pkl")]),
        ]),
        errors: BTreeMap::new(),
        read_calls: Arc::clone(&mod_reads),
        list_calls: Arc::clone(&mod_lists),
    };

    let res_reader = MapResourceReader {
        spec: ResourceReaderSpec {
            scheme: "ext".into(),
            has_hierarchical_uris: true,
            is_globbable: true,
        },
        files: BTreeMap::from([
            ("ext:/hello.txt".into(), Some(b"hello world".to_vec())),
            ("ext:/missing.txt".into(), None), // contents == None && error == None -> Missing!
            ("ext:/dir/item.txt".into(), Some(b"nested".to_vec())),
        ]),
        dirs: BTreeMap::from([
            (
                "ext:/".into(),
                vec![
                    WirePathElement::directory("dir"),
                    WirePathElement::file("hello.txt"),
                ],
            ),
            ("ext:/dir/".into(), vec![WirePathElement::file("item.txt")]),
        ]),
        errors: BTreeMap::new(),
        read_calls: Arc::clone(&res_reads),
    };

    let peer = Arc::new(MockExternalReaderPeer::new(
        vec![Box::new(mod_reader)],
        vec![Box::new(res_reader)],
    ));
    let transport = Arc::new(DirectTransport::new(Arc::clone(&peer)));
    let factory_calls = Arc::new(AtomicUsize::new(0));

    let shared_cfg = ExternalReaderConfig::new("shared-ext-reader", vec!["--flag".into()], None);
    let mut registry = ExternalReaderRegistry::new(0);
    registry.register_module_reader_with("ext", shared_cfg.clone(), |_| {
        factory_calls.fetch_add(1, Ordering::SeqCst);
        Arc::clone(&transport) as Arc<dyn ExternalReaderTransport>
    });
    registry.register_resource_reader_with("ext", shared_cfg, |_| {
        factory_calls.fetch_add(1, Ordering::SeqCst);
        Arc::clone(&transport) as Arc<dyn ExternalReaderTransport>
    });

    // Exact ExternalReaderConfig deduplication across module & resource readers.
    assert_eq!(registry.process_count(), 1);
    assert_eq!(factory_calls.load(Ordering::SeqCst), 1);

    let pkl = r#"
import "ext:/a.pkl" as a1
import "ext:/a.pkl" as a2
import "ext:/empty.pkl" as emptyMod
import "ext:/dir/c.pkl" as child
import* "ext:/**.pkl" as allMods

v1 = a1.val
v2 = a2.val
childVal = child.val
modKeys = allMods.keys.toList()
r1 = read("ext:/hello.txt").text
r2 = read("ext:/hello.txt").base64
rMissing = read?("ext:/missing.txt")
resKeys = read*("ext:/**.txt").keys.toList()
"#;

    let out = eval_with_registry(pkl, "file:///main.pkl", &registry, &["ext"])
        .expect("evaluation succeeded");
    assert!(out.contains("v1 = \"a\""), "{out}");
    assert!(out.contains("v2 = \"a\""), "{out}");
    assert!(out.contains("childVal = \"fromParent\""), "{out}");
    assert!(out.contains("r1 = \"hello world\""), "{out}");
    assert!(out.contains("r2 = \"aGVsbG8gd29ybGQ=\""), "{out}");
    assert!(out.contains("rMissing = null"), "{out}");
    // PathElement ordering: non-directories sorted before directories.
    assert!(
        out.contains("modKeys = List(\"ext:/a.pkl\", \"ext:/b.pkl\", \"ext:/dir/c.pkl\")"),
        "{out}"
    );
    assert!(
        out.contains("resKeys = List(\"ext:/hello.txt\", \"ext:/dir/item.txt\")"),
        "{out}"
    );

    // Caching: ext:/a.pkl was imported twice but read over wire only once;
    // InitializeModuleReaderRequest & InitializeResourceReaderRequest sent once each.
    let incoming = peer.recorded_incoming();
    let init_mod_count = incoming
        .iter()
        .filter(|m| {
            matches!(
                m,
                ExternalReaderMessage::InitializeModuleReaderRequest { .. }
            )
        })
        .count();
    let init_res_count = incoming
        .iter()
        .filter(|m| {
            matches!(
                m,
                ExternalReaderMessage::InitializeResourceReaderRequest { .. }
            )
        })
        .count();
    let read_a_count = incoming
        .iter()
        .filter(|m| {
            matches!(
                m,
                ExternalReaderMessage::ReadModuleRequest { uri, .. } if uri == "ext:/a.pkl"
            )
        })
        .count();
    assert_eq!(init_mod_count, 1);
    assert_eq!(init_res_count, 1);
    assert_eq!(read_a_count, 1);

    // Every request sent across the shared process handle, module resolver, and
    // resource resolver must carry a unique, monotonically increasing requestId.
    let request_ids: Vec<i64> = incoming.iter().filter_map(|m| m.request_id()).collect();
    let expected_ids: Vec<i64> = (1..=request_ids.len() as i64).collect();
    assert_eq!(
        request_ids, expected_ids,
        "shared process handle & resolvers must draw from one requestId counter"
    );

    // Idempotent close sends CloseExternalProcess exactly once.
    registry.close_all();
    registry.close_all();
    assert_eq!(peer.close_count(), 1);
}

#[test]
fn mock_opaque_glob_sends_dummy_and_non_local_rejects_triple_dot() {
    // 1. Opaque globbable resource reader
    let opaque_res = MapResourceReader {
        spec: ResourceReaderSpec {
            scheme: "kv".into(),
            has_hierarchical_uris: false,
            is_globbable: true,
        },
        files: BTreeMap::from([
            ("kv:alpha".into(), Some(b"1".to_vec())),
            ("kv:beta".into(), Some(b"2".to_vec())),
        ]),
        dirs: BTreeMap::from([(
            "kv:*".into(),
            vec![
                WirePathElement::file("beta"),
                WirePathElement::file("alpha"),
            ],
        )]),
        errors: BTreeMap::new(),
        read_calls: Arc::new(AtomicUsize::new(0)),
    };
    let peer = Arc::new(MockExternalReaderPeer::new(
        vec![],
        vec![Box::new(opaque_res)],
    ));
    let mut reg = ExternalReaderRegistry::new(0);
    reg.register_resource_reader_with(
        "kv",
        ExternalReaderConfig::new("kv-reader", vec![], None),
        |_| Arc::new(DirectTransport::new(Arc::clone(&peer))),
    );
    let out = eval_with_registry(
        r#"vals = read*("kv:*").toMap().mapValues((_, v) -> v.text)"#,
        "file:///main.pkl",
        &reg,
        &["kv"],
    )
    .expect("opaque glob");
    assert!(
        out.contains("vals = Map(\"kv:alpha\", \"1\", \"kv:beta\", \"2\")"),
        "{out}"
    );

    // 2. Non-local hierarchical module reader rejects triple-dot import
    let non_local_mod = MapModuleReader {
        spec: ModuleReaderSpec {
            scheme: "rem".into(),
            has_hierarchical_uris: true,
            is_local: false,
            is_globbable: false,
        },
        files: BTreeMap::from([(
            "rem:/a/b/child.pkl".into(),
            Some("import \".../parent.pkl\"\nx = parent\n".into()),
        )]),
        dirs: BTreeMap::new(),
        errors: BTreeMap::new(),
        read_calls: Arc::new(AtomicUsize::new(0)),
        list_calls: Arc::new(AtomicUsize::new(0)),
    };
    let peer2 = Arc::new(MockExternalReaderPeer::new(
        vec![Box::new(non_local_mod)],
        vec![],
    ));
    let mut reg2 = ExternalReaderRegistry::new(0);
    reg2.register_module_reader_with(
        "rem",
        ExternalReaderConfig::new("rem-reader", vec![], None),
        |_| Arc::new(DirectTransport::new(Arc::clone(&peer2))),
    );
    let err = eval_with_registry(
        r#"import "rem:/a/b/child.pkl"
v = child.x"#,
        "file:///main.pkl",
        &reg2,
        &["rem"],
    )
    .unwrap_err();
    assert!(
        err.contains("Cannot resolve a triple-dot import from module URI `rem:/a/b/child.pkl`."),
        "unexpected error: {err}"
    );

    // 3. Non-globbable module reader rejects import*
    let err = eval_with_registry(
        r#"m = import*("rem:/*.pkl")"#,
        "file:///main.pkl",
        &reg2,
        &["rem"],
    )
    .unwrap_err();
    assert!(
        err.contains(
            "Cannot expand glob pattern `rem:/*.pkl` because scheme `rem` is not globbable."
        ),
        "unexpected error: {err}"
    );
}

#[test]
fn unsupported_scheme_and_reader_error_formatting_match_pinned_java() {
    // Peer implements NO schemes -> returns spec: None
    let empty_peer = Arc::new(MockExternalReaderPeer::new(vec![], vec![]));
    let mut reg = ExternalReaderRegistry::new(0);
    let cfg = ExternalReaderConfig::new("empty-reader", vec![], None);
    reg.register_module_reader_with("ghost", cfg.clone(), |_| {
        Arc::new(DirectTransport::new(Arc::clone(&empty_peer)))
    });
    reg.register_resource_reader_with("ghost", cfg, |_| {
        Arc::new(DirectTransport::new(Arc::clone(&empty_peer)))
    });

    // Module unsupported scheme: wrapped with `externalReaderFailure` + ExternalReaderProcessException
    let mod_err = eval_with_registry(
        r#"import "ghost:/x.pkl""#,
        "file:///main.pkl",
        &reg,
        &["ghost"],
    )
    .unwrap_err();
    assert!(
        mod_err.contains(
            "Failed to communicate with external reader process.\nExternalReaderProcessException: External module reader does not support scheme `ghost`."
        ),
        "module err:\n{mod_err}"
    );

    // Resource read unsupported scheme: bare message (`ResourceManager.java:91-92`)
    let res_err = eval_with_registry(
        r#"x = read("ghost:/x.txt")"#,
        "file:///main.pkl",
        &reg,
        &["ghost"],
    )
    .unwrap_err();
    assert!(
        res_err.contains("External resource reader does not support scheme `ghost`."),
        "res err:\n{res_err}"
    );
    assert!(
        !res_err.contains("Failed to communicate with external reader process."),
        "ResourceManager.doRead must NOT wrap with externalReaderFailure:\n{res_err}"
    );

    // Resource read* unsupported scheme: wrapped with `externalReaderFailure` (`ReadGlobNode.java:109`)
    let glob_err = eval_with_registry(
        r#"x = read*("ghost:*")"#,
        "file:///main.pkl",
        &reg,
        &["ghost"],
    )
    .unwrap_err();
    assert!(
        glob_err.contains(
            "Failed to communicate with external reader process.\nExternalReaderProcessException: External resource reader does not support scheme `ghost`."
        ),
        "glob err:\n{glob_err}"
    );

    // Reader-returned `error` string -> IOException
    let err_res = MapResourceReader {
        spec: ResourceReaderSpec {
            scheme: "boom".into(),
            has_hierarchical_uris: true,
            is_globbable: false,
        },
        files: BTreeMap::new(),
        dirs: BTreeMap::new(),
        errors: BTreeMap::from([("boom:/fail.txt".into(), "boom backend denied".into())]),
        read_calls: Arc::new(AtomicUsize::new(0)),
    };
    let err_peer = Arc::new(MockExternalReaderPeer::new(vec![], vec![Box::new(err_res)]));
    let mut reg_err = ExternalReaderRegistry::new(0);
    reg_err.register_resource_reader_with(
        "boom",
        ExternalReaderConfig::new("boom-reader", vec![], None),
        |_| Arc::new(DirectTransport::new(Arc::clone(&err_peer))),
    );
    let io_err = eval_with_registry(
        r#"x = read("boom:/fail.txt")"#,
        "file:///main.pkl",
        &reg_err,
        &["boom"],
    )
    .unwrap_err();
    assert!(
        io_err.contains(
            "I/O error reading resource `boom:/fail.txt`.\nIOException: boom backend denied"
        ),
        "io err:\n{io_err}"
    );

    // Non-nullable read of missing resource (contents: None, error: None) -> Cannot find resource
    let miss_res = MapResourceReader {
        spec: ResourceReaderSpec {
            scheme: "miss".into(),
            has_hierarchical_uris: false,
            is_globbable: false,
        },
        files: BTreeMap::new(),
        dirs: BTreeMap::new(),
        errors: BTreeMap::new(),
        read_calls: Arc::new(AtomicUsize::new(0)),
    };
    let miss_peer = Arc::new(MockExternalReaderPeer::new(
        vec![],
        vec![Box::new(miss_res)],
    ));
    let mut reg_miss = ExternalReaderRegistry::new(0);
    reg_miss.register_resource_reader_with(
        "miss",
        ExternalReaderConfig::new("miss-reader", vec![], None),
        |_| Arc::new(DirectTransport::new(Arc::clone(&miss_peer))),
    );
    let miss_err = eval_with_registry(
        r#"x = read("miss:absent")"#,
        "file:///main.pkl",
        &reg_miss,
        &["miss"],
    )
    .unwrap_err();
    assert!(
        miss_err.contains("Cannot find resource `miss:absent`."),
        "miss err:\n{miss_err}"
    );
}

#[test]
fn allowlist_rejection_precedes_external_process_initialization() {
    let peer = Arc::new(MockExternalReaderPeer::new(vec![], vec![]));
    let mut reg = ExternalReaderRegistry::new(0);
    reg.register_resource_reader_with(
        "secret",
        ExternalReaderConfig::new("secret-reader", vec![], None),
        |_| Arc::new(DirectTransport::new(Arc::clone(&peer))),
    );

    // "secret" is NOT in allowed_schemes -> SecurityManager rejects before any wire RPC.
    let err =
        eval_with_registry(r#"x = read("secret:key")"#, "file:///main.pkl", &reg, &[]).unwrap_err();
    assert!(
        err.contains("Refusing to read resource `secret:key`"),
        "err:\n{err}"
    );
    assert!(
        peer.recorded_incoming().is_empty(),
        "reader must not be contacted when allowlist denies"
    );
}

#[test]
fn trust_level_prevents_external_module_from_importing_local_file() {
    // External scheme has default trust level 10; file: has trust level 30.
    let mod_reader = MapModuleReader {
        spec: ModuleReaderSpec {
            scheme: "untrusted".into(),
            has_hierarchical_uris: true,
            is_local: false,
            is_globbable: false,
        },
        files: BTreeMap::from([(
            "untrusted:/bad.pkl".into(),
            Some("import \"file:///etc/passwd\"\nx = passwd\n".into()),
        )]),
        dirs: BTreeMap::new(),
        errors: BTreeMap::new(),
        read_calls: Arc::new(AtomicUsize::new(0)),
        list_calls: Arc::new(AtomicUsize::new(0)),
    };
    let peer = Arc::new(MockExternalReaderPeer::new(
        vec![Box::new(mod_reader)],
        vec![],
    ));
    let mut reg = ExternalReaderRegistry::new(0);
    reg.register_module_reader_with(
        "untrusted",
        ExternalReaderConfig::new("untrusted-reader", vec![], None),
        |_| Arc::new(DirectTransport::new(Arc::clone(&peer))),
    );
    let err = eval_with_registry(
        r#"import "untrusted:/bad.pkl"
res = bad.x"#,
        "file:///main.pkl",
        &reg,
        &["untrusted"],
    )
    .unwrap_err();
    assert!(
        err.contains("insufficient trust level"),
        "expected trust level denial:\n{err}"
    );
}

#[test]
fn raw_frame_transport_detects_mismatched_request_id_and_wrong_response_type() {
    // Peer responds with wrong requestId (999)
    let bad_id_transport = Arc::new(RawFrameTransport::new(|_| {
        codec::encode_to_vec(&ExternalReaderMessage::InitializeModuleReaderResponse {
            request_id: 999,
            spec: None,
        })
    }));
    let handle = ExternalReaderProcessHandle::new(bad_id_transport);
    let err = handle.get_module_reader_spec("foo").unwrap_err();
    assert!(
        err.to_string().contains(
            "Received response InitializeModuleReaderResponse for unknown request ID `999`."
        ),
        "{err}"
    );

    // Peer responds with InitializeResourceReaderResponse to InitializeModuleReaderRequest
    let wrong_type_transport = Arc::new(RawFrameTransport::new(|req_bytes| {
        let req = codec::decode_from_slice(req_bytes)?.unwrap();
        codec::encode_to_vec(&ExternalReaderMessage::InitializeResourceReaderResponse {
            request_id: req.request_id().unwrap(),
            spec: None,
        })
    }));
    let handle2 = ExternalReaderProcessHandle::new(wrong_type_transport);
    let err2 = handle2.get_module_reader_spec("foo").unwrap_err();
    assert!(err2.to_string().contains("unexpected response"), "{err2}");
}

#[test]
fn relative_imports_opaque_dummy_uri_and_terminated_process_errors() {
    // 1. Hierarchical external module importing a relative sibling (`ext:/dir/a.pkl` -> `sibling.pkl`)
    let mod_reader = MapModuleReader {
        spec: ModuleReaderSpec {
            scheme: "hier".into(),
            has_hierarchical_uris: true,
            is_local: true,
            is_globbable: true,
        },
        files: BTreeMap::from([
            (
                "hier:/dir/a.pkl".into(),
                Some("import \"sibling.pkl\"\nres = sibling.answer\n".into()),
            ),
            ("hier:/dir/sibling.pkl".into(), Some("answer = 42\n".into())),
        ]),
        dirs: BTreeMap::new(),
        errors: BTreeMap::new(),
        read_calls: Arc::new(AtomicUsize::new(0)),
        list_calls: Arc::new(AtomicUsize::new(0)),
    };
    // 2. Non-hierarchical globbable module (`hasHierarchicalUris = false`) with
    //    URI `op:/entry` performing relative `import*("*")` -> GlobResolver sends `op:dummy`
    let opaque_mod = MapModuleReader {
        spec: ModuleReaderSpec {
            scheme: "op".into(),
            has_hierarchical_uris: false,
            is_local: true,
            is_globbable: true,
        },
        files: BTreeMap::from([
            (
                "op:/entry".into(),
                Some(
                    "mods = import*(\"op:*\").keys.toList()\nrel = import*(\"*\").keys.toList()\n"
                        .into(),
                ),
            ),
            ("op:one".into(), Some("v = 1\n".into())),
            ("op:two".into(), Some("v = 2\n".into())),
        ]),
        dirs: BTreeMap::from([
            (
                "op:*".into(),
                vec![WirePathElement::file("two"), WirePathElement::file("one")],
            ),
            (
                "op:dummy".into(),
                vec![WirePathElement::file("two"), WirePathElement::file("one")],
            ),
        ]),
        errors: BTreeMap::new(),
        read_calls: Arc::new(AtomicUsize::new(0)),
        list_calls: Arc::new(AtomicUsize::new(0)),
    };

    let peer = Arc::new(MockExternalReaderPeer::new(
        vec![Box::new(mod_reader), Box::new(opaque_mod)],
        vec![],
    ));
    let transport = Arc::new(DirectTransport::new(Arc::clone(&peer)));
    let mut reg = ExternalReaderRegistry::new(0);
    let cfg = ExternalReaderConfig::new("multi-reader", vec![], None);
    reg.register_module_reader_with("hier", cfg.clone(), |_| {
        Arc::clone(&transport) as Arc<dyn ExternalReaderTransport>
    });
    reg.register_module_reader_with("op", cfg, |_| {
        Arc::clone(&transport) as Arc<dyn ExternalReaderTransport>
    });

    let out = eval_with_registry(
        r#"
import "hier:/dir/a.pkl" as a
import "op:/entry" as e
answer = a.res
absKeys = e.mods
relKeys = e.rel
"#,
        "file:///main.pkl",
        &reg,
        &["hier", "op"],
    )
    .expect("relative hierarchical & opaque dummy glob");
    assert!(out.contains("answer = 42"), "{out}");
    assert!(
        out.contains("absKeys = List(\"op:one\", \"op:two\")"),
        "{out}"
    );
    assert!(
        out.contains("relKeys = List(\"op:one\", \"op:two\")"),
        "expected relative opaque glob via op:dummy:\n{out}"
    );
    assert!(
        peer.recorded_incoming().iter().any(|m| matches!(
            m,
            ExternalReaderMessage::ListModulesRequest { uri, .. } if uri == "op:dummy"
        )),
        "wire log must record ListModulesRequest with uri `op:dummy`"
    );

    // Multiple schemes ("hier" + "op") sharing one process handle must also use
    // strictly unique requestIds across both resolvers.
    let multi_scheme_ids: Vec<i64> = peer
        .recorded_incoming()
        .iter()
        .filter_map(|m| m.request_id())
        .collect();
    let expected_multi_ids: Vec<i64> = (1..=multi_scheme_ids.len() as i64).collect();
    assert_eq!(
        multi_scheme_ids, expected_multi_ids,
        "multiple schemes sharing one process must not collide on requestId"
    );

    // 3. Process terminates mid-session -> `External reader process has already terminated.`
    transport.mark_terminated();
    let err = eval_with_registry(
        r#"import "hier:/dir/another.pkl" as another
x = another"#,
        "file:///main.pkl",
        &reg,
        &["hier"],
    )
    .unwrap_err();
    assert!(
        err.contains("External reader process has already terminated."),
        "expected terminated error:\n{err}"
    );
}

#[test]
fn evaluator_settings_resolve_external_reader_paths_and_working_dir() {
    let pkl = r#"
import "pkl:EvaluatorSettings"
import "pkl:platform"

local macOS = new platform.OperatingSystem { name = "macOS" }
local settings: EvaluatorSettings = new {
  externalModuleReaders {
    ["bare"] {
      executable = "my-reader"
    }
    ["rel"] {
      executable = "bin/my-reader"
      arguments { "--flag"; "val" }
      workingDir = "../work"
    }
    ["abs"] {
      executable = "/opt/bin/reader"
      workingDir = "/var/tmp"
    }
  }
}
local resolved = settings.resolveForOs("file:///workspace/proj/PklProject", macOS)
bareExe = resolved.externalModuleReaders!!["bare"].executable
bareWd = resolved.externalModuleReaders!!["bare"].workingDir
relExe = resolved.externalModuleReaders!!["rel"].executable
relArgs = resolved.externalModuleReaders!!["rel"].arguments
relWd = resolved.externalModuleReaders!!["rel"].workingDir
absExe = resolved.externalModuleReaders!!["abs"].executable
absWd = resolved.externalModuleReaders!!["abs"].workingDir
"#;
    let reg = ExternalReaderRegistry::new(0);
    let out = eval_with_registry(pkl, "file:///settings_check.pkl", &reg, &[])
        .expect("EvaluatorSettings.resolveForOs");
    assert!(out.contains("bareExe = \"my-reader\""), "{out}");
    assert!(out.contains("bareWd = \"/workspace/proj\""), "{out}");
    assert!(
        out.contains("relExe = \"/workspace/proj/bin/my-reader\""),
        "{out}"
    );
    assert!(out.contains("relWd = \"/workspace/work\""), "{out}");
    assert!(out.contains("absExe = \"/opt/bin/reader\""), "{out}");
    assert!(out.contains("absWd = \"/var/tmp\""), "{out}");
}

#[test]
fn cli_option_parser_rejects_malformed_specs() {
    assert!(ExternalReaderConfig::parse_cli_option("no-equals").is_err());
    assert!(ExternalReaderConfig::parse_cli_option("=cmd").is_err());
    assert!(ExternalReaderConfig::parse_cli_option("foo=   ").is_err());
}
