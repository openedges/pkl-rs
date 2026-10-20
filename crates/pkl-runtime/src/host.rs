//! Cooperative Host boundary. Platform adapters supply a monotonic clock and I/O.
use crate::{
    value::{ErrorKind, RuntimeError, Value},
    vm::{Program, Vm, VmPoll},
};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

static NEXT_SESSION: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug, PartialEq)]
pub enum HostPoll {
    Ready(Result<Value, RuntimeError>),
    Read { token: String, uri: Arc<str> },
    Yielded,
}

struct HostContext {
    id: u64,
    deadline: Option<u64>,
}

/// One evaluator, with no filesystem, network, timer, or Java dependency.
pub struct HostSession {
    vm: Vm,
    identity: u64,
    last_clock: u64,
    contexts: Vec<HostContext>,
    next_context: u64,
    cancelled_reads: Vec<String>,
    terminal: Option<RuntimeError>,
    active: bool,
}

impl HostSession {
    pub fn new(program: Arc<Program>) -> Self {
        let identity = NEXT_SESSION
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
            .expect("Host session identity exhausted");
        Self {
            vm: Vm::new(program),
            identity,
            last_clock: 0,
            contexts: Vec::new(),
            next_context: 1,
            cancelled_reads: Vec::new(),
            terminal: None,
            active: false,
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
        self.clock(now_ms)?;
        if self.active {
            return Err(error(ErrorKind::Busy, "evaluation is already active"));
        }
        let context = self.context(now_ms, timeout_ms)?;
        self.vm.start(entry, inputs)?;
        self.contexts.push(context);
        self.active = true;
        Ok(())
    }

    /// Reenter this evaluator from its current reader callback. A completed nested
    /// poll restores the parent's pending read; its token can then be resumed.
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
            VmPoll::Ready(result) => {
                if self.contexts.len() > 1 {
                    self.vm.finish_reentry();
                    self.contexts.pop();
                } else {
                    self.active = false;
                    self.contexts.clear();
                }
                HostPoll::Ready(result)
            }
            VmPoll::Pending(request) => HostPoll::Read {
                token: self.token(self.contexts.last().expect("active context").id, request.id),
                uri: request.uri,
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
        self.vm.collect(external_roots)
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
        Ok(HostContext { id, deadline })
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
                    .map(|(context, request)| self.token(context.id, request))
                    .collect::<Vec<_>>(),
            );
            self.vm.close();
            self.active = false;
            self.contexts.clear();
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
