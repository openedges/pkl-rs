//! Browser experiment; all evaluation and lifecycle state lives in pkl-runtime.
use js_sys::{BigInt, JsString, Uint8Array};
use pkl_runtime::{
    probe,
    source::SourceText,
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
fn source_text(value: &JsString) -> Result<SourceText, JsValue> {
    pkl_text(value).map(SourceText::from)
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
    probe::evaluate_member_text(source_text(&source)?, pkl_text(&member)?)
        .map(probe::encode_value)
        .map_err(text_error)
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
        source_text(&source)?,
        pkl_text(&uri)?,
        pkl_text(&module_name)?,
        source_text(&expression)?,
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
    use pkl_runtime::evaluation::frontend::{
        LosslessModuleSourceInput, arguments::parse_module_eval_args,
    };
    let arguments = arguments
        .iter()
        .map(|value| string(&JsString::from(value)))
        .collect::<Result<Vec<_>, _>>()?;
    let invocation = parse_module_eval_args(&arguments).map_err(error)?;
    let source = source_text(&source)?;
    let uri = pkl_text(&uri)?;
    let module_name = pkl_text(&module_name)?;
    let result = pkl_runtime::evaluation::frontend::evaluate_module_frontend(
        LosslessModuleSourceInput {
            text: source,
            uri,
            inferred_name: module_name,
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
    use pkl_runtime::evaluation::frontend::{
        LosslessModuleSourceInput, arguments::parse_module_eval_args,
    };
    let arguments = arguments
        .iter()
        .map(|value| string(&JsString::from(value)))
        .collect::<Result<Vec<_>, _>>()?;
    let invocation = parse_module_eval_args(&arguments).map_err(error)?;
    let source = source_text(&source)?;
    let uri = pkl_text(&uri)?;
    let module_name = pkl_text(&module_name)?;
    let result = pkl_runtime::evaluation::frontend::evaluate_module_output(
        LosslessModuleSourceInput {
            text: source,
            uri,
            inferred_name: module_name,
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
        .map(|value| source_text(&JsString::from(value)))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(
        pkl_runtime::evaluation::scalar_expressions_source_with_options(
            source_text(&source)?,
            pkl_text(&uri)?,
            pkl_text(&module_name)?,
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
pub fn roundtrip_string(input: JsString) -> Result<JsString, JsValue> {
    match probe::identity(Value::String(pkl_text(&input)?)).map_err(error)? {
        Value::String(value) => Ok(js_text(&value)),
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

fn source_frame(frame: probe::SourceFrame) -> Result<JsValue, JsValue> {
    use js_sys::{Array, Object, Reflect, Uint16Array};
    let record = Object::new();
    let set = |key: &str, value: JsValue| Reflect::set(&record, &key.into(), &value).map(|_| ());
    set(
        "moduleUri",
        Uint16Array::from(frame.module_uri.as_slice()).into(),
    )?;
    set(
        "memberName",
        frame
            .member_name
            .as_deref()
            .map_or(JsValue::NULL, |units| Uint16Array::from(units).into()),
    )?;
    let lines = Array::new();
    for line in frame.source_lines {
        lines.push(&Uint16Array::from(line.as_slice()));
    }
    set("sourceLines", lines.into())?;
    for (key, value) in [
        ("startLine", frame.start_line),
        ("startColumn", frame.start_column),
        ("endLine", frame.end_line),
        ("endColumn", frame.end_column),
    ] {
        set(key, JsValue::from_f64(value.into()))?;
    }
    Ok(record.into())
}
fn completed_frame(value: JsValue) -> Result<probe::SourceFrame, JsValue> {
    use js_sys::{Array, Reflect, Uint16Array};
    let get = |key: &str| Reflect::get(&value, &key.into());
    let units = |value: JsValue| {
        value
            .dyn_into::<Uint16Array>()
            .map(|value| value.to_vec())
            .map_err(|_| error("expected Uint16Array frame text"))
    };
    let member = get("memberName")?;
    let lines = get("sourceLines")?;
    if !Array::is_array(&lines) {
        return Err(error("expected frame sourceLines array"));
    }
    let position = |key: &str| {
        unsigned(
            get(key)?
                .as_f64()
                .ok_or_else(|| error("expected numeric frame position"))?,
        )
    };
    Ok(probe::SourceFrame {
        module_uri: units(get("moduleUri")?)?,
        member_name: if member.is_null() {
            None
        } else {
            Some(units(member)?)
        },
        source_lines: Array::from(&lines)
            .iter()
            .map(units)
            .collect::<Result<_, _>>()?,
        start_line: position("startLine")?,
        start_column: position("startColumn")?,
        end_line: position("endLine")?,
        end_column: position("endColumn")?,
    })
}

/// Apply the existing shared frame policy without creating an evaluator session.
#[wasm_bindgen(js_name = transformFrameForPolicy)]
pub fn transform_frame_for_policy(frame: JsValue, policy: JsString) -> Result<JsValue, JsValue> {
    use pkl_runtime::logging::FrameTransformer;
    let policy = match string(&policy)?.as_str() {
        "default" => FrameTransformer::Default,
        "identity" => FrameTransformer::Identity,
        _ => return Err(error("expected default or identity frame policy")),
    };
    let frame = completed_frame(frame)?;
    let transformed = policy.transform(frame.into());
    source_frame(
        probe::SourceFrame::try_from(&transformed)
            .map_err(|failure| text_error(failure.message))?,
    )
}

fn strict_boolean(value: JsValue) -> Result<bool, JsValue> {
    value.as_bool().ok_or_else(|| error("expected boolean"))
}
fn glob_elements(elements: JsValue) -> Result<Vec<pkl_runtime::glob::PathElement>, JsValue> {
    if !js_sys::Array::is_array(&elements) {
        return Err(error("expected an array of glob path elements"));
    }
    js_sys::Array::from(&elements)
        .iter()
        .map(|element| {
            let name = js_sys::Reflect::get(&element, &JsValue::from_str("name"))?;
            if !JsValue::is_instance_of::<js_sys::Uint16Array>(&name) {
                return Err(error("glob path element name must be Uint16Array"));
            }
            let is_directory = js_sys::Reflect::get(&element, &JsValue::from_str("isDirectory"))?
                .as_bool()
                .ok_or_else(|| error("glob isDirectory must be a boolean"))?;
            Ok(pkl_runtime::glob::PathElement {
                name: PklString::from_utf16(js_sys::Uint16Array::new(&name).to_vec()),
                is_directory,
            })
        })
        .collect()
}
/// Codec-only control through the same decoder and event encoder as completions.
#[wasm_bindgen(js_name = globTransportCodec)]
pub fn glob_transport_codec(elements: JsValue) -> Result<js_sys::Array, JsValue> {
    probe::glob_transport_records(glob_elements(elements)?)
        .into_iter()
        .map(source_event)
        .collect()
}

fn source_event(event: probe::SourceTextEvent) -> Result<JsValue, JsValue> {
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
        ("callbackOrigin", event.callback_origin),
    ] {
        if let Some(value) = value {
            set(&object, key, JsValue::from_str(value))?;
        }
    }
    if let Some(token) = event.token {
        set(&object, "token", JsValue::from_str(&token))?;
    }
    if let Some(uri) = event.uri {
        set(&object, "uri", js_text(&uri).into())?;
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
            ("id", PklString::from(module.id)),
            ("kind", module.kind.into()),
            ("originalUri", module.original_uri),
        ] {
            set(&record, key, js_text(&value).into())?;
        }
        for (key, value) in [
            ("importerOriginalUri", module.importer_original_uri),
            ("importerResolvedUri", module.importer_resolved_uri),
            ("importUri", module.import_uri),
            ("resolvedUri", module.resolved_uri),
            ("sourceUri", module.source_uri),
            ("keyId", module.key_id.map(PklString::from)),
            ("keyKind", module.key_kind.map(PklString::from)),
            ("keyHandle", module.key_handle),
            ("resolvedKeyId", module.resolved_key_id.map(PklString::from)),
            ("resolvedKeyHandle", module.resolved_key_handle),
            (
                "selectionPhase",
                module.selection_phase.map(PklString::from),
            ),
            (
                "resolutionPhase",
                module.resolution_phase.map(PklString::from),
            ),
        ] {
            if let Some(value) = value {
                set(&record, key, js_text(&value).into())?;
            }
        }
        set(&object, "module", record.into())?;
    }
    if let Some(glob) = event.glob {
        let record = Object::new();
        for (key, value) in [
            ("id", glob.id),
            ("kind", glob.kind.into()),
            ("readerKind", glob.reader_kind.into()),
            ("readerId", glob.reader_id),
        ] {
            set(&record, key, JsValue::from_str(&value))?;
        }
        for (key, value) in [
            ("uri", glob.uri),
            ("readerUri", glob.reader_uri),
            ("readerScheme", glob.reader_scheme),
            ("readerHandle", glob.reader_handle),
            ("keyKind", glob.key_kind.map(PklString::from)),
        ] {
            if let Some(value) = value {
                set(&record, key, js_text(&value).into())?;
            }
        }
        set(&object, "glob", record.into())?;
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
    if let Some(frame) = event.frame {
        set(&object, "frame", source_frame(frame)?)?;
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
            DiagnosticProfile, LosslessModuleSourceInput, arguments::parse_module_eval_args,
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
        let source = source_text(&source)?;
        let uri = pkl_text(&uri)?;
        let name = pkl_text(&module_name)?;
        Ok(Self {
            inner: probe::SourceProbe::new_output_text(
                LosslessModuleSourceInput {
                    text: source,
                    uri,
                    inferred_name: name,
                },
                &paths,
                &invocation.options,
                options,
            )
            .map_err(|failure| text_error(failure.text))?,
        })
    }
    #[wasm_bindgen(js_name = moduleEvaluator)]
    pub fn module_evaluator(
        trace_mode: Option<JsString>,
        host_logger: Option<bool>,
        host_frame_transformer: Option<bool>,
        power_assertions: Option<bool>,
    ) -> Result<SourceSession, JsValue> {
        let mode = trace_mode
            .as_ref()
            .map(string)
            .transpose()?
            .unwrap_or_else(|| "compact".into());
        Ok(Self {
            inner: probe::SourceProbe::new_module_evaluator(
                &mode,
                host_logger.unwrap_or(false),
                host_frame_transformer.unwrap_or(false),
                power_assertions.unwrap_or(false),
            )
            .map_err(text_error)?,
        })
    }
    #[allow(clippy::too_many_arguments)]
    #[wasm_bindgen(js_name = evaluateModuleSource)]
    pub fn evaluate_module_source(
        &mut self,
        uri: JsString,
        contents: Option<JsString>,
        expression: JsString,
        now: f64,
        timeout: Option<f64>,
    ) -> Result<(), JsValue> {
        let contents = contents.as_ref().map(source_text).transpose()?;
        self.inner
            .evaluate_module_source_text(
                pkl_text(&uri)?,
                contents,
                source_text(&expression)?,
                unsigned(now)?,
                timeout.map(unsigned).transpose()?,
            )
            .map_err(|failure| text_error(failure.message))
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
            .map(|value| source_text(&JsString::from(value)))
            .collect::<Result<Vec<_>, _>>()?;
        let mode = trace_mode
            .as_ref()
            .map(string)
            .transpose()?
            .unwrap_or_else(|| "compact".into());
        Ok(Self {
            inner: probe::SourceProbe::new_expressions_source(
                source_text(&source)?,
                pkl_text(&uri)?,
                pkl_text(&module_name)?,
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
                source_text(&source)?,
                pkl_text(&uri)?,
                pkl_text(&module_name)?,
                &paths,
                &mode,
                host_logger.unwrap_or(false),
                host_frame_transformer.unwrap_or(false),
                power_assertions.unwrap_or(false),
            )
            .map_err(text_error)?,
        })
    }
    #[wasm_bindgen(js_name = setGlobTestMode)]
    pub fn set_glob_test_mode(&mut self, enabled: JsValue) -> Result<(), JsValue> {
        self.inner
            .set_glob_test_mode(strict_boolean(enabled)?)
            .map_err(|e| text_error(e.message))
    }
    #[wasm_bindgen(js_name = completeGlobReader)]
    pub fn complete_glob_reader(
        &mut self,
        token: JsString,
        reader_id: JsString,
        scheme: JsString,
        handle: JsString,
        now: f64,
    ) -> Result<(), JsValue> {
        self.inner
            .complete_glob_reader(
                &string(&token)?,
                &string(&reader_id)?,
                pkl_text(&scheme)?,
                pkl_text(&handle)?,
                unsigned(now)?,
            )
            .map_err(|e| text_error(e.message))
    }
    #[wasm_bindgen(js_name = completeGlobBoolean)]
    pub fn complete_glob_boolean(
        &mut self,
        token: JsString,
        kind: JsString,
        value: JsValue,
        now: f64,
    ) -> Result<(), JsValue> {
        self.inner
            .complete_glob_boolean(
                &string(&token)?,
                &string(&kind)?,
                strict_boolean(value)?,
                unsigned(now)?,
            )
            .map_err(|e| text_error(e.message))
    }
    #[wasm_bindgen(js_name = completeGlobElements)]
    pub fn complete_glob_elements(
        &mut self,
        token: JsString,
        elements: JsValue,
        now: f64,
    ) -> Result<(), JsValue> {
        self.inner
            .complete_glob_elements(&string(&token)?, glob_elements(elements)?, unsigned(now)?)
            .map_err(|e| text_error(e.message))
    }
    #[wasm_bindgen(js_name = rejectGlob)]
    pub fn reject_glob(
        &mut self,
        token: JsString,
        kind: JsString,
        message: Option<JsString>,
        now: f64,
    ) -> Result<(), JsValue> {
        self.inner
            .reject_glob(
                &string(&token)?,
                &string(&kind)?,
                message.as_ref().map(pkl_text).transpose()?,
                unsigned(now)?,
            )
            .map_err(|e| text_error(e.message))
    }
    #[wasm_bindgen(js_name = completeDefaultGlob)]
    pub fn complete_default_glob(&mut self, token: JsString, now: f64) -> Result<bool, JsValue> {
        self.inner
            .complete_default_glob(&string(&token)?, unsigned(now)?)
            .map_err(|e| text_error(e.message))
    }
    #[wasm_bindgen(js_name = drainCancelledGlobs)]
    pub fn drain_cancelled_globs(&mut self) -> js_sys::Array {
        self.inner
            .drain_cancelled_globs()
            .into_iter()
            .map(JsValue::from)
            .collect()
    }
    #[wasm_bindgen(js_name = resolveGlobHostUri)]
    pub fn resolve_glob_host_uri(
        &self,
        base: JsString,
        relative: JsString,
        fragment_paths: JsValue,
    ) -> Result<JsString, JsValue> {
        probe::SourceProbe::resolve_glob_host_uri(
            &pkl_text(&base)?,
            &pkl_text(&relative)?,
            strict_boolean(fragment_paths)?,
        )
        .map(|uri| js_text(&uri))
        .map_err(|e| text_error(e.message))
    }
    #[wasm_bindgen(js_name = normalizeGlobHostUri)]
    pub fn normalize_glob_host_uri(&self, uri: JsString) -> Result<JsString, JsValue> {
        probe::SourceProbe::normalize_glob_host_uri(&pkl_text(&uri)?)
            .map(|uri| js_text(&uri))
            .map_err(|e| text_error(e.message))
    }
    #[wasm_bindgen(js_name = rejectImportUri)]
    pub fn reject_import_uri(
        &mut self,
        token: JsString,
        key_id: JsString,
        kind: JsString,
        message: Option<JsString>,
        now: f64,
    ) -> Result<(), JsValue> {
        self.inner
            .reject_import_uri(
                &string(&token)?,
                &string(&key_id)?,
                &string(&kind)?,
                message.as_ref().map(pkl_text).transpose()?,
                unsigned(now)?,
            )
            .map_err(|e| text_error(e.message))
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
    #[wasm_bindgen(js_name = completeModuleImportCheck)]
    pub fn complete_module_import_check(
        &mut self,
        token: JsString,
        now: f64,
    ) -> Result<(), JsValue> {
        self.inner
            .complete_module_import_check(&string(&token)?, unsigned(now)?)
            .map_err(|failure| text_error(failure.message))
    }
    #[wasm_bindgen(js_name = rejectModuleImportCheck)]
    pub fn reject_module_import_check(
        &mut self,
        token: JsString,
        kind: JsString,
        message: Option<JsString>,
        now: f64,
    ) -> Result<(), JsValue> {
        self.inner
            .reject_module_import_check(
                &string(&token)?,
                &string(&kind)?,
                message.as_ref().map(pkl_text).transpose()?,
                unsigned(now)?,
            )
            .map_err(|failure| text_error(failure.message))
    }
    #[wasm_bindgen(js_name = completeDefaultModule)]
    pub fn complete_default_module(&mut self, token: JsString, now: f64) -> Result<bool, JsValue> {
        self.inner
            .complete_default_module(&string(&token)?, unsigned(now)?)
            .map_err(|failure| text_error(failure.message))
    }
    #[allow(clippy::too_many_arguments)]
    #[wasm_bindgen(js_name = completeModuleKey)]
    pub fn complete_module_key(
        &mut self,
        token: JsString,
        key_id: JsString,
        uri: JsString,
        kind: JsString,
        handle: JsString,
        now: f64,
    ) -> Result<(), JsValue> {
        self.inner
            .complete_module_key(
                &string(&token)?,
                &string(&key_id)?,
                pkl_text(&uri)?,
                &string(&kind)?,
                pkl_text(&handle)?,
                unsigned(now)?,
            )
            .map_err(|failure| text_error(failure.message))
    }
    #[allow(clippy::too_many_arguments)]
    #[wasm_bindgen(js_name = completeModuleCachePolicy)]
    pub fn complete_module_cache_policy(
        &mut self,
        token: JsString,
        key_id: JsString,
        cached: bool,
        now: f64,
    ) -> Result<(), JsValue> {
        self.inner
            .complete_module_cache_policy(
                &string(&token)?,
                &string(&key_id)?,
                cached,
                unsigned(now)?,
            )
            .map_err(|failure| text_error(failure.message))
    }
    #[wasm_bindgen(js_name = completeImportUri)]
    pub fn complete_import_uri(
        &mut self,
        token: JsString,
        key_id: JsString,
        uri: JsString,
        now: f64,
    ) -> Result<(), JsValue> {
        self.inner
            .complete_import_uri(
                &string(&token)?,
                &string(&key_id)?,
                pkl_text(&uri)?,
                unsigned(now)?,
            )
            .map_err(|failure| text_error(failure.message))
    }
    #[allow(clippy::too_many_arguments)]
    #[wasm_bindgen(js_name = reenterModuleSource)]
    pub fn reenter_module_source(
        &mut self,
        token: JsString,
        uri: JsString,
        contents: Option<JsString>,
        expression: JsString,
        now: f64,
        timeout: Option<f64>,
    ) -> Result<(), JsValue> {
        let contents = contents.as_ref().map(source_text).transpose()?;
        self.inner
            .reenter_module_source_text(
                &string(&token)?,
                pkl_text(&uri)?,
                contents,
                source_text(&expression)?,
                unsigned(now)?,
                timeout.map(unsigned).transpose()?,
            )
            .map_err(|failure| text_error(failure.message))
    }
    #[allow(clippy::too_many_arguments)]
    #[wasm_bindgen(js_name = completeModuleFragmentPaths)]
    pub fn complete_module_fragment_paths(
        &mut self,
        token: JsString,
        key_id: JsString,
        fragment_paths: bool,
        now: f64,
    ) -> Result<(), JsValue> {
        self.inner
            .complete_module_fragment_paths(
                &string(&token)?,
                &string(&key_id)?,
                fragment_paths,
                unsigned(now)?,
            )
            .map_err(|failure| text_error(failure.message))
    }
    #[allow(clippy::too_many_arguments)]
    #[wasm_bindgen(js_name = completeModuleResolvedKey)]
    pub fn complete_module_resolved_key(
        &mut self,
        token: JsString,
        key_id: JsString,
        resolved_key_id: JsString,
        handle: JsString,
        original_uri: JsString,
        resolved_uri: JsString,
        source_uri: JsString,
        now: f64,
    ) -> Result<(), JsValue> {
        self.inner
            .complete_module_resolved_key(
                &string(&token)?,
                &string(&key_id)?,
                &string(&resolved_key_id)?,
                pkl_text(&handle)?,
                pkl_text(&original_uri)?,
                pkl_text(&resolved_uri)?,
                pkl_text(&source_uri)?,
                unsigned(now)?,
            )
            .map_err(|failure| text_error(failure.message))
    }
    #[allow(clippy::too_many_arguments)]
    #[wasm_bindgen(js_name = rejectModuleKey)]
    pub fn reject_module_key(
        &mut self,
        token: JsString,
        cache_policy: bool,
        kind: JsString,
        message: Option<JsString>,
        now: f64,
    ) -> Result<(), JsValue> {
        self.inner
            .reject_module_key(
                &string(&token)?,
                cache_policy,
                &string(&kind)?,
                message.as_ref().map(pkl_text).transpose()?,
                unsigned(now)?,
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
                pkl_text(&original_uri)?,
                pkl_text(&resolved_uri)?,
                pkl_text(&source_uri)?,
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
                source_text(&text)?,
                pkl_text(&inferred_name)?,
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
    pub fn resolved_read_uri(&self, token: JsString) -> Result<JsString, JsValue> {
        self.inner
            .resolved_read_uri_text(&string(&token)?)
            .map(|uri| js_text(&uri))
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
                .poll_text(unsigned(now)?, unsigned(budget)?)
                .map_err(|failure| text_error(failure.message))?,
        )
    }
    #[wasm_bindgen(js_name = setFrameTransformer)]
    pub fn set_frame_transformer(&mut self, mode: JsString) -> Result<(), JsValue> {
        self.inner
            .set_frame_transformer(&string(&mode)?)
            .map_err(|failure| text_error(failure.message))
    }
    #[wasm_bindgen(js_name = completeFrameTransform)]
    pub fn complete_frame_transform(
        &mut self,
        token: JsString,
        frame: JsValue,
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
            .complete_frame_transform(
                &string(&token)?,
                if frame.is_null() || frame.is_undefined() {
                    None
                } else {
                    Some(completed_frame(frame)?)
                },
                failure.map(|value| value.to_vec()),
                unsigned(now)?,
            )
            .map_err(|failure| text_error(failure.message))
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
