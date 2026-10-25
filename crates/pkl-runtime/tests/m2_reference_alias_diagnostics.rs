//! Full Java diagnostics for alias-body and authored-use Reference validation.
#[path = "support/module_keys.rs"]
mod module_keys;
use pkl_runtime::{probe::SourceProbe, value::PklString};

#[test]
fn reference_alias_frames_match_java_and_retry_without_retained_roots() {
    let rows = [
        (
            "exact-nested-alias",
            include_str!(
                "../../../tests/fixtures/m2-reference-alias-diagnostics/00-exact-nested-alias.pkl"
            ),
            include_bytes!(
                "../../../tests/fixtures/m2-reference-alias-diagnostics/00-exact-nested-alias.utf8.bin"
            ) as &[u8],
            include_bytes!(
                "../../../tests/fixtures/m2-reference-alias-diagnostics/00-exact-nested-alias.utf16be.bin"
            ) as &[u8],
        ),
        (
            "constraint-at-usage",
            include_str!(
                "../../../tests/fixtures/m2-reference-alias-diagnostics/01-constraint-at-usage.pkl"
            ),
            include_bytes!(
                "../../../tests/fixtures/m2-reference-alias-diagnostics/01-constraint-at-usage.utf8.bin"
            ) as &[u8],
            include_bytes!(
                "../../../tests/fixtures/m2-reference-alias-diagnostics/01-constraint-at-usage.utf16be.bin"
            ) as &[u8],
        ),
        (
            "direct-constrained-reference",
            include_str!(
                "../../../tests/fixtures/m2-reference-alias-diagnostics/02-direct-constrained-reference.pkl"
            ),
            include_bytes!(
                "../../../tests/fixtures/m2-reference-alias-diagnostics/02-direct-constrained-reference.utf8.bin"
            ) as &[u8],
            include_bytes!(
                "../../../tests/fixtures/m2-reference-alias-diagnostics/02-direct-constrained-reference.utf16be.bin"
            ) as &[u8],
        ),
        (
            "valid-unconstrained-reference",
            include_str!(
                "../../../tests/fixtures/m2-reference-alias-diagnostics/03-valid-unconstrained-reference.pkl"
            ),
            include_bytes!(
                "../../../tests/fixtures/m2-reference-alias-diagnostics/03-valid-unconstrained-reference.utf8.bin"
            ) as &[u8],
            include_bytes!(
                "../../../tests/fixtures/m2-reference-alias-diagnostics/03-valid-unconstrained-reference.utf16be.bin"
            ) as &[u8],
        ),
    ];
    for (id, source, utf8, utf16) in rows {
        let (pairs, tail) = utf16.as_chunks::<2>();
        assert!(tail.is_empty(), "{id}: complete Java UTF-16 units");
        let expected: Vec<u16> = pairs.iter().map(|pair| u16::from_be_bytes(*pair)).collect();
        let mut probe = SourceProbe::new_module_evaluator("compact", true, false, false).unwrap();
        // Reissue the same failed query in one evaluator. Signature validation
        // must preserve its alias origin after failure and GC, as on first use.
        for attempt in 0..2 {
            probe
                .evaluate_module_source(
                    "file:///stage69-72/reference-alias-regression.pkl",
                    Some(source),
                    "foo",
                    0,
                    None,
                )
                .unwrap();
            let mut terminal = None;
            for _ in 0..100_000 {
                probe.collect();
                let event = probe.poll(0, 16).unwrap();
                probe.collect();
                if event.kind == "yield" {
                    continue;
                }
                if event.kind == "module-request"
                    && module_keys::complete_probe(&mut probe, event.token.as_deref().unwrap(), 0)
                        .unwrap()
                {
                    continue;
                }
                terminal = Some(event);
                break;
            }
            let event = terminal.expect("bounded Reference alias evaluation");
            assert_eq!(event.kind, "error", "{id}, attempt {attempt}");
            let actual = event.error.expect("diagnostic UTF-16 units");
            assert_eq!(
                actual, expected,
                "{id}, attempt {attempt}: full UTF-16 diagnostic"
            );
            assert_eq!(
                PklString::from_utf16(actual).to_java_utf8().as_bytes(),
                utf8,
                "{id}, attempt {attempt}: full UTF-8 diagnostic"
            );
        }
        probe.close();
        assert_eq!(probe.collect().slots, 0, "{id}: release evaluator roots");
    }
}
