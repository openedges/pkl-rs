//! Pkl ParserNodes constructors, distinct from schema tag resolution.
use super::*;
use crate::vm::primitives::string_parse;
fn foreign(class: &str, message: PklString) -> RuntimeError {
    parse_error(PklString::from(class).concat(&": ".into()).concat(&message))
}
fn int_error(units: &[u16], offset: usize, index: usize) -> RuntimeError {
    let message = PklString::from(format!("Error at index {} in: \"", index - offset))
        .concat(&PklString::from_utf16(units[offset..].to_vec()))
        .concat(&"\"".into());
    foreign("java.lang.NumberFormatException", message)
}
fn digit(u: u16, radix: i64) -> Option<i64> {
    let n = match u {
        0x30..=0x39 => i64::from(u - 0x30),
        0x41..=0x5a => i64::from(u - 0x41 + 10),
        0x61..=0x7a => i64::from(u - 0x61 + 10),
        0xff21..=0xff3a => i64::from(u - 0xff21 + 10),
        0xff41..=0xff5a => i64::from(u - 0xff41 + 10),
        _ => string_parse::decimal_digit(u)?,
    };
    (n < radix).then_some(n)
}
fn integer(value: &Text, mode: Mode) -> Result<Value, RuntimeError> {
    let units = value
        .units()
        .iter()
        .copied()
        .filter(|u| *u != 0x5f)
        .collect::<Vec<_>>();
    let Some(first) = units.first() else {
        return Err(unsupported("explicit empty integer constructor exception"));
    };
    let negative = *first == 0x2d;
    let mut offset = usize::from(negative || *first == 0x2b);
    if units.contains(&0x3a) {
        let mut value = 0i64;
        let mut pieces = units[offset..].split(|u| *u == 0x3a).collect::<Vec<_>>();
        while pieces.last().is_some_and(|p| p.is_empty()) {
            pieces.pop();
        }
        for part in pieces {
            let segment =
                string_parse::integer(&PklString::from_utf16(part.to_vec())).ok_or_else(|| {
                    unsupported("Long.parseLong(String) base60 invalid-segment message")
                })?;
            value = value
                .checked_mul(60)
                .and_then(|n| n.checked_add(segment))
                .ok_or_else(|| {
                    // VmSafeMath maps either checked multiply/add failure to
                    // VmExceptionBuilder.evalError("integerOverflow"). The
                    // constructor catches that RuntimeException and preserves
                    // its raw class/message in YamlEngineException's hint.
                    foreign(
                        "org.pkl.core.runtime.VmEvalException",
                        "integerOverflow".into(),
                    )
                })?;
        }
        return Ok(Value::Int(
            (if negative {
                value.wrapping_neg()
            } else {
                value
            })
            .into(),
        ));
    }
    if offset >= units.len() {
        return Err(unsupported(
            "explicit sign-only integer constructor exception",
        ));
    }
    let mut radix = 10i64;
    if units[offset] == 0x30 && units.len() > offset + 1 {
        match units[offset + 1] {
            0x62 => {
                radix = 2;
                offset += 2;
            }
            0x6f => {
                radix = 8;
                offset += 2;
            }
            0x78 => {
                radix = 16;
                offset += 2;
            }
            _ if mode != Mode::Yaml12 => {
                radix = 8;
                offset += 1;
            }
            _ => (),
        }
    }
    if offset == units.len() {
        return Err(foreign("java.lang.NumberFormatException", "".into()));
    }
    // The substring parser itself accepts a sign (including after a radix
    // prefix); its negative accumulator admits MIN only for that inner sign.
    let parse_offset = offset;
    let inner_negative = units[offset] == 0x2d;
    if inner_negative || units[offset] == 0x2b {
        offset += 1;
        if offset == units.len() {
            return Err(int_error(&units, parse_offset, parse_offset));
        }
    }
    let limit = if inner_negative { i64::MIN } else { -i64::MAX };
    let mut result = 0i64;
    for (index, &unit) in units.iter().enumerate().skip(offset) {
        let Some(n) = digit(unit, radix) else {
            return Err(int_error(&units, parse_offset, index));
        };
        result = result
            .checked_mul(radix)
            .and_then(|v| v.checked_sub(n))
            .filter(|v| *v >= limit)
            .ok_or_else(|| int_error(&units, parse_offset, index))?;
    }
    let result = if inner_negative { result } else { -result };
    Ok(Value::Int(
        (if negative {
            result.wrapping_neg()
        } else {
            result
        })
        .into(),
    ))
}
fn floating(value: &Text) -> Result<Value, RuntimeError> {
    let cleaned = PklString::from_utf16(
        value
            .units()
            .iter()
            .copied()
            .filter(|u| *u != 0x5f)
            .collect::<Vec<_>>(),
    );
    let special = match cleaned.as_str() {
        Some(".") => Some(0.0),
        Some(".nan" | ".NaN" | ".NAN") => Some(f64::NAN),
        Some(".inf" | ".Inf" | ".INF" | "+.inf" | "+.Inf" | "+.INF") => Some(f64::INFINITY),
        Some("-.inf" | "-.Inf" | "-.INF") => Some(f64::NEG_INFINITY),
        _ => None,
    };
    if let Some(v) = special {
        return Ok(Value::Float(v.into()));
    }
    let units = cleaned.encode_utf16().collect::<Vec<_>>();
    if units.contains(&0x3a) {
        let negative = units.first() == Some(&0x2d);
        let offset = usize::from(negative || units.first() == Some(&0x2b));
        let mut result = 0.0;
        let mut parts = units[offset..].split(|u| *u == 0x3a).collect::<Vec<_>>();
        while parts.last().is_some_and(|p| p.is_empty()) {
            parts.pop();
        }
        for part in parts {
            let segment = string_parse::float(&PklString::from_utf16(part.to_vec()))
                .ok_or_else(|| unsupported("Double.parseDouble base60 invalid-segment message"))?;
            result = result * 60.0 + segment;
        }
        return Ok(Value::Float(
            (if negative { -result } else { result }).into(),
        ));
    }
    string_parse::float(&cleaned)
        .map(|v| Value::Float(v.into()))
        .ok_or_else(|| {
            // FloatingDecimal trims UTF-16 code units <= U+0020 before
            // choosing its empty-input or invalid-input diagnostic.
            let start = units.iter().position(|&u| u > 0x20).unwrap_or(units.len());
            let end = units
                .iter()
                .rposition(|&u| u > 0x20)
                .map_or(start, |i| i + 1);
            let message = if start == end {
                "empty String".into()
            } else {
                PklString::from("For input string: \"")
                    .concat(&PklString::from_utf16(units[start..end].to_vec()))
                    .concat(&"\"".into())
            };
            foreign("java.lang.NumberFormatException", message)
        })
}
pub(super) fn construct(tag: &Text, value: &Text, mode: Mode) -> Result<Value, RuntimeError> {
    if tag.is_ascii("tag:yaml.org,2002:null") {
        return Ok(Value::Null);
    }
    if tag.is_ascii("tag:yaml.org,2002:bool") {
        return Ok(Value::Bool(
            [
                "true", "True", "TRUE", "on", "On", "ON", "y", "Y", "yes", "Yes", "YES",
            ]
            .iter()
            .any(|s| value.is_ascii(s)),
        ));
    }
    if tag.is_ascii("tag:yaml.org,2002:str") || tag.is_ascii("tag:yaml.org,2002:timestamp") {
        return Ok(Value::String(text(value)));
    }
    if tag.is_ascii("tag:yaml.org,2002:int") {
        return integer(value, mode);
    }
    if tag.is_ascii("tag:yaml.org,2002:float") {
        return floating(value);
    }
    if tag.is_ascii("tag:yaml.org,2002:binary") {
        let encoded = PklString::from_utf16(
            value
                .units()
                .iter()
                .copied()
                .filter(|u| !matches!(u, 0x09..=0x0d | 0x20))
                .collect::<Vec<_>>(),
        );
        return resources::decode_base64(&encoded)
            .map(|bytes| Value::Bytes(bytes.into()))
            .map_err(|m| foreign("java.lang.IllegalArgumentException", m.into()));
    }
    Err(unsupported(
        "explicit scalar tag/node-kind constructor mismatch",
    ))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn constructors_preserve_schema_and_raw_scalar_distinctions() {
        assert_eq!(
            integer(&Text::from("012"), Mode::Compat).unwrap(),
            Value::Int(10.into())
        );
        assert_eq!(
            integer(&Text::from("012"), Mode::Yaml12).unwrap(),
            Value::Int(12.into())
        );
        assert_eq!(
            construct(
                &Text::from("tag:yaml.org,2002:bool"),
                &Text::from("unexpected"),
                Mode::Yaml12
            )
            .unwrap(),
            Value::Bool(false)
        );
        assert_eq!(
            construct(
                &Text::from("tag:yaml.org,2002:str"),
                &Text::from_units(vec![0xd800]),
                Mode::Yaml12
            )
            .unwrap(),
            Value::String(PklString::from_utf16(vec![0xd800]))
        );
        let error = integer(&Text::from("9223372036854775808"), Mode::Compat).unwrap_err();
        assert_eq!(
            error.hint,
            Some(
                "java.lang.NumberFormatException: Error at index 18 in: \"9223372036854775808\""
                    .into()
            )
        );
    }
}
