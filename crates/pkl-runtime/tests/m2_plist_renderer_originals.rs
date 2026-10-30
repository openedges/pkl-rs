//! Nine unchanged upstream Plist renderer modules bound to complete Stage67 Java CLI bytes.
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
        .join("../../tests/fixtures/m2-plist-renderer-originals");
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
            "plist-original label={label} repeat={repeat} exact_match={exact} exit={}",
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
                    "../../../tests/fixtures/m2-plist-renderer-originals/",
                    $label,
                    "/module.pkl"
                )),
                include_str!(concat!(
                    "../../../tests/fixtures/m2-plist-renderer-originals/",
                    $label,
                    "/uri.txt"
                )),
                $exit,
                include_bytes!(concat!(
                    "../../../tests/fixtures/m2-plist-renderer-originals/",
                    $label,
                    "/stdout.bin"
                )),
                include_bytes!(concat!(
                    "../../../tests/fixtures/m2-plist-renderer-originals/",
                    $label,
                    "/stderr.bin"
                )),
            );
        }
    };
}
original!(java_plistrenderer1_plist, "pListRenderer1.plist", 0);

original!(java_plistrenderer2_plist, "pListRenderer2.plist", 0);

original!(java_plistrenderer3_plist, "pListRenderer3.plist", 0);

original!(java_plistrenderer4, "pListRenderer4", 0);

original!(java_plistrenderer5, "pListRenderer5", 0);

original!(java_plistrenderer6_plist, "pListRenderer6.plist", 0);

original!(java_plistrenderer7, "pListRenderer7", 1);

original!(java_plistrenderer8_plist, "pListRenderer8.plist", 0);

original!(java_plistrenderer2b, "plistRenderer2b", 0);
