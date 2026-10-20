//! The constant key subset produced by the pinned AST builder.
use super::Binding;
use crate::source::Span;
use crate::syntax::{Expr, ExprKind, UnaryOp};
use std::collections::BTreeMap;

#[derive(PartialEq, Eq)]
pub(super) enum ConstantKey {
    Null,
    Bool(bool),
    Int(i64),
    Float(u64),
    String(String),
    List(Vec<Self>),
}

pub(super) fn resolve(expression: &Expr, names: &BTreeMap<Span, Binding>) -> Option<ConstantKey> {
    Some(match &expression.unparenthesized().kind {
        ExprKind::Null => ConstantKey::Null,
        ExprKind::Bool(value) => ConstantKey::Bool(*value),
        ExprKind::Int(value) => ConstantKey::Int(*value),
        ExprKind::Float(value) => ConstantKey::Float(value.to_bits()),
        ExprKind::String(value) => ConstantKey::String(value.clone()),
        ExprKind::Unary {
            op: UnaryOp::Negate,
            operand,
        } => match operand.kind {
            ExprKind::Int(value) => ConstantKey::Int(value.checked_neg()?),
            ExprKind::Float(value) => ConstantKey::Float((-value).to_bits()),
            _ => return None,
        },
        ExprKind::Call { callee, arguments }
            if matches!(&callee.kind, ExprKind::Name(name) if name == "List")
                && names.get(&callee.span) == Some(&Binding::ImplicitMethod) =>
        {
            ConstantKey::List(
                arguments
                    .iter()
                    .map(|argument| resolve(argument, names))
                    .collect::<Option<_>>()?,
            )
        }
        _ => return None,
    })
}

impl ConstantKey {
    pub(super) fn render(&self) -> String {
        let text = self.text();
        let mut units: Vec<_> = text.encode_utf16().collect();
        if units.len() >= 80 {
            units.truncate(77);
            units.extend("...".encode_utf16());
        }
        char::decode_utf16(units)
            .map(|unit| unit.unwrap_or('?'))
            .collect()
    }

    fn text(&self) -> String {
        match self {
            Self::Null => "null".into(),
            Self::Bool(value) => value.to_string(),
            Self::Int(value) => value.to_string(),
            Self::Float(value) => crate::numeric::double_to_string(f64::from_bits(*value)),
            Self::String(value) => {
                let mut text = String::from("\"");
                for ch in value.chars() {
                    match ch {
                        '\n' => text.push_str("\\n"),
                        '\r' => text.push_str("\\r"),
                        '\t' => text.push_str("\\t"),
                        '\\' => text.push_str("\\\\"),
                        '"' => text.push_str("\\\""),
                        ch => text.push(ch),
                    }
                }
                text.push('"');
                text
            }
            Self::List(values) => format!(
                "List({})",
                values.iter().map(Self::text).collect::<Vec<_>>().join(", ")
            ),
        }
    }
}
