//! Explicitly experimental M1 adapter entry points, not the final public Pkl API.
use crate::{
    host::{HostPoll, HostSession},
    value::{ErrorKind, PklString, RuntimeError, Value},
    vm::{Instruction, Program, Vm, VmPoll},
};
use std::sync::Arc;

/// Reject JavaScript numeric coercion before narrowing experimental clock/budget arguments.
pub fn unsigned_argument(value: f64) -> Result<u32, RuntimeError> {
    if !value.is_finite() || value < 0.0 || value > u32::MAX as f64 || value.fract() != 0.0 {
        return Err(RuntimeError::new(
            ErrorKind::Host,
            "expected an integer in 0..=4294967295",
        ));
    }
    Ok(value as u32)
}

pub fn primitive(family: &str, input: &str) -> Result<String, String> {
    match family {
        "numeric" => crate::numeric::probe(input),
        "regex" => crate::regex::probe(input),
        _ => Err("unknown probe family".into()),
    }
}

pub fn identity(value: Value) -> Result<Value, RuntimeError> {
    if matches!(
        value,
        Value::Object(_) | Value::Function(_) | Value::Class(_) | Value::TypeAlias(_)
    ) {
        return Err(unsupported(
            "object handles cannot cross the probe boundary",
        ));
    }
    let mut vm = Vm::new(Arc::new(Program::new(vec![vec![
        Instruction::Input(0),
        Instruction::Return,
    ]])));
    vm.start(0, vec![value])?;
    finish(&mut vm)
}

pub fn evaluate_member(source: &str, member: &str) -> Result<Value, String> {
    let syntax = crate::parser::parse_module(source).map_err(|e| e.to_string())?;
    let resolved = crate::resolver::resolve_module(&syntax).map_err(|e| e.to_string())?;
    let mut program = crate::vm::compile_module(&resolved).map_err(|e| e.to_string())?;
    let module_name = program.module_name.clone();
    Arc::make_mut(&mut program)
        .register_primary_source(
            Arc::new(crate::value::RuntimeSource {
                text: source.into(),
                uri: "repl:text".into(),
                module_name,
                line_anchors: false,
            }),
            crate::modules::ModuleIdentity {
                original_uri: "repl:text".into(),
                resolved_uri: "repl:text".into(),
            },
        )
        .map_err(|e| e.to_string())?;
    let entry = program.entry;
    let mut vm = Vm::new(program);
    vm.start(entry, vec![]).map_err(|e| e.to_string())?;
    let object = finish(&mut vm)
        .and_then(|v| v.as_object())
        .map_err(|e| e.to_string())?;
    vm.start_member(object, member).map_err(|e| e.to_string())?;
    let value = finish(&mut vm).map_err(|e| e.to_string())?;
    if matches!(
        value,
        Value::Object(_) | Value::Function(_) | Value::Class(_) | Value::TypeAlias(_)
    ) {
        return Err("object results require evaluator-owned handles".into());
    }
    Ok(value)
}

fn finish(vm: &mut Vm) -> Result<Value, RuntimeError> {
    let start = vm.instructions_executed();
    loop {
        let remaining =
            1_000_000usize.saturating_sub((vm.instructions_executed() - start) as usize);
        if remaining == 0 {
            return Err(unsupported("probe instruction limit exceeded"));
        }
        match vm.poll(remaining) {
            VmPoll::Ready(value) => return value,
            VmPoll::Yielded => return Err(unsupported("probe instruction limit exceeded")),
            VmPoll::Module(request) => {
                let completion = crate::modules::default_completion(&request)
                    .ok_or_else(|| unsupported("use HostSession for asynchronous evaluation"))?;
                vm.complete_module(request.id, completion)?;
            }
            VmPoll::Pending(_) | VmPoll::Log(_) => {
                return Err(unsupported("use HostSession for asynchronous evaluation"));
            }
        }
    }
}
fn unsupported(message: &str) -> RuntimeError {
    RuntimeError::new(ErrorKind::Unsupported, message)
}

pub fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(text, "{byte:02x}").unwrap();
    }
    text
}
pub fn encode_value(value: Value) -> String {
    match value {
        Value::Null | Value::NullWithDefault(_) => "null\t".into(),
        Value::Bool(v) => format!("bool\t{v}"),
        Value::Int(v) => format!("int\t{v}"),
        Value::Float(v) => format!("float\t{:016x}", v.to_bits()),
        Value::String(v) => format!("string\t{}", hex(v.to_java_utf8().as_bytes())),
        Value::Bytes(v) => format!("bytes\t{}", hex(&v)),
        Value::Object(_) => "unsupported\tobject".into(),
        Value::Function(_) => "unsupported\tfunction".into(),
        Value::Class(_) => "unsupported\tclass".into(),
        Value::TypeAlias(_) => "unsupported\ttype-alias".into(),
    }
}

pub struct HostProbe {
    session: HostSession,
}
impl HostProbe {
    pub fn new(mode: &str) -> Result<Self, RuntimeError> {
        let code = match mode {
            "read" => vec![
                Instruction::Input(0),
                Instruction::ReadResource,
                Instruction::Return,
            ],
            "add" => vec![
                Instruction::Constant(Value::Int(40.into())),
                Instruction::Input(0),
                Instruction::ReadResource,
                Instruction::Add,
                Instruction::Return,
            ],
            "loop" => vec![Instruction::Jump(0)],
            _ => return Err(unsupported("unknown Host probe mode")),
        };
        Ok(Self {
            session: HostSession::new(Arc::new(Program::new(vec![
                code,
                vec![
                    Instruction::Constant(Value::Int(100.into())),
                    Instruction::Input(0),
                    Instruction::ReadResource,
                    Instruction::Add,
                    Instruction::Return,
                ],
            ]))),
        })
    }
    pub fn start(&mut self, now: u32, timeout: u32) -> Result<(), RuntimeError> {
        self.session.start(
            0,
            vec![Value::string("test:/resource")],
            now.into(),
            Some(timeout.into()),
        )
    }
    pub fn poll(&mut self, now: u32, budget: u32) -> String {
        match self.session.poll(now.into(), budget as usize) {
            HostPoll::Yielded => "yield".into(),
            HostPoll::Read { token, uri } => format!("read\t{token}\t{}", hex(uri.as_bytes())),
            HostPoll::Log { .. } => {
                unreachable!("fixed-bytecode Host probe has no trace instructions")
            }
            HostPoll::Module { .. } => {
                unreachable!("fixed-bytecode Host probe has no module instructions")
            }
            HostPoll::Ready(Ok(v)) => format!("ready\t{}", encode_value(v)),
            HostPoll::Ready(Err(e)) => {
                format!(
                    "error\t{:?}\t{}",
                    e.kind,
                    hex(e.message.to_java_utf8().as_bytes())
                )
            }
        }
    }
    pub fn reenter(&mut self, token: &str, now: u32, timeout: u32) -> Result<(), RuntimeError> {
        self.session.reenter(
            token,
            1,
            vec![Value::string("test:/resource")],
            now.into(),
            Some(timeout.into()),
        )
    }
    /// Drain the shared Host's exactly-once abort signals for platform I/O adapters.
    pub fn drain_cancelled_reads(&mut self) -> Vec<String> {
        self.session.drain_cancelled_reads()
    }
    pub fn respond(&mut self, token: &str, value: Value, now: u32) -> Result<(), RuntimeError> {
        self.session.resume(token, Ok(value), now.into())
    }
    pub fn reject(&mut self, token: &str, message: &str, now: u32) -> Result<(), RuntimeError> {
        self.session.resume(
            token,
            Err(RuntimeError::new(ErrorKind::Host, message)),
            now.into(),
        )
    }
    pub fn cancel(&mut self) {
        self.session.cancel();
    }
    pub fn close(&mut self) {
        self.session.close();
    }
}

/// A lossless development transport record. Text is UTF-16 code units, not a
/// JavaScript UTF-8 conversion or a rendered imitation of a logger callback.
#[derive(Clone, Debug)]
pub struct SourceLog {
    pub level: &'static str,
    pub message: Vec<u16>,
    pub module_uri: Vec<u16>,
    pub member_name: Option<Vec<u16>>,
    pub source_lines: Vec<Vec<u16>>,
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
}

#[derive(Clone, Debug, Default)]
pub struct SourceEvent {
    pub kind: &'static str,
    pub token: Option<String>,
    pub uri: Option<String>,
    pub value: Option<Vec<u16>>,
    pub bytes: Option<Vec<u8>>,
    pub error: Option<Vec<u16>>,
    pub error_bytes: Option<Vec<u8>>,
    pub log: Option<SourceLog>,
    pub module: Option<SourceModuleRequest>,
    pub exit_code: Option<i32>,
    pub phase: Option<&'static str>,
    pub disposition: Option<&'static str>,
    pub reason: Option<&'static str>,
}

/// Module URIs are validated UTF-8 identities. The decimal request ID avoids
/// narrowing the shared u64 counter through a JavaScript Number.
#[derive(Clone, Debug)]
pub struct SourceModuleRequest {
    pub id: String,
    pub kind: &'static str,
    pub importer_original_uri: Option<String>,
    pub importer_resolved_uri: Option<String>,
    pub import_uri: Option<String>,
    pub original_uri: String,
    pub resolved_uri: Option<String>,
    pub source_uri: Option<String>,
}
impl From<crate::modules::ModuleRequest> for SourceModuleRequest {
    fn from(request: crate::modules::ModuleRequest) -> Self {
        use crate::modules::ModuleRequestKind;
        match request.kind {
            ModuleRequestKind::CheckImport {
                importer,
                import_uri,
            } => Self {
                id: request.id.to_string(),
                kind: "check-import",
                importer_original_uri: Some(importer.original_uri.to_string()),
                importer_resolved_uri: Some(importer.resolved_uri.to_string()),
                import_uri: Some(import_uri.to_string()),
                // No ModuleKey has been selected yet. Keep the transport's
                // required target field equal to the prepared edge URI.
                original_uri: import_uri.to_string(),
                resolved_uri: None,
                source_uri: None,
            },
            ModuleRequestKind::Capabilities { module } => Self {
                id: request.id.to_string(),
                kind: "capabilities",
                importer_original_uri: Some(module.original_uri.to_string()),
                importer_resolved_uri: Some(module.resolved_uri.to_string()),
                import_uri: None,
                original_uri: module.original_uri.to_string(),
                resolved_uri: None,
                source_uri: None,
            },
            ModuleRequestKind::HasElement {
                module,
                element_uri,
            } => Self {
                id: request.id.to_string(),
                kind: "has-element",
                importer_original_uri: Some(module.original_uri.to_string()),
                importer_resolved_uri: Some(module.resolved_uri.to_string()),
                import_uri: None,
                original_uri: element_uri.to_string(),
                resolved_uri: None,
                source_uri: None,
            },
            ModuleRequestKind::Resolve {
                importer,
                import_uri,
                original_uri,
            } => Self {
                id: request.id.to_string(),
                kind: "resolve",
                importer_original_uri: Some(importer.original_uri.to_string()),
                importer_resolved_uri: Some(importer.resolved_uri.to_string()),
                import_uri: Some(import_uri.to_string()),
                original_uri: original_uri.to_string(),
                resolved_uri: None,
                source_uri: None,
            },
            ModuleRequestKind::LoadSource { resolved } => Self {
                id: request.id.to_string(),
                kind: "load-source",
                importer_original_uri: None,
                importer_resolved_uri: None,
                import_uri: None,
                original_uri: resolved.original_uri.to_string(),
                resolved_uri: Some(resolved.resolved_uri.to_string()),
                source_uri: Some(resolved.source_uri.to_string()),
            },
        }
    }
}
impl SourceEvent {
    fn new(kind: &'static str) -> Self {
        Self {
            kind,
            token: None,
            uri: None,
            value: None,
            bytes: None,
            error: None,
            error_bytes: None,
            log: None,
            module: None,
            exit_code: None,
            phase: None,
            disposition: None,
            reason: None,
        }
    }
}

/// Shared actual-source session used by native development tools and JS bindings.
/// Poll returns before the caller executes a logger/transformer callback, so the
/// same evaluator can be reentered without holding a Rust mutable borrow.
pub struct SourceProbe {
    session: crate::evaluation::session::SourceSession,
    output: bool,
    properties: Vec<(String, String)>,
    environment: Vec<(String, String)>,
}
impl SourceProbe {
    /// Legacy constructor; Java UTF-8 conversion is explicit at this boundary.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        source: &str,
        uri: &str,
        module_name: &str,
        paths: &[String],
        trace_mode: &str,
        host_logger: bool,
        host_frame_transformer: bool,
        power_assertions: bool,
    ) -> Result<Self, String> {
        Self::new_text(
            source,
            uri,
            module_name,
            paths,
            trace_mode,
            host_logger,
            host_frame_transformer,
            power_assertions,
        )
        .map_err(|text| text.to_java_utf8())
    }
    #[allow(clippy::too_many_arguments)]
    pub fn new_text(
        source: &str,
        uri: &str,
        module_name: &str,
        paths: &[String],
        trace_mode: &str,
        host_logger: bool,
        host_frame_transformer: bool,
        power_assertions: bool,
    ) -> Result<Self, PklString> {
        Ok(Self {
            session: crate::evaluation::session::SourceSession::compile_text(
                source,
                uri,
                module_name,
                paths,
                Self::options(
                    trace_mode,
                    host_logger,
                    host_frame_transformer,
                    power_assertions,
                )
                .map_err(PklString::from)?,
            )?,
            output: false,
            properties: Vec::new(),
            environment: Vec::new(),
        })
    }
    #[allow(clippy::too_many_arguments)]
    pub fn new_expressions(
        source: &str,
        uri: &str,
        module_name: &str,
        expressions: &[String],
        trace_mode: &str,
        host_logger: bool,
        host_frame_transformer: bool,
        power_assertions: bool,
    ) -> Result<Self, String> {
        Self::new_expressions_text(
            source,
            uri,
            module_name,
            expressions,
            trace_mode,
            host_logger,
            host_frame_transformer,
            power_assertions,
        )
        .map_err(|text| text.to_java_utf8())
    }
    #[allow(clippy::too_many_arguments)]
    pub fn new_expressions_text(
        source: &str,
        uri: &str,
        module_name: &str,
        expressions: &[String],
        trace_mode: &str,
        host_logger: bool,
        host_frame_transformer: bool,
        power_assertions: bool,
    ) -> Result<Self, PklString> {
        Ok(Self {
            session: crate::evaluation::session::SourceSession::compile_expressions_text(
                source,
                uri,
                module_name,
                expressions,
                Self::options(
                    trace_mode,
                    host_logger,
                    host_frame_transformer,
                    power_assertions,
                )
                .map_err(PklString::from)?,
            )?,
            output: false,
            properties: Vec::new(),
            environment: Vec::new(),
        })
    }
    pub fn options(
        trace_mode: &str,
        host_logger: bool,
        host_frame_transformer: bool,
        power_assertions: bool,
    ) -> Result<crate::evaluation::session::SourceSessionOptions, String> {
        use crate::logging::{LogDelivery, TraceMode};
        Ok(crate::evaluation::session::SourceSessionOptions {
            evaluation: crate::evaluation::EvaluationOptions { power_assertions },
            trace_mode: match trace_mode {
                "compact" | "COMPACT" => TraceMode::Compact,
                "pretty" | "PRETTY" => TraceMode::Pretty,
                _ => return Err("expected trace mode compact or pretty".into()),
            },
            log_delivery: if host_logger {
                LogDelivery::Host
            } else {
                LogDelivery::Noop
            },
            host_frame_transformer,
        })
    }
    pub fn new_output(
        input: crate::evaluation::frontend::ModuleSourceInput<'_>,
        paths: &[String],
        module_options: &crate::evaluation::frontend::ModuleEvalOptions,
        session_options: crate::evaluation::session::SourceSessionOptions,
    ) -> Result<Self, crate::evaluation::frontend::ModuleFrontendResult> {
        Self::new_output_text(input, paths, module_options, session_options).map_err(Into::into)
    }
    pub fn new_output_text(
        input: crate::evaluation::frontend::ModuleSourceInput<'_>,
        paths: &[String],
        module_options: &crate::evaluation::frontend::ModuleEvalOptions,
        session_options: crate::evaluation::session::SourceSessionOptions,
    ) -> Result<Self, crate::evaluation::frontend::ModuleFrontendFailure> {
        Ok(Self {
            session: crate::evaluation::session::SourceSession::compile_output_text(
                input,
                paths,
                module_options,
                session_options,
            )?,
            output: true,
            properties: module_options.properties.clone(),
            environment: module_options.environment.clone(),
        })
    }
    pub fn start_output_bytes(
        &mut self,
        now: u32,
        timeout: Option<u32>,
    ) -> Result<(), RuntimeError> {
        self.session
            .start_output_bytes(now.into(), timeout.map(Into::into))
    }
    pub fn reenter_output_bytes(
        &mut self,
        token: &str,
        now: u32,
        timeout: Option<u32>,
    ) -> Result<(), RuntimeError> {
        self.session
            .reenter_output_bytes(token, now.into(), timeout.map(Into::into))
    }
    pub fn start_module(&mut self, now: u32, timeout: Option<u32>) -> Result<(), RuntimeError> {
        self.session
            .start_module(now.into(), timeout.map(Into::into))
    }
    pub fn start_path(
        &mut self,
        index: u32,
        now: u32,
        timeout: Option<u32>,
    ) -> Result<(), RuntimeError> {
        self.session
            .start_path(index as usize, now.into(), timeout.map(Into::into))
    }
    pub fn setup_failed(&self) -> bool {
        self.session.setup_failed()
    }
    pub fn reenter_path(
        &mut self,
        token: &str,
        index: u32,
        now: u32,
        timeout: Option<u32>,
    ) -> Result<(), RuntimeError> {
        self.session
            .reenter_path(token, index as usize, now.into(), timeout.map(Into::into))
    }
    pub fn poll(&mut self, now: u32, budget: u32) -> Result<SourceEvent, RuntimeError> {
        use crate::logging::LogLevel;
        let previous_depth = self.session.context_depth();
        let event = match self.session.poll(now.into(), budget as usize) {
            HostPoll::Yielded => SourceEvent::new("yield"),
            HostPoll::Read { token, uri } => {
                let mut event = SourceEvent::new("read");
                event.token = Some(token);
                event.uri = Some(uri.to_string());
                event
            }
            HostPoll::Module { token, request } => {
                let mut event = SourceEvent::new("module-request");
                event.token = Some(token);
                event.module = Some(request.into());
                event
            }
            HostPoll::Log { token, event: log } => {
                let mut event = SourceEvent::new("log");
                event.token = Some(token);
                let position = |n| {
                    u32::try_from(n).map_err(|_| {
                        unsupported("logger frame position exceeds the development transport range")
                    })
                };
                event.log = Some(SourceLog {
                    level: match log.level {
                        LogLevel::Trace => "trace",
                        LogLevel::Warn => "warn",
                    },
                    message: log.message.encode_utf16().collect(),
                    module_uri: log.frame.module_uri.encode_utf16().collect(),
                    member_name: log
                        .frame
                        .member_name
                        .as_ref()
                        .map(|s| s.encode_utf16().collect()),
                    source_lines: log
                        .frame
                        .source_lines
                        .iter()
                        .map(|s| s.encode_utf16().collect())
                        .collect(),
                    start_line: position(log.frame.start_line)?,
                    start_column: position(log.frame.start_column)?,
                    end_line: position(log.frame.end_line)?,
                    end_column: position(log.frame.end_column)?,
                });
                event
            }
            HostPoll::Ready(Ok(Value::Object(_))) => SourceEvent::new("module"),
            HostPoll::Ready(Ok(Value::Bytes(bytes))) => {
                let mut event = SourceEvent::new("bytes");
                event.bytes = Some(bytes.to_vec());
                event.exit_code = Some(0);
                event.phase =
                    Some(crate::evaluation::frontend::FrontendPhase::ModuleOutput.as_str());
                event
            }
            HostPoll::Ready(Ok(value)) => {
                let value = crate::evaluation::session::SourceSession::result_string(value)?;
                let mut event = SourceEvent::new("value");
                event.value = Some(value.encode_utf16().collect());
                event
            }
            HostPoll::Ready(Err(error)) => {
                if previous_depth > 0 && self.session.context_depth() == previous_depth {
                    return Err(error);
                }
                let mut event = SourceEvent::new("error");
                // This is shared implementation classification, not a Java API
                // process exit. Expression diagnostics retain their raw units.
                let failure = self.session.output_failure_text(error.clone());
                event.phase = Some(failure.phase.as_str());
                event.disposition = Some(failure.disposition.as_str());
                event.reason = failure.reason;
                if self.output {
                    let failure = self.session.output_session_failure_text(error);
                    event.error = Some(failure.text.encode_utf16().collect());
                    let failure: crate::evaluation::frontend::ModuleFrontendResult = failure.into();
                    event.error_bytes = Some(failure.stderr);
                    event.exit_code = Some(failure.exit_code);
                } else {
                    event.error = Some(
                        self.session
                            .format_error_text(error)
                            .encode_utf16()
                            .collect(),
                    );
                }
                event
            }
        };
        Ok(event)
    }
    pub fn respond(&mut self, token: &str, value: Value, now: u32) -> Result<(), RuntimeError> {
        self.session.resume(token, Ok(value), now.into())
    }
    pub fn respond_configured(&mut self, token: &str, now: u32) -> Result<(), RuntimeError> {
        self.session
            .resume_default_read(token, &self.properties, &self.environment, now.into())
    }
    pub fn complete_read(
        &mut self,
        token: &str,
        result: Result<crate::resources::ReadResult, crate::host::ReadFailure>,
        now: u32,
    ) -> Result<(), RuntimeError> {
        self.session.complete_read(token, result, now.into())
    }
    pub fn complete_read_missing(&mut self, token: &str, now: u32) -> Result<(), RuntimeError> {
        self.complete_read(token, Ok(crate::resources::ReadResult::Missing), now)
    }
    pub fn complete_read_text(
        &mut self,
        token: &str,
        value: crate::value::PklString,
        now: u32,
    ) -> Result<(), RuntimeError> {
        self.complete_read(token, Ok(crate::resources::ReadResult::Text(value)), now)
    }
    pub fn complete_read_resource(
        &mut self,
        token: &str,
        uri: crate::value::PklString,
        bytes: Vec<u8>,
        now: u32,
    ) -> Result<(), RuntimeError> {
        self.complete_read(
            token,
            Ok(crate::resources::ReadResult::Resource(
                crate::resources::Resource {
                    uri,
                    bytes: bytes.into(),
                },
            )),
            now,
        )
    }
    pub fn reject_read_security_manager(
        &mut self,
        token: &str,
        message: Option<crate::value::PklString>,
        now: u32,
    ) -> Result<(), RuntimeError> {
        self.complete_read(
            token,
            Err(crate::host::ReadFailure::SecurityManagerException { message }),
            now,
        )
    }
    /// URI delivered to a typed reader, distinct from the legacy event's
    /// authored operand. Parsing and resolution stay in the shared runtime.
    pub fn resolved_read_uri(&self, token: &str) -> Result<String, RuntimeError> {
        crate::host::resources::resolved_uri(&self.session.read_request(token)?)
    }
    pub fn module_request(
        &self,
        token: &str,
    ) -> Result<crate::modules::ModuleRequest, RuntimeError> {
        self.session.module_request(token)
    }
    pub fn read_request(&self, token: &str) -> Result<crate::vm::ReadRequest, RuntimeError> {
        self.session.read_request(token)
    }
    pub fn complete_module(
        &mut self,
        token: &str,
        completion: crate::modules::ModuleCompletion,
        now: u32,
    ) -> Result<(), RuntimeError> {
        self.session.complete_module(token, completion, now.into())
    }
    pub fn complete_module_capabilities(
        &mut self,
        token: &str,
        local: bool,
        hierarchical: bool,
        fragment_paths: bool,
        now: u32,
    ) -> Result<(), RuntimeError> {
        self.complete_module(
            token,
            crate::modules::ModuleCompletion::Capabilities(Ok(
                crate::modules::ModuleCapabilities {
                    local,
                    hierarchical,
                    fragment_paths,
                },
            )),
            now,
        )
    }

    pub fn complete_module_import_check(
        &mut self,
        token: &str,
        now: u32,
    ) -> Result<(), RuntimeError> {
        self.complete_module(
            token,
            crate::modules::ModuleCompletion::ImportChecked(Ok(())),
            now,
        )
    }

    pub fn reject_module_import_check(
        &mut self,
        token: &str,
        kind: &str,
        message: Option<crate::value::PklString>,
        now: u32,
    ) -> Result<(), RuntimeError> {
        let failure = Self::module_failure(kind, message)?;
        self.complete_module(
            token,
            crate::modules::ModuleCompletion::ImportChecked(Err(failure)),
            now,
        )
    }

    pub fn complete_default_module(&mut self, token: &str, now: u32) -> Result<bool, RuntimeError> {
        self.session.complete_default_module(token, now.into())
    }
    pub fn complete_module_has_element(
        &mut self,
        token: &str,
        exists: bool,
        now: u32,
    ) -> Result<(), RuntimeError> {
        self.complete_module(
            token,
            crate::modules::ModuleCompletion::HasElement(Ok(exists)),
            now,
        )
    }
    pub fn complete_module_resolution(
        &mut self,
        token: &str,
        original_uri: &str,
        resolved_uri: &str,
        source_uri: &str,
        now: u32,
    ) -> Result<(), RuntimeError> {
        self.complete_module(
            token,
            crate::modules::ModuleCompletion::Resolved(Ok(crate::modules::ResolvedModule {
                original_uri: original_uri.into(),
                resolved_uri: resolved_uri.into(),
                source_uri: source_uri.into(),
            })),
            now,
        )
    }
    pub fn complete_module_source(
        &mut self,
        token: &str,
        text: &str,
        inferred_name: &str,
        now: u32,
    ) -> Result<(), RuntimeError> {
        self.complete_module(
            token,
            crate::modules::ModuleCompletion::Source(Ok(crate::modules::ModuleSource {
                text: text.into(),
                inferred_name: inferred_name.into(),
            })),
            now,
        )
    }
    /// Legacy text convenience wrapper; the typed path below accepts all UTF-16.
    pub fn reject_module(
        &mut self,
        token: &str,
        loading_source: bool,
        kind: &str,
        message: &str,
        now: u32,
    ) -> Result<(), RuntimeError> {
        self.reject_module_text(token, loading_source, kind, Some(message.into()), now)
    }
    pub fn reject_module_text(
        &mut self,
        token: &str,
        loading_source: bool,
        kind: &str,
        message: Option<crate::value::PklString>,
        now: u32,
    ) -> Result<(), RuntimeError> {
        use crate::modules::ModuleCompletion;
        let failure = Self::module_failure(kind, message)?;
        self.complete_module(
            token,
            if loading_source {
                ModuleCompletion::Source(Err(failure))
            } else {
                ModuleCompletion::Resolved(Err(failure))
            },
            now,
        )
    }
    fn module_failure(
        kind: &str,
        message: Option<crate::value::PklString>,
    ) -> Result<crate::modules::ModuleHostError, RuntimeError> {
        use crate::modules::ModuleHostError;
        Ok(match kind {
            "not-found" => ModuleHostError::NotFound,
            "io" => ModuleHostError::Io(crate::host::IoFailure::new(message)),
            "security-manager-exception" => ModuleHostError::SecurityManagerException { message },
            "denied" | "unsupported" => {
                let message = message.ok_or_else(|| {
                    RuntimeError::new(ErrorKind::Host, "this module Host error requires a message")
                })?;
                if kind == "denied" {
                    ModuleHostError::Denied(message)
                } else {
                    ModuleHostError::Unsupported(message)
                }
            }
            _ => {
                return Err(RuntimeError::new(
                    ErrorKind::Host,
                    "unknown module Host error kind",
                ));
            }
        })
    }
    pub fn reject_module_preparation_text(
        &mut self,
        token: &str,
        has_element: bool,
        kind: &str,
        message: Option<crate::value::PklString>,
        now: u32,
    ) -> Result<(), RuntimeError> {
        let failure = Self::module_failure(kind, message)?;
        self.complete_module(
            token,
            if has_element {
                crate::modules::ModuleCompletion::HasElement(Err(failure))
            } else {
                crate::modules::ModuleCompletion::Capabilities(Err(failure))
            },
            now,
        )
    }
    /// A resource-reader IOException, distinct from raw diagnostic injection.
    pub fn reject_read_io(
        &mut self,
        token: &str,
        message: Option<crate::value::PklString>,
        now: u32,
    ) -> Result<(), RuntimeError> {
        self.session.reject_read(
            token,
            crate::host::ReadFailure::Io(crate::host::IoFailure::new(message)),
            now.into(),
        )
    }
    pub fn drain_cancelled_modules(&mut self) -> Vec<String> {
        self.session.drain_cancelled_modules()
    }
    pub fn reject(&mut self, token: &str, message: &str, now: u32) -> Result<(), RuntimeError> {
        self.reject_text(token, message.into(), now)
    }
    /// Inject an already constructed Host diagnostic; this is not an IOException.
    pub fn reject_text(
        &mut self,
        token: &str,
        message: crate::value::PklString,
        now: u32,
    ) -> Result<(), RuntimeError> {
        self.session.resume(
            token,
            Err(RuntimeError::new(ErrorKind::Host, message)),
            now.into(),
        )
    }
    pub fn acknowledge_log(
        &mut self,
        token: &str,
        failure: Option<Vec<u16>>,
        now: u32,
    ) -> Result<(), RuntimeError> {
        self.session.acknowledge_log(
            token,
            failure.map_or(Ok(()), |units| {
                Err(crate::logging::LoggerFailure::new(
                    crate::value::PklString::from_utf16(units),
                ))
            }),
            now.into(),
        )
    }
    pub fn close_from_callback(&mut self, token: &str) -> Result<(), RuntimeError> {
        self.session.close_from_callback(token)
    }
    pub fn collect(&mut self) -> crate::heap::HeapStats {
        self.session.collect(&[])
    }
    pub fn heap_slots(&self) -> usize {
        self.session.heap_stats().slots
    }
    pub fn context_depth(&self) -> usize {
        self.session.context_depth()
    }
    pub fn drain_cancelled_reads(&mut self) -> Vec<String> {
        self.session.drain_cancelled_reads()
    }
    pub fn cancel(&mut self) {
        self.session.cancel();
    }
    pub fn close(&mut self) {
        self.session.close();
    }
}
