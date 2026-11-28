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

#[test]
fn wave2_bind_local_int8_soundness_rejects_out_of_range() {
    let res = eval_single(
        "file:///wave2_int8.pkl",
        "res = List(1, 2, 999).map((x: Int8) -> x)\n",
    );
    assert_ne!(
        res.exit_code, 0,
        "Int8 lambda parameter must reject 999 after Int8-valid elements"
    );
    let stderr = String::from_utf8_lossy(&res.stderr);
    assert!(
        stderr.contains("Int8") || stderr.contains("constraint"),
        "expected Int8 constraint failure, got:\n{stderr}"
    );
}

#[test]
fn wave2_recursive_typealias_constraint_depth_limit() {
    let res = eval_single(
        "file:///wave2_rec_typealias.pkl",
        "typealias Boom = Int(this is Boom)\nx: Boom = 1\n",
    );
    assert_ne!(
        res.exit_code, 0,
        "recursive typealias constraint must fail cleanly"
    );
    let stderr = String::from_utf8_lossy(&res.stderr);
    assert!(
        stderr.contains("frame limit")
            || stderr.contains("stack overflow")
            || stderr.contains("cycle")
            || stderr.contains("recursion"),
        "unexpected stderr:\n{stderr}"
    );
}

#[test]
fn wave2_fast_int_deep_recursion_and_bool_return() {
    let res = eval_single(
        "file:///wave2_fast_int.pkl",
        "\
function sum(n: Int): Int = if (n <= 0) 0 else n + sum(n - 1)
function isZero(n: Int): Boolean = if (n <= 0) true else isZero(n - 1)
total = sum(500)
flag = isZero(10)
",
    );
    assert_eq!(
        res.exit_code,
        0,
        "expected clean evaluation, got:\n{}",
        String::from_utf8_lossy(&res.stderr)
    );
    let stdout = String::from_utf8_lossy(&res.stdout);
    assert!(
        stdout.contains("total = 125250"),
        "unexpected stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("flag = true"),
        "unexpected stdout:\n{stdout}"
    );
}

#[test]
fn wave2_java_regex_unicode_property_flag_scoping_and_long_quantifier() {
    let res = eval_single(
        "file:///wave2_regex.pkl",
        r##"
propOk = Regex(#"(?-U)\p{Alpha}"#).matchEntire("a") != null
flagScoped = Regex(#"(?:(?i)a)?b"#).matchEntire("B") == null
longMatch = Regex(".*").matchEntire("a".repeat(600)) != null
"##,
    );
    assert_eq!(
        res.exit_code,
        0,
        "expected clean evaluation, got:\n{}",
        String::from_utf8_lossy(&res.stderr)
    );
    let stdout = String::from_utf8_lossy(&res.stdout);
    assert!(
        stdout.contains("propOk = true"),
        "unexpected stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("flagScoped = true"),
        "unexpected stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("longMatch = true"),
        "unexpected stdout:\n{stdout}"
    );
}

#[test]
fn wave2_yaml_renderer_indent_and_merge_bomb_limits() {
    let neg = eval_single(
        "file:///wave2_yaml_neg.pkl",
        "res = new YamlRenderer { indentWidth = -1 }.renderValue(new Dynamic { a = 1 })\n",
    );
    assert_ne!(
        neg.exit_code, 0,
        "negative YAML indentWidth must error cleanly"
    );

    let huge = eval_single(
        "file:///wave2_yaml_huge.pkl",
        "res = new YamlRenderer { indentWidth = 10000000 }.renderValue(new Dynamic { a = 1 })\n",
    );
    assert_ne!(
        huge.exit_code, 0,
        "huge YAML indentWidth must error cleanly"
    );
    let huge_stderr = String::from_utf8_lossy(&huge.stderr);
    assert!(
        huge_stderr.contains("out of supported range"),
        "expected range error for huge YAML indentWidth, got:\n{huge_stderr}"
    );

    // Explicit `!!merge` bomb with non-scalar keys: 20 levels = 40 aliases (<= maxCollectionAliases=50)
    // would produce 4 * 2^20 = 4,194,304 merged entries without MAX_YAML_MERGE_ENTRIES.
    let mut bomb_yaml = String::from("a0: &a0 { [1]: 1, [2]: 2, [3]: 3, [4]: 4 }\n");
    for i in 1..=20 {
        bomb_yaml.push_str(&format!(
            "a{i}: &a{i}\n  !!merge <<: [*a{prev}, *a{prev}]\n",
            prev = i - 1
        ));
    }
    let src = format!(
        "import \"pkl:yaml\"\nres = new yaml.Parser {{}}.parse(#\"\"\"\n{bomb_yaml}\"\"\"#)\n"
    );
    let bomb = eval_single("file:///wave2_yaml_bomb.pkl", &src);
    assert_ne!(
        bomb.exit_code, 0,
        "YAML merge bomb must be rejected cleanly"
    );
    let bomb_stderr = String::from_utf8_lossy(&bomb.stderr);
    assert!(
        bomb_stderr.contains("merged mapping entries"),
        "expected merged mapping entries limit error, got:\n{bomb_stderr}"
    );
}

#[test]
fn wave2_pkl_binary_decode_depth_limit() {
    // 1200 nested Pair tags (`[0x93, 0x09, <inner>, 0x00]`) exceed DEFAULT_MAX_DECODE_DEPTH (1024).
    let depth = 1200usize;
    let mut payload = Vec::with_capacity(depth * 3 + 1);
    for _ in 0..depth {
        payload.push(0x93); // fixarray len 3
        payload.push(0x09); // tag Pair
    }
    payload.push(0xc0); // nil
    payload.resize(payload.len() + depth, 0xc0); // second element nil for each pair
    let err = pkl_runtime::pkl_binary_decode::decode_lossless(&payload)
        .expect_err("deeply nested pkl-binary payload must exceed max_depth");
    assert!(
        err.to_string().contains("depth"),
        "expected depth error, got: {err}"
    );
}

#[test]
fn wave2_resolver_nested_for_generator_is_linear() {
    // Previously `Work::ObjectBody(members, None)` re-walked earlier members on every nested
    // generator, taking O(2^D) resolver work for 22 nested `for` blocks.
    let mut src = String::from("res = new Dynamic {\n  p0 = 1\n");
    for i in 0..22 {
        src.push_str(&format!("  for (_ in List({i})) {{\n"));
    }
    src.push_str("  42\n");
    for _ in 0..22 {
        src.push_str("  }\n");
    }
    src.push_str("}\n");
    let res = eval_single("file:///wave2_for_linear.pkl", &src);
    assert_eq!(
        res.exit_code,
        0,
        "expected nested for-generators to resolve linearly, got:\n{}",
        String::from_utf8_lossy(&res.stderr)
    );
    assert!(
        String::from_utf8_lossy(&res.stdout).contains("42"),
        "expected element 42 in output"
    );
}

#[test]
fn round3_vm_get_implicit_method_this_value_and_list_repeat_soundness() {
    // 1. P0-10: GetImplicitMethod inside inherited/amended object preserves `this` receiver.
    let implicit_res = eval_single(
        "file:///round3_implicit_this.pkl",
        r#"
class Base {
  x: Int = 10
  function value(): Int = x
  y: Int = value()
}
child = new Base {
  x = 42
}
res = child.y
"#,
    );
    assert_eq!(
        implicit_res.exit_code,
        0,
        "stderr:\n{}",
        String::from_utf8_lossy(&implicit_res.stderr)
    );
    assert!(
        String::from_utf8_lossy(&implicit_res.stdout).contains("res = 42"),
        "stdout:\n{}",
        String::from_utf8_lossy(&implicit_res.stdout)
    );

    // 2. P1-6: List.repeat supports addition, equality, subscript, and typecheck accurately.
    let repeat_res = eval_single(
        "file:///round3_list_repeat.pkl",
        r#"
a: List<Int> = List(1, 2).repeat(3)
b: List<Int> = List(1, 2, 1, 2, 1, 2)
eq = a == b
sum = (a + List(9)).length
elem = a[5]
"#,
    );
    assert_eq!(
        repeat_res.exit_code,
        0,
        "stderr:\n{}",
        String::from_utf8_lossy(&repeat_res.stderr)
    );
    let out = String::from_utf8_lossy(&repeat_res.stdout);
    assert!(out.contains("eq = true"), "{out}");
    assert!(out.contains("sum = 7"), "{out}");
    assert!(out.contains("elem = 2"), "{out}");

    // 3. P1-6: Huge List.repeat rejects cleanly instead of OOMing.
    let huge_repeat = eval_single(
        "file:///round3_huge_repeat.pkl",
        "x = List(1).repeat(1000000000).toString()\n",
    );
    assert_ne!(
        huge_repeat.exit_code, 0,
        "huge List.repeat must fail cleanly"
    );
}

#[test]
fn round3_typecheck_union_alias_dag_and_regex_long_literal_bounds() {
    // 1. P1-5: Exponential typealias DAG is bounded cleanly without hang/OOM.
    let mut dag = String::from("typealias T0 = Int | String\n");
    for i in 1..=22 {
        dag.push_str(&format!(
            "typealias T{i} = T{prev} | T{prev}\n",
            prev = i - 1
        ));
    }
    dag.push_str("x: T22 = true\n");
    let dag_res = eval_single("file:///round3_type_dag.pkl", &dag);
    assert_ne!(
        dag_res.exit_code, 0,
        "type mismatch/depth/budget must fail cleanly"
    );

    // 2. P1-7 / P1-8: Long (>500 char) regex literal & replaceAll work without panic or false truncation.
    let long_lit = "a".repeat(600);
    let regex_src = format!(
        "matched = Regex(\"{long_lit}\").matchEntire(\"{long_lit}\") != null\n\
         replaced = \"x{long_lit}y\".replaceAll(Regex(\"{long_lit}\"), \"OK\")\n"
    );
    let rx_res = eval_single("file:///round3_long_regex.pkl", &regex_src);
    assert_eq!(
        rx_res.exit_code,
        0,
        "stderr:\n{}",
        String::from_utf8_lossy(&rx_res.stderr)
    );
    let out = String::from_utf8_lossy(&rx_res.stdout);
    assert!(out.contains("matched = true"), "{out}");
    assert!(out.contains("replaced = \"xOKy\""), "{out}");
}

#[test]
fn round4_cancel_during_module_init_does_not_poison_module_cache() {
    let mut session = SourceSession::module_evaluator(SourceSessionOptions::default())
        .with_reusable_timeouts(true);

    let slow_src = "function spin(n: Int): Int = if (n <= 1) n else spin(n - 1) + spin(n - 2)\n\
                    v = spin(36)\n";
    let fast_src = "v = 42\n";
    let uri = PklString::from("file:///round4_cancel_cache.pkl");
    let spec = ModuleSourceSpec {
        uri: uri.clone(),
        contents: None,
    };

    // Pass 1: start evaluating `slow_src` with a tiny timeout so it times out mid-module-init.
    session
        .start_evaluation_output_bytes(spec.clone(), 0, Some(5))
        .unwrap();
    let mut now = 0u64;
    let first_err = loop {
        now += 1;
        match session.poll(now, 64) {
            HostPoll::Yielded => {}
            HostPoll::Ready(Err(err)) => break err,
            HostPoll::Ready(Ok(_)) => panic!("expected timeout during slow module init"),
            HostPoll::Module { token, request } => {
                let comp = if let Some(d) = default_completion(&request) {
                    d
                } else {
                    match &request.kind {
                        ModuleRequestKind::SelectKey { id, uri, .. } => {
                            ModuleCompletion::KeySelected(Ok(ModuleKey {
                                id: *id,
                                uri: uri.clone(),
                                kind: ModuleKeyKind::Host,
                                handle: "test".into(),
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
                                text: SourceText::from(slow_src),
                                inferred_name: "round4_cancel_cache".into(),
                            }))
                        }
                        _ => ModuleCompletion::Source(Err(ModuleHostError::NotFound)),
                    }
                };
                session.complete_module(&token, comp, now).unwrap();
            }
            HostPoll::Read { token, .. } => {
                session
                    .complete_read(&token, Ok(pkl_runtime::resources::ReadResult::Missing), now)
                    .unwrap();
            }
            other => panic!("unexpected poll: {other:?}"),
        }
    };
    assert_eq!(first_err.kind, pkl_runtime::value::ErrorKind::Timeout);

    // Pass 2: re-evaluate the SAME URI on the SAME session (now fast) — module cache MUST NOT
    // be poisoned by the earlier timeout/cancellation.
    session
        .start_evaluation_output_bytes(spec, now + 10, Some(5_000))
        .unwrap();
    let second_out = loop {
        match session.poll(now + 10, 4096) {
            HostPoll::Yielded => {}
            HostPoll::Ready(Ok(val)) => {
                break String::from_utf8(SourceSession::result_bytes(val).unwrap().to_vec())
                    .unwrap();
            }
            HostPoll::Ready(Err(err)) => {
                panic!("subsequent eval after timeout must succeed, got: {err:?}")
            }
            HostPoll::Read { token, .. } => {
                session
                    .complete_read(
                        &token,
                        Ok(pkl_runtime::resources::ReadResult::Missing),
                        now + 10,
                    )
                    .unwrap();
            }
            HostPoll::Module { token, request } => {
                let comp = if let Some(d) = default_completion(&request) {
                    d
                } else {
                    match &request.kind {
                        ModuleRequestKind::SelectKey { id, uri, .. } => {
                            ModuleCompletion::KeySelected(Ok(ModuleKey {
                                id: *id,
                                uri: uri.clone(),
                                kind: ModuleKeyKind::Host,
                                handle: "test".into(),
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
                                text: SourceText::from(fast_src),
                                inferred_name: "round4_cancel_cache".into(),
                            }))
                        }
                        _ => ModuleCompletion::Source(Err(ModuleHostError::NotFound)),
                    }
                };
                session.complete_module(&token, comp, now + 10).unwrap();
            }
            other => panic!("unexpected poll: {other:?}"),
        }
    };
    assert_eq!(second_out, "v = 42\n");
}

#[test]
fn round4_shared_typecheck_union_dag_is_bounded() {
    let ok_src = "\
typealias O<T> = T | T | T | T | T | T | T | T
typealias B = O<O<O<O<O<O<O<O<O<O<O<O<Int>>>>>>>>>>>>
x: B = 1
";
    let ok_res = eval_single("file:///round4_union_dag_ok.pkl", ok_src);
    assert_eq!(
        ok_res.exit_code,
        0,
        "12-level shared union DAG must accept matching value without hang:\n{}",
        String::from_utf8_lossy(&ok_res.stderr)
    );
    assert!(String::from_utf8_lossy(&ok_res.stdout).contains("x = 1"));

    let fail_src = "\
typealias O<T> = T | T | T | T | T | T | T | T
typealias B = O<O<O<O<O<O<O<O<O<O<O<O<Int>>>>>>>>>>>>
x: B = \"nope\"
";
    let fail_res = eval_single("file:///round4_union_dag_fail.pkl", fail_src);
    assert_ne!(
        fail_res.exit_code, 0,
        "12-level shared union DAG mismatch must fail cleanly without OOM or hang"
    );
    assert!(
        fail_res.stderr.len() < 256 * 1024,
        "diagnostic output must remain bounded, got {} bytes",
        fail_res.stderr.len()
    );
}

#[test]
fn round4_list_repeat_typed_cast_and_structural_key_soundness() {
    // P1-9: repeated list element past base_len must still enforce generic typecheck.
    let bad = eval_single(
        "file:///round4_repeat_cast_bad.pkl",
        "xs: List<Listing<Int>> = List(new Listing { \"oops\" }).repeat(70000)\nres = xs[1][0]\n",
    );
    assert_ne!(
        bad.exit_code, 0,
        "List.repeat typed cast must not bypass typecheck past base_len"
    );

    let good = eval_single(
        "file:///round4_repeat_cast_ok.pkl",
        "xs: List<Listing<Int>> = List(new Listing { 7 }).repeat(70000)\nres = xs[69999][0]\n",
    );
    assert_eq!(
        good.exit_code,
        0,
        "stderr:\n{}",
        String::from_utf8_lossy(&good.stderr)
    );
    assert!(String::from_utf8_lossy(&good.stdout).contains("res = 7"));

    // P1-10: repeated list structural key in Set/Map must distinguish length and match equal sequences.
    let set_eq = eval_single(
        "file:///round4_repeat_set.pkl",
        "\
a = List(1, 2).repeat(40000)
b = List(1, 2)
c = List(1, 2, 1, 2).repeat(20000)
eqSame = Set(a) == Set(c)
eqDiff = Set(a) == Set(b)
",
    );
    assert_eq!(
        set_eq.exit_code,
        0,
        "stderr:\n{}",
        String::from_utf8_lossy(&set_eq.stderr)
    );
    let out = String::from_utf8_lossy(&set_eq.stdout);
    assert!(out.contains("eqSame = true"), "{out}");
    assert!(out.contains("eqDiff = false"), "{out}");
}

#[test]
fn round4_java_regex_nested_group_backref_and_supplementary_lookbehind() {
    let src = r##"
nested = Regex("(".repeat(64) + "a" + ")".repeat(64) + "b".repeat(1000)).matchEntire("a" + "b".repeat(1000)) != null
lbRaw = Regex("(?<=😀)x").findMatchesIn("😀x").length
lbHexJdk21 = Regex(#"(?<=\x{1F600})x"#).findMatchesIn("😀x").length
lbAfter = Regex("(?<=a)😀").findMatchesIn("a😀").length
"##;
    let res = eval_single("file:///round4_regex.pkl", src);
    assert_eq!(
        res.exit_code,
        0,
        "stderr:\n{}",
        String::from_utf8_lossy(&res.stderr)
    );
    let out = String::from_utf8_lossy(&res.stdout);
    assert!(out.contains("nested = true"), "{out}");
    assert!(out.contains("lbRaw = 1"), "{out}");
    assert!(out.contains("lbHexJdk21 = 0"), "{out}");
    assert!(out.contains("lbAfter = 1"), "{out}");

    // Backreference step-budget check must terminate promptly instead of hanging.
    let backref = eval_single(
        "file:///round4_regex_backref.pkl",
        r#"res = Regex("(?i)^(a+)\\1b$").matchEntire("a".repeat(2000) + "c") == null"#,
    );
    assert!(
        backref.exit_code == 0 || !backref.stderr.is_empty(),
        "backreference must either complete cleanly or trip step budget"
    );
}

#[test]
fn round4_yaml_merge_flatten_fingerprint_is_bounded() {
    let mut bomb_yaml = String::from("a0: &a0 { k0: 1, k1: 2 }\n");
    for i in 1..=26 {
        bomb_yaml.push_str(&format!(
            "a{i}: &a{i}\n  <<: [*a{prev}, *a{prev}]\n",
            prev = i - 1
        ));
    }
    let src = format!(
        "import \"pkl:yaml\"\nres = new yaml.Parser {{}}.parse(#\"\"\"\n{bomb_yaml}\"\"\"#)\n"
    );
    let res = eval_single("file:///round4_yaml_merge_bounded.pkl", &src);
    assert!(
        res.exit_code == 0 || !res.stderr.is_empty(),
        "deep duplicate-key YAML merge must remain bounded without OOM"
    );
}

#[test]
fn round5_generic_constrained_equivalence_and_typecheck_dag_budget() {
    // P0-4: generic Constrained typealiases with distinct type arguments MUST NOT be considered
    // equivalent and skip cast wrappers on Listing/Mapping.
    let bad = eval_single(
        "file:///round5_constrained_bad.pkl",
        "\
typealias Check<T> = Any(this is T)
xs: Listing<Check<Int>> = new Listing { 1 }
ys: Listing<Check<String>> = xs
res = ys[0]
",
    );
    assert_ne!(
        bad.exit_code, 0,
        "Listing<Check<Int>> must NOT pass unchecked as Listing<Check<String>>"
    );

    let ok = eval_single(
        "file:///round5_constrained_ok.pkl",
        "\
typealias Check<T> = Any(this is T)
xs: Listing<Check<Int>> = new Listing { 7 }
ys: Listing<Check<Int>> = xs
res = ys[0]
",
    );
    assert_eq!(
        ok.exit_code,
        0,
        "stderr:\n{}",
        String::from_utf8_lossy(&ok.stderr)
    );
    assert!(String::from_utf8_lossy(&ok.stdout).contains("res = 7"));

    // P1-7: Shared union DAG inside List<...> completes quickly under fast-typecheck node budget.
    let dag_ok = eval_single(
        "file:///round5_fan_ok.pkl",
        "\
typealias Fan<T> = T | T | T | T | T | T
typealias Big = Fan<Fan<Fan<Fan<Fan<Int>>>>>
xs: List<Big> = List(1, 2, 3)
res = xs.length
",
    );
    assert_eq!(
        dag_ok.exit_code,
        0,
        "stderr:\n{}",
        String::from_utf8_lossy(&dag_ok.stderr)
    );
    assert!(String::from_utf8_lossy(&dag_ok.stdout).contains("res = 3"));

    let dag_bad = eval_single(
        "file:///round5_fan_bad.pkl",
        "\
typealias Fan<T> = T | T | T | T | T | T
typealias Big = Fan<Fan<Fan<Fan<Fan<Int>>>>>
xs: List<Big> = List(\"bad\")
res = xs.length
",
    );
    assert_ne!(
        dag_bad.exit_code, 0,
        "shared union DAG mismatch must fail cleanly"
    );
}

#[test]
fn round5_list_repeat_chain_equality_and_string_chars_oom_guards() {
    // P1-10: Chained repeat and equality on large virtual repeated Lists must not OOM.
    let rep = eval_single(
        "file:///round5_repeat_chain.pkl",
        "\
local a = List(1, 2).repeat(1000000)
local b = a.repeat(2)
local c = List(1, 2, 1, 2).repeat(500000)
local d = List(1, 3).repeat(1000000)
lenB = b.length
eqSame = a == c
eqDiff = a == d
",
    );
    assert_eq!(
        rep.exit_code,
        0,
        "stderr:\n{}",
        String::from_utf8_lossy(&rep.stderr)
    );
    let out = String::from_utf8_lossy(&rep.stdout);
    assert!(out.contains("lenB = 4000000"), "{out}");
    assert!(out.contains("eqSame = true"), "{out}");
    assert!(out.contains("eqDiff = false"), "{out}");

    // P1-11: Huge String.chars / String.codePoints fail cleanly instead of OOM panicking.
    let huge_chars = eval_single(
        "file:///round5_huge_chars.pkl",
        "res = \"a\".repeat(3000000).chars.length\n",
    );
    assert_ne!(
        huge_chars.exit_code, 0,
        "huge String.chars must fail cleanly instead of OOM"
    );
    let huge_cps = eval_single(
        "file:///round5_huge_cps.pkl",
        "res = \"a\".repeat(3000000).codePoints.length\n",
    );
    assert_ne!(
        huge_cps.exit_code, 0,
        "huge String.codePoints must fail cleanly instead of OOM"
    );
}

#[test]
fn round5_java_regex_inline_flags_zero_width_quant_and_interpolation_undo() {
    // P1-13 & P1-14: Inline flag restoration in possessive group + zero-width min quantifier.
    let rx = eval_single(
        "file:///round5_regex.pkl",
        r#"
noLeak = Regex("(?i:a)++b").matchEntire("aB") == null
matchExact = Regex("(?i:a)++b").matchEntire("Ab") != null
zwMin = Regex("(^|a){2}$").matchEntire("a") != null
zwCap = Regex("(|a){2}b").matchEntire("ab").groups[1].value
"#,
    );
    assert_eq!(
        rx.exit_code,
        0,
        "stderr:\n{}",
        String::from_utf8_lossy(&rx.stderr)
    );
    let out = String::from_utf8_lossy(&rx.stdout);
    assert!(out.contains("noLeak = true"), "{out}");
    assert!(out.contains("matchExact = true"), "{out}");
    assert!(out.contains("zwMin = true"), "{out}");
    assert!(out.contains("zwCap = \"a\""), "{out}");

    // P1-15: String interpolation inside object body tracks entry counts cleanly.
    let interp = eval_single(
        "file:///round5_interp.pkl",
        "obj = new Dynamic { a = \"\\(new Listing { 1; 2 }.length)\"; 10; 20 }\nres = obj.a\n",
    );
    assert_eq!(
        interp.exit_code,
        0,
        "stderr:\n{}",
        String::from_utf8_lossy(&interp.stderr)
    );
    assert!(String::from_utf8_lossy(&interp.stdout).contains("res = \"2\""));
}
