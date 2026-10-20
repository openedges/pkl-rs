//! Shared source syntax. Unsupported runtime features remain explicit errors.
use crate::source::Span;

#[derive(Clone, Debug, PartialEq)]
pub struct Module {
    pub declaration: Option<ModuleDeclaration>,
    pub members: Vec<Member>,
    /// Numeric token locations for signed integers folded by the parser.
    /// The expression span still includes the unary minus and intervening trivia.
    pub folded_integer_spans: std::collections::BTreeMap<Span, Span>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Member {
    pub name: String,
    pub name_span: Span,
    pub expr: Expr,
    pub span: Span,
    pub local: bool,
    pub ty: Option<Type>,
    pub modifiers: Vec<Modifier>,
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
        condition: Expr,
        then_members: Vec<ObjectMember>,
        else_members: Vec<ObjectMember>,
        span: Span,
    },
    For {
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
    pub name: String,
    pub ty: Option<Type>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ModuleDeclaration {
    pub name: Option<Vec<String>>,
    pub parent: Option<ModuleParent>,
    pub modifiers: Vec<Modifier>,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ModuleParent {
    pub kind: ModuleParentKind,
    pub uri: String,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModuleParentKind {
    Extends,
    Amends,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TypeParameter {
    pub name: String,
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
    pub kind: TypeKind,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TypeKind {
    Named {
        path: Vec<String>,
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
    String(String),
    Constrained {
        base: Box<Type>,
        constraints: Vec<Expr>,
    },
}

#[derive(Clone, Debug, PartialEq)]
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

#[derive(Clone, Debug, PartialEq)]
pub enum ExprKind {
    Int(i64),
    Float(f64),
    Null,
    String(String),
    InterpolatedString(Vec<Expr>),
    Bool(bool),
    Name(String),
    This,
    Outer,
    Module,
    Super,
    /// Inherited value at the current entry key, used by `[key] { ... }`.
    EntryParent,
    Parenthesized(Box<Expr>),
    NonNull(Box<Expr>),
    Throw(Box<Expr>),
    TypeCheck {
        value: Box<Expr>,
        ty: Type,
    },
    TypeCast {
        value: Box<Expr>,
        ty: Type,
    },
    NullableMember(Box<Expr>, String),
    Member(Box<Expr>, String),
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
        name: String,
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
    },
    New {
        ty: Type,
        members: Vec<ObjectMember>,
    },
    Uninitialized,
    Object {
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
