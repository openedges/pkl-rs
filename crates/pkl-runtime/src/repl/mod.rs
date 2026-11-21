//! Pure-Rust Wasm-compatible Pkl 0.32.0 REPL session, commands, messages, and highlighting.

pub mod commands;
pub mod highlight;
pub mod messages;
pub mod session;

pub use commands::{
    ParsedReplCommand, ReplCommandKind, format_ambiguous_commands, format_unknown_command,
    match_commands,
};
pub use highlight::{
    format_prompt, highlight_backticks, strip_expression_preamble, syntax_highlight,
};
pub use messages::{HELP, WELCOME, examples};
pub use session::{ReplPoll, ReplRequest, ReplResponse, ReplSession, ReplSessionOptions};
