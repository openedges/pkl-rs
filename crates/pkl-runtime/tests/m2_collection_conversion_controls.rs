//! Complete independent Java 0.32.0 string observations, capture-02/repeat-0.
//! Authority: m2-collection-conversion-controls/oracle-evidence.tar.gz.
#[path = "support/module_keys.rs"]
mod module_keys;
use pkl_runtime::probe::SourceProbe;
#[test]
fn complete_java_conversion_controls() {
    let mut probe = SourceProbe::new_module_evaluator("compact", false, false, false).unwrap();
    let mut failures = Vec::new();
    let rows = [
(r########"list-empty-listing-class"########,r########"List().toListing().getClass().simpleName"########,r########"value"########,r########"Listing"########.encode_utf16().collect::<Vec<_>>()),
(r########"list-listing-class"########,r########"List(1,2).toListing().getClass().simpleName"########,r########"value"########,r########"Listing"########.encode_utf16().collect::<Vec<_>>()),
(r########"set-empty-listing-class"########,r########"Set().toListing().getClass().simpleName"########,r########"value"########,r########"Listing"########.encode_utf16().collect::<Vec<_>>()),
(r########"set-listing-class"########,r########"Set(2,1).toListing().getClass().simpleName"########,r########"value"########,r########"Listing"########.encode_utf16().collect::<Vec<_>>()),
(r########"list-empty-dynamic-class"########,r########"List().toDynamic().getClass().simpleName"########,r########"value"########,r########"Dynamic"########.encode_utf16().collect::<Vec<_>>()),
(r########"list-dynamic-class"########,r########"List(1,2).toDynamic().getClass().simpleName"########,r########"value"########,r########"Dynamic"########.encode_utf16().collect::<Vec<_>>()),
(r########"set-empty-dynamic-class"########,r########"Set().toDynamic().getClass().simpleName"########,r########"value"########,r########"Dynamic"########.encode_utf16().collect::<Vec<_>>()),
(r########"set-dynamic-class"########,r########"Set(2,1).toDynamic().getClass().simpleName"########,r########"value"########,r########"Dynamic"########.encode_utf16().collect::<Vec<_>>()),
(r########"map-empty-mapping-class"########,r########"Map().toMapping().getClass().simpleName"########,r########"value"########,r########"Mapping"########.encode_utf16().collect::<Vec<_>>()),
(r########"map-mapping-class"########,r########"Map("a",1).toMapping().getClass().simpleName"########,r########"value"########,r########"Mapping"########.encode_utf16().collect::<Vec<_>>()),
(r########"map-empty-dynamic-class"########,r########"Map().toDynamic().getClass().simpleName"########,r########"value"########,r########"Dynamic"########.encode_utf16().collect::<Vec<_>>()),
(r########"map-dynamic-class"########,r########"Map("a",1).toDynamic().getClass().simpleName"########,r########"value"########,r########"Dynamic"########.encode_utf16().collect::<Vec<_>>()),
(r########"typed-dynamic-class"########,r########"new Dependent {}.toDynamic().getClass().simpleName"########,r########"value"########,r########"Dynamic"########.encode_utf16().collect::<Vec<_>>()),
(r########"list-listing-missing-null"########,r########"List(1).toListing().getOrNull(3)"########,r########"value"########,r########"null"########.encode_utf16().collect::<Vec<_>>()),
(r########"list-listing-missing-default"########,r########"List(1).toListing().getOrDefault(3).getClass().simpleName"########,r########"value"########,r########"Dynamic"########.encode_utf16().collect::<Vec<_>>()),
(r########"list-listing-missing-subscript"########,r########"List(1).toListing()[3].getClass().simpleName"########,r########"error"########,r########"–– Pkl Error ––
Element index `3` is out of range `0`..`0`.

1 | List(1).toListing()[3].getClass().simpleName
    ^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########.encode_utf16().collect::<Vec<_>>()),
(r########"list-listing-negative-null"########,r########"List(1).toListing().getOrNull(-1)"########,r########"value"########,r########"null"########.encode_utf16().collect::<Vec<_>>()),
(r########"set-listing-missing-null"########,r########"Set(1).toListing().getOrNull(3)"########,r########"value"########,r########"null"########.encode_utf16().collect::<Vec<_>>()),
(r########"set-listing-missing-default"########,r########"Set(1).toListing().getOrDefault(3).getClass().simpleName"########,r########"value"########,r########"Dynamic"########.encode_utf16().collect::<Vec<_>>()),
(r########"set-listing-missing-subscript"########,r########"Set(1).toListing()[3].getClass().simpleName"########,r########"error"########,r########"–– Pkl Error ––
Element index `3` is out of range `0`..`0`.

1 | Set(1).toListing()[3].getClass().simpleName
    ^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########.encode_utf16().collect::<Vec<_>>()),
(r########"map-mapping-missing-null"########,r########"Map("a",1).toMapping().getOrNull("missing")"########,r########"value"########,r########"null"########.encode_utf16().collect::<Vec<_>>()),
(r########"map-mapping-missing-default"########,r########"Map("a",1).toMapping().getOrDefault("missing").getClass().simpleName"########,r########"value"########,r########"Dynamic"########.encode_utf16().collect::<Vec<_>>()),
(r########"map-mapping-missing-subscript"########,r########"Map("a",1).toMapping()["missing"].getClass().simpleName"########,r########"error"########,r########"–– Pkl Error ––
Cannot find key `"missing"`.

1 | Map("a",1).toMapping()["missing"].getClass().simpleName
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########.encode_utf16().collect::<Vec<_>>()),
(r########"map-dynamic-missing-null"########,r########"Map("a",1).toDynamic().getPropertyOrNull("missing")"########,r########"value"########,r########"null"########.encode_utf16().collect::<Vec<_>>()),
(r########"map-dynamic-missing-property"########,r########"Map("a",1).toDynamic().getProperty("missing")"########,r########"error"########,r########"–– Pkl Error ––
Cannot find property `missing` in object of type `Dynamic`.

1 | Map("a",1).toDynamic().getProperty("missing")
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)

Available properties:
a
default
"########.encode_utf16().collect::<Vec<_>>()),
(r########"map-dynamic-missing-subscript"########,r########"Map("a",1).toDynamic()["missing"].getClass().simpleName"########,r########"error"########,r########"–– Pkl Error ––
Cannot find key `"missing"`.

1 | Map("a",1).toDynamic()["missing"].getClass().simpleName
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########.encode_utf16().collect::<Vec<_>>()),
(r########"map-dynamic-present-property"########,r########"Map("a",1).toDynamic().getProperty("a")"########,r########"value"########,r########"1"########.encode_utf16().collect::<Vec<_>>()),
(r########"map-dynamic-present-subscript"########,r########"Map("a",1).toDynamic()["a"]"########,r########"error"########,r########"–– Pkl Error ––
Cannot find key `"a"`.

1 | Map("a",1).toDynamic()["a"]
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########.encode_utf16().collect::<Vec<_>>()),
(r########"direct-dependent-base"########,r########"new Dependent {}.greeting"########,r########"value"########,r########"hello-base"########.encode_utf16().collect::<Vec<_>>()),
(r########"direct-dependent-overridden"########,r########"new Dependent { name = "source" }.greeting"########,r########"value"########,r########"hello-source"########.encode_utf16().collect::<Vec<_>>()),
(r########"dynamic-dependent-missing-name"########,r########"new Dynamic {}.toTyped(Dependent).greeting"########,r########"value"########,r########"hello-base"########.encode_utf16().collect::<Vec<_>>()),
(r########"dynamic-dependent-source-name"########,r########"new Dynamic { name = "source" }.toTyped(Dependent).greeting"########,r########"value"########,r########"hello-base"########.encode_utf16().collect::<Vec<_>>()),
(r########"dynamic-dependent-source-name-read"########,r########"new Dynamic { name = "source" }.toTyped(Dependent).name"########,r########"value"########,r########"source"########.encode_utf16().collect::<Vec<_>>()),
(r########"dynamic-required-missing-name"########,r########"new Dynamic {}.toTyped(RequiredDependent).greeting"########,r########"error"########,r########"–– Pkl Error ––
Tried to read property `name` but its value is undefined.

7 | name: String
    ^^^^
at collection-conversion-controls#RequiredDependent.name (file:///stage62/collection-conversion-controls.pkl)

8 | greeting: String = "hello-\(this.name)"
                                ^^^^^^^^^
at collection-conversion-controls#RequiredDependent.greeting (file:///stage62/collection-conversion-controls.pkl)

1 | new Dynamic {}.toTyped(RequiredDependent).greeting
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########.encode_utf16().collect::<Vec<_>>()),
(r########"dynamic-required-source-name"########,r########"new Dynamic { name = "source" }.toTyped(RequiredDependent).greeting"########,r########"error"########,r########"–– Pkl Error ––
Tried to read property `name` but its value is undefined.

7 | name: String
    ^^^^
at collection-conversion-controls#RequiredDependent.name (file:///stage62/collection-conversion-controls.pkl)

8 | greeting: String = "hello-\(this.name)"
                                ^^^^^^^^^
at collection-conversion-controls#RequiredDependent.greeting (file:///stage62/collection-conversion-controls.pkl)

1 | new Dynamic { name = "source" }.toTyped(RequiredDependent).greeting
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########.encode_utf16().collect::<Vec<_>>()),
(r########"map-dependent-missing-name"########,r########"Map().toTyped(Dependent).greeting"########,r########"value"########,r########"hello-base"########.encode_utf16().collect::<Vec<_>>()),
(r########"map-dependent-source-name"########,r########"Map("name","source").toTyped(Dependent).greeting"########,r########"value"########,r########"hello-base"########.encode_utf16().collect::<Vec<_>>()),
(r########"map-dependent-source-name-read"########,r########"Map("name","source").toTyped(Dependent).name"########,r########"value"########,r########"source"########.encode_utf16().collect::<Vec<_>>()),
(r########"map-required-missing-name"########,r########"Map().toTyped(RequiredDependent).greeting"########,r########"error"########,r########"–– Pkl Error ––
Tried to read property `name` but its value is undefined.

7 | name: String
    ^^^^
at collection-conversion-controls#RequiredDependent.name (file:///stage62/collection-conversion-controls.pkl)

8 | greeting: String = "hello-\(this.name)"
                                ^^^^^^^^^
at collection-conversion-controls#RequiredDependent.greeting (file:///stage62/collection-conversion-controls.pkl)

1 | Map().toTyped(RequiredDependent).greeting
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########.encode_utf16().collect::<Vec<_>>()),
(r########"map-required-source-name"########,r########"Map("name","source").toTyped(RequiredDependent).greeting"########,r########"error"########,r########"–– Pkl Error ––
Tried to read property `name` but its value is undefined.

7 | name: String
    ^^^^
at collection-conversion-controls#RequiredDependent.name (file:///stage62/collection-conversion-controls.pkl)

8 | greeting: String = "hello-\(this.name)"
                                ^^^^^^^^^
at collection-conversion-controls#RequiredDependent.greeting (file:///stage62/collection-conversion-controls.pkl)

1 | Map("name","source").toTyped(RequiredDependent).greeting
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########.encode_utf16().collect::<Vec<_>>()),
(r########"dynamic-nullable-missing"########,r########"new Dynamic {}.toTyped(NullableDependent).greeting"########,r########"value"########,r########"hello-base"########.encode_utf16().collect::<Vec<_>>()),
(r########"dynamic-nullable-explicit-null"########,r########"new Dynamic { name = null }.toTyped(NullableDependent).greeting"########,r########"value"########,r########"hello-base"########.encode_utf16().collect::<Vec<_>>()),
(r########"map-nullable-missing"########,r########"Map().toTyped(NullableDependent).greeting"########,r########"value"########,r########"hello-base"########.encode_utf16().collect::<Vec<_>>()),
(r########"map-nullable-explicit-null"########,r########"Map("name",null).toTyped(NullableDependent).greeting"########,r########"value"########,r########"hello-base"########.encode_utf16().collect::<Vec<_>>()),
(r########"dynamic-dependent-class"########,r########"new Dynamic { name = "source" }.toTyped(Dependent).getClass().simpleName"########,r########"value"########,r########"Dependent"########.encode_utf16().collect::<Vec<_>>()),
(r########"map-dependent-class"########,r########"Map("name","source").toTyped(Dependent).getClass().simpleName"########,r########"value"########,r########"Dependent"########.encode_utf16().collect::<Vec<_>>()),
(r########"high-key-map"########,r########"Map("\u{d800}",7).toDynamic().toMap()"########,r########"value"########,vec![77,97,112,40,34,55296,34,44,32,55,41]),
(r########"high-key-present"########,r########"Map("\u{d800}",7).toDynamic().hasProperty("\u{d800}")"########,r########"value"########,r########"true"########.encode_utf16().collect::<Vec<_>>()),
(r########"high-key-get-property"########,r########"Map("\u{d800}",7).toDynamic().getProperty("\u{d800}")"########,r########"value"########,r########"7"########.encode_utf16().collect::<Vec<_>>()),
(r########"high-key-get-or-null"########,r########"Map("\u{d800}",7).toDynamic().getPropertyOrNull("\u{d800}")"########,r########"value"########,r########"7"########.encode_utf16().collect::<Vec<_>>()),
(r########"high-key-subscript"########,r########"Map("\u{d800}",7).toDynamic()["\u{d800}"]"########,r########"error"########,vec![8211,8211,32,80,107,108,32,69,114,114,111,114,32,8211,8211,10,67,97,110,110,111,116,32,102,105,110,100,32,107,101,121,32,96,34,55296,34,96,46,10,10,49,32,124,32,77,97,112,40,34,92,117,123,100,56,48,48,125,34,44,55,41,46,116,111,68,121,110,97,109,105,99,40,41,91,34,92,117,123,100,56,48,48,125,34,93,10,32,32,32,32,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,10,97,116,32,32,40,114,101,112,108,58,116,101,120,116,41,10]),
(r########"low-key-subscript"########,r########"Map("\u{dc00}",8).toDynamic()["\u{dc00}"]"########,r########"error"########,vec![8211,8211,32,80,107,108,32,69,114,114,111,114,32,8211,8211,10,67,97,110,110,111,116,32,102,105,110,100,32,107,101,121,32,96,34,56320,34,96,46,10,10,49,32,124,32,77,97,112,40,34,92,117,123,100,99,48,48,125,34,44,56,41,46,116,111,68,121,110,97,109,105,99,40,41,91,34,92,117,123,100,99,48,48,125,34,93,10,32,32,32,32,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,10,97,116,32,32,40,114,101,112,108,58,116,101,120,116,41,10]),
(r########"high-plus-ascii-subscript"########,r########"Map("\u{d800}x",9).toDynamic()["\u{d800}x"]"########,r########"error"########,vec![8211,8211,32,80,107,108,32,69,114,114,111,114,32,8211,8211,10,67,97,110,110,111,116,32,102,105,110,100,32,107,101,121,32,96,34,55296,120,34,96,46,10,10,49,32,124,32,77,97,112,40,34,92,117,123,100,56,48,48,125,120,34,44,57,41,46,116,111,68,121,110,97,109,105,99,40,41,91,34,92,117,123,100,56,48,48,125,120,34,93,10,32,32,32,32,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,94,10,97,116,32,32,40,114,101,112,108,58,116,101,120,116,41,10]),
(r########"pair-key-subscript"########,r########"Map("\u{d83d}\u{de00}",10).toDynamic()["\u{d83d}\u{de00}"]"########,r########"error"########,r########"–– Pkl Error ––
Cannot find key `"😀"`.

1 | Map("\u{d83d}\u{de00}",10).toDynamic()["\u{d83d}\u{de00}"]
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########.encode_utf16().collect::<Vec<_>>()),
(r########"high-replacement-distinct-length"########,r########"Map("\u{d800}",7,"\u{fffd}",8).toDynamic().toMap().length"########,r########"value"########,r########"2"########.encode_utf16().collect::<Vec<_>>()),
(r########"high-replacement-distinct-keys"########,r########"Map("\u{d800}",7,"\u{fffd}",8).toDynamic().toMap().keys"########,r########"value"########,vec![83,101,116,40,34,55296,34,44,32,34,65533,34,41]),
(r########"high-replacement-high-value"########,r########"Map("\u{d800}",7,"\u{fffd}",8).toDynamic().getProperty("\u{d800}")"########,r########"value"########,r########"7"########.encode_utf16().collect::<Vec<_>>()),
(r########"high-replacement-replacement-value"########,r########"Map("\u{d800}",7,"\u{fffd}",8).toDynamic().getProperty("\u{fffd}")"########,r########"value"########,r########"8"########.encode_utf16().collect::<Vec<_>>()),
];
    assert_eq!(rows.len(), 58);
    for (id, expression, kind, expected) in rows {
        probe
            .evaluate_module_source(
                "file:///stage62/collection-conversion-controls.pkl",
                Some(include_str!(
                    "../../../tests/fixtures/m2-collection-conversion-controls/module.pkl"
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
