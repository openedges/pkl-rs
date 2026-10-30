//! Fourteen unchanged upstream modules bound to complete Stage67 Java CLI bytes.
#[path = "support/module_keys.rs"]
mod module_keys;
use pkl_runtime::evaluation::frontend::{
    DiagnosticProfile, ModuleEvalOptions, ModuleSourceInput, evaluate_module_output_with_host,
};
use pkl_runtime::modules::{
    ModuleCapabilities, ModuleCompletion, ModuleRequestKind, ModuleSource, ResolvedModule,
};
use std::{fs, path::PathBuf};

fn check(label: &str, source: &str, uri: &str, exit: i32, stdout: &[u8], stderr: &[u8]) {
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
    let helper_uri =
        include_str!("../../../tests/fixtures/m2-properties-renderer-originals/helper-uri.txt");
    let mut failures = Vec::new();
    for repeat in 0..2 {
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
                match &request.kind {
                ModuleRequestKind::Capabilities { .. } => ModuleCompletion::Capabilities(Ok(ModuleCapabilities { local: true, hierarchical: true, fragment_paths: false })),
                ModuleRequestKind::HasElement { element_uri, .. } => ModuleCompletion::HasElement(Ok(element_uri == helper_uri)),
                ModuleRequestKind::Resolve { original_uri, .. } if original_uri == helper_uri => ModuleCompletion::Resolved(Ok(ResolvedModule { original_uri: original_uri.clone(), resolved_uri: original_uri.clone(), source_uri: original_uri.clone() })),
                ModuleRequestKind::LoadSource { resolved, .. } if resolved.original_uri == helper_uri => ModuleCompletion::Source(Ok(ModuleSource { text: include_str!("../../../tests/fixtures/m2-properties-renderer-originals/annotationConverter.pkl").into(), inferred_name: "annotationConverter".into() })),
                other => panic!("unexpected original dependency {other:?}"),
            }
            },
        );
        if let Some(base) = std::env::var_os("PKL_FRAME_CAPTURE_DIR") {
            let out = PathBuf::from(base)
                .join("originals")
                .join(label)
                .join(format!("repeat-{repeat}"));
            fs::create_dir_all(&out).unwrap();
            fs::write(out.join("stdout.bin"), &result.stdout).unwrap();
            fs::write(out.join("stderr.bin"), &result.stderr).unwrap();
            fs::write(out.join("exit.txt"), format!("{}\n", result.exit_code)).unwrap();
        }
        let exact = result.exit_code == exit && result.stdout == stdout && result.stderr == stderr;
        eprintln!(
            "properties-original label={label} repeat={repeat} exact_match={exact} exit={}",
            result.exit_code
        );
        if !exact {
            failures.push(format!(
                "{label} repeat={repeat}: exit={} stdout={:?} stderr={:?}",
                result.exit_code, result.stdout, result.stderr
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
macro_rules! original {
    ($test:ident, $label:literal, $exit:literal) => {
        #[test]
        fn $test() {
            check(
                $label,
                include_str!(concat!(
                    "../../../tests/fixtures/m2-properties-renderer-originals/",
                    $label,
                    "/module.pkl"
                )),
                include_str!(concat!(
                    "../../../tests/fixtures/m2-properties-renderer-originals/",
                    $label,
                    "/uri.txt"
                )),
                $exit,
                include_bytes!(concat!(
                    "../../../tests/fixtures/m2-properties-renderer-originals/",
                    $label,
                    "/stdout.bin"
                )),
                include_bytes!(concat!(
                    "../../../tests/fixtures/m2-properties-renderer-originals/",
                    $label,
                    "/stderr.bin"
                )),
            );
        }
    };
}
original!(
    original_propertiesrenderer1_properties,
    "propertiesRenderer1.properties",
    0
);
original!(
    original_propertiesrenderer10_properties,
    "propertiesRenderer10.properties",
    1
);
original!(original_propertiesrenderer11, "propertiesRenderer11", 1);
original!(
    original_propertiesrenderer12_properties,
    "propertiesRenderer12.properties",
    0
);
original!(
    original_propertiesrenderer2_properties,
    "propertiesRenderer2.properties",
    0
);
original!(original_propertiesrenderer2b, "propertiesRenderer2b", 0);
original!(
    original_propertiesrenderer3_properties,
    "propertiesRenderer3.properties",
    0
);
original!(original_propertiesrenderer4, "propertiesRenderer4", 0);
original!(original_propertiesrenderer5, "propertiesRenderer5", 0);
original!(
    original_propertiesrenderer6_properties,
    "propertiesRenderer6.properties",
    0
);
original!(
    original_propertiesrenderer7_properties,
    "propertiesRenderer7.properties",
    0
);
original!(
    original_propertiesrenderer8_properties,
    "propertiesRenderer8.properties",
    0
);
original!(
    original_propertiesrenderer9_properties,
    "propertiesRenderer9.properties",
    0
);
original!(
    original_propertiesrendererbug66849708_properties,
    "propertiesRendererBug66849708.properties",
    0
);
