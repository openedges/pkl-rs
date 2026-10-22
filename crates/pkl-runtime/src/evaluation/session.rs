//! Development-only parsed-source Host session. Paths are fixed before execution;
//! this is not a general REPL or module loader.
use super::{EvaluationOptions, compile_program, runtime_error};
use crate::{
    heap::HeapStats,
    host::{HostPoll, HostSession},
    logging::{EvaluationContext, LogDelivery, LoggerFailure, TraceMode},
    source::{Source, SourceMap, Span},
    value::{ErrorKind, PklString, RuntimeError, RuntimeSource, Value},
    vm::{BlockMetadata, Instruction},
};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, Default)]
pub struct SourceSessionOptions {
    pub evaluation: EvaluationOptions,
    pub trace_mode: TraceMode,
    pub log_delivery: LogDelivery,
    pub host_frame_transformer: bool,
}

#[derive(Clone, Copy)]
enum Call {
    Module,
    Path(usize),
    InitialPath(usize),
}

/// Owns one actual compiled source, module receiver, heap and Host context stack.
/// Raw heap handles stay inside the runtime or native caller's explicit roots.
pub struct SourceSession {
    host: HostSession,
    source: Source,
    uri: String,
    name: String,
    paths: Vec<String>,
    entries: Vec<usize>,
    module_entry: usize,
    module: Option<Value>,
    module_started: bool,
    setup_failure: Option<RuntimeError>,
    initialization_failed: bool,
    calls: Vec<Call>,
    last_expression: Option<usize>,
}

impl SourceSession {
    pub fn compile(
        text: &str,
        uri: &str,
        module_name: &str,
        property_paths: &[String],
        options: SourceSessionOptions,
    ) -> Result<Self, String> {
        let mut sources = SourceMap::default();
        let id = sources.add(uri, text);
        let source = sources.get(id).expect("inserted source").clone();
        let (mut program, name) = compile_program(
            &source,
            uri,
            module_name,
            property_paths.first().map_or("", String::as_str),
            options.evaluation,
        )?;
        let module_entry = program.entry;
        let program_mut = Arc::make_mut(&mut program);
        let mut entries = Vec::with_capacity(property_paths.len());
        for expression in property_paths {
            let mut block = Vec::new();
            let mut spans = Vec::new();
            let mut prefix_end = 0;
            for (index, member) in expression.split('.').enumerate() {
                if member.is_empty() || !member.chars().all(|ch| ch.is_alphanumeric() || ch == '_')
                {
                    return Err(
                        "the M2 source session currently accepts property paths only".into(),
                    );
                }
                prefix_end += usize::from(prefix_end > 0) + member.len();
                block.push(if index == 0 {
                    Instruction::GetImplicitProperty(member.into())
                } else {
                    Instruction::GetMember(member.into())
                });
                spans.push(Some(Span::new(0, prefix_end)));
            }
            // Java's expression-string API invokes ToString on the returned
            // value with an unavailable callsite. The shared opcode owns forcing
            // and virtual dispatch; adapters do not render compound values.
            block.push(Instruction::ToString);
            spans.push(None);
            block.push(Instruction::Return);
            spans.push(None);
            entries.push(program_mut.blocks.len());
            program_mut.blocks.push(block);
            let metadata = BlockMetadata {
                source: Some(Arc::new(RuntimeSource {
                    text: expression.clone().into(),
                    uri: "repl:text".into(),
                    module_name: "".into(),
                    line_anchors: false,
                })),
                spans,
                module_receiver_input: Some(0),
                ..Default::default()
            };
            program_mut.metadata.push(metadata);
        }
        let context = EvaluationContext {
            primary_source: Some(Arc::new(RuntimeSource {
                text: text.into(),
                uri: uri.into(),
                module_name: name.clone().into(),
                line_anchors: false,
            })),
            trace_mode: options.trace_mode,
            log_delivery: options.log_delivery,
            host_frame_transformer: options.host_frame_transformer,
        };
        let mut host = HostSession::with_context(program, context);
        host.set_power_assertions(options.evaluation.power_assertions);
        Ok(Self {
            host,
            source,
            uri: uri.to_owned(),
            name,
            paths: property_paths.to_vec(),
            entries,
            module_entry,
            module: None,
            module_started: false,
            setup_failure: None,
            initialization_failed: false,
            calls: Vec::new(),
            last_expression: None,
        })
    }

    pub fn start_module(
        &mut self,
        now_ms: u64,
        timeout_ms: Option<u64>,
    ) -> Result<(), RuntimeError> {
        self.open()?;
        if self.module_started {
            return Err(error("module setup has already started"));
        }
        self.host
            .start(self.module_entry, Vec::new(), now_ms, timeout_ms)?;
        self.module_started = true;
        self.calls.push(Call::Module);
        self.last_expression = None;
        Ok(())
    }

    pub fn start_path(
        &mut self,
        index: usize,
        now_ms: u64,
        timeout_ms: Option<u64>,
    ) -> Result<(), RuntimeError> {
        self.open()?;
        let entry = self.entry(index)?;
        if !self.module_started {
            self.host
                .start_then(self.module_entry, Vec::new(), entry, now_ms, timeout_ms)?;
            self.module_started = true;
            self.calls.push(Call::InitialPath(index));
            self.last_expression = None;
            return Ok(());
        }
        if let Some(error) = &self.setup_failure {
            self.host.start_failed(error.clone(), now_ms, timeout_ms)?;
            self.calls.push(Call::Path(index));
            self.last_expression = Some(index);
            return Ok(());
        }
        let module = self
            .module
            .clone()
            .ok_or_else(|| error("module setup has not completed"))?;
        self.host.start(entry, vec![module], now_ms, timeout_ms)?;
        self.calls.push(Call::Path(index));
        self.last_expression = Some(index);
        Ok(())
    }

    pub fn reenter_path(
        &mut self,
        parent_token: &str,
        index: usize,
        now_ms: u64,
        timeout_ms: Option<u64>,
    ) -> Result<(), RuntimeError> {
        self.open()?;
        let entry = self.entry(index)?;
        // This is the VM's actual bootstrap receiver, never a second allocation
        // or a fabricated completed module. Publication semantics are bounded by
        // the shared VM's primary-module query.
        let module = self
            .module
            .clone()
            .or_else(|| self.host.primary_module())
            .ok_or_else(|| error("same-module callback receiver is not yet published"))?;
        self.host
            .reenter(parent_token, entry, vec![module], now_ms, timeout_ms)?;
        self.calls.push(Call::Path(index));
        self.last_expression = Some(index);
        Ok(())
    }

    pub fn poll(&mut self, now_ms: u64, budget: usize) -> HostPoll {
        let previous_depth = self.host.context_depth();
        let event = self.host.poll(now_ms, budget);
        if let Some(result) = self.host.take_intermediate_result() {
            match result {
                Ok(module) => {
                    self.module = Some(module);
                    self.last_expression = self.calls.last().and_then(|call| match call {
                        Call::InitialPath(index) => Some(*index),
                        _ => None,
                    });
                }
                Err(error) => {
                    self.initialization_failed = true;
                    self.last_expression = None;
                    if !self.host.is_closed() {
                        self.setup_failure = Some(error);
                    }
                }
            }
        }
        if let HostPoll::Ready(result) = &event {
            let completed = self.host.context_depth() < previous_depth;
            if let Some(call) = completed.then(|| self.calls.pop()).flatten() {
                match call {
                    Call::Module => {
                        self.last_expression = None;
                        match result {
                            Ok(module) => self.module = Some(module.clone()),
                            Err(error) => {
                                if !matches!(
                                    error.kind,
                                    ErrorKind::Timeout | ErrorKind::Cancelled | ErrorKind::Closed
                                ) {
                                    self.initialization_failed = true;
                                    if !self.host.is_closed() {
                                        self.setup_failure = Some(error.clone());
                                    }
                                }
                            }
                        }
                    }
                    Call::Path(index) => self.last_expression = Some(index),
                    Call::InitialPath(index) => {
                        self.last_expression =
                            (self.module.is_some() && !self.initialization_failed).then_some(index);
                    }
                }
            }
            if self.host.is_closed() {
                self.calls.clear();
                self.module = None;
                self.setup_failure = None;
            }
        }
        event
    }

    pub fn resume(
        &mut self,
        token: &str,
        completion: Result<Value, RuntimeError>,
        now_ms: u64,
    ) -> Result<(), RuntimeError> {
        self.host.resume(token, completion, now_ms)
    }
    pub fn acknowledge_log(
        &mut self,
        token: &str,
        completion: Result<(), LoggerFailure>,
        now_ms: u64,
    ) -> Result<(), RuntimeError> {
        self.host.acknowledge_log(token, completion, now_ms)
    }
    pub fn close_from_callback(&mut self, token: &str) -> Result<(), RuntimeError> {
        self.host.close_from_callback(token)
    }
    pub fn cancel(&mut self) {
        self.host.cancel();
        self.release_module();
    }
    pub fn close(&mut self) {
        self.host.close();
        self.release_module();
    }
    pub fn is_closed(&self) -> bool {
        self.host.is_closed()
    }
    /// A recorded initialization failure survives cleanup of the cached error.
    /// Property errors and Host-wide cancellation/timeouts do not change it:
    /// termination before initialization completes is not an initialization result.
    pub fn setup_failed(&self) -> bool {
        self.initialization_failed
    }
    pub fn context_depth(&self) -> usize {
        self.host.context_depth()
    }
    pub fn drain_cancelled_reads(&mut self) -> Vec<String> {
        self.host.drain_cancelled_reads()
    }
    pub fn heap_stats(&self) -> HeapStats {
        self.host.heap_stats()
    }

    /// The retained module and a just-returned result are roots even after the
    /// Host has restored a suspended parent context.
    pub fn collect(&mut self, external_roots: &[Value]) -> HeapStats {
        let mut roots = external_roots.to_vec();
        if let Some(module) = &self.module {
            roots.push(module.clone());
        }
        self.host.collect(&roots)
    }
    pub fn format_error(&self, error: RuntimeError) -> String {
        let expression = self
            .last_expression
            .and_then(|index| self.paths.get(index))
            .map_or("", String::as_str);
        runtime_error(&self.source, &self.uri, &self.name, expression, error, None)
    }
    pub fn result_string(value: Value) -> Result<PklString, RuntimeError> {
        match value {
            Value::String(value) => Ok(value),
            _ => Err(error("source expression did not return a string")),
        }
    }
    fn open(&self) -> Result<(), RuntimeError> {
        if self.host.is_closed() {
            Err(RuntimeError::new(ErrorKind::Closed, "evaluator is closed"))
        } else {
            Ok(())
        }
    }
    fn entry(&self, index: usize) -> Result<usize, RuntimeError> {
        self.entries
            .get(index)
            .copied()
            .ok_or_else(|| error("source property-path index is out of range"))
    }
    fn release_module(&mut self) {
        self.module = None;
        self.calls.clear();
        self.setup_failure = None;
        self.last_expression = None;
    }
}

fn error(message: &str) -> RuntimeError {
    RuntimeError::new(ErrorKind::Host, message)
}
