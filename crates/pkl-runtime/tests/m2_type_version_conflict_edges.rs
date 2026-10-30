//! Java-first Stage82 edges; result kind and complete UTF-8/UTF-16 text only.
//! The ordinary ModuleClass row is a retained Java API exception, not a Rust pass claim.
#[path = "support/module_keys.rs"]
mod module_keys;

use pkl_runtime::modules::ModuleRequestKind;
use pkl_runtime::probe::SourceProbe;
use pkl_runtime::value::PklString;
use std::fs;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/m2-type-version-conflict-edge-oracle")
}

fn matches_text(row: &Path, name: &str, actual: &[u16]) -> bool {
    let raw = fs::read(row.join(format!("{name}.code-units.bin"))).unwrap();
    let (pairs, tail) = raw.as_chunks::<2>();
    assert!(tail.is_empty());
    let expected = pairs
        .iter()
        .map(|pair| u16::from_be_bytes(*pair))
        .collect::<Vec<_>>();
    assert_eq!(expected, actual, "{} {name} UTF-16 units", row.display());
    assert_eq!(
        fs::read(row.join(format!("{name}.bin"))).unwrap(),
        PklString::from_utf16(actual.to_vec())
            .to_java_utf8()
            .as_bytes(),
        "{} {name} UTF-8 bytes",
        row.display()
    );
    true
}

fn run(id: &str) {
    let fixture = root().join(id);
    let source = fs::read_to_string(fixture.join("module.pkl")).unwrap();
    let uri = fs::read_to_string(fixture.join("uri.txt")).unwrap();
    let query = fs::read_to_string(fixture.join("queries.tsv")).unwrap();
    let columns = query.trim_end_matches('\n').split('\t').collect::<Vec<_>>();
    assert_eq!(columns.len(), 3, "query for {id}");
    assert_eq!(columns[0], "string");
    assert_eq!(columns[1], id);
    let expression = columns[2];
    let helper_uri = uri.rsplit_once('/').unwrap().0.to_owned() + "/helper.pkl";
    let helper = fixture.join("helper.pkl");
    for repeat in 0..2 {
        let expected = fixture
            .join(format!("repeat-{repeat}"))
            .join(format!("000-{id}"));
        assert_eq!(
            fs::read(expected.join("expression.bin")).unwrap(),
            expression.as_bytes()
        );
        let mut probe = SourceProbe::new_module_evaluator("compact", false, false, false).unwrap();
        probe
            .evaluate_module_source(&uri, Some(&source), expression, 0, None)
            .unwrap();
        let mut terminal = None;
        for _ in 0..200_000 {
            probe.collect();
            let event = probe.poll(0, 16).unwrap();
            probe.collect();
            if event.kind == "yield" {
                continue;
            }
            if event.kind == "read" {
                // The Java evaluator has an empty external-properties map.
                assert_eq!(event.uri.as_deref(), Some("prop:pkl.outputFormat"));
                probe
                    .complete_read_missing(event.token.as_deref().unwrap(), 0)
                    .unwrap();
                continue;
            }
            if event.kind == "module-request" {
                let token = event.token.as_deref().unwrap();
                if module_keys::complete_probe(&mut probe, token, 0).unwrap() {
                    continue;
                }
                let request = probe.module_request(token).unwrap();
                match request.kind {
                    ModuleRequestKind::Resolve { original_uri, .. }
                        if helper.is_file() && original_uri == helper_uri.as_str() =>
                    {
                        probe
                            .complete_module_resolution(
                                token,
                                &original_uri,
                                &original_uri,
                                &original_uri,
                                0,
                            )
                            .unwrap();
                    }
                    ModuleRequestKind::LoadSource { resolved, .. }
                        if helper.is_file() && resolved.original_uri == helper_uri.as_str() =>
                    {
                        probe
                            .complete_module_source(
                                token,
                                fs::read_to_string(&helper).unwrap(),
                                "helper",
                                0,
                            )
                            .unwrap();
                    }
                    other => panic!("{id} unexpected Host request: {other:?}"),
                }
                continue;
            }
            if event.kind == "log" {
                // The CollectionConversionsOracle did not capture Java Logger events.
                probe
                    .acknowledge_log(event.token.as_deref().unwrap(), None, 0)
                    .unwrap();
                continue;
            }
            assert!(
                matches!(event.kind, "value" | "error"),
                "{id} unexpected pending event: {event:?}"
            );
            terminal = Some(event);
            break;
        }
        let terminal = terminal.expect("bounded Stage82 edge poll");
        let status = fs::read_to_string(expected.join("result.status.bin")).unwrap();
        let actual_status = match terminal.kind {
            "value" => "success\n",
            "error" if terminal.disposition == Some("language-error") => "error\n",
            other => panic!("{id} repeat {repeat} unexpected terminal {other}: {terminal:?}"),
        };
        assert_eq!(actual_status, status, "{id} repeat {repeat} status");
        let stdout = terminal.value.as_deref().unwrap_or(&[]);
        let stderr = terminal.error.as_deref().unwrap_or(&[]);
        assert!(
            matches_text(&expected, "result.stdout", stdout),
            "{id} repeat {repeat} stdout"
        );
        assert!(
            matches_text(&expected, "result.stderr", stderr),
            "{id} repeat {repeat} stderr"
        );
        probe.close();
        assert_eq!(probe.collect().slots, 0);
    }
}

#[test]
fn java_seven_typed_name_edges_match_result_bytes() {
    for id in [
        "ordinary-same-qualified",
        "same-declaration-control",
        "alias-same-qualified",
        "nullable-same-qualified",
        "hash-delimiter-collision",
        "builtin-pair-negative",
        "builtin-reference-negative",
    ] {
        run(id);
    }
}
