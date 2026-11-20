//! Pinned standard-library source text retained for authored diagnostic frames.
//! Evaluation remains in the shared Rust engine.
use crate::value::RuntimeSource;
use std::sync::{Arc, OnceLock};

pub(crate) fn analyze() -> Arc<RuntimeSource> {
    static SOURCE: OnceLock<Arc<RuntimeSource>> = OnceLock::new();
    SOURCE
        .get_or_init(|| {
            Arc::new(RuntimeSource {
                text: include_str!("stdlib/analyze.pkl").into(),
                uri: "pkl:analyze".into(),
                module_name: "pkl.analyze".into(),
            })
        })
        .clone()
}

pub(crate) fn reference() -> Arc<RuntimeSource> {
    static SOURCE: OnceLock<Arc<RuntimeSource>> = OnceLock::new();
    SOURCE
        .get_or_init(|| {
            Arc::new(RuntimeSource {
                text: include_str!("stdlib/ref.pkl").into(),
                uri: "pkl:ref".into(),
                module_name: "pkl.ref".into(),
            })
        })
        .clone()
}

pub(crate) fn base() -> Arc<RuntimeSource> {
    static SOURCE: OnceLock<Arc<RuntimeSource>> = OnceLock::new();
    SOURCE
        .get_or_init(|| {
            Arc::new(RuntimeSource {
                text: include_str!("stdlib/base.pkl").into(),
                uri: "pkl:base".into(),
                module_name: "pkl.base".into(),
            })
        })
        .clone()
}

pub(crate) fn test() -> Arc<RuntimeSource> {
    static SOURCE: OnceLock<Arc<RuntimeSource>> = OnceLock::new();
    SOURCE
        .get_or_init(|| {
            Arc::new(RuntimeSource {
                text: include_str!("stdlib/test.pkl").into(),
                uri: "pkl:test".into(),
                module_name: "pkl.test".into(),
            })
        })
        .clone()
}

pub(crate) fn xml() -> Arc<RuntimeSource> {
    static SOURCE: OnceLock<Arc<RuntimeSource>> = OnceLock::new();
    SOURCE
        .get_or_init(|| {
            Arc::new(RuntimeSource {
                text: include_str!("stdlib/xml.pkl").into(),
                uri: "pkl:xml".into(),
                module_name: "pkl.xml".into(),
            })
        })
        .clone()
}

pub(crate) fn pklbinary() -> Arc<RuntimeSource> {
    static SOURCE: OnceLock<Arc<RuntimeSource>> = OnceLock::new();
    SOURCE
        .get_or_init(|| {
            Arc::new(RuntimeSource {
                text: include_str!("stdlib/pklbinary.pkl").into(),
                uri: "pkl:pklbinary".into(),
                module_name: "pkl.pklbinary".into(),
            })
        })
        .clone()
}

pub(crate) fn jsonnet() -> Arc<RuntimeSource> {
    static SOURCE: OnceLock<Arc<RuntimeSource>> = OnceLock::new();
    SOURCE
        .get_or_init(|| {
            Arc::new(RuntimeSource {
                text: include_str!("stdlib/jsonnet.pkl").into(),
                uri: "pkl:jsonnet".into(),
                module_name: "pkl.jsonnet".into(),
            })
        })
        .clone()
}

pub(crate) fn json() -> Arc<RuntimeSource> {
    static SOURCE: OnceLock<Arc<RuntimeSource>> = OnceLock::new();
    SOURCE
        .get_or_init(|| {
            Arc::new(RuntimeSource {
                text: include_str!("stdlib/json.pkl").into(),
                uri: "pkl:json".into(),
                module_name: "pkl.json".into(),
            })
        })
        .clone()
}

pub(crate) fn math() -> Arc<RuntimeSource> {
    static SOURCE: OnceLock<Arc<RuntimeSource>> = OnceLock::new();
    SOURCE
        .get_or_init(|| {
            Arc::new(RuntimeSource {
                text: include_str!("stdlib/math.pkl").into(),
                uri: "pkl:math".into(),
                module_name: "pkl.math".into(),
            })
        })
        .clone()
}

pub(crate) fn yaml() -> Arc<RuntimeSource> {
    static SOURCE: OnceLock<Arc<RuntimeSource>> = OnceLock::new();
    SOURCE
        .get_or_init(|| {
            Arc::new(RuntimeSource {
                text: include_str!("stdlib/yaml.pkl").into(),
                uri: "pkl:yaml".into(),
                module_name: "pkl.yaml".into(),
            })
        })
        .clone()
}

/// Exact pinned module; native bindings retain the authored declarations.
pub(crate) fn reflect() -> Arc<RuntimeSource> {
    static SOURCE: OnceLock<Arc<RuntimeSource>> = OnceLock::new();
    SOURCE
        .get_or_init(|| {
            Arc::new(RuntimeSource {
                text: include_str!("stdlib/reflect.pkl").into(),
                uri: "pkl:reflect".into(),
                module_name: "pkl.reflect".into(),
            })
        })
        .clone()
}

/// Exact pinned pure-Pkl module; methods and classes compile as authored.
pub(crate) fn semver() -> Arc<RuntimeSource> {
    static SOURCE: OnceLock<Arc<RuntimeSource>> = OnceLock::new();
    SOURCE
        .get_or_init(|| {
            Arc::new(RuntimeSource {
                text: include_str!("stdlib/semver.pkl").into(),
                uri: "pkl:semver".into(),
                module_name: "pkl.semver".into(),
            })
        })
        .clone()
}

/// Exact pinned pure-Pkl shell utilities; evaluated through ordinary VM frames.
pub(crate) fn shell() -> Arc<RuntimeSource> {
    static SOURCE: OnceLock<Arc<RuntimeSource>> = OnceLock::new();
    SOURCE
        .get_or_init(|| {
            Arc::new(RuntimeSource {
                text: include_str!("stdlib/shell.pkl").into(),
                uri: "pkl:shell".into(),
                module_name: "pkl.shell".into(),
            })
        })
        .clone()
}

/// Unmodified Pkl 0.32.0 authored module; externals use the shared renderer VM.
pub(crate) fn protobuf() -> Arc<RuntimeSource> {
    static SOURCE: OnceLock<Arc<RuntimeSource>> = OnceLock::new();
    SOURCE
        .get_or_init(|| {
            Arc::new(RuntimeSource {
                text: include_str!("stdlib/protobuf.pkl").into(),
                uri: "pkl:protobuf".into(),
                module_name: "pkl.protobuf".into(),
            })
        })
        .clone()
}

/// Exact pinned authored declarations; environment-dependent externals are separate bindings.
pub(crate) fn evaluator_settings() -> Arc<RuntimeSource> {
    static SOURCE: OnceLock<Arc<RuntimeSource>> = OnceLock::new();
    SOURCE
        .get_or_init(|| {
            Arc::new(RuntimeSource {
                text: include_str!("stdlib/EvaluatorSettings.pkl").into(),
                uri: "pkl:EvaluatorSettings".into(),
                module_name: "pkl.EvaluatorSettings".into(),
            })
        })
        .clone()
}

/// Exact pinned authored declarations; environment-dependent externals are separate bindings.
pub(crate) fn project() -> Arc<RuntimeSource> {
    static SOURCE: OnceLock<Arc<RuntimeSource>> = OnceLock::new();
    SOURCE
        .get_or_init(|| {
            Arc::new(RuntimeSource {
                text: include_str!("stdlib/Project.pkl").into(),
                uri: "pkl:Project".into(),
                module_name: "pkl.Project".into(),
            })
        })
        .clone()
}

/// Exact pinned authored declarations; environment-dependent externals are separate bindings.
pub(crate) fn platform() -> Arc<RuntimeSource> {
    static SOURCE: OnceLock<Arc<RuntimeSource>> = OnceLock::new();
    SOURCE
        .get_or_init(|| {
            Arc::new(RuntimeSource {
                text: include_str!("stdlib/platform.pkl").into(),
                uri: "pkl:platform".into(),
                module_name: "pkl.platform".into(),
            })
        })
        .clone()
}

/// Exact pinned authored declarations; environment-dependent externals are separate bindings.
pub(crate) fn release() -> Arc<RuntimeSource> {
    static SOURCE: OnceLock<Arc<RuntimeSource>> = OnceLock::new();
    SOURCE
        .get_or_init(|| {
            Arc::new(RuntimeSource {
                text: include_str!("stdlib/release.pkl").into(),
                uri: "pkl:release".into(),
                module_name: "pkl.release".into(),
            })
        })
        .clone()
}

/// Exact pinned authored declarations; Benchmark runner module.
pub(crate) fn benchmark() -> Arc<RuntimeSource> {
    static SOURCE: OnceLock<Arc<RuntimeSource>> = OnceLock::new();
    SOURCE
        .get_or_init(|| {
            Arc::new(RuntimeSource {
                text: include_str!("stdlib/Benchmark.pkl").into(),
                uri: "pkl:Benchmark".into(),
                module_name: "pkl.Benchmark".into(),
            })
        })
        .clone()
}

/// Exact pinned pure-Pkl CLI command schema; evaluated through ordinary VM frames.
pub(crate) fn command() -> Arc<RuntimeSource> {
    static SOURCE: OnceLock<Arc<RuntimeSource>> = OnceLock::new();
    SOURCE
        .get_or_init(|| {
            Arc::new(RuntimeSource {
                text: include_str!("stdlib/Command.pkl").into(),
                uri: "pkl:Command".into(),
                module_name: "pkl.Command".into(),
            })
        })
        .clone()
}

/// Exact pinned pure-Pkl Pkldoc package descriptor template.
pub(crate) fn doc_package_info() -> Arc<RuntimeSource> {
    static SOURCE: OnceLock<Arc<RuntimeSource>> = OnceLock::new();
    SOURCE
        .get_or_init(|| {
            Arc::new(RuntimeSource {
                text: include_str!("stdlib/DocPackageInfo.pkl").into(),
                uri: "pkl:DocPackageInfo".into(),
                module_name: "pkl.DocPackageInfo".into(),
            })
        })
        .clone()
}

/// Exact pinned pure-Pkl Pkldoc website descriptor template.
pub(crate) fn docsite_info() -> Arc<RuntimeSource> {
    static SOURCE: OnceLock<Arc<RuntimeSource>> = OnceLock::new();
    SOURCE
        .get_or_init(|| {
            Arc::new(RuntimeSource {
                text: include_str!("stdlib/DocsiteInfo.pkl").into(),
                uri: "pkl:DocsiteInfo".into(),
                module_name: "pkl.DocsiteInfo".into(),
            })
        })
        .clone()
}

/// Exact pinned pure-Pkl settings template.
pub(crate) fn settings() -> Arc<RuntimeSource> {
    static SOURCE: OnceLock<Arc<RuntimeSource>> = OnceLock::new();
    SOURCE
        .get_or_init(|| {
            Arc::new(RuntimeSource {
                text: include_str!("stdlib/settings.pkl").into(),
                uri: "pkl:settings".into(),
                module_name: "pkl.settings".into(),
            })
        })
        .clone()
}

#[cfg(test)]
mod tests {
    #[test]
    fn all_23_bundled_stdlib_sources_match_the_pinned_checkout() {
        let stdlib_dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../external/pkl/stdlib");
        let bundled: &[(&str, &[u8])] = &[
            ("analyze.pkl", include_bytes!("stdlib/analyze.pkl")),
            ("base.pkl", include_bytes!("stdlib/base.pkl")),
            ("Benchmark.pkl", include_bytes!("stdlib/Benchmark.pkl")),
            ("Command.pkl", include_bytes!("stdlib/Command.pkl")),
            (
                "DocPackageInfo.pkl",
                include_bytes!("stdlib/DocPackageInfo.pkl"),
            ),
            ("DocsiteInfo.pkl", include_bytes!("stdlib/DocsiteInfo.pkl")),
            (
                "EvaluatorSettings.pkl",
                include_bytes!("stdlib/EvaluatorSettings.pkl"),
            ),
            ("json.pkl", include_bytes!("stdlib/json.pkl")),
            ("jsonnet.pkl", include_bytes!("stdlib/jsonnet.pkl")),
            ("math.pkl", include_bytes!("stdlib/math.pkl")),
            ("pklbinary.pkl", include_bytes!("stdlib/pklbinary.pkl")),
            ("platform.pkl", include_bytes!("stdlib/platform.pkl")),
            ("Project.pkl", include_bytes!("stdlib/Project.pkl")),
            ("protobuf.pkl", include_bytes!("stdlib/protobuf.pkl")),
            ("ref.pkl", include_bytes!("stdlib/ref.pkl")),
            ("reflect.pkl", include_bytes!("stdlib/reflect.pkl")),
            ("release.pkl", include_bytes!("stdlib/release.pkl")),
            ("semver.pkl", include_bytes!("stdlib/semver.pkl")),
            ("settings.pkl", include_bytes!("stdlib/settings.pkl")),
            ("shell.pkl", include_bytes!("stdlib/shell.pkl")),
            ("test.pkl", include_bytes!("stdlib/test.pkl")),
            ("xml.pkl", include_bytes!("stdlib/xml.pkl")),
            ("yaml.pkl", include_bytes!("stdlib/yaml.pkl")),
        ];
        assert_eq!(bundled.len(), 23);
        let mut expected_modules: Vec<String> = bundled
            .iter()
            .map(|(file, _)| format!("pkl:{}", file.strip_suffix(".pkl").unwrap()))
            .collect();
        expected_modules.sort_by_key(|name| name.to_ascii_lowercase());
        let listed_modules: Vec<&str> = include_str!("stdlib/modules.txt").lines().collect();
        assert_eq!(
            listed_modules,
            expected_modules
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>()
        );
        for (file, bytes) in bundled {
            let path = stdlib_dir.join(file);
            assert_eq!(
                *bytes,
                std::fs::read(&path)
                    .unwrap_or_else(|e| panic!("pinned {file} is available: {e}"))
                    .as_slice(),
                "bundled stdlib/{file} drifted from external/pkl/stdlib/{file}"
            );
        }
        // Verify upstream stdlib contains no additional .pkl module beyond the 23 + doc-package-info.pkl.
        let mut upstream_pkl: Vec<String> = std::fs::read_dir(&stdlib_dir)
            .expect("pinned external/pkl/stdlib directory")
            .filter_map(|entry| {
                let name = entry.ok()?.file_name().into_string().ok()?;
                name.ends_with(".pkl").then_some(name)
            })
            .collect();
        upstream_pkl.sort();
        let mut expected_files: Vec<String> = bundled
            .iter()
            .map(|(file, _)| (*file).to_owned())
            .chain(std::iter::once("doc-package-info.pkl".to_owned()))
            .collect();
        expected_files.sort();
        assert_eq!(upstream_pkl, expected_files);
    }
}
