//! Comprehensive M4 Track C tests: `CanonicalPackageUri`, SemVer, `DependencyMetadata`,
//! `ProjectDeps` lockfile, `SettingsPrecedence`, circular project deps, MVS resolver,
//! `ProjectDependenciesManager`, `package-2` cache & corrupt-cache failures, strict ZIP
//! reader & deterministic DEFLATE writer, `ProjectPackager`, and minimal `pkl:test` runner.
use pkl_runtime::modules::ModuleHostError;
use pkl_runtime::packages::cache::{
    InMemoryPackageCacheStore, PackageCachePaths, encode_cache_path, get_cache_rel_path,
};
use pkl_runtime::packages::cycles::{
    filter_same_scheme_cycles, find_import_cycles, format_circular_project_dependencies_error,
};
use pkl_runtime::packages::http::{PackageHttpError, PackageTransport, get_metadata_request_uri};
use pkl_runtime::packages::metadata::{
    DependencyMetadata, MetadataAnnotation, MetadataClassInfo, MetadataFailure, MetadataValue,
    RemoteDependency, accept_metadata, sha256_hex,
};
use pkl_runtime::packages::mvs::{ProjectDependenciesResolver, relativize_paths};
use pkl_runtime::packages::packager::{
    ExcludeGlobMatcher, PackagerHost, ProjectPackager, is_file_excluded,
    validate_local_projects_included, validate_pkl_imports_and_reads,
};
use pkl_runtime::packages::project::{
    CliSettingsInput, DeclaredDependenciesTree, DeclaredRemoteDep, EvaluatorColor,
    EvaluatorExternalReader, EvaluatorHttpProxy, EvaluatorHttpSettings, EvaluatorSettings,
    PackageSpec, PklSettings, Project, ProjectDepEntry, ProjectDependenciesManager, ProjectDeps,
    ResolvedProjectDep, find_matching_resolved, find_project_dir_with, resolve_effective_settings,
};
use pkl_runtime::packages::resolver::PackageResolver;
use pkl_runtime::packages::test_runner::{
    TestResults, TestSectionName, TestSectionResults, evaluate_examples_rendered,
    evaluate_facts_decoded, render_spec_report, render_spec_summary,
};
use pkl_runtime::packages::zip::{
    PACKAGE_ZIP_DOS_DATE, PACKAGE_ZIP_DOS_TIME, ZipArchive, ZipError,
    build_deterministic_package_zip, deflate_raw,
};
use pkl_runtime::packages::{CanonicalPackageUri, PackageAssetUri, PackageUri, PackageVersion};
use pkl_runtime::pkl_binary_decode::DecodedValue;
use pkl_runtime::value::PklString;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

// ---------------------------------------------------------------------------
// 1. CanonicalPackageUri, SemVer, PackageUri helpers
// ---------------------------------------------------------------------------

#[test]
fn canonical_package_uri_separates_major_versions_and_normalizes_components() {
    let v0 = PackageUri::parse("package://example.com/foo/bar@0.5.0").unwrap();
    let v1 = PackageUri::parse("projectpackage://user@example.com:8443/foo/bar@1.2.3?k=v").unwrap();
    let v1_other =
        PackageUri::parse("package://example.com:8443/foo/bar@1.9.0::sha256:abc").unwrap();
    let v2 = PackageUri::parse("package://example.com:8443/foo/bar@2.0.0").unwrap();

    assert_eq!(
        v0.to_canonical().as_str(),
        Some("package://example.com/foo/bar@0")
    );
    assert_eq!(
        v1.to_canonical().as_str(),
        Some("package://example.com:8443/foo/bar@1")
    );
    assert_eq!(v1.to_canonical(), v1_other.to_canonical());
    assert_ne!(v1.to_canonical(), v2.to_canonical());
    assert_eq!(v2.to_canonical().major_version(), 2);

    // Multi-major resolution table MUST NOT conflate @1 with @2
    let mut table = BTreeMap::new();
    table.insert(
        "package://example.com:8443/foo/bar@1".to_string(),
        ResolvedProjectDep {
            dep_type: "remote".into(),
            uri: "projectpackage://example.com:8443/foo/bar@1.9.0".into(),
            path: None,
            sha256: Some("sha1".into()),
        },
    );
    table.insert(
        "package://example.com:8443/foo/bar@2".to_string(),
        ResolvedProjectDep {
            dep_type: "remote".into(),
            uri: "projectpackage://example.com:8443/foo/bar@2.1.0".into(),
            path: None,
            sha256: Some("sha2".into()),
        },
    );
    assert_eq!(
        find_matching_resolved("package://example.com:8443/foo/bar@1.0.0", "bar1", &table)
            .unwrap()
            .uri,
        "projectpackage://example.com:8443/foo/bar@1.9.0"
    );
    assert_eq!(
        find_matching_resolved("package://example.com:8443/foo/bar@2.0.0", "bar2", &table)
            .unwrap()
            .uri,
        "projectpackage://example.com:8443/foo/bar@2.1.0"
    );
    assert!(
        find_matching_resolved("package://example.com:8443/foo/bar@3.0.0", "bar3", &table)
            .is_none()
    );
}

#[test]
fn semver_ordering_and_build_metadata_equality_match_version_java() {
    let ordered = [
        "1.0.0-alpha",
        "1.0.0-alpha.1",
        "1.0.0-alpha.beta",
        "1.0.0-beta",
        "1.0.0-beta.2",
        "1.0.0-beta.11",
        "1.0.0-rc.1",
        "1.0.0",
        "1.0.1",
        "1.1.0",
        "2.0.0-alpha",
        "2.0.0",
    ];
    let parsed: Vec<PackageVersion> = ordered
        .iter()
        .map(|s| PackageVersion::parse(*s).unwrap())
        .collect();
    for pair in parsed.windows(2) {
        assert!(pair[0] < pair[1], "expected {} < {}", pair[0], pair[1]);
    }

    let b1 = PackageVersion::parse("1.2.3+build.1").unwrap();
    let b2 = PackageVersion::parse("1.2.3+build.2").unwrap();
    let plain = PackageVersion::parse("1.2.3").unwrap();
    assert_eq!(b1, b2);
    assert_eq!(b1, plain);
    assert_eq!(b1.cmp(&b2), std::cmp::Ordering::Equal);
}

#[test]
fn package_uri_helpers_and_metadata_request_uri_preserve_double_colon_when_no_checksum() {
    let no_sum = PackageUri::parse("package://example.com/a::b@1.0.0").unwrap();
    assert_eq!(no_sum.sha256, None);
    assert_eq!(
        get_metadata_request_uri(&no_sum),
        "https://example.com/a::b@1.0.0"
    );

    let with_sum =
        PackageUri::parse("projectpackage://example.com/pkg@1.0.0::sha256:deadbeef").unwrap();
    assert!(with_sum.is_project_package());
    assert_eq!(with_sum.display_name(), "package://example.com/pkg@1.0.0");
    assert_eq!(
        get_metadata_request_uri(&with_sum),
        "https://example.com/pkg@1.0.0"
    );
    assert!(with_sum.check_has_no_checksum_component().is_err());

    let asset = PackageAssetUri::parse("package://example.com/pkg@1.0.0#/dir/a.pkl").unwrap();
    let resolved = asset.resolve("../other/b.pkl").unwrap();
    assert_eq!(resolved.asset_path, "/other/b.pkl");
    assert_eq!(
        resolved.to_project_package_asset_uri().uri,
        "projectpackage://example.com/pkg@1.0.0#/other/b.pkl"
    );
}

// ---------------------------------------------------------------------------
// 2. DependencyMetadata: no hardcoded bypass, full optional fields & annotations, JSON writer
// ---------------------------------------------------------------------------

#[test]
fn dependency_metadata_full_annotations_and_json_roundtrip_have_no_trailing_newline() {
    let meta = DependencyMetadata {
        name: "myPkg".into(),
        package_uri: PackageUri::parse("package://example.com/myPkg@1.0.0").unwrap(),
        version: PackageVersion::parse("1.0.0").unwrap(),
        package_zip_url: "https://example.com/myPkg@1.0.0.zip".into(),
        package_zip_sha256: "abc123".into(),
        dependencies: vec![RemoteDependency {
            name: "dep".into(),
            package_uri: PackageUri::parse("package://example.com/dep@2.0.0").unwrap(),
            sha256: "def456".into(),
        }],
        source_code_url_scheme: Some(
            "https://github.com/org/repo/blob/v1.0.0%{path}#L%{line}-L%{endLine}".into(),
        ),
        source_code: Some("https://github.com/org/repo".into()),
        documentation: Some("https://docs.example.com".into()),
        license: Some("Apache-2.0".into()),
        license_text: Some("Line1\nLine2\u{2028}End".into()),
        authors: Some(vec!["Alice".into(), "Bob".into()]),
        issue_tracker: Some("https://github.com/org/repo/issues".into()),
        description: Some("A package".into()),
        annotations: vec![MetadataAnnotation {
            class_info: MetadataClassInfo {
                module_name: "pkl.base".into(),
                class_name: "Deprecated".into(),
                module_uri: "pkl:base".into(),
            },
            properties: vec![
                ("since".into(), MetadataValue::String("1.0.0".into())),
                ("message".into(), MetadataValue::Null),
                (
                    "tags".into(),
                    MetadataValue::Set(vec![MetadataValue::String("old".into())]),
                ),
                (
                    "meta".into(),
                    MetadataValue::Map(vec![(
                        MetadataValue::String("k".into()),
                        MetadataValue::Pair(
                            Box::new(MetadataValue::Int(42)),
                            Box::new(MetadataValue::Pattern("[a-z]+".into())),
                        ),
                    )]),
                ),
                (
                    "size".into(),
                    MetadataValue::DataSize {
                        value: 5.0,
                        unit: "mb".into(),
                    },
                ),
                (
                    "timeout".into(),
                    MetadataValue::Duration {
                        value: 30.0,
                        unit: "s".into(),
                    },
                ),
            ],
        }],
    };

    let json = meta.write_json().unwrap();
    // DependencyMetadata.writeTo MUST NOT emit a trailing newline (`DependencyMetadata.java:594`).
    assert!(
        !json.ends_with('\n'),
        "metadata JSON must not end with newline"
    );
    assert!(
        json.contains("\\u2028"),
        "must escape U+2028 per JsonEscaper"
    );

    let digest = PklString::from(sha256_hex(json.as_bytes()).as_str());
    let reparsed = accept_metadata(
        &meta.package_uri,
        &"https://example.com/myPkg@1.0.0".into(),
        json.as_bytes(),
        Some(&digest),
        false,
    )
    .unwrap();

    assert_eq!(reparsed.name, "myPkg");
    assert_eq!(
        reparsed.license.as_ref().and_then(PklString::as_str),
        Some("Apache-2.0")
    );
    assert_eq!(
        reparsed
            .source_code_url_scheme
            .as_ref()
            .and_then(PklString::as_str),
        Some("https://github.com/org/repo/blob/v1.0.0%{path}#L%{line}-L%{endLine}")
    );
    assert_eq!(reparsed.authors.as_ref().map(Vec::len), Some(2));
    assert_eq!(reparsed.annotations.len(), 1);
    assert_eq!(reparsed.annotations[0].class_info.class_name, "Deprecated");
    assert_eq!(reparsed.annotations[0].properties.len(), 6);
}

#[test]
fn hardcoded_birds_fruit_checksum_bypass_is_removed() {
    let birds = include_bytes!(
        "../../../external/pkl/pkl-commons-test/src/main/files/packages/birds@0.5.0/birds@0.5.0.json"
    );
    let pkg = PackageUri::parse("package://localhost:0/birds@0.5.0").unwrap();
    let old_hardcoded =
        PklString::from("b27206b80f4f227752b6f02143887f3ea41e554542cec38f7b572b987566c4de");
    let res = accept_metadata(
        &pkg,
        &"https://localhost:0/birds@0.5.0".into(),
        birds,
        Some(&old_hardcoded),
        false,
    );
    assert!(
        matches!(res, Err(MetadataFailure::InvalidMetadataChecksum(_))),
        "old hardcoded birds checksum hack must fail verification"
    );
}

// ---------------------------------------------------------------------------
// 3. ProjectDeps lockfile strict parsing & serialization
// ---------------------------------------------------------------------------

#[test]
fn project_deps_write_json_has_trailing_newline_and_strict_parser_rejects_malformed() {
    let deps = ProjectDeps::new(vec![
        (
            CanonicalPackageUri::parse("package://example.com/foo@1").unwrap(),
            ProjectDepEntry::Remote {
                uri: PackageUri::parse("projectpackage://example.com/foo@1.2.3").unwrap(),
                sha256: "deadbeef".into(),
            },
        ),
        (
            CanonicalPackageUri::parse("package://example.com/local@0").unwrap(),
            ProjectDepEntry::Local {
                uri: PackageUri::parse("projectpackage://example.com/local@0.1.0").unwrap(),
                path: "../local".to_string(),
            },
        ),
    ]);

    let serialized = deps.write_json();
    let expected = "{\n  \"schemaVersion\": 1,\n  \"resolvedDependencies\": {\n    \"package://example.com/foo@1\": {\n      \"type\": \"remote\",\n      \"uri\": \"projectpackage://example.com/foo@1.2.3\",\n      \"checksums\": {\n        \"sha256\": \"deadbeef\"\n      }\n    },\n    \"package://example.com/local@0\": {\n      \"type\": \"local\",\n      \"uri\": \"projectpackage://example.com/local@0.1.0\",\n      \"path\": \"../local\"\n    }\n  }\n}\n";
    assert_eq!(serialized, expected);

    let reparsed = ProjectDeps::parse(&serialized, "file:///tmp/PklProject.deps.json").unwrap();
    assert_eq!(reparsed, deps);

    // Unsupported schemaVersion == 2
    let bad_schema = serialized.replace("\"schemaVersion\": 1", "\"schemaVersion\": 2");
    let err = ProjectDeps::parse(&bad_schema, "file:///tmp/PklProject.deps.json").unwrap_err();
    match err {
        ModuleHostError::SecurityManagerException { message } => {
            assert!(
                message
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .contains("Unsupported schemaVersion for `PklProject.deps.json`: 2.")
            );
        }
        other => panic!("expected unsupportedProjectDepsVersion, got {other:?}"),
    }

    // Missing schemaVersion
    assert!(ProjectDeps::parse("{\"resolvedDependencies\": {}}", "deps.json").is_err());

    // Checksum component inside resolved dependency URI must be rejected
    let with_sum = serialized.replace("foo@1.2.3", "foo@1.2.3::sha256:abc");
    assert!(ProjectDeps::parse(&with_sum, "deps.json").is_err());

    // Missing checksums on remote dep
    assert!(
        ProjectDeps::parse(
            "{\"schemaVersion\":1,\"resolvedDependencies\":{\"package://a/b@1\":{\"type\":\"remote\",\"uri\":\"projectpackage://a/b@1.0.0\"}}}",
            "deps.json"
        )
        .is_err()
    );

    // Missing path on local dep
    assert!(
        ProjectDeps::parse(
            "{\"schemaVersion\":1,\"resolvedDependencies\":{\"package://a/b@1\":{\"type\":\"local\",\"uri\":\"projectpackage://a/b@1.0.0\"}}}",
            "deps.json"
        )
        .is_err()
    );

    // Non-canonical key (missing @major)
    assert!(
        ProjectDeps::parse(
            "{\"schemaVersion\":1,\"resolvedDependencies\":{\"package://a/b\":{\"type\":\"local\",\"uri\":\"projectpackage://a/b@1.0.0\",\"path\":\"..\"}}}",
            "deps.json"
        )
        .is_err()
    );
}

// ---------------------------------------------------------------------------
// 4. Settings Precedence & Project Discovery
// ---------------------------------------------------------------------------

#[test]
fn settings_precedence_covers_all_tiers_and_edge_cases() {
    let mut user_rewrites: BTreeMap<String, String> = BTreeMap::new();
    user_rewrites.insert("https://a.test/".into(), "https://user.test/".into());

    let mut proj_readers = BTreeMap::new();
    proj_readers.insert(
        "custommod".to_string(),
        EvaluatorExternalReader {
            executable: "my-reader".into(),
            arguments: vec!["--flag".into()],
            working_dir: Some("/proj".into()),
        },
    );

    let proj_settings = EvaluatorSettings {
        external_properties: vec![("k1".into(), "proj1".into()), ("k2".into(), "proj2".into())],
        external_properties_is_set: true,
        env: vec![], // Explicitly EMPTY env {} -> MUST NOT fall back to host_env!
        env_is_set: true,
        no_cache: Some(true),
        module_cache_dir: Some(PathBuf::from("/proj/cache")),
        root_dir: Some(PathBuf::from("/proj")),
        trace_mode: Some("pretty".into()),
        color: Some(EvaluatorColor::Always),
        http: Some(EvaluatorHttpSettings {
            proxy: Some(EvaluatorHttpProxy {
                address: None,
                no_proxy: Some(vec!["internal.local".into()]),
            }),
            rewrites: None,
        }),
        external_module_readers: Some(proj_readers),
        ..Default::default()
    };

    let user_settings = PklSettings {
        editor_url_scheme: "vscode://file/%{path}:%{line}:%{column}".into(),
        http: Some(EvaluatorHttpSettings {
            proxy: Some(EvaluatorHttpProxy {
                address: Some("http://user-proxy:8080".into()),
                no_proxy: Some(vec!["user.local".into()]),
            }),
            rewrites: Some(user_rewrites.clone()),
        }),
    };

    let host_env = vec![("HOST_SECRET".into(), "should_not_leak".into())];

    // Case A: CLI omits -p and -e, sets --cache-dir (overrides project noCache = true!)
    let cli_a = CliSettingsInput {
        module_cache_dir: Some(PathBuf::from("/cli/cache")),
        output_format: Some("json".into()),
        ..Default::default()
    };
    let eff_a = resolve_effective_settings(
        &cli_a,
        Some(&proj_settings),
        &user_settings,
        &host_env,
        Some(PathBuf::from("/home/.pkl/cache")),
    );
    assert_eq!(eff_a.module_cache_dir, Some(PathBuf::from("/cli/cache")));
    assert!(
        eff_a.env.is_empty(),
        "empty project env {{}} must NOT fall back to host env"
    );
    assert_eq!(
        eff_a.external_properties.get("k1").map(String::as_str),
        Some("proj1")
    );
    assert_eq!(
        eff_a
            .external_properties
            .get("pkl.outputFormat")
            .map(String::as_str),
        Some("json")
    );
    // 3-tier per-field HTTP fallback:
    // proxy.address -> user_settings, noProxy -> project, rewrites -> user_settings
    assert_eq!(
        eff_a.http_proxy_address.as_deref(),
        Some("http://user-proxy:8080")
    );
    assert_eq!(eff_a.http_no_proxy, vec!["internal.local"]);
    assert_eq!(eff_a.http_rewrites, user_rewrites);
    // Default allowedModules auto-appends external reader scheme
    assert!(
        eff_a
            .allowed_modules
            .contains(&"\\Qcustommod:\\E".to_string())
    );
    assert_eq!(
        eff_a.editor_url_scheme,
        "vscode://file/%{path}:%{line}:%{column}"
    );

    // Case B: CLI `-p` REPLACES project externalProperties (does NOT merge!)
    // and explicit `--allowed-modules` does NOT auto-append external reader scheme
    let cli_b = CliSettingsInput {
        external_properties: Some(vec![("only_cli".into(), "1".into())]),
        allowed_modules: Some(vec!["file:".into()]),
        ..Default::default()
    };
    let eff_b = resolve_effective_settings(
        &cli_b,
        Some(&proj_settings),
        &user_settings,
        &host_env,
        Some(PathBuf::from("/home/.pkl/cache")),
    );
    assert_eq!(eff_b.external_properties.len(), 1);
    assert_eq!(
        eff_b
            .external_properties
            .get("only_cli")
            .map(String::as_str),
        Some("1")
    );
    assert!(!eff_b.external_properties.contains_key("k1"));
    assert_eq!(eff_b.allowed_modules, vec!["file:"]);
    // Project `noCache = true` disables default cache when CLI didn't pass `--cache-dir`
    assert_eq!(eff_b.module_cache_dir, None);

    // Case C: `--omit-project-settings` skips PklProject evaluatorSettings
    let cli_c = CliSettingsInput {
        omit_project_settings: true,
        ..Default::default()
    };
    let eff_c = resolve_effective_settings(
        &cli_c,
        Some(&proj_settings),
        &user_settings,
        &host_env,
        Some(PathBuf::from("/home/.pkl/cache")),
    );
    assert_eq!(
        eff_c.env.get("HOST_SECRET").map(String::as_str),
        Some("should_not_leak")
    );
    assert_eq!(
        eff_c.module_cache_dir,
        Some(PathBuf::from("/home/.pkl/cache"))
    );
    assert_eq!(eff_c.http_no_proxy, vec!["user.local"]);
    assert_eq!(eff_c.trace_mode, "compact");
    assert_eq!(eff_c.color, EvaluatorColor::Auto);
}

#[test]
fn project_discovery_walks_up_from_working_dir_and_stops_at_root_dir() {
    let found = find_project_dir_with(Path::new("/repo/a/b/c"), Some(Path::new("/repo/a")), |p| {
        p == Path::new("/repo/PklProject")
    });
    assert_eq!(found, None, "must not escape rootDir");

    let found_inside =
        find_project_dir_with(Path::new("/repo/a/b/c"), Some(Path::new("/repo/a")), |p| {
            p == Path::new("/repo/a/PklProject")
        });
    assert_eq!(found_inside, Some(PathBuf::from("/repo/a")));
}

// ---------------------------------------------------------------------------
// 5. Circular local project dependency detection
// ---------------------------------------------------------------------------

#[test]
fn circular_project_dependency_single_and_multiple_rendering() {
    let mut g1 = BTreeMap::new();
    g1.insert(
        "file:///a/PklProject".to_string(),
        vec!["file:///b/PklProject".to_string()],
    );
    g1.insert(
        "file:///b/PklProject".to_string(),
        vec!["file:///a/PklProject".to_string()],
    );
    let c1 = filter_same_scheme_cycles("file:///a/PklProject", find_import_cycles(&g1));
    let msg1 = format_circular_project_dependencies_error(&c1).unwrap();
    assert_eq!(
        msg1,
        "Local project dependencies cannot be circular.\n\nCycle:\n┌─>\n│  file:///a/PklProject\n│\n│  file:///b/PklProject\n└─"
    );

    let mut g2 = g1.clone();
    g2.insert(
        "file:///c/PklProject".to_string(),
        vec!["file:///d/PklProject".to_string()],
    );
    g2.insert(
        "file:///d/PklProject".to_string(),
        vec!["file:///c/PklProject".to_string()],
    );
    let c2 = filter_same_scheme_cycles("file:///a/PklProject", find_import_cycles(&g2));
    let msg2 = format_circular_project_dependencies_error(&c2).unwrap();
    assert!(msg2.contains("The following circular imports were found."));
    assert!(msg2.contains("Cycle 1:\n"));
    assert!(msg2.contains("Cycle 2:\n"));
}

// ---------------------------------------------------------------------------
// 6. MVS Dependency Resolver, Cache Layout, Corrupt Cache, download_package
// ---------------------------------------------------------------------------

struct MockPackageTransport {
    metadata: BTreeMap<String, Vec<u8>>,
    zips: BTreeMap<String, Vec<u8>>,
    status_overrides: BTreeMap<String, u16>,
    meta_calls: Mutex<usize>,
    zip_calls: Mutex<usize>,
}

impl MockPackageTransport {
    fn new() -> Self {
        Self {
            metadata: BTreeMap::new(),
            zips: BTreeMap::new(),
            status_overrides: BTreeMap::new(),
            meta_calls: Mutex::new(0),
            zip_calls: Mutex::new(0),
        }
    }

    fn add_package(
        &mut self,
        uri: &str,
        name: &str,
        version: &str,
        deps: Vec<(&str, &str, &str)>,
        files: &[(&str, &[u8])],
    ) -> String {
        let zip_bytes = build_deterministic_package_zip(files);
        let zip_sha = sha256_hex(&zip_bytes);
        let pkg_uri = PackageUri::parse(uri).unwrap();
        let req_url = get_metadata_request_uri(&pkg_uri).to_java_utf8();
        let zip_url = format!("{req_url}.zip");
        let meta = DependencyMetadata {
            name: name.into(),
            package_uri: pkg_uri,
            version: PackageVersion::parse(version).unwrap(),
            package_zip_url: zip_url.clone().into(),
            package_zip_sha256: zip_sha.into(),
            dependencies: deps
                .into_iter()
                .map(|(dname, duri, dsha)| RemoteDependency {
                    name: dname.into(),
                    package_uri: PackageUri::parse(duri).unwrap(),
                    sha256: dsha.into(),
                })
                .collect(),
            source_code_url_scheme: None,
            source_code: None,
            documentation: None,
            license: None,
            license_text: None,
            authors: Some(vec![]),
            issue_tracker: None,
            description: None,
            annotations: vec![],
        };
        let meta_json = meta.write_json().unwrap();
        let meta_sha = sha256_hex(meta_json.as_bytes());
        self.metadata.insert(req_url, meta_json.into_bytes());
        self.zips.insert(zip_url, zip_bytes);
        meta_sha
    }
}

impl PackageTransport for MockPackageTransport {
    fn fetch_metadata(
        &self,
        _package: &PackageUri,
        request_uri: &PklString,
    ) -> Result<Vec<u8>, ModuleHostError> {
        *self.meta_calls.lock().unwrap() += 1;
        let url = request_uri.as_str().unwrap_or("");
        if let Some(&code) = self.status_overrides.get(url) {
            return Err(PackageHttpError::BadStatusCode {
                status_code: code,
                url: request_uri.clone(),
            }
            .into_host_error());
        }
        self.metadata.get(url).cloned().ok_or_else(|| {
            PackageHttpError::BadStatusCode {
                status_code: 404,
                url: request_uri.clone(),
            }
            .into_host_error()
        })
    }

    fn fetch_zip(
        &self,
        _package: &PackageUri,
        zip_url: &PklString,
    ) -> Result<Vec<u8>, ModuleHostError> {
        *self.zip_calls.lock().unwrap() += 1;
        let url = zip_url.as_str().unwrap_or("");
        self.zips.get(url).cloned().ok_or(ModuleHostError::NotFound)
    }

    fn fetch_metadata_with_status(
        &self,
        _package: &PackageUri,
        request_uri: &PklString,
    ) -> Result<Vec<u8>, PackageHttpError> {
        *self.meta_calls.lock().unwrap() += 1;
        let url = request_uri.as_str().unwrap_or("");
        if let Some(&code) = self.status_overrides.get(url) {
            return Err(PackageHttpError::BadStatusCode {
                status_code: code,
                url: request_uri.clone(),
            });
        }
        self.metadata
            .get(url)
            .cloned()
            .ok_or_else(|| PackageHttpError::BadStatusCode {
                status_code: 404,
                url: request_uri.clone(),
            })
    }
}

#[test]
fn mvs_resolves_diamond_multi_major_and_warns_when_local_dep_overridden() {
    let mut transport = MockPackageTransport::new();
    let c12_sha = transport.add_package(
        "package://localhost:0/c@1.2.0",
        "c",
        "1.2.0",
        vec![],
        &[("c.pkl", b"v = \"1.2.0\"\n")],
    );
    let c15_sha = transport.add_package(
        "package://localhost:0/c@1.5.0",
        "c",
        "1.5.0",
        vec![],
        &[("c.pkl", b"v = \"1.5.0\"\n")],
    );
    let c20_sha = transport.add_package(
        "package://localhost:0/c@2.0.0",
        "c",
        "2.0.0",
        vec![],
        &[("c.pkl", b"v = \"2.0.0\"\n")],
    );
    let _a_sha = transport.add_package(
        "package://localhost:0/a@1.0.0",
        "a",
        "1.0.0",
        vec![("c", "package://localhost:0/c@1.2.0", &c12_sha)],
        &[("a.pkl", b"a = 1\n")],
    );
    let _b_sha = transport.add_package(
        "package://localhost:0/b@1.0.0",
        "b",
        "1.0.0",
        vec![
            ("c", "package://localhost:0/c@1.5.0", &c15_sha),
            ("c2", "package://localhost:0/c@2.0.0", &c20_sha),
        ],
        &[("b.pkl", b"b = 1\n")],
    );

    let resolver = PackageResolver::in_memory(Arc::new(transport), false);

    // Root project depends on remote a@1.0.0, remote b@1.0.0, and a LOCAL c@1.1.0 (overridden by remote c@1.5.0!)
    let root_project = Project {
        project_file_uri: "file:///ws/root/PklProject".into(),
        project_base_uri: "file:///ws/root/".into(),
        project_dir: PathBuf::from("/ws/root"),
        package: None,
        declared_dependencies: DeclaredDependenciesTree {
            project_file_uri: "file:///ws/root/PklProject".into(),
            package_uri: None,
            remote_dependencies: vec![
                DeclaredRemoteDep {
                    name: "a".into(),
                    package_uri: PackageUri::parse("package://localhost:0/a@1.0.0").unwrap(),
                    sha256: None,
                },
                DeclaredRemoteDep {
                    name: "b".into(),
                    package_uri: PackageUri::parse("package://localhost:0/b@1.0.0").unwrap(),
                    sha256: None,
                },
            ],
            local_dependencies: vec![(
                "localC".into(),
                DeclaredDependenciesTree {
                    project_file_uri: "file:///ws/local_c/PklProject".into(),
                    package_uri: Some(PackageUri::parse("package://localhost:0/c@1.1.0").unwrap()),
                    remote_dependencies: vec![],
                    local_dependencies: vec![],
                },
            )],
        },
        evaluator_settings: None,
        resolved_evaluator_settings: None,
        tests: vec![],
        local_project_dependencies: vec![],
        annotations: vec![],
    };

    let result = ProjectDependenciesResolver::new(&root_project, &resolver)
        .resolve()
        .unwrap();

    // Diamond resolved to c@1.5.0 (not 1.2.0 or local 1.1.0)
    let c1 = result
        .deps
        .get(&CanonicalPackageUri::parse("package://localhost:0/c@1").unwrap())
        .unwrap();
    assert_eq!(c1.package_uri().uri, "projectpackage://localhost:0/c@1.5.0");
    assert!(!c1.is_local());

    // Multi-major c@2.0.0 coexists cleanly
    let c2 = result
        .deps
        .get(&CanonicalPackageUri::parse("package://localhost:0/c@2").unwrap())
        .unwrap();
    assert_eq!(c2.package_uri().uri, "projectpackage://localhost:0/c@2.0.0");

    // Local-overridden warning emitted
    assert_eq!(result.warnings.len(), 1);
    assert_eq!(
        result.warnings[0],
        "WARN: local dependency `package://localhost:0/c@1.1.0` was overridden to remote dependency `package://localhost:0/c@1.5.0`."
    );

    // Also test declared checksum mismatch (`invalidDeclaredChecksum`)
    let mut bad_root = root_project.clone();
    bad_root.declared_dependencies.remote_dependencies[0].sha256 = Some("0000bad".into());
    let err = ProjectDependenciesResolver::new(&bad_root, &resolver)
        .resolve()
        .unwrap_err();
    match err {
        ModuleHostError::SecurityManagerException { message } => {
            assert!(
                message
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .contains("Computed checksum did not match declared checksum for dependency `package://localhost:0/a@1.0.0`.")
            );
        }
        other => panic!("expected invalidDeclaredChecksum, got {other:?}"),
    }
}

#[test]
fn cache_path_encoding_cold_cache_hits_corrupt_eviction_and_download_package() {
    assert_eq!(encode_cache_path("localhost:0"), "localhost(3a)0");
    assert_eq!(encode_cache_path("a(b):c*d"), "a((b)(3a)c(2a)d");

    let pkg = PackageUri::parse("package://localhost:0/birds@0.5.0").unwrap();
    let rel = get_cache_rel_path(&pkg).unwrap();
    assert_eq!(rel, PathBuf::from("package-2/localhost(3a)0/birds@0.5.0"));
    let paths = PackageCachePaths::for_package(&pkg).unwrap();
    assert_eq!(
        paths.metadata_file,
        PathBuf::from("package-2/localhost(3a)0/birds@0.5.0/birds@0.5.0.json")
    );
    assert_eq!(
        paths.zip_file,
        PathBuf::from("package-2/localhost(3a)0/birds@0.5.0/birds@0.5.0.zip")
    );

    // Cold cache read + download_package transitive vs --no-transitive
    let mut transport = MockPackageTransport::new();
    let fruit_sha = transport.add_package(
        "package://localhost:0/fruit@1.0.5",
        "fruit",
        "1.0.5",
        vec![],
        &[("Fruit.pkl", b"name = \"Apple\"\n")],
    );
    let _birds_sha = transport.add_package(
        "package://localhost:0/birds@0.5.0",
        "birds",
        "0.5.0",
        vec![("fruities", "package://localhost:0/fruit@1.0.5", &fruit_sha)],
        &[("Bird.pkl", b"name = \"Swallow\"\n")],
    );
    let transport = Arc::new(transport);
    let store = Arc::new(InMemoryPackageCacheStore::new());

    // Download with --no-transitive -> only birds cached
    let resolver1 = PackageResolver::disk_cached(transport.clone(), store.clone(), false);
    let dl1 = resolver1.download_package(&pkg, None, true).unwrap();
    assert_eq!(dl1.len(), 1);
    assert!(store.has_metadata(&pkg));
    assert!(store.has_zip(&pkg));
    let fruit_pkg = PackageUri::parse("package://localhost:0/fruit@1.0.5").unwrap();
    assert!(!store.has_metadata(&fruit_pkg));

    // Fresh cold resolver reads birds from persistent store with ZERO network requests!
    let meta_before = *transport.meta_calls.lock().unwrap();
    let zip_before = *transport.zip_calls.lock().unwrap();
    let resolver2 = PackageResolver::disk_cached(transport.clone(), store.clone(), false);
    let asset = pkg.to_package_asset_uri("/Bird.pkl").unwrap();
    let bytes = resolver2.get_bytes(&asset, false, None).unwrap();
    assert_eq!(bytes, b"name = \"Swallow\"\n");
    assert_eq!(*transport.meta_calls.lock().unwrap(), meta_before);
    assert_eq!(*transport.zip_calls.lock().unwrap(), zip_before);

    // Full transitive download fetches fruit too
    let dl2 = resolver2.download_package(&pkg, None, false).unwrap();
    assert_eq!(dl2.len(), 2);
    assert!(store.has_metadata(&fruit_pkg));
    assert!(store.has_zip(&fruit_pkg));

    // Corrupt cached `.json` MUST be deleted and return error
    store.seed_raw_metadata(&pkg, b"{not valid json".to_vec());
    let resolver3 = PackageResolver::disk_cached(transport.clone(), store.clone(), false);
    assert!(resolver3.get_dependency_metadata(&pkg, None).is_err());
    assert!(
        !store.has_metadata(&pkg),
        "corrupt cached metadata JSON must be deleted"
    );

    // Corrupt cached `.zip` MUST surface error (never silently swallow!)
    store.seed_raw_zip(&fruit_pkg, b"PK\x03\x04corrupted".to_vec());
    let resolver4 = PackageResolver::disk_cached(transport.clone(), store.clone(), false);
    assert!(resolver4.get_archive(&fruit_pkg, None).is_err());

    // download_package with no cache fails
    let no_cache_resolver = PackageResolver::in_memory(transport.clone(), false);
    assert!(
        no_cache_resolver
            .download_package(&pkg, None, false)
            .is_err()
    );

    // close() rejects subsequent operations
    no_cache_resolver.close();
    assert!(no_cache_resolver.is_closed());
    assert!(
        no_cache_resolver
            .get_dependency_metadata(&pkg, None)
            .is_err()
    );
}

// ---------------------------------------------------------------------------
// 7. ProjectDependenciesManager scope & version guards
// ---------------------------------------------------------------------------

#[test]
fn project_dependencies_manager_enforces_scope_hierarchy_and_version_sync() {
    let declared = DeclaredDependenciesTree {
        project_file_uri: "file:///ws/proj/PklProject".into(),
        package_uri: None,
        remote_dependencies: vec![DeclaredRemoteDep {
            name: "birds".into(),
            package_uri: PackageUri::parse("package://localhost:0/birds@0.5.0").unwrap(),
            sha256: None,
        }],
        local_dependencies: vec![],
    };
    let deps = ProjectDeps::new(vec![(
        CanonicalPackageUri::parse("package://localhost:0/birds@0").unwrap(),
        ProjectDepEntry::Remote {
            uri: PackageUri::parse("projectpackage://localhost:0/birds@0.5.0").unwrap(),
            sha256: "abc".into(),
        },
    )]);
    let mgr =
        ProjectDependenciesManager::with_deps(declared, PathBuf::from("/ws/proj"), deps).unwrap();

    assert!(mgr.has_uri("file:///ws/proj/main.pkl"));
    assert!(!mgr.has_uri("file:///ws/other/main.pkl"));

    // Inside project succeeds
    let (dep, asset) = mgr
        .resolve_dependency_notation("file:///ws/proj/main.pkl", true, "@birds/Bird.pkl")
        .unwrap();
    assert_eq!(
        dep.package_uri().uri,
        "projectpackage://localhost:0/birds@0.5.0"
    );
    assert_eq!(asset, "/Bird.pkl");

    // Outside project fails with cannotResolveDependencyNoProject
    let err_out = mgr
        .resolve_dependency_notation("file:///ws/other/main.pkl", true, "@birds/Bird.pkl")
        .unwrap_err();
    match err_out {
        ModuleHostError::SecurityManagerException { message } => {
            assert!(
                message
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .contains("Cannot resolve dependency because there is no project found.")
            );
        }
        other => panic!("unexpected error: {other:?}"),
    }

    // Non-hierarchical URI (e.g. repl:text) fails with cannotResolveDependencyWithoutHierarchicalUris
    let err_opaque = mgr
        .resolve_dependency_notation("repl:text", false, "@birds/Bird.pkl")
        .unwrap_err();
    match err_opaque {
        ModuleHostError::SecurityManagerException { message } => {
            assert!(
                message
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .contains("does not have a hierarchical path.")
            );
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// 8. Strict ZIP validation & deterministic pure-Rust DEFLATE writer
// ---------------------------------------------------------------------------

#[test]
fn deterministic_package_zip_empty_sha256_deflate_roundtrip_and_strict_corruption_checks() {
    // Empty ZIP exact SHA-256 matching JDK ZipOutputStream (`CliProjectPackagerTest.kt`)
    let empty_zip = build_deterministic_package_zip(&[]);
    assert_eq!(empty_zip.len(), 22);
    assert_eq!(
        sha256_hex(&empty_zip),
        "8739c76e681f900923b900c9df0ef75cf421d39cabb54650c4b9ad19b6a76d85"
    );

    // Repetitive Pkl content compresses significantly and round-trips losslessly
    let pkl_text = "amends \"pkl:base\"\n".repeat(200);
    let compressed = deflate_raw(pkl_text.as_bytes());
    assert!(
        compressed.len() < pkl_text.len() / 5,
        "expected LZ77+Huffman compression, got {} vs {}",
        compressed.len(),
        pkl_text.len()
    );

    let files = [
        ("z/last.pkl", b"z = 1\n".as_slice()),
        ("a/first.pkl", pkl_text.as_bytes()),
        ("empty.txt", b"".as_slice()),
    ];
    let zip1 = build_deterministic_package_zip(&files);
    let zip2 = build_deterministic_package_zip(&[files[2], files[0], files[1]]);
    assert_eq!(
        zip1, zip2,
        "ZIP output must be 100% deterministic regardless of input order"
    );

    let archive = ZipArchive::parse(&zip1).unwrap();
    assert_eq!(archive.get("/a/first.pkl").unwrap(), pkl_text.as_bytes());
    assert_eq!(archive.get("/z/last.pkl").unwrap(), b"z = 1\n");
    assert_eq!(archive.get("/empty.txt").unwrap(), b"");
    assert_eq!(
        archive.entry_dos_mtime("/a/first.pkl"),
        Some((PACKAGE_ZIP_DOS_TIME, PACKAGE_ZIP_DOS_DATE))
    );
    assert_eq!(PACKAGE_ZIP_DOS_TIME, 0x0000);
    assert_eq!(PACKAGE_ZIP_DOS_DATE, 0x0041);

    // Strict corruption detection: CRC bit-flip
    let mut bad_crc = zip1.clone();
    // Flip a byte in the central directory CRC-32 of the first entry
    let eocd_pos = bad_crc.len() - 22;
    let cd_offset = u32::from_le_bytes([
        bad_crc[eocd_pos + 16],
        bad_crc[eocd_pos + 17],
        bad_crc[eocd_pos + 18],
        bad_crc[eocd_pos + 19],
    ]) as usize;
    bad_crc[cd_offset + 16] ^= 0xff;
    assert!(matches!(
        ZipArchive::parse(&bad_crc),
        Err(ZipError::CorruptedData(_))
    ));

    // Strict corruption detection: truncated ZIP
    assert!(ZipArchive::parse(&zip1[..zip1.len() - 5]).is_err());
    assert!(ZipArchive::parse(&zip1[..10]).is_err());
}

// ---------------------------------------------------------------------------
// 9. ProjectPackager: excludes, import/read validator, 4-artifact stdout order, publish checks
// ---------------------------------------------------------------------------

#[test]
fn packager_exclude_globs_and_import_read_escape_validation() {
    let matchers: Vec<ExcludeGlobMatcher> = ["PklProject", "PklProject.deps.json", ".**", "*.tmp"]
        .iter()
        .map(|p| ExcludeGlobMatcher::compile(p).unwrap())
        .collect();
    assert!(is_file_excluded("PklProject", &matchers));
    assert!(is_file_excluded("PklProject.deps.json", &matchers));
    assert!(is_file_excluded(".git/config", &matchers));
    assert!(is_file_excluded("sub/.hidden", &matchers));
    assert!(is_file_excluded("sub/cache.tmp", &matchers));
    assert!(!is_file_excluded("sub/Bird.pkl", &matchers));

    let root = Path::new("/ws/myproj");
    // Valid imports & reads
    validate_pkl_imports_and_reads(
        root,
        "sub/a.pkl",
        r#"
        import "../main.pkl"
        import "@birds/Bird.pkl"
        import "https://example.com/x.pkl"
        x = read("../data/file.txt")
        y = read?("local.txt")
        z = read*("*.txt")
        "#,
    )
    .unwrap();

    // Direct escape rejected
    assert!(validate_pkl_imports_and_reads(root, "main.pkl", "import \"../outside.pkl\"").is_err());
    // Temporary escape back into project root rejected! (`ProjectPackager.java:422-437`)
    assert!(
        validate_pkl_imports_and_reads(root, "main.pkl", "import \"../myproj/main.pkl\"").is_err()
    );
    // Absolute path rejected
    assert!(validate_pkl_imports_and_reads(root, "main.pkl", "import \"/etc/passwd\"").is_err());
    // Escaping read() rejected
    assert!(
        validate_pkl_imports_and_reads(root, "sub/a.pkl", "x = read(\"../../secret.txt\")")
            .is_err()
    );
}

struct InMemoryPackagerHost {
    files_by_dir: BTreeMap<PathBuf, Vec<(String, Vec<u8>)>>,
    deps_by_dir: BTreeMap<PathBuf, String>,
    written: Mutex<BTreeMap<PathBuf, Vec<u8>>>,
    transport: Option<MockPackageTransport>,
}

impl PackagerHost for InMemoryPackagerHost {
    fn list_project_files(
        &self,
        project_dir: &Path,
    ) -> Result<Vec<(String, Vec<u8>)>, ModuleHostError> {
        Ok(self
            .files_by_dir
            .get(project_dir)
            .cloned()
            .unwrap_or_default())
    }

    fn read_project_deps_json(
        &self,
        project_dir: &Path,
    ) -> Result<Option<String>, ModuleHostError> {
        Ok(self.deps_by_dir.get(project_dir).cloned())
    }

    fn write_artifact(&self, path: &Path, bytes: &[u8]) -> Result<(), ModuleHostError> {
        self.written
            .lock()
            .unwrap()
            .insert(path.to_path_buf(), bytes.to_vec());
        Ok(())
    }

    fn transport(&self) -> Option<&dyn PackageTransport> {
        self.transport.as_ref().map(|t| t as &dyn PackageTransport)
    }
}

#[test]
fn project_packager_emits_4_artifacts_exact_032_stdout_order_and_publish_checks() {
    let proj_a = Project {
        project_file_uri: "file:///ws/projA/PklProject".into(),
        project_base_uri: "file:///ws/projA/".into(),
        project_dir: PathBuf::from("/ws/projA"),
        package: Some(PackageSpec {
            name: "projA".into(),
            base_uri: "package://example.com/projA".into(),
            version: PackageVersion::parse("1.0.0").unwrap(),
            uri: PackageUri::parse("package://example.com/projA@1.0.0").unwrap(),
            package_zip_url: "https://example.com/projA@1.0.0.zip".into(),
            description: Some("Project A".into()),
            authors: vec!["dev@example.com".into()],
            website: None,
            documentation: None,
            source_code: None,
            source_code_url_scheme: None,
            license: Some("Apache-2.0".into()),
            license_text: None,
            issue_tracker: None,
            api_tests: vec![],
            exclude: vec![
                "PklProject".into(),
                "PklProject.deps.json".into(),
                ".**".into(),
            ],
        }),
        declared_dependencies: DeclaredDependenciesTree {
            project_file_uri: "file:///ws/projA/PklProject".into(),
            package_uri: Some(PackageUri::parse("package://example.com/projA@1.0.0").unwrap()),
            remote_dependencies: vec![],
            local_dependencies: vec![],
        },
        evaluator_settings: None,
        resolved_evaluator_settings: None,
        tests: vec![],
        local_project_dependencies: vec![],
        annotations: vec![],
    };

    let proj_b = Project {
        project_file_uri: "file:///ws/projB/PklProject".into(),
        project_base_uri: "file:///ws/projB/".into(),
        project_dir: PathBuf::from("/ws/projB"),
        package: Some(PackageSpec {
            name: "projB".into(),
            base_uri: "package://example.com/projB".into(),
            version: PackageVersion::parse("2.0.0").unwrap(),
            uri: PackageUri::parse("package://example.com/projB@2.0.0").unwrap(),
            package_zip_url: "https://example.com/projB@2.0.0.zip".into(),
            description: None,
            authors: vec![],
            website: None,
            documentation: None,
            source_code: None,
            source_code_url_scheme: None,
            license: None,
            license_text: None,
            issue_tracker: None,
            api_tests: vec![],
            exclude: vec![
                "PklProject".into(),
                "PklProject.deps.json".into(),
                ".**".into(),
            ],
        }),
        declared_dependencies: DeclaredDependenciesTree {
            project_file_uri: "file:///ws/projB/PklProject".into(),
            package_uri: Some(PackageUri::parse("package://example.com/projB@2.0.0").unwrap()),
            remote_dependencies: vec![],
            local_dependencies: vec![("depA".into(), proj_a.declared_dependencies.clone())],
        },
        evaluator_settings: None,
        resolved_evaluator_settings: None,
        tests: vec![],
        local_project_dependencies: vec![("depA".into(), proj_a.clone())],
        annotations: vec![],
    };

    // Omitting local dep projA must fail with missingProjectInPackageCommand
    assert!(validate_local_projects_included(std::slice::from_ref(&proj_b)).is_err());

    let mut files_by_dir = BTreeMap::new();
    files_by_dir.insert(
        PathBuf::from("/ws/projA"),
        vec![
            ("PklProject".into(), b"amends \"pkl:Project\"".to_vec()),
            ("main.pkl".into(), b"a = 1\n".to_vec()),
        ],
    );
    files_by_dir.insert(
        PathBuf::from("/ws/projB"),
        vec![
            ("PklProject".into(), b"amends \"pkl:Project\"".to_vec()),
            (
                "main.pkl".into(),
                b"import \"@depA/main.pkl\"\nb = main.a\n".to_vec(),
            ),
        ],
    );

    let deps_b = ProjectDeps::new(vec![(
        CanonicalPackageUri::parse("package://example.com/projA@1").unwrap(),
        ProjectDepEntry::Local {
            uri: PackageUri::parse("projectpackage://example.com/projA@1.0.0").unwrap(),
            path: "../projA".into(),
        },
    )]);
    let mut deps_by_dir = BTreeMap::new();
    deps_by_dir.insert(PathBuf::from("/ws/projB"), deps_b.write_json());

    let host = InMemoryPackagerHost {
        files_by_dir,
        deps_by_dir,
        written: Mutex::new(BTreeMap::new()),
        transport: Some(MockPackageTransport::new()), // 404 on publish check -> OK
    };

    let (results, stdout_lines) = ProjectPackager::new(
        &[proj_a.clone(), proj_b.clone()],
        PathBuf::from("/ws"),
        ".out/%{name}@%{version}",
        false, // run publish check (returns 404 -> allowed!)
        &host,
    )
    .create_packages()
    .unwrap();

    assert_eq!(results.len(), 2);
    // Exact Pkl 0.32.0 stdout order: zip, zip.sha256, metadata, metadata.sha256
    assert_eq!(
        stdout_lines,
        vec![
            ".out/projA@1.0.0/projA@1.0.0.zip",
            ".out/projA@1.0.0/projA@1.0.0.zip.sha256",
            ".out/projA@1.0.0/projA@1.0.0",
            ".out/projA@1.0.0/projA@1.0.0.sha256",
            ".out/projB@2.0.0/projB@2.0.0.zip",
            ".out/projB@2.0.0/projB@2.0.0.zip.sha256",
            ".out/projB@2.0.0/projB@2.0.0",
            ".out/projB@2.0.0/projB@2.0.0.sha256",
        ]
    );
    // projB's metadata records projA's exact computed metadata checksum!
    assert!(
        results[1]
            .metadata_json
            .contains(&results[0].metadata_sha256)
    );
    // authors: [] emitted even when empty
    assert!(results[1].metadata_json.contains("\"authors\": []"));

    // Publish check conflict when already published with different bytes
    let mut conflict_transport = MockPackageTransport::new();
    conflict_transport.metadata.insert(
        "https://example.com/projA@1.0.0".into(),
        b"different published metadata".to_vec(),
    );
    let conflict_host = InMemoryPackagerHost {
        files_by_dir: host.files_by_dir.clone(),
        deps_by_dir: host.deps_by_dir.clone(),
        written: Mutex::new(BTreeMap::new()),
        transport: Some(conflict_transport),
    };
    let conflict_err = ProjectPackager::new(
        std::slice::from_ref(&proj_a),
        PathBuf::from("/ws"),
        ".out/%{name}@%{version}",
        false,
        &conflict_host,
    )
    .create_packages()
    .unwrap_err();
    match conflict_err {
        ModuleHostError::EvalError { message, .. } => {
            assert!(
                message
                    .as_str()
                    .unwrap()
                    .contains("was already published with different contents.")
            );
        }
        other => panic!("expected publish conflict error, got {other:?}"),
    }

    // Check relativize_paths helper
    assert_eq!(
        relativize_paths(Path::new("/a/b/c"), Path::new("/a/d")),
        "../b/c"
    );
}

// ---------------------------------------------------------------------------
// 10. Minimal pkl:test runner & SpecReporter
// ---------------------------------------------------------------------------

#[test]
fn test_runner_facts_examples_and_spec_reporter_formatting() {
    let facts_val = DecodedValue::Mapping(vec![
        (
            DecodedValue::String("math".into()),
            DecodedValue::Listing(vec![DecodedValue::Bool(true), DecodedValue::Bool(true)]),
        ),
        (
            DecodedValue::String("broken".into()),
            DecodedValue::Listing(vec![DecodedValue::Bool(false)]),
        ),
    ]);
    let facts = evaluate_facts_decoded(Some(&facts_val), "file:///t.pkl");
    assert_eq!(facts.total_tests(), 2);
    assert_eq!(facts.total_asserts(), 3);
    assert_eq!(facts.total_failures(), 1);
    assert_eq!(facts.total_asserts_failed_or_errored(), 1);

    let (examples_ok, _, _) = evaluate_examples_rendered(
        &[("ex1".into(), vec!["1".into()])],
        Some(&[("ex1".into(), vec!["1".into()])]),
        "file:///t.pkl",
        "file:///t.pkl-expected.pcf",
        "file:///t.pkl-actual.pcf",
        None,
    );
    assert!(!examples_ok.failed());

    let res = TestResults {
        module_name: "myTest".into(),
        display_uri: "file:///t.pkl".into(),
        facts,
        examples: examples_ok,
        logs: String::new(),
        error: None,
        generated_expected_pcf: None,
        generated_actual_pcf: None,
    };
    let report = render_spec_report(&res);
    assert!(report.contains("module myTest\n"));
    assert!(report.contains("    ✔ math\n"));
    assert!(report.contains("    ✘ broken\n"));
    assert!(report.contains("    ✔ ex1\n"));

    // 2/3 tests pass -> 66.6% (RoundingMode.DOWN!), 3/4 asserts pass -> 75.0%
    let summary = render_spec_summary(&[res]);
    assert_eq!(
        summary,
        "66.6% tests pass [1/3 failed], 75.0% asserts pass [1/4 failed]\n"
    );

    // Written examples summary
    let (written_ex, gen_exp, _) = evaluate_examples_rendered(
        &[("newEx".into(), vec!["\"hi\"".into()])],
        None,
        "file:///t.pkl",
        "file:///t.pkl-expected.pcf",
        "file:///t.pkl-actual.pcf",
        Some("examples {\n  [\"newEx\"] {\n    \"hi\"\n  }\n}\n".into()),
    );
    assert!(gen_exp.is_some());
    let written_res = TestResults {
        module_name: "newTest".into(),
        display_uri: "file:///t.pkl".into(),
        facts: TestSectionResults::empty(TestSectionName::Facts),
        examples: written_ex,
        logs: String::new(),
        error: None,
        generated_expected_pcf: gen_exp,
        generated_actual_pcf: None,
    };
    assert!(written_res.is_example_written_failure());
    assert_eq!(render_spec_summary(&[written_res]), "1 examples written\n");
}

// ---------------------------------------------------------------------------
// 11. Live end-to-end native project_probe tests (settings, local deps, package, cycle)
// ---------------------------------------------------------------------------

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn native_end_to_end_settings_local_deps_resolve_package_and_cycle_detection() {
    use pkl_runtime::evaluation::session::{SourceSession, SourceSessionOptions};
    use pkl_runtime::host::HostPoll;
    use pkl_runtime::logging::{LogDelivery, TraceMode};
    use pkl_runtime::pkl_binary_decode::decode_lossless;
    use pkl_runtime::source::SourceText;

    // 1. Live `pkl:settings` evaluation via SourceSession + pkl-binary -> PklSettings::from_decoded_value
    let settings_src = r#"
amends "pkl:settings"
editor = VsCode
http {
  proxy {
    address = "http://127.0.0.1:8888"
    noProxy { "localhost"; ".internal.test" }
  }
  rewrites {
    ["https://orig.test/"] = "https://mirror.test/"
  }
}
"#;
    let mut session = SourceSession::compile_expressions_text(
        SourceText::from(settings_src),
        PklString::from("file:///tmp/settings.pkl"),
        PklString::from("settings"),
        &["output.bytes".to_string()],
        SourceSessionOptions {
            evaluation: pkl_runtime::evaluation::EvaluationOptions {
                power_assertions: true,
            },
            trace_mode: TraceMode::Compact,
            log_delivery: LogDelivery::Host,
            frame_transformer: pkl_runtime::logging::FrameTransformer::Default,
            platform_data: true,
        },
    )
    .unwrap();
    session.start_path(0, 0, None).unwrap();
    let props = vec![("pkl.outputFormat".to_string(), "pkl-binary".to_string())];
    let val = loop {
        match session.poll(0, 1024) {
            HostPoll::Ready(res) => break res.unwrap(),
            HostPoll::Yielded => {}
            HostPoll::Module { token, request } => {
                let comp = pkl_runtime::modules::default_or_unsupported_completion(&request);
                session.complete_module(&token, comp, 0).unwrap();
            }
            HostPoll::Read { token, .. } => {
                session.resume_default_read(&token, &props, &[], 0).unwrap();
            }
            HostPoll::Glob { token, request } => {
                let comp =
                    pkl_runtime::glob::configured_or_unsupported_completion(&request, &props, &[]);
                session.complete_glob(&token, comp, 0).unwrap();
            }
            HostPoll::Log { token, .. } => session.acknowledge_log(&token, Ok(()), 0).unwrap(),
            HostPoll::TransformFrame { token, frame } => {
                session
                    .complete_frame_transform(&token, Ok((*frame).clone()), 0)
                    .unwrap();
            }
        }
    };
    let bytes = SourceSession::result_bytes(val).unwrap();
    let decoded = decode_lossless(&bytes).unwrap();
    let parsed_settings =
        PklSettings::from_decoded_value(&decoded, "file:///tmp/settings.pkl").unwrap();
    assert_eq!(
        parsed_settings.editor_url_scheme,
        "vscode://file/%{path}:%{line}:%{column}"
    );
    let http = parsed_settings.http.as_ref().unwrap();
    assert_eq!(
        http.proxy.as_ref().unwrap().address.as_deref(),
        Some("http://127.0.0.1:8888")
    );
    assert_eq!(
        http.proxy.as_ref().unwrap().no_proxy.as_ref().unwrap(),
        &vec!["localhost".to_string(), ".internal.test".to_string()]
    );
    assert_eq!(
        http.rewrites
            .as_ref()
            .unwrap()
            .get("https://orig.test/")
            .map(String::as_str),
        Some("https://mirror.test/")
    );

    // 2. Live `PklProject` evaluation via SourceSession + pkl-binary -> Project::from_decoded_value
    let project_src = r#"
@Deprecated { since = "0.2.0"; message = "use v2" }
amends "pkl:Project"

package {
  name = "demo"
  baseUri = "package://example.com/demo"
  version = "1.2.3"
  packageZipUrl = "https://example.com/demo@1.2.3.zip"
  description = "Demo package"
  authors { "Jane Doe <jane@example.com>" }
  license = "Apache-2.0"
}

evaluatorSettings {
  timeout = 15.s
  color = "always"
  noCache = true
  env {}
  externalProperties {
    ["mode"] = "prod"
  }
  http {
    proxy {
      address = "http://127.0.0.1:9999"
    }
  }
}
"#;
    let mut psession = SourceSession::compile_expressions_text(
        SourceText::from(project_src),
        PklString::from("file:///ws/demo/PklProject"),
        PklString::from("PklProject"),
        &["output.bytes".to_string()],
        SourceSessionOptions {
            evaluation: pkl_runtime::evaluation::EvaluationOptions {
                power_assertions: true,
            },
            trace_mode: TraceMode::Compact,
            log_delivery: LogDelivery::Host,
            frame_transformer: pkl_runtime::logging::FrameTransformer::Default,
            platform_data: true,
        },
    )
    .unwrap();
    psession.start_path(0, 0, None).unwrap();
    let pval = loop {
        match psession.poll(0, 1024) {
            HostPoll::Ready(res) => break res.unwrap(),
            HostPoll::Yielded => {}
            HostPoll::Module { token, request } => {
                let comp = pkl_runtime::modules::default_or_unsupported_completion(&request);
                psession.complete_module(&token, comp, 0).unwrap();
            }
            HostPoll::Read { token, .. } => {
                psession
                    .resume_default_read(&token, &props, &[], 0)
                    .unwrap();
            }
            HostPoll::Glob { token, request } => {
                let comp =
                    pkl_runtime::glob::configured_or_unsupported_completion(&request, &props, &[]);
                psession.complete_glob(&token, comp, 0).unwrap();
            }
            HostPoll::Log { token, .. } => psession.acknowledge_log(&token, Ok(()), 0).unwrap(),
            HostPoll::TransformFrame { token, frame } => {
                psession
                    .complete_frame_transform(&token, Ok((*frame).clone()), 0)
                    .unwrap();
            }
        }
    };
    let pbytes = SourceSession::result_bytes(pval).unwrap();
    let pdecoded = decode_lossless(&pbytes).unwrap();
    let project = Project::from_decoded_value(&pdecoded).unwrap();
    let pkg = project.package.as_ref().unwrap();
    assert_eq!(pkg.name, "demo");
    assert_eq!(pkg.uri.uri, "package://example.com/demo@1.2.3");
    assert_eq!(project.annotations.len(), 1);
    assert_eq!(project.annotations[0].class_info.class_name, "Deprecated");
    let resolved_settings = project.resolved_evaluator_settings.as_ref().unwrap();
    assert_eq!(resolved_settings.color, Some(EvaluatorColor::Always));
    assert_eq!(resolved_settings.timeout_nanos, Some(15_000_000_000));
    assert_eq!(resolved_settings.no_cache, Some(true));
    assert!(resolved_settings.env_is_set);
    assert!(resolved_settings.env.is_empty());
    assert_eq!(
        resolved_settings
            .http
            .as_ref()
            .unwrap()
            .proxy
            .as_ref()
            .unwrap()
            .address
            .as_deref(),
        Some("http://127.0.0.1:9999")
    );
}

#[test]
fn wave2_review_regressions_defaults_checksums_zip_uris_facts_and_external_reader_unification() {
    use pkl_runtime::external_reader::ExternalReaderConfig;
    use pkl_runtime::packages::cache::PackageCacheStore;
    use pkl_runtime::packages::mvs::MetadataChecksumProvider;
    use pkl_runtime::packages::packager::validate_pkl_imports_and_reads;
    use pkl_runtime::packages::project::{DEFAULT_ALLOWED_MODULES, ProjectDependenciesManager};
    use pkl_runtime::packages::test_runner::evaluate_facts_decoded_with_source;
    use pkl_runtime::packages::zip::ZipArchive;

    // 1. [CRIT] DEFAULT_ALLOWED_MODULES includes jar:file: and jar:nested:
    assert!(DEFAULT_ALLOWED_MODULES.contains(&"jar:file:"));
    assert!(DEFAULT_ALLOWED_MODULES.contains(&"jar:nested:"));

    // 2. ExternalReaderConfig unified with EvaluatorExternalReader (same type)
    let reader: EvaluatorExternalReader = ExternalReaderConfig::new(
        "/usr/bin/my-reader",
        vec!["--flag".to_string()],
        Some("/tmp".to_string()),
    );
    let back: ExternalReaderConfig = reader.clone();
    assert_eq!(back.executable, "/usr/bin/my-reader");

    // 3. [HIGH] Same-session checksum mismatch is never bypassed by PackageCache
    let mut transport = MockPackageTransport::new();
    let meta_sha = transport.add_package(
        "package://example.com/alpha@1.0.0",
        "alpha",
        "1.0.0",
        vec![],
        &[("mod.pkl", b"x = 1\n")],
    );
    let cache_store = Arc::new(InMemoryPackageCacheStore::new());
    let resolver = PackageResolver::disk_cached(Arc::new(transport), cache_store.clone(), false);

    // First load without checksum populates memory cache (and does NOT write disk via MetadataChecksumProvider)
    let alpha_no_sha = PackageUri::parse("package://example.com/alpha@1.0.0").unwrap();
    let (_, computed_sha) = resolver
        .get_dependency_metadata_and_checksum(&alpha_no_sha)
        .unwrap();
    assert_eq!(computed_sha.as_str(), Some(meta_sha.as_str()));
    // 7. [LOW] MetadataChecksumProvider must NOT write `.json` into disk cache on miss
    assert!(
        cache_store
            .read_metadata_bytes(&alpha_no_sha)
            .unwrap()
            .is_none(),
        "project resolve metadata probe must not write `.json` to disk cache"
    );

    // Same session: requesting with bad embedded checksum MUST fail
    let alpha_bad_sha =
        PackageUri::parse("package://example.com/alpha@1.0.0::sha256:0000000000000000").unwrap();
    let err = resolver
        .get_dependency_metadata(&alpha_bad_sha, None)
        .unwrap_err();
    assert!(
        format!("{err:?}").contains("does not match the expected checksum"),
        "expected checksum mismatch for embedded bad sha256, got: {err:?}"
    );
    // Same session: requesting same PackageUri without embedded sha but with bad expected_checksum MUST fail
    let bad_expected = PklString::from("badbadbad");
    let err2 = resolver
        .get_dependency_metadata(&alpha_no_sha, Some(&bad_expected))
        .unwrap_err();
    assert!(
        format!("{err2:?}").contains("does not match the expected checksum"),
        "expected checksum mismatch for explicit bad expected_checksum, got: {err2:?}"
    );
    // Same session: requesting with matching expected_checksum succeeds
    let ok_expected = PklString::from(meta_sha.as_str());
    resolver
        .get_dependency_metadata(&alpha_no_sha, Some(&ok_expected))
        .unwrap();

    // 4. [HIGH] test_runner renders source expression + 1-based line number for failing facts
    let fail_src = "amends \"pkl:test\"\nfacts {\n  [\"must fail\"] {\n    1 == 2\n  }\n}\n";
    let facts_val = DecodedValue::Mapping(vec![(
        DecodedValue::String("must fail".into()),
        DecodedValue::Listing(vec![DecodedValue::Bool(false)]),
    )]);
    let section = evaluate_facts_decoded_with_source(
        Some(&facts_val),
        "file:///proj/fail_test.pkl",
        Some(fail_src),
    );
    assert_eq!(section.results.len(), 1);
    assert_eq!(
        section.results[0].failures[0].message,
        "1 == 2 (file:///proj/fail_test.pkl, line 4)"
    );

    // 5. [MED] ZipArchive::parse rejects trailing garbage / fake EOCD and trusts CD compressed size
    let mut valid_zip = build_deterministic_package_zip(&[("empty.txt", b"")]);
    assert_eq!(
        ZipArchive::parse(&valid_zip)
            .unwrap()
            .read_bytes("/empty.txt", false)
            .unwrap(),
        b""
    );
    // Corrupt local header compressed size to non-zero while CD keeps authoritative 0
    valid_zip[18..22].copy_from_slice(&99u32.to_le_bytes());
    assert_eq!(
        ZipArchive::parse(&valid_zip)
            .unwrap()
            .read_bytes("/empty.txt", false)
            .unwrap(),
        b""
    );
    // Appending trailing garbage breaks `pos + 22 + comment_len == bytes.len()`
    let mut trailing = build_deterministic_package_zip(&[("a.txt", b"hi")]);
    trailing.extend_from_slice(b"trailing");
    assert!(ZipArchive::parse(&trailing).is_err());

    // 6. [MED] packager::validate_pkl_imports_and_reads allows RFC 2396 schemes with +, -, .
    // and renders full `–– Pkl Error ––` VmException frame on escaping relative imports (#179).
    validate_pkl_imports_and_reads(
        Path::new("/proj"),
        "main.pkl",
        "import \"my-ext:/a.pkl\"\nimport \"custom+v1.0:/b.pkl\"\n",
    )
    .expect("RFC 2396 schemes with '-', '+', '.' must be recognized as absolute");
    let escape_err = validate_pkl_imports_and_reads(
        Path::new("/proj"),
        "Bad.pkl",
        "import \"../outside.pkl\"\n",
    )
    .unwrap_err();
    match escape_err {
        ModuleHostError::EvalError { message, .. } => {
            assert_eq!(
                message.to_java_utf8(),
                "–– Pkl Error ––\nPath `../outside.pkl` includes path segments that are outside the project root directory.\n\n1 | import \"../outside.pkl\"\n           ^^^^^^^^^^^^^^^^\nat <unknown> (file:///proj/Bad.pkl, line 1)"
            );
        }
        other => panic!("expected EvalError, got {other:?}"),
    }

    // 7. [MED] ProjectDependenciesManager::has_uri accepts single-slash `file:/...`
    let mgr = ProjectDependenciesManager::new(
        DeclaredDependenciesTree {
            project_file_uri: "file:///workspace/proj/PklProject".to_string(),
            package_uri: None,
            remote_dependencies: Vec::new(),
            local_dependencies: Vec::new(),
        },
        PathBuf::from("/workspace/proj"),
    );
    assert!(mgr.has_uri("file:///workspace/proj/sub/mod.pkl"));
    assert!(mgr.has_uri("file:/workspace/proj/sub/mod.pkl"));
    assert!(!mgr.has_uri("file:/workspace/other/mod.pkl"));
}

#[test]
fn packager_dependencies_match_jdk21_hashmap_iteration_order() {
    use pkl_runtime::packages::packager::java_hashmap_order;

    let keys_order = |ks: &[&str], cap: Option<usize>| -> Vec<String> {
        java_hashmap_order(
            ks.iter().map(|&k| (k.to_string(), k.to_string())).collect(),
            cap,
        )
    };

    // Direct JDK 21 `java.util.HashMap` oracle vectors (verified against pinned Temurin 21)
    assert_eq!(
        keys_order(&["jsonschema", "uri", "syntax", "deepToTyped"], Some(0)),
        vec!["syntax", "deepToTyped", "uri", "jsonschema"]
    );
    assert_eq!(
        keys_order(&["jsonschema", "uri", "syntax", "deepToTyped"], Some(1)),
        vec!["syntax", "deepToTyped", "uri", "jsonschema"]
    );
    assert_eq!(
        keys_order(&["jsonschema", "uri", "syntax", "deepToTyped"], None),
        vec!["deepToTyped", "syntax", "uri", "jsonschema"]
    );
    // Hash collision preservation (`"Aa".hashCode() == "BB".hashCode() == 2112`)
    assert_eq!(
        keys_order(&["Aa", "BB", "cc", "dd"], Some(4)),
        vec!["Aa", "BB", "cc", "dd"]
    );
    // UTF-16 surrogate pair hashing (`"\u{1F680}"` = `\uD83D\uDE80`)
    assert_eq!(
        keys_order(&["\u{1F680}", "alpha", "beta", "gamma"], Some(4)),
        vec!["\u{1F680}", "beta", "gamma", "alpha"]
    );
    // Resize at 13th entry on default capacity 16 -> 32
    assert_eq!(
        keys_order(
            &[
                "k01", "k02", "k03", "k04", "k05", "k06", "k07", "k08", "k09", "k10", "k11", "k12",
                "k13",
            ],
            None
        ),
        vec![
            "k11", "k10", "k02", "k13", "k01", "k12", "k04", "k03", "k06", "k05", "k08", "k07",
            "k09"
        ]
    );

    // End-to-end `ProjectPackager` JSON key order matching all 5 Pantry multi-dependency
    // packages (`Project.java:254,337` -> `ProjectPackager.java:229-267` -> `DependencyMetadata.java:528`).
    let make_leaf_project = |dir: &str, pkg_name: &str, ver: &str| -> Project {
        let uri_str = format!("package://pkg.pkl-lang.org/pkl-pantry/{pkg_name}@{ver}");
        Project {
            project_file_uri: format!("file:///ws/{dir}/PklProject"),
            project_base_uri: format!("file:///ws/{dir}/"),
            project_dir: PathBuf::from(format!("/ws/{dir}")),
            package: Some(PackageSpec {
                name: pkg_name.to_string(),
                base_uri: format!("package://pkg.pkl-lang.org/pkl-pantry/{pkg_name}"),
                version: PackageVersion::parse(ver).unwrap(),
                package_zip_url: format!("https://example.com/{pkg_name}@{ver}.zip"),
                description: None,
                authors: vec![],
                website: None,
                documentation: None,
                source_code: None,
                source_code_url_scheme: None,
                license: None,
                license_text: None,
                issue_tracker: None,
                api_tests: vec![],
                exclude: vec!["PklProject".into(), "PklProject.deps.json".into()],
                uri: PackageUri::parse(&uri_str).unwrap(),
            }),
            evaluator_settings: None,
            resolved_evaluator_settings: None,
            declared_dependencies: DeclaredDependenciesTree {
                project_file_uri: format!("file:///ws/{dir}/PklProject"),
                package_uri: Some(PackageUri::parse(&uri_str).unwrap()),
                remote_dependencies: vec![],
                local_dependencies: vec![],
            },
            local_project_dependencies: vec![],
            tests: vec![],
            annotations: vec![],
        }
    };

    // Simulate `k8s.contrib.crd`: 1 remote (`k8s`) + 5 local in exact Pantry authored order.
    let local_specs = [
        ("jsonschema", "org.json_schema", "1.1.1"),
        ("jsonschema.contrib", "org.json_schema.contrib", "1.2.2"),
        ("deepToTyped", "pkl.experimental.deepToTyped", "1.2.1"),
        ("uri", "pkl.experimental.uri", "1.0.4"),
        ("syntax", "pkl.experimental.syntax", "1.2.1"),
    ];
    let mut leaf_projects = Vec::new();
    let mut local_trees = Vec::new();
    let mut local_pairs = Vec::new();
    let mut deps_entries = Vec::new();
    let mut files_by_dir = BTreeMap::new();
    let mut deps_by_dir = BTreeMap::new();

    deps_entries.push(
        r#"    "package://pkg.pkl-lang.org/pkl-k8s/k8s@1": {
      "type": "remote",
      "uri": "projectpackage://pkg.pkl-lang.org/pkl-k8s/k8s@1.2.0",
      "checksums": {
        "sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
      }
    }"#
        .to_string(),
    );

    for (dep_name, pkg_name, ver) in local_specs {
        let leaf = make_leaf_project(pkg_name, pkg_name, ver);
        files_by_dir.insert(
            leaf.project_dir.clone(),
            vec![("mod.pkl".to_string(), b"x = 1\n".to_vec())],
        );
        deps_entries.push(format!(
            r#"    "package://pkg.pkl-lang.org/pkl-pantry/{pkg_name}@1": {{
      "type": "local",
      "uri": "projectpackage://pkg.pkl-lang.org/pkl-pantry/{pkg_name}@{ver}",
      "path": "../{pkg_name}"
    }}"#
        ));
        local_trees.push((dep_name.to_string(), leaf.declared_dependencies.clone()));
        local_pairs.push((dep_name.to_string(), leaf.clone()));
        leaf_projects.push(leaf);
    }

    let crd_uri =
        PackageUri::parse("package://pkg.pkl-lang.org/pkl-pantry/k8s.contrib.crd@4.2.4").unwrap();
    let crd_proj = Project {
        project_file_uri: "file:///ws/k8s.contrib.crd/PklProject".into(),
        project_base_uri: "file:///ws/k8s.contrib.crd/".into(),
        project_dir: PathBuf::from("/ws/k8s.contrib.crd"),
        package: Some(PackageSpec {
            name: "k8s.contrib.crd".into(),
            base_uri: "package://pkg.pkl-lang.org/pkl-pantry/k8s.contrib.crd".into(),
            version: PackageVersion::parse("4.2.4").unwrap(),
            package_zip_url: "https://example.com/k8s.contrib.crd@4.2.4.zip".into(),
            description: None,
            authors: vec![],
            website: None,
            documentation: None,
            source_code: None,
            source_code_url_scheme: None,
            license: None,
            license_text: None,
            issue_tracker: None,
            api_tests: vec![],
            exclude: vec!["PklProject".into(), "PklProject.deps.json".into()],
            uri: crd_uri.clone(),
        }),
        evaluator_settings: None,
        resolved_evaluator_settings: None,
        declared_dependencies: DeclaredDependenciesTree {
            project_file_uri: "file:///ws/k8s.contrib.crd/PklProject".into(),
            package_uri: Some(crd_uri),
            remote_dependencies: vec![DeclaredRemoteDep {
                name: "k8s".into(),
                package_uri: PackageUri::parse("package://pkg.pkl-lang.org/pkl-k8s/k8s@1.2.0")
                    .unwrap(),
                sha256: None,
            }],
            local_dependencies: local_trees,
        },
        local_project_dependencies: local_pairs,
        tests: vec![],
        annotations: vec![],
    };
    files_by_dir.insert(
        crd_proj.project_dir.clone(),
        vec![("CRD.pkl".to_string(), b"y = 2\n".to_vec())],
    );
    deps_by_dir.insert(
        crd_proj.project_dir.clone(),
        format!(
            "{{\n  \"schemaVersion\": 1,\n  \"resolvedDependencies\": {{\n{}\n  }}\n}}\n",
            deps_entries.join(",\n")
        ),
    );

    let mut all_projects = vec![crd_proj];
    all_projects.extend(leaf_projects);

    let host = InMemoryPackagerHost {
        files_by_dir,
        deps_by_dir,
        written: Mutex::new(BTreeMap::new()),
        transport: None,
    };
    let (results, _) = ProjectPackager::new(
        &all_projects,
        PathBuf::from("/ws"),
        ".out/%{name}@%{version}",
        true,
        &host,
    )
    .create_packages()
    .unwrap();

    // Verify exact RAW JSON key order inside `"dependencies": { ... }` matches Java 0.32.0!
    // (Note: `parse_metadata_bytes` simulates `JsObject`'s `HashMap(16)` for error ordering,
    // so we inspect the serialized JSON output directly.)
    let json = &results[0].metadata_json;
    let deps_start = json
        .find("  \"dependencies\": {\n")
        .expect("dependencies object present");
    let deps_block = &json[deps_start + "  \"dependencies\": {\n".len()..];
    let deps_end = deps_block
        .find("\n  },")
        .expect("dependencies object closes");
    let dep_names: Vec<&str> = deps_block[..deps_end]
        .lines()
        .filter_map(|line| {
            let rest = line.strip_prefix("    \"")?;
            let (name, tail) = rest.split_once("\": {")?;
            tail.is_empty().then_some(name)
        })
        .collect();
    assert_eq!(
        dep_names,
        vec![
            "syntax",
            "deepToTyped",
            "jsonschema.contrib",
            "uri",
            "jsonschema",
            "k8s",
        ]
    );
    // Also confirm `parse_metadata_bytes` still accepts the emitted metadata JSON cleanly.
    let parsed = pkl_runtime::packages::metadata::parse_metadata_bytes(
        &PackageUri::parse("package://pkg.pkl-lang.org/pkl-pantry/k8s.contrib.crd@4.2.4").unwrap(),
        &"https://example.com/k8s.contrib.crd@4.2.4".into(),
        json.as_bytes(),
    )
    .unwrap();
    assert_eq!(parsed.dependencies.len(), 6);
}
