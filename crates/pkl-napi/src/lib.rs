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
) -> Result<String> {
    pkl_runtime::evaluation::scalar_expression(
        &string(&source)?,
        &string(&uri)?,
        &string(&module_name)?,
        &string(&expression)?,
    )
    .map_err(error)
}
#[napi]
pub fn roundtrip_string(input: Utf16String) -> Result<String> {
    match probe::identity(Value::string(string(&input)?)).map_err(error)? {
        Value::String(value) => Ok(value.to_string()),
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
