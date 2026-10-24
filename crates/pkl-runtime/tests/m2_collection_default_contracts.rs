//! Complete independent Java 0.32.0 string observations, capture-01/repeat-0.
//! Authority: m2-collection-default-contracts/oracle-evidence.tar.gz.
#[path = "support/module_keys.rs"]
mod module_keys;
use pkl_runtime::probe::SourceProbe;
#[test]
fn complete_java_matrix() {
    let mut probe = SourceProbe::new_module_evaluator("compact", false, false, false).unwrap();
    let mut failures = Vec::new();
    let rows = [
        (
            r########"list-nonfunction-missing"########,
            r########"new Listing { default = 1; 1 }.getOrDefault(2)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected value of type `Function1`, but got type `Int`.
Value: 1

2068 | hidden default: (Int) -> Element = (_) -> new Dynamic {}
                       ^^^^^^^^^^^^^^^^
at pkl.base#Listing.default (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L2068)

1 | new Listing { default = 1; 1 }.getOrDefault(2)
                            ^
at repl:text.default (repl:text)

1 | new Listing { default = 1; 1 }.getOrDefault(2)
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"list-nonfunction-present"########,
            r########"new Listing { default = 1; 1 }.getOrDefault(0)"########,
            r########"value"########,
            r########"1"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"list-arity-zero-missing"########,
            r########"new Listing { default = () -> 9; 1 }.getOrDefault(2)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected value of type `Function1`, but got type `Function0`.
Value: new Function0 {}

2068 | hidden default: (Int) -> Element = (_) -> new Dynamic {}
                       ^^^^^^^^^^^^^^^^
at pkl.base#Listing.default (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L2068)

1 | new Listing { default = () -> 9; 1 }.getOrDefault(2)
                            ^^^^^^^
at repl:text.default (repl:text)

1 | new Listing { default = () -> 9; 1 }.getOrDefault(2)
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"list-arity-zero-present"########,
            r########"new Listing { default = () -> 9; 1 }.getOrDefault(0)"########,
            r########"value"########,
            r########"1"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"list-arity-two-missing"########,
            r########"new Listing { default = (a, b) -> 9; 1 }.getOrDefault(2)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected value of type `Function1`, but got type `Function2`.
Value: new Function2 {}

2068 | hidden default: (Int) -> Element = (_) -> new Dynamic {}
                       ^^^^^^^^^^^^^^^^
at pkl.base#Listing.default (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L2068)

1 | new Listing { default = (a, b) -> 9; 1 }.getOrDefault(2)
                            ^^^^^^^^^^^
at repl:text.default (repl:text)

1 | new Listing { default = (a, b) -> 9; 1 }.getOrDefault(2)
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"list-arity-two-present"########,
            r########"new Listing { default = (a, b) -> 9; 1 }.getOrDefault(0)"########,
            r########"value"########,
            r########"1"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"map-nonfunction-missing"########,
            r########"new Mapping { default = 1; ["x"] = 1 }.getOrDefault("y")"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected value of type `Function1`, but got type `Int`.
Value: 1

2255 | hidden default: (Key) -> Value = (_) -> new Dynamic {}
                       ^^^^^^^^^^^^^^
at pkl.base#Mapping.default (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L2255)

1 | new Mapping { default = 1; ["x"] = 1 }.getOrDefault("y")
                            ^
at repl:text.default (repl:text)

1 | new Mapping { default = 1; ["x"] = 1 }.getOrDefault("y")
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"map-nonfunction-present"########,
            r########"new Mapping { default = 1; ["x"] = 1 }.getOrDefault("x")"########,
            r########"value"########,
            r########"1"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"map-arity-zero-missing"########,
            r########"new Mapping { default = () -> 9; ["x"] = 1 }.getOrDefault("y")"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected value of type `Function1`, but got type `Function0`.
Value: new Function0 {}

2255 | hidden default: (Key) -> Value = (_) -> new Dynamic {}
                       ^^^^^^^^^^^^^^
at pkl.base#Mapping.default (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L2255)

1 | new Mapping { default = () -> 9; ["x"] = 1 }.getOrDefault("y")
                            ^^^^^^^
at repl:text.default (repl:text)

1 | new Mapping { default = () -> 9; ["x"] = 1 }.getOrDefault("y")
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"map-arity-zero-present"########,
            r########"new Mapping { default = () -> 9; ["x"] = 1 }.getOrDefault("x")"########,
            r########"value"########,
            r########"1"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"map-arity-two-missing"########,
            r########"new Mapping { default = (a, b) -> 9; ["x"] = 1 }.getOrDefault("y")"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected value of type `Function1`, but got type `Function2`.
Value: new Function2 {}

2255 | hidden default: (Key) -> Value = (_) -> new Dynamic {}
                       ^^^^^^^^^^^^^^
at pkl.base#Mapping.default (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L2255)

1 | new Mapping { default = (a, b) -> 9; ["x"] = 1 }.getOrDefault("y")
                            ^^^^^^^^^^^
at repl:text.default (repl:text)

1 | new Mapping { default = (a, b) -> 9; ["x"] = 1 }.getOrDefault("y")
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"map-arity-two-present"########,
            r########"new Mapping { default = (a, b) -> 9; ["x"] = 1 }.getOrDefault("x")"########,
            r########"value"########,
            r########"1"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"list-typed-inherited-default"########,
            r########"((new Listing<Int> { default = (_) -> 2; 1 }) { 3 }).getOrDefault(5)"########,
            r########"value"########,
            r########"2"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"list-typed-null-present"########,
            r########"(new Listing<Int?> { default = (_) -> 3; null }).getOrDefault(0)"########,
            r########"value"########,
            r########"null"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"map-typed-inherited-default"########,
            r########"((new Mapping<String, Int> { default = (_) -> 2; ["x"] = 1 }) { ["y"] = 3 }).getOrDefault("z")"########,
            r########"value"########,
            r########"2"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"map-typed-null-present"########,
            r########"(new Mapping<String, Int?> { default = (_) -> 3; ["x"] = null }).getOrDefault("x")"########,
            r########"value"########,
            r########"null"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"list-nonfunction-present-throw"########,
            r########"new Listing { default = 1; throw("list-present-first") }.getOrDefault(0)"########,
            r########"error"########,
            r########"–– Pkl Error ––
list-present-first

1 | new Listing { default = 1; throw("list-present-first") }.getOrDefault(0)
                               ^^^^^^^^^^^^^^^^^^^^^^^^^^^
at repl:text[#1] (repl:text)

1 | new Listing { default = 1; throw("list-present-first") }.getOrDefault(0)
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"map-nonfunction-present-throw"########,
            r########"new Mapping { default = 1; ["x"] = throw("map-present-first") }.getOrDefault("x")"########,
            r########"error"########,
            r########"–– Pkl Error ––
map-present-first

1 | new Mapping { default = 1; ["x"] = throw("map-present-first") }.getOrDefault("x")
                                       ^^^^^^^^^^^^^^^^^^^^^^^^^^
at repl:text["x"] (repl:text)

1 | new Mapping { default = 1; ["x"] = throw("map-present-first") }.getOrDefault("x")
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"list-nonfunction-key-throw"########,
            r########"new Listing { default = 1; 1 }.getOrDefault(throw("list-key-first"))"########,
            r########"error"########,
            r########"–– Pkl Error ––
list-key-first

1 | new Listing { default = 1; 1 }.getOrDefault(throw("list-key-first"))
                                                ^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"map-nonfunction-key-throw"########,
            r########"new Mapping { default = 1; ["x"] = 1 }.getOrDefault(throw("map-key-first"))"########,
            r########"error"########,
            r########"–– Pkl Error ––
map-key-first

1 | new Mapping { default = 1; ["x"] = 1 }.getOrDefault(throw("map-key-first"))
                                                        ^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"list-nonfunction-null-present"########,
            r########"new Listing { default = 1; null }.getOrDefault(0)"########,
            r########"value"########,
            r########"null"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"map-nonfunction-null-present"########,
            r########"new Mapping { default = 1; ["x"] = null }.getOrDefault("x")"########,
            r########"value"########,
            r########"null"########.encode_utf16().collect::<Vec<_>>(),
        ),
    ];
    assert_eq!(rows.len(), 22);
    for (id, expression, kind, expected) in rows {
        probe
            .evaluate_module_source(
                "file:///stage62/collection-default-contracts.pkl",
                Some(include_str!(
                    "../../../tests/fixtures/m2-collection-default-contracts/module.pkl"
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
        let event = terminal.expect("conversion query poll bound");
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
