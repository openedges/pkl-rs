//! Wire message types, reader specs, configuration, and `shlex` parsing for
//! Pkl 0.32.0 external module and resource readers.
//!
//! Pure `wasm32-unknown-unknown` compatible — zero OS process or filesystem I/O.
//! Grounded in pinned upstream Pkl 0.32.0:
//! - `org.pkl.core.messaging.Message` (`0x20`–`0x32`, external reader subset `0x26`–`0x32`)
//! - `org.pkl.core.messaging.Messages`
//! - `org.pkl.core.externalreader.{ExternalReaderMessages,ModuleReaderSpec,ResourceReaderSpec}`
//! - `org.pkl.core.evaluatorSettings.PklEvaluatorSettings.ExternalReader`
//! - `org.pkl.commons.StringsKt.shlex`

use crate::glob::PathElement;
use crate::host::{IoFailure, ReadFailure};
use crate::modules::ModuleHostError;
use crate::value::PklString;
use std::cmp::Ordering;
use std::fmt;

/// All 19 `org.pkl.core.messaging.Message.Type` wire codes (`0x20`–`0x32`).
///
/// External readers use codes `0x26`–`0x32`. Codes `0x20`–`0x25` belong to the
/// `pkl server` protocol (M5) and are kept in this enum so the decoder can
/// distinguish unrecognized codes (`malformedMessageHeaderUnrecognizedCode`)
/// from recognized-but-unhandled codes (`unhandledMessageCode`), matching
/// `AbstractMessagePackDecoder.java:68-84`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(i32)]
pub enum MessageType {
    CreateEvaluatorRequest = 0x20,
    CreateEvaluatorResponse = 0x21,
    CloseEvaluator = 0x22,
    EvaluateRequest = 0x23,
    EvaluateResponse = 0x24,
    LogMessage = 0x25,
    ReadResourceRequest = 0x26,
    ReadResourceResponse = 0x27,
    ReadModuleRequest = 0x28,
    ReadModuleResponse = 0x29,
    ListResourcesRequest = 0x2a,
    ListResourcesResponse = 0x2b,
    ListModulesRequest = 0x2c,
    ListModulesResponse = 0x2d,
    InitializeModuleReaderRequest = 0x2e,
    InitializeModuleReaderResponse = 0x2f,
    InitializeResourceReaderRequest = 0x30,
    InitializeResourceReaderResponse = 0x31,
    CloseExternalProcess = 0x32,
}

impl MessageType {
    /// Numeric wire code matching `Message.Type.getCode()`.
    pub const fn code(self) -> i32 {
        self as i32
    }

    /// Look up a `MessageType` by its integer wire code (`Message.Type.fromInt`).
    pub const fn from_code(code: i32) -> Option<Self> {
        match code {
            0x20 => Some(Self::CreateEvaluatorRequest),
            0x21 => Some(Self::CreateEvaluatorResponse),
            0x22 => Some(Self::CloseEvaluator),
            0x23 => Some(Self::EvaluateRequest),
            0x24 => Some(Self::EvaluateResponse),
            0x25 => Some(Self::LogMessage),
            0x26 => Some(Self::ReadResourceRequest),
            0x27 => Some(Self::ReadResourceResponse),
            0x28 => Some(Self::ReadModuleRequest),
            0x29 => Some(Self::ReadModuleResponse),
            0x2a => Some(Self::ListResourcesRequest),
            0x2b => Some(Self::ListResourcesResponse),
            0x2c => Some(Self::ListModulesRequest),
            0x2d => Some(Self::ListModulesResponse),
            0x2e => Some(Self::InitializeModuleReaderRequest),
            0x2f => Some(Self::InitializeModuleReaderResponse),
            0x30 => Some(Self::InitializeResourceReaderRequest),
            0x31 => Some(Self::InitializeResourceReaderResponse),
            0x32 => Some(Self::CloseExternalProcess),
            _ => None,
        }
    }

    /// Exact Java `Message.Type.toString()` enum constant name.
    pub const fn java_name(self) -> &'static str {
        match self {
            Self::CreateEvaluatorRequest => "CREATE_EVALUATOR_REQUEST",
            Self::CreateEvaluatorResponse => "CREATE_EVALUATOR_RESPONSE",
            Self::CloseEvaluator => "CLOSE_EVALUATOR",
            Self::EvaluateRequest => "EVALUATE_REQUEST",
            Self::EvaluateResponse => "EVALUATE_RESPONSE",
            Self::LogMessage => "LOG_MESSAGE",
            Self::ReadResourceRequest => "READ_RESOURCE_REQUEST",
            Self::ReadResourceResponse => "READ_RESOURCE_RESPONSE",
            Self::ReadModuleRequest => "READ_MODULE_REQUEST",
            Self::ReadModuleResponse => "READ_MODULE_RESPONSE",
            Self::ListResourcesRequest => "LIST_RESOURCES_REQUEST",
            Self::ListResourcesResponse => "LIST_RESOURCES_RESPONSE",
            Self::ListModulesRequest => "LIST_MODULES_REQUEST",
            Self::ListModulesResponse => "LIST_MODULES_RESPONSE",
            Self::InitializeModuleReaderRequest => "INITIALIZE_MODULE_READER_REQUEST",
            Self::InitializeModuleReaderResponse => "INITIALIZE_MODULE_READER_RESPONSE",
            Self::InitializeResourceReaderRequest => "INITIALIZE_RESOURCE_READER_REQUEST",
            Self::InitializeResourceReaderResponse => "INITIALIZE_RESOURCE_READER_RESPONSE",
            Self::CloseExternalProcess => "CLOSE_EXTERNAL_PROCESS",
        }
    }

    /// Whether this code belongs to the external-reader / shared reader subset (`0x26`–`0x32`).
    pub const fn is_external_reader_code(self) -> bool {
        (self as i32) >= 0x26 && (self as i32) <= 0x32
    }
}

impl fmt::Display for MessageType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.java_name())
    }
}

/// Configuration for spawning an external reader executable.
///
/// Mirrors `PklEvaluatorSettings.ExternalReader(executable, arguments, workingDir)`.
/// Equality/hashing across all three fields is used to deduplicate OS processes
/// across multiple schemes and across both module and resource readers
/// (`CliCommand.kt:208-210`, `EvaluatorBuilder.java:530-546`).
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ExternalReaderConfig {
    pub executable: String,
    pub arguments: Vec<String>,
    pub working_dir: Option<String>,
}

impl ExternalReaderConfig {
    pub fn new(
        executable: impl Into<String>,
        arguments: Vec<String>,
        working_dir: Option<String>,
    ) -> Self {
        Self {
            executable: executable.into(),
            arguments,
            working_dir,
        }
    }

    /// Parse a `<executable> [<arg> ...]` command string via Pkl's `shlex` rules.
    ///
    /// CLI-configured external readers always have `working_dir = None` (inheriting
    /// Pkl's current working directory), matching `BaseOptions.kt:95`.
    pub fn from_command_line(command: &str) -> Result<Self, ExternalReaderError> {
        let mut tokens = shlex(command);
        if tokens.is_empty() {
            return Err(ExternalReaderError::Configuration(
                "External reader command cannot be empty.".into(),
            ));
        }
        let executable = tokens.remove(0);
        Ok(Self {
            executable,
            arguments: tokens,
            working_dir: None,
        })
    }

    /// Parse a CLI `--external-module-reader` or `--external-resource-reader`
    /// option value of the form `<scheme>=<executable> [<arguments>]`
    /// (`BaseOptions.kt:86-97`).
    pub fn parse_cli_option(spec: &str) -> Result<(String, Self), ExternalReaderError> {
        let Some((scheme, command)) = spec.split_once('=') else {
            return Err(ExternalReaderError::Configuration(format!(
                "Invalid external reader specification `{spec}`: expected `<scheme>=<command>`."
            )));
        };
        if scheme.is_empty() {
            return Err(ExternalReaderError::Configuration(format!(
                "Invalid external reader specification `{spec}`: scheme cannot be empty."
            )));
        }
        let config = Self::from_command_line(command)?;
        Ok((scheme.to_string(), config))
    }

    /// Whether `executable` contains a path separator for the target OS
    /// (`EvaluatorSettings.pkl:198-200`). Bare executable names without a
    /// separator are resolved via OS `PATH` rather than against `PklProject`.
    pub fn executable_has_path_separator(&self, for_windows: bool) -> bool {
        if for_windows {
            self.executable.contains(['/', '\\'])
        } else {
            self.executable.contains('/')
        }
    }
}

/// Lex a command line string into tokens matching `org.pkl.commons.StringsKt.shlex`
/// (`external/pkl/pkl-commons/src/main/kotlin/org/pkl/commons/Strings.kt:41-88`).
pub fn shlex(input: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut in_escape = false;
    let mut quote: Option<char> = None;
    let mut last_close_quote_index: Option<usize> = None;
    let mut current = String::new();
    let mut total_chars = 0usize;

    for (idx, ch) in input.chars().enumerate() {
        total_chars = idx + 1;
        if in_escape {
            in_escape = false;
            current.push(ch);
        } else if ch == '\\' && quote != Some('\'') {
            in_escape = true;
        } else if quote == Some(ch) {
            quote = None;
            last_close_quote_index = Some(idx);
        } else if quote.is_none() && (ch == '\'' || ch == '"') {
            quote = Some(ch);
        } else if quote.is_none() && ch.is_whitespace() {
            if !current.is_empty() || last_close_quote_index.is_some_and(|last| last + 1 == idx) {
                result.push(std::mem::take(&mut current));
            }
        } else {
            current.push(ch);
        }
    }

    if !current.is_empty() || (total_chars > 0 && last_close_quote_index == Some(total_chars - 1)) {
        result.push(current);
    }

    result
}

/// Wire representation of `org.pkl.core.externalreader.ModuleReaderSpec`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModuleReaderSpec {
    pub scheme: String,
    pub has_hierarchical_uris: bool,
    pub is_local: bool,
    pub is_globbable: bool,
}

/// Wire representation of `org.pkl.core.externalreader.ResourceReaderSpec`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResourceReaderSpec {
    pub scheme: String,
    pub has_hierarchical_uris: bool,
    pub is_globbable: bool,
}

/// Wire representation of `org.pkl.core.module.PathElement`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WirePathElement {
    pub name: String,
    pub is_directory: bool,
}

impl WirePathElement {
    pub fn file(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            is_directory: false,
        }
    }

    pub fn directory(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            is_directory: true,
        }
    }

    /// Ordering matching `PathElement.comparator` (`PathElement.java:28-36`):
    /// non-directories before directories, then UTF-16 code-unit order.
    pub fn cmp_java(&self, other: &Self) -> Ordering {
        match (self.is_directory, other.is_directory) {
            (true, false) => Ordering::Greater,
            (false, true) => Ordering::Less,
            _ => self.name.encode_utf16().cmp(other.name.encode_utf16()),
        }
    }

    /// Convert into the runtime's `glob::PathElement`.
    pub fn to_glob_element(&self) -> PathElement {
        PathElement {
            name: PklString::from(self.name.as_str()),
            is_directory: self.is_directory,
        }
    }
}

impl From<WirePathElement> for PathElement {
    fn from(value: WirePathElement) -> Self {
        Self {
            name: PklString::from(value.name),
            is_directory: value.is_directory,
        }
    }
}

/// All 13 external-reader wire messages (`0x26`–`0x32`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExternalReaderMessage {
    /// `0x26` — Server → Client request.
    ReadResourceRequest {
        request_id: i64,
        evaluator_id: i64,
        uri: String,
    },
    /// `0x27` — Client → Server response.
    ReadResourceResponse {
        request_id: i64,
        evaluator_id: i64,
        contents: Option<Vec<u8>>,
        error: Option<String>,
    },
    /// `0x28` — Server → Client request.
    ReadModuleRequest {
        request_id: i64,
        evaluator_id: i64,
        uri: String,
    },
    /// `0x29` — Client → Server response.
    ReadModuleResponse {
        request_id: i64,
        evaluator_id: i64,
        contents: Option<String>,
        error: Option<String>,
    },
    /// `0x2a` — Server → Client request.
    ListResourcesRequest {
        request_id: i64,
        evaluator_id: i64,
        uri: String,
    },
    /// `0x2b` — Client → Server response.
    ListResourcesResponse {
        request_id: i64,
        evaluator_id: i64,
        path_elements: Option<Vec<WirePathElement>>,
        error: Option<String>,
    },
    /// `0x2c` — Server → Client request.
    ListModulesRequest {
        request_id: i64,
        evaluator_id: i64,
        uri: String,
    },
    /// `0x2d` — Client → Server response.
    ListModulesResponse {
        request_id: i64,
        evaluator_id: i64,
        path_elements: Option<Vec<WirePathElement>>,
        error: Option<String>,
    },
    /// `0x2e` — Server → Client request.
    InitializeModuleReaderRequest { request_id: i64, scheme: String },
    /// `0x2f` — Client → Server response.
    InitializeModuleReaderResponse {
        request_id: i64,
        spec: Option<ModuleReaderSpec>,
    },
    /// `0x30` — Server → Client request.
    InitializeResourceReaderRequest { request_id: i64, scheme: String },
    /// `0x31` — Client → Server response.
    InitializeResourceReaderResponse {
        request_id: i64,
        spec: Option<ResourceReaderSpec>,
    },
    /// `0x32` — Server → Client one-way graceful shutdown notice.
    CloseExternalProcess,
}

impl ExternalReaderMessage {
    pub const fn message_type(&self) -> MessageType {
        match self {
            Self::ReadResourceRequest { .. } => MessageType::ReadResourceRequest,
            Self::ReadResourceResponse { .. } => MessageType::ReadResourceResponse,
            Self::ReadModuleRequest { .. } => MessageType::ReadModuleRequest,
            Self::ReadModuleResponse { .. } => MessageType::ReadModuleResponse,
            Self::ListResourcesRequest { .. } => MessageType::ListResourcesRequest,
            Self::ListResourcesResponse { .. } => MessageType::ListResourcesResponse,
            Self::ListModulesRequest { .. } => MessageType::ListModulesRequest,
            Self::ListModulesResponse { .. } => MessageType::ListModulesResponse,
            Self::InitializeModuleReaderRequest { .. } => {
                MessageType::InitializeModuleReaderRequest
            }
            Self::InitializeModuleReaderResponse { .. } => {
                MessageType::InitializeModuleReaderResponse
            }
            Self::InitializeResourceReaderRequest { .. } => {
                MessageType::InitializeResourceReaderRequest
            }
            Self::InitializeResourceReaderResponse { .. } => {
                MessageType::InitializeResourceReaderResponse
            }
            Self::CloseExternalProcess => MessageType::CloseExternalProcess,
        }
    }

    pub const fn request_id(&self) -> Option<i64> {
        match self {
            Self::ReadResourceRequest { request_id, .. }
            | Self::ReadResourceResponse { request_id, .. }
            | Self::ReadModuleRequest { request_id, .. }
            | Self::ReadModuleResponse { request_id, .. }
            | Self::ListResourcesRequest { request_id, .. }
            | Self::ListResourcesResponse { request_id, .. }
            | Self::ListModulesRequest { request_id, .. }
            | Self::ListModulesResponse { request_id, .. }
            | Self::InitializeModuleReaderRequest { request_id, .. }
            | Self::InitializeModuleReaderResponse { request_id, .. }
            | Self::InitializeResourceReaderRequest { request_id, .. }
            | Self::InitializeResourceReaderResponse { request_id, .. } => Some(*request_id),
            Self::CloseExternalProcess => None,
        }
    }

    pub const fn is_request(&self) -> bool {
        matches!(
            self,
            Self::ReadResourceRequest { .. }
                | Self::ReadModuleRequest { .. }
                | Self::ListResourcesRequest { .. }
                | Self::ListModulesRequest { .. }
                | Self::InitializeModuleReaderRequest { .. }
                | Self::InitializeResourceReaderRequest { .. }
        )
    }

    pub const fn is_response(&self) -> bool {
        matches!(
            self,
            Self::ReadResourceResponse { .. }
                | Self::ReadModuleResponse { .. }
                | Self::ListResourcesResponse { .. }
                | Self::ListModulesResponse { .. }
                | Self::InitializeModuleReaderResponse { .. }
                | Self::InitializeResourceReaderResponse { .. }
        )
    }

    pub const fn is_one_way(&self) -> bool {
        matches!(self, Self::CloseExternalProcess)
    }

    /// Exact Java simple class name used in `unknownRequestId` (`MessageTransports.java:165`).
    pub const fn java_simple_name(&self) -> &'static str {
        match self {
            Self::ReadResourceRequest { .. } => "ReadResourceRequest",
            Self::ReadResourceResponse { .. } => "ReadResourceResponse",
            Self::ReadModuleRequest { .. } => "ReadModuleRequest",
            Self::ReadModuleResponse { .. } => "ReadModuleResponse",
            Self::ListResourcesRequest { .. } => "ListResourcesRequest",
            Self::ListResourcesResponse { .. } => "ListResourcesResponse",
            Self::ListModulesRequest { .. } => "ListModulesRequest",
            Self::ListModulesResponse { .. } => "ListModulesResponse",
            Self::InitializeModuleReaderRequest { .. } => "InitializeModuleReaderRequest",
            Self::InitializeModuleReaderResponse { .. } => "InitializeModuleReaderResponse",
            Self::InitializeResourceReaderRequest { .. } => "InitializeResourceReaderRequest",
            Self::InitializeResourceReaderResponse { .. } => "InitializeResourceReaderResponse",
            Self::CloseExternalProcess => "CloseExternalProcess",
        }
    }
}

/// Typed external-reader errors matching Pkl 0.32.0 exception hierarchy & templates.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExternalReaderError {
    /// Invalid CLI/config syntax (e.g. malformed `--external-*-reader`).
    Configuration(String),
    /// Wire decode failure (`org.pkl.core.messaging.DecodeException`).
    Decode {
        message: String,
        cause: Option<String>,
    },
    /// Protocol violation (`org.pkl.core.messaging.ProtocolException`).
    Protocol(String),
    /// Process-level failure (`org.pkl.core.externalreader.ExternalReaderProcessException`),
    /// such as unsupported scheme, already terminated, or spawn failure.
    Process(String),
    /// Reader-returned error string or transport `IOException` (`java.io.IOException`).
    Io(String),
    /// Process/transport was already closed (`IllegalStateException` in Java).
    Closed,
    /// Evaluation was cancelled while awaiting an external reader response.
    Cancelled,
    /// Request exceeded configured timeout deadline.
    Timeout,
}

impl ExternalReaderError {
    /// `errorMessages.properties:1104-1105` — `externalReaderDoesNotSupportScheme`.
    pub fn does_not_support_scheme(kind: &str, scheme: &str) -> Self {
        Self::Process(format!(
            "External {kind} reader does not support scheme `{scheme}`."
        ))
    }

    /// `errorMessages.properties:1107-1108` — `externalReaderAlreadyTerminated`.
    pub fn already_terminated() -> Self {
        Self::Process("External reader process has already terminated.".into())
    }

    /// `errorMessages.properties:1098-1099` — `unknownRequestId`.
    pub fn unknown_request_id(simple_name: &str, request_id: i64) -> Self {
        Self::Protocol(format!(
            "Received response {simple_name} for unknown request ID `{request_id}`."
        ))
    }

    /// `errorMessages.properties:1089-1090` — `unhandledMessageType`.
    pub fn unhandled_message_type(msg_type: MessageType) -> Self {
        Self::Protocol(format!(
            "Unhandled encoding message type `{}`.",
            msg_type.java_name()
        ))
    }

    /// Wrap a non-`IOException` failure inside `MessageTransports.resolveFuture`
    /// (`MessageTransports.java:57-61`).
    pub fn into_resolved_future_io(self) -> Self {
        match self {
            Self::Io(msg) => Self::Io(msg),
            Self::Process(msg) => Self::Io(format!(
                "external read failure: org.pkl.core.externalreader.ExternalReaderProcessException: {msg}"
            )),
            Self::Decode { message, .. } => Self::Io(format!(
                "external read failure: org.pkl.core.messaging.DecodeException: {message}"
            )),
            Self::Protocol(msg) => Self::Io(format!(
                "external read failure: org.pkl.core.messaging.ProtocolException: {msg}"
            )),
            Self::Closed => Self::Io(
                "external read failure: java.lang.IllegalStateException: External reader process has already been closed."
                    .into(),
            ),
            Self::Cancelled => Self::Io("external read failure: evaluation cancelled".into()),
            Self::Timeout => Self::Io("external read failure: evaluation timed out".into()),
            Self::Configuration(msg) => Self::Io(format!("external read failure: {msg}")),
        }
    }

    /// Map into `ModuleHostError` for `ModuleResolver`, `ModuleCache`, and `GlobResolver`.
    pub fn into_module_host_error(self) -> ModuleHostError {
        match self {
            Self::Process(msg) => ModuleHostError::ExternalReaderProcess {
                message: Some(PklString::from(msg)),
            },
            Self::Io(msg) => ModuleHostError::Io(IoFailure::from(msg)),
            Self::Decode { message, .. } => ModuleHostError::Io(IoFailure::from(format!(
                "external read failure: org.pkl.core.messaging.DecodeException: {message}"
            ))),
            Self::Protocol(msg) => ModuleHostError::Io(IoFailure::from(format!(
                "external read failure: org.pkl.core.messaging.ProtocolException: {msg}"
            ))),
            Self::Closed => ModuleHostError::ExternalReaderProcess {
                message: Some("External reader process has already been closed.".into()),
            },
            Self::Cancelled => ModuleHostError::Denied("evaluation cancelled".into()),
            Self::Timeout => ModuleHostError::Denied("evaluation timed out".into()),
            Self::Configuration(msg) => ModuleHostError::ExternalReaderProcess {
                message: Some(PklString::from(msg)),
            },
        }
    }

    /// Map into `ReadFailure` for `ResourceManager.doRead` (`ResourceManager.java:76-93`).
    ///
    /// Note that `ResourceManager.doRead` renders `ExternalReaderProcessException`
    /// with its bare message (no `externalReaderFailure` wrapper), whereas `IOException`
    /// renders ``I/O error reading resource `<uri>`.\nIOException: <msg>``.
    pub fn into_read_failure(self) -> ReadFailure {
        match self {
            Self::Process(msg) => ReadFailure::ExternalReaderProcess {
                message: Some(PklString::from(msg)),
            },
            Self::Io(msg) => ReadFailure::Io(IoFailure::from(msg)),
            Self::Decode { message, .. } => ReadFailure::Io(IoFailure::from(format!(
                "external read failure: org.pkl.core.messaging.DecodeException: {message}"
            ))),
            Self::Protocol(msg) => ReadFailure::Io(IoFailure::from(format!(
                "external read failure: org.pkl.core.messaging.ProtocolException: {msg}"
            ))),
            Self::Closed => ReadFailure::ExternalReaderProcess {
                message: Some("External reader process has already been closed.".into()),
            },
            Self::Cancelled => ReadFailure::EvalError {
                message: "evaluation cancelled".into(),
                hint: None,
            },
            Self::Timeout => ReadFailure::EvalError {
                message: "evaluation timed out".into(),
                hint: None,
            },
            Self::Configuration(msg) => ReadFailure::ExternalReaderProcess {
                message: Some(PklString::from(msg)),
            },
        }
    }
}

impl fmt::Display for ExternalReaderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Configuration(msg) | Self::Protocol(msg) | Self::Process(msg) | Self::Io(msg) => {
                f.write_str(msg)
            }
            Self::Decode { message, cause } => {
                f.write_str(message)?;
                if let Some(cause) = cause {
                    write!(f, " ({cause})")?;
                }
                Ok(())
            }
            Self::Closed => f.write_str("External reader process has already been closed."),
            Self::Cancelled => f.write_str("External reader request cancelled."),
            Self::Timeout => f.write_str("External reader request timed out."),
        }
    }
}

impl std::error::Error for ExternalReaderError {}
