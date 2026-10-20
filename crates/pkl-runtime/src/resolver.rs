//! Static lexical binding for shared source syntax.
//!
//! Property lookup preserves both lexical owner depth and virtual receiver.
//! An absent lexical declaration resolves to the current receiver, rather than
//! searching enclosing objects dynamically. See Pkl 0.32 SymbolTable.resolveVariable.
use crate::diagnostic::{Diagnostic, DiagnosticKind};
use crate::source::Span;
use crate::syntax::{Expr, ExprKind, Member, Module, Parameter};
use std::collections::BTreeMap;

// Nonlocal properties/classes/type aliases in the pinned stdlib/base.pkl,
// following BaseModuleMembersGenerator. Base semantics are outside this prototype.
fn is_base_property(name: &str) -> bool {
    matches!(
        name,
        "AlsoKnownAs"
            | "Annotation"
            | "Any"
            | "BaseValueRenderer"
            | "Boolean"
            | "Bytes"
            | "BytesRenderer"
            | "Char"
            | "Charset"
            | "Class"
            | "Collection"
            | "Comparable"
            | "ConvertProperty"
            | "DataSize"
            | "DataSizeUnit"
            | "Deprecated"
            | "DocExample"
            | "Duration"
            | "DurationUnit"
            | "Dynamic"
            | "FileOutput"
            | "Float"
            | "Function"
            | "Function0"
            | "Function1"
            | "Function2"
            | "Function3"
            | "Function4"
            | "Function5"
            | "Infinity"
            | "Int"
            | "Int16"
            | "Int32"
            | "Int8"
            | "IntSeq"
            | "JsonRenderer"
            | "List"
            | "Listing"
            | "Map"
            | "Mapping"
            | "Mixin"
            | "Module"
            | "ModuleInfo"
            | "ModuleOutput"
            | "NaN"
            | "NonNull"
            | "Null"
            | "Number"
            | "Object"
            | "PListRenderer"
            | "Pair"
            | "PcfRenderDirective"
            | "PcfRenderer"
            | "PropertiesRenderer"
            | "Regex"
            | "RegexMatch"
            | "RenderDirective"
            | "Resource"
            | "Set"
            | "Since"
            | "SourceCode"
            | "String"
            | "TypeAlias"
            | "Typed"
            | "UInt"
            | "UInt16"
            | "UInt32"
            | "UInt8"
            | "Unlisted"
            | "Uri"
            | "ValueRenderer"
            | "VarArgs"
            | "YamlRenderer"
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Binding {
    /// Reverse index in the flat lexical environment; slot is currently 0.
    /// Parameters are appended in declaration order, and lets append one value.
    /// Object creation captures that environment without adding a let level.
    Lexical {
        depth: usize,
        slot: usize,
    },
    /// Nonlocal properties dispatch virtually on the receiver at owner_depth.
    /// Local properties read from the exact lexical owner instead.
    Property {
        owner_depth: usize,
        local: bool,
    },
    /// No lexical declaration: lookup only on the current receiver.
    ImplicitThis,
    /// Methods occupy a namespace separate from properties.
    Method {
        owner_depth: usize,
        local: bool,
    },
    ImplicitMethod,
}

#[derive(Clone, Debug)]
pub struct ResolvedModule {
    pub module: Module,
    pub names: BTreeMap<Span, Binding>,
}

fn is_method(member: &Member) -> bool {
    matches!(member.expr.kind, ExprKind::Function { method: true, .. })
}

enum Scope<'a> {
    Object(&'a [Member]),
    Let(&'a str),
    Parameters(&'a [Parameter]),
}

struct Resolver<'a> {
    scopes: Vec<Scope<'a>>,
    names: BTreeMap<Span, Binding>,
}

impl<'a> Resolver<'a> {
    fn binding(&self, name: &str, method: bool) -> Binding {
        let mut owner_depth = 0;
        let mut let_depth = 0;
        for scope in self.scopes.iter().rev() {
            match scope {
                Scope::Let(binding) => {
                    if !method && name != "_" && *binding == name {
                        return Binding::Lexical {
                            depth: let_depth,
                            slot: 0,
                        };
                    }
                    let_depth += 1;
                }
                Scope::Parameters(parameters) => {
                    if !method
                        && name != "_"
                        && let Some(index) = parameters
                            .iter()
                            .position(|parameter| parameter.name == name)
                    {
                        return Binding::Lexical {
                            depth: let_depth + parameters.len() - 1 - index,
                            slot: 0,
                        };
                    }
                    let_depth += parameters.len();
                }
                Scope::Object(members) => {
                    if let Some(member) = members
                        .iter()
                        .find(|member| member.name == name && is_method(member) == method)
                    {
                        return if method {
                            Binding::Method {
                                owner_depth,
                                local: member.local,
                            }
                        } else {
                            Binding::Property {
                                owner_depth,
                                local: member.local,
                            }
                        };
                    }
                    owner_depth += 1;
                }
            }
        }
        if method {
            Binding::ImplicitMethod
        } else {
            Binding::ImplicitThis
        }
    }

    fn members(&mut self, members: &'a [Member], object: bool) -> Result<(), Diagnostic> {
        let mut seen = std::collections::BTreeSet::new();
        for member in members {
            if !seen.insert((&member.name, is_method(member))) {
                return Err(Diagnostic::new(
                    DiagnosticKind::DuplicateMember,
                    member.span,
                    format!("M1 duplicate member `{}`.", member.name),
                ));
            }
        }
        self.scopes.push(Scope::Object(members));
        for member in members {
            if object && is_method(member) && !member.local {
                let ExprKind::Function { header_span, .. } = member.expr.kind else {
                    unreachable!()
                };
                return Err(Diagnostic::new(
                    DiagnosticKind::Syntax,
                    header_span,
                    "Method needs a `local` modifier because it is defined in an object, not a class.",
                ));
            }
            self.expression(&member.expr, true)?;
        }
        self.scopes.pop();
        Ok(())
    }

    fn expression(&mut self, expression: &'a Expr, contextual_new: bool) -> Result<(), Diagnostic> {
        match &expression.kind {
            ExprKind::Name(name) => {
                let binding = self.binding(name, false);
                // Base-module static resolution belongs to M2. Refuse the
                // supported spelling rather than silently treating it as this.
                if binding == Binding::ImplicitThis && is_base_property(name) {
                    return Err(Diagnostic::new(
                        DiagnosticKind::Unsupported,
                        expression.span,
                        "M1 base-module name resolution is unsupported.",
                    ));
                }
                self.names.insert(expression.span, binding);
            }
            ExprKind::Unary { operand, .. }
            | ExprKind::NonNull(operand)
            | ExprKind::Throw(operand)
            | ExprKind::TypeCheck { value: operand, .. }
            | ExprKind::TypeCast { value: operand, .. } => self.expression(operand, false)?,
            ExprKind::InterpolatedString(parts) => {
                for part in parts {
                    self.expression(part, false)?;
                }
            }
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                self.expression(condition, false)?;
                self.expression(then_branch, contextual_new)?;
                self.expression(else_branch, contextual_new)?;
            }
            ExprKind::Function {
                parameters,
                body,
                method,
                ..
            } => {
                if !method && parameters.len() > 5 {
                    return Err(Diagnostic::new(
                        DiagnosticKind::Syntax,
                        expression.span,
                        "Function literals can have at most five parameters.",
                    ));
                }
                let mut seen = std::collections::BTreeSet::new();
                for parameter in parameters {
                    if parameter.name != "_" && !seen.insert(&parameter.name) {
                        return Err(Diagnostic::new(
                            DiagnosticKind::DuplicateMember,
                            parameter.span,
                            format!("Duplicate parameter `{}`.", parameter.name),
                        ));
                    }
                }
                self.scopes.push(Scope::Parameters(parameters));
                self.expression(body, false)?;
                self.scopes.pop();
            }
            ExprKind::Call { callee, arguments } => {
                if let ExprKind::Name(name) = &callee.kind {
                    self.names.insert(callee.span, self.binding(name, true));
                } else {
                    self.expression(callee, false)?;
                }
                for argument in arguments {
                    self.expression(argument, false)?;
                }
            }
            ExprKind::Member(receiver, _) | ExprKind::NullableMember(receiver, _) => {
                self.expression(receiver, false)?
            }
            ExprKind::Binary { left, right, .. } => {
                self.expression(left, false)?;
                self.expression(right, false)?;
            }
            ExprKind::Let { name, value, body } => {
                self.expression(value, false)?;
                self.scopes.push(Scope::Let(name));
                self.expression(body, contextual_new)?;
                self.scopes.pop();
            }
            ExprKind::Object { parent, members } => {
                if parent.is_none() && !contextual_new {
                    return Err(Diagnostic::new(
                        DiagnosticKind::Unsupported,
                        expression.span,
                        "M1 contextual new requires a member RHS; use an explicit amendment parent.",
                    ));
                }
                if let Some(parent) = parent {
                    self.expression(parent, false)?;
                }
                self.members(members, true)?;
            }
            ExprKind::Int(_)
            | ExprKind::Float(_)
            | ExprKind::Null
            | ExprKind::String(_)
            | ExprKind::Bool(_)
            | ExprKind::This
            | ExprKind::Outer
            | ExprKind::Module
            | ExprKind::Super => {}
        }
        Ok(())
    }
}

pub fn resolve_module(module: &Module) -> Result<ResolvedModule, Diagnostic> {
    let mut resolver = Resolver {
        scopes: Vec::new(),
        names: BTreeMap::new(),
    };
    resolver.members(&module.members, false)?;
    Ok(ResolvedModule {
        module: module.clone(),
        names: resolver.names,
    })
}
