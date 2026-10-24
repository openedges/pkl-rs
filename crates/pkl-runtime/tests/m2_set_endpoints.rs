//! Complete independent Java 0.32.0 string observations, capture-02/repeat-0.
//! Authority: m2-set-endpoints/oracle-evidence.tar.gz (70 ordered rows).
#[path = "support/module_keys.rs"]
mod module_keys;
use pkl_runtime::probe::SourceProbe;
#[test]
fn complete_java_set_endpoints_matrix() {
    let mut probe = SourceProbe::new_module_evaluator("compact", false, false, false).unwrap();
    let mut failures = Vec::new();
    let rows = [
        (
            r########"set-empty-first"########,
            r########"Set().first"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected a non-empty collection.
Collection: Set()

1 | Set().first
    ^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"set-empty-first-or-null"########,
            r########"Set().firstOrNull"########,
            r########"value"########,
            r########"null"########,
        ),
        (
            r########"set-empty-last"########,
            r########"Set().last"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected a non-empty collection.
Collection: Set()

1 | Set().last
    ^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"set-empty-last-or-null"########,
            r########"Set().lastOrNull"########,
            r########"value"########,
            r########"null"########,
        ),
        (
            r########"list-empty-first"########,
            r########"List().first"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected a non-empty collection.
Collection: List()

1 | List().first
    ^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"list-empty-first-or-null"########,
            r########"List().firstOrNull"########,
            r########"value"########,
            r########"null"########,
        ),
        (
            r########"list-empty-last"########,
            r########"List().last"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected a non-empty collection.
Collection: List()

1 | List().last
    ^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"list-empty-last-or-null"########,
            r########"List().lastOrNull"########,
            r########"value"########,
            r########"null"########,
        ),
        (
            r########"set-single-first"########,
            r########"Set(7).first"########,
            r########"value"########,
            r########"7"########,
        ),
        (
            r########"set-single-first-or-null"########,
            r########"Set(7).firstOrNull"########,
            r########"value"########,
            r########"7"########,
        ),
        (
            r########"set-single-last"########,
            r########"Set(7).last"########,
            r########"value"########,
            r########"7"########,
        ),
        (
            r########"set-single-last-or-null"########,
            r########"Set(7).lastOrNull"########,
            r########"value"########,
            r########"7"########,
        ),
        (
            r########"set-null-first"########,
            r########"Set(null).first"########,
            r########"value"########,
            r########"null"########,
        ),
        (
            r########"set-null-first-or-null"########,
            r########"Set(null).firstOrNull"########,
            r########"value"########,
            r########"null"########,
        ),
        (
            r########"set-null-last"########,
            r########"Set(null).last"########,
            r########"value"########,
            r########"null"########,
        ),
        (
            r########"set-null-last-or-null"########,
            r########"Set(null).lastOrNull"########,
            r########"value"########,
            r########"null"########,
        ),
        (
            r########"list-single-first"########,
            r########"List(7).first"########,
            r########"value"########,
            r########"7"########,
        ),
        (
            r########"list-single-last"########,
            r########"List(7).last"########,
            r########"value"########,
            r########"7"########,
        ),
        (
            r########"set-many-first"########,
            r########"Set(3, 1, 2).first"########,
            r########"value"########,
            r########"3"########,
        ),
        (
            r########"set-many-first-or-null"########,
            r########"Set(3, 1, 2).firstOrNull"########,
            r########"value"########,
            r########"3"########,
        ),
        (
            r########"set-many-last"########,
            r########"Set(3, 1, 2).last"########,
            r########"value"########,
            r########"2"########,
        ),
        (
            r########"set-many-last-or-null"########,
            r########"Set(3, 1, 2).lastOrNull"########,
            r########"value"########,
            r########"2"########,
        ),
        (
            r########"set-dup-first"########,
            r########"Set(3, 1, 3, 2, 1).first"########,
            r########"value"########,
            r########"3"########,
        ),
        (
            r########"set-dup-first-or-null"########,
            r########"Set(3, 1, 3, 2, 1).firstOrNull"########,
            r########"value"########,
            r########"3"########,
        ),
        (
            r########"set-dup-last"########,
            r########"Set(3, 1, 3, 2, 1).last"########,
            r########"value"########,
            r########"2"########,
        ),
        (
            r########"set-dup-last-or-null"########,
            r########"Set(3, 1, 3, 2, 1).lastOrNull"########,
            r########"value"########,
            r########"2"########,
        ),
        (
            r########"union-list-first"########,
            r########"(Set(1) + List(2, 1, 3)).first"########,
            r########"value"########,
            r########"1"########,
        ),
        (
            r########"union-list-last"########,
            r########"(Set(1) + List(2, 1, 3)).last"########,
            r########"value"########,
            r########"3"########,
        ),
        (
            r########"union-set-first"########,
            r########"(Set(1) + Set(2, 3)).first"########,
            r########"value"########,
            r########"1"########,
        ),
        (
            r########"union-set-last"########,
            r########"(Set(1) + Set(2, 3)).last"########,
            r########"value"########,
            r########"3"########,
        ),
        (
            r########"list-many-first"########,
            r########"List(3, 1, 2).first"########,
            r########"value"########,
            r########"3"########,
        ),
        (
            r########"list-many-last"########,
            r########"List(3, 1, 2).last"########,
            r########"value"########,
            r########"2"########,
        ),
        (
            r########"set-duration-first-unit"########,
            r########"Set(1.min, 60.s).first.unit"########,
            r########"value"########,
            r########"min"########,
        ),
        (
            r########"set-duration-last-unit"########,
            r########"Set(1.min, 60.s).last.unit"########,
            r########"value"########,
            r########"min"########,
        ),
        (
            r########"set-duration-reverse-first-unit"########,
            r########"Set(60.s, 1.min).first.unit"########,
            r########"value"########,
            r########"s"########,
        ),
        (
            r########"set-duration-reverse-last-unit"########,
            r########"Set(60.s, 1.min).last.unit"########,
            r########"value"########,
            r########"s"########,
        ),
        (
            r########"union-duration-left-unit"########,
            r########"(Set(1.min) + Set(60.s)).first.unit"########,
            r########"value"########,
            r########"min"########,
        ),
        (
            r########"union-duration-right-unit"########,
            r########"(Set(60.s) + Set(1.min)).last.unit"########,
            r########"value"########,
            r########"s"########,
        ),
        (
            r########"set-object-first-value"########,
            r########"Set(new Dynamic { value = 1 }).first.value"########,
            r########"value"########,
            r########"1"########,
        ),
        (
            r########"set-object-last-value"########,
            r########"Set(new Dynamic { value = 1 }).last.value"########,
            r########"value"########,
            r########"1"########,
        ),
        (
            r########"set-function-first-apply"########,
            r########"Set((x) -> x + 1).first.apply(2)"########,
            r########"value"########,
            r########"3"########,
        ),
        (
            r########"set-hash-before-first"########,
            r########"Set(new Dynamic { value = throw("hash-first") }).first"########,
            r########"error"########,
            r########"–– Pkl Error ––
hash-first

1 | Set(new Dynamic { value = throw("hash-first") }).first
                              ^^^^^^^^^^^^^^^^^^^
at repl:text.value (repl:text)
"########,
        ),
        (
            r########"set-hash-before-last"########,
            r########"Set(1, new Dynamic { value = throw("hash-last") }).last"########,
            r########"error"########,
            r########"–– Pkl Error ––
hash-last

1 | Set(1, new Dynamic { value = throw("hash-last") }).last
                                 ^^^^^^^^^^^^^^^^^^
at repl:text.value (repl:text)
"########,
        ),
        (
            r########"set-arg-before-empty-check"########,
            r########"Set(throw("argument")).first"########,
            r########"error"########,
            r########"–– Pkl Error ––
argument

1 | Set(throw("argument")).first
        ^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"list-lazy-first-class"########,
            r########"List(new Dynamic { value = throw("nested") }).first.getClass()"########,
            r########"value"########,
            r########"Dynamic"########,
        ),
        (
            r########"list-lazy-first-read"########,
            r########"List(new Dynamic { value = throw("nested") }).first.value"########,
            r########"error"########,
            r########"–– Pkl Error ––
nested

1 | List(new Dynamic { value = throw("nested") }).first.value
                               ^^^^^^^^^^^^^^^
at repl:text.value (repl:text)

1 | List(new Dynamic { value = throw("nested") }).first.value
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"set-empty-first-fallback"########,
            r########"Set().first ?? 9"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected a non-empty collection.
Collection: Set()

1 | Set().first ?? 9
    ^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"set-empty-or-null-fallback"########,
            r########"Set().firstOrNull ?? 9"########,
            r########"value"########,
            r########"9"########,
        ),
        (
            r########"set-first-as-method"########,
            r########"Set(1).first()"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot find method `first` in class `Set`.

1 | Set(1).first()
    ^^^^^^^^^^^^^^
at  (repl:text)

Did you mean any of the following?
first
firstOrNull
findLast(predicate)
"########,
        ),
        (
            r########"set-last-as-method"########,
            r########"Set(1).last()"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot find method `last` in class `Set`.

1 | Set(1).last()
    ^^^^^^^^^^^^^
at  (repl:text)

Did you mean any of the following?
last
lastOrNull
"########,
        ),
        (
            r########"set-first-or-null-as-method"########,
            r########"Set(1).firstOrNull()"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot find method `firstOrNull` in class `Set`.

1 | Set(1).firstOrNull()
    ^^^^^^^^^^^^^^^^^^^^
at  (repl:text)

Did you mean any of the following?
firstOrNull
restOrNull
first
findLastOrNull(predicate)
findOrNull(predicate)
splitOrNull(index)
minWithOrNull(comparator)
maxWithOrNull(comparator)
filterNonNull()
"########,
        ),
        (
            r########"set-last-or-null-as-method"########,
            r########"Set(1).lastOrNull()"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot find method `lastOrNull` in class `Set`.

1 | Set(1).lastOrNull()
    ^^^^^^^^^^^^^^^^^^^
at  (repl:text)

Did you mean any of the following?
lastOrNull
last
splitOrNull(index)
restOrNull
findLastOrNull(predicate)
maxOrNull
maxWithOrNull(comparator)
singleOrNull
maxByOrNull(selector)
"########,
        ),
        (
            r########"set-empty-first-as-method"########,
            r########"Set().first()"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot find method `first` in class `Set`.

1 | Set().first()
    ^^^^^^^^^^^^^
at  (repl:text)

Did you mean any of the following?
first
firstOrNull
findLast(predicate)
"########,
        ),
        (
            r########"set-misspelled-first"########,
            r########"Set(1).frist"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot find property `frist` in object of type `Set`.

1 | Set(1).frist
    ^^^^^^^^^^^^
at  (repl:text)

Did you mean any of the following?
first
findLast(predicate)
rest
firstOrNull
"########,
        ),
        (
            r########"set-method-argument-throw"########,
            r########"Set(1).first(throw("argument"))"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot find method `first` in class `Set`.

1 | Set(1).first(throw("argument"))
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)

Did you mean any of the following?
first.apply(...)
firstOrNull
findLast(predicate)
"########,
        ),
        (
            r########"set-empty-method-argument-throw"########,
            r########"Set().first(throw("argument"))"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot find method `first` in class `Set`.

1 | Set().first(throw("argument"))
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)

Did you mean any of the following?
first.apply(...)
firstOrNull
findLast(predicate)
"########,
        ),
        (
            r########"list-first-as-method"########,
            r########"List(1).first()"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot find method `first` in class `List`.

1 | List(1).first()
    ^^^^^^^^^^^^^^^
at  (repl:text)

Did you mean any of the following?
first
firstOrNull
findLast(predicate)
"########,
        ),
        (
            r########"list-last-as-method"########,
            r########"List(1).last()"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot find method `last` in class `List`.

1 | List(1).last()
    ^^^^^^^^^^^^^^
at  (repl:text)

Did you mean any of the following?
last
lastIndex
lastOrNull
lastIndexOf(element)
lastIndexOfOrNull(element)
"########,
        ),
        (
            r########"union-list-first-or-null"########,
            r########"(Set(1) + List(2, 1, 3)).firstOrNull"########,
            r########"value"########,
            r########"1"########,
        ),
        (
            r########"union-list-last-or-null"########,
            r########"(Set(1) + List(2, 1, 3)).lastOrNull"########,
            r########"value"########,
            r########"3"########,
        ),
        (
            r########"union-set-first-or-null"########,
            r########"(Set(1) + Set(2, 3)).firstOrNull"########,
            r########"value"########,
            r########"1"########,
        ),
        (
            r########"union-set-last-or-null"########,
            r########"(Set(1) + Set(2, 3)).lastOrNull"########,
            r########"value"########,
            r########"3"########,
        ),
        (
            r########"empty-union-first"########,
            r########"(Set() + List()).first"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected a non-empty collection.
Collection: Set()

1 | (Set() + List()).first
    ^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"empty-union-first-or-null"########,
            r########"(Set() + List()).firstOrNull"########,
            r########"value"########,
            r########"null"########,
        ),
        (
            r########"empty-union-last"########,
            r########"(Set() + List()).last"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected a non-empty collection.
Collection: Set()

1 | (Set() + List()).last
    ^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"empty-union-last-or-null"########,
            r########"(Set() + List()).lastOrNull"########,
            r########"value"########,
            r########"null"########,
        ),
        (
            r########"null-many-first-or-null"########,
            r########"Set(null, 1).firstOrNull"########,
            r########"value"########,
            r########"null"########,
        ),
        (
            r########"null-many-last-or-null"########,
            r########"Set(1, null).lastOrNull"########,
            r########"value"########,
            r########"null"########,
        ),
        (
            r########"first-or-null-method-argument-throw"########,
            r########"Set(1).firstOrNull(throw("argument"))"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot find method `firstOrNull` in class `Set`.

1 | Set(1).firstOrNull(throw("argument"))
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)

Did you mean any of the following?
firstOrNull.apply(...)
restOrNull
first
findLastOrNull(predicate)
findOrNull(predicate)
splitOrNull(index)
minWithOrNull(comparator)
maxWithOrNull(comparator)
filterNonNull()
"########,
        ),
        (
            r########"last-or-null-method-argument-throw"########,
            r########"Set(1).lastOrNull(throw("argument"))"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot find method `lastOrNull` in class `Set`.

1 | Set(1).lastOrNull(throw("argument"))
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)

Did you mean any of the following?
lastOrNull.apply(...)
last
splitOrNull(index)
restOrNull
findLastOrNull(predicate)
maxOrNull
maxWithOrNull(comparator)
singleOrNull
maxByOrNull(selector)
"########,
        ),
    ];
    assert_eq!(rows.len(), 70);
    for (id, expression, kind, expected) in rows {
        probe
            .evaluate_module_source(
                "file:///stage63/set-endpoints.pkl",
                Some(include_str!(
                    "../../../tests/fixtures/m2-set-endpoints/module.pkl"
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
