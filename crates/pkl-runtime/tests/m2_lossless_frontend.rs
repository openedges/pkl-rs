//! Original Java setup failures retain complete diagnostic text through frontend APIs.
use pkl_runtime::evaluation::frontend::{
    DiagnosticProfile, FailureDisposition, FrontendPhase, ModuleEvalOptions, ModuleFrontendFailure,
    ModuleFrontendResult, ModuleSourceInput, PreparedModule, prepare_module, prepare_module_text,
    render_module_runtime_failure_text,
};
use pkl_runtime::evaluation::session::{SourceSession, SourceSessionOptions};
use pkl_runtime::probe::SourceProbe;

const SOURCE: &str =
    include_str!("../../../tests/fixtures/m2-module-duplicates/lambda-property-header/input.pkl");
const URI: &str = "file:///Users/cody.ij.hwang/Projects/pkl-rs.stage42/target/m2/stage42-duplicate-members-kind-oracle-01/workspace/lambda-property-header/main.pkl";
const JAVA_UNITS: &[u8] = include_bytes!(
    "../../../tests/fixtures/m2-module-duplicates/lambda-property-header/0/calls/outer-0.stderr.code-units.bin"
);
const JAVA_UTF8: &[u8] = include_bytes!(
    "../../../tests/fixtures/m2-module-duplicates/lambda-property-header/0/calls/outer-0.stderr.bin"
);

fn input() -> ModuleSourceInput<'static> {
    ModuleSourceInput {
        text: SOURCE,
        uri: URI,
        inferred_name: "main",
    }
}

fn expected_units() -> Vec<u16> {
    let (units, remainder) = JAVA_UNITS.as_chunks::<2>();
    assert!(remainder.is_empty());
    units.iter().map(|unit| u16::from_be_bytes(*unit)).collect()
}

fn assert_byte_failure(failure: ModuleFrontendResult) {
    // Duplicate registration is classified by the shared resolver.
    assert_eq!(failure.phase, FrontendPhase::Resolution);
    assert_eq!(failure.disposition, FailureDisposition::LanguageError);
    assert_eq!(failure.reason, None);
    assert_eq!(failure.exit_code, 1);
    assert!(failure.stdout.is_empty());
    assert_eq!(failure.stderr, JAVA_UTF8);
}

fn assert_canonical_failure(failure: ModuleFrontendFailure) {
    assert_eq!(
        failure.text.encode_utf16().collect::<Vec<_>>(),
        expected_units()
    );
    assert_byte_failure(failure.into());
}

fn prepared_failure(prepared: PreparedModule) -> ModuleFrontendFailure {
    let entry = prepared.program.entry;
    let mut vm = pkl_runtime::vm::Vm::new(prepared.program);
    vm.start(entry, Vec::new()).unwrap();
    for _ in 0..100_000 {
        match vm.poll(1) {
            pkl_runtime::vm::VmPoll::Yielded => (),
            pkl_runtime::vm::VmPoll::Ready(Err(error)) => {
                return render_module_runtime_failure_text(
                    &prepared.source,
                    URI,
                    &prepared.name,
                    DiagnosticProfile::ApiIdentity,
                    error,
                );
            }
            other => panic!("unexpected setup effect: {other:?}"),
        }
    }
    panic!("setup did not finish");
}
fn session_failure(mut session: SourceSession) {
    session.start_module(0, None).unwrap();
    for _ in 0..100_000 {
        match session.poll(0, 1) {
            pkl_runtime::host::HostPoll::Yielded => (),
            pkl_runtime::host::HostPoll::Ready(Err(error)) => {
                let failure = session.format_error_text(error);
                assert_eq!(failure.encode_utf16().collect::<Vec<_>>(), expected_units());
                assert_eq!(failure.to_java_utf8().as_bytes(), JAVA_UTF8);
                return;
            }
            other => panic!("unexpected setup effect: {other:?}"),
        }
    }
    panic!("setup did not finish");
}

#[test]
fn original_java_duplicate_error_survives_frontend_and_session_setup() {
    // Java source, URI, expected UTF-16 and UTF-8 remain unchanged. AST setup
    // failures now complete through the VM so earlier ModuleKey effects can run.
    let options = ModuleEvalOptions {
        diagnostic_profile: DiagnosticProfile::ApiIdentity,
        ..Default::default()
    };
    assert_canonical_failure(prepared_failure(
        prepare_module_text(input(), &options).unwrap(),
    ));
    assert_byte_failure(prepared_failure(prepare_module(input(), &options).unwrap()).into());
    session_failure(
        SourceSession::compile_output_text(input(), &[], &options, SourceSessionOptions::default())
            .unwrap(),
    );
    session_failure(
        SourceSession::compile_text(SOURCE, URI, "main", &[], SourceSessionOptions::default())
            .unwrap(),
    );
    let mut probe =
        SourceProbe::new_output_text(input(), &[], &options, SourceSessionOptions::default())
            .unwrap();
    probe.start_module(0, None).unwrap();
    for _ in 0..100_000 {
        let event = probe.poll(0, 1).unwrap();
        if event.kind == "yield" {
            continue;
        }
        assert_eq!(event.kind, "error");
        assert_eq!(event.error.as_ref().unwrap(), &expected_units());
        let text = pkl_runtime::value::PklString::from_utf16(event.error.unwrap());
        assert_eq!(text.to_java_utf8().as_bytes(), JAVA_UTF8);
        return;
    }
    panic!("probe setup did not finish");
}
