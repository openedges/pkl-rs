//! Pinned Pkl 0.32 `read?` frontend regressions; Host policy is tested separately.
use std::sync::Arc;

use pkl_runtime::parser::parse_module;
use pkl_runtime::resolver::resolve_module;
use pkl_runtime::source::{SourceMap, Span};
use pkl_runtime::syntax::ExprKind;
use pkl_runtime::value::Value;
use pkl_runtime::vm::{Instruction, Program, ReadRequest, Vm, VmPoll, compile_module};

fn program(text: &str) -> Arc<Program> {
    let module = parse_module(text).unwrap();
    compile_module(&resolve_module(&module).unwrap()).unwrap()
}

fn settle(vm: &mut Vm, requests: &mut Vec<ReadRequest>) -> Value {
    for _ in 0..10_000 {
        match vm.poll(32) {
            VmPoll::Ready(result) => return result.unwrap(),
            VmPoll::Pending(request) => {
                requests.push(request.clone());
                vm.resume(request.id, Ok(Value::Null)).unwrap();
            }
            VmPoll::Log(request) => vm.acknowledge_log(request.id, Ok(())).unwrap(),
            VmPoll::Yielded => {}
        }
    }
    panic!("read frontend test did not settle");
}

#[test]
fn read_nullable_compiles_a_keyword_operand_with_full_authored_span() {
    let text = "prefix=\"😀\"\r\nresult=read? ( (\"env:missing\") )?.value\r\n";
    let module = parse_module(text).unwrap();
    let ExprKind::NullableMember(read, _) = &module.members[1].expr.kind else {
        panic!("nullable member must follow read? expression");
    };
    let ExprKind::ReadResourceOrNull(operand) = &read.kind else {
        panic!("read? must be distinct from an authored method call");
    };
    assert!(matches!(operand.kind, ExprKind::Parenthesized(_)));
    assert_eq!(
        &text[read.span.start..read.span.end],
        "read? ( (\"env:missing\") )"
    );
    let compiled = program(text);
    let mut found = Vec::new();
    for (block, metadata) in compiled.blocks.iter().zip(&compiled.metadata) {
        for (index, instruction) in block.iter().enumerate() {
            if matches!(instruction, Instruction::ReadResourceOrNull) {
                found.push(metadata.spans[index].unwrap());
            }
        }
    }
    assert_eq!(found, [read.span]);
    let mut sources = SourceMap::default();
    let id = sources.add("file:///stage38-read-frontend.pkl", text);
    let source = sources.get(id).unwrap();
    assert_eq!(source.position(read.span.start).unwrap().column_utf16, 8);
}

#[test]
fn read_nullable_has_one_operand_and_contiguous_keyword_spelling() {
    // Fresh Java stage38-read-frontend-oracle-01 keeps every failed source.
    for text in [
        "result=read?()\n",
        "result=read?(\"env:missing\",throw(\"second\"))\n",
        "result=read?\n",
        "result=read ?(\"env:missing\")\n",
        "obj=new Dynamic {function `read?`(x)=x}\nresult=obj.read?(1)\n",
    ] {
        assert!(parse_module(text).is_err(), "{text}");
    }
    for text in [
        "result=read?(\"env:missing\")\n",
        "result=read? (\"env:missing\")\n",
        "result=read?\n(\"env:missing\")\n",
        "result=read?/* comment */(\"env:missing\")\n",
    ] {
        assert!(matches!(
            parse_module(text).unwrap().members[0].expr.kind,
            ExprKind::ReadResourceOrNull(_)
        ));
    }
}

#[test]
fn quoted_read_names_remain_ordinary_user_methods() {
    for name in ["read", "read?"] {
        let text = format!("function `{name}`(x)=x+1\nresult=`{name}`(41)\n");
        let compiled = program(&text);
        assert!(
            compiled
                .blocks
                .iter()
                .flatten()
                .all(|instruction| !matches!(
                    instruction,
                    Instruction::ReadResource | Instruction::ReadResourceOrNull
                ))
        );
        let mut vm = Vm::new(compiled.clone());
        let mut requests = Vec::new();
        vm.start(compiled.entry, vec![]).unwrap();
        let module = settle(&mut vm, &mut requests);
        vm.start_member(module.as_object().unwrap(), "result")
            .unwrap();
        assert_eq!(settle(&mut vm, &mut requests), Value::Int(42));
        assert!(requests.is_empty());
    }
}

#[test]
fn ordinary_and_nullable_read_opcodes_preserve_their_request_flags() {
    let compiled = program("normal=read(\"life:normal\")\nnullable=read?(\"life:nullable\")\n");
    let mut vm = Vm::new(compiled.clone());
    let mut requests = Vec::new();
    vm.start(compiled.entry, vec![]).unwrap();
    let module = settle(&mut vm, &mut requests);
    for name in ["normal", "nullable", "nullable"] {
        vm.start_member(module.as_object().unwrap(), name).unwrap();
        assert_eq!(settle(&mut vm, &mut requests), Value::Null);
    }
    assert_eq!(requests.len(), 2);
    assert!(!requests[0].nullable);
    assert!(requests[1].nullable);
    assert_eq!(requests[0].uri.as_ref(), "life:normal");
    assert_eq!(requests[1].uri.as_ref(), "life:nullable");
}

#[test]
fn nullable_read_operand_uses_the_enclosing_lexical_scope() {
    let compiled = program("result=let (uri=\"life:scoped\") read?(uri)\n");
    let mut vm = Vm::new(compiled.clone());
    let mut requests = Vec::new();
    vm.start(compiled.entry, vec![]).unwrap();
    let module = settle(&mut vm, &mut requests);
    vm.start_member(module.as_object().unwrap(), "result")
        .unwrap();
    assert_eq!(settle(&mut vm, &mut requests), Value::Null);
    assert_eq!(requests.len(), 1);
    assert!(requests[0].nullable);
    assert_eq!(requests[0].uri.as_ref(), "life:scoped");
}

#[test]
fn nullable_read_does_not_forward_contextual_new_to_its_operand() {
    let text = "result: String=read?(new {})\n";
    let module = parse_module(text).unwrap();
    let error = resolve_module(&module).unwrap_err();
    assert_eq!(error.message, "Cannot tell which parent to amend.");
    assert_eq!(&text[error.span.start..error.span.end], "new");
}

#[test]
fn nested_nullable_reads_keep_distinct_instruction_source_spans() {
    let text = "result=read?(read?(\"prop:missing\"))\n";
    let compiled = program(text);
    let mut actual = Vec::new();
    for (block, metadata) in compiled.blocks.iter().zip(&compiled.metadata) {
        for (index, instruction) in block.iter().enumerate() {
            if matches!(instruction, Instruction::ReadResourceOrNull) {
                let span = metadata.spans[index].unwrap();
                actual.push(&text[span.start..span.end]);
            }
        }
    }
    assert_eq!(
        actual,
        ["read?(\"prop:missing\")", "read?(read?(\"prop:missing\"))"]
    );
}

#[test]
fn deep_nullable_read_syntax_clone_and_drop_use_explicit_stacks() {
    // As in the Stage37 trace regression, parser stack sizing is a separate
    // obligation. This isolates clone/drop on an already parsed authored tree.
    let depth = 2048;
    let text = format!(
        "result={}\"env:missing\"{}\n",
        "read?(".repeat(depth),
        ")".repeat(depth)
    );
    let module = parse_module(&text).unwrap();
    std::thread::Builder::new()
        .stack_size(128 * 1024)
        .spawn(move || {
            let copied = module.clone();
            let mut expression = &copied.members[0].expr;
            let mut count = 0;
            while let ExprKind::ReadResourceOrNull(operand) = &expression.kind {
                count += 1;
                expression = operand;
            }
            assert_eq!(count, depth);
            assert!(matches!(expression.kind, ExprKind::String(_)));
            assert_eq!(expression.span, Span::new(7 + depth * 6, 20 + depth * 6));
            drop(module);
            drop(copied);
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn unquoted_read_keywords_cannot_be_shadowed_by_quoted_methods() {
    for (name, nullable) in [("read", false), ("read?", true)] {
        let text = format!("function `{name}`(x)=x+1\nresult={name}(\"life:keyword\")\n");
        let compiled = program(&text);
        let mut vm = Vm::new(compiled.clone());
        let mut requests = Vec::new();
        vm.start(compiled.entry, vec![]).unwrap();
        let module = settle(&mut vm, &mut requests);
        vm.start_member(module.as_object().unwrap(), "result")
            .unwrap();
        assert_eq!(settle(&mut vm, &mut requests), Value::Null);
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].nullable, nullable);
        assert_eq!(requests[0].uri.as_ref(), "life:keyword");
    }
}

#[test]
fn read_keywords_reject_invalid_arity_before_evaluating_an_argument() {
    for text in [
        "result=read()\n",
        "result=read(\"env:missing\",throw(\"second\"))\n",
    ] {
        assert!(parse_module(text).is_err());
    }
}

#[test]
fn read_nullable_power_assertion_keeps_operand_observations_and_full_java_error() {
    // Pinned full raw stderr: stage38-read-power-oracle-01.
    let cases = [
        (
            "result: Int(read?(\"env:STAGE38_MISSING\") == this) = 7\n",
            "–– Pkl Error ––\nType constraint `read?(\"env:STAGE38_MISSING\") == this` violated.\nValue: 7\n\n    read?(\"env:STAGE38_MISSING\") == this\n    │                            │  │\n    null                         │  7\n                                 false\n\n1 | result: Int(read?(\"env:STAGE38_MISSING\") == this) = 7\n                ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^\nat stage38-read-frontend#result (file:///stage38-read-frontend.pkl)\n\n1 | result: Int(read?(\"env:STAGE38_MISSING\") == this) = 7\n                                                        ^\nat stage38-read-frontend#result (file:///stage38-read-frontend.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n",
        ),
        (
            "result: Int(let (uri=\"env:STAGE38_MISSING\") read?(uri) == this) = 7\n",
            "–– Pkl Error ––\nType constraint `let (uri=\"env:STAGE38_MISSING\") read?(uri) == this` violated.\nValue: 7\n\n    let (uri=\"env:STAGE38_MISSING\") read?(uri) == this\n                                    │     │    │  │\n                                    null  │    │  7\n                                          │    false\n                                          \"env:STAGE38_MISSING\"\n\n1 | result: Int(let (uri=\"env:STAGE38_MISSING\") read?(uri) == this) = 7\n                ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^\nat stage38-read-frontend#result (file:///stage38-read-frontend.pkl)\n\n1 | result: Int(let (uri=\"env:STAGE38_MISSING\") read?(uri) == this) = 7\n                                                                      ^\nat stage38-read-frontend#result (file:///stage38-read-frontend.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n",
        ),
    ];
    for (source, expected) in cases {
        let actual = pkl_runtime::evaluation::scalar_expression_with_options(
            source,
            "file:///stage38-read-frontend.pkl",
            "stage38-read-frontend",
            "result",
            pkl_runtime::evaluation::EvaluationOptions {
                power_assertions: true,
            },
        )
        .unwrap_err();
        assert_eq!(actual.as_bytes(), expected.as_bytes());
    }
}

#[test]
fn failed_unqualified_method_lookup_uses_the_whole_call_source_section() {
    // Fresh generic Java cases in stage38-method-diagnostic-oracle-01 confirm
    // lookup fails at the call before any argument expression is evaluated.
    for text in [
        "result=absent(throw(\"argument\"))\n",
        "result=`read`(\"env:missing\")\n",
        "result=`read?`(\"env:missing\")\n",
    ] {
        let module = parse_module(text).unwrap();
        let span = module.members[0].expr.span;
        let compiled = program(text);
        let mut actual = Vec::new();
        for (block, metadata) in compiled.blocks.iter().zip(&compiled.metadata) {
            for (index, instruction) in block.iter().enumerate() {
                if matches!(instruction, Instruction::GetImplicitMethod(_)) {
                    actual.push(metadata.spans[index].unwrap());
                }
            }
        }
        assert_eq!(actual, [span], "{text}");
    }
}
