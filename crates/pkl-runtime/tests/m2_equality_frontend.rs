//! Structural keys retain authored syntax and the enclosing generator scope.
use pkl_runtime::diagnostic::DiagnosticKind;
use pkl_runtime::parser::parse_module;
use pkl_runtime::resolver::{Binding, resolve_module};
use pkl_runtime::source::Span;
use pkl_runtime::syntax::{BinaryOp, ExprKind, ObjectMember};
use pkl_runtime::value::{ErrorKind, RuntimeError, Value};
use pkl_runtime::vm::{Vm, VmPoll, compile_module};

#[test]
fn nested_structural_key_keeps_generator_locals_in_its_own_object_scope() {
    let source = "result = new Mapping { for (value in List(7)) { [new Dynamic { nested = new Dynamic { captured = value } }] = value } }";
    let module = parse_module(source).unwrap();
    let resolved = resolve_module(&module).unwrap();
    let key_value = source.find("value }").unwrap();
    let body_value = source.rfind("value }").unwrap();
    for position in [key_value, body_value] {
        assert_eq!(
            resolved.names[&Span::new(position, position + 5)],
            Binding::Lexical { depth: 0, slot: 0 }
        );
    }
    let ExprKind::New { members, .. } = &module.members[0].expr.kind else {
        panic!("mapping")
    };
    let ObjectMember::For { members, .. } = &members[0] else {
        panic!("generator")
    };
    let ObjectMember::Entry { key, value, .. } = &members[0] else {
        panic!("entry")
    };
    assert_eq!(
        &source[key.span.start..key.span.end],
        "new Dynamic { nested = new Dynamic { captured = value } }"
    );
    assert_eq!(&source[value.span.start..value.span.end], "value");
    assert!(resolved.custom_this.is_empty());
}

#[test]
fn equality_and_subscript_preserve_nested_object_key_and_operator_spans() {
    let source =
        "result = (new Mapping { [new Dynamic { x = 1 }] = 2 })[new Dynamic { x = 1 }] == 2";
    let module = parse_module(source).unwrap();
    resolve_module(&module).unwrap();
    let ExprKind::Binary { op, left, right } = &module.members[0].expr.kind else {
        panic!("equality")
    };
    assert_eq!(*op, BinaryOp::Equal);
    assert_eq!(&source[right.span.start..right.span.end], "2");
    let ExprKind::Subscript { receiver, key } = &left.kind else {
        panic!("subscript")
    };
    assert_eq!(
        &source[key.span.start..key.span.end],
        "new Dynamic { x = 1 }"
    );
    let ExprKind::New { members, .. } = &receiver.unparenthesized().kind else {
        panic!("mapping")
    };
    let ObjectMember::Entry { key: authored, .. } = &members[0] else {
        panic!("entry")
    };
    assert_eq!(
        &source[authored.span.start..authored.span.end],
        "new Dynamic { x = 1 }"
    );
    assert_ne!(key.span, authored.span);
}

#[test]
fn constant_list_duplicates_are_rejected_even_in_unused_properties() {
    for key in ["List(1)", "List(List(1), null, true)", "List(-1)"] {
        let source = format!("unused = new Mapping {{ [{key}] = 1; [{key}] = 2 }}\nresult = 42");
        let error = resolve_module(&parse_module(&source).unwrap()).unwrap_err();
        assert_eq!(error.kind, DiagnosticKind::DuplicateMember);
        assert_eq!(
            error.message,
            format!("Duplicate definition of member `{key}`.")
        );
        let start = source.rfind(key).unwrap();
        assert_eq!(error.span, Span::new(start, start + key.len()));
        assert_eq!(
            error
                .member
                .as_deref()
                .and_then(pkl_runtime::value::PklString::as_str),
            Some("unused")
        );
    }
}

#[test]
fn a_dynamic_key_or_generator_defers_constant_duplicate_detection() {
    // Pinned Java's AST builder selects a dynamic body for the entire object.
    // Resolution must leave the duplicate for construction at evaluation time.
    for source in [
        "key = List(2)\nm = new Mapping { [List(1)] = 1; [List(1)] = 2; [key] = 3 }\nresult = m.length",
        "m = new Mapping { [List(1)] = 1; [List(1)] = 2; when(false) { [List(2)] = 3 } }\nresult = m.length",
    ] {
        let module = parse_module(source).unwrap();
        let resolved = resolve_module(&module).unwrap();
        let program = compile_module(&resolved).unwrap();
        let mut vm = Vm::new(program.clone());
        vm.start(program.entry, vec![]).unwrap();
        let root = complete(&mut vm).unwrap();
        vm.start_member(root.as_object().unwrap(), "result")
            .unwrap();
        let error = complete(&mut vm).unwrap_err();
        assert_eq!(error.kind, ErrorKind::Type);
        assert_eq!(error.message, "Duplicate definition of member `List(1)`.");
    }
}

fn complete(vm: &mut Vm) -> Result<Value, RuntimeError> {
    loop {
        match vm.poll(100) {
            VmPoll::Ready(value) => return value,
            VmPoll::Yielded => {}
            VmPoll::Pending(request) => panic!("unexpected Host read {request:?}"),
        }
    }
}

#[test]
fn constant_list_keys_preserve_boxed_number_distinctions() {
    for keys in [["List(1)", "List(1.0)"], ["List(0.0)", "List(-0.0)"]] {
        let source = format!("m = new Mapping {{ [{}] = 1; [{}] = 2 }}", keys[0], keys[1]);
        resolve_module(&parse_module(&source).unwrap()).unwrap();
    }
}

#[test]
fn shadowed_list_method_is_not_a_constant_collection_constructor() {
    let source =
        "function List(x) = x + 1\nm = new Mapping { [List(1)] = 1; [List(1)] = 2 }\nresult = 42";
    let module = parse_module(source).unwrap();
    let resolved = resolve_module(&module).unwrap();
    let first = source.find("[List(").unwrap() + 1;
    let second = source.rfind("[List(").unwrap() + 1;
    for start in [first, second] {
        assert_eq!(
            resolved.names[&Span::new(start, start + 4)],
            Binding::Method {
                owner_depth: 0,
                local: false
            }
        );
    }
}

#[test]
fn negative_constant_keys_keep_expression_spans_and_report_numeric_token_spans() {
    // The pinned Java observations underline the unsigned token even though
    // parsing folds the signed integer. Interpolation uses a nested parser.
    let mut cases = ["- 1", "-0x1", "-0"]
        .map(|negative| {
            (
                format!("m = new Mapping {{ [{negative}] = 1; [{negative}] = 2 }}"),
                negative,
            )
        })
        .to_vec();
    cases.push((
        r#"result = "value \(let (m = new Mapping { [-0x1] = 1; [-0x1] = 2 }) m.length)""#.into(),
        "-0x1",
    ));
    for (source, negative) in cases {
        let module = parse_module(&source).unwrap();
        assert_eq!(module.folded_integer_spans.len(), 2);
        let token_text = negative.trim_start_matches('-').trim_start();
        for (expression, token) in &module.folded_integer_spans {
            assert_eq!(&source[expression.start..expression.end], negative);
            assert_eq!(&source[token.start..token.end], token_text);
            assert!(expression.start < token.start);
            assert_eq!(expression.end, token.end);
        }
        let error = resolve_module(&module).unwrap_err();
        assert_eq!(error.kind, DiagnosticKind::DuplicateMember);
        assert_eq!(
            error.span,
            *module.folded_integer_spans.last_key_value().unwrap().1
        );
        assert_eq!(
            error.message,
            format!(
                "Duplicate definition of member `{}`.",
                if negative == "-0" { "0" } else { "-1" }
            )
        );
    }
}
