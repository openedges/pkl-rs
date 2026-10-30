//! Actual Java-loaded source/value channels, retained without source rewriting.
//! These controls isolate the shared loaded-body parser/VM path. They do not
//! replace glob Host lifecycle or the full portable contract comparison.
use pkl_runtime::evaluation::session::{SourceSession, SourceSessionOptions};
use pkl_runtime::host::HostPoll;
use pkl_runtime::source::SourceText;
use pkl_runtime::value::{PklString, Value};

fn units(bytes: &[u8]) -> PklString {
    assert_eq!(bytes.len() % 2, 0);
    PklString::from_utf16(
        bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u16::from_be_bytes([pair[0], pair[1]]))
            .collect::<Vec<_>>(),
    )
}

#[test]
fn actual_java_loaded_raw_utf16_bodies_retain_values_across_repeat_and_gc() {
    let cases: &[(&[u8], &[u8], &[u16])] = &[
        (
            include_bytes!("fixtures/m2-glob-lossless-source/high.source.utf16be"),
            include_bytes!("fixtures/m2-glob-lossless-source/high.value.utf16be"),
            &[0xd800],
        ),
        (
            include_bytes!("fixtures/m2-glob-lossless-source/low.source.utf16be"),
            include_bytes!("fixtures/m2-glob-lossless-source/low.value.utf16be"),
            &[0xdfff],
        ),
        (
            include_bytes!("fixtures/m2-glob-lossless-source/mixed.source.utf16be"),
            include_bytes!("fixtures/m2-glob-lossless-source/mixed.value.utf16be"),
            &[0x78, 0xd800, 0x79],
        ),
    ];
    for &(source_bytes, expected_bytes, raw_name) in cases {
        let source = SourceText::from(units(source_bytes));
        let name = PklString::from_utf16(raw_name.to_vec());
        let uri = PklString::from("gmod:").concat(&name);
        let mut session = SourceSession::compile_expressions_source(
            source,
            uri,
            name,
            &[SourceText::from("name")],
            SourceSessionOptions::default(),
        )
        .expect("actual Java body parses and compiles");
        for _ in 0..2 {
            session.start_path(0, 0, None).unwrap();
            let mut outcome = None;
            for _ in 0..4096 {
                match session.poll(0, 4096) {
                    HostPoll::Ready(value) => {
                        outcome = Some(value);
                        break;
                    }
                    HostPoll::Yielded => {}
                    HostPoll::Module { token, .. } => {
                        assert!(
                            session.complete_default_module(&token, 0).unwrap(),
                            "body must not request external module I/O"
                        );
                    }
                    effect => panic!("unexpected effect from the literal body: {effect:?}"),
                }
            }
            let Value::String(value) = outcome
                .expect("bounded body finishes")
                .expect("Java-success body succeeds")
            else {
                panic!("body returned a non-string");
            };
            let actual = value
                .encode_utf16()
                .flat_map(u16::to_be_bytes)
                .collect::<Vec<_>>();
            assert_eq!(
                actual, expected_bytes,
                "compare the complete retained Java UTF-16 channel"
            );
            session.collect(&[]);
        }
        session.close();
    }
}
