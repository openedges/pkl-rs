//! Exact Java 0.32.0 primary-root Resource observations. These calls do not
//! represent the separately retained 224 independent-root/URI-policy controls.
#[path = "support/module_keys.rs"]
mod module_keys;
use pkl_runtime::{
    host::ReadFailure,
    probe::SourceProbe,
    resources::{ReadResult, Resource},
    value::PklString,
};
use std::{collections::HashMap, path::PathBuf};
const EXPECTED: &str = include_str!("../../../tests/fixtures/m2-resource-oracle/expected.tsv");
fn hex(text: &str) -> Vec<u8> {
    assert_eq!(text.len() % 2, 0);
    text.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}
fn units(text: &str) -> Vec<u16> {
    let bytes = hex(text);
    assert_eq!(bytes.len() % 2, 0);
    bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u16::from_be_bytes(*pair))
        .collect()
}

#[test]
fn java_resource_primary_calls_keep_complete_channels_counts_and_gc_roots() {
    let root =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/m2-resource-oracle");
    let cases = [
        ("cache-text-resource-missing-retry", 21),
        ("resource-bytes-utf8", 108),
    ];
    let mut compared = 0;
    for (id, count) in cases {
        let rows = EXPECTED
            .lines()
            .map(|line| line.split('\t').collect::<Vec<_>>())
            .filter(|row| row[0] == id)
            .collect::<Vec<_>>();
        assert_eq!(rows.len(), count);
        let source = std::fs::read_to_string(root.join(format!("sources/{id}/main.pkl"))).unwrap();
        let uri = format!(
            "file:///Users/cody.ij.hwang/Projects/pkl-rs.stage46/target/m2/stage46-resource-oracle-01/workspace/{id}/main.pkl"
        );
        let expressions = rows.iter().map(|row| row[2].to_owned()).collect::<Vec<_>>();
        for _ in 0..4 {
            let mut probe = SourceProbe::new_expressions_text(
                &source,
                &uri,
                "main",
                &expressions,
                "compact",
                false,
                false,
                true,
            )
            .unwrap();
            let mut reads = 0usize;
            let mut attempts = HashMap::<String, usize>::new();
            for (index, row) in rows.iter().enumerate() {
                assert_eq!(row.len(), 10);
                assert_eq!(row[1].parse::<usize>().unwrap(), index);
                assert_eq!(
                    reads,
                    row[8].parse::<usize>().unwrap(),
                    "{id}:{index} before"
                );
                probe.start_path(index as u32, 0, None).unwrap();
                let mut settled = false;
                for _ in 0..1_000_000 {
                    let event = probe.poll(0, 1).unwrap();
                    probe.collect();
                    match event.kind {
                        "yield" => (),
                        "module-request" => assert!(
                            module_keys::complete_probe(
                                &mut probe,
                                event.token.as_deref().unwrap(),
                                0
                            )
                            .unwrap(),
                            "explicit default module policy"
                        ),
                        "read" => {
                            let token = event.token.unwrap();
                            let uri = probe.resolved_read_uri(&token).unwrap();
                            reads += 1;
                            let attempt = attempts.entry(uri.clone()).or_default();
                            *attempt += 1;
                            let result = if uri.ends_with("/missing") {
                                Ok(ReadResult::Missing)
                            } else if uri.ends_with("/retry") && *attempt == 1 {
                                Err(ReadFailure::Io("reader-first".into()))
                            } else if uri == "res:/resource" {
                                Ok(ReadResult::Resource(Resource {
                                    uri: "res:/different-returned-uri".into(),
                                    bytes: vec![65, 13, 10, 0, 66].into(),
                                }))
                            } else if let Some(vector) = uri.strip_prefix("res:/vector") {
                                let number: usize = vector.parse().unwrap();
                                assert!(number < 24);
                                Ok(ReadResult::Resource(Resource {
                                    uri: format!("res:/returned/vector{number}").into(),
                                    bytes: std::fs::read(root.join(format!(
                                        "sources/resource-bytes-utf8/payloads/vector{number}.bin"
                                    )))
                                    .unwrap()
                                    .into(),
                                }))
                            } else {
                                Ok(ReadResult::Text(format!("payload-{reads} {uri}").into()))
                            };
                            probe.complete_read(&token, result, 0).unwrap();
                        }
                        "value" | "error" => {
                            let (status, out, err) = if event.kind == "value" {
                                ("success\n", event.value.unwrap(), Vec::new())
                            } else {
                                ("error\n", Vec::new(), event.error.unwrap())
                            };
                            assert_eq!(status.as_bytes(), hex(row[3]), "{id}:{index} status");
                            assert_eq!(out, units(row[6]), "{id}:{index} stdout UTF16");
                            assert_eq!(err, units(row[7]), "{id}:{index} stderr UTF16");
                            assert_eq!(
                                PklString::from_utf16(out).to_java_utf8().as_bytes(),
                                hex(row[4]),
                                "{id}:{index} stdout JavaUTF8"
                            );
                            assert_eq!(
                                PklString::from_utf16(err).to_java_utf8().as_bytes(),
                                hex(row[5]),
                                "{id}:{index} stderr JavaUTF8"
                            );
                            settled = true;
                            break;
                        }
                        other => panic!("{id}:{index} unexpected event {other}"),
                    }
                }
                assert!(settled);
                assert_eq!(
                    reads,
                    row[9].parse::<usize>().unwrap(),
                    "{id}:{index} after"
                );
                compared += 1;
            }
            probe.close();
            assert_eq!(probe.collect().slots, 0);
        }
    }
    assert_eq!(compared, 516);
}
