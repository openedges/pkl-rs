//! Cooperative async static import graph analyzer (`analyzeImportGraph`) wrapping
//! `pkl_runtime::modules::analyze::ImportGraphAnalysis` and `WasmHostCore`.

use super::{
    host_core::{
        WasmHostCore, WasmHostOutcome, WasmSuspendRequest, parse_host_config_js,
        parse_module_host_error,
    },
    util::{
        StructuredError, bytes_from_js, define_own_prop, js_string_from_pkl, pkl_string_from_js,
        rust_string_from_js, set_prop, source_text_from_js,
    },
};
use js_sys::{Array, Object};
use pkl_runtime::{
    modules::analyze::{
        AnalysisFailure, AnalysisPoll, FailureCause, ImportGraph, ImportGraphAnalysis,
    },
    value::PklString,
};
use std::collections::VecDeque;
use wasm_bindgen::prelude::*;

enum AnalyzerGlobSecurityState {
    CheckingPattern {
        token: String,
        base_uri: PklString,
        pattern_uri: PklString,
    },
    CheckingResults {
        token: String,
        pending: VecDeque<PklString>,
        checked: Vec<PklString>,
    },
}

#[wasm_bindgen(js_name = WasmImportAnalyzer)]
pub struct WasmImportAnalyzer {
    core: WasmHostCore,
    analysis: ImportGraphAnalysis,
    pending_request_id: Option<u64>,
    pending_suspend: Option<WasmSuspendRequest>,
    pending_capability_error: Option<(&'static str, String)>,
    pending_glob_security: Option<AnalyzerGlobSecurityState>,
    pending_glob_emit: Option<(PklString, PklString)>,
    glob_check_seq: u64,
    closed: bool,
}

#[wasm_bindgen]
impl WasmImportAnalyzer {
    #[wasm_bindgen(constructor)]
    pub fn new(uris: JsValue, options: JsValue) -> Result<WasmImportAnalyzer, JsValue> {
        if !Array::is_array(&uris) {
            return Err(StructuredError::invalid_argument(
                "`uris` must be an array of URI strings",
            )
            .into());
        }
        let mut root_uris = Vec::new();
        for item in Array::from(&uris).iter() {
            root_uris.push(pkl_string_from_js(&item)?);
        }
        let config = parse_host_config_js(&options)?;
        let core = WasmHostCore::new(config)?;
        let analysis = ImportGraphAnalysis::new(root_uris);
        Ok(Self {
            core,
            analysis,
            pending_request_id: None,
            pending_suspend: None,
            pending_capability_error: None,
            pending_glob_security: None,
            pending_glob_emit: None,
            glob_check_seq: 0,
            closed: false,
        })
    }

    pub fn poll(&mut self) -> Result<JsValue, JsValue> {
        if self.closed {
            return make_error_event(&StructuredError::closed("Import analyzer is closed."));
        }
        if let Some(suspend) = self.pending_suspend.take() {
            return suspend.to_js();
        }
        if let Some((base_uri, pattern_uri)) = self.pending_glob_emit.take() {
            return make_resolve_analysis_glob_event(&base_uri, &pattern_uri);
        }
        if let Some((cap, msg)) = self.pending_capability_error.take() {
            return make_error_event(&StructuredError::capability_unavailable(cap, msg));
        }

        const MAX_STEPS: usize = 512;
        for _ in 0..MAX_STEPS {
            if let Some(glob_info) = self.analysis.take_pending_glob() {
                if self.core.config.has_custom_security_check_resolve {
                    self.glob_check_seq += 1;
                    let token = format!("analyze-glob-pat:{}", self.glob_check_seq);
                    let pattern_uri = glob_info.pattern_uri.clone();
                    self.pending_glob_security = Some(AnalyzerGlobSecurityState::CheckingPattern {
                        token: token.clone(),
                        base_uri: glob_info.base_uri,
                        pattern_uri: glob_info.pattern_uri,
                    });
                    return WasmSuspendRequest::CheckResolveModule {
                        token,
                        uri: pattern_uri,
                    }
                    .to_js();
                }
                if let Err(err) = self
                    .core
                    .policy
                    .check_resolve_module(&glob_info.pattern_uri)
                {
                    self.analysis.fail_glob(FailureCause::Host(err));
                    continue;
                }
                return make_resolve_analysis_glob_event(
                    &glob_info.base_uri,
                    &glob_info.pattern_uri,
                );
            }

            match self.analysis.poll() {
                AnalysisPoll::Closed => {
                    return make_error_event(&StructuredError::closed(
                        "Import analyzer is closed.",
                    ));
                }
                AnalysisPoll::Ready(Ok(graph)) => {
                    return import_graph_to_ready_js(&graph);
                }
                AnalysisPoll::Ready(Err(failure)) => {
                    return make_error_event(&format_analysis_failure(&failure));
                }
                AnalysisPoll::Request(request) => {
                    let req_id = request.id;
                    let tok = format!("analyze:{req_id}");
                    let outcome = self.core.handle_module(tok, *request);
                    match outcome {
                        WasmHostOutcome::CompleteModule(comp) => {
                            self.analysis
                                .complete(req_id, comp)
                                .map_err(StructuredError::evaluation)?;
                        }
                        WasmHostOutcome::Suspend(suspend) => {
                            self.pending_request_id = Some(req_id);
                            return suspend.to_js();
                        }
                        WasmHostOutcome::CapabilityUnavailable {
                            capability,
                            message,
                        } => {
                            return make_error_event(&StructuredError::capability_unavailable(
                                capability, message,
                            ));
                        }
                        _ => {
                            return make_error_event(&StructuredError::evaluation(
                                "unexpected host outcome during import graph analysis",
                            ));
                        }
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
    ) -> Result<(), JsValue> {
        let tok = rust_string_from_js(&token)?;
        let resolved = pkl_string_from_js(&resolved_uri)?;
        let source = source_text_from_js(&text)?;
        let outcome = self
            .core
            .complete_read_module(&tok, Ok((resolved, source)))?;
        self.apply_resumed(outcome)
    }

    #[wasm_bindgen(js_name = rejectReadModule)]
    pub fn reject_read_module(
        &mut self,
        token: JsValue,
        error_kind: JsValue,
        message: JsValue,
    ) -> Result<(), JsValue> {
        let tok = rust_string_from_js(&token)?;
        let kind = rust_string_from_js(&error_kind)?;
        let msg = opt_pkl_string(&message)?;
        let outcome = self
            .core
            .complete_read_module(&tok, Err(parse_module_host_error(&kind, msg)))?;
        self.apply_resumed(outcome)
    }

    #[wasm_bindgen(js_name = completeModuleHasElement)]
    pub fn complete_module_has_element(
        &mut self,
        token: JsValue,
        exists: bool,
    ) -> Result<(), JsValue> {
        let tok = rust_string_from_js(&token)?;
        let outcome = self.core.complete_module_has_element(&tok, Ok(exists))?;
        self.apply_resumed(outcome)
    }

    #[wasm_bindgen(js_name = rejectModuleHasElement)]
    pub fn reject_module_has_element(
        &mut self,
        token: JsValue,
        error_kind: JsValue,
        message: JsValue,
    ) -> Result<(), JsValue> {
        let tok = rust_string_from_js(&token)?;
        let kind = rust_string_from_js(&error_kind)?;
        let msg = opt_pkl_string(&message)?;
        let outcome = self
            .core
            .complete_module_has_element(&tok, Err(parse_module_host_error(&kind, msg)))?;
        self.apply_resumed(outcome)
    }

    #[wasm_bindgen(js_name = completePackageMetadata)]
    pub fn complete_package_metadata(
        &mut self,
        token: JsValue,
        bytes: JsValue,
    ) -> Result<(), JsValue> {
        let tok = rust_string_from_js(&token)?;
        let data = bytes_from_js(&bytes)?;
        let outcome = self.core.complete_package_metadata(&tok, Ok(data))?;
        self.apply_resumed(outcome)
    }

    #[wasm_bindgen(js_name = rejectPackageMetadata)]
    pub fn reject_package_metadata(
        &mut self,
        token: JsValue,
        error_kind: JsValue,
        message: JsValue,
    ) -> Result<(), JsValue> {
        let tok = rust_string_from_js(&token)?;
        let kind = rust_string_from_js(&error_kind)?;
        let msg = opt_pkl_string(&message)?;
        let outcome = self
            .core
            .complete_package_metadata(&tok, Err(parse_module_host_error(&kind, msg)))?;
        self.apply_resumed(outcome)
    }

    #[wasm_bindgen(js_name = completePackageArchive)]
    pub fn complete_package_archive(
        &mut self,
        token: JsValue,
        bytes: JsValue,
    ) -> Result<(), JsValue> {
        let tok = rust_string_from_js(&token)?;
        let data = bytes_from_js(&bytes)?;
        let outcome = self.core.complete_package_archive(&tok, Ok(data))?;
        self.apply_resumed(outcome)
    }

    #[wasm_bindgen(js_name = rejectPackageArchive)]
    pub fn reject_package_archive(
        &mut self,
        token: JsValue,
        error_kind: JsValue,
        message: JsValue,
    ) -> Result<(), JsValue> {
        let tok = rust_string_from_js(&token)?;
        let kind = rust_string_from_js(&error_kind)?;
        let msg = opt_pkl_string(&message)?;
        let outcome = self
            .core
            .complete_package_archive(&tok, Err(parse_module_host_error(&kind, msg)))?;
        self.apply_resumed(outcome)
    }

    #[wasm_bindgen(js_name = completeSecurityCheck)]
    pub fn complete_security_check(
        &mut self,
        token: JsValue,
        error_message: JsValue,
    ) -> Result<(), JsValue> {
        let tok = rust_string_from_js(&token)?;
        let err_msg = opt_pkl_string(&error_message)?;
        if let Some(state) = self.pending_glob_security.take() {
            match state {
                AnalyzerGlobSecurityState::CheckingPattern {
                    token: expected,
                    base_uri,
                    pattern_uri,
                } if expected == tok => {
                    if let Some(msg) = err_msg {
                        self.analysis.fail_glob(FailureCause::Host(
                            pkl_runtime::modules::ModuleHostError::SecurityManagerException {
                                message: Some(msg),
                            },
                        ));
                    } else {
                        self.pending_glob_emit = Some((base_uri, pattern_uri));
                    }
                    return Ok(());
                }
                AnalyzerGlobSecurityState::CheckingResults {
                    token: expected,
                    mut pending,
                    checked,
                } if expected == tok => {
                    if let Some(msg) = err_msg {
                        self.analysis.fail_glob(FailureCause::Host(
                            pkl_runtime::modules::ModuleHostError::SecurityManagerException {
                                message: Some(msg),
                            },
                        ));
                    } else if let Some(next_uri) = pending.pop_front() {
                        self.glob_check_seq += 1;
                        let next_tok = format!("analyze-glob-res:{}", self.glob_check_seq);
                        self.pending_glob_security =
                            Some(AnalyzerGlobSecurityState::CheckingResults {
                                token: next_tok.clone(),
                                pending,
                                checked,
                            });
                        self.pending_suspend = Some(WasmSuspendRequest::CheckResolveModule {
                            token: next_tok,
                            uri: next_uri,
                        });
                    } else {
                        self.analysis.complete_glob(checked);
                    }
                    return Ok(());
                }
                other => {
                    self.pending_glob_security = Some(other);
                }
            }
        }
        let outcome = self.core.complete_security_check(&tok, err_msg)?;
        self.apply_resumed(outcome)
    }

    #[wasm_bindgen(js_name = completeAnalysisGlob)]
    pub fn complete_analysis_glob(&mut self, uris: JsValue) -> Result<(), JsValue> {
        if !Array::is_array(&uris) {
            return Err(StructuredError::invalid_argument(
                "glob result must be an array of URI strings",
            )
            .into());
        }
        let mut out = Vec::new();
        for item in Array::from(&uris).iter() {
            out.push(pkl_string_from_js(&item)?);
        }
        if self.core.config.has_custom_security_check_resolve {
            let mut pending: VecDeque<PklString> = out.iter().cloned().collect();
            if let Some(first) = pending.pop_front() {
                self.glob_check_seq += 1;
                let token = format!("analyze-glob-res:{}", self.glob_check_seq);
                self.pending_glob_security = Some(AnalyzerGlobSecurityState::CheckingResults {
                    token: token.clone(),
                    pending,
                    checked: out,
                });
                self.pending_suspend =
                    Some(WasmSuspendRequest::CheckResolveModule { token, uri: first });
            } else {
                self.analysis.complete_glob(out);
            }
            return Ok(());
        }
        for uri in &out {
            if let Err(err) = self.core.policy.check_resolve_module(uri) {
                self.analysis.fail_glob(FailureCause::Host(err));
                return Ok(());
            }
        }
        self.analysis.complete_glob(out);
        Ok(())
    }

    #[wasm_bindgen(js_name = rejectAnalysisGlob)]
    pub fn reject_analysis_glob(
        &mut self,
        error_kind: JsValue,
        message: JsValue,
        capability: JsValue,
    ) -> Result<(), JsValue> {
        let kind = rust_string_from_js(&error_kind)?;
        let msg = opt_pkl_string(&message)?;
        if kind == "capability-unavailable" {
            let cap_raw = if capability.is_null() || capability.is_undefined() {
                None
            } else {
                Some(rust_string_from_js(&capability)?)
            };
            let cap: &'static str = match cap_raw.as_deref() {
                Some("nativeHttp") => "nativeHttp",
                Some("externalReaders") => "externalReaders",
                Some("modulePathJar") => "modulePathJar",
                Some("projectResolution") => "projectResolution",
                Some("packageCache") => "packageCache",
                _ => "nativeFs",
            };
            let text = msg
                .map(|m| m.to_java_utf8())
                .unwrap_or_else(|| "Required capability is unavailable in Wasm.".to_string());
            self.pending_capability_error = Some((cap, text));
            return Ok(());
        }
        self.analysis
            .fail_glob(FailureCause::Host(parse_module_host_error(&kind, msg)));
        Ok(())
    }

    pub fn close(&mut self) {
        if !self.closed {
            self.closed = true;
            self.core.close();
            self.pending_request_id = None;
            self.pending_suspend = None;
            self.pending_glob_security = None;
            self.pending_glob_emit = None;
            self.analysis.close();
        }
    }
}

impl WasmImportAnalyzer {
    fn apply_resumed(&mut self, outcome: WasmHostOutcome) -> Result<(), JsValue> {
        match outcome {
            WasmHostOutcome::CompleteModule(comp) => {
                let req_id = self.pending_request_id.take().ok_or_else(|| {
                    StructuredError::invalid_argument("no pending import analysis request")
                })?;
                self.analysis
                    .complete(req_id, comp)
                    .map_err(|msg| StructuredError::evaluation(msg).into())
            }
            WasmHostOutcome::Suspend(suspend) => {
                self.pending_suspend = Some(suspend);
                Ok(())
            }
            WasmHostOutcome::CapabilityUnavailable {
                capability,
                message,
            } => {
                self.pending_request_id = None;
                self.pending_capability_error = Some((capability, message));
                Ok(())
            }
            _ => Err(
                StructuredError::evaluation("unexpected host outcome in import analyzer").into(),
            ),
        }
    }
}

fn make_resolve_analysis_glob_event(
    base_uri: &PklString,
    pattern_uri: &PklString,
) -> Result<JsValue, JsValue> {
    let obj = Object::new();
    set_prop(&obj, "kind", JsValue::from_str("resolveAnalysisGlob"))?;
    set_prop(&obj, "token", JsValue::from_str("glob"))?;
    set_prop(&obj, "baseUri", js_string_from_pkl(base_uri).into())?;
    set_prop(&obj, "patternUri", js_string_from_pkl(pattern_uri).into())?;
    Ok(obj.into())
}

fn opt_pkl_string(val: &JsValue) -> Result<Option<PklString>, StructuredError> {
    if val.is_null() || val.is_undefined() {
        Ok(None)
    } else {
        pkl_string_from_js(val).map(Some)
    }
}

fn import_graph_to_ready_js(graph: &ImportGraph) -> Result<JsValue, JsValue> {
    let imports = Object::new();
    let resolved_imports = Object::new();
    for node in &graph.nodes {
        let uri = node.uri.to_java_utf8();
        let resolved = node.resolved_uri.to_java_utf8();
        let arr = Array::new();
        for imp in &node.imports {
            arr.push(js_string_from_pkl(imp).as_ref());
        }
        define_own_prop(&imports, &uri, arr.into())?;
        define_own_prop(&resolved_imports, &uri, JsValue::from_str(&resolved))?;
    }
    let obj = Object::new();
    set_prop(&obj, "kind", JsValue::from_str("ready"))?;
    set_prop(&obj, "imports", imports.into())?;
    set_prop(&obj, "resolvedImports", resolved_imports.into())?;
    Ok(obj.into())
}

fn format_analysis_failure(failure: &AnalysisFailure) -> StructuredError {
    match &failure.cause {
        FailureCause::Host(pkl_runtime::modules::ModuleHostError::SecurityManagerException {
            message,
        }) => StructuredError::security(message.clone().unwrap_or_else(|| {
            format!("Security error analyzing `{}`", failure.module_uri).into()
        })),
        FailureCause::Host(err) => StructuredError::evaluation(format!(
            "Failed to analyze `{}` during {}: {err:?}",
            failure.module_uri, failure.phase
        )),
        FailureCause::Syntax(diag) => StructuredError::evaluation(diag.message.clone()),
        FailureCause::Uri { input, reason } => {
            StructuredError::evaluation(format!("Invalid URI `{input}`: {reason:?}"))
        }
        FailureCause::Unsupported(msg) => StructuredError::evaluation(msg.clone()),
    }
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
