//! Eager string tokens with heap-backed nested string/interpolation scanning.
use std::collections::BTreeMap;
use std::sync::Arc;

use super::{Diagnostic, DiagnosticKind, Kind, Lexer, Parser, Span, StringPart, unicode_escape};

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
    pending_unicode: Option<usize>,
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
                    let mut consumed_end = self.cursor;
                    let result = self.finish_string(frame, &mut consumed_end);
                    if frames.is_empty() {
                        return result;
                    }
                    if let Err(error) = result {
                        self.remember_string_error_through(start, consumed_end, error);
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
        self.remember_string_error_through(start, self.cursor, error);
    }

    fn remember_string_error_through(
        &mut self,
        start: usize,
        consumed_end: usize,
        error: Diagnostic,
    ) {
        Arc::make_mut(&mut self.string_errors)
            .insert((start, self.source.len()), (consumed_end, error));
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
            pending_unicode: None,
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
            self.validate_pending_unicode(frame)?;
            self.cursor += frame.delimiter.len();
            return Ok(StringFlow::Done);
        }
        let position = self.cursor;
        let Some(ch) = self.advance() else {
            self.validate_pending_unicode(frame)?;
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
            self.cursor = position;
            frame.flush_text(position, true);
            self.validate_pending_unicode(frame)?;
            self.cursor += frame.escape.len();
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
                    if self.peek() != Some('{') {
                        let got = match self.peek() {
                            Some(character) if character as u32 > 0xffff => "?".to_owned(),
                            Some(character) => character.to_string(),
                            None => "EOF".into(),
                        };
                        return Err(self.unexpected_unicode_character(&got, "{"));
                    }
                    self.advance();
                    while self.peek().is_some_and(|character| {
                        super::identifier_contains(
                            unicode_escape::LETTER_OR_DIGIT_RANGES,
                            character,
                        )
                    }) {
                        self.advance();
                    }
                    if self.peek() != Some('}') {
                        return Err(Diagnostic::new(
                            DiagnosticKind::Syntax,
                            Span::new(position, self.cursor),
                            format!(
                                "Unterminated Unicode escape sequence `{}`.\n\nUnicode escape sequences must end with `}}`.",
                                &self.source[position..self.cursor]
                            ),
                        ));
                    }
                    self.advance();
                    if !frame.multiline {
                        frame.pending_unicode = Some(frame.parts.len());
                    }
                    frame.parts.push(SpannedStringPart {
                        part: StringPart::Unicode,
                        span: Span::new(position, self.cursor),
                    });
                    return Ok(StringFlow::Continue);
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

    fn validate_pending_unicode(&self, frame: &mut StringScan) -> Result<(), Diagnostic> {
        if let Some(index) = frame.pending_unicode.take() {
            // ParserImpl decodes an ordinary Unicode token after reading its
            // following text token. A newline in that text is a lexical error
            // first; an empty text token before another escape still succeeds.
            let part = &mut frame.parts[index];
            part.part = StringPart::Escape(decode_unicode(self.source, part.span)?);
        }
        Ok(())
    }

    fn unexpected_unicode_character(&self, got: &str, expected: &str) -> Diagnostic {
        let span = self.peek().map_or_else(
            || Span::new(self.cursor - 1, self.cursor - 1),
            |character| Span::new(self.cursor, self.cursor + character.len_utf8()),
        );
        let mut error = Diagnostic::new(
            DiagnosticKind::Syntax,
            span,
            format!("Unexpected character `{got}`. Did you mean `{expected}`?"),
        );
        // The Java lexer tests one UTF-16 code unit, including the first
        // surrogate of a supplementary character. EOF has an empty span.
        error.highlight_length_utf16 = Some(1);
        error
    }

    fn unicode_lookahead(&mut self) -> Result<(), Diagnostic> {
        // This cursor inspects only the token after a multiline delimiter.
        // Cached failures describe whole strings and cannot replace the Java
        // STRING_START token, whose contents have not yet been scanned.
        self.string_errors = Default::default();
        loop {
            let token = self.next_token(true).map_err(|error| {
                match &self.source[error.span.start..error.span.end] {
                    "&" if error.kind == DiagnosticKind::Unsupported => {
                        self.unexpected_unicode_character("&", "&&")
                    }
                    "~" if error.kind == DiagnosticKind::Unsupported => {
                        self.unexpected_unicode_character("~", "~/")
                    }
                    _ => error,
                }
            })?;
            let Kind::StringStart(first) = token.kind else {
                if matches!(token.kind, Kind::Symbol(';')) {
                    continue;
                }
                return Ok(());
            };
            if first == '#' {
                if self.peek() == Some('!') {
                    while self.peek().is_some_and(|ch| ch != '\n' && ch != '\r') {
                        self.advance();
                    }
                    return Ok(());
                }
                while self.peek() == Some('#') {
                    self.advance();
                }
                if self.peek().is_none() {
                    return Err(Diagnostic::new(
                        DiagnosticKind::Syntax,
                        Span::new(token.span.start, self.cursor),
                        "Unexpected end of file.",
                    ));
                }
                if self.peek() != Some('"') {
                    let character = self.peek().expect("visible string prefix character");
                    let got = if character as u32 > 0xffff {
                        "?".to_owned()
                    } else {
                        character.to_string()
                    };
                    return Err(self.unexpected_unicode_character(&got, "\""));
                }
                self.advance();
            }
            if self.source[self.cursor..].starts_with("\"\"") {
                self.cursor += 2;
            }
            return Ok(());
        }
    }

    fn finish_string(
        &self,
        frame: StringScan,
        consumed_end: &mut usize,
    ) -> Result<Kind, Diagnostic> {
        let mut parts = frame.parts;
        if frame.multiline {
            let ranges: Vec<_> = expression_ranges(&parts).collect();
            let end = self.cursor;
            let result = (|| {
                if parts
                    .iter()
                    .any(|part| matches!(part.part, StringPart::Unicode))
                {
                    // Before validating multiline content ParserImpl consumes
                    // STRING_END and lexes one following token. Ordinary
                    // strings already validated their Unicode tokens earlier.
                    // Do not scan a following string's whole content here.
                    let mut lookahead = self.clone();
                    let result = lookahead.unicode_lookahead();
                    // The scan resumes at STRING_END, while failure replay
                    // also requires visibility of the inspected lookahead.
                    *consumed_end = lookahead.cursor;
                    result?;
                }
                multiline_parts(self.source, parts, Span::new(frame.start, end))
            })();
            parts = result.map_err(|error| {
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
                StringPart::Unicode => unreachable!("Unicode tokens have been decoded"),
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
    source: &str,
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
            StringPart::Escape(_) | StringPart::Unicode => {
                if line_start && !indent.is_empty() {
                    return Err(Diagnostic::new(
                        DiagnosticKind::Syntax,
                        part.span,
                        "Line must match or exceed indentation of the String's last line.",
                    ));
                }
                if matches!(part.part, StringPart::Unicode) {
                    part.part = StringPart::Escape(decode_unicode(source, part.span)?);
                }
                line_start = false;
            }
            StringPart::Expression(_, _) => {}
        }
    }
    Ok(parts)
}

fn decode_unicode(source: &str, span: Span) -> Result<String, Diagnostic> {
    let text = &source[span.start..span.end];
    let brace = text.find('{').expect("lexed Unicode opening brace");
    let digits = &text[brace + 1..text.len() - 1];
    // Integer.parseInt accepts Java Character.digit values and arbitrary
    // leading zeroes, but rejects values above signed i32::MAX. Its advertised
    // 1–6 character limit is not an independent validation rule.
    let point = (!digits.is_empty()).then_some(0_i32).and_then(|point| {
        digits.chars().try_fold(point, |value, character| {
            let character = character as u32;
            let index =
                unicode_escape::HEX_DIGIT_RANGES.partition_point(|(_, end, _)| *end < character);
            let (start, end, first) = *unicode_escape::HEX_DIGIT_RANGES.get(index)?;
            if character < start || character > end {
                return None;
            }
            value
                .checked_mul(16)?
                .checked_add((first + character - start) as i32)
        })
    });
    let Some(point) = point else {
        let prefix = &text[..brace];
        return Err(Diagnostic::new(
            DiagnosticKind::Syntax,
            span,
            format!(
                "Invalid Unicode escape sequence `{text}`.\n\nValid Unicode escape sequences are {prefix}{{0}} to {prefix}{{10FFFF}} (1-6 hexadecimal characters)."
            ),
        ));
    };
    let Some(character) = char::from_u32(point as u32) else {
        // Java represents lone surrogates and has a distinct internal error for
        // larger code points. Preserve both as explicit unsupported gaps.
        return Err(Diagnostic::new(
            DiagnosticKind::Unsupported,
            span,
            "Unicode escape is outside the supported scalar range.",
        ));
    };
    Ok(character.to_string())
}
