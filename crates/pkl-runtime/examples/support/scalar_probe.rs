//! Native filesystem adapter for the shared expression-string evaluator.
//! This development probe is not a complete product CLI.
use pkl_runtime::{probe::SourceProbe, value::PklString};
use std::{io::Write, path::Path};

#[path = "module_host.rs"]
mod module_host;
#[path = "resource_host.rs"]
mod resource_host;

#[derive(Debug)]
enum AdapterError {
    Unsupported(String),
    Internal(String),
}
impl AdapterError {
    fn exit_code(&self) -> i32 {
        match self {
            Self::Unsupported(_) => 2,
            Self::Internal(_) => 4,
        }
    }
    fn message(&self) -> &str {
        match self {
            Self::Unsupported(message) | Self::Internal(message) => message,
        }
    }
}

#[derive(Debug)]
struct Observation {
    exit_code: i32,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

type AdapterResult<T> = Result<T, AdapterError>;
fn internal(error: impl std::fmt::Display) -> AdapterError {
    AdapterError::Internal(error.to_string())
}
fn file_uri(path: &Path) -> AdapterResult<String> {
    // ModuleSource.path uses UnixPath.toUri's percent-encoded path spelling,
    // not the CLI's Unicode-preserving IoUtils.toUri spelling.
    module_host::file_uri(path)
        .map_err(|error| AdapterError::Unsupported(format!("original module URI: {error:?}")))
}
fn bytes(units: Option<Vec<u16>>, field: &str) -> AdapterResult<Vec<u8>> {
    units
        .map(|units| PklString::from_utf16(units).to_java_utf8().into_bytes())
        .ok_or_else(|| internal(format!("missing terminal {field}")))
}

fn run(arguments: &[String]) -> AdapterResult<Observation> {
    if arguments.len() != 2 && !(arguments.len() == 3 && arguments[2] == "--power-assertions") {
        return Err(AdapterError::Unsupported(
            "expected source path, expression, and optional --power-assertions".into(),
        ));
    }
    // ModuleSource.path uses an original absolute URI. Canonical resolution and
    // loading belong to the native ModuleKey Host, after evaluator admission.
    let supplied = Path::new(&arguments[0]);
    let path = if supplied.is_absolute() {
        supplied.to_path_buf()
    } else {
        // UnixPath.toAbsolutePath retains authored dot components. Joining the
        // cwd avoids std::path::absolute's removal of `.` components.
        std::env::current_dir().map_err(internal)?.join(supplied)
    };
    let uri = file_uri(&path)?;
    // EvaluatorBuilder.preconfigured supplies a noop Logger/default transformer;
    // M2LanguageProbe explicitly supplies empty environment/property maps.
    let mut probe =
        SourceProbe::new_module_evaluator("compact", false, false, arguments.len() == 3)
            .map_err(|error| internal(error.to_java_utf8()))?;
    let result = (|| {
        // Keep the exact query. The shared public operation loads/initializes
        // the module before parsing it, applies shared ToString, and dispatches
        // output.text directly without a synthetic expression/ToString frame.
        probe
            .evaluate_module_source(&uri, None, &arguments[1], 0, None)
            .map_err(internal)?;
        loop {
            let event = probe.poll(0, 1024).map_err(internal)?;
            probe.collect();
            match event.kind {
                "yield" => {}
                "module-request" => {
                    let token = event
                        .token
                        .as_deref()
                        .ok_or_else(|| internal("missing module token"))?;
                    let request = probe.module_request(token).map_err(internal)?;
                    probe
                        .complete_module(token, module_host::complete(&request), 0)
                        .map_err(internal)?;
                }
                "read" => {
                    let token = event
                        .token
                        .as_deref()
                        .ok_or_else(|| internal("missing read token"))?;
                    let request = probe.read_request(token).map_err(internal)?;
                    if let Some(completion) = resource_host::complete(&request) {
                        probe
                            .complete_read(token, completion, 0)
                            .map_err(internal)?;
                    } else {
                        probe.respond_configured(token, 0).map_err(internal)?;
                    }
                }
                "value" => {
                    return Ok(Observation {
                        exit_code: 0,
                        stdout: bytes(event.value, "value")?,
                        stderr: Vec::new(),
                    });
                }
                "error" => {
                    let exit_code = match event.disposition {
                        Some("language-error") => 1,
                        Some("unsupported") => 2,
                        Some("limit-exceeded") => 3,
                        Some("internal-failure") => 4,
                        other => {
                            return Err(internal(format!(
                                "unknown failure disposition: {other:?}"
                            )));
                        }
                    };
                    return Ok(Observation {
                        exit_code,
                        stdout: Vec::new(),
                        stderr: bytes(event.error, "error")?,
                    });
                }
                other => return Err(internal(format!("unexpected expression event: {other}"))),
            }
        }
    })();
    // Close on language and Host/API failures as well as success. The external
    // acquisition harness owns process deadlines; this adapter adds no VM limit.
    probe.close();
    let heap = probe.collect();
    if heap.slots != 0 || probe.context_depth() != 0 {
        return Err(internal("expression close retained heap slots or contexts"));
    }
    result
}

pub fn main() {
    let arguments = std::env::args_os()
        .skip(1)
        .map(|argument| {
            argument.into_string().map_err(|_| {
                AdapterError::Unsupported("expression adapter requires UTF-8 arguments".into())
            })
        })
        .collect::<AdapterResult<Vec<_>>>();
    let outcome = arguments.and_then(|arguments| run(&arguments));
    let result = outcome.and_then(|observation| {
        std::io::stdout()
            .write_all(&observation.stdout)
            .map_err(internal)?;
        std::io::stderr()
            .write_all(&observation.stderr)
            .map_err(internal)?;
        Ok(observation.exit_code)
    });
    let code = match result {
        Ok(code) => code,
        Err(error) => {
            let category = match error {
                AdapterError::Unsupported(_) => "unsupported",
                AdapterError::Internal(_) => "internal failure",
            };
            let _ = writeln!(
                std::io::stderr(),
                "expression adapter {category}: {}",
                error.message()
            );
            error.exit_code()
        }
    };
    std::process::exit(code);
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Workspace(std::path::PathBuf);
    impl Workspace {
        fn new() -> Self {
            static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let id = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "pkl-expression-adapter-{}-{id}",
                std::process::id()
            ));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn evaluate(&self, expression: &str) -> Observation {
            run(&[
                self.0.join("main.pkl").to_str().unwrap().into(),
                expression.into(),
            ])
            .unwrap()
        }
    }
    impl Drop for Workspace {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).unwrap();
        }
    }
    #[test]
    fn native_import_resource_and_output_operations_preserve_exact_bytes() {
        let workspace = Workspace::new();
        std::fs::write(workspace.0.join("child %.pkl"), "value = 7").unwrap();
        std::fs::write(workspace.0.join("payload.txt"), b"go\r\n").unwrap();
        std::fs::write(workspace.0.join("main.pkl"), "child = import(\"child %.pkl\")\noutput { text = child.value.toString() + read(\"payload.txt\").text }").unwrap();
        for (expression, expected) in [
            ("output.text", b"7go\r\n".as_slice()),
            ("output.text + \"tail\"", b"7go\r\ntail".as_slice()),
        ] {
            let observation = workspace.evaluate(expression);
            assert_eq!(observation.exit_code, 0, "{observation:?}");
            assert_eq!(observation.stdout, expected);
            assert!(observation.stderr.is_empty());
        }
    }
    #[test]
    fn primary_initialization_failure_precedes_malformed_query() {
        let workspace = Workspace::new();
        std::fs::write(
            workspace.0.join("main.pkl"),
            "amends \"missing-parent.pkl\"",
        )
        .unwrap();
        let ordinary = workspace.evaluate("result");
        let malformed = workspace.evaluate(")");
        assert_eq!(ordinary.exit_code, 1, "{ordinary:?}");
        assert_eq!(malformed.exit_code, ordinary.exit_code);
        assert!(ordinary.stdout.is_empty());
        assert_eq!(malformed.stdout, ordinary.stdout);
        assert!(!ordinary.stderr.is_empty());
        assert_eq!(
            malformed.stderr, ordinary.stderr,
            "complete error bytes preserve module-first ordering"
        );
    }
    #[test]
    fn java_probe_configuration_uses_noop_logger_and_empty_maps() {
        let workspace = Workspace::new();
        std::fs::write(workspace.0.join("main.pkl"), "result = trace(3) == 3 && read?(\"env:PATH\") == null && read?(\"prop:java.version\") == null").unwrap();
        let observation = workspace.evaluate("result.toString()");
        assert_eq!(observation.exit_code, 0, "{observation:?}");
        assert_eq!(observation.stdout, b"true");
        assert!(observation.stderr.is_empty());
    }
}
