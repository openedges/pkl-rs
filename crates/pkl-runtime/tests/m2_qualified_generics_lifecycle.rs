//! Host ownership/order controls. Java raw compatibility is checked separately.
#[path = "support/module_keys.rs"]
mod module_keys;
use pkl_runtime::probe::{SourceEvent, SourceProbe};

fn probe() -> SourceProbe {
    SourceProbe::new_expressions(
        "import \"pkl:base\" as b\nimport \"life:/type\" as dep\nstable=9\n",
        "file:///stage70/qualified-host.pkl",
        "qualified-host",
        &[
            "Int is b.Class<dep.T>".into(),
            "List(1) is b.List<dep.T>".into(),
            "1 is b.VarArgs<dep.T>".into(),
            "stable".into(),
        ],
        "compact",
        false,
        false,
        false,
    )
    .unwrap()
}
fn next(probe: &mut SourceProbe) -> SourceEvent {
    for _ in 0..100_000 {
        probe.collect();
        let event = probe.poll(0, 1).unwrap();
        probe.collect();
        if event.kind == "yield" {
            continue;
        }
        if event.kind == "module-request"
            && module_keys::complete_probe(probe, event.token.as_deref().unwrap(), 0).unwrap()
        {
            continue;
        }
        return event;
    }
    panic!("bounded qualified type continuation");
}
fn value(event: SourceEvent, text: &str) {
    assert_eq!(event.kind, "value");
    assert_eq!(
        event.value.unwrap(),
        text.encode_utf16().collect::<Vec<_>>()
    );
}
fn load(probe: &mut SourceProbe, request: SourceEvent) -> SourceEvent {
    assert_eq!(request.kind, "module-request");
    let module = request.module.unwrap();
    assert_eq!(module.kind, "resolve");
    assert_eq!(module.original_uri, "life:/type");
    probe
        .complete_module_resolution(
            request.token.as_deref().unwrap(),
            "life:/type",
            "life:/type",
            "life:/type",
            0,
        )
        .unwrap();
    let loading = next(probe);
    assert_eq!(loading.kind, "module-request");
    assert_eq!(loading.module.as_ref().unwrap().kind, "load-source");
    loading
}

#[test]
fn class_erasure_skips_host_argument_but_collection_and_varargs_retain_it() {
    let mut probe = probe();
    probe.start_path(0, 0, None).unwrap();
    // No user-module request may precede this value: Class<T> erases T.
    value(next(&mut probe), "true");
    probe.start_path(1, 0, None).unwrap();
    let request = next(&mut probe);
    assert_eq!(request.kind, "module-request");
    let failed = request.token.unwrap();
    probe
        .reject_module(&failed, false, "not-found", "", 0)
        .unwrap();
    assert_eq!(next(&mut probe).kind, "error");
    probe.start_path(1, 0, None).unwrap();
    let request = next(&mut probe);
    assert_ne!(request.token.as_deref(), Some(failed.as_str()));
    let loading = load(&mut probe, request);
    let token = loading.token.unwrap();
    probe.reenter_path(&token, 0, 0, None).unwrap();
    value(next(&mut probe), "true");
    assert_eq!(next(&mut probe).token.as_deref(), Some(token.as_str()));
    probe
        .complete_module_source(&token, "typealias T = Int\n", "type", 0)
        .unwrap();
    value(next(&mut probe), "true");
    // The resolved argument is retained, but VarArgs public value checking is
    // a fatal type operation, not a false `is` result or a construction error.
    for _ in 0..2 {
        probe.start_path(2, 0, None).unwrap();
        let error = next(&mut probe);
        assert_eq!(error.kind, "error");
        assert_eq!(error.disposition, Some("language-error"));
        assert!(
            String::from_utf16_lossy(error.error.as_ref().unwrap())
                .contains("Class `VarArgs` can only be used by the standard library.")
        );
        probe.start_path(3, 0, None).unwrap();
        value(next(&mut probe), "9");
    }
    probe.close();
    assert_eq!(probe.collect().slots, 0);
}

#[test]
fn cancelled_qualified_argument_load_releases_and_rejects_stale_response() {
    let mut probe = probe();
    probe.start_path(2, 0, None).unwrap();
    let request = next(&mut probe);
    let loading = load(&mut probe, request);
    let token = loading.token.unwrap();
    probe.cancel();
    assert_eq!(probe.drain_cancelled_modules(), vec![token.clone()]);
    assert!(probe.drain_cancelled_modules().is_empty());
    assert!(
        probe
            .complete_module_source(&token, "typealias T = Int\n", "type", 0)
            .is_err()
    );
    probe.close();
    assert_eq!(probe.collect().slots, 0);
}
