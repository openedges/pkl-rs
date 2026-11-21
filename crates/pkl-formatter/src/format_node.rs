//! Document IR mirroring `org.pkl.formatter.FormatNode`.

use crate::generator::MAX_WIDTH;
use std::collections::HashSet;

#[derive(Clone, Debug)]
pub(crate) enum FormatNode {
    Text(String),
    Empty,
    Line,
    ForceLine,
    SpaceOrLine,
    Space,
    Indent(Vec<FormatNode>),
    Nodes(Vec<FormatNode>),
    Group {
        id: usize,
        nodes: Vec<FormatNode>,
    },
    MultilineStringGroup {
        end_quote_col: usize,
        nodes: Vec<FormatNode>,
    },
    IfWrap {
        id: usize,
        if_wrap: Box<FormatNode>,
        if_not_wrap: Box<FormatNode>,
    },
}

impl FormatNode {
    pub(crate) fn width(&self, wrapped: &HashSet<usize>) -> usize {
        match self {
            Self::Nodes(nodes) | Self::Group { nodes, .. } | Self::Indent(nodes) => {
                nodes.iter().map(|node| node.width(wrapped)).sum()
            }
            Self::IfWrap {
                id,
                if_wrap,
                if_not_wrap,
            } => {
                if wrapped.contains(id) {
                    if_wrap.width(wrapped)
                } else {
                    if_not_wrap.width(wrapped)
                }
            }
            Self::Text(text) => text.encode_utf16().count(),
            Self::SpaceOrLine | Self::Space => 1,
            Self::ForceLine | Self::MultilineStringGroup { .. } => MAX_WIDTH,
            Self::Empty | Self::Line => 0,
        }
    }
}
