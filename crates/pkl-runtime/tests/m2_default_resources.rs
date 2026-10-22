//! Complete diagnostics from the pinned Stage38 default-Host Java captures.
use pkl_runtime::evaluation::{
    EvaluationOptions, scalar_expression, scalar_expressions_with_options,
};

const URI: &str = "file:///m2-platform.pkl";

#[test]
fn noop_trace_resource_denial_keeps_the_read_frame_for_objects_and_strict_lists() {
    let expected = "–– Pkl Error ––\nRefusing to read resource `life:text` because it does not match any entry in the resource allowlist (`--allowed-resources`).\n\n1 | obj=new Dynamic { x=read(\"life:text\") }\n                        ^^^^^^^^^^^^^^^^^\nat m2-platform#obj.x (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n";
    for tail in ["result=trace(obj).x\n", "result=trace(List(obj)).length\n"] {
        let text = format!("obj=new Dynamic {{ x=read(\"life:text\") }}\n{tail}");
        assert_eq!(
            scalar_expression(&text, URI, "m2-platform", "result"),
            Err(expected.into())
        );
    }
}

#[test]
fn ordinary_read_keeps_its_caller_and_denied_members_retry_after_collection() {
    let text = "obj=new Dynamic { x=read(\"life:text\") }\nresult=obj.x\nstable=7\n";
    let expected = "–– Pkl Error ––\nRefusing to read resource `life:text` because it does not match any entry in the resource allowlist (`--allowed-resources`).\n\n1 | obj=new Dynamic { x=read(\"life:text\") }\n                        ^^^^^^^^^^^^^^^^^\nat m2-platform#obj.x (file:///m2-platform.pkl)\n\n2 | result=obj.x\n           ^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n";
    let calls = ["result".into(), "stable".into(), "result".into()];
    assert_eq!(
        scalar_expressions_with_options(
            text,
            URI,
            "m2-platform",
            &calls,
            EvaluationOptions::default()
        )
        .unwrap(),
        [Err(expected.into()), Ok("7".into()), Err(expected.into())]
    );
}

#[test]
fn read_operand_type_failure_precedes_uri_or_policy_processing() {
    let expected = "–– Pkl Error ––\nOperator `read` is not defined for operand type `Int`.\nOperand: 1\n\n1 | result=read(1)\n           ^^^^^^^\nat m2-platform#result (file:///m2-platform.pkl)\n\n1 | result\n    ^^^^^^\nat  (repl:text)\n";
    assert_eq!(
        scalar_expression("result=read(1)\n", URI, "m2-platform", "result"),
        Err(expected.into())
    );
}
