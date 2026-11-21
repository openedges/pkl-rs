//! Thread-safe JS callback bridge enabling sync and async (`Promise`) JS
//! `ModuleReader`, `ResourceReader`, `Logger`, `StackFrameTransformer`, and
//! `SecurityManager` extensions from libuv worker threads with zero deadlocks
//! and instant abort/timeout wakeup.

use crate::error::utf16_to_pkl;
use napi::{
    Status,
    bindgen_prelude::{BigInt, Function, Uint8Array, Uint16Array, Utf16String},
    threadsafe_function::{
        ThreadsafeCallContext, ThreadsafeFunction, ThreadsafeFunctionCallMode, UnknownReturnValue,
    },
};
use napi_derive::napi;
use pkl_host_native::{
    CancelFlag, CustomModuleReader, CustomResourceReader, CustomSecurityChecker,
    HostFrameTransformer, HostLogger,
};
use pkl_runtime::{
    glob::PathElement,
    host::{IoFailure, ReadFailure, security::SecurityPolicy},
    logging::{LogFrame, LogLevel},
    modules::{ModuleHostError, ModuleSource, ResolvedModule},
    resources::{ReadResult, Resource},
    source::SourceText,
    value::PklString,
};
use std::collections::HashMap;
use std::sync::{
    Arc, Condvar, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use std::time::{Duration, Instant};

#[napi(object)]
pub struct BridgeFrameJs {
    pub module_uri_u16: Uint16Array,
    pub member_name_u16: Option<Uint16Array>,
    pub source_lines_u16: Vec<Uint16Array>,
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
}

#[napi(object)]
pub struct BridgeRequestJs {
    pub id: BigInt,
    pub op: String,
    pub reader_index: Option<u32>,
    pub uri_u16: Option<Uint16Array>,
    pub uri2_u16: Option<Uint16Array>,
    pub level: Option<String>,
    pub message_u16: Option<Uint16Array>,
    pub frame: Option<BridgeFrameJs>,
}

#[napi(object)]
pub struct BridgePathElementIn {
    pub name: Utf16String,
    pub is_directory: bool,
}

#[napi(object)]
pub struct BridgeFrameIn {
    pub module_uri: Utf16String,
    pub member_name: Option<Utf16String>,
    pub source_lines: Vec<Utf16String>,
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
}

#[napi(object)]
pub struct BridgeCallbackResponseIn {
    pub ok: bool,
    pub error_kind: Option<String>,
    pub error_message: Option<Utf16String>,
    pub text: Option<Utf16String>,
    pub bytes: Option<Uint8Array>,
    pub missing: Option<bool>,
    pub bool_value: Option<bool>,
    pub elements: Option<Vec<BridgePathElementIn>>,
    pub frame: Option<BridgeFrameIn>,
}

#[derive(Clone, Debug)]
pub struct BridgeFramePayload {
    pub module_uri: Vec<u16>,
    pub member_name: Option<Vec<u16>>,
    pub source_lines: Vec<Vec<u16>>,
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
}

impl From<&LogFrame> for BridgeFramePayload {
    fn from(frame: &LogFrame) -> Self {
        Self {
            module_uri: frame.module_uri.encode_utf16().collect(),
            member_name: frame
                .member_name
                .as_ref()
                .map(|m| m.encode_utf16().collect()),
            source_lines: frame
                .source_lines
                .iter()
                .map(|line| line.encode_utf16().collect())
                .collect(),
            start_line: frame.start_line as u32,
            start_column: frame.start_column as u32,
            end_line: frame.end_line as u32,
            end_column: frame.end_column as u32,
        }
    }
}

#[derive(Clone, Debug)]
pub struct BridgeRequestPayload {
    pub id: u64,
    pub op: String,
    pub reader_index: Option<u32>,
    pub uri: Option<Vec<u16>>,
    pub uri2: Option<Vec<u16>>,
    pub level: Option<String>,
    pub message: Option<Vec<u16>>,
    pub frame: Option<BridgeFramePayload>,
}

#[derive(Clone, Debug)]
pub struct BridgeResponse {
    pub ok: bool,
    pub error_kind: Option<String>,
    pub error_message: Option<PklString>,
    pub text: Option<PklString>,
    pub bytes: Option<Vec<u8>>,
    pub missing: bool,
    pub bool_value: Option<bool>,
    pub elements: Option<Vec<PathElement>>,
    pub frame: Option<LogFrame>,
}

impl From<BridgeCallbackResponseIn> for BridgeResponse {
    fn from(input: BridgeCallbackResponseIn) -> Self {
        Self {
            ok: input.ok,
            error_kind: input.error_kind,
            error_message: input.error_message.as_ref().map(utf16_to_pkl),
            text: input.text.as_ref().map(utf16_to_pkl),
            bytes: input.bytes.map(|b| b.to_vec()),
            missing: input.missing.unwrap_or(false),
            bool_value: input.bool_value,
            elements: input.elements.map(|elems| {
                elems
                    .into_iter()
                    .map(|e| PathElement {
                        name: utf16_to_pkl(&e.name),
                        is_directory: e.is_directory,
                    })
                    .collect()
            }),
            frame: input.frame.map(|f| LogFrame {
                module_uri: utf16_to_pkl(&f.module_uri),
                member_name: f.member_name.as_ref().map(utf16_to_pkl),
                source_lines: f
                    .source_lines
                    .iter()
                    .map(utf16_to_pkl)
                    .collect::<Vec<_>>()
                    .into(),
                start_line: f.start_line as usize,
                start_column: f.start_column as usize,
                end_line: f.end_line as usize,
                end_column: f.end_column as usize,
            }),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BridgeAbort {
    Cancelled,
    TimedOut,
    Closed,
}

type WaitSlot = Arc<(Mutex<Option<BridgeResponse>>, Condvar)>;
type DispatchTsfn = ThreadsafeFunction<
    BridgeRequestPayload,
    UnknownReturnValue,
    BridgeRequestJs,
    Status,
    false,
    true,
>;

#[derive(Clone)]
struct ActiveCallState {
    call_cancel: Arc<AtomicBool>,
    deadline: Option<Instant>,
}

/// Shared JS callback dispatcher held by `NativeEvaluator`, `NativeReplSession`, and `Analyzer`.
pub struct JsBridge {
    tsfn: DispatchTsfn,
    next_id: AtomicU64,
    pending: Mutex<HashMap<u64, WaitSlot>>,
    active_call: Mutex<Option<ActiveCallState>>,
    closed: AtomicBool,
    host_cancel: Arc<CancelFlag>,
}

impl JsBridge {
    pub fn new(
        callback: Function<'_, BridgeRequestJs, UnknownReturnValue>,
        host_cancel: Arc<CancelFlag>,
    ) -> napi::Result<Arc<Self>> {
        let tsfn: DispatchTsfn = callback
            .build_threadsafe_function::<BridgeRequestPayload>()
            .callee_handled::<false>()
            .weak::<true>()
            .build_callback(|ctx: ThreadsafeCallContext<BridgeRequestPayload>| {
                let req = ctx.value;
                Ok(BridgeRequestJs {
                    id: BigInt::from(req.id),
                    op: req.op,
                    reader_index: req.reader_index,
                    uri_u16: req.uri.map(Uint16Array::new),
                    uri2_u16: req.uri2.map(Uint16Array::new),
                    level: req.level,
                    message_u16: req.message.map(Uint16Array::new),
                    frame: req.frame.map(|f| BridgeFrameJs {
                        module_uri_u16: Uint16Array::new(f.module_uri),
                        member_name_u16: f.member_name.map(Uint16Array::new),
                        source_lines_u16: f
                            .source_lines
                            .into_iter()
                            .map(Uint16Array::new)
                            .collect(),
                        start_line: f.start_line,
                        start_column: f.start_column,
                        end_line: f.end_line,
                        end_column: f.end_column,
                    }),
                })
            })?;
        Ok(Arc::new(Self {
            tsfn,
            next_id: AtomicU64::new(1),
            pending: Mutex::new(HashMap::new()),
            active_call: Mutex::new(None),
            closed: AtomicBool::new(false),
            host_cancel,
        }))
    }

    /// Registers the active evaluation's cancel flag and deadline for callback waiting.
    pub fn enter_call(
        self: &Arc<Self>,
        call_cancel: Arc<AtomicBool>,
        deadline: Option<Instant>,
    ) -> ActiveCallGuard {
        let mut active = self.active_call.lock().unwrap_or_else(|e| e.into_inner());
        *active = Some(ActiveCallState {
            call_cancel,
            deadline,
        });
        ActiveCallGuard {
            bridge: Arc::clone(self),
        }
    }

    /// Completes a pending JS callback request from Node's main thread.
    pub fn complete(&self, id: u64, response: BridgeResponse) {
        let slot = {
            let mut pending = self.pending.lock().unwrap_or_else(|e| e.into_inner());
            pending.remove(&id)
        };
        if let Some(slot) = slot {
            let (lock, cvar) = &*slot;
            let mut guard = lock.lock().unwrap_or_else(|e| e.into_inner());
            *guard = Some(response);
            cvar.notify_all();
        }
    }

    /// Wakes all waiting worker threads immediately (used on cancel or timeout).
    pub fn wake_all(&self) {
        let slots: Vec<WaitSlot> = {
            let pending = self.pending.lock().unwrap_or_else(|e| e.into_inner());
            pending.values().cloned().collect()
        };
        for slot in slots {
            let (lock, cvar) = &*slot;
            let _guard = lock.lock().unwrap_or_else(|e| e.into_inner());
            cvar.notify_all();
        }
    }

    /// Marks the bridge closed and wakes every waiting worker thread immediately.
    pub fn abort_all(&self) {
        self.closed.store(true, Ordering::SeqCst);
        self.wake_all();
    }

    fn check_abort(&self) -> Result<(), BridgeAbort> {
        if self.closed.load(Ordering::SeqCst) {
            return Err(BridgeAbort::Closed);
        }
        if self.host_cancel.is_cancelled() {
            return Err(BridgeAbort::Cancelled);
        }
        let active = self
            .active_call
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        if let Some(state) = active {
            if state.call_cancel.load(Ordering::SeqCst) {
                return Err(BridgeAbort::Cancelled);
            }
            if let Some(deadline) = state.deadline
                && Instant::now() >= deadline
            {
                return Err(BridgeAbort::TimedOut);
            }
        }
        Ok(())
    }

    pub fn invoke(&self, mut payload: BridgeRequestPayload) -> Result<BridgeResponse, BridgeAbort> {
        self.check_abort()?;
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        payload.id = id;
        let slot: WaitSlot = Arc::new((Mutex::new(None), Condvar::new()));
        {
            let mut pending = self.pending.lock().unwrap_or_else(|e| e.into_inner());
            pending.insert(id, Arc::clone(&slot));
        }
        let status = self
            .tsfn
            .call(payload, ThreadsafeFunctionCallMode::NonBlocking);
        if status != Status::Ok {
            let mut pending = self.pending.lock().unwrap_or_else(|e| e.into_inner());
            pending.remove(&id);
            return Err(BridgeAbort::Closed);
        }

        let (lock, cvar) = &*slot;
        let mut guard = lock.lock().unwrap_or_else(|e| e.into_inner());
        loop {
            if let Some(resp) = guard.take() {
                return Ok(resp);
            }
            if let Err(abort) = self.check_abort() {
                drop(guard);
                let mut pending = self.pending.lock().unwrap_or_else(|e| e.into_inner());
                pending.remove(&id);
                return Err(abort);
            }
            let (next, _) = cvar
                .wait_timeout(guard, Duration::from_millis(20))
                .unwrap_or_else(|e| e.into_inner());
            guard = next;
        }
    }
}

pub struct ActiveCallGuard {
    bridge: Arc<JsBridge>,
}

impl Drop for ActiveCallGuard {
    fn drop(&mut self) {
        let mut active = self
            .bridge
            .active_call
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        *active = None;
    }
}

/// Optional test-log capturing sink forwarding to a delegate logger (JS or stderr).
pub struct CapturingHostLogger {
    js_bridge: Option<Arc<JsBridge>>,
    has_trace: bool,
    has_warn: bool,
    use_stderr_default: bool,
    capturing: AtomicBool,
    buffer: Mutex<String>,
}

impl CapturingHostLogger {
    pub fn new(
        js_bridge: Option<Arc<JsBridge>>,
        has_trace: bool,
        has_warn: bool,
        use_stderr_default: bool,
    ) -> Arc<Self> {
        Arc::new(Self {
            js_bridge,
            has_trace,
            has_warn,
            use_stderr_default,
            capturing: AtomicBool::new(false),
            buffer: Mutex::new(String::new()),
        })
    }

    pub fn begin_capture(&self) {
        {
            let mut buf = self.buffer.lock().unwrap_or_else(|e| e.into_inner());
            buf.clear();
        }
        self.capturing.store(true, Ordering::SeqCst);
    }

    pub fn end_capture(&self) -> String {
        self.capturing.store(false, Ordering::SeqCst);
        let mut buf = self.buffer.lock().unwrap_or_else(|e| e.into_inner());
        std::mem::take(&mut *buf)
    }
}

impl HostLogger for CapturingHostLogger {
    fn log(&self, level: LogLevel, message: &PklString, frame: &LogFrame) {
        if self.capturing.load(Ordering::SeqCst) {
            let mut buf = self.buffer.lock().unwrap_or_else(|e| e.into_inner());
            buf.push_str(&message.to_java_utf8());
            buf.push('\n');
        }
        if let Some(ref bridge) = self.js_bridge {
            let dispatch = match level {
                LogLevel::Trace => self.has_trace,
                LogLevel::Warn => self.has_warn,
            };
            if dispatch {
                let _ = bridge.invoke(BridgeRequestPayload {
                    id: 0,
                    op: "log".to_string(),
                    reader_index: None,
                    uri: None,
                    uri2: None,
                    level: Some(match level {
                        LogLevel::Trace => "trace".to_string(),
                        LogLevel::Warn => "warn".to_string(),
                    }),
                    message: Some(message.encode_utf16().collect()),
                    frame: Some(BridgeFramePayload::from(frame)),
                });
            }
            return;
        }
        if self.use_stderr_default {
            use std::io::Write;
            let level_str = match level {
                LogLevel::Trace => "TRACE",
                LogLevel::Warn => "WARN",
            };
            let msg = message.to_java_utf8();
            let space = if msg.ends_with('\n') { "" } else { " " };
            let formatted = format!(
                "pkl: {level_str}: {msg}{space}({})\n",
                frame.module_uri.to_java_utf8()
            );
            let mut stderr = std::io::stderr().lock();
            let _ = stderr.write_all(formatted.as_bytes());
            let _ = stderr.flush();
        }
    }
}

/// Custom JS `frameTransformer` adapter.
pub struct JsFrameTransformerAdapter {
    bridge: Arc<JsBridge>,
}

impl JsFrameTransformerAdapter {
    pub fn new(bridge: Arc<JsBridge>) -> Arc<Self> {
        Arc::new(Self { bridge })
    }
}

impl HostFrameTransformer for JsFrameTransformerAdapter {
    fn transform(&self, frame: LogFrame) -> LogFrame {
        let payload = BridgeRequestPayload {
            id: 0,
            op: "transformFrame".to_string(),
            reader_index: None,
            uri: None,
            uri2: None,
            level: None,
            message: None,
            frame: Some(BridgeFramePayload::from(&frame)),
        };
        match self.bridge.invoke(payload) {
            Ok(resp) if resp.ok => resp.frame.unwrap_or(frame),
            _ => frame,
        }
    }
}

/// Flags describing which `securityManager` hooks were supplied by JS.
#[derive(Clone, Copy, Debug, Default)]
pub struct JsSecurityHooks {
    pub has_resolve_module: bool,
    pub has_import_module: bool,
    pub has_read_resource: bool,
    pub has_resolve_resource: bool,
}

impl JsSecurityHooks {
    pub fn any(self) -> bool {
        self.has_resolve_module
            || self.has_import_module
            || self.has_read_resource
            || self.has_resolve_resource
    }
}

/// Custom JS `SecurityManager` adapter with standard `SecurityPolicy` fallback for omitted hooks.
pub struct JsSecurityAdapter {
    bridge: Arc<JsBridge>,
    hooks: JsSecurityHooks,
    fallback: SecurityPolicy,
}

impl JsSecurityAdapter {
    pub fn new(
        bridge: Arc<JsBridge>,
        hooks: JsSecurityHooks,
        fallback: SecurityPolicy,
    ) -> Arc<Self> {
        Arc::new(Self {
            bridge,
            hooks,
            fallback,
        })
    }

    fn call_check(
        &self,
        op: &str,
        uri: &PklString,
        uri2: Option<&PklString>,
    ) -> Result<(), PklString> {
        let payload = BridgeRequestPayload {
            id: 0,
            op: op.to_string(),
            reader_index: None,
            uri: Some(uri.encode_utf16().collect()),
            uri2: uri2.map(|u| u.encode_utf16().collect()),
            level: None,
            message: None,
            frame: None,
        };
        match self.bridge.invoke(payload) {
            Ok(resp) if resp.ok => Ok(()),
            Ok(resp) => Err(resp
                .error_message
                .unwrap_or_else(|| PklString::from("Security manager denied access."))),
            Err(BridgeAbort::Cancelled) => Err(PklString::from("Evaluation was cancelled.")),
            Err(BridgeAbort::TimedOut) => Err(PklString::from("Evaluation timed out.")),
            Err(BridgeAbort::Closed) => Err(PklString::from("Evaluator has been closed.")),
        }
    }
}

impl CustomSecurityChecker for JsSecurityAdapter {
    fn check_resolve_module(&self, uri: &PklString) -> Result<(), PklString> {
        if self.hooks.has_resolve_module {
            self.call_check("checkResolveModule", uri, None)
        } else {
            self.fallback
                .check_resolve_module(uri)
                .map_err(|err| match err {
                    ModuleHostError::SecurityManagerException { message } => {
                        message.unwrap_or_default()
                    }
                    other => PklString::from(format!("{other:?}")),
                })
        }
    }

    fn check_import_module(
        &self,
        importing_module: &PklString,
        imported_module: &PklString,
    ) -> Result<(), PklString> {
        if self.hooks.has_import_module {
            self.call_check("checkImportModule", importing_module, Some(imported_module))
        } else {
            self.fallback
                .check_import_module(importing_module, imported_module)
                .map_err(|err| match err {
                    ModuleHostError::SecurityManagerException { message } => {
                        message.unwrap_or_default()
                    }
                    other => PklString::from(format!("{other:?}")),
                })
        }
    }

    fn check_read_resource(&self, resource: &PklString) -> Result<(), PklString> {
        if self.hooks.has_read_resource {
            self.call_check("checkReadResource", resource, None)
        } else {
            self.fallback
                .check_read_resource(resource)
                .map_err(|err| match err {
                    ReadFailure::SecurityManagerException { message } => {
                        message.unwrap_or_default()
                    }
                    other => PklString::from(format!("{other:?}")),
                })
        }
    }

    fn check_resolve_resource(&self, resource: &PklString) -> Result<(), PklString> {
        if self.hooks.has_resolve_resource {
            self.call_check("checkResolveResource", resource, None)
        } else if self.hooks.has_read_resource {
            self.call_check("checkReadResource", resource, None)
        } else {
            self.fallback
                .check_read_resource(resource)
                .map_err(|err| match err {
                    ReadFailure::SecurityManagerException { message } => {
                        message.unwrap_or_default()
                    }
                    other => PklString::from(format!("{other:?}")),
                })
        }
    }
}

/// Metadata describing one registered JS `ModuleReader`.
#[derive(Clone, Debug)]
pub struct JsModuleReaderSpec {
    pub index: u32,
    pub scheme: String,
    pub is_local: bool,
    pub is_globbable: bool,
    pub has_hierarchical_uris: bool,
    pub is_cached: bool,
    pub has_has_element: bool,
    pub has_list_elements: bool,
}

/// Custom JS `ModuleReader` adapter implementing `pkl_host_native::CustomModuleReader`.
pub struct JsModuleReaderAdapter {
    bridge: Arc<JsBridge>,
    spec: JsModuleReaderSpec,
}

impl JsModuleReaderAdapter {
    pub fn new(bridge: Arc<JsBridge>, spec: JsModuleReaderSpec) -> Arc<Self> {
        Arc::new(Self { bridge, spec })
    }

    fn scheme_matches(&self, uri: &PklString) -> bool {
        let s = uri.to_java_utf8();
        let Some((scheme, _)) = s.split_once(':') else {
            return false;
        };
        scheme.eq_ignore_ascii_case(&self.spec.scheme)
    }
}

impl CustomModuleReader for JsModuleReaderAdapter {
    fn matches_uri(&self, uri: &PklString) -> bool {
        self.scheme_matches(uri)
    }

    fn is_hierarchical(&self, _uri: &PklString) -> bool {
        self.spec.has_hierarchical_uris
    }

    fn is_local(&self, _uri: &PklString) -> bool {
        self.spec.is_local
    }

    fn is_globbable(&self, _uri: &PklString) -> bool {
        self.spec.is_globbable
    }

    fn is_cached(&self, _uri: &PklString) -> bool {
        self.spec.is_cached
    }

    fn resolve_module(
        &self,
        uri: &PklString,
    ) -> Result<(ResolvedModule, ModuleSource), ModuleHostError> {
        let resp = self
            .bridge
            .invoke(BridgeRequestPayload {
                id: 0,
                op: "moduleRead".to_string(),
                reader_index: Some(self.spec.index),
                uri: Some(uri.encode_utf16().collect()),
                uri2: None,
                level: None,
                message: None,
                frame: None,
            })
            .map_err(abort_to_module_err)?;
        if !resp.ok {
            return Err(response_err_to_module_err(&resp));
        }
        let text = resp.text.ok_or(ModuleHostError::NotFound)?;
        let module = ResolvedModule {
            original_uri: uri.clone(),
            resolved_uri: uri.clone(),
            source_uri: uri.clone(),
        };
        let source = ModuleSource {
            text: SourceText::from(text),
            inferred_name: PklString::default(),
        };
        Ok((module, source))
    }

    fn has_element(&self, element_uri: &PklString) -> Result<bool, ModuleHostError> {
        if !self.spec.has_has_element {
            return Ok(false);
        }
        let resp = self
            .bridge
            .invoke(BridgeRequestPayload {
                id: 0,
                op: "moduleHasElement".to_string(),
                reader_index: Some(self.spec.index),
                uri: Some(element_uri.encode_utf16().collect()),
                uri2: None,
                level: None,
                message: None,
                frame: None,
            })
            .map_err(abort_to_module_err)?;
        if !resp.ok {
            return Err(response_err_to_module_err(&resp));
        }
        Ok(resp.bool_value.unwrap_or(false))
    }

    fn list_elements(&self, base_uri: &PklString) -> Result<Vec<PathElement>, ModuleHostError> {
        if !self.spec.has_list_elements {
            return Err(ModuleHostError::Unsupported(
                "Module reader does not support listElements.".into(),
            ));
        }
        let resp = self
            .bridge
            .invoke(BridgeRequestPayload {
                id: 0,
                op: "moduleListElements".to_string(),
                reader_index: Some(self.spec.index),
                uri: Some(base_uri.encode_utf16().collect()),
                uri2: None,
                level: None,
                message: None,
                frame: None,
            })
            .map_err(abort_to_module_err)?;
        if !resp.ok {
            return Err(response_err_to_module_err(&resp));
        }
        Ok(resp.elements.unwrap_or_default())
    }
}

/// Metadata describing one registered JS `ResourceReader`.
#[derive(Clone, Debug)]
pub struct JsResourceReaderSpec {
    pub index: u32,
    pub scheme: String,
    pub is_globbable: bool,
    pub has_hierarchical_uris: bool,
    pub has_has_element: bool,
    pub has_list_elements: bool,
}

/// Custom JS `ResourceReader` adapter implementing `pkl_host_native::CustomResourceReader`.
pub struct JsResourceReaderAdapter {
    bridge: Arc<JsBridge>,
    spec: JsResourceReaderSpec,
}

impl JsResourceReaderAdapter {
    pub fn new(bridge: Arc<JsBridge>, spec: JsResourceReaderSpec) -> Arc<Self> {
        Arc::new(Self { bridge, spec })
    }
}

impl CustomResourceReader for JsResourceReaderAdapter {
    fn scheme(&self) -> &str {
        &self.spec.scheme
    }

    fn has_hierarchical_uris(&self) -> bool {
        self.spec.has_hierarchical_uris
    }

    fn is_globbable(&self) -> bool {
        self.spec.is_globbable
    }

    fn read(&self, uri: &PklString) -> Result<ReadResult, ReadFailure> {
        let resp = self
            .bridge
            .invoke(BridgeRequestPayload {
                id: 0,
                op: "resourceRead".to_string(),
                reader_index: Some(self.spec.index),
                uri: Some(uri.encode_utf16().collect()),
                uri2: None,
                level: None,
                message: None,
                frame: None,
            })
            .map_err(abort_to_read_err)?;
        if !resp.ok {
            return Err(response_err_to_read_err(&resp));
        }
        if resp.missing {
            return Ok(ReadResult::Missing);
        }
        if let Some(bytes) = resp.bytes {
            return Ok(ReadResult::Resource(Resource {
                uri: uri.clone(),
                bytes: bytes.into(),
            }));
        }
        if let Some(text) = resp.text {
            return Ok(ReadResult::Text(text));
        }
        Ok(ReadResult::Missing)
    }

    fn has_element(&self, uri: &PklString) -> Result<bool, ModuleHostError> {
        if !self.spec.has_has_element {
            return match self.read(uri) {
                Ok(ReadResult::Missing) => Ok(false),
                Ok(_) => Ok(true),
                Err(err) => Err(read_failure_to_module_host(err)),
            };
        }
        let resp = self
            .bridge
            .invoke(BridgeRequestPayload {
                id: 0,
                op: "resourceHasElement".to_string(),
                reader_index: Some(self.spec.index),
                uri: Some(uri.encode_utf16().collect()),
                uri2: None,
                level: None,
                message: None,
                frame: None,
            })
            .map_err(abort_to_module_err)?;
        if !resp.ok {
            return Err(response_err_to_module_err(&resp));
        }
        Ok(resp.bool_value.unwrap_or(false))
    }

    fn list_elements(&self, base_uri: &PklString) -> Result<Vec<PathElement>, ModuleHostError> {
        if !self.spec.has_list_elements {
            return Err(ModuleHostError::Unsupported(
                "Resource reader does not support listElements.".into(),
            ));
        }
        let resp = self
            .bridge
            .invoke(BridgeRequestPayload {
                id: 0,
                op: "resourceListElements".to_string(),
                reader_index: Some(self.spec.index),
                uri: Some(base_uri.encode_utf16().collect()),
                uri2: None,
                level: None,
                message: None,
                frame: None,
            })
            .map_err(abort_to_module_err)?;
        if !resp.ok {
            return Err(response_err_to_module_err(&resp));
        }
        Ok(resp.elements.unwrap_or_default())
    }
}

fn abort_to_module_err(abort: BridgeAbort) -> ModuleHostError {
    match abort {
        BridgeAbort::Cancelled => ModuleHostError::Io(IoFailure::from("Evaluation was cancelled.")),
        BridgeAbort::TimedOut => ModuleHostError::Io(IoFailure::from("Evaluation timed out.")),
        BridgeAbort::Closed => ModuleHostError::Io(IoFailure::from("Evaluator has been closed.")),
    }
}

fn abort_to_read_err(abort: BridgeAbort) -> ReadFailure {
    match abort {
        BridgeAbort::Cancelled => ReadFailure::Io(IoFailure::from("Evaluation was cancelled.")),
        BridgeAbort::TimedOut => ReadFailure::Io(IoFailure::from("Evaluation timed out.")),
        BridgeAbort::Closed => ReadFailure::Io(IoFailure::from("Evaluator has been closed.")),
    }
}

fn response_err_to_module_err(resp: &BridgeResponse) -> ModuleHostError {
    let msg = resp
        .error_message
        .clone()
        .unwrap_or_else(|| PklString::from("Custom reader callback failed."));
    match resp.error_kind.as_deref() {
        Some("security") => ModuleHostError::SecurityManagerException { message: Some(msg) },
        Some("not-found") => ModuleHostError::NotFound,
        Some("io") => ModuleHostError::Io(IoFailure::from(msg.to_java_utf8())),
        _ => ModuleHostError::EvalError {
            message: msg,
            hint: None,
        },
    }
}

fn response_err_to_read_err(resp: &BridgeResponse) -> ReadFailure {
    let msg = resp
        .error_message
        .clone()
        .unwrap_or_else(|| PklString::from("Custom resource reader callback failed."));
    match resp.error_kind.as_deref() {
        Some("security") => ReadFailure::SecurityManagerException { message: Some(msg) },
        Some("io") => ReadFailure::Io(IoFailure::from(msg.to_java_utf8())),
        _ => ReadFailure::EvalError {
            message: msg,
            hint: None,
        },
    }
}

fn read_failure_to_module_host(err: ReadFailure) -> ModuleHostError {
    match err {
        ReadFailure::SecurityManagerException { message } => {
            ModuleHostError::SecurityManagerException { message }
        }
        ReadFailure::Io(io) => ModuleHostError::Io(io),
        ReadFailure::EvalError { message, hint } => ModuleHostError::EvalError { message, hint },
        other => ModuleHostError::EvalError {
            message: PklString::from(format!("{other:?}")),
            hint: None,
        },
    }
}
