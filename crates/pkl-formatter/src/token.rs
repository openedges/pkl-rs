//! Token and operator definitions mirroring `org.pkl.parser.Token` and `Operator`.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Token {
    Abstract,
    Amends,
    As,
    Class,
    Const,
    Else,
    Extends,
    External,
    False,
    Fixed,
    For,
    Function,
    Hidden,
    If,
    Import,
    ImportStar,
    In,
    Is,
    Let,
    Local,
    Module,
    New,
    Nothing,
    Null,
    Open,
    Out,
    Outer,
    Read,
    ReadStar,
    ReadQuestion,
    Super,
    This,
    Throw,
    Trace,
    True,
    TypeAlias,
    Unknown,
    When,

    // reserved for future use
    Protected,
    Override,
    Record,
    Delete,
    Case,
    Switch,
    Vararg,

    // punctuation
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBrack,
    RBrack,
    LPred,
    Comma,
    Dot,
    QDot,
    Coalesce,
    NonNull,
    At,
    Assign,
    Gt,
    Lt,

    // rest
    Not,
    Question,
    Colon,
    Arrow,
    Equal,
    NotEqual,
    Lte,
    Gte,
    And,
    Or,
    Plus,
    Minus,
    Pow,
    Star,
    Div,
    IntDiv,
    Mod,
    Union,
    Pipe,
    Spread,
    QSpread,
    Underscore,
    Eof,
    Semicolon,

    Int,
    Float,
    Bin,
    Oct,
    Hex,
    Identifier,
    LineComment,
    BlockComment,
    DocComment,
    Shebang,
    InterpolationStart,
    StringStart,
    StringMultiStart,
    StringNewline,
    StringEscapeNewline,
    StringEscapeTab,
    StringEscapeReturn,
    StringEscapeQuote,
    StringEscapeBackslash,
    StringEscapeUnicode,
    StringEscapeContinuation,
    StringEnd,
    StringPart,
}

impl Token {
    pub(crate) const fn is_modifier(self) -> bool {
        matches!(
            self,
            Self::External
                | Self::Abstract
                | Self::Open
                | Self::Local
                | Self::Hidden
                | Self::Fixed
                | Self::Const
        )
    }

    pub(crate) const fn is_keyword(self) -> bool {
        matches!(
            self,
            Self::Abstract
                | Self::Amends
                | Self::As
                | Self::Class
                | Self::Const
                | Self::Else
                | Self::Extends
                | Self::External
                | Self::False
                | Self::Fixed
                | Self::For
                | Self::Function
                | Self::Hidden
                | Self::If
                | Self::Import
                | Self::ImportStar
                | Self::In
                | Self::Is
                | Self::Let
                | Self::Local
                | Self::Module
                | Self::New
                | Self::Nothing
                | Self::Null
                | Self::Open
                | Self::Out
                | Self::Outer
                | Self::Read
                | Self::ReadStar
                | Self::ReadQuestion
                | Self::Super
                | Self::This
                | Self::Throw
                | Self::Trace
                | Self::True
                | Self::TypeAlias
                | Self::Unknown
                | Self::When
                | Self::Underscore
                | Self::Protected
                | Self::Override
                | Self::Record
                | Self::Delete
                | Self::Case
                | Self::Switch
                | Self::Vararg
        )
    }

    pub(crate) const fn is_affix(self) -> bool {
        matches!(
            self,
            Self::LineComment | Self::BlockComment | Self::Semicolon
        )
    }

    /// Matches `org.pkl.parser.Token.text()` (`UNDERSCORE` -> `"_"`, others `name().toLowerCase(ROOT)`).
    pub(crate) const fn text(self) -> &'static str {
        match self {
            Self::Abstract => "abstract",
            Self::Amends => "amends",
            Self::As => "as",
            Self::Class => "class",
            Self::Const => "const",
            Self::Else => "else",
            Self::Extends => "extends",
            Self::External => "external",
            Self::False => "false",
            Self::Fixed => "fixed",
            Self::For => "for",
            Self::Function => "function",
            Self::Hidden => "hidden",
            Self::If => "if",
            Self::Import => "import",
            Self::ImportStar => "import_star",
            Self::In => "in",
            Self::Is => "is",
            Self::Let => "let",
            Self::Local => "local",
            Self::Module => "module",
            Self::New => "new",
            Self::Nothing => "nothing",
            Self::Null => "null",
            Self::Open => "open",
            Self::Out => "out",
            Self::Outer => "outer",
            Self::Read => "read",
            Self::ReadStar => "read_star",
            Self::ReadQuestion => "read_question",
            Self::Super => "super",
            Self::This => "this",
            Self::Throw => "throw",
            Self::Trace => "trace",
            Self::True => "true",
            Self::TypeAlias => "type_alias",
            Self::Unknown => "unknown",
            Self::When => "when",
            Self::Protected => "protected",
            Self::Override => "override",
            Self::Record => "record",
            Self::Delete => "delete",
            Self::Case => "case",
            Self::Switch => "switch",
            Self::Vararg => "vararg",
            Self::Underscore => "_",
            Self::LParen => "lparen",
            Self::RParen => "rparen",
            Self::LBrace => "lbrace",
            Self::RBrace => "rbrace",
            Self::LBrack => "lbrack",
            Self::RBrack => "rbrack",
            Self::LPred => "lpred",
            Self::Comma => "comma",
            Self::Dot => "dot",
            Self::QDot => "qdot",
            Self::Coalesce => "coalesce",
            Self::NonNull => "non_null",
            Self::At => "at",
            Self::Assign => "assign",
            Self::Gt => "gt",
            Self::Lt => "lt",
            Self::Not => "not",
            Self::Question => "question",
            Self::Colon => "colon",
            Self::Arrow => "arrow",
            Self::Equal => "equal",
            Self::NotEqual => "not_equal",
            Self::Lte => "lte",
            Self::Gte => "gte",
            Self::And => "and",
            Self::Or => "or",
            Self::Plus => "plus",
            Self::Minus => "minus",
            Self::Pow => "pow",
            Self::Star => "star",
            Self::Div => "div",
            Self::IntDiv => "int_div",
            Self::Mod => "mod",
            Self::Union => "union",
            Self::Pipe => "pipe",
            Self::Spread => "spread",
            Self::QSpread => "qspread",
            Self::Eof => "eof",
            Self::Semicolon => "semicolon",
            Self::Int => "int",
            Self::Float => "float",
            Self::Bin => "bin",
            Self::Oct => "oct",
            Self::Hex => "hex",
            Self::Identifier => "identifier",
            Self::LineComment => "line_comment",
            Self::BlockComment => "block_comment",
            Self::DocComment => "doc_comment",
            Self::Shebang => "shebang",
            Self::InterpolationStart => "interpolation_start",
            Self::StringStart => "string_start",
            Self::StringMultiStart => "string_multi_start",
            Self::StringNewline => "string_newline",
            Self::StringEscapeNewline => "string_escape_newline",
            Self::StringEscapeTab => "string_escape_tab",
            Self::StringEscapeReturn => "string_escape_return",
            Self::StringEscapeQuote => "string_escape_quote",
            Self::StringEscapeBackslash => "string_escape_backslash",
            Self::StringEscapeUnicode => "string_escape_unicode",
            Self::StringEscapeContinuation => "string_escape_continuation",
            Self::StringEnd => "string_end",
            Self::StringPart => "string_part",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Operator {
    NullCoalesce,
    Pipe,
    Or,
    And,
    EqEq,
    NotEq,
    Is,
    As,
    Lt,
    Gt,
    Lte,
    Gte,
    Plus,
    Minus,
    Mult,
    Div,
    IntDiv,
    Mod,
    Pow,
    NonNull,
    Subscript,
    Dot,
    QDot,
}

impl Operator {
    pub(crate) const fn prec(self) -> u8 {
        match self {
            Self::NullCoalesce => 1,
            Self::Pipe => 2,
            Self::Or => 3,
            Self::And => 4,
            Self::EqEq | Self::NotEq => 5,
            Self::Is | Self::As => 6,
            Self::Lt | Self::Gt | Self::Lte | Self::Gte => 7,
            Self::Plus | Self::Minus => 8,
            Self::Mult | Self::Div | Self::IntDiv | Self::Mod => 9,
            Self::Pow => 10,
            Self::NonNull => 16,
            Self::Subscript => 18,
            Self::Dot | Self::QDot => 20,
        }
    }

    pub(crate) const fn is_left_assoc(self) -> bool {
        !matches!(self, Self::NullCoalesce | Self::Pow)
    }

    pub(crate) fn from_token(token: Token) -> Option<Self> {
        match token {
            Token::Pow => Some(Self::Pow),
            Token::Star => Some(Self::Mult),
            Token::Div => Some(Self::Div),
            Token::IntDiv => Some(Self::IntDiv),
            Token::Mod => Some(Self::Mod),
            Token::Plus => Some(Self::Plus),
            Token::Minus => Some(Self::Minus),
            Token::Gt => Some(Self::Gt),
            Token::Gte => Some(Self::Gte),
            Token::Lt => Some(Self::Lt),
            Token::Lte => Some(Self::Lte),
            Token::Is => Some(Self::Is),
            Token::As => Some(Self::As),
            Token::Equal => Some(Self::EqEq),
            Token::NotEqual => Some(Self::NotEq),
            Token::And => Some(Self::And),
            Token::Or => Some(Self::Or),
            Token::Pipe => Some(Self::Pipe),
            Token::Coalesce => Some(Self::NullCoalesce),
            Token::Dot => Some(Self::Dot),
            Token::QDot => Some(Self::QDot),
            Token::LBrack => Some(Self::Subscript),
            Token::NonNull => Some(Self::NonNull),
            _ => None,
        }
    }

    pub(crate) fn by_name(name: &str) -> Self {
        match name {
            "??" => Self::NullCoalesce,
            "|>" => Self::Pipe,
            "||" => Self::Or,
            "&&" => Self::And,
            "==" => Self::EqEq,
            "!=" => Self::NotEq,
            "is" => Self::Is,
            "as" => Self::As,
            "<" => Self::Lt,
            "<=" => Self::Lte,
            ">" => Self::Gt,
            ">=" => Self::Gte,
            "+" => Self::Plus,
            "-" => Self::Minus,
            "*" => Self::Mult,
            "/" => Self::Div,
            "~/" => Self::IntDiv,
            "%" => Self::Mod,
            "**" => Self::Pow,
            "!!" => Self::NonNull,
            "[" => Self::Subscript,
            "." => Self::Dot,
            "?." => Self::QDot,
            other => panic!("Unknown operator: {other}"),
        }
    }
}
