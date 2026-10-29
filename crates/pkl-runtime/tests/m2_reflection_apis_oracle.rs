//! All419 required reflection contracts from pinned Pkl 0.32.0 Java.
//! Java Base64 transport is provenance only; Rust reads original expression bytes.
//! Compare full status, UTF8/UTF16 results and ordered Logger fields; Java
//! exception/class/cause metadata is retained in fixtures, not compared here.
#[path = "support/module_keys.rs"]
mod module_keys;
use pkl_runtime::{probe::SourceProbe, value::PklString};
use std::{
    fs,
    path::{Path, PathBuf},
};
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/m2-reflection-apis-oracle")
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
                "reflection-api-row program={program} repeat={repeat} id={id} exact_match={equal}"
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
fn java_reflection_types() {
    run("reflection-types", 64, false);
}

#[test]
fn java_reflection_declarations() {
    run("reflection-declarations", 57, false);
}

#[test]
fn java_reflection_annotations_defaults() {
    run("reflection-annotations-defaults", 14, false);
}

#[test]
fn java_reflection_locations() {
    run("reflection-locations", 9, false);
}

#[test]
fn java_reference_declaration_origin() {
    run("reference-declaration-origin", 4, false);
}

#[test]
fn java_reflection_types_power() {
    run("reflection-types-power", 18, true);
}

#[test]
fn java_doc_plain() {
    run("doc-plain", 4, false);
}

#[test]
fn java_doc_empty() {
    run("doc-empty", 4, false);
}

#[test]
fn java_doc_spacing() {
    run("doc-spacing", 4, false);
}

#[test]
fn java_doc_crlf() {
    run("doc-crlf", 4, false);
}

#[test]
fn java_doc_cr() {
    run("doc-cr", 4, false);
}

#[test]
fn java_doc_separator() {
    run("doc-separator", 4, false);
}

#[test]
fn java_doc_block_separator() {
    run("doc-block-separator", 4, false);
}

#[test]
fn java_doc_blank_before_class() {
    run("doc-blank-before-class", 4, false);
}

#[test]
fn java_doc_unicode() {
    run("doc-unicode", 4, false);
}

#[test]
fn java_doc_absent() {
    run("doc-absent", 4, false);
}

#[test]
fn java_doc_module() {
    run("doc-module", 3, false);
}

#[test]
fn java_doc_no_module() {
    run("doc-no-module", 3, false);
}

#[test]
fn java_doc_separated_docs() {
    run("doc-separated-docs", 1, false);
}

#[test]
fn java_reference_module_subclass() {
    run("reference-module-subclass", 6, false);
}

#[test]
fn java_reference_module_subclass_legal() {
    run("reference-module-subclass-legal", 6, false);
}

#[test]
fn java_reflection_types_exposed() {
    run("reflection-types-exposed", 64, false);
}

#[test]
fn java_reflection_types_power_exposed() {
    run("reflection-types-power-exposed", 18, true);
}

#[test]
fn java_reflection_declarations_exposed() {
    run("reflection-declarations-exposed", 18, false);
}

#[test]
fn java_reflection_annotations_defaults_exposed() {
    run("reflection-annotations-defaults-exposed", 6, false);
}

#[test]
fn java_reflection_locations_exposed() {
    run("reflection-locations-exposed", 9, false);
}

#[test]
fn java_reference_declaration_origin_exposed() {
    run("reference-declaration-origin-exposed", 4, false);
}

#[test]
fn java_doc_plain_exposed() {
    run("doc-plain-exposed", 4, false);
}

#[test]
fn java_doc_empty_exposed() {
    run("doc-empty-exposed", 4, false);
}

#[test]
fn java_doc_spacing_exposed() {
    run("doc-spacing-exposed", 4, false);
}

#[test]
fn java_doc_crlf_exposed() {
    run("doc-crlf-exposed", 4, false);
}

#[test]
fn java_doc_cr_exposed() {
    run("doc-cr-exposed", 4, false);
}

#[test]
fn java_doc_separator_exposed() {
    run("doc-separator-exposed", 4, false);
}

#[test]
fn java_doc_block_separator_exposed() {
    run("doc-block-separator-exposed", 4, false);
}

#[test]
fn java_doc_blank_before_class_exposed() {
    run("doc-blank-before-class-exposed", 4, false);
}

#[test]
fn java_doc_unicode_exposed() {
    run("doc-unicode-exposed", 4, false);
}

#[test]
fn java_doc_absent_exposed() {
    run("doc-absent-exposed", 4, false);
}

#[test]
fn java_doc_module_exposed() {
    run("doc-module-exposed", 3, false);
}

#[test]
fn java_doc_no_module_exposed() {
    run("doc-no-module-exposed", 3, false);
}

#[test]
fn java_doc_separated_docs_exposed() {
    run("doc-separated-docs-exposed", 1, false);
}

#[test]
fn java_reference_module_subclass_exposed() {
    run("reference-module-subclass-exposed", 6, false);
}

#[test]
fn java_reference_module_subclass_legal_exposed() {
    run("reference-module-subclass-legal-exposed", 6, false);
}

#[test]
fn java_named_imported_module_type() {
    run("named-imported-module-type", 3, false);
}

#[test]
fn java_class_value_does_not_shadow_simple_type() {
    run("class-value-does-not-shadow-simple-type", 2, false);
}

#[test]
fn java_class_value_does_not_shadow_qualified_import() {
    run("class-value-does-not-shadow-qualified-import", 2, false);
}

#[test]
fn java_named_module_versus_self_type() {
    run("named-module-versus-self-type", 2, false);
}

#[test]
fn java_canonical_class_erasure() {
    run("canonical-class-erasure", 5, false);
}

#[test]
fn java_alias_named_class_retains_arguments() {
    run("alias-named-class-retains-arguments", 2, false);
}
