#![forbid(unsafe_code)]

use pkl_runtime::external_reader::{
    ExternalReaderError, ExternalReaderMessage, MessageType, ModuleReaderSpec, ResourceReaderSpec,
    WirePathElement, codec as ext_codec,
};
use pkl_runtime::logging::TraceMode;
use pkl_runtime::server::{
    ServerChecksums, ServerDependency, ServerExternalReader, ServerHttp, ServerMessage,
    ServerProject, ServerProxy, codec,
};
use std::collections::BTreeMap;

fn sample_full_create_evaluator_request() -> ServerMessage {
    let mut env = BTreeMap::new();
    env.insert("FOO".into(), "BAR".into());
    env.insert("EMPTY".into(), String::new());

    let mut properties = BTreeMap::new();
    properties.insert("prop1".into(), "val1".into());
    properties.insert("prop2".into(), "val2".into());

    let mut nested_deps = BTreeMap::new();
    nested_deps.insert(
        "nestedRemote".into(),
        ServerDependency::Remote {
            package_uri: "package://example.com/nested@1.2.3".into(),
            checksums: Some(ServerChecksums {
                sha256: "deadbeef0123456789abcdef".into(),
            }),
        },
    );

    let mut root_deps = BTreeMap::new();
    root_deps.insert(
        "remoteNoChecksum".into(),
        ServerDependency::Remote {
            package_uri: "package://example.com/lib@0.1.0".into(),
            checksums: None,
        },
    );
    root_deps.insert(
        "localDep".into(),
        ServerDependency::Local(ServerProject {
            project_file_uri: "file:///workspace/dep/PklProject".into(),
            package_uri: Some("package://example.com/localDep@2.0.0".into()),
            dependencies: nested_deps,
        }),
    );

    let mut rewrites = BTreeMap::new();
    rewrites.insert(
        "https://example.com/".into(),
        "https://localhost:8443/".into(),
    );

    let mut header_values = BTreeMap::new();
    header_values.insert("Authorization".into(), vec!["Bearer token".into()]);
    header_values.insert("X-Multi".into(), vec!["a".into(), "b".into()]);
    let mut headers = BTreeMap::new();
    headers.insert("https://localhost:8443/*".into(), header_values);

    let mut ext_mod = BTreeMap::new();
    ext_mod.insert(
        "extmod".into(),
        ServerExternalReader {
            executable: "/usr/local/bin/ext-reader".into(),
            arguments: Some(vec!["--mode".into(), "module".into()]),
            working_dir: Some("/tmp".into()),
        },
    );

    let mut ext_res = BTreeMap::new();
    ext_res.insert(
        "extres".into(),
        ServerExternalReader {
            executable: "/usr/local/bin/ext-res".into(),
            arguments: None,
            working_dir: None,
        },
    );

    ServerMessage::CreateEvaluatorRequest {
        request_id: 12345,
        allowed_modules: Some(vec!["pkl:".into(), "repl:".into(), "file:".into()]),
        allowed_resources: Some(vec!["prop:".into(), "env:".into()]),
        client_module_readers: Some(vec![ModuleReaderSpec {
            scheme: "bird".into(),
            has_hierarchical_uris: true,
            is_local: true,
            is_globbable: true,
        }]),
        client_resource_readers: Some(vec![ResourceReaderSpec {
            scheme: "secret".into(),
            has_hierarchical_uris: false,
            is_globbable: false,
        }]),
        module_paths: Some(vec!["/path/one".into(), "/path/two.jar".into()]),
        env: Some(env),
        properties: Some(properties),
        timeout_seconds: Some(30),
        root_dir: Some("/workspace".into()),
        cache_dir: Some("/tmp/pkl-cache".into()),
        output_format: Some("json".into()),
        project: Some(ServerProject {
            project_file_uri: "file:///workspace/PklProject".into(),
            package_uri: None,
            dependencies: root_deps,
        }),
        http: Some(ServerHttp {
            ca_certificates: Some(
                b"-----BEGIN CERTIFICATE-----\nTEST\n-----END CERTIFICATE-----\n".to_vec(),
            ),
            proxy: Some(ServerProxy {
                address: Some("http://127.0.0.1:8080".into()),
                no_proxy: Some(vec!["localhost".into(), "127.0.0.1".into()]),
            }),
            rewrites: Some(rewrites),
            headers: Some(headers),
        }),
        external_module_readers: Some(ext_mod),
        external_resource_readers: Some(ext_res),
        trace_mode: Some(TraceMode::Pretty),
    }
}

#[test]
fn round_trip_all_14_server_message_variants() {
    let messages = vec![
        sample_full_create_evaluator_request(),
        ServerMessage::CreateEvaluatorRequest {
            request_id: -7,
            allowed_modules: None,
            allowed_resources: None,
            client_module_readers: None,
            client_resource_readers: None,
            module_paths: None,
            env: None,
            properties: None,
            timeout_seconds: None,
            root_dir: None,
            cache_dir: None,
            output_format: None,
            project: None,
            http: None,
            external_module_readers: None,
            external_resource_readers: None,
            trace_mode: Some(TraceMode::Compact),
        },
        ServerMessage::CreateEvaluatorResponse {
            request_id: 1,
            evaluator_id: Some(i64::MAX),
            error: None,
        },
        ServerMessage::CreateEvaluatorResponse {
            request_id: 2,
            evaluator_id: None,
            error: Some("Invalid configuration".into()),
        },
        ServerMessage::CloseEvaluator { evaluator_id: -42 },
        ServerMessage::EvaluateRequest {
            request_id: 10,
            evaluator_id: 99,
            module_uri: "file:///a/b.pkl".into(),
            module_text: Some("x = 1".into()),
            expr: Some("output.text".into()),
        },
        ServerMessage::EvaluateRequest {
            request_id: 11,
            evaluator_id: 99,
            module_uri: "repl:text".into(),
            module_text: None,
            expr: None,
        },
        ServerMessage::EvaluateResponse {
            request_id: 10,
            evaluator_id: 99,
            result: Some(vec![0x94, 0x01, 0xa1, b'a']),
            error: None,
        },
        ServerMessage::EvaluateResponse {
            request_id: 11,
            evaluator_id: 99,
            result: None,
            error: Some("–– Pkl Error ––\nboom\n".into()),
        },
        ServerMessage::LogMessage {
            evaluator_id: 99,
            level: 0,
            message: "x = 1".into(),
            frame_uri: "repl:text".into(),
        },
        ServerMessage::LogMessage {
            evaluator_id: 99,
            level: 1,
            message: "deprecated".into(),
            frame_uri: "file:///a.pkl".into(),
        },
        ServerMessage::ReadResourceRequest {
            request_id: 20,
            evaluator_id: 99,
            uri: "secret:/db-pass".into(),
        },
        ServerMessage::ReadResourceResponse {
            request_id: 20,
            evaluator_id: 99,
            contents: Some(vec![0, 1, 2, 255]),
            error: None,
        },
        ServerMessage::ReadResourceResponse {
            request_id: 21,
            evaluator_id: 99,
            contents: None,
            error: Some("read failed".into()),
        },
        ServerMessage::ReadModuleRequest {
            request_id: 30,
            evaluator_id: 99,
            uri: "bird:/pigeon.pkl".into(),
        },
        ServerMessage::ReadModuleResponse {
            request_id: 30,
            evaluator_id: 99,
            contents: Some("name = \"Pigeon\"".into()),
            error: None,
        },
        ServerMessage::ListResourcesRequest {
            request_id: 40,
            evaluator_id: 99,
            uri: "secret:/".into(),
        },
        ServerMessage::ListResourcesResponse {
            request_id: 40,
            evaluator_id: 99,
            path_elements: Some(vec![
                WirePathElement {
                    name: "a.txt".into(),
                    is_directory: false,
                },
                WirePathElement {
                    name: "sub".into(),
                    is_directory: true,
                },
            ]),
            error: None,
        },
        ServerMessage::ListModulesRequest {
            request_id: 50,
            evaluator_id: 99,
            uri: "bird:/".into(),
        },
        ServerMessage::ListModulesResponse {
            request_id: 50,
            evaluator_id: 99,
            path_elements: Some(vec![]),
            error: None,
        },
    ];

    let mut stream = Vec::new();
    for msg in &messages {
        let encoded = codec::encode_to_vec(msg).expect("encode");
        let decoded = codec::decode_from_slice(&encoded)
            .expect("decode")
            .expect("some message");
        assert_eq!(&decoded, msg, "round-trip mismatch for {msg:?}");
        stream.extend_from_slice(&encoded);
    }

    let decoded_all = codec::decode_all_from_slice(&stream).expect("decode concatenated stream");
    assert_eq!(decoded_all, messages);
}

#[test]
fn trace_mode_encodes_uppercase_and_decodes_case_insensitively() {
    let pretty = ServerMessage::CreateEvaluatorRequest {
        request_id: 1,
        allowed_modules: None,
        allowed_resources: None,
        client_module_readers: None,
        client_resource_readers: None,
        module_paths: None,
        env: None,
        properties: None,
        timeout_seconds: None,
        root_dir: None,
        cache_dir: None,
        output_format: None,
        project: None,
        http: None,
        external_module_readers: None,
        external_resource_readers: None,
        trace_mode: Some(TraceMode::Pretty),
    };
    let wire = codec::encode_to_vec(&pretty).expect("encode");
    assert!(
        wire.windows(6).any(|w| w == b"PRETTY"),
        "expected uppercase PRETTY in encoded wire bytes"
    );

    // Replace "PRETTY" with lowercase "pretty" on wire; decoder must accept case-insensitively.
    let mut lower_wire = wire.clone();
    let pos = lower_wire.windows(6).position(|w| w == b"PRETTY").unwrap();
    lower_wire[pos..pos + 6].copy_from_slice(b"pretty");
    let decoded = codec::decode_from_slice(&lower_wire).unwrap().unwrap();
    assert_eq!(decoded, pretty);
}

#[test]
fn external_reader_only_codes_rejected_with_exact_java_error() {
    let ext_messages = [
        (
            "2e",
            ExternalReaderMessage::InitializeModuleReaderRequest {
                request_id: 1,
                scheme: "ext".into(),
            },
        ),
        (
            "2f",
            ExternalReaderMessage::InitializeModuleReaderResponse {
                request_id: 1,
                spec: None,
            },
        ),
        (
            "30",
            ExternalReaderMessage::InitializeResourceReaderRequest {
                request_id: 2,
                scheme: "ext".into(),
            },
        ),
        (
            "31",
            ExternalReaderMessage::InitializeResourceReaderResponse {
                request_id: 2,
                spec: None,
            },
        ),
        ("32", ExternalReaderMessage::CloseExternalProcess),
    ];

    for (hex, ext_msg) in ext_messages {
        let bytes = ext_codec::encode_to_vec(&ext_msg).expect("encode ext message");
        let err = codec::decode_from_slice(&bytes).expect_err("must reject ext code on server");
        match err {
            ExternalReaderError::Decode { message, .. } => {
                assert_eq!(message, format!("Unhandled decoding message code `{hex}`."));
            }
            other => panic!("unexpected error variant: {other:?}"),
        }
    }
}

#[test]
fn malformed_headers_and_unrecognized_codes() {
    // Empty input -> clean EOF (Ok(None)).
    assert_eq!(codec::decode_from_slice(&[]).unwrap(), None);

    // Unrecognized code 0x7f: [0x7f, {}]
    let unknown = [0x92u8, 0x7f, 0x80];
    let err = codec::decode_from_slice(&unknown).unwrap_err();
    assert_eq!(
        err.to_string(),
        "Malformed message header (unrecognized code `7f`)."
    );

    // Wrong array size: [0x20]
    let bad_len = [0x91u8, 0x20];
    let err = codec::decode_from_slice(&bad_len).unwrap_err();
    assert_eq!(
        err.to_string(),
        "Malformed message header (expected size 2, but got 1)."
    );

    // Non-array header: 0x01
    let bad_header = [0x01u8];
    let err = codec::decode_from_slice(&bad_header).unwrap_err();
    assert!(
        err.to_string().starts_with("Malformed message header."),
        "got: {err}"
    );
}

#[test]
fn bridge_0x26_to_0x2d_preserves_all_fields() {
    let pairs = vec![
        (
            ServerMessage::ReadResourceRequest {
                request_id: 1,
                evaluator_id: 2,
                uri: "cb:/a".into(),
            },
            MessageType::ReadResourceRequest,
        ),
        (
            ServerMessage::ReadResourceResponse {
                request_id: 1,
                evaluator_id: 2,
                contents: Some(b"ok".to_vec()),
                error: None,
            },
            MessageType::ReadResourceResponse,
        ),
        (
            ServerMessage::ReadModuleRequest {
                request_id: 3,
                evaluator_id: 2,
                uri: "cb:/m.pkl".into(),
            },
            MessageType::ReadModuleRequest,
        ),
        (
            ServerMessage::ReadModuleResponse {
                request_id: 3,
                evaluator_id: 2,
                contents: Some("x = 1".into()),
                error: None,
            },
            MessageType::ReadModuleResponse,
        ),
        (
            ServerMessage::ListResourcesRequest {
                request_id: 4,
                evaluator_id: 2,
                uri: "cb:/".into(),
            },
            MessageType::ListResourcesRequest,
        ),
        (
            ServerMessage::ListResourcesResponse {
                request_id: 4,
                evaluator_id: 2,
                path_elements: Some(vec![WirePathElement {
                    name: "a".into(),
                    is_directory: false,
                }]),
                error: None,
            },
            MessageType::ListResourcesResponse,
        ),
        (
            ServerMessage::ListModulesRequest {
                request_id: 5,
                evaluator_id: 2,
                uri: "cb:/".into(),
            },
            MessageType::ListModulesRequest,
        ),
        (
            ServerMessage::ListModulesResponse {
                request_id: 5,
                evaluator_id: 2,
                path_elements: None,
                error: Some("nope".into()),
            },
            MessageType::ListModulesResponse,
        ),
    ];

    for (msg, expected_type) in pairs {
        assert_eq!(msg.message_type(), expected_type);
        let ext = msg.clone().try_into_external_reader().expect("to ext");
        let back = ServerMessage::try_from_external_reader(ext).expect("from ext");
        assert_eq!(back, msg);
    }
}
