//! Wire message types for Pkl 0.32.0 `pkl server` (`0x20`–`0x2d`).
//!
//! Pure `wasm32-unknown-unknown` compatible — zero OS/FS/network dependencies.
//! Reuses `MessageType`, `ModuleReaderSpec`, `ResourceReaderSpec`, `WirePathElement`,
//! `ExternalReaderConfig`, `ExternalReaderError`, and `ExternalReaderMessage` from
//! `crate::external_reader` to avoid duplicating the shared `0x26`–`0x2d` protocol.

use crate::external_reader::{
    ExternalReaderConfig, ExternalReaderError, ExternalReaderMessage, MessageType,
    ModuleReaderSpec, ResourceReaderSpec, WirePathElement,
};
use crate::logging::TraceMode;
use std::collections::BTreeMap;

/// Wire representation of `org.pkl.server.ExternalReader`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerExternalReader {
    pub executable: String,
    pub arguments: Option<Vec<String>>,
    pub working_dir: Option<String>,
}

impl ServerExternalReader {
    pub fn to_external_reader_config(&self) -> ExternalReaderConfig {
        ExternalReaderConfig::new(
            self.executable.clone(),
            self.arguments.clone().unwrap_or_default(),
            self.working_dir.clone(),
        )
    }
}

/// Wire representation of `org.pkl.server.Proxy`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerProxy {
    pub address: Option<String>,
    pub no_proxy: Option<Vec<String>>,
}

/// Wire representation of `org.pkl.server.Http`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerHttp {
    /// PEM-format CA certificates as raw bytes.
    pub ca_certificates: Option<Vec<u8>>,
    /// Proxy settings.
    pub proxy: Option<ServerProxy>,
    /// HTTP URL prefix rewrites (`from -> to`).
    pub rewrites: Option<BTreeMap<String, String>>,
    /// HTTP headers (`url_glob_pattern -> (header_name -> header_values)`).
    pub headers: Option<BTreeMap<String, BTreeMap<String, Vec<String>>>>,
}

/// Wire representation of `org.pkl.core.packages.Checksums`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerChecksums {
    pub sha256: String,
}

/// Wire representation of `org.pkl.server.Dependency` (`"local"` or `"remote"`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ServerDependency {
    Remote {
        package_uri: String,
        checksums: Option<ServerChecksums>,
    },
    Local(ServerProject),
}

/// Wire representation of `org.pkl.server.Project`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerProject {
    pub project_file_uri: String,
    pub package_uri: Option<String>,
    pub dependencies: BTreeMap<String, ServerDependency>,
}

/// All 14 `pkl server` wire messages (`0x20`–`0x2d`).
///
/// Codes `0x20`–`0x25` are server-specific; codes `0x26`–`0x2d` are shared with
/// external readers and carry client module/resource reader callbacks.
#[derive(Clone, Debug, PartialEq, Eq)]
#[allow(clippy::large_enum_variant)]
pub enum ServerMessage {
    /// `0x20` — Client -> Server request.
    CreateEvaluatorRequest {
        request_id: i64,
        allowed_modules: Option<Vec<String>>,
        allowed_resources: Option<Vec<String>>,
        client_module_readers: Option<Vec<ModuleReaderSpec>>,
        client_resource_readers: Option<Vec<ResourceReaderSpec>>,
        module_paths: Option<Vec<String>>,
        env: Option<BTreeMap<String, String>>,
        properties: Option<BTreeMap<String, String>>,
        timeout_seconds: Option<i64>,
        root_dir: Option<String>,
        cache_dir: Option<String>,
        output_format: Option<String>,
        project: Option<ServerProject>,
        http: Option<ServerHttp>,
        external_module_readers: Option<BTreeMap<String, ServerExternalReader>>,
        external_resource_readers: Option<BTreeMap<String, ServerExternalReader>>,
        trace_mode: Option<TraceMode>,
    },
    /// `0x21` — Server -> Client response.
    CreateEvaluatorResponse {
        request_id: i64,
        evaluator_id: Option<i64>,
        error: Option<String>,
    },
    /// `0x22` — Client -> Server one-way notice.
    CloseEvaluator { evaluator_id: i64 },
    /// `0x23` — Client -> Server request.
    EvaluateRequest {
        request_id: i64,
        evaluator_id: i64,
        module_uri: String,
        module_text: Option<String>,
        expr: Option<String>,
    },
    /// `0x24` — Server -> Client response.
    EvaluateResponse {
        request_id: i64,
        evaluator_id: i64,
        result: Option<Vec<u8>>,
        error: Option<String>,
    },
    /// `0x25` — Server -> Client one-way log event (`level`: `0` = trace, `1` = warn).
    LogMessage {
        evaluator_id: i64,
        level: i32,
        message: String,
        frame_uri: String,
    },
    /// `0x26` — Server -> Client request.
    ReadResourceRequest {
        request_id: i64,
        evaluator_id: i64,
        uri: String,
    },
    /// `0x27` — Client -> Server response.
    ReadResourceResponse {
        request_id: i64,
        evaluator_id: i64,
        contents: Option<Vec<u8>>,
        error: Option<String>,
    },
    /// `0x28` — Server -> Client request.
    ReadModuleRequest {
        request_id: i64,
        evaluator_id: i64,
        uri: String,
    },
    /// `0x29` — Client -> Server response.
    ReadModuleResponse {
        request_id: i64,
        evaluator_id: i64,
        contents: Option<String>,
        error: Option<String>,
    },
    /// `0x2a` — Server -> Client request.
    ListResourcesRequest {
        request_id: i64,
        evaluator_id: i64,
        uri: String,
    },
    /// `0x2b` — Client -> Server response.
    ListResourcesResponse {
        request_id: i64,
        evaluator_id: i64,
        path_elements: Option<Vec<WirePathElement>>,
        error: Option<String>,
    },
    /// `0x2c` — Server -> Client request.
    ListModulesRequest {
        request_id: i64,
        evaluator_id: i64,
        uri: String,
    },
    /// `0x2d` — Client -> Server response.
    ListModulesResponse {
        request_id: i64,
        evaluator_id: i64,
        path_elements: Option<Vec<WirePathElement>>,
        error: Option<String>,
    },
}

impl ServerMessage {
    pub const fn message_type(&self) -> MessageType {
        match self {
            Self::CreateEvaluatorRequest { .. } => MessageType::CreateEvaluatorRequest,
            Self::CreateEvaluatorResponse { .. } => MessageType::CreateEvaluatorResponse,
            Self::CloseEvaluator { .. } => MessageType::CloseEvaluator,
            Self::EvaluateRequest { .. } => MessageType::EvaluateRequest,
            Self::EvaluateResponse { .. } => MessageType::EvaluateResponse,
            Self::LogMessage { .. } => MessageType::LogMessage,
            Self::ReadResourceRequest { .. } => MessageType::ReadResourceRequest,
            Self::ReadResourceResponse { .. } => MessageType::ReadResourceResponse,
            Self::ReadModuleRequest { .. } => MessageType::ReadModuleRequest,
            Self::ReadModuleResponse { .. } => MessageType::ReadModuleResponse,
            Self::ListResourcesRequest { .. } => MessageType::ListResourcesRequest,
            Self::ListResourcesResponse { .. } => MessageType::ListResourcesResponse,
            Self::ListModulesRequest { .. } => MessageType::ListModulesRequest,
            Self::ListModulesResponse { .. } => MessageType::ListModulesResponse,
        }
    }

    pub const fn request_id(&self) -> Option<i64> {
        match self {
            Self::CreateEvaluatorRequest { request_id, .. }
            | Self::CreateEvaluatorResponse { request_id, .. }
            | Self::EvaluateRequest { request_id, .. }
            | Self::EvaluateResponse { request_id, .. }
            | Self::ReadResourceRequest { request_id, .. }
            | Self::ReadResourceResponse { request_id, .. }
            | Self::ReadModuleRequest { request_id, .. }
            | Self::ReadModuleResponse { request_id, .. }
            | Self::ListResourcesRequest { request_id, .. }
            | Self::ListResourcesResponse { request_id, .. }
            | Self::ListModulesRequest { request_id, .. }
            | Self::ListModulesResponse { request_id, .. } => Some(*request_id),
            Self::CloseEvaluator { .. } | Self::LogMessage { .. } => None,
        }
    }

    pub const fn is_request(&self) -> bool {
        matches!(
            self,
            Self::CreateEvaluatorRequest { .. }
                | Self::EvaluateRequest { .. }
                | Self::ReadResourceRequest { .. }
                | Self::ReadModuleRequest { .. }
                | Self::ListResourcesRequest { .. }
                | Self::ListModulesRequest { .. }
        )
    }

    pub const fn is_response(&self) -> bool {
        matches!(
            self,
            Self::CreateEvaluatorResponse { .. }
                | Self::EvaluateResponse { .. }
                | Self::ReadResourceResponse { .. }
                | Self::ReadModuleResponse { .. }
                | Self::ListResourcesResponse { .. }
                | Self::ListModulesResponse { .. }
        )
    }

    pub const fn is_one_way(&self) -> bool {
        matches!(self, Self::CloseEvaluator { .. } | Self::LogMessage { .. })
    }

    /// Exact Java/Kotlin simple class name used in `unknownRequestId` diagnostics.
    pub const fn java_simple_name(&self) -> &'static str {
        match self {
            Self::CreateEvaluatorRequest { .. } => "CreateEvaluatorRequest",
            Self::CreateEvaluatorResponse { .. } => "CreateEvaluatorResponse",
            Self::CloseEvaluator { .. } => "CloseEvaluator",
            Self::EvaluateRequest { .. } => "EvaluateRequest",
            Self::EvaluateResponse { .. } => "EvaluateResponse",
            Self::LogMessage { .. } => "LogMessage",
            Self::ReadResourceRequest { .. } => "ReadResourceRequest",
            Self::ReadResourceResponse { .. } => "ReadResourceResponse",
            Self::ReadModuleRequest { .. } => "ReadModuleRequest",
            Self::ReadModuleResponse { .. } => "ReadModuleResponse",
            Self::ListResourcesRequest { .. } => "ListResourcesRequest",
            Self::ListResourcesResponse { .. } => "ListResourcesResponse",
            Self::ListModulesRequest { .. } => "ListModulesRequest",
            Self::ListModulesResponse { .. } => "ListModulesResponse",
        }
    }

    /// Convert a shared `0x26`–`0x2d` `ExternalReaderMessage` into a `ServerMessage`.
    pub fn try_from_external_reader(
        msg: ExternalReaderMessage,
    ) -> Result<Self, ExternalReaderError> {
        match msg {
            ExternalReaderMessage::ReadResourceRequest {
                request_id,
                evaluator_id,
                uri,
            } => Ok(Self::ReadResourceRequest {
                request_id,
                evaluator_id,
                uri,
            }),
            ExternalReaderMessage::ReadResourceResponse {
                request_id,
                evaluator_id,
                contents,
                error,
            } => Ok(Self::ReadResourceResponse {
                request_id,
                evaluator_id,
                contents,
                error,
            }),
            ExternalReaderMessage::ReadModuleRequest {
                request_id,
                evaluator_id,
                uri,
            } => Ok(Self::ReadModuleRequest {
                request_id,
                evaluator_id,
                uri,
            }),
            ExternalReaderMessage::ReadModuleResponse {
                request_id,
                evaluator_id,
                contents,
                error,
            } => Ok(Self::ReadModuleResponse {
                request_id,
                evaluator_id,
                contents,
                error,
            }),
            ExternalReaderMessage::ListResourcesRequest {
                request_id,
                evaluator_id,
                uri,
            } => Ok(Self::ListResourcesRequest {
                request_id,
                evaluator_id,
                uri,
            }),
            ExternalReaderMessage::ListResourcesResponse {
                request_id,
                evaluator_id,
                path_elements,
                error,
            } => Ok(Self::ListResourcesResponse {
                request_id,
                evaluator_id,
                path_elements,
                error,
            }),
            ExternalReaderMessage::ListModulesRequest {
                request_id,
                evaluator_id,
                uri,
            } => Ok(Self::ListModulesRequest {
                request_id,
                evaluator_id,
                uri,
            }),
            ExternalReaderMessage::ListModulesResponse {
                request_id,
                evaluator_id,
                path_elements,
                error,
            } => Ok(Self::ListModulesResponse {
                request_id,
                evaluator_id,
                path_elements,
                error,
            }),
            other => Err(ExternalReaderError::unhandled_message_type(
                other.message_type(),
            )),
        }
    }

    /// Convert a shared `0x26`–`0x2d` `ServerMessage` into an `ExternalReaderMessage`.
    pub fn try_into_external_reader(self) -> Result<ExternalReaderMessage, ExternalReaderError> {
        match self {
            Self::ReadResourceRequest {
                request_id,
                evaluator_id,
                uri,
            } => Ok(ExternalReaderMessage::ReadResourceRequest {
                request_id,
                evaluator_id,
                uri,
            }),
            Self::ReadResourceResponse {
                request_id,
                evaluator_id,
                contents,
                error,
            } => Ok(ExternalReaderMessage::ReadResourceResponse {
                request_id,
                evaluator_id,
                contents,
                error,
            }),
            Self::ReadModuleRequest {
                request_id,
                evaluator_id,
                uri,
            } => Ok(ExternalReaderMessage::ReadModuleRequest {
                request_id,
                evaluator_id,
                uri,
            }),
            Self::ReadModuleResponse {
                request_id,
                evaluator_id,
                contents,
                error,
            } => Ok(ExternalReaderMessage::ReadModuleResponse {
                request_id,
                evaluator_id,
                contents,
                error,
            }),
            Self::ListResourcesRequest {
                request_id,
                evaluator_id,
                uri,
            } => Ok(ExternalReaderMessage::ListResourcesRequest {
                request_id,
                evaluator_id,
                uri,
            }),
            Self::ListResourcesResponse {
                request_id,
                evaluator_id,
                path_elements,
                error,
            } => Ok(ExternalReaderMessage::ListResourcesResponse {
                request_id,
                evaluator_id,
                path_elements,
                error,
            }),
            Self::ListModulesRequest {
                request_id,
                evaluator_id,
                uri,
            } => Ok(ExternalReaderMessage::ListModulesRequest {
                request_id,
                evaluator_id,
                uri,
            }),
            Self::ListModulesResponse {
                request_id,
                evaluator_id,
                path_elements,
                error,
            } => Ok(ExternalReaderMessage::ListModulesResponse {
                request_id,
                evaluator_id,
                path_elements,
                error,
            }),
            other => Err(ExternalReaderError::unhandled_message_type(
                other.message_type(),
            )),
        }
    }
}
