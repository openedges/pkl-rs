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
        VmPoll::Pending(_) => Err(unsupported("use HostSession for asynchronous evaluation")),
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
        Value::String(v) => format!("string\t{}", hex(v.as_bytes())),
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
