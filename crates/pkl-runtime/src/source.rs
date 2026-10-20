//! Explicit UTF-8, UTF-16, and Unicode scalar coordinates. Spans use UTF-8 bytes.

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct SourceId(pub usize);

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Position {
    pub byte: usize,
    pub utf16: usize,
    pub codepoint: usize,
    /// One-based line and UTF-16 column, like the pinned Java lexer.
    pub line: usize,
    pub column_utf16: usize,
}

#[derive(Clone, Debug)]
pub struct Source {
    pub name: String,
    text: String,
    positions: Vec<Position>,
}

impl Source {
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Rejects a UTF-8 continuation byte instead of rounding its position.
    pub fn position(&self, byte: usize) -> Option<Position> {
        self.positions
            .binary_search_by_key(&byte, |position| position.byte)
            .ok()
            .map(|index| self.positions[index])
    }

    /// A UTF-16 offset inside a surrogate pair has no UTF-8 scalar boundary.
    pub fn byte_from_utf16(&self, utf16: usize) -> Option<usize> {
        self.positions
            .binary_search_by_key(&utf16, |position| position.utf16)
            .ok()
            .map(|index| self.positions[index].byte)
    }

    pub fn utf16_span(&self, span: Span) -> Option<(usize, usize)> {
        let start = self.position(span.start)?.utf16;
        let end = self.position(span.end)?.utf16;
        (end >= start).then_some((start, end - start))
    }

    pub fn slice(&self, span: Span) -> Option<&str> {
        self.text.get(span.start..span.end)
    }
}

#[derive(Clone, Debug, Default)]
pub struct SourceMap {
    sources: Vec<Source>,
}

impl SourceMap {
    pub fn add(&mut self, name: impl Into<String>, text: impl Into<String>) -> SourceId {
        let text = text.into();
        let mut position = Position {
            byte: 0,
            utf16: 0,
            codepoint: 0,
            line: 1,
            column_utf16: 1,
        };
        let mut positions = vec![position];
        for ch in text.chars() {
            position.byte += ch.len_utf8();
            position.utf16 += ch.len_utf16();
            position.codepoint += 1;
            if ch == '\n' || (ch == '\r' && text.as_bytes().get(position.byte) != Some(&b'\n')) {
                position.line += 1;
                position.column_utf16 = 1;
            } else {
                position.column_utf16 += ch.len_utf16();
            }
            positions.push(position);
        }
        let id = SourceId(self.sources.len());
        self.sources.push(Source {
            name: name.into(),
            text,
            positions,
        });
        id
    }

    pub fn get(&self, id: SourceId) -> Option<&Source> {
        self.sources.get(id.0)
    }
}
