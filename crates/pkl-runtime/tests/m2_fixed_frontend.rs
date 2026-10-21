//! Fixed declaration syntax and diagnostics checked against pinned Pkl 0.32 sources.
use pkl_runtime::evaluation::scalar_expression;
use pkl_runtime::parser::parse_module;
use pkl_runtime::resolver::resolve_module;
use pkl_runtime::source::Span;
use pkl_runtime::syntax::{ExprKind, ModifierKind, modifier_flags};

#[test]
fn inherited_fixed_properties_keep_authored_modifier_and_type_spans() {
    let source = "open class Base { hidden fixed value: String = throw(\"deferred\") }\nclass Child extends Base { hidden fixed value: String = \"child\" }";
    let module = parse_module(source).unwrap();
    resolve_module(&module).unwrap();
    for class in &module.members {
        let ExprKind::Class { members, .. } = &class.expr.kind else {
            panic!("class")
        };
        let property = &members[0];
        assert!(!property.local);
        assert_eq!(
            modifier_flags(&property.modifiers),
            ModifierKind::Hidden.flag() | ModifierKind::Fixed.flag()
        );
        assert_eq!(
            &source
                [property.ty.as_ref().unwrap().span.start..property.ty.as_ref().unwrap().span.end],
            "String"
        );
        assert_eq!(
            &source[property.modifiers[1].span.start..property.modifiers[1].span.end],
            "fixed"
        );
    }
}

#[test]
fn fixed_modifier_rejections_keep_the_invalid_or_redundant_token() {
    for (source, target) in [
        ("fixed module example", "modules"),
        ("fixed class C", "classes"),
        ("fixed typealias T = Int", "type aliases"),
        ("fixed function f() = 1", "methods"),
        ("result = new { fixed value = 1 }", "object members"),
        ("result = new Listing { fixed value = 1 }", "object members"),
    ] {
        let error = resolve_module(&parse_module(source).unwrap()).unwrap_err();
        let start = source.find("fixed").unwrap();
        assert_eq!(error.span, Span::new(start, start + 5), "{source}");
        assert_eq!(
            error.message,
            format!("Modifier `fixed` is not applicable to {target}."),
            "{source}"
        );
    }
    for source in ["local fixed value = 1", "fixed local value = 1"] {
        let error = resolve_module(&parse_module(source).unwrap()).unwrap_err();
        let start = source.find("fixed").unwrap();
        assert_eq!(error.span, Span::new(start, start + 5));
        assert_eq!(
            error.message,
            "Modifier `fixed` is redundant here; just use `local`."
        );
    }
}

#[test]
fn abstract_fixed_body_diagnostics_use_only_the_header_and_declaration_frame() {
    // Complete Java captures: target/m2/stage11-platform-java-01.
    for (source, expected) in [
        (
            "abstract class C { abstract fixed value: Int = 42 }\nresult = 42\n",
            "–– Pkl Error ––\nAbstract members cannot have a body.\n\n1 | abstract class C { abstract fixed value: Int = 42 }\n                       ^^^^^^^^^^^^^^^^^^^^^^^^^\nat m2-platform#C.value (file:///m2-platform.pkl)\n",
        ),
        (
            "abstract class C { abstract fixed value = 42 }\nresult = 42\n",
            "–– Pkl Error ––\nAbstract members cannot have a body.\n\n1 | abstract class C { abstract fixed value = 42 }\n                       ^^^^^^^^^^^^^^^^^^^^\nat m2-platform#C.value (file:///m2-platform.pkl)\n",
        ),
    ] {
        assert_eq!(
            scalar_expression(source, "file:///m2-platform.pkl", "m2-platform", "result")
                .unwrap_err(),
            expected
        );
    }
}

#[test]
fn repeated_fixed_tokens_preserve_the_upstream_const_carry_and_const_barrier() {
    // AstBuilder adds flags, so two fixed tokens carry into const. Reading this
    // member's nonconstant neighbor must still be rejected by ordinary dispatch.
    let source = "open class C { fixed fixed value: Int = other; other: Int = 42 }\nresult = new C {}.value\n";
    let module = parse_module(source).unwrap();
    let resolved = resolve_module(&module).unwrap();
    let other = source.find("other;").unwrap();
    assert!(resolved.const_names.contains(&Span::new(other, other + 5)));
    let error =
        scalar_expression(source, "file:///m2-platform.pkl", "m2-platform", "result").unwrap_err();
    assert_eq!(
        error,
        "–– Pkl Error ––\nCannot reference property `other` from here because it is not `const`.\n\n1 | open class C { fixed fixed value: Int = other; other: Int = 42 }\n                                            ^^^^^\nat m2-platform#C.value (file:///m2-platform.pkl)\n\nTo fix, do either of:\n1. Add modifier `const` to property `other`\n2. Self-import this module, and reference this property from the import.\n\n2 | result = new C {}.value\n             ^^^^^^^^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
    );
}

#[test]
fn abstract_fixed_amendments_are_distinct_from_assignment_bodies() {
    let source = "open class Base { fixed value = new Dynamic { x = 1 } }\nabstract class Child extends Base { abstract fixed value { x = 2 } }\nclass GrandChild extends Child\nresult = new GrandChild {}.value.x\n";
    assert_eq!(
        scalar_expression(source, "file:///m2-platform.pkl", "m2-platform", "result"),
        Ok("2".to_owned())
    );
    let source = "open class Base { fixed value = new Dynamic { x = 1 } }\nabstract class Child extends Base { abstract fixed value = (super.value) { x = 2 } }\nresult = 42\n";
    let error = resolve_module(&parse_module(source).unwrap()).unwrap_err();
    assert_eq!(error.message, "Abstract members cannot have a body.");
    let start = source.find("abstract fixed").unwrap();
    assert_eq!(
        error.span,
        Span::new(start, start + "abstract fixed value".len())
    );
    assert_eq!(error.member.as_deref(), Some("Child.value"));
}

#[test]
fn fixed_modifier_validation_runs_in_the_enclosing_declaration_scope() {
    for (source, owner) in [
        (
            "abstract class A { local fixed p: Int = 1 }\nresult = A\n",
            "A",
        ),
        ("result = new Dynamic { fixed p = 1 }\n", "result"),
        ("result = new Listing { fixed p = 1 }\n", "result"),
        (
            "class A { value = new Dynamic { fixed p = 1 } }\n",
            "A.value",
        ),
    ] {
        let error = resolve_module(&parse_module(source).unwrap()).unwrap_err();
        assert_eq!(error.member.as_deref(), Some(owner), "{source}");
        let start = source.find("fixed").unwrap();
        assert_eq!(error.span, Span::new(start, start + 5));
    }
}

#[test]
fn override_modifier_diagnostics_locate_header_substrings_after_duplicate_flag_carry() {
    // UnresolvedPropertyNode uses headerSource.indexOf rather than a modifier
    // token. A carried const flag can therefore underline an identifier/comment.
    for (source, expected) in [
        ("class C { fixed fixed constant: Int = 1 }", "const"),
        ("class C { fixed /*const*/ fixed value: Int = 1 }", "const"),
        ("class C { fixed fixed value: Int = 1 }", " fixe"),
    ] {
        let module = parse_module(source).unwrap();
        let ExprKind::Class { members, .. } = &module.members[0].expr.kind else {
            panic!("class")
        };
        let span = members[0].override_const_span.unwrap();
        assert_eq!(&source[span.start..span.end], expected);
        let fixed = members[0].override_fixed_span.unwrap();
        assert_eq!(&source[fixed.start..fixed.end], "fixed");
    }
}
