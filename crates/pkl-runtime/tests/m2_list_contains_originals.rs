//! Two unchanged upstream power-assertion modules; complete recorded Java CLI stdout/stderr.
#[path = "support/module_keys.rs"]
mod module_keys;

use pkl_runtime::evaluation::frontend::{
    DiagnosticProfile, ModuleEvalOptions, ModuleSourceInput, evaluate_module_output_with_host,
};

fn check(name: &str, source: &str, uri: &str, stdout: &[u8], stderr: &[u8]) {
    let options = ModuleEvalOptions {
        no_cache: true,
        no_project: true,
        settings: "pkl:settings".into(),
        properties: vec![
            ("name1".into(), "value1".into()),
            ("name2".into(), "value2".into()),
            ("/foo/bar".into(), "foobar".into()),
        ],
        environment: vec![
            ("NAME1".into(), "value1".into()),
            ("NAME2".into(), "value2".into()),
            ("/foo/bar".into(), "foobar".into()),
            ("foo bar".into(), "foo bar".into()),
            ("file:///foo/bar".into(), "file:///foo/bar".into()),
        ],
        diagnostic_profile: DiagnosticProfile::CliBundledSettings,
    };
    for attempt in 0..2 {
        let result = evaluate_module_output_with_host(
            ModuleSourceInput {
                text: source,
                uri,
                inferred_name: uri
                    .rsplit('/')
                    .next()
                    .unwrap()
                    .strip_suffix(".pkl")
                    .unwrap(),
            },
            &options,
            |request| {
                if let Some(completion) = module_keys::defaults(request) {
                    return completion;
                }
                panic!("unexpected module request: {:?}", request.kind);
            },
        );
        assert_eq!(
            result.exit_code, 1,
            "{name} attempt {attempt}: {:?}",
            result.stderr
        );
        assert_eq!(
            result.stdout, stdout,
            "{name} complete stdout, attempt {attempt}"
        );
        assert_eq!(
            result.stderr, stderr,
            "{name} complete stderr, attempt {attempt}"
        );
    }
}

macro_rules! original {
    ($test:ident, $name:literal) => {
        #[test]
        fn $test() {
            check(
                $name,
                include_str!(concat!(
                    "../../../tests/fixtures/m2-list-contains-originals/",
                    $name,
                    "/module.pkl"
                )),
                include_str!(concat!(
                    "../../../tests/fixtures/m2-list-contains-originals/",
                    $name,
                    "/uri.txt"
                )),
                include_bytes!(concat!(
                    "../../../tests/fixtures/m2-list-contains-originals/",
                    $name,
                    "/stdout.bin"
                )),
                include_bytes!(concat!(
                    "../../../tests/fixtures/m2-list-contains-originals/",
                    $name,
                    "/stderr.bin"
                )),
            );
        }
    };
}

original!(type_constraints9, "type_constraints9");
original!(type_constraints9a, "type_constraints9a");
