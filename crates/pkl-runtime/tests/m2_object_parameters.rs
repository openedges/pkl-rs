//! Shared parameterized amendments retain their first checked lexical activation.
#[path = "support/module_policy.rs"]
mod module_policy;
use pkl_runtime::parser::parse_module;
use pkl_runtime::resolver::resolve_module;
use pkl_runtime::value::{RuntimeError, Value};
use pkl_runtime::vm::{Vm, VmPoll, compile_module};

fn finish(vm: &mut Vm) -> Result<Value, RuntimeError> {
    loop {
        vm.collect(&[]);
        let poll = vm.poll(2);
        vm.collect(&[]);
        match poll {
            VmPoll::Ready(result) => return result,
            VmPoll::Yielded => {}
            VmPoll::Module(request) => vm
                .complete_module(
                    request.id,
                    pkl_runtime::modules::default_completion(&request).unwrap(),
                )
                .unwrap(),
            other => panic!("unexpected {other:?}"),
        }
    }
}
fn evaluate(source: &str, member: &str) -> Result<Value, RuntimeError> {
    let parsed = parse_module(source).unwrap();
    let resolved = resolve_module(&parsed).unwrap();
    let mut program = compile_module(&resolved).unwrap();
    module_policy::register(std::sync::Arc::make_mut(&mut program), source);
    let mut vm = Vm::new(program.clone());
    vm.start(program.entry, vec![]).unwrap();
    let module = finish(&mut vm)?.as_object()?;
    vm.start_member(module, member)?;
    finish(&mut vm)
}
#[test]
fn first_parameters_survive_currying_and_lazy_member_capture() {
    let source = r#"
      local parent = (first) -> (second) -> (third) -> new Dynamic { fromParent = first + second + third }
      local amended = (parent) { first: Int -> fromBody = () -> first }
      result = let (object = amended.apply(2).apply(30).apply(400)) object.fromParent + object.fromBody.apply()
    "#;
    assert_eq!(evaluate(source, "result"), Ok(Value::Int(434.into())));
}
#[test]
fn explicit_and_inferred_mixins_bind_arguments() {
    let source = r#"
      local explicit = new Mixin { object -> answer = object.original + 1 }
      local inferred: Mixin = new { object, -> answer = object.original + 2 }
      local original = new Dynamic { original = 40 }
      result = explicit.apply(original).answer + inferred.apply(original).answer
    "#;
    assert_eq!(evaluate(source, "result"), Ok(Value::Int(83.into())));
}
#[test]
fn generator_entries_capture_object_parameters() {
    let source = r#"
      local make = (prefix) -> new Mapping {}
      local amended = (make) { prefix: String -> for (suffix in List("a", "b")) { [suffix] = () -> prefix + suffix } }
      result = amended.apply("x")["b"].apply()
    "#;
    assert_eq!(evaluate(source, "result"), Ok(Value::string("xb")));
}

use pkl_runtime::host::{HostPoll, HostSession};
use pkl_runtime::vm::Instruction as I;
use std::sync::Arc;

fn suspended_session() -> (HostSession, Value, usize, usize) {
    let source = r#"
      typealias Gate = Dynamic(read("mem:check:" + this.name) == "go")
      local parent = (input) -> let (wait = read("mem:parent:" + input.name)) (other) -> new Dynamic { parentValue = input.value + other }
      local amended = (parent) { input: Gate -> captured = () -> read("mem:body:" + input.name) + input.value.toString() }
      outerResult = amended.apply(new Dynamic { name = "outer"; value = 7 }).apply(20).captured.apply()
      innerResult = amended.apply(new Dynamic { name = "inner"; value = 9 }).apply(30).captured.apply()
    "#;
    let parsed = parse_module(source).unwrap();
    let mut program = (*compile_module(&resolve_module(&parsed).unwrap()).unwrap()).clone();
    module_policy::register(&mut program, source);
    let module_entry = program.entry;
    let entries = ["outerResult", "innerResult"].map(|member| {
        let entry = program.blocks.len();
        program.blocks.push(vec![
            I::Input(0),
            I::GetMember(Arc::from(member)),
            I::Return,
        ]);
        entry
    });
    let mut session = HostSession::new(Arc::new(program));
    session.start(module_entry, vec![], 0, None).unwrap();
    let HostPoll::Ready(Ok(root)) = event(&mut session) else {
        panic!("module initialization")
    };
    (session, root, entries[0], entries[1])
}
fn event(session: &mut HostSession) -> HostPoll {
    loop {
        session.collect(&[]);
        let poll = session.poll(0, 1);
        session.collect(&[]);
        match poll {
            HostPoll::Yielded => {}
            event => return event,
        }
    }
}
fn read(session: &mut HostSession, expected: &str) -> String {
    match event(session) {
        HostPoll::Read { token, uri } => {
            assert_eq!(&*uri, expected);
            token
        }
        other => panic!("expected {expected}, got {other:?}"),
    }
}
fn answer(session: &mut HostSession, token: &str, value: &str) {
    session.resume(token, Ok(Value::string(value)), 0).unwrap();
}

#[test]
fn checked_parameters_survive_host_reentry_failure_retry_and_lazy_currying() {
    let (mut session, root, outer_entry, inner_entry) = suspended_session();
    session
        .start(outer_entry, vec![root.clone()], 0, None)
        .unwrap();
    let outer = read(&mut session, "mem:check:outer");
    session
        .reenter(&outer, inner_entry, vec![root.clone()], 0, None)
        .unwrap();
    let failed = read(&mut session, "mem:check:inner");
    session
        .resume(
            &failed,
            Err(RuntimeError::new(
                pkl_runtime::value::ErrorKind::Host,
                "retry",
            )),
            0,
        )
        .unwrap();
    let HostPoll::Ready(Err(error)) = event(&mut session) else {
        panic!("nested failure")
    };
    assert_eq!(error.message, "retry");
    assert_eq!(read(&mut session, "mem:check:outer"), outer);
    session
        .reenter(&outer, inner_entry, vec![root], 0, None)
        .unwrap();
    let inner = read(&mut session, "mem:check:inner");
    assert_ne!(failed, inner);
    answer(&mut session, &inner, "go");
    let parent = read(&mut session, "mem:parent:inner");
    answer(&mut session, &parent, "ready");
    let body = read(&mut session, "mem:body:inner");
    answer(&mut session, &body, "inner/");
    assert!(
        matches!(event(&mut session), HostPoll::Ready(Ok(value)) if value == Value::string("inner/9"))
    );
    assert_eq!(read(&mut session, "mem:check:outer"), outer);
    answer(&mut session, &outer, "go");
    let parent = read(&mut session, "mem:parent:outer");
    answer(&mut session, &parent, "ready");
    let body = read(&mut session, "mem:body:outer");
    answer(&mut session, &body, "outer/");
    assert!(
        matches!(event(&mut session), HostPoll::Ready(Ok(value)) if value == Value::string("outer/7"))
    );
    assert_eq!(session.context_depth(), 0);
    session.close();
    assert_eq!(session.heap_stats().slots, 0);
}

#[test]
fn cancelling_a_suspended_parameter_check_releases_both_activations() {
    let (mut session, root, outer_entry, inner_entry) = suspended_session();
    session
        .start(outer_entry, vec![root.clone()], 0, None)
        .unwrap();
    let outer = read(&mut session, "mem:check:outer");
    session
        .reenter(&outer, inner_entry, vec![root], 0, None)
        .unwrap();
    let inner = read(&mut session, "mem:check:inner");
    session.cancel();
    assert!(
        matches!(event(&mut session), HostPoll::Ready(Err(error)) if error.kind == pkl_runtime::value::ErrorKind::Cancelled)
    );
    assert_eq!(session.drain_cancelled_reads(), [outer, inner]);
    assert_eq!(session.context_depth(), 0);
    assert_eq!(session.heap_stats().slots, 0);
}

#[test]
fn forbidden_generator_body_parameters_remain_explicit_upstream_error_boundaries() {
    for source in [
        "result = new Listing { for (x in List(1)) { p -> x } }",
        "result = new Dynamic { when (false) { p -> value = 1 } }",
        "result = new Dynamic { when (true) { value = 1 } else { p -> value = 2 } }",
    ] {
        let parsed = parse_module(source).unwrap();
        let failure = resolve_module(&parsed).expect_err("generator body parameter failure");
        assert_eq!(
            failure.provenance.reason,
            Some("parameterized-generator-body")
        );
    }
}

#[test]
fn explicit_mixin_aliases_delegate_default_selection_by_declaration_identity() {
    let source = r#"
      typealias Alias<T> = Mixin<T>
      typealias Chain<T> = Alias<T>
      typealias Structural<T> = (T) -> T
      local first = new Alias<Dynamic> { original -> value = original.value + 1 }
      local second = new Chain<Dynamic> { original -> value = original.value + 2 }
      local seed = new Dynamic { value = 4 }
      firstValue = first.apply(seed).value
      secondValue = second.apply(seed).value
      missingDefault = new Structural<Dynamic> { original -> value = original.value }
    "#;
    assert_eq!(evaluate(source, "firstValue"), Ok(Value::Int(5.into())));
    assert_eq!(evaluate(source, "secondValue"), Ok(Value::Int(6.into())));
    assert_eq!(
        evaluate(source, "missingDefault").unwrap_err().message,
        "Cannot instantiate, or amend an instance of, external class `Function1`."
    );
}

#[test]
fn selected_mixin_defaults_preserve_nullable_union_and_constraint_wrappers() {
    // Java cohorts 10/11 pin these default and declaration-identity distinctions.
    let source = r#"
      typealias Maybe<T> = Mixin<T>?
      typealias Pick<T> = *Mixin<T> | String
      typealias Checked<T> = Mixin<T>(false)
      local maybe = new Maybe<Dynamic> { arg -> value = arg.value + 1 }
      local pick = new Pick<Dynamic> { arg -> value = arg.value + 2 }
      local checked = new Checked<Dynamic> { arg -> value = 1 }
      maybeValue = maybe.apply(new Dynamic { value = 4 }).value
      pickValue = pick.apply(new Dynamic { value = 4 }).value
      checkedError = checked
    "#;
    assert_eq!(evaluate(source, "maybeValue"), Ok(Value::Int(5.into())));
    assert_eq!(evaluate(source, "pickValue"), Ok(Value::Int(6.into())));
    assert_eq!(
        evaluate(source, "checkedError").unwrap_err().kind,
        pkl_runtime::value::ErrorKind::Type
    );
    let shadow = "typealias Mixin<T> = (T) -> T\nresult = new Mixin<Dynamic> { arg -> value = 1 }";
    assert_eq!(
        evaluate(shadow, "result").unwrap_err().message,
        "Cannot instantiate, or amend an instance of, external class `Function1`."
    );
}
