//! Additive Java 0.32.0 observations from stage58-any-oracle-supplement-02.
//! Full raw authority: tests/fixtures/m2-any-if-non-null-supplement/.
#[path = "support/module_keys.rs"]
mod module_keys;

use pkl_runtime::probe::{SourceEvent, SourceProbe};

fn query(probe: &mut SourceProbe, expression: &str) -> SourceEvent {
    probe
        .evaluate_module_source(
            "file:///stage58/any-if-non-null-supplement.pkl",
            Some(include_str!(
                "../../../tests/fixtures/m2-any-if-non-null-supplement/module.pkl"
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
    panic!("Any.ifNonNull supplement poll bound")
}

#[test]
fn return_checks_escaped_closures_function_defaults_and_implicit_receivers_match_java() {
    let mut probe = SourceProbe::new_module_evaluator("compact", false, false, false).unwrap();
    let mut mismatches = Vec::new();
    for (id, expression, kind, expected) in [
        (
            r####"return-value-error"####,
            r####"1.ifNonNull((x) -> badReturn(x))"####,
            "error",
            r####"–– Pkl Error ––
Expected value of type `Int`, but got type `String`.
Value: "bad"

3 | function badReturn(x): Int = "bad"
                           ^^^
at any-if-non-null-supplement#badReturn (file:///stage58/any-if-non-null-supplement.pkl)

1 | 1.ifNonNull((x) -> badReturn(x))
                       ^^^^^^^^^^^^
at repl:text.<function#1> (repl:text)

1 | 1.ifNonNull((x) -> badReturn(x))
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"####,
        ),
        (
            r####"return-null-skip"####,
            r####"null.ifNonNull((x) -> badReturn(x))"####,
            "value",
            r####"null"####,
        ),
        (
            r####"return-default-null-skip"####,
            r####"Null(new Dynamic { marker = "default" }).ifNonNull((x) -> badReturn(x))"####,
            "value",
            r####"null"####,
        ),
        (
            r####"return-retry"####,
            r####"1.ifNonNull((x) -> x + 1)"####,
            "value",
            r####"2"####,
        ),
        (
            r####"default-lambda-marker"####,
            r####"((Null(() -> new Dynamic { marker = "deferred" }).ifNonNull((x) -> x)) { extra = 8 }).apply().marker"####,
            "value",
            r####"deferred"####,
        ),
        (
            r####"default-lambda-extra"####,
            r####"((Null(() -> new Dynamic { marker = "deferred" }).ifNonNull((x) -> x)) { extra = 8 }).apply().extra"####,
            "value",
            r####"8"####,
        ),
        (
            r####"dynamic-local-implicit"####,
            r####"(new Dynamic { local function ifNonNull(f) = "local"; result = ifNonNull((x) -> "base") }).result"####,
            "value",
            r####"local"####,
        ),
        (
            r####"dynamic-property-implicit"####,
            r####"(new Dynamic { ifNonNull = "property"; result = ifNonNull((x) -> "base") }).result"####,
            "value",
            r####"base"####,
        ),
        (
            r####"escaped-closure"####,
            r####"(new Dynamic { value = "receiver" }).ifNonNull((x) -> (suffix) -> x.value + suffix).apply("!")"####,
            "value",
            r####"receiver!"####,
        ),
        (
            r####"null-escaped-closure"####,
            r####"null.ifNonNull((x) -> (suffix) -> "unreachable")"####,
            "value",
            r####"null"####,
        ),
        (
            r####"constraint-int-implicit"####,
            r####"10 as Int(ifNonNull((x) -> x) > 0)"####,
            "value",
            r####"10"####,
        ),
        (
            r####"constraint-null-implicit"####,
            r####"null as Null(ifNonNull((x) -> throw("never")) == null)"####,
            "value",
            r####"null"####,
        ),
        (
            r####"constraint-default-implicit"####,
            r####"(Null(new Dynamic { x = 1 }) as Null(ifNonNull((x) -> throw("never")) == null)) == null"####,
            "value",
            r####"true"####,
        ),
    ] {
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
