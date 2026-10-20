// Portions adapted from Pkl 0.32.0 SyntaxHighlighter.java and Lexer.java.
// Copyright © 2025-2026 Apple Inc. and the Pkl project authors. All rights reserved.
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at https://www.apache.org/licenses/LICENSE-2.0
// Unless required by applicable law or agreed to in writing, software distributed
// under the License is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR
// CONDITIONS OF ANY KIND, either express or implied. See the License for the
// specific language governing permissions and limitations under the License.

//! Uncolored replay of Pkl 0.32.0 SyntaxHighlighter token traversal.
//!
//! The upstream highlighter stops interpolation highlighting at the first `)`,
//! even when the lexer remains inside a nested expression. Its two direct calls
//! to `next` omit intervening whitespace. Keep that behavior separate from source
//! positions and the parser, and discard the entire replay on a lexical error.

#[derive(Clone, Copy, PartialEq, Eq)]
enum Token {
    Other,
    Start,
    End,
    Part,
    Escape,
    Interpolation,
    RightParen,
    Eof,
}
struct Scope {
    quotes: usize,
    pounds: usize,
    parens: i64,
}
struct Lexer<'a> {
    source: &'a str,
    cursor: usize,
    start: usize,
    string: bool,
    ended: bool,
    escape: bool,
    scopes: Vec<Scope>,
}
type LexResult<T> = Result<T, ()>;

pub(super) fn uncolored(source: &str) -> String {
    let mut lexer = Lexer {
        source,
        cursor: 0,
        start: 0,
        string: false,
        ended: false,
        escape: false,
        scopes: Vec::new(),
    };
    let mut output = String::with_capacity(source.len());
    let result = replay(&mut output, &mut lexer);
    if result.is_err() {
        source.into()
    } else {
        output
    }
}
fn advance(output: &mut String, lexer: &mut Lexer<'_>) -> LexResult<Token> {
    let previous = lexer.cursor;
    let token = lexer.next()?;
    output.push_str(&lexer.source[previous..lexer.start]);
    Ok(token)
}
// The Java implementation recurses between normal and string highlighting.
// An explicit stack also renders parser depth-limit failures safely.
enum Traversal {
    Normal(Token, Token),
    NormalAdvance(Token),
    String(Token),
    AfterInterpolation,
}
fn replay(output: &mut String, lexer: &mut Lexer<'_>) -> LexResult<()> {
    let mut work = vec![Traversal::Normal(lexer.next()?, Token::Eof)];
    while let Some(step) = work.pop() {
        match step {
            Traversal::Normal(token, end) => {
                if token == end || token == Token::Eof {
                    continue;
                }
                work.push(Traversal::NormalAdvance(end));
                if token == Token::Start {
                    work.push(Traversal::String(token));
                } else {
                    output.push_str(lexer.text());
                }
            }
            Traversal::NormalAdvance(end) => {
                work.push(Traversal::Normal(advance(output, lexer)?, end));
            }
            Traversal::String(token) => {
                if token == Token::End || token == Token::Eof {
                    output.push_str(lexer.text());
                } else if token == Token::Interpolation {
                    output.push_str(lexer.text());
                    work.push(Traversal::AfterInterpolation);
                    work.push(Traversal::Normal(lexer.next()?, Token::RightParen));
                } else {
                    output.push_str(lexer.text());
                    work.push(Traversal::String(advance(output, lexer)?));
                }
            }
            Traversal::AfterInterpolation => {
                output.push_str(lexer.text());
                lexer.next()?;
                output.push_str(lexer.text());
                work.push(Traversal::String(advance(output, lexer)?));
            }
        }
    }
    Ok(())
}
impl Lexer<'_> {
    fn text(&self) -> &str {
        &self.source[self.start..self.cursor]
    }
    fn peek(&self) -> Option<char> {
        self.source[self.cursor..].chars().next()
    }
    fn take(&mut self) -> Option<char> {
        let ch = self.peek()?;
        self.cursor += ch.len_utf8();
        Some(ch)
    }
    fn consume(&mut self, text: &str) -> bool {
        if self.source[self.cursor..].starts_with(text) {
            self.cursor += text.len();
            true
        } else {
            false
        }
    }
    fn next(&mut self) -> LexResult<Token> {
        self.start = self.cursor;
        if self.string {
            return self.next_string();
        }
        while self
            .peek()
            .is_some_and(|ch| matches!(ch, ' ' | '\n' | '\t' | '\x0c' | '\r'))
        {
            self.take();
        }
        self.start = self.cursor;
        let Some(ch) = self.take() else {
            return Ok(Token::Eof);
        };
        match ch {
            '(' => {
                if let Some(scope) = self.scopes.last_mut() {
                    scope.parens += 1;
                }
            }
            ')' => {
                if let Some(scope) = self.scopes.last_mut() {
                    scope.parens -= 1;
                    self.string = scope.parens <= 0;
                }
                return Ok(Token::RightParen);
            }
            '"' => return Ok(self.start_string(0)),
            '#' => {
                if self.consume("!") {
                    self.line_comment();
                } else {
                    let mut pounds = 1;
                    while self.consume("#") {
                        pounds += 1;
                    }
                    if !self.consume("\"") {
                        return Err(());
                    }
                    return Ok(self.start_string(pounds));
                }
            }
            '`' => {
                while self
                    .peek()
                    .is_some_and(|ch| !matches!(ch, '`' | '\n' | '\r'))
                {
                    self.take();
                }
                if !self.consume("`") {
                    return Err(());
                }
            }
            '/' => {
                if self.consume("/") {
                    self.line_comment();
                } else if self.consume("*") {
                    let Some(end) = self.source[self.cursor..].find("*/") else {
                        return Err(());
                    };
                    self.cursor += end + 2;
                    // Pinned lexBlockComment checks EOF after consuming the terminator.
                    if self.cursor == self.source.len() {
                        return Err(());
                    }
                }
            }
            '&' => {
                if !self.consume("&") {
                    return Err(());
                }
            }
            '~' => {
                if !self.consume("/") {
                    return Err(());
                }
            }
            '.' => {
                if self.consume(".") {
                    if !self.consume(".") {
                        return Err(());
                    }
                    self.consume("?");
                } else if self.peek().is_some_and(|ch| ch.is_ascii_digit()) {
                    self.number('.')?;
                }
            }
            ';' | '{' | '}' | ',' | '@' | ':' | '+' | '%' | '[' | ']' | '=' | '>' | '<' | '-'
            | '!' | '?' | '|' | '*' => {}
            ch if is_digit(ch) => self.number(ch)?,
            ch if crate::parser::is_identifier_start(ch) => {
                while self.peek().is_some_and(crate::parser::is_identifier_part) {
                    self.take();
                }
            }
            _ => return Err(()),
        }
        Ok(Token::Other)
    }
    fn line_comment(&mut self) {
        while self.peek().is_some_and(|ch| !matches!(ch, '\n' | '\r')) {
            self.take();
        }
    }
    fn number(&mut self, first: char) -> LexResult<()> {
        if first == '0' {
            let radix = match self.peek() {
                Some('x' | 'X') => Some(16),
                Some('b' | 'B') => Some(2),
                Some('o' | 'O') => Some(8),
                _ => None,
            };
            if let Some(radix) = radix {
                self.take();
                if !self.peek().is_some_and(|ch| ch.is_digit(radix)) {
                    return Err(());
                }
                while self
                    .peek()
                    .is_some_and(|ch| ch == '_' || ch.is_digit(radix))
                {
                    self.take();
                }
                return Ok(());
            }
        }
        while self
            .peek()
            .is_some_and(|ch| ch.is_ascii_digit() || ch == '_')
        {
            self.take();
        }
        if first != '.' && self.peek() == Some('.') {
            let saved = self.cursor;
            self.take();
            if self.peek() == Some('_') {
                return Err(());
            }
            if self.peek().is_some_and(|ch| ch.is_ascii_digit()) {
                while self
                    .peek()
                    .is_some_and(|ch| ch.is_ascii_digit() || ch == '_')
                {
                    self.take();
                }
            } else {
                self.cursor = saved;
                return Ok(());
            }
        }
        if self.peek().is_some_and(|ch| matches!(ch, 'e' | 'E')) {
            self.take();
            if self.peek().is_some_and(|ch| matches!(ch, '+' | '-')) {
                self.take();
            }
            if !self.peek().is_some_and(|ch| ch.is_ascii_digit()) {
                return Err(());
            }
            while self
                .peek()
                .is_some_and(|ch| ch.is_ascii_digit() || ch == '_')
            {
                self.take();
            }
        }
        Ok(())
    }
    fn start_string(&mut self, pounds: usize) -> Token {
        let quotes = if self.consume("\"\"") { 3 } else { 1 };
        self.scopes.push(Scope {
            quotes,
            pounds,
            parens: 0,
        });
        self.string = true;
        self.ended = false;
        Token::Start
    }
    fn next_string(&mut self) -> LexResult<Token> {
        let scope = self.scopes.last().ok_or(())?;
        let (quotes, pounds) = (scope.quotes, scope.pounds);
        if self.ended {
            self.cursor += quotes + pounds;
            self.ended = false;
            self.scopes.pop();
            self.string = false;
            return Ok(Token::End);
        }
        if self.peek().is_none() {
            return Ok(Token::Eof);
        }
        if self.escape {
            self.escape = false;
            self.cursor += pounds + 1;
            return self.escape_token();
        }
        if quotes == 3 && self.peek().is_some_and(|ch| matches!(ch, '\r' | '\n')) {
            if self.take() == Some('\r') {
                self.consume("\n");
            }
            return Ok(Token::Part);
        }
        let mut quote_count = 0;
        let mut pound_count = 0;
        let mut backslash = false;
        while let Some(ch) = self.peek() {
            if matches!(ch, '\r' | '\n') {
                if quotes == 1 {
                    return Err(());
                }
                break;
            }
            self.take();
            match ch {
                '"' => {
                    quote_count = if quotes == 1 { 1 } else { quote_count + 1 };
                    pound_count = 0;
                    backslash = false;
                    if quote_count == quotes && pounds == 0 {
                        self.cursor -= quotes;
                        self.ended = true;
                        break;
                    }
                }
                '\\' => {
                    quote_count = 0;
                    pound_count = 0;
                    backslash = true;
                    if pounds == 0 {
                        self.cursor -= 1;
                        self.escape = true;
                        break;
                    }
                }
                '#' => {
                    pound_count += 1;
                    if quote_count == quotes && pound_count == pounds {
                        self.cursor -= quotes + pounds;
                        self.ended = true;
                        break;
                    }
                    if backslash && pound_count == pounds {
                        self.cursor -= pounds + 1;
                        self.escape = true;
                        break;
                    }
                }
                _ => {
                    quote_count = 0;
                    pound_count = 0;
                    backslash = false;
                }
            }
        }
        Ok(Token::Part)
    }
    fn escape_token(&mut self) -> LexResult<Token> {
        match self.take().ok_or(())? {
            'n' | '"' | '\\' | 't' | 'r' | '\n' => {}
            '\r' => {
                self.consume("\n");
            }
            '(' => {
                self.scopes.last_mut().ok_or(())?.parens += 1;
                self.string = false;
                return Ok(Token::Interpolation);
            }
            'u' => {
                if !self.consume("{") {
                    return Err(());
                }
                while self.peek().is_some_and(is_letter_or_digit) {
                    self.take();
                }
                if !self.consume("}") {
                    return Err(());
                }
            }
            _ => return Err(()),
        }
        Ok(Token::Escape)
    }
}

// BMP Character predicates from the pinned oracle JDK 21.0.12.1. The Java
// lexer consumes UTF-16 units, so supplementary characters fail these tests.
fn is_digit(ch: char) -> bool {
    const RANGES: &[(u32, u32)] = &[
        (0x30, 0x39),
        (0x660, 0x669),
        (0x6f0, 0x6f9),
        (0x7c0, 0x7c9),
        (0x966, 0x96f),
        (0x9e6, 0x9ef),
        (0xa66, 0xa6f),
        (0xae6, 0xaef),
        (0xb66, 0xb6f),
        (0xbe6, 0xbef),
        (0xc66, 0xc6f),
        (0xce6, 0xcef),
        (0xd66, 0xd6f),
        (0xde6, 0xdef),
        (0xe50, 0xe59),
        (0xed0, 0xed9),
        (0xf20, 0xf29),
        (0x1040, 0x1049),
        (0x1090, 0x1099),
        (0x17e0, 0x17e9),
        (0x1810, 0x1819),
        (0x1946, 0x194f),
        (0x19d0, 0x19d9),
        (0x1a80, 0x1a89),
        (0x1a90, 0x1a99),
        (0x1b50, 0x1b59),
        (0x1bb0, 0x1bb9),
        (0x1c40, 0x1c49),
        (0x1c50, 0x1c59),
        (0xa620, 0xa629),
        (0xa8d0, 0xa8d9),
        (0xa900, 0xa909),
        (0xa9d0, 0xa9d9),
        (0xa9f0, 0xa9f9),
        (0xaa50, 0xaa59),
        (0xabf0, 0xabf9),
        (0xff10, 0xff19),
    ];
    RANGES
        .iter()
        .any(|&(start, end)| (start..=end).contains(&(ch as u32)))
}
fn is_letter_or_digit(ch: char) -> bool {
    const NON_LETTER_STARTS: &[(u32, u32)] = &[
        (0x24, 0x24),
        (0x5f, 0x5f),
        (0x16ee, 0x16f0),
        (0x1885, 0x1886),
        (0x2118, 0x2118),
        (0x212e, 0x212e),
        (0x2160, 0x2182),
        (0x2185, 0x2188),
        (0x3007, 0x3007),
        (0x3021, 0x3029),
        (0x3038, 0x303a),
        (0x309b, 0x309c),
        (0xa6e6, 0xa6ef),
    ];
    is_digit(ch)
        || (crate::parser::is_identifier_start(ch)
            && !NON_LETTER_STARTS
                .iter()
                .any(|&(start, end)| (start..=end).contains(&(ch as u32))))
}

#[cfg(test)]
mod tests {
    use super::uncolored;
    #[test]
    fn interpolation_traversal_keeps_lexer_and_highlighter_parentheses_distinct() {
        assert_eq!(
            uncolored(r#"x = "\(  f(1) + 2) tail""#),
            r#"x = "\(f(1)+ 2) tail""#
        );
        assert_eq!(uncolored(r#"x = "\(1) tail""#), r#"x = "\(1) tail""#);
        assert_eq!(
            uncolored(r##"x = #"\#(  f(1)  + 2) tail"#"##),
            r##"x = #"\#(f(1)+ 2) tail"#"##
        );
    }
    #[test]
    fn comments_quoted_identifiers_and_nested_strings_are_tokens() {
        assert_eq!(
            uncolored(r#"x = "\( /* ) */ `)` + f(1)  + 2)""#),
            r#"x = "\(/* ) */ `)` + f(1)+ 2)""#
        );
        assert_eq!(
            uncolored(r#"x = "\( "\( f(1) + 2)" + g(3) + 4)""#),
            r#"x = "\("\(f(1)+ 2)" + g(3)+ 4)""#
        );
    }
    #[test]
    fn lexical_failure_restores_whole_original() {
        for line in [
            r#"x = "\( f(1) + 2)" &"#,
            r#"x = "\( f(1) + 2)\q""#,
            r#"x = "\( f(1) + 2)" /* end */"#,
        ] {
            assert_eq!(uncolored(line), line);
        }
        // EOF itself is not a lexer error, including incomplete interpolation.
        assert_eq!(uncolored(r#"x = "\( f(1) +"#), r#"x = "\(f(1)+"#);
    }
    #[test]
    fn malformed_deep_interpolation_uses_an_explicit_traversal_stack() {
        let source = "\"\\(".repeat(20_000);
        assert_eq!(uncolored(&source), source);
    }
}
