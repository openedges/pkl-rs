//! Experimental scalar language boundary shared by development adapters.
//! This does not implement the full evaluator API, compound stringification, or Host I/O.
use crate::diagnostic::{RenderFrame, render_error};
use crate::source::{Source, SourceMap, Span};
use crate::value::{ErrorKind, RuntimeError, Value};
use crate::vm::{Vm, VmPoll};

/// Evaluate a scalar property path and retain complete source diagnostic text.
/// The supplied URI and inferred module name are transport context, never rewritten
/// after evaluation. Explicit module declarations override the inferred name.
pub fn scalar_expression(
    source_text: &str,
    uri: &str,
    name: &str,
    expression: &str,
) -> Result<String, String> {
    let mut sources = SourceMap::default();
    let source_id = sources.add(uri, source_text);
    let source = sources.get(source_id).expect("source inserted");
    let module = crate::parser::parse_module(source.text()).map_err(|error| {
        let name = error.module_name.as_deref().unwrap_or(name);
        let member = error
            .member
            .as_ref()
            .map_or_else(|| name.to_owned(), |suffix| format!("{name}#{suffix}"));
        render_error(
            &error.message,
            &[RenderFrame {
                source,
                uri,
                member: &member,
                span: error.span,
                highlight_length_utf16: error.highlight_length_utf16,
            }],
            None,
        )
    })?;
    let declared_name = module
        .declaration
        .as_ref()
        .and_then(|header| header.name.as_ref())
        .map(|parts| parts.join("."));
    let name = declared_name.as_deref().unwrap_or(name);
    let resolved = crate::resolver::resolve_module(&module).map_err(|error| {
        let member = error
            .member
            .as_ref()
            .map_or_else(|| name.to_owned(), |suffix| format!("{name}#{suffix}"));
        render_error(
            &error.message,
            &[RenderFrame {
                source,
                uri,
                member: &member,
                span: error.span,
                highlight_length_utf16: error.highlight_length_utf16,
            }],
            None,
        )
    })?;
    let mut program = crate::vm::compile_module(&resolved)
        .map_err(|error| runtime_error(source, uri, name, expression, error, false))?;
    std::sync::Arc::make_mut(&mut program).module_name = name.into();
    let entry = program.entry;
    let mut vm = Vm::new(program);
    let mut evaluate = || -> Result<Value, RuntimeError> {
        vm.start(entry, vec![])?;
        let mut value = poll(&mut vm)?;
        // This initial boundary accepts property paths; the full expression API
        // remains an explicit adapter requirement in the M2 inventory.
        for member in expression.split('.') {
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
    let value =
        evaluate().map_err(|error| runtime_error(source, uri, name, expression, error, true))?;
    match value {
        Value::Null => Ok("null".into()),
        Value::Bool(value) => Ok(value.to_string()),
        Value::Int(value) => Ok(value.to_string()),
        Value::Float(value) => Ok(crate::numeric::double_to_string(value)),
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
    mut error: RuntimeError,
    include_expression: bool,
) -> String {
    if error.kind == ErrorKind::MissingType {
        error.hint = Some(format!(
            "Searched in the following places:\n* Module `{module}` and any supermodules\n* Module `pkl#base`"
        ));
    }
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
    if include_expression {
        frames.push(RenderFrame {
            source: sources.get(repl_id).expect("expression source inserted"),
            uri: "repl:text",
            member: "",
            span: Span::new(0, expression.len()),
            highlight_length_utf16: None,
        });
    }
    render_error(&error.message, &frames, error.hint.as_deref())
}
