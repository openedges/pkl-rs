//! Structural and binding regressions against the pinned parser's grammar.
//! These tests do not establish Java/Rust evaluation byte compatibility.
use pkl_runtime::diagnostic::DiagnosticKind;
use pkl_runtime::parser::{parse_expression, parse_module};
use pkl_runtime::resolver::{Binding, resolve_module};
use pkl_runtime::source::Span;
use pkl_runtime::syntax::{BinaryOp, ExprKind, TypeKind, UnaryOp};

#[test]
fn operator_associativity_and_unary_precedence_follow_pkl() {
    let expression = parse_expression("-2 ** 3 ** 2").unwrap();
    let ExprKind::Binary {
        op: BinaryOp::Power,
        left,
        right,
    } = expression.kind
    else {
        panic!("expected power")
    };
    assert_eq!(left.kind, ExprKind::Int(-2));
    assert!(matches!(
        right.kind,
        ExprKind::Binary {
            op: BinaryOp::Power,
            ..
        }
    ));
    let expression = parse_expression("!flag || 1 + 2 * 3 < 8 && true == false ?? null").unwrap();
    let ExprKind::Binary {
        op: BinaryOp::NullCoalesce,
        left,
        ..
    } = expression.kind
    else {
        panic!("expected coalesce")
    };
    let ExprKind::Binary {
        op: BinaryOp::Or,
        left,
        right,
    } = left.kind
    else {
        panic!("expected or")
    };
    assert!(matches!(
        left.kind,
        ExprKind::Unary {
            op: UnaryOp::Not,
            ..
        }
    ));
    assert!(matches!(
        right.kind,
        ExprKind::Binary {
            op: BinaryOp::And,
            ..
        }
    ));
    let expression = parse_expression("a ?? b ?? c").unwrap();
    let ExprKind::Binary { right, .. } = expression.kind else {
        panic!("coalesce")
    };
    assert!(matches!(
        right.kind,
        ExprKind::Binary {
            op: BinaryOp::NullCoalesce,
            ..
        }
    ));
}

#[test]
fn numeric_lexemes_and_ranges_are_preserved_without_float_rounding() {
    for (source, expected) in [
        ("0x7fff_ffff_ffff_ffff", i64::MAX),
        ("-0x8000000000000000", i64::MIN),
        ("0o777", 511),
        ("0b101_010", 42),
        ("1__2_", 12),
    ] {
        assert_eq!(
            parse_expression(source).unwrap().kind,
            ExprKind::Int(expected)
        );
    }
    for source in [
        "0x8000000000000000",
        "-9223372036854775809",
        "0b_1",
        "1._2",
        "1e_2",
    ] {
        assert!(parse_expression(source).is_err(), "{source}");
    }
    for source in [".5", "5e-1", "0.5", "0.5_0e+0"] {
        assert_eq!(parse_expression(source).unwrap().kind, ExprKind::Float(0.5));
    }
    assert_eq!(parse_expression("null").unwrap().kind, ExprKind::Null);
    assert_eq!(parse_expression("!null").unwrap().span, Span::new(0, 5));
}

#[test]
fn methods_and_properties_have_distinct_lexical_namespaces() {
    let source = "f = 1 function f(x) = x + f result = f(2)";
    let module = parse_module(source).unwrap();
    let resolved = resolve_module(&module).unwrap();
    let property = source.find("+ f").unwrap() + 2;
    let method = source.find("f(2)").unwrap();
    assert_eq!(
        resolved.names[&Span::new(property, property + 1)],
        Binding::Property {
            owner_depth: 0,
            local: false
        }
    );
    assert_eq!(
        resolved.names[&Span::new(method, method + 1)],
        Binding::Method {
            owner_depth: 0,
            local: false
        }
    );
    let duplicate = parse_module("function f() = 1 function f(x) = x").unwrap();
    assert_eq!(
        resolve_module(&duplicate).unwrap_err().kind,
        DiagnosticKind::DuplicateMember
    );
}

#[test]
fn closures_bind_parameter_slots_and_enclosing_lets_statically() {
    let source = "result = let (z = 7) (x, y) -> (q) -> x + y + z + q";
    let module = parse_module(source).unwrap();
    let resolved = resolve_module(&module).unwrap();
    for (name, depth) in [("x", 2), ("y", 1), ("z", 3), ("q", 0)] {
        let start = source.rfind(name).unwrap();
        assert_eq!(
            resolved.names[&Span::new(start, start + 1)],
            Binding::Lexical { depth, slot: 0 }
        );
    }
    let duplicate = parse_module("result = (x, x) -> x").unwrap();
    assert_eq!(
        resolve_module(&duplicate).unwrap_err().kind,
        DiagnosticKind::DuplicateMember
    );
    resolve_module(&parse_module("result = (_, _) -> 1").unwrap()).unwrap();
}

#[test]
fn typed_methods_and_function_types_keep_source_information() {
    let source = "function f(x: Int, callback: (Int) -> String): String? = if (x > 0) callback.apply(x) else null";
    let module = parse_module(source).unwrap();
    assert_eq!(module.members[0].span, Span::new(0, source.len()));
    let ExprKind::Function {
        parameters,
        return_type,
        body,
        method,
        ..
    } = &module.members[0].expr.kind
    else {
        panic!("method")
    };
    assert!(*method);
    assert!(matches!(
        parameters[1].ty.as_ref().unwrap().kind,
        TypeKind::Function { .. }
    ));
    assert!(matches!(
        return_type.as_ref().unwrap().kind,
        TypeKind::Nullable(_)
    ));
    assert!(matches!(body.kind, ExprKind::If { .. }));
    let module = parse_module("value: List<Map<String, Int?>> | *Null = null").unwrap();
    assert!(matches!(
        module.members[0].ty.as_ref().unwrap().kind,
        TypeKind::Union {
            default: Some(1),
            ..
        }
    ));
    assert!(parse_module("value: *Int = 1").is_err());
    assert!(parse_module("value: *Int | *String = 1").is_err());
}

#[test]
fn comments_and_quoted_identifiers_do_not_change_binding_names() {
    let source =
        "/* heading */ `if` = 1 function `class`(`let`: Int) = `let` result = `class`(`if`)";
    let module = parse_module(source).unwrap();
    resolve_module(&module).unwrap();
    assert_eq!(module.members[0].name, "if");
    assert_eq!(module.members[1].name, "class");
    assert!(parse_module("x = 1 /* missing").is_err());
}

#[test]
fn raw_and_multiline_strings_decode_before_evaluation() {
    for (source, expected) in [
        (r##"#"a\n\#n"#"##, "a\\n\n"),
        (r###"##"a\#n\##n"##"###, "a\\#n\n"),
        (r#""\u{1f600}\u{41}""#, "😀A"),
        ("\"\"\"\n  first\n  second\n  \"\"\"", "first\nsecond"),
        ("\"\"\"\r\n\tfirst\r\n\tsecond\r\n\t\"\"\"", "first\nsecond"),
        ("\"\"\"\n  a\\\n  b\n  \"\"\"", "ab"),
        ("\"\"\"\n  \"\"\"", ""),
    ] {
        assert_eq!(
            parse_expression(source).unwrap().kind,
            ExprKind::String(expected.to_owned()),
            "{source}"
        );
    }
    for source in [
        "\"\"\"text\n\"\"\"",
        "\"\"\"\n a\n  \"\"\"",
        "\"\"\"\n  a\"\"\"",
        "\"line\\\ncontinued\"",
    ] {
        assert!(parse_expression(source).is_err(), "{source}");
    }
}

#[test]
fn interpolation_retains_global_source_spans_and_bindings() {
    let source = r#"result = let (x = 3) "value: \(x + 1), \("nested \(x)")""#;
    let module = parse_module(source).unwrap();
    let resolved = resolve_module(&module).unwrap();
    for (start, _) in source.match_indices("x") {
        if source[start + 1..].starts_with(" =") {
            continue;
        }
        assert_eq!(
            resolved.names[&Span::new(start, start + 1)],
            Binding::Lexical { depth: 0, slot: 0 }
        );
    }
    let ExprKind::Let { body, .. } = &module.members[0].expr.kind else {
        panic!("let")
    };
    assert!(matches!(body.kind, ExprKind::InterpolatedString(_)));
}

#[test]
fn type_operators_nonnull_nullable_access_and_context_receivers_are_structural() {
    let expression =
        parse_expression("null?.value.member!!.field is Int || outer.x == module.y").unwrap();
    assert!(matches!(
        expression.kind,
        ExprKind::Binary {
            op: BinaryOp::Or,
            ..
        }
    ));
    assert!(matches!(
        parse_expression("x as String?").unwrap().kind,
        ExprKind::TypeCast { .. }
    ));
    assert!(matches!(
        parse_expression("throw(\"failure\")").unwrap().kind,
        ExprKind::Throw(_)
    ));
    assert!(parse_expression("((x) -> x)(1)").is_err());
    assert!(parse_expression("1(2)").is_err());
    assert!(parse_expression("((x) -> x).apply(1)").is_ok());
}

#[test]
fn method_header_span_excludes_assignment_and_body() {
    let source = "local function calculate(\n x: Int, y: Int\n): Int   = x + y";
    let module = parse_module(source).unwrap();
    let ExprKind::Function { header_span, .. } = module.members[0].expr.kind else {
        panic!("method")
    };
    assert_eq!(
        &source[header_span.start..header_span.end],
        "local function calculate(\n x: Int, y: Int\n): Int"
    );
}

#[test]
fn expression_context_selects_the_pinned_expected_token_message() {
    for token in [",", "}", ")"] {
        let source = format!("result = if (true) {token} else 1");
        let error = parse_module(&source).unwrap_err();
        assert_eq!(
            error.message,
            format!("Unexpected token `{token}`. Expected `else`.")
        );
        assert_eq!(error.span, Span::new(19, 20));
    }
    for token in [",", "}"] {
        let source = format!("function f(x) = x result = f({token})");
        assert_eq!(
            parse_module(&source).unwrap_err().message,
            format!("Unexpected token `{token}`.")
        );
    }
    assert_eq!(
        parse_module("result = ((x) -> x)(1)").unwrap_err().message,
        "Invalid token at position. Expected a class, typealias, method, or property."
    );
}

#[test]
fn object_methods_need_local_and_lambda_arity_is_bounded_by_the_language() {
    let module = parse_module("result = new { function f() = 1 }").unwrap();
    let error = resolve_module(&module).unwrap_err();
    assert_eq!(
        error.message,
        "Method needs a `local` modifier because it is defined in an object, not a class."
    );
    assert_eq!(error.span, Span::new(15, 27));
    resolve_module(&parse_module("result = new { local function f() = 1 }").unwrap()).unwrap();
    let module = parse_module("result = (a, b, c, d, e, f) -> a").unwrap();
    assert_eq!(
        resolve_module(&module).unwrap_err().message,
        "Function literals can have at most five parameters."
    );
    resolve_module(&parse_module("function f(a, b, c, d, e, f) = a").unwrap()).unwrap();
}

#[test]
fn let_parameters_accept_types_and_discard_bindings() {
    let source = "result = let (x: Int = 4) let (_: Int = x) x";
    let module = parse_module(source).unwrap();
    resolve_module(&module).unwrap();
    let ExprKind::Let { ty, body, .. } = &module.members[0].expr.kind else {
        panic!("let")
    };
    assert!(matches!(ty.as_ref().unwrap().kind, TypeKind::Named { .. }));
    let ExprKind::Let { name, .. } = &body.kind else {
        panic!("discard let")
    };
    assert_eq!(name, "_");
}

#[test]
fn interpolation_syntax_precedes_later_bad_tokens_and_keeps_closing_paren() {
    for (source, token) in [(r#"result = "\()""#, ")"), (r#"result = "\(, @)""#, ",")] {
        let error = parse_module(source).unwrap_err();
        assert_eq!(
            error.message,
            format!("Unexpected token `{token}`. Expected `)`.")
        );
        assert_eq!(&source[error.span.start..error.span.end], token);
    }
}

#[test]
fn identifiers_use_the_pinned_java_unicode_predicates() {
    let source = "이름 = 1 π = 2 `𐐀` = 3 a\u{301} = 4 $value = 5 a\u{200c}b = 6 result = 이름 + π + `𐐀` + a\u{301} + $value + a\u{200c}b";
    let module = parse_module(source).unwrap();
    resolve_module(&module).unwrap();
    assert_eq!(module.members[0].name, "이름");
    assert_eq!(module.members[2].name, "𐐀");
    assert_eq!(module.members[3].name, "a\u{301}");
    assert_eq!(module.members[5].name, "a\u{200c}b");
    for source in ["\u{301} = 1", "€ = 1", "😀 = 1", "𐐀 = 1"] {
        assert!(parse_module(source).is_err(), "{source}");
    }
    assert_eq!(parse_module("`€` = 1").unwrap().members[0].name, "€");
}

#[test]
fn numeric_validation_waits_for_syntax_and_unary_postfix_precedence() {
    let error = parse_module("x = 9223372036854775808\ny = )").unwrap_err();
    assert_eq!(error.message, "Unexpected token `)`.");
    let error = parse_module("result = -0X10").unwrap_err();
    assert_eq!(error.message, "Int literal `0X10` is too large.");
    assert_eq!(error.span, Span::new(10, 14));
    assert_eq!(error.member.as_deref(), Some("result"));
    let expression = parse_expression("-1.toString()").unwrap();
    assert!(
        matches!(expression.kind, ExprKind::Unary { op: UnaryOp::Negate, operand } if matches!(operand.kind, ExprKind::Call { .. }))
    );
    let error = parse_expression("-9223372036854775808.member").unwrap_err();
    assert_eq!(error.span, Span::new(1, 20));
    assert_eq!(
        parse_expression("-9223372036854775808").unwrap().kind,
        ExprKind::Int(i64::MIN)
    );
    let error = parse_module("value = (1\rnext = 2").unwrap_err();
    assert_eq!(error.span, Span::new(10, 10));
    assert_eq!(error.highlight_length_utf16, Some(1));
}

#[test]
fn type_only_keywords_in_object_bodies_preserve_expression_expectation() {
    for keyword in ["unknown", "nothing"] {
        let source = format!("result = new C {{ {keyword} = 1 }}");
        let error = parse_module(&source).unwrap_err();
        assert_eq!(
            error.message,
            format!("Unexpected token `{keyword}`. Expected `}}`.")
        );
        assert_eq!(&source[error.span.start..error.span.end], keyword);
    }
}
