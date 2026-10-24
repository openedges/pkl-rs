//! Complete independent Java 0.32.0 string observations, capture-02/repeat-0.
//! Authority: m2-prototype-diagnostics/oracle-evidence.tar.gz (39 ordered rows).
//! SHA-256: 2edd02820027c84c618458ef93d72a3cfa69071153c77fed557da1096f739ca8.
//! Checks full UTF-16 strings and event kinds, not the whole process protocol.
#[path = "support/module_keys.rs"]
mod module_keys;
use pkl_runtime::probe::SourceProbe;
#[test]
fn complete_java_prototype_diagnostic_matrix() {
    let mut probe = SourceProbe::new_module_evaluator("compact", false, false, false).unwrap();
    let mut failures = Vec::new();
    let rows = [
        (
            r########"pipe-dynamic-empty-class"########,
            r########"1 |> (Dynamic) {}"########,
            r########"error"########,
            r########"–– Pkl Error ––
Operator `|>` is not defined for operand types `Int` and `Dynamic`.
Left operand : 1
Right operand: new Dynamic {}

1 | 1 |> (Dynamic) {}
    ^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"pipe-dynamic-empty-new"########,
            r########"1 |> new Dynamic {}"########,
            r########"error"########,
            r########"–– Pkl Error ––
Operator `|>` is not defined for operand types `Int` and `Dynamic`.
Left operand : 1
Right operand: new Dynamic {}

1 | 1 |> new Dynamic {}
    ^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"pipe-dynamic-empty-generator"########,
            r########"1 |> (Dynamic) { when (false) { x = 1 } }"########,
            r########"error"########,
            r########"–– Pkl Error ––
Operator `|>` is not defined for operand types `Int` and `Dynamic`.
Left operand : 1
Right operand: new Dynamic {}

1 | 1 |> (Dynamic) { when (false) { x = 1 } }
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"pipe-dynamic-nonempty"########,
            r########"1 |> (Dynamic) { x = 1 }"########,
            r########"error"########,
            r########"–– Pkl Error ––
Operator `|>` is not defined for operand types `Int` and `Dynamic`.
Left operand : 1
Right operand: new Dynamic { x = ? }

1 | 1 |> (Dynamic) { x = 1 }
    ^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"pipe-listing-empty-class"########,
            r########"1 |> (Listing) {}"########,
            r########"error"########,
            r########"–– Pkl Error ––
Operator `|>` is not defined for operand types `Int` and `Listing`.
Left operand : 1
Right operand: new Listing {}

1 | 1 |> (Listing) {}
    ^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"pipe-listing-empty-new"########,
            r########"1 |> new Listing {}"########,
            r########"error"########,
            r########"–– Pkl Error ––
Operator `|>` is not defined for operand types `Int` and `Listing`.
Left operand : 1
Right operand: new Listing {}

1 | 1 |> new Listing {}
    ^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"pipe-listing-empty-generator"########,
            r########"1 |> (Listing) { when (false) { 1 } }"########,
            r########"error"########,
            r########"–– Pkl Error ––
Operator `|>` is not defined for operand types `Int` and `Listing`.
Left operand : 1
Right operand: new Listing {}

1 | 1 |> (Listing) { when (false) { 1 } }
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"pipe-listing-nonempty"########,
            r########"1 |> (Listing) { 1 }"########,
            r########"error"########,
            r########"–– Pkl Error ––
Operator `|>` is not defined for operand types `Int` and `Listing`.
Left operand : 1
Right operand: new Listing { ? }

1 | 1 |> (Listing) { 1 }
    ^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"pipe-mapping-empty-class"########,
            r########"1 |> (Mapping) {}"########,
            r########"error"########,
            r########"–– Pkl Error ––
Operator `|>` is not defined for operand types `Int` and `Mapping`.
Left operand : 1
Right operand: new Mapping {}

1 | 1 |> (Mapping) {}
    ^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"pipe-mapping-empty-new"########,
            r########"1 |> new Mapping {}"########,
            r########"error"########,
            r########"–– Pkl Error ––
Operator `|>` is not defined for operand types `Int` and `Mapping`.
Left operand : 1
Right operand: new Mapping {}

1 | 1 |> new Mapping {}
    ^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"pipe-mapping-empty-generator"########,
            r########"1 |> (Mapping) { when (false) { ["x"] = 1 } }"########,
            r########"error"########,
            r########"–– Pkl Error ––
Operator `|>` is not defined for operand types `Int` and `Mapping`.
Left operand : 1
Right operand: new Mapping {}

1 | 1 |> (Mapping) { when (false) { ["x"] = 1 } }
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"pipe-mapping-nonempty"########,
            r########"1 |> (Mapping) { ["x"] = 1 }"########,
            r########"error"########,
            r########"–– Pkl Error ––
Operator `|>` is not defined for operand types `Int` and `Mapping`.
Left operand : 1
Right operand: new Mapping { ["x"] = ? }

1 | 1 |> (Mapping) { ["x"] = 1 }
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"pipe-user-empty-class"########,
            r########"1 |> (Plain) {}"########,
            r########"error"########,
            r########"–– Pkl Error ––
Operator `|>` is not defined for operand types `Int` and `prototype-diagnostics#Plain`.
Left operand : 1
Right operand: new Plain {}

1 | 1 |> (Plain) {}
    ^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"pipe-user-empty-new"########,
            r########"1 |> new Plain {}"########,
            r########"error"########,
            r########"–– Pkl Error ––
Operator `|>` is not defined for operand types `Int` and `prototype-diagnostics#Plain`.
Left operand : 1
Right operand: new Plain {}

1 | 1 |> new Plain {}
    ^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"pipe-user-lazy"########,
            r########"1 |> new C {}"########,
            r########"error"########,
            r########"–– Pkl Error ––
Operator `|>` is not defined for operand types `Int` and `prototype-diagnostics#C`.
Left operand : 1
Right operand: new C { x = ? }

1 | 1 |> new C {}
    ^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"pipe-class-value"########,
            r########"1 |> Dynamic"########,
            r########"error"########,
            r########"–– Pkl Error ––
Operator `|>` is not defined for operand types `Int` and `Class`.
Left operand : 1
Right operand: Dynamic

1 | 1 |> Dynamic
    ^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"add-dynamic-empty-left"########,
            r########"(Dynamic) {} + 1"########,
            r########"error"########,
            r########"–– Pkl Error ––
Operator `+` is not defined for operand types `Dynamic` and `Int`.
Left operand : new Dynamic {}
Right operand: 1

1 | (Dynamic) {} + 1
    ^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"add-dynamic-empty-right"########,
            r########"1 + (Dynamic) {}"########,
            r########"error"########,
            r########"–– Pkl Error ––
Operator `+` is not defined for operand types `Int` and `Dynamic`.
Left operand : 1
Right operand: new Dynamic {}

1 | 1 + (Dynamic) {}
    ^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"add-listing-empty-left"########,
            r########"(Listing) {} + 1"########,
            r########"error"########,
            r########"–– Pkl Error ––
Operator `+` is not defined for operand types `Listing` and `Int`.
Left operand : new Listing {}
Right operand: 1

1 | (Listing) {} + 1
    ^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"add-listing-empty-right"########,
            r########"1 + (Listing) {}"########,
            r########"error"########,
            r########"–– Pkl Error ––
Operator `+` is not defined for operand types `Int` and `Listing`.
Left operand : 1
Right operand: new Listing {}

1 | 1 + (Listing) {}
    ^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"add-mapping-empty-left"########,
            r########"(Mapping) {} + 1"########,
            r########"error"########,
            r########"–– Pkl Error ––
Operator `+` is not defined for operand types `Mapping` and `Int`.
Left operand : new Mapping {}
Right operand: 1

1 | (Mapping) {} + 1
    ^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"add-mapping-empty-right"########,
            r########"1 + (Mapping) {}"########,
            r########"error"########,
            r########"–– Pkl Error ––
Operator `+` is not defined for operand types `Int` and `Mapping`.
Left operand : 1
Right operand: new Mapping {}

1 | 1 + (Mapping) {}
    ^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"add-dynamic-nonempty"########,
            r########"1 + (Dynamic) { x = 1 }"########,
            r########"error"########,
            r########"–– Pkl Error ––
Operator `+` is not defined for operand types `Int` and `Dynamic`.
Left operand : 1
Right operand: new Dynamic { x = ? }

1 | 1 + (Dynamic) { x = 1 }
    ^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"add-listing-nonempty"########,
            r########"1 + (Listing) { 1 }"########,
            r########"error"########,
            r########"–– Pkl Error ––
Operator `+` is not defined for operand types `Int` and `Listing`.
Left operand : 1
Right operand: new Listing { ? }

1 | 1 + (Listing) { 1 }
    ^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"add-mapping-nonempty"########,
            r########"1 + (Mapping) { ["x"] = 1 }"########,
            r########"error"########,
            r########"–– Pkl Error ––
Operator `+` is not defined for operand types `Int` and `Mapping`.
Left operand : 1
Right operand: new Mapping { ["x"] = ? }

1 | 1 + (Mapping) { ["x"] = 1 }
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"add-user-empty"########,
            r########"1 + (Plain) {}"########,
            r########"error"########,
            r########"–– Pkl Error ––
Operator `+` is not defined for operand types `Int` and `prototype-diagnostics#Plain`.
Left operand : 1
Right operand: new Plain {}

1 | 1 + (Plain) {}
    ^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"add-user-lazy"########,
            r########"1 + new C {}"########,
            r########"error"########,
            r########"–– Pkl Error ––
Operator `+` is not defined for operand types `Int` and `prototype-diagnostics#C`.
Left operand : 1
Right operand: new C { x = ? }

1 | 1 + new C {}
    ^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"method-dynamic-empty"########,
            r########"((Dynamic) {}).missingMethod()"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot find method `missingMethod` in class `Dynamic`.

1 | ((Dynamic) {}).missingMethod()
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)

Available methods in class `Dynamic`:
getClass
getProperty
getPropertyOrNull
hasProperty
ifNonNull
length
toList
toMap
toString
toTyped
"########,
        ),
        (
            r########"method-dynamic-nonempty"########,
            r########"((Dynamic) { x = 1 }).missingMethod()"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot find method `missingMethod` in class `Dynamic`.

1 | ((Dynamic) { x = 1 }).missingMethod()
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)

Available methods in class `Dynamic`:
getClass
getProperty
getPropertyOrNull
hasProperty
ifNonNull
length
toList
toMap
toString
toTyped
"########,
        ),
        (
            r########"method-listing-empty"########,
            r########"((Listing) {}).missingMethod()"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot find method `missingMethod` in class `Listing`.

1 | ((Listing) {}).missingMethod()
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)

Available methods in class `Listing`:
any
contains
distinctBy
every
fold
foldIndexed
getClass
getOrDefault
getOrNull
ifNonNull
isDistinctBy
join
toList
toSet
toString
"########,
        ),
        (
            r########"method-listing-nonempty"########,
            r########"((Listing) { 1 }).missingMethod()"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot find method `missingMethod` in class `Listing`.

1 | ((Listing) { 1 }).missingMethod()
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)

Available methods in class `Listing`:
any
contains
distinctBy
every
fold
foldIndexed
getClass
getOrDefault
getOrNull
ifNonNull
isDistinctBy
join
toList
toSet
toString
"########,
        ),
        (
            r########"method-mapping-empty"########,
            r########"((Mapping) {}).missingMethod()"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot find method `missingMethod` in class `Mapping`.

1 | ((Mapping) {}).missingMethod()
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)

Available methods in class `Mapping`:
any
containsKey
containsValue
every
fold
getClass
getOrDefault
getOrNull
ifNonNull
toMap
toString
"########,
        ),
        (
            r########"method-mapping-nonempty"########,
            r########"((Mapping) { ["x"] = 1 }).missingMethod()"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot find method `missingMethod` in class `Mapping`.

1 | ((Mapping) { ["x"] = 1 }).missingMethod()
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)

Available methods in class `Mapping`:
any
containsKey
containsValue
every
fold
getClass
getOrDefault
getOrNull
ifNonNull
toMap
toString
"########,
        ),
        (
            r########"method-user-empty"########,
            r########"((Plain) {}).missingMethod()"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot find method `missingMethod` in class `prototype-diagnostics#Plain`.

1 | ((Plain) {}).missingMethod()
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)

Available methods in class `prototype-diagnostics#Plain`:
getClass
getProperty
getPropertyOrNull
hasProperty
ifNonNull
toDynamic
toMap
toString
"########,
        ),
        (
            r########"method-user-lazy"########,
            r########"(new C {}).missingMethod()"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot find method `missingMethod` in class `prototype-diagnostics#C`.

1 | (new C {}).missingMethod()
    ^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)

Available methods in class `prototype-diagnostics#C`:
getClass
getProperty
getPropertyOrNull
hasProperty
ifNonNull
toDynamic
toMap
toString
"########,
        ),
        (
            r########"class-dynamic-empty"########,
            r########"((Dynamic) {}).getClass().simpleName"########,
            r########"value"########,
            r########"Dynamic"########,
        ),
        (
            r########"class-listing-empty"########,
            r########"((Listing) {}).getClass().simpleName"########,
            r########"value"########,
            r########"Listing"########,
        ),
        (
            r########"class-mapping-empty"########,
            r########"((Mapping) {}).getClass().simpleName"########,
            r########"value"########,
            r########"Mapping"########,
        ),
        (
            r########"class-user-empty"########,
            r########"((Plain) {}).getClass().simpleName"########,
            r########"value"########,
            r########"Plain"########,
        ),
    ];
    assert_eq!(rows.len(), 39);
    for (id, expression, kind, expected) in rows {
        probe
            .evaluate_module_source(
                "file:///stage64/prototype-diagnostics.pkl",
                Some(include_str!(
                    "../../../tests/fixtures/m2-prototype-diagnostics/module.pkl"
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
        let event = terminal.expect("addition query poll bound");
        let actual = if event.kind == "value" {
            event.value
        } else {
            event.error
        };
        if event.kind != kind
            || actual.as_ref() != Some(&expected.encode_utf16().collect::<Vec<_>>())
        {
            failures.push(format!(
                "{id}: {expression}\nexpected {kind}: {expected}\nactual {}: {}",
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
