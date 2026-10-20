//! Shared runtime for the Pkl 0.32.0 compatibility project.
//!
//! M1 prototypes establish shared source, value, heap, and Host contracts.
//! Their explicitly bounded language and probe coverage is not a complete evaluator.

pub mod diagnostic;
pub mod heap;
pub mod host;
pub mod numeric;
pub mod parser;
pub mod probe;
pub mod regex;
pub mod resolver;
pub mod source;
pub mod syntax;
pub mod value;
pub mod vm;
