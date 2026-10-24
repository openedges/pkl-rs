//! Development-only arbitrary-expression capture, with exact fixture identities.
//! Usage: SOURCE URI MODULE EXPRESSION [RESOURCE_URI RESOURCE_PATH ...].
//! Java's expression-string profile uses compact diagnostics, a noop logger,
//! the default frame transformer, and disabled power assertions. This adapter
//! records its own Host/lifecycle ledger separately; it does not claim Java
//! callback agreement or manufacture Java exception classes.
use pkl_runtime::{
    diagnostic::FailureDisposition,
    evaluation::frontend::{
        DiagnosticProfile, ModuleEvalOptions, ModuleSourceInput, prepare_module_text,
    },
    probe::SourceProbe,
    value::PklString,
};
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{self, Write},
};

type AdapterResult<T> = Result<T, String>;

struct Input {
    source: String,
    uri: String,
    name: String,
    expression: String,
    resources: BTreeMap<String, Vec<u8>>,
}

#[derive(Default)]
struct Ledger {
    file: Option<File>,
}

impl Ledger {
    fn record(&mut self, key: &str, value: impl std::fmt::Display) -> AdapterResult<()> {
        if let Some(file) = &mut self.file {
            writeln!(file, "{key}\t{value}").map_err(|error| error.to_string())?;
        }
        Ok(())
    }

    fn bytes(&mut self, key: &str, value: &[u8]) -> AdapterResult<()> {
        let mut hex = String::with_capacity(value.len() * 2);
        for byte in value {
            use std::fmt::Write as _;
            write!(hex, "{byte:02x}").expect("String write");
        }
        self.record(key, hex)
    }

    fn units(&mut self, key: &str, value: &[u16]) -> AdapterResult<()> {
        self.bytes(
            key,
            &value
                .iter()
                .flat_map(|unit| unit.to_be_bytes())
                .collect::<Vec<_>>(),
        )
    }
}

struct Observation {
    exit: i32,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

fn constructor_failure(
    input: &Input,
    error: PklString,
    ledger: &mut Ledger,
) -> AdapterResult<Observation> {
    // Recover discarded provenance through the public structured frontend only
    // when its entire diagnostic equals the original constructor diagnostic.
    // This pass never evaluates a guest expression or supplies expected bytes.
    let options = ModuleEvalOptions {
        diagnostic_profile: DiagnosticProfile::ApiIdentity,
        ..Default::default()
    };
    let meaning = prepare_module_text(
        ModuleSourceInput {
            text: &input.source,
            uri: &input.uri,
            inferred_name: &input.name,
        },
        &options,
    )
    .err()
    .filter(|failure| failure.text == error);
    ledger.record("session_constructed", false)?;
    ledger.units(
        "constructor_error_utf16be_hex",
        &error.encode_utf16().collect::<Vec<_>>(),
    )?;
    let Some(meaning) = meaning else {
        return Err(format!(
            "expression constructor transport failed: {}",
            error.to_java_utf8()
        ));
    };
    ledger.record("phase", meaning.phase.as_str())?;
    ledger.record("disposition", meaning.disposition.as_str())?;
    ledger.record("reason", meaning.reason.unwrap_or(""))?;
    let exit = if meaning.disposition == FailureDisposition::LanguageError {
        1
    } else {
        2
    };
    ledger.record("guest_exit_code", exit)?;
    Ok(Observation {
        exit,
        stdout: Vec::new(),
        stderr: error.to_java_utf8().into_bytes(),
    })
}

fn capture(input: &Input, ledger: &mut Ledger) -> AdapterResult<Observation> {
    ledger.record("profile", "arbitrary-expression-noop-logger-power-off-v1")?;
    ledger.bytes("source_utf8_hex", input.source.as_bytes())?;
    ledger.bytes("source_uri_utf8_hex", input.uri.as_bytes())?;
    ledger.bytes("module_name_utf8_hex", input.name.as_bytes())?;
    ledger.bytes("expression_utf8_hex", input.expression.as_bytes())?;
    let mut probe = match SourceProbe::new_expressions_text(
        &input.source,
        &input.uri,
        &input.name,
        std::slice::from_ref(&input.expression),
        "compact",
        false,
        false,
        false,
    ) {
        Ok(probe) => probe,
        Err(error) => return constructor_failure(input, error, ledger),
    };
    ledger.record("session_constructed", true)?;
    ledger.record("setup_failed", probe.setup_failed())?;
    let mut polls = 0usize;
    let mut collections = 0usize;
    let mut reads = 0usize;
    let mut modules = 0usize;
    let result = (|| {
        probe
            .start_path(0, 0, None)
            .map_err(|error| format!("expression admission: {error:?}"))?;
        for _ in 0..1_000_000 {
            probe.collect();
            collections += 1;
            let event = probe
                .poll(0, 1)
                .map_err(|error| format!("expression poll: {error:?}"))?;
            polls += 1;
            probe.collect();
            collections += 1;
            if event.kind != "yield" {
                ledger.record("event", event.kind)?;
            }
            match event.kind {
                "yield" => {}
                "module-request" => {
                    modules += 1;
                    let token = event.token.as_deref().ok_or("missing module token")?;
                    let request = probe
                        .module_request(token)
                        .map_err(|error| format!("module request: {error:?}"))?;
                    ledger.bytes("module_request_utf8_hex", format!("{request:?}").as_bytes())?;
                    if !probe
                        .complete_default_module(token, 0)
                        .map_err(|error| format!("default module completion: {error:?}"))?
                    {
                        return Err(format!("unconfigured module request: {request:?}"));
                    }
                }
                "read" => {
                    reads += 1;
                    let token = event.token.as_deref().ok_or("missing read token")?;
                    let uri = probe
                        .resolved_read_uri(token)
                        .map_err(|error| format!("resolved resource URI: {error:?}"))?;
                    let requested = event.uri.as_deref().ok_or("missing authored read URI")?;
                    ledger.bytes("read_requested_uri_utf8_hex", requested.as_bytes())?;
                    ledger.bytes("read_resolved_uri_utf8_hex", uri.as_bytes())?;
                    // The event retains the authored operand (e.g. gate.txt).
                    // The shared runtime resolves it against the source URI;
                    // only that exact resolved identity selects fixture bytes.
                    let bytes = input
                        .resources
                        .get(&uri)
                        .ok_or_else(|| format!("unconfigured fixture resource URI: {uri}"))?;
                    ledger.bytes("read_bytes_hex", bytes)?;
                    probe
                        .complete_read_resource(token, uri.into(), bytes.clone(), 0)
                        .map_err(|error| format!("resource completion: {error:?}"))?;
                }
                "value" | "error" => {
                    ledger.record("phase", event.phase.unwrap_or(""))?;
                    ledger.record("disposition", event.disposition.unwrap_or(""))?;
                    ledger.record("reason", event.reason.unwrap_or(""))?;
                    let (exit, stdout, stderr) = if event.kind == "value" {
                        let units = event.value.ok_or("missing result code units")?;
                        ledger.units("value_utf16be_hex", &units)?;
                        (
                            0,
                            PklString::from_utf16(units).to_java_utf8().into_bytes(),
                            Vec::new(),
                        )
                    } else {
                        let units = event.error.ok_or("missing error code units")?;
                        ledger.units("error_utf16be_hex", &units)?;
                        (
                            1,
                            Vec::new(),
                            PklString::from_utf16(units).to_java_utf8().into_bytes(),
                        )
                    };
                    ledger.record("guest_exit_code", exit)?;
                    return Ok(Observation {
                        exit,
                        stdout,
                        stderr,
                    });
                }
                other => return Err(format!("unexpected expression event: {other}")),
            }
        }
        Err("expression capture poll limit exceeded".into())
    })();
    // Close even when acquisition fails. These are actual Rust observations,
    // separate from raw guest bytes and from unobserved Java lifecycle events.
    let slots_before_close = probe.heap_slots();
    let depth_before_close = probe.context_depth();
    probe.close();
    let final_heap = probe.collect();
    collections += 1;
    ledger.record("slots_before_close", slots_before_close)?;
    ledger.record("depth_before_close", depth_before_close)?;
    ledger.record("polls", polls)?;
    ledger.record("collections", collections)?;
    ledger.record("reads", reads)?;
    ledger.record("module_requests", modules)?;
    ledger.record("terminal_slots", final_heap.slots)?;
    ledger.record("terminal_depth", probe.context_depth())?;
    if final_heap.slots != 0 || probe.context_depth() != 0 {
        return Err("expression close retained guest heap slots or contexts".into());
    }
    result
}

fn input(arguments: &[String]) -> AdapterResult<Input> {
    if arguments.len() < 5 || !(arguments.len() - 5).is_multiple_of(2) {
        return Err(
            "expected SOURCE URI MODULE EXPRESSION [RESOURCE_URI RESOURCE_PATH ...]".into(),
        );
    }
    let mut resources = BTreeMap::new();
    for pair in arguments[5..].as_chunks::<2>().0 {
        if resources
            .insert(
                pair[0].clone(),
                fs::read(&pair[1]).map_err(|error| format!("fixture resource input: {error}"))?,
            )
            .is_some()
        {
            return Err(format!("duplicate fixture resource URI: {}", pair[0]));
        }
    }
    Ok(Input {
        source: fs::read_to_string(&arguments[1])
            .map_err(|error| format!("source input: {error}"))?,
        uri: arguments[2].clone(),
        name: arguments[3].clone(),
        expression: arguments[4].clone(),
        resources,
    })
}

fn main() {
    let mut ledger = Ledger::default();
    if let Some(path) = std::env::var_os("PKL_FIXTURE_TRACE") {
        match File::create_new(path) {
            Ok(file) => ledger.file = Some(file),
            Err(error) => {
                eprintln!("expression fixture trace: {error}");
                std::process::exit(2);
            }
        }
    }
    let result =
        input(&std::env::args().collect::<Vec<_>>()).and_then(|input| capture(&input, &mut ledger));
    let observation = match result {
        Ok(observation) => observation,
        Err(error) => {
            let _ = ledger.bytes("adapter_error_utf8_hex", error.as_bytes());
            Observation {
                exit: 2,
                stdout: Vec::new(),
                stderr: format!("expression fixture adapter: {error}\n").into_bytes(),
            }
        }
    };
    let write = io::stdout()
        .lock()
        .write_all(&observation.stdout)
        .and_then(|()| io::stderr().lock().write_all(&observation.stderr));
    if write.is_err() {
        std::process::exit(2);
    }
    std::process::exit(observation.exit);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arbitrary_expression_and_exact_resource_bytes_use_shared_continuations() {
        let input = Input {
            source: "f = (x) -> x + read(\"file:///fixture/gate.txt\").text".into(),
            uri: "file:///original/location/source.pkl".into(),
            name: "source".into(),
            expression: "f.apply(\"value/\") + \"end\"".into(),
            resources: BTreeMap::from([("file:///fixture/gate.txt".into(), b"go\n".to_vec())]),
        };
        let result = capture(&input, &mut Ledger::default()).unwrap();
        assert_eq!(
            result.exit,
            0,
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(result.stdout, b"value/go\nend");
        assert!(result.stderr.is_empty());
    }

    #[test]
    fn unconfigured_read_is_an_adapter_failure_with_no_fabricated_response() {
        let input = Input {
            source: "value = read(\"missing.txt\")".into(),
            uri: "file:///original/location/source.pkl".into(),
            name: "source".into(),
            expression: "value".into(),
            // An authored operand is not an admissible substitute for the
            // missing resolved URI, even when its bytes have been supplied.
            resources: BTreeMap::from([("missing.txt".into(), b"must-not-read".to_vec())]),
        };
        assert_eq!(
            capture(&input, &mut Ledger::default()).err().as_deref(),
            Some("unconfigured fixture resource URI: file:///original/location/missing.txt"),
        );
    }

    #[test]
    fn relative_read_uses_exact_resolved_resource_identity_and_preserves_bytes() {
        let input = Input {
            source: "f = (x) -> x + read(\"gate.txt\").text".into(),
            uri: "file:///original/location/source.pkl".into(),
            name: "source".into(),
            expression: "f.apply(\"value/\") + \"end\"".into(),
            resources: BTreeMap::from([(
                "file:///original/location/gate.txt".into(),
                b"go\n".to_vec(),
            )]),
        };
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../target/m2/stage53-native-relative-read-unit-{}-{stamp}",
            std::process::id(),
        ));
        fs::create_dir(&directory).unwrap();
        let trace_path = directory.join("trace.tsv");
        let mut ledger = Ledger {
            file: Some(File::create_new(&trace_path).unwrap()),
        };
        let result = capture(&input, &mut ledger).unwrap();
        drop(ledger);
        assert_eq!(
            result.exit,
            0,
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(result.stdout, b"value/go\nend");
        assert!(result.stderr.is_empty());
        let trace = fs::read_to_string(trace_path).unwrap();
        assert!(trace.contains("read_requested_uri_utf8_hex\t676174652e747874\n"));
        assert!(trace.contains("read_resolved_uri_utf8_hex\t66696c653a2f2f2f6f726967696e616c2f6c6f636174696f6e2f676174652e747874\n"));
    }

    #[test]
    fn constructor_language_errors_retain_raw_diagnostics_and_language_exit() {
        let input = Input {
            source: "value = )".into(),
            uri: "file:///original/location/syntax.pkl".into(),
            name: "syntax".into(),
            expression: "value".into(),
            resources: BTreeMap::new(),
        };
        let result = capture(&input, &mut Ledger::default()).unwrap();
        assert_eq!(result.exit, 1);
        assert!(result.stdout.is_empty());
        let error = String::from_utf8(result.stderr).unwrap();
        assert!(error.starts_with("–– Pkl Error ––\n"));
        assert!(error.contains("file:///original/location/syntax.pkl"));
        assert!(!error.contains("expression fixture adapter"));
    }
}
