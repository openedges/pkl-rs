//! Complete independent Java 0.32.0 string observations, capture-01/repeat-0.
//! Authority: m2-amendment-construction/oracle-evidence.tar.gz (188 ordered rows).
#[path = "support/module_keys.rs"]
mod module_keys;
use pkl_runtime::probe::SourceProbe;
#[test]
fn complete_java_amendment_construction_matrix() {
    let mut probe = SourceProbe::new_module_evaluator("compact", false, false, false).unwrap();
    let mut failures = Vec::new();
    let rows = [
        (
            r########"module-first"########,
            r########"moduleRepeated.first"########,
            r########"value"########,
            r########"1"########,
        ),
        (
            r########"module-second"########,
            r########"moduleRepeated.second"########,
            r########"value"########,
            r########"2"########,
        ),
        (
            r########"module-render"########,
            r########"moduleRepeated"########,
            r########"value"########,
            r########"new Dynamic { first = 1; second = 2 }"########,
        ),
        (
            r########"module-chain-first"########,
            r########"moduleAmended.first"########,
            r########"value"########,
            r########"1"########,
        ),
        (
            r########"module-chain-second"########,
            r########"moduleAmended.second"########,
            r########"value"########,
            r########"2"########,
        ),
        (
            r########"module-chain-third"########,
            r########"moduleAmended.third"########,
            r########"value"########,
            r########"3"########,
        ),
        (
            r########"module-chain-render"########,
            r########"moduleAmended"########,
            r########"value"########,
            r########"new Dynamic { first = 1; second = 2; third = 3 }"########,
        ),
        (
            r########"object-chain-a"########,
            r########"(new Dynamic { a = 1 } { b = 2 } { c = 3 }).a"########,
            r########"value"########,
            r########"1"########,
        ),
        (
            r########"object-chain-b"########,
            r########"(new Dynamic { a = 1 } { b = 2 } { c = 3 }).b"########,
            r########"value"########,
            r########"2"########,
        ),
        (
            r########"object-chain-c"########,
            r########"(new Dynamic { a = 1 } { b = 2 } { c = 3 }).c"########,
            r########"value"########,
            r########"3"########,
        ),
        (
            r########"object-override-first"########,
            r########"(new Dynamic { value = 1 } { value = 2 } { value = 3 }).value"########,
            r########"value"########,
            r########"3"########,
        ),
        (
            r########"object-nested-chain"########,
            r########"(new Dynamic { inner = new Dynamic { a = 1 } } { inner { b = 2 } { c = 3 } }).inner"########,
            r########"value"########,
            r########"new Dynamic { a = 1; b = 2; c = 3 }"########,
        ),
        (
            r########"typed-chain"########,
            r########"(new Box {} { value = 3 } { extra = 4 }).value"########,
            r########"value"########,
            r########"3"########,
        ),
        (
            r########"typed-nested-inferred"########,
            r########"(new Owner {} { child { value = 3 } { extra = 4 } }).child"########,
            r########"value"########,
            r########"new Box { value = 3; nested { left = 10 }; extra = 4 }"########,
        ),
        (
            r########"class-declared-child"########,
            r########"(new OwnerAmended {}).child"########,
            r########"value"########,
            r########"new Box { value = 4; nested { left = 10 }; extra = 5 }"########,
        ),
        (
            r########"class-declared-value"########,
            r########"(new OwnerAmended {}).child.value"########,
            r########"value"########,
            r########"4"########,
        ),
        (
            r########"class-declared-extra"########,
            r########"(new OwnerAmended {}).child.extra"########,
            r########"value"########,
            r########"5"########,
        ),
        (
            r########"mapping-entry-chain"########,
            r########"((mapBase) { ["k"] { second = 2 } { third = 3 } })["k"]"########,
            r########"value"########,
            r########"new Dynamic { first = 1; second = 2; third = 3 }"########,
        ),
        (
            r########"listing-element-chain"########,
            r########"(new Listing { new Dynamic { a = 1 } } { [0] { b = 2 } { c = 3 } })[0]"########,
            r########"value"########,
            r########"new Dynamic { a = 1; b = 2; c = 3 }"########,
        ),
        (
            r########"predicate-chain"########,
            r########"(new Listing { new Dynamic { tag = "x" } } { [[ tag == "x" ]] { good = true } { also = true } })[0]"########,
            r########"value"########,
            r########"new Dynamic { tag = "x"; good = true; also = true }"########,
        ),
        (
            r########"module-decl-syntax-repeated"########,
            r########"new Dynamic { x = 1 } { y = 2 }"########,
            r########"value"########,
            r########"new Dynamic { x = 1; y = 2 }"########,
        ),
        (
            r########"module-decl-empty-middle"########,
            r########"new Dynamic { x = 1 } {} { y = 2 }"########,
            r########"value"########,
            r########"new Dynamic { x = 1; y = 2 }"########,
        ),
        (
            r########"dynamic-empty"########,
            r########"(Dynamic) {}"########,
            r########"value"########,
            r########"new Dynamic {}"########,
        ),
        (
            r########"dynamic-property"########,
            r########"(Dynamic) { value = 7 }"########,
            r########"value"########,
            r########"new Dynamic { value = 7 }"########,
        ),
        (
            r########"dynamic-element"########,
            r########"(Dynamic) { 7 }"########,
            r########"value"########,
            r########"new Dynamic { 7 }"########,
        ),
        (
            r########"dynamic-entry"########,
            r########"(Dynamic) { ["k"] = 7 }"########,
            r########"value"########,
            r########"new Dynamic { ["k"] = 7 }"########,
        ),
        (
            r########"dynamic-generator-empty"########,
            r########"(Dynamic) { for (x in List()) { x } }"########,
            r########"value"########,
            r########"new Dynamic {}"########,
        ),
        (
            r########"dynamic-generator-property"########,
            r########"(Dynamic) { for (x in List(1)) { value = x } }"########,
            r########"error"########,
            r########"–– Pkl Error ––
A for-generator cannot generate object properties (only entries and elements).

1 | (Dynamic) { for (x in List(1)) { value = x } }
                ^^^
at repl:text (repl:text)
"########,
        ),
        (
            r########"listing-empty"########,
            r########"(Listing) {}"########,
            r########"value"########,
            r########"new Listing {}"########,
        ),
        (
            r########"listing-property"########,
            r########"(Listing) { value = 7 }"########,
            r########"error"########,
            r########"–– Pkl Error ––
Object of type `Listing` cannot have a property (other than `default`).

1 | (Listing) { value = 7 }
                ^^^^^
at  (repl:text)

Objects of type `Listing` and `Mapping` can only have a `default` property (and `local` properties). If you did not mean to set a property, you probably got the syntax wrong. For example, `obj { name = "Pigeon" }` sets a property, whereas `obj { ["name"] = "Pigeon" }` adds an entry.
"########,
        ),
        (
            r########"listing-element"########,
            r########"(Listing) { 7 }"########,
            r########"value"########,
            r########"new Listing { 7 }"########,
        ),
        (
            r########"listing-entry"########,
            r########"(Listing) { ["k"] = 7 }"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected key of type `Int`, but got type `String`.

1 | (Listing) { ["k"] = 7 }
                 ^^^
at  (repl:text)
"########,
        ),
        (
            r########"listing-generator-empty"########,
            r########"(Listing) { for (x in List()) { x } }"########,
            r########"value"########,
            r########"new Listing {}"########,
        ),
        (
            r########"listing-generator-property"########,
            r########"(Listing) { for (x in List(1)) { value = x } }"########,
            r########"error"########,
            r########"–– Pkl Error ––
A for-generator cannot generate object properties (only entries and elements).

1 | (Listing) { for (x in List(1)) { value = x } }
                ^^^
at repl:text (repl:text)
"########,
        ),
        (
            r########"mapping-empty"########,
            r########"(Mapping) {}"########,
            r########"value"########,
            r########"new Mapping {}"########,
        ),
        (
            r########"mapping-property"########,
            r########"(Mapping) { value = 7 }"########,
            r########"error"########,
            r########"–– Pkl Error ––
Object of type `Mapping` cannot have a property (other than `default`).

1 | (Mapping) { value = 7 }
                ^^^^^
at  (repl:text)

Objects of type `Listing` and `Mapping` can only have a `default` property (and `local` properties). If you did not mean to set a property, you probably got the syntax wrong. For example, `obj { name = "Pigeon" }` sets a property, whereas `obj { ["name"] = "Pigeon" }` adds an entry.
"########,
        ),
        (
            r########"mapping-element"########,
            r########"(Mapping) { 7 }"########,
            r########"error"########,
            r########"–– Pkl Error ––
Object of type `Mapping` cannot have an element.

1 | (Mapping) { 7 }
                ^
at  (repl:text)

Only objects of type `Listing` and `Dynamic` can have elements. If you did not mean to add an element, you probably got the syntax wrong. For example, `obj { name }` adds an element, whereas `obj { name = "Pigeon" }` sets a property.
"########,
        ),
        (
            r########"mapping-entry"########,
            r########"(Mapping) { ["k"] = 7 }"########,
            r########"value"########,
            r########"new Mapping { ["k"] = 7 }"########,
        ),
        (
            r########"mapping-generator-empty"########,
            r########"(Mapping) { for (x in List()) { [x] = x } }"########,
            r########"value"########,
            r########"new Mapping {}"########,
        ),
        (
            r########"mapping-generator-property"########,
            r########"(Mapping) { for (x in List(1)) { value = x } }"########,
            r########"error"########,
            r########"–– Pkl Error ––
A for-generator cannot generate object properties (only entries and elements).

1 | (Mapping) { for (x in List(1)) { value = x } }
                ^^^
at repl:text (repl:text)
"########,
        ),
        (
            r########"user-empty"########,
            r########"(Box) {}"########,
            r########"value"########,
            r########"new Box { value = 1; nested { left = 10 }; extra = 2 }"########,
        ),
        (
            r########"user-property"########,
            r########"(Box) { value = 7 }"########,
            r########"value"########,
            r########"new Box { value = 7; nested { left = 10 }; extra = 2 }"########,
        ),
        (
            r########"user-element"########,
            r########"(Box) { 7 }"########,
            r########"error"########,
            r########"–– Pkl Error ––
Object of type `amendment-construction#Box` cannot have an element.

1 | (Box) { 7 }
            ^
at  (repl:text)

Only objects of type `Listing` and `Dynamic` can have elements. If you did not mean to add an element, you probably got the syntax wrong. For example, `obj { name }` adds an element, whereas `obj { name = "Pigeon" }` sets a property.
"########,
        ),
        (
            r########"user-entry"########,
            r########"(Box) { ["k"] = 7 }"########,
            r########"error"########,
            r########"–– Pkl Error ––
Object of type `amendment-construction#Box` cannot have an entry.

1 | (Box) { ["k"] = 7 }
             ^^^
at  (repl:text)

Only objects of type `Mapping` and `Dynamic` can have entries. If you did not mean to add an entry, you probably got the syntax wrong. For example, `obj { ["name"] = "Pigeon" }` adds an entry, whereas `obj { name = "Pigeon" }` sets a property.
"########,
        ),
        (
            r########"user-generator-empty"########,
            r########"(Box) { when (false) { value = 7 } }"########,
            r########"value"########,
            r########"new Box { value = 1; nested { left = 10 }; extra = 2 }"########,
        ),
        (
            r########"user-generator-property"########,
            r########"(Box) { for (x in List(1)) { value = x } }"########,
            r########"error"########,
            r########"–– Pkl Error ––
A for-generator cannot generate object properties (only entries and elements).

1 | (Box) { for (x in List(1)) { value = x } }
            ^^^
at repl:text (repl:text)
"########,
        ),
        (
            r########"abstract-empty"########,
            r########"(AbstractBox) {}"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot instantiate abstract class `amendment-construction#AbstractBox`.

1 | (AbstractBox) {}
     ^^^^^^^^^^^
at  (repl:text)

Abstract classes cannot be instantiated.
Instead, instantiate a concrete subclass.
"########,
        ),
        (
            r########"abstract-property"########,
            r########"(AbstractBox) { value = 7 }"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot instantiate abstract class `amendment-construction#AbstractBox`.

1 | (AbstractBox) { value = 7 }
     ^^^^^^^^^^^
at  (repl:text)

Abstract classes cannot be instantiated.
Instead, instantiate a concrete subclass.
"########,
        ),
        (
            r########"abstract-element"########,
            r########"(AbstractBox) { 7 }"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot instantiate abstract class `amendment-construction#AbstractBox`.

1 | (AbstractBox) { 7 }
     ^^^^^^^^^^^
at  (repl:text)

Abstract classes cannot be instantiated.
Instead, instantiate a concrete subclass.
"########,
        ),
        (
            r########"abstract-entry"########,
            r########"(AbstractBox) { ["k"] = 7 }"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot instantiate abstract class `amendment-construction#AbstractBox`.

1 | (AbstractBox) { ["k"] = 7 }
     ^^^^^^^^^^^
at  (repl:text)

Abstract classes cannot be instantiated.
Instead, instantiate a concrete subclass.
"########,
        ),
        (
            r########"abstract-generator-empty"########,
            r########"(AbstractBox) { when (false) { value = 7 } }"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot instantiate abstract class `amendment-construction#AbstractBox`.

1 | (AbstractBox) { when (false) { value = 7 } }
     ^^^^^^^^^^^
at  (repl:text)

Abstract classes cannot be instantiated.
Instead, instantiate a concrete subclass.
"########,
        ),
        (
            r########"abstract-generator-property"########,
            r########"(AbstractBox) { for (x in List(1)) { value = x } }"########,
            r########"error"########,
            r########"–– Pkl Error ––
A for-generator cannot generate object properties (only entries and elements).

1 | (AbstractBox) { for (x in List(1)) { value = x } }
                    ^^^
at repl:text (repl:text)
"########,
        ),
        (
            r########"external-empty"########,
            r########"(Int) {}"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot instantiate, or amend an instance of, external class `Int`.

1 | (Int) {}
     ^^^
at  (repl:text)

To create an instance of an external class, use its literal syntax or constructor method.
Examples:
* `123` uses literal syntax to create an instance of class `Int`.
* `Pair(1, 2)` uses the `Pair()` constructor method to create an instance of class `Pair`.
"########,
        ),
        (
            r########"external-property"########,
            r########"(Int) { value = 7 }"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot instantiate, or amend an instance of, external class `Int`.

1 | (Int) { value = 7 }
     ^^^
at  (repl:text)

To create an instance of an external class, use its literal syntax or constructor method.
Examples:
* `123` uses literal syntax to create an instance of class `Int`.
* `Pair(1, 2)` uses the `Pair()` constructor method to create an instance of class `Pair`.
"########,
        ),
        (
            r########"external-element"########,
            r########"(Int) { 7 }"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot instantiate, or amend an instance of, external class `Int`.

1 | (Int) { 7 }
     ^^^
at  (repl:text)

To create an instance of an external class, use its literal syntax or constructor method.
Examples:
* `123` uses literal syntax to create an instance of class `Int`.
* `Pair(1, 2)` uses the `Pair()` constructor method to create an instance of class `Pair`.
"########,
        ),
        (
            r########"external-entry"########,
            r########"(Int) { ["k"] = 7 }"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot instantiate, or amend an instance of, external class `Int`.

1 | (Int) { ["k"] = 7 }
     ^^^
at  (repl:text)

To create an instance of an external class, use its literal syntax or constructor method.
Examples:
* `123` uses literal syntax to create an instance of class `Int`.
* `Pair(1, 2)` uses the `Pair()` constructor method to create an instance of class `Pair`.
"########,
        ),
        (
            r########"external-generator-empty"########,
            r########"(Int) { when (false) { value = 7 } }"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot instantiate, or amend an instance of, external class `Int`.

1 | (Int) { when (false) { value = 7 } }
     ^^^
at  (repl:text)

To create an instance of an external class, use its literal syntax or constructor method.
Examples:
* `123` uses literal syntax to create an instance of class `Int`.
* `Pair(1, 2)` uses the `Pair()` constructor method to create an instance of class `Pair`.
"########,
        ),
        (
            r########"external-generator-property"########,
            r########"(Int) { for (x in List(1)) { value = x } }"########,
            r########"error"########,
            r########"–– Pkl Error ––
A for-generator cannot generate object properties (only entries and elements).

1 | (Int) { for (x in List(1)) { value = x } }
            ^^^
at repl:text (repl:text)
"########,
        ),
        (
            r########"alias-empty"########,
            r########"(BoxAlias) {}"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot instantiate, or amend an instance of, external class `TypeAlias`.

1 | (BoxAlias) {}
     ^^^^^^^^
at  (repl:text)

To create an instance of an external class, use its literal syntax or constructor method.
Examples:
* `123` uses literal syntax to create an instance of class `Int`.
* `Pair(1, 2)` uses the `Pair()` constructor method to create an instance of class `Pair`.
"########,
        ),
        (
            r########"alias-property"########,
            r########"(BoxAlias) { value = 7 }"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot instantiate, or amend an instance of, external class `TypeAlias`.

1 | (BoxAlias) { value = 7 }
     ^^^^^^^^
at  (repl:text)

To create an instance of an external class, use its literal syntax or constructor method.
Examples:
* `123` uses literal syntax to create an instance of class `Int`.
* `Pair(1, 2)` uses the `Pair()` constructor method to create an instance of class `Pair`.
"########,
        ),
        (
            r########"alias-element"########,
            r########"(BoxAlias) { 7 }"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot instantiate, or amend an instance of, external class `TypeAlias`.

1 | (BoxAlias) { 7 }
     ^^^^^^^^
at  (repl:text)

To create an instance of an external class, use its literal syntax or constructor method.
Examples:
* `123` uses literal syntax to create an instance of class `Int`.
* `Pair(1, 2)` uses the `Pair()` constructor method to create an instance of class `Pair`.
"########,
        ),
        (
            r########"alias-entry"########,
            r########"(BoxAlias) { ["k"] = 7 }"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot instantiate, or amend an instance of, external class `TypeAlias`.

1 | (BoxAlias) { ["k"] = 7 }
     ^^^^^^^^
at  (repl:text)

To create an instance of an external class, use its literal syntax or constructor method.
Examples:
* `123` uses literal syntax to create an instance of class `Int`.
* `Pair(1, 2)` uses the `Pair()` constructor method to create an instance of class `Pair`.
"########,
        ),
        (
            r########"alias-generator-empty"########,
            r########"(BoxAlias) { when (false) { value = 7 } }"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot instantiate, or amend an instance of, external class `TypeAlias`.

1 | (BoxAlias) { when (false) { value = 7 } }
     ^^^^^^^^
at  (repl:text)

To create an instance of an external class, use its literal syntax or constructor method.
Examples:
* `123` uses literal syntax to create an instance of class `Int`.
* `Pair(1, 2)` uses the `Pair()` constructor method to create an instance of class `Pair`.
"########,
        ),
        (
            r########"alias-generator-property"########,
            r########"(BoxAlias) { for (x in List(1)) { value = x } }"########,
            r########"error"########,
            r########"–– Pkl Error ––
A for-generator cannot generate object properties (only entries and elements).

1 | (BoxAlias) { for (x in List(1)) { value = x } }
                 ^^^
at repl:text (repl:text)
"########,
        ),
        (
            r########"listing-alias-empty"########,
            r########"(NumberListing) {}"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot instantiate, or amend an instance of, external class `TypeAlias`.

1 | (NumberListing) {}
     ^^^^^^^^^^^^^
at  (repl:text)

To create an instance of an external class, use its literal syntax or constructor method.
Examples:
* `123` uses literal syntax to create an instance of class `Int`.
* `Pair(1, 2)` uses the `Pair()` constructor method to create an instance of class `Pair`.
"########,
        ),
        (
            r########"listing-alias-property"########,
            r########"(NumberListing) { value = 7 }"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot instantiate, or amend an instance of, external class `TypeAlias`.

1 | (NumberListing) { value = 7 }
     ^^^^^^^^^^^^^
at  (repl:text)

To create an instance of an external class, use its literal syntax or constructor method.
Examples:
* `123` uses literal syntax to create an instance of class `Int`.
* `Pair(1, 2)` uses the `Pair()` constructor method to create an instance of class `Pair`.
"########,
        ),
        (
            r########"listing-alias-element"########,
            r########"(NumberListing) { 7 }"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot instantiate, or amend an instance of, external class `TypeAlias`.

1 | (NumberListing) { 7 }
     ^^^^^^^^^^^^^
at  (repl:text)

To create an instance of an external class, use its literal syntax or constructor method.
Examples:
* `123` uses literal syntax to create an instance of class `Int`.
* `Pair(1, 2)` uses the `Pair()` constructor method to create an instance of class `Pair`.
"########,
        ),
        (
            r########"listing-alias-entry"########,
            r########"(NumberListing) { ["k"] = 7 }"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot instantiate, or amend an instance of, external class `TypeAlias`.

1 | (NumberListing) { ["k"] = 7 }
     ^^^^^^^^^^^^^
at  (repl:text)

To create an instance of an external class, use its literal syntax or constructor method.
Examples:
* `123` uses literal syntax to create an instance of class `Int`.
* `Pair(1, 2)` uses the `Pair()` constructor method to create an instance of class `Pair`.
"########,
        ),
        (
            r########"listing-alias-generator-empty"########,
            r########"(NumberListing) { when (false) { value = 7 } }"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot instantiate, or amend an instance of, external class `TypeAlias`.

1 | (NumberListing) { when (false) { value = 7 } }
     ^^^^^^^^^^^^^
at  (repl:text)

To create an instance of an external class, use its literal syntax or constructor method.
Examples:
* `123` uses literal syntax to create an instance of class `Int`.
* `Pair(1, 2)` uses the `Pair()` constructor method to create an instance of class `Pair`.
"########,
        ),
        (
            r########"listing-alias-generator-property"########,
            r########"(NumberListing) { for (x in List(1)) { value = x } }"########,
            r########"error"########,
            r########"–– Pkl Error ––
A for-generator cannot generate object properties (only entries and elements).

1 | (NumberListing) { for (x in List(1)) { value = x } }
                      ^^^
at repl:text (repl:text)
"########,
        ),
        (
            r########"mapping-alias-empty"########,
            r########"(StringIntMapping) {}"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot instantiate, or amend an instance of, external class `TypeAlias`.

1 | (StringIntMapping) {}
     ^^^^^^^^^^^^^^^^
at  (repl:text)

To create an instance of an external class, use its literal syntax or constructor method.
Examples:
* `123` uses literal syntax to create an instance of class `Int`.
* `Pair(1, 2)` uses the `Pair()` constructor method to create an instance of class `Pair`.
"########,
        ),
        (
            r########"mapping-alias-property"########,
            r########"(StringIntMapping) { value = 7 }"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot instantiate, or amend an instance of, external class `TypeAlias`.

1 | (StringIntMapping) { value = 7 }
     ^^^^^^^^^^^^^^^^
at  (repl:text)

To create an instance of an external class, use its literal syntax or constructor method.
Examples:
* `123` uses literal syntax to create an instance of class `Int`.
* `Pair(1, 2)` uses the `Pair()` constructor method to create an instance of class `Pair`.
"########,
        ),
        (
            r########"mapping-alias-element"########,
            r########"(StringIntMapping) { 7 }"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot instantiate, or amend an instance of, external class `TypeAlias`.

1 | (StringIntMapping) { 7 }
     ^^^^^^^^^^^^^^^^
at  (repl:text)

To create an instance of an external class, use its literal syntax or constructor method.
Examples:
* `123` uses literal syntax to create an instance of class `Int`.
* `Pair(1, 2)` uses the `Pair()` constructor method to create an instance of class `Pair`.
"########,
        ),
        (
            r########"mapping-alias-entry"########,
            r########"(StringIntMapping) { ["k"] = 7 }"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot instantiate, or amend an instance of, external class `TypeAlias`.

1 | (StringIntMapping) { ["k"] = 7 }
     ^^^^^^^^^^^^^^^^
at  (repl:text)

To create an instance of an external class, use its literal syntax or constructor method.
Examples:
* `123` uses literal syntax to create an instance of class `Int`.
* `Pair(1, 2)` uses the `Pair()` constructor method to create an instance of class `Pair`.
"########,
        ),
        (
            r########"mapping-alias-generator-empty"########,
            r########"(StringIntMapping) { when (false) { value = 7 } }"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot instantiate, or amend an instance of, external class `TypeAlias`.

1 | (StringIntMapping) { when (false) { value = 7 } }
     ^^^^^^^^^^^^^^^^
at  (repl:text)

To create an instance of an external class, use its literal syntax or constructor method.
Examples:
* `123` uses literal syntax to create an instance of class `Int`.
* `Pair(1, 2)` uses the `Pair()` constructor method to create an instance of class `Pair`.
"########,
        ),
        (
            r########"mapping-alias-generator-property"########,
            r########"(StringIntMapping) { for (x in List(1)) { value = x } }"########,
            r########"error"########,
            r########"–– Pkl Error ––
A for-generator cannot generate object properties (only entries and elements).

1 | (StringIntMapping) { for (x in List(1)) { value = x } }
                         ^^^
at repl:text (repl:text)
"########,
        ),
        (
            r########"dynamic-empty-rhs"########,
            r########"(moduleBase) {}"########,
            r########"value"########,
            r########"new Dynamic { first = 1 }"########,
        ),
        (
            r########"dynamic-when-false"########,
            r########"(moduleBase) { when (false) { ignored = throw("no") } }"########,
            r########"value"########,
            r########"new Dynamic { first = 1 }"########,
        ),
        (
            r########"dynamic-empty-for"########,
            r########"(moduleBase) { for (x in List()) { [x] = x } }"########,
            r########"value"########,
            r########"new Dynamic { first = 1 }"########,
        ),
        (
            r########"dynamic-empty-spread"########,
            r########"(moduleBase) { ...new Dynamic {} }"########,
            r########"value"########,
            r########"new Dynamic { first = 1 }"########,
        ),
        (
            r########"dynamic-empty-for-read"########,
            r########"((moduleBase) { for (x in List()) { [x] = x } }).first"########,
            r########"value"########,
            r########"1"########,
        ),
        (
            r########"listing-empty-rhs"########,
            r########"(listBase) {}"########,
            r########"value"########,
            r########"new Listing { 1; 2 }"########,
        ),
        (
            r########"listing-empty-for"########,
            r########"(listBase) { for (x in List()) { x } }"########,
            r########"value"########,
            r########"new Listing { 1; 2 }"########,
        ),
        (
            r########"listing-empty-for-length"########,
            r########"((listBase) { for (x in List()) { x } }).length"########,
            r########"value"########,
            r########"2"########,
        ),
        (
            r########"listing-empty-when"########,
            r########"((listBase) { when(false) { 9 } }).length"########,
            r########"value"########,
            r########"2"########,
        ),
        (
            r########"mapping-empty-rhs"########,
            r########"(mapBase) {}"########,
            r########"value"########,
            r########"new Mapping { ["k"] { first = 1 } }"########,
        ),
        (
            r########"mapping-empty-for"########,
            r########"(mapBase) { for (x in List()) { [x] = x } }"########,
            r########"value"########,
            r########"new Mapping { ["k"] { first = 1 } }"########,
        ),
        (
            r########"typed-empty-rhs"########,
            r########"(boxBase) {}"########,
            r########"value"########,
            r########"new Box { value = 1; nested { left = 10 }; extra = 2 }"########,
        ),
        (
            r########"typed-empty-for"########,
            r########"(boxBase) { when (false) { value = 7 } }"########,
            r########"value"########,
            r########"new Box { value = 1; nested { left = 10 }; extra = 2 }"########,
        ),
        (
            r########"zero-generator-after-lazy"########,
            r########"(new Dynamic { lazy = throw("later") } { for (x in List()) { [x] = x } }).length()"########,
            r########"value"########,
            r########"0"########,
        ),
        (
            r########"for-source-first-error"########,
            r########"(moduleBase) { for (x in List(throw("source"))) { [x] = x } }"########,
            r########"error"########,
            r########"–– Pkl Error ––
source

1 | (moduleBase) { for (x in List(throw("source"))) { [x] = x } }
                                  ^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"for-body-first-error"########,
            r########"(moduleBase) { for (x in List(1)) { [throw("key")] = x } }"########,
            r########"error"########,
            r########"–– Pkl Error ––
key

1 | (moduleBase) { for (x in List(1)) { [throw("key")] = x } }
                                         ^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"when-condition-first-error"########,
            r########"(moduleBase) { when (throw("condition")) { a = 1 } }"########,
            r########"error"########,
            r########"–– Pkl Error ––
condition

1 | (moduleBase) { when (throw("condition")) { a = 1 } }
                         ^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"when-false-skip-error"########,
            r########"(moduleBase) { when (false) { [throw("skipped")] = 1 } }"########,
            r########"value"########,
            r########"new Dynamic { first = 1 }"########,
        ),
        (
            r########"one-amend"########,
            r########"((one) { x -> amended = x + 1 }).apply(2)"########,
            r########"value"########,
            r########"new Dynamic { original = 2; amended = 3 }"########,
        ),
        (
            r########"one-amend-read"########,
            r########"((one) { x -> amended = x + 1 }).apply(2).amended"########,
            r########"value"########,
            r########"3"########,
        ),
        (
            r########"one-amend-repeated"########,
            r########"((one) { x -> first = x + 1 } { x -> second = x + 2 }).apply(2)"########,
            r########"value"########,
            r########"new Dynamic { original = 2; first = 3; second = 4 }"########,
        ),
        (
            r########"one-amend-zero-parameter"########,
            r########"((one) { amended = 7 }).apply(2)"########,
            r########"value"########,
            r########"new Dynamic { original = 2; amended = 7 }"########,
        ),
        (
            r########"one-amend-wrong-two"########,
            r########"((one) { x, y -> amended = x + y }).apply(2)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected a function with 2 parameters, but got 1.

1 | ((one) { x, y -> amended = x + y }).apply(2)
      ^^^
at  (repl:text)
"########,
        ),
        (
            r########"one-amend-typed-good"########,
            r########"((one) { x: Int -> amended = x + 1 }).apply(2)"########,
            r########"value"########,
            r########"new Dynamic { original = 2; amended = 3 }"########,
        ),
        (
            r########"one-amend-typed-bad"########,
            r########"((one) { x: String -> amended = x }).apply(2)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected value of type `String`, but got type `Int`.
Value: 2

1 | ((one) { x: String -> amended = x }).apply(2)
                ^^^^^^
at repl:text.<function> (repl:text)

1 | ((one) { x: String -> amended = x }).apply(2)
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"two-amend"########,
            r########"((two) { x, y -> amended = x * y }).apply(2, 3)"########,
            r########"value"########,
            r########"new Dynamic { original = 5; amended = 6 }"########,
        ),
        (
            r########"two-amend-wrong-one"########,
            r########"((two) { x -> amended = x }).apply(2, 3)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected a function with 1 parameters, but got 2.

1 | ((two) { x -> amended = x }).apply(2, 3)
      ^^^
at  (repl:text)
"########,
        ),
        (
            r########"two-amend-typed-second"########,
            r########"((two) { x, y: Int -> amended = x + y }).apply(2, 3)"########,
            r########"value"########,
            r########"new Dynamic { original = 5; amended = 5 }"########,
        ),
        (
            r########"curried-first"########,
            r########"((curried) { x -> first = x }).apply(2).apply(3)"########,
            r########"value"########,
            r########"new Dynamic { original = 5; first = 2 }"########,
        ),
        (
            r########"curried-second"########,
            r########"(((curried).apply(2)) { y -> second = y }).apply(3)"########,
            r########"value"########,
            r########"new Dynamic { original = 5; second = 3 }"########,
        ),
        (
            r########"curried-both"########,
            r########"((((curried) { x -> first = x }).apply(2)) { y -> second = y }).apply(3)"########,
            r########"value"########,
            r########"new Dynamic { original = 5; first = 2; second = 3 }"########,
        ),
        (
            r########"function-property-error"########,
            r########"(one) { x -> amended = throw("callback") }.apply(2).amended"########,
            r########"error"########,
            r########"–– Pkl Error ––
callback

1 | (one) { x -> amended = throw("callback") }.apply(2).amended
                           ^^^^^^^^^^^^^^^^^
at repl:text.amended (repl:text)

1 | (one) { x -> amended = throw("callback") }.apply(2).amended
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"function-body-arity-first"########,
            r########"((one) { x, y -> amended = throw("body") }).apply(2)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected a function with 2 parameters, but got 1.

1 | ((one) { x, y -> amended = throw("body") }).apply(2)
      ^^^
at  (repl:text)
"########,
        ),
        (
            r########"null-default-empty"########,
            r########"(nullBase) {}"########,
            r########"value"########,
            r########"new Dynamic { first = 1 }"########,
        ),
        (
            r########"null-default-property"########,
            r########"(nullBase) { second = 2 }"########,
            r########"value"########,
            r########"new Dynamic { first = 1; second = 2 }"########,
        ),
        (
            r########"null-default-property-read"########,
            r########"((nullBase) { second = 2 }).first"########,
            r########"value"########,
            r########"1"########,
        ),
        (
            r########"null-bare-empty"########,
            r########"(null) {}"########,
            r########"value"########,
            r########"new Dynamic {}"########,
        ),
        (
            r########"null-bare-property"########,
            r########"(null) { value = 1 }"########,
            r########"value"########,
            r########"new Dynamic { value = 1 }"########,
        ),
        (
            r########"new-inferred-owner"########,
            r########"new Owner { child { value = 5 } }.child.value"########,
            r########"value"########,
            r########"5"########,
        ),
        (
            r########"new-inferred-nested"########,
            r########"new Owner { child { nested { left = 5 } } }.child.nested.left"########,
            r########"value"########,
            r########"5"########,
        ),
        (
            r########"new-alias-user"########,
            r########"new BoxAlias { value = 5 }"########,
            r########"value"########,
            r########"new Box { value = 5; nested { left = 10 }; extra = 2 }"########,
        ),
        (
            r########"new-alias-listing"########,
            r########"new NumberListing { 1; 2 }"########,
            r########"value"########,
            r########"new Listing { 1; 2 }"########,
        ),
        (
            r########"new-alias-mapping"########,
            r########"new StringIntMapping { ["x"] = 1 }"########,
            r########"value"########,
            r########"new Mapping { ["x"] = 1 }"########,
        ),
        (
            r########"direct-alias-user"########,
            r########"(BoxAlias) { value = 5 }"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot instantiate, or amend an instance of, external class `TypeAlias`.

1 | (BoxAlias) { value = 5 }
     ^^^^^^^^
at  (repl:text)

To create an instance of an external class, use its literal syntax or constructor method.
Examples:
* `123` uses literal syntax to create an instance of class `Int`.
* `Pair(1, 2)` uses the `Pair()` constructor method to create an instance of class `Pair`.
"########,
        ),
        (
            r########"direct-alias-listing"########,
            r########"(NumberListing) { 1; 2 }"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot instantiate, or amend an instance of, external class `TypeAlias`.

1 | (NumberListing) { 1; 2 }
     ^^^^^^^^^^^^^
at  (repl:text)

To create an instance of an external class, use its literal syntax or constructor method.
Examples:
* `123` uses literal syntax to create an instance of class `Int`.
* `Pair(1, 2)` uses the `Pair()` constructor method to create an instance of class `Pair`.
"########,
        ),
        (
            r########"direct-alias-mapping"########,
            r########"(StringIntMapping) { ["x"] = 1 }"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot instantiate, or amend an instance of, external class `TypeAlias`.

1 | (StringIntMapping) { ["x"] = 1 }
     ^^^^^^^^^^^^^^^^
at  (repl:text)

To create an instance of an external class, use its literal syntax or constructor method.
Examples:
* `123` uses literal syntax to create an instance of class `Int`.
* `Pair(1, 2)` uses the `Pair()` constructor method to create an instance of class `Pair`.
"########,
        ),
        (
            r########"generic-listing-good"########,
            r########"new Listing<Int> { 1; 2 }"########,
            r########"value"########,
            r########"new Listing { 1; 2 }"########,
        ),
        (
            r########"generic-listing-bad"########,
            r########"new Listing<Int> { "wrong" }"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected value of type `Int`, but got type `String`.
Value: "wrong"

1 | new Listing<Int> { "wrong" }
                ^^^
at  (repl:text)

1 | new Listing<Int> { "wrong" }
                       ^^^^^^^
at repl:text[#1] (repl:text)
"########,
        ),
        (
            r########"generic-mapping-good"########,
            r########"new Mapping<String, Int> { ["x"] = 1 }"########,
            r########"value"########,
            r########"new Mapping { ["x"] = 1 }"########,
        ),
        (
            r########"generic-mapping-bad-key"########,
            r########"new Mapping<String, Int> { [1] = 2 }"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected value of type `String`, but got type `Int`.
Value: 1

1 | new Mapping<String, Int> { [1] = 2 }
                ^^^^^^
at  (repl:text)

1 | new Mapping<String, Int> { [1] = 2 }
                                ^
at repl:text[1] (repl:text)
"########,
        ),
        (
            r########"generic-mapping-bad-value"########,
            r########"new Mapping<String, Int> { ["x"] = "wrong" }"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected value of type `Int`, but got type `String`.
Value: "wrong"

1 | new Mapping<String, Int> { ["x"] = "wrong" }
                        ^^^
at  (repl:text)

1 | new Mapping<String, Int> { ["x"] = "wrong" }
                                       ^^^^^^^
at repl:text["x"] (repl:text)
"########,
        ),
        (
            r########"parent-null-then-error"########,
            r########"(Null(new Dynamic { value = throw("default") })) { value = 2 }.value"########,
            r########"value"########,
            r########"2"########,
        ),
        (
            r########"parent-error-before-body"########,
            r########"(throw("parent")) { value = throw("body") }"########,
            r########"error"########,
            r########"–– Pkl Error ––
parent

1 | (throw("parent")) { value = throw("body") }
     ^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"fold-empty-value"########,
            r########"(new Listing {}).fold(7, (acc, it) -> acc + it)"########,
            r########"value"########,
            r########"7"########,
        ),
        (
            r########"fold-empty-bad-callback"########,
            r########"(new Listing {}).fold(7, 1)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected value of type `Function2`, but got type `Int`.
Value: 1

2225 | external function fold<Result>(initial: Result, operator: (Result, Element) -> Result): Result
                                                                 ^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#Listing.fold (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L2225)

1 | (new Listing {}).fold(7, 1)
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"fold-empty-arity-zero"########,
            r########"(new Listing {}).fold(7, () -> 1)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected value of type `Function2`, but got type `Function0`.
Value: new Function0 {}

2225 | external function fold<Result>(initial: Result, operator: (Result, Element) -> Result): Result
                                                                 ^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#Listing.fold (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L2225)

1 | (new Listing {}).fold(7, () -> 1)
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"fold-empty-arity-one"########,
            r########"(new Listing {}).fold(7, (x) -> x)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected value of type `Function2`, but got type `Function1`.
Value: new Function1 {}

2225 | external function fold<Result>(initial: Result, operator: (Result, Element) -> Result): Result
                                                                 ^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#Listing.fold (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L2225)

1 | (new Listing {}).fold(7, (x) -> x)
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"fold-two-sum"########,
            r########"(new Listing { 1; 2 }).fold(0, (acc, it) -> acc + it)"########,
            r########"value"########,
            r########"3"########,
        ),
        (
            r########"fold-two-list"########,
            r########"(new Listing { 1; 2 }).fold(List(), (acc, it) -> acc.add(it))"########,
            r########"value"########,
            r########"List(1, 2)"########,
        ),
        (
            r########"fold-first-callback-error"########,
            r########"(new Listing { 1; 2 }).fold(0, (acc, it) -> throw("callback-first"))"########,
            r########"error"########,
            r########"–– Pkl Error ––
callback-first

1 | (new Listing { 1; 2 }).fold(0, (acc, it) -> throw("callback-first"))
                                                ^^^^^^^^^^^^^^^^^^^^^^^
at repl:text.<function#1> (repl:text)

1 | (new Listing { 1; 2 }).fold(0, (acc, it) -> throw("callback-first"))
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"fold-later-value-before-callback"########,
            r########"(new Listing { 1; throw("later-value") }).fold(0, (acc, it) -> throw("callback-first"))"########,
            r########"error"########,
            r########"–– Pkl Error ––
later-value

1 | (new Listing { 1; throw("later-value") }).fold(0, (acc, it) -> throw("callback-first"))
                      ^^^^^^^^^^^^^^^^^^^^
at repl:text[#2] (repl:text)

1 | (new Listing { 1; throw("later-value") }).fold(0, (acc, it) -> throw("callback-first"))
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"fold-first-value-before-later"########,
            r########"(new Listing { throw("first-value"); throw("later-value") }).fold(0, (acc, it) -> throw("callback"))"########,
            r########"error"########,
            r########"–– Pkl Error ––
first-value

1 | (new Listing { throw("first-value"); throw("later-value") }).fold(0, (acc, it) -> throw("callback"))
                   ^^^^^^^^^^^^^^^^^^^^
at repl:text[#1] (repl:text)

1 | (new Listing { throw("first-value"); throw("later-value") }).fold(0, (acc, it) -> throw("callback"))
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"fold-later-vs-first-callback"########,
            r########"(new Listing { 1; 2; throw("third-value") }).fold(0, (acc, it) -> throw("callback-first"))"########,
            r########"error"########,
            r########"–– Pkl Error ––
third-value

1 | (new Listing { 1; 2; throw("third-value") }).fold(0, (acc, it) -> throw("callback-first"))
                         ^^^^^^^^^^^^^^^^^^^^
at repl:text[#3] (repl:text)

1 | (new Listing { 1; 2; throw("third-value") }).fold(0, (acc, it) -> throw("callback-first"))
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"fold-trace-before-callback"########,
            r########"(new Listing { trace("first-value"); trace("later-value") }).fold(0, (acc, it) -> throw("callback-first"))"########,
            r########"error"########,
            r########"–– Pkl Error ––
callback-first

1 | (new Listing { trace("first-value"); trace("later-value") }).fold(0, (acc, it) -> throw("callback-first"))
                                                                                      ^^^^^^^^^^^^^^^^^^^^^^^
at repl:text.<function#1> (repl:text)

1 | (new Listing { trace("first-value"); trace("later-value") }).fold(0, (acc, it) -> throw("callback-first"))
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"fold-nested"########,
            r########"(new Listing { new Listing { 1; 2 }; new Listing { 3 } }).fold(List(), (acc, inner) -> inner.fold(acc, (xs, it) -> xs.add(it)))"########,
            r########"value"########,
            r########"List(1, 2, 3)"########,
        ),
        (
            r########"fold-typed-good"########,
            r########"(new Listing<Int> { 1; 2 }).fold(0, (acc: Int, it: Int) -> acc + it)"########,
            r########"value"########,
            r########"3"########,
        ),
        (
            r########"fold-typed-bad"########,
            r########"(new Listing<Int> { 1; 2 }).fold(0, (acc: String, it) -> acc)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected value of type `String`, but got type `Int`.
Value: 0

1 | (new Listing<Int> { 1; 2 }).fold(0, (acc: String, it) -> acc)
                                              ^^^^^^
at repl:text.<function#1> (repl:text)

1 | (new Listing<Int> { 1; 2 }).fold(0, (acc: String, it) -> acc)
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"fold-generator-zero"########,
            r########"(new Listing { for (x in List()) { x } }).fold(9, (acc, it) -> throw("callback"))"########,
            r########"value"########,
            r########"9"########,
        ),
        (
            r########"fold-amended-chain"########,
            r########"(new Listing { new Listing { 1 } } { [0] { 2 } }).fold(List(), (acc, inner) -> inner.fold(acc, (xs, it) -> xs.add(it)))"########,
            r########"value"########,
            r########"List(1, 2)"########,
        ),
        (
            r########"syntax-missing-body"########,
            r########"new Dynamic { value = 1 } {"########,
            r########"error"########,
            r########"–– Pkl Error ––
Missing `}` delimiter.

1 | new Dynamic { value = 1 } {
    ^
at repl:text (repl:text)
"########,
        ),
        (
            r########"syntax-double-arrow"########,
            r########"((one) { x -> -> value = x }).apply(1)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Unexpected token `->`. Expected `}`.

1 | ((one) { x -> -> value = x }).apply(1)
                  ^^
at repl:text (repl:text)
"########,
        ),
        (
            r########"syntax-property-annotation-amend"########,
            r########"new Dynamic { value: Int { 1 } }"########,
            r########"error"########,
            r########"–– Pkl Error ––
Unexpected token `{`. Expected `=`.

1 | new Dynamic { value: Int { 1 } }
                             ^
at repl:text (repl:text)
"########,
        ),
        (
            r########"syntax-entry-missing-key"########,
            r########"new Mapping { [] { value = 1 } }"########,
            r########"error"########,
            r########"–– Pkl Error ––
Unexpected token `]`. Expected `]`.

1 | new Mapping { [] { value = 1 } }
                   ^
at repl:text (repl:text)
"########,
        ),
        (
            r########"syntax-predicate-unclosed"########,
            r########"new Listing { [[ true ] { 1 } }"########,
            r########"error"########,
            r########"–– Pkl Error ––
Unexpected token `{`. Expected `]]`.

1 | new Listing { [[ true ] { 1 } }
                          ^
at repl:text (repl:text)
"########,
        ),
        (
            r########"invalid-primitive-amend"########,
            r########"(1) { value = 2 }"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot instantiate, or amend an instance of, external class `Int`.

1 | (1) { value = 2 }
     ^
at  (repl:text)

To create an instance of an external class, use its literal syntax or constructor method.
Examples:
* `123` uses literal syntax to create an instance of class `Int`.
* `Pair(1, 2)` uses the `Pair()` constructor method to create an instance of class `Pair`.
"########,
        ),
        (
            r########"invalid-string-amend"########,
            r########"("text") { value = 2 }"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot instantiate, or amend an instance of, external class `String`.

1 | ("text") { value = 2 }
     ^^^^^^
at  (repl:text)

To create an instance of an external class, use its literal syntax or constructor method.
Examples:
* `123` uses literal syntax to create an instance of class `Int`.
* `Pair(1, 2)` uses the `Pair()` constructor method to create an instance of class `Pair`.
"########,
        ),
        (
            r########"invalid-list-amend"########,
            r########"(List(1)) { 2 }"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot instantiate, or amend an instance of, external class `List`.

1 | (List(1)) { 2 }
     ^^^^^^^
at  (repl:text)

To create an instance of an external class, use its literal syntax or constructor method.
Examples:
* `123` uses literal syntax to create an instance of class `Int`.
* `Pair(1, 2)` uses the `Pair()` constructor method to create an instance of class `Pair`.
"########,
        ),
        (
            r########"invalid-map-amend"########,
            r########"(Map("k",1)) { ["x"] = 2 }"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot instantiate, or amend an instance of, external class `Map`.

1 | (Map("k",1)) { ["x"] = 2 }
     ^^^^^^^^^^
at  (repl:text)

To create an instance of an external class, use its literal syntax or constructor method.
Examples:
* `123` uses literal syntax to create an instance of class `Int`.
* `Pair(1, 2)` uses the `Pair()` constructor method to create an instance of class `Pair`.
"########,
        ),
        (
            r########"dynamic-class-when-property"########,
            r########"((Dynamic) { when (true) { value = 7 } }).value"########,
            r########"value"########,
            r########"7"########,
        ),
        (
            r########"dynamic-class-for-entry"########,
            r########"((Dynamic) { for (x in List(1, 2)) { [x] = x } }).toMap()"########,
            r########"value"########,
            r########"Map(1, 1, 2, 2)"########,
        ),
        (
            r########"listing-class-for-element"########,
            r########"((Listing) { for (x in List(1, 2)) { x } }).toList()"########,
            r########"value"########,
            r########"List(1, 2)"########,
        ),
        (
            r########"mapping-class-for-entry"########,
            r########"((Mapping) { for (x in List("a", "b")) { [x] = x } }).toMap()"########,
            r########"value"########,
            r########"Map("a", "a", "b", "b")"########,
        ),
        (
            r########"user-class-when-property"########,
            r########"((Box) { when (true) { value = 7 } }).value"########,
            r########"value"########,
            r########"7"########,
        ),
        (
            r########"dynamic-object-when-property"########,
            r########"((moduleBase) { when (true) { value = 7 } }).value"########,
            r########"value"########,
            r########"7"########,
        ),
        (
            r########"listing-object-for-element"########,
            r########"((listBase) { for (x in List(3)) { x } }).length"########,
            r########"value"########,
            r########"3"########,
        ),
        (
            r########"mapping-object-for-entry"########,
            r########"((mapBase) { for (x in List("j")) { [x] = 9 } })["j"]"########,
            r########"value"########,
            r########"9"########,
        ),
        (
            r########"zero-function-amend"########,
            r########"((zero) { added = 8 }).apply().added"########,
            r########"value"########,
            r########"8"########,
        ),
        (
            r########"zero-function-bad-parameter"########,
            r########"((zero) { x -> added = x }).apply()"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected a function with 1 parameters, but got 0.

1 | ((zero) { x -> added = x }).apply()
      ^^^^
at  (repl:text)
"########,
        ),
        (
            r########"primitive-function-result"########,
            r########"((primitiveResult) { x -> added = 8 }).apply(3)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot instantiate, or amend an instance of, external class `Int`.

1 | ((primitiveResult) { x -> added = 8 }).apply(3)
      ^^^^^^^^^^^^^^^
at repl:text.<function> (repl:text)

To create an instance of an external class, use its literal syntax or constructor method.
Examples:
* `123` uses literal syntax to create an instance of class `Int`.
* `Pair(1, 2)` uses the `Pair()` constructor method to create an instance of class `Pair`.

1 | ((primitiveResult) { x -> added = 8 }).apply(3)
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"function-this-original"########,
            r########"((one) { x -> added = this.original + x }).apply(2).added"########,
            r########"value"########,
            r########"4"########,
        ),
        (
            r########"dependent-base"########,
            r########"(new Dependent {}).greeting"########,
            r########"value"########,
            r########"hello-base"########,
        ),
        (
            r########"dependent-override"########,
            r########"(new Dependent { name = "child" }).greeting"########,
            r########"value"########,
            r########"hello-child"########,
        ),
        (
            r########"dependent-amend"########,
            r########"((new Dependent {}) { name = "child" }).greeting"########,
            r########"value"########,
            r########"hello-child"########,
        ),
        (
            r########"fixed-member-amend"########,
            r########"(new Dynamic { fixed item = new Dynamic { x = 1 } }) { item { y = 2 } }"########,
            r########"error"########,
            r########"–– Pkl Error ––
Modifier `fixed` is not applicable to object members.

1 | (new Dynamic { fixed item = new Dynamic { x = 1 } }) { item { y = 2 } }
                   ^^^^^
at repl:text (repl:text)
"########,
        ),
        (
            r########"const-member-amend"########,
            r########"(new Dynamic { const item = new Dynamic { x = 1 } }) { item { y = 2 } }"########,
            r########"error"########,
            r########"–– Pkl Error ––
Modifier `const` can only be applied to object members that are also `local`.

1 | (new Dynamic { const item = new Dynamic { x = 1 } }) { item { y = 2 } }
                   ^^^^^
at repl:text (repl:text)
"########,
        ),
        (
            r########"local-member-amend"########,
            r########"(new Dynamic { local item = new Dynamic { x = 1 } }) { item { y = 2 } }"########,
            r########"value"########,
            r########"new Dynamic { item { y = 2 } }"########,
        ),
        (
            r########"hidden-member-amend"########,
            r########"(new Dynamic { hidden item = new Dynamic { x = 1 } }) { item { y = 2 } }"########,
            r########"error"########,
            r########"–– Pkl Error ––
Modifier `hidden` is not applicable to object members.

1 | (new Dynamic { hidden item = new Dynamic { x = 1 } }) { item { y = 2 } }
                   ^^^^^^
at repl:text (repl:text)
"########,
        ),
        (
            r########"parent-before-generator-source"########,
            r########"(throw("parent-first")) { for (x in List(throw("source"))) { [x] = throw("body") } }"########,
            r########"error"########,
            r########"–– Pkl Error ––
parent-first

1 | (throw("parent-first")) { for (x in List(throw("source"))) { [x] = throw("body") } }
     ^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"generator-source-before-key"########,
            r########"(new Dynamic {}) { for (x in List(throw("source"))) { [throw("key")] = throw("body") } }"########,
            r########"error"########,
            r########"–– Pkl Error ––
source

1 | (new Dynamic {}) { for (x in List(throw("source"))) { [throw("key")] = throw("body") } }
                                      ^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"generator-key-before-body"########,
            r########"(new Dynamic {}) { for (x in List(1)) { [throw("key")] = throw("body") } }"########,
            r########"error"########,
            r########"–– Pkl Error ––
key

1 | (new Dynamic {}) { for (x in List(1)) { [throw("key")] = throw("body") } }
                                             ^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"when-condition-before-key"########,
            r########"(new Dynamic {}) { when (throw("condition")) { [throw("key")] = throw("body") } }"########,
            r########"error"########,
            r########"–– Pkl Error ––
condition

1 | (new Dynamic {}) { when (throw("condition")) { [throw("key")] = throw("body") } }
                             ^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"zero-after-lazy-access"########,
            r########"((new Dynamic { lazy = throw("later") }) { when (false) { [1] = 2 } }).lazy"########,
            r########"error"########,
            r########"–– Pkl Error ––
later

1 | ((new Dynamic { lazy = throw("later") }) { when (false) { [1] = 2 } }).lazy
                           ^^^^^^^^^^^^^^
at repl:text.lazy (repl:text)

1 | ((new Dynamic { lazy = throw("later") }) { when (false) { [1] = 2 } }).lazy
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"typed-empty-for-entry"########,
            r########"(boxBase) { for (x in List()) { [x] = x } }"########,
            r########"value"########,
            r########"new Box { value = 1; nested { left = 10 }; extra = 2 }"########,
        ),
        (
            r########"typed-false-when"########,
            r########"(boxBase) { when (false) { value = throw("unused") } }"########,
            r########"value"########,
            r########"new Box { value = 1; nested { left = 10 }; extra = 2 }"########,
        ),
        (
            r########"typed-empty-spread"########,
            r########"(boxBase) { ...new Dynamic {} }"########,
            r########"value"########,
            r########"new Box { value = 1; nested { left = 10 }; extra = 2 }"########,
        ),
        (
            r########"inherited-empty-for"########,
            r########"((nestedBase) { added = 2 }) { for (x in List()) { [x] = x } }"########,
            r########"value"########,
            r########"new Dynamic { nested { value = 1 }; added = 2 }"########,
        ),
        (
            r########"inherited-empty-spread-read"########,
            r########"(((nestedBase) { added = 2 }) { ...new Dynamic {} }).nested.value"########,
            r########"value"########,
            r########"1"########,
        ),
        (
            r########"inherited-false-when-read"########,
            r########"(((nestedBase) { added = 2 }) { when (false) { ["skip"] = 9 } }).added"########,
            r########"value"########,
            r########"2"########,
        ),
        (
            r########"fold-overridden-parent-throw"########,
            r########"(new Listing { throw("parent-first"); 2 } { [0] = 1 }).fold(0, (acc, it) -> throw("callback"))"########,
            r########"error"########,
            r########"–– Pkl Error ––
callback

1 | (new Listing { throw("parent-first"); 2 } { [0] = 1 }).fold(0, (acc, it) -> throw("callback"))
                                                                                ^^^^^^^^^^^^^^^^^
at repl:text.<function#1> (repl:text)

1 | (new Listing { throw("parent-first"); 2 } { [0] = 1 }).fold(0, (acc, it) -> throw("callback"))
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"fold-child-before-inherited"########,
            r########"(new Listing { 1; throw("inherited-second") } { [0] = throw("child-first") }).fold(0, (acc, it) -> throw("callback"))"########,
            r########"error"########,
            r########"–– Pkl Error ––
child-first

1 | (new Listing { 1; throw("inherited-second") } { [0] = throw("child-first") }).fold(0, (acc, it) -> throw("callback"))
                                                          ^^^^^^^^^^^^^^^^^^^^
at repl:text[0] (repl:text)

1 | (new Listing { 1; throw("inherited-second") } { [0] = throw("child-first") }).fold(0, (acc, it) -> throw("callback"))
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"fold-inherited-before-callback"########,
            r########"(new Listing { 1; throw("inherited-second") } { [0] = 2 }).fold(0, (acc, it) -> throw("callback"))"########,
            r########"error"########,
            r########"–– Pkl Error ––
inherited-second

1 | (new Listing { 1; throw("inherited-second") } { [0] = 2 }).fold(0, (acc, it) -> throw("callback"))
                      ^^^^^^^^^^^^^^^^^^^^^^^^^
at repl:text[#2] (repl:text)

1 | (new Listing { 1; throw("inherited-second") } { [0] = 2 }).fold(0, (acc, it) -> throw("callback"))
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"fold-overridden-lazy-success"########,
            r########"(new Listing { throw("parent-first"); 2 } { [0] = 1 }).fold(0, (acc, it) -> acc + it)"########,
            r########"value"########,
            r########"3"########,
        ),
    ];
    assert_eq!(rows.len(), 188);
    for (id, expression, kind, expected) in rows {
        probe
            .evaluate_module_source(
                "file:///stage64/amendment-construction.pkl",
                Some(include_str!(
                    "../../../tests/fixtures/m2-amendment-construction/module.pkl"
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
