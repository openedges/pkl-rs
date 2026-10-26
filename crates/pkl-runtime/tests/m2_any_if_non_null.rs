//! Complete string API observations acquired from the pinned Java 0.32.0 oracle.
//! Expected payloads below are copied from stage58-any-oracle-04/repeat-0,
//! whose independently executed repeat-1 matches; the sealed authority is
//! tests/fixtures/m2-any-if-non-null/oracle-evidence.tar.gz.
#[path = "support/module_keys.rs"]
mod module_keys;

use pkl_runtime::probe::{SourceEvent, SourceProbe};

fn query(probe: &mut SourceProbe, expression: &str) -> SourceEvent {
    probe
        .evaluate_module_source(
            "file:///stage58/any-if-non-null.pkl",
            Some(include_str!(
                "../../../tests/fixtures/m2-any-if-non-null/module.pkl"
            )),
            expression,
            0,
            None,
        )
        .unwrap();
    for _ in 0..200_000 {
        probe.collect();
        let event = probe.poll(0, 1).unwrap();
        probe.collect();
        if event.kind == "yield" {
            continue;
        }
        if event.kind == "module-request"
            && module_keys::complete_probe(probe, event.token.as_deref().unwrap(), 0).unwrap()
        {
            continue;
        }
        return event;
    }
    panic!("Any.ifNonNull public-query poll bound")
}

fn compare(rows: &[(&str, &str, &str, &str)]) {
    let mut probe = SourceProbe::new_module_evaluator("compact", false, false, false).unwrap();
    let mut mismatches = Vec::new();
    for &(id, expression, kind, expected) in rows {
        let event = query(&mut probe, expression);
        let actual_kind = event.kind;
        let actual = if actual_kind == "error" {
            event.error
        } else {
            event.value
        };
        if actual_kind != kind
            || actual.as_deref() != Some(expected.encode_utf16().collect::<Vec<_>>().as_slice())
        {
            mismatches.push(format!(
                "{id}: expected {kind} {expected:?}; actual {actual_kind} {:?}",
                actual.map(|text| String::from_utf16_lossy(&text))
            ));
        }
    }
    probe.close();
    assert_eq!(probe.collect().slots, 0);
    assert!(mismatches.is_empty(), "{}", mismatches.join("\n\n"));
}

#[test]
fn inherited_dispatch_null_argument_checks_and_complete_frames_match_java() {
    compare(&[
        (
            r####"scalar-int-identity"####,
            r####"10.ifNonNull((x) -> x)"####,
            "value",
            r####"10"####,
        ),
        (
            r####"scalar-int-transform"####,
            r####"10.ifNonNull((x) -> x + 2)"####,
            "value",
            r####"12"####,
        ),
        (
            r####"scalar-int-null-result"####,
            r####"10.ifNonNull((x) -> null)"####,
            "value",
            r####"null"####,
        ),
        (
            r####"scalar-float"####,
            r####"1.5.ifNonNull((x) -> x + 0.5)"####,
            "value",
            r####"2.0"####,
        ),
        (
            r####"scalar-boolean"####,
            r####"true.ifNonNull((x) -> !x)"####,
            "value",
            r####"false"####,
        ),
        (
            r####"scalar-string"####,
            r####""abc".ifNonNull((x) -> x.length)"####,
            "value",
            r####"3"####,
        ),
        (
            r####"scalar-duration"####,
            r####"2.s.ifNonNull((x) -> x.value)"####,
            "value",
            r####"2"####,
        ),
        (
            r####"scalar-datasize"####,
            r####"3.mb.ifNonNull((x) -> x.unit)"####,
            "value",
            r####"mb"####,
        ),
        (
            r####"scalar-bytes"####,
            r####"Bytes(1, 2, 3).ifNonNull((x) -> x.length)"####,
            "value",
            r####"3"####,
        ),
        (
            r####"scalar-pair"####,
            r####"Pair("a", 4).ifNonNull((x) -> x.second)"####,
            "value",
            r####"4"####,
        ),
        (
            r####"null-plain"####,
            r####"null.ifNonNull((x) -> x)"####,
            "value",
            r####"null"####,
        ),
        (
            r####"null-plain-constant"####,
            r####"null.ifNonNull((x) -> "unreachable")"####,
            "value",
            r####"null"####,
        ),
        (
            r####"null-plain-throw"####,
            r####"null.ifNonNull((x) -> throw("unreachable"))"####,
            "value",
            r####"null"####,
        ),
        (
            r####"null-default-identity"####,
            r####"Null(new Dynamic { marker = "default" }).ifNonNull((x) -> x)"####,
            "value",
            r####"null"####,
        ),
        (
            r####"null-default-constant"####,
            r####"Null(new Dynamic { marker = "default" }).ifNonNull((x) -> "unreachable")"####,
            "value",
            r####"null"####,
        ),
        (
            r####"null-default-throw"####,
            r####"Null(new Dynamic { marker = "default" }).ifNonNull((x) -> throw("unreachable"))"####,
            "value",
            r####"null"####,
        ),
        (
            r####"null-default-coalesce"####,
            r####"Null(new Dynamic { marker = "default" }).ifNonNull((x) -> x) ?? "fallback""####,
            "value",
            r####"fallback"####,
        ),
        (
            r####"null-default-amend"####,
            r####"((Null(new Dynamic { marker = "base" }).ifNonNull((x) -> x)) { extra = 7 }).extra"####,
            "value",
            r####"7"####,
        ),
        (
            r####"null-default-equality"####,
            r####"Null(new Dynamic { marker = "default" }).ifNonNull((x) -> x) == null"####,
            "value",
            r####"true"####,
        ),
        (
            r####"null-default-lambda"####,
            r####"Null(() -> new Dynamic { marker = "deferred" }).ifNonNull((x) -> "unreachable")"####,
            "value",
            r####"null"####,
        ),
        (
            r####"null-default-lambda-amend"####,
            r####"((Null(() -> new Dynamic { marker = "deferred" }).ifNonNull((x) -> x)) { extra = 8 }).extra"####,
            "error",
            r####"–– Pkl Error ––
Cannot find property `extra` in object of type `Function0`.

1 | ((Null(() -> new Dynamic { marker = "deferred" }).ifNonNull((x) -> x)) { extra = 8 }).extra
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)

Available properties:
(none)
"####,
        ),
        (
            r####"null-nested"####,
            r####"null.ifNonNull((x) -> x).ifNonNull((x) -> "bad")"####,
            "value",
            r####"null"####,
        ),
        (
            r####"collections-list"####,
            r####"List(1, 2).ifNonNull((x) -> x.length)"####,
            "value",
            r####"2"####,
        ),
        (
            r####"collections-set"####,
            r####"Set(1, 2).ifNonNull((x) -> x.length)"####,
            "value",
            r####"2"####,
        ),
        (
            r####"collections-map"####,
            r####"Map("a", 1).ifNonNull((x) -> x["a"])"####,
            "value",
            r####"1"####,
        ),
        (
            r####"collections-listing"####,
            r####"(new Listing { 1; 2 }).ifNonNull((x) -> x.length)"####,
            "value",
            r####"2"####,
        ),
        (
            r####"collections-mapping"####,
            r####"(new Mapping { ["a"] = 1 }).ifNonNull((x) -> x["a"])"####,
            "value",
            r####"1"####,
        ),
        (
            r####"collections-dynamic"####,
            r####"(new Dynamic { n = 7 }).ifNonNull((x) -> x.n)"####,
            "value",
            r####"7"####,
        ),
        (
            r####"collections-typed-object"####,
            r####"(new Box {}).ifNonNull((x) -> x.value)"####,
            "value",
            r####"7"####,
        ),
        (
            r####"collections-module"####,
            r####"module.ifNonNull((x) -> x.marker)"####,
            "value",
            r####"stage58"####,
        ),
        (
            r####"metavalues-class-value"####,
            r####"Box.ifNonNull((x) -> x.simpleName)"####,
            "value",
            r####"Box"####,
        ),
        (
            r####"metavalues-typealias-value"####,
            r####"BoxAlias.ifNonNull((x) -> x.toString())"####,
            "value",
            r####"any-if-non-null#BoxAlias"####,
        ),
        (
            r####"metavalues-function-value"####,
            r####"((x) -> x + 1).ifNonNull((f) -> f.apply(4))"####,
            "value",
            r####"5"####,
        ),
        (
            r####"metavalues-typed-function-value"####,
            r####"typedTransform.ifNonNull((f) -> f.apply(4))"####,
            "value",
            r####"5"####,
        ),
        (
            r####"metavalues-reference-type"####,
            r####"import("pkl:ref").Reference.ifNonNull((x) -> x.simpleName)"####,
            "value",
            r####"Reference"####,
        ),
        (
            r####"metavalues-reference-instance"####,
            r####"referenceValue.ifNonNull((x) -> x.getPath().length)"####,
            "value",
            r####"0"####,
        ),
        (
            r####"metavalues-reference-path"####,
            r####"referenceValue.ifNonNull((x) -> x.value.getPath().length)"####,
            "value",
            r####"1"####,
        ),
        (
            r####"types-and-arity-int-wrong-param"####,
            r####"10.ifNonNull((x: String) -> x)"####,
            "error",
            r####"–– Pkl Error ––
Expected value of type `String`, but got type `Int`.
Value: 10

1 | 10.ifNonNull((x: String) -> x)
                     ^^^^^^
at repl:text.<function#1> (repl:text)

1 | 10.ifNonNull((x: String) -> x)
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"####,
        ),
        (
            r####"types-and-arity-null-wrong-param"####,
            r####"null.ifNonNull((x: String) -> x)"####,
            "value",
            r####"null"####,
        ),
        (
            r####"types-and-arity-default-wrong-param"####,
            r####"Null(new Dynamic { n = 3 }).ifNonNull((x: String) -> x)"####,
            "value",
            r####"null"####,
        ),
        (
            r####"types-and-arity-nonfunction"####,
            r####"10.ifNonNull(4)"####,
            "error",
            r####"–– Pkl Error ––
Expected value of type `Function1`, but got type `Int`.
Value: 4

60 | external function ifNonNull<Result>(transform: (NonNull) -> Result): Result?
                                                    ^^^^^^^^^^^^^^^^^^^
at pkl.base#Any.ifNonNull (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L60)

1 | 10.ifNonNull(4)
    ^^^^^^^^^^^^^^^
at  (repl:text)
"####,
        ),
        (
            r####"types-and-arity-null-nonfunction"####,
            r####"null.ifNonNull(4)"####,
            "error",
            r####"–– Pkl Error ––
Expected value of type `Function1`, but got type `Int`.
Value: 4

60 | external function ifNonNull<Result>(transform: (NonNull) -> Result): Result?
                                                    ^^^^^^^^^^^^^^^^^^^
at pkl.base#Any.ifNonNull (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L60)

1 | null.ifNonNull(4)
    ^^^^^^^^^^^^^^^^^
at  (repl:text)
"####,
        ),
        (
            r####"types-and-arity-default-nonfunction"####,
            r####"Null(new Dynamic { n = 3 }).ifNonNull(4)"####,
            "error",
            r####"–– Pkl Error ––
Expected value of type `Function1`, but got type `Int`.
Value: 4

60 | external function ifNonNull<Result>(transform: (NonNull) -> Result): Result?
                                                    ^^^^^^^^^^^^^^^^^^^
at pkl.base#Any.ifNonNull (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L60)

1 | Null(new Dynamic { n = 3 }).ifNonNull(4)
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"####,
        ),
        (
            r####"types-and-arity-no-args"####,
            r####"10.ifNonNull()"####,
            "error",
            r####"–– Pkl Error ––
Expected 1 function arguments but got 0.

60 | external function ifNonNull<Result>(transform: (NonNull) -> Result): Result?
     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#Any.ifNonNull (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L60)

1 | 10.ifNonNull()
    ^^^^^^^^^^^^^^
at  (repl:text)
"####,
        ),
        (
            r####"types-and-arity-null-no-args"####,
            r####"null.ifNonNull()"####,
            "error",
            r####"–– Pkl Error ––
Expected 1 function arguments but got 0.

60 | external function ifNonNull<Result>(transform: (NonNull) -> Result): Result?
     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#Any.ifNonNull (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L60)

1 | null.ifNonNull()
    ^^^^^^^^^^^^^^^^
at  (repl:text)
"####,
        ),
        (
            r####"types-and-arity-too-many"####,
            r####"10.ifNonNull((x) -> x, 4)"####,
            "error",
            r####"–– Pkl Error ––
Expected 1 function arguments but got 2.

60 | external function ifNonNull<Result>(transform: (NonNull) -> Result): Result?
     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#Any.ifNonNull (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L60)

1 | 10.ifNonNull((x) -> x, 4)
    ^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"####,
        ),
        (
            r####"types-and-arity-null-too-many"####,
            r####"null.ifNonNull((x) -> x, 4)"####,
            "error",
            r####"–– Pkl Error ––
Expected 1 function arguments but got 2.

60 | external function ifNonNull<Result>(transform: (NonNull) -> Result): Result?
     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#Any.ifNonNull (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L60)

1 | null.ifNonNull((x) -> x, 4)
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"####,
        ),
        (
            r####"types-and-arity-null-too-many-throw"####,
            r####"null.ifNonNull((x) -> x, throw("second argument"))"####,
            "error",
            r####"–– Pkl Error ––
second argument

1 | null.ifNonNull((x) -> x, throw("second argument"))
                             ^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"####,
        ),
        (
            r####"types-and-arity-value-too-many-throw"####,
            r####"10.ifNonNull((x) -> x, throw("second argument"))"####,
            "error",
            r####"–– Pkl Error ––
second argument

1 | 10.ifNonNull((x) -> x, throw("second argument"))
                           ^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"####,
        ),
        (
            r####"types-and-arity-zero-param"####,
            r####"10.ifNonNull(() -> 9)"####,
            "error",
            r####"–– Pkl Error ––
Expected value of type `Function1`, but got type `Function0`.
Value: new Function0 {}

60 | external function ifNonNull<Result>(transform: (NonNull) -> Result): Result?
                                                    ^^^^^^^^^^^^^^^^^^^
at pkl.base#Any.ifNonNull (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L60)

1 | 10.ifNonNull(() -> 9)
    ^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"####,
        ),
        (
            r####"types-and-arity-null-zero-param"####,
            r####"null.ifNonNull(() -> 9)"####,
            "error",
            r####"–– Pkl Error ––
Expected value of type `Function1`, but got type `Function0`.
Value: new Function0 {}

60 | external function ifNonNull<Result>(transform: (NonNull) -> Result): Result?
                                                    ^^^^^^^^^^^^^^^^^^^
at pkl.base#Any.ifNonNull (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L60)

1 | null.ifNonNull(() -> 9)
    ^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"####,
        ),
        (
            r####"types-and-arity-two-param"####,
            r####"10.ifNonNull((x, y) -> x)"####,
            "error",
            r####"–– Pkl Error ––
Expected value of type `Function1`, but got type `Function2`.
Value: new Function2 {}

60 | external function ifNonNull<Result>(transform: (NonNull) -> Result): Result?
                                                    ^^^^^^^^^^^^^^^^^^^
at pkl.base#Any.ifNonNull (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L60)

1 | 10.ifNonNull((x, y) -> x)
    ^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"####,
        ),
        (
            r####"types-and-arity-null-two-param"####,
            r####"null.ifNonNull((x, y) -> x)"####,
            "error",
            r####"–– Pkl Error ––
Expected value of type `Function1`, but got type `Function2`.
Value: new Function2 {}

60 | external function ifNonNull<Result>(transform: (NonNull) -> Result): Result?
                                                    ^^^^^^^^^^^^^^^^^^^
at pkl.base#Any.ifNonNull (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L60)

1 | null.ifNonNull((x, y) -> x)
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"####,
        ),
        (
            r####"types-and-arity-typed-return-value"####,
            r####"typedTransform.ifNonNull((f) -> f.apply(4))"####,
            "value",
            r####"5"####,
        ),
        (
            r####"types-and-arity-typed-return-null"####,
            r####"null.ifNonNull((x: Int) -> x + 1)"####,
            "value",
            r####"null"####,
        ),
        (
            r####"evaluation-order-null-argument-throw"####,
            r####"null.ifNonNull(throw("argument first"))"####,
            "error",
            r####"–– Pkl Error ––
argument first

1 | null.ifNonNull(throw("argument first"))
                   ^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"####,
        ),
        (
            r####"evaluation-order-value-argument-throw"####,
            r####"10.ifNonNull(throw("argument first"))"####,
            "error",
            r####"–– Pkl Error ––
argument first

1 | 10.ifNonNull(throw("argument first"))
                 ^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"####,
        ),
        (
            r####"evaluation-order-null-receiver-throw"####,
            r####"throw("receiver first").ifNonNull(throw("argument second"))"####,
            "error",
            r####"–– Pkl Error ––
receiver first

1 | throw("receiver first").ifNonNull(throw("argument second"))
    ^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"####,
        ),
        (
            r####"evaluation-order-nested-first-error"####,
            r####"10.ifNonNull((x) -> throw("transform first")).ifNonNull(throw("argument second"))"####,
            "error",
            r####"–– Pkl Error ––
transform first

1 | 10.ifNonNull((x) -> throw("transform first")).ifNonNull(throw("argument second"))
                        ^^^^^^^^^^^^^^^^^^^^^^^^
at repl:text.<function#1> (repl:text)

1 | 10.ifNonNull((x) -> throw("transform first")).ifNonNull(throw("argument second"))
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"####,
        ),
        (
            r####"evaluation-order-null-deferred-property"####,
            r####"null.ifNonNull((x) -> lazyBomb)"####,
            "value",
            r####"null"####,
        ),
        (
            r####"evaluation-order-value-forced-property"####,
            r####"10.ifNonNull((x) -> lazyBomb)"####,
            "error",
            r####"–– Pkl Error ––
lazy property boom

13 | lazyBomb = throw("lazy property boom")
                ^^^^^^^^^^^^^^^^^^^^^^^^^^^
at any-if-non-null#lazyBomb (file:///stage58/any-if-non-null.pkl)

1 | 10.ifNonNull((x) -> lazyBomb)
                        ^^^^^^^^
at repl:text.<function#1> (repl:text)

1 | 10.ifNonNull((x) -> lazyBomb)
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"####,
        ),
        (
            r####"evaluation-order-null-typed-function"####,
            r####"null.ifNonNull(typedTransform)"####,
            "value",
            r####"null"####,
        ),
        (
            r####"evaluation-order-value-typed-function"####,
            r####"10.ifNonNull(typedTransform)"####,
            "value",
            r####"11"####,
        ),
        (
            r####"evaluation-order-null-closure-throw"####,
            r####"null.ifNonNull((x) -> throw("closure boom"))"####,
            "value",
            r####"null"####,
        ),
        (
            r####"evaluation-order-value-closure-throw"####,
            r####"10.ifNonNull((x) -> throw("closure boom"))"####,
            "error",
            r####"–– Pkl Error ––
closure boom

1 | 10.ifNonNull((x) -> throw("closure boom"))
                        ^^^^^^^^^^^^^^^^^^^^^
at repl:text.<function#1> (repl:text)

1 | 10.ifNonNull((x) -> throw("closure boom"))
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"####,
        ),
        (
            r####"evaluation-order-retry-after-error"####,
            r####"10.ifNonNull((x) -> x + 1)"####,
            "value",
            r####"11"####,
        ),
        (
            r####"evaluation-order-repeat-same-receiver"####,
            r####"holder.value.ifNonNull((x) -> x + 1)"####,
            "value",
            r####"11"####,
        ),
        (
            r####"evaluation-order-repeat-other-result"####,
            r####"holder.value.ifNonNull((x) -> x + 2)"####,
            "value",
            r####"12"####,
        ),
        (
            r####"dispatch-dynamic-custom-method"####,
            r####"(new Dynamic { local function ifNonNull(f) = "custom" }).ifNonNull((x) -> "base")"####,
            "value",
            r####"base"####,
        ),
        (
            r####"dispatch-dynamic-shadow-property"####,
            r####"(new Dynamic { ifNonNull = "shadow" }).ifNonNull((x) -> "base")"####,
            "value",
            r####"base"####,
        ),
        (
            r####"dispatch-object-custom-method"####,
            r####"(new BoxWithMethod {}).ifNonNull((x) -> "base")"####,
            "value",
            r####"override"####,
        ),
        (
            r####"dispatch-object-custom-wrong-arity"####,
            r####"(new BoxWithMethod {}).ifNonNull()"####,
            "error",
            r####"–– Pkl Error ––
Expected 1 function arguments but got 0.

8 | open class BoxWithMethod { function ifNonNull(f) = "override" }
                               ^^^^^^^^^^^^^^^^^^^^^
at any-if-non-null#BoxWithMethod.ifNonNull (file:///stage58/any-if-non-null.pkl)

1 | (new BoxWithMethod {}).ifNonNull()
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"####,
        ),
        (
            r####"dispatch-object-super-method"####,
            r####"(new ChildMethod {}).ifNonNull((x) -> "base")"####,
            "value",
            r####"override:child"####,
        ),
        (
            r####"dispatch-object-super-any"####,
            r####"(new DelegatingMethod {}).ifNonNull((x) -> x.value)"####,
            "value",
            r####"7"####,
        ),
        (
            r####"dispatch-implicit-module-method"####,
            r####"ifNonNull((x) -> x.marker)"####,
            "value",
            r####"stage58"####,
        ),
        (
            r####"dispatch-object-method-throw-arg"####,
            r####"(new BoxWithMethod {}).ifNonNull(throw("custom argument"))"####,
            "error",
            r####"–– Pkl Error ––
custom argument

1 | (new BoxWithMethod {}).ifNonNull(throw("custom argument"))
                                     ^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"####,
        ),
        (
            r####"dispatch-generic-on-any"####,
            r####"(10 as Any).ifNonNull((x) -> x.toString())"####,
            "value",
            r####"10"####,
        ),
        (
            r####"const-literal-property"####,
            r####"constLiteral"####,
            "value",
            r####"1"####,
        ),
        (
            r####"const-module-implicit"####,
            r####"constImplicit()"####,
            "value",
            r####"stage58"####,
        ),
        (
            r####"const-module-explicit"####,
            r####"constExplicit()"####,
            "error",
            r####"–– Pkl Error ––
Cannot call method `ifNonNull` from here because it is not `const`.

17 | const function constExplicit() = module.ifNonNull((x) -> x.marker)
                                      ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at any-if-non-null#constExplicit (file:///stage58/any-if-non-null.pkl)

To fix, do either of:
1. Add modifier `const` to method `ifNonNull`
2. Self-import this module, and reference this method from the import.

1 | constExplicit()
    ^^^^^^^^^^^^^^^
at  (repl:text)
"####,
        ),
        (
            r####"const-class-inherited"####,
            r####"(new ConstMethod {}).run()"####,
            "value",
            r####"9"####,
        ),
        (
            r####"const-class-shadow"####,
            r####"(new ConstShadow {}).run()"####,
            "value",
            r####"const shadow"####,
        ),
        (
            r####"const-class-shadow-direct"####,
            r####"(new ConstShadow {}).ifNonNull((x) -> x)"####,
            "value",
            r####"const shadow"####,
        ),
        (
            r####"const-class-ordinary-direct"####,
            r####"(new ConstMethod {}).ifNonNull((x) -> x.value)"####,
            "value",
            r####"9"####,
        ),
    ]);
}

#[test]
fn inherited_method_on_regex_retains_original_java_requirement() {
    compare(&[(
        r####"scalar-regex"####,
        r####"Regex("a+").ifNonNull((x) -> x.toString())"####,
        "value",
        r####"Regex("a+")"####,
    )]);
}

#[test]
fn inherited_method_on_intseq_retains_original_java_requirement() {
    compare(&[(
        r####"scalar-intseq"####,
        r####"IntSeq(1, 3).ifNonNull((x) -> x.length)"####,
        "error",
        r####"–– Pkl Error ––
Cannot find property `length` in object of type `IntSeq`.

1 | IntSeq(1, 3).ifNonNull((x) -> x.length)
                                  ^^^^^^^^
at repl:text.<function#1> (repl:text)

Available properties:
end
start
step

1 | IntSeq(1, 3).ifNonNull((x) -> x.length)
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"####,
    )]);
}
