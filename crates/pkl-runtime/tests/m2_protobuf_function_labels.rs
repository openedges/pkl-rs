//! Full diagnostic bytes/code units from 12 independently authored Java programs,
//! each captured with power assertions on and off. No substring comparisons.
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
        .join("../../tests/fixtures/m2-protobuf-function-labels")
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
    assert_eq!(bytes(&expected, "process.stdout.bin"), b"");
    assert_eq!(bytes(&expected, "process.stderr.bin"), b"");
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
                            handle: "function-labels-supplied-primary".into(),
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
control!(function_0_value_root_on, "function-0-value-root", true);
control!(function_0_value_root_off, "function-0-value-root", false);
control!(
    function_0_document_root_on,
    "function-0-document-root",
    true
);
control!(
    function_0_document_root_off,
    "function-0-document-root",
    false
);
control!(function_0_value_nested_on, "function-0-value-nested", true);
control!(
    function_0_value_nested_off,
    "function-0-value-nested",
    false
);
control!(
    function_0_document_nested_on,
    "function-0-document-nested",
    true
);
control!(
    function_0_document_nested_off,
    "function-0-document-nested",
    false
);
control!(function_1_value_root_on, "function-1-value-root", true);
control!(function_1_value_root_off, "function-1-value-root", false);
control!(
    function_1_document_root_on,
    "function-1-document-root",
    true
);
control!(
    function_1_document_root_off,
    "function-1-document-root",
    false
);
control!(function_1_value_nested_on, "function-1-value-nested", true);
control!(
    function_1_value_nested_off,
    "function-1-value-nested",
    false
);
control!(
    function_1_document_nested_on,
    "function-1-document-nested",
    true
);
control!(
    function_1_document_nested_off,
    "function-1-document-nested",
    false
);
control!(function_2_value_root_on, "function-2-value-root", true);
control!(function_2_value_root_off, "function-2-value-root", false);
control!(
    function_2_document_root_on,
    "function-2-document-root",
    true
);
control!(
    function_2_document_root_off,
    "function-2-document-root",
    false
);
control!(function_2_value_nested_on, "function-2-value-nested", true);
control!(
    function_2_value_nested_off,
    "function-2-value-nested",
    false
);
control!(
    function_2_document_nested_on,
    "function-2-document-nested",
    true
);
control!(
    function_2_document_nested_off,
    "function-2-document-nested",
    false
);
