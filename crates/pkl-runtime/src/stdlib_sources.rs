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

#[cfg(test)]
mod tests {
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
