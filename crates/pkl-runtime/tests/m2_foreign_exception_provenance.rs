//! Core provenance tests, not full Java diagnostic compatibility claims.
//! Historical raw diagnostics remain pending until JVM/API projection exists.

use std::sync::Arc;

use pkl_runtime::host::{HostPoll, HostSession};
use pkl_runtime::java_exception::provenance::{ForeignExceptionProvenance, ForeignOperation};
use pkl_runtime::logging::EvaluationContext;
use pkl_runtime::modules::ModuleIdentity;
use pkl_runtime::parser::parse_module;
use pkl_runtime::resolver::resolve_module;
use pkl_runtime::value::{ErrorKind, RuntimeError, RuntimeSource, Value};
use pkl_runtime::vm::{Instruction, compile_module};

fn opened(source: &str, uri: &str) -> (HostSession, Value, usize) {
    let mut program = (*compile_module(&resolve_module(&parse_module(source).unwrap()).unwrap())
        .unwrap())
    .clone();
    let primary_source = Arc::new(RuntimeSource {
        text: source.into(),
        uri: uri.into(),
        module_name: program.module_name.clone(),
    });
    program
        .register_primary_source(
            primary_source.clone(),
            ModuleIdentity {
                original_uri: uri.into(),
                resolved_uri: uri.into(),
            },
        )
        .unwrap();
    let entry = program.entry;
    let get = program.blocks.len();
    program.blocks.push(vec![
        Instruction::Input(0),
        Instruction::GetMember("result".into()),
        Instruction::Return,
    ]);
    let mut session = HostSession::with_context(
        Arc::new(program),
        EvaluationContext {
            primary_source: Some(primary_source),
            ..EvaluationContext::default()
        },
    );
    session.start(entry, vec![], 0, None).unwrap();
    let module = drive(&mut session, &Value::Null, &mut Vec::new()).unwrap();
    (session, module, get)
}

fn next(session: &mut HostSession, module: &Value) -> HostPoll {
    for _ in 0..200_000 {
        session.collect(std::slice::from_ref(module));
        match session.poll(0, 1) {
            HostPoll::Yielded => {}
            HostPoll::Module { token, request } => session
                .complete_module(
                    &token,
                    pkl_runtime::modules::default_completion(&request)
                        .expect("pinned builtin module"),
                    0,
                )
                .unwrap(),
            event => return event,
        }
    }
    panic!("foreign provenance test did not settle");
}

fn drive(
    session: &mut HostSession,
    module: &Value,
    reads: &mut Vec<String>,
) -> Result<Value, RuntimeError> {
    loop {
        match next(session, module) {
            HostPoll::Ready(result) => return result,
            HostPoll::Read { token, uri } => {
                reads.push(uri.to_java_utf8());
                session
                    .resume_default_read(&token, &[("divisor".into(), "0".into())], &[], 0)
                    .unwrap();
            }
            event => panic!("unexpected Host event: {event:?}"),
        }
    }
}

fn evaluate(source: &str) -> (Result<Value, RuntimeError>, Vec<String>) {
    let (mut session, module, get) = opened(source, "file:///foreign-provenance/main.pkl");
    session.start(get, vec![module.clone()], 0, None).unwrap();
    let mut reads = Vec::new();
    let result = drive(&mut session, &module, &mut reads);
    session.close();
    (result, reads)
}

fn cause(error: &RuntimeError) -> &ForeignExceptionProvenance {
    assert_eq!(error.kind, ErrorKind::Unsupported);
    let cause = error.foreign_exception.as_deref().expect("foreign cause");
    let origin = cause.origin.as_ref().expect("pre-unwind origin");
    assert!(origin.vm_entry().is_some());
    assert!(origin.declared_root_candidates().next().is_some());
    cause
}

#[test]
fn remainder_waits_for_operands_and_preserves_the_authored_root() {
    let source = "result = 17 % read(\"prop:divisor\").toInt()\n";
    let (result, reads) = evaluate(source);
    assert_eq!(reads, ["prop:divisor"]);
    let error = result.unwrap_err();
    let cause = cause(&error);
    assert_eq!(
        cause.operation,
        ForeignOperation::IntegerRemainderByZero { dividend: 17 }
    );
    assert_eq!(cause.class_name, "java.lang.ArithmeticException");
    assert_eq!(cause.message, "/ by zero");
    let origin = cause.origin.as_ref().unwrap();
    let declared = origin.declared_root_candidates().next().unwrap();
    // compile_module has no per-activation source for the primary module.
    // The explicitly supplied evaluation source remains a separate provenance.
    assert!(declared.source.is_none());
    let retained = origin.primary_source.as_ref().unwrap();
    assert_eq!(retained.uri, "file:///foreign-provenance/main.pkl");
    assert_eq!(retained.text, source.into());

    let (first, reads) = evaluate("result = (1 ~/ 0) % read(\"prop:later\").toInt()\n");
    let first = first.unwrap_err();
    assert_eq!(first.kind, ErrorKind::Arithmetic);
    assert!(first.foreign_exception.is_none());
    assert!(
        reads.is_empty(),
        "left failure must prevent the right Host callback"
    );
    assert_eq!(evaluate("result = 17 % 3\n").0, Ok(Value::Int(2.into())));
    let Value::Float(value) = evaluate("result = 1.0 % 0\n").0.unwrap() else {
        panic!("float remainder")
    };
    assert!(value.get().is_nan());
}

#[test]
fn repeat_retains_original_count_after_java_narrowing_and_checked_length() {
    for (count, narrowed) in [(2_147_483_648_i64, i32::MIN), (i64::MAX, -1)] {
        let (result, reads) = evaluate(&format!("result = \"\".repeat({count})\n"));
        assert!(reads.is_empty());
        let error = result.unwrap_err();
        let cause = cause(&error);
        assert_eq!(
            cause.operation,
            ForeignOperation::StringRepeatNegativeCount {
                length_utf16: 0,
                requested_count: count,
                narrowed_count: narrowed,
            }
        );
        assert_eq!(cause.class_name, "java.lang.IllegalArgumentException");
        assert_eq!(
            cause.message.to_java_utf8(),
            format!("count is negative: {narrowed}")
        );
        let declared = cause
            .origin
            .as_ref()
            .unwrap()
            .declared_root_candidates()
            .next()
            .unwrap();
        assert_eq!(declared.member.as_ref().unwrap(), "#String.repeat");
        assert_eq!(declared.source.as_ref().unwrap().uri, "pkl:base");
        assert!(
            declared.instruction_span.is_none(),
            "external body has no authored section"
        );
    }
    for source in [
        "result = \"a\".repeat(2147483648)\n",
        "result = \"ab\".repeat(9223372036854775807)\n",
    ] {
        let error = evaluate(source).0.unwrap_err();
        assert_eq!(error.kind, ErrorKind::Arithmetic);
        assert!(
            error.foreign_exception.is_none(),
            "checked length must win before narrowing"
        );
    }
    let error = evaluate("result = \"\".repeat(-1)\n").0.unwrap_err();
    assert_eq!(error.kind, ErrorKind::Type);
    assert!(
        error.foreign_exception.is_none(),
        "UInt validation precedes the external body"
    );
    assert_eq!(
        evaluate("result = \"\".repeat(4294967296)\n").0,
        Ok(Value::string(""))
    );
}

#[test]
fn malformed_http_construction_precedes_policy_without_mislabeling_valid_authorities() {
    for uri in ["http:opaque", "http://bad_host/x", "http://host:bad/x"] {
        let (result, reads) = evaluate(&format!("result = read(\"{uri}\")\n"));
        assert_eq!(reads, [uri]);
        let error = result.unwrap_err();
        let cause = cause(&error);
        assert_eq!(
            cause.operation,
            ForeignOperation::HttpRequestWithoutHost { uri: uri.into() }
        );
        assert_eq!(cause.class_name, "java.lang.IllegalArgumentException");
        assert_eq!(
            cause.message.to_java_utf8(),
            format!("unsupported URI {uri}")
        );
    }
    let valid_denied = evaluate("result = read(\"http://host/x\")\n")
        .0
        .unwrap_err();
    assert_eq!(valid_denied.kind, ErrorKind::Host);
    assert!(valid_denied.foreign_exception.is_none());
    for uri in ["http://user@host/x", "http://[::1]/x"] {
        let error = evaluate(&format!("result = read(\"{uri}\")\n"))
            .0
            .unwrap_err();
        assert_eq!(error.kind, ErrorKind::Unsupported);
        assert!(
            error.foreign_exception.is_none(),
            "valid but unmodeled authority: {uri}"
        );
    }
    let syntax = evaluate("result = read(\"http://host/%zz\")\n")
        .0
        .unwrap_err();
    assert!(
        syntax.foreign_exception.is_none(),
        "URI syntax parsing must precede request construction"
    );
}

#[test]
fn original_origin_survives_unwind_host_rethrow_and_session_close() {
    let (mut original, module, get) = opened("result = 1 % 0\n", "file:///original/main.pkl");
    original.set_power_assertions(true);
    original.start(get, vec![module.clone()], 0, None).unwrap();
    let first = drive(&mut original, &module, &mut Vec::new()).unwrap_err();
    let saved = first.foreign_exception.as_ref().unwrap().clone();
    cause(&first);
    original.start(get, vec![module.clone()], 0, None).unwrap();
    let retried = drive(&mut original, &module, &mut Vec::new()).unwrap_err();
    assert_eq!(cause(&first), cause(&retried));
    original.close();
    drop(original);

    let (mut later, module, get) = opened(
        "result = read(\"prop:later\")\n",
        "file:///rethrow/main.pkl",
    );
    later.start(get, vec![module.clone()], 0, None).unwrap();
    let HostPoll::Read { token, .. } = next(&mut later, &module) else {
        panic!("expected pending rethrow")
    };
    later.resume(&token, Err(first.clone()), 0).unwrap();
    let rethrown = drive(&mut later, &module, &mut Vec::new()).unwrap_err();
    later.close();
    drop(later);
    assert!(Arc::ptr_eq(
        &saved,
        rethrown.foreign_exception.as_ref().unwrap()
    ));
    assert!(
        saved.origin.as_ref().unwrap().power_assertions,
        "rethrow must not replace the original VM mode"
    );
    assert_eq!(cause(&first), cause(&rethrown));
    let original_origin = saved.origin.as_ref().unwrap();
    let original_root = original_origin.declared_root_candidates().next().unwrap();
    assert!(original_root.source.is_none());
    let retained = original_origin.primary_source.as_ref().unwrap();
    assert_eq!(retained.uri, "file:///original/main.pkl");
    assert_eq!(retained.text, "result = 1 % 0\n".into());
}
