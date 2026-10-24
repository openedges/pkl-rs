//! Complete independent Java 0.32.0 string observations, capture-04/repeat-0.
//! Authority: m2-collection-addition/oracle-evidence.tar.gz (125 ordered rows).
#[path = "support/module_keys.rs"]
mod module_keys;
use pkl_runtime::probe::SourceProbe;
#[test]
fn complete_java_collection_addition_matrix() {
    let mut probe = SourceProbe::new_module_evaluator("compact", false, false, false).unwrap();
    let mut failures = Vec::new();
    let rows = [
        (
            r########"ll-ordered"########,
            r########"List(1, 2) + List(3, 4)"########,
            r########"value"########,
            r########"List(1, 2, 3, 4)"########,
        ),
        (
            r########"ls-ordered"########,
            r########"List(1, 2) + Set(2, 3)"########,
            r########"value"########,
            r########"List(1, 2, 2, 3)"########,
        ),
        (
            r########"ls-repeat"########,
            r########"List(1, 1) + Set(1, 2)"########,
            r########"value"########,
            r########"List(1, 1, 1, 2)"########,
        ),
        (
            r########"ll-empty-both"########,
            r########"List() + List()"########,
            r########"value"########,
            r########"List()"########,
        ),
        (
            r########"ll-empty-right"########,
            r########"List(1, 2) + List()"########,
            r########"value"########,
            r########"List(1, 2)"########,
        ),
        (
            r########"ll-empty-left"########,
            r########"List() + List(1, 2)"########,
            r########"value"########,
            r########"List(1, 2)"########,
        ),
        (
            r########"ls-empty-right"########,
            r########"List(1, 2) + Set()"########,
            r########"value"########,
            r########"List(1, 2)"########,
        ),
        (
            r########"ls-empty-left"########,
            r########"List() + Set(1, 2)"########,
            r########"value"########,
            r########"List(1, 2)"########,
        ),
        (
            r########"ll-chain"########,
            r########"List(1) + List(2) + List(3)"########,
            r########"value"########,
            r########"List(1, 2, 3)"########,
        ),
        (
            r########"ls-chain"########,
            r########"List(1) + Set(2, 1) + List(3)"########,
            r########"value"########,
            r########"List(1, 2, 1, 3)"########,
        ),
        (
            r########"ls-result-class"########,
            r########"(List(1) + Set(2)).getClass()"########,
            r########"value"########,
            r########"List"########,
        ),
        (
            r########"ls-result-first"########,
            r########"(List(1) + Set(2)).first"########,
            r########"value"########,
            r########"1"########,
        ),
        (
            r########"ls-result-last"########,
            r########"(List(1) + Set(2)).last"########,
            r########"value"########,
            r########"2"########,
        ),
        (
            r########"ls-equal-list"########,
            r########"List(1, 2) + Set(3) == List(1, 2, 3)"########,
            r########"value"########,
            r########"true"########,
        ),
        (
            r########"ls-equal-set"########,
            r########"List(1, 2) + Set(3) == Set(1, 2, 3)"########,
            r########"value"########,
            r########"false"########,
        ),
        (
            r########"ss-ordered"########,
            r########"Set(1, 2) + Set(2, 3)"########,
            r########"value"########,
            r########"Set(1, 2, 3)"########,
        ),
        (
            r########"sl-ordered"########,
            r########"Set(1, 2) + List(2, 3)"########,
            r########"value"########,
            r########"Set(1, 2, 3)"########,
        ),
        (
            r########"sl-repeat"########,
            r########"Set(1) + List(2, 1, 2, 3, 1)"########,
            r########"value"########,
            r########"Set(1, 2, 3)"########,
        ),
        (
            r########"ss-empty-both"########,
            r########"Set() + Set()"########,
            r########"value"########,
            r########"Set()"########,
        ),
        (
            r########"ss-empty-right"########,
            r########"Set(1, 2) + Set()"########,
            r########"value"########,
            r########"Set(1, 2)"########,
        ),
        (
            r########"ss-empty-left"########,
            r########"Set() + Set(1, 2)"########,
            r########"value"########,
            r########"Set(1, 2)"########,
        ),
        (
            r########"sl-empty-right"########,
            r########"Set(1, 2) + List()"########,
            r########"value"########,
            r########"Set(1, 2)"########,
        ),
        (
            r########"sl-empty-left"########,
            r########"Set() + List(1, 2)"########,
            r########"value"########,
            r########"Set(1, 2)"########,
        ),
        (
            r########"ss-chain"########,
            r########"Set(1, 2) + Set(3) + Set(1, 4)"########,
            r########"value"########,
            r########"Set(1, 2, 3, 4)"########,
        ),
        (
            r########"sl-chain"########,
            r########"Set(1) + List(2, 1) + Set(3, 2)"########,
            r########"value"########,
            r########"Set(1, 2, 3)"########,
        ),
        (
            r########"sl-result-class"########,
            r########"(Set(1) + List(2)).getClass()"########,
            r########"value"########,
            r########"Set"########,
        ),
        (
            r########"sl-result-first"########,
            r########"(Set(1) + List(2)).first"########,
            r########"value"########,
            r########"1"########,
        ),
        (
            r########"sl-result-last"########,
            r########"(Set(1) + List(2)).last"########,
            r########"value"########,
            r########"2"########,
        ),
        (
            r########"sl-equal-set"########,
            r########"Set(1, 2) + List(3) == Set(3, 2, 1)"########,
            r########"value"########,
            r########"true"########,
        ),
        (
            r########"sl-equal-list"########,
            r########"Set(1, 2) + List(3) == List(1, 2, 3)"########,
            r########"value"########,
            r########"false"########,
        ),
        (
            r########"mm-ordered"########,
            r########"Map("a", 1, "b", 2) + Map("c", 3, "d", 4)"########,
            r########"value"########,
            r########"Map("a", 1, "b", 2, "c", 3, "d", 4)"########,
        ),
        (
            r########"mm-overwrite-first"########,
            r########"Map("a", 1, "b", 2) + Map("a", 9, "c", 3)"########,
            r########"value"########,
            r########"Map("a", 9, "b", 2, "c", 3)"########,
        ),
        (
            r########"mm-overwrite-last"########,
            r########"Map("a", 1, "b", 2) + Map("c", 3, "b", 9)"########,
            r########"value"########,
            r########"Map("a", 1, "b", 9, "c", 3)"########,
        ),
        (
            r########"mm-overwrite-both"########,
            r########"Map("a", 1, "b", 2) + Map("b", 9, "a", 8)"########,
            r########"value"########,
            r########"Map("a", 8, "b", 9)"########,
        ),
        (
            r########"mm-empty-both"########,
            r########"Map() + Map()"########,
            r########"value"########,
            r########"Map()"########,
        ),
        (
            r########"mm-empty-right"########,
            r########"Map("a", 1) + Map()"########,
            r########"value"########,
            r########"Map("a", 1)"########,
        ),
        (
            r########"mm-empty-left"########,
            r########"Map() + Map("a", 1)"########,
            r########"value"########,
            r########"Map("a", 1)"########,
        ),
        (
            r########"mm-chain"########,
            r########"Map("a", 1) + Map("b", 2, "a", 3) + Map("b", 4, "c", 5)"########,
            r########"value"########,
            r########"Map("a", 3, "b", 4, "c", 5)"########,
        ),
        (
            r########"mm-result-class"########,
            r########"(Map("a", 1) + Map("b", 2)).getClass()"########,
            r########"value"########,
            r########"Map"########,
        ),
        (
            r########"mm-equal-reordered"########,
            r########"Map("a", 1, "b", 2) + Map("c", 3) == Map("c", 3, "b", 2, "a", 1)"########,
            r########"value"########,
            r########"true"########,
        ),
        (
            r########"mm-overwrite-lookup"########,
            r########"(Map("a", 1) + Map("a", 9))["a"]"########,
            r########"value"########,
            r########"9"########,
        ),
        (
            r########"mm-overwrite-length"########,
            r########"(Map("a", 1) + Map("a", 9)).length"########,
            r########"value"########,
            r########"1"########,
        ),
        (
            r########"set-int-float"########,
            r########"Set(1) + Set(1.0)"########,
            r########"value"########,
            r########"Set(1, 1.0)"########,
        ),
        (
            r########"set-int-float-reversed"########,
            r########"Set(1.0) + Set(1)"########,
            r########"value"########,
            r########"Set(1.0, 1)"########,
        ),
        (
            r########"set-float-signed-zeros"########,
            r########"Set(0.0) + Set(-0.0)"########,
            r########"value"########,
            r########"Set(0.0, -0.0)"########,
        ),
        (
            r########"set-nan"########,
            r########"Set(0.0 / 0.0) + Set(0.0 / 0.0)"########,
            r########"value"########,
            r########"Set(NaN)"########,
        ),
        (
            r########"set-string-hash-collision"########,
            r########"Set("Aa") + Set("BB")"########,
            r########"value"########,
            r########"Set("Aa", "BB")"########,
        ),
        (
            r########"set-string-hash-collision-duplicate"########,
            r########"Set("Aa", "BB") + Set("Aa")"########,
            r########"value"########,
            r########"Set("Aa", "BB")"########,
        ),
        (
            r########"set-nested-list"########,
            r########"Set(List(1)) + Set(List(1), List(2))"########,
            r########"value"########,
            r########"Set(List(1), List(2))"########,
        ),
        (
            r########"set-nested-object"########,
            r########"Set(new Dynamic { value = 1 }) + Set(new Dynamic { value = 1 })"########,
            r########"value"########,
            r########"Set(new Dynamic {
  value = 1
})"########,
        ),
        (
            r########"map-int-float"########,
            r########"Map(1, "int") + Map(1.0, "float")"########,
            r########"value"########,
            r########"Map(1, "int", 1.0, "float")"########,
        ),
        (
            r########"map-int-float-reversed"########,
            r########"Map(1.0, "float") + Map(1, "int")"########,
            r########"value"########,
            r########"Map(1.0, "float", 1, "int")"########,
        ),
        (
            r########"map-float-signed-zeros"########,
            r########"Map(0.0, "pos") + Map(-0.0, "neg")"########,
            r########"value"########,
            r########"Map(0.0, "pos", -0.0, "neg")"########,
        ),
        (
            r########"map-nan-overwrite"########,
            r########"Map(0.0 / 0.0, 1) + Map(0.0 / 0.0, 2)"########,
            r########"value"########,
            r########"Map(NaN, 2)"########,
        ),
        (
            r########"map-string-hash-collision"########,
            r########"Map("Aa", 1) + Map("BB", 2)"########,
            r########"value"########,
            r########"Map("Aa", 1, "BB", 2)"########,
        ),
        (
            r########"map-string-hash-collision-duplicate"########,
            r########"Map("Aa", 1, "BB", 2) + Map("Aa", 3)"########,
            r########"value"########,
            r########"Map("Aa", 3, "BB", 2)"########,
        ),
        (
            r########"map-nested-list-key"########,
            r########"Map(List(1), "old") + Map(List(1), "new")"########,
            r########"value"########,
            r########"Map(List(1), "new")"########,
        ),
        (
            r########"map-nested-object-key"########,
            r########"Map(new Dynamic { value = 1 }, "old") + Map(new Dynamic { value = 1 }, "new")"########,
            r########"value"########,
            r########"Map(new Dynamic { value = 1 }, "new")"########,
        ),
        (
            r########"list-lazy-length"########,
            r########"(List(new Dynamic { value = throw("lazy-left") }) + List(new Dynamic { value = throw("lazy-right") })).length"########,
            r########"value"########,
            r########"2"########,
        ),
        (
            r########"list-lazy-first-read"########,
            r########"(List(new Dynamic { value = throw("lazy-left") }) + List(new Dynamic { value = throw("lazy-right") }))[0].value"########,
            r########"error"########,
            r########"–– Pkl Error ––
lazy-left

1 | (List(new Dynamic { value = throw("lazy-left") }) + List(new Dynamic { value = throw("lazy-right") }))[0].value
                                ^^^^^^^^^^^^^^^^^^
at repl:text.value (repl:text)

1 | (List(new Dynamic { value = throw("lazy-left") }) + List(new Dynamic { value = throw("lazy-right") }))[0].value
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"list-lazy-second-read"########,
            r########"(List(new Dynamic { value = throw("lazy-left") }) + List(new Dynamic { value = throw("lazy-right") }))[1].value"########,
            r########"error"########,
            r########"–– Pkl Error ––
lazy-right

1 | (List(new Dynamic { value = throw("lazy-left") }) + List(new Dynamic { value = throw("lazy-right") }))[1].value
                                                                                   ^^^^^^^^^^^^^^^^^^^
at repl:text.value (repl:text)

1 | (List(new Dynamic { value = throw("lazy-left") }) + List(new Dynamic { value = throw("lazy-right") }))[1].value
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"map-lazy-length"########,
            r########"(Map("a", new Dynamic { value = throw("lazy-left") }) + Map("b", new Dynamic { value = throw("lazy-right") })).length"########,
            r########"value"########,
            r########"2"########,
        ),
        (
            r########"map-lazy-first-read"########,
            r########"(Map("a", new Dynamic { value = throw("lazy-left") }) + Map("b", new Dynamic { value = throw("lazy-right") }))["a"].value"########,
            r########"error"########,
            r########"–– Pkl Error ––
lazy-left

1 | (Map("a", new Dynamic { value = throw("lazy-left") }) + Map("b", new Dynamic { value = throw("lazy-right") }))["a"].value
                                    ^^^^^^^^^^^^^^^^^^
at repl:text.value (repl:text)

1 | (Map("a", new Dynamic { value = throw("lazy-left") }) + Map("b", new Dynamic { value = throw("lazy-right") }))["a"].value
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"map-lazy-second-read"########,
            r########"(Map("a", new Dynamic { value = throw("lazy-left") }) + Map("b", new Dynamic { value = throw("lazy-right") }))["b"].value"########,
            r########"error"########,
            r########"–– Pkl Error ––
lazy-right

1 | (Map("a", new Dynamic { value = throw("lazy-left") }) + Map("b", new Dynamic { value = throw("lazy-right") }))["b"].value
                                                                                           ^^^^^^^^^^^^^^^^^^^
at repl:text.value (repl:text)

1 | (Map("a", new Dynamic { value = throw("lazy-left") }) + Map("b", new Dynamic { value = throw("lazy-right") }))["b"].value
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"map-lazy-overwrite-length"########,
            r########"(Map("a", new Dynamic { value = throw("discarded") }) + Map("a", new Dynamic { value = 2 })).length"########,
            r########"value"########,
            r########"1"########,
        ),
        (
            r########"map-lazy-overwrite-read"########,
            r########"(Map("a", new Dynamic { value = throw("discarded") }) + Map("a", new Dynamic { value = 2 }))["a"].value"########,
            r########"value"########,
            r########"2"########,
        ),
        (
            r########"set-hash-left-first"########,
            r########"Set(new Dynamic { value = throw("hash-left") }) + Set(new Dynamic { value = throw("hash-right") })"########,
            r########"error"########,
            r########"–– Pkl Error ––
hash-left

1 | Set(new Dynamic { value = throw("hash-left") }) + Set(new Dynamic { value = throw("hash-right") })
                              ^^^^^^^^^^^^^^^^^^
at repl:text.value (repl:text)
"########,
        ),
        (
            r########"set-hash-right-only"########,
            r########"Set(1) + Set(new Dynamic { value = throw("hash-right") })"########,
            r########"error"########,
            r########"–– Pkl Error ––
hash-right

1 | Set(1) + Set(new Dynamic { value = throw("hash-right") })
                                       ^^^^^^^^^^^^^^^^^^^
at repl:text.value (repl:text)
"########,
        ),
        (
            r########"map-key-left-first"########,
            r########"Map(new Dynamic { value = throw("key-left") }, 1) + Map(new Dynamic { value = throw("key-right") }, 2)"########,
            r########"error"########,
            r########"–– Pkl Error ––
key-left

1 | Map(new Dynamic { value = throw("key-left") }, 1) + Map(new Dynamic { value = throw("key-right") }, 2)
                              ^^^^^^^^^^^^^^^^^
at repl:text.value (repl:text)
"########,
        ),
        (
            r########"map-key-right-only"########,
            r########"Map(1, 1) + Map(new Dynamic { value = throw("key-right") }, 2)"########,
            r########"error"########,
            r########"–– Pkl Error ––
key-right

1 | Map(1, 1) + Map(new Dynamic { value = throw("key-right") }, 2)
                                          ^^^^^^^^^^^^^^^^^^
at repl:text.value (repl:text)
"########,
        ),
        (
            r########"strict-left-before-right"########,
            r########"throw("left") + throw("right")"########,
            r########"error"########,
            r########"–– Pkl Error ––
left

1 | throw("left") + throw("right")
    ^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"strict-right-after-left"########,
            r########"List(1) + throw("right")"########,
            r########"error"########,
            r########"–– Pkl Error ––
right

1 | List(1) + throw("right")
              ^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"bad-list-map"########,
            r########"List(1) + Map(1, 2)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Operator `+` is not defined for operand types `List` and `Map`.
Left operand : List(1)
Right operand: Map(1, 2)

1 | List(1) + Map(1, 2)
    ^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"bad-map-list"########,
            r########"Map(1, 2) + List(1)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Operator `+` is not defined for operand types `Map` and `List`.
Left operand : Map(1, 2)
Right operand: List(1)

1 | Map(1, 2) + List(1)
    ^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"bad-set-map"########,
            r########"Set(1) + Map(1, 2)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Operator `+` is not defined for operand types `Set` and `Map`.
Left operand : Set(1)
Right operand: Map(1, 2)

1 | Set(1) + Map(1, 2)
    ^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"bad-map-set"########,
            r########"Map(1, 2) + Set(1)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Operator `+` is not defined for operand types `Map` and `Set`.
Left operand : Map(1, 2)
Right operand: Set(1)

1 | Map(1, 2) + Set(1)
    ^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"bad-list-int"########,
            r########"List(1) + 1"########,
            r########"error"########,
            r########"–– Pkl Error ––
Operator `+` is not defined for operand types `List` and `Int`.
Left operand : List(1)
Right operand: 1

1 | List(1) + 1
    ^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"bad-int-list"########,
            r########"1 + List(1)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Operator `+` is not defined for operand types `Int` and `List`.
Left operand : 1
Right operand: List(1)

1 | 1 + List(1)
    ^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"bad-set-null"########,
            r########"Set(1) + null"########,
            r########"error"########,
            r########"–– Pkl Error ––
Operator `+` is not defined for operand types `Set` and `Null`.
Left operand : Set(1)
Right operand: null

1 | Set(1) + null
    ^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"bad-null-map"########,
            r########"null + Map(1, 2)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Operator `+` is not defined for operand types `Null` and `Map`.
Left operand : null
Right operand: Map(1, 2)

1 | null + Map(1, 2)
    ^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"bad-dynamic-dynamic"########,
            r########"new Dynamic {} + new Dynamic {}"########,
            r########"error"########,
            r########"–– Pkl Error ––
Operator `+` is not defined for operand types `Dynamic` and `Dynamic`.
Left operand : new Dynamic {}
Right operand: new Dynamic {}

1 | new Dynamic {} + new Dynamic {}
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"bad-listing-listing"########,
            r########"new Listing { 1 } + new Listing { 2 }"########,
            r########"error"########,
            r########"–– Pkl Error ––
Operator `+` is not defined for operand types `Listing` and `Listing`.
Left operand : new Listing { ? }
Right operand: new Listing { ? }

1 | new Listing { 1 } + new Listing { 2 }
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"bad-mapping-mapping"########,
            r########"new Mapping { ["a"] = 1 } + new Mapping { ["b"] = 2 }"########,
            r########"error"########,
            r########"–– Pkl Error ––
Operator `+` is not defined for operand types `Mapping` and `Mapping`.
Left operand : new Mapping { ["a"] = ? }
Right operand: new Mapping { ["b"] = ? }

1 | new Mapping { ["a"] = 1 } + new Mapping { ["b"] = 2 }
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"bad-list-subtract"########,
            r########"List(1) - List(2)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Operator `-` is not defined for operand types `List` and `List`.
Left operand : List(1)
Right operand: List(2)

1 | List(1) - List(2)
    ^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"bad-map-subtract"########,
            r########"Map(1, 2) - Map(3, 4)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Operator `-` is not defined for operand types `Map` and `Map`.
Left operand : Map(1, 2)
Right operand: Map(3, 4)

1 | Map(1, 2) - Map(3, 4)
    ^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"bad-set-subtract"########,
            r########"Set(1) - Set(2)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Operator `-` is not defined for operand types `Set` and `Set`.
Left operand : Set(1)
Right operand: Set(2)

1 | Set(1) - Set(2)
    ^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"map-duration-representative-left"########,
            r########"Map(1.min, "old") + Map(60.s, "new")"########,
            r########"value"########,
            r########"Map(1.min, "new")"########,
        ),
        (
            r########"map-duration-representative-right"########,
            r########"Map(60.s, "old") + Map(1.min, "new")"########,
            r########"value"########,
            r########"Map(60.s, "new")"########,
        ),
        (
            r########"map-duration-left-unit"########,
            r########"(Map(1.min, "old") + Map(60.s, "new")).keys.toList()[0].unit"########,
            r########"value"########,
            r########"min"########,
        ),
        (
            r########"map-duration-right-unit"########,
            r########"(Map(60.s, "old") + Map(1.min, "new")).keys.toList()[0].unit"########,
            r########"value"########,
            r########"s"########,
        ),
        (
            r########"map-duration-overwrite-order"########,
            r########"Map(1.min, "old", 2.s, "second") + Map(60.s, "new", 3.s, "third")"########,
            r########"value"########,
            r########"Map(1.min, "new", 2.s, "second", 3.s, "third")"########,
        ),
        (
            r########"set-duration-representative-left"########,
            r########"Set(1.min) + Set(60.s)"########,
            r########"value"########,
            r########"Set(1.min)"########,
        ),
        (
            r########"set-duration-representative-right"########,
            r########"Set(60.s) + Set(1.min)"########,
            r########"value"########,
            r########"Set(60.s)"########,
        ),
        (
            r########"set-duration-left-unit"########,
            r########"(Set(1.min) + Set(60.s)).first.unit"########,
            r########"value"########,
            r########"min"########,
        ),
        (
            r########"set-duration-right-unit"########,
            r########"(Set(60.s) + Set(1.min)).first.unit"########,
            r########"value"########,
            r########"s"########,
        ),
        (
            r########"map-same-object-key"########,
            r########"let (key = new Dynamic { value = 1 }) Map(key, "old") + Map(key, "new")"########,
            r########"value"########,
            r########"Map(new Dynamic { value = 1 }, "new")"########,
        ),
        (
            r########"set-same-object-element"########,
            r########"let (key = new Dynamic { value = 1 }) Set(key) + Set(key)"########,
            r########"value"########,
            r########"Set(new Dynamic {
  value = 1
})"########,
        ),
        (
            r########"map-warmed-key-cache"########,
            r########"let (key = new Dynamic { value = 1 }) (Map(key, "old") + Map(key, "new"))[key]"########,
            r########"value"########,
            r########"new"########,
        ),
        (
            r########"bad-list-function"########,
            r########"List(1) + ((x) -> x)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Operator `+` is not defined for operand types `List` and `Function1`.
Left operand : List(1)
Right operand: new Function1 {}

1 | List(1) + ((x) -> x)
    ^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"bad-function-list"########,
            r########"((x) -> x) + List(1)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Operator `+` is not defined for operand types `Function1` and `List`.
Left operand : new Function1 {}
Right operand: List(1)

1 | ((x) -> x) + List(1)
    ^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"bad-map-class"########,
            r########"Map(1, 2) + List"########,
            r########"error"########,
            r########"–– Pkl Error ––
Operator `+` is not defined for operand types `Map` and `Class`.
Left operand : Map(1, 2)
Right operand: List

1 | Map(1, 2) + List
    ^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"bad-class-map"########,
            r########"List + Map(1, 2)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Operator `+` is not defined for operand types `Class` and `Map`.
Left operand : List
Right operand: Map(1, 2)

1 | List + Map(1, 2)
    ^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"bad-set-function"########,
            r########"Set(1) + ((x) -> x)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Operator `+` is not defined for operand types `Set` and `Function1`.
Left operand : Set(1)
Right operand: new Function1 {}

1 | Set(1) + ((x) -> x)
    ^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"bad-map-int"########,
            r########"Map(1, 2) + 1"########,
            r########"error"########,
            r########"–– Pkl Error ––
Operator `+` is not defined for operand types `Map` and `Int`.
Left operand : Map(1, 2)
Right operand: 1

1 | Map(1, 2) + 1
    ^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"bad-set-int"########,
            r########"Set(1) + 1"########,
            r########"error"########,
            r########"–– Pkl Error ––
Operator `+` is not defined for operand types `Set` and `Int`.
Left operand : Set(1)
Right operand: 1

1 | Set(1) + 1
    ^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"map-slot-sixteen"########,
            r########"Map(0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13, 14, 14) + Map(15, 15)"########,
            r########"value"########,
            r########"Map(0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13, 14, 14, 15, 15)"########,
        ),
        (
            r########"map-slot-seventeen"########,
            r########"Map(0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13, 14, 14, 15, 15) + Map(16, 16)"########,
            r########"value"########,
            r########"Map(0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13, 14, 14, 15, 15, 16, 16)"########,
        ),
        (
            r########"map-slot-seventeen-overwrite"########,
            r########"Map(0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13, 14, 14, 15, 15) + Map(0, 99, 16, 16)"########,
            r########"value"########,
            r########"Map(0, 99, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13, 14, 14, 15, 15, 16, 16)"########,
        ),
        (
            r########"set-slot-sixteen"########,
            r########"Set(0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14) + Set(15)"########,
            r########"value"########,
            r########"Set(0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15)"########,
        ),
        (
            r########"set-slot-seventeen"########,
            r########"Set(0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15) + Set(16)"########,
            r########"value"########,
            r########"Set(0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16)"########,
        ),
        (
            r########"set-slot-seventeen-duplicate"########,
            r########"Set(0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15) + List(0, 16, 0)"########,
            r########"value"########,
            r########"Set(0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16)"########,
        ),
        (
            r########"invalid-list-lazy-left"########,
            r########"List(new Dynamic { value = throw("latent-left") }) + 1"########,
            r########"error"########,
            r########"–– Pkl Error ––
Operator `+` is not defined for operand types `List` and `Int`.
Left operand : List(new Dynamic { value = ? })
Right operand: 1

1 | List(new Dynamic { value = throw("latent-left") }) + 1
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"invalid-list-lazy-right"########,
            r########"1 + List(new Dynamic { value = throw("latent-right") })"########,
            r########"error"########,
            r########"–– Pkl Error ––
Operator `+` is not defined for operand types `Int` and `List`.
Left operand : 1
Right operand: List(new Dynamic { value = ? })

1 | 1 + List(new Dynamic { value = throw("latent-right") })
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"invalid-map-lazy-left"########,
            r########"Map("a", new Dynamic { value = throw("latent-left") }) + 1"########,
            r########"error"########,
            r########"–– Pkl Error ––
Operator `+` is not defined for operand types `Map` and `Int`.
Left operand : Map("a", new Dynamic { value = ? })
Right operand: 1

1 | Map("a", new Dynamic { value = throw("latent-left") }) + 1
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"invalid-map-lazy-right"########,
            r########"1 + Map("a", new Dynamic { value = throw("latent-right") })"########,
            r########"error"########,
            r########"–– Pkl Error ––
Operator `+` is not defined for operand types `Int` and `Map`.
Left operand : 1
Right operand: Map("a", new Dynamic { value = ? })

1 | 1 + Map("a", new Dynamic { value = throw("latent-right") })
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"invalid-set-hash-left"########,
            r########"Set(new Dynamic { value = throw("hash-left") }) + 1"########,
            r########"error"########,
            r########"–– Pkl Error ––
hash-left

1 | Set(new Dynamic { value = throw("hash-left") }) + 1
                              ^^^^^^^^^^^^^^^^^^
at repl:text.value (repl:text)
"########,
        ),
        (
            r########"invalid-list-dynamic-right"########,
            r########"List(1) + new Dynamic { value = throw("latent-right") }"########,
            r########"error"########,
            r########"–– Pkl Error ––
Operator `+` is not defined for operand types `List` and `Dynamic`.
Left operand : List(1)
Right operand: new Dynamic { value = ? }

1 | List(1) + new Dynamic { value = throw("latent-right") }
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"invalid-map-dynamic-right"########,
            r########"Map("a", 1) + new Dynamic { value = throw("latent-right") }"########,
            r########"error"########,
            r########"–– Pkl Error ––
Operator `+` is not defined for operand types `Map` and `Dynamic`.
Left operand : Map("a", 1)
Right operand: new Dynamic { value = ? }

1 | Map("a", 1) + new Dynamic { value = throw("latent-right") }
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"invalid-set-dynamic-right"########,
            r########"Set(1) + new Dynamic { value = throw("latent-right") }"########,
            r########"error"########,
            r########"–– Pkl Error ––
Operator `+` is not defined for operand types `Set` and `Dynamic`.
Left operand : Set(1)
Right operand: new Dynamic { value = ? }

1 | Set(1) + new Dynamic { value = throw("latent-right") }
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"prebuilt-set-equal"########,
            r########"setLeft + setRight"########,
            r########"value"########,
            r########"Set(new Dynamic {
  value = 1
})"########,
        ),
        (
            r########"prebuilt-set-length"########,
            r########"(setLeft + setRight).length"########,
            r########"value"########,
            r########"1"########,
        ),
        (
            r########"prebuilt-set-warm"########,
            r########"let (warm = setLeft.length) setLeft + setRight"########,
            r########"value"########,
            r########"Set(new Dynamic {
  value = 1
})"########,
        ),
        (
            r########"prebuilt-map-equal"########,
            r########"mapLeft + mapRight"########,
            r########"value"########,
            r########"Map(new Dynamic { value = 1 }, "new")"########,
        ),
        (
            r########"prebuilt-map-lookup"########,
            r########"(mapLeft + mapRight)[keyLeft]"########,
            r########"value"########,
            r########"new"########,
        ),
        (
            r########"prebuilt-map-warm"########,
            r########"let (warm = mapLeft.length) (mapLeft + mapRight)[keyRight]"########,
            r########"value"########,
            r########"new"########,
        ),
    ];
    assert_eq!(rows.len(), 125);
    for (id, expression, kind, expected) in rows {
        probe
            .evaluate_module_source(
                "file:///stage63/collection-addition.pkl",
                Some(include_str!(
                    "../../../tests/fixtures/m2-collection-addition/module.pkl"
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
