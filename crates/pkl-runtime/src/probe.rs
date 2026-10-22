//! Explicitly experimental M1 adapter entry points, not the final public Pkl API.
use crate::{
    host::{HostPoll, HostSession},
    value::{ErrorKind, RuntimeError, Value},
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
    let program = crate::vm::compile_module(&resolved).map_err(|e| e.to_string())?;
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
    match vm.poll(1_000_000) {
        VmPoll::Ready(value) => value,
        VmPoll::Yielded => Err(unsupported("probe instruction limit exceeded")),
        VmPoll::Pending(_) | VmPoll::Log(_) => {
            Err(unsupported("use HostSession for asynchronous evaluation"))
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
                Instruction::Constant(Value::Int(40)),
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
                    Instruction::Constant(Value::Int(100)),
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
            HostPoll::Ready(Ok(v)) => format!("ready\t{}", encode_value(v)),
            HostPoll::Ready(Err(e)) => {
                format!("error\t{:?}\t{}", e.kind, hex(e.message.as_bytes()))
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

#[derive(Clone, Debug)]
pub struct SourceEvent {
    pub kind: &'static str,
    pub token: Option<String>,
    pub uri: Option<String>,
    pub value: Option<Vec<u16>>,
    pub error: Option<Vec<u16>>,
    pub log: Option<SourceLog>,
}
impl SourceEvent {
    fn new(kind: &'static str) -> Self {
        Self {
            kind,
            token: None,
            uri: None,
            value: None,
            error: None,
            log: None,
        }
    }
}

/// Shared actual-source session used by native development tools and JS bindings.
/// Poll returns before the caller executes a logger/transformer callback, so the
/// same evaluator can be reentered without holding a Rust mutable borrow.
pub struct SourceProbe {
    session: crate::evaluation::session::SourceSession,
}
impl SourceProbe {
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
        use crate::logging::{LogDelivery, TraceMode};
        let trace_mode = match trace_mode {
            "compact" | "COMPACT" => TraceMode::Compact,
            "pretty" | "PRETTY" => TraceMode::Pretty,
            _ => return Err("expected trace mode compact or pretty".into()),
        };
        Ok(Self {
            session: crate::evaluation::session::SourceSession::compile(
                source,
                uri,
                module_name,
                paths,
                crate::evaluation::session::SourceSessionOptions {
                    evaluation: crate::evaluation::EvaluationOptions { power_assertions },
                    trace_mode,
                    log_delivery: if host_logger {
                        LogDelivery::Host
                    } else {
                        LogDelivery::Noop
                    },
                    host_frame_transformer,
                },
            )?,
        })
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
                let message = self.session.format_error(error);
                let mut event = SourceEvent::new("error");
                event.error = Some(message.encode_utf16().collect());
                event
            }
        };
        Ok(event)
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
