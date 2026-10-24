//! Trace frontend regressions grounded in the pinned Stage37 Java observations.
//! Deep clone/drop is a Rust stack-safety check, not a Java depth-compatibility claim.
use std::sync::Arc;

use pkl_runtime::logging::{EvaluationContext, LogDelivery, LogEvent};
use pkl_runtime::parser::parse_module;
use pkl_runtime::resolver::resolve_module;
use pkl_runtime::source::SourceMap;
use pkl_runtime::syntax::ExprKind;
use pkl_runtime::value::{RuntimeSource, Value};
use pkl_runtime::vm::{Instruction, Program, Vm, VmPoll, compile_module};

const URI: &str = "file:///stage37-trace.pkl";

fn program(text: &str) -> Arc<Program> {
    let module = parse_module(text).unwrap();
    compile_module(&resolve_module(&module).unwrap()).unwrap()
}

fn ready(vm: &mut Vm, logs: &mut Vec<Arc<LogEvent>>, reads: &mut Vec<String>) -> Value {
    for _ in 0..10_000 {
        match vm.poll(32) {
            VmPoll::Ready(result) => return result.unwrap(),
            VmPoll::Log(request) => {
                logs.push(request.event);
                vm.acknowledge_log(request.id, Ok(())).unwrap();
            }
            VmPoll::Pending(request) => {
                reads.push(request.uri.to_string());
                vm.resume(request.id, Ok(Value::string("resource")))
                    .unwrap();
            }
            VmPoll::Module(_) => panic!("unexpected module request"),
            VmPoll::Yielded => {}
            VmPoll::TransformFrame(_) => panic!("unexpected transformer in default trace policy"),
        }
    }
    panic!("trace test did not settle");
}

fn evaluate(
    text: &str,
    delivery: LogDelivery,
    repetitions: usize,
) -> (Vec<Value>, Vec<Arc<LogEvent>>, Vec<String>) {
    let mut compiled = program(text);
    Arc::make_mut(&mut compiled).module_name = "stage37-trace".into();
    let entry = compiled.entry;
    let context = EvaluationContext {
        primary_source: Some(Arc::new(RuntimeSource {
            text: text.into(),
            uri: URI.into(),
            module_name: "stage37-trace".into(),
        })),
        log_delivery: delivery,
        ..EvaluationContext::default()
    };
    let mut vm = Vm::with_context(compiled, context);
    let mut logs = Vec::new();
    let mut reads = Vec::new();
    vm.start(entry, vec![]).unwrap();
    let module = ready(&mut vm, &mut logs, &mut reads);
    let mut values = Vec::new();
    for _ in 0..repetitions {
        vm.start_member(module.as_object().unwrap(), "result")
            .unwrap();
        values.push(ready(&mut vm, &mut logs, &mut reads));
    }
    vm.close();
    (values, logs, reads)
}

#[test]
fn trace_compiler_uses_executable_operand_spans_and_keeps_source_bytes() {
    // Java: stage37-frontend-binding-review-01 plus trace-design-01 UTF-16/CRLF.
    let cases = [
        (
            "result=trace( ((let (x=1) (x + 2))) )\n",
            vec![("let (x=1) (x + 2)", (1, 17, 1, 33))],
        ),
        (
            "result=trace( (trace( (1 + 2) )) )\n",
            vec![
                ("1 + 2", (1, 24, 1, 28)),
                ("trace( (1 + 2) )", (1, 16, 1, 31)),
            ],
        ),
        (
            "result=trace( ( if (true) 1 else 2 ) )\n",
            vec![("if (true) 1 else 2", (1, 17, 1, 34))],
        ),
        (
            "prefix=\"😀\"; result=trace(\"😀\")\n",
            vec![("\"😀\"", (1, 27, 1, 30))],
        ),
        (
            "result=trace(new Dynamic {\r\n  x=1\r\n}).x\r\n",
            vec![("new Dynamic {\r\n  x=1\r\n}", (1, 14, 3, 1))],
        ),
    ];
    for (text, expected) in cases {
        let compiled = program(text);
        let mut sources = SourceMap::default();
        let id = sources.add(URI, text);
        let source = sources.get(id).unwrap();
        let mut actual = Vec::new();
        for (block, metadata) in compiled.blocks.iter().zip(&compiled.metadata) {
            for (index, instruction) in block.iter().enumerate() {
                if let Instruction::Trace { operand_span } = instruction {
                    let start = source.position(operand_span.start).unwrap();
                    let end = source.position(operand_span.end).unwrap();
                    actual.push((
                        source.slice(*operand_span).unwrap(),
                        (
                            start.line,
                            start.column_utf16,
                            end.line,
                            end.column_utf16 - 1,
                        ),
                    ));
                    let execution = source.slice(metadata.spans[index].unwrap()).unwrap();
                    assert!(execution.starts_with("trace(") && execution.ends_with(')'));
                }
            }
        }
        assert_eq!(actual, expected, "{text}");
    }
}

#[test]
fn trace_preserves_let_and_property_inferred_parents() {
    let cases = [
        (
            "class C { x=7 }\nfoo: C=trace(new {})\nresult=foo.x\n",
            7,
            "new {} = new C { x = 7 }",
            (2, 14, 2, 19),
        ),
        (
            "class C { x=7 }\nfunction make(): C=trace(let (n=1) new { x=n })\nresult=make().x\n",
            1,
            "let (n=1) new { x=n } = new C { x = 1 }",
            (2, 26, 2, 46),
        ),
    ];
    for (text, value, message, position) in cases {
        let (values, logs, reads) = evaluate(text, LogDelivery::Host, 1);
        assert_eq!(values, [Value::Int(value.into())]);
        assert!(reads.is_empty());
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].message, message);
        let frame = &logs[0].frame;
        assert_eq!(frame.module_uri, URI);
        assert_eq!(frame.member_name, None);
        assert_eq!(
            (
                frame.start_line,
                frame.start_column,
                frame.end_line,
                frame.end_column
            ),
            position
        );
    }
}

#[test]
fn grouping_inside_trace_still_blocks_inferred_new() {
    let text = "class C { x=7 }\nfoo: C=trace((new {}))\nresult=foo.x\n";
    let module = parse_module(text).unwrap();
    let error = resolve_module(&module).unwrap_err();
    assert_eq!(error.message, "Cannot tell which parent to amend.");
    assert_eq!(&text[error.span.start..error.span.end], "new");
}

#[test]
fn nested_trace_logs_inner_then_outer_with_distinct_operand_sections() {
    let text = "result=trace( (trace( (1 + 2) )) )\n";
    let (values, logs, reads) = evaluate(text, LogDelivery::Host, 1);
    assert_eq!(values, [Value::Int(3.into())]);
    assert!(reads.is_empty());
    assert_eq!(
        logs.iter()
            .map(|event| event.message.clone())
            .collect::<Vec<_>>(),
        ["1 + 2 = 3", "trace( (1 + 2) ) = 3"]
    );
    assert_eq!(
        (logs[0].frame.start_column, logs[0].frame.end_column),
        (24, 28)
    );
    assert_eq!(
        (logs[1].frame.start_column, logs[1].frame.end_column),
        (16, 31)
    );
}

#[test]
fn trace_operand_is_evaluated_once_and_cached_property_does_not_log_again() {
    for delivery in [LogDelivery::Host, LogDelivery::Noop] {
        let (values, logs, reads) = evaluate("result=trace(read(\"life:text\"))\n", delivery, 2);
        assert_eq!(
            values,
            [Value::string("resource"), Value::string("resource")]
        );
        assert_eq!(reads, ["life:text"]);
        assert_eq!(logs.len(), usize::from(delivery == LogDelivery::Host));
        if let Some(event) = logs.first() {
            assert_eq!(event.message, "read(\"life:text\") = \"resource\"");
        }
    }
}

#[test]
fn trace_keyword_has_one_operand_while_quoted_method_name_remains_a_call() {
    for text in [
        "result=trace()\n",
        "result=trace(1,2)\n",
        "result=trace 1\n",
    ] {
        assert!(parse_module(text).is_err(), "{text}");
    }
    let module = parse_module("function `trace`(x)=x+1\nresult=`trace`(41)\n").unwrap();
    assert!(matches!(module.members[1].expr.kind, ExprKind::Call { .. }));
    assert!(
        program(&module.source)
            .blocks
            .iter()
            .flatten()
            .all(|instruction| !matches!(instruction, Instruction::Trace { .. }))
    );
}

#[test]
fn unmatched_surrogate_log_units_survive_trace_frontend() {
    // Exact logger char units from trace-design-02/surrogate-code-units.
    let (values, logs, _) = evaluate("result=trace(\"\\u{D800}\")\n", LogDelivery::Host, 1);
    assert_eq!(
        values,
        [Value::String(pkl_runtime::value::PklString::from_utf16(
            vec![0xd800]
        ))]
    );
    assert_eq!(logs.len(), 1);
    assert_eq!(
        logs[0].message.encode_utf16().collect::<Vec<_>>(),
        [
            0x22, 0x5c, 0x75, 0x7b, 0x44, 0x38, 0x30, 0x30, 0x7d, 0x22, 0x20, 0x3d, 0x20, 0x22,
            0xd800, 0x22
        ]
    );
}

#[test]
fn deep_trace_syntax_clone_and_drop_use_the_explicit_work_stack() {
    // The parser's existing debug stack footprint exceeds 128KB even for a
    // literal. Isolate the clone/drop guarantee from parser stack sizing.
    let depth = 2048;
    let text = format!("result={}1{}\n", "trace(".repeat(depth), ")".repeat(depth));
    let module = parse_module(&text).unwrap();
    std::thread::Builder::new()
        .stack_size(128 * 1024)
        .spawn(move || {
            let copied = module.clone();
            let mut expression = &copied.members[0].expr;
            let mut count = 0;
            while let ExprKind::Trace(operand) = &expression.kind {
                count += 1;
                expression = operand;
            }
            assert_eq!(count, depth);
            assert!(matches!(expression.kind, ExprKind::Int(1)));
            drop(module);
            drop(copied);
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn power_assertion_trace_and_operand_anchors_match_java_raw_errors() {
    // Fresh pinned Java stage37-frontend-power-oracle-01; complete raw stderr,
    // including every marker, source column, logical frame and final newline.
    let cases = [
        (
            r#"result: Int(trace(this > 0)) = -1
"#,
            r#"–– Pkl Error ––
Type constraint `trace(this > 0)` violated.
Value: -1

    trace(this > 0)
    │     │    │
    │     -1   false
    false

1 | result: Int(trace(this > 0)) = -1
                ^^^^^^^^^^^^^^^
at stage37-trace#result (file:///stage37-trace.pkl)

1 | result: Int(trace(this > 0)) = -1
                                   ^^
at stage37-trace#result (file:///stage37-trace.pkl)

1 | result
    ^^^^^^
at  (repl:text)
"#,
        ),
        (
            r#"result: Int(trace(this > 0) && this < 10) = -1
"#,
            r#"–– Pkl Error ––
Type constraint `trace(this > 0) && this < 10` violated.
Value: -1

    trace(this > 0) && this < 10
    │     │    │    │
    │     -1   │    false
    false      false

1 | result: Int(trace(this > 0) && this < 10) = -1
                ^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at stage37-trace#result (file:///stage37-trace.pkl)

1 | result: Int(trace(this > 0) && this < 10) = -1
                                                ^^
at stage37-trace#result (file:///stage37-trace.pkl)

1 | result
    ^^^^^^
at  (repl:text)
"#,
        ),
    ];
    for (text, expected) in cases {
        let actual = pkl_runtime::evaluation::scalar_expression_with_options(
            text,
            URI,
            "stage37-trace",
            "result",
            pkl_runtime::evaluation::EvaluationOptions {
                power_assertions: true,
            },
        )
        .unwrap_err();
        assert_eq!(actual.as_bytes(), expected.as_bytes());
    }
}
