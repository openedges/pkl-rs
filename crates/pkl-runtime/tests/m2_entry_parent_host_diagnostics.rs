//! Java fixes the diagnostic and import boundary; Rust separately tests suspension.
//! Synchronous Java callback logs do not establish full asynchronous SPI count parity.
#[path = "support/module_keys.rs"]
mod module_keys;

use pkl_runtime::{
    modules::ModuleRequestKind,
    probe::{SourceEvent, SourceProbe},
};

const MAIN_URI: &str = "stage68:/entry-parent-host/main.pkl";
const ALIAS_URI: &str = "stage68:/entry-parent-host/aliases.pkl";
const MAIN: &str = include_str!("../../../tests/fixtures/m2-entry-parent-host/main.pkl");
const ALIASES: &str = include_str!("../../../tests/fixtures/m2-entry-parent-host/aliases.pkl");

fn next_import_or_terminal(probe: &mut SourceProbe) -> SourceEvent {
    for _ in 0..200_000 {
        probe.collect();
        let event = probe.poll(0, 1).unwrap();
        probe.collect();
        if event.kind == "yield" {
            continue;
        }
        if event.kind != "module-request" {
            return event;
        }
        let token = event.token.as_deref().unwrap();
        if module_keys::complete_probe(probe, token, 0).unwrap() {
            continue;
        }
        match probe.module_request(token).unwrap().kind {
            ModuleRequestKind::Resolve { original_uri, .. } => {
                assert!(original_uri == MAIN_URI || original_uri == ALIAS_URI);
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
            ModuleRequestKind::LoadSource { resolved, .. } => {
                assert_eq!(resolved.source_uri, ALIAS_URI);
                return event;
            }
            other => panic!("unexpected module request: {other:?}"),
        }
    }
    panic!("entry parent Host continuation exceeded poll bound")
}

fn keep_suspended(probe: &mut SourceProbe, token: &str) {
    // No completion is delivered during these polls. The continuation and its
    // type/default source metadata must remain reachable across collection.
    for _ in 0..4 {
        probe.collect();
        let waiting = probe.poll(0, 1).unwrap();
        assert_eq!(waiting.kind, "module-request");
        assert_eq!(waiting.token.as_deref(), Some(token));
        let ModuleRequestKind::LoadSource { resolved, .. } =
            probe.module_request(token).unwrap().kind
        else {
            panic!("suspended source request changed kind")
        };
        assert_eq!(resolved.source_uri, ALIAS_URI);
        probe.collect();
    }
}

fn complete_error(probe: &mut SourceProbe, expected: &[u8]) -> usize {
    assert_eq!(expected.len() % 2, 0);
    let expected = expected
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u16::from_be_bytes(*pair))
        .collect::<Vec<_>>();
    let mut delayed_loads = 0;
    loop {
        let event = next_import_or_terminal(probe);
        if event.kind == "module-request" {
            let token = event.token.unwrap();
            keep_suspended(probe, &token);
            probe
                .complete_module_source(&token, ALIASES, "aliases", 0)
                .unwrap();
            delayed_loads += 1;
            continue;
        }
        assert_eq!(event.kind, "error");
        assert_eq!(event.disposition, Some("language-error"));
        assert_eq!(event.error, Some(expected));
        return delayed_loads;
    }
}

#[test]
fn imported_parent_resume_preserves_all_three_java_diagnostics_and_retry() {
    let rows: [(&str, &[u8]); 3] = [
        (
            "inferredBag()[0]",
            include_bytes!(
                "../../../tests/fixtures/m2-entry-parent-host/repeat-0/000-inferred-set-alias/result.stderr.code-units.bin"
            ),
        ),
        (
            "inferredText()[0]",
            include_bytes!(
                "../../../tests/fixtures/m2-entry-parent-host/repeat-0/001-inferred-string-alias/result.stderr.code-units.bin"
            ),
        ),
        (
            "explicitBag()[0]",
            include_bytes!(
                "../../../tests/fixtures/m2-entry-parent-host/repeat-0/002-explicit-set-control/result.stderr.code-units.bin"
            ),
        ),
    ];
    let mut probe = SourceProbe::new_module_evaluator("compact", false, false, false).unwrap();
    for repetition in 0..2 {
        // The first repetition retains the exact recorded Java query order.
        // The second checks Rust error retry and cache ownership with GC.
        for (index, (expression, expected)) in rows.iter().enumerate() {
            probe
                .evaluate_module_source(MAIN_URI, Some(MAIN), *expression, 0, None)
                .unwrap();
            let delayed_loads = complete_error(&mut probe, expected);
            if repetition == 0 && index == 0 {
                assert!(delayed_loads > 0, "alias load did not suspend in Host");
            }
        }
    }
    probe.close();
    assert_eq!(probe.collect().slots, 0);
}

#[test]
fn cancelled_import_rejects_late_completion_and_close_releases_ownership() {
    let mut probe = SourceProbe::new_module_evaluator("compact", false, false, false).unwrap();
    probe
        .evaluate_module_source(MAIN_URI, Some(MAIN), "inferredBag()[0]", 0, None)
        .unwrap();
    let pending = next_import_or_terminal(&mut probe);
    assert_eq!(pending.kind, "module-request");
    let token = pending.token.unwrap();
    keep_suspended(&mut probe, &token);
    probe.cancel();
    assert_eq!(probe.drain_cancelled_modules(), vec![token.clone()]);
    assert!(probe.drain_cancelled_modules().is_empty());
    assert!(
        probe
            .complete_module_source(&token, ALIASES, "aliases", 0)
            .is_err()
    );
    probe.close();
    assert_eq!(probe.collect().slots, 0);
}
