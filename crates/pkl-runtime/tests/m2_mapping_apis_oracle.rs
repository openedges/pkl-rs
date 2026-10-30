//! All488 required Mapping contracts from pinned Pkl 0.32.0 Java.
//! Java Base64 transport is provenance only; Rust reads original expression bytes.
//! Compare full status, UTF8/UTF16 results and ordered Logger fields; Java
//! exception/class/cause metadata is retained in fixtures, not compared here.
#[path = "support/module_keys.rs"]
mod module_keys;
use pkl_runtime::{modules::ModuleRequestKind, probe::SourceProbe, value::PklString};
use std::{
    fs,
    path::{Path, PathBuf},
};
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/m2-mapping-apis-oracle")
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
            for _ in 0..200_000 {
                probe.collect();
                let event = probe.poll(0, 1).unwrap();
                probe.collect();
                if event.kind == "yield" {
                    continue;
                }
                if event.kind == "module-request" {
                    let token = event.token.as_deref().unwrap();
                    if module_keys::complete_probe(&mut probe, token, 0).unwrap() {
                        continue;
                    }
                    let auxiliary_uri = "file:///Users/cody.ij.hwang/Projects/pkl-rs.stage57/target/m2/stage102-mapping-api-candidate-05/auxiliary-modules/cached-failure.pkl";
                    let request = probe.module_request(token).unwrap();
                    match request.kind {
                        ModuleRequestKind::Resolve { original_uri, .. } => {
                            assert_eq!(original_uri, auxiliary_uri);
                            probe
                                .complete_module_resolution(
                                    token,
                                    auxiliary_uri,
                                    auxiliary_uri,
                                    auxiliary_uri,
                                    0,
                                )
                                .unwrap();
                        }
                        ModuleRequestKind::LoadSource { resolved, .. } => {
                            assert_eq!(resolved.original_uri, auxiliary_uri);
                            let text = fs::read_to_string(
                                root().join("provenance/review99-auxiliary/cached-failure.pkl"),
                            )
                            .unwrap();
                            probe
                                .complete_module_source(token, &text, "cached-failure", 0)
                                .unwrap();
                        }
                        other => panic!("unexpected Mapping fixture module boundary {other:?}"),
                    }
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
            let event = terminal.expect("bounded Mapping API query");
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
                "mapping-api-row program={program} repeat={repeat} id={id} exact_match={equal}"
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
fn java_mapping_lookup_values() {
    run("mapping-lookup-values", 155, false);
}

#[test]
fn java_mapping_inheritance_cache() {
    run("mapping-inheritance-cache", 17, false);
}

#[test]
fn java_mapping_type_cache() {
    run("mapping-type-cache", 15, false);
}

#[test]
fn java_mapping_fold_order() {
    run("mapping-fold-order", 30, false);
}

#[test]
fn java_mapping_contracts() {
    run("mapping-contracts", 20, false);
}

#[test]
fn java_mapping_contracts_power() {
    run("mapping-contracts-power", 20, true);
}

#[test]
fn java_mapping_identity() {
    run("mapping-identity", 5, false);
}

#[test]
fn java_mapping_key_table_5() {
    run("mapping-key-table-5", 4, false);
}

#[test]
fn java_mapping_key_table_18() {
    run("mapping-key-table-18", 4, false);
}

#[test]
fn java_mapping_default_cache_identity() {
    run("mapping-default-cache-identity", 4, false);
}

#[test]
fn java_mapping_probe_key_identity() {
    run("mapping-probe-key-identity", 7, false);
}

#[test]
fn java_probe_shared_mapping_1() {
    run("probe-shared-mapping-1", 11, false);
}

#[test]
fn java_probe_shared_mapping_6() {
    run("probe-shared-mapping-6", 11, false);
}

#[test]
fn java_probe_shared_dynamic_1() {
    run("probe-shared-dynamic-1", 11, false);
}

#[test]
fn java_probe_shared_dynamic_6() {
    run("probe-shared-dynamic-6", 11, false);
}

#[test]
fn java_probe_shared_mapping_1_capture() {
    run("probe-shared-mapping-1-capture", 6, false);
}

#[test]
fn java_probe_shared_mapping_6_capture() {
    run("probe-shared-mapping-6-capture", 6, false);
}

#[test]
fn java_probe_shared_dynamic_1_capture() {
    run("probe-shared-dynamic-1-capture", 6, false);
}

#[test]
fn java_probe_shared_dynamic_6_capture() {
    run("probe-shared-dynamic-6-capture", 6, false);
}

#[test]
fn java_probe_reach_minimal() {
    run("probe-reach-minimal", 4, false);
}

#[test]
fn java_probe_shared_mapping_1_capture_value() {
    run("probe-shared-mapping-1-capture-value", 6, false);
}

#[test]
fn java_probe_shared_mapping_6_capture_value() {
    run("probe-shared-mapping-6-capture-value", 6, false);
}

#[test]
fn java_probe_shared_dynamic_1_capture_value() {
    run("probe-shared-dynamic-1-capture-value", 6, false);
}

#[test]
fn java_probe_shared_dynamic_6_capture_value() {
    run("probe-shared-dynamic-6-capture-value", 6, false);
}

#[test]
fn java_cache_steal_casts() {
    run("cache-steal-casts", 5, false);
}

#[test]
fn java_cache_fold_recursive() {
    run("cache-fold-recursive", 8, false);
}

#[test]
fn java_cache_super_wrapper() {
    run("cache-super-wrapper", 4, false);
}

#[test]
fn java_cache_cast_errors() {
    run("cache-cast-errors", 6, false);
}

#[test]
fn java_cache_cast_errors_power() {
    run("cache-cast-errors-power", 6, true);
}

#[test]
fn java_cache_cast_hashed_parent() {
    run("cache-cast-hashed-parent", 4, false);
}

#[test]
fn java_cache_cast_recursive_probe() {
    run("cache-cast-recursive-probe", 5, false);
}

#[test]
fn java_same_cast_recursion() {
    run("same-cast-recursion", 6, false);
}

#[test]
fn java_same_cast_recursion_power() {
    run("same-cast-recursion-power", 6, true);
}

#[test]
fn java_different_cast_recursion() {
    run("different-cast-recursion", 4, false);
}

#[test]
fn java_shared_cast_wrappers() {
    run("shared-cast-wrappers", 4, false);
}

#[test]
fn java_same_cast_mixed() {
    run("same-cast-mixed", 5, false);
}

#[test]
fn java_same_cast_mixed_power() {
    run("same-cast-mixed-power", 5, true);
}

#[test]
fn java_cached_error_absent_cast() {
    run("cached-error-absent-cast", 5, false);
}

#[test]
fn java_cached_error_present_cast() {
    run("cached-error-present-cast", 6, false);
}

#[test]
fn java_same_cast_let_frames() {
    run("same-cast-let-frames", 4, false);
}

#[test]
fn java_ordinary_outer_cached_inner() {
    run("ordinary-outer-cached-inner", 4, false);
}

#[test]
fn java_ordinary_outer_cached_inner_power() {
    run("ordinary-outer-cached-inner-power", 4, true);
}

#[test]
fn java_inline_let_byte_body() {
    run("inline-let-byte-body", 4, false);
}

#[test]
fn java_inline_let_byte_binding() {
    run("inline-let-byte-binding", 4, false);
}

#[test]
fn java_cache_cast_hashed_parent_source_corrected() {
    run("cache-cast-hashed-parent-source-corrected", 4, false);
}

#[test]
fn java_different_cast_recursion_source_corrected() {
    run("different-cast-recursion-source-corrected", 4, false);
}

#[test]
fn java_shared_cast_wrappers_source_corrected() {
    run("shared-cast-wrappers-source-corrected", 4, false);
}
