//! Experimental scalar language boundary shared by development adapters.
//! This does not implement the full evaluator API, general compound stringification, or Host I/O.
use crate::diagnostic::{RenderFrame, render_error};
use crate::source::{Source, SourceMap, Span};
use crate::value::{ErrorKind, RuntimeError, Value};
use crate::vm::{BlockMetadata, Instruction, Vm, VmPoll};

/// Options shared by all experimental language adapters.
#[derive(Clone, Copy, Debug, Default)]
pub struct EvaluationOptions {
    /// Match the Java API default (disabled) unless explicitly requested.
    pub power_assertions: bool,
}

/// Evaluate a property path to a scalar or domain-rendered Reference string.
/// The supplied URI and inferred module name are transport context, never rewritten
/// after evaluation. Explicit module declarations override the inferred name.
pub fn scalar_expression(
    source_text: &str,
    uri: &str,
    name: &str,
    expression: &str,
) -> Result<String, String> {
    scalar_expression_with_options(
        source_text,
        uri,
        name,
        expression,
        EvaluationOptions::default(),
    )
}

pub fn scalar_expression_with_options(
    source_text: &str,
    uri: &str,
    name: &str,
    expression: &str,
    options: EvaluationOptions,
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
            error.hint(),
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
            error.hint(),
        )
    })?;
    let mut program = if options.power_assertions {
        crate::vm::compile_module_with_power_assertions(&resolved)
    } else {
        crate::vm::compile_module(&resolved)
    }
    .map_err(|error| runtime_error(source, uri, name, expression, error, false))?;
    let program_mut = std::sync::Arc::make_mut(&mut program);
    program_mut.module_name = name.into();
    // The Java expression-string boundary converts a returned Reference through
    // its domain renderer after evaluating the requested expression.
    let stringify = program_mut.blocks.len();
    program_mut.blocks.push(vec![
        Instruction::Input(0),
        Instruction::ToString,
        Instruction::Return,
    ]);
    program_mut.metadata.push(BlockMetadata::default());
    let entry = program.entry;
    let mut vm = Vm::new(program);
    vm.set_power_assertions(options.power_assertions);
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
    let mut value =
        evaluate().map_err(|error| runtime_error(source, uri, name, expression, error, true))?;
    if vm.is_reference(&value) {
        value = vm
            .start(stringify, vec![value])
            .and_then(|()| poll(&mut vm))
            .map_err(|error| runtime_error(source, uri, name, expression, error, false))?;
    }
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
    let mut sources = SourceMap::default();
    let repl_id = sources.add("repl:text", expression);
    let mut foreign_sources = Vec::new();
    let source_ids: Vec<_> = error
        .frames
        .iter()
        .map(|frame| {
            frame.source.as_ref().map(|foreign| {
                if let Some((_, id)) = foreign_sources
                    .iter()
                    .find(|(seen, _)| std::sync::Arc::ptr_eq(seen, foreign))
                {
                    return *id;
                }
                let id = sources.add(foreign.uri.to_string(), foreign.text.to_string());
                foreign_sources.push((foreign.clone(), id));
                id
            })
        })
        .collect();
    let diagnostic_source = source_ids
        .first()
        .copied()
        .flatten()
        .and_then(|id| sources.get(id))
        .unwrap_or(source);
    if let Some(span) = error.spread_hint.as_deref()
        && let Some(operand) = diagnostic_source.slice(*span)
    {
        error.hint = Some(format!(
            "To guard against a nullable value, use `...?` instead of `...`.\nTry: `...?{operand}`"
        ));
    }
    if let Some(diagnostic) = &error.type_diagnostic {
        let (message, hint) = diagnostic.render(diagnostic_source);
        error.message = message;
        error.hint = hint;
    }
    if error.kind == ErrorKind::MissingType {
        let module = error
            .frames
            .first()
            .and_then(|frame| frame.source.as_ref())
            .map_or(module, |source| source.module_name.as_ref());
        error.hint = Some(format!(
            "Searched in the following places:\n* Module `{module}` and any supermodules\n* Module `pkl#base`"
        ));
    }
    let names: Vec<_> = error
        .frames
        .iter()
        .map(|frame| {
            let module = frame
                .source
                .as_ref()
                .map_or(module, |source| source.module_name.as_ref());
            format!("{module}{}", frame.member)
        })
        .collect();
    let uris: Vec<_> = error
        .frames
        .iter()
        .zip(&source_ids)
        .map(|(frame, id)| {
            if let Some(foreign) = &frame.source {
                if foreign.line_anchors
                    && let Some(position) = id
                        .and_then(|id| sources.get(id))
                        .and_then(|source| source.position(frame.span.start))
                {
                    return format!("{}#L{}", foreign.uri, position.line);
                }
                foreign.uri.to_string()
            } else {
                uri.to_owned()
            }
        })
        .collect();
    let mut frames: Vec<_> = error
        .frames
        .iter()
        .zip(&names)
        .zip(uris.iter().zip(&source_ids))
        .map(|((frame, name), (uri, id))| RenderFrame {
            source: id.and_then(|id| sources.get(id)).unwrap_or(source),
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
