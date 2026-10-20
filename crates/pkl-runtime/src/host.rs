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

/// One evaluator, with no filesystem, network, timer, or Java dependency.
pub struct HostSession {
    vm: Vm,
    identity: u64,
    last_clock: u64,
    deadline: Option<u64>,
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
            deadline: None,
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
        let deadline = timeout_ms
            .map(|timeout| {
                now_ms
                    .checked_add(timeout)
                    .ok_or_else(|| error(ErrorKind::Host, "deadline overflow"))
            })
            .transpose()?;
        self.vm.start(entry, inputs)?;
        self.deadline = deadline;
        self.active = true;
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
                self.active = false;
                self.deadline = None;
                HostPoll::Ready(result)
            }
            VmPoll::Pending(request) => HostPoll::Read {
                token: format!("{}:{}", self.identity, request.id),
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
        let (session, request) = token.split_once(':').ok_or_else(stale)?;
        let session: u64 = session.parse().map_err(|_| stale())?;
        let request: u64 = request.parse().map_err(|_| stale())?;
        if session != self.identity || token != format!("{session}:{request}") {
            return Err(stale());
        }
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
        if self.active && self.deadline.is_some_and(|deadline| now >= deadline) {
            self.terminate(ErrorKind::Timeout, "evaluation timed out");
            true
        } else {
            false
        }
    }
    fn terminate(&mut self, kind: ErrorKind, message: &str) {
        if self.terminal.is_none() {
            self.vm.close();
            self.active = false;
            self.deadline = None;
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
