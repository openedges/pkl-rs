//! Exact Java 0.32.0 UTF-16 result/error text and kind observations.
//! Sealed inferred-new-values72 and inferred-new-amends7, capture-01/repeat-0.
//! Raw protocol/platform comparisons remain separate from these VM regressions.
#[path = "support/module_keys.rs"]
mod module_keys;
use pkl_runtime::{
    modules::{ModuleCompletion, ModuleRequestKind, ModuleSource, ResolvedModule},
    probe::SourceProbe,
};

fn matrix(uri: &str, source: &str, base: Option<&str>, rows: &[(&str, &str, &str, &str)]) {
    let mut probe = SourceProbe::new_module_evaluator("compact", false, false, false).unwrap();
    let mut failures = Vec::new();
    for &(id, expression, kind, expected) in rows {
        probe
            .evaluate_module_source(uri, Some(source), expression, 0, None)
            .unwrap();
        let mut terminal = None;
        for _ in 0..200_000 {
            probe.collect();
            let event = probe.poll(0, 16).unwrap();
            probe.collect();
            if event.kind == "yield" {
                continue;
            }
            if event.kind == "module-request" {
                let token = event.token.as_deref().unwrap();
                let request = probe.module_request(token).unwrap();
                let completion = if let Some(completion) = module_keys::defaults(&request) {
                    completion
                } else {
                    match request.kind {
                        ModuleRequestKind::Resolve { original_uri, .. } => {
                            ModuleCompletion::Resolved(Ok(ResolvedModule {
                                source_uri: original_uri.clone(),
                                resolved_uri: original_uri.clone(),
                                original_uri,
                            }))
                        }
                        ModuleRequestKind::LoadSource { resolved, .. } => {
                            assert_eq!(
                                resolved.source_uri,
                                uri.replace("/module.pkl", "/base.pkl")
                            );
                            ModuleCompletion::Source(Ok(ModuleSource {
                                text: base.expect("amends sibling source").into(),
                                inferred_name: "base".into(),
                            }))
                        }
                        other => panic!("unexpected fixture request {other:?}"),
                    }
                };
                probe.complete_module(token, completion, 0).unwrap();
                continue;
            }
            terminal = Some(event);
            break;
        }
        let event = terminal.expect("inferred new poll bound");
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
                    .map(|units| String::from_utf16_lossy(&units))
                    .unwrap_or_default()
            ));
        }
    }
    probe.close();
    assert_eq!(probe.collect().slots, 0);
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

#[test]
fn complete_java_inferred_new_values_matrix() {
    let rows = [
        (
            r########"module-self-type"########,
            r########"this.getClass().simpleName"########,
            r########"value"########,
            r########"ModuleClass"########,
        ),
        (
            r########"module-typed-method"########,
            r########"typedMethod().getClass().simpleName"########,
            r########"value"########,
            r########"Person"########,
        ),
        (
            r########"module-implicit-method"########,
            r########"implicitMethod().getClass().simpleName"########,
            r########"value"########,
            r########"Dynamic"########,
        ),
        (
            r########"module-unknown-method"########,
            r########"unknownMethod().getClass().simpleName"########,
            r########"value"########,
            r########"Dynamic"########,
        ),
        (
            r########"module-implicit-kind"########,
            r########"implicitMethod().kind"########,
            r########"value"########,
            r########"Dynamic"########,
        ),
        (
            r########"module-unknown-kind"########,
            r########"unknownMethod().kind"########,
            r########"value"########,
            r########"Dynamic"########,
        ),
        (
            r########"class-typed-method"########,
            r########"new MethodHost {}.typed().getClass().simpleName"########,
            r########"value"########,
            r########"Person"########,
        ),
        (
            r########"class-implicit-method"########,
            r########"new MethodHost {}.implicit().getClass().simpleName"########,
            r########"value"########,
            r########"Dynamic"########,
        ),
        (
            r########"class-unknown-method"########,
            r########"new MethodHost {}.unknownResult().getClass().simpleName"########,
            r########"value"########,
            r########"Dynamic"########,
        ),
        (
            r########"object-typed-method"########,
            r########"objectMethod.typedValue.getClass().simpleName"########,
            r########"value"########,
            r########"Person"########,
        ),
        (
            r########"object-implicit-method"########,
            r########"(new Dynamic { local function implicit() = new { kind = getClass().simpleName }; value = implicit() }).value.getClass().simpleName"########,
            r########"value"########,
            r########"Dynamic"########,
        ),
        (
            r########"object-unknown-method"########,
            r########"(new Dynamic { local function unknownResult(): unknown = new { kind = getClass().simpleName }; value = unknownResult() }).value.getClass().simpleName"########,
            r########"value"########,
            r########"Dynamic"########,
        ),
        (
            r########"typed-override"########,
            r########"new ChildHolder {}.child.name"########,
            r########"value"########,
            r########"override"########,
        ),
        (
            r########"typed-override-default"########,
            r########"new ChildHolder {}.child.tag"########,
            r########"value"########,
            r########"person"########,
        ),
        (
            r########"typed-if"########,
            r########"typedIf.name"########,
            r########"value"########,
            r########"if"########,
        ),
        (
            r########"typed-let"########,
            r########"typedLet.name"########,
            r########"value"########,
            r########"let"########,
        ),
        (
            r########"if-condition-context"########,
            r########"(if (new { name = "condition" } is Person) new { name = "yes" } else new { name = "no" }).name"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot tell which parent to amend.

1 | (if (new { name = "condition" } is Person) new { name = "yes" } else new { name = "no" }).name
         ^^^
at repl:text (repl:text)

Please specify a parent explicitly, for example `new Person { ... }` instead of `new { ... }`.
"########,
        ),
        (
            r########"if-condition-direct"########,
            r########"(new Mapping<String, Person> { ["k"] = if (new { name = "condition" }) new { name = "yes" } else new { name = "no" } })["k"].name"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected value of type `Boolean`, but got type `inferred-new-values#Person`.
Value: new Person { name = ?; tag = ? }

1 | (new Mapping<String, Person> { ["k"] = if (new { name = "condition" }) new { name = "yes" } else new { name = "no" } })["k"].name
                                               ^^^^^^^^^^^^^^^^^^^^^^^^^^
at repl:text["k"] (repl:text)

1 | (new Mapping<String, Person> { ["k"] = if (new { name = "condition" }) new { name = "yes" } else new { name = "no" } })["k"].name
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"let-binding-boundary"########,
            r########"(let (x = new { name = "binding" }) new { name = x.getClass().simpleName }).name"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot tell which parent to amend.

1 | (let (x = new { name = "binding" }) new { name = x.getClass().simpleName }).name
              ^^^
at repl:text (repl:text)

Please specify a parent explicitly, for example `new Person { ... }` instead of `new { ... }`.
"########,
        ),
        (
            r########"lambda-boundary"########,
            r########"List(1).map((x) -> new { kind = getClass().simpleName })[0].getClass().simpleName"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot tell which parent to amend.

1 | List(1).map((x) -> new { kind = getClass().simpleName })[0].getClass().simpleName
                       ^^^
at repl:text.<function#1> (repl:text)

Please specify a parent explicitly, for example `new Person { ... }` instead of `new { ... }`.
"########,
        ),
        (
            r########"lambda-explicit-parent"########,
            r########"List(1).map((x) -> new Dynamic { marker = "explicit" })[0].marker"########,
            r########"value"########,
            r########"explicit"########,
        ),
        (
            r########"root-inferred"########,
            r########"(new { kind = getClass().simpleName }).getClass().simpleName"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot tell which parent to amend.

1 | (new { kind = getClass().simpleName }).getClass().simpleName
     ^^^
at repl:text (repl:text)

Please specify a parent explicitly, for example `new Person { ... }` instead of `new { ... }`.
"########,
        ),
        (
            r########"list-assigned-root"########,
            r########"(customList) { [0] = new { marker = "assigned" } }[0].marker"########,
            r########"value"########,
            r########"assigned"########,
        ),
        (
            r########"list-assigned-parent"########,
            r########"(customList) { [0] = new { marker = "assigned" } }[0].key"########,
            r########"value"########,
            r########"0"########,
        ),
        (
            r########"list-amendment"########,
            r########"(customList) { [0] { marker = "amended" } }[0].marker"########,
            r########"value"########,
            r########"amended"########,
        ),
        (
            r########"list-amendment-parent"########,
            r########"(customList) { [0] { marker = "amended" } }[0].key"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot find property `key` in object of type `Dynamic`.

1 | (customList) { [0] { marker = "amended" } }[0].key
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)

Available properties:
default
marker
"########,
        ),
        (
            r########"map-assigned-root"########,
            r########"(customMap) { ["k"] = new { marker = "assigned" } }["k"].marker"########,
            r########"value"########,
            r########"assigned"########,
        ),
        (
            r########"map-assigned-parent"########,
            r########"(customMap) { ["k"] = new { marker = "assigned" } }["k"].key"########,
            r########"value"########,
            r########"k"########,
        ),
        (
            r########"map-amendment"########,
            r########"(customMap) { ["k"] { marker = "amended" } }["k"].marker"########,
            r########"value"########,
            r########"amended"########,
        ),
        (
            r########"map-amendment-parent"########,
            r########"(customMap) { ["k"] { marker = "amended" } }["k"].key"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot find property `key` in object of type `Dynamic`.

1 | (customMap) { ["k"] { marker = "amended" } }["k"].key
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)

Available properties:
default
marker
"########,
        ),
        (
            r########"list-if-then"########,
            r########"(customList) { [0] = if (true) new { marker = "then" } else new { marker = "else" } }[0].key"########,
            r########"value"########,
            r########"0"########,
        ),
        (
            r########"list-if-else"########,
            r########"(customList) { [0] = if (false) new { marker = "then" } else new { marker = "else" } }[0].key"########,
            r########"value"########,
            r########"0"########,
        ),
        (
            r########"map-if-then"########,
            r########"(customMap) { ["k"] = if (true) new { marker = "then" } else new { marker = "else" } }["k"].key"########,
            r########"value"########,
            r########"k"########,
        ),
        (
            r########"map-if-else"########,
            r########"(customMap) { ["k"] = if (false) new { marker = "then" } else new { marker = "else" } }["k"].key"########,
            r########"value"########,
            r########"k"########,
        ),
        (
            r########"list-let-body"########,
            r########"(customList) { [0] = let (x = 1) new { marker = "body" } }[0].key"########,
            r########"value"########,
            r########"0"########,
        ),
        (
            r########"map-let-body"########,
            r########"(customMap) { ["k"] = let (x = 1) new { marker = "body" } }["k"].key"########,
            r########"value"########,
            r########"k"########,
        ),
        (
            r########"list-trace"########,
            r########"(customList) { [0] = trace(new { marker = "trace" }) }[0].key"########,
            r########"value"########,
            r########"0"########,
        ),
        (
            r########"map-trace"########,
            r########"(customMap) { ["k"] = trace(new { marker = "trace" }) }["k"].key"########,
            r########"value"########,
            r########"k"########,
        ),
        (
            r########"list-skipped-else"########,
            r########"(customList) { [0] = if (true) new { marker = "selected" } else new { marker = throw("skipped-else") } }[0].marker"########,
            r########"value"########,
            r########"selected"########,
        ),
        (
            r########"map-skipped-then"########,
            r########"(customMap) { ["k"] = if (false) new { marker = throw("skipped-then") } else new { marker = "selected" } }["k"].marker"########,
            r########"value"########,
            r########"selected"########,
        ),
        (
            r########"list-let-binding-boundary"########,
            r########"(customList) { [0] = let (x = new { marker = "binding" }) new { marker = x.getClass().simpleName } }[0].key"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot tell which parent to amend.

1 | (customList) { [0] = let (x = new { marker = "binding" }) new { marker = x.getClass().simpleName } }[0].key
                                  ^^^
at repl:text[0] (repl:text)

Please specify a parent explicitly, for example `new Person { ... }` instead of `new { ... }`.
"########,
        ),
        (
            r########"map-lambda-boundary"########,
            r########"(customMap) { ["k"] = List(1).map((x) -> new { marker = "lambda" })[0] }["k"].getClass().simpleName"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot tell which parent to amend.

1 | (customMap) { ["k"] = List(1).map((x) -> new { marker = "lambda" })[0] }["k"].getClass().simpleName
                                             ^^^
at repl:text["k"].<function#1> (repl:text)

Please specify a parent explicitly, for example `new Person { ... }` instead of `new { ... }`.
"########,
        ),
        (
            r########"list-key-error-first"########,
            r########"(customList) { [throw("list-key-first")] = new { marker = "value" } }[0]"########,
            r########"error"########,
            r########"–– Pkl Error ––
list-key-first

1 | (customList) { [throw("list-key-first")] = new { marker = "value" } }[0]
                    ^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"map-key-error-first"########,
            r########"(customMap) { [throw("map-key-first")] = new { marker = "value" } }["k"]"########,
            r########"error"########,
            r########"–– Pkl Error ––
map-key-first

1 | (customMap) { [throw("map-key-first")] = new { marker = "value" } }["k"]
                   ^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"list-default-error"########,
            r########"(new Listing { default = (_) -> throw("list-default-first"); 1 }) { [0] = new { marker = "value" } }[0]"########,
            r########"error"########,
            r########"–– Pkl Error ––
list-default-first

1 | (new Listing { default = (_) -> throw("list-default-first"); 1 }) { [0] = new { marker = "value" } }[0]
                                    ^^^^^^^^^^^^^^^^^^^^^^^^^^^
at repl:text.default.<function#1> (repl:text)

1 | (new Listing { default = (_) -> throw("list-default-first"); 1 }) { [0] = new { marker = "value" } }[0]
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"map-default-error"########,
            r########"(new Mapping { default = (_) -> throw("map-default-first"); ["k"] = 1 }) { ["k"] = new { marker = "value" } }["k"]"########,
            r########"error"########,
            r########"–– Pkl Error ––
map-default-first

1 | (new Mapping { default = (_) -> throw("map-default-first"); ["k"] = 1 }) { ["k"] = new { marker = "value" } }["k"]
                                    ^^^^^^^^^^^^^^^^^^^^^^^^^^
at repl:text.default.<function#1> (repl:text)

1 | (new Mapping { default = (_) -> throw("map-default-first"); ["k"] = 1 }) { ["k"] = new { marker = "value" } }["k"]
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"list-nonfunction-default"########,
            r########"(new Listing { default = 1; 1 }) { [0] = new { marker = "value" } }[0]"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected value of type `Function1`, but got type `Int`.
Value: 1

2068 | hidden default: (Int) -> Element = (_) -> new Dynamic {}
                       ^^^^^^^^^^^^^^^^
at pkl.base#Listing.default (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L2068)

1 | (new Listing { default = 1; 1 }) { [0] = new { marker = "value" } }[0]
                             ^
at repl:text.default (repl:text)

1 | (new Listing { default = 1; 1 }) { [0] = new { marker = "value" } }[0]
                                             ^^^
at repl:text[0] (repl:text)

1 | (new Listing { default = 1; 1 }) { [0] = new { marker = "value" } }[0]
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"map-nonfunction-default"########,
            r########"(new Mapping { default = 1; ["k"] = 1 }) { ["k"] = new { marker = "value" } }["k"]"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected value of type `Function1`, but got type `Int`.
Value: 1

2255 | hidden default: (Key) -> Value = (_) -> new Dynamic {}
                       ^^^^^^^^^^^^^^
at pkl.base#Mapping.default (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L2255)

1 | (new Mapping { default = 1; ["k"] = 1 }) { ["k"] = new { marker = "value" } }["k"]
                             ^
at repl:text.default (repl:text)

1 | (new Mapping { default = 1; ["k"] = 1 }) { ["k"] = new { marker = "value" } }["k"]
                                                       ^^^
at repl:text["k"] (repl:text)

1 | (new Mapping { default = 1; ["k"] = 1 }) { ["k"] = new { marker = "value" } }["k"]
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"typed-list-assigned"########,
            r########"(listBase) { [0] = new { name = "typed" } }[0].getClass().simpleName"########,
            r########"value"########,
            r########"Person"########,
        ),
        (
            r########"typed-list-amend"########,
            r########"(listBase) { [0] { name = "typed-amend" } }[0].name"########,
            r########"value"########,
            r########"typed-amend"########,
        ),
        (
            r########"typed-map-assigned"########,
            r########"(mapBase) { ["k"] = new { name = "typed" } }["k"].getClass().simpleName"########,
            r########"value"########,
            r########"Person"########,
        ),
        (
            r########"typed-map-amend"########,
            r########"(mapBase) { ["k"] { name = "typed-amend" } }["k"].name"########,
            r########"value"########,
            r########"typed-amend"########,
        ),
        (
            r########"class"########,
            r########"(new Person { name = "direct" }).name"########,
            r########"value"########,
            r########"direct"########,
        ),
        (
            r########"nullable-class"########,
            r########"(new Person? { name = "nullable" }).name"########,
            r########"value"########,
            r########"nullable"########,
        ),
        (
            r########"default-union"########,
            r########"(new *Person|String { name = "union" }).getClass().simpleName"########,
            r########"value"########,
            r########"Person"########,
        ),
        (
            r########"no-default-union"########,
            r########"new String|Person {}"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot instantiate type `String|Person`.

1 | new String|Person {}
    ^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"alias"########,
            r########"(new PersonAlias { name = "alias" }).name"########,
            r########"value"########,
            r########"alias"########,
        ),
        (
            r########"nullable-alias"########,
            r########"(new PersonAlias? { name = "alias-nullable" }).name"########,
            r########"value"########,
            r########"alias-nullable"########,
        ),
        (
            r########"generic-listing"########,
            r########"(new Listing<Person> { new { name = "generic" } })[0].name"########,
            r########"value"########,
            r########"generic"########,
        ),
        (
            r########"generic-alias"########,
            r########"(new PersonListing { new { name = "alias-generic" } })[0].name"########,
            r########"value"########,
            r########"alias-generic"########,
        ),
        (
            r########"generic-mapping"########,
            r########"(new Mapping<String, Person> { ["k"] = new { name = "generic-map" } })["k"].name"########,
            r########"value"########,
            r########"generic-map"########,
        ),
        (
            r########"constraint-alias-external"########,
            r########"new NonEmptyText {}"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot instantiate type `NonEmptyText`.

1 | new NonEmptyText {}
    ^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"constraint-class"########,
            r########"(new Person(this.name == "ok") { name = "ok" }).name"########,
            r########"value"########,
            r########"ok"########,
        ),
        (
            r########"constraint-class-bad-value"########,
            r########"new Person(this.name == "ok") { name = "bad" }"########,
            r########"value"########,
            r########"new Person { name = "bad"; tag = "person" }"########,
        ),
        (
            r########"constraint-check-ok"########,
            r########"new Person { name = "ok" } is Person(this.name == "ok")"########,
            r########"value"########,
            r########"true"########,
        ),
        (
            r########"constraint-check-bad"########,
            r########"new Person { name = "bad" } is Person(this.name == "ok")"########,
            r########"value"########,
            r########"false"########,
        ),
        (
            r########"external-int"########,
            r########"new Int {}"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot instantiate, or amend an instance of, external class `Int`.

1 | new Int {}
        ^^^
at  (repl:text)

To create an instance of an external class, use its literal syntax or constructor method.
Examples:
* `123` uses literal syntax to create an instance of class `Int`.
* `Pair(1, 2)` uses the `Pair()` constructor method to create an instance of class `Pair`.
"########,
        ),
        (
            r########"external-set"########,
            r########"new Set {}"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot instantiate, or amend an instance of, external class `Set`.

1 | new Set {}
    ^^^^^^^^^^
at  (repl:text)

To create an instance of an external class, use its literal syntax or constructor method.
Examples:
* `123` uses literal syntax to create an instance of class `Int`.
* `Pair(1, 2)` uses the `Pair()` constructor method to create an instance of class `Pair`.
"########,
        ),
        (
            r########"external-string"########,
            r########"new String {}"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot instantiate, or amend an instance of, external class `String`.

1 | new String {}
        ^^^^^^
at  (repl:text)

To create an instance of an external class, use its literal syntax or constructor method.
Examples:
* `123` uses literal syntax to create an instance of class `Int`.
* `Pair(1, 2)` uses the `Pair()` constructor method to create an instance of class `Pair`.
"########,
        ),
        (
            r########"string-literal"########,
            r########"new "foo" {}"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot instantiate, or amend an instance of, external class `String`.

1 | new "foo" {}
    ^^^^^^^^^^^^
at  (repl:text)

To create an instance of an external class, use its literal syntax or constructor method.
Examples:
* `123` uses literal syntax to create an instance of class `Int`.
* `Pair(1, 2)` uses the `Pair()` constructor method to create an instance of class `Pair`.
"########,
        ),
        (
            r########"abstract-class"########,
            r########"new AbstractThing {}"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot instantiate abstract class `inferred-new-values#AbstractThing`.

1 | new AbstractThing {}
        ^^^^^^^^^^^^^
at  (repl:text)

Abstract classes cannot be instantiated.
Instead, instantiate a concrete subclass.
"########,
        ),
        (
            r########"unknown-type"########,
            r########"new unknown {}"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot instantiate type `unknown`.

1 | new unknown {}
    ^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
    ];
    assert_eq!(rows.len(), 72);
    matrix(
        r########"file:///stage65/inferred-new-values.pkl"########,
        include_str!("../../../tests/fixtures/m2-inferred-new-values/module.pkl"),
        None,
        &rows,
    );
}

#[test]
fn complete_java_inferred_new_amends_matrix() {
    let rows = [
        (
            r########"amends-module-self-type"########,
            r########"this.getClass().simpleName"########,
            r########"value"########,
            r########"ModuleClass"########,
        ),
        (
            r########"amends-typed"########,
            r########"result[0].getClass().simpleName"########,
            r########"value"########,
            r########"Person"########,
        ),
        (
            r########"amends-implicit"########,
            r########"result[1].getClass().simpleName"########,
            r########"value"########,
            r########"Dynamic"########,
        ),
        (
            r########"amends-unknown"########,
            r########"result[2].getClass().simpleName"########,
            r########"value"########,
            r########"Dynamic"########,
        ),
        (
            r########"amends-implicit-kind"########,
            r########"result[1].kind"########,
            r########"value"########,
            r########"Dynamic"########,
        ),
        (
            r########"amends-unknown-kind"########,
            r########"result[2].kind"########,
            r########"value"########,
            r########"Dynamic"########,
        ),
        (
            r########"amends-result"########,
            r########"result"########,
            r########"value"########,
            r########"List(new Person {
  name = ?
  tag = ?
}, new Dynamic {
  kind = "Dynamic"
}, new Dynamic {
  kind = "Dynamic"
})"########,
        ),
    ];
    assert_eq!(rows.len(), 7);
    matrix(
        r########"file:///Users/cody.ij.hwang/Projects/pkl-rs.stage57/target/m2/stage65-inferred-new-amends-oracle-01/module.pkl"########,
        include_str!("../../../tests/fixtures/m2-inferred-new-amends/module.pkl"),
        Some(include_str!(
            "../../../tests/fixtures/m2-inferred-new-amends/base.pkl"
        )),
        &rows,
    );
}
