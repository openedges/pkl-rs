//! Wasm-pure external reader transport abstraction, in-memory direct transport,
//! protocol state machine (`ExternalReaderProcessHandle`, `ExternalModuleResolver`,
//! `ExternalResourceResolver`), and Host dispatch registry (`ExternalReaderRegistry`).
//!
//! Pure `wasm32-unknown-unknown` compatible — zero `std::process`, zero filesystem
//! I/O, zero `unsafe`. Native subprocess spawning lives in [`super::process`] behind
//! `#[cfg(not(target_arch = "wasm32"))]`.

use super::codec;
use super::messages::{
    ExternalReaderConfig, ExternalReaderError, ExternalReaderMessage, ModuleReaderSpec,
    ResourceReaderSpec, WirePathElement,
};
use crate::glob::{
    GlobCompletion, GlobHostError, GlobReader, GlobRequest, GlobRequestKind, PathElement,
    ResourceReader,
};
use crate::host::ReadFailure;
use crate::modules::{
    ModuleCapabilities, ModuleCompletion, ModuleHostError, ModuleKey, ModuleKeyKind, ModuleRequest,
    ModuleRequestKind, ModuleSource, ResolvedModule, ResolvedModuleKey,
};
use crate::resources::{ReadResult, Resource};
use crate::value::PklString;
use std::collections::BTreeMap;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicI64, Ordering},
};

const EXTERNAL_HANDLE_PREFIX: &str = "external:";

// ============================================================================
// Transport Trait (shared by Wasm mocks & native subprocess runner)
// ============================================================================

/// Bidirectional MessagePack message transport for an external reader process.
///
/// Implementations own the underlying wire channel (in-memory mock peer on Wasm,
/// or child process stdio pipes on native targets).
pub trait ExternalReaderTransport: Send + Sync {
    /// Send a request message and await its correlated response message.
    fn request(
        &self,
        message: ExternalReaderMessage,
    ) -> Result<ExternalReaderMessage, ExternalReaderError>;

    /// Send a one-way message (such as `CloseExternalProcess`).
    fn send_one_way(&self, message: ExternalReaderMessage) -> Result<(), ExternalReaderError>;

    /// Verify that the underlying process/transport is alive and ready (lazy spawn
    /// on native targets). Matches `ExternalReaderProcessImpl.getTransport()`.
    fn ensure_alive(&self) -> Result<(), ExternalReaderError> {
        if self.is_closed() {
            Err(ExternalReaderError::Closed)
        } else {
            Ok(())
        }
    }

    /// Idempotently close the transport and release any associated resources.
    fn close(&self);

    /// Returns `true` once `close()` has been called.
    fn is_closed(&self) -> bool;

    /// Cancel any in-flight request and terminate the underlying transport/process.
    fn cancel(&self) {
        self.close();
    }
}

// ============================================================================
// Wasm-Pure In-Memory Mock Peer & Wire-Level DirectTransport
// ============================================================================

/// Client-side external module reader trait for in-memory Wasm/unit testing,
/// mirroring `org.pkl.core.externalreader.ExternalModuleReader`.
pub trait MockExternalModuleReader: Send + Sync {
    fn spec(&self) -> ModuleReaderSpec;
    fn read(&self, uri: &str) -> Result<Option<String>, String>;
    fn list_elements(&self, base_uri: &str) -> Result<Option<Vec<WirePathElement>>, String> {
        let _ = base_uri;
        Ok(Some(Vec::new()))
    }
}

/// Client-side external resource reader trait for in-memory Wasm/unit testing,
/// mirroring `org.pkl.core.externalreader.ExternalResourceReader`.
pub trait MockExternalResourceReader: Send + Sync {
    fn spec(&self) -> ResourceReaderSpec;
    fn read(&self, uri: &str) -> Result<Option<Vec<u8>>, String>;
    fn list_elements(&self, base_uri: &str) -> Result<Option<Vec<WirePathElement>>, String> {
        let _ = base_uri;
        Ok(Some(Vec::new()))
    }
}

/// In-memory client implementation mirroring `ExternalReaderClient.kt` that
/// speaks the real MessagePack wire protocol over raw byte slices.
pub struct MockExternalReaderPeer {
    module_readers: Vec<Box<dyn MockExternalModuleReader>>,
    resource_readers: Vec<Box<dyn MockExternalResourceReader>>,
    close_count: AtomicI64,
    wire_log: Mutex<Vec<ExternalReaderMessage>>,
}

impl MockExternalReaderPeer {
    pub fn new(
        module_readers: Vec<Box<dyn MockExternalModuleReader>>,
        resource_readers: Vec<Box<dyn MockExternalResourceReader>>,
    ) -> Self {
        Self {
            module_readers,
            resource_readers,
            close_count: AtomicI64::new(0),
            wire_log: Mutex::new(Vec::new()),
        }
    }

    pub fn close_count(&self) -> i64 {
        self.close_count.load(Ordering::SeqCst)
    }

    pub fn recorded_incoming(&self) -> Vec<ExternalReaderMessage> {
        self.wire_log
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    fn find_module_reader(&self, scheme: &str) -> Option<&dyn MockExternalModuleReader> {
        self.module_readers
            .iter()
            .find(|r| r.spec().scheme.eq_ignore_ascii_case(scheme))
            .map(Box::as_ref)
    }

    fn find_resource_reader(&self, scheme: &str) -> Option<&dyn MockExternalResourceReader> {
        self.resource_readers
            .iter()
            .find(|r| r.spec().scheme.eq_ignore_ascii_case(scheme))
            .map(Box::as_ref)
    }

    /// Process a single raw MessagePack frame from the server and return the
    /// encoded MessagePack response frame (or `None` for one-way messages).
    pub fn handle_wire_frame(
        &self,
        incoming_bytes: &[u8],
    ) -> Result<Option<Vec<u8>>, ExternalReaderError> {
        let Some(msg) = codec::decode_from_slice(incoming_bytes)? else {
            return Err(ExternalReaderError::Io("Unexpected EOF".into()));
        };
        if let Ok(mut log) = self.wire_log.lock() {
            log.push(msg.clone());
        }
        let response = match msg {
            ExternalReaderMessage::CloseExternalProcess => {
                self.close_count.fetch_add(1, Ordering::SeqCst);
                return Ok(None);
            }
            ExternalReaderMessage::InitializeModuleReaderRequest { request_id, scheme } => {
                let spec = self.find_module_reader(&scheme).map(|r| r.spec());
                ExternalReaderMessage::InitializeModuleReaderResponse { request_id, spec }
            }
            ExternalReaderMessage::InitializeResourceReaderRequest { request_id, scheme } => {
                let spec = self.find_resource_reader(&scheme).map(|r| r.spec());
                ExternalReaderMessage::InitializeResourceReaderResponse { request_id, spec }
            }
            ExternalReaderMessage::ReadModuleRequest {
                request_id,
                evaluator_id,
                uri,
            } => {
                let scheme = uri_scheme(&uri).unwrap_or("");
                match self.find_module_reader(scheme) {
                    None => ExternalReaderMessage::ReadModuleResponse {
                        request_id,
                        evaluator_id,
                        contents: None,
                        error: Some(format!("No module reader found for scheme {scheme}")),
                    },
                    Some(reader) => match reader.read(&uri) {
                        Ok(contents) => ExternalReaderMessage::ReadModuleResponse {
                            request_id,
                            evaluator_id,
                            contents,
                            error: None,
                        },
                        Err(err) => ExternalReaderMessage::ReadModuleResponse {
                            request_id,
                            evaluator_id,
                            contents: None,
                            error: Some(err),
                        },
                    },
                }
            }
            ExternalReaderMessage::ReadResourceRequest {
                request_id,
                evaluator_id,
                uri,
            } => {
                let scheme = uri_scheme(&uri).unwrap_or("");
                match self.find_resource_reader(scheme) {
                    None => ExternalReaderMessage::ReadResourceResponse {
                        request_id,
                        evaluator_id,
                        contents: None,
                        error: Some(format!("No resource reader found for scheme {scheme}")),
                    },
                    Some(reader) => match reader.read(&uri) {
                        Ok(contents) => ExternalReaderMessage::ReadResourceResponse {
                            request_id,
                            evaluator_id,
                            contents,
                            error: None,
                        },
                        Err(err) => ExternalReaderMessage::ReadResourceResponse {
                            request_id,
                            evaluator_id,
                            contents: None,
                            error: Some(err),
                        },
                    },
                }
            }
            ExternalReaderMessage::ListModulesRequest {
                request_id,
                evaluator_id,
                uri,
            } => {
                let scheme = uri_scheme(&uri).unwrap_or("");
                match self.find_module_reader(scheme) {
                    None => ExternalReaderMessage::ListModulesResponse {
                        request_id,
                        evaluator_id,
                        path_elements: None,
                        error: Some(format!("No module reader found for scheme {scheme}")),
                    },
                    Some(reader) => match reader.list_elements(&uri) {
                        Ok(path_elements) => ExternalReaderMessage::ListModulesResponse {
                            request_id,
                            evaluator_id,
                            path_elements,
                            error: None,
                        },
                        Err(err) => ExternalReaderMessage::ListModulesResponse {
                            request_id,
                            evaluator_id,
                            path_elements: None,
                            error: Some(err),
                        },
                    },
                }
            }
            ExternalReaderMessage::ListResourcesRequest {
                request_id,
                evaluator_id,
                uri,
            } => {
                let scheme = uri_scheme(&uri).unwrap_or("");
                match self.find_resource_reader(scheme) {
                    None => ExternalReaderMessage::ListResourcesResponse {
                        request_id,
                        evaluator_id,
                        path_elements: None,
                        error: Some(format!("No resource reader found for scheme {scheme}")),
                    },
                    Some(reader) => match reader.list_elements(&uri) {
                        Ok(path_elements) => ExternalReaderMessage::ListResourcesResponse {
                            request_id,
                            evaluator_id,
                            path_elements,
                            error: None,
                        },
                        Err(err) => ExternalReaderMessage::ListResourcesResponse {
                            request_id,
                            evaluator_id,
                            path_elements: None,
                            error: Some(err),
                        },
                    },
                }
            }
            other => {
                return Err(ExternalReaderError::Protocol(format!(
                    "Unexpected incoming request message: {other:?}"
                )));
            }
        };
        Ok(Some(codec::encode_to_vec(&response)?))
    }
}

/// Wasm-pure in-memory transport that exercises the full MessagePack wire
/// encoder, decoder, and request-ID correlation synchronously against a peer.
pub struct DirectTransport {
    peer: Arc<MockExternalReaderPeer>,
    closed: AtomicBool,
    terminated: AtomicBool,
}

impl DirectTransport {
    pub fn new(peer: Arc<MockExternalReaderPeer>) -> Self {
        Self {
            peer,
            closed: AtomicBool::new(false),
            terminated: AtomicBool::new(false),
        }
    }

    /// Simulate unexpected child process death before the next request.
    pub fn mark_terminated(&self) {
        self.terminated.store(true, Ordering::SeqCst);
    }

    pub fn peer(&self) -> &Arc<MockExternalReaderPeer> {
        &self.peer
    }
}

impl ExternalReaderTransport for DirectTransport {
    fn ensure_alive(&self) -> Result<(), ExternalReaderError> {
        if self.closed.load(Ordering::SeqCst) {
            return Err(ExternalReaderError::Closed);
        }
        if self.terminated.load(Ordering::SeqCst) {
            return Err(ExternalReaderError::already_terminated());
        }
        Ok(())
    }

    fn request(
        &self,
        message: ExternalReaderMessage,
    ) -> Result<ExternalReaderMessage, ExternalReaderError> {
        self.ensure_alive()?;
        let expected_id = message.request_id().ok_or_else(|| {
            ExternalReaderError::Protocol("Expected request message with requestId".into())
        })?;
        let wire_out = codec::encode_to_vec(&message)?;
        let Some(wire_in) = self.peer.handle_wire_frame(&wire_out)? else {
            return Err(ExternalReaderError::Protocol(
                "Peer returned no response for request".into(),
            ));
        };
        let Some(response) = codec::decode_from_slice(&wire_in)? else {
            return Err(ExternalReaderError::Io("Unexpected EOF".into()));
        };
        if !response.is_response() {
            return Err(ExternalReaderError::Protocol(format!(
                "Unexpected incoming message: {response:?}"
            )));
        }
        if response.request_id() != Some(expected_id) {
            return Err(ExternalReaderError::unknown_request_id(
                response.java_simple_name(),
                response.request_id().unwrap_or(0),
            ));
        }
        Ok(response)
    }

    fn send_one_way(&self, message: ExternalReaderMessage) -> Result<(), ExternalReaderError> {
        let wire_out = codec::encode_to_vec(&message)?;
        let _ = self.peer.handle_wire_frame(&wire_out)?;
        Ok(())
    }

    fn close(&self) {
        if self
            .closed
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
            && !self.terminated.load(Ordering::SeqCst)
        {
            let _ = self.send_one_way(ExternalReaderMessage::CloseExternalProcess);
        }
    }

    fn is_closed(&self) -> bool {
        self.closed.load(Ordering::SeqCst)
    }
}

/// Raw-byte handler transport for testing malformed/mismatched peer frames
/// (e.g. wrong `requestId`, wrong response message type, explicit `nil` fields)
/// in Wasm-pure tests.
pub struct RawFrameTransport<F>
where
    F: Fn(&[u8]) -> Result<Vec<u8>, ExternalReaderError> + Send + Sync,
{
    handler: F,
    closed: AtomicBool,
}

impl<F> RawFrameTransport<F>
where
    F: Fn(&[u8]) -> Result<Vec<u8>, ExternalReaderError> + Send + Sync,
{
    pub fn new(handler: F) -> Self {
        Self {
            handler,
            closed: AtomicBool::new(false),
        }
    }
}

impl<F> ExternalReaderTransport for RawFrameTransport<F>
where
    F: Fn(&[u8]) -> Result<Vec<u8>, ExternalReaderError> + Send + Sync,
{
    fn request(
        &self,
        message: ExternalReaderMessage,
    ) -> Result<ExternalReaderMessage, ExternalReaderError> {
        if self.closed.load(Ordering::SeqCst) {
            return Err(ExternalReaderError::Closed);
        }
        let expected_id = message.request_id().ok_or_else(|| {
            ExternalReaderError::Protocol("Expected request message with requestId".into())
        })?;
        let wire_out = codec::encode_to_vec(&message)?;
        let wire_in = (self.handler)(&wire_out)?;
        let Some(response) = codec::decode_from_slice(&wire_in)? else {
            return Err(ExternalReaderError::Io("Unexpected EOF".into()));
        };
        if !response.is_response() {
            return Err(ExternalReaderError::Protocol(format!(
                "Unexpected incoming message: {response:?}"
            )));
        }
        if response.request_id() != Some(expected_id) {
            return Err(ExternalReaderError::unknown_request_id(
                response.java_simple_name(),
                response.request_id().unwrap_or(0),
            ));
        }
        Ok(response)
    }

    fn send_one_way(&self, message: ExternalReaderMessage) -> Result<(), ExternalReaderError> {
        let wire_out = codec::encode_to_vec(&message)?;
        let _ = (self.handler)(&wire_out);
        Ok(())
    }

    fn close(&self) {
        self.closed.store(true, Ordering::SeqCst);
    }

    fn is_closed(&self) -> bool {
        self.closed.load(Ordering::SeqCst)
    }
}

// ============================================================================
// Process Handle & Per-Evaluator Resolvers (mirrors upstream pkl-core)
// ============================================================================

/// Shared process handle mirroring `org.pkl.core.externalreader.ExternalReaderProcessImpl`.
///
/// Caches `InitializeModuleReaderResponse` and `InitializeResourceReaderResponse`
/// per registered URI scheme across evaluators sharing the same process.
pub struct ExternalReaderProcessHandle {
    transport: Arc<dyn ExternalReaderTransport>,
    next_request_id: Arc<AtomicI64>,
    module_specs: Mutex<BTreeMap<String, Result<Option<ModuleReaderSpec>, ExternalReaderError>>>,
    resource_specs:
        Mutex<BTreeMap<String, Result<Option<ResourceReaderSpec>, ExternalReaderError>>>,
}

impl ExternalReaderProcessHandle {
    pub fn new(transport: Arc<dyn ExternalReaderTransport>) -> Self {
        Self {
            transport,
            next_request_id: Arc::new(AtomicI64::new(1)),
            module_specs: Mutex::new(BTreeMap::new()),
            resource_specs: Mutex::new(BTreeMap::new()),
        }
    }

    fn next_id(&self) -> i64 {
        self.next_request_id.fetch_add(1, Ordering::Relaxed)
    }

    /// Mirrors `ExternalReaderProcess.getModuleReaderSpec(String uriScheme)`.
    ///
    /// Lazily sends `InitializeModuleReaderRequest` (`0x2e`) once per scheme and
    /// caches the result (including failures, matching `computeIfAbsent` on `Future`).
    pub fn get_module_reader_spec(
        &self,
        scheme: &str,
    ) -> Result<Option<ModuleReaderSpec>, ExternalReaderError> {
        if let Some(cached) = self
            .module_specs
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(scheme)
            .cloned()
        {
            return cached;
        }
        let req = ExternalReaderMessage::InitializeModuleReaderRequest {
            request_id: self.next_id(),
            scheme: scheme.to_string(),
        };
        let outcome = match self.transport.request(req) {
            Ok(ExternalReaderMessage::InitializeModuleReaderResponse { spec, .. }) => Ok(spec),
            Ok(_) => Err(ExternalReaderError::Protocol("unexpected response".into())
                .into_resolved_future_io()),
            Err(err) => Err(err.into_resolved_future_io()),
        };
        let mut guard = self.module_specs.lock().unwrap_or_else(|e| e.into_inner());
        guard
            .entry(scheme.to_string())
            .or_insert_with(|| outcome.clone());
        outcome
    }

    /// Mirrors `ExternalReaderProcess.getResourceReaderSpec(String uriScheme)`.
    ///
    /// Lazily sends `InitializeResourceReaderRequest` (`0x30`) once per scheme and
    /// caches the result.
    pub fn get_resource_reader_spec(
        &self,
        scheme: &str,
    ) -> Result<Option<ResourceReaderSpec>, ExternalReaderError> {
        if let Some(cached) = self
            .resource_specs
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(scheme)
            .cloned()
        {
            return cached;
        }
        let req = ExternalReaderMessage::InitializeResourceReaderRequest {
            request_id: self.next_id(),
            scheme: scheme.to_string(),
        };
        let outcome = match self.transport.request(req) {
            Ok(ExternalReaderMessage::InitializeResourceReaderResponse { spec, .. }) => Ok(spec),
            Ok(_) => Err(ExternalReaderError::Protocol("unexpected response".into())
                .into_resolved_future_io()),
            Err(err) => Err(err.into_resolved_future_io()),
        };
        let mut guard = self
            .resource_specs
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        guard
            .entry(scheme.to_string())
            .or_insert_with(|| outcome.clone());
        outcome
    }

    /// Mirrors `ExternalReaderProcess.getModuleResolver(long evaluatorId)`.
    pub fn module_resolver(
        &self,
        evaluator_id: i64,
    ) -> Result<ExternalModuleResolver, ExternalReaderError> {
        self.transport.ensure_alive()?;
        Ok(ExternalModuleResolver::with_request_id_counter(
            Arc::clone(&self.transport),
            evaluator_id,
            Arc::clone(&self.next_request_id),
        ))
    }

    /// Mirrors `ExternalReaderProcess.getResourceResolver(long evaluatorId)`.
    pub fn resource_resolver(
        &self,
        evaluator_id: i64,
    ) -> Result<ExternalResourceResolver, ExternalReaderError> {
        self.transport.ensure_alive()?;
        Ok(ExternalResourceResolver::with_request_id_counter(
            Arc::clone(&self.transport),
            evaluator_id,
            Arc::clone(&self.next_request_id),
        ))
    }

    /// Idempotently closes the underlying transport/process.
    pub fn close(&self) {
        self.transport.close();
    }

    /// Cancels in-flight requests and closes the underlying transport/process.
    pub fn cancel(&self) {
        self.transport.cancel();
    }

    pub fn is_closed(&self) -> bool {
        self.transport.is_closed()
    }
}

/// Mirrors `org.pkl.core.externalreader.ExternalModuleResolverImpl`.
///
/// Caches `ReadModuleResponse` and `ListModulesResponse` per URI for the evaluator.
pub struct ExternalModuleResolver {
    transport: Arc<dyn ExternalReaderTransport>,
    evaluator_id: i64,
    next_request_id: Arc<AtomicI64>,
    read_cache: Mutex<BTreeMap<String, Result<String, ExternalReaderError>>>,
    list_cache: Mutex<BTreeMap<String, Result<Vec<WirePathElement>, ExternalReaderError>>>,
}

impl ExternalModuleResolver {
    pub fn new(transport: Arc<dyn ExternalReaderTransport>, evaluator_id: i64) -> Self {
        Self::with_request_id_counter(transport, evaluator_id, Arc::new(AtomicI64::new(1)))
    }

    /// Construct a module resolver sharing a process-wide request-ID counter.
    pub fn with_request_id_counter(
        transport: Arc<dyn ExternalReaderTransport>,
        evaluator_id: i64,
        next_request_id: Arc<AtomicI64>,
    ) -> Self {
        Self {
            transport,
            evaluator_id,
            next_request_id,
            read_cache: Mutex::new(BTreeMap::new()),
            list_cache: Mutex::new(BTreeMap::new()),
        }
    }

    fn next_id(&self) -> i64 {
        self.next_request_id.fetch_add(1, Ordering::Relaxed)
    }

    /// Mirrors `ExternalModuleResolverImpl.doReadModule(URI)`.
    ///
    /// - If `error != null` → `Err(Io(error))` (error wins even if `contents` is set).
    /// - Else if `contents != null` → `Ok(contents)`.
    /// - Else → `Ok("")` (empty string default, `ExternalModuleResolverImpl.java:89-91`).
    pub fn resolve_module(&self, uri: &str) -> Result<String, ExternalReaderError> {
        if let Some(cached) = self
            .read_cache
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(uri)
            .cloned()
        {
            return cached;
        }
        let req = ExternalReaderMessage::ReadModuleRequest {
            request_id: self.next_id(),
            evaluator_id: self.evaluator_id,
            uri: uri.to_string(),
        };
        let outcome = match self.transport.request(req) {
            Ok(ExternalReaderMessage::ReadModuleResponse {
                contents, error, ..
            }) => {
                if let Some(err) = error {
                    Err(ExternalReaderError::Io(err))
                } else {
                    Ok(contents.unwrap_or_default())
                }
            }
            Ok(_) => Err(ExternalReaderError::Protocol("unexpected response".into())
                .into_resolved_future_io()),
            Err(err) => Err(err.into_resolved_future_io()),
        };
        let mut guard = self.read_cache.lock().unwrap_or_else(|e| e.into_inner());
        guard
            .entry(uri.to_string())
            .or_insert_with(|| outcome.clone());
        outcome
    }

    /// Mirrors `ExternalModuleResolverImpl.hasElement` (`:56-65`): attempts to read
    /// the module and returns `true` on success or `false` on `IOException`.
    pub fn has_element(&self, uri: &str) -> bool {
        self.resolve_module(uri).is_ok()
    }

    /// Mirrors `ExternalModuleResolverImpl.doListElements(URI)` (`:103-130`).
    ///
    /// Absent `pathElements` (with no error) defaults to an empty list.
    pub fn list_elements(
        &self,
        base_uri: &str,
    ) -> Result<Vec<WirePathElement>, ExternalReaderError> {
        if let Some(cached) = self
            .list_cache
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(base_uri)
            .cloned()
        {
            return cached;
        }
        let req = ExternalReaderMessage::ListModulesRequest {
            request_id: self.next_id(),
            evaluator_id: self.evaluator_id,
            uri: base_uri.to_string(),
        };
        let outcome = match self.transport.request(req) {
            Ok(ExternalReaderMessage::ListModulesResponse {
                path_elements,
                error,
                ..
            }) => {
                if let Some(err) = error {
                    Err(ExternalReaderError::Io(err))
                } else {
                    Ok(path_elements.unwrap_or_default())
                }
            }
            Ok(_) => Err(ExternalReaderError::Protocol("unexpected response".into())
                .into_resolved_future_io()),
            Err(err) => Err(err.into_resolved_future_io()),
        };
        let mut guard = self.list_cache.lock().unwrap_or_else(|e| e.into_inner());
        guard
            .entry(base_uri.to_string())
            .or_insert_with(|| outcome.clone());
        outcome
    }
}

type ResourceReadCache = BTreeMap<String, Result<Option<Vec<u8>>, ExternalReaderError>>;
type PathListCache = BTreeMap<String, Result<Vec<WirePathElement>, ExternalReaderError>>;

/// Mirrors `org.pkl.core.externalreader.ExternalResourceResolverImpl`.
///
/// Caches `ReadResourceResponse` and `ListResourcesResponse` per URI for the evaluator.
pub struct ExternalResourceResolver {
    transport: Arc<dyn ExternalReaderTransport>,
    evaluator_id: i64,
    next_request_id: Arc<AtomicI64>,
    read_cache: Mutex<ResourceReadCache>,
    list_cache: Mutex<PathListCache>,
}

impl ExternalResourceResolver {
    pub fn new(transport: Arc<dyn ExternalReaderTransport>, evaluator_id: i64) -> Self {
        Self::with_request_id_counter(transport, evaluator_id, Arc::new(AtomicI64::new(1)))
    }

    /// Construct a resource resolver sharing a process-wide request-ID counter.
    pub fn with_request_id_counter(
        transport: Arc<dyn ExternalReaderTransport>,
        evaluator_id: i64,
        next_request_id: Arc<AtomicI64>,
    ) -> Self {
        Self {
            transport,
            evaluator_id,
            next_request_id,
            read_cache: Mutex::new(BTreeMap::new()),
            list_cache: Mutex::new(BTreeMap::new()),
        }
    }

    fn next_id(&self) -> i64 {
        self.next_request_id.fetch_add(1, Ordering::Relaxed)
    }

    /// Mirrors `ExternalResourceResolverImpl.doRead(URI)` (`:50-53, 101-128`).
    ///
    /// - If `error != null` → `Err(Io(error))`.
    /// - Else if `contents == None` → `Ok(None)` (**missing resource**, matching
    ///   pinned Pkl 0.32.0 Java behavior `result == null ? Optional.empty() : ...`).
    /// - Else → `Ok(Some(bytes))`.
    pub fn read(&self, uri: &str) -> Result<Option<Vec<u8>>, ExternalReaderError> {
        if let Some(cached) = self
            .read_cache
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(uri)
            .cloned()
        {
            return cached;
        }
        let req = ExternalReaderMessage::ReadResourceRequest {
            request_id: self.next_id(),
            evaluator_id: self.evaluator_id,
            uri: uri.to_string(),
        };
        let outcome = match self.transport.request(req) {
            Ok(ExternalReaderMessage::ReadResourceResponse {
                contents, error, ..
            }) => {
                if let Some(err) = error {
                    Err(ExternalReaderError::Io(err))
                } else {
                    Ok(contents)
                }
            }
            Ok(_) => Err(ExternalReaderError::Protocol("unexpected response".into())
                .into_resolved_future_io()),
            Err(err) => Err(err.into_resolved_future_io()),
        };
        let mut guard = self.read_cache.lock().unwrap_or_else(|e| e.into_inner());
        guard
            .entry(uri.to_string())
            .or_insert_with(|| outcome.clone());
        outcome
    }

    /// Mirrors `ExternalResourceResolverImpl.hasElement` (`:55-63`):
    /// returns `true` iff `doRead(uri)` returns `Ok(Some(_))`.
    pub fn has_element(&self, uri: &str) -> bool {
        matches!(self.read(uri), Ok(Some(_)))
    }

    /// Mirrors `ExternalResourceResolverImpl.doListElements(URI)` (`:71-99`).
    ///
    /// Absent `pathElements` (with no error) defaults to an empty list.
    pub fn list_elements(
        &self,
        base_uri: &str,
    ) -> Result<Vec<WirePathElement>, ExternalReaderError> {
        if let Some(cached) = self
            .list_cache
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(base_uri)
            .cloned()
        {
            return cached;
        }
        let req = ExternalReaderMessage::ListResourcesRequest {
            request_id: self.next_id(),
            evaluator_id: self.evaluator_id,
            uri: base_uri.to_string(),
        };
        let outcome = match self.transport.request(req) {
            Ok(ExternalReaderMessage::ListResourcesResponse {
                path_elements,
                error,
                ..
            }) => {
                if let Some(err) = error {
                    Err(ExternalReaderError::Io(err))
                } else {
                    Ok(path_elements.unwrap_or_default())
                }
            }
            Ok(_) => Err(ExternalReaderError::Protocol("unexpected response".into())
                .into_resolved_future_io()),
            Err(err) => Err(err.into_resolved_future_io()),
        };
        let mut guard = self.list_cache.lock().unwrap_or_else(|e| e.into_inner());
        guard
            .entry(base_uri.to_string())
            .or_insert_with(|| outcome.clone());
        outcome
    }
}

// ============================================================================
// ExternalReaderRegistry: Process Deduplication + HostSession Dispatch
// ============================================================================

struct RegisteredModuleReader {
    scheme: String,
    process: Arc<ExternalReaderProcessHandle>,
    resolver: Mutex<Option<Arc<ExternalModuleResolver>>>,
}

struct RegisteredResourceReader {
    scheme: String,
    process: Arc<ExternalReaderProcessHandle>,
    resolver: Mutex<Option<Arc<ExternalResourceResolver>>>,
}

/// Manages configured external module and resource readers for an evaluator.
///
/// - Deduplicates `ExternalReaderProcessHandle` instances by exact
///   `ExternalReaderConfig` equality across both module and resource readers
///   (`CliCommand.kt:208-210`, `EvaluatorBuilder.java:530-546`).
/// - Module schemes match **case-insensitively** (`ModuleKeyFactories.java:289`).
/// - Resource schemes match **case-sensitively**, last registration wins
///   (`ResourceManager.java:49-51, 141`).
/// - Closes all underlying processes idempotently on `close_all()` and `Drop`.
pub struct ExternalReaderRegistry {
    evaluator_id: i64,
    processes: BTreeMap<ExternalReaderConfig, Arc<ExternalReaderProcessHandle>>,
    module_readers: Vec<RegisteredModuleReader>,
    resource_readers: BTreeMap<String, RegisteredResourceReader>,
}

impl Default for ExternalReaderRegistry {
    fn default() -> Self {
        Self::new(0)
    }
}

impl ExternalReaderRegistry {
    pub fn new(evaluator_id: i64) -> Self {
        Self {
            evaluator_id,
            processes: BTreeMap::new(),
            module_readers: Vec::new(),
            resource_readers: BTreeMap::new(),
        }
    }

    pub fn evaluator_id(&self) -> i64 {
        self.evaluator_id
    }

    /// Number of unique underlying external reader process handles (after deduplication).
    pub fn process_count(&self) -> usize {
        self.processes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.module_readers.is_empty() && self.resource_readers.is_empty()
    }

    /// Registered module reader schemes (in registration order), for auto-populating
    /// default `allowedModules` patterns (`Pattern.quote("$scheme:")`).
    pub fn module_schemes(&self) -> Vec<&str> {
        self.module_readers
            .iter()
            .map(|r| r.scheme.as_str())
            .collect()
    }

    /// Registered resource reader schemes, for auto-populating default
    /// `allowedResources` patterns (`Pattern.quote("$scheme:")`).
    pub fn resource_schemes(&self) -> Vec<&str> {
        self.resource_readers.keys().map(String::as_str).collect()
    }

    /// Register a module reader with an explicit transport factory (Wasm-pure).
    ///
    /// If `config` was already registered (for another scheme or for a resource
    /// reader), the existing `ExternalReaderProcessHandle` is reused and
    /// `make_transport` is NOT called.
    pub fn register_module_reader_with<F>(
        &mut self,
        scheme: impl Into<String>,
        config: ExternalReaderConfig,
        make_transport: F,
    ) where
        F: FnOnce(&ExternalReaderConfig) -> Arc<dyn ExternalReaderTransport>,
    {
        let scheme = scheme.into();
        let process = self.get_or_insert_process(config, make_transport);
        self.module_readers
            .retain(|r| !r.scheme.eq_ignore_ascii_case(&scheme));
        self.module_readers.push(RegisteredModuleReader {
            scheme,
            process,
            resolver: Mutex::new(None),
        });
    }

    /// Register a resource reader with an explicit transport factory (Wasm-pure).
    ///
    /// If `config` was already registered (for another scheme or for a module
    /// reader), the existing `ExternalReaderProcessHandle` is reused and
    /// `make_transport` is NOT called.
    pub fn register_resource_reader_with<F>(
        &mut self,
        scheme: impl Into<String>,
        config: ExternalReaderConfig,
        make_transport: F,
    ) where
        F: FnOnce(&ExternalReaderConfig) -> Arc<dyn ExternalReaderTransport>,
    {
        let scheme = scheme.into();
        let process = self.get_or_insert_process(config, make_transport);
        self.resource_readers.insert(
            scheme.clone(),
            RegisteredResourceReader {
                scheme,
                process,
                resolver: Mutex::new(None),
            },
        );
    }

    /// Register an external module reader backed by a native OS child process.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn register_module_reader(
        &mut self,
        scheme: impl Into<String>,
        config: ExternalReaderConfig,
    ) {
        self.register_module_reader_with(scheme, config, |cfg| {
            Arc::new(super::process::NativeExternalReaderProcess::new(
                cfg.clone(),
            ))
        });
    }

    /// Register an external resource reader backed by a native OS child process.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn register_resource_reader(
        &mut self,
        scheme: impl Into<String>,
        config: ExternalReaderConfig,
    ) {
        self.register_resource_reader_with(scheme, config, |cfg| {
            Arc::new(super::process::NativeExternalReaderProcess::new(
                cfg.clone(),
            ))
        });
    }

    fn get_or_insert_process<F>(
        &mut self,
        config: ExternalReaderConfig,
        make_transport: F,
    ) -> Arc<ExternalReaderProcessHandle>
    where
        F: FnOnce(&ExternalReaderConfig) -> Arc<dyn ExternalReaderTransport>,
    {
        if let Some(existing) = self.processes.get(&config) {
            return Arc::clone(existing);
        }
        let transport = make_transport(&config);
        let handle = Arc::new(ExternalReaderProcessHandle::new(transport));
        self.processes.insert(config, Arc::clone(&handle));
        handle
    }

    fn find_module_reader_by_scheme(&self, scheme: &str) -> Option<&RegisteredModuleReader> {
        self.module_readers
            .iter()
            .find(|r| r.scheme.eq_ignore_ascii_case(scheme))
    }

    fn find_module_reader_for_handle(&self, handle: &PklString) -> Option<&RegisteredModuleReader> {
        let s = handle.as_str()?;
        let scheme = s.strip_prefix(EXTERNAL_HANDLE_PREFIX)?;
        self.find_module_reader_by_scheme(scheme)
    }

    fn find_resource_reader_for_handle(
        &self,
        handle: &PklString,
    ) -> Option<&RegisteredResourceReader> {
        let s = handle.as_str()?;
        let scheme = s.strip_prefix(EXTERNAL_HANDLE_PREFIX)?;
        self.resource_readers.get(scheme)
    }

    fn module_resolver_for(
        &self,
        reg: &RegisteredModuleReader,
    ) -> Result<Arc<ExternalModuleResolver>, ExternalReaderError> {
        let mut guard = reg.resolver.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(existing) = guard.as_ref() {
            return Ok(Arc::clone(existing));
        }
        let created = Arc::new(reg.process.module_resolver(self.evaluator_id)?);
        *guard = Some(Arc::clone(&created));
        Ok(created)
    }

    fn resource_resolver_for(
        &self,
        reg: &RegisteredResourceReader,
    ) -> Result<Arc<ExternalResourceResolver>, ExternalReaderError> {
        let mut guard = reg.resolver.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(existing) = guard.as_ref() {
            return Ok(Arc::clone(existing));
        }
        let created = Arc::new(reg.process.resource_resolver(self.evaluator_id)?);
        *guard = Some(Arc::clone(&created));
        Ok(created)
    }

    fn require_module_spec(
        reg: &RegisteredModuleReader,
    ) -> Result<ModuleReaderSpec, ExternalReaderError> {
        match reg.process.get_module_reader_spec(&reg.scheme)? {
            Some(spec) => Ok(spec),
            None => Err(ExternalReaderError::does_not_support_scheme(
                "module",
                &reg.scheme,
            )),
        }
    }

    fn require_resource_spec(
        reg: &RegisteredResourceReader,
    ) -> Result<ResourceReaderSpec, ExternalReaderError> {
        match reg.process.get_resource_reader_spec(&reg.scheme)? {
            Some(spec) => Ok(spec),
            None => Err(ExternalReaderError::does_not_support_scheme(
                "resource",
                &reg.scheme,
            )),
        }
    }

    /// Attempt to complete a `HostPoll::Module` request if it targets a registered
    /// external module reader.
    ///
    /// Callers MUST invoke this **before** built-in module factories (`file:`, `http:`,
    /// `genericUrl`) to match `CliCommand.kt:264-275`.
    pub fn try_complete_module<F>(
        &self,
        request: &ModuleRequest,
        check_resolve_module: F,
    ) -> Option<ModuleCompletion>
    where
        F: Fn(&PklString) -> Result<(), ModuleHostError>,
    {
        match &request.kind {
            ModuleRequestKind::SelectKey { id, uri, .. } => {
                let scheme = uri.as_str().and_then(uri_scheme)?;
                let reg = self.find_module_reader_by_scheme(scheme)?;
                let result = (|| -> Result<ModuleKey, ModuleHostError> {
                    let _spec = Self::require_module_spec(reg)
                        .map_err(ExternalReaderError::into_module_host_error)?;
                    let _resolver = self
                        .module_resolver_for(reg)
                        .map_err(ExternalReaderError::into_module_host_error)?;
                    Ok(ModuleKey {
                        id: *id,
                        uri: uri.clone(),
                        kind: ModuleKeyKind::Host,
                        handle: PklString::from(format!("{EXTERNAL_HANDLE_PREFIX}{}", reg.scheme)),
                    })
                })();
                Some(ModuleCompletion::KeySelected(result))
            }
            ModuleRequestKind::QueryCachePolicy { key }
                if self.find_module_reader_for_handle(&key.handle).is_some() =>
            {
                Some(ModuleCompletion::CachePolicy(Ok(true)))
            }
            ModuleRequestKind::QueryFragmentPaths { key }
                if self.find_module_reader_for_handle(&key.handle).is_some() =>
            {
                Some(ModuleCompletion::FragmentPaths(Ok(false)))
            }
            ModuleRequestKind::Capabilities { key, module } => {
                let reg = key
                    .as_ref()
                    .and_then(|k| self.find_module_reader_for_handle(&k.handle))
                    .or_else(|| {
                        module
                            .original_uri
                            .as_str()
                            .and_then(uri_scheme)
                            .and_then(|s| self.find_module_reader_by_scheme(s))
                    })?;
                let result = Self::require_module_spec(reg)
                    .map(|spec| ModuleCapabilities {
                        local: spec.is_local,
                        hierarchical: spec.has_hierarchical_uris,
                        fragment_paths: false,
                    })
                    .map_err(ExternalReaderError::into_module_host_error);
                Some(ModuleCompletion::Capabilities(result))
            }
            ModuleRequestKind::HasElement {
                key,
                module,
                element_uri,
            } => {
                let reg = key
                    .as_ref()
                    .and_then(|k| self.find_module_reader_for_handle(&k.handle))
                    .or_else(|| {
                        module
                            .original_uri
                            .as_str()
                            .and_then(uri_scheme)
                            .and_then(|s| self.find_module_reader_by_scheme(s))
                    })?;
                let result = (|| -> Result<bool, ModuleHostError> {
                    check_resolve_module(element_uri)?;
                    let resolver = self
                        .module_resolver_for(reg)
                        .map_err(ExternalReaderError::into_module_host_error)?;
                    let uri_str = element_uri.as_str().ok_or_else(|| {
                        ModuleHostError::Unsupported(
                            "External module reader URI must be valid UTF-8.".into(),
                        )
                    })?;
                    Ok(resolver.has_element(uri_str))
                })();
                Some(ModuleCompletion::HasElement(result))
            }
            ModuleRequestKind::Resolve {
                key,
                resolved_key_id,
                ..
            }
            | ModuleRequestKind::ResolveKey {
                key,
                resolved_key_id,
                ..
            } => {
                let reg = self.find_module_reader_for_handle(&key.handle)?;
                let result = (|| -> Result<ResolvedModuleKey, ModuleHostError> {
                    check_resolve_module(&key.uri)?;
                    let resolver = self
                        .module_resolver_for(reg)
                        .map_err(ExternalReaderError::into_module_host_error)?;
                    let uri_str = key.uri.as_str().ok_or_else(|| {
                        ModuleHostError::Unsupported(
                            "External module reader URI must be valid UTF-8.".into(),
                        )
                    })?;
                    // Matches `ModuleKeys.ExternalResolver.resolve()`: executes
                    // `ReadModuleRequest` during resolution and caches the source text.
                    let _contents = resolver
                        .resolve_module(uri_str)
                        .map_err(ExternalReaderError::into_module_host_error)?;
                    Ok(ResolvedModuleKey {
                        id: *resolved_key_id,
                        key: key.clone(),
                        handle: key.handle.clone(),
                        module: ResolvedModule {
                            original_uri: key.uri.clone(),
                            resolved_uri: key.uri.clone(),
                            source_uri: key.uri.clone(),
                        },
                    })
                })();
                Some(ModuleCompletion::KeyResolved(result))
            }
            ModuleRequestKind::LoadSource { key, resolved } => {
                let reg = self.find_module_reader_for_handle(&key.handle)?;
                let result = (|| -> Result<ModuleSource, ModuleHostError> {
                    let resolver = self
                        .module_resolver_for(reg)
                        .map_err(ExternalReaderError::into_module_host_error)?;
                    let uri_str = resolved.resolved_uri.as_str().ok_or_else(|| {
                        ModuleHostError::Unsupported(
                            "External module reader URI must be valid UTF-8.".into(),
                        )
                    })?;
                    let text = resolver
                        .resolve_module(uri_str)
                        .map_err(ExternalReaderError::into_module_host_error)?;
                    let inferred_name =
                        crate::host::resources::infer_module_name(&resolved.original_uri, false)
                            .map_err(|e| ModuleHostError::Unsupported(e.message))?;
                    Ok(ModuleSource {
                        text: text.into(),
                        inferred_name,
                    })
                })();
                Some(ModuleCompletion::Source(result))
            }
            _ => None,
        }
    }

    /// Attempt to complete a `HostPoll::Read` request if `resolved_uri` targets a
    /// registered external resource reader.
    ///
    /// Performs `check_read_resource(resolved_uri)` BEFORE initializing or contacting
    /// the external reader (`ResourceManager.java:108-119`).
    pub fn try_complete_read<F>(
        &self,
        resolved_uri: &PklString,
        check_read_resource: F,
    ) -> Option<Result<ReadResult, ReadFailure>>
    where
        F: Fn(&PklString) -> Result<(), ReadFailure>,
    {
        let uri_str = resolved_uri.as_str()?;
        let scheme = uri_scheme(uri_str)?;
        let reg = self.resource_readers.get(scheme)?;
        Some((|| -> Result<ReadResult, ReadFailure> {
            check_read_resource(resolved_uri)?;
            let _spec =
                Self::require_resource_spec(reg).map_err(ExternalReaderError::into_read_failure)?;
            let resolver = self
                .resource_resolver_for(reg)
                .map_err(ExternalReaderError::into_read_failure)?;
            match resolver
                .read(uri_str)
                .map_err(ExternalReaderError::into_read_failure)?
            {
                Some(bytes) => Ok(ReadResult::Resource(Resource {
                    uri: resolved_uri.clone(),
                    bytes: bytes.into(),
                })),
                None => Ok(ReadResult::Missing),
            }
        })())
    }

    /// Attempt to complete a `HostPoll::Glob` request if it targets a registered
    /// external module or resource reader.
    pub fn try_complete_glob<FM, FR>(
        &self,
        request: &GlobRequest,
        check_resolve_module: FM,
        check_resolve_resource: FR,
    ) -> Option<GlobCompletion>
    where
        FM: Fn(&PklString) -> Result<(), ModuleHostError>,
        FR: Fn(&PklString) -> Result<(), ModuleHostError>,
    {
        match &request.kind {
            GlobRequestKind::SelectResource { id, uri } => {
                let scheme = uri.as_str().and_then(uri_scheme)?;
                let reg = self.resource_readers.get(scheme)?;
                // Selection does NOT eagerly spawn the process (`ResourceReaders.java:687`).
                Some(GlobCompletion::Resource(Ok(ResourceReader {
                    id: *id,
                    scheme: PklString::from(reg.scheme.as_str()),
                    handle: PklString::from(format!("{EXTERNAL_HANDLE_PREFIX}{}", reg.scheme)),
                })))
            }
            GlobRequestKind::IsGlobbable {
                reader: GlobReader::Module(key),
            } => {
                let reg = self.find_module_reader_for_handle(&key.handle)?;
                let result = Self::require_module_spec(reg)
                    .map(|spec| spec.is_globbable)
                    .map_err(|e| GlobHostError::Failure(e.into_module_host_error()));
                Some(GlobCompletion::Globbable(result))
            }
            GlobRequestKind::IsGlobbable {
                reader: GlobReader::Resource(reader),
            } => {
                let reg = self.find_resource_reader_for_handle(&reader.handle)?;
                let result = Self::require_resource_spec(reg)
                    .map(|spec| spec.is_globbable)
                    .map_err(|e| GlobHostError::Failure(e.into_module_host_error()));
                Some(GlobCompletion::Globbable(result))
            }
            GlobRequestKind::IsHierarchical {
                reader: GlobReader::Module(key),
            } => {
                let reg = self.find_module_reader_for_handle(&key.handle)?;
                let result = Self::require_module_spec(reg)
                    .map(|spec| spec.has_hierarchical_uris)
                    .map_err(|e| GlobHostError::Failure(e.into_module_host_error()));
                Some(GlobCompletion::Hierarchical(result))
            }
            GlobRequestKind::IsHierarchical {
                reader: GlobReader::Resource(reader),
            } => {
                let reg = self.find_resource_reader_for_handle(&reader.handle)?;
                let result = Self::require_resource_spec(reg)
                    .map(|spec| spec.has_hierarchical_uris)
                    .map_err(|e| GlobHostError::Failure(e.into_module_host_error()));
                Some(GlobCompletion::Hierarchical(result))
            }
            GlobRequestKind::FragmentPaths { reader }
                if self
                    .find_resource_reader_for_handle(&reader.handle)
                    .is_some() =>
            {
                Some(GlobCompletion::FragmentPaths(Ok(false)))
            }
            GlobRequestKind::HasElement { reader, uri } => {
                let reg = self.find_resource_reader_for_handle(&reader.handle)?;
                let result = (|| -> Result<bool, GlobHostError> {
                    check_resolve_resource(uri).map_err(GlobHostError::Failure)?;
                    let _spec = Self::require_resource_spec(reg)
                        .map_err(|e| GlobHostError::Failure(e.into_module_host_error()))?;
                    let resolver = self
                        .resource_resolver_for(reg)
                        .map_err(|e| GlobHostError::Failure(e.into_module_host_error()))?;
                    let uri_str = uri.as_str().ok_or_else(|| {
                        GlobHostError::Failure(ModuleHostError::Unsupported(
                            "External resource reader URI must be valid UTF-8.".into(),
                        ))
                    })?;
                    Ok(resolver.has_element(uri_str))
                })();
                Some(GlobCompletion::HasElement(result))
            }
            GlobRequestKind::ListElements {
                reader: GlobReader::Module(key),
                uri,
            } => {
                let reg = self.find_module_reader_for_handle(&key.handle)?;
                let result = (|| -> Result<Vec<PathElement>, GlobHostError> {
                    check_resolve_module(uri).map_err(GlobHostError::Failure)?;
                    let resolver = self
                        .module_resolver_for(reg)
                        .map_err(|e| GlobHostError::Failure(e.into_module_host_error()))?;
                    let uri_str = uri.as_str().ok_or_else(|| {
                        GlobHostError::Failure(ModuleHostError::Unsupported(
                            "External module reader URI must be valid UTF-8.".into(),
                        ))
                    })?;
                    let elements = resolver
                        .list_elements(uri_str)
                        .map_err(|e| GlobHostError::Failure(e.into_module_host_error()))?;
                    Ok(elements.into_iter().map(PathElement::from).collect())
                })();
                Some(GlobCompletion::Elements(result))
            }
            GlobRequestKind::ListElements {
                reader: GlobReader::Resource(reader),
                uri,
            } => {
                let reg = self.find_resource_reader_for_handle(&reader.handle)?;
                let result = (|| -> Result<Vec<PathElement>, GlobHostError> {
                    check_resolve_resource(uri).map_err(GlobHostError::Failure)?;
                    let _spec = Self::require_resource_spec(reg)
                        .map_err(|e| GlobHostError::Failure(e.into_module_host_error()))?;
                    let resolver = self
                        .resource_resolver_for(reg)
                        .map_err(|e| GlobHostError::Failure(e.into_module_host_error()))?;
                    let uri_str = uri.as_str().ok_or_else(|| {
                        GlobHostError::Failure(ModuleHostError::Unsupported(
                            "External resource reader URI must be valid UTF-8.".into(),
                        ))
                    })?;
                    let elements = resolver
                        .list_elements(uri_str)
                        .map_err(|e| GlobHostError::Failure(e.into_module_host_error()))?;
                    Ok(elements.into_iter().map(PathElement::from).collect())
                })();
                Some(GlobCompletion::Elements(result))
            }
            _ => None,
        }
    }

    /// Gracefully close every registered external reader process (idempotent).
    pub fn close_all(&self) {
        for process in self.processes.values() {
            process.close();
        }
    }

    /// Cancel in-flight requests and terminate every registered process.
    pub fn cancel_all(&self) {
        for process in self.processes.values() {
            process.cancel();
        }
    }
}

impl Drop for ExternalReaderRegistry {
    fn drop(&mut self) {
        self.close_all();
    }
}

fn uri_scheme(uri: &str) -> Option<&str> {
    let (scheme, _) = uri.split_once(':')?;
    if scheme.is_empty() || scheme.contains(['/', '?', '#']) {
        None
    } else {
        Some(scheme)
    }
}
