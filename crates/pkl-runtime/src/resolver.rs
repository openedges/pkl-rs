//! Static lexical binding for shared source syntax.
//!
//! Property lookup preserves both lexical owner depth and virtual receiver.
//! An absent lexical declaration resolves to the current receiver, rather than
//! searching enclosing objects dynamically. See Pkl 0.32 SymbolTable.resolveVariable.
use crate::diagnostic::{Diagnostic, DiagnosticKind, DiagnosticOrigin};
use crate::source::Span;
use crate::syntax::{
    Expr, ExprKind, Member, Modifier, ModifierKind, Module, ModuleParentKind, ObjectMember,
    Parameter, Type, TypeKind, TypeParameter, modifier_flags,
};
use std::collections::BTreeMap;
mod constant_keys;
mod diagnostic_owner;

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
        "Annotation"
            | "Deprecated"
            | "Since"
            | "Unlisted"
            | "AlsoKnownAs"
            | "DocExample"
            | "SourceCode"
            | "Class"
            | "TypeAlias"
            | "Function"
            | "Function0"
            | "Function1"
            | "Function2"
            | "Function3"
            | "Function4"
            | "Function5"
            | "Any"
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
            | "Collection"
            | "List"
            | "Set"
            | "Map"
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
    Annotation(bool),
}

#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
enum ConstLevel {
    None,
    Module,
    All,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ConstScopeKind {
    Object,
    Method,
    Lambda,
    Let,
    For,
}

struct ConstScope {
    level: ConstLevel,
    kind: ConstScopeKind,
    // Eager predicates skip an object for lookup but retain its const barrier.
    lookup: bool,
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
    const_scopes: Vec<ConstScope>,
    entry_counts: BTreeMap<crate::value::PklString, usize>,
    member_path: Vec<String>,
    diagnostic_path: Vec<crate::value::PklString>,
    diagnostic_owners: diagnostic_owner::OwnerScopes,
    diagnostic_owner_requested: bool,
}

// Resolver continuations own temporary scope state, never authored AST nodes.
// Scope exits run before the next sibling; expression/type/member visitation is
// ordered exactly as the pinned builder, including generator body-first checks.
enum Work<'a> {
    Expression(&'a Expr, bool),
    Annotations(&'a [Expr], bool, ConstLevel),
    Annotation(&'a Expr, bool),
    AnnotationBody(&'a [ObjectMember], bool),
    EndAnnotations(ConstLevel),
    EndAnnotationBody(ConstLevel),
    Receiver(&'a Expr),
    Type(&'a Type),
    Constraints(&'a [Expr]),
    Members(&'a [Member], bool),
    Member(&'a Member, bool),
    MemberBody(&'a Member, bool),
    ObjectMembers(&'a [ObjectMember]),
    ObjectBody(&'a [ObjectMember], Option<Span>),
    ObjectMember(&'a ObjectMember, Option<Span>),
    ConstantEntries(&'a [ObjectMember]),
    Eager(&'a Expr, bool),
    EndEager {
        index: usize,
        scope: Scope<'a>,
        level: ConstLevel,
        lambda: bool,
        const_index: usize,
        custom: bool,
    },
    EntryValue(&'a Expr, &'a Expr),
    PredicateValue(&'a Expr, &'a Expr),
    EndPredicate(crate::value::PklString),
    ForHeader(&'a ObjectMember, usize, std::collections::BTreeSet<&'a str>),
    ForBody(&'a [Parameter], &'a [ObjectMember], Span),
    EndFor,
    LetBody(&'a str, &'a Expr, bool),
    EndLet(bool),
    ClassMembers(&'a [Member]),
    EndClass(ConstLevel),
    EndAlias(ConstLevel),
    EndFunction(bool, bool),
    EndMember(ConstLevel),
    EndObject(bool),
    PopScope,
    EndDiagnosticOwner,
    DiagnosticOwnerPrefix(&'a [ObjectMember]),
}

impl<'a> Resolver<'a> {
    fn binding(&self, name: &str, method: bool) -> Binding {
        let mut owner_depth: isize = 0;
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
                Scope::Annotation(class) => {
                    if *class {
                        owner_depth -= 1;
                    }
                }
                Scope::Object(members) => {
                    if let Some(member) = members
                        .iter()
                        .find(|member| member.name == name && is_method(member) == method)
                    {
                        return if method {
                            Binding::Method {
                                owner_depth: owner_depth.max(0) as usize,
                                local: member.local,
                            }
                        } else {
                            Binding::Property {
                                owner_depth: owner_depth.max(0) as usize,
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
        self.visit(Work::Members(members, object))
    }

    fn enter_object(&mut self, names: Vec<&'a Member>) -> bool {
        self.diagnostic_owners.enter_inherited();
        self.scopes.push(Scope::Object(names));
        self.object_levels.push(self.const_level);
        self.const_scopes.push(ConstScope {
            level: self.const_level,
            kind: ConstScopeKind::Object,
            lookup: true,
        });
        let previous = std::mem::replace(&mut self.receiver_is_lambda, false);
        self.object_receiver_lambdas.push(previous);
        previous
    }

    fn visit(&mut self, initial: Work<'a>) -> Result<(), Diagnostic> {
        // Errors abort this one-shot resolver. The queue then releases borrowed
        // scopes without recursively visiting or owning any authored subtrees.
        let mut work = vec![initial];
        while let Some(task) = work.pop() {
            match task {
                Work::Annotations(annotations, class, level) => {
                    let previous = std::mem::replace(&mut self.const_level, level);
                    work.push(Work::EndAnnotations(previous));
                    work.extend(
                        annotations
                            .iter()
                            .rev()
                            .map(|annotation| Work::Annotation(annotation, class)),
                    );
                }
                Work::Annotation(annotation, class) => {
                    let ExprKind::Annotation { ty, members, .. } = &annotation.kind else {
                        unreachable!("annotation syntax")
                    };
                    work.push(Work::AnnotationBody(members, class));
                    work.push(Work::Type(ty));
                }
                Work::AnnotationBody(members, class) => {
                    let previous = std::mem::replace(&mut self.const_level, ConstLevel::Module);
                    self.scopes.push(Scope::Annotation(class));
                    work.push(Work::EndAnnotationBody(previous));
                    work.push(Work::ObjectMembers(members));
                }
                Work::EndAnnotations(previous) => self.const_level = previous,
                Work::EndAnnotationBody(previous) => {
                    self.scopes.pop();
                    self.const_level = previous;
                }
                Work::EndMember(level) => {
                    self.diagnostic_owners.exit();
                    self.const_level = level;
                    self.member_path.pop();
                    self.diagnostic_path.pop();
                }
                Work::EndObject(lambda) => {
                    self.diagnostic_owners.exit();
                    self.receiver_is_lambda = lambda;
                    self.object_receiver_lambdas.pop();
                    self.object_levels.pop();
                    self.const_scopes.pop();
                    self.scopes.pop();
                }
                Work::EndFunction(lambda, owner_is_lambda) => {
                    if owner_is_lambda {
                        self.diagnostic_owners.exit();
                    }
                    self.receiver_is_lambda = lambda;
                    self.const_scopes.pop();
                    self.scopes.pop();
                }
                Work::EndClass(level) => self.const_level = level,
                Work::EndAlias(level) => {
                    self.const_level = level;
                    self.scopes.pop();
                }
                Work::PopScope => {
                    self.diagnostic_owners.exit();
                    self.scopes.pop();
                }
                Work::EndLet(nested) => {
                    self.diagnostic_owners.exit();
                    self.const_scopes.pop();
                    self.scopes.pop();
                    if !nested {
                        self.diagnostic_path.pop();
                    }
                }
                Work::EndFor => {
                    self.diagnostic_owners.exit();
                    self.const_scopes.pop();
                    self.scopes.pop();
                }
                Work::EndPredicate(previous) => {
                    self.diagnostic_path.pop();
                    if !previous.is_empty() {
                        self.diagnostic_path.push(previous);
                    }
                }
                Work::EndEager {
                    index,
                    scope,
                    level,
                    lambda,
                    const_index,
                    custom,
                } => {
                    self.diagnostic_owners.exit();
                    if custom {
                        self.diagnostic_owners.exit();
                        self.scopes.pop();
                    }
                    self.const_scopes[const_index].lookup = true;
                    self.receiver_is_lambda = lambda;
                    self.object_levels.push(level);
                    self.scopes.insert(index, scope);
                }
                Work::EndDiagnosticOwner => self.diagnostic_owners.exit(),
                Work::DiagnosticOwnerPrefix(members) => {
                    // AstBuilder's specialized object pass visits the prefix,
                    // then its generator fallback revisits it in the same scope.
                    // Only that scope's entry ordinals survive the first pass.
                    for member in members {
                        let key = match member {
                            ObjectMember::Element { .. } => None,
                            ObjectMember::Entry { key, .. } => Some(key),
                            _ => continue,
                        };
                        self.diagnostic_owners.enter_entry(key);
                        self.diagnostic_owners.exit();
                    }
                }
                Work::ConstantEntries(members) => self.constant_entries(members)?,
                Work::Members(members, object) => {
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
                            return Err(Diagnostic::language(
                                DiagnosticOrigin::Resolver,
                                DiagnosticKind::DuplicateMember,
                                span,
                                format!("Duplicate definition of member `{}`.", member.name),
                            ));
                        }
                    }
                    let previous = self.enter_object(members.iter().collect());
                    work.push(Work::EndObject(previous));
                    work.extend(
                        members
                            .iter()
                            .rev()
                            .map(|member| Work::Member(member, object)),
                    );
                }
                Work::Member(member, object) => {
                    self.diagnostic_owners.enter_member(&member.name);
                    self.member_path.push(member.name.clone());
                    self.diagnostic_path.push(member.name.clone().into());
                    let previous = self.const_level;
                    if modifier_flags(&member.modifiers) & ModifierKind::Const.flag() != 0 {
                        self.const_level = ConstLevel::All;
                    }
                    self.validate_member(member, object)?;
                    work.push(Work::EndMember(previous));
                    work.push(Work::MemberBody(member, object));
                    if let Some(ty) = &member.ty {
                        work.push(Work::Type(ty));
                    }
                    work.push(Work::Annotations(
                        &member.annotations,
                        !object && self.object_levels.len() > 1,
                        previous,
                    ));
                }
                Work::MemberBody(member, object) => {
                    if object
                        && is_method(member)
                        && modifier_flags(&member.modifiers) & ModifierKind::Local.flag() == 0
                    {
                        let ExprKind::Function { header_span, .. } = member.expr.kind else {
                            unreachable!()
                        };
                        let mut error = Diagnostic::language(
                            DiagnosticOrigin::Resolver,
                            DiagnosticKind::Syntax,
                            header_span,
                            "Method needs a `local` modifier because it is defined in an object, not a class.",
                        );
                        let enclosing = &self.diagnostic_path[..self.diagnostic_path.len() - 1];
                        if !enclosing.is_empty() {
                            error.member =
                                Some(crate::value::PklString::join(enclosing, ".").into());
                        }
                        return Err(error);
                    }
                    work.push(Work::Expression(&member.expr, true));
                }
                Work::ObjectMembers(members) => {
                    let mut declarations = members.iter().rev().collect::<Vec<_>>();
                    let mut names = Vec::new();
                    while let Some(member) = declarations.pop() {
                        match member {
                            ObjectMember::Property(member) => names.push(member),
                            ObjectMember::When {
                                then_members,
                                else_members,
                                ..
                            } => {
                                declarations.extend(else_members.iter().rev());
                                declarations.extend(then_members.iter().rev());
                            }
                            _ => {}
                        }
                    }
                    for pair in members.windows(2) {
                        if pair[0].span().end == pair[1].span().start {
                            let mut error = Diagnostic::language(
                                DiagnosticOrigin::Resolver,
                                DiagnosticKind::Syntax,
                                pair[1].span(),
                                "Object members must be separated by whitespace, newline, or semicolon.\nObject entries must be separated by newline or semicolon.",
                            );
                            if !self.diagnostic_path.is_empty() {
                                error.member = Some(
                                    crate::value::PklString::join(&self.diagnostic_path, ".")
                                        .into(),
                                );
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
                                let mut error = Diagnostic::language(
                                    DiagnosticOrigin::Resolver,
                                    DiagnosticKind::DuplicateMember,
                                    member.name_span,
                                    format!("Duplicate definition of member `{}`.", member.name),
                                );
                                if self
                                    .scopes
                                    .iter()
                                    .any(|scope| matches!(scope, Scope::Annotation(_)))
                                {
                                    error.member = Some(
                                        crate::value::PklString::join(&self.diagnostic_path, ".")
                                            .into(),
                                    );
                                }
                                return Err(error);
                            }
                        }
                    }
                    let previous = self.enter_object(names);
                    work.push(Work::EndObject(previous));
                    work.push(Work::ConstantEntries(members));
                    if self.diagnostic_owners.is_enabled()
                        && let Some(generator) = members.iter().position(|member| {
                            !matches!(
                                member,
                                ObjectMember::Property(_)
                                    | ObjectMember::Element { .. }
                                    | ObjectMember::Entry { .. }
                            )
                        })
                    {
                        for (index, member) in members.iter().enumerate().rev() {
                            work.push(Work::ObjectMember(member, None));
                            if index == generator {
                                work.push(Work::DiagnosticOwnerPrefix(&members[..generator]));
                            }
                        }
                    } else {
                        work.push(Work::ObjectBody(members, None));
                    }
                }
                Work::ObjectBody(members, for_span) => work.extend(
                    members
                        .iter()
                        .rev()
                        .map(|member| Work::ObjectMember(member, for_span)),
                ),
                Work::ObjectMember(member, for_span) => match member {
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
                        work.push(Work::Member(member, true));
                    }
                    ObjectMember::Element { value, .. } => {
                        self.entry_label(None);
                        if self.diagnostic_owners.is_enabled() {
                            self.diagnostic_owners.enter_entry(None);
                            work.push(Work::EndDiagnosticOwner);
                        }
                        work.push(Work::Expression(value, true));
                    }
                    ObjectMember::Entry { key, value, .. } => {
                        work.push(Work::EntryValue(key, value));
                        work.push(Work::Eager(key, false));
                    }
                    ObjectMember::Spread { value, .. } => work.push(Work::Eager(value, false)),
                    ObjectMember::Predicate {
                        condition, value, ..
                    } => {
                        work.push(Work::PredicateValue(condition, value));
                        work.push(Work::Eager(condition, true));
                    }
                    ObjectMember::When {
                        condition,
                        then_members,
                        else_members,
                        ..
                    } => {
                        work.push(Work::Eager(condition, false));
                        work.push(Work::ObjectBody(else_members, for_span));
                        work.push(Work::ObjectBody(then_members, for_span));
                    }
                    ObjectMember::For { .. } => work.push(Work::ForHeader(
                        member,
                        0,
                        std::collections::BTreeSet::new(),
                    )),
                },
                Work::EntryValue(key, value) => {
                    self.entry_label(Some(key));
                    if self.diagnostic_owners.is_enabled() {
                        self.diagnostic_owners.enter_entry(Some(key));
                        work.push(Work::EndDiagnosticOwner);
                    }
                    work.push(Work::Expression(value, true));
                }
                Work::PredicateValue(condition, value) => {
                    if self.diagnostic_owners.is_enabled() {
                        self.diagnostic_owners.enter_entry(Some(condition));
                        work.push(Work::EndDiagnosticOwner);
                    }
                    let label = self.entry_label(Some(condition));
                    let previous = self.diagnostic_path.pop().unwrap_or_default();
                    self.diagnostic_path.push(
                        previous
                            .concat(&"[".into())
                            .concat(&label)
                            .concat(&"]".into()),
                    );
                    work.push(Work::EndPredicate(previous));
                    work.push(Work::Expression(value, false));
                }
                Work::ForHeader(member, index, mut seen) => {
                    let ObjectMember::For {
                        parameters,
                        iterable,
                        members,
                        span,
                    } = member
                    else {
                        unreachable!()
                    };
                    if let Some(parameter) = parameters.get(index) {
                        if parameter.name != "_" && !seen.insert(&parameter.name) {
                            let mut error = Diagnostic::language(
                                DiagnosticOrigin::Resolver,
                                DiagnosticKind::DuplicateMember,
                                parameter.span,
                                format!("Duplicate definition of member `{}`.", parameter.name),
                            );
                            if !self.diagnostic_path.is_empty() {
                                error.member = Some(
                                    crate::value::PklString::join(&self.diagnostic_path, ".")
                                        .into(),
                                );
                            }
                            return Err(error);
                        }
                        work.push(Work::ForHeader(member, index + 1, seen));
                        if let Some(ty) = &parameter.ty {
                            work.push(Work::Type(ty));
                        }
                    } else {
                        work.push(Work::ForBody(parameters, members, *span));
                        work.push(Work::Eager(iterable, false));
                    }
                }
                Work::ForBody(parameters, members, span) => {
                    self.diagnostic_owners.enter_for();
                    self.scopes.push(Scope::GeneratorParameters(parameters));
                    self.const_scopes.push(ConstScope {
                        level: self.const_level,
                        kind: ConstScopeKind::For,
                        lookup: true,
                    });
                    work.push(Work::EndFor);
                    work.push(Work::ObjectBody(members, Some(span)));
                }
                Work::Eager(expression, custom) => {
                    self.diagnostic_owners.enter_inherited();
                    if custom {
                        self.diagnostic_owners.enter_inherited();
                    }
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
                    let const_index = self
                        .const_scopes
                        .iter()
                        .rposition(|scope| scope.kind == ConstScopeKind::Object && scope.lookup)
                        .expect("object const scope");
                    self.const_scopes[const_index].lookup = false;
                    if custom {
                        self.scopes.push(Scope::CustomThis);
                    }
                    work.push(Work::EndEager {
                        index,
                        scope,
                        level,
                        lambda,
                        const_index,
                        custom,
                    });
                    work.push(Work::Expression(expression, false));
                }
                Work::LetBody(name, body, contextual_new) => {
                    self.diagnostic_owners.enter_let();
                    let nested = matches!(self.scopes.last(), Some(Scope::Let(_)));
                    if !nested {
                        self.diagnostic_path.push("<let expr>".into());
                    }
                    self.scopes.push(Scope::Let(name));
                    self.const_scopes.push(ConstScope {
                        level: self.const_level,
                        kind: ConstScopeKind::Let,
                        lookup: true,
                    });
                    work.push(Work::EndLet(nested));
                    work.push(Work::Expression(body, contextual_new));
                }
                Work::ClassMembers(members) => {
                    let previous = self.const_level;
                    self.const_level = self.const_level.max(ConstLevel::Module);
                    work.push(Work::EndClass(previous));
                    work.push(Work::Members(members, false));
                }
                Work::Constraints(constraints) => {
                    self.diagnostic_owners.enter_inherited();
                    self.scopes.push(Scope::CustomThis);
                    work.push(Work::PopScope);
                    work.extend(
                        constraints
                            .iter()
                            .rev()
                            .map(|expression| Work::Expression(expression, false)),
                    );
                }
                Work::Type(ty) => match &ty.kind {
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
                            return Err(Diagnostic::language(
                                DiagnosticOrigin::Resolver,
                                DiagnosticKind::Syntax,
                                ty.span,
                                format!(
                                    "Invalid type name `{}`.\n\nA qualified type name consists of a module import name and a type name.\nExample: `importedModule.SomeClass`",
                                    path.join(".")
                                ),
                            ));
                        }
                        if !parameter {
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
                        work.extend(arguments.iter().rev().map(Work::Type));
                    }
                    TypeKind::Nullable(inner) => work.push(Work::Type(inner)),
                    TypeKind::Union { types, .. } => {
                        work.extend(types.iter().rev().map(Work::Type))
                    }
                    TypeKind::Function {
                        parameters,
                        return_type,
                    } => {
                        work.push(Work::Type(return_type));
                        work.extend(parameters.iter().rev().map(Work::Type));
                    }
                    TypeKind::Constrained { base, constraints } => {
                        work.push(Work::Constraints(constraints));
                        work.push(Work::Type(base));
                    }
                    TypeKind::Unknown
                    | TypeKind::Nothing
                    | TypeKind::Module
                    | TypeKind::String(_) => {}
                },
                Work::Receiver(receiver) => {
                    let receiver = receiver.unparenthesized();
                    if matches!(receiver.kind, ExprKind::This) && self.is_custom_this_scope() {
                        self.custom_this.insert(receiver.span);
                    }
                    if !matches!(
                        receiver.kind,
                        ExprKind::This | ExprKind::Module | ExprKind::Outer
                    ) {
                        work.push(Work::Expression(receiver, false));
                    }
                }
                Work::Expression(expression, contextual_new) => match &expression.kind {
                    ExprKind::Trace(value) => {
                        work.push(Work::Expression(value, contextual_new));
                    }
                    ExprKind::Parenthesized(value)
                    | ExprKind::NonNull(value)
                    | ExprKind::Throw(value)
                    | ExprKind::ReadResource(value)
                    | ExprKind::ReadResourceOrNull(value)
                    | ExprKind::Unary { operand: value, .. } => {
                        work.push(Work::Expression(value, false))
                    }
                    ExprKind::TypeCheck { value, ty } | ExprKind::TypeCast { value, ty } => {
                        work.push(Work::Type(ty));
                        work.push(Work::Expression(value, false));
                    }
                    ExprKind::InterpolatedString(parts) => {
                        work.extend(parts.iter().rev().map(|part| Work::Expression(part, false)))
                    }
                    ExprKind::Call { callee, arguments } => {
                        work.extend(
                            arguments
                                .iter()
                                .rev()
                                .map(|argument| Work::Expression(argument, false)),
                        );
                        if let ExprKind::Name(name) = &callee.kind {
                            let mut binding = self.binding(name, true);
                            if binding == Binding::ImplicitMethod && self.is_custom_this_scope() {
                                if is_base_method(name) {
                                    return Err(Diagnostic::unsupported(
                                        DiagnosticOrigin::Resolver,
                                        "custom-this-base-method",
                                        callee.span,
                                        "Base-module method resolution is not implemented.",
                                    ));
                                }
                                binding = Binding::CustomMethod;
                            }
                            self.mark_const_binding(callee.span, binding);
                            self.names.insert(callee.span, binding);
                        } else {
                            work.push(Work::Expression(callee, false));
                        }
                    }
                    ExprKind::Member(receiver, _) | ExprKind::NullableMember(receiver, _) => {
                        if self.receiver_const(receiver) {
                            self.const_names.insert(expression.span);
                        }
                        work.push(Work::Receiver(receiver));
                    }
                    ExprKind::Subscript { receiver, key } => {
                        work.push(Work::Expression(key, false));
                        work.push(Work::Receiver(receiver));
                    }
                    ExprKind::Binary { left, right, .. } => {
                        work.push(Work::Expression(right, false));
                        work.push(Work::Expression(left, false));
                    }
                    ExprKind::If {
                        condition,
                        then_branch,
                        else_branch,
                    } => {
                        work.push(Work::Expression(else_branch, contextual_new));
                        work.push(Work::Expression(then_branch, contextual_new));
                        work.push(Work::Expression(condition, contextual_new));
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
                            return Err(Diagnostic::language(
                                DiagnosticOrigin::Resolver,
                                DiagnosticKind::Syntax,
                                expression.span,
                                "Function literals can have at most five parameters.",
                            ));
                        }
                        let mut seen = std::collections::BTreeSet::new();
                        for parameter in parameters {
                            if parameter.name != "_" && !seen.insert(&parameter.name) {
                                return Err(Diagnostic::language(
                                    DiagnosticOrigin::Resolver,
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
                        self.const_scopes.push(ConstScope {
                            level: self.const_level,
                            kind: if *method {
                                ConstScopeKind::Method
                            } else {
                                ConstScopeKind::Lambda
                            },
                            lookup: true,
                        });
                        let previous = std::mem::replace(&mut self.receiver_is_lambda, !method);
                        if !method {
                            self.diagnostic_owners.enter_lambda();
                        }
                        work.push(Work::EndFunction(previous, !method));
                        // A method body can infer its parent from the return type;
                        // a lambda body is not an inference context.
                        work.push(Work::Expression(body, *method));
                        if let Some(ty) = return_type {
                            work.push(Work::Type(ty));
                        }
                        work.extend(
                            parameters
                                .iter()
                                .rev()
                                .filter_map(|parameter| parameter.ty.as_ref().map(Work::Type)),
                        );
                    }
                    ExprKind::Let {
                        name,
                        ty,
                        value,
                        body,
                    } => {
                        work.push(Work::LetBody(name, body, contextual_new));
                        work.push(Work::Expression(value, false));
                        if let Some(ty) = ty {
                            work.push(Work::Type(ty));
                        }
                    }
                    ExprKind::Class {
                        type_parameters,
                        superclass,
                        members,
                        ..
                    } => {
                        self.reject_nonstdlib_type_parameters(type_parameters)?;
                        work.push(Work::ClassMembers(members));
                        if let Some(ty) = superclass {
                            work.push(Work::Type(ty));
                        }
                    }
                    ExprKind::TypeAlias {
                        type_parameters,
                        ty,
                    } => {
                        let mut seen = std::collections::BTreeSet::new();
                        for parameter in type_parameters {
                            if !seen.insert(&parameter.name) {
                                return Err(Diagnostic::language(
                                    DiagnosticOrigin::Resolver,
                                    DiagnosticKind::DuplicateMember,
                                    parameter.span,
                                    format!("Duplicate type parameter `{}`.", parameter.name),
                                ));
                            }
                        }
                        self.scopes.push(Scope::TypeParameters(type_parameters));
                        let previous = self.const_level;
                        self.const_level = self.const_level.max(ConstLevel::Module);
                        work.push(Work::EndAlias(previous));
                        work.push(Work::Type(ty));
                    }
                    ExprKind::New { ty, members } | ExprKind::Annotation { ty, members, .. } => {
                        work.push(Work::ObjectMembers(members));
                        work.push(Work::Type(ty));
                    }
                    ExprKind::Object { parent, members } => {
                        if parent.is_none() && !contextual_new {
                            self.diagnostic_owner_requested = true;
                            let mut error = self.const_error(expression, "Cannot tell which parent to amend.").with_hint("Please specify a parent explicitly, for example `new Person { ... }` instead of `new { ... }`.");
                            if let Some(owner) = self.diagnostic_owners.name() {
                                error.member = Some(owner.into());
                            }
                            error.span.end = error.span.start + 3;
                            return Err(error);
                        }
                        work.push(Work::ObjectMembers(members));
                        if let Some(parent) = parent {
                            work.push(Work::Expression(parent, false));
                        }
                    }
                    _ => self.leaf_expression(expression)?,
                },
            }
        }
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

    fn entry_label(&mut self, key: Option<&Expr>) -> crate::value::PklString {
        let literal = key.and_then(|key| match &key.kind {
            ExprKind::String(value) => Some(
                crate::value::PklString::from("\"")
                    .concat(value)
                    .concat(&"\"".into()),
            ),
            ExprKind::Int(value) => Some(value.to_string().into()),
            ExprKind::Float(value) => Some(crate::numeric::double_to_string(*value).into()),
            ExprKind::Bool(value) => Some(value.to_string().into()),
            _ => None,
        });
        literal.unwrap_or_else(|| {
            let count = self
                .entry_counts
                .entry(crate::value::PklString::join(&self.diagnostic_path, "."))
                .or_default();
            *count += 1;
            format!("#{count}").into()
        })
    }

    fn reject_nonstdlib_type_parameters(
        &self,
        parameters: &[TypeParameter],
    ) -> Result<(), Diagnostic> {
        if let Some(parameter) = parameters.first() {
            return Err(Diagnostic::language(
                DiagnosticOrigin::Resolver,
                DiagnosticKind::Syntax,
                parameter.span,
                "Only standard library members can have type parameters.",
            ));
        }
        Ok(())
    }

    fn validate_member(&mut self, member: &Member, object: bool) -> Result<(), Diagnostic> {
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
        validate_modifiers(&member.modifiers, valid, target).map_err(|mut error| {
            // The pinned builder checks modifiers before entering the member's
            // declaration scope, so the enclosing class or property owns it.
            let enclosing = &self.diagnostic_path[..self.diagnostic_path.len() - 1];
            if !enclosing.is_empty() {
                error.member = Some(crate::value::PklString::join(enclosing, ".").into());
            }
            error
        })?;
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
                return Err(Diagnostic::unsupported(
                    DiagnosticOrigin::Resolver,
                    "derived-const-diagnostic",
                    member.modifiers[0].span,
                    "Derived const modifier diagnostics are not implemented.",
                ));
            };
            self.diagnostic_owner_requested = true;
            let mut error = Diagnostic::language(
                DiagnosticOrigin::Resolver,
                DiagnosticKind::Syntax,
                const_modifier.span,
                "Modifier `const` can only be applied to object members that are also `local`.",
            );
            // AstBuilder rejects this modifier before entering the member scope.
            error.member = self.diagnostic_owners.enclosing_name().map(Into::into);
            return Err(error);
        }
        if object
            && !has(M::Local)
            && let Some(ty) = &member.ty
        {
            self.diagnostic_owner_requested = true;
            let mut error = Diagnostic::language(
                DiagnosticOrigin::Resolver,
                DiagnosticKind::Syntax,
                ty.span,
                "A non-local object property cannot have a type annotation.",
            );
            // AstBuilder enters this property scope before rejecting its type.
            error.member = self.diagnostic_owners.name().map(Into::into);
            return Err(error);
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
            let mut error = Diagnostic::language(
                DiagnosticOrigin::Resolver,
                DiagnosticKind::Syntax,
                *header_span,
                format!("Method `{}` is missing a body.", member.name),
            );
            error.member = Some(crate::value::PklString::join(&self.diagnostic_path, ".").into());
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
            if has_body && !member.is_amendment() {
                let header_span = match &member.expr.kind {
                    ExprKind::Function { header_span, .. } => *header_span,
                    _ => Span::new(
                        member.span.start,
                        member
                            .ty
                            .as_ref()
                            .map_or(member.name_span.end, |ty| ty.span.end),
                    ),
                };
                let mut error = Diagnostic::language(
                    DiagnosticOrigin::Resolver,
                    DiagnosticKind::Syntax,
                    header_span,
                    "Abstract members cannot have a body.",
                );
                error.member =
                    Some(crate::value::PklString::join(&self.diagnostic_path, ".").into());
                return Err(error);
            }
        }
        if has(M::Local) && matches!(member.expr.kind, ExprKind::Uninitialized) {
            return Err(Diagnostic::language(
                DiagnosticOrigin::Resolver,
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
            ConstLevel::Module => {
                depth + 1
                    == self.object_levels.len()
                        - self
                            .scopes
                            .iter()
                            .filter(|scope| matches!(scope, Scope::Annotation(true)))
                            .count()
            }
            ConstLevel::All => {
                let mut remaining = depth;
                let mut levels_up = 0;
                for scope in self.const_scopes.iter().rev().filter(|scope| scope.lookup) {
                    match scope.kind {
                        ConstScopeKind::Object if remaining == 0 => break,
                        ConstScopeKind::Object => {
                            remaining -= 1;
                            levels_up += 1;
                        }
                        ConstScopeKind::Lambda => levels_up += 1,
                        ConstScopeKind::Method | ConstScopeKind::Let | ConstScopeKind::For => {}
                    }
                }
                levels_up >= self.const_scope_count()
            }
        }
    }

    fn const_scope_count(&self) -> usize {
        // Methods, lets and for scopes create const barriers without increasing
        // lookup depth; lambdas increase lookup depth without adding barriers.
        self.const_scopes
            .iter()
            .rev()
            .take_while(|scope| scope.level == ConstLevel::All)
            .filter(|scope| scope.kind != ConstScopeKind::Lambda)
            .count()
    }

    fn mark_const_binding(&mut self, span: Span, binding: Binding) {
        let needed = match binding {
            Binding::Property { owner_depth, .. } | Binding::Method { owner_depth, .. } => {
                self.needs_const(owner_depth)
            }
            Binding::ImplicitThis | Binding::ImplicitMethod => {
                self.const_level == ConstLevel::All && self.const_scope_count() == 0
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
                self.const_level == ConstLevel::All && self.const_scope_count() == 0
            }
            ExprKind::Outer => {
                self.parent_const_scope()
                    .is_some_and(|(index, scope)| match self.const_level {
                        ConstLevel::None => false,
                        ConstLevel::Module => index == 0,
                        ConstLevel::All => scope.level != ConstLevel::All,
                    })
            }
            _ => false,
        }
    }

    fn parent_const_scope(&self) -> Option<(usize, &ConstScope)> {
        // `outer` uses the immediately enclosing lexical scope, including
        // methods and lambdas, rather than the property's lookup distance.
        let index = self.const_scopes.len().checked_sub(2)?;
        Some((index, &self.const_scopes[index]))
    }

    fn const_error(&self, expression: &Expr, message: impl Into<String>) -> Diagnostic {
        let mut error = Diagnostic::language(
            DiagnosticOrigin::Resolver,
            DiagnosticKind::Syntax,
            expression.span,
            message,
        );
        if !self.diagnostic_path.is_empty() {
            error.member = Some(crate::value::PklString::join(&self.diagnostic_path, ".").into());
        }
        error
    }

    fn leaf_expression(&mut self, expression: &'a Expr) -> Result<(), Diagnostic> {
        match &expression.kind {
            ExprKind::Name(name) => {
                let mut binding = self.binding(name, false);
                if binding == Binding::ImplicitThis && matches!(name.as_str(), "NaN" | "Infinity") {
                    binding = Binding::BaseConstant;
                }
                if binding == Binding::ImplicitThis && is_base_class(name) {
                    binding = Binding::BaseClass;
                }
                if binding == Binding::ImplicitThis
                    && matches!(
                        name.as_str(),
                        "NonNull"
                            | "Int8"
                            | "Int16"
                            | "Int32"
                            | "UInt8"
                            | "UInt16"
                            | "UInt32"
                            | "UInt"
                    )
                {
                    binding = Binding::BaseTypeAlias;
                }
                // Base-module static resolution belongs to M2. Refuse the
                // supported spelling rather than silently treating it as this.
                if binding == Binding::ImplicitThis && is_base_property(name) {
                    return Err(Diagnostic::unsupported(
                        DiagnosticOrigin::Resolver,
                        "base-name-resolution",
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
            ExprKind::This => {
                if self.is_custom_this_scope() {
                    self.custom_this.insert(expression.span);
                } else if self.const_level == ConstLevel::All && self.const_scope_count() == 0 {
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
                if self
                    .parent_const_scope()
                    .is_some_and(|(_, scope)| self.const_level > scope.level)
                {
                    return Err(self.const_error(
                        expression,
                        "Cannot reference `outer` from here because it's not `const`.",
                    ));
                }
            }
            ExprKind::Uninitialized
            | ExprKind::Import { .. }
            | ExprKind::Int(_)
            | ExprKind::Float(_)
            | ExprKind::Null
            | ExprKind::String(_)
            | ExprKind::Bool(_)
            | ExprKind::Super
            | ExprKind::EntryParent => {}
            _ => unreachable!("traversing expressions use resolver continuations"),
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
            return Err(Diagnostic::language(
                DiagnosticOrigin::Resolver,
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
            return Err(Diagnostic::unsupported(
                DiagnosticOrigin::Resolver,
                "derived-external-modifier",
                modifiers[0].span,
                "Derived external modifier semantics are not implemented.",
            ));
        };
        return Err(Diagnostic::language(
            DiagnosticOrigin::Resolver,
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
                return Err(Diagnostic::unsupported(
                    DiagnosticOrigin::Resolver,
                    "derived-modifier-diagnostic",
                    modifiers[0].span,
                    "Derived modifier diagnostics are not implemented.",
                ));
            };
            return Err(Diagnostic::language(
                DiagnosticOrigin::Resolver,
                DiagnosticKind::Syntax,
                modifier.span,
                message,
            ));
        }
    }
    Ok(())
}

pub fn resolve_module(module: &Module) -> Result<ResolvedModule, Diagnostic> {
    let (result, diagnostic_owner_requested) = resolve_module_inner(module, false);
    if !diagnostic_owner_requested {
        return result;
    }
    let Err(mut error) = result else {
        unreachable!("diagnostic ownership is requested only on rejection");
    };
    // Valid modules never allocate this naming stack. Replay the same worklist
    // once for the selected rejection, so sibling ordinals retain visit order.
    // Adopt only ownership, and only if every other diagnostic field is equal.
    if let (Err(mut replay), _) = resolve_module_inner(module, true) {
        let owner = replay.member.take();
        let original_owner = error.member.take();
        error.member = if replay == error {
            owner
        } else {
            original_owner
        };
    }
    Err(error)
}

fn resolve_module_inner(
    module: &Module,
    capture_diagnostic_owner: bool,
) -> (Result<ResolvedModule, Diagnostic>, bool) {
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
        const_scopes: Vec::new(),
        entry_counts: BTreeMap::new(),
        member_path: Vec::new(),
        diagnostic_path: Vec::new(),
        diagnostic_owners: diagnostic_owner::OwnerScopes::new(capture_diagnostic_owner),
        diagnostic_owner_requested: false,
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
        if let Some(declaration) = &module.declaration {
            let previous = resolver.enter_object(module.members.iter().collect());
            resolver.visit(Work::Annotations(
                &declaration.annotations,
                false,
                ConstLevel::None,
            ))?;
            resolver.visit(Work::EndObject(previous))?;
        }
        resolver.members(&module.members, false)?;
        Ok::<(), Diagnostic>(())
    })();
    let diagnostic_owner_requested = resolver.diagnostic_owner_requested;
    let result = validation
        .map_err(|mut error| {
            error.module_name = module
                .declaration
                .as_ref()
                .and_then(|declaration| declaration.name.as_ref())
                .map(|name| name.join(".").into_boxed_str());
            error
        })
        .map(|()| ResolvedModule {
            module: module.clone(),
            names: resolver.names,
            const_names: resolver.const_names,
            custom_this: resolver.custom_this,
            custom_functions: resolver.custom_functions,
        });
    (result, diagnostic_owner_requested)
}
