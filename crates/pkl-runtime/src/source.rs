//! Explicit UTF-8, UTF-16, and Unicode scalar coordinates.
//! Scalar sources retain UTF-8 byte spans; SourceText also represents lone surrogates.

mod text;
pub use text::{
    SourceCharacter, SourceCharacters, SourceLines, SourceSlice, SourceText, SourceTextIndex,
};

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
    pub name: crate::value::PklString,
    text: SourceText,
    positions: std::sync::OnceLock<SourceTextIndex>,
}

impl Source {
    #[inline]
    fn positions(&self) -> &SourceTextIndex {
        self.positions.get_or_init(|| self.text.index())
    }

    /// Retained authored UTF-16 and the shared lexer's byte address space.
    pub fn text(&self) -> &SourceText {
        &self.text
    }

    /// Reject continuation bytes; never round a position into another unit.
    pub fn position(&self, byte: usize) -> Option<Position> {
        self.positions().position(byte)
    }

    /// An interior UTF-16 position in a scalar pair has no byte boundary.
    pub fn byte_from_utf16(&self, utf16: usize) -> Option<usize> {
        self.positions().byte_from_utf16(utf16)
    }

    pub fn utf16_span(&self, span: Span) -> Option<(usize, usize)> {
        self.positions().utf16_span(span)
    }

    pub fn slice(&self, span: Span) -> Option<SourceSlice<'_>> {
        self.text.slice(span)
    }

    pub fn line_slice(&self, line_1based: usize) -> Option<SourceSlice<'_>> {
        self.positions().line_slice(line_1based)
    }
}

#[derive(Clone, Debug, Default)]
pub struct SourceMap {
    sources: Vec<Source>,
}

impl SourceMap {
    pub fn add(
        &mut self,
        name: impl Into<crate::value::PklString>,
        text: impl Into<SourceText>,
    ) -> SourceId {
        let text = text.into();
        let id = SourceId(self.sources.len());
        self.sources.push(Source {
            name: name.into(),
            text,
            positions: std::sync::OnceLock::new(),
        });
        id
    }

    pub fn get(&self, id: SourceId) -> Option<&Source> {
        self.sources.get(id.0)
    }
}
