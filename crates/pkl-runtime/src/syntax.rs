//! Deliberately bounded M1 syntax; unsupported constructs are errors.
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
}

#[derive(Clone, Debug, PartialEq)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ExprKind {
    Int(i64),
    String(String),
    Bool(bool),
    Name(String),
    This,
    Super,
    Member(Box<Expr>, String),
    Binary {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
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
pub enum BinaryOp {
    Add,
    Subtract,
    Multiply,
    Divide,
}
