//! Ordered syntax and lexical scope regressions checked against pinned builder rules.
use pkl_runtime::parser::parse_module;
use pkl_runtime::resolver::{Binding, resolve_module};
use pkl_runtime::source::Span;
use pkl_runtime::syntax::{ExprKind, ObjectMember};

fn span(source: &str, fragment: &str) -> Span {
    let start = source.find(fragment).unwrap();
    Span::new(start, start + fragment.len())
}

#[test]
fn mixed_body_preserves_authored_order_and_spans() {
    let source = "result = new Dynamic { first = 1; 2; [\"key\"] = 3; when (true) { 4 } else { 5 }; for (i, v in List(6)) { [i] = v }; ...List(7); ...?null; [[this == 1]] = 8 }";
    let module = parse_module(source).unwrap();
    let ExprKind::New { members, .. } = &module.members[0].expr.kind else {
        panic!("new")
    };
    assert_eq!(members.len(), 8);
    assert!(matches!(&members[0], ObjectMember::Property(member) if member.name == "first"));
    assert!(matches!(members[1], ObjectMember::Element { .. }));
    assert!(matches!(members[2], ObjectMember::Entry { .. }));
    assert!(matches!(members[3], ObjectMember::When { .. }));
    assert!(matches!(members[4], ObjectMember::For { .. }));
    assert!(matches!(
        members[5],
        ObjectMember::Spread {
            nullable: false,
            ..
        }
    ));
    assert!(matches!(
        members[6],
        ObjectMember::Spread { nullable: true, .. }
    ));
    assert!(matches!(members[7], ObjectMember::Predicate { .. }));
    assert_eq!(members[2].span(), span(source, "[\"key\"] = 3"));
    assert_eq!(members[6].span(), span(source, "...?null"));
    assert!(
        members
            .windows(2)
            .all(|pair| pair[0].span().end < pair[1].span().start)
    );
}

#[test]
fn eager_headers_skip_the_body_but_lazy_values_keep_its_receiver() {
    let source =
        "x = 10 result = new Dynamic { x = 20; [x] = x; when (x > 0) { flag = x }; ...List(x) }";
    let resolved = resolve_module(&parse_module(source).unwrap()).unwrap();
    let key = span(source, "[x]");
    let value = span(source, "= x;");
    let condition = span(source, "x > 0");
    let flag = span(source, "flag = x");
    let spread = span(source, "List(x)");
    for start in [key.start + 1, condition.start, spread.end - 2] {
        assert_eq!(
            resolved.names[&Span::new(start, start + 1)],
            Binding::Property {
                owner_depth: 0,
                local: false,
            }
        );
    }
    for start in [value.start + 2, flag.end - 1] {
        assert_eq!(
            resolved.names[&Span::new(start, start + 1)],
            Binding::Property {
                owner_depth: 0,
                local: false,
            }
        );
    }
    // A distinct enclosing name distinguishes a skipped scope from accidental
    // resolution against the body's own declaration.
    let source = "key = 1 result = new Dynamic { own = 2; [key] = key }";
    let resolved = resolve_module(&parse_module(source).unwrap()).unwrap();
    let key = span(source, "[key]");
    assert_eq!(
        resolved.names[&Span::new(key.start + 1, key.end - 1)],
        Binding::Property {
            owner_depth: 0,
            local: false,
        }
    );
    let last = source.rfind("key").unwrap();
    assert_eq!(
        resolved.names[&Span::new(last, last + 3)],
        Binding::Property {
            owner_depth: 1,
            local: false,
        }
    );
}

#[test]
fn nested_generators_and_closures_keep_iteration_captures() {
    let source = "result = new Listing { for (i, value in List(1)) { for (inner in List(value)) { [i] = let (copy = value) (arg) -> copy + inner + i + arg } } }";
    let resolved = resolve_module(&parse_module(source).unwrap()).unwrap();
    let call = span(source, "List(value)");
    assert_eq!(
        resolved.names[&Span::new(call.start + 5, call.end - 1)],
        Binding::Lexical { depth: 0, slot: 0 }
    );
    let key = span(source, "[i]");
    assert_eq!(
        resolved.names[&Span::new(key.start + 1, key.end - 1)],
        Binding::Lexical { depth: 2, slot: 0 }
    );
    for (fragment, depth) in [("copy +", 1), ("inner +", 2), ("i + arg", 4), ("arg }", 0)] {
        let start = span(source, fragment).start;
        let length = fragment.split_whitespace().next().unwrap().len();
        assert_eq!(
            resolved.names[&Span::new(start, start + length)],
            Binding::Lexical { depth, slot: 0 }
        );
    }
}

#[test]
fn conditional_properties_are_lexically_visible_across_branches() {
    let source = "result = new Dynamic { when (true) { selected = other } else { other = 2 }; answer = selected }";
    let resolved = resolve_module(&parse_module(source).unwrap()).unwrap();
    let other = span(source, "other }");
    let selected = span(source, "= selected");
    for target in [
        Span::new(other.start, other.start + 5),
        Span::new(selected.start + 2, selected.end),
    ] {
        assert_eq!(
            resolved.names[&target],
            Binding::Property {
                owner_depth: 0,
                local: false
            }
        );
    }
}

#[test]
fn predicate_custom_this_does_not_escape_into_values_or_nested_objects() {
    let source = "limit = 1 result = new Dynamic { [[this > limit && missing]] = (new Dynamic { own = this }).own }";
    let resolved = resolve_module(&parse_module(source).unwrap()).unwrap();
    assert_eq!(resolved.custom_this.len(), 1);
    assert!(resolved.custom_this.contains(&span(source, "this")));
    assert_eq!(
        resolved.names[&span(source, "missing")],
        Binding::CustomThis
    );
    let limit = span(source, "limit &&").start;
    assert_eq!(
        resolved.names[&Span::new(limit, limit + 5)],
        Binding::Property {
            owner_depth: 0,
            local: false
        }
    );
}

#[test]
fn entry_amendments_use_the_inherited_runtime_key() {
    let source = "result = new Mapping { [\"key\"] { first = 1 } { second = 2 } }";
    let module = parse_module(source).unwrap();
    let ExprKind::New { members, .. } = &module.members[0].expr.kind else {
        panic!("new")
    };
    let ObjectMember::Entry { value, .. } = &members[0] else {
        panic!("entry")
    };
    let ExprKind::Object {
        parent: Some(first),
        ..
    } = &value.kind
    else {
        panic!("second body")
    };
    let ExprKind::Object {
        parent: Some(parent),
        ..
    } = &first.kind
    else {
        panic!("first body")
    };
    assert_eq!(parent.kind, ExprKind::EntryParent);
    assert_eq!(parent.span, span(source, "[\"key\"]"));
    resolve_module(&module).unwrap();
}

#[test]
fn invalid_generator_declarations_keep_keyword_and_parameter_spans() {
    let source = "result = new Dynamic { for (i, i in List(1)) { 1 } }";
    let error = resolve_module(&parse_module(source).unwrap()).unwrap_err();
    let duplicate = span(source, "i in").start;
    assert_eq!(error.span, Span::new(duplicate, duplicate + 1));
    assert_eq!(error.message, "Duplicate definition of member `i`.");
    let source = "result = new Dynamic { for (i in List(1)) { when (true) { invalid = i } } }";
    let error = resolve_module(&parse_module(source).unwrap()).unwrap_err();
    assert_eq!(error.span, span(source, "for"));
    assert_eq!(
        error.message,
        "A for-generator cannot generate object properties (only entries and elements)."
    );
}

#[test]
fn subscript_is_a_postfix_expression_and_a_newline_starts_an_entry() {
    let source = "result = new Dynamic { List(1)[0]\n[\"key\"] = super[\"key\"] }";
    let module = parse_module(source).unwrap();
    let ExprKind::New { members, .. } = &module.members[0].expr.kind else {
        panic!("new")
    };
    assert_eq!(members.len(), 2);
    let ObjectMember::Element { value, .. } = &members[0] else {
        panic!("element")
    };
    assert!(matches!(value.kind, ExprKind::Subscript { .. }));
    let ObjectMember::Entry { value, .. } = &members[1] else {
        panic!("entry")
    };
    assert!(
        matches!(&value.kind, ExprKind::Subscript { receiver, .. } if receiver.kind == ExprKind::Super)
    );
    resolve_module(&module).unwrap();
}

#[test]
fn predicate_closing_delimiter_requires_adjacent_brackets() {
    let source = "result = new Dynamic { [[true] ] = 1 }";
    let error = parse_module(source).unwrap_err();
    let close = span(source, "] ]").start;
    assert_eq!(error.span, Span::new(close, close + 1));
    assert_eq!(error.message, "Unexpected token `] ]`. Expected `]]`.");
}
