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
) -> Result<String, JsValue> {
    pkl_runtime::evaluation::scalar_expression(
        &string(&source)?,
        &string(&uri)?,
        &string(&module_name)?,
        &string(&expression)?,
    )
    .map_err(error)
}
#[wasm_bindgen(js_name = roundtripString)]
pub fn roundtrip_string(input: JsString) -> Result<String, JsValue> {
    match probe::identity(Value::string(string(&input)?)).map_err(error)? {
        Value::String(value) => Ok(value.to_string()),
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
