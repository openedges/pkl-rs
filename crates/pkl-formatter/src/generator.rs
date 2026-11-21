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
        self.node(node, Wrap::Detect)
    }

    fn node(&mut self, node: &FormatNode, wrap: Wrap) -> fmt::Result {
        match node {
            FormatNode::Empty => Ok(()),
            FormatNode::Nodes(children) => {
                for child in children {
                    self.node(child, wrap)?;
                }
                Ok(())
            }
            FormatNode::Group { id, nodes } => {
                let width: usize = nodes.iter().map(|child| child.width(&self.wrapped)).sum();
                let group_wrap = if self.size + width > MAX_WIDTH {
                    self.wrapped.insert(*id);
                    Wrap::Enabled
                } else {
                    Wrap::Detect
                };
                for child in nodes {
                    self.node(child, group_wrap)?;
                }
                Ok(())
            }
            FormatNode::IfWrap {
                id,
                if_wrap,
                if_not_wrap,
            } => {
                if self.wrapped.contains(id) {
                    self.node(if_wrap, Wrap::Enabled)
                } else {
                    self.node(if_not_wrap, wrap)
                }
            }
            FormatNode::Text(value) => self.text(value),
            FormatNode::Line => {
                if wrap.is_enabled() {
                    self.newline(true)?;
                }
                Ok(())
            }
            FormatNode::ForceLine => self.newline(true),
            FormatNode::SpaceOrLine => {
                if wrap.is_enabled() {
                    self.newline(true)
                } else {
                    self.text(" ")
                }
            }
            FormatNode::Space => self.text(" "),
            FormatNode::Indent(children) => {
                if wrap.is_enabled() && !children.is_empty() {
                    self.size += INDENT.len();
                    self.indent += 1;
                    for child in children {
                        self.node(child, wrap)?;
                    }
                    self.indent -= 1;
                } else {
                    for child in children {
                        self.node(child, wrap)?;
                    }
                }
                Ok(())
            }
            FormatNode::MultilineStringGroup {
                end_quote_col,
                nodes,
            } => {
                let indent_length = self.indent * INDENT.len();
                let old_indent = indent_for(nodes);
                let mut previous_newline = false;
                for (i, child) in nodes.iter().enumerate() {
                    match child {
                        FormatNode::ForceLine => {
                            self.newline(false)?;
                        }
                        FormatNode::Text(t)
                            if previous_newline
                                && is_java_blank(t)
                                && t.encode_utf16().count() == old_indent
                                && matches!(nodes.get(i + 1), Some(FormatNode::ForceLine)) =>
                        {
                            // skip blank line indentation that will be repositioned
                        }
                        FormatNode::Text(t) if previous_newline => {
                            let repositioned =
                                reposition(t, end_quote_col.saturating_sub(1), indent_length);
                            self.text(&repositioned)?;
                        }
                        _ => {
                            self.node(child, Wrap::Detect)?;
                        }
                    }
                    previous_newline = matches!(child, FormatNode::ForceLine);
                }
                Ok(())
            }
        }
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
