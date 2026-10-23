//! Development-only original module-output operation, not a complete product CLI.
use pkl_runtime::evaluation::frontend::{
    ModuleSourceInput, arguments::parse_module_eval_args, evaluate_module_output_with_host,
};
use std::{io::Write, path::Path};

#[path = "module_eval_probe/environment.rs"]
mod environment;
#[path = "support/module_host.rs"]
mod module_host;

#[derive(Debug)]
enum ProbeError {
    Unsupported(String),
    Internal(String),
}
impl ProbeError {
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

fn file_uri(path: &Path) -> Result<String, ProbeError> {
    let path = path.to_str().ok_or_else(|| {
        ProbeError::Unsupported("module-eval adapter requires a UTF-8 source path".into())
    })?;
    let mut uri = String::from("file://");
    for byte in path.bytes() {
        if byte.is_ascii_alphanumeric() || b"/-._~:".contains(&byte) {
            uri.push(char::from(byte));
        } else {
            use std::fmt::Write;
            write!(uri, "%{byte:02X}").expect("writing to String cannot fail");
        }
    }
    Ok(uri)
}

fn run(arguments: &[String]) -> Result<i32, ProbeError> {
    let mut invocation = parse_module_eval_args(arguments).map_err(ProbeError::Unsupported)?;
    // Resolve against the real process cwd without canonicalizing symlinks or
    // substituting an oracle display URI.
    let path = std::path::absolute(&invocation.source_path)
        .map_err(|error| ProbeError::Internal(error.to_string()))?;
    let text =
        std::fs::read_to_string(&path).map_err(|error| ProbeError::Internal(error.to_string()))?;
    let uri = file_uri(&path)?;
    let inferred_name = path
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or_else(|| {
            ProbeError::Unsupported("module-eval adapter requires a UTF-8 module name".into())
        })?;
    // The preconfigured Java CLI inherits the process environment before
    // applying explicit options. Keep process access at the native boundary.
    let mut inherited = environment::inherited().map_err(ProbeError::Unsupported)?;
    inherited.append(&mut invocation.options.environment);
    invocation.options.environment = inherited;
    let result = evaluate_module_output_with_host(
        ModuleSourceInput {
            text: &text,
            uri: &uri,
            inferred_name,
        },
        &invocation.options,
        module_host::complete,
    );
    std::io::stdout()
        .write_all(&result.stdout)
        .map_err(|error| ProbeError::Internal(error.to_string()))?;
    std::io::stderr()
        .write_all(&result.stderr)
        .map_err(|error| ProbeError::Internal(error.to_string()))?;
    Ok(result.exit_code)
}

fn main() {
    let arguments = std::env::args_os()
        .skip(1)
        .map(|argument| {
            argument.into_string().map_err(|_| {
                ProbeError::Unsupported("module-eval adapter requires UTF-8 arguments".into())
            })
        })
        .collect::<Result<Vec<_>, _>>();
    let result = arguments.and_then(|arguments| run(&arguments));
    match result {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            let category = match error {
                ProbeError::Unsupported(_) => "unsupported",
                ProbeError::Internal(_) => "internal failure",
            };
            let _ = writeln!(
                std::io::stderr(),
                "module-eval adapter {category}: {}",
                error.message()
            );
            std::process::exit(error.exit_code());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nonexistent_input_is_an_internal_acquisition_failure_not_unsupported() {
        let path = std::env::temp_dir()
            .join(format!("pkl-stage39-absent-{}", std::process::id()))
            .join("missing.pkl");
        let args = vec![
            "eval".into(),
            "--no-cache".into(),
            "--no-project".into(),
            path.to_string_lossy().into_owned(),
        ];
        assert!(matches!(run(&args), Err(ProbeError::Internal(_))));
        let error = run(&["not-eval".into()]).unwrap_err();
        assert!(matches!(error, ProbeError::Unsupported(_)));
        assert_eq!(error.exit_code(), 2);
        assert_eq!(ProbeError::Internal("I/O".into()).exit_code(), 4);
    }
}
