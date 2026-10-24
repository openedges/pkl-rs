//! Complete independent Java 0.32.0 string observations, capture-02/repeat-0.
//! Authority: m2-reversal-implicit/oracle-evidence.tar.gz.
#[path = "support/module_keys.rs"]
mod module_keys;
use pkl_runtime::probe::SourceProbe;
#[test]
fn complete_java_matrix() {
    let mut probe = SourceProbe::new_module_evaluator("compact", false, false, false).unwrap();
    let mut failures = Vec::new();
    let rows = [
        (
            r########"list-implicit"########,
            r########"List(1,2) as List(reverse() == List(2,1))"########,
            r########"value"########,
            r########"List(1, 2)"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"list-this"########,
            r########"List(1,2) as List(this.reverse() == List(2,1))"########,
            r########"value"########,
            r########"List(1, 2)"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"set-implicit"########,
            r########"Set(1,2) as Set(reverse() == List(2,1))"########,
            r########"value"########,
            r########"Set(1, 2)"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"set-this"########,
            r########"Set(1,2) as Set(this.reverse() == List(2,1))"########,
            r########"value"########,
            r########"Set(1, 2)"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"string-implicit"########,
            r########""ab" as String(reverse() == "ba")"########,
            r########"value"########,
            r########"ab"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"string-this"########,
            r########""ab" as String(this.reverse() == "ba")"########,
            r########"value"########,
            r########"ab"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"list-implicit-false"########,
            r########"List(1,2) as List(reverse() == List(1,2))"########,
            r########"error"########,
            r########"–– Pkl Error ––
Type constraint `reverse() == List(1,2)` violated.
Value: List(1, 2)

1 | List(1,2) as List(reverse() == List(1,2))
                      ^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"set-implicit-false"########,
            r########"Set(1,2) as Set(reverse() == List(1,2))"########,
            r########"error"########,
            r########"–– Pkl Error ––
Type constraint `reverse() == List(1,2)` violated.
Value: Set(1, 2)

1 | Set(1,2) as Set(reverse() == List(1,2))
                    ^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
    ];
    assert_eq!(rows.len(), 8);
    for (id, expression, kind, expected) in rows {
        probe
            .evaluate_module_source(
                "file:///stage66/reversal-implicit.pkl",
                Some(include_str!(
                    "../../../tests/fixtures/m2-reversal-implicit/module.pkl"
                )),
                expression,
                0,
                None,
            )
            .unwrap();
        let mut terminal = None;
        for _ in 0..200_000 {
            probe.collect();
            let event = probe.poll(0, 16).unwrap();
            probe.collect();
            if event.kind == "yield" {
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
        let event = terminal.expect("reversal query poll bound");
        let actual = if event.kind == "value" {
            event.value
        } else {
            event.error
        };
        if event.kind != kind || actual.as_ref() != Some(&expected) {
            failures.push(format!(
                "{id}: {expression}\nexpected {kind}: {expected:?}\nactual {}: {}",
                event.kind,
                actual
                    .map(|v| String::from_utf16_lossy(&v))
                    .unwrap_or_default()
            ));
        }
    }
    probe.close();
    assert_eq!(probe.collect().slots, 0);
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}
