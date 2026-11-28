//! Document IR mirroring `org.pkl.formatter.FormatNode`.

use crate::generator::MAX_WIDTH;
use std::collections::HashSet;

#[derive(Debug)]
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
    /// Shared-body conditional wrapper avoiding exponential cloning in nested object parameter lists.
    IfWrapPrefix {
        id: usize,
        wrap_indent: u8,
        wrap_prefix: Box<FormatNode>,
        no_wrap_prefix: Box<FormatNode>,
        body: Vec<FormatNode>,
    },
}

impl Clone for FormatNode {
    fn clone(&self) -> Self {
        enum Work<'a> {
            Visit(&'a FormatNode),
            FinishIndent(usize),
            FinishNodes(usize),
            FinishGroup {
                id: usize,
                start: usize,
            },
            FinishMultilineStringGroup {
                end_quote_col: usize,
                start: usize,
            },
            FinishIfWrap {
                id: usize,
            },
            FinishIfWrapPrefix {
                id: usize,
                wrap_indent: u8,
                start: usize,
            },
        }
        let mut work = vec![Work::Visit(self)];
        let mut out: Vec<FormatNode> = Vec::new();
        while let Some(task) = work.pop() {
            match task {
                Work::Visit(node) => match node {
                    Self::Text(s) => out.push(Self::Text(s.clone())),
                    Self::Empty => out.push(Self::Empty),
                    Self::Line => out.push(Self::Line),
                    Self::ForceLine => out.push(Self::ForceLine),
                    Self::SpaceOrLine => out.push(Self::SpaceOrLine),
                    Self::Space => out.push(Self::Space),
                    Self::Indent(children) => {
                        work.push(Work::FinishIndent(out.len()));
                        work.extend(children.iter().rev().map(Work::Visit));
                    }
                    Self::Nodes(children) => {
                        work.push(Work::FinishNodes(out.len()));
                        work.extend(children.iter().rev().map(Work::Visit));
                    }
                    Self::Group { id, nodes } => {
                        work.push(Work::FinishGroup {
                            id: *id,
                            start: out.len(),
                        });
                        work.extend(nodes.iter().rev().map(Work::Visit));
                    }
                    Self::MultilineStringGroup {
                        end_quote_col,
                        nodes,
                    } => {
                        work.push(Work::FinishMultilineStringGroup {
                            end_quote_col: *end_quote_col,
                            start: out.len(),
                        });
                        work.extend(nodes.iter().rev().map(Work::Visit));
                    }
                    Self::IfWrap {
                        id,
                        if_wrap,
                        if_not_wrap,
                    } => {
                        work.push(Work::FinishIfWrap { id: *id });
                        work.push(Work::Visit(if_not_wrap));
                        work.push(Work::Visit(if_wrap));
                    }
                    Self::IfWrapPrefix {
                        id,
                        wrap_indent,
                        wrap_prefix,
                        no_wrap_prefix,
                        body,
                    } => {
                        work.push(Work::FinishIfWrapPrefix {
                            id: *id,
                            wrap_indent: *wrap_indent,
                            start: out.len(),
                        });
                        work.extend(body.iter().rev().map(Work::Visit));
                        work.push(Work::Visit(no_wrap_prefix));
                        work.push(Work::Visit(wrap_prefix));
                    }
                },
                Work::FinishIndent(start) => {
                    let children = out.split_off(start);
                    out.push(Self::Indent(children));
                }
                Work::FinishNodes(start) => {
                    let children = out.split_off(start);
                    out.push(Self::Nodes(children));
                }
                Work::FinishGroup { id, start } => {
                    let nodes = out.split_off(start);
                    out.push(Self::Group { id, nodes });
                }
                Work::FinishMultilineStringGroup {
                    end_quote_col,
                    start,
                } => {
                    let nodes = out.split_off(start);
                    out.push(Self::MultilineStringGroup {
                        end_quote_col,
                        nodes,
                    });
                }
                Work::FinishIfWrap { id } => {
                    let if_not_wrap = Box::new(out.pop().expect("if_not_wrap"));
                    let if_wrap = Box::new(out.pop().expect("if_wrap"));
                    out.push(Self::IfWrap {
                        id,
                        if_wrap,
                        if_not_wrap,
                    });
                }
                Work::FinishIfWrapPrefix {
                    id,
                    wrap_indent,
                    start,
                } => {
                    let mut items = out.split_off(start).into_iter();
                    let wrap_prefix = Box::new(items.next().expect("wrap_prefix"));
                    let no_wrap_prefix = Box::new(items.next().expect("no_wrap_prefix"));
                    let body = items.collect();
                    out.push(Self::IfWrapPrefix {
                        id,
                        wrap_indent,
                        wrap_prefix,
                        no_wrap_prefix,
                        body,
                    });
                }
            }
        }
        out.pop().expect("cloned FormatNode")
    }
}

impl Drop for FormatNode {
    fn drop(&mut self) {
        let mut stack: Vec<FormatNode> = Vec::new();
        let drain = |node: &mut FormatNode, stack: &mut Vec<FormatNode>| match node {
            Self::Indent(children)
            | Self::Nodes(children)
            | Self::Group {
                nodes: children, ..
            }
            | Self::MultilineStringGroup {
                nodes: children, ..
            } => stack.extend(std::mem::take(children)),
            Self::IfWrap {
                if_wrap,
                if_not_wrap,
                ..
            } => {
                stack.push(std::mem::replace(if_wrap.as_mut(), Self::Empty));
                stack.push(std::mem::replace(if_not_wrap.as_mut(), Self::Empty));
            }
            Self::IfWrapPrefix {
                wrap_prefix,
                no_wrap_prefix,
                body,
                ..
            } => {
                stack.push(std::mem::replace(wrap_prefix.as_mut(), Self::Empty));
                stack.push(std::mem::replace(no_wrap_prefix.as_mut(), Self::Empty));
                stack.extend(std::mem::take(body));
            }
            _ => {}
        };
        drain(self, &mut stack);
        while let Some(mut item) = stack.pop() {
            drain(&mut item, &mut stack);
        }
    }
}

impl FormatNode {
    /// Computes width up to `limit + 1`, short-circuiting as soon as `total > limit`
    /// so deeply nested groups cannot trigger O(N^2) layout width traversals.
    pub(crate) fn width_bounded(&self, wrapped: &HashSet<usize>, limit: usize) -> usize {
        let mut total = 0usize;
        let mut stack = vec![self];
        while let Some(node) = stack.pop() {
            match node {
                Self::Nodes(nodes) | Self::Group { nodes, .. } | Self::Indent(nodes) => {
                    stack.extend(nodes.iter());
                }
                Self::IfWrap {
                    id,
                    if_wrap,
                    if_not_wrap,
                } => {
                    if wrapped.contains(id) {
                        stack.push(if_wrap);
                    } else {
                        stack.push(if_not_wrap);
                    }
                }
                Self::IfWrapPrefix {
                    id,
                    wrap_prefix,
                    no_wrap_prefix,
                    body,
                    ..
                } => {
                    if wrapped.contains(id) {
                        stack.push(wrap_prefix);
                    } else {
                        stack.push(no_wrap_prefix);
                    }
                    stack.extend(body.iter());
                }
                Self::Text(text) => {
                    total = total.saturating_add(text.encode_utf16().count());
                    if total > limit {
                        return total;
                    }
                }
                Self::SpaceOrLine | Self::Space => {
                    total = total.saturating_add(1);
                    if total > limit {
                        return total;
                    }
                }
                Self::ForceLine | Self::MultilineStringGroup { .. } => {
                    total = total.saturating_add(MAX_WIDTH);
                    if total > limit {
                        return total;
                    }
                }
                Self::Empty | Self::Line => {}
            }
        }
        total
    }
}
