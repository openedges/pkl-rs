//! Shared source syntax. Unsupported runtime features remain explicit errors.
use crate::source::Span;

#[derive(Clone, Debug, PartialEq)]
pub struct Module {
    pub members: Vec<Member>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Member {
    pub name: String,
    pub expr: Expr,
    pub span: Span,
    pub local: bool,
    pub ty: Option<Type>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Parameter {
    pub name: String,
    pub ty: Option<Type>,
    pub span: Span,
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
        value: Box<Expr>,
        body: Box<Expr>,
    },
    Object {
        parent: Option<Box<Expr>>,
        members: Vec<Member>,
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
}
