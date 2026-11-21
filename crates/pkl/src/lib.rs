//! Public ergonomic Rust API for Apple Pkl 0.32.0.
//!
//! Provides `Evaluator`, `EvaluatorBuilder`, `ModuleSource`, `PklValue`/`FromPklValue`,
//! `PModule`/`PObject`, `ModuleSchema`, `SecurityManager`, `Logger`, `StackFrameTransformer`,
//! `ModuleKeyFactory`/`ResourceReader`, `HttpClient`, `Project`, `Analyzer`, `Release`,
//! `pkl:test` runners & reporters, and `pkl:Command` support backed by the shared pure-Rust
//! `pkl-runtime` VM and `pkl-host-native` host adapter (zero Java fallback).

pub mod analyzer;
pub mod error;
pub mod evaluator;
pub mod frame;
pub mod http;
pub mod logger;
pub mod module_source;
pub mod project;
pub mod reader;
pub mod release;
pub mod security;
pub mod settings;
pub mod value;

pub use analyzer::{Analyzer, AnalyzerBuilder, Import, ImportGraph};
pub use error::{PklError, PklErrorKind, Result};
pub use evaluator::{CancelHandle, CommandEvalResult, Evaluator, EvaluatorBuilder, FileOutput};
pub use frame::{StackFrame, StackFrameTransformer, StackFrameTransformers};
pub use http::{HttpClient, HttpClientBuilder};
pub use logger::{BufferedLogger, Logger, Loggers};
pub use module_source::ModuleSource;
pub use project::Project;
pub use reader::{
    ModuleKey, ModuleKeyFactories, ModuleKeyFactory, PathElement, ResolvedModuleKey, Resource,
    ResourceReader, ResourceValue,
};
pub use release::{PKL_MAJOR, PKL_MINOR, PKL_PATCH, PKL_VERSION, Release};
pub use security::{SecurityManager, SecurityManagers, StandardSecurityManager};
pub use settings::{
    Color, DEFAULT_EDITOR_URL_SCHEME, Editor, ExternalReaderSpec, HttpSettings, OutputFormat,
    PklEvaluatorSettings, PklSettings, ProxySettings, TraceMode,
};
pub use value::{
    FromPklValue, PModule, PObject, PklBinaryDecoder, PklDataSize, PklDuration, PklValue,
};

/// Re-exports of `pkl-runtime` schema types (`Evaluator::evaluate_schema`).
pub mod schema {
    pub use pkl_runtime::schema::*;
}

/// Re-exports of `pkl:test` result and reporter APIs (`SpecReporter`, `MinimalReporter`, `JUnitReporter`).
pub mod test_report {
    pub use pkl_runtime::packages::test_runner::{
        TestError, TestFailure, TestReporterKind, TestResult, TestResults, TestSectionName,
        TestSectionResults, render_junit_aggregate_report, render_junit_report,
        render_minimal_report, render_report, render_spec_report, render_spec_report_colored,
        render_spec_summary, render_summary_colored,
    };
}

/// Re-exports of `pkl:Command` specification and parsing APIs.
pub mod command {
    pub use pkl_runtime::evaluation::command::{
        ArgumentSpec, BooleanFlagSpec, CommandCollectionKind, CommandCompletionCandidates,
        CommandMapKind, CommandOptionSpec, CommandParseOutcome, CommandSpec, CommandValueKind,
        CountedFlagSpec, FlagSpec, InvokedCommandLevel, ParsedOptionValue,
        parse_command_invocation, render_command_help,
    };
}
