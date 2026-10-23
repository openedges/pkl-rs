//! Browser experiment; all evaluation and lifecycle state lives in pkl-runtime.
use js_sys::{BigInt, JsString, Uint8Array};
use pkl_runtime::{
    probe,
    value::{PklString, Value},
};
use wasm_bindgen::prelude::*;

fn error(e: impl ToString) -> JsValue {
    js_sys::Error::new(&e.to_string()).into()
}
fn js_text(text: &PklString) -> JsString {
    let units: Vec<_> = text.encode_utf16().collect();
    // js-sys uses variadic String.fromCharCode; bound every call before joining.
    let pieces = js_sys::Array::new();
    for chunk in units.chunks(1024) {
        pieces.push(JsString::from_char_code(chunk).as_ref());
    }
    pieces.join("")
}
fn text_error(text: PklString) -> JsValue {
    let object = js_sys::Error::new("");
    match js_sys::Reflect::set(
        object.as_ref(),
        &JsValue::from_str("message"),
        js_text(&text).as_ref(),
    ) {
        Ok(_) => object.into(),
        Err(error) => error,
    }
}
fn unsigned(value: f64) -> Result<u32, JsValue> {
    probe::unsigned_argument(value).map_err(error)
}
fn string(value: &JsString) -> Result<String, JsValue> {
    if !JsValue::is_string(value.as_ref()) {
        return Err(error("expected string"));
    }
    let units: Vec<u16> = (0..value.length())
        .map(|i| value.char_code_at(i) as u16)
        .collect();
    String::from_utf16(&units).map_err(|_| error("unpaired UTF-16 surrogate is unsupported"))
}
fn pkl_text(value: &JsString) -> Result<PklString, JsValue> {
    if !JsValue::is_string(value.as_ref()) {
        return Err(error("expected string"));
    }
    Ok(PklString::from_utf16(
        (0..value.length())
            .map(|i| value.char_code_at(i) as u16)
            .collect::<Vec<_>>(),
    ))
}
fn integer(value: JsValue) -> Result<i64, JsValue> {
    if !value.is_bigint() {
        return Err(error("expected BigInt"));
    }
    let text = BigInt::from(value).to_string(10).map_err(JsValue::from)?;
    string(&text)?
        .parse()
        .map_err(|_| error("BigInt is outside signed 64-bit range"))
}

#[wasm_bindgen(js_name = primitive)]
pub fn primitive(family: JsString, input: JsString) -> Result<String, JsValue> {
    probe::primitive(&string(&family)?, &string(&input)?).map_err(error)
}
#[wasm_bindgen(js_name = evaluate)]
pub fn evaluate(source: JsString, member: JsString) -> Result<String, JsValue> {
    probe::evaluate_member(&string(&source)?, &string(&member)?)
        .map(probe::encode_value)
        .map_err(error)
}
/// Development-only scalar boundary with shared Pkl diagnostic rendering.
#[wasm_bindgen(js_name = evaluateLanguage)]
pub fn evaluate_language(
    source: JsString,
    uri: JsString,
    module_name: JsString,
    expression: JsString,
    power_assertions: Option<bool>,
) -> Result<JsString, JsValue> {
    pkl_runtime::evaluation::scalar_expression_text_with_options(
        &string(&source)?,
        &string(&uri)?,
        &string(&module_name)?,
        &string(&expression)?,
        pkl_runtime::evaluation::EvaluationOptions {
            power_assertions: power_assertions.unwrap_or(false),
        },
    )
    .map(|text| js_text(&text))
    .map_err(text_error)
}
/// Portable frontend bytes; no native process exit or generated-files execution.
#[wasm_bindgen(js_name = evaluateModuleFrontend)]
pub fn evaluate_module_frontend(
    source: JsString,
    uri: JsString,
    module_name: JsString,
    arguments: js_sys::Array,
) -> Result<js_sys::Object, JsValue> {
    use pkl_runtime::evaluation::frontend::{ModuleSourceInput, arguments::parse_module_eval_args};
    let arguments = arguments
        .iter()
        .map(|value| string(&JsString::from(value)))
        .collect::<Result<Vec<_>, _>>()?;
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
    let outcome = js_sys::Object::new();
    for (name, value) in [
        ("phase", JsValue::from_str(result.phase.as_str())),
        (
            "disposition",
            JsValue::from_str(result.disposition.as_str()),
        ),
        (
            "reason",
            result.reason.map_or(JsValue::NULL, JsValue::from_str),
        ),
        ("stdout", Uint8Array::from(result.stdout.as_slice()).into()),
        ("stderr", Uint8Array::from(result.stderr.as_slice()).into()),
        ("exitCode", JsValue::from(result.exit_code)),
    ] {
        js_sys::Reflect::set(&outcome, &JsValue::from_str(name), &value)?;
    }
    Ok(outcome)
}

/// Actual shared module-output bytes; PCF and encoding stay in pkl-runtime.
#[wasm_bindgen(js_name = evaluateModuleOutput)]
pub fn evaluate_module_output(
    source: JsString,
    uri: JsString,
    module_name: JsString,
    arguments: js_sys::Array,
) -> Result<js_sys::Object, JsValue> {
    use pkl_runtime::evaluation::frontend::{ModuleSourceInput, arguments::parse_module_eval_args};
    let arguments = arguments
        .iter()
        .map(|value| string(&JsString::from(value)))
        .collect::<Result<Vec<_>, _>>()?;
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
    let outcome = js_sys::Object::new();
    for (name, value) in [
        ("phase", JsValue::from_str(result.phase.as_str())),
        (
            "disposition",
            result
                .disposition
                .map_or(JsValue::NULL, |value| JsValue::from_str(value.as_str())),
        ),
        (
            "reason",
            result.reason.map_or(JsValue::NULL, JsValue::from_str),
        ),
        ("stdout", Uint8Array::from(result.stdout.as_slice()).into()),
        ("stderr", Uint8Array::from(result.stderr.as_slice()).into()),
        ("exitCode", JsValue::from(result.exit_code)),
    ] {
        js_sys::Reflect::set(&outcome, &JsValue::from_str(name), &value)?;
    }
    Ok(outcome)
}

/// Development-only ordered calls on one evaluator, retaining failures as data.
#[wasm_bindgen(js_name = evaluateLanguageSequence)]
pub fn evaluate_language_sequence(
    source: JsString,
    uri: JsString,
    module_name: JsString,
    expressions: js_sys::Array,
    power_assertions: Option<bool>,
) -> Result<js_sys::Array, JsValue> {
    let expressions = expressions
        .iter()
        .map(|value| string(&JsString::from(value)))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(
        pkl_runtime::evaluation::scalar_expressions_text_with_options(
            &string(&source)?,
            &string(&uri)?,
            &string(&module_name)?,
            &expressions,
            pkl_runtime::evaluation::EvaluationOptions {
                power_assertions: power_assertions.unwrap_or(false),
            },
        )
        .map_err(text_error)?
        .into_iter()
        .map(|result| {
            let (prefix, text) = match result {
                Ok(value) => ("value\t", value),
                Err(error) => ("error\t", error),
            };
            let units = prefix
                .encode_utf16()
                .chain(text.encode_utf16())
                .collect::<Vec<_>>();
            JsValue::from(js_text(&PklString::from_utf16(units)))
        })
        .collect(),
    )
}
#[wasm_bindgen(js_name = roundtripString)]
pub fn roundtrip_string(input: JsString) -> Result<String, JsValue> {
    match probe::identity(Value::string(string(&input)?)).map_err(error)? {
        Value::String(value) => Ok(value.to_java_utf8()),
        _ => Err(error("invalid engine result")),
    }
}
#[wasm_bindgen(js_name = roundtripInteger)]
pub fn roundtrip_integer(input: JsValue) -> Result<BigInt, JsValue> {
    match probe::identity(Value::Int(integer(input)?.into())).map_err(error)? {
        Value::Int(value) => Ok(BigInt::from(value.get())),
        _ => Err(error("invalid engine result")),
    }
}
#[wasm_bindgen(js_name = roundtripBytes)]
pub fn roundtrip_bytes(input: Uint8Array) -> Result<Uint8Array, JsValue> {
    if !JsValue::is_instance_of::<Uint8Array>(input.as_ref()) {
        return Err(error("expected Uint8Array"));
    }
    match probe::identity(Value::Bytes(input.to_vec().into())).map_err(error)? {
        Value::Bytes(value) => Ok(Uint8Array::from(value.as_ref())),
        _ => Err(error("invalid engine result")),
    }
}

#[wasm_bindgen]
pub struct HostProbe {
    inner: probe::HostProbe,
}
#[wasm_bindgen]
impl HostProbe {
    #[wasm_bindgen(constructor)]
    pub fn new(mode: JsString) -> Result<HostProbe, JsValue> {
        Ok(Self {
            inner: probe::HostProbe::new(&string(&mode)?).map_err(error)?,
        })
    }
    pub fn start(&mut self, now: f64, timeout: f64) -> Result<(), JsValue> {
        self.inner
            .start(unsigned(now)?, unsigned(timeout)?)
            .map_err(error)
    }
    pub fn poll(&mut self, now: f64, budget: f64) -> Result<String, JsValue> {
        Ok(self.inner.poll(unsigned(now)?, unsigned(budget)?))
    }
    pub fn reenter(&mut self, token: JsString, now: f64, timeout: f64) -> Result<(), JsValue> {
        self.inner
            .reenter(&string(&token)?, unsigned(now)?, unsigned(timeout)?)
            .map_err(error)
    }
    #[wasm_bindgen(js_name = drainCancelledReads)]
    pub fn drain_cancelled_reads(&mut self) -> Vec<String> {
        self.inner.drain_cancelled_reads()
    }
    #[wasm_bindgen(js_name = respondInteger)]
    pub fn respond_integer(
        &mut self,
        token: JsString,
        value: JsValue,
        now: f64,
    ) -> Result<(), JsValue> {
        self.inner
            .respond(
                &string(&token)?,
                Value::Int(integer(value)?.into()),
                unsigned(now)?,
            )
            .map_err(error)
    }
    #[wasm_bindgen(js_name = respondBytes)]
    pub fn respond_bytes(
        &mut self,
        token: JsString,
        value: Uint8Array,
        now: f64,
    ) -> Result<(), JsValue> {
        if !JsValue::is_instance_of::<Uint8Array>(value.as_ref()) {
            return Err(error("expected Uint8Array"));
        }
        self.inner
            .respond(
                &string(&token)?,
                Value::Bytes(value.to_vec().into()),
                unsigned(now)?,
            )
            .map_err(error)
    }
    pub fn reject(&mut self, token: JsString, message: JsString, now: f64) -> Result<(), JsValue> {
        self.inner
            .reject(&string(&token)?, &string(&message)?, unsigned(now)?)
            .map_err(error)
    }
    pub fn cancel(&mut self) {
        self.inner.cancel();
    }
    pub fn close(&mut self) {
        self.inner.close();
    }
}

fn source_event(event: probe::SourceEvent) -> Result<JsValue, JsValue> {
    use js_sys::{Array, Object, Reflect, Uint16Array};
    let object = Object::new();
    let set = |object: &Object, key: &str, value: JsValue| {
        Reflect::set(object, &JsValue::from_str(key), &value).map(|_| ())
    };
    set(&object, "kind", JsValue::from_str(event.kind))?;
    if let Some(code) = event.exit_code {
        set(&object, "exitCode", JsValue::from_f64(f64::from(code)))?;
    }
    for (key, value) in [
        ("phase", event.phase),
        ("disposition", event.disposition),
        ("reason", event.reason),
    ] {
        if let Some(value) = value {
            set(&object, key, JsValue::from_str(value))?;
        }
    }
    if let Some(token) = event.token {
        set(&object, "token", JsValue::from_str(&token))?;
    }
    if let Some(uri) = event.uri {
        set(&object, "uri", JsValue::from_str(&uri))?;
    }
    if let Some(value) = event.value {
        set(&object, "value", Uint16Array::from(value.as_slice()).into())?;
    }
    if let Some(value) = event.bytes {
        set(&object, "bytes", Uint8Array::from(value.as_slice()).into())?;
    }
    if let Some(value) = event.error {
        set(&object, "error", Uint16Array::from(value.as_slice()).into())?;
    }
    if let Some(value) = event.error_bytes {
        set(
            &object,
            "errorBytes",
            Uint8Array::from(value.as_slice()).into(),
        )?;
    }
    if let Some(module) = event.module {
        let record = Object::new();
        for (key, value) in [
            ("id", module.id),
            ("kind", module.kind.into()),
            ("originalUri", module.original_uri),
        ] {
            set(&record, key, JsValue::from_str(&value))?;
        }
        for (key, value) in [
            ("importerOriginalUri", module.importer_original_uri),
            ("importerResolvedUri", module.importer_resolved_uri),
            ("importUri", module.import_uri),
            ("resolvedUri", module.resolved_uri),
            ("sourceUri", module.source_uri),
        ] {
            if let Some(value) = value {
                set(&record, key, JsValue::from_str(&value))?;
            }
        }
        set(&object, "module", record.into())?;
    }
    if let Some(log) = event.log {
        let record = Object::new();
        set(&record, "level", JsValue::from_str(log.level))?;
        set(
            &record,
            "message",
            Uint16Array::from(log.message.as_slice()).into(),
        )?;
        set(
            &record,
            "moduleUri",
            Uint16Array::from(log.module_uri.as_slice()).into(),
        )?;
        set(
            &record,
            "memberName",
            log.member_name
                .as_deref()
                .map_or(JsValue::NULL, |units| Uint16Array::from(units).into()),
        )?;
        let lines = Array::new();
        for line in log.source_lines {
            lines.push(&Uint16Array::from(line.as_slice()));
        }
        set(&record, "sourceLines", lines.into())?;
        for (key, value) in [
            ("startLine", log.start_line),
            ("startColumn", log.start_column),
            ("endLine", log.end_line),
            ("endColumn", log.end_column),
        ] {
            set(&record, key, JsValue::from_f64(value.into()))?;
        }
        set(&object, "log", record.into())?;
    }
    Ok(object.into())
}

/// Development-only parsed-source session. Logger callbacks run in the caller
/// after poll returns; this object owns no JavaScript callback or I/O semantics.
#[wasm_bindgen(js_name = SourceSession)]
pub struct SourceSession {
    inner: probe::SourceProbe,
}
#[wasm_bindgen]
impl SourceSession {
    #[wasm_bindgen(js_name = forOutput)]
    #[allow(clippy::too_many_arguments)]
    pub fn for_output(
        source: JsString,
        uri: JsString,
        module_name: JsString,
        paths: js_sys::Array,
        arguments: js_sys::Array,
        trace_mode: Option<JsString>,
        host_logger: Option<bool>,
        host_frame_transformer: Option<bool>,
        power_assertions: Option<bool>,
    ) -> Result<SourceSession, JsValue> {
        use pkl_runtime::evaluation::frontend::{
            DiagnosticProfile, ModuleSourceInput, arguments::parse_module_eval_args,
        };
        let arguments = arguments
            .iter()
            .map(|v| string(&JsString::from(v)))
            .collect::<Result<Vec<_>, _>>()?;
        let mut invocation = parse_module_eval_args(&arguments).map_err(error)?;
        invocation.options.diagnostic_profile = DiagnosticProfile::ApiIdentity;
        let paths = paths
            .iter()
            .map(|v| string(&JsString::from(v)))
            .collect::<Result<Vec<_>, _>>()?;
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
            inner: probe::SourceProbe::new_output_text(
                ModuleSourceInput {
                    text: &source,
                    uri: &uri,
                    inferred_name: &name,
                },
                &paths,
                &invocation.options,
                options,
            )
            .map_err(|failure| text_error(failure.text))?,
        })
    }
    #[wasm_bindgen(js_name = forExpressions)]
    #[allow(clippy::too_many_arguments)]
    pub fn for_expressions(
        source: JsString,
        uri: JsString,
        module_name: JsString,
        paths: js_sys::Array,
        trace_mode: Option<JsString>,
        host_logger: Option<bool>,
        host_frame_transformer: Option<bool>,
        power_assertions: Option<bool>,
    ) -> Result<SourceSession, JsValue> {
        let paths = paths
            .iter()
            .map(|value| string(&JsString::from(value)))
            .collect::<Result<Vec<_>, _>>()?;
        let mode = trace_mode
            .as_ref()
            .map(string)
            .transpose()?
            .unwrap_or_else(|| "compact".into());
        Ok(Self {
            inner: probe::SourceProbe::new_expressions_text(
                &string(&source)?,
                &string(&uri)?,
                &string(&module_name)?,
                &paths,
                &mode,
                host_logger.unwrap_or(false),
                host_frame_transformer.unwrap_or(false),
                power_assertions.unwrap_or(false),
            )
            .map_err(text_error)?,
        })
    }
    #[wasm_bindgen(constructor)]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        source: JsString,
        uri: JsString,
        module_name: JsString,
        paths: js_sys::Array,
        trace_mode: Option<JsString>,
        host_logger: Option<bool>,
        host_frame_transformer: Option<bool>,
        power_assertions: Option<bool>,
    ) -> Result<SourceSession, JsValue> {
        let paths = paths
            .iter()
            .map(|value| string(&JsString::from(value)))
            .collect::<Result<Vec<_>, _>>()?;
        let mode = trace_mode
            .as_ref()
            .map(string)
            .transpose()?
            .unwrap_or_else(|| "compact".into());
        Ok(Self {
            inner: probe::SourceProbe::new_text(
                &string(&source)?,
                &string(&uri)?,
                &string(&module_name)?,
                &paths,
                &mode,
                host_logger.unwrap_or(false),
                host_frame_transformer.unwrap_or(false),
                power_assertions.unwrap_or(false),
            )
            .map_err(text_error)?,
        })
    }
    #[wasm_bindgen(js_name = startModule)]
    pub fn start_module(&mut self, now: f64, timeout: Option<f64>) -> Result<(), JsValue> {
        self.inner
            .start_module(unsigned(now)?, timeout.map(unsigned).transpose()?)
            .map_err(|failure| text_error(failure.message))
    }
    #[wasm_bindgen(js_name = startOutputBytes)]
    pub fn start_output_bytes(&mut self, now: f64, timeout: Option<f64>) -> Result<(), JsValue> {
        self.inner
            .start_output_bytes(unsigned(now)?, timeout.map(unsigned).transpose()?)
            .map_err(|failure| text_error(failure.message))
    }
    #[wasm_bindgen(js_name = reenterOutputBytes)]
    pub fn reenter_output_bytes(
        &mut self,
        token: JsString,
        now: f64,
        timeout: Option<f64>,
    ) -> Result<(), JsValue> {
        self.inner
            .reenter_output_bytes(
                &string(&token)?,
                unsigned(now)?,
                timeout.map(unsigned).transpose()?,
            )
            .map_err(|failure| text_error(failure.message))
    }
    #[wasm_bindgen(js_name = completeModuleCapabilities)]
    pub fn complete_module_capabilities(
        &mut self,
        token: JsString,
        local: bool,
        hierarchical: bool,
        fragment_paths: bool,
        now: f64,
    ) -> Result<(), JsValue> {
        self.inner
            .complete_module_capabilities(
                &string(&token)?,
                local,
                hierarchical,
                fragment_paths,
                unsigned(now)?,
            )
            .map_err(|failure| text_error(failure.message))
    }
    #[wasm_bindgen(js_name = completeModuleHasElement)]
    pub fn complete_module_has_element(
        &mut self,
        token: JsString,
        exists: bool,
        now: f64,
    ) -> Result<(), JsValue> {
        self.inner
            .complete_module_has_element(&string(&token)?, exists, unsigned(now)?)
            .map_err(|failure| text_error(failure.message))
    }
    #[wasm_bindgen(js_name = completeModuleResolution)]
    pub fn complete_module_resolution(
        &mut self,
        token: JsString,
        original_uri: JsString,
        resolved_uri: JsString,
        source_uri: JsString,
        now: f64,
    ) -> Result<(), JsValue> {
        self.inner
            .complete_module_resolution(
                &string(&token)?,
                &string(&original_uri)?,
                &string(&resolved_uri)?,
                &string(&source_uri)?,
                unsigned(now)?,
            )
            .map_err(|failure| text_error(failure.message))
    }
    #[wasm_bindgen(js_name = completeModuleSource)]
    pub fn complete_module_source(
        &mut self,
        token: JsString,
        text: JsString,
        inferred_name: JsString,
        now: f64,
    ) -> Result<(), JsValue> {
        self.inner
            .complete_module_source(
                &string(&token)?,
                &string(&text)?,
                &string(&inferred_name)?,
                unsigned(now)?,
            )
            .map_err(|failure| text_error(failure.message))
    }
    #[wasm_bindgen(js_name = rejectModuleResolution)]
    pub fn reject_module_resolution(
        &mut self,
        token: JsString,
        kind: JsString,
        message: Option<JsString>,
        now: f64,
    ) -> Result<(), JsValue> {
        self.inner
            .reject_module_text(
                &string(&token)?,
                false,
                &string(&kind)?,
                message.as_ref().map(pkl_text).transpose()?,
                unsigned(now)?,
            )
            .map_err(|failure| text_error(failure.message))
    }
    #[wasm_bindgen(js_name = rejectModuleSource)]
    pub fn reject_module_source(
        &mut self,
        token: JsString,
        kind: JsString,
        message: Option<JsString>,
        now: f64,
    ) -> Result<(), JsValue> {
        self.inner
            .reject_module_text(
                &string(&token)?,
                true,
                &string(&kind)?,
                message.as_ref().map(pkl_text).transpose()?,
                unsigned(now)?,
            )
            .map_err(|failure| text_error(failure.message))
    }
    #[wasm_bindgen(js_name = rejectModulePreparation)]
    pub fn reject_module_preparation(
        &mut self,
        token: JsString,
        has_element: bool,
        kind: JsString,
        message: Option<JsString>,
        now: f64,
    ) -> Result<(), JsValue> {
        self.inner
            .reject_module_preparation_text(
                &string(&token)?,
                has_element,
                &string(&kind)?,
                message.as_ref().map(pkl_text).transpose()?,
                unsigned(now)?,
            )
            .map_err(|failure| text_error(failure.message))
    }
    #[wasm_bindgen(js_name = resolvedReadUri)]
    pub fn resolved_read_uri(&self, token: JsString) -> Result<String, JsValue> {
        self.inner
            .resolved_read_uri(&string(&token)?)
            .map_err(|failure| text_error(failure.message))
    }
    #[wasm_bindgen(js_name = completeReadMissing)]
    pub fn complete_read_missing(&mut self, token: JsString, now: f64) -> Result<(), JsValue> {
        self.inner
            .complete_read_missing(&string(&token)?, unsigned(now)?)
            .map_err(|failure| text_error(failure.message))
    }
    #[wasm_bindgen(js_name = completeReadText)]
    pub fn complete_read_text(
        &mut self,
        token: JsString,
        units: js_sys::Uint16Array,
        now: f64,
    ) -> Result<(), JsValue> {
        if !JsValue::is_instance_of::<js_sys::Uint16Array>(units.as_ref()) {
            return Err(error("expected Uint16Array"));
        }
        self.inner
            .complete_read_text(
                &string(&token)?,
                PklString::from_utf16(units.to_vec()),
                unsigned(now)?,
            )
            .map_err(|failure| text_error(failure.message))
    }
    #[wasm_bindgen(js_name = completeReadResource)]
    pub fn complete_read_resource(
        &mut self,
        token: JsString,
        uri: JsString,
        bytes: Uint8Array,
        now: f64,
    ) -> Result<(), JsValue> {
        if !JsValue::is_instance_of::<Uint8Array>(bytes.as_ref()) {
            return Err(error("expected Uint8Array"));
        }
        self.inner
            .complete_read_resource(
                &string(&token)?,
                pkl_text(&uri)?,
                bytes.to_vec(),
                unsigned(now)?,
            )
            .map_err(|failure| text_error(failure.message))
    }
    #[wasm_bindgen(js_name = rejectReadSecurityManager)]
    pub fn reject_read_security_manager(
        &mut self,
        token: JsString,
        message: Option<JsString>,
        now: f64,
    ) -> Result<(), JsValue> {
        self.inner
            .reject_read_security_manager(
                &string(&token)?,
                message.as_ref().map(pkl_text).transpose()?,
                unsigned(now)?,
            )
            .map_err(|failure| text_error(failure.message))
    }
    #[wasm_bindgen(js_name = respondConfigured)]
    pub fn respond_configured(&mut self, token: JsString, now: f64) -> Result<(), JsValue> {
        self.inner
            .respond_configured(&string(&token)?, unsigned(now)?)
            .map_err(|failure| text_error(failure.message))
    }
    #[wasm_bindgen(js_name = startPath)]
    pub fn start_path(
        &mut self,
        index: f64,
        now: f64,
        timeout: Option<f64>,
    ) -> Result<(), JsValue> {
        self.inner
            .start_path(
                unsigned(index)?,
                unsigned(now)?,
                timeout.map(unsigned).transpose()?,
            )
            .map_err(|failure| text_error(failure.message))
    }
    #[wasm_bindgen(js_name = setupFailed)]
    pub fn setup_failed(&self) -> bool {
        self.inner.setup_failed()
    }
    #[wasm_bindgen(js_name = reenterPath)]
    pub fn reenter_path(
        &mut self,
        token: JsString,
        index: f64,
        now: f64,
        timeout: Option<f64>,
    ) -> Result<(), JsValue> {
        self.inner
            .reenter_path(
                &string(&token)?,
                unsigned(index)?,
                unsigned(now)?,
                timeout.map(unsigned).transpose()?,
            )
            .map_err(|failure| text_error(failure.message))
    }
    pub fn poll(&mut self, now: f64, budget: f64) -> Result<JsValue, JsValue> {
        source_event(
            self.inner
                .poll(unsigned(now)?, unsigned(budget)?)
                .map_err(|failure| text_error(failure.message))?,
        )
    }
    #[wasm_bindgen(js_name = acknowledgeLog)]
    pub fn acknowledge_log(
        &mut self,
        token: JsString,
        failure: Option<js_sys::Uint16Array>,
        now: f64,
    ) -> Result<(), JsValue> {
        if failure
            .as_ref()
            .is_some_and(|value| !JsValue::is_instance_of::<js_sys::Uint16Array>(value.as_ref()))
        {
            return Err(error("expected Uint16Array"));
        }
        self.inner
            .acknowledge_log(
                &string(&token)?,
                failure.map(|units| units.to_vec()),
                unsigned(now)?,
            )
            .map_err(|failure| text_error(failure.message))
    }
    #[wasm_bindgen(js_name = respondString)]
    pub fn respond_string(
        &mut self,
        token: JsString,
        value: js_sys::Uint16Array,
        now: f64,
    ) -> Result<(), JsValue> {
        if !JsValue::is_instance_of::<js_sys::Uint16Array>(value.as_ref()) {
            return Err(error("expected Uint16Array"));
        }
        self.inner
            .respond(
                &string(&token)?,
                Value::String(pkl_runtime::value::PklString::from_utf16(value.to_vec())),
                unsigned(now)?,
            )
            .map_err(|failure| text_error(failure.message))
    }
    #[wasm_bindgen(js_name = respondInteger)]
    pub fn respond_integer(
        &mut self,
        token: JsString,
        value: JsValue,
        now: f64,
    ) -> Result<(), JsValue> {
        self.inner
            .respond(
                &string(&token)?,
                Value::Int(integer(value)?.into()),
                unsigned(now)?,
            )
            .map_err(|failure| text_error(failure.message))
    }
    #[wasm_bindgen(js_name = respondBytes)]
    pub fn respond_bytes(
        &mut self,
        token: JsString,
        value: Uint8Array,
        now: f64,
    ) -> Result<(), JsValue> {
        if !JsValue::is_instance_of::<Uint8Array>(value.as_ref()) {
            return Err(error("expected Uint8Array"));
        }
        self.inner
            .respond(
                &string(&token)?,
                Value::Bytes(value.to_vec().into()),
                unsigned(now)?,
            )
            .map_err(|failure| text_error(failure.message))
    }
    pub fn reject(&mut self, token: JsString, message: JsString, now: f64) -> Result<(), JsValue> {
        self.inner
            .reject_text(&string(&token)?, pkl_text(&message)?, unsigned(now)?)
            .map_err(|failure| text_error(failure.message))
    }
    #[wasm_bindgen(js_name = rejectReadIo)]
    pub fn reject_read_io(
        &mut self,
        token: JsString,
        message: Option<JsString>,
        now: f64,
    ) -> Result<(), JsValue> {
        self.inner
            .reject_read_io(
                &string(&token)?,
                message.as_ref().map(pkl_text).transpose()?,
                unsigned(now)?,
            )
            .map_err(|failure| text_error(failure.message))
    }
    #[wasm_bindgen(js_name = closeFromCallback)]
    pub fn close_from_callback(&mut self, token: JsString) -> Result<(), JsValue> {
        self.inner
            .close_from_callback(&string(&token)?)
            .map_err(|failure| text_error(failure.message))
    }
    #[wasm_bindgen(js_name = drainCancelledModules)]
    pub fn drain_cancelled_modules(&mut self) -> Vec<String> {
        self.inner.drain_cancelled_modules()
    }
    #[wasm_bindgen(js_name = drainCancelledReads)]
    pub fn drain_cancelled_reads(&mut self) -> Vec<String> {
        self.inner.drain_cancelled_reads()
    }
    pub fn collect(&mut self) -> Result<u32, JsValue> {
        u32::try_from(self.inner.collect().slots).map_err(error)
    }
    #[wasm_bindgen(js_name = heapSlots)]
    pub fn heap_slots(&self) -> Result<u32, JsValue> {
        u32::try_from(self.inner.heap_slots()).map_err(error)
    }
    #[wasm_bindgen(js_name = contextDepth)]
    pub fn context_depth(&self) -> u32 {
        self.inner.context_depth() as u32
    }
    pub fn cancel(&mut self) {
        self.inner.cancel();
    }
    pub fn close(&mut self) {
        self.inner.close();
    }
}
