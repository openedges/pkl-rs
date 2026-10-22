//! Eager string tokens with heap-backed nested string/interpolation scanning.
use std::collections::BTreeMap;
use std::sync::Arc;

use super::{Diagnostic, DiagnosticKind, Kind, Lexer, Parser, Span, StringPart, multiline_parts};

// EOF validation parses an interpolation to preserve its first syntax error.
// Replaying already failed child strings prevents that validation from entering
// the same nested scanner again. The origin source end retains EOF provenance;
// a fully consumed failed token can also be replayed in a shorter visible prefix.
pub(super) type StringErrors = Arc<BTreeMap<(usize, usize), (usize, Diagnostic)>>;

struct StringScan {
    start: usize,
    hashes: usize,
    multiline: bool,
    delimiter: String,
    escape: String,
    parts: Vec<StringPart>,
    text: String,
    interpolation: Option<InterpolationScan>,
}

struct InterpolationScan {
    start: usize,
    begin: usize,
    nesting: usize,
}

enum StringFlow {
    Continue,
    Start(usize, char),
    Done,
    Unterminated(Span),
}

impl Lexer<'_> {
    pub(super) fn string(&mut self, start: usize, first: char) -> Result<Kind, Diagnostic> {
        let mut frames = vec![self.start_string(start, first)?];
        loop {
            let frame = frames.last_mut().expect("active string scan");
            let result = match self.string_step(frame) {
                Ok(StringFlow::Continue) => continue,
                Ok(StringFlow::Start(start, first)) => {
                    if frames.len() >= 32 {
                        // Resource guards must not be swallowed by the lexical
                        // recovery used to defer ordinary expression errors.
                        return Err(Diagnostic::new(
                            DiagnosticKind::Unsupported,
                            Span::new(start, self.cursor),
                            "String interpolation nesting limit exceeded.",
                        ));
                    }
                    match self.start_string(start, first) {
                        Ok(frame) => frames.push(frame),
                        Err(error) => self.remember_string_error(start, error),
                    }
                    continue;
                }
                Ok(StringFlow::Done) => {
                    let frame = frames.pop().expect("completed string scan");
                    let start = frame.start;
                    let result = self.finish_string(frame);
                    if frames.is_empty() {
                        return result;
                    }
                    if let Err(error) = result {
                        self.remember_string_error(start, error);
                    }
                    continue;
                }
                Ok(StringFlow::Unterminated(range)) => {
                    let result = self
                        .validate_interpolations(expression_ranges(&frame.parts))
                        .and_then(|()| {
                            Parser::interpolation_with_errors(
                                self.source,
                                range,
                                self.string_errors.clone(),
                            )
                        })
                        .and_then(|mut parser| parser.expression_expected(0, Some(')')));
                    Err(result.err().unwrap_or_else(|| {
                        Diagnostic::new(
                            DiagnosticKind::Syntax,
                            range,
                            "Missing `)` delimiter in interpolation.",
                        )
                    }))
                }
                Err(error) => Err(self
                    .validate_interpolations(expression_ranges(&frame.parts))
                    .err()
                    .unwrap_or(error)),
            };
            let frame = frames.pop().expect("failed string scan");
            if frames.is_empty() {
                return result;
            }
            self.remember_string_error(frame.start, result.expect_err("failed string scan"));
        }
    }

    fn validate_interpolations(
        &self,
        ranges: impl Iterator<Item = Span>,
    ) -> Result<(), Diagnostic> {
        for range in ranges {
            let mut parser =
                Parser::interpolation_with_errors(self.source, range, self.string_errors.clone())?;
            parser.expression_expected(0, Some(')'))?;
            parser.expect(')')?;
        }
        Ok(())
    }

    pub(super) fn cached_string_error(&self, start: usize) -> Option<(usize, Diagnostic)> {
        let visible_end = self.source.len();
        self.string_errors
            .range((start, visible_end)..=(start, usize::MAX))
            .find_map(|(_, (end, error))| {
                // Never replay a truncated-EOF failure into a longer source.
                // Shortening is safe only when the whole failed token and its
                // diagnostic locations remain visible. This includes nested
                // multiline failures validated inside an interpolation range.
                (*end <= visible_end
                    && error.span.end <= visible_end
                    && error
                        .frames
                        .iter()
                        .all(|frame| frame.span.end <= visible_end))
                .then(|| (*end, error.clone()))
            })
    }

    fn remember_string_error(&mut self, start: usize, error: Diagnostic) {
        Arc::make_mut(&mut self.string_errors)
            .insert((start, self.source.len()), (self.cursor, error));
    }

    fn start_string(&mut self, start: usize, first: char) -> Result<StringScan, Diagnostic> {
        let mut hashes = usize::from(first == '#');
        if first == '#' {
            while self.peek() == Some('#') {
                hashes += 1;
                self.advance();
            }
            if self.advance() != Some('"') {
                return Err(Diagnostic::new(
                    DiagnosticKind::Syntax,
                    Span::new(start, self.cursor),
                    "Expected string after `#`.",
                ));
            }
        }
        let multiline = self.source[self.cursor..].starts_with("\"\"");
        if multiline {
            self.cursor += 2;
        }
        Ok(StringScan {
            start,
            hashes,
            multiline,
            delimiter: format!(
                "{}{}",
                if multiline { "\"\"\"" } else { "\"" },
                "#".repeat(hashes)
            ),
            escape: format!("\\{}", "#".repeat(hashes)),
            parts: Vec::new(),
            text: String::new(),
            interpolation: None,
        })
    }

    fn string_step(&mut self, frame: &mut StringScan) -> Result<StringFlow, Diagnostic> {
        if let Some(interpolation) = &mut frame.interpolation {
            // Scan delimiters without letting a later lexical failure supersede
            // an earlier expression syntax error. Captured ranges are parsed in
            // order; failed nested strings are replayed with their exact error.
            let token = match self.next_token(true) {
                Ok(token) => token,
                Err(_) => return Ok(StringFlow::Continue),
            };
            match token.kind {
                Kind::StringStart(first) => return Ok(StringFlow::Start(token.span.start, first)),
                Kind::Symbol('(') => interpolation.nesting += 1,
                Kind::Symbol(')') => {
                    interpolation.nesting -= 1;
                    if interpolation.nesting == 0 {
                        frame.parts.push(StringPart::Expression(
                            Span::new(interpolation.begin, token.span.start),
                            Span::new(interpolation.start, token.span.end),
                        ));
                        frame.interpolation = None;
                    }
                }
                Kind::Eof => {
                    return Ok(StringFlow::Unterminated(Span::new(
                        interpolation.begin,
                        self.cursor,
                    )));
                }
                _ => {}
            }
            return Ok(StringFlow::Continue);
        }
        if self.source[self.cursor..].starts_with(&frame.delimiter) {
            self.cursor += frame.delimiter.len();
            if !frame.text.is_empty() {
                frame
                    .parts
                    .push(StringPart::Text(std::mem::take(&mut frame.text)));
            }
            return Ok(StringFlow::Done);
        }
        let position = self.cursor;
        let Some(ch) = self.advance() else {
            return Err(Diagnostic::new(
                DiagnosticKind::Syntax,
                Span::new(frame.start, self.cursor),
                format!("Missing `{}` delimiter.", frame.delimiter),
            ));
        };
        if ch == '\n' || ch == '\r' {
            if !frame.multiline {
                return Err(Diagnostic::new(
                    DiagnosticKind::Syntax,
                    Span::new(position, self.cursor),
                    "String content must remain on one line.",
                ));
            }
            if ch == '\r' && self.peek() == Some('\n') {
                self.advance();
            }
            if !frame.text.is_empty() {
                frame
                    .parts
                    .push(StringPart::Text(std::mem::take(&mut frame.text)));
            }
            frame.parts.push(StringPart::Newline);
        } else if ch == '\\'
            && self.source[position..].starts_with(&frame.escape)
            && self.source[position + 1..]
                .chars()
                .take_while(|ch| *ch == '#')
                .count()
                == frame.hashes
        {
            self.cursor = position + frame.escape.len();
            if !frame.text.is_empty() {
                frame
                    .parts
                    .push(StringPart::Text(std::mem::take(&mut frame.text)));
            }
            let escaped = self.advance().ok_or_else(|| {
                Diagnostic::new(
                    DiagnosticKind::Syntax,
                    Span::new(position, self.cursor),
                    "Missing string escape character.",
                )
            })?;
            let decoded = match escaped {
                'n' => "\n".to_owned(),
                'r' => "\r".to_owned(),
                't' => "\t".to_owned(),
                '"' => "\"".to_owned(),
                '\\' => "\\".to_owned(),
                'u' => {
                    if self.advance() != Some('{') {
                        return Err(Diagnostic::new(
                            DiagnosticKind::Syntax,
                            Span::new(position, self.cursor),
                            "Expected `{` in Unicode escape.",
                        ));
                    }
                    let begin = self.cursor;
                    while self.peek().is_some_and(|ch| ch.is_ascii_hexdigit()) {
                        self.advance();
                    }
                    let end = self.cursor;
                    if self.advance() != Some('}') {
                        return Err(Diagnostic::new(
                            DiagnosticKind::Syntax,
                            Span::new(position, self.cursor),
                            "Invalid Unicode escape.",
                        ));
                    }
                    let point = u32::from_str_radix(&self.source[begin..end], 16).ok();
                    let Some(character) = point.and_then(char::from_u32) else {
                        return Err(Diagnostic::new(
                            DiagnosticKind::Unsupported,
                            Span::new(position, self.cursor),
                            "Unicode escape is outside the supported scalar range.",
                        ));
                    };
                    character.to_string()
                }
                '(' => {
                    frame.interpolation = Some(InterpolationScan {
                        start: position,
                        begin: self.cursor,
                        nesting: 1,
                    });
                    return Ok(StringFlow::Continue);
                }
                '\n' | '\r' if frame.multiline => {
                    if escaped == '\r' && self.peek() == Some('\n') {
                        self.advance();
                    }
                    frame.parts.push(StringPart::Continuation);
                    return Ok(StringFlow::Continue);
                }
                _ => {
                    return Err(Diagnostic::new(
                        DiagnosticKind::Syntax,
                        Span::new(position, self.cursor),
                        "Invalid string escape.",
                    ));
                }
            };
            frame.parts.push(StringPart::Escape(decoded));
        } else {
            frame.text.push(ch);
        }
        Ok(StringFlow::Continue)
    }

    fn finish_string(&self, frame: StringScan) -> Result<Kind, Diagnostic> {
        let mut parts = frame.parts;
        if frame.multiline {
            let ranges: Vec<_> = expression_ranges(&parts).collect();
            parts =
                multiline_parts(parts, Span::new(frame.start, self.cursor)).map_err(|error| {
                    self.validate_interpolations(ranges.into_iter())
                        .err()
                        .unwrap_or(error)
                })?;
        }
        let mut rendered = Vec::new();
        let mut text = String::new();
        for part in parts {
            match part {
                StringPart::Text(value) | StringPart::Escape(value) => text.push_str(&value),
                StringPart::Newline => text.push('\n'),
                StringPart::Continuation => {}
                StringPart::Expression(span, interpolation) => {
                    if !text.is_empty() {
                        rendered.push(StringPart::Text(std::mem::take(&mut text)));
                    }
                    rendered.push(StringPart::Expression(span, interpolation));
                }
            }
        }
        if rendered.is_empty() {
            Ok(Kind::String(text))
        } else {
            if !text.is_empty() {
                rendered.push(StringPart::Text(text));
            }
            Ok(Kind::StringTemplate(rendered))
        }
    }
}

fn expression_ranges(parts: &[StringPart]) -> impl Iterator<Item = Span> + '_ {
    parts.iter().filter_map(|part| match part {
        StringPart::Expression(range, _) => Some(*range),
        _ => None,
    })
}
