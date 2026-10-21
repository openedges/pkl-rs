use pkl_runtime::evaluation::{EvaluationOptions, scalar_expressions_with_options};

#[test]
fn unsupported_host_call_unwinds_before_next_persistent_call() {
    let results = scalar_expressions_with_options(
        "first = read(\"life:unsupported\")\nsecond = 42\n",
        "file:///m2-platform.pkl",
        "m2-platform",
        &["first".into(), "second".into(), "first".into()],
        EvaluationOptions::default(),
    )
    .expect("module setup succeeds");
    assert!(
        results[0]
            .as_ref()
            .unwrap_err()
            .contains("does not supply a Host")
    );
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
