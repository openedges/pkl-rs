//! Actual pinned Java history, including raw error hints before and after
//! promotion of one shared converter's parameter slot from Double to Object.
use pkl_runtime::{probe::SourceProbe, value::PklString};
use std::{fs, path::PathBuf};

#[test]
fn persistent_converter_history_matches_all_four_java_sessions() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
        "../../tests/fixtures/m2-renderer-portable/provenance/stage47-renderer-api-oracle-04",
    );
    let case = "primitive-polymorphic-function-history";
    let source = fs::read_to_string(root.join(format!("workspace/{case}/main.pkl"))).unwrap();
    let operations =
        fs::read_to_string(root.join(format!("workspace/{case}/operations.tsv"))).unwrap();
    let expressions = operations
        .lines()
        .filter_map(|line| line.split_once('\t').map(|(_, expr)| expr.to_owned()))
        .collect::<Vec<_>>();
    // This is the captured observation identity, not a file to load on this host.
    let uri = "file:///Users/cody.ij.hwang/Projects/pkl-rs.stage47/target/m2/stage47-renderer-api-oracle-04/workspace/primitive-polymorphic-function-history/main.pkl";
    for repeat in 0..2 {
        for session in 0..2 {
            let raw = root.join(format!("cases/{case}/{repeat}/calls/session-{session}"));
            let mut probe = SourceProbe::new_expressions_text(
                &source,
                uri,
                "main",
                &expressions,
                "compact",
                false,
                false,
                true,
            )
            .unwrap();
            for (index, expression) in expressions.iter().enumerate() {
                let read =
                    |suffix: &str| fs::read(raw.join(format!("outer-{index}.{suffix}"))).unwrap();
                assert_eq!(read("expression.bin"), expression.as_bytes());
                assert_eq!(read("reads-before.bin"), b"0");
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
                        "value" => (b"success\n".as_slice(), event.value.unwrap(), Vec::new()),
                        "error" => (b"error\n".as_slice(), Vec::new(), event.error.unwrap()),
                        other => panic!("unexpected {other} at {expression}: {event:?}"),
                    };
                    assert_eq!(read("status.bin"), status, "{expression}");
                    for (channel, units) in [("stdout", stdout), ("stderr", stderr)] {
                        let bytes = units
                            .iter()
                            .flat_map(|unit| unit.to_be_bytes())
                            .collect::<Vec<_>>();
                        assert_eq!(
                            read(&format!("{channel}.code-units.bin")),
                            bytes,
                            "{expression} {channel}"
                        );
                        assert_eq!(
                            read(&format!("{channel}.bin")),
                            PklString::from_utf16(units).to_java_utf8().as_bytes(),
                            "{expression} {channel}"
                        );
                    }
                    assert_eq!(read("reads-after.bin"), b"0");
                    complete = true;
                    break;
                }
                assert!(complete, "unfinished {expression}");
            }
            probe.close();
            assert_eq!(probe.collect().slots, 0);
        }
    }
}
