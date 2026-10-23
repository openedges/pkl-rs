//! Stage42 source-session typed continuations. Lifecycle tests are architecture
//! regressions; the unchanged Java8 control below compares actual oracle units.
use pkl_runtime::probe::{SourceEvent, SourceProbe};

fn probe(source: &str, expressions: &[&str]) -> SourceProbe {
    SourceProbe::new_expressions(
        source,
        "file:///stage42-types/main.pkl",
        "main",
        &expressions.iter().map(|s| (*s).into()).collect::<Vec<_>>(),
        "compact",
        false,
        false,
        false,
    )
    .unwrap()
}
fn next(probe: &mut SourceProbe) -> SourceEvent {
    for _ in 0..100_000 {
        let event = probe.poll(0, 1).unwrap();
        probe.collect();
        if event.kind != "yield" {
            return event;
        }
    }
    panic!("typed session failed to settle");
}
fn resolve(probe: &mut SourceProbe, request: SourceEvent) -> SourceEvent {
    assert_eq!(request.kind, "module-request");
    let module = request.module.unwrap();
    assert_eq!(module.kind, "resolve");
    probe
        .complete_module_resolution(
            request.token.as_deref().unwrap(),
            &module.original_uri,
            &module.original_uri,
            &module.original_uri,
            0,
        )
        .unwrap();
    let source = next(probe);
    assert_eq!(source.kind, "module-request");
    assert_eq!(source.module.as_ref().unwrap().kind, "load-source");
    source
}
fn assert_value(event: SourceEvent, expected: &str) {
    assert_eq!(event.kind, "value");
    assert_eq!(
        event.value.unwrap(),
        expected.encode_utf16().collect::<Vec<_>>()
    );
}

#[test]
fn synthetic_expression_types_use_runtime_owner_without_exposing_lexical_variables() {
    // Original closed Stage41 Java8 source/physical URI; source SHA256 9ceb5972ab3d5601148be3bee3e0264ee2de986fa5a71356428a52a1b9f812a2.
    // Expected UTF16 is copied from both byte-identical Java repetitions, never Rust.
    let mut probe=SourceProbe::new_expressions(
        r####"import "life:/child" as hiddenImport
class Person { name="person" }
local class Private { name="private" }
visible=7
"####,
        r####"file:///Users/cody.ij.hwang/Projects/pkl-rs.stage41/target/m2/stage41-expression-context-oracle-01/workspace/module-declared-type-context/main.pkl"####,
        "main", &["new Person {}.name".into(),"new hiddenImport.Person {}.name".into(),
            "new Private {}.name".into(),"hiddenImport".into()], "compact",false,false,false,
    ).unwrap();
    let expected: [&[u16]; 3] = [
        &[112, 101, 114, 115, 111, 110],
        &[99, 104, 105, 108, 100],
        &[112, 114, 105, 118, 97, 116, 101],
    ];
    let mut requests = 0;
    for (index, expected) in expected.into_iter().enumerate() {
        probe
            .start_path(index.try_into().unwrap(), 0, None)
            .unwrap();
        let mut event = next(&mut probe);
        if event.kind == "module-request" {
            requests += 1;
            let loading = resolve(&mut probe, event);
            probe
                .complete_module_source(
                    loading.token.as_deref().unwrap(),
                    "class Person { name=\"child\" }\nvalue=9\n",
                    "child",
                    0,
                )
                .unwrap();
            event = next(&mut probe);
        }
        assert_eq!(event.kind, "value");
        assert_eq!(event.value.unwrap(), expected);
    }
    assert_eq!(requests, 1);
    probe.start_path(3, 0, None).unwrap();
    let hidden = next(&mut probe);
    assert_eq!(hidden.kind, "error");
    assert_eq!(hidden.disposition, Some("language-error"));
    probe.close();
    assert_eq!(probe.collect().slots, 0);
}

#[test]
fn type_source_continuation_keeps_receiver_alive_across_gc_and_callback() {
    let mut probe = probe(
        "import \"life:/dep\" as dep\nstable=9\n",
        &["new dep.Person {}.name", "stable+1"],
    );
    probe.start_path(0, 0, Some(100)).unwrap();
    let request = next(&mut probe);
    let source = resolve(&mut probe, request);
    let token = source.token.unwrap();
    probe.reenter_path(&token, 1, 0, None).unwrap();
    assert_value(next(&mut probe), "10");
    assert_eq!(next(&mut probe).token.as_deref(), Some(token.as_str()));
    probe
        .complete_module_source(&token, "class Person { name=\"child\" }\n", "dep", 0)
        .unwrap();
    assert_value(next(&mut probe), "child");
    probe.start_path(0, 0, None).unwrap();
    assert_value(next(&mut probe), "child");
    probe.close();
    assert_eq!(probe.collect().slots, 0);
}

#[test]
fn failed_type_resolution_retries_without_losing_other_session_entries() {
    let mut probe = probe(
        "import \"life:/dep\" as dep\nstable=9\n",
        &["new dep.Person {}.name", "stable"],
    );
    probe.start_path(0, 0, None).unwrap();
    let request = next(&mut probe);
    assert_eq!(request.kind, "module-request");
    let first = request.token.unwrap();
    probe
        .reject_module(&first, false, "not-found", "", 0)
        .unwrap();
    let failure = next(&mut probe);
    assert_eq!(failure.kind, "error");
    assert_eq!(failure.disposition, Some("language-error"));
    probe.start_path(1, 0, None).unwrap();
    assert_value(next(&mut probe), "9");
    probe.start_path(0, 0, None).unwrap();
    let request = next(&mut probe);
    assert_ne!(request.token.as_deref(), Some(first.as_str()));
    let source = resolve(&mut probe, request);
    probe
        .complete_module_source(
            source.token.as_deref().unwrap(),
            "class Person { name=\"child\" }\n",
            "dep",
            0,
        )
        .unwrap();
    assert_value(next(&mut probe), "child");
    probe.close();
    assert_eq!(probe.collect().slots, 0);
}

#[test]
fn cancelled_type_source_token_is_drained_once_and_cannot_resume_the_session() {
    let mut probe = probe("import \"life:/dep\" as dep\n", &["new dep.Person {}.name"]);
    probe.start_path(0, 0, None).unwrap();
    let request = next(&mut probe);
    let source = resolve(&mut probe, request);
    let token = source.token.unwrap();
    probe.cancel();
    assert_eq!(probe.drain_cancelled_modules(), vec![token.clone()]);
    assert!(probe.drain_cancelled_modules().is_empty());
    assert!(probe.drain_cancelled_reads().is_empty());
    assert!(
        probe
            .complete_module_source(&token, "class Person { name=\"child\" }", "dep", 0)
            .is_err()
    );
    probe.close();
    assert_eq!(probe.collect().slots, 0);
}

#[test]
fn same_typed_expression_can_reenter_while_its_import_source_is_pending() {
    // Unchanged Java Stage42 typed-expression-source-reentry source/call order.
    // Java outer and inner values are exact repeat-identical UTF16: 7, true, 7.
    let mut probe = SourceProbe::new_expressions(
r####"import "life:/child" as child
marker = 1
instance = new child.C {}
"####,
r####"file:///Users/cody.ij.hwang/Projects/pkl-rs.stage42/target/m2/stage42-type-reentry-alias-oracle-01/workspace/typed-expression-source-reentry/main.pkl"####,
"main", &[r####"new child.C {}.x"####.into(),r####"instance.getClass() == (new child.C {}).getClass()"####.into(),r####"new child.C {}.x"####.into()], "compact",false,false,false,
).unwrap();
    probe.start_path(0, 0, None).unwrap();
    let request = next(&mut probe);
    let outer_source = resolve(&mut probe, request);
    let outer_token = outer_source.token.unwrap();
    probe.reenter_path(&outer_token, 0, 0, None).unwrap();
    let request = next(&mut probe);
    let inner_source = resolve(&mut probe, request);
    let inner_token = inner_source.token.unwrap();
    assert_ne!(inner_token, outer_token);
    probe
        .complete_module_source(
            &inner_token,
            r####"class C { x = 7 }
"####,
            "child",
            0,
        )
        .unwrap();
    assert_value(next(&mut probe), "7");
    assert_eq!(
        next(&mut probe).token.as_deref(),
        Some(outer_token.as_str())
    );
    probe
        .complete_module_source(
            &outer_token,
            r####"class C { x = 7 }
"####,
            "child",
            0,
        )
        .unwrap();
    assert_value(next(&mut probe), "7");
    probe.start_path(1, 0, None).unwrap();
    assert_value(next(&mut probe), "true");
    probe.start_path(2, 0, None).unwrap();
    assert_value(next(&mut probe), "7");
    probe.close();
    assert_eq!(probe.collect().slots, 0);
}

#[test]
fn partially_published_alias_survives_its_first_post_setup_resolution_failure() {
    // Unchanged Java Stage42 partial-alias-post-setup-access input and ordered calls.
    let mut probe = SourceProbe::new_expressions(
r####"import "life:/child" as child
marker = 1
handle = child.Bad
"####,
r####"file:///Users/cody.ij.hwang/Projects/pkl-rs.stage42/target/m2/stage42-type-reentry-alias-oracle-01/workspace/partial-alias-post-setup-access/main.pkl"####,
"main", &[r####"marker"####.into(),r####"handle"####.into(),r####"handle.getClass()"####.into(),r####"handle"####.into()], "compact",false,false,false,
).unwrap();
    probe.start_path(0, 0, None).unwrap();
    assert_value(next(&mut probe), "1");
    probe.start_path(1, 0, None).unwrap();
    let request = next(&mut probe);
    let loading = resolve(&mut probe, request);
    probe
        .complete_module_source(
            loading.token.as_deref().unwrap(),
            r####"typealias Bad = missing.C
"####,
            "child",
            0,
        )
        .unwrap();
    let failure = next(&mut probe);
    assert_eq!(failure.kind, "error");
    assert_eq!(failure.disposition, Some("language-error"));
    // Java raw UTF8 SHA256 9b6fd8e0c219c1e6bfa6c1a06bbb8abc0d3332b627d7cfe544e861dfc923ce6f; both Java repetitions and BE units match.
    assert_eq!(failure.error.unwrap(), r####"–– Pkl Error ––
Cannot find module import `missing`.

1 | typealias Bad = missing.C
                    ^^^^^^^
at child#Bad (life:/child)

3 | handle = child.Bad
             ^^^^^^^^^
at main#handle (file:///Users/cody.ij.hwang/Projects/pkl-rs.stage42/target/m2/stage42-type-reentry-alias-oracle-01/workspace/partial-alias-post-setup-access/main.pkl)

1 | handle
    ^^^^^^
at  (repl:text)
"####.encode_utf16().collect::<Vec<_>>());
    probe.start_path(2, 0, None).unwrap();
    assert_value(next(&mut probe), "TypeAlias");
    probe.start_path(3, 0, None).unwrap();
    assert_value(next(&mut probe), "child#Bad");
    probe.close();
    assert_eq!(probe.collect().slots, 0);
}
