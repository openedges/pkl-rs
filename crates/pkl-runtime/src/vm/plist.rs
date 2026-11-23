//! Pinned Pkl 0.32 PListRenderer lexical output. Forcing, converters, member
//! paths and Host continuations remain owned by the shared renderer.
use super::*;

pub(super) const DOCUMENT_HEADER: &str = concat!(
    "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
    "<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" ",
    "\"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n",
    "<plist version=\"1.0\">\n",
);

/// ArrayCharEscaper escapes exactly these five Java chars. Invalid XML control
/// characters and unpaired surrogates are retained as UTF-16 units by Java.
pub(super) fn write_escaped_str(out: &mut String, s: &str) {
    let bytes = s.as_bytes();
    let mut start = 0;
    for (i, &b) in bytes.iter().enumerate() {
        let repl = match b {
            b'"' => "&quot;",
            b'\'' => "&apos;",
            b'<' => "&lt;",
            b'>' => "&gt;",
            b'&' => "&amp;",
            _ => continue,
        };
        out.push_str(&s[start..i]);
        out.push_str(repl);
        start = i + 1;
    }
    out.push_str(&s[start..]);
}

pub(super) fn escape(value: &PklString) -> PklString {
    if let Some(s) = value.as_str() {
        if !s
            .as_bytes()
            .iter()
            .any(|&b| matches!(b, b'"' | b'\'' | b'<' | b'>' | b'&'))
        {
            return value.clone();
        }
        let mut out = String::with_capacity(s.len() + 8);
        write_escaped_str(&mut out, s);
        return PklString::from(out);
    }
    let mut units = Vec::new();
    for unit in value.encode_utf16() {
        let text = match unit {
            0x22 => Some("&quot;"),
            0x27 => Some("&apos;"),
            0x3c => Some("&lt;"),
            0x3e => Some("&gt;"),
            0x26 => Some("&amp;"),
            _ => None,
        };
        if let Some(text) = text {
            units.extend(text.encode_utf16());
        } else {
            units.push(unit);
        }
    }
    PklString::from_utf16(units)
}

/// Fast UTF-8 direct PList scalar writer. Returns `false` on non-scalars or unpaired surrogates.
pub(super) fn write_scalar(out: &mut String, value: &Value) -> bool {
    match value {
        Value::String(text) => {
            let Some(s) = text.as_str() else {
                return false;
            };
            out.push_str("<string>");
            write_escaped_str(out, s);
            out.push_str("</string>");
        }
        Value::Int(value) => {
            let mut buf = [0u8; 24];
            out.push_str("<integer>");
            out.push_str(super::format_i64_buf(value.get(), &mut buf));
            out.push_str("</integer>");
        }
        Value::Float(value) => {
            out.push_str("<real>");
            if value.is_nan() {
                out.push_str("nan");
            } else if value.is_infinite() {
                if value.is_sign_negative() {
                    out.push_str("-infinity");
                } else {
                    out.push_str("+infinity");
                }
            } else {
                out.push_str(&crate::numeric::double_to_string(**value));
            }
            out.push_str("</real>");
        }
        Value::Bool(value) => out.push_str(if *value { "<true/>" } else { "<false/>" }),
        Value::Bytes(value) => {
            out.push_str("<data>");
            let b64 = super::resources::base64(value);
            if let Some(s) = b64.as_str() {
                out.push_str(s);
            } else {
                return false;
            }
            out.push_str("</data>");
        }
        _ => return false,
    }
    true
}

pub(super) fn scalar(value: &Value) -> Option<PklString> {
    let mut out = crate::pkl_string::PklStringBuilder::default();
    match value {
        Value::String(text) => {
            out.push_str("<string>");
            out.push_value(&escape(text));
            out.push_str("</string>");
        }
        Value::Int(value) => out.push_str(&format!("<integer>{value}</integer>")),
        Value::Float(value) => {
            out.push_str("<real>");
            out.push_str(&if value.is_nan() {
                "nan".into()
            } else if value.is_infinite() {
                if value.is_sign_negative() {
                    "-infinity"
                } else {
                    "+infinity"
                }
                .into()
            } else {
                crate::numeric::double_to_string(**value)
            });
            out.push_str("</real>");
        }
        Value::Bool(value) => out.push_str(if *value { "<true/>" } else { "<false/>" }),
        Value::Bytes(value) => {
            out.push_str("<data>");
            out.push_value(&super::resources::base64(value));
            out.push_str("</data>");
        }
        _ => return None,
    }
    Some(out.finish())
}
