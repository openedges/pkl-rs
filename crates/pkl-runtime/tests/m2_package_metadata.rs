//! Source-derived invariants. Exact Java metadata API comparisons are acquired
//! separately; these assertions do not count original package cases as passed.
use pkl_runtime::{
    packages::{
        PackageUri,
        metadata::{MetadataFailure, accept_metadata},
    },
    value::PklString,
};
use sha2::{Digest, Sha256};

const BIRDS: &[u8] = include_bytes!(
    "../../../external/pkl/pkl-commons-test/src/main/files/packages/birds@0.5.0/birds@0.5.0.json"
);
const FRUIT: &[u8] = include_bytes!(
    "../../../external/pkl/pkl-commons-test/src/main/files/packages/fruit@1.0.5/fruit@1.0.5.json"
);

fn accept(
    bytes: &[u8],
    expected: Option<&PklString>,
    test_mode: bool,
) -> Result<pkl_runtime::packages::metadata::DependencyMetadata, MetadataFailure> {
    accept_metadata(
        &PackageUri::parse("package://localhost:0/birds@0.5.0").unwrap(),
        &"https://localhost:0/birds@0.5.0".into(),
        bytes,
        expected,
        test_mode,
    )
}

#[test]
fn unchanged_metadata_sources_validate_optional_fields_and_dependency_identity() {
    let birds = accept(BIRDS, None, false).unwrap();
    let fruit = accept(FRUIT, None, false).unwrap();
    assert_eq!(birds.name, "birds");
    assert_eq!(birds.dependencies.len(), 1);
    assert_eq!(birds.dependencies[0].name, "fruities");
    assert_eq!(birds.dependencies[0].package_uri.uri, fruit.package_uri.uri);
    assert_eq!(birds.dependencies[0].package_uri.sha256, None);
    // The raw upstream source has not yet substituted its generated ZIP digest.
    assert_eq!(birds.package_zip_sha256, "$computedChecksum");
}

#[test]
fn metadata_checksum_precedes_json_decoding_and_marker_is_exact_and_opt_in() {
    let malformed = b"{";
    let digest: PklString = Sha256::digest(malformed)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>()
        .into();
    assert!(matches!(
        accept(malformed, Some(&"wrong".into()), false),
        Err(MetadataFailure::InvalidMetadataChecksum(_))
    ));
    assert!(matches!(
        accept(malformed, Some(&digest), false),
        Err(MetadataFailure::InvalidDependencyMetadata { .. })
    ));
    for (expected, test_mode, bypass) in [
        ("$skipChecksumVerification", false, false),
        ("$skipChecksumVerification", true, true),
        ("$skipChecksumVerification ", true, false),
    ] {
        let result = accept(malformed, Some(&expected.into()), test_mode);
        assert_eq!(
            matches!(
                result,
                Err(MetadataFailure::InvalidDependencyMetadata { .. })
            ),
            bypass
        );
        assert_eq!(
            matches!(result, Err(MetadataFailure::InvalidMetadataChecksum(_))),
            !bypass
        );
    }
    let metadata = accept(BIRDS, Some(&"$skipChecksumVerification".into()), true).unwrap();
    assert_eq!(metadata.package_zip_sha256, "$computedChecksum");
}

#[test]
fn optional_metadata_failure_precedes_https_zip_validation() {
    let ftp = String::from_utf8(FRUIT.to_vec())
        .unwrap()
        .replace("https://localhost:0/", "ftp://localhost:0/");
    assert!(matches!(
        accept(ftp.as_bytes(), None, false),
        Err(MetadataFailure::InvalidZipUrl { .. })
    ));
    let invalid_optional = ftp.replace("\"UNLICENSED\"", "false");
    assert!(matches!(
        accept(invalid_optional.as_bytes(), None, false),
        Err(MetadataFailure::InvalidDependencyMetadata { .. })
    ));
}

#[test]
fn invalid_annotations_and_bad_uri_projection_never_become_successful_metadata() {
    let fruit = String::from_utf8(FRUIT.to_vec()).unwrap();
    let annotated = fruit.replacen('{', "{\"annotations\":[{}],", 1);
    assert!(matches!(
        accept(annotated.as_bytes(), None, false),
        Err(MetadataFailure::InvalidDependencyMetadata { .. })
    ));
    let bad_uri = fruit.replace(
        "https://example.com/fruit",
        "https://example.com/fruit space",
    );
    assert!(matches!(
        accept(bad_uri.as_bytes(), None, false),
        Err(MetadataFailure::Unsupported { .. })
    ));
}

#[test]
fn valid_metadata_annotations_parse_typed_values() {
    use pkl_runtime::packages::metadata::{MetadataAnnotation, MetadataClassInfo, MetadataValue};

    let fruit = String::from_utf8(FRUIT.to_vec()).unwrap();
    let annotation_json = r#""annotations":[{"type":"PObject","classInfo":{"moduleName":"pkl.base","class":"Deprecated","moduleUri":"pkl:base"},"properties":{"since":"1.0.0","message":null,"count":3,"flag":true,"tags":{"type":"Set","value":["a","b"]},"meta":{"type":"Map","value":[{"key":"k","value":42}]},"regex":{"type":"Pattern","value":"^foo$"},"size":{"type":"DataSize","value":5.5,"unit":"mb"},"ttl":{"type":"Duration","value":2.0,"unit":"s"},"pair":{"type":"Pair","first":"x","second":[1,2]},"nested":{"type":"PObject","classInfo":{"moduleName":"pkl.base","class":"Since","moduleUri":"pkl:base"},"properties":{"version":"0.32.0"}}}}],"#;
    let annotated = fruit.replacen('{', &format!("{{{annotation_json}"), 1);
    let parsed = accept(annotated.as_bytes(), None, false).unwrap();
    assert_eq!(parsed.annotations.len(), 1);
    let ann = &parsed.annotations[0];
    assert_eq!(
        ann.class_info,
        MetadataClassInfo {
            module_name: "pkl.base".into(),
            class_name: "Deprecated".into(),
            module_uri: "pkl:base".into(),
        }
    );
    let prop = |name: &str| {
        ann.properties
            .iter()
            .find(|(k, _)| k.as_str() == Some(name))
            .map(|(_, v)| v)
            .expect("annotation property")
    };
    assert_eq!(prop("since"), &MetadataValue::String("1.0.0".into()));
    assert_eq!(prop("message"), &MetadataValue::Null);
    assert_eq!(prop("count"), &MetadataValue::Int(3));
    assert_eq!(prop("flag"), &MetadataValue::Bool(true));
    assert_eq!(
        prop("tags"),
        &MetadataValue::Set(vec![
            MetadataValue::String("a".into()),
            MetadataValue::String("b".into()),
        ])
    );
    assert_eq!(
        prop("meta"),
        &MetadataValue::Map(vec![(
            MetadataValue::String("k".into()),
            MetadataValue::Int(42),
        )])
    );
    assert_eq!(prop("regex"), &MetadataValue::Pattern("^foo$".into()));
    assert_eq!(
        prop("size"),
        &MetadataValue::DataSize {
            value: 5.5,
            unit: "mb".into(),
        }
    );
    assert_eq!(
        prop("ttl"),
        &MetadataValue::Duration {
            value: 2.0,
            unit: "s".into(),
        }
    );
    assert_eq!(
        prop("pair"),
        &MetadataValue::Pair(
            Box::new(MetadataValue::String("x".into())),
            Box::new(MetadataValue::List(vec![
                MetadataValue::Int(1),
                MetadataValue::Int(2),
            ])),
        )
    );
    assert_eq!(
        prop("nested"),
        &MetadataValue::PObject(MetadataAnnotation {
            class_info: MetadataClassInfo {
                module_name: "pkl.base".into(),
                class_name: "Since".into(),
                module_uri: "pkl:base".into(),
            },
            properties: vec![("version".into(), MetadataValue::String("0.32.0".into()))],
        })
    );
}

#[test]
fn duplicate_json_fields_replace_values_before_metadata_extraction() {
    let fruit = String::from_utf8(FRUIT.to_vec()).unwrap();
    let replaced = fruit.replacen('{', "{\"name\":false,", 1);
    assert_eq!(
        accept(replaced.as_bytes(), None, false).unwrap().name,
        "fruit"
    );
}

#[test]
fn stage1_package_metadata_error_projection_and_request_counts() {
    use pkl_runtime::modules::ModuleHostError;
    use pkl_runtime::packages::http::{
        metadata_request_count, reset_request_counts, zip_request_count,
    };
    use pkl_runtime::packages::project::PklProject;
    use pkl_runtime::packages::resolver::{PackageHost, resolve_package_module, with_package_host};
    use pkl_runtime::packages::zip::ZipArchive;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    struct TestPackageHost;
    impl PackageHost for TestPackageHost {
        fn fetch_metadata(
            &self,
            _package: &PackageUri,
            uri: &PklString,
        ) -> Result<Vec<u8>, ModuleHostError> {
            let raw_uri = uri.as_str().unwrap_or("");
            if raw_uri.contains("badPackageZipUrl@1.0.0") {
                Ok(include_bytes!(
                    "../../../external/pkl/pkl-commons-test/src/main/files/packages/badPackageZipUrl@1.0.0/badPackageZipUrl@1.0.0.json"
                ).to_vec())
            } else if raw_uri.contains("badMetadataJson@1.0.0") {
                Ok(include_bytes!(
                    "../../../external/pkl/pkl-commons-test/src/main/files/packages/badMetadataJson@1.0.0/badMetadataJson@1.0.0.json"
                ).to_vec())
            } else {
                Err(ModuleHostError::SecurityManagerException {
                    message: Some(
                        format!(
                            "Exception when making request `GET {raw_uri}`:\nConnection refused (localhost)"
                        )
                        .into(),
                    ),
                })
            }
        }

        fn fetch_zip(
            &self,
            _package: &PackageUri,
            _url: &PklString,
        ) -> Result<Vec<u8>, ModuleHostError> {
            Err(ModuleHostError::SecurityManagerException { message: None })
        }

        fn read_disk_archive(&self, _package: &PackageUri) -> Option<ZipArchive> {
            None
        }

        fn write_disk_archive(
            &self,
            _package: &PackageUri,
            _metadata_bytes: &[u8],
            _zip_bytes: &[u8],
        ) -> Result<(), ModuleHostError> {
            Ok(())
        }

        fn find_project_dir(&self, _start: &Path) -> Option<PathBuf> {
            None
        }

        fn load_project(&self, project_dir: &Path) -> Result<PklProject, ModuleHostError> {
            Ok(PklProject {
                project_dir: project_dir.to_path_buf(),
                dependencies: std::collections::BTreeMap::new(),
                ..Default::default()
            })
        }

        fn read_file(&self, _path: &Path) -> Result<Vec<u8>, ModuleHostError> {
            Err(ModuleHostError::SecurityManagerException { message: None })
        }

        fn list_dir(
            &self,
            _path: &Path,
        ) -> Result<Vec<pkl_runtime::glob::PathElement>, ModuleHostError> {
            Ok(Vec::new())
        }

        fn is_test_mode(&self) -> bool {
            false
        }
    }

    with_package_host(Arc::new(TestPackageHost), || {
        // Test badImport6: invalid zip asset URL (ftp instead of https)
        reset_request_counts();
        let bad_zip_uri: PklString = "package://localhost:0/badPackageZipUrl@1.0.0#/Bug.pkl".into();
        let err = resolve_package_module(&bad_zip_uri).unwrap_err();
        assert_eq!(metadata_request_count(), 1);
        assert_eq!(zip_request_count(), 0);
        match err {
            ModuleHostError::SecurityManagerException { message } => {
                let msg = message.expect("error message");
                assert!(
                    msg.contains("Expected the zip asset for package `package://localhost:0/badPackageZipUrl@1.0.0` to be an HTTPS URI, but got `ftp://wait/a/minute`."),
                    "Unexpected message: {msg}"
                );
            }
            other => panic!("Expected SecurityManagerException, got {other:?}"),
        }

        // Test badImport7: malformed JSON in metadata
        reset_request_counts();
        let bad_json_uri: PklString = "package://localhost:0/badMetadataJson@1.0.0#/Bug.pkl".into();
        let err = resolve_package_module(&bad_json_uri).unwrap_err();
        assert_eq!(metadata_request_count(), 1);
        assert_eq!(zip_request_count(), 0);
        match err {
            ModuleHostError::SecurityManagerException { message } => {
                let msg = message.expect("error message");
                assert!(
                    msg.contains("Received invalid dependency metadata for package `package://localhost:0/badMetadataJson@1.0.0` when making request `GET https://localhost:0/badMetadataJson@1.0.0`:\nMalformed JSON: Expected 'r' at 1:2.\n\nOriginal input:\n`this is not json\n`"),
                    "Unexpected message: {msg}"
                );
            }
            other => panic!("Expected SecurityManagerException, got {other:?}"),
        }
    });
}

#[test]
fn package_vs_projectpackage_dependency_lookup_isolation() {
    use pkl_runtime::packages::project::{PklProject, ResolvedProjectDep};
    use pkl_runtime::packages::resolver::{PackageHost, resolve_import_uri, with_package_host};
    use pkl_runtime::packages::zip::ZipArchive;
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    struct MockHost;
    impl PackageHost for MockHost {
        fn fetch_metadata(
            &self,
            _package: &PackageUri,
            uri: &PklString,
        ) -> Result<Vec<u8>, pkl_runtime::modules::ModuleHostError> {
            let raw_uri = uri.as_str().unwrap_or("");
            if raw_uri.contains("birds@0.5.0") {
                Ok(BIRDS.to_vec())
            } else {
                Err(pkl_runtime::modules::ModuleHostError::NotFound)
            }
        }
        fn fetch_zip(
            &self,
            _package: &PackageUri,
            _url: &PklString,
        ) -> Result<Vec<u8>, pkl_runtime::modules::ModuleHostError> {
            Ok(pkl_runtime::packages::zip::build_uncompressed_zip(&[]))
        }
        fn read_disk_archive(&self, _package: &PackageUri) -> Option<ZipArchive> {
            None
        }
        fn write_disk_archive(
            &self,
            _package: &PackageUri,
            _meta: &[u8],
            _zip: &[u8],
        ) -> Result<(), pkl_runtime::modules::ModuleHostError> {
            Ok(())
        }
        fn find_project_dir(&self, _start: &Path) -> Option<PathBuf> {
            Some(PathBuf::from("/mock/project"))
        }
        fn load_project(
            &self,
            project_dir: &Path,
        ) -> Result<PklProject, pkl_runtime::modules::ModuleHostError> {
            let mut deps = BTreeMap::new();
            deps.insert(
                "fruities".to_string(),
                ResolvedProjectDep {
                    dep_type: "remote".to_string(),
                    uri: "projectpackage://localhost:0/fruit@1.1.0".into(),
                    path: None,
                    sha256: None,
                },
            );
            Ok(PklProject {
                project_dir: project_dir.to_path_buf(),
                dependencies: deps,
                deps_loaded: true,
                ..Default::default()
            })
        }
        fn read_file(
            &self,
            _path: &Path,
        ) -> Result<Vec<u8>, pkl_runtime::modules::ModuleHostError> {
            Err(pkl_runtime::modules::ModuleHostError::NotFound)
        }
        fn list_dir(
            &self,
            _path: &Path,
        ) -> Result<Vec<pkl_runtime::glob::PathElement>, pkl_runtime::modules::ModuleHostError>
        {
            Ok(Vec::new())
        }
        fn is_test_mode(&self) -> bool {
            true
        }
    }

    with_package_host(Arc::new(MockHost), || {
        // Absolute `package:` base MUST resolve strictly to package's own metadata (fruit@1.0.5)
        let pkg_res = resolve_import_uri(
            &"package://localhost:0/birds@0.5.0#/catalog/Swallow.pkl".into(),
            &"@fruities/Apple.pkl".into(),
        )
        .unwrap();
        assert_eq!(
            pkg_res.as_str().unwrap(),
            "package://localhost:0/fruit@1.0.5#/Apple.pkl"
        );

        // `projectpackage:` base MUST consult project dependencies (rewritten to fruit@1.1.0)
        let proj_res = resolve_import_uri(
            &"projectpackage://localhost:0/birds@0.5.0#/catalog/Swallow.pkl".into(),
            &"@fruities/Apple.pkl".into(),
        )
        .unwrap();
        assert_eq!(
            proj_res.as_str().unwrap(),
            "projectpackage://localhost:0/fruit@1.1.0#/Apple.pkl"
        );
    });
}
