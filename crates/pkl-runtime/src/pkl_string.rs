//! Pkl strings retain Java UTF-16 code units, including unmatched surrogates.
//!
//! Well-formed strings use UTF-8 storage. The UTF-16 representation is canonical:
//! it is used only when at least one surrogate is unmatched. Encoding for a Java
//! UTF-8 writer is an explicit boundary operation, never a value conversion.
use std::{cmp::Ordering, fmt, sync::Arc};

#[derive(Clone)]
pub struct PklString(Repr, crate::value_identity::Token);

impl PartialEq for PklString {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}
impl Eq for PklString {}

#[derive(Clone, PartialEq, Eq)]
enum Repr {
    Utf8(Arc<str>),
    Utf16(Arc<[u16]>),
}

impl PklString {
    /// AstBuilder emits Java's canonical empty literal for an empty source
    /// string. Host-provided empty Strings keep their own allocation identity.
    pub(crate) fn source_literal(&self) -> Self {
        if self.is_empty() {
            Self(
                Repr::Utf8("".into()),
                crate::value_identity::Token::empty_string(),
            )
        } else {
            self.clone()
        }
    }
    pub(crate) fn reference(&self) -> crate::value_identity::Token {
        self.1
    }
    pub(crate) fn join(values: &[Self], separator: &str) -> Self {
        let mut builder = PklStringBuilder::default();
        for (index, value) in values.iter().enumerate() {
            if index > 0 {
                builder.push_str(separator);
            }
            builder.push_value(value);
        }
        builder.finish()
    }

    pub fn from_utf16(units: impl Into<Vec<u16>>) -> Self {
        let units = units.into();
        match String::from_utf16(&units) {
            Ok(text) => text.into(),
            Err(_) => Self(
                Repr::Utf16(units.into()),
                crate::value_identity::Token::fresh(),
            ),
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match &self.0 {
            Repr::Utf8(text) => Some(text),
            Repr::Utf16(_) => None,
        }
    }

    pub fn encode_utf16(&self) -> impl Iterator<Item = u16> + Clone + '_ {
        match &self.0 {
            Repr::Utf8(text) => Units::Utf8(text.encode_utf16()),
            Repr::Utf16(units) => Units::Utf16(units.iter().copied()),
        }
    }

    pub fn code_point_len(&self) -> usize {
        match &self.0 {
            Repr::Utf8(text) => text.chars().count(),
            Repr::Utf16(units) => char::decode_utf16(units.iter().copied()).count(),
        }
    }

    /// Select one Java code point, retaining an unmatched surrogate as a unit.
    pub(crate) fn code_point_at(&self, index: usize) -> Option<Self> {
        match &self.0 {
            Repr::Utf8(text) => text
                .chars()
                .nth(index)
                .map(|point| point.to_string().into()),
            Repr::Utf16(units) => {
                char::decode_utf16(units.iter().copied())
                    .nth(index)
                    .map(|point| match point {
                        Ok(point) => point.to_string().into(),
                        Err(point) => Self::from_utf16(vec![point.unpaired_surrogate()]),
                    })
            }
        }
    }

    /// Java String.substring preserves the receiver for the full interval and
    /// returns the canonical empty literal for an empty proper substring.
    pub(crate) fn drop_last_code_points(&self, mut count: u64) -> Self {
        if count == 0 {
            return self.clone();
        }
        let mut units = self.encode_utf16().collect::<Vec<_>>();
        let mut end = units.len();
        while end > 0 && count > 0 {
            end -= 1;
            if (0xdc00..=0xdfff).contains(&units[end])
                && end > 0
                && (0xd800..=0xdbff).contains(&units[end - 1])
            {
                end -= 1;
            }
            count -= 1;
        }
        if end == 0 {
            return Self(
                Repr::Utf8("".into()),
                crate::value_identity::Token::empty_string(),
            );
        }
        units.truncate(end);
        Self::from_utf16(units)
    }

    pub fn is_empty(&self) -> bool {
        match &self.0 {
            Repr::Utf8(text) => text.is_empty(),
            Repr::Utf16(units) => units.is_empty(),
        }
    }

    /// Pinned StringUtils.isBlank tests Unicode White_Space on Java char units.
    /// Keep its exact 25-unit set independent of Rust and JDK Unicode tables.
    pub(crate) fn is_blank(&self) -> bool {
        self.encode_utf16().all(|unit| {
            matches!(
                unit,
                0x0009..=0x000d
                    | 0x0020
                    | 0x0085
                    | 0x00a0
                    | 0x1680
                    | 0x2000..=0x200a
                    | 0x2028
                    | 0x2029
                    | 0x202f
                    | 0x205f
                    | 0x3000
            )
        })
    }

    /// Match StringBuilder.reverse: reverse UTF-16 units, then restore each
    /// reversed surrogate pair. Originally unmatched low/high surrogates can
    /// become a new pair, so reversing twice need not recover malformed input.
    pub(crate) fn reversed(&self) -> Self {
        let mut units = self.encode_utf16().collect::<Vec<_>>();
        units.reverse();
        let mut index = 0;
        while index + 1 < units.len() {
            if (0xdc00..=0xdfff).contains(&units[index])
                && (0xd800..=0xdbff).contains(&units[index + 1])
            {
                units.swap(index, index + 1);
                index += 2;
            } else {
                index += 1;
            }
        }
        let result = Self::from_utf16(units);
        if result.is_empty() {
            result.source_literal()
        } else {
            result
        }
    }

    pub fn concat(&self, other: &Self) -> Self {
        let mut builder = PklStringBuilder::default();
        builder.push_value(self);
        builder.push_value(other);
        builder.finish()
    }

    /// The value renderer's quoted form, still a lossless language string.
    pub(crate) fn quoted(&self) -> Self {
        let mut units = vec![u16::from(b'"')];
        for unit in self.encode_utf16() {
            let escaped = match unit {
                10 => Some(b'n'),
                13 => Some(b'r'),
                9 => Some(b't'),
                92 => Some(b'\\'),
                34 => Some(b'"'),
                _ => None,
            };
            if let Some(escaped) = escaped {
                units.extend([u16::from(b'\\'), u16::from(escaped)]);
            } else {
                units.push(unit);
            }
        }
        units.push(u16::from(b'"'));
        Self::from_utf16(units)
    }

    /// Match Java's UTF-8 writer, which replaces each unmatched surrogate with
    /// ASCII `?`. Call only when leaving the lossless language-value boundary.
    pub fn to_java_utf8(&self) -> String {
        match &self.0 {
            Repr::Utf8(text) => text.to_string(),
            Repr::Utf16(units) => char::decode_utf16(units.iter().copied())
                .map(|unit| unit.unwrap_or('?'))
                .collect(),
        }
    }
}

impl Default for PklString {
    fn default() -> Self {
        "".into()
    }
}

impl From<&str> for PklString {
    fn from(text: &str) -> Self {
        Self(
            Repr::Utf8(text.into()),
            crate::value_identity::Token::fresh(),
        )
    }
}

impl From<String> for PklString {
    fn from(text: String) -> Self {
        Self(
            Repr::Utf8(text.into()),
            crate::value_identity::Token::fresh(),
        )
    }
}

impl From<Arc<str>> for PklString {
    fn from(text: Arc<str>) -> Self {
        Self(Repr::Utf8(text), crate::value_identity::Token::fresh())
    }
}

impl Ord for PklString {
    fn cmp(&self, other: &Self) -> Ordering {
        self.encode_utf16().cmp(other.encode_utf16())
    }
}

impl PartialOrd for PklString {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

// Display is an explicit Java UTF-8 convenience boundary. Canonical diagnostic
// and language-value composition uses PklStringBuilder::push_value instead.
impl fmt::Display for PklString {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        output.write_str(&self.to_java_utf8())
    }
}

impl fmt::Debug for PklString {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(text) = self.as_str() {
            return fmt::Debug::fmt(text, output);
        }
        output.write_str("\"")?;
        for point in char::decode_utf16(self.encode_utf16()) {
            match point {
                Ok('\'') => output.write_str("'")?,
                Ok(ch) => write!(output, "{}", ch.escape_debug())?,
                Err(unit) => write!(output, "\\u{{{:x}}}", unit.unpaired_surrogate())?,
            }
        }
        output.write_str("\"")
    }
}

#[derive(Clone)]
enum Units<'a> {
    Utf8(std::str::EncodeUtf16<'a>),
    Utf16(std::iter::Copied<std::slice::Iter<'a, u16>>),
}

impl Iterator for Units<'_> {
    type Item = u16;

    fn next(&mut self) -> Option<u16> {
        match self {
            Self::Utf8(units) => units.next(),
            Self::Utf16(units) => units.next(),
        }
    }
}

#[derive(Default)]
pub(crate) struct PklStringBuilder {
    text: String,
    units: Option<Vec<u16>>,
}

impl PklStringBuilder {
    pub(crate) fn is_empty(&self) -> bool {
        self.units
            .as_ref()
            .map_or(self.text.is_empty(), Vec::is_empty)
    }

    pub(crate) fn push_str(&mut self, text: &str) {
        match &mut self.units {
            Some(units) => units.extend(text.encode_utf16()),
            None => self.text.push_str(text),
        }
    }

    pub(crate) fn push(&mut self, character: char) {
        self.push_str(character.encode_utf8(&mut [0; 4]));
    }

    pub(crate) fn push_value(&mut self, value: &PklString) {
        if let Some(text) = value.as_str() {
            self.push_str(text);
        } else {
            let units = self.units.get_or_insert_with(|| {
                let units = self.text.encode_utf16().collect();
                self.text.clear();
                units
            });
            units.extend(value.encode_utf16());
        }
    }

    pub(crate) fn finish(self) -> PklString {
        match self.units {
            Some(units) => PklString::from_utf16(units),
            None => self.text.into(),
        }
    }
}

// Formatting fixed UTF-8 labels and numbers is safe. Language values must use
// push_value so formatting never invokes their byte-oriented Display boundary.
impl fmt::Write for PklStringBuilder {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        self.push_str(text);
        Ok(())
    }
}

impl PartialEq<str> for PklString {
    fn eq(&self, other: &str) -> bool {
        self.encode_utf16().eq(other.encode_utf16())
    }
}
impl PartialEq<&str> for PklString {
    fn eq(&self, other: &&str) -> bool {
        self == *other
    }
}
impl PartialEq<String> for PklString {
    fn eq(&self, other: &String) -> bool {
        self == other.as_str()
    }
}
impl PklString {
    pub fn starts_with(&self, prefix: &str) -> bool {
        let mut units = self.encode_utf16();
        prefix.encode_utf16().all(|unit| units.next() == Some(unit))
    }
    pub fn ends_with(&self, suffix: &str) -> bool {
        self.encode_utf16()
            .collect::<Vec<_>>()
            .ends_with(&suffix.encode_utf16().collect::<Vec<_>>())
    }
    pub fn contains(&self, needle: &str) -> bool {
        let needle: Vec<_> = needle.encode_utf16().collect();
        needle.is_empty()
            || self
                .encode_utf16()
                .collect::<Vec<_>>()
                .windows(needle.len())
                .any(|window| window == needle)
    }
    pub(crate) fn push_str(&mut self, suffix: &str) {
        *self = self.concat(&suffix.into());
    }
}

#[cfg(test)]
mod string_properties_tests;
