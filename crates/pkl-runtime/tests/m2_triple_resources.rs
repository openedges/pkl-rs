//! Shared ModuleKey preparation and ResourceManager boundaries. Actual File
//! oracle captures remain separate from these typed Host regressions.
use pkl_runtime::{
    modules::{ModuleCompletion, ModuleHostError},
    probe::{SourceEvent, SourceProbe},
    resources::ReadResult,
};
fn probe(source: &str, expressions: &[&str]) -> SourceProbe {
    SourceProbe::new_expressions_text(
        source,
        "file:///one/two/main.pkl",
        "main",
        &expressions.iter().map(|s| (*s).into()).collect::<Vec<_>>(),
        "compact",
        false,
        false,
        false,
    )
    .unwrap()
}
fn next(p: &mut SourceProbe) -> SourceEvent {
    for _ in 0..100_000 {
        let e = p.poll(0, 1).unwrap();
        p.collect();
        if e.kind != "yield" {
            return e;
        }
    }
    panic!("did not settle")
}
fn capabilities(p: &mut SourceProbe) {
    let e = next(p);
    assert_eq!(e.kind, "module-request");
    assert_eq!(e.module.unwrap().kind, "capabilities");
    p.complete_module_capabilities(e.token.as_deref().unwrap(), true, true, false, 0)
        .unwrap();
}
fn element(p: &mut SourceProbe, expected: &str, exists: bool) {
    let e = next(p);
    assert_eq!(e.kind, "module-request");
    let m = e.module.unwrap();
    assert_eq!(m.kind, "has-element");
    assert_eq!(m.original_uri, expected);
    p.complete_module_has_element(e.token.as_deref().unwrap(), exists, 0)
        .unwrap();
}
fn value(e: SourceEvent) -> String {
    assert_eq!(e.kind, "value", "{e:?}");
    String::from_utf16(&e.value.unwrap()).unwrap()
}

#[test]
fn module_key_search_precedes_every_resource_cache_lookup() {
    let mut p = probe(
        "a=read(\".../resource.txt\")\nb=read?(\".../resource.txt\")",
        &["a", "b"],
    );
    for index in 0..2 {
        p.start_path(index, 0, None).unwrap();
        capabilities(&mut p);
        element(&mut p, "file:///one/resource.txt", true);
        if index == 0 {
            let e = next(&mut p);
            assert_eq!(e.kind, "read");
            let t = e.token.unwrap();
            assert_eq!(p.resolved_read_uri(&t).unwrap(), "file:///one/resource.txt");
            p.complete_read(&t, Ok(ReadResult::Text("cached".into())), 0)
                .unwrap();
        }
        assert_eq!(value(next(&mut p)), "cached");
    }
    p.close();
    assert_eq!(p.collect().slots, 0);
}

#[test]
fn nullable_search_missing_is_an_error_before_any_resource_read() {
    let mut p = probe("a=read?(\".../missing\")", &["a"]);
    p.start_path(0, 0, None).unwrap();
    capabilities(&mut p);
    element(&mut p, "file:///one/missing", false);
    element(&mut p, "file:///missing", false);
    let e = next(&mut p);
    assert_eq!(e.kind, "error");
    assert!(
        String::from_utf16(e.error.as_ref().unwrap())
            .unwrap()
            .contains("Cannot find resource `.../missing`.")
    );
}

#[test]
fn unused_import_and_nested_method_are_prepared_in_astbuilder_order() {
    let mut p = probe(
        "function unused() = import(\".../method.pkl\")\nx=import(\".../property.pkl\")\nstable=7",
        &["stable"],
    );
    p.start_path(0, 0, None).unwrap();
    capabilities(&mut p);
    // Java visits class/module property bodies before method bodies.
    element(&mut p, "file:///one/property.pkl", true);
    capabilities(&mut p);
    element(&mut p, "file:///one/method.pkl", true);
    assert_eq!(value(next(&mut p)), "7");
}

#[test]
fn eager_missing_unused_import_is_retained_on_repeated_module_calls() {
    let mut p = probe(
        "unused=import(\".../missing.pkl\")\nlazy=read(\"res:/unused\")\nstable=7",
        &["stable", "lazy"],
    );
    p.start_path(0, 0, None).unwrap();
    capabilities(&mut p);
    element(&mut p, "file:///one/missing.pkl", false);
    element(&mut p, "file:///missing.pkl", false);
    let first = next(&mut p);
    assert_eq!(first.kind, "error");
    p.start_path(1, 0, None).unwrap();
    let second = next(&mut p);
    assert_eq!(second.error, first.error);
}

#[test]
fn unrequested_query_ast_does_not_run_import_preparation() {
    let mut p = probe("stable=7", &["stable", "import(\".../future.pkl\")"]);
    p.start_path(0, 0, None).unwrap();
    assert_eq!(value(next(&mut p)), "7");
    p.start_path(1, 0, None).unwrap();
    let e = next(&mut p);
    assert_eq!(e.module.unwrap().kind, "capabilities");
    p.complete_module_capabilities(e.token.as_deref().unwrap(), false, false, false, 0)
        .unwrap();
    assert_eq!(next(&mut p).kind, "error");
}

#[test]
fn module_key_security_denial_precedes_cached_read_and_invalid_completion_keeps_token() {
    let mut p = probe(
        "a=read(\"file:///one/item\")\nb=read(\".../item\")",
        &["a", "b"],
    );
    p.start_path(0, 0, None).unwrap();
    let e = next(&mut p);
    p.complete_read(
        e.token.as_deref().unwrap(),
        Ok(ReadResult::Text("cached".into())),
        0,
    )
    .unwrap();
    assert_eq!(value(next(&mut p)), "cached");
    p.start_path(1, 0, None).unwrap();
    capabilities(&mut p);
    let e = next(&mut p);
    let token = e.token.unwrap();
    assert!(
        p.complete_module(
            &token,
            ModuleCompletion::Source(Err(ModuleHostError::NotFound)),
            0
        )
        .is_err()
    );
    p.complete_module(
        &token,
        ModuleCompletion::HasElement(Err(ModuleHostError::SecurityManagerException {
            message: Some("typed ModuleKey denial".into()),
        })),
        0,
    )
    .unwrap();
    let error = next(&mut p);
    assert_eq!(error.kind, "error");
    assert!(
        String::from_utf16(error.error.as_ref().unwrap())
            .unwrap()
            .contains("typed ModuleKey denial")
    );
}

#[test]
fn deferred_ast_validation_runs_between_module_key_effects() {
    let overflow = "9223372036854775808";
    let mut before = probe(
        &format!("early={overflow}\nunused=import(\".../child.pkl\")\nstable=7"),
        &["stable"],
    );
    before.start_path(0, 0, None).unwrap();
    let e = next(&mut before);
    assert_eq!(e.kind, "error");
    assert!(
        String::from_utf16(e.error.as_ref().unwrap())
            .unwrap()
            .contains("Int literal `9223372036854775808` is too large.")
    );
    let mut after = probe(
        &format!("unused=import(\".../child.pkl\")\nlate={overflow}\nstable=7"),
        &["stable"],
    );
    after.start_path(0, 0, None).unwrap();
    capabilities(&mut after);
    element(&mut after, "file:///one/child.pkl", true);
    let e = next(&mut after);
    assert_eq!(e.kind, "error");
    assert!(
        String::from_utf16(e.error.as_ref().unwrap())
            .unwrap()
            .contains("Int literal `9223372036854775808` is too large.")
    );
}

#[test]
fn generator_build_visits_branches_before_predicate_and_revisits_prefix() {
    let mut p = probe(
        "o = new { prefix=import(\".../prefix.pkl\"); when (import(\".../predicate.pkl\").ok) { thenValue=import(\".../then.pkl\") } else { elseValue=import(\".../else.pkl\") } }\nstable=7",
        &["stable"],
    );
    p.start_path(0, 0, None).unwrap();
    for uri in [
        "file:///one/prefix.pkl",
        "file:///one/prefix.pkl",
        "file:///one/then.pkl",
        "file:///one/else.pkl",
        "file:///one/predicate.pkl",
    ] {
        capabilities(&mut p);
        element(&mut p, uri, true);
    }
    assert_eq!(value(next(&mut p)), "7");
}

#[test]
fn static_validation_stops_at_the_actual_member_and_argument_phase() {
    for source in [
        "x=Map(import(\".../first.pkl\"))\nafter=import(\".../after.pkl\")\nstable=7",
        "x=import(\".../first.pkl\")\nx=import(\".../second.pkl\")\nafter=import(\".../after.pkl\")\nstable=7",
        "x=new Dynamic { a=import(\".../first.pkl\"); a=import(\".../second.pkl\") }\nafter=import(\".../after.pkl\")\nstable=7",
    ] {
        let mut p = probe(source, &["stable", "stable"]);
        p.start_path(0, 0, None).unwrap();
        capabilities(&mut p);
        element(&mut p, "file:///one/first.pkl", true);
        if !source.contains("Map(") {
            capabilities(&mut p);
            element(&mut p, "file:///one/second.pkl", true);
        }
        let failure = next(&mut p);
        assert_eq!(failure.kind, "error", "{failure:?}");
        assert_eq!(failure.disposition, Some("language-error"));
        let text = String::from_utf16(failure.error.as_ref().unwrap()).unwrap();
        assert!(
            text.contains(if source.contains("Map(") {
                "Number of `Map()` arguments must be a multiple of two"
            } else {
                "Duplicate definition of member"
            }),
            "{text}"
        );
        p.start_path(1, 0, None).unwrap();
        assert_eq!(next(&mut p).error, failure.error);
        p.close();
        assert_eq!(p.collect().slots, 0);
    }
}

#[test]
fn short_circuit_ast_construction_visits_right_before_left() {
    for operator in ["&&", "||", "??"] {
        let mut p = probe(
            &format!(
                "unused=import(\".../left.pkl\") {operator} import(\".../right.pkl\")\nstable=7"
            ),
            &["stable"],
        );
        p.start_path(0, 0, None).unwrap();
        capabilities(&mut p);
        element(&mut p, "file:///one/right.pkl", true);
        capabilities(&mut p);
        element(&mut p, "file:///one/left.pkl", true);
        assert_eq!(value(next(&mut p)), "7");
    }
}

#[test]
fn typed_resource_security_is_guest_failure_and_preparation_phase_errors_keep_token() {
    let mut p = probe(
        "import \"pkl:test\"\nx=test.catch(() -> read(\"res:/denied\"))\ny=read(\".../item\")",
        &["x", "y"],
    );
    p.start_path(0, 0, None).unwrap();
    let e = next(&mut p);
    assert_eq!(e.kind, "read");
    p.reject_read_security_manager(e.token.as_deref().unwrap(), Some("typed denial".into()), 0)
        .unwrap();
    assert_eq!(value(next(&mut p)), "typed denial");
    p.start_path(1, 0, None).unwrap();
    let e = next(&mut p);
    assert_eq!(e.module.as_ref().unwrap().kind, "capabilities");
    let token = e.token.unwrap();
    assert!(
        p.reject_module_preparation_text(&token, true, "io", Some("wrong phase".into()), 0)
            .is_err()
    );
    assert!(
        p.reject_module_preparation_text(
            &token,
            false,
            "security-manager-exception",
            Some("unproven getter denial".into()),
            0
        )
        .is_err()
    );
    p.complete_module_capabilities(&token, true, true, false, 0)
        .unwrap();
    element(&mut p, "file:///one/item", true);
    let e = next(&mut p);
    p.complete_read_text(e.token.as_deref().unwrap(), "ok".into(), 0)
        .unwrap();
    assert_eq!(value(next(&mut p)), "ok");
}

#[test]
fn duplicate_function_parameter_names_do_not_skip_ast_imports() {
    for source in [
        "unused=(x,x)->import(\".../body.pkl\")\nstable=7",
        "function unused(x,x)=import(\".../body.pkl\")\nstable=7",
    ] {
        let mut p = probe(source, &["stable"]);
        p.start_path(0, 0, None).unwrap();
        capabilities(&mut p);
        element(&mut p, "file:///one/body.pkl", false);
        element(&mut p, "file:///body.pkl", false);
        assert_eq!(next(&mut p).kind, "error");
    }
}

#[test]
fn unsupported_constructor_parent_value_remains_lazy_after_ast_effects() {
    let mut p = probe(
        "bad=new Int {}\nunused=import(\".../body.pkl\")\nstable=7",
        &["stable", "bad"],
    );
    p.start_path(0, 0, None).unwrap();
    capabilities(&mut p);
    element(&mut p, "file:///one/body.pkl", true);
    assert_eq!(value(next(&mut p)), "7");
    p.start_path(1, 0, None).unwrap();
    assert_eq!(next(&mut p).kind, "error");
}
