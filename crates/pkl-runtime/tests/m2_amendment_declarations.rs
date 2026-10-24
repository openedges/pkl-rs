//! Complete status and UTF-16 text from eleven independently evaluated Java modules.
//! Authority: m2-amendment-declarations/oracle-evidence.tar.gz, capture-01/repeat-0.
//! Archive SHA-256: 7eb633d508b3a28db95c29789f92a84cfb273fe811312d93b3696ce72bb6ffa3.
//! This native regression does not replace process, portable or full API comparisons.
#[path = "support/module_keys.rs"]
mod module_keys;
use pkl_runtime::probe::SourceProbe;

#[test]
fn complete_java_amendment_declaration_matrix() {
    let rows = [
        (
            r########"abstract-class-property-amend"########,
            include_str!(
                "../../../tests/fixtures/m2-amendment-declarations/modules/00-abstract-class-property-amend.pkl"
            ),
            r########"value"########,
            r########"new Dynamic { first = 1 }"########,
        ),
        (
            r########"abstract-property-unfilled"########,
            include_str!(
                "../../../tests/fixtures/m2-amendment-declarations/modules/01-abstract-property-unfilled.pkl"
            ),
            r########"error"########,
            r########"–– Pkl Error ––
Tried to read property `value` but its value is undefined.

1 | abstract class Parent { value: Int }
                            ^^^^^
at amendment-declarations#Parent.value (file:///stage64/amendment-declarations.pkl)

3 | test = (new Child {}).value
           ^^^^^^^^^^^^^^^^^^^^
at amendment-declarations#test (file:///stage64/amendment-declarations.pkl)

1 | test
    ^^^^
at  (repl:text)
"########,
        ),
        (
            r########"class-body-amends-int"########,
            include_str!(
                "../../../tests/fixtures/m2-amendment-declarations/modules/02-class-body-amends-int.pkl"
            ),
            r########"error"########,
            r########"–– Pkl Error ––
Cannot instantiate, or amend an instance of, external class `Int`.

To create an instance of an external class, use its literal syntax or constructor method.
Examples:
* `123` uses literal syntax to create an instance of class `Int`.
* `Pair(1, 2)` uses the `Pair()` constructor method to create an instance of class `Pair`."########,
        ),
        (
            r########"const-class-property-amend"########,
            include_str!(
                "../../../tests/fixtures/m2-amendment-declarations/modules/03-const-class-property-amend.pkl"
            ),
            r########"error"########,
            r########"–– Pkl Error ––
Missing modifier `const`.

2 | class Child extends Parent { child { second = 2 } }
                                 ^^^^^
at amendment-declarations#Child (file:///stage64/amendment-declarations.pkl)

Property `child` must be declared const, because it overrides a const property on parent class `amendment-declarations#Parent`.

3 | test = (new Child {}).child
                ^^^^^
at amendment-declarations#test (file:///stage64/amendment-declarations.pkl)

1 | test
    ^^^^
at  (repl:text)
"########,
        ),
        (
            r########"extends-external-int"########,
            include_str!(
                "../../../tests/fixtures/m2-amendment-declarations/modules/04-extends-external-int.pkl"
            ),
            r########"error"########,
            r########"–– Pkl Error ––
Cannot extend non-open class `Int`.

1 | class Bad extends Int {}
                      ^^^
at amendment-declarations#Bad (file:///stage64/amendment-declarations.pkl)

By default, classes are closed for extension.
To make a class extensible, add an `open` modifier: `open class MyClass { ... }`

2 | test = new Bad {}
               ^^^
at amendment-declarations#test (file:///stage64/amendment-declarations.pkl)

1 | test
    ^^^^
at  (repl:text)
"########,
        ),
        (
            r########"fixed-class-property-amend"########,
            include_str!(
                "../../../tests/fixtures/m2-amendment-declarations/modules/05-fixed-class-property-amend.pkl"
            ),
            r########"error"########,
            r########"–– Pkl Error ––
Missing modifier `fixed`.

2 | class Child extends Parent { child { second = 2 } }
                                 ^^^^^
at amendment-declarations#Child (file:///stage64/amendment-declarations.pkl)

Property `child` must be declared fixed, because it overrides a fixed property on parent class `amendment-declarations#Parent`.

3 | test = (new Child {}).child
                ^^^^^
at amendment-declarations#test (file:///stage64/amendment-declarations.pkl)

1 | test
    ^^^^
at  (repl:text)
"########,
        ),
        (
            r########"later-syntax-versus-abstract"########,
            include_str!(
                "../../../tests/fixtures/m2-amendment-declarations/modules/06-later-syntax-versus-abstract.pkl"
            ),
            r########"error"########,
            r########"–– Pkl Error ––
Missing `}` delimiter.

4 | later = new Dynamic {
                         ^
at amendment-declarations (file:///stage64/amendment-declarations.pkl)
"########,
        ),
        (
            r########"later-syntax-versus-fixed"########,
            include_str!(
                "../../../tests/fixtures/m2-amendment-declarations/modules/07-later-syntax-versus-fixed.pkl"
            ),
            r########"error"########,
            r########"–– Pkl Error ––
Missing `}` delimiter.

4 | later = new Dynamic {
                         ^
at amendment-declarations (file:///stage64/amendment-declarations.pkl)
"########,
        ),
        (
            r########"local-class-property-amend"########,
            include_str!(
                "../../../tests/fixtures/m2-amendment-declarations/modules/08-local-class-property-amend.pkl"
            ),
            r########"value"########,
            r########"new Dynamic { second = 2 }"########,
        ),
        (
            r########"typed-annotation-amend"########,
            include_str!(
                "../../../tests/fixtures/m2-amendment-declarations/modules/09-typed-annotation-amend.pkl"
            ),
            r########"error"########,
            r########"–– Pkl Error ––
Properties with type annotations cannot have object bodies.

To define both a type annotation and an object body, try using assignment instead.
For example: `obj: Bird = new { ... }`.

1 | class Bad { child: Dynamic { x = 1 } }
                               ^
at amendment-declarations (file:///stage64/amendment-declarations.pkl)
"########,
        ),
        (
            r########"valid-repeated-class-bodies"########,
            include_str!(
                "../../../tests/fixtures/m2-amendment-declarations/modules/10-valid-repeated-class-bodies.pkl"
            ),
            r########"value"########,
            r########"new Dynamic { first = 1; second = 2 }"########,
        ),
    ];
    assert_eq!(rows.len(), 11);
    let mut failures = Vec::new();
    for (id, source, kind, expected) in rows {
        // Each source has the same URI in a fresh Java evaluator. Keep separate
        // Rust evaluators too, so module caches cannot cross program boundaries.
        let mut probe = SourceProbe::new_module_evaluator("compact", false, false, false).unwrap();
        probe
            .evaluate_module_source(
                "file:///stage64/amendment-declarations.pkl",
                Some(source),
                "test",
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
        let event = terminal.expect("declaration query poll bound");
        let actual = if event.kind == "value" {
            event.value
        } else {
            event.error
        };
        if event.kind != kind
            || actual.as_ref() != Some(&expected.encode_utf16().collect::<Vec<_>>())
        {
            failures.push(format!(
                "{id}\nexpected {kind}: {expected}\nactual {}: {}",
                event.kind,
                actual
                    .map(|units| String::from_utf16_lossy(&units))
                    .unwrap_or_default()
            ));
        }
        probe.close();
        assert_eq!(probe.collect().slots, 0, "{id}: evaluator cleanup");
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}
