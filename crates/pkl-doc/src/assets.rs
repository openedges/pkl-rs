//! All 17 embedded static web resources for `pkldoc`:
//! - 13 functional web assets verbatim from Apple Pkl 0.32.0 (`fonts/*`, `scripts/*`, `styles/*`)
//! - 4 original `pkl-rs` brand favicons (`images/*`, Copyright 2025-2026 The pkl-rs Authors, Apache-2.0)

use std::fs;
use std::io;
use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

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

/// Writes all 17 static resources into `target_dir` concurrently across scoped threads,
/// matching upstream `HtmlGenerator.kt:87-111`.
pub fn write_all_static_resources(target_dir: &Path) -> io::Result<()> {
    for dir in ["fonts", "scripts", "styles", "images"] {
        fs::create_dir_all(target_dir.join(dir))?;
    }
    let next = AtomicUsize::new(0);
    let first_err: Mutex<Option<io::Error>> = Mutex::new(None);
    let workers = std::thread::available_parallelism()
        .map(|n| n.get().min(8))
        .unwrap_or(8);
    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| {
                loop {
                    let idx = next.fetch_add(1, Ordering::Relaxed);
                    if idx >= STATIC_RESOURCES.len() {
                        break;
                    }
                    let (name, bytes) = STATIC_RESOURCES[idx];
                    if let Err(err) = fs::write(target_dir.join(name), bytes) {
                        let mut guard = first_err.lock().unwrap_or_else(|e| e.into_inner());
                        if guard.is_none() {
                            *guard = Some(err);
                        }
                        break;
                    }
                }
            });
        }
    });
    if let Some(err) = first_err.into_inner().unwrap_or_else(|e| e.into_inner()) {
        return Err(err);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const BRANDED_ICON_RESOURCES: &[&str] = &[
        "images/apple-touch-icon.png",
        "images/favicon.svg",
        "images/favicon-16x16.png",
        "images/favicon-32x32.png",
    ];

    #[test]
    fn all_17_embedded_assets_match_pinned_upstream_or_branded_icons() {
        assert_eq!(STATIC_RESOURCES.len(), 17);
        let upstream_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../external/pkl/pkl-doc/src/main/resources/org/pkl/doc");
        for (rel, embedded) in STATIC_RESOURCES {
            let upstream = fs::read(upstream_root.join(rel))
                .unwrap_or_else(|e| panic!("failed to read upstream resource {rel}: {e}"));
            if BRANDED_ICON_RESOURCES.contains(rel) {
                assert!(
                    !embedded.is_empty(),
                    "branded icon asset {rel} must not be empty"
                );
                assert_ne!(
                    *embedded,
                    upstream.as_slice(),
                    "branded icon asset {rel} must not reuse Apple Pkl trademarked logo"
                );
                if rel.ends_with(".svg") {
                    let text = std::str::from_utf8(embedded).expect("valid utf8 svg");
                    assert!(text.contains("<svg"), "invalid svg header in {rel}");
                } else {
                    assert!(
                        embedded.starts_with(b"\x89PNG\r\n\x1a\n"),
                        "invalid PNG signature in {rel}"
                    );
                }
            } else {
                assert_eq!(
                    *embedded,
                    upstream.as_slice(),
                    "embedded functional asset {rel} drifted from pinned Pkl 0.32.0 resource"
                );
            }
        }
    }
}
