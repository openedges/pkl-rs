//! Static lexical binding for shared source syntax.
//!
//! Property lookup preserves both lexical owner depth and virtual receiver.
//! An absent lexical declaration resolves to the current receiver, rather than
//! searching enclosing objects dynamically. See Pkl 0.32 SymbolTable.resolveVariable.
use crate::diagnostic::{Diagnostic, DiagnosticKind, DiagnosticOrigin};
use crate::source::{SourceSlice, SourceText, Span};
use crate::syntax::{
    Expr, ExprKind, Member, Modifier, ModifierKind, Module, ModuleParentKind, ObjectMember,
    Parameter, Type, TypeKind, TypeParameter, modifier_flags,
};
use crate::value::PklString;
use std::collections::BTreeMap;
use std::sync::Arc;
mod constant_keys;
mod diagnostic_owner;

// Nonlocal properties/classes/type aliases in the pinned stdlib/base.pkl,
// following BaseModuleMembersGenerator. Base semantics are outside this prototype.
fn is_base_property(name: &PklString) -> bool {
    let Some(name) = name.as_str() else {
        return false;
    };
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
fn is_base_method(name: &PklString) -> bool {
    let Some(name) = name.as_str() else {
        return false;
    };
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

fn is_base_class(name: &PklString) -> bool {
    let Some(name) = name.as_str() else {
        return false;
    };
    matches!(
        name,
        "Module"
            | "Resource"
            | "FileOutput"
            | "ModuleOutput"
            | "BaseValueRenderer"
            | "ValueRenderer"
            | "BytesRenderer"
            | "PcfRenderer"
            | "JsonRenderer"
            | "YamlRenderer"
            | "PListRenderer"
            | "PropertiesRenderer"
            | "ConvertProperty"
            | "RenderDirective"
            | "PcfRenderDirective"
            | "Pair"
            | "Annotation"
            | "Deprecated"
            | "Since"
            | "Unlisted"
            | "AlsoKnownAs"
            | "DocExample"
            | "SourceCode"
            | "ModuleInfo"
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
            | "Duration"
            | "DataSize"
            | "IntSeq"
            | "Regex"
            | "RegexMatch"
            | "Object"
            | "Typed"
            | "VarArgs"
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
    /// Generator branches declare the same property with different locality.
    AmbiguousProperty {
        owner_depth: usize,
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

/// Ordered construction observations, including repeated visits.
#[derive(Clone, Debug)]
pub(crate) enum AstPreparation {
    Import {
        clause: bool,
        glob: bool,
        node_span: Span,
        uri: PklString,
        span: Span,
        member: crate::value::PklString,
    },
}

#[derive(Clone, Debug)]
pub struct ResolvedModule {
    pub module: Arc<Module>,
    pub(crate) standard_library: bool,
    pub(crate) preparation: Vec<AstPreparation>,
    pub(crate) preparation_is_evaluation: bool,
    pub(crate) function_owners: BTreeMap<Span, crate::value::PklString>,
    pub(crate) preparation_failure: Option<Diagnostic>,
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
    Let(&'a PklString),
    Parameters(&'a [Parameter]),
    ObjectParameters(&'a [Parameter], bool),
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
    function_owners: BTreeMap<Span, crate::value::PklString>,
    source: SourceSlice<'a>,
    preparation: Vec<AstPreparation>,
    deferred_diagnostics: &'a [Diagnostic],
    evaluation_preparation: bool,
    standard_library: bool,
    scopes: Vec<Scope<'a>>,
    folded_integer_spans: &'a BTreeMap<Span, Span>,
    names: BTreeMap<Span, Binding>,
    const_names: std::collections::BTreeSet<Span>,
    custom_this: std::collections::BTreeSet<Span>,
    custom_functions: std::collections::BTreeSet<Span>,
    const_level: ConstLevel,
    object_levels: Vec<ConstLevel>,
    module_const_owners: Vec<&'static str>,
    object_receiver_lambdas: Vec<bool>,
    receiver_is_lambda: bool,
    const_scopes: Vec<ConstScope>,
    entry_counts: BTreeMap<crate::value::PklString, usize>,
    member_path: Vec<PklString>,
    diagnostic_path: Vec<crate::value::PklString>,
    diagnostic_owners: diagnostic_owner::OwnerScopes,
    diagnostic_owner_requested: bool,
    amending_module: bool,
}

fn member_category(member: &Member) -> u8 {
    match member.expr.kind {
        // Import clauses lower to a member whose name is the entire clause.
        // A normal property initialized by import(...) stays a class property.
        ExprKind::Import { .. } if member.name_span == member.expr.span => 0,
        ExprKind::Class { .. } => 1,
        ExprKind::TypeAlias { .. } => 2,
        ExprKind::Function { method: true, .. } => 4,
        _ => 3,
    }
}

fn member_header_span(member: &Member) -> Span {
    match &member.expr.kind {
        ExprKind::Class { header_span, .. }
        | ExprKind::TypeAlias { header_span, .. }
        | ExprKind::Function {
            header_span,
            method: true,
            ..
        } => *header_span,
        ExprKind::Import { .. } if member.name_span == member.expr.span => member.expr.span,
        _ => Span::new(
            member
                .modifiers
                .first()
                .map_or(member.name_span.start, |modifier| modifier.span.start),
            member
                .ty
                .as_ref()
                .map_or(member.name_span.end, |ty| ty.span.end),
        ),
    }
}

// Resolver continuations own temporary scope state, never authored AST nodes.
// Scope exits run before the next sibling; expression/type/member visitation is
// ordered exactly as the pinned builder, including generator body-first checks.
enum Work<'a> {
    Expression(&'a Expr, bool),
    MapArguments(&'a Expr),
    Annotations(&'a [Expr], bool, ConstLevel),
    Annotation(&'a Expr, bool),
    AnnotationBody(&'a [ObjectMember], bool),
    EndAnnotations(ConstLevel),
    EndAnnotationBody(ConstLevel),
    Receiver(&'a Expr),
    Type(&'a Type),
    NamedType(&'a Type),
    Supertype(&'a Type),
    Constraints(&'a [Expr]),
    Members(&'a [Member], bool),
    MembersNext {
        members: Vec<&'a Member>,
        index: usize,
        seen: std::collections::BTreeSet<(&'a PklString, bool)>,
        object: bool,
    },
    Member(&'a Member, bool),
    MemberBody(&'a Member, bool),
    ObjectMembers(&'a [ObjectMember], &'a [Parameter]),
    PopObjectParameters,
    CheckGeneratorParameters(&'a [Parameter]),
    ObjectBody(&'a [ObjectMember], Option<Span>),
    SpecializedBody {
        members: &'a [ObjectMember],
        index: usize,
        seen: std::collections::BTreeSet<(&'a PklString, bool)>,
    },
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
        parameter_visibility: Option<bool>,
    },
    EntryValue(&'a Expr, &'a Expr),
    PredicateValue(&'a Expr, &'a Expr),
    EndPredicate(crate::value::PklString),
    ForHeader(
        &'a ObjectMember,
        usize,
        std::collections::BTreeSet<&'a PklString>,
    ),
    ForBody(&'a [Parameter], &'a [ObjectMember], Span),
    EndFor,
    LetBody(&'a PklString, &'a Expr, bool),
    EndLet(bool),
    ClassMembers(&'a [Member]),
    EndClass(ConstLevel),
    EndAlias(ConstLevel),
    EndFunction(bool, bool),
    EndMember(ConstLevel),
    EndObject(bool),
    EndTypeParameters,
    PopScope,
    EndDiagnosticOwner,
    DiagnosticOwnerPrefix(&'a [ObjectMember]),
}

impl<'a> Resolver<'a> {
    fn binding(&self, name: &PklString, method: bool) -> Binding {
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
                Scope::Parameters(parameters)
                | Scope::GeneratorParameters(parameters)
                | Scope::ObjectParameters(parameters, _) => {
                    if !method
                        && !matches!(scope, Scope::ObjectParameters(_, false))
                        && name != "_"
                        && let Some(index) = parameters
                            .iter()
                            .position(|parameter| &parameter.name == name)
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
                        .find(|member| &member.name == name && is_method(member) == method)
                    {
                        return if method {
                            Binding::Method {
                                owner_depth: owner_depth.max(0) as usize,
                                local: member.local,
                            }
                        } else if members.iter().any(|other| {
                            &other.name == name && !is_method(other) && other.local != member.local
                        }) {
                            Binding::AmbiguousProperty {
                                owner_depth: owner_depth.max(0) as usize,
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

    /// AstBuilder builds an amending module as object members, in grouped
    /// declaration order. A class/alias is fully visited before its local check.
    fn amending_members(&mut self, members: &'a [Member]) -> Result<(), Diagnostic> {
        let previous = self.enter_object(members.iter().collect());
        let mut seen = std::collections::BTreeSet::new();
        for category in 0..5 {
            for member in members
                .iter()
                .filter(|member| member_category(member) == category)
            {
                self.visit(Work::Member(member, category >= 3))?;
                let header = member_header_span(member);
                if !member.local && (category == 1 || category == 2) {
                    let (target, label) = if category == 1 {
                        ("class", "Class")
                    } else {
                        ("type alias", "Type alias")
                    };
                    return Err(Diagnostic::language(
                        DiagnosticOrigin::Resolver, DiagnosticKind::Syntax, header,
                        format!("{label} needs a `local` modifier."),
                    ).with_hint(format!("To define a non-local {target}, extend rather than amend the parent module (which must be `open` for extension).")));
                }
                if !seen.insert((&member.name, is_method(member))) {
                    return Err(Diagnostic::language(
                        DiagnosticOrigin::Resolver,
                        DiagnosticKind::DuplicateMember,
                        header,
                        PklString::from("Duplicate definition of member `")
                            .concat(&member.name)
                            .concat(&"`.".into()),
                    ));
                }
            }
        }
        self.visit(Work::EndObject(previous))
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
            if let Work::Expression(expression, _) = &task
                && let Some(diagnostic) = self.deferred_diagnostics.iter().find(|diagnostic| {
                    diagnostic.span == expression.span
                        || (matches!(expression.kind, ExprKind::Int(_))
                            && expression.span.start <= diagnostic.span.start
                            && expression.span.end >= diagnostic.span.end)
                })
            {
                let mut diagnostic = diagnostic.clone();
                if self.evaluation_preparation {
                    diagnostic.member = self
                        .diagnostic_owners
                        .name()
                        .filter(|name| !name.is_empty())
                        .map(Into::into);
                }
                return Err(diagnostic);
            }
            match task {
                Work::MapArguments(expression) => {
                    let mut error = Diagnostic::language(
                        DiagnosticOrigin::DeferredAstValidation,
                        DiagnosticKind::Syntax,
                        Span::new(expression.span.end - 1, expression.span.end),
                        "Number of `Map()` arguments must be a multiple of two: `Map(key1, value1, key2, value2, ..., keyN, valueN)`",
                    );
                    error.member = self
                        .diagnostic_owners
                        .name()
                        .filter(|name| !name.is_empty())
                        .map(Into::into);
                    return Err(error);
                }
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
                    self.diagnostic_owners.enter_annotation();
                    self.module_const_owners.push("annotation");
                    work.push(Work::EndAnnotationBody(previous));
                    work.push(Work::ObjectMembers(members, &[]));
                }
                Work::EndAnnotations(previous) => self.const_level = previous,
                Work::EndAnnotationBody(previous) => {
                    self.scopes.pop();
                    self.diagnostic_owners.exit();
                    self.module_const_owners.pop();
                    self.const_level = previous;
                }
                Work::EndMember(level) => {
                    self.diagnostic_owners.exit();
                    self.const_level = level;
                    self.member_path.pop();
                    self.diagnostic_path.pop();
                }
                Work::CheckGeneratorParameters(parameters) => {
                    if let Some(parameter) = parameters.first() {
                        return Err(Diagnostic::unsupported(
                            DiagnosticOrigin::DeferredAstValidation,
                            "parameterized-generator-body",
                            parameter.span,
                            "Parameterized for/when bodies encounter an upstream internal error.",
                        ));
                    }
                }
                Work::PopObjectParameters => {
                    self.scopes.pop();
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
                Work::EndClass(level) => {
                    self.module_const_owners.pop();
                    self.const_level = level;
                }
                Work::EndAlias(level) => {
                    self.const_level = level;
                    self.scopes.pop();
                }
                Work::EndTypeParameters => {
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
                    parameter_visibility,
                } => {
                    self.diagnostic_owners.exit();
                    if custom {
                        self.diagnostic_owners.exit();
                        self.scopes.pop();
                    }
                    self.const_scopes[const_index].lookup = true;
                    self.receiver_is_lambda = lambda;
                    self.object_levels.push(level);
                    if let Some(previous) = parameter_visibility
                        && let Scope::ObjectParameters(_, visible) = &mut self.scopes[index - 1]
                    {
                        *visible = previous;
                    }
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
                    let previous = self.enter_object(members.iter().collect());
                    work.push(Work::EndObject(previous));
                    // AstBuilder builds module imports, classes and aliases before
                    // class properties, then methods. Class bodies have only the
                    // final two categories. Preserve source order within each.
                    let ordered = (0..5)
                        .flat_map(|category| {
                            members
                                .iter()
                                .filter(move |member| member_category(member) == category)
                        })
                        .collect();
                    work.push(Work::MembersNext {
                        members: ordered,
                        index: 0,
                        seen: std::collections::BTreeSet::new(),
                        object,
                    });
                }
                Work::MembersNext {
                    members,
                    index,
                    mut seen,
                    object,
                } => {
                    // A member's annotations, modifiers, types and body are
                    // validated before its duplicate check, as in AstBuilder.
                    // This continuation runs after EndMember restores ownership.
                    if index > 0 {
                        let member = members[index - 1];
                        let category = member_category(member);
                        if self.evaluation_preparation
                            && object
                            && self.amending_module
                            && self.object_levels.len() == 1
                            && !member.local
                            && (category == 1 || category == 2)
                        {
                            let (target, label) = if category == 1 {
                                ("class", "Class")
                            } else {
                                ("type alias", "Type alias")
                            };
                            return Err(Diagnostic::language(DiagnosticOrigin::Resolver, DiagnosticKind::Syntax,
                                member_header_span(member), format!("{label} needs a `local` modifier."))
                                .with_hint(format!("To define a non-local {target}, extend rather than amend the parent module (which must be `open` for extension).")));
                        }
                        if !seen.insert((&member.name, is_method(member))) {
                            // The member visitor has exited; Java reports this
                            // duplicate from the enclosing declaration scope.
                            self.diagnostic_owner_requested = true;
                            let mut error = Diagnostic::language(
                                DiagnosticOrigin::Resolver,
                                DiagnosticKind::DuplicateMember,
                                member_header_span(member),
                                PklString::from("Duplicate definition of member `")
                                    .concat(&member.name)
                                    .concat(&"`.".into()),
                            );
                            error.member = self
                                .diagnostic_owners
                                .name()
                                .filter(|name| !name.is_empty())
                                .map(Into::into);
                            return Err(error);
                        }
                    }
                    if let Some(&member) = members.get(index) {
                        work.push(Work::MembersNext {
                            members,
                            index: index + 1,
                            seen,
                            object,
                        });
                        work.push(Work::Member(member, object && member_category(member) >= 3));
                    }
                }
                Work::Member(member, object) => {
                    self.diagnostic_owners.enter_member(&member.name);
                    self.member_path.push(member.name.clone());
                    self.diagnostic_path.push(member.name.clone());
                    let previous = self.const_level;
                    if modifier_flags(&member.modifiers) & ModifierKind::Const.flag() != 0 {
                        self.const_level = ConstLevel::All;
                    }
                    if self.evaluation_preparation && !object {
                        match &member.expr.kind {
                            ExprKind::Class {
                                type_parameters, ..
                            }
                            | ExprKind::Function {
                                type_parameters,
                                method: true,
                                ..
                            } => {
                                self.reject_nonstdlib_type_parameters(type_parameters)
                                    .map_err(|mut error| {
                                        error.member = self
                                            .diagnostic_owners
                                            .enclosing_name()
                                            .filter(|name| !name.is_empty())
                                            .map(Into::into);
                                        error
                                    })?;
                            }
                            _ => (),
                        }
                    }
                    self.validate_member(member, object, false)?;
                    if self.evaluation_preparation
                        && let ExprKind::TypeAlias {
                            type_parameters, ..
                        } = &member.expr.kind
                    {
                        let mut seen = std::collections::BTreeSet::new();
                        for parameter in type_parameters {
                            if !seen.insert(&parameter.name) {
                                let mut error = Diagnostic::language(
                                    DiagnosticOrigin::Resolver,
                                    DiagnosticKind::DuplicateMember,
                                    parameter.span,
                                    PklString::from("Duplicate type parameter `")
                                        .concat(&parameter.name)
                                        .concat(&"`.".into()),
                                );
                                error.member = self
                                    .diagnostic_owners
                                    .enclosing_name()
                                    .filter(|name| !name.is_empty())
                                    .map(Into::into);
                                return Err(error);
                            }
                        }
                    }
                    work.push(Work::EndMember(previous));
                    if let Some(ty) = &member.ty {
                        work.push(Work::Type(ty));
                    }
                    // AstBuilder visits a property's body before its type AST.
                    work.push(Work::MemberBody(member, object));
                    if !(object && self.amending_module && self.object_levels.len() == 1) {
                        work.push(Work::Annotations(
                            &member.annotations,
                            !object && self.object_levels.len() > 1,
                            previous,
                        ));
                    }
                }
                Work::MemberBody(member, object) => {
                    if self.evaluation_preparation {
                        self.validate_member(member, object, true)?;
                    }
                    if object
                        && is_method(member)
                        && modifier_flags(&member.modifiers) & ModifierKind::Local.flag() == 0
                    {
                        let ExprKind::Function { header_span, .. } = member.expr.kind else {
                            unreachable!()
                        };
                        let amending = self.amending_module && self.object_levels.len() == 1;
                        let mut error = Diagnostic::language(
                            DiagnosticOrigin::Resolver,
                            DiagnosticKind::Syntax,
                            header_span,
                            if amending {
                                "Method needs a `local` modifier."
                            } else {
                                "Method needs a `local` modifier because it is defined in an object, not a class."
                            },
                        );
                        if amending {
                            error = error.with_hint("To define a non-local method, extend rather than amend the parent module (which must be `open` for extension).");
                        }
                        let enclosing = &self.diagnostic_path[..self.diagnostic_path.len() - 1];
                        if !enclosing.is_empty() {
                            error.member =
                                Some(crate::value::PklString::join(enclosing, ".").into());
                        }
                        return Err(error);
                    }
                    if object
                        && self.amending_module
                        && !self.standard_library
                        && self.object_levels.len() == 1
                        && let ExprKind::Function {
                            type_parameters_span: Some(span),
                            method: true,
                            ..
                        } = &member.expr.kind
                    {
                        let mut error = Diagnostic::language(
                            DiagnosticOrigin::Resolver,
                            DiagnosticKind::Syntax,
                            **span,
                            "Only standard library members can have type parameters.",
                        );
                        error.member =
                            Some(crate::value::PklString::join(&self.diagnostic_path, ".").into());
                        return Err(error);
                    }
                    work.push(Work::Expression(&member.expr, true));
                }
                Work::ObjectMembers(members, parameters) => {
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
                    if !self.evaluation_preparation
                        && members.iter().all(|member| {
                            matches!(
                                member,
                                ObjectMember::Property(_)
                                    | ObjectMember::Element { .. }
                                    | ObjectMember::Entry { .. }
                            )
                        })
                    {
                        let mut seen = std::collections::BTreeSet::new();
                        for member in &names {
                            if !seen.insert((&member.name, is_method(member))) {
                                let mut error = Diagnostic::language(
                                    DiagnosticOrigin::Resolver,
                                    DiagnosticKind::DuplicateMember,
                                    member.name_span,
                                    PklString::from("Duplicate definition of member `")
                                        .concat(&member.name)
                                        .concat(&"`.".into()),
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
                    if !parameters.is_empty() {
                        self.scopes.push(Scope::ObjectParameters(parameters, true));
                        work.push(Work::PopObjectParameters);
                    }
                    let previous = self.enter_object(names);
                    work.push(Work::EndObject(previous));
                    work.push(Work::ConstantEntries(members));
                    if self.evaluation_preparation {
                        work.push(Work::SpecializedBody {
                            members,
                            index: 0,
                            seen: Default::default(),
                        });
                    } else if self.diagnostic_owners.is_enabled()
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
                    if !members.is_empty() {
                        work.extend(
                            parameters
                                .iter()
                                .rev()
                                .filter_map(|parameter| parameter.ty.as_ref())
                                .map(Work::Type),
                        );
                    }
                }
                Work::SpecializedBody {
                    members,
                    index,
                    mut seen,
                } => {
                    if index > 0
                        && let ObjectMember::Property(member) = &members[index - 1]
                        && !seen.insert((&member.name, is_method(member)))
                    {
                        let mut error = Diagnostic::language(
                            DiagnosticOrigin::Resolver,
                            DiagnosticKind::DuplicateMember,
                            member.name_span,
                            PklString::from("Duplicate definition of member `")
                                .concat(&member.name)
                                .concat(&"`.".into()),
                        );
                        error.member = self
                            .diagnostic_owners
                            .name()
                            .filter(|name| !name.is_empty())
                            .map(Into::into);
                        return Err(error);
                    }
                    if let Some(member) = members.get(index) {
                        if matches!(
                            member,
                            ObjectMember::Property(_)
                                | ObjectMember::Element { .. }
                                | ObjectMember::Entry { .. }
                        ) {
                            work.push(Work::SpecializedBody {
                                members,
                                index: index + 1,
                                seen,
                            });
                            work.push(Work::ObjectMember(member, None));
                        } else {
                            // The specialized builder has already visited its prefix.
                            // Its generator fallback visits the complete body again.
                            work.push(Work::ObjectBody(members, None));
                        }
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
                        then_parameters,
                        else_parameters,
                        ..
                    } => {
                        work.push(Work::Eager(condition, false));
                        work.push(Work::ObjectBody(else_members, for_span));
                        work.push(Work::CheckGeneratorParameters(else_parameters));
                        work.push(Work::ObjectBody(then_members, for_span));
                        work.push(Work::CheckGeneratorParameters(then_parameters));
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
                        body_parameters,
                        iterable,
                        members,
                        span,
                    } = member
                    else {
                        unreachable!()
                    };
                    if self.evaluation_preparation && index == 0 {
                        for parameter in parameters {
                            if parameter.name != "_" && !seen.insert(&parameter.name) {
                                let mut error = Diagnostic::language(
                                    DiagnosticOrigin::Resolver,
                                    DiagnosticKind::DuplicateMember,
                                    parameter.span,
                                    PklString::from("Duplicate definition of member `")
                                        .concat(&parameter.name)
                                        .concat(&"`.".into()),
                                );
                                error.member = self
                                    .diagnostic_owners
                                    .name()
                                    .filter(|name| !name.is_empty())
                                    .map(Into::into);
                                return Err(error);
                            }
                        }
                    }
                    if let Some(parameter) = parameters.get(index) {
                        if !self.evaluation_preparation
                            && parameter.name != "_"
                            && !seen.insert(&parameter.name)
                        {
                            let mut error = Diagnostic::language(
                                DiagnosticOrigin::Resolver,
                                DiagnosticKind::DuplicateMember,
                                parameter.span,
                                PklString::from("Duplicate definition of member `")
                                    .concat(&parameter.name)
                                    .concat(&"`.".into()),
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
                        work.push(Work::CheckGeneratorParameters(body_parameters));
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
                    let parameter_visibility = if index > 0
                        && let Scope::ObjectParameters(_, visible) = &mut self.scopes[index - 1]
                    {
                        Some(std::mem::replace(visible, false))
                    } else {
                        None
                    };
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
                        parameter_visibility,
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
                Work::Supertype(ty) => {
                    let span = ty.unparenthesized_span.unwrap_or(ty.span);
                    if !matches!(ty.kind, TypeKind::Named { .. } | TypeKind::Module) {
                        let mut error = Diagnostic::language(
                            DiagnosticOrigin::Resolver,
                            DiagnosticKind::Syntax,
                            span,
                            PklString::from("`")
                                .concat(&self.source.range(span.start..span.end).to_pkl_string())
                                .concat(&"` is not a valid supertype.".into()),
                        );
                        error.member = self
                            .diagnostic_owners
                            .name()
                            .filter(|name| !name.is_empty())
                            .map(Into::into);
                        return Err(error);
                    }
                }
                Work::NamedType(ty) => {
                    let TypeKind::Named {
                        path, name_spans, ..
                    } = &ty.kind
                    else {
                        unreachable!()
                    };
                    let parameter = path.len() == 1
                        && self.scopes.iter().rev().any(|scope| match scope {
                            Scope::TypeParameters(parameters) => {
                                parameters.iter().any(|parameter| parameter.name == path[0])
                            }
                            _ => false,
                        });
                    if path.len() > 2 {
                        self.diagnostic_owner_requested = true;
                        let identifier_span = Span::new(
                            name_spans.first().unwrap().start,
                            name_spans.last().unwrap().end,
                        );
                        let mut error = Diagnostic::language(
                                DiagnosticOrigin::Resolver,
                                DiagnosticKind::Syntax,
                                identifier_span,
                                PklString::from("Invalid type name `").concat(&self.source.range(identifier_span.start..identifier_span.end).to_pkl_string()).concat(&"`.".into()),
                            ).with_hint("A qualified type name consists of a module import name and a type name.\nExample: `importedModule.SomeClass`");
                        error.member = self.diagnostic_owners.name().map(Into::into);
                        return Err(error);
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
                }
                Work::Type(ty) => match &ty.kind {
                    TypeKind::Named { arguments, .. } => {
                        work.push(Work::NamedType(ty));
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
                    ExprKind::Import {
                        uri,
                        uri_span,
                        glob,
                        ..
                    } => {
                        self.preparation.push(AstPreparation::Import {
                            clause: false,
                            glob: *glob,
                            node_span: expression.span,
                            uri: uri.clone(),
                            span: *uri_span,
                            member: self.diagnostic_owners.name().unwrap_or_default(),
                        });
                    }
                    ExprKind::Trace(value) => {
                        work.push(Work::Expression(value, contextual_new));
                    }
                    ExprKind::Parenthesized(value)
                    | ExprKind::NonNull(value)
                    | ExprKind::Throw(value)
                    | ExprKind::ReadResourceGlob(value)
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
                        if self.evaluation_preparation
                            && matches!(&callee.kind, ExprKind::Name(name)
                            if name == "Map" && self.binding(name, true) == Binding::ImplicitMethod)
                            && arguments.len() % 2 != 0
                        {
                            work.push(Work::MapArguments(expression));
                        }
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
                                    // SymbolTable resolves lexical methods before
                                    // base methods, and base methods before the
                                    // custom receiver. Keep existing constructor
                                    // intrinsics on their ordinary lowering path.
                                    if !matches!(
                                        name.as_str(),
                                        Some(
                                            "List"
                                                | "Set"
                                                | "Map"
                                                | "Bytes"
                                                | "Pair"
                                                | "IntSeq"
                                                | "Regex"
                                                | "Null"
                                                | "TODO"
                                                | "Undefined"
                                        )
                                    ) {
                                        return Err(Diagnostic::unsupported(
                                            DiagnosticOrigin::Resolver,
                                            "custom-this-base-method",
                                            callee.span,
                                            "Base-module method resolution is not implemented.",
                                        ));
                                    }
                                } else {
                                    binding = Binding::CustomMethod;
                                }
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
                    ExprKind::Binary { left, right, op } => {
                        if self.evaluation_preparation
                            && matches!(
                                op,
                                crate::syntax::BinaryOp::And
                                    | crate::syntax::BinaryOp::Or
                                    | crate::syntax::BinaryOp::NullCoalesce
                            )
                        {
                            work.push(Work::Expression(left, false));
                            work.push(Work::Expression(right, false));
                        } else {
                            work.push(Work::Expression(right, false));
                            work.push(Work::Expression(left, false));
                        }
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
                        if !type_parameters.is_empty() {
                            self.scopes.push(Scope::TypeParameters(type_parameters));
                            work.push(Work::EndTypeParameters);
                        }
                        if !method && parameters.len() > 5 {
                            // AstBuilder validates arity before entering the
                            // lambda scope: the enclosing property/entry owns
                            // this failure, without a new function ordinal.
                            self.diagnostic_owner_requested = true;
                            let mut error = Diagnostic::language(
                                DiagnosticOrigin::Resolver,
                                DiagnosticKind::Syntax,
                                expression.span,
                                "Function literals can have at most five parameters.",
                            );
                            error.member = self
                                .diagnostic_owners
                                .name()
                                .filter(|name| !name.is_empty())
                                .map(Into::into);
                            return Err(error);
                        }
                        let mut seen = std::collections::BTreeSet::new();
                        for parameter in parameters {
                            if !self.evaluation_preparation
                                && parameter.name != "_"
                                && !seen.insert(&parameter.name)
                            {
                                return Err(Diagnostic::language(
                                    DiagnosticOrigin::Resolver,
                                    DiagnosticKind::DuplicateMember,
                                    parameter.span,
                                    PklString::from("Duplicate parameter `")
                                        .concat(&parameter.name)
                                        .concat(&"`.".into()),
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
                            if self.evaluation_preparation
                                && let Some(owner) = self.diagnostic_owners.name()
                            {
                                self.function_owners.insert(expression.span, owner);
                            }
                        }
                        work.push(Work::EndFunction(previous, !method));
                        if let Some(ty) = return_type {
                            work.push(Work::Type(ty));
                        }
                        work.extend(
                            parameters
                                .iter()
                                .rev()
                                .filter_map(|parameter| parameter.ty.as_ref().map(Work::Type)),
                        );
                        // AstBuilder visits the body before parameter/return
                        // type ASTs (parameter names were checked above).
                        // Methods infer an untyped new from the return type;
                        // a lambda body is not an inference context.
                        work.push(Work::Expression(body, *method));
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
                        if !type_parameters.is_empty() {
                            self.scopes.push(Scope::TypeParameters(type_parameters));
                            work.push(Work::EndTypeParameters);
                        }
                        // Member annotations have already been visited. Java enters
                        // ClassScope before visiting the superclass (including its
                        // constraints), not only when it reaches the class body.
                        let previous = self.const_level;
                        self.const_level = self.const_level.max(ConstLevel::Module);
                        self.module_const_owners.push("class");
                        work.push(Work::EndClass(previous));
                        work.push(Work::ClassMembers(members));
                        if let Some(ty) = superclass {
                            if self.evaluation_preparation {
                                work.push(Work::Supertype(ty));
                            }
                            work.push(Work::Type(ty));
                        }
                    }
                    ExprKind::TypeAlias {
                        type_parameters,
                        ty,
                        ..
                    } => {
                        let mut seen = std::collections::BTreeSet::new();
                        for parameter in type_parameters {
                            if !seen.insert(&parameter.name) {
                                return Err(Diagnostic::language(
                                    DiagnosticOrigin::Resolver,
                                    DiagnosticKind::DuplicateMember,
                                    parameter.span,
                                    PklString::from("Duplicate type parameter `")
                                        .concat(&parameter.name)
                                        .concat(&"`.".into()),
                                ));
                            }
                        }
                        self.scopes.push(Scope::TypeParameters(type_parameters));
                        let previous = self.const_level;
                        self.const_level = self.const_level.max(ConstLevel::Module);
                        work.push(Work::EndAlias(previous));
                        work.push(Work::Type(ty));
                    }
                    ExprKind::New {
                        ty,
                        members,
                        parameters,
                    }
                    | ExprKind::Annotation {
                        ty,
                        members,
                        parameters,
                    } => {
                        work.push(Work::ObjectMembers(members, parameters));
                        work.push(Work::Type(ty));
                    }
                    ExprKind::Object {
                        parent,
                        members,
                        parameters,
                    } => {
                        if parent.is_none() && !contextual_new {
                            self.diagnostic_owner_requested = true;
                            let mut error = self.const_error(expression, "Cannot tell which parent to amend.").with_hint("Please specify a parent explicitly, for example `new Person { ... }` instead of `new { ... }`.");
                            if let Some(owner) = self.diagnostic_owners.name() {
                                error.member = Some(owner.into());
                            }
                            error.span.end = error.span.start + 3;
                            return Err(error);
                        }
                        work.push(Work::ObjectMembers(members, parameters));
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
    #[cfg_attr(target_arch = "wasm32", inline(never))]
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
                    PklString::from("Duplicate definition of member `")
                        .concat(&value.render())
                        .concat(&"`.".into()),
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
        if !self.standard_library
            && let Some(parameter) = parameters.first()
        {
            return Err(Diagnostic::language(
                DiagnosticOrigin::Resolver,
                DiagnosticKind::Syntax,
                parameter.span,
                "Only standard library members can have type parameters.",
            ));
        }
        let mut seen = std::collections::BTreeSet::new();
        for parameter in parameters {
            if !seen.insert(&parameter.name) {
                return Err(Diagnostic::language(
                    DiagnosticOrigin::Resolver,
                    DiagnosticKind::DuplicateMember,
                    parameter.span,
                    PklString::from("Duplicate type parameter `")
                        .concat(&parameter.name)
                        .concat(&"`.".into()),
                ));
            }
        }
        Ok(())
    }

    #[cfg_attr(target_arch = "wasm32", inline(never))]
    fn validate_member(
        &mut self,
        member: &Member,
        object: bool,
        body: bool,
    ) -> Result<(), Diagnostic> {
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
        validate_modifiers(&member.modifiers, valid, target, self.standard_library).map_err(
            |mut error| {
                // The pinned builder checks modifiers before entering the member's
                // declaration scope, so the enclosing class or property owns it.
                let enclosing = &self.diagnostic_path[..self.diagnostic_path.len() - 1];
                if !enclosing.is_empty() {
                    error.member = Some(crate::value::PklString::join(enclosing, ".").into());
                }
                error
            },
        )?;
        let flags = modifier_flags(&member.modifiers);
        let has = |kind: ModifierKind| flags & kind.flag() != 0;
        let modifier = |kind| {
            member
                .modifiers
                .iter()
                .find(|modifier| modifier.kind == kind)
        };
        if object && !is_method(member) && has(M::Const) && !has(M::Local) {
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
            error.member = self
                .diagnostic_owners
                .enclosing_name()
                .filter(|name| !name.is_empty())
                .map(Into::into);
            return Err(error);
        }
        if self.evaluation_preparation && !body {
            return Ok(());
        }
        if object
            && self.amending_module
            && self.object_levels.len() == 1
            && has(M::Local)
            && matches!(member.expr.kind, ExprKind::Uninitialized)
            && let Some(ty) = &member.ty
        {
            self.diagnostic_owner_requested = true;
            let mut error = Diagnostic::language(
                DiagnosticOrigin::Resolver,
                DiagnosticKind::Syntax,
                Span::new(ty.span.end, ty.span.end),
                "Missing property value.",
            );
            error.member = self.diagnostic_owners.name().map(Into::into);
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
        if object
            && has(M::Local)
            && let Some(span) = member.amendment_body_span
        {
            // AstBuilder rejects an authored local amendment after entering
            // its property scope, before visiting the first body's members.
            self.diagnostic_owner_requested = true;
            let mut error = Diagnostic::language(
                DiagnosticOrigin::Resolver,
                DiagnosticKind::Syntax,
                span,
                "A local property definition cannot be amended.",
            );
            error = error.with_hint("Use definition syntax instead, for example `local person = new { ... }` instead of `local person { ... }`.");
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
                PklString::from("Method `")
                    .concat(&member.name)
                    .concat(&"` is missing a body.".into()),
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
            let mut error = Diagnostic::language(
                DiagnosticOrigin::Resolver,
                DiagnosticKind::Syntax,
                Span::new(member.span.end, member.span.end),
                "Missing property value.",
            );
            error.member = Some(crate::value::PklString::join(&self.diagnostic_path, ".").into());
            return Err(error);
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
            Binding::Property { owner_depth, .. }
            | Binding::AmbiguousProperty { owner_depth }
            | Binding::Method { owner_depth, .. } => self.needs_const(owner_depth),
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

    fn const_error(&self, expression: &Expr, message: impl Into<PklString>) -> Diagnostic {
        let mut error = Diagnostic::language(
            DiagnosticOrigin::Resolver,
            DiagnosticKind::Syntax,
            expression.span,
            message.into(),
        );
        error.member = self
            .diagnostic_owners
            .name()
            .filter(|name| !name.is_empty())
            .or_else(|| {
                (!self.diagnostic_path.is_empty())
                    .then(|| crate::value::PklString::join(&self.diagnostic_path, "."))
            })
            .map(Into::into);
        error
    }

    #[cfg_attr(target_arch = "wasm32", inline(never))]
    fn leaf_expression(&mut self, expression: &'a Expr) -> Result<(), Diagnostic> {
        match &expression.kind {
            ExprKind::Name(name) => {
                let mut binding = self.binding(name, false);
                if binding == Binding::ImplicitThis
                    && matches!(name.as_str(), Some("NaN" | "Infinity"))
                {
                    binding = Binding::BaseConstant;
                }
                if binding == Binding::ImplicitThis && is_base_class(name) {
                    binding = Binding::BaseClass;
                }
                if binding == Binding::ImplicitThis
                    && matches!(
                        name.as_str(),
                        Some(
                            "NonNull"
                                | "Int8"
                                | "Int16"
                                | "Int32"
                                | "UInt8"
                                | "UInt16"
                                | "UInt32"
                                | "UInt"
                                | "DataSizeUnit"
                                | "DurationUnit"
                                | "Charset"
                                | "Comparable"
                                | "Char"
                                | "Uri"
                                | "Mixin"
                        )
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
                    self.diagnostic_owner_requested = true;
                    return Err(self.const_error(
                        expression,
                        "Cannot reference `this` from here because it is not `const`.",
                    ));
                }
            }
            ExprKind::Module => {
                if self.const_level != ConstLevel::None {
                    self.diagnostic_owner_requested = true;
                    if self.receiver_is_lambda && self.module_const_owners.is_empty() {
                        let mut error = self.const_error(
                            expression,
                            "An unexpected error has occurred. Would you mind filing a bug report?",
                        );
                        error.provenance.disposition =
                            crate::diagnostic::FailureDisposition::OracleError;
                        error.provenance.foreign_exception = Some(std::sync::Arc::new(
                            crate::java_exception::provenance::ForeignExceptionProvenance::new(
                                crate::java_exception::provenance::ForeignOperation::ConstLambdaNullPointer,
                            ),
                        ));
                        return Err(error);
                    }
                    let message = if let Some(owner) = self.module_const_owners.last() {
                        let article = if *owner == "annotation" { "an" } else { "a" };
                        PklString::from(format!(
                            "Cannot reference `module` within {article} {owner} body."
                        ))
                    } else {
                        PklString::from("Cannot reference `module` from const property `")
                            .concat(&self.member_path.last().cloned().unwrap_or_default())
                            .concat(&"`.".into())
                    };
                    return Err(self.const_error(expression, message));
                }
            }
            ExprKind::Outer => {
                if self
                    .parent_const_scope()
                    .is_some_and(|(_, scope)| self.const_level > scope.level)
                {
                    self.diagnostic_owner_requested = true;
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

#[cfg_attr(target_arch = "wasm32", inline(never))]
fn validate_modifiers(
    modifiers: &[Modifier],
    valid: &[ModifierKind],
    target: &str,
    standard_library: bool,
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
    if flags & M::External.flag() != 0 && !standard_library {
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
    resolve_module_with_context(Arc::new(module.clone()), false)
}

/// Only the bundled-source loader may opt into the pinned stdlib declaration rules.
/// User modules and separately parsed session expressions never receive this context.
pub(crate) fn resolve_stdlib_module(module: &Module) -> Result<ResolvedModule, Diagnostic> {
    resolve_module_with_context(Arc::new(module.clone()), true)
}

fn resolve_module_with_context(
    module: Arc<Module>,
    standard_library: bool,
) -> Result<ResolvedModule, Diagnostic> {
    let (resolved, error, owner_requested) =
        resolve_module_inner(Arc::clone(&module), false, false, standard_library);
    let Some(mut error) = error else {
        return Ok(resolved);
    };
    if owner_requested {
        let (_, replay, _) = resolve_module_inner(module, true, false, standard_library);
        if let Some(mut replay) = replay {
            let owner = replay.member.take();
            let original = error.member.take();
            error.member = if replay == error { owner } else { original };
        }
    }
    Err(error)
}

/// Resolve an owned `Module` without cloning its syntax tree.
pub(crate) fn resolve_module_owned(module: Module) -> Result<ResolvedModule, Diagnostic> {
    resolve_module_with_context(Arc::new(module), false)
}

/// An incomplete binding result can only compile to a failing initializer.
pub(crate) fn resolve_module_for_evaluation(module: &Module) -> ResolvedModule {
    resolve_module_for_evaluation_arc(Arc::new(module.clone()))
}

/// Resolve a shared `Arc<Module>` for evaluation without cloning its syntax tree.
pub(crate) fn resolve_module_for_evaluation_arc(module: Arc<Module>) -> ResolvedModule {
    let (mut resolved, error, _) = resolve_module_inner(module, true, true, false);
    resolved.preparation_failure = error;
    resolved
}

fn resolve_module_inner(
    module: Arc<Module>,
    capture_diagnostic_owner: bool,
    evaluation_preparation: bool,
    standard_library: bool,
) -> (ResolvedModule, Option<Diagnostic>, bool) {
    let mut resolver = Resolver {
        function_owners: BTreeMap::new(),
        preparation: Vec::new(),
        deferred_diagnostics: &module.deferred_diagnostics,
        source: module.source.entire(),
        evaluation_preparation,
        standard_library,
        scopes: Vec::new(),
        folded_integer_spans: &module.folded_integer_spans,
        names: BTreeMap::new(),
        const_names: std::collections::BTreeSet::new(),
        custom_this: std::collections::BTreeSet::new(),
        custom_functions: std::collections::BTreeSet::new(),
        const_level: ConstLevel::None,
        object_levels: Vec::new(),
        module_const_owners: Vec::new(),
        object_receiver_lambdas: Vec::new(),
        receiver_is_lambda: false,
        const_scopes: Vec::new(),
        entry_counts: BTreeMap::new(),
        member_path: Vec::new(),
        diagnostic_path: Vec::new(),
        diagnostic_owners: diagnostic_owner::OwnerScopes::new(capture_diagnostic_owner),
        diagnostic_owner_requested: false,
        amending_module: module
            .declaration
            .as_ref()
            .and_then(|decl| decl.parent.as_ref())
            .is_some_and(|parent| parent.kind == ModuleParentKind::Amends),
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
                standard_library,
            )?;
        }
        if evaluation_preparation {
            let previous = resolver.enter_object(module.members.iter().collect());
            for member in module.members.iter().filter(|m| member_category(m) == 0) {
                let ExprKind::Import {
                    uri,
                    uri_span,
                    glob,
                    ..
                } = &member.expr.kind
                else {
                    unreachable!()
                };
                resolver.preparation.push(AstPreparation::Import {
                    clause: true,
                    glob: *glob,
                    node_span: member.expr.span,
                    uri: uri.clone(),
                    span: *uri_span,
                    member: Default::default(),
                });
            }
            if let Some(declaration) = &module.declaration {
                resolver.visit(Work::Annotations(
                    &declaration.annotations,
                    false,
                    ConstLevel::None,
                ))?;
                if let Some(parent) = &declaration.parent {
                    resolver.preparation.push(AstPreparation::Import {
                        clause: false,
                        glob: false,
                        node_span: parent.span,
                        uri: parent.uri.clone(),
                        span: parent.span,
                        member: Default::default(),
                    });
                }
            }
            let ordered = (1..5)
                .flat_map(|category| {
                    module
                        .members
                        .iter()
                        .filter(move |m| member_category(m) == category)
                })
                .collect();
            let mut seen = std::collections::BTreeSet::new();
            for member in module.members.iter().filter(|m| member_category(m) == 0) {
                if !seen.insert((&member.name, false)) {
                    return Err(Diagnostic::language(
                        DiagnosticOrigin::Resolver,
                        DiagnosticKind::DuplicateMember,
                        member_header_span(member),
                        PklString::from("Duplicate definition of member `")
                            .concat(&member.name)
                            .concat(&"`.".into()),
                    ));
                }
            }
            resolver.visit(Work::MembersNext {
                members: ordered,
                index: 0,
                seen,
                object: resolver.amending_module,
            })?;
            resolver.visit(Work::EndObject(previous))?;
        } else {
            if let Some(declaration) = &module.declaration {
                let previous = resolver.enter_object(module.members.iter().collect());
                resolver.visit(Work::Annotations(
                    &declaration.annotations,
                    false,
                    ConstLevel::None,
                ))?;
                resolver.visit(Work::EndObject(previous))?;
            }
            if resolver.amending_module {
                resolver.amending_members(&module.members)?;
            } else {
                resolver.members(&module.members, false)?;
            }
        }
        Ok::<(), Diagnostic>(())
    })();
    let Resolver {
        preparation,
        function_owners,
        names,
        const_names,
        custom_this,
        custom_functions,
        diagnostic_owner_requested,
        ..
    } = resolver;
    let error = validation.err().map(|mut error| {
        error.module_name = module
            .declaration
            .as_ref()
            .and_then(|d| d.name.as_ref())
            .map(|name| Box::new(PklString::join(name, ".")));
        error
    });
    let resolved = ResolvedModule {
        module,
        standard_library,
        preparation,
        function_owners,
        preparation_is_evaluation: evaluation_preparation,
        preparation_failure: None,
        names,
        const_names,
        custom_this,
        custom_functions,
    };
    (resolved, error, diagnostic_owner_requested)
}

/// Resolve a separately parsed expression in a synthetic module scope. The
/// actual module is the runtime receiver, not this expression's lexical owner.
/// Original local/import variables therefore stay invisible to the expression.
/// Declared types are resolved separately through the actual runtime owner.
pub(crate) fn resolve_session_expression<'a>(
    _module: &Module,
    text: impl Into<SourceSlice<'a>>,
    parsed: &crate::parser::ParsedExpression,
) -> Result<ResolvedModule, Diagnostic> {
    let text = text.into();
    let mut resolver = Resolver {
        function_owners: BTreeMap::new(),
        preparation: Vec::new(),
        deferred_diagnostics: &parsed.deferred_diagnostics,
        source: text,
        evaluation_preparation: true,
        standard_library: false,
        scopes: Vec::new(),
        folded_integer_spans: &parsed.folded_integer_spans,
        names: BTreeMap::new(),
        const_names: Default::default(),
        custom_this: Default::default(),
        custom_functions: Default::default(),
        const_level: ConstLevel::None,
        object_levels: Vec::new(),
        module_const_owners: Vec::new(),
        object_receiver_lambdas: Vec::new(),
        receiver_is_lambda: false,
        const_scopes: Vec::new(),
        entry_counts: BTreeMap::new(),
        member_path: Vec::new(),
        diagnostic_path: Vec::new(),
        diagnostic_owners: diagnostic_owner::OwnerScopes::new(true),
        diagnostic_owner_requested: false,
        amending_module: false,
    };
    resolver.enter_object(Vec::new());
    let preparation_failure = resolver
        .visit(Work::Expression(&parsed.expression, false))
        .err()
        .map(|mut error| {
            // AstBuilder diagnostics use the synthetic ModuleInfo name. The
            // expression's runtime SimpleRootNode still has an empty name.
            error.module_name = Some(Box::new("repl:text".into()));
            error
        });
    Ok(ResolvedModule {
        standard_library: false,
        preparation: resolver.preparation,
        function_owners: resolver.function_owners,
        preparation_is_evaluation: true,
        preparation_failure,
        module: Arc::new(Module {
            deferred_diagnostics: Vec::new(),
            source: SourceText::from(text.to_pkl_string()),
            declaration: None,
            members: Vec::new(),
            folded_integer_spans: parsed.folded_integer_spans.clone(),
            interpolation_spans: parsed.interpolation_spans.clone(),
        }),
        names: resolver.names,
        const_names: resolver.const_names,
        custom_this: resolver.custom_this,
        custom_functions: resolver.custom_functions,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_module;

    #[test]
    fn bundled_stdlib_resolution_preserves_external_and_generic_declarations() {
        for source in [
            crate::stdlib_sources::base(),
            crate::stdlib_sources::reflect(),
        ] {
            let parsed = parse_module(&source.text).unwrap();
            let resolved = resolve_stdlib_module(&parsed).unwrap();
            assert!(resolved.standard_library);
            assert_eq!(*resolved.module, parsed, "{}", source.uri);
        }

        let parsed = parse_module(
            "external class Box<out T> { value: T; external function get<U>(value: U): T }\n\
             external function pass<V>(value: V): V\noutside: T\n",
        )
        .unwrap();
        let resolved = resolve_stdlib_module(&parsed).unwrap();
        assert_eq!(*resolved.module, parsed);
        let ExprKind::Class { members, .. } = &parsed.members[0].expr.kind else {
            panic!("generic class");
        };
        assert!(
            !resolved
                .names
                .contains_key(&members[0].ty.as_ref().unwrap().span)
        );
        assert_eq!(
            resolved
                .names
                .get(&parsed.members[2].ty.as_ref().unwrap().span),
            Some(&Binding::ImplicitThis),
            "class/method type parameters must not escape their declaration"
        );
    }

    #[test]
    fn user_resolution_keeps_stdlib_restrictions_and_diagnostic_metadata() {
        for (source, message) in [
            (
                "external class C {}",
                "External members can only be defined by standard library modules.",
            ),
            (
                "class C<T> {}",
                "Only standard library members can have type parameters.",
            ),
            (
                "function f<T>(value: T): T = value",
                "Only standard library members can have type parameters.",
            ),
        ] {
            let parsed = parse_module(source).unwrap();
            let error = resolve_module(&parsed).unwrap_err();
            let prepared = resolve_module_for_evaluation(&parsed);
            assert!(!prepared.standard_library);
            let deferred = prepared.preparation_failure.unwrap();
            assert_eq!(error.message.to_java_utf8(), message);
            assert_eq!(deferred.message, error.message);
            assert_eq!(deferred.span, error.span);
            assert_eq!(parsed.source.as_str(), Some(source));
        }
        let alias = parse_module("typealias Values<T> = List<T>").unwrap();
        assert!(resolve_module(&alias).is_ok());
    }

    #[test]
    fn trusted_context_does_not_disable_other_declaration_validation() {
        for (source, message) in [
            (
                "hidden function f() = 1",
                "Modifier `hidden` is not applicable to methods.",
            ),
            (
                "local hidden value = 1",
                "Modifier `hidden` is redundant here; just use `local`.",
            ),
        ] {
            let parsed = parse_module(source).unwrap();
            let ordinary = resolve_module(&parsed).unwrap_err();
            let trusted = resolve_stdlib_module(&parsed).unwrap_err();
            assert_eq!(trusted, ordinary);
            assert_eq!(trusted.message.to_java_utf8(), message);
        }
        for source in ["class C<T, T> {}", "function f<T, T>() = 1"] {
            let parsed = parse_module(source).unwrap();
            let error = resolve_stdlib_module(&parsed).unwrap_err();
            assert_eq!(
                error.message.to_java_utf8(),
                "Duplicate type parameter `T`."
            );
        }
    }
}
