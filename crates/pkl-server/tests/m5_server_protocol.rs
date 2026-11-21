#![forbid(unsafe_code)]

use pkl_runtime::external_reader::{
    ExternalReaderMessage, ModuleReaderSpec, ResourceReaderSpec, WirePathElement,
    codec as ext_codec,
};
use pkl_runtime::logging::TraceMode;
use pkl_runtime::pkl_binary_decode::{DecodedObject, DecodedValue, decode as decode_pkl_binary};
use pkl_runtime::server::{
    ServerDependency, ServerExternalReader, ServerHttp, ServerMessage, ServerProject, codec,
};
use pkl_server::{ServerError, run_stream_server};
use std::collections::{BTreeMap, VecDeque};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::thread;

// ============================================================================
// In-Memory Blocking Byte Pipe (zero OS FDs, zero buffer-full deadlocks)
// ============================================================================

struct PipeState {
    buf: VecDeque<u8>,
    closed: bool,
}

struct PipeReader {
    inner: Arc<(Mutex<PipeState>, Condvar)>,
}

struct PipeWriter {
    inner: Arc<(Mutex<PipeState>, Condvar)>,
}

fn memory_pipe() -> (PipeWriter, PipeReader) {
    let inner = Arc::new((
        Mutex::new(PipeState {
            buf: VecDeque::new(),
            closed: false,
        }),
        Condvar::new(),
    ));
    (
        PipeWriter {
            inner: Arc::clone(&inner),
        },
        PipeReader { inner },
    )
}

impl Write for PipeWriter {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        let (lock, cv) = &*self.inner;
        let mut state = lock.lock().unwrap_or_else(|e| e.into_inner());
        if state.closed {
            return Err(io::Error::new(io::ErrorKind::BrokenPipe, "pipe closed"));
        }
        state.buf.extend(data);
        cv.notify_all();
        Ok(data.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Drop for PipeWriter {
    fn drop(&mut self) {
        let (lock, cv) = &*self.inner;
        let mut state = lock.lock().unwrap_or_else(|e| e.into_inner());
        state.closed = true;
        cv.notify_all();
    }
}

impl Read for PipeReader {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        let (lock, cv) = &*self.inner;
        let mut state = lock.lock().unwrap_or_else(|e| e.into_inner());
        loop {
            if !state.buf.is_empty() {
                let mut n = 0;
                while n < out.len() {
                    if let Some(b) = state.buf.pop_front() {
                        out[n] = b;
                        n += 1;
                    } else {
                        break;
                    }
                }
                return Ok(n);
            }
            if state.closed {
                return Ok(0);
            }
            state = cv.wait(state).unwrap_or_else(|e| e.into_inner());
        }
    }
}

struct TestClient {
    tx: Option<PipeWriter>,
    rx: PipeReader,
    server_thread: Option<thread::JoinHandle<Result<(), ServerError>>>,
}

impl TestClient {
    fn spawn() -> Self {
        let (c_to_s_tx, c_to_s_rx) = memory_pipe();
        let (s_to_c_tx, s_to_c_rx) = memory_pipe();
        let handle = thread::spawn(move || run_stream_server(c_to_s_rx, s_to_c_tx));
        Self {
            tx: Some(c_to_s_tx),
            rx: s_to_c_rx,
            server_thread: Some(handle),
        }
    }

    fn send(&mut self, msg: &ServerMessage) {
        let tx = self.tx.as_mut().expect("client tx open");
        codec::write_message(tx, msg).expect("write client message");
    }

    fn send_raw(&mut self, bytes: &[u8]) {
        let tx = self.tx.as_mut().expect("client tx open");
        tx.write_all(bytes).expect("write raw bytes");
        tx.flush().expect("flush raw bytes");
    }

    fn recv(&mut self) -> Option<ServerMessage> {
        codec::read_message(&mut self.rx).expect("read server message")
    }

    fn close_and_join(mut self) -> Result<(), ServerError> {
        drop(self.tx.take());
        self.server_thread
            .take()
            .expect("server handle")
            .join()
            .expect("server thread did not panic")
    }
}

fn minimal_create(request_id: i64) -> ServerMessage {
    ServerMessage::CreateEvaluatorRequest {
        request_id,
        allowed_modules: Some(vec!["pkl:".into(), "repl:".into()]),
        allowed_resources: Some(vec!["prop:".into()]),
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
        trace_mode: None,
    }
}

fn create_ok(client: &mut TestClient, msg: ServerMessage) -> i64 {
    let expected_req_id = msg.request_id().unwrap();
    client.send(&msg);
    match client.recv().expect("CreateEvaluatorResponse") {
        ServerMessage::CreateEvaluatorResponse {
            request_id,
            evaluator_id: Some(id),
            error: None,
        } => {
            assert_eq!(request_id, expected_req_id);
            id
        }
        other => panic!("expected successful CreateEvaluatorResponse, got {other:?}"),
    }
}

#[test]
fn eof_empty_exits_cleanly() {
    let client = TestClient::spawn();
    assert!(client.close_and_join().is_ok());
}

#[test]
fn create_close_and_unknown_close_ignored() {
    let mut client = TestClient::spawn();
    let eval_id = create_ok(&mut client, minimal_create(1));
    assert_ne!(eval_id, 0);

    client.send(&ServerMessage::CloseEvaluator {
        evaluator_id: eval_id,
    });
    // Closing unknown evaluator ID is logged/ignored, not fatal.
    client.send(&ServerMessage::CloseEvaluator {
        evaluator_id: 999_999_999,
    });
    assert!(client.close_and_join().is_ok());
}

#[test]
fn evaluate_module_binary_and_output_format() {
    let mut client = TestClient::spawn();
    let mut create = minimal_create(1);
    if let ServerMessage::CreateEvaluatorRequest { output_format, .. } = &mut create {
        *output_format = Some("json".into());
    }
    let eval_id = create_ok(&mut client, create);

    // Default expr == None -> "module"
    client.send(&ServerMessage::EvaluateRequest {
        request_id: 2,
        evaluator_id: eval_id,
        module_uri: "repl:text".into(),
        module_text: Some("a = 42\nb = List(\"x\", true, null)\n".into()),
        expr: None,
    });
    let resp = client.recv().expect("EvaluateResponse");
    match resp {
        ServerMessage::EvaluateResponse {
            request_id: 2,
            evaluator_id,
            result: Some(bytes),
            error: None,
        } => {
            assert_eq!(evaluator_id, eval_id);
            let decoded = decode_pkl_binary(&bytes).expect("decode pkl-binary");
            match decoded {
                DecodedValue::Object(DecodedObject {
                    class_name,
                    module_uri,
                    members,
                    ..
                }) => {
                    assert_eq!(class_name.to_java_utf8(), "text");
                    assert_eq!(module_uri.to_java_utf8(), "repl:text");
                    assert_eq!(members.len(), 2);
                }
                other => panic!("expected Object, got {other:?}"),
            }
        }
        other => panic!("unexpected response: {other:?}"),
    }

    // Explicit expr == "output.text" honoring outputFormat == "json"
    client.send(&ServerMessage::EvaluateRequest {
        request_id: 3,
        evaluator_id: eval_id,
        module_uri: "repl:text".into(),
        module_text: Some("a = 42\nb = List(\"x\", true, null)\n".into()),
        expr: Some("output.text".into()),
    });
    let resp = client.recv().expect("EvaluateResponse");
    match resp {
        ServerMessage::EvaluateResponse {
            request_id: 3,
            evaluator_id,
            result: Some(bytes),
            error: None,
        } => {
            assert_eq!(evaluator_id, eval_id);
            let decoded = decode_pkl_binary(&bytes).expect("decode pkl-binary");
            assert_eq!(
                decoded,
                DecodedValue::String(
                    "{\n  \"a\": 42,\n  \"b\": [\n    \"x\",\n    true,\n    null\n  ]\n}\n".into()
                )
            );
        }
        other => panic!("unexpected response: {other:?}"),
    }

    client.send(&ServerMessage::CloseEvaluator {
        evaluator_id: eval_id,
    });
    assert!(client.close_and_join().is_ok());
}

#[test]
fn evaluate_error_and_unknown_evaluator() {
    let mut client = TestClient::spawn();
    let eval_id = create_ok(&mut client, minimal_create(1));

    client.send(&ServerMessage::EvaluateRequest {
        request_id: 2,
        evaluator_id: eval_id,
        module_uri: "repl:text".into(),
        module_text: Some("x = throw(\"server-eval-err\")\n".into()),
        expr: None,
    });
    match client.recv().expect("EvaluateResponse") {
        ServerMessage::EvaluateResponse {
            request_id: 2,
            evaluator_id,
            result: None,
            error: Some(err),
        } => {
            assert_eq!(evaluator_id, eval_id);
            assert!(
                err.starts_with("–– Pkl Error ––\nserver-eval-err\n"),
                "got: {err:?}"
            );
            assert!(
                err.ends_with('\n'),
                "Pkl error should end with newline: {err:?}"
            );
        }
        other => panic!("unexpected response: {other:?}"),
    }

    client.send(&ServerMessage::CloseEvaluator {
        evaluator_id: eval_id,
    });

    client.send(&ServerMessage::EvaluateRequest {
        request_id: 3,
        evaluator_id: 999_999_999,
        module_uri: "repl:text".into(),
        module_text: Some("x = 1".into()),
        expr: None,
    });
    match client.recv().expect("EvaluateResponse") {
        ServerMessage::EvaluateResponse {
            request_id: 3,
            evaluator_id: 999_999_999,
            result: None,
            error: Some(err),
        } => {
            assert_eq!(err, "Evaluator with ID 999999999 was not found.");
        }
        other => panic!("unexpected response: {other:?}"),
    }

    assert!(client.close_and_join().is_ok());
}

#[test]
fn create_invalid_http_rewrite_returns_protocol_error_before_security_check() {
    let mut client = TestClient::spawn();

    // Both allowlists omitted, yet invalid HTTP rewrite MUST return CreateEvaluatorResponse.error
    // (NOT fatal "No security manager set."), matching AbstractServerTest.
    let mut rewrites = BTreeMap::new();
    rewrites.insert(
        "https://example.com/no-slash".into(),
        "https://other.com/".into(),
    );
    client.send(&ServerMessage::CreateEvaluatorRequest {
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
        http: Some(ServerHttp {
            ca_certificates: None,
            proxy: None,
            rewrites: Some(rewrites),
            headers: None,
        }),
        external_module_readers: None,
        external_resource_readers: None,
        trace_mode: None,
    });
    match client.recv().expect("CreateEvaluatorResponse") {
        ServerMessage::CreateEvaluatorResponse {
            request_id: 1,
            evaluator_id: None,
            error: Some(err),
        } => {
            assert_eq!(
                err,
                "Rewrite rule must end with '/', but was 'https://example.com/no-slash'"
            );
        }
        other => panic!("expected CreateEvaluatorResponse error, got {other:?}"),
    }

    // Server stays alive and serves subsequent requests.
    let ok_id = create_ok(&mut client, minimal_create(2));
    client.send(&ServerMessage::EvaluateRequest {
        request_id: 3,
        evaluator_id: ok_id,
        module_uri: "repl:text".into(),
        module_text: Some("ok = true".into()),
        expr: Some("ok".into()),
    });
    match client.recv().expect("EvaluateResponse") {
        ServerMessage::EvaluateResponse {
            request_id: 3,
            result: Some(bytes),
            error: None,
            ..
        } => {
            assert_eq!(decode_pkl_binary(&bytes).unwrap(), DecodedValue::Bool(true));
        }
        other => panic!("unexpected response: {other:?}"),
    }
    assert!(client.close_and_join().is_ok());
}

#[test]
fn both_allowlists_missing_is_fatal_security_manager_error() {
    let mut client = TestClient::spawn();
    client.send(&ServerMessage::CreateEvaluatorRequest {
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
        trace_mode: None,
    });
    let err = client
        .close_and_join()
        .expect_err("expected fatal error when both allowlists omitted");
    assert_eq!(err.to_string(), "No security manager set.");
}

#[test]
fn unhandled_external_reader_code_rejected_fatally() {
    let mut client = TestClient::spawn();
    let raw = ext_codec::encode_to_vec(&ExternalReaderMessage::InitializeModuleReaderRequest {
        request_id: 1,
        scheme: "extmod".into(),
    })
    .unwrap();
    client.send_raw(&raw);
    let err = client
        .close_and_join()
        .expect_err("expected fatal decode error");
    assert_eq!(err.to_string(), "Unhandled decoding message code `2e`.");
}

#[test]
fn logs_trace_compact_pretty_and_warn() {
    let mut client = TestClient::spawn();

    // Compact trace + deprecation warning
    let mut compact_req = minimal_create(1);
    if let ServerMessage::CreateEvaluatorRequest { trace_mode, .. } = &mut compact_req {
        *trace_mode = Some(TraceMode::Compact);
    }
    let eval_c = create_ok(&mut client, compact_req);
    client.send(&ServerMessage::EvaluateRequest {
        request_id: 2,
        evaluator_id: eval_c,
        module_uri: "repl:text".into(),
        module_text: Some(
            "@Deprecated { message = \"use bar\" }\nfunction foo() = 5\nx = trace(new { a = 1; b = 2 })\ny = foo()\n"
                .into(),
        ),
        expr: None,
    });

    let mut logs = Vec::new();
    let eval_resp = loop {
        match client.recv().expect("message") {
            ServerMessage::LogMessage {
                evaluator_id,
                level,
                message,
                frame_uri,
            } => {
                assert_eq!(evaluator_id, eval_c);
                logs.push((level, message, frame_uri));
            }
            resp @ ServerMessage::EvaluateResponse { .. } => break resp,
            other => panic!("unexpected message: {other:?}"),
        }
    };
    assert!(matches!(
        eval_resp,
        ServerMessage::EvaluateResponse {
            request_id: 2,
            result: Some(_),
            error: None,
            ..
        }
    ));
    assert_eq!(
        logs,
        vec![
            (
                0,
                "new { a = 1; b = 2 } = new Dynamic { a = 1; b = 2 }".to_string(),
                "repl:text".to_string(),
            ),
            (
                1,
                "Method `text#foo` is deprecated: use bar".to_string(),
                "repl:text".to_string(),
            ),
        ]
    );
    client.send(&ServerMessage::CloseEvaluator {
        evaluator_id: eval_c,
    });

    // Pretty trace
    let mut pretty_req = minimal_create(3);
    if let ServerMessage::CreateEvaluatorRequest { trace_mode, .. } = &mut pretty_req {
        *trace_mode = Some(TraceMode::Pretty);
    }
    let eval_p = create_ok(&mut client, pretty_req);
    client.send(&ServerMessage::EvaluateRequest {
        request_id: 4,
        evaluator_id: eval_p,
        module_uri: "repl:text".into(),
        module_text: Some("x = trace(new { a = 1; b = 2 })\n".into()),
        expr: None,
    });
    let mut pretty_logs = Vec::new();
    loop {
        match client.recv().expect("message") {
            ServerMessage::LogMessage { level, message, .. } => pretty_logs.push((level, message)),
            ServerMessage::EvaluateResponse {
                request_id: 4,
                result: Some(_),
                error: None,
                ..
            } => break,
            other => panic!("unexpected message: {other:?}"),
        }
    }
    assert_eq!(pretty_logs.len(), 1);
    assert_eq!(pretty_logs[0].0, 0);
    assert!(
        pretty_logs[0].1.contains('\n'),
        "pretty trace should be multiline: {:?}",
        pretty_logs[0].1
    );

    client.send(&ServerMessage::CloseEvaluator {
        evaluator_id: eval_p,
    });
    assert!(client.close_and_join().is_ok());
}

#[test]
fn client_resource_read_and_glob_callbacks() {
    let mut client = TestClient::spawn();
    let eval_id = create_ok(
        &mut client,
        ServerMessage::CreateEvaluatorRequest {
            request_id: 1,
            allowed_modules: Some(vec!["pkl:".into(), "repl:".into()]),
            allowed_resources: Some(vec!["cbres:".into()]),
            client_module_readers: None,
            client_resource_readers: Some(vec![ResourceReaderSpec {
                scheme: "cbres".into(),
                has_hierarchical_uris: true,
                is_globbable: true,
            }]),
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
            trace_mode: None,
        },
    );

    client.send(&ServerMessage::EvaluateRequest {
        request_id: 2,
        evaluator_id: eval_id,
        module_uri: "repl:text".into(),
        module_text: Some(
            "ok = read(\"cbres:/ok.txt\").text\nmiss = read?(\"cbres:/missing.txt\")\nall = read*(\"cbres:/*.txt\").toMap().mapValues((_, r) -> r.text)\n"
                .into(),
        ),
        expr: Some("List(ok, miss, all)".into()),
    });

    let result_bytes = loop {
        match client.recv().expect("server message") {
            ServerMessage::ReadResourceRequest {
                request_id,
                evaluator_id,
                uri,
            } => {
                assert_eq!(evaluator_id, eval_id);
                let (contents, error) = match uri.as_str() {
                    "cbres:/ok.txt" => (Some(b"hello-cb".to_vec()), None),
                    "cbres:/a.txt" => (Some(b"A".to_vec()), None),
                    "cbres:/b.txt" => (Some(b"B".to_vec()), None),
                    "cbres:/missing.txt" => (None, None),
                    other => (None, Some(format!("unexpected {other}"))),
                };
                client.send(&ServerMessage::ReadResourceResponse {
                    request_id,
                    evaluator_id,
                    contents,
                    error,
                });
            }
            ServerMessage::ListResourcesRequest {
                request_id,
                evaluator_id,
                uri,
            } => {
                assert_eq!(evaluator_id, eval_id);
                assert_eq!(uri, "cbres:/");
                client.send(&ServerMessage::ListResourcesResponse {
                    request_id,
                    evaluator_id,
                    path_elements: Some(vec![
                        WirePathElement {
                            name: "a.txt".into(),
                            is_directory: false,
                        },
                        WirePathElement {
                            name: "b.txt".into(),
                            is_directory: false,
                        },
                    ]),
                    error: None,
                });
            }
            ServerMessage::EvaluateResponse {
                request_id: 2,
                result: Some(bytes),
                error: None,
                ..
            } => break bytes,
            other => panic!("unexpected message: {other:?}"),
        }
    };

    let decoded = decode_pkl_binary(&result_bytes).expect("decode");
    match decoded {
        DecodedValue::List(items) => {
            assert_eq!(items.len(), 3);
            assert_eq!(items[0], DecodedValue::String("hello-cb".into()));
            assert_eq!(items[1], DecodedValue::Null);
            assert!(matches!(items[2], DecodedValue::Map(ref m) if m.len() == 2));
        }
        other => panic!("expected List, got {other:?}"),
    }

    // Second read of cbres:/missing.txt must hit cache (no callback) and report Cannot find resource.
    client.send(&ServerMessage::EvaluateRequest {
        request_id: 3,
        evaluator_id: eval_id,
        module_uri: "repl:text".into(),
        module_text: Some("x = read(\"cbres:/missing.txt\")".into()),
        expr: None,
    });
    match client.recv().expect("EvaluateResponse") {
        ServerMessage::EvaluateResponse {
            request_id: 3,
            result: None,
            error: Some(err),
            ..
        } => {
            assert!(
                err.contains("Cannot find resource `cbres:/missing.txt`."),
                "got: {err}"
            );
        }
        other => panic!("expected cached missing error without callback, got {other:?}"),
    }

    client.send(&ServerMessage::CloseEvaluator {
        evaluator_id: eval_id,
    });
    assert!(client.close_and_join().is_ok());
}

#[test]
fn client_module_read_glob_and_triple_dot_callbacks() {
    let mut client = TestClient::spawn();
    let eval_id = create_ok(
        &mut client,
        ServerMessage::CreateEvaluatorRequest {
            request_id: 1,
            allowed_modules: Some(vec!["pkl:".into(), "repl:".into(), "cbmod:".into()]),
            allowed_resources: Some(vec!["prop:".into()]),
            client_module_readers: Some(vec![ModuleReaderSpec {
                scheme: "cbmod".into(),
                has_hierarchical_uris: true,
                is_local: true,
                is_globbable: true,
            }]),
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
            trace_mode: None,
        },
    );

    client.send(&ServerMessage::EvaluateRequest {
        request_id: 2,
        evaluator_id: eval_id,
        module_uri: "cbmod:/a/b/child.pkl".into(),
        module_text: None,
        expr: Some("val".into()),
    });

    let bytes = loop {
        match client.recv().expect("server message") {
            ServerMessage::ReadModuleRequest {
                request_id,
                evaluator_id,
                uri,
            } => {
                let (contents, error) = match uri.as_str() {
                    "cbmod:/a/b/child.pkl" => (
                        Some("import \".../shared.pkl\"\nval = shared.x\n".into()),
                        None,
                    ),
                    "cbmod:/shared.pkl" => (Some("x = 77\n".into()), None),
                    _ => (None, Some(format!("not found: {uri}"))),
                };
                client.send(&ServerMessage::ReadModuleResponse {
                    request_id,
                    evaluator_id,
                    contents,
                    error,
                });
            }
            ServerMessage::EvaluateResponse {
                request_id: 2,
                result: Some(bytes),
                error: None,
                ..
            } => break bytes,
            other => panic!("unexpected message: {other:?}"),
        }
    };
    assert_eq!(decode_pkl_binary(&bytes).unwrap(), DecodedValue::Int(77));

    // Globbing modules
    client.send(&ServerMessage::EvaluateRequest {
        request_id: 3,
        evaluator_id: eval_id,
        module_uri: "repl:text".into(),
        module_text: Some("import* \"cbmod:/g/*.pkl\" as ms\ncount = ms.length\n".into()),
        expr: Some("count".into()),
    });
    let bytes = loop {
        match client.recv().expect("server message") {
            ServerMessage::ListModulesRequest {
                request_id,
                evaluator_id,
                uri,
            } => {
                assert_eq!(uri, "cbmod:/g/");
                client.send(&ServerMessage::ListModulesResponse {
                    request_id,
                    evaluator_id,
                    path_elements: Some(vec![
                        WirePathElement {
                            name: "m1.pkl".into(),
                            is_directory: false,
                        },
                        WirePathElement {
                            name: "m2.pkl".into(),
                            is_directory: false,
                        },
                    ]),
                    error: None,
                });
            }
            ServerMessage::ReadModuleRequest {
                request_id,
                evaluator_id,
                ..
            } => {
                client.send(&ServerMessage::ReadModuleResponse {
                    request_id,
                    evaluator_id,
                    contents: Some("v = 1\n".into()),
                    error: None,
                });
            }
            ServerMessage::EvaluateResponse {
                request_id: 3,
                result: Some(bytes),
                error: None,
                ..
            } => break bytes,
            other => panic!("unexpected message: {other:?}"),
        }
    };
    assert_eq!(decode_pkl_binary(&bytes).unwrap(), DecodedValue::Int(2));

    client.send(&ServerMessage::CloseEvaluator {
        evaluator_id: eval_id,
    });
    assert!(client.close_and_join().is_ok());
}

#[test]
fn pipelined_fifo_and_interleaved_create_while_blocked_on_callback() {
    let mut client = TestClient::spawn();
    let eval0 = create_ok(
        &mut client,
        ServerMessage::CreateEvaluatorRequest {
            request_id: 1,
            allowed_modules: Some(vec!["pkl:".into(), "repl:".into()]),
            allowed_resources: Some(vec!["cbres:".into()]),
            client_module_readers: None,
            client_resource_readers: Some(vec![ResourceReaderSpec {
                scheme: "cbres".into(),
                has_hierarchical_uris: true,
                is_globbable: false,
            }]),
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
            trace_mode: None,
        },
    );

    // Pipeline two evaluations on eval0: first blocks on cbres:/gate.txt.
    client.send(&ServerMessage::EvaluateRequest {
        request_id: 10,
        evaluator_id: eval0,
        module_uri: "repl:text".into(),
        module_text: Some("v = read(\"cbres:/gate.txt\").text".into()),
        expr: Some("v".into()),
    });
    client.send(&ServerMessage::EvaluateRequest {
        request_id: 11,
        evaluator_id: eval0,
        module_uri: "repl:text".into(),
        module_text: Some("v = \"second\"".into()),
        expr: Some("v".into()),
    });

    // Server sends ReadResourceRequest for cbres:/gate.txt.
    let cb_req_id = match client.recv().expect("ReadResourceRequest") {
        ServerMessage::ReadResourceRequest {
            request_id,
            evaluator_id,
            uri,
        } => {
            assert_eq!(evaluator_id, eval0);
            assert_eq!(uri, "cbres:/gate.txt");
            request_id
        }
        other => panic!("expected ReadResourceRequest, got {other:?}"),
    };

    // WHILE eval0 is blocked waiting for cb_req_id, create eval1!
    // Must complete immediately without deadlocking on the worker thread.
    let eval1 = create_ok(&mut client, minimal_create(2));
    assert_ne!(eval1, eval0);

    // Now unblock eval0's callback.
    client.send(&ServerMessage::ReadResourceResponse {
        request_id: cb_req_id,
        evaluator_id: eval0,
        contents: Some(b"unblocked".to_vec()),
        error: None,
    });

    // Receive FIFO responses: 10 then 11.
    match client.recv().expect("EvaluateResponse 10") {
        ServerMessage::EvaluateResponse {
            request_id: 10,
            result: Some(bytes),
            error: None,
            ..
        } => {
            assert_eq!(
                decode_pkl_binary(&bytes).unwrap(),
                DecodedValue::String("unblocked".into())
            );
        }
        other => panic!("expected req 10 first, got {other:?}"),
    }
    match client.recv().expect("EvaluateResponse 11") {
        ServerMessage::EvaluateResponse {
            request_id: 11,
            result: Some(bytes),
            error: None,
            ..
        } => {
            assert_eq!(
                decode_pkl_binary(&bytes).unwrap(),
                DecodedValue::String("second".into())
            );
        }
        other => panic!("expected req 11 second, got {other:?}"),
    }

    // Evaluate on eval1 as well.
    client.send(&ServerMessage::EvaluateRequest {
        request_id: 12,
        evaluator_id: eval1,
        module_uri: "repl:text".into(),
        module_text: Some("ok = 2 + 2".into()),
        expr: Some("ok".into()),
    });
    match client.recv().expect("EvaluateResponse 12") {
        ServerMessage::EvaluateResponse {
            request_id: 12,
            result: Some(bytes),
            error: None,
            ..
        } => {
            assert_eq!(decode_pkl_binary(&bytes).unwrap(), DecodedValue::Int(4));
        }
        other => panic!("unexpected response: {other:?}"),
    }

    client.send(&ServerMessage::CloseEvaluator {
        evaluator_id: eval0,
    });
    client.send(&ServerMessage::CloseEvaluator {
        evaluator_id: eval1,
    });
    assert!(client.close_and_join().is_ok());
}

#[test]
fn env_properties_isolation_and_project_local_deps() {
    let mut client = TestClient::spawn();

    let mut props_a = BTreeMap::new();
    props_a.insert("who".into(), "A".into());
    let mut env_a = BTreeMap::new();
    env_a.insert("GREET".into(), "hello".into());
    let mut req_a = minimal_create(1);
    if let ServerMessage::CreateEvaluatorRequest {
        allowed_resources,
        properties,
        env,
        ..
    } = &mut req_a
    {
        *allowed_resources = Some(vec!["prop:".into(), "env:".into()]);
        *properties = Some(props_a);
        *env = Some(env_a);
    }
    let eval_a = create_ok(&mut client, req_a);

    let mut props_b = BTreeMap::new();
    props_b.insert("who".into(), "B".into());
    let mut req_b = minimal_create(2);
    if let ServerMessage::CreateEvaluatorRequest { properties, .. } = &mut req_b {
        *properties = Some(props_b);
    }
    let eval_b = create_ok(&mut client, req_b);

    client.send(&ServerMessage::EvaluateRequest {
        request_id: 3,
        evaluator_id: eval_a,
        module_uri: "repl:text".into(),
        module_text: Some("r = List(read(\"prop:who\"), read(\"env:GREET\"))".into()),
        expr: Some("r".into()),
    });
    match client.recv().expect("resp A") {
        ServerMessage::EvaluateResponse {
            request_id: 3,
            result: Some(bytes),
            error: None,
            ..
        } => {
            assert_eq!(
                decode_pkl_binary(&bytes).unwrap(),
                DecodedValue::List(vec![
                    DecodedValue::String("A".into()),
                    DecodedValue::String("hello".into()),
                ])
            );
        }
        other => panic!("unexpected resp A: {other:?}"),
    }

    client.send(&ServerMessage::EvaluateRequest {
        request_id: 4,
        evaluator_id: eval_b,
        module_uri: "repl:text".into(),
        module_text: Some("r = read(\"prop:who\")".into()),
        expr: Some("r".into()),
    });
    match client.recv().expect("resp B") {
        ServerMessage::EvaluateResponse {
            request_id: 4,
            result: Some(bytes),
            error: None,
            ..
        } => {
            assert_eq!(
                decode_pkl_binary(&bytes).unwrap(),
                DecodedValue::String("B".into())
            );
        }
        other => panic!("unexpected resp B: {other:?}"),
    }

    // Local project dependency test using existing fixture dir under tests/fixtures/m5/server/proj
    let proj_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/m5/server/proj")
        .canonicalize()
        .expect("proj fixture dir");
    let proj_uri = format!("file://{}/PklProject", proj_dir.display());
    let dep_uri = format!("file://{}/local_dep/PklProject", proj_dir.display());
    let main_uri = format!("file://{}/main.pkl", proj_dir.display());
    let mut deps = BTreeMap::new();
    deps.insert(
        "localDep".into(),
        ServerDependency::Local(ServerProject {
            project_file_uri: dep_uri,
            package_uri: Some("package://example.com/localDep@1.0.0".into()),
            dependencies: BTreeMap::new(),
        }),
    );
    let eval_proj = create_ok(
        &mut client,
        ServerMessage::CreateEvaluatorRequest {
            request_id: 5,
            allowed_modules: Some(vec![
                "pkl:".into(),
                "repl:".into(),
                "file:".into(),
                "projectpackage:".into(),
            ]),
            allowed_resources: Some(vec!["prop:".into(), "file:".into()]),
            client_module_readers: None,
            client_resource_readers: None,
            module_paths: None,
            env: None,
            properties: None,
            timeout_seconds: None,
            root_dir: None,
            cache_dir: None,
            output_format: None,
            project: Some(ServerProject {
                project_file_uri: proj_uri,
                package_uri: None,
                dependencies: deps,
            }),
            http: None,
            external_module_readers: None,
            external_resource_readers: None,
            trace_mode: None,
        },
    );
    client.send(&ServerMessage::EvaluateRequest {
        request_id: 6,
        evaluator_id: eval_proj,
        module_uri: main_uri,
        module_text: None,
        expr: Some("value".into()),
    });
    match client.recv().expect("resp proj") {
        ServerMessage::EvaluateResponse {
            request_id: 6,
            result: Some(bytes),
            error: None,
            ..
        } => {
            let val = decode_pkl_binary(&bytes).unwrap();
            assert!(!matches!(val, DecodedValue::Null), "got: {val:?}");
        }
        other => panic!("unexpected resp proj: {other:?}"),
    }

    assert!(client.close_and_join().is_ok());
}

#[test]
fn timeout_seconds_triggers_exact_timeout_error() {
    let mut client = TestClient::spawn();
    let mut req = minimal_create(1);
    if let ServerMessage::CreateEvaluatorRequest {
        timeout_seconds, ..
    } = &mut req
    {
        *timeout_seconds = Some(1);
    }
    let eval_id = create_ok(&mut client, req);

    client.send(&ServerMessage::EvaluateRequest {
        request_id: 2,
        evaluator_id: eval_id,
        module_uri: "repl:text".into(),
        module_text: Some(
            "function busy(n) = if (n <= 0) 0 else busy(n - 1) + busy(n - 2)\nx = busy(40)\n"
                .into(),
        ),
        expr: None,
    });
    match client.recv().expect("timeout response") {
        ServerMessage::EvaluateResponse {
            request_id: 2,
            evaluator_id,
            result: None,
            error: Some(err),
        } => {
            assert_eq!(evaluator_id, eval_id);
            assert_eq!(err, "Evaluation timed out after 1 second(s).");
        }
        other => panic!("expected timeout response, got {other:?}"),
    }

    client.send(&ServerMessage::CloseEvaluator {
        evaluator_id: eval_id,
    });
    assert!(client.close_and_join().is_ok());
}

fn external_reader_fixture_bin() -> &'static Path {
    static BIN: OnceLock<PathBuf> = OnceLock::new();
    BIN.get_or_init(|| {
        let exe = std::env::current_exe().expect("current_exe");
        let deps_dir = exe.parent().expect("deps dir");
        let profile_dir = deps_dir.parent().expect("profile dir");
        let bin_name = if cfg!(windows) {
            "external_reader_fixture.exe"
        } else {
            "external_reader_fixture"
        };
        let candidate = profile_dir.join("examples").join(bin_name);
        if candidate.is_file() {
            return candidate;
        }
        let mut cmd = Command::new(env!("CARGO"));
        cmd.args([
            "build",
            "-p",
            "pkl-runtime",
            "--example",
            "external_reader_fixture",
            "--locked",
        ]);
        if profile_dir.file_name().is_some_and(|n| n == "release") {
            cmd.arg("--release");
        }
        let status = cmd.status().expect("cargo build external_reader_fixture");
        assert!(
            status.success(),
            "failed to build external_reader_fixture example"
        );
        assert!(
            candidate.is_file(),
            "built fixture missing at {}",
            candidate.display()
        );
        candidate
    })
    .as_path()
}

#[test]
fn synthetic_query_module_enforces_allowed_modules() {
    let mut client = TestClient::spawn();
    let eval_id = create_ok(
        &mut client,
        ServerMessage::CreateEvaluatorRequest {
            request_id: 1,
            // Intentionally omit `repl:` to verify `ModuleKeys.Synthetic.resolve`
            // enforces `checkResolveModule("repl:text")` for arbitrary `expr`.
            allowed_modules: Some(vec!["pkl:".into(), "cbmod:".into()]),
            allowed_resources: Some(vec!["prop:".into()]),
            client_module_readers: Some(vec![ModuleReaderSpec {
                scheme: "cbmod".into(),
                has_hierarchical_uris: true,
                is_local: true,
                is_globbable: false,
            }]),
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
            trace_mode: None,
        },
    );

    client.send(&ServerMessage::EvaluateRequest {
        request_id: 2,
        evaluator_id: eval_id,
        module_uri: "cbmod:/sample.pkl".into(),
        module_text: None,
        expr: Some("val".into()),
    });

    let err = loop {
        match client.recv().expect("server message") {
            ServerMessage::ReadModuleRequest {
                request_id,
                evaluator_id,
                uri,
            } => {
                assert_eq!(evaluator_id, eval_id);
                assert_eq!(uri, "cbmod:/sample.pkl");
                client.send(&ServerMessage::ReadModuleResponse {
                    request_id,
                    evaluator_id,
                    contents: Some("val = 42\n".into()),
                    error: None,
                });
            }
            ServerMessage::EvaluateResponse {
                request_id: 2,
                evaluator_id,
                result: None,
                error: Some(err),
            } => {
                assert_eq!(evaluator_id, eval_id);
                break err;
            }
            other => panic!("expected repl:text allowlist rejection, got {other:?}"),
        }
    };
    assert_eq!(
        err,
        "–– Pkl Error ––\nRefusing to load module `repl:text` because it does not match any entry in the module allowlist (`--allowed-modules`).\n"
    );

    // Default `expr: None` evaluates `"module"` directly without resolving `repl:text`.
    client.send(&ServerMessage::EvaluateRequest {
        request_id: 3,
        evaluator_id: eval_id,
        module_uri: "cbmod:/sample.pkl".into(),
        module_text: None,
        expr: None,
    });
    match client.recv().expect("EvaluateResponse 3") {
        ServerMessage::EvaluateResponse {
            request_id: 3,
            evaluator_id,
            result: Some(bytes),
            error: None,
        } => {
            assert_eq!(evaluator_id, eval_id);
            let decoded = decode_pkl_binary(&bytes).expect("decode module");
            assert!(matches!(decoded, DecodedValue::Object(_)));
        }
        other => panic!("expected default module eval to succeed, got {other:?}"),
    }

    client.send(&ServerMessage::CloseEvaluator {
        evaluator_id: eval_id,
    });
    assert!(client.close_and_join().is_ok());
}

#[test]
fn close_evaluator_sends_close_external_process_0x32() {
    let fixture = external_reader_fixture_bin();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let marker = std::env::temp_dir().join(format!(
        "pkl-server-close-0x32-{}-{nanos}.marker",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&marker);

    let mut ext_res = BTreeMap::new();
    ext_res.insert(
        "extres".into(),
        ServerExternalReader {
            executable: fixture.to_string_lossy().into_owned(),
            arguments: Some(vec![
                "--scheme".into(),
                "extres".into(),
                "--close-marker".into(),
                marker.to_string_lossy().into_owned(),
            ]),
            working_dir: None,
        },
    );

    let mut client = TestClient::spawn();
    let eval_id = create_ok(
        &mut client,
        ServerMessage::CreateEvaluatorRequest {
            request_id: 1,
            allowed_modules: Some(vec!["pkl:".into(), "repl:".into()]),
            allowed_resources: Some(vec!["extres:".into()]),
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
            external_resource_readers: Some(ext_res),
            trace_mode: None,
        },
    );

    client.send(&ServerMessage::EvaluateRequest {
        request_id: 2,
        evaluator_id: eval_id,
        module_uri: "repl:text".into(),
        module_text: Some("v = read(\"extres:/hello.txt\").text\n".into()),
        expr: Some("v".into()),
    });
    match client.recv().expect("EvaluateResponse 2") {
        ServerMessage::EvaluateResponse {
            request_id: 2,
            evaluator_id,
            result: Some(bytes),
            error: None,
        } => {
            assert_eq!(evaluator_id, eval_id);
            let val = decode_pkl_binary(&bytes).expect("decode");
            assert!(matches!(val, DecodedValue::String(ref s) if !s.is_empty()));
        }
        other => panic!("expected external resource read to succeed, got {other:?}"),
    }

    client.send(&ServerMessage::CloseEvaluator {
        evaluator_id: eval_id,
    });
    assert!(client.close_and_join().is_ok());

    let marker_contents = std::fs::read_to_string(&marker).unwrap_or_default();
    let _ = std::fs::remove_file(&marker);
    assert_eq!(
        marker_contents, "closed\n",
        "expected external reader to receive 0x32 CloseExternalProcess on CloseEvaluator"
    );
}
