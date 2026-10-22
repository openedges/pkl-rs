//! Lowered numeric source spans and authored expression identity are distinct.
//! Fresh Java captures separately establish complete diagnostic byte equality.
use std::sync::Arc;

use pkl_runtime::host::{HostPoll, HostSession};
use pkl_runtime::parser::parse_module;
use pkl_runtime::resolver::resolve_module;
use pkl_runtime::value::{ErrorKind, RuntimeError, Value};
use pkl_runtime::vm::{Instruction as I, Program, Vm, VmPoll, compile_module};

fn compile(source: &str) -> Arc<Program> {
    compile_module(&resolve_module(&parse_module(source).unwrap()).unwrap()).unwrap()
}

fn finish(vm: &mut Vm, roots: &[Value]) -> Result<Value, RuntimeError> {
    for _ in 0..200_000 {
        vm.collect(roots);
        if let VmPoll::Ready(result) = vm.poll(1) {
            return result;
        }
    }
    panic!("numeric value frame evaluation did not settle");
}

#[test]
fn numeric_value_frames_preserve_declaration_origin_and_grouping_semantics() {
    for (expression, body, constant) in [
        ("-1", "1", true),
        ("- 0x1", "0x1", true),
        ("(-0.0)", "0.0", true),
        ("((-0b1))", "0b1", true),
        ("(-(1))", "-(1)", false),
        ("--1", "--1", false),
        ("-(-1)", "-(-1)", false),
    ] {
        for (source, owner, inline, constant_property) in [
            (
                format!("local bad: String = {expression}\nresult = bad\n"),
                "#bad",
                false,
                false,
            ),
            (
                format!(
                    "class C {{ local bad: String = {expression}; checked = bad }}\nx = new C {{}}\nresult = x.checked\n"
                ),
                "#C.bad",
                false,
                false,
            ),
            (
                format!(
                    "class C {{ bad: String = {expression} }}\nx = new C {{}}\nresult = x.bad\n"
                ),
                "#C.bad",
                false,
                constant,
            ),
            (
                format!(
                    "x = new {{ local bad: String = {expression}; checked = bad }}\nresult = x.checked\n"
                ),
                "#x.bad",
                true,
                false,
            ),
        ] {
            let program = compile(&source);
            let mut vm = Vm::new(program.clone());
            vm.start(program.entry, vec![]).unwrap();
            let module = finish(&mut vm, &[]).unwrap();
            let mut detached = None;
            for _ in 0..2 {
                vm.start_member(module.as_object().unwrap(), "result")
                    .unwrap();
                let error = finish(&mut vm, std::slice::from_ref(&module)).unwrap_err();
                assert_eq!(error.kind, ErrorKind::Type);
                let spans: Vec<&str> = error
                    .frames
                    .iter()
                    .filter(|frame| frame.member.encode_utf16().eq(owner.encode_utf16()))
                    .map(|frame| &source[frame.span.start..frame.span.end])
                    .collect();
                // Public constant class defaults bypass a MemberNode in Java;
                // ObjectMember recovers the complete authored body instead.
                let expected_body = if constant_property { expression } else { body };
                let expected = if inline {
                    vec!["String"]
                } else {
                    vec!["String", expected_body]
                };
                assert_eq!(spans, expected, "{source}\n{error:?}");
                if let Some(previous) = &detached {
                    assert_eq!(previous, &error);
                }
                detached = Some(error);
            }
            vm.close();
            assert_eq!(vm.heap_stats().slots, 0);
            assert_eq!(detached.unwrap().kind, ErrorKind::Type);
        }
    }
    for (source, owner, body) in [
        (
            "x: Mapping<String,Int(this > 0)> = new { [\"a\"] = -1 }\nresult = x[\"a\"]\n",
            "#x[\"a\"]",
            "-1",
        ),
        (
            "x: Mapping<String,Int(this > 0)> = new { [\"a\"] = (-1) }\nresult = x[\"a\"]\n",
            "#x[\"a\"]",
            "(-1)",
        ),
        (
            "x: Listing<Int(this > 0)> = new { -1 }\nresult = x[0]\n",
            "#x[#1]",
            "-1",
        ),
        (
            "x: Listing<Int(this > 0)> = new { (-1) }\nresult = x[0]\n",
            "#x[#1]",
            ")",
        ),
    ] {
        let program = compile(source);
        let mut vm = Vm::new(program.clone());
        vm.start(program.entry, vec![]).unwrap();
        let module = finish(&mut vm, &[]).unwrap();
        let mut detached = None;
        for _ in 0..2 {
            vm.start_member(module.as_object().unwrap(), "result")
                .unwrap();
            let error = finish(&mut vm, std::slice::from_ref(&module)).unwrap_err();
            assert_eq!(error.kind, ErrorKind::Type);
            let spans: Vec<&str> = error
                .frames
                .iter()
                .filter(|frame| frame.member.encode_utf16().eq(owner.encode_utf16()))
                .map(|frame| &source[frame.span.start..frame.span.end])
                .collect();
            // Listing's constant source fallback retains a closing grouping
            // suffix. Preserve the pinned behavior even when it looks unusual.
            assert_eq!(spans, vec![body], "{source}\n{error:?}");
            if let Some(previous) = &detached {
                assert_eq!(previous, &error);
            }
            detached = Some(error);
        }
        vm.close();
        assert_eq!(vm.heap_stats().slots, 0);
        assert_eq!(detached.unwrap().kind, ErrorKind::Type);
    }
    for (expression, lowered) in [
        ("(\"wrong\")", "\"wrong\""),
        ("(true)", "true"),
        ("(null)", "null"),
    ] {
        for (source, owner, body) in [
            (
                format!("local bad: Int = {expression}\nresult = bad\n"),
                "#bad",
                lowered,
            ),
            (
                format!("class C {{ bad: Int = {expression} }}\nresult = new C {{}}.bad\n"),
                "#C.bad",
                expression,
            ),
        ] {
            let program = compile(&source);
            let mut vm = Vm::new(program.clone());
            vm.start(program.entry, vec![]).unwrap();
            let module = finish(&mut vm, &[]).unwrap();
            vm.start_member(module.as_object().unwrap(), "result")
                .unwrap();
            let error = finish(&mut vm, std::slice::from_ref(&module)).unwrap_err();
            assert_eq!(error.kind, ErrorKind::Type);
            let spans: Vec<&str> = error
                .frames
                .iter()
                .filter(|frame| frame.member.encode_utf16().eq(owner.encode_utf16()))
                .map(|frame| &source[frame.span.start..frame.span.end])
                .collect();
            assert_eq!(spans, vec!["Int", body], "{source}\n{error:?}");
            vm.close();
            assert_eq!(vm.heap_stats().slots, 0);
        }
    }
}

fn event(session: &mut HostSession, module: &Value, now: u64) -> HostPoll {
    for _ in 0..200_000 {
        session.collect(std::slice::from_ref(module));
        match session.poll(now, 1) {
            HostPoll::Yielded => {}
            event => return event,
        }
    }
    panic!("numeric value frame Host work did not settle");
}

fn ready(session: &mut HostSession, module: &Value, now: u64) -> Result<Value, RuntimeError> {
    match event(session, module, now) {
        HostPoll::Ready(result) => result,
        event => panic!("expected completed numeric work, got {event:?}"),
    }
}

#[test]
fn suspended_numeric_constraint_frames_survive_reentry_retry_gc_and_close() {
    let source = "typealias Checked = Int(read(\"mem:check\") == \"yes\")\nclass C { local bad: Checked = (- 0x1); checked = bad }\nx = new C {}\nparent = read(\"mem:outer\")\nresult = x.checked\n";
    let mut program = (*compile(source)).clone();
    let [parent, result] = ["parent", "result"].map(|name| {
        let block = program.blocks.len();
        program
            .blocks
            .push(vec![I::Input(0), I::GetMember(name.into()), I::Return]);
        block
    });
    let program = Arc::new(program);
    for _ in 0..2 {
        let mut session = HostSession::new(program.clone());
        session.start(program.entry, vec![], 0, None).unwrap();
        let module = ready(&mut session, &Value::Null, 0).unwrap();
        session
            .start(parent, vec![module.clone()], 1, None)
            .unwrap();
        let HostPoll::Read { token: outer, uri } = event(&mut session, &module, 1) else {
            panic!("expected outer read");
        };
        assert_eq!(&*uri, "mem:outer");
        let mut detached = None;
        let mut previous_token = None;
        for now in [2, 3] {
            session
                .reenter(&outer, result, vec![module.clone()], now, None)
                .unwrap();
            let HostPoll::Read { token, uri } = event(&mut session, &module, now) else {
                panic!("expected constraint read");
            };
            assert_eq!(&*uri, "mem:check");
            assert_ne!(previous_token.as_ref(), Some(&token));
            previous_token = Some(token.clone());
            session
                .resume(&token, Ok(Value::String("no".into())), now)
                .unwrap();
            let error = ready(&mut session, &module, now).unwrap_err();
            assert_eq!(error.kind, ErrorKind::Type);
            assert!(
                error.frames.iter().any(|frame| {
                    frame.member.encode_utf16().eq("#C.bad".encode_utf16())
                        && &source[frame.span.start..frame.span.end] == "0x1"
                }),
                "{error:?}"
            );
            if let Some(previous) = &detached {
                assert_eq!(previous, &error);
            }
            detached = Some(error);
            assert_eq!(session.context_depth(), 1);
            assert!(
                matches!(event(&mut session, &module, now), HostPoll::Read { token, .. } if token == outer)
            );
        }
        session
            .reenter(&outer, result, vec![module.clone()], 4, None)
            .unwrap();
        let HostPoll::Read { token, .. } = event(&mut session, &module, 4) else {
            panic!("expected retry constraint read");
        };
        session
            .resume(&token, Ok(Value::String("yes".into())), 4)
            .unwrap();
        assert_eq!(ready(&mut session, &module, 4), Ok(Value::Int(-1)));
        session
            .reenter(&outer, result, vec![module.clone()], 5, None)
            .unwrap();
        assert_eq!(ready(&mut session, &module, 5), Ok(Value::Int(-1)));
        session.resume(&outer, Ok(Value::Int(42)), 6).unwrap();
        assert_eq!(ready(&mut session, &module, 6), Ok(Value::Int(42)));
        session.close();
        assert_eq!(session.heap_stats().slots, 0);
        assert!(
            detached
                .unwrap()
                .frames
                .iter()
                .any(|frame| &source[frame.span.start..frame.span.end] == "0x1")
        );
    }
}

#[test]
fn signed_values_keep_minint_and_negative_zero_across_collection_and_reuse() {
    let program = compile(
        "minimum = -9223372036854775808\nradix = -0x8000000000000000\nzero = (-0.0)\ngrouped = -(1)\ndouble = --1\n",
    );
    for _ in 0..2 {
        let mut vm = Vm::new(program.clone());
        vm.start(program.entry, vec![]).unwrap();
        let module = finish(&mut vm, &[]).unwrap();
        for (name, expected) in [
            ("minimum", i64::MIN),
            ("radix", i64::MIN),
            ("grouped", -1),
            ("double", 1),
        ] {
            vm.start_member(module.as_object().unwrap(), name).unwrap();
            assert_eq!(
                finish(&mut vm, std::slice::from_ref(&module)),
                Ok(Value::Int(expected))
            );
        }
        vm.start_member(module.as_object().unwrap(), "zero")
            .unwrap();
        let Value::Float(zero) = finish(&mut vm, std::slice::from_ref(&module)).unwrap() else {
            panic!("expected floating negative zero");
        };
        assert_eq!(zero.to_bits(), (-0.0_f64).to_bits());
        vm.close();
        assert_eq!(vm.heap_stats().slots, 0);
    }
}
