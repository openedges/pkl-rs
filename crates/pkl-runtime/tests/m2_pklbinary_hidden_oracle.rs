//! Raw `Evaluator.evaluateOutputBytes` observations; no string or hex projection.
use pkl_runtime::{
    evaluation::{
        EvaluationOptions,
        frontend::{
            DiagnosticProfile, FailureDisposition, ModuleEvalOptions, ModuleFrontendFailure,
            ModuleSourceInput,
        },
        session::{SourceSession, SourceSessionOptions},
    },
    host::HostPoll,
    modules::default_completion,
    value::{PklString, Value},
};
use std::{fs, path::Path};

const CASES: [&str; 2] = ["dynamic-inherited-local", "typed-hidden"];

struct Observation {
    status: &'static [u8],
    bytes: Vec<u8>,
    diagnostic: PklString,
    log_events: usize,
}

fn failure(failure: ModuleFrontendFailure, log_events: usize) -> Observation {
    Observation {
        status: match failure.disposition {
            FailureDisposition::LanguageError => b"error\n",
            FailureDisposition::Unsupported => b"unsupported\n",
            FailureDisposition::LimitExceeded => b"limit\n",
            FailureDisposition::InternalFailure => b"internal-failure\n",
        },
        bytes: Vec::new(),
        diagnostic: failure.text,
        log_events,
    }
}

fn evaluate(source: &str, uri: &str, power: bool) -> Observation {
    let options = ModuleEvalOptions {
        diagnostic_profile: DiagnosticProfile::ApiIdentity,
        ..ModuleEvalOptions::default()
    };
    let mut session = match SourceSession::compile_output_text(
        ModuleSourceInput {
            text: source,
            uri,
            inferred_name: "main",
        },
        &[],
        &options,
        SourceSessionOptions {
            evaluation: EvaluationOptions {
                power_assertions: power,
            },
            ..SourceSessionOptions::default()
        },
    ) {
        Ok(session) => session,
        Err(error) => return failure(error, 0),
    };
    if let Err(error) = session.start_output_bytes(0, None) {
        let result = failure(session.output_failure_text(error), 0);
        session.close();
        return result;
    }
    let mut logs = 0;
    for _ in 0..500_000 {
        match session.poll(0, 32) {
            HostPoll::Yielded => (),
            HostPoll::Module { token, request } => {
                let Some(completion) = default_completion(&request) else {
                    session.close();
                    return Observation {
                        status: b"unsupported-host\n",
                        bytes: Vec::new(),
                        diagnostic: PklString::default(),
                        log_events: logs,
                    };
                };
                session.complete_module(&token, completion, 0).unwrap();
            }
            HostPoll::Log { token, .. } => {
                logs += 1;
                session.acknowledge_log(&token, Ok(()), 0).unwrap();
            }
            HostPoll::Ready(result) => {
                let actual = match result {
                    Ok(Value::Bytes(bytes)) => Observation {
                        status: b"success\n",
                        bytes: bytes.into_data().to_vec(),
                        diagnostic: PklString::default(),
                        log_events: logs,
                    },
                    Ok(_) => Observation {
                        status: b"non-bytes\n",
                        bytes: Vec::new(),
                        diagnostic: PklString::default(),
                        log_events: logs,
                    },
                    Err(error) => failure(session.output_failure_text(error), logs),
                };
                session.close();
                return actual;
            }
            _ => {
                session.close();
                return Observation {
                    status: b"unexpected-host-event\n",
                    bytes: Vec::new(),
                    diagnostic: PklString::default(),
                    log_events: logs,
                };
            }
        }
    }
    session.close();
    Observation {
        status: b"iteration-limit\n",
        bytes: Vec::new(),
        diagnostic: PklString::default(),
        log_events: logs,
    }
}

fn read(folder: &Path, name: &str) -> Vec<u8> {
    fs::read(folder.join(name))
        .unwrap_or_else(|error| panic!("{}: {error}", folder.join(name).display()))
}

#[test]
fn java_binary_hidden_inheritance_raw_fields_match() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/m2-pklbinary-hidden-oracle");
    let roster = String::from_utf8(read(&root, "cases.txt")).unwrap();
    assert_eq!(roster.lines().collect::<Vec<_>>(), CASES);
    let mut mismatches = Vec::new();
    for case in CASES {
        let folder = root.join(case);
        let source = String::from_utf8(read(&folder, "module.pkl")).unwrap();
        let uri = String::from_utf8(read(&folder, "uri.txt")).unwrap();
        assert_eq!(uri, "file:///stage144-pklbinary/main.pkl");
        for (mode, power) in [("on", true), ("off", false)] {
            let expected = folder.join(mode);
            let actual = evaluate(&source, &uri, power);
            let units = actual
                .diagnostic
                .encode_utf16()
                .flat_map(u16::to_be_bytes)
                .collect::<Vec<_>>();
            let diagnostic_bytes = actual.diagnostic.to_java_utf8().into_bytes();
            for (field, value) in [
                ("result.status.bin", actual.status),
                ("result.bytes.bin", actual.bytes.as_slice()),
                ("result.stderr.bin", diagnostic_bytes.as_slice()),
                ("result.stderr.code-units.bin", units.as_slice()),
            ] {
                if read(&expected, field) != value {
                    mismatches.push(format!("{case}/{mode}/{field} differs"));
                }
            }
            // Exception class is Java provenance, not a fabricated Rust class.
            // Its absence/presence is checked against the captured Java status.
            let java_class = read(&expected, "result.error-class.bin");
            let java_class_units = read(&expected, "result.error-class.code-units.bin");
            let java_error = read(&expected, "result.status.bin") == b"error\n";
            if java_class.is_empty() == java_error
                || java_class_units.is_empty() == java_error
                || actual.log_events != 0
            {
                mismatches.push(format!(
                    "{case}/{mode}: inconsistent Java class provenance or unexpected Rust log"
                ));
            }
        }
    }
    assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
}
