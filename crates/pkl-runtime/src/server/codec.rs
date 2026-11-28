//! Pure-Rust MessagePack wire codec for Pkl 0.32.0 `pkl server` (`0x20`–`0x2d`).
//!
//! Matches:
//! - `org.pkl.server.ServerMessagePackEncoder`
//! - `org.pkl.server.ServerMessagePackDecoder`
//! - `org.pkl.core.messaging.{Abstract,Base}MessagePack{Encoder,Decoder}`
//!
//! Reuses the low-level streaming MessagePack primitives from
//! `crate::external_reader::codec` with zero `unsafe` and zero extra dependencies.

use super::messages::{
    ServerChecksums, ServerDependency, ServerExternalReader, ServerHttp, ServerMessage,
    ServerProject, ServerProxy,
};
use crate::external_reader::codec::{
    MapView, MsgPackReader, MsgPackValue, MsgPackWriter, malformed_body, validate_java_uri,
};
use crate::external_reader::{
    ExternalReaderError, MessageType, ModuleReaderSpec, ResourceReaderSpec, WirePathElement,
};
use crate::logging::TraceMode;
use std::collections::BTreeMap;
use std::io::{Cursor, Read, Write};

/// Encode a single server message into a newly allocated MessagePack buffer.
pub fn encode_to_vec(msg: &ServerMessage) -> Result<Vec<u8>, ExternalReaderError> {
    let mut buf = Vec::with_capacity(64);
    write_message(&mut buf, msg)?;
    Ok(buf)
}

/// Encode and flush a single server message onto any `std::io::Write` sink.
///
/// Wire format: `[code: int, body: map]` (2-element MessagePack array, no length prefix).
/// Optional fields whose value is `None` are omitted from the body map.
pub fn write_message<W: Write>(
    writer: &mut W,
    msg: &ServerMessage,
) -> Result<(), ExternalReaderError> {
    let mut packer = MsgPackWriter::new(writer);
    packer.pack_array_header(2)?;
    packer.pack_i32(msg.message_type().code())?;
    encode_body(&mut packer, msg)?;
    packer.flush()
}

/// Decode the next server message (`0x20`–`0x2d`) from any `std::io::Read` stream.
///
/// Returns `Ok(None)` on clean EOF before the first byte of the next message.
/// External-reader-only codes (`0x2e`–`0x32`) fail with exact upstream
/// ``Unhandled decoding message code `<hex>`.`` (`AbstractMessagePackDecoder.java:82-83`).
pub fn read_message<R: Read>(reader: &mut R) -> Result<Option<ServerMessage>, ExternalReaderError> {
    let mut unpacker = MsgPackReader::new(reader);
    let Some(first_byte) = unpacker.read_first_byte_or_eof()? else {
        return Ok(None);
    };

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

    let Some(msg_type) = MessageType::from_code(code) else {
        return Err(ExternalReaderError::Decode {
            message: format!(
                "Malformed message header (unrecognized code `{:x}`).",
                code as u32
            ),
            cause: None,
        });
    };

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

    let map = MapView::new(&entries);
    match decode_body(msg_type, code, &map)? {
        Some(message) => Ok(Some(message)),
        None => Err(ExternalReaderError::Decode {
            message: format!("Unhandled decoding message code `{:x}`.", code as u32),
            cause: None,
        }),
    }
}

/// Decode a single server message from an in-memory byte slice.
pub fn decode_from_slice(bytes: &[u8]) -> Result<Option<ServerMessage>, ExternalReaderError> {
    let mut cursor = Cursor::new(bytes);
    read_message(&mut cursor)
}

/// Decode all concatenated server messages from an in-memory byte slice until clean EOF.
pub fn decode_all_from_slice(bytes: &[u8]) -> Result<Vec<ServerMessage>, ExternalReaderError> {
    let mut cursor = Cursor::new(bytes);
    let mut out = Vec::new();
    while let Some(msg) = read_message(&mut cursor)? {
        out.push(msg);
    }
    Ok(out)
}

// ============================================================================
// Body Encoding (exact field order matching ServerMessagePackEncoder.kt)
// ============================================================================

fn encode_body<W: Write>(
    packer: &mut MsgPackWriter<'_, W>,
    msg: &ServerMessage,
) -> Result<(), ExternalReaderError> {
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
            let count = 1
                + usize::from(allowed_modules.is_some())
                + usize::from(allowed_resources.is_some())
                + usize::from(client_module_readers.is_some())
                + usize::from(client_resource_readers.is_some())
                + usize::from(module_paths.is_some())
                + usize::from(env.is_some())
                + usize::from(properties.is_some())
                + usize::from(timeout_seconds.is_some())
                + usize::from(root_dir.is_some())
                + usize::from(cache_dir.is_some())
                + usize::from(output_format.is_some())
                + usize::from(project.is_some())
                + usize::from(http.is_some())
                + usize::from(external_module_readers.is_some())
                + usize::from(external_resource_readers.is_some())
                + usize::from(trace_mode.is_some());
            packer.pack_map_header(count)?;
            packer.pack_kv_i64("requestId", *request_id)?;
            if let Some(list) = allowed_modules {
                pack_kv_str_list(packer, "allowedModules", list)?;
            }
            if let Some(list) = allowed_resources {
                pack_kv_str_list(packer, "allowedResources", list)?;
            }
            if let Some(readers) = client_module_readers {
                packer.pack_str("clientModuleReaders")?;
                packer.pack_array_header(readers.len())?;
                for spec in readers {
                    pack_module_reader_spec(packer, spec)?;
                }
            }
            if let Some(readers) = client_resource_readers {
                packer.pack_str("clientResourceReaders")?;
                packer.pack_array_header(readers.len())?;
                for spec in readers {
                    pack_resource_reader_spec(packer, spec)?;
                }
            }
            if let Some(paths) = module_paths {
                pack_kv_str_list(packer, "modulePaths", paths)?;
            }
            if let Some(map) = env {
                pack_kv_str_map(packer, "env", map)?;
            }
            if let Some(map) = properties {
                pack_kv_str_map(packer, "properties", map)?;
            }
            if let Some(secs) = timeout_seconds {
                packer.pack_kv_i64("timeoutSeconds", *secs)?;
            }
            if let Some(dir) = root_dir {
                packer.pack_kv_str("rootDir", dir)?;
            }
            if let Some(dir) = cache_dir {
                packer.pack_kv_str("cacheDir", dir)?;
            }
            if let Some(fmt) = output_format {
                packer.pack_kv_str("outputFormat", fmt)?;
            }
            if let Some(proj) = project {
                packer.pack_str("project")?;
                pack_project(packer, proj)?;
            }
            if let Some(h) = http {
                packer.pack_str("http")?;
                pack_http(packer, h)?;
            }
            if let Some(readers) = external_module_readers {
                packer.pack_str("externalModuleReaders")?;
                packer.pack_map_header(readers.len())?;
                for (scheme, spec) in readers {
                    packer.pack_str(scheme)?;
                    pack_external_reader(packer, spec)?;
                }
            }
            if let Some(readers) = external_resource_readers {
                packer.pack_str("externalResourceReaders")?;
                packer.pack_map_header(readers.len())?;
                for (scheme, spec) in readers {
                    packer.pack_str(scheme)?;
                    pack_external_reader(packer, spec)?;
                }
            }
            if let Some(mode) = trace_mode {
                let s = match mode {
                    TraceMode::Compact => "COMPACT",
                    TraceMode::Pretty => "PRETTY",
                };
                packer.pack_kv_str("traceMode", s)?;
            }
        }
        ServerMessage::CreateEvaluatorResponse {
            request_id,
            evaluator_id,
            error,
        } => {
            let count = 1 + usize::from(evaluator_id.is_some()) + usize::from(error.is_some());
            packer.pack_map_header(count)?;
            packer.pack_kv_i64("requestId", *request_id)?;
            if let Some(id) = evaluator_id {
                packer.pack_kv_i64("evaluatorId", *id)?;
            }
            if let Some(err) = error {
                packer.pack_kv_str("error", err)?;
            }
        }
        ServerMessage::CloseEvaluator { evaluator_id } => {
            packer.pack_map_header(1)?;
            packer.pack_kv_i64("evaluatorId", *evaluator_id)?;
        }
        ServerMessage::EvaluateRequest {
            request_id,
            evaluator_id,
            module_uri,
            module_text,
            expr,
        } => {
            let count = 3 + usize::from(module_text.is_some()) + usize::from(expr.is_some());
            packer.pack_map_header(count)?;
            packer.pack_kv_i64("requestId", *request_id)?;
            packer.pack_kv_i64("evaluatorId", *evaluator_id)?;
            packer.pack_kv_str("moduleUri", module_uri)?;
            if let Some(text) = module_text {
                packer.pack_kv_str("moduleText", text)?;
            }
            if let Some(expression) = expr {
                packer.pack_kv_str("expr", expression)?;
            }
        }
        ServerMessage::EvaluateResponse {
            request_id,
            evaluator_id,
            result,
            error,
        } => {
            let count = 2 + usize::from(result.is_some()) + usize::from(error.is_some());
            packer.pack_map_header(count)?;
            packer.pack_kv_i64("requestId", *request_id)?;
            packer.pack_kv_i64("evaluatorId", *evaluator_id)?;
            if let Some(bytes) = result {
                packer.pack_kv_bin("result", bytes)?;
            }
            if let Some(err) = error {
                packer.pack_kv_str("error", err)?;
            }
        }
        ServerMessage::LogMessage {
            evaluator_id,
            level,
            message,
            frame_uri,
        } => {
            packer.pack_map_header(4)?;
            packer.pack_kv_i64("evaluatorId", *evaluator_id)?;
            packer.pack_str("level")?;
            packer.pack_i32(*level)?;
            packer.pack_kv_str("message", message)?;
            packer.pack_kv_str("frameUri", frame_uri)?;
        }
        ServerMessage::ReadResourceRequest {
            request_id,
            evaluator_id,
            uri,
        } => {
            packer.pack_map_header(3)?;
            packer.pack_kv_i64("requestId", *request_id)?;
            packer.pack_kv_i64("evaluatorId", *evaluator_id)?;
            packer.pack_kv_str("uri", uri)?;
        }
        ServerMessage::ReadResourceResponse {
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
        ServerMessage::ReadModuleRequest {
            request_id,
            evaluator_id,
            uri,
        } => {
            packer.pack_map_header(3)?;
            packer.pack_kv_i64("requestId", *request_id)?;
            packer.pack_kv_i64("evaluatorId", *evaluator_id)?;
            packer.pack_kv_str("uri", uri)?;
        }
        ServerMessage::ReadModuleResponse {
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
        ServerMessage::ListResourcesRequest {
            request_id,
            evaluator_id,
            uri,
        } => {
            packer.pack_map_header(3)?;
            packer.pack_kv_i64("requestId", *request_id)?;
            packer.pack_kv_i64("evaluatorId", *evaluator_id)?;
            packer.pack_kv_str("uri", uri)?;
        }
        ServerMessage::ListResourcesResponse {
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
        ServerMessage::ListModulesRequest {
            request_id,
            evaluator_id,
            uri,
        } => {
            packer.pack_map_header(3)?;
            packer.pack_kv_i64("requestId", *request_id)?;
            packer.pack_kv_i64("evaluatorId", *evaluator_id)?;
            packer.pack_kv_str("uri", uri)?;
        }
        ServerMessage::ListModulesResponse {
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
    }
    Ok(())
}

fn pack_kv_str_list<W: Write>(
    packer: &mut MsgPackWriter<'_, W>,
    key: &str,
    items: &[String],
) -> Result<(), ExternalReaderError> {
    packer.pack_str(key)?;
    packer.pack_array_header(items.len())?;
    for item in items {
        packer.pack_str(item)?;
    }
    Ok(())
}

fn pack_kv_str_map<W: Write>(
    packer: &mut MsgPackWriter<'_, W>,
    key: &str,
    map: &BTreeMap<String, String>,
) -> Result<(), ExternalReaderError> {
    packer.pack_str(key)?;
    packer.pack_map_header(map.len())?;
    for (k, v) in map {
        packer.pack_str(k)?;
        packer.pack_str(v)?;
    }
    Ok(())
}

fn pack_project<W: Write>(
    packer: &mut MsgPackWriter<'_, W>,
    project: &ServerProject,
) -> Result<(), ExternalReaderError> {
    packer.pack_map_header(2)?;
    packer.pack_kv_str("projectFileUri", &project.project_file_uri)?;
    packer.pack_str("dependencies")?;
    pack_dependencies(packer, &project.dependencies)?;
    Ok(())
}

fn pack_dependencies<W: Write>(
    packer: &mut MsgPackWriter<'_, W>,
    deps: &BTreeMap<String, ServerDependency>,
) -> Result<(), ExternalReaderError> {
    packer.pack_map_header(deps.len())?;
    for (name, dep) in deps {
        packer.pack_str(name)?;
        match dep {
            ServerDependency::Local(proj) => {
                packer.pack_map_header(4)?;
                packer.pack_kv_str("type", "local")?;
                packer.pack_kv_str("packageUri", proj.package_uri.as_deref().unwrap_or("null"))?;
                packer.pack_kv_str("projectFileUri", &proj.project_file_uri)?;
                packer.pack_str("dependencies")?;
                pack_dependencies(packer, &proj.dependencies)?;
            }
            ServerDependency::Remote {
                package_uri,
                checksums,
            } => {
                packer.pack_map_header(if checksums.is_some() { 3 } else { 2 })?;
                packer.pack_kv_str("type", "remote")?;
                packer.pack_kv_str("packageUri", package_uri)?;
                if let Some(cs) = checksums {
                    packer.pack_str("checksums")?;
                    packer.pack_map_header(1)?;
                    packer.pack_kv_str("sha256", &cs.sha256)?;
                }
            }
        }
    }
    Ok(())
}

fn pack_http<W: Write>(
    packer: &mut MsgPackWriter<'_, W>,
    http: &ServerHttp,
) -> Result<(), ExternalReaderError> {
    let count = usize::from(http.ca_certificates.is_some())
        + usize::from(http.proxy.is_some())
        + usize::from(http.rewrites.is_some())
        + usize::from(http.headers.is_some());
    packer.pack_map_header(count)?;
    if let Some(ca) = &http.ca_certificates {
        packer.pack_kv_bin("caCertificates", ca)?;
    }
    if let Some(proxy) = &http.proxy {
        packer.pack_str("proxy")?;
        let pcount = usize::from(proxy.address.is_some()) + usize::from(proxy.no_proxy.is_some());
        packer.pack_map_header(pcount)?;
        if let Some(addr) = &proxy.address {
            packer.pack_kv_str("address", addr)?;
        }
        if let Some(no_proxy) = &proxy.no_proxy {
            pack_kv_str_list(packer, "noProxy", no_proxy)?;
        }
    }
    if let Some(rewrites) = &http.rewrites {
        pack_kv_str_map(packer, "rewrites", rewrites)?;
    }
    if let Some(headers) = &http.headers {
        packer.pack_str("headers")?;
        packer.pack_map_header(headers.len())?;
        for (pattern, header_map) in headers {
            packer.pack_str(pattern)?;
            packer.pack_map_header(header_map.len())?;
            for (name, values) in header_map {
                packer.pack_str(name)?;
                packer.pack_array_header(values.len())?;
                for elem in values {
                    packer.pack_str(elem)?;
                }
            }
        }
    }
    Ok(())
}

fn pack_external_reader<W: Write>(
    packer: &mut MsgPackWriter<'_, W>,
    spec: &ServerExternalReader,
) -> Result<(), ExternalReaderError> {
    let count = 1 + usize::from(spec.arguments.is_some()) + usize::from(spec.working_dir.is_some());
    packer.pack_map_header(count)?;
    packer.pack_kv_str("executable", &spec.executable)?;
    if let Some(args) = &spec.arguments {
        pack_kv_str_list(packer, "arguments", args)?;
    }
    if let Some(wd) = &spec.working_dir {
        packer.pack_kv_str("workingDir", wd)?;
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
// Body Decoding (exact validation & error rules matching ServerMessagePackDecoder.kt)
// ============================================================================

fn decode_body(
    msg_type: MessageType,
    code: i32,
    map: &MapView<'_>,
) -> Result<Option<ServerMessage>, ExternalReaderError> {
    let msg = match msg_type {
        MessageType::CreateEvaluatorRequest => ServerMessage::CreateEvaluatorRequest {
            request_id: map.req_i64("requestId", code)?,
            allowed_modules: opt_str_list(map, "allowedModules", code)?,
            allowed_resources: opt_str_list(map, "allowedResources", code)?,
            client_module_readers: opt_module_reader_specs(map, "clientModuleReaders", code)?,
            client_resource_readers: opt_resource_reader_specs(map, "clientResourceReaders", code)?,
            module_paths: opt_str_list(map, "modulePaths", code)?,
            env: opt_str_map(map, "env", code)?,
            properties: opt_str_map(map, "properties", code)?,
            timeout_seconds: opt_i64(map, "timeoutSeconds", code)?,
            root_dir: map.opt_str("rootDir", code)?,
            cache_dir: map.opt_str("cacheDir", code)?,
            output_format: map.opt_str("outputFormat", code)?,
            project: unpack_project(map, code)?,
            http: unpack_http(map, code)?,
            external_module_readers: unpack_external_readers(map, "externalModuleReaders", code)?,
            external_resource_readers: unpack_external_readers(
                map,
                "externalResourceReaders",
                code,
            )?,
            trace_mode: unpack_trace_mode(map, code)?,
        },
        MessageType::CreateEvaluatorResponse => ServerMessage::CreateEvaluatorResponse {
            request_id: map.req_i64("requestId", code)?,
            evaluator_id: opt_i64(map, "evaluatorId", code)?,
            error: map.opt_str("error", code)?,
        },
        MessageType::CloseEvaluator => ServerMessage::CloseEvaluator {
            evaluator_id: map.req_i64("evaluatorId", code)?,
        },
        MessageType::EvaluateRequest => ServerMessage::EvaluateRequest {
            request_id: map.req_i64("requestId", code)?,
            evaluator_id: map.req_i64("evaluatorId", code)?,
            module_uri: map.req_uri("moduleUri", code)?,
            module_text: map.opt_str("moduleText", code)?,
            expr: map.opt_str("expr", code)?,
        },
        MessageType::EvaluateResponse => ServerMessage::EvaluateResponse {
            request_id: map.req_i64("requestId", code)?,
            evaluator_id: map.req_i64("evaluatorId", code)?,
            result: map.opt_bin("result", code)?,
            error: map.opt_str("error", code)?,
        },
        MessageType::LogMessage => ServerMessage::LogMessage {
            evaluator_id: map.req_i64("evaluatorId", code)?,
            level: req_i32(map, "level", code)?,
            message: map.req_str("message", code)?,
            frame_uri: map.req_str("frameUri", code)?,
        },
        MessageType::ReadResourceRequest => ServerMessage::ReadResourceRequest {
            request_id: map.req_i64("requestId", code)?,
            evaluator_id: map.req_i64("evaluatorId", code)?,
            uri: map.req_uri("uri", code)?,
        },
        MessageType::ReadResourceResponse => ServerMessage::ReadResourceResponse {
            request_id: map.req_i64("requestId", code)?,
            evaluator_id: map.req_i64("evaluatorId", code)?,
            contents: map.opt_bin("contents", code)?,
            error: map.opt_str("error", code)?,
        },
        MessageType::ReadModuleRequest => ServerMessage::ReadModuleRequest {
            request_id: map.req_i64("requestId", code)?,
            evaluator_id: map.req_i64("evaluatorId", code)?,
            uri: map.req_uri("uri", code)?,
        },
        MessageType::ReadModuleResponse => ServerMessage::ReadModuleResponse {
            request_id: map.req_i64("requestId", code)?,
            evaluator_id: map.req_i64("evaluatorId", code)?,
            contents: map.opt_str("contents", code)?,
            error: map.opt_str("error", code)?,
        },
        MessageType::ListResourcesRequest => ServerMessage::ListResourcesRequest {
            request_id: map.req_i64("requestId", code)?,
            evaluator_id: map.req_i64("evaluatorId", code)?,
            uri: map.req_uri("uri", code)?,
        },
        MessageType::ListResourcesResponse => ServerMessage::ListResourcesResponse {
            request_id: map.req_i64("requestId", code)?,
            evaluator_id: map.req_i64("evaluatorId", code)?,
            path_elements: unpack_path_elements(map.opt("pathElements"), code)?,
            error: map.opt_str("error", code)?,
        },
        MessageType::ListModulesRequest => ServerMessage::ListModulesRequest {
            request_id: map.req_i64("requestId", code)?,
            evaluator_id: map.req_i64("evaluatorId", code)?,
            uri: map.req_uri("uri", code)?,
        },
        MessageType::ListModulesResponse => ServerMessage::ListModulesResponse {
            request_id: map.req_i64("requestId", code)?,
            evaluator_id: map.req_i64("evaluatorId", code)?,
            path_elements: unpack_path_elements(map.opt("pathElements"), code)?,
            error: map.opt_str("error", code)?,
        },
        // External-reader-only codes (`0x2e`–`0x32`): recognized by MessageType
        // but rejected by `ServerMessagePackDecoder` with `unhandledMessageCode`.
        MessageType::InitializeModuleReaderRequest
        | MessageType::InitializeModuleReaderResponse
        | MessageType::InitializeResourceReaderRequest
        | MessageType::InitializeResourceReaderResponse
        | MessageType::CloseExternalProcess => return Ok(None),
    };
    Ok(Some(msg))
}

fn req_i32(map: &MapView<'_>, key: &str, code: i32) -> Result<i32, ExternalReaderError> {
    let val = map.req_i64(key, code)?;
    i32::try_from(val).map_err(|_| malformed_body(code, format!("Integer overflow for `{key}`")))
}

fn opt_i64(map: &MapView<'_>, key: &str, code: i32) -> Result<Option<i64>, ExternalReaderError> {
    let Some(value) = map.opt(key) else {
        return Ok(None);
    };
    match value {
        MsgPackValue::Int(v) => Ok(Some(*v)),
        MsgPackValue::Uint(v) => i64::try_from(*v)
            .map(Some)
            .map_err(|_| malformed_body(code, format!("Integer overflow for `{key}`: {v}"))),
        other => Err(malformed_body(
            code,
            format!("Expected Integer for `{key}`, got {other:?}"),
        )),
    }
}

fn opt_str_list(
    map: &MapView<'_>,
    key: &str,
    code: i32,
) -> Result<Option<Vec<String>>, ExternalReaderError> {
    let Some(value) = map.opt(key) else {
        return Ok(None);
    };
    let MsgPackValue::Array(items) = value else {
        return Err(malformed_body(code, format!("Expected Array for `{key}`.")));
    };
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        let MsgPackValue::Str(s) = item else {
            return Err(malformed_body(
                code,
                format!("Expected String element in `{key}`."),
            ));
        };
        out.push(s.clone());
    }
    Ok(Some(out))
}

fn opt_str_map(
    map: &MapView<'_>,
    key: &str,
    code: i32,
) -> Result<Option<BTreeMap<String, String>>, ExternalReaderError> {
    let Some(value) = map.opt(key) else {
        return Ok(None);
    };
    let MsgPackValue::Map(entries) = value else {
        return Err(malformed_body(code, format!("Expected Map for `{key}`.")));
    };
    let mut out = BTreeMap::new();
    for (k, v) in entries {
        let MsgPackValue::Str(ks) = k else {
            return Err(malformed_body(
                code,
                format!("Expected String key in `{key}`."),
            ));
        };
        let MsgPackValue::Str(vs) = v else {
            return Err(malformed_body(
                code,
                format!("Expected String value in `{key}`."),
            ));
        };
        out.insert(ks.clone(), vs.clone());
    }
    Ok(Some(out))
}

fn opt_module_reader_specs(
    map: &MapView<'_>,
    key: &str,
    code: i32,
) -> Result<Option<Vec<ModuleReaderSpec>>, ExternalReaderError> {
    let Some(value) = map.opt(key) else {
        return Ok(None);
    };
    let MsgPackValue::Array(items) = value else {
        return Err(malformed_body(code, format!("Expected Array for `{key}`.")));
    };
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        let MsgPackValue::Map(entries) = item else {
            return Err(malformed_body(
                code,
                format!("Expected Map item in `{key}`."),
            ));
        };
        let m = MapView::new(entries);
        out.push(ModuleReaderSpec {
            scheme: m.req_str("scheme", code)?,
            has_hierarchical_uris: m.req_bool("hasHierarchicalUris", code)?,
            is_local: m.req_bool("isLocal", code)?,
            is_globbable: m.req_bool("isGlobbable", code)?,
        });
    }
    Ok(Some(out))
}

fn opt_resource_reader_specs(
    map: &MapView<'_>,
    key: &str,
    code: i32,
) -> Result<Option<Vec<ResourceReaderSpec>>, ExternalReaderError> {
    let Some(value) = map.opt(key) else {
        return Ok(None);
    };
    let MsgPackValue::Array(items) = value else {
        return Err(malformed_body(code, format!("Expected Array for `{key}`.")));
    };
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        let MsgPackValue::Map(entries) = item else {
            return Err(malformed_body(
                code,
                format!("Expected Map item in `{key}`."),
            ));
        };
        let m = MapView::new(entries);
        out.push(ResourceReaderSpec {
            scheme: m.req_str("scheme", code)?,
            has_hierarchical_uris: m.req_bool("hasHierarchicalUris", code)?,
            is_globbable: m.req_bool("isGlobbable", code)?,
        });
    }
    Ok(Some(out))
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
        let m = MapView::new(entries);
        elements.push(WirePathElement {
            name: m.req_str("name", code)?,
            is_directory: m.req_bool("isDirectory", code)?,
        });
    }
    Ok(Some(elements))
}

fn unpack_project(
    map: &MapView<'_>,
    code: i32,
) -> Result<Option<ServerProject>, ExternalReaderError> {
    let Some(value) = map.opt("project") else {
        return Ok(None);
    };
    let MsgPackValue::Map(entries) = value else {
        return Err(malformed_body(code, "Expected Map for `project`."));
    };
    let proj_map = MapView::new(entries);
    let project_file_uri = proj_map.req_uri("projectFileUri", code)?;
    let dependencies = unpack_dependencies(&proj_map, "dependencies", code)?;
    Ok(Some(ServerProject {
        project_file_uri,
        package_uri: None,
        dependencies,
    }))
}

fn unpack_dependencies(
    map: &MapView<'_>,
    key: &str,
    code: i32,
) -> Result<BTreeMap<String, ServerDependency>, ExternalReaderError> {
    let value = map.req(key)?;
    let MsgPackValue::Map(entries) = value else {
        return Err(malformed_body(code, format!("Expected Map for `{key}`.")));
    };
    let mut out = BTreeMap::new();
    for (k, v) in entries {
        let MsgPackValue::Str(dep_name) = k else {
            return Err(malformed_body(
                code,
                format!("Expected String key in `{key}`."),
            ));
        };
        let MsgPackValue::Map(dep_entries) = v else {
            return Err(malformed_body(
                code,
                format!("Expected Map value for dependency `{dep_name}`."),
            ));
        };
        let dep_map = MapView::new(dep_entries);
        let dep_type = dep_map.req_str("type", code)?;
        let package_uri = dep_map.req_uri("packageUri", code)?;
        if dep_type == "remote" {
            let checksums = match dep_map.opt("checksums") {
                None => None,
                Some(MsgPackValue::Map(cs_entries)) => {
                    let cs_map = MapView::new(cs_entries);
                    let sha256 = cs_map.req_str("sha256", code)?;
                    Some(ServerChecksums { sha256 })
                }
                Some(_) => {
                    return Err(malformed_body(code, "Expected Map for `checksums`."));
                }
            };
            out.insert(
                dep_name.clone(),
                ServerDependency::Remote {
                    package_uri,
                    checksums,
                },
            );
        } else {
            let dependencies = unpack_dependencies(&dep_map, "dependencies", code)?;
            let project_file_uri = dep_map.req_uri("projectFileUri", code)?;
            out.insert(
                dep_name.clone(),
                ServerDependency::Local(ServerProject {
                    project_file_uri,
                    package_uri: Some(package_uri),
                    dependencies,
                }),
            );
        }
    }
    Ok(out)
}

fn unpack_http(map: &MapView<'_>, code: i32) -> Result<Option<ServerHttp>, ExternalReaderError> {
    let Some(value) = map.opt("http") else {
        return Ok(None);
    };
    let MsgPackValue::Map(entries) = value else {
        return Err(malformed_body(code, "Expected Map for `http`."));
    };
    let http_map = MapView::new(entries);
    let proxy = match http_map.opt("proxy") {
        None => None,
        Some(MsgPackValue::Map(p_entries)) => {
            let p_map = MapView::new(p_entries);
            // Exact match to `ServerMessagePackDecoder.kt:125-127`: `address` is required when `proxy` is present.
            let address = p_map.req_uri("address", code)?;
            let no_proxy = opt_str_list(&p_map, "noProxy", code)?;
            Some(ServerProxy {
                address: Some(address),
                no_proxy,
            })
        }
        Some(_) => return Err(malformed_body(code, "Expected Map for `proxy`.")),
    };
    let ca_certificates = http_map.opt_bin("caCertificates", code)?;
    let rewrites = match http_map.opt("rewrites") {
        None => None,
        Some(MsgPackValue::Map(rw_entries)) => {
            let mut out = BTreeMap::new();
            for (k, v) in rw_entries {
                let MsgPackValue::Str(ks) = k else {
                    return Err(malformed_body(code, "Expected String key in `rewrites`."));
                };
                let MsgPackValue::Str(vs) = v else {
                    return Err(malformed_body(code, "Expected String value in `rewrites`."));
                };
                validate_java_uri(ks).map_err(|reason| malformed_body(code, reason))?;
                validate_java_uri(vs).map_err(|reason| malformed_body(code, reason))?;
                out.insert(ks.clone(), vs.clone());
            }
            Some(out)
        }
        Some(_) => return Err(malformed_body(code, "Expected Map for `rewrites`.")),
    };
    let headers = match http_map.opt("headers") {
        None => None,
        Some(MsgPackValue::Map(h_entries)) => {
            let mut out = BTreeMap::new();
            for (pat_val, map_val) in h_entries {
                let MsgPackValue::Str(pattern) = pat_val else {
                    return Err(malformed_body(code, "Expected String key in `headers`."));
                };
                let MsgPackValue::Map(inner_entries) = map_val else {
                    return Err(malformed_body(code, "Expected Map value in `headers`."));
                };
                let mut inner = BTreeMap::new();
                for (name_val, list_val) in inner_entries {
                    let MsgPackValue::Str(name) = name_val else {
                        return Err(malformed_body(
                            code,
                            "Expected String header name in `headers`.",
                        ));
                    };
                    let MsgPackValue::Array(vals) = list_val else {
                        return Err(malformed_body(
                            code,
                            "Expected Array header values in `headers`.",
                        ));
                    };
                    let mut list = Vec::with_capacity(vals.len());
                    for item in vals {
                        let MsgPackValue::Str(s) = item else {
                            return Err(malformed_body(
                                code,
                                "Expected String header value in `headers`.",
                            ));
                        };
                        list.push(s.clone());
                    }
                    inner.insert(name.clone(), list);
                }
                out.insert(pattern.clone(), inner);
            }
            Some(out)
        }
        Some(_) => return Err(malformed_body(code, "Expected Map for `headers`.")),
    };
    Ok(Some(ServerHttp {
        ca_certificates,
        proxy,
        rewrites,
        headers,
    }))
}

fn unpack_external_readers(
    map: &MapView<'_>,
    key: &str,
    code: i32,
) -> Result<Option<BTreeMap<String, ServerExternalReader>>, ExternalReaderError> {
    let Some(value) = map.opt(key) else {
        return Ok(None);
    };
    let MsgPackValue::Map(entries) = value else {
        return Err(malformed_body(code, format!("Expected Map for `{key}`.")));
    };
    let mut out = BTreeMap::new();
    for (k, v) in entries {
        let MsgPackValue::Str(scheme) = k else {
            return Err(malformed_body(
                code,
                format!("Expected String key in `{key}`."),
            ));
        };
        let MsgPackValue::Map(spec_entries) = v else {
            return Err(malformed_body(
                code,
                format!("Expected Map value in `{key}`."),
            ));
        };
        let sm = MapView::new(spec_entries);
        out.insert(
            scheme.clone(),
            ServerExternalReader {
                executable: sm.req_str("executable", code)?,
                arguments: opt_str_list(&sm, "arguments", code)?,
                working_dir: sm.opt_str("workingDir", code)?,
            },
        );
    }
    Ok(Some(out))
}

fn unpack_trace_mode(
    map: &MapView<'_>,
    code: i32,
) -> Result<Option<TraceMode>, ExternalReaderError> {
    let Some(raw) = map.opt_str("traceMode", code)? else {
        return Ok(None);
    };
    match raw.to_ascii_uppercase().as_str() {
        "COMPACT" => Ok(Some(TraceMode::Compact)),
        "PRETTY" => Ok(Some(TraceMode::Pretty)),
        _ => Err(malformed_body(
            code,
            format!("Invalid traceMode `{raw}`: expected `compact` or `pretty`."),
        )),
    }
}
