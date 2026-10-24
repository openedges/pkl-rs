//! Complete independent Java 0.32.0 string observations, capture-04/repeat-0.
//! Authority: m2-collection-conversions/oracle-evidence.tar.gz.
#[path = "support/module_keys.rs"]
mod module_keys;
use pkl_runtime::probe::SourceProbe;
#[test]
fn complete_java_collection_conversion_matrix() {
    let mut probe = SourceProbe::new_module_evaluator("compact", false, false, false).unwrap();
    let mut failures = Vec::new();
    let rows = [
        (
            r########"typed-empty-map"########,
            r########"new EmptyClass {}.toMap()"########,
            r########"value"########,
            r########"Map()"########,
        ),
        (
            r########"typed-base-map"########,
            r########"typedBase.toMap()"########,
            r########"value"########,
            r########"Map("name", "pigeon", "age", 3)"########,
        ),
        (
            r########"typed-derived-map"########,
            r########"typedDerived.toMap()"########,
            r########"value"########,
            r########"Map("name", "owl", "age", 4, "wings", 2)"########,
        ),
        (
            r########"typed-base-keys"########,
            r########"typedBase.toMap().keys"########,
            r########"value"########,
            r########"Set("name", "age")"########,
        ),
        (
            r########"typed-derived-keys"########,
            r########"typedDerived.toMap().keys"########,
            r########"value"########,
            r########"Set("name", "age", "wings")"########,
        ),
        (
            r########"typed-derived-values"########,
            r########"typedDerived.toMap().values"########,
            r########"value"########,
            r########"List("owl", 4, 2)"########,
        ),
        (
            r########"typed-derived-name"########,
            r########"typedDerived.toMap()["name"]"########,
            r########"value"########,
            r########"owl"########,
        ),
        (
            r########"typed-class-name"########,
            r########"typedDerived.toMap().getClass().simpleName"########,
            r########"value"########,
            r########"Map"########,
        ),
        (
            r########"typed-null-value"########,
            r########"new Holder {}.toMap().getOrNull("missing")"########,
            r########"value"########,
            r########"null"########,
        ),
        (
            r########"typed-local-excluded"########,
            r########"typedBase.toMap().containsKey("hiddenModule")"########,
            r########"value"########,
            r########"false"########,
        ),
        (
            r########"typed-map-empty-class"########,
            r########"new EmptyClass {}.toMap().isEmpty"########,
            r########"value"########,
            r########"true"########,
        ),
        (
            r########"typed-map-override"########,
            r########"(typedBase) { name = "sparrow" }.toMap()"########,
            r########"value"########,
            r########"Map("name", "sparrow", "age", 3)"########,
        ),
        (
            r########"typed-map-lazy-error"########,
            r########"new Holder { value = throw("typed-bang") }.toMap().length"########,
            r########"error"########,
            r########"–– Pkl Error ––
typed-bang

1 | new Holder { value = throw("typed-bang") }.toMap().length
                         ^^^^^^^^^^^^^^^^^^^
at repl:text.value (repl:text)

1 | new Holder { value = throw("typed-bang") }.toMap().length
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"typed-map-nested-lazy"########,
            r########"new Carrier { child = new Holder { value = throw("typed-nested") } }.toMap().length"########,
            r########"value"########,
            r########"1"########,
        ),
        (
            r########"typed-map-nested-forced"########,
            r########"new Carrier { child = new Holder { value = throw("typed-nested") } }.toMap()["child"].value"########,
            r########"error"########,
            r########"–– Pkl Error ––
typed-nested

1 | new Carrier { child = new Holder { value = throw("typed-nested") } }.toMap()["child"].value
                                               ^^^^^^^^^^^^^^^^^^^^^
at repl:text.child.value (repl:text)

1 | new Carrier { child = new Holder { value = throw("typed-nested") } }.toMap()["child"].value
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"typed-hidden-inherited-map"########,
            r########"new HiddenChild {}.toMap()"########,
            r########"value"########,
            r########"Map("visible", 3)"########,
        ),
        (
            r########"typed-hidden-inherited-keys"########,
            r########"new HiddenChild {}.toMap().keys"########,
            r########"value"########,
            r########"Set("visible")"########,
        ),
        (
            r########"typed-map-arity"########,
            r########"typedBase.toMap(1)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected 0 function arguments but got 1.

1997 | external function toMap(): Map<String, unknown>
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#Typed.toMap (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L1997)

1 | typedBase.toMap(1)
    ^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"typed-map-property"########,
            r########"typedBase.toMap"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot find property `toMap` in object of type `collection-conversions#Animal`.

1 | typedBase.toMap
    ^^^^^^^^^^^^^^^
at  (repl:text)

Available properties:
age
name
"########,
        ),
        (
            r########"typed-map-missing-member"########,
            r########"typedBase.missingField"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot find property `missingField` in object of type `collection-conversions#Animal`.

1 | typedBase.missingField
    ^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)

Available properties:
age
name
"########,
        ),
        (
            r########"module-map-keys"########,
            r########"module.toMap().keys"########,
            r########"value"########,
            r########"Set("tag", "count", "typedBase", "typedDerived", "dynamicMixed", "mappingBase", "listingBase")"########,
        ),
        (
            r########"module-map-count"########,
            r########"module.toMap()["count"]"########,
            r########"value"########,
            r########"7"########,
        ),
        (
            r########"module-map-tag"########,
            r########"module.toMap()["tag"]"########,
            r########"value"########,
            r########"stage62"########,
        ),
        (
            r########"module-map-class-excluded"########,
            r########"module.toMap().containsKey("Animal")"########,
            r########"value"########,
            r########"false"########,
        ),
        (
            r########"module-map-local-excluded"########,
            r########"module.toMap().containsKey("hiddenModule")"########,
            r########"value"########,
            r########"false"########,
        ),
        (
            r########"module-map-size"########,
            r########"module.toMap().length"########,
            r########"value"########,
            r########"7"########,
        ),
        (
            r########"module-map-arity"########,
            r########"module.toMap(1)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected 0 function arguments but got 1.

1997 | external function toMap(): Map<String, unknown>
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#Typed.toMap (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L1997)

1 | module.toMap(1)
    ^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"dynamic-empty-length"########,
            r########"new Dynamic {}.length()"########,
            r########"value"########,
            r########"0"########,
        ),
        (
            r########"dynamic-property-length"########,
            r########"new Dynamic { name = "bird" }.length()"########,
            r########"value"########,
            r########"0"########,
        ),
        (
            r########"dynamic-entry-length"########,
            r########"new Dynamic { ["name"] = "bird" }.length()"########,
            r########"value"########,
            r########"0"########,
        ),
        (
            r########"dynamic-elements-length"########,
            r########"new Dynamic { 1; 2; 3 }.length()"########,
            r########"value"########,
            r########"3"########,
        ),
        (
            r########"dynamic-mixed-length"########,
            r########"dynamicMixed.length()"########,
            r########"value"########,
            r########"2"########,
        ),
        (
            r########"dynamic-extended-length"########,
            r########"(dynamicMixed) { "third" }.length()"########,
            r########"value"########,
            r########"3"########,
        ),
        (
            r########"dynamic-replaced-index-length"########,
            r########"(dynamicMixed) { [0] = "zero" }.length()"########,
            r########"value"########,
            r########"2"########,
        ),
        (
            r########"dynamic-length-property"########,
            r########"dynamicMixed.length"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot find property `length` in object of type `Dynamic`.

1 | dynamicMixed.length
    ^^^^^^^^^^^^^^^^^^^
at  (repl:text)

Available properties:
default
name
"########,
        ),
        (
            r########"dynamic-length-arity"########,
            r########"dynamicMixed.length(1)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected 0 function arguments but got 1.

2018 | external function length(): Int
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#Dynamic.length (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L2018)

1 | dynamicMixed.length(1)
    ^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"dynamic-length-wrong-member"########,
            r########"dynamicMixed.missingField"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot find property `missingField` in object of type `Dynamic`.

1 | dynamicMixed.missingField
    ^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)

Available properties:
default
name
"########,
        ),
        (
            r########"dynamic-empty-map"########,
            r########"new Dynamic {}.toMap()"########,
            r########"value"########,
            r########"Map()"########,
        ),
        (
            r########"dynamic-mixed-map"########,
            r########"dynamicMixed.toMap()"########,
            r########"value"########,
            r########"Map("name", "bird", "entry", 23, 7, "seven")"########,
        ),
        (
            r########"dynamic-mixed-map-keys"########,
            r########"dynamicMixed.toMap().keys"########,
            r########"value"########,
            r########"Set("name", "entry", 7)"########,
        ),
        (
            r########"dynamic-mixed-map-values"########,
            r########"dynamicMixed.toMap().values"########,
            r########"value"########,
            r########"List("bird", 23, "seven")"########,
        ),
        (
            r########"dynamic-mixed-map-size"########,
            r########"dynamicMixed.toMap().length"########,
            r########"value"########,
            r########"3"########,
        ),
        (
            r########"dynamic-mixed-map-element-excluded"########,
            r########"dynamicMixed.toMap().containsKey(0)"########,
            r########"value"########,
            r########"false"########,
        ),
        (
            r########"dynamic-mixed-map-property"########,
            r########"dynamicMixed.toMap()["name"]"########,
            r########"value"########,
            r########"bird"########,
        ),
        (
            r########"dynamic-mixed-map-entry"########,
            r########"dynamicMixed.toMap()["entry"]"########,
            r########"value"########,
            r########"23"########,
        ),
        (
            r########"dynamic-mixed-map-number-key"########,
            r########"dynamicMixed.toMap()[7]"########,
            r########"value"########,
            r########"seven"########,
        ),
        (
            r########"dynamic-prop-entry-collision"########,
            r########"new Dynamic { name = "property"; ["name"] = "entry" }.toMap()"########,
            r########"value"########,
            r########"Map("name", "entry")"########,
        ),
        (
            r########"dynamic-boolean-null-keys"########,
            r########"new Dynamic { [true] = 1; [null] = 2 }.toMap()"########,
            r########"value"########,
            r########"Map(true, 1, null, 2)"########,
        ),
        (
            r########"dynamic-empty-list"########,
            r########"new Dynamic {}.toList()"########,
            r########"value"########,
            r########"List()"########,
        ),
        (
            r########"dynamic-mixed-list"########,
            r########"dynamicMixed.toList()"########,
            r########"value"########,
            r########"List("first", "second")"########,
        ),
        (
            r########"dynamic-mixed-list-size"########,
            r########"dynamicMixed.toList().length"########,
            r########"value"########,
            r########"2"########,
        ),
        (
            r########"dynamic-extended-list"########,
            r########"(dynamicMixed) { "third" }.toList()"########,
            r########"value"########,
            r########"List("first", "second", "third")"########,
        ),
        (
            r########"dynamic-replaced-list"########,
            r########"(dynamicMixed) { [0] = "zero" }.toList()"########,
            r########"value"########,
            r########"List("zero", "second")"########,
        ),
        (
            r########"dynamic-property-list-empty"########,
            r########"new Dynamic { name = "bird" }.toList()"########,
            r########"value"########,
            r########"List()"########,
        ),
        (
            r########"dynamic-entry-list-empty"########,
            r########"new Dynamic { ["name"] = "bird" }.toList()"########,
            r########"value"########,
            r########"List()"########,
        ),
        (
            r########"dynamic-list-arity"########,
            r########"dynamicMixed.toList(1)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected 0 function arguments but got 1.

2037 | external function toList(): List<unknown>
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#Dynamic.toList (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L2037)

1 | dynamicMixed.toList(1)
    ^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"dynamic-map-arity"########,
            r########"dynamicMixed.toMap(1)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected 0 function arguments but got 1.

2034 | external function toMap(): Map<unknown, unknown>
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#Dynamic.toMap (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L2034)

1 | dynamicMixed.toMap(1)
    ^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"dynamic-map-property"########,
            r########"dynamicMixed.toMap"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot find property `toMap` in object of type `Dynamic`.

1 | dynamicMixed.toMap
    ^^^^^^^^^^^^^^^^^^
at  (repl:text)

Available properties:
default
name
"########,
        ),
        (
            r########"dynamic-list-property"########,
            r########"dynamicMixed.toList"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot find property `toList` in object of type `Dynamic`.

1 | dynamicMixed.toList
    ^^^^^^^^^^^^^^^^^^^
at  (repl:text)

Available properties:
default
name
"########,
        ),
        (
            r########"dynamic-discarded-element-error"########,
            r########"new Dynamic { name = 1; throw("discarded-element") }.toMap().length"########,
            r########"error"########,
            r########"–– Pkl Error ––
discarded-element

1 | new Dynamic { name = 1; throw("discarded-element") }.toMap().length
                            ^^^^^^^^^^^^^^^^^^^^^^^^^^
at repl:text[#1] (repl:text)

1 | new Dynamic { name = 1; throw("discarded-element") }.toMap().length
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"dynamic-discarded-property-error"########,
            r########"new Dynamic { name = throw("discarded-property"); 1 }.toList().length"########,
            r########"error"########,
            r########"–– Pkl Error ––
discarded-property

1 | new Dynamic { name = throw("discarded-property"); 1 }.toList().length
                         ^^^^^^^^^^^^^^^^^^^^^^^^^^^
at repl:text.name (repl:text)

1 | new Dynamic { name = throw("discarded-property"); 1 }.toList().length
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"dynamic-discarded-entry-error"########,
            r########"new Dynamic { ["bad"] = throw("discarded-entry"); 1 }.toList().length"########,
            r########"error"########,
            r########"–– Pkl Error ––
discarded-entry

1 | new Dynamic { ["bad"] = throw("discarded-entry"); 1 }.toList().length
                            ^^^^^^^^^^^^^^^^^^^^^^^^
at repl:text["bad"] (repl:text)

1 | new Dynamic { ["bad"] = throw("discarded-entry"); 1 }.toList().length
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"dynamic-first-error-self"########,
            r########"new Dynamic { name = throw("first-property"); ["bad"] = throw("second-entry"); throw("third-element") }.toMap()"########,
            r########"error"########,
            r########"–– Pkl Error ––
first-property

1 | new Dynamic { name = throw("first-property"); ["bad"] = throw("second-entry"); throw("third-element") }.toMap()
                         ^^^^^^^^^^^^^^^^^^^^^^^
at repl:text.name (repl:text)

1 | new Dynamic { name = throw("first-property"); ["bad"] = throw("second-entry"); throw("third-element") }.toMap()
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"dynamic-child-before-parent-error"########,
            r########"(new Dynamic { parent = throw("parent-bang") }) { child = throw("child-bang") }.toMap()"########,
            r########"error"########,
            r########"–– Pkl Error ––
child-bang

1 | (new Dynamic { parent = throw("parent-bang") }) { child = throw("child-bang") }.toMap()
                                                              ^^^^^^^^^^^^^^^^^^^
at repl:text.child (repl:text)

1 | (new Dynamic { parent = throw("parent-bang") }) { child = throw("child-bang") }.toMap()
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"dynamic-hidden-typed-source"########,
            r########"new HiddenChild {}.toDynamic().toMap()"########,
            r########"value"########,
            r########"Map("visible", 3)"########,
        ),
        (
            r########"dynamic-shadow-method-call"########,
            r########"new Dynamic { toMap = 42 }.toMap()"########,
            r########"value"########,
            r########"Map("toMap", 42)"########,
        ),
        (
            r########"dynamic-shadow-method-property"########,
            r########"new Dynamic { toMap = 42 }.toMap"########,
            r########"value"########,
            r########"42"########,
        ),
        (
            r########"dynamic-nested-object-shallow"########,
            r########"new Dynamic { x = new Holder { value = throw("nested") } }.toMap().length"########,
            r########"value"########,
            r########"1"########,
        ),
        (
            r########"dynamic-nested-object-forced"########,
            r########"new Dynamic { x = new Holder { value = throw("nested") } }.toMap()["x"].value"########,
            r########"error"########,
            r########"–– Pkl Error ––
nested

1 | new Dynamic { x = new Holder { value = throw("nested") } }.toMap()["x"].value
                                           ^^^^^^^^^^^^^^^
at repl:text.x.value (repl:text)

1 | new Dynamic { x = new Holder { value = throw("nested") } }.toMap()["x"].value
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"dynamic-map-inherited"########,
            r########"(dynamicMixed) { name = "owl" }.toMap()"########,
            r########"value"########,
            r########"Map("name", "owl", "entry", 23, 7, "seven")"########,
        ),
        (
            r########"dynamic-list-inherited"########,
            r########"(dynamicMixed) { name = "owl" }.toList()"########,
            r########"value"########,
            r########"List("first", "second")"########,
        ),
        (
            r########"dynamic-override-kind-element-to-entry-map"########,
            r########"(dynamicMixed) { [0] = "zero" }.toMap()"########,
            r########"value"########,
            r########"Map("name", "bird", "entry", 23, 7, "seven")"########,
        ),
        (
            r########"dynamic-override-kind-element-to-entry-list"########,
            r########"(dynamicMixed) { [0] = "zero" }.toList()"########,
            r########"value"########,
            r########"List("zero", "second")"########,
        ),
        (
            r########"mapping-empty-map"########,
            r########"new Mapping {}.toMap()"########,
            r########"value"########,
            r########"Map()"########,
        ),
        (
            r########"mapping-base-map"########,
            r########"mappingBase.toMap()"########,
            r########"value"########,
            r########"Map("a", 1, "b", 2)"########,
        ),
        (
            r########"mapping-base-keys"########,
            r########"mappingBase.toMap().keys"########,
            r########"value"########,
            r########"Set("a", "b")"########,
        ),
        (
            r########"mapping-base-values"########,
            r########"mappingBase.toMap().values"########,
            r########"value"########,
            r########"List(1, 2)"########,
        ),
        (
            r########"mapping-override-map"########,
            r########"(mappingBase) { ["a"] = 9; ["c"] = 3 }.toMap()"########,
            r########"value"########,
            r########"Map("a", 9, "b", 2, "c", 3)"########,
        ),
        (
            r########"mapping-override-keys"########,
            r########"(mappingBase) { ["a"] = 9; ["c"] = 3 }.toMap().keys"########,
            r########"value"########,
            r########"Set("a", "b", "c")"########,
        ),
        (
            r########"mapping-null-key"########,
            r########"new Mapping { [null] = 1 }.toMap()"########,
            r########"value"########,
            r########"Map(null, 1)"########,
        ),
        (
            r########"mapping-null-value"########,
            r########"new Mapping { ["x"] = null }.toMap()"########,
            r########"value"########,
            r########"Map("x", null)"########,
        ),
        (
            r########"mapping-local-default-excluded"########,
            r########"new Mapping { local x = 1; default = (_) -> 4; ["a"] = 2 }.toMap()"########,
            r########"value"########,
            r########"Map("a", 2)"########,
        ),
        (
            r########"mapping-lazy-error"########,
            r########"new Mapping { ["x"] = throw("mapping-bang") }.toMap().length"########,
            r########"error"########,
            r########"–– Pkl Error ––
mapping-bang

1 | new Mapping { ["x"] = throw("mapping-bang") }.toMap().length
                          ^^^^^^^^^^^^^^^^^^^^^
at repl:text["x"] (repl:text)

1 | new Mapping { ["x"] = throw("mapping-bang") }.toMap().length
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"mapping-nested-shallow"########,
            r########"new Mapping { ["x"] = new Holder { value = throw("nested-mapping") } }.toMap().length"########,
            r########"value"########,
            r########"1"########,
        ),
        (
            r########"mapping-nested-forced"########,
            r########"new Mapping { ["x"] = new Holder { value = throw("nested-mapping") } }.toMap()["x"].value"########,
            r########"error"########,
            r########"–– Pkl Error ––
nested-mapping

1 | new Mapping { ["x"] = new Holder { value = throw("nested-mapping") } }.toMap()["x"].value
                                               ^^^^^^^^^^^^^^^^^^^^^^^
at repl:text["x"].value (repl:text)

1 | new Mapping { ["x"] = new Holder { value = throw("nested-mapping") } }.toMap()["x"].value
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"mapping-map-arity"########,
            r########"mappingBase.toMap(1)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected 0 function arguments but got 1.

2306 | external function toMap(): Map<Key, Value>
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#Mapping.toMap (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L2306)

1 | mappingBase.toMap(1)
    ^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"mapping-map-property"########,
            r########"mappingBase.toMap"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot find property `toMap` in object of type `Mapping`.

1 | mappingBase.toMap
    ^^^^^^^^^^^^^^^^^
at  (repl:text)

Available properties:
default
isEmpty
isNotEmpty
keys
length
"########,
        ),
        (
            r########"mapping-typed-good"########,
            r########"(new Mapping<String, Int> { ["a"] = 1 }).toMap()"########,
            r########"value"########,
            r########"Map("a", 1)"########,
        ),
        (
            r########"mapping-typed-bad-key"########,
            r########"(new Mapping<String, Int> { [1] = 2 }).toMap()"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected value of type `String`, but got type `Int`.
Value: 1

1 | (new Mapping<String, Int> { [1] = 2 }).toMap()
                 ^^^^^^
at  (repl:text)

1 | (new Mapping<String, Int> { [1] = 2 }).toMap()
                                 ^
at repl:text[1] (repl:text)
"########,
        ),
        (
            r########"mapping-typed-bad-value"########,
            r########"(new Mapping<String, Int> { ["a"] = "bad" }).toMap()"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected value of type `Int`, but got type `String`.
Value: "bad"

1 | (new Mapping<String, Int> { ["a"] = "bad" }).toMap()
                         ^^^
at  (repl:text)

1 | (new Mapping<String, Int> { ["a"] = "bad" }).toMap()
                                        ^^^^^
at repl:text["a"] (repl:text)

1 | (new Mapping<String, Int> { ["a"] = "bad" }).toMap()
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"listing-empty-list"########,
            r########"new Listing {}.toList()"########,
            r########"value"########,
            r########"List()"########,
        ),
        (
            r########"listing-base-list"########,
            r########"listingBase.toList()"########,
            r########"value"########,
            r########"List("a", "b")"########,
        ),
        (
            r########"listing-extended-list"########,
            r########"(listingBase) { "c" }.toList()"########,
            r########"value"########,
            r########"List("a", "b", "c")"########,
        ),
        (
            r########"listing-overridden-list"########,
            r########"(listingBase) { [0] = "z" }.toList()"########,
            r########"value"########,
            r########"List("z", "b")"########,
        ),
        (
            r########"listing-null-list"########,
            r########"new Listing { null; 1 }.toList()"########,
            r########"value"########,
            r########"List(null, 1)"########,
        ),
        (
            r########"listing-lazy-error"########,
            r########"new Listing { throw("listing-bang") }.toList().length"########,
            r########"error"########,
            r########"–– Pkl Error ––
listing-bang

1 | new Listing { throw("listing-bang") }.toList().length
                  ^^^^^^^^^^^^^^^^^^^^^
at repl:text[#1] (repl:text)

1 | new Listing { throw("listing-bang") }.toList().length
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"listing-nested-shallow"########,
            r########"new Listing { new Holder { value = throw("nested-listing") } }.toList().length"########,
            r########"value"########,
            r########"1"########,
        ),
        (
            r########"listing-nested-forced"########,
            r########"new Listing { new Holder { value = throw("nested-listing") } }.toList()[0].value"########,
            r########"error"########,
            r########"–– Pkl Error ––
nested-listing

1 | new Listing { new Holder { value = throw("nested-listing") } }.toList()[0].value
                                       ^^^^^^^^^^^^^^^^^^^^^^^
at repl:text[#1].value (repl:text)

1 | new Listing { new Holder { value = throw("nested-listing") } }.toList()[0].value
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"listing-empty-set"########,
            r########"new Listing {}.toSet()"########,
            r########"value"########,
            r########"Set()"########,
        ),
        (
            r########"listing-base-set"########,
            r########"listingBase.toSet()"########,
            r########"value"########,
            r########"Set("a", "b")"########,
        ),
        (
            r########"listing-duplicate-set"########,
            r########"new Listing { 1; 2; 1; 3 }.toSet()"########,
            r########"value"########,
            r########"Set(1, 2, 3)"########,
        ),
        (
            r########"listing-extended-set"########,
            r########"(listingBase) { "c" }.toSet()"########,
            r########"value"########,
            r########"Set("a", "b", "c")"########,
        ),
        (
            r########"listing-typed-good"########,
            r########"(new Listing<Int> { 1; 2 }).toList()"########,
            r########"value"########,
            r########"List(1, 2)"########,
        ),
        (
            r########"listing-typed-bad-value"########,
            r########"(new Listing<Int> { 1; "bad" }).toList()"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected value of type `Int`, but got type `String`.
Value: "bad"

1 | (new Listing<Int> { 1; "bad" }).toList()
                 ^^^
at  (repl:text)

1 | (new Listing<Int> { 1; "bad" }).toList()
                           ^^^^^
at repl:text[#2] (repl:text)

1 | (new Listing<Int> { 1; "bad" }).toList()
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"listing-list-arity"########,
            r########"listingBase.toList(1)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected 0 function arguments but got 1.

2240 | external function toList(): List<Element>
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#Listing.toList (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L2240)

1 | listingBase.toList(1)
    ^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"listing-set-arity"########,
            r########"listingBase.toSet(1)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected 0 function arguments but got 1.

2243 | external function toSet(): Set<Element>
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#Listing.toSet (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L2243)

1 | listingBase.toSet(1)
    ^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"listing-list-property"########,
            r########"listingBase.toList"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot find property `toList` in object of type `Listing`.

1 | listingBase.toList
    ^^^^^^^^^^^^^^^^^^
at  (repl:text)

Available properties:
default
distinct
first
firstOrNull
isDistinct
isEmpty
isNotEmpty
last
lastIndex
lastOrNull
length
single
singleOrNull
"########,
        ),
        (
            r########"map-empty-self"########,
            r########"Map().toMap()"########,
            r########"value"########,
            r########"Map()"########,
        ),
        (
            r########"map-nonempty-self"########,
            r########"Map("a", 1, "b", 2).toMap()"########,
            r########"value"########,
            r########"Map("a", 1, "b", 2)"########,
        ),
        (
            r########"map-null-self"########,
            r########"Map(null, null).toMap()"########,
            r########"value"########,
            r########"Map(null, null)"########,
        ),
        (
            r########"map-self-equality"########,
            r########"Map("a",1).toMap() == Map("a",1)"########,
            r########"value"########,
            r########"true"########,
        ),
        (
            r########"map-self-arity"########,
            r########"Map().toMap(1)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected 0 function arguments but got 1.

3652 | external function toMap(): Map
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#Map.toMap (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L3652)

1 | Map().toMap(1)
    ^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"map-self-property"########,
            r########"Map().toMap"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot find property `toMap` in object of type `Map`.

1 | Map().toMap
    ^^^^^^^^^^^
at  (repl:text)

Did you mean any of the following?
toMapping()
"########,
        ),
        (
            r########"list-empty-self"########,
            r########"List().toList()"########,
            r########"value"########,
            r########"List()"########,
        ),
        (
            r########"list-nonempty-self"########,
            r########"List(1, null, 2).toList()"########,
            r########"value"########,
            r########"List(1, null, 2)"########,
        ),
        (
            r########"list-self-arity"########,
            r########"List().toList(1)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected 0 function arguments but got 1.

3373 | external function toList(): List<Element>
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#List.toList (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L3373)

1 | List().toList(1)
    ^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"set-empty-self"########,
            r########"Set().toSet()"########,
            r########"value"########,
            r########"Set()"########,
        ),
        (
            r########"set-nonempty-self"########,
            r########"Set(2,1,2).toSet()"########,
            r########"value"########,
            r########"Set(2, 1)"########,
        ),
        (
            r########"set-self-arity"########,
            r########"Set().toSet(1)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected 0 function arguments but got 1.

3536 | external function toSet(): Set<Element>
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#Set.toSet (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L3536)

1 | Set().toSet(1)
    ^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"list-to-map-empty"########,
            r########"List().toMap((x) -> x, (x) -> x)"########,
            r########"value"########,
            r########"Map()"########,
        ),
        (
            r########"list-to-map-basic"########,
            r########"List(1,2,3).toMap((x) -> x, (x) -> x * 10)"########,
            r########"value"########,
            r########"Map(1, 10, 2, 20, 3, 30)"########,
        ),
        (
            r########"list-to-map-duplicate"########,
            r########"List(1,2,3).toMap((x) -> x % 2, (x) -> x * 10)"########,
            r########"value"########,
            r########"Map(1, 30, 0, 20)"########,
        ),
        (
            r########"list-to-map-duplicate-keys"########,
            r########"List(1,2,3).toMap((x) -> x % 2, (x) -> x * 10).keys"########,
            r########"value"########,
            r########"Set(1, 0)"########,
        ),
        (
            r########"list-to-map-null-key"########,
            r########"List(1,2).toMap((x) -> null, (x) -> x)"########,
            r########"value"########,
            r########"Map(null, 2)"########,
        ),
        (
            r########"list-to-map-null-value"########,
            r########"List(1,2).toMap((x) -> x, (x) -> null)"########,
            r########"value"########,
            r########"Map(1, null, 2, null)"########,
        ),
        (
            r########"list-to-map-key-throw"########,
            r########"List(1,2).toMap((x) -> throw("key-first"), (x) -> throw("value-second"))"########,
            r########"error"########,
            r########"–– Pkl Error ––
key-first

1 | List(1,2).toMap((x) -> throw("key-first"), (x) -> throw("value-second"))
                           ^^^^^^^^^^^^^^^^^^
at repl:text.<function#1> (repl:text)

1 | List(1,2).toMap((x) -> throw("key-first"), (x) -> throw("value-second"))
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"list-to-map-value-throw"########,
            r########"List(1,2).toMap((x) -> x, (x) -> throw("value-first"))"########,
            r########"error"########,
            r########"–– Pkl Error ––
value-first

1 | List(1,2).toMap((x) -> x, (x) -> throw("value-first"))
                                     ^^^^^^^^^^^^^^^^^^^^
at repl:text.<function#2> (repl:text)

1 | List(1,2).toMap((x) -> x, (x) -> throw("value-first"))
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"list-to-map-second-key-throw"########,
            r########"List(1,2).toMap((x) -> if (x == 2) throw("second-key") else x, (x) -> x)"########,
            r########"error"########,
            r########"–– Pkl Error ––
second-key

1 | List(1,2).toMap((x) -> if (x == 2) throw("second-key") else x, (x) -> x)
                                       ^^^^^^^^^^^^^^^^^^^
at repl:text.<function#1> (repl:text)

1 | List(1,2).toMap((x) -> if (x == 2) throw("second-key") else x, (x) -> x)
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"list-to-map-second-value-throw"########,
            r########"List(1,2).toMap((x) -> x, (x) -> if (x == 2) throw("second-value") else x)"########,
            r########"error"########,
            r########"–– Pkl Error ––
second-value

1 | List(1,2).toMap((x) -> x, (x) -> if (x == 2) throw("second-value") else x)
                                                 ^^^^^^^^^^^^^^^^^^^^^
at repl:text.<function#2> (repl:text)

1 | List(1,2).toMap((x) -> x, (x) -> if (x == 2) throw("second-value") else x)
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"list-to-map-no-args"########,
            r########"List(1).toMap()"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected 2 function arguments but got 0.

3377 | external function toMap<Key, Value>(
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#List.toMap (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L3377)

1 | List(1).toMap()
    ^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"list-to-map-one-arg"########,
            r########"List(1).toMap((x) -> x)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected 2 function arguments but got 1.

3377 | external function toMap<Key, Value>(
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#List.toMap (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L3377)

1 | List(1).toMap((x) -> x)
    ^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"list-to-map-three-args"########,
            r########"List(1).toMap((x) -> x, (x) -> x, 1)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected 2 function arguments but got 3.

3377 | external function toMap<Key, Value>(
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#List.toMap (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L3377)

1 | List(1).toMap((x) -> x, (x) -> x, 1)
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"list-to-map-wrong-first"########,
            r########"List().toMap(1, (x) -> x)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected value of type `Function1`, but got type `Int`.
Value: 1

3378 | keyExtractor: (Element) -> Key,
                     ^^^^^^^^^^^^^^^^
at pkl.base#List.toMap (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L3378)

1 | List().toMap(1, (x) -> x)
    ^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"list-to-map-wrong-second"########,
            r########"List().toMap((x) -> x, 1)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected value of type `Function1`, but got type `Int`.
Value: 1

3379 | valueExtractor: (Element) -> Value,
                       ^^^^^^^^^^^^^^^^^^
at pkl.base#List.toMap (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L3379)

1 | List().toMap((x) -> x, 1)
    ^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"list-to-map-wrong-callback-arity"########,
            r########"List(1).toMap((a,b) -> a, (x) -> x)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected value of type `Function1`, but got type `Function2`.
Value: new Function2 {}

3378 | keyExtractor: (Element) -> Key,
                     ^^^^^^^^^^^^^^^^
at pkl.base#List.toMap (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L3378)

1 | List(1).toMap((a,b) -> a, (x) -> x)
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"set-to-map-empty"########,
            r########"Set().toMap((x) -> x, (x) -> x)"########,
            r########"value"########,
            r########"Map()"########,
        ),
        (
            r########"set-to-map-basic"########,
            r########"Set(3,1,2).toMap((x) -> x, (x) -> x * 10)"########,
            r########"value"########,
            r########"Map(3, 30, 1, 10, 2, 20)"########,
        ),
        (
            r########"set-to-map-duplicate"########,
            r########"Set(1,2,3).toMap((x) -> x % 2, (x) -> x * 10)"########,
            r########"value"########,
            r########"Map(1, 30, 0, 20)"########,
        ),
        (
            r########"set-to-map-duplicate-keys"########,
            r########"Set(1,2,3).toMap((x) -> x % 2, (x) -> x * 10).keys"########,
            r########"value"########,
            r########"Set(1, 0)"########,
        ),
        (
            r########"set-to-map-null-key"########,
            r########"Set(1,2).toMap((x) -> null, (x) -> x)"########,
            r########"value"########,
            r########"Map(null, 2)"########,
        ),
        (
            r########"set-to-map-key-throw"########,
            r########"Set(1,2).toMap((x) -> throw("set-key"), (x) -> throw("set-value"))"########,
            r########"error"########,
            r########"–– Pkl Error ––
set-key

1 | Set(1,2).toMap((x) -> throw("set-key"), (x) -> throw("set-value"))
                          ^^^^^^^^^^^^^^^^
at repl:text.<function#1> (repl:text)

1 | Set(1,2).toMap((x) -> throw("set-key"), (x) -> throw("set-value"))
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"set-to-map-wrong-first"########,
            r########"Set().toMap(1, (x) -> x)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected value of type `Function1`, but got type `Int`.
Value: 1

3539 | keyExtractor: (Element) -> Key,
                     ^^^^^^^^^^^^^^^^
at pkl.base#Set.toMap (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L3539)

1 | Set().toMap(1, (x) -> x)
    ^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"set-to-map-wrong-second"########,
            r########"Set().toMap((x) -> x, 1)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected value of type `Function1`, but got type `Int`.
Value: 1

3540 | valueExtractor: (Element) -> Value,
                       ^^^^^^^^^^^^^^^^^^
at pkl.base#Set.toMap (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L3540)

1 | Set().toMap((x) -> x, 1)
    ^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"set-to-map-wrong-callback-arity"########,
            r########"Set(1).toMap((x) -> x, (a,b) -> a)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected value of type `Function1`, but got type `Function2`.
Value: new Function2 {}

3540 | valueExtractor: (Element) -> Value,
                       ^^^^^^^^^^^^^^^^^^
at pkl.base#Set.toMap (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L3540)

1 | Set(1).toMap((x) -> x, (a,b) -> a)
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"list-empty-listing"########,
            r########"List().toListing()"########,
            r########"value"########,
            r########"new Listing {}"########,
        ),
        (
            r########"list-basic-listing"########,
            r########"List(1,2).toListing()"########,
            r########"value"########,
            r########"new Listing { 1; 2 }"########,
        ),
        (
            r########"list-listing-length"########,
            r########"List(1,2).toListing().length"########,
            r########"value"########,
            r########"2"########,
        ),
        (
            r########"list-listing-index"########,
            r########"List(1,2).toListing()[1]"########,
            r########"value"########,
            r########"2"########,
        ),
        (
            r########"list-empty-dynamic"########,
            r########"List().toDynamic()"########,
            r########"value"########,
            r########"new Dynamic {}"########,
        ),
        (
            r########"list-basic-dynamic"########,
            r########"List(1,2).toDynamic().toList()"########,
            r########"value"########,
            r########"List(1, 2)"########,
        ),
        (
            r########"list-dynamic-length"########,
            r########"List(1,2).toDynamic().length()"########,
            r########"value"########,
            r########"2"########,
        ),
        (
            r########"list-listing-arity"########,
            r########"List(1).toListing(1)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected 0 function arguments but got 1.

3382 | external function toListing(): Listing<Element>
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#List.toListing (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L3382)

1 | List(1).toListing(1)
    ^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"list-dynamic-arity"########,
            r########"List(1).toDynamic(1)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected 0 function arguments but got 1.

3384 | external function toDynamic(): Dynamic
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#List.toDynamic (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L3384)

1 | List(1).toDynamic(1)
    ^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"set-empty-listing"########,
            r########"Set().toListing()"########,
            r########"value"########,
            r########"new Listing {}"########,
        ),
        (
            r########"set-basic-listing"########,
            r########"Set(2,1).toListing()"########,
            r########"value"########,
            r########"new Listing { 2; 1 }"########,
        ),
        (
            r########"set-listing-length"########,
            r########"Set(2,1).toListing().length"########,
            r########"value"########,
            r########"2"########,
        ),
        (
            r########"set-empty-dynamic"########,
            r########"Set().toDynamic()"########,
            r########"value"########,
            r########"new Dynamic {}"########,
        ),
        (
            r########"set-basic-dynamic"########,
            r########"Set(2,1).toDynamic().toList()"########,
            r########"value"########,
            r########"List(2, 1)"########,
        ),
        (
            r########"set-dynamic-length"########,
            r########"Set(2,1).toDynamic().length()"########,
            r########"value"########,
            r########"2"########,
        ),
        (
            r########"set-listing-arity"########,
            r########"Set(1).toListing(1)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected 0 function arguments but got 1.

3543 | external function toListing(): Listing<Element>
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#Set.toListing (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L3543)

1 | Set(1).toListing(1)
    ^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"set-dynamic-arity"########,
            r########"Set(1).toDynamic(1)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected 0 function arguments but got 1.

3545 | external function toDynamic(): Dynamic
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#Set.toDynamic (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L3545)

1 | Set(1).toDynamic(1)
    ^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"map-empty-mapping"########,
            r########"Map().toMapping()"########,
            r########"value"########,
            r########"new Mapping {}"########,
        ),
        (
            r########"map-basic-mapping"########,
            r########"Map("a",1,"b",2).toMapping().toMap()"########,
            r########"value"########,
            r########"Map("a", 1, "b", 2)"########,
        ),
        (
            r########"map-mapping-length"########,
            r########"Map("a",1).toMapping().length"########,
            r########"value"########,
            r########"1"########,
        ),
        (
            r########"map-mapping-index"########,
            r########"Map("a",1).toMapping()["a"]"########,
            r########"value"########,
            r########"1"########,
        ),
        (
            r########"map-empty-dynamic"########,
            r########"Map().toDynamic()"########,
            r########"value"########,
            r########"new Dynamic {}"########,
        ),
        (
            r########"map-basic-dynamic"########,
            r########"Map("a",1,"b",2).toDynamic().toMap()"########,
            r########"value"########,
            r########"Map("a", 1, "b", 2)"########,
        ),
        (
            r########"map-dynamic-length"########,
            r########"Map("a",1).toDynamic().length()"########,
            r########"value"########,
            r########"0"########,
        ),
        (
            r########"map-nonstr-dynamic"########,
            r########"Map(1,"one",true,"yes").toDynamic().toMap()"########,
            r########"value"########,
            r########"Map(1, "one", true, "yes")"########,
        ),
        (
            r########"map-mapping-arity"########,
            r########"Map().toMapping(1)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected 0 function arguments but got 1.

3679 | external function toMapping(): Mapping<Key, Value>
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#Map.toMapping (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L3679)

1 | Map().toMapping(1)
    ^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"map-dynamic-arity"########,
            r########"Map().toDynamic(1)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected 0 function arguments but got 1.

3655 | external function toDynamic(): Dynamic
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#Map.toDynamic (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L3655)

1 | Map().toDynamic(1)
    ^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"typed-empty-dynamic"########,
            r########"new Holder {}.toDynamic().toMap()"########,
            r########"value"########,
            r########"Map("value", 1)"########,
        ),
        (
            r########"typed-base-dynamic"########,
            r########"typedBase.toDynamic().toMap()"########,
            r########"value"########,
            r########"Map("name", "pigeon", "age", 3)"########,
        ),
        (
            r########"typed-dynamic-length"########,
            r########"typedBase.toDynamic().length()"########,
            r########"value"########,
            r########"0"########,
        ),
        (
            r########"typed-dynamic-amend"########,
            r########"(typedBase.toDynamic()) { name = "new" }.toMap()"########,
            r########"value"########,
            r########"Map("name", "new", "age", 3)"########,
        ),
        (
            r########"typed-dynamic-arity"########,
            r########"typedBase.toDynamic(1)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected 0 function arguments but got 1.

1994 | external function toDynamic(): Dynamic
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#Typed.toDynamic (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L1994)

1 | typedBase.toDynamic(1)
    ^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"typed-dynamic-shallow"########,
            r########"new Holder { value = throw("typed-dynamic-lazy") }.toDynamic().length()"########,
            r########"value"########,
            r########"0"########,
        ),
        (
            r########"typed-dynamic-forced"########,
            r########"new Holder { value = throw("typed-dynamic-force") }.toDynamic().toMap().length"########,
            r########"error"########,
            r########"–– Pkl Error ––
typed-dynamic-force

1 | new Holder { value = throw("typed-dynamic-force") }.toDynamic().toMap().length
                         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at repl:text.value (repl:text)

1 | new Holder { value = throw("typed-dynamic-force") }.toDynamic().toMap().length
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"dynamic-typed-basic"########,
            r########"new Dynamic { name = "bird"; age = 5 }.toTyped(Animal).toMap()"########,
            r########"value"########,
            r########"Map("name", "bird", "age", 5)"########,
        ),
        (
            r########"dynamic-typed-defaults"########,
            r########"new Dynamic { name = "bird" }.toTyped(Animal).toMap()"########,
            r########"value"########,
            r########"Map("name", "bird", "age", 3)"########,
        ),
        (
            r########"dynamic-typed-missing"########,
            r########"new Dynamic {}.toTyped(Animal).toMap()"########,
            r########"value"########,
            r########"Map("name", "default", "age", 3)"########,
        ),
        (
            r########"dynamic-typed-null"########,
            r########"new Dynamic { name = null }.toTyped(Animal).name"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected value of type `String`, but got `null`.

9 | name: String = "default"
          ^^^^^^
at collection-conversions#Animal.name (file:///stage62/collection-conversions.pkl)

1 | new Dynamic { name = null }.toTyped(Animal).name
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"dynamic-typed-null-present"########,
            r########"new Dynamic { name = null }.toTyped(NullableAnimal).name"########,
            r########"value"########,
            r########"null"########,
        ),
        (
            r########"dynamic-typed-extra"########,
            r########"new Dynamic { name = "bird"; extra = 9 }.toTyped(Animal).toMap()"########,
            r########"value"########,
            r########"Map("name", "bird", "age", 3)"########,
        ),
        (
            r########"dynamic-typed-lazy"########,
            r########"new Dynamic { name = throw("dynamic-typed-lazy") }.toTyped(Animal).getClass().simpleName"########,
            r########"value"########,
            r########"Animal"########,
        ),
        (
            r########"dynamic-typed-force"########,
            r########"new Dynamic { name = throw("dynamic-typed-force") }.toTyped(Animal).name"########,
            r########"error"########,
            r########"–– Pkl Error ––
dynamic-typed-force

1 | new Dynamic { name = throw("dynamic-typed-force") }.toTyped(Animal).name
                         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at repl:text.name (repl:text)

1 | new Dynamic { name = throw("dynamic-typed-force") }.toTyped(Animal).name
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"dynamic-typed-wrong-value"########,
            r########"new Dynamic { age = "old" }.toTyped(Animal).age"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected value of type `Int`, but got type `String`.
Value: "old"

10 | age: Int = 3
          ^^^
at collection-conversions#Animal.age (file:///stage62/collection-conversions.pkl)

1 | new Dynamic { age = "old" }.toTyped(Animal).age
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"dynamic-typed-non-typed"########,
            r########"new Dynamic {}.toTyped(Dynamic)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Class `Dynamic` is not a subtype of `Typed`.

1 | new Dynamic {}.toTyped(Dynamic)
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"dynamic-typed-abstract"########,
            r########"new Dynamic {}.toTyped(AbstractAnimal)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot instantiate abstract class `collection-conversions#AbstractAnimal`.

1 | new Dynamic {}.toTyped(AbstractAnimal)
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)

Abstract classes cannot be instantiated.
Instead, instantiate a concrete subclass.
"########,
        ),
        (
            r########"dynamic-typed-wrong-argument"########,
            r########"new Dynamic {}.toTyped(1)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected value of type `Class`, but got type `Int`.
Value: 1

2060 | external function toTyped<Type>(clazz: Class<Type>): Type(this is Typed)
                                              ^^^^^^^^^^^
at pkl.base#Dynamic.toTyped (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L2060)

1 | new Dynamic {}.toTyped(1)
    ^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"dynamic-typed-arity"########,
            r########"new Dynamic {}.toTyped()"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected 1 function arguments but got 0.

2060 | external function toTyped<Type>(clazz: Class<Type>): Type(this is Typed)
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#Dynamic.toTyped (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L2060)

1 | new Dynamic {}.toTyped()
    ^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"map-typed-basic"########,
            r########"Map("name","bird","age",5).toTyped(Animal).toMap()"########,
            r########"value"########,
            r########"Map("name", "bird", "age", 5)"########,
        ),
        (
            r########"map-typed-defaults"########,
            r########"Map("name","bird").toTyped(Animal).toMap()"########,
            r########"value"########,
            r########"Map("name", "bird", "age", 3)"########,
        ),
        (
            r########"map-typed-missing"########,
            r########"Map().toTyped(Animal).toMap()"########,
            r########"value"########,
            r########"Map("name", "default", "age", 3)"########,
        ),
        (
            r########"map-typed-null"########,
            r########"Map("name",null).toTyped(Animal).name"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected value of type `String`, but got `null`.

9 | name: String = "default"
          ^^^^^^
at collection-conversions#Animal.name (file:///stage62/collection-conversions.pkl)

1 | Map("name",null).toTyped(Animal).name
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"map-typed-null-present"########,
            r########"Map("name",null).toTyped(NullableAnimal).name"########,
            r########"value"########,
            r########"null"########,
        ),
        (
            r########"map-typed-extra"########,
            r########"Map("name","bird","extra",9).toTyped(Animal).toMap()"########,
            r########"value"########,
            r########"Map("name", "bird", "age", 3)"########,
        ),
        (
            r########"map-typed-wrong-value"########,
            r########"Map("age","old").toTyped(Animal).age"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected value of type `Int`, but got type `String`.
Value: "old"

10 | age: Int = 3
          ^^^
at collection-conversions#Animal.age (file:///stage62/collection-conversions.pkl)

1 | Map("age","old").toTyped(Animal).age
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"map-typed-nonstr-key"########,
            r########"Map(1,"one","name","bird").toTyped(Animal).name"########,
            r########"value"########,
            r########"bird"########,
        ),
        (
            r########"map-typed-non-typed"########,
            r########"Map().toTyped(Dynamic)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Class `Dynamic` is not a subtype of `Typed`.

1 | Map().toTyped(Dynamic)
    ^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"map-typed-abstract"########,
            r########"Map().toTyped(AbstractAnimal)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot instantiate abstract class `collection-conversions#AbstractAnimal`.

1 | Map().toTyped(AbstractAnimal)
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)

Abstract classes cannot be instantiated.
Instead, instantiate a concrete subclass.
"########,
        ),
        (
            r########"map-typed-wrong-argument"########,
            r########"Map().toTyped(1)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected value of type `Class`, but got type `Int`.
Value: 1

3676 | external function toTyped<Type>(clazz: Class<Type>): Type(this is Typed)
                                              ^^^^^^^^^^^
at pkl.base#Map.toTyped (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L3676)

1 | Map().toTyped(1)
    ^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"map-typed-arity"########,
            r########"Map().toTyped()"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected 1 function arguments but got 0.

3676 | external function toTyped<Type>(clazz: Class<Type>): Type(this is Typed)
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#Map.toTyped (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L3676)

1 | Map().toTyped()
    ^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"map-typed-nested-shallow"########,
            r########"Map("child", new Holder { value = throw("map-nested") }).toTyped(Carrier).getClass().simpleName"########,
            r########"value"########,
            r########"Carrier"########,
        ),
        (
            r########"map-typed-nested-forced"########,
            r########"Map("child", new Holder { value = throw("map-nested") }).toTyped(Carrier).child.value"########,
            r########"error"########,
            r########"–– Pkl Error ––
map-nested

1 | Map("child", new Holder { value = throw("map-nested") }).toTyped(Carrier).child.value
                                      ^^^^^^^^^^^^^^^^^^^
at repl:text.value (repl:text)

1 | Map("child", new Holder { value = throw("map-nested") }).toTyped(Carrier).child.value
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"set-add-empty"########,
            r########"Set().add(1)"########,
            r########"value"########,
            r########"Set(1)"########,
        ),
        (
            r########"set-add-new"########,
            r########"Set(1,2).add(3)"########,
            r########"value"########,
            r########"Set(1, 2, 3)"########,
        ),
        (
            r########"set-add-duplicate"########,
            r########"Set(1,2).add(1)"########,
            r########"value"########,
            r########"Set(1, 2)"########,
        ),
        (
            r########"set-add-duplicate-order"########,
            r########"Set(2,1).add(2).toList()"########,
            r########"value"########,
            r########"List(2, 1)"########,
        ),
        (
            r########"set-add-null"########,
            r########"Set(1).add(null)"########,
            r########"value"########,
            r########"Set(1, null)"########,
        ),
        (
            r########"set-add-double-null"########,
            r########"Set(null).add(null)"########,
            r########"value"########,
            r########"Set(null)"########,
        ),
        (
            r########"set-add-equality"########,
            r########"Set(1,2).add(2) == Set(2,1)"########,
            r########"value"########,
            r########"true"########,
        ),
        (
            r########"set-add-contains"########,
            r########"Set(1,2).add(2).contains(2)"########,
            r########"value"########,
            r########"true"########,
        ),
        (
            r########"set-add-object"########,
            r########"Set(new Dynamic { x = 1 }).add(new Dynamic { x = 1 }).length"########,
            r########"value"########,
            r########"1"########,
        ),
        (
            r########"set-add-list"########,
            r########"Set(List(1)).add(List(1)).length"########,
            r########"value"########,
            r########"1"########,
        ),
        (
            r########"set-add-arity"########,
            r########"Set().add()"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected 1 function arguments but got 0.

3466 | external function add<Other>(element: Other): Set<Element | Other>
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#Set.add (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L3466)

1 | Set().add()
    ^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"set-add-extra-arg"########,
            r########"Set().add(1,2)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected 1 function arguments but got 2.

3466 | external function add<Other>(element: Other): Set<Element | Other>
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#Set.add (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L3466)

1 | Set().add(1,2)
    ^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"set-add-method-value"########,
            r########"Set().add"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot find property `add` in object of type `Set`.

1 | Set().add
    ^^^^^^^^^
at  (repl:text)

Available properties:
first
firstOrNull
isEmpty
isNotEmpty
last
lastOrNull
length
max
maxOrNull
min
minOrNull
rest
restOrNull
single
singleOrNull
"########,
        ),
        (
            r########"bytes-for-empty"########,
            r########"new Listing { for (b in Bytes()) { b } }"########,
            r########"value"########,
            r########"new Listing {}"########,
        ),
        (
            r########"bytes-spread-empty"########,
            r########"new Listing { ...Bytes() }"########,
            r########"value"########,
            r########"new Listing {}"########,
        ),
        (
            r########"bytes-for-low"########,
            r########"new Listing { for (b in Bytes(0,1,127)) { b } }"########,
            r########"value"########,
            r########"new Listing { 0; 1; 127 }"########,
        ),
        (
            r########"bytes-spread-low"########,
            r########"new Listing { ...Bytes(0,1,127) }"########,
            r########"value"########,
            r########"new Listing { 0; 1; 127 }"########,
        ),
        (
            r########"bytes-for-high"########,
            r########"new Listing { for (b in Bytes(128,255)) { b } }"########,
            r########"value"########,
            r########"new Listing { -128; -1 }"########,
        ),
        (
            r########"bytes-spread-high"########,
            r########"new Listing { ...Bytes(128,255) }"########,
            r########"value"########,
            r########"new Listing { 128; 255 }"########,
        ),
        (
            r########"bytes-for-boundaries"########,
            r########"new Listing { for (b in Bytes(0,127,128,254,255)) { b } }"########,
            r########"value"########,
            r########"new Listing { 0; 127; -128; -2; -1 }"########,
        ),
        (
            r########"bytes-spread-boundaries"########,
            r########"new Listing { ...Bytes(0,127,128,254,255) }"########,
            r########"value"########,
            r########"new Listing { 0; 127; 128; 254; 255 }"########,
        ),
        (
            r########"bytes-for-pairs"########,
            r########"new Listing { for (i,b in Bytes(128,255)) { Pair(i,b) } }"########,
            r########"value"########,
            r########"new Listing { Pair(0, -128); Pair(1, -1) }"########,
        ),
        (
            r########"bytes-for-typed-int"########,
            r########"new Listing { for (b: Int in Bytes(128,255)) { b } }"########,
            r########"value"########,
            r########"new Listing { -128; -1 }"########,
        ),
        (
            r########"bytes-for-typed-uint8"########,
            r########"new Listing { for (b: UInt8 in Bytes(128,255)) { b } }"########,
            r########"error"########,
            r########"–– Pkl Error ––
Type constraint `isBetween(0, 255)` violated.
Value: -128

    isBetween(0, 255)
    │
    false

1109 | typealias UInt8 = Int(isBetween(0, 255))
                             ^^^^^^^^^^^^^^^^^
at  (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L1109)
"########,
        ),
        (
            r########"bytes-for-typed-wrong"########,
            r########"new Listing { for (b: String in Bytes(128,255)) { b } }"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected value of type `String`, but got type `Int`.
Value: -128

1 | new Listing { for (b: String in Bytes(128,255)) { b } }
                          ^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"bytes-spread-dynamic"########,
            r########"new Dynamic { ...Bytes(128,255) }.toList()"########,
            r########"value"########,
            r########"List(128, 255)"########,
        ),
        (
            r########"bytes-for-dynamic"########,
            r########"new Dynamic { for (b in Bytes(128,255)) { b } }.toList()"########,
            r########"value"########,
            r########"List(-128, -1)"########,
        ),
        (
            r########"bytes-spread-listing-typed"########,
            r########"(new Listing<UInt8> { ...Bytes(128,255) }).toList()"########,
            r########"value"########,
            r########"List(128, 255)"########,
        ),
        (
            r########"bytes-for-listing-typed"########,
            r########"(new Listing<UInt8> { for (b in Bytes(128,255)) { b } }).toList()"########,
            r########"error"########,
            r########"–– Pkl Error ––
Type constraint `isBetween(0, 255)` violated.
Value: -128

    isBetween(0, 255)
    │
    false

1109 | typealias UInt8 = Int(isBetween(0, 255))
                             ^^^^^^^^^^^^^^^^^
at  (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L1109)

1 | (new Listing<UInt8> { for (b in Bytes(128,255)) { b } }).toList()
                                                      ^
at repl:text[#1] (repl:text)

1 | (new Listing<UInt8> { for (b in Bytes(128,255)) { b } }).toList()
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"bytes-spread-mapping-empty"########,
            r########"new Mapping { ...Bytes() }"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot spread value of type `Bytes` into object of type `Mapping`.
Value: Bytes()

1 | new Mapping { ...Bytes() }
                  ^^^^^^^^^^
at  (repl:text)

`Bytes` can only be spread into objects of type `Dynamic` and `Listing`.
"########,
        ),
        (
            r########"bytes-for-mapping-empty"########,
            r########"new Mapping { for (b in Bytes()) { b } }"########,
            r########"value"########,
            r########"new Mapping {}"########,
        ),
    ];
    assert_eq!(rows.len(), 236);
    for (id, expression, kind, expected) in rows {
        probe
            .evaluate_module_source(
                "file:///stage62/collection-conversions.pkl",
                Some(include_str!(
                    "../../../tests/fixtures/m2-collection-conversions/module.pkl"
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
