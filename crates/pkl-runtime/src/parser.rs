//! Shared incremental parser, following the pinned Pkl 0.32 grammar.
use crate::diagnostic::{Diagnostic, DiagnosticKind, DiagnosticOrigin};
use crate::source::Span;
use crate::syntax::{
    BinaryOp, Expr, ExprKind, Member, Modifier, ModifierKind, Module, ModuleDeclaration,
    ModuleParent, ModuleParentKind, ObjectMember, Parameter, Type, TypeKind, TypeParameter,
    UnaryOp, Variance, modifier_flags,
};
use crate::value::PklString;

mod string_lexer;
mod string_parser;
mod type_parser;
mod unicode_escape;
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
            | "read?"
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
    String(crate::value::PklString),
    StringTemplate(Vec<StringPart>),
    DocComment,
    LineComment,
    BlockComment,
    StringStart(char),
    Symbol(char),
    Eof,
}

#[derive(Clone, Debug, PartialEq)]
enum StringPart {
    Text(String),
    Escape(crate::value::PklString),
    Value(crate::value::PklString),
    Unicode,
    Newline,
    Continuation,
    Expression(Span, Span),
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
    string_errors: string_lexer::StringErrors,
}

impl Lexer<'_> {
    fn next(&mut self) -> Result<Token, Diagnostic> {
        self.next_token(false)
    }

    fn next_token(&mut self, string_start_only: bool) -> Result<Token, Diagnostic> {
        self.next_with_comments(string_start_only, false)
    }

    fn next_with_comments(
        &mut self,
        string_start_only: bool,
        comments: bool,
    ) -> Result<Token, Diagnostic> {
        loop {
            while self
                .peek()
                .is_some_and(|ch| matches!(ch, ' ' | '\t' | '\n' | '\r' | '\x0c'))
            {
                self.advance();
            }
            if self.source[self.cursor..].starts_with("//") {
                let start = self.cursor;
                let doc = self.source[start..].starts_with("///");
                while self.peek().is_some_and(|ch| ch != '\n' && ch != '\r') {
                    self.advance();
                }
                if doc || comments {
                    return Ok(Token {
                        kind: if doc {
                            Kind::DocComment
                        } else {
                            Kind::LineComment
                        },
                        span: Span::new(start, self.cursor),
                    });
                }
            } else if self.source[self.cursor..].starts_with("/*") {
                let start = self.cursor;
                self.cursor += 2;
                let end = self.source[self.cursor..].find("*/");
                self.cursor = end.map_or(self.source.len(), |end| self.cursor + end + 2);
                // The pinned lexer reports EOF even when a block comment's
                // closing delimiter is the final source token.
                if self.cursor == self.source.len() {
                    return Err(Diagnostic::language(
                        DiagnosticOrigin::Lexer,
                        DiagnosticKind::Syntax,
                        Span::new(self.cursor, self.cursor),
                        "Unexpected end of file.",
                    ));
                }
                if comments {
                    return Ok(Token {
                        kind: Kind::BlockComment,
                        span: Span::new(start, self.cursor),
                    });
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
                return Err(Diagnostic::language(
                    DiagnosticOrigin::Lexer,
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
            // The pinned lexer makes `read?` one keyword only when adjacent.
            // A quoted name or a separated `?` retains its ordinary grammar.
            if &self.source[start..self.cursor] == "read" && self.peek() == Some('?') {
                self.advance();
            }
            Kind::Name(self.source[start..self.cursor].to_owned())
        } else if ch.is_ascii_digit()
            || (ch == '.' && self.peek().is_some_and(|ch| ch.is_ascii_digit()))
        {
            self.number(start, ch)?
        } else if ch == '"' || ch == '#' {
            if let Some((end, error)) = self.cached_string_error(start) {
                self.cursor = end;
                return Err(error);
            }
            if string_start_only {
                Kind::StringStart(ch)
            } else {
                self.string(start, ch)?
            }
        } else if let Some(operator) = [
            "...?", "...", "[[", "**", "~/", "==", "!=", "<=", ">=", "&&", "||", "??", "->", "!!",
            "?.", "|>",
        ]
        .into_iter()
        .find(|operator| self.source[start..].starts_with(operator))
        {
            self.cursor = start + operator.len();
            Kind::Operator(operator)
        } else if "=(){};.,+-*/%<>!:?|[]@".contains(ch) {
            Kind::Symbol(ch)
        } else if !ch.is_ascii() {
            let mut diagnostic = Diagnostic::language(
                DiagnosticOrigin::Lexer,
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
            if ch == '&' || ch == '~' {
                return Err(self.unexpected_unicode_character(
                    &ch.to_string(),
                    if ch == '&' { "&&" } else { "~/" },
                ));
            }
            return Err(Diagnostic::language(
                DiagnosticOrigin::Lexer,
                DiagnosticKind::Syntax,
                Span::new(start, self.cursor),
                format!("Invalid identifier `{ch}`."),
            ));
        };
        Ok(Token {
            kind,
            span: Span::new(start, self.cursor),
        })
    }

    fn numeric_separator(&self, offset: usize) -> Diagnostic {
        Diagnostic::language(
            DiagnosticOrigin::Lexer,
            DiagnosticKind::Syntax,
            Span::new(offset, offset + 1),
            "Unexpected separator character.\n\nThe separator character (`_`) cannot follow `0x`, `0b`, `.`, `e`, or 'E' in a number literal.",
        )
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
                if self.peek() == Some('_') {
                    return Err(self.numeric_separator(self.cursor));
                }
                if !self.peek().is_some_and(|ch| ch.is_digit(radix)) {
                    return Err(Diagnostic::language(
                        DiagnosticOrigin::Lexer,
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
                return Err(self.numeric_separator(self.cursor + 1));
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
            if self.peek() == Some('_') {
                return Err(self.numeric_separator(self.cursor));
            }
            if !self.peek().is_some_and(|ch| ch.is_ascii_digit()) {
                return Err(Diagnostic::language(
                    DiagnosticOrigin::Lexer,
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

struct ObjectBuild {
    start: usize,
    parameters: Vec<Parameter>,
    parent: Option<Expr>,
    ty: Option<Type>,
    members: Vec<ObjectMember>,
    missing_delimiter: bool,
    postfix: bool,
}

struct PropertyHead {
    start: usize,
    name: String,
    name_span: Span,
    local: bool,
    modifiers: Vec<Modifier>,
    ty: Option<Type>,
    method: Option<MethodHead>,
}

struct MethodHead {
    type_parameters: Vec<TypeParameter>,
    type_parameters_span: Option<Box<Span>>,
    parameters: Vec<Parameter>,
    return_type: Option<Type>,
    header_span: Span,
}

enum ObjectValue {
    Property(Box<PropertyHead>),
    Element(usize, Option<PklString>),
    Spread(usize, bool),
    Entry(usize, Box<Expr>, bool, bool, Option<PklString>),
}

enum ExpressionFrame {
    Group(usize),
    Throw(usize),
    Trace(usize),
    ReadResource(usize),
    ReadResourceGlob(usize),
    ReadResourceOrNull(usize),
    Unary(usize, UnaryOp),
    Binary(Expr, BinaryOp),
    Subscript(Expr),
    Arguments(Expr, Vec<Expr>),
    IfCondition(usize),
    IfThen(usize, Expr),
    IfElse(usize, Expr, Expr),
    LetValue(usize, String, Option<Type>),
    LetBody(usize, String, Option<Type>, Expr),
    Function(usize, Span, Vec<Parameter>),
    ObjectValue(Box<ObjectBuild>, ObjectValue),
    ObjectKey(Box<ObjectBuild>, usize, bool),
    WhenCondition(Box<ObjectBuild>, usize),
    WhenThen(Box<ObjectBuild>, usize, Expr),
    WhenElse(Box<ObjectBuild>, usize, Expr, Vec<ObjectMember>),
    ForIterable(Box<ObjectBuild>, usize, Vec<Parameter>),
    ForBody(Box<ObjectBuild>, usize, Vec<Parameter>, Expr),
}
struct ExpressionMachine {
    frames: Vec<(u8, Option<String>, ExpressionFrame)>,
    minimum: u8,
    current: Option<Expr>,
    object_build: Option<Box<ObjectBuild>>,
    object_root: bool,
    finish_expression: bool,
}

enum ExpressionFlow {
    Continue,
    Primary,
    Done,
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
    member_path: Vec<PklString>,
    entry_counts: std::collections::BTreeMap<PklString, usize>,
    folded_integer_spans: std::collections::BTreeMap<Span, Span>,
    interpolation_spans: std::collections::BTreeMap<Span, Span>,
}

impl<'a> Parser<'a> {
    fn new(source: &'a str) -> Result<Self, Diagnostic> {
        let mut lexer = Lexer {
            source,
            cursor: 0,
            string_errors: Default::default(),
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
            entry_counts: std::collections::BTreeMap::new(),
            folded_integer_spans: std::collections::BTreeMap::new(),
            interpolation_spans: std::collections::BTreeMap::new(),
        })
    }

    fn interpolation_with_errors(
        source: &'a str,
        span: Span,
        string_errors: string_lexer::StringErrors,
    ) -> Result<Self, Diagnostic> {
        let mut lexer = Lexer {
            source: &source[..(span.end + 1).min(source.len())],
            cursor: span.start,
            string_errors,
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
            entry_counts: std::collections::BTreeMap::new(),
            folded_integer_spans: std::collections::BTreeMap::new(),
            interpolation_spans: std::collections::BTreeMap::new(),
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

    fn diagnostic_token_span(&self) -> Span {
        if matches!(self.token.kind, Kind::String(_) | Kind::StringTemplate(_)) {
            let text = &self.lexer.source[self.token.span.start..self.token.span.end];
            let hashes = text.bytes().take_while(|&b| b == b'#').count();
            let quotes = if text[hashes..].starts_with("\"\"\"") {
                3
            } else {
                1
            };
            Span::new(
                self.token.span.start,
                self.token.span.start + hashes + quotes,
            )
        } else {
            self.token.span
        }
    }

    fn unexpected(&self, expected: Option<&str>) -> Diagnostic {
        let eof = self.token.kind == Kind::Eof;
        // Pinned Lexer emits STRING_START/STRING_MULTI_START before content.
        // Our lexer coalesces strings, so grammar errors must recover the
        // opening-delimiter token rather than display/highlight the full value.
        let token_span = self.diagnostic_token_span();
        let token = if eof {
            "EOF"
        } else {
            &self.lexer.source[token_span.start..token_span.end]
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
        let mut diagnostic = Diagnostic::language(
            DiagnosticOrigin::Parser,
            DiagnosticKind::Syntax,
            if synthetic {
                Span::new(self.previous_end, self.previous_end)
            } else {
                token_span
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
            if name == "read?" {
                return Err(Diagnostic::language(
                    DiagnosticOrigin::Parser,
                    DiagnosticKind::Syntax,
                    self.token.span,
                    "Keyword `read_question` is not allowed here.\n\nIf you must use this name as identifier, enclose it in backticks.",
                ));
            }
            return Err(Diagnostic::language(
                DiagnosticOrigin::Parser,
                DiagnosticKind::Syntax,
                self.token.span,
                format!(
                    "Keyword `{name}` is not allowed here.\n\nIf you must use this name as identifier, enclose it in backticks."
                ),
            ));
        }
        let name = name.clone();
        let token = self.advance()?;
        Ok((name, token.span))
    }

    fn doc_comment(&mut self) -> Result<bool, Diagnostic> {
        if self.token.kind != Kind::DocComment {
            return Ok(false);
        }
        let mut newlines = self.advance_comment()?;
        while matches!(
            self.token.kind,
            Kind::DocComment | Kind::LineComment | Kind::BlockComment
        ) {
            if newlines > 1 {
                if self.token.kind != Kind::DocComment {
                    self.advance_comment()?;
                }
                break;
            }
            newlines = self.advance_comment()?;
        }
        while matches!(self.token.kind, Kind::LineComment | Kind::BlockComment) {
            self.advance_comment()?;
        }
        Ok(true)
    }

    fn advance_comment(&mut self) -> Result<usize, Diagnostic> {
        self.previous_end = self.token.span.end;
        let mut gap_start = self.previous_end;
        loop {
            self.token = self.lexer.next_with_comments(false, true)?;
            if !self.is_symbol(';') {
                break;
            }
            // Pinned forceNextComment ignores semicolons and reports only the
            // final lexer's inter-token newlines, unlike normal forceNext.
            gap_start = self.token.span.end;
        }
        let gap = &self.lexer.source[gap_start..self.token.span.start];
        Ok(gap
            .bytes()
            .enumerate()
            .filter(|(i, b)| {
                *b == b'\n' || *b == b'\r' && gap.as_bytes().get(i + 1) != Some(&b'\n')
            })
            .count())
    }

    fn dangling_doc_comment(&self) -> Diagnostic {
        Diagnostic::language(
            DiagnosticOrigin::Parser,
            DiagnosticKind::Syntax,
            self.token.span,
            "Dangling documentation comment.\n\nDocumentation comments must be attached to modules, classes, typealiases, methods, or properties.",
        )
    }

    fn advance_constant(&mut self) -> Result<Token, Diagnostic> {
        let previous = self.token.clone();
        self.previous_end = previous.span.end;
        let mut token = self.lexer.next_token(true)?;
        if let Kind::StringStart(first) = token.kind {
            token.kind = self
                .lexer
                .string_in_context(token.span.start, first, true)?;
            token.span.end = self.lexer.cursor;
        }
        self.token = token;
        Ok(previous)
    }

    fn string_constant(&mut self) -> Result<(PklString, Span), Diagnostic> {
        match self.token.kind.clone() {
            Kind::String(text) => Ok((text, self.advance()?.span)),
            Kind::StringTemplate(parts) => {
                let span = parts
                    .iter()
                    .find_map(|part| match part {
                        StringPart::Expression(_, span) => Some(*span),
                        _ => None,
                    })
                    .unwrap_or(self.token.span);
                let prefix = &self.lexer.source[span.start..span.end];
                let width = 2 + prefix.bytes().skip(1).take_while(|&b| b == b'#').count();
                Err(Diagnostic::language(
                    DiagnosticOrigin::Parser,
                    DiagnosticKind::Syntax,
                    Span::new(span.start, span.start + width),
                    "String constant cannot have interpolated values.",
                ))
            }
            _ => Err(self.unexpected(Some("\""))),
        }
    }

    fn annotations(&mut self) -> Result<Vec<Expr>, Diagnostic> {
        let mut annotations = Vec::new();
        while self.is_symbol('@') {
            let start = self.advance()?.span.start;
            let ty = self.type_expression()?;
            let mut parameters = Vec::new();
            let members = if self.is_symbol('{') {
                let mut build =
                    self.start_object(None, None, self.token.span.start, true, false)?;
                parameters = std::mem::take(&mut build.parameters);
                let mut body = self.expression_machine(0, Some(build))?;
                Self::take_object_members(&mut body)
            } else {
                Vec::new()
            };
            annotations.push(Expr {
                kind: ExprKind::Annotation {
                    parameters,
                    ty,
                    members,
                },
                span: Span::new(start, self.previous_end),
            });
        }
        Ok(annotations)
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
        annotations: Vec<Expr>,
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
            let begin = self.advance_constant()?.span.start;
            let (uri, uri_span) = self.string_constant()?;
            let end = uri_span.end;
            let Some(uri) = uri.as_str() else {
                return Err(Diagnostic::unsupported(
                    DiagnosticOrigin::Parser,
                    "module-uri-utf16",
                    Span::new(begin, end),
                    "Module URIs containing unmatched UTF-16 surrogates are not implemented.",
                ));
            };
            Some(ModuleParent {
                kind,
                uri: uri.to_owned(),
                span: Span::new(begin, end),
            })
        } else {
            None
        };
        Ok(if name.is_some() || parent.is_some() {
            Some(ModuleDeclaration {
                annotations,
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
        mut initial: Option<(Vec<Modifier>, Vec<Expr>, bool)>,
    ) -> Result<Vec<Member>, Diagnostic> {
        let mut members = Vec::new();
        while self.token.kind != Kind::Eof
            && !(context != MemberContext::Module && self.is_symbol('}'))
        {
            if context == MemberContext::Module && self.is_name("import") {
                return Err(Diagnostic::language(
                    DiagnosticOrigin::Parser,
                    DiagnosticKind::Syntax,
                    self.token.span,
                    "Keyword `import` is not allowed here.\n\nIf you must use this name as identifier, enclose it in backticks.",
                ));
            }
            if !self.is_symbol('@')
                && !matches!(
                    self.token.kind,
                    Kind::Name(_) | Kind::QuotedName(_) | Kind::DocComment
                )
            {
                return Err(Diagnostic::language(
                    DiagnosticOrigin::Parser,
                    DiagnosticKind::Syntax,
                    self.diagnostic_token_span(),
                    "Invalid token at position. Expected a class, typealias, method, or property.",
                ));
            }
            members.push(self.member(context, initial.take())?);
            if self.is_symbol(';') {
                self.advance()?;
            }
        }
        Ok(members)
    }

    fn import(&mut self) -> Result<Member, Diagnostic> {
        let start = self.advance_constant()?.span.start;
        if self.is_symbol('*') {
            return Err(Diagnostic::unsupported(
                DiagnosticOrigin::Parser,
                "glob-module-import",
                Span::new(start, self.token.span.end),
                "Glob module imports are not implemented.",
            ));
        }
        let (uri, uri_span) = self.string_constant()?;
        let Some(uri) = uri.as_str() else {
            return Err(Diagnostic::unsupported(
                DiagnosticOrigin::Parser,
                "module-uri-utf16",
                uri_span,
                "Module URIs containing unmatched UTF-16 surrogates are not implemented.",
            ));
        };
        let uri = uri.to_owned();
        let (name, _) = if self.is_name("as") {
            self.advance()?;
            self.identifier()?
        } else {
            // Module URI loading is deliberately bounded separately from parsing.
            // Pkl's opaque standard-library URIs use the last dotted segment;
            // hierarchical module URIs use the filename without its extension.
            let name = if let Some(opaque) = uri.strip_prefix("pkl:") {
                opaque.rsplit('.').next().unwrap_or(opaque)
            } else {
                let path = uri.rsplit('#').next().unwrap_or(&uri);
                let filename = path.rsplit('/').next().unwrap_or(path);
                filename.rsplit_once('.').map_or(filename, |(name, _)| name)
            };
            (name.to_owned(), uri_span)
        };
        let span = Span::new(start, self.previous_end);
        Ok(Member {
            annotations: Vec::new(),
            name,
            name_span: span,
            expr: Expr {
                kind: ExprKind::Import { uri, uri_span },
                span,
            },
            span,
            local: true,
            ty: None,
            modifiers: vec![
                Modifier {
                    kind: ModifierKind::Local,
                    span,
                },
                Modifier {
                    kind: ModifierKind::Const,
                    span,
                },
            ],
            override_fixed_span: None,
            override_const_span: None,
        })
    }

    fn member(
        &mut self,
        context: MemberContext,
        initial: Option<(Vec<Modifier>, Vec<Expr>, bool)>,
    ) -> Result<Member, Diagnostic> {
        let (modifiers, annotations, _documented) = match initial {
            Some(initial) => initial,
            None => {
                let documented = self.doc_comment()?;
                let annotations = self.annotations()?;
                (self.modifiers()?, annotations, documented)
            }
        };
        if self.token.kind == Kind::DocComment {
            return Err(self.dangling_doc_comment());
        }
        if self.token.kind == Kind::Eof {
            return Err(Diagnostic::language(
                DiagnosticOrigin::Parser,
                DiagnosticKind::Syntax,
                self.token.span,
                "Unexpected end of file.",
            ));
        }
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
        self.member_path.push(name.clone().into());
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
                    error.message = "Missing `}` delimiter.".to_owned().into();
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
            let header_span = Span::new(start, self.previous_end);
            self.expect('=')?;
            let ty = self.type_expression()?;
            let span = Span::new(start, ty.span.end);
            (
                Expr {
                    kind: ExprKind::TypeAlias {
                        type_parameters,
                        ty,
                        header_span,
                    },
                    span,
                },
                None,
            )
        } else if method {
            let type_parameters_start = self.token.span.start;
            let type_parameters = self.type_parameters()?;
            let type_parameters_span = (!type_parameters.is_empty())
                .then(|| Box::new(Span::new(type_parameters_start, self.previous_end)));
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
                        type_parameters_span,
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
            } else if self.is_symbol('{') && ty.is_some() {
                return Err(Diagnostic::language(
                    DiagnosticOrigin::Parser,
                    DiagnosticKind::Syntax,
                    self.token.span,
                    "Properties with type annotations cannot have object bodies.\n\nTo define both a type annotation and an object body, try using assignment instead.\nFor example: `obj: Bird = new { ... }`.",
                ));
            } else if self.is_symbol('{') {
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
        let header_end = ty.as_ref().map_or(name_span.end, |ty| ty.span.end);
        let override_modifier_span = |word: &str| {
            let source = self.lexer.source;
            let begin = source[start..header_end]
                .find(word)
                .map(|offset| start + offset)
                // String.indexOf returns -1 when repeated flags carry into a
                // modifier absent from the authored header.
                .or_else(|| {
                    source[..start]
                        .char_indices()
                        .next_back()
                        .map(|(index, _)| index)
                })?;
            let mut end = begin;
            let mut units = 0;
            for character in source[begin..].chars() {
                if units >= word.len() {
                    break;
                }
                units += character.len_utf16();
                end += character.len_utf8();
            }
            Some(Span::new(begin, end))
        };
        let override_fixed_span = override_modifier_span("fixed");
        let override_const_span = override_modifier_span("const");
        Ok(Member {
            annotations,
            name,
            name_span,
            expr,
            span: Span::new(start, end),
            local,
            ty,
            modifiers,
            override_fixed_span,
            override_const_span,
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
        // Only unconverted grammar scopes consume the native call stack. The
        // ordinary Pratt expression machine below stores its continuations on
        // the heap; one shared work budget bounds both wide and deep inputs.
        if self.depth >= 32 {
            return Err(Diagnostic::limit(
                DiagnosticOrigin::Parser,
                "expression-scope-depth-32",
                self.token.span,
                "Expression scope nesting limit exceeded.",
            ));
        }
        self.depth += 1;
        let result = self.expression_inner(minimum_precedence);
        self.depth -= 1;
        result
    }

    fn enter_expression_scope(&mut self) -> Result<(), Diagnostic> {
        if self.depth >= 32 {
            return Err(Diagnostic::limit(
                DiagnosticOrigin::Parser,
                "expression-scope-depth-32",
                self.token.span,
                "Expression scope nesting limit exceeded.",
            ));
        }
        self.depth += 1;
        Ok(())
    }

    fn expression_step(&mut self) -> Result<(), Diagnostic> {
        self.expressions += 1;
        if self.expressions > 65_536 {
            return Err(Diagnostic::limit(
                DiagnosticOrigin::Parser,
                "expression-work-65536",
                self.token.span,
                "Expression work limit exceeded.",
            ));
        }
        Ok(())
    }

    fn expression_inner(&mut self, minimum_precedence: u8) -> Result<Expr, Diagnostic> {
        self.expression_machine(minimum_precedence, None)
    }

    fn expression_machine(
        &mut self,
        minimum_precedence: u8,
        initial_object: Option<Box<ObjectBuild>>,
    ) -> Result<Expr, Diagnostic> {
        self.interpolated_expression_machine(minimum_precedence, initial_object)
    }

    fn expression_machine_step(
        &mut self,
        machine: &mut ExpressionMachine,
    ) -> Result<ExpressionFlow, Diagnostic> {
        if let Some(mut build) = machine.object_build.take() {
            if self.is_symbol('}') || self.token.kind == Kind::Eof {
                if self.token.kind == Kind::Eof && build.missing_delimiter {
                    let mut error = self.unexpected(None);
                    error.message = "Missing `}` delimiter.".to_owned().into();
                    return Err(error);
                }
                let end = self.expect('}')?.span.end;
                if !build.parameters.is_empty() {
                    self.pending.push(Diagnostic::unsupported(
                        DiagnosticOrigin::DeferredAstValidation,
                        "parameterized-object-body",
                        Span::new(build.start, end),
                        "Parameterized object body evaluation is not implemented.",
                    ));
                }
                self.depth -= 1;
                machine.current = Some(Expr {
                    kind: if let Some(ty) = build.ty.take() {
                        ExprKind::New {
                            ty,
                            members: std::mem::take(&mut build.members),
                        }
                    } else {
                        ExprKind::Object {
                            parent: build.parent.take().map(Box::new),
                            members: std::mem::take(&mut build.members),
                        }
                    },
                    span: Span::new(build.start, end),
                });
                if machine.object_root && machine.frames.is_empty() {
                    return Ok(ExpressionFlow::Done);
                }
                if !build.postfix {
                    if self.is_symbol('{')
                        && machine.frames.last().is_some_and(|frame| {
                            matches!(
                                &frame.2,
                                ExpressionFrame::ObjectValue(
                                    _,
                                    ObjectValue::Entry(_, _, _, true, _)
                                )
                            )
                        })
                    {
                        let parent = machine.current.take().expect("entry amendment");
                        let start = parent.span.start;
                        machine.object_build =
                            Some(self.start_object(Some(parent), None, start, true, false)?);
                        return Ok(ExpressionFlow::Continue);
                    }
                    machine.finish_expression = true;
                }
            } else {
                let start = self.token.span.start;
                if self.is_name("class") || self.is_name("typealias") {
                    let mut error = Diagnostic::language(
                        DiagnosticOrigin::Parser,
                        DiagnosticKind::Syntax,
                        Span::new(self.previous_end, self.previous_end),
                        "Missing `}` delimiter.",
                    );
                    error.highlight_length_utf16 = Some(1);
                    return Err(error);
                }
                if self.is_name("unknown") || self.is_name("nothing") {
                    return Err(self.unexpected(Some("}")));
                }
                if self.is_name("when") {
                    self.advance()?;
                    self.expect('(')?;
                    machine.frames.push((
                        machine.minimum,
                        self.expectation.replace(")".into()),
                        ExpressionFrame::WhenCondition(build, start),
                    ));
                } else if self.is_name("for") {
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
                    machine.frames.push((
                        machine.minimum,
                        self.expectation.replace(")".into()),
                        ExpressionFrame::ForIterable(build, start, parameters),
                    ));
                } else if self.is_operator("...") || self.is_operator("...?") {
                    let nullable = self.is_operator("...?");
                    self.advance()?;
                    machine.frames.push((
                        machine.minimum,
                        self.expectation.replace("}".into()),
                        ExpressionFrame::ObjectValue(build, ObjectValue::Spread(start, nullable)),
                    ));
                } else if self.is_symbol('[') || self.is_operator("[[") {
                    let predicate = self.is_operator("[[");
                    self.advance()?;
                    machine.frames.push((
                        machine.minimum,
                        self.expectation
                            .replace(if predicate { "]]" } else { "]" }.into()),
                        ExpressionFrame::ObjectKey(build, start, predicate),
                    ));
                } else if self.object_property()? {
                    let head = self.object_property_head()?;
                    if self.is_symbol('=') {
                        self.advance()?;
                        machine.frames.push((
                            machine.minimum,
                            self.expectation.replace("}".into()),
                            ExpressionFrame::ObjectValue(
                                build,
                                ObjectValue::Property(Box::new(head)),
                            ),
                        ));
                    } else if self.is_symbol('{') && head.ty.is_none() && head.method.is_none() {
                        let parent = Expr {
                            kind: ExprKind::Member(
                                Box::new(Expr {
                                    kind: ExprKind::Super,
                                    span: head.name_span,
                                }),
                                head.name.clone(),
                            ),
                            span: head.name_span,
                        };
                        let child = self.start_object(
                            Some(parent),
                            None,
                            head.name_span.start,
                            true,
                            false,
                        )?;
                        machine.frames.push((
                            machine.minimum,
                            self.expectation.clone(),
                            ExpressionFrame::ObjectValue(
                                build,
                                ObjectValue::Property(Box::new(head)),
                            ),
                        ));
                        machine.object_build = Some(child);
                        machine.minimum = 0;
                        return Ok(ExpressionFlow::Continue);
                    } else {
                        return Err(self.unexpected(Some("=")));
                    }
                } else {
                    let previous = self.enter_entry_path(None);
                    machine.frames.push((
                        machine.minimum,
                        self.expectation.replace("}".into()),
                        ExpressionFrame::ObjectValue(build, ObjectValue::Element(start, previous)),
                    ));
                }
                machine.minimum = 0;
                return Ok(ExpressionFlow::Continue);
            }
        }
        if machine.current.is_none() {
            self.expression_step()?;
            let span = self.token.span;
            if self.is_symbol('(') {
                let mut candidate = self.clone();
                let mut head = self.clone();
                head.advance()?;
                let mut committed = head.is_name("_") || head.is_symbol(')');
                if (matches!(&head.token.kind, Kind::Name(name) if !is_keyword(name))
                    || matches!(head.token.kind, Kind::QuotedName(_)))
                    && !head.is_name("_")
                {
                    head.advance()?;
                    committed = head.is_symbol(',') || head.is_symbol(':');
                }
                let parameters = candidate.parameters();
                if committed && let Err(error) = &parameters {
                    return Err(error.clone());
                }
                if committed && parameters.is_ok() && !candidate.is_operator("->") {
                    return Err(candidate.unexpected(Some("->")));
                }
                if let Ok(parameters) = parameters
                    && candidate.is_operator("->")
                {
                    let header = Span::new(span.start, candidate.previous_end);
                    candidate.advance()?;
                    *self = candidate;
                    self.enter_expression_scope()?;
                    machine.frames.push((
                        machine.minimum,
                        self.expectation.clone(),
                        ExpressionFrame::Function(span.start, header, parameters),
                    ));
                    machine.minimum = 0;
                    return Ok(ExpressionFlow::Continue);
                } else {
                    self.advance()?;
                    machine.frames.push((
                        machine.minimum,
                        self.expectation.replace(")".into()),
                        ExpressionFrame::Group(span.start),
                    ));
                    machine.minimum = 0;
                    return Ok(ExpressionFlow::Continue);
                }
            }
            if self.is_name("new") {
                self.advance()?;
                let ty = if self.is_symbol('{') {
                    None
                } else {
                    Some(self.type_expression_expected(Some("{"))?)
                };
                let missing_delimiter = ty.is_none();
                machine.object_build =
                    Some(self.start_object(None, ty, span.start, missing_delimiter, true)?);
                return Ok(ExpressionFlow::Continue);
            }
            if self.is_name("if") {
                self.enter_expression_scope()?;
                self.advance()?;
                self.expect('(')?;
                machine.frames.push((
                    machine.minimum,
                    self.expectation.replace(")".into()),
                    ExpressionFrame::IfCondition(span.start),
                ));
                machine.minimum = 0;
                return Ok(ExpressionFlow::Continue);
            }
            if self.is_name("let") {
                self.enter_expression_scope()?;
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
                machine.frames.push((
                    machine.minimum,
                    self.expectation.replace(")".into()),
                    ExpressionFrame::LetValue(span.start, name, ty),
                ));
                machine.minimum = 0;
                return Ok(ExpressionFlow::Continue);
            }
            if self.is_name("throw") {
                self.advance()?;
                self.expect('(')?;
                machine.frames.push((
                    machine.minimum,
                    self.expectation.replace(")".into()),
                    ExpressionFrame::Throw(span.start),
                ));
                machine.minimum = 0;
                return Ok(ExpressionFlow::Continue);
            }
            if self.is_name("trace") {
                self.advance()?;
                self.expect('(')?;
                machine.frames.push((
                    machine.minimum,
                    self.expectation.replace(")".into()),
                    ExpressionFrame::Trace(span.start),
                ));
                machine.minimum = 0;
                return Ok(ExpressionFlow::Continue);
            }
            if self.is_name("read?") {
                self.advance()?;
                self.expect('(')?;
                machine.frames.push((
                    machine.minimum,
                    self.expectation.replace(")".into()),
                    ExpressionFrame::ReadResourceOrNull(span.start),
                ));
                machine.minimum = 0;
                return Ok(ExpressionFlow::Continue);
            }
            if self.is_symbol('-') || self.is_symbol('!') {
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
                        machine.current = Some(Expr {
                            kind: ExprKind::Int(value),
                            span: Span::new(span.start, end),
                        });
                    }
                }
                if machine.current.is_none() {
                    machine.frames.push((
                        machine.minimum,
                        self.expectation.clone(),
                        ExpressionFrame::Unary(
                            span.start,
                            if negate {
                                UnaryOp::Negate
                            } else {
                                UnaryOp::Not
                            },
                        ),
                    ));
                    machine.minimum = 11;
                    return Ok(ExpressionFlow::Continue);
                }
            } else if self.is_name("read") {
                self.advance()?;
                let glob = self.is_symbol('*');
                if glob {
                    self.advance()?;
                }
                self.expect('(')?;
                machine.frames.push((
                    machine.minimum,
                    self.expectation.replace(")".into()),
                    if glob {
                        ExpressionFrame::ReadResourceGlob(span.start)
                    } else {
                        ExpressionFrame::ReadResource(span.start)
                    },
                ));
                machine.minimum = 0;
                return Ok(ExpressionFlow::Continue);
            } else {
                return Ok(ExpressionFlow::Primary);
            }
        }
        self.expression_step()?;
        let mut left = machine.current.take().expect("expression machine value");
        'postfix: {
            if machine.finish_expression {
                machine.finish_expression = false;
                break 'postfix;
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
                machine.current = Some(left);
                return Ok(ExpressionFlow::Continue);
            }
            if self.is_operator("!!") {
                let end = self.advance()?.span.end;
                let span = Span::new(left.span.start, end);
                left = Expr {
                    kind: ExprKind::NonNull(Box::new(left)),
                    span,
                };
                machine.current = Some(left);
                return Ok(ExpressionFlow::Continue);
            }
            if self.is_symbol('[') && self.same_line() {
                self.advance()?;
                machine.frames.push((
                    machine.minimum,
                    self.expectation.replace("]".into()),
                    ExpressionFrame::Subscript(left),
                ));
                machine.minimum = 0;
                machine.current = None;
                return Ok(ExpressionFlow::Continue);
            }
            if (self.is_name("is") || self.is_name("as")) && machine.minimum <= 6 {
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
                machine.current = Some(left);
                return Ok(ExpressionFlow::Continue);
            }
            if self.is_symbol('{') {
                // Pkl requires an explicitly parenthesized amendment parent.
                if !matches!(
                    left.kind,
                    ExprKind::Parenthesized(_) | ExprKind::New { .. } | ExprKind::Object { .. }
                ) {
                    let parent = &self.lexer.source[left.span.start..left.span.end];
                    return Err(Diagnostic::language(
                        DiagnosticOrigin::Parser,
                        DiagnosticKind::Syntax,
                        self.token.span,
                        format!(
                            "Unexpected token: `{{`.\n\nIf you meant to write an amends expression, wrap the parent in parentheses. Try: `({parent}) {{ ... }}`"
                        ),
                    ));
                }
                let start = left.span.start;
                machine.object_build =
                    Some(self.start_object(Some(left), None, start, true, true)?);
                return Ok(ExpressionFlow::Continue);
            }
            if self.is_symbol('(') && self.same_line() {
                if !matches!(
                    left.kind,
                    ExprKind::Name(_) | ExprKind::Member(_, _) | ExprKind::NullableMember(_, _)
                ) || (matches!(left.kind, ExprKind::Name(_))
                    && self.lexer.source[left.span.start..left.span.end].starts_with('('))
                {
                    // Calls are only postfix syntax on unqualified or qualified
                    // names. Let the enclosing grammar report its own delimiter.
                    break 'postfix;
                }
                self.advance()?;
                if self.is_symbol(')') {
                    let end = self.advance()?.span.end;
                    let span = Span::new(left.span.start, end);
                    left = Expr {
                        kind: ExprKind::Call {
                            callee: Box::new(left),
                            arguments: Vec::new(),
                        },
                        span,
                    };
                    machine.current = Some(left);
                } else {
                    machine.frames.push((
                        machine.minimum,
                        self.expectation.take(),
                        ExpressionFrame::Arguments(left, Vec::new()),
                    ));
                    machine.minimum = 0;
                    machine.current = None;
                }
                return Ok(ExpressionFlow::Continue);
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
                        break 'postfix;
                    }
                    (8, BinaryOp::Subtract, false)
                }
                Kind::Symbol('*') => (9, BinaryOp::Multiply, false),
                Kind::Symbol('/') => (9, BinaryOp::Divide, false),
                Kind::Operator("~/") => (9, BinaryOp::IntegerDivide, false),
                Kind::Symbol('%') => (9, BinaryOp::Remainder, false),
                Kind::Operator("**") => (10, BinaryOp::Power, true),
                _ => break 'postfix,
            };
            if precedence < machine.minimum {
                break 'postfix;
            }
            self.advance()?;
            machine.frames.push((
                machine.minimum,
                self.expectation.clone(),
                ExpressionFrame::Binary(left, op),
            ));
            machine.minimum = precedence + u8::from(!right_associative);
            return Ok(ExpressionFlow::Continue);
        }
        let Some((parent_minimum, expectation, frame)) = machine.frames.pop() else {
            machine.current = Some(left);
            return Ok(ExpressionFlow::Done);
        };
        machine.minimum = parent_minimum;
        self.expectation = expectation;
        let end = left.span.end;
        left = match frame {
            ExpressionFrame::Group(start) => {
                let end = self.expect(')')?.span.end;
                Expr {
                    kind: ExprKind::Parenthesized(Box::new(left)),
                    span: Span::new(start, end),
                }
            }
            ExpressionFrame::Throw(start) => {
                let end = self.expect(')')?.span.end;
                Expr {
                    kind: ExprKind::Throw(Box::new(left)),
                    span: Span::new(start, end),
                }
            }
            ExpressionFrame::Trace(start) => {
                let end = self.expect(')')?.span.end;
                Expr {
                    kind: ExprKind::Trace(Box::new(left)),
                    span: Span::new(start, end),
                }
            }
            ExpressionFrame::ReadResourceOrNull(start) => {
                let end = self.expect(')')?.span.end;
                Expr {
                    kind: ExprKind::ReadResourceOrNull(Box::new(left)),
                    span: Span::new(start, end),
                }
            }
            ExpressionFrame::ReadResource(start) => {
                let end = self.expect(')')?.span.end;
                Expr {
                    kind: ExprKind::ReadResource(Box::new(left)),
                    span: Span::new(start, end),
                }
            }
            ExpressionFrame::ReadResourceGlob(start) => {
                let end = self.expect(')')?.span.end;
                Expr {
                    kind: ExprKind::ReadResourceGlob(Box::new(left)),
                    span: Span::new(start, end),
                }
            }
            ExpressionFrame::Unary(start, op) => Expr {
                kind: ExprKind::Unary {
                    op,
                    operand: Box::new(left),
                },
                span: Span::new(start, end),
            },
            ExpressionFrame::Binary(previous, op) => {
                let start = previous.span.start;
                Expr {
                    kind: ExprKind::Binary {
                        op,
                        left: Box::new(previous),
                        right: Box::new(left),
                    },
                    span: Span::new(start, end),
                }
            }
            ExpressionFrame::Subscript(receiver) => {
                let end = self.expect(']')?.span.end;
                let start = receiver.span.start;
                Expr {
                    kind: ExprKind::Subscript {
                        receiver: Box::new(receiver),
                        key: Box::new(left),
                    },
                    span: Span::new(start, end),
                }
            }
            ExpressionFrame::IfCondition(start) => {
                self.expect(')')?;
                machine.frames.push((
                    machine.minimum,
                    self.expectation.replace("else".into()),
                    ExpressionFrame::IfThen(start, left),
                ));
                machine.minimum = 0;
                return Ok(ExpressionFlow::Continue);
            }
            ExpressionFrame::IfThen(start, condition) => {
                if !self.is_name("else") {
                    return Err(self.unexpected(Some("else")));
                }
                self.advance()?;
                machine.frames.push((
                    machine.minimum,
                    self.expectation.clone(),
                    ExpressionFrame::IfElse(start, condition, left),
                ));
                machine.minimum = 0;
                return Ok(ExpressionFlow::Continue);
            }
            ExpressionFrame::IfElse(start, condition, then_branch) => {
                self.depth -= 1;
                Expr {
                    kind: ExprKind::If {
                        condition: Box::new(condition),
                        then_branch: Box::new(then_branch),
                        else_branch: Box::new(left),
                    },
                    span: Span::new(start, end),
                }
            }
            ExpressionFrame::LetValue(start, name, ty) => {
                self.expect(')')?;
                machine.frames.push((
                    machine.minimum,
                    self.expectation.take(),
                    ExpressionFrame::LetBody(start, name, ty, left),
                ));
                machine.minimum = 0;
                return Ok(ExpressionFlow::Continue);
            }
            ExpressionFrame::LetBody(start, name, ty, value) => {
                self.depth -= 1;
                Expr {
                    kind: ExprKind::Let {
                        name,
                        ty,
                        value: Box::new(value),
                        body: Box::new(left),
                    },
                    span: Span::new(start, end),
                }
            }
            ExpressionFrame::Function(start, header_span, parameters) => {
                self.depth -= 1;
                Expr {
                    kind: ExprKind::Function {
                        type_parameters: Vec::new(),
                        type_parameters_span: None,
                        header_span,
                        parameters,
                        return_type: None,
                        body: Box::new(left),
                        method: false,
                    },
                    span: Span::new(start, end),
                }
            }
            ExpressionFrame::ObjectValue(mut build, value) => {
                let span = Span::new(
                    match &value {
                        ObjectValue::Property(head) => head.start,
                        ObjectValue::Element(start, _)
                        | ObjectValue::Spread(start, _)
                        | ObjectValue::Entry(start, _, _, _, _) => *start,
                    },
                    end,
                );
                let member = match value {
                    ObjectValue::Property(head) => {
                        ObjectMember::Property(self.finish_object_property(*head, left))
                    }
                    ObjectValue::Element(_, previous) => {
                        self.exit_entry_path(previous);
                        ObjectMember::Element { value: left, span }
                    }
                    ObjectValue::Spread(_, nullable) => ObjectMember::Spread {
                        value: left,
                        nullable,
                        span,
                    },
                    ObjectValue::Entry(_, key, predicate, _, previous) => {
                        self.exit_entry_path(previous);
                        if predicate {
                            ObjectMember::Predicate {
                                condition: *key,
                                value: left,
                                span,
                            }
                        } else {
                            ObjectMember::Entry {
                                key: *key,
                                value: left,
                                span,
                            }
                        }
                    }
                };
                build.members.push(member);
                if self.is_symbol(';') {
                    self.advance()?;
                }
                machine.object_build = Some(build);
                return Ok(ExpressionFlow::Continue);
            }
            ExpressionFrame::ObjectKey(build, start, predicate) => {
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
                        return Err(Diagnostic::language(
                            DiagnosticOrigin::Parser,
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
                let previous = self.enter_entry_path(Some(&left));
                if self.is_symbol('=') {
                    self.advance()?;
                    machine.frames.push((
                        machine.minimum,
                        self.expectation.replace("}".into()),
                        ExpressionFrame::ObjectValue(
                            build,
                            ObjectValue::Entry(start, Box::new(left), predicate, false, previous),
                        ),
                    ));
                } else {
                    if !self.is_symbol('{') {
                        return Err(self.unexpected(Some("{` or `=")));
                    }
                    let parent = Expr {
                        kind: ExprKind::EntryParent,
                        span: Span::new(start, self.previous_end),
                    };
                    machine.object_build =
                        Some(self.start_object(Some(parent), None, start, true, false)?);
                    machine.frames.push((
                        machine.minimum,
                        self.expectation.clone(),
                        ExpressionFrame::ObjectValue(
                            build,
                            ObjectValue::Entry(start, Box::new(left), predicate, true, previous),
                        ),
                    ));
                }
                machine.minimum = 0;
                return Ok(ExpressionFlow::Continue);
            }
            ExpressionFrame::WhenCondition(build, start) => {
                self.expect(')')?;
                machine.object_build =
                    Some(self.start_object(None, None, self.token.span.start, false, false)?);
                machine.frames.push((
                    machine.minimum,
                    self.expectation.clone(),
                    ExpressionFrame::WhenThen(build, start, left),
                ));
                machine.minimum = 0;
                return Ok(ExpressionFlow::Continue);
            }
            ExpressionFrame::WhenThen(mut build, start, condition) => {
                let then_members = Self::take_object_members(&mut left);
                if self.is_name("else") {
                    self.advance()?;
                    machine.object_build =
                        Some(self.start_object(None, None, self.token.span.start, false, false)?);
                    machine.frames.push((
                        machine.minimum,
                        self.expectation.clone(),
                        ExpressionFrame::WhenElse(build, start, condition, then_members),
                    ));
                    machine.minimum = 0;
                } else {
                    build.members.push(ObjectMember::When {
                        condition,
                        then_members,
                        else_members: Vec::new(),
                        span: Span::new(start, self.previous_end),
                    });
                    if self.is_symbol(';') {
                        self.advance()?;
                    }
                    machine.object_build = Some(build);
                }
                return Ok(ExpressionFlow::Continue);
            }
            ExpressionFrame::WhenElse(mut build, start, condition, then_members) => {
                build.members.push(ObjectMember::When {
                    condition,
                    then_members,
                    else_members: Self::take_object_members(&mut left),
                    span: Span::new(start, self.previous_end),
                });
                if self.is_symbol(';') {
                    self.advance()?;
                }
                machine.object_build = Some(build);
                return Ok(ExpressionFlow::Continue);
            }
            ExpressionFrame::ForIterable(build, start, parameters) => {
                self.expect(')')?;
                machine.object_build =
                    Some(self.start_object(None, None, self.token.span.start, false, false)?);
                machine.frames.push((
                    machine.minimum,
                    self.expectation.clone(),
                    ExpressionFrame::ForBody(build, start, parameters, left),
                ));
                machine.minimum = 0;
                return Ok(ExpressionFlow::Continue);
            }
            ExpressionFrame::ForBody(mut build, start, parameters, iterable) => {
                build.members.push(ObjectMember::For {
                    parameters,
                    iterable,
                    members: Self::take_object_members(&mut left),
                    span: Span::new(start, self.previous_end),
                });
                if self.is_symbol(';') {
                    self.advance()?;
                }
                machine.object_build = Some(build);
                return Ok(ExpressionFlow::Continue);
            }
            ExpressionFrame::Arguments(callee, mut arguments) => {
                arguments.push(left);
                if self.is_symbol(',') {
                    self.advance()?;
                    if !self.is_symbol(')') {
                        machine.frames.push((
                            machine.minimum,
                            self.expectation.take(),
                            ExpressionFrame::Arguments(callee, arguments),
                        ));
                        machine.minimum = 0;
                        return Ok(ExpressionFlow::Continue);
                    }
                }
                if !self.is_symbol(')') {
                    return Err(self.unexpected(Some(",` or `)")));
                }
                let end = self.advance()?.span.end;
                let start = callee.span.start;
                Expr {
                    kind: ExprKind::Call {
                        callee: Box::new(callee),
                        arguments,
                    },
                    span: Span::new(start, end),
                }
            }
        };
        machine.current = Some(left);
        Ok(ExpressionFlow::Continue)
    }

    fn start_object(
        &mut self,
        parent: Option<Expr>,
        ty: Option<Type>,
        start: usize,
        missing_delimiter: bool,
        postfix: bool,
    ) -> Result<Box<ObjectBuild>, Diagnostic> {
        self.enter_expression_scope()?;
        self.expect('{')?;
        let parameters = self.object_parameters()?;
        Ok(Box::new(ObjectBuild {
            start,
            parameters,
            parent,
            ty,
            members: Vec::new(),
            missing_delimiter,
            postfix,
        }))
    }

    fn object_parameters(&mut self) -> Result<Vec<Parameter>, Diagnostic> {
        let underscore = self.is_name("_");
        if !underscore
            && !match &self.token.kind {
                Kind::QuotedName(_) => true,
                Kind::Name(name) => !is_keyword(name),
                _ => false,
            }
        {
            return Ok(Vec::new());
        }
        let mut candidate = self.clone();
        let first = candidate.parameter()?;
        if !underscore && !candidate.is_symbol(',') && !candidate.is_operator("->") {
            return Ok(Vec::new());
        }
        let mut parameters = vec![first];
        while candidate.is_symbol(',') {
            candidate.advance()?;
            if candidate.is_operator("->") {
                break;
            }
            parameters.push(candidate.parameter()?);
        }
        if !candidate.is_operator("->") {
            return Err(candidate.unexpected(Some(",` or `->")));
        }
        candidate.advance()?;
        *self = candidate;
        Ok(parameters)
    }

    fn take_object_members(expression: &mut Expr) -> Vec<ObjectMember> {
        match &mut expression.kind {
            ExprKind::Object { members, .. } => std::mem::take(members),
            _ => unreachable!("generator body syntax"),
        }
    }

    fn object_property(&mut self) -> Result<bool, Diagnostic> {
        Ok(match &self.token.kind {
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
        })
    }

    fn object_property_head(&mut self) -> Result<PropertyHead, Diagnostic> {
        let modifiers = self.modifiers()?;
        let start = modifiers
            .first()
            .map_or(self.token.span.start, |modifier| modifier.span.start);
        let local = modifiers
            .iter()
            .any(|modifier| modifier.kind == ModifierKind::Local);
        let method = self.is_name("function");
        if method {
            self.advance()?;
        }
        let (name, name_span) = self.identifier()?;
        self.member_path.push(name.clone().into());
        let (ty, method) = if method {
            let type_parameters_start = self.token.span.start;
            let type_parameters = self.type_parameters()?;
            let type_parameters_span = (!type_parameters.is_empty())
                .then(|| Box::new(Span::new(type_parameters_start, self.previous_end)));
            let parameters = self.parameters()?;
            let return_type = self.type_annotation()?;
            let header_span = Span::new(start, self.previous_end);
            (
                None,
                Some(MethodHead {
                    type_parameters,
                    type_parameters_span,
                    parameters,
                    return_type,
                    header_span,
                }),
            )
        } else {
            (self.type_annotation()?, None)
        };
        Ok(PropertyHead {
            start,
            name,
            name_span,
            local,
            modifiers,
            ty,
            method,
        })
    }

    fn finish_object_property(&mut self, head: PropertyHead, mut expr: Expr) -> Member {
        let PropertyHead {
            start,
            name,
            name_span,
            local,
            modifiers,
            ty,
            method,
        } = head;
        let end = expr.span.end;
        if let Some(MethodHead {
            type_parameters,
            type_parameters_span,
            parameters,
            return_type,
            header_span,
        }) = method
        {
            expr = Expr {
                kind: ExprKind::Function {
                    type_parameters,
                    type_parameters_span,
                    parameters,
                    return_type,
                    header_span,
                    body: Box::new(expr),
                    method: true,
                },
                span: Span::new(start, end),
            };
        }
        self.member_path.pop();
        let header_end = ty.as_ref().map_or(name_span.end, |ty| ty.span.end);
        let modifier_span = |word: &str| {
            let source = self.lexer.source;
            let begin = source[start..header_end]
                .find(word)
                .map(|offset| start + offset)
                .or_else(|| {
                    source[..start]
                        .char_indices()
                        .next_back()
                        .map(|(index, _)| index)
                })?;
            let mut end = begin;
            let mut units = 0;
            for character in source[begin..].chars() {
                if units >= word.len() {
                    break;
                }
                units += character.len_utf16();
                end += character.len_utf8();
            }
            Some(Span::new(begin, end))
        };
        Member {
            annotations: Vec::new(),
            name,
            name_span,
            expr,
            span: Span::new(start, end),
            local,
            ty,
            modifiers,
            override_fixed_span: modifier_span("fixed"),
            override_const_span: modifier_span("const"),
        }
    }

    fn object(&mut self, parent: Option<Expr>) -> Result<Expr, Diagnostic> {
        let start = parent
            .as_ref()
            .map_or(self.token.span.start, |expr| expr.span.start);
        let build = self.start_object(parent, None, start, true, false)?;
        self.expression_machine(0, Some(build))
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
            if self.is_symbol(')') {
                break;
            }
        }
        if !self.is_symbol(')') {
            return Err(self.unexpected(Some(",` or `)")));
        }
        self.advance()?;
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
        if !self.is_symbol(')') {
            return Err(self.unexpected(Some(",` or `)")));
        }
        self.advance()?;
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
        self.type_expression_expected(None)
    }

    fn type_expression_expected(
        &mut self,
        expected: Option<&'static str>,
    ) -> Result<Type, Diagnostic> {
        let depth = self.depth;
        let result = self.type_inner(expected);
        self.depth = depth;
        result
    }

    // Integer validation is deferred until syntax succeeds, but its authored
    // entry scope must be retained when the value is parsed. Key expressions
    // are visited before entering this scope, as in SymbolTable.
    fn enter_entry_path(&mut self, key: Option<&Expr>) -> Option<PklString> {
        let literal = key.and_then(|key| match &key.kind {
            ExprKind::String(value) => {
                Some(PklString::from("\"").concat(value).concat(&"\"".into()))
            }
            ExprKind::Int(value) => Some(value.to_string().into()),
            ExprKind::Float(value) => Some(crate::numeric::double_to_string(*value).into()),
            ExprKind::Bool(value) => Some(value.to_string().into()),
            _ => None,
        });
        let label = literal.unwrap_or_else(|| {
            let count = self
                .entry_counts
                .entry(PklString::join(&self.member_path, "."))
                .or_default();
            *count += 1;
            format!("#{count}").into()
        });
        let previous = self.member_path.pop();
        let parent = previous.clone().unwrap_or_default();
        self.member_path.push(
            parent
                .concat(&"[".into())
                .concat(&label)
                .concat(&"]".into()),
        );
        previous
    }

    fn exit_entry_path(&mut self, previous: Option<PklString>) {
        self.member_path.pop();
        if let Some(previous) = previous {
            self.member_path.push(previous);
        }
    }

    fn integer_literal(&mut self, text: &str, negative: bool, span: Span) -> i64 {
        match integer(text, negative, span) {
            Ok(value) => value,
            Err(mut diagnostic) => {
                if !self.member_path.is_empty() {
                    diagnostic.member = Some(Box::new(PklString::join(&self.member_path, ".")));
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
            Kind::Name(name) if name == "true" || name == "false" => {
                self.advance()?;
                ExprKind::Bool(name == "true")
            }
            Kind::Name(name) if name == "null" => {
                self.advance()?;
                ExprKind::Null
            }
            Kind::Name(name) if name == "outer" || name == "module" => {
                self.advance()?;
                if name == "outer" {
                    ExprKind::Outer
                } else {
                    ExprKind::Module
                }
            }
            Kind::Name(name) if name == "import" => {
                self.advance_constant()?;
                self.expect('(')?;
                let (uri, uri_span) = self.string_constant()?;
                let Some(uri) = uri.as_str() else {
                    return Err(Diagnostic::unsupported(
                        DiagnosticOrigin::Parser,
                        "module-uri-utf16",
                        uri_span,
                        "Module URIs containing unmatched UTF-16 surrogates are not implemented.",
                    ));
                };
                let uri = uri.to_owned();
                let end = self.expect(')')?.span.end;
                return Ok(Expr {
                    kind: ExprKind::Import { uri, uri_span },
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
            Kind::QuotedName(name) => {
                self.advance()?;
                ExprKind::Name(name)
            }
            Kind::Name(name) => {
                if is_keyword(&name) || name == "_" {
                    if name == "import" {
                        return Err(Diagnostic::unsupported(
                            DiagnosticOrigin::Parser,
                            "import-expression",
                            span,
                            "Import expressions are not implemented.",
                        ));
                    }
                    return Err(self.unexpected(self.expectation.as_deref()));
                }
                self.advance()?;
                ExprKind::Name(name)
            }
            Kind::Symbol('[') => {
                return Err(self.unexpected(self.expectation.as_deref()));
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
        Diagnostic::language(
            DiagnosticOrigin::DeferredAstValidation,
            DiagnosticKind::IntegerRange,
            span,
            format!("Int literal `{literal}` is too large."),
        )
    })
}

pub fn parse_module(source: &str) -> Result<Module, Diagnostic> {
    let module = parse_module_for_evaluation(source)?;
    if let Some(error) = module.deferred_diagnostics.first() {
        return Err(error.clone());
    }
    Ok(module)
}

/// Syntax errors still precede every AST-building effect. Evaluation preserves
/// deferred AST failures so the shared preparation plan can interleave them
/// with ModuleKey operations in the upstream visitor's order.
pub(crate) fn parse_module_for_evaluation(source: &str) -> Result<Module, Diagnostic> {
    let mut parser = Parser::new(source)?;
    let documented = parser.doc_comment()?;
    let annotations = parser.annotations()?;
    let modifiers = parser.modifiers()?;
    let has_declaration =
        parser.is_name("module") || parser.is_name("extends") || parser.is_name("amends");
    let (declaration, initial) = if has_declaration {
        (parser.module_declaration(modifiers, annotations)?, None)
    } else {
        (None, Some((modifiers, annotations, documented)))
    };
    let mut members = Vec::new();
    let mut initial = initial;
    let has_header = initial
        .as_ref()
        .is_some_and(|(modifiers, annotations, documented)| {
            *documented || !modifiers.is_empty() || !annotations.is_empty()
        });
    if has_header && parser.is_name("import") {
        return Err(Diagnostic::language(
            DiagnosticOrigin::Parser,
            DiagnosticKind::Syntax,
            parser.token.span,
            "Imports cannot have doc comments, annotations or modifiers.",
        ));
    }
    if has_header && parser.token.kind == Kind::Eof {
        return Err(Diagnostic::language(
            DiagnosticOrigin::Parser,
            DiagnosticKind::Syntax,
            parser.token.span,
            "Unexpected end of file.",
        ));
    }
    if !has_header {
        while parser.is_name("import") {
            members.push(parser.import()?);
            initial = None;
            if parser.is_symbol(';') {
                parser.advance()?;
            }
        }
    }
    members.extend(parser.member_list(MemberContext::Module, initial)?);
    for error in &mut parser.pending {
        error.module_name = declaration
            .as_ref()
            .and_then(|declaration| declaration.name.as_ref())
            .map(|name| name.join(".").into_boxed_str());
    }
    Ok(Module {
        deferred_diagnostics: parser.pending,
        source: std::sync::Arc::from(source),
        declaration,
        members,
        folded_integer_spans: parser.folded_integer_spans,
        interpolation_spans: parser.interpolation_spans,
    })
}

/// Expression parser metadata is retained for signed-number and interpolation diagnostics.
#[derive(Clone, Debug)]
pub(crate) struct ParsedExpression {
    pub expression: Expr,
    pub deferred_diagnostics: Vec<Diagnostic>,
    pub folded_integer_spans: std::collections::BTreeMap<Span, Span>,
    pub interpolation_spans: std::collections::BTreeMap<Span, Span>,
}

pub fn parse_expression(source: &str) -> Result<Expr, Diagnostic> {
    parse_expression_with_metadata(source).map(|parsed| parsed.expression)
}

pub(crate) fn parse_expression_with_metadata(source: &str) -> Result<ParsedExpression, Diagnostic> {
    let parsed = parse_expression_for_evaluation(source)?;
    if let Some(error) = parsed.deferred_diagnostics.first() {
        return Err(error.clone());
    }
    Ok(parsed)
}

pub(crate) fn parse_expression_for_evaluation(
    source: &str,
) -> Result<ParsedExpression, Diagnostic> {
    let mut parser = Parser::new(source)?;
    let expression = parser.expression(0)?;
    if parser.token.kind != Kind::Eof {
        return Err(parser.unexpected(Some("end of file")));
    }
    Ok(ParsedExpression {
        expression,
        deferred_diagnostics: parser.pending,
        folded_integer_spans: parser.folded_integer_spans,
        interpolation_spans: parser.interpolation_spans,
    })
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
