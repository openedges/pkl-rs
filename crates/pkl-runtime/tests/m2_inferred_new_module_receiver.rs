//! Ordered Java module-receiver observations, exact canonical UTF-16 text and kind.
//! Each order uses an independent evaluator; collection runs between all VM polls.
#[path = "support/module_keys.rs"]
mod module_keys;
use pkl_runtime::{
    modules::{ModuleCompletion, ModuleRequestKind, ModuleSource, ResolvedModule},
    probe::SourceProbe,
};

fn matrix(uri: &str, source: &str, rows: &[(&str, &str, &str, &str)]) {
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
                            let name = resolved.source_uri.rsplit('/').next().unwrap();
                            let (text, inferred_name) = match name {
                                "alpha.pkl" => (
                                    include_str!(
                                        "../../../tests/fixtures/m2-inferred-new-module-receiver/alpha.pkl"
                                    ),
                                    "alpha",
                                ),
                                "beta.pkl" => (
                                    include_str!(
                                        "../../../tests/fixtures/m2-inferred-new-module-receiver/beta.pkl"
                                    ),
                                    "beta",
                                ),
                                "base.pkl" => (
                                    include_str!(
                                        "../../../tests/fixtures/m2-inferred-new-module-receiver/base.pkl"
                                    ),
                                    "base",
                                ),
                                _ => panic!("unexpected sibling {name}"),
                            };
                            assert!(
                                resolved
                                    .source_uri
                                    .starts_with(uri.rsplit_once('/').unwrap().0)
                            );
                            ModuleCompletion::Source(Ok(ModuleSource {
                                text: text.into(),
                                inferred_name: inferred_name.into(),
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
fn java_module_receiver_alpha_first() {
    let rows = [
        (
            r########"alpha-module-is-alpha"########,
            r########"alphaModule.makeModule().getClass() == alphaModule.getClass()"########,
            r########"value"########,
            r########"true"########,
        ),
        (
            r########"beta-module-is-beta"########,
            r########"betaModule.makeModule().getClass() == betaModule.getClass()"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected value of type `beta`, but got type `alpha`.
Value: new ModuleClass { marker = ? }

5 | function makeModule(): module = new module { marker = "made" }
                           ^^^^^^
at Base#makeModule (file:///Users/cody.ij.hwang/Projects/pkl-rs.stage57/target/m2/stage65-module-receiver-oracle-01/alpha-first/base.pkl)

1 | betaModule.makeModule().getClass() == betaModule.getClass()
    ^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"alpha-nullable-is-alpha"########,
            r########"alphaModule.makeNullable().getClass() == alphaModule.getClass()"########,
            r########"value"########,
            r########"true"########,
        ),
        (
            r########"beta-nullable-is-beta"########,
            r########"betaModule.makeNullable().getClass() == betaModule.getClass()"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected value of type `beta`, but got type `alpha`.
Value: new ModuleClass { marker = ? }

6 | function makeNullable(): module? = new module { marker = "nullable" }
                             ^^^^^^
at Base#makeNullable (file:///Users/cody.ij.hwang/Projects/pkl-rs.stage57/target/m2/stage65-module-receiver-oracle-01/alpha-first/base.pkl)

1 | betaModule.makeNullable().getClass() == betaModule.getClass()
    ^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"alpha-alias-is-alpha"########,
            r########"alphaModule.makeAlias().getClass() == alphaModule.getClass()"########,
            r########"value"########,
            r########"true"########,
        ),
        (
            r########"beta-alias-is-beta"########,
            r########"betaModule.makeAlias().getClass() == betaModule.getClass()"########,
            r########"value"########,
            r########"true"########,
        ),
        (
            r########"cross-module-distinct"########,
            r########"alphaModule.makeModule().getClass() == betaModule.makeModule().getClass()"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected value of type `beta`, but got type `alpha`.
Value: new ModuleClass { marker = ? }

5 | function makeModule(): module = new module { marker = "made" }
                           ^^^^^^
at Base#makeModule (file:///Users/cody.ij.hwang/Projects/pkl-rs.stage57/target/m2/stage65-module-receiver-oracle-01/alpha-first/base.pkl)

1 | alphaModule.makeModule().getClass() == betaModule.makeModule().getClass()
                                           ^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"cross-alias-distinct"########,
            r########"alphaModule.makeAlias().getClass() == betaModule.makeAlias().getClass()"########,
            r########"value"########,
            r########"false"########,
        ),
        (
            r########"alpha-marker"########,
            r########"alphaModule.marker"########,
            r########"value"########,
            r########"alpha"########,
        ),
        (
            r########"beta-marker"########,
            r########"betaModule.marker"########,
            r########"value"########,
            r########"beta"########,
        ),
        (
            r########"modules-distinct"########,
            r########"alphaModule.getClass() == betaModule.getClass()"########,
            r########"value"########,
            r########"false"########,
        ),
    ];
    matrix(
        r########"file:///Users/cody.ij.hwang/Projects/pkl-rs.stage57/target/m2/stage65-module-receiver-oracle-01/alpha-first/module.pkl"########,
        include_str!("../../../tests/fixtures/m2-inferred-new-module-receiver/module.pkl"),
        &rows,
    );
}

#[test]
fn java_module_receiver_beta_first() {
    let rows = [
        (
            r########"beta-module-is-beta"########,
            r########"betaModule.makeModule().getClass() == betaModule.getClass()"########,
            r########"value"########,
            r########"true"########,
        ),
        (
            r########"alpha-module-is-alpha"########,
            r########"alphaModule.makeModule().getClass() == alphaModule.getClass()"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected value of type `alpha`, but got type `beta`.
Value: new ModuleClass { marker = ? }

5 | function makeModule(): module = new module { marker = "made" }
                           ^^^^^^
at Base#makeModule (file:///Users/cody.ij.hwang/Projects/pkl-rs.stage57/target/m2/stage65-module-receiver-oracle-01/beta-first/base.pkl)

1 | alphaModule.makeModule().getClass() == alphaModule.getClass()
    ^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"beta-nullable-is-beta"########,
            r########"betaModule.makeNullable().getClass() == betaModule.getClass()"########,
            r########"value"########,
            r########"true"########,
        ),
        (
            r########"alpha-nullable-is-alpha"########,
            r########"alphaModule.makeNullable().getClass() == alphaModule.getClass()"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected value of type `alpha`, but got type `beta`.
Value: new ModuleClass { marker = ? }

6 | function makeNullable(): module? = new module { marker = "nullable" }
                             ^^^^^^
at Base#makeNullable (file:///Users/cody.ij.hwang/Projects/pkl-rs.stage57/target/m2/stage65-module-receiver-oracle-01/beta-first/base.pkl)

1 | alphaModule.makeNullable().getClass() == alphaModule.getClass()
    ^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"beta-alias-is-beta"########,
            r########"betaModule.makeAlias().getClass() == betaModule.getClass()"########,
            r########"value"########,
            r########"true"########,
        ),
        (
            r########"alpha-alias-is-alpha"########,
            r########"alphaModule.makeAlias().getClass() == alphaModule.getClass()"########,
            r########"value"########,
            r########"true"########,
        ),
        (
            r########"cross-module-distinct"########,
            r########"alphaModule.makeModule().getClass() == betaModule.makeModule().getClass()"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected value of type `alpha`, but got type `beta`.
Value: new ModuleClass { marker = ? }

5 | function makeModule(): module = new module { marker = "made" }
                           ^^^^^^
at Base#makeModule (file:///Users/cody.ij.hwang/Projects/pkl-rs.stage57/target/m2/stage65-module-receiver-oracle-01/beta-first/base.pkl)

1 | alphaModule.makeModule().getClass() == betaModule.makeModule().getClass()
    ^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"cross-alias-distinct"########,
            r########"alphaModule.makeAlias().getClass() == betaModule.makeAlias().getClass()"########,
            r########"value"########,
            r########"false"########,
        ),
        (
            r########"alpha-marker"########,
            r########"alphaModule.marker"########,
            r########"value"########,
            r########"alpha"########,
        ),
        (
            r########"beta-marker"########,
            r########"betaModule.marker"########,
            r########"value"########,
            r########"beta"########,
        ),
        (
            r########"modules-distinct"########,
            r########"alphaModule.getClass() == betaModule.getClass()"########,
            r########"value"########,
            r########"false"########,
        ),
    ];
    matrix(
        r########"file:///Users/cody.ij.hwang/Projects/pkl-rs.stage57/target/m2/stage65-module-receiver-oracle-01/beta-first/module.pkl"########,
        include_str!("../../../tests/fixtures/m2-inferred-new-module-receiver/module.pkl"),
        &rows,
    );
}
