//! Bidirectional streaming `pkl server` loop, FIFO single-thread worker, mutex-serialized
//! MessagePack writer, and deadlock-free client callback transport.

use crate::ServerError;
use crate::evaluator::{
    CreateEvaluatorFailure, CreateEvaluatorParams, OutboundSink, ServerEvaluator,
};
use pkl_runtime::external_reader::{
    ExternalReaderError, ExternalReaderMessage, ExternalReaderTransport,
};
use pkl_runtime::server::{ServerMessage, codec};
use std::collections::HashMap;
use std::ffi::OsStr;
use std::io::{BufReader, BufWriter, Read, Write};
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

fn debug_enabled() -> bool {
    std::env::var_os("PKL_DEBUG").as_deref() == Some(OsStr::new("1"))
}

fn debug_log(args: std::fmt::Arguments<'_>) {
    eprintln!("[pkl-server] {args}");
}

// ============================================================================
// Serialized Outbound MessagePack Writer (matches AbstractMessagePackEncoder)
// ============================================================================

struct StreamWriter<W: Write + Send + 'static> {
    inner: Mutex<Option<BufWriter<W>>>,
    closed: AtomicBool,
}

impl<W: Write + Send + 'static> StreamWriter<W> {
    fn new(writer: W) -> Self {
        Self {
            inner: Mutex::new(Some(BufWriter::new(writer))),
            closed: AtomicBool::new(false),
        }
    }

    fn mark_closed(&self) {
        self.closed.store(true, Ordering::SeqCst);
    }

    fn close(&self) {
        self.closed.store(true, Ordering::SeqCst);
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(mut w) = guard.take() {
            let _ = w.flush();
        }
    }
}

impl<W: Write + Send + 'static> OutboundSink for StreamWriter<W> {
    fn send(&self, msg: &ServerMessage) -> Result<(), ExternalReaderError> {
        if self.closed.load(Ordering::SeqCst) {
            return Err(ExternalReaderError::Closed);
        }
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let Some(writer) = guard.as_mut() else {
            return Err(ExternalReaderError::Closed);
        };
        if debug_enabled() {
            debug_log(format_args!("Sending message: {msg:?}"));
        }
        codec::write_message(writer, msg)
    }
}

// ============================================================================
// Pending Client Callback Slots (shared between reader thread & worker thread)
// ============================================================================

type CallbackResult = Result<ExternalReaderMessage, ExternalReaderError>;

struct PendingSlot {
    state: Mutex<Option<CallbackResult>>,
    ready: Condvar,
}

impl PendingSlot {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(None),
            ready: Condvar::new(),
        })
    }

    fn complete(&self, result: CallbackResult) {
        let mut guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if guard.is_none() {
            *guard = Some(result);
            self.ready.notify_all();
        }
    }

    fn wait(&self) -> CallbackResult {
        let mut guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
        loop {
            if let Some(res) = guard.take() {
                return res;
            }
            if pkl_host_native::is_current_thread_eval_cancelled() {
                return Err(ExternalReaderError::Cancelled);
            }
            let (next_guard, _) = self
                .ready
                .wait_timeout(guard, std::time::Duration::from_millis(50))
                .unwrap_or_else(|e| e.into_inner());
            guard = next_guard;
        }
    }
}

const MAX_RECENTLY_CANCELLED_IDS: usize = 4096;

struct PendingCallbacks {
    slots: Mutex<HashMap<i64, (i64, Arc<PendingSlot>)>>,
    /// Bounded FIFO + set of recently cancelled/closed request IDs so late
    /// in-flight client responses arriving after `CloseEvaluator` or eval
    /// timeout are discarded cleanly instead of crashing the entire server.
    cancelled: Mutex<(
        std::collections::VecDeque<i64>,
        std::collections::HashSet<i64>,
    )>,
    closed: AtomicBool,
}

impl PendingCallbacks {
    fn new() -> Self {
        Self {
            slots: Mutex::new(HashMap::new()),
            cancelled: Mutex::new((
                std::collections::VecDeque::new(),
                std::collections::HashSet::new(),
            )),
            closed: AtomicBool::new(false),
        }
    }

    fn record_cancelled(&self, request_id: i64) {
        let mut guard = self.cancelled.lock().unwrap_or_else(|e| e.into_inner());
        let (queue, set) = &mut *guard;
        if set.insert(request_id) {
            queue.push_back(request_id);
            while set.len() > MAX_RECENTLY_CANCELLED_IDS {
                while let Some(&front) = queue.front() {
                    if set.contains(&front) {
                        break;
                    }
                    queue.pop_front();
                }
                if let Some(evicted) = queue.pop_front() {
                    set.remove(&evicted);
                } else {
                    break;
                }
            }
            if queue.len() > MAX_RECENTLY_CANCELLED_IDS * 2 {
                queue.retain(|id| set.contains(id));
            }
        }
    }

    fn take_cancelled(&self, request_id: i64) -> bool {
        let mut guard = self.cancelled.lock().unwrap_or_else(|e| e.into_inner());
        guard.1.remove(&request_id)
    }

    fn register(
        &self,
        request_id: i64,
        evaluator_id: i64,
    ) -> Result<Arc<PendingSlot>, ExternalReaderError> {
        let mut map = self.slots.lock().unwrap_or_else(|e| e.into_inner());
        if self.closed.load(Ordering::SeqCst) {
            return Err(ExternalReaderError::Closed);
        }
        if map.contains_key(&request_id) {
            return Err(ExternalReaderError::Protocol(format!(
                "Duplicate in-flight request ID `{request_id}`."
            )));
        }
        let slot = PendingSlot::new();
        map.insert(request_id, (evaluator_id, Arc::clone(&slot)));
        Ok(slot)
    }

    fn remove(&self, request_id: i64) {
        self.slots
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&request_id);
    }

    fn cancel_request(&self, request_id: i64) {
        let removed = self
            .slots
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&request_id)
            .is_some();
        if removed {
            self.record_cancelled(request_id);
        }
    }

    fn complete_response(&self, msg: ServerMessage) -> Result<(), ExternalReaderError> {
        let request_id = msg.request_id().unwrap_or(0);
        let response_evaluator_id = msg.evaluator_id();
        let simple_name = msg.java_simple_name();
        let entry = {
            let mut map = self.slots.lock().unwrap_or_else(|e| e.into_inner());
            match map.get(&request_id) {
                Some(&(expected_eval_id, _))
                    if response_evaluator_id.is_some_and(|got| got != expected_eval_id) =>
                {
                    return Err(ExternalReaderError::Protocol(format!(
                        "Mismatched evaluatorId in `{simple_name}` for requestId `{request_id}`: expected `{expected_eval_id}`, got `{}`.",
                        response_evaluator_id.unwrap_or_default()
                    )));
                }
                _ => map.remove(&request_id).map(|(_, slot)| slot),
            }
        };
        let Some(slot) = entry else {
            if self.take_cancelled(request_id) {
                if debug_enabled() {
                    debug_log(format_args!(
                        "Discarding late `{simple_name}` for cancelled/closed request ID `{request_id}`."
                    ));
                }
                return Ok(());
            }
            return Err(ExternalReaderError::unknown_request_id(
                simple_name,
                request_id,
            ));
        };
        let ext_msg = msg.try_into_external_reader()?;
        slot.complete(Ok(ext_msg));
        Ok(())
    }

    fn cancel_evaluator(&self, evaluator_id: i64, err: ExternalReaderError) {
        let (ids, removed): (Vec<i64>, Vec<Arc<PendingSlot>>) = {
            let mut map = self.slots.lock().unwrap_or_else(|e| e.into_inner());
            let ids: Vec<i64> = map
                .iter()
                .filter_map(|(&req_id, &(ev_id, _))| (ev_id == evaluator_id).then_some(req_id))
                .collect();
            let slots = ids
                .iter()
                .filter_map(|id| map.remove(id).map(|(_, slot)| slot))
                .collect();
            (ids, slots)
        };
        for id in ids {
            self.record_cancelled(id);
        }
        for slot in removed {
            slot.complete(Err(err.clone()));
        }
    }

    fn fail_all(&self, err: ExternalReaderError) {
        self.closed.store(true, Ordering::SeqCst);
        let all: Vec<Arc<PendingSlot>> = {
            let mut map = self.slots.lock().unwrap_or_else(|e| e.into_inner());
            map.drain().map(|(_, (_, slot))| slot).collect()
        };
        for slot in all {
            slot.complete(Err(err.clone()));
        }
    }
}

/// Per-evaluator `ExternalReaderTransport` forwarding `0x26`/`0x28`/`0x2a`/`0x2c`
/// requests to the connected `pkl server` client and awaiting correlated responses.
struct ServerClientTransport {
    evaluator_id: i64,
    outbound: Arc<dyn OutboundSink>,
    pending: Arc<PendingCallbacks>,
    closed: AtomicBool,
}

impl ExternalReaderTransport for ServerClientTransport {
    fn request(
        &self,
        message: ExternalReaderMessage,
    ) -> Result<ExternalReaderMessage, ExternalReaderError> {
        if self.closed.load(Ordering::SeqCst) {
            return Err(ExternalReaderError::Closed);
        }
        if pkl_host_native::is_current_thread_eval_cancelled() {
            return Err(ExternalReaderError::Cancelled);
        }
        let request_id = message.request_id().ok_or_else(|| {
            ExternalReaderError::Protocol("Expected request message with requestId".into())
        })?;
        let server_msg = ServerMessage::try_from_external_reader(message)?;
        let slot = self.pending.register(request_id, self.evaluator_id)?;
        if let Err(err) = self.outbound.send(&server_msg) {
            self.pending.remove(request_id);
            return Err(err);
        }
        let res = slot.wait();
        if matches!(res, Err(ExternalReaderError::Cancelled)) {
            self.pending.cancel_request(request_id);
        }
        res
    }

    fn send_one_way(&self, message: ExternalReaderMessage) -> Result<(), ExternalReaderError> {
        let server_msg = ServerMessage::try_from_external_reader(message)?;
        self.outbound.send(&server_msg)
    }

    fn close(&self) {
        if self
            .closed
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            self.pending
                .cancel_evaluator(self.evaluator_id, ExternalReaderError::Closed);
        }
    }

    fn is_closed(&self) -> bool {
        self.closed.load(Ordering::SeqCst)
    }

    fn cancel(&self) {
        self.pending
            .cancel_evaluator(self.evaluator_id, ExternalReaderError::Cancelled);
    }
}

// ============================================================================
// Worker Queue & Non-Colliding `i64` Evaluator ID Generator
// ============================================================================

struct EvalJob {
    request_id: i64,
    evaluator: Arc<ServerEvaluator>,
    module_uri: String,
    module_text: Option<String>,
    expr: Option<String>,
}

struct EvaluatorIdGenerator {
    state: AtomicU64,
}

impl EvaluatorIdGenerator {
    fn new() -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x9e37_79b9_7f4a_7c15);
        let pid = u64::from(std::process::id());
        let seed = nanos ^ pid.rotate_left(17) ^ 0xa076_1d64_78bd_642f;
        Self {
            state: AtomicU64::new(if seed == 0 { 1 } else { seed }),
        }
    }

    fn next_id(&self) -> i64 {
        // SplitMix64 step producing well-distributed non-zero i64 IDs.
        let prev = self
            .state
            .fetch_add(0x9e37_79b9_7f4a_7c15, Ordering::Relaxed);
        let mut z = prev.wrapping_add(0x9e37_79b9_7f4a_7c15);
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^= z >> 31;
        let id = z as i64;
        if id == 0 { 1 } else { id }
    }
}

// ============================================================================
// Main Streaming Server Loop
// ============================================================================

/// Runs a complete `pkl server` session over arbitrary `Read` / `Write` streams.
pub fn run_stream_server<R: Read + Send + 'static, W: Write + Send + 'static>(
    input: R,
    output: W,
) -> Result<(), ServerError> {
    let writer = Arc::new(StreamWriter::new(output));
    let outbound: Arc<dyn OutboundSink> = Arc::clone(&writer) as Arc<dyn OutboundSink>;
    let pending = Arc::new(PendingCallbacks::new());
    let id_gen = EvaluatorIdGenerator::new();
    let request_id_counter = Arc::new(AtomicI64::new(1));

    let (tx, rx) = mpsc::channel::<EvalJob>();
    let worker_outbound = Arc::clone(&outbound);
    let worker = thread::Builder::new()
        .name("pkl-server-worker".into())
        .spawn(move || {
            for job in rx {
                let evaluator_id = job.evaluator.evaluator_id();
                let resp = match job
                    .evaluator
                    .evaluate(job.module_uri, job.module_text, job.expr)
                {
                    Ok(bytes) => ServerMessage::EvaluateResponse {
                        request_id: job.request_id,
                        evaluator_id,
                        result: Some(bytes),
                        error: None,
                    },
                    Err(err_msg) => ServerMessage::EvaluateResponse {
                        request_id: job.request_id,
                        evaluator_id,
                        result: None,
                        error: Some(err_msg),
                    },
                };
                if worker_outbound.send(&resp).is_err() {
                    break;
                }
            }
        })
        .map_err(|e| ServerError::new(format!("Failed to spawn server worker thread: {e}")))?;

    let (close_tx, close_rx) = mpsc::channel::<Arc<ServerEvaluator>>();
    let closer = thread::Builder::new()
        .name("pkl-server-closer".into())
        .spawn(move || {
            for evaluator in close_rx {
                evaluator.finish_close();
            }
        })
        .map_err(|e| ServerError::new(format!("Failed to spawn server closer thread: {e}")))?;

    let mut evaluators: HashMap<i64, Arc<ServerEvaluator>> = HashMap::new();
    let mut reader = BufReader::new(input);
    let mut loop_error: Option<ServerError> = None;

    loop {
        let msg = match codec::read_message(&mut reader) {
            Ok(Some(msg)) => msg,
            Ok(None) => break, // Clean EOF on stdin.
            Err(ExternalReaderError::Decode { message, .. }) => {
                loop_error = Some(ServerError::new(message));
                break;
            }
            Err(ExternalReaderError::Protocol(message)) => {
                loop_error = Some(ServerError::new(message));
                break;
            }
            Err(err) => {
                loop_error = Some(ServerError::new(err.to_string()));
                break;
            }
        };

        if debug_enabled() {
            debug_log(format_args!("Received message: {msg:?}"));
        }

        match msg {
            ServerMessage::CreateEvaluatorRequest {
                request_id,
                allowed_modules,
                allowed_resources,
                client_module_readers,
                client_resource_readers,
                module_paths,
                env,
                properties,
                timeout_seconds,
                root_dir,
                cache_dir,
                output_format,
                project,
                http,
                external_module_readers,
                external_resource_readers,
                trace_mode,
            } => {
                let mut evaluator_id = id_gen.next_id();
                while evaluators.contains_key(&evaluator_id) {
                    evaluator_id = id_gen.next_id();
                }
                let client_transport: Arc<dyn ExternalReaderTransport> =
                    Arc::new(ServerClientTransport {
                        evaluator_id,
                        outbound: Arc::clone(&outbound),
                        pending: Arc::clone(&pending),
                        closed: AtomicBool::new(false),
                    });
                let params = CreateEvaluatorParams {
                    allowed_modules,
                    allowed_resources,
                    client_module_readers,
                    client_resource_readers,
                    module_paths,
                    env,
                    properties,
                    timeout_seconds,
                    root_dir,
                    cache_dir,
                    output_format,
                    project,
                    http,
                    external_module_readers,
                    external_resource_readers,
                    trace_mode,
                };
                match ServerEvaluator::create(
                    evaluator_id,
                    params,
                    Arc::clone(&outbound),
                    client_transport,
                    Arc::clone(&request_id_counter),
                ) {
                    Ok(evaluator) => {
                        evaluators.insert(evaluator_id, Arc::new(evaluator));
                        let resp = ServerMessage::CreateEvaluatorResponse {
                            request_id,
                            evaluator_id: Some(evaluator_id),
                            error: None,
                        };
                        if let Err(err) = outbound.send(&resp) {
                            loop_error = Some(ServerError::new(err.to_string()));
                            break;
                        }
                    }
                    Err(CreateEvaluatorFailure::Protocol(err_msg)) => {
                        let resp = ServerMessage::CreateEvaluatorResponse {
                            request_id,
                            evaluator_id: None,
                            error: Some(err_msg),
                        };
                        if let Err(err) = outbound.send(&resp) {
                            loop_error = Some(ServerError::new(err.to_string()));
                            break;
                        }
                    }
                    Err(CreateEvaluatorFailure::Fatal(err_msg)) => {
                        loop_error = Some(ServerError::new(err_msg));
                        break;
                    }
                }
            }
            ServerMessage::CloseEvaluator { evaluator_id } => {
                if let Some(evaluator) = evaluators.remove(&evaluator_id) {
                    evaluator.begin_close();
                    let _ = close_tx.send(evaluator);
                } else if debug_enabled() {
                    debug_log(format_args!(
                        "Ignoring close request for unknown evaluator ID `{evaluator_id}`."
                    ));
                }
            }
            ServerMessage::EvaluateRequest {
                request_id,
                evaluator_id,
                module_uri,
                module_text,
                expr,
            } => {
                let Some(evaluator) = evaluators.get(&evaluator_id).cloned() else {
                    let resp = ServerMessage::EvaluateResponse {
                        request_id,
                        evaluator_id,
                        result: None,
                        error: Some(format!("Evaluator with ID {evaluator_id} was not found.")),
                    };
                    if let Err(err) = outbound.send(&resp) {
                        loop_error = Some(ServerError::new(err.to_string()));
                        break;
                    }
                    continue;
                };
                let job = EvalJob {
                    request_id,
                    evaluator,
                    module_uri,
                    module_text,
                    expr,
                };
                if tx.send(job).is_err() {
                    loop_error = Some(ServerError::new(
                        "Server worker thread exited unexpectedly.",
                    ));
                    break;
                }
            }
            // Client callback responses (`0x27`, `0x29`, `0x2b`, `0x2d`).
            ServerMessage::ReadResourceResponse { .. }
            | ServerMessage::ReadModuleResponse { .. }
            | ServerMessage::ListResourcesResponse { .. }
            | ServerMessage::ListModulesResponse { .. } => {
                if let Err(err) = pending.complete_response(msg) {
                    loop_error = Some(ServerError::new(err.to_string()));
                    break;
                }
            }
            // Unexpected messages received by server (`Server.kt:71-81`).
            other if other.is_one_way() => {
                loop_error = Some(ServerError::new(format!(
                    "Unexpected incoming one-way message: {}",
                    other.java_simple_name()
                )));
                break;
            }
            other if other.is_request() => {
                loop_error = Some(ServerError::new(format!(
                    "Unexpected incoming request message: {}",
                    other.java_simple_name()
                )));
                break;
            }
            other => {
                // Unexpected response (e.g. CreateEvaluatorResponse / EvaluateResponse sent by client).
                let req_id = other.request_id().unwrap_or(0);
                let err = ExternalReaderError::unknown_request_id(other.java_simple_name(), req_id);
                loop_error = Some(ServerError::new(err.to_string()));
                break;
            }
        }
    }

    // Shutdown sequence matching `Server.close()` (`Server.kt:90-97`):
    // 1. Fail any pending client callbacks so blocked evaluations unblock immediately.
    // 2. Drop the job queue sender so no further jobs wait after in-flight work completes.
    // 3. On error, mark writer closed and abort all evaluators before joining worker/closer
    //    so LogMessage pipe backpressure or blocking external readers cannot deadlock teardown.
    pending.fail_all(ExternalReaderError::Closed);
    drop(tx);
    if loop_error.is_some() {
        writer.mark_closed();
        for (_, evaluator) in evaluators.drain() {
            evaluator.abort_immediately();
            let _ = close_tx.send(evaluator);
        }
        drop(close_tx);
        let _ = worker.join();
        let _ = closer.join();
        writer.close();
    } else {
        // Wait for any already-queued non-callback evaluations to flush responses before closing.
        let _ = worker.join();
        for (_, evaluator) in evaluators.drain() {
            evaluator.begin_close();
            let _ = close_tx.send(evaluator);
        }
        drop(close_tx);
        let _ = closer.join();
        writer.close();
    }

    match loop_error {
        Some(err) => Err(err),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::{MAX_RECENTLY_CANCELLED_IDS, PendingCallbacks};

    #[test]
    fn round5_pending_callbacks_cancelled_eviction_preserves_live_ids() {
        let pending = PendingCallbacks::new();
        for id in 1..=(MAX_RECENTLY_CANCELLED_IDS as i64) {
            pending.record_cancelled(id);
        }
        // Consume all but the last ID; previously stale entries remained in the FIFO queue
        // and caused the live ID to be evicted prematurely on subsequent cancellations.
        for id in 1..(MAX_RECENTLY_CANCELLED_IDS as i64) {
            assert!(pending.take_cancelled(id));
        }
        let survivor = MAX_RECENTLY_CANCELLED_IDS as i64;
        for id in 100_000..100_100 {
            pending.record_cancelled(id);
        }
        assert!(
            pending.take_cancelled(survivor),
            "live cancelled ID {survivor} must not be evicted by consumed queue entries"
        );
    }
}
