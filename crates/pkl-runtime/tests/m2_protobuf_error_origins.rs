//! Protobuf visitor error-origin controls from the closed Java04 acquisition.
//! Full diagnostics/code units retain source-frame and hint order in both modes.
//! Java exception-class channels are retained as provenance, not synthesized by Rust.
use pkl_runtime::{
    modules::{
        ModuleCompletion, ModuleKey, ModuleKeyKind, ModuleRequestKind, SelectionPhase,
        default_completion,
    },
    probe::SourceProbe,
    value::PklString,
};
use std::{
    fs,
    path::{Path, PathBuf},
};

fn bytes(path: &Path, name: &str) -> Vec<u8> {
    fs::read(path.join(name)).unwrap()
}
fn line(path: &Path, name: &str) -> String {
    let text = String::from_utf8(bytes(path, name)).unwrap();
    let value = text.strip_suffix('\n').expect("authored terminal LF");
    assert!(!value.contains(['\n', '\r']));
    value.to_owned()
}
fn channel(expected: &Path, stem: &str, units: &[u16]) {
    let string = PklString::from_utf16(units.to_vec());
    assert_eq!(
        string.to_java_utf8().as_bytes(),
        bytes(expected, &format!("{stem}.bin")),
        "{stem}: full UTF8"
    );
    assert_eq!(
        units
            .iter()
            .flat_map(|u| u.to_be_bytes())
            .collect::<Vec<_>>(),
        bytes(expected, &format!("{stem}.code-units.bin")),
        "{stem}: full UTF16"
    );
}
fn check(id: &str, power: bool) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/m2-protobuf-java04-full")
        .join(id);
    let expected = root.join(if power { "on" } else { "off" });
    let source = String::from_utf8(bytes(&root, "module.pkl")).unwrap();
    let uri = line(&root, "uri.txt");
    let expression = line(&root, "expression.txt");
    assert_eq!(
        uri,
        format!("file:///stage145-protobuf/{id}/main.pkl"),
        "retain the exact Java control source URI"
    );
    assert_eq!(expression, "output.text", "retain the Java control query");
    // These controls have no authored external fixture callbacks. Any actual
    // read/log/custom module request fails explicitly rather than disappearing.
    for registry in ["modules.tsv", "resources.tsv", "properties.tsv"] {
        assert!(bytes(&root, registry).is_empty());
    }
    assert_eq!(bytes(&expected, "events.count.bin"), b"0\n");
    let mut probe = SourceProbe::new_module_evaluator("compact", true, false, power).unwrap();
    probe
        .evaluate_module_source(
            uri.as_str(),
            Some(source.as_str()),
            expression.as_str(),
            0,
            None,
        )
        .unwrap();
    let mut terminal = None;
    let mut primary_selected = false;
    for _ in 0..200_000 {
        let event = probe.poll_text(0, 10_000).unwrap();
        probe.collect();
        match event.kind {
            "yield" => {}
            "module-request" => {
                let token = event.token.as_deref().expect("module token");
                let request = probe.module_request(token).unwrap();
                let completion = match &request.kind {
                    ModuleRequestKind::SelectKey {
                        id: key_id,
                        uri: requested,
                        phase: SelectionPhase::Primary,
                    } if requested == uri.as_str() => {
                        // Validate only this exact supplied-source primary key.
                        // The shared evaluator wraps it as SuppliedText, so this
                        // grants no source-loading or filesystem capability.
                        assert!(!primary_selected, "primary key selected more than once");
                        primary_selected = true;
                        ModuleCompletion::KeySelected(Ok(ModuleKey {
                            id: *key_id,
                            uri: requested.clone(),
                            kind: ModuleKeyKind::Host,
                            handle: "protobuf-error-origin-supplied-primary".into(),
                        }))
                    }
                    _ => default_completion(&request).expect(
                        "only the exact supplied primary and bundled standard module requests",
                    ),
                };
                probe.complete_module(token, completion, 0).unwrap();
            }
            "error" | "value" => {
                terminal = Some(event);
                break;
            }
            other => panic!("{id}: unexpected Host/evaluation event {other}"),
        }
    }
    probe.close();
    assert_eq!(
        probe.collect().slots,
        0,
        "closed control retained heap slots"
    );
    assert_eq!(probe.context_depth(), 0, "closed control retained contexts");
    assert!(
        primary_selected,
        "control did not validate the supplied primary key"
    );
    let event = terminal.expect("control reached a terminal outcome");
    assert_eq!(event.kind, "error");
    assert_eq!(event.disposition, Some("language-error"));
    assert_eq!(bytes(&expected, "result.status.bin"), b"error\n");
    channel(
        &expected,
        "result.stdout",
        event.value.as_deref().unwrap_or(&[]),
    );
    channel(
        &expected,
        "result.stderr",
        event.error.as_deref().expect("full error units"),
    );
    channel(
        &expected,
        "expression",
        &expression.encode_utf16().collect::<Vec<_>>(),
    );
}
macro_rules! control {
    ($test:ident, $id:literal, $power:literal) => {
        #[test]
        fn $test() {
            check($id, $power);
        }
    };
}
control!(unsupported_dynamic_on, "unsupported-dynamic", true);
control!(unsupported_dynamic_off, "unsupported-dynamic", false);
control!(unsupported_data_size_on, "unsupported-data-size", true);
control!(unsupported_data_size_off, "unsupported-data-size", false);
control!(unsupported_bytes_on, "unsupported-bytes", true);
control!(unsupported_bytes_off, "unsupported-bytes", false);
control!(unsupported_class_on, "unsupported-class", true);
control!(unsupported_class_off, "unsupported-class", false);
control!(unsupported_alias_on, "unsupported-alias", true);
control!(unsupported_alias_off, "unsupported-alias", false);
control!(unsupported_function_on, "unsupported-function", true);
control!(unsupported_function_off, "unsupported-function", false);
