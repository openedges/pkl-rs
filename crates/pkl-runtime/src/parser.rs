//! Shared incremental parser, following the pinned Pkl 0.32 grammar.
use crate::diagnostic::{Diagnostic, DiagnosticKind};
use crate::source::Span;
use crate::syntax::{
    BinaryOp, Expr, ExprKind, Member, Modifier, ModifierKind, Module, ModuleDeclaration,
    ModuleParent, ModuleParentKind, ObjectMember, Parameter, Type, TypeKind, TypeParameter,
    UnaryOp, Variance, modifier_flags,
};

mod unicode_identifier;

fn identifier_contains(ranges: &[(u32, u32)], character: char) -> bool {
    use std::cmp::Ordering;
    let point = character as u32;
    ranges
        .binary_search_by(|&(start, end)| {
            if point < start {
                Ordering::Greater
            } else if point > end {
                Ordering::Less
            } else {
                Ordering::Equal
            }
        })
        .is_ok()
}

pub(crate) fn is_identifier_start(character: char) -> bool {
    identifier_contains(unicode_identifier::START_RANGES, character)
}

pub(crate) fn is_identifier_part(character: char) -> bool {
    identifier_contains(unicode_identifier::PART_RANGES, character)
}

/// The pinned renderer uses code points here, unlike the UTF-16 lexer.
pub(crate) fn is_regular_identifier(name: &str) -> bool {
    if name == "_" || is_keyword(name) {
        return false;
    }
    let mut characters = name.chars();
    characters.next().is_some_and(|character| {
        identifier_contains(unicode_identifier::REGULAR_START_RANGES, character)
    }) && characters
        .all(|character| identifier_contains(unicode_identifier::REGULAR_PART_RANGES, character))
}

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
        } else if is_identifier_start(ch) {
            while self.peek().is_some_and(is_identifier_part) {
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
            "...?", "...", "[[", "**", "~/", "==", "!=", "<=", ">=", "&&", "||", "??", "->", "!!",
            "?.", "|>",
        ]
        .into_iter()
        .find(|operator| self.source[start..].starts_with(operator))
        {
            self.cursor = start + operator.len();
            Kind::Operator(operator)
        } else if "=(){};.,+-*/%<>!:?|[]".contains(ch) {
            Kind::Symbol(ch)
        } else if !ch.is_ascii() {
            let mut diagnostic = Diagnostic::new(
                DiagnosticKind::Syntax,
                Span::new(start, self.cursor),
                format!(
                    "Invalid identifier `{}`.",
                    if ch as u32 > 0xffff {
                        '?'.to_string()
                    } else {
                        ch.to_string()
                    }
                ),
            );
            diagnostic.highlight_length_utf16 = Some(1);
            return Err(diagnostic);
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
                            // Scan delimiters without letting a later lexical failure
                            // supersede an earlier expression syntax error. Parsing
                            // the captured range below reports failures in order.
                            let token = match self.next() {
                                Ok(token) => token,
                                Err(_) => continue,
                            };
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
                                    let mut parser = Parser::interpolation(
                                        self.source,
                                        Span::new(begin, self.cursor),
                                    )?;
                                    parser.expression_expected(0, Some(')'))?;
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

#[derive(Clone, Copy, Eq, PartialEq)]
enum MemberContext {
    Module,
    Class,
    Object,
}

#[derive(Clone)]
struct Parser<'a> {
    lexer: Lexer<'a>,
    token: Token,
    depth: usize,
    previous_end: usize,
    expectation: Option<String>,
    expressions: usize,
    pending: Vec<Diagnostic>,
    member_path: Vec<String>,
    folded_integer_spans: std::collections::BTreeMap<Span, Span>,
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
            pending: Vec::new(),
            member_path: Vec::new(),
            folded_integer_spans: std::collections::BTreeMap::new(),
        })
    }

    fn interpolation(source: &'a str, span: Span) -> Result<Self, Diagnostic> {
        let mut lexer = Lexer {
            source: &source[..(span.end + 1).min(source.len())],
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
            pending: Vec::new(),
            member_path: Vec::new(),
            folded_integer_spans: std::collections::BTreeMap::new(),
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
                && self.lexer.source[self.previous_end..self.token.span.start]
                    .contains(['\n', '\r']));
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

    fn modifiers(&mut self) -> Result<Vec<Modifier>, Diagnostic> {
        let mut modifiers = Vec::new();
        loop {
            let kind = if self.is_name("abstract") {
                ModifierKind::Abstract
            } else if self.is_name("open") {
                ModifierKind::Open
            } else if self.is_name("local") {
                ModifierKind::Local
            } else if self.is_name("hidden") {
                ModifierKind::Hidden
            } else if self.is_name("external") {
                ModifierKind::External
            } else if self.is_name("fixed") {
                ModifierKind::Fixed
            } else if self.is_name("const") {
                ModifierKind::Const
            } else {
                break;
            };
            let span = self.advance()?.span;
            modifiers.push(Modifier { kind, span });
        }
        Ok(modifiers)
    }

    fn module_declaration(
        &mut self,
        modifiers: Vec<Modifier>,
    ) -> Result<Option<ModuleDeclaration>, Diagnostic> {
        let start = modifiers
            .first()
            .map_or(self.token.span.start, |modifier| modifier.span.start);
        let name = if self.is_name("module") {
            self.advance()?;
            let mut path = vec![self.identifier()?.0];
            while self.is_symbol('.') {
                self.advance()?;
                path.push(self.identifier()?.0);
            }
            Some(path)
        } else {
            None
        };
        let parent = if self.is_name("extends") || self.is_name("amends") {
            let kind = if self.is_name("extends") {
                ModuleParentKind::Extends
            } else {
                ModuleParentKind::Amends
            };
            let begin = self.advance()?.span.start;
            let Kind::String(uri) = self.token.kind.clone() else {
                return Err(self.unexpected(Some("string constant")));
            };
            let end = self.advance()?.span.end;
            Some(ModuleParent {
                kind,
                uri,
                span: Span::new(begin, end),
            })
        } else {
            None
        };
        Ok(if name.is_some() || parent.is_some() {
            Some(ModuleDeclaration {
                name,
                parent,
                modifiers,
                span: Span::new(start, self.previous_end),
            })
        } else {
            None
        })
    }

    fn member_list(
        &mut self,
        context: MemberContext,
        mut initial: Option<Vec<Modifier>>,
    ) -> Result<Vec<Member>, Diagnostic> {
        let mut members = Vec::new();
        while self.token.kind != Kind::Eof
            && !(context != MemberContext::Module && self.is_symbol('}'))
        {
            members.push(self.member(context, initial.take())?);
            if self.is_symbol(';') {
                self.advance()?;
            }
        }
        Ok(members)
    }

    fn member(
        &mut self,
        context: MemberContext,
        initial: Option<Vec<Modifier>>,
    ) -> Result<Member, Diagnostic> {
        let modifiers = match initial {
            Some(modifiers) => modifiers,
            None => self.modifiers()?,
        };
        let start = modifiers
            .first()
            .map_or(self.token.span.start, |modifier| modifier.span.start);
        let local = modifiers
            .iter()
            .any(|modifier| modifier.kind == ModifierKind::Local);
        let method = self.is_name("function");
        let class = self.is_name("class");
        let alias = self.is_name("typealias");
        // Property/method lexical declarations use syntactic locality;
        // class/typealias value members use the computed modifier flags.
        let local = if class || alias {
            modifier_flags(&modifiers) & ModifierKind::Local.flag() != 0
        } else {
            local
        };
        if (class || alias) && context != MemberContext::Module {
            return Err(self.unexpected(Some("property or method")));
        }
        if method || class || alias {
            self.advance()?;
        }
        if context == MemberContext::Object
            && modifiers.is_empty()
            && (self.is_name("unknown") || self.is_name("nothing"))
        {
            return Err(self.unexpected(Some("}")));
        }
        let (name, name_span) = self.identifier()?;
        self.member_path.push(name.clone());
        let (expr, ty) = if class {
            let type_parameters = self.type_parameters()?;
            let superclass = if self.is_name("extends") {
                self.advance()?;
                Some(self.type_expression()?)
            } else {
                None
            };
            let header_span = Span::new(start, self.previous_end);
            let class_members = if self.is_symbol('{') {
                self.advance()?;
                let members = self.member_list(MemberContext::Class, None)?;
                if self.token.kind == Kind::Eof {
                    let mut error = self.unexpected(None);
                    error.message = "Missing `}` delimiter.".to_owned();
                    return Err(error);
                }
                self.expect('}')?;
                members
            } else {
                Vec::new()
            };
            (
                Expr {
                    kind: ExprKind::Class {
                        type_parameters,
                        superclass,
                        members: class_members,
                        header_span,
                    },
                    span: Span::new(start, self.previous_end),
                },
                None,
            )
        } else if alias {
            let type_parameters = self.type_parameters()?;
            self.expect('=')?;
            let ty = self.type_expression()?;
            let span = Span::new(start, ty.span.end);
            (
                Expr {
                    kind: ExprKind::TypeAlias {
                        type_parameters,
                        ty,
                    },
                    span,
                },
                None,
            )
        } else if method {
            let type_parameters = self.type_parameters()?;
            let parameters = self.parameters()?;
            let return_type = self.type_annotation()?;
            let header_span = Span::new(start, self.previous_end);
            let body = if self.is_symbol('=') {
                self.advance()?;
                self.expression_expected(0, (context == MemberContext::Object).then_some('}'))?
            } else if context != MemberContext::Object {
                Expr {
                    kind: ExprKind::Uninitialized,
                    span: header_span,
                }
            } else {
                return Err(self.unexpected(Some("=")));
            };
            let span = Span::new(start, body.span.end);
            (
                Expr {
                    kind: ExprKind::Function {
                        type_parameters,
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
            let expr = if self.is_symbol('=') {
                self.advance()?;
                self.expression_expected(0, (context == MemberContext::Object).then_some('}'))?
            } else if self.is_symbol('{') && ty.is_none() {
                let parent = Expr {
                    kind: ExprKind::Member(
                        Box::new(Expr {
                            kind: ExprKind::Super,
                            span: name_span,
                        }),
                        name.clone(),
                    ),
                    span: name_span,
                };
                self.object(Some(parent))?
            } else if context != MemberContext::Object && ty.is_some() {
                Expr {
                    kind: ExprKind::Uninitialized,
                    span: Span::new(name_span.start, self.previous_end),
                }
            } else {
                return Err(self.unexpected(Some("=")));
            };
            (expr, ty)
        };
        let end = expr.span.end;
        self.member_path.pop();
        Ok(Member {
            name,
            name_span,
            expr,
            span: Span::new(start, end),
            local,
            ty,
            modifiers,
        })
    }

    fn type_parameters(&mut self) -> Result<Vec<TypeParameter>, Diagnostic> {
        if !self.is_symbol('<') {
            return Ok(Vec::new());
        }
        self.advance()?;
        let mut parameters = Vec::new();
        loop {
            let start = self.token.span.start;
            let variance = if self.is_name("in") {
                self.advance()?;
                Variance::In
            } else if self.is_name("out") {
                self.advance()?;
                Variance::Out
            } else {
                Variance::Invariant
            };
            let (name, span) = self.identifier()?;
            parameters.push(TypeParameter {
                name,
                variance,
                span: Span::new(start, span.end),
            });
            if !self.is_symbol(',') {
                break;
            }
            self.advance()?;
            if self.is_symbol('>') {
                break;
            }
        }
        self.expect('>')?;
        Ok(parameters)
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
            if self.is_symbol('[') && self.same_line() {
                self.advance()?;
                let key = self.expression_expected(0, Some(']'))?;
                let end = self.expect(']')?.span.end;
                let span = Span::new(left.span.start, end);
                left = Expr {
                    kind: ExprKind::Subscript {
                        receiver: Box::new(left),
                        key: Box::new(key),
                    },
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
                Kind::Operator("|>") => (2, BinaryOp::Pipe, false),
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
        let members = self.object_members()?;
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

    fn object_members(&mut self) -> Result<Vec<ObjectMember>, Diagnostic> {
        let mut members = Vec::new();
        while !self.is_symbol('}') && self.token.kind != Kind::Eof {
            members.push(self.object_member()?);
            if self.is_symbol(';') {
                self.advance()?;
            }
        }
        Ok(members)
    }

    fn generator_body(&mut self) -> Result<Vec<ObjectMember>, Diagnostic> {
        self.expect('{')?;
        let members = self.object_members()?;
        self.expect('}')?;
        Ok(members)
    }

    fn object_member(&mut self) -> Result<ObjectMember, Diagnostic> {
        let start = self.token.span.start;
        if self.is_name("unknown") || self.is_name("nothing") {
            return Err(self.unexpected(Some("}")));
        }
        if self.is_name("when") {
            self.advance()?;
            self.expect('(')?;
            let condition = self.expression_expected(0, Some(')'))?;
            self.expect(')')?;
            let then_members = self.generator_body()?;
            let else_members = if self.is_name("else") {
                self.advance()?;
                self.generator_body()?
            } else {
                Vec::new()
            };
            return Ok(ObjectMember::When {
                condition,
                then_members,
                else_members,
                span: Span::new(start, self.previous_end),
            });
        }
        if self.is_name("for") {
            self.advance()?;
            self.expect('(')?;
            let mut parameters = vec![self.parameter()?];
            if self.is_symbol(',') {
                self.advance()?;
                parameters.push(self.parameter()?);
            }
            if !self.is_name("in") {
                return Err(self.unexpected(Some("in")));
            }
            self.advance()?;
            let iterable = self.expression_expected(0, Some(')'))?;
            self.expect(')')?;
            let members = self.generator_body()?;
            return Ok(ObjectMember::For {
                parameters,
                iterable,
                members,
                span: Span::new(start, self.previous_end),
            });
        }
        if self.is_operator("...") || self.is_operator("...?") {
            let nullable = self.is_operator("...?");
            self.advance()?;
            let value = self.expression_expected(0, Some('}'))?;
            return Ok(ObjectMember::Spread {
                span: Span::new(start, value.span.end),
                value,
                nullable,
            });
        }
        if self.is_symbol('[') || self.is_operator("[[") {
            let predicate = self.is_operator("[[");
            self.advance()?;
            let key = self.expression_with_expectation(
                0,
                Some(if predicate { "]]" } else { "]" }.to_owned()),
            )?;
            if predicate && !self.is_symbol(']') {
                return Err(self.unexpected(Some("]]")));
            }
            let close = self.expect(']')?;
            if predicate {
                if !self.is_symbol(']') {
                    let mut error = self.unexpected(Some("]]"));
                    error.span = close.span;
                    return Err(error);
                }
                if self.token.span.start != close.span.end {
                    return Err(Diagnostic::new(
                        DiagnosticKind::Syntax,
                        close.span,
                        format!(
                            "Unexpected token `{}`. Expected `]]`.",
                            &self.lexer.source[close.span.start..self.token.span.end]
                        ),
                    ));
                }
                self.advance()?;
            }
            let value = if self.is_symbol('=') {
                self.advance()?;
                self.expression_expected(0, Some('}'))?
            } else {
                let mut value = self.object(Some(Expr {
                    kind: ExprKind::EntryParent,
                    span: Span::new(start, self.previous_end),
                }))?;
                while self.is_symbol('{') {
                    value = self.object(Some(value))?;
                }
                value
            };
            let span = Span::new(start, value.span.end);
            return Ok(if predicate {
                ObjectMember::Predicate {
                    condition: key,
                    value,
                    span,
                }
            } else {
                ObjectMember::Entry { key, value, span }
            });
        }
        // A bare identifier starts a property only before `=`, `:`, or an
        // amendment body. In every other position it is an element expression.
        let property = match &self.token.kind {
            Kind::Name(name) if is_keyword(name) => matches!(
                name.as_str(),
                "function"
                    | "local"
                    | "const"
                    | "hidden"
                    | "fixed"
                    | "external"
                    | "abstract"
                    | "open"
            ),
            Kind::Name(_) | Kind::QuotedName(_) => matches!(
                self.lexer.clone().next()?.kind,
                Kind::Symbol('=' | ':' | '{')
            ),
            _ => false,
        };
        if property {
            return self
                .member(MemberContext::Object, None)
                .map(ObjectMember::Property);
        }
        let value = self.expression_expected(0, Some('}'))?;
        Ok(ObjectMember::Element {
            span: value.span,
            value,
        })
    }

    fn parameter(&mut self) -> Result<Parameter, Diagnostic> {
        let (name, span) = if self.is_name("_") {
            let token = self.advance()?;
            ("_".to_owned(), token.span)
        } else {
            self.identifier()?
        };
        let ty = self.type_annotation()?;
        let end = ty.as_ref().map_or(span.end, |ty| ty.span.end);
        Ok(Parameter {
            name,
            ty,
            span: Span::new(span.start, end),
        })
    }

    fn parameters(&mut self) -> Result<Vec<Parameter>, Diagnostic> {
        self.expect('(')?;
        let mut parameters = Vec::new();
        while !self.is_symbol(')') {
            parameters.push(self.parameter()?);
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

    fn integer_literal(&mut self, text: &str, negative: bool, span: Span) -> i64 {
        match integer(text, negative, span) {
            Ok(value) => value,
            Err(mut diagnostic) => {
                if !self.member_path.is_empty() {
                    diagnostic.member = Some(self.member_path.join(".").into());
                }
                self.pending.push(diagnostic);
                0 // Never escapes public parsing while pending errors exist.
            }
        }
    }

    fn primary(&mut self) -> Result<Expr, Diagnostic> {
        let span = self.token.span;
        let kind = match self.token.kind.clone() {
            Kind::Int(text) => {
                let value = self.integer_literal(&text, false, span);
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
                            parser.member_path.clone_from(&self.member_path);
                            let expression = parser.expression_expected(0, Some(')'))?;
                            parser.expect(')')?;
                            self.pending.append(&mut parser.pending);
                            self.folded_integer_spans
                                .append(&mut parser.folded_integer_spans);
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
                if !self.is_symbol('.') && !self.is_symbol('[') {
                    return Err(self.unexpected(Some(".")));
                }
                ExprKind::Super
            }
            Kind::Name(name) if name == "new" => {
                self.advance()?;
                if self.is_symbol('{') {
                    let mut expr = self.object(None)?;
                    expr.span.start = span.start;
                    return Ok(expr);
                }
                let ty = self.type_expression()?;
                self.expect('{')?;
                let members = self.object_members()?;
                let end = self.expect('}')?.span.end;
                return Ok(Expr {
                    kind: ExprKind::New { ty, members },
                    span: Span::new(span.start, end),
                });
            }
            Kind::Name(name) if name == "let" => {
                self.advance()?;
                self.expect('(')?;
                let name = if self.is_name("_") {
                    self.advance()?;
                    "_".to_owned()
                } else {
                    self.identifier()?.0
                };
                let ty = self.type_annotation()?;
                self.expect('=')?;
                let value = self.expression_expected(0, Some(')'))?;
                self.expect(')')?;
                let body = self.expression_expected(0, None)?;
                let end = body.span.end;
                return Ok(Expr {
                    kind: ExprKind::Let {
                        name,
                        ty,
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
                            type_parameters: Vec::new(),
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
                let expr = self.expression_expected(0, Some(')'))?;
                let end = self.expect(')')?.span.end;
                return Ok(Expr {
                    kind: ExprKind::Parenthesized(Box::new(expr)),
                    span: Span::new(span.start, end),
                });
            }
            Kind::Symbol('-' | '!') => {
                let negate = self.is_symbol('-');
                self.advance()?;
                if negate && let Kind::Int(text) = self.token.kind.clone() {
                    let mut following = self.clone();
                    following.advance()?;
                    if !following.is_symbol('.')
                        && !following.is_operator("?.")
                        && !following.is_operator("!!")
                        && !following.is_symbol('[')
                    {
                        let token_span = self.token.span;
                        let value = self.integer_literal(&text, true, token_span);
                        let end = self.advance()?.span.end;
                        self.folded_integer_spans
                            .insert(Span::new(span.start, end), token_span);
                        return Ok(Expr {
                            kind: ExprKind::Int(value),
                            span: Span::new(span.start, end),
                        });
                    }
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
    let literal = text;
    let text = text.replace('_', "");
    let (digits, radix) = if let Some(digits) = text.strip_prefix("0x") {
        (digits, 16)
    } else if let Some(digits) = text.strip_prefix("0b") {
        (digits, 2)
    } else if let Some(digits) = text.strip_prefix("0o") {
        (digits, 8)
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
            format!("Int literal `{literal}` is too large."),
        )
    })
}

pub fn parse_module(source: &str) -> Result<Module, Diagnostic> {
    let mut parser = Parser::new(source)?;
    let modifiers = parser.modifiers()?;
    let has_declaration =
        parser.is_name("module") || parser.is_name("extends") || parser.is_name("amends");
    let (declaration, initial) = if has_declaration {
        (parser.module_declaration(modifiers)?, None)
    } else {
        (None, Some(modifiers))
    };
    let members = parser.member_list(MemberContext::Module, initial)?;
    if let Some(mut error) = parser.pending.into_iter().next() {
        error.module_name = declaration
            .as_ref()
            .and_then(|declaration| declaration.name.as_ref())
            .map(|name| name.join(".").into_boxed_str());
        return Err(error);
    }
    Ok(Module {
        declaration,
        members,
        folded_integer_spans: parser.folded_integer_spans,
    })
}

pub fn parse_expression(source: &str) -> Result<Expr, Diagnostic> {
    let mut parser = Parser::new(source)?;
    let expression = parser.expression(0)?;
    if parser.token.kind != Kind::Eof {
        return Err(parser.unexpected(Some("end of file")));
    }
    if let Some(error) = parser.pending.into_iter().next() {
        return Err(error);
    }
    Ok(expression)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regular_identifiers_render_code_points_while_lexing_utf16_units() {
        for name in ["value", "$", "_name", "α\u{301}", "a\u{200d}", "\u{10400}"] {
            assert!(is_regular_identifier(name), "{name:?}");
        }
        for name in ["", "_", "class", "unknown", "read", "case", "1value", "a-b"] {
            assert!(!is_regular_identifier(name), "{name:?}");
        }
        assert!(parse_module("`\u{10400}` = 1").is_ok());
        let error = parse_module("\u{10400} = 1").unwrap_err();
        assert_eq!(error.message, "Invalid identifier `?`.");
    }
}
