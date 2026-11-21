//! Lossless Pkl 0.32.0 lexer mirroring `org.pkl.parser.Lexer`.

use crate::error;
use crate::token::Token;
use crate::unicode_tables::{
    is_identifier_part, is_identifier_start, is_java_digit, is_java_letter_or_digit,
};
use crate::{FormatError, FullSpan, Span};

const EOF: i32 = -1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    Default,
    String,
}

#[derive(Clone, Debug)]
struct InterpolationScope {
    quotes: usize,
    pounds: usize,
    parens: isize,
}

pub(crate) struct Lexer {
    source: Vec<u16>,
    size: usize,
    cursor: usize,
    s_cursor: usize,
    line: usize,
    s_line: usize,
    col: usize,
    s_col: usize,
    lookahead: i32,
    state: State,
    interpolation_stack: Vec<InterpolationScope>,
    string_ended: bool,
    is_escape: bool,
    new_lines_between: usize,
}

impl Lexer {
    pub(crate) fn new(input: &str) -> Self {
        let source: Vec<u16> = input.encode_utf16().collect();
        let size = source.len();
        let lookahead = if size > 0 { i32::from(source[0]) } else { EOF };
        Self {
            source,
            size,
            cursor: 0,
            s_cursor: 0,
            line: 1,
            s_line: 1,
            col: 1,
            s_col: 1,
            lookahead,
            state: State::Default,
            interpolation_stack: Vec::new(),
            string_ended: false,
            is_escape: false,
            new_lines_between: 0,
        }
    }

    pub(crate) fn span(&self) -> Span {
        Span {
            char_index: self.s_cursor,
            length: self.cursor - self.s_cursor,
        }
    }

    pub(crate) fn full_span(&self) -> FullSpan {
        FullSpan {
            char_index: self.s_cursor,
            length: self.cursor - self.s_cursor,
            line_begin: self.s_line,
            col_begin: self.s_col,
            line_end: self.line,
            col_end: self.col,
        }
    }

    pub(crate) fn text(&self) -> String {
        String::from_utf16_lossy(&self.source[self.s_cursor..self.cursor])
    }

    pub(crate) fn get_new_lines_between(&self) -> usize {
        self.new_lines_between
    }

    pub(crate) fn into_source(self) -> Vec<u16> {
        self.source
    }

    pub(crate) fn next_token(&mut self) -> Result<Token, FormatError> {
        self.s_cursor = self.cursor;
        self.s_line = self.line;
        self.s_col = self.col;
        self.new_lines_between = 0;
        match self.state {
            State::Default => self.next_default(),
            State::String => self.next_string(),
        }
    }

    fn next_default(&mut self) -> Result<Token, FormatError> {
        let mut ch = self.next_char();
        while matches!(ch, 0x20 | 0x0a | 0x09 | 0x0c | 0x0d) {
            self.s_cursor = self.cursor;
            if ch == 0x0a {
                self.new_lines_between += 1;
                self.s_line = self.line;
            }
            self.s_col = self.col;
            ch = self.next_char();
        }
        match ch {
            EOF => {
                self.cursor -= 1;
                Ok(Token::Eof)
            }
            0x3b /* ; */ => Ok(Token::Semicolon),
            0x28 /* ( */ => {
                if let Some(scope) = self.interpolation_stack.last_mut() {
                    scope.parens += 1;
                }
                Ok(Token::LParen)
            }
            0x29 /* ) */ => {
                if let Some(scope) = self.interpolation_stack.last_mut() {
                    scope.parens -= 1;
                    if scope.parens <= 0 {
                        self.state = State::String;
                    }
                }
                Ok(Token::RParen)
            }
            0x7b /* { */ => Ok(Token::LBrace),
            0x7d /* } */ => Ok(Token::RBrace),
            0x2c /* , */ => Ok(Token::Comma),
            0x40 /* @ */ => Ok(Token::At),
            0x3a /* : */ => Ok(Token::Colon),
            0x2b /* + */ => Ok(Token::Plus),
            0x25 /* % */ => Ok(Token::Mod),
            0x5b /* [ */ => {
                if self.lookahead == i32::from(b'[') {
                    self.next_char();
                    Ok(Token::LPred)
                } else {
                    Ok(Token::LBrack)
                }
            }
            0x5d /* ] */ => Ok(Token::RBrack),
            0x3d /* = */ => {
                if self.lookahead == i32::from(b'=') {
                    self.next_char();
                    Ok(Token::Equal)
                } else {
                    Ok(Token::Assign)
                }
            }
            0x3e /* > */ => {
                if self.lookahead == i32::from(b'=') {
                    self.next_char();
                    Ok(Token::Gte)
                } else {
                    Ok(Token::Gt)
                }
            }
            0x3c /* < */ => {
                if self.lookahead == i32::from(b'=') {
                    self.next_char();
                    Ok(Token::Lte)
                } else {
                    Ok(Token::Lt)
                }
            }
            0x2d /* - */ => {
                if self.lookahead == i32::from(b'>') {
                    self.next_char();
                    Ok(Token::Arrow)
                } else {
                    Ok(Token::Minus)
                }
            }
            0x21 /* ! */ => {
                if self.lookahead == i32::from(b'!') {
                    self.next_char();
                    Ok(Token::NonNull)
                } else if self.lookahead == i32::from(b'=') {
                    self.next_char();
                    Ok(Token::NotEqual)
                } else {
                    Ok(Token::Not)
                }
            }
            0x3f /* ? */ => {
                if self.lookahead == i32::from(b'.') {
                    self.next_char();
                    Ok(Token::QDot)
                } else if self.lookahead == i32::from(b'?') {
                    self.next_char();
                    Ok(Token::Coalesce)
                } else {
                    Ok(Token::Question)
                }
            }
            0x26 /* & */ => {
                if self.lookahead == i32::from(b'&') {
                    self.next_char();
                    Ok(Token::And)
                } else {
                    Err(self.unexpected_char(ch, "&&"))
                }
            }
            0x7c /* | */ => {
                if self.lookahead == i32::from(b'>') {
                    self.next_char();
                    Ok(Token::Pipe)
                } else if self.lookahead == i32::from(b'|') {
                    self.next_char();
                    Ok(Token::Or)
                } else {
                    Ok(Token::Union)
                }
            }
            0x2a /* * */ => {
                if self.lookahead == i32::from(b'*') {
                    self.next_char();
                    Ok(Token::Pow)
                } else {
                    Ok(Token::Star)
                }
            }
            0x7e /* ~ */ => {
                if self.lookahead == i32::from(b'/') {
                    self.next_char();
                    Ok(Token::IntDiv)
                } else {
                    Err(self.unexpected_char(ch, "~/"))
                }
            }
            0x2e /* . */ => {
                if self.lookahead == i32::from(b'.') {
                    self.next_char();
                    if self.lookahead == i32::from(b'.') {
                        self.next_char();
                        if self.lookahead == i32::from(b'?') {
                            self.next_char();
                            Ok(Token::QSpread)
                        } else {
                            Ok(Token::Spread)
                        }
                    } else {
                        Err(self.unexpected_char_3("..", ".", "...", "...?"))
                    }
                } else if (48..=57).contains(&self.lookahead) {
                    self.lex_number(ch)
                } else {
                    Ok(Token::Dot)
                }
            }
            0x60 /* ` */ => {
                self.lex_quoted_identifier()?;
                Ok(Token::Identifier)
            }
            0x2f /* / */ => self.lex_slash(),
            0x22 /* " */ => Ok(self.lex_string_start(0)),
            0x23 /* # */ => {
                if self.lookahead == i32::from(b'!') {
                    Ok(self.lex_shebang())
                } else {
                    self.lex_string_start_pounds()
                }
            }
            _ => {
                let unit = ch as u16;
                if is_java_digit(unit) {
                    self.lex_number(ch)
                } else if is_identifier_start(unit) {
                    Ok(self.lex_identifier())
                } else {
                    let ch_str = unit_to_string(unit);
                    Err(self.lex_error_at(error::invalid_character(&ch_str), self.cursor - 1, 1))
                }
            }
        }
    }

    fn next_string(&mut self) -> Result<Token, FormatError> {
        let scope = self
            .interpolation_stack
            .last()
            .expect("interpolation stack must be non-empty in String state")
            .clone();
        if self.string_ended {
            self.lex_string_end(&scope);
            self.string_ended = false;
            self.interpolation_stack.pop();
            self.state = State::Default;
            return Ok(Token::StringEnd);
        }
        if self.lookahead == EOF {
            return Ok(Token::Eof);
        }
        if self.is_escape {
            self.is_escape = false;
            for _ in 0..=scope.pounds {
                self.next_char();
            }
            return self.lex_escape();
        }
        if scope.quotes == 1 {
            self.lex_string(scope.pounds)?;
        } else {
            if self.lookahead == 0x0d {
                self.next_char();
                if self.lookahead == 0x0a {
                    self.next_char();
                }
                return Ok(Token::StringNewline);
            }
            if self.lookahead == 0x0a {
                self.next_char();
                return Ok(Token::StringNewline);
            }
            self.lex_multi_string(scope.pounds);
        }
        Ok(Token::StringPart)
    }

    fn lex_string_start_pounds(&mut self) -> Result<Token, FormatError> {
        let mut pounds = 1;
        while self.lookahead == i32::from(b'#') {
            self.next_char();
            pounds += 1;
        }
        if self.lookahead == EOF {
            return Err(self.lex_error_span(error::unexpected_end_of_file(), self.span()));
        }
        if self.lookahead != i32::from(b'"') {
            return Err(self.unexpected_char(self.lookahead, "\""));
        }
        self.next_char();
        Ok(self.lex_string_start(pounds))
    }

    fn lex_string_start(&mut self, pounds: usize) -> Token {
        let mut quotes = 1;
        if self.lookahead == i32::from(b'"') {
            self.next_char();
            if self.lookahead == i32::from(b'"') {
                self.next_char();
                quotes = 3;
            } else {
                self.backup();
            }
        }
        self.state = State::String;
        self.interpolation_stack.push(InterpolationScope {
            quotes,
            pounds,
            parens: 0,
        });
        self.string_ended = false;
        if quotes == 1 {
            Token::StringStart
        } else {
            Token::StringMultiStart
        }
    }

    fn lex_string_end(&mut self, scope: &InterpolationScope) {
        for _ in 0..(scope.quotes + scope.pounds) {
            self.next_char();
        }
    }

    fn lex_string(&mut self, pounds: usize) -> Result<(), FormatError> {
        let mut pounds_in_a_row = 0;
        let mut found_quote = false;
        let mut found_backslash = false;
        while self.lookahead != EOF {
            let ch = self.next_char();
            match ch {
                0x0a | 0x0d => {
                    let delim = format!("\"{}", "#".repeat(pounds));
                    return Err(self.lex_error_at(
                        error::missing_delimiter(&delim),
                        self.cursor - 1,
                        1,
                    ));
                }
                0x22 /* " */ => {
                    if pounds == 0 {
                        self.backup();
                        self.string_ended = true;
                        return Ok(());
                    }
                    found_quote = true;
                    found_backslash = false;
                    pounds_in_a_row = 0;
                }
                0x5c /* \ */ => {
                    found_quote = false;
                    found_backslash = true;
                    pounds_in_a_row = 0;
                    if pounds == pounds_in_a_row {
                        self.backup_n(pounds + 1);
                        self.is_escape = true;
                        return Ok(());
                    }
                }
                0x23 /* # */ => {
                    pounds_in_a_row += 1;
                    if found_quote && pounds == pounds_in_a_row {
                        self.backup_n(pounds + 1);
                        self.string_ended = true;
                        return Ok(());
                    }
                    if found_backslash && pounds == pounds_in_a_row {
                        self.backup_n(pounds + 1);
                        self.is_escape = true;
                        return Ok(());
                    }
                }
                _ => {
                    found_quote = false;
                    found_backslash = false;
                    pounds_in_a_row = 0;
                }
            }
        }
        Ok(())
    }

    fn lex_multi_string(&mut self, pounds: usize) {
        let mut pounds_in_a_row = 0;
        let mut quotes_in_a_row = 0;
        let mut found_backslash = false;
        while self.lookahead != EOF && self.lookahead != 0x0a && self.lookahead != 0x0d {
            let ch = self.next_char();
            match ch {
                0x22 /* " */ => {
                    quotes_in_a_row += 1;
                    if quotes_in_a_row == 3 && pounds == 0 {
                        self.backup_n(3);
                        self.string_ended = true;
                        return;
                    }
                    pounds_in_a_row = 0;
                    found_backslash = false;
                }
                0x5c /* \ */ => {
                    quotes_in_a_row = 0;
                    pounds_in_a_row = 0;
                    found_backslash = true;
                    if pounds == pounds_in_a_row {
                        self.backup_n(pounds + 1);
                        self.is_escape = true;
                        return;
                    }
                }
                0x23 /* # */ => {
                    pounds_in_a_row += 1;
                    if quotes_in_a_row == 3 && pounds == pounds_in_a_row {
                        self.backup_n(pounds + 3);
                        self.string_ended = true;
                        return;
                    }
                    if found_backslash && pounds == pounds_in_a_row {
                        self.backup_n(pounds + 1);
                        self.is_escape = true;
                        return;
                    }
                }
                _ => {
                    quotes_in_a_row = 0;
                    pounds_in_a_row = 0;
                    found_backslash = false;
                }
            }
        }
    }

    fn lex_escape(&mut self) -> Result<Token, FormatError> {
        if self.lookahead == EOF {
            return Err(self.unexpected_end_of_file());
        }
        let ch = self.next_char();
        match ch {
            0x6e /* n */ => Ok(Token::StringEscapeNewline),
            0x22 /* " */ => Ok(Token::StringEscapeQuote),
            0x5c /* \ */ => Ok(Token::StringEscapeBackslash),
            0x74 /* t */ => Ok(Token::StringEscapeTab),
            0x72 /* r */ => Ok(Token::StringEscapeReturn),
            0x28 /* ( */ => {
                if let Some(scope) = self.interpolation_stack.last_mut() {
                    scope.parens += 1;
                }
                self.state = State::Default;
                Ok(Token::InterpolationStart)
            }
            0x75 /* u */ => self.lex_unicode_escape(),
            0x0a /* \n */ => Ok(Token::StringEscapeContinuation),
            0x0d /* \r */ => {
                if self.lookahead == 0x0a {
                    self.next_char();
                }
                Ok(Token::StringEscapeContinuation)
            }
            0x20 | 0x09 => {
                let c = self.cursor;
                let mut next = self.next_char();
                while next == 0x20 || next == 0x09 {
                    next = self.next_char();
                }
                if next == 0x0a || next == 0x0d {
                    return Err(self.lex_error_at(
                        error::invalid_line_continuation_escape_sequence_whitespace(),
                        c - 2,
                        self.cursor - c + 2,
                    ));
                }
                let seq = format!("\\{}", unit_to_string(ch as u16));
                Err(self.lex_error_at(
                    error::invalid_character_escape_sequence(&seq, "\\"),
                    c - 2,
                    2,
                ))
            }
            _ => {
                let seq = format!("\\{}", unit_to_string(ch as u16));
                Err(self.lex_error_at(
                    error::invalid_character_escape_sequence(&seq, "\\"),
                    self.cursor - 2,
                    2,
                ))
            }
        }
    }

    fn lex_unicode_escape(&mut self) -> Result<Token, FormatError> {
        if self.lookahead != i32::from(b'{') {
            return Err(self.unexpected_char(self.lookahead, "{"));
        }
        loop {
            self.next_char();
            if self.lookahead == i32::from(b'}')
                || self.lookahead == EOF
                || !is_java_letter_or_digit(self.lookahead as u16)
            {
                break;
            }
        }
        if self.lookahead == i32::from(b'}') {
            self.next_char();
            Ok(Token::StringEscapeUnicode)
        } else {
            Err(self.lex_error_span(
                error::unterminated_unicode_escape_sequence(&self.text()),
                self.span(),
            ))
        }
    }

    fn lex_identifier(&mut self) -> Token {
        while self.lookahead != EOF && is_identifier_part(self.lookahead as u16) {
            self.next_char();
        }
        let identifier_str = self.text();
        let identifier = get_keyword_or_identifier(&identifier_str);
        match identifier {
            Token::Import => {
                if self.lookahead == i32::from(b'*') {
                    self.next_char();
                    Token::ImportStar
                } else {
                    Token::Import
                }
            }
            Token::Read => match self.lookahead {
                0x2a /* * */ => {
                    self.next_char();
                    Token::ReadStar
                }
                0x3f /* ? */ => {
                    self.next_char();
                    Token::ReadQuestion
                }
                _ => Token::Read,
            },
            other => other,
        }
    }

    fn lex_quoted_identifier(&mut self) -> Result<(), FormatError> {
        while self.lookahead != i32::from(b'`')
            && self.lookahead != 0x0a
            && self.lookahead != 0x0d
            && self.lookahead != EOF
        {
            self.next_char();
        }
        if self.lookahead == i32::from(b'`') {
            self.next_char();
            Ok(())
        } else {
            Err(self.unexpected_char(self.lookahead, "backquote"))
        }
    }

    fn lex_number(&mut self, start: i32) -> Result<Token, FormatError> {
        if start == i32::from(b'0') {
            if self.lookahead == i32::from(b'x') || self.lookahead == i32::from(b'X') {
                self.next_char();
                self.lex_hex_number()?;
                return Ok(Token::Hex);
            }
            if self.lookahead == i32::from(b'b') || self.lookahead == i32::from(b'B') {
                self.next_char();
                self.lex_bin_number()?;
                return Ok(Token::Bin);
            }
            if self.lookahead == i32::from(b'o') || self.lookahead == i32::from(b'O') {
                self.next_char();
                self.lex_oct_number()?;
                return Ok(Token::Oct);
            }
            if self.lookahead == i32::from(b'e') || self.lookahead == i32::from(b'E') {
                self.next_char();
                self.lex_exponent()?;
                return Ok(Token::Float);
            }
        } else if start == i32::from(b'.') {
            self.lex_dot_number()?;
            return Ok(Token::Float);
        }

        while (48..=57).contains(&self.lookahead) || self.lookahead == i32::from(b'_') {
            self.next_char();
        }

        if self.lookahead == i32::from(b'e') || self.lookahead == i32::from(b'E') {
            self.next_char();
            self.lex_exponent()?;
            return Ok(Token::Float);
        } else if self.lookahead == i32::from(b'.') {
            self.next_char();
            if self.lookahead == i32::from(b'_') {
                return Err(self.lex_error_here(error::invalid_separator_position()));
            }
            if !(48..=57).contains(&self.lookahead) {
                self.backup();
                return Ok(Token::Int);
            }
            self.lex_dot_number()?;
            return Ok(Token::Float);
        }
        Ok(Token::Int)
    }

    fn lex_slash(&mut self) -> Result<Token, FormatError> {
        match self.lookahead {
            0x2f /* / */ => {
                self.next_char();
                let token = if self.lookahead == i32::from(b'/') {
                    Token::DocComment
                } else {
                    Token::LineComment
                };
                while self.lookahead != 0x0a && self.lookahead != 0x0d && self.lookahead != EOF {
                    self.next_char();
                }
                Ok(token)
            }
            0x2a /* * */ => {
                self.next_char();
                self.lex_block_comment()?;
                Ok(Token::BlockComment)
            }
            _ => Ok(Token::Div),
        }
    }

    fn lex_block_comment(&mut self) -> Result<(), FormatError> {
        if self.lookahead == EOF {
            return Err(self.unexpected_end_of_file());
        }
        let mut prev = self.next_char();
        while self.lookahead != EOF {
            if prev == i32::from(b'*') && self.lookahead == i32::from(b'/') {
                self.next_char();
                break;
            }
            prev = self.next_char();
        }
        if self.lookahead == EOF {
            return Err(self.unexpected_end_of_file());
        }
        Ok(())
    }

    fn lex_hex_number(&mut self) -> Result<(), FormatError> {
        if self.lookahead == i32::from(b'_') {
            return Err(self.lex_error_here(error::invalid_separator_position()));
        }
        if !is_hex(self.lookahead) {
            return Err(self.unexpected_char(self.lookahead, "hexadecimal number"));
        }
        while is_hex(self.lookahead) || self.lookahead == i32::from(b'_') {
            self.next_char();
        }
        Ok(())
    }

    fn lex_bin_number(&mut self) -> Result<(), FormatError> {
        if self.lookahead == i32::from(b'_') {
            return Err(self.lex_error_here(error::invalid_separator_position()));
        }
        if self.lookahead != i32::from(b'0') && self.lookahead != i32::from(b'1') {
            return Err(self.unexpected_char(self.lookahead, "binary number"));
        }
        while self.lookahead == i32::from(b'0')
            || self.lookahead == i32::from(b'1')
            || self.lookahead == i32::from(b'_')
        {
            self.next_char();
        }
        Ok(())
    }

    fn lex_oct_number(&mut self) -> Result<(), FormatError> {
        if self.lookahead == i32::from(b'_') {
            return Err(self.lex_error_here(error::invalid_separator_position()));
        }
        if !(48..=55).contains(&self.lookahead) {
            return Err(self.unexpected_char(self.lookahead, "octal number"));
        }
        while (48..=55).contains(&self.lookahead) || self.lookahead == i32::from(b'_') {
            self.next_char();
        }
        Ok(())
    }

    fn lex_exponent(&mut self) -> Result<(), FormatError> {
        if self.lookahead == i32::from(b'+') || self.lookahead == i32::from(b'-') {
            self.next_char();
        }
        if self.lookahead == i32::from(b'_') {
            return Err(self.lex_error_here(error::invalid_separator_position()));
        }
        if !(48..=57).contains(&self.lookahead) {
            return Err(self.unexpected_char(self.lookahead, "number"));
        }
        while (48..=57).contains(&self.lookahead) || self.lookahead == i32::from(b'_') {
            self.next_char();
        }
        Ok(())
    }

    fn lex_dot_number(&mut self) -> Result<(), FormatError> {
        if self.lookahead == i32::from(b'_') {
            return Err(self.lex_error_here(error::invalid_separator_position()));
        }
        while (48..=57).contains(&self.lookahead) || self.lookahead == i32::from(b'_') {
            self.next_char();
        }
        if self.lookahead == i32::from(b'e') || self.lookahead == i32::from(b'E') {
            self.next_char();
            self.lex_exponent()?;
        }
        Ok(())
    }

    fn lex_shebang(&mut self) -> Token {
        loop {
            self.next_char();
            if self.lookahead == 0x0a || self.lookahead == 0x0d || self.lookahead == EOF {
                break;
            }
        }
        Token::Shebang
    }

    fn next_char(&mut self) -> i32 {
        let tmp = self.lookahead;
        self.cursor += 1;
        if self.cursor >= self.size {
            self.lookahead = EOF;
        } else {
            self.lookahead = i32::from(self.source[self.cursor]);
        }
        if tmp == 0x0a {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        tmp
    }

    fn backup(&mut self) {
        self.cursor -= 1;
        self.lookahead = i32::from(self.source[self.cursor]);
        self.col -= 1;
    }

    fn backup_n(&mut self, amount: usize) {
        self.cursor -= amount;
        self.col -= amount;
        self.lookahead = i32::from(self.source[self.cursor]);
    }

    fn lex_error_here(&self, message: String) -> FormatError {
        let length = usize::from(self.lookahead != EOF);
        let char_index = if self.lookahead == EOF {
            self.cursor.saturating_sub(1)
        } else {
            self.cursor
        };
        FormatError::Lexer {
            message,
            span: Span { char_index, length },
        }
    }

    fn lex_error_at(&self, message: String, char_index: usize, length: usize) -> FormatError {
        FormatError::Lexer {
            message,
            span: Span { char_index, length },
        }
    }

    fn lex_error_span(&self, message: String, span: Span) -> FormatError {
        FormatError::Lexer { message, span }
    }

    fn unexpected_char(&self, got: i32, did_you_mean: &str) -> FormatError {
        if got == EOF {
            self.lex_error_here(error::unexpected_character("EOF", did_you_mean))
        } else {
            let s = unit_to_string(got as u16);
            self.lex_error_here(error::unexpected_character(&s, did_you_mean))
        }
    }

    fn unexpected_char_3(&self, got: &str, opt1: &str, opt2: &str, opt3: &str) -> FormatError {
        self.lex_error_here(error::unexpected_character_3(got, opt1, opt2, opt3))
    }

    fn unexpected_end_of_file(&self) -> FormatError {
        self.lex_error_at(error::unexpected_end_of_file(), self.cursor, 0)
    }
}

fn is_hex(code: i32) -> bool {
    (48..=57).contains(&code) || (97..=102).contains(&code) || (65..=70).contains(&code)
}

fn unit_to_string(unit: u16) -> String {
    char::from_u32(u32::from(unit))
        .unwrap_or(char::REPLACEMENT_CHARACTER)
        .to_string()
}

fn get_keyword_or_identifier(keyword: &str) -> Token {
    match keyword {
        "_" => Token::Underscore,
        "abstract" => Token::Abstract,
        "amends" => Token::Amends,
        "as" => Token::As,
        "case" => Token::Case,
        "class" => Token::Class,
        "const" => Token::Const,
        "delete" => Token::Delete,
        "else" => Token::Else,
        "extends" => Token::Extends,
        "external" => Token::External,
        "false" => Token::False,
        "fixed" => Token::Fixed,
        "for" => Token::For,
        "function" => Token::Function,
        "hidden" => Token::Hidden,
        "if" => Token::If,
        "import" => Token::Import,
        "in" => Token::In,
        "is" => Token::Is,
        "let" => Token::Let,
        "local" => Token::Local,
        "module" => Token::Module,
        "new" => Token::New,
        "nothing" => Token::Nothing,
        "null" => Token::Null,
        "open" => Token::Open,
        "out" => Token::Out,
        "outer" => Token::Outer,
        "override" => Token::Override,
        "protected" => Token::Protected,
        "read" => Token::Read,
        "record" => Token::Record,
        "super" => Token::Super,
        "switch" => Token::Switch,
        "this" => Token::This,
        "throw" => Token::Throw,
        "trace" => Token::Trace,
        "true" => Token::True,
        "typealias" => Token::TypeAlias,
        "unknown" => Token::Unknown,
        "vararg" => Token::Vararg,
        "when" => Token::When,
        _ => Token::Identifier,
    }
}
