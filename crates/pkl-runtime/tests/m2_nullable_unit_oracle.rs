//! Exact Java key/lambda diagnostics, one test per case and power mode.
use pkl_runtime::evaluation::{EvaluationOptions, scalar_expression_with_options};
use std::{
    fs,
    path::{Path, PathBuf},
};
const CASES: [&str; 3] = [
    "nullable-positive",
    "nullable-negative-grouped",
    "nonnull-positive",
];
fn read(p: &Path) -> Vec<u8> {
    fs::read(p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}
fn text(p: &Path) -> String {
    String::from_utf8(read(p)).unwrap()
}
fn verify(case: &str, mode: &str, power_assertions: bool) {
    let root =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/m2-nullable-units");
    let folder = root.join(case);
    let source = text(&folder.join("module.pkl"));
    let uri = text(&folder.join("uri.txt"));
    let expression = text(&folder.join("expression.txt"));
    assert_eq!(uri, format!("file:///stage133-nullable-unit/{case}.pkl"));
    assert_eq!(expression, "output.text");
    let oracle = folder.join(mode);
    let (status, stdout, stderr) = match scalar_expression_with_options(
        &source,
        &uri,
        case,
        &expression,
        EvaluationOptions { power_assertions },
    ) {
        Ok(v) => (b"success\n".as_slice(), v, String::new()),
        Err(e) => (b"error\n".as_slice(), String::new(), e),
    };
    assert_eq!(
        read(&oracle.join("result.status.bin")),
        status,
        "{case}/{mode} status"
    );
    for (stem, actual) in [
        ("expression", expression.as_str()),
        ("result.stdout", stdout.as_str()),
        ("result.stderr", stderr.as_str()),
    ] {
        assert_eq!(
            read(&oracle.join(format!("{stem}.bin"))),
            actual.as_bytes(),
            "{case}/{mode}/{stem} UTF-8"
        );
        assert_eq!(
            read(&oracle.join(format!("{stem}.code-units.bin"))),
            actual
                .encode_utf16()
                .flat_map(u16::to_be_bytes)
                .collect::<Vec<_>>(),
            "{case}/{mode}/{stem} UTF-16"
        );
    }
    let _ = read(&oracle.join("result.error-class.bin"));
    let _ = read(&oracle.join("result.error-class.code-units.bin"));
}
#[test]
fn roster() {
    let root =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/m2-nullable-units");
    assert_eq!(
        text(&root.join("cases.txt")).lines().collect::<Vec<_>>(),
        CASES
    );
}
macro_rules! observations { ($(($on:ident,$off:ident,$case:literal)),+ $(,)?) => { $( #[test] fn $on(){verify($case,"on",true)} #[test] fn $off(){verify($case,"off",false)} )+ }; }
observations!(
    (case0_on, case0_off, "nullable-positive"),
    (case1_on, case1_off, "nullable-negative-grouped"),
    (case2_on, case2_off, "nonnull-positive"),
);
