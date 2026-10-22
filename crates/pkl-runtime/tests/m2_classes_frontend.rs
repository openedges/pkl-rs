use pkl_runtime::parser::parse_module;
use pkl_runtime::resolver::{Binding, resolve_module};
use pkl_runtime::syntax::{ExprKind, ModuleParentKind, TypeKind, Variance};

#[test]
fn classes_and_typealiases_keep_declaration_order_and_type_bindings() {
    let source = "typealias Count = Int\nopen class Item { value: Count }\nclass Container { item: Item = new Item {} }\nresult = new Container {}";
    let module = parse_module(source).unwrap();
    let resolved = resolve_module(&module).unwrap();
    assert_eq!(
        module
            .members
            .iter()
            .map(|member| member.name.as_str())
            .collect::<Vec<_>>(),
        vec!["Count", "Item", "Container", "result"]
    );
    let ExprKind::Class { members, .. } = &module.members[2].expr.kind else {
        panic!("class")
    };
    let ty = members[0].ty.as_ref().unwrap();
    assert_eq!(
        resolved.names[&ty.span],
        Binding::Property {
            owner_depth: 1,
            local: false
        }
    );
    let ExprKind::New { ty, .. } = &members[0].expr.kind else {
        panic!("typed new")
    };
    assert_eq!(
        resolved.names[&ty.span],
        Binding::Property {
            owner_depth: 1,
            local: false
        }
    );
    let ExprKind::Class { members, .. } = &module.members[1].expr.kind else {
        panic!("class")
    };
    assert!(matches!(members[0].expr.kind, ExprKind::Uninitialized));
}

#[test]
fn generic_typealias_parameters_are_scoped_and_duplicate_checked() {
    let module = parse_module("typealias Either<out A, out B> = A | B").unwrap();
    resolve_module(&module).unwrap();
    let ExprKind::TypeAlias {
        type_parameters,
        ty,
    } = &module.members[0].expr.kind
    else {
        panic!("alias")
    };
    assert_eq!(type_parameters[0].variance, Variance::Out);
    assert!(matches!(ty.kind, TypeKind::Union { .. }));
    for source in [
        "class Box<T> { value: T }",
        "function identity<T>(x: T): T = x",
    ] {
        assert_eq!(
            resolve_module(&parse_module(source).unwrap())
                .unwrap_err()
                .message,
            "Only standard library members can have type parameters."
        );
    }
    let module = parse_module("typealias Pair<T, T> = T").unwrap();
    assert_eq!(
        resolve_module(&module).unwrap_err().message,
        "Duplicate type parameter `T`."
    );
}

#[test]
fn module_headers_and_parent_modes_are_explicit() {
    let module = parse_module("open module example.config extends \"parent.pkl\"\nx = 1").unwrap();
    let declaration = module.declaration.as_ref().unwrap();
    assert_eq!(declaration.name.as_ref().unwrap(), &["example", "config"]);
    assert_eq!(
        declaration.parent.as_ref().unwrap().kind,
        ModuleParentKind::Extends
    );
    resolve_module(&module).unwrap();
    let module = parse_module("amends \"base.pkl\"\nx = 1").unwrap();
    assert!(module.declaration.unwrap().name.is_none());
    let module = parse_module("open amends \"base.pkl\"").unwrap();
    assert_eq!(
        resolve_module(&module).unwrap_err().message,
        "Modifier `open` is not applicable to modules that amend another module."
    );
}

#[test]
fn declaration_modifiers_and_duplicate_names_are_rejected_in_their_context() {
    for (source, message) in [
        (
            "hidden class C",
            "Modifier `hidden` is not applicable to classes.",
        ),
        (
            "open typealias T = Int",
            "Modifier `open` is not applicable to type aliases.",
        ),
        (
            "open function f() = 1",
            "Modifier `open` is not applicable to methods.",
        ),
        (
            "open x = 1",
            "Modifier `open` is not applicable to properties.",
        ),
        (
            "abstract open class C",
            "Modifier `open` is redundant here; just use `abstract`.",
        ),
        (
            "local hidden x = 1",
            "Modifier `hidden` is redundant here; just use `local`.",
        ),
        (
            "x = new { hidden y = 1 }",
            "Modifier `hidden` is not applicable to object members.",
        ),
        (
            "x = new { const y = 1 }",
            "Modifier `const` can only be applied to object members that are also `local`.",
        ),
        (
            "x = new { y: Int = 1 }",
            "A non-local object property cannot have a type annotation.",
        ),
    ] {
        assert_eq!(
            resolve_module(&parse_module(source).unwrap())
                .unwrap_err()
                .message,
            message,
            "{source}"
        );
    }
    for source in [
        "class C C = 1",
        "class C { x: Int x = 1 }",
        "typealias T = Int class T",
    ] {
        assert!(
            resolve_module(&parse_module(source).unwrap()).is_err(),
            "{source}"
        );
    }
}

#[test]
fn declared_types_ignore_value_bindings_in_nested_scopes() {
    let source = "class C {}\nclass Box { C = 1; value: C = let (C = 2) new C {} }\nfunction f(Int, value: Int) = value\nresult = new Box {}";
    let module = parse_module(source).unwrap();
    let resolved = resolve_module(&module).unwrap();
    let ExprKind::Class { members, .. } = &module.members[1].expr.kind else {
        panic!("class")
    };
    assert_eq!(
        resolved.names[&members[1].ty.as_ref().unwrap().span],
        Binding::Property {
            owner_depth: 1,
            local: false
        }
    );
    let ExprKind::Let { body, .. } = &members[1].expr.kind else {
        panic!("let")
    };
    let ExprKind::New { ty, .. } = &body.kind else {
        panic!("new")
    };
    assert_eq!(
        resolved.names[&ty.span],
        Binding::Property {
            owner_depth: 1,
            local: false
        }
    );
    let ExprKind::Function { parameters, .. } = &module.members[2].expr.kind else {
        panic!("method")
    };
    assert_eq!(
        resolved.names[&parameters[1].ty.as_ref().unwrap().span],
        Binding::ImplicitThis
    );
    let module = parse_module("value: one.two.Three = null").unwrap();
    assert!(
        resolve_module(&module)
            .unwrap_err()
            .message
            .starts_with("Invalid type name `one.two.Three`.\n\n")
    );
    for source in [
        "result = new { class Nested {} }",
        "class C { class Nested {} }",
    ] {
        assert!(parse_module(source).is_err());
    }
}

#[test]
fn const_requirements_are_recorded_on_reads_across_scope_barriers() {
    let source = "x = 1 const stable = 2 class C { own = 3; value = own + stable + module.x + outer.x } const result = new { inner = 1; value = inner + stable }";
    let module = parse_module(source).unwrap();
    let resolved = resolve_module(&module).unwrap();
    for fragment in ["stable +", "module.x", "outer.x"] {
        let start = source.find(fragment).unwrap();
        let end = start
            + if fragment == "stable +" {
                6
            } else {
                fragment.len()
            };
        assert!(
            resolved
                .const_names
                .contains(&pkl_runtime::source::Span::new(start, end)),
            "{fragment}"
        );
    }
    let inner = source.rfind("inner +").unwrap();
    assert!(
        !resolved
            .const_names
            .contains(&pkl_runtime::source::Span::new(inner, inner + 5))
    );
    let stable = source.rfind("stable").unwrap();
    assert!(
        resolved
            .const_names
            .contains(&pkl_runtime::source::Span::new(stable, stable + 6))
    );
    let module = parse_module("class C { value = module }").unwrap();
    assert_eq!(
        resolve_module(&module).unwrap_err().message,
        "Cannot reference `module` within a class body."
    );
    let module = parse_module("const value = this").unwrap();
    assert_eq!(
        resolve_module(&module).unwrap_err().message,
        "Cannot reference `this` from here because it is not `const`."
    );
}

#[test]
fn semantic_diagnostics_preserve_declared_module_and_identifier_spans() {
    let error = parse_module("module example.named\nresult = 9223372036854775808").unwrap_err();
    assert_eq!(error.module_name.as_deref(), Some("example.named"));
    assert_eq!(
        error
            .member
            .as_deref()
            .and_then(pkl_runtime::value::PklString::as_str),
        Some("result")
    );
    let source = "module example.named\nresult = 1\nlocal result = 2";
    let error = resolve_module(&parse_module(source).unwrap()).unwrap_err();
    assert_eq!(error.message, "Duplicate definition of member `result`.");
    assert_eq!(&source[error.span.start..error.span.end], "result");
    assert_eq!(error.module_name.as_deref(), Some("example.named"));
}

#[test]
fn const_super_reads_respect_property_and_method_scope_barriers() {
    let source = "open class Parent { value = 1; function method() = 1 } class Child extends Parent { const result = super.value; const function calculate() = super.method() }";
    let resolved = resolve_module(&parse_module(source).unwrap()).unwrap();
    for (fragment, required) in [("super.value", true), ("super.method", false)] {
        let start = source.find(fragment).unwrap();
        assert_eq!(
            resolved
                .const_names
                .contains(&pkl_runtime::source::Span::new(
                    start,
                    start + fragment.len()
                )),
            required,
            "A const method creates a lexical const barrier; a const property does not."
        );
    }
}

#[test]
fn repeated_modifiers_follow_upstream_flags_and_static_locality_rules() {
    let module = parse_module("open open class C {}\nabstract abstract class D {}\nlocal local value = 1\nconst const stable = 2").unwrap();
    assert!(
        module.members[0].local,
        "two open flags produce runtime class locality"
    );
    assert!(
        module.members[2].local,
        "property lexical locality retains the syntactic local token"
    );
    resolve_module(&module).unwrap();
    let module = parse_module("abstract abstract value: Int?\nresult = value").unwrap();
    resolve_module(&module).unwrap();
    let source = "const const value = new { prop = this }";
    resolve_module(&parse_module(source).unwrap()).unwrap();
    // Carrying repeated flags must never panic when upstream's diagnostic
    // requests a modifier token absent from the original declaration.
    let source = format!("result = new {{ {} value = 1 }}", "local ".repeat(16));
    assert!(resolve_module(&parse_module(&source).unwrap()).is_err());
}

#[test]
fn unused_nonabstract_methods_still_require_a_body() {
    for source in [
        "function value(): Int\nresult = 42",
        "abstract abstract function value(): Int\nresult = 42",
    ] {
        let error = resolve_module(&parse_module(source).unwrap()).unwrap_err();
        assert_eq!(error.message, "Method `value` is missing a body.");
        assert_eq!(
            error
                .member
                .as_deref()
                .and_then(pkl_runtime::value::PklString::as_str),
            Some("value")
        );
        assert_eq!(
            &source[error.span.start..error.span.end],
            source.lines().next().unwrap()
        );
    }
    resolve_module(
        &parse_module("abstract class C { abstract function value(): Int } result = 42").unwrap(),
    )
    .unwrap();
}
