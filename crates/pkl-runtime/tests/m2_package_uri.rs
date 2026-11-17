//! Source-derived PackageUri controls; no Java observation is claimed here.
use pkl_runtime::modules::{
    ModuleCompletion, ModuleHostError, ModuleIdentity, ModuleKeyId, ModuleRequest,
    ModuleRequestKind, SelectionPhase, default_completion, standard_key_completion,
};
use pkl_runtime::packages::{PackageAssetUri, PackageUri, PackageUriFailure};
use pkl_runtime::value::PklString;

#[test]
fn validation_order_precedes_version_fragment_and_transport() {
    for (uri, predicate) in [
        ("package:opaque", 0),
        ("PACKAGE://host/name@1.0.0#/", 1),
        ("package:/name@1.0.0#/", 2),
        ("package://host#/", 3),
        ("package://host/%2e%2e/name@bad::bad#/", 4),
        ("package://host/name#/", 5),
        ("package://host/name@bad::invalid:x#relative", 6),
        ("package://host/name@bad::sha256:x#relative", 7),
        ("package://host/name@1.0.0", 8),
        ("package://host/name@1.0.0#", 9),
    ] {
        let failure = PackageAssetUri::parse(uri).unwrap_err().failure;
        assert!(matches!(
            (predicate, failure),
            (0, PackageUriFailure::Opaque)
                | (1, PackageUriFailure::Scheme(_))
                | (2, PackageUriFailure::MissingAuthority)
                | (3, PackageUriFailure::MissingPath)
                | (4, PackageUriFailure::ParentSegment)
                | (5, PackageUriFailure::MissingVersion(_))
                | (6, PackageUriFailure::UnknownChecksum(_))
                | (
                    7,
                    PackageUriFailure::InvalidVersion {
                        overflow: false,
                        ..
                    }
                )
                | (8, PackageUriFailure::MissingFragment)
                | (9, PackageUriFailure::RelativeFragment(_))
        ));
    }
}

#[test]
fn checksum_split_and_permissive_version_are_not_strict_semver() {
    let asset = PackageAssetUri::parse(
        "package://Host:00080/name@0001.02.3-pre..x+build+more::sha256:not-hex::#/../asset",
    )
    .unwrap();
    assert_eq!(
        (
            asset.package.version.major,
            asset.package.version.minor,
            asset.package.version.patch
        ),
        (1, 2, 3)
    );
    assert_eq!(asset.package.sha256.as_ref().unwrap(), "not-hex");
    assert_eq!(
        asset.package.version.pre_release.as_ref().unwrap(),
        "pre..x"
    );
    assert_eq!(asset.package.version.build.as_ref().unwrap(), "build+more");
    assert_eq!(asset.asset_path, "/../asset");
    assert!(matches!(
        PackageUri::parse("package://host/name@2147483648.0.0")
            .unwrap_err()
            .failure,
        PackageUriFailure::InvalidVersion { overflow: true, .. }
    ));
    assert!(matches!(
        PackageUri::parse("package://host/name@2147483648.0.bad")
            .unwrap_err()
            .failure,
        PackageUriFailure::InvalidVersion {
            overflow: false,
            ..
        }
    ));
    assert!(matches!(
        PackageUri::parse("package://host/name@1.0.0::sha256:")
            .unwrap_err()
            .failure,
        PackageUriFailure::InvalidChecksum(_)
    ));
    assert!(matches!(
        PackageUri::parse("package://host/x::ignored@1.0.0::sha256:x")
            .unwrap_err()
            .failure,
        PackageUriFailure::InvalidVersion { .. }
    ));
}

#[test]
fn decoded_components_and_rebuilt_package_uri_keep_distinct_identity() {
    let asset = PackageAssetUri::parse(
        "package://Host:00080/%62ird%40home@1.0.0?key=%2F#%2Fcatalog%2Fbird",
    )
    .unwrap();
    assert_eq!(asset.package.uri, "package://Host:80/bird@home@1.0.0?key=/");
    assert_eq!(asset.package.path_without_version, "/bird@home");
    assert_eq!(asset.asset_path, "/catalog/bird");
    assert_eq!(
        asset.uri,
        "package://Host:00080/%62ird%40home@1.0.0?key=%2F#%2Fcatalog%2Fbird"
    );
    assert_eq!(
        PackageUri::parse("package://reg_name/name@1.0.0")
            .unwrap()
            .uri,
        "package:/name@1.0.0"
    );
    let checksum = PackageUri::parse("package://host/%62ird@1.0.0::bad#/asset").unwrap_err();
    assert_eq!(checksum.input, "package://host/bird@1.0.0::bad");
}

#[test]
fn lossless_error_operands_and_version_labels_preserve_utf16_units() {
    let invalid =
        PklString::from("package://host/name@bad-").concat(&PklString::from_utf16(vec![0xd800]));
    let error = PackageUri::parse(invalid.clone()).unwrap_err();
    assert_eq!(error.input, invalid);
    assert!(error.reason().encode_utf16().any(|unit| unit == 0xd800));
    let accepted =
        PklString::from("package://host/name@1.0.0-").concat(&PklString::from_utf16(vec![0xd800]));
    assert_eq!(
        PackageUri::parse(accepted)
            .unwrap()
            .version
            .pre_release
            .unwrap()
            .encode_utf16()
            .collect::<Vec<_>>(),
        vec![0xd800]
    );
    assert!(matches!(
        PackageUri::parse("package://host/name@1.0.0+bad%0A")
            .unwrap_err()
            .failure,
        PackageUriFailure::InvalidVersion { .. }
    ));
    assert_eq!(
        PackageUri::parse("package://host/name@1.0.0-pre%0A")
            .unwrap()
            .version
            .pre_release
            .unwrap(),
        "pre\n"
    );
}

#[test]
fn explicit_standard_factory_validates_package_but_does_not_claim_io() {
    let bad_request = ModuleRequest {
        id: 1,
        kind: ModuleRequestKind::SelectKey {
            id: ModuleKeyId(1),
            uri: "package://host/name#/asset".into(),
            phase: SelectionPhase::RuntimeImport,
        },
    };
    let Some(ModuleCompletion::KeySelected(Err(error))) = standard_key_completion(&bad_request)
    else {
        panic!("bounded factory result")
    };
    assert!(matches!(error, ModuleHostError::UriSyntax { .. }));

    let good_request = ModuleRequest {
        id: 1,
        kind: ModuleRequestKind::SelectKey {
            id: ModuleKeyId(1),
            uri: "package://host/name@1.0.0#/asset".into(),
            phase: SelectionPhase::RuntimeImport,
        },
    };
    let Some(ModuleCompletion::KeySelected(Ok(key))) = standard_key_completion(&good_request)
    else {
        panic!("bounded factory result")
    };
    assert_eq!(
        key.kind,
        pkl_runtime::modules::ModuleKeyKind::StandardGenericUrl
    );

    // Import-node trust admission is a separate earlier callback. A pkl module
    // cannot import this less trusted package, even when package syntax is bad.
    let request = ModuleRequest {
        id: 2,
        kind: ModuleRequestKind::CheckImport {
            importer: ModuleIdentity {
                original_uri: "pkl:base".into(),
                resolved_uri: "pkl:base".into(),
            },
            import_uri: "package://host/no-version".into(),
        },
    };
    assert!(matches!(
        default_completion(&request),
        Some(ModuleCompletion::ImportChecked(Err(
            ModuleHostError::SecurityManagerException { .. }
        )))
    ));
}

#[test]
fn reconstructed_authority_is_validated_before_version_and_fragment() {
    for uri in [
        "package://reg_name//name@1.0.0#/asset",
        "package://reg_name/%2Fname@1.0.0#/asset",
        "package://reg_name/%2Fname@1.0.0#relative",
        "package://reg_name/%2Fname@1.0.0",
    ] {
        let error = PackageAssetUri::parse(uri).unwrap_err();
        assert_eq!(error.input, uri);
        assert!(matches!(error.failure, PackageUriFailure::Unsupported(_)));
    }
    for (uri, path) in [
        ("package://reg_name//name@1.0.0-pre.com#/asset", "//name"),
        ("package://reg_name/%2Fname@1.0.0-pre.com#/asset", "//name"),
    ] {
        let asset = PackageAssetUri::parse(uri).unwrap();
        assert_eq!(asset.uri, uri);
        assert_eq!(asset.package.uri, "package://name@1.0.0-pre.com");
        assert_eq!(asset.package.path_without_version, path);
        assert_eq!(asset.package.version.pre_release.unwrap(), "pre.com");
        assert_eq!(asset.asset_path, "/asset");
    }
}
