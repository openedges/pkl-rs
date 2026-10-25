//! Exact Java diagnostic boundaries for inferred collection-parent validation.
//! Captured absolute URIs are virtual identities; source text is supplied in memory.
#[path = "support/module_keys.rs"]
mod module_keys;
use pkl_runtime::probe::SourceProbe;

fn observe(id: &str, uri: &str, source: &str, expression: &str, kind: &str, expected: &str) {
    let mut probe = SourceProbe::new_module_evaluator("compact", false, false, false).unwrap();
    let expected = expected.encode_utf16().collect::<Vec<_>>();
    for _ in 0..2 {
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
            if event.kind == "module-request"
                && module_keys::complete_probe(&mut probe, event.token.as_deref().unwrap(), 0)
                    .unwrap()
            {
                continue;
            }
            terminal = Some(event);
            break;
        }
        let event = terminal.expect("inferred entry diagnostic poll bound");
        assert_eq!(event.kind, kind, "{id}");
        let actual = if kind == "value" {
            event.value
        } else {
            event.error
        };
        assert_eq!(actual, Some(expected.clone()), "{id}");
    }
    probe.close();
    assert_eq!(probe.collect().slots, 0, "{id}");
}

#[test]
fn inferred_entry_parent_validation_uses_unavailable_source() {
    observe(
        r########"types-stage18-default-listing-literal"########,
        r########"file:///Users/cody.ij.hwang/Projects/pkl-rs.stage57/tests/fixtures/m2/types-stage18-default-listing-literal.pkl"########,
        include_str!("../../../tests/fixtures/m2/types-stage18-default-listing-literal.pkl"),
        r########"result"########,
        r########"error"########,
        r########"–– Pkl Error ––
Cannot instantiate, or amend an instance of, external class `String`.

4 | result: A<"literal"> = 1
                           ^
at types-stage18-default-listing-literal#result (file:///Users/cody.ij.hwang/Projects/pkl-rs.stage57/tests/fixtures/m2/types-stage18-default-listing-literal.pkl)

To create an instance of an external class, use its literal syntax or constructor method.
Examples:
* `123` uses literal syntax to create an instance of class `Int`.
* `Pair(1, 2)` uses the `Pair()` constructor method to create an instance of class `Pair`.

1 | result
    ^^^^^^
at  (repl:text)
"########,
    );
    observe(
        r########"set-stage23-constructor-listing-default"########,
        r########"file:///Users/cody.ij.hwang/Projects/pkl-rs.stage57/tests/fixtures/m2/set-stage23-constructor-listing-default.pkl"########,
        include_str!("../../../tests/fixtures/m2/set-stage23-constructor-listing-default.pkl"),
        r########"result"########,
        r########"error"########,
        r########"–– Pkl Error ––
Cannot instantiate, or amend an instance of, external class `Set`.

3 | result = values[0].length
             ^^^^^^^^^
at set-stage23-constructor-listing-default#result (file:///Users/cody.ij.hwang/Projects/pkl-rs.stage57/tests/fixtures/m2/set-stage23-constructor-listing-default.pkl)

To create an instance of an external class, use its literal syntax or constructor method.
Examples:
* `123` uses literal syntax to create an instance of class `Int`.
* `Pair(1, 2)` uses the `Pair()` constructor method to create an instance of class `Pair`.

1 | result
    ^^^^^^
at  (repl:text)
"########,
    );
}

#[test]
fn authored_default_and_explicit_constructor_frames_remain_visible() {
    observe(
        r########"list-default-error"########,
        r########"file:///stage65/inferred-new-values.pkl"########,
        include_str!("../../../tests/fixtures/m2-inferred-new-values/module.pkl"),
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
    );
    observe(
        r########"map-default-error"########,
        r########"file:///stage65/inferred-new-values.pkl"########,
        include_str!("../../../tests/fixtures/m2-inferred-new-values/module.pkl"),
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
    );
    observe(
        r########"list-nonfunction-default"########,
        r########"file:///stage65/inferred-new-values.pkl"########,
        include_str!("../../../tests/fixtures/m2-inferred-new-values/module.pkl"),
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
    );
    observe(
        r########"map-nonfunction-default"########,
        r########"file:///stage65/inferred-new-values.pkl"########,
        include_str!("../../../tests/fixtures/m2-inferred-new-values/module.pkl"),
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
    );
    observe(
        r########"external-string"########,
        r########"file:///stage65/inferred-new-values.pkl"########,
        include_str!("../../../tests/fixtures/m2-inferred-new-values/module.pkl"),
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
    );
    observe(
        r########"external-set"########,
        r########"file:///stage65/inferred-new-values.pkl"########,
        include_str!("../../../tests/fixtures/m2-inferred-new-values/module.pkl"),
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
    );
    observe(
        r########"list-if-then"########,
        r########"file:///stage65/inferred-new-values.pkl"########,
        include_str!("../../../tests/fixtures/m2-inferred-new-values/module.pkl"),
        r########"(customList) { [0] = if (true) new { marker = "then" } else new { marker = "else" } }[0].key"########,
        r########"value"########,
        r########"0"########,
    );
    observe(
        r########"map-if-else"########,
        r########"file:///stage65/inferred-new-values.pkl"########,
        include_str!("../../../tests/fixtures/m2-inferred-new-values/module.pkl"),
        r########"(customMap) { ["k"] = if (false) new { marker = "then" } else new { marker = "else" } }["k"].key"########,
        r########"value"########,
        r########"k"########,
    );
}
