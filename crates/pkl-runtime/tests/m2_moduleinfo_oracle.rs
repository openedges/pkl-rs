//! Pinned Java ModuleInfo values/errors. JVM internal/version and API continuity gaps stay explicit.
#[path = "support/module_keys.rs"]
mod module_keys;
use pkl_runtime::{
    probe::{SourceEvent, SourceProbe},
    value::PklString,
};
use std::{
    fs,
    path::{Path, PathBuf},
};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/m2-moduleinfo/oracle")
}
fn raw(path: &Path, name: &str) -> Vec<u8> {
    fs::read(path.join(name)).unwrap()
}
fn text(path: &Path, name: &str) -> String {
    fs::read_to_string(path.join(name)).unwrap()
}
fn units(path: &Path, name: &str) -> Vec<u16> {
    let bytes = raw(path, name);
    let (pairs, remainder) = bytes.as_chunks::<2>();
    assert!(remainder.is_empty());
    pairs.iter().map(|pair| u16::from_be_bytes(*pair)).collect()
}
fn compare(actual: &[u16], path: &Path, stem: &str) {
    assert_eq!(
        actual,
        units(path, &format!("{stem}.code-units.bin")),
        "{} {stem}",
        path.display()
    );
    assert_eq!(
        PklString::from_utf16(actual.to_vec())
            .to_java_utf8()
            .as_bytes(),
        raw(path, &format!("{stem}.bin")),
        "{} {stem}",
        path.display()
    );
}
fn next(probe: &mut SourceProbe) -> SourceEvent {
    for _ in 0..1_000_000 {
        let event = probe.poll(0, 1).unwrap();
        probe.collect();
        if event.kind == "module-request"
            && module_keys::complete_probe(probe, event.token.as_deref().unwrap(), 0).unwrap()
        {
            continue;
        }
        if event.kind != "yield" {
            return event;
        }
    }
    panic!("ModuleInfo did not settle");
}
fn run(id: &str, java_internal_boundary: bool) {
    let fixture = root().join("workspaces").join(id);
    let expressions = text(&fixture, "expressions.txt")
        .lines()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    for repeat in 0..2 {
        let expected = root()
            .join("repeats")
            .join(repeat.to_string())
            .join("calls")
            .join(id);
        let mut probe = SourceProbe::new_expressions_text(
            &text(&fixture, "main.pkl"),
            &text(&fixture, "uri.txt"),
            &text(&fixture, "module-name.txt"),
            &expressions,
            "compact",
            false,
            false,
            true,
        )
        .unwrap();
        let mut retained = Vec::new();
        for (index, expression) in expressions.iter().enumerate() {
            let prefix = format!("outer-{index}");
            assert_eq!(
                expression.as_bytes(),
                raw(&expected, &format!("{prefix}.expression.bin"))
            );
            probe.start_path(index as u32, 0, None).unwrap();
            let event = next(&mut probe);
            if java_internal_boundary {
                assert_eq!(
                    raw(&expected, &format!("{prefix}.error-class.bin")),
                    b"org.pkl.core.PklBugException"
                );
                assert_eq!(raw(&expected, &format!("{prefix}.status.bin")), b"error\n");
                assert_eq!(event.kind, "error");
                assert_eq!(event.disposition, Some("unsupported"));
                // Native Unsupported is an architecture boundary, not Java PklBugException parity.
                continue;
            }
            let (status, channel, data) = match event.kind {
                "value" => {
                    assert!(event.error.is_none());
                    (b"success\n".as_slice(), "stdout", event.value.unwrap())
                }
                "error" => {
                    assert!(event.value.is_none());
                    assert_eq!(event.disposition, Some("language-error"));
                    (b"error\n".as_slice(), "stderr", event.error.unwrap())
                }
                kind => panic!("unexpected ModuleInfo event {kind}"),
            };
            assert_eq!(raw(&expected, &format!("{prefix}.status.bin")), status);
            compare(&data, &expected, &format!("{prefix}.{channel}"));
            let opposite = if channel == "stdout" {
                "stderr"
            } else {
                "stdout"
            };
            assert!(raw(&expected, &format!("{prefix}.{opposite}.bin")).is_empty());
            assert!(raw(&expected, &format!("{prefix}.{opposite}.code-units.bin")).is_empty());
            retained.push((data, format!("{prefix}.{channel}")));
        }
        probe.close();
        assert_eq!(probe.collect().slots, 0);
        for (data, stem) in retained {
            compare(&data, &expected, &stem);
        }
        // Java's followup-0 used the same evaluator with an independent root module.
        // Those original captures remain in the fixture; SourceProbe cannot establish that API contract.
    }
}
macro_rules! ordinary {
    ($name:ident, $id:literal) => {
        #[test]
        fn $name() {
            run($id, false);
        }
    };
}
ordinary!(construct_current, "construct-current");
ordinary!(construct_older, "construct-older");
ordinary!(
    construct_future_is_only_a_string_constraint,
    "construct-future"
);
ordinary!(construct_leading_zero, "construct-leading-zero");
ordinary!(construct_one_digit, "construct-one-digit");
ordinary!(
    construct_three_digit_constraint_and_lazy_class,
    "construct-three-digit"
);
ordinary!(
    construct_suffix_constraint_and_lazy_class,
    "construct-suffix"
);
ordinary!(
    construct_spaces_constraint_and_lazy_class,
    "construct-spaces"
);
ordinary!(construct_arabic_digits, "construct-arabic-digits");
ordinary!(construct_fullwidth_digits, "construct-fullwidth-digits");
ordinary!(construct_short_constraint_and_lazy_class, "construct-short");
ordinary!(
    construct_extra_part_constraint_and_lazy_class,
    "construct-extra-part"
);
ordinary!(construct_integer_type_and_lazy_class, "construct-integer");
ordinary!(construct_missing_value_and_lazy_class, "construct-missing");
ordinary!(annotation_current, "annotation-current");
ordinary!(annotation_older, "annotation-older");
ordinary!(
    annotation_future_enforces_module_version,
    "annotation-future"
);
ordinary!(
    annotation_leading_zero_normalizes_required_version,
    "annotation-leading-zero"
);
ordinary!(annotation_one_digit, "annotation-one-digit");
ordinary!(
    annotation_three_digit_full_power_diagnostic,
    "annotation-three-digit"
);
ordinary!(annotation_suffix_full_power_diagnostic, "annotation-suffix");
ordinary!(annotation_spaces_full_power_diagnostic, "annotation-spaces");
ordinary!(annotation_short_full_power_diagnostic, "annotation-short");
ordinary!(
    annotation_extra_part_full_power_diagnostic,
    "annotation-extra-part"
);
ordinary!(
    annotation_integer_full_type_diagnostic,
    "annotation-integer"
);
ordinary!(
    annotation_missing_full_undefined_diagnostic,
    "annotation-missing"
);
#[test]
fn java_internal_non_ascii_module_versions_remain_explicit_unsupported() {
    for id in ["annotation-arabic-digits", "annotation-fullwidth-digits"] {
        run(id, true);
    }
}
