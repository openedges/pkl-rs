//! Complete unchanged upstream module outputs from recorded Java 0.32.0.
//! Captured URIs use in-memory source; expected output bytes remain Java-only.
#[path = "support/module_keys.rs"]
mod module_keys;
use pkl_runtime::{
    evaluation::frontend::{ModuleEvalOptions, ModuleSourceInput},
    probe::SourceProbe,
};

fn original(source: &str, uri: &str, expected: &[u8]) {
    let mut probe = SourceProbe::new_output(
        ModuleSourceInput {
            text: source,
            uri,
            inferred_name: "listing3",
        },
        &[],
        &ModuleEvalOptions::default(),
        SourceProbe::options("compact", false, false, false).unwrap(),
    )
    .unwrap();
    for _ in 0..2 {
        probe.start_output_bytes(0, None).unwrap();
        let mut terminal = None;
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
            if event.kind == "module-request"
                && module_keys::complete_probe(&mut probe, event.token.as_deref().unwrap(), 0)
                    .unwrap()
            {
                continue;
            }
            terminal = Some(event);
            break;
        }
        let event = terminal.expect("super entry original poll bound");
        assert_eq!(event.kind, "bytes", "{event:?}");
        assert_eq!(event.bytes.as_deref(), Some(expected));
    }
    probe.close();
    assert_eq!(probe.collect().slots, 0);
}
#[test]
fn pinned_listing_and_generator_listing_keep_complete_java_output() {
    original(
        include_str!("../../../tests/fixtures/m2-super-entry-originals/listings.pkl"),
        "file:///super-entry-originals/listings/listing3.pkl",
        include_bytes!("../../../tests/fixtures/m2-super-entry-originals/listings.stdout.bin"),
    );
    original(
        include_str!("../../../tests/fixtures/m2-super-entry-originals/listings2.pkl"),
        "file:///super-entry-originals/listings2/listing3.pkl",
        include_bytes!("../../../tests/fixtures/m2-super-entry-originals/listings2.stdout.bin"),
    );
}
