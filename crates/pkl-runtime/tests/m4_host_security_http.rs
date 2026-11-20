//! M4 Track A — Wasm-pure Host, SecurityPolicy, HTTP Policy/Redirect/Proxy/Header,
//! ModulePath URI, Glob Policy, Import Graph (`...` / errors), and Timeout tests.

use pkl_runtime::glob::{
    GlobCompletion, GlobReader, GlobRequest, GlobRequestKind, ResourceReader, ResourceReaderId,
    configured_completion_with_policy,
};
use pkl_runtime::host::http::{
    HttpClientConfig, HttpError, HttpRequest, HttpResponse, HttpTransport, LazyHttpTransport,
    ParsedHttpUri, ProxyAddress, ProxyConfig, RewritingHttpClient, validate_header_name,
    validate_header_value, validate_rewrite_rule,
};
use pkl_runtime::host::modulepath::{InMemoryModulePath, parse_jar_file_uri, parse_modulepath_uri};
use pkl_runtime::host::security::SecurityPolicy;
use pkl_runtime::host::{
    HostPoll, HostSession, ReadFailure, format_timeout_message, format_timeout_seconds,
    resolve_resource_read_uri, try_configured_scalar_read_with_policy,
};
use pkl_runtime::modules::analyze::{AnalysisPoll, ImportGraphAnalysis};
use pkl_runtime::modules::{
    ModuleCapabilities, ModuleCompletion, ModuleHostError, ModuleKey, ModuleKeyId, ModuleKeyKind,
    ModuleRequest, ModuleRequestKind, ModuleSource, ResolvedModule, ResolvedModuleKey,
    SelectionPhase, standard_key_completion_with_policy,
};
use pkl_runtime::resources::ReadResult;
use pkl_runtime::value::{ErrorKind, PklString};
use pkl_runtime::vm::{Instruction, Program};
use std::collections::VecDeque;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

// ---------------------------------------------------------------------------
// 1. SecurityPolicy: allowlists, external schemes, trust levels, rootDir
// ---------------------------------------------------------------------------

#[test]
fn security_policy_allowlists_and_looking_at_prefix_matching() {
    let policy = SecurityPolicy::new(
        Some(&[
            "https://allowed\\.example\\.com/".into(),
            "file:/safe/".into(),
        ]),
        Some(&["env:APP_".into(), "prop:pkl\\.".into()]),
        None,
    )
    .unwrap();

    // Prefix regex match (`lookingAt`)
    assert!(
        policy
            .check_resolve_module(&"https://allowed.example.com/sub/mod.pkl".into())
            .is_ok()
    );
    assert!(
        policy
            .check_resolve_module(&"https://evil.example.com/mod.pkl".into())
            .is_err()
    );
    assert!(
        policy
            .check_resolve_module(&"file:/safe/a.pkl".into())
            .is_ok()
    );
    assert!(
        policy
            .check_resolve_module(&"file:/other/a.pkl".into())
            .is_err()
    );

    assert!(policy.check_read_resource(&"env:APP_TOKEN".into()).is_ok());
    assert!(policy.check_read_resource(&"env:SECRET".into()).is_err());
    assert!(
        policy
            .check_read_resource(&"prop:pkl.version".into())
            .is_ok()
    );
    assert!(policy.check_read_resource(&"prop:os.name".into()).is_err());

    // Exact upstream error messages
    let err = policy
        .check_resolve_module(&"http://blocked.test/a.pkl".into())
        .unwrap_err();
    assert_eq!(
        err,
        ModuleHostError::SecurityManagerException {
            message: Some(
                "Refusing to load module `http://blocked.test/a.pkl` because it does not match any entry in the module allowlist (`--allowed-modules`)."
                    .into()
            )
        }
    );
    let rerr = policy
        .check_read_resource(&"env:SECRET".into())
        .unwrap_err();
    assert_eq!(
        rerr,
        ReadFailure::SecurityManagerException {
            message: Some(
                "Refusing to read resource `env:SECRET` because it does not match any entry in the resource allowlist (`--allowed-resources`)."
                    .into()
            )
        }
    );
}

#[test]
fn security_policy_trust_levels_and_root_dir_variants() {
    let policy = SecurityPolicy::default_standard().with_root_dir(Some("/workspace/root".into()));

    // Trust levels: repl(40) > local file(30) > modulepath(20) == jar:nested(20) > https/remote file(10) > pkl(0)
    assert!(
        policy
            .check_import_module(&"file:///a.pkl".into(), &"https://x.test/b.pkl".into())
            .is_ok()
    );
    assert!(
        policy
            .check_import_module(&"https://x.test/b.pkl".into(), &"file:///a.pkl".into())
            .is_err()
    );
    assert!(
        policy
            .check_import_module(&"modulepath:/a.pkl".into(), &"file:///a.pkl".into())
            .is_err()
    );
    assert!(
        policy
            .check_import_module(&"file:///a.pkl".into(), &"modulepath:/a.pkl".into())
            .is_ok()
    );
    assert!(
        policy
            .check_import_module(&"pkl:base".into(), &"https://x.test/b.pkl".into())
            .is_err()
    );
    assert!(
        policy
            .check_import_module(
                &"file://remote.host/share/a.pkl".into(),
                &"file:///local.pkl".into()
            )
            .is_err()
    );

    // rootDir containment: component boundaries, traversal, percent-encoding, jar:file:, UNC
    assert!(
        policy
            .check_resolve_module(&"file:///workspace/root/dir/a.pkl".into())
            .is_ok()
    );
    assert!(
        policy
            .check_resolve_module(&"file:///workspace/root2/a.pkl".into())
            .is_err()
    );
    assert!(
        policy
            .check_resolve_module(&"file:///workspace/root/../secret.pkl".into())
            .is_err()
    );
    assert!(
        policy
            .check_resolve_module(&"file:///workspace/root/%2e%2e/secret.pkl".into())
            .is_err()
    );
    assert!(
        policy
            .check_resolve_module(&"jar:file:///workspace/root/lib.jar!/m.pkl".into())
            .is_ok()
    );
    assert!(
        policy
            .check_resolve_module(&"jar:file:///workspace/other/lib.jar!/m.pkl".into())
            .is_err()
    );
    assert!(
        policy
            .check_resolve_module(&"file://127.0.0.1/workspace/root/a.pkl".into())
            .is_err()
    );

    // ResourceReaders.FileResource.read IOException wrapping of SecurityManagerException
    let wrapped = SecurityPolicy::wrap_file_resource_security_error(
        "Refusing to read resource `file:///etc/passwd` because it is not within the root directory (`--root-dir`)."
            .into(),
    );
    match wrapped {
        ReadFailure::Io(io) => {
            assert_eq!(
                io.message.unwrap(),
                "org.pkl.core.SecurityManagerException: Refusing to read resource `file:///etc/passwd` because it is not within the root directory (`--root-dir`)."
            );
        }
        other => panic!("expected IOException-wrapped SecurityManagerException, got {other:?}"),
    }
}

#[test]
fn env_and_prop_scalar_reads_respect_custom_security_policy() {
    let deny_env = SecurityPolicy::new(None, Some(&["prop:".into()]), None).unwrap();
    let props = vec![("foo".into(), "bar".into())];
    let env = vec![("SECRET".into(), "42".into())];

    assert_eq!(
        try_configured_scalar_read_with_policy(&"prop:foo".into(), &props, &env, &deny_env)
            .unwrap(),
        Some(ReadResult::Text("bar".into()))
    );
    assert!(
        try_configured_scalar_read_with_policy(&"env:SECRET".into(), &props, &env, &deny_env)
            .is_err()
    );
}

// ---------------------------------------------------------------------------
// 2. HTTP Policy, Redirects (20-hop / downgrade / Location), Headers, Rewrites, Proxy, Lazy
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct MockTransport(Arc<MockTransportInner>);

struct MockTransportInner {
    responses: Mutex<VecDeque<Result<HttpResponse, HttpError>>>,
    seen: Mutex<Vec<HttpRequest>>,
    closed: AtomicUsize,
}

impl MockTransport {
    fn new(responses: Vec<Result<HttpResponse, HttpError>>) -> Self {
        Self(Arc::new(MockTransportInner {
            responses: Mutex::new(responses.into()),
            seen: Mutex::new(Vec::new()),
            closed: AtomicUsize::new(0),
        }))
    }

    fn seen_requests(&self) -> Vec<HttpRequest> {
        self.0.seen.lock().unwrap().clone()
    }

    fn closed_count(&self) -> usize {
        self.0.closed.load(Ordering::SeqCst)
    }
}

impl HttpTransport for MockTransport {
    fn send_single_hop(&self, request: &HttpRequest) -> Result<HttpResponse, HttpError> {
        self.0.seen.lock().unwrap().push(request.clone());
        self.0
            .responses
            .lock()
            .unwrap()
            .pop_front()
            .expect("unexpected extra HTTP request")
    }

    fn close(&self) {
        self.0.closed.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn http_redirects_301_302_303_307_308_and_per_hop_header_isolation() {
    let mut cfg = HttpClientConfig::default();
    cfg.add_headers(
        "https://a.test/**",
        [
            ("Authorization".into(), "Bearer secret-a".into()),
            ("X-Multi".into(), "v1".into()),
            ("X-Multi".into(), "v2".into()),
        ],
    )
    .unwrap();
    cfg.add_headers(
        "https://b.test/**",
        [("User-Agent".into(), "CustomUA/1.0".into())],
    )
    .unwrap();

    let mock = MockTransport::new(vec![
        Ok(HttpResponse {
            status: 301,
            headers: vec![("Location".into(), "/hop2".into())],
            body: Vec::new(),
        }),
        Ok(HttpResponse {
            status: 302,
            headers: vec![("location".into(), "hop3".into())],
            body: Vec::new(),
        }),
        Ok(HttpResponse {
            status: 303,
            headers: vec![("Location".into(), "https://b.test/start".into())],
            body: Vec::new(),
        }),
        Ok(HttpResponse {
            status: 307,
            headers: vec![("Location".into(), "//b.test/next".into())],
            body: Vec::new(),
        }),
        Ok(HttpResponse {
            status: 308,
            headers: vec![("Location".into(), "https://b.test/final".into())],
            body: Vec::new(),
        }),
        Ok(HttpResponse {
            status: 200,
            headers: vec![],
            body: b"ok".to_vec(),
        }),
    ]);

    let client = RewritingHttpClient::new(cfg, mock.clone());
    let resp = client.get("https://a.test/dir/start", |_| Ok(())).unwrap();
    assert_eq!(resp.status, 200);
    assert_eq!(resp.body, b"ok");

    let seen = mock.seen_requests();
    assert_eq!(seen.len(), 6);
    assert_eq!(seen[0].uri, "https://a.test/dir/start");
    assert_eq!(seen[1].uri, "https://a.test/hop2");
    assert_eq!(seen[2].uri, "https://a.test/hop3");
    assert_eq!(seen[3].uri, "https://b.test/start");
    assert_eq!(seen[4].uri, "https://b.test/next");
    assert_eq!(seen[5].uri, "https://b.test/final");

    // Hop 0 on a.test receives Authorization, X-Multi v1/v2, and default User-Agent
    assert!(
        seen[0]
            .headers
            .contains(&("Authorization".into(), "Bearer secret-a".into()))
    );
    assert!(
        pkl_runtime::host::http::DEFAULT_USER_AGENT.starts_with("Pkl/0.32.0 ("),
        "unexpected default User-Agent: {}",
        pkl_runtime::host::http::DEFAULT_USER_AGENT
    );
    assert!(seen[0].headers.contains(&(
        "User-Agent".into(),
        pkl_runtime::host::http::DEFAULT_USER_AGENT.into()
    )));
    // Hop 3 on b.test MUST NOT receive a.test's Authorization header and MUST use custom User-Agent
    assert!(!seen[3].headers.iter().any(|(k, _)| k == "Authorization"));
    assert_eq!(
        seen[3].headers,
        vec![("User-Agent".into(), "CustomUA/1.0".into())]
    );
}

#[test]
fn http_redirect_limit_20_and_error_templates() {
    // Exactly 20 redirects succeed
    let mut ok_resps: Vec<_> = (0..20)
        .map(|i| {
            Ok(HttpResponse {
                status: 302,
                headers: vec![("Location".into(), format!("https://r.test/{}", i + 1))],
                body: Vec::new(),
            })
        })
        .collect();
    ok_resps.push(Ok(HttpResponse {
        status: 200,
        headers: vec![],
        body: b"done".to_vec(),
    }));
    let client_ok =
        RewritingHttpClient::new(HttpClientConfig::default(), MockTransport::new(ok_resps));
    assert_eq!(
        client_ok.get("https://r.test/0", |_| Ok(())).unwrap().body,
        b"done"
    );

    // 21st redirect fails with exact `httpTooManyRedirects`
    let too_many: Vec<_> = (0..21)
        .map(|i| {
            Ok(HttpResponse {
                status: 302,
                headers: vec![("Location".into(), format!("https://r.test/{}", i + 1))],
                body: Vec::new(),
            })
        })
        .collect();
    let client_many =
        RewritingHttpClient::new(HttpClientConfig::default(), MockTransport::new(too_many));
    assert_eq!(
        client_many.get("https://r.test/0", |_| Ok(())).unwrap_err(),
        HttpError::HttpClientException(
            "Too many redirects, exceeded the redirect threshold (20).".into()
        )
    );

    // Missing Location
    let no_loc = RewritingHttpClient::new(
        HttpClientConfig::default(),
        MockTransport::new(vec![Ok(HttpResponse {
            status: 302,
            headers: vec![],
            body: Vec::new(),
        })]),
    );
    assert_eq!(
        no_loc.get("https://r.test/a", |_| Ok(())).unwrap_err(),
        HttpError::HttpClientException(
            "Cannot follow HTTP redirect because no 'Location' header was found.\n\nHTTP Request: `GET https://r.test/a`"
                .into()
        )
    );

    // Malformed Location
    let bad_loc = RewritingHttpClient::new(
        HttpClientConfig::default(),
        MockTransport::new(vec![Ok(HttpResponse {
            status: 302,
            headers: vec![("Location".into(), "http://bad host/x".into())],
            body: Vec::new(),
        })]),
    );
    assert_eq!(
        bad_loc.get("https://r.test/a", |_| Ok(())).unwrap_err(),
        HttpError::HttpClientException(
            "Cannot follow HTTP redirect because the response 'Location' header has a malformed URI.\n\nHTTP Request: `GET https://r.test/a`\nLocation header: `http://bad host/x`"
                .into()
        )
    );

    // HTTPS -> HTTP downgrade forbidden
    let downgrade = RewritingHttpClient::new(
        HttpClientConfig::default(),
        MockTransport::new(vec![Ok(HttpResponse {
            status: 301,
            headers: vec![("Location".into(), "http://r.test/insecure".into())],
            body: Vec::new(),
        })]),
    );
    assert_eq!(
        downgrade.get("https://r.test/secure", |_| Ok(())).unwrap_err(),
        HttpError::HttpClientException(
            "Cannot follow redirect from 'https:' URL to 'http:' URL.\nHTTP Request: `GET https://r.test/secure`\nRedirected to: `http://r.test/insecure`"
                .into()
        )
    );

    // HTTP -> HTTPS upgrade allowed
    let upgrade = RewritingHttpClient::new(
        HttpClientConfig::default(),
        MockTransport::new(vec![
            Ok(HttpResponse {
                status: 301,
                headers: vec![("Location".into(), "https://r.test/secure".into())],
                body: Vec::new(),
            }),
            Ok(HttpResponse {
                status: 200,
                headers: vec![],
                body: b"upgraded".to_vec(),
            }),
        ]),
    );
    assert_eq!(
        upgrade.get("http://r.test/start", |_| Ok(())).unwrap().body,
        b"upgraded"
    );
}

#[test]
fn http_rewrites_longest_prefix_security_per_hop_and_404_distinction() {
    let policy = SecurityPolicy::default_standard(); // allows https:, denies http:

    // http://old.test/ rewritten to https://new.test/ passes default allowlist!
    let cfg = HttpClientConfig::default()
        .with_rewrites([
            ("http://old.test/".into(), "https://generic.test/".into()),
            ("http://old.test/v2/".into(), "https://v2.test/".into()),
        ])
        .unwrap()
        .with_test_port(Some(9443));

    let mock = MockTransport::new(vec![
        Ok(HttpResponse {
            status: 200,
            headers: vec![],
            body: b"v2-body".to_vec(),
        }),
        Ok(HttpResponse {
            status: 404,
            headers: vec![],
            body: Vec::new(),
        }),
        Ok(HttpResponse {
            status: 404,
            headers: vec![],
            body: Vec::new(),
        }),
        Err(HttpError::Io("connection reset".into())),
    ]);
    let client = RewritingHttpClient::new(cfg, mock.clone());

    // Longest prefix wins (`v2/` over `/`), case-insensitive scheme/host
    let res = client
        .fetch_module("HTTP://OLD.TEST/v2/mod.pkl", |hop| {
            policy.check_allowlist_only(&hop.into(), false)
        })
        .unwrap();
    assert_eq!(res, b"v2-body");
    assert_eq!(mock.seen_requests()[0].uri, "https://v2.test/mod.pkl");

    // 404 on resource returns Ok(None) (Missing -> read? null), 404 on module returns IOException
    let missing_res = client
        .fetch_resource("https://v2.test/missing.txt", |hop| {
            policy.check_allowlist_only(&hop.into(), true)
        })
        .unwrap();
    assert!(missing_res.is_none());

    let missing_mod = client
        .fetch_module("https://v2.test/missing.pkl", |hop| {
            policy.check_allowlist_only(&hop.into(), false)
        })
        .unwrap_err();
    assert!(matches!(missing_mod, ModuleHostError::Io(_)));

    // IOException appends `(request was rewritten: A -> B)` suffix
    let io_err = client.get("http://old.test/fail", |_| Ok(())).unwrap_err();
    assert_eq!(
        io_err,
        HttpError::Io(
            "connection reset (request was rewritten: http://old.test/fail -> https://generic.test/fail)"
                .into()
        )
    );

    // Per-hop SecurityManager denies disallowed redirect target BEFORE sending request
    let per_hop_mock = MockTransport::new(vec![Ok(HttpResponse {
        status: 302,
        headers: vec![("Location".into(), "https://blocked.test/evil.pkl".into())],
        body: Vec::new(),
    })]);
    let restricted =
        SecurityPolicy::new(Some(&["https://allowed\\.test/".into()]), None, None).unwrap();
    let per_hop_client =
        RewritingHttpClient::new(HttpClientConfig::default(), per_hop_mock.clone());
    let denied = per_hop_client
        .get("https://allowed.test/go", |hop| {
            restricted.check_allowlist_only(&hop.into(), false)
        })
        .unwrap_err();
    assert!(matches!(denied, HttpError::SecurityManagerException(_)));
    // Only hop 0 was ever sent; blocked.test was NEVER contacted!
    assert_eq!(per_hop_mock.seen_requests().len(), 1);
}

#[test]
fn http_validation_proxy_and_lazy_transport() {
    assert!(validate_rewrite_rule("ftp://a/").is_err());
    assert!(validate_rewrite_rule("https://a/no-slash").is_err());
    assert!(validate_rewrite_rule("https://A.TEST/").unwrap() == "https://a.test/");
    assert!(validate_header_name("Host").is_err());
    assert!(validate_header_name("Proxy-Foo").is_err());
    assert!(validate_header_name("Sec-Bar").is_err());
    assert!(validate_header_value(&"x".repeat(4097)).is_err());
    assert!(ProxyAddress::parse("https://p:8080").is_err());

    let proxy = ProxyConfig::new(Some("http://127.0.0.1:8080"), &["*".into()]).unwrap();
    let target = ParsedHttpUri::parse("https://any.example/a").unwrap();
    assert!(proxy.select(&target).is_none());

    // LazyHttpTransport caches initialization error and calls factory only once
    let init_calls = Arc::new(AtomicUsize::new(0));
    let calls_clone = init_calls.clone();
    let lazy = LazyHttpTransport::new(move || -> Result<MockTransport, HttpError> {
        calls_clone.fetch_add(1, Ordering::SeqCst);
        Err(HttpError::HttpClientException(
            "Cannot find CA certificate file `/nope.pem`.".into(),
        ))
    });
    assert!(!lazy.is_initialized());
    let req = HttpRequest {
        uri: "https://a.test/".into(),
        parsed: ParsedHttpUri::parse("https://a.test/").unwrap(),
        headers: vec![],
        proxy: None,
        connect_timeout_ms: 1000,
        request_timeout_ms: 1000,
    };
    assert!(lazy.send_single_hop(&req).is_err());
    assert!(lazy.send_single_hop(&req).is_err());
    assert_eq!(init_calls.load(Ordering::SeqCst), 1);

    // Close prevents subsequent requests
    let mock = MockTransport::new(vec![]);
    let client = RewritingHttpClient::new(HttpClientConfig::default(), mock.clone());
    client.close();
    assert!(matches!(
        client.get("https://a.test/", |_| Ok(())),
        Err(HttpError::Closed(_))
    ));
    assert_eq!(mock.closed_count(), 1);
}

// ---------------------------------------------------------------------------
// 3. ModulePath, Relative Hierarchical Reads, Glob Policy, ImportGraph Triple-Dot, Timeout
// ---------------------------------------------------------------------------

#[test]
fn modulepath_relative_reads_and_glob_policy() {
    // Missing leading slash error matches Pkl 0.32.0
    assert_eq!(
        parse_modulepath_uri(&"modulepath:bad.pkl".into()).unwrap_err(),
        "Module URI `modulepath:bad.pkl` is missing a `/` after `modulepath:`."
    );
    let jar = parse_jar_file_uri(&"jar:file:///libs/a.jar!/sub/m.pkl".into()).unwrap();
    assert_eq!(jar.archive_path, "/libs/a.jar");
    assert_eq!(jar.entry_path, "sub/m.pkl");

    let mut mp = InMemoryModulePath::new();
    mp.add_dir_entry("pkg/base.pkl", "file:///d1/pkg/base.pkl", b"d1".to_vec());
    mp.add_jar_entry("file:///libs/lib.jar", "pkg/base.pkl", b"jar".to_vec());
    mp.add_jar_entry("file:///libs/lib.jar", "pkg/only.pkl", b"only".to_vec());
    mp.add_jar_entry("file:///libs/lib.jar", "pkg/Skip.class", b"bc".to_vec());
    assert_eq!(
        mp.resolve(&"modulepath:/pkg/base.pkl".into())
            .unwrap()
            .unwrap()
            .bytes,
        b"d1"
    );
    assert!(mp.has_element(&"modulepath:/pkg/only.pkl".into()).unwrap());
    assert!(
        !mp.has_element(&"modulepath:/pkg/Skip.class".into())
            .unwrap()
    );

    // Relative resource reads across ALL hierarchical module schemes
    assert_eq!(
        resolve_resource_read_uri(
            &"sibling.txt".into(),
            Some(&"https://a.test/dir/m.pkl".into())
        )
        .unwrap(),
        "https://a.test/dir/sibling.txt"
    );
    assert_eq!(
        resolve_resource_read_uri(&"../top.txt".into(), Some(&"modulepath:/a/b/m.pkl".into()))
            .unwrap(),
        "modulepath:/a/top.txt"
    );
    assert_eq!(
        resolve_resource_read_uri(
            &"data.json".into(),
            Some(&"jar:file:///libs/a.jar!/dir/m.pkl".into())
        )
        .unwrap(),
        "jar:file:///libs/a.jar!/dir/data.json"
    );

    // modulepath resource reader & module key both report IsGlobbable = false
    let res_req = GlobRequest {
        id: 1,
        kind: GlobRequestKind::IsGlobbable {
            reader: GlobReader::Resource(ResourceReader {
                id: ResourceReaderId(1),
                scheme: "modulepath".into(),
                handle: "builtin:modulepath".into(),
            }),
        },
    };
    assert_eq!(
        configured_completion_with_policy(&res_req, &[], &[], &SecurityPolicy::default_standard()),
        Some(GlobCompletion::Globbable(Ok(false)))
    );

    let mod_req = GlobRequest {
        id: 2,
        kind: GlobRequestKind::IsGlobbable {
            reader: GlobReader::Module(ModuleKey {
                id: ModuleKeyId(1),
                uri: "modulepath:/a.pkl".into(),
                kind: ModuleKeyKind::Host,
                handle: "builtin:modulepath".into(),
            }),
        },
    };
    assert_eq!(
        configured_completion_with_policy(&mod_req, &[], &[], &SecurityPolicy::default_standard()),
        Some(GlobCompletion::Globbable(Ok(false)))
    );

    // standard_key_completion_with_policy selects modulepath and rejects missing slash
    let select_bad = ModuleRequest {
        id: 1,
        kind: ModuleRequestKind::SelectKey {
            id: ModuleKeyId(1),
            uri: "modulepath:no-slash.pkl".into(),
            phase: SelectionPhase::Primary,
        },
    };
    assert!(matches!(
        standard_key_completion_with_policy(&select_bad, &SecurityPolicy::default_standard()),
        Some(ModuleCompletion::KeySelected(Err(
            ModuleHostError::UriSyntax { .. }
        )))
    ));
}

#[test]
fn import_graph_analysis_resolves_triple_dot_imports() {
    let mut analysis = ImportGraphAnalysis::new(["file:///root/sub/child.pkl".into()]);
    let graph = loop {
        match analysis.poll() {
            AnalysisPoll::Ready(res) => break res.unwrap(),
            AnalysisPoll::Closed => panic!("unexpected close"),
            AnalysisPoll::Request(req) => {
                let completion = match &req.kind {
                    ModuleRequestKind::SelectKey { id, uri, .. } => {
                        ModuleCompletion::KeySelected(Ok(ModuleKey {
                            id: *id,
                            uri: uri.clone(),
                            kind: ModuleKeyKind::StandardFile,
                            handle: PklString::default(),
                        }))
                    }
                    ModuleRequestKind::ResolveKey {
                        key,
                        resolved_key_id,
                        ..
                    } => ModuleCompletion::KeyResolved(Ok(ResolvedModuleKey {
                        id: *resolved_key_id,
                        key: key.clone(),
                        handle: PklString::default(),
                        module: ResolvedModule {
                            original_uri: key.uri.clone(),
                            resolved_uri: key.uri.clone(),
                            source_uri: key.uri.clone(),
                        },
                    })),
                    ModuleRequestKind::LoadSource { resolved, .. } => {
                        let text = if resolved.original_uri == "file:///root/sub/child.pkl" {
                            "import \".../shared.pkl\"\n"
                        } else {
                            "x = 1\n"
                        };
                        ModuleCompletion::Source(Ok(ModuleSource {
                            text: text.into(),
                            inferred_name: "m".into(),
                        }))
                    }
                    ModuleRequestKind::Capabilities { .. } => {
                        ModuleCompletion::Capabilities(Ok(ModuleCapabilities {
                            local: true,
                            hierarchical: true,
                            fragment_paths: false,
                        }))
                    }
                    ModuleRequestKind::HasElement { element_uri, .. } => {
                        ModuleCompletion::HasElement(Ok(element_uri == "file:///root/shared.pkl"))
                    }
                    other => panic!("unexpected analysis request: {other:?}"),
                };
                analysis.complete(req.id, completion).unwrap();
            }
        }
    };
    let child = graph
        .nodes
        .iter()
        .find(|n| n.uri == "file:///root/sub/child.pkl")
        .unwrap();
    assert_eq!(
        child.imports,
        vec![PklString::from("file:///root/shared.pkl")]
    );
}

#[test]
fn timeout_formatting_and_host_session_timeout_message_match_pkl_0_32_0() {
    assert_eq!(format_timeout_seconds(1000), "1");
    assert_eq!(format_timeout_seconds(500), "0.5");
    assert_eq!(format_timeout_seconds(1250), "1.25");
    assert_eq!(format_timeout_seconds(1235), "1.24"); // half-even
    assert_eq!(format_timeout_seconds(1225), "1.22"); // half-even
    assert_eq!(
        format_timeout_message(500),
        "Evaluation timed out after 0.5 second(s)."
    );

    // Infinite loop program expires with exact upstream timeout message
    let program = Arc::new(Program::new(vec![vec![Instruction::Jump(0)]]));
    let mut session = HostSession::new(program);
    session.start(0, vec![], 100, Some(500)).unwrap();
    assert!(matches!(session.poll(200, 10), HostPoll::Yielded));
    match session.poll(600, 10) {
        HostPoll::Ready(Err(err)) => {
            assert_eq!(err.kind, ErrorKind::Timeout);
            assert_eq!(err.message, "Evaluation timed out after 0.5 second(s).");
        }
        other => panic!("expected Timeout error, got {other:?}"),
    }
}

#[test]
fn review_regressions_user_agent_redirect_final_uri_cidr_port_jar_magic_and_io_subclasses() {
    use pkl_runtime::host::IoFailure;
    use pkl_runtime::host::http::DEFAULT_USER_AGENT;
    use pkl_runtime::host::modulepath::{EXECUTABLE_JAR_HEADER, is_zip_or_executable_jar_magic};

    // 1. Default User-Agent includes platform/flavor (`Pkl/0.32.0 (<os>; <flavor>)`)
    assert!(DEFAULT_USER_AGENT.starts_with("Pkl/0.32.0 ("));
    assert!(DEFAULT_USER_AGENT.ends_with(')'));

    // 2. Final redirected URI is reported in non-200 status errors (`HttpUtils.checkHasStatusCode200`)
    let redirect_to_404 = MockTransport::new(vec![
        Ok(HttpResponse {
            status: 302,
            headers: vec![("Location".into(), "https://final.test/moved.pkl".into())],
            body: Vec::new(),
        }),
        Ok(HttpResponse {
            status: 404,
            headers: Vec::new(),
            body: Vec::new(),
        }),
    ]);
    let client = RewritingHttpClient::new(HttpClientConfig::default(), redirect_to_404);
    let err = client
        .fetch_module("https://origin.test/start.pkl", |_| Ok(()))
        .unwrap_err();
    match err {
        ModuleHostError::Io(io) => {
            let msg = io.message.unwrap().to_java_utf8();
            assert!(
                msg.contains("https://final.test/moved.pkl"),
                "expected final redirected URI in 404 message, got: {msg}"
            );
            assert!(
                !msg.contains("https://origin.test/start.pkl"),
                "must report final URI, not initial URI: {msg}"
            );
        }
        other => panic!("expected Io error for 404 module, got: {other:?}"),
    }

    // 3. NoProxyRule supports CIDR+port and falls back to hostname on invalid prefix (>32 / >128)
    let proxy = ProxyConfig::new(
        Some("http://127.0.0.1:8080"),
        &[
            "10.0.0.0/8:8443".into(),
            "[fe80::]/10:9443".into(),
            "10.0.0.1/99".into(),
        ],
    )
    .unwrap();
    assert!(
        proxy
            .select(&ParsedHttpUri::parse("https://10.1.2.3:8443/a").unwrap())
            .is_none()
    );
    assert!(
        proxy
            .select(&ParsedHttpUri::parse("https://10.1.2.3:443/a").unwrap())
            .is_some()
    );
    assert!(
        proxy
            .select(&ParsedHttpUri::parse("https://[fe80::1]:9443/a").unwrap())
            .is_none()
    );
    assert!(
        proxy
            .select(&ParsedHttpUri::parse("https://[fe80::1]:443/a").unwrap())
            .is_some()
    );
    // Invalid CIDR prefix > 32 does not match arbitrary 10.0.0.1 as CIDR
    assert!(
        proxy
            .select(&ParsedHttpUri::parse("https://10.0.0.1/a").unwrap())
            .is_some()
    );

    // 4. Executable JAR magic requires exact 39-byte header, not any `#!` script
    assert!(is_zip_or_executable_jar_magic(EXECUTABLE_JAR_HEADER));
    assert!(is_zip_or_executable_jar_magic(b"PK\x03\x04rest"));
    assert!(!is_zip_or_executable_jar_magic(b"#!/bin/sh\necho hi\n"));

    // 5. IoFailure::with_class preserves Java IOException subclass names without double-prefixing
    let ssl_err = HttpError::Io(
        "Error during SSL handshake with host `localhost`:\nunable to find valid certification path to requested target".into(),
    )
    .into_module_error();
    match ssl_err {
        ModuleHostError::Io(io) => {
            assert_eq!(
                io.message.unwrap().to_java_utf8(),
                "SSLHandshakeException: Error during SSL handshake with host `localhost`:\nunable to find valid certification path to requested target"
            );
        }
        other => panic!("expected SSLHandshakeException Io error, got: {other:?}"),
    }
    let zip_io = IoFailure::with_class("ZipException", "zip END header not found");
    assert_eq!(
        zip_io.message.unwrap().to_java_utf8(),
        "ZipException: zip END header not found"
    );

    // 6. Relative module imports inside `package:` and `projectpackage:` resolve against fragment path
    use pkl_runtime::host::resolve_module_key_uri;
    let pkg_base: PklString =
        "package://pkg.pkl-lang.org/pkl-pantry/com.influxdata.telegraf@1.8.1#/plugins/inputs/CpuInput.pkl".into();
    assert_eq!(
        resolve_module_key_uri(&"Input.pkl".into(), &pkg_base)
            .unwrap()
            .to_java_utf8(),
        "package://pkg.pkl-lang.org/pkl-pantry/com.influxdata.telegraf@1.8.1#/plugins/inputs/Input.pkl"
    );
    assert_eq!(
        resolve_module_key_uri(&"../Plugin.pkl".into(), &pkg_base)
            .unwrap()
            .to_java_utf8(),
        "package://pkg.pkl-lang.org/pkl-pantry/com.influxdata.telegraf@1.8.1#/plugins/Plugin.pkl"
    );
    assert_eq!(
        resolve_module_key_uri(&"/Top.pkl".into(), &pkg_base)
            .unwrap()
            .to_java_utf8(),
        "package://pkg.pkl-lang.org/pkl-pantry/com.influxdata.telegraf@1.8.1#/Top.pkl"
    );
    let proj_pkg_base: PklString =
        "projectpackage://pkg.pkl-lang.org/pkl-pantry/com.github.actions@1.9.1#/github/codeql-action/init/v4/CodeqlActionInit.pkl".into();
    assert_eq!(
        resolve_module_key_uri(&"../Config.pkl".into(), &proj_pkg_base)
            .unwrap()
            .to_java_utf8(),
        "projectpackage://pkg.pkl-lang.org/pkl-pantry/com.github.actions@1.9.1#/github/codeql-action/init/Config.pkl"
    );
}
