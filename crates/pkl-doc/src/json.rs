//! Minimal pure-Rust JSON parser & serializer matching `kotlinx.serialization.json` 1.9.0.

use crate::model::DocGeneratorError;

#[derive(Clone, Debug, PartialEq)]
pub enum JsonValue {
    Null,
    Bool(bool),
    Number(String),
    String(String),
    Array(Vec<JsonValue>),
    Object(Vec<(String, JsonValue)>),
}

impl JsonValue {
    pub fn as_object(&self) -> Option<&[(String, JsonValue)]> {
        match self {
            Self::Object(entries) => Some(entries),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[JsonValue]> {
        match self {
            Self::Array(items) => Some(items),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn get(&self, key: &str) -> Option<&JsonValue> {
        let entries = self.as_object()?;
        entries.iter().rev().find(|(k, _)| k == key).map(|(_, v)| v)
    }
}

/// Escapes `s` as a JSON string literal (including surrounding quotes), matching
/// `kotlinx.serialization.json.internal.StringOps.kt::printQuoted`.
pub fn push_json_string(out: &mut String, s: &str) {
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{0008}' => out.push_str("\\b"),
            '\u{0009}' => out.push_str("\\t"),
            '\u{000A}' => out.push_str("\\n"),
            '\u{000C}' => out.push_str("\\f"),
            '\u{000D}' => out.push_str("\\r"),
            c if (c as u32) < 0x20 => {
                let code = c as u32;
                out.push_str("\\u00");
                out.push(hex_digit((code >> 4) & 0xF));
                out.push(hex_digit(code & 0xF));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

fn hex_digit(n: u32) -> char {
    match n {
        0..=9 => (b'0' + n as u8) as char,
        10..=15 => (b'a' + (n - 10) as u8) as char,
        _ => '0',
    }
}

/// Parses a JSON document into [`JsonValue`].
pub fn parse_json(input: &str) -> Result<JsonValue, DocGeneratorError> {
    let mut parser = Parser {
        bytes: input.as_bytes(),
        pos: 0,
    };
    parser.skip_ws();
    let val = parser.parse_value()?;
    parser.skip_ws();
    if parser.pos != parser.bytes.len() {
        return Err(DocGeneratorError::bug(format!(
            "Trailing JSON content at byte {}",
            parser.pos
        )));
    }
    Ok(val)
}

struct Parser<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn skip_ws(&mut self) {
        while let Some(&b) = self.bytes.get(self.pos) {
            if matches!(b, b' ' | b'\t' | b'\r' | b'\n') {
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn parse_value(&mut self) -> Result<JsonValue, DocGeneratorError> {
        self.skip_ws();
        match self.peek() {
            Some(b'{') => self.parse_object(),
            Some(b'[') => self.parse_array(),
            Some(b'"') => self.parse_string().map(JsonValue::String),
            Some(b't') => {
                self.expect_bytes(b"true")?;
                Ok(JsonValue::Bool(true))
            }
            Some(b'f') => {
                self.expect_bytes(b"false")?;
                Ok(JsonValue::Bool(false))
            }
            Some(b'n') => {
                self.expect_bytes(b"null")?;
                Ok(JsonValue::Null)
            }
            Some(b'-' | b'0'..=b'9') => self.parse_number(),
            other => Err(DocGeneratorError::bug(format!(
                "Unexpected JSON byte {:?} at {}",
                other, self.pos
            ))),
        }
    }

    fn expect_bytes(&mut self, expected: &[u8]) -> Result<(), DocGeneratorError> {
        if self.bytes.get(self.pos..self.pos + expected.len()) == Some(expected) {
            self.pos += expected.len();
            Ok(())
        } else {
            Err(DocGeneratorError::bug(format!(
                "Invalid JSON token at byte {}",
                self.pos
            )))
        }
    }

    fn parse_object(&mut self) -> Result<JsonValue, DocGeneratorError> {
        self.pos += 1; // '{'
        let mut entries = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b'}') {
            self.pos += 1;
            return Ok(JsonValue::Object(entries));
        }
        loop {
            self.skip_ws();
            if self.peek() != Some(b'"') {
                return Err(DocGeneratorError::bug(format!(
                    "Expected JSON object key at byte {}",
                    self.pos
                )));
            }
            let key = self.parse_string()?;
            self.skip_ws();
            if self.peek() != Some(b':') {
                return Err(DocGeneratorError::bug(format!(
                    "Expected ':' after JSON key at byte {}",
                    self.pos
                )));
            }
            self.pos += 1;
            let value = self.parse_value()?;
            entries.push((key, value));
            self.skip_ws();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b'}') => {
                    self.pos += 1;
                    return Ok(JsonValue::Object(entries));
                }
                _ => {
                    return Err(DocGeneratorError::bug(format!(
                        "Expected ',' or '}}' in JSON object at byte {}",
                        self.pos
                    )));
                }
            }
        }
    }

    fn parse_array(&mut self) -> Result<JsonValue, DocGeneratorError> {
        self.pos += 1; // '['
        let mut items = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b']') {
            self.pos += 1;
            return Ok(JsonValue::Array(items));
        }
        loop {
            let value = self.parse_value()?;
            items.push(value);
            self.skip_ws();
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b']') => {
                    self.pos += 1;
                    return Ok(JsonValue::Array(items));
                }
                _ => {
                    return Err(DocGeneratorError::bug(format!(
                        "Expected ',' or ']' in JSON array at byte {}",
                        self.pos
                    )));
                }
            }
        }
    }

    fn parse_number(&mut self) -> Result<JsonValue, DocGeneratorError> {
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        while let Some(b'0'..=b'9') = self.peek() {
            self.pos += 1;
        }
        if self.peek() == Some(b'.') {
            self.pos += 1;
            while let Some(b'0'..=b'9') = self.peek() {
                self.pos += 1;
            }
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.pos += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.pos += 1;
            }
            while let Some(b'0'..=b'9') = self.peek() {
                self.pos += 1;
            }
        }
        let s = std::str::from_utf8(&self.bytes[start..self.pos])
            .map_err(|e| DocGeneratorError::bug(e.to_string()))?;
        Ok(JsonValue::Number(s.to_string()))
    }

    fn parse_string(&mut self) -> Result<String, DocGeneratorError> {
        self.pos += 1; // '"'
        let mut out = String::new();
        while self.pos < self.bytes.len() {
            let b = self.bytes[self.pos];
            if b == b'"' {
                self.pos += 1;
                return Ok(out);
            }
            if b == b'\\' {
                self.pos += 1;
                let esc = *self
                    .bytes
                    .get(self.pos)
                    .ok_or_else(|| DocGeneratorError::bug("Unterminated JSON string escape"))?;
                self.pos += 1;
                match esc {
                    b'"' => out.push('"'),
                    b'\\' => out.push('\\'),
                    b'/' => out.push('/'),
                    b'b' => out.push('\u{0008}'),
                    b'f' => out.push('\u{000C}'),
                    b'n' => out.push('\n'),
                    b'r' => out.push('\r'),
                    b't' => out.push('\t'),
                    b'u' => {
                        let code = self.parse_hex4()?;
                        if (0xD800..=0xDBFF).contains(&code) {
                            if self.bytes.get(self.pos..self.pos + 2) != Some(b"\\u") {
                                return Err(DocGeneratorError::bug(
                                    "Invalid surrogate pair in JSON string",
                                ));
                            }
                            self.pos += 2;
                            let low = self.parse_hex4()?;
                            if !(0xDC00..=0xDFFF).contains(&low) {
                                return Err(DocGeneratorError::bug(
                                    "Invalid low surrogate in JSON string",
                                ));
                            }
                            let cp =
                                0x10000 + (((code as u32 - 0xD800) << 10) | (low as u32 - 0xDC00));
                            let ch = char::from_u32(cp).ok_or_else(|| {
                                DocGeneratorError::bug("Invalid Unicode scalar in JSON")
                            })?;
                            out.push(ch);
                        } else {
                            let ch = char::from_u32(code as u32).ok_or_else(|| {
                                DocGeneratorError::bug("Invalid Unicode scalar in JSON")
                            })?;
                            out.push(ch);
                        }
                    }
                    other => {
                        return Err(DocGeneratorError::bug(format!(
                            "Invalid JSON escape \\{}",
                            other as char
                        )));
                    }
                }
            } else {
                // Consume a valid UTF-8 chunk up to next '"' or '\\'.
                let start = self.pos;
                while self.pos < self.bytes.len()
                    && self.bytes[self.pos] != b'"'
                    && self.bytes[self.pos] != b'\\'
                {
                    self.pos += 1;
                }
                let slice = std::str::from_utf8(&self.bytes[start..self.pos])
                    .map_err(|e| DocGeneratorError::bug(e.to_string()))?;
                out.push_str(slice);
            }
        }
        Err(DocGeneratorError::bug("Unterminated JSON string"))
    }

    fn parse_hex4(&mut self) -> Result<u16, DocGeneratorError> {
        let slice = self
            .bytes
            .get(self.pos..self.pos + 4)
            .ok_or_else(|| DocGeneratorError::bug("Truncated \\uXXXX escape"))?;
        self.pos += 4;
        let mut val = 0u16;
        for &b in slice {
            let d = match b {
                b'0'..=b'9' => (b - b'0') as u16,
                b'a'..=b'f' => (b - b'a' + 10) as u16,
                b'A'..=b'F' => (b - b'A' + 10) as u16,
                _ => return Err(DocGeneratorError::bug("Invalid hex digit in \\uXXXX")),
            };
            val = (val << 4) | d;
        }
        Ok(val)
    }
}
