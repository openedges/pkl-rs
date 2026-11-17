//! Explicitly experimental M1 adapter entry points, not the final public Pkl API.
use crate::source::SourceText;
use crate::{
    host::{HostPoll, HostSession},
    value::{ErrorKind, PklString, RuntimeError, Value},
    vm::{Instruction, Program, Vm, VmPoll},
};
use std::sync::Arc;

/// Reject JavaScript numeric coercion before narrowing experimental clock/budget arguments.
pub fn unsigned_argument(value: f64) -> Result<u32, RuntimeError> {
    if !value.is_finite() || value < 0.0 || value > u32::MAX as f64 || value.fract() != 0.0 {
        return Err(RuntimeError::new(
            ErrorKind::Host,
            "expected an integer in 0..=4294967295",
        ));
    }
    Ok(value as u32)
}

pub fn primitive(family: &str, input: &str) -> Result<String, String> {
    match family {
        "numeric" => crate::numeric::probe(input),
        "regex" => crate::regex::probe(input),
        _ => Err("unknown probe family".into()),
    }
}

pub fn identity(value: Value) -> Result<Value, RuntimeError> {
    if matches!(
        value,
        Value::Object(_) | Value::Function(_) | Value::Class(_) | Value::TypeAlias(_)
    ) {
        return Err(unsupported(
            "object handles cannot cross the probe boundary",
        ));
    }
    let mut vm = Vm::new(Arc::new(Program::new(vec![vec![
        Instruction::Input(0),
        Instruction::Return,
    ]])));
    vm.start(0, vec![value])?;
    finish(&mut vm)
}

/// Legacy diagnostic byte boundary: Java UTF8 encoding is explicit here.
pub fn evaluate_member(
    source: impl Into<SourceText>,
    member: impl Into<PklString>,
) -> Result<Value, String> {
    evaluate_member_text(source, member).map_err(|text| text.to_java_utf8())
}

/// Keep complete diagnostic units for JavaScript and native text callers.
pub fn evaluate_member_text(
    source: impl Into<SourceText>,
    member: impl Into<PklString>,
) -> Result<Value, PklString> {
    let source = source.into();
    let syntax = crate::parser::parse_module(&source).map_err(|e| e.message)?;
    let resolved = crate::resolver::resolve_module(&syntax).map_err(|e| e.message)?;
    let mut program = crate::vm::compile_module(&resolved).map_err(|e| e.message)?;
    let module_name = program.module_name.clone();
    Arc::make_mut(&mut program)
        .register_primary_source(
            Arc::new(crate::value::RuntimeSource {
                text: source,
                uri: "repl:text".into(),
                module_name,
            }),
            crate::modules::ModuleIdentity {
                original_uri: "repl:text".into(),
                resolved_uri: "repl:text".into(),
            },
        )
        .map_err(|e| e.message)?;
    let entry = program.entry;
    let mut vm = Vm::new(program);
    vm.start(entry, vec![]).map_err(|e| e.message)?;
    let object = finish(&mut vm)
        .and_then(|v| v.as_object())
        .map_err(|e| e.message)?;
    vm.start_member(object, member).map_err(|e| e.message)?;
    let value = finish(&mut vm).map_err(|e| e.message)?;
    if matches!(
        value,
        Value::Object(_) | Value::Function(_) | Value::Class(_) | Value::TypeAlias(_)
    ) {
        return Err("object results require evaluator-owned handles".into());
    }
    Ok(value)
}

fn finish(vm: &mut Vm) -> Result<Value, RuntimeError> {
    let start = vm.instructions_executed();
    loop {
        let remaining =
            1_000_000usize.saturating_sub((vm.instructions_executed() - start) as usize);
        if remaining == 0 {
            return Err(unsupported("probe instruction limit exceeded"));
        }
        match vm.poll(remaining) {
            VmPoll::Ready(value) => return value,
            VmPoll::Yielded => return Err(unsupported("probe instruction limit exceeded")),
            VmPoll::Module(request) => {
                let completion = crate::modules::default_completion(&request)
                    .ok_or_else(|| unsupported("use HostSession for asynchronous evaluation"))?;
                vm.complete_module(request.id, completion)?;
            }
            VmPoll::Glob(request) => vm.complete_glob(
                request.id,
                crate::glob::configured_or_unsupported_completion(&request, &[], &[]),
            )?,
            VmPoll::Pending(_) | VmPoll::Log(_) | VmPoll::TransformFrame(_) => {
                return Err(unsupported("use HostSession for asynchronous evaluation"));
            }
        }
    }
}
fn unsupported(message: &str) -> RuntimeError {
    RuntimeError::new(ErrorKind::Unsupported, message)
}

pub fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(text, "{byte:02x}").unwrap();
    }
    text
}
pub fn encode_value(value: Value) -> String {
    match value {
        Value::Null | Value::NullWithDefault(_) => "null\t".into(),
        Value::Bool(v) => format!("bool\t{v}"),
        Value::Int(v) => format!("int\t{v}"),
        Value::Float(v) => format!("float\t{:016x}", v.to_bits()),
        Value::DataSize(_) => "unsupported\tdata-size".into(),
        Value::Duration(_) => "unsupported\tduration".into(),
        Value::IntSeq(_) => "unsupported\tint-seq".into(),
        Value::Regex(_) => "unsupported\tregex".into(),
        Value::String(v) => format!("string\t{}", hex(v.to_java_utf8().as_bytes())),
        Value::Bytes(v) => format!("bytes\t{}", hex(&v)),
        Value::Object(_) => "unsupported\tobject".into(),
        Value::Function(_) => "unsupported\tfunction".into(),
        Value::Class(_) => "unsupported\tclass".into(),
        Value::TypeAlias(_) => "unsupported\ttype-alias".into(),
    }
}

pub struct HostProbe {
    session: HostSession,
}
impl HostProbe {
    pub fn new(mode: &str) -> Result<Self, RuntimeError> {
        let code = match mode {
            "read" => vec![
                Instruction::Input(0),
                Instruction::ReadResource,
                Instruction::Return,
            ],
            "add" => vec![
                Instruction::Constant(Value::Int(40.into())),
                Instruction::Input(0),
                Instruction::ReadResource,
                Instruction::Add,
                Instruction::Return,
            ],
            "loop" => vec![Instruction::Jump(0)],
            _ => return Err(unsupported("unknown Host probe mode")),
        };
        Ok(Self {
            session: HostSession::new(Arc::new(Program::new(vec![
                code,
                vec![
                    Instruction::Constant(Value::Int(100.into())),
                    Instruction::Input(0),
                    Instruction::ReadResource,
                    Instruction::Add,
                    Instruction::Return,
                ],
            ]))),
        })
    }
    pub fn start(&mut self, now: u32, timeout: u32) -> Result<(), RuntimeError> {
        self.session.start(
            0,
            vec![Value::string("test:/resource")],
            now.into(),
            Some(timeout.into()),
        )
    }
    pub fn poll(&mut self, now: u32, budget: u32) -> String {
        match self.session.poll(now.into(), budget as usize) {
            HostPoll::Yielded => "yield".into(),
            HostPoll::Read { token, uri } => format!(
                "read\t{token}\t{}",
                hex(uri
                    .as_str()
                    .expect("fixed-bytecode probe URI is scalar")
                    .as_bytes())
            ),
            HostPoll::Log { .. } | HostPoll::TransformFrame { .. } => {
                unreachable!("fixed-bytecode Host probe has no trace instructions")
            }
            HostPoll::Module { .. } | HostPoll::Glob { .. } => {
                unreachable!("fixed-bytecode Host probe has no module instructions")
            }
            HostPoll::Ready(Ok(v)) => format!("ready\t{}", encode_value(v)),
            HostPoll::Ready(Err(e)) => {
                format!(
                    "error\t{:?}\t{}",
                    e.kind,
                    hex(e.message.to_java_utf8().as_bytes())
                )
            }
        }
    }
    pub fn reenter(&mut self, token: &str, now: u32, timeout: u32) -> Result<(), RuntimeError> {
        self.session.reenter(
            token,
            1,
            vec![Value::string("test:/resource")],
            now.into(),
            Some(timeout.into()),
        )
    }
    /// Drain the shared Host's exactly-once abort signals for platform I/O adapters.
    pub fn drain_cancelled_reads(&mut self) -> Vec<String> {
        self.session.drain_cancelled_reads()
    }
    pub fn respond(&mut self, token: &str, value: Value, now: u32) -> Result<(), RuntimeError> {
        self.session.resume(token, Ok(value), now.into())
    }
    pub fn reject(&mut self, token: &str, message: &str, now: u32) -> Result<(), RuntimeError> {
        self.session.resume(
            token,
            Err(RuntimeError::new(ErrorKind::Host, message)),
            now.into(),
        )
    }
    pub fn cancel(&mut self) {
        self.session.cancel();
    }
    pub fn close(&mut self) {
        self.session.close();
    }
}

/// A lossless development transport record. Text is UTF-16 code units, not a
/// JavaScript UTF-8 conversion or a rendered imitation of a logger callback.
#[derive(Clone, Debug)]
pub struct SourceLog {
    pub level: &'static str,
    pub message: Vec<u16>,
    pub module_uri: Vec<u16>,
    pub member_name: Option<Vec<u16>>,
    pub source_lines: Vec<Vec<u16>>,
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
}

/// An owned full frame for the separate transformer callback and completion.
#[derive(Clone, Debug)]
pub struct SourceFrame {
    pub module_uri: Vec<u16>,
    pub member_name: Option<Vec<u16>>,
    pub source_lines: Vec<Vec<u16>>,
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
}
impl TryFrom<&crate::logging::LogFrame> for SourceFrame {
    type Error = RuntimeError;
    fn try_from(frame: &crate::logging::LogFrame) -> Result<Self, Self::Error> {
        let position = |value| {
            u32::try_from(value)
                .map_err(|_| unsupported("frame position exceeds the development transport range"))
        };
        Ok(Self {
            module_uri: frame.module_uri.encode_utf16().collect(),
            member_name: frame
                .member_name
                .as_ref()
                .map(|s| s.encode_utf16().collect()),
            source_lines: frame
                .source_lines
                .iter()
                .map(|s| s.encode_utf16().collect())
                .collect(),
            start_line: position(frame.start_line)?,
            start_column: position(frame.start_column)?,
            end_line: position(frame.end_line)?,
            end_column: position(frame.end_column)?,
        })
    }
}
impl From<SourceFrame> for crate::logging::LogFrame {
    fn from(frame: SourceFrame) -> Self {
        Self {
            module_uri: PklString::from_utf16(frame.module_uri),
            member_name: frame.member_name.map(PklString::from_utf16),
            source_lines: frame
                .source_lines
                .into_iter()
                .map(PklString::from_utf16)
                .collect(),
            start_line: frame.start_line as usize,
            start_column: frame.start_column as usize,
            end_line: frame.end_line as usize,
            end_column: frame.end_column as usize,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct SourceEvent {
    pub kind: &'static str,
    pub token: Option<String>,
    pub uri: Option<String>,
    pub value: Option<Vec<u16>>,
    pub bytes: Option<Vec<u8>>,
    pub error: Option<Vec<u16>>,
    pub error_bytes: Option<Vec<u8>>,
    pub log: Option<SourceLog>,
    pub frame: Option<SourceFrame>,
    pub module: Option<SourceModuleRequest>,
    pub exit_code: Option<i32>,
    pub phase: Option<&'static str>,
    pub disposition: Option<&'static str>,
    pub reason: Option<&'static str>,
    pub callback_origin: Option<&'static str>,
}

#[derive(Clone, Debug, Default)]
pub struct SourceModuleRequest {
    pub id: String,
    pub kind: &'static str,
    pub importer_original_uri: Option<String>,
    pub importer_resolved_uri: Option<String>,
    pub import_uri: Option<String>,
    pub original_uri: String,
    pub resolved_uri: Option<String>,
    pub source_uri: Option<String>,
    pub key_id: Option<String>,
    pub key_kind: Option<&'static str>,
    pub key_handle: Option<String>,
    pub resolved_key_id: Option<String>,
    pub resolved_key_handle: Option<String>,
    pub selection_phase: Option<&'static str>,
    pub resolution_phase: Option<&'static str>,
}

/// Scalar native conveniences retain their established field types. Lossless
/// callers and JavaScript adapters use SourceTextEvent / poll_text instead.
fn scalar_probe_text(value: PklString) -> Result<String, RuntimeError> {
    value.as_str().map(str::to_owned).ok_or_else(|| unsupported(
        "This scalar String API cannot represent unpaired UTF-16; use its lossless text counterpart.",
    ))
}
/// Codec-only observation through the production request conversion. This
/// does not validate URI acceptance or establish Java evaluator parity.
pub fn glob_transport_records(elements: Vec<crate::glob::PathElement>) -> Vec<SourceTextEvent> {
    use crate::glob::{GlobReader, GlobRequest, GlobRequestKind, ResourceReader, ResourceReaderId};
    elements
        .into_iter()
        .enumerate()
        .map(|(index, element)| {
            let request = GlobRequest {
                id: u64::MAX,
                kind: GlobRequestKind::ListElements {
                    reader: GlobReader::Resource(ResourceReader {
                        id: ResourceReaderId(9_007_199_254_740_993),
                        scheme: "codec".into(),
                        handle: "codec-reader".into(),
                    }),
                    uri: "codec:**".into(),
                },
            };
            SourceTextEvent {
                kind: "glob-request",
                token: Some(format!("codec:{index}")),
                glob: Some(request.into()),
                value: Some(element.name.encode_utf16().collect()),
                reason: Some(if element.is_directory {
                    "directory"
                } else {
                    "file"
                }),
                ..Default::default()
            }
        })
        .collect()
}

impl TryFrom<SourceTextModuleRequest> for SourceModuleRequest {
    type Error = RuntimeError;
    fn try_from(r: SourceTextModuleRequest) -> Result<Self, Self::Error> {
        Ok(Self {
            id: r.id,
            kind: r.kind,
            importer_original_uri: r.importer_original_uri.map(scalar_probe_text).transpose()?,
            importer_resolved_uri: r.importer_resolved_uri.map(scalar_probe_text).transpose()?,
            import_uri: r.import_uri.map(scalar_probe_text).transpose()?,
            original_uri: scalar_probe_text(r.original_uri)?,
            resolved_uri: r.resolved_uri.map(scalar_probe_text).transpose()?,
            source_uri: r.source_uri.map(scalar_probe_text).transpose()?,
            key_id: r.key_id,
            key_kind: r.key_kind,
            key_handle: r.key_handle.map(scalar_probe_text).transpose()?,
            resolved_key_id: r.resolved_key_id,
            resolved_key_handle: r.resolved_key_handle.map(scalar_probe_text).transpose()?,
            selection_phase: r.selection_phase,
            resolution_phase: r.resolution_phase,
        })
    }
}
impl TryFrom<SourceTextEvent> for SourceEvent {
    type Error = RuntimeError;
    fn try_from(e: SourceTextEvent) -> Result<Self, Self::Error> {
        if e.glob.is_some() {
            return Err(unsupported(
                "Typed glob events require the lossless poll_text API.",
            ));
        }
        Ok(Self {
            kind: e.kind,
            token: e.token,
            uri: e.uri.map(scalar_probe_text).transpose()?,
            value: e.value,
            bytes: e.bytes,
            error: e.error,
            error_bytes: e.error_bytes,
            log: e.log,
            frame: e.frame,
            module: e.module.map(SourceModuleRequest::try_from).transpose()?,
            exit_code: e.exit_code,
            phase: e.phase,
            disposition: e.disposition,
            reason: e.reason,
            callback_origin: e.callback_origin,
        })
    }
}

#[derive(Clone, Debug, Default)]
pub struct SourceTextEvent {
    pub kind: &'static str,
    pub token: Option<String>,
    pub uri: Option<PklString>,
    pub value: Option<Vec<u16>>,
    pub bytes: Option<Vec<u8>>,
    pub error: Option<Vec<u16>>,
    pub error_bytes: Option<Vec<u8>>,
    pub log: Option<SourceLog>,
    pub frame: Option<SourceFrame>,
    pub module: Option<SourceTextModuleRequest>,
    pub glob: Option<SourceGlobRequest>,
    pub exit_code: Option<i32>,
    pub phase: Option<&'static str>,
    pub disposition: Option<&'static str>,
    pub reason: Option<&'static str>,
    pub callback_origin: Option<&'static str>,
}

/// Lossless decimal identities for typed reader effects; path names arrive only
/// in completion payloads and retain their UTF-16 units.
#[derive(Clone, Debug, Default)]
pub struct SourceGlobRequest {
    pub id: String,
    pub kind: &'static str,
    pub uri: Option<PklString>,
    pub reader_kind: &'static str,
    pub reader_id: String,
    pub reader_uri: Option<PklString>,
    pub reader_scheme: Option<PklString>,
    pub reader_handle: Option<PklString>,
    pub key_kind: Option<&'static str>,
}
impl From<crate::glob::GlobRequest> for SourceGlobRequest {
    fn from(request: crate::glob::GlobRequest) -> Self {
        use crate::glob::{GlobReader, GlobRequestKind};
        let (kind, reader, uri) = match &request.kind {
            GlobRequestKind::SelectResource { id, uri } => {
                return Self {
                    id: request.id.to_string(),
                    kind: "select-resource",
                    uri: Some(uri.clone()),
                    reader_kind: "resource",
                    reader_id: id.0.to_string(),
                    ..Default::default()
                };
            }
            GlobRequestKind::IsGlobbable { reader } => ("is-globbable", reader.clone(), None),
            GlobRequestKind::IsHierarchical { reader } => ("is-hierarchical", reader.clone(), None),
            GlobRequestKind::FragmentPaths { reader } => {
                ("fragment-paths", GlobReader::Resource(reader.clone()), None)
            }
            GlobRequestKind::HasElement { reader, uri } => (
                "has-element",
                GlobReader::Resource(reader.clone()),
                Some(uri.clone()),
            ),
            GlobRequestKind::ListElements { reader, uri } => {
                ("list-elements", reader.clone(), Some(uri.clone()))
            }
        };
        let mut result = Self {
            id: request.id.to_string(),
            kind,
            uri,
            ..Default::default()
        };
        match reader {
            GlobReader::Module(key) => {
                result.reader_kind = "module";
                result.reader_id = key.id.0.to_string();
                result.reader_uri = Some(key.uri.clone());
                result.reader_handle = Some(key.handle.clone());
                result.key_kind = Some(key_kind_name(key.kind));
            }
            GlobReader::Resource(reader) => {
                result.reader_kind = "resource";
                result.reader_id = reader.id.0.to_string();
                result.reader_scheme = Some(reader.scheme.clone());
                result.reader_handle = Some(reader.handle.clone());
            }
        }
        result
    }
}

/// Module URI and opaque handle fields retain all UTF-16 units. Decimal IDs avoid
/// narrowing the shared u64 counter through a JavaScript Number.
#[derive(Clone, Debug, Default)]
pub struct SourceTextModuleRequest {
    pub id: String,
    pub kind: &'static str,
    pub importer_original_uri: Option<PklString>,
    pub importer_resolved_uri: Option<PklString>,
    pub import_uri: Option<PklString>,
    pub original_uri: PklString,
    pub resolved_uri: Option<PklString>,
    pub source_uri: Option<PklString>,
    pub key_id: Option<String>,
    pub key_kind: Option<&'static str>,
    pub key_handle: Option<PklString>,
    pub resolved_key_id: Option<String>,
    pub resolved_key_handle: Option<PklString>,
    pub selection_phase: Option<&'static str>,
    pub resolution_phase: Option<&'static str>,
}
impl From<crate::modules::ModuleRequest> for SourceTextModuleRequest {
    fn from(request: crate::modules::ModuleRequest) -> Self {
        use crate::modules::ModuleRequestKind;
        match request.kind {
            ModuleRequestKind::CheckImport {
                importer,
                import_uri,
            } => Self {
                id: request.id.to_string(),
                kind: "check-import",
                importer_original_uri: Some(importer.original_uri.clone()),
                importer_resolved_uri: Some(importer.resolved_uri.clone()),
                import_uri: Some(import_uri.clone()),
                // No ModuleKey has been selected yet. Keep the transport's
                // required target field equal to the prepared edge URI.
                original_uri: import_uri.clone(),
                resolved_uri: None,
                source_uri: None,
                ..Default::default()
            },
            ModuleRequestKind::SelectKey { id, uri, phase } => Self {
                id: request.id.to_string(),
                kind: "select-key",
                original_uri: uri.clone(),
                key_id: Some(id.0.to_string()),
                selection_phase: Some(match phase {
                    crate::modules::SelectionPhase::RuntimeImport => "runtime-import",
                    crate::modules::SelectionPhase::ClausePreparation => "clause-preparation",
                    crate::modules::SelectionPhase::Primary => "primary",
                }),
                ..Default::default()
            },
            ModuleRequestKind::ResolveImportUri { key, authored_uri } => Self {
                id: request.id.to_string(),
                kind: "resolve-import-uri",
                original_uri: key.uri.clone(),
                import_uri: Some(authored_uri.clone()),
                key_id: Some(key.id.0.to_string()),
                key_kind: Some(key_kind_name(key.kind)),
                key_handle: Some(key.handle.clone()),
                ..Default::default()
            },
            ModuleRequestKind::QueryFragmentPaths { key } => Self {
                id: request.id.to_string(),
                kind: "query-fragment-paths",
                original_uri: key.uri.clone(),
                key_id: Some(key.id.0.to_string()),
                key_kind: Some(key_kind_name(key.kind)),
                key_handle: Some(key.handle.clone()),
                ..Default::default()
            },
            ModuleRequestKind::QueryCachePolicy { key } => Self {
                id: request.id.to_string(),
                kind: "query-cache-policy",
                original_uri: key.uri.clone(),
                key_id: Some(key.id.0.to_string()),
                key_kind: Some(key_kind_name(key.kind)),
                key_handle: Some(key.handle.clone()),
                ..Default::default()
            },
            ModuleRequestKind::Capabilities { module, key } => Self {
                id: request.id.to_string(),
                kind: "capabilities",
                key_id: key.as_ref().map(|key| key.id.0.to_string()),
                key_kind: key.as_ref().map(|key| key_kind_name(key.kind)),
                key_handle: key.as_ref().map(|key| key.handle.clone()),
                importer_original_uri: Some(module.original_uri.clone()),
                importer_resolved_uri: Some(module.resolved_uri.clone()),
                import_uri: None,
                original_uri: module.original_uri.clone(),
                resolved_uri: None,
                source_uri: None,
                ..Default::default()
            },
            ModuleRequestKind::HasElement {
                module,
                key,
                element_uri,
            } => Self {
                id: request.id.to_string(),
                kind: "has-element",
                key_id: key.as_ref().map(|key| key.id.0.to_string()),
                key_kind: key.as_ref().map(|key| key_kind_name(key.kind)),
                key_handle: key.as_ref().map(|key| key.handle.clone()),
                importer_original_uri: Some(module.original_uri.clone()),
                importer_resolved_uri: Some(module.resolved_uri.clone()),
                import_uri: None,
                original_uri: element_uri.clone(),
                resolved_uri: None,
                source_uri: None,
                ..Default::default()
            },
            ModuleRequestKind::Resolve {
                importer,
                import_uri,
                original_uri,
                key,
                resolved_key_id,
            } => Self {
                id: request.id.to_string(),
                kind: "resolve",
                key_id: Some(key.id.0.to_string()),
                key_kind: Some(key_kind_name(key.kind)),
                key_handle: Some(key.handle.clone()),
                resolved_key_id: Some(resolved_key_id.0.to_string()),
                importer_original_uri: Some(importer.original_uri.clone()),
                importer_resolved_uri: Some(importer.resolved_uri.clone()),
                import_uri: Some(import_uri.clone()),
                original_uri: original_uri.clone(),
                resolved_uri: None,
                source_uri: None,
                ..Default::default()
            },
            ModuleRequestKind::ResolveKey {
                key,
                resolved_key_id,
                phase,
            } => Self {
                id: request.id.to_string(),
                kind: "resolve-key",
                original_uri: key.uri.clone(),
                key_id: Some(key.id.0.to_string()),
                key_kind: Some(key_kind_name(key.kind)),
                key_handle: Some(key.handle.clone()),
                resolved_key_id: Some(resolved_key_id.0.to_string()),
                resolution_phase: Some(match phase {
                    crate::modules::ResolutionPhase::Primary => "primary",
                    crate::modules::ResolutionPhase::Query => "query",
                }),
                ..Default::default()
            },
            ModuleRequestKind::LoadSource { resolved, key } => Self {
                id: request.id.to_string(),
                kind: "load-source",
                importer_original_uri: None,
                importer_resolved_uri: None,
                import_uri: None,
                original_uri: resolved.original_uri.clone(),
                resolved_uri: Some(resolved.resolved_uri.clone()),
                source_uri: Some(resolved.source_uri.clone()),
                key_id: Some(key.key.id.0.to_string()),
                key_kind: Some(key_kind_name(key.key.kind)),
                key_handle: Some(key.key.handle.clone()),
                resolved_key_id: Some(key.id.0.to_string()),
                resolved_key_handle: Some(key.handle.clone()),
                ..Default::default()
            },
        }
    }
}
fn key_kind_name(kind: crate::modules::ModuleKeyKind) -> &'static str {
    use crate::modules::ModuleKeyKind;
    match kind {
        ModuleKeyKind::StandardLibrary => "standard-library",
        ModuleKeyKind::Host => "host",
        ModuleKeyKind::StandardFile => "standard-file",
        ModuleKeyKind::StandardHttp => "standard-http",
        ModuleKeyKind::StandardGenericUrl => "standard-generic-url",
        ModuleKeyKind::SuppliedText => "supplied-text",
        ModuleKeyKind::Synthetic => "synthetic",
    }
}

impl SourceTextEvent {
    fn new(kind: &'static str) -> Self {
        Self {
            kind,
            token: None,
            uri: None,
            value: None,
            bytes: None,
            error: None,
            error_bytes: None,
            log: None,
            frame: None,
            module: None,
            glob: None,
            exit_code: None,
            phase: None,
            disposition: None,
            reason: None,
            callback_origin: None,
        }
    }
}

/// Shared actual-source session used by native development tools and JS bindings.
/// Poll returns before the caller executes a logger/transformer callback, so the
/// same evaluator can be reentered without holding a Rust mutable borrow.
pub struct SourceProbe {
    session: crate::evaluation::session::SourceSession,
    output: bool,
    properties: Vec<(String, String)>,
    environment: Vec<(String, String)>,
}
impl SourceProbe {
    /// Development custom-Host helper. URI semantics are exactly the same
    /// production operations used by glob traversal; this performs no I/O and
    /// does not manufacture Java SPI callback observations.
    pub fn resolve_glob_host_uri(
        base: &PklString,
        relative: &PklString,
        fragment_paths: bool,
    ) -> Result<PklString, RuntimeError> {
        crate::host::resources::glob::resolve(base, relative, fragment_paths)
    }
    /// A Host's resolved key may normalize its own URI while retaining the
    /// original key identity. Raw mapping keys remain evaluator-owned.
    pub fn normalize_glob_host_uri(uri: &PklString) -> Result<PklString, RuntimeError> {
        crate::host::resources::glob::normalize(uri)
    }
    pub fn new_module_evaluator(
        trace_mode: &str,
        host_logger: bool,
        host_frame_transformer: bool,
        power_assertions: bool,
    ) -> Result<Self, PklString> {
        Ok(Self {
            session: crate::evaluation::session::SourceSession::module_evaluator(
                Self::options(
                    trace_mode,
                    host_logger,
                    host_frame_transformer,
                    power_assertions,
                )
                .map_err(PklString::from)?,
            ),
            output: false,
            properties: Vec::new(),
            environment: Vec::new(),
        })
    }
    /// Existing scalar source convenience; no UTF-16 conversion is performed.
    pub fn evaluate_module_source(
        &mut self,
        uri: impl Into<PklString>,
        contents: Option<&str>,
        expression: impl Into<SourceText>,
        now: u32,
        timeout: Option<u32>,
    ) -> Result<(), RuntimeError> {
        self.evaluate_module_source_text(
            uri,
            contents.map(SourceText::from),
            expression,
            now,
            timeout,
        )
    }
    pub fn evaluate_module_source_text(
        &mut self,
        uri: impl Into<PklString>,
        contents: Option<SourceText>,
        expression: impl Into<SourceText>,
        now: u32,
        timeout: Option<u32>,
    ) -> Result<(), RuntimeError> {
        self.session.start_evaluation(
            crate::modules::ModuleSourceSpec {
                uri: uri.into(),
                contents,
            },
            expression,
            now.into(),
            timeout.map(Into::into),
        )
    }
    #[allow(clippy::too_many_arguments)]
    /// Existing scalar reentry convenience; use _text for retained code units.
    pub fn reenter_module_source(
        &mut self,
        token: &str,
        uri: impl Into<PklString>,
        contents: Option<&str>,
        expression: impl Into<SourceText>,
        now: u32,
        timeout: Option<u32>,
    ) -> Result<(), RuntimeError> {
        self.reenter_module_source_text(
            token,
            uri,
            contents.map(SourceText::from),
            expression,
            now,
            timeout,
        )
    }
    #[allow(clippy::too_many_arguments)]
    pub fn reenter_module_source_text(
        &mut self,
        token: &str,
        uri: impl Into<PklString>,
        contents: Option<SourceText>,
        expression: impl Into<SourceText>,
        now: u32,
        timeout: Option<u32>,
    ) -> Result<(), RuntimeError> {
        self.session.reenter_evaluation(
            token,
            crate::modules::ModuleSourceSpec {
                uri: uri.into(),
                contents,
            },
            expression,
            now.into(),
            timeout.map(Into::into),
        )
    }
    /// Legacy constructor; Java UTF-8 conversion is explicit at this boundary.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        source: &str,
        uri: &str,
        module_name: &str,
        paths: &[String],
        trace_mode: &str,
        host_logger: bool,
        host_frame_transformer: bool,
        power_assertions: bool,
    ) -> Result<Self, String> {
        Self::new_text(
            source,
            uri,
            module_name,
            paths,
            trace_mode,
            host_logger,
            host_frame_transformer,
            power_assertions,
        )
        .map_err(|text| text.to_java_utf8())
    }
    #[allow(clippy::too_many_arguments)]
    pub fn new_text(
        source: impl Into<SourceText>,
        uri: impl Into<PklString>,
        module_name: impl Into<PklString>,
        paths: &[String],
        trace_mode: &str,
        host_logger: bool,
        host_frame_transformer: bool,
        power_assertions: bool,
    ) -> Result<Self, PklString> {
        Ok(Self {
            session: crate::evaluation::session::SourceSession::compile_text(
                source,
                uri,
                module_name,
                paths,
                Self::options(
                    trace_mode,
                    host_logger,
                    host_frame_transformer,
                    power_assertions,
                )
                .map_err(PklString::from)?,
            )?,
            output: false,
            properties: Vec::new(),
            environment: Vec::new(),
        })
    }
    #[allow(clippy::too_many_arguments)]
    pub fn new_expressions(
        source: &str,
        uri: &str,
        module_name: &str,
        expressions: &[String],
        trace_mode: &str,
        host_logger: bool,
        host_frame_transformer: bool,
        power_assertions: bool,
    ) -> Result<Self, String> {
        Self::new_expressions_text(
            source,
            uri,
            module_name,
            expressions,
            trace_mode,
            host_logger,
            host_frame_transformer,
            power_assertions,
        )
        .map_err(|text| text.to_java_utf8())
    }
    #[allow(clippy::too_many_arguments)]
    pub fn new_expressions_text(
        source: impl Into<SourceText>,
        uri: impl Into<PklString>,
        module_name: impl Into<PklString>,
        expressions: &[String],
        trace_mode: &str,
        host_logger: bool,
        host_frame_transformer: bool,
        power_assertions: bool,
    ) -> Result<Self, PklString> {
        Ok(Self {
            session: crate::evaluation::session::SourceSession::compile_expressions_text(
                source,
                uri,
                module_name,
                expressions,
                Self::options(
                    trace_mode,
                    host_logger,
                    host_frame_transformer,
                    power_assertions,
                )
                .map_err(PklString::from)?,
            )?,
            output: false,
            properties: Vec::new(),
            environment: Vec::new(),
        })
    }
    #[allow(clippy::too_many_arguments)]
    pub fn new_expressions_source(
        source: impl Into<SourceText>,
        uri: impl Into<PklString>,
        module_name: impl Into<PklString>,
        expressions: &[SourceText],
        trace_mode: &str,
        host_logger: bool,
        host_frame_transformer: bool,
        power_assertions: bool,
    ) -> Result<Self, PklString> {
        Ok(Self {
            session: crate::evaluation::session::SourceSession::compile_expressions_source(
                source,
                uri,
                module_name,
                expressions,
                Self::options(
                    trace_mode,
                    host_logger,
                    host_frame_transformer,
                    power_assertions,
                )
                .map_err(PklString::from)?,
            )?,
            output: false,
            properties: Vec::new(),
            environment: Vec::new(),
        })
    }
    pub fn options(
        trace_mode: &str,
        host_logger: bool,
        host_frame_transformer: bool,
        power_assertions: bool,
    ) -> Result<crate::evaluation::session::SourceSessionOptions, String> {
        use crate::logging::{FrameTransformer, LogDelivery, TraceMode};
        Ok(crate::evaluation::session::SourceSessionOptions {
            evaluation: crate::evaluation::EvaluationOptions { power_assertions },
            trace_mode: match trace_mode {
                "compact" | "COMPACT" => TraceMode::Compact,
                "pretty" | "PRETTY" => TraceMode::Pretty,
                _ => return Err("expected trace mode compact or pretty".into()),
            },
            log_delivery: if host_logger {
                LogDelivery::Host
            } else {
                LogDelivery::Noop
            },
            frame_transformer: if host_frame_transformer {
                FrameTransformer::Host
            } else {
                FrameTransformer::Default
            },
            platform_data: false,
        })
    }
    pub fn new_output(
        input: impl Into<crate::evaluation::frontend::LosslessModuleSourceInput>,
        paths: &[String],
        module_options: &crate::evaluation::frontend::ModuleEvalOptions,
        session_options: crate::evaluation::session::SourceSessionOptions,
    ) -> Result<Self, crate::evaluation::frontend::ModuleFrontendResult> {
        Self::new_output_text(input, paths, module_options, session_options).map_err(Into::into)
    }
    pub fn new_output_text(
        input: impl Into<crate::evaluation::frontend::LosslessModuleSourceInput>,
        paths: &[String],
        module_options: &crate::evaluation::frontend::ModuleEvalOptions,
        session_options: crate::evaluation::session::SourceSessionOptions,
    ) -> Result<Self, crate::evaluation::frontend::ModuleFrontendFailure> {
        Ok(Self {
            session: crate::evaluation::session::SourceSession::compile_output_text(
                input,
                paths,
                module_options,
                session_options,
            )?,
            output: true,
            properties: module_options.properties.clone(),
            environment: module_options.environment.clone(),
        })
    }
    pub fn start_output_bytes(
        &mut self,
        now: u32,
        timeout: Option<u32>,
    ) -> Result<(), RuntimeError> {
        self.session
            .start_output_bytes(now.into(), timeout.map(Into::into))
    }
    pub fn reenter_output_bytes(
        &mut self,
        token: &str,
        now: u32,
        timeout: Option<u32>,
    ) -> Result<(), RuntimeError> {
        self.session
            .reenter_output_bytes(token, now.into(), timeout.map(Into::into))
    }
    pub fn start_module(&mut self, now: u32, timeout: Option<u32>) -> Result<(), RuntimeError> {
        self.session
            .start_module(now.into(), timeout.map(Into::into))
    }
    pub fn start_path(
        &mut self,
        index: u32,
        now: u32,
        timeout: Option<u32>,
    ) -> Result<(), RuntimeError> {
        self.session
            .start_path(index as usize, now.into(), timeout.map(Into::into))
    }
    pub fn setup_failed(&self) -> bool {
        self.session.setup_failed()
    }
    pub fn reenter_path(
        &mut self,
        token: &str,
        index: u32,
        now: u32,
        timeout: Option<u32>,
    ) -> Result<(), RuntimeError> {
        self.session
            .reenter_path(token, index as usize, now.into(), timeout.map(Into::into))
    }
    /// Scalar native transport retained for existing callers.
    pub fn poll(&mut self, now: u32, budget: u32) -> Result<SourceEvent, RuntimeError> {
        SourceEvent::try_from(self.poll_text(now, budget)?)
    }
    /// Complete UTF-16 identities for native and JavaScript Host callbacks.
    pub fn poll_text(&mut self, now: u32, budget: u32) -> Result<SourceTextEvent, RuntimeError> {
        use crate::logging::LogLevel;
        let previous_depth = self.session.context_depth();
        let event = match self.session.poll(now.into(), budget as usize) {
            HostPoll::Yielded => SourceTextEvent::new("yield"),
            HostPoll::Read { token, uri } => {
                let mut event = SourceTextEvent::new("read");
                event.token = Some(token);
                event.uri = Some(uri.clone());
                event
            }
            HostPoll::Glob { token, request } => {
                let mut event = SourceTextEvent::new("glob-request");
                event.token = Some(token);
                event.glob = Some(request.into());
                event
            }
            HostPoll::Module { token, request } => {
                let mut event = SourceTextEvent::new("module-request");
                event.token = Some(token);
                event.module = Some(request.into());
                event
            }
            HostPoll::TransformFrame { token, frame } => {
                let mut event = SourceTextEvent::new("transform-frame");
                event.token = Some(token);
                event.frame = Some(SourceFrame::try_from(frame.as_ref())?);
                event
            }
            HostPoll::Log { token, event: log } => {
                let mut event = SourceTextEvent::new("log");
                event.token = Some(token);
                let position = |n| {
                    u32::try_from(n).map_err(|_| {
                        unsupported("logger frame position exceeds the development transport range")
                    })
                };
                event.log = Some(SourceLog {
                    level: match log.level {
                        LogLevel::Trace => "trace",
                        LogLevel::Warn => "warn",
                    },
                    message: log.message.encode_utf16().collect(),
                    module_uri: log.frame.module_uri.encode_utf16().collect(),
                    member_name: log
                        .frame
                        .member_name
                        .as_ref()
                        .map(|s| s.encode_utf16().collect()),
                    source_lines: log
                        .frame
                        .source_lines
                        .iter()
                        .map(|s| s.encode_utf16().collect())
                        .collect(),
                    start_line: position(log.frame.start_line)?,
                    start_column: position(log.frame.start_column)?,
                    end_line: position(log.frame.end_line)?,
                    end_column: position(log.frame.end_column)?,
                });
                event
            }
            HostPoll::Ready(Ok(Value::Object(_))) => SourceTextEvent::new("module"),
            HostPoll::Ready(Ok(Value::Bytes(bytes))) => {
                let mut event = SourceTextEvent::new("bytes");
                event.bytes = Some(bytes.to_vec());
                event.exit_code = Some(0);
                event.phase =
                    Some(crate::evaluation::frontend::FrontendPhase::ModuleOutput.as_str());
                event
            }
            HostPoll::Ready(Ok(value)) => {
                let value = crate::evaluation::session::SourceSession::result_string(value)?;
                let mut event = SourceTextEvent::new("value");
                event.value = Some(value.encode_utf16().collect());
                event
            }
            HostPoll::Ready(Err(error)) => {
                if previous_depth > 0 && self.session.context_depth() == previous_depth {
                    return Err(error);
                }
                let mut event = SourceTextEvent::new("error");
                event.callback_origin = error
                    .callback_origin()
                    .map(crate::logging::CallbackOrigin::as_str);
                // This is shared implementation classification, not a Java API
                // process exit. Expression diagnostics retain their raw units.
                let failure = self.session.output_failure_text(error.clone());
                event.phase = Some(failure.phase.as_str());
                event.disposition = Some(failure.disposition.as_str());
                event.reason = failure.reason;
                if self.output {
                    let failure = self.session.output_session_failure_text(error);
                    event.error = Some(failure.text.encode_utf16().collect());
                    let failure: crate::evaluation::frontend::ModuleFrontendResult = failure.into();
                    event.error_bytes = Some(failure.stderr);
                    event.exit_code = Some(failure.exit_code);
                } else {
                    event.error = Some(
                        self.session
                            .format_error_text(error)
                            .encode_utf16()
                            .collect(),
                    );
                }
                event
            }
        };
        Ok(event)
    }
    pub fn respond(&mut self, token: &str, value: Value, now: u32) -> Result<(), RuntimeError> {
        self.session.resume(token, Ok(value), now.into())
    }
    pub fn respond_configured(&mut self, token: &str, now: u32) -> Result<(), RuntimeError> {
        self.session
            .resume_default_read(token, &self.properties, &self.environment, now.into())
    }
    pub fn complete_read(
        &mut self,
        token: &str,
        result: Result<crate::resources::ReadResult, crate::host::ReadFailure>,
        now: u32,
    ) -> Result<(), RuntimeError> {
        self.session.complete_read(token, result, now.into())
    }
    pub fn complete_read_missing(&mut self, token: &str, now: u32) -> Result<(), RuntimeError> {
        self.complete_read(token, Ok(crate::resources::ReadResult::Missing), now)
    }
    pub fn complete_read_text(
        &mut self,
        token: &str,
        value: crate::value::PklString,
        now: u32,
    ) -> Result<(), RuntimeError> {
        self.complete_read(token, Ok(crate::resources::ReadResult::Text(value)), now)
    }
    pub fn complete_read_resource(
        &mut self,
        token: &str,
        uri: crate::value::PklString,
        bytes: Vec<u8>,
        now: u32,
    ) -> Result<(), RuntimeError> {
        self.complete_read(
            token,
            Ok(crate::resources::ReadResult::Resource(
                crate::resources::Resource {
                    uri,
                    bytes: bytes.into(),
                },
            )),
            now,
        )
    }
    pub fn reject_read_security_manager(
        &mut self,
        token: &str,
        message: Option<crate::value::PklString>,
        now: u32,
    ) -> Result<(), RuntimeError> {
        self.complete_read(
            token,
            Err(crate::host::ReadFailure::SecurityManagerException { message }),
            now,
        )
    }
    /// URI delivered to a typed reader, distinct from the legacy event's
    /// authored operand. Parsing and resolution stay in the shared runtime.
    pub fn resolved_read_uri(&self, token: &str) -> Result<String, RuntimeError> {
        scalar_probe_text(self.resolved_read_uri_text(token)?)
    }
    pub fn resolved_read_uri_text(&self, token: &str) -> Result<PklString, RuntimeError> {
        crate::host::resources::resolved_uri(&self.session.read_request(token)?)
    }
    pub fn set_glob_test_mode(&mut self, enabled: bool) -> Result<(), RuntimeError> {
        self.session.set_glob_test_mode(enabled)
    }
    pub fn set_platform_metadata(&mut self, metadata: Option<crate::logging::PlatformMetadata>) {
        self.session.set_platform_metadata(metadata);
    }
    pub fn glob_request(&self, token: &str) -> Result<crate::glob::GlobRequest, RuntimeError> {
        self.session.glob_request(token)
    }
    pub fn complete_glob(
        &mut self,
        token: &str,
        completion: crate::glob::GlobCompletion,
        now: u32,
    ) -> Result<(), RuntimeError> {
        self.session.complete_glob(token, completion, now.into())
    }
    /// Decimal IDs must echo the VM-issued reader identity, never a JS Number.
    pub fn complete_glob_reader(
        &mut self,
        token: &str,
        reader_id: &str,
        scheme: impl Into<PklString>,
        handle: impl Into<PklString>,
        now: u32,
    ) -> Result<(), RuntimeError> {
        let id = reader_id
            .parse::<u64>()
            .map_err(|_| RuntimeError::new(ErrorKind::Host, "invalid resource reader ID"))?;
        self.complete_glob(
            token,
            crate::glob::GlobCompletion::Resource(Ok(crate::glob::ResourceReader {
                id: crate::glob::ResourceReaderId(id),
                scheme: scheme.into(),
                handle: handle.into(),
            })),
            now,
        )
    }
    pub fn complete_glob_boolean(
        &mut self,
        token: &str,
        kind: &str,
        value: bool,
        now: u32,
    ) -> Result<(), RuntimeError> {
        use crate::glob::GlobCompletion;
        let completion = match kind {
            "is-globbable" => GlobCompletion::Globbable(Ok(value)),
            "is-hierarchical" => GlobCompletion::Hierarchical(Ok(value)),
            "fragment-paths" => GlobCompletion::FragmentPaths(Ok(value)),
            "has-element" => GlobCompletion::HasElement(Ok(value)),
            _ => {
                return Err(RuntimeError::new(
                    ErrorKind::Host,
                    "unknown glob boolean completion kind",
                ));
            }
        };
        self.complete_glob(token, completion, now)
    }
    pub fn complete_glob_elements(
        &mut self,
        token: &str,
        elements: Vec<crate::glob::PathElement>,
        now: u32,
    ) -> Result<(), RuntimeError> {
        self.complete_glob(
            token,
            crate::glob::GlobCompletion::Elements(Ok(elements)),
            now,
        )
    }
    pub fn reject_glob(
        &mut self,
        token: &str,
        kind: &str,
        message: Option<PklString>,
        now: u32,
    ) -> Result<(), RuntimeError> {
        let request = self.glob_request(token)?;
        let failure = if kind == "reader-not-found" {
            if message.is_some() {
                return Err(RuntimeError::new(
                    ErrorKind::Host,
                    "reader-not-found has no message",
                ));
            }
            crate::glob::GlobHostError::ReaderNotFound
        } else {
            crate::glob::GlobHostError::Failure(Self::module_failure(kind, message)?)
        };
        self.complete_glob(
            token,
            crate::glob::failure_completion(&request, failure),
            now,
        )
    }
    /// Complete only effects supported by the explicit configured profile.
    /// Unavailable I/O returns false with the request intact.
    pub fn complete_default_glob(&mut self, token: &str, now: u32) -> Result<bool, RuntimeError> {
        self.session
            .complete_default_glob(token, &self.properties, &self.environment, now.into())
    }
    pub fn drain_cancelled_globs(&mut self) -> Vec<String> {
        self.session.drain_cancelled_globs()
    }
    pub fn module_request(
        &self,
        token: &str,
    ) -> Result<crate::modules::ModuleRequest, RuntimeError> {
        self.session.module_request(token)
    }
    pub fn read_request(&self, token: &str) -> Result<crate::vm::ReadRequest, RuntimeError> {
        self.session.read_request(token)
    }
    pub fn complete_module(
        &mut self,
        token: &str,
        completion: crate::modules::ModuleCompletion,
        now: u32,
    ) -> Result<(), RuntimeError> {
        self.session.complete_module(token, completion, now.into())
    }
    #[allow(clippy::too_many_arguments)]
    pub fn complete_module_key(
        &mut self,
        token: &str,
        key_id: &str,
        uri: impl Into<PklString>,
        kind: &str,
        handle: impl Into<PklString>,
        now: u32,
    ) -> Result<(), RuntimeError> {
        use crate::modules::{ModuleCompletion, ModuleKey, ModuleKeyId, ModuleKeyKind};
        let kind = match kind {
            "host" => ModuleKeyKind::Host,
            "standard-file" => ModuleKeyKind::StandardFile,
            "standard-http" => ModuleKeyKind::StandardHttp,
            "standard-generic-url" => ModuleKeyKind::StandardGenericUrl,
            "standard-library" => ModuleKeyKind::StandardLibrary,
            "supplied-text" => ModuleKeyKind::SuppliedText,
            "synthetic" => ModuleKeyKind::Synthetic,
            _ => return Err(RuntimeError::new(ErrorKind::Host, "unknown ModuleKey kind")),
        };
        let id = key_id
            .parse()
            .map_err(|_| RuntimeError::new(ErrorKind::Host, "invalid ModuleKey ID"))?;
        self.complete_module(
            token,
            ModuleCompletion::KeySelected(Ok(ModuleKey {
                id: ModuleKeyId(id),
                uri: uri.into(),
                kind,
                handle: handle.into(),
            })),
            now,
        )
    }
    pub fn complete_module_cache_policy(
        &mut self,
        token: &str,
        key_id: &str,
        cached: bool,
        now: u32,
    ) -> Result<(), RuntimeError> {
        let request = self.module_request(token)?;
        if !matches!(&request.kind, crate::modules::ModuleRequestKind::QueryCachePolicy { key } if key.id.0.to_string() == key_id)
        {
            return Err(RuntimeError::new(
                ErrorKind::Host,
                "stale or unexpected ModuleKey completion",
            ));
        }
        self.complete_module(
            token,
            crate::modules::ModuleCompletion::CachePolicy(Ok(cached)),
            now,
        )
    }
    pub fn complete_import_uri(
        &mut self,
        token: &str,
        key_id: &str,
        uri: impl Into<PklString>,
        now: u32,
    ) -> Result<(), RuntimeError> {
        let request = self.module_request(token)?;
        if !matches!(&request.kind, crate::modules::ModuleRequestKind::ResolveImportUri { key, .. } if key.id.0.to_string() == key_id)
        {
            return Err(RuntimeError::new(
                ErrorKind::Host,
                "stale or unexpected ModuleKey completion",
            ));
        }
        self.complete_module(
            token,
            crate::modules::ModuleCompletion::ImportUri(Ok(uri.into())),
            now,
        )
    }
    /// ModuleKey.resolveUri failure, with the exact pending key identity.
    /// The VM retains phase-specific failure admission before consuming state.
    pub fn reject_import_uri(
        &mut self,
        token: &str,
        key_id: &str,
        kind: &str,
        message: Option<PklString>,
        now: u32,
    ) -> Result<(), RuntimeError> {
        let request = self.module_request(token)?;
        if !matches!(&request.kind, crate::modules::ModuleRequestKind::ResolveImportUri { key, .. }
            if key.id.0.to_string() == key_id)
        {
            return Err(RuntimeError::new(
                ErrorKind::Host,
                "stale or unexpected ModuleKey completion",
            ));
        }
        let failure = Self::module_failure(kind, message)?;
        self.complete_module(
            token,
            crate::modules::ModuleCompletion::ImportUri(Err(failure)),
            now,
        )
    }
    pub fn complete_module_fragment_paths(
        &mut self,
        token: &str,
        key_id: &str,
        fragment_paths: bool,
        now: u32,
    ) -> Result<(), RuntimeError> {
        let request = self.module_request(token)?;
        if !matches!(&request.kind, crate::modules::ModuleRequestKind::QueryFragmentPaths { key } if key.id.0.to_string() == key_id)
        {
            return Err(RuntimeError::new(
                ErrorKind::Host,
                "stale or unexpected ModuleKey completion",
            ));
        }
        self.complete_module(
            token,
            crate::modules::ModuleCompletion::FragmentPaths(Ok(fragment_paths)),
            now,
        )
    }
    #[allow(clippy::too_many_arguments)]
    pub fn complete_module_resolved_key(
        &mut self,
        token: &str,
        key_id: &str,
        resolved_key_id: &str,
        handle: impl Into<PklString>,
        original_uri: impl Into<PklString>,
        resolved_uri: impl Into<PklString>,
        source_uri: impl Into<PklString>,
        now: u32,
    ) -> Result<(), RuntimeError> {
        use crate::modules::{
            ModuleCompletion, ModuleRequestKind, ResolvedModule, ResolvedModuleKey,
        };
        let request = self.module_request(token)?;
        let (ModuleRequestKind::Resolve {
            key,
            resolved_key_id: id,
            ..
        }
        | ModuleRequestKind::ResolveKey {
            key,
            resolved_key_id: id,
            ..
        }) = request.kind
        else {
            return Err(RuntimeError::new(
                ErrorKind::Host,
                "stale or unexpected resolved ModuleKey completion",
            ));
        };
        if key.id.0.to_string() != key_id || id.0.to_string() != resolved_key_id {
            return Err(RuntimeError::new(
                ErrorKind::Host,
                "stale or unexpected resolved ModuleKey identity",
            ));
        }
        self.complete_module(
            token,
            ModuleCompletion::KeyResolved(Ok(ResolvedModuleKey {
                id,
                key,
                handle: handle.into(),
                module: ResolvedModule {
                    original_uri: original_uri.into(),
                    resolved_uri: resolved_uri.into(),
                    source_uri: source_uri.into(),
                },
            })),
            now,
        )
    }
    pub fn reject_module_key(
        &mut self,
        token: &str,
        cache_policy: bool,
        kind: &str,
        message: Option<PklString>,
        now: u32,
    ) -> Result<(), RuntimeError> {
        let error = Self::module_failure(kind, message)?;
        self.complete_module(
            token,
            if cache_policy {
                crate::modules::ModuleCompletion::CachePolicy(Err(error))
            } else {
                crate::modules::ModuleCompletion::KeySelected(Err(error))
            },
            now,
        )
    }
    pub fn complete_module_capabilities(
        &mut self,
        token: &str,
        local: bool,
        hierarchical: bool,
        fragment_paths: bool,
        now: u32,
    ) -> Result<(), RuntimeError> {
        self.complete_module(
            token,
            crate::modules::ModuleCompletion::Capabilities(Ok(
                crate::modules::ModuleCapabilities {
                    local,
                    hierarchical,
                    fragment_paths,
                },
            )),
            now,
        )
    }

    pub fn complete_module_import_check(
        &mut self,
        token: &str,
        now: u32,
    ) -> Result<(), RuntimeError> {
        self.complete_module(
            token,
            crate::modules::ModuleCompletion::ImportChecked(Ok(())),
            now,
        )
    }

    pub fn reject_module_import_check(
        &mut self,
        token: &str,
        kind: &str,
        message: Option<crate::value::PklString>,
        now: u32,
    ) -> Result<(), RuntimeError> {
        let failure = Self::module_failure(kind, message)?;
        self.complete_module(
            token,
            crate::modules::ModuleCompletion::ImportChecked(Err(failure)),
            now,
        )
    }

    pub fn complete_default_module(&mut self, token: &str, now: u32) -> Result<bool, RuntimeError> {
        self.session.complete_default_module(token, now.into())
    }
    pub fn complete_module_has_element(
        &mut self,
        token: &str,
        exists: bool,
        now: u32,
    ) -> Result<(), RuntimeError> {
        self.complete_module(
            token,
            crate::modules::ModuleCompletion::HasElement(Ok(exists)),
            now,
        )
    }
    pub fn complete_module_resolution(
        &mut self,
        token: &str,
        original_uri: impl Into<PklString>,
        resolved_uri: impl Into<PklString>,
        source_uri: impl Into<PklString>,
        now: u32,
    ) -> Result<(), RuntimeError> {
        self.complete_module(
            token,
            crate::modules::ModuleCompletion::Resolved(Ok(crate::modules::ResolvedModule {
                original_uri: original_uri.into(),
                resolved_uri: resolved_uri.into(),
                source_uri: source_uri.into(),
            })),
            now,
        )
    }
    pub fn complete_module_source(
        &mut self,
        token: &str,
        text: impl Into<SourceText>,
        inferred_name: impl Into<PklString>,
        now: u32,
    ) -> Result<(), RuntimeError> {
        self.complete_module(
            token,
            crate::modules::ModuleCompletion::Source(Ok(crate::modules::ModuleSource {
                text: text.into(),
                inferred_name: inferred_name.into(),
            })),
            now,
        )
    }
    /// Legacy text convenience wrapper; the typed path below accepts all UTF-16.
    pub fn reject_module(
        &mut self,
        token: &str,
        loading_source: bool,
        kind: &str,
        message: &str,
        now: u32,
    ) -> Result<(), RuntimeError> {
        self.reject_module_text(token, loading_source, kind, Some(message.into()), now)
    }
    pub fn reject_module_text(
        &mut self,
        token: &str,
        loading_source: bool,
        kind: &str,
        message: Option<crate::value::PklString>,
        now: u32,
    ) -> Result<(), RuntimeError> {
        use crate::modules::ModuleCompletion;
        let failure = Self::module_failure(kind, message)?;
        self.complete_module(
            token,
            if loading_source {
                ModuleCompletion::Source(Err(failure))
            } else {
                ModuleCompletion::Resolved(Err(failure))
            },
            now,
        )
    }
    fn module_failure(
        kind: &str,
        message: Option<crate::value::PklString>,
    ) -> Result<crate::modules::ModuleHostError, RuntimeError> {
        use crate::modules::ModuleHostError;
        Ok(match kind {
            "not-found" => ModuleHostError::NotFound,
            "no-loader" => ModuleHostError::NoLoader,
            "uri-syntax" => ModuleHostError::UriSyntax {
                reason: message.ok_or_else(|| {
                    RuntimeError::new(ErrorKind::Host, "URI syntax reason is required")
                })?,
            },
            "external-reader-process" => ModuleHostError::ExternalReaderProcess { message },
            "io" => ModuleHostError::Io(crate::host::IoFailure::new(message)),
            "security-manager-exception" => ModuleHostError::SecurityManagerException { message },
            "denied" | "unsupported" => {
                let message = message.ok_or_else(|| {
                    RuntimeError::new(ErrorKind::Host, "this module Host error requires a message")
                })?;
                if kind == "denied" {
                    ModuleHostError::Denied(message)
                } else {
                    ModuleHostError::Unsupported(message)
                }
            }
            _ => {
                return Err(RuntimeError::new(
                    ErrorKind::Host,
                    "unknown module Host error kind",
                ));
            }
        })
    }
    pub fn reject_module_preparation_text(
        &mut self,
        token: &str,
        has_element: bool,
        kind: &str,
        message: Option<crate::value::PklString>,
        now: u32,
    ) -> Result<(), RuntimeError> {
        let failure = Self::module_failure(kind, message)?;
        self.complete_module(
            token,
            if has_element {
                crate::modules::ModuleCompletion::HasElement(Err(failure))
            } else {
                crate::modules::ModuleCompletion::Capabilities(Err(failure))
            },
            now,
        )
    }
    /// A resource-reader IOException, distinct from raw diagnostic injection.
    pub fn reject_read_io(
        &mut self,
        token: &str,
        message: Option<crate::value::PklString>,
        now: u32,
    ) -> Result<(), RuntimeError> {
        self.session.reject_read(
            token,
            crate::host::ReadFailure::Io(crate::host::IoFailure::new(message)),
            now.into(),
        )
    }
    pub fn drain_cancelled_modules(&mut self) -> Vec<String> {
        self.session.drain_cancelled_modules()
    }
    pub fn reject(&mut self, token: &str, message: &str, now: u32) -> Result<(), RuntimeError> {
        self.reject_text(token, message.into(), now)
    }
    /// Inject an already constructed Host diagnostic; this is not an IOException.
    pub fn reject_text(
        &mut self,
        token: &str,
        message: crate::value::PklString,
        now: u32,
    ) -> Result<(), RuntimeError> {
        self.session.resume(
            token,
            Err(RuntimeError::new(ErrorKind::Host, message)),
            now.into(),
        )
    }
    pub fn set_frame_transformer(&mut self, mode: &str) -> Result<(), RuntimeError> {
        use crate::logging::FrameTransformer;
        self.session.set_frame_transformer(match mode {
            "default" => FrameTransformer::Default,
            "identity" => FrameTransformer::Identity,
            "host" => FrameTransformer::Host,
            _ => {
                return Err(unsupported(
                    "expected frame transformer default, identity or host",
                ));
            }
        })
    }
    pub fn complete_frame_transform(
        &mut self,
        token: &str,
        frame: Option<SourceFrame>,
        failure: Option<Vec<u16>>,
        now: u32,
    ) -> Result<(), RuntimeError> {
        let completion = match (frame, failure) {
            (Some(frame), None) => Ok(frame.into()),
            (None, Some(failure)) => Err(crate::logging::LoggerFailure::new(
                PklString::from_utf16(failure),
            )),
            _ => {
                return Err(unsupported(
                    "provide exactly one transformed frame or transformer failure",
                ));
            }
        };
        self.session
            .complete_frame_transform(token, completion, now.into())
    }
    pub fn acknowledge_log(
        &mut self,
        token: &str,
        failure: Option<Vec<u16>>,
        now: u32,
    ) -> Result<(), RuntimeError> {
        self.session.acknowledge_log(
            token,
            failure.map_or(Ok(()), |units| {
                Err(crate::logging::LoggerFailure::new(
                    crate::value::PklString::from_utf16(units),
                ))
            }),
            now.into(),
        )
    }
    pub fn close_from_callback(&mut self, token: &str) -> Result<(), RuntimeError> {
        self.session.close_from_callback(token)
    }
    pub fn collect(&mut self) -> crate::heap::HeapStats {
        self.session.collect(&[])
    }
    pub fn heap_slots(&self) -> usize {
        self.session.heap_stats().slots
    }
    pub fn context_depth(&self) -> usize {
        self.session.context_depth()
    }
    pub fn drain_cancelled_reads(&mut self) -> Vec<String> {
        self.session.drain_cancelled_reads()
    }
    pub fn cancel(&mut self) {
        self.session.cancel();
    }
    pub fn close(&mut self) {
        self.session.close();
    }
}
