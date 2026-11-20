//! Minimal Wasm-pure `pkl:test` runner & `SpecReporter` formatter needed for
//! `ProjectPackager` `apiTests` and Pantry `project-test` verification.
//!
//! Full CLI `pkl test` options (`--junit-reports`, `minimal` reporter, power assertion
//! sub-expression diagrams) remain M5 scope.
use crate::pkl_binary_decode::DecodedValue;
use crate::source::SourceText;
use crate::syntax::{ExprKind, ObjectMember};
use std::collections::BTreeMap;

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
        collect_fact_groups_from_expr(&member.expr, source, &index, &mut map);
    }
    map
}

fn collect_fact_groups_from_expr(
    expr: &crate::syntax::Expr,
    source: &str,
    index: &crate::source::SourceTextIndex,
    out: &mut BTreeMap<String, Vec<(String, usize)>>,
) {
    let members: &[ObjectMember] = match &expr.kind {
        ExprKind::Object {
            parent, members, ..
        } => {
            if let Some(p) = parent {
                collect_fact_groups_from_expr(p, source, index, out);
            }
            members
        }
        ExprKind::New { members, .. } => members,
        ExprKind::Parenthesized(inner) => {
            collect_fact_groups_from_expr(inner, source, index, out);
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
    out: &mut Vec<(String, usize)>,
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
            out.push((expr_src.to_string(), line));
        }
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
    if actual_groups.is_empty() {
        return (
            TestSectionResults::empty(TestSectionName::Examples),
            None,
            None,
        );
    }

    let Some(expected) = expected_groups else {
        // Expected file does not exist -> write expected output (`doRunAndWriteExamples`)
        let results = actual_groups
            .iter()
            .map(|(name, _)| TestResult {
                name: name.clone(),
                total_asserts: 0,
                failures: vec![TestFailure {
                    kind: "Example Output Written".to_string(),
                    message: format!("({display_uri})\nWrote expected output for test {name}"),
                }],
                errors: Vec::new(),
                is_example_written: true,
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
        let Some((_, expected_items)) = expected.iter().find(|(k, _)| k == name) else {
            any_mismatch = true;
            results.push(TestResult {
                name: name.clone(),
                total_asserts: 1,
                failures: vec![TestFailure {
                    kind: "Output Mismatch".to_string(),
                    message: format!(
                        "({display_uri})\nOutput mismatch: \"{name}\" exists in actual but not in expected output"
                    ),
                }],
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
                failures: vec![TestFailure {
                    kind: "Output Mismatch (Length)".to_string(),
                    message: format!(
                        "({display_uri})\nOutput mismatch: Expected \"{name}\" to contain {} examples, but found {}",
                        expected_items.len(),
                        actual_items.len()
                    ),
                }],
                errors: Vec::new(),
                is_example_written: false,
            });
            continue;
        }

        let mut total_asserts = 0usize;
        let mut failures = Vec::new();
        for (idx, (act, exp)) in actual_items.iter().zip(expected_items.iter()).enumerate() {
            total_asserts += 1;
            if act != exp {
                any_mismatch = true;
                let exp_indented = exp.replace('\n', "\n  ");
                let act_indented = act.replace('\n', "\n  ");
                failures.push(TestFailure {
                    kind: "Example Failure".to_string(),
                    message: format!(
                        "#{idx}: ({display_uri})\n  Expected: ({expected_uri})\n  {exp_indented}\n  Actual: ({actual_uri})\n  {act_indented}"
                    ),
                });
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
            results.push(TestResult {
                name: name.clone(),
                total_asserts: 1,
                failures: vec![TestFailure {
                    kind: "Output Mismatch".to_string(),
                    message: format!(
                        "({expected_uri})\nOutput mismatch: \"{name}\" exists in expected but not in actual output"
                    ),
                }],
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
    let mut out = String::new();
    out.push_str("module ");
    out.push_str(&results.module_name);
    out.push('\n');

    if let Some(ref err) = results.error {
        append_padded(&mut out, &err.rendered, "  ");
        out.push('\n');
    } else {
        render_section(&mut out, &results.facts);
        render_section(&mut out, &results.examples);
    }
    out
}

fn render_section(out: &mut String, section: &TestSectionResults) {
    if section.results.is_empty() {
        return;
    }
    out.push_str("  ");
    out.push_str(section.name.as_str());
    out.push('\n');
    for (idx, res) in section.results.iter().enumerate() {
        if idx > 0 {
            out.push('\n');
        }
        render_result(out, res);
    }
    out.push('\n');
}

fn render_result(out: &mut String, result: &TestResult) {
    out.push_str("    ");
    if result.is_example_written {
        out.push_str("✍️ ");
        out.push_str(&result.name);
        return;
    }
    if result.is_failure() {
        out.push_str("✘ ");
    } else {
        out.push_str("✔ ");
    }
    out.push_str(&result.name);
    if result.is_failure() {
        let padding = "       ";
        out.push('\n');
        for (idx, failure) in result.failures.iter().enumerate() {
            if idx > 0 {
                out.push('\n');
            }
            append_padded(out, &failure.message, padding);
        }
        for (idx, error) in result.errors.iter().enumerate() {
            if idx > 0 || !result.failures.is_empty() {
                out.push('\n');
            }
            append_padded(out, &error.rendered, padding);
        }
    }
}

fn append_padded(out: &mut String, text: &str, padding: &str) {
    for (idx, line) in text.lines().enumerate() {
        if idx > 0 {
            out.push('\n');
        }
        if !line.is_empty() {
            out.push_str(padding);
            out.push_str(line);
        }
    }
}

/// Renders the final summary line matching `BaseReporter.summarize` (`BaseReporter.java:43-72,113-134`).
pub fn render_spec_summary(all_results: &[TestResults]) -> String {
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

    let mut out = String::new();
    if is_failed && is_example_written_failure {
        use std::fmt::Write;
        let _ = writeln!(&mut out, "{total_failed_tests} examples written");
    } else {
        append_stats_line(
            &mut out,
            "tests",
            total_tests,
            total_failed_tests,
            is_failed,
        );
        out.push_str(", ");
        append_stats_line(
            &mut out,
            "asserts",
            total_asserts,
            total_failed_asserts,
            is_failed,
        );
        out.push('\n');
    }
    out
}

fn append_stats_line(out: &mut String, kind: &str, total: usize, failed: usize, is_failed: bool) {
    let passed = total.saturating_sub(failed);
    // Exact RoundingMode.DOWN to 1 decimal place (`BaseReporter.java:121-122`):
    // tenths = floor(1000 * passed / total)
    let pass_rate = if total == 0 {
        "0.0".to_string()
    } else {
        let tenths = (passed as u128 * 1000) / (total as u128);
        format!("{}.{}", tenths / 10, tenths % 10)
    };
    use std::fmt::Write;
    let _ = write!(out, "{pass_rate}% {kind} pass");
    if is_failed {
        let _ = write!(out, " [{failed}/{total} failed]");
    } else {
        let _ = write!(out, " [{passed} passed]");
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
