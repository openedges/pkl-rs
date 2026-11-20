//! Shared `Module.output.files` evaluation (Wasm-pure; no filesystem I/O).
//!
//! Matches upstream `EvaluatorImpl.evaluateOutputFiles` and `VmUtils.readFilesProperty`:
//! `output` is validated as `ModuleOutput`, `output.files == null` produces an empty
//! ordered list, and a `Mapping<String, FileOutput>` shallow-forces its entries and
//! evaluates each `FileOutput.bytes` in insertion order.
use super::frontend::{
    DiagnosticProfile, FailureDisposition, FrontendPhase, LosslessModuleSourceInput,
    ModuleEvalOptions, ModuleFrontendResult,
};
use super::session::{SourceSession, SourceSessionOptions};
use crate::{
    heap::CollectionKind,
    host::HostPoll,
    logging::{FrameTransformer, LogDelivery, LogLevel, PlatformMetadata, TraceMode},
    source::Span,
    value::{ErrorKind, PklString, RuntimeError, Value},
    vm::{BlockMetadata, Instruction, Program},
};
use std::sync::Arc;

/// One evaluated `Module.output.files` entry, retaining all Java UTF-16 code units
/// in `path_spec` and exact raw output bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutputFileEntry {
    pub path_spec: PklString,
    pub bytes: Arc<[u8]>,
}

/// Actual `Module.output.files` operation result. Success is distinct from every
/// failure disposition.
#[derive(Clone, Debug)]
pub struct ModuleOutputFilesResult {
    pub phase: FrontendPhase,
    pub disposition: Option<FailureDisposition>,
    pub reason: Option<&'static str>,
    pub files: Vec<OutputFileEntry>,
    pub stderr: Vec<u8>,
    pub exit_code: i32,
}

impl From<ModuleFrontendResult> for ModuleOutputFilesResult {
    fn from(value: ModuleFrontendResult) -> Self {
        Self {
            phase: value.phase,
            disposition: Some(value.disposition),
            reason: value.reason,
            files: Vec::new(),
            stderr: value.stderr,
            exit_code: value.exit_code,
        }
    }
}

/// Pure `OutputUtils.checkPathSpec` / `IoUtils.isReservedFilenameChar` validation.
/// Returns the exact upstream error message text when an illegal character is found.
pub fn check_path_spec(path_spec: &PklString, windows: bool) -> Result<(), PklString> {
    for unit in path_spec.encode_utf16() {
        let reserved = if windows {
            matches!(
                unit,
                0..=31
                    | 0x3c /* '<' */
                    | 0x3e /* '>' */
                    | 0x3a /* ':' */
                    | 0x22 /* '"' */
                    | 0x5c /* '\\' */
                    | 0x2f /* '/' */
                    | 0x7c /* '|' */
                    | 0x3f /* '?' */
                    | 0x2a /* '*' */
            )
        } else {
            unit == 0 || unit == 0x2f /* '/' */
        };
        if reserved && unit != 0x2f {
            let illegal = PklString::from_utf16(vec![unit]);
            return Err(PklString::from("Path spec `")
                .concat(path_spec)
                .concat(&"` contains illegal character `".into())
                .concat(&illegal)
                .concat(&"`.".into()));
        }
    }
    Ok(())
}

/// Bytecode block evaluating `module.output.files` and every `FileOutput.bytes`
/// in insertion order, encoding `(path_spec, bytes)` losslessly as a single
/// `Value::String` so GC, Host suspension, reentry, and cancellation stay purely
/// inside the shared VM frame machinery.
pub(super) fn append_output_files_entry(program: &mut Program) -> Result<usize, RuntimeError> {
    const LOOP_START: usize = 8;
    const LOOP_END: usize = 27;
    const EMPTY: usize = 33;
    let code = vec![
        // 0..=5: validate module.output as ModuleOutput, read .files, short-circuit null
        Instruction::Input(0),
        Instruction::GetMember("output".into()),
        Instruction::CheckModuleOutput,
        Instruction::GetMember("files".into()),
        Instruction::Duplicate,
        Instruction::JumpIfNull(EMPTY),
        // 6..=7: open iterator on Mapping<String, FileOutput> and List<String> accumulator
        Instruction::OpenIterator {
            nullable: false,
            spread: false,
        },
        Instruction::BeginCollection {
            kind: CollectionKind::List,
            has_parent: false,
            specialized: false,
        },
        // 8 (LOOP_START): shallow-force all entries once, then yield (key, FileOutput) in order
        Instruction::IteratorNext { end: LOOP_END },
        // 9..=10: evaluate FileOutput.bytes and lossless ASCII base64 representation
        Instruction::GetMember("bytes".into()),
        Instruction::GetMember("base64".into()),
        // 11..=18: append path_spec code-point length, ":", path_spec
        Instruction::Swap,
        Instruction::Duplicate,
        Instruction::GetMember("length".into()),
        Instruction::ToString,
        Instruction::AppendStrictElement,
        Instruction::Constant(Value::string(":")),
        Instruction::AppendStrictElement,
        Instruction::AppendStrictElement,
        // 19..=25: append base64 length, ":", base64
        Instruction::Duplicate,
        Instruction::GetMember("length".into()),
        Instruction::ToString,
        Instruction::AppendStrictElement,
        Instruction::Constant(Value::string(":")),
        Instruction::AppendStrictElement,
        Instruction::AppendStrictElement,
        // 26: next entry
        Instruction::Jump(LOOP_START),
        // 27 (LOOP_END)..=32: join List<String> in linear time and return
        Instruction::Pop,
        Instruction::EndCollection,
        Instruction::GetMethod("join".into()),
        Instruction::Constant(Value::string("")),
        Instruction::Call {
            arity: 1,
            method: true,
        },
        Instruction::Return,
        // 33 (EMPTY)..=35: files == null -> empty encoded payload
        Instruction::Pop,
        Instruction::Constant(Value::string("")),
        Instruction::Return,
    ];
    debug_assert_eq!(code.len(), EMPTY + 3);
    program.alloc_block(
        code,
        BlockMetadata {
            module_receiver_input: Some(0),
            ..Default::default()
        },
    )
}

/// Bytecode block matching `VmUtils.readFilesProperty`: validates `module.output`,
/// shallow-forces `output.files` entries without forcing `FileOutput.bytes`, and
/// returns the ordered path-spec keys encoded losslessly in a `Value::String`.
pub(super) fn append_output_file_keys_entry(program: &mut Program) -> Result<usize, RuntimeError> {
    const LOOP_START: usize = 8;
    const LOOP_END: usize = 18;
    const EMPTY: usize = 24;
    let code = vec![
        // 0..=5
        Instruction::Input(0),
        Instruction::GetMember("output".into()),
        Instruction::CheckModuleOutput,
        Instruction::GetMember("files".into()),
        Instruction::Duplicate,
        Instruction::JumpIfNull(EMPTY),
        // 6..=7
        Instruction::OpenIterator {
            nullable: false,
            spread: false,
        },
        Instruction::BeginCollection {
            kind: CollectionKind::List,
            has_parent: false,
            specialized: false,
        },
        // 8 (LOOP_START): yields [iterator, key, FileOutput]
        Instruction::IteratorNext { end: LOOP_END },
        // 9: discard FileOutput (already shallow-forced and cached on files)
        Instruction::Pop,
        // 10..=16: append key code-point length, ":", key
        Instruction::Duplicate,
        Instruction::GetMember("length".into()),
        Instruction::ToString,
        Instruction::AppendStrictElement,
        Instruction::Constant(Value::string(":")),
        Instruction::AppendStrictElement,
        Instruction::AppendStrictElement,
        // 17: next entry
        Instruction::Jump(LOOP_START),
        // 18 (LOOP_END)..=23
        Instruction::Pop,
        Instruction::EndCollection,
        Instruction::GetMethod("join".into()),
        Instruction::Constant(Value::string("")),
        Instruction::Call {
            arity: 1,
            method: true,
        },
        Instruction::Return,
        // 24 (EMPTY)..=26
        Instruction::Pop,
        Instruction::Constant(Value::string("")),
        Instruction::Return,
    ];
    debug_assert_eq!(code.len(), EMPTY + 3);
    program.alloc_block(
        code,
        BlockMetadata {
            module_receiver_input: Some(0),
            ..Default::default()
        },
    )
}

/// Bytecode block evaluating `module.output.files[path_spec].bytes` lazily on a
/// module whose `output.files` entries were already shallow-forced and cached.
pub(super) fn append_output_file_bytes_entry(program: &mut Program) -> Result<usize, RuntimeError> {
    program.alloc_block(
        vec![
            Instruction::Input(0),
            Instruction::GetMember("output".into()),
            Instruction::CheckModuleOutput,
            Instruction::GetMember("files".into()),
            Instruction::Input(1),
            Instruction::Subscript {
                key_span: Span::default(),
            },
            Instruction::GetMember("bytes".into()),
            Instruction::Return,
        ],
        BlockMetadata {
            module_receiver_input: Some(0),
            ..Default::default()
        },
    )
}

pub(crate) fn decode_output_files(value: Value) -> Result<Vec<OutputFileEntry>, RuntimeError> {
    let Value::String(encoded) = value else {
        return Err(RuntimeError::new(
            ErrorKind::InvalidBytecode,
            "module output.files operation did not return encoded entries",
        ));
    };
    let units: Vec<u16> = encoded.encode_utf16().collect();
    let mut pos = 0;
    let mut entries = Vec::new();
    while pos < units.len() {
        let path_len = parse_length_prefix(&units, &mut pos)?;
        let path_spec = take_code_points(&units, &mut pos, path_len)?;
        let b64_len = parse_length_prefix(&units, &mut pos)?;
        let end = pos.checked_add(b64_len).ok_or_else(invalid_encoding)?;
        let b64_slice = units.get(pos..end).ok_or_else(invalid_encoding)?;
        pos = end;
        let bytes = decode_standard_base64_u16(b64_slice)?;
        entries.push(OutputFileEntry { path_spec, bytes });
    }
    Ok(entries)
}

pub(crate) fn decode_output_file_keys(value: Value) -> Result<Vec<PklString>, RuntimeError> {
    let Value::String(encoded) = value else {
        return Err(RuntimeError::new(
            ErrorKind::InvalidBytecode,
            "module output.files keys operation did not return encoded keys",
        ));
    };
    let units: Vec<u16> = encoded.encode_utf16().collect();
    let mut pos = 0;
    let mut keys = Vec::new();
    while pos < units.len() {
        let path_len = parse_length_prefix(&units, &mut pos)?;
        let path_spec = take_code_points(&units, &mut pos, path_len)?;
        keys.push(path_spec);
    }
    Ok(keys)
}

fn invalid_encoding() -> RuntimeError {
    RuntimeError::new(
        ErrorKind::InvalidBytecode,
        "malformed internal output.files encoding",
    )
}

fn parse_length_prefix(units: &[u16], pos: &mut usize) -> Result<usize, RuntimeError> {
    let start = *pos;
    let mut value: usize = 0;
    while let Some(&unit) = units.get(*pos) {
        if unit == u16::from(b':') {
            if *pos == start {
                return Err(invalid_encoding());
            }
            *pos += 1;
            return Ok(value);
        }
        if !(u16::from(b'0')..=u16::from(b'9')).contains(&unit) {
            return Err(invalid_encoding());
        }
        let digit = usize::from(unit - u16::from(b'0'));
        value = value
            .checked_mul(10)
            .and_then(|v| v.checked_add(digit))
            .ok_or_else(invalid_encoding)?;
        *pos += 1;
    }
    Err(invalid_encoding())
}

fn take_code_points(
    units: &[u16],
    pos: &mut usize,
    count: usize,
) -> Result<PklString, RuntimeError> {
    let start = *pos;
    for _ in 0..count {
        let first = *units.get(*pos).ok_or_else(invalid_encoding)?;
        *pos += 1;
        if (0xd800..=0xdbff).contains(&first)
            && units
                .get(*pos)
                .is_some_and(|next| (0xdc00..=0xdfff).contains(next))
        {
            *pos += 1;
        }
    }
    Ok(PklString::from_utf16(units[start..*pos].to_vec()))
}

fn decode_standard_base64_u16(units: &[u16]) -> Result<Arc<[u8]>, RuntimeError> {
    if units.is_empty() {
        return Ok(Arc::from([]));
    }
    let (chunks, remainder) = units.as_chunks::<4>();
    if !remainder.is_empty() {
        return Err(invalid_encoding());
    }
    let decode_sextet = |unit: u16| -> Result<u8, RuntimeError> {
        match unit {
            0x41..=0x5a => Ok((unit - 0x41) as u8),      // A-Z
            0x61..=0x7a => Ok((unit - 0x61 + 26) as u8), // a-z
            0x30..=0x39 => Ok((unit - 0x30 + 52) as u8), // 0-9
            0x2b => Ok(62),                              // +
            0x2f => Ok(63),                              // /
            _ => Err(invalid_encoding()),
        }
    };
    let mut output = Vec::with_capacity(chunks.len() * 3);
    let chunk_count = chunks.len();
    for (idx, chunk) in chunks.iter().enumerate() {
        let is_last = idx + 1 == chunk_count;
        let a = decode_sextet(chunk[0])?;
        let b = decode_sextet(chunk[1])?;
        output.push((a << 2) | (b >> 4));
        if chunk[2] == u16::from(b'=') {
            if !is_last || chunk[3] != u16::from(b'=') {
                return Err(invalid_encoding());
            }
        } else {
            let c = decode_sextet(chunk[2])?;
            output.push(((b & 0x0f) << 4) | (c >> 2));
            if chunk[3] == u16::from(b'=') {
                if !is_last {
                    return Err(invalid_encoding());
                }
            } else {
                let d = decode_sextet(chunk[3])?;
                output.push(((c & 0x03) << 6) | d);
            }
        }
    }
    Ok(Arc::from(output))
}

/// Evaluate `Module.output.files` using the default configured policy.
pub fn evaluate_output_files(
    input: impl Into<LosslessModuleSourceInput>,
    options: &ModuleEvalOptions,
) -> ModuleOutputFilesResult {
    evaluate_output_files_with_host(
        input,
        options,
        crate::modules::default_or_unsupported_completion,
    )
}

/// Evaluate `Module.output.files` with module I/O supplied by the Host.
pub fn evaluate_output_files_with_host(
    input: impl Into<LosslessModuleSourceInput>,
    options: &ModuleEvalOptions,
    module_host: impl FnMut(&crate::modules::ModuleRequest) -> crate::modules::ModuleCompletion,
) -> ModuleOutputFilesResult {
    evaluate_output_files_with_hosts(input, options, module_host, |_| None)
}

/// Evaluate `Module.output.files` with module and resource I/O supplied by the Host.
pub fn evaluate_output_files_with_hosts(
    input: impl Into<LosslessModuleSourceInput>,
    options: &ModuleEvalOptions,
    module_host: impl FnMut(&crate::modules::ModuleRequest) -> crate::modules::ModuleCompletion,
    resource_host: impl FnMut(
        &crate::vm::ReadRequest,
    )
        -> Option<Result<crate::resources::ReadResult, crate::host::ReadFailure>>,
) -> ModuleOutputFilesResult {
    evaluate_output_files_with_glob_host(input, options, module_host, resource_host, |_| None)
}

/// Evaluate `Module.output.files` with module, resource, and glob Host callbacks.
pub fn evaluate_output_files_with_glob_host(
    input: impl Into<LosslessModuleSourceInput>,
    options: &ModuleEvalOptions,
    module_host: impl FnMut(&crate::modules::ModuleRequest) -> crate::modules::ModuleCompletion,
    resource_host: impl FnMut(
        &crate::vm::ReadRequest,
    )
        -> Option<Result<crate::resources::ReadResult, crate::host::ReadFailure>>,
    glob_host: impl FnMut(&crate::glob::GlobRequest) -> Option<crate::glob::GlobCompletion>,
) -> ModuleOutputFilesResult {
    evaluate_output_files_with_platform_and_hosts(
        input,
        options,
        None,
        module_host,
        resource_host,
        glob_host,
    )
}

/// Evaluate `Module.output.files` with explicit optional Host `PlatformMetadata`.
pub fn evaluate_output_files_with_platform_and_hosts(
    input: impl Into<LosslessModuleSourceInput>,
    options: &ModuleEvalOptions,
    platform_metadata: Option<PlatformMetadata>,
    module_host: impl FnMut(&crate::modules::ModuleRequest) -> crate::modules::ModuleCompletion,
    resource_host: impl FnMut(
        &crate::vm::ReadRequest,
    )
        -> Option<Result<crate::resources::ReadResult, crate::host::ReadFailure>>,
    glob_host: impl FnMut(&crate::glob::GlobRequest) -> Option<crate::glob::GlobCompletion>,
) -> ModuleOutputFilesResult {
    evaluate_output_files_with_budget_and_hosts(
        input,
        options,
        platform_metadata,
        1_048_576,
        module_host,
        resource_host,
        glob_host,
    )
}

fn build_output_session(
    input: LosslessModuleSourceInput,
    options: &ModuleEvalOptions,
    platform_metadata: Option<PlatformMetadata>,
) -> Result<SourceSession, ModuleFrontendResult> {
    let mut session = SourceSession::compile_output(
        input,
        &[],
        options,
        SourceSessionOptions {
            evaluation: super::EvaluationOptions {
                power_assertions: true,
            },
            trace_mode: crate::packages::resolver::current_trace_mode()
                .unwrap_or(TraceMode::Compact),
            log_delivery: LogDelivery::Host,
            frame_transformer: if options.diagnostic_profile
                == DiagnosticProfile::CliBundledSettings
            {
                FrameTransformer::Host
            } else {
                FrameTransformer::Default
            },
            platform_data: options.diagnostic_profile == DiagnosticProfile::CliBundledSettings,
        },
    )?;
    if let Some(metadata) = platform_metadata {
        session.set_platform_metadata(Some(metadata));
    }
    Ok(session)
}

#[allow(clippy::too_many_arguments)]
fn poll_until_ready(
    session: &mut SourceSession,
    options: &ModuleEvalOptions,
    remaining_budget: &mut usize,
    stderr: &mut Vec<u8>,
    budget_exhausted: &mut bool,
    module_host: &mut impl FnMut(&crate::modules::ModuleRequest) -> crate::modules::ModuleCompletion,
    resource_host: &mut impl FnMut(
        &crate::vm::ReadRequest,
    ) -> Option<
        Result<crate::resources::ReadResult, crate::host::ReadFailure>,
    >,
    glob_host: &mut impl FnMut(&crate::glob::GlobRequest) -> Option<crate::glob::GlobCompletion>,
) -> Result<Value, RuntimeError> {
    while *remaining_budget > 0 {
        *remaining_budget -= 1;
        match session.poll(0, 1024) {
            HostPoll::Ready(result) => return result,
            HostPoll::Yielded => {}
            HostPoll::Module { token, request } => {
                session.complete_module(&token, module_host(&request), 0)?;
            }
            HostPoll::Glob { token, request } => {
                let completion = glob_host(&request).unwrap_or_else(|| {
                    crate::glob::configured_or_unsupported_completion(
                        &request,
                        &options.properties,
                        &options.environment,
                    )
                });
                session.complete_glob(&token, completion, 0)?;
            }
            HostPoll::Read { token, .. } => {
                let request = session.read_request(&token)?;
                if let Some(result) = resource_host(&request) {
                    session.complete_read(&token, result, 0)?;
                } else {
                    session.resume_default_read(
                        &token,
                        &options.properties,
                        &options.environment,
                        0,
                    )?;
                }
            }
            HostPoll::TransformFrame { token, frame } => {
                let mut frame = FrameTransformer::Default.transform((*frame).clone());
                if frame.module_uri.to_java_utf8().starts_with("file:") {
                    frame.module_uri = frame
                        .module_uri
                        .concat(&format!(", line {}", frame.start_line).into());
                }
                session.complete_frame_transform(&token, Ok(frame), 0)?;
            }
            HostPoll::Log { token, event } => {
                let level = match event.level {
                    LogLevel::Trace => "TRACE",
                    LogLevel::Warn => "WARN",
                };
                let uri = event.frame.module_uri.to_java_utf8();
                let message = event.message.to_java_utf8();
                let space = if message.ends_with('\n') { "" } else { " " };
                stderr.extend_from_slice(
                    format!("pkl: {level}: {message}{space}({uri})\n").as_bytes(),
                );
                session.acknowledge_log(&token, Ok(()), 0)?;
            }
        }
    }
    *budget_exhausted = true;
    session.cancel();
    Err(RuntimeError::new(
        ErrorKind::Unsupported,
        "Module output instruction budget exceeded.",
    ))
}

fn evaluate_output_files_with_budget_and_hosts(
    input: impl Into<LosslessModuleSourceInput>,
    options: &ModuleEvalOptions,
    platform_metadata: Option<PlatformMetadata>,
    poll_budget: usize,
    mut module_host: impl FnMut(&crate::modules::ModuleRequest) -> crate::modules::ModuleCompletion,
    mut resource_host: impl FnMut(
        &crate::vm::ReadRequest,
    ) -> Option<
        Result<crate::resources::ReadResult, crate::host::ReadFailure>,
    >,
    mut glob_host: impl FnMut(&crate::glob::GlobRequest) -> Option<crate::glob::GlobCompletion>,
) -> ModuleOutputFilesResult {
    let mut session = match build_output_session(input.into(), options, platform_metadata) {
        Ok(session) => session,
        Err(error) => return error.into(),
    };
    let mut stderr = Vec::new();
    let mut budget_exhausted = false;
    let mut remaining = poll_budget;
    let run = (|| {
        session.start_output_files(0, None)?;
        let value = poll_until_ready(
            &mut session,
            options,
            &mut remaining,
            &mut stderr,
            &mut budget_exhausted,
            &mut module_host,
            &mut resource_host,
            &mut glob_host,
        )?;
        SourceSession::result_output_files(value)
    })();
    let mut result = match run {
        Ok(files) => ModuleOutputFilesResult {
            phase: FrontendPhase::ModuleOutput,
            disposition: None,
            reason: None,
            files,
            stderr: Vec::new(),
            exit_code: 0,
        },
        Err(error) => session.output_failure(error).into(),
    };
    if budget_exhausted {
        result.disposition = Some(FailureDisposition::LimitExceeded);
        result.reason = Some("instruction-budget");
        result.exit_code = 3;
    }
    stderr.extend_from_slice(&result.stderr);
    result.stderr = stderr;
    session.close();
    result
}

/// Outcome of streaming lazy `output.files` evaluation (`CliEvaluator.writeMultipleFileOutput`).
#[derive(Debug)]
pub enum StreamingOutputFilesOutcome<E> {
    Ok { stderr: Vec<u8> },
    EvalFailure(ModuleOutputFilesResult),
    ConsumerError { error: E, stderr: Vec<u8> },
}

/// Evaluate `output.files` matching `CliEvaluator.writeMultipleFileOutput`:
/// shallow-forces `output.files` entries first, then invokes `on_entry` for each
/// ordered `path_spec` with a lazy closure that evaluates `FileOutput.bytes` only
/// if the caller accepts the path.
#[allow(clippy::too_many_arguments)]
pub fn evaluate_output_files_streaming_with_hosts<E>(
    input: impl Into<LosslessModuleSourceInput>,
    options: &ModuleEvalOptions,
    platform_metadata: Option<PlatformMetadata>,
    mut module_host: impl FnMut(&crate::modules::ModuleRequest) -> crate::modules::ModuleCompletion,
    mut resource_host: impl FnMut(
        &crate::vm::ReadRequest,
    ) -> Option<
        Result<crate::resources::ReadResult, crate::host::ReadFailure>,
    >,
    mut glob_host: impl FnMut(&crate::glob::GlobRequest) -> Option<crate::glob::GlobCompletion>,
    mut on_entry: impl FnMut(
        &PklString,
        &mut dyn FnMut() -> Result<Arc<[u8]>, ModuleOutputFilesResult>,
    ) -> Result<(), E>,
) -> StreamingOutputFilesOutcome<E> {
    let mut session = match build_output_session(input.into(), options, platform_metadata) {
        Ok(session) => session,
        Err(error) => return StreamingOutputFilesOutcome::EvalFailure(error.into()),
    };
    let mut stderr = Vec::new();
    let mut budget_exhausted = false;
    let mut remaining: usize = 1_048_576;

    let keys_run = (|| {
        session.start_output_file_keys(0, None)?;
        let value = poll_until_ready(
            &mut session,
            options,
            &mut remaining,
            &mut stderr,
            &mut budget_exhausted,
            &mut module_host,
            &mut resource_host,
            &mut glob_host,
        )?;
        SourceSession::result_output_file_keys(value)
    })();

    let keys = match keys_run {
        Ok(keys) => keys,
        Err(error) => {
            let mut result: ModuleOutputFilesResult = session.output_failure(error).into();
            if budget_exhausted {
                result.disposition = Some(FailureDisposition::LimitExceeded);
                result.reason = Some("instruction-budget");
                result.exit_code = 3;
            }
            stderr.extend_from_slice(&result.stderr);
            result.stderr = stderr;
            session.close();
            return StreamingOutputFilesOutcome::EvalFailure(result);
        }
    };

    for key in &keys {
        let mut eval_failure: Option<ModuleOutputFilesResult> = None;
        let key_clone = key.clone();
        let mut eval_bytes = || -> Result<Arc<[u8]>, ModuleOutputFilesResult> {
            let run = (|| {
                session.start_output_file_bytes(key_clone.clone(), 0, None)?;
                let value = poll_until_ready(
                    &mut session,
                    options,
                    &mut remaining,
                    &mut stderr,
                    &mut budget_exhausted,
                    &mut module_host,
                    &mut resource_host,
                    &mut glob_host,
                )?;
                SourceSession::result_bytes(value)
            })();
            match run {
                Ok(bytes) => Ok(bytes),
                Err(error) => {
                    let mut result: ModuleOutputFilesResult = session.output_failure(error).into();
                    if budget_exhausted {
                        result.disposition = Some(FailureDisposition::LimitExceeded);
                        result.reason = Some("instruction-budget");
                        result.exit_code = 3;
                    }
                    let mut full_stderr = std::mem::take(&mut stderr);
                    full_stderr.extend_from_slice(&result.stderr);
                    result.stderr = full_stderr;
                    eval_failure = Some(result.clone());
                    Err(result)
                }
            }
        };
        if let Err(consumer_error) = on_entry(key, &mut eval_bytes) {
            session.close();
            if let Some(failure) = eval_failure {
                return StreamingOutputFilesOutcome::EvalFailure(failure);
            }
            return StreamingOutputFilesOutcome::ConsumerError {
                error: consumer_error,
                stderr,
            };
        }
        if let Some(failure) = eval_failure {
            session.close();
            return StreamingOutputFilesOutcome::EvalFailure(failure);
        }
    }

    session.close();
    StreamingOutputFilesOutcome::Ok { stderr }
}
