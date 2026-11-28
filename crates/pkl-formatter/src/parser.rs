//! Lossless generic CST parser mirroring `org.pkl.parser.GenericParserImpl`.

use crate::cst::{Node, NodeType};
use crate::error;
use crate::lexer::Lexer;
use crate::token::{Operator, Token};
use crate::unicode_tables::is_java_blank;
use crate::{FormatError, FullSpan};

#[derive(Clone, Copy, Debug)]
struct FullToken {
    token: Token,
    span: FullSpan,
    new_lines_between: usize,
}

impl FullToken {
    fn text(self, source: &[u16]) -> String {
        String::from_utf16_lossy(
            &source[self.span.char_index..self.span.char_index + self.span.length],
        )
    }
}

struct HeaderResult {
    has_doc_comment: bool,
    has_annotations: bool,
    has_modifiers: bool,
}

struct HeaderParts {
    prefixes: Vec<Node>,
    modifier_list: Option<Node>,
}

struct LookaheadSearch {
    tk: FullToken,
    has_semicolon: bool,
}

enum ExpectError<'a> {
    UnexpectedToken(&'a str),
    UnexpectedToken2(&'a str, &'a str),
    MissingDelimiter(&'a str),
}

use std::collections::HashMap;

const MAX_PARSE_DEPTH: usize = 128;

pub(crate) struct GenericParser {
    source: Vec<u16>,
    tokens: Vec<FullToken>,
    cursor: usize,
    depth: usize,
    param_cache: HashMap<usize, bool>,
}

impl GenericParser {
    pub(crate) fn parse_module_with_source(input: &str) -> Result<(Node, Vec<u16>), FormatError> {
        let mut lexer = Lexer::new(input);
        let mut tokens = Vec::new();
        loop {
            let token = lexer.next_token()?;
            let ft = FullToken {
                token,
                span: lexer.full_span(),
                new_lines_between: lexer.get_new_lines_between(),
            };
            let is_eof = ft.token == Token::Eof;
            tokens.push(ft);
            if is_eof {
                break;
            }
        }
        let mut parser = Self {
            source: lexer.into_source(),
            tokens,
            cursor: 0,
            depth: 0,
            param_cache: HashMap::new(),
        };
        let module = parser.parse_module()?;
        Ok((module, parser.source))
    }

    fn enter_depth(&mut self) -> Result<(), FormatError> {
        if self.depth >= MAX_PARSE_DEPTH {
            return Err(self.parser_error(format!(
                "Maximum syntax nesting depth ({MAX_PARSE_DEPTH}) exceeded."
            )));
        }
        self.depth += 1;
        Ok(())
    }

    fn leave_depth(&mut self) {
        self.depth = self.depth.saturating_sub(1);
    }

    fn token_at(&self, index: usize) -> FullToken {
        let idx = index.min(self.tokens.len().saturating_sub(1));
        self.tokens[idx]
    }

    fn current(&self) -> FullToken {
        self.token_at(self.cursor)
    }

    fn lookahead_token(&self) -> Token {
        self.current().token
    }

    fn span_lookahead(&self) -> FullSpan {
        self.current().span
    }

    fn next(&mut self) -> FullToken {
        let tmp = self.current();
        if self.cursor + 1 < self.tokens.len() {
            self.cursor += 1;
        }
        tmp
    }

    fn prev(&self) -> FullToken {
        self.token_at(self.cursor.saturating_sub(1))
    }

    fn no_semicolon_inbetween(&self) -> bool {
        self.cursor == 0 || self.prev().token != Token::Semicolon
    }

    fn backtrack_to(&mut self, point: usize) {
        self.cursor = point.min(self.tokens.len().saturating_sub(1));
    }

    fn lookahead(&self) -> Token {
        let mut i = self.cursor;
        let mut tmp = self.token_at(i);
        while tmp.token.is_affix() && tmp.token != Token::Eof && i + 1 < self.tokens.len() {
            i += 1;
            tmp = self.token_at(i);
        }
        tmp.token
    }

    fn full_lookahead(&self) -> LookaheadSearch {
        let mut i = self.cursor;
        let mut has_semicolon = false;
        let mut tmp = self.token_at(i);
        while tmp.token.is_affix() && tmp.token != Token::Eof && i + 1 < self.tokens.len() {
            if tmp.token == Token::Semicolon {
                has_semicolon = true;
            }
            i += 1;
            tmp = self.token_at(i);
        }
        LookaheadSearch {
            tk: tmp,
            has_semicolon,
        }
    }

    fn ff_into(&mut self, children: &mut Vec<Node>) {
        while self.current().token.is_affix() && self.cursor + 1 < self.tokens.len() {
            let tmp = self.current();
            children.push(make_affix(tmp));
            self.cursor += 1;
        }
    }

    fn ff(&mut self) {
        while self.current().token.is_affix() && self.cursor + 1 < self.tokens.len() {
            self.cursor += 1;
        }
    }

    fn is_module_decl(&self) -> bool {
        let mut c = self.cursor;
        let mut ftk = self.token_at(c);
        while (ftk.token.is_affix() || ftk.token.is_modifier()) && c + 1 < self.tokens.len() {
            c += 1;
            ftk = self.token_at(c);
        }
        matches!(ftk.token, Token::Module | Token::Extends | Token::Amends)
    }

    fn is_import(&self) -> bool {
        let mut c = self.cursor;
        let mut ftk = self.token_at(c);
        while ftk.token.is_affix() && c + 1 < self.tokens.len() {
            c += 1;
            ftk = self.token_at(c);
        }
        matches!(ftk.token, Token::Import | Token::ImportStar)
    }

    fn parser_error(&self, message: String) -> FormatError {
        FormatError::Parser {
            message,
            span: self.span_lookahead(),
        }
    }

    fn parser_error_at(message: String, span: FullSpan) -> FormatError {
        FormatError::Parser { message, span }
    }

    fn expect(&mut self, expected: Token, err: ExpectError<'_>) -> Result<FullToken, FormatError> {
        if self.lookahead_token() != expected {
            let cur = self.current();
            let span = if cur.token == Token::Eof || cur.new_lines_between > 0 {
                self.prev().span.stop_span()
            } else {
                cur.span
            };
            let got = if cur.token == Token::Eof {
                "EOF".to_string()
            } else {
                cur.text(&self.source)
            };
            let message = match err {
                ExpectError::UnexpectedToken(exp) => error::unexpected_token(&got, exp),
                ExpectError::UnexpectedToken2(e1, e2) => error::unexpected_token_2(&got, e1, e2),
                ExpectError::MissingDelimiter(delim) => error::missing_delimiter(delim),
            };
            return Err(Self::parser_error_at(message, span));
        }
        Ok(self.next())
    }

    fn expect_into(
        &mut self,
        expected: Token,
        children: &mut Vec<Node>,
        err: ExpectError<'_>,
    ) -> Result<(), FormatError> {
        let tk = self.expect(expected, err)?;
        children.push(make_terminal(tk));
        Ok(())
    }

    fn parse_module(&mut self) -> Result<Node, FormatError> {
        if self.lookahead_token() == Token::Eof {
            return Ok(Node::with_span(
                NodeType::Module,
                FullSpan {
                    char_index: 0,
                    length: 0,
                    line_begin: 1,
                    col_begin: 1,
                    line_end: 1,
                    col_end: 1,
                },
                Vec::new(),
            ));
        }
        let mut children = Vec::new();
        let mut nodes = Vec::new();
        if self.lookahead_token() == Token::Shebang {
            let tk = self.next();
            nodes.push(make_affix(tk));
        }
        self.ff_into(&mut nodes);

        let mut res = self.parse_member_header(&mut children)?;

        if self.is_module_decl() {
            let pre = std::mem::take(&mut children);
            nodes.push(self.parse_module_decl(pre)?);
            res = HeaderResult {
                has_doc_comment: false,
                has_annotations: false,
                has_modifiers: false,
            };
            self.ff_into(&mut nodes);
        }

        // imports
        let mut imports = Vec::new();
        while matches!(self.lookahead_token(), Token::Import | Token::ImportStar) {
            if res.has_doc_comment || res.has_annotations || res.has_modifiers {
                return Err(self.parser_error(error::wrong_headers("Imports")));
            }
            let last_import = self.parse_import_decl()?;
            let last_span = last_import.span;
            imports.push(last_import);
            while self.lookahead_token().is_affix() && last_span.is_same_line(self.span_lookahead())
            {
                let tk = self.next();
                imports.push(make_affix(tk));
            }
            if !self.is_import() {
                break;
            }
            self.ff_into(&mut imports);
        }
        if !imports.is_empty() {
            nodes.push(Node::from_children(NodeType::ImportList, imports));
            self.ff_into(&mut nodes);
        }

        // entries
        if res.has_doc_comment || res.has_annotations || res.has_modifiers {
            let pre = std::mem::take(&mut children);
            nodes.push(self.parse_module_member(pre)?);
            self.ff_into(&mut nodes);
        }

        while self.lookahead_token() != Token::Eof {
            children.clear();
            self.parse_member_header(&mut children)?;
            let pre = std::mem::take(&mut children);
            nodes.push(self.parse_module_member(pre)?);
            self.ff_into(&mut nodes);
        }
        Ok(Node::from_children(NodeType::Module, nodes))
    }

    fn parse_module_decl(&mut self, pre_children: Vec<Node>) -> Result<Node, FormatError> {
        let header_parts = get_header_parts(pre_children);
        let mut children = header_parts.prefixes;
        let mut headers = Vec::new();
        if let Some(mod_list) = header_parts.modifier_list {
            headers.push(mod_list);
        }
        if self.lookahead_token() == Token::Module {
            let mut sub_children = headers;
            let tk = self.next();
            sub_children.push(make_terminal(tk));
            self.ff_into(&mut sub_children);
            sub_children.push(self.parse_qualified_identifier()?);
            children.push(Node::from_children(
                NodeType::ModuleDefinition,
                sub_children,
            ));
        } else {
            let has_mods = !headers.is_empty();
            children.extend(headers);
            if has_mods {
                return Err(
                    self.parser_error(error::wrong_headers("Amends or extends declaration"))
                );
            }
        }
        let looka = self.lookahead();
        if matches!(looka, Token::Amends | Token::Extends) {
            let kind = if looka == Token::Amends {
                NodeType::AmendsClause
            } else {
                NodeType::ExtendsClause
            };
            self.ff_into(&mut children);
            let mut sub_children = Vec::new();
            let tk = self.next();
            sub_children.push(make_terminal(tk));
            self.ff_into(&mut sub_children);
            sub_children.push(self.parse_string_constant()?);
            children.push(Node::from_children(kind, sub_children));
        }
        Ok(Node::from_children(NodeType::ModuleDeclaration, children))
    }

    fn parse_qualified_identifier(&mut self) -> Result<Node, FormatError> {
        let mut children = Vec::new();
        children.push(self.parse_identifier()?);
        while self.lookahead() == Token::Dot {
            self.ff_into(&mut children);
            let tk = self.next();
            children.push(Node::leaf(NodeType::Terminal, tk.span));
            self.ff_into(&mut children);
            children.push(self.parse_identifier()?);
        }
        Ok(Node::from_children(NodeType::QualifiedIdentifier, children))
    }

    fn parse_import_decl(&mut self) -> Result<Node, FormatError> {
        let mut children = Vec::new();
        let tk = self.next();
        children.push(make_terminal(tk));
        self.ff_into(&mut children);
        children.push(self.parse_string_constant()?);
        if self.lookahead() == Token::As {
            self.ff_into(&mut children);
            let mut alias = Vec::new();
            let as_tk = self.next();
            alias.push(make_terminal(as_tk));
            self.ff_into(&mut alias);
            alias.push(self.parse_identifier()?);
            children.push(Node::from_children(NodeType::ImportAlias, alias));
        }
        Ok(Node::from_children(NodeType::Import, children))
    }

    fn parse_member_header(
        &mut self,
        children: &mut Vec<Node>,
    ) -> Result<HeaderResult, FormatError> {
        let mut has_doc_comment = false;
        let mut has_annotations = false;
        let mut has_modifiers = false;
        let mut docs = Vec::new();
        self.ff_into(children);
        while self.lookahead() == Token::DocComment {
            self.ff_into(&mut docs);
            let tk = self.next();
            docs.push(Node::leaf(NodeType::DocCommentLine, tk.span));
            has_doc_comment = true;
        }
        if has_doc_comment {
            children.push(Node::from_children(NodeType::DocComment, docs));
        }
        self.ff_into(children);
        while self.lookahead_token() == Token::At {
            children.push(self.parse_annotation()?);
            has_annotations = true;
            self.ff_into(children);
        }
        let mut modifiers = Vec::new();
        while self.lookahead_token().is_modifier() {
            let tk = self.next();
            modifiers.push(Node::leaf(NodeType::Modifier, tk.span));
            has_modifiers = true;
            self.ff_into(children);
        }
        if has_modifiers {
            children.push(Node::from_children(NodeType::ModifierList, modifiers));
        }
        Ok(HeaderResult {
            has_doc_comment,
            has_annotations,
            has_modifiers,
        })
    }

    fn parse_module_member(&mut self, pre_children: Vec<Node>) -> Result<Node, FormatError> {
        match self.lookahead_token() {
            Token::Identifier => self.parse_class_property(pre_children),
            Token::TypeAlias => self.parse_type_alias(pre_children),
            Token::Class => self.parse_class(pre_children),
            Token::Function => self.parse_class_method(pre_children),
            Token::Eof => Err(self.parser_error(error::unexpected_end_of_file())),
            tok => {
                if tok.is_keyword() {
                    return Err(self.parser_error(error::keyword_not_allowed_here(tok.text())));
                }
                if tok == Token::DocComment {
                    return Err(self.parser_error(error::dangling_doc_comment()));
                }
                Err(self.parser_error(error::invalid_top_level_token()))
            }
        }
    }

    fn parse_type_alias(&mut self, pre_children: Vec<Node>) -> Result<Node, FormatError> {
        let header_parts = get_header_parts(pre_children);
        let mut children = header_parts.prefixes;
        let mut headers = Vec::new();
        if let Some(mod_list) = header_parts.modifier_list {
            headers.push(mod_list);
        }
        let kw = self.next();
        headers.push(make_terminal(kw));
        self.ff_into(&mut headers);
        headers.push(self.parse_identifier()?);
        self.ff_into(&mut headers);
        if self.lookahead_token() == Token::Lt {
            headers.push(self.parse_type_parameter_list()?);
            self.ff_into(&mut headers);
        }
        self.expect_into(
            Token::Assign,
            &mut headers,
            ExpectError::UnexpectedToken("="),
        )?;
        children.push(Node::from_children(NodeType::TypealiasHeader, headers));
        let mut body = Vec::new();
        self.ff_into(&mut body);
        body.push(self.parse_type(None)?);
        children.push(Node::from_children(NodeType::TypealiasBody, body));
        Ok(Node::from_children(NodeType::Typealias, children))
    }

    fn parse_class(&mut self, pre_children: Vec<Node>) -> Result<Node, FormatError> {
        let header_parts = get_header_parts(pre_children);
        let mut children = header_parts.prefixes;
        let mut headers = Vec::new();
        if let Some(mod_list) = header_parts.modifier_list {
            headers.push(mod_list);
        }
        let kw = self.next();
        headers.push(make_terminal(kw));
        self.ff_into(&mut headers);
        headers.push(self.parse_identifier()?);
        if self.lookahead() == Token::Lt {
            self.ff_into(&mut headers);
            headers.push(self.parse_type_parameter_list()?);
        }
        if self.lookahead() == Token::Extends {
            let mut extend = Vec::new();
            self.ff_into(&mut extend);
            let ext_tk = self.next();
            extend.push(make_terminal(ext_tk));
            self.ff_into(&mut extend);
            extend.push(self.parse_type(None)?);
            headers.push(Node::from_children(NodeType::ClassHeaderExtends, extend));
        }
        children.push(Node::from_children(NodeType::ClassHeader, headers));
        if self.lookahead() == Token::LBrace {
            self.ff_into(&mut children);
            children.push(self.parse_class_body()?);
        }
        Ok(Node::from_children(NodeType::Class, children))
    }

    fn parse_class_body(&mut self) -> Result<Node, FormatError> {
        let mut children = Vec::new();
        let lbrace = self.next();
        children.push(make_terminal(lbrace));
        let mut elements = Vec::new();
        let mut has_elements = false;
        self.ff_into(&mut elements);
        while !matches!(self.lookahead_token(), Token::RBrace | Token::Eof) {
            has_elements = true;
            let mut pre_children = Vec::new();
            self.parse_member_header(&mut pre_children)?;
            if self.lookahead_token() == Token::Function {
                elements.push(self.parse_class_method(pre_children)?);
            } else {
                elements.push(self.parse_class_property(pre_children)?);
            }
            self.ff_into(&mut elements);
        }
        if self.lookahead_token() == Token::Eof {
            return Err(Self::parser_error_at(
                error::missing_delimiter("}"),
                self.prev().span.stop_span(),
            ));
        }
        if has_elements {
            children.push(Node::from_children(NodeType::ClassBodyElements, elements));
        } else if !elements.is_empty() {
            children.extend(elements);
        }
        self.expect_into(
            Token::RBrace,
            &mut children,
            ExpectError::MissingDelimiter("}"),
        )?;
        Ok(Node::from_children(NodeType::ClassBody, children))
    }

    fn parse_class_property(&mut self, pre_children: Vec<Node>) -> Result<Node, FormatError> {
        let header_parts = get_header_parts(pre_children);
        let mut children = header_parts.prefixes;
        let mut header = Vec::new();
        let mut header_begin = Vec::new();
        if let Some(mod_list) = header_parts.modifier_list {
            header_begin.push(mod_list);
        }
        header_begin.push(self.parse_identifier()?);
        header.push(Node::from_children(
            NodeType::ClassPropertyHeaderBegin,
            header_begin,
        ));
        let mut has_type_annotation = false;
        if self.lookahead() == Token::Colon {
            self.ff_into(&mut header);
            header.push(self.parse_type_annotation()?);
            has_type_annotation = true;
        }
        children.push(Node::from_children(NodeType::ClassPropertyHeader, header));
        if self.lookahead() == Token::Assign {
            self.ff_into(&mut children);
            let assign = self.next();
            children.push(make_terminal(assign));
            let mut body = Vec::new();
            self.ff_into(&mut body);
            body.push(self.parse_expr(None, 1)?);
            children.push(Node::from_children(NodeType::ClassPropertyBody, body));
        } else if self.lookahead() == Token::LBrace {
            if has_type_annotation {
                return Err(self.parser_error(error::type_annotation_in_amends()));
            }
            while self.lookahead() == Token::LBrace {
                self.ff_into(&mut children);
                children.push(self.parse_object_body()?);
            }
        }
        Ok(Node::from_children(NodeType::ClassProperty, children))
    }

    fn parse_class_method(&mut self, pre_children: Vec<Node>) -> Result<Node, FormatError> {
        let header_parts = get_header_parts(pre_children);
        let mut children = header_parts.prefixes;
        let mut headers = Vec::new();
        if let Some(mod_list) = header_parts.modifier_list {
            headers.push(mod_list);
        }
        self.expect_into(
            Token::Function,
            &mut headers,
            ExpectError::UnexpectedToken("function"),
        )?;
        self.ff_into(&mut headers);
        headers.push(self.parse_identifier()?);
        children.push(Node::from_children(NodeType::ClassMethodHeader, headers));
        self.ff_into(&mut children);
        if self.lookahead_token() == Token::Lt {
            children.push(self.parse_type_parameter_list()?);
            self.ff_into(&mut children);
        }
        children.push(self.parse_parameter_list()?);
        if self.lookahead() == Token::Colon {
            self.ff_into(&mut children);
            children.push(self.parse_type_annotation()?);
        }
        if self.lookahead() == Token::Assign {
            self.ff_into(&mut children);
            let assign = self.next();
            children.push(make_terminal(assign));
            let mut body = Vec::new();
            self.ff_into(&mut body);
            body.push(self.parse_expr(None, 1)?);
            children.push(Node::from_children(NodeType::ClassMethodBody, body));
        }
        Ok(Node::from_children(NodeType::ClassMethod, children))
    }

    fn parse_object_body(&mut self) -> Result<Node, FormatError> {
        self.enter_depth()?;
        let res = self.parse_object_body_inner();
        self.leave_depth();
        res
    }

    fn parse_object_body_inner(&mut self) -> Result<Node, FormatError> {
        let mut children = Vec::new();
        self.expect_into(
            Token::LBrace,
            &mut children,
            ExpectError::UnexpectedToken("{"),
        )?;
        if self.lookahead() == Token::RBrace {
            self.ff_into(&mut children);
            let rbrace = self.next();
            children.push(make_terminal(rbrace));
            return Ok(Node::from_children(NodeType::ObjectBody, children));
        }
        if self.is_parameter()? {
            let mut params = Vec::new();
            self.ff_into(&mut params);
            self.parse_list_of(Token::Arrow, &mut params, Self::parse_parameter)?;
            self.expect_into(
                Token::Arrow,
                &mut params,
                ExpectError::UnexpectedToken2(",", "->"),
            )?;
            children.push(Node::from_children(NodeType::ObjectParameterList, params));
            self.ff_into(&mut children);
        }
        let mut members = Vec::new();
        self.ff_into(&mut members);
        while self.lookahead_token() != Token::RBrace {
            if self.lookahead_token() == Token::Eof {
                return Err(Self::parser_error_at(
                    error::missing_delimiter("}"),
                    self.prev().span.stop_span(),
                ));
            }
            members.push(self.parse_object_member()?);
            self.ff_into(&mut members);
        }
        if !members.is_empty() {
            children.push(Node::from_children(NodeType::ObjectMemberList, members));
        }
        let rbrace = self.next();
        children.push(make_terminal(rbrace));
        Ok(Node::from_children(NodeType::ObjectBody, children))
    }

    fn is_parameter(&mut self) -> Result<bool, FormatError> {
        if self.lookahead_token() == Token::Underscore {
            return Ok(true);
        }
        if self.lookahead_token() != Token::Identifier {
            return Ok(false);
        }
        let original_cursor = self.cursor;
        if let Some(&cached) = self.param_cache.get(&original_cursor) {
            return Ok(cached);
        }
        let mut result = false;
        self.next(); // identifier
        self.ff();
        if matches!(self.lookahead_token(), Token::Arrow | Token::Comma) {
            result = true;
        } else if self.lookahead_token() == Token::Colon {
            self.next(); // colon
            self.ff();
            match self.parse_type(None) {
                Ok(_) => {
                    self.ff();
                    result = matches!(self.lookahead_token(), Token::Comma | Token::Arrow);
                }
                Err(err) => {
                    self.backtrack_to(original_cursor);
                    return Err(err);
                }
            }
        }
        self.backtrack_to(original_cursor);
        self.param_cache.insert(original_cursor, result);
        Ok(result)
    }

    fn parse_object_member(&mut self) -> Result<Node, FormatError> {
        match self.lookahead_token() {
            Token::Identifier => {
                let original_cursor = self.cursor;
                self.next();
                self.ff();
                let is_prop = matches!(
                    self.lookahead_token(),
                    Token::LBrace | Token::Colon | Token::Assign
                );
                self.backtrack_to(original_cursor);
                if is_prop {
                    self.parse_object_property(None)
                } else {
                    self.parse_object_element()
                }
            }
            Token::Function => self.parse_object_method(Vec::new()),
            Token::LPred => self.parse_member_predicate(),
            Token::LBrack => self.parse_object_entry(),
            Token::Spread | Token::QSpread => self.parse_object_spread(),
            Token::When => self.parse_when_generator(),
            Token::For => self.parse_for_generator(),
            Token::TypeAlias | Token::Class => Err(Self::parser_error_at(
                error::missing_delimiter("}"),
                self.prev().span.stop_span(),
            )),
            _ => {
                let mut pre_children = Vec::new();
                while self.lookahead_token().is_modifier() {
                    let tk = self.next();
                    pre_children.push(Node::leaf(NodeType::Modifier, tk.span));
                    self.ff_into(&mut pre_children);
                }
                if !pre_children.is_empty() {
                    let mod_list = vec![Node::from_children(NodeType::ModifierList, pre_children)];
                    if self.lookahead_token() == Token::Function {
                        self.parse_object_method(mod_list)
                    } else {
                        self.parse_object_property(Some(mod_list))
                    }
                } else {
                    self.parse_object_element()
                }
            }
        }
    }

    fn parse_object_element(&mut self) -> Result<Node, FormatError> {
        Ok(Node::from_children(
            NodeType::ObjectElement,
            vec![self.parse_expr(None, 1)?],
        ))
    }

    fn parse_object_property(
        &mut self,
        pre_children: Option<Vec<Node>>,
    ) -> Result<Node, FormatError> {
        let mut children = Vec::new();
        let mut header = Vec::new();
        let mut header_begin = pre_children.unwrap_or_default();
        self.ff_into(&mut header_begin);
        let mut modifier_list = Vec::new();
        while self.lookahead_token().is_modifier() {
            let tk = self.next();
            modifier_list.push(Node::leaf(NodeType::Modifier, tk.span));
            self.ff_into(&mut modifier_list);
        }
        if !modifier_list.is_empty() {
            header_begin.push(Node::from_children(NodeType::ModifierList, modifier_list));
        }
        header_begin.push(self.parse_identifier()?);
        header.push(Node::from_children(
            NodeType::ObjectPropertyHeaderBegin,
            header_begin,
        ));
        let mut has_type_annotation = false;
        if self.lookahead() == Token::Colon {
            self.ff_into(&mut header);
            header.push(self.parse_type_annotation()?);
            has_type_annotation = true;
        }
        children.push(Node::from_children(NodeType::ObjectPropertyHeader, header));
        if has_type_annotation || self.lookahead() == Token::Assign {
            self.ff_into(&mut children);
            self.expect_into(
                Token::Assign,
                &mut children,
                ExpectError::UnexpectedToken("="),
            )?;
            let mut body = Vec::new();
            self.ff_into(&mut body);
            body.push(self.parse_expr(Some("}"), 1)?);
            children.push(Node::from_children(NodeType::ObjectPropertyBody, body));
            return Ok(Node::from_children(NodeType::ObjectProperty, children));
        }
        self.ff_into(&mut children);
        children.extend(self.parse_body_list()?);
        Ok(Node::from_children(NodeType::ObjectProperty, children))
    }

    fn parse_object_method(&mut self, pre_children: Vec<Node>) -> Result<Node, FormatError> {
        let header_parts = get_header_parts(pre_children);
        let mut children = header_parts.prefixes;
        let mut headers = Vec::new();
        if let Some(mod_list) = header_parts.modifier_list {
            headers.push(mod_list);
        }
        self.expect_into(
            Token::Function,
            &mut headers,
            ExpectError::UnexpectedToken("function"),
        )?;
        self.ff_into(&mut headers);
        headers.push(self.parse_identifier()?);
        children.push(Node::from_children(NodeType::ClassMethodHeader, headers));
        self.ff_into(&mut children);
        if self.lookahead_token() == Token::Lt {
            children.push(self.parse_type_parameter_list()?);
            self.ff_into(&mut children);
        }
        children.push(self.parse_parameter_list()?);
        self.ff_into(&mut children);
        if self.lookahead_token() == Token::Colon {
            children.push(self.parse_type_annotation()?);
            self.ff_into(&mut children);
        }
        self.expect_into(
            Token::Assign,
            &mut children,
            ExpectError::UnexpectedToken("="),
        )?;
        let mut body = Vec::new();
        self.ff_into(&mut body);
        body.push(self.parse_expr(None, 1)?);
        children.push(Node::from_children(NodeType::ClassMethodBody, body));
        Ok(Node::from_children(NodeType::ObjectMethod, children))
    }

    fn parse_member_predicate(&mut self) -> Result<Node, FormatError> {
        let mut children = Vec::new();
        let lpred = self.next();
        children.push(make_terminal(lpred));
        self.ff_into(&mut children);
        children.push(self.parse_expr(None, 1)?);
        self.ff_into(&mut children);
        let first_brack = self.expect(Token::RBrack, ExpectError::UnexpectedToken("]]"))?;
        children.push(make_terminal(first_brack));
        let second_brack = self.expect(Token::RBrack, ExpectError::UnexpectedToken("]]"))?;
        children.push(make_terminal(second_brack));
        if first_brack.span.char_index + 1 != second_brack.span.char_index {
            let span = first_brack.span.end_with(second_brack.span);
            let text = String::from_utf16_lossy(
                &self.source[span.char_index..span.char_index + span.length],
            );
            return Err(Self::parser_error_at(
                error::unexpected_token(&text, "]]"),
                first_brack.span,
            ));
        }
        self.ff_into(&mut children);
        if self.lookahead_token() == Token::Assign {
            let assign = self.next();
            children.push(make_terminal(assign));
            self.ff_into(&mut children);
            children.push(self.parse_expr(Some("}"), 1)?);
            return Ok(Node::from_children(NodeType::MemberPredicate, children));
        }
        children.extend(self.parse_body_list()?);
        Ok(Node::from_children(NodeType::MemberPredicate, children))
    }

    fn parse_object_entry(&mut self) -> Result<Node, FormatError> {
        let mut children = Vec::new();
        let mut header = Vec::new();
        self.expect_into(
            Token::LBrack,
            &mut header,
            ExpectError::UnexpectedToken("["),
        )?;
        self.ff_into(&mut header);
        header.push(self.parse_expr(None, 1)?);
        self.expect_into(
            Token::RBrack,
            &mut header,
            ExpectError::UnexpectedToken("]"),
        )?;
        if self.lookahead() == Token::Assign {
            self.ff_into(&mut header);
            let assign = self.next();
            header.push(make_terminal(assign));
            children.push(Node::from_children(NodeType::ObjectEntryHeader, header));
            self.ff_into(&mut children);
            children.push(self.parse_expr(None, 1)?);
            return Ok(Node::from_children(NodeType::ObjectEntry, children));
        }
        children.push(Node::from_children(NodeType::ObjectEntryHeader, header));
        self.ff_into(&mut children);
        children.extend(self.parse_body_list()?);
        Ok(Node::from_children(NodeType::ObjectEntry, children))
    }

    fn parse_object_spread(&mut self) -> Result<Node, FormatError> {
        let mut children = Vec::new();
        let tk = self.next();
        children.push(make_terminal(tk));
        self.ff_into(&mut children);
        children.push(self.parse_expr(None, 1)?);
        Ok(Node::from_children(NodeType::ObjectSpread, children))
    }

    fn parse_when_generator(&mut self) -> Result<Node, FormatError> {
        let mut children = Vec::new();
        let mut header = Vec::new();
        let when_tk = self.next();
        children.push(make_terminal(when_tk));
        self.ff_into(&mut children);
        self.expect_into(
            Token::LParen,
            &mut header,
            ExpectError::UnexpectedToken("("),
        )?;
        self.ff_into(&mut header);
        header.push(self.parse_expr(None, 1)?);
        self.ff_into(&mut header);
        self.expect_into(
            Token::RParen,
            &mut header,
            ExpectError::UnexpectedToken(")"),
        )?;
        children.push(Node::from_children(NodeType::WhenGeneratorHeader, header));
        self.ff_into(&mut children);
        children.push(self.parse_object_body()?);
        if self.lookahead() == Token::Else {
            self.ff_into(&mut children);
            let else_tk = self.next();
            children.push(make_terminal(else_tk));
            self.ff_into(&mut children);
            children.push(self.parse_object_body()?);
        }
        Ok(Node::from_children(NodeType::WhenGenerator, children))
    }

    fn parse_for_generator(&mut self) -> Result<Node, FormatError> {
        let mut children = Vec::new();
        let for_tk = self.next();
        children.push(make_terminal(for_tk));
        self.ff_into(&mut children);
        let mut header = Vec::new();
        self.expect_into(
            Token::LParen,
            &mut header,
            ExpectError::UnexpectedToken("("),
        )?;
        let mut header_def = Vec::new();
        let mut header_def_header = Vec::new();
        self.ff_into(&mut header_def_header);
        header_def_header.push(self.parse_parameter()?);
        self.ff_into(&mut header_def_header);
        if self.lookahead_token() == Token::Comma {
            let comma = self.next();
            header_def_header.push(make_terminal(comma));
            self.ff_into(&mut header_def_header);
            header_def_header.push(self.parse_parameter()?);
            self.ff_into(&mut header_def_header);
        }
        self.expect_into(
            Token::In,
            &mut header_def_header,
            ExpectError::UnexpectedToken("in"),
        )?;
        header_def.push(Node::from_children(
            NodeType::ForGeneratorHeaderDefinitionHeader,
            header_def_header,
        ));
        self.ff_into(&mut header_def);
        header_def.push(self.parse_expr(None, 1)?);
        self.ff_into(&mut header_def);
        header.push(Node::from_children(
            NodeType::ForGeneratorHeaderDefinition,
            header_def,
        ));
        self.expect_into(
            Token::RParen,
            &mut header,
            ExpectError::UnexpectedToken(")"),
        )?;
        children.push(Node::from_children(NodeType::ForGeneratorHeader, header));
        self.ff_into(&mut children);
        children.push(self.parse_object_body()?);
        Ok(Node::from_children(NodeType::ForGenerator, children))
    }

    fn parse_expr(
        &mut self,
        expectation: Option<&str>,
        min_precedence: u8,
    ) -> Result<Node, FormatError> {
        self.enter_depth()?;
        let res = self.parse_expr_inner(expectation, min_precedence);
        self.leave_depth();
        res
    }

    fn parse_expr_inner(
        &mut self,
        expectation: Option<&str>,
        min_precedence: u8,
    ) -> Result<Node, FormatError> {
        let mut expr = self.parse_expr_atom(expectation)?;
        let mut full_op = self.full_lookahead();
        while let Some(operator) = Operator::from_token(full_op.tk.token) {
            if operator.prec() < min_precedence {
                break;
            }
            if matches!(operator, Operator::Minus | Operator::Subscript)
                && (full_op.has_semicolon || !expr.span.is_same_line(full_op.tk.span))
            {
                break;
            }
            let mut children = Vec::new();
            children.push(expr);
            self.ff_into(&mut children);
            let op = self.next();
            children.push(Node::leaf(NodeType::Operator, op.span));
            self.ff_into(&mut children);
            let mut node_type = NodeType::BinaryOpExpr;
            let next_min_prec = if operator.is_left_assoc() {
                operator.prec() + 1
            } else {
                operator.prec()
            };
            match op.token {
                Token::Is | Token::As => children.push(self.parse_type(None)?),
                Token::LBrack => {
                    node_type = NodeType::SubscriptExpr;
                    children.push(self.parse_expr(Some("]"), 1)?);
                    self.ff_into(&mut children);
                    self.expect_into(
                        Token::RBrack,
                        &mut children,
                        ExpectError::UnexpectedToken("]"),
                    )?;
                }
                Token::Dot | Token::QDot => {
                    node_type = NodeType::QualifiedAccessExpr;
                    children.push(self.parse_unqualified_access_expr()?);
                }
                Token::NonNull => node_type = NodeType::NonNullExpr,
                _ => children.push(self.parse_expr(expectation, next_min_prec)?),
            }
            expr = Node::from_children(node_type, children);
            full_op = self.full_lookahead();
        }
        Ok(expr)
    }

    fn parse_unqualified_access_expr(&mut self) -> Result<Node, FormatError> {
        let mut children = Vec::new();
        children.push(self.parse_identifier()?);
        if self.lookahead() == Token::LParen
            && self.no_semicolon_inbetween()
            && self.current().new_lines_between == 0
        {
            self.ff_into(&mut children);
            children.push(self.parse_argument_list()?);
        }
        Ok(Node::from_children(
            NodeType::UnqualifiedAccessExpr,
            children,
        ))
    }

    fn parse_expr_atom(&mut self, expectation: Option<&str>) -> Result<Node, FormatError> {
        let expr = match self.lookahead_token() {
            Token::This => Node::leaf(NodeType::ThisExpr, self.next().span),
            Token::Outer => Node::leaf(NodeType::OuterExpr, self.next().span),
            Token::Module => Node::leaf(NodeType::ModuleExpr, self.next().span),
            Token::Null => Node::leaf(NodeType::NullExpr, self.next().span),
            Token::Throw => self.parse_unary_paren_expr(NodeType::ThrowExpr)?,
            Token::Trace => self.parse_unary_paren_expr(NodeType::TraceExpr)?,
            Token::Import | Token::ImportStar => self.parse_import_expr()?,
            Token::Read | Token::ReadStar | Token::ReadQuestion => {
                self.parse_unary_paren_expr(NodeType::ReadExpr)?
            }
            Token::New => self.parse_new_expr()?,
            Token::Minus => {
                self.parse_prefix_unary_expr(NodeType::UnaryMinusExpr, expectation, 12)?
            }
            Token::Not => {
                self.parse_prefix_unary_expr(NodeType::LogicalNotExpr, expectation, 11)?
            }
            Token::LParen => {
                if self.is_function_literal() {
                    self.parse_function_literal()?
                } else {
                    self.parse_parenthesized_expr()?
                }
            }
            Token::Super => self.parse_super_expr()?,
            Token::If => self.parse_if_expr(expectation)?,
            Token::Let => self.parse_let_expr()?,
            Token::True | Token::False => Node::leaf(NodeType::BoolLiteralExpr, self.next().span),
            Token::Int | Token::Hex | Token::Bin | Token::Oct => {
                Node::leaf(NodeType::IntLiteralExpr, self.next().span)
            }
            Token::Float => Node::leaf(NodeType::FloatLiteralExpr, self.next().span),
            Token::StringStart => self.parse_single_line_string_literal_expr()?,
            Token::StringMultiStart => self.parse_multi_line_string_literal_expr()?,
            Token::Identifier => self.parse_unqualified_access_expr()?,
            Token::Eof => {
                return Err(Self::parser_error_at(
                    error::unexpected_end_of_file(),
                    self.prev().span.stop_span(),
                ));
            }
            _ => {
                let text = self.current().text(&self.source);
                if let Some(exp) = expectation {
                    return Err(self.parser_error(error::unexpected_token(&text, exp)));
                }
                return Err(self.parser_error(error::unexpected_token_for_expression(&text)));
            }
        };
        self.parse_expr_rest(expr)
    }

    fn parse_unary_paren_expr(&mut self, kind: NodeType) -> Result<Node, FormatError> {
        let mut children = Vec::new();
        let tk = self.next();
        children.push(make_terminal(tk));
        self.ff_into(&mut children);
        self.expect_into(
            Token::LParen,
            &mut children,
            ExpectError::UnexpectedToken("("),
        )?;
        self.ff_into(&mut children);
        children.push(self.parse_expr(Some(")"), 1)?);
        self.ff_into(&mut children);
        self.expect_into(
            Token::RParen,
            &mut children,
            ExpectError::UnexpectedToken(")"),
        )?;
        Ok(Node::from_children(kind, children))
    }

    fn parse_import_expr(&mut self) -> Result<Node, FormatError> {
        let mut children = Vec::new();
        let tk = self.next();
        children.push(make_terminal(tk));
        self.ff_into(&mut children);
        self.expect_into(
            Token::LParen,
            &mut children,
            ExpectError::UnexpectedToken("("),
        )?;
        self.ff_into(&mut children);
        children.push(self.parse_string_constant()?);
        self.ff_into(&mut children);
        self.expect_into(
            Token::RParen,
            &mut children,
            ExpectError::UnexpectedToken(")"),
        )?;
        Ok(Node::from_children(NodeType::ImportExpr, children))
    }

    fn parse_new_expr(&mut self) -> Result<Node, FormatError> {
        let mut children = Vec::new();
        let mut header = Vec::new();
        let tk = self.next();
        header.push(make_terminal(tk));
        self.ff_into(&mut header);
        if self.lookahead_token() != Token::LBrace {
            header.push(self.parse_type(Some("{"))?);
            children.push(Node::from_children(NodeType::NewHeader, header));
            self.ff_into(&mut children);
        } else {
            children.push(Node::from_children(NodeType::NewHeader, header));
        }
        children.push(self.parse_object_body()?);
        Ok(Node::from_children(NodeType::NewExpr, children))
    }

    fn parse_prefix_unary_expr(
        &mut self,
        kind: NodeType,
        expectation: Option<&str>,
        min_precedence: u8,
    ) -> Result<Node, FormatError> {
        let mut children = Vec::new();
        let tk = self.next();
        children.push(make_terminal(tk));
        self.ff_into(&mut children);
        children.push(self.parse_expr(expectation, min_precedence)?);
        Ok(Node::from_children(kind, children))
    }

    fn parse_super_expr(&mut self) -> Result<Node, FormatError> {
        let mut children = Vec::new();
        let super_tk = self.next();
        children.push(make_terminal(super_tk));
        self.ff_into(&mut children);
        if self.lookahead_token() == Token::Dot {
            let dot = self.next();
            children.push(make_terminal(dot));
            self.ff_into(&mut children);
            children.push(self.parse_identifier()?);
            if self.lookahead() == Token::LParen {
                self.ff_into(&mut children);
                children.push(self.parse_argument_list()?);
            }
            Ok(Node::from_children(NodeType::SuperAccessExpr, children))
        } else {
            self.expect_into(
                Token::LBrack,
                &mut children,
                ExpectError::UnexpectedToken("["),
            )?;
            self.ff_into(&mut children);
            children.push(self.parse_expr(None, 1)?);
            self.ff_into(&mut children);
            self.expect_into(
                Token::RBrack,
                &mut children,
                ExpectError::UnexpectedToken("]"),
            )?;
            Ok(Node::from_children(NodeType::SuperSubscriptExpr, children))
        }
    }

    fn parse_if_expr(&mut self, expectation: Option<&str>) -> Result<Node, FormatError> {
        let mut children = Vec::new();
        let mut header = Vec::new();
        let if_tk = self.next();
        header.push(make_terminal(if_tk));
        self.ff_into(&mut header);
        let mut condition = Vec::new();
        let mut condition_expr = Vec::new();
        self.expect_into(
            Token::LParen,
            &mut condition,
            ExpectError::UnexpectedToken("("),
        )?;
        self.ff_into(&mut condition_expr);
        condition_expr.push(self.parse_expr(Some(")"), 1)?);
        self.ff_into(&mut condition_expr);
        condition.push(Node::from_children(
            NodeType::IfConditionExpr,
            condition_expr,
        ));
        self.expect_into(
            Token::RParen,
            &mut condition,
            ExpectError::UnexpectedToken(")"),
        )?;
        header.push(Node::from_children(NodeType::IfCondition, condition));
        children.push(Node::from_children(NodeType::IfHeader, header));
        let mut then_expr = Vec::new();
        self.ff_into(&mut then_expr);
        then_expr.push(self.parse_expr(Some("else"), 1)?);
        self.ff_into(&mut then_expr);
        children.push(Node::from_children(NodeType::IfThenExpr, then_expr));
        self.expect_into(
            Token::Else,
            &mut children,
            ExpectError::UnexpectedToken("else"),
        )?;
        let mut else_expr = Vec::new();
        self.ff_into(&mut else_expr);
        else_expr.push(self.parse_expr(expectation, 1)?);
        children.push(Node::from_children(NodeType::IfElseExpr, else_expr));
        Ok(Node::from_children(NodeType::IfExpr, children))
    }

    fn parse_let_expr(&mut self) -> Result<Node, FormatError> {
        let mut children = Vec::new();
        let let_tk = self.next();
        children.push(make_terminal(let_tk));
        self.ff_into(&mut children);
        let mut param_def = Vec::new();
        self.expect_into(
            Token::LParen,
            &mut param_def,
            ExpectError::UnexpectedToken("("),
        )?;
        let mut param = Vec::new();
        self.ff_into(&mut param);
        param.push(self.parse_parameter()?);
        self.ff_into(&mut param);
        self.expect_into(Token::Assign, &mut param, ExpectError::UnexpectedToken("="))?;
        self.ff_into(&mut param);
        param.push(self.parse_expr(Some(")"), 1)?);
        param_def.push(Node::from_children(NodeType::LetParameter, param));
        self.ff_into(&mut param_def);
        self.expect_into(
            Token::RParen,
            &mut param_def,
            ExpectError::UnexpectedToken(")"),
        )?;
        children.push(Node::from_children(
            NodeType::LetParameterDefinition,
            param_def,
        ));
        self.ff_into(&mut children);
        children.push(self.parse_expr(None, 1)?);
        Ok(Node::from_children(NodeType::LetExpr, children))
    }

    fn parse_expr_rest(&mut self, mut expr: Node) -> Result<Node, FormatError> {
        while self.lookahead() == Token::LBrace {
            let expr_kind = expr.kind;
            let mut children = Vec::new();
            children.push(expr);
            self.ff_into(&mut children);
            if matches!(
                expr_kind,
                NodeType::ParenthesizedExpr | NodeType::AmendsExpr | NodeType::NewExpr
            ) {
                children.push(self.parse_object_body()?);
                expr = Node::from_children(NodeType::AmendsExpr, children);
            } else {
                let expr_text = children[0].text(&self.source);
                return Err(
                    self.parser_error(error::unexpected_curly_probably_amends_expression(
                        &expr_text,
                    )),
                );
            }
        }
        Ok(expr)
    }

    fn is_function_literal(&mut self) -> bool {
        let original_cursor = self.cursor;
        let result = (|| {
            self.next(); // open (
            self.ff();
            let token = self.next().token;
            self.ff();
            if token == Token::RParen {
                return self.lookahead_token() == Token::Arrow;
            }
            if token == Token::Underscore {
                return true;
            }
            if token != Token::Identifier {
                return false;
            }
            if matches!(self.lookahead_token(), Token::Comma | Token::Colon) {
                return true;
            }
            if self.lookahead_token() == Token::RParen {
                self.next();
                self.ff();
                return self.lookahead_token() == Token::Arrow;
            }
            false
        })();
        self.backtrack_to(original_cursor);
        result
    }

    fn parse_single_line_string_literal_expr(&mut self) -> Result<Node, FormatError> {
        let mut children = Vec::new();
        let start = self.next();
        children.push(make_terminal(start));
        while self.lookahead_token() != Token::StringEnd {
            match self.lookahead_token() {
                Token::StringPart => {
                    let tk = self.next();
                    if tk.span.length > 0 {
                        children.push(Node::leaf(NodeType::StringChars, tk.span));
                    }
                }
                Token::StringEscapeNewline
                | Token::StringEscapeTab
                | Token::StringEscapeQuote
                | Token::StringEscapeBackslash
                | Token::StringEscapeReturn
                | Token::StringEscapeUnicode => {
                    let tk = self.next();
                    children.push(Node::leaf(NodeType::StringEscape, tk.span));
                }
                Token::StringEscapeContinuation => {
                    return Err(
                        self.parser_error(error::invalid_line_continuation_escape_sequence())
                    );
                }
                Token::InterpolationStart => {
                    let tk = self.next();
                    children.push(make_terminal(tk));
                    self.ff_into(&mut children);
                    children.push(self.parse_expr(Some(")"), 1)?);
                    self.ff_into(&mut children);
                    self.expect_into(
                        Token::RParen,
                        &mut children,
                        ExpectError::UnexpectedToken(")"),
                    )?;
                }
                Token::Eof => {
                    let delimiter: String = start.text(&self.source).chars().rev().collect();
                    return Err(self.parser_error(error::missing_delimiter(&delimiter)));
                }
                _ => {
                    let text = self.current().text(&self.source);
                    let delimiter: String = start.text(&self.source).chars().rev().collect();
                    return Err(self.parser_error(error::unexpected_token(&text, &delimiter)));
                }
            }
        }
        let end = self.next();
        children.push(make_terminal(end));
        Ok(Node::from_children(
            NodeType::SingleLineStringLiteralExpr,
            children,
        ))
    }

    fn parse_multi_line_string_literal_expr(&mut self) -> Result<Node, FormatError> {
        let mut children = Vec::new();
        let start = self.next();
        children.push(make_terminal(start));
        if self.lookahead_token() != Token::StringNewline {
            return Err(Self::parser_error_at(
                error::string_content_must_begin_on_new_line(),
                self.span_lookahead(),
            ));
        }
        while self.lookahead_token() != Token::StringEnd {
            match self.lookahead_token() {
                Token::StringPart => {
                    let tk = self.next();
                    if tk.span.length > 0 {
                        children.push(Node::leaf(NodeType::StringChars, tk.span));
                    }
                }
                Token::StringNewline => {
                    let tk = self.next();
                    children.push(Node::leaf(NodeType::StringNewline, tk.span));
                }
                Token::StringEscapeContinuation => {
                    let tk = self.next();
                    children.push(Node::leaf(NodeType::StringContinuation, tk.span));
                }
                Token::StringEscapeNewline
                | Token::StringEscapeTab
                | Token::StringEscapeQuote
                | Token::StringEscapeBackslash
                | Token::StringEscapeReturn
                | Token::StringEscapeUnicode => {
                    let tk = self.next();
                    children.push(Node::leaf(NodeType::StringEscape, tk.span));
                }
                Token::InterpolationStart => {
                    let tk = self.next();
                    children.push(make_terminal(tk));
                    self.ff_into(&mut children);
                    children.push(self.parse_expr(Some(")"), 1)?);
                    self.ff_into(&mut children);
                    self.expect_into(
                        Token::RParen,
                        &mut children,
                        ExpectError::UnexpectedToken(")"),
                    )?;
                }
                Token::Eof => {
                    let delimiter: String = start.text(&self.source).chars().rev().collect();
                    return Err(self.parser_error(error::missing_delimiter(&delimiter)));
                }
                _ => {
                    let text = self.current().text(&self.source);
                    let delimiter: String = start.text(&self.source).chars().rev().collect();
                    return Err(self.parser_error(error::unexpected_token(&text, &delimiter)));
                }
            }
        }
        let end = self.next();
        children.push(make_terminal(end));
        self.validate_string_end_delimiter(&children)?;
        self.validate_string_indentation(&children)?;
        Ok(Node::from_children(
            NodeType::MultiLineStringLiteralExpr,
            children,
        ))
    }

    fn validate_string_end_delimiter(&self, nodes: &[Node]) -> Result<(), FormatError> {
        let before_last = &nodes[nodes.len() - 2];
        if before_last.kind == NodeType::StringNewline {
            return Ok(());
        }
        let text = before_last.text(&self.source);
        if !is_java_blank(&text) {
            return Err(Self::parser_error_at(
                error::closing_string_delimiter_must_begin_on_new_line(),
                before_last.span,
            ));
        }
        Ok(())
    }

    fn validate_string_indentation(&self, nodes: &[Node]) -> Result<(), FormatError> {
        let indent_node = &nodes[nodes.len() - 2];
        if indent_node.kind == NodeType::StringNewline {
            return Ok(());
        }
        let indent = indent_node.text(&self.source);
        let mut previous_newline = false;
        for child in &nodes[1..nodes.len() - 2] {
            if child.kind != NodeType::StringNewline && previous_newline {
                let text = child.text(&self.source);
                if !text.starts_with(&indent) {
                    return Err(Self::parser_error_at(
                        error::string_indentation_must_match_last_line(),
                        child.span,
                    ));
                }
            }
            previous_newline = matches!(
                child.kind,
                NodeType::StringNewline | NodeType::StringContinuation
            );
        }
        Ok(())
    }

    fn parse_parenthesized_expr(&mut self) -> Result<Node, FormatError> {
        let mut children = Vec::new();
        self.expect_into(
            Token::LParen,
            &mut children,
            ExpectError::UnexpectedToken("("),
        )?;
        if self.lookahead() == Token::RParen {
            self.ff_into(&mut children);
            let rparen = self.next();
            children.push(make_terminal(rparen));
            return Ok(Node::from_children(NodeType::ParenthesizedExpr, children));
        }
        let mut elements = Vec::new();
        self.ff_into(&mut elements);
        elements.push(self.parse_expr(Some(")"), 1)?);
        self.ff_into(&mut elements);
        children.push(Node::from_children(
            NodeType::ParenthesizedExprElements,
            elements,
        ));
        self.expect_into(
            Token::RParen,
            &mut children,
            ExpectError::UnexpectedToken(")"),
        )?;
        Ok(Node::from_children(NodeType::ParenthesizedExpr, children))
    }

    fn parse_function_literal(&mut self) -> Result<Node, FormatError> {
        let mut param_list_children = Vec::new();
        self.expect_into(
            Token::LParen,
            &mut param_list_children,
            ExpectError::UnexpectedToken("("),
        )?;
        if self.lookahead() == Token::RParen {
            self.ff_into(&mut param_list_children);
            let rparen = self.next();
            param_list_children.push(make_terminal(rparen));
        } else {
            let mut elements = Vec::new();
            self.ff_into(&mut elements);
            self.parse_list_of(Token::RParen, &mut elements, Self::parse_parameter)?;
            param_list_children.push(Node::from_children(
                NodeType::ParameterListElements,
                elements,
            ));
            self.expect_into(
                Token::RParen,
                &mut param_list_children,
                ExpectError::UnexpectedToken2(",", ")"),
            )?;
        }
        let mut children = Vec::new();
        children.push(Node::from_children(
            NodeType::ParameterList,
            param_list_children,
        ));
        self.ff_into(&mut children);
        self.expect_into(
            Token::Arrow,
            &mut children,
            ExpectError::UnexpectedToken("->"),
        )?;
        let mut body = Vec::new();
        self.ff_into(&mut body);
        body.push(self.parse_expr(None, 1)?);
        children.push(Node::from_children(NodeType::FunctionLiteralBody, body));
        Ok(Node::from_children(NodeType::FunctionLiteralExpr, children))
    }

    fn parse_type(&mut self, expectation: Option<&str>) -> Result<Node, FormatError> {
        self.enter_depth()?;
        let res = self.parse_type_inner(expectation);
        self.leave_depth();
        res
    }

    fn parse_type_inner(&mut self, expectation: Option<&str>) -> Result<Node, FormatError> {
        let mut children = Vec::new();
        let mut has_default = false;
        let mut start = None;
        if self.lookahead_token() == Token::Star {
            let tk = self.next();
            start = Some(tk.span);
            children.push(make_terminal(tk));
            self.ff_into(&mut children);
            has_default = true;
        }
        let first = self.parse_type_atom(expectation)?;
        let first_span = first.span;
        children.push(first);

        if self.lookahead() != Token::Union {
            if has_default {
                let s = start.expect("start span set when has_default");
                return Err(Self::parser_error_at(
                    error::not_a_union(),
                    s.end_with(first_span),
                ));
            }
            return Ok(children.pop().expect("first type present"));
        }

        while self.lookahead() == Token::Union {
            self.ff_into(&mut children);
            let union_tk = self.next();
            children.push(make_terminal(union_tk));
            self.ff_into(&mut children);
            if self.lookahead_token() == Token::Star {
                if has_default {
                    return Err(self.parser_error(error::multiple_union_defaults()));
                }
                let star = self.next();
                children.push(make_terminal(star));
                self.ff_into(&mut children);
                has_default = true;
            }
            let ty = self.parse_type_atom(expectation)?;
            children.push(ty);
        }
        Ok(Node::from_children(NodeType::UnionType, children))
    }

    fn parse_type_atom(&mut self, expectation: Option<&str>) -> Result<Node, FormatError> {
        let typ = match self.lookahead_token() {
            Token::Unknown => Node::leaf(NodeType::UnknownType, self.next().span),
            Token::Nothing => Node::leaf(NodeType::NothingType, self.next().span),
            Token::Module => Node::leaf(NodeType::ModuleType, self.next().span),
            Token::LParen => self.parse_parenthesized_or_function_type(expectation)?,
            Token::Identifier => {
                let mut children = Vec::new();
                children.push(self.parse_qualified_identifier()?);
                if self.lookahead() == Token::Lt {
                    self.ff_into(&mut children);
                    children.push(self.parse_type_argument_list()?);
                }
                Node::from_children(NodeType::DeclaredType, children)
            }
            Token::StringStart => Node::from_children(
                NodeType::StringConstantType,
                vec![self.parse_string_constant()?],
            ),
            _ => {
                let text = self.current().text(&self.source);
                if let Some(exp) = expectation {
                    return Err(self.parser_error(error::unexpected_token_for_type_2(&text, exp)));
                }
                return Err(self.parser_error(error::unexpected_token_for_type(&text)));
            }
        };

        if typ.kind == NodeType::FunctionType {
            return Ok(typ);
        }
        self.parse_type_end(typ)
    }

    fn parse_parenthesized_or_function_type(
        &mut self,
        expectation: Option<&str>,
    ) -> Result<Node, FormatError> {
        let mut children = Vec::new();
        let lparen = self.next();
        children.push(make_terminal(lparen));
        let mut total_types = 0usize;
        if self.lookahead() == Token::RParen {
            self.ff_into(&mut children);
            let rparen = self.next();
            children.push(make_terminal(rparen));
        } else {
            let mut elements = Vec::new();
            self.ff_into(&mut elements);
            elements.push(self.parse_type(Some(")"))?);
            self.ff_into(&mut elements);
            while self.lookahead_token() == Token::Comma {
                let comma = self.next();
                if self.lookahead() == Token::RParen {
                    self.ff_into(&mut elements);
                    break;
                }
                elements.push(make_terminal(comma));
                self.ff_into(&mut elements);
                elements.push(self.parse_type(Some(")"))?);
                total_types += 1;
                self.ff_into(&mut elements);
            }
            children.push(Node::from_children(
                NodeType::ParenthesizedTypeElements,
                elements,
            ));
            self.expect_into(
                Token::RParen,
                &mut children,
                ExpectError::UnexpectedToken2(",", ")"),
            )?;
        }
        if total_types > 1 || self.lookahead() == Token::Arrow {
            let mut actual_children = Vec::new();
            actual_children.push(Node::from_children(
                NodeType::FunctionTypeParameters,
                children,
            ));
            self.ff_into(&mut actual_children);
            self.expect_into(
                Token::Arrow,
                &mut actual_children,
                ExpectError::UnexpectedToken("->"),
            )?;
            self.ff_into(&mut actual_children);
            actual_children.push(self.parse_type(expectation)?);
            Ok(Node::from_children(NodeType::FunctionType, actual_children))
        } else {
            Ok(Node::from_children(NodeType::ParenthesizedType, children))
        }
    }

    fn parse_type_end(&mut self, mut ty: Node) -> Result<Node, FormatError> {
        loop {
            if self.lookahead() == Token::Question {
                let mut children = vec![ty];
                self.ff_into(&mut children);
                let q = self.next();
                children.push(make_terminal(q));
                ty = Node::from_children(NodeType::NullableType, children);
                continue;
            }
            let fla = self.full_lookahead();
            if fla.tk.token == Token::LParen
                && self.no_semicolon_inbetween()
                && fla.tk.new_lines_between == 0
            {
                let mut children = vec![ty];
                self.ff_into(&mut children);
                let mut constraint = Vec::new();
                let lparen = self.next();
                constraint.push(make_terminal(lparen));
                let mut elements = Vec::new();
                self.ff_into(&mut elements);
                self.parse_list_of(Token::RParen, &mut elements, |p| p.parse_expr(Some(")"), 1))?;
                constraint.push(Node::from_children(
                    NodeType::ConstrainedTypeElements,
                    elements,
                ));
                self.expect_into(
                    Token::RParen,
                    &mut constraint,
                    ExpectError::UnexpectedToken2(",", ")"),
                )?;
                children.push(Node::from_children(
                    NodeType::ConstrainedTypeConstraint,
                    constraint,
                ));
                ty = Node::from_children(NodeType::ConstrainedType, children);
                continue;
            }
            return Ok(ty);
        }
    }

    fn parse_annotation(&mut self) -> Result<Node, FormatError> {
        let mut children = Vec::new();
        let at = self.next();
        children.push(make_terminal(at));
        children.push(self.parse_type(None)?);
        if self.lookahead() == Token::LBrace {
            self.ff_into(&mut children);
            children.push(self.parse_object_body()?);
        }
        Ok(Node::from_children(NodeType::Annotation, children))
    }

    fn parse_parameter(&mut self) -> Result<Node, FormatError> {
        if self.lookahead_token() == Token::Underscore {
            let tk = self.next();
            return Ok(Node::from_children(
                NodeType::Parameter,
                vec![make_terminal(tk)],
            ));
        }
        self.parse_typed_identifier()
    }

    fn parse_typed_identifier(&mut self) -> Result<Node, FormatError> {
        let mut children = Vec::new();
        children.push(self.parse_identifier()?);
        if self.lookahead() == Token::Colon {
            self.ff_into(&mut children);
            children.push(self.parse_type_annotation()?);
        }
        Ok(Node::from_children(NodeType::Parameter, children))
    }

    fn parse_parameter_list(&mut self) -> Result<Node, FormatError> {
        let mut children = Vec::new();
        self.expect_into(
            Token::LParen,
            &mut children,
            ExpectError::UnexpectedToken("("),
        )?;
        self.ff_into(&mut children);
        if self.lookahead_token() == Token::RParen {
            let rparen = self.next();
            children.push(make_terminal(rparen));
        } else {
            let mut elements = Vec::new();
            self.parse_list_of(Token::RParen, &mut elements, Self::parse_parameter)?;
            children.push(Node::from_children(
                NodeType::ParameterListElements,
                elements,
            ));
            self.expect_into(
                Token::RParen,
                &mut children,
                ExpectError::UnexpectedToken2(",", ")"),
            )?;
        }
        Ok(Node::from_children(NodeType::ParameterList, children))
    }

    fn parse_body_list(&mut self) -> Result<Vec<Node>, FormatError> {
        if self.lookahead_token() != Token::LBrace {
            let text = self.current().text(&self.source);
            return Err(self.parser_error(error::unexpected_token_2(&text, "{", "=")));
        }
        let mut bodies = Vec::new();
        loop {
            bodies.push(self.parse_object_body()?);
            if self.lookahead() != Token::LBrace {
                break;
            }
        }
        Ok(bodies)
    }

    fn parse_type_parameter_list(&mut self) -> Result<Node, FormatError> {
        let mut children = Vec::new();
        self.expect_into(Token::Lt, &mut children, ExpectError::UnexpectedToken("<"))?;
        self.ff_into(&mut children);
        let mut elements = Vec::new();
        self.parse_list_of(Token::Gt, &mut elements, Self::parse_type_parameter)?;
        children.push(Node::from_children(
            NodeType::TypeParameterListElements,
            elements,
        ));
        self.expect_into(
            Token::Gt,
            &mut children,
            ExpectError::UnexpectedToken2(",", ">"),
        )?;
        Ok(Node::from_children(NodeType::TypeParameterList, children))
    }

    fn parse_type_argument_list(&mut self) -> Result<Node, FormatError> {
        let mut children = Vec::new();
        self.expect_into(Token::Lt, &mut children, ExpectError::UnexpectedToken("<"))?;
        self.ff_into(&mut children);
        let mut elements = Vec::new();
        self.parse_list_of(Token::Gt, &mut elements, |p| p.parse_type(Some(">")))?;
        children.push(Node::from_children(
            NodeType::TypeArgumentListElements,
            elements,
        ));
        self.expect_into(
            Token::Gt,
            &mut children,
            ExpectError::UnexpectedToken2(",", ">"),
        )?;
        Ok(Node::from_children(NodeType::TypeArgumentList, children))
    }

    fn parse_argument_list(&mut self) -> Result<Node, FormatError> {
        let mut children = Vec::new();
        self.expect_into(
            Token::LParen,
            &mut children,
            ExpectError::UnexpectedToken("("),
        )?;
        if self.lookahead() == Token::RParen {
            self.ff_into(&mut children);
            let rparen = self.next();
            children.push(make_terminal(rparen));
            return Ok(Node::from_children(NodeType::ArgumentList, children));
        }
        let mut elements = Vec::new();
        self.ff_into(&mut elements);
        self.parse_list_of(Token::RParen, &mut elements, |p| p.parse_expr(Some(")"), 1))?;
        self.ff_into(&mut elements);
        children.push(Node::from_children(
            NodeType::ArgumentListElements,
            elements,
        ));
        self.expect_into(
            Token::RParen,
            &mut children,
            ExpectError::UnexpectedToken2(",", ")"),
        )?;
        Ok(Node::from_children(NodeType::ArgumentList, children))
    }

    fn parse_type_parameter(&mut self) -> Result<Node, FormatError> {
        let mut children = Vec::new();
        if matches!(self.lookahead_token(), Token::In | Token::Out) {
            let tk = self.next();
            children.push(make_terminal(tk));
        }
        children.push(self.parse_identifier()?);
        Ok(Node::from_children(NodeType::TypeParameter, children))
    }

    fn parse_type_annotation(&mut self) -> Result<Node, FormatError> {
        let mut children = Vec::new();
        self.expect_into(
            Token::Colon,
            &mut children,
            ExpectError::UnexpectedToken(":"),
        )?;
        self.ff_into(&mut children);
        children.push(self.parse_type(None)?);
        Ok(Node::from_children(NodeType::TypeAnnotation, children))
    }

    fn parse_identifier(&mut self) -> Result<Node, FormatError> {
        if self.lookahead_token() != Token::Identifier {
            if self.lookahead_token().is_keyword() {
                return Err(self.parser_error(error::keyword_not_allowed_here(
                    self.lookahead_token().text(),
                )));
            }
            let text = self.current().text(&self.source);
            return Err(self.parser_error(error::unexpected_token(&text, "identifier")));
        }
        Ok(Node::leaf(NodeType::Identifier, self.next().span))
    }

    fn parse_string_constant(&mut self) -> Result<Node, FormatError> {
        let mut children = Vec::new();
        let start_tk = self.expect(Token::StringStart, ExpectError::UnexpectedToken("\""))?;
        children.push(make_terminal(start_tk));
        while self.lookahead_token() != Token::StringEnd {
            match self.lookahead_token() {
                Token::StringPart
                | Token::StringEscapeNewline
                | Token::StringEscapeTab
                | Token::StringEscapeQuote
                | Token::StringEscapeBackslash
                | Token::StringEscapeReturn
                | Token::StringEscapeUnicode => {
                    let tk = self.next();
                    children.push(make_terminal(tk));
                }
                Token::StringEscapeContinuation => {
                    return Err(
                        self.parser_error(error::invalid_line_continuation_escape_sequence())
                    );
                }
                Token::Eof => {
                    let delimiter: String = start_tk.text(&self.source).chars().rev().collect();
                    return Err(self.parser_error(error::missing_delimiter(&delimiter)));
                }
                Token::InterpolationStart => {
                    return Err(self.parser_error(error::interpolation_in_constant()));
                }
                _ => {
                    let text = self.current().text(&self.source);
                    let delimiter: String = start_tk.text(&self.source).chars().rev().collect();
                    return Err(self.parser_error(error::unexpected_token(&text, &delimiter)));
                }
            }
        }
        let end_tk = self.next();
        children.push(make_terminal(end_tk));
        Ok(Node::from_children(NodeType::StringChars, children))
    }

    fn parse_list_of<F>(
        &mut self,
        terminator: Token,
        children: &mut Vec<Node>,
        mut item_parser: F,
    ) -> Result<(), FormatError>
    where
        F: FnMut(&mut Self) -> Result<Node, FormatError>,
    {
        children.push(item_parser(self)?);
        self.ff_into(children);
        while self.lookahead_token() == Token::Comma {
            let comma = make_terminal(self.next());
            if self.lookahead() == terminator {
                break;
            }
            children.push(comma);
            self.ff_into(children);
            children.push(item_parser(self)?);
            self.ff_into(children);
        }
        Ok(())
    }
}

fn get_header_parts(mut nodes: Vec<Node>) -> HeaderParts {
    if nodes
        .last()
        .is_some_and(|n| n.kind == NodeType::ModifierList)
    {
        let modifier_list = nodes.pop();
        HeaderParts {
            prefixes: nodes,
            modifier_list,
        }
    } else {
        HeaderParts {
            prefixes: nodes,
            modifier_list: None,
        }
    }
}

fn make_affix(tk: FullToken) -> Node {
    let kind = match tk.token {
        Token::LineComment => NodeType::LineComment,
        Token::BlockComment => NodeType::BlockComment,
        Token::Shebang => NodeType::Shebang,
        Token::Semicolon => NodeType::Semicolon,
        other => unreachable!("not an affix token: {other:?}"),
    };
    Node::leaf(kind, tk.span)
}

fn make_terminal(tk: FullToken) -> Node {
    Node::leaf(NodeType::Terminal, tk.span)
}
