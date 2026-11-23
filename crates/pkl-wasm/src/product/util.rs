//! Shared boundary helpers for lossless UTF-16, exact i64 BigInt, 64-bit unsigned
//! monotonic clocks/timeouts, owned Uint8Array bytes, and structured PklError projection.

use js_sys::{Array, BigInt, JsString, Object, Reflect, Uint8Array};
use pkl_runtime::{logging::LogFrame, source::SourceText, value::PklString};
use std::sync::{
    Arc,
    atomic::{AtomicU8, Ordering},
};
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::{JsCast, JsValue};

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(extends = JsString)]
    type WellFormedJsString;

    #[wasm_bindgen(method, js_name = isWellFormed)]
    fn is_well_formed(this: &WellFormedJsString) -> bool;
}

#[inline]
fn js_string_is_well_formed(value: &JsString) -> bool {
    value.unchecked_ref::<WellFormedJsString>().is_well_formed()
}

static HAS_IS_WELL_FORMED: AtomicU8 = AtomicU8::new(0);

#[inline]
fn supports_is_well_formed() -> bool {
    match HAS_IS_WELL_FORMED.load(Ordering::Relaxed) {
        1 => true,
        2 => false,
        _ => {
            // `Reflect.get` throws `TypeError` on primitive JS strings; probe
            // `globalThis.String.prototype.isWellFormed` instead.
            let ok = Reflect::get(js_sys::global().as_ref(), &JsValue::from_str("String"))
                .ok()
                .and_then(|ctor| Reflect::get(&ctor, &JsValue::from_str("prototype")).ok())
                .and_then(|proto| Reflect::get(&proto, &JsValue::from_str("isWellFormed")).ok())
                .is_some_and(|v| v.is_function());
            HAS_IS_WELL_FORMED.store(if ok { 1 } else { 2 }, Ordering::Relaxed);
            ok
        }
    }
}

/// Maximum exact integer representable in an IEEE-754 double (`2^53 - 1`).
pub const MAX_SAFE_INTEGER_F64: f64 = 9_007_199_254_740_991.0;

/// Structured error crossing the Wasm <-> JS boundary without losing error kind,
/// optional missing capability name, or unpaired UTF-16 diagnostic code units.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StructuredError {
    pub kind: &'static str,
    pub message: PklString,
    pub capability: Option<&'static str>,
}

impl StructuredError {
    pub fn new(kind: &'static str, message: impl Into<PklString>) -> Self {
        Self {
            kind,
            message: message.into(),
            capability: None,
        }
    }

    pub fn evaluation(message: impl Into<PklString>) -> Self {
        Self::new("evaluation", message)
    }

    pub fn security(message: impl Into<PklString>) -> Self {
        Self::new("security", message)
    }

    pub fn timeout(message: impl Into<PklString>) -> Self {
        Self::new("timeout", message)
    }

    pub fn closed(message: impl Into<PklString>) -> Self {
        Self::new("closed", message)
    }

    pub fn cancelled(message: impl Into<PklString>) -> Self {
        Self::new("cancelled", message)
    }

    pub fn configuration(message: impl Into<PklString>) -> Self {
        Self::new("configuration", message)
    }

    pub fn decode(message: impl Into<PklString>) -> Self {
        Self::new("decode", message)
    }

    pub fn io(message: impl Into<PklString>) -> Self {
        Self::new("io", message)
    }

    pub fn invalid_argument(message: impl Into<PklString>) -> Self {
        Self::new("invalid-argument", message)
    }

    pub fn capability_unavailable(capability: &'static str, message: impl Into<PklString>) -> Self {
        Self {
            kind: "capability-unavailable",
            message: message.into(),
            capability: Some(capability),
        }
    }

    pub fn into_js_value(self) -> JsValue {
        let err = js_sys::Error::new("");
        let _ = Reflect::set(
            err.as_ref(),
            &JsValue::from_str("message"),
            js_string_from_pkl(&self.message).as_ref(),
        );
        let _ = Reflect::set(
            err.as_ref(),
            &JsValue::from_str("__pklKind"),
            &JsValue::from_str(self.kind),
        );
        if let Some(cap) = self.capability {
            let _ = Reflect::set(
                err.as_ref(),
                &JsValue::from_str("__pklCapability"),
                &JsValue::from_str(cap),
            );
        }
        err.into()
    }
}

impl From<StructuredError> for JsValue {
    fn from(err: StructuredError) -> Self {
        err.into_js_value()
    }
}

/// Pure-Rust validation for JS `number` 64-bit unsigned clock/timeout arguments (`0..=2^53-1`).
pub fn parse_u64_from_f64(value: f64) -> Result<u64, String> {
    if !value.is_finite() {
        return Err("expected finite non-negative integer".to_string());
    }
    if value < 0.0 || value.fract() != 0.0 {
        return Err("expected non-negative integer".to_string());
    }
    if value > MAX_SAFE_INTEGER_F64 {
        return Err(
            "number exceeds Number.MAX_SAFE_INTEGER; pass a BigInt for values above 2^53 - 1"
                .to_string(),
        );
    }
    Ok(value as u64)
}

/// Pure-Rust validation for decimal `BigInt` string as unsigned 64-bit integer (`0..=u64::MAX`).
pub fn parse_u64_from_decimal(text: &str) -> Result<u64, String> {
    if text.starts_with('-') {
        return Err("expected non-negative 64-bit integer".to_string());
    }
    text.parse::<u64>()
        .map_err(|_| "BigInt is outside unsigned 64-bit range (0..=2^64-1)".to_string())
}

/// Pure-Rust validation for decimal `BigInt` string as signed 64-bit integer (`i64::MIN..=i64::MAX`).
pub fn parse_i64_from_decimal(text: &str) -> Result<i64, String> {
    text.parse::<i64>()
        .map_err(|_| "BigInt is outside signed 64-bit range (-2^63..=2^63-1)".to_string())
}

/// Validates a JS `number` (safe integer `0..=2^53-1`) or `bigint` (`0n..=u64::MAX`) as `u64`.
pub fn parse_u64_js(value: &JsValue) -> Result<u64, StructuredError> {
    if let Some(num) = value.as_f64() {
        return parse_u64_from_f64(num).map_err(StructuredError::invalid_argument);
    }
    if value.is_bigint() {
        let bigint = BigInt::from(value.clone());
        let text = bigint
            .to_string(10)
            .map_err(|_| StructuredError::invalid_argument("failed to format BigInt"))?;
        let decimal = rust_string_from_js_string(&text)?;
        return parse_u64_from_decimal(&decimal).map_err(StructuredError::invalid_argument);
    }
    Err(StructuredError::invalid_argument(
        "expected non-negative integer number or BigInt",
    ))
}

/// Validates an optional JS clock/timeout value (`null`/`undefined` -> `None`).
pub fn parse_opt_u64_js(value: &JsValue) -> Result<Option<u64>, StructuredError> {
    if value.is_null() || value.is_undefined() {
        return Ok(None);
    }
    parse_u64_js(value).map(Some)
}

/// Validates a JS `BigInt` strictly within signed 64-bit integer range (`i64::MIN..=i64::MAX`).
pub fn parse_i64_js(value: &JsValue) -> Result<i64, StructuredError> {
    if !value.is_bigint() {
        return Err(StructuredError::invalid_argument("expected BigInt"));
    }
    let bigint = BigInt::from(value.clone());
    let text = bigint
        .to_string(10)
        .map_err(|_| StructuredError::invalid_argument("failed to format BigInt"))?;
    let decimal = rust_string_from_js_string(&text)?;
    parse_i64_from_decimal(&decimal).map_err(StructuredError::invalid_argument)
}

/// Converts a `PklString` to a JS `String` preserving all UTF-16 code units (including unpaired surrogates).
pub fn js_string_from_pkl(text: &PklString) -> JsString {
    if let Some(utf8) = text.as_str() {
        return JsString::from(utf8);
    }
    let units: Vec<u16> = text.encode_utf16().collect();
    if units.len() <= 1024 {
        return JsString::from_char_code(&units);
    }
    let pieces = Array::new();
    for chunk in units.chunks(1024) {
        pieces.push(JsString::from_char_code(chunk).as_ref());
    }
    pieces.join("")
}

/// Converts a JS `String` to a `PklString` preserving all UTF-16 code units losslessly.
pub fn pkl_string_from_js(value: &JsValue) -> Result<PklString, StructuredError> {
    if !value.is_string() {
        return Err(StructuredError::invalid_argument("expected string"));
    }
    let js_str = JsString::from(value.clone());
    Ok(pkl_string_from_js_string(&js_str))
}

pub fn pkl_string_from_js_string(value: &JsString) -> PklString {
    if supports_is_well_formed()
        && js_string_is_well_formed(value)
        && let Some(s) = value.as_string()
    {
        return PklString::from(s);
    }
    let len = value.length();
    let mut units = Vec::with_capacity(len as usize);
    for i in 0..len {
        units.push(value.char_code_at(i) as u16);
    }
    PklString::from_utf16(units)
}

/// Converts a JS `String` to a `SourceText` preserving all UTF-16 code units losslessly.
pub fn source_text_from_js(value: &JsValue) -> Result<SourceText, StructuredError> {
    pkl_string_from_js(value).map(SourceText::from)
}

/// Converts a JS `String` to a UTF-8 Rust `String`, rejecting unpaired UTF-16 surrogates.
pub fn rust_string_from_js(value: &JsValue) -> Result<String, StructuredError> {
    if !value.is_string() {
        return Err(StructuredError::invalid_argument("expected string"));
    }
    rust_string_from_js_string(&JsString::from(value.clone()))
}

pub fn rust_string_from_js_string(value: &JsString) -> Result<String, StructuredError> {
    if supports_is_well_formed() {
        if !js_string_is_well_formed(value) {
            return Err(StructuredError::invalid_argument(
                "unpaired UTF-16 surrogate is unsupported in UTF-8 input",
            ));
        }
        if let Some(s) = value.as_string() {
            return Ok(s);
        }
    }
    let len = value.length();
    let mut units = Vec::with_capacity(len as usize);
    for i in 0..len {
        units.push(value.char_code_at(i) as u16);
    }
    String::from_utf16(&units).map_err(|_| {
        StructuredError::invalid_argument("unpaired UTF-16 surrogate is unsupported in UTF-8 input")
    })
}

/// Copies a JS `Uint8Array` into an owned `Vec<u8>`.
pub fn bytes_from_js(value: &JsValue) -> Result<Vec<u8>, StructuredError> {
    if !value.is_instance_of::<Uint8Array>() {
        return Err(StructuredError::invalid_argument("expected Uint8Array"));
    }
    let array = value
        .clone()
        .dyn_into::<Uint8Array>()
        .map_err(|_| StructuredError::invalid_argument("expected Uint8Array"))?;
    Ok(array.to_vec())
}

/// Allocates a fresh owned JS `Uint8Array` copy (never aliased into Wasm linear memory).
pub fn js_from_bytes(bytes: &[u8]) -> Uint8Array {
    Uint8Array::from(bytes)
}

pub fn set_prop(target: &Object, key: &str, value: JsValue) -> Result<(), JsValue> {
    Reflect::set(target, &JsValue::from_str(key), &value).map(|_| ())
}

pub fn get_prop(target: &JsValue, key: &str) -> Result<JsValue, StructuredError> {
    Reflect::get(target, &JsValue::from_str(key))
        .map_err(|_| StructuredError::invalid_argument(format!("failed to read property `{key}`")))
}

pub fn log_frame_to_js(frame: &LogFrame) -> Result<JsValue, JsValue> {
    let obj = Object::new();
    set_prop(
        &obj,
        "moduleUri",
        js_string_from_pkl(&frame.module_uri).into(),
    )?;
    set_prop(
        &obj,
        "memberName",
        frame
            .member_name
            .as_ref()
            .map_or(JsValue::NULL, |m| js_string_from_pkl(m).into()),
    )?;
    let lines = Array::new();
    for line in frame.source_lines.iter() {
        lines.push(js_string_from_pkl(line).as_ref());
    }
    set_prop(&obj, "sourceLines", lines.into())?;
    set_prop(
        &obj,
        "startLine",
        JsValue::from_f64(frame.start_line as f64),
    )?;
    set_prop(
        &obj,
        "startColumn",
        JsValue::from_f64(frame.start_column as f64),
    )?;
    set_prop(&obj, "endLine", JsValue::from_f64(frame.end_line as f64))?;
    set_prop(
        &obj,
        "endColumn",
        JsValue::from_f64(frame.end_column as f64),
    )?;
    Ok(obj.into())
}

pub fn log_frame_from_js(value: &JsValue) -> Result<LogFrame, StructuredError> {
    if !value.is_object() || value.is_null() {
        return Err(StructuredError::invalid_argument(
            "expected StackFrame object",
        ));
    }
    let module_uri = pkl_string_from_js(&get_prop(value, "moduleUri")?)?;
    let member_raw = get_prop(value, "memberName")?;
    let member_name = if member_raw.is_null() || member_raw.is_undefined() {
        None
    } else {
        Some(pkl_string_from_js(&member_raw)?)
    };
    let lines_raw = get_prop(value, "sourceLines")?;
    let source_lines: Vec<PklString> = if lines_raw.is_null() || lines_raw.is_undefined() {
        Vec::new()
    } else if Array::is_array(&lines_raw) {
        Array::from(&lines_raw)
            .iter()
            .map(|item| pkl_string_from_js(&item))
            .collect::<Result<Vec<_>, _>>()?
    } else {
        return Err(StructuredError::invalid_argument(
            "StackFrame.sourceLines must be an array of strings",
        ));
    };
    let pos = |key: &str| -> Result<usize, StructuredError> {
        let v = parse_u64_js(&get_prop(value, key)?)?;
        usize::try_from(v).map_err(|_| {
            StructuredError::invalid_argument(format!("StackFrame.{key} out of range"))
        })
    };
    Ok(LogFrame {
        module_uri,
        member_name,
        source_lines: Arc::from(source_lines),
        start_line: pos("startLine")?,
        start_column: pos("startColumn")?,
        end_line: pos("endLine")?,
        end_column: pos("endColumn")?,
    })
}
