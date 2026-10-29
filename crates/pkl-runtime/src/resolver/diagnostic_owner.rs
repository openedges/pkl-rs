//! Scope names for selected resolver diagnostics.
//!
//! Evaluation records owners during AST preparation; legacy standalone binding
//! requests a pure replay for selected diagnostics. The pinned SymbolTable
//! assigns child ordinals to scopes, not to rendered path strings. Compiled
//! lambda metadata reuses the AST-assigned owner despite bytecode visit order.
use crate::syntax::{Expr, ExprKind, UnaryOp};
use crate::value::PklString;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Other,
    Annotation,
    Lambda,
    Let,
    For,
}

struct Scope {
    name: PklString,
    kind: Kind,
    lambdas: usize,
    entries: usize,
}

impl Scope {
    fn new(name: PklString, kind: Kind) -> Self {
        Self {
            name,
            kind,
            lambdas: 0,
            entries: 0,
        }
    }
}

pub(super) struct OwnerScopes(Option<Vec<Scope>>);

impl OwnerScopes {
    pub(super) fn new(enabled: bool) -> Self {
        Self(enabled.then(|| vec![Scope::new(PklString::default(), Kind::Other)]))
    }

    pub(super) fn name(&self) -> Option<PklString> {
        self.0
            .as_ref()
            .map(|scopes| Self::qualified_name(scopes, scopes.len() - 1))
    }

    pub(super) fn enclosing_name(&self) -> Option<PklString> {
        self.0
            .as_ref()
            .map(|scopes| Self::qualified_name(scopes, scopes.len() - 2))
    }

    fn qualified_name(scopes: &[Scope], index: usize) -> PklString {
        let name = scopes[index].name.clone();
        // A module annotation enters AnnotationScope without a target name.
        // Java then joins its child properties with '.', not ModuleScope '#'.
        // Keep that explicit root separator in the structured suffix.
        if !name.is_empty()
            && scopes[..=index]
                .iter()
                .any(|scope| scope.kind == Kind::Annotation && scope.name.is_empty())
        {
            PklString::from(".").concat(&name)
        } else {
            name
        }
    }

    pub(super) fn enter_annotation(&mut self) {
        if let Some(scopes) = &mut self.0 {
            let name = scopes.last().unwrap().name.clone();
            scopes.push(Scope::new(name, Kind::Annotation));
        }
    }

    pub(super) fn is_enabled(&self) -> bool {
        self.0.is_some()
    }

    pub(super) fn enter_inherited(&mut self) {
        if let Some(scopes) = &mut self.0 {
            let name = scopes.last().unwrap().name.clone();
            scopes.push(Scope::new(name, Kind::Other));
        }
    }

    pub(super) fn enter_member(&mut self, name: &str) {
        if let Some(scopes) = &mut self.0 {
            let name = crate::diagnostic_names::quote_identifier(name);
            let parent = &scopes.last().unwrap().name;
            let name = if parent.is_empty() {
                name.into()
            } else {
                parent.concat(&format!(".{name}").into())
            };
            scopes.push(Scope::new(name, Kind::Other));
        }
    }

    pub(super) fn enter_lambda(&mut self) {
        if let Some(scopes) = &mut self.0 {
            // Java flattens consecutive lambdas and numbers them in their
            // nearest non-lambda parent. Object and let scopes break the chain.
            let parent = scopes
                .iter_mut()
                .rfind(|scope| scope.kind != Kind::Lambda)
                .unwrap();
            parent.lambdas += 1;
            let name = parent
                .name
                .concat(&format!(".<function#{}>", parent.lambdas).into());
            scopes.push(Scope::new(name, Kind::Lambda));
        }
    }

    pub(super) fn enter_let(&mut self) {
        if let Some(scopes) = &mut self.0 {
            let parent = scopes
                .iter()
                .rfind(|scope| scope.kind != Kind::Let)
                .unwrap();
            let name = parent.name.concat(&".<let expr>".into());
            scopes.push(Scope::new(name, Kind::Let));
        }
    }

    pub(super) fn enter_for(&mut self) {
        if let Some(scopes) = &mut self.0 {
            let name = scopes.last().unwrap().name.clone();
            scopes.push(Scope::new(name, Kind::For));
        }
    }

    pub(super) fn enter_entry(&mut self, key: Option<&Expr>) {
        if let Some(scopes) = &mut self.0 {
            let name = scopes.last().unwrap().name.clone();
            // ForGeneratorScope delegates entry ordinals through enclosing for
            // scopes to the containing object. Its lambda counter is separate.
            let parent = scopes
                .iter_mut()
                .rfind(|scope| scope.kind != Kind::For)
                .unwrap();
            let label = key.and_then(literal_label).unwrap_or_else(|| {
                parent.entries += 1;
                format!("#{}", parent.entries).into()
            });
            let name = name.concat(&"[".into()).concat(&label).concat(&"]".into());
            scopes.push(Scope::new(name, Kind::Other));
        }
    }

    pub(super) fn exit(&mut self) {
        if let Some(scopes) = &mut self.0 {
            assert!(scopes.len() > 1, "diagnostic owner root scope");
            scopes.pop();
        }
    }
}

fn literal_label(expression: &Expr) -> Option<PklString> {
    match &expression.unparenthesized().kind {
        ExprKind::String(value) => Some(PklString::from("\"").concat(value).concat(&"\"".into())),
        ExprKind::Int(value) => Some(value.to_string().into()),
        ExprKind::Float(value) => Some(crate::numeric::double_to_string(*value).into()),
        ExprKind::Bool(value) => Some(value.to_string().into()),
        ExprKind::Member(receiver, unit)
            if matches!(
                unit.as_str(),
                "ns" | "us"
                    | "ms"
                    | "s"
                    | "min"
                    | "h"
                    | "d"
                    | "b"
                    | "kb"
                    | "kib"
                    | "mb"
                    | "mib"
                    | "gb"
                    | "gib"
                    | "tb"
                    | "tib"
                    | "pb"
                    | "pib"
            ) =>
        {
            let number = match &receiver.unparenthesized().kind {
                ExprKind::Int(value) => *value as f64,
                ExprKind::Float(value) => *value,
                ExprKind::Unary {
                    op: UnaryOp::Negate,
                    operand,
                } => match operand.kind {
                    ExprKind::Float(value) => -value,
                    _ => return None,
                },
                _ => return None,
            };
            // VmDuration/VmDataSize render integral double values with Java's
            // saturating long cast, preserving the authored unit.
            let value = if number.is_finite() && number.fract() == 0.0 {
                (number as i64).to_string()
            } else {
                crate::numeric::double_to_string(number)
            };
            Some(format!("{value}.{unit}").into())
        }
        // Integer negatives are already folded by the parser. A remaining
        // Unary(Int) is another operation (e.g. --1), not a constant Java key.
        // Float negatives remain authored unary nodes with a direct operand.
        ExprKind::Unary {
            op: UnaryOp::Negate,
            operand,
        } => match operand.kind {
            ExprKind::Float(value) => Some(crate::numeric::double_to_string(-value).into()),
            _ => None,
        },
        _ => None,
    }
}
