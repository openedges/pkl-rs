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
                uri: "https://github.com/apple/pkl/blob/0.32.0/stdlib/ref.pkl".into(),
                module_name: "pkl.ref".into(),
                line_anchors: true,
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
                uri: "https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl".into(),
                module_name: "pkl.base".into(),
                line_anchors: true,
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
                uri: "https://github.com/apple/pkl/blob/0.32.0/stdlib/test.pkl".into(),
                module_name: "pkl.test".into(),
                line_anchors: true,
            })
        })
        .clone()
}

#[cfg(test)]
mod tests {
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
