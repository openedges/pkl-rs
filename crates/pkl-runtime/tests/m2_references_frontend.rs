//! Import identity and type/name scope prerequisites for deferred references.
use pkl_runtime::parser::parse_module;
use pkl_runtime::resolver::{Binding, resolve_module};
use pkl_runtime::source::Span;
use pkl_runtime::syntax::{ExprKind, ModifierKind, TypeKind, modifier_flags};

fn span(source: &str, fragment: &str) -> Span {
    let start = source.find(fragment).unwrap();
    Span::new(start, start + fragment.len())
}

// Type names resolve during owner initialization so earlier declarations can
// publish or fail before a later unresolved name is reached.
fn initialize_error(source: &str) -> pkl_runtime::value::RuntimeError {
    use pkl_runtime::vm::{Vm, VmPoll, compile_module};
    let program = compile_module(&resolve_module(&parse_module(source).unwrap()).unwrap()).unwrap();
    let mut vm = Vm::new(program.clone());
    vm.start(program.entry, vec![]).unwrap();
    for _ in 0..1000 {
        match vm.poll(100) {
            VmPoll::Ready(result) => return result.unwrap_err(),
            VmPoll::Yielded => {}
            VmPoll::Pending(_) => panic!("unexpected Host request"),
        }
    }
    panic!("type initialization instruction budget exceeded")
}

#[test]
fn imports_are_local_const_members_with_authored_namespace_and_source_identity() {
    let source = "module example\nimport \"pkl:ref\" as deferred\nresult = deferred.Reference";
    let module = parse_module(source).unwrap();
    let import = &module.members[0];
    assert_eq!(import.name, "deferred");
    assert!(import.local);
    assert_eq!(
        modifier_flags(&import.modifiers),
        ModifierKind::Local.flag() | ModifierKind::Const.flag()
    );
    assert!(matches!(&import.expr.kind, ExprKind::Import { uri } if uri == "pkl:ref"));
    assert_eq!(
        import.expr.span,
        span(source, "import \"pkl:ref\" as deferred")
    );
    let resolved = resolve_module(&module).unwrap();
    let receiver = span(source, "deferred.Reference");
    assert_eq!(
        resolved.names[&Span::new(receiver.start, receiver.start + 8)],
        Binding::Property {
            owner_depth: 0,
            local: true
        }
    );
}

#[test]
fn qualified_type_names_keep_token_spans_and_resolve_imports_outside_value_scopes() {
    let source = "import \"pkl:ref\"\nclass D extends ref.Domain {}\nfunction check(ref, value: ref . `Reference`<D, String>) = ref";
    let module = parse_module(source).unwrap();
    let resolved = resolve_module(&module).unwrap();
    let ExprKind::Function {
        parameters, body, ..
    } = &module.members[2].expr.kind
    else {
        panic!("method")
    };
    let ty = parameters[1].ty.as_ref().unwrap();
    let TypeKind::Named {
        path,
        name_spans,
        arguments,
    } = &ty.kind
    else {
        panic!("named type")
    };
    assert_eq!(path, &["ref", "Reference"]);
    assert_eq!(&source[name_spans[0].start..name_spans[0].end], "ref");
    assert_eq!(
        &source[name_spans[1].start..name_spans[1].end],
        "`Reference`"
    );
    assert_eq!(arguments.len(), 2);
    assert_eq!(
        resolved.names[&ty.span],
        Binding::Property {
            owner_depth: 0,
            local: true
        }
    );
    assert_eq!(
        resolved.names[&body.span],
        Binding::Lexical { depth: 1, slot: 0 }
    );
}

#[test]
fn base_classes_are_values_after_lexical_shadowing_and_reference_is_not_global() {
    let source =
        "import \"pkl:ref\"\nfunction f(String) = String\nresult = List(Int, Any, Reference)";
    let module = parse_module(source).unwrap();
    let resolved = resolve_module(&module).unwrap();
    assert_eq!(resolved.names[&span(source, "Int")], Binding::BaseClass);
    assert_eq!(resolved.names[&span(source, "Any")], Binding::BaseClass);
    assert_eq!(
        resolved.names[&span(source, "Reference")],
        Binding::ImplicitThis
    );
    let ExprKind::Function { body, .. } = &module.members[1].expr.kind else {
        panic!("method")
    };
    assert_eq!(
        resolved.names[&body.span],
        Binding::Lexical { depth: 0, slot: 0 }
    );
}

#[test]
fn nonnull_alias_value_keeps_its_declaration_kind_and_value_shadowing() {
    let source = "aliasValue = NonNull\nfunction f(NonNull) = NonNull";
    let module = parse_module(source).unwrap();
    let resolved = resolve_module(&module).unwrap();
    assert_eq!(
        resolved.names[&span(source, "NonNull")],
        Binding::BaseTypeAlias
    );
    let ExprKind::Function { body, .. } = &module.members[1].expr.kind else {
        panic!("method")
    };
    assert_eq!(
        resolved.names[&body.span],
        Binding::Lexical { depth: 0, slot: 0 }
    );
}

#[test]
fn imports_share_the_property_namespace_but_keep_methods_separate() {
    for source in [
        "import \"pkl:ref\"\nref = 1",
        "import \"pkl:ref\"\nlocal ref = 1",
        "import \"pkl:ref\"\nimport \"pkl:ref\"",
    ] {
        assert_eq!(
            resolve_module(&parse_module(source).unwrap())
                .unwrap_err()
                .message,
            "Duplicate definition of member `ref`."
        );
    }
    resolve_module(
        &parse_module("import \"pkl:ref\"\nfunction ref() = 1\nresult = ref()").unwrap(),
    )
    .unwrap();
    let misplaced = "result = 1\nimport \"pkl:ref\"";
    let error = parse_module(misplaced).unwrap_err();
    assert_eq!(
        error.message,
        "Keyword `import` is not allowed here.\n\nIf you must use this name as identifier, enclose it in backticks."
    );
    assert_eq!(error.span, span(misplaced, "import"));
    assert!(parse_module("import* \"pkl:ref\"").is_err());
    let source = "import \"pkl:ref\"\nlocal ref = 1";
    assert_eq!(
        resolve_module(&parse_module(source).unwrap())
            .unwrap_err()
            .span,
        span(source, "local ref")
    );
}

#[test]
fn generic_reference_constraint_validation_keeps_alias_bodies_and_the_declaration_name() {
    let source = "import \"pkl:ref\"\ntypealias Ref<T> = ref.Reference<ref.Domain, T>\ntypealias Ref2<T> = Ref<T(isEmpty)>\nfoo: Ref2<String>";
    let error = initialize_error(source);
    assert_eq!(
        error.message,
        "`Reference` referent type argument may not include type constraints."
    );
    assert_eq!(
        error
            .frames
            .iter()
            .map(|frame| (frame.member.as_ref(), frame.span))
            .collect::<Vec<_>>(),
        vec![
            ("#Ref", span(source, "ref.Reference<ref.Domain, T>")),
            ("#Ref2", span(source, "Ref<T(isEmpty)>")),
            ("", {
                let usage = span(source, "Ref2<String>");
                Span::new(usage.start, usage.start + 4)
            }),
        ]
    );
    assert!(error.frames.iter().all(|frame| frame.source.is_none()));
}

#[test]
fn constraints_authored_in_alias_arguments_keep_the_complete_use_site_span() {
    let source = "import \"pkl:ref\"\ntypealias Ref<T> = ref.Reference<ref.Domain, T>\ntypealias Ref2<T> = Ref<T>\nfoo: Ref2<String(isEmpty)>";
    let error = initialize_error(source);
    assert_eq!(
        error.frames.last().unwrap().span,
        span(source, "Ref2<String(isEmpty)>")
    );
}

#[test]
fn nested_raw_interpolations_keep_full_delimiters_separate_from_expression_spans() {
    let source = r###"result = #"outer \#("inner \(42)")"#"###;
    let module = parse_module(source).unwrap();
    let spans = module
        .interpolation_spans
        .values()
        .map(|span| &source[span.start..span.end])
        .collect::<Vec<_>>();
    assert_eq!(spans.len(), 2);
    assert!(spans.contains(&r##"\#("inner \(42)")"##));
    assert!(spans.contains(&r"\(42)"));
    assert_eq!(
        module.interpolation_spans[&span(source, "42")],
        span(source, r"\(42)")
    );
}

#[test]
fn direct_reference_constraint_validation_omits_only_the_outer_referent_alias_frame() {
    let source = "import \"pkl:ref\"\ntypealias First = Second\ntypealias Second = String(isEmpty)\nfoo: ref.Reference<ref.Domain, First>";
    let error = initialize_error(source);
    assert_eq!(
        error
            .frames
            .iter()
            .map(|frame| (frame.member.as_ref(), frame.span))
            .collect::<Vec<_>>(),
        vec![
            ("#Second", span(source, "String(isEmpty)")),
            ("", span(source, "ref.Reference<ref.Domain, First>")),
        ]
    );
}

#[test]
fn parenthesized_non_type_constructor_reports_the_authored_name_when_executed() {
    let source = "local r = 42\nresult = new (r) {}\n";
    let error = pkl_runtime::evaluation::scalar_expression(
        source,
        "file:///constructor.pkl",
        "constructor",
        "result",
    )
    .unwrap_err();
    assert_eq!(
        error,
        "–– Pkl Error ––\nExpected `r` to be a type, but it is not.\n\n2 | result = new (r) {}\n                  ^\nat constructor#result (file:///constructor.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n"
    );
    let source = "local r = 42\nresult = if (false) new (r) {} else 42\n";
    assert_eq!(
        pkl_runtime::evaluation::scalar_expression(
            source,
            "file:///constructor.pkl",
            "constructor",
            "result"
        )
        .unwrap(),
        "42"
    );
}
