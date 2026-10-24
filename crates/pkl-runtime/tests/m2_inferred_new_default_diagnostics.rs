//! Four separate canonical Java modules; complete UTF-16 result/error text and kind.
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
                                resolved.source_uri.as_ref(),
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
fn java_literal_unknown() {
    matrix(
        r########"file:///Users/cody.ij.hwang/Projects/pkl-rs.stage57/target/m2/stage65-default-diagnostics-oracle-01/inputs/literal-unknown/module.pkl"########,
        include_str!(
            "../../../tests/fixtures/m2-inferred-new-default-diagnostics/literal-unknown/module.pkl"
        ),
        None,
        &[(
            r########"literal-unknown"########,
            r########"result"########,
            r########"value"########,
            r########"true"########,
        )],
    );
}

#[test]
fn java_any() {
    matrix(
        r########"file:///Users/cody.ij.hwang/Projects/pkl-rs.stage57/target/m2/stage65-default-diagnostics-oracle-01/inputs/any/module.pkl"########,
        include_str!("../../../tests/fixtures/m2-inferred-new-default-diagnostics/any/module.pkl"),
        None,
        &[(
            r########"any"########,
            r########"result"########,
            r########"error"########,
            r########"–– Pkl Error ––
Tried to read property `default` but its value is undefined.

2 | function make(): Listing<Any> = new { new {} }
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at module#make.default (file:///Users/cody.ij.hwang/Projects/pkl-rs.stage57/target/m2/stage65-default-diagnostics-oracle-01/inputs/any/module.pkl)

2 | function make(): Listing<Any> = new { new {} }
                                          ^^^
at module#make[#1] (file:///Users/cody.ij.hwang/Projects/pkl-rs.stage57/target/m2/stage65-default-diagnostics-oracle-01/inputs/any/module.pkl)

3 | result = make()[0] is Dynamic
             ^^^^^^^^^
at module#result (file:///Users/cody.ij.hwang/Projects/pkl-rs.stage57/target/m2/stage65-default-diagnostics-oracle-01/inputs/any/module.pkl)

1 | result
    ^^^^^^
at  (repl:text)
"########,
        )],
    );
}

#[test]
fn java_alias_unknown() {
    matrix(
        r########"file:///Users/cody.ij.hwang/Projects/pkl-rs.stage57/target/m2/stage65-default-diagnostics-oracle-01/inputs/alias-unknown/module.pkl"########,
        include_str!(
            "../../../tests/fixtures/m2-inferred-new-default-diagnostics/alias-unknown/module.pkl"
        ),
        None,
        &[(
            r########"alias-unknown"########,
            r########"result"########,
            r########"error"########,
            r########"–– Pkl Error ––
Tried to read property `default` but its value is undefined.

2 | function make(): Listing<U> = new { new {} }
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^
at module#make.default (file:///Users/cody.ij.hwang/Projects/pkl-rs.stage57/target/m2/stage65-default-diagnostics-oracle-01/inputs/alias-unknown/module.pkl)

2 | function make(): Listing<U> = new { new {} }
                                        ^^^
at module#make[#1] (file:///Users/cody.ij.hwang/Projects/pkl-rs.stage57/target/m2/stage65-default-diagnostics-oracle-01/inputs/alias-unknown/module.pkl)

3 | result = make()[0] is Dynamic
             ^^^^^^^^^
at module#result (file:///Users/cody.ij.hwang/Projects/pkl-rs.stage57/target/m2/stage65-default-diagnostics-oracle-01/inputs/alias-unknown/module.pkl)

1 | result
    ^^^^^^
at  (repl:text)
"########,
        )],
    );
}

#[test]
fn java_generic_alias_unknown() {
    matrix(
        r########"file:///Users/cody.ij.hwang/Projects/pkl-rs.stage57/target/m2/stage65-default-diagnostics-oracle-01/inputs/generic-alias-unknown/module.pkl"########,
        include_str!(
            "../../../tests/fixtures/m2-inferred-new-default-diagnostics/generic-alias-unknown/module.pkl"
        ),
        None,
        &[(
            r########"generic-alias-unknown"########,
            r########"result"########,
            r########"value"########,
            r########"true"########,
        )],
    );
}
