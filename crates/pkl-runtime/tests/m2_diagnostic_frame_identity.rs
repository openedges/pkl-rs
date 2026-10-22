//! Frame names retain UTF-16 identity until the shared text renderer.
//! These native lifetime invariants complement separately captured Java bytes.
use std::sync::Arc;

use pkl_runtime::host::{HostPoll, HostSession};
use pkl_runtime::parser::parse_module;
use pkl_runtime::resolver::resolve_module;
use pkl_runtime::value::{ErrorKind, PklString, RuntimeError, Value};
use pkl_runtime::vm::{Instruction as I, Program, Vm, VmPoll, compile_module};

fn compile(source: &str) -> Arc<Program> {
    compile_module(&resolve_module(&parse_module(source).unwrap()).unwrap()).unwrap()
}

fn label(prefix: &str, key: &[u16], suffix: &str) -> Vec<u16> {
    prefix
        .encode_utf16()
        .chain(key.iter().copied())
        .chain(suffix.encode_utf16())
        .collect()
}

#[test]
fn literal_entry_metadata_preserves_units_and_independent_numbered_children() {
    let program = compile(
        r#"x = new Mapping {
  ["\u{d800}"] = new Listing { throw("high") }
  ["\u{dfff}"] = new Listing { throw("low") }
  ["?"] = new Listing { throw("question") }
  ["\u{fffd}"] = new Listing { throw("replacement") }
  ["\u{d83d}\u{de00}"] = new Listing { throw("pair") }
}
"#,
    );
    let names: Vec<Vec<u16>> = program
        .metadata
        .iter()
        .map(|metadata| metadata.member.encode_utf16().collect())
        .collect();
    for key in [
        &[0xd800][..],
        &[0xdfff][..],
        &[0x3f][..],
        &[0xfffd][..],
        &[0xd83d, 0xde00][..],
    ] {
        assert!(names.contains(&label("#x[\"", key, "\"][#1]")));
    }
    assert!(
        !names
            .iter()
            .any(|name| name.ends_with(&"[#2]".encode_utf16().collect::<Vec<_>>()))
    );

    let escaped = compile(r#"x = new Mapping { ["\u{d83d}\u{de00}"] = 1 }"#);
    let scalar = compile(r#"x = new Mapping { ["😀"] = 1 }"#);
    let names = |program: &Program| {
        program
            .metadata
            .iter()
            .map(|metadata| metadata.member.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(names(&escaped), names(&scalar));
}

#[test]
fn delayed_literal_and_local_type_errors_retain_the_authored_entry_units() {
    let diagnostic = parse_module(
        r#"x = new Mapping { ["\u{d800}"] = 9223372036854775808 }
result = x["\u{d800}"]
"#,
    )
    .unwrap_err();
    assert_eq!(
        diagnostic
            .member
            .unwrap()
            .encode_utf16()
            .collect::<Vec<_>>(),
        label("x[\"", &[0xd800], "\"]")
    );

    let program = compile(
        r#"x = new Mapping { ["\u{d800}"] = new { local bad: Int = "wrong"; checked = bad } }
result = x["\u{d800}"].checked
"#,
    );
    let mut vm = Vm::new(program.clone());
    vm.start(program.entry, vec![]).unwrap();
    let VmPoll::Ready(Ok(module)) = vm.poll(1_000_000) else {
        panic!("expected module");
    };
    vm.start_member(module.as_object().unwrap(), "result")
        .unwrap();
    let VmPoll::Ready(Err(error)) = vm.poll(1_000_000) else {
        panic!("expected local property type failure");
    };
    for suffix in ["\"].bad", "\"].checked"] {
        let expected = label("#x[\"", &[0xd800], suffix);
        assert!(
            error
                .frames
                .iter()
                .any(|frame| frame.member.encode_utf16().eq(expected.iter().copied()))
        );
    }
    vm.collect(&[]);
    vm.close();
    assert_eq!(vm.heap_stats().slots, 0);
    assert!(
        error
            .frames
            .iter()
            .any(|frame| frame.member.encode_utf16().any(|unit| unit == 0xd800))
    );
}

#[test]
fn local_type_frames_follow_declaration_origin_for_direct_and_aliased_types() {
    // Fresh pinned Java controls establish two frames for typed module/class
    // locals and one for directly checked anonymous object locals.
    for (source, member, expected) in [
        ("local bad: Int = \"wrong\"\nresult = bad\n", "#bad", 2),
        (
            "typealias Fn = (Int) -> String\nlocal bad: Fn = () -> \"bad\"\nresult = bad\n",
            "#bad",
            2,
        ),
        (
            "class C { local bad: Int = \"wrong\"; checked = bad }\nx = new C {}\nresult = x.checked\n",
            "#C.bad",
            2,
        ),
        (
            "typealias Fn = (Int) -> String\nclass C { local bad: Fn = () -> \"bad\"; checked = bad }\nx = new C {}\nresult = x.checked\n",
            "#C.bad",
            2,
        ),
        (
            "x = new { local bad: Int = \"wrong\"; checked = bad }\nresult = x.checked\n",
            "#x.bad",
            1,
        ),
        (
            "typealias Fn = (Int) -> String\nx = new { local bad: Fn = () -> \"bad\"; checked = bad }\nresult = x.checked\n",
            "#x.bad",
            1,
        ),
    ] {
        let program = compile(source);
        let mut vm = Vm::new(program.clone());
        vm.start(program.entry, vec![]).unwrap();
        let VmPoll::Ready(Ok(module)) = vm.poll(1_000_000) else {
            panic!("expected module");
        };
        vm.start_member(module.as_object().unwrap(), "result")
            .unwrap();
        let VmPoll::Ready(Err(error)) = vm.poll(1_000_000) else {
            panic!("expected local property type failure");
        };
        assert_eq!(error.kind, ErrorKind::Type);
        assert_eq!(
            error
                .frames
                .iter()
                .filter(|frame| frame.member.encode_utf16().eq(member.encode_utf16()))
                .count(),
            expected,
            "{source}\n{:?}",
            error.frames
        );
        vm.close();
        assert_eq!(vm.heap_stats().slots, 0);
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
    panic!("diagnostic frame Host work did not settle");
}

fn ready(session: &mut HostSession, module: &Value, now: u64) -> Result<Value, RuntimeError> {
    match event(session, module, now) {
        HostPoll::Ready(result) => result,
        event => panic!("expected result, got {event:?}"),
    }
}

#[test]
fn suspended_entry_frames_survive_reentry_retry_collection_and_evaluator_close() {
    let source = r#"parent = read("mem:outer")
x = new Mapping {
  ["\u{d800}"] = new Listing { let (f = () -> read("mem:inner")) f.apply() }
}
child = x["\u{d800}"][0]
"#;
    let mut program = (*compile(source)).clone();
    let [parent, child] = ["parent", "child"].map(|name| {
        let entry = program.blocks.len();
        program
            .blocks
            .push(vec![I::Input(0), I::GetMember(name.into()), I::Return]);
        entry
    });
    let program = Arc::new(program);
    let expected_let = PklString::from_utf16(label("#x[\"", &[0xd800], "\"][#1].<let expr>"));
    let expected = PklString::from_utf16(label("#x[\"", &[0xd800], "\"][#1].<function#1>"));
    for _ in 0..2 {
        let mut session = HostSession::new(program.clone());
        session.start(program.entry, vec![], 0, None).unwrap();
        let module = ready(&mut session, &Value::Null, 0).unwrap();
        session
            .start(parent, vec![module.clone()], 1, None)
            .unwrap();
        let HostPoll::Read { token: outer, .. } = event(&mut session, &module, 1) else {
            panic!("expected outer read");
        };
        let mut detached = None;
        let mut previous_token = None;
        for now in [2, 3] {
            session
                .reenter(&outer, child, vec![module.clone()], now, None)
                .unwrap();
            let HostPoll::Read { token, uri } = event(&mut session, &module, now) else {
                panic!("expected inner read");
            };
            assert_eq!(&*uri, "mem:inner");
            assert_ne!(previous_token.as_ref(), Some(&token));
            previous_token = Some(token.clone());
            session
                .resume(
                    &token,
                    Err(RuntimeError::new(ErrorKind::Host, "frame retry")),
                    now,
                )
                .unwrap();
            let error = ready(&mut session, &module, now).unwrap_err();
            assert_eq!(
                (error.kind, error.message.as_str()),
                (ErrorKind::Host, "frame retry")
            );
            assert!(
                error
                    .frames
                    .iter()
                    .any(|frame| frame.member.encode_utf16().eq(expected.encode_utf16())),
                "{:?}",
                error.frames
            );
            assert!(
                error
                    .frames
                    .iter()
                    .any(|frame| frame.member.encode_utf16().eq(expected_let.encode_utf16()))
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
        session.resume(&outer, Ok(Value::Int(42)), 4).unwrap();
        assert_eq!(ready(&mut session, &module, 4), Ok(Value::Int(42)));
        session.close();
        assert_eq!(session.heap_stats().slots, 0);
        let detached = detached.unwrap();
        assert!(
            detached
                .frames
                .iter()
                .any(|frame| frame.member.encode_utf16().eq(expected.encode_utf16()))
        );
        assert_eq!(expected.to_java_utf8(), "#x[\"?\"][#1].<function#1>");
    }
}
