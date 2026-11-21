//! Wasm-pure cooperative REPL session mirroring Apple Pkl 0.32.0 `org.pkl.core.repl.ReplServer`.
//!
//! All evaluation state lives on a single persistent `HostSession` / `Vm` heap.
//! Platform hosts (native `NativeHost`, Wasm, or unit test harnesses) drive `ReplSession`
//! cooperatively via `start_request` + `poll` + `complete_*`.

use super::highlight::strip_expression_preamble;
use crate::diagnostic::{Diagnostic, RenderFrame, render_frontend_error_text_colored};
use crate::evaluation::frontend::DiagnosticProfile;
use crate::evaluation::runtime_error_text_with_profile_and_color;
use crate::glob::{GlobCompletion, GlobRequest};
use crate::host::{HostPoll, HostSession, ReadFailure};
use crate::logging::{
    EvaluationContext, FrameTransformer, LogDelivery, LogEvent, LogFrame, LoggerFailure, TraceMode,
};
use crate::modules::{ModuleCompletion, ModuleRequest};
use crate::parser::{ReplItem, parse_repl_input};
use crate::resources::ReadResult;
use crate::source::{SourceMap, SourceText};
use crate::value::{ErrorKind, ObjectId, PklString, RuntimeError, RuntimeSource, Value};
use crate::vm::repl::CompiledReplMember;
use crate::vm::{Instruction, Program, ReadRequest};
use std::collections::{BTreeSet, VecDeque};
use std::sync::Arc;

/// Configuration for creating a [`ReplSession`].
#[derive(Clone, Debug)]
pub struct ReplSessionOptions {
    /// Base directory URI for resolving relative `:load`, `import`, and `read` paths.
    /// Should end with `/` (normalized automatically if omitted).
    pub working_dir_uri: PklString,
    /// Enable ANSI syntax highlighting on result values and ANSI colors on diagnostics.
    pub color: bool,
    /// `trace()` output mode (`Compact` or `Pretty`).
    pub trace_mode: TraceMode,
    /// Log delivery mode (`Host` delivers `ReplPoll::Log` events to the caller).
    pub log_delivery: LogDelivery,
    /// Stack frame transformation policy (`Default` or `Host`).
    pub frame_transformer: FrameTransformer,
    /// Enable power assertions inside REPL evaluations.
    pub power_assertions: bool,
    /// Enable `pkl:platform` host data.
    pub platform_data: bool,
}

impl Default for ReplSessionOptions {
    fn default() -> Self {
        Self {
            working_dir_uri: "file:///".into(),
            color: false,
            trace_mode: TraceMode::Compact,
            log_delivery: LogDelivery::Host,
            frame_transformer: FrameTransformer::Default,
            power_assertions: false,
            platform_data: false,
        }
    }
}

/// Request sent to [`ReplSession`], mirroring `org.pkl.core.repl.ReplRequest`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReplRequest {
    /// Evaluate zero or more REPL items (expressions, properties, methods, classes,
    /// typealiases, imports, module declarations).
    Eval {
        id: String,
        text: String,
        eval_definitions: bool,
        force_results: bool,
    },
    /// Load a module from `uri` (resolved against `working_dir_uri`) and replace the REPL
    /// module with its declared properties, methods, members, and parent.
    Load { id: String, uri: String },
    /// Evaluate `text` against a non-mutating snapshot of the REPL module and return
    /// available member/method completion candidates.
    Completion { id: String, text: String },
    /// Reset the REPL environment to a fresh empty module inheriting from `pkl:base#Module`.
    Reset { id: String },
}

/// Response emitted by [`ReplSession`], mirroring `org.pkl.core.repl.ReplResponse`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReplResponse {
    EvalSuccess { result: String },
    EvalError { message: String },
    Completion { members: BTreeSet<String> },
    IncompleteInput,
    InvalidRequest { message: String },
    InternalError { message: String },
}

/// Cooperative poll step returned by [`ReplSession::poll`].
#[derive(Clone, Debug)]
pub enum ReplPoll {
    Ready(Vec<ReplResponse>),
    Read {
        token: String,
        uri: PklString,
    },
    Log {
        token: String,
        event: Arc<LogEvent>,
    },
    TransformFrame {
        token: String,
        frame: Arc<LogFrame>,
    },
    Module {
        token: String,
        request: ModuleRequest,
    },
    Glob {
        token: String,
        request: GlobRequest,
    },
    Yielded,
}

enum ItemPhase {
    NextItem,
    RunningExpr {
        source: Arc<RuntimeSource>,
    },
    RunningMemberInit {
        candidate_module: ObjectId,
        compiled: Box<CompiledReplMember>,
        is_static_decl: bool,
    },
    ForcingValue {
        source: Arc<RuntimeSource>,
    },
}

struct EvalSeqState {
    is_completion: bool,
    target_module: ObjectId,
    request_uri: PklString,
    src_text: String,
    items: VecDeque<ReplItem>,
    eval_definitions: bool,
    force_results: bool,
    responses: Vec<ReplResponse>,
    last_value: Option<Value>,
    last_failed: bool,
    phase: ItemPhase,
}

enum ActiveOp {
    Bootstrap { pending: ReplRequest },
    ReadyImmediate(Vec<ReplResponse>),
    Load { resolved_uri: PklString },
    EvalSeq(Box<EvalSeqState>),
}

/// Wasm-pure stateful REPL evaluator owning one persistent live `HostSession`.
pub struct ReplSession {
    host: HostSession,
    working_dir_uri: PklString,
    color: bool,
    base_loaded: bool,
    return_entry: Option<usize>,
    repl_module: Option<ObjectId>,
    active: Option<ActiveOp>,
    request_timeout_ms: Option<u64>,
    request_start_ms: u64,
}

impl ReplSession {
    /// Creates a new REPL session without performing any I/O.
    pub fn new(options: ReplSessionOptions) -> Self {
        let mut working_dir_uri = options.working_dir_uri.to_java_utf8();
        if !working_dir_uri.ends_with('/') {
            working_dir_uri.push('/');
        }
        let program = Arc::new(Program::new(vec![vec![Instruction::Return]]));
        let mut host = HostSession::with_context(
            program,
            EvaluationContext {
                primary_source: None,
                trace_mode: options.trace_mode,
                log_delivery: options.log_delivery,
                frame_transformer: options.frame_transformer,
                platform_metadata: options
                    .platform_data
                    .then(crate::logging::PlatformMetadata::default),
            },
        );
        host.set_power_assertions(options.power_assertions);
        Self {
            host,
            working_dir_uri: PklString::from(working_dir_uri),
            color: options.color,
            base_loaded: false,
            return_entry: None,
            repl_module: None,
            active: None,
            request_timeout_ms: None,
            request_start_ms: 0,
        }
    }

    pub fn use_color(&self) -> bool {
        self.color
    }

    pub fn working_dir_uri(&self) -> &PklString {
        &self.working_dir_uri
    }

    /// Starts handling a single [`ReplRequest`]. Drive to completion with [`Self::poll`].
    pub fn start_request(
        &mut self,
        request: ReplRequest,
        now_ms: u64,
        timeout_ms: Option<u64>,
    ) -> Result<(), RuntimeError> {
        if self.active.is_some() {
            return Err(RuntimeError::new(
                ErrorKind::Busy,
                "REPL session already has an active request",
            ));
        }
        self.request_start_ms = now_ms;
        self.request_timeout_ms = timeout_ms;
        if !self.base_loaded {
            let entry = self.host.vm_mut().repl_bootstrap_entry()?;
            self.host.start(entry, Vec::new(), now_ms, timeout_ms)?;
            self.active = Some(ActiveOp::Bootstrap { pending: request });
            return Ok(());
        }
        self.begin_dispatched_request(request, now_ms)
    }

    fn remaining_timeout(&self, now_ms: u64) -> Option<u64> {
        self.request_timeout_ms
            .map(|limit| limit.saturating_sub(now_ms.saturating_sub(self.request_start_ms)))
    }

    fn begin_dispatched_request(
        &mut self,
        request: ReplRequest,
        now_ms: u64,
    ) -> Result<(), RuntimeError> {
        let repl_module = self
            .repl_module
            .ok_or_else(|| RuntimeError::new(ErrorKind::Host, "REPL module not initialized"))?;
        match request {
            ReplRequest::Reset { .. } => {
                let fresh = self
                    .host
                    .vm_mut()
                    .repl_create_empty_module(None, &self.working_dir_uri)?;
                self.repl_module = Some(fresh);
                self.active = Some(ActiveOp::ReadyImmediate(Vec::new()));
                Ok(())
            }
            ReplRequest::Load { uri, .. } => {
                let authored = PklString::from(uri);
                let resolved_uri =
                    match crate::host::resolve_module_key_uri(&authored, &self.working_dir_uri) {
                        Ok(u) => u,
                        Err(err) => {
                            let msg = self.format_repl_error(err, None);
                            self.active =
                                Some(ActiveOp::ReadyImmediate(vec![ReplResponse::EvalError {
                                    message: msg,
                                }]));
                            return Ok(());
                        }
                    };
                let entry = match self
                    .host
                    .vm_mut()
                    .repl_load_module_entry(resolved_uri.clone())
                {
                    Ok(e) => e,
                    Err(err) => {
                        let msg = self.format_repl_error(err, None);
                        self.active =
                            Some(ActiveOp::ReadyImmediate(vec![ReplResponse::EvalError {
                                message: msg,
                            }]));
                        return Ok(());
                    }
                };
                self.host
                    .start(entry, Vec::new(), now_ms, self.remaining_timeout(now_ms))?;
                self.active = Some(ActiveOp::Load { resolved_uri });
                Ok(())
            }
            ReplRequest::Completion { id, text } => {
                if text.trim().is_empty() {
                    let members = self
                        .host
                        .vm_mut()
                        .repl_collect_completions(None, repl_module)?;
                    self.active = Some(ActiveOp::ReadyImmediate(vec![ReplResponse::Completion {
                        members,
                    }]));
                    return Ok(());
                }
                let parsed = match parse_repl_input(text.as_str()) {
                    Ok(p) => p,
                    Err(_) => {
                        self.active =
                            Some(ActiveOp::ReadyImmediate(vec![ReplResponse::Completion {
                                members: BTreeSet::new(),
                            }]));
                        return Ok(());
                    }
                };
                if parsed.items.is_empty() {
                    self.active = Some(ActiveOp::ReadyImmediate(vec![ReplResponse::Completion {
                        members: BTreeSet::new(),
                    }]));
                    return Ok(());
                }
                let snapshot = self
                    .host
                    .vm_mut()
                    .repl_snapshot_module(repl_module, &self.working_dir_uri)?;
                self.active = Some(ActiveOp::EvalSeq(Box::new(EvalSeqState {
                    is_completion: true,
                    target_module: snapshot,
                    request_uri: PklString::from(format!("repl:{id}")),
                    src_text: text,
                    items: parsed.items.into(),
                    eval_definitions: false,
                    force_results: false,
                    responses: Vec::new(),
                    last_value: None,
                    last_failed: false,
                    phase: ItemPhase::NextItem,
                })));
                Ok(())
            }
            ReplRequest::Eval {
                id,
                text,
                eval_definitions,
                force_results,
            } => {
                let request_uri = PklString::from(format!("repl:{id}"));
                let parsed = match parse_repl_input(text.as_str()) {
                    Ok(p) => p,
                    Err(diag) => {
                        let msg = self.format_parse_error(&diag, &request_uri, &text);
                        self.active =
                            Some(ActiveOp::ReadyImmediate(vec![ReplResponse::EvalError {
                                message: msg,
                            }]));
                        return Ok(());
                    }
                };
                self.active = Some(ActiveOp::EvalSeq(Box::new(EvalSeqState {
                    is_completion: false,
                    target_module: repl_module,
                    request_uri,
                    src_text: text,
                    items: parsed.items.into(),
                    eval_definitions,
                    force_results,
                    responses: Vec::new(),
                    last_value: None,
                    last_failed: false,
                    phase: ItemPhase::NextItem,
                })));
                Ok(())
            }
        }
    }

    /// Advances the active REPL request cooperatively, returning either `ReplPoll::Ready`,
    /// `ReplPoll::Yielded`, or a Host I/O / logging request to complete.
    pub fn poll(&mut self, now_ms: u64, budget: usize) -> ReplPoll {
        loop {
            let Some(op) = self.active.take() else {
                return ReplPoll::Ready(Vec::new());
            };
            match op {
                ActiveOp::ReadyImmediate(responses) => return ReplPoll::Ready(responses),
                ActiveOp::Bootstrap { pending } => match self.host.poll(now_ms, budget) {
                    HostPoll::Ready(Ok(_)) => {
                        self.base_loaded = true;
                        let init = (|| -> Result<(), RuntimeError> {
                            self.return_entry = Some(self.host.vm_mut().repl_return_entry()?);
                            let empty = self
                                .host
                                .vm_mut()
                                .repl_create_empty_module(None, &self.working_dir_uri)?;
                            self.repl_module = Some(empty);
                            self.begin_dispatched_request(pending, now_ms)
                        })();
                        if let Err(err) = init {
                            return ReplPoll::Ready(vec![ReplResponse::InternalError {
                                message: err.message.to_java_utf8(),
                            }]);
                        }
                    }
                    HostPoll::Ready(Err(err)) => {
                        return ReplPoll::Ready(vec![ReplResponse::InternalError {
                            message: err.message.to_java_utf8(),
                        }]);
                    }
                    other => {
                        self.active = Some(ActiveOp::Bootstrap { pending });
                        return map_host_poll(other);
                    }
                },
                ActiveOp::Load { resolved_uri } => match self.host.poll(now_ms, budget) {
                    HostPoll::Ready(Ok(Value::Object(loaded))) => {
                        match self
                            .host
                            .vm_mut()
                            .repl_adopt_loaded_module(loaded, &self.working_dir_uri)
                        {
                            Ok(adopted) => {
                                self.repl_module = Some(adopted);
                                return ReplPoll::Ready(Vec::new());
                            }
                            Err(err) => {
                                return ReplPoll::Ready(vec![ReplResponse::InternalError {
                                    message: err.message.to_java_utf8(),
                                }]);
                            }
                        }
                    }
                    HostPoll::Ready(Ok(_)) => {
                        return ReplPoll::Ready(vec![ReplResponse::InternalError {
                            message: format!(
                                "Loaded module `{}` did not produce a module object.",
                                resolved_uri.to_java_utf8()
                            ),
                        }]);
                    }
                    HostPoll::Ready(Err(err)) => {
                        let message = self.format_repl_error(err, None);
                        return ReplPoll::Ready(vec![ReplResponse::EvalError { message }]);
                    }
                    other => {
                        self.active = Some(ActiveOp::Load { resolved_uri });
                        return map_host_poll(other);
                    }
                },
                ActiveOp::EvalSeq(mut seq) => match self.step_eval_seq(&mut seq, now_ms, budget) {
                    SeqStep::Continue => {
                        self.active = Some(ActiveOp::EvalSeq(seq));
                    }
                    SeqStep::Suspend(poll) => {
                        self.active = Some(ActiveOp::EvalSeq(seq));
                        return *poll;
                    }
                    SeqStep::Finished(responses) => return ReplPoll::Ready(responses),
                },
            }
        }
    }

    fn step_eval_seq(&mut self, seq: &mut EvalSeqState, now_ms: u64, budget: usize) -> SeqStep {
        let phase = std::mem::replace(&mut seq.phase, ItemPhase::NextItem);
        match phase {
            ItemPhase::NextItem => {
                let Some(item) = seq.items.pop_front() else {
                    if seq.is_completion {
                        if seq.last_failed || seq.last_value.is_none() {
                            return SeqStep::Finished(vec![ReplResponse::Completion {
                                members: BTreeSet::new(),
                            }]);
                        }
                        let res = self
                            .host
                            .vm_mut()
                            .repl_collect_completions(seq.last_value.as_ref(), seq.target_module);
                        return match res {
                            Ok(members) => {
                                SeqStep::Finished(vec![ReplResponse::Completion { members }])
                            }
                            Err(_) => SeqStep::Finished(vec![ReplResponse::Completion {
                                members: BTreeSet::new(),
                            }]),
                        };
                    }
                    return SeqStep::Finished(std::mem::take(&mut seq.responses));
                };

                let span = item.span();
                let node_text = seq
                    .src_text
                    .get(span.start..span.end)
                    .unwrap_or(&seq.src_text);
                let rem = self.remaining_timeout(now_ms);

                match item {
                    ReplItem::ModuleDecl { .. } => SeqStep::Continue,
                    ReplItem::Expr { .. } => {
                        let compiled = match self.host.vm_mut().repl_compile_expr(
                            seq.target_module,
                            node_text,
                            &seq.request_uri,
                            &self.working_dir_uri,
                        ) {
                            Ok(c) => c,
                            Err(err) => {
                                self.record_seq_error(seq, err, None, true);
                                return SeqStep::Continue;
                            }
                        };
                        if let Err(err) = self.host.start(
                            compiled.entry,
                            vec![Value::Object(seq.target_module)],
                            now_ms,
                            rem,
                        ) {
                            self.record_seq_error(seq, err, Some(&compiled.source), true);
                            return SeqStep::Continue;
                        }
                        seq.phase = ItemPhase::RunningExpr {
                            source: compiled.source,
                        };
                        SeqStep::Continue
                    }
                    ReplItem::Import { member, .. }
                    | ReplItem::Class { member, .. }
                    | ReplItem::TypeAlias { member, .. }
                    | ReplItem::Method { member, .. }
                    | ReplItem::Property { member, .. } => {
                        let is_static_decl = matches!(
                            member.expr.kind,
                            crate::syntax::ExprKind::Import { .. }
                                | crate::syntax::ExprKind::Class { .. }
                                | crate::syntax::ExprKind::TypeAlias { .. }
                        );
                        let is_method = matches!(
                            member.expr.kind,
                            crate::syntax::ExprKind::Function { method: true, .. }
                        );
                        let is_property = !is_static_decl && !is_method;

                        let needs_child = !is_static_decl
                            && !member.local
                            && self
                                .host
                                .vm()
                                .repl_module_has_member(seq.target_module, &member.name, is_method)
                                .unwrap_or(false);
                        let candidate_module = if needs_child {
                            match self.host.vm_mut().repl_create_empty_module(
                                Some(seq.target_module),
                                &self.working_dir_uri,
                            ) {
                                Ok(m) => m,
                                Err(err) => {
                                    self.record_seq_error(seq, err, None, false);
                                    return SeqStep::Continue;
                                }
                            }
                        } else {
                            seq.target_module
                        };

                        let compiled = match self.host.vm_mut().repl_compile_member(
                            candidate_module,
                            node_text,
                            &member.name,
                            is_method,
                            is_property,
                            seq.eval_definitions,
                            &seq.request_uri,
                            &self.working_dir_uri,
                        ) {
                            Ok(c) => c,
                            Err(err) => {
                                self.record_seq_error(seq, err, None, false);
                                return SeqStep::Continue;
                            }
                        };
                        if let Err(err) = self.host.start(
                            compiled.init_entry,
                            vec![Value::Object(candidate_module)],
                            now_ms,
                            rem,
                        ) {
                            self.record_seq_error(seq, err, Some(&compiled.source), false);
                            return SeqStep::Continue;
                        }
                        seq.phase = ItemPhase::RunningMemberInit {
                            candidate_module,
                            compiled: Box::new(compiled),
                            is_static_decl,
                        };
                        SeqStep::Continue
                    }
                }
            }
            ItemPhase::RunningExpr { source } => match self.host.poll(now_ms, budget) {
                HostPoll::Ready(Ok(val)) => {
                    if seq.force_results && !seq.is_completion {
                        let Some(ret_entry) = self.return_entry else {
                            self.record_seq_error(
                                seq,
                                RuntimeError::new(ErrorKind::Host, "missing REPL force entry"),
                                Some(&source),
                                true,
                            );
                            return SeqStep::Continue;
                        };
                        let rem = self.remaining_timeout(now_ms);
                        if let Err(err) = self.host.start(ret_entry, Vec::new(), now_ms, rem) {
                            self.record_seq_error(seq, err, Some(&source), true);
                            return SeqStep::Continue;
                        }
                        if let Err(err) = self.host.vm_mut().repl_begin_force(val) {
                            self.record_seq_error(seq, err, Some(&source), true);
                            return SeqStep::Continue;
                        }
                        seq.phase = ItemPhase::ForcingValue { source };
                        return SeqStep::Continue;
                    }
                    self.record_seq_value(seq, &val, &source);
                    SeqStep::Continue
                }
                HostPoll::Ready(Err(err)) => {
                    self.record_seq_error(seq, err, Some(&source), true);
                    SeqStep::Continue
                }
                other => {
                    seq.phase = ItemPhase::RunningExpr { source };
                    SeqStep::Suspend(Box::new(map_host_poll(other)))
                }
            },
            ItemPhase::RunningMemberInit {
                candidate_module,
                compiled,
                is_static_decl,
            } => match self.host.poll(now_ms, budget) {
                HostPoll::Ready(Ok(_)) => {
                    let CompiledReplMember {
                        eval_entry,
                        member,
                        source,
                        ..
                    } = *compiled;
                    if let Err(err) = self.host.vm_mut().repl_publish_member(
                        candidate_module,
                        member,
                        is_static_decl,
                    ) {
                        self.record_seq_error(seq, err, Some(&source), false);
                        return SeqStep::Continue;
                    }
                    seq.target_module = candidate_module;
                    if !seq.is_completion {
                        self.repl_module = Some(candidate_module);
                    }
                    if let Some(eval_entry) = eval_entry {
                        let rem = self.remaining_timeout(now_ms);
                        if let Err(err) = self.host.start(
                            eval_entry,
                            vec![Value::Object(candidate_module)],
                            now_ms,
                            rem,
                        ) {
                            self.record_seq_error(seq, err, Some(&source), false);
                            return SeqStep::Continue;
                        }
                        seq.phase = ItemPhase::RunningExpr { source };
                    }
                    SeqStep::Continue
                }
                HostPoll::Ready(Err(err)) => {
                    self.record_seq_error(seq, err, Some(&compiled.source), false);
                    SeqStep::Continue
                }
                other => {
                    seq.phase = ItemPhase::RunningMemberInit {
                        candidate_module,
                        compiled,
                        is_static_decl,
                    };
                    SeqStep::Suspend(Box::new(map_host_poll(other)))
                }
            },
            ItemPhase::ForcingValue { source } => match self.host.poll(now_ms, budget) {
                HostPoll::Ready(Ok(val)) => {
                    self.record_seq_value(seq, &val, &source);
                    SeqStep::Continue
                }
                HostPoll::Ready(Err(err)) => {
                    self.record_seq_error(seq, err, Some(&source), true);
                    SeqStep::Continue
                }
                other => {
                    seq.phase = ItemPhase::ForcingValue { source };
                    SeqStep::Suspend(Box::new(map_host_poll(other)))
                }
            },
        }
    }

    fn record_seq_value(&self, seq: &mut EvalSeqState, val: &Value, source: &Arc<RuntimeSource>) {
        if seq.is_completion {
            seq.last_value = Some(val.clone());
            seq.last_failed = false;
            return;
        }
        match self
            .host
            .vm()
            .repl_render_value(val, seq.target_module, self.color)
        {
            Ok(rendered) => seq
                .responses
                .push(ReplResponse::EvalSuccess { result: rendered }),
            Err(err) => {
                let msg = strip_expression_preamble(&self.format_repl_error(err, Some(source)));
                seq.responses.push(ReplResponse::EvalError { message: msg });
            }
        }
    }

    fn record_seq_error(
        &self,
        seq: &mut EvalSeqState,
        err: RuntimeError,
        source: Option<&Arc<RuntimeSource>>,
        strip_preamble: bool,
    ) {
        if seq.is_completion {
            seq.last_value = None;
            seq.last_failed = true;
            return;
        }
        let mut msg = self.format_repl_error(err, source);
        if strip_preamble {
            msg = strip_expression_preamble(&msg);
        }
        seq.responses.push(ReplResponse::EvalError { message: msg });
    }

    fn format_parse_error(&self, diag: &Diagnostic, uri: &PklString, text: &str) -> String {
        let mut sources = SourceMap::default();
        let id = sources.add(uri.clone(), SourceText::from(text));
        let source = sources.get(id).expect("parse error source");
        let frame = RenderFrame {
            source,
            span: diag.span,
            member: uri,
            uri: uri.clone(),
            highlight_length_utf16: diag.highlight_length_utf16,
        };
        render_frontend_error_text_colored(diag, &[frame], self.color).to_java_utf8()
    }

    fn format_repl_error(&self, err: RuntimeError, source: Option<&Arc<RuntimeSource>>) -> String {
        if err.kind == ErrorKind::Timeout {
            return err.message.to_java_utf8();
        }
        let mut sources = SourceMap::default();
        let (uri, module_name, text) = match source {
            Some(src) => (src.uri.clone(), src.module_name.clone(), src.text.clone()),
            None => (
                PklString::from("repl:repl"),
                PklString::from("repl"),
                SourceText::from(""),
            ),
        };
        let id = sources.add(uri.clone(), text);
        let src = sources.get(id).expect("repl error source");
        runtime_error_text_with_profile_and_color(
            src,
            uri,
            module_name,
            SourceText::from(""),
            err,
            None,
            DiagnosticProfile::ApiIdentity,
            self.color,
        )
        .to_java_utf8()
    }

    // ------------------------------------------------------------------------
    // Cooperative Host Completion Pass-Throughs
    // ------------------------------------------------------------------------

    pub fn read_request(&self, token: &str) -> Result<ReadRequest, RuntimeError> {
        self.host.read_request(token)
    }

    pub fn complete_read(
        &mut self,
        token: &str,
        result: Result<ReadResult, ReadFailure>,
        now_ms: u64,
    ) -> Result<(), RuntimeError> {
        self.host.complete_read(token, result, now_ms)
    }

    pub fn complete_module(
        &mut self,
        token: &str,
        completion: ModuleCompletion,
        now_ms: u64,
    ) -> Result<(), RuntimeError> {
        self.host.complete_module(token, completion, now_ms)
    }

    pub fn complete_glob(
        &mut self,
        token: &str,
        completion: GlobCompletion,
        now_ms: u64,
    ) -> Result<(), RuntimeError> {
        self.host.complete_glob(token, completion, now_ms)
    }

    pub fn acknowledge_log(
        &mut self,
        token: &str,
        result: Result<(), LoggerFailure>,
        now_ms: u64,
    ) -> Result<(), RuntimeError> {
        self.host.acknowledge_log(token, result, now_ms)
    }

    pub fn complete_frame_transform(
        &mut self,
        token: &str,
        result: Result<LogFrame, LoggerFailure>,
        now_ms: u64,
    ) -> Result<(), RuntimeError> {
        self.host.complete_frame_transform(token, result, now_ms)
    }

    pub fn cancel(&mut self) {
        self.host.cancel();
    }

    pub fn close(&mut self) {
        self.active = None;
        self.host.close();
    }
}

enum SeqStep {
    Continue,
    Suspend(Box<ReplPoll>),
    Finished(Vec<ReplResponse>),
}

fn map_host_poll(poll: HostPoll) -> ReplPoll {
    match poll {
        HostPoll::Read { token, uri } => ReplPoll::Read { token, uri },
        HostPoll::Log { token, event } => ReplPoll::Log { token, event },
        HostPoll::TransformFrame { token, frame } => ReplPoll::TransformFrame { token, frame },
        HostPoll::Module { token, request } => ReplPoll::Module { token, request },
        HostPoll::Glob { token, request } => ReplPoll::Glob { token, request },
        HostPoll::Yielded => ReplPoll::Yielded,
        HostPoll::Ready(_) => unreachable!("Ready handled by caller"),
    }
}
