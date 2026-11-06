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
fn unsupported_annotations_and_uri_projection_never_become_successful_metadata() {
    let fruit = String::from_utf8(FRUIT.to_vec()).unwrap();
    let annotated = fruit.replacen('{', "{\"annotations\":[{}],", 1);
    assert!(matches!(
        accept(annotated.as_bytes(), None, false),
        Err(MetadataFailure::Unsupported { .. })
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
fn duplicate_json_fields_replace_values_before_metadata_extraction() {
    let fruit = String::from_utf8(FRUIT.to_vec()).unwrap();
    let replaced = fruit.replacen('{', "{\"name\":false,", 1);
    assert_eq!(
        accept(replaced.as_bytes(), None, false).unwrap().name,
        "fruit"
    );
}
