// Copyright (c) 2018, SnakeYAML. Apache-2.0. Rust adaptation: see PROVENANCE.md.
//! Engine ScannerImpl with parseComments=false, the Pkl setting.
use super::diagnostic::ErrorKind;
use super::reader::Reader;
use super::{Mark, ScalarStyle, Text, YamlError};
mod scalar;
mod tags;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Id {
    Alias,
    Anchor,
    BlockEnd,
    BlockEntry,
    BlockMappingStart,
    BlockSequenceStart,
    Directive,
    DocumentEnd,
    DocumentStart,
    FlowEntry,
    FlowMappingEnd,
    FlowMappingStart,
    FlowSequenceEnd,
    FlowSequenceStart,
    Key,
    Scalar,
    StreamEnd,
    StreamStart,
    Tag,
    Value,
}
impl Id {
    pub fn description(self) -> &'static str {
        match self {
            Self::Alias => "<alias>",
            Self::Anchor => "<anchor>",
            Self::BlockEnd => "<block end>",
            Self::BlockEntry => "-",
            Self::BlockMappingStart => "<block mapping start>",
            Self::BlockSequenceStart => "<block sequence start>",
            Self::Directive => "<directive>",
            Self::DocumentEnd => "<document end>",
            Self::DocumentStart => "<document start>",
            Self::FlowEntry => ",",
            Self::FlowMappingEnd => "}",
            Self::FlowMappingStart => "{",
            Self::FlowSequenceEnd => "]",
            Self::FlowSequenceStart => "[",
            Self::Key => "?",
            Self::Scalar => "<scalar>",
            Self::StreamEnd => "<stream end>",
            Self::StreamStart => "<stream start>",
            Self::Tag => "<tag>",
            Self::Value => ":",
        }
    }
}
#[derive(Clone, Debug)]
pub(crate) enum Data {
    None,
    Name(Text),
    Scalar(Text, ScalarStyle),
    Tag(Option<Text>, Text),
    Directive(
        #[allow(
            dead_code,
            reason = "Preserve the original directive name alongside its parsed payload; unknown directives are ignored by the parser."
        )]
        Text,
        Option<Directive>,
    ),
}
#[derive(Clone, Debug)]
pub(crate) enum Directive {
    Yaml(i32, i32),
    Tag(Text, Text),
}
#[derive(Clone, Debug)]
pub(crate) struct Token {
    pub id: Id,
    pub start: Mark,
    pub end: Mark,
    pub data: Data,
}
impl Token {
    pub fn new(id: Id, start: Mark, end: Mark) -> Self {
        Self {
            id,
            start,
            end,
            data: Data::None,
        }
    }
    fn scalar(value: Vec<u16>, style: ScalarStyle, start: Mark, end: Mark) -> Self {
        Self {
            id: Id::Scalar,
            start,
            end,
            data: Data::Scalar(Text::from_units(value), style),
        }
    }
}
#[derive(Clone)]
struct SimpleKey {
    token: usize,
    required: bool,
    index: usize,
    line: usize,
    column: usize,
    mark: Mark,
}
#[derive(Clone)]
pub(crate) struct Scanner {
    reader: Reader,
    tokens: Vec<Token>,
    indents: Vec<i32>,
    keys: Vec<(i32, SimpleKey)>,
    done: bool,
    flow: i32,
    taken: usize,
    indent: i32,
    allow_key: bool,
}
fn blank(c: u32) -> bool {
    matches!(c, 0 | 9 | 10 | 13 | 32)
}
fn line(c: u32) -> bool {
    matches!(c, 10 | 13)
}
fn nul_line(c: u32) -> bool {
    c == 0 || line(c)
}
fn contains(chars: &str, c: u32) -> bool {
    chars.chars().any(|x| u32::from(x) == c)
}
fn point(c: u32) -> String {
    char::from_u32(c)
        .expect("validated reader code point")
        .to_string()
}
fn text_string(t: &Text) -> String {
    String::from_utf16(t.units()).expect("raw source token is valid Unicode")
}
impl Scanner {
    pub fn new(input: Text, label: Text) -> Self {
        let reader = Reader::new(input, label);
        let mark = reader.mark();
        Self {
            reader,
            tokens: vec![Token::new(Id::StreamStart, mark.clone(), mark)],
            indents: Vec::new(),
            keys: Vec::new(),
            done: false,
            flow: 0,
            taken: 0,
            indent: -1,
            allow_key: true,
        }
    }
    pub fn reset_document_index(&mut self) {
        self.reader.document_index = 0;
    }
    pub fn peek(&mut self) -> Result<Token, YamlError> {
        while self.need_more()? {
            self.fetch()?;
        }
        self.tokens
            .first()
            .cloned()
            .ok_or_else(|| YamlError::plain(ErrorKind::Engine, "No more tokens"))
    }
    /// Cooperative step-budgeted peek: yields (`Ok(None)`) when `budget` reaches 0
    /// while skipping whitespace/comments or fetching lookahead tokens.
    pub fn step_peek(&mut self, budget: &mut usize) -> Result<Option<Token>, YamlError> {
        while self.need_more()? {
            if *budget == 0 {
                return Ok(None);
            }
            if !self.step_fetch(budget)? {
                return Ok(None);
            }
        }
        self.tokens
            .first()
            .cloned()
            .map(Some)
            .ok_or_else(|| YamlError::plain(ErrorKind::Engine, "No more tokens"))
    }
    pub fn next(&mut self) -> Result<Token, YamlError> {
        let t = self.peek()?;
        self.taken += 1;
        self.tokens.remove(0);
        Ok(t)
    }
    pub fn check(&mut self, ids: &[Id]) -> Result<bool, YamlError> {
        Ok(ids.contains(&self.peek()?.id))
    }
    fn need_more(&mut self) -> Result<bool, YamlError> {
        if self.done {
            return Ok(false);
        }
        if self.tokens.is_empty() {
            return Ok(true);
        }
        self.stale_keys()?;
        Ok(self
            .keys
            .first()
            .is_some_and(|(_, k)| k.token == self.taken))
    }
    fn error(
        &self,
        context: Option<&str>,
        start: Option<Mark>,
        problem: impl Into<Text>,
    ) -> YamlError {
        YamlError::marked(
            ErrorKind::Scanner,
            context,
            start,
            problem,
            Some(self.reader.mark()),
        )
    }
    fn stale_keys(&mut self) -> Result<(), YamlError> {
        let mut i = 0;
        while i < self.keys.len() {
            let k = &self.keys[i].1;
            if k.line != self.reader.line || self.reader.index - k.index > 1024 {
                if k.required {
                    return Err(self.error(
                        Some("while scanning a simple key"),
                        Some(k.mark.clone()),
                        "could not find expected ':'",
                    ));
                }
                self.keys.remove(i);
            } else {
                i += 1;
            }
        }
        Ok(())
    }
    fn take_key(&mut self) -> Option<SimpleKey> {
        self.keys
            .iter()
            .position(|(f, _)| *f == self.flow)
            .map(|i| self.keys.remove(i).1)
    }
    fn remove_key(&mut self) -> Result<(), YamlError> {
        if let Some(k) = self.take_key()
            && k.required
        {
            return Err(self.error(
                Some("while scanning a simple key"),
                Some(k.mark),
                "could not find expected ':'",
            ));
        }
        Ok(())
    }
    fn save_key(&mut self) -> Result<(), YamlError> {
        let required = self.flow == 0 && self.indent == self.reader.column as i32;
        if !self.allow_key && required {
            return Err(YamlError::plain(
                ErrorKind::Engine,
                "A simple key is required only if it is the first token in the current line",
            ));
        }
        if self.allow_key {
            self.remove_key()?;
            self.keys.push((
                self.flow,
                SimpleKey {
                    token: self.taken + self.tokens.len(),
                    required,
                    index: self.reader.index,
                    line: self.reader.line,
                    column: self.reader.column,
                    mark: self.reader.mark(),
                },
            ));
        }
        Ok(())
    }
    fn unwind(&mut self, col: i32) {
        if self.flow != 0 {
            return;
        }
        while self.indent > col {
            self.indent = self.indents.pop().expect("indent stack");
            let m = self.reader.mark();
            self.tokens.push(Token::new(Id::BlockEnd, m.clone(), m));
        }
    }
    fn add_indent(&mut self, col: usize) -> bool {
        if self.indent < (col as i32) {
            self.indents.push(self.indent);
            self.indent = col as i32;
            true
        } else {
            false
        }
    }
    fn simple(&mut self, id: Id, count: usize) -> Result<(), YamlError> {
        let start = self.reader.mark();
        self.reader.forward(count)?;
        self.tokens.push(Token::new(id, start, self.reader.mark()));
        Ok(())
    }
    fn fetch(&mut self) -> Result<(), YamlError> {
        let mut unlimited = usize::MAX;
        let done = self.step_fetch(&mut unlimited)?;
        debug_assert!(done);
        Ok(())
    }
    fn step_fetch(&mut self, budget: &mut usize) -> Result<bool, YamlError> {
        if self.reader.document_index > 3 * 1024 * 1024 {
            return Err(YamlError::plain(
                ErrorKind::Engine,
                "The incoming YAML document exceeds the limit: 3145728 code points.",
            ));
        }
        if !self.step_skip(budget)? {
            return Ok(false);
        }
        if *budget == 0 {
            return Ok(false);
        }
        *budget -= 1;
        self.fetch_after_skip()?;
        Ok(true)
    }
    fn fetch_after_skip(&mut self) -> Result<(), YamlError> {
        self.stale_keys()?;
        self.unwind(self.reader.column as i32);
        let c = self.reader.peek(0)?;
        match c {
            0 => {
                self.unwind(-1);
                self.remove_key()?;
                self.allow_key = false;
                self.keys.clear();
                self.simple(Id::StreamEnd, 0)?;
                self.done = true;
                return Ok(());
            }
            37 if self.reader.column == 0 => {
                self.unwind(-1);
                self.remove_key()?;
                self.allow_key = false;
                let t = self.directive()?;
                self.tokens.push(t);
                return Ok(());
            }
            45 if self.document("---")? => {
                self.document_indicator(Id::DocumentStart)?;
                return Ok(());
            }
            46 if self.document("...")? => {
                self.document_indicator(Id::DocumentEnd)?;
                return Ok(());
            }
            91 | 123 => {
                self.save_key()?;
                self.flow += 1;
                self.allow_key = true;
                self.simple(
                    if c == 91 {
                        Id::FlowSequenceStart
                    } else {
                        Id::FlowMappingStart
                    },
                    1,
                )?;
                return Ok(());
            }
            93 | 125 => {
                self.remove_key()?;
                self.flow -= 1;
                self.allow_key = false;
                self.simple(
                    if c == 93 {
                        Id::FlowSequenceEnd
                    } else {
                        Id::FlowMappingEnd
                    },
                    1,
                )?;
                return Ok(());
            }
            44 => {
                self.allow_key = true;
                self.remove_key()?;
                self.simple(Id::FlowEntry, 1)?;
                return Ok(());
            }
            45 if blank(self.reader.peek(1)?) => {
                if self.flow == 0 {
                    if !self.allow_key {
                        return Err(self.error(
                            Some(""),
                            None,
                            "sequence entries are not allowed here",
                        ));
                    }
                    if self.add_indent(self.reader.column) {
                        let m = self.reader.mark();
                        self.tokens
                            .push(Token::new(Id::BlockSequenceStart, m.clone(), m));
                    }
                }
                self.allow_key = true;
                self.remove_key()?;
                self.simple(Id::BlockEntry, 1)?;
                return Ok(());
            }
            63 if blank(self.reader.peek(1)?) => {
                if self.flow == 0 {
                    if !self.allow_key {
                        return Err(self.error(None, None, "mapping keys are not allowed here"));
                    }
                    if self.add_indent(self.reader.column) {
                        let m = self.reader.mark();
                        self.tokens
                            .push(Token::new(Id::BlockMappingStart, m.clone(), m));
                    }
                }
                self.allow_key = self.flow == 0;
                self.remove_key()?;
                self.simple(Id::Key, 1)?;
                return Ok(());
            }
            58 if self.flow != 0 || blank(self.reader.peek(1)?) => {
                self.value()?;
                return Ok(());
            }
            42 | 38 => {
                self.save_key()?;
                self.allow_key = false;
                let t = self.anchor(c == 38)?;
                self.tokens.push(t);
                return Ok(());
            }
            33 => {
                self.save_key()?;
                self.allow_key = false;
                let t = self.tag()?;
                self.tokens.push(t);
                return Ok(());
            }
            124 | 62 if self.flow == 0 => {
                self.allow_key = true;
                self.remove_key()?;
                let t = self.block_scalar(if c == 124 {
                    ScalarStyle::Literal
                } else {
                    ScalarStyle::Folded
                })?;
                self.tokens.push(t);
                return Ok(());
            }
            39 | 34 => {
                self.save_key()?;
                self.allow_key = false;
                let t = self.flow_scalar(if c == 39 {
                    ScalarStyle::SingleQuoted
                } else {
                    ScalarStyle::DoubleQuoted
                })?;
                self.tokens.push(t);
                return Ok(());
            }
            _ => {}
        }
        if self.check_plain()? {
            self.save_key()?;
            self.allow_key = false;
            let t = self.plain()?;
            self.tokens.push(t);
            return Ok(());
        }
        let rep = match c {
            9 => "\\t(TAB)".into(),
            0 => "\\0".into(),
            7 => "\\a".into(),
            8 => "\\b".into(),
            10 => "\\n".into(),
            11 => "\\v".into(),
            12 => "\\f".into(),
            13 => "\\r".into(),
            27 => "\\e".into(),
            92 => "\\\\".into(),
            0x85 => "\\N".into(),
            0xa0 => "\\_".into(),
            _ => point(c),
        };
        Err(self.error(Some("while scanning for the next token"),None,format!("found character '{rep}' that cannot start any token. (Do not use {rep} for indentation)")))
    }
    fn document(&mut self, s: &str) -> Result<bool, YamlError> {
        Ok(self.reader.column == 0
            && self.reader.prefix(3)?.is_ascii(s)
            && blank(self.reader.peek(3)?))
    }
    fn document_indicator(&mut self, id: Id) -> Result<(), YamlError> {
        self.unwind(-1);
        self.remove_key()?;
        self.allow_key = false;
        self.simple(id, 3)
    }
    fn value(&mut self) -> Result<(), YamlError> {
        if let Some(k) = self.take_key() {
            let pos = k.token - self.taken;
            self.tokens
                .insert(pos, Token::new(Id::Key, k.mark.clone(), k.mark.clone()));
            if self.flow == 0 && self.add_indent(k.column) {
                self.tokens.insert(
                    pos,
                    Token::new(Id::BlockMappingStart, k.mark.clone(), k.mark),
                );
            }
            self.allow_key = false;
        } else {
            if self.flow == 0 && !self.allow_key {
                return Err(self.error(None, None, "mapping values are not allowed here"));
            }
            if self.flow == 0 && self.add_indent(self.reader.column) {
                let m = self.reader.mark();
                self.tokens
                    .push(Token::new(Id::BlockMappingStart, m.clone(), m));
            }
            self.allow_key = self.flow == 0;
            self.remove_key()?;
        }
        self.simple(Id::Value, 1)
    }
    fn check_plain(&mut self) -> Result<bool, YamlError> {
        let c = self.reader.peek(0)?;
        if !blank(c) && !contains("-?:,[]{}#&*!|>'\"%@`", c) {
            return Ok(true);
        }
        let n = self.reader.peek(1)?;
        Ok(if self.flow == 0 {
            !blank(n) && contains("-?:", c)
        } else {
            !blank(n) && !contains(",]", n) && contains("-?", c)
        })
    }
    fn step_skip(&mut self, budget: &mut usize) -> Result<bool, YamlError> {
        if self.reader.index == 0 && self.reader.peek(0)? == 0xfeff {
            self.reader.forward(1)?;
        }
        loop {
            if *budget == 0 {
                return Ok(false);
            }
            *budget -= 1;
            let mut n = 0;
            while self.reader.peek(n)? == 32 {
                n += 1;
                if n >= 256 {
                    break;
                }
            }
            if n >= 256 {
                self.reader.forward(n)?;
                continue;
            }
            if self.reader.peek(n)? == 9 && self.flow != 0 {
                n += 1;
            }
            if n > 0 {
                self.reader.forward(n)?;
            }
            if self.reader.peek(0)? == 35 {
                let mut n = 0;
                while !nul_line(self.reader.peek(n)?) {
                    n += 1;
                }
                self.reader.forward(n)?;
            }
            if self.line_break()?.is_some() {
                if self.flow == 0 {
                    self.allow_key = true;
                }
            } else {
                return Ok(true);
            }
        }
    }
    fn line_break(&mut self) -> Result<Option<u16>, YamlError> {
        let c = self.reader.peek(0)?;
        if c == 13 || c == 10 || c == 0x85 {
            let n = if c == 13 && self.reader.peek(1)? == 10 {
                2
            } else {
                1
            };
            self.reader.forward(n)?;
            Ok(Some(10))
        } else {
            Ok(None)
        }
    }
}
