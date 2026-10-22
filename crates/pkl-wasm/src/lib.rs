//! Browser experiment; all evaluation and lifecycle state lives in pkl-runtime.
use js_sys::{BigInt, JsString, Uint8Array};
use pkl_runtime::{probe, value::Value};
use wasm_bindgen::prelude::*;

fn error(e: impl ToString) -> JsValue {
    js_sys::Error::new(&e.to_string()).into()
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
) -> Result<String, JsValue> {
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
    .map(|result| {
        JsValue::from_str(&match result {
            Ok(value) => format!("value\t{value}"),
            Err(error) => format!("error\t{error}"),
        })
    })
    .collect())
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
    match probe::identity(Value::Int(integer(input)?)).map_err(error)? {
        Value::Int(value) => Ok(BigInt::from(value)),
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
                Value::Int(integer(value)?),
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
    if let Some(token) = event.token {
        set(&object, "token", JsValue::from_str(&token))?;
    }
    if let Some(uri) = event.uri {
        set(&object, "uri", JsValue::from_str(&uri))?;
    }
    if let Some(value) = event.value {
        set(&object, "value", Uint16Array::from(value.as_slice()).into())?;
    }
    if let Some(value) = event.error {
        set(&object, "error", Uint16Array::from(value.as_slice()).into())?;
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
    #[wasm_bindgen(js_name = startModule)]
    pub fn start_module(&mut self, now: f64, timeout: Option<f64>) -> Result<(), JsValue> {
        self.inner
            .start_module(unsigned(now)?, timeout.map(unsigned).transpose()?)
            .map_err(error)
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
            .map_err(error)
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
            .map_err(error)
    }
    pub fn poll(&mut self, now: f64, budget: f64) -> Result<JsValue, JsValue> {
        source_event(
            self.inner
                .poll(unsigned(now)?, unsigned(budget)?)
                .map_err(error)?,
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
            .map_err(error)
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
            .map_err(error)
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
                Value::Int(integer(value)?),
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
    #[wasm_bindgen(js_name = closeFromCallback)]
    pub fn close_from_callback(&mut self, token: JsString) -> Result<(), JsValue> {
        self.inner
            .close_from_callback(&string(&token)?)
            .map_err(error)
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
