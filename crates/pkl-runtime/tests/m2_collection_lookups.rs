//! Complete independent Java 0.32.0 string observations, capture-03/repeat-0.
//! Authority: m2-collection-lookups/oracle-evidence.tar.gz.
#[path = "support/module_keys.rs"]
mod module_keys;
use pkl_runtime::probe::SourceProbe;
#[test]
fn complete_java_collection_lookups() {
    let mut probe = SourceProbe::new_module_evaluator("compact", false, false, false).unwrap();
    let mut failures = Vec::new();
    let rows = [
(r########"dyn-has-empty"########,r########"new Dynamic {}.hasProperty("x")"########,r########"value"########,r########"false"########.encode_utf16().collect::<Vec<_>>()),
(r########"dyn-has-own"########,r########"dynamicProps.hasProperty("name")"########,r########"value"########,r########"true"########.encode_utf16().collect::<Vec<_>>()),
(r########"dyn-has-null"########,r########"dynamicProps.hasProperty("nullable")"########,r########"value"########,r########"true"########.encode_utf16().collect::<Vec<_>>()),
(r########"dyn-has-absent"########,r########"dynamicProps.hasProperty("missing")"########,r########"value"########,r########"false"########.encode_utf16().collect::<Vec<_>>()),
(r########"dyn-has-entry"########,r########"dynamicProps.hasProperty("entry")"########,r########"value"########,r########"false"########.encode_utf16().collect::<Vec<_>>()),
(r########"dyn-has-numeric-entry"########,r########"dynamicProps.hasProperty("1")"########,r########"value"########,r########"false"########.encode_utf16().collect::<Vec<_>>()),
(r########"dyn-has-inherited"########,r########"dynamicInherited.hasProperty("nullable")"########,r########"value"########,r########"true"########.encode_utf16().collect::<Vec<_>>()),
(r########"dyn-has-overridden"########,r########"dynamicInherited.hasProperty("name")"########,r########"value"########,r########"true"########.encode_utf16().collect::<Vec<_>>()),
(r########"dyn-has-added"########,r########"dynamicInherited.hasProperty("extra")"########,r########"value"########,r########"true"########.encode_utf16().collect::<Vec<_>>()),
(r########"dyn-has-hidden-typed"########,r########"new HiddenChild {}.toDynamic().hasProperty("secret")"########,r########"value"########,r########"false"########.encode_utf16().collect::<Vec<_>>()),
(r########"dyn-has-visible-typed"########,r########"new HiddenChild {}.toDynamic().hasProperty("visible")"########,r########"value"########,r########"true"########.encode_utf16().collect::<Vec<_>>()),
(r########"dyn-has-local-typed"########,r########"new LocalThing {}.toDynamic().hasProperty("scratch")"########,r########"value"########,r########"false"########.encode_utf16().collect::<Vec<_>>()),
(r########"dyn-has-external-method-name"########,r########"dynamicProps.hasProperty("length")"########,r########"value"########,r########"false"########.encode_utf16().collect::<Vec<_>>()),
(r########"dyn-has-hidden-default"########,r########"new Dynamic {}.hasProperty("default")"########,r########"value"########,r########"true"########.encode_utf16().collect::<Vec<_>>()),
(r########"dyn-has-no-force"########,r########"new Dynamic { x = throw("has-no-force") }.hasProperty("x")"########,r########"value"########,r########"true"########.encode_utf16().collect::<Vec<_>>()),
(r########"dyn-has-wrong-int"########,r########"dynamicProps.hasProperty(1)"########,r########"error"########,r########"–– Pkl Error ––
Expected value of type `String`, but got type `Int`.
Value: 1

2021 | external function hasProperty(name: String): Boolean
                                           ^^^^^^
at pkl.base#Dynamic.hasProperty (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L2021)

1 | dynamicProps.hasProperty(1)
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########.encode_utf16().collect::<Vec<_>>()),
(r########"dyn-has-wrong-null"########,r########"dynamicProps.hasProperty(null)"########,r########"error"########,r########"–– Pkl Error ––
Expected value of type `String`, but got `null`.

2021 | external function hasProperty(name: String): Boolean
                                           ^^^^^^
at pkl.base#Dynamic.hasProperty (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L2021)

1 | dynamicProps.hasProperty(null)
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########.encode_utf16().collect::<Vec<_>>()),
(r########"dyn-has-no-args"########,r########"dynamicProps.hasProperty()"########,r########"error"########,r########"–– Pkl Error ––
Expected 1 function arguments but got 0.

2021 | external function hasProperty(name: String): Boolean
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#Dynamic.hasProperty (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L2021)

1 | dynamicProps.hasProperty()
    ^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########.encode_utf16().collect::<Vec<_>>()),
(r########"dyn-has-two-args"########,r########"dynamicProps.hasProperty("name", "age")"########,r########"error"########,r########"–– Pkl Error ––
Expected 1 function arguments but got 2.

2021 | external function hasProperty(name: String): Boolean
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#Dynamic.hasProperty (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L2021)

1 | dynamicProps.hasProperty("name", "age")
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########.encode_utf16().collect::<Vec<_>>()),
(r########"dyn-has-as-property"########,r########"dynamicProps.hasProperty"########,r########"error"########,r########"–– Pkl Error ––
Cannot find property `hasProperty` in object of type `Dynamic`.

1 | dynamicProps.hasProperty
    ^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)

Did you mean any of the following?
getProperty(name)
"########.encode_utf16().collect::<Vec<_>>()),
(r########"dyn-has-high-surrogate"########,r########"Map("\u{d800}",7).toDynamic().hasProperty("\u{d800}")"########,r########"value"########,r########"true"########.encode_utf16().collect::<Vec<_>>()),
(r########"dyn-has-replacement-distinct"########,r########"Map("\u{d800}",7).toDynamic().hasProperty("\u{fffd}")"########,r########"value"########,r########"false"########.encode_utf16().collect::<Vec<_>>()),
(r########"dyn-cache-module-before"########,r########"entryDynamic.hasProperty("cached")"########,r########"value"########,r########"false"########.encode_utf16().collect::<Vec<_>>()),
(r########"dyn-cache-module-read-entry"########,r########"entryDynamic["cached"]"########,r########"value"########,r########"13"########.encode_utf16().collect::<Vec<_>>()),
(r########"dyn-cache-module-after"########,r########"entryDynamic.hasProperty("cached")"########,r########"value"########,r########"true"########.encode_utf16().collect::<Vec<_>>()),
(r########"dyn-cache-module-ornull-after"########,r########"entryDynamic.getPropertyOrNull("cached")"########,r########"value"########,r########"null"########.encode_utf16().collect::<Vec<_>>()),
(r########"dyn-cache-module-get-after"########,r########"entryDynamic.getProperty("cached")"########,r########"error"########,r########"–– Pkl Error ––
Cannot find property `cached` in object of type `Dynamic`.

1 | entryDynamic.getProperty("cached")
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)

Available properties:
default
"########.encode_utf16().collect::<Vec<_>>()),
(r########"dyn-cache-let-before"########,r########"let (x = new Dynamic { ["cached"] = module.marker.length }) x.hasProperty("cached")"########,r########"value"########,r########"false"########.encode_utf16().collect::<Vec<_>>()),
(r########"dyn-cache-let-after-read"########,r########"let (x = new Dynamic { ["cached"] = module.marker.length }) x["cached"] >= 0 && x.hasProperty("cached")"########,r########"value"########,r########"true"########.encode_utf16().collect::<Vec<_>>()),
(r########"dyn-cache-let-after-map"########,r########"let (x = new Dynamic { ["cached"] = module.marker.length }) x.toMap().length >= 0 && x.hasProperty("cached")"########,r########"value"########,r########"true"########.encode_utf16().collect::<Vec<_>>()),
(r########"dyn-cache-let-still-no-property"########,r########"let (x = new Dynamic { ["cached"] = module.marker.length }) x["cached"] >= 0 && x.getPropertyOrNull("cached") == null"########,r########"value"########,r########"true"########.encode_utf16().collect::<Vec<_>>()),
(r########"dyn-cache-let-get-property"########,r########"let (x = new Dynamic { ["cached"] = module.marker.length }) x["cached"] >= 0 && x.getProperty("cached") == 13"########,r########"error"########,r########"–– Pkl Error ––
Cannot find property `cached` in object of type `Dynamic`.

1 | let (x = new Dynamic { ["cached"] = module.marker.length }) x["cached"] >= 0 && x.getProperty("cached") == 13
                                                                                    ^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)

Available properties:
default

1 | let (x = new Dynamic { ["cached"] = module.marker.length }) x["cached"] >= 0 && x.getProperty("cached") == 13
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at repl:text.<let expr> (repl:text)
"########.encode_utf16().collect::<Vec<_>>()),
(r########"dyn-get-own"########,r########"dynamicProps.getProperty("name")"########,r########"value"########,r########"bird"########.encode_utf16().collect::<Vec<_>>()),
(r########"dyn-get-null"########,r########"dynamicProps.getProperty("nullable")"########,r########"value"########,r########"null"########.encode_utf16().collect::<Vec<_>>()),
(r########"dyn-get-absent"########,r########"dynamicProps.getProperty("missing")"########,r########"error"########,r########"–– Pkl Error ––
Cannot find property `missing` in object of type `Dynamic`.

1 | dynamicProps.getProperty("missing")
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)

Available properties:
default
name
nullable
"########.encode_utf16().collect::<Vec<_>>()),
(r########"dyn-get-entry-as-property"########,r########"dynamicProps.getProperty("entry")"########,r########"error"########,r########"–– Pkl Error ––
Cannot find property `entry` in object of type `Dynamic`.

1 | dynamicProps.getProperty("entry")
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)

Available properties:
default
name
nullable
"########.encode_utf16().collect::<Vec<_>>()),
(r########"dyn-get-inherited"########,r########"dynamicInherited.getProperty("nullable")"########,r########"value"########,r########"null"########.encode_utf16().collect::<Vec<_>>()),
(r########"dyn-get-overridden"########,r########"dynamicInherited.getProperty("name")"########,r########"value"########,r########"owl"########.encode_utf16().collect::<Vec<_>>()),
(r########"dyn-get-added"########,r########"dynamicInherited.getProperty("extra")"########,r########"value"########,r########"true"########.encode_utf16().collect::<Vec<_>>()),
(r########"dyn-get-hidden-typed"########,r########"new HiddenChild {}.toDynamic().getProperty("secret")"########,r########"error"########,r########"–– Pkl Error ––
Cannot find property `secret` in object of type `Dynamic`.

1 | new HiddenChild {}.toDynamic().getProperty("secret")
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)

Available properties:
default
visible
"########.encode_utf16().collect::<Vec<_>>()),
(r########"dyn-get-visible-typed"########,r########"new HiddenChild {}.toDynamic().getProperty("visible")"########,r########"value"########,r########"5"########.encode_utf16().collect::<Vec<_>>()),
(r########"dyn-get-local-typed"########,r########"new LocalThing {}.toDynamic().getProperty("scratch")"########,r########"error"########,r########"–– Pkl Error ––
Cannot find property `scratch` in object of type `Dynamic`.

1 | new LocalThing {}.toDynamic().getProperty("scratch")
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)

Available properties:
default
visible
"########.encode_utf16().collect::<Vec<_>>()),
(r########"dyn-get-external-method-name"########,r########"dynamicProps.getProperty("length")"########,r########"error"########,r########"–– Pkl Error ––
Cannot find property `length` in object of type `Dynamic`.

1 | dynamicProps.getProperty("length")
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)

Available properties:
default
name
nullable
"########.encode_utf16().collect::<Vec<_>>()),
(r########"dyn-get-lazy-error"########,r########"new Dynamic { x = throw("dyn-get-lazy") }.getProperty("x")"########,r########"error"########,r########"–– Pkl Error ––
dyn-get-lazy

1 | new Dynamic { x = throw("dyn-get-lazy") }.getProperty("x")
                      ^^^^^^^^^^^^^^^^^^^^^
at repl:text.x (repl:text)

1 | new Dynamic { x = throw("dyn-get-lazy") }.getProperty("x")
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########.encode_utf16().collect::<Vec<_>>()),
(r########"dyn-get-wrong-int"########,r########"dynamicProps.getProperty(1)"########,r########"error"########,r########"–– Pkl Error ––
Expected value of type `String`, but got type `Int`.
Value: 1

2026 | external function getProperty(name: String): unknown
                                           ^^^^^^
at pkl.base#Dynamic.getProperty (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L2026)

1 | dynamicProps.getProperty(1)
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########.encode_utf16().collect::<Vec<_>>()),
(r########"dyn-get-wrong-null"########,r########"dynamicProps.getProperty(null)"########,r########"error"########,r########"–– Pkl Error ––
Expected value of type `String`, but got `null`.

2026 | external function getProperty(name: String): unknown
                                           ^^^^^^
at pkl.base#Dynamic.getProperty (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L2026)

1 | dynamicProps.getProperty(null)
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########.encode_utf16().collect::<Vec<_>>()),
(r########"dyn-get-no-args"########,r########"dynamicProps.getProperty()"########,r########"error"########,r########"–– Pkl Error ––
Expected 1 function arguments but got 0.

2026 | external function getProperty(name: String): unknown
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#Dynamic.getProperty (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L2026)

1 | dynamicProps.getProperty()
    ^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########.encode_utf16().collect::<Vec<_>>()),
(r########"dyn-get-two-args"########,r########"dynamicProps.getProperty("name", "age")"########,r########"error"########,r########"–– Pkl Error ––
Expected 1 function arguments but got 2.

2026 | external function getProperty(name: String): unknown
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#Dynamic.getProperty (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L2026)

1 | dynamicProps.getProperty("name", "age")
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########.encode_utf16().collect::<Vec<_>>()),
(r########"dyn-get-as-property"########,r########"dynamicProps.getProperty"########,r########"error"########,r########"–– Pkl Error ––
Cannot find property `getProperty` in object of type `Dynamic`.

1 | dynamicProps.getProperty
    ^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)

Did you mean any of the following?
getPropertyOrNull(name)
hasProperty(name)
"########.encode_utf16().collect::<Vec<_>>()),
(r########"dyn-get-high-surrogate"########,r########"Map("\u{d800}",7).toDynamic().getProperty("\u{d800}")"########,r########"value"########,r########"7"########.encode_utf16().collect::<Vec<_>>()),
(r########"dyn-get-high-vs-replacement"########,r########"Map("\u{d800}",7).toDynamic().getProperty("\u{fffd}")"########,r########"error"########,vec![8211,8211,32,80,107,108,32,69,114,114,111,114,32,8211,8211,10,67,97,110,110,111,116,32,102,105,110,100,32,112,114,111,112,101,114,116,121,32,96,65533,96,32,105,110,32,111,98,106,101,99,116,32,111,102,32,116,121,112,101,32,96,68,121,110,97,109,105,99,96,46,10,10,49,32,124,32,77,97,112,40,34,92,117,123,100,56,48,48,125,34,44,55,41,46,116,111,68,121,110,97,109,105,99,40,41,46,103,101,116,80,114,111,112,101,114,116,121,40,34,92,117,123,102,102,102,100,125,34,41,10,32,32,32,32,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,10,97,116,32,32,40,114,101,112,108,58,116,101,120,116,41,10,10,65,118,97,105,108,97,98,108,101,32,112,114,111,112,101,114,116,105,101,115,58,10,100,101,102,97,117,108,116,10,55296,10]),
(r########"dyn-ornull-own"########,r########"dynamicProps.getPropertyOrNull("name")"########,r########"value"########,r########"bird"########.encode_utf16().collect::<Vec<_>>()),
(r########"dyn-ornull-null"########,r########"dynamicProps.getPropertyOrNull("nullable")"########,r########"value"########,r########"null"########.encode_utf16().collect::<Vec<_>>()),
(r########"dyn-ornull-absent"########,r########"dynamicProps.getPropertyOrNull("missing")"########,r########"value"########,r########"null"########.encode_utf16().collect::<Vec<_>>()),
(r########"dyn-ornull-entry-as-property"########,r########"dynamicProps.getPropertyOrNull("entry")"########,r########"value"########,r########"null"########.encode_utf16().collect::<Vec<_>>()),
(r########"dyn-ornull-inherited"########,r########"dynamicInherited.getPropertyOrNull("name")"########,r########"value"########,r########"owl"########.encode_utf16().collect::<Vec<_>>()),
(r########"dyn-ornull-hidden-typed"########,r########"new HiddenChild {}.toDynamic().getPropertyOrNull("secret")"########,r########"value"########,r########"null"########.encode_utf16().collect::<Vec<_>>()),
(r########"dyn-ornull-external-method-name"########,r########"dynamicProps.getPropertyOrNull("length")"########,r########"value"########,r########"null"########.encode_utf16().collect::<Vec<_>>()),
(r########"dyn-ornull-lazy-error"########,r########"new Dynamic { x = throw("dyn-ornull-lazy") }.getPropertyOrNull("x")"########,r########"error"########,r########"–– Pkl Error ––
dyn-ornull-lazy

1 | new Dynamic { x = throw("dyn-ornull-lazy") }.getPropertyOrNull("x")
                      ^^^^^^^^^^^^^^^^^^^^^^^^
at repl:text.x (repl:text)

1 | new Dynamic { x = throw("dyn-ornull-lazy") }.getPropertyOrNull("x")
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########.encode_utf16().collect::<Vec<_>>()),
(r########"dyn-ornull-wrong-int"########,r########"dynamicProps.getPropertyOrNull(1)"########,r########"error"########,r########"–– Pkl Error ––
Expected value of type `String`, but got type `Int`.
Value: 1

2031 | external function getPropertyOrNull(name: String): unknown
                                                 ^^^^^^
at pkl.base#Dynamic.getPropertyOrNull (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L2031)

1 | dynamicProps.getPropertyOrNull(1)
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########.encode_utf16().collect::<Vec<_>>()),
(r########"dyn-ornull-wrong-null"########,r########"dynamicProps.getPropertyOrNull(null)"########,r########"error"########,r########"–– Pkl Error ––
Expected value of type `String`, but got `null`.

2031 | external function getPropertyOrNull(name: String): unknown
                                                 ^^^^^^
at pkl.base#Dynamic.getPropertyOrNull (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L2031)

1 | dynamicProps.getPropertyOrNull(null)
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########.encode_utf16().collect::<Vec<_>>()),
(r########"dyn-ornull-no-args"########,r########"dynamicProps.getPropertyOrNull()"########,r########"error"########,r########"–– Pkl Error ––
Expected 1 function arguments but got 0.

2031 | external function getPropertyOrNull(name: String): unknown
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#Dynamic.getPropertyOrNull (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L2031)

1 | dynamicProps.getPropertyOrNull()
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########.encode_utf16().collect::<Vec<_>>()),
(r########"dyn-ornull-two-args"########,r########"dynamicProps.getPropertyOrNull("name", "age")"########,r########"error"########,r########"–– Pkl Error ––
Expected 1 function arguments but got 2.

2031 | external function getPropertyOrNull(name: String): unknown
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#Dynamic.getPropertyOrNull (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L2031)

1 | dynamicProps.getPropertyOrNull("name", "age")
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########.encode_utf16().collect::<Vec<_>>()),
(r########"dyn-ornull-as-property"########,r########"dynamicProps.getPropertyOrNull"########,r########"error"########,r########"–– Pkl Error ––
Cannot find property `getPropertyOrNull` in object of type `Dynamic`.

1 | dynamicProps.getPropertyOrNull
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)

Did you mean any of the following?
getProperty(name)
"########.encode_utf16().collect::<Vec<_>>()),
(r########"dyn-ornull-high-surrogate"########,r########"Map("\u{d800}",7).toDynamic().getPropertyOrNull("\u{d800}")"########,r########"value"########,r########"7"########.encode_utf16().collect::<Vec<_>>()),
(r########"list-ornull-empty-zero"########,r########"new Listing {}.getOrNull(0)"########,r########"value"########,r########"null"########.encode_utf16().collect::<Vec<_>>()),
(r########"list-ornull-zero"########,r########"listingBase.getOrNull(0)"########,r########"value"########,r########"1"########.encode_utf16().collect::<Vec<_>>()),
(r########"list-ornull-null"########,r########"listingBase.getOrNull(1)"########,r########"value"########,r########"null"########.encode_utf16().collect::<Vec<_>>()),
(r########"list-ornull-last"########,r########"listingBase.getOrNull(2)"########,r########"value"########,r########"3"########.encode_utf16().collect::<Vec<_>>()),
(r########"list-ornull-end"########,r########"listingBase.getOrNull(3)"########,r########"value"########,r########"null"########.encode_utf16().collect::<Vec<_>>()),
(r########"list-ornull-negative"########,r########"listingBase.getOrNull(-1)"########,r########"value"########,r########"null"########.encode_utf16().collect::<Vec<_>>()),
(r########"list-ornull-max-int"########,r########"listingBase.getOrNull(9223372036854775807)"########,r########"value"########,r########"null"########.encode_utf16().collect::<Vec<_>>()),
(r########"list-ornull-inherited"########,r########"(listingBase) { [0] = 9; 4 }.getOrNull(0)"########,r########"value"########,r########"9"########.encode_utf16().collect::<Vec<_>>()),
(r########"list-ornull-appended"########,r########"(listingBase) { 4 }.getOrNull(3)"########,r########"value"########,r########"4"########.encode_utf16().collect::<Vec<_>>()),
(r########"list-ornull-lazy-error"########,r########"new Listing { throw("list-ornull-lazy") }.getOrNull(0)"########,r########"error"########,r########"–– Pkl Error ––
list-ornull-lazy

1 | new Listing { throw("list-ornull-lazy") }.getOrNull(0)
                  ^^^^^^^^^^^^^^^^^^^^^^^^^
at repl:text[#1] (repl:text)

1 | new Listing { throw("list-ornull-lazy") }.getOrNull(0)
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########.encode_utf16().collect::<Vec<_>>()),
(r########"list-ornull-missing-no-force"########,r########"new Listing { throw("list-ornull-skip") }.getOrNull(9)"########,r########"value"########,r########"null"########.encode_utf16().collect::<Vec<_>>()),
(r########"list-ornull-typed"########,r########"(new Listing<Int> { 1; 2 }).getOrNull(1)"########,r########"value"########,r########"2"########.encode_utf16().collect::<Vec<_>>()),
(r########"list-ornull-typed-bad"########,r########"(new Listing<Int> { 1; "bad" }).getOrNull(1)"########,r########"error"########,r########"–– Pkl Error ––
Expected value of type `Int`, but got type `String`.
Value: "bad"

1 | (new Listing<Int> { 1; "bad" }).getOrNull(1)
                 ^^^
at  (repl:text)

1 | (new Listing<Int> { 1; "bad" }).getOrNull(1)
                           ^^^^^
at repl:text[#2] (repl:text)

1 | (new Listing<Int> { 1; "bad" }).getOrNull(1)
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########.encode_utf16().collect::<Vec<_>>()),
(r########"list-ornull-wrong-string"########,r########"listingBase.getOrNull("1")"########,r########"error"########,r########"–– Pkl Error ––
Expected value of type `Int`, but got type `String`.
Value: "1"

2100 | external function getOrNull(index: Int): Element?
                                          ^^^
at pkl.base#Listing.getOrNull (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L2100)

1 | listingBase.getOrNull("1")
    ^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########.encode_utf16().collect::<Vec<_>>()),
(r########"list-ornull-wrong-null"########,r########"listingBase.getOrNull(null)"########,r########"error"########,r########"–– Pkl Error ––
Expected value of type `Int`, but got `null`.

2100 | external function getOrNull(index: Int): Element?
                                          ^^^
at pkl.base#Listing.getOrNull (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L2100)

1 | listingBase.getOrNull(null)
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########.encode_utf16().collect::<Vec<_>>()),
(r########"list-ornull-no-args"########,r########"listingBase.getOrNull()"########,r########"error"########,r########"–– Pkl Error ––
Expected 1 function arguments but got 0.

2100 | external function getOrNull(index: Int): Element?
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#Listing.getOrNull (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L2100)

1 | listingBase.getOrNull()
    ^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########.encode_utf16().collect::<Vec<_>>()),
(r########"list-ornull-two-args"########,r########"listingBase.getOrNull(0, 1)"########,r########"error"########,r########"–– Pkl Error ––
Expected 1 function arguments but got 2.

2100 | external function getOrNull(index: Int): Element?
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#Listing.getOrNull (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L2100)

1 | listingBase.getOrNull(0, 1)
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########.encode_utf16().collect::<Vec<_>>()),
(r########"list-ornull-as-property"########,r########"listingBase.getOrNull"########,r########"error"########,r########"–– Pkl Error ––
Cannot find property `getOrNull` in object of type `Listing`.

1 | listingBase.getOrNull
    ^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)

Did you mean any of the following?
getOrDefault(index)
lastOrNull
"########.encode_utf16().collect::<Vec<_>>()),
(r########"list-default-empty"########,r########"new Listing {}.getOrDefault(0).getClass().simpleName"########,r########"value"########,r########"Dynamic"########.encode_utf16().collect::<Vec<_>>()),
(r########"list-default-present"########,r########"listingBase.getOrDefault(0)"########,r########"value"########,r########"1"########.encode_utf16().collect::<Vec<_>>()),
(r########"list-default-null"########,r########"listingBase.getOrDefault(1)"########,r########"value"########,r########"null"########.encode_utf16().collect::<Vec<_>>()),
(r########"list-default-missing"########,r########"listingBase.getOrDefault(3).getClass().simpleName"########,r########"value"########,r########"Dynamic"########.encode_utf16().collect::<Vec<_>>()),
(r########"list-default-negative"########,r########"listingBase.getOrDefault(-1).getClass().simpleName"########,r########"value"########,r########"Dynamic"########.encode_utf16().collect::<Vec<_>>()),
(r########"list-default-custom-positive"########,r########"new Listing { default = (i) -> i * 10; 1 }.getOrDefault(2)"########,r########"value"########,r########"20"########.encode_utf16().collect::<Vec<_>>()),
(r########"list-default-custom-negative"########,r########"new Listing { default = (i) -> i * 10; 1 }.getOrDefault(-1)"########,r########"value"########,r########"-10"########.encode_utf16().collect::<Vec<_>>()),
(r########"list-default-custom-present"########,r########"new Listing { default = (_) -> throw("should-skip"); 1 }.getOrDefault(0)"########,r########"value"########,r########"1"########.encode_utf16().collect::<Vec<_>>()),
(r########"list-default-custom-null"########,r########"new Listing { default = (_) -> throw("should-skip-null"); null }.getOrDefault(0)"########,r########"value"########,r########"null"########.encode_utf16().collect::<Vec<_>>()),
(r########"list-default-custom-throw"########,r########"new Listing { default = (_) -> throw("list-default-throw"); 1 }.getOrDefault(2)"########,r########"error"########,r########"–– Pkl Error ––
list-default-throw

1 | new Listing { default = (_) -> throw("list-default-throw"); 1 }.getOrDefault(2)
                                   ^^^^^^^^^^^^^^^^^^^^^^^^^^^
at repl:text.default.<function#1> (repl:text)

1 | new Listing { default = (_) -> throw("list-default-throw"); 1 }.getOrDefault(2)
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########.encode_utf16().collect::<Vec<_>>()),
(r########"list-default-present-throw"########,r########"new Listing { default = (_) -> 2; throw("list-present-throw") }.getOrDefault(0)"########,r########"error"########,r########"–– Pkl Error ––
list-present-throw

1 | new Listing { default = (_) -> 2; throw("list-present-throw") }.getOrDefault(0)
                                      ^^^^^^^^^^^^^^^^^^^^^^^^^^^
at repl:text[#1] (repl:text)

1 | new Listing { default = (_) -> 2; throw("list-present-throw") }.getOrDefault(0)
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########.encode_utf16().collect::<Vec<_>>()),
(r########"list-default-typed-missing"########,r########"(new Listing<Int> { 1 }).getOrDefault(2)"########,r########"error"########,r########"–– Pkl Error ––
Tried to read property `default` but its value is undefined.

1 | (new Listing<Int> { 1 }).getOrDefault(2)
     ^^^^^^^^^^^^^^^^^^^^^^
at repl:text.default (repl:text)

1 | (new Listing<Int> { 1 }).getOrDefault(2)
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########.encode_utf16().collect::<Vec<_>>()),
(r########"list-default-typed-bad-return"########,r########"(new Listing<Int> { default = (_) -> "bad"; 1 }).getOrDefault(2)"########,r########"value"########,r########"bad"########.encode_utf16().collect::<Vec<_>>()),
(r########"list-default-wrong-string"########,r########"listingBase.getOrDefault("1")"########,r########"error"########,r########"–– Pkl Error ––
Expected value of type `Int`, but got type `String`.
Value: "1"

2108 | external function getOrDefault(index: Int): Element
                                             ^^^
at pkl.base#Listing.getOrDefault (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L2108)

1 | listingBase.getOrDefault("1")
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########.encode_utf16().collect::<Vec<_>>()),
(r########"list-default-no-args"########,r########"listingBase.getOrDefault()"########,r########"error"########,r########"–– Pkl Error ––
Expected 1 function arguments but got 0.

2108 | external function getOrDefault(index: Int): Element
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#Listing.getOrDefault (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L2108)

1 | listingBase.getOrDefault()
    ^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########.encode_utf16().collect::<Vec<_>>()),
(r########"list-default-two-args"########,r########"listingBase.getOrDefault(0, 1)"########,r########"error"########,r########"–– Pkl Error ––
Expected 1 function arguments but got 2.

2108 | external function getOrDefault(index: Int): Element
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#Listing.getOrDefault (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L2108)

1 | listingBase.getOrDefault(0, 1)
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########.encode_utf16().collect::<Vec<_>>()),
(r########"list-default-as-property"########,r########"listingBase.getOrDefault"########,r########"error"########,r########"–– Pkl Error ––
Cannot find property `getOrDefault` in object of type `Listing`.

1 | listingBase.getOrDefault
    ^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)

Did you mean any of the following?
getOrNull(index)
"########.encode_utf16().collect::<Vec<_>>()),
(r########"map-ornull-empty"########,r########"new Mapping {}.getOrNull("x")"########,r########"value"########,r########"null"########.encode_utf16().collect::<Vec<_>>()),
(r########"map-ornull-present"########,r########"mappingBase.getOrNull("a")"########,r########"value"########,r########"1"########.encode_utf16().collect::<Vec<_>>()),
(r########"map-ornull-null"########,r########"mappingBase.getOrNull("null")"########,r########"value"########,r########"null"########.encode_utf16().collect::<Vec<_>>()),
(r########"map-ornull-number"########,r########"mappingBase.getOrNull(1)"########,r########"value"########,r########"one"########.encode_utf16().collect::<Vec<_>>()),
(r########"map-ornull-absent"########,r########"mappingBase.getOrNull("missing")"########,r########"value"########,r########"null"########.encode_utf16().collect::<Vec<_>>()),
(r########"map-ornull-inherited"########,r########"(mappingBase) { ["a"] = 9 }.getOrNull("a")"########,r########"value"########,r########"9"########.encode_utf16().collect::<Vec<_>>()),
(r########"map-ornull-lazy-error"########,r########"new Mapping { ["x"] = throw("map-ornull-lazy") }.getOrNull("x")"########,r########"error"########,r########"–– Pkl Error ––
map-ornull-lazy

1 | new Mapping { ["x"] = throw("map-ornull-lazy") }.getOrNull("x")
                          ^^^^^^^^^^^^^^^^^^^^^^^^
at repl:text["x"] (repl:text)

1 | new Mapping { ["x"] = throw("map-ornull-lazy") }.getOrNull("x")
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########.encode_utf16().collect::<Vec<_>>()),
(r########"map-ornull-missing-no-force"########,r########"new Mapping { ["x"] = throw("map-ornull-skip") }.getOrNull("missing")"########,r########"value"########,r########"null"########.encode_utf16().collect::<Vec<_>>()),
(r########"map-ornull-object-key"########,r########"let (k = new Dynamic { n = 1 }) (new Mapping { [k] = 42 }).getOrNull(new Dynamic { n = 1 })"########,r########"value"########,r########"42"########.encode_utf16().collect::<Vec<_>>()),
(r########"map-ornull-typed"########,r########"(new Mapping<String, Int> { ["x"] = 2 }).getOrNull("x")"########,r########"value"########,r########"2"########.encode_utf16().collect::<Vec<_>>()),
(r########"map-ornull-typed-wrong-key"########,r########"(new Mapping<String, Int> { ["x"] = 2 }).getOrNull(1)"########,r########"value"########,r########"null"########.encode_utf16().collect::<Vec<_>>()),
(r########"map-ornull-typed-bad-value"########,r########"(new Mapping<String, Int> { ["x"] = "bad" }).getOrNull("x")"########,r########"error"########,r########"–– Pkl Error ––
Expected value of type `Int`, but got type `String`.
Value: "bad"

1 | (new Mapping<String, Int> { ["x"] = "bad" }).getOrNull("x")
                         ^^^
at  (repl:text)

1 | (new Mapping<String, Int> { ["x"] = "bad" }).getOrNull("x")
                                        ^^^^^
at repl:text["x"] (repl:text)

1 | (new Mapping<String, Int> { ["x"] = "bad" }).getOrNull("x")
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########.encode_utf16().collect::<Vec<_>>()),
(r########"map-ornull-no-args"########,r########"mappingBase.getOrNull()"########,r########"error"########,r########"–– Pkl Error ––
Expected 1 function arguments but got 0.

2280 | external function getOrNull(key: Any): Value?
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#Mapping.getOrNull (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L2280)

1 | mappingBase.getOrNull()
    ^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########.encode_utf16().collect::<Vec<_>>()),
(r########"map-ornull-two-args"########,r########"mappingBase.getOrNull("a", 1)"########,r########"error"########,r########"–– Pkl Error ––
Expected 1 function arguments but got 2.

2280 | external function getOrNull(key: Any): Value?
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#Mapping.getOrNull (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L2280)

1 | mappingBase.getOrNull("a", 1)
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########.encode_utf16().collect::<Vec<_>>()),
(r########"map-ornull-as-property"########,r########"mappingBase.getOrNull"########,r########"error"########,r########"–– Pkl Error ––
Cannot find property `getOrNull` in object of type `Mapping`.

1 | mappingBase.getOrNull
    ^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)

Did you mean any of the following?
getOrDefault(key)
"########.encode_utf16().collect::<Vec<_>>()),
(r########"map-default-empty"########,r########"new Mapping {}.getOrDefault("x").getClass().simpleName"########,r########"value"########,r########"Dynamic"########.encode_utf16().collect::<Vec<_>>()),
(r########"map-default-present"########,r########"mappingBase.getOrDefault("a")"########,r########"value"########,r########"1"########.encode_utf16().collect::<Vec<_>>()),
(r########"map-default-null"########,r########"mappingBase.getOrDefault("null")"########,r########"value"########,r########"null"########.encode_utf16().collect::<Vec<_>>()),
(r########"map-default-missing"########,r########"mappingBase.getOrDefault("missing").getClass().simpleName"########,r########"value"########,r########"Dynamic"########.encode_utf16().collect::<Vec<_>>()),
(r########"map-default-custom-string"########,r########"new Mapping { default = (k) -> "fallback-\(k)"; ["x"] = 1 }.getOrDefault("y")"########,r########"value"########,r########"fallback-y"########.encode_utf16().collect::<Vec<_>>()),
(r########"map-default-custom-int-key"########,r########"new Mapping { default = (k) -> k * 2; [1] = 1 }.getOrDefault(4)"########,r########"value"########,r########"8"########.encode_utf16().collect::<Vec<_>>()),
(r########"map-default-custom-present"########,r########"new Mapping { default = (_) -> throw("should-skip"); ["x"] = 1 }.getOrDefault("x")"########,r########"value"########,r########"1"########.encode_utf16().collect::<Vec<_>>()),
(r########"map-default-custom-null"########,r########"new Mapping { default = (_) -> throw("should-skip-null"); ["x"] = null }.getOrDefault("x")"########,r########"value"########,r########"null"########.encode_utf16().collect::<Vec<_>>()),
(r########"map-default-custom-throw"########,r########"new Mapping { default = (_) -> throw("map-default-throw"); ["x"] = 1 }.getOrDefault("missing")"########,r########"error"########,r########"–– Pkl Error ––
map-default-throw

1 | new Mapping { default = (_) -> throw("map-default-throw"); ["x"] = 1 }.getOrDefault("missing")
                                   ^^^^^^^^^^^^^^^^^^^^^^^^^^
at repl:text.default.<function#1> (repl:text)

1 | new Mapping { default = (_) -> throw("map-default-throw"); ["x"] = 1 }.getOrDefault("missing")
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########.encode_utf16().collect::<Vec<_>>()),
(r########"map-default-present-throw"########,r########"new Mapping { default = (_) -> 2; ["x"] = throw("map-present-throw") }.getOrDefault("x")"########,r########"error"########,r########"–– Pkl Error ––
map-present-throw

1 | new Mapping { default = (_) -> 2; ["x"] = throw("map-present-throw") }.getOrDefault("x")
                                              ^^^^^^^^^^^^^^^^^^^^^^^^^^
at repl:text["x"] (repl:text)

1 | new Mapping { default = (_) -> 2; ["x"] = throw("map-present-throw") }.getOrDefault("x")
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########.encode_utf16().collect::<Vec<_>>()),
(r########"map-default-inherited"########,r########"(new Mapping { default = (k) -> "base-\(k)"; ["x"] = 1 }) { ["y"] = 2 }.getOrDefault("z")"########,r########"value"########,r########"base-z"########.encode_utf16().collect::<Vec<_>>()),
(r########"map-default-object-key"########,r########"new Mapping { default = (k) -> k.toString() }.getOrDefault(new Dynamic { n = 1 })"########,r########"value"########,r########"new Dynamic { n = 1 }"########.encode_utf16().collect::<Vec<_>>()),
(r########"map-default-typed-missing"########,r########"(new Mapping<String, Int> { ["x"] = 2 }).getOrDefault("missing")"########,r########"error"########,r########"–– Pkl Error ––
Tried to read property `default` but its value is undefined.

1 | (new Mapping<String, Int> { ["x"] = 2 }).getOrDefault("missing")
     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at repl:text.default (repl:text)

1 | (new Mapping<String, Int> { ["x"] = 2 }).getOrDefault("missing")
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########.encode_utf16().collect::<Vec<_>>()),
(r########"map-default-typed-bad-return"########,r########"(new Mapping<String, Int> { default = (_) -> "bad"; ["x"] = 2 }).getOrDefault("missing")"########,r########"value"########,r########"bad"########.encode_utf16().collect::<Vec<_>>()),
(r########"map-default-typed-wrong-key-no-default"########,r########"(new Mapping<String, Int> { ["x"] = 2 }).getOrDefault(1)"########,r########"error"########,r########"–– Pkl Error ––
Tried to read property `default` but its value is undefined.

1 | (new Mapping<String, Int> { ["x"] = 2 }).getOrDefault(1)
     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at repl:text.default (repl:text)

1 | (new Mapping<String, Int> { ["x"] = 2 }).getOrDefault(1)
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########.encode_utf16().collect::<Vec<_>>()),
(r########"map-default-no-args"########,r########"mappingBase.getOrDefault()"########,r########"error"########,r########"–– Pkl Error ––
Expected 1 function arguments but got 0.

2287 | external function getOrDefault(key: Any): Value
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#Mapping.getOrDefault (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L2287)

1 | mappingBase.getOrDefault()
    ^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########.encode_utf16().collect::<Vec<_>>()),
(r########"map-default-two-args"########,r########"mappingBase.getOrDefault("a", 1)"########,r########"error"########,r########"–– Pkl Error ––
Expected 1 function arguments but got 2.

2287 | external function getOrDefault(key: Any): Value
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#Mapping.getOrDefault (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L2287)

1 | mappingBase.getOrDefault("a", 1)
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########.encode_utf16().collect::<Vec<_>>()),
(r########"map-default-as-property"########,r########"mappingBase.getOrDefault"########,r########"error"########,r########"–– Pkl Error ––
Cannot find property `getOrDefault` in object of type `Mapping`.

1 | mappingBase.getOrDefault
    ^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)

Did you mean any of the following?
getOrNull(key)
"########.encode_utf16().collect::<Vec<_>>()),
];
    assert_eq!(rows.len(), 133);
    for (id, expression, kind, expected) in rows {
        probe
            .evaluate_module_source(
                "file:///stage62/collection-lookups.pkl",
                Some(include_str!(
                    "../../../tests/fixtures/m2-collection-lookups/module.pkl"
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
