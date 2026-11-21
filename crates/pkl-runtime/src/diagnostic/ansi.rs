// Portions adapted from Pkl 0.32.0 AnsiStringBuilder.java and AnsiTheme.java.
// Copyright © 2024-2026 Apple Inc. and the Pkl project authors. All rights reserved.
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at https://www.apache.org/licenses/LICENSE-2.0
// Unless required by applicable law or agreed to in writing, software distributed
// under the License is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR
// CONDITIONS OF ANY KIND, either express or implied. See the License for the
// specific language governing permissions and limitations under the License.

//! Wasm-pure ANSI SGR escape-code builder and theme mirroring Apple Pkl 0.32.0
//! `AnsiStringBuilder` and `AnsiTheme` verbatim (including `EnumSet` ordinal ordering,
//! incremental code diffing, `appendUntrusted`, and `appendSandboxed`).

use crate::value::PklString;

/// ANSI SGR codes in exact Java `AnsiStringBuilder.AnsiCode` declaration order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum AnsiCode {
    Reset = 0,
    Bold = 1,
    Faint = 2,

    Black = 3,
    Red = 4,
    Green = 5,
    Yellow = 6,
    Blue = 7,
    Magenta = 8,
    Cyan = 9,
    White = 10,

    BgBlack = 11,
    BgRed = 12,
    BgGreen = 13,
    BgYellow = 14,
    BgBlue = 15,
    BgMagenta = 16,
    BgCyan = 17,
    BgWhite = 18,

    BrightBlack = 19,
    BrightRed = 20,
    BrightGreen = 21,
    BrightYellow = 22,
    BrightBlue = 23,
    BrightMagenta = 24,
    BrightCyan = 25,
    BrightWhite = 26,

    BgBrightBlack = 27,
    BgBrightRed = 28,
    BgBrightGreen = 29,
    BgBrightYellow = 30,
    BgBrightBlue = 31,
    BgBrightMagenta = 32,
    BgBrightCyan = 33,
    BgBrightWhite = 34,
}

const ALL_CODES: [AnsiCode; 35] = [
    AnsiCode::Reset,
    AnsiCode::Bold,
    AnsiCode::Faint,
    AnsiCode::Black,
    AnsiCode::Red,
    AnsiCode::Green,
    AnsiCode::Yellow,
    AnsiCode::Blue,
    AnsiCode::Magenta,
    AnsiCode::Cyan,
    AnsiCode::White,
    AnsiCode::BgBlack,
    AnsiCode::BgRed,
    AnsiCode::BgGreen,
    AnsiCode::BgYellow,
    AnsiCode::BgBlue,
    AnsiCode::BgMagenta,
    AnsiCode::BgCyan,
    AnsiCode::BgWhite,
    AnsiCode::BrightBlack,
    AnsiCode::BrightRed,
    AnsiCode::BrightGreen,
    AnsiCode::BrightYellow,
    AnsiCode::BrightBlue,
    AnsiCode::BrightMagenta,
    AnsiCode::BrightCyan,
    AnsiCode::BrightWhite,
    AnsiCode::BgBrightBlack,
    AnsiCode::BgBrightRed,
    AnsiCode::BgBrightGreen,
    AnsiCode::BgBrightYellow,
    AnsiCode::BgBrightBlue,
    AnsiCode::BgBrightMagenta,
    AnsiCode::BgBrightCyan,
    AnsiCode::BgBrightWhite,
];

impl AnsiCode {
    pub const fn sgr_value(self) -> u8 {
        match self {
            Self::Reset => 0,
            Self::Bold => 1,
            Self::Faint => 2,
            Self::Black => 30,
            Self::Red => 31,
            Self::Green => 32,
            Self::Yellow => 33,
            Self::Blue => 34,
            Self::Magenta => 35,
            Self::Cyan => 36,
            Self::White => 37,
            Self::BgBlack => 40,
            Self::BgRed => 41,
            Self::BgGreen => 42,
            Self::BgYellow => 43,
            Self::BgBlue => 44,
            Self::BgMagenta => 45,
            Self::BgCyan => 46,
            Self::BgWhite => 47,
            Self::BrightBlack => 90,
            Self::BrightRed => 91,
            Self::BrightGreen => 92,
            Self::BrightYellow => 93,
            Self::BrightBlue => 94,
            Self::BrightMagenta => 95,
            Self::BrightCyan => 96,
            Self::BrightWhite => 97,
            Self::BgBrightBlack => 100,
            Self::BgBrightRed => 101,
            Self::BgBrightGreen => 102,
            Self::BgBrightYellow => 103,
            Self::BgBrightBlue => 104,
            Self::BgBrightMagenta => 105,
            Self::BgBrightCyan => 106,
            Self::BgBrightWhite => 107,
        }
    }

    pub const fn as_set(self) -> AnsiCodeSet {
        AnsiCodeSet(1u64 << (self as u8))
    }
}

/// Bitset matching Java `EnumSet<AnsiCode>` iteration order (enum ordinal order).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AnsiCodeSet(u64);

impl AnsiCodeSet {
    pub const EMPTY: Self = Self(0);

    pub const fn of(code: AnsiCode) -> Self {
        code.as_set()
    }

    pub const fn of2(a: AnsiCode, b: AnsiCode) -> Self {
        Self((1u64 << (a as u8)) | (1u64 << (b as u8)))
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub const fn contains_all(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    pub const fn difference(self, other: Self) -> Self {
        Self(self.0 & !other.0)
    }
}

impl From<AnsiCode> for AnsiCodeSet {
    fn from(code: AnsiCode) -> Self {
        Self::of(code)
    }
}

/// Exact theme constants from `org.pkl.core.util.AnsiTheme`.
pub mod theme {
    use super::{AnsiCode, AnsiCodeSet};

    pub const ERROR_MESSAGE_HINT: AnsiCodeSet = AnsiCodeSet::of(AnsiCode::Yellow);
    pub const ERROR_HEADER: AnsiCodeSet = AnsiCodeSet::of(AnsiCode::Red);
    pub const ERROR_MESSAGE: AnsiCodeSet = AnsiCodeSet::of2(AnsiCode::Red, AnsiCode::Bold);

    pub const STACK_FRAME: AnsiCodeSet = AnsiCodeSet::of(AnsiCode::Faint);
    pub const STACK_TRACE_MARGIN: AnsiCodeSet = AnsiCodeSet::of(AnsiCode::Yellow);
    pub const STACK_TRACE_LINE_NUMBER: AnsiCodeSet = AnsiCodeSet::of(AnsiCode::Faint);
    pub const STACK_TRACE_LOOP_COUNT: AnsiCodeSet = AnsiCodeSet::of(AnsiCode::Magenta);
    pub const STACK_TRACE_CARET: AnsiCodeSet = AnsiCodeSet::of(AnsiCode::Red);

    pub const FAILING_TEST_MARK: AnsiCodeSet = AnsiCodeSet::of(AnsiCode::Red);
    pub const PASSING_TEST_MARK: AnsiCodeSet = AnsiCodeSet::of(AnsiCode::Green);
    pub const TEST_NAME: AnsiCodeSet = AnsiCodeSet::of(AnsiCode::Faint);
    pub const TEST_FACT_SOURCE: AnsiCodeSet = AnsiCodeSet::of(AnsiCode::Red);
    pub const TEST_FAILURE_MESSAGE: AnsiCodeSet = AnsiCodeSet::of(AnsiCode::Red);
    pub const TEST_EXAMPLE_OUTPUT: AnsiCodeSet = AnsiCodeSet::of2(AnsiCode::Red, AnsiCode::Bold);

    pub const SYNTAX_KEYWORD: AnsiCodeSet = AnsiCodeSet::of(AnsiCode::Blue);
    pub const SYNTAX_NUMBER: AnsiCodeSet = AnsiCodeSet::of(AnsiCode::Green);
    pub const SYNTAX_STRING: AnsiCodeSet = AnsiCodeSet::of(AnsiCode::Yellow);
    pub const SYNTAX_STRING_ESCAPE: AnsiCodeSet = AnsiCodeSet::of(AnsiCode::BrightYellow);
    pub const SYNTAX_COMMENT: AnsiCodeSet = AnsiCodeSet::of(AnsiCode::Faint);
    pub const SYNTAX_OPERATOR: AnsiCodeSet = AnsiCodeSet::of(AnsiCode::Reset);
    pub const SYNTAX_CONTROL: AnsiCodeSet = AnsiCodeSet::of(AnsiCode::Blue);
    pub const SYNTAX_CONSTANT: AnsiCodeSet = AnsiCodeSet::of(AnsiCode::Cyan);
}

/// UTF-16-lossless ANSI escape-code builder matching `org.pkl.core.util.AnsiStringBuilder`.
#[derive(Clone, Debug)]
pub struct AnsiStringBuilder {
    units: Vec<u16>,
    using_color: bool,
    current_codes: AnsiCodeSet,
    declared_codes: AnsiCodeSet,
}

impl AnsiStringBuilder {
    pub fn new(using_color: bool) -> Self {
        Self {
            units: Vec::new(),
            using_color,
            current_codes: AnsiCodeSet::EMPTY,
            declared_codes: AnsiCodeSet::EMPTY,
        }
    }

    pub fn new_instance(&self) -> Self {
        Self::new(self.using_color)
    }

    pub fn using_color(&self) -> bool {
        self.using_color
    }

    pub fn len(&self) -> usize {
        self.units.len()
    }

    pub fn is_empty(&self) -> bool {
        self.units.is_empty()
    }

    pub fn set_length(&mut self, len: usize) {
        self.units.truncate(len);
    }

    pub fn append_str(&mut self, value: &str) -> &mut Self {
        self.append_codes();
        self.units.extend(value.encode_utf16());
        self
    }

    pub fn append_char(&mut self, value: char) -> &mut Self {
        self.append_codes();
        let mut buf = [0u16; 2];
        self.units.extend_from_slice(value.encode_utf16(&mut buf));
        self
    }

    pub fn append_usize(&mut self, value: usize) -> &mut Self {
        self.append_str(&value.to_string())
    }

    pub fn append_utf16(&mut self, value: &[u16]) -> &mut Self {
        self.append_codes();
        self.units.extend_from_slice(value);
        self
    }

    pub fn append_pkl(&mut self, value: &PklString) -> &mut Self {
        self.append_codes();
        self.units.extend(value.encode_utf16());
        self
    }

    pub fn append_styled(&mut self, codes: impl Into<AnsiCodeSet>, value: &str) -> &mut Self {
        if !self.using_color {
            self.units.extend(value.encode_utf16());
            return self;
        }
        let prev = self.declared_codes;
        self.declared_codes = codes.into().union(prev);
        self.append_str(value);
        self.declared_codes = prev;
        self
    }

    pub fn append_styled_utf16(
        &mut self,
        codes: impl Into<AnsiCodeSet>,
        value: &[u16],
    ) -> &mut Self {
        if !self.using_color {
            self.units.extend_from_slice(value);
            return self;
        }
        let prev = self.declared_codes;
        self.declared_codes = codes.into().union(prev);
        self.append_utf16(value);
        self.declared_codes = prev;
        self
    }

    pub fn append_styled_pkl(
        &mut self,
        codes: impl Into<AnsiCodeSet>,
        value: &PklString,
    ) -> &mut Self {
        if !self.using_color {
            self.units.extend(value.encode_utf16());
            return self;
        }
        let prev = self.declared_codes;
        self.declared_codes = codes.into().union(prev);
        self.append_pkl(value);
        self.declared_codes = prev;
        self
    }

    pub fn append_styled_usize(
        &mut self,
        codes: impl Into<AnsiCodeSet>,
        value: usize,
    ) -> &mut Self {
        self.append_styled(codes, &value.to_string())
    }

    pub(crate) fn enter_scope(&mut self, codes: impl Into<AnsiCodeSet>) -> AnsiCodeSet {
        let prev = self.declared_codes;
        if self.using_color {
            self.declared_codes = codes.into().union(prev);
        }
        prev
    }

    pub(crate) fn exit_scope(&mut self, prev: AnsiCodeSet) {
        if self.using_color {
            self.declared_codes = prev;
        }
    }

    pub(crate) fn enter_sandbox(&mut self) -> (AnsiCodeSet, AnsiCodeSet) {
        let saved = (self.current_codes, self.declared_codes);
        if self.using_color {
            self.current_codes = AnsiCodeSet::EMPTY;
            self.declared_codes = AnsiCodeSet::EMPTY;
            self.do_reset();
        }
        saved
    }

    pub(crate) fn exit_sandbox(&mut self, cur: AnsiCodeSet, decl: AnsiCodeSet) {
        if self.using_color {
            self.do_reset();
            self.current_codes = cur;
            self.declared_codes = decl;
            self.do_append_codes(self.current_codes);
        }
    }

    pub fn append_scoped(
        &mut self,
        codes: impl Into<AnsiCodeSet>,
        f: impl FnOnce(&mut Self),
    ) -> &mut Self {
        if !self.using_color {
            f(self);
            return self;
        }
        let prev = self.declared_codes;
        self.declared_codes = codes.into().union(prev);
        f(self);
        self.declared_codes = prev;
        self
    }

    pub fn append_sandboxed(&mut self, f: impl FnOnce(&mut Self)) -> &mut Self {
        if !self.using_color {
            f(self);
            return self;
        }
        let my_codes = self.current_codes;
        let my_declared = self.declared_codes;
        self.current_codes = AnsiCodeSet::EMPTY;
        self.declared_codes = AnsiCodeSet::EMPTY;
        self.do_reset();
        f(self);
        self.do_reset();
        self.current_codes = my_codes;
        self.declared_codes = my_declared;
        self.do_append_codes(self.current_codes);
        self
    }

    pub fn append_untrusted_str(&mut self, value: &str) -> &mut Self {
        self.append_codes();
        self.units.extend(value.encode_utf16());
        if self.using_color {
            self.do_reset();
            self.do_append_codes(self.current_codes);
        }
        self
    }

    pub fn append_untrusted_pkl(&mut self, value: &PklString) -> &mut Self {
        self.append_codes();
        self.units.extend(value.encode_utf16());
        if self.using_color {
            self.do_reset();
            self.do_append_codes(self.current_codes);
        }
        self
    }

    pub fn finish_pkl(mut self) -> PklString {
        self.reset();
        PklString::from_utf16(self.units)
    }

    pub fn finish_string(self) -> String {
        self.finish_pkl().to_java_utf8()
    }

    fn do_append_codes(&mut self, codes: AnsiCodeSet) {
        if codes.is_empty() {
            return;
        }
        self.units.extend("\x1b[".encode_utf16());
        let mut first = true;
        for code in ALL_CODES {
            if (codes.0 & (1u64 << (code as u8))) != 0 {
                if first {
                    first = false;
                } else {
                    self.units.push(b';' as u16);
                }
                let s = code.sgr_value().to_string();
                self.units.extend(s.encode_utf16());
            }
        }
        self.units.push(b'm' as u16);
    }

    fn append_codes(&mut self) {
        if !self.using_color || self.current_codes == self.declared_codes {
            return;
        }
        if self.declared_codes.contains_all(self.current_codes) {
            let new_codes = self.declared_codes.difference(self.current_codes);
            self.do_append_codes(new_codes);
        } else {
            self.reset();
            self.do_append_codes(self.declared_codes);
        }
        self.current_codes = self.declared_codes;
    }

    fn reset(&mut self) {
        if !self.using_color || self.current_codes.is_empty() {
            return;
        }
        self.do_reset();
        self.current_codes = AnsiCodeSet::EMPTY;
    }

    fn do_reset(&mut self) {
        self.units.extend("\x1b[0m".encode_utf16());
    }
}

/// Strips ANSI SGR color escape sequences (`\x1b\[[;\d]*m`), matching `JUnitReporter.java`.
pub fn strip_ansi_codes(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = String::with_capacity(input.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == 0x1b && i + 1 < bytes.len() && bytes[i + 1] == b'[' {
            let mut j = i + 2;
            while j < bytes.len() && (bytes[j].is_ascii_digit() || bytes[j] == b';') {
                j += 1;
            }
            if j < bytes.len() && bytes[j] == b'm' {
                i = j + 1;
                continue;
            }
        }
        let ch = input[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enum_set_ordinal_ordering_matches_java() {
        let mut sb = AnsiStringBuilder::new(true);
        sb.append_styled(theme::ERROR_HEADER, "–– Pkl Error ––")
            .append_char('\n')
            .append_styled(theme::ERROR_MESSAGE, "boom")
            .append_char('\n');
        assert_eq!(
            sb.finish_string(),
            "\x1b[31m–– Pkl Error ––\x1b[0m\n\x1b[1;31mboom\x1b[0m\n"
        );
    }

    #[test]
    fn untrusted_resets_and_reapplies_active_style() {
        let mut sb = AnsiStringBuilder::new(true);
        sb.append_scoped(theme::STACK_FRAME, |sb| {
            sb.append_str("at text#x (")
                .append_untrusted_str("repl:text")
                .append_str(")\n");
        });
        assert_eq!(
            sb.finish_string(),
            "\x1b[2mat text#x (repl:text\x1b[0m\x1b[2m)\n\x1b[0m"
        );
    }

    #[test]
    fn strip_ansi_removes_sgr_sequences() {
        assert_eq!(
            strip_ansi_codes("\x1b[1;31mboom\x1b[0m ok \x1b[33mhi\x1b[0m"),
            "boom ok hi"
        );
    }
}
