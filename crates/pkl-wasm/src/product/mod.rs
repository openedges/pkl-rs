//! Product WebAssembly bindings for Apple Pkl 0.32.0 (`@pkl-rs/wasm`).
//!
//! All evaluation, formatting, schema extraction, REPL, static import-graph analysis,
//! package verification, ZIP decompression, and `pkl-binary` decoding stay inside
//! `pkl-runtime` and `pkl-formatter` (`unsafe_code = "deny"`, zero production Java fallback).

pub mod analyzer;
pub mod decode;
pub mod evaluator;
pub mod host_core;
pub mod repl;
pub mod schema;
pub mod test_runner;
pub mod util;

use js_sys::{BigInt, JsString, Uint8Array};
use pkl_formatter::{GrammarVersion, format_str};
use util::{
    StructuredError, bytes_from_js, js_string_from_pkl, parse_i64_js, parse_u64_js,
    rust_string_from_js,
};
use wasm_bindgen::prelude::*;

/// Formats Pkl source text using `pkl-formatter` (grammar version 1 or 2, default 2).
#[wasm_bindgen(js_name = wasmFormat)]
pub fn wasm_format(source: JsValue, grammar_version: JsValue) -> Result<JsString, JsValue> {
    let text = rust_string_from_js(&source)?;
    let gv = if grammar_version.is_null() || grammar_version.is_undefined() {
        GrammarVersion::V2
    } else if let Some(n) = grammar_version.as_f64() {
        if n == 1.0 {
            GrammarVersion::V1
        } else if n == 2.0 {
            GrammarVersion::V2
        } else {
            return Err(StructuredError::invalid_argument("grammarVersion must be 1 or 2").into());
        }
    } else {
        return Err(StructuredError::invalid_argument("grammarVersion must be 1 or 2").into());
    };
    let formatted =
        format_str(&text, gv).map_err(|err| StructuredError::evaluation(err.to_string()))?;
    Ok(js_string_from_pkl(&formatted.into()))
}

/// Decodes raw `pkl-binary` MessagePack bytes into a lossless JS descriptor tree.
#[wasm_bindgen(js_name = wasmDecodePklBinary)]
pub fn wasm_decode_pkl_binary(bytes: JsValue, strict: Option<bool>) -> Result<JsValue, JsValue> {
    let raw = bytes_from_js(&bytes)?;
    decode::decode_pkl_binary_to_js(&raw, strict.unwrap_or(false)).map_err(Into::into)
}

/// Round-trips a JS descriptor tree through Rust `DecodedValue` and back to JS losslessly.
#[wasm_bindgen(js_name = wasmRoundtripValue)]
pub fn wasm_roundtrip_value(value: JsValue) -> Result<JsValue, JsValue> {
    decode::roundtrip_value_js(&value).map_err(Into::into)
}

/// Validates a 64-bit unsigned clock/timeout value (`0..=2^53-1` number or `0n..=u64::MAX` BigInt)
/// and returns it as a JS `BigInt`.
#[wasm_bindgen(js_name = wasmValidateClockMs)]
pub fn wasm_validate_clock_ms(value: JsValue) -> Result<BigInt, JsValue> {
    let ms = parse_u64_js(&value)?;
    Ok(BigInt::from(ms))
}

/// Validates a signed 64-bit `BigInt` (`i64::MIN..=i64::MAX`) and returns it losslessly.
#[wasm_bindgen(js_name = wasmValidateInt64)]
pub fn wasm_validate_int64(value: JsValue) -> Result<BigInt, JsValue> {
    let n = parse_i64_js(&value)?;
    Ok(BigInt::from(n))
}

/// Extracts `facts` group spans and 1-based line numbers from test module source text.
#[wasm_bindgen(js_name = wasmExtractFactSpans)]
pub fn wasm_extract_fact_spans(source: JsValue) -> Result<JsValue, JsValue> {
    let text = rust_string_from_js(&source)?;
    test_runner::extract_fact_spans_js(&text)
}

/// Formats a single failing fact using Pkl 0.32.0's `format_fact_failure`.
#[wasm_bindgen(js_name = wasmFormatFactFailure)]
pub fn wasm_format_fact_failure(
    expr_text: JsValue,
    location: JsValue,
    use_color: bool,
) -> Result<JsValue, JsValue> {
    let expr = rust_string_from_js(&expr_text)?;
    let loc = rust_string_from_js(&location)?;
    test_runner::format_fact_failure_js(&expr, &loc, use_color)
}

/// Evaluates rendered `examples` groups against optional expected PCF groups.
#[allow(clippy::too_many_arguments)]
#[wasm_bindgen(js_name = wasmEvaluateExamples)]
pub fn wasm_evaluate_examples(
    actual_groups: JsValue,
    expected_groups: JsValue,
    module_uri: JsValue,
    expected_uri: JsValue,
    actual_uri: JsValue,
    full_actual_doc: JsValue,
    module_source: JsValue,
    expected_source: JsValue,
    use_color: bool,
) -> Result<JsValue, JsValue> {
    let mod_uri = rust_string_from_js(&module_uri)?;
    let exp_uri = rust_string_from_js(&expected_uri)?;
    let act_uri = rust_string_from_js(&actual_uri)?;
    let full_doc = if full_actual_doc.is_null() || full_actual_doc.is_undefined() {
        None
    } else {
        Some(rust_string_from_js(&full_actual_doc)?)
    };
    let mod_src = if module_source.is_null() || module_source.is_undefined() {
        None
    } else {
        Some(rust_string_from_js(&module_source)?)
    };
    let exp_src = if expected_source.is_null() || expected_source.is_undefined() {
        None
    } else {
        Some(rust_string_from_js(&expected_source)?)
    };
    test_runner::evaluate_examples_js(
        &actual_groups,
        &expected_groups,
        &mod_uri,
        &exp_uri,
        &act_uri,
        full_doc,
        mod_src.as_deref(),
        exp_src.as_deref(),
        use_color,
    )
}

/// Renders `TestResults` with Pkl 0.32.0's `spec`, `minimal`, or `junit` reporter.
#[wasm_bindgen(js_name = wasmRenderTestReport)]
pub fn wasm_render_test_report(
    results: JsValue,
    reporter: JsValue,
    use_color: bool,
) -> Result<JsString, JsValue> {
    let rep = rust_string_from_js(&reporter)?;
    let rendered = test_runner::render_test_report_js(&results, &rep, use_color)?;
    Ok(js_string_from_pkl(&rendered.into()))
}

/// Pure-Rust SHA-256 hex digest helper (used for package checksums & verification tests).
#[wasm_bindgen(js_name = wasmSha256Hex)]
pub fn wasm_sha256_hex(bytes: Uint8Array) -> Result<String, JsValue> {
    let raw = bytes_from_js(bytes.as_ref())?;
    Ok(pkl_runtime::packages::metadata::sha256_hex(&raw))
}

#[cfg(test)]
mod tests {
    use super::host_core::{WasmHostConfig, WasmHostCore, WasmHostOutcome, WasmSuspendRequest};
    use super::util::{parse_i64_from_decimal, parse_u64_from_decimal, parse_u64_from_f64};
    use pkl_formatter::{GrammarVersion, format_str};
    use pkl_runtime::{
        evaluation::session::{SourceSession, SourceSessionOptions},
        host::HostPoll,
        modules::{
            ModuleCompletion, ModuleKey, ModuleKeyId, ModuleKeyKind, ModuleRequest,
            ModuleRequestKind, ModuleSourceSpec, ResolutionPhase, ResolvedKeyId,
        },
        packages::{PackageUri, metadata::sha256_hex, zip::ZipArchive},
        pkl_binary_decode::{DecodedValue, decode_lossless},
        repl::{ReplPoll, ReplRequest, ReplResponse, ReplSession, ReplSessionOptions},
        schema::{SchemaExtractor, SchemaStep},
        value::PklString,
    };
    use std::collections::BTreeMap;

    fn crc32(data: &[u8]) -> u32 {
        let mut crc: u32 = 0xffff_ffff;
        for &byte in data {
            crc ^= u32::from(byte);
            for _ in 0..8 {
                if crc & 1 != 0 {
                    crc = (crc >> 1) ^ 0xedb8_8320;
                } else {
                    crc >>= 1;
                }
            }
        }
        !crc
    }

    fn build_stored_zip(files: &BTreeMap<String, Vec<u8>>) -> Vec<u8> {
        let mut out = Vec::new();
        let mut central = Vec::new();
        for (name, data) in files {
            let offset = out.len() as u32;
            let crc = crc32(data);
            let size = data.len() as u32;
            let name_bytes = name.as_bytes();
            let name_len = name_bytes.len() as u16;
            // Local file header
            out.extend_from_slice(&0x0403_4b50_u32.to_le_bytes());
            out.extend_from_slice(&20_u16.to_le_bytes());
            out.extend_from_slice(&0_u16.to_le_bytes());
            out.extend_from_slice(&0_u16.to_le_bytes()); // stored
            out.extend_from_slice(&0_u16.to_le_bytes());
            out.extend_from_slice(&0_u16.to_le_bytes());
            out.extend_from_slice(&crc.to_le_bytes());
            out.extend_from_slice(&size.to_le_bytes());
            out.extend_from_slice(&size.to_le_bytes());
            out.extend_from_slice(&name_len.to_le_bytes());
            out.extend_from_slice(&0_u16.to_le_bytes());
            out.extend_from_slice(name_bytes);
            out.extend_from_slice(data);
            // Central directory header
            central.extend_from_slice(&0x0201_4b50_u32.to_le_bytes());
            central.extend_from_slice(&20_u16.to_le_bytes());
            central.extend_from_slice(&20_u16.to_le_bytes());
            central.extend_from_slice(&0_u16.to_le_bytes());
            central.extend_from_slice(&0_u16.to_le_bytes());
            central.extend_from_slice(&0_u16.to_le_bytes());
            central.extend_from_slice(&0_u16.to_le_bytes());
            central.extend_from_slice(&crc.to_le_bytes());
            central.extend_from_slice(&size.to_le_bytes());
            central.extend_from_slice(&size.to_le_bytes());
            central.extend_from_slice(&name_len.to_le_bytes());
            central.extend_from_slice(&0_u16.to_le_bytes());
            central.extend_from_slice(&0_u16.to_le_bytes());
            central.extend_from_slice(&0_u16.to_le_bytes());
            central.extend_from_slice(&0_u16.to_le_bytes());
            central.extend_from_slice(&0_u32.to_le_bytes());
            central.extend_from_slice(&offset.to_le_bytes());
            central.extend_from_slice(name_bytes);
        }
        let cd_offset = out.len() as u32;
        let cd_size = central.len() as u32;
        let count = files.len() as u16;
        out.extend_from_slice(&central);
        // EOCD
        out.extend_from_slice(&0x0605_4b50_u32.to_le_bytes());
        out.extend_from_slice(&0_u16.to_le_bytes());
        out.extend_from_slice(&0_u16.to_le_bytes());
        out.extend_from_slice(&count.to_le_bytes());
        out.extend_from_slice(&count.to_le_bytes());
        out.extend_from_slice(&cd_size.to_le_bytes());
        out.extend_from_slice(&cd_offset.to_le_bytes());
        out.extend_from_slice(&0_u16.to_le_bytes());
        out
    }

    #[test]
    fn clock_u64_accepts_above_u32_max_and_rejects_invalid() {
        // Beyond M1's 49.7-day u32::MAX limit (4_294_967_295 ms)
        assert_eq!(parse_u64_from_f64(5_000_000_000.0).unwrap(), 5_000_000_000);
        assert_eq!(
            parse_u64_from_f64(9_007_199_254_740_991.0).unwrap(),
            9_007_199_254_740_991
        );
        assert_eq!(
            parse_u64_from_decimal("18446744073709551615").unwrap(),
            u64::MAX
        );
        assert!(parse_u64_from_f64(-1.0).is_err());
        assert!(parse_u64_from_f64(f64::NAN).is_err());
        assert!(parse_u64_from_f64(f64::INFINITY).is_err());
        assert!(parse_u64_from_f64(1.5).is_err());
        assert!(parse_u64_from_f64(9_007_199_254_740_992.0).is_err());
        assert!(parse_u64_from_decimal("-1").is_err());
        assert!(parse_u64_from_decimal("18446744073709551616").is_err());
    }

    #[test]
    fn int64_decimal_validation_exact_bounds() {
        assert_eq!(
            parse_i64_from_decimal("-9223372036854775808").unwrap(),
            i64::MIN
        );
        assert_eq!(
            parse_i64_from_decimal("9223372036854775807").unwrap(),
            i64::MAX
        );
        assert!(parse_i64_from_decimal("-9223372036854775809").is_err());
        assert!(parse_i64_from_decimal("9223372036854775808").is_err());
    }

    #[test]
    fn formatter_supports_v1_and_v2() {
        let src = "x=1+2\n";
        assert_eq!(format_str(src, GrammarVersion::V2).unwrap(), "x = 1 + 2\n");
        assert_eq!(format_str(src, GrammarVersion::V1).unwrap(), "x = 1 + 2\n");
    }

    #[test]
    fn host_core_reports_explicit_capability_unavailable_without_vfs_or_http() {
        let mut core = WasmHostCore::new(WasmHostConfig::default()).unwrap();
        let file_req = ModuleRequest {
            id: 1,
            kind: ModuleRequestKind::ResolveKey {
                key: ModuleKey {
                    id: ModuleKeyId(1),
                    uri: "file:///sandbox/a.pkl".into(),
                    kind: ModuleKeyKind::StandardFile,
                    handle: "".into(),
                },
                resolved_key_id: ResolvedKeyId(1),
                phase: ResolutionPhase::Primary,
            },
        };
        match core.handle_module("t1".into(), file_req) {
            WasmHostOutcome::CapabilityUnavailable { capability, .. } => {
                assert_eq!(capability, "nativeFs");
            }
            _ => panic!("expected capability-unavailable for file: without VFS"),
        }

        let https_req = ModuleRequest {
            id: 2,
            kind: ModuleRequestKind::ResolveKey {
                key: ModuleKey {
                    id: ModuleKeyId(2),
                    uri: "https://example.invalid/a.pkl".into(),
                    kind: ModuleKeyKind::StandardHttp,
                    handle: "".into(),
                },
                resolved_key_id: ResolvedKeyId(2),
                phase: ResolutionPhase::Primary,
            },
        };
        match core.handle_module("t2".into(), https_req) {
            WasmHostOutcome::CapabilityUnavailable { capability, .. } => {
                assert_eq!(capability, "nativeHttp");
            }
            _ => panic!("expected capability-unavailable for https: without httpFetch"),
        }
    }

    #[test]
    fn host_core_enforces_security_policy_before_capability_check() {
        let mut core = WasmHostCore::new(WasmHostConfig {
            root_dir: Some("/sandbox".into()),
            ..Default::default()
        })
        .unwrap();

        // http: is denied by default allowedModules BEFORE nativeHttp capability check
        let http_req = ModuleRequest {
            id: 1,
            kind: ModuleRequestKind::ResolveKey {
                key: ModuleKey {
                    id: ModuleKeyId(1),
                    uri: "http://example.invalid/a.pkl".into(),
                    kind: ModuleKeyKind::StandardHttp,
                    handle: "".into(),
                },
                resolved_key_id: ResolvedKeyId(1),
                phase: ResolutionPhase::Primary,
            },
        };
        match core.handle_module("t1".into(), http_req) {
            WasmHostOutcome::CompleteModule(ModuleCompletion::KeyResolved(Err(
                pkl_runtime::modules::ModuleHostError::SecurityManagerException { .. },
            ))) => {}
            _ => panic!("expected SecurityManagerException for disallowed http: scheme"),
        }
        assert!(core.saw_security_denial);

        // file: outside rootDir is denied BEFORE nativeFs capability check
        let escaped_req = ModuleRequest {
            id: 2,
            kind: ModuleRequestKind::ResolveKey {
                key: ModuleKey {
                    id: ModuleKeyId(2),
                    uri: "file:///etc/passwd".into(),
                    kind: ModuleKeyKind::StandardFile,
                    handle: "".into(),
                },
                resolved_key_id: ResolvedKeyId(2),
                phase: ResolutionPhase::Primary,
            },
        };
        match core.handle_module("t2".into(), escaped_req) {
            WasmHostOutcome::CompleteModule(ModuleCompletion::KeyResolved(Err(
                pkl_runtime::modules::ModuleHostError::SecurityManagerException { .. },
            ))) => {}
            _ => panic!("expected SecurityManagerException for path outside rootDir"),
        }
    }

    #[test]
    fn host_core_drives_source_session_with_64bit_clock_and_output_format() {
        let mut core = WasmHostCore::new(WasmHostConfig {
            output_format: Some("json".into()),
            properties: vec![("greeting".into(), "hello".into())],
            ..Default::default()
        })
        .unwrap();
        let mut session = SourceSession::module_evaluator(SourceSessionOptions::default());
        let big_clock: u64 = 5_000_000_000; // > u32::MAX (57.8 days)
        session
            .start_evaluation(
                ModuleSourceSpec {
                    uri: "repl:text".into(),
                    contents: Some(
                        "msg = read(\"prop:greeting\")\nn = 9223372036854775807\n".into(),
                    ),
                },
                "output.text",
                big_clock,
                Some(60_000),
            )
            .unwrap();
        let value = loop {
            match session.poll(big_clock + 5, 4096) {
                HostPoll::Ready(res) => break res.unwrap(),
                HostPoll::Yielded => {}
                HostPoll::Module { token, request } => {
                    let WasmHostOutcome::CompleteModule(comp) =
                        core.handle_module(token.clone(), request)
                    else {
                        panic!("expected local module completion");
                    };
                    session
                        .complete_module(&token, comp, big_clock + 5)
                        .unwrap();
                }
                HostPoll::Read { token, .. } => {
                    let req = session.read_request(&token).unwrap();
                    let WasmHostOutcome::CompleteRead(res) = core.handle_read(token.clone(), &req)
                    else {
                        panic!("expected local read completion");
                    };
                    session.complete_read(&token, res, big_clock + 5).unwrap();
                }
                HostPoll::Glob { token, request } => {
                    let WasmHostOutcome::CompleteGlob(comp) =
                        core.handle_glob(token.clone(), request)
                    else {
                        panic!("expected local glob completion");
                    };
                    session.complete_glob(&token, comp, big_clock + 5).unwrap();
                }
                HostPoll::Log { token, .. } => {
                    session
                        .acknowledge_log(&token, Ok(()), big_clock + 5)
                        .unwrap();
                }
                HostPoll::TransformFrame { token, frame } => {
                    session
                        .complete_frame_transform(&token, Ok((*frame).clone()), big_clock + 5)
                        .unwrap();
                }
            }
        };
        let text = SourceSession::result_string(value).unwrap().to_java_utf8();
        assert_eq!(
            text,
            "{\n  \"msg\": \"hello\",\n  \"n\": 9223372036854775807\n}\n"
        );
        session.close();
    }

    #[test]
    fn host_core_async_package_metadata_and_zip_verification() {
        let mut files = BTreeMap::new();
        files.insert("lib.pkl".to_string(), b"answer = 42\n".to_vec());
        let zip_bytes = build_stored_zip(&files);
        let zip_sha = sha256_hex(&zip_bytes);
        let metadata_json = format!(
            r#"{{
  "name": "demo",
  "packageUri": "package://example.com/demo@1.0.0",
  "version": "1.0.0",
  "packageZipUrl": "https://example.com/demo@1.0.0.zip",
  "packageZipChecksums": {{ "sha256": "{zip_sha}" }},
  "dependencies": {{}}
}}"#
        );
        let meta_bytes = metadata_json.as_bytes().to_vec();
        let meta_sha = sha256_hex(&meta_bytes);
        let pkg_uri_str = format!("package://example.com/demo@1.0.0::sha256:{meta_sha}#/lib.pkl");

        // Sanity check pure-Rust ZipArchive parses it
        let parsed = ZipArchive::parse(&zip_bytes).unwrap();
        assert_eq!(
            parsed.read_bytes("/lib.pkl", false).unwrap(),
            b"answer = 42\n"
        );

        let mut core = WasmHostCore::new(WasmHostConfig {
            has_package_transport: true,
            ..Default::default()
        })
        .unwrap();

        let req = ModuleRequest {
            id: 1,
            kind: ModuleRequestKind::ResolveKey {
                key: ModuleKey {
                    id: ModuleKeyId(1),
                    uri: PklString::from(pkg_uri_str.as_str()),
                    kind: ModuleKeyKind::StandardGenericUrl,
                    handle: "".into(),
                },
                resolved_key_id: ResolvedKeyId(1),
                phase: ResolutionPhase::Primary,
            },
        };
        // Step 1: suspends for packageMetadata
        match core.handle_module("pkg-tok".into(), req) {
            WasmHostOutcome::Suspend(WasmSuspendRequest::PackageMetadata {
                request_url, ..
            }) => {
                assert_eq!(request_url.to_java_utf8(), "https://example.com/demo@1.0.0");
            }
            _ => panic!("expected PackageMetadata suspend"),
        }
        // Step 2: provide metadata -> suspends for packageArchive
        match core
            .complete_package_metadata("pkg-tok", Ok(meta_bytes))
            .unwrap()
        {
            WasmHostOutcome::Suspend(WasmSuspendRequest::PackageArchive { zip_url, .. }) => {
                assert_eq!(zip_url.to_java_utf8(), "https://example.com/demo@1.0.0.zip");
            }
            _ => panic!("expected PackageArchive suspend"),
        }
        // Step 3: provide ZIP -> completes module resolution and stages source!
        match core
            .complete_package_archive("pkg-tok", Ok(zip_bytes))
            .unwrap()
        {
            WasmHostOutcome::CompleteModule(ModuleCompletion::KeyResolved(Ok(rk))) => {
                assert_eq!(rk.module.original_uri.to_java_utf8(), pkg_uri_str);
                let staged = core.staged_sources.get(&rk.module.resolved_uri).unwrap();
                assert_eq!(staged.text.original().to_java_utf8(), "answer = 42\n");
            }
            _ => panic!("expected KeyResolved after package archive"),
        }

        // Verify PackageUri parse works cleanly
        assert!(PackageUri::parse("package://example.com/demo@1.0.0").is_ok());
    }

    #[test]
    fn repl_and_schema_and_pkl_binary_work_pure_rust() {
        let mut core = WasmHostCore::new(WasmHostConfig::default()).unwrap();
        // Schema extraction
        let mut session = SourceSession::module_evaluator(SourceSessionOptions::default());
        session
            .start_load_module(
                ModuleSourceSpec {
                    uri: "repl:text".into(),
                    contents: Some(
                        "class Person { name: String }\np: Person = new { name = \"Ada\" }\n"
                            .into(),
                    ),
                },
                0,
                None,
            )
            .unwrap();
        let drive = |session: &mut SourceSession, core: &mut WasmHostCore| loop {
            match session.poll(0, 4096) {
                HostPoll::Ready(res) => break res.unwrap(),
                HostPoll::Yielded => {}
                HostPoll::Module { token, request } => {
                    let WasmHostOutcome::CompleteModule(c) =
                        core.handle_module(token.clone(), request)
                    else {
                        panic!("local module");
                    };
                    session.complete_module(&token, c, 0).unwrap();
                }
                HostPoll::Read { token, .. } => {
                    let req = session.read_request(&token).unwrap();
                    let WasmHostOutcome::CompleteRead(r) = core.handle_read(token.clone(), &req)
                    else {
                        panic!("local read");
                    };
                    session.complete_read(&token, r, 0).unwrap();
                }
                HostPoll::Glob { token, request } => {
                    let WasmHostOutcome::CompleteGlob(c) = core.handle_glob(token.clone(), request)
                    else {
                        panic!("local glob");
                    };
                    session.complete_glob(&token, c, 0).unwrap();
                }
                HostPoll::Log { token, .. } => session.acknowledge_log(&token, Ok(()), 0).unwrap(),
                HostPoll::TransformFrame { token, frame } => session
                    .complete_frame_transform(&token, Ok((*frame).clone()), 0)
                    .unwrap(),
            }
        };
        let module_val = drive(&mut session, &mut core);
        let mut ext = SchemaExtractor::new(module_val).unwrap();
        let mut last = None;
        let schema = loop {
            match ext.step(&mut session, last, 0, None).unwrap() {
                SchemaStep::RunPending => last = Some(drive(&mut session, &mut core)),
                SchemaStep::Done(s) => break s,
            }
        };
        assert!(schema.class_by_name("Person").is_some());

        // pkl-binary encode + decode lossless
        session
            .start_evaluation_pkl_binary(
                ModuleSourceSpec {
                    uri: "repl:text".into(),
                    contents: Some("x = 42\n".into()),
                },
                "x",
                0,
                None,
            )
            .unwrap();
        let bytes = SourceSession::result_bytes(drive(&mut session, &mut core)).unwrap();
        assert_eq!(decode_lossless(&bytes).unwrap(), DecodedValue::Int(42));
        session.close();

        // REPL session
        let mut repl = ReplSession::new(ReplSessionOptions::default());
        repl.start_request(
            ReplRequest::Eval {
                id: "1".into(),
                text: "a = 21 * 2".into(),
                eval_definitions: true,
                force_results: false,
            },
            0,
            None,
        )
        .unwrap();
        let responses = loop {
            match repl.poll(0, 4096) {
                ReplPoll::Ready(r) => break r,
                ReplPoll::Yielded => {}
                ReplPoll::Module { token, request } => {
                    let WasmHostOutcome::CompleteModule(c) =
                        core.handle_module(token.clone(), request)
                    else {
                        panic!("local module");
                    };
                    repl.complete_module(&token, c, 0).unwrap();
                }
                _ => panic!("unexpected repl poll"),
            }
        };
        assert_eq!(
            responses,
            vec![ReplResponse::EvalSuccess {
                result: "42".into()
            }]
        );
        repl.close();
    }

    #[test]
    fn import_graph_analysis_resolves_builtin_stdlib_modules() {
        use pkl_runtime::modules::analyze::{AnalysisPoll, ImportGraphAnalysis};

        let mut core = WasmHostCore::new(WasmHostConfig::default()).unwrap();
        let mut analysis = ImportGraphAnalysis::new(["pkl:math".into(), "pkl:release".into()]);
        let graph = loop {
            match analysis.poll() {
                AnalysisPoll::Ready(Ok(graph)) => break graph,
                AnalysisPoll::Ready(Err(err)) => {
                    panic!("unexpected analysis failure: {err:?}");
                }
                AnalysisPoll::Closed => panic!("unexpected analysis closed"),
                AnalysisPoll::Request(req) => {
                    let req_id = req.id;
                    let WasmHostOutcome::CompleteModule(comp) =
                        core.handle_module(format!("t:{req_id}"), *req)
                    else {
                        panic!("expected local stdlib completion");
                    };
                    analysis.complete(req_id, comp).unwrap();
                }
            }
        };
        let math_node = graph.nodes.iter().find(|n| n.uri == "pkl:math").unwrap();
        assert!(math_node.imports.is_empty());
        let rel_node = graph.nodes.iter().find(|n| n.uri == "pkl:release").unwrap();
        assert_eq!(rel_node.imports, vec!["pkl:semver"]);
        let semver_node = graph.nodes.iter().find(|n| n.uri == "pkl:semver").unwrap();
        assert!(semver_node.imports.is_empty());
    }
}
