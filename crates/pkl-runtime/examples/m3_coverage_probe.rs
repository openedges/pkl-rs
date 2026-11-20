//! Mechanical M3 standard-library, external-builtin, and renderer runtime census probe.
//!
//! Emits deterministic JSON reporting:
//! 1. Byte-for-byte SHA-256 parity of vendored `crates/pkl-runtime/src/stdlib/*.pkl`
//!    against `external/pkl/stdlib/*.pkl` (all 24 upstream files, marking the 23
//!    `pkl:*` modules vs non-module `doc-package-info.pkl`).
//! 2. Live load and `pkl:reflect` declaration census for all 23 `pkl:*` modules.
//! 3. Live probe classification for every external builtin and renderer entry
//!    supplied in the TSV probe plan generated from `compatibility/inventory.json`.

#![forbid(unsafe_code)]

use pkl_runtime::glob::configured_or_unsupported_completion;
use pkl_runtime::logging::PlatformMetadata;
use pkl_runtime::modules::{
    ModuleCompletion, ModuleKey, ModuleKeyKind, ModuleRequestKind, ModuleSource, ResolvedModule,
    ResolvedModuleKey, default_completion, default_or_unsupported_completion,
};
use pkl_runtime::probe::SourceProbe;
use pkl_runtime::value::PklString;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

const MAX_POLL_STEPS: usize = 250_000;
const POLL_BUDGET: u32 = 100_000;
const LEAF_URI: &str = "file:///m3-coverage-probe/leaf.pkl";
const LEAF_SOURCE: &str = "x = 1\n";

#[derive(Debug)]
struct PlanEntry {
    category: String,
    id: String,
    symbol: String,
    kind: String,
    source_code: String,
    expression: String,
    output_format: Option<String>,
}

#[derive(Debug)]
struct EvalOutcome {
    ok: bool,
    value: Option<String>,
    disposition: Option<String>,
    error: Option<String>,
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut out = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(out, "{byte:02x}");
    }
    out
}

fn json_escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0C}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                use std::fmt::Write as _;
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn json_opt_str(value: Option<&str>) -> String {
    match value {
        Some(v) => json_escape(v),
        None => "null".to_string(),
    }
}

fn decode_plan_field(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('r') => out.push('\r'),
                Some('\\') => out.push('\\'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(ch);
        }
    }
    out
}

fn parse_plan(path: &Path) -> Result<Vec<PlanEntry>, String> {
    let text =
        fs::read_to_string(path).map_err(|e| format!("read plan {}: {e}", path.display()))?;
    let mut entries = Vec::new();
    for (idx, line) in text.lines().enumerate() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() != 7 {
            return Err(format!(
                "invalid plan line {}: expected 7 tab-separated columns, got {}",
                idx + 1,
                parts.len()
            ));
        }
        let fmt = decode_plan_field(parts[6]);
        entries.push(PlanEntry {
            category: parts[0].to_string(),
            id: parts[1].to_string(),
            symbol: parts[2].to_string(),
            kind: parts[3].to_string(),
            source_code: decode_plan_field(parts[4]),
            expression: decode_plan_field(parts[5]),
            output_format: if fmt.is_empty() { None } else { Some(fmt) },
        });
    }
    Ok(entries)
}

fn new_probe() -> Result<SourceProbe, EvalOutcome> {
    let mut probe =
        SourceProbe::new_module_evaluator("compact", false, false, false).map_err(|err| {
            EvalOutcome {
                ok: false,
                value: None,
                disposition: Some("setup-error".to_string()),
                error: Some(err.to_java_utf8()),
            }
        })?;
    probe.set_platform_metadata(Some(PlatformMetadata::default()));
    Ok(probe)
}

fn eval_on_probe(
    probe: &mut SourceProbe,
    started: Instant,
    primary_uri: &str,
    source: &str,
    expression: &str,
    output_format: Option<&str>,
) -> EvalOutcome {
    let start_ms = started.elapsed().as_millis().min(u32::MAX as u128) as u32;
    if let Err(err) =
        probe.evaluate_module_source(primary_uri, Some(source), expression, start_ms, None)
    {
        return EvalOutcome {
            ok: false,
            value: None,
            disposition: Some("setup-error".to_string()),
            error: Some(err.to_string()),
        };
    }

    let mut outcome = None;
    for _ in 0..MAX_POLL_STEPS {
        let now_ms = started.elapsed().as_millis().min(u32::MAX as u128) as u32;
        let event = match probe.poll_text(now_ms, POLL_BUDGET) {
            Ok(ev) => ev,
            Err(err) => {
                outcome = Some(EvalOutcome {
                    ok: false,
                    value: None,
                    disposition: Some("internal-error".to_string()),
                    error: Some(format!("{err:?}")),
                });
                break;
            }
        };
        match event.kind {
            "yield" => {}
            "module-request" => {
                let Some(token) = event.token.as_deref() else {
                    outcome = Some(EvalOutcome {
                        ok: false,
                        value: None,
                        disposition: Some("internal-error".to_string()),
                        error: Some("missing module token".to_string()),
                    });
                    break;
                };
                let request = match probe.module_request(token) {
                    Ok(req) => req,
                    Err(err) => {
                        outcome = Some(EvalOutcome {
                            ok: false,
                            value: None,
                            disposition: Some("internal-error".to_string()),
                            error: Some(format!("{err:?}")),
                        });
                        break;
                    }
                };
                let completion = match &request.kind {
                    ModuleRequestKind::SelectKey {
                        id, uri: req_uri, ..
                    } if req_uri.as_str() == Some(primary_uri)
                        || req_uri.as_str() == Some(LEAF_URI) =>
                    {
                        ModuleCompletion::KeySelected(Ok(ModuleKey {
                            id: *id,
                            uri: req_uri.clone(),
                            kind: ModuleKeyKind::Host,
                            handle: format!("m3-probe:{}", id.0).into(),
                        }))
                    }
                    ModuleRequestKind::ResolveKey {
                        key,
                        resolved_key_id,
                        ..
                    } if key.uri.as_str() == Some(primary_uri)
                        || key.uri.as_str() == Some(LEAF_URI) =>
                    {
                        ModuleCompletion::KeyResolved(Ok(ResolvedModuleKey {
                            id: *resolved_key_id,
                            key: key.clone(),
                            handle: format!("m3-probe-resolved:{}", resolved_key_id.0).into(),
                            module: ResolvedModule {
                                original_uri: key.uri.clone(),
                                resolved_uri: key.uri.clone(),
                                source_uri: key.uri.clone(),
                            },
                        }))
                    }
                    ModuleRequestKind::LoadSource { key, .. }
                        if key.key.uri.as_str() == Some(primary_uri) =>
                    {
                        ModuleCompletion::Source(Ok(ModuleSource {
                            text: source.into(),
                            inferred_name: "probe".into(),
                        }))
                    }
                    ModuleRequestKind::LoadSource { key, .. }
                        if key.key.uri.as_str() == Some(LEAF_URI) =>
                    {
                        ModuleCompletion::Source(Ok(ModuleSource {
                            text: LEAF_SOURCE.into(),
                            inferred_name: "leaf".into(),
                        }))
                    }
                    _ => default_completion(&request)
                        .unwrap_or_else(|| default_or_unsupported_completion(&request)),
                };
                if let Err(err) = probe.complete_module(token, completion, now_ms) {
                    outcome = Some(EvalOutcome {
                        ok: false,
                        value: None,
                        disposition: Some("internal-error".to_string()),
                        error: Some(format!("{err:?}")),
                    });
                    break;
                }
            }
            "glob-request" => {
                let Some(token) = event.token.as_deref() else {
                    outcome = Some(EvalOutcome {
                        ok: false,
                        value: None,
                        disposition: Some("internal-error".to_string()),
                        error: Some("missing glob token".to_string()),
                    });
                    break;
                };
                let request = match probe.glob_request(token) {
                    Ok(req) => req,
                    Err(err) => {
                        outcome = Some(EvalOutcome {
                            ok: false,
                            value: None,
                            disposition: Some("internal-error".to_string()),
                            error: Some(format!("{err:?}")),
                        });
                        break;
                    }
                };
                let completion = configured_or_unsupported_completion(&request, &[], &[]);
                if let Err(err) = probe.complete_glob(token, completion, now_ms) {
                    outcome = Some(EvalOutcome {
                        ok: false,
                        value: None,
                        disposition: Some("internal-error".to_string()),
                        error: Some(format!("{err:?}")),
                    });
                    break;
                }
            }
            "read" => {
                let Some(token) = event.token.as_deref() else {
                    outcome = Some(EvalOutcome {
                        ok: false,
                        value: None,
                        disposition: Some("internal-error".to_string()),
                        error: Some("missing read token".to_string()),
                    });
                    break;
                };
                let uri_str = event.uri.as_ref().and_then(PklString::as_str);
                let res = if uri_str == Some("prop:pkl.outputFormat") {
                    match output_format {
                        Some(fmt) => probe.complete_read_text(token, PklString::from(fmt), now_ms),
                        None => probe.complete_read_missing(token, now_ms),
                    }
                } else {
                    probe.complete_read_missing(token, now_ms)
                };
                if let Err(err) = res {
                    outcome = Some(EvalOutcome {
                        ok: false,
                        value: None,
                        disposition: Some("internal-error".to_string()),
                        error: Some(format!("{err:?}")),
                    });
                    break;
                }
            }
            "log" => {
                if let Some(token) = event.token.as_deref() {
                    let _ = probe.acknowledge_log(token, None, now_ms);
                }
            }
            "transform-frame" => {
                if let (Some(token), Some(frame)) = (event.token.as_deref(), event.frame) {
                    let _ = probe.complete_frame_transform(token, Some(frame), None, now_ms);
                }
            }
            "value" => {
                let text = event
                    .value
                    .map(PklString::from_utf16)
                    .map(|s| s.to_java_utf8())
                    .unwrap_or_default();
                outcome = Some(EvalOutcome {
                    ok: true,
                    value: Some(text),
                    disposition: None,
                    error: None,
                });
                break;
            }
            "error" => {
                let text = event
                    .error
                    .map(PklString::from_utf16)
                    .map(|s| s.to_java_utf8())
                    .unwrap_or_default();
                outcome = Some(EvalOutcome {
                    ok: false,
                    value: None,
                    disposition: event.disposition.map(str::to_string),
                    error: Some(text),
                });
                break;
            }
            other => {
                outcome = Some(EvalOutcome {
                    ok: false,
                    value: None,
                    disposition: Some("unexpected-event".to_string()),
                    error: Some(format!("unexpected Host event: {other}")),
                });
                break;
            }
        }
    }
    outcome.unwrap_or_else(|| EvalOutcome {
        ok: false,
        value: None,
        disposition: Some("limit-exceeded".to_string()),
        error: Some("probe poll budget exceeded".to_string()),
    })
}

fn eval_pkl_fresh(
    primary_uri: &str,
    source: &str,
    expression: &str,
    output_format: Option<&str>,
) -> EvalOutcome {
    let mut probe = match new_probe() {
        Ok(p) => p,
        Err(err) => return err,
    };
    let outcome = eval_on_probe(
        &mut probe,
        Instant::now(),
        primary_uri,
        source,
        expression,
        output_format,
    );
    probe.close();
    outcome
}

fn classify_builtin(outcome: &EvalOutcome) -> (&'static str, &'static str) {
    if outcome.ok {
        return ("implemented", "evaluated_ok");
    }
    let disp = outcome.disposition.as_deref().unwrap_or("");
    let err = outcome.error.as_deref().unwrap_or("");
    if disp == "unsupported"
        || err.contains("is not implemented")
        || err.contains("not implemented")
    {
        return ("unsupported", "unsupported");
    }
    if err.contains("Cannot find standard library module") {
        return ("missing", "missing_module");
    }
    if err.contains("Cannot find property `") || err.contains("Cannot find method `") {
        return ("missing", "missing_member");
    }
    if disp == "language-error" {
        return ("implemented", "language_error_reached");
    }
    ("probe_error", "probe_error")
}

fn classify_renderer(outcome: &EvalOutcome) -> &'static str {
    if outcome.ok {
        return "implemented";
    }
    let disp = outcome.disposition.as_deref().unwrap_or("");
    let err = outcome.error.as_deref().unwrap_or("");
    if disp == "unsupported"
        || err.contains("is not implemented")
        || err.contains("not implemented")
    {
        return "unsupported";
    }
    if err.contains("Cannot find ") || err.contains("Unknown output format") {
        return "missing";
    }
    "probe_error"
}

fn truncate_detail(text: &str) -> String {
    let first_lines: Vec<&str> = text.lines().take(4).collect();
    let joined = first_lines.join(" | ");
    if joined.len() > 280 {
        let mut end = 280;
        while !joined.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}...", &joined[..end])
    } else {
        joined
    }
}

fn reflect_module_json(probe: &mut SourceProbe, started: Instant, seq: usize, uri: &str) -> String {
    let source = format!(
        r#"import "pkl:reflect"
local m = reflect.Module(import("{uri}"))
local payload = new Dynamic {{
  ["module_name"] = m.name
  ["uri"] = m.uri
  ["properties"] = m.moduleClass.properties.keys.toList()
  ["methods"] = m.moduleClass.methods.keys.toList()
  ["type_aliases"] = m.typeAliases.keys.toList()
  ["classes"] = new Dynamic {{
    for (cname, cls in m.classes) {{
      [cname] = new Dynamic {{
        ["properties"] = cls.properties.keys.toList()
        ["methods"] = cls.methods.keys.toList()
      }}
    }}
  }}
}}
output {{
  text = new JsonRenderer {{ indent = "" }}.renderValue(payload)
}}
"#
    );
    let primary_uri = format!("file:///m3-coverage-probe/reflect-{seq}.pkl");
    let outcome = eval_on_probe(probe, started, &primary_uri, &source, "output.text", None);
    probe.collect();
    if outcome.ok {
        let reflected = outcome.value.unwrap_or_else(|| "{}".to_string());
        format!(
            "{{\"status\":\"loaded\",\"disposition\":null,\"error\":null,\"reflected\":{}}}",
            reflected.trim()
        )
    } else {
        let disp = outcome.disposition.as_deref();
        let err = outcome.error.as_deref().unwrap_or("");
        let status = if disp == Some("unsupported") || err.contains("is not implemented") {
            "unsupported"
        } else {
            "error"
        };
        format!(
            "{{\"status\":{},\"disposition\":{},\"error\":{},\"reflected\":null}}",
            json_escape(status),
            json_opt_str(disp),
            json_escape(&truncate_detail(err))
        )
    }
}

fn run(root: &Path, plan_path: Option<&Path>, output_path: Option<&Path>) -> Result<(), String> {
    let upstream_dir = root.join("external/pkl/stdlib");
    let vendored_dir = root.join("crates/pkl-runtime/src/stdlib");
    let modules_txt = fs::read_to_string(vendored_dir.join("modules.txt"))
        .map_err(|e| format!("read modules.txt: {e}"))?;
    let module_uris: Vec<String> = modules_txt
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect();
    let module_stems: BTreeSet<String> = module_uris
        .iter()
        .filter_map(|u| u.strip_prefix("pkl:").map(str::to_string))
        .collect();

    // 1. Vendored stdlib byte-parity census across all upstream stdlib/*.pkl files
    let mut upstream_files: Vec<String> = fs::read_dir(&upstream_dir)
        .map_err(|e| format!("read {}: {e}", upstream_dir.display()))?
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            if path.extension().and_then(|s| s.to_str()) == Some("pkl") {
                path.file_name()?.to_str().map(str::to_string)
            } else {
                None
            }
        })
        .collect();
    upstream_files.sort();

    let mut vendored_rows = Vec::new();
    for filename in &upstream_files {
        let stem = filename.strip_suffix(".pkl").unwrap_or(filename);
        let is_module = module_stems.contains(stem);
        let module_uri = is_module.then(|| format!("pkl:{stem}"));
        let up_bytes = fs::read(upstream_dir.join(filename))
            .map_err(|e| format!("read upstream {filename}: {e}"))?;
        let up_sha = sha256_hex(&up_bytes);
        let vend_path = vendored_dir.join(filename);
        let (vend_sha, status) = if vend_path.is_file() {
            let vb = fs::read(&vend_path).map_err(|e| format!("read vendored {filename}: {e}"))?;
            let vs = sha256_hex(&vb);
            let st = if vb == up_bytes {
                "exact_match"
            } else {
                "byte_mismatch"
            };
            (Some(vs), st)
        } else {
            (None, "missing_vendored")
        };
        vendored_rows.push(format!(
            "{{\"filename\":{},\"is_module\":{},\"module_uri\":{},\"status\":{},\"upstream_sha256\":{},\"vendored_sha256\":{}}}",
            json_escape(filename),
            if is_module { "true" } else { "false" },
            json_opt_str(module_uri.as_deref()),
            json_escape(status),
            json_escape(&up_sha),
            json_opt_str(vend_sha.as_deref()),
        ));
    }

    // 2. Live load + reflection census for all 23 pkl:* modules (reusing one evaluator so stdlib parses once)
    let mut shared_probe = new_probe()
        .map_err(|err| format!("create shared probe: {}", err.error.unwrap_or_default()))?;
    let shared_started = Instant::now();
    let mut sorted_uris = module_uris.clone();
    sorted_uris.sort();
    let mut module_entries = Vec::new();
    for (idx, uri) in sorted_uris.iter().enumerate() {
        let body = reflect_module_json(&mut shared_probe, shared_started, idx, uri);
        module_entries.push(format!("{}:{}", json_escape(uri), body));
    }

    // 3. Builtin and renderer live probes from TSV plan (if supplied)
    let mut builtin_entries: BTreeMap<String, String> = BTreeMap::new();
    let mut renderer_entries: BTreeMap<String, String> = BTreeMap::new();
    if let Some(path) = plan_path {
        let plan = parse_plan(path)?;
        for (idx, item) in plan.iter().enumerate() {
            let primary_uri = format!("file:///m3-coverage-probe/plan-{idx}.pkl");
            let outcome = if item.output_format.is_some() {
                eval_pkl_fresh(
                    &primary_uri,
                    &item.source_code,
                    &item.expression,
                    item.output_format.as_deref(),
                )
            } else {
                let res = eval_on_probe(
                    &mut shared_probe,
                    shared_started,
                    &primary_uri,
                    &item.source_code,
                    &item.expression,
                    None,
                );
                shared_probe.collect();
                res
            };
            let detail = outcome.error.as_deref().map(truncate_detail);
            match item.category.as_str() {
                "builtin" => {
                    let (status, outcome_kind) = classify_builtin(&outcome);
                    let val = format!(
                        "{{\"detail\":{},\"disposition\":{},\"kind\":{},\"outcome\":{},\"status\":{},\"symbol\":{}}}",
                        json_opt_str(detail.as_deref()),
                        json_opt_str(outcome.disposition.as_deref()),
                        json_escape(&item.kind),
                        json_escape(outcome_kind),
                        json_escape(status),
                        json_escape(&item.symbol),
                    );
                    builtin_entries.insert(item.id.clone(), val);
                }
                "renderer" => {
                    let status = classify_renderer(&outcome);
                    let val = format!(
                        "{{\"detail\":{},\"disposition\":{},\"kind\":{},\"status\":{},\"symbol\":{}}}",
                        json_opt_str(detail.as_deref()),
                        json_opt_str(outcome.disposition.as_deref()),
                        json_escape(&item.kind),
                        json_escape(status),
                        json_escape(&item.symbol),
                    );
                    renderer_entries.insert(item.id.clone(), val);
                }
                other => {
                    return Err(format!("unknown plan category: {other}"));
                }
            }
        }
    }
    shared_probe.close();

    let builtins_json: Vec<String> = builtin_entries
        .iter()
        .map(|(k, v)| format!("{}:{}", json_escape(k), v))
        .collect();
    let renderers_json: Vec<String> = renderer_entries
        .iter()
        .map(|(k, v)| format!("{}:{}", json_escape(k), v))
        .collect();

    let payload = format!(
        "{{\"schema_version\":1,\"vendored_stdlib_files\":[{}],\"modules\":{{{}}},\"builtins\":{{{}}},\"renderers\":{{{}}}}}\n",
        vendored_rows.join(","),
        module_entries.join(","),
        builtins_json.join(","),
        renderers_json.join(","),
    );

    if let Some(out) = output_path {
        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("mkdir {}: {e}", parent.display()))?;
        }
        fs::write(out, payload.as_bytes()).map_err(|e| format!("write {}: {e}", out.display()))?;
    } else {
        std::io::stdout()
            .write_all(payload.as_bytes())
            .map_err(|e| format!("stdout write: {e}"))?;
    }
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut root = PathBuf::from(".");
    let mut plan: Option<PathBuf> = None;
    let mut output: Option<PathBuf> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--root" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("m3_coverage_probe: --root requires a path");
                    std::process::exit(2);
                }
                root = PathBuf::from(&args[i]);
            }
            "--plan" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("m3_coverage_probe: --plan requires a path");
                    std::process::exit(2);
                }
                plan = Some(PathBuf::from(&args[i]));
            }
            "--output" => {
                i += 1;
                if i >= args.len() {
                    eprintln!("m3_coverage_probe: --output requires a path");
                    std::process::exit(2);
                }
                output = Some(PathBuf::from(&args[i]));
            }
            other => {
                eprintln!("m3_coverage_probe: unknown option {other}");
                std::process::exit(2);
            }
        }
        i += 1;
    }
    if let Err(err) = run(&root, plan.as_deref(), output.as_deref()) {
        eprintln!("m3_coverage_probe: {err}");
        std::process::exit(1);
    }
}
