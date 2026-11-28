//! M4 Track A — Real Native OS Host I/O integration tests:
//! - Real filesystem, `rootDir`, symlinks, `O_NOFOLLOW`, `jar:file:`, UNC fast-reject
//! - Real `NativeModulePathResolver` (multi-entry dirs + real ZIP/JAR + `...` + `close()`)
//! - Real local HTTP & HTTPS TLS (`rcgen` + `rustls`) & HTTP/CONNECT proxy servers
//! - Real CA certificate precedence (`--ca-certificates`, `cacerts_dir`, bundled `PklCARoots.pem`)
//! - Real monotonic `Instant` timeout, `CancelFlag` socket abort, and `close()`/`Drop` cleanup.

use pkl_host_native::{
    BUNDLED_PKL_CA_ROOTS_PEM, CaCertSource, NativeHost, NativeHostConfig, NativeModulePathResolver,
    path_to_file_uri,
};
use pkl_runtime::evaluation::frontend::{ModuleEvalOptions, ModuleSourceInput};
use pkl_runtime::host::http::{HttpClientConfig, ProxyConfig};
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use rustls::{ServerConfig, ServerConnection, StreamOwned};
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

static TEMP_COUNTER: AtomicUsize = AtomicUsize::new(1);

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(prefix: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let n = TEMP_COUNTER.fetch_add(1, Ordering::SeqCst);
        let path = std::env::temp_dir().join(format!("pkl_m4_track_a_{prefix}_{nanos}_{n}"));
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

/// Builds a minimal valid STORE-mode ZIP archive in memory (no external tools required).
fn build_store_zip(files: &[(&str, &[u8])]) -> Vec<u8> {
    fn crc32(bytes: &[u8]) -> u32 {
        let mut crc = 0xFFFF_FFFFu32;
        for &b in bytes {
            crc ^= u32::from(b);
            for _ in 0..8 {
                crc = if crc & 1 != 0 {
                    (crc >> 1) ^ 0xEDB8_8320
                } else {
                    crc >> 1
                };
            }
        }
        !crc
    }
    let mut out = Vec::new();
    let mut central = Vec::new();
    for (name, data) in files {
        let offset = out.len() as u32;
        let crc = crc32(data);
        let size = data.len() as u32;
        let name_bytes = name.as_bytes();
        // Local file header
        out.extend_from_slice(b"PK\x03\x04");
        out.extend_from_slice(&20u16.to_le_bytes()); // version
        out.extend_from_slice(&0u16.to_le_bytes()); // flags
        out.extend_from_slice(&0u16.to_le_bytes()); // STORE
        out.extend_from_slice(&0u16.to_le_bytes()); // mtime
        out.extend_from_slice(&0u16.to_le_bytes()); // mdate
        out.extend_from_slice(&crc.to_le_bytes());
        out.extend_from_slice(&size.to_le_bytes());
        out.extend_from_slice(&size.to_le_bytes());
        out.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(name_bytes);
        out.extend_from_slice(data);

        // Central directory header
        central.extend_from_slice(b"PK\x01\x02");
        central.extend_from_slice(&20u16.to_le_bytes());
        central.extend_from_slice(&20u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&crc.to_le_bytes());
        central.extend_from_slice(&size.to_le_bytes());
        central.extend_from_slice(&size.to_le_bytes());
        central.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes());
        central.extend_from_slice(&0u32.to_le_bytes());
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(name_bytes);
    }
    let cd_offset = out.len() as u32;
    let cd_size = central.len() as u32;
    out.extend_from_slice(&central);
    // EOCD
    out.extend_from_slice(b"PK\x05\x06");
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&(files.len() as u16).to_le_bytes());
    out.extend_from_slice(&(files.len() as u16).to_le_bytes());
    out.extend_from_slice(&cd_size.to_le_bytes());
    out.extend_from_slice(&cd_offset.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

fn eval_text(host: &NativeHost, uri: &str, text: &str) -> (i32, String, String) {
    let (res, report) = host.evaluate_module_output(
        ModuleSourceInput {
            text,
            uri,
            inferred_name: "main",
        },
        &ModuleEvalOptions::default(),
    );
    assert!(report.http_closed);
    assert!(report.modulepath_closed);
    (
        res.exit_code,
        String::from_utf8_lossy(&res.stdout).into_owned(),
        String::from_utf8_lossy(&res.stderr).into_owned(),
    )
}

// ---------------------------------------------------------------------------
// 1. Filesystem, rootDir, Symlink Escapes, jar:file:, UNC, O_NOFOLLOW
// ---------------------------------------------------------------------------

#[test]
fn root_dir_blocks_escape_nonexistent_symlink_jar_and_unc() {
    let tmp = TempDir::new("rootdir");
    let root = tmp.path().join("sandbox");
    let outside = tmp.path().join("outside");
    fs::create_dir_all(&root).unwrap();
    fs::create_dir_all(&outside).unwrap();

    let inside_mod = root.join("ok.pkl");
    fs::write(&inside_mod, "value = \"inside\"\n").unwrap();
    let outside_mod = outside.join("secret.pkl");
    fs::write(&outside_mod, "value = \"secret\"\n").unwrap();
    let outside_txt = outside.join("secret.txt");
    fs::write(&outside_txt, "top-secret").unwrap();

    // Symlink inside sandbox pointing outside
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&outside_mod, root.join("link_escape.pkl")).unwrap();
        std::os::unix::fs::symlink(&outside_txt, root.join("link_escape.txt")).unwrap();
        std::os::unix::fs::symlink(&outside, root.join("link_dir")).unwrap();
    }

    // JARs inside and outside sandbox
    let jar_bytes = build_store_zip(&[("mod.pkl", b"v = \"from-jar\"\n")]);
    let inside_jar = root.join("ok.jar");
    let outside_jar = outside.join("out.jar");
    fs::write(&inside_jar, &jar_bytes).unwrap();
    fs::write(&outside_jar, &jar_bytes).unwrap();

    let real_root = fs::canonicalize(&root).unwrap();
    let main_uri = path_to_file_uri(&real_root.join("main.pkl")).to_java_utf8();

    let make_host = || {
        NativeHost::new(NativeHostConfig {
            root_dir: Some(root.clone()),
            ca_certs: CaCertSource::bundled_only(),
            ..Default::default()
        })
        .unwrap()
    };

    // Allowed file inside rootDir succeeds
    let (code, stdout, stderr) = eval_text(
        &make_host(),
        &main_uri,
        "import \"ok.pkl\"\noutput { text = ok.value }\n",
    );
    assert_eq!(code, 0, "stderr: {stderr}");
    assert_eq!(stdout, "inside");

    // Non-existent path outside rootDir returns rootDir SecurityManagerException (NOT NotFound!)
    let (code, _, stderr) = eval_text(
        &make_host(),
        &main_uri,
        "import \"../outside/does_not_exist.pkl\" as missing\noutput { text = missing.value }\n",
    );
    assert_ne!(code, 0);
    assert!(
        stderr.contains("is not within the root directory (`--root-dir`)"),
        "expected rootDir denial, got: {stderr}"
    );

    // Symlink escaping rootDir fails for import, read, and glob!
    #[cfg(unix)]
    {
        let (code, _, stderr) = eval_text(
            &make_host(),
            &main_uri,
            "import \"link_escape.pkl\"\noutput { text = link_escape.value }\n",
        );
        assert_ne!(code, 0);
        assert!(stderr.contains("root directory (`--root-dir`)"), "{stderr}");

        let (code, _, stderr) = eval_text(
            &make_host(),
            &main_uri,
            "output { text = read(\"link_escape.txt\").text }\n",
        );
        assert_ne!(code, 0);
        assert!(stderr.contains("root directory (`--root-dir`)"), "{stderr}");

        let (code, _, stderr) = eval_text(
            &make_host(),
            &main_uri,
            "m = import*(\"link_dir/*.pkl\")\noutput { text = m.length.toString() }\n",
        );
        assert_ne!(code, 0);
        assert!(stderr.contains("root directory (`--root-dir`)"), "{stderr}");
    }

    // jar:file: inside rootDir works; jar:file: outside rootDir is denied
    let in_jar_uri = format!(
        "jar:{}!/mod.pkl",
        path_to_file_uri(&inside_jar).to_java_utf8()
    );
    let (code, stdout, stderr) = eval_text(
        &make_host(),
        &main_uri,
        &format!("import \"{in_jar_uri}\" as j\noutput {{ text = j.v }}\n"),
    );
    assert_eq!(code, 0, "stderr: {stderr}");
    assert_eq!(stdout, "from-jar");

    let out_jar_uri = format!(
        "jar:{}!/mod.pkl",
        path_to_file_uri(&outside_jar).to_java_utf8()
    );
    let (code, _, stderr) = eval_text(
        &make_host(),
        &main_uri,
        &format!("import \"{out_jar_uri}\" as j\noutput {{ text = j.v }}\n"),
    );
    assert_ne!(code, 0);
    assert!(stderr.contains("root directory (`--root-dir`)"), "{stderr}");

    // UNC/authority file://127.0.0.1/... rejected immediately without network I/O
    let start = Instant::now();
    let (code, _, stderr) = eval_text(
        &make_host(),
        &main_uri,
        "import \"file://127.0.0.1/share/secret.pkl\" as unc\noutput { text = unc.value }\n",
    );
    assert_ne!(code, 0);
    assert!(start.elapsed() < Duration::from_secs(2));
    assert!(stderr.contains("root directory (`--root-dir`)"), "{stderr}");
}

// ---------------------------------------------------------------------------
// 2. ModulePathResolver: multi-entry dir + JAR + missing + ... + non-globbable + close
// ---------------------------------------------------------------------------

#[test]
fn modulepath_multi_entry_dir_jar_triple_dot_and_close() {
    let tmp = TempDir::new("mp");
    let dir1 = tmp.path().join("dir1");
    fs::create_dir_all(dir1.join("pkg")).unwrap();
    fs::write(dir1.join("pkg/dup.pkl"), "who = \"dir1\"\n").unwrap();
    fs::write(dir1.join("pkg/shared.pkl"), "base = \"from-parent\"\n").unwrap();
    fs::write(dir1.join("pkg/data.txt"), "hello-mp").unwrap();

    let jar_path = tmp.path().join("lib.jar");
    let jar_bytes = build_store_zip(&[
        ("pkg/dup.pkl", b"who = \"jar\"\n"),
        (
            "pkg/sub/child.pkl",
            b"import \".../shared.pkl\"\nimport \"../dup.pkl\"\nmsg = \"\\(shared.base):\\(dup.who):\\(read(\"../data.txt\").text)\"\n",
        ),
        ("pkg/Ignored.class", b"\xCA\xFE\xBA\xBE"),
    ]);
    fs::write(&jar_path, &jar_bytes).unwrap();

    let missing_entry = tmp.path().join("does-not-exist-dir");

    let make_host = || {
        NativeHost::new(NativeHostConfig {
            module_paths: vec![missing_entry.clone(), dir1.clone(), jar_path.clone()],
            ca_certs: CaCertSource::bundled_only(),
            ..Default::default()
        })
        .unwrap()
    };

    // Multi-entry dir+JAR + first-wins + triple-dot + relative read from modulepath module
    let (code, stdout, stderr) = eval_text(
        &make_host(),
        "file:///entry.pkl",
        "import \"modulepath:/pkg/sub/child.pkl\"\noutput { text = child.msg }\n",
    );
    assert_eq!(code, 0, "stderr: {stderr}");
    assert_eq!(stdout, "from-parent:dir1:hello-mp");

    // .class files skipped
    let (code, stdout, stderr) = eval_text(
        &make_host(),
        "file:///entry.pkl",
        "output { text = read?(\"modulepath:/pkg/Ignored.class\")?.text ?? \"absent\" }\n",
    );
    assert_eq!(code, 0, "stderr: {stderr}");
    assert_eq!(stdout, "absent");

    // Missing leading slash error
    let (code, _, stderr) = eval_text(
        &make_host(),
        "file:///entry.pkl",
        "import \"modulepath:no_slash.pkl\" as bad\noutput { text = bad.x }\n",
    );
    assert_ne!(code, 0);
    assert!(
        stderr.contains("missing a `/` after `modulepath:`"),
        "{stderr}"
    );

    // modulepath is NOT globbable
    let (code, _, stderr) = eval_text(
        &make_host(),
        "file:///entry.pkl",
        "m = import*(\"modulepath:/pkg/*.pkl\")\noutput { text = m.length.toString() }\n",
    );
    assert_ne!(code, 0);
    assert!(stderr.contains("is not globbable"), "{stderr}");

    // Direct resolver close() rejects subsequent lookups
    let mp = NativeModulePathResolver::new(vec![dir1.clone(), jar_path.clone()]);
    let fs_host = make_host().fs().clone();
    assert!(
        mp.has_element(&"modulepath:/pkg/dup.pkl".into(), &fs_host, false)
            .unwrap()
    );
    mp.close();
    assert!(mp.is_closed());
    assert!(
        mp.has_element(&"modulepath:/pkg/dup.pkl".into(), &fs_host, false)
            .is_err()
    );
}

// ---------------------------------------------------------------------------
// 3. Real Local HTTP Server: modules, resources, relative read, chunked, redirects, headers, 404
// ---------------------------------------------------------------------------

fn read_request_head(stream: &mut TcpStream) -> (String, Vec<(String, String)>) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut req_line = String::new();
    let _ = reader.read_line(&mut req_line);
    let mut headers = Vec::new();
    loop {
        let mut line = String::new();
        let n = reader.read_line(&mut line).unwrap_or(0);
        if n == 0 || line.trim().is_empty() {
            break;
        }
        if let Some((k, v)) = line.trim_end_matches(['\r', '\n']).split_once(':') {
            headers.push((k.trim().to_string(), v.trim().to_string()));
        }
    }
    (req_line.trim().to_string(), headers)
}

type RequestHeaderLog = Vec<(String, Vec<(String, String)>)>;

#[test]
fn real_http_server_modules_resources_redirects_chunked_headers_and_404() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let captured_headers: Arc<Mutex<RequestHeaderLog>> = Arc::new(Mutex::new(Vec::new()));
    let cap_clone = captured_headers.clone();

    let server = thread::spawn(move || {
        // Serve 6 requests then exit
        for _ in 0..6 {
            let Ok((mut stream, _)) = listener.accept() else {
                break;
            };
            let (req_line, headers) = read_request_head(&mut stream);
            let path = req_line
                .split_whitespace()
                .nth(1)
                .unwrap_or("/")
                .to_string();
            cap_clone.lock().unwrap().push((path.clone(), headers));

            let response: Vec<u8> = match path.as_str() {
                "/redir" => format!(
                    "HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:{port}/sub/mod.pkl\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                )
                .into_bytes(),
                "/sub/mod.pkl" => {
                    let body = b"val = read(\"data.txt\").text\n";
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    )
                    .into_bytes()
                    .into_iter()
                    .chain(body.iter().copied())
                    .collect()
                }
                "/data.txt" | "/sub/data.txt" => {
                    // Chunked transfer encoding
                    b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n5\r\nhello\r\n6\r\n-chunk\r\n0\r\n\r\n".to_vec()
                }
                "/missing.txt" | "/missing.pkl" => {
                    b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                        .to_vec()
                }
                _ => b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok"
                    .to_vec(),
            };
            let _ = stream.write_all(&response);
            let _ = stream.flush();
        }
    });

    let mut http_cfg = HttpClientConfig::default().with_test_port(Some(port));
    http_cfg
        .add_headers(
            &format!("http://127.0.0.1:{port}/redir"),
            [("X-Hop-Only".into(), "secret".into())],
        )
        .unwrap();

    let make_host = || {
        NativeHost::new(NativeHostConfig {
            allowed_modules: Some(vec!["pkl:".into(), "file:".into(), "http:".into()]),
            allowed_resources: Some(vec!["prop:".into(), "http:".into()]),
            http: http_cfg.clone(),
            ca_certs: CaCertSource::bundled_only(),
            ..Default::default()
        })
        .unwrap()
    };

    // Test --test-port (port 0 -> real port) + 302 redirect + relative read from HTTP module + chunked body + read? 404
    let src = format!(
        "import \"http://127.0.0.1:0/redir\" as m\n\
         miss = read?(\"http://127.0.0.1:{port}/missing.txt\")?.text ?? \"was-404\"\n\
         output {{ text = \"\\(m.val)|\\(miss)\" }}\n"
    );
    let (code, stdout, stderr) = eval_text(&make_host(), "file:///main.pkl", &src);
    assert_eq!(code, 0, "stderr: {stderr}");
    assert_eq!(stdout, "hello-chunk|was-404");

    // 404 on module import throws IOException (`badHttpStatusCode`)
    let (code, _, stderr) = eval_text(
        &make_host(),
        "file:///main.pkl",
        &format!("import \"http://127.0.0.1:{port}/missing.pkl\" as m\noutput {{ text = m.x }}\n"),
    );
    assert_ne!(code, 0);
    assert!(
        stderr.contains("Received unexpected status code `404`"),
        "{stderr}"
    );

    // 1 extra dummy request so server finishes cleanly
    let _ = make_host()
        .http_client()
        .get(&format!("http://127.0.0.1:{port}/done"), |_| Ok(()));
    server.join().unwrap();

    // Verify per-hop header isolation on real wire!
    let log = captured_headers.lock().unwrap();
    let redir_hdrs = &log.iter().find(|(p, _)| p == "/redir").unwrap().1;
    assert!(
        redir_hdrs
            .iter()
            .any(|(k, v)| k == "X-Hop-Only" && v == "secret")
    );
    let mod_hdrs = &log.iter().find(|(p, _)| p == "/sub/mod.pkl").unwrap().1;
    assert!(!mod_hdrs.iter().any(|(k, _)| k == "X-Hop-Only"));
}

// ---------------------------------------------------------------------------
// 4. Real HTTPS TLS Server + CA Certificate Precedence + Invalid CA Errors
// ---------------------------------------------------------------------------

#[test]
fn real_https_tls_ca_certificates_and_invalid_ca_errors() {
    // Bundled PklCARoots.pem is non-empty and loads cleanly into rustls
    assert!(BUNDLED_PKL_CA_ROOTS_PEM.len() > 100_000);
    assert!(CaCertSource::bundled_only().build_tls_config().is_ok());

    let tmp = TempDir::new("certs");
    // Missing CA cert file -> `cannotFindCertFile`
    let missing_pem = tmp.path().join("nope.pem");
    let err = CaCertSource::from_files(vec![missing_pem.clone()])
        .build_tls_config()
        .unwrap_err();
    assert!(format!("{err:?}").contains("Cannot find CA certificate file"));

    // Empty CA cert file -> `emptyCertFile`
    let empty_pem = tmp.path().join("empty.pem");
    fs::write(&empty_pem, "   \n").unwrap();
    let err = CaCertSource::from_files(vec![empty_pem])
        .build_tls_config()
        .unwrap_err();
    assert!(format!("{err:?}").contains("is empty."));

    // Corrupt PEM -> `cannotParseCertFile`
    let bad_pem = tmp.path().join("corrupt.pem");
    fs::write(&bad_pem, "not a certificate at all").unwrap();
    let err = CaCertSource::from_files(vec![bad_pem])
        .build_tls_config()
        .unwrap_err();
    assert!(format!("{err:?}").contains("Error parsing CA certificate file"));

    // Generate self-signed TLS cert for `localhost` / `127.0.0.1` via rcgen
    let cert =
        rcgen::generate_simple_self_signed(vec!["localhost".to_string(), "127.0.0.1".to_string()])
            .unwrap();
    let cert_pem = cert.cert.pem();
    let cert_der = CertificateDer::from(cert.cert.der().to_vec());
    let key_der = PrivateKeyDer::try_from(cert.key_pair.serialize_der()).unwrap();

    let cert_file = tmp.path().join("test-ca.pem");
    fs::write(&cert_file, &cert_pem).unwrap();

    let cacerts_dir = tmp.path().join("cacerts");
    fs::create_dir_all(&cacerts_dir).unwrap();
    fs::write(cacerts_dir.join("custom.pem"), &cert_pem).unwrap();

    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let server_cfg = Arc::new(
        ServerConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_no_client_auth()
            .with_single_cert(vec![cert_der], key_der)
            .unwrap(),
    );

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let srv_cfg = server_cfg.clone();
    let server = thread::spawn(move || {
        for _ in 0..3 {
            let Ok((tcp, _)) = listener.accept() else {
                break;
            };
            let conn = ServerConnection::new(srv_cfg.clone()).unwrap();
            let mut tls = StreamOwned::new(conn, tcp);
            let mut req_buf = [0u8; 1024];
            if tls.read(&mut req_buf).is_ok() {
                let body = b"secret = \"tls-ok\"\n";
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = tls.write_all(resp.as_bytes());
                let _ = tls.write_all(body);
                let _ = tls.flush();
            }
        }
    });

    let url = format!("https://127.0.0.1:{port}/secure.pkl");

    // 1. Default bundled PklCARoots.pem rejects self-signed server (`errorSslHandshake`)
    let host_untrusted = NativeHost::new(NativeHostConfig {
        ca_certs: CaCertSource::bundled_only(),
        ..Default::default()
    })
    .unwrap();
    let (code, _, stderr) = eval_text(
        &host_untrusted,
        "file:///main.pkl",
        &format!("import \"{url}\" as s\noutput {{ text = s.secret }}\n"),
    );
    assert_ne!(code, 0);
    assert!(
        stderr.contains("Error during SSL handshake with host"),
        "{stderr}"
    );

    // 2. Explicit `--ca-certificates` succeeds
    let host_explicit = NativeHost::new(NativeHostConfig {
        ca_certs: CaCertSource::from_files(vec![cert_file]),
        ..Default::default()
    })
    .unwrap();
    let (code, stdout, stderr) = eval_text(
        &host_explicit,
        "file:///main.pkl",
        &format!("import \"{url}\" as s\noutput {{ text = s.secret }}\n"),
    );
    assert_eq!(code, 0, "stderr: {stderr}");
    assert_eq!(stdout, "tls-ok");

    // 3. `~/.pkl/cacerts/` directory discovery succeeds
    let host_dir = NativeHost::new(NativeHostConfig {
        ca_certs: CaCertSource {
            cacerts_dir: Some(cacerts_dir),
            disable_home_cacerts: true,
            ..Default::default()
        },
        ..Default::default()
    })
    .unwrap();
    let (code, stdout, stderr) = eval_text(
        &host_dir,
        "file:///main.pkl",
        &format!("import \"{url}\" as s\noutput {{ text = s.secret }}\n"),
    );
    assert_eq!(code, 0, "stderr: {stderr}");
    assert_eq!(stdout, "tls-ok");

    server.join().unwrap();
}

// ---------------------------------------------------------------------------
// 5. Real Local HTTP Proxy + HTTPS CONNECT Tunnel Proxy
// ---------------------------------------------------------------------------

#[test]
fn real_http_and_https_connect_proxy_with_no_proxy() {
    // 1. Origin TLS server
    let cert = rcgen::generate_simple_self_signed(vec!["127.0.0.1".to_string()]).unwrap();
    let cert_pem = cert.cert.pem();
    let cert_der = CertificateDer::from(cert.cert.der().to_vec());
    let key_der = PrivateKeyDer::try_from(cert.key_pair.serialize_der()).unwrap();
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let tls_srv_cfg = Arc::new(
        ServerConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_no_client_auth()
            .with_single_cert(vec![cert_der], key_der)
            .unwrap(),
    );

    let origin_listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let origin_port = origin_listener.local_addr().unwrap().port();
    let origin_thread = thread::spawn(move || {
        let (tcp, _) = origin_listener.accept().unwrap();
        let conn = ServerConnection::new(tls_srv_cfg).unwrap();
        let mut tls = StreamOwned::new(conn, tcp);
        let mut buf = [0u8; 1024];
        let _ = tls.read(&mut buf);
        let body = b"proxied = \"https-via-connect\"\n";
        let resp = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        let _ = tls.write_all(resp.as_bytes());
        let _ = tls.write_all(body);
        let _ = tls.flush();
    });

    // 2. Proxy server supporting both plain HTTP GET and HTTPS CONNECT tunneling
    let proxy_listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let proxy_port = proxy_listener.local_addr().unwrap().port();
    let proxy_hits = Arc::new(AtomicUsize::new(0));
    let hits_clone = proxy_hits.clone();
    let proxy_thread = thread::spawn(move || {
        for _ in 0..2 {
            let Ok((mut client_stream, _)) = proxy_listener.accept() else {
                break;
            };
            hits_clone.fetch_add(1, Ordering::SeqCst);
            let (req_line, _) = read_request_head(&mut client_stream);
            if req_line.starts_with("CONNECT ") {
                let target = req_line.split_whitespace().nth(1).unwrap();
                let mut upstream = TcpStream::connect(target).unwrap();
                client_stream
                    .write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
                    .unwrap();
                client_stream.flush().unwrap();
                let mut c2 = client_stream.try_clone().unwrap();
                let mut u2 = upstream.try_clone().unwrap();
                let t1 = thread::spawn(move || {
                    let _ = std::io::copy(&mut client_stream, &mut upstream);
                });
                let _ = std::io::copy(&mut u2, &mut c2);
                let _ = t1.join();
            } else {
                // Plain HTTP proxy request uses absolute-form URI
                assert!(req_line.starts_with("GET http://"));
                let body = b"plain-via-proxy";
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = client_stream.write_all(resp.as_bytes());
                let _ = client_stream.write_all(body);
                let _ = client_stream.flush();
            }
        }
    });

    let proxy_cfg = ProxyConfig::new(Some(&format!("http://127.0.0.1:{proxy_port}")), &[]).unwrap();
    let ca = CaCertSource {
        extra_pem_bytes: vec![cert_pem.into_bytes()],
        disable_home_cacerts: true,
        ..Default::default()
    };

    // Plain HTTP via proxy
    let host_http = NativeHost::new(NativeHostConfig {
        allowed_resources: Some(vec!["prop:".into(), "http:".into()]),
        http: HttpClientConfig::default().with_proxy(proxy_cfg.clone()),
        ca_certs: ca.clone(),
        ..Default::default()
    })
    .unwrap();
    let (code, stdout, stderr) = eval_text(
        &host_http,
        "file:///main.pkl",
        "output { text = read(\"http://example.invalid/item\").text }\n",
    );
    assert_eq!(code, 0, "stderr: {stderr}");
    assert_eq!(stdout, "plain-via-proxy");

    // HTTPS via CONNECT tunnel proxy
    let host_https = NativeHost::new(NativeHostConfig {
        http: HttpClientConfig::default().with_proxy(proxy_cfg),
        ca_certs: ca,
        ..Default::default()
    })
    .unwrap();
    let (code, stdout, stderr) = eval_text(
        &host_https,
        "file:///main.pkl",
        &format!(
            "import \"https://127.0.0.1:{origin_port}/m.pkl\"\noutput {{ text = m.proxied }}\n"
        ),
    );
    assert_eq!(code, 0, "stderr: {stderr}");
    assert_eq!(stdout, "https-via-connect");

    origin_thread.join().unwrap();
    proxy_thread.join().unwrap();
    assert_eq!(proxy_hits.load(Ordering::SeqCst), 2);
}

// ---------------------------------------------------------------------------
// 6. Monotonic Timeout, Stalled HTTP CancelFlag Abort, & Standalone ImportGraph
// ---------------------------------------------------------------------------

#[test]
fn stalled_http_aborts_on_cancel_flag_and_monotonic_timeout_enforces_deadline() {
    // 1. Stalled TCP server: accepts connection and holds it open without responding
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let hold_thread = thread::spawn(move || {
        if let Ok((_sock, _)) = listener.accept() {
            thread::sleep(Duration::from_secs(3));
        }
    });

    let host = Arc::new(
        NativeHost::new(NativeHostConfig {
            allowed_resources: Some(vec!["prop:".into(), "http:".into()]),
            ca_certs: CaCertSource::bundled_only(),
            ..Default::default()
        })
        .unwrap(),
    );
    let cancel = host.cancel_flag();
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(120));
        cancel.cancel();
    });

    let start = Instant::now();
    let (code, _, stderr) = eval_text(
        &host,
        "file:///main.pkl",
        &format!("output {{ text = read(\"http://127.0.0.1:{port}/hang\").text }}\n"),
    );
    assert_ne!(code, 0);
    assert!(
        start.elapsed() < Duration::from_secs(2),
        "CancelFlag failed to abort stalled HTTP socket promptly ({:?})",
        start.elapsed()
    );
    assert!(!stderr.is_empty());
    let _ = hold_thread;

    // 2. Real wall-clock monotonic timeout on slow pure-Pkl evaluation
    let slow_host = NativeHost::new(NativeHostConfig {
        timeout_ms: Some(150),
        ca_certs: CaCertSource::bundled_only(),
        ..Default::default()
    })
    .unwrap();
    let (code, _, stderr) = eval_text(
        &slow_host,
        "file:///slow.pkl",
        "function spin(n) = if (n == 0) 0 else spin(n - 1) + spin(n - 1)\noutput { text = spin(40).toString() }\n",
    );
    assert_ne!(code, 0);
    assert_eq!(
        stderr, "Evaluation timed out after 0.15 second(s).\n",
        "expected exact timeout diagnostic without Pkl Error banner, got: {stderr:?}"
    );
}

#[test]
fn native_host_triple_dot_resource_read_and_nullable_read() {
    let tmp = TempDir::new("triple_dot_read");
    let nested = tmp.path().join("a").join("b");
    fs::create_dir_all(&nested).unwrap();

    let resource_path = tmp.path().join("shared.txt");
    fs::write(&resource_path, "triple-dot-content\n").unwrap();
    let module_path = nested.join("child.pkl");
    fs::write(
        &module_path,
        concat!(
            "found = read(\".../shared.txt\").text\n",
            "foundNullable = read?(\".../shared.txt\")?.text\n",
        ),
    )
    .unwrap();
    let missing_path = nested.join("missing.pkl");
    fs::write(&missing_path, "missing = read(\".../missing.txt\")\n").unwrap();

    let host = NativeHost::new(NativeHostConfig {
        ca_certs: CaCertSource::bundled_only(),
        ..Default::default()
    })
    .unwrap();
    let module_uri = path_to_file_uri(&fs::canonicalize(&module_path).unwrap());
    let res = host.eval_uri_output(
        &module_uri.to_java_utf8(),
        None,
        None,
        &ModuleEvalOptions::default(),
    );
    assert_eq!(
        res.exit_code,
        0,
        "stderr: {}",
        String::from_utf8_lossy(&res.stderr)
    );
    let stdout = String::from_utf8(res.stdout).unwrap();
    assert_eq!(
        stdout,
        "found = \"\"\"\n  triple-dot-content\n  \n  \"\"\"\nfoundNullable = \"\"\"\n  triple-dot-content\n  \n  \"\"\"\n"
    );

    let missing_uri = path_to_file_uri(&fs::canonicalize(&missing_path).unwrap());
    let missing_res = host.eval_uri_output(
        &missing_uri.to_java_utf8(),
        None,
        None,
        &ModuleEvalOptions::default(),
    );
    assert_eq!(missing_res.exit_code, 1);
    let missing_stderr = String::from_utf8(missing_res.stderr).unwrap();
    assert!(
        missing_stderr.contains("Cannot find resource `.../missing.txt`."),
        "unexpected missing triple-dot stderr: {missing_stderr}"
    );
}

#[test]
fn standalone_native_import_graph_handles_cycles_and_triple_dot() {
    let tmp = TempDir::new("analyze");
    let sub = tmp.path().join("sub");
    fs::create_dir_all(&sub).unwrap();

    let shared = tmp.path().join("shared.pkl");
    let a = sub.join("a.pkl");
    let b = sub.join("b.pkl");
    fs::write(&shared, "x = 1\n").unwrap();
    fs::write(&a, "import \"b.pkl\"\nimport \".../shared.pkl\"\n").unwrap();
    fs::write(&b, "import \"a.pkl\"\n").unwrap();

    let host = NativeHost::new(NativeHostConfig {
        ca_certs: CaCertSource::bundled_only(),
        ..Default::default()
    })
    .unwrap();
    let a_uri = path_to_file_uri(&fs::canonicalize(&a).unwrap());
    let b_uri = path_to_file_uri(&fs::canonicalize(&b).unwrap());
    let shared_uri = path_to_file_uri(&fs::canonicalize(&shared).unwrap());

    let graph = host.analyze_import_graph([a_uri.clone()]).unwrap();
    assert_eq!(graph.nodes.len(), 3);
    let node_a = graph.nodes.iter().find(|n| n.uri == a_uri).unwrap();
    assert!(node_a.imports.contains(&b_uri));
    assert!(node_a.imports.contains(&shared_uri));
    let node_b = graph.nodes.iter().find(|n| n.uri == b_uri).unwrap();
    assert_eq!(node_b.imports, vec![a_uri]);
}

#[test]
fn review_regressions_ca_errors_symlink_safe_modulepath_and_disk_package_cache() {
    use pkl_host_native::DiskPackageCache;
    use pkl_runtime::modules::ModuleHostError;
    use pkl_runtime::packages::PackageUri;
    use pkl_runtime::packages::cache::PackageCacheStore;

    let tmp = TempDir::new("wave2_regressions");

    // 1. CA certificate parsing messages match JDK CertificateFactory
    let random_pem = tmp.path().join("random.pem");
    fs::write(&random_pem, "not a certificate").unwrap();
    let err = CaCertSource::from_files(vec![random_pem.clone()])
        .build_tls_config()
        .unwrap_err();
    let msg = format!("{err:?}");
    assert!(
        msg.contains("No certificate data found"),
        "expected JDK message, got: {msg}"
    );

    let trunc_pem = tmp.path().join("truncated.pem");
    fs::write(&trunc_pem, "-----BEGIN CERTIFICATE-----\nMIIBIjANBgkqhkiG").unwrap();
    let err = CaCertSource::from_files(vec![trunc_pem])
        .build_tls_config()
        .unwrap_err();
    let msg = format!("{err:?}");
    assert!(
        msg.contains("Incomplete data"),
        "expected JDK Incomplete data message, got: {msg}"
    );

    // 2. ModulePath directory walker does NOT follow symlinks (symlink cycle safe) & rejects plain #!/bin/sh
    let mp_dir = tmp.path().join("mp");
    fs::create_dir_all(&mp_dir).unwrap();
    fs::write(mp_dir.join("real.pkl"), "v = 1\n").unwrap();
    let script = tmp.path().join("not_a_jar.sh");
    fs::write(&script, "#!/bin/sh\necho hello\n").unwrap();
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&mp_dir, mp_dir.join("loop_link")).unwrap();
    }
    let mp = NativeModulePathResolver::new(vec![mp_dir.clone(), script]);
    let fs = pkl_host_native::NativeFs::new(
        pkl_runtime::host::security::SecurityPolicy::default_standard(),
    );
    assert!(
        mp.has_element(&"modulepath:/real.pkl".into(), &fs, false)
            .unwrap()
    );
    assert!(
        !mp.has_element(&"modulepath:/loop_link/real.pkl".into(), &fs, false)
            .unwrap()
    );

    // 3. DiskPackageCache writes `package-2` 0444 files, deletes corrupt readonly metadata,
    //    and reports corrupt cached ZIP with `ZipException: zip END header not found`
    let cache_dir = tmp.path().join("cache");
    let cache = DiskPackageCache::new(cache_dir);
    let pkg = PackageUri::parse("package://localhost:0/alpha@1.0.0").unwrap();
    cache.write_metadata_bytes(&pkg, b"{bad json").unwrap();
    assert_eq!(
        cache.read_metadata_bytes(&pkg).unwrap().as_deref(),
        Some(&b"{bad json"[..])
    );
    cache.delete_metadata(&pkg).unwrap();
    assert!(cache.read_metadata_bytes(&pkg).unwrap().is_none());

    cache.write_zip_bytes(&pkg, b"not a zip archive").unwrap();
    match cache.read_zip_archive(&pkg) {
        Err(ModuleHostError::Io(io)) => {
            assert_eq!(
                io.message.unwrap().to_java_utf8(),
                "ZipException: zip END header not found"
            );
        }
        other => panic!("expected ZipException Io error, got: {other:?}"),
    }
}

#[test]
fn round4_platform_metadata_windows_uri_and_custom_security_enforcement() {
    use pkl_host_native::{
        CustomSecurityChecker, NativeHostExtensions, detect_native_platform_metadata, is_uri_like,
    };
    use pkl_runtime::value::PklString;

    // 1. P0-1: Native platform metadata reflects host OS & arch and never uses "Java native".
    let meta = detect_native_platform_metadata();
    assert_eq!(meta.java_version, "native");
    assert!(!meta.os_name.is_empty());
    assert!(!meta.cpu_architecture.is_empty());

    let default_host = NativeHost::new(NativeHostConfig {
        ca_certs: CaCertSource::bundled_only(),
        ..Default::default()
    })
    .unwrap();
    let (code, stdout, stderr) = eval_text(
        &default_host,
        "file:///round4_platform.pkl",
        "import \"pkl:platform\"\nimport \"pkl:release\"\n\
         output { text = \"\\(platform.current.operatingSystem.name)|\\(platform.current.processor.architecture)|\\(release.current.versionInfo)\" }\n",
    );
    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(
        stdout.starts_with(&format!("{}|", meta.os_name)),
        "expected host OS {}, got {stdout:?}",
        meta.os_name
    );
    assert!(
        !stdout.contains("Java native"),
        "unexpected stdout: {stdout}"
    );

    // 2. P0-2: Windows drive paths are never misclassified as URI schemes.
    assert!(!is_uri_like(r"C:\Users\dev\main.pkl"));
    assert!(!is_uri_like("C:/Users/dev/main.pkl"));
    assert!(!is_uri_like("d:\\config.pkl"));
    assert!(is_uri_like("file:///tmp/main.pkl"));
    assert!(is_uri_like("pkl:base"));
    assert!(is_uri_like("https://example.com/a.pkl"));

    // 3. P0-3 & P1-5: CustomSecurityChecker enforces env: glob, symlink canonicalization, and pkl:* stdlib.
    let tmp = TempDir::new("round4_custom_sec");
    let allowed_dir = tmp.path().join("allowed");
    let secret_dir = tmp.path().join("secret");
    fs::create_dir_all(&allowed_dir).unwrap();
    fs::create_dir_all(&secret_dir).unwrap();
    let allowed_canon = fs::canonicalize(&allowed_dir).unwrap();
    let secret_file = secret_dir.join("secret.txt");
    fs::write(&secret_file, "TOP_SECRET").unwrap();

    struct StrictChecker {
        allowed_prefix: String,
    }
    impl CustomSecurityChecker for StrictChecker {
        fn check_resolve_module(&self, uri: &PklString) -> Result<(), PklString> {
            let s = uri.to_java_utf8();
            if s == "pkl:json" {
                return Err("blocked pkl:json".into());
            }
            Ok(())
        }
        fn check_import_module(
            &self,
            _importing: &PklString,
            _imported: &PklString,
        ) -> Result<(), PklString> {
            Ok(())
        }
        fn check_read_resource(&self, resource: &PklString) -> Result<(), PklString> {
            let s = resource.to_java_utf8();
            if s.starts_with("env:") {
                return Err("blocked env".into());
            }
            if s.starts_with("file:") && !s.starts_with(&self.allowed_prefix) {
                return Err(format!("blocked outside allowed dir: {s}").into());
            }
            Ok(())
        }
    }

    let allowed_prefix = path_to_file_uri(&allowed_canon).to_java_utf8();
    let make_custom_host = || {
        let env_list = vec![("SECRET_TOKEN".to_string(), "supersecret".to_string())];
        NativeHost::with_extensions(
            NativeHostConfig {
                environment: env_list,
                ca_certs: CaCertSource::bundled_only(),
                ..Default::default()
            },
            NativeHostExtensions {
                custom_security: Some(Arc::new(StrictChecker {
                    allowed_prefix: allowed_prefix.clone(),
                })),
                ..Default::default()
            },
        )
        .unwrap()
    };

    // 3a. `read*("env:*")` must NOT bypass CustomSecurityChecker.
    let (code, _, stderr) = eval_text(
        &make_custom_host(),
        "file:///round4_env_glob.pkl",
        "x = read*(\"env:*\")\n",
    );
    assert_ne!(
        code, 0,
        "env:* glob must be rejected by CustomSecurityChecker"
    );
    assert!(stderr.contains("blocked env"), "stderr: {stderr}");

    // 3b. `import "pkl:json"` must NOT bypass CustomSecurityChecker.
    let (code, _, stderr) = eval_text(
        &make_custom_host(),
        "file:///round4_stdlib_deny.pkl",
        "import \"pkl:json\"\nx = new json.Renderer {}\n",
    );
    assert_ne!(
        code, 0,
        "pkl:json import must be rejected by CustomSecurityChecker"
    );
    assert!(stderr.contains("blocked pkl:json"), "stderr: {stderr}");

    // 3c. Symlink inside allowed dir pointing to secret file outside allowed dir must be blocked.
    #[cfg(unix)]
    {
        let link = allowed_dir.join("escape.txt");
        std::os::unix::fs::symlink(&secret_file, &link).unwrap();
        let link_uri = path_to_file_uri(&allowed_canon.join("escape.txt")).to_java_utf8();
        let (code, _, stderr) = eval_text(
            &make_custom_host(),
            "file:///round4_symlink_escape.pkl",
            &format!("x = read(\"{link_uri}\").text\n"),
        );
        assert_ne!(
            code, 0,
            "symlink escaping allowed dir must be blocked by canonicalized CustomSecurityChecker check"
        );
        assert!(
            stderr.contains("blocked outside allowed dir"),
            "stderr: {stderr}"
        );
    }
}
