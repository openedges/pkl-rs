//! Parse-only import discovery. It never compiles a Program or constructs a Vm.
use crate::{
    diagnostic::Diagnostic,
    source::{SourceText, Span},
    syntax::*,
    value::PklString,
};

#[derive(Clone, Debug)]
pub(super) struct Entry {
    pub(super) uri: PklString,
    pub(super) span: Span,
    pub(super) glob: bool,
    pub(super) parent_clause: bool,
}
enum Work<'a> {
    Expr(&'a Expr),
    Member(&'a Member),
    Type(&'a Type),
    Object(&'a ObjectMember),
    Parameter(&'a Parameter),
}

pub(super) fn imports(source: &SourceText) -> Result<Vec<Entry>, Diagnostic> {
    // Integer-range failures belong to executable AST building, not Java's
    // ImportsAndReadsParser. The syntax still must finish before any import I/O.
    let module = crate::parser::parse_module_for_evaluation(source.entire())
        .map_err(|failure| failure.diagnostic)?;
    let mut entries = Vec::new();
    let mut work = Vec::new();
    if let Some(declaration) = &module.declaration {
        work.extend(declaration.annotations.iter().map(Work::Expr));
        if let Some(parent) = &declaration.parent {
            entries.push(Entry {
                uri: parent.uri.clone(),
                span: parent.span,
                glob: false,
                parent_clause: true,
            });
        }
    }
    work.extend(module.members.iter().map(Work::Member));
    while let Some(next) = work.pop() {
        match next {
            Work::Member(member) => {
                work.extend(member.annotations.iter().map(Work::Expr));
                work.extend(member.ty.iter().map(Work::Type));
                work.push(Work::Expr(&member.expr));
            }
            Work::Parameter(parameter) => work.extend(parameter.ty.iter().map(Work::Type)),
            Work::Type(ty) => match &ty.kind {
                TypeKind::Named { arguments, .. } => work.extend(arguments.iter().map(Work::Type)),
                TypeKind::Nullable(inner) => work.push(Work::Type(inner)),
                TypeKind::Union { types, .. } => work.extend(types.iter().map(Work::Type)),
                TypeKind::Function {
                    parameters,
                    return_type,
                } => {
                    work.extend(parameters.iter().map(Work::Type));
                    work.push(Work::Type(return_type));
                }
                TypeKind::Constrained { base, constraints } => {
                    work.push(Work::Type(base));
                    work.extend(constraints.iter().map(Work::Expr));
                }
                _ => {}
            },
            Work::Object(member) => match member {
                ObjectMember::Property(member) => work.push(Work::Member(member)),
                ObjectMember::Element { value, .. } | ObjectMember::Spread { value, .. } => {
                    work.push(Work::Expr(value))
                }
                ObjectMember::Entry { key, value, .. } => {
                    work.extend([Work::Expr(key), Work::Expr(value)])
                }
                ObjectMember::Predicate {
                    condition, value, ..
                } => work.extend([Work::Expr(condition), Work::Expr(value)]),
                ObjectMember::When {
                    then_parameters,
                    else_parameters,
                    condition,
                    then_members,
                    else_members,
                    ..
                } => {
                    work.extend(
                        then_parameters
                            .iter()
                            .chain(else_parameters)
                            .map(Work::Parameter),
                    );
                    work.push(Work::Expr(condition));
                    work.extend(then_members.iter().chain(else_members).map(Work::Object));
                }
                ObjectMember::For {
                    parameters,
                    body_parameters,
                    iterable,
                    members,
                    ..
                } => {
                    work.extend(
                        parameters
                            .iter()
                            .chain(body_parameters)
                            .map(Work::Parameter),
                    );
                    work.push(Work::Expr(iterable));
                    work.extend(members.iter().map(Work::Object));
                }
            },
            Work::Expr(expr) => {
                match &expr.kind {
                    ExprKind::Import {
                        uri,
                        uri_span,
                        glob,
                        ..
                    } => entries.push(Entry {
                        uri: uri.clone(),
                        span: *uri_span,
                        glob: *glob,
                        parent_clause: false,
                    }),
                    // Java's specialized visitor returns directly for read
                    // expressions: even a nested import argument is not visited.
                    ExprKind::ReadResource(_)
                    | ExprKind::ReadResourceGlob(_)
                    | ExprKind::ReadResourceOrNull(_) => continue,
                    ExprKind::TypeCheck { ty, .. }
                    | ExprKind::TypeCast { ty, .. }
                    | ExprKind::TypeAlias { ty, .. } => work.push(Work::Type(ty)),
                    ExprKind::Let { ty, .. } => work.extend(ty.iter().map(Work::Type)),
                    ExprKind::Function {
                        parameters,
                        return_type,
                        ..
                    } => {
                        work.extend(parameters.iter().map(Work::Parameter));
                        work.extend(return_type.iter().map(Work::Type));
                    }
                    ExprKind::Class {
                        superclass,
                        members,
                        ..
                    } => {
                        work.extend(superclass.iter().map(Work::Type));
                        work.extend(members.iter().map(Work::Member));
                        continue;
                    }
                    ExprKind::Object {
                        parameters,
                        parent,
                        members,
                    } => {
                        work.extend(parameters.iter().map(Work::Parameter));
                        work.extend(parent.iter().map(|p| Work::Expr(p)));
                        work.extend(members.iter().map(Work::Object));
                        continue;
                    }
                    ExprKind::New {
                        parameters,
                        ty,
                        members,
                    }
                    | ExprKind::Annotation {
                        parameters,
                        ty,
                        members,
                    } => {
                        work.extend(parameters.iter().map(Work::Parameter));
                        work.push(Work::Type(ty));
                        work.extend(members.iter().map(Work::Object));
                        continue;
                    }
                    _ => {}
                }
                work.extend(expr.expression_children().into_iter().map(Work::Expr));
            }
        }
    }
    // Syntax stores some metadata separately from expression children. Restore
    // authored syntax visitation order; this is not graph/Host-result sorting.
    entries.sort_by_key(|entry| entry.span.start);
    Ok(entries)
}
