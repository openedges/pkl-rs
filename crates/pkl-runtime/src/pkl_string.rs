//! Pkl strings retain Java UTF-16 code units, including unmatched surrogates.
//!
//! Well-formed strings use UTF-8 storage. The UTF-16 representation is canonical:
//! it is used only when at least one surrogate is unmatched. Encoding for a Java
//! UTF-8 writer is an explicit boundary operation, never a value conversion.
use std::{cmp::Ordering, fmt, sync::Arc};

#[derive(Clone, PartialEq, Eq)]
pub struct PklString(Repr);

#[derive(Clone, PartialEq, Eq)]
enum Repr {
    Utf8(Arc<str>),
    Utf16(Arc<[u16]>),
}

impl PklString {
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
            Err(_) => Self(Repr::Utf16(units.into())),
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

    pub fn is_empty(&self) -> bool {
        match &self.0 {
            Repr::Utf8(text) => text.is_empty(),
            Repr::Utf16(units) => units.is_empty(),
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
        Self(Repr::Utf8(text.into()))
    }
}

impl From<String> for PklString {
    fn from(text: String) -> Self {
        Self(Repr::Utf8(text.into()))
    }
}

impl From<Arc<str>> for PklString {
    fn from(text: Arc<str>) -> Self {
        Self(Repr::Utf8(text))
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

// Formatting is used by diagnostic text builders. Runtime concatenation and
// toString use the lossless operations above, never formatting.
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
