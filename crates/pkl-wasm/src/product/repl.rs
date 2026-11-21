//! Stateful cooperative `WasmReplSession` wrapping `pkl_runtime::repl::ReplSession`
//! and `WasmHostCore`.

use super::{
    host_core::{
        WasmHostCore, WasmHostOutcome, WasmSuspendRequest, parse_host_config_js,
        parse_module_host_error, parse_path_elements_js, parse_read_failure,
    },
    util::{
        StructuredError, bytes_from_js, js_string_from_pkl, log_frame_from_js, parse_opt_u64_js,
        parse_u64_js, pkl_string_from_js, rust_string_from_js, set_prop, source_text_from_js,
    },
};
use js_sys::{Array, Object};
use pkl_runtime::{
    glob::GlobHostError,
    host::ReadFailure,
    logging::{FrameTransformer, LogDelivery, LoggerFailure},
    modules::{ModuleCompletion, ModuleHostError},
    repl::{ReplPoll, ReplRequest, ReplResponse, ReplSession, ReplSessionOptions},
    value::PklString,
};
use wasm_bindgen::prelude::*;

#[wasm_bindgen(js_name = WasmReplSession)]
pub struct WasmReplSession {
    core: WasmHostCore,
    repl: ReplSession,
    pending_suspend: Option<WasmSuspendRequest>,
    pending_capability_error: Option<(&'static str, String)>,
    closed: bool,
}

#[wasm_bindgen]
impl WasmReplSession {
    #[wasm_bindgen(constructor)]
    pub fn new(options: JsValue, working_dir_uri: JsValue) -> Result<WasmReplSession, JsValue> {
        let config = parse_host_config_js(&options)?;
        let working_dir = if working_dir_uri.is_null() || working_dir_uri.is_undefined() {
            PklString::from("file:///")
        } else {
            pkl_string_from_js(&working_dir_uri)?
        };
        let repl_opts = ReplSessionOptions {
            working_dir_uri: working_dir,
            color: config.use_color,
            trace_mode: config.trace_mode,
            log_delivery: LogDelivery::Host,
            frame_transformer: if config.has_custom_frame_transformer {
                FrameTransformer::Host
            } else {
                FrameTransformer::Default
            },
            power_assertions: config.power_assertions,
            platform_data: config.platform_metadata.is_some(),
        };
        let core = WasmHostCore::new(config)?;
        let repl = ReplSession::new(repl_opts);
        Ok(Self {
            core,
            repl,
            pending_suspend: None,
            pending_capability_error: None,
            closed: false,
        })
    }

    #[wasm_bindgen(js_name = isClosed)]
    pub fn is_closed(&self) -> bool {
        self.closed
    }

    #[wasm_bindgen(js_name = startEval)]
    #[allow(clippy::too_many_arguments)]
    pub fn start_eval(
        &mut self,
        id: JsValue,
        text: JsValue,
        eval_definitions: bool,
        force_results: bool,
        now: JsValue,
        timeout_ms: JsValue,
    ) -> Result<(), JsValue> {
        self.ensure_open()?;
        let req = ReplRequest::Eval {
            id: rust_string_from_js(&id)?,
            text: rust_string_from_js(&text)?,
            eval_definitions,
            force_results,
        };
        self.start_request(req, now, timeout_ms)
    }

    #[wasm_bindgen(js_name = startLoad)]
    pub fn start_load(
        &mut self,
        id: JsValue,
        uri: JsValue,
        now: JsValue,
        timeout_ms: JsValue,
    ) -> Result<(), JsValue> {
        self.ensure_open()?;
        let req = ReplRequest::Load {
            id: rust_string_from_js(&id)?,
            uri: rust_string_from_js(&uri)?,
        };
        self.start_request(req, now, timeout_ms)
    }

    #[wasm_bindgen(js_name = startComplete)]
    pub fn start_complete(
        &mut self,
        id: JsValue,
        text: JsValue,
        now: JsValue,
        timeout_ms: JsValue,
    ) -> Result<(), JsValue> {
        self.ensure_open()?;
        let req = ReplRequest::Completion {
            id: rust_string_from_js(&id)?,
            text: rust_string_from_js(&text)?,
        };
        self.start_request(req, now, timeout_ms)
    }

    #[wasm_bindgen(js_name = startReset)]
    pub fn start_reset(
        &mut self,
        id: JsValue,
        now: JsValue,
        timeout_ms: JsValue,
    ) -> Result<(), JsValue> {
        self.ensure_open()?;
        let req = ReplRequest::Reset {
            id: rust_string_from_js(&id)?,
        };
        self.start_request(req, now, timeout_ms)
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
            return make_repl_error_event(&StructuredError::capability_unavailable(cap, msg));
        }

        const MAX_LOCAL_STEPS: usize = 256;
        for _ in 0..MAX_LOCAL_STEPS {
            match self.repl.poll(now_ms, step_budget) {
                ReplPoll::Yielded => {
                    let obj = Object::new();
                    set_prop(&obj, "kind", JsValue::from_str("yielded"))?;
                    return Ok(obj.into());
                }
                ReplPoll::Ready(responses) => {
                    if let Some((cap, msg)) = self.pending_capability_error.take() {
                        return make_repl_error_event(&StructuredError::capability_unavailable(
                            cap, msg,
                        ));
                    }
                    let arr = Array::new();
                    for resp in responses {
                        arr.push(&repl_response_to_js(resp)?);
                    }
                    let obj = Object::new();
                    set_prop(&obj, "kind", JsValue::from_str("ready"))?;
                    set_prop(&obj, "responses", arr.into())?;
                    return Ok(obj.into());
                }
                ReplPoll::Module { token, request } => {
                    let outcome = self.core.handle_module(token.clone(), request);
                    if let Some(ev) = self.apply_outcome(&token, outcome, now_ms)? {
                        return Ok(ev);
                    }
                }
                ReplPoll::Read { token, .. } => {
                    let req = self
                        .repl
                        .read_request(&token)
                        .map_err(|err| StructuredError::evaluation(err.message.clone()))?;
                    let outcome = self.core.handle_read(token.clone(), &req);
                    if let Some(ev) = self.apply_outcome(&token, outcome, now_ms)? {
                        return Ok(ev);
                    }
                }
                ReplPoll::Glob { token, request } => {
                    let outcome = self.core.handle_glob(token.clone(), request);
                    if let Some(ev) = self.apply_outcome(&token, outcome, now_ms)? {
                        return Ok(ev);
                    }
                }
                ReplPoll::Log { token, event } => {
                    let outcome = self.core.handle_log(
                        token.clone(),
                        event.level,
                        event.message.clone(),
                        event.frame.clone(),
                        false,
                    );
                    if let Some(ev) = self.apply_outcome(&token, outcome, now_ms)? {
                        return Ok(ev);
                    }
                }
                ReplPoll::TransformFrame { token, frame } => {
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
        self.apply_resumed(&tok, outcome, now_ms)
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
        self.apply_resumed(&tok, outcome, now_ms)
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
        self.apply_resumed(&tok, outcome, now_ms)
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
        self.apply_resumed(&tok, outcome, now_ms)
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
        self.apply_resumed(&tok, outcome, now_ms)
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
        self.apply_resumed(&tok, outcome, now_ms)
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
        self.apply_resumed(&tok, outcome, now_ms)
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
        self.apply_resumed(&tok, outcome, now_ms)
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
        self.apply_resumed(&tok, outcome, now_ms)
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
        self.apply_resumed(&tok, outcome, now_ms)
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
        self.apply_resumed(&tok, outcome, now_ms)
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
        self.apply_resumed(&tok, outcome, now_ms)
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
        self.apply_resumed(&tok, outcome, now_ms)
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
        self.apply_resumed(&tok, outcome, now_ms)
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
        self.apply_resumed(&tok, outcome, now_ms)
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
        self.repl
            .acknowledge_log(&tok, res, now_ms)
            .map_err(|err| StructuredError::evaluation(err.message).into())
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
        self.repl
            .complete_frame_transform(&tok, completion, now_ms)
            .map_err(|err| StructuredError::evaluation(err.message).into())
    }

    pub fn cancel(&mut self) {
        self.core.cancel_pending();
        self.pending_suspend = None;
        self.repl.cancel();
    }

    pub fn close(&mut self) {
        if !self.closed {
            self.closed = true;
            self.core.close();
            self.pending_suspend = None;
            self.pending_capability_error = None;
            self.repl.close();
        }
    }

    #[wasm_bindgen(js_name = drainCancelledOperations)]
    pub fn drain_cancelled_operations(&mut self) -> Array {
        let arr = Array::new();
        for tok in self.core.drain_cancelled_tokens() {
            arr.push(&JsValue::from_str(&tok));
        }
        arr
    }
}

impl WasmReplSession {
    fn ensure_open(&self) -> Result<(), StructuredError> {
        if self.closed {
            Err(StructuredError::closed("REPL session has been closed."))
        } else {
            Ok(())
        }
    }

    fn start_request(
        &mut self,
        request: ReplRequest,
        now: JsValue,
        timeout_ms: JsValue,
    ) -> Result<(), JsValue> {
        let now_ms = parse_u64_js(&now)?;
        let timeout = parse_opt_u64_js(&timeout_ms)?;
        self.core.reset_call_flags();
        self.pending_suspend = None;
        self.pending_capability_error = None;
        self.repl
            .start_request(request, now_ms, timeout)
            .map_err(|err| StructuredError::evaluation(err.message).into())
    }

    fn apply_outcome(
        &mut self,
        token: &str,
        outcome: WasmHostOutcome,
        now_ms: u64,
    ) -> Result<Option<JsValue>, JsValue> {
        match outcome {
            WasmHostOutcome::CompleteModule(comp) => {
                self.repl
                    .complete_module(token, comp, now_ms)
                    .map_err(|err| StructuredError::evaluation(err.message))?;
                Ok(None)
            }
            WasmHostOutcome::CompleteRead(res) => {
                self.repl
                    .complete_read(token, res, now_ms)
                    .map_err(|err| StructuredError::evaluation(err.message))?;
                Ok(None)
            }
            WasmHostOutcome::CompleteGlob(comp) => {
                self.repl
                    .complete_glob(token, comp, now_ms)
                    .map_err(|err| StructuredError::evaluation(err.message))?;
                Ok(None)
            }
            WasmHostOutcome::AckLog => {
                self.repl
                    .acknowledge_log(token, Ok(()), now_ms)
                    .map_err(|err| StructuredError::evaluation(err.message))?;
                Ok(None)
            }
            WasmHostOutcome::CompleteFrame(frame) => {
                self.repl
                    .complete_frame_transform(token, Ok(frame), now_ms)
                    .map_err(|err| StructuredError::evaluation(err.message))?;
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

    fn apply_resumed(
        &mut self,
        token: &str,
        outcome: WasmHostOutcome,
        now_ms: u64,
    ) -> Result<(), JsValue> {
        match outcome {
            WasmHostOutcome::CompleteModule(comp) => self
                .repl
                .complete_module(token, comp, now_ms)
                .map_err(|err| StructuredError::evaluation(err.message).into()),
            WasmHostOutcome::CompleteRead(res) => self
                .repl
                .complete_read(token, res, now_ms)
                .map_err(|err| StructuredError::evaluation(err.message).into()),
            WasmHostOutcome::CompleteGlob(comp) => self
                .repl
                .complete_glob(token, comp, now_ms)
                .map_err(|err| StructuredError::evaluation(err.message).into()),
            WasmHostOutcome::AckLog => self
                .repl
                .acknowledge_log(token, Ok(()), now_ms)
                .map_err(|err| StructuredError::evaluation(err.message).into()),
            WasmHostOutcome::CompleteFrame(frame) => self
                .repl
                .complete_frame_transform(token, Ok(frame), now_ms)
                .map_err(|err| StructuredError::evaluation(err.message).into()),
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

    fn inject_capability_unwind(
        &mut self,
        token: &str,
        capability: &'static str,
        message: String,
        now_ms: u64,
    ) {
        self.pending_capability_error = Some((capability, message.clone()));
        let msg_pkl: PklString = message.into();
        if self.repl.read_request(token).is_ok() {
            let _ = self.repl.complete_read(
                token,
                Err(ReadFailure::EvalError {
                    message: msg_pkl,
                    hint: None,
                }),
                now_ms,
            );
            return;
        }
        // Try module or glob fallback completion (unsupported error unwinds the current REPL item)
        let err = ModuleHostError::Unsupported(msg_pkl.clone());
        let _ = self
            .repl
            .complete_module(token, ModuleCompletion::Resolved(Err(err.clone())), now_ms)
            .or_else(|_| {
                self.repl.complete_module(
                    token,
                    ModuleCompletion::KeyResolved(Err(err.clone())),
                    now_ms,
                )
            })
            .or_else(|_| {
                self.repl.complete_module(
                    token,
                    ModuleCompletion::HasElement(Err(err.clone())),
                    now_ms,
                )
            })
            .or_else(|_| {
                self.repl.complete_glob(
                    token,
                    pkl_runtime::glob::GlobCompletion::Elements(Err(GlobHostError::Failure(
                        err.clone(),
                    ))),
                    now_ms,
                )
            })
            .or_else(|_| {
                self.repl.complete_glob(
                    token,
                    pkl_runtime::glob::GlobCompletion::HasElement(Err(GlobHostError::Failure(err))),
                    now_ms,
                )
            });
    }
}

fn opt_pkl_string(val: &JsValue) -> Result<Option<PklString>, StructuredError> {
    if val.is_null() || val.is_undefined() {
        Ok(None)
    } else {
        pkl_string_from_js(val).map(Some)
    }
}

fn repl_response_to_js(resp: ReplResponse) -> Result<JsValue, JsValue> {
    let obj = Object::new();
    match resp {
        ReplResponse::EvalSuccess { result } => {
            set_prop(&obj, "kind", JsValue::from_str("evalSuccess"))?;
            set_prop(&obj, "result", JsValue::from_str(&result))?;
        }
        ReplResponse::EvalError { message } => {
            set_prop(&obj, "kind", JsValue::from_str("evalError"))?;
            set_prop(&obj, "message", JsValue::from_str(&message))?;
        }
        ReplResponse::Completion { members } => {
            set_prop(&obj, "kind", JsValue::from_str("completion"))?;
            let arr = Array::new();
            for m in members {
                arr.push(&JsValue::from_str(&m));
            }
            set_prop(&obj, "members", arr.into())?;
        }
        ReplResponse::IncompleteInput => {
            set_prop(&obj, "kind", JsValue::from_str("incompleteInput"))?;
        }
        ReplResponse::InvalidRequest { message } => {
            set_prop(&obj, "kind", JsValue::from_str("invalidRequest"))?;
            set_prop(&obj, "message", JsValue::from_str(&message))?;
        }
        ReplResponse::InternalError { message } => {
            set_prop(&obj, "kind", JsValue::from_str("internalError"))?;
            set_prop(&obj, "message", JsValue::from_str(&message))?;
        }
    }
    Ok(obj.into())
}

fn make_repl_error_event(err: &StructuredError) -> Result<JsValue, JsValue> {
    let obj = Object::new();
    set_prop(&obj, "kind", JsValue::from_str("error"))?;
    set_prop(&obj, "errorKind", JsValue::from_str(err.kind))?;
    set_prop(&obj, "message", js_string_from_pkl(&err.message).into())?;
    if let Some(cap) = err.capability {
        set_prop(&obj, "capability", JsValue::from_str(cap))?;
    }
    Ok(obj.into())
}
