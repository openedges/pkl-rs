//! UTF-16 Jsonnet syntax from the pinned RendererNodes, independent of traversal.
use crate::value::PklString;

pub(super) fn quoted(value: &PklString) -> PklString {
    let units: Vec<_> = value.encode_utf16().collect();
    let quote = if units.contains(&0x27) && !units.contains(&0x22) {
        0x22
    } else {
        0x27
    };
    let mut result = vec![quote];
    for unit in units {
        let escaped = match unit {
            0x5c => Some("\\\\"),
            0x08 => Some("\\b"),
            0x0c => Some("\\f"),
            0x0a => Some("\\n"),
            0x0d => Some("\\r"),
            0x27 if quote == 0x27 => Some("\\'"),
            0x22 if quote == 0x22 => Some("\\\""),
            _ => None,
        };
        if let Some(escaped) = escaped {
            result.extend(escaped.encode_utf16());
        } else if unit < 0x20 || (0x80..=0x9f).contains(&unit) {
            // ArrayCharEscaper uses IoUtils.toUnicodeEscape (lowercase hex).
            result.extend(format!("\\u{unit:04x}").encode_utf16());
        } else {
            result.push(unit);
        }
    }
    result.push(quote);
    PklString::from_utf16(result)
}

pub(super) fn string(value: &PklString, indent: &PklString, current: &PklString) -> PklString {
    let units: Vec<_> = value.encode_utf16().collect();
    // Java's regex is `\n*\S` without UNICODE_CHARACTER_CLASS: only the
    // six ASCII whitespace chars are excluded, not Rust's Unicode whitespace.
    let safe = units
        .iter()
        .find(|unit| **unit != 10)
        .is_some_and(|unit| !matches!(*unit, 0x09..=0x0d | 0x20));
    if indent.is_empty() || !units.contains(&10) || !safe {
        return quoted(value);
    }
    let mut result = "|||\n".encode_utf16().collect::<Vec<_>>();
    let mut offset = 0;
    // String.lines(): LF, CR and CRLF terminate a line. There is no final
    // empty line after a trailing terminator, while earlier empty lines remain.
    while offset < units.len() {
        let start = offset;
        while offset < units.len() && units[offset] != 10 && units[offset] != 13 {
            offset += 1;
        }
        result.extend(current.encode_utf16());
        result.extend(indent.encode_utf16());
        result.extend_from_slice(&units[start..offset]);
        result.push(10);
        if offset < units.len() {
            let cr = units[offset] == 13;
            offset += 1;
            if cr && offset < units.len() && units[offset] == 10 {
                offset += 1;
            }
        }
    }
    result.extend(current.encode_utf16());
    result.extend("|||".encode_utf16());
    PklString::from_utf16(result)
}

pub(super) fn field_name(value: &PklString, indent: &PklString, current: &PklString) -> PklString {
    let regular = value.as_str().is_some_and(|text| {
        let mut bytes = text.bytes();
        let initial = bytes
            .next()
            .is_some_and(|b| b == b'_' || b.is_ascii_alphabetic());
        initial
            && bytes.all(|b| b == b'_' || b.is_ascii_alphanumeric())
            && !matches!(
                text,
                "assert"
                    | "else"
                    | "error"
                    | "false"
                    | "for"
                    | "function"
                    | "if"
                    | "import"
                    | "importstr"
                    | "in"
                    | "local"
                    | "null"
                    | "self"
                    | "super"
                    | "tailstrict"
                    | "then"
                    | "true"
            )
    });
    if regular {
        value.clone()
    } else {
        string(value, indent, current)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quote_and_text_block_keep_java_char_and_line_boundaries() {
        assert_eq!(quoted(&"a'b".into()), "\"a'b\"");
        assert_eq!(quoted(&"a'\"b\t\u{80}".into()), "'a\\'\"b\\u0009\\u0080'");
        let lone = PklString::from_utf16(vec![0xd800, 0x2028, 0xdc00]);
        assert_eq!(
            quoted(&lone).encode_utf16().collect::<Vec<_>>(),
            [0x27, 0xd800, 0x2028, 0xdc00, 0x27]
        );
        assert_eq!(
            string(&"\na\r\nb\n\n".into(), &"  ".into(), &"  ".into()),
            "|||\n    \n    a\n    b\n    \n  |||"
        );
        assert_eq!(string(&" \na".into(), &"  ".into(), &"".into()), "' \\na'");
        assert_eq!(string(&"\n\n".into(), &"  ".into(), &"".into()), "'\\n\\n'");
        assert_eq!(
            field_name(&"local".into(), &"".into(), &"".into()),
            "'local'"
        );
        assert_eq!(
            field_name(&"importbin".into(), &"".into(), &"".into()),
            "importbin"
        );
    }
}
