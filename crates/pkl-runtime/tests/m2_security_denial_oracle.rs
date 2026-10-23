//! Actual Java Standard-policy SecurityManagerException cause through typed resolve completion.
use pkl_runtime::{
    probe::{SourceEvent, SourceProbe},
    value::PklString,
};
use std::{
    fs,
    path::{Path, PathBuf},
};
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/m2-security-denial")
}
fn raw(path: &Path, name: &str) -> Vec<u8> {
    fs::read(path.join(name)).unwrap()
}
fn text(path: &Path, name: &str) -> String {
    fs::read_to_string(path.join(name)).unwrap()
}
fn units(path: &Path, name: &str) -> Vec<u16> {
    let data = raw(path, name);
    let (pairs, rest) = data.as_chunks::<2>();
    assert!(rest.is_empty());
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
fn next(probe: &mut SourceProbe, expected: &Path, denials: &mut usize) -> SourceEvent {
    for _ in 0..1_000_000 {
        let event = probe.poll(0, 1).unwrap();
        probe.collect();
        match event.kind {
            "yield" => {}
            "module-request" => {
                if probe
                    .complete_default_module(event.token.as_deref().unwrap(), 0)
                    .unwrap()
                {
                    continue;
                }
                let request = event.module.unwrap();
                assert_eq!(request.kind, "resolve");
                assert_eq!(request.original_uri, text(&root(), "denied-uri.txt"));
                let prefix = format!("denial-{}", *denials);
                assert_eq!(
                    raw(expected, &format!("{prefix}.error-class.bin")),
                    b"org.pkl.core.SecurityManagerException"
                );
                assert_eq!(
                    raw(expected, &format!("{prefix}.uri.bin")),
                    request.original_uri.as_bytes()
                );
                let message = PklString::from_utf16(units(
                    expected,
                    &format!("{prefix}.message.code-units.bin"),
                ));
                assert_eq!(
                    message.to_java_utf8().as_bytes(),
                    raw(expected, &format!("{prefix}.message.bin"))
                );
                probe
                    .reject_module_text(
                        event.token.as_deref().unwrap(),
                        false,
                        "security-manager-exception",
                        Some(message),
                        0,
                    )
                    .unwrap();
                *denials += 1;
            }
            "value" | "error" => return event,
            kind => panic!("unexpected typed security event {kind}"),
        }
    }
    panic!("typed security control did not settle");
}
#[test]
fn actual_policy_denial_is_caught_and_retries_while_successful_caught_value_is_cached() {
    let r = root();
    let workspace = r.join("oracle/workspace");
    let expressions = text(&workspace, "expressions.txt")
        .lines()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    for repeat in 0..2 {
        let expected = r
            .join("oracle/repeats")
            .join(repeat.to_string())
            .join("calls");
        let mut probe = SourceProbe::new_expressions_text(
            &text(&workspace, "main.pkl"),
            &text(&r, "uri.txt"),
            "main",
            &expressions,
            "compact",
            false,
            false,
            true,
        )
        .unwrap();
        let mut denials = 0;
        let mut retained = Vec::new();
        for (index, expression) in expressions.iter().enumerate() {
            let prefix = format!("outer-{index}");
            assert_eq!(
                raw(&expected, &format!("{prefix}.expression.bin")),
                expression.as_bytes()
            );
            probe.start_path(index as u32, 0, None).unwrap();
            let event = next(&mut probe, &expected, &mut denials);
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
                kind => panic!("unexpected completion {kind}"),
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
            let counters = text(&expected, &format!("{prefix}.counters-after.bin"));
            let actual_java_denials = counters
                .lines()
                .find_map(|line| line.strip_prefix("denied="))
                .unwrap()
                .parse::<usize>()
                .unwrap();
            assert_eq!(denials, actual_java_denials);
            retained.push((data, format!("{prefix}.{channel}")));
        }
        assert_eq!(denials, 3);
        probe.close();
        assert_eq!(probe.collect().slots, 0);
        for (data, stem) in retained {
            compare(&data, &expected, &stem);
        }
    }
    // Full Java policy callbacks and file-policy implementation are separately retained,
    // not relabeled as SourceProbe Host callbacks or complete security compatibility.
}
