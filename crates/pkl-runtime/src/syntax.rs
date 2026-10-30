//! Shared source syntax. Unsupported runtime features remain explicit errors.
use crate::source::Span;
use crate::value::PklString;

#[derive(Clone, Debug, PartialEq)]
pub struct Module {
    /// Exact source retained for exported constraint metadata and declaration names.
    pub source: crate::source::SourceText,
    pub declaration: Option<ModuleDeclaration>,
    pub members: Vec<Member>,
    /// AST construction failures retained for ordered Host-aware preparation.
    pub deferred_diagnostics: Vec<crate::diagnostic::Diagnostic>,
    /// Numeric token locations for signed integers folded by the parser.
    /// The expression span still includes the unary minus and intervening trivia.
    pub folded_integer_spans: std::collections::BTreeMap<Span, Span>,
    /// Full interpolation delimiters for implicit toString call-site frames.
    pub interpolation_spans: std::collections::BTreeMap<Span, Span>,
}

/// Exact documentation token sections, retained in authored order. The parser
/// excludes intervening ordinary comments without losing their source offsets.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocComment {
    pub sections: Vec<Span>,
}

impl DocComment {
    /// Export the parser's single-line sections using VmUtils.exportDocComment's
    /// prefix rule. At most one ASCII space, tab, or formfeed follows `///`.
    /// Invalid source/section associations return None instead of slicing panic.
    pub fn text<'a>(&self, source: impl Into<crate::source::SourceSlice<'a>>) -> Option<PklString> {
        let source = source.into();
        let mut result = crate::pkl_string::PklStringBuilder::default();
        for (index, section) in self.sections.iter().enumerate() {
            let text = source
                .get(section.start..section.end)?
                .strip_prefix("///")?;
            let text = if matches!(text.as_bytes().first(), Some(b' ' | b'\t' | b'\x0c')) {
                text.get(1..)?
            } else {
                text
            };
            if index != 0 {
                result.push('\n');
            }
            result.push_value(&text.to_pkl_string());
        }
        Some(result.finish())
    }
}

/// Immutable source metadata for a compiled declaration body. Its owning
/// BlockMetadata supplies unit/source identity; runtime mirror identity and
/// owner/prototype objects are separate and must be retained by the heap.
#[derive(Clone, Debug, PartialEq)]
pub struct DeclarationSource {
    pub name: PklString,
    pub name_span: Span,
    pub header_span: Span,
    pub declaration_span: Span,
    pub doc_comment: Option<DocComment>,
    pub modifiers: Vec<Modifier>,
    /// Original type syntax is independent of executable check normalization.
    pub recipe: DeclarationRecipe,
}

/// Reflection retains declaration recipes instead of reconstructing aliases and
/// type variables from their expanded runtime checks. No guest value is stored.
#[derive(Clone, Debug, PartialEq)]
pub enum DeclarationRecipe {
    Property {
        ty: Option<Type>,
        undefined: bool,
    },
    Method {
        type_parameters: Vec<TypeParameter>,
        parameters: Vec<Parameter>,
        return_type: Option<Type>,
    },
    Class {
        type_parameters: Vec<TypeParameter>,
        superclass: Option<Type>,
    },
    TypeAlias {
        type_parameters: Vec<TypeParameter>,
        referent: Type,
    },
    Import {
        uri: PklString,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Member {
    pub doc_comment: Option<DocComment>,
    pub annotations: Vec<Expr>,
    pub name: PklString,
    pub name_span: Span,
    /// The first authored `{ ... }` body, excluding the property header and
    /// any later bodies. Retained separately from the synthetic super read.
    pub amendment_body_span: Option<Span>,
    pub expr: Expr,
    pub span: Span,
    pub local: bool,
    pub ty: Option<Type>,
    pub modifiers: Vec<Modifier>,
    /// Pinned class override diagnostics locate these words in the complete
    /// header source, including comments and identifiers, rather than tokens.
    pub override_fixed_span: Option<Span>,
    pub override_const_span: Option<Span>,
}

impl Member {
    /// Reflection locations use declaration headers, never initializer spans.
    pub fn declaration_source(&self) -> DeclarationSource {
        let header_span = match &self.expr.kind {
            ExprKind::Class { header_span, .. }
            | ExprKind::TypeAlias { header_span, .. }
            | ExprKind::Function {
                header_span,
                method: true,
                ..
            } => *header_span,
            _ => Span::new(
                self.span.start,
                self.ty
                    .as_ref()
                    .map_or(self.name_span.end, |ty| ty.span.end),
            ),
        };
        DeclarationSource {
            name: self.name.clone(),
            name_span: self.name_span,
            header_span,
            declaration_span: self.span,
            doc_comment: self.doc_comment.clone(),
            modifiers: self.modifiers.clone(),
            recipe: match &self.expr.kind {
                ExprKind::Class {
                    type_parameters,
                    superclass,
                    ..
                } => DeclarationRecipe::Class {
                    type_parameters: type_parameters.clone(),
                    superclass: superclass.clone(),
                },
                ExprKind::TypeAlias {
                    type_parameters,
                    ty,
                    ..
                } => DeclarationRecipe::TypeAlias {
                    type_parameters: type_parameters.clone(),
                    referent: ty.clone(),
                },
                ExprKind::Function {
                    type_parameters,
                    parameters,
                    return_type,
                    method: true,
                    ..
                } => DeclarationRecipe::Method {
                    type_parameters: type_parameters.clone(),
                    parameters: parameters.clone(),
                    return_type: return_type.clone(),
                },
                ExprKind::Import {
                    uri,
                    clause_alias: Some(_),
                    ..
                } => DeclarationRecipe::Import { uri: uri.clone() },
                _ => DeclarationRecipe::Property {
                    ty: self.ty.clone(),
                    undefined: matches!(self.expr.kind, ExprKind::Uninitialized),
                },
            },
        }
    }

    /// An authored `property { ... }` carries a synthetic super read at the
    /// property's name, including when more object bodies wrap that read.
    /// An explicit amendment expression keeps its own span.
    pub(crate) fn is_amendment(&self) -> bool {
        let mut expression = &self.expr;
        while let ExprKind::Object {
            parent: Some(parent),
            ..
        } = &expression.kind
        {
            if parent.span == self.name_span {
                return matches!(
                    &parent.kind,
                    ExprKind::Member(receiver, name)
                        if matches!(receiver.kind, ExprKind::Super) && name == &self.name
                );
            }
            expression = parent;
        }
        false
    }
}

/// Authored object members, retained in source order including generators.
#[derive(Clone, Debug, PartialEq)]
pub enum ObjectMember {
    Property(Member),
    Element {
        value: Expr,
        span: Span,
    },
    Entry {
        key: Expr,
        value: Expr,
        span: Span,
    },
    When {
        then_parameters: Vec<Parameter>,
        else_parameters: Vec<Parameter>,
        condition: Expr,
        then_members: Vec<ObjectMember>,
        else_members: Vec<ObjectMember>,
        span: Span,
    },
    For {
        body_parameters: Vec<Parameter>,
        parameters: Vec<Parameter>,
        iterable: Expr,
        members: Vec<ObjectMember>,
        span: Span,
    },
    Spread {
        value: Expr,
        nullable: bool,
        span: Span,
    },
    Predicate {
        condition: Expr,
        value: Expr,
        span: Span,
    },
}

impl ObjectMember {
    pub fn span(&self) -> Span {
        match self {
            Self::Property(member) => member.span,
            Self::Element { span, .. }
            | Self::Entry { span, .. }
            | Self::When { span, .. }
            | Self::For { span, .. }
            | Self::Spread { span, .. }
            | Self::Predicate { span, .. } => *span,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Parameter {
    pub name: PklString,
    pub ty: Option<Type>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ModuleDeclaration {
    pub doc_comment: Option<DocComment>,
    pub annotations: Vec<Expr>,
    pub name: Option<Vec<PklString>>,
    pub parent: Option<ModuleParent>,
    pub modifiers: Vec<Modifier>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ModuleParent {
    pub kind: ModuleParentKind,
    pub uri: PklString,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModuleParentKind {
    Extends,
    Amends,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TypeParameter {
    pub name: PklString,
    pub variance: Variance,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Variance {
    Invariant,
    In,
    Out,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Modifier {
    pub kind: ModifierKind,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModifierKind {
    Abstract,
    Open,
    Local,
    Hidden,
    External,
    Fixed,
    Const,
}

/// Match Pkl 0.32 AstBuilder.doVisitModifiers, including repeated tokens.
/// Upstream adds flag values rather than combining them with bitwise OR.
pub fn modifier_flags(modifiers: &[Modifier]) -> u32 {
    modifiers.iter().fold(0_u32, |flags, modifier| {
        flags.wrapping_add(modifier.kind.flag())
    })
}

impl ModifierKind {
    pub const fn flag(self) -> u32 {
        match self {
            Self::Abstract => 0x01,
            Self::Open => 0x02,
            Self::Local => 0x04,
            Self::Hidden => 0x08,
            Self::External => 0x10,
            Self::Fixed => 0x20,
            Self::Const => 0x40,
        }
    }

    pub fn spelling(self) -> &'static str {
        match self {
            Self::Abstract => "abstract",
            Self::Open => "open",
            Self::Local => "local",
            Self::Hidden => "hidden",
            Self::External => "external",
            Self::Fixed => "fixed",
            Self::Const => "const",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Type {
    /// Parenthesized constants are not the AST string-literal union optimization.
    pub parenthesized: bool,
    /// The actual constructed node excludes transparent grouping parentheses.
    pub unparenthesized_span: Option<Span>,
    pub kind: TypeKind,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TypeKind {
    Named {
        path: Vec<PklString>,
        name_spans: Vec<Span>,
        arguments: Vec<Type>,
    },
    Unknown,
    Nothing,
    Module,
    Nullable(Box<Type>),
    Union {
        types: Vec<Type>,
        default: Option<usize>,
    },
    Function {
        parameters: Vec<Type>,
        return_type: Box<Type>,
    },
    String(PklString),
    Constrained {
        base: Box<Type>,
        constraints: Vec<Expr>,
    },
}

#[derive(Debug, PartialEq)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

impl Expr {
    /// The executable expression inside any authored grouping parentheses.
    pub fn unparenthesized(&self) -> &Self {
        let mut expression = self;
        while let ExprKind::Parenthesized(inner) = &expression.kind {
            expression = inner;
        }
        expression
    }
}

// Expression syntax uses explicit traversal for cloning and destruction.
// Type syntax and generator member structure retain their parser bounds.
impl Clone for Expr {
    fn clone(&self) -> Self {
        enum Work<'a> {
            Visit(&'a Expr),
            Finish(&'a Expr, usize),
        }
        let mut work = vec![Work::Visit(self)];
        let mut values: Vec<Expr> = Vec::new();
        while let Some(task) = work.pop() {
            match task {
                Work::Visit(expr) => {
                    let children = expr.expression_children();
                    work.push(Work::Finish(expr, values.len()));
                    work.extend(children.into_iter().rev().map(Work::Visit));
                }
                Work::Finish(expr, start) => {
                    let mut children = values.split_off(start).into_iter();
                    let mut next = || children.next().expect("syntax clone child");
                    use ExprKind as E;
                    let kind = match &expr.kind {
                        E::Parenthesized(_) => E::Parenthesized(Box::new(next())),
                        E::NonNull(_) => E::NonNull(Box::new(next())),
                        E::Throw(_) => E::Throw(Box::new(next())),
                        E::Trace(_) => E::Trace(Box::new(next())),
                        E::ReadResource(_) => E::ReadResource(Box::new(next())),
                        E::ReadResourceGlob(_) => E::ReadResourceGlob(Box::new(next())),
                        E::ReadResourceOrNull(_) => E::ReadResourceOrNull(Box::new(next())),
                        E::Member(_, name) => E::Member(Box::new(next()), name.clone()),
                        E::NullableMember(_, name) => {
                            E::NullableMember(Box::new(next()), name.clone())
                        }
                        E::Unary { op, .. } => E::Unary {
                            op: *op,
                            operand: Box::new(next()),
                        },
                        E::Binary { op, .. } => E::Binary {
                            op: *op,
                            left: Box::new(next()),
                            right: Box::new(next()),
                        },
                        E::Subscript { .. } => E::Subscript {
                            receiver: Box::new(next()),
                            key: Box::new(next()),
                        },
                        E::Call { arguments, .. } => E::Call {
                            callee: Box::new(next()),
                            arguments: (0..arguments.len()).map(|_| next()).collect(),
                        },
                        E::TypeCheck { ty, .. } => E::TypeCheck {
                            value: Box::new(next()),
                            ty: ty.clone(),
                        },
                        E::TypeCast { ty, .. } => E::TypeCast {
                            value: Box::new(next()),
                            ty: ty.clone(),
                        },
                        E::InterpolatedString(parts) => {
                            E::InterpolatedString((0..parts.len()).map(|_| next()).collect())
                        }
                        E::If { .. } => E::If {
                            condition: Box::new(next()),
                            then_branch: Box::new(next()),
                            else_branch: Box::new(next()),
                        },
                        E::Let { name, ty, .. } => E::Let {
                            name: name.clone(),
                            ty: ty.clone(),
                            value: Box::new(next()),
                            body: Box::new(next()),
                        },
                        E::Function {
                            type_parameters,
                            type_parameters_span,
                            header_span,
                            parameters,
                            return_type,
                            method,
                            ..
                        } => E::Function {
                            type_parameters: type_parameters.clone(),
                            type_parameters_span: type_parameters_span.clone(),
                            header_span: *header_span,
                            parameters: parameters.clone(),
                            return_type: return_type.clone(),
                            method: *method,
                            body: Box::new(next()),
                        },
                        E::Object {
                            parent,
                            parameters,
                            members,
                        } => E::Object {
                            parameters: parameters.clone(),
                            parent: parent.as_ref().map(|_| Box::new(next())),
                            members: clone_object_members(members, &mut next),
                        },
                        E::Annotation {
                            parameters,
                            ty,
                            members,
                        } => E::Annotation {
                            parameters: parameters.clone(),
                            ty: ty.clone(),
                            members: clone_object_members(members, &mut next),
                        },
                        E::New {
                            ty,
                            parameters,
                            members,
                        } => E::New {
                            parameters: parameters.clone(),
                            ty: ty.clone(),
                            members: clone_object_members(members, &mut next),
                        },
                        E::Class {
                            type_parameters,
                            superclass,
                            members,
                            header_span,
                        } => E::Class {
                            type_parameters: type_parameters.clone(),
                            superclass: superclass.clone(),
                            members: members
                                .iter()
                                .map(|member| clone_member(member, next()))
                                .collect(),
                            header_span: *header_span,
                        },
                        kind => kind.clone(),
                    };
                    values.push(Self {
                        kind,
                        span: expr.span,
                    });
                }
            }
        }
        values.pop().expect("syntax clone root")
    }
}

impl Expr {
    pub(crate) fn expression_children(&self) -> Vec<&Expr> {
        use ExprKind as E;
        match &self.kind {
            E::Parenthesized(value)
            | E::NonNull(value)
            | E::Throw(value)
            | E::Trace(value)
            | E::ReadResourceGlob(value)
            | E::ReadResource(value)
            | E::ReadResourceOrNull(value)
            | E::Member(value, _)
            | E::NullableMember(value, _)
            | E::Unary { operand: value, .. }
            | E::TypeCheck { value, .. }
            | E::TypeCast { value, .. } => vec![value],
            E::Binary { left, right, .. } => vec![left, right],
            E::Subscript { receiver, key } => vec![receiver, key],
            E::Call { callee, arguments } => {
                std::iter::once(callee.as_ref()).chain(arguments).collect()
            }
            E::InterpolatedString(parts) => parts.iter().collect(),
            E::If {
                condition,
                then_branch,
                else_branch,
            } => vec![condition, then_branch, else_branch],
            E::Let { value, body, .. } => vec![value, body],
            E::Function { body, .. } => vec![body],
            E::Object {
                parent, members, ..
            } => {
                let mut children = parent.iter().map(AsRef::as_ref).collect::<Vec<_>>();
                object_expressions(members, &mut children);
                children
            }
            E::New { members, .. } | E::Annotation { members, .. } => {
                let mut children = Vec::new();
                object_expressions(members, &mut children);
                children
            }
            E::Class { members, .. } => members.iter().map(|member| &member.expr).collect(),
            _ => Vec::new(),
        }
    }
}

fn clone_member(member: &Member, expr: Expr) -> Member {
    Member {
        doc_comment: member.doc_comment.clone(),
        annotations: member.annotations.clone(),
        name: member.name.clone(),
        name_span: member.name_span,
        amendment_body_span: member.amendment_body_span,
        expr,
        span: member.span,
        local: member.local,
        ty: member.ty.clone(),
        modifiers: member.modifiers.clone(),
        override_fixed_span: member.override_fixed_span,
        override_const_span: member.override_const_span,
    }
}

fn object_expressions<'a>(members: &'a [ObjectMember], expressions: &mut Vec<&'a Expr>) {
    let mut work = members.iter().rev().collect::<Vec<_>>();
    while let Some(member) = work.pop() {
        match member {
            ObjectMember::Property(member) => expressions.push(&member.expr),
            ObjectMember::Element { value, .. } | ObjectMember::Spread { value, .. } => {
                expressions.push(value)
            }
            ObjectMember::Entry { key, value, .. } => expressions.extend([key, value]),
            ObjectMember::Predicate {
                condition, value, ..
            } => expressions.extend([condition, value]),
            ObjectMember::When {
                condition,
                then_members,
                else_members,
                ..
            } => {
                expressions.push(condition);
                work.extend(else_members.iter().rev());
                work.extend(then_members.iter().rev());
            }
            ObjectMember::For {
                iterable, members, ..
            } => {
                expressions.push(iterable);
                work.extend(members.iter().rev());
            }
        }
    }
}

fn clone_object_members(
    members: &[ObjectMember],
    next: &mut impl FnMut() -> Expr,
) -> Vec<ObjectMember> {
    members
        .iter()
        .map(|member| match member {
            ObjectMember::Property(member) => ObjectMember::Property(clone_member(member, next())),
            ObjectMember::Element { span, .. } => ObjectMember::Element {
                value: next(),
                span: *span,
            },
            ObjectMember::Entry { span, .. } => ObjectMember::Entry {
                key: next(),
                value: next(),
                span: *span,
            },
            ObjectMember::Spread { nullable, span, .. } => ObjectMember::Spread {
                value: next(),
                nullable: *nullable,
                span: *span,
            },
            ObjectMember::Predicate { span, .. } => ObjectMember::Predicate {
                condition: next(),
                value: next(),
                span: *span,
            },
            ObjectMember::When {
                then_members,
                else_members,
                then_parameters,
                else_parameters,
                span,
                ..
            } => ObjectMember::When {
                then_parameters: then_parameters.clone(),
                else_parameters: else_parameters.clone(),
                condition: next(),
                then_members: clone_object_members(then_members, next),
                else_members: clone_object_members(else_members, next),
                span: *span,
            },
            ObjectMember::For {
                parameters,
                body_parameters,
                members,
                span,
                ..
            } => ObjectMember::For {
                body_parameters: body_parameters.clone(),
                parameters: parameters.clone(),
                iterable: next(),
                members: clone_object_members(members, next),
                span: *span,
            },
        })
        .collect()
}

fn detach_members(members: Vec<ObjectMember>, expressions: &mut Vec<Expr>) {
    let mut work = members;
    while let Some(member) = work.pop() {
        match member {
            ObjectMember::Property(member) => expressions.push(member.expr),
            ObjectMember::Element { value, .. } | ObjectMember::Spread { value, .. } => {
                expressions.push(value)
            }
            ObjectMember::Entry { key, value, .. } => expressions.extend([key, value]),
            ObjectMember::Predicate {
                condition, value, ..
            } => expressions.extend([condition, value]),
            ObjectMember::When {
                condition,
                mut then_members,
                mut else_members,
                ..
            } => {
                expressions.push(condition);
                work.append(&mut then_members);
                work.append(&mut else_members);
            }
            ObjectMember::For {
                iterable,
                mut members,
                ..
            } => {
                expressions.push(iterable);
                work.append(&mut members);
            }
        }
    }
}

impl Drop for Expr {
    fn drop(&mut self) {
        fn detach(kind: ExprKind, pending: &mut Vec<Expr>) {
            use ExprKind as E;
            match kind {
                E::Parenthesized(value)
                | E::NonNull(value)
                | E::Throw(value)
                | E::Trace(value)
                | E::ReadResourceGlob(value)
                | E::ReadResource(value)
                | E::ReadResourceOrNull(value)
                | E::Member(value, _)
                | E::NullableMember(value, _)
                | E::Unary { operand: value, .. }
                | E::TypeCheck { value, .. }
                | E::TypeCast { value, .. } => pending.push(*value),
                E::Binary { left, right, .. } => {
                    pending.push(*left);
                    pending.push(*right);
                }
                E::Subscript { receiver, key } => {
                    pending.push(*receiver);
                    pending.push(*key);
                }
                E::Call {
                    callee,
                    mut arguments,
                } => {
                    pending.push(*callee);
                    pending.append(&mut arguments);
                }
                E::InterpolatedString(mut parts) => pending.append(&mut parts),
                E::If {
                    condition,
                    then_branch,
                    else_branch,
                } => {
                    pending.push(*condition);
                    pending.push(*then_branch);
                    pending.push(*else_branch);
                }
                E::Let { value, body, .. } => {
                    pending.push(*value);
                    pending.push(*body);
                }
                E::Function { body, .. } => pending.push(*body),
                E::Class { members, .. } => {
                    pending.extend(members.into_iter().map(|member| member.expr))
                }
                E::Object {
                    parent, members, ..
                } => {
                    pending.extend(parent.map(|parent| *parent));
                    detach_members(members, pending);
                }
                E::New { members, .. } | E::Annotation { members, .. } => {
                    detach_members(members, pending)
                }
                _ => {}
            }
        }
        let mut pending = Vec::new();
        detach(
            std::mem::replace(&mut self.kind, ExprKind::Null),
            &mut pending,
        );
        while let Some(mut expression) = pending.pop() {
            detach(
                std::mem::replace(&mut expression.kind, ExprKind::Null),
                &mut pending,
            );
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ExprKind {
    /// A module import declaration, lowered to a local const module member.
    /// Loading unsupported module schemes remains an explicit runtime error.
    Import {
        uri: PklString,
        glob: bool,
        uri_span: Span,
        /// None denotes an expression; Some(None) is a clause requiring key-based inference.
        clause_alias: Option<Option<PklString>>,
    },
    Int(i64),
    Float(f64),
    Null,
    String(PklString),
    InterpolatedString(Vec<Expr>),
    Bool(bool),
    Name(PklString),
    This,
    Outer,
    Module,
    Super,
    /// Inherited value at the current entry key, used by `[key] { ... }`.
    EntryParent,
    Parenthesized(Box<Expr>),
    NonNull(Box<Expr>),
    Throw(Box<Expr>),
    Trace(Box<Expr>),
    ReadResource(Box<Expr>),
    ReadResourceGlob(Box<Expr>),
    ReadResourceOrNull(Box<Expr>),
    TypeCheck {
        value: Box<Expr>,
        ty: Type,
    },
    TypeCast {
        value: Box<Expr>,
        ty: Type,
    },
    NullableMember(Box<Expr>, PklString),
    Member(Box<Expr>, PklString),
    Subscript {
        receiver: Box<Expr>,
        key: Box<Expr>,
    },
    Unary {
        op: UnaryOp,
        operand: Box<Expr>,
    },
    Binary {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    If {
        condition: Box<Expr>,
        then_branch: Box<Expr>,
        else_branch: Box<Expr>,
    },
    Function {
        type_parameters: Vec<TypeParameter>,
        type_parameters_span: Option<Box<Span>>,
        header_span: Span,
        parameters: Vec<Parameter>,
        return_type: Option<Type>,
        body: Box<Expr>,
        method: bool,
    },
    Call {
        callee: Box<Expr>,
        arguments: Vec<Expr>,
    },
    Let {
        name: PklString,
        ty: Option<Type>,
        value: Box<Expr>,
        body: Box<Expr>,
    },
    Class {
        type_parameters: Vec<TypeParameter>,
        superclass: Option<Type>,
        members: Vec<Member>,
        header_span: Span,
    },
    TypeAlias {
        type_parameters: Vec<TypeParameter>,
        ty: Type,
        header_span: Span,
    },
    Annotation {
        parameters: Vec<Parameter>,
        ty: Type,
        members: Vec<ObjectMember>,
    },
    New {
        parameters: Vec<Parameter>,
        ty: Type,
        members: Vec<ObjectMember>,
    },
    Uninitialized,
    Object {
        parameters: Vec<Parameter>,
        parent: Option<Box<Expr>>,
        members: Vec<ObjectMember>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnaryOp {
    Negate,
    Not,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Subtract,
    Multiply,
    Divide,
    Power,
    IntegerDivide,
    Remainder,
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    And,
    Or,
    NullCoalesce,
    Pipe,
}
