//! Static lexical binding for the bounded M1 AST.
//!
//! Property lookup preserves both lexical owner depth and virtual receiver.
//! An absent lexical declaration resolves to the current receiver, rather than
//! searching enclosing objects dynamically. See Pkl 0.32 SymbolTable.resolveVariable.
use crate::diagnostic::{Diagnostic, DiagnosticKind};
use crate::source::Span;
use crate::syntax::{Expr, ExprKind, Member, Module};
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
    /// De Bruijn depth in the prototype's let environment; each let has slot 0.
    /// Object creation captures that environment without adding a let level.
    Lexical { depth: usize, slot: usize },
    /// Nonlocal properties dispatch virtually on the receiver at owner_depth.
    /// Local properties read from the exact lexical owner instead.
    Property { owner_depth: usize, local: bool },
    /// No lexical declaration: lookup only on the current receiver.
    ImplicitThis,
}

#[derive(Clone, Debug)]
pub struct ResolvedModule {
    pub module: Module,
    pub names: BTreeMap<Span, Binding>,
}

enum Scope<'a> {
    Object(&'a [Member]),
    Let(&'a str),
}

struct Resolver<'a> {
    scopes: Vec<Scope<'a>>,
    names: BTreeMap<Span, Binding>,
}

impl<'a> Resolver<'a> {
    fn binding(&self, name: &str) -> Binding {
        let mut owner_depth = 0;
        let mut let_depth = 0;
        for scope in self.scopes.iter().rev() {
            match scope {
                Scope::Let(binding) => {
                    if name != "_" && *binding == name {
                        return Binding::Lexical {
                            depth: let_depth,
                            slot: 0,
                        };
                    }
                    let_depth += 1;
                }
                Scope::Object(members) => {
                    if let Some(member) = members.iter().find(|member| member.name == name) {
                        return Binding::Property {
                            owner_depth,
                            local: member.local,
                        };
                    }
                    owner_depth += 1;
                }
            }
        }
        Binding::ImplicitThis
    }

    fn members(&mut self, members: &'a [Member]) -> Result<(), Diagnostic> {
        let mut seen = std::collections::BTreeSet::new();
        for member in members {
            if !seen.insert(&member.name) {
                return Err(Diagnostic::new(
                    DiagnosticKind::DuplicateMember,
                    member.span,
                    format!("M1 duplicate member `{}`.", member.name),
                ));
            }
        }
        self.scopes.push(Scope::Object(members));
        for member in members {
            self.expression(&member.expr, true)?;
        }
        self.scopes.pop();
        Ok(())
    }

    fn expression(&mut self, expression: &'a Expr, contextual_new: bool) -> Result<(), Diagnostic> {
        match &expression.kind {
            ExprKind::Name(name) => {
                let binding = self.binding(name);
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
            ExprKind::Member(receiver, _) => self.expression(receiver, false)?,
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
                self.members(members)?;
            }
            ExprKind::Int(_)
            | ExprKind::String(_)
            | ExprKind::Bool(_)
            | ExprKind::This
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
    resolver.members(&module.members)?;
    Ok(ResolvedModule {
        module: module.clone(),
        names: resolver.names,
    })
}
