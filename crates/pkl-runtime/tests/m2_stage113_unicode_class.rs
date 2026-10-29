//! Reuse the existing Stage56 Java quoted-class authority; no new expected bytes.
#[path = "support/module_keys.rs"]
mod module_keys;
use pkl_runtime::{probe::SourceProbe, value::PklString};
use std::{fs, path::Path};

fn check_text(expected: &Path, actual_dir: Option<&Path>, name: &str, actual: &[u16]) {
    let utf8 = PklString::from_utf16(actual.to_vec()).to_java_utf8();
    let utf16 = actual
        .iter()
        .flat_map(|unit| unit.to_be_bytes())
        .collect::<Vec<_>>();
    if let Some(path) = actual_dir {
        fs::write(path.join(format!("{name}.bin")), utf8.as_bytes()).unwrap();
        fs::write(path.join(format!("{name}.code-units.bin")), &utf16).unwrap();
    }
    assert_eq!(
        fs::read(expected.with_extension(format!("{name}.bin"))).unwrap(),
        utf8.as_bytes()
    );
    assert_eq!(
        fs::read(expected.with_extension(format!("{name}.code-units.bin"))).unwrap(),
        utf16
    );
}

#[test]
fn quoted_unicode_class_name_keeps_raw_name_display_and_metaclass_identity() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/m2-stage113-unicode-class");
    let capture = std::env::var_os("PKL_FRAME_CAPTURE_DIR").map(std::path::PathBuf::from);
    for repeat in 0..2 {
        let input = root.join(repeat.to_string());
        let source = fs::read_to_string(input.join("input.pkl")).unwrap();
        let queries = fs::read_to_string(input.join("expressions.txt")).unwrap();
        assert_eq!(queries.lines().count(), 4);
        let mut probe = SourceProbe::new_module_evaluator("compact", false, false, true).unwrap();
        for (ordinal, query) in queries.lines().enumerate() {
            probe
                .evaluate_module_source(
                    "file:///stage56-class-values/main.pkl",
                    Some(&source),
                    query,
                    0,
                    None,
                )
                .unwrap();
            let mut terminal = None;
            for _ in 0..100_000 {
                let event = probe.poll(0, 1).unwrap();
                probe.collect();
                if event.kind == "yield" {
                    continue;
                }
                if event.kind == "module-request" {
                    assert!(
                        module_keys::complete_probe(&mut probe, event.token.as_deref().unwrap(), 0)
                            .unwrap()
                    );
                    continue;
                }
                assert!(
                    matches!(event.kind, "value" | "error"),
                    "unexpected event (Java has zero Logger): {event:?}"
                );
                terminal = Some(event);
                break;
            }
            let event = terminal.expect("bounded quoted-class query");
            let status = match event.kind {
                "value" => "success\n",
                "error" if event.disposition == Some("language-error") => "error\n",
                _ => "unsupported-or-api-error\n",
            };
            let actual_dir = capture.as_ref().map(|root| {
                root.join("unicode-class")
                    .join(repeat.to_string())
                    .join(ordinal.to_string())
            });
            if let Some(path) = &actual_dir {
                fs::create_dir_all(path).unwrap();
                fs::write(path.join("status.bin"), status).unwrap();
                fs::write(path.join("logs.count.bin"), b"0\n").unwrap();
            }
            let expected = input.join(format!("outer-{ordinal}"));
            check_text(
                &expected,
                actual_dir.as_deref(),
                "stdout",
                event.value.as_deref().unwrap_or(&[]),
            );
            check_text(
                &expected,
                actual_dir.as_deref(),
                "stderr",
                event.error.as_deref().unwrap_or(&[]),
            );
            assert_eq!(
                fs::read(expected.with_extension("status.bin")).unwrap(),
                status.as_bytes()
            );
        }
        probe.close();
        assert_eq!(probe.collect().slots, 0);
    }
}
