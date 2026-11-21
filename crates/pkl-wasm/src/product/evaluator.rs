//! Full cooperative async `WasmEvaluator` session backed by `pkl_runtime::evaluation::session::SourceSession`.

use super::{
    decode::decoded_value_to_js,
    host_core::{
        WasmHostCore, WasmHostOutcome, WasmSuspendRequest, parse_host_config_js,
        parse_module_host_error, parse_path_elements_js, parse_read_failure,
    },
    schema::module_schema_to_js,
    util::{
        StructuredError, bytes_from_js, js_from_bytes, js_string_from_pkl, log_frame_from_js,
        parse_opt_u64_js, parse_u64_js, pkl_string_from_js, rust_string_from_js, set_prop,
        source_text_from_js,
    },
};
use js_sys::{Array, Object};
use pkl_runtime::{
    diagnostic::TypeDiagnostic,
    evaluation::{
        EvaluationOptions,
        session::{SourceSession, SourceSessionOptions},
    },
    glob::GlobHostError,
    host::{HostPoll, ReadFailure, format_timeout_message},
    logging::{FrameTransformer, LogDelivery, LoggerFailure},
    modules::{ModuleCompletion, ModuleHostError, ModuleSourceSpec},
    packages::test_runner::format_fact_failure_with_power_assertions,
    schema::{SchemaExtractor, SchemaStep},
    source::SourceMap,
    value::{ErrorKind, PklString, RuntimeError, Value},
};
use wasm_bindgen::prelude::*;

enum ActiveOperation {
    OutputText,
    OutputBytes,
    Expression,
    ExpressionPklBinary,
    Schema {
        extractor: Option<SchemaExtractor>,
    },
    PowerAssertionProbe {
        probe_expr: String,
        location: String,
        use_color: bool,
    },
}

#[wasm_bindgen(js_name = WasmEvaluator)]
pub struct WasmEvaluator {
    core: WasmHostCore,
    session: SourceSession,
    active: Option<ActiveOperation>,
    active_timeout_ms: Option<u64>,
    pending_suspend: Option<WasmSuspendRequest>,
    pending_capability_error: Option<(&'static str, String)>,
    force_suspend_logs: bool,
    closed: bool,
}

#[wasm_bindgen]
impl WasmEvaluator {
    #[wasm_bindgen(constructor)]
    pub fn new(options: JsValue) -> Result<WasmEvaluator, JsValue> {
        let config = parse_host_config_js(&options)?;
        let session_opts = SourceSessionOptions {
            evaluation: EvaluationOptions {
                power_assertions: config.power_assertions,
            },
            trace_mode: config.trace_mode,
            log_delivery: LogDelivery::Host,
            frame_transformer: if config.has_custom_frame_transformer {
                FrameTransformer::Host
            } else {
                FrameTransformer::Default
            },
            platform_data: config.platform_metadata.is_some(),
        };
        let use_color = config.use_color;
        let platform_metadata = config.platform_metadata.clone();
        let core = WasmHostCore::new(config)?;
        let mut session = SourceSession::module_evaluator(session_opts).with_color(use_color);
        if let Some(meta) = platform_metadata {
            session.set_platform_metadata(Some(meta));
        }
        Ok(Self {
            core,
            session,
            active: None,
            active_timeout_ms: None,
            pending_suspend: None,
            pending_capability_error: None,
            force_suspend_logs: false,
            closed: false,
        })
    }

    #[wasm_bindgen(js_name = setCaptureLogs)]
    pub fn set_capture_logs(&mut self, capture: bool) {
        self.force_suspend_logs = capture;
    }

    #[wasm_bindgen(js_name = isClosed)]
    pub fn is_closed(&self) -> bool {
        self.closed || self.session.is_closed()
    }

    #[wasm_bindgen(js_name = startOutputText)]
    pub fn start_output_text(
        &mut self,
        uri: JsValue,
        contents: JsValue,
        now: JsValue,
        timeout_ms: JsValue,
    ) -> Result<(), JsValue> {
        self.begin_call(now, timeout_ms, |this, now_ms, timeout| {
            let spec = build_source_spec(&uri, &contents)?;
            this.session
                .start_evaluation(spec, "output.text", now_ms, timeout)
                .map_err(|err| this.map_runtime_error(err))?;
            this.active = Some(ActiveOperation::OutputText);
            Ok(())
        })
    }

    #[wasm_bindgen(js_name = startOutputBytes)]
    pub fn start_output_bytes(
        &mut self,
        uri: JsValue,
        contents: JsValue,
        now: JsValue,
        timeout_ms: JsValue,
    ) -> Result<(), JsValue> {
        self.begin_call(now, timeout_ms, |this, now_ms, timeout| {
            let spec = build_source_spec(&uri, &contents)?;
            this.session
                .start_evaluation_output_bytes(spec, now_ms, timeout)
                .map_err(|err| this.map_runtime_error(err))?;
            this.active = Some(ActiveOperation::OutputBytes);
            Ok(())
        })
    }

    #[wasm_bindgen(js_name = startExpression)]
    pub fn start_expression(
        &mut self,
        uri: JsValue,
        contents: JsValue,
        expression: JsValue,
        now: JsValue,
        timeout_ms: JsValue,
    ) -> Result<(), JsValue> {
        self.begin_call(now, timeout_ms, |this, now_ms, timeout| {
            let spec = build_source_spec(&uri, &contents)?;
            let expr = source_text_from_js(&expression)?;
            this.session
                .start_evaluation(spec, expr, now_ms, timeout)
                .map_err(|err| this.map_runtime_error(err))?;
            this.active = Some(ActiveOperation::Expression);
            Ok(())
        })
    }

    #[wasm_bindgen(js_name = startExpressionPklBinary)]
    pub fn start_expression_pkl_binary(
        &mut self,
        uri: JsValue,
        contents: JsValue,
        expression: JsValue,
        now: JsValue,
        timeout_ms: JsValue,
    ) -> Result<(), JsValue> {
        self.begin_call(now, timeout_ms, |this, now_ms, timeout| {
            let spec = build_source_spec(&uri, &contents)?;
            let expr = source_text_from_js(&expression)?;
            this.session
                .start_evaluation_pkl_binary(spec, expr, now_ms, timeout)
                .map_err(|err| this.map_runtime_error(err))?;
            this.active = Some(ActiveOperation::ExpressionPklBinary);
            Ok(())
        })
    }

    #[wasm_bindgen(js_name = startSchema)]
    pub fn start_schema(
        &mut self,
        uri: JsValue,
        contents: JsValue,
        now: JsValue,
        timeout_ms: JsValue,
    ) -> Result<(), JsValue> {
        self.begin_call(now, timeout_ms, |this, now_ms, timeout| {
            let spec = build_source_spec(&uri, &contents)?;
            this.session
                .start_load_module(spec, now_ms, timeout)
                .map_err(|err| this.map_runtime_error(err))?;
            this.active = Some(ActiveOperation::Schema { extractor: None });
            Ok(())
        })
    }

    #[allow(clippy::too_many_arguments)]
    #[wasm_bindgen(js_name = startPowerAssertionProbe)]
    pub fn start_power_assertion_probe(
        &mut self,
        uri: JsValue,
        contents: JsValue,
        probe_expr: JsValue,
        location: JsValue,
        use_color: bool,
        now: JsValue,
        timeout_ms: JsValue,
    ) -> Result<(), JsValue> {
        self.begin_call(now, timeout_ms, |this, now_ms, timeout| {
            let spec = build_source_spec(&uri, &contents)?;
            let probe_str = rust_string_from_js(&probe_expr)?;
            let loc_str = rust_string_from_js(&location)?;
            this.session
                .start_evaluation_pkl_binary(spec, probe_str.as_str(), now_ms, timeout)
                .map_err(|err| this.map_runtime_error(err))?;
            this.active = Some(ActiveOperation::PowerAssertionProbe {
                probe_expr: probe_str,
                location: loc_str,
                use_color,
            });
            Ok(())
        })
    }

    pub fn poll(&mut self, now: JsValue, budget: JsValue) -> Result<JsValue, JsValue> {
        let now_ms = parse_u64_js(&now)?;
        let budget_u64 = parse_u64_js(&budget)?;
        let step_budget = usize::try_from(budget_u64)
            .map_err(|_| StructuredError::invalid_argument("budget out of usize range"))?;
        if step_budget == 0 {
            return Err(StructuredError::invalid_argument("budget must be positive").into());
        }

        if let Some(suspend) = self.pending_suspend.take() {
            return suspend.to_js();
        }

        if let Some((cap, msg)) = self.pending_capability_error.take() {
            self.active = None;
            return make_error_event(&StructuredError::capability_unavailable(cap, msg));
        }

        if self.active.is_none() {
            if self.is_closed() {
                return make_error_event(&StructuredError::closed("Evaluator has been closed."));
            }
            return Err(StructuredError::invalid_argument("no active evaluation").into());
        }

        // Drive locally resolvable Host effects in a bounded loop before yielding to JS.
        const MAX_LOCAL_STEPS: usize = 256;
        for _ in 0..MAX_LOCAL_STEPS {
            match self.session.poll(now_ms, step_budget) {
                HostPoll::Yielded => {
                    let obj = Object::new();
                    set_prop(&obj, "kind", JsValue::from_str("yielded"))?;
                    return Ok(obj.into());
                }
                HostPoll::Ready(result) => {
                    return self.handle_ready(result, now_ms);
                }
                HostPoll::Module { token, request } => {
                    let outcome = self.core.handle_module(token.clone(), request);
                    if let Some(event) = self.apply_outcome(&token, outcome, now_ms)? {
                        return Ok(event);
                    }
                }
                HostPoll::Read { token, .. } => {
                    let req = self
                        .session
                        .read_request(&token)
                        .map_err(|err| self.map_runtime_error(err))?;
                    let outcome = self.core.handle_read(token.clone(), &req);
                    if let Some(event) = self.apply_outcome(&token, outcome, now_ms)? {
                        return Ok(event);
                    }
                }
                HostPoll::Glob { token, request } => {
                    let outcome = self.core.handle_glob(token.clone(), request);
                    if let Some(event) = self.apply_outcome(&token, outcome, now_ms)? {
                        return Ok(event);
                    }
                }
                HostPoll::Log { token, event } => {
                    let outcome = self.core.handle_log(
                        token.clone(),
                        event.level,
                        event.message.clone(),
                        event.frame.clone(),
                        self.force_suspend_logs,
                    );
                    if let Some(ev) = self.apply_outcome(&token, outcome, now_ms)? {
                        return Ok(ev);
                    }
                }
                HostPoll::TransformFrame { token, frame } => {
                    let outcome = self
                        .core
                        .handle_transform_frame(token.clone(), (*frame).clone());
                    if let Some(ev) = self.apply_outcome(&token, outcome, now_ms)? {
                        return Ok(ev);
                    }
                }
            }
        }

        let obj = Object::new();
        set_prop(&obj, "kind", JsValue::from_str("yielded"))?;
        Ok(obj.into())
    }

    // -----------------------------------------------------------------------
    // Async Host completion methods
    // -----------------------------------------------------------------------

    #[wasm_bindgen(js_name = completeReadModule)]
    pub fn complete_read_module(
        &mut self,
        token: JsValue,
        resolved_uri: JsValue,
        text: JsValue,
        now: JsValue,
    ) -> Result<(), JsValue> {
        let tok = rust_string_from_js(&token)?;
        let resolved = pkl_string_from_js(&resolved_uri)?;
        let source = source_text_from_js(&text)?;
        let now_ms = parse_u64_js(&now)?;
        let outcome = self
            .core
            .complete_read_module(&tok, Ok((resolved, source)))?;
        self.apply_resumed_outcome(&tok, outcome, now_ms)
    }

    #[wasm_bindgen(js_name = rejectReadModule)]
    pub fn reject_read_module(
        &mut self,
        token: JsValue,
        error_kind: JsValue,
        message: JsValue,
        now: JsValue,
    ) -> Result<(), JsValue> {
        let tok = rust_string_from_js(&token)?;
        let kind = rust_string_from_js(&error_kind)?;
        let msg = opt_pkl_string(&message)?;
        let now_ms = parse_u64_js(&now)?;
        let outcome = self
            .core
            .complete_read_module(&tok, Err(parse_module_host_error(&kind, msg)))?;
        self.apply_resumed_outcome(&tok, outcome, now_ms)
    }

    #[wasm_bindgen(js_name = completeModuleHasElement)]
    pub fn complete_module_has_element(
        &mut self,
        token: JsValue,
        exists: bool,
        now: JsValue,
    ) -> Result<(), JsValue> {
        let tok = rust_string_from_js(&token)?;
        let now_ms = parse_u64_js(&now)?;
        let outcome = self.core.complete_module_has_element(&tok, Ok(exists))?;
        self.apply_resumed_outcome(&tok, outcome, now_ms)
    }

    #[wasm_bindgen(js_name = rejectModuleHasElement)]
    pub fn reject_module_has_element(
        &mut self,
        token: JsValue,
        error_kind: JsValue,
        message: JsValue,
        now: JsValue,
    ) -> Result<(), JsValue> {
        let tok = rust_string_from_js(&token)?;
        let kind = rust_string_from_js(&error_kind)?;
        let msg = opt_pkl_string(&message)?;
        let now_ms = parse_u64_js(&now)?;
        let outcome = self
            .core
            .complete_module_has_element(&tok, Err(parse_module_host_error(&kind, msg)))?;
        self.apply_resumed_outcome(&tok, outcome, now_ms)
    }

    #[wasm_bindgen(js_name = completeReadResource)]
    pub fn complete_read_resource(
        &mut self,
        token: JsValue,
        bytes_or_null: JsValue,
        now: JsValue,
    ) -> Result<(), JsValue> {
        let tok = rust_string_from_js(&token)?;
        let now_ms = parse_u64_js(&now)?;
        let data = if bytes_or_null.is_null() || bytes_or_null.is_undefined() {
            None
        } else {
            Some(bytes_from_js(&bytes_or_null)?)
        };
        let outcome = self.core.complete_read_resource(&tok, Ok(data))?;
        self.apply_resumed_outcome(&tok, outcome, now_ms)
    }

    #[wasm_bindgen(js_name = rejectReadResource)]
    pub fn reject_read_resource(
        &mut self,
        token: JsValue,
        error_kind: JsValue,
        message: JsValue,
        now: JsValue,
    ) -> Result<(), JsValue> {
        let tok = rust_string_from_js(&token)?;
        let kind = rust_string_from_js(&error_kind)?;
        let msg = opt_pkl_string(&message)?;
        let now_ms = parse_u64_js(&now)?;
        let outcome = self
            .core
            .complete_read_resource(&tok, Err(parse_read_failure(&kind, msg)))?;
        self.apply_resumed_outcome(&tok, outcome, now_ms)
    }

    #[wasm_bindgen(js_name = completeResourceHasElement)]
    pub fn complete_resource_has_element(
        &mut self,
        token: JsValue,
        exists: bool,
        now: JsValue,
    ) -> Result<(), JsValue> {
        let tok = rust_string_from_js(&token)?;
        let now_ms = parse_u64_js(&now)?;
        let outcome = self.core.complete_glob_has_element(&tok, Ok(exists))?;
        self.apply_resumed_outcome(&tok, outcome, now_ms)
    }

    #[wasm_bindgen(js_name = rejectResourceHasElement)]
    pub fn reject_resource_has_element(
        &mut self,
        token: JsValue,
        error_kind: JsValue,
        message: JsValue,
        now: JsValue,
    ) -> Result<(), JsValue> {
        let tok = rust_string_from_js(&token)?;
        let kind = rust_string_from_js(&error_kind)?;
        let msg = opt_pkl_string(&message)?;
        let now_ms = parse_u64_js(&now)?;
        let outcome = self
            .core
            .complete_glob_has_element(&tok, Err(parse_module_host_error(&kind, msg)))?;
        self.apply_resumed_outcome(&tok, outcome, now_ms)
    }

    #[wasm_bindgen(js_name = completeListElements)]
    pub fn complete_list_elements(
        &mut self,
        token: JsValue,
        elements: JsValue,
        now: JsValue,
    ) -> Result<(), JsValue> {
        let tok = rust_string_from_js(&token)?;
        let parsed = parse_path_elements_js(&elements)?;
        let now_ms = parse_u64_js(&now)?;
        let outcome = self.core.complete_list_elements(&tok, Ok(parsed))?;
        self.apply_resumed_outcome(&tok, outcome, now_ms)
    }

    #[wasm_bindgen(js_name = rejectListElements)]
    pub fn reject_list_elements(
        &mut self,
        token: JsValue,
        error_kind: JsValue,
        message: JsValue,
        now: JsValue,
    ) -> Result<(), JsValue> {
        let tok = rust_string_from_js(&token)?;
        let kind = rust_string_from_js(&error_kind)?;
        let msg = opt_pkl_string(&message)?;
        let now_ms = parse_u64_js(&now)?;
        let outcome = self
            .core
            .complete_list_elements(&tok, Err(parse_module_host_error(&kind, msg)))?;
        self.apply_resumed_outcome(&tok, outcome, now_ms)
    }

    #[wasm_bindgen(js_name = completePackageMetadata)]
    pub fn complete_package_metadata(
        &mut self,
        token: JsValue,
        bytes: JsValue,
        now: JsValue,
    ) -> Result<(), JsValue> {
        let tok = rust_string_from_js(&token)?;
        let data = bytes_from_js(&bytes)?;
        let now_ms = parse_u64_js(&now)?;
        let outcome = self.core.complete_package_metadata(&tok, Ok(data))?;
        self.apply_resumed_outcome(&tok, outcome, now_ms)
    }

    #[wasm_bindgen(js_name = rejectPackageMetadata)]
    pub fn reject_package_metadata(
        &mut self,
        token: JsValue,
        error_kind: JsValue,
        message: JsValue,
        now: JsValue,
    ) -> Result<(), JsValue> {
        let tok = rust_string_from_js(&token)?;
        let kind = rust_string_from_js(&error_kind)?;
        let msg = opt_pkl_string(&message)?;
        let now_ms = parse_u64_js(&now)?;
        let outcome = self
            .core
            .complete_package_metadata(&tok, Err(parse_module_host_error(&kind, msg)))?;
        self.apply_resumed_outcome(&tok, outcome, now_ms)
    }

    #[wasm_bindgen(js_name = completePackageArchive)]
    pub fn complete_package_archive(
        &mut self,
        token: JsValue,
        bytes: JsValue,
        now: JsValue,
    ) -> Result<(), JsValue> {
        let tok = rust_string_from_js(&token)?;
        let data = bytes_from_js(&bytes)?;
        let now_ms = parse_u64_js(&now)?;
        let outcome = self.core.complete_package_archive(&tok, Ok(data))?;
        self.apply_resumed_outcome(&tok, outcome, now_ms)
    }

    #[wasm_bindgen(js_name = rejectPackageArchive)]
    pub fn reject_package_archive(
        &mut self,
        token: JsValue,
        error_kind: JsValue,
        message: JsValue,
        now: JsValue,
    ) -> Result<(), JsValue> {
        let tok = rust_string_from_js(&token)?;
        let kind = rust_string_from_js(&error_kind)?;
        let msg = opt_pkl_string(&message)?;
        let now_ms = parse_u64_js(&now)?;
        let outcome = self
            .core
            .complete_package_archive(&tok, Err(parse_module_host_error(&kind, msg)))?;
        self.apply_resumed_outcome(&tok, outcome, now_ms)
    }

    #[wasm_bindgen(js_name = completeSecurityCheck)]
    pub fn complete_security_check(
        &mut self,
        token: JsValue,
        error_message: JsValue,
        now: JsValue,
    ) -> Result<(), JsValue> {
        let tok = rust_string_from_js(&token)?;
        let err_msg = opt_pkl_string(&error_message)?;
        let now_ms = parse_u64_js(&now)?;
        let outcome = self.core.complete_security_check(&tok, err_msg)?;
        self.apply_resumed_outcome(&tok, outcome, now_ms)
    }

    #[wasm_bindgen(js_name = acknowledgeLog)]
    pub fn acknowledge_log(
        &mut self,
        token: JsValue,
        error_message: JsValue,
        now: JsValue,
    ) -> Result<(), JsValue> {
        let tok = rust_string_from_js(&token)?;
        let err_msg = opt_pkl_string(&error_message)?;
        let now_ms = parse_u64_js(&now)?;
        self.core.take_pending_log(&tok)?;
        let res = match err_msg {
            Some(msg) => Err(LoggerFailure::new(msg)),
            None => Ok(()),
        };
        self.session
            .acknowledge_log(&tok, res, now_ms)
            .map_err(|err| self.map_runtime_error(err).into())
    }

    #[wasm_bindgen(js_name = completeFrameTransform)]
    pub fn complete_frame_transform(
        &mut self,
        token: JsValue,
        frame: JsValue,
        error_message: JsValue,
        now: JsValue,
    ) -> Result<(), JsValue> {
        let tok = rust_string_from_js(&token)?;
        let err_msg = opt_pkl_string(&error_message)?;
        let now_ms = parse_u64_js(&now)?;
        self.core.take_pending_frame_transform(&tok)?;
        let completion = if let Some(msg) = err_msg {
            Err(LoggerFailure::new(msg))
        } else {
            Ok(log_frame_from_js(&frame)?)
        };
        self.session
            .complete_frame_transform(&tok, completion, now_ms)
            .map_err(|err| self.map_runtime_error(err).into())
    }

    // -----------------------------------------------------------------------
    // Lifecycle & cleanup
    // -----------------------------------------------------------------------

    pub fn cancel(&mut self) {
        self.core.cancel_pending();
        self.pending_suspend = None;
        self.active = None;
        self.session.cancel();
    }

    pub fn close(&mut self) {
        if !self.closed {
            self.closed = true;
            self.core.close();
            self.pending_suspend = None;
            self.pending_capability_error = None;
            self.active = None;
            self.session.close();
        }
    }

    #[wasm_bindgen(js_name = drainCancelledOperations)]
    pub fn drain_cancelled_operations(&mut self) -> Array {
        let mut all = self.core.drain_cancelled_tokens();
        for tok in self
            .session
            .drain_cancelled_reads()
            .into_iter()
            .chain(self.session.drain_cancelled_modules())
            .chain(self.session.drain_cancelled_globs())
        {
            if !all.contains(&tok) {
                all.push(tok);
            }
        }
        let arr = Array::new();
        for tok in all {
            arr.push(&JsValue::from_str(&tok));
        }
        arr
    }

    pub fn collect(&mut self) -> u32 {
        self.session.collect(&[]).slots as u32
    }
}

impl WasmEvaluator {
    fn begin_call(
        &mut self,
        now: JsValue,
        timeout_ms: JsValue,
        f: impl FnOnce(&mut Self, u64, Option<u64>) -> Result<(), StructuredError>,
    ) -> Result<(), JsValue> {
        if self.is_closed() {
            return Err(StructuredError::closed("Evaluator has been closed.").into());
        }
        if self.active.is_some() {
            return Err(StructuredError::invalid_argument(
                "Evaluator already has an active evaluation in progress.",
            )
            .into());
        }
        let now_ms = parse_u64_js(&now)?;
        let timeout = parse_opt_u64_js(&timeout_ms)?;
        self.core.reset_call_flags();
        self.active_timeout_ms = timeout;
        self.pending_suspend = None;
        self.pending_capability_error = None;
        f(self, now_ms, timeout).map_err(Into::into)
    }

    fn map_runtime_error(&self, err: RuntimeError) -> StructuredError {
        match err.kind {
            ErrorKind::Timeout => StructuredError::timeout(format_timeout_message(
                self.active_timeout_ms.unwrap_or(0),
            )),
            ErrorKind::Closed => StructuredError::closed("Evaluator has been closed."),
            ErrorKind::Cancelled => StructuredError::cancelled("Evaluation was cancelled."),
            _ => {
                let rendered = self.session.format_error_text(err);
                if self.core.saw_security_denial || is_security_rendered(&rendered) {
                    StructuredError::security(rendered)
                } else {
                    StructuredError::evaluation(rendered)
                }
            }
        }
    }

    fn apply_outcome(
        &mut self,
        token: &str,
        outcome: WasmHostOutcome,
        now_ms: u64,
    ) -> Result<Option<JsValue>, JsValue> {
        match outcome {
            WasmHostOutcome::CompleteModule(comp) => {
                self.session
                    .complete_module(token, comp, now_ms)
                    .map_err(|err| self.map_runtime_error(err))?;
                Ok(None)
            }
            WasmHostOutcome::CompleteRead(res) => {
                self.session
                    .complete_read(token, res, now_ms)
                    .map_err(|err| self.map_runtime_error(err))?;
                Ok(None)
            }
            WasmHostOutcome::CompleteGlob(comp) => {
                self.session
                    .complete_glob(token, comp, now_ms)
                    .map_err(|err| self.map_runtime_error(err))?;
                Ok(None)
            }
            WasmHostOutcome::AckLog => {
                self.session
                    .acknowledge_log(token, Ok(()), now_ms)
                    .map_err(|err| self.map_runtime_error(err))?;
                Ok(None)
            }
            WasmHostOutcome::CompleteFrame(frame) => {
                self.session
                    .complete_frame_transform(token, Ok(frame), now_ms)
                    .map_err(|err| self.map_runtime_error(err))?;
                Ok(None)
            }
            WasmHostOutcome::Suspend(req) => Ok(Some(req.to_js()?)),
            WasmHostOutcome::CapabilityUnavailable {
                capability,
                message,
            } => {
                self.inject_capability_unwind(token, capability, message, now_ms);
                Ok(None)
            }
        }
    }

    fn apply_resumed_outcome(
        &mut self,
        token: &str,
        outcome: WasmHostOutcome,
        now_ms: u64,
    ) -> Result<(), JsValue> {
        match outcome {
            WasmHostOutcome::CompleteModule(comp) => self
                .session
                .complete_module(token, comp, now_ms)
                .map_err(|err| self.map_runtime_error(err).into()),
            WasmHostOutcome::CompleteRead(res) => self
                .session
                .complete_read(token, res, now_ms)
                .map_err(|err| self.map_runtime_error(err).into()),
            WasmHostOutcome::CompleteGlob(comp) => self
                .session
                .complete_glob(token, comp, now_ms)
                .map_err(|err| self.map_runtime_error(err).into()),
            WasmHostOutcome::AckLog => self
                .session
                .acknowledge_log(token, Ok(()), now_ms)
                .map_err(|err| self.map_runtime_error(err).into()),
            WasmHostOutcome::CompleteFrame(frame) => self
                .session
                .complete_frame_transform(token, Ok(frame), now_ms)
                .map_err(|err| self.map_runtime_error(err).into()),
            WasmHostOutcome::Suspend(req) => {
                self.pending_suspend = Some(req);
                Ok(())
            }
            WasmHostOutcome::CapabilityUnavailable {
                capability,
                message,
            } => {
                self.inject_capability_unwind(token, capability, message, now_ms);
                Ok(())
            }
        }
    }

    /// Cleanly unwinds the currently active VM operation after a capability-unavailable
    /// rejection without poisoning the `SourceSession` for subsequent valid evaluations.
    fn inject_capability_unwind(
        &mut self,
        token: &str,
        capability: &'static str,
        message: String,
        now_ms: u64,
    ) {
        self.pending_capability_error = Some((capability, message.clone()));
        let msg_pkl: PklString = message.into();
        if let Ok(req) = self.session.module_request(token) {
            let err = ModuleHostError::Unsupported(msg_pkl);
            let comp = match req.kind {
                pkl_runtime::modules::ModuleRequestKind::CheckImport { .. } => {
                    ModuleCompletion::ImportChecked(Err(err))
                }
                pkl_runtime::modules::ModuleRequestKind::SelectKey { .. } => {
                    ModuleCompletion::KeySelected(Err(err))
                }
                pkl_runtime::modules::ModuleRequestKind::QueryCachePolicy { .. } => {
                    ModuleCompletion::CachePolicy(Err(err))
                }
                pkl_runtime::modules::ModuleRequestKind::QueryFragmentPaths { .. } => {
                    ModuleCompletion::FragmentPaths(Err(err))
                }
                pkl_runtime::modules::ModuleRequestKind::ResolveImportUri { .. } => {
                    ModuleCompletion::ImportUri(Err(err))
                }
                pkl_runtime::modules::ModuleRequestKind::Capabilities { .. } => {
                    ModuleCompletion::Capabilities(Err(err))
                }
                pkl_runtime::modules::ModuleRequestKind::HasElement { .. } => {
                    ModuleCompletion::HasElement(Err(err))
                }
                pkl_runtime::modules::ModuleRequestKind::ResolveKey { .. } => {
                    ModuleCompletion::KeyResolved(Err(err))
                }
                pkl_runtime::modules::ModuleRequestKind::Resolve { .. } => {
                    ModuleCompletion::Resolved(Err(err))
                }
                pkl_runtime::modules::ModuleRequestKind::LoadSource { .. } => {
                    ModuleCompletion::Source(Err(err))
                }
            };
            let _ = self.session.complete_module(token, comp, now_ms);
        } else if self.session.read_request(token).is_ok() {
            let _ = self.session.complete_read(
                token,
                Err(ReadFailure::EvalError {
                    message: msg_pkl,
                    hint: None,
                }),
                now_ms,
            );
        } else if let Ok(req) = self.session.glob_request(token) {
            let err = GlobHostError::Failure(ModuleHostError::Unsupported(msg_pkl));
            let comp = pkl_runtime::glob::failure_completion(&req, err);
            let _ = self.session.complete_glob(token, comp, now_ms);
        }
    }

    fn handle_ready(
        &mut self,
        result: Result<Value, RuntimeError>,
        now_ms: u64,
    ) -> Result<JsValue, JsValue> {
        if let Some((cap, msg)) = self.pending_capability_error.take() {
            self.active = None;
            return make_error_event(&StructuredError::capability_unavailable(cap, msg));
        }

        let Some(op) = self.active.take() else {
            return Err(StructuredError::invalid_argument("missing active operation").into());
        };

        match op {
            ActiveOperation::OutputText | ActiveOperation::Expression => match result {
                Ok(val) => match SourceSession::result_string(val) {
                    Ok(text) => make_ready_event("string", js_string_from_pkl(&text).into()),
                    Err(err) => make_error_event(&self.map_runtime_error(err)),
                },
                Err(err) => make_error_event(&self.map_runtime_error(err)),
            },
            ActiveOperation::OutputBytes | ActiveOperation::ExpressionPklBinary => match result {
                Ok(val) => match SourceSession::result_bytes(val) {
                    Ok(bytes) => make_ready_event("bytes", js_from_bytes(&bytes).into()),
                    Err(err) => make_error_event(&self.map_runtime_error(err)),
                },
                Err(err) => make_error_event(&self.map_runtime_error(err)),
            },
            ActiveOperation::Schema { extractor } => {
                let value = match result {
                    Ok(v) => v,
                    Err(err) => return make_error_event(&self.map_runtime_error(err)),
                };
                let (mut ext, last_result) = match extractor {
                    None => match SchemaExtractor::new(value) {
                        Ok(ext) => (ext, None),
                        Err(err) => return make_error_event(&self.map_runtime_error(err)),
                    },
                    Some(ext) => (ext, Some(value)),
                };
                match ext.step(
                    &mut self.session,
                    last_result,
                    now_ms,
                    self.active_timeout_ms,
                ) {
                    Ok(SchemaStep::RunPending) => {
                        self.active = Some(ActiveOperation::Schema {
                            extractor: Some(ext),
                        });
                        let obj = Object::new();
                        set_prop(&obj, "kind", JsValue::from_str("yielded"))?;
                        Ok(obj.into())
                    }
                    Ok(SchemaStep::Done(schema)) => {
                        let js = module_schema_to_js(&schema)?;
                        make_ready_event("schema", js)
                    }
                    Err(err) => make_error_event(&self.map_runtime_error(err)),
                }
            }
            ActiveOperation::PowerAssertionProbe {
                probe_expr,
                location,
                use_color,
            } => {
                let failure_js = match result {
                    Err(err) => {
                        if let Some(ref td) = err.type_diagnostic
                            && let Some((body_span, observations)) =
                                extract_constraint_observations(td)
                        {
                            let mut sm = SourceMap::default();
                            let sid = sm.add("repl:text", probe_expr.as_str());
                            if let Some(src) = sm.get(sid) {
                                let failure = format_fact_failure_with_power_assertions(
                                    src,
                                    body_span,
                                    observations,
                                    &location,
                                    use_color,
                                );
                                let obj = Object::new();
                                set_prop(&obj, "kind", JsValue::from_str(&failure.kind))?;
                                set_prop(&obj, "message", JsValue::from_str(&failure.message))?;
                                obj.into()
                            } else {
                                JsValue::NULL
                            }
                        } else {
                            JsValue::NULL
                        }
                    }
                    Ok(_) => JsValue::NULL,
                };
                make_ready_event("powerAssertionProbe", failure_js)
            }
        }
    }
}

fn build_source_spec(
    uri: &JsValue,
    contents: &JsValue,
) -> Result<ModuleSourceSpec, StructuredError> {
    let uri_pkl = pkl_string_from_js(uri)?;
    let contents_opt = if contents.is_null() || contents.is_undefined() {
        None
    } else {
        Some(source_text_from_js(contents)?)
    };
    Ok(ModuleSourceSpec {
        uri: uri_pkl,
        contents: contents_opt,
    })
}

fn opt_pkl_string(val: &JsValue) -> Result<Option<PklString>, StructuredError> {
    if val.is_null() || val.is_undefined() {
        Ok(None)
    } else {
        pkl_string_from_js(val).map(Some)
    }
}

fn make_ready_event(value_type: &str, value: JsValue) -> Result<JsValue, JsValue> {
    let obj = Object::new();
    set_prop(&obj, "kind", JsValue::from_str("ready"))?;
    set_prop(&obj, "valueType", JsValue::from_str(value_type))?;
    set_prop(&obj, "value", value)?;
    Ok(obj.into())
}

fn make_error_event(err: &StructuredError) -> Result<JsValue, JsValue> {
    let obj = Object::new();
    set_prop(&obj, "kind", JsValue::from_str("error"))?;
    set_prop(&obj, "errorKind", JsValue::from_str(err.kind))?;
    set_prop(&obj, "message", js_string_from_pkl(&err.message).into())?;
    if let Some(cap) = err.capability {
        set_prop(&obj, "capability", JsValue::from_str(cap))?;
    }
    Ok(obj.into())
}

fn is_security_rendered(rendered: &PklString) -> bool {
    let text = rendered.to_java_utf8();
    text.contains("Refusing to load module `")
        || text.contains("Refusing to import module `")
        || text.contains("Refusing to read resource `")
        || text.contains("SecurityManagerException")
}

fn extract_constraint_observations(
    td: &TypeDiagnostic,
) -> Option<(
    pkl_runtime::source::Span,
    &[pkl_runtime::diagnostic::PowerAssertionValue],
)> {
    match td {
        TypeDiagnostic::AtSource { diagnostic, .. } => extract_constraint_observations(diagnostic),
        TypeDiagnostic::Constraint {
            body_span,
            observations: Some(obs),
            ..
        } => Some((*body_span, obs.as_slice())),
        _ => None,
    }
}

// Keep unused import check satisfied when decode module is referenced cleanly
#[allow(dead_code)]
fn _assert_decode_wired(
    val: &pkl_runtime::pkl_binary_decode::DecodedValue,
) -> Result<JsValue, JsValue> {
    decoded_value_to_js(val)
}
