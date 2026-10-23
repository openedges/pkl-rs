//! Original Java setup failures retain complete diagnostic text through frontend APIs.
use pkl_runtime::evaluation::frontend::{
    DiagnosticProfile, FailureDisposition, FrontendPhase, ModuleEvalOptions, ModuleFrontendFailure,
    ModuleFrontendResult, ModuleSourceInput, prepare_module, prepare_module_text,
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

#[test]
fn original_java_duplicate_error_survives_frontend_and_session_setup() {
    // The Java oracle source, physical URI and complete payloads are unchanged.
    // This case checks setup routing; unpaired runtime payloads have separate tests.
    let options = ModuleEvalOptions {
        diagnostic_profile: DiagnosticProfile::ApiIdentity,
        ..Default::default()
    };
    assert_canonical_failure(prepare_module_text(input(), &options).err().unwrap());
    assert_byte_failure(prepare_module(input(), &options).err().unwrap());
    assert_canonical_failure(
        SourceSession::compile_output_text(input(), &[], &options, SourceSessionOptions::default())
            .err()
            .unwrap(),
    );
    assert_canonical_failure(
        SourceProbe::new_output_text(input(), &[], &options, SourceSessionOptions::default())
            .err()
            .unwrap(),
    );
    let failure =
        SourceSession::compile_text(SOURCE, URI, "main", &[], SourceSessionOptions::default())
            .err()
            .unwrap();
    assert_eq!(failure.encode_utf16().collect::<Vec<_>>(), expected_units());
    assert_eq!(failure.to_java_utf8().as_bytes(), JAVA_UTF8);
}
