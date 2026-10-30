//! Unchanged pinned module pair; expected complete bytes are current Java output.
#[path = "support/module_keys.rs"]
mod module_keys;

use pkl_runtime::{
    evaluation::frontend::{ModuleEvalOptions, ModuleSourceInput},
    modules::ModuleRequestKind,
    probe::{SourceEvent, SourceProbe},
};

const CHILD: &str = include_str!("../../../tests/fixtures/m2-local-module-class/child.pkl");
const PARENT: &str = include_str!("../../../tests/fixtures/m2-local-module-class/parent.pkl");
const EXPECTED: &[u8] =
    include_bytes!("../../../tests/fixtures/m2-local-module-class/expected-output.bin");
const CHILD_URI: &str = "file:///local-module-class/localModuleMemberOverride2.pkl";
const PARENT_URI: &str = "file:///local-module-class/localModuleMemberOverride1.pkl";

fn probe() -> SourceProbe {
    SourceProbe::new_output(
        ModuleSourceInput {
            text: CHILD,
            uri: CHILD_URI,
            inferred_name: "localModuleMemberOverride2",
        },
        &[],
        &ModuleEvalOptions::default(),
        SourceProbe::options("compact", false, false, false).unwrap(),
    )
    .unwrap()
}

fn next(probe: &mut SourceProbe) -> SourceEvent {
    for _ in 0..500_000 {
        probe.collect();
        let event = probe.poll(0, 1).unwrap();
        probe.collect();
        if event.kind == "yield" {
            continue;
        }
        if event.kind == "read" {
            probe
                .respond_configured(event.token.as_deref().unwrap(), 0)
                .unwrap();
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
                assert_eq!(original_uri, PARENT_URI);
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
                assert_eq!(resolved.source_uri, PARENT_URI);
                return event;
            }
            other => panic!("unexpected module request {other:?}"),
        }
    }
    panic!("local module class continuation exceeded bound")
}

fn suspended(probe: &mut SourceProbe, token: &str) {
    for _ in 0..4 {
        probe.collect();
        let event = probe.poll(0, 1).unwrap();
        assert_eq!(event.kind, "module-request");
        assert_eq!(event.token.as_deref(), Some(token));
    }
}

#[test]
fn inherited_local_classes_use_lexical_module_with_gc_and_cached_retry() {
    let mut probe = probe();
    for repetition in 0..2 {
        probe.start_output_bytes(0, None).unwrap();
        let mut event = next(&mut probe);
        if repetition == 0 {
            assert_eq!(event.kind, "module-request");
            let token = event.token.unwrap();
            suspended(&mut probe, &token);
            probe
                .complete_module_source(&token, PARENT, "localModuleMemberOverride1", 0)
                .unwrap();
            event = next(&mut probe);
        }
        assert_eq!(event.kind, "bytes", "{event:?}");
        assert_eq!(event.bytes.as_deref(), Some(EXPECTED));
    }
    probe.close();
    assert_eq!(probe.collect().slots, 0);
}

#[test]
fn cancelled_lexical_module_load_rejects_late_response() {
    let mut probe = probe();
    probe.start_output_bytes(0, None).unwrap();
    let event = next(&mut probe);
    assert_eq!(event.kind, "module-request");
    let token = event.token.unwrap();
    suspended(&mut probe, &token);
    probe.cancel();
    assert_eq!(probe.drain_cancelled_modules(), vec![token.clone()]);
    assert!(
        probe
            .complete_module_source(&token, PARENT, "localModuleMemberOverride1", 0)
            .is_err()
    );
    probe.close();
    assert_eq!(probe.collect().slots, 0);
}
