//! Static lexical binding for shared source syntax.
//!
//! Property lookup preserves both lexical owner depth and virtual receiver.
//! An absent lexical declaration resolves to the current receiver, rather than
//! searching enclosing objects dynamically. See Pkl 0.32 SymbolTable.resolveVariable.
use crate::diagnostic::{Diagnostic, DiagnosticKind};
use crate::source::Span;
use crate::syntax::{
    Expr, ExprKind, Member, Modifier, ModifierKind, Module, ModuleParentKind, Parameter, Type,
    TypeKind, TypeParameter, modifier_flags,
};
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

// Top-level nonlocal methods from the pinned stdlib/base.pkl.
fn is_base_method(name: &str) -> bool {
    matches!(
        name,
        "Regex"
            | "Undefined"
            | "TODO"
            | "Null"
            | "Pair"
            | "IntSeq"
            | "List"
            | "Set"
            | "Map"
            | "Bytes"
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
    /// Missing lexical/base name in an immediate type-constraint scope.
    CustomThis,
    CustomMethod,
}

#[derive(Clone, Debug)]
pub struct ResolvedModule {
    pub module: Module,
    pub names: BTreeMap<Span, Binding>,
    pub const_names: std::collections::BTreeSet<Span>,
    /// Direct custom-this reads; lambda bodies use their captured normal receiver.
    pub custom_this: std::collections::BTreeSet<Span>,
    /// Lambda literals that capture the constraint's input as their receiver.
    pub custom_functions: std::collections::BTreeSet<Span>,
}

fn is_method(member: &Member) -> bool {
    matches!(member.expr.kind, ExprKind::Function { method: true, .. })
}

enum Scope<'a> {
    Object(&'a [Member]),
    Let(&'a str),
    Parameters(&'a [Parameter]),
    TypeParameters(&'a [TypeParameter]),
    CustomThis,
}

#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
enum ConstLevel {
    None,
    Module,
    All,
}

struct Resolver<'a> {
    scopes: Vec<Scope<'a>>,
    names: BTreeMap<Span, Binding>,
    const_names: std::collections::BTreeSet<Span>,
    custom_this: std::collections::BTreeSet<Span>,
    custom_functions: std::collections::BTreeSet<Span>,
    const_level: ConstLevel,
    object_levels: Vec<ConstLevel>,
    receiver_is_lambda: bool,
    member_path: Vec<String>,
    diagnostic_path: Vec<String>,
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
                Scope::TypeParameters(_) | Scope::CustomThis => {}
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

    fn is_custom_this_scope(&self) -> bool {
        self.scopes
            .iter()
            .rev()
            .find(|scope| !matches!(scope, Scope::Let(_)))
            .is_some_and(|scope| matches!(scope, Scope::CustomThis))
    }

    fn members(&mut self, members: &'a [Member], object: bool) -> Result<(), Diagnostic> {
        let mut seen = std::collections::BTreeSet::new();
        for member in members {
            if !seen.insert((&member.name, is_method(member))) {
                return Err(Diagnostic::new(
                    DiagnosticKind::DuplicateMember,
                    member.name_span,
                    format!("Duplicate definition of member `{}`.", member.name),
                ));
            }
        }
        self.scopes.push(Scope::Object(members));
        self.object_levels.push(self.const_level);
        let previous_lambda = std::mem::replace(&mut self.receiver_is_lambda, false);
        for member in members {
            self.member_path.push(member.name.clone());
            self.diagnostic_path.push(member.name.clone());
            let previous_level = self.const_level;
            if modifier_flags(&member.modifiers) & ModifierKind::Const.flag() != 0 {
                self.const_level = ConstLevel::All;
            }
            self.validate_member(member, object)?;
            if let Some(ty) = &member.ty {
                self.type_expression(ty)?;
            }
            if object
                && is_method(member)
                && modifier_flags(&member.modifiers) & ModifierKind::Local.flag() == 0
            {
                let ExprKind::Function { header_span, .. } = member.expr.kind else {
                    unreachable!()
                };
                let mut error = Diagnostic::new(
                    DiagnosticKind::Syntax,
                    header_span,
                    "Method needs a `local` modifier because it is defined in an object, not a class.",
                );
                // The object builder rejects this declaration before entering
                // its method scope, so the enclosing authored member owns it.
                let enclosing = &self.diagnostic_path[..self.diagnostic_path.len() - 1];
                if !enclosing.is_empty() {
                    error.member = Some(enclosing.join(".").into());
                }
                return Err(error);
            }
            self.expression(&member.expr, true)?;
            self.const_level = previous_level;
            self.member_path.pop();
            self.diagnostic_path.pop();
        }
        self.receiver_is_lambda = previous_lambda;
        self.object_levels.pop();
        self.scopes.pop();
        Ok(())
    }

    fn reject_nonstdlib_type_parameters(
        &self,
        parameters: &[TypeParameter],
    ) -> Result<(), Diagnostic> {
        if let Some(parameter) = parameters.first() {
            return Err(Diagnostic::new(
                DiagnosticKind::Syntax,
                parameter.span,
                "Only standard library members can have type parameters.",
            ));
        }
        Ok(())
    }

    fn type_expression(&mut self, ty: &'a Type) -> Result<(), Diagnostic> {
        match &ty.kind {
            TypeKind::Named { path, arguments } => {
                let parameter = self.scopes.iter().rev().any(|scope| match scope {
                    Scope::TypeParameters(parameters) => {
                        parameters.iter().any(|parameter| parameter.name == path[0])
                    }
                    _ => false,
                });
                if path.len() > 2 {
                    return Err(Diagnostic::new(
                        DiagnosticKind::Syntax,
                        ty.span,
                        format!(
                            "Invalid type name `{}`.\n\nA qualified type name consists of a module import name and a type name.\nExample: `importedModule.SomeClass`",
                            path.join(".")
                        ),
                    ));
                }
                if !parameter {
                    // Declared types resolve in the enclosing module, not in
                    // value/parameter or nested object scopes. Type variables
                    // are handled separately above. See ResolveSimpleDeclaredTypeNode.
                    let objects = self
                        .scopes
                        .iter()
                        .filter_map(|scope| match scope {
                            Scope::Object(members) => Some(*members),
                            _ => None,
                        })
                        .collect::<Vec<_>>();
                    let binding = objects
                        .first()
                        .and_then(|members| {
                            members
                                .iter()
                                .find(|member| member.name == path[0] && !is_method(member))
                        })
                        .map_or(Binding::ImplicitThis, |member| Binding::Property {
                            owner_depth: objects.len() - 1,
                            local: member.local,
                        });
                    self.names.insert(ty.span, binding);
                }
                for argument in arguments {
                    self.type_expression(argument)?;
                }
            }
            TypeKind::Nullable(inner) => self.type_expression(inner)?,
            TypeKind::Union { types, .. } => {
                for ty in types {
                    self.type_expression(ty)?;
                }
            }
            TypeKind::Function {
                parameters,
                return_type,
            } => {
                for parameter in parameters {
                    self.type_expression(parameter)?;
                }
                self.type_expression(return_type)?;
            }
            TypeKind::Constrained { base, constraints } => {
                self.type_expression(base)?;
                self.scopes.push(Scope::CustomThis);
                for constraint in constraints {
                    self.expression(constraint, false)?;
                }
                self.scopes.pop();
            }
            TypeKind::Unknown | TypeKind::Nothing | TypeKind::Module | TypeKind::String(_) => {}
        }
        Ok(())
    }

    fn validate_member(&self, member: &Member, object: bool) -> Result<(), Diagnostic> {
        use ModifierKind as M;
        let (valid, target): (&[ModifierKind], &str) = if object {
            (&[M::Local, M::Const], "object members")
        } else {
            match member.expr.kind {
                ExprKind::Class { .. } => {
                    (&[M::Abstract, M::Open, M::Local, M::External], "classes")
                }
                ExprKind::TypeAlias { .. } => (&[M::Local, M::External], "type aliases"),
                ExprKind::Function { method: true, .. } => {
                    (&[M::Abstract, M::Local, M::External, M::Const], "methods")
                }
                _ => (
                    &[
                        M::Abstract,
                        M::Local,
                        M::Hidden,
                        M::External,
                        M::Fixed,
                        M::Const,
                    ],
                    "properties",
                ),
            }
        };
        validate_modifiers(&member.modifiers, valid, target)?;
        let flags = modifier_flags(&member.modifiers);
        let has = |kind: ModifierKind| flags & kind.flag() != 0;
        let modifier = |kind| {
            member
                .modifiers
                .iter()
                .find(|modifier| modifier.kind == kind)
        };
        if object && has(M::Const) && !has(M::Local) {
            let Some(const_modifier) = modifier(M::Const) else {
                return Err(Diagnostic::new(
                    DiagnosticKind::Unsupported,
                    member.modifiers[0].span,
                    "Derived const modifier diagnostics are not implemented.",
                ));
            };
            return Err(Diagnostic::new(
                DiagnosticKind::Syntax,
                const_modifier.span,
                "Modifier `const` can only be applied to object members that are also `local`.",
            ));
        }
        if object
            && !has(M::Local)
            && let Some(ty) = &member.ty
        {
            return Err(Diagnostic::new(
                DiagnosticKind::Syntax,
                ty.span,
                "A non-local object property cannot have a type annotation.",
            ));
        }
        if let ExprKind::Function {
            method: true,
            body,
            header_span,
            ..
        } = &member.expr.kind
            && matches!(body.kind, ExprKind::Uninitialized)
            && !has(M::Abstract)
            && !has(M::External)
        {
            let mut error = Diagnostic::new(
                DiagnosticKind::Syntax,
                *header_span,
                format!("Method `{}` is missing a body.", member.name),
            );
            error.member = Some(self.diagnostic_path.join(".").into());
            return Err(error);
        }
        if has(M::Abstract) {
            let has_body = match &member.expr.kind {
                ExprKind::Function { body, .. } => !matches!(body.kind, ExprKind::Uninitialized),
                _ => !matches!(
                    member.expr.kind,
                    ExprKind::Class { .. } | ExprKind::Uninitialized
                ),
            };
            if has_body {
                return Err(Diagnostic::new(
                    DiagnosticKind::Syntax,
                    member.span,
                    "Abstract members cannot have a body.",
                ));
            }
        }
        if has(M::Local) && matches!(member.expr.kind, ExprKind::Uninitialized) {
            return Err(Diagnostic::new(
                DiagnosticKind::Syntax,
                member.ty.as_ref().map_or(member.span, |ty| ty.span),
                "Missing property value.",
            ));
        }
        Ok(())
    }

    fn needs_const(&self, depth: usize) -> bool {
        match self.const_level {
            ConstLevel::None => false,
            ConstLevel::Module => depth + 1 == self.object_levels.len(),
            ConstLevel::All => {
                depth
                    >= self
                        .object_levels
                        .iter()
                        .rev()
                        .take_while(|level| **level == ConstLevel::All)
                        .count()
            }
        }
    }

    fn mark_const_binding(&mut self, span: Span, binding: Binding) {
        let needed = match binding {
            Binding::Property { owner_depth, .. } | Binding::Method { owner_depth, .. } => {
                self.needs_const(owner_depth)
            }
            Binding::ImplicitThis | Binding::ImplicitMethod => {
                self.const_level == ConstLevel::All && self.needs_const(0)
            }
            Binding::Lexical { .. } | Binding::CustomThis | Binding::CustomMethod => false,
        };
        if needed {
            self.const_names.insert(span);
        }
    }

    fn receiver_const(&self, receiver: &Expr) -> bool {
        let receiver = receiver.unparenthesized();
        match receiver.kind {
            ExprKind::Module => self.const_level != ConstLevel::None,
            ExprKind::This if self.is_custom_this_scope() => false,
            ExprKind::This | ExprKind::Super => {
                self.const_level == ConstLevel::All && self.needs_const(0)
            }
            ExprKind::Outer => self.needs_const(usize::from(!self.receiver_is_lambda)),
            _ => false,
        }
    }

    fn receiver(&mut self, receiver: &'a Expr) -> Result<(), Diagnostic> {
        let receiver = receiver.unparenthesized();
        if matches!(receiver.kind, ExprKind::This) && self.is_custom_this_scope() {
            self.custom_this.insert(receiver.span);
        }
        if matches!(
            receiver.kind,
            ExprKind::This | ExprKind::Module | ExprKind::Outer
        ) {
            return Ok(());
        }
        self.expression(receiver, false)
    }

    fn const_error(&self, expression: &Expr, message: impl Into<String>) -> Diagnostic {
        let mut error = Diagnostic::new(DiagnosticKind::Syntax, expression.span, message);
        if !self.diagnostic_path.is_empty() {
            error.member = Some(self.diagnostic_path.join(".").into());
        }
        error
    }

    fn expression(&mut self, expression: &'a Expr, contextual_new: bool) -> Result<(), Diagnostic> {
        match &expression.kind {
            // Parent inference walks through if/let, but not a grouping node.
            // See AstBuilder.doVisitNewExprWithInferredParent.
            ExprKind::Parenthesized(inner) => self.expression(inner, false)?,
            ExprKind::Name(name) => {
                let mut binding = self.binding(name, false);
                // Base-module static resolution belongs to M2. Refuse the
                // supported spelling rather than silently treating it as this.
                if binding == Binding::ImplicitThis && is_base_property(name) {
                    return Err(Diagnostic::new(
                        DiagnosticKind::Unsupported,
                        expression.span,
                        "M1 base-module name resolution is unsupported.",
                    ));
                }
                if binding == Binding::ImplicitThis && self.is_custom_this_scope() {
                    binding = Binding::CustomThis;
                }
                self.mark_const_binding(expression.span, binding);
                self.names.insert(expression.span, binding);
            }
            ExprKind::Unary { operand, .. }
            | ExprKind::NonNull(operand)
            | ExprKind::Throw(operand) => self.expression(operand, false)?,
            ExprKind::TypeCheck { value, ty } | ExprKind::TypeCast { value, ty } => {
                self.expression(value, false)?;
                self.type_expression(ty)?;
            }
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
                type_parameters,
                parameters,
                return_type,
                body,
                method,
                ..
            } => {
                self.reject_nonstdlib_type_parameters(type_parameters)?;
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
                if !method && self.is_custom_this_scope() {
                    self.custom_functions.insert(expression.span);
                }
                self.scopes.push(Scope::Parameters(parameters));
                let previous_lambda = std::mem::replace(&mut self.receiver_is_lambda, !method);
                for parameter in parameters {
                    if let Some(ty) = &parameter.ty {
                        self.type_expression(ty)?;
                    }
                }
                if let Some(ty) = return_type {
                    self.type_expression(ty)?;
                }
                self.expression(body, false)?;
                self.receiver_is_lambda = previous_lambda;
                self.scopes.pop();
            }
            ExprKind::Call { callee, arguments } => {
                if let ExprKind::Name(name) = &callee.kind {
                    let mut binding = self.binding(name, true);
                    if binding == Binding::ImplicitMethod
                        && self.is_custom_this_scope()
                        && name != "read"
                    {
                        if is_base_method(name) {
                            return Err(Diagnostic::new(
                                DiagnosticKind::Unsupported,
                                callee.span,
                                "Base-module method resolution is not implemented.",
                            ));
                        }
                        binding = Binding::CustomMethod;
                    }
                    self.mark_const_binding(callee.span, binding);
                    self.names.insert(callee.span, binding);
                } else {
                    self.expression(callee, false)?;
                }
                for argument in arguments {
                    self.expression(argument, false)?;
                }
            }
            ExprKind::Member(receiver, _) | ExprKind::NullableMember(receiver, _) => {
                if self.receiver_const(receiver) {
                    self.const_names.insert(expression.span);
                }
                self.receiver(receiver)?;
            }
            ExprKind::Binary { left, right, .. } => {
                self.expression(left, false)?;
                self.expression(right, false)?;
            }
            ExprKind::Let {
                name,
                ty,
                value,
                body,
            } => {
                if let Some(ty) = ty {
                    self.type_expression(ty)?;
                }
                self.expression(value, false)?;
                // Upstream flattens consecutive let scopes for display, but
                // keeps a new component when another scope intervenes.
                let nested_let = matches!(self.scopes.last(), Some(Scope::Let(_)));
                if !nested_let {
                    self.diagnostic_path.push("<let expr>".to_owned());
                }
                self.scopes.push(Scope::Let(name));
                self.expression(body, contextual_new)?;
                self.scopes.pop();
                if !nested_let {
                    self.diagnostic_path.pop();
                }
            }
            ExprKind::Class {
                type_parameters,
                superclass,
                members,
                ..
            } => {
                self.reject_nonstdlib_type_parameters(type_parameters)?;
                if let Some(ty) = superclass {
                    self.type_expression(ty)?;
                }
                let previous_level = self.const_level;
                self.const_level = self.const_level.max(ConstLevel::Module);
                self.members(members, false)?;
                self.const_level = previous_level;
            }
            ExprKind::TypeAlias {
                type_parameters,
                ty,
            } => {
                let mut seen = std::collections::BTreeSet::new();
                for parameter in type_parameters {
                    if !seen.insert(&parameter.name) {
                        return Err(Diagnostic::new(
                            DiagnosticKind::DuplicateMember,
                            parameter.span,
                            format!("Duplicate type parameter `{}`.", parameter.name),
                        ));
                    }
                }
                self.scopes.push(Scope::TypeParameters(type_parameters));
                self.type_expression(ty)?;
                self.scopes.pop();
            }
            ExprKind::New { ty, members } => {
                self.type_expression(ty)?;
                self.members(members, true)?;
            }
            ExprKind::Uninitialized => {}
            ExprKind::Object { parent, members } => {
                if parent.is_none() && !contextual_new {
                    let mut error = self
                        .const_error(expression, "Cannot tell which parent to amend.")
                        .with_hint("Please specify a parent explicitly, for example `new Person { ... }` instead of `new { ... }`.");
                    error.span.end = error.span.start + 3; // `new`, as upstream newSpan().
                    return Err(error);
                }
                if let Some(parent) = parent {
                    self.expression(parent, false)?;
                }
                self.members(members, true)?;
            }
            ExprKind::This => {
                if self.is_custom_this_scope() {
                    self.custom_this.insert(expression.span);
                } else if self.const_level == ConstLevel::All && self.needs_const(0) {
                    return Err(self.const_error(
                        expression,
                        "Cannot reference `this` from here because it is not `const`.",
                    ));
                }
            }
            ExprKind::Module => {
                if self.const_level != ConstLevel::None {
                    let message = if self.object_levels.contains(&ConstLevel::Module) {
                        "Cannot reference `module` within a class body.".to_owned()
                    } else {
                        format!(
                            "Cannot reference `module` from const property `{}`.",
                            self.member_path.last().map(String::as_str).unwrap_or("")
                        )
                    };
                    return Err(self.const_error(expression, message));
                }
            }
            ExprKind::Outer => {
                let target = self
                    .object_levels
                    .len()
                    .checked_sub(1 + usize::from(!self.receiver_is_lambda));
                if target.is_some_and(|index| self.const_level > self.object_levels[index]) {
                    return Err(self.const_error(
                        expression,
                        "Cannot reference `outer` from here because it's not `const`.",
                    ));
                }
            }
            ExprKind::Int(_)
            | ExprKind::Float(_)
            | ExprKind::Null
            | ExprKind::String(_)
            | ExprKind::Bool(_)
            | ExprKind::Super => {}
        }
        Ok(())
    }
}

fn validate_modifiers(
    modifiers: &[Modifier],
    valid: &[ModifierKind],
    target: &str,
) -> Result<(), Diagnostic> {
    use ModifierKind as M;
    for modifier in modifiers {
        if !valid.contains(&modifier.kind) {
            return Err(Diagnostic::new(
                DiagnosticKind::Syntax,
                modifier.span,
                format!(
                    "Modifier `{}` is not applicable to {target}.",
                    modifier.kind.spelling()
                ),
            ));
        }
    }
    let find = |kind| modifiers.iter().find(|modifier| modifier.kind == kind);
    let flags = modifier_flags(modifiers);
    if flags & M::External.flag() != 0 {
        let Some(modifier) = find(M::External) else {
            return Err(Diagnostic::new(
                DiagnosticKind::Unsupported,
                modifiers[0].span,
                "Derived external modifier semantics are not implemented.",
            ));
        };
        return Err(Diagnostic::new(
            DiagnosticKind::Unsupported,
            modifier.span,
            "External members can only be defined by standard library modules.",
        ));
    }
    for (first, second, message) in [
        (
            M::Local,
            M::Hidden,
            "Modifier `hidden` is redundant here; just use `local`.",
        ),
        (
            M::Local,
            M::Fixed,
            "Modifier `fixed` is redundant here; just use `local`.",
        ),
        (
            M::Abstract,
            M::Open,
            "Modifier `open` is redundant here; just use `abstract`.",
        ),
    ] {
        if flags & first.flag() != 0 && flags & second.flag() != 0 {
            let Some(modifier) = find(second) else {
                return Err(Diagnostic::new(
                    DiagnosticKind::Unsupported,
                    modifiers[0].span,
                    "Derived modifier diagnostics are not implemented.",
                ));
            };
            return Err(Diagnostic::new(
                DiagnosticKind::Syntax,
                modifier.span,
                message,
            ));
        }
    }
    Ok(())
}

pub fn resolve_module(module: &Module) -> Result<ResolvedModule, Diagnostic> {
    let mut resolver = Resolver {
        scopes: Vec::new(),
        names: BTreeMap::new(),
        const_names: std::collections::BTreeSet::new(),
        custom_this: std::collections::BTreeSet::new(),
        custom_functions: std::collections::BTreeSet::new(),
        const_level: ConstLevel::None,
        object_levels: Vec::new(),
        receiver_is_lambda: false,
        member_path: Vec::new(),
        diagnostic_path: Vec::new(),
    };
    let validation = (|| {
        if let Some(declaration) = &module.declaration {
            let amends = declaration
                .parent
                .as_ref()
                .is_some_and(|parent| parent.kind == ModuleParentKind::Amends);
            validate_modifiers(
                &declaration.modifiers,
                if amends {
                    &[]
                } else {
                    &[ModifierKind::Abstract, ModifierKind::Open]
                },
                if amends {
                    "modules that amend another module"
                } else {
                    "modules"
                },
            )?;
        }
        resolver.members(&module.members, false)?;
        Ok::<(), Diagnostic>(())
    })();
    validation.map_err(|mut error| {
        error.module_name = module
            .declaration
            .as_ref()
            .and_then(|declaration| declaration.name.as_ref())
            .map(|name| name.join(".").into_boxed_str());
        error
    })?;
    Ok(ResolvedModule {
        module: module.clone(),
        names: resolver.names,
        const_names: resolver.const_names,
        custom_this: resolver.custom_this,
        custom_functions: resolver.custom_functions,
    })
}
