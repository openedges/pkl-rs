//! Exact UTF-16 source with a collision-free byte address space for the lexer.
//!
//! Valid Unicode uses its original UTF-8 bytes. A lone surrogate has its own
//! WTF-8 triple, which cannot alias the UTF-8 encoding of any Unicode scalar.
//! This representation never rewrites a token, quotes source, or substitutes a
//! private-use character. Rendering uses the retained original code units.
use std::ops::{Bound, RangeBounds};
use std::sync::Arc;

use crate::value::PklString;

use super::{Position, Span};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceText {
    original: PklString,
    // None keeps the existing scalar UTF-8 representation without a second
    // encoding/allocation. Some is canonical WTF-8, derived only from UTF-16.
    wtf8: Option<Arc<[u8]>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceCharacter {
    Scalar(char),
    Surrogate(u16),
}

impl SourceCharacter {
    pub fn scalar(self) -> Option<char> {
        match self {
            Self::Scalar(character) => Some(character),
            Self::Surrogate(_) => None,
        }
    }

    pub fn len_bytes(self) -> usize {
        match self {
            Self::Scalar(character) => character.len_utf8(),
            Self::Surrogate(_) => 3,
        }
    }

    pub fn len_utf16(self) -> usize {
        match self {
            Self::Scalar(character) => character.len_utf16(),
            Self::Surrogate(_) => 1,
        }
    }

    pub(crate) fn first_utf16(self) -> u16 {
        match self {
            Self::Scalar(character) => character.encode_utf16(&mut [0; 2])[0],
            Self::Surrogate(unit) => unit,
        }
    }

    fn append_utf16(self, output: &mut Vec<u16>) {
        match self {
            Self::Scalar(character) => {
                let mut units = [0; 2];
                output.extend_from_slice(character.encode_utf16(&mut units));
            }
            Self::Surrogate(unit) => output.push(unit),
        }
    }
}

impl From<&PklString> for SourceText {
    fn from(value: &PklString) -> Self {
        Self::from(value.clone())
    }
}

impl From<PklString> for SourceText {
    fn from(original: PklString) -> Self {
        let wtf8 = if original.as_str().is_some() {
            None
        } else {
            let mut bytes = Vec::new();
            for character in char::decode_utf16(original.encode_utf16()) {
                match character {
                    Ok(character) => {
                        let mut encoded = [0; 4];
                        bytes.extend_from_slice(character.encode_utf8(&mut encoded).as_bytes());
                    }
                    Err(error) => {
                        let unit = error.unpaired_surrogate();
                        bytes.extend_from_slice(&[
                            0xe0 | ((unit >> 12) as u8),
                            0x80 | (((unit >> 6) & 0x3f) as u8),
                            0x80 | ((unit & 0x3f) as u8),
                        ]);
                    }
                }
            }
            Some(bytes.into())
        };
        Self { original, wtf8 }
    }
}

impl From<&str> for SourceText {
    fn from(text: &str) -> Self {
        Self::from(PklString::from(text))
    }
}

impl From<String> for SourceText {
    fn from(text: String) -> Self {
        Self::from(PklString::from(text))
    }
}

impl From<Arc<str>> for SourceText {
    fn from(text: Arc<str>) -> Self {
        Self::from(PklString::from(text))
    }
}

impl<'a> From<&'a str> for SourceSlice<'a> {
    fn from(text: &'a str) -> Self {
        Self::from_utf8(text)
    }
}

impl<'a> From<&'a String> for SourceSlice<'a> {
    fn from(text: &'a String) -> Self {
        Self::from_utf8(text.as_str())
    }
}

impl From<&String> for SourceText {
    fn from(text: &String) -> Self {
        Self::from(text.as_str())
    }
}

impl From<&SourceText> for SourceText {
    fn from(text: &SourceText) -> Self {
        text.clone()
    }
}

impl<'a> From<&'a SourceText> for SourceSlice<'a> {
    fn from(text: &'a SourceText) -> Self {
        text.entire()
    }
}

impl SourceText {
    pub fn from_utf16(units: impl Into<Vec<u16>>) -> Self {
        Self::from(PklString::from_utf16(units))
    }

    pub fn original(&self) -> &PklString {
        &self.original
    }

    pub fn as_str(&self) -> Option<&str> {
        self.original.as_str()
    }

    /// Internal lexer bytes, not a UTF-8 output channel or serialized source.
    pub fn as_bytes(&self) -> &[u8] {
        match &self.wtf8 {
            Some(bytes) => bytes,
            None => self
                .original
                .as_str()
                .expect("scalar source invariant")
                .as_bytes(),
        }
    }

    pub fn len_bytes(&self) -> usize {
        self.as_bytes().len()
    }

    pub fn is_empty(&self) -> bool {
        self.as_bytes().is_empty()
    }

    pub fn entire(&self) -> SourceSlice<'_> {
        SourceSlice {
            bytes: self.as_bytes(),
        }
    }

    pub fn find(&self, pattern: &str) -> Option<usize> {
        self.entire().find(pattern)
    }
    pub fn rfind(&self, pattern: &str) -> Option<usize> {
        self.entire().rfind(pattern)
    }

    pub fn get(&self, range: impl RangeBounds<usize>) -> Option<SourceSlice<'_>> {
        self.entire().get(range)
    }

    pub fn slice(&self, span: Span) -> Option<SourceSlice<'_>> {
        self.entire().slice(span)
    }

    pub fn characters(&self) -> SourceCharacters<'_> {
        self.entire().characters()
    }

    pub fn index(&self) -> SourceTextIndex {
        let mut position = Position {
            byte: 0,
            utf16: 0,
            codepoint: 0,
            line: 1,
            column_utf16: 1,
        };
        let mut positions = vec![position];
        for (_, character) in self.characters() {
            position.byte += character.len_bytes();
            position.utf16 += character.len_utf16();
            position.codepoint += 1;
            if character == SourceCharacter::Scalar('\n')
                || (character == SourceCharacter::Scalar('\r')
                    && self.as_bytes().get(position.byte) != Some(&b'\n'))
            {
                position.line += 1;
                position.column_utf16 = 1;
            } else {
                position.column_utf16 += character.len_utf16();
            }
            positions.push(position);
        }
        SourceTextIndex {
            positions: positions.into(),
        }
    }

    /// Exact line contents, excluding CR, LF, or CRLF terminators.
    pub fn lines(&self) -> SourceLines<'_> {
        SourceLines {
            remaining: self.entire(),
            done: false,
        }
    }

    pub fn line(&self, wanted: usize) -> Option<PklString> {
        self.lines()
            .nth(wanted.checked_sub(1)?)
            .map(SourceSlice::to_pkl_string)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceSlice<'a> {
    bytes: &'a [u8],
}

impl<'a> SourceSlice<'a> {
    pub fn from_utf8(text: &'a str) -> Self {
        Self {
            bytes: text.as_bytes(),
        }
    }

    pub fn as_bytes(self) -> &'a [u8] {
        self.bytes
    }

    pub fn as_str(self) -> Option<&'a str> {
        std::str::from_utf8(self.bytes).ok()
    }

    pub fn is_empty(self) -> bool {
        self.bytes.is_empty()
    }

    pub fn len_bytes(self) -> usize {
        self.bytes.len()
    }

    pub fn is_boundary(self, byte: usize) -> bool {
        byte == self.bytes.len()
            || self
                .bytes
                .get(byte)
                .is_some_and(|value| value & 0xc0 != 0x80)
    }

    pub fn slice(self, span: Span) -> Option<Self> {
        if span.start > span.end || !self.is_boundary(span.start) || !self.is_boundary(span.end) {
            return None;
        }
        Some(Self {
            bytes: self.bytes.get(span.start..span.end)?,
        })
    }

    /// A checked byte range. A scalar pair and a lone surrogate each remain
    /// indivisible in the internal byte address space.
    pub fn get(self, range: impl RangeBounds<usize>) -> Option<Self> {
        let start = match range.start_bound() {
            Bound::Included(value) => *value,
            Bound::Excluded(value) => value.checked_add(1)?,
            Bound::Unbounded => 0,
        };
        let end = match range.end_bound() {
            Bound::Included(value) => value.checked_add(1)?,
            Bound::Excluded(value) => *value,
            Bound::Unbounded => self.len_bytes(),
        };
        self.slice(Span::new(start, end))
    }

    /// Parser cursors and token spans are already checked source boundaries.
    pub(crate) fn range(self, range: impl RangeBounds<usize>) -> Self {
        self.get(range).expect("parser source boundary")
    }

    pub(crate) fn bytes(self) -> impl Iterator<Item = u8> + 'a {
        self.bytes.iter().copied()
    }

    pub fn starts_with(self, prefix: &str) -> bool {
        self.bytes.starts_with(prefix.as_bytes())
    }

    pub fn ends_with(self, suffix: &str) -> bool {
        self.bytes.ends_with(suffix.as_bytes())
    }

    pub fn strip_prefix(self, prefix: &str) -> Option<Self> {
        self.starts_with(prefix)
            .then(|| self.get(prefix.len()..))
            .flatten()
    }

    pub fn strip_suffix(self, suffix: &str) -> Option<Self> {
        self.ends_with(suffix)
            .then(|| self.get(..self.len_bytes() - suffix.len()))
            .flatten()
    }

    pub fn find(self, pattern: &str) -> Option<usize> {
        if pattern.is_empty() {
            return Some(0);
        }
        self.bytes
            .windows(pattern.len())
            .position(|bytes| bytes == pattern.as_bytes())
    }

    pub(crate) fn rfind(self, pattern: &str) -> Option<usize> {
        if pattern.is_empty() {
            return Some(self.len_bytes());
        }
        self.bytes
            .windows(pattern.len())
            .rposition(|bytes| bytes == pattern.as_bytes())
    }

    pub(crate) fn split_once(self, pattern: &str) -> Option<(Self, Self)> {
        let at = self.find(pattern)?;
        Some((self.range(..at), self.range(at + pattern.len()..)))
    }

    pub(crate) fn rsplit_once(self, pattern: &str) -> Option<(Self, Self)> {
        let at = self.rfind(pattern)?;
        Some((self.range(..at), self.range(at + pattern.len()..)))
    }

    pub(crate) fn split(self, pattern: &'static str) -> impl Iterator<Item = Self> {
        assert!(
            !pattern.is_empty(),
            "source split delimiter must not be empty"
        );
        let mut remaining = Some(self);
        std::iter::from_fn(move || {
            let view = remaining.take()?;
            if let Some((before, after)) = view.split_once(pattern) {
                remaining = Some(after);
                Some(before)
            } else {
                Some(view)
            }
        })
    }

    pub(crate) fn append_to(self, output: &mut crate::pkl_string::PklStringBuilder) {
        if let Some(text) = self.as_str() {
            output.push_str(text);
        } else {
            for (_, character) in self.characters() {
                match character {
                    SourceCharacter::Scalar(ch) => output.push(ch),
                    SourceCharacter::Surrogate(unit) => output.push_unit(unit),
                }
            }
        }
    }

    pub fn utf16_len(self) -> usize {
        self.characters()
            .map(|(_, character)| character.len_utf16())
            .sum()
    }

    pub fn first(self) -> Option<SourceCharacter> {
        self.characters().next().map(|(_, character)| character)
    }

    pub fn characters(self) -> SourceCharacters<'a> {
        SourceCharacters {
            bytes: self.bytes,
            offset: 0,
            end: self.bytes.len(),
        }
    }

    pub fn to_pkl_string(self) -> PklString {
        if let Some(text) = self.as_str() {
            return text.into();
        }
        let mut units = Vec::new();
        for (_, character) in self.characters() {
            character.append_utf16(&mut units);
        }
        PklString::from_utf16(units)
    }
}

/// Linear traversal of authored lines, including an empty final line.
#[derive(Clone, Debug)]
pub struct SourceLines<'a> {
    remaining: SourceSlice<'a>,
    done: bool,
}

impl<'a> Iterator for SourceLines<'a> {
    type Item = SourceSlice<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        let Some((end, character)) = self
            .remaining
            .characters()
            .find(|(_, ch)| matches!(ch, SourceCharacter::Scalar('\r' | '\n')))
        else {
            self.done = true;
            return Some(self.remaining);
        };
        let line = self.remaining.range(..end);
        let after = end + character.len_bytes();
        let after = if character == SourceCharacter::Scalar('\r')
            && self.remaining.as_bytes().get(after) == Some(&b'\n')
        {
            after + 1
        } else {
            after
        };
        self.remaining = self.remaining.range(after..);
        Some(line)
    }
}

impl std::iter::FusedIterator for SourceLines<'_> {}

#[derive(Clone, Debug)]
pub struct SourceCharacters<'a> {
    bytes: &'a [u8],
    offset: usize,
    end: usize,
}

impl Iterator for SourceCharacters<'_> {
    type Item = (usize, SourceCharacter);

    fn next(&mut self) -> Option<Self::Item> {
        if self.offset >= self.end {
            return None;
        }
        let first = *self.bytes.get(self.offset)?;
        let count = if first < 0x80 {
            1
        } else if first < 0xe0 {
            2
        } else if first < 0xf0 {
            3
        } else {
            4
        };
        let mut point = u32::from(
            first
                & match count {
                    1 => 0x7f,
                    2 => 0x1f,
                    3 => 0x0f,
                    _ => 0x07,
                },
        );
        for next in &self.bytes[self.offset + 1..self.offset + count] {
            point = (point << 6) | u32::from(next & 0x3f);
        }
        let character = if (0xd800..=0xdfff).contains(&point) {
            SourceCharacter::Surrogate(point as u16)
        } else {
            SourceCharacter::Scalar(char::from_u32(point).expect("canonical source encoding"))
        };
        let offset = self.offset;
        self.offset += count;
        Some((offset, character))
    }
}

impl DoubleEndedIterator for SourceCharacters<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.offset >= self.end {
            return None;
        }
        let mut start = self.end - 1;
        while self.bytes[start] & 0xc0 == 0x80 {
            start -= 1;
        }
        let mut last = Self {
            bytes: self.bytes,
            offset: start,
            end: self.end,
        };
        let result = last.next();
        self.end = start;
        result
    }
}

impl std::iter::FusedIterator for SourceCharacters<'_> {}

#[derive(Clone, Debug)]
pub struct SourceTextIndex {
    positions: Arc<[Position]>,
}

impl SourceTextIndex {
    pub fn position(&self, byte: usize) -> Option<Position> {
        self.positions
            .binary_search_by_key(&byte, |position| position.byte)
            .ok()
            .map(|index| self.positions[index])
    }

    pub fn byte_from_utf16(&self, utf16: usize) -> Option<usize> {
        self.positions
            .binary_search_by_key(&utf16, |position| position.utf16)
            .ok()
            .map(|index| self.positions[index].byte)
    }

    pub fn utf16_span(&self, span: Span) -> Option<(usize, usize)> {
        let start = self.position(span.start)?.utf16;
        let end = self.position(span.end)?.utf16;
        end.checked_sub(start).map(|length| (start, length))
    }
}

impl From<SourceSlice<'_>> for SourceText {
    fn from(value: SourceSlice<'_>) -> Self {
        value.to_pkl_string().into()
    }
}
impl From<&SourceSlice<'_>> for SourceText {
    fn from(value: &SourceSlice<'_>) -> Self {
        value.to_pkl_string().into()
    }
}
impl From<SourceSlice<'_>> for PklString {
    fn from(value: SourceSlice<'_>) -> Self {
        value.to_pkl_string()
    }
}
impl From<&SourceSlice<'_>> for PklString {
    fn from(value: &SourceSlice<'_>) -> Self {
        value.to_pkl_string()
    }
}
impl<'a> From<&SourceSlice<'a>> for SourceSlice<'a> {
    fn from(value: &SourceSlice<'a>) -> Self {
        *value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn units(value: &PklString) -> Vec<u16> {
        value.encode_utf16().collect()
    }

    #[test]
    fn surrogate_triples_do_not_alias_real_unicode_or_replacement_characters() {
        let original = vec![0xd800, 0xe000, 0xdb80, 0xdc00, 0xfffd, 0xdfff];
        let source = SourceText::from_utf16(original.clone());
        assert_eq!(units(&source.entire().to_pkl_string()), original);
        assert_eq!(
            source.characters().map(|(_, c)| c).collect::<Vec<_>>(),
            vec![
                SourceCharacter::Surrogate(0xd800),
                SourceCharacter::Scalar('\u{e000}'),
                SourceCharacter::Scalar('\u{f0000}'),
                SourceCharacter::Scalar('\u{fffd}'),
                SourceCharacter::Surrogate(0xdfff),
            ]
        );
        assert_eq!(&source.as_bytes()[..3], &[0xed, 0xa0, 0x80]);
        assert_eq!(&source.as_bytes()[3..6], "\u{e000}".as_bytes());
        assert_eq!(source.slice(Span::new(0, 3)).unwrap().as_str(), None);
        assert_eq!(
            source.slice(Span::new(3, 6)).unwrap().as_str(),
            Some("\u{e000}")
        );
    }

    #[test]
    fn adjacent_pairs_and_lone_units_have_distinct_byte_and_utf16_boundaries() {
        let source = SourceText::from_utf16(vec![0xd800, 0xd800, 0xdc00, 0xdc00]);
        let index = source.index();
        assert_eq!(source.len_bytes(), 10);
        assert_eq!(index.byte_from_utf16(0), Some(0));
        assert_eq!(index.byte_from_utf16(1), Some(3));
        assert_eq!(index.byte_from_utf16(2), None);
        assert_eq!(index.byte_from_utf16(3), Some(7));
        assert_eq!(index.byte_from_utf16(4), Some(10));
        for byte in [1, 2, 4, 5, 6, 8, 9] {
            assert_eq!(index.position(byte), None);
            assert!(source.slice(Span::new(0, byte)).is_none());
        }
        assert_eq!(index.utf16_span(Span::new(3, 7)), Some((1, 2)));
        assert_eq!(
            units(&source.slice(Span::new(3, 7)).unwrap().to_pkl_string()),
            vec![0xd800, 0xdc00]
        );
    }

    #[test]
    fn original_lines_and_cr_crlf_positions_preserve_surrogates() {
        let source = SourceText::from_utf16(vec![0x61, 0xd800, 13, 10, 0xdc00, 13, 0x62, 10]);
        let index = source.index();
        assert_eq!(units(&source.line(1).unwrap()), vec![0x61, 0xd800]);
        assert_eq!(units(&source.line(2).unwrap()), vec![0xdc00]);
        assert_eq!(source.line(3).unwrap(), "b");
        assert_eq!(source.line(4).unwrap(), "");
        assert_eq!(source.line(5), None);
        let second = index.position(6).unwrap();
        assert_eq!((second.line, second.column_utf16, second.utf16), (2, 1, 4));
        let third = index.position(10).unwrap();
        assert_eq!((third.line, third.column_utf16, third.utf16), (3, 1, 6));
    }

    #[test]
    fn valid_utf8_keeps_existing_bytes_and_positions() {
        let text = "a😀e\u{301}\r\nz";
        let source = SourceText::from(text);
        assert!(source.wtf8.is_none());
        assert_eq!(source.as_str(), Some(text));
        assert_eq!(source.as_bytes(), text.as_bytes());
        let index = source.index();
        // Hand-authored byte/UTF-16/codepoint/line/column table, independent of
        // SourceMap (which now delegates to this same index implementation).
        let expected = [
            (0, 0, 0, 1, 1),
            (1, 1, 1, 1, 2),
            (5, 3, 2, 1, 4),
            (6, 4, 3, 1, 5),
            (8, 5, 4, 1, 6),
            (9, 6, 5, 1, 7),
            (10, 7, 6, 2, 1),
            (11, 8, 7, 2, 2),
        ];
        for (byte, utf16, codepoint, line, column_utf16) in expected {
            assert_eq!(
                index.position(byte),
                Some(Position {
                    byte,
                    utf16,
                    codepoint,
                    line,
                    column_utf16,
                })
            );
            assert_eq!(index.byte_from_utf16(utf16), Some(byte));
        }
        for byte in [2, 3, 4, 7] {
            assert_eq!(index.position(byte), None);
        }
        assert_eq!(index.byte_from_utf16(2), None);
    }

    #[test]
    fn reversed_outside_and_interior_byte_spans_are_rejected_without_arithmetic_overflow() {
        let source = SourceText::from_utf16(vec![0x61, 0xd800, 0xd83d, 0xde00]);
        let index = source.index();
        assert_eq!(source.len_bytes(), 8);
        for span in [
            Span::new(4, 1),
            Span::new(8, 0),
            Span::new(0, 9),
            Span::new(0, usize::MAX),
            Span::new(2, 4),
            Span::new(4, 7),
        ] {
            assert_eq!(index.utf16_span(span), None, "{span:?}");
            assert_eq!(source.slice(span), None, "{span:?}");
        }
        assert_eq!(index.utf16_span(Span::new(8, 8)), Some((4, 0)));
        assert_eq!(index.utf16_span(Span::new(1, 4)), Some((1, 1)));
        assert_eq!(index.utf16_span(Span::new(4, 8)), Some((2, 2)));
    }

    #[test]
    fn mixed_direction_iteration_and_checked_ranges_preserve_unit_identity() {
        let source = SourceText::from_utf16(vec![0x61, 0xd800, 0xd83d, 0xde00, 0xdc00, 0x7a]);
        let view = source.entire();
        let mut characters = view.characters();
        assert_eq!(
            characters.next_back(),
            Some((11, SourceCharacter::Scalar('z')))
        );
        assert_eq!(characters.next(), Some((0, SourceCharacter::Scalar('a'))));
        assert_eq!(
            characters.next_back(),
            Some((8, SourceCharacter::Surrogate(0xdc00)))
        );
        assert_eq!(
            characters.next(),
            Some((1, SourceCharacter::Surrogate(0xd800)))
        );
        assert_eq!(
            characters.next_back(),
            Some((4, SourceCharacter::Scalar('😀')))
        );
        assert_eq!(characters.next(), None);
        assert_eq!(characters.next_back(), None);
        assert_eq!(view.find("😀"), Some(4));
        assert_eq!(view.find("\u{fffd}"), None);
        assert_eq!(view.get(2..4), None);
        assert_eq!(view.get(..=usize::MAX), None);
        assert_eq!(view.get(13..), None);
        let inner = view.strip_prefix("a").unwrap().strip_suffix("z").unwrap();
        assert_eq!(inner.utf16_len(), 4);
        assert_eq!(
            units(&inner.to_pkl_string()),
            vec![0xd800, 0xd83d, 0xde00, 0xdc00]
        );
    }

    #[test]
    fn actual_java_loaded_sources_retain_all_authored_units() {
        for bytes in [
            include_bytes!("../../tests/fixtures/m2-glob-lossless-source/high.source.utf16be")
                .as_slice(),
            include_bytes!("../../tests/fixtures/m2-glob-lossless-source/low.source.utf16be")
                .as_slice(),
            include_bytes!("../../tests/fixtures/m2-glob-lossless-source/mixed.source.utf16be")
                .as_slice(),
        ] {
            let original = bytes
                .as_chunks::<2>()
                .0
                .iter()
                .map(|b| u16::from_be_bytes([b[0], b[1]]))
                .collect::<Vec<_>>();
            let source = SourceText::from_utf16(original.clone());
            assert_eq!(units(source.original()), original);
            assert_eq!(units(&source.entire().to_pkl_string()), original);
            assert_eq!(
                units(&source.line(1).unwrap()),
                original[..original.len() - 1]
            );
            let surrogate = source
                .characters()
                .find(|(_, c)| matches!(c, SourceCharacter::Surrogate(_)))
                .unwrap();
            assert_eq!(surrogate.1.scalar(), None);
            let index = source.index();
            assert_eq!(
                index
                    .utf16_span(Span::new(surrogate.0, surrogate.0 + 3))
                    .unwrap()
                    .1,
                1
            );
        }
    }
}
