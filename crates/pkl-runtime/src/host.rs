//! Cooperative Host boundary. Platform adapters supply a monotonic clock and I/O.
mod errors;
pub mod http;
pub mod modulepath;
pub(crate) mod resources;
pub mod security;
use crate::{
    logging::{EvaluationContext, LogEvent, LoggerFailure},
    value::{ErrorKind, RuntimeError, Value},
    vm::{PendingRequest, Program, Vm, VmPoll},
};
pub use errors::{IoFailure, ReadFailure};
pub use resources::try_configured_scalar_read_with_policy;
pub use security::SecurityPolicy;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

/// Formats a millisecond timeout duration using Java `MessageFormat` `{0,number,#.##}`
/// semantics (seconds rounded half-even to at most 2 decimal places, no trailing zeros).
pub fn format_timeout_seconds(timeout_ms: u64) -> String {
    // Convert ms to hundredths of a second using exact integer half-even rounding:
    // ms / 1000.0 * 100 = ms / 10.
    let quotient = timeout_ms / 10;
    let remainder = timeout_ms % 10;
    let hundredths = if remainder > 5 || (remainder == 5 && quotient % 2 == 1) {
        quotient + 1
    } else {
        quotient
    };
    let whole = hundredths / 100;
    let frac = hundredths % 100;
    if frac == 0 {
        whole.to_string()
    } else if frac % 10 == 0 {
        format!("{whole}.{}", frac / 10)
    } else {
        format!("{whole}.{frac:02}")
    }
}

/// Formats Pkl 0.32.0's `evaluationTimedOut` message (`errorMessages.properties:807-808`).
pub fn format_timeout_message(timeout_ms: u64) -> String {
    format!(
        "Evaluation timed out after {} second(s).",
        format_timeout_seconds(timeout_ms)
    )
}

/// Resolves an authored module/resource URI against a declaration module URI using the
/// shared lossless UTF-16 URI engine.
pub fn resolve_module_key_uri(
    authored: &crate::value::PklString,
    base_module_uri: &crate::value::PklString,
) -> Result<crate::value::PklString, RuntimeError> {
    resources::resolve_module_key_uri(authored, base_module_uri)
}

/// Resolves a relative or absolute resource read URI against an optional declaration module URI.
pub fn resolve_resource_read_uri(
    authored: &crate::value::PklString,
    base_module_uri: Option<&crate::value::PklString>,
) -> Result<crate::value::PklString, RuntimeError> {
    resources::resolve_authored_resource_uri(authored, base_module_uri)
}

/// Percent-encodes a filesystem path segment sequence following Pkl `IoUtils.toUri` rules.
pub fn quote_uri_path(path: &crate::value::PklString) -> crate::value::PklString {
    resources::quote_path(path)
}

static NEXT_SESSION: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug, PartialEq)]
pub enum HostPoll {
    Ready(Result<Value, RuntimeError>),
    Read {
        token: String,
        uri: crate::value::PklString,
    },
    Log {
        token: String,
        event: Arc<LogEvent>,
    },
    TransformFrame {
        token: String,
        frame: Arc<crate::logging::LogFrame>,
    },
    Module {
        token: String,
        request: crate::modules::ModuleRequest,
    },
    Glob {
        token: String,
        request: crate::glob::GlobRequest,
    },
    Yielded,
}

struct HostContext {
    id: u64,
    timeout_ms: Option<u64>,
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
    cancelled_globs: Vec<String>,
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
            cancelled_globs: Vec::new(),
            terminal: None,
            active: false,
            intermediate_result: None,
        }
    }

    pub(crate) fn with_base_snapshot(context: EvaluationContext) -> Self {
        let identity = NEXT_SESSION
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
            .expect("Host session identity exhausted");
        Self {
            vm: Vm::from_base_snapshot(context),
            identity,
            last_clock: 0,
            contexts: Vec::new(),
            next_context: 1,
            cancelled_reads: Vec::new(),
            cancelled_modules: Vec::new(),
            cancelled_globs: Vec::new(),
            terminal: None,
            active: false,
            intermediate_result: None,
        }
    }

    #[inline]
    pub(crate) fn has_warm_cached_module(&self, uri: &crate::value::PklString) -> bool {
        !self.active && self.open().is_ok() && self.vm.has_warm_cached_module(uri)
    }

    #[inline]
    pub(crate) fn try_pack_warm_expression_columnar(
        &mut self,
        uri: &crate::value::PklString,
        expression: &str,
        out: &mut crate::vm::columnar_value::ColumnarValueBuffer,
    ) -> bool {
        !self.active
            && self.open().is_ok()
            && self
                .vm
                .try_pack_warm_expression_columnar(uri, expression, out)
    }

    pub(crate) fn try_start_warm_cached_public_op(
        &mut self,
        uri: &crate::value::PklString,
        op: crate::vm::WarmPublicOp<'_>,
        now_ms: u64,
        timeout_ms: Option<u64>,
    ) -> Result<bool, RuntimeError> {
        self.open()?;
        self.admit_new()?;
        self.clock(now_ms)?;
        if self.active {
            return Err(error(ErrorKind::Busy, "evaluation is already active"));
        }
        let context = self.context(now_ms, timeout_ms)?;
        if !self.vm.try_start_warm_cached_public_op(uri, op)? {
            return Ok(false);
        }
        self.intermediate_result = None;
        self.contexts.push(context);
        self.active = true;
        Ok(true)
    }

    pub(crate) fn append_public_operation(
        &mut self,
        source: crate::modules::ModuleSourceSpec,
        expression: crate::source::SourceText,
    ) -> Result<usize, RuntimeError> {
        self.open()?;
        if self.active {
            return Err(error(ErrorKind::Busy, "evaluation is already active"));
        }
        self.vm.append_public_operation(source, expression)
    }
    pub(crate) fn append_public_pkl_binary_operation(
        &mut self,
        source: crate::modules::ModuleSourceSpec,
        expression: crate::source::SourceText,
    ) -> Result<usize, RuntimeError> {
        self.open()?;
        if self.active {
            return Err(error(ErrorKind::Busy, "evaluation is already active"));
        }
        self.vm
            .append_public_pkl_binary_operation(source, expression)
    }
    pub(crate) fn append_public_output_bytes_operation(
        &mut self,
        source: crate::modules::ModuleSourceSpec,
    ) -> Result<usize, RuntimeError> {
        self.open()?;
        if self.active {
            return Err(error(ErrorKind::Busy, "evaluation is already active"));
        }
        self.vm.append_public_output_bytes_operation(source)
    }
    pub(crate) fn append_public_load_module_operation(
        &mut self,
        source: crate::modules::ModuleSourceSpec,
    ) -> Result<usize, RuntimeError> {
        self.open()?;
        if self.active {
            return Err(error(ErrorKind::Busy, "evaluation is already active"));
        }
        self.vm.append_public_load_module_operation(source)
    }
    #[allow(dead_code)]
    pub(crate) fn vm(&self) -> &Vm {
        &self.vm
    }
    #[allow(dead_code)]
    pub(crate) fn vm_mut(&mut self) -> &mut Vm {
        &mut self.vm
    }
    pub(crate) fn ensure_base_full_metadata(&mut self) -> Result<(), RuntimeError> {
        self.vm.ensure_base_full_metadata()
    }
    pub(crate) fn reenter_public_operation(
        &mut self,
        token: &str,
        source: crate::modules::ModuleSourceSpec,
        expression: crate::source::SourceText,
        now: u64,
        timeout: Option<u64>,
    ) -> Result<(), RuntimeError> {
        self.open()?;
        self.clock(now)?;
        if self.expire(now) {
            return Err(self.terminal.clone().unwrap());
        }
        self.admit_new()?;
        let request = self.request_id(token)?;
        self.vm.validate_pending_request(request)?;
        if self.contexts.len() >= 64 {
            return Err(error(ErrorKind::Busy, "experiment context limit exceeded"));
        }
        let entry = self.vm.append_public_operation(source, expression)?;
        self.reenter(token, entry, Vec::new(), now, timeout)
    }
    pub(crate) fn public_primary_source(&self) -> Option<Arc<crate::value::RuntimeSource>> {
        self.vm.public_primary_source()
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
            VmPoll::TransformFrame(request) => HostPoll::TransformFrame {
                token: self.token(self.contexts.last().expect("active context").id, request.id),
                frame: request.frame,
            },
            VmPoll::Log(request) => HostPoll::Log {
                token: self.token(self.contexts.last().expect("active context").id, request.id),
                event: request.event,
            },
            VmPoll::Module(request) => HostPoll::Module {
                token: self.token(self.contexts.last().expect("active context").id, request.id),
                request,
            },
            VmPoll::Glob(request) => HostPoll::Glob {
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
        let result = resources::configured_read(&request, properties, environment);
        self.vm.complete_read(id, result)
    }

    /// Complete a typed ResourceReader result. Absence and successful values
    /// enter the evaluator cache; raw `resume` callbacks retain their contract.
    pub fn complete_read(
        &mut self,
        token: &str,
        result: Result<crate::resources::ReadResult, ReadFailure>,
        now_ms: u64,
    ) -> Result<(), RuntimeError> {
        self.open()?;
        self.clock(now_ms)?;
        if self.expire(now_ms) {
            return Err(self.terminal.clone().unwrap());
        }
        let request = self.read_request(token)?;
        let result = result.map_err(|error| match resources::resolved_uri(&request) {
            Ok(uri) => error.into_runtime(&uri),
            Err(error) => error,
        });
        self.vm.complete_read(request.id, result)
    }

    pub fn read_request(&self, token: &str) -> Result<crate::vm::ReadRequest, RuntimeError> {
        self.open()?;
        let id = self.request_id(token)?;
        match self.vm.pending_requests().pop() {
            Some(Some(PendingRequest::Read(request))) if request.id == id => Ok(request),
            _ => Err(stale()),
        }
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

    /// Explicit org.pkl.testMode counterpart for glob enumeration limits.
    /// Production mode remains the default; no environment or case-name lookup.
    pub fn set_glob_test_mode(&mut self, enabled: bool) -> Result<(), RuntimeError> {
        self.open()?;
        if self.active {
            return Err(error(ErrorKind::Busy, "evaluation is already active"));
        }
        self.vm.set_glob_test_mode(enabled)
    }

    pub fn glob_request(&self, token: &str) -> Result<crate::glob::GlobRequest, RuntimeError> {
        self.open()?;
        let id = self.request_id(token)?;
        match self.vm.pending_requests().pop() {
            Some(Some(PendingRequest::Glob(request))) if request.id == id => Ok(request),
            _ => Err(stale()),
        }
    }

    /// Typed completion for the current context only. VM admission validates
    /// request kind, reader identity and failure phase before consuming state.
    pub fn complete_glob(
        &mut self,
        token: &str,
        completion: crate::glob::GlobCompletion,
        now_ms: u64,
    ) -> Result<(), RuntimeError> {
        self.open()?;
        self.clock(now_ms)?;
        if self.expire(now_ms) {
            return Err(self.terminal.clone().unwrap());
        }
        let request = self.request_id(token)?;
        self.vm.complete_glob(request, completion)
    }

    /// Delegate only to the explicit configured profile. Admission, clock and
    /// deadline checks run even when that profile cannot complete the operation.
    pub fn complete_default_glob(
        &mut self,
        token: &str,
        properties: &[(String, String)],
        environment: &[(String, String)],
        now_ms: u64,
    ) -> Result<bool, RuntimeError> {
        self.open()?;
        self.clock(now_ms)?;
        if self.expire(now_ms) {
            return Err(self.terminal.clone().unwrap());
        }
        let request = self.glob_request(token)?;
        let Some(completion) =
            crate::glob::configured_completion(&request, properties, environment)
        else {
            return Ok(false);
        };
        self.vm.complete_glob(request.id, completion)?;
        Ok(true)
    }

    pub fn drain_cancelled_globs(&mut self) -> Vec<String> {
        std::mem::take(&mut self.cancelled_globs)
    }

    pub fn module_request(
        &self,
        token: &str,
    ) -> Result<crate::modules::ModuleRequest, RuntimeError> {
        self.open()?;
        let id = self.request_id(token)?;
        match self.vm.pending_requests().pop() {
            Some(Some(PendingRequest::Module(request))) if request.id == id => Ok(request),
            _ => Err(stale()),
        }
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

    /// Explicit default policy delegation. An unsupported request returns false
    /// without consuming it; stale requests and terminal deadlines still fail.
    pub fn complete_default_module(
        &mut self,
        token: &str,
        now_ms: u64,
    ) -> Result<bool, RuntimeError> {
        self.open()?;
        self.clock(now_ms)?;
        if self.expire(now_ms) {
            return Err(self.terminal.clone().unwrap());
        }
        let request = self.module_request(token)?;
        let Some(completion) = crate::modules::default_completion(&request) else {
            return Ok(false);
        };
        self.vm.complete_module(request.id, completion)?;
        Ok(true)
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

    pub fn complete_frame_transform(
        &mut self,
        token: &str,
        completion: Result<crate::logging::LogFrame, LoggerFailure>,
        now_ms: u64,
    ) -> Result<(), RuntimeError> {
        self.open()?;
        self.clock(now_ms)?;
        if self.expire(now_ms) {
            return Err(self.terminal.clone().unwrap());
        }
        let request = self.request_id(token)?;
        self.vm.complete_frame_transform(request, completion)
    }

    pub fn set_frame_transformer(
        &mut self,
        transformer: crate::logging::FrameTransformer,
    ) -> Result<(), RuntimeError> {
        self.open()?;
        if self.active {
            return Err(error(ErrorKind::Busy, "evaluation is already active"));
        }
        self.vm.set_frame_transformer(transformer)
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
    pub fn begin_fact_trace(
        &mut self,
        module_uri: &crate::value::PklString,
        group_name: &crate::value::PklString,
        fact_span: crate::source::Span,
    ) -> bool {
        self.vm.begin_fact_trace(module_uri, group_name, fact_span)
    }
    pub fn finish_fact_trace(&mut self) -> Vec<crate::diagnostic::PowerAssertionValue> {
        self.vm.finish_fact_trace()
    }
    pub fn set_platform_metadata(&mut self, metadata: Option<crate::logging::PlatformMetadata>) {
        self.vm.set_platform_metadata(metadata);
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

    pub(crate) fn enable_auto_gc(&mut self) {
        self.vm.enable_auto_gc();
    }

    pub(crate) fn set_session_root(&mut self, root: Option<Value>) {
        self.vm.set_session_root(root);
    }

    pub(crate) fn maybe_auto_gc_between_operations(&mut self) {
        if !self.active && self.contexts.is_empty() && self.intermediate_result.is_none() {
            self.vm.maybe_auto_gc_between_operations();
        }
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

    pub(crate) fn set_pure_standard_host(&mut self, enabled: bool) {
        self.vm.set_pure_standard_host(enabled);
    }

    pub(crate) fn public_primary_id(&self) -> Option<crate::value::ObjectId> {
        self.vm.public_primary_id()
    }

    fn token(&self, context: u64, request: u64) -> String {
        fn write_u64(buf: &mut [u8; 64], pos: &mut usize, mut v: u64) {
            if v == 0 {
                *pos -= 1;
                buf[*pos] = b'0';
                return;
            }
            while v > 0 {
                *pos -= 1;
                buf[*pos] = b'0' + (v % 10) as u8;
                v /= 10;
            }
        }
        let mut buf = [0u8; 64];
        let mut pos = buf.len();
        write_u64(&mut buf, &mut pos, request);
        pos -= 1;
        buf[pos] = b':';
        write_u64(&mut buf, &mut pos, context);
        pos -= 1;
        buf[pos] = b':';
        write_u64(&mut buf, &mut pos, self.identity);
        std::str::from_utf8(&buf[pos..])
            .expect("ASCII token")
            .to_owned()
    }

    fn request_id(&self, token: &str) -> Result<u64, RuntimeError> {
        let mut parts = token.split(':');
        let session = parse_canonical_u64(parts.next().ok_or_else(stale)?).ok_or_else(stale)?;
        let context = parse_canonical_u64(parts.next().ok_or_else(stale)?).ok_or_else(stale)?;
        let request = parse_canonical_u64(parts.next().ok_or_else(stale)?).ok_or_else(stale)?;
        if parts.next().is_some()
            || session != self.identity
            || self
                .contexts
                .last()
                .is_none_or(|current| current.id != context)
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
            timeout_ms: timeout,
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
    pub fn last_clock(&self) -> u64 {
        self.last_clock
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
            && let Some(expired) = self
                .contexts
                .iter()
                .find(|context| context.deadline.is_some_and(|deadline| now >= deadline))
        {
            let message = match expired.timeout_ms {
                Some(ms) => format_timeout_message(ms),
                None => "Evaluation timed out.".to_string(),
            };
            self.terminate(ErrorKind::Timeout, &message);
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
            self.cancelled_globs.extend(
                self.contexts
                    .iter()
                    .zip(self.vm.pending_requests())
                    .filter_map(|(context, request)| match request {
                        Some(PendingRequest::Glob(request)) => {
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
fn parse_canonical_u64(s: &str) -> Option<u64> {
    let bytes = s.as_bytes();
    if bytes.is_empty() || (bytes.len() > 1 && bytes[0] == b'0') {
        return None;
    }
    let mut acc = 0u64;
    for &b in bytes {
        if !b.is_ascii_digit() {
            return None;
        }
        acc = acc.checked_mul(10)?.checked_add(u64::from(b - b'0'))?;
    }
    Some(acc)
}
fn error(kind: ErrorKind, message: &str) -> RuntimeError {
    RuntimeError::new(kind, message)
}
fn stale() -> RuntimeError {
    error(ErrorKind::Host, "stale or unexpected Host completion")
}
