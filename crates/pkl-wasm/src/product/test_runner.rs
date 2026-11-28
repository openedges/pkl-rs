//! Pure-Wasm `pkl:test` helpers wrapping `pkl_runtime::packages::test_runner` so
//! `evaluateTest` in Wasm shares 100% of fact span extraction, example diffing, and
//! Spec / Minimal / JUnit rendering with native Pkl 0.32.0.

use super::util::{StructuredError, define_own_prop, get_prop, rust_string_from_js, set_prop};
use js_sys::{Array, Object};
use pkl_runtime::packages::test_runner::{
    TestError, TestFailure, TestReporterKind, TestResult, TestResults, TestSectionName,
    TestSectionResults, evaluate_examples_rendered_detailed, extract_fact_spans,
    format_fact_failure, render_junit_report, render_report, render_summary_colored,
};
use wasm_bindgen::JsValue;

pub fn extract_fact_spans_js(source: &str) -> Result<JsValue, JsValue> {
    let spans = extract_fact_spans(source);
    let obj = Object::new();
    for (group, items) in spans {
        let arr = Array::new();
        for (expr, line, _) in items {
            let entry = Object::new();
            set_prop(&entry, "expr", JsValue::from_str(&expr))?;
            set_prop(&entry, "line", JsValue::from_f64(line as f64))?;
            arr.push(&entry);
        }
        define_own_prop(&obj, &group, arr.into())?;
    }
    Ok(obj.into())
}

pub fn format_fact_failure_js(
    expr_text: &str,
    location: &str,
    use_color: bool,
) -> Result<JsValue, JsValue> {
    let failure = format_fact_failure(expr_text, location, use_color);
    let obj = Object::new();
    set_prop(&obj, "kind", JsValue::from_str(&failure.kind))?;
    set_prop(&obj, "message", JsValue::from_str(&failure.message))?;
    Ok(obj.into())
}

#[allow(clippy::too_many_arguments)]
pub fn evaluate_examples_js(
    actual_groups_js: &JsValue,
    expected_groups_js: &JsValue,
    module_uri: &str,
    expected_uri: &str,
    actual_uri: &str,
    full_actual_doc: Option<String>,
    module_source: Option<&str>,
    expected_source: Option<&str>,
    use_color: bool,
) -> Result<JsValue, JsValue> {
    let actual_groups = parse_rendered_groups(actual_groups_js)?;
    let expected_groups = if expected_groups_js.is_null() || expected_groups_js.is_undefined() {
        None
    } else {
        Some(parse_rendered_groups(expected_groups_js)?)
    };
    let (section, gen_expected, gen_actual) = evaluate_examples_rendered_detailed(
        &actual_groups,
        expected_groups.as_deref(),
        module_uri,
        expected_uri,
        actual_uri,
        full_actual_doc,
        module_source,
        expected_source,
        use_color,
        |uri, line| match line {
            Some(l) if uri.starts_with("file:") => format!("{uri}, line {l}"),
            _ => uri.to_string(),
        },
    );
    let obj = Object::new();
    set_prop(&obj, "section", section_to_js(&section)?)?;
    set_prop(
        &obj,
        "generatedExpectedPcf",
        gen_expected
            .as_deref()
            .map_or(JsValue::NULL, JsValue::from_str),
    )?;
    set_prop(
        &obj,
        "generatedActualPcf",
        gen_actual
            .as_deref()
            .map_or(JsValue::NULL, JsValue::from_str),
    )?;
    Ok(obj.into())
}

pub fn render_test_report_js(
    results_js: &JsValue,
    reporter: &str,
    use_color: bool,
) -> Result<String, StructuredError> {
    let results = parse_test_results(results_js)?;
    match reporter {
        "spec" => {
            let mut out = render_report(TestReporterKind::Spec, &results, use_color);
            out.push('\n');
            out.push_str(&render_summary_colored(
                std::slice::from_ref(&results),
                use_color,
            ));
            Ok(out)
        }
        "minimal" => {
            let mut out = render_report(TestReporterKind::Minimal, &results, use_color);
            out.push('\n');
            out.push_str(&render_summary_colored(
                std::slice::from_ref(&results),
                use_color,
            ));
            Ok(out)
        }
        "junit" => Ok(render_junit_report(&results)),
        other => Err(StructuredError::invalid_argument(format!(
            "invalid test reporter `{other}` (expected \"spec\", \"minimal\", or \"junit\")"
        ))),
    }
}

fn parse_rendered_groups(val: &JsValue) -> Result<Vec<(String, Vec<String>)>, StructuredError> {
    if !Array::is_array(val) {
        return Err(StructuredError::invalid_argument(
            "expected array of [groupName, renderedValues]",
        ));
    }
    let mut out = Vec::new();
    for entry in Array::from(val).iter() {
        let pair = Array::from(&entry);
        let name = rust_string_from_js(&pair.get(0))?;
        let vals_raw = pair.get(1);
        let mut vals = Vec::new();
        if Array::is_array(&vals_raw) {
            for item in Array::from(&vals_raw).iter() {
                vals.push(rust_string_from_js(&item)?);
            }
        }
        out.push((name, vals));
    }
    Ok(out)
}

fn section_to_js(section: &TestSectionResults) -> Result<JsValue, JsValue> {
    let obj = Object::new();
    set_prop(&obj, "name", JsValue::from_str(section.name.as_str()))?;
    let results = Array::new();
    for r in &section.results {
        let item = Object::new();
        set_prop(&item, "name", JsValue::from_str(&r.name))?;
        set_prop(
            &item,
            "totalAsserts",
            JsValue::from_f64(r.total_asserts as f64),
        )?;
        set_prop(
            &item,
            "isExampleWritten",
            JsValue::from_bool(r.is_example_written),
        )?;
        let failures = Array::new();
        for f in &r.failures {
            let fo = Object::new();
            set_prop(&fo, "kind", JsValue::from_str(&f.kind))?;
            set_prop(&fo, "message", JsValue::from_str(&f.message))?;
            failures.push(&fo);
        }
        set_prop(&item, "failures", failures.into())?;
        let errors = Array::new();
        for e in &r.errors {
            let eo = Object::new();
            set_prop(&eo, "message", JsValue::from_str(&e.message))?;
            set_prop(&eo, "rendered", JsValue::from_str(&e.rendered))?;
            errors.push(&eo);
        }
        set_prop(&item, "errors", errors.into())?;
        results.push(&item);
    }
    set_prop(&obj, "results", results.into())?;
    Ok(obj.into())
}

fn parse_test_results(val: &JsValue) -> Result<TestResults, StructuredError> {
    let module_name = rust_string_from_js(&get_prop(val, "moduleName")?)?;
    let display_uri = rust_string_from_js(&get_prop(val, "displayUri")?)?;
    let logs = {
        let v = get_prop(val, "logs")?;
        if v.is_null() || v.is_undefined() {
            String::new()
        } else {
            rust_string_from_js(&v)?
        }
    };
    let error = {
        let v = get_prop(val, "error")?;
        if v.is_null() || v.is_undefined() {
            None
        } else {
            Some(TestError {
                message: rust_string_from_js(&get_prop(&v, "message")?)?,
                rendered: rust_string_from_js(&get_prop(&v, "rendered")?)?,
            })
        }
    };
    let facts = parse_section(&get_prop(val, "facts")?, TestSectionName::Facts)?;
    let examples = parse_section(&get_prop(val, "examples")?, TestSectionName::Examples)?;
    Ok(TestResults {
        module_name,
        display_uri,
        facts,
        examples,
        logs,
        error,
        generated_expected_pcf: None,
        generated_actual_pcf: None,
    })
}

fn parse_section(
    val: &JsValue,
    name: TestSectionName,
) -> Result<TestSectionResults, StructuredError> {
    if val.is_null() || val.is_undefined() {
        return Ok(TestSectionResults::empty(name));
    }
    let results_val = get_prop(val, "results")?;
    let mut results = Vec::new();
    if Array::is_array(&results_val) {
        for item in Array::from(&results_val).iter() {
            let r_name = rust_string_from_js(&get_prop(&item, "name")?)?;
            let total_asserts = get_prop(&item, "totalAsserts")?.as_f64().unwrap_or(0.0) as usize;
            let is_example_written = get_prop(&item, "isExampleWritten")?
                .as_bool()
                .unwrap_or(false);
            let mut failures = Vec::new();
            let f_val = get_prop(&item, "failures")?;
            if Array::is_array(&f_val) {
                for f in Array::from(&f_val).iter() {
                    failures.push(TestFailure {
                        kind: rust_string_from_js(&get_prop(&f, "kind")?)?,
                        message: rust_string_from_js(&get_prop(&f, "message")?)?,
                    });
                }
            }
            let mut errors = Vec::new();
            let e_val = get_prop(&item, "errors")?;
            if Array::is_array(&e_val) {
                for e in Array::from(&e_val).iter() {
                    errors.push(TestError {
                        message: rust_string_from_js(&get_prop(&e, "message")?)?,
                        rendered: rust_string_from_js(&get_prop(&e, "rendered")?)?,
                    });
                }
            }
            results.push(TestResult {
                name: r_name,
                total_asserts,
                failures,
                errors,
                is_example_written,
            });
        }
    }
    Ok(TestSectionResults { name, results })
}
