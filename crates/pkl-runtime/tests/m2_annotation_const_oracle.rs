//! All39 ordinary annotation/const contracts from pinned Java; one Java internal contract remains required/unresolved.
//! Exact status/result/error UTF8+UTF16 and ordered Logger comparison; original cause/process fields remain provenance.
#[path = "support/module_keys.rs"]
mod module_keys;
use pkl_runtime::{probe::SourceProbe, value::PklString};
use std::{
    fs,
    path::{Path, PathBuf},
};
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/m2-annotation-const-oracle")
}
fn units(raw: &[u8]) -> Vec<u16> {
    let (pairs, tail) = raw.as_chunks::<2>();
    assert!(tail.is_empty());
    pairs.iter().map(|p| u16::from_be_bytes(*p)).collect()
}
fn field(path: &Path, name: &str, actual: &[u16]) -> bool {
    units(&fs::read(path.join(format!("{name}.code-units.bin"))).unwrap()) == actual
        && fs::read(path.join(format!("{name}.bin"))).unwrap()
            == PklString::from_utf16(actual.to_vec())
                .to_java_utf8()
                .as_bytes()
}
fn capture(path: Option<&Path>, name: &str, value: &[u16]) {
    if let Some(path) = path {
        fs::write(
            path.join(format!("{name}.bin")),
            PklString::from_utf16(value.to_vec())
                .to_java_utf8()
                .as_bytes(),
        )
        .unwrap();
        fs::write(
            path.join(format!("{name}.code-units.bin")),
            value
                .iter()
                .flat_map(|x| x.to_be_bytes())
                .collect::<Vec<_>>(),
        )
        .unwrap();
    }
}
fn run(program: &str, count: usize, power_assertions: bool) {
    let input = root().join(program);
    let source = fs::read_to_string(input.join("module.pkl")).unwrap();
    let uri = fs::read_to_string(input.join("uri.txt")).unwrap();
    let queries = fs::read_to_string(input.join("queries.index.tsv")).unwrap();
    assert_eq!(
        queries.lines().count(),
        count,
        "exact admitted query inventory"
    );
    let capture_root = std::env::var_os("PKL_FRAME_CAPTURE_DIR").map(PathBuf::from);
    let mut failures = Vec::new();
    for repeat in 0..2 {
        let mut probe =
            SourceProbe::new_module_evaluator("compact", true, false, power_assertions).unwrap();
        for line in queries.lines() {
            let columns = line.split('\t').collect::<Vec<_>>();
            assert_eq!(columns.len(), 3);
            assert_eq!(columns[0], "string");
            let id = columns[1];
            let expected = input.join(format!("repeat-{repeat}")).join(columns[2]);
            let expression = fs::read_to_string(expected.join("expression.bin")).unwrap();
            assert!(field(
                &expected,
                "expression",
                &expression.encode_utf16().collect::<Vec<_>>()
            ));
            let actual_dir = capture_root.as_ref().map(|r| {
                r.join(program)
                    .join(format!("repeat-{repeat}"))
                    .join(columns[2])
            });
            if let Some(p) = &actual_dir {
                fs::create_dir_all(p).unwrap();
            }
            probe
                .evaluate_module_source(&uri, Some(&source), &expression, 0, None)
                .unwrap();
            let mut terminal = None;
            let mut logs = 0usize;
            let mut row_logs_match = true;
            // Compare complete Java observations with GC between bounded VM
            // batches. The eager pinned reflection singleton takes more than
            // 200,000 instructions; instruction-by-instruction GC stress is
            // covered separately by the lifecycle and escaped-mirror tests.
            const INSTRUCTIONS_PER_POLL: u32 = 64;
            const MAX_INSTRUCTIONS: u32 = 1_000_000;
            for _ in 0..MAX_INSTRUCTIONS.div_ceil(INSTRUCTIONS_PER_POLL) {
                probe.collect();
                let event = probe.poll(0, INSTRUCTIONS_PER_POLL).unwrap();
                probe.collect();
                if event.kind == "yield" {
                    continue;
                }
                if event.kind == "read" {
                    // Java capture evaluator externalProperties is empty.
                    // Complete absence, allowing base.pkl's own fallback;
                    // never substitute a requested renderer format/value.
                    assert_eq!(event.uri.as_deref(), Some("prop:pkl.outputFormat"));
                    probe
                        .complete_read_missing(event.token.as_deref().unwrap(), 0)
                        .unwrap();
                    continue;
                }
                if event.kind == "module-request" {
                    let token = event.token.as_deref().unwrap();
                    assert!(module_keys::complete_probe(&mut probe, token, 0).unwrap());
                    continue;
                }
                if event.kind == "log" {
                    let log = event.log.as_ref().unwrap();
                    let position = format!(
                        "{}:{}-{}:{}",
                        log.start_line, log.start_column, log.end_line, log.end_column
                    );
                    let fields = [
                        log.level.encode_utf16().collect::<Vec<_>>(),
                        log.message.clone(),
                        log.module_uri.clone(),
                        log.member_name
                            .clone()
                            .unwrap_or_else(|| "null".encode_utf16().collect()),
                        position.encode_utf16().collect(),
                    ];
                    for (name, actual) in ["level", "message", "module-uri", "member", "position"]
                        .iter()
                        .zip(&fields)
                    {
                        let name = format!("log-{logs:03}.{name}");
                        capture(actual_dir.as_deref(), &name, actual);
                        if !expected.join(format!("{name}.bin")).exists()
                            || !field(&expected, &name, actual)
                        {
                            row_logs_match = false;
                            failures.push(format!("{program}/{id}: Logger {name}"));
                        }
                    }
                    logs += 1;
                    probe
                        .acknowledge_log(event.token.as_deref().unwrap(), None, 0)
                        .unwrap();
                    continue;
                }
                assert!(
                    matches!(event.kind, "value" | "error"),
                    "unexpected pending event: {event:?}"
                );
                terminal = Some(event);
                break;
            }
            let event = terminal.expect("bounded reflection API query");
            let actual_status = match event.kind {
                "value" => "success\n",
                "error" if event.disposition == Some("oracle-error") => "oracle-error\n",
                "error" if event.disposition == Some("language-error") => "error\n",
                _ => "unsupported-or-api-error\n",
            };
            let stdout = event.value.as_deref().unwrap_or(&[]);
            let stderr = event.error.as_deref().unwrap_or(&[]);
            capture(actual_dir.as_deref(), "result.stdout", stdout);
            capture(actual_dir.as_deref(), "result.stderr", stderr);
            if let Some(p) = &actual_dir {
                fs::write(p.join("result.status.bin"), actual_status).unwrap();
                fs::write(p.join("logs.count.bin"), format!("{logs}\n")).unwrap();
            }
            let expected_logs = fs::read(expected.join("logs.count.bin")).unwrap();
            let equal = fs::read(expected.join("result.status.bin")).unwrap()
                == actual_status.as_bytes()
                && field(&expected, "result.stdout", stdout)
                && field(&expected, "result.stderr", stderr)
                && expected_logs == format!("{logs}\n").as_bytes()
                && row_logs_match;
            eprintln!(
                "annotation-const-row program={program} repeat={repeat} id={id} exact_match={equal}"
            );
            if !equal {
                failures.push(format!(
                    "{program}/{id}/repeat-{repeat}: {event:?}; logs={logs}, expected-count-bytes={expected_logs:?}"
                ));
            }
        }
        probe.close();
        assert_eq!(probe.collect().slots, 0);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn java_main_original_constannotation3() {
    run("main-original-constannotation3", 1, false);
}

#[test]
fn java_main_original_constclassbody() {
    run("main-original-constclassbody", 1, false);
}

#[test]
fn java_main_annotation_direct() {
    run("main-annotation-direct", 1, false);
}

#[test]
fn java_main_annotation_object() {
    run("main-annotation-object", 1, false);
}

#[test]
fn java_main_annotation_lambda() {
    run("main-annotation-lambda", 1, false);
}

#[test]
fn java_main_annotation_let() {
    run("main-annotation-let", 1, false);
}

#[test]
fn java_main_annotation_class_property() {
    run("main-annotation-class-property", 1, false);
}

#[test]
fn java_main_annotation_class_method() {
    run("main-annotation-class-method", 1, false);
}

#[test]
fn java_main_annotation_qualified() {
    run("main-annotation-qualified", 1, false);
}

#[test]
fn java_main_annotation_qualified_nonconst() {
    run("main-annotation-qualified-nonconst", 1, false);
}

#[test]
fn java_main_annotation_class_definition() {
    run("main-annotation-class-definition", 1, false);
}

#[test]
fn java_main_class_direct() {
    run("main-class-direct", 1, false);
}

#[test]
fn java_main_class_object() {
    run("main-class-object", 1, false);
}

#[test]
fn java_main_class_lambda() {
    run("main-class-lambda", 1, false);
}

#[test]
fn java_main_class_let() {
    run("main-class-let", 1, false);
}

#[test]
fn java_main_class_qualified() {
    run("main-class-qualified", 1, false);
}

#[test]
fn java_main_class_super_constraint() {
    run("main-class-super-constraint", 1, false);
}

#[test]
fn java_main_class_property_constraint() {
    run("main-class-property-constraint", 1, false);
}

#[test]
fn java_main_annotation_constraint() {
    run("main-annotation-constraint", 1, false);
}

#[test]
fn java_main_const_direct() {
    run("main-const-direct", 1, false);
}

#[test]
fn java_main_const_object() {
    run("main-const-object", 1, false);
}

#[test]
fn java_main_const_lambda() {
    run("main-const-lambda", 1, false);
}

#[test]
fn java_main_const_let() {
    run("main-const-let", 1, false);
}

#[test]
fn java_main_const_qualified() {
    run("main-const-qualified", 1, false);
}

#[test]
fn java_main_const_qualified_nonconst() {
    run("main-const-qualified-nonconst", 1, false);
}

#[test]
fn java_main_sibling_owner_restoration() {
    run("main-sibling-owner-restoration", 1, false);
}

#[test]
fn java_main_sibling_annotations() {
    run("main-sibling-annotations", 1, false);
}

#[test]
fn java_main_unicode_annotation() {
    run("main-unicode-annotation", 1, false);
}

#[test]
fn java_main_module_annotation() {
    run("main-module-annotation", 1, false);
}

#[test]
fn java_main_query_synthetic() {
    run("main-query-synthetic", 3, false);
}

#[test]
fn java_supplement_annotation_lambda() {
    run("supplement-annotation-lambda", 1, false);
}

#[test]
fn java_supplement_class_lambda() {
    run("supplement-class-lambda", 1, false);
}

#[test]
fn java_supplement_const_lambda() {
    run("supplement-const-lambda", 1, false);
}

#[test]
fn java_supplement_query_typed_lambda() {
    run("supplement-query-typed-lambda", 1, false);
}

#[test]
fn java_supplement_annotation_qualified_value() {
    run("supplement-annotation-qualified-value", 1, false);
}

#[test]
fn java_supplement_annotation_qualified_nonconst_value() {
    run("supplement-annotation-qualified-nonconst-value", 1, false);
}

#[test]
fn java_values_annotation_qualified_value_source() {
    run("values-annotation-qualified-value-source", 1, false);
}

#[test]
fn java_values_annotation_qualified_nonconst_value_source() {
    run(
        "values-annotation-qualified-nonconst-value-source",
        1,
        false,
    );
}
