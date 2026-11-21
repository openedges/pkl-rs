//! Pure-Rust Wasm-compatible Pkl 0.32.0 code formatter.

mod builder;
#[cfg(not(target_arch = "wasm32"))]
pub mod cli;
mod cst;
mod error;
mod format_node;
mod generator;
mod lexer;
mod natural_order;
mod parser;
mod token;
mod unicode_tables;

use builder::Builder;
use format_node::FormatNode;
use generator::Generator;
use parser::GenericParser;
use std::fmt;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum GrammarVersion {
    V1,
    #[default]
    V2,
}

impl GrammarVersion {
    pub const fn latest() -> Self {
        Self::V2
    }

    pub const fn version(self) -> u8 {
        match self {
            Self::V1 => 1,
            Self::V2 => 2,
        }
    }

    pub const fn version_span(self) -> &'static str {
        match self {
            Self::V1 => "0.25 - 0.29",
            Self::V2 => "0.30+",
        }
    }

    pub fn parse_cli_str(s: &str) -> Option<Self> {
        match s {
            "1" => Some(Self::V1),
            "2" => Some(Self::V2),
            _ => None,
        }
    }
}

impl fmt::Display for GrammarVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.version())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
    pub char_index: usize,
    pub length: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FullSpan {
    pub char_index: usize,
    pub length: usize,
    pub line_begin: usize,
    pub col_begin: usize,
    pub line_end: usize,
    pub col_end: usize,
}

impl fmt::Display for FullSpan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "({}:{} - {}:{})",
            self.line_begin, self.col_begin, self.line_end, self.col_end
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FormatError {
    Lexer { message: String, span: Span },
    Parser { message: String, span: FullSpan },
}

impl fmt::Display for FormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Lexer { message, .. } => write!(f, "org.pkl.parser.ParserError: {message}"),
            Self::Parser { message, span } => write!(f, "{message} at {span}"),
        }
    }
}

impl std::error::Error for FormatError {}

#[derive(Clone, Copy, Debug, Default)]
pub struct Formatter {
    grammar_version: GrammarVersion,
}

impl Formatter {
    pub const fn new(grammar_version: GrammarVersion) -> Self {
        Self { grammar_version }
    }

    pub const fn grammar_version(&self) -> GrammarVersion {
        self.grammar_version
    }

    pub fn format(&self, input: &str) -> Result<String, FormatError> {
        let mut out = String::new();
        self.format_to(input, &mut out)?;
        Ok(out)
    }

    pub fn format_to(&self, input: &str, out: &mut impl fmt::Write) -> Result<(), FormatError> {
        let (ast, source) = GenericParser::parse_module_with_source(input)?;
        let format_ast = Builder::new(&source, self.grammar_version).format(&ast);
        // force a line at the end of the file
        let nodes = FormatNode::Nodes(vec![format_ast, FormatNode::ForceLine]);
        let mut generator = Generator::new(out);
        let _ = generator.generate(&nodes);
        Ok(())
    }
}

pub fn format_str(input: &str, grammar_version: GrammarVersion) -> Result<String, FormatError> {
    Formatter::new(grammar_version).format(input)
}

pub fn format_str_latest(input: &str) -> Result<String, FormatError> {
    format_str(input, GrammarVersion::latest())
}
