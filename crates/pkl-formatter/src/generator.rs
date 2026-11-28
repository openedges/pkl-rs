//! Layout generator mirroring `org.pkl.formatter.Generator` and `Wrap`.

use crate::format_node::FormatNode;
use crate::unicode_tables::is_java_blank;
use std::collections::HashSet;
use std::fmt;

pub(crate) const MAX_WIDTH: usize = 100;
const INDENT: &str = "  ";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Wrap {
    Enabled,
    Detect,
}

impl Wrap {
    const fn is_enabled(self) -> bool {
        matches!(self, Self::Enabled)
    }
}

pub(crate) struct Generator<'a, W: fmt::Write> {
    buf: &'a mut W,
    indent: usize,
    size: usize,
    wrapped: HashSet<usize>,
    should_add_indent: bool,
}

impl<'a, W: fmt::Write> Generator<'a, W> {
    pub(crate) fn new(buf: &'a mut W) -> Self {
        Self {
            buf,
            indent: 0,
            size: 0,
            wrapped: HashSet::new(),
            should_add_indent: false,
        }
    }

    pub(crate) fn generate(&mut self, node: &FormatNode) -> fmt::Result {
        enum GenTask<'n> {
            Node(&'n FormatNode, Wrap),
            PopIndent(usize),
            Multiline {
                end_quote_col: usize,
                indent_length: usize,
                old_indent: usize,
                nodes: &'n [FormatNode],
                index: usize,
                previous_newline: bool,
            },
        }
        let mut stack = vec![GenTask::Node(node, Wrap::Detect)];
        while let Some(task) = stack.pop() {
            match task {
                GenTask::PopIndent(levels) => {
                    self.indent = self.indent.saturating_sub(levels);
                }
                GenTask::Multiline {
                    end_quote_col,
                    indent_length,
                    old_indent,
                    nodes,
                    mut index,
                    mut previous_newline,
                } => {
                    while index < nodes.len() {
                        let child = &nodes[index];
                        index += 1;
                        match child {
                            FormatNode::ForceLine => {
                                self.newline(false)?;
                                previous_newline = true;
                            }
                            FormatNode::Text(t)
                                if previous_newline
                                    && is_java_blank(t)
                                    && t.encode_utf16().count() == old_indent
                                    && matches!(nodes.get(index), Some(FormatNode::ForceLine)) =>
                            {
                                // skip blank line indentation that will be repositioned
                                previous_newline = false;
                            }
                            FormatNode::Text(t) if previous_newline => {
                                let repositioned =
                                    reposition(t, end_quote_col.saturating_sub(1), indent_length);
                                self.text(&repositioned)?;
                                previous_newline = false;
                            }
                            _ => {
                                stack.push(GenTask::Multiline {
                                    end_quote_col,
                                    indent_length,
                                    old_indent,
                                    nodes,
                                    index,
                                    previous_newline: false,
                                });
                                stack.push(GenTask::Node(child, Wrap::Detect));
                                break;
                            }
                        }
                    }
                }
                GenTask::Node(node, wrap) => match node {
                    FormatNode::Empty => {}
                    FormatNode::Nodes(children) => {
                        for child in children.iter().rev() {
                            stack.push(GenTask::Node(child, wrap));
                        }
                    }
                    FormatNode::Group { id, nodes } => {
                        let remaining = MAX_WIDTH.saturating_sub(self.size);
                        let mut width = 0usize;
                        for child in nodes {
                            if width > remaining {
                                break;
                            }
                            let child_limit = remaining.saturating_sub(width);
                            width = width
                                .saturating_add(child.width_bounded(&self.wrapped, child_limit));
                        }
                        let group_wrap = if self.size.saturating_add(width) > MAX_WIDTH {
                            self.wrapped.insert(*id);
                            Wrap::Enabled
                        } else {
                            Wrap::Detect
                        };
                        for child in nodes.iter().rev() {
                            stack.push(GenTask::Node(child, group_wrap));
                        }
                    }
                    FormatNode::IfWrap {
                        id,
                        if_wrap,
                        if_not_wrap,
                    } => {
                        if self.wrapped.contains(id) {
                            stack.push(GenTask::Node(if_wrap, Wrap::Enabled));
                        } else {
                            stack.push(GenTask::Node(if_not_wrap, wrap));
                        }
                    }
                    FormatNode::IfWrapPrefix {
                        id,
                        wrap_indent,
                        wrap_prefix,
                        no_wrap_prefix,
                        body,
                    } => {
                        if self.wrapped.contains(id) {
                            let levels = usize::from(*wrap_indent);
                            if levels > 0 {
                                self.size = self.size.saturating_add(INDENT.len() * levels);
                                self.indent += levels;
                                stack.push(GenTask::PopIndent(levels));
                            }
                            for child in body.iter().rev() {
                                stack.push(GenTask::Node(child, Wrap::Enabled));
                            }
                            stack.push(GenTask::Node(wrap_prefix, Wrap::Enabled));
                        } else {
                            for child in body.iter().rev() {
                                stack.push(GenTask::Node(child, wrap));
                            }
                            stack.push(GenTask::Node(no_wrap_prefix, wrap));
                        }
                    }
                    FormatNode::Text(value) => self.text(value)?,
                    FormatNode::Line => {
                        if wrap.is_enabled() {
                            self.newline(true)?;
                        }
                    }
                    FormatNode::ForceLine => self.newline(true)?,
                    FormatNode::SpaceOrLine => {
                        if wrap.is_enabled() {
                            self.newline(true)?;
                        } else {
                            self.text(" ")?;
                        }
                    }
                    FormatNode::Space => self.text(" ")?,
                    FormatNode::Indent(children) => {
                        if wrap.is_enabled() && !children.is_empty() {
                            self.size = self.size.saturating_add(INDENT.len());
                            self.indent += 1;
                            stack.push(GenTask::PopIndent(1));
                        }
                        for child in children.iter().rev() {
                            stack.push(GenTask::Node(child, wrap));
                        }
                    }
                    FormatNode::MultilineStringGroup {
                        end_quote_col,
                        nodes,
                    } => {
                        let indent_length = self.indent * INDENT.len();
                        let old_indent = indent_for(nodes);
                        stack.push(GenTask::Multiline {
                            end_quote_col: *end_quote_col,
                            indent_length,
                            old_indent,
                            nodes,
                            index: 0,
                            previous_newline: false,
                        });
                    }
                },
            }
        }
        Ok(())
    }

    fn text(&mut self, value: &str) -> fmt::Result {
        if self.should_add_indent {
            for _ in 0..self.indent {
                self.buf.write_str(INDENT)?;
            }
            self.should_add_indent = false;
        }
        self.size += value.encode_utf16().count();
        self.buf.write_str(value)
    }

    fn newline(&mut self, should_indent: bool) -> fmt::Result {
        self.size = INDENT.len() * self.indent;
        self.buf.write_char('\n')?;
        self.should_add_indent = should_indent;
        Ok(())
    }
}

/// Accept text indented by `original_offset` UTF-16 code units (tabs or spaces)
/// and return it indented by `new_offset` spaces.
fn reposition(text: &str, original_offset: usize, new_offset: usize) -> String {
    let suffix = utf16_slice_from(text, original_offset);
    let mut out = String::with_capacity(new_offset + suffix.len());
    for _ in 0..new_offset {
        out.push(' ');
    }
    out.push_str(suffix);
    out
}

fn utf16_slice_from(text: &str, utf16_offset: usize) -> &str {
    let mut units = 0;
    for (byte_idx, ch) in text.char_indices() {
        if units >= utf16_offset {
            return &text[byte_idx..];
        }
        units += ch.len_utf16();
    }
    ""
}

fn indent_for(nodes: &[FormatNode]) -> usize {
    if nodes.len() < 2 {
        return 0;
    }
    match &nodes[nodes.len() - 2] {
        FormatNode::Text(t) => t.encode_utf16().count(),
        _ => 0,
    }
}
