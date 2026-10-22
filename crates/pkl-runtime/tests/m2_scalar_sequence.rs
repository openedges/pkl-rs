use pkl_runtime::evaluation::{EvaluationOptions, scalar_expressions_with_options};

#[test]
fn denied_default_host_call_unwinds_before_next_persistent_call() {
    let results = scalar_expressions_with_options(
        "first = read(\"life:unsupported\")\nsecond = 42\n",
        "file:///m2-platform.pkl",
        "m2-platform",
        &["first".into(), "second".into(), "first".into()],
        EvaluationOptions::default(),
    )
    .expect("module setup succeeds");
    // Pinned Java same-evaluator first/first/second/first capture: Stage38 sequence oracle.
    let expected = "–– Pkl Error ––\nRefusing to read resource `life:unsupported` because it does not match any entry in the resource allowlist (`--allowed-resources`).\n\n1 | first = read(\"life:unsupported\")\n            ^^^^^^^^^^^^^^^^^^^^^^^^\nat m2-platform#first (file:///m2-platform.pkl)\n\n1 | first\n    ^^^^^\nat  (repl:text)\n";
    assert_eq!(results[0], Err(expected.into()));
    assert_eq!(results[1], Ok("42".into()));
    assert_eq!(results[2], results[0]);
}

#[test]
fn setup_failure_aborts_without_manufactured_call_results() {
    let result = scalar_expressions_with_options(
        "class {",
        "file:///m2-platform.pkl",
        "m2-platform",
        &["first".into(), "second".into()],
        EvaluationOptions::default(),
    );
    assert!(result.is_err());
}
