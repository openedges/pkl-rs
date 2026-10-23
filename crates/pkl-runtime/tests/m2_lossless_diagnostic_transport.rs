//! Original Java diagnostics retain UTF-16 units through shared parsed sessions.
use pkl_runtime::{
    evaluation::session::{SourceSession, SourceSessionOptions},
    probe::{SourceEvent, SourceProbe},
};

const SOURCE: &str =
    include_str!("../../../tests/fixtures/m2-string-subscript-error-units/main.pkl");
const URI: &str = "file:///Users/cody.ij.hwang/Projects/pkl-rs.stage42/target/m2/stage42-string-subscript-oracle-01/workspace/main.pkl";
const EXPRESSIONS: [&str; 2] = ["mixed[4]", "ascii[mixed[0]]"];
const ERRORS: [(&[u8], &[u8]); 2] = [
    (
        include_bytes!(
            "../../../tests/fixtures/m2-string-subscript-error-units/raw/0/outer-0.stderr.code-units.bin"
        ),
        include_bytes!(
            "../../../tests/fixtures/m2-string-subscript-error-units/raw/0/outer-0.stderr.bin"
        ),
    ),
    (
        include_bytes!(
            "../../../tests/fixtures/m2-string-subscript-error-units/raw/0/outer-1.stderr.code-units.bin"
        ),
        include_bytes!(
            "../../../tests/fixtures/m2-string-subscript-error-units/raw/0/outer-1.stderr.bin"
        ),
    ),
];
fn units(bytes: &[u8]) -> Vec<u16> {
    let (chunks, remainder) = bytes.as_chunks::<2>();
    assert!(remainder.is_empty());
    chunks
        .iter()
        .map(|unit| u16::from_be_bytes(*unit))
        .collect()
}
fn finish(probe: &mut SourceProbe) -> SourceEvent {
    for _ in 0..100_000 {
        let event = probe.poll(0, 3).unwrap();
        probe.collect();
        match event.kind {
            "yield" => {}
            "error" => return event,
            other => panic!("unexpected effect or result {other}"),
        }
    }
    panic!("source operation did not terminate")
}
#[test]
fn source_probe_retains_both_original_java_unpaired_diagnostic_payloads() {
    for _ in 0..2 {
        let expressions = EXPRESSIONS.map(String::from);
        let mut probe = SourceProbe::new_expressions_text(
            SOURCE,
            URI,
            "main",
            &expressions,
            "compact",
            false,
            false,
            false,
        )
        .unwrap();
        for (index, (java_units, java_utf8)) in ERRORS.iter().enumerate() {
            probe.start_path(index as u32, 0, None).unwrap();
            let event = finish(&mut probe);
            assert_eq!(event.disposition, Some("language-error"));
            let actual = event.error.expect("canonical API text units");
            assert_eq!(actual, units(java_units));
            let text = pkl_runtime::value::PklString::from_utf16(actual);
            assert_eq!(text.to_java_utf8().as_bytes(), *java_utf8);
        }
        probe.close();
        assert_eq!(probe.collect().slots, 0);
    }
}
#[test]
fn canonical_session_formatter_and_legacy_java_utf8_boundary_are_distinct() {
    use pkl_runtime::{host::HostPoll, value::PklString};
    let expressions = EXPRESSIONS.map(String::from);
    let mut session = SourceSession::compile_expressions_text(
        SOURCE,
        URI,
        "main",
        &expressions,
        SourceSessionOptions::default(),
    )
    .unwrap();
    for (index, (java_units, java_utf8)) in ERRORS.iter().enumerate() {
        session.start_path(index, 0, None).unwrap();
        let error = loop {
            match session.poll(0, 3) {
                HostPoll::Yielded => {}
                HostPoll::Ready(Err(error)) => break error,
                event => panic!("unexpected effect or result {event:?}"),
            }
        };
        let canonical: PklString = session.format_error_text(error.clone());
        assert_eq!(
            canonical.encode_utf16().collect::<Vec<_>>(),
            units(java_units)
        );
        let legacy = session.format_error(error);
        assert_eq!(legacy.as_bytes(), *java_utf8);
        assert_ne!(legacy.encode_utf16().collect::<Vec<_>>(), units(java_units));
    }
    session.close();
    assert_eq!(session.collect(&[]).slots, 0);
}

#[test]
fn output_operation_derives_api_units_and_bytes_from_one_canonical_failure() {
    use pkl_runtime::{
        evaluation::frontend::{ModuleEvalOptions, ModuleSourceInput},
        value::PklString,
    };
    // Transport invariant, not an additional Java compatibility observation.
    // The actual source/VM throw must survive both output API representations.
    let mut probe = SourceProbe::new_output_text(
        ModuleSourceInput {
            text: "output { bytes = throw(\"\\u{d800}x\\u{dc00}\") }\n",
            uri: "file:///lossless-output-transport.pkl",
            inferred_name: "lossless-output-transport",
        },
        &[],
        &ModuleEvalOptions::default(),
        SourceSessionOptions::default(),
    )
    .unwrap();
    probe.start_output_bytes(0, None).unwrap();
    let failure = loop {
        let event = probe.poll(0, 7).unwrap();
        probe.collect();
        match event.kind {
            "yield" => {}
            "read" => probe
                .respond_configured(event.token.as_deref().unwrap(), 0)
                .unwrap(),
            "error" => break event,
            other => panic!("unexpected effect or result {other}"),
        }
    };
    assert_eq!(failure.disposition, Some("language-error"));
    assert_eq!(failure.exit_code, Some(1));
    let units = failure.error.expect("output API diagnostic units");
    assert!(
        units
            .windows(3)
            .any(|window| window == [0xd800, u16::from(b'x'), 0xdc00])
    );
    let canonical = PklString::from_utf16(units);
    assert_eq!(
        failure.error_bytes.unwrap(),
        canonical.to_java_utf8().into_bytes()
    );
    probe.close();
    assert_eq!(probe.collect().slots, 0);
}
