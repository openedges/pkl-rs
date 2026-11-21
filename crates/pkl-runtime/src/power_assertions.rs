//! Source-based layout for the pinned evaluator's uncolored power assertions.
//!
//! Observations come from VM execution. Parsing here only associates authored
//! expressions with display positions; it never evaluates the source.
use crate::diagnostic::PowerAssertionValue;
use crate::diagnostic::ansi::{AnsiCode, AnsiStringBuilder};
use crate::pkl_string::PklString;
use crate::source::{Source, SourceCharacter, SourceSlice, Span};
use crate::syntax::{Expr, ExprKind, ObjectMember};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Entry {
    line: usize,
    column: usize,
    text: PklString,
    is_marker: bool,
}

impl Entry {
    fn width(&self) -> usize {
        self.text.encode_utf16().count()
    }

    fn marker(&self) -> Self {
        Self {
            text: "│".into(),
            is_marker: true,
            ..self.clone()
        }
    }
}

#[cfg(test)]
pub(crate) fn render(
    source: &Source,
    body: Span,
    observations: &[PowerAssertionValue],
    indent: &str,
) -> PklString {
    let mut out = AnsiStringBuilder::new(false);
    render_to_ansi(&mut out, source, body, observations, indent, None);
    out.finish_pkl()
}

/// Renders a power-assertion diagram into an [`AnsiStringBuilder`], mirroring
/// `PowerAssertions.render(out, indent, sourceSection, trackedValues, firstFrameSuffix)`.
pub fn render_to_ansi(
    out: &mut AnsiStringBuilder,
    source: &Source,
    body: Span,
    observations: &[PowerAssertionValue],
    indent: &str,
    first_line_suffix: Option<&dyn Fn(&mut AnsiStringBuilder)>,
) {
    let text = source
        .slice(body)
        .unwrap_or_else(|| SourceSlice::from_utf8(""));
    let mut anchors = BTreeMap::new();
    if let Ok(expression) = crate::parser::parse_expression_source(text) {
        expression_anchors(&expression, text, true, &mut anchors);
    }
    let entries: BTreeSet<_> = observations
        .iter()
        .filter(|item| !item.hidden && item.executions == 1)
        .filter_map(|item| {
            let relative = Span::new(
                item.span.start.checked_sub(body.start)?,
                item.span.end.checked_sub(body.start)?,
            );
            let anchor = anchors.get(&relative).copied().flatten()?;
            let position = source.position(body.start + anchor)?;
            Some(Entry {
                line: position.line,
                column: position.column_utf16,
                text: item.value.clone(),
                is_marker: false,
            })
        })
        .collect();
    let lines = source_lines(source, body);
    let indentation = if lines.len() == 1 {
        0
    } else {
        lines
            .iter()
            .skip(1)
            .map(|(_, text)| {
                text.characters()
                    .take_while(|(_, ch)| matches!(ch, SourceCharacter::Scalar(' ' | '\t')))
                    .count()
            })
            .min()
            .unwrap_or(0)
    };
    out.append_sandboxed(|out| {
        let mut had_markers = false;
        for (index, (start, text)) in lines.iter().enumerate() {
            if had_markers {
                out.append_str("\n\n");
            }
            let Some(position) = source.position(*start) else {
                continue;
            };
            let trim = if index == 0 {
                position.column_utf16 - 1
            } else {
                indentation
            };
            let trim_bytes: usize = text
                .characters()
                .take(trim)
                .take_while(|(_, ch)| ch.scalar().is_some_and(crate::diagnostic::java_whitespace))
                .map(|(_, ch)| ch.len_bytes())
                .sum();
            out.append_str(indent);
            out.append_pkl(
                &text
                    .get(trim_bytes..)
                    .expect("whitespace boundary")
                    .to_pkl_string(),
            );
            if index == 0
                && let Some(suffix) = first_line_suffix
            {
                suffix(out);
            }
            out.append_char('\n');
            let layers = layers(
                entries
                    .iter()
                    .filter(|entry| entry.line == position.line)
                    .cloned()
                    .collect(),
            );
            had_markers = !layers.is_empty();
            for (layer_idx, layer) in layers.iter().enumerate() {
                if layer_idx > 0 {
                    out.append_char('\n');
                }
                out.append_str(indent);
                let mut cursor = 0;
                for entry in layer {
                    let offset = entry.column.saturating_sub(1 + trim + cursor);
                    out.append_str(&" ".repeat(offset));
                    if entry.is_marker {
                        out.append_styled(AnsiCode::Faint, "│");
                    } else if out.using_color() {
                        crate::diagnostic::highlight::write_to(out, &entry.text);
                    } else {
                        out.append_pkl(&entry.text);
                    }
                    cursor += offset + entry.width();
                }
            }
        }
    });
}

fn source_lines(source: &Source, span: Span) -> Vec<(usize, SourceSlice<'_>)> {
    let mut lines = Vec::new();
    let text = source.text().entire();
    let mut start = span.start;
    let mut position = span.start;
    while position < span.end {
        let Some(ch) = text.get(position..).and_then(SourceSlice::first) else {
            break;
        };
        if matches!(ch, SourceCharacter::Scalar('\r' | '\n')) {
            if position > start {
                lines.push((start, text.get(start..position).expect("line boundaries")));
            }
            position += ch.len_bytes();
            if ch == SourceCharacter::Scalar('\r') && text.as_bytes().get(position) == Some(&b'\n')
            {
                position += 1;
            }
            start = position;
        } else {
            position += ch.len_bytes();
        }
    }
    if start < span.end
        && let Some(line) = text.get(start..span.end)
    {
        lines.push((start, line));
    }
    lines
}

fn layers(mut remaining: Vec<Entry>) -> Vec<Vec<Entry>> {
    remaining.sort_by_key(|entry| std::cmp::Reverse(entry.column));
    if remaining.is_empty() {
        return Vec::new();
    }
    let mut markers = VecDeque::new();
    let mut previous = None;
    for entry in &remaining {
        if previous != Some(entry.column) {
            markers.push_front(entry.marker());
            previous = Some(entry.column);
        }
    }
    let mut result = vec![markers.into_iter().collect()];
    while !remaining.is_empty() {
        let mut layer: VecDeque<Entry> = VecDeque::new();
        remaining.retain(|entry| {
            if layer
                .front()
                .is_none_or(|next| entry.column + entry.width() + 1 < next.column)
            {
                layer.push_front(entry.clone());
                false
            } else {
                if layer.front().is_none_or(|next| entry.column < next.column) {
                    layer.push_front(entry.marker());
                }
                true
            }
        });
        result.push(layer.into_iter().collect());
    }
    result
}

fn expression_anchors(
    expression: &Expr,
    text: SourceSlice<'_>,
    root: bool,
    anchors: &mut BTreeMap<Span, Option<usize>>,
) {
    let mut work = vec![(expression, root, true, None)];
    while let Some((expression, root, visible, child_visibility)) = work.pop() {
        if let ExprKind::Parenthesized(inner) = &expression.kind {
            anchors.insert(expression.span, None);
            work.push((inner, root, child_visibility.unwrap_or(visible), None));
            continue;
        }
        let mut children = Vec::new();
        let mut scoped_children = Vec::new();
        let child_visible = child_visibility.unwrap_or(visible);
        let mut anchor = Some(expression.span.start);
        match &expression.kind {
            ExprKind::Parenthesized(_) => unreachable!("handled above"),
            ExprKind::Import { .. }
            | ExprKind::Int(_)
            | ExprKind::Float(_)
            | ExprKind::Null
            | ExprKind::String(_)
            | ExprKind::Bool(_)
            | ExprKind::Uninitialized
            | ExprKind::EntryParent
            | ExprKind::TypeAlias { .. }
            | ExprKind::Class { .. } => anchor = None,
            ExprKind::Function { body, method, .. } => {
                anchor = None;
                // Object methods are not FunctionLiteralExpr boundaries in
                // the Java parser. Only lambda bodies reset visibility.
                let body_visible = if *method { child_visible } else { root };
                scoped_children.push((body.as_ref(), body_visible, None));
            }
            ExprKind::Let { value, body, .. } => {
                anchor = None;
                children.extend([value.as_ref(), body.as_ref()]);
            }
            ExprKind::Binary { left, right, .. } => {
                anchor = Some(next_token(text, left.span.end, false));
                children.extend([left.as_ref(), right.as_ref()]);
            }
            ExprKind::TypeCheck { value, .. } | ExprKind::TypeCast { value, .. } => {
                anchor = Some(next_token(text, value.span.end, false));
                children.push(value.as_ref());
            }
            ExprKind::NullableMember(receiver, _) | ExprKind::Member(receiver, _) => {
                anchor = Some(next_token(text, receiver.span.end, true));
                children.push(receiver.as_ref());
            }
            ExprKind::Subscript { receiver, key } => {
                anchor = Some(next_token(text, receiver.span.end, false));
                children.extend([receiver.as_ref(), key.as_ref()]);
            }
            ExprKind::Call { callee, arguments } => {
                anchor = match &callee.kind {
                    ExprKind::NullableMember(receiver, _) | ExprKind::Member(receiver, _) => {
                        Some(next_token(text, receiver.span.end, true))
                    }
                    _ => Some(callee.span.start),
                };
                children.push(callee.as_ref());
                children.extend(arguments);
            }
            ExprKind::NonNull(value)
            | ExprKind::Throw(value)
            | ExprKind::Trace(value)
            | ExprKind::ReadResourceGlob(value)
            | ExprKind::ReadResource(value)
            | ExprKind::ReadResourceOrNull(value)
            | ExprKind::Unary { operand: value, .. } => children.push(value.as_ref()),
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => children.extend([
                condition.as_ref(),
                then_branch.as_ref(),
                else_branch.as_ref(),
            ]),
            ExprKind::InterpolatedString(parts) => {
                // Synthetic StringChars share the full literal span in our AST.
                // Visiting them would replace the literal's visible anchor with
                // None; retain only actual interpolation expressions as children.
                children.extend(parts.iter().filter(|part| {
                    !matches!(part.kind, ExprKind::String(_)) || part.span != expression.span
                }));
            }
            ExprKind::Object {
                parent, members, ..
            } => {
                children.extend(parent.iter().map(AsRef::as_ref));
                member_expressions(members, child_visible, &mut scoped_children);
            }
            ExprKind::New { members, .. } | ExprKind::Annotation { members, .. } => {
                member_expressions(members, child_visible, &mut scoped_children);
            }
            ExprKind::Name(_)
            | ExprKind::This
            | ExprKind::Outer
            | ExprKind::Module
            | ExprKind::Super => {}
        }
        anchors.insert(expression.span, if visible { anchor } else { None });
        work.extend(
            scoped_children
                .into_iter()
                .rev()
                .map(|(child, visible, child_visibility)| {
                    (child, false, visible, child_visibility)
                }),
        );
        work.extend(
            children
                .into_iter()
                .rev()
                .map(|child| (child, false, child_visible, None)),
        );
    }
}

fn member_expressions<'a>(
    members: &'a [ObjectMember],
    visible: bool,
    expressions: &mut Vec<(&'a Expr, bool, Option<bool>)>,
) {
    for member in members {
        match member {
            ObjectMember::Property(member) => expressions.push((&member.expr, visible, None)),
            ObjectMember::Element { value, .. } | ObjectMember::Spread { value, .. } => {
                expressions.push((value, visible, None));
            }
            ObjectMember::Entry { key, value, .. } => {
                expressions.extend([(key, visible, None), (value, visible, None)]);
            }
            ObjectMember::When {
                condition,
                then_members,
                else_members,
                ..
            } => {
                expressions.push((condition, visible, None));
                member_expressions(then_members, visible, expressions);
                member_expressions(else_members, visible, expressions);
            }
            ObjectMember::For {
                iterable, members, ..
            } => {
                // Java stops at the nearest for-generator ancestor. Its
                // iterable is visible even when an outer boundary hides it.
                expressions.push((iterable, true, None));
                member_expressions(members, false, expressions);
            }
            ObjectMember::Predicate {
                condition,
                value,
                span,
                ..
            } => {
                expressions.push((condition, false, None));
                if value.span == *span {
                    // The parser synthesizes an amendment expression spanning
                    // the predicate itself. Java tests the parser node's parent,
                    // so its value is visible subject to enclosing boundaries;
                    // descendants still belong to the predicate body.
                    expressions.push((value, visible, Some(false)));
                } else {
                    expressions.push((value, false, None));
                }
            }
        }
    }
}

// The operator can follow whitespace and comments. Source spans are byte
// offsets; conversion to the pinned UTF-16 columns happens only at rendering.
fn next_token(text: SourceSlice<'_>, mut offset: usize, after_dot: bool) -> usize {
    loop {
        while let Some(ch) = text
            .get(offset..)
            .and_then(SourceSlice::first)
            .filter(|ch| ch.scalar().is_some_and(char::is_whitespace))
        {
            offset += ch.len_bytes();
        }
        let Some(rest) = text.get(offset..) else {
            return offset;
        };
        if rest.starts_with("//") {
            offset = rest
                .characters()
                .find(|(_, ch)| matches!(ch, SourceCharacter::Scalar('\r' | '\n')))
                .map_or(text.len_bytes(), |(end, _)| offset + end);
        } else if rest.starts_with("/*") {
            let mut depth = 1;
            offset += 2;
            while offset < text.len_bytes() && depth > 0 {
                let rest = text.get(offset..).expect("comment character boundary");
                if rest.starts_with("/*") {
                    depth += 1;
                    offset += 2;
                } else if rest.starts_with("*/") {
                    depth -= 1;
                    offset += 2;
                } else {
                    offset += rest.first().expect("nonempty comment").len_bytes();
                }
            }
        } else {
            break;
        }
    }
    if after_dot && let Some(rest) = text.get(offset..) {
        if rest.starts_with("?.") {
            offset += 2;
        } else if rest.starts_with(".") {
            offset += 1;
        }
    }
    offset
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::SourceMap;

    fn graph(expression: &str, observations: &[(&str, &str, usize)]) -> PklString {
        let text = format!("result: Int({expression}) = 42\n");
        let mut map = SourceMap::default();
        let id = map.add("test.pkl", &text);
        let source = map.get(id).unwrap();
        let start = "result: Int(".len();
        let observations: Vec<_> = observations
            .iter()
            .map(|(section, value, executions)| {
                let offset = expression.rfind(section).unwrap() + start;
                PowerAssertionValue {
                    span: Span::new(offset, offset + section.len()),
                    value: (*value).into(),
                    executions: *executions,
                    hidden: false,
                }
            })
            .collect();
        render(
            source,
            Span::new(start, start + expression.len()),
            &observations,
            "    ",
        )
    }

    // Layout expectations are taken from the pinned Java power-assertion
    // specimens, including marker collision and the literal-only newline.
    #[test]
    fn scalar_comparison_and_literal_layout() {
        assert_eq!(
            graph("this > 42", &[("this", "42", 1), ("this > 42", "false", 1)]),
            "    this > 42\n    │    │\n    42   false"
        );
        assert_eq!(graph("false", &[("false", "false", 1)]), "    false\n");
    }

    #[test]
    fn function_body_collision_and_utf16_value_width() {
        assert_eq!(
            graph("(x) -> x > 0", &[("x > 0", "false", 1), ("x", "-1", 1)]),
            "    (x) -> x > 0\n           │ │\n           │ false\n           -1"
        );
        assert_eq!(
            graph(
                "this == \"😀é\"",
                &[("this", "\"😀x\"", 1), ("this == \"😀é\"", "false", 1)]
            ),
            "    this == \"😀é\"\n    │    │\n    │    false\n    \"😀x\""
        );
    }

    #[test]
    fn repeated_observations_and_nested_lambda_body_are_suppressed() {
        assert_eq!(
            graph("this > 42", &[("this", "42", 2), ("this > 42", "false", 1)]),
            "    this > 42\n         │\n         false"
        );
        assert_eq!(
            graph(
                "let (f = (x) -> x > 0) f.apply(this)",
                &[("x > 0", "false", 1), ("this", "42", 1)]
            ),
            "    let (f = (x) -> x > 0) f.apply(this)\n                                   │\n                                   42"
        );
    }

    #[test]
    fn multiline_operators_and_comments_keep_authored_positions() {
        assert_eq!(
            graph(
                "this +\n  1 > 100",
                &[
                    ("this", "42", 1),
                    ("this +\n  1", "43", 1),
                    ("this +\n  1 > 100", "false", 1)
                ]
            ),
            "    this +\n    │    │\n    42   43\n\n    1 > 100\n      │\n      false"
        );
        assert_eq!(
            graph(
                "this /* op */ > 42",
                &[("this", "42", 1), ("this /* op */ > 42", "false", 1)]
            ),
            "    this /* op */ > 42\n    │             │\n    42            false"
        );
    }

    #[test]
    fn grouped_receivers_keep_inner_value_and_outer_operator_positions() {
        assert_eq!(
            graph(
                "(this) > 42",
                &[("this", "5", 1), ("(this) > 42", "false", 1)]
            ),
            "    (this) > 42\n     │     │\n     5     false"
        );
        assert_eq!(
            graph(
                "(limit) > this",
                &[
                    ("limit", "0", 1),
                    ("this", "5", 1),
                    ("(limit) > this", "false", 1)
                ]
            ),
            "    (limit) > this\n     │      │ │\n     0      │ 5\n            false"
        );
    }
}
