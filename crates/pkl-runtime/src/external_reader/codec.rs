//! Pure-Rust MessagePack wire codec for Pkl 0.32.0 external readers (`0x26`–`0x32`).
//!
//! Matches:
//! - `org.pkl.core.messaging.AbstractMessagePackEncoder`
//! - `org.pkl.core.messaging.BaseMessagePackEncoder`
//! - `org.pkl.core.externalreader.ExternalReaderMessagePackEncoder`
//! - `org.pkl.core.messaging.AbstractMessagePackDecoder`
//! - `org.pkl.core.messaging.BaseMessagePackDecoder`
//! - `org.pkl.core.externalreader.ExternalReaderMessagePackDecoder`
//!
//! Pure `wasm32-unknown-unknown` compatible, zero `unsafe` code, and zero extra crate
//! dependencies. Uses `std::io::{Read, Write}` so the same streaming codec drives both
//! in-memory Wasm buffers (`Cursor<&[u8]>` / `Vec<u8>`) and native subprocess stdio pipes.

use super::messages::{
    ExternalReaderError, ExternalReaderMessage, MessageType, ModuleReaderSpec, ResourceReaderSpec,
    WirePathElement,
};
use std::io::{Cursor, Read, Write};

const MAX_NESTING_DEPTH: usize = 64;
const MAX_PAYLOAD_BYTES: usize = 256 * 1024 * 1024;
const MAX_MESSAGE_TOTAL_BYTES: usize = 256 * 1024 * 1024;
const MAX_COLLECTION_ELEMENTS: usize = 4 * 1024 * 1024;
const MAX_MESSAGE_NODES: usize = 4 * 1024 * 1024;
const MSGPACK_READ_CHUNK_BYTES: usize = 64 * 1024;

// ============================================================================
// Public Convenience Functions
// ============================================================================

/// Encode a single external-reader message into a newly allocated MessagePack buffer.
pub fn encode_to_vec(msg: &ExternalReaderMessage) -> Result<Vec<u8>, ExternalReaderError> {
    let mut buf = Vec::with_capacity(64);
    write_message(&mut buf, msg)?;
    Ok(buf)
}

/// Encode and flush a single external-reader message onto any `std::io::Write` sink.
///
/// Wire format: `[code: int, body: map]` (2-element MessagePack array, no length prefix).
/// Optional fields whose value is `None` are omitted from the body map.
pub fn write_message<W: Write>(
    writer: &mut W,
    msg: &ExternalReaderMessage,
) -> Result<(), ExternalReaderError> {
    let mut packer = MsgPackWriter::new(writer);
    packer.pack_array_header(2)?;
    packer.pack_i32(msg.message_type().code())?;
    encode_body(&mut packer, msg)?;
    packer.flush()
}

/// Decode the next external-reader message from any `std::io::Read` stream.
///
/// Returns `Ok(None)` on clean EOF before the first byte of the next message
/// (`AbstractMessagePackDecoder.java:52-54`). Truncation mid-message or invalid
/// MessagePack framing returns `Err(ExternalReaderError::Decode { .. })` or `Io`.
pub fn read_message<R: Read>(
    reader: &mut R,
) -> Result<Option<ExternalReaderMessage>, ExternalReaderError> {
    let mut unpacker = MsgPackReader::new(reader);
    let Some(first_byte) = unpacker.read_first_byte_or_eof()? else {
        return Ok(None);
    };

    // Step 1: Unpack 2-element array header + integer message code.
    let array_size = unpacker
        .unpack_array_header_from_byte(first_byte)
        .map_err(|cause| ExternalReaderError::Decode {
            message: "Malformed message header.".into(),
            cause: Some(cause),
        })?;
    if array_size != 2 {
        return Err(ExternalReaderError::Decode {
            message: format!("Malformed message header (expected size 2, but got {array_size})."),
            cause: None,
        });
    }

    let code = unpacker
        .unpack_i32()
        .map_err(|cause| ExternalReaderError::Decode {
            message: "Malformed message header.".into(),
            cause: Some(cause),
        })?;

    // Step 2: Validate message type code against Message.Type (`0x20`–`0x32`).
    let Some(msg_type) = MessageType::from_code(code) else {
        return Err(ExternalReaderError::Decode {
            message: format!(
                "Malformed message header (unrecognized code `{:x}`).",
                code as u32
            ),
            cause: None,
        });
    };

    // Step 3: Unpack body value (must be a MessagePack map).
    let body_value = unpacker
        .unpack_value(0)
        .map_err(|cause| ExternalReaderError::Decode {
            message: format!("Malformed message body for message with code `{code}`."),
            cause: Some(cause),
        })?;

    let MsgPackValue::Map(entries) = body_value else {
        return Err(ExternalReaderError::Decode {
            message: format!("Malformed message body for message with code `{code}`."),
            cause: Some("Expected Map value for message body.".into()),
        });
    };

    // Step 4: Decode specific message payload.
    let map = MapView::new(&entries);
    match decode_body(msg_type, code, &map)? {
        Some(message) => Ok(Some(message)),
        None => Err(ExternalReaderError::Decode {
            message: format!("Unhandled decoding message code `{:x}`.", code as u32),
            cause: None,
        }),
    }
}

/// Decode a single message from an in-memory byte slice.
pub fn decode_from_slice(
    bytes: &[u8],
) -> Result<Option<ExternalReaderMessage>, ExternalReaderError> {
    let mut cursor = Cursor::new(bytes);
    read_message(&mut cursor)
}

/// Decode all concatenated messages from an in-memory byte slice until clean EOF.
pub fn decode_all_from_slice(
    bytes: &[u8],
) -> Result<Vec<ExternalReaderMessage>, ExternalReaderError> {
    let mut cursor = Cursor::new(bytes);
    let mut out = Vec::new();
    while let Some(msg) = read_message(&mut cursor)? {
        out.push(msg);
    }
    Ok(out)
}

// ============================================================================
// Message Body Encoding (exact field order matching upstream Java encoders)
// ============================================================================

fn encode_body<W: Write>(
    packer: &mut MsgPackWriter<'_, W>,
    msg: &ExternalReaderMessage,
) -> Result<(), ExternalReaderError> {
    match msg {
        ExternalReaderMessage::ReadResourceRequest {
            request_id,
            evaluator_id,
            uri,
        } => {
            packer.pack_map_header(3)?;
            packer.pack_kv_i64("requestId", *request_id)?;
            packer.pack_kv_i64("evaluatorId", *evaluator_id)?;
            packer.pack_kv_str("uri", uri)?;
        }
        ExternalReaderMessage::ReadResourceResponse {
            request_id,
            evaluator_id,
            contents,
            error,
        } => {
            let count = 2 + usize::from(contents.is_some()) + usize::from(error.is_some());
            packer.pack_map_header(count)?;
            packer.pack_kv_i64("requestId", *request_id)?;
            packer.pack_kv_i64("evaluatorId", *evaluator_id)?;
            if let Some(bytes) = contents {
                packer.pack_kv_bin("contents", bytes)?;
            }
            if let Some(err) = error {
                packer.pack_kv_str("error", err)?;
            }
        }
        ExternalReaderMessage::ReadModuleRequest {
            request_id,
            evaluator_id,
            uri,
        } => {
            packer.pack_map_header(3)?;
            packer.pack_kv_i64("requestId", *request_id)?;
            packer.pack_kv_i64("evaluatorId", *evaluator_id)?;
            packer.pack_kv_str("uri", uri)?;
        }
        ExternalReaderMessage::ReadModuleResponse {
            request_id,
            evaluator_id,
            contents,
            error,
        } => {
            let count = 2 + usize::from(contents.is_some()) + usize::from(error.is_some());
            packer.pack_map_header(count)?;
            packer.pack_kv_i64("requestId", *request_id)?;
            packer.pack_kv_i64("evaluatorId", *evaluator_id)?;
            if let Some(text) = contents {
                packer.pack_kv_str("contents", text)?;
            }
            if let Some(err) = error {
                packer.pack_kv_str("error", err)?;
            }
        }
        ExternalReaderMessage::ListResourcesRequest {
            request_id,
            evaluator_id,
            uri,
        } => {
            packer.pack_map_header(3)?;
            packer.pack_kv_i64("requestId", *request_id)?;
            packer.pack_kv_i64("evaluatorId", *evaluator_id)?;
            packer.pack_kv_str("uri", uri)?;
        }
        ExternalReaderMessage::ListResourcesResponse {
            request_id,
            evaluator_id,
            path_elements,
            error,
        } => {
            let count = 2 + usize::from(path_elements.is_some()) + usize::from(error.is_some());
            packer.pack_map_header(count)?;
            packer.pack_kv_i64("requestId", *request_id)?;
            packer.pack_kv_i64("evaluatorId", *evaluator_id)?;
            if let Some(elements) = path_elements {
                packer.pack_str("pathElements")?;
                packer.pack_array_header(elements.len())?;
                for element in elements {
                    pack_path_element(packer, element)?;
                }
            }
            if let Some(err) = error {
                packer.pack_kv_str("error", err)?;
            }
        }
        ExternalReaderMessage::ListModulesRequest {
            request_id,
            evaluator_id,
            uri,
        } => {
            packer.pack_map_header(3)?;
            packer.pack_kv_i64("requestId", *request_id)?;
            packer.pack_kv_i64("evaluatorId", *evaluator_id)?;
            packer.pack_kv_str("uri", uri)?;
        }
        ExternalReaderMessage::ListModulesResponse {
            request_id,
            evaluator_id,
            path_elements,
            error,
        } => {
            let count = 2 + usize::from(path_elements.is_some()) + usize::from(error.is_some());
            packer.pack_map_header(count)?;
            packer.pack_kv_i64("requestId", *request_id)?;
            packer.pack_kv_i64("evaluatorId", *evaluator_id)?;
            if let Some(elements) = path_elements {
                packer.pack_str("pathElements")?;
                packer.pack_array_header(elements.len())?;
                for element in elements {
                    pack_path_element(packer, element)?;
                }
            }
            if let Some(err) = error {
                packer.pack_kv_str("error", err)?;
            }
        }
        ExternalReaderMessage::InitializeModuleReaderRequest { request_id, scheme } => {
            packer.pack_map_header(2)?;
            packer.pack_kv_i64("requestId", *request_id)?;
            packer.pack_kv_str("scheme", scheme)?;
        }
        ExternalReaderMessage::InitializeResourceReaderRequest { request_id, scheme } => {
            packer.pack_map_header(2)?;
            packer.pack_kv_i64("requestId", *request_id)?;
            packer.pack_kv_str("scheme", scheme)?;
        }
        ExternalReaderMessage::InitializeModuleReaderResponse { request_id, spec } => {
            let count = 1 + usize::from(spec.is_some());
            packer.pack_map_header(count)?;
            packer.pack_kv_i64("requestId", *request_id)?;
            if let Some(spec) = spec {
                packer.pack_str("spec")?;
                pack_module_reader_spec(packer, spec)?;
            }
        }
        ExternalReaderMessage::InitializeResourceReaderResponse { request_id, spec } => {
            let count = 1 + usize::from(spec.is_some());
            packer.pack_map_header(count)?;
            packer.pack_kv_i64("requestId", *request_id)?;
            if let Some(spec) = spec {
                packer.pack_str("spec")?;
                pack_resource_reader_spec(packer, spec)?;
            }
        }
        ExternalReaderMessage::CloseExternalProcess => {
            packer.pack_map_header(0)?;
        }
    }
    Ok(())
}

fn pack_module_reader_spec<W: Write>(
    packer: &mut MsgPackWriter<'_, W>,
    spec: &ModuleReaderSpec,
) -> Result<(), ExternalReaderError> {
    packer.pack_map_header(4)?;
    packer.pack_kv_str("scheme", &spec.scheme)?;
    packer.pack_kv_bool("hasHierarchicalUris", spec.has_hierarchical_uris)?;
    packer.pack_kv_bool("isLocal", spec.is_local)?;
    packer.pack_kv_bool("isGlobbable", spec.is_globbable)?;
    Ok(())
}

fn pack_resource_reader_spec<W: Write>(
    packer: &mut MsgPackWriter<'_, W>,
    spec: &ResourceReaderSpec,
) -> Result<(), ExternalReaderError> {
    packer.pack_map_header(3)?;
    packer.pack_kv_str("scheme", &spec.scheme)?;
    packer.pack_kv_bool("hasHierarchicalUris", spec.has_hierarchical_uris)?;
    packer.pack_kv_bool("isGlobbable", spec.is_globbable)?;
    Ok(())
}

fn pack_path_element<W: Write>(
    packer: &mut MsgPackWriter<'_, W>,
    element: &WirePathElement,
) -> Result<(), ExternalReaderError> {
    packer.pack_map_header(2)?;
    packer.pack_kv_str("name", &element.name)?;
    packer.pack_kv_bool("isDirectory", element.is_directory)?;
    Ok(())
}

// ============================================================================
// Message Body Decoding (exact error rules matching upstream Java decoders)
// ============================================================================

fn decode_body(
    msg_type: MessageType,
    code: i32,
    map: &MapView<'_>,
) -> Result<Option<ExternalReaderMessage>, ExternalReaderError> {
    let msg = match msg_type {
        MessageType::InitializeModuleReaderRequest => {
            ExternalReaderMessage::InitializeModuleReaderRequest {
                request_id: map.req_i64("requestId", code)?,
                scheme: map.req_str("scheme", code)?,
            }
        }
        MessageType::InitializeResourceReaderRequest => {
            ExternalReaderMessage::InitializeResourceReaderRequest {
                request_id: map.req_i64("requestId", code)?,
                scheme: map.req_str("scheme", code)?,
            }
        }
        MessageType::InitializeModuleReaderResponse => {
            ExternalReaderMessage::InitializeModuleReaderResponse {
                request_id: map.req_i64("requestId", code)?,
                spec: unpack_module_reader_spec(map.opt("spec"), code)?,
            }
        }
        MessageType::InitializeResourceReaderResponse => {
            ExternalReaderMessage::InitializeResourceReaderResponse {
                request_id: map.req_i64("requestId", code)?,
                spec: unpack_resource_reader_spec(map.opt("spec"), code)?,
            }
        }
        MessageType::CloseExternalProcess => ExternalReaderMessage::CloseExternalProcess,
        MessageType::ReadResourceRequest => ExternalReaderMessage::ReadResourceRequest {
            request_id: map.req_i64("requestId", code)?,
            evaluator_id: map.req_i64("evaluatorId", code)?,
            uri: map.req_uri("uri", code)?,
        },
        MessageType::ReadResourceResponse => ExternalReaderMessage::ReadResourceResponse {
            request_id: map.req_i64("requestId", code)?,
            evaluator_id: map.req_i64("evaluatorId", code)?,
            contents: map.opt_bin("contents", code)?,
            error: map.opt_str("error", code)?,
        },
        MessageType::ReadModuleRequest => ExternalReaderMessage::ReadModuleRequest {
            request_id: map.req_i64("requestId", code)?,
            evaluator_id: map.req_i64("evaluatorId", code)?,
            uri: map.req_uri("uri", code)?,
        },
        MessageType::ReadModuleResponse => ExternalReaderMessage::ReadModuleResponse {
            request_id: map.req_i64("requestId", code)?,
            evaluator_id: map.req_i64("evaluatorId", code)?,
            contents: map.opt_str("contents", code)?,
            error: map.opt_str("error", code)?,
        },
        MessageType::ListResourcesRequest => ExternalReaderMessage::ListResourcesRequest {
            request_id: map.req_i64("requestId", code)?,
            evaluator_id: map.req_i64("evaluatorId", code)?,
            uri: map.req_uri("uri", code)?,
        },
        MessageType::ListResourcesResponse => ExternalReaderMessage::ListResourcesResponse {
            request_id: map.req_i64("requestId", code)?,
            evaluator_id: map.req_i64("evaluatorId", code)?,
            path_elements: unpack_path_elements(map.opt("pathElements"), code)?,
            error: map.opt_str("error", code)?,
        },
        MessageType::ListModulesRequest => ExternalReaderMessage::ListModulesRequest {
            request_id: map.req_i64("requestId", code)?,
            evaluator_id: map.req_i64("evaluatorId", code)?,
            uri: map.req_uri("uri", code)?,
        },
        MessageType::ListModulesResponse => ExternalReaderMessage::ListModulesResponse {
            request_id: map.req_i64("requestId", code)?,
            evaluator_id: map.req_i64("evaluatorId", code)?,
            path_elements: unpack_path_elements(map.opt("pathElements"), code)?,
            error: map.opt_str("error", code)?,
        },
        // Server-only codes (`0x20`–`0x25`): recognized by MessageType but unhandled by
        // ExternalReaderMessagePackDecoder (`AbstractMessagePackDecoder.java:82-83`).
        MessageType::CreateEvaluatorRequest
        | MessageType::CreateEvaluatorResponse
        | MessageType::CloseEvaluator
        | MessageType::EvaluateRequest
        | MessageType::EvaluateResponse
        | MessageType::LogMessage => return Ok(None),
    };
    Ok(Some(msg))
}

pub(crate) fn malformed_body(code: i32, cause: impl Into<String>) -> ExternalReaderError {
    ExternalReaderError::Decode {
        message: format!("Malformed message body for message with code `{code}`."),
        cause: Some(cause.into()),
    }
}

pub(crate) fn missing_param(key: &str) -> ExternalReaderError {
    // Exact template in `errorMessages.properties:1095-1096` (no trailing period).
    ExternalReaderError::Decode {
        message: format!("Missing message parameter `{key}`"),
        cause: None,
    }
}

fn unpack_module_reader_spec(
    value: Option<&MsgPackValue>,
    code: i32,
) -> Result<Option<ModuleReaderSpec>, ExternalReaderError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let MsgPackValue::Map(entries) = value else {
        return Err(malformed_body(code, "Expected Map for `spec`."));
    };
    let map = MapView::new(entries);
    Ok(Some(ModuleReaderSpec {
        scheme: map.req_str("scheme", code)?,
        has_hierarchical_uris: map.req_bool("hasHierarchicalUris", code)?,
        is_local: map.req_bool("isLocal", code)?,
        is_globbable: map.req_bool("isGlobbable", code)?,
    }))
}

fn unpack_resource_reader_spec(
    value: Option<&MsgPackValue>,
    code: i32,
) -> Result<Option<ResourceReaderSpec>, ExternalReaderError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let MsgPackValue::Map(entries) = value else {
        return Err(malformed_body(code, "Expected Map for `spec`."));
    };
    let map = MapView::new(entries);
    Ok(Some(ResourceReaderSpec {
        scheme: map.req_str("scheme", code)?,
        has_hierarchical_uris: map.req_bool("hasHierarchicalUris", code)?,
        is_globbable: map.req_bool("isGlobbable", code)?,
    }))
}

fn unpack_path_elements(
    value: Option<&MsgPackValue>,
    code: i32,
) -> Result<Option<Vec<WirePathElement>>, ExternalReaderError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let MsgPackValue::Array(items) = value else {
        return Err(malformed_body(code, "Expected Array for `pathElements`."));
    };
    let mut elements = Vec::with_capacity(items.len());
    for item in items {
        let MsgPackValue::Map(entries) = item else {
            return Err(malformed_body(code, "Expected Map for `PathElement` item."));
        };
        let map = MapView::new(entries);
        elements.push(WirePathElement {
            name: map.req_str("name", code)?,
            is_directory: map.req_bool("isDirectory", code)?,
        });
    }
    Ok(Some(elements))
}

/// Validates RFC 2396 / `java.net.URI(String)` syntax (accepting both relative
/// and absolute URIs, matching `BaseMessagePackDecoder.java:46,57,68,79`).
pub(crate) fn validate_java_uri(uri: &str) -> Result<(), String> {
    let bytes = uri.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b <= 0x20
            || b == 0x7f
            || matches!(
                b,
                b'<' | b'>' | b'"' | b'{' | b'}' | b'|' | b'\\' | b'^' | b'`'
            )
        {
            return Err(format!("Illegal character in URI at index {i}: `{uri}`"));
        }
        if b == b'%' {
            let valid = bytes.get(i + 1).is_some_and(u8::is_ascii_hexdigit)
                && bytes.get(i + 2).is_some_and(u8::is_ascii_hexdigit);
            if !valid {
                return Err(format!("Malformed escape pair at index {i}: `{uri}`"));
            }
            i += 3;
            continue;
        }
        i += 1;
    }
    // If a scheme is present before `/`, `?`, `#`, validate scheme starts with ASCII alpha.
    if let Some(colon) = uri.find(':') {
        let before = &uri[..colon];
        if !before.contains(['/', '?', '#']) {
            if before.is_empty() {
                return Err(format!("Expected scheme name at index 0: `{uri}`"));
            }
            let mut chars = before.chars();
            let first = chars.next().expect("non-empty scheme");
            if !first.is_ascii_alphabetic()
                || !chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
            {
                return Err(format!("Illegal character in scheme name: `{uri}`"));
            }
            // Java `URI` forbids an empty scheme-specific part (`foo:` with nothing after `:`).
            if colon + 1 == uri.len() {
                return Err(format!(
                    "Expected scheme-specific part at index {}: `{uri}`",
                    colon + 1
                ));
            }
        }
    }
    Ok(())
}

// ============================================================================
// Low-Level MessagePack Value & Map Lookup
// ============================================================================

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum MsgPackValue {
    Nil,
    Bool(bool),
    Int(i64),
    Uint(u64),
    Float(f64),
    Str(String),
    Bin(Vec<u8>),
    Array(Vec<MsgPackValue>),
    Map(Vec<(MsgPackValue, MsgPackValue)>),
    Ext(i8, Vec<u8>),
}

pub(crate) struct MapView<'a> {
    entries: &'a [(MsgPackValue, MsgPackValue)],
}

impl<'a> MapView<'a> {
    pub(crate) fn new(entries: &'a [(MsgPackValue, MsgPackValue)]) -> Self {
        Self { entries }
    }

    /// Look up by string key; if duplicate keys appear, the last one wins
    /// (matching `msgpack-java` `MapValue` insertion semantics).
    pub(crate) fn opt(&self, key: &str) -> Option<&'a MsgPackValue> {
        self.entries.iter().rev().find_map(|(k, v)| match k {
            MsgPackValue::Str(s) if s == key => Some(v),
            _ => None,
        })
    }

    pub(crate) fn req(&self, key: &str) -> Result<&'a MsgPackValue, ExternalReaderError> {
        self.opt(key).ok_or_else(|| missing_param(key))
    }

    pub(crate) fn req_i64(&self, key: &str, code: i32) -> Result<i64, ExternalReaderError> {
        match self.req(key)? {
            MsgPackValue::Int(v) => Ok(*v),
            MsgPackValue::Uint(v) => i64::try_from(*v)
                .map_err(|_| malformed_body(code, format!("Integer overflow for `{key}`: {v}"))),
            other => Err(malformed_body(
                code,
                format!("Expected Integer for `{key}`, got {other:?}"),
            )),
        }
    }

    pub(crate) fn req_str(&self, key: &str, code: i32) -> Result<String, ExternalReaderError> {
        match self.req(key)? {
            MsgPackValue::Str(s) => Ok(s.clone()),
            other => Err(malformed_body(
                code,
                format!("Expected String for `{key}`, got {other:?}"),
            )),
        }
    }

    pub(crate) fn req_uri(&self, key: &str, code: i32) -> Result<String, ExternalReaderError> {
        let uri = self.req_str(key, code)?;
        validate_java_uri(&uri).map_err(|reason| malformed_body(code, reason))?;
        Ok(uri)
    }

    pub(crate) fn req_bool(&self, key: &str, code: i32) -> Result<bool, ExternalReaderError> {
        match self.req(key)? {
            MsgPackValue::Bool(b) => Ok(*b),
            other => Err(malformed_body(
                code,
                format!("Expected Boolean for `{key}`, got {other:?}"),
            )),
        }
    }

    pub(crate) fn opt_str(
        &self,
        key: &str,
        code: i32,
    ) -> Result<Option<String>, ExternalReaderError> {
        let Some(value) = self.opt(key) else {
            return Ok(None);
        };
        // Exact Java `AbstractMessagePackDecoder.unpackStringOrNull` behavior:
        // explicit `nil` is NOT treated as null and triggers `Malformed message body`.
        match value {
            MsgPackValue::Str(s) => Ok(Some(s.clone())),
            other => Err(malformed_body(
                code,
                format!("Expected String for `{key}`, got {other:?}"),
            )),
        }
    }

    pub(crate) fn opt_bin(
        &self,
        key: &str,
        code: i32,
    ) -> Result<Option<Vec<u8>>, ExternalReaderError> {
        let Some(value) = self.opt(key) else {
            return Ok(None);
        };
        match value {
            MsgPackValue::Bin(b) => Ok(Some(b.clone())),
            other => Err(malformed_body(
                code,
                format!("Expected Binary for `{key}`, got {other:?}"),
            )),
        }
    }
}

// ============================================================================
// Low-Level Streaming MessagePack Writer (matches msgpack-core compact rules)
// ============================================================================

pub(crate) struct MsgPackWriter<'a, W: Write> {
    out: &'a mut W,
}

impl<'a, W: Write> MsgPackWriter<'a, W> {
    pub(crate) fn new(out: &'a mut W) -> Self {
        Self { out }
    }

    fn write_all(&mut self, bytes: &[u8]) -> Result<(), ExternalReaderError> {
        self.out
            .write_all(bytes)
            .map_err(|e| ExternalReaderError::Io(e.to_string()))
    }

    pub(crate) fn flush(&mut self) -> Result<(), ExternalReaderError> {
        self.out
            .flush()
            .map_err(|e| ExternalReaderError::Io(e.to_string()))
    }

    fn len32(len: usize) -> Result<u32, ExternalReaderError> {
        u32::try_from(len)
            .map_err(|_| ExternalReaderError::Protocol("MessagePack length exceeds u32".into()))
    }

    pub(crate) fn pack_bool(&mut self, value: bool) -> Result<(), ExternalReaderError> {
        self.write_all(&[if value { 0xc3 } else { 0xc2 }])
    }

    pub(crate) fn pack_i32(&mut self, value: i32) -> Result<(), ExternalReaderError> {
        self.pack_i64(i64::from(value))
    }

    pub(crate) fn pack_i64(&mut self, value: i64) -> Result<(), ExternalReaderError> {
        if (0..=127).contains(&value) || (-32..=-1).contains(&value) {
            return self.write_all(&[value as u8]);
        }
        if value >= 0 {
            let v = value as u64;
            if v <= u64::from(u8::MAX) {
                self.write_all(&[0xcc, v as u8])
            } else if v <= u64::from(u16::MAX) {
                let b = (v as u16).to_be_bytes();
                self.write_all(&[0xcd, b[0], b[1]])
            } else if v <= u64::from(u32::MAX) {
                let b = (v as u32).to_be_bytes();
                self.write_all(&[0xce, b[0], b[1], b[2], b[3]])
            } else {
                self.write_all(&[0xcf])?;
                self.write_all(&v.to_be_bytes())
            }
        } else if value >= i64::from(i8::MIN) {
            self.write_all(&[0xd0, value as u8])
        } else if value >= i64::from(i16::MIN) {
            let b = (value as i16).to_be_bytes();
            self.write_all(&[0xd1, b[0], b[1]])
        } else if value >= i64::from(i32::MIN) {
            let b = (value as i32).to_be_bytes();
            self.write_all(&[0xd2, b[0], b[1], b[2], b[3]])
        } else {
            self.write_all(&[0xd3])?;
            self.write_all(&value.to_be_bytes())
        }
    }

    pub(crate) fn pack_array_header(&mut self, len: usize) -> Result<(), ExternalReaderError> {
        let len = Self::len32(len)?;
        if len <= 15 {
            self.write_all(&[0x90 | (len as u8)])
        } else if len <= u32::from(u16::MAX) {
            let b = (len as u16).to_be_bytes();
            self.write_all(&[0xdc, b[0], b[1]])
        } else {
            self.write_all(&[0xdd])?;
            self.write_all(&len.to_be_bytes())
        }
    }

    pub(crate) fn pack_map_header(&mut self, len: usize) -> Result<(), ExternalReaderError> {
        let len = Self::len32(len)?;
        if len <= 15 {
            self.write_all(&[0x80 | (len as u8)])
        } else if len <= u32::from(u16::MAX) {
            let b = (len as u16).to_be_bytes();
            self.write_all(&[0xde, b[0], b[1]])
        } else {
            self.write_all(&[0xdf])?;
            self.write_all(&len.to_be_bytes())
        }
    }

    pub(crate) fn pack_str(&mut self, s: &str) -> Result<(), ExternalReaderError> {
        let bytes = s.as_bytes();
        let len = Self::len32(bytes.len())?;
        if len <= 31 {
            self.write_all(&[0xa0 | (len as u8)])?;
        } else if len <= u32::from(u8::MAX) {
            self.write_all(&[0xd9, len as u8])?;
        } else if len <= u32::from(u16::MAX) {
            let b = (len as u16).to_be_bytes();
            self.write_all(&[0xda, b[0], b[1]])?;
        } else {
            self.write_all(&[0xdb])?;
            self.write_all(&len.to_be_bytes())?;
        }
        self.write_all(bytes)
    }

    pub(crate) fn pack_bin(&mut self, bytes: &[u8]) -> Result<(), ExternalReaderError> {
        let len = Self::len32(bytes.len())?;
        if len <= u32::from(u8::MAX) {
            self.write_all(&[0xc4, len as u8])?;
        } else if len <= u32::from(u16::MAX) {
            let b = (len as u16).to_be_bytes();
            self.write_all(&[0xc5, b[0], b[1]])?;
        } else {
            self.write_all(&[0xc6])?;
            self.write_all(&len.to_be_bytes())?;
        }
        self.write_all(bytes)
    }

    pub(crate) fn pack_kv_i64(&mut self, key: &str, value: i64) -> Result<(), ExternalReaderError> {
        self.pack_str(key)?;
        self.pack_i64(value)
    }

    pub(crate) fn pack_kv_str(
        &mut self,
        key: &str,
        value: &str,
    ) -> Result<(), ExternalReaderError> {
        self.pack_str(key)?;
        self.pack_str(value)
    }

    pub(crate) fn pack_kv_bin(
        &mut self,
        key: &str,
        value: &[u8],
    ) -> Result<(), ExternalReaderError> {
        self.pack_str(key)?;
        self.pack_bin(value)
    }

    pub(crate) fn pack_kv_bool(
        &mut self,
        key: &str,
        value: bool,
    ) -> Result<(), ExternalReaderError> {
        self.pack_str(key)?;
        self.pack_bool(value)
    }
}

// ============================================================================
// Low-Level Streaming MessagePack Reader
// ============================================================================

pub(crate) struct MsgPackReader<'a, R: Read> {
    input: &'a mut R,
    decoded_bytes: usize,
    decoded_nodes: usize,
}

impl<'a, R: Read> MsgPackReader<'a, R> {
    pub(crate) fn new(input: &'a mut R) -> Self {
        Self {
            input,
            decoded_bytes: 0,
            decoded_nodes: 0,
        }
    }

    pub(crate) fn read_first_byte_or_eof(&mut self) -> Result<Option<u8>, ExternalReaderError> {
        let mut buf = [0u8; 1];
        loop {
            match self.input.read(&mut buf) {
                Ok(0) => return Ok(None),
                Ok(_) => return Ok(Some(buf[0])),
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(ExternalReaderError::Io(e.to_string())),
            }
        }
    }

    fn read_byte(&mut self) -> Result<u8, String> {
        let mut buf = [0u8; 1];
        self.read_exact(&mut buf)?;
        Ok(buf[0])
    }

    fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), String> {
        self.input
            .read_exact(buf)
            .map_err(|e| format!("Unexpected EOF or I/O error: {e}"))
    }

    fn read_u16(&mut self) -> Result<u16, String> {
        let mut buf = [0u8; 2];
        self.read_exact(&mut buf)?;
        Ok(u16::from_be_bytes(buf))
    }

    fn read_u32(&mut self) -> Result<u32, String> {
        let mut buf = [0u8; 4];
        self.read_exact(&mut buf)?;
        Ok(u32::from_be_bytes(buf))
    }

    fn read_u64(&mut self) -> Result<u64, String> {
        let mut buf = [0u8; 8];
        self.read_exact(&mut buf)?;
        Ok(u64::from_be_bytes(buf))
    }

    fn read_bytes(&mut self, len: usize) -> Result<Vec<u8>, String> {
        if len > MAX_PAYLOAD_BYTES {
            return Err(format!("MessagePack payload length {len} exceeds maximum"));
        }
        let next_total = self
            .decoded_bytes
            .checked_add(len)
            .ok_or_else(|| "MessagePack cumulative message byte limit exceeded".to_string())?;
        if next_total > MAX_MESSAGE_TOTAL_BYTES {
            return Err(format!(
                "MessagePack cumulative payload size {next_total} exceeds maximum of {MAX_MESSAGE_TOTAL_BYTES} bytes"
            ));
        }
        // Read incrementally in bounded chunks so a truncated frame declaring a huge
        // length header cannot trigger an immediate multi-hundred-MiB allocation.
        let mut buf = Vec::with_capacity(len.min(MSGPACK_READ_CHUNK_BYTES));
        let mut remaining = len;
        let mut chunk = [0u8; MSGPACK_READ_CHUNK_BYTES];
        while remaining > 0 {
            let want = remaining.min(chunk.len());
            let n = loop {
                match self.input.read(&mut chunk[..want]) {
                    Ok(n) => break n,
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(e) => return Err(format!("Unexpected EOF or I/O error: {e}")),
                }
            };
            if n == 0 {
                return Err("Unexpected EOF or I/O error: failed to fill whole buffer".into());
            }
            buf.try_reserve(n)
                .map_err(|_| "MessagePack payload allocation failed".to_string())?;
            buf.extend_from_slice(&chunk[..n]);
            remaining -= n;
        }
        self.decoded_bytes = next_total;
        Ok(buf)
    }

    pub(crate) fn unpack_array_header_from_byte(&mut self, b: u8) -> Result<usize, String> {
        match b {
            0x90..=0x9f => Ok(usize::from(b & 0x0f)),
            0xdc => Ok(usize::from(self.read_u16()?)),
            0xdd => Ok(self.read_u32()? as usize),
            _ => Err(format!(
                "Expected Array, but got {} ({b:02x})",
                format_type_name(b)
            )),
        }
    }

    pub(crate) fn unpack_i32(&mut self) -> Result<i32, String> {
        let b = self.read_byte()?;
        match self.unpack_int_from_byte(b)? {
            IntValue::Signed(v) => {
                i32::try_from(v).map_err(|_| format!("Integer overflow for i32: {v}"))
            }
            IntValue::Unsigned(v) => {
                i32::try_from(v).map_err(|_| format!("Integer overflow for i32: {v}"))
            }
        }
    }

    fn unpack_int_from_byte(&mut self, b: u8) -> Result<IntValue, String> {
        if b <= 0x7f {
            return Ok(IntValue::Signed(i64::from(b)));
        }
        if b >= 0xe0 {
            return Ok(IntValue::Signed(i64::from(b as i8)));
        }
        match b {
            0xcc => Ok(IntValue::Unsigned(u64::from(self.read_byte()?))),
            0xcd => Ok(IntValue::Unsigned(u64::from(self.read_u16()?))),
            0xce => Ok(IntValue::Unsigned(u64::from(self.read_u32()?))),
            0xcf => Ok(IntValue::Unsigned(self.read_u64()?)),
            0xd0 => Ok(IntValue::Signed(i64::from(self.read_byte()? as i8))),
            0xd1 => {
                let mut buf = [0u8; 2];
                self.read_exact(&mut buf)?;
                Ok(IntValue::Signed(i64::from(i16::from_be_bytes(buf))))
            }
            0xd2 => {
                let mut buf = [0u8; 4];
                self.read_exact(&mut buf)?;
                Ok(IntValue::Signed(i64::from(i32::from_be_bytes(buf))))
            }
            0xd3 => {
                let mut buf = [0u8; 8];
                self.read_exact(&mut buf)?;
                Ok(IntValue::Signed(i64::from_be_bytes(buf)))
            }
            _ => Err(format!(
                "Expected Integer, but got {} ({b:02x})",
                format_type_name(b)
            )),
        }
    }

    pub(crate) fn unpack_value(&mut self, depth: usize) -> Result<MsgPackValue, String> {
        if depth > MAX_NESTING_DEPTH {
            return Err("MessagePack nesting depth exceeded".into());
        }
        self.decoded_nodes = self.decoded_nodes.saturating_add(1);
        if self.decoded_nodes > MAX_MESSAGE_NODES {
            return Err(format!(
                "MessagePack value node count exceeds maximum of {MAX_MESSAGE_NODES}"
            ));
        }
        let b = self.read_byte()?;
        match b {
            0x00..=0x7f => Ok(MsgPackValue::Int(i64::from(b))),
            0x80..=0x8f => self.unpack_map(usize::from(b & 0x0f), depth + 1),
            0x90..=0x9f => self.unpack_array(usize::from(b & 0x0f), depth + 1),
            0xa0..=0xbf => self.unpack_str(usize::from(b & 0x1f)),
            0xc0 => Ok(MsgPackValue::Nil),
            0xc1 => Err("Invalid MessagePack format byte 0xc1 (NEVER_USED)".into()),
            0xc2 => Ok(MsgPackValue::Bool(false)),
            0xc3 => Ok(MsgPackValue::Bool(true)),
            0xc4 => {
                let len = usize::from(self.read_byte()?);
                Ok(MsgPackValue::Bin(self.read_bytes(len)?))
            }
            0xc5 => {
                let len = usize::from(self.read_u16()?);
                Ok(MsgPackValue::Bin(self.read_bytes(len)?))
            }
            0xc6 => {
                let len = self.read_u32()? as usize;
                Ok(MsgPackValue::Bin(self.read_bytes(len)?))
            }
            0xc7 => {
                let len = usize::from(self.read_byte()?);
                let ty = self.read_byte()? as i8;
                Ok(MsgPackValue::Ext(ty, self.read_bytes(len)?))
            }
            0xc8 => {
                let len = usize::from(self.read_u16()?);
                let ty = self.read_byte()? as i8;
                Ok(MsgPackValue::Ext(ty, self.read_bytes(len)?))
            }
            0xc9 => {
                let len = self.read_u32()? as usize;
                let ty = self.read_byte()? as i8;
                Ok(MsgPackValue::Ext(ty, self.read_bytes(len)?))
            }
            0xca => {
                let mut buf = [0u8; 4];
                self.read_exact(&mut buf)?;
                Ok(MsgPackValue::Float(f64::from(f32::from_be_bytes(buf))))
            }
            0xcb => {
                let mut buf = [0u8; 8];
                self.read_exact(&mut buf)?;
                Ok(MsgPackValue::Float(f64::from_be_bytes(buf)))
            }
            0xcc..=0xd3 => match self.unpack_int_from_byte(b)? {
                IntValue::Signed(v) => Ok(MsgPackValue::Int(v)),
                IntValue::Unsigned(v) => Ok(MsgPackValue::Uint(v)),
            },
            0xd4 => self.unpack_fixext(1),
            0xd5 => self.unpack_fixext(2),
            0xd6 => self.unpack_fixext(4),
            0xd7 => self.unpack_fixext(8),
            0xd8 => self.unpack_fixext(16),
            0xd9 => {
                let len = usize::from(self.read_byte()?);
                self.unpack_str(len)
            }
            0xda => {
                let len = usize::from(self.read_u16()?);
                self.unpack_str(len)
            }
            0xdb => {
                let len = self.read_u32()? as usize;
                self.unpack_str(len)
            }
            0xdc => {
                let len = usize::from(self.read_u16()?);
                self.unpack_array(len, depth + 1)
            }
            0xdd => {
                let len = self.read_u32()? as usize;
                self.unpack_array(len, depth + 1)
            }
            0xde => {
                let len = usize::from(self.read_u16()?);
                self.unpack_map(len, depth + 1)
            }
            0xdf => {
                let len = self.read_u32()? as usize;
                self.unpack_map(len, depth + 1)
            }
            0xe0..=0xff => Ok(MsgPackValue::Int(i64::from(b as i8))),
        }
    }

    fn unpack_fixext(&mut self, len: usize) -> Result<MsgPackValue, String> {
        let ty = self.read_byte()? as i8;
        Ok(MsgPackValue::Ext(ty, self.read_bytes(len)?))
    }

    fn unpack_str(&mut self, len: usize) -> Result<MsgPackValue, String> {
        let bytes = self.read_bytes(len)?;
        let s = String::from_utf8(bytes)
            .map_err(|e| format!("Invalid UTF-8 in MessagePack string: {e}"))?;
        Ok(MsgPackValue::Str(s))
    }

    fn unpack_array(&mut self, len: usize, depth: usize) -> Result<MsgPackValue, String> {
        if len > MAX_COLLECTION_ELEMENTS {
            return Err(format!("MessagePack array length {len} exceeds maximum"));
        }
        let mut items = Vec::with_capacity(len.min(1024));
        for _ in 0..len {
            items.push(self.unpack_value(depth)?);
        }
        Ok(MsgPackValue::Array(items))
    }

    fn unpack_map(&mut self, len: usize, depth: usize) -> Result<MsgPackValue, String> {
        if len > MAX_COLLECTION_ELEMENTS {
            return Err(format!("MessagePack map length {len} exceeds maximum"));
        }
        let mut entries = Vec::with_capacity(len.min(1024));
        for _ in 0..len {
            let k = self.unpack_value(depth)?;
            let v = self.unpack_value(depth)?;
            entries.push((k, v));
        }
        Ok(MsgPackValue::Map(entries))
    }
}

enum IntValue {
    Signed(i64),
    Unsigned(u64),
}

/// Human-readable MessagePack value type name matching `msgpack-java` `ValueType`.
const fn format_type_name(b: u8) -> &'static str {
    match b {
        0x00..=0x7f | 0xcc..=0xd3 | 0xe0..=0xff => "Integer",
        0x80..=0x8f | 0xde | 0xdf => "Map",
        0x90..=0x9f | 0xdc | 0xdd => "Array",
        0xa0..=0xbf | 0xd9..=0xdb => "String",
        0xc0 => "Nil",
        0xc1 => "NeverUsed",
        0xc2 | 0xc3 => "Boolean",
        0xc4..=0xc6 => "Binary",
        0xc7..=0xc9 | 0xd4..=0xd8 => "Extension",
        0xca | 0xcb => "Float",
    }
}

#[cfg(test)]
mod security_tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn msgpack_reader_rejects_oversized_and_truncated_bin32_without_oom() {
        // 5-byte frame declaring a 256 MiB + 1 byte `bin32` inside a 2-element array:
        // [0x26, bin32(256 MiB + 1)]
        let over_limit = [0x92u8, 0x26, 0xc6, 0x10, 0x00, 0x00, 0x01];
        let mut cur = Cursor::new(&over_limit[..]);
        let err = read_message(&mut cur).expect_err("oversized bin32 must be rejected");
        assert!(format!("{err:?}").contains("exceeds") || format!("{err:?}").contains("Decode"));

        // 5-byte frame declaring a 200 MiB `bin32` with only 4 bytes of payload:
        // must fail incrementally at EOF without pre-allocating 200 MiB.
        let truncated_huge = [0x92u8, 0x26, 0xc6, 0x0c, 0x80, 0x00, 0x00, 1, 2, 3, 4];
        let mut cur2 = Cursor::new(&truncated_huge[..]);
        assert!(read_message(&mut cur2).is_err());
    }
}
