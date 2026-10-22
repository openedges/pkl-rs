use pkl_runtime::diagnostic::DiagnosticKind;
use pkl_runtime::parser::{parse_expression, parse_module};
use pkl_runtime::resolver::{Binding, resolve_module};
use pkl_runtime::source::{SourceMap, Span};
use pkl_runtime::syntax::ExprKind;

#[test]
fn unicode_coordinates_are_not_interchangeable() {
    let mut sources = SourceMap::default();
    let id = sources.add("unicode", "a😀e\u{301}\r\nz");
    let source = sources.get(id).unwrap();
    let after_astral = source.position(5).unwrap();
    assert_eq!((after_astral.utf16, after_astral.codepoint), (3, 2));
    assert_eq!(source.position(2), None);
    assert_eq!(source.byte_from_utf16(2), None);
    let after_crlf = source.position(10).unwrap();
    assert_eq!((after_crlf.line, after_crlf.column_utf16), (2, 1));
    assert_eq!(source.utf16_span(Span::new(1, 8)), Some((1, 4)));
    assert_eq!(source.slice(Span::new(1, 5)), Some("😀"));
}

#[test]
fn bounded_parser_preserves_precedence_and_rejects_trailing_input() {
    let expression = parse_expression("1 + 2 * 3").unwrap();
    let ExprKind::Binary { right, .. } = &expression.kind else {
        panic!("expected addition")
    };
    assert!(matches!(right.kind, ExprKind::Binary { .. }));
    assert!(parse_expression("1 2").is_err());
    assert!(parse_module("x = 1\n- 2").is_err());
    assert!(parse_module("local hidden = 1").is_err());
    assert!(parse_expression("new { x = 1").is_err());
    assert_eq!(
        parse_expression("-9223372036854775808").unwrap().kind,
        ExprKind::Int(i64::MIN)
    );
    assert!(parse_expression("9223372036854775808").is_err());
    assert_eq!(
        parse_expression("[1]").unwrap_err().kind,
        DiagnosticKind::Unsupported
    );
}

#[test]
fn first_syntax_error_precedes_a_later_invalid_character() {
    let error = parse_module("x = )\ny = @").unwrap_err();
    assert_eq!(error.span, Span::new(4, 5));
    assert_eq!(error.message, "Unexpected token `)`.");
    assert_eq!(error.kind, DiagnosticKind::Syntax);
}

#[test]
fn static_binding_distinguishes_owner_from_receiver_and_let() {
    let text = "x = 1 local privateValue = 2 obj = new { a = x b = privateValue c = missing d = this.x e = super.x f = let (x = 3) x }";
    let module = parse_module(text).unwrap();
    let resolved = resolve_module(&module).unwrap();
    let lookup = |fragment: &str, offset: usize| {
        resolved.names[&Span::new(
            text.find(fragment).unwrap() + offset,
            text.find(fragment).unwrap() + fragment.len(),
        )]
    };
    assert_eq!(
        lookup("a = x", 4),
        Binding::Property {
            owner_depth: 1,
            local: false
        }
    );
    assert_eq!(
        lookup("b = privateValue", 4),
        Binding::Property {
            owner_depth: 1,
            local: true
        }
    );
    assert_eq!(lookup("c = missing", 4), Binding::ImplicitThis);
    assert_eq!(
        lookup("f = let (x = 3) x", 16),
        Binding::Lexical { depth: 0, slot: 0 }
    );
}

#[test]
fn lexical_declaration_beats_later_inherited_receiver_members() {
    let text = "x = 1 base = new { result = x } derived = (base) { x = 2 }";
    let module = parse_module(text).unwrap();
    let resolved = resolve_module(&module).unwrap();
    let byte = text.find("result = x").unwrap() + 9;
    assert_eq!(
        resolved.names[&Span::new(byte, byte + 1)],
        Binding::Property {
            owner_depth: 1,
            local: false
        }
    );
}

#[test]
fn let_initializers_and_object_captures_have_separate_scopes() {
    let text = "x = 5 result = let (x = x) new { value = x }";
    let module = parse_module(text).unwrap();
    let resolved = resolve_module(&module).unwrap();
    let initial = text.find("x = x)").unwrap() + 4;
    let captured = text.find("value = x").unwrap() + 8;
    assert_eq!(
        resolved.names[&Span::new(initial, initial + 1)],
        Binding::Property {
            owner_depth: 0,
            local: false
        }
    );
    assert_eq!(
        resolved.names[&Span::new(captured, captured + 1)],
        Binding::Lexical { depth: 0, slot: 0 }
    );
}

#[test]
fn eof_underlines_have_a_virtual_utf16_width_not_an_invalid_byte_span() {
    let source = "answer = (1 +";
    let diagnostic = parse_module(source).unwrap_err();
    assert_eq!(diagnostic.span, Span::new(source.len(), source.len()));
    assert_eq!(diagnostic.highlight_length_utf16, Some(1));
    assert_eq!(diagnostic.message, "Unexpected end of file.");
    assert_eq!(
        parse_module("answer = (1").unwrap_err().message,
        "Unexpected token `EOF`. Expected `)`."
    );
    assert_eq!(
        parse_module("answer = new { x = 1").unwrap_err().message,
        "Missing `}` delimiter."
    );
}

#[test]
fn unsupported_base_properties_and_iterative_ordinary_nesting_are_explicit() {
    for name in ["Regex", "VarArgs", "Int8"] {
        let module = parse_module(&format!("result = {name}")).unwrap();
        assert_eq!(
            resolve_module(&module).unwrap_err().kind,
            DiagnosticKind::Unsupported
        );
    }
    let grouped = format!("{}1{}", "(".repeat(129), ")".repeat(129));
    let expression = parse_expression(&grouped).unwrap();
    assert_eq!(expression.unparenthesized().kind, ExprKind::Int(1));
    assert_eq!(expression.span, Span::new(0, grouped.len()));

    let chain = format!("value{}", ".member".repeat(600));
    let expression = parse_expression(&chain).unwrap();
    assert_eq!(expression.span, Span::new(0, chain.len()));
    let mut receiver = &expression;
    let mut members = 0;
    while let ExprKind::Member(next, name) = &receiver.kind {
        assert_eq!(name, "member");
        receiver = next;
        members += 1;
    }
    assert_eq!(members, 600);
    assert!(matches!(&receiver.kind, ExprKind::Name(name) if name == "value"));
}

#[test]
fn contextual_new_is_not_silently_accepted_as_a_qualified_receiver() {
    let source =
        "result = let (x = 7) let (y = x + 1) new { nested = new { value = x + y } }.nested.value";
    let module = parse_module(source).unwrap();
    let error = resolve_module(&module).unwrap_err();
    assert_eq!(error.kind, DiagnosticKind::Syntax);
    assert_eq!(error.message, "Cannot tell which parent to amend.");
    assert_eq!(&source[error.span.start..error.span.end], "new");
    assert_eq!(error.member.as_deref(), Some("result.<let expr>"));
}

#[test]
fn duplicate_members_are_an_explicit_resolution_error() {
    let module = parse_module("x = 1 x = 2").unwrap();
    assert_eq!(
        resolve_module(&module).unwrap_err().kind,
        DiagnosticKind::DuplicateMember
    );
}

/// File protocol used by tools/m1_frontend.py, so libtest's progress text never
/// contaminates the raw candidate bytes compared with the JVM probe's stdout.
#[test]
#[ignore = "invoked explicitly by the M1 differential evidence runner"]
fn frontend_probe() {
    let Some(path) = std::env::var_os("PKL_M1_FRONTEND_INPUT") else {
        panic!("missing input")
    };
    let source = std::fs::read_to_string(path).unwrap();
    let mut map = SourceMap::default();
    let id = map.add("probe", source.clone());
    let mapped = map.get(id).unwrap();
    let mut output = String::new();
    match std::env::var("PKL_M1_FRONTEND_MODE").unwrap().as_str() {
        "positions" => {
            use std::fmt::Write;
            for byte in (0..=source.len()).filter(|byte| source.is_char_boundary(*byte)) {
                let p = mapped.position(byte).unwrap();
                writeln!(
                    output,
                    "{}\t{}\t{}\t{}\t{}",
                    p.byte, p.utf16, p.codepoint, p.line, p.column_utf16
                )
                .unwrap();
            }
        }
        "parse" => {
            use std::fmt::Write;
            match parse_module(&source) {
                Ok(module) => {
                    output.push_str("ok\n");
                    for property in module.members {
                        let (start, length) = mapped.utf16_span(property.span).unwrap();
                        let (expr_start, expr_length) =
                            mapped.utf16_span(property.expr.span).unwrap();
                        writeln!(
                            output,
                            "{}\t{start}\t{length}\t{expr_start}\t{expr_length}",
                            property.name
                        )
                        .unwrap();
                    }
                }
                Err(error) => {
                    let (start, span_length) = mapped.utf16_span(error.span).unwrap();
                    let length = error.highlight_length_utf16.unwrap_or(span_length);
                    writeln!(output, "error\t{start}\t{length}").unwrap();
                    output.push_str(&error.message);
                }
            }
        }
        "value" => {
            use pkl_runtime::value::Value;
            use pkl_runtime::vm::{Vm, VmPoll, compile_module};
            let module = parse_module(&source).unwrap();
            let program = compile_module(&resolve_module(&module).unwrap()).unwrap();
            let entry = program.entry;
            let mut vm = Vm::new(program);
            vm.start(entry, vec![]).unwrap();
            let module = loop {
                match vm.poll(1000) {
                    VmPoll::Ready(result) => break result.unwrap().as_object().unwrap(),
                    VmPoll::Yielded => {}
                    VmPoll::Pending(_) => panic!("unexpected Host request"),
                }
            };
            vm.start_member(module, "result").unwrap();
            let result = loop {
                match vm.poll(1000) {
                    VmPoll::Ready(result) => break result.unwrap(),
                    VmPoll::Yielded => {}
                    VmPoll::Pending(_) => panic!("unexpected Host request"),
                }
            };
            output = match result {
                Value::Int(value) => value.to_string(),
                Value::Bool(value) => value.to_string(),
                Value::String(value) => value.to_string(),
                _ => panic!("unsupported scalar result"),
            };
        }
        _ => panic!("unknown mode"),
    }
    std::fs::write(std::env::var_os("PKL_M1_FRONTEND_OUTPUT").unwrap(), output).unwrap();
}
