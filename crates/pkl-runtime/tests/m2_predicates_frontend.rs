//! Predicate scopes and authored spans from the pinned Pkl 0.32 builder rules.
use pkl_runtime::parser::parse_module;
use pkl_runtime::resolver::{Binding, resolve_module};
use pkl_runtime::source::Span;
use pkl_runtime::syntax::{ExprKind, ObjectMember};

fn span(source: &str, fragment: &str) -> Span {
    let start = source.find(fragment).unwrap();
    Span::new(start, start + fragment.len())
}

#[test]
fn predicate_amendment_chain_retains_condition_and_inherited_key_spans() {
    let source = "result = (source) { [[this.name == \"owl\"]] { age = 1 } { age = 2 } }";
    let module = parse_module(source).unwrap();
    let ExprKind::Object { members, .. } = &module.members[0].expr.kind else {
        panic!("amendment")
    };
    let ObjectMember::Predicate {
        condition,
        value,
        span: member_span,
    } = &members[0]
    else {
        panic!("predicate")
    };
    assert_eq!(condition.span, span(source, "this.name == \"owl\""));
    assert_eq!(
        *member_span,
        span(source, "[[this.name == \"owl\"]] { age = 1 } { age = 2 }")
    );
    let ExprKind::Object {
        parent: Some(first),
        ..
    } = &value.kind
    else {
        panic!("second amendment")
    };
    let ExprKind::Object {
        parent: Some(inherited),
        ..
    } = &first.kind
    else {
        panic!("first amendment")
    };
    assert_eq!(inherited.kind, ExprKind::EntryParent);
    assert_eq!(inherited.span, span(source, "[[this.name == \"owl\"]]"));
    resolve_module(&module).unwrap();
}

#[test]
fn condition_skips_target_declarations_but_keeps_surrounding_properties() {
    let source = "limit = 1 result = new Dynamic { limit = 2; local function allowed() = false; [[this > limit && absent && allowed()]] = limit }";
    let resolved = resolve_module(&parse_module(source).unwrap()).unwrap();
    let condition_limit = span(source, "limit &&").start;
    assert_eq!(
        resolved.names[&Span::new(condition_limit, condition_limit + 5)],
        Binding::Property {
            owner_depth: 0,
            local: false,
        }
    );
    assert_eq!(resolved.names[&span(source, "absent")], Binding::CustomThis);
    let allowed = source.rfind("allowed()").unwrap();
    assert_eq!(
        resolved.names[&Span::new(allowed, allowed + 7)],
        Binding::CustomMethod
    );
    let replacement = source.rfind("limit").unwrap();
    assert_eq!(
        resolved.names[&Span::new(replacement, replacement + 5)],
        Binding::Property {
            owner_depth: 0,
            local: false,
        }
    );
    assert_eq!(resolved.custom_this.len(), 1);
}

#[test]
fn condition_let_and_closure_preserve_for_bindings_and_capture_custom_this_once() {
    let source = "result = new Listing { for (key, value in List(1)) { [[let (copy = this) (arg) -> (() -> this == copy && value == key + arg).apply()]] = (replacement) -> this } }";
    let module = parse_module(source).unwrap();
    let resolved = resolve_module(&module).unwrap();
    assert_eq!(resolved.custom_this.len(), 1);
    assert_eq!(resolved.custom_functions.len(), 1);
    let ExprKind::New { members, .. } = &module.members[0].expr.kind else {
        panic!("listing")
    };
    let ObjectMember::For { members, .. } = &members[0] else {
        panic!("for")
    };
    let ObjectMember::Predicate {
        condition, value, ..
    } = &members[0]
    else {
        panic!("predicate")
    };
    let ExprKind::Let { body, .. } = &condition.kind else {
        panic!("let")
    };
    assert!(resolved.custom_functions.contains(&body.span));
    assert!(!resolved.custom_functions.contains(&value.span));
    for (fragment, length, depth) in [
        ("copy &&", 4, 1),
        ("value ==", 5, 2),
        ("key +", 3, 3),
        ("arg).apply", 3, 0),
    ] {
        let start = span(source, fragment).start;
        assert_eq!(
            resolved.names[&Span::new(start, start + length)],
            Binding::Lexical { depth, slot: 0 }
        );
    }
}

#[test]
fn nested_predicate_restores_the_enclosing_custom_scope() {
    let source =
        "result = (source) { [[this == (other) { [[this == 1]] = this } && this == 2]] = this }";
    let resolved = resolve_module(&parse_module(source).unwrap()).unwrap();
    assert_eq!(resolved.custom_this.len(), 3);
    let nested_value = span(source, "= this }").start + 2;
    let outer_value = source.rfind("this").unwrap();
    for start in [nested_value, outer_value] {
        assert!(!resolved.custom_this.contains(&Span::new(start, start + 4)));
    }
}

#[test]
fn predicate_values_do_not_infer_a_parent_from_the_matched_entry() {
    for source in [
        "result = (source) { [[true]] = new {} }",
        "result = (source) { [[true]] = if (true) new {} else new {} }",
        "result = (source) { [[true]] = let (ignored = 1) new {} }",
    ] {
        let error = resolve_module(&parse_module(source).unwrap()).unwrap_err();
        assert_eq!(error.span, span(source, "new"));
        assert_eq!(error.message, "Cannot tell which parent to amend.");
        assert_eq!(
            error
                .member
                .as_deref()
                .and_then(pkl_runtime::value::PklString::as_str),
            Some(if source.contains("let (") {
                "result[true].<let expr>"
            } else {
                "result[true]"
            })
        );
    }
}

#[test]
fn dynamic_predicate_body_diagnostics_keep_authored_entry_numbering() {
    let source = "result = (source) { 1; [key] = 2; [[this == 1]] = new {} }";
    let error = resolve_module(&parse_module(source).unwrap()).unwrap_err();
    assert_eq!(
        error
            .member
            .as_deref()
            .and_then(pkl_runtime::value::PklString::as_str),
        Some("result[#3]")
    );
}

#[test]
fn condition_validation_precedes_replacement_validation() {
    let source = "result = (source) { [[(condition, condition) -> true]] = (replacement, replacement) -> true }";
    let error = resolve_module(&parse_module(source).unwrap()).unwrap_err();
    let start = source.find("condition)").unwrap();
    assert_eq!(error.span, Span::new(start, start + 9));
    assert_eq!(error.message, "Duplicate parameter `condition`.");
}

#[test]
fn eager_predicate_lookup_preserves_the_skipped_const_barrier() {
    // Pinned Java accepts the direct non-const lexical read but rejects that
    // same read inside a lambda. The target const object remains a barrier
    // even though eager lexical lookup cannot see its declarations.
    let source = "x = 1 const p = new Listing { 1 } const q = (p) { [[x == 1 && (() -> x == 1).apply()]] = 2 }";
    let resolved = resolve_module(&parse_module(source).unwrap()).unwrap();
    let direct = span(source, "x == 1 &&").start;
    let closure = span(source, "x == 1).apply").start;
    assert!(
        !resolved
            .const_names
            .contains(&Span::new(direct, direct + 1))
    );
    assert!(
        resolved
            .const_names
            .contains(&Span::new(closure, closure + 1))
    );
}
