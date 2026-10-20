//! Shared incremental parser, following the pinned Pkl 0.32 grammar.
use crate::diagnostic::{Diagnostic, DiagnosticKind};
use crate::source::Span;
use crate::syntax::{BinaryOp, Expr, ExprKind, Member, Module, Parameter, Type, TypeKind, UnaryOp};

fn is_keyword(name: &str) -> bool {
    matches!(
        name,
        "abstract"
            | "amends"
            | "as"
            | "class"
            | "const"
            | "else"
            | "extends"
            | "external"
            | "false"
            | "fixed"
            | "for"
            | "function"
            | "hidden"
            | "if"
            | "import"
            | "in"
            | "is"
            | "let"
            | "local"
            | "module"
            | "new"
            | "nothing"
            | "null"
            | "open"
            | "out"
            | "outer"
            | "read"
            | "super"
            | "this"
            | "throw"
            | "trace"
            | "true"
            | "typealias"
            | "unknown"
            | "when"
            | "protected"
            | "override"
            | "record"
            | "delete"
            | "case"
            | "switch"
            | "vararg"
    )
}

#[derive(Clone, Debug, PartialEq)]
enum Kind {
    Name(String),
    Int(String),
    Float(String),
    QuotedName(String),
    Operator(&'static str),
    String(String),
    StringTemplate(Vec<StringPart>),
    Symbol(char),
    Eof,
}

#[derive(Clone, Debug, PartialEq)]
enum StringPart {
    Text(String),
    Escape(String),
    Newline,
    Continuation,
    Expression(Span),
}

#[derive(Clone, Debug)]
struct Token {
    kind: Kind,
    span: Span,
}

#[derive(Clone)]
struct Lexer<'a> {
    source: &'a str,
    cursor: usize,
    string_depth: usize,
}

impl Lexer<'_> {
    fn next(&mut self) -> Result<Token, Diagnostic> {
        loop {
            while self
                .peek()
                .is_some_and(|ch| matches!(ch, ' ' | '\t' | '\n' | '\r' | '\x0c'))
            {
                self.advance();
            }
            if self.source[self.cursor..].starts_with("//") {
                while self.peek().is_some_and(|ch| ch != '\n' && ch != '\r') {
                    self.advance();
                }
            } else if self.source[self.cursor..].starts_with("/*") {
                let start = self.cursor;
                self.cursor += 2;
                let Some(end) = self.source[self.cursor..].find("*/") else {
                    return Err(Diagnostic::new(
                        DiagnosticKind::Syntax,
                        Span::new(start, self.source.len()),
                        "Unterminated block comment.",
                    ));
                };
                self.cursor += end + 2;
            } else {
                break;
            }
        }
        let start = self.cursor;
        let Some(ch) = self.advance() else {
            return Ok(Token {
                kind: Kind::Eof,
                span: Span::new(start, start),
            });
        };
        let kind = if ch == '`' {
            let begin = self.cursor;
            while self
                .peek()
                .is_some_and(|ch| ch != '`' && ch != '\n' && ch != '\r')
            {
                self.advance();
            }
            let end = self.cursor;
            if self.advance() != Some('`') {
                return Err(Diagnostic::new(
                    DiagnosticKind::Syntax,
                    Span::new(start, self.cursor),
                    "Missing backtick delimiter.",
                ));
            }
            Kind::QuotedName(self.source[begin..end].to_owned())
        } else if ch.is_ascii_alphabetic() || ch == '_' || ch == '$' {
            while self
                .peek()
                .is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '$')
            {
                self.advance();
            }
            Kind::Name(self.source[start..self.cursor].to_owned())
        } else if ch.is_ascii_digit()
            || (ch == '.' && self.peek().is_some_and(|ch| ch.is_ascii_digit()))
        {
            self.number(start, ch)?
        } else if ch == '"' || ch == '#' {
            self.string(start, ch)?
        } else if let Some(operator) = [
            "**", "~/", "==", "!=", "<=", ">=", "&&", "||", "??", "->", "!!", "?.", "|>",
        ]
        .into_iter()
        .find(|operator| self.source[start..].starts_with(operator))
        {
            self.cursor = start + operator.len();
            Kind::Operator(operator)
        } else if "=(){};.,+-*/%<>!:?|[]".contains(ch) {
            Kind::Symbol(ch)
        } else {
            return Err(Diagnostic::new(
                DiagnosticKind::Unsupported,
                Span::new(start, self.cursor),
                format!("M1 token `{ch}` is unsupported."),
            ));
        };
        Ok(Token {
            kind,
            span: Span::new(start, self.cursor),
        })
    }

    fn string(&mut self, start: usize, first: char) -> Result<Kind, Diagnostic> {
        if self.string_depth >= 32 {
            return Err(Diagnostic::new(
                DiagnosticKind::Unsupported,
                Span::new(start, self.cursor),
                "String interpolation nesting limit exceeded.",
            ));
        }
        self.string_depth += 1;
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
        let delimiter = format!(
            "{}{}",
            if multiline { "\"\"\"" } else { "\"" },
            "#".repeat(hashes)
        );
        let escape = format!("\\{}", "#".repeat(hashes));
        let mut parts = Vec::new();
        let mut text = String::new();
        loop {
            if self.source[self.cursor..].starts_with(&delimiter) {
                self.cursor += delimiter.len();
                if !text.is_empty() {
                    parts.push(StringPart::Text(std::mem::take(&mut text)));
                }
                break;
            }
            let position = self.cursor;
            let Some(ch) = self.advance() else {
                return Err(Diagnostic::new(
                    DiagnosticKind::Syntax,
                    Span::new(start, self.cursor),
                    format!("Missing `{delimiter}` delimiter."),
                ));
            };
            if ch == '\n' || ch == '\r' {
                if !multiline {
                    return Err(Diagnostic::new(
                        DiagnosticKind::Syntax,
                        Span::new(position, self.cursor),
                        "String content must remain on one line.",
                    ));
                }
                if ch == '\r' && self.peek() == Some('\n') {
                    self.advance();
                }
                if !text.is_empty() {
                    parts.push(StringPart::Text(std::mem::take(&mut text)));
                }
                parts.push(StringPart::Newline);
            } else if ch == '\\'
                && self.source[position..].starts_with(&escape)
                && self.source[position + 1..]
                    .chars()
                    .take_while(|ch| *ch == '#')
                    .count()
                    == hashes
            {
                self.cursor = position + escape.len();
                if !text.is_empty() {
                    parts.push(StringPart::Text(std::mem::take(&mut text)));
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
                        let begin = self.cursor;
                        let mut nesting = 1;
                        loop {
                            let token = self.next()?;
                            match token.kind {
                                Kind::Symbol('(') => nesting += 1,
                                Kind::Symbol(')') => {
                                    nesting -= 1;
                                    if nesting == 0 {
                                        parts.push(StringPart::Expression(Span::new(
                                            begin,
                                            token.span.start,
                                        )));
                                        break;
                                    }
                                }
                                Kind::Eof => {
                                    return Err(Diagnostic::new(
                                        DiagnosticKind::Syntax,
                                        Span::new(begin, self.cursor),
                                        "Missing `)` delimiter in interpolation.",
                                    ));
                                }
                                _ => {}
                            }
                        }
                        continue;
                    }
                    '\n' | '\r' if multiline => {
                        if escaped == '\r' && self.peek() == Some('\n') {
                            self.advance();
                        }
                        parts.push(StringPart::Continuation);
                        continue;
                    }
                    _ => {
                        return Err(Diagnostic::new(
                            DiagnosticKind::Syntax,
                            Span::new(position, self.cursor),
                            "Invalid string escape.",
                        ));
                    }
                };
                parts.push(StringPart::Escape(decoded));
            } else {
                text.push(ch);
            }
        }
        if multiline {
            parts = multiline_parts(parts, Span::new(start, self.cursor))?;
        }
        let mut rendered = Vec::new();
        let mut text = String::new();
        for part in parts {
            match part {
                StringPart::Text(value) | StringPart::Escape(value) => text.push_str(&value),
                StringPart::Newline => text.push('\n'),
                StringPart::Continuation => {}
                StringPart::Expression(span) => {
                    if !text.is_empty() {
                        rendered.push(StringPart::Text(std::mem::take(&mut text)));
                    }
                    rendered.push(StringPart::Expression(span));
                }
            }
        }
        self.string_depth -= 1;
        if rendered.is_empty() {
            Ok(Kind::String(text))
        } else {
            if !text.is_empty() {
                rendered.push(StringPart::Text(text));
            }
            Ok(Kind::StringTemplate(rendered))
        }
    }

    fn number(&mut self, start: usize, first: char) -> Result<Kind, Diagnostic> {
        if first == '0' {
            let radix = match self.peek() {
                Some('x' | 'X') => Some(16),
                Some('o' | 'O') => Some(8),
                Some('b' | 'B') => Some(2),
                _ => None,
            };
            if let Some(radix) = radix {
                self.advance();
                if !self.peek().is_some_and(|ch| ch.is_digit(radix)) {
                    return Err(Diagnostic::new(
                        DiagnosticKind::Syntax,
                        Span::new(start, self.cursor),
                        "Expected digits after numeric base prefix.",
                    ));
                }
                while self
                    .peek()
                    .is_some_and(|ch| ch.is_digit(radix) || ch == '_')
                {
                    self.advance();
                }
                return Ok(Kind::Int(self.source[start..self.cursor].to_owned()));
            }
        }
        while self
            .peek()
            .is_some_and(|ch| ch.is_ascii_digit() || ch == '_')
        {
            self.advance();
        }
        let mut float = first == '.';
        if !float && self.peek() == Some('.') {
            let next = self.source[self.cursor + 1..].chars().next();
            if next == Some('_') {
                return Err(Diagnostic::new(
                    DiagnosticKind::Syntax,
                    Span::new(start, self.cursor + 2),
                    "Invalid numeric separator position.",
                ));
            }
            if next.is_some_and(|ch| ch.is_ascii_digit()) {
                float = true;
                self.advance();
                while self
                    .peek()
                    .is_some_and(|ch| ch.is_ascii_digit() || ch == '_')
                {
                    self.advance();
                }
            }
        }
        if matches!(self.peek(), Some('e' | 'E')) {
            float = true;
            self.advance();
            if matches!(self.peek(), Some('+' | '-')) {
                self.advance();
            }
            if !self.peek().is_some_and(|ch| ch.is_ascii_digit()) {
                return Err(Diagnostic::new(
                    DiagnosticKind::Syntax,
                    Span::new(start, self.cursor),
                    "Expected exponent digits.",
                ));
            }
            while self
                .peek()
                .is_some_and(|ch| ch.is_ascii_digit() || ch == '_')
            {
                self.advance();
            }
        }
        let text = self.source[start..self.cursor].to_owned();
        Ok(if float {
            Kind::Float(text)
        } else {
            Kind::Int(text)
        })
    }

    fn peek(&self) -> Option<char> {
        self.source[self.cursor..].chars().next()
    }

    fn advance(&mut self) -> Option<char> {
        let ch = self.peek()?;
        self.cursor += ch.len_utf8();
        Some(ch)
    }
}

#[derive(Clone)]
struct Parser<'a> {
    lexer: Lexer<'a>,
    token: Token,
    depth: usize,
    previous_end: usize,
    expectation: Option<String>,
    expressions: usize,
}

impl<'a> Parser<'a> {
    fn new(source: &'a str) -> Result<Self, Diagnostic> {
        let mut lexer = Lexer {
            source,
            cursor: 0,
            string_depth: 0,
        };
        let token = lexer.next()?;
        Ok(Self {
            lexer,
            token,
            depth: 0,
            previous_end: 0,
            expectation: None,
            expressions: 0,
        })
    }

    fn interpolation(source: &'a str, span: Span) -> Result<Self, Diagnostic> {
        let mut lexer = Lexer {
            source: &source[..span.end],
            cursor: span.start,
            string_depth: 0,
        };
        let token = lexer.next()?;
        Ok(Self {
            lexer,
            token,
            depth: 0,
            previous_end: span.start,
            expectation: None,
            expressions: 0,
        })
    }

    fn advance(&mut self) -> Result<Token, Diagnostic> {
        let next = self.lexer.next()?;
        self.previous_end = self.token.span.end;
        Ok(std::mem::replace(&mut self.token, next))
    }

    fn is_symbol(&self, expected: char) -> bool {
        self.token.kind == Kind::Symbol(expected)
    }

    fn is_operator(&self, expected: &str) -> bool {
        matches!(self.token.kind, Kind::Operator(op) if op == expected)
    }

    fn same_line(&self) -> bool {
        !self.lexer.source[self.previous_end..self.token.span.start].contains(['\n', '\r'])
    }

    fn is_name(&self, expected: &str) -> bool {
        matches!(&self.token.kind, Kind::Name(name) if name == expected)
    }

    fn unexpected(&self, expected: Option<&str>) -> Diagnostic {
        let eof = self.token.kind == Kind::Eof;
        let token = if eof {
            "EOF"
        } else {
            &self.lexer.source[self.token.span.start..self.token.span.end]
        };
        let message = match expected {
            Some(expected) => format!("Unexpected token `{token}`. Expected `{expected}`."),
            None if eof => "Unexpected end of file.".to_owned(),
            None => format!("Unexpected token `{token}`."),
        };
        let synthetic = eof
            || (expected.is_some()
                && self.lexer.source[self.previous_end..self.token.span.start].contains('\n'));
        let mut diagnostic = Diagnostic::new(
            DiagnosticKind::Syntax,
            if synthetic {
                Span::new(self.previous_end, self.previous_end)
            } else {
                self.token.span
            },
            message,
        );
        if synthetic {
            diagnostic.highlight_length_utf16 = Some(1);
        }
        diagnostic
    }

    fn expect(&mut self, expected: char) -> Result<Token, Diagnostic> {
        if !self.is_symbol(expected) {
            return Err(self.unexpected(Some(&expected.to_string())));
        }
        self.advance()
    }

    fn identifier(&mut self) -> Result<(String, Span), Diagnostic> {
        let (Kind::Name(name) | Kind::QuotedName(name)) = &self.token.kind else {
            return Err(self.unexpected(Some("identifier")));
        };
        if matches!(self.token.kind, Kind::Name(_)) && (is_keyword(name) || name == "_") {
            return Err(Diagnostic::new(
                DiagnosticKind::Unsupported,
                self.token.span,
                "M1 keyword cannot be used as an identifier.",
            ));
        }
        let name = name.clone();
        let token = self.advance()?;
        Ok((name, token.span))
    }

    fn members(&mut self, in_object: bool) -> Result<Vec<Member>, Diagnostic> {
        let mut members = Vec::new();
        while self.token.kind != Kind::Eof && !(in_object && self.is_symbol('}')) {
            let start = self.token.span.start;
            let local = self.is_name("local");
            if local {
                self.advance()?;
            }
            let method = self.is_name("function");
            if method {
                self.advance()?;
            }
            let (name, _) = self.identifier()?;
            let (expr, ty) = if method {
                let parameters = self.parameters()?;
                let return_type = self.type_annotation()?;
                let header_span = Span::new(start, self.previous_end);
                self.expect('=')?;
                let body = self.expression_expected(0, in_object.then_some('}'))?;
                let span = Span::new(start, body.span.end);
                (
                    Expr {
                        kind: ExprKind::Function {
                            header_span,
                            parameters,
                            return_type,
                            body: Box::new(body),
                            method: true,
                        },
                        span,
                    },
                    None,
                )
            } else {
                let ty = self.type_annotation()?;
                self.expect('=')?;
                (self.expression_expected(0, in_object.then_some('}'))?, ty)
            };
            let end = expr.span.end;
            members.push(Member {
                name,
                expr,
                span: Span::new(start, end),
                local,
                ty,
            });
            if self.is_symbol(';') {
                self.advance()?;
            }
        }
        Ok(members)
    }

    fn expression_expected(
        &mut self,
        precedence: u8,
        expectation: Option<char>,
    ) -> Result<Expr, Diagnostic> {
        self.expression_with_expectation(precedence, expectation.map(|ch| ch.to_string()))
    }

    fn expression_with_expectation(
        &mut self,
        precedence: u8,
        expectation: Option<String>,
    ) -> Result<Expr, Diagnostic> {
        let previous = std::mem::replace(&mut self.expectation, expectation);
        let result = self.expression(precedence);
        self.expectation = previous;
        result
    }

    fn expression(&mut self, minimum_precedence: u8) -> Result<Expr, Diagnostic> {
        // Bound parser recursion before consuming hostile nesting. This is an
        // explicit prototype limitation, never successful partial parsing.
        self.expressions += 1;
        if self.depth >= 32 || self.expressions > 512 {
            return Err(Diagnostic::new(
                DiagnosticKind::Unsupported,
                self.token.span,
                "M1 expression nesting or count limit exceeded.",
            ));
        }
        self.depth += 1;
        let result = self.expression_inner(minimum_precedence);
        self.depth -= 1;
        result
    }

    fn expression_inner(&mut self, minimum_precedence: u8) -> Result<Expr, Diagnostic> {
        let mut left = self.primary()?;
        loop {
            self.expressions += 1;
            if self.expressions > 512 {
                return Err(Diagnostic::new(
                    DiagnosticKind::Unsupported,
                    self.token.span,
                    "M1 expression count limit exceeded.",
                ));
            }
            if self.is_symbol('.') || self.is_operator("?.") {
                let nullable = self.is_operator("?.");
                self.advance()?;
                let (name, span) = self.identifier()?;
                let start = left.span.start;
                left = Expr {
                    kind: if nullable {
                        ExprKind::NullableMember(Box::new(left), name)
                    } else {
                        ExprKind::Member(Box::new(left), name)
                    },
                    span: Span::new(start, span.end),
                };
                continue;
            }
            if self.is_operator("!!") {
                let end = self.advance()?.span.end;
                let span = Span::new(left.span.start, end);
                left = Expr {
                    kind: ExprKind::NonNull(Box::new(left)),
                    span,
                };
                continue;
            }
            if (self.is_name("is") || self.is_name("as")) && minimum_precedence <= 6 {
                let cast = self.is_name("as");
                self.advance()?;
                let ty = self.type_expression()?;
                let span = Span::new(left.span.start, ty.span.end);
                left = Expr {
                    kind: if cast {
                        ExprKind::TypeCast {
                            value: Box::new(left),
                            ty,
                        }
                    } else {
                        ExprKind::TypeCheck {
                            value: Box::new(left),
                            ty,
                        }
                    },
                    span,
                };
                continue;
            }
            if self.is_symbol('{') {
                // Pkl requires an explicitly parenthesized amendment parent.
                if !self.lexer.source[left.span.start..left.span.end].starts_with('(') {
                    return Err(Diagnostic::new(
                        DiagnosticKind::Unsupported,
                        self.token.span,
                        "M1 amendment parent must be parenthesized.",
                    ));
                }
                left = self.object(Some(left))?;
                continue;
            }
            if self.is_symbol('(') && self.same_line() {
                if !matches!(
                    left.kind,
                    ExprKind::Name(_) | ExprKind::Member(_, _) | ExprKind::NullableMember(_, _)
                ) || (matches!(left.kind, ExprKind::Name(_))
                    && self.lexer.source[left.span.start..left.span.end].starts_with('('))
                {
                    return Err(Diagnostic::new(
                        DiagnosticKind::Syntax,
                        self.token.span,
                        "Invalid token at position. Expected a class, typealias, method, or property.",
                    ));
                }
                let arguments = self.arguments()?;
                let span = Span::new(left.span.start, self.previous_end);
                left = Expr {
                    kind: ExprKind::Call {
                        callee: Box::new(left),
                        arguments,
                    },
                    span,
                };
                continue;
            }
            let (precedence, op, right_associative) = match self.token.kind {
                Kind::Operator("??") => (1, BinaryOp::NullCoalesce, true),
                Kind::Operator("||") => (3, BinaryOp::Or, false),
                Kind::Operator("&&") => (4, BinaryOp::And, false),
                Kind::Operator("==") => (5, BinaryOp::Equal, false),
                Kind::Operator("!=") => (5, BinaryOp::NotEqual, false),
                Kind::Symbol('<') => (7, BinaryOp::Less, false),
                Kind::Operator("<=") => (7, BinaryOp::LessEqual, false),
                Kind::Symbol('>') => (7, BinaryOp::Greater, false),
                Kind::Operator(">=") => (7, BinaryOp::GreaterEqual, false),
                Kind::Symbol('+') => (8, BinaryOp::Add, false),
                Kind::Symbol('-') => {
                    if !self.same_line() {
                        break;
                    }
                    (8, BinaryOp::Subtract, false)
                }
                Kind::Symbol('*') => (9, BinaryOp::Multiply, false),
                Kind::Symbol('/') => (9, BinaryOp::Divide, false),
                Kind::Operator("~/") => (9, BinaryOp::IntegerDivide, false),
                Kind::Symbol('%') => (9, BinaryOp::Remainder, false),
                Kind::Operator("**") => (10, BinaryOp::Power, true),
                _ => break,
            };
            if precedence < minimum_precedence {
                break;
            }
            self.advance()?;
            let right = self.expression(precedence + u8::from(!right_associative))?;
            let span = Span::new(left.span.start, right.span.end);
            left = Expr {
                kind: ExprKind::Binary {
                    op,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                span,
            };
        }
        Ok(left)
    }

    fn object(&mut self, parent: Option<Expr>) -> Result<Expr, Diagnostic> {
        let start = parent
            .as_ref()
            .map_or(self.token.span.start, |expr| expr.span.start);
        self.expect('{')?;
        let members = self.members(true)?;
        if self.token.kind == Kind::Eof {
            let mut error = self.unexpected(None);
            error.message = "Missing `}` delimiter.".to_owned();
            return Err(error);
        }
        let end = self.expect('}')?.span.end;
        Ok(Expr {
            kind: ExprKind::Object {
                parent: parent.map(Box::new),
                members,
            },
            span: Span::new(start, end),
        })
    }

    fn parameters(&mut self) -> Result<Vec<Parameter>, Diagnostic> {
        self.expect('(')?;
        let mut parameters = Vec::new();
        while !self.is_symbol(')') {
            let (name, span) = if self.is_name("_") {
                let token = self.advance()?;
                ("_".to_owned(), token.span)
            } else {
                self.identifier()?
            };
            let ty = self.type_annotation()?;
            let end = ty.as_ref().map_or(span.end, |ty| ty.span.end);
            parameters.push(Parameter {
                name,
                ty,
                span: Span::new(span.start, end),
            });
            if !self.is_symbol(',') {
                break;
            }
            self.advance()?;
        }
        self.expect(')')?;
        Ok(parameters)
    }

    fn arguments(&mut self) -> Result<Vec<Expr>, Diagnostic> {
        self.expect('(')?;
        let mut arguments = Vec::new();
        while !self.is_symbol(')') {
            arguments.push(self.expression_expected(0, None)?);
            if !self.is_symbol(',') {
                break;
            }
            self.advance()?;
        }
        self.expect(')')?;
        Ok(arguments)
    }

    fn type_annotation(&mut self) -> Result<Option<Type>, Diagnostic> {
        if !self.is_symbol(':') {
            return Ok(None);
        }
        self.advance()?;
        self.type_expression().map(Some)
    }

    fn type_expression(&mut self) -> Result<Type, Diagnostic> {
        if self.depth >= 32 {
            return Err(Diagnostic::new(
                DiagnosticKind::Unsupported,
                self.token.span,
                "Type nesting limit exceeded.",
            ));
        }
        self.depth += 1;
        let result = self.type_inner();
        self.depth -= 1;
        result
    }

    fn type_inner(&mut self) -> Result<Type, Diagnostic> {
        let start = self.token.span.start;
        let mut default = None;
        if self.is_symbol('*') {
            self.advance()?;
            default = Some(0);
        }
        let first = self.type_atom()?;
        if !self.is_symbol('|') {
            if default.is_some() {
                return Err(self.unexpected(Some("|")));
            }
            return Ok(first);
        }
        let mut types = vec![first];
        while self.is_symbol('|') {
            self.advance()?;
            if self.is_symbol('*') {
                if default.is_some() {
                    return Err(self.unexpected(Some("single union default")));
                }
                default = Some(types.len());
                self.advance()?;
            }
            types.push(self.type_atom()?);
        }
        Ok(Type {
            span: Span::new(start, self.previous_end),
            kind: TypeKind::Union { types, default },
        })
    }

    fn type_atom(&mut self) -> Result<Type, Diagnostic> {
        let start = self.token.span.start;
        let kind = match self.token.kind.clone() {
            Kind::Name(name) if matches!(name.as_str(), "unknown" | "nothing" | "module") => {
                self.advance()?;
                match name.as_str() {
                    "unknown" => TypeKind::Unknown,
                    "nothing" => TypeKind::Nothing,
                    _ => TypeKind::Module,
                }
            }
            Kind::Name(_) | Kind::QuotedName(_) => {
                let (name, _) = self.identifier()?;
                let mut path = vec![name];
                while self.is_symbol('.') {
                    self.advance()?;
                    path.push(self.identifier()?.0);
                }
                let mut arguments = Vec::new();
                if self.is_symbol('<') {
                    self.advance()?;
                    while !self.is_symbol('>') {
                        arguments.push(self.type_expression()?);
                        if !self.is_symbol(',') {
                            break;
                        }
                        self.advance()?;
                    }
                    self.expect('>')?;
                }
                TypeKind::Named { path, arguments }
            }
            Kind::String(value) => {
                self.advance()?;
                TypeKind::String(value)
            }
            Kind::Symbol('(') => {
                self.advance()?;
                let mut parameters = Vec::new();
                while !self.is_symbol(')') {
                    parameters.push(self.type_expression()?);
                    if !self.is_symbol(',') {
                        break;
                    }
                    self.advance()?;
                }
                self.expect(')')?;
                if self.is_operator("->") {
                    self.advance()?;
                    let return_type = Box::new(self.type_expression()?);
                    return Ok(Type {
                        span: Span::new(start, return_type.span.end),
                        kind: TypeKind::Function {
                            parameters,
                            return_type,
                        },
                    });
                }
                if parameters.len() != 1 {
                    return Err(self.unexpected(Some("->")));
                }
                parameters.pop().unwrap().kind
            }
            _ => return Err(self.unexpected(Some("type"))),
        };
        let mut ty = Type {
            kind,
            span: Span::new(start, self.previous_end),
        };
        loop {
            if self.is_symbol('?') {
                let end = self.advance()?.span.end;
                ty = Type {
                    kind: TypeKind::Nullable(Box::new(ty)),
                    span: Span::new(start, end),
                };
            } else if self.is_symbol('(') && self.same_line() {
                let constraints = self.arguments()?;
                ty = Type {
                    kind: TypeKind::Constrained {
                        base: Box::new(ty),
                        constraints,
                    },
                    span: Span::new(start, self.previous_end),
                };
            } else {
                break;
            }
        }
        Ok(ty)
    }

    fn primary(&mut self) -> Result<Expr, Diagnostic> {
        let span = self.token.span;
        let kind = match self.token.kind.clone() {
            Kind::Int(text) => {
                let value = integer(&text, false, span)?;
                self.advance()?;
                ExprKind::Int(value)
            }
            Kind::Float(text) => {
                let value = text
                    .replace('_', "")
                    .parse::<f64>()
                    .map_err(|_| self.unexpected(Some("number")))?;
                self.advance()?;
                ExprKind::Float(value)
            }
            Kind::String(text) => {
                self.advance()?;
                ExprKind::String(text)
            }
            Kind::StringTemplate(parts) => {
                let mut expressions = Vec::new();
                for part in parts {
                    match part {
                        StringPart::Text(value) => expressions.push(Expr {
                            kind: ExprKind::String(value),
                            span,
                        }),
                        StringPart::Expression(range) => {
                            let mut parser = Self::interpolation(self.lexer.source, range)?;
                            let expression = parser.expression(0)?;
                            if parser.token.kind != Kind::Eof {
                                return Err(parser.unexpected(Some(")")));
                            }
                            expressions.push(expression);
                        }
                        _ => unreachable!("string lexer returns rendered parts"),
                    }
                }
                self.advance()?;
                ExprKind::InterpolatedString(expressions)
            }
            Kind::Name(name) if name == "true" || name == "false" => {
                self.advance()?;
                ExprKind::Bool(name == "true")
            }
            Kind::Name(name) if name == "null" => {
                self.advance()?;
                ExprKind::Null
            }
            Kind::Name(name) if name == "if" => {
                self.advance()?;
                self.expect('(')?;
                let condition = self.expression_expected(0, Some(')'))?;
                self.expect(')')?;
                let then_branch = self.expression_with_expectation(0, Some("else".to_owned()))?;
                if !self.is_name("else") {
                    return Err(self.unexpected(Some("else")));
                }
                self.advance()?;
                let else_branch = self.expression(0)?;
                let end = else_branch.span.end;
                return Ok(Expr {
                    kind: ExprKind::If {
                        condition: Box::new(condition),
                        then_branch: Box::new(then_branch),
                        else_branch: Box::new(else_branch),
                    },
                    span: Span::new(span.start, end),
                });
            }
            Kind::Name(name) if name == "outer" || name == "module" => {
                self.advance()?;
                if name == "outer" {
                    ExprKind::Outer
                } else {
                    ExprKind::Module
                }
            }
            Kind::Name(name) if name == "read" => {
                self.advance()?;
                let arguments = self.arguments()?;
                let end = self.previous_end;
                return Ok(Expr {
                    kind: ExprKind::Call {
                        callee: Box::new(Expr {
                            kind: ExprKind::Name(name),
                            span,
                        }),
                        arguments,
                    },
                    span: Span::new(span.start, end),
                });
            }
            Kind::Name(name) if name == "throw" => {
                self.advance()?;
                self.expect('(')?;
                let value = self.expression_expected(0, Some(')'))?;
                let end = self.expect(')')?.span.end;
                return Ok(Expr {
                    kind: ExprKind::Throw(Box::new(value)),
                    span: Span::new(span.start, end),
                });
            }
            Kind::Name(name) if name == "this" => {
                self.advance()?;
                ExprKind::This
            }
            Kind::Name(name) if name == "super" => {
                self.advance()?;
                if !self.is_symbol('.') {
                    return Err(self.unexpected(Some(".")));
                }
                ExprKind::Super
            }
            Kind::Name(name) if name == "new" => {
                self.advance()?;
                let mut expr = self.object(None)?;
                expr.span.start = span.start;
                return Ok(expr);
            }
            Kind::Name(name) if name == "let" => {
                self.advance()?;
                self.expect('(')?;
                let (name, _) = self.identifier()?;
                self.expect('=')?;
                let value = self.expression_expected(0, Some(')'))?;
                self.expect(')')?;
                let body = self.expression_expected(0, None)?;
                let end = body.span.end;
                return Ok(Expr {
                    kind: ExprKind::Let {
                        name,
                        value: Box::new(value),
                        body: Box::new(body),
                    },
                    span: Span::new(span.start, end),
                });
            }
            Kind::QuotedName(name) => {
                self.advance()?;
                ExprKind::Name(name)
            }
            Kind::Name(name) => {
                if is_keyword(&name) || name == "_" {
                    return Err(Diagnostic::new(
                        DiagnosticKind::Unsupported,
                        span,
                        format!("M1 expression `{name}` is unsupported."),
                    ));
                }
                self.advance()?;
                ExprKind::Name(name)
            }
            Kind::Symbol('(') => {
                let mut candidate = self.clone();
                if let Ok(parameters) = candidate.parameters()
                    && candidate.is_operator("->")
                {
                    let header_span = Span::new(span.start, candidate.previous_end);
                    candidate.advance()?;
                    *self = candidate;
                    let body = self.expression(0)?;
                    let end = body.span.end;
                    return Ok(Expr {
                        kind: ExprKind::Function {
                            header_span,
                            parameters,
                            return_type: None,
                            body: Box::new(body),
                            method: false,
                        },
                        span: Span::new(span.start, end),
                    });
                }
                self.advance()?;
                let mut expr = self.expression_expected(0, Some(')'))?;
                let end = self.expect(')')?.span.end;
                expr.span = Span::new(span.start, end);
                return Ok(expr);
            }
            Kind::Symbol('-' | '!') => {
                let negate = self.is_symbol('-');
                self.advance()?;
                if negate && let Kind::Int(text) = &self.token.kind {
                    let value = integer(text, true, Span::new(span.start, self.token.span.end))?;
                    let end = self.advance()?.span.end;
                    return Ok(Expr {
                        kind: ExprKind::Int(value),
                        span: Span::new(span.start, end),
                    });
                }
                let operand = self.expression(11)?;
                let end = operand.span.end;
                return Ok(Expr {
                    kind: ExprKind::Unary {
                        op: if negate {
                            UnaryOp::Negate
                        } else {
                            UnaryOp::Not
                        },
                        operand: Box::new(operand),
                    },
                    span: Span::new(span.start, end),
                });
            }
            Kind::Symbol('[') => {
                return Err(Diagnostic::new(
                    DiagnosticKind::Unsupported,
                    span,
                    "M1 token `[` is unsupported.",
                ));
            }
            Kind::Eof => return Err(self.unexpected(None)),
            _ => {
                let expected = self.expectation.clone();
                return Err(self.unexpected(expected.as_deref()));
            }
        };
        Ok(Expr { kind, span })
    }
}

fn multiline_parts(mut parts: Vec<StringPart>, span: Span) -> Result<Vec<StringPart>, Diagnostic> {
    let error = |message| Diagnostic::new(DiagnosticKind::Syntax, span, message);
    if parts.first() != Some(&StringPart::Newline) {
        return Err(error("String content must begin on a new line."));
    }
    if parts.len() == 1 {
        return Ok(Vec::new());
    }
    let indent = match parts.last() {
        Some(StringPart::Newline) => String::new(),
        Some(StringPart::Text(text))
            if text.chars().all(|ch| ch == ' ' || ch == '\t')
                && parts.get(parts.len() - 2) == Some(&StringPart::Newline) =>
        {
            let indent = text.clone();
            parts.pop();
            indent
        }
        _ => return Err(error("Closing string delimiter must begin on a new line.")),
    };
    parts.pop(); // final source newline, which is not string content
    if parts.is_empty() {
        return Ok(Vec::new());
    }
    parts.remove(0); // opening newline
    let mut line_start = true;
    for part in &mut parts {
        match part {
            StringPart::Text(text) => {
                if line_start {
                    let Some(stripped) = text.strip_prefix(&indent) else {
                        return Err(error("String indentation must match the last line."));
                    };
                    *text = stripped.to_owned();
                }
                line_start = false;
            }
            StringPart::Newline | StringPart::Continuation => line_start = true,
            StringPart::Escape(_) => {
                if line_start && !indent.is_empty() {
                    return Err(error("String indentation must match the last line."));
                }
                line_start = false;
            }
            StringPart::Expression(_) => {}
        }
    }
    Ok(parts)
}

fn integer(text: &str, negative: bool, span: Span) -> Result<i64, Diagnostic> {
    let text = text.replace('_', "");
    let (digits, radix) = if text.starts_with("0x") || text.starts_with("0X") {
        (&text[2..], 16)
    } else if text.starts_with("0b") || text.starts_with("0B") {
        (&text[2..], 2)
    } else if text.starts_with("0o") || text.starts_with("0O") {
        (&text[2..], 8)
    } else {
        (text.as_str(), 10)
    };
    let value = i128::from_str_radix(digits, radix)
        .ok()
        .and_then(|value| i64::try_from(if negative { -value } else { value }).ok());
    value.ok_or_else(|| {
        Diagnostic::new(
            DiagnosticKind::IntegerRange,
            span,
            "M1 integer literal is outside the signed 64-bit range.",
        )
    })
}

pub fn parse_module(source: &str) -> Result<Module, Diagnostic> {
    let mut parser = Parser::new(source)?;
    let members = parser.members(false)?;
    Ok(Module { members })
}

pub fn parse_expression(source: &str) -> Result<Expr, Diagnostic> {
    let mut parser = Parser::new(source)?;
    let expression = parser.expression(0)?;
    if parser.token.kind != Kind::Eof {
        return Err(parser.unexpected(Some("end of file")));
    }
    Ok(expression)
}
