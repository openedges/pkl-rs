//! Experimental expression-string boundary shared by development adapters.
//! This does not implement the full evaluator API or general REPL/module loading.
pub mod frontend;
pub mod session;
use crate::diagnostic::{RenderFrame, render_error_text};
use crate::source::{Source, SourceMap, Span};
use crate::value::{ErrorKind, PklString, RuntimeError, Value};
use crate::vm::{BlockMetadata, Instruction, Vm, VmPoll};

/// Options shared by all experimental language adapters.
#[derive(Clone, Copy, Debug, Default)]
pub struct EvaluationOptions {
    /// Match the Java API default (disabled) unless explicitly requested.
    pub power_assertions: bool,
}

/// Evaluate a property path to a scalar, strict Map/Set, or domain-rendered Reference string.
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
    scalar_expression_text_with_options(source_text, uri, name, expression, options)
        .map(|text| text.to_java_utf8())
        .map_err(|text| text.to_java_utf8())
}

/// Evaluate the same bounded property path while retaining value and error units.
pub fn scalar_expression_text_with_options(
    source_text: &str,
    uri: &str,
    name: &str,
    expression: &str,
    options: EvaluationOptions,
) -> Result<PklString, PklString> {
    scalar_expressions_text_with_options(source_text, uri, name, &[expression.to_owned()], options)?
        .pop()
        .expect("one expression supplied")
}

/// Bounded development sequence boundary. Setup errors abort the sequence; call
/// errors leave the same VM and module available to subsequent property paths.
/// The module is retained across a GC after every call, and the VM is closed at
/// the end. This is not the complete public evaluator or module-cache API.
pub fn scalar_expressions_with_options(
    source_text: &str,
    uri: &str,
    name: &str,
    expressions: &[String],
    options: EvaluationOptions,
) -> Result<Vec<Result<String, String>>, String> {
    scalar_expressions_text_with_options(source_text, uri, name, expressions, options)
        .map(|results| {
            results
                .into_iter()
                .map(|result| {
                    result
                        .map(|text| text.to_java_utf8())
                        .map_err(|text| text.to_java_utf8())
                })
                .collect()
        })
        .map_err(|text| text.to_java_utf8())
}

/// Canonical sequence text; encoding is chosen by the consuming adapter.
/// Evaluation, GC and property-path restrictions match the legacy byte wrapper.
pub fn scalar_expressions_text_with_options(
    source_text: &str,
    uri: &str,
    name: &str,
    expressions: &[String],
    options: EvaluationOptions,
) -> Result<Vec<Result<PklString, PklString>>, PklString> {
    let expression = expressions.first().map_or("", String::as_str);
    let mut sources = SourceMap::default();
    let source_id = sources.add(uri, source_text);
    let source = sources.get(source_id).expect("source inserted");
    let (mut program, name) = compile_program_text(source, uri, name, expression, options)?;
    let name = name.as_str();
    let program_mut = std::sync::Arc::make_mut(&mut program);
    // The Java expression-string boundary converts a returned Reference through
    // its domain renderer and a strict Map/Set through its non-forcing value renderer.
    let stringify = program_mut
        .alloc_block(
            vec![
                Instruction::Input(0),
                Instruction::ToString,
                Instruction::Return,
            ],
            BlockMetadata::default(),
        )
        .map_err(|error| error.message)?;
    let output_text = expressions
        .iter()
        .any(|expression| expression == "output.text")
        .then(|| session::append_output_entry(program_mut, "text"))
        .transpose()
        .map_err(|error| error.message)?;
    let entry = program.entry;
    std::sync::Arc::make_mut(&mut program)
        .register_primary_source(
            std::sync::Arc::new(crate::value::RuntimeSource {
                text: source_text.into(),
                uri: uri.into(),
                module_name: name.into(),
                line_anchors: false,
            }),
            crate::modules::ModuleIdentity {
                original_uri: uri.into(),
                resolved_uri: uri.into(),
            },
        )
        .map_err(|error| runtime_error_text(source, uri, name, expression, error, None))?;
    let mut vm = Vm::with_context(
        program,
        crate::logging::EvaluationContext {
            primary_source: Some(std::sync::Arc::new(crate::value::RuntimeSource {
                text: source_text.into(),
                uri: uri.into(),
                module_name: name.into(),
                line_anchors: false,
            })),
            ..Default::default()
        },
    );
    vm.set_power_assertions(options.power_assertions);
    vm.start(entry, vec![])
        .map_err(|error| runtime_error_text(source, uri, name, expression, error, None))?;
    let module_value = poll(&mut vm)
        .map_err(|error| runtime_error_text(source, uri, name, expression, error, None))?;
    let mut results = Vec::with_capacity(expressions.len());
    for expression in expressions {
        if expression == "output.text" {
            // Same real output operation as the session API, without ToString
            // or a synthetic REPL frame (EvaluatorImpl.evaluateExpressionString).
            let result = vm
                .start(
                    output_text.expect("output entry"),
                    vec![module_value.clone()],
                )
                .and_then(|()| poll(&mut vm))
                .and_then(session::SourceSession::result_string)
                .map_err(|error| runtime_error_text(source, uri, name, "", error, None));
            results.push(result);
            vm.collect(std::slice::from_ref(&module_value));
            continue;
        }
        let mut expression_end = 0;
        let mut evaluate = || -> Result<Value, RuntimeError> {
            let mut value = module_value.clone();
            // The bounded adapter accepts property paths, not arbitrary REPL expressions.
            for member in expression.split('.') {
                if expression_end > 0 {
                    expression_end += 1; // The property separator is part of the prefix.
                }
                expression_end += member.len();
                if member.is_empty() || !member.chars().all(|ch| ch.is_alphanumeric() || ch == '_')
                {
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
        let result = evaluate()
            .map_err(|error| {
                runtime_error_text(source, uri, name, expression, error, Some(expression_end))
            })
            .and_then(|mut value| {
                if vm.is_reference(&value)
                    || vm.is_map(&value)
                    || vm.is_set(&value)
                    || vm.is_list(&value)
                    || matches!(value, Value::TypeAlias(_))
                {
                    value = vm
                        .start(stringify, vec![value])
                        .and_then(|()| poll(&mut vm))
                        .map_err(|error| {
                            runtime_error_text(source, uri, name, expression, error, None)
                        })?;
                }
                match value {
                    Value::Null | Value::NullWithDefault(_) => Ok("null".into()),
                    Value::Bool(value) => Ok(value.to_string().into()),
                    Value::Int(value) => Ok(value.to_string().into()),
                    Value::Float(value) => Ok(crate::numeric::double_to_string(*value).into()),
                    Value::String(value) => Ok(value),
                    _ => Err(render_error_text(
                        &"the M2 probe does not yet stringify compound values".into(),
                        &[],
                        None,
                    )),
                }
            });
        results.push(result);
        vm.collect(std::slice::from_ref(&module_value));
    }
    vm.close();
    Ok(results)
}

/// Preserve diagnostic code units until the caller selects a transport encoding.
fn compile_program_text(
    source: &Source,
    uri: &str,
    name: &str,
    expression: &str,
    options: EvaluationOptions,
) -> Result<(std::sync::Arc<crate::vm::Program>, String), PklString> {
    let module = crate::parser::parse_module_for_evaluation(source.text()).map_err(|error| {
        let name = error.module_name.as_deref().unwrap_or(name);
        let member = error.member.as_ref().map_or_else(
            || name.into(),
            |suffix| PklString::from(format!("{name}#")).concat(suffix),
        );
        render_error_text(
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
    let resolved = crate::resolver::resolve_module_for_evaluation(&module);
    let mut program = if options.power_assertions {
        crate::vm::compile_module_with_power_assertions(&resolved)
    } else {
        crate::vm::compile_module(&resolved)
    }
    .map_err(|error| runtime_error_text(source, uri, name, expression, error, None))?;
    std::sync::Arc::make_mut(&mut program).module_name = name.into();
    Ok((program, name.to_owned()))
}

fn poll(vm: &mut Vm) -> Result<Value, RuntimeError> {
    for _ in 0..1024 {
        match vm.poll(1024) {
            VmPoll::Ready(result) => return result,
            VmPoll::Pending(request) => {
                let result = crate::host::resources::default_result(&request);
                vm.resume(request.id, result)?;
            }
            VmPoll::Log(_) => {
                vm.cancel();
                return Err(RuntimeError::new(
                    ErrorKind::Unsupported,
                    "the M2 probe does not supply a Host",
                ));
            }
            VmPoll::Module(request) => vm.complete_module(
                request.id,
                crate::modules::default_or_unsupported_completion(&request),
            )?,
            VmPoll::Yielded => {}
        }
    }
    vm.cancel();
    Err(RuntimeError::new(
        ErrorKind::Unsupported,
        "M2 probe instruction budget exceeded",
    ))
}

/// Shared raw diagnostic formatting for source evaluation and development probes.
/// This helper is not a complete product evaluation API.
pub fn runtime_error(
    source: &Source,
    uri: &str,
    module: &str,
    expression: &str,
    error: RuntimeError,
    expression_end: Option<usize>,
) -> String {
    runtime_error_text(source, uri, module, expression, error, expression_end).to_java_utf8()
}

/// Render a source diagnostic without losing unmatched Java UTF-16 code units.
pub fn runtime_error_text(
    source: &Source,
    uri: &str,
    module: &str,
    expression: &str,
    error: RuntimeError,
    expression_end: Option<usize>,
) -> PklString {
    runtime_error_text_with_profile(
        source,
        uri,
        module,
        expression,
        error,
        expression_end,
        frontend::DiagnosticProfile::ApiIdentity,
    )
}

pub(crate) fn runtime_error_text_with_profile(
    source: &Source,
    uri: &str,
    module: &str,
    expression: &str,
    mut error: RuntimeError,
    expression_end: Option<usize>,
    profile: frontend::DiagnosticProfile,
) -> PklString {
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
        ).into());
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
        ).into());
    }
    let names: Vec<_> = error
        .frames
        .iter()
        .enumerate()
        .map(|(index, frame)| {
            if index == 0
                && let Some(frontend) = &error.frontend
            {
                return frontend.member.clone();
            }
            let module = frame
                .source
                .as_ref()
                .map_or(module, |source| source.module_name.as_ref());
            PklString::from(module).concat(&frame.member)
        })
        .collect();
    let uris: Vec<_> = error
        .frames
        .iter()
        .zip(&source_ids)
        .map(|(frame, id)| {
            let location = if let Some(foreign) = &frame.source {
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
            };
            if profile == frontend::DiagnosticProfile::CliBundledSettings
                && location.starts_with("file:")
            {
                let line = id
                    .and_then(|id| sources.get(id))
                    .unwrap_or(source)
                    .position(frame.span.start)
                    .map_or(1, |position| position.line);
                format!("{location}, line {line}")
            } else {
                location
            }
        })
        .collect();
    let module_member = PklString::from(module);
    let expression_member = PklString::default();
    let mut frames: Vec<_> = error
        .frames
        .iter()
        .zip(&names)
        .zip(uris.iter().zip(&source_ids))
        .enumerate()
        .map(|(index, ((frame, name), (uri, id)))| RenderFrame {
            source: id.and_then(|id| sources.get(id)).unwrap_or(source),
            uri,
            member: name,
            span: frame.span,
            highlight_length_utf16: if index == 0 {
                error
                    .frontend
                    .as_ref()
                    .and_then(|frontend| frontend.diagnostic.highlight_length_utf16)
            } else {
                None
            },
        })
        .collect();
    let fallback_uri =
        if profile == frontend::DiagnosticProfile::CliBundledSettings && uri.starts_with("file:") {
            format!(
                "{uri}, line {}",
                error
                    .span
                    .and_then(|span| source.position(span.start))
                    .map_or(1, |position| position.line)
            )
        } else {
            uri.to_owned()
        };
    if frames.is_empty()
        && let Some(span) = error.span
    {
        frames.push(RenderFrame {
            source,
            uri: &fallback_uri,
            member: &module_member,
            span,
            highlight_length_utf16: None,
        });
    }
    if let Some(end) = expression_end {
        frames.push(RenderFrame {
            source: sources.get(repl_id).expect("expression source inserted"),
            uri: "repl:text",
            member: &expression_member,
            span: Span::new(0, end),
            highlight_length_utf16: None,
        });
    }
    render_error_text(&error.message, &frames, error.hint.as_ref())
}
