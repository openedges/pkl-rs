//! Cooperative Host boundary. Platform adapters supply a monotonic clock and I/O.
mod errors;
pub(crate) mod resources;
use crate::{
    logging::{EvaluationContext, LogEvent, LoggerFailure},
    value::{ErrorKind, RuntimeError, Value},
    vm::{PendingRequest, Program, Vm, VmPoll},
};
pub use errors::{IoFailure, ReadFailure};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

static NEXT_SESSION: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug, PartialEq)]
pub enum HostPoll {
    Ready(Result<Value, RuntimeError>),
    Read {
        token: String,
        uri: Arc<str>,
    },
    Log {
        token: String,
        event: Arc<LogEvent>,
    },
    Module {
        token: String,
        request: crate::modules::ModuleRequest,
    },
    Yielded,
}

struct HostContext {
    id: u64,
    deadline: Option<u64>,
    next_entry: Option<usize>,
}

/// One evaluator, with no filesystem, network, timer, or Java dependency.
pub struct HostSession {
    vm: Vm,
    identity: u64,
    last_clock: u64,
    contexts: Vec<HostContext>,
    next_context: u64,
    cancelled_reads: Vec<String>,
    cancelled_modules: Vec<String>,
    terminal: Option<RuntimeError>,
    active: bool,
    intermediate_result: Option<Result<Value, RuntimeError>>,
}

impl HostSession {
    pub fn new(program: Arc<Program>) -> Self {
        Self::with_context(program, EvaluationContext::default())
    }

    pub fn with_context(program: Arc<Program>, context: EvaluationContext) -> Self {
        let identity = NEXT_SESSION
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
            .expect("Host session identity exhausted");
        Self {
            vm: Vm::with_context(program, context),
            identity,
            last_clock: 0,
            contexts: Vec::new(),
            next_context: 1,
            cancelled_reads: Vec::new(),
            cancelled_modules: Vec::new(),
            terminal: None,
            active: false,
            intermediate_result: None,
        }
    }

    pub fn start(
        &mut self,
        entry: usize,
        inputs: Vec<Value>,
        now_ms: u64,
        timeout_ms: Option<u64>,
    ) -> Result<(), RuntimeError> {
        self.open()?;
        self.admit_new()?;
        self.clock(now_ms)?;
        if self.active {
            return Err(error(ErrorKind::Busy, "evaluation is already active"));
        }
        let context = self.context(now_ms, timeout_ms)?;
        self.vm.start(entry, inputs)?;
        self.intermediate_result = None;
        self.contexts.push(context);
        self.active = true;
        Ok(())
    }

    /// Run two entries within one admitted evaluation and deadline. The first
    /// result becomes input zero of the second entry. Callback closing permits
    /// this existing continuation but still rejects new starts and reentries.
    pub fn start_then(
        &mut self,
        first_entry: usize,
        inputs: Vec<Value>,
        next_entry: usize,
        now_ms: u64,
        timeout_ms: Option<u64>,
    ) -> Result<(), RuntimeError> {
        self.start(first_entry, inputs, now_ms, timeout_ms)?;
        self.contexts
            .last_mut()
            .expect("started context")
            .next_entry = Some(next_entry);
        Ok(())
    }

    /// Take the first entry's outcome. A successful transition yields before the
    /// next entry runs, allowing the caller to retain its actual result. Until
    /// consumed, a successful intermediate result is a Host-owned GC root.
    pub fn take_intermediate_result(&mut self) -> Option<Result<Value, RuntimeError>> {
        self.intermediate_result.take()
    }

    /// Replay an already cached initialization failure under normal Host admission.
    /// No guest code, module allocation or logger callback is repeated.
    pub fn start_failed(
        &mut self,
        failure: RuntimeError,
        now_ms: u64,
        timeout_ms: Option<u64>,
    ) -> Result<(), RuntimeError> {
        self.open()?;
        self.admit_new()?;
        self.clock(now_ms)?;
        if self.active {
            return Err(error(ErrorKind::Busy, "evaluation is already active"));
        }
        let context = self.context(now_ms, timeout_ms)?;
        self.vm.start_failed(failure)?;
        self.intermediate_result = None;
        self.contexts.push(context);
        self.active = true;
        Ok(())
    }

    /// Reenter this evaluator from its current Host callback. A completed nested
    /// poll restores the parent's pending read or log and its completion token.
    /// Up to 64 contexts share one VM, heap and successful member cache.
    pub fn reenter(
        &mut self,
        parent_token: &str,
        entry: usize,
        inputs: Vec<Value>,
        now_ms: u64,
        timeout_ms: Option<u64>,
    ) -> Result<(), RuntimeError> {
        self.open()?;
        self.clock(now_ms)?;
        if self.expire(now_ms) {
            return Err(self.terminal.clone().unwrap());
        }
        self.admit_new()?;
        let request = self.request_id(parent_token)?;
        let context = self.context(now_ms, timeout_ms)?;
        self.vm.reenter(request, entry, inputs)?;
        self.contexts.push(context);
        Ok(())
    }

    pub fn poll(&mut self, now_ms: u64, budget: usize) -> HostPoll {
        if let Some(error) = &self.terminal {
            return HostPoll::Ready(Err(error.clone()));
        }
        if let Err(error) = self.clock(now_ms) {
            return HostPoll::Ready(Err(error));
        }
        if self.expire(now_ms) {
            return HostPoll::Ready(Err(self.terminal.clone().unwrap()));
        }
        if budget == 0 {
            return HostPoll::Ready(Err(error(
                ErrorKind::Host,
                "instruction budget must be positive",
            )));
        }
        match self.vm.poll(budget) {
            VmPoll::Ready(mut result) => {
                if let Some(next_entry) = self
                    .contexts
                    .last_mut()
                    .and_then(|context| context.next_entry.take())
                {
                    self.intermediate_result = Some(result.clone());
                    if let Ok(value) = &result {
                        match self.vm.continue_entry(next_entry, vec![value.clone()]) {
                            Ok(()) => return HostPoll::Yielded,
                            Err(error) => result = Err(error),
                        }
                    }
                }
                if self.contexts.len() > 1 {
                    self.vm.finish_reentry();
                    self.contexts.pop();
                } else {
                    self.active = false;
                    self.contexts.clear();
                    if self.vm.is_closing() {
                        self.terminate(ErrorKind::Closed, "evaluator is closed");
                    }
                }
                HostPoll::Ready(result)
            }
            VmPoll::Pending(request) => HostPoll::Read {
                token: self.token(self.contexts.last().expect("active context").id, request.id),
                uri: request.uri,
            },
            VmPoll::Log(request) => HostPoll::Log {
                token: self.token(self.contexts.last().expect("active context").id, request.id),
                event: request.event,
            },
            VmPoll::Module(request) => HostPoll::Module {
                token: self.token(self.contexts.last().expect("active context").id, request.id),
                request,
            },
            VmPoll::Yielded => HostPoll::Yielded,
        }
    }

    pub fn resume(
        &mut self,
        token: &str,
        result: Result<Value, RuntimeError>,
        now_ms: u64,
    ) -> Result<(), RuntimeError> {
        self.open()?;
        self.clock(now_ms)?;
        if self.expire(now_ms) {
            return Err(self.terminal.clone().unwrap());
        }
        let request = self.request_id(token)?;
        self.vm.resume(request, result)
    }

    /// Complete a pending resource read through the shared configured policy.
    pub fn resume_default_read(
        &mut self,
        token: &str,
        properties: &[(String, String)],
        environment: &[(String, String)],
        now_ms: u64,
    ) -> Result<(), RuntimeError> {
        self.open()?;
        self.clock(now_ms)?;
        if self.expire(now_ms) {
            return Err(self.terminal.clone().unwrap());
        }
        let id = self.request_id(token)?;
        let Some(Some(PendingRequest::Read(request))) = self.vm.pending_requests().pop() else {
            return Err(stale());
        };
        if request.id != id {
            return Err(stale());
        }
        let result = resources::configured_result(&request, properties, environment);
        self.vm.resume(id, result)
    }

    /// Complete a resource SPI failure using the actual pending request URI.
    /// This is distinct from injecting an already constructed RuntimeError.
    pub fn reject_read(
        &mut self,
        token: &str,
        failure: ReadFailure,
        now_ms: u64,
    ) -> Result<(), RuntimeError> {
        self.open()?;
        self.clock(now_ms)?;
        if self.expire(now_ms) {
            return Err(self.terminal.clone().unwrap());
        }
        let id = self.request_id(token)?;
        let Some(Some(PendingRequest::Read(request))) = self.vm.pending_requests().pop() else {
            return Err(stale());
        };
        if request.id != id {
            return Err(stale());
        }
        let error = match resources::resolved_uri(&request) {
            Ok(uri) => failure.into_runtime(&uri),
            Err(error) => error,
        };
        self.vm.resume(id, Err(error))
    }

    /// Complete exactly one typed module request owned by the current context.
    pub fn complete_module(
        &mut self,
        token: &str,
        completion: crate::modules::ModuleCompletion,
        now_ms: u64,
    ) -> Result<(), RuntimeError> {
        self.open()?;
        self.clock(now_ms)?;
        if self.expire(now_ms) {
            return Err(self.terminal.clone().unwrap());
        }
        let request = self.request_id(token)?;
        self.vm.complete_module(request, completion)
    }

    /// Abort inventory for module resolution/source I/O, separate from reads.
    pub fn drain_cancelled_modules(&mut self) -> Vec<String> {
        std::mem::take(&mut self.cancelled_modules)
    }

    pub fn acknowledge_log(
        &mut self,
        token: &str,
        completion: Result<(), LoggerFailure>,
        now_ms: u64,
    ) -> Result<(), RuntimeError> {
        self.open()?;
        self.clock(now_ms)?;
        if self.expire(now_ms) {
            return Err(self.terminal.clone().unwrap());
        }
        let request = self.request_id(token)?;
        self.vm.acknowledge_log(request, completion)
    }

    pub fn close_from_callback(&mut self, token: &str) -> Result<(), RuntimeError> {
        self.open()?;
        let request = self.request_id(token)?;
        self.vm.close_from_callback(request)
    }

    /// Public cancellation permanently closes the evaluator, like Java close.
    pub fn cancel(&mut self) {
        self.terminate(ErrorKind::Cancelled, "evaluation cancelled");
    }
    pub fn close(&mut self) {
        self.terminate(ErrorKind::Closed, "evaluator is closed");
    }
    pub fn is_closed(&self) -> bool {
        self.vm.is_closed()
    }
    pub fn set_power_assertions(&mut self, enabled: bool) {
        self.vm.set_power_assertions(enabled);
    }
    pub fn primary_module(&self) -> Option<Value> {
        self.vm.primary_module()
    }
    pub fn expression_string_needs_conversion(&self, value: &Value) -> bool {
        self.vm.expression_string_needs_conversion(value)
    }
    #[cfg(test)]
    pub(crate) fn vm_for_test(&self) -> &Vm {
        &self.vm
    }
    #[cfg(test)]
    pub(crate) fn vm_for_test_mut(&mut self) -> &mut Vm {
        &mut self.vm
    }
    pub fn heap_stats(&self) -> crate::heap::HeapStats {
        self.vm.heap_stats()
    }

    /// Adapters must abort these outstanding operations on their own platform.
    /// Each terminal transition queues every pending read once, ancestors first.
    pub fn drain_cancelled_reads(&mut self) -> Vec<String> {
        std::mem::take(&mut self.cancelled_reads)
    }

    pub fn context_depth(&self) -> usize {
        self.contexts.len()
    }

    /// Externally retained result handles must be supplied as roots by the caller.
    pub fn collect(&mut self, external_roots: &[Value]) -> crate::heap::HeapStats {
        if let Some(Ok(value)) = &self.intermediate_result {
            let mut roots = external_roots.to_vec();
            roots.push(value.clone());
            self.vm.collect(&roots)
        } else {
            self.vm.collect(external_roots)
        }
    }

    fn token(&self, context: u64, request: u64) -> String {
        format!("{}:{context}:{request}", self.identity)
    }

    fn request_id(&self, token: &str) -> Result<u64, RuntimeError> {
        let mut parts = token.split(':');
        let session: u64 = parts
            .next()
            .ok_or_else(stale)?
            .parse()
            .map_err(|_| stale())?;
        let context: u64 = parts
            .next()
            .ok_or_else(stale)?
            .parse()
            .map_err(|_| stale())?;
        let request: u64 = parts
            .next()
            .ok_or_else(stale)?
            .parse()
            .map_err(|_| stale())?;
        if parts.next().is_some()
            || session != self.identity
            || self
                .contexts
                .last()
                .is_none_or(|current| current.id != context)
            || token != self.token(context, request)
        {
            return Err(stale());
        }
        Ok(request)
    }

    fn context(&mut self, now: u64, timeout: Option<u64>) -> Result<HostContext, RuntimeError> {
        let deadline = timeout
            .map(|timeout| {
                now.checked_add(timeout)
                    .ok_or_else(|| error(ErrorKind::Host, "deadline overflow"))
            })
            .transpose()?;
        let id = self.next_context;
        self.next_context = id
            .checked_add(1)
            .ok_or_else(|| error(ErrorKind::Host, "context IDs exhausted"))?;
        Ok(HostContext {
            id,
            deadline,
            next_entry: None,
        })
    }

    fn admit_new(&self) -> Result<(), RuntimeError> {
        if self.vm.is_closing() {
            Err(error(ErrorKind::Closed, "evaluator is closing"))
        } else {
            Ok(())
        }
    }
    fn open(&self) -> Result<(), RuntimeError> {
        if self.vm.is_closed() {
            Err(error(ErrorKind::Closed, "evaluator is closed"))
        } else {
            Ok(())
        }
    }
    fn clock(&mut self, now: u64) -> Result<(), RuntimeError> {
        if now < self.last_clock {
            return Err(error(ErrorKind::Host, "monotonic clock moved backwards"));
        }
        self.last_clock = now;
        Ok(())
    }
    fn expire(&mut self, now: u64) -> bool {
        if self.active
            && self
                .contexts
                .iter()
                .any(|context| context.deadline.is_some_and(|deadline| now >= deadline))
        {
            self.terminate(ErrorKind::Timeout, "evaluation timed out");
            true
        } else {
            false
        }
    }
    fn terminate(&mut self, kind: ErrorKind, message: &str) {
        if self.terminal.is_none() {
            self.cancelled_reads.extend(
                self.contexts
                    .iter()
                    .zip(self.vm.pending_requests())
                    .filter_map(|(context, request)| match request {
                        Some(PendingRequest::Read(request)) => {
                            Some(self.token(context.id, request.id))
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>(),
            );
            self.cancelled_modules.extend(
                self.contexts
                    .iter()
                    .zip(self.vm.pending_requests())
                    .filter_map(|(context, request)| match request {
                        Some(PendingRequest::Module(request)) => {
                            Some(self.token(context.id, request.id))
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>(),
            );
            self.vm.close();
            self.active = false;
            self.contexts.clear();
            if matches!(self.intermediate_result, Some(Ok(_))) {
                self.intermediate_result = None;
            }
            self.terminal = Some(error(kind, message));
        }
    }
}
fn error(kind: ErrorKind, message: &str) -> RuntimeError {
    RuntimeError::new(kind, message)
}
fn stale() -> RuntimeError {
    error(ErrorKind::Host, "stale or unexpected Host completion")
}
