//! Regression tests for M7 differential fuzzing fixes (#107) and automatic
//! persistent-evaluator tracing GC / uncached module alias bounding (#105).

use pkl_runtime::evaluation::frontend::{
    LosslessModuleSourceInput, ModuleEvalOptions, evaluate_module_output_with_hosts,
};
use pkl_runtime::evaluation::session::{SourceSession, SourceSessionOptions};
use pkl_runtime::host::HostPoll;
use pkl_runtime::modules::{
    ModuleCompletion, ModuleHostError, ModuleKey, ModuleKeyKind, ModuleRequest, ModuleRequestKind,
    ModuleSource, ModuleSourceSpec, ResolvedModule, ResolvedModuleKey, default_completion,
};
use pkl_runtime::source::SourceText;
use pkl_runtime::value::PklString;

fn eval_single(uri: &str, text: &str) -> pkl_runtime::evaluation::frontend::ModuleOutputResult {
    let options = ModuleEvalOptions::default();
    evaluate_module_output_with_hosts(
        LosslessModuleSourceInput {
            text: SourceText::from(text),
            uri: PklString::from(uri),
            inferred_name: PklString::from("test"),
        },
        &options,
        |request: &ModuleRequest| -> ModuleCompletion {
            if let Some(c) = default_completion(request) {
                return c;
            }
            ModuleCompletion::Source(Err(ModuleHostError::NotFound))
        },
        |_| None,
    )
}

#[test]
fn fuzz_reg_01_top_level_outer_error_message() {
    let res = eval_single("file:///reg_01.pkl", "fromOuter = outer.tag\n");
    assert_ne!(res.exit_code, 0);
    let stderr = String::from_utf8_lossy(&res.stderr);
    assert!(
        stderr.contains("Top-level scope does not have an outer scope."),
        "unexpected stderr:\n{stderr}"
    );
    assert!(
        !stderr.contains("No outer scope is available."),
        "stale error message:\n{stderr}"
    );
}

#[test]
fn fuzz_reg_02_predicate_implicit_method_not_found_message() {
    let src = "\
baseMap: Mapping<String, Dynamic> = new {
  [\"alpha\"] { v = 9 }
}
amendedMap = (baseMap) {
  [[startsWith(\"a\")]] {
    v = 100
  }
}
";
    let res = eval_single("file:///reg_02.pkl", src);
    assert_ne!(res.exit_code, 0);
    let stderr = String::from_utf8_lossy(&res.stderr);
    assert!(
        stderr.contains("Cannot find method `startsWith`.\n"),
        "expected implicit-receiver message, got:\n{stderr}"
    );
    assert!(
        !stderr.contains("in class `Dynamic`"),
        "unexpected explicit-receiver suffix:\n{stderr}"
    );
    assert!(
        !stderr.contains("Available methods in class `Dynamic`"),
        "unexpected available-methods listing:\n{stderr}"
    );
}

#[test]
fn fuzz_reg_03_parser_error_precedes_later_string_escape_error() {
    let src = "title = \"quotes \"and\" \\ slashes\"\n";
    let res = eval_single("file:///reg_03.pkl", src);
    assert_ne!(res.exit_code, 0);
    let stderr = String::from_utf8_lossy(&res.stderr);
    assert!(
        stderr.contains("Invalid property definition. Expected a type annotation, `=` or `{`."),
        "expected parser error before later string escape error, got:\n{stderr}"
    );
    assert!(
        !stderr.contains("Invalid character escape sequence"),
        "lexer error leaked ahead of parser error:\n{stderr}"
    );
    // Ensure a genuine string escape error is still reported where a string IS expected.
    let direct = eval_single("file:///reg_03_direct.pkl", "title = \"\\ slashes\"\n");
    assert_ne!(direct.exit_code, 0);
    let direct_stderr = String::from_utf8_lossy(&direct.stderr);
    assert!(
        direct_stderr.contains("Invalid character escape sequence `\\ `."),
        "expected direct string escape error, got:\n{direct_stderr}"
    );
}

#[test]
fn fuzz_reg_04_parser_error_precedes_later_unterminated_string() {
    let src = "name = \"xml<&>'\"chars\"\n";
    let res = eval_single("file:///reg_04.pkl", src);
    assert_ne!(res.exit_code, 0);
    let stderr = String::from_utf8_lossy(&res.stderr);
    assert!(
        stderr.contains("Invalid property definition. Expected a type annotation, `=` or `{`."),
        "expected parser error before later missing-quote error, got:\n{stderr}"
    );
    assert!(
        !stderr.contains("Missing `\"` delimiter."),
        "lexer error leaked ahead of parser error:\n{stderr}"
    );
    // Ensure a genuine unterminated string error is still reported where a string IS expected.
    let direct = eval_single("file:///reg_04_direct.pkl", "name = \"unterminated\n");
    assert_ne!(direct.exit_code, 0);
    let direct_stderr = String::from_utf8_lossy(&direct.stderr);
    assert!(
        direct_stderr.contains("Missing `\"` delimiter."),
        "expected direct missing delimiter error, got:\n{direct_stderr}"
    );
}

#[test]
fn fuzz_reg_05_list_constructor_preserves_element_order() {
    let src = "\
xs = List(1, 2, 3)
zipped = IntSeq(0, 4).step(2).toList().zip(List(\"a\", \"b\", \"c\"))
joined = List(\"first\", \"second\", \"third\").join(\",\")
taken = List(10, 20, 30, 40).take(2)
folded = List(1, 2, 3, 4).fold(\"\", (acc, x) -> acc + x.toString())
";
    let res = eval_single("file:///reg_05.pkl", src);
    assert_eq!(
        res.exit_code,
        0,
        "unexpected stderr:\n{}",
        String::from_utf8_lossy(&res.stderr)
    );
    let stdout = String::from_utf8_lossy(&res.stdout);
    let expected = "\
xs = List(1, 2, 3)
zipped = List(Pair(0, \"a\"), Pair(2, \"b\"), Pair(4, \"c\"))
joined = \"first,second,third\"
taken = List(10, 20)
folded = \"1234\"
";
    assert_eq!(stdout, expected);
}

#[test]
fn persistent_public_session_auto_gc_bounds_repeated_and_uncached_churn() {
    let mut session = SourceSession::module_evaluator(SourceSessionOptions::default())
        .with_pure_standard_host(true);

    let drive = |session: &mut SourceSession, source_map: &dyn Fn(&str) -> Option<String>| {
        loop {
            match session.poll(0, 10_000) {
                HostPoll::Yielded => {}
                HostPoll::Ready(res) => return res,
                HostPoll::Module { token, request } => {
                    let completion = if let Some(default) = default_completion(&request) {
                        default
                    } else {
                        match &request.kind {
                            ModuleRequestKind::SelectKey { id, uri, .. } => {
                                ModuleCompletion::KeySelected(Ok(ModuleKey {
                                    id: *id,
                                    uri: uri.clone(),
                                    kind: ModuleKeyKind::Host,
                                    handle: "uncached-test".into(),
                                }))
                            }
                            ModuleRequestKind::QueryCachePolicy { .. } => {
                                ModuleCompletion::CachePolicy(Ok(false))
                            }
                            ModuleRequestKind::QueryFragmentPaths { .. } => {
                                ModuleCompletion::FragmentPaths(Ok(false))
                            }
                            ModuleRequestKind::ResolveKey {
                                key,
                                resolved_key_id,
                                ..
                            }
                            | ModuleRequestKind::Resolve {
                                key,
                                resolved_key_id,
                                ..
                            } => ModuleCompletion::KeyResolved(Ok(ResolvedModuleKey {
                                id: *resolved_key_id,
                                key: key.clone(),
                                handle: "".into(),
                                module: ResolvedModule {
                                    original_uri: key.uri.clone(),
                                    resolved_uri: key.uri.clone(),
                                    source_uri: key.uri.clone(),
                                },
                            })),
                            ModuleRequestKind::LoadSource { resolved, .. } => {
                                let uri = resolved.source_uri.as_str().unwrap_or("");
                                match source_map(uri) {
                                    Some(text) => ModuleCompletion::Source(Ok(ModuleSource {
                                        text: SourceText::from(text),
                                        inferred_name: "mod".into(),
                                    })),
                                    None => {
                                        ModuleCompletion::Source(Err(ModuleHostError::NotFound))
                                    }
                                }
                            }
                            _ => ModuleCompletion::Source(Err(ModuleHostError::NotFound)),
                        }
                    };
                    session.complete_module(&token, completion, 0).unwrap();
                }
                other => panic!("unexpected host event: {other:?}"),
            }
        }
    };

    // 1. Repeated inline cyclic evaluations without ANY explicit session.collect() call.
    let mut after_warmup = None;
    for step in 0..80 {
        let src = format!(
            "local a {{ tag = \"a-{step}\"; peer = b }}\n\
             local b {{ tag = \"b-{step}\"; peer = a }}\n\
             result = a.peer.peer.tag\n"
        );
        let spec = ModuleSourceSpec {
            uri: "repl:text".into(),
            contents: Some(src.into()),
        };
        session.start_evaluation(spec, "result", 0, None).unwrap();
        let val = drive(&mut session, &|_| None).unwrap();
        assert_eq!(
            SourceSession::result_string(val).unwrap().to_java_utf8(),
            format!("a-{step}")
        );
        if step == 39 {
            after_warmup = Some(session.heap_stats());
        }
    }
    let after_cyclic = session.heap_stats();
    assert!(
        after_cyclic.collections > 0,
        "expected automatic GC collections during repeated evaluations, got {after_cyclic:?}"
    );
    let warmup_stats = after_warmup.unwrap();
    assert!(
        after_cyclic.slots <= warmup_stats.slots.saturating_mul(2).max(2048),
        "heap slots grew unboundedly without explicit collect: warmup={warmup_stats:?}, final={after_cyclic:?}"
    );

    // 2. Unique uncached module URI churn without ANY explicit session.collect() call.
    let mut uncached_warmup = None;
    for step in 0..64 {
        let uri = format!("uncached:///mod-{step}.pkl");
        let spec = ModuleSourceSpec {
            uri: uri.clone().into(),
            contents: None,
        };
        session.start_evaluation(spec, "checksum", 0, None).unwrap();
        let val = drive(&mut session, &|requested| {
            let n: usize = requested
                .strip_prefix("uncached:///mod-")?
                .strip_suffix(".pkl")?
                .parse()
                .ok()?;
            Some(format!("checksum = {} * 2 + 3\n", n))
        })
        .unwrap();
        assert_eq!(
            SourceSession::result_string(val).unwrap().to_java_utf8(),
            format!("{}", step * 2 + 3)
        );
        if step == 31 {
            uncached_warmup = Some(session.heap_stats());
        }
    }
    let after_uncached = session.heap_stats();
    let uncached_warmup = uncached_warmup.unwrap();
    assert!(
        after_uncached.live <= uncached_warmup.live + 512,
        "uncached URI churn leaked live objects: warmup={uncached_warmup:?}, final={after_uncached:?}"
    );

    session.close();
    assert_eq!(session.heap_stats().slots, 0);
}

#[test]
fn fast_pkl_binary_encode_hits_and_matches_render_run() {
    let src = "\
local rows = IntSeq(1, 200).map((i) -> new Dynamic {
  id = 9007199254740993 + i
  negId = -9007199254740993 - i
  label = \"row-\\(i)-한글-😀\"
  raw = Bytes(i % 256, (i * 3) % 256, 128, 255)
  active = i % 2 == 0
})

count = rows.length
maxBig = 9007199254740993 + 200
payload = Bytes(0, 1, 2, 127, 128, 254, 255)
items = rows

output {
  text = \"ok\"
}
";
    let mut session = SourceSession::module_evaluator(SourceSessionOptions::default())
        .with_pure_standard_host(true);
    let spec = ModuleSourceSpec {
        uri: "file:///js_boundary_payload.pkl".into(),
        contents: None,
    };

    let drive = |session: &mut SourceSession, budget: usize| -> Result<Vec<u8>, String> {
        let mut polls = 0usize;
        loop {
            polls += 1;
            match session.poll(0, budget) {
                HostPoll::Yielded => {
                    if polls > 2000 {
                        return Err(format!("exceeded poll cap with budget {budget}"));
                    }
                }
                HostPoll::Ready(Ok(val)) => {
                    return SourceSession::result_bytes(val)
                        .map(|b| b.to_vec())
                        .map_err(|e| format!("result_bytes error: {e:?}"));
                }
                HostPoll::Ready(Err(err)) => return Err(format!("eval error: {err:?}")),
                HostPoll::Module { token, request } => {
                    let completion = if let Some(default) = default_completion(&request) {
                        default
                    } else {
                        match &request.kind {
                            ModuleRequestKind::SelectKey { id, uri, .. } => {
                                ModuleCompletion::KeySelected(Ok(ModuleKey {
                                    id: *id,
                                    uri: uri.clone(),
                                    kind: ModuleKeyKind::Host,
                                    handle: "cached-test".into(),
                                }))
                            }
                            ModuleRequestKind::QueryCachePolicy { .. } => {
                                ModuleCompletion::CachePolicy(Ok(true))
                            }
                            ModuleRequestKind::QueryFragmentPaths { .. } => {
                                ModuleCompletion::FragmentPaths(Ok(false))
                            }
                            ModuleRequestKind::ResolveKey {
                                key,
                                resolved_key_id,
                                ..
                            }
                            | ModuleRequestKind::Resolve {
                                key,
                                resolved_key_id,
                                ..
                            } => ModuleCompletion::KeyResolved(Ok(ResolvedModuleKey {
                                id: *resolved_key_id,
                                key: key.clone(),
                                handle: "".into(),
                                module: ResolvedModule {
                                    original_uri: key.uri.clone(),
                                    resolved_uri: key.uri.clone(),
                                    source_uri: key.uri.clone(),
                                },
                            })),
                            ModuleRequestKind::LoadSource { .. } => {
                                ModuleCompletion::Source(Ok(ModuleSource {
                                    text: SourceText::from(src),
                                    inferred_name: "js_boundary_payload".into(),
                                }))
                            }
                            _ => ModuleCompletion::Source(Err(ModuleHostError::NotFound)),
                        }
                    };
                    session.complete_module(&token, completion, 0).unwrap();
                }
                other => return Err(format!("unexpected host poll: {other:?}")),
            }
        }
    };

    // Call 1: forces all lazy Dynamic properties through RenderRun.
    session
        .start_evaluation_pkl_binary(spec.clone(), "module", 0, None)
        .unwrap();
    let first_bytes = drive(&mut session, 10_000).unwrap();
    assert!(
        first_bytes.len() > 15_000,
        "unexpected len {}",
        first_bytes.len()
    );

    // Call 2: cached module + forced Dynamic rows MUST hit try_fast_encode_pkl_binary
    // with ZERO Yielded polls at a 64-step budget and match Call 1 byte-for-byte!
    session
        .start_evaluation_pkl_binary(spec, "module", 0, None)
        .unwrap();
    let mut yields = 0usize;
    let second_bytes = loop {
        match session.poll(0, 64) {
            HostPoll::Yielded => yields += 1,
            HostPoll::Ready(Ok(val)) => break SourceSession::result_bytes(val).unwrap().to_vec(),
            HostPoll::Module { token, request } => {
                let completion = if let Some(default) = default_completion(&request) {
                    default
                } else {
                    match &request.kind {
                        ModuleRequestKind::SelectKey { id, uri, .. } => {
                            ModuleCompletion::KeySelected(Ok(ModuleKey {
                                id: *id,
                                uri: uri.clone(),
                                kind: ModuleKeyKind::Host,
                                handle: "cached-test".into(),
                            }))
                        }
                        ModuleRequestKind::QueryCachePolicy { .. } => {
                            ModuleCompletion::CachePolicy(Ok(true))
                        }
                        ModuleRequestKind::QueryFragmentPaths { .. } => {
                            ModuleCompletion::FragmentPaths(Ok(false))
                        }
                        ModuleRequestKind::ResolveKey {
                            key,
                            resolved_key_id,
                            ..
                        }
                        | ModuleRequestKind::Resolve {
                            key,
                            resolved_key_id,
                            ..
                        } => ModuleCompletion::KeyResolved(Ok(ResolvedModuleKey {
                            id: *resolved_key_id,
                            key: key.clone(),
                            handle: "".into(),
                            module: ResolvedModule {
                                original_uri: key.uri.clone(),
                                resolved_uri: key.uri.clone(),
                                source_uri: key.uri.clone(),
                            },
                        })),
                        _ => ModuleCompletion::Source(Err(ModuleHostError::NotFound)),
                    }
                };
                session.complete_module(&token, completion, 0).unwrap();
            }
            other => panic!("unexpected poll: {other:?}"),
        }
    };
    assert_eq!(second_bytes, first_bytes);
    assert_eq!(yields, 0, "expected zero Yielded polls, got {yields}");

    // Call 3: columnar fast pack MUST succeed on warm-cached forced module with 8-byte alignment
    // and pack all 200 Dynamic rows via OP_COLUMNAR_LIST.
    let mut col_buf = pkl_runtime::vm::columnar_value::ColumnarValueBuffer::new();
    let uri = PklString::from("file:///js_boundary_payload.pkl");
    assert!(
        session.try_pack_warm_expression_columnar(&uri, "module", &mut col_buf),
        "expected columnar fast pack to hit for warm forced module"
    );
    let packed = col_buf.as_bytes();
    assert!(packed.len() >= 80, "packed too short: {}", packed.len());
    assert_eq!(
        (packed.as_ptr() as usize) & 7,
        0,
        "packed columnar buffer must be 8-byte aligned"
    );
    let magic = u32::from_le_bytes([packed[0], packed[1], packed[2], packed[3]]);
    assert_eq!(magic, pkl_runtime::vm::columnar_value::COLUMNAR_MAGIC);
    let bigs_len = u32::from_le_bytes([packed[16], packed[17], packed[18], packed[19]]);
    let bools_len = u32::from_le_bytes([packed[32], packed[33], packed[34], packed[35]]);
    let shape_count = u32::from_le_bytes([packed[68], packed[69], packed[70], packed[71]]);
    assert_eq!(bigs_len, 402, "expected 2 root + 400 row i64s");
    assert_eq!(bools_len, 200, "expected 200 columnar row bools");
    assert_eq!(shape_count, 2, "expected root Module + Dynamic row shapes");
    col_buf.clear_and_release();
    assert_eq!(col_buf.byte_len(), 0);
}
