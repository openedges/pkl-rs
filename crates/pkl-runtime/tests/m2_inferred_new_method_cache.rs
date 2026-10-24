//! Canonical Java inferred class/object method cache order: exact UTF-16 text and kind.
//! Oracle bug history is preserved separately and is not counted as compatibility success.
#[path = "support/module_keys.rs"]
mod module_keys;
use pkl_runtime::{
    modules::{ModuleCompletion, ModuleRequestKind, ModuleSource, ResolvedModule},
    probe::SourceProbe,
};

fn matrix(uri: &str, source: &str, siblings: &[(&str, &str)], rows: &[(&str, &str, &str, &str)]) {
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
                            let text = siblings
                                .iter()
                                .find(|(file, _)| *file == name)
                                .expect("recorded sibling")
                                .1;
                            let inferred_name = name.strip_suffix(".pkl").unwrap();
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
fn java_method_alpha_first() {
    let rows = [
        (
            r########"module-alpha"########,
            r########"alphaModule.inferModule().getClass() == alphaModule.getClass()"########,
            r########"value"########,
            r########"true"########,
        ),
        (
            r########"module-beta"########,
            r########"betaModule.inferModule().getClass() == betaModule.getClass()"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected value of type `beta`, but got type `alpha`.
Value: new ModuleClass { marker = ? }

10 | function inferModule(): module = new { marker = "inferred-module" }
                             ^^^^^^
at Base#inferModule (file:///Users/cody.ij.hwang/Projects/pkl-rs.stage57/target/m2/stage65-inferred-method-cache-method-oracle-01/alpha-first/base.pkl)

1 | betaModule.inferModule().getClass() == betaModule.getClass()
    ^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"nullable-alpha"########,
            r########"alphaModule.inferNullable().getClass() == alphaModule.getClass()"########,
            r########"value"########,
            r########"true"########,
        ),
        (
            r########"nullable-beta"########,
            r########"betaModule.inferNullable().getClass() == betaModule.getClass()"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected value of type `beta`, but got type `alpha`.
Value: new ModuleClass { marker = ? }

11 | function inferNullable(): module? = new { marker = "inferred-nullable" }
                               ^^^^^^
at Base#inferNullable (file:///Users/cody.ij.hwang/Projects/pkl-rs.stage57/target/m2/stage65-inferred-method-cache-method-oracle-01/alpha-first/base.pkl)

1 | betaModule.inferNullable().getClass() == betaModule.getClass()
    ^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"alias-alpha"########,
            r########"alphaModule.inferAlias().getClass() == alphaModule.getClass()"########,
            r########"value"########,
            r########"true"########,
        ),
        (
            r########"alias-beta"########,
            r########"betaModule.inferAlias().getClass() == betaModule.getClass()"########,
            r########"value"########,
            r########"false"########,
        ),
        (
            r########"nullable-alias-alpha"########,
            r########"alphaModule.inferNullableAlias().getClass() == alphaModule.getClass()"########,
            r########"value"########,
            r########"true"########,
        ),
        (
            r########"nullable-alias-beta"########,
            r########"betaModule.inferNullableAlias().getClass() == betaModule.getClass()"########,
            r########"value"########,
            r########"false"########,
        ),
        (
            r########"explicit-alias-second"########,
            r########"betaModule.makeAlias().getClass() == betaModule.getClass()"########,
            r########"value"########,
            r########"true"########,
        ),
        (
            r########"module-distinct"########,
            r########"alphaModule.getClass() == betaModule.getClass()"########,
            r########"value"########,
            r########"false"########,
        ),
        (
            r########"second-marker"########,
            r########"betaModule.marker"########,
            r########"value"########,
            r########"beta"########,
        ),
    ];
    matrix(
        r########"file:///Users/cody.ij.hwang/Projects/pkl-rs.stage57/target/m2/stage65-inferred-method-cache-method-oracle-01/alpha-first/module.pkl"########,
        include_str!("../../../tests/fixtures/m2-inferred-new-method-cache/method/module.pkl"),
        &[
            (
                "alpha.pkl",
                include_str!(
                    "../../../tests/fixtures/m2-inferred-new-method-cache/method/alpha.pkl"
                ),
            ),
            (
                "beta.pkl",
                include_str!(
                    "../../../tests/fixtures/m2-inferred-new-method-cache/method/beta.pkl"
                ),
            ),
            (
                "base.pkl",
                include_str!(
                    "../../../tests/fixtures/m2-inferred-new-method-cache/method/base.pkl"
                ),
            ),
        ],
        &rows,
    );
}

#[test]
fn java_method_beta_first() {
    let rows = [
        (
            r########"module-beta"########,
            r########"betaModule.inferModule().getClass() == betaModule.getClass()"########,
            r########"value"########,
            r########"true"########,
        ),
        (
            r########"module-alpha"########,
            r########"alphaModule.inferModule().getClass() == alphaModule.getClass()"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected value of type `alpha`, but got type `beta`.
Value: new ModuleClass { marker = ? }

10 | function inferModule(): module = new { marker = "inferred-module" }
                             ^^^^^^
at Base#inferModule (file:///Users/cody.ij.hwang/Projects/pkl-rs.stage57/target/m2/stage65-inferred-method-cache-method-oracle-01/beta-first/base.pkl)

1 | alphaModule.inferModule().getClass() == alphaModule.getClass()
    ^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"nullable-beta"########,
            r########"betaModule.inferNullable().getClass() == betaModule.getClass()"########,
            r########"value"########,
            r########"true"########,
        ),
        (
            r########"nullable-alpha"########,
            r########"alphaModule.inferNullable().getClass() == alphaModule.getClass()"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected value of type `alpha`, but got type `beta`.
Value: new ModuleClass { marker = ? }

11 | function inferNullable(): module? = new { marker = "inferred-nullable" }
                               ^^^^^^
at Base#inferNullable (file:///Users/cody.ij.hwang/Projects/pkl-rs.stage57/target/m2/stage65-inferred-method-cache-method-oracle-01/beta-first/base.pkl)

1 | alphaModule.inferNullable().getClass() == alphaModule.getClass()
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"alias-beta"########,
            r########"betaModule.inferAlias().getClass() == betaModule.getClass()"########,
            r########"value"########,
            r########"true"########,
        ),
        (
            r########"alias-alpha"########,
            r########"alphaModule.inferAlias().getClass() == alphaModule.getClass()"########,
            r########"value"########,
            r########"false"########,
        ),
        (
            r########"nullable-alias-beta"########,
            r########"betaModule.inferNullableAlias().getClass() == betaModule.getClass()"########,
            r########"value"########,
            r########"true"########,
        ),
        (
            r########"nullable-alias-alpha"########,
            r########"alphaModule.inferNullableAlias().getClass() == alphaModule.getClass()"########,
            r########"value"########,
            r########"false"########,
        ),
        (
            r########"explicit-alias-second"########,
            r########"alphaModule.makeAlias().getClass() == alphaModule.getClass()"########,
            r########"value"########,
            r########"true"########,
        ),
        (
            r########"module-distinct"########,
            r########"alphaModule.getClass() == betaModule.getClass()"########,
            r########"value"########,
            r########"false"########,
        ),
        (
            r########"second-marker"########,
            r########"alphaModule.marker"########,
            r########"value"########,
            r########"alpha"########,
        ),
    ];
    matrix(
        r########"file:///Users/cody.ij.hwang/Projects/pkl-rs.stage57/target/m2/stage65-inferred-method-cache-method-oracle-01/beta-first/module.pkl"########,
        include_str!("../../../tests/fixtures/m2-inferred-new-method-cache/method/module.pkl"),
        &[
            (
                "alpha.pkl",
                include_str!(
                    "../../../tests/fixtures/m2-inferred-new-method-cache/method/alpha.pkl"
                ),
            ),
            (
                "beta.pkl",
                include_str!(
                    "../../../tests/fixtures/m2-inferred-new-method-cache/method/beta.pkl"
                ),
            ),
            (
                "base.pkl",
                include_str!(
                    "../../../tests/fixtures/m2-inferred-new-method-cache/method/base.pkl"
                ),
            ),
        ],
        &rows,
    );
}

#[test]
fn java_object_method_alpha_first() {
    let rows = [
        (
            r########"module-alpha-class"########,
            r########"alphaModule.objectmodule.result.getClass() == alphaModule.getClass()"########,
            r########"value"########,
            r########"true"########,
        ),
        (
            r########"module-beta-class"########,
            r########"betaModule.objectmodule.result.getClass() == betaModule.getClass()"########,
            r########"value"########,
            r########"false"########,
        ),
        (
            r########"nullable-alpha-class"########,
            r########"alphaModule.objectnullable.result.getClass() == alphaModule.getClass()"########,
            r########"value"########,
            r########"true"########,
        ),
        (
            r########"nullable-beta-class"########,
            r########"betaModule.objectnullable.result.getClass() == betaModule.getClass()"########,
            r########"value"########,
            r########"false"########,
        ),
        (
            r########"module-alpha-marker"########,
            r########"alphaModule.objectmodule.result.marker"########,
            r########"value"########,
            r########"object-module"########,
        ),
        (
            r########"module-beta-marker"########,
            r########"betaModule.objectmodule.result.marker"########,
            r########"value"########,
            r########"object-module"########,
        ),
        (
            r########"nullable-alpha-marker"########,
            r########"alphaModule.objectnullable.result.marker"########,
            r########"value"########,
            r########"object-nullable"########,
        ),
        (
            r########"nullable-beta-marker"########,
            r########"betaModule.objectnullable.result.marker"########,
            r########"value"########,
            r########"object-nullable"########,
        ),
        (
            r########"explicit-alias-second"########,
            r########"betaModule.makeAlias().getClass() == betaModule.getClass()"########,
            r########"value"########,
            r########"true"########,
        ),
        (
            r########"module-distinct"########,
            r########"alphaModule.getClass() == betaModule.getClass()"########,
            r########"value"########,
            r########"false"########,
        ),
        (
            r########"second-marker"########,
            r########"betaModule.marker"########,
            r########"value"########,
            r########"beta"########,
        ),
    ];
    matrix(
        r########"file:///Users/cody.ij.hwang/Projects/pkl-rs.stage57/target/m2/stage65-inferred-method-cache-object-method-oracle-01/alpha-first/module.pkl"########,
        include_str!(
            "../../../tests/fixtures/m2-inferred-new-method-cache/object-method/module.pkl"
        ),
        &[
            (
                "alpha.pkl",
                include_str!(
                    "../../../tests/fixtures/m2-inferred-new-method-cache/object-method/alpha.pkl"
                ),
            ),
            (
                "beta.pkl",
                include_str!(
                    "../../../tests/fixtures/m2-inferred-new-method-cache/object-method/beta.pkl"
                ),
            ),
            (
                "base.pkl",
                include_str!(
                    "../../../tests/fixtures/m2-inferred-new-method-cache/object-method/base.pkl"
                ),
            ),
        ],
        &rows,
    );
}

#[test]
fn java_object_method_beta_first() {
    let rows = [
        (
            r########"module-beta-class"########,
            r########"betaModule.objectmodule.result.getClass() == betaModule.getClass()"########,
            r########"value"########,
            r########"true"########,
        ),
        (
            r########"module-alpha-class"########,
            r########"alphaModule.objectmodule.result.getClass() == alphaModule.getClass()"########,
            r########"value"########,
            r########"false"########,
        ),
        (
            r########"nullable-beta-class"########,
            r########"betaModule.objectnullable.result.getClass() == betaModule.getClass()"########,
            r########"value"########,
            r########"true"########,
        ),
        (
            r########"nullable-alpha-class"########,
            r########"alphaModule.objectnullable.result.getClass() == alphaModule.getClass()"########,
            r########"value"########,
            r########"false"########,
        ),
        (
            r########"module-beta-marker"########,
            r########"betaModule.objectmodule.result.marker"########,
            r########"value"########,
            r########"object-module"########,
        ),
        (
            r########"module-alpha-marker"########,
            r########"alphaModule.objectmodule.result.marker"########,
            r########"value"########,
            r########"object-module"########,
        ),
        (
            r########"nullable-beta-marker"########,
            r########"betaModule.objectnullable.result.marker"########,
            r########"value"########,
            r########"object-nullable"########,
        ),
        (
            r########"nullable-alpha-marker"########,
            r########"alphaModule.objectnullable.result.marker"########,
            r########"value"########,
            r########"object-nullable"########,
        ),
        (
            r########"explicit-alias-second"########,
            r########"alphaModule.makeAlias().getClass() == alphaModule.getClass()"########,
            r########"value"########,
            r########"true"########,
        ),
        (
            r########"module-distinct"########,
            r########"alphaModule.getClass() == betaModule.getClass()"########,
            r########"value"########,
            r########"false"########,
        ),
        (
            r########"second-marker"########,
            r########"alphaModule.marker"########,
            r########"value"########,
            r########"alpha"########,
        ),
    ];
    matrix(
        r########"file:///Users/cody.ij.hwang/Projects/pkl-rs.stage57/target/m2/stage65-inferred-method-cache-object-method-oracle-01/beta-first/module.pkl"########,
        include_str!(
            "../../../tests/fixtures/m2-inferred-new-method-cache/object-method/module.pkl"
        ),
        &[
            (
                "alpha.pkl",
                include_str!(
                    "../../../tests/fixtures/m2-inferred-new-method-cache/object-method/alpha.pkl"
                ),
            ),
            (
                "beta.pkl",
                include_str!(
                    "../../../tests/fixtures/m2-inferred-new-method-cache/object-method/beta.pkl"
                ),
            ),
            (
                "base.pkl",
                include_str!(
                    "../../../tests/fixtures/m2-inferred-new-method-cache/object-method/base.pkl"
                ),
            ),
        ],
        &rows,
    );
}
