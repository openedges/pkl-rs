// Copyright © 2024-2025 Apple Inc. and the Pkl project authors. Apache-2.0.
//! Exact ASCII languages of Pkl's three custom scalar resolver regexes.
//! This chooses tags only. Numeric conversion belongs to Pkl constructors.
use super::nodes::tag;
use super::{Mode, Text};
pub fn resolve(value: &Text, implicit: bool, mode: Mode) -> Text {
    if !implicit {
        return tag("str");
    }
    let Some(s) = ascii(value) else {
        return tag("str");
    };
    if matches!(s.as_str(), "" | "~" | "null" | "Null" | "NULL") {
        return tag("null");
    }
    let boolean = matches!(
        s.as_str(),
        "true" | "True" | "TRUE" | "false" | "False" | "FALSE"
    ) || mode != Mode::Yaml12
        && matches!(
            s.as_str(),
            "y" | "Y"
                | "yes"
                | "Yes"
                | "YES"
                | "n"
                | "N"
                | "no"
                | "No"
                | "NO"
                | "on"
                | "On"
                | "ON"
                | "off"
                | "Off"
                | "OFF"
        );
    if boolean {
        return tag("bool");
    }
    if integer(s.as_bytes(), mode) {
        return tag("int");
    }
    if float(s.as_bytes(), mode) {
        return tag("float");
    }
    tag("str")
}
fn ascii(t: &Text) -> Option<String> {
    let mut s = String::new();
    for &u in t.units() {
        if u > 127 {
            return None;
        }
        s.push(u as u8 as char);
    }
    Some(s)
}
fn sign(s: &[u8]) -> &[u8] {
    if s.first().is_some_and(|x| matches!(x, b'+' | b'-')) {
        &s[1..]
    } else {
        s
    }
}
fn digits(s: &[u8], underscores: bool, radix: u8) -> bool {
    !s.is_empty()
        && s.iter().all(|&c| {
            c == b'_' && underscores
                || match radix {
                    2 => matches!(c, b'0' | b'1'),
                    8 => matches!(c, b'0'..=b'7'),
                    16 => c.is_ascii_hexdigit(),
                    _ => c.is_ascii_digit(),
                }
        })
}
fn decimal_head(s: &[u8]) -> bool {
    s.first().is_some_and(u8::is_ascii_digit)
        && s[1..].iter().all(|c| c.is_ascii_digit() || *c == b'_')
}
fn sexagesimal(s: &[u8], nonzero: bool) -> bool {
    let parts = s.split(|c| *c == b':').collect::<Vec<_>>();
    if parts.len() < 2 || !decimal_head(parts[0]) || (nonzero && parts[0][0] == b'0') {
        return false;
    }
    parts[1..].iter().all(|s| match s.len() {
        1 => s[0].is_ascii_digit(),
        2 => (b'0'..=b'5').contains(&s[0]) && s[1].is_ascii_digit(),
        _ => false,
    })
}
fn integer(s: &[u8], mode: Mode) -> bool {
    let v = sign(s);
    if v.is_empty() {
        return false;
    }
    if mode == Mode::Yaml12 {
        return digits(v, false, 10)
            || s.strip_prefix(b"0o").is_some_and(|v| digits(v, false, 8))
            || s.strip_prefix(b"0x").is_some_and(|v| digits(v, false, 16));
    }
    if v.strip_prefix(b"0b").is_some_and(|x| digits(x, true, 2))
        || v.strip_prefix(b"0x").is_some_and(|x| digits(x, true, 16))
    {
        return true;
    }
    if let Some(x) = v.strip_prefix(b"0") {
        let x = if mode == Mode::Compat {
            x.strip_prefix(b"o").unwrap_or(x)
        } else {
            x
        };
        if digits(x, true, 8) {
            return true;
        }
    }
    if v == b"0"
        || (matches!(v[0], b'1'..=b'9') && v[1..].iter().all(|c| c.is_ascii_digit() || *c == b'_'))
    {
        return true;
    }
    mode == Mode::Yaml11 && sexagesimal(v, true)
}
fn float(s: &[u8], mode: Mode) -> bool {
    if matches!(s, b".nan" | b".NaN" | b".NAN") {
        return true;
    }
    let v = sign(s);
    if matches!(v, b".inf" | b".Inf" | b".INF") {
        return true;
    }
    if v.is_empty() {
        return false;
    }
    if mode == Mode::Yaml11 && v.contains(&b':') {
        let Some(dot) = v.iter().position(|c| *c == b'.') else {
            return false;
        };
        return sexagesimal(&v[..dot], false)
            && v[dot + 1..]
                .iter()
                .all(|c| c.is_ascii_digit() || *c == b'_');
    }
    let exponent = v.iter().position(|c| matches!(*c, b'e' | b'E'));
    let base = if let Some(i) = exponent {
        let e = &v[i + 1..];
        if mode == Mode::Yaml11 && !e.first().is_some_and(|c| matches!(c, b'+' | b'-')) {
            return false;
        }
        if !digits(sign(e), false, 10) {
            return false;
        }
        &v[..i]
    } else {
        v
    };
    let underscore = mode != Mode::Yaml12;
    let tail_ok = |s: &[u8]| {
        s.iter()
            .all(|c| c.is_ascii_digit() || (underscore && *c == b'_'))
    };
    if let Some(dot) = base.iter().position(|c| *c == b'.') {
        let left = &base[..dot];
        let right = &base[dot + 1..];
        if !tail_ok(right) {
            return false;
        }
        if mode == Mode::Yaml11 {
            return left.is_empty() || decimal_head(left);
        }
        if left.is_empty() {
            return !right.is_empty();
        }
        left.first().is_some_and(u8::is_ascii_digit) && tail_ok(left)
    } else {
        mode != Mode::Yaml11 && base.first().is_some_and(u8::is_ascii_digit) && tail_ok(base)
    }
}
