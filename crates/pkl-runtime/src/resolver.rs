//! Static lexical binding for shared source syntax.
//!
//! Property lookup preserves both lexical owner depth and virtual receiver.
//! An absent lexical declaration resolves to the current receiver, rather than
//! searching enclosing objects dynamically. See Pkl 0.32 SymbolTable.resolveVariable.
use crate::diagnostic::{Diagnostic, DiagnosticKind};
use crate::source::Span;
use crate::syntax::{
    Expr, ExprKind, Member, Modifier, ModifierKind, Module, ModuleParentKind, ObjectMember,
    Parameter, Type, TypeKind, TypeParameter, modifier_flags,
};
use std::collections::BTreeMap;
mod constant_keys;

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

fn is_base_class(name: &str) -> bool {
    matches!(
        name,
        "Any"
            | "Null"
            | "Boolean"
            | "Int"
            | "Float"
            | "Number"
            | "String"
            | "Bytes"
            | "Dynamic"
            | "Listing"
            | "Mapping"
            | "List"
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
    /// Pinned scalar constants, after lexical declarations have been checked.
    BaseConstant,
    /// A class exported by pkl:base, after lexical value declarations.
    BaseClass,
    /// A supported pkl:base type alias value, after lexical declarations.
    BaseTypeAlias,
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
    Object(Vec<&'a Member>),
    Let(&'a str),
    Parameters(&'a [Parameter]),
    GeneratorParameters(&'a [Parameter]),
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
    folded_integer_spans: &'a BTreeMap<Span, Span>,
    names: BTreeMap<Span, Binding>,
    const_names: std::collections::BTreeSet<Span>,
    custom_this: std::collections::BTreeSet<Span>,
    custom_functions: std::collections::BTreeSet<Span>,
    const_level: ConstLevel,
    object_levels: Vec<ConstLevel>,
    object_receiver_lambdas: Vec<bool>,
    receiver_is_lambda: bool,
    lambda_depth: usize,
    predicate_const_scopes: Vec<usize>,
    entry_counts: BTreeMap<String, usize>,
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
                Scope::Parameters(parameters) | Scope::GeneratorParameters(parameters) => {
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
            .find(|scope| !matches!(scope, Scope::Let(_) | Scope::GeneratorParameters(_)))
            .is_some_and(|scope| matches!(scope, Scope::CustomThis))
    }

    fn members(&mut self, members: &'a [Member], object: bool) -> Result<(), Diagnostic> {
        let mut seen = std::collections::BTreeSet::new();
        for (index, member) in members.iter().enumerate() {
            if !seen.insert((&member.name, is_method(member))) {
                let span = if members[..index].iter().any(|previous| {
                    previous.name == member.name
                        && matches!(previous.expr.kind, ExprKind::Import { .. })
                }) {
                    Span::new(
                        member.span.start,
                        member
                            .ty
                            .as_ref()
                            .map_or(member.name_span.end, |ty| ty.span.end),
                    )
                } else {
                    member.name_span
                };
                return Err(Diagnostic::new(
                    DiagnosticKind::DuplicateMember,
                    span,
                    format!("Duplicate definition of member `{}`.", member.name),
                ));
            }
        }
        self.scopes.push(Scope::Object(members.iter().collect()));
        self.object_levels.push(self.const_level);
        let previous_lambda = std::mem::replace(&mut self.receiver_is_lambda, false);
        self.object_receiver_lambdas.push(previous_lambda);
        for member in members {
            self.member(member, object)?;
        }
        self.receiver_is_lambda = previous_lambda;
        self.object_receiver_lambdas.pop();
        self.object_levels.pop();
        self.scopes.pop();
        Ok(())
    }

    fn member(&mut self, member: &'a Member, object: bool) -> Result<(), Diagnostic> {
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
        Ok(())
    }

    fn object_members(&mut self, members: &'a [ObjectMember]) -> Result<(), Diagnostic> {
        fn declarations<'b>(members: &'b [ObjectMember], names: &mut Vec<&'b Member>) {
            for member in members {
                match member {
                    ObjectMember::Property(member) => names.push(member),
                    ObjectMember::When {
                        then_members,
                        else_members,
                        ..
                    } => {
                        declarations(then_members, names);
                        declarations(else_members, names);
                    }
                    _ => {}
                }
            }
        }
        let mut names = Vec::new();
        declarations(members, &mut names);
        for pair in members.windows(2) {
            if pair[0].span().end == pair[1].span().start {
                let mut error = Diagnostic::new(
                    DiagnosticKind::Syntax,
                    pair[1].span(),
                    "Object members must be separated by whitespace, newline, or semicolon.\nObject entries must be separated by newline or semicolon.",
                );
                if !self.diagnostic_path.is_empty() {
                    error.member = Some(self.diagnostic_path.join(".").into());
                }
                return Err(error);
            }
        }
        if members.iter().all(|member| {
            matches!(
                member,
                ObjectMember::Property(_)
                    | ObjectMember::Element { .. }
                    | ObjectMember::Entry { .. }
            )
        }) {
            let mut seen = std::collections::BTreeSet::new();
            for member in &names {
                if !seen.insert((&member.name, is_method(member))) {
                    return Err(Diagnostic::new(
                        DiagnosticKind::DuplicateMember,
                        member.name_span,
                        format!("Duplicate definition of member `{}`.", member.name),
                    ));
                }
            }
        }
        self.scopes.push(Scope::Object(names));
        self.object_levels.push(self.const_level);
        let previous_lambda = std::mem::replace(&mut self.receiver_is_lambda, false);
        self.object_receiver_lambdas.push(previous_lambda);
        self.object_body(members, None)?;
        self.constant_entries(members)?;
        self.object_receiver_lambdas.pop();
        self.receiver_is_lambda = previous_lambda;
        self.object_levels.pop();
        self.scopes.pop();
        Ok(())
    }

    /// The pinned AST builder installs entries before evaluation only when the
    /// whole specialized body has constant keys. A generator or dynamic key
    /// leaves duplicate detection to the runtime.
    fn constant_entries(&self, members: &[ObjectMember]) -> Result<(), Diagnostic> {
        let mut keys = Vec::new();
        for member in members {
            match member {
                ObjectMember::Property(_) | ObjectMember::Element { .. } => {}
                ObjectMember::Entry { key, .. } => {
                    let Some(value) = constant_keys::resolve(key, &self.names) else {
                        return Ok(());
                    };
                    keys.push((key, value));
                }
                _ => return Ok(()),
            }
        }
        for (index, (key, value)) in keys.iter().enumerate() {
            if keys[..index].iter().any(|(_, previous)| previous == value) {
                let mut error = self.const_error(
                    key.unparenthesized(),
                    format!("Duplicate definition of member `{}`.", value.render()),
                );
                let key = key.unparenthesized();
                if let Some(span) = self.folded_integer_spans.get(&key.span) {
                    error.span = *span;
                } else if let ExprKind::Unary {
                    op: crate::syntax::UnaryOp::Negate,
                    operand,
                } = &key.kind
                    && matches!(operand.kind, ExprKind::Float(_))
                {
                    error.span = operand.span;
                }
                error.kind = DiagnosticKind::DuplicateMember;
                return Err(error);
            }
        }
        Ok(())
    }

    /// Generator headers execute in the enclosing receiver while retaining
    /// every active for binding. See pinned SymbolTable.EagerGeneratorScope.
    fn eager_expression(&mut self, expression: &'a Expr, custom: bool) -> Result<(), Diagnostic> {
        let index = self
            .scopes
            .iter()
            .rposition(|scope| matches!(scope, Scope::Object(_)))
            .expect("object generator scope");
        let scope = self.scopes.remove(index);
        let level = self.object_levels.pop().expect("object level");
        let lambda = std::mem::replace(
            &mut self.receiver_is_lambda,
            *self
                .object_receiver_lambdas
                .last()
                .expect("object receiver"),
        );
        // Eager lookup skips the target object, but Java's getConstDepth
        // still counts that object's const scope. A lambda adds a lexical
        // lookup level without adding a const barrier of its own.
        let const_barrier = custom && level == ConstLevel::All;
        if const_barrier {
            self.predicate_const_scopes.push(self.lambda_depth);
        }
        if custom {
            self.scopes.push(Scope::CustomThis);
        }
        let result = self.expression(expression, false);
        if custom {
            self.scopes.pop();
        }
        if const_barrier {
            self.predicate_const_scopes.pop();
        }
        self.receiver_is_lambda = lambda;
        self.object_levels.push(level);
        self.scopes.insert(index, scope);
        result
    }

    fn object_body(
        &mut self,
        members: &'a [ObjectMember],
        for_span: Option<Span>,
    ) -> Result<(), Diagnostic> {
        for member in members {
            match member {
                ObjectMember::Property(member) => {
                    if let Some(span) = for_span {
                        let message = if is_method(member) {
                            "A for-generator cannot generate object methods (only entries and elements)."
                        } else {
                            "A for-generator cannot generate object properties (only entries and elements)."
                        };
                        return Err(self.const_error(
                            &Expr {
                                kind: ExprKind::Uninitialized,
                                span: Span::new(span.start, span.start + 3),
                            },
                            message,
                        ));
                    }
                    self.member(member, true)?;
                }
                ObjectMember::Element { value, .. } => {
                    self.entry_label(None);
                    self.expression(value, true)?;
                }
                ObjectMember::Entry { key, value, .. } => {
                    self.eager_expression(key, false)?;
                    self.entry_label(Some(key));
                    self.expression(value, true)?;
                }
                ObjectMember::Spread { value, .. } => self.eager_expression(value, false)?,
                ObjectMember::Predicate {
                    condition, value, ..
                } => {
                    self.eager_expression(condition, true)?;
                    let label = self.entry_label(Some(condition));
                    let previous = self.diagnostic_path.pop().unwrap_or_default();
                    self.diagnostic_path.push(format!("{previous}[{label}]"));
                    self.expression(value, false)?;
                    self.diagnostic_path.pop();
                    if !previous.is_empty() {
                        self.diagnostic_path.push(previous);
                    }
                }
                ObjectMember::When {
                    condition,
                    then_members,
                    else_members,
                    ..
                } => {
                    // Upstream visits authored bodies before the eager predicate.
                    self.object_body(then_members, for_span)?;
                    self.object_body(else_members, for_span)?;
                    self.eager_expression(condition, false)?;
                }
                ObjectMember::For {
                    parameters,
                    iterable,
                    members,
                    span,
                } => {
                    let mut seen = std::collections::BTreeSet::new();
                    for parameter in parameters {
                        if parameter.name != "_" && !seen.insert(&parameter.name) {
                            let mut error = Diagnostic::new(
                                DiagnosticKind::DuplicateMember,
                                parameter.span,
                                format!("Duplicate definition of member `{}`.", parameter.name),
                            );
                            if !self.diagnostic_path.is_empty() {
                                error.member = Some(self.diagnostic_path.join(".").into());
                            }
                            return Err(error);
                        }
                        if let Some(ty) = &parameter.ty {
                            self.type_expression(ty)?;
                        }
                    }
                    self.eager_expression(iterable, false)?;
                    self.scopes.push(Scope::GeneratorParameters(parameters));
                    self.object_body(members, Some(*span))?;
                    self.scopes.pop();
                }
            }
        }
        Ok(())
    }

    fn entry_label(&mut self, key: Option<&Expr>) -> String {
        let literal = key.and_then(|key| match &key.kind {
            ExprKind::String(value) => Some(format!("\"{value}\"")),
            ExprKind::Int(value) => Some(value.to_string()),
            ExprKind::Float(value) => Some(crate::numeric::double_to_string(*value)),
            ExprKind::Bool(value) => Some(value.to_string()),
            _ => None,
        });
        literal.unwrap_or_else(|| {
            let count = self
                .entry_counts
                .entry(self.diagnostic_path.join("."))
                .or_default();
            *count += 1;
            format!("#{count}")
        })
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
            TypeKind::Named {
                path, arguments, ..
            } => {
                let parameter = path.len() == 1
                    && self.scopes.iter().rev().any(|scope| match scope {
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
                            Scope::Object(members) => Some(members),
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
                let object_depth = self
                    .object_levels
                    .iter()
                    .rev()
                    .take_while(|level| **level == ConstLevel::All)
                    .count();
                let lambda_depth = self
                    .predicate_const_scopes
                    .first()
                    .map_or(0, |depth| self.lambda_depth - depth);
                depth
                    >= (object_depth + self.predicate_const_scopes.len())
                        .saturating_sub(lambda_depth)
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
            Binding::Lexical { .. }
            | Binding::BaseConstant
            | Binding::BaseClass
            | Binding::BaseTypeAlias
            | Binding::CustomThis
            | Binding::CustomMethod => false,
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
                if binding == Binding::ImplicitThis && matches!(name.as_str(), "NaN" | "Infinity") {
                    binding = Binding::BaseConstant;
                }
                if binding == Binding::ImplicitThis && is_base_class(name) {
                    binding = Binding::BaseClass;
                }
                if binding == Binding::ImplicitThis && name == "NonNull" {
                    binding = Binding::BaseTypeAlias;
                }
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
                self.lambda_depth += usize::from(!method);
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
                self.lambda_depth -= usize::from(!method);
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
            ExprKind::Subscript { receiver, key } => {
                self.receiver(receiver)?;
                self.expression(key, false)?;
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
                self.object_members(members)?;
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
                self.object_members(members)?;
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
            ExprKind::Import { .. }
            | ExprKind::Int(_)
            | ExprKind::Float(_)
            | ExprKind::Null
            | ExprKind::String(_)
            | ExprKind::Bool(_)
            | ExprKind::Super
            | ExprKind::EntryParent => {}
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
        folded_integer_spans: &module.folded_integer_spans,
        names: BTreeMap::new(),
        const_names: std::collections::BTreeSet::new(),
        custom_this: std::collections::BTreeSet::new(),
        custom_functions: std::collections::BTreeSet::new(),
        const_level: ConstLevel::None,
        object_levels: Vec::new(),
        object_receiver_lambdas: Vec::new(),
        receiver_is_lambda: false,
        lambda_depth: 0,
        predicate_const_scopes: Vec::new(),
        entry_counts: BTreeMap::new(),
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
