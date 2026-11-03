//! Pinned standard-library source text retained for authored diagnostic frames.
//! Evaluation remains in the shared Rust engine.
use crate::value::RuntimeSource;
use std::sync::{Arc, OnceLock};

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

#[cfg(test)]
mod tests {
    #[test]
    fn bundled_xml_source_matches_the_pinned_checkout() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../external/pkl/stdlib/xml.pkl");
        assert_eq!(
            include_bytes!("stdlib/xml.pkl").as_slice(),
            std::fs::read(path).unwrap()
        );
    }

    #[test]
    fn bundled_jsonnet_source_matches_the_pinned_checkout() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../external/pkl/stdlib/jsonnet.pkl");
        assert_eq!(
            include_bytes!("stdlib/jsonnet.pkl").as_slice(),
            std::fs::read(path).unwrap()
        );
    }

    #[test]
    fn bundled_semver_source_matches_the_pinned_checkout() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../external/pkl/stdlib/semver.pkl");
        assert_eq!(
            include_bytes!("stdlib/semver.pkl").as_slice(),
            std::fs::read(path).unwrap()
        );
    }

    #[test]
    fn bundled_yaml_source_matches_the_pinned_checkout() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../external/pkl/stdlib/yaml.pkl");
        assert_eq!(
            include_bytes!("stdlib/yaml.pkl").as_slice(),
            std::fs::read(path).unwrap()
        );
    }

    #[test]
    fn bundled_math_source_matches_the_pinned_checkout() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../external/pkl/stdlib/math.pkl");
        assert_eq!(
            include_bytes!("stdlib/math.pkl").as_slice(),
            std::fs::read(path).unwrap()
        );
    }

    #[test]
    fn bundled_json_source_matches_the_pinned_checkout() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../external/pkl/stdlib/json.pkl");
        assert_eq!(
            include_bytes!("stdlib/json.pkl").as_slice(),
            std::fs::read(path).unwrap()
        );
    }
    #[test]
    fn bundled_base_source_matches_the_pinned_checkout() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../external/pkl/stdlib/base.pkl");
        assert_eq!(
            include_bytes!("stdlib/base.pkl").as_slice(),
            std::fs::read(path).expect("pinned base source is available")
        );
    }

    #[test]
    fn bundled_reference_source_matches_the_pinned_checkout() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../external/pkl/stdlib/ref.pkl");
        assert_eq!(
            include_bytes!("stdlib/ref.pkl").as_slice(),
            std::fs::read(path).expect("pinned reference source is available")
        );
    }
}
