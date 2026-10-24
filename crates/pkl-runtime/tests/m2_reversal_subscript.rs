//! Complete independent Java 0.32.0 string observations, capture-02/repeat-0.
//! Authority: m2-reversal-subscript/oracle-evidence.tar.gz.
#[path = "support/module_keys.rs"]
mod module_keys;
use pkl_runtime::probe::SourceProbe;
#[test]
fn complete_java_matrix() {
    let mut probe = SourceProbe::new_module_evaluator("compact", false, false, false).unwrap();
    let mut failures = Vec::new();
    let rows = [
        (
            r########"empty-zero"########,
            r########"List().reverse()[0]"########,
            r########"error"########,
            r########"–– Pkl Error ––
Element index `0` is out of range `0`..`-1`.
Collection: List()

1 | List().reverse()[0]
    ^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"empty-negative"########,
            r########"List().reverse()[-1]"########,
            r########"error"########,
            r########"–– Pkl Error ––
Element index `-1` is out of range `0`..`-1`.
Collection: List()

1 | List().reverse()[-1]
    ^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"one-negative"########,
            r########"List(1).reverse()[-1]"########,
            r########"error"########,
            r########"–– Pkl Error ––
Element index `-1` is out of range `0`..`0`.
Collection: List(1)

1 | List(1).reverse()[-1]
    ^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"one-upper"########,
            r########"List(1).reverse()[1]"########,
            r########"error"########,
            r########"–– Pkl Error ––
Element index `1` is out of range `0`..`0`.
Collection: List(1)

1 | List(1).reverse()[1]
    ^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"one-large"########,
            r########"List(1).reverse()[1000]"########,
            r########"error"########,
            r########"–– Pkl Error ––
Element index `1,000` is out of range `0`..`0`.
Collection: List(1)

1 | List(1).reverse()[1000]
    ^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"one-large-negative"########,
            r########"List(1).reverse()[-1000]"########,
            r########"error"########,
            r########"–– Pkl Error ––
Element index `-1,000` is out of range `0`..`0`.
Collection: List(1)

1 | List(1).reverse()[-1000]
    ^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"two-large"########,
            r########"List(1,2).reverse()[1000]"########,
            r########"error"########,
            r########"–– Pkl Error ––
Element index `1,000` is out of range `0`..`1`.
Collection: List(2, 1)

1 | List(1,2).reverse()[1000]
    ^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"null-upper"########,
            r########"List(null).reverse()[1]"########,
            r########"error"########,
            r########"–– Pkl Error ––
Element index `1` is out of range `0`..`0`.
Collection: List(null)

1 | List(null).reverse()[1]
    ^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"lazy-out"########,
            r########"List(new Dynamic { value = throw("nested") }).reverse()[1]"########,
            r########"error"########,
            r########"–– Pkl Error ––
Element index `1` is out of range `0`..`0`.
Collection: List(new Dynamic { value = ? })

1 | List(new Dynamic { value = throw("nested") }).reverse()[1]
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"lazy-in"########,
            r########"List(new Dynamic { value = throw("nested") }).reverse()[0].value"########,
            r########"error"########,
            r########"–– Pkl Error ––
nested

1 | List(new Dynamic { value = throw("nested") }).reverse()[0].value
                               ^^^^^^^^^^^^^^^
at repl:text.value (repl:text)

1 | List(new Dynamic { value = throw("nested") }).reverse()[0].value
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"high-surrogate-out"########,
            r########"List("\u{d800}").reverse()[1]"########,
            r########"error"########,
            vec![
                8211, 8211, 32, 80, 107, 108, 32, 69, 114, 114, 111, 114, 32, 8211, 8211, 10, 69,
                108, 101, 109, 101, 110, 116, 32, 105, 110, 100, 101, 120, 32, 96, 49, 96, 32, 105,
                115, 32, 111, 117, 116, 32, 111, 102, 32, 114, 97, 110, 103, 101, 32, 96, 48, 96,
                46, 46, 96, 48, 96, 46, 10, 67, 111, 108, 108, 101, 99, 116, 105, 111, 110, 58, 32,
                76, 105, 115, 116, 40, 34, 55296, 34, 41, 10, 10, 49, 32, 124, 32, 76, 105, 115,
                116, 40, 34, 92, 117, 123, 100, 56, 48, 48, 125, 34, 41, 46, 114, 101, 118, 101,
                114, 115, 101, 40, 41, 91, 49, 93, 10, 32, 32, 32, 32, 94, 94, 94, 94, 94, 94, 94,
                94, 94, 94, 94, 94, 94, 94, 94, 94, 94, 94, 94, 94, 94, 94, 94, 94, 94, 94, 94, 94,
                94, 10, 97, 116, 32, 32, 40, 114, 101, 112, 108, 58, 116, 101, 120, 116, 41, 10,
            ],
        ),
        (
            r########"low-surrogate-out"########,
            r########"List("\u{dc00}").reverse()[1]"########,
            r########"error"########,
            vec![
                8211, 8211, 32, 80, 107, 108, 32, 69, 114, 114, 111, 114, 32, 8211, 8211, 10, 69,
                108, 101, 109, 101, 110, 116, 32, 105, 110, 100, 101, 120, 32, 96, 49, 96, 32, 105,
                115, 32, 111, 117, 116, 32, 111, 102, 32, 114, 97, 110, 103, 101, 32, 96, 48, 96,
                46, 46, 96, 48, 96, 46, 10, 67, 111, 108, 108, 101, 99, 116, 105, 111, 110, 58, 32,
                76, 105, 115, 116, 40, 34, 56320, 34, 41, 10, 10, 49, 32, 124, 32, 76, 105, 115,
                116, 40, 34, 92, 117, 123, 100, 99, 48, 48, 125, 34, 41, 46, 114, 101, 118, 101,
                114, 115, 101, 40, 41, 91, 49, 93, 10, 32, 32, 32, 32, 94, 94, 94, 94, 94, 94, 94,
                94, 94, 94, 94, 94, 94, 94, 94, 94, 94, 94, 94, 94, 94, 94, 94, 94, 94, 94, 94, 94,
                94, 10, 97, 116, 32, 32, 40, 114, 101, 112, 108, 58, 116, 101, 120, 116, 41, 10,
            ],
        ),
        (
            r########"replacement-out"########,
            r########"List("\u{fffd}").reverse()[1]"########,
            r########"error"########,
            r########"–– Pkl Error ––
Element index `1` is out of range `0`..`0`.
Collection: List("�")

1 | List("\u{fffd}").reverse()[1]
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"high-surrogate-in"########,
            r########"List("\u{d800}").reverse()[0]"########,
            r########"value"########,
            vec![55296],
        ),
        (
            r########"listing-empty"########,
            r########"new Listing {}[0]"########,
            r########"error"########,
            r########"–– Pkl Error ––
Element index `0` is out of range `0`..`-1`.

1 | new Listing {}[0]
    ^^^^^^^^^^^^^^^^^
at  (repl:text)
"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"listing-upper"########,
            r########"new Listing { 1 }[1]"########,
            r########"error"########,
            r########"–– Pkl Error ––
Element index `1` is out of range `0`..`0`.

1 | new Listing { 1 }[1]
    ^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"bytes-upper"########,
            r########"Bytes(1)[1]"########,
            r########"error"########,
            r########"–– Pkl Error ––
Element index `1` is out of range `0`..`0`.
Value: Bytes(1)

1 | Bytes(1)[1]
    ^^^^^^^^^^^
at  (repl:text)
"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"wrong-key-type"########,
            r########"List(1).reverse()["1"]"########,
            r########"error"########,
            r########"–– Pkl Error ––
Operator `[]` is not defined for operand types `List` and `String`.
Left operand : List(1)
Right operand: "1"

1 | List(1).reverse()["1"]
    ^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"key-throw"########,
            r########"List(1).reverse()[throw("key")]"########,
            r########"error"########,
            r########"–– Pkl Error ––
key

1 | List(1).reverse()[throw("key")]
                      ^^^^^^^^^^^^
at  (repl:text)
"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
    ];
    assert_eq!(rows.len(), 19);
    for (id, expression, kind, expected) in rows {
        probe
            .evaluate_module_source(
                "file:///stage66/reversal-subscript.pkl",
                Some(include_str!(
                    "../../../tests/fixtures/m2-reversal-subscript/module.pkl"
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
