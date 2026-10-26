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
pub(super) fn escape(value: &PklString) -> PklString {
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
