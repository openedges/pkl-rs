//! Wasm-pure `pkl:test` runner & reporters (`SpecReporter`, `MinimalReporter`, `JUnitReporter`)
//! with ANSI color support and power-assertion diagrams matching Apple Pkl 0.32.0.
use crate::diagnostic::ansi::{AnsiCode, AnsiStringBuilder, strip_ansi_codes, theme};
use crate::pkl_binary_decode::DecodedValue;
use crate::source::{SourceText, Span};
use crate::syntax::{ExprKind, ObjectMember};
use std::collections::BTreeMap;

#[non_exhaustive]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TestReporterKind {
    #[default]
    Spec,
    Minimal,
}

#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TestSectionName {
    Facts,
    Examples,
}

impl TestSectionName {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Facts => "facts",
            Self::Examples => "examples",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TestFailure {
    pub kind: String,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TestError {
    pub message: String,
    pub rendered: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TestResult {
    pub name: String,
    pub total_asserts: usize,
    pub failures: Vec<TestFailure>,
    pub errors: Vec<TestError>,
    pub is_example_written: bool,
}

impl TestResult {
    pub fn total_asserts_failed_or_errored(&self) -> usize {
        self.failures.len() + self.errors.len()
    }

    pub fn is_failure(&self) -> bool {
        self.total_asserts_failed_or_errored() > 0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TestSectionResults {
    pub name: TestSectionName,
    pub results: Vec<TestResult>,
}

impl TestSectionResults {
    pub fn empty(name: TestSectionName) -> Self {
        Self {
            name,
            results: Vec::new(),
        }
    }

    pub fn total_tests(&self) -> usize {
        self.results.len()
    }

    pub fn total_asserts(&self) -> usize {
        self.results.iter().map(|r| r.total_asserts).sum()
    }

    pub fn total_asserts_failed_or_errored(&self) -> usize {
        self.results
            .iter()
            .map(TestResult::total_asserts_failed_or_errored)
            .sum()
    }

    pub fn total_failures(&self) -> usize {
        self.results.iter().filter(|r| r.is_failure()).count()
    }

    pub fn failed(&self) -> bool {
        self.results.iter().any(TestResult::is_failure)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TestResults {
    pub module_name: String,
    pub display_uri: String,
    pub facts: TestSectionResults,
    pub examples: TestSectionResults,
    pub logs: String,
    pub error: Option<TestError>,
    /// Optional `*-expected.pcf` text generated when expected file did not exist (or `--overwrite`).
    pub generated_expected_pcf: Option<String>,
    /// Optional `*-actual.pcf` text generated when example output did not match expected.
    pub generated_actual_pcf: Option<String>,
}

impl TestResults {
    pub fn total_tests(&self) -> usize {
        self.facts.total_tests() + self.examples.total_tests()
    }

    pub fn total_failures(&self) -> usize {
        self.facts.total_failures() + self.examples.total_failures()
    }

    pub fn total_asserts(&self) -> usize {
        self.facts.total_asserts() + self.examples.total_asserts()
    }

    pub fn total_asserts_failed(&self) -> usize {
        self.facts.total_asserts_failed_or_errored()
            + self.examples.total_asserts_failed_or_errored()
    }

    pub fn failed(&self) -> bool {
        self.error.is_some() || self.facts.failed() || self.examples.failed()
    }

    pub fn is_example_written_failure(&self) -> bool {
        if !self.failed() || self.facts.failed() || !self.examples.failed() {
            return false;
        }
        self.examples.results.iter().all(|r| r.is_example_written)
    }
}

/// Extracts `(source_expression, one_based_line)` for each element expression inside
/// `facts { ["group"] { ... } }` from the test module source (`TestRunner.java:105-120, 406-427`).
pub fn extract_fact_locations(source: &str) -> BTreeMap<String, Vec<(String, usize)>> {
    extract_fact_spans(source)
        .into_iter()
        .map(|(k, v)| (k, v.into_iter().map(|(s, l, _)| (s, l)).collect()))
        .collect()
}

/// Extracts `(source_expression, one_based_line, byte_span)` for each element expression inside
/// `facts { ["group"] { ... } }`.
pub fn extract_fact_spans(source: &str) -> BTreeMap<String, Vec<(String, usize, Span)>> {
    let mut map = BTreeMap::new();
    let Ok(parsed) = crate::parser::parse_module(source) else {
        return map;
    };
    let source_text = SourceText::from(source);
    let index = source_text.index();
    for member in &parsed.members {
        if member.name.as_str() != Some("facts") {
            continue;
        }
        collect_section_groups_from_expr(&member.expr, source, &index, &mut map);
    }
    map
}

/// Extracts `(group_line, Vec<element_line>)` for each group in `examples { ["group"] { ... } }`
/// (works on both test modules and generated `*-expected.pcf` / `*-actual.pcf` files).
pub fn extract_example_locations(source: &str) -> BTreeMap<String, (usize, Vec<usize>)> {
    let mut map = BTreeMap::new();
    let Ok(parsed) = crate::parser::parse_module(source) else {
        return map;
    };
    let source_text = SourceText::from(source);
    let index = source_text.index();
    for member in &parsed.members {
        if member.name.as_str() != Some("examples") {
            continue;
        }
        collect_example_group_lines(&member.expr, &index, &mut map);
    }
    map
}

fn collect_example_group_lines(
    expr: &crate::syntax::Expr,
    index: &crate::source::SourceTextIndex,
    out: &mut BTreeMap<String, (usize, Vec<usize>)>,
) {
    let members: &[ObjectMember] = match &expr.kind {
        ExprKind::Object {
            parent, members, ..
        } => {
            if let Some(p) = parent {
                collect_example_group_lines(p, index, out);
            }
            members
        }
        ExprKind::New { members, .. } => members,
        ExprKind::Parenthesized(inner) => {
            collect_example_group_lines(inner, index, out);
            return;
        }
        _ => return,
    };
    for m in members {
        match m {
            ObjectMember::Entry {
                key, value, span, ..
            } => {
                if let ExprKind::String(group_name) = &key.kind
                    && let Some(group) = group_name.as_str()
                {
                    let group_line = index.position(span.start).map_or(1, |pos| pos.line);
                    let entry = out
                        .entry(group.to_string())
                        .or_insert_with(|| (group_line, Vec::new()));
                    collect_element_lines(value, index, &mut entry.1);
                }
            }
            ObjectMember::Property(prop) => {
                if let Some(group) = prop.name.as_str() {
                    let group_line = index.position(prop.span.start).map_or(1, |pos| pos.line);
                    let entry = out
                        .entry(group.to_string())
                        .or_insert_with(|| (group_line, Vec::new()));
                    collect_element_lines(&prop.expr, index, &mut entry.1);
                }
            }
            _ => {}
        }
    }
}

fn collect_element_lines(
    expr: &crate::syntax::Expr,
    index: &crate::source::SourceTextIndex,
    out: &mut Vec<usize>,
) {
    let members: &[ObjectMember] = match &expr.kind {
        ExprKind::Object {
            parent, members, ..
        } => {
            if let Some(p) = parent {
                collect_element_lines(p, index, out);
            }
            members
        }
        ExprKind::New { members, .. } => members,
        ExprKind::Parenthesized(inner) => {
            collect_element_lines(inner, index, out);
            return;
        }
        _ => return,
    };
    for m in members {
        if let ObjectMember::Element { value, .. } = m {
            let line = index.position(value.span.start).map_or(1, |pos| pos.line);
            out.push(line);
        }
    }
}

fn collect_section_groups_from_expr(
    expr: &crate::syntax::Expr,
    source: &str,
    index: &crate::source::SourceTextIndex,
    out: &mut BTreeMap<String, Vec<(String, usize, Span)>>,
) {
    let members: &[ObjectMember] = match &expr.kind {
        ExprKind::Object {
            parent, members, ..
        } => {
            if let Some(p) = parent {
                collect_section_groups_from_expr(p, source, index, out);
            }
            members
        }
        ExprKind::New { members, .. } => members,
        ExprKind::Parenthesized(inner) => {
            collect_section_groups_from_expr(inner, source, index, out);
            return;
        }
        _ => return,
    };
    for m in members {
        if let ObjectMember::Entry { key, value, .. } = m
            && let ExprKind::String(group_name) = &key.kind
            && let Some(group) = group_name.as_str()
        {
            let entry = out.entry(group.to_string()).or_default();
            collect_fact_elements_from_expr(value, source, index, entry);
        }
    }
}

fn collect_fact_elements_from_expr(
    expr: &crate::syntax::Expr,
    source: &str,
    index: &crate::source::SourceTextIndex,
    out: &mut Vec<(String, usize, Span)>,
) {
    let members: &[ObjectMember] = match &expr.kind {
        ExprKind::Object {
            parent, members, ..
        } => {
            if let Some(p) = parent {
                collect_fact_elements_from_expr(p, source, index, out);
            }
            members
        }
        ExprKind::New { members, .. } => members,
        ExprKind::Parenthesized(inner) => {
            collect_fact_elements_from_expr(inner, source, index, out);
            return;
        }
        _ => return,
    };
    for m in members {
        if let ObjectMember::Element { value, .. } = m
            && let Some(expr_src) = source.get(value.span.start..value.span.end)
        {
            let line = index.position(value.span.start).map_or(1, |pos| pos.line);
            out.push((expr_src.to_string(), line, value.span));
        }
    }
}

/// Formats a location `(<location>)` matching `TestRunner.appendLocation`.
pub fn append_location(sb: &mut AnsiStringBuilder, location: &str) {
    sb.append_scoped(theme::STACK_FRAME, |sb| {
        sb.append_char('(')
            .append_untrusted_str(location)
            .append_char(')');
    });
}

/// Formats a failing fact message (optionally with a power-assertion diagram),
/// matching `TestRunner.factFailure`.
pub fn format_fact_failure(expr_text: &str, location: &str, use_color: bool) -> TestFailure {
    let mut sb = AnsiStringBuilder::new(use_color);
    sb.append_str(expr_text).append_char(' ');
    append_location(&mut sb, location);
    TestFailure {
        kind: "Fact Failure".to_string(),
        message: sb.finish_string(),
    }
}

/// Formats a failing fact with captured power-assertion observations,
/// matching `TestRunner.factFailure` + `PowerAssertions.render`.
pub fn format_fact_failure_with_power_assertions(
    source: &crate::source::Source,
    body_span: Span,
    observations: &[crate::diagnostic::PowerAssertionValue],
    location: &str,
    use_color: bool,
) -> TestFailure {
    let mut sb = AnsiStringBuilder::new(use_color);
    let suffix = |out: &mut AnsiStringBuilder| {
        out.append_char(' ');
        append_location(out, location);
    };
    crate::power_assertions::render_to_ansi(
        &mut sb,
        source,
        body_span,
        observations,
        "",
        Some(&suffix),
    );
    TestFailure {
        kind: "Fact Failure".to_string(),
        message: sb.finish_string(),
    }
}

/// Formats an example value mismatch failure matching `TestRunner.exampleFailure`.
pub fn format_example_failure(
    location: &str,
    expected_location: &str,
    expected_value: &str,
    actual_location: &str,
    actual_value: &str,
    example_number: usize,
    use_color: bool,
) -> TestFailure {
    let mut sb = AnsiStringBuilder::new(use_color);
    sb.append_styled(theme::TEST_NAME, &format!("#{example_number}: "));
    sb.append_scoped(theme::TEST_FAILURE_MESSAGE, |sb| {
        append_location(sb, location);
        sb.append_str("\n  Expected: ");
        append_location(sb, expected_location);
        sb.append_str("\n  ");
        sb.append_styled(
            theme::TEST_EXAMPLE_OUTPUT,
            &expected_value.replace('\n', "\n  "),
        );
        sb.append_str("\n  Actual: ");
        append_location(sb, actual_location);
        sb.append_str("\n  ");
        sb.append_styled(
            theme::TEST_EXAMPLE_OUTPUT,
            &actual_value.replace('\n', "\n  "),
        );
    });
    TestFailure {
        kind: "Example Failure".to_string(),
        message: sb.finish_string(),
    }
}

/// Formats an example length mismatch failure matching `TestRunner.exampleLengthMismatchFailure`.
pub fn format_example_length_mismatch(
    location: &str,
    property: &str,
    expected_len: usize,
    actual_len: usize,
    use_color: bool,
) -> TestFailure {
    let mut sb = AnsiStringBuilder::new(use_color);
    append_location(&mut sb, location);
    sb.append_char('\n')
        .append_scoped(theme::TEST_FAILURE_MESSAGE, |sb| {
            sb.append_str("Output mismatch: Expected \"")
                .append_str(property)
                .append_str("\" to contain ")
                .append_usize(expected_len)
                .append_str(" examples, but found ")
                .append_usize(actual_len);
        });
    TestFailure {
        kind: "Output Mismatch (Length)".to_string(),
        message: sb.finish_string(),
    }
}

/// Formats a missing example group failure matching `TestRunner.examplePropertyMismatchFailure`.
pub fn format_example_property_mismatch(
    location: &str,
    property: &str,
    is_missing_in_expected: bool,
    use_color: bool,
) -> TestFailure {
    let (exists_in, missing_in) = if is_missing_in_expected {
        ("actual", "expected")
    } else {
        ("expected", "actual")
    };
    let mut sb = AnsiStringBuilder::new(use_color);
    append_location(&mut sb, location);
    sb.append_char('\n')
        .append_scoped(theme::TEST_FAILURE_MESSAGE, |sb| {
            sb.append_str("Output mismatch: \"")
                .append_str(property)
                .append_str("\" exists in ")
                .append_str(exists_in)
                .append_str(" but not in ")
                .append_str(missing_in)
                .append_str(" output");
        });
    TestFailure {
        kind: "Output Mismatch".to_string(),
        message: sb.finish_string(),
    }
}

/// Formats a newly-written example notice matching `TestRunner.writtenExampleOutputFailure`.
pub fn format_example_written(test_name: &str, location: &str, use_color: bool) -> TestFailure {
    let mut sb = AnsiStringBuilder::new(use_color);
    append_location(&mut sb, location);
    sb.append_styled(
        theme::TEST_FAILURE_MESSAGE,
        "\nWrote expected output for test ",
    )
    .append_str(test_name);
    TestFailure {
        kind: "Example Output Written".to_string(),
        message: sb.finish_string(),
    }
}

/// Evaluates `facts` decoded from `pkl-binary` (`Mapping<String, Listing<Boolean>>`)
/// into `TestSectionResults` (`TestRunner.java:86-136`).
pub fn evaluate_facts_decoded(
    facts_value: Option<&DecodedValue>,
    display_uri: &str,
) -> TestSectionResults {
    evaluate_facts_decoded_with_source(facts_value, display_uri, None)
}

/// Evaluates `facts` decoded from `pkl-binary` (`Mapping<String, Listing<Boolean>>`)
/// with optional module source text so failing facts render exact source expressions
/// and 1-based line numbers (`TestRunner.java:406-427`).
pub fn evaluate_facts_decoded_with_source(
    facts_value: Option<&DecodedValue>,
    display_uri: &str,
    source: Option<&str>,
) -> TestSectionResults {
    let Some(DecodedValue::Mapping(groups) | DecodedValue::Map(groups)) = facts_value else {
        return TestSectionResults::empty(TestSectionName::Facts);
    };
    let fact_locations = source.map(extract_fact_locations).unwrap_or_default();
    let mut results = Vec::with_capacity(groups.len());
    for (k, v) in groups {
        let name = k.to_java_key_string().as_str().unwrap_or("").to_string();
        let group_locs = fact_locations.get(&name);
        let mut total_asserts = 0usize;
        let mut failures = Vec::new();
        if let DecodedValue::Listing(items) | DecodedValue::List(items) = v {
            for (idx, item) in items.iter().enumerate() {
                total_asserts += 1;
                if matches!(item, DecodedValue::Bool(false)) {
                    let message = if let Some((expr_text, line)) =
                        group_locs.and_then(|locs| locs.get(idx))
                    {
                        format!("{expr_text} ({display_uri}, line {line})")
                    } else {
                        format!("fact #{idx} == false ({display_uri})")
                    };
                    failures.push(TestFailure {
                        kind: "Fact Failure".to_string(),
                        message,
                    });
                }
            }
        }
        results.push(TestResult {
            name,
            total_asserts,
            failures,
            errors: Vec::new(),
            is_example_written: false,
        });
    }
    TestSectionResults {
        name: TestSectionName::Facts,
        results,
    }
}

/// Evaluates rendered `examples` (`Vec<(group_name, Vec<pcf_value_str>)>`) against optional
/// `expected` (`Vec<(group_name, Vec<pcf_value_str>)>`) (`TestRunner.java:181-344`).
pub fn evaluate_examples_rendered(
    actual_groups: &[(String, Vec<String>)],
    expected_groups: Option<&[(String, Vec<String>)]>,
    display_uri: &str,
    expected_uri: &str,
    actual_uri: &str,
    full_actual_pcf_doc: Option<String>,
) -> (TestSectionResults, Option<String>, Option<String>) {
    evaluate_examples_rendered_detailed(
        actual_groups,
        expected_groups,
        display_uri,
        expected_uri,
        actual_uri,
        full_actual_pcf_doc,
        None,
        None,
        false,
        |uri, _| uri.to_string(),
    )
}

/// Full-fidelity `examples` evaluator with exact per-group and per-element line numbers
/// (reloading line numbers from the test module, expected `.pcf`, and generated actual `.pcf`)
/// and optional ANSI color codes (`TestRunner.java:181-344`).
#[allow(clippy::too_many_arguments)]
pub fn evaluate_examples_rendered_detailed(
    actual_groups: &[(String, Vec<String>)],
    expected_groups: Option<&[(String, Vec<String>)]>,
    display_uri: &str,
    expected_uri: &str,
    actual_uri: &str,
    full_actual_pcf_doc: Option<String>,
    test_source: Option<&str>,
    expected_source: Option<&str>,
    use_color: bool,
    format_location: impl Fn(&str, Option<usize>) -> String,
) -> (TestSectionResults, Option<String>, Option<String>) {
    if actual_groups.is_empty() {
        return (
            TestSectionResults::empty(TestSectionName::Examples),
            None,
            None,
        );
    }

    let test_locs = test_source
        .map(extract_example_locations)
        .unwrap_or_default();
    let expected_locs = expected_source
        .map(extract_example_locations)
        .unwrap_or_default();
    let actual_pcf_locs = full_actual_pcf_doc
        .as_deref()
        .map(extract_example_locations)
        .unwrap_or_default();

    let Some(expected) = expected_groups else {
        // Expected file does not exist -> write expected output (`doRunAndWriteExamples`)
        let results = actual_groups
            .iter()
            .map(|(name, _)| {
                let line = test_locs.get(name).map(|(l, _)| *l);
                let loc = format_location(display_uri, line);
                TestResult {
                    name: name.clone(),
                    total_asserts: 0,
                    failures: vec![format_example_written(name, &loc, use_color)],
                    errors: Vec::new(),
                    is_example_written: true,
                }
            })
            .collect();
        return (
            TestSectionResults {
                name: TestSectionName::Examples,
                results,
            },
            full_actual_pcf_doc,
            None,
        );
    };

    let mut results = Vec::new();
    let mut any_mismatch = false;

    for (name, actual_items) in actual_groups {
        let test_group_loc = test_locs.get(name);
        let group_loc = format_location(display_uri, test_group_loc.map(|(l, _)| *l));

        let Some((_, expected_items)) = expected.iter().find(|(k, _)| k == name) else {
            any_mismatch = true;
            results.push(TestResult {
                name: name.clone(),
                total_asserts: 1,
                failures: vec![format_example_property_mismatch(
                    &group_loc, name, true, use_color,
                )],
                errors: Vec::new(),
                is_example_written: false,
            });
            continue;
        };

        if actual_items.len() != expected_items.len() {
            any_mismatch = true;
            results.push(TestResult {
                name: name.clone(),
                total_asserts: 1,
                failures: vec![format_example_length_mismatch(
                    &group_loc,
                    name,
                    expected_items.len(),
                    actual_items.len(),
                    use_color,
                )],
                errors: Vec::new(),
                is_example_written: false,
            });
            continue;
        }

        let exp_group_loc = expected_locs.get(name);
        let act_group_loc = actual_pcf_locs.get(name);

        let mut total_asserts = 0usize;
        let mut failures = Vec::new();
        for (idx, (act, exp)) in actual_items.iter().zip(expected_items.iter()).enumerate() {
            let example_num = total_asserts;
            total_asserts += 1;
            if act != exp {
                any_mismatch = true;
                let item_loc = format_location(
                    display_uri,
                    test_group_loc.and_then(|(_, items)| items.get(idx).copied()),
                );
                let exp_loc = format_location(
                    expected_uri,
                    exp_group_loc.and_then(|(_, items)| items.get(idx).copied()),
                );
                let act_loc = format_location(
                    actual_uri,
                    act_group_loc.and_then(|(_, items)| items.get(idx).copied()),
                );
                failures.push(format_example_failure(
                    &item_loc,
                    &exp_loc,
                    exp,
                    &act_loc,
                    act,
                    example_num,
                    use_color,
                ));
            }
        }
        results.push(TestResult {
            name: name.clone(),
            total_asserts,
            failures,
            errors: Vec::new(),
            is_example_written: false,
        });
    }

    for (name, _) in expected {
        if !actual_groups.iter().any(|(k, _)| k == name) {
            any_mismatch = true;
            let exp_loc = format_location(expected_uri, expected_locs.get(name).map(|(l, _)| *l));
            results.push(TestResult {
                name: name.clone(),
                total_asserts: 1,
                failures: vec![format_example_property_mismatch(
                    &exp_loc, name, false, use_color,
                )],
                errors: Vec::new(),
                is_example_written: false,
            });
        }
    }

    let generated_actual = if any_mismatch {
        full_actual_pcf_doc
    } else {
        None
    };

    (
        TestSectionResults {
            name: TestSectionName::Examples,
            results,
        },
        None,
        generated_actual,
    )
}

/// Renders one module's test results matching `SpecReporter.java` (`useColor = false`).
pub fn render_spec_report(results: &TestResults) -> String {
    render_spec_report_colored(results, false)
}

/// Renders one module's test results matching `SpecReporter.java` with optional ANSI color.
pub fn render_spec_report_colored(results: &TestResults, use_color: bool) -> String {
    let mut out = AnsiStringBuilder::new(use_color);
    out.append_str("module ")
        .append_str(&results.module_name)
        .append_char('\n');

    if let Some(ref err) = results.error {
        append_padded_ansi(&mut out, &err.rendered, "  ");
        out.append_char('\n');
    } else {
        render_section_ansi(&mut out, &results.facts, &results.facts.results);
        render_section_ansi(&mut out, &results.examples, &results.examples.results);
    }
    out.finish_string()
}

/// Renders one module's test results matching `MinimalReporter.java` (failures/errors only).
pub fn render_minimal_report(results: &TestResults, use_color: bool) -> String {
    let mut out = AnsiStringBuilder::new(use_color);
    if let Some(ref err) = results.error {
        out.append_str("module ")
            .append_str(&results.module_name)
            .append_char('\n');
        append_padded_ansi(&mut out, &err.rendered, "  ");
        out.append_char('\n');
    } else {
        let fact_failures: Vec<TestResult> = results
            .facts
            .results
            .iter()
            .filter(|r| r.is_failure())
            .cloned()
            .collect();
        let example_failures: Vec<TestResult> = results
            .examples
            .results
            .iter()
            .filter(|r| r.is_failure())
            .cloned()
            .collect();
        if !fact_failures.is_empty() || !example_failures.is_empty() {
            out.append_str("module ")
                .append_str(&results.module_name)
                .append_char('\n');
            render_section_ansi(&mut out, &results.facts, &fact_failures);
            render_section_ansi(&mut out, &results.examples, &example_failures);
        }
    }
    out.finish_string()
}

/// Renders one module's test results using the selected reporter (`spec` or `minimal`).
pub fn render_report(reporter: TestReporterKind, results: &TestResults, use_color: bool) -> String {
    match reporter {
        TestReporterKind::Spec => render_spec_report_colored(results, use_color),
        TestReporterKind::Minimal => render_minimal_report(results, use_color),
    }
}

fn render_section_ansi(
    out: &mut AnsiStringBuilder,
    section: &TestSectionResults,
    items: &[TestResult],
) {
    if items.is_empty() {
        return;
    }
    out.append_str("  ")
        .append_str(section.name.as_str())
        .append_char('\n');
    for (idx, res) in items.iter().enumerate() {
        if idx > 0 {
            out.append_char('\n');
        }
        render_result_ansi(out, res);
    }
    out.append_char('\n');
}

fn render_result_ansi(out: &mut AnsiStringBuilder, result: &TestResult) {
    out.append_str("    ");
    if result.is_example_written {
        out.append_str("✍️ ").append_str(&result.name);
        return;
    }
    if result.is_failure() {
        out.append_styled(theme::FAILING_TEST_MARK, "✘ ");
    } else {
        out.append_styled(theme::PASSING_TEST_MARK, "✔ ");
    }
    out.append_styled(theme::TEST_NAME, &result.name);
    if result.is_failure() {
        let padding = "       ";
        out.append_char('\n');
        for (idx, failure) in result.failures.iter().enumerate() {
            if idx > 0 {
                out.append_char('\n');
            }
            append_padded_ansi(out, &failure.message, padding);
        }
        for (idx, error) in result.errors.iter().enumerate() {
            if idx > 0 {
                out.append_char('\n');
            }
            append_padded_ansi(out, &error.rendered, padding);
        }
    }
}

fn append_padded_ansi(out: &mut AnsiStringBuilder, text: &str, padding: &str) {
    for (idx, line) in text.lines().enumerate() {
        if idx > 0 {
            out.append_char('\n');
        }
        if !line.is_empty() {
            out.append_str(padding).append_str(line);
        }
    }
}

/// Renders the final summary line matching `BaseReporter.summarize` (`useColor = false`).
pub fn render_spec_summary(all_results: &[TestResults]) -> String {
    render_summary_colored(all_results, false)
}

/// Renders the final summary line matching `BaseReporter.summarize` with optional ANSI color.
pub fn render_summary_colored(all_results: &[TestResults], use_color: bool) -> String {
    let mut total_tests = 0usize;
    let mut total_failed_tests = 0usize;
    let mut total_asserts = 0usize;
    let mut total_failed_asserts = 0usize;
    let mut is_failed = false;
    let mut is_example_written_failure = true;

    for r in all_results {
        if !is_failed {
            is_failed = r.failed();
        }
        if r.failed() {
            is_example_written_failure =
                r.is_example_written_failure() && is_example_written_failure;
        }
        total_tests += r.total_tests();
        total_failed_tests += r.total_failures();
        total_asserts += r.total_asserts();
        total_failed_asserts += r.total_asserts_failed();
    }

    let mut out = AnsiStringBuilder::new(use_color);
    if is_failed && is_example_written_failure {
        out.append_usize(total_failed_tests)
            .append_str(" examples written");
    } else {
        append_stats_line_ansi(
            &mut out,
            "tests",
            total_tests,
            total_failed_tests,
            is_failed,
        );
        out.append_str(", ");
        append_stats_line_ansi(
            &mut out,
            "asserts",
            total_asserts,
            total_failed_asserts,
            is_failed,
        );
    }
    out.append_char('\n');
    out.finish_string()
}

fn append_stats_line_ansi(
    out: &mut AnsiStringBuilder,
    kind: &str,
    total: usize,
    failed: usize,
    is_failed: bool,
) {
    let passed = total.saturating_sub(failed);
    // Exact RoundingMode.DOWN to 1 decimal place (`BaseReporter.java:121-122`):
    // tenths = floor(1000 * passed / total)
    let pass_rate = if total == 0 {
        "0.0".to_string()
    } else {
        let tenths = (passed as u128 * 1000) / (total as u128);
        format!("{}.{}", tenths / 10, tenths % 10)
    };
    let pass_rate_line = format!("{pass_rate}% {kind} pass");
    let color = if is_failed {
        AnsiCode::Red
    } else {
        AnsiCode::Green
    };
    out.append_scoped(color, |sb| {
        sb.append_str(&pass_rate_line);
    });
    if is_failed {
        out.append_str(" [")
            .append_usize(failed)
            .append_char('/')
            .append_usize(total)
            .append_str(" failed]");
    } else {
        out.append_str(" [")
            .append_usize(passed)
            .append_str(" passed]");
    }
}

// =============================================================================
// JUnit XML Reporter (`JUnitReporter.java`)
// =============================================================================

/// Renders a single-module JUnit XML report matching `JUnitReporter.report`.
pub fn render_junit_report(results: &TestResults) -> String {
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    write_junit_testsuite(&mut out, results, 0);
    out.push('\n');
    out
}

/// Renders an aggregated JUnit XML report matching `JUnitReporter.summarize`.
pub fn render_junit_aggregate_report(suite_name: &str, all_results: &[TestResults]) -> String {
    let total_tests: usize = all_results.iter().map(TestResults::total_tests).sum();
    let total_failures: usize = all_results.iter().map(TestResults::total_failures).sum();
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str("<testsuites name=\"");
    push_xml_escaped(&mut out, suite_name);
    out.push_str(&format!(
        "\" tests=\"{total_tests}\" failures=\"{total_failures}\">"
    ));
    if !all_results.is_empty() {
        out.push('\n');
        for res in all_results {
            write_junit_testsuite(&mut out, res, 1);
            out.push('\n');
        }
    }
    out.push_str("</testsuites>\n");
    out
}

fn write_junit_testsuite(out: &mut String, results: &TestResults, depth: usize) {
    let indent = "    ".repeat(depth);
    let child_indent = "    ".repeat(depth + 1);
    let leaf_indent = "    ".repeat(depth + 2);

    if let Some(ref err) = results.error {
        out.push_str(&indent);
        out.push_str("<testsuite name=\"");
        push_xml_escaped(out, &results.module_name);
        out.push_str("\" tests=\"1\" failures=\"1\">\n");
        out.push_str(&child_indent);
        out.push_str("<testcase classname=\"");
        push_xml_escaped(out, &results.module_name);
        out.push_str("\" name=\"");
        push_xml_escaped(out, &results.module_name);
        out.push_str("\">\n");
        out.push_str(&leaf_indent);
        out.push_str("<error message=\"");
        push_xml_escaped(out, &err.message);
        out.push_str("\">");
        let body = strip_ansi_codes(&format!("\n{}", err.rendered));
        push_xml_escaped(out, &body);
        out.push_str("</error>\n");
        out.push_str(&child_indent);
        out.push_str("</testcase>\n");
        out.push_str(&indent);
        out.push_str("</testsuite>");
        return;
    }

    let has_logs = !results.logs.trim().is_empty();
    let has_children =
        !results.facts.results.is_empty() || !results.examples.results.is_empty() || has_logs;

    out.push_str(&indent);
    out.push_str("<testsuite name=\"");
    push_xml_escaped(out, &results.module_name);
    out.push_str(&format!(
        "\" tests=\"{}\" failures=\"{}\">",
        results.total_tests(),
        results.total_failures()
    ));

    if has_children {
        out.push('\n');
        for section in [&results.facts, &results.examples] {
            for res in &section.results {
                let classname = format!("{}.{}", results.module_name, section.name.as_str());
                out.push_str(&child_indent);
                out.push_str("<testcase classname=\"");
                push_xml_escaped(out, &classname);
                out.push_str("\" name=\"");
                push_xml_escaped(out, &res.name);
                out.push_str("\">");
                if res.failures.is_empty() && res.errors.is_empty() {
                    out.push_str("</testcase>\n");
                } else {
                    out.push('\n');
                    for fail in &res.failures {
                        out.push_str(&leaf_indent);
                        out.push_str("<failure message=\"");
                        push_xml_escaped(out, &fail.kind);
                        out.push_str("\">");
                        let body = strip_ansi_codes(&fail.message);
                        push_xml_escaped(out, &body);
                        out.push_str("</failure>\n");
                    }
                    for error in &res.errors {
                        out.push_str(&leaf_indent);
                        out.push_str("<error message=\"");
                        push_xml_escaped(out, &error.message);
                        out.push_str("\">");
                        let body = strip_ansi_codes(&error.rendered);
                        push_xml_escaped(out, &body);
                        out.push_str("</error>\n");
                    }
                    out.push_str(&child_indent);
                    out.push_str("</testcase>\n");
                }
            }
        }
        if has_logs {
            out.push_str(&child_indent);
            out.push_str("<system-err><![CDATA[");
            out.push_str(&results.logs.replace("]]>", "]]]]><![CDATA[>"));
            out.push_str("]]></system-err>\n");
        }
        out.push_str(&indent);
    }
    out.push_str("</testsuite>");
}

fn push_xml_escaped(out: &mut String, text: &str) {
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '&' => out.push_str("&amp;"),
            other => out.push(other),
        }
    }
}

/// Extracts `Vec<(group_name, Vec<String>)>` from a `pkl-binary`-decoded mapping of rendered PCF strings.
pub fn decode_rendered_examples_map(val: Option<&DecodedValue>) -> Vec<(String, Vec<String>)> {
    let Some(DecodedValue::Mapping(groups) | DecodedValue::Map(groups)) = val else {
        return Vec::new();
    };
    let mut out = Vec::with_capacity(groups.len());
    for (k, v) in groups {
        let name = k.to_java_key_string().as_str().unwrap_or("").to_string();
        let items = match v {
            DecodedValue::Listing(xs) | DecodedValue::List(xs) => xs
                .iter()
                .map(|x| match x {
                    DecodedValue::String(s) => s.as_str().unwrap_or("").to_string(),
                    other => format!("{other:?}"),
                })
                .collect(),
            _ => Vec::new(),
        };
        out.push((name, items));
    }
    out
}

/// Pkl expression that renders every `examples` element to its exact PCF comparison string
/// (`TestRunner.renderAsPcf`, L387-394: `new PcfRenderer { omitNullProperties = false }.renderValue(v)`
/// with `"new "` prefix for objects) and encodes the mapping as base64 `pkl-binary` via `output.renderer`
/// (requires `pkl.outputFormat = "pkl-binary"`).
pub const RENDER_EXAMPLES_PKL_BINARY_EXPR: &str = r#"
let (r = new PcfRenderer { omitNullProperties = false })
output.renderer.renderValue(
  if (examples == null) null
  else examples.toMap().mapValues((_, g) ->
    g.toList().map((v) ->
      let (s = r.renderValue(v))
      if (v is Typed || v is Dynamic || v is Listing || v is Mapping) "new " + s else s
    )
  )
).base64
"#;

/// Pkl expression that renders the full `*-expected.pcf` / `*-actual.pcf` document
/// (`TestRunner.writeExampleOutputs`, L346-357).
pub const RENDER_EXAMPLES_DOC_EXPR: &str = r#"
let (ex = examples)
if (ex == null) ""
else new PcfRenderer { omitNullProperties = false; useCustomStringDelimiters = true }.renderDocument(new Dynamic { examples = ex })
"#;
