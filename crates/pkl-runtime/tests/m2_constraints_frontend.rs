//! Binding regressions for pinned Pkl custom-this scopes, separate from runtime evidence.
use pkl_runtime::parser::parse_module;
use pkl_runtime::resolver::{Binding, resolve_module};
use pkl_runtime::source::Span;
use pkl_runtime::syntax::{ExprKind, TypeKind};

fn fragment_span(source: &str, fragment: &str, offset: usize, length: usize) -> Span {
    let start = source.find(fragment).unwrap() + offset;
    Span::new(start, start + length)
}

#[test]
fn constraint_input_is_distinct_from_lexical_properties_and_implicit_members() {
    let source = "limit = 4 local function helper() = true result: Int(this > limit && missing == 1 && helper() && allowed()) = 5";
    let resolved = resolve_module(&parse_module(source).unwrap()).unwrap();
    assert!(
        resolved
            .custom_this
            .contains(&fragment_span(source, "this >", 0, 4))
    );
    assert_eq!(
        resolved.names[&fragment_span(source, "> limit", 2, 5)],
        Binding::Property {
            owner_depth: 0,
            local: false
        }
    );
    assert_eq!(
        resolved.names[&fragment_span(source, "missing ==", 0, 7)],
        Binding::CustomThis
    );
    assert_eq!(
        resolved.names[&fragment_span(source, "helper() &&", 0, 6)],
        Binding::Method {
            owner_depth: 0,
            local: true
        }
    );
    assert_eq!(
        resolved.names[&fragment_span(source, "allowed()", 0, 7)],
        Binding::CustomMethod
    );
}

#[test]
fn parameter_and_return_constraints_see_all_function_parameters() {
    let source = "function check(other: Int, value: Int(this > other)): Int(this > other) = value";
    let resolved = resolve_module(&parse_module(source).unwrap()).unwrap();
    assert_eq!(resolved.custom_this.len(), 2);
    for (start, _) in source.match_indices("other)") {
        assert_eq!(
            resolved.names[&Span::new(start, start + 5)],
            Binding::Lexical { depth: 1, slot: 0 }
        );
    }
    let start = source.rfind("value").unwrap();
    assert_eq!(
        resolved.names[&Span::new(start, start + 5)],
        Binding::Lexical { depth: 0, slot: 0 }
    );
}

#[test]
fn lambda_capture_and_nested_objects_do_not_reclassify_ordinary_this() {
    let source = "result: Int(let (n = 1) (ignored) -> this > n && (new Dynamic { own = this; f = (x) -> this }).own != null) = 5";
    let module = parse_module(source).unwrap();
    let resolved = resolve_module(&module).unwrap();
    let TypeKind::Constrained { constraints, .. } = &module.members[0].ty.as_ref().unwrap().kind
    else {
        panic!("constraint")
    };
    let ExprKind::Let { body, .. } = &constraints[0].kind else {
        panic!("let")
    };
    assert!(resolved.custom_functions.contains(&body.span));
    assert_eq!(resolved.custom_functions.len(), 1);
    assert!(
        resolved.custom_this.is_empty(),
        "lambda this is its ordinary captured receiver; object this is the object"
    );
    assert_eq!(
        resolved.names[&fragment_span(source, "> n &&", 2, 1)],
        Binding::Lexical { depth: 1, slot: 0 }
    );
}

#[test]
fn direct_constraint_member_receivers_and_let_body_keep_custom_this() {
    let source = "const result: Int(let (x = this) this.value > x) = 5";
    let resolved = resolve_module(&parse_module(source).unwrap()).unwrap();
    assert_eq!(resolved.custom_this.len(), 2);
    assert!(
        resolved
            .custom_this
            .contains(&fragment_span(source, "this.value", 0, 4))
    );
    let member = fragment_span(source, "this.value", 0, 10);
    assert!(
        !resolved.const_names.contains(&member),
        "custom input does not require a const object property"
    );
}

#[test]
fn only_the_first_lambda_in_a_custom_scope_captures_custom_this_directly() {
    let module = parse_module("result: Int((a) -> (b) -> this == 5) = 5").unwrap();
    let resolved = resolve_module(&module).unwrap();
    let TypeKind::Constrained { constraints, .. } = &module.members[0].ty.as_ref().unwrap().kind
    else {
        panic!("constraint")
    };
    let ExprKind::Function { body, .. } = &constraints[0].kind else {
        panic!("function")
    };
    assert!(resolved.custom_functions.contains(&constraints[0].span));
    assert!(!resolved.custom_functions.contains(&body.span));
    assert!(resolved.custom_this.is_empty());
}

#[test]
fn typed_let_constraint_does_not_see_the_binding_being_declared() {
    let source = "x = 3 result = let (x: Int(this > x) = 5) x";
    let resolved = resolve_module(&parse_module(source).unwrap()).unwrap();
    assert_eq!(
        resolved.names[&fragment_span(source, "> x)", 2, 1)],
        Binding::Property {
            owner_depth: 0,
            local: false
        }
    );
    let start = source.rfind('x').unwrap();
    assert_eq!(
        resolved.names[&Span::new(start, start + 1)],
        Binding::Lexical { depth: 0, slot: 0 }
    );
}
