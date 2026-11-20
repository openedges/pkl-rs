//! Shared runtime for the Pkl 0.32.0 compatibility project.
//!
//! M1 prototypes establish shared source, value, heap, and Host contracts.
//! Their explicitly bounded language and probe coverage is not a complete evaluator.

pub mod data_size;
pub mod diagnostic;
mod diagnostic_names;
mod diagnostic_values;
pub mod duration;
mod economic_map;
pub mod evaluation;
pub mod glob;
pub mod heap;
pub mod host;
pub mod int_seq;
pub mod java_exception;
pub mod logging;
pub mod modules;
pub mod numeric;
pub mod packages;
pub mod parser;
pub mod pkl_binary_decode;
mod pkl_regex;
mod pkl_string;
mod power_assertions;
pub mod probe;
pub mod regex;
pub mod resolver;
pub mod resources;
pub mod source;
mod stdlib_sources;
mod string_case;
pub mod syntax;
pub mod value;
mod value_identity;
pub mod vm;

mod strict_map;

pub mod type_source;
