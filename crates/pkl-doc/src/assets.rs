//! All 17 embedded static web resources verbatim from Apple Pkl 0.32.0
//! `pkl-doc/src/main/resources/org/pkl/doc/**`.

use std::fs;
use std::io;
use std::path::Path;

/// All 17 static assets in exact `HtmlGenerator.kt:89-107` order.
pub const STATIC_RESOURCES: &[(&str, &[u8])] = &[
    (
        "fonts/lato-v14-latin_latin-ext-regular.woff2",
        include_bytes!("../assets/fonts/lato-v14-latin_latin-ext-regular.woff2"),
    ),
    (
        "fonts/lato-v14-latin_latin-ext-700.woff2",
        include_bytes!("../assets/fonts/lato-v14-latin_latin-ext-700.woff2"),
    ),
    (
        "fonts/open-sans-v15-latin_latin-ext-regular.woff2",
        include_bytes!("../assets/fonts/open-sans-v15-latin_latin-ext-regular.woff2"),
    ),
    (
        "fonts/open-sans-v15-latin_latin-ext-italic.woff2",
        include_bytes!("../assets/fonts/open-sans-v15-latin_latin-ext-italic.woff2"),
    ),
    (
        "fonts/open-sans-v15-latin_latin-ext-700.woff2",
        include_bytes!("../assets/fonts/open-sans-v15-latin_latin-ext-700.woff2"),
    ),
    (
        "fonts/open-sans-v15-latin_latin-ext-700italic.woff2",
        include_bytes!("../assets/fonts/open-sans-v15-latin_latin-ext-700italic.woff2"),
    ),
    (
        "fonts/source-code-pro-v7-latin_latin-ext-regular.woff2",
        include_bytes!("../assets/fonts/source-code-pro-v7-latin_latin-ext-regular.woff2"),
    ),
    (
        "fonts/source-code-pro-v7-latin_latin-ext-700.woff2",
        include_bytes!("../assets/fonts/source-code-pro-v7-latin_latin-ext-700.woff2"),
    ),
    (
        "fonts/MaterialIcons-Regular.woff2",
        include_bytes!("../assets/fonts/MaterialIcons-Regular.woff2"),
    ),
    (
        "scripts/pkldoc.js",
        include_bytes!("../assets/scripts/pkldoc.js"),
    ),
    (
        "scripts/search-worker.js",
        include_bytes!("../assets/scripts/search-worker.js"),
    ),
    (
        "scripts/scroll-into-view.min.js",
        include_bytes!("../assets/scripts/scroll-into-view.min.js"),
    ),
    (
        "styles/pkldoc.css",
        include_bytes!("../assets/styles/pkldoc.css"),
    ),
    (
        "images/apple-touch-icon.png",
        include_bytes!("../assets/images/apple-touch-icon.png"),
    ),
    (
        "images/favicon.svg",
        include_bytes!("../assets/images/favicon.svg"),
    ),
    (
        "images/favicon-16x16.png",
        include_bytes!("../assets/images/favicon-16x16.png"),
    ),
    (
        "images/favicon-32x32.png",
        include_bytes!("../assets/images/favicon-32x32.png"),
    ),
];

/// Returns the raw bytes of an embedded resource by relative path (e.g. `"scripts/pkldoc.js"`).
pub fn get_resource(name: &str) -> Option<&'static [u8]> {
    STATIC_RESOURCES
        .iter()
        .find(|(k, _)| *k == name)
        .map(|(_, bytes)| *bytes)
}

/// Copies a named embedded resource into `target_dir/<name>`, overwriting any existing file.
pub fn copy_resource(resource_name: &str, target_dir: &Path) -> io::Result<()> {
    let bytes = get_resource(resource_name).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("Failed to load class path resource `{resource_name}`."),
        )
    })?;
    let target_file = target_dir.join(resource_name);
    if let Some(parent) = target_file.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&target_file, bytes)
}

/// Writes all 17 static resources into `target_dir`.
pub fn write_all_static_resources(target_dir: &Path) -> io::Result<()> {
    for (name, _) in STATIC_RESOURCES {
        copy_resource(name, target_dir)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_17_embedded_assets_match_pinned_upstream_bytes() {
        assert_eq!(STATIC_RESOURCES.len(), 17);
        let upstream_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../external/pkl/pkl-doc/src/main/resources/org/pkl/doc");
        for (rel, embedded) in STATIC_RESOURCES {
            let upstream = fs::read(upstream_root.join(rel))
                .unwrap_or_else(|e| panic!("failed to read upstream resource {rel}: {e}"));
            assert_eq!(
                *embedded,
                upstream.as_slice(),
                "embedded asset {rel} drifted from pinned Pkl 0.32.0 resource"
            );
        }
    }
}
