//! Error types, lossless UTF-16 helpers, and 64-bit integer boundary parsing
//! for the product Node.js N-API bindings.

use napi::{
    Env, JsString, Result,
    bindgen_prelude::{BigInt, JsObjectValue, Object, ToNapiValue, Utf16String},
};
use pkl::{PklError, PklErrorKind};
use pkl_runtime::value::PklString;

/// Standardized M6 error kinds matching the cross-target JS/TS API contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeErrorKind {
    Evaluation,
    Security,
    Timeout,
    Closed,
    Cancelled,
    Configuration,
    Decode,
    Io,
    InvalidArgument,
    CapabilityUnavailable,
}

impl NativeErrorKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Evaluation => "evaluation",
            Self::Security => "security",
            Self::Timeout => "timeout",
            Self::Closed => "closed",
            Self::Cancelled => "cancelled",
            Self::Configuration => "configuration",
            Self::Decode => "decode",
            Self::Io => "io",
            Self::InvalidArgument => "invalid-argument",
            Self::CapabilityUnavailable => "capability-unavailable",
        }
    }
}

/// Lossless native error carried across the N-API boundary to construct `PklError`.
#[derive(Clone, Debug)]
pub struct NativeError {
    pub kind: NativeErrorKind,
    pub message: PklString,
    pub capability: Option<String>,
}

impl NativeError {
    pub fn new(kind: NativeErrorKind, message: impl Into<PklString>) -> Self {
        Self {
            kind,
            message: message.into(),
            capability: None,
        }
    }

    pub fn evaluation(message: impl Into<PklString>) -> Self {
        Self::new(NativeErrorKind::Evaluation, message)
    }

    pub fn security(message: impl Into<PklString>) -> Self {
        Self::new(NativeErrorKind::Security, message)
    }

    pub fn timeout(message: impl Into<PklString>) -> Self {
        Self::new(NativeErrorKind::Timeout, message)
    }

    pub fn closed(message: impl Into<PklString>) -> Self {
        Self::new(NativeErrorKind::Closed, message)
    }

    pub fn cancelled(message: impl Into<PklString>) -> Self {
        Self::new(NativeErrorKind::Cancelled, message)
    }

    pub fn configuration(message: impl Into<PklString>) -> Self {
        Self::new(NativeErrorKind::Configuration, message)
    }

    pub fn decode(message: impl Into<PklString>) -> Self {
        Self::new(NativeErrorKind::Decode, message)
    }

    pub fn io(message: impl Into<PklString>) -> Self {
        Self::new(NativeErrorKind::Io, message)
    }

    pub fn invalid_argument(message: impl Into<PklString>) -> Self {
        Self::new(NativeErrorKind::InvalidArgument, message)
    }

    #[allow(dead_code)]
    pub fn capability_unavailable(
        capability: impl Into<String>,
        message: impl Into<PklString>,
    ) -> Self {
        Self {
            kind: NativeErrorKind::CapabilityUnavailable,
            message: message.into(),
            capability: Some(capability.into()),
        }
    }

    pub fn from_pkl(err: PklError) -> Self {
        let kind = match err.kind() {
            PklErrorKind::Evaluation => NativeErrorKind::Evaluation,
            PklErrorKind::Security => NativeErrorKind::Security,
            PklErrorKind::Timeout => NativeErrorKind::Timeout,
            PklErrorKind::Closed => NativeErrorKind::Closed,
            PklErrorKind::Cancelled => NativeErrorKind::Cancelled,
            PklErrorKind::Configuration => NativeErrorKind::Configuration,
            PklErrorKind::Decode => NativeErrorKind::Decode,
            PklErrorKind::Io => NativeErrorKind::Io,
            PklErrorKind::InvalidArgument => NativeErrorKind::InvalidArgument,
            _ => NativeErrorKind::Evaluation,
        };
        Self::new(kind, PklString::from(err.message()))
    }
}

/// Converts a `Result<V, NativeError>` into a JS outcome object `{ ok, value | kind, message, capability }`
/// preserving lossless UTF-16 error messages and structured error kinds.
pub fn to_js_outcome<'env, V: ToNapiValue>(
    env: &'env Env,
    result: std::result::Result<V, NativeError>,
) -> Result<Object<'env>> {
    let mut obj = Object::new(env)?;
    match result {
        Ok(val) => {
            obj.set("ok", true)?;
            obj.set("value", val)?;
        }
        Err(err) => {
            obj.set("ok", false)?;
            obj.set("kind", err.kind.as_str())?;
            obj.set("message", js_pkl_string(env, &err.message)?)?;
            if let Some(cap) = err.capability {
                obj.set("capability", cap)?;
            }
        }
    }
    Ok(obj)
}

/// Converts a `PklString` into a lossless JS UTF-16 string.
pub fn js_pkl_string<'env>(env: &'env Env, text: &PklString) -> Result<JsString<'env>> {
    let units: Vec<u16> = text.encode_utf16().collect();
    env.create_string_utf16(units)
}

/// Defines an own enumerable/writable/configurable data property on `obj` without
/// invoking inherited setters such as `Object.prototype.__proto__`.
pub fn define_own_prop<'env, V: napi::JsValue<'env>>(
    env: &'env Env,
    obj: &mut Object<'env>,
    key: &str,
    value: &V,
) -> Result<()> {
    let prop = napi::Property::new().with_name(env, key)?.with_value(value);
    obj.define_properties(&[prop])
}

/// Converts a `Utf16String` into a lossless `PklString`.
pub fn utf16_to_pkl(value: &Utf16String) -> PklString {
    PklString::from_utf16(value.to_vec())
}

/// Converts a `Utf16String` into a Rust `String`, rejecting unpaired UTF-16 surrogates.
pub fn utf16_to_string(value: &Utf16String) -> std::result::Result<String, NativeError> {
    String::from_utf16(value).map_err(|_| {
        NativeError::invalid_argument("Unpaired UTF-16 surrogate is unsupported in this field.")
    })
}

/// Losslessly parses a JS `BigInt` into signed `i64`, rejecting out-of-range values.
pub fn parse_i64_bigint(value: &BigInt) -> std::result::Result<i64, NativeError> {
    if value.words.len() > 1 {
        return Err(NativeError::invalid_argument(
            "BigInt is outside signed 64-bit integer range (-9223372036854775808..=9223372036854775807).",
        ));
    }
    let magnitude = value.words.first().copied().unwrap_or(0);
    if value.sign_bit && magnitude == (1u64 << 63) {
        return Ok(i64::MIN);
    }
    let signed = i64::try_from(magnitude).map_err(|_| {
        NativeError::invalid_argument(
            "BigInt is outside signed 64-bit integer range (-9223372036854775808..=9223372036854775807).",
        )
    })?;
    Ok(if value.sign_bit { -signed } else { signed })
}

/// Losslessly parses a JS `BigInt` into unsigned `u64`, rejecting negative or overflowing values.
pub fn parse_u64_bigint(value: &BigInt) -> std::result::Result<u64, NativeError> {
    let magnitude = value.words.first().copied().unwrap_or(0);
    if value.sign_bit && (magnitude != 0 || value.words.len() > 1) {
        return Err(NativeError::invalid_argument(
            "BigInt must be a non-negative unsigned 64-bit integer.",
        ));
    }
    if value.words.len() > 1 {
        return Err(NativeError::invalid_argument(
            "BigInt exceeds unsigned 64-bit integer range (0..=18446744073709551615).",
        ));
    }
    Ok(magnitude)
}
