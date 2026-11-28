//! Formatting IR builder mirroring `org.pkl.formatter.Builder`.

use crate::GrammarVersion;
use crate::cst::{Node, NodeType};
use crate::format_node::FormatNode;
use crate::natural_order::compare_natural;
use crate::token::Operator;
use std::cmp::Ordering;

pub(crate) struct Builder<'a> {
    id: usize,
    source: &'a [u16],
    no_newlines: bool,
    grammar_version: GrammarVersion,
}

struct ImportWithComments<'a> {
    leading_affixes: Vec<&'a Node>,
    import_node: &'a Node,
    trailing_affixes: Vec<&'a Node>,
}

impl<'a> Builder<'a> {
    pub(crate) fn new(source: &'a [u16], grammar_version: GrammarVersion) -> Self {
        Self {
            id: 0,
            source,
            no_newlines: false,
            grammar_version,
        }
    }

    pub(crate) fn format(&mut self, node: &Node) -> FormatNode {
        match node.kind {
            NodeType::Module => self.format_module(node),
            NodeType::DocComment
            | NodeType::ObjectSpread
            | NodeType::SuperAccessExpr
            | NodeType::ImportExpr => {
                FormatNode::Nodes(self.format_generic_const(&node.children, None))
            }
            NodeType::NonNullExpr
            | NodeType::NullableType
            | NodeType::SubscriptExpr
            | NodeType::SuperSubscriptExpr
            | NodeType::AmendsExpr
            | NodeType::QualifiedAccessExpr
            | NodeType::BinaryOpExpr
            | NodeType::ConstrainedType
            | NodeType::UnionType => self.format_left_spine(node),
            NodeType::UnaryMinusExpr | NodeType::LogicalNotExpr => self.format_unary_chain(node),
            NodeType::DocCommentLine => self.format_doc_comment(node),
            NodeType::LineComment
            | NodeType::BlockComment
            | NodeType::Terminal
            | NodeType::Modifier
            | NodeType::Identifier
            | NodeType::StringChars
            | NodeType::StringEscape
            | NodeType::IntLiteralExpr
            | NodeType::FloatLiteralExpr
            | NodeType::BoolLiteralExpr
            | NodeType::ThisExpr
            | NodeType::OuterExpr
            | NodeType::ModuleExpr
            | NodeType::NullExpr
            | NodeType::ModuleType
            | NodeType::UnknownType
            | NodeType::NothingType
            | NodeType::Shebang
            | NodeType::Operator => FormatNode::Text(self.text(node)),
            NodeType::StringNewline => self.must_force_line(),
            NodeType::StringContinuation => {
                // Mirror upstream `Builder.java` (`escape.substring(0, escape.length() - 1)`):
                // preserves CRLF `\\\r\n` as `\\\r` + ForceLine (`\n`) while staying panic-free.
                let mut prefix = self.text(node);
                prefix.pop();
                FormatNode::Nodes(vec![FormatNode::Text(prefix), self.must_force_line()])
            }
            NodeType::ModuleDeclaration => self.format_module_declaration(node),
            NodeType::ModuleDefinition => self.format_module_definition(node),
            NodeType::SingleLineStringLiteralExpr => self.format_single_line_string(node),
            NodeType::MultiLineStringLiteralExpr => self.format_multiline_string(node),
            NodeType::Annotation => self.format_annotation(node),
            NodeType::Typealias => self.format_typealias(node),
            NodeType::TypealiasHeader => self.format_typealias_header(node),
            NodeType::TypealiasBody => self.format_typealias_body(node),
            NodeType::ModifierList => self.format_modifier_list(node),
            NodeType::ParameterList
            | NodeType::ConstrainedTypeConstraint
            | NodeType::FunctionTypeParameters => self.format_parameter_list(node, None),
            NodeType::ParameterListElements
            | NodeType::TypeParameterListElements
            | NodeType::TypeArgumentListElements
            | NodeType::ConstrainedTypeElements => self.format_parameter_list_elements(node),
            NodeType::TypeParameterList | NodeType::TypeArgumentList => {
                self.format_type_parameter_list(node)
            }
            NodeType::TypeParameter | NodeType::ImportAlias => {
                let id = self.new_id();
                let sep = self.space_or_line();
                let nodes = self.format_generic_const(&node.children, Some(sep));
                FormatNode::Group { id, nodes }
            }
            NodeType::Parameter => self.format_parameter(node),
            NodeType::ExtendsClause | NodeType::AmendsClause => {
                self.format_amends_extends_clause(node)
            }
            NodeType::ImportList => self.format_import_list(node),
            NodeType::Import => self.format_import(node),
            NodeType::Class => self.format_class(node),
            NodeType::ClassHeader => self.format_class_header(node),
            NodeType::ClassHeaderExtends => self.format_class_header_extends(node),
            NodeType::ClassBody => self.format_class_body(node),
            NodeType::ClassBodyElements => self.format_class_body_elements(node),
            NodeType::ClassProperty | NodeType::ObjectProperty | NodeType::ObjectEntry => {
                self.format_class_property(node)
            }
            NodeType::ClassPropertyHeader | NodeType::ObjectPropertyHeader => {
                self.format_class_property_header(node)
            }
            NodeType::ClassPropertyHeaderBegin | NodeType::ObjectPropertyHeaderBegin => {
                self.format_class_property_header_begin(node)
            }
            NodeType::ClassPropertyBody | NodeType::ObjectPropertyBody => {
                self.format_class_property_body(node)
            }
            NodeType::ClassMethod | NodeType::ObjectMethod => self.format_class_method(node),
            NodeType::ClassMethodHeader => self.format_class_method_header(node),
            NodeType::ClassMethodBody => self.format_class_method_body(node),
            NodeType::ObjectBody => self.format_object_body(node),
            NodeType::ObjectElement => self.format(&node.children[0]),
            NodeType::ObjectEntryHeader => self.format_object_entry_header(node),
            NodeType::ForGenerator => self.format_for_generator(node),
            NodeType::ForGeneratorHeader => self.format_for_generator_header(node),
            NodeType::ForGeneratorHeaderDefinition => {
                self.format_for_generator_header_definition(node)
            }
            NodeType::ForGeneratorHeaderDefinitionHeader => {
                self.format_for_generator_header_definition_header(node)
            }
            NodeType::WhenGenerator => self.format_when_generator(node),
            NodeType::WhenGeneratorHeader => self.format_when_generator_header(node),
            NodeType::MemberPredicate => self.format_member_predicate(node),
            NodeType::QualifiedIdentifier => self.format_qualified_identifier(node),
            NodeType::ArgumentList => self.format_argument_list(node, false),
            NodeType::ArgumentListElements => {
                self.format_argument_list_elements(node, false, false)
            }
            NodeType::ObjectParameterList => self.format_object_parameter_list(node),
            NodeType::IfExpr => self.format_if(node),
            NodeType::IfHeader => self.format_if_header(node),
            NodeType::IfCondition => self.format_if_condition(node),
            NodeType::IfConditionExpr => {
                FormatNode::Indent(self.format_generic_const(&node.children, None))
            }
            NodeType::IfThenExpr => self.format_if_then(node),
            NodeType::IfElseExpr => self.format_if_else(node),
            NodeType::NewExpr => self.format_new_expr(node),
            NodeType::NewHeader => self.format_new_header(node),
            NodeType::UnqualifiedAccessExpr => self.format_unqualified_access_expression(node),
            NodeType::FunctionLiteralExpr => self.format_function_literal_expr(node),
            NodeType::FunctionLiteralBody => self.format_function_literal_body(node),
            NodeType::TraceExpr | NodeType::ThrowExpr | NodeType::ReadExpr => {
                self.format_trace_throw_read_expr(node)
            }
            NodeType::ParenthesizedExpr => self.format_parenthesized_expr(node),
            NodeType::ParenthesizedExprElements => self.format_parenthesized_expr_elements(node),
            NodeType::LetExpr => self.format_let_expr(node),
            NodeType::LetParameterDefinition => self.format_let_parameter_definition(node),
            NodeType::LetParameter => self.format_let_parameter(node),
            NodeType::TypeAnnotation => self.format_type_annotation(node),
            NodeType::DeclaredType => self.format_declared_type(node),
            NodeType::FunctionType => self.format_function_type(node),
            NodeType::StringConstantType => self.format(&node.children[0]),
            NodeType::ParenthesizedType => self.format_parenthesized_type(node),
            NodeType::ParenthesizedTypeElements => self.format_parenthesized_type_elements(node),
            other => panic!("Unknown node type: {other:?}"),
        }
    }

    fn format_module(&mut self, node: &Node) -> FormatNode {
        let nodes = self.format_generic(&node.children, |this, prev, next| {
            let mut sep = if lines_between(prev, next) > 1 {
                two_newlines()
            } else {
                this.force_line()
            };
            if matches!(
                prev.kind,
                NodeType::ModuleDeclaration | NodeType::ImportList
            ) {
                sep = two_newlines();
            }
            Some(sep)
        });
        FormatNode::Nodes(nodes)
    }

    fn format_module_declaration(&mut self, node: &Node) -> FormatNode {
        FormatNode::Nodes(self.format_generic_const(&node.children, Some(two_newlines())))
    }

    fn format_module_definition(&mut self, node: &Node) -> FormatNode {
        let (prefixes, rest) = split_prefixes(&node.children);
        let fnodes = self.format_generic_with_gen(
            rest,
            |_, _, _| Some(FormatNode::Space),
            |this, n, _| {
                if n.kind == NodeType::QualifiedIdentifier {
                    FormatNode::Nodes(this.format_generic_const(&n.children, None))
                } else {
                    this.format(n)
                }
            },
        );
        let res = FormatNode::Nodes(fnodes);
        if prefixes.is_empty() {
            return res;
        }
        let default_sep = self.space_or_line();
        let sep = self.get_separator_const(&prefixes[prefixes.len() - 1], &rest[0], default_sep);
        let mut result = self.format_generic_const(prefixes, Some(self.space_or_line()));
        result.push(sep);
        result.push(res);
        FormatNode::Nodes(result)
    }

    fn format_doc_comment(&self, node: &Node) -> FormatNode {
        let txt = self.text(node);
        if txt == "///" || txt == "/// " {
            return FormatNode::Text("///".to_string());
        }
        let comment = &txt[3..];
        if is_strict_blank(comment) {
            return FormatNode::Text("///".to_string());
        }
        if !comment.is_empty() && !comment.starts_with(' ') {
            FormatNode::Text(format!("/// {comment}"))
        } else {
            FormatNode::Text(format!("///{comment}"))
        }
    }

    fn format_qualified_identifier(&mut self, node: &Node) -> FormatNode {
        if node.children.len() == 1 {
            return self.format(&node.children[0]);
        }
        let mut first = Vec::new();
        first.push(self.format(&node.children[0]));
        first.push(self.line());
        let rest = &node.children[1..];
        let nodes = self.format_generic(rest, |this, n1, _| {
            if n1.kind == NodeType::Terminal {
                None
            } else {
                Some(this.line())
            }
        });
        first.push(FormatNode::Indent(nodes));
        FormatNode::Group {
            id: self.new_id(),
            nodes: first,
        }
    }

    fn format_unqualified_access_expression(&mut self, node: &Node) -> FormatNode {
        let children = &node.children;
        if children.len() == 1 {
            return self.format(&children[0]);
        }
        if first_proper_child(node).is_some_and(|first| self.text(first) == "Map") {
            let nodes = self.format_generic_with_gen(
                children,
                |_, _, _| None,
                |this, n, _| {
                    if n.kind == NodeType::ArgumentList {
                        this.format_argument_list(n, true)
                    } else {
                        this.format(n)
                    }
                },
            );
            return FormatNode::Nodes(nodes);
        }
        FormatNode::Nodes(self.format_generic_const(children, None))
    }

    fn format_amends_extends_clause(&mut self, node: &Node) -> FormatNode {
        let sep = self.space_or_line();
        let mut result =
            self.format_generic_const(&node.children[..node.children.len() - 1], Some(sep));
        let suffix = FormatNode::Indent(vec![self.format(&node.children[node.children.len() - 1])]);
        result.push(self.space_or_line());
        result.push(suffix);
        FormatNode::Group {
            id: self.new_id(),
            nodes: result,
        }
    }

    fn format_import(&mut self, node: &Node) -> FormatNode {
        let sep = self.space_or_line();
        let nodes = self.format_generic_with_gen(
            &node.children,
            |_, _, _| Some(sep.clone()),
            |this, n, _| {
                if this.is_terminal_single(n, "import") {
                    this.format(n)
                } else {
                    indent_one(this.format(n))
                }
            },
        );
        FormatNode::Group {
            id: self.new_id(),
            nodes,
        }
    }

    fn format_annotation(&mut self, node: &Node) -> FormatNode {
        let sep = self.space_or_line();
        let nodes = self.format_generic_const(&node.children, Some(sep));
        FormatNode::Group {
            id: self.new_id(),
            nodes,
        }
    }

    fn format_typealias(&mut self, node: &Node) -> FormatNode {
        let nodes = self.group_non_prefixes(node, |this, children| {
            let sep = this.space_or_line();
            let inner = this.format_generic_const(children, Some(sep));
            FormatNode::Group {
                id: this.new_id(),
                nodes: inner,
            }
        });
        FormatNode::Nodes(nodes)
    }

    fn format_typealias_header(&mut self, node: &Node) -> FormatNode {
        let nodes = self.format_generic_const(&node.children, Some(FormatNode::Space));
        FormatNode::Group {
            id: self.new_id(),
            nodes,
        }
    }

    fn format_typealias_body(&mut self, node: &Node) -> FormatNode {
        let sep = self.space_or_line();
        FormatNode::Indent(self.format_generic_const(&node.children, Some(sep)))
    }

    fn format_class(&mut self, node: &Node) -> FormatNode {
        let sep = self.space_or_line();
        FormatNode::Nodes(self.format_generic_const(&node.children, Some(sep)))
    }

    fn format_class_header(&mut self, node: &Node) -> FormatNode {
        let sep = self.space_or_line();
        let nodes = self.format_generic_const(&node.children, Some(sep));
        self.group_on_space(nodes)
    }

    fn format_class_header_extends(&mut self, node: &Node) -> FormatNode {
        let sep = self.space_or_line();
        let nodes = self.format_generic_const(&node.children, Some(sep));
        indent_one(FormatNode::Group {
            id: self.new_id(),
            nodes,
        })
    }

    fn format_class_body(&mut self, node: &Node) -> FormatNode {
        if node.children.len() == 2 {
            return FormatNode::Nodes(self.format_generic_const(&node.children, None));
        }
        let sep = self.force_line();
        let nodes = self.format_generic_const(&node.children, Some(sep));
        FormatNode::Group {
            id: self.new_id(),
            nodes,
        }
    }

    fn format_class_body_elements(&mut self, node: &Node) -> FormatNode {
        let nodes = self.format_generic(&node.children, |this, prev, next| {
            let line_diff = lines_between(prev, next);
            Some(if line_diff > 1 || line_diff == 0 {
                two_newlines()
            } else {
                this.force_line()
            })
        });
        FormatNode::Indent(nodes)
    }

    fn format_class_property(&mut self, node: &Node) -> FormatNode {
        let last_expr_or_body = node
            .children
            .iter()
            .rev()
            .find(|child| is_expression_or_property_body(child));
        let same_line = if let Some(last) = last_expr_or_body {
            if last.kind.is_expression() {
                is_same_line_expr(last)
            } else {
                is_same_line_expr(&last.children[last.children.len() - 1])
            }
        } else {
            false
        };
        let nodes = self.group_non_prefixes(node, |this, children| {
            let formatted = this.format_generic_with_gen(
                children,
                |this, _, _| {
                    Some(if same_line {
                        FormatNode::Space
                    } else {
                        this.space_or_line()
                    })
                },
                |this, n, _| {
                    if is_expression_or_property_body(n) && !same_line {
                        indent_one(this.format(n))
                    } else {
                        this.format(n)
                    }
                },
            );
            this.group_on_space(formatted)
        });
        FormatNode::Nodes(nodes)
    }

    fn format_class_property_header(&mut self, node: &Node) -> FormatNode {
        let sep = self.space_or_line();
        let nodes = self.format_generic_const(&node.children, Some(sep));
        FormatNode::Group {
            id: self.new_id(),
            nodes,
        }
    }

    fn format_class_property_header_begin(&mut self, node: &Node) -> FormatNode {
        let sep = self.space_or_line();
        let nodes = self.format_generic_const(&node.children, Some(sep));
        FormatNode::Group {
            id: self.new_id(),
            nodes,
        }
    }

    fn format_class_property_body(&mut self, node: &Node) -> FormatNode {
        FormatNode::Nodes(self.format_generic_const(&node.children, None))
    }

    fn format_class_method(&mut self, node: &Node) -> FormatNode {
        let mut prefixes = Vec::new();
        let method_nodes = if node.children[0].kind == NodeType::ClassMethodHeader {
            &node.children[..]
        } else {
            let idx = node
                .children
                .iter()
                .position(|c| c.kind == NodeType::ClassMethodHeader)
                .expect("ClassMethod must contain ClassMethodHeader");
            let prefix_nodes = &node.children[..idx];
            prefixes.extend(self.format_generic_const(prefix_nodes, None));
            let force = self.force_line();
            prefixes.push(self.get_separator_const(
                &prefix_nodes[prefix_nodes.len() - 1],
                &node.children[idx],
                force,
            ));
            &node.children[idx..]
        };

        let body_idx = method_nodes
            .iter()
            .position(|n| n.kind == NodeType::ClassMethodBody)
            .map(|i| i - 1);

        let header = match body_idx {
            Some(idx) => &method_nodes[..idx],
            None => method_nodes,
        };
        let header_group_id = self.new_id();
        let method_group_id = self.new_id();
        let sep = self.space_or_line();
        let header_nodes = self.format_generic_with_gen(
            header,
            |_, _, _| Some(sep.clone()),
            |this, n, _| {
                if n.kind == NodeType::ParameterList {
                    this.format_parameter_list(n, Some(header_group_id))
                } else {
                    this.format(n)
                }
            },
        );
        let Some(body_idx) = body_idx else {
            let header_group = FormatNode::Group {
                id: header_group_id,
                nodes: header_nodes,
            };
            if prefixes.is_empty() {
                return header_group;
            }
            prefixes.push(header_group);
            return FormatNode::Nodes(prefixes);
        };

        let body_nodes = &method_nodes[body_idx..];
        let expr = &body_nodes[body_nodes.len() - 1].children[0];
        let is_same_line_body = is_same_line_expr(expr);

        let body_format = if is_same_line_body {
            self.format_generic_const(body_nodes, Some(FormatNode::Space))
        } else {
            let body_sep = self.space_or_line();
            self.format_generic_with_gen(
                body_nodes,
                |_, _, _| Some(body_sep.clone()),
                |this, n, next| {
                    if next.is_none() {
                        indent_one(this.format(n))
                    } else {
                        this.format(n)
                    }
                },
            )
        };

        let header_group = FormatNode::Group {
            id: header_group_id,
            nodes: header_nodes,
        };
        let body_group = FormatNode::Group {
            id: self.new_id(),
            nodes: body_format,
        };
        let separator =
            self.get_separator_const(&header[header.len() - 1], &body_nodes[0], FormatNode::Space);
        let all_nodes = FormatNode::Group {
            id: method_group_id,
            nodes: vec![header_group, separator, body_group],
        };

        if prefixes.is_empty() {
            return all_nodes;
        }
        prefixes.push(all_nodes);
        FormatNode::Nodes(prefixes)
    }

    fn format_class_method_header(&mut self, node: &Node) -> FormatNode {
        FormatNode::Nodes(self.format_generic_const(&node.children, Some(FormatNode::Space)))
    }

    fn format_class_method_body(&mut self, node: &Node) -> FormatNode {
        let nodes = self.format_generic_const(&node.children, None);
        FormatNode::Group {
            id: self.new_id(),
            nodes,
        }
    }

    fn format_parameter(&mut self, node: &Node) -> FormatNode {
        if node.children.len() == 1 {
            return self.format(&node.children[0]);
        }
        let sep = self.space_or_line();
        let nodes = self.format_generic_const(&node.children, Some(sep));
        FormatNode::Group {
            id: self.new_id(),
            nodes,
        }
    }

    fn format_parameter_list(&mut self, node: &Node, id: Option<usize>) -> FormatNode {
        if node.children.len() == 2 {
            return FormatNode::Text("()".to_string());
        }
        let group_id = id.unwrap_or_else(|| self.new_id());
        let nodes = self.format_generic(&node.children, |this, prev, next| {
            if this.is_terminal_single(prev, "(") || this.is_terminal_single(next, ")") {
                if this.is_terminal_single(next, ")") {
                    if this.grammar_version == GrammarVersion::V1 {
                        return Some(this.line());
                    }
                    let wrap_node =
                        FormatNode::Nodes(vec![FormatNode::Text(",".to_string()), this.line()]);
                    let no_wrap_node = this.line();
                    return Some(this.if_wrap(group_id, wrap_node, no_wrap_node));
                }
                return Some(this.line());
            }
            Some(this.space_or_line())
        });
        if id.is_some() {
            FormatNode::Nodes(nodes)
        } else {
            FormatNode::Group {
                id: group_id,
                nodes,
            }
        }
    }

    fn format_argument_list(&mut self, node: &Node, two_by_2: bool) -> FormatNode {
        if node.children.len() == 2 {
            return FormatNode::Text("()".to_string());
        }
        let has_trailing_lambda = self.has_trailing_lambda(node);
        let group_id = self.new_id();
        let nodes = self.format_generic_with_gen(
            &node.children,
            |this, prev, next| {
                if this.is_terminal_single(prev, "(") || this.is_terminal_single(next, ")") {
                    let line_node = if has_trailing_lambda {
                        FormatNode::Empty
                    } else {
                        this.line()
                    };
                    if this.is_terminal_single(next, ")") && !has_trailing_lambda {
                        if this.grammar_version == GrammarVersion::V1 {
                            return Some(line_node);
                        }
                        let wrap_node = FormatNode::Nodes(vec![
                            FormatNode::Text(",".to_string()),
                            line_node.clone(),
                        ]);
                        return Some(this.if_wrap(group_id, wrap_node, line_node));
                    }
                    return Some(line_node);
                }
                Some(this.space_or_line())
            },
            |this, n, _| {
                if n.kind == NodeType::ArgumentListElements {
                    this.format_argument_list_elements(n, has_trailing_lambda, two_by_2)
                } else {
                    this.format(n)
                }
            },
        );
        FormatNode::Group {
            id: group_id,
            nodes,
        }
    }

    fn format_argument_list_elements(
        &mut self,
        node: &Node,
        has_trailing_lambda: bool,
        two_by_2: bool,
    ) -> FormatNode {
        let children = &node.children;
        let should_multiline =
            self.should_multiline_nodes(node, |this, n| this.is_terminal_single(n, ","));
        let sep_fn = |this: &Self, _: &Node, _: &Node| {
            Some(if should_multiline {
                this.force_spacey_line()
            } else {
                this.space_or_line()
            })
        };
        if two_by_2 {
            let pairs = self.pair_arguments(children);
            let nodes = self.format_paired_arguments(&pairs, should_multiline);
            return FormatNode::Indent(nodes);
        }
        if has_trailing_lambda {
            let split_index = children
                .iter()
                .rposition(is_same_line_expr)
                .expect("hasTrailingLambda implies same-line expr present");
            let normal_params = &children[..split_index];
            let last_param = &children[split_index..];
            let trailing_node =
                if self.ends_with_closing_curly_brace(&last_param[last_param.len() - 1]) {
                    FormatNode::Empty
                } else {
                    self.line()
                };
            let last_nodes = self.format_generic(last_param, sep_fn);
            if normal_params.is_empty() {
                let last_group = FormatNode::Group {
                    id: self.new_id(),
                    nodes: last_nodes,
                };
                return FormatNode::Group {
                    id: self.new_id(),
                    nodes: vec![last_group, trailing_node],
                };
            }
            let separator = self.get_separator_const(
                &normal_params[normal_params.len() - 1],
                &last_param[0],
                FormatNode::Space,
            );
            let param_nodes = self.format_generic(normal_params, sep_fn);
            let param_group = FormatNode::Group {
                id: self.new_id(),
                nodes: param_nodes,
            };
            let last_group = FormatNode::Group {
                id: self.new_id(),
                nodes: last_nodes,
            };
            return FormatNode::Group {
                id: self.new_id(),
                nodes: vec![param_group, separator, last_group, trailing_node],
            };
        }
        FormatNode::Indent(self.format_generic(children, sep_fn))
    }

    fn should_multiline_nodes<F>(&self, node: &Node, predicate: F) -> bool
    where
        F: Fn(&Self, &Node) -> bool,
    {
        for idx in 0..node.children.len().saturating_sub(1) {
            let prev = &node.children[idx];
            let next = &node.children[idx + 1];
            if (predicate(self, prev) || predicate(self, next)) && lines_between(prev, next) > 0 {
                return true;
            }
        }
        false
    }

    fn ends_with_closing_curly_brace(&self, mut node: &Node) -> bool {
        while let Some(last) = node.children.last() {
            node = last;
        }
        self.is_terminal_single(node, "}")
    }

    fn has_trailing_lambda(&self, arg_list: &Node) -> bool {
        let Some(elements_node) = first_proper_child(arg_list) else {
            return false;
        };
        let children = &elements_node.children;
        if children.is_empty() {
            return false;
        }
        if children[children.len() - 1].kind == NodeType::LineComment {
            return false;
        }
        let proper_arg_count = children.iter().filter(|c| is_proper(c)).count();
        let mut seen_lambda = false;
        for i in (0..children.len()).rev() {
            let child = &children[i];
            if !is_proper(child) {
                continue;
            }
            if !seen_lambda {
                if !is_same_line_expr(child) {
                    return false;
                }
                if i > 0 && children[i - 1].kind == NodeType::LineComment {
                    return false;
                }
                seen_lambda = true;
            } else if is_same_line_expr(child) {
                return false;
            }
        }
        proper_arg_count <= 2
            || !self
                .should_multiline_nodes(elements_node, |this, n| this.is_terminal_single(n, ","))
    }

    fn should_multiline_refs<F>(&self, nodes: &[&Node], predicate: F) -> bool
    where
        F: Fn(&Self, &Node) -> bool,
    {
        for idx in 0..nodes.len().saturating_sub(1) {
            let prev = nodes[idx];
            let next = nodes[idx + 1];
            if (predicate(self, prev) || predicate(self, next)) && lines_between(prev, next) > 0 {
                return true;
            }
        }
        false
    }

    fn pair_arguments<'b>(&self, nodes: &'b [Node]) -> Vec<PairedArg<'b>> {
        let mut res = Vec::new();
        let mut tmp: Vec<&'b Node> = Vec::new();
        let mut commas = 0usize;
        for node in nodes {
            if self.is_terminal_single(node, ",") {
                commas += 1;
                if commas == 2 {
                    let mut suffixes = Vec::new();
                    while tmp.last().is_some_and(|n| n.kind.is_affix()) {
                        suffixes.push(tmp.pop().expect("non-empty"));
                    }
                    res.push(PairedArg::from_group(std::mem::take(&mut tmp)));
                    for suffix in suffixes.into_iter().rev() {
                        res.push(PairedArg::Single(suffix));
                    }
                    res.push(PairedArg::Single(node));
                    commas = 0;
                } else {
                    tmp.push(node);
                }
            } else if tmp.is_empty() && node.kind.is_affix() {
                res.push(PairedArg::Single(node));
            } else {
                tmp.push(node);
            }
        }
        if !tmp.is_empty() {
            res.push(PairedArg::from_group(tmp));
        }
        res
    }

    fn format_paired_arguments(
        &mut self,
        pairs: &[PairedArg<'_>],
        should_multiline: bool,
    ) -> Vec<FormatNode> {
        let filtered: Vec<&PairedArg<'_>> = pairs
            .iter()
            .filter(|pair| match pair {
                PairedArg::Single(node) => !self.is_semicolon(node),
                PairedArg::Group { .. } => true,
            })
            .collect();
        if filtered.is_empty() {
            return Vec::new();
        }
        if filtered.len() == 1 {
            return vec![match filtered[0] {
                PairedArg::Single(node) => self.format(node),
                PairedArg::Group { children, .. } => {
                    let inner_multiline = self
                        .should_multiline_refs(children, |this, n| this.is_terminal_single(n, ","));
                    let inner_sep = |this: &Self, _: &Node, _: &Node| {
                        Some(if inner_multiline {
                            this.force_spacey_line()
                        } else {
                            this.space_or_line()
                        })
                    };
                    FormatNode::Indent(self.format_generic_refs(children, inner_sep))
                }
            }];
        }
        let format_pair = |this: &mut Self, item: &PairedArg<'_>| match item {
            PairedArg::Group { children, .. } => {
                let s = this.space_or_line();
                let inner = this.format_generic_refs(children, |_, _, _| Some(s.clone()));
                FormatNode::Group {
                    id: this.new_id(),
                    nodes: inner,
                }
            }
            PairedArg::Single(node) => this.format(node),
        };
        let mut nodes = Vec::new();
        let mut prev = filtered[0];
        for &child in &filtered[1..] {
            nodes.push(format_pair(self, prev));
            let sep = self
                .get_base_separator_paired(prev, child)
                .unwrap_or_else(|| {
                    if should_multiline {
                        self.force_spacey_line()
                    } else {
                        self.space_or_line()
                    }
                });
            nodes.push(sep);
            prev = child;
        }
        nodes.push(format_pair(self, filtered[filtered.len() - 1]));
        nodes
    }

    fn get_base_separator_paired(
        &self,
        prev: &PairedArg<'_>,
        next: &PairedArg<'_>,
    ) -> Option<FormatNode> {
        let prev_span = prev.span();
        let next_span = next.span();
        let line_diff = next_span.line_begin as isize - prev_span.line_end as isize;
        if prev.ends_in_line_comment() {
            return Some(if line_diff > 1 {
                two_newlines()
            } else {
                self.must_force_line()
            });
        }
        if next.has_leading_affix_on_line(prev_span.line_end) {
            return Some(FormatNode::Space);
        }
        let prev_kind = prev.kind();
        let next_kind = next.kind();
        if prev_kind == NodeType::DocComment {
            return Some(self.must_force_line());
        }
        if prev_kind == NodeType::Annotation {
            return Some(self.force_line());
        }
        if is_force_line_affix(prev_kind) || next_kind.is_affix() {
            return Some(if line_diff > 1 {
                two_newlines()
            } else {
                self.must_force_line()
            });
        }
        if prev_kind == NodeType::BlockComment {
            return Some(if line_diff > 0 {
                self.force_spacey_line()
            } else {
                FormatNode::Space
            });
        }
        if is_empty_suffix(next_kind)
            || prev.is_terminal(self, &["[", "!", "@", "[["])
            || next.is_terminal(self, &["]", "?", ","])
        {
            return Some(FormatNode::Empty);
        }
        if prev.is_terminal(self, &["class", "function", "new"])
            || next.is_terminal(self, &["=", "{", "->", "class", "function"])
            || next_kind == NodeType::ObjectBody
            || prev_kind == NodeType::ModifierList
        {
            return Some(FormatNode::Space);
        }
        if next_kind == NodeType::DocComment {
            return Some(two_newlines());
        }
        None
    }

    fn format_parameter_list_elements(&mut self, node: &Node) -> FormatNode {
        let sep = self.space_or_line();
        FormatNode::Indent(self.format_generic_const(&node.children, Some(sep)))
    }

    fn format_type_parameter_list(&mut self, node: &Node) -> FormatNode {
        if node.children.len() == 2 {
            return FormatNode::Text("<>".to_string());
        }
        let group_id = self.new_id();
        let nodes = self.format_generic(&node.children, |this, prev, next| {
            if this.is_terminal_single(prev, "<") || this.is_terminal_single(next, ">") {
                if this.is_terminal_single(next, ">") {
                    if this.grammar_version == GrammarVersion::V1 {
                        return Some(FormatNode::Line);
                    }
                    let wrap_node =
                        FormatNode::Nodes(vec![FormatNode::Text(",".to_string()), this.line()]);
                    let no_wrap_node = this.line();
                    return Some(this.if_wrap(group_id, wrap_node, no_wrap_node));
                }
                return Some(this.line());
            }
            Some(this.space_or_line())
        });
        FormatNode::Group {
            id: group_id,
            nodes,
        }
    }

    fn format_object_parameter_list(&mut self, node: &Node) -> FormatNode {
        let group_id = self.new_id();
        let sep = self.space_or_line();
        let body = self.format_generic_const(&node.children, Some(sep));
        let wrapped_node = if self.no_newlines {
            let mut flat = Vec::with_capacity(body.len() + 1);
            flat.push(FormatNode::Space);
            flat.extend(body);
            FormatNode::Nodes(flat)
        } else {
            FormatNode::IfWrapPrefix {
                id: group_id,
                wrap_indent: 2,
                wrap_prefix: Box::new(self.line()),
                no_wrap_prefix: Box::new(FormatNode::Space),
                body,
            }
        };
        FormatNode::Group {
            id: group_id,
            nodes: vec![wrapped_node],
        }
    }

    fn format_object_body(&mut self, node: &Node) -> FormatNode {
        if node.children.len() == 2 {
            return FormatNode::Text("{}".to_string());
        }
        let group_id = self.new_id();
        let nodes = self.format_generic_with_gen(
            &node.children,
            |this, prev, next| {
                if next.kind == NodeType::ObjectParameterList {
                    return Some(FormatNode::Empty);
                }
                if this.is_terminal_single(prev, "{") || this.is_terminal_single(next, "}") {
                    let lines = lines_between(prev, next);
                    return Some(if lines == 0 {
                        this.space_or_line()
                    } else {
                        this.force_spacey_line()
                    });
                }
                Some(this.space_or_line())
            },
            |this, n, _| {
                if n.kind == NodeType::ObjectMemberList {
                    this.format_object_member_list(n, group_id)
                } else {
                    this.format(n)
                }
            },
        );
        FormatNode::Group {
            id: group_id,
            nodes,
        }
    }

    fn format_object_member_list(&mut self, node: &Node, group_id: usize) -> FormatNode {
        let nodes = self.format_generic(&node.children, |this, prev, next| {
            let lines = lines_between(prev, next);
            if lines == 0 {
                return Some(this.if_wrap(
                    group_id,
                    this.line(),
                    FormatNode::Text("; ".to_string()),
                ));
            }
            if lines == 1 {
                return Some(this.force_line());
            }
            Some(two_newlines())
        });
        FormatNode::Indent(nodes)
    }

    fn format_object_entry_header(&mut self, node: &Node) -> FormatNode {
        let sep = self.space_or_line();
        let nodes = self.format_generic_const(&node.children, Some(sep));
        FormatNode::Group {
            id: self.new_id(),
            nodes,
        }
    }

    fn format_for_generator(&mut self, node: &Node) -> FormatNode {
        let nodes = self.format_generic(&node.children, |this, prev, next| {
            Some(
                if prev.kind == NodeType::ForGeneratorHeader
                    || next.kind == NodeType::ForGeneratorHeader
                {
                    FormatNode::Space
                } else {
                    this.space_or_line()
                },
            )
        });
        FormatNode::Group {
            id: self.new_id(),
            nodes,
        }
    }

    fn format_for_generator_header(&mut self, node: &Node) -> FormatNode {
        let nodes = self.format_generic(&node.children, |this, prev, next| {
            if this.is_terminal_single(prev, "(") || this.is_terminal_single(next, ")") {
                Some(this.line())
            } else {
                None
            }
        });
        FormatNode::Group {
            id: self.new_id(),
            nodes,
        }
    }

    fn format_for_generator_header_definition(&mut self, node: &Node) -> FormatNode {
        let nodes = self.format_generic_with_gen(
            &node.children,
            |this, _, next| {
                Some(if is_same_line_expr(next) {
                    FormatNode::Space
                } else {
                    this.space_or_line()
                })
            },
            |this, n, _| {
                if n.kind.is_expression() && !is_same_line_expr(n) {
                    indent_one(this.format(n))
                } else {
                    this.format(n)
                }
            },
        );
        indent_one(FormatNode::Group {
            id: self.new_id(),
            nodes,
        })
    }

    fn format_for_generator_header_definition_header(&mut self, node: &Node) -> FormatNode {
        let sep = self.space_or_line();
        let nodes = self.format_generic_const(&node.children, Some(sep));
        FormatNode::Group {
            id: self.new_id(),
            nodes,
        }
    }

    fn format_when_generator(&mut self, node: &Node) -> FormatNode {
        let nodes = self.format_generic(&node.children, |this, prev, next| {
            Some(
                if prev.kind == NodeType::WhenGeneratorHeader
                    || this.is_terminal(prev, &["when", "else"])
                    || this.is_terminal_single(next, "else")
                {
                    FormatNode::Space
                } else {
                    this.space_or_line()
                },
            )
        });
        FormatNode::Group {
            id: self.new_id(),
            nodes,
        }
    }

    fn format_when_generator_header(&mut self, node: &Node) -> FormatNode {
        let nodes = self.format_generic_with_gen(
            &node.children,
            |this, prev, next| {
                Some(
                    if this.is_terminal_single(prev, "(") || this.is_terminal_single(next, ")") {
                        this.line()
                    } else {
                        this.space_or_line()
                    },
                )
            },
            |this, n, _| {
                if !n.kind.is_affix() && n.kind != NodeType::Terminal {
                    indent_one(this.format(n))
                } else {
                    this.format(n)
                }
            },
        );
        FormatNode::Group {
            id: self.new_id(),
            nodes,
        }
    }

    fn format_member_predicate(&mut self, node: &Node) -> FormatNode {
        let sep = self.space_or_line();
        let nodes = self.format_generic_with_gen(
            &node.children,
            |_, _, _| Some(sep.clone()),
            |this, n, next| {
                if next.is_none() && n.kind != NodeType::ObjectBody {
                    indent_one(this.format(n))
                } else {
                    this.format(n)
                }
            },
        );
        FormatNode::Group {
            id: self.new_id(),
            nodes,
        }
    }

    fn format_string_parts(&mut self, nodes: &[Node]) -> Vec<FormatNode> {
        let mut result = Vec::new();
        let mut is_in_interpolation = false;
        let mut i = 0;
        let mut prev: Option<&Node> = None;
        while i < nodes.len() {
            if is_in_interpolation {
                let prev_no_newlines = self.no_newlines;
                let start = i;
                while i < nodes.len() && !self.is_terminal_single(&nodes[i], ")") {
                    i += 1;
                }
                let elems = &nodes[start..i];
                let prev_node = prev.expect("prev must exist before interpolation");
                let spans_newline = is_multiline_list(elems)
                    || (i < nodes.len() && prev_node.span.line_begin < nodes[i].span.line_end)
                    || elems.iter().any(ends_in_line_comment);
                self.no_newlines = !spans_newline;
                if !elems.is_empty() {
                    if let Some(base_sep) = self.get_base_separator(prev_node, &elems[0]) {
                        result.push(base_sep);
                    }
                    result.extend(self.format_generic_const(elems, None));
                    if i < nodes.len()
                        && let Some(end_sep) =
                            self.get_base_separator(&elems[elems.len() - 1], &nodes[i])
                    {
                        result.push(end_sep);
                    }
                }
                self.no_newlines = prev_no_newlines;
                is_in_interpolation = false;
                continue;
            }
            let elem = &nodes[i];
            i += 1;
            if elem.kind == NodeType::Terminal && self.text(elem).ends_with('(') {
                is_in_interpolation = true;
            }
            let mut formatted = self.format(elem);
            match &mut formatted {
                FormatNode::Nodes(inner) => result.extend(std::mem::take(inner)),
                _ => result.push(formatted),
            }
            prev = Some(elem);
        }
        result
    }

    fn format_single_line_string(&mut self, node: &Node) -> FormatNode {
        let nodes = self.format_string_parts(&node.children);
        FormatNode::Group {
            id: self.new_id(),
            nodes,
        }
    }

    fn format_multiline_string(&mut self, node: &Node) -> FormatNode {
        let nodes = self.format_string_parts(&node.children);
        let end_quote_col = node.children[node.children.len() - 1].span.col_begin;
        FormatNode::MultilineStringGroup {
            end_quote_col,
            nodes,
        }
    }

    fn format_if(&mut self, node: &Node) -> FormatNode {
        let separator = if is_multiline(node) {
            self.force_spacey_line()
        } else {
            self.space_or_line()
        };
        let nodes = self.format_generic(&node.children, |_, _, next| {
            if next.kind == NodeType::IfElseExpr && next.children[0].kind == NodeType::IfExpr {
                Some(FormatNode::Space)
            } else {
                Some(separator.clone())
            }
        });
        FormatNode::Group {
            id: self.new_id(),
            nodes,
        }
    }

    fn format_if_header(&mut self, node: &Node) -> FormatNode {
        let nodes = self.format_generic(&node.children, |this, _, next| {
            Some(if next.kind == NodeType::IfCondition {
                FormatNode::Space
            } else {
                this.space_or_line()
            })
        });
        FormatNode::Group {
            id: self.new_id(),
            nodes,
        }
    }

    fn format_if_condition(&mut self, node: &Node) -> FormatNode {
        let nodes = self.format_generic(&node.children, |this, prev, next| {
            Some(
                if this.is_terminal_single(prev, "(") || this.is_terminal_single(next, ")") {
                    this.line()
                } else {
                    this.space_or_line()
                },
            )
        });
        FormatNode::Group {
            id: self.new_id(),
            nodes,
        }
    }

    fn format_if_then(&mut self, node: &Node) -> FormatNode {
        FormatNode::Indent(self.format_generic_const(&node.children, None))
    }

    fn format_if_else(&mut self, node: &Node) -> FormatNode {
        if node.children.len() == 1 {
            let expr = &node.children[0];
            if expr.kind == NodeType::IfExpr {
                if let FormatNode::Group { nodes, .. } = &mut self.format_if(expr) {
                    return FormatNode::Nodes(std::mem::take(nodes));
                }
                unreachable!("format_if always returns Group");
            }
            return indent_one(self.format(expr));
        }
        FormatNode::Indent(self.format_generic_const(&node.children, None))
    }

    /// Iteratively formats arbitrary interleaved left-leaning CST chains
    /// (`NonNullExpr`, `NullableType`, `SubscriptExpr`, `SuperSubscriptExpr`,
    /// `AmendsExpr`, `QualifiedAccessExpr`, `BinaryOpExpr`, `ConstrainedType`,
    /// `UnionType`) in O(1) call-stack depth so deep interleaved chains
    /// (e.g. `a.b!!.c[0]!!...`, `Int?(true)?(false)...`, `a + b * c...`) cannot
    /// overflow the native stack.
    fn format_left_spine(&mut self, root: &Node) -> FormatNode {
        enum LeftFrame<'b> {
            Postfix(&'b Node),
            Amends(&'b Node),
            Constrained(&'b Node),
            Union(&'b Node),
            Qualified {
                node: &'b Node,
                flat: Vec<&'b Node>,
                lambda_count: usize,
                method_call_count: usize,
                index_before_first_method_call: usize,
            },
            Binary {
                node: &'b Node,
                flat: Vec<&'b Node>,
                should_multiline: bool,
            },
        }

        let mut spine: Vec<LeftFrame<'_>> = Vec::new();
        let mut cur = root;
        loop {
            match cur.kind {
                NodeType::NonNullExpr
                | NodeType::NullableType
                | NodeType::SubscriptExpr
                | NodeType::SuperSubscriptExpr => {
                    if let Some(first) = cur.children.iter().find(|c| !self.is_semicolon(c)) {
                        spine.push(LeftFrame::Postfix(cur));
                        cur = first;
                    } else {
                        break;
                    }
                }
                NodeType::AmendsExpr => {
                    if let Some(first) = cur.children.iter().find(|c| !self.is_semicolon(c)) {
                        spine.push(LeftFrame::Amends(cur));
                        cur = first;
                    } else {
                        break;
                    }
                }
                NodeType::ConstrainedType => {
                    if let Some(first) = cur.children.iter().find(|c| !self.is_semicolon(c)) {
                        spine.push(LeftFrame::Constrained(cur));
                        cur = first;
                    } else {
                        break;
                    }
                }
                NodeType::UnionType => {
                    if let Some(first) = cur.children.iter().find(|c| !self.is_semicolon(c)) {
                        spine.push(LeftFrame::Union(cur));
                        cur = first;
                    } else {
                        break;
                    }
                }
                NodeType::QualifiedAccessExpr => {
                    let mut lambda_count = 0usize;
                    let mut method_call_count = 0usize;
                    let mut index_before_first_method_call = 0usize;
                    let mut flat: Vec<&Node> = Vec::new();
                    gather_facts(
                        cur,
                        &mut flat,
                        &mut lambda_count,
                        &mut method_call_count,
                        &mut index_before_first_method_call,
                    );
                    if let Some(&first) = flat.iter().find(|c| !self.is_semicolon(c)) {
                        spine.push(LeftFrame::Qualified {
                            node: cur,
                            flat,
                            lambda_count,
                            method_call_count,
                            index_before_first_method_call,
                        });
                        cur = first;
                    } else {
                        break;
                    }
                }
                NodeType::BinaryOpExpr => {
                    let flat = self.flatten_binary_operator_exprs(cur);
                    let should_multiline =
                        self.should_multiline_nodes(cur, |_, n| n.kind == NodeType::Operator);
                    if let Some(&first) = flat.iter().find(|c| !self.is_semicolon(c)) {
                        spine.push(LeftFrame::Binary {
                            node: cur,
                            flat,
                            should_multiline,
                        });
                        cur = first;
                    } else {
                        break;
                    }
                }
                _ => break,
            }
        }

        let mut acc = match cur.kind {
            NodeType::NonNullExpr
            | NodeType::NullableType
            | NodeType::SubscriptExpr
            | NodeType::SuperSubscriptExpr => FormatNode::Nodes(Vec::new()),
            NodeType::AmendsExpr
            | NodeType::ConstrainedType
            | NodeType::UnionType
            | NodeType::QualifiedAccessExpr
            | NodeType::BinaryOpExpr => FormatNode::Group {
                id: self.new_id(),
                nodes: Vec::new(),
            },
            _ => self.format(cur),
        };

        while let Some(frame) = spine.pop() {
            acc = match frame {
                LeftFrame::Postfix(parent) => {
                    let refs: Vec<&Node> = parent.children.iter().collect();
                    FormatNode::Nodes(
                        self.format_generic_refs_with_first(&refs, acc, |_, _, _| None),
                    )
                }
                LeftFrame::Amends(parent) => {
                    let sep = self.space_or_line();
                    let refs: Vec<&Node> = parent.children.iter().collect();
                    let nodes = self
                        .format_generic_refs_with_first(&refs, acc, |_, _, _| Some(sep.clone()));
                    FormatNode::Group {
                        id: self.new_id(),
                        nodes,
                    }
                }
                LeftFrame::Constrained(parent) => {
                    let refs: Vec<&Node> = parent.children.iter().collect();
                    let nodes = self.format_generic_refs_with_first(&refs, acc, |this, _, next| {
                        if next.kind == NodeType::ConstrainedTypeConstraint {
                            None
                        } else {
                            Some(this.space_or_line())
                        }
                    });
                    FormatNode::Group {
                        id: self.new_id(),
                        nodes,
                    }
                }
                LeftFrame::Union(parent) => {
                    let refs: Vec<&Node> = parent.children.iter().collect();
                    let nodes =
                        self.format_generic_refs_with_first(&refs, acc, |this, prev, next| {
                            if this.is_terminal_single(next, "|") {
                                Some(this.space_or_line())
                            } else if this.is_terminal_single(prev, "|") {
                                Some(FormatNode::Space)
                            } else {
                                None
                            }
                        });
                    let indented = self.indent_after_first_newline(nodes, false);
                    FormatNode::Group {
                        id: self.new_id(),
                        nodes: indented,
                    }
                }
                LeftFrame::Qualified {
                    node,
                    flat,
                    lambda_count,
                    method_call_count,
                    index_before_first_method_call,
                } => {
                    let leading_sep = |this: &Self, prev: &Node, next: &Node| {
                        if prev.kind == NodeType::Operator {
                            None
                        } else if next.kind == NodeType::Operator {
                            Some(this.line())
                        } else {
                            Some(this.space_or_line())
                        }
                    };
                    let trailing_sep = |this: &Self, prev: &Node, next: &Node| {
                        if prev.kind == NodeType::Operator {
                            None
                        } else if next.kind == NodeType::Operator {
                            Some(if lambda_count > 1 {
                                this.force_line()
                            } else {
                                this.line()
                            })
                        } else {
                            Some(this.space_or_line())
                        }
                    };
                    let nodes =
                        if method_call_count == 1 && is_method_call(last_proper_node_refs(&flat)) {
                            let (call_chain, args_list) = split_function_call_node(&flat);
                            let leading_formatted =
                                self.format_generic_refs_with_first(&call_chain, acc, leading_sep);
                            let leading_nodes =
                                self.indent_after_first_newline(leading_formatted, true);
                            let trailing_nodes = self.format_generic_refs(&args_list, trailing_sep);
                            let sep = self
                                .get_base_separator(call_chain[call_chain.len() - 1], args_list[0]);
                            if let Some(s) = sep {
                                concat3(leading_nodes, s, trailing_nodes)
                            } else {
                                concat2(leading_nodes, trailing_nodes)
                            }
                        } else if method_call_count > 0 && index_before_first_method_call > 0 {
                            let leading = &flat[..index_before_first_method_call];
                            let trailing = &flat[index_before_first_method_call..];
                            let leading_formatted =
                                self.format_generic_refs_with_first(leading, acc, leading_sep);
                            let leading_nodes =
                                self.indent_after_first_newline(leading_formatted, true);
                            let trailing_nodes = self.format_generic_refs(trailing, trailing_sep);
                            concat3(leading_nodes, self.line(), trailing_nodes)
                        } else {
                            self.format_generic_refs_with_first(&flat, acc, trailing_sep)
                        };
                    let should_group = node.children.len() == flat.len();
                    let indented = self.indent_after_first_newline(nodes, should_group);
                    FormatNode::Group {
                        id: self.new_id(),
                        nodes: indented,
                    }
                }
                LeftFrame::Binary {
                    node,
                    flat,
                    should_multiline,
                } => {
                    let nodes =
                        self.format_generic_refs_with_first(&flat, acc, |this, prev, next| {
                            let sep = if should_multiline {
                                this.force_spacey_line()
                            } else {
                                this.space_or_line()
                            };
                            if prev.kind == NodeType::Operator {
                                return Some(if this.text(prev) == "-" {
                                    sep
                                } else {
                                    FormatNode::Space
                                });
                            }
                            if next.kind == NodeType::Operator {
                                return Some(if this.text(next) == "-" {
                                    FormatNode::Space
                                } else {
                                    sep
                                });
                            }
                            Some(sep)
                        });
                    let should_group = node.children.len() == flat.len();
                    let indented = self.indent_after_first_newline(nodes, should_group);
                    FormatNode::Group {
                        id: self.new_id(),
                        nodes: indented,
                    }
                }
            };
        }
        acc
    }

    fn format_generic_refs_with_first<S>(
        &mut self,
        children: &[&Node],
        first_node: FormatNode,
        separator_fn: S,
    ) -> Vec<FormatNode>
    where
        S: Fn(&Self, &Node, &Node) -> Option<FormatNode>,
    {
        let filtered: Vec<&Node> = children
            .iter()
            .copied()
            .filter(|child| !self.is_semicolon(child))
            .collect();
        if filtered.is_empty() {
            return Vec::new();
        }
        if filtered.len() == 1 {
            return vec![first_node];
        }
        let mut nodes = Vec::with_capacity(filtered.len() * 2);
        let mut prev = filtered[0];
        nodes.push(first_node);
        for &child in &filtered[1..] {
            if let Some(separator) = self.get_separator(prev, child, &separator_fn) {
                nodes.push(separator);
            }
            nodes.push(self.format(child));
            prev = child;
        }
        nodes
    }

    /// Iteratively formats right-leaning chains of `UnaryMinusExpr` / `LogicalNotExpr`.
    fn format_unary_chain(&mut self, node: &Node) -> FormatNode {
        fn is_unary_kind(kind: NodeType) -> bool {
            matches!(kind, NodeType::UnaryMinusExpr | NodeType::LogicalNotExpr)
        }
        let mut spine: Vec<&Node> = Vec::new();
        let mut cur = node;
        loop {
            spine.push(cur);
            if let Some(last) = cur.children.iter().rev().find(|c| !self.is_semicolon(c))
                && is_unary_kind(last.kind)
            {
                cur = last;
            } else {
                break;
            }
        }
        let base = spine.pop().expect("non-empty spine");
        let mut acc = FormatNode::Nodes(self.format_generic_const(&base.children, None));
        while let Some(parent) = spine.pop() {
            let filtered: Vec<&Node> = parent
                .children
                .iter()
                .filter(|c| !self.is_semicolon(c))
                .collect();
            if filtered.len() <= 1 {
                acc = FormatNode::Nodes(vec![acc]);
                continue;
            }
            let last_idx = filtered.len() - 1;
            let mut nodes = Vec::with_capacity(filtered.len() * 2);
            let mut prev = filtered[0];
            for &child in &filtered[1..last_idx] {
                nodes.push(self.format(prev));
                if let Some(sep) = self.get_base_separator(prev, child) {
                    nodes.push(sep);
                }
                prev = child;
            }
            nodes.push(self.format(prev));
            if let Some(sep) = self.get_base_separator(prev, filtered[last_idx]) {
                nodes.push(sep);
            }
            nodes.push(acc);
            acc = FormatNode::Nodes(nodes);
        }
        acc
    }

    fn format_new_expr(&mut self, node: &Node) -> FormatNode {
        let sep = self.space_or_line();
        let nodes = self.format_generic_const(&node.children, Some(sep));
        FormatNode::Group {
            id: self.new_id(),
            nodes,
        }
    }

    fn format_new_header(&mut self, node: &Node) -> FormatNode {
        let sep = self.space_or_line();
        let nodes = self.format_generic_const(&node.children, Some(sep));
        FormatNode::Group {
            id: self.new_id(),
            nodes,
        }
    }

    fn format_parenthesized_expr(&mut self, node: &Node) -> FormatNode {
        if node.children.len() == 2 {
            return FormatNode::Text("()".to_string());
        }
        let nodes = self.format_generic_with_gen(
            &node.children,
            |this, prev, next| {
                Some(
                    if this.is_terminal_single(prev, "(") || this.is_terminal_single(next, ")") {
                        this.line()
                    } else {
                        this.space_or_line()
                    },
                )
            },
            |this, n, _| {
                if n.kind.is_expression() {
                    indent_one(this.format(n))
                } else {
                    this.format(n)
                }
            },
        );
        FormatNode::Group {
            id: self.new_id(),
            nodes,
        }
    }

    fn format_parenthesized_expr_elements(&mut self, node: &Node) -> FormatNode {
        let nodes = self.format_generic_const(&node.children, None);
        indent_one(FormatNode::Group {
            id: self.new_id(),
            nodes,
        })
    }

    fn format_function_literal_expr(&mut self, node: &Node) -> FormatNode {
        let arrow_idx = node
            .children
            .iter()
            .position(|n| self.is_terminal_single(n, "->"))
            .unwrap_or(node.children.len());
        let params = &node.children[..arrow_idx];
        let rest = &node.children[arrow_idx..];
        let body_node = node
            .children
            .iter()
            .find(|c| c.kind == NodeType::FunctionLiteralBody)
            .expect("FunctionLiteralExpr must contain FunctionLiteralBody");
        let expr_node = body_node
            .children
            .iter()
            .find(|c| c.kind.is_expression())
            .expect("FunctionLiteralBody must contain expression");
        let same_line = is_same_line_expr(expr_node);
        let sep = if same_line {
            FormatNode::Space
        } else {
            self.space_or_line()
        };
        let body_sep = self.get_separator_const(&params[params.len() - 1], &rest[0], sep.clone());

        let mut result = self.format_generic_const(params, Some(sep.clone()));
        let mut rest_nodes = Vec::new();
        rest_nodes.push(body_sep);
        rest_nodes.extend(self.format_generic_const(rest, Some(sep)));
        result.push(FormatNode::Group {
            id: self.new_id(),
            nodes: rest_nodes,
        });
        FormatNode::Group {
            id: self.new_id(),
            nodes: result,
        }
    }

    fn format_function_literal_body(&mut self, node: &Node) -> FormatNode {
        let expr = node
            .children
            .iter()
            .find(|c| c.kind.is_expression())
            .expect("FunctionLiteralBody must contain expression");
        let nodes = self.format_generic_const(&node.children, None);
        if is_same_line_expr(expr) {
            FormatNode::Group {
                id: self.new_id(),
                nodes,
            }
        } else {
            FormatNode::Indent(nodes)
        }
    }

    fn format_let_expr(&mut self, node: &Node) -> FormatNode {
        let separator = if is_multiline(node) {
            self.force_spacey_line()
        } else {
            self.space_or_line()
        };
        let ends_with_let = node.children[node.children.len() - 1].kind == NodeType::LetExpr;
        let nodes = self.format_generic_with_gen(
            &node.children,
            |_, _, next| {
                Some(if next.kind == NodeType::LetParameterDefinition {
                    FormatNode::Space
                } else {
                    separator.clone()
                })
            },
            |this, n, _| {
                if n.kind == NodeType::LetExpr {
                    if let FormatNode::Group { nodes, .. } = &mut this.format_let_expr(n) {
                        return FormatNode::Nodes(std::mem::take(nodes));
                    }
                    unreachable!("format_let_expr always returns Group");
                }
                if ends_with_let {
                    return this.format(n);
                }
                if n.kind.is_expression() || n.kind.is_affix() {
                    indent_one(this.format(n))
                } else {
                    this.format(n)
                }
            },
        );
        FormatNode::Group {
            id: self.new_id(),
            nodes,
        }
    }

    fn format_let_parameter_definition(&mut self, node: &Node) -> FormatNode {
        let nodes = self.format_generic(&node.children, |this, prev, next| {
            Some(
                if this.is_terminal_single(prev, "(") || this.is_terminal_single(next, ")") {
                    this.line()
                } else {
                    this.space_or_line()
                },
            )
        });
        FormatNode::Group {
            id: self.new_id(),
            nodes,
        }
    }

    fn format_let_parameter(&mut self, node: &Node) -> FormatNode {
        indent_one(self.format_class_property(node))
    }

    fn format_trace_throw_read_expr(&mut self, node: &Node) -> FormatNode {
        let nodes = self.format_generic_with_gen(
            &node.children,
            |this, prev, next| {
                if this.is_terminal_single(prev, "(") || this.is_terminal_single(next, ")") {
                    Some(this.line())
                } else {
                    None
                }
            },
            |this, n, _| {
                if n.kind.is_expression() {
                    indent_one(this.format(n))
                } else {
                    this.format(n)
                }
            },
        );
        FormatNode::Group {
            id: self.new_id(),
            nodes,
        }
    }

    fn format_declared_type(&mut self, node: &Node) -> FormatNode {
        let sep = self.space_or_line();
        FormatNode::Nodes(self.format_generic_const(&node.children, Some(sep)))
    }

    fn format_function_type(&mut self, node: &Node) -> FormatNode {
        let nodes = self.format_generic_with_gen(
            &node.children,
            |this, prev, next| {
                Some(
                    if this.is_terminal_single(prev, "(") || this.is_terminal_single(next, ")") {
                        this.line()
                    } else {
                        this.space_or_line()
                    },
                )
            },
            |this, n, next| {
                if next.is_none() {
                    indent_one(this.format(n))
                } else {
                    this.format(n)
                }
            },
        );
        FormatNode::Group {
            id: self.new_id(),
            nodes,
        }
    }

    fn format_parenthesized_type(&mut self, node: &Node) -> FormatNode {
        if node.children.len() == 2 {
            return FormatNode::Text("()".to_string());
        }
        let group_id = self.new_id();
        let nodes = self.format_generic(&node.children, |this, prev, next| {
            Some(
                if this.is_terminal_single(prev, "(") || this.is_terminal_single(next, ")") {
                    this.line()
                } else {
                    this.space_or_line()
                },
            )
        });
        FormatNode::Group {
            id: group_id,
            nodes,
        }
    }

    fn format_parenthesized_type_elements(&mut self, node: &Node) -> FormatNode {
        let sep = self.space_or_line();
        let nodes = self.format_generic_const(&node.children, Some(sep));
        indent_one(FormatNode::Group {
            id: self.new_id(),
            nodes,
        })
    }

    fn format_type_annotation(&mut self, node: &Node) -> FormatNode {
        let nodes = self.format_generic_const(&node.children, Some(FormatNode::Space));
        FormatNode::Group {
            id: self.new_id(),
            nodes,
        }
    }

    fn format_modifier_list(&mut self, node: &Node) -> FormatNode {
        let mut result = Vec::new();
        let mut affixes: Vec<&Node> = Vec::new();
        let mut modifiers: Vec<&Node> = Vec::new();
        for child in &node.children {
            if child.kind.is_affix() {
                affixes.push(child);
            } else {
                modifiers.push(child);
            }
        }
        if !affixes.is_empty() {
            let sep = self.space_or_line();
            result.extend(self.format_generic_refs(&affixes, |_, _, _| Some(sep.clone())));
        }
        modifiers.sort_by_key(|m| self.modifier_precedence(m));
        result.extend(self.format_generic_refs(&modifiers, |_, _, _| Some(FormatNode::Space)));
        FormatNode::Nodes(result)
    }

    fn build_imports_with_comments<'b>(children: &'b [Node]) -> Vec<ImportWithComments<'b>> {
        let mut result = Vec::new();
        let mut last_import: Option<&'b Node> = None;
        let mut last_trailing: Vec<&'b Node> = Vec::new();
        let mut last_leading: Vec<&'b Node> = Vec::new();
        let mut pending_affixes: Vec<&'b Node> = Vec::new();

        for child in children {
            if child.kind.is_affix() {
                if let Some(imp) = last_import
                    && imp.span.line_end == child.span.line_begin
                {
                    last_trailing.push(child);
                } else {
                    if let Some(imp) = last_import.take() {
                        result.push(ImportWithComments {
                            leading_affixes: std::mem::take(&mut last_leading),
                            import_node: imp,
                            trailing_affixes: std::mem::take(&mut last_trailing),
                        });
                    }
                    pending_affixes.push(child);
                }
            } else {
                if let Some(imp) = last_import.take() {
                    result.push(ImportWithComments {
                        leading_affixes: std::mem::take(&mut last_leading),
                        import_node: imp,
                        trailing_affixes: std::mem::take(&mut last_trailing),
                    });
                }
                last_leading = std::mem::take(&mut pending_affixes);
                last_import = Some(child);
            }
        }
        if let Some(imp) = last_import {
            result.push(ImportWithComments {
                leading_affixes: last_leading,
                import_node: imp,
                trailing_affixes: last_trailing,
            });
        }
        result
    }

    fn format_import_list(&mut self, node: &Node) -> FormatNode {
        let mut nodes = Vec::new();
        let all_imports = Self::build_imports_with_comments(&node.children);
        let mut regular_imports = Vec::new();
        let mut glob_imports = Vec::new();
        for entry in all_imports {
            let is_glob = entry
                .import_node
                .find_child_by_type(NodeType::Terminal)
                .is_some_and(|t| self.text(t) == "import*");
            if is_glob {
                glob_imports.push(entry);
            } else {
                regular_imports.push(entry);
            }
        }
        if !regular_imports.is_empty() {
            self.format_import_list_helper(regular_imports, &mut nodes);
            if !glob_imports.is_empty() {
                nodes.push(two_newlines());
            }
        }
        if !glob_imports.is_empty() {
            self.format_import_list_helper(glob_imports, &mut nodes);
        }
        FormatNode::Nodes(nodes)
    }

    fn format_import_with_comments(
        &mut self,
        entry: &ImportWithComments<'_>,
        nodes: &mut Vec<FormatNode>,
    ) {
        if !entry.leading_affixes.is_empty() {
            let sep = self.space_or_line();
            let leading =
                self.format_generic_refs(&entry.leading_affixes, |_, _, _| Some(sep.clone()));
            if !leading.is_empty() {
                nodes.extend(leading);
                nodes.push(self.force_line());
            }
        }
        nodes.push(self.format(entry.import_node));
        for affix in &entry.trailing_affixes {
            if self.is_semicolon(affix) {
                continue;
            }
            nodes.push(FormatNode::Space);
            nodes.push(self.format(affix));
        }
    }

    fn format_import_list_helper(
        &mut self,
        all_imports: Vec<ImportWithComments<'_>>,
        nodes: &mut Vec<FormatNode>,
    ) {
        let mut absolute = Vec::new();
        let mut projects = Vec::new();
        let mut relatives = Vec::new();

        for entry in all_imports {
            let url = self.get_import_url(entry.import_node);
            if is_absolute_url(&url) {
                absolute.push(entry);
            } else if url.starts_with('@') {
                projects.push(entry);
            } else {
                relatives.push(entry);
            }
        }

        let source = self.source;
        let cmp_imports = |a: &ImportWithComments<'_>, b: &ImportWithComments<'_>| -> Ordering {
            let imp1 = a
                .import_node
                .find_child_by_type(NodeType::StringChars)
                .expect("ImportComparator: not an import");
            let imp2 = b
                .import_node
                .find_child_by_type(NodeType::StringChars)
                .expect("ImportComparator: not an import");
            compare_natural(&imp1.text(source), &imp2.text(source), true)
        };
        absolute.sort_by(cmp_imports);
        projects.sort_by(cmp_imports);
        relatives.sort_by(cmp_imports);

        let mut should_newline = false;
        if !absolute.is_empty() {
            for (i, entry) in absolute.iter().enumerate() {
                if i > 0 {
                    nodes.push(self.force_line());
                }
                self.format_import_with_comments(entry, nodes);
            }
            if !projects.is_empty() || !relatives.is_empty() {
                nodes.push(self.force_line());
            }
            should_newline = true;
        }
        if !projects.is_empty() {
            if should_newline {
                nodes.push(self.force_line());
            }
            for (i, entry) in projects.iter().enumerate() {
                if i > 0 {
                    nodes.push(self.force_line());
                }
                self.format_import_with_comments(entry, nodes);
            }
            if !relatives.is_empty() {
                nodes.push(self.force_line());
            }
            should_newline = true;
        }
        if !relatives.is_empty() {
            if should_newline {
                nodes.push(self.force_line());
            }
            for (i, entry) in relatives.iter().enumerate() {
                if i > 0 {
                    nodes.push(self.force_line());
                }
                self.format_import_with_comments(entry, nodes);
            }
        }
    }

    // --- formatGeneric overloads ---

    fn format_generic_const(
        &mut self,
        children: &[Node],
        separator: Option<FormatNode>,
    ) -> Vec<FormatNode> {
        self.format_generic(children, |_, _, _| separator.clone())
    }

    fn format_generic<S>(&mut self, children: &[Node], separator_fn: S) -> Vec<FormatNode>
    where
        S: Fn(&Self, &Node, &Node) -> Option<FormatNode>,
    {
        let refs: Vec<&Node> = children.iter().collect();
        self.format_generic_refs(&refs, separator_fn)
    }

    fn format_generic_refs<S>(&mut self, children: &[&Node], separator_fn: S) -> Vec<FormatNode>
    where
        S: Fn(&Self, &Node, &Node) -> Option<FormatNode>,
    {
        self.format_generic_with_gen_refs(children, separator_fn, |this, n, _| this.format(n))
    }

    fn format_generic_with_gen<S, G>(
        &mut self,
        children: &[Node],
        separator_fn: S,
        generator_fn: G,
    ) -> Vec<FormatNode>
    where
        S: Fn(&Self, &Node, &Node) -> Option<FormatNode>,
        G: Fn(&mut Self, &Node, Option<&Node>) -> FormatNode,
    {
        let refs: Vec<&Node> = children.iter().collect();
        self.format_generic_with_gen_refs(&refs, separator_fn, generator_fn)
    }

    fn format_generic_with_gen_refs<S, G>(
        &mut self,
        children: &[&Node],
        separator_fn: S,
        generator_fn: G,
    ) -> Vec<FormatNode>
    where
        S: Fn(&Self, &Node, &Node) -> Option<FormatNode>,
        G: Fn(&mut Self, &Node, Option<&Node>) -> FormatNode,
    {
        let filtered: Vec<&Node> = children
            .iter()
            .copied()
            .filter(|child| !self.is_semicolon(child))
            .collect();

        if filtered.is_empty() {
            return Vec::new();
        }
        if filtered.len() == 1 {
            return vec![self.format(filtered[0])];
        }

        let mut nodes = Vec::new();
        let mut prev = filtered[0];
        for &child in &filtered[1..] {
            nodes.push(generator_fn(self, prev, Some(child)));
            if let Some(separator) = self.get_separator(prev, child, &separator_fn) {
                nodes.push(separator);
            }
            prev = child;
        }
        nodes.push(generator_fn(self, filtered[filtered.len() - 1], None));
        nodes
    }

    fn is_semicolon(&self, node: &Node) -> bool {
        node.kind.is_affix() && self.text(node) == ";"
    }

    fn group_non_prefixes<F>(&mut self, node: &Node, group_fn: F) -> Vec<FormatNode>
    where
        F: Fn(&mut Self, &[Node]) -> FormatNode,
    {
        let children = &node.children;
        let index = children.iter().position(|child| {
            !child.kind.is_affix()
                && child.kind != NodeType::DocComment
                && child.kind != NodeType::Annotation
        });
        let Some(index) = index else {
            return vec![group_fn(self, children)];
        };
        if index == 0 {
            return vec![group_fn(self, children)];
        }
        let prefixes = &children[..index];
        let rest = &children[index..];
        let sep = self.space_or_line();
        let mut res = self.format_generic_const(prefixes, Some(sep.clone()));
        res.push(self.get_separator_const(&prefixes[prefixes.len() - 1], &rest[0], sep));
        res.push(group_fn(self, rest));
        res
    }

    fn get_import_url(&self, node: &Node) -> String {
        let str_chars = node
            .find_child_by_type(NodeType::StringChars)
            .expect("import must have StringChars");
        let start = str_chars.span.char_index;
        let len = str_chars.span.length;
        if len < 2 {
            return String::new();
        }
        String::from_utf16_lossy(&self.source[start + 1..start + len - 1])
    }

    fn get_separator_const(&self, prev: &Node, next: &Node, separator: FormatNode) -> FormatNode {
        self.get_base_separator(prev, next).unwrap_or(separator)
    }

    fn get_separator<S>(&self, prev: &Node, next: &Node, separator_fn: &S) -> Option<FormatNode>
    where
        S: Fn(&Self, &Node, &Node) -> Option<FormatNode>,
    {
        self.get_base_separator(prev, next)
            .or_else(|| separator_fn(self, prev, next))
    }

    fn get_base_separator(&self, prev: &Node, next: &Node) -> Option<FormatNode> {
        if ends_in_line_comment(prev) {
            return Some(if lines_between(prev, next) > 1 {
                two_newlines()
            } else {
                self.must_force_line()
            });
        }
        if has_trailing_affix(prev, next) {
            return Some(FormatNode::Space);
        }
        if prev.kind == NodeType::DocComment {
            return Some(self.must_force_line());
        }
        if prev.kind == NodeType::Annotation {
            return Some(self.force_line());
        }
        if is_force_line_affix(prev.kind) || next.kind.is_affix() {
            return Some(if lines_between(prev, next) > 1 {
                two_newlines()
            } else {
                self.must_force_line()
            });
        }
        if prev.kind == NodeType::BlockComment {
            return Some(if lines_between(prev, next) > 0 {
                self.force_spacey_line()
            } else {
                FormatNode::Space
            });
        }
        if is_empty_suffix(next.kind)
            || self.is_terminal(prev, &["[", "!", "@", "[["])
            || self.is_terminal(next, &["]", "?", ","])
        {
            return Some(FormatNode::Empty);
        }
        if self.is_terminal(prev, &["class", "function", "new"])
            || self.is_terminal(next, &["=", "{", "->", "class", "function"])
            || next.kind == NodeType::ObjectBody
            || prev.kind == NodeType::ModifierList
        {
            return Some(FormatNode::Space);
        }
        if next.kind == NodeType::DocComment {
            return Some(two_newlines());
        }
        None
    }

    fn line(&self) -> FormatNode {
        if self.no_newlines {
            FormatNode::Empty
        } else {
            FormatNode::Line
        }
    }

    fn space_or_line(&self) -> FormatNode {
        if self.no_newlines {
            FormatNode::Space
        } else {
            FormatNode::SpaceOrLine
        }
    }

    fn must_force_line(&self) -> FormatNode {
        FormatNode::ForceLine
    }

    fn force_line(&self) -> FormatNode {
        if self.no_newlines {
            FormatNode::Empty
        } else {
            FormatNode::ForceLine
        }
    }

    fn force_spacey_line(&self) -> FormatNode {
        if self.no_newlines {
            FormatNode::Space
        } else {
            FormatNode::ForceLine
        }
    }

    fn if_wrap(&self, id: usize, if_wrap: FormatNode, if_not_wrap: FormatNode) -> FormatNode {
        if self.no_newlines {
            if_not_wrap
        } else {
            FormatNode::IfWrap {
                id,
                if_wrap: Box::new(if_wrap),
                if_not_wrap: Box::new(if_not_wrap),
            }
        }
    }

    fn modifier_precedence(&self, modifier: &Node) -> u8 {
        match self.text(modifier).as_str() {
            "abstract" | "open" => 0,
            "external" => 1,
            "local" | "hidden" => 2,
            "fixed" | "const" => 3,
            other => panic!("Unknown modifier `{other}`"),
        }
    }

    fn indent_after_first_newline(
        &mut self,
        mut nodes: Vec<FormatNode>,
        should_group: bool,
    ) -> Vec<FormatNode> {
        let index = nodes.iter().position(|n| {
            matches!(
                n,
                FormatNode::SpaceOrLine | FormatNode::ForceLine | FormatNode::Line
            )
        });
        let Some(index) = index else {
            return nodes;
        };
        if index == 0 {
            return nodes;
        }
        let rest = nodes.split_off(index);
        let indented = if should_group {
            FormatNode::Group {
                id: self.new_id(),
                nodes: vec![FormatNode::Indent(rest)],
            }
        } else {
            FormatNode::Indent(rest)
        };
        nodes.push(indented);
        nodes
    }

    fn group_on_space(&mut self, mut fnodes: Vec<FormatNode>) -> FormatNode {
        let mut split_starts = Vec::new();
        let mut start = 0usize;
        for (i, node) in fnodes.iter().enumerate() {
            if i > start && matches!(node, FormatNode::SpaceOrLine | FormatNode::Space) {
                split_starts.push(start);
                start = i;
            }
        }
        split_starts.push(start);
        let mut acc: Option<FormatNode> = None;
        for start in split_starts.into_iter().rev() {
            let mut nodes = fnodes.split_off(start);
            if let Some(tail) = acc.take() {
                nodes.push(tail);
            }
            acc = Some(FormatNode::Group {
                id: self.new_id(),
                nodes,
            });
        }
        acc.unwrap_or_else(|| FormatNode::Group {
            id: self.new_id(),
            nodes: Vec::new(),
        })
    }

    fn flatten_binary_operator_exprs<'b>(&self, node: &'b Node) -> Vec<&'b Node> {
        let op_node = node
            .children
            .iter()
            .find(|c| c.kind == NodeType::Operator)
            .expect("BinaryOpExpr must contain Operator");
        let prec = Operator::by_name(&self.text(op_node)).prec();
        self.flatten_binary_operator_exprs_prec(node, prec)
    }

    fn flatten_binary_operator_exprs_prec<'b>(&self, node: &'b Node, prec: u8) -> Vec<&'b Node> {
        let mut result = Vec::new();
        let mut stack: Vec<&'b Node> = vec![node];
        while let Some(cur) = stack.pop() {
            if cur.kind == NodeType::BinaryOpExpr {
                let op_node = cur
                    .children
                    .iter()
                    .find(|c| c.kind == NodeType::Operator)
                    .expect("BinaryOpExpr must contain Operator");
                if prec == Operator::by_name(&self.text(op_node)).prec() {
                    for child in cur.children.iter().rev() {
                        stack.push(child);
                    }
                    continue;
                }
            }
            result.push(cur);
        }
        result
    }

    fn text(&self, node: &Node) -> String {
        node.text(self.source)
    }

    fn is_terminal(&self, node: &Node, texts: &[&str]) -> bool {
        if node.kind != NodeType::Terminal {
            return false;
        }
        let t = self.text(node);
        texts.iter().any(|&expected| t == expected)
    }

    fn is_terminal_single(&self, node: &Node, expected: &str) -> bool {
        node.kind == NodeType::Terminal && self.text(node) == expected
    }

    fn new_id(&mut self) -> usize {
        let id = self.id;
        self.id += 1;
        id
    }
}

fn gather_facts<'a>(
    current: &'a Node,
    flat: &mut Vec<&'a Node>,
    lambda_count: &mut usize,
    method_call_count: &mut usize,
    index_before_first_method_call: &mut usize,
) {
    let mut stack: Vec<&'a Node> = current.children.iter().rev().collect();
    while let Some(child) = stack.pop() {
        if child.kind == NodeType::QualifiedAccessExpr {
            for grandchild in child.children.iter().rev() {
                stack.push(grandchild);
            }
        } else {
            flat.push(child);
            if is_method_call(Some(child)) {
                *method_call_count += 1;
                if has_function_literal(child, 2) {
                    *lambda_count += 1;
                }
            } else if *method_call_count == 0 {
                *index_before_first_method_call = flat.len() - 1;
            }
        }
    }
}

fn split_function_call_node<'a>(nodes: &[&'a Node]) -> (Vec<&'a Node>, Vec<&'a Node>) {
    assert!(!nodes.is_empty());
    let last_node = nodes[nodes.len() - 1];
    let arg_list_idx = last_node
        .children
        .iter()
        .position(|c| c.kind == NodeType::ArgumentList)
        .expect("method call must have ArgumentList");
    let mut leading: Vec<&'a Node> = nodes[..nodes.len() - 1].to_vec();
    leading.extend(last_node.children[..arg_list_idx].iter());
    let trailing: Vec<&'a Node> = last_node.children[arg_list_idx..].iter().collect();
    (leading, trailing)
}

fn is_method_call(node: Option<&Node>) -> bool {
    let Some(node) = node else {
        return false;
    };
    if node.kind != NodeType::UnqualifiedAccessExpr {
        return false;
    }
    node.children
        .iter()
        .any(|c| c.kind == NodeType::ArgumentList)
}

fn has_function_literal(node: &Node, depth: usize) -> bool {
    if node.kind == NodeType::FunctionLiteralExpr {
        return true;
    }
    for child in &node.children {
        if child.kind == NodeType::FunctionLiteralExpr {
            return true;
        }
        if depth > 0 && has_function_literal(child, depth - 1) {
            return true;
        }
    }
    false
}

fn is_expression_or_property_body(node: &Node) -> bool {
    node.kind.is_expression()
        || matches!(
            node.kind,
            NodeType::ClassPropertyBody | NodeType::ObjectPropertyBody
        )
}

fn is_same_line_expr(node: &Node) -> bool {
    matches!(
        node.kind,
        NodeType::NewExpr | NodeType::AmendsExpr | NodeType::FunctionLiteralExpr
    )
}

fn is_force_line_affix(kind: NodeType) -> bool {
    matches!(
        kind,
        NodeType::DocCommentLine | NodeType::LineComment | NodeType::Semicolon | NodeType::Shebang
    )
}

fn is_empty_suffix(kind: NodeType) -> bool {
    matches!(
        kind,
        NodeType::TypeArgumentList
            | NodeType::TypeAnnotation
            | NodeType::TypeParameterList
            | NodeType::ParameterList
    )
}

fn split_prefixes(nodes: &[Node]) -> (&[Node], &[Node]) {
    let mut split_point = 0;
    for (i, n) in nodes.iter().enumerate() {
        if !n.kind.is_affix() && n.kind != NodeType::DocComment {
            split_point = i;
            break;
        }
    }
    (&nodes[..split_point], &nodes[split_point..])
}

fn ends_in_line_comment(mut node: &Node) -> bool {
    loop {
        if node.kind == NodeType::LineComment {
            return true;
        }
        let Some(last) = node.children.last() else {
            return false;
        };
        node = last;
    }
}

fn has_trailing_affix(node: &Node, next: &Node) -> bool {
    let mut cur = Some(next);
    while let Some(n) = cur {
        if n.kind.is_affix() && node.span.line_end == n.span.line_begin {
            return true;
        }
        cur = n.children.first();
    }
    false
}

fn first_proper_child(node: &Node) -> Option<&Node> {
    node.children.iter().find(|c| is_proper(c))
}

fn last_proper_node_refs<'a>(nodes: &[&'a Node]) -> Option<&'a Node> {
    nodes.iter().rev().copied().find(|n| is_proper(n))
}

fn is_proper(node: &Node) -> bool {
    !node.kind.is_affix() && node.kind != NodeType::Terminal
}

fn is_multiline(node: &Node) -> bool {
    node.span.line_begin < node.span.line_end
}

fn is_multiline_list(nodes: &[Node]) -> bool {
    if nodes.is_empty() {
        return false;
    }
    nodes[0].span.line_begin < nodes[nodes.len() - 1].span.line_end
}

fn lines_between(prev: &Node, next: &Node) -> isize {
    next.span.line_begin as isize - prev.span.line_end as isize
}

fn is_strict_blank(s: &str) -> bool {
    s.bytes().all(|b| b == b' ' || b == b'\t')
}

fn is_absolute_url(url: &str) -> bool {
    let Some((scheme, rest)) = url.split_once(':') else {
        return false;
    };
    !scheme.is_empty()
        && scheme
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_')
        && !rest
            .chars()
            .any(|c| matches!(c, '\n' | '\r' | '\u{0085}' | '\u{2028}' | '\u{2029}'))
}

fn two_newlines() -> FormatNode {
    FormatNode::Nodes(vec![FormatNode::ForceLine, FormatNode::ForceLine])
}

fn indent_one(node: FormatNode) -> FormatNode {
    FormatNode::Indent(vec![node])
}

fn concat2(mut a: Vec<FormatNode>, b: Vec<FormatNode>) -> Vec<FormatNode> {
    a.extend(b);
    a
}

fn concat3(mut a: Vec<FormatNode>, mid: FormatNode, b: Vec<FormatNode>) -> Vec<FormatNode> {
    a.push(mid);
    a.extend(b);
    a
}

enum PairedArg<'a> {
    Single(&'a Node),
    Group {
        span: crate::FullSpan,
        children: Vec<&'a Node>,
    },
}

impl<'a> PairedArg<'a> {
    fn from_group(children: Vec<&'a Node>) -> Self {
        assert!(
            !children.is_empty(),
            "No children given for PairedArg::Group"
        );
        let first = children[0].span;
        let last = children[children.len() - 1].span;
        Self::Group {
            span: first.end_with(last),
            children,
        }
    }

    fn span(&self) -> crate::FullSpan {
        match self {
            Self::Single(n) => n.span,
            Self::Group { span, .. } => *span,
        }
    }

    fn kind(&self) -> NodeType {
        match self {
            Self::Single(n) => n.kind,
            Self::Group { .. } => NodeType::ArgumentListElements,
        }
    }

    fn ends_in_line_comment(&self) -> bool {
        match self {
            Self::Single(n) => ends_in_line_comment(n),
            Self::Group { children, .. } => {
                children.last().is_some_and(|n| ends_in_line_comment(n))
            }
        }
    }

    fn has_leading_affix_on_line(&self, line_end: usize) -> bool {
        let mut cur = match self {
            Self::Single(n) => Some(*n),
            Self::Group { children, .. } => children.first().copied(),
        };
        while let Some(n) = cur {
            if n.kind.is_affix() && line_end == n.span.line_begin {
                return true;
            }
            cur = n.children.first();
        }
        false
    }

    fn is_terminal(&self, builder: &Builder<'_>, texts: &[&str]) -> bool {
        match self {
            Self::Single(n) => builder.is_terminal(n, texts),
            Self::Group { .. } => false,
        }
    }
}
