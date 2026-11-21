//! Lossless generic CST nodes mirroring `org.pkl.parser.syntax.generic.{Node, NodeType, FullSpan}`.

use crate::FullSpan;

impl FullSpan {
    pub(crate) const fn end_with(self, end: Self) -> Self {
        Self {
            char_index: self.char_index,
            length: end.char_index - self.char_index + end.length,
            line_begin: self.line_begin,
            col_begin: self.col_begin,
            line_end: end.line_end,
            col_end: end.col_end,
        }
    }

    pub(crate) const fn is_same_line(self, other: Self) -> bool {
        self.line_end == other.line_begin
    }

    pub(crate) const fn stop_span(self) -> Self {
        Self {
            char_index: (self.char_index + self.length).saturating_sub(1),
            length: 1,
            line_begin: self.line_end,
            col_begin: self.col_end,
            line_end: self.line_end,
            col_end: self.col_end,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum NodeType {
    Terminal,
    Shebang,
    // affixes
    LineComment,
    BlockComment,
    Semicolon,

    Module,
    DocComment,
    DocCommentLine,
    Modifier,
    ModifierList,
    AmendsClause,
    ExtendsClause,
    ModuleDeclaration,
    ModuleDefinition,
    Annotation,
    Identifier,
    QualifiedIdentifier,
    Import,
    ImportAlias,
    ImportList,
    Typealias,
    TypealiasHeader,
    TypealiasBody,
    Class,
    ClassHeader,
    ClassHeaderExtends,
    ClassBody,
    ClassBodyElements,
    ClassMethod,
    ClassMethodHeader,
    ClassMethodBody,
    ClassProperty,
    ClassPropertyHeader,
    ClassPropertyHeaderBegin,
    ClassPropertyBody,
    ObjectBody,
    ObjectMemberList,
    Parameter,
    TypeAnnotation,
    ParameterList,
    ParameterListElements,
    TypeParameterList,
    TypeParameterListElements,
    ArgumentList,
    ArgumentListElements,
    TypeArgumentList,
    TypeArgumentListElements,
    ObjectParameterList,
    TypeParameter,
    StringChars,
    Operator,
    StringNewline,
    StringContinuation,
    StringEscape,

    // members
    ObjectElement,
    ObjectProperty,
    ObjectPropertyHeader,
    ObjectPropertyHeaderBegin,
    ObjectPropertyBody,
    ObjectMethod,
    MemberPredicate,
    ObjectEntry,
    ObjectEntryHeader,
    ObjectSpread,
    WhenGenerator,
    WhenGeneratorHeader,
    ForGenerator,
    ForGeneratorHeader,
    ForGeneratorHeaderDefinition,
    ForGeneratorHeaderDefinitionHeader,

    // expressions
    ThisExpr,
    OuterExpr,
    ModuleExpr,
    NullExpr,
    ThrowExpr,
    TraceExpr,
    ImportExpr,
    ReadExpr,
    NewExpr,
    NewHeader,
    UnaryMinusExpr,
    LogicalNotExpr,
    FunctionLiteralExpr,
    FunctionLiteralBody,
    ParenthesizedExpr,
    ParenthesizedExprElements,
    SuperSubscriptExpr,
    SuperAccessExpr,
    SubscriptExpr,
    QualifiedAccessExpr,
    IfExpr,
    IfHeader,
    IfCondition,
    IfConditionExpr,
    IfThenExpr,
    IfElseExpr,
    LetExpr,
    LetParameterDefinition,
    LetParameter,
    BoolLiteralExpr,
    IntLiteralExpr,
    FloatLiteralExpr,
    SingleLineStringLiteralExpr,
    MultiLineStringLiteralExpr,
    UnqualifiedAccessExpr,
    NonNullExpr,
    AmendsExpr,
    BinaryOpExpr,

    // types
    UnknownType,
    NothingType,
    ModuleType,
    UnionType,
    FunctionType,
    FunctionTypeParameters,
    ParenthesizedType,
    ParenthesizedTypeElements,
    DeclaredType,
    NullableType,
    StringConstantType,
    ConstrainedType,
    ConstrainedTypeConstraint,
    ConstrainedTypeElements,
}

impl NodeType {
    pub(crate) const fn is_affix(self) -> bool {
        matches!(
            self,
            Self::LineComment | Self::BlockComment | Self::Semicolon
        )
    }

    pub(crate) const fn is_expression(self) -> bool {
        matches!(
            self,
            Self::ThisExpr
                | Self::OuterExpr
                | Self::ModuleExpr
                | Self::NullExpr
                | Self::ThrowExpr
                | Self::TraceExpr
                | Self::ImportExpr
                | Self::ReadExpr
                | Self::NewExpr
                | Self::UnaryMinusExpr
                | Self::LogicalNotExpr
                | Self::FunctionLiteralExpr
                | Self::ParenthesizedExpr
                | Self::SuperSubscriptExpr
                | Self::SuperAccessExpr
                | Self::SubscriptExpr
                | Self::QualifiedAccessExpr
                | Self::IfExpr
                | Self::LetExpr
                | Self::BoolLiteralExpr
                | Self::IntLiteralExpr
                | Self::FloatLiteralExpr
                | Self::SingleLineStringLiteralExpr
                | Self::MultiLineStringLiteralExpr
                | Self::UnqualifiedAccessExpr
                | Self::NonNullExpr
                | Self::AmendsExpr
                | Self::BinaryOpExpr
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Node {
    pub(crate) kind: NodeType,
    pub(crate) span: FullSpan,
    pub(crate) children: Vec<Node>,
}

impl Node {
    pub(crate) fn leaf(kind: NodeType, span: FullSpan) -> Self {
        Self {
            kind,
            span,
            children: Vec::new(),
        }
    }

    pub(crate) fn with_span(kind: NodeType, span: FullSpan, children: Vec<Node>) -> Self {
        Self {
            kind,
            span,
            children,
        }
    }

    pub(crate) fn from_children(kind: NodeType, children: Vec<Node>) -> Self {
        assert!(!children.is_empty(), "No children or span given for node");
        let first = children[0].span;
        let last = children[children.len() - 1].span;
        Self {
            kind,
            span: first.end_with(last),
            children,
        }
    }

    pub(crate) fn text(&self, source: &[u16]) -> String {
        String::from_utf16_lossy(
            &source[self.span.char_index..self.span.char_index + self.span.length],
        )
    }

    pub(crate) fn find_child_by_type(&self, kind: NodeType) -> Option<&Node> {
        self.children.iter().find(|child| child.kind == kind)
    }
}
