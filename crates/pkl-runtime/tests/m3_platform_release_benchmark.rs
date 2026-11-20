//! M3-02b regression & parity tests for `pkl:platform`, `pkl:release`, and `pkl:Benchmark`.
use pkl_runtime::{
    evaluation::{
        EvaluationOptions,
        frontend::{ModuleEvalOptions, ModuleSourceInput},
        session::{SourceSession, SourceSessionOptions},
    },
    host::HostPoll,
    logging::PlatformMetadata,
    modules::{ModuleRequestKind, default_completion},
    value::{ErrorKind, RuntimeError, Value},
};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

fn compile_session(
    source: &str,
    paths: &[&str],
    platform_metadata: Option<PlatformMetadata>,
) -> SourceSession {
    let mut session = SourceSession::compile(
        source,
        "file:///m3-platform-bench/main.pkl",
        "main",
        &paths.iter().map(|p| (*p).to_owned()).collect::<Vec<_>>(),
        SourceSessionOptions {
            evaluation: EvaluationOptions {
                power_assertions: false,
            },
            ..Default::default()
        },
    )
    .unwrap();
    session.set_platform_metadata(platform_metadata);
    session
}

fn eval_path(
    session: &mut SourceSession,
    index: usize,
    budget: usize,
    collect_each_yield: bool,
) -> Result<Value, RuntimeError> {
    session.start_path(index, 0, None).unwrap();
    for _ in 0..2_000_000 {
        match session.poll(0, budget) {
            HostPoll::Yielded => {
                if collect_each_yield {
                    session.collect(&[]);
                }
            }
            HostPoll::Module { token, request } => {
                assert!(
                    !matches!(request.kind, ModuleRequestKind::LoadSource { .. }),
                    "unexpected Host LoadSource: {request:?}"
                );
                let completion = default_completion(&request).expect("bundled stdlib completion");
                session.complete_module(&token, completion, 0).unwrap();
                if collect_each_yield {
                    session.collect(&[]);
                }
            }
            HostPoll::Read { token, .. } => {
                session.resume_default_read(&token, &[], &[], 0).unwrap();
            }
            HostPoll::Ready(result) => return result,
            other => panic!("unexpected Host event: {other:?}"),
        }
    }
    panic!("evaluation did not settle");
}

fn eval_output_text(
    source: &str,
    platform_metadata: Option<PlatformMetadata>,
) -> Result<String, RuntimeError> {
    let mut session = SourceSession::compile_output(
        ModuleSourceInput {
            text: source,
            uri: "file:///m3-platform-bench/main.pkl",
            inferred_name: "main",
        },
        &[],
        &ModuleEvalOptions::default(),
        SourceSessionOptions::default(),
    )
    .unwrap();
    session.set_platform_metadata(platform_metadata);
    session.start_output_bytes(0, None).unwrap();
    let result = loop {
        match session.poll(0, 512) {
            HostPoll::Yielded => {
                session.collect(&[]);
            }
            HostPoll::Module { token, request } => {
                let completion = default_completion(&request).expect("bundled stdlib completion");
                session.complete_module(&token, completion, 0).unwrap();
            }
            HostPoll::Read { token, .. } => {
                session.resume_default_read(&token, &[], &[], 0).unwrap();
            }
            HostPoll::Ready(result) => break result,
            other => panic!("unexpected Host event: {other:?}"),
        }
    };
    let text = result.map(|v| match v {
        Value::Bytes(b) => String::from_utf8(b.to_vec()).expect("UTF-8 output bytes"),
        other => panic!("expected Bytes output.bytes, got {other:?}"),
    });
    session.close();
    assert_eq!(session.collect(&[]).slots, 0);
    text
}

fn assert_str(value: Value, expected: &str) {
    let Value::String(actual) = value else {
        panic!("expected String, got {value:?}");
    };
    assert_eq!(actual.to_java_utf8(), expected);
}

#[test]
fn release_current_matches_pinned_0_32_0_and_imports_semver_cooperatively() {
    // Deliberately do NOT import `pkl:semver` in user source, and poll with
    // budget=1 + GC on every yield to prove cooperative `pkl:semver` loading.
    let source = r#"import "pkl:release"
local c = release.current
versionStr = c.version.toString()
isNormal = c.version.isNormal()
commitId = c.commitId
versionInfo = c.versionInfo
srcHome = c.sourceCode.homepage
docHome = c.documentation.homepage
modCount = c.standardLibrary.modules.length
hasCommand = c.standardLibrary.modules.contains("pkl:Command")
hasSettings = c.standardLibrary.modules.contains("pkl:settings")
hasDocPkg = c.standardLibrary.modules.contains("pkl:DocPackageInfo")
hasDocSite = c.standardLibrary.modules.contains("pkl:DocsiteInfo")
noPkgInfo = !c.standardLibrary.modules.contains("pkl:doc-package-info")
"#;
    let meta = PlatformMetadata {
        os_name: "macOS".into(),
        os_version: "27.2".into(),
        cpu_architecture: "aarch64".into(),
        java_version: "21.0.13.1".into(),
    };
    let mut session = compile_session(
        source,
        &[
            "versionStr",
            "isNormal",
            "commitId",
            "versionInfo",
            "srcHome",
            "docHome",
            "modCount",
            "hasCommand",
            "hasSettings",
            "hasDocPkg",
            "hasDocSite",
            "noPkgInfo",
        ],
        Some(meta),
    );
    // Single-step budget + GC on first query that triggers cooperative `pkl:semver` load.
    assert_str(eval_path(&mut session, 0, 1, true).unwrap(), "0.32.0");
    assert_str(eval_path(&mut session, 1, 256, true).unwrap(), "true");
    assert_str(eval_path(&mut session, 2, 256, true).unwrap(), "e51fb2f");
    assert_str(
        eval_path(&mut session, 3, 256, true).unwrap(),
        "Pkl 0.32.0 (macOS 27.2, Java 21.0.13.1)",
    );
    assert_str(
        eval_path(&mut session, 4, 256, true).unwrap(),
        "https://github.com/apple/pkl/",
    );
    assert_str(
        eval_path(&mut session, 5, 256, true).unwrap(),
        "https://pkl-lang.org/main/0.32.0/",
    );
    assert_str(eval_path(&mut session, 6, 256, true).unwrap(), "23");
    assert_str(eval_path(&mut session, 7, 256, true).unwrap(), "true");
    assert_str(eval_path(&mut session, 8, 256, true).unwrap(), "true");
    assert_str(eval_path(&mut session, 9, 256, true).unwrap(), "true");
    assert_str(eval_path(&mut session, 10, 256, true).unwrap(), "true");
    assert_str(eval_path(&mut session, 11, 256, true).unwrap(), "true");
    session.close();
    assert_eq!(session.collect(&[]).slots, 0);

    // Native flavor when java_version is "native" or empty.
    let native_meta = PlatformMetadata {
        os_name: "Linux".into(),
        os_version: "6.8".into(),
        cpu_architecture: "x86_64".into(),
        java_version: "native".into(),
    };
    let mut native_session = compile_session(
        r#"import "pkl:release"; info = release.current.versionInfo"#,
        &["info"],
        Some(native_meta),
    );
    assert_str(
        eval_path(&mut native_session, 0, 256, true).unwrap(),
        "Pkl 0.32.0 (Linux 6.8, native)",
    );
    native_session.close();
    assert_eq!(native_session.collect(&[]).slots, 0);
}

#[test]
fn platform_current_and_project_resolved_settings_respect_host_metadata() {
    let source = r#"import "pkl:platform"
import "pkl:Project"
local c = platform.current
langVer = c.language.version
osName = c.operatingSystem.name
osVer = c.operatingSystem.version
arch = c.processor.architecture
rtNonEmpty = c.runtime.name != "" && c.runtime.version != ""
vmNonEmpty = c.virtualMachine.name != "" && c.virtualMachine.version != ""
local p = new Project {
  projectFileUri = "file:///C:/work/proj/PklProject"
  evaluatorSettings { rootDir = "../shared" }
}
resolvedRoot = p.resolvedEvaluatorSettings.rootDir
"#;
    let windows_meta = PlatformMetadata {
        os_name: "Windows".into(),
        os_version: "11".into(),
        cpu_architecture: "x86_64".into(),
        java_version: "21.0.13.1".into(),
    };
    let mut session = compile_session(
        source,
        &[
            "langVer",
            "osName",
            "osVer",
            "arch",
            "rtNonEmpty",
            "vmNonEmpty",
            "resolvedRoot",
        ],
        Some(windows_meta),
    );
    assert_str(eval_path(&mut session, 0, 256, true).unwrap(), "0.32.0");
    assert_str(eval_path(&mut session, 1, 256, true).unwrap(), "Windows");
    assert_str(eval_path(&mut session, 2, 256, true).unwrap(), "11");
    assert_str(eval_path(&mut session, 3, 256, true).unwrap(), "x86_64");
    assert_str(eval_path(&mut session, 4, 256, true).unwrap(), "true");
    assert_str(eval_path(&mut session, 5, 256, true).unwrap(), "true");
    assert_str(
        eval_path(&mut session, 6, 256, true).unwrap(),
        r"C:\work\shared",
    );
    session.close();
    assert_eq!(session.collect(&[]).slots, 0);
}

#[test]
fn microbenchmark_preserves_lexical_scope_lazy_defaults_and_rejects_constants() {
    let source = r#"amends "pkl:Benchmark"
local baseVal = 40
iterations = 3
iterationTime = 1.ms
isVerbose = true

microbenchmarks {
  ["scoped"] {
    expression = baseVal + IntSeq(1, 5).fold(0, (acc, n) -> acc + n)
  }
}
"#;
    let meta = Some(PlatformMetadata::default());
    let rendered = eval_output_text(source, meta.clone()).unwrap();
    assert!(rendered.contains("[\"scoped\"]"), "rendered:\n{rendered}");
    assert!(rendered.contains("iterations = 3"), "rendered:\n{rendered}");
    assert!(
        rendered.contains("samples = List("),
        "rendered:\n{rendered}"
    );

    // Constant expressions must fail with `Microbenchmark is a constant expression.`
    for expr in [
        "42",
        "\"hi\"",
        "true",
        "null",
        "List()",
        "List(1, 2)",
        "Set(1, 2)",
        "Map(\"a\", 1)",
        "Bytes()",
        "Bytes(1, 2)",
    ] {
        let bad = format!(
            "amends \"pkl:Benchmark\"\niterations = 2\niterationTime = 1.ms\nmicrobenchmarks {{ [\"c\"] {{ expression = {expr} }} }}\n"
        );
        let err = eval_output_text(&bad, meta.clone()).unwrap_err();
        assert_eq!(err.kind, ErrorKind::Type, "expr {expr}");
        assert_eq!(
            err.message.to_java_utf8(),
            "Microbenchmark is a constant expression.",
            "expr {expr}"
        );
    }

    // Non-constant List(...) argument must be accepted and executed.
    let non_const_list = "amends \"pkl:Benchmark\"\niterations = 2\niterationTime = 1.ms\nmicrobenchmarks { [\"nc\"] { expression = List(1 + 2) } }\n";
    assert!(
        eval_output_text(non_const_list, meta.clone())
            .unwrap()
            .contains("[\"nc\"]")
    );

    // Missing expression fails as undefined property (matching Java).
    let missing = "amends \"pkl:Benchmark\"\niterations = 2\niterationTime = 1.ms\nmicrobenchmarks { [\"u\"] {} }\n";
    let err = eval_output_text(missing, meta.clone()).unwrap_err();
    assert_eq!(err.kind, ErrorKind::MissingMember);
    assert!(
        err.message
            .to_java_utf8()
            .contains("Tried to read property `expression` but its value is undefined."),
        "unexpected error: {}",
        err.message.to_java_utf8()
    );

    // `iterations = 1` yields NaN stdev and error (matching BenchmarkUtils.java:74).
    let single_iter = r#"import "pkl:Benchmark"
local r = new Benchmark.Microbenchmark {
  expression = 1 + 2
  iterations = 1
  iterationTime = 1.ms
  isVerbose = false
}.run()
stdevNaN = r.stdev.value.isNaN
errorNaN = r.error.value.isNaN
"#;
    let mut session = compile_session(single_iter, &["stdevNaN", "errorNaN"], meta);
    assert_str(eval_path(&mut session, 0, 256, true).unwrap(), "true");
    assert_str(eval_path(&mut session, 1, 256, true).unwrap(), "true");
    session.close();
    assert_eq!(session.collect(&[]).slots, 0);
}

#[test]
fn output_and_parser_benchmarks_and_full_benchmark_module_work_under_gc() {
    let source = r#"import "pkl:Benchmark"

local outputBench = new Benchmark.OutputBenchmark {
  sourceModule = import("pkl:shell")
  iterations = 3
  iterationTime = 1.ms
  isVerbose = true
}
local outRes = outputBench.run()
outOk = outRes.iterations == 3
  && outRes.repetitions >= 1
  && outRes.samples != null
  && outRes.samples.length == 3
  && outRes.min == outRes.samples.min
  && outRes.max == outRes.samples.max
  && outRes.mean >= outRes.min
  && outRes.mean <= outRes.max

local parserBench = new Benchmark.ParserBenchmark {
  sourceText = "a = throw(\"lazy not forced\")"
  iterations = 3
  iterationTime = 1.ms
  isVerbose = false
}
local parseRes = parserBench.run()
parseOk = parseRes.iterations == 3
  && parseRes.repetitions >= 1
  && parseRes.samples == null
  && parseRes.min > 0.s
  && parseRes.max >= parseRes.min

local full = new Benchmark {
  iterations = 2
  iterationTime = 1.ms
  microbenchmarks {
    ["micro"] { expression = IntSeq(1, 5).fold(0, (a, b) -> a + b) }
  }
  outputBenchmarks {
    ["output"] { sourceModule = import("pkl:shell") }
  }
  parserBenchmarks {
    ["parser"] { sourceText = "x = 1 + 2" }
  }
}
local fullText = full.output.text
fullOk = fullText.contains("[\"micro\"]")
  && fullText.contains("[\"output\"]")
  && fullText.contains("[\"parser\"]")
"#;
    let mut session = compile_session(
        source,
        &["outOk", "parseOk", "fullOk"],
        Some(PlatformMetadata::default()),
    );
    assert_str(eval_path(&mut session, 0, 256, true).unwrap(), "true");
    assert_str(eval_path(&mut session, 1, 256, true).unwrap(), "true");
    assert_str(eval_path(&mut session, 2, 256, true).unwrap(), "true");
    session.close();
    assert_eq!(session.collect(&[]).slots, 0);

    // ParserBenchmark must propagate syntax errors (not silently discard them).
    let bad_parse = r#"import "pkl:Benchmark"
res = new Benchmark.ParserBenchmark {
  sourceText = "a = ("
  iterations = 2
  iterationTime = 1.ms
  isVerbose = false
}.run()
"#;
    let mut bad_session = compile_session(bad_parse, &["res"], Some(PlatformMetadata::default()));
    let err = eval_path(&mut bad_session, 0, 256, true).unwrap_err();
    assert_eq!(err.kind, ErrorKind::Type);
    assert!(
        err.message
            .to_java_utf8()
            .contains("Unexpected end of file"),
        "unexpected error: {}",
        err.message.to_java_utf8()
    );
    bad_session.close();
    assert_eq!(bad_session.collect(&[]).slots, 0);
}

#[test]
fn benchmark_respects_injected_monotonic_clock() {
    let source = r#"import "pkl:Benchmark"
local r = new Benchmark.Microbenchmark {
  expression = 1 + 2
  iterations = 2
  iterationTime = 1.ms
  isVerbose = true
}.run()
ok = r.iterations == 2 && r.repetitions == 2 && r.samples.length == 2 && r.min == 500.us
"#;
    let parsed = pkl_runtime::parser::parse_module(source).unwrap();
    let resolved = pkl_runtime::resolver::resolve_module(&parsed).unwrap();
    let mut program = pkl_runtime::vm::compile_module(&resolved).unwrap();
    let runtime_source = Arc::new(pkl_runtime::value::RuntimeSource {
        text: parsed.source.clone(),
        uri: "file:///m3-platform-bench/clock.pkl".into(),
        module_name: "clock".into(),
    });
    Arc::make_mut(&mut program)
        .register_primary_source(
            runtime_source.clone(),
            pkl_runtime::modules::ModuleIdentity {
                original_uri: "file:///m3-platform-bench/clock.pkl".into(),
                resolved_uri: "file:///m3-platform-bench/clock.pkl".into(),
            },
        )
        .unwrap();
    let mut vm = pkl_runtime::vm::Vm::with_context(
        program.clone(),
        pkl_runtime::logging::EvaluationContext {
            primary_source: Some(runtime_source),
            platform_metadata: Some(PlatformMetadata::default()),
            ..Default::default()
        },
    );
    // Advance clock by 1_000_000 ns (1ms) on each read:
    // warmup iter 0: start=1ms, after 1 rep now=2ms (elapsed=1ms >= 666us) -> next reps=2.
    // warmup iter 1: start=3ms, after 1 rep now=4ms (elapsed=1ms >= 666us) -> next reps=2.
    // measure iter 0: start=5ms, after 2 reps now=6ms (elapsed=1ms / 2 = 500us).
    // measure iter 1: start=7ms, after 2 reps now=8ms (elapsed=1ms / 2 = 500us).
    let tick = Arc::new(AtomicU64::new(0));
    let clock_tick = tick.clone();
    vm.set_monotonic_clock(Arc::new(move || {
        u128::from(clock_tick.fetch_add(1, Ordering::Relaxed) + 1) * 1_000_000
    }));
    vm.start(program.entry, vec![]).unwrap();
    let module = loop {
        match vm.poll(64) {
            pkl_runtime::vm::VmPoll::Yielded => {
                vm.collect(&[]);
            }
            pkl_runtime::vm::VmPoll::Module(req) => {
                vm.complete_module(req.id, default_completion(&req).unwrap())
                    .unwrap();
            }
            pkl_runtime::vm::VmPoll::Ready(val) => break val.unwrap().as_object().unwrap(),
            other => panic!("unexpected VM event: {other:?}"),
        }
    };
    vm.start_member(module, "ok").unwrap();
    let ok = loop {
        match vm.poll(64) {
            pkl_runtime::vm::VmPoll::Yielded => {
                vm.collect(&[]);
            }
            pkl_runtime::vm::VmPoll::Module(req) => {
                vm.complete_module(req.id, default_completion(&req).unwrap())
                    .unwrap();
            }
            pkl_runtime::vm::VmPoll::Ready(val) => break val.unwrap(),
            other => panic!("unexpected VM event: {other:?}"),
        }
    };
    assert_eq!(ok, Value::Bool(true));
    vm.close();
    assert_eq!(vm.heap_stats().slots, 0);
}

#[test]
fn output_benchmark_on_primary_module_preserves_cache_and_reclaims_temporaries() {
    use pkl_runtime::modules::{ModuleCompletion, ModuleKey, ModuleKeyKind};

    let source = r#"import "pkl:Benchmark"
x = 1 + 2
hidden benchOk = let (r = new Benchmark.OutputBenchmark {
  sourceModule = module
  iterations = 2
  iterationTime = 1.ms
  isVerbose = true
}.run()) r.iterations == 2 && r.repetitions >= 1 && r.samples.length == 2
hidden samePrimary = import("file:///m3-platform-bench/main.pkl") == module
"#;
    let mut session = compile_session(
        source,
        &["benchOk", "samePrimary"],
        Some(PlatformMetadata::default()),
    );
    assert_str(eval_path(&mut session, 0, 128, true).unwrap(), "true");

    // Verify `import("file:///m3-platform-bench/main.pkl")` hits the cached
    // primary module without triggering ResolveKey or LoadSource.
    session.start_path(1, 0, None).unwrap();
    let res = loop {
        match session.poll(0, 128) {
            HostPoll::Yielded => {
                session.collect(&[]);
            }
            HostPoll::Module { token, request } => {
                let completion = match &request.kind {
                    ModuleRequestKind::SelectKey { id, uri, .. }
                        if uri == "file:///m3-platform-bench/main.pkl" =>
                    {
                        ModuleCompletion::KeySelected(Ok(ModuleKey {
                            id: *id,
                            uri: uri.clone(),
                            kind: ModuleKeyKind::StandardFile,
                            handle: Default::default(),
                        }))
                    }
                    ModuleRequestKind::QueryCachePolicy { key }
                        if key.uri == "file:///m3-platform-bench/main.pkl" =>
                    {
                        ModuleCompletion::CachePolicy(Ok(true))
                    }
                    ModuleRequestKind::ResolveKey { .. } | ModuleRequestKind::LoadSource { .. } => {
                        panic!("expected primary module cache hit, got {request:?}");
                    }
                    _ => default_completion(&request).expect("default completion"),
                };
                session.complete_module(&token, completion, 0).unwrap();
                session.collect(&[]);
            }
            HostPoll::Read { token, .. } => {
                session.resume_default_read(&token, &[], &[], 0).unwrap();
            }
            HostPoll::Ready(result) => break result,
            other => panic!("unexpected Host event: {other:?}"),
        }
    };
    assert_str(res.unwrap(), "true");
    session.close();
    assert_eq!(session.collect(&[]).slots, 0);
}
