//! Pinned YAML emitter semantics, independent of evaluator traversal and Host I/O.
//! The output buffer counts Java UTF-16 units, including unmatched surrogates.
use crate::value::PklString;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Mode {
    Compat,
    Yaml11,
    Yaml12,
}

pub(super) struct Emitter {
    output: Vec<u16>,
    width: usize,
    indent: usize,
    containers: Vec<bool>,
    mode: Mode,
}
impl Emitter {
    pub(super) fn new(mode: Mode, width: usize) -> Self {
        Self {
            output: Vec::new(),
            width,
            indent: 0,
            containers: Vec::new(),
            mode,
        }
    }
    pub(super) fn is_empty(&self) -> bool {
        self.output.is_empty()
    }
    pub(super) fn raw(&mut self, text: &PklString) {
        self.output.extend(text.encode_utf16());
    }
    pub(super) fn text(&mut self, text: &str) {
        self.output.extend(text.encode_utf16());
    }
    pub(super) fn finish(self) -> PklString {
        PklString::from_utf16(self.output)
    }
    fn spaces(&mut self, length: usize) {
        self.output.resize(self.output.len() + length, 32);
    }
    pub(super) fn newline(&mut self) {
        if self.output.is_empty() {
            return;
        }
        if self.output.last() != Some(&10) {
            self.output.push(10);
        }
        self.spaces(self.indent);
    }
    fn undo_newline(&mut self) {
        if !self.output.is_empty() {
            self.output.truncate(self.output.len() - self.indent - 1);
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
        if !self.output.is_empty() {
            self.text(" ");
        }
        self.raw(value);
    }
    pub(super) fn string_value(&mut self, value: &PklString) {
        if !self.output.is_empty() {
            self.text(" ");
        }
        self.string(value, false);
    }
    pub(super) fn bytes(&mut self, value: &[u8]) {
        if !self.output.is_empty() {
            self.text(" ");
        }
        self.text("!!binary ");
        self.string(&super::resources::base64(value), false);
    }
    pub(super) fn string(&mut self, value: &PklString, key: bool) {
        let u: Vec<u16> = value.encode_utf16().collect();
        if reserved(value.as_str(), self.mode) {
            self.single_quoted(&u);
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
        let start = self.output.len();
        if escape {
            self.double_quoted(&u);
        } else if let Some(newline) = newline {
            self.multiline(&u, newline);
        } else if quote || (!non_number && number(&u, colon, self.mode)) {
            self.single_quoted(&u);
        } else {
            self.output.extend_from_slice(&u);
        }
        if key && (self.output.len() - start > 1024 || (!escape && newline.is_some())) {
            self.output.splice(start..start, [63, 32]);
            self.output.push(10);
            self.spaces(self.indent);
        }
    }
    fn single_quoted(&mut self, u: &[u16]) {
        self.output.push(39);
        for &ch in u {
            self.output.push(ch);
            if ch == 39 {
                self.output.push(ch);
            }
        }
        self.output.push(39);
    }
    fn double_quoted(&mut self, u: &[u16]) {
        self.output.push(34);
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
                self.output.push(ch);
            }
        }
        self.output.push(34);
    }
    fn multiline(&mut self, u: &[u16], first_newline: usize) {
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
                self.output.extend_from_slice(&u[start..=i]);
            }
            start = i + 1;
        }
        if start < u.len() {
            self.spaces(self.indent + self.width);
            self.output.extend_from_slice(&u[start..]);
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
