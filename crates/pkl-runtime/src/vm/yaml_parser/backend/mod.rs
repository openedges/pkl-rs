//! Source-guided Pkl 0.32.0 / SnakeYAML Engine 3.0.1 YAML backend draft.
//! No Pkl values, callbacks, I/O, or Java execution belong in this crate.
//! See PROVENANCE.md and STATUS.md before integration.
#![forbid(unsafe_code)]
mod composer;
pub mod diagnostic;
pub mod nodes;
mod parser;
mod reader;
pub mod resolve;
mod scanner;
pub use composer::{Backend, StepOutcome};
pub use diagnostic::{Mark, YamlError};
pub use nodes::{Document, Node, NodeId, NodeKind, ScalarStyle, Text};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Compat,
    Yaml11,
    Yaml12,
}
#[derive(Clone, Debug)]
pub struct Settings {
    pub mode: Mode,
    pub max_collection_aliases: usize,
    pub label: Text,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            mode: Mode::Compat,
            max_collection_aliases: 50,
            label: Text::from("input_string"),
        }
    }
}
