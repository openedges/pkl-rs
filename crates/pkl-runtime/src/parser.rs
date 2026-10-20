//! A bounded parser for the M1 experiments, not the complete Pkl grammar.
use crate::diagnostic::{Diagnostic, DiagnosticKind};
use crate::source::Span;
use crate::syntax::{BinaryOp, Expr, ExprKind, Member, Module};

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
    String(String),
    Symbol(char),
    Eof,
}

#[derive(Clone, Debug)]
struct Token {
    kind: Kind,
    span: Span,
}

struct Lexer<'a> {
    source: &'a str,
    cursor: usize,
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
                while self.peek().is_some_and(|ch| ch != '\n') {
                    self.advance();
                }
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
        let kind = if ch.is_ascii_alphabetic() || ch == '_' {
            while self
                .peek()
                .is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_')
            {
                self.advance();
            }
            Kind::Name(self.source[start..self.cursor].to_owned())
        } else if ch.is_ascii_digit() {
            while self.peek().is_some_and(|ch| ch.is_ascii_digit()) {
                self.advance();
            }
            Kind::Int(self.source[start..self.cursor].to_owned())
        } else if ch == '"' {
            let mut value = String::new();
            loop {
                let Some(ch) = self.advance() else {
                    return Err(Diagnostic::new(
                        DiagnosticKind::Syntax,
                        Span::new(start, self.cursor),
                        "Missing `\"` delimiter.",
                    ));
                };
                match ch {
                    '"' => break,
                    '\n' | '\r' => {
                        return Err(Diagnostic::new(
                            DiagnosticKind::Unsupported,
                            Span::new(start, self.cursor),
                            "M1 only supports single-line strings.",
                        ));
                    }
                    '\\' => {
                        let escaped = match self.advance() {
                            Some('n') => '\n',
                            Some('r') => '\r',
                            Some('t') => '\t',
                            Some('"') => '"',
                            Some('\\') => '\\',
                            _ => {
                                return Err(Diagnostic::new(
                                    DiagnosticKind::Unsupported,
                                    Span::new(start, self.cursor),
                                    "M1 string escape is unsupported.",
                                ));
                            }
                        };
                        value.push(escaped);
                    }
                    ch => value.push(ch),
                }
            }
            Kind::String(value)
        } else if "=(){};.,+-*/".contains(ch) {
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

    fn peek(&self) -> Option<char> {
        self.source[self.cursor..].chars().next()
    }

    fn advance(&mut self) -> Option<char> {
        let ch = self.peek()?;
        self.cursor += ch.len_utf8();
        Some(ch)
    }
}

struct Parser<'a> {
    lexer: Lexer<'a>,
    token: Token,
    depth: usize,
    previous_end: usize,
    expectation: Option<char>,
    expressions: usize,
}

impl<'a> Parser<'a> {
    fn new(source: &'a str) -> Result<Self, Diagnostic> {
        let mut lexer = Lexer { source, cursor: 0 };
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

    fn advance(&mut self) -> Result<Token, Diagnostic> {
        let next = self.lexer.next()?;
        self.previous_end = self.token.span.end;
        Ok(std::mem::replace(&mut self.token, next))
    }

    fn is_symbol(&self, expected: char) -> bool {
        self.token.kind == Kind::Symbol(expected)
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
        let Kind::Name(name) = &self.token.kind else {
            return Err(self.unexpected(Some("identifier")));
        };
        if is_keyword(name) || name == "_" {
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
            let (name, _) = self.identifier()?;
            self.expect('=')?;
            let expr = self.expression_expected(0, in_object.then_some('}'))?;
            let end = expr.span.end;
            members.push(Member {
                name,
                expr,
                span: Span::new(start, end),
                local,
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
        let previous = self.expectation;
        self.expectation = expectation;
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
            if self.is_symbol('.') {
                self.advance()?;
                let (name, span) = self.identifier()?;
                let start = left.span.start;
                left = Expr {
                    kind: ExprKind::Member(Box::new(left), name),
                    span: Span::new(start, span.end),
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
            let (precedence, op) = match self.token.kind {
                Kind::Symbol('+') => (1, BinaryOp::Add),
                Kind::Symbol('-') => {
                    if self.lexer.source[self.previous_end..self.token.span.start].contains('\n') {
                        break;
                    }
                    (1, BinaryOp::Subtract)
                }
                Kind::Symbol('*') => (2, BinaryOp::Multiply),
                Kind::Symbol('/') => (2, BinaryOp::Divide),
                _ => break,
            };
            if precedence < minimum_precedence {
                break;
            }
            self.advance()?;
            let right = self.expression(precedence + 1)?;
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

    fn primary(&mut self) -> Result<Expr, Diagnostic> {
        let span = self.token.span;
        let kind = match self.token.kind.clone() {
            Kind::Int(text) => {
                let value = text.parse::<i64>().map_err(|_| {
                    Diagnostic::new(
                        DiagnosticKind::IntegerRange,
                        span,
                        "M1 integer literal is outside the signed 64-bit range.",
                    )
                })?;
                self.advance()?;
                ExprKind::Int(value)
            }
            Kind::String(text) => {
                self.advance()?;
                ExprKind::String(text)
            }
            Kind::Name(name) if name == "true" || name == "false" => {
                self.advance()?;
                ExprKind::Bool(name == "true")
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
                self.advance()?;
                let mut expr = self.expression_expected(0, Some(')'))?;
                let end = self.expect(')')?.span.end;
                expr.span = Span::new(span.start, end);
                return Ok(expr);
            }
            Kind::Symbol('-') => {
                self.advance()?;
                if let Kind::Int(text) = &self.token.kind {
                    let value = format!("-{text}").parse::<i64>().map_err(|_| {
                        Diagnostic::new(
                            DiagnosticKind::IntegerRange,
                            span,
                            "M1 integer literal is outside the signed 64-bit range.",
                        )
                    })?;
                    let end = self.advance()?.span.end;
                    return Ok(Expr {
                        kind: ExprKind::Int(value),
                        span: Span::new(span.start, end),
                    });
                }
                return Err(Diagnostic::new(
                    DiagnosticKind::Unsupported,
                    span,
                    "M1 unary minus only supports integer literals.",
                ));
            }
            Kind::Eof => return Err(self.unexpected(None)),
            _ => {
                let expected = self.expectation.map(|ch| ch.to_string());
                return Err(self.unexpected(expected.as_deref()));
            }
        };
        Ok(Expr { kind, span })
    }
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
