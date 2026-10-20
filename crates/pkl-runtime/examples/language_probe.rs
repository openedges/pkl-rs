//! Development-only M2 scalar expression probe, not a product CLI or API.
use std::path::Path;

use pkl_runtime::diagnostic::{RenderFrame, render_error};
use pkl_runtime::source::{Source, SourceMap, Span};
use pkl_runtime::value::{ErrorKind, RuntimeError, Value};
use pkl_runtime::vm::{Vm, VmPoll};

fn main() {
    match run() {
        Ok(value) => print!("{value}"),
        Err(error) => {
            eprint!("{error}");
            std::process::exit(1);
        }
    }
}

fn run() -> Result<String, String> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 3 {
        return Err("expected source path and expression".into());
    }
    let path = std::fs::canonicalize(&args[1]).map_err(|error| error.to_string())?;
    let text = std::fs::read_to_string(&path).map_err(|error| error.to_string())?;
    let uri = file_uri(&path)?;
    let name = path
        .file_stem()
        .and_then(|name| name.to_str())
        .ok_or("source path must be UTF-8")?;
    let mut sources = SourceMap::default();
    let source_id = sources.add(&uri, text);
    let source = sources.get(source_id).expect("source inserted");
    let module = pkl_runtime::parser::parse_module(source.text()).map_err(|error| {
        render_error(
            &error.message,
            &[RenderFrame {
                source,
                uri: &uri,
                member: name,
                span: error.span,
                highlight_length_utf16: error.highlight_length_utf16,
            }],
            None,
        )
    })?;
    let resolved = pkl_runtime::resolver::resolve_module(&module).map_err(|error| {
        render_error(
            &error.message,
            &[RenderFrame {
                source,
                uri: &uri,
                member: name,
                span: error.span,
                highlight_length_utf16: error.highlight_length_utf16,
            }],
            None,
        )
    })?;
    let program = pkl_runtime::vm::compile_module(&resolved)
        .map_err(|error| runtime_error(source, &uri, name, &args[2], error))?;
    let entry = program.entry;
    let mut vm = Vm::new(program);
    let mut evaluate = || -> Result<Value, RuntimeError> {
        vm.start(entry, vec![])?;
        let mut value = poll(&mut vm)?;
        // This initial boundary accepts property paths; the full expression API
        // remains an explicit adapter requirement in the M2 inventory.
        for member in args[2].split('.') {
            if member.is_empty() || !member.chars().all(|ch| ch.is_alphanumeric() || ch == '_') {
                return Err(RuntimeError::new(
                    ErrorKind::Unsupported,
                    "the M2 probe currently accepts property paths only",
                ));
            }
            vm.start_member(value.as_object()?, member)?;
            value = poll(&mut vm)?;
        }
        Ok(value)
    };
    let value = evaluate().map_err(|error| runtime_error(source, &uri, name, &args[2], error))?;
    match value {
        Value::Null => Ok("null".into()),
        Value::Bool(value) => Ok(value.to_string()),
        Value::Int(value) => Ok(value.to_string()),
        Value::Float(value) => Ok(pkl_runtime::numeric::double_to_string(value)),
        Value::String(value) => Ok(value.to_string()),
        _ => Err(render_error(
            "the M2 probe does not yet stringify compound values",
            &[],
            None,
        )),
    }
}

fn poll(vm: &mut Vm) -> Result<Value, RuntimeError> {
    for _ in 0..1024 {
        match vm.poll(1024) {
            VmPoll::Ready(result) => return result,
            VmPoll::Pending(_) => {
                return Err(RuntimeError::new(
                    ErrorKind::Unsupported,
                    "the M2 probe does not supply a Host",
                ));
            }
            VmPoll::Yielded => {}
        }
    }
    Err(RuntimeError::new(
        ErrorKind::Unsupported,
        "M2 probe instruction budget exceeded",
    ))
}

fn runtime_error(
    source: &Source,
    uri: &str,
    module: &str,
    expression: &str,
    error: RuntimeError,
) -> String {
    let mut sources = SourceMap::default();
    let repl_id = sources.add("repl:text", expression);
    let names: Vec<_> = error
        .frames
        .iter()
        .map(|frame| format!("{module}{}", frame.member))
        .collect();
    let mut frames: Vec<_> = error
        .frames
        .iter()
        .zip(&names)
        .map(|(frame, name)| RenderFrame {
            source,
            uri,
            member: name,
            span: frame.span,
            highlight_length_utf16: None,
        })
        .collect();
    if frames.is_empty()
        && let Some(span) = error.span
    {
        frames.push(RenderFrame {
            source,
            uri,
            member: module,
            span,
            highlight_length_utf16: None,
        });
    }
    frames.push(RenderFrame {
        source: sources.get(repl_id).expect("expression source inserted"),
        uri: "repl:text",
        member: "",
        span: Span::new(0, expression.len()),
        highlight_length_utf16: None,
    });
    render_error(&error.message, &frames, error.hint.as_deref())
}

fn file_uri(path: &Path) -> Result<String, String> {
    use std::fmt::Write;
    let mut uri = String::from("file://");
    for byte in path.to_str().ok_or("source path must be UTF-8")?.bytes() {
        if byte.is_ascii_alphanumeric() || b"/-._~".contains(&byte) {
            uri.push(char::from(byte));
        } else {
            write!(uri, "%{byte:02X}").unwrap();
        }
    }
    Ok(uri)
}
