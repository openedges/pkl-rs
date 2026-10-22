//! Eager string tokens with heap-backed nested string/interpolation scanning.
use std::collections::BTreeMap;
use std::sync::Arc;

use super::{Diagnostic, DiagnosticKind, Kind, Lexer, Parser, Span, StringPart};

// EOF validation parses an interpolation to preserve its first syntax error.
// Replaying already failed child strings prevents that validation from entering
// the same nested scanner again. The origin source end retains EOF provenance;
// a fully consumed failed token can also be replayed in a shorter visible prefix.
pub(super) type StringErrors = Arc<BTreeMap<(usize, usize), (usize, Diagnostic)>>;

struct StringScan {
    start: usize,
    multiline: bool,
    delimiter: String,
    escape: String,
    parts: Vec<SpannedStringPart>,
    text: String,
    interpolation: Option<InterpolationScan>,
}

struct SpannedStringPart {
    part: StringPart,
    span: Span,
}

impl StringScan {
    fn flush_text(&mut self, end: usize, include_empty: bool) {
        if include_empty || !self.text.is_empty() {
            self.parts.push(SpannedStringPart {
                span: Span::new(end - self.text.len(), end),
                part: StringPart::Text(std::mem::take(&mut self.text)),
            });
        }
    }
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
                        .and_then(|mut parser| {
                            parser.expression_expected(0, Some(')'))?;
                            parser.expect(')')?;
                            Ok(())
                        });
                    let mut error = result.err().unwrap_or_else(|| {
                        let mut error = Diagnostic::new(
                            DiagnosticKind::Syntax,
                            Span::new(range.end, range.end),
                            "Unexpected token `EOF`. Expected `)`.",
                        );
                        error.highlight_length_utf16 = Some(1);
                        error
                    });
                    if error.span.start == self.source.len()
                        && error.highlight_length_utf16 == Some(1)
                    {
                        // ParserImpl expects a one-unit span after the final
                        // token. At true EOF that lies outside the Source; the
                        // Java exception renders its unavailable section at
                        // the beginning of the module. Whitespace after the
                        // token keeps the source section available.
                        error.span = Span::new(0, 0);
                    }
                    Err(error)
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
                        let span = Span::new(interpolation.start, token.span.end);
                        frame.parts.push(SpannedStringPart {
                            part: StringPart::Expression(
                                Span::new(interpolation.begin, token.span.start),
                                span,
                            ),
                            span,
                        });
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
            // Java emits an empty text token before delimiters and escapes.
            // Its location participates in multiline validation and indentation.
            frame.flush_text(self.cursor, true);
            self.cursor += frame.delimiter.len();
            return Ok(StringFlow::Done);
        }
        let position = self.cursor;
        let Some(ch) = self.advance() else {
            return Err(Diagnostic::new(
                DiagnosticKind::Syntax,
                Span::new(self.cursor, self.cursor),
                format!("Missing `{}` delimiter.", frame.delimiter),
            ));
        };
        if ch == '\n' || ch == '\r' {
            if !frame.multiline {
                return Err(Diagnostic::new(
                    DiagnosticKind::Syntax,
                    Span::new(position, self.cursor),
                    format!("Missing `{}` delimiter.", frame.delimiter),
                ));
            }
            if ch == '\r' && self.peek() == Some('\n') {
                self.advance();
            }
            frame.flush_text(position, false);
            frame.parts.push(SpannedStringPart {
                part: StringPart::Newline,
                span: Span::new(position, self.cursor),
            });
        } else if ch == '\\' && self.source[position..].starts_with(&frame.escape) {
            self.cursor = position + frame.escape.len();
            frame.flush_text(position, true);
            let escaped_start = self.cursor;
            let escaped = self.advance().ok_or_else(|| {
                Diagnostic::new(
                    DiagnosticKind::Syntax,
                    Span::new(self.cursor, self.cursor),
                    "Unexpected end of file.",
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
                '\n' | '\r' => {
                    if escaped == '\r' && self.peek() == Some('\n') {
                        self.advance();
                    }
                    if !frame.multiline {
                        let mut error = Diagnostic::new(
                            DiagnosticKind::Syntax,
                            Span::new(position, self.cursor),
                            "Invalid line continuation escape sequence.\n\nLine continuations are only allowed in multi-line strings.",
                        );
                        error.highlight_length_utf16 =
                            Some(self.source[position..self.cursor].encode_utf16().count());
                        return Err(error);
                    }
                    frame.parts.push(SpannedStringPart {
                        part: StringPart::Continuation,
                        span: Span::new(position, self.cursor),
                    });
                    return Ok(StringFlow::Continue);
                }
                _ => {
                    let end = self.cursor;
                    if matches!(escaped, ' ' | '\t') {
                        while self.peek().is_some_and(|ch| matches!(ch, ' ' | '\t')) {
                            self.advance();
                        }
                        if self.advance().is_some_and(|ch| matches!(ch, '\n' | '\r')) {
                            let mut error = Diagnostic::new(
                                DiagnosticKind::Syntax,
                                Span::new(escaped_start - 1, self.cursor),
                                "Invalid line continuation escape sequence.\n\nWhitespace between the continuation escape and following newline is not allowed.",
                            );
                            // Java keeps the whitespace/newline token width
                            // even when rendering strips trailing whitespace.
                            error.highlight_length_utf16 = Some(
                                self.source[escaped_start - 1..self.cursor]
                                    .encode_utf16()
                                    .count(),
                            );
                            return Err(error);
                        }
                    }
                    return Err(Diagnostic::new(
                        DiagnosticKind::Syntax,
                        // Java uses the last hash rather than the backslash
                        // when an active raw-string escape is invalid.
                        Span::new(escaped_start - 1, end),
                        format!(
                            "Invalid character escape sequence `\\{escaped}`.\n\nValid character escape sequences are:  \\n  \\r  \\t  \\\"  \\\\ \\<newline>"
                        ),
                    ));
                }
            };
            frame.parts.push(SpannedStringPart {
                part: StringPart::Escape(decoded),
                span: Span::new(position, self.cursor),
            });
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
            match part.part {
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

fn expression_ranges(parts: &[SpannedStringPart]) -> impl Iterator<Item = Span> + '_ {
    parts.iter().filter_map(|part| match &part.part {
        StringPart::Expression(range, _) => Some(*range),
        _ => None,
    })
}

// Pkl 0.32.0 ParserImpl validates the complete token sequence after parsing
// interpolation expressions. Preserve token spans, including empty text parts,
// because an opening-line or indentation diagnostic can point at those tokens.
fn multiline_parts(
    mut parts: Vec<SpannedStringPart>,
    span: Span,
) -> Result<Vec<SpannedStringPart>, Diagnostic> {
    if !matches!(
        parts.first().map(|part| &part.part),
        Some(StringPart::Newline)
    ) {
        return Err(Diagnostic::new(
            DiagnosticKind::Syntax,
            parts.first().map_or(span, |part| part.span),
            "The content of a multi-line string must begin on a new line.",
        ));
    }
    if parts.len() == 1 {
        return Ok(Vec::new());
    }
    let indent = match parts.last().map(|part| &part.part) {
        Some(StringPart::Newline) => String::new(),
        Some(StringPart::Text(text))
            if text.chars().all(|ch| ch == ' ' || ch == '\t')
                && matches!(
                    parts.get(parts.len() - 2).map(|part| &part.part),
                    Some(StringPart::Newline)
                ) =>
        {
            let indent = text.clone();
            parts.pop();
            indent
        }
        _ => {
            return Err(Diagnostic::new(
                DiagnosticKind::Syntax,
                span,
                "The closing delimiter of a multi-line string must begin on a new line.",
            ));
        }
    };
    parts.pop(); // final source newline, which is not string content
    if parts.is_empty() {
        return Ok(Vec::new());
    }
    parts.remove(0); // opening newline
    let mut line_start = true;
    for part in &mut parts {
        match &mut part.part {
            StringPart::Text(text) => {
                if line_start {
                    let Some(stripped) = text.strip_prefix(&indent) else {
                        let leading = text
                            .bytes()
                            .take_while(|ch| matches!(ch, b' ' | b'\t'))
                            .count();
                        return Err(Diagnostic::new(
                            DiagnosticKind::Syntax,
                            Span::new(part.span.start + leading, part.span.end),
                            "Line must match or exceed indentation of the String's last line.",
                        ));
                    };
                    *text = stripped.to_owned();
                }
                line_start = false;
            }
            StringPart::Newline | StringPart::Continuation => line_start = true,
            StringPart::Escape(_) => {
                if line_start && !indent.is_empty() {
                    return Err(Diagnostic::new(
                        DiagnosticKind::Syntax,
                        part.span,
                        "Line must match or exceed indentation of the String's last line.",
                    ));
                }
                line_start = false;
            }
            StringPart::Expression(_, _) => {}
        }
    }
    Ok(parts)
}
