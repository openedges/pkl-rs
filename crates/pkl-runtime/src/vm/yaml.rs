//! Pinned YAML emitter semantics, independent of evaluator traversal and Host I/O.
//! The output buffer counts Java UTF-16 units, including unmatched surrogates.
use crate::value::PklString;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Mode {
    Compat,
    Yaml11,
    Yaml12,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Copy)]
pub(super) struct Checkpoint {
    len: usize,
    indent: usize,
    containers_len: usize,
    utf16: bool,
}

pub(super) struct Emitter {
    text_buf: String,
    units: Option<Vec<u16>>,
    width: usize,
    indent: usize,
    containers: Vec<bool>,
    mode: Mode,
}
impl Emitter {
    pub(super) fn new(mode: Mode, width: usize) -> Self {
        Self {
            text_buf: String::with_capacity(1024),
            units: None,
            width,
            indent: 0,
            containers: Vec::with_capacity(8),
            mode,
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    #[inline]
    pub(super) fn is_utf8(&self) -> bool {
        self.units.is_none()
    }
    #[cfg(not(target_arch = "wasm32"))]
    #[inline]
    pub(super) fn checkpoint(&self) -> Checkpoint {
        Checkpoint {
            len: match &self.units {
                Some(u) => u.len(),
                None => self.text_buf.len(),
            },
            indent: self.indent,
            containers_len: self.containers.len(),
            utf16: self.units.is_some(),
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    #[inline]
    pub(super) fn rollback(&mut self, cp: Checkpoint) {
        if cp.utf16 {
            if let Some(u) = &mut self.units {
                u.truncate(cp.len);
            }
        } else {
            debug_assert!(self.units.is_none());
            self.text_buf.truncate(cp.len);
        }
        self.indent = cp.indent;
        self.containers.truncate(cp.containers_len);
    }
    fn promote_utf16(&mut self) -> &mut Vec<u16> {
        let text_buf = &mut self.text_buf;
        self.units.get_or_insert_with(|| {
            let units = text_buf.encode_utf16().collect();
            text_buf.clear();
            units
        })
    }
    pub(super) fn is_empty(&self) -> bool {
        match &self.units {
            Some(u) => u.is_empty(),
            None => self.text_buf.is_empty(),
        }
    }
    pub(super) fn raw(&mut self, text: &PklString) {
        if let Some(s) = text.as_str() {
            self.text(s);
        } else {
            self.promote_utf16().extend(text.encode_utf16());
        }
    }
    #[inline]
    pub(super) fn text(&mut self, text: &str) {
        match &mut self.units {
            Some(u) => u.extend(text.encode_utf16()),
            None => self.text_buf.push_str(text),
        }
    }
    pub(super) fn finish(self) -> PklString {
        match self.units {
            Some(u) => PklString::from_utf16(u),
            None => PklString::from(self.text_buf),
        }
    }
    #[inline]
    fn spaces(&mut self, mut length: usize) {
        match &mut self.units {
            Some(u) => u.resize(u.len() + length, 32),
            None => {
                const SPACES: &str = "                                ";
                while length > 0 {
                    let chunk = length.min(SPACES.len());
                    self.text_buf.push_str(&SPACES[..chunk]);
                    length -= chunk;
                }
            }
        }
    }
    #[inline]
    pub(super) fn newline(&mut self) {
        if self.is_empty() {
            return;
        }
        match &mut self.units {
            Some(u) => {
                if u.last() != Some(&10) {
                    u.push(10);
                }
            }
            None => {
                if !self.text_buf.ends_with('\n') {
                    self.text_buf.push('\n');
                }
            }
        }
        self.spaces(self.indent);
    }
    fn undo_newline(&mut self) {
        if !self.is_empty() {
            match &mut self.units {
                Some(u) => u.truncate(u.len() - self.indent - 1),
                None => self
                    .text_buf
                    .truncate(self.text_buf.len() - self.indent - 1),
            }
        }
    }
    pub(super) fn start(&mut self, sequence: bool) {
        let enclosing_sequence = self.containers.last() == Some(&true);
        if enclosing_sequence || (!sequence && !self.containers.is_empty()) {
            self.indent += self.width;
        }
        if enclosing_sequence {
            self.spaces(self.width - 1);
        } else {
            self.newline();
        }
        self.containers.push(sequence);
    }
    pub(super) fn close(&mut self, empty: bool) {
        let sequence = self.containers.pop().expect("YAML container");
        let enclosing_sequence = self.containers.last() == Some(&true);
        if empty {
            if !enclosing_sequence {
                self.undo_newline();
                self.text(" ");
            }
            self.text(if sequence { "[]" } else { "{}" });
        }
        if enclosing_sequence || (!sequence && !self.containers.is_empty()) {
            self.indent -= self.width;
        }
    }
    pub(super) fn member(&mut self, element: bool, first: bool) {
        if !first {
            self.newline();
        }
        if element {
            self.text("-");
        }
    }
    pub(super) fn separator(&mut self) {
        self.newline();
        self.text("---");
    }
    pub(super) fn scalar(&mut self, value: &PklString) {
        if !self.is_empty() {
            self.text(" ");
        }
        self.raw(value);
    }
    #[cfg(not(target_arch = "wasm32"))]
    #[inline]
    pub(super) fn scalar_str(&mut self, value: &str) {
        if !self.is_empty() {
            self.text(" ");
        }
        self.text(value);
    }
    pub(super) fn string_value(&mut self, value: &PklString) {
        if !self.is_empty() {
            self.text(" ");
        }
        self.string(value, false);
    }
    pub(super) fn bytes(&mut self, value: &[u8]) {
        if !self.is_empty() {
            self.text(" ");
        }
        self.text("!!binary ");
        self.string(&super::resources::base64(value), false);
    }
    pub(super) fn string(&mut self, value: &PklString, key: bool) {
        if self.units.is_none()
            && let Some(s) = value.as_str()
        {
            self.string_utf8(s, key);
            return;
        }
        let u: Vec<u16> = value.encode_utf16().collect();
        if reserved(value.as_str(), self.mode) {
            self.single_quoted_u16(&u);
            return;
        }
        let first = u[0];
        let mut escape = false;
        let mut quote = false;
        let mut non_number = false;
        let mut newline = None;
        let mut colon = None;
        match first {
            10 => newline = Some(0),
            39 | 33 | 37 | 38 | 42 | 123 | 125 | 91 | 93 | 44 | 35 | 124 | 62 | 64 | 96 | 34
            | 32 => quote = true,
            45 | 58 | 63 => quote = u.len() == 1 || u[1] == 32,
            48..=57 | 43 | 46 | 111 => (),
            _ => {
                escape = first < 32;
                non_number = true;
            }
        }
        for (i, &ch) in u.iter().enumerate().skip(1) {
            match ch {
                10 => {
                    if newline.is_none() {
                        newline = Some(i);
                    }
                }
                39 => non_number = true,
                32 => {
                    quote |= i == u.len() - 1;
                    non_number = true;
                }
                91 | 93 | 123 | 125 | 44 => {
                    quote |= key;
                    non_number = true;
                }
                35 => {
                    quote |= u[i - 1] == 32;
                    non_number = true;
                }
                58 => {
                    if colon.is_none() {
                        colon = Some(i);
                    }
                    quote |= i == u.len() - 1 || u[i + 1] == 32;
                }
                48..=57 | 65..=70 | 97..=102 | 43 | 45 | 95 | 46 | 111 | 120 => (),
                _ => {
                    escape |= ch < 32;
                    non_number = true;
                }
            }
        }
        let start = self.promote_utf16().len();
        if escape {
            self.double_quoted_u16(&u);
        } else if let Some(newline) = newline {
            self.multiline_u16(&u, newline);
        } else if quote || (!non_number && number(&u, colon, self.mode)) {
            self.single_quoted_u16(&u);
        } else {
            self.promote_utf16().extend_from_slice(&u);
        }
        let added = self.promote_utf16().len() - start;
        if key && (added > 1024 || (!escape && newline.is_some())) {
            let units = self.promote_utf16();
            units.splice(start..start, [63, 32]);
            units.push(10);
            self.spaces(self.indent);
        }
    }
    fn string_utf8(&mut self, s: &str, key: bool) {
        if reserved(Some(s), self.mode) {
            self.single_quoted_str(s);
            return;
        }
        let bytes = s.as_bytes();
        let first = bytes[0];
        let mut escape = false;
        let mut quote = false;
        let mut non_number = false;
        let mut newline = None;
        let mut colon = None;
        match first {
            10 => newline = Some(0),
            39 | 33 | 37 | 38 | 42 | 123 | 125 | 91 | 93 | 44 | 35 | 124 | 62 | 64 | 96 | 34
            | 32 => quote = true,
            45 | 58 | 63 => quote = bytes.len() == 1 || bytes[1] == 32,
            48..=57 | 43 | 46 | 111 => (),
            _ => {
                escape = first < 32;
                non_number = true;
            }
        }
        for (i, &ch) in bytes.iter().enumerate().skip(1) {
            match ch {
                10 => {
                    if newline.is_none() {
                        newline = Some(i);
                    }
                }
                39 => non_number = true,
                32 => {
                    quote |= i == bytes.len() - 1;
                    non_number = true;
                }
                91 | 93 | 123 | 125 | 44 => {
                    quote |= key;
                    non_number = true;
                }
                35 => {
                    quote |= bytes[i - 1] == 32;
                    non_number = true;
                }
                58 => {
                    if colon.is_none() {
                        colon = Some(i);
                    }
                    quote |= i == bytes.len() - 1 || bytes[i + 1] == 32;
                }
                48..=57 | 65..=70 | 97..=102 | 43 | 45 | 95 | 46 | 111 | 120 => (),
                _ => {
                    escape |= ch < 32;
                    non_number = true;
                }
            }
        }
        let start = self.text_buf.len();
        if escape {
            self.double_quoted_str(s);
        } else if let Some(newline) = newline {
            self.multiline_str(s, newline);
        } else if quote || (!non_number && number_ascii(bytes, colon, self.mode)) {
            self.single_quoted_str(s);
        } else {
            self.text_buf.push_str(s);
        }
        if key {
            let long_key = self.text_buf.len() - start > 1024
                && self.text_buf[start..].encode_utf16().count() > 1024;
            if long_key || (!escape && newline.is_some()) {
                self.text_buf.insert_str(start, "? ");
                self.text_buf.push('\n');
                self.spaces(self.indent);
            }
        }
    }
    fn single_quoted_str(&mut self, s: &str) {
        self.text_buf.push('\'');
        if !s.as_bytes().contains(&b'\'') {
            self.text_buf.push_str(s);
        } else {
            let mut start = 0;
            for (i, &b) in s.as_bytes().iter().enumerate() {
                if b == b'\'' {
                    self.text_buf.push_str(&s[start..=i]);
                    self.text_buf.push('\'');
                    start = i + 1;
                }
            }
            self.text_buf.push_str(&s[start..]);
        }
        self.text_buf.push('\'');
    }
    fn double_quoted_str(&mut self, s: &str) {
        self.text_buf.push('"');
        for ch in s.chars() {
            let escaped = match ch {
                '\0' => Some("\\0"),
                '\x07' => Some("\\a"),
                '\x08' => Some("\\b"),
                '\t' => Some("\\t"),
                '\n' => Some("\\n"),
                '\x0b' => Some("\\v"),
                '\x0c' => Some("\\f"),
                '\r' => Some("\\r"),
                '\x1b' => Some("\\e"),
                '"' => Some("\\\""),
                '\\' => Some("\\\\"),
                '\u{85}' => Some("\\N"),
                '\u{a0}' => Some("\\_"),
                '\u{2028}' => Some("\\L"),
                '\u{2029}' => Some("\\P"),
                _ => None,
            };
            if let Some(escaped) = escaped {
                self.text_buf.push_str(escaped);
            } else if (ch as u32) < 32 {
                use std::fmt::Write;
                let _ = write!(self.text_buf, "\\x{:02x}", ch as u32);
            } else {
                self.text_buf.push(ch);
            }
        }
        self.text_buf.push('"');
    }
    fn multiline_str(&mut self, s: &str, first_newline: usize) {
        let bytes = s.as_bytes();
        self.text_buf.push('|');
        if bytes[0] == 32 {
            use std::fmt::Write;
            let _ = write!(self.text_buf, "{}", self.width);
        }
        if bytes.last() == Some(&10) {
            if bytes.len() == 1 || bytes[bytes.len() - 2] == 10 {
                self.text_buf.push('+');
            }
        } else {
            self.text_buf.push('-');
        }
        self.text_buf.push('\n');
        let mut start = 0;
        for i in first_newline..bytes.len() {
            if bytes[i] != 10 {
                continue;
            }
            if i == start {
                self.text_buf.push('\n');
            } else {
                self.spaces(self.indent + self.width);
                self.text_buf.push_str(&s[start..=i]);
            }
            start = i + 1;
        }
        if start < bytes.len() {
            self.spaces(self.indent + self.width);
            self.text_buf.push_str(&s[start..]);
        }
    }
    fn single_quoted_u16(&mut self, u: &[u16]) {
        let out = self.promote_utf16();
        out.push(39);
        for &ch in u {
            out.push(ch);
            if ch == 39 {
                out.push(ch);
            }
        }
        out.push(39);
    }
    fn double_quoted_u16(&mut self, u: &[u16]) {
        self.promote_utf16().push(34);
        for &ch in u {
            let escaped = match ch {
                0 => Some("\\0"),
                7 => Some("\\a"),
                8 => Some("\\b"),
                9 => Some("\\t"),
                10 => Some("\\n"),
                11 => Some("\\v"),
                12 => Some("\\f"),
                13 => Some("\\r"),
                27 => Some("\\e"),
                34 => Some("\\\""),
                92 => Some("\\\\"),
                0x85 => Some("\\N"),
                0xa0 => Some("\\_"),
                0x2028 => Some("\\L"),
                0x2029 => Some("\\P"),
                _ => None,
            };
            if let Some(escaped) = escaped {
                self.text(escaped);
            } else if ch < 32 {
                self.text(&format!("\\x{ch:02x}"));
            } else {
                self.promote_utf16().push(ch);
            }
        }
        self.promote_utf16().push(34);
    }
    fn multiline_u16(&mut self, u: &[u16], first_newline: usize) {
        self.text("|");
        if u[0] == 32 {
            self.text(&self.width.to_string());
        }
        if u.last() == Some(&10) {
            if u.len() == 1 || u[u.len() - 2] == 10 {
                self.text("+");
            }
        } else {
            self.text("-");
        }
        self.text("\n");
        let mut start = 0;
        for i in first_newline..u.len() {
            if u[i] != 10 {
                continue;
            }
            if i == start {
                self.text("\n");
            } else {
                self.spaces(self.indent + self.width);
                self.promote_utf16().extend_from_slice(&u[start..=i]);
            }
            start = i + 1;
        }
        if start < u.len() {
            self.spaces(self.indent + self.width);
            self.promote_utf16().extend_from_slice(&u[start..]);
        }
    }
}
fn reserved(text: Option<&str>, mode: Mode) -> bool {
    let Some(text) = text else {
        return false;
    };
    matches!(
        text,
        "" | "~"
            | "null"
            | "Null"
            | "NULL"
            | ".nan"
            | ".NaN"
            | ".NAN"
            | ".inf"
            | ".Inf"
            | ".INF"
            | "+.inf"
            | "+.Inf"
            | "+.INF"
            | "-.inf"
            | "-.Inf"
            | "-.INF"
            | "true"
            | "True"
            | "TRUE"
            | "false"
            | "False"
            | "FALSE"
    ) || mode != Mode::Yaml12
        && matches!(
            text,
            "on" | "On"
                | "ON"
                | "off"
                | "Off"
                | "OFF"
                | "y"
                | "Y"
                | "yes"
                | "Yes"
                | "YES"
                | "n"
                | "N"
                | "no"
                | "No"
                | "NO"
        )
}
fn digit(ch: u16) -> bool {
    (48..=57).contains(&ch)
}
fn digits(u: &[u16], start: usize, radix: u16, underscore: bool) -> bool {
    start < u.len()
        && u[start..].iter().all(|&c| {
            underscore && c == 95
                || (48..48 + radix.min(10)).contains(&c)
                || radix == 16 && ((65..=70).contains(&c) || (97..=102).contains(&c))
        })
}
fn decimal(u: &[u16], start: usize, mode: Mode) -> bool {
    let accepted = |c| digit(c) || (mode != Mode::Yaml12 && c == 95);
    let mut i = start;
    while i < u.len() && accepted(u[i]) {
        i += 1;
    }
    if i == u.len() {
        return true;
    }
    if u[i] == 46 {
        i += 1;
        while i < u.len() && accepted(u[i]) {
            i += 1;
        }
        if i == u.len() {
            return true;
        }
    } else if mode == Mode::Yaml11 {
        return false;
    }
    if matches!(u[i], 101 | 69) {
        i += 1;
        if i == u.len() {
            return false;
        }
        if mode == Mode::Yaml11 && !matches!(u[i], 43 | 45) {
            return false;
        }
        if matches!(u[i], 43 | 45) {
            i += 1;
            if i == u.len() {
                return false;
            }
        }
        while i < u.len() && digit(u[i]) {
            i += 1;
        }
    }
    i == u.len()
}
fn sexagesimal(u: &[u16], start: usize, colon: usize) -> bool {
    if !(49..=57).contains(&u[start]) || !u[start + 1..colon].iter().all(|&c| digit(c) || c == 95) {
        return false;
    }
    let mut state = 1;
    for (i, &c) in u.iter().enumerate().skip(colon + 1) {
        state = match state {
            0 => match c {
                58 => 1,
                46 => 3,
                _ => return false,
            },
            1 => match c {
                48..=53 => 2,
                54..=57 => 0,
                _ => return false,
            },
            2 => match c {
                58 => 1,
                46 => 3,
                48..=57 => 0,
                _ => return false,
            },
            _ => return u[i..].iter().all(|&c| digit(c) || c == 95),
        };
    }
    state != 1
}
fn number(u: &[u16], colon: Option<usize>, mode: Mode) -> bool {
    if u.len() == 1 {
        return digit(u[0]) || (mode != Mode::Yaml12 && u[0] == 46);
    }
    if mode == Mode::Yaml12 {
        return match u[0] {
            48 => match u[1] {
                111 => digits(u, 2, 8, false),
                120 => digits(u, 2, 16, false),
                _ => decimal(u, 1, mode),
            },
            43 | 45 => decimal(u, 1, mode),
            _ => decimal(u, 0, mode),
        };
    }
    let start = usize::from(matches!(u[0], 43 | 45));
    if let Some(colon) = colon {
        return sexagesimal(u, start, colon);
    }
    match u[start] {
        111 => return digits(u, start + 1, 8, true),
        48 => {
            if start == u.len() - 1 {
                return true;
            }
            match u[start + 1] {
                98 => return digits(u, start + 2, 2, true),
                111 if mode == Mode::Compat => return digits(u, start + 2, 8, true),
                120 => return digits(u, start + 2, 16, true),
                _ => (),
            }
        }
        _ => (),
    }
    decimal(u, start, mode)
}
fn number_ascii(bytes: &[u8], colon: Option<usize>, mode: Mode) -> bool {
    if bytes.len() <= 64 {
        let mut buf = [0u16; 64];
        for (dst, &src) in buf.iter_mut().zip(bytes.iter()) {
            *dst = u16::from(src);
        }
        return number(&buf[..bytes.len()], colon, mode);
    }
    let units: Vec<u16> = bytes.iter().map(|&b| u16::from(b)).collect();
    number(&units, colon, mode)
}
