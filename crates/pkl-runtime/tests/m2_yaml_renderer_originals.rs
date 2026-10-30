//! Twenty-three unchanged upstream YAML renderer modules bound to complete Stage67 Java CLI bytes.
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
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/m2-yaml-renderer-originals");
    let modules = fs::read_to_string(root.join("modules.tsv")).unwrap();
    let modules = modules
        .lines()
        .map(|line| line.split('\t').collect::<Vec<_>>())
        .collect::<Vec<_>>();
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
                    ModuleRequestKind::Capabilities { .. } => {
                        ModuleCompletion::Capabilities(Ok(ModuleCapabilities {
                            local: true,
                            hierarchical: true,
                            fragment_paths: false,
                        }))
                    }
                    ModuleRequestKind::HasElement { element_uri, .. } => {
                        ModuleCompletion::HasElement(Ok(modules
                            .iter()
                            .any(|row| element_uri == row[0])))
                    }
                    ModuleRequestKind::Resolve { original_uri, .. }
                        if modules.iter().any(|row| original_uri == row[0]) =>
                    {
                        ModuleCompletion::Resolved(Ok(ResolvedModule {
                            original_uri: original_uri.clone(),
                            resolved_uri: original_uri.clone(),
                            source_uri: original_uri.clone(),
                        }))
                    }
                    ModuleRequestKind::LoadSource { resolved, .. }
                        if modules.iter().any(|row| resolved.original_uri == row[0]) =>
                    {
                        let row = modules
                            .iter()
                            .find(|row| resolved.original_uri == row[0])
                            .unwrap();
                        ModuleCompletion::Source(Ok(ModuleSource {
                            text: fs::read_to_string(root.join(row[1])).unwrap().into(),
                            inferred_name: row[2].into(),
                        }))
                    }
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
            "yaml-original label={label} repeat={repeat} exact_match={exact} exit={}",
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
                    "../../../tests/fixtures/m2-yaml-renderer-originals/",
                    $label,
                    "/module.pkl"
                )),
                include_str!(concat!(
                    "../../../tests/fixtures/m2-yaml-renderer-originals/",
                    $label,
                    "/uri.txt"
                )),
                $exit,
                include_bytes!(concat!(
                    "../../../tests/fixtures/m2-yaml-renderer-originals/",
                    $label,
                    "/stdout.bin"
                )),
                include_bytes!(concat!(
                    "../../../tests/fixtures/m2-yaml-renderer-originals/",
                    $label,
                    "/stderr.bin"
                )),
            );
        }
    };
}
original!(original_yamlrenderer1_yml, "yamlRenderer1.yml", 0);
original!(original_yamlrenderer10_yml, "yamlRenderer10.yml", 0);
original!(original_yamlrenderer2_yml, "yamlRenderer2.yml", 0);
original!(original_yamlrenderer2b, "yamlRenderer2b", 0);
original!(original_yamlrenderer3_yml, "yamlRenderer3.yml", 0);
original!(original_yamlrenderer4, "yamlRenderer4", 0);
original!(original_yamlrenderer5, "yamlRenderer5", 0);
original!(original_yamlrenderer6_yml, "yamlRenderer6.yml", 0);
original!(original_yamlrenderer7, "yamlRenderer7", 1);
original!(original_yamlrenderer8_yml, "yamlRenderer8.yml", 0);
original!(original_yamlrenderer9_yml, "yamlRenderer9.yml", 0);
original!(
    original_yamlrendererbug66849708_yml,
    "yamlRendererBug66849708.yml",
    0
);
original!(original_yamlrendererempty_yml, "yamlRendererEmpty.yml", 0);
original!(
    original_yamlrendererindentationwidth2_yml,
    "yamlRendererIndentationWidth2.yml",
    0
);
original!(
    original_yamlrendererindentationwidth4_yml,
    "yamlRendererIndentationWidth4.yml",
    0
);
original!(
    original_yamlrendererindentationwidth5_yml,
    "yamlRendererIndentationWidth5.yml",
    0
);
original!(original_yamlrendererkeys_yml, "yamlRendererKeys.yml", 0);
original!(original_yamlrendererstream1, "yamlRendererStream1", 0);
original!(original_yamlrendererstream2, "yamlRendererStream2", 0);
original!(
    original_yamlrendererstrings_yml,
    "yamlRendererStrings.yml",
    0
);
original!(
    original_yamlrendererstringscompat,
    "yamlRendererStringsCompat",
    0
);
original!(
    original_yamlrendererstringsyaml11,
    "yamlRendererStringsYaml11",
    0
);
original!(
    original_yamlrendererstringsyaml12,
    "yamlRendererStringsYaml12",
    0
);
