//! Module path argument Host suspension/reentry/GC, independent from raw Java authority.
#[path = "support/module_keys.rs"]
mod module_keys;
use pkl_runtime::{
    modules::ModuleRequestKind,
    probe::{SourceEvent, SourceProbe},
};
const URI: &str = "life:/stage106/root/root.pkl";
const CHILD: &str = "life:/stage106/root/sub/child.pkl";
const SOURCE: &str =
    "module stage106.lifecycle\nmarker = false\nunforced = throw(\"must stay lazy\")\n";

fn next(probe: &mut SourceProbe) -> SourceEvent {
    for _ in 0..200_000 {
        probe.collect();
        let event = probe.poll(0, 1).unwrap();
        probe.collect();
        if event.kind == "yield" {
            continue;
        }
        if event.kind == "module-request" {
            let token = event.token.as_deref().unwrap();
            if module_keys::complete_probe(probe, token, 0).unwrap() {
                continue;
            }
            let request = probe.module_request(token).unwrap();
            let original = match &request.kind {
                ModuleRequestKind::Resolve { original_uri, .. } => Some(original_uri),
                ModuleRequestKind::ResolveKey { key, .. } => Some(&key.uri),
                _ => None,
            };
            if let Some(original) = original {
                assert_eq!(original, CHILD);
                probe
                    .complete_module_resolution(
                        token,
                        original,
                        "life:/different/resolved.pkl",
                        "life:/diagnostic/source.pkl",
                        0,
                    )
                    .unwrap();
                continue;
            }
        }
        return event;
    }
    panic!("Module.relativePathTo Host poll bound")
}

#[test]
fn argument_module_loader_suspends_reenters_and_uses_original_key_without_forcing_values() {
    let mut probe = SourceProbe::new_module_evaluator("compact", false, false, false).unwrap();
    probe
        .evaluate_module_source(
            URI,
            Some(SOURCE),
            format!("module.relativePathTo(import(\"{CHILD}\")).toString()"),
            0,
            None,
        )
        .unwrap();
    let pending = next(&mut probe);
    assert_eq!(pending.kind, "module-request", "{pending:?}");
    let token = pending.token.unwrap();
    assert!(matches!(
        probe.module_request(&token).unwrap().kind,
        ModuleRequestKind::LoadSource { .. }
    ));
    probe
        .reenter_module_source(
            &token,
            URI,
            Some(SOURCE),
            "module.hasProperty(\"unforced\").toString()",
            0,
            None,
        )
        .unwrap();
    let inner = next(&mut probe);
    assert_eq!(inner.kind, "value", "{inner:?}");
    assert_eq!(
        inner.value.unwrap(),
        "true".encode_utf16().collect::<Vec<_>>()
    );
    assert_eq!(next(&mut probe).token.as_deref(), Some(token.as_str()));
    probe
        .complete_module_source(
            &token,
            "module stage106.child\nunforced = throw(\"must stay lazy\")\n",
            "child",
            0,
        )
        .unwrap();
    let result = next(&mut probe);
    assert_eq!(result.kind, "value", "{result:?}");
    // Using the resolved/source URI would reject this path. The ModuleKey URI
    // is the actual original descendant; neither module's values are forced.
    assert_eq!(
        result.value.unwrap(),
        "List(\"sub\")".encode_utf16().collect::<Vec<_>>()
    );
    probe.close();
    assert_eq!(probe.collect().slots, 0);
}
