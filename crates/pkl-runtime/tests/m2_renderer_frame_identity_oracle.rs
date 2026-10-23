//! Declaration constant identity and generator physical-frame copying, compared
//! against unchanged pinned Java observations. Java was acquired once with power
//! assertions enabled; both Rust instrumentation modes consume those same bytes.
use pkl_runtime::{probe::SourceProbe, value::PklString};
use std::{fs, path::PathBuf};

fn unhex(value: &str) -> Vec<u8> {
    assert_eq!(value.len() % 2, 0);
    value
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}

#[test]
fn constant_and_restored_generator_identity_preserve_java_bytes_with_and_without_observe() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/m2-renderer-frame-identity");
    let cases = fs::read_to_string(root.join("cases.tsv")).unwrap();
    let expected = fs::read_to_string(root.join("expected.tsv")).unwrap();
    let rows = expected
        .lines()
        .map(|line| line.split('\t').collect::<Vec<_>>())
        .collect::<Vec<_>>();
    assert_eq!(cases.lines().count(), 3);
    assert_eq!(rows.len(), 240);
    assert!(rows.iter().all(|row| row.len() == 12));
    let mut comparisons = 0;
    for case in cases.lines() {
        let fields = case.split('\t').collect::<Vec<_>>();
        let [id, relative_source, uri, name] = fields[..] else {
            panic!("invalid case row")
        };
        let source = fs::read_to_string(root.join(relative_source)).unwrap();
        for instrument in [false, true] {
            for repeat in 0..2 {
                for session in 0..2 {
                    let calls = rows
                        .iter()
                        .filter(|row| {
                            row[0] == id
                                && row[1] == repeat.to_string()
                                && row[2] == session.to_string()
                        })
                        .collect::<Vec<_>>();
                    assert!(!calls.is_empty(), "missing {id}/{repeat}/{session}");
                    let expressions = calls
                        .iter()
                        .enumerate()
                        .map(|(index, row)| {
                            assert_eq!(row[3].parse::<usize>().unwrap(), index);
                            String::from_utf8(unhex(row[4])).unwrap()
                        })
                        .collect::<Vec<_>>();
                    let mut probe = SourceProbe::new_expressions_text(
                        &source,
                        uri,
                        name,
                        &expressions,
                        "compact",
                        false,
                        false,
                        instrument,
                    )
                    .unwrap();
                    for (index, row) in calls.iter().enumerate() {
                        assert_eq!(row[10], "0");
                        probe
                            .start_path(u32::try_from(index).unwrap(), 0, None)
                            .unwrap();
                        let mut complete = false;
                        for _ in 0..1_000_000 {
                            let event = probe.poll(0, 1).unwrap();
                            probe.collect();
                            if event.kind == "yield" {
                                continue;
                            }
                            let (status, stdout, stderr) = match event.kind {
                                "value" => ("success", event.value.unwrap(), Vec::new()),
                                "error" => ("error", Vec::new(), event.error.unwrap()),
                                other => panic!("unexpected {other}: {event:?}"),
                            };
                            let context =
                                format!("{id}/{repeat}/{session}/{index}, instrument={instrument}");
                            assert_eq!(status, row[5], "{context}");
                            for (units, utf16, java_utf8) in
                                [(stdout, row[6], row[8]), (stderr, row[7], row[9])]
                            {
                                let bytes = units
                                    .iter()
                                    .flat_map(|unit| unit.to_be_bytes())
                                    .collect::<Vec<_>>();
                                assert_eq!(bytes, unhex(utf16), "UTF-16 {context}");
                                assert_eq!(
                                    PklString::from_utf16(units).to_java_utf8().as_bytes(),
                                    unhex(java_utf8),
                                    "Java UTF-8 {context}"
                                );
                            }
                            assert_eq!(row[11], "0");
                            comparisons += 1;
                            complete = true;
                            break;
                        }
                        assert!(complete, "unfinished {id}/{index}");
                    }
                    probe.close();
                    assert_eq!(probe.collect().slots, 0);
                }
            }
        }
    }
    assert_eq!(comparisons, rows.len() * 2);
}
