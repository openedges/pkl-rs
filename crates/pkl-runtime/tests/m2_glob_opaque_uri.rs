//! Native Host-boundary regressions for opaque glob URI validation. These do
//! not assert byte parity with Java's unimplemented internal-error renderer.
use pkl_runtime::{
    probe::{SourceProbe, SourceTextEvent},
    source::SourceText,
};

fn next(session: &mut SourceProbe) -> SourceTextEvent {
    for _ in 0..100_000 {
        let event = session.poll_text(0, 64).unwrap();
        session.collect();
        match event.kind {
            "yield" => (),
            "module-request" => assert!(
                session
                    .complete_default_module(event.token.as_deref().unwrap(), 0)
                    .unwrap()
            ),
            _ => return event,
        }
    }
    panic!("glob did not settle");
}

fn probe(source: &str, uri: &str) -> SourceProbe {
    SourceProbe::new_expressions_source(
        source,
        uri,
        "opaque",
        &[
            SourceText::from("entries.keys.toList().join(\"|\")"),
            SourceText::from("entries.keys.toList().join(\"|\")"),
        ],
        "compact",
        false,
        false,
        false,
    )
    .unwrap()
}

fn opaque_reader(session: &mut SourceProbe, event: SourceTextEvent) {
    assert_eq!(event.kind, "glob-request");
    let descriptor = event.glob.unwrap();
    assert_eq!(descriptor.kind, "select-resource");
    session
        .complete_glob_reader(
            event.token.as_deref().unwrap(),
            &descriptor.reader_id,
            "gdata",
            "opaque-reader",
            0,
        )
        .unwrap();
    let event = next(session);
    assert_eq!(event.glob.as_ref().unwrap().kind, "is-globbable");
    session
        .complete_glob_boolean(event.token.as_deref().unwrap(), "is-globbable", true, 0)
        .unwrap();
    let event = next(session);
    assert_eq!(event.glob.as_ref().unwrap().kind, "is-hierarchical");
    session
        .complete_glob_boolean(event.token.as_deref().unwrap(), "is-hierarchical", false, 0)
        .unwrap();
}

fn terminal_without_listing(session: &mut SourceProbe) -> SourceTextEvent {
    for _ in 0..16 {
        let event = next(session);
        match event.kind {
            "glob-request" => {
                assert_eq!(
                    event.glob.as_ref().unwrap().kind,
                    "select-resource",
                    "no listing may occur after validation fails"
                );
                opaque_reader(session, event);
            }
            "error" => return event,
            other => panic!("unexpected event after opaque glob: {other}"),
        }
    }
    panic!("opaque glob did not reach a terminal error");
}

#[test]
fn java_escaped_opaque_patterns_fail_before_listing_across_queries_and_close() {
    // These five authored strings are the retained Java 0.32.0 read fixtures.
    for source in [
        r#"entries = read*("gdata:\\*")"#,
        r#"entries = read*("gdata:\\?")"#,
        r#"entries = read*("gdata:\\[")"#,
        r#"entries = read*("gdata:\\{")"#,
        r#"entries = read*("gdata:\\\\")"#,
    ] {
        let mut session = probe(source, "gmain:/tree/opaque.pkl");
        let mut held = Vec::new();
        for index in 0..2 {
            session.start_path(index, 0, None).unwrap();
            let event = terminal_without_listing(&mut session);
            assert!(event.error.is_some());
            assert_eq!(event.disposition, Some("unsupported"));
            held.push(event.error.unwrap());
            session.collect();
        }
        // This checks Rust diagnostic lifetime only; it is not Java parity.
        let snapshots = held.clone();
        session.close();
        session.collect();
        assert_eq!(held, snapshots);
        assert_eq!(session.heap_slots(), 0);
    }
}

#[test]
fn relative_escaped_glob_lists_the_dummy_uri() {
    let mut session = probe(r#"entries = read*("\\*")"#, "gdata:/tree/main.pkl");
    session.start_path(0, 0, None).unwrap();
    let selected = next(&mut session);
    opaque_reader(&mut session, selected);
    let listing = next(&mut session);
    assert_eq!(listing.kind, "glob-request");
    let request = listing.glob.unwrap();
    assert_eq!(request.kind, "list-elements");
    assert_eq!(request.uri.unwrap(), "gdata:dummy");
    session
        .complete_glob_elements(listing.token.as_deref().unwrap(), vec![], 0)
        .unwrap();
    assert_eq!(next(&mut session).kind, "value");
    session.close();
    assert_eq!(session.heap_slots(), 0);
}

#[test]
fn malformed_glob_wins_over_invalid_raw_uri() {
    let mut session = probe(r#"entries = read*("gdata:\\")"#, "gmain:/tree/opaque.pkl");
    session.start_path(0, 0, None).unwrap();
    let error = terminal_without_listing(&mut session);
    let text = String::from_utf16(&error.error.unwrap()).unwrap();
    assert!(text.contains("backslash"), "{text}");
    assert_ne!(error.disposition, Some("unsupported"));
    session.close();
    assert_eq!(session.heap_slots(), 0);
}
