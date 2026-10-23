//! Node experiment; no separate evaluator or Java fallback.
use napi::{
    Error, Result,
    bindgen_prelude::{BigInt, Uint8Array, Utf16String},
};
use napi_derive::napi;
use pkl_runtime::{probe, value::Value};

fn error(e: impl ToString) -> Error {
    Error::from_reason(e.to_string())
}
fn unsigned(value: f64) -> Result<u32> {
    probe::unsigned_argument(value).map_err(error)
}
fn string(value: &Utf16String) -> Result<String> {
    String::from_utf16(value).map_err(|_| error("unpaired UTF-16 surrogate is unsupported"))
}
fn integer(value: BigInt) -> Result<i64> {
    if value.words.len() > 1 {
        return Err(error("BigInt is outside signed 64-bit range"));
    }
    let magnitude = value.words.first().copied().unwrap_or(0);
    if value.sign_bit && magnitude == (1u64 << 63) {
        return Ok(i64::MIN);
    }
    let magnitude =
        i64::try_from(magnitude).map_err(|_| error("BigInt is outside signed 64-bit range"))?;
    Ok(if value.sign_bit {
        -magnitude
    } else {
        magnitude
    })
}

#[napi]
pub fn primitive(family: Utf16String, input: Utf16String) -> Result<String> {
    probe::primitive(&string(&family)?, &string(&input)?).map_err(error)
}
#[napi]
pub fn evaluate(source: Utf16String, member: Utf16String) -> Result<String> {
    probe::evaluate_member(&string(&source)?, &string(&member)?)
        .map(probe::encode_value)
        .map_err(error)
}
/// Development-only scalar boundary with shared Pkl diagnostic rendering.
#[napi]
pub fn evaluate_language(
    source: Utf16String,
    uri: Utf16String,
    module_name: Utf16String,
    expression: Utf16String,
    power_assertions: Option<bool>,
) -> Result<String> {
    pkl_runtime::evaluation::scalar_expression_with_options(
        &string(&source)?,
        &string(&uri)?,
        &string(&module_name)?,
        &string(&expression)?,
        pkl_runtime::evaluation::EvaluationOptions {
            power_assertions: power_assertions.unwrap_or(false),
        },
    )
    .map_err(error)
}
/// Portable frontend bytes; this does not execute a native process or render output.
#[napi(object)]
pub struct ModuleFrontendOutcome {
    pub phase: String,
    pub disposition: String,
    pub reason: Option<String>,
    pub stdout: Uint8Array,
    pub stderr: Uint8Array,
    pub exit_code: i32,
}

#[napi]
pub fn evaluate_module_frontend(
    source: Utf16String,
    uri: Utf16String,
    module_name: Utf16String,
    arguments: Vec<Utf16String>,
) -> Result<ModuleFrontendOutcome> {
    use pkl_runtime::evaluation::frontend::{ModuleSourceInput, arguments::parse_module_eval_args};
    let arguments = arguments.iter().map(string).collect::<Result<Vec<_>>>()?;
    let invocation = parse_module_eval_args(&arguments).map_err(error)?;
    let source = string(&source)?;
    let uri = string(&uri)?;
    let module_name = string(&module_name)?;
    let result = pkl_runtime::evaluation::frontend::evaluate_module_frontend(
        ModuleSourceInput {
            text: &source,
            uri: &uri,
            inferred_name: &module_name,
        },
        &invocation.options,
    );
    Ok(ModuleFrontendOutcome {
        phase: result.phase.as_str().to_owned(),
        disposition: result.disposition.as_str().to_owned(),
        reason: result.reason.map(str::to_owned),
        stdout: Uint8Array::new(result.stdout),
        stderr: Uint8Array::new(result.stderr),
        exit_code: result.exit_code,
    })
}

/// Actual shared module-output bytes. The legacy frontend-only export remains
/// separate; platform code neither walks objects nor renders their values.
#[napi(object)]
pub struct ModuleOutputOutcome {
    pub phase: String,
    pub disposition: Option<String>,
    pub reason: Option<String>,
    pub stdout: Uint8Array,
    pub stderr: Uint8Array,
    pub exit_code: i32,
}

#[napi]
pub fn evaluate_module_output(
    source: Utf16String,
    uri: Utf16String,
    module_name: Utf16String,
    arguments: Vec<Utf16String>,
) -> Result<ModuleOutputOutcome> {
    use pkl_runtime::evaluation::frontend::{ModuleSourceInput, arguments::parse_module_eval_args};
    let arguments = arguments.iter().map(string).collect::<Result<Vec<_>>>()?;
    let invocation = parse_module_eval_args(&arguments).map_err(error)?;
    let source = string(&source)?;
    let uri = string(&uri)?;
    let module_name = string(&module_name)?;
    let result = pkl_runtime::evaluation::frontend::evaluate_module_output(
        ModuleSourceInput {
            text: &source,
            uri: &uri,
            inferred_name: &module_name,
        },
        &invocation.options,
    );
    Ok(ModuleOutputOutcome {
        phase: result.phase.as_str().to_owned(),
        disposition: result.disposition.map(|value| value.as_str().to_owned()),
        reason: result.reason.map(str::to_owned),
        stdout: Uint8Array::new(result.stdout),
        stderr: Uint8Array::new(result.stderr),
        exit_code: result.exit_code,
    })
}

/// Development-only ordered calls on one evaluator, retaining failures as data.
#[napi]
pub fn evaluate_language_sequence(
    source: Utf16String,
    uri: Utf16String,
    module_name: Utf16String,
    expressions: Vec<Utf16String>,
    power_assertions: Option<bool>,
) -> Result<Vec<String>> {
    let expressions = expressions.iter().map(string).collect::<Result<Vec<_>>>()?;
    Ok(pkl_runtime::evaluation::scalar_expressions_with_options(
        &string(&source)?,
        &string(&uri)?,
        &string(&module_name)?,
        &expressions,
        pkl_runtime::evaluation::EvaluationOptions {
            power_assertions: power_assertions.unwrap_or(false),
        },
    )
    .map_err(error)?
    .into_iter()
    .map(|result| match result {
        Ok(value) => format!("value\t{value}"),
        Err(error) => format!("error\t{error}"),
    })
    .collect())
}
#[napi]
pub fn roundtrip_string(input: Utf16String) -> Result<String> {
    match probe::identity(Value::string(string(&input)?)).map_err(error)? {
        Value::String(value) => Ok(value.to_java_utf8()),
        _ => Err(error("invalid engine result")),
    }
}
#[napi]
pub fn roundtrip_integer(input: BigInt) -> Result<BigInt> {
    match probe::identity(Value::Int(integer(input)?)).map_err(error)? {
        Value::Int(value) => Ok(BigInt::from(value)),
        _ => Err(error("invalid engine result")),
    }
}
#[napi]
pub fn roundtrip_bytes(input: Uint8Array) -> Result<Uint8Array> {
    match probe::identity(Value::Bytes(input.to_vec().into())).map_err(error)? {
        Value::Bytes(value) => Ok(Uint8Array::new(value.to_vec())),
        _ => Err(error("invalid engine result")),
    }
}

#[napi]
pub struct HostProbe {
    inner: probe::HostProbe,
}
#[napi]
impl HostProbe {
    #[napi(constructor)]
    pub fn new(mode: Utf16String) -> Result<Self> {
        Ok(Self {
            inner: probe::HostProbe::new(&string(&mode)?).map_err(error)?,
        })
    }
    #[napi]
    pub fn start(&mut self, now: f64, timeout: f64) -> Result<()> {
        self.inner
            .start(unsigned(now)?, unsigned(timeout)?)
            .map_err(error)
    }
    #[napi]
    pub fn poll(&mut self, now: f64, budget: f64) -> Result<String> {
        Ok(self.inner.poll(unsigned(now)?, unsigned(budget)?))
    }
    #[napi]
    pub fn reenter(&mut self, token: Utf16String, now: f64, timeout: f64) -> Result<()> {
        self.inner
            .reenter(&string(&token)?, unsigned(now)?, unsigned(timeout)?)
            .map_err(error)
    }
    #[napi]
    pub fn drain_cancelled_reads(&mut self) -> Vec<String> {
        self.inner.drain_cancelled_reads()
    }
    #[napi]
    pub fn respond_integer(&mut self, token: Utf16String, value: BigInt, now: f64) -> Result<()> {
        self.inner
            .respond(
                &string(&token)?,
                Value::Int(integer(value)?),
                unsigned(now)?,
            )
            .map_err(error)
    }
    #[napi]
    pub fn respond_bytes(&mut self, token: Utf16String, value: Uint8Array, now: f64) -> Result<()> {
        self.inner
            .respond(
                &string(&token)?,
                Value::Bytes(value.to_vec().into()),
                unsigned(now)?,
            )
            .map_err(error)
    }
    #[napi]
    pub fn reject(&mut self, token: Utf16String, message: Utf16String, now: f64) -> Result<()> {
        self.inner
            .reject(&string(&token)?, &string(&message)?, unsigned(now)?)
            .map_err(error)
    }
    #[napi]
    pub fn cancel(&mut self) {
        self.inner.cancel();
    }
    #[napi]
    pub fn close(&mut self) {
        self.inner.close();
    }
}

/// Lossless logger transport; all textual fields contain UTF-16 code units.
#[napi(object, use_nullable = true)]
pub struct SourceLog {
    pub level: String,
    pub message: napi::bindgen_prelude::Uint16Array,
    pub module_uri: napi::bindgen_prelude::Uint16Array,
    pub member_name: Option<napi::bindgen_prelude::Uint16Array>,
    pub source_lines: Vec<napi::bindgen_prelude::Uint16Array>,
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
}
#[napi(object)]
pub struct SourceModuleRequest {
    pub id: String,
    pub kind: String,
    pub importer_original_uri: Option<String>,
    pub importer_resolved_uri: Option<String>,
    pub import_uri: Option<String>,
    pub original_uri: String,
    pub resolved_uri: Option<String>,
    pub source_uri: Option<String>,
}
impl From<probe::SourceModuleRequest> for SourceModuleRequest {
    fn from(request: probe::SourceModuleRequest) -> Self {
        Self {
            id: request.id,
            kind: request.kind.into(),
            importer_original_uri: request.importer_original_uri,
            importer_resolved_uri: request.importer_resolved_uri,
            import_uri: request.import_uri,
            original_uri: request.original_uri,
            resolved_uri: request.resolved_uri,
            source_uri: request.source_uri,
        }
    }
}
#[napi(object)]
pub struct SourceEvent {
    pub kind: String,
    pub token: Option<String>,
    pub uri: Option<String>,
    pub value: Option<napi::bindgen_prelude::Uint16Array>,
    pub bytes: Option<Uint8Array>,
    pub error: Option<napi::bindgen_prelude::Uint16Array>,
    pub error_bytes: Option<Uint8Array>,
    pub log: Option<SourceLog>,
    pub module: Option<SourceModuleRequest>,
    pub exit_code: Option<i32>,
    pub phase: Option<String>,
    pub disposition: Option<String>,
    pub reason: Option<String>,
}
impl From<probe::SourceEvent> for SourceEvent {
    fn from(event: probe::SourceEvent) -> Self {
        use napi::bindgen_prelude::Uint16Array;
        Self {
            kind: event.kind.into(),
            token: event.token,
            uri: event.uri,
            value: event.value.map(Uint16Array::new),
            bytes: event.bytes.map(Uint8Array::new),
            error: event.error.map(Uint16Array::new),
            error_bytes: event.error_bytes.map(Uint8Array::new),
            module: event.module.map(Into::into),
            exit_code: event.exit_code,
            phase: event.phase.map(Into::into),
            disposition: event.disposition.map(Into::into),
            reason: event.reason.map(Into::into),
            log: event.log.map(|log| SourceLog {
                level: log.level.into(),
                message: Uint16Array::new(log.message),
                module_uri: Uint16Array::new(log.module_uri),
                member_name: log.member_name.map(Uint16Array::new),
                source_lines: log.source_lines.into_iter().map(Uint16Array::new).collect(),
                start_line: log.start_line,
                start_column: log.start_column,
                end_line: log.end_line,
                end_column: log.end_column,
            }),
        }
    }
}

/// Development-only parsed-source session; JS callbacks execute after poll has
/// released its Rust borrow and explicitly acknowledge the pending log token.
#[napi]
pub struct SourceSession {
    inner: probe::SourceProbe,
}
#[napi]
impl SourceSession {
    /// Output API identity profile; the separate synchronous CLI-style export
    /// accepts original CLI arguments and reports complete byte channels.
    #[napi(factory)]
    #[allow(clippy::too_many_arguments)]
    pub fn for_output(
        source: Utf16String,
        uri: Utf16String,
        module_name: Utf16String,
        paths: Vec<Utf16String>,
        arguments: Vec<Utf16String>,
        trace_mode: Option<Utf16String>,
        host_logger: Option<bool>,
        host_frame_transformer: Option<bool>,
        power_assertions: Option<bool>,
    ) -> Result<Self> {
        use pkl_runtime::evaluation::frontend::{
            DiagnosticProfile, ModuleSourceInput, arguments::parse_module_eval_args,
        };
        let arguments = arguments.iter().map(string).collect::<Result<Vec<_>>>()?;
        let mut invocation = parse_module_eval_args(&arguments).map_err(error)?;
        invocation.options.diagnostic_profile = DiagnosticProfile::ApiIdentity;
        let paths = paths.iter().map(string).collect::<Result<Vec<_>>>()?;
        let mode = trace_mode
            .as_ref()
            .map(string)
            .transpose()?
            .unwrap_or_else(|| "compact".into());
        let options = probe::SourceProbe::options(
            &mode,
            host_logger.unwrap_or(false),
            host_frame_transformer.unwrap_or(false),
            power_assertions.unwrap_or(false),
        )
        .map_err(error)?;
        let source = string(&source)?;
        let uri = string(&uri)?;
        let name = string(&module_name)?;
        Ok(Self {
            inner: probe::SourceProbe::new_output(
                ModuleSourceInput {
                    text: &source,
                    uri: &uri,
                    inferred_name: &name,
                },
                &paths,
                &invocation.options,
                options,
            )
            .map_err(|failure| {
                error(
                    String::from_utf8(failure.stderr)
                        .expect("shared diagnostic renderer emits UTF-8"),
                )
            })?,
        })
    }
    #[napi(factory)]
    #[allow(clippy::too_many_arguments)]
    pub fn for_expressions(
        source: Utf16String,
        uri: Utf16String,
        module_name: Utf16String,
        paths: Vec<Utf16String>,
        trace_mode: Option<Utf16String>,
        host_logger: Option<bool>,
        host_frame_transformer: Option<bool>,
        power_assertions: Option<bool>,
    ) -> Result<Self> {
        let paths = paths.iter().map(string).collect::<Result<Vec<_>>>()?;
        let mode = trace_mode
            .as_ref()
            .map(string)
            .transpose()?
            .unwrap_or_else(|| "compact".into());
        Ok(Self {
            inner: probe::SourceProbe::new_expressions(
                &string(&source)?,
                &string(&uri)?,
                &string(&module_name)?,
                &paths,
                &mode,
                host_logger.unwrap_or(false),
                host_frame_transformer.unwrap_or(false),
                power_assertions.unwrap_or(false),
            )
            .map_err(error)?,
        })
    }
    #[napi(constructor)]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        source: Utf16String,
        uri: Utf16String,
        module_name: Utf16String,
        paths: Vec<Utf16String>,
        trace_mode: Option<Utf16String>,
        host_logger: Option<bool>,
        host_frame_transformer: Option<bool>,
        power_assertions: Option<bool>,
    ) -> Result<Self> {
        let paths = paths.iter().map(string).collect::<Result<Vec<_>>>()?;
        let mode = trace_mode
            .as_ref()
            .map(string)
            .transpose()?
            .unwrap_or_else(|| "compact".into());
        Ok(Self {
            inner: probe::SourceProbe::new(
                &string(&source)?,
                &string(&uri)?,
                &string(&module_name)?,
                &paths,
                &mode,
                host_logger.unwrap_or(false),
                host_frame_transformer.unwrap_or(false),
                power_assertions.unwrap_or(false),
            )
            .map_err(error)?,
        })
    }
    #[napi]
    pub fn start_module(&mut self, now: f64, timeout: Option<f64>) -> Result<()> {
        self.inner
            .start_module(unsigned(now)?, timeout.map(unsigned).transpose()?)
            .map_err(error)
    }
    #[napi]
    pub fn start_output_bytes(&mut self, now: f64, timeout: Option<f64>) -> Result<()> {
        self.inner
            .start_output_bytes(unsigned(now)?, timeout.map(unsigned).transpose()?)
            .map_err(error)
    }
    #[napi]
    pub fn reenter_output_bytes(
        &mut self,
        token: Utf16String,
        now: f64,
        timeout: Option<f64>,
    ) -> Result<()> {
        self.inner
            .reenter_output_bytes(
                &string(&token)?,
                unsigned(now)?,
                timeout.map(unsigned).transpose()?,
            )
            .map_err(error)
    }
    #[napi]
    pub fn complete_module_resolution(
        &mut self,
        token: Utf16String,
        original_uri: Utf16String,
        resolved_uri: Utf16String,
        source_uri: Utf16String,
        now: f64,
    ) -> Result<()> {
        self.inner
            .complete_module_resolution(
                &string(&token)?,
                &string(&original_uri)?,
                &string(&resolved_uri)?,
                &string(&source_uri)?,
                unsigned(now)?,
            )
            .map_err(error)
    }
    #[napi]
    pub fn complete_module_source(
        &mut self,
        token: Utf16String,
        text: Utf16String,
        inferred_name: Utf16String,
        now: f64,
    ) -> Result<()> {
        self.inner
            .complete_module_source(
                &string(&token)?,
                &string(&text)?,
                &string(&inferred_name)?,
                unsigned(now)?,
            )
            .map_err(error)
    }
    #[napi]
    pub fn reject_module_resolution(
        &mut self,
        token: Utf16String,
        kind: Utf16String,
        message: Utf16String,
        now: f64,
    ) -> Result<()> {
        self.inner
            .reject_module(
                &string(&token)?,
                false,
                &string(&kind)?,
                &string(&message)?,
                unsigned(now)?,
            )
            .map_err(error)
    }
    #[napi]
    pub fn reject_module_source(
        &mut self,
        token: Utf16String,
        kind: Utf16String,
        message: Utf16String,
        now: f64,
    ) -> Result<()> {
        self.inner
            .reject_module(
                &string(&token)?,
                true,
                &string(&kind)?,
                &string(&message)?,
                unsigned(now)?,
            )
            .map_err(error)
    }
    #[napi]
    pub fn respond_configured(&mut self, token: Utf16String, now: f64) -> Result<()> {
        self.inner
            .respond_configured(&string(&token)?, unsigned(now)?)
            .map_err(error)
    }
    #[napi]
    pub fn start_path(&mut self, index: f64, now: f64, timeout: Option<f64>) -> Result<()> {
        self.inner
            .start_path(
                unsigned(index)?,
                unsigned(now)?,
                timeout.map(unsigned).transpose()?,
            )
            .map_err(error)
    }
    #[napi]
    pub fn setup_failed(&self) -> bool {
        self.inner.setup_failed()
    }
    #[napi]
    pub fn reenter_path(
        &mut self,
        token: Utf16String,
        index: f64,
        now: f64,
        timeout: Option<f64>,
    ) -> Result<()> {
        self.inner
            .reenter_path(
                &string(&token)?,
                unsigned(index)?,
                unsigned(now)?,
                timeout.map(unsigned).transpose()?,
            )
            .map_err(error)
    }
    #[napi]
    pub fn poll(&mut self, now: f64, budget: f64) -> Result<SourceEvent> {
        self.inner
            .poll(unsigned(now)?, unsigned(budget)?)
            .map(Into::into)
            .map_err(error)
    }
    #[napi]
    pub fn acknowledge_log(
        &mut self,
        token: Utf16String,
        failure: Option<napi::bindgen_prelude::Uint16Array>,
        now: f64,
    ) -> Result<()> {
        self.inner
            .acknowledge_log(
                &string(&token)?,
                failure.map(|units| units.to_vec()),
                unsigned(now)?,
            )
            .map_err(error)
    }
    #[napi]
    pub fn respond_string(
        &mut self,
        token: Utf16String,
        value: napi::bindgen_prelude::Uint16Array,
        now: f64,
    ) -> Result<()> {
        self.inner
            .respond(
                &string(&token)?,
                Value::String(pkl_runtime::value::PklString::from_utf16(value.to_vec())),
                unsigned(now)?,
            )
            .map_err(error)
    }
    #[napi]
    pub fn respond_integer(&mut self, token: Utf16String, value: BigInt, now: f64) -> Result<()> {
        self.inner
            .respond(
                &string(&token)?,
                Value::Int(integer(value)?),
                unsigned(now)?,
            )
            .map_err(error)
    }
    #[napi]
    pub fn respond_bytes(&mut self, token: Utf16String, value: Uint8Array, now: f64) -> Result<()> {
        self.inner
            .respond(
                &string(&token)?,
                Value::Bytes(value.to_vec().into()),
                unsigned(now)?,
            )
            .map_err(error)
    }
    #[napi]
    pub fn reject(&mut self, token: Utf16String, message: Utf16String, now: f64) -> Result<()> {
        self.inner
            .reject(&string(&token)?, &string(&message)?, unsigned(now)?)
            .map_err(error)
    }
    #[napi]
    pub fn close_from_callback(&mut self, token: Utf16String) -> Result<()> {
        self.inner
            .close_from_callback(&string(&token)?)
            .map_err(error)
    }
    #[napi]
    pub fn drain_cancelled_modules(&mut self) -> Vec<String> {
        self.inner.drain_cancelled_modules()
    }
    #[napi]
    pub fn drain_cancelled_reads(&mut self) -> Vec<String> {
        self.inner.drain_cancelled_reads()
    }
    #[napi]
    pub fn collect(&mut self) -> Result<u32> {
        u32::try_from(self.inner.collect().slots).map_err(error)
    }
    #[napi]
    pub fn heap_slots(&self) -> Result<u32> {
        u32::try_from(self.inner.heap_slots()).map_err(error)
    }
    #[napi]
    pub fn context_depth(&self) -> u32 {
        self.inner.context_depth() as u32
    }
    #[napi]
    pub fn cancel(&mut self) {
        self.inner.cancel();
    }
    #[napi]
    pub fn close(&mut self) {
        self.inner.close();
    }
}
