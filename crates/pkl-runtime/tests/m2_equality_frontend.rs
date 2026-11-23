//! Structural keys retain authored syntax and the enclosing generator scope.
use pkl_runtime::diagnostic::DiagnosticKind;
use pkl_runtime::evaluation::frontend::{
    ModuleEvalOptions, ModuleSourceInput, evaluate_module_output,
};
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
            pkl_runtime::vm::VmPoll::Glob(_) => {
                panic!("unexpected glob request in non-glob regression")
            }
            VmPoll::Ready(value) => return value,
            VmPoll::Yielded => {}
            VmPoll::Log(_) | VmPoll::TransformFrame(_) => panic!("unexpected logger request"),
            VmPoll::Module(_) => panic!("unexpected module request"),
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

#[test]
fn cache_capacity_threshold_rehash_runs_at_225_slots() {
    // Verified against pinned Apple Pkl 0.32.0 (`pkl-macos-aarch64 eval -`):
    // crossing the 225 -> 337 (>= 256) EconomicMap width threshold must trigger
    // a pre-publish rehash that forces `k` before publishing `p225`.
    let mut source = String::from("import \"pkl:test\"\nlocal base = new Dynamic {\n");
    for i in 1..=227 {
        source.push_str(&format!("  p{i} = {i}\n"));
    }
    source.push_str("}\nlocal k = new Dynamic { x = throw(\"rehash\") }\n");
    source.push_str("local d = (base) { [k] = 0 }\n");
    source.push_str("local init = List(d[k], d.p1, d.p2, d.p3)\n");
    for i in 4..=224 {
        source.push_str(&format!(
            "local c{i} = test.catch(() -> throw(\"OK:\" + d.p{i}.toString()))\n"
        ));
    }
    source.push_str("local mid = List(");
    for i in 4..=224 {
        if i > 4 {
            source.push_str(", ");
        }
        source.push_str(&format!("c{i}"));
    }
    source.push_str(")\n");
    source.push_str("local r1 = test.catch(() -> throw(\"OK:\" + d.p225.toString()))\n");
    source.push_str("local r2 = test.catch(() -> throw(\"OK:\" + d.p225.toString()))\n");
    source.push_str("local r3 = test.catch(() -> throw(\"OK:\" + d.p226.toString()))\n");
    source.push_str(
        "res = if (init.length == 4 && mid.length == 221) List(r1, r2, r3) else List()\n",
    );

    let out = evaluate_module_output(
        ModuleSourceInput {
            text: &source,
            uri: "repl:text",
            inferred_name: "text",
        },
        &ModuleEvalOptions::default(),
    );
    assert_eq!(
        out.exit_code,
        0,
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        String::from_utf8(out.stdout).expect("UTF-8 PCF"),
        "res = List(\"rehash\", \"OK:225\", \"OK:226\")\n"
    );
}

#[test]
fn deep_force_respects_ancestor_object_keys_and_incomplete_rehash() {
    // Verified against pinned Apple Pkl 0.32.0 (`pkl-macos-aarch64 eval -`):
    // after `child.p4` fails rehash on inherited `[k0]`, forcing `child` for `==`
    // must re-probe `[k0]` instead of reading the stale `cache` map directly.
    let source = r#"import "pkl:test"
local k0 = new Dynamic { x = throw("k0-rehash") }
local base = new Dynamic { [k0] = "u" }
local child = (base) { p1 = 1; p2 = 2; p3 = 3; p4 = 4 }
local s0 = List(child[k0], child.p1, child.p2, child.p3)
local e1 = if (s0.length == 4) test.catch(() -> child.p4) else ""
res = if (e1 == "k0-rehash") test.catch(() -> let (eq = child == new Dynamic { p1 = 999 }) throw("EQ:" + eq.toString())) else "wrong"
"#;

    let out = evaluate_module_output(
        ModuleSourceInput {
            text: source,
            uri: "repl:text",
            inferred_name: "text",
        },
        &ModuleEvalOptions::default(),
    );
    assert_eq!(
        out.exit_code,
        0,
        "stderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        String::from_utf8(out.stdout).expect("UTF-8 PCF"),
        "res = \"k0-rehash\"\n"
    );
}

#[test]
fn trivial_member_eval_preserves_hidden_callsite_on_cache_rebuild() {
    // Verified against pinned Apple Pkl 0.32.0 (`pkl-macos-aarch64 eval -`):
    // async cache rebuild triggered while publishing `d.p4` hides the caller
    // frame `text#res` from the stack trace.
    let source = r#"local k = new Dynamic { x = throw("boom") }
local base = new Dynamic { [k] = 0 }
local d = (base) { p1 = 1; p2 = 2; p3 = 3; p4 = 4 }
res = List(d[k], d.p1, d.p2, d.p3, d.p4)
"#;

    let out = evaluate_module_output(
        ModuleSourceInput {
            text: source,
            uri: "repl:text",
            inferred_name: "text",
        },
        &ModuleEvalOptions::default(),
    );
    assert_ne!(out.exit_code, 0, "expected boom error");
    let stderr = String::from_utf8(out.stderr).expect("UTF-8 stderr");
    assert!(
        stderr.contains("boom\n"),
        "expected boom message, got:\n{stderr}"
    );
    assert!(
        stderr.contains("at text#k.x (repl:text)"),
        "expected k.x frame, got:\n{stderr}"
    );
    assert!(
        !stderr.contains("at text#res (repl:text)"),
        "caller frame text#res must be hidden during CacheRebuild, got:\n{stderr}"
    );
}
