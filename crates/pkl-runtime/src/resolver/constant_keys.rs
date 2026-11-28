//! The constant key subset produced by the pinned AST builder.
use super::Binding;
use crate::heap::FastMap;
use crate::source::Span;
use crate::syntax::{Expr, ExprKind, UnaryOp};
use crate::value::PklString;

// A prefix token stream keeps construction, equality, rendering and destruction
// iterative even when a literal List key is nested thousands of times.
#[derive(PartialEq, Eq, Hash)]
pub(super) struct ConstantKey(Vec<ConstantToken>);

#[derive(PartialEq, Eq, Hash)]
enum ConstantToken {
    Null,
    Bool(bool),
    Int(i64),
    Float(u64),
    String(PklString),
    List(usize),
}

pub(super) fn resolve(expression: &Expr, names: &FastMap<Span, Binding>) -> Option<ConstantKey> {
    let mut work = vec![expression];
    let mut tokens = Vec::new();
    while let Some(expression) = work.pop() {
        tokens.push(match &expression.unparenthesized().kind {
            ExprKind::Null => ConstantToken::Null,
            ExprKind::Bool(value) => ConstantToken::Bool(*value),
            ExprKind::Int(value) => ConstantToken::Int(*value),
            ExprKind::Float(value) => ConstantToken::Float(value.to_bits()),
            ExprKind::String(value) => ConstantToken::String(value.clone()),
            ExprKind::Unary {
                op: UnaryOp::Negate,
                operand,
            } => match operand.kind {
                ExprKind::Int(value) => ConstantToken::Int(value.checked_neg()?),
                ExprKind::Float(value) => ConstantToken::Float((-value).to_bits()),
                _ => return None,
            },
            ExprKind::Call { callee, arguments }
                if matches!(&callee.kind, ExprKind::Name(name) if name == "List")
                    && names.get(&callee.span) == Some(&Binding::ImplicitMethod) =>
            {
                work.extend(arguments.iter().rev());
                ConstantToken::List(arguments.len())
            }
            _ => return None,
        });
    }
    Some(ConstantKey(tokens))
}

impl ConstantKey {
    pub(super) fn render(&self) -> PklString {
        let text = self.text();
        let mut units: Vec<_> = text.encode_utf16().collect();
        if units.len() >= 80 {
            units.truncate(77);
            units.extend("...".encode_utf16());
        }
        PklString::from_utf16(units)
    }

    fn text(&self) -> PklString {
        let mut text = crate::pkl_string::PklStringBuilder::default();
        let mut remaining = Vec::new();
        for token in &self.0 {
            if let Some(count) = remaining.last_mut() {
                *count -= 1;
            }
            match token {
                ConstantToken::Null => text.push_str("null"),
                ConstantToken::Bool(value) => text.push_str(&value.to_string()),
                ConstantToken::Int(value) => text.push_str(&value.to_string()),
                ConstantToken::Float(value) => {
                    text.push_str(&crate::numeric::double_to_string(f64::from_bits(*value)))
                }
                ConstantToken::String(value) => {
                    text.push_value(&value.quoted());
                }
                ConstantToken::List(count) => {
                    text.push_str("List(");
                    remaining.push(*count);
                    if *count != 0 {
                        continue;
                    }
                }
            }
            while remaining.last() == Some(&0) {
                remaining.pop();
                text.push(')');
            }
            if !remaining.is_empty() {
                text.push_str(", ");
            }
        }
        text.finish()
    }
}
