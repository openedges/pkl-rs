//! `pkl test` subcommand and shared `pkl:test` runner for `pkl project package`
//! (`CliTestRunner.kt`, `TestRunner.java`, `SpecReporter`, `MinimalReporter`, `JUnitReporter`).

use crate::env::{
    LoadedProject, build_effective_settings, build_native_host, discover_optional_project,
    effective_working_dir, format_module_host_error, load_user_settings, normalize_lexical,
    parse_module_header_info, resolve_path, resolve_use_color,
};
use crate::help::HELP_TEST;
use crate::options::{CliError, ParsedCliOptions, SubcommandKind, parse_options};
use pkl_host_native::{NativeHost, file_uri_to_path, path_to_file_uri};
use pkl_runtime::diagnostic::ansi::strip_ansi_codes;
use pkl_runtime::evaluation::frontend::DiagnosticProfile;
use pkl_runtime::evaluation::session::SourceSession;
use pkl_runtime::modules::ModuleSourceSpec;
use pkl_runtime::packages::resolver::file_uri_from_path;
use pkl_runtime::packages::test_runner::{
    RENDER_EXAMPLES_DOC_EXPR, TestError, TestFailure, TestReporterKind, TestResult, TestResults,
    TestSectionName, TestSectionResults, decode_rendered_examples_map,
    evaluate_examples_rendered_detailed, extract_fact_spans, format_fact_failure,
    format_fact_failure_with_power_assertions, render_junit_aggregate_report, render_junit_report,
    render_report, render_summary_colored,
};
use pkl_runtime::pkl_binary_decode::{DecodedValue, decode_lossless};
use pkl_runtime::source::SourceMap;
use pkl_runtime::value::PklString;
use std::collections::HashSet;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

const ACTUAL_EXAMPLES_MAP_EXPR: &str = r#"
let (r = new PcfRenderer { omitNullProperties = false })
examples.toMap().mapValues((_, g) ->
  g.toList().map((v) ->
    let (s = r.renderValue(v))
    if (v is Typed || v is Dynamic || v is Listing || v is Mapping) "new " + s else s
  )
)
"#;

const EXPECTED_EXAMPLES_MAP_EXPR: &str = r#"
let (r = new PcfRenderer { omitNullProperties = false })
let (ex = examples)
if (ex == null) throw("Invalid output file structure")
else ex.toMap().mapValues((_, g) ->
  g.toList().map((v) ->
    let (s = r.renderValue(v))
    if (v is Typed || v is Dynamic || v is Listing || v is Mapping) "new " + s else s
  )
)
"#;

pub fn cmd_test(args: &[String]) -> Result<i32, CliError> {
    let (opts, positionals) = parse_options(SubcommandKind::Test, args)?;
    if opts.show_help {
        crate::write_help(HELP_TEST);
        return Ok(0);
    }

    let working_dir = effective_working_dir(&opts)?;
    let host_env: Vec<(String, String)> = std::env::vars().collect();
    let user_settings = load_user_settings(&opts, &working_dir, &host_env)?;
    let project = discover_optional_project(&opts, &working_dir, &host_env)?;

    let mut sources: Vec<(String, PathBuf)> = if !positionals.is_empty() {
        let mut list = Vec::with_capacity(positionals.len());
        for p in &positionals {
            let path = if p.starts_with("file:") {
                file_uri_to_path(&PklString::from(p.as_str()))
                    .map(|pb| normalize_lexical(&pb))
                    .unwrap_or_else(|_| resolve_path(&working_dir, Path::new(p)))
            } else {
                resolve_path(&working_dir, Path::new(p))
            };
            let uri = file_uri_from_path(&path)
                .map_err(|e| CliError::err(format_module_host_error(&e)))?;
            list.push((uri, path));
        }
        list.sort_by(|a, b| a.0.cmp(&b.0));
        list
    } else if let Some(ref proj) = project {
        let mut list = Vec::with_capacity(proj.project.tests.len());
        for t in &proj.project.tests {
            let path = normalize_lexical(&proj.project.project_dir.join(t));
            let uri = file_uri_from_path(&path)
                .map_err(|e| CliError::err(format_module_host_error(&e)))?;
            list.push((uri, path));
        }
        list
    } else {
        return Err(CliError::usage(
            "Usage: pkl test [<options>] <modules>...",
            "missing argument <modules>",
        ));
    };
    sources.dedup_by(|a, b| a.0 == b.0);

    let (effective, effective_headers) = build_effective_settings(
        &opts,
        &working_dir,
        project.as_ref(),
        &user_settings,
        &host_env,
    );
    let use_color = resolve_use_color(effective.color);
    let shared_host = build_native_host(
        &opts,
        &working_dir,
        project.as_ref(),
        &effective,
        effective_headers.as_ref(),
        &[],
        use_color,
    )?;

    let junit_dir = opts
        .junit_reports
        .as_deref()
        .map(|d| resolve_path(&working_dir, d));

    let outcome = run_test_suite(
        &sources,
        &shared_host,
        opts.overwrite,
        opts.test_reporter,
        use_color,
        opts.power_assertions,
        opts.test_mode,
        &effective.editor_url_scheme,
        junit_dir.as_deref(),
        opts.junit_aggregate_reports,
        &opts.junit_aggregate_suite_name,
    );
    shared_host.close();

    match outcome? {
        TestSuiteOutcome::Passed => Ok(0),
        TestSuiteOutcome::ExampleWritten => Err(CliError::Silent(10)),
        TestSuiteOutcome::Failed => Err(CliError::Silent(1)),
    }
}

pub enum TestSuiteOutcome {
    Passed,
    ExampleWritten,
    Failed,
}

fn validate_junit_filename_stem(label: &str, stem: &str) -> Result<(), CliError> {
    if stem.is_empty()
        || matches!(stem, "." | "..")
        || stem.contains(['/', '\\', '\0'])
        || (cfg!(windows) && stem.contains(':'))
    {
        return Err(CliError::err(format!(
            "Cannot generate JUnit report: invalid {label} `{stem}` contains path separator or traversal segment."
        )));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn run_test_suite(
    sources: &[(String, PathBuf)],
    shared_host: &NativeHost,
    overwrite: bool,
    reporter: TestReporterKind,
    use_color: bool,
    power_assertions: bool,
    test_mode: bool,
    editor_url_scheme: &str,
    junit_dir: Option<&Path>,
    junit_aggregate_reports: bool,
    junit_aggregate_suite_name: &str,
) -> Result<TestSuiteOutcome, CliError> {
    if let Some(dir) = junit_dir {
        std::fs::create_dir_all(dir).map_err(|e| {
            CliError::err(format!(
                "Failed to create JUnit report directory `{}`: {e}",
                dir.display()
            ))
        })?;
    }

    let mut all_results = Vec::with_capacity(sources.len());
    let mut module_names: HashSet<String> = HashSet::new();
    let mut failed = false;
    let mut all_example_written = true;

    let mut stdout = io::stdout().lock();
    let mut stderr = io::stderr().lock();

    let mut session = shared_host.new_public_session(DiagnosticProfile::CliBundledSettings);

    for (idx, (uri, path)) in sources.iter().enumerate() {
        match run_single_test_module(
            path,
            uri,
            overwrite,
            shared_host,
            &mut session,
            use_color,
            power_assertions,
            test_mode,
            editor_url_scheme,
        ) {
            Ok(res) => {
                if !res.logs.is_empty() {
                    let _ = stderr.write_all(res.logs.as_bytes());
                    let _ = stderr.flush();
                }
                if res.failed() {
                    failed = true;
                    all_example_written = all_example_written && res.is_example_written_failure();
                }

                let report = render_report(reporter, &res, use_color);
                let _ = stdout.write_all(report.as_bytes());
                if !report.is_empty() && sources.len() > 1 && idx + 1 != sources.len() {
                    let _ = stdout.write_all(b"\n");
                }
                let _ = stdout.flush();

                if let Some(dir) = junit_dir {
                    if let Err(err) = validate_junit_filename_stem("module name", &res.module_name)
                    {
                        session.close();
                        return Err(err);
                    }
                    let file_name = format!("{}.xml", res.module_name);
                    if !module_names.insert(file_name.clone()) {
                        session.close();
                        return Err(CliError::err(format!(
                            "Cannot generate JUnit report for {uri}.\nA report with the same name was already generated.\n\nTo fix, provide a different name for this module by adding a module header."
                        )));
                    }
                    if !junit_aggregate_reports {
                        let xml = render_junit_report(&res);
                        let target = dir.join(&file_name);
                        pkl_host_native::write_file_nofollow(&target, xml.as_bytes()).map_err(
                            |e| {
                                CliError::err(format!(
                                    "Failed to write JUnit report `{}`: {e}",
                                    target.display()
                                ))
                            },
                        )?;
                    }
                }
                all_results.push(res);
            }
            Err(err_msg) => {
                let display_path = path.to_string_lossy();
                let _ = writeln!(stderr, "Error evaluating module {display_path}:");
                let _ = stderr.write_all(err_msg.as_bytes());
                if !strip_ansi_codes(&err_msg).ends_with('\n') {
                    let _ = stderr.write_all(b"\n");
                }
                if idx + 1 != sources.len() {
                    let _ = stderr.write_all(b"\n");
                }
                let _ = stderr.flush();
                failed = true;
                all_example_written = false;
            }
        }
    }

    session.close();

    if junit_aggregate_reports && let Some(dir) = junit_dir {
        validate_junit_filename_stem("aggregate suite name", junit_aggregate_suite_name)?;
        let file_name = format!("{junit_aggregate_suite_name}.xml");
        let xml = render_junit_aggregate_report(junit_aggregate_suite_name, &all_results);
        let target = dir.join(&file_name);
        pkl_host_native::write_file_nofollow(&target, xml.as_bytes()).map_err(|e| {
            CliError::err(format!(
                "Failed to write aggregated JUnit report `{}`: {e}",
                target.display()
            ))
        })?;
    }

    let _ = stdout.write_all(b"\n");
    let summary = render_summary_colored(&all_results, use_color);
    let _ = stdout.write_all(summary.as_bytes());
    let _ = stdout.flush();

    if failed {
        if all_example_written {
            Ok(TestSuiteOutcome::ExampleWritten)
        } else {
            Ok(TestSuiteOutcome::Failed)
        }
    } else {
        Ok(TestSuiteOutcome::Passed)
    }
}

/// Runs API tests for `pkl project package` using the same test runner & reporters.
pub fn run_project_api_tests(
    opts: &ParsedCliOptions,
    working_dir: &Path,
    loaded: &LoadedProject,
    user_settings: &crate::env::LoadedSettings,
    host_env: &[(String, String)],
    api_test_files: &[(String, PathBuf)],
) -> Result<bool, CliError> {
    let (effective, effective_headers) =
        build_effective_settings(opts, working_dir, Some(loaded), user_settings, host_env);
    let use_color = resolve_use_color(effective.color);
    let test_host = build_native_host(
        opts,
        working_dir,
        Some(loaded),
        &effective,
        effective_headers.as_ref(),
        &[],
        use_color,
    )?;
    let junit_dir = opts
        .junit_reports
        .as_deref()
        .map(|d| resolve_path(working_dir, d));
    let outcome = run_test_suite(
        api_test_files,
        &test_host,
        opts.overwrite,
        opts.test_reporter,
        use_color,
        opts.power_assertions,
        opts.test_mode,
        &effective.editor_url_scheme,
        junit_dir.as_deref(),
        opts.junit_aggregate_reports,
        &opts.junit_aggregate_suite_name,
    );
    test_host.close();
    match outcome? {
        TestSuiteOutcome::Passed => Ok(true),
        TestSuiteOutcome::ExampleWritten | TestSuiteOutcome::Failed => Ok(false),
    }
}

fn format_test_location(
    uri: &str,
    line: Option<usize>,
    test_mode: bool,
    editor_url_scheme: &str,
) -> String {
    if test_mode {
        return match line {
            Some(l) if uri.starts_with("file:") => format!("{uri}, line {l}"),
            _ => uri.to_string(),
        };
    }
    if uri.starts_with("file:") {
        match line {
            Some(l) => editor_url_scheme
                .replace("%{url}", uri)
                .replace("%{line}", &l.to_string()),
            None => uri.to_string(),
        }
    } else {
        uri.to_string()
    }
}

#[allow(clippy::too_many_arguments)]
fn run_single_test_module(
    test_path: &Path,
    uri: &str,
    overwrite: bool,
    host: &NativeHost,
    session: &mut SourceSession,
    use_color: bool,
    power_assertions: bool,
    test_mode: bool,
    editor_url_scheme: &str,
) -> Result<TestResults, String> {
    let norm = normalize_lexical(test_path);
    let inferred_name = norm
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("test")
        .to_string();

    let spec = ModuleSourceSpec {
        uri: PklString::from(uri),
        contents: None,
    };

    let test_text = match host.fs().resolve_file_module(&spec.uri) {
        Ok((_, real_path)) => {
            let bytes = host
                .fs()
                .read_file_module_bytes(&real_path)
                .map_err(|err| format!("Cannot read `{}`: {err:?}", norm.display()))?;
            String::from_utf8(bytes)
                .map_err(|err| format!("Cannot read `{}`: {err}", norm.display()))?
        }
        Err(pkl_runtime::modules::ModuleHostError::SecurityManagerException { .. }) => {
            // Do not read the file directly; let the VM emit the exact Pkl 0.32.0
            // SecurityManagerException diagnostic.
            let _ = host
                .evaluate_expression_pkl_binary_on(session, spec.clone(), "true")
                .map_err(|e| strip_trailing_repl_frames(&e))?;
            return Err(format!("Security manager rejected `{uri}`."));
        }
        Err(err) => {
            return Err(format!("Cannot read `{}`: {err:?}", norm.display()));
        }
    };

    // Fast path for direct `amends "pkl:test"` / `extends "pkl:test"` modules.
    let (direct_test_module, declared_name) = parse_module_header_info(&test_text, "pkl:test");
    let (module_name, is_test) = if direct_test_module {
        let name = declared_name.unwrap_or(inferred_name);
        // Ensure the module loads/parses cleanly (propagating syntax errors as Err).
        let _ = host
            .evaluate_expression_pkl_binary_on(session, spec.clone(), "true")
            .map_err(|e| strip_trailing_repl_frames(&e))?;
        (name, true)
    } else {
        let check_expr = r#"
let (reflect = import("pkl:reflect"))
let (testMod = import("pkl:test"))
let (m = reflect.Module(module))
new Mapping {
  ["moduleName"] = m.name
  ["isTest"] = reflect.Class(module.getClass()).isSubclassOf(reflect.Class(testMod.getClass()))
}
"#;
        let check_bytes = host
            .evaluate_expression_pkl_binary_on(session, spec.clone(), check_expr)
            .map_err(|e| strip_trailing_repl_frames(&e))?;
        let decoded = decode_lossless(&check_bytes)
            .map_err(|e| format!("Failed to decode test module check: {e:?}"))?;
        let mut mod_name = inferred_name;
        let mut amends_test = false;
        if let DecodedValue::Mapping(entries) | DecodedValue::Map(entries) = &decoded {
            for (k, v) in entries {
                match (k.to_java_key_string().as_str(), v) {
                    (Some("moduleName"), DecodedValue::String(s)) => mod_name = s.to_java_utf8(),
                    (Some("isTest"), DecodedValue::Bool(b)) => amends_test = *b,
                    _ => {}
                }
            }
        }
        (mod_name, amends_test)
    };

    let display_uri = format_test_location(uri, None, test_mode, editor_url_scheme);
    if !is_test {
        // Mirrors `TestRunner.java:71-77` (`VmUtils.checkAmends(testModule, TestModule.getModule().getVmClass())`):
        // frameless `VmEvalException` with uncolored `Value: <moduleName>` program value.
        let err_msg = pkl_runtime::diagnostic::render_error_text_with_trailer_colored(
            &"Expected value of type `pkl.test`, but got type `Class`.".into(),
            &format!("Value: {module_name}").into(),
            &[],
            None,
            use_color,
        )
        .to_java_utf8();
        return Ok(TestResults {
            module_name,
            display_uri,
            facts: TestSectionResults::empty(TestSectionName::Facts),
            examples: TestSectionResults::empty(TestSectionName::Examples),
            logs: String::new(),
            error: Some(TestError {
                message: extract_short_error_message(&err_msg),
                rendered: err_msg,
            }),
            generated_expected_pcf: None,
            generated_actual_pcf: None,
        });
    }

    // 1. Evaluate `facts` element-by-element so errors in one fact don't abort others.
    let facts = run_test_facts(
        host,
        session,
        &spec,
        uri,
        &test_text,
        use_color,
        power_assertions,
        test_mode,
        editor_url_scheme,
    )?;

    // 2. Evaluate `examples` against `*-expected.pcf` / `*-actual.pcf`.
    let (examples, gen_expected, gen_actual) = run_test_examples(
        host,
        session,
        &spec,
        &norm,
        uri,
        &test_text,
        overwrite,
        use_color,
        test_mode,
        editor_url_scheme,
    )?;

    Ok(TestResults {
        module_name,
        display_uri,
        facts,
        examples,
        logs: String::new(),
        error: None,
        generated_expected_pcf: gen_expected,
        generated_actual_pcf: gen_actual,
    })
}

#[allow(clippy::too_many_arguments)]
fn run_test_facts(
    host: &NativeHost,
    session: &mut SourceSession,
    spec: &ModuleSourceSpec,
    module_uri: &str,
    test_text: &str,
    use_color: bool,
    power_assertions: bool,
    test_mode: bool,
    editor_url_scheme: &str,
) -> Result<TestSectionResults, String> {
    let shape_expr = "if (facts == null) null else facts.toMap().mapValues((_, g) -> g.length)";
    let shape_bytes = host
        .evaluate_expression_pkl_binary_on(session, spec.clone(), shape_expr)
        .map_err(|e| strip_trailing_repl_frames(&e))?;
    let shape_decoded = decode_lossless(&shape_bytes)
        .map_err(|e| format!("Failed to decode facts shape: {e:?}"))?;
    let groups = match &shape_decoded {
        DecodedValue::Null => return Ok(TestSectionResults::empty(TestSectionName::Facts)),
        DecodedValue::Map(entries) | DecodedValue::Mapping(entries) => entries,
        _ => return Ok(TestSectionResults::empty(TestSectionName::Facts)),
    };

    let fact_spans = extract_fact_spans(test_text);
    let mut results = Vec::with_capacity(groups.len());

    for (group_key, len_val) in groups {
        let group_name = group_key.to_java_key_string().to_java_utf8();
        let count = match len_val {
            DecodedValue::Int(n) if *n >= 0 => *n as usize,
            _ => 0,
        };
        let group_locs = fact_spans.get(&group_name);
        let mut total_asserts = 0usize;
        let mut failures = Vec::new();
        let mut errors = Vec::new();

        for idx in 0..count {
            let expr = format!("facts[{}][{idx}]", pkl_quote_str(&group_name));
            total_asserts += 1;
            match host.evaluate_expression_pkl_binary_on(session, spec.clone(), &expr) {
                Ok(bytes) => {
                    let decoded = decode_lossless(&bytes)
                        .map_err(|e| format!("Failed to decode fact value: {e:?}"))?;
                    if matches!(decoded, DecodedValue::Bool(false)) {
                        let failure = build_fact_failure(
                            host,
                            session,
                            spec,
                            module_uri,
                            test_text,
                            &group_name,
                            group_locs.and_then(|v| v.get(idx)),
                            idx,
                            use_color,
                            power_assertions,
                            test_mode,
                            editor_url_scheme,
                        );
                        failures.push(failure);
                    }
                }
                Err(rendered) => {
                    let cleaned = strip_trailing_repl_frames(&rendered);
                    let short = extract_short_error_message(&cleaned);
                    errors.push(TestError {
                        message: short,
                        rendered: cleaned,
                    });
                }
            }
        }

        results.push(TestResult {
            name: group_name,
            total_asserts,
            failures,
            errors,
            is_example_written: false,
        });
    }

    Ok(TestSectionResults {
        name: TestSectionName::Facts,
        results,
    })
}

#[allow(clippy::too_many_arguments)]
fn build_fact_failure(
    host: &NativeHost,
    session: &mut SourceSession,
    spec: &ModuleSourceSpec,
    module_uri: &str,
    test_text: &str,
    group_name: &str,
    loc_info: Option<&(String, usize, pkl_runtime::source::Span)>,
    idx: usize,
    use_color: bool,
    power_assertions: bool,
    test_mode: bool,
    editor_url_scheme: &str,
) -> TestFailure {
    let Some((expr_text, line, fact_span)) = loc_info else {
        let loc = format_test_location(module_uri, None, test_mode, editor_url_scheme);
        return format_fact_failure(&format!("fact #{idx} == false"), &loc, use_color);
    };
    let location = format_test_location(module_uri, Some(*line), test_mode, editor_url_scheme);

    if power_assertions {
        // Mirror `TestRunner.java:105-115`: clear cached values on the `facts[group]`
        // listing and re-evaluate `facts[group][idx]` in its real lexical module scope
        // under an active VM power-assertion value tracker.
        let fact_expr = format!("facts[{}][{idx}]", pkl_quote_str(group_name));
        let mod_uri = PklString::from(module_uri);
        let grp_name = PklString::from(group_name);
        if session.begin_fact_trace(&mod_uri, &grp_name, *fact_span) {
            let timeout_ms = host.timeout_ms();
            let now = if timeout_ms.is_some() {
                host.clock_ms()
            } else {
                session.host_session().last_clock()
            };
            let mut stderr = Vec::new();
            let _ = session
                .start_evaluation_pkl_binary(spec.clone(), fact_expr.as_str(), now, timeout_ms)
                .and_then(|()| host.drive_public_session(session, &mut stderr));
            let observations = session.finish_fact_trace();
            let mut sm = SourceMap::default();
            let sid = sm.add(module_uri, test_text);
            if let Some(src) = sm.get(sid) {
                return format_fact_failure_with_power_assertions(
                    src,
                    *fact_span,
                    &observations,
                    &location,
                    use_color,
                );
            }
        }
    }

    format_fact_failure(expr_text, &location, use_color)
}

#[allow(clippy::too_many_arguments)]
fn run_test_examples(
    host: &NativeHost,
    session: &mut SourceSession,
    spec: &ModuleSourceSpec,
    module_path: &Path,
    module_uri: &str,
    test_text: &str,
    overwrite: bool,
    use_color: bool,
    test_mode: bool,
    editor_url_scheme: &str,
) -> Result<(TestSectionResults, Option<String>, Option<String>), String> {
    let is_null_bytes = host
        .evaluate_expression_pkl_binary_on(session, spec.clone(), "examples == null")
        .map_err(|e| strip_trailing_repl_frames(&e))?;
    if matches!(
        decode_lossless(&is_null_bytes),
        Ok(DecodedValue::Bool(true))
    ) {
        return Ok((
            TestSectionResults::empty(TestSectionName::Examples),
            None,
            None,
        ));
    }

    let file_name = module_path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let expected_path = module_path.with_file_name(format!("{file_name}-expected.pcf"));
    let actual_path = module_path.with_file_name(format!("{file_name}-actual.pcf"));

    let expected_uri = path_to_file_uri(&expected_path).to_java_utf8();
    let actual_uri = path_to_file_uri(&actual_path).to_java_utf8();
    let expected_uri_pkl = PklString::from(expected_uri.as_str());
    let actual_uri_pkl = PklString::from(actual_uri.as_str());

    host.check_resolve_module(&expected_uri_pkl)
        .map_err(|err| format!("{err:?}"))?;
    host.check_resolve_module(&actual_uri_pkl)
        .map_err(|err| format!("{err:?}"))?;
    let canon_expected = pkl_host_native::try_resolve_canonical_or_ancestor(&expected_path)
        .map_err(|()| {
            pkl_runtime::host::security::root_dir_denied_message(&expected_uri_pkl, false)
                .to_java_utf8()
        })?;
    let canon_expected_uri = path_to_file_uri(&canon_expected);
    if canon_expected_uri != expected_uri_pkl {
        host.check_resolve_module(&canon_expected_uri)
            .map_err(|err| format!("{err:?}"))?;
    }
    let canon_actual =
        pkl_host_native::try_resolve_canonical_or_ancestor(&actual_path).map_err(|()| {
            pkl_runtime::host::security::root_dir_denied_message(&actual_uri_pkl, false)
                .to_java_utf8()
        })?;
    let canon_actual_uri = path_to_file_uri(&canon_actual);
    if canon_actual_uri != actual_uri_pkl {
        host.check_resolve_module(&canon_actual_uri)
            .map_err(|err| format!("{err:?}"))?;
    }

    // Unlink any existing file or dangling symlink before generating test outputs.
    match std::fs::remove_file(&actual_path) {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => {
            return Err(format!(
                "I/O error writing test output file `{}`: {err}",
                actual_path.display()
            ));
        }
    }
    if overwrite {
        match std::fs::remove_file(&expected_path) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => {
                return Err(format!(
                    "I/O error writing test output file `{}`: {err}",
                    expected_path.display()
                ));
            }
        }
    }

    let (expected_groups, expected_source) = match std::fs::symlink_metadata(&expected_path) {
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => (None, None),
        Err(err) => {
            return Err(format!(
                "I/O error reading test output file `{}`: {err}",
                expected_path.display()
            ));
        }
        Ok(meta) if meta.is_dir() => (None, None),
        Ok(_) => {
            let exp_bytes_opt = match host.fs().resolve_file_module(&expected_uri_pkl) {
                Ok((resolved, real_path)) => {
                    if resolved.resolved_uri != expected_uri_pkl {
                        host.check_resolve_module(&resolved.resolved_uri)
                            .map_err(|err| format!("{err:?}"))?;
                    }
                    match host.fs().read_file_module_bytes(&real_path) {
                        Ok(b) => Some(b),
                        Err(pkl_runtime::modules::ModuleHostError::NotFound) => None,
                        Err(err) => {
                            return Err(format!(
                                "I/O error reading test output file `{}`: {err:?}",
                                expected_path.display()
                            ));
                        }
                    }
                }
                Err(pkl_runtime::modules::ModuleHostError::NotFound) => None,
                Err(pkl_runtime::modules::ModuleHostError::SecurityManagerException {
                    message,
                }) => {
                    return Err(message.map(|m| m.to_java_utf8()).unwrap_or_else(|| {
                        format!("Security violation reading `{}`", expected_path.display())
                    }));
                }
                Err(err) => {
                    return Err(format!(
                        "I/O error reading test output file `{}`: {err:?}",
                        expected_path.display()
                    ));
                }
            };
            if let Some(raw_bytes) = exp_bytes_opt {
                let exp_text = String::from_utf8(raw_bytes).map_err(|err| {
                    format!(
                        "I/O error reading test output file `{}`: {err}",
                        expected_path.display()
                    )
                })?;
                let exp_spec = ModuleSourceSpec {
                    uri: expected_uri_pkl.clone(),
                    contents: Some(exp_text.as_str().into()),
                };
                let exp_bytes = host
                    .evaluate_expression_pkl_binary_on(
                        session,
                        exp_spec,
                        EXPECTED_EXAMPLES_MAP_EXPR,
                    )
                    .map_err(|e| strip_trailing_repl_frames(&e))?;
                let exp_decoded = decode_lossless(&exp_bytes)
                    .map_err(|e| format!("Failed to decode expected examples: {e:?}"))?;
                (
                    Some(decode_rendered_examples_map(Some(&exp_decoded))),
                    Some(exp_text),
                )
            } else {
                (None, None)
            }
        }
    };

    // Fast path: bulk evaluate all example groups. Fallback to isolated per-example on error.
    let (section, gen_expected, gen_actual) = match host.evaluate_expression_pkl_binary_on(
        session,
        spec.clone(),
        ACTUAL_EXAMPLES_MAP_EXPR,
    ) {
        Ok(actual_bytes) => {
            let actual_decoded = decode_lossless(&actual_bytes)
                .map_err(|e| format!("Failed to decode examples: {e:?}"))?;
            let actual_groups = decode_rendered_examples_map(Some(&actual_decoded));
            let full_doc = host
                .evaluate_expression_string_on(session, spec.clone(), RENDER_EXAMPLES_DOC_EXPR)
                .ok()
                .filter(|s| !s.is_empty());
            evaluate_examples_rendered_detailed(
                &actual_groups,
                expected_groups.as_deref(),
                module_uri,
                &expected_uri,
                &actual_uri,
                full_doc,
                Some(test_text),
                expected_source.as_deref(),
                use_color,
                |u, l| format_test_location(u, l, test_mode, editor_url_scheme),
            )
        }
        Err(_) => {
            let section = evaluate_examples_isolated(
                host,
                session,
                spec,
                module_uri,
                expected_groups.as_deref(),
                &expected_uri,
                &actual_uri,
                use_color,
                test_mode,
                editor_url_scheme,
            )?;
            (section, None, None)
        }
    };

    if let Some(ref content) = gen_expected {
        if host.fs().policy().root_dir().is_some() {
            host.fs()
                .check_uri(&expected_uri_pkl, true)
                .map_err(|msg| msg.to_java_utf8())?;
        }
        pkl_host_native::write_file_nofollow(&expected_path, content.as_bytes()).map_err(
            |err| {
                format!(
                    "I/O error writing test output file `{}`: {err}",
                    expected_path.display()
                )
            },
        )?;
    }
    if let Some(ref content) = gen_actual {
        if host.fs().policy().root_dir().is_some() {
            host.fs()
                .check_uri(&actual_uri_pkl, true)
                .map_err(|msg| msg.to_java_utf8())?;
        }
        pkl_host_native::write_file_nofollow(&actual_path, content.as_bytes()).map_err(|err| {
            format!(
                "I/O error writing test output file `{}`: {err}",
                actual_path.display()
            )
        })?;
    }

    Ok((section, gen_expected, gen_actual))
}

#[allow(clippy::too_many_arguments)]
fn evaluate_examples_isolated(
    host: &NativeHost,
    session: &mut SourceSession,
    spec: &ModuleSourceSpec,
    module_uri: &str,
    expected_groups: Option<&[(String, Vec<String>)]>,
    expected_uri: &str,
    actual_uri: &str,
    use_color: bool,
    test_mode: bool,
    editor_url_scheme: &str,
) -> Result<TestSectionResults, String> {
    let shape_expr =
        "if (examples == null) null else examples.toMap().mapValues((_, g) -> g.length)";
    let shape_bytes = host
        .evaluate_expression_pkl_binary_on(session, spec.clone(), shape_expr)
        .map_err(|e| strip_trailing_repl_frames(&e))?;
    let shape_decoded = decode_lossless(&shape_bytes)
        .map_err(|e| format!("Failed to decode examples shape: {e:?}"))?;
    let groups = match &shape_decoded {
        DecodedValue::Map(entries) | DecodedValue::Mapping(entries) => entries,
        _ => return Ok(TestSectionResults::empty(TestSectionName::Examples)),
    };

    let mut results = Vec::with_capacity(groups.len());
    for (group_key, len_val) in groups {
        let group_name = group_key.to_java_key_string().to_java_utf8();
        let count = match len_val {
            DecodedValue::Int(n) if *n >= 0 => *n as usize,
            _ => 0,
        };
        let mut failures = Vec::new();
        let mut errors = Vec::new();
        let mut total_asserts = 0usize;
        let group_loc = format_test_location(module_uri, None, test_mode, editor_url_scheme);

        match expected_groups {
            Some(exp_list) => {
                let Some((_, exp_items)) = exp_list.iter().find(|(k, _)| k == &group_name) else {
                    failures.push(
                        pkl_runtime::packages::test_runner::format_example_property_mismatch(
                            &group_loc,
                            &group_name,
                            true,
                            use_color,
                        ),
                    );
                    results.push(TestResult {
                        name: group_name,
                        total_asserts: 1,
                        failures,
                        errors,
                        is_example_written: false,
                    });
                    continue;
                };
                if exp_items.len() != count {
                    failures.push(
                        pkl_runtime::packages::test_runner::format_example_length_mismatch(
                            &group_loc,
                            &group_name,
                            exp_items.len(),
                            count,
                            use_color,
                        ),
                    );
                    results.push(TestResult {
                        name: group_name,
                        total_asserts: 1,
                        failures,
                        errors,
                        is_example_written: false,
                    });
                    continue;
                }
                for (idx, exp_item) in exp_items.iter().enumerate() {
                    total_asserts += 1;
                    let expr = format!(
                        "let (r = new PcfRenderer {{ omitNullProperties = false }}) let (v = examples[{}][{idx}]) let (s = r.renderValue(v)) if (v is Typed || v is Dynamic || v is Listing || v is Mapping) \"new \" + s else s",
                        pkl_quote_str(&group_name)
                    );
                    match host.evaluate_expression_string_on(session, spec.clone(), &expr) {
                        Ok(actual_pcf) => {
                            if &actual_pcf != exp_item {
                                let exp_loc = format_test_location(
                                    expected_uri,
                                    None,
                                    test_mode,
                                    editor_url_scheme,
                                );
                                let act_loc = format_test_location(
                                    actual_uri,
                                    None,
                                    test_mode,
                                    editor_url_scheme,
                                );
                                failures.push(
                                    pkl_runtime::packages::test_runner::format_example_failure(
                                        &group_loc,
                                        &exp_loc,
                                        exp_item,
                                        &act_loc,
                                        &actual_pcf,
                                        idx,
                                        use_color,
                                    ),
                                );
                            }
                        }
                        Err(rendered) => {
                            let cleaned = strip_trailing_repl_frames(&rendered);
                            let short = extract_short_error_message(&cleaned);
                            errors.push(TestError {
                                message: short,
                                rendered: cleaned,
                            });
                        }
                    }
                }
            }
            None => {
                for idx in 0..count {
                    total_asserts += 1;
                    let expr = format!("examples[{}][{idx}]", pkl_quote_str(&group_name));
                    if let Err(rendered) =
                        host.evaluate_expression_pkl_binary_on(session, spec.clone(), &expr)
                    {
                        let cleaned = strip_trailing_repl_frames(&rendered);
                        let short = extract_short_error_message(&cleaned);
                        errors.push(TestError {
                            message: short,
                            rendered: cleaned,
                        });
                    }
                }
            }
        }

        results.push(TestResult {
            name: group_name,
            total_asserts,
            failures,
            errors,
            is_example_written: false,
        });
    }

    Ok(TestSectionResults {
        name: TestSectionName::Examples,
        results,
    })
}

/// Removes trailing synthetic `repl:text` probe frames (supporting both plain and ANSI-colored
/// error renders) while preserving trailing newline / ANSI reset semantics matching
/// `PklException.getMessage()`.
pub fn strip_trailing_repl_frames(rendered: &str) -> String {
    let had_trailing_nl = rendered.ends_with('\n');
    let mut lines: Vec<&str> = rendered.split('\n').collect();
    if had_trailing_nl && lines.last() == Some(&"") {
        lines.pop();
    }
    let mut trailing_reset: Option<&str> = None;
    if lines
        .last()
        .is_some_and(|l| !l.is_empty() && strip_ansi_codes(l).is_empty())
    {
        trailing_reset = lines.pop();
    }

    while let Some(last_idx) = lines
        .iter()
        .rposition(|l| !strip_ansi_codes(l).trim().is_empty())
    {
        let plain_last = strip_ansi_codes(lines[last_idx]);
        let trimmed = plain_last.trim_end();
        let is_repl_frame = trimmed.ends_with("(repl:text)")
            || (trimmed.ends_with(')') && trimmed.contains(" (repl:text, line "));
        if !is_repl_frame {
            break;
        }
        let Some(sep_idx) = lines[..last_idx]
            .iter()
            .rposition(|l| strip_ansi_codes(l).trim().is_empty())
        else {
            break;
        };
        if sep_idx == 0 {
            break;
        }
        let sep_line = lines[sep_idx];
        if sep_line.starts_with("\x1b[0m") {
            trailing_reset = Some("\x1b[0m");
        } else {
            trailing_reset = None;
        }
        lines.truncate(sep_idx);
    }

    let mut out = lines.join("\n");
    if let Some(reset) = trailing_reset {
        out.push('\n');
        out.push_str(reset);
    }
    if had_trailing_nl && !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

fn extract_short_error_message(rendered: &str) -> String {
    let stripped = strip_ansi_codes(rendered);
    let body = stripped
        .strip_prefix("–– Pkl Error ––\n")
        .unwrap_or(&stripped);
    body.split("\n\n")
        .next()
        .unwrap_or(body)
        .trim_end_matches('\n')
        .to_string()
}

pub fn pkl_quote_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
