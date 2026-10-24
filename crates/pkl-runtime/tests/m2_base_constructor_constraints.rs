//! Complete independent Java 0.32.0 string observations, capture-06/repeat-0.
//! Authority: m2-base-constructor-constraints/oracle-evidence.tar.gz.
#[path = "support/module_keys.rs"]
mod module_keys;
use pkl_runtime::probe::SourceProbe;
#[test]
fn complete_java_matrix() {
    let mut probe = SourceProbe::new_module_evaluator("compact", false, false, false).unwrap();
    let mut failures = Vec::new();
    let rows = [
(r########"list"########,r########"1 as Int(List(1,2).length == 2)"########,r########"value"########,r########"1"########.encode_utf16().collect::<Vec<_>>()),
(r########"set"########,r########"1 as Int(Set(1,2).length == 2)"########,r########"value"########,r########"1"########.encode_utf16().collect::<Vec<_>>()),
(r########"map"########,r########"1 as Int(Map("a",1).length == 1)"########,r########"value"########,r########"1"########.encode_utf16().collect::<Vec<_>>()),
(r########"bytes"########,r########"1 as Int(Bytes(0,255).length == 2)"########,r########"value"########,r########"1"########.encode_utf16().collect::<Vec<_>>()),
(r########"pair"########,r########"1 as Int(Pair(1,2).first == 1)"########,r########"value"########,r########"1"########.encode_utf16().collect::<Vec<_>>()),
(r########"intseq"########,r########"1 as Int(IntSeq(1,3).start == 1)"########,r########"value"########,r########"1"########.encode_utf16().collect::<Vec<_>>()),
(r########"null-object"########,r########"1 as Int(Null(new Dynamic {}) is Null)"########,r########"value"########,r########"1"########.encode_utf16().collect::<Vec<_>>()),
(r########"null-function"########,r########"1 as Int(Null(() -> new Dynamic {}) is Null)"########,r########"value"########,r########"1"########.encode_utf16().collect::<Vec<_>>()),
(r########"nested-list"########,r########"List(1,2) as List(List(this[0],this[1]).length == 2)"########,r########"value"########,r########"List(1, 2)"########.encode_utf16().collect::<Vec<_>>()),
(r########"nested-set"########,r########"Set(1,2) as Set(Set(this.first,this.last).length == 2)"########,r########"value"########,r########"Set(1, 2)"########.encode_utf16().collect::<Vec<_>>()),
(r########"nested-map"########,r########"List(Map("a",1)) as List(this[0].length == 1)"########,r########"value"########,r########"List(Map("a", 1))"########.encode_utf16().collect::<Vec<_>>()),
(r########"map-odd"########,r########"1 as Int(Map("a").length == 0)"########,r########"error"########,r########"–– Pkl Error ––
Number of `Map()` arguments must be a multiple of two: `Map(key1, value1, key2, value2, ..., keyN, valueN)`

1 | 1 as Int(Map("a").length == 0)
                    ^
at repl:text (repl:text)
"########.encode_utf16().collect::<Vec<_>>()),
(r########"map-odd-before-throw"########,r########"1 as Int(Map("a",throw("value"),"b").length == 1)"########,r########"error"########,r########"–– Pkl Error ––
Number of `Map()` arguments must be a multiple of two: `Map(key1, value1, key2, value2, ..., keyN, valueN)`

1 | 1 as Int(Map("a",throw("value"),"b").length == 1)
                                       ^
at repl:text (repl:text)
"########.encode_utf16().collect::<Vec<_>>()),
(r########"map-even-value-throw"########,r########"1 as Int(Map("a",throw("value")).length == 1)"########,r########"error"########,r########"–– Pkl Error ––
value

1 | 1 as Int(Map("a",throw("value")).length == 1)
                     ^^^^^^^^^^^^^^
at  (repl:text)
"########.encode_utf16().collect::<Vec<_>>()),
(r########"bytes-range"########,r########"1 as Int(Bytes(256).length == 1)"########,r########"error"########,r########"–– Pkl Error ––
Type constraint `isBetween(0, 255)` violated.
Value: 256

    isBetween(0, 255)
    │
    false

1109 | typealias UInt8 = Int(isBetween(0, 255))
                             ^^^^^^^^^^^^^^^^^
at  (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L1109)

1 | 1 as Int(Bytes(256).length == 1)
                   ^^^
at  (repl:text)
"########.encode_utf16().collect::<Vec<_>>()),
(r########"bytes-range-before-throw"########,r########"1 as Int(Bytes(256,throw("second")).length == 2)"########,r########"error"########,r########"–– Pkl Error ––
Type constraint `isBetween(0, 255)` violated.
Value: 256

    isBetween(0, 255)
    │
    false

1109 | typealias UInt8 = Int(isBetween(0, 255))
                             ^^^^^^^^^^^^^^^^^
at  (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L1109)

1 | 1 as Int(Bytes(256,throw("second")).length == 2)
                   ^^^
at  (repl:text)
"########.encode_utf16().collect::<Vec<_>>()),
(r########"bytes-throw-before-range"########,r########"1 as Int(Bytes(throw("first"),256).length == 2)"########,r########"error"########,r########"–– Pkl Error ––
first

1 | 1 as Int(Bytes(throw("first"),256).length == 2)
                   ^^^^^^^^^^^^^^
at  (repl:text)
"########.encode_utf16().collect::<Vec<_>>()),
(r########"pair-arity"########,r########"1 as Int(Pair(1).first == 1)"########,r########"error"########,r########"–– Pkl Error ––
Expected 2 function arguments but got 1.

2383 | external const function Pair<First, Second>(first: First, second: Second): Pair<First, Second>
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#Pair (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L2383)

1 | 1 as Int(Pair(1).first == 1)
             ^^^^^^^
at  (repl:text)
"########.encode_utf16().collect::<Vec<_>>()),
(r########"intseq-type"########,r########"1 as Int(IntSeq("x",3).start == 1)"########,r########"error"########,r########"–– Pkl Error ––
Expected value of type `Int`, but got type `String`.
Value: "x"

2931 | external const function IntSeq(start: Int, end: Int): IntSeq
                                             ^^^
at pkl.base#IntSeq (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L2931)

1 | 1 as Int(IntSeq("x",3).start == 1)
             ^^^^^^^^^^^^^
at  (repl:text)
"########.encode_utf16().collect::<Vec<_>>()),
(r########"null-invalid-type"########,r########"1 as Int(Null(42) is Null)"########,r########"error"########,r########"–– Pkl Error ––
Expected value of type `Object | Function<Object>`, but got type `Int`.
Value: 42

2380 | external const function Null(defaultValue: Object | Function<Object>): Null
                                                  ^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#Null (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L2380)

1 | 1 as Int(Null(42) is Null)
             ^^^^^^^^
at  (repl:text)
"########.encode_utf16().collect::<Vec<_>>()),
(r########"null-arity"########,r########"1 as Int(Null() is Null)"########,r########"error"########,r########"–– Pkl Error ––
Expected 1 function arguments but got 0.

2380 | external const function Null(defaultValue: Object | Function<Object>): Null
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#Null (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L2380)

1 | 1 as Int(Null() is Null)
             ^^^^^^
at  (repl:text)
"########.encode_utf16().collect::<Vec<_>>()),
(r########"null-throw"########,r########"1 as Int(Null(throw("nullarg")) is Null)"########,r########"error"########,r########"–– Pkl Error ––
nullarg

1 | 1 as Int(Null(throw("nullarg")) is Null)
                  ^^^^^^^^^^^^^^^^
at  (repl:text)
"########.encode_utf16().collect::<Vec<_>>()),
(r########"module-qualified"########,r########"module.List(1)"########,r########"value"########,r########"42"########.encode_utf16().collect::<Vec<_>>()),
(r########"constraint-qualified"########,r########"1 as Int(module.List(1) == 42)"########,r########"value"########,r########"1"########.encode_utf16().collect::<Vec<_>>()),
(r########"constraint-unqualified"########,r########"1 as Int(List(1) == 42)"########,r########"error"########,r########"–– Pkl Error ––
Type constraint `List(1) == 42` violated.
Value: 1

1 | 1 as Int(List(1) == 42)
             ^^^^^^^^^^^^^
at  (repl:text)
"########.encode_utf16().collect::<Vec<_>>()),
(r########"constraint-unqualified-is-list"########,r########"1 as Int(List(1) is List)"########,r########"value"########,r########"1"########.encode_utf16().collect::<Vec<_>>()),
(r########"let-unqualified"########,r########"1 as Int(let (List = (x) -> 42) List(1) == 42)"########,r########"error"########,r########"–– Pkl Error ––
Type constraint `let (List = (x) -> 42) List(1) == 42` violated.
Value: 1

1 | 1 as Int(let (List = (x) -> 42) List(1) == 42)
             ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########.encode_utf16().collect::<Vec<_>>()),
(r########"let-qualified"########,r########"1 as Int(let (List = (x) -> 42) module.List(1) == 42)"########,r########"value"########,r########"1"########.encode_utf16().collect::<Vec<_>>()),
(r########"authored-public-direct"########,r########"direct"########,r########"value"########,r########"42"########.encode_utf16().collect::<Vec<_>>()),
(r########"authored-public-constraint"########,r########"constrained"########,r########"value"########,r########"1"########.encode_utf16().collect::<Vec<_>>()),
(r########"authored-public-is-list"########,r########"constrainedIsList"########,r########"error"########,r########"–– Pkl Error ––
Type constraint `List(1) is List` violated.
Value: 1

11 | constrainedIsList = 1 as Int(List(1) is List)
                                  ^^^^^^^^^^^^^^^
at base-constructors-in-constraints#constrainedIsList (file:///stage66/base-constructors-in-constraints.pkl)

1 | constrainedIsList
    ^^^^^^^^^^^^^^^^^
at  (repl:text)
"########.encode_utf16().collect::<Vec<_>>()),
(r########"authored-class-local-direct"########,r########"new LocalHolder {}.direct"########,r########"value"########,r########"43"########.encode_utf16().collect::<Vec<_>>()),
(r########"authored-class-local-constraint"########,r########"new LocalHolder {}.constrained"########,r########"value"########,r########"1"########.encode_utf16().collect::<Vec<_>>()),
(r########"authored-class-local-is-list"########,r########"new LocalHolder {}.constrainedIsList"########,r########"error"########,r########"–– Pkl Error ––
Type constraint `List(1) is List` violated.
Value: 1

9 | constrainedIsList = 1 as Int(List(1) is List)
                                 ^^^^^^^^^^^^^^^
at base-constructors-in-constraints#LocalHolder.constrainedIsList (file:///stage66/base-constructors-in-constraints.pkl)

1 | new LocalHolder {}.constrainedIsList
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########.encode_utf16().collect::<Vec<_>>()),
(r########"external-list-direct"########,r########"List(1)"########,r########"value"########,r########"List(1)"########.encode_utf16().collect::<Vec<_>>()),
(r########"external-class-local-call"########,r########"new LocalHolder {}.List(1)"########,r########"error"########,r########"–– Pkl Error ––
Cannot find method `List` in class `base-constructors-in-constraints#LocalHolder`.

1 | new LocalHolder {}.List(1)
    ^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)

Available methods in class `base-constructors-in-constraints#LocalHolder`:
getClass
getProperty
getPropertyOrNull
hasProperty
ifNonNull
toDynamic
toMap
toString
"########.encode_utf16().collect::<Vec<_>>()),
    ];
    assert_eq!(rows.len(), 36);
    for (id, expression, kind, expected) in rows {
        probe
            .evaluate_module_source(
                "file:///stage66/base-constructors-in-constraints.pkl",
                Some(include_str!(
                    "../../../tests/fixtures/m2-base-constructor-constraints/module.pkl"
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
