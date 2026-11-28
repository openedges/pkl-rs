//! Non-blocking stateful `NativeReplSession` backed by `pkl_runtime::repl::ReplSession`
//! and `pkl_host_native::NativeHost`.

use crate::bridge::{
    BridgeCallbackResponseIn, BridgeRequestJs, BridgeResponse, CapturingHostLogger, JsBridge,
    JsFrameTransformerAdapter,
};
use crate::error::{NativeError, parse_u64_bigint, to_js_outcome, utf16_to_pkl, utf16_to_string};
use crate::product::{
    ActiveCallCancelSlot, NativeEvaluatorOptions, NativeProject, SafeTimeoutWatchdog,
    build_native_host, path_to_dir_uri,
};
use napi::{
    Env, Result, ScopedTask,
    bindgen_prelude::{Array, AsyncTask, BigInt, Function, Null, Object, Utf16String},
    threadsafe_function::UnknownReturnValue,
};
use napi_derive::napi;
use pkl_host_native::{CancelFlag, NativeHost, ScopedEvalCancel};
use pkl_runtime::{
    logging::{FrameTransformer, LogDelivery},
    repl::{ReplPoll, ReplRequest, ReplResponse, ReplSession, ReplSessionOptions},
};
use std::path::PathBuf;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
};

struct ReplInner {
    session: ReplSession,
    next_request_id: usize,
    closed: bool,
}

#[napi]
pub struct NativeReplSession {
    host: Arc<NativeHost>,
    inner: Arc<Mutex<ReplInner>>,
    bridge: Option<Arc<JsBridge>>,
    logger: Arc<CapturingHostLogger>,
    frame_transformer: Option<Arc<JsFrameTransformerAdapter>>,
    test_mode: bool,
    closed_flag: Arc<AtomicBool>,
    cancel_generation: Arc<AtomicU64>,
    active_call_cancel: ActiveCallCancelSlot,
    default_timeout_ms: Option<u64>,
}

#[napi]
impl NativeReplSession {
    #[napi]
    pub fn create<'env>(
        env: &'env Env,
        options: Option<NativeEvaluatorOptions>,
        project: Option<&NativeProject>,
        bridge_cb: Option<Function<'env, BridgeRequestJs, UnknownReturnValue>>,
    ) -> Result<Object<'env>> {
        let opts = options.unwrap_or_default();
        let working_dir = match &opts.working_dir {
            Some(dir) => match utf16_to_string(dir) {
                Ok(s) => PathBuf::from(s),
                Err(err) => return to_js_outcome::<Null>(env, Err(err)),
            },
            None => std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        };
        let built = match build_native_host(opts, project, bridge_cb) {
            Ok(b) => b,
            Err(err) => return to_js_outcome::<Null>(env, Err(err)),
        };

        let working_dir_uri = path_to_dir_uri(&working_dir);
        let mut session = ReplSession::new(ReplSessionOptions {
            working_dir_uri,
            color: built.use_color,
            trace_mode: built.trace_mode,
            log_delivery: LogDelivery::Host,
            frame_transformer: if built.frame_transformer.is_some() || built.test_mode {
                FrameTransformer::Host
            } else {
                FrameTransformer::Default
            },
            power_assertions: built.power_assertions,
            platform_data: true,
        });
        session.set_reusable_timeouts(true);
        session.set_platform_metadata(Some(pkl_host_native::detect_native_platform_metadata()));

        let repl = Self {
            host: built.host,
            inner: Arc::new(Mutex::new(ReplInner {
                session,
                next_request_id: 0,
                closed: false,
            })),
            bridge: built.bridge,
            logger: built.logger,
            frame_transformer: built.frame_transformer,
            test_mode: built.test_mode,
            closed_flag: Arc::new(AtomicBool::new(false)),
            cancel_generation: Arc::new(AtomicU64::new(0)),
            active_call_cancel: Arc::new(Mutex::new(None)),
            default_timeout_ms: built.default_timeout_ms,
        };
        to_js_outcome(env, Ok(repl))
    }

    #[napi(getter)]
    pub fn is_closed(&self) -> bool {
        self.closed_flag.load(Ordering::SeqCst)
    }

    #[napi]
    pub fn complete_bridge_callback(
        &self,
        id: BigInt,
        response: BridgeCallbackResponseIn,
    ) -> Result<()> {
        if let Some(ref bridge) = self.bridge
            && let Ok(req_id) = parse_u64_bigint(&id)
        {
            bridge.complete(req_id, BridgeResponse::from(response));
        }
        Ok(())
    }

    #[napi]
    pub fn cancel(&self) {
        self.cancel_generation.fetch_add(1, Ordering::SeqCst);
        if let Some((cancel, http_cancel)) = self
            .active_call_cancel
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
        {
            cancel.store(true, Ordering::SeqCst);
            http_cancel.cancel();
            self.host.external_readers().cancel_all();
            if let Some(ref bridge) = self.bridge {
                bridge.wake_all();
            }
        }
    }

    #[napi]
    pub fn close(&self) -> bool {
        self.closed_flag.store(true, Ordering::SeqCst);
        self.cancel();
        self.host.cancel();
        if let Some(ref bridge) = self.bridge {
            bridge.abort_all();
        }
        if let Ok(mut guard) = self.inner.try_lock() {
            if !guard.closed {
                guard.closed = true;
                guard.session.close();
                self.host.close();
            }
            return true;
        }
        false
    }

    #[napi]
    pub fn close_async(&self) -> AsyncTask<ReplCloseTask> {
        self.closed_flag.store(true, Ordering::SeqCst);
        self.cancel();
        self.host.cancel();
        if let Some(ref bridge) = self.bridge {
            bridge.abort_all();
        }
        AsyncTask::new(ReplCloseTask {
            host: Arc::clone(&self.host),
            inner: Arc::clone(&self.inner),
        })
    }

    #[napi]
    pub fn eval(
        &self,
        input: Utf16String,
        force: Option<bool>,
        eval_definitions: Option<bool>,
        id: Option<Utf16String>,
    ) -> AsyncTask<ReplOpTask> {
        let text = utf16_to_string(&input);
        let req_id = id.as_ref().map(utf16_to_string).transpose();
        AsyncTask::new(self.make_task(
            ReplOpKind::Eval {
                text,
                force: force.unwrap_or(false),
                eval_definitions: eval_definitions.unwrap_or(false),
            },
            req_id,
        ))
    }

    #[napi]
    pub fn load(
        &self,
        uri: Utf16String,
        contents: Option<Utf16String>,
        id: Option<Utf16String>,
    ) -> AsyncTask<ReplOpTask> {
        let uri = utf16_to_string(&uri);
        let contents = contents.as_ref().map(utf16_to_pkl);
        let req_id = id.as_ref().map(utf16_to_string).transpose();
        AsyncTask::new(self.make_task(ReplOpKind::Load { uri, contents }, req_id))
    }

    #[napi]
    pub fn complete(&self, prefix: Utf16String, id: Option<Utf16String>) -> AsyncTask<ReplOpTask> {
        let prefix = utf16_to_string(&prefix);
        let req_id = id.as_ref().map(utf16_to_string).transpose();
        AsyncTask::new(self.make_task(ReplOpKind::Complete { prefix }, req_id))
    }

    #[napi]
    pub fn reset(&self, id: Option<Utf16String>) -> AsyncTask<ReplOpTask> {
        let req_id = id.as_ref().map(utf16_to_string).transpose();
        AsyncTask::new(self.make_task(ReplOpKind::Reset, req_id))
    }

    fn make_task(
        &self,
        op: ReplOpKind,
        explicit_id: std::result::Result<Option<String>, NativeError>,
    ) -> ReplOpTask {
        ReplOpTask {
            host: Arc::clone(&self.host),
            inner: Arc::clone(&self.inner),
            bridge: self.bridge.clone(),
            logger: Arc::clone(&self.logger),
            frame_transformer: self.frame_transformer.clone(),
            test_mode: self.test_mode,
            closed_flag: Arc::clone(&self.closed_flag),
            cancel_generation: Arc::clone(&self.cancel_generation),
            start_gen: self.cancel_generation.load(Ordering::SeqCst),
            active_call_cancel: Arc::clone(&self.active_call_cancel),
            timeout_ms: self.default_timeout_ms,
            op,
            explicit_id,
        }
    }
}

impl Drop for NativeReplSession {
    fn drop(&mut self) {
        // Never call blocking `.lock()` on the V8 main thread: a libuv worker holding `inner`
        // may be waiting on a JS callback or unwinding. Signal abort and try_lock only; if the
        // worker still holds `inner`, it checks `closed_flag` on exit and closes `session`/`host`.
        self.closed_flag.store(true, Ordering::SeqCst);
        self.cancel();
        self.host.cancel();
        if let Some(ref bridge) = self.bridge {
            bridge.abort_all();
        }
        if let Ok(mut guard) = self.inner.try_lock()
            && !guard.closed
        {
            guard.closed = true;
            guard.session.close();
            self.host.close();
        }
    }
}

pub struct ReplCloseTask {
    host: Arc<NativeHost>,
    inner: Arc<Mutex<ReplInner>>,
}

impl<'task> ScopedTask<'task> for ReplCloseTask {
    type Output = ();
    type JsValue = ();

    fn compute(&mut self) -> Result<Self::Output> {
        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if !guard.closed {
            guard.closed = true;
            guard.session.close();
            self.host.close();
        }
        Ok(())
    }

    fn resolve(&mut self, _env: &'task Env, _output: Self::Output) -> Result<Self::JsValue> {
        Ok(())
    }
}

enum ReplOpKind {
    Eval {
        text: std::result::Result<String, NativeError>,
        force: bool,
        eval_definitions: bool,
    },
    Load {
        uri: std::result::Result<String, NativeError>,
        contents: Option<pkl_runtime::value::PklString>,
    },
    Complete {
        prefix: std::result::Result<String, NativeError>,
    },
    Reset,
}

pub enum ReplTaskOutput {
    Responses(Vec<ReplResponse>),
    Completions(Vec<String>),
    ResetDone,
}

pub struct ReplOpTask {
    host: Arc<NativeHost>,
    inner: Arc<Mutex<ReplInner>>,
    bridge: Option<Arc<JsBridge>>,
    logger: Arc<CapturingHostLogger>,
    frame_transformer: Option<Arc<JsFrameTransformerAdapter>>,
    test_mode: bool,
    closed_flag: Arc<AtomicBool>,
    cancel_generation: Arc<AtomicU64>,
    start_gen: u64,
    active_call_cancel: ActiveCallCancelSlot,
    timeout_ms: Option<u64>,
    op: ReplOpKind,
    explicit_id: std::result::Result<Option<String>, NativeError>,
}

impl<'task> ScopedTask<'task> for ReplOpTask {
    type Output = std::result::Result<ReplTaskOutput, NativeError>;
    type JsValue = Object<'task>;

    fn compute(&mut self) -> Result<Self::Output> {
        if self.closed_flag.load(Ordering::SeqCst) {
            return Ok(Err(NativeError::closed("REPL session has been closed.")));
        }
        if self.cancel_generation.load(Ordering::SeqCst) != self.start_gen {
            return Ok(Err(NativeError::cancelled(
                "REPL evaluation was cancelled.",
            )));
        }

        let mut guard = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if guard.closed || self.closed_flag.load(Ordering::SeqCst) {
            if !guard.closed {
                guard.closed = true;
                guard.session.close();
                self.host.close();
            }
            return Ok(Err(NativeError::closed("REPL session has been closed.")));
        }
        if self.cancel_generation.load(Ordering::SeqCst) != self.start_gen {
            return Ok(Err(NativeError::cancelled(
                "REPL evaluation was cancelled.",
            )));
        }

        guard.next_request_id += 1;
        let id = match &self.explicit_id {
            Ok(Some(custom)) => custom.clone(),
            Ok(None) => format!("req-{}", guard.next_request_id),
            Err(e) => return Ok(Err(e.clone())),
        };

        let mut staged_inline: Option<(
            pkl_runtime::value::PklString,
            pkl_runtime::value::PklString,
            pkl_runtime::source::SourceText,
        )> = None;
        let request = match &self.op {
            ReplOpKind::Eval {
                text,
                force,
                eval_definitions,
            } => match text {
                Ok(t) => ReplRequest::Eval {
                    id,
                    text: t.clone(),
                    eval_definitions: *eval_definitions,
                    force_results: *force,
                },
                Err(e) => return Ok(Err(e.clone())),
            },
            ReplOpKind::Load { uri, contents } => {
                let u = match uri {
                    Ok(u) => u.clone(),
                    Err(e) => return Ok(Err(e.clone())),
                };
                if let Some(text) = contents {
                    let raw_uri = pkl_runtime::value::PklString::from(u.as_str());
                    let resolved_uri = pkl_runtime::host::resolve_module_key_uri(
                        &raw_uri,
                        guard.session.working_dir_uri(),
                    )
                    .unwrap_or_else(|_| raw_uri.clone());
                    staged_inline = Some((
                        raw_uri,
                        resolved_uri,
                        pkl_runtime::source::SourceText::from(text.clone()),
                    ));
                }
                ReplRequest::Load { id, uri: u }
            }
            ReplOpKind::Complete { prefix } => match prefix {
                Ok(p) => ReplRequest::Completion {
                    id,
                    text: p.clone(),
                },
                Err(e) => return Ok(Err(e.clone())),
            },
            ReplOpKind::Reset => ReplRequest::Reset { id },
        };

        let call_cancel = Arc::new(AtomicBool::new(false));
        let http_cancel = CancelFlag::new();
        {
            let mut active = self
                .active_call_cancel
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            *active = Some((Arc::clone(&call_cancel), Arc::clone(&http_cancel)));
        }
        // Re-check close/cancel after installing `active_call_cancel` so a concurrent
        // `cancel()`/`close()` arriving right before installation is never dropped.
        if self.closed_flag.load(Ordering::SeqCst) {
            *self
                .active_call_cancel
                .lock()
                .unwrap_or_else(|e| e.into_inner()) = None;
            if !guard.closed {
                guard.closed = true;
                guard.session.close();
                self.host.close();
            }
            return Ok(Err(NativeError::closed("REPL session has been closed.")));
        }
        if self.cancel_generation.load(Ordering::SeqCst) != self.start_gen {
            *self
                .active_call_cancel
                .lock()
                .unwrap_or_else(|e| e.into_inner()) = None;
            return Ok(Err(NativeError::cancelled(
                "REPL evaluation was cancelled.",
            )));
        }
        let watchdog = SafeTimeoutWatchdog::start(
            self.timeout_ms,
            Arc::clone(&call_cancel),
            Arc::clone(&http_cancel),
            Arc::clone(&self.host),
            self.bridge.clone(),
        );
        let _scoped_http_cancel =
            ScopedEvalCancel::push_with_deadline(Arc::clone(&http_cancel), watchdog.deadline());
        let _bridge_guard = self
            .bridge
            .as_ref()
            .map(|b| b.enter_call(Arc::clone(&call_cancel), watchdog.deadline()));

        let map_runtime_err = |session: &mut ReplSession, err: pkl_runtime::value::RuntimeError| {
            session.abort_active_operation();
            match err.kind {
                pkl_runtime::value::ErrorKind::Timeout => NativeError::timeout(err.message),
                pkl_runtime::value::ErrorKind::Cancelled => NativeError::cancelled(err.message),
                pkl_runtime::value::ErrorKind::Closed => NativeError::closed(err.message),
                _ => NativeError::evaluation(err.message),
            }
        };

        let now_ms = self.host.clock_ms();
        let clamped_timeout = self
            .timeout_ms
            .map(|ms| ms.min(u64::MAX.saturating_sub(now_ms)));
        let deadline_ms = clamped_timeout.map(|ms| now_ms.saturating_add(ms));
        let responses = if let Err(err) =
            guard
                .session
                .start_request(request, now_ms, clamped_timeout)
        {
            Err(map_runtime_err(&mut guard.session, err))
        } else {
            let step_budget = if self.timeout_ms.is_some() {
                4096
            } else {
                32_768
            };
            loop {
                if self.closed_flag.load(Ordering::SeqCst) {
                    guard.session.close();
                    break Err(NativeError::closed("REPL session has been closed."));
                }
                let t = self.host.clock_ms();
                if watchdog.triggered() || deadline_ms.is_some_and(|dl| t >= dl) {
                    guard.session.abort_active_operation();
                    break Err(NativeError::timeout(
                        pkl_runtime::host::format_timeout_message(self.timeout_ms.unwrap_or(0)),
                    ));
                }
                if call_cancel.load(Ordering::SeqCst) {
                    guard.session.abort_active_operation();
                    break Err(NativeError::cancelled("Evaluation was cancelled."));
                }
                match guard.session.poll(t, step_budget) {
                    ReplPoll::Ready(resps) => break Ok(resps),
                    ReplPoll::Yielded => {}
                    ReplPoll::Read { token, .. } => match guard.session.read_request(&token) {
                        Ok(req) => {
                            let res = self.host.handle_read_request(&req);
                            if self.closed_flag.load(Ordering::SeqCst) {
                                guard.session.close();
                                break Err(NativeError::closed("REPL session has been closed."));
                            }
                            let t_done = self.host.clock_ms();
                            if watchdog.triggered() || deadline_ms.is_some_and(|dl| t_done >= dl) {
                                guard.session.abort_active_operation();
                                break Err(NativeError::timeout(
                                    pkl_runtime::host::format_timeout_message(
                                        self.timeout_ms.unwrap_or(0),
                                    ),
                                ));
                            }
                            if call_cancel.load(Ordering::SeqCst) {
                                guard.session.abort_active_operation();
                                break Err(NativeError::cancelled("Evaluation was cancelled."));
                            }
                            if let Err(err) = guard.session.complete_read(&token, res, t_done) {
                                break Err(map_runtime_err(&mut guard.session, err));
                            }
                        }
                        Err(err) => break Err(map_runtime_err(&mut guard.session, err)),
                    },
                    ReplPoll::Module { token, request } => {
                        let completion = if let Some((
                            ref staged_raw,
                            ref staged_resolved,
                            ref staged_text,
                        )) = staged_inline
                        {
                            let matches_staged = |u: &pkl_runtime::value::PklString| {
                                u == staged_resolved || u == staged_raw
                            };
                            match &request.kind {
                                pkl_runtime::modules::ModuleRequestKind::SelectKey {
                                    id,
                                    uri,
                                    ..
                                } if matches_staged(uri) => {
                                    match self.host.handle_module_request(&request) {
                                        pkl_runtime::modules::ModuleCompletion::KeySelected(
                                            Err(pkl_runtime::modules::ModuleHostError::NoLoader),
                                        ) => pkl_runtime::modules::ModuleCompletion::KeySelected(
                                            Ok(pkl_runtime::modules::ModuleKey {
                                                id: *id,
                                                uri: uri.clone(),
                                                kind:
                                                    pkl_runtime::modules::ModuleKeyKind::Synthetic,
                                                handle: pkl_runtime::value::PklString::from(
                                                    "repl:inline",
                                                ),
                                            }),
                                        ),
                                        other => other,
                                    }
                                }
                                pkl_runtime::modules::ModuleRequestKind::QueryCachePolicy {
                                    key,
                                } if matches_staged(&key.uri) => {
                                    pkl_runtime::modules::ModuleCompletion::CachePolicy(Ok(false))
                                }
                                pkl_runtime::modules::ModuleRequestKind::Resolve {
                                    key, ..
                                } if matches_staged(&key.uri) => {
                                    match self.host.check_resolve_module(&key.uri) {
                                        Ok(()) => pkl_runtime::modules::ModuleCompletion::Resolved(
                                            Ok(pkl_runtime::modules::ResolvedModule {
                                                original_uri: key.uri.clone(),
                                                resolved_uri: key.uri.clone(),
                                                source_uri: key.uri.clone(),
                                            }),
                                        ),
                                        Err(err) => {
                                            pkl_runtime::modules::ModuleCompletion::Resolved(Err(
                                                err,
                                            ))
                                        }
                                    }
                                }
                                pkl_runtime::modules::ModuleRequestKind::ResolveKey {
                                    key,
                                    resolved_key_id,
                                    ..
                                } if matches_staged(&key.uri) => {
                                    match self.host.check_resolve_module(&key.uri) {
                                        Ok(()) => {
                                            pkl_runtime::modules::ModuleCompletion::KeyResolved(Ok(
                                                pkl_runtime::modules::ResolvedModuleKey {
                                                    id: *resolved_key_id,
                                                    key: key.clone(),
                                                    handle: pkl_runtime::value::PklString::default(
                                                    ),
                                                    module: pkl_runtime::modules::ResolvedModule {
                                                        original_uri: key.uri.clone(),
                                                        resolved_uri: key.uri.clone(),
                                                        source_uri: key.uri.clone(),
                                                    },
                                                },
                                            ))
                                        }
                                        Err(err) => {
                                            pkl_runtime::modules::ModuleCompletion::KeyResolved(
                                                Err(err),
                                            )
                                        }
                                    }
                                }
                                pkl_runtime::modules::ModuleRequestKind::LoadSource {
                                    resolved,
                                    ..
                                } if matches_staged(&resolved.resolved_uri)
                                    || matches_staged(&resolved.original_uri) =>
                                {
                                    pkl_runtime::modules::ModuleCompletion::Source(Ok(
                                        pkl_runtime::modules::ModuleSource {
                                            text: staged_text.clone(),
                                            inferred_name: pkl_runtime::value::PklString::default(),
                                        },
                                    ))
                                }
                                _ => self.host.handle_module_request(&request),
                            }
                        } else {
                            self.host.handle_module_request(&request)
                        };
                        if self.closed_flag.load(Ordering::SeqCst) {
                            guard.session.close();
                            break Err(NativeError::closed("REPL session has been closed."));
                        }
                        let t_done = self.host.clock_ms();
                        if watchdog.triggered() || deadline_ms.is_some_and(|dl| t_done >= dl) {
                            guard.session.abort_active_operation();
                            break Err(NativeError::timeout(
                                pkl_runtime::host::format_timeout_message(
                                    self.timeout_ms.unwrap_or(0),
                                ),
                            ));
                        }
                        if call_cancel.load(Ordering::SeqCst) {
                            guard.session.abort_active_operation();
                            break Err(NativeError::cancelled("Evaluation was cancelled."));
                        }
                        if let Err(err) = guard.session.complete_module(&token, completion, t_done)
                        {
                            break Err(map_runtime_err(&mut guard.session, err));
                        }
                    }
                    ReplPoll::Glob { token, request } => {
                        let completion = self.host.handle_glob_request(&request);
                        if self.closed_flag.load(Ordering::SeqCst) {
                            guard.session.close();
                            break Err(NativeError::closed("REPL session has been closed."));
                        }
                        let t_done = self.host.clock_ms();
                        if watchdog.triggered() || deadline_ms.is_some_and(|dl| t_done >= dl) {
                            guard.session.abort_active_operation();
                            break Err(NativeError::timeout(
                                pkl_runtime::host::format_timeout_message(
                                    self.timeout_ms.unwrap_or(0),
                                ),
                            ));
                        }
                        if call_cancel.load(Ordering::SeqCst) {
                            guard.session.abort_active_operation();
                            break Err(NativeError::cancelled("Evaluation was cancelled."));
                        }
                        if let Err(err) = guard.session.complete_glob(&token, completion, t_done) {
                            break Err(map_runtime_err(&mut guard.session, err));
                        }
                    }
                    ReplPoll::Log { token, event } => {
                        let log_res =
                            self.logger
                                .log_checked(event.level, &event.message, &event.frame);
                        if self.closed_flag.load(Ordering::SeqCst) {
                            guard.session.close();
                            break Err(NativeError::closed("REPL session has been closed."));
                        }
                        let t_done = self.host.clock_ms();
                        if watchdog.triggered() || deadline_ms.is_some_and(|dl| t_done >= dl) {
                            guard.session.abort_active_operation();
                            break Err(NativeError::timeout(
                                pkl_runtime::host::format_timeout_message(
                                    self.timeout_ms.unwrap_or(0),
                                ),
                            ));
                        }
                        if call_cancel.load(Ordering::SeqCst) {
                            guard.session.abort_active_operation();
                            break Err(NativeError::cancelled("Evaluation was cancelled."));
                        }
                        if let Err(err) = guard.session.acknowledge_log(&token, log_res, t_done) {
                            break Err(map_runtime_err(&mut guard.session, err));
                        }
                    }
                    ReplPoll::TransformFrame { token, frame } => {
                        let base = if self.test_mode {
                            (*frame).clone()
                        } else {
                            FrameTransformer::Default.transform((*frame).clone())
                        };
                        let transformed = match &self.frame_transformer {
                            Some(tf) => tf.transform_checked(base),
                            None => Ok(base),
                        };
                        if self.closed_flag.load(Ordering::SeqCst) {
                            guard.session.close();
                            break Err(NativeError::closed("REPL session has been closed."));
                        }
                        let t_done = self.host.clock_ms();
                        if watchdog.triggered() || deadline_ms.is_some_and(|dl| t_done >= dl) {
                            guard.session.abort_active_operation();
                            break Err(NativeError::timeout(
                                pkl_runtime::host::format_timeout_message(
                                    self.timeout_ms.unwrap_or(0),
                                ),
                            ));
                        }
                        if call_cancel.load(Ordering::SeqCst) {
                            guard.session.abort_active_operation();
                            break Err(NativeError::cancelled("Evaluation was cancelled."));
                        }
                        if let Err(err) =
                            guard
                                .session
                                .complete_frame_transform(&token, transformed, t_done)
                        {
                            break Err(map_runtime_err(&mut guard.session, err));
                        }
                    }
                }
            }
        };

        let timed_out = watchdog.stop();
        {
            let mut active = self
                .active_call_cancel
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            *active = None;
        }
        if self.closed_flag.load(Ordering::SeqCst) && !guard.closed {
            guard.closed = true;
            guard.session.close();
            self.host.close();
            return Ok(Err(NativeError::closed("REPL session has been closed.")));
        }

        if timed_out && responses.is_err() {
            return Ok(Err(NativeError::timeout(
                pkl_runtime::host::format_timeout_message(self.timeout_ms.unwrap_or(0)),
            )));
        }

        let resps = match responses {
            Ok(r) => r,
            Err(e) => return Ok(Err(e)),
        };

        match self.op {
            ReplOpKind::Complete { .. } => {
                let mut members = Vec::new();
                for r in resps {
                    if let ReplResponse::Completion { members: set } = r {
                        members.extend(set);
                    }
                }
                Ok(Ok(ReplTaskOutput::Completions(members)))
            }
            ReplOpKind::Reset => Ok(Ok(ReplTaskOutput::ResetDone)),
            ReplOpKind::Eval { .. } | ReplOpKind::Load { .. } => {
                Ok(Ok(ReplTaskOutput::Responses(resps)))
            }
        }
    }

    fn resolve(&mut self, env: &'task Env, output: Self::Output) -> Result<Self::JsValue> {
        match output {
            Ok(ReplTaskOutput::ResetDone) => to_js_outcome(env, Ok(Null)),
            Ok(ReplTaskOutput::Completions(members)) => to_js_outcome(env, Ok(members)),
            Ok(ReplTaskOutput::Responses(resps)) => {
                let mut js_resps = Vec::with_capacity(resps.len());
                for r in resps {
                    let mut item = Object::new(env)?;
                    match r {
                        ReplResponse::EvalSuccess { result } => {
                            item.set("kind", "evalSuccess")?;
                            item.set("result", result)?;
                        }
                        ReplResponse::EvalError { message } => {
                            item.set("kind", "evalError")?;
                            item.set("message", message)?;
                        }
                        ReplResponse::IncompleteInput => {
                            item.set("kind", "incompleteInput")?;
                        }
                        ReplResponse::InvalidRequest { message } => {
                            item.set("kind", "invalidRequest")?;
                            item.set("message", message)?;
                        }
                        ReplResponse::InternalError { message } => {
                            item.set("kind", "internalError")?;
                            item.set("message", message)?;
                        }
                        ReplResponse::Completion { members } => {
                            item.set("kind", "completion")?;
                            item.set("members", members.into_iter().collect::<Vec<_>>())?;
                        }
                    }
                    js_resps.push(item);
                }
                to_js_outcome(env, Ok(Array::from_vec(env, js_resps)?))
            }
            Err(err) => to_js_outcome::<Null>(env, Err(err)),
        }
    }
}
