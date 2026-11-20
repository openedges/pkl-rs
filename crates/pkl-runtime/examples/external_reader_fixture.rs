//! Fast, zero-JVM external module & resource reader test fixture speaking Pkl 0.32.0
//! MessagePack wire protocol (`0x26`–`0x32`) over stdin/stdout.
//!
//! Used by both native Rust integration tests (`tests/m4_external_reader_process.rs`)
//! and Java 0.32.0 raw-byte differential tests.
//!
//! Supported CLI flags:
//! - `--scheme <name>` (repeatable; defaults to `foo`)
//! - `--mode <default|echo-cwd|tree|opaque-glob|non-local|non-globbable|missing-resource|empty-module|unsupported-scheme|hang-on-close|hang-on-read|crash-immediately|crash-on-read|corrupt-stdout>`
//! - `--error <message>` (return `error: Some(message)` on read/list)
//! - `--stderr-noise` (write diagnostic text to stderr on each request)
//! - `--close-marker <path>` (write `closed\n` to `<path>` when `CloseExternalProcess` arrives)
//! - `--extra-arg <val>` (accepted and echoed in `echo-cwd` mode)

use pkl_runtime::external_reader::{
    ExternalReaderMessage, ModuleReaderSpec, ResourceReaderSpec, WirePathElement, codec,
};
use std::io::{self, BufReader, BufWriter, Write};
use std::path::PathBuf;
use std::time::Duration;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Mode {
    Default,
    EchoCwd,
    Tree,
    OpaqueGlob,
    NonLocal,
    NonGlobbable,
    MissingResource,
    EmptyModule,
    UnsupportedScheme,
    HangOnClose,
    HangOnRead,
    CrashImmediately,
    CrashOnRead,
    CorruptStdout,
}

struct Options {
    schemes: Vec<String>,
    mode: Mode,
    error_message: Option<String>,
    stderr_noise: bool,
    close_marker: Option<PathBuf>,
    raw_args: Vec<String>,
}

fn parse_args() -> Options {
    let raw_args: Vec<String> = std::env::args().skip(1).collect();
    let mut schemes = Vec::new();
    let mut mode = Mode::Default;
    let mut error_message = None;
    let mut stderr_noise = false;
    let mut close_marker = None;

    let mut i = 0;
    while i < raw_args.len() {
        match raw_args[i].as_str() {
            "--scheme" => {
                if let Some(val) = raw_args.get(i + 1) {
                    schemes.push(val.clone());
                    i += 2;
                    continue;
                }
            }
            "--mode" => {
                if let Some(val) = raw_args.get(i + 1) {
                    mode = match val.as_str() {
                        "default" => Mode::Default,
                        "echo-cwd" => Mode::EchoCwd,
                        "tree" => Mode::Tree,
                        "opaque-glob" => Mode::OpaqueGlob,
                        "non-local" => Mode::NonLocal,
                        "non-globbable" => Mode::NonGlobbable,
                        "missing-resource" => Mode::MissingResource,
                        "empty-module" => Mode::EmptyModule,
                        "unsupported-scheme" => Mode::UnsupportedScheme,
                        "hang-on-close" => Mode::HangOnClose,
                        "hang-on-read" => Mode::HangOnRead,
                        "crash-immediately" => Mode::CrashImmediately,
                        "crash-on-read" => Mode::CrashOnRead,
                        "corrupt-stdout" => Mode::CorruptStdout,
                        other => {
                            eprintln!("unknown --mode: {other}");
                            std::process::exit(2);
                        }
                    };
                    i += 2;
                    continue;
                }
            }
            "--error" => {
                if let Some(val) = raw_args.get(i + 1) {
                    error_message = Some(val.clone());
                    i += 2;
                    continue;
                }
            }
            "--stderr-noise" => {
                stderr_noise = true;
                i += 1;
                continue;
            }
            "--close-marker" => {
                if let Some(val) = raw_args.get(i + 1) {
                    close_marker = Some(PathBuf::from(val));
                    i += 2;
                    continue;
                }
            }
            _ => {}
        }
        i += 1;
    }

    if schemes.is_empty() {
        schemes.push("foo".to_string());
    }

    Options {
        schemes,
        mode,
        error_message,
        stderr_noise,
        close_marker,
        raw_args,
    }
}

fn supports_scheme(opts: &Options, scheme: &str) -> bool {
    opts.mode != Mode::UnsupportedScheme
        && opts.schemes.iter().any(|s| s.eq_ignore_ascii_case(scheme))
}

fn module_spec(opts: &Options, scheme: &str) -> Option<ModuleReaderSpec> {
    if !supports_scheme(opts, scheme) {
        return None;
    }
    let (has_hierarchical_uris, is_local, is_globbable) = match opts.mode {
        Mode::Tree => (true, true, true),
        Mode::OpaqueGlob => (false, true, true),
        Mode::NonLocal => (true, false, true),
        Mode::NonGlobbable => (true, true, false),
        _ => (false, true, false),
    };
    Some(ModuleReaderSpec {
        scheme: scheme.to_string(),
        has_hierarchical_uris,
        is_local,
        is_globbable,
    })
}

fn resource_spec(opts: &Options, scheme: &str) -> Option<ResourceReaderSpec> {
    if !supports_scheme(opts, scheme) {
        return None;
    }
    let (has_hierarchical_uris, is_globbable) = match opts.mode {
        Mode::Tree => (true, true),
        Mode::OpaqueGlob => (false, true),
        Mode::NonLocal => (true, true),
        Mode::NonGlobbable => (true, false),
        _ => (false, false),
    };
    Some(ResourceReaderSpec {
        scheme: scheme.to_string(),
        has_hierarchical_uris,
        is_globbable,
    })
}

fn pkl_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out
}

fn handle_read_module(opts: &Options, uri: &str) -> (Option<String>, Option<String>) {
    if let Some(err) = &opts.error_message {
        return (None, Some(err.clone()));
    }
    match opts.mode {
        Mode::EmptyModule => (None, None),
        Mode::EchoCwd => {
            let cwd = std::env::current_dir()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default();
            let pid = std::process::id();
            let args_pkl: Vec<String> = opts
                .raw_args
                .iter()
                .map(|a| format!("\"{}\"", pkl_escape(a)))
                .collect();
            let body = format!(
                "cwd = \"{}\"\npid = {pid}\nuri = \"{}\"\nargs = List({})\n",
                pkl_escape(&cwd),
                pkl_escape(uri),
                args_pkl.join(", ")
            );
            (Some(body), None)
        }
        Mode::Tree | Mode::NonLocal | Mode::NonGlobbable => {
            let path = uri.split_once(':').map(|(_, p)| p).unwrap_or("");
            match path {
                "/parent.pkl" => (Some("fromParent = \"yes\"\n".into()), None),
                "/a.pkl" => (Some("val = \"a\"\n".into()), None),
                "/b.pkl" => (Some("val = \"b\"\n".into()), None),
                "/dir/c.pkl" => (Some("val = \"c\"\n".into()), None),
                "/dir/sub/d.pkl" => (Some("val = \"d\"\n".into()), None),
                "/dir/sub/child.pkl" => (
                    Some("import \".../parent.pkl\"\nres = parent.fromParent\n".into()),
                    None,
                ),
                other => (None, Some(format!("module not found: {other}"))),
            }
        }
        Mode::OpaqueGlob => {
            let key = uri.split_once(':').map(|(_, k)| k).unwrap_or("");
            match key {
                "alpha" | "beta" | "gamma" => (Some(format!("item = \"{key}\"\n")), None),
                other => (None, Some(format!("unknown opaque module: {other}"))),
            }
        }
        _ => (
            Some(format!(
                "name = \"Pigeon\"\nage = 40\npid = {}\nuri = \"{}\"\n",
                std::process::id(),
                pkl_escape(uri)
            )),
            None,
        ),
    }
}

fn handle_read_resource(opts: &Options, uri: &str) -> (Option<Vec<u8>>, Option<String>) {
    if let Some(err) = &opts.error_message {
        return (None, Some(err.clone()));
    }
    match opts.mode {
        Mode::MissingResource => (None, None),
        Mode::EchoCwd => {
            let cwd = std::env::current_dir()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default();
            let payload = format!("cwd={cwd};pid={};uri={uri}", std::process::id());
            (Some(payload.into_bytes()), None)
        }
        Mode::Tree | Mode::NonLocal | Mode::NonGlobbable => {
            let path = uri.split_once(':').map(|(_, p)| p).unwrap_or("");
            match path {
                "/a.txt" => (Some(b"alpha".to_vec()), None),
                "/b.txt" => (Some(b"beta".to_vec()), None),
                "/dir/c.txt" => (Some(b"gamma".to_vec()), None),
                "/dir/sub/d.txt" => (Some(b"delta".to_vec()), None),
                _ => (None, None),
            }
        }
        Mode::OpaqueGlob => {
            let key = uri.split_once(':').map(|(_, k)| k).unwrap_or("");
            match key {
                "alpha" | "beta" | "gamma" => (Some(format!("res:{key}").into_bytes()), None),
                _ => (None, None),
            }
        }
        _ => (Some(format!("hello:{uri}").into_bytes()), None),
    }
}

fn handle_list_elements(
    opts: &Options,
    uri: &str,
    for_modules: bool,
) -> (Option<Vec<WirePathElement>>, Option<String>) {
    if let Some(err) = &opts.error_message {
        return (None, Some(err.clone()));
    }
    match opts.mode {
        Mode::Tree | Mode::NonLocal => {
            let path = uri.split_once(':').map(|(_, p)| p).unwrap_or("/");
            let ext = if for_modules { "pkl" } else { "txt" };
            let elems = match path {
                "/" | "" => vec![
                    WirePathElement::directory("dir"),
                    WirePathElement::file(format!("b.{ext}")),
                    WirePathElement::file(format!("a.{ext}")),
                ],
                "/dir/" | "/dir" => vec![
                    WirePathElement::directory("sub"),
                    WirePathElement::file(format!("c.{ext}")),
                ],
                "/dir/sub/" | "/dir/sub" => vec![WirePathElement::file(format!("d.{ext}"))],
                _ => vec![],
            };
            (Some(elems), None)
        }
        Mode::OpaqueGlob => (
            Some(vec![
                WirePathElement::file("gamma"),
                WirePathElement::file("alpha"),
                WirePathElement::file("beta"),
            ]),
            None,
        ),
        _ => (Some(Vec::new()), None),
    }
}

fn main() {
    let opts = parse_args();

    if opts.mode == Mode::CrashImmediately {
        eprintln!("external_reader_fixture: intentional immediate crash");
        std::process::exit(42);
    }

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = BufReader::new(stdin.lock());
    let mut writer = BufWriter::new(stdout.lock());

    loop {
        let msg = match codec::read_message(&mut reader) {
            Ok(Some(msg)) => msg,
            Ok(None) => break,
            Err(err) => {
                eprintln!("external_reader_fixture decode error: {err}");
                std::process::exit(3);
            }
        };

        if opts.stderr_noise {
            eprintln!(
                "external_reader_fixture[stderr-noise]: received {:?}",
                msg.message_type()
            );
        }

        if opts.mode == Mode::CorruptStdout
            && !matches!(msg, ExternalReaderMessage::CloseExternalProcess)
        {
            let _ = writer.write_all(&[0x02]);
            let _ = writer.flush();
            return;
        }

        let response = match msg {
            ExternalReaderMessage::CloseExternalProcess => {
                if let Some(marker) = &opts.close_marker {
                    let _ = std::fs::write(marker, b"closed\n");
                }
                if opts.mode == Mode::HangOnClose {
                    loop {
                        std::thread::sleep(Duration::from_secs(60));
                    }
                }
                break;
            }
            ExternalReaderMessage::InitializeModuleReaderRequest { request_id, scheme } => {
                ExternalReaderMessage::InitializeModuleReaderResponse {
                    request_id,
                    spec: module_spec(&opts, &scheme),
                }
            }
            ExternalReaderMessage::InitializeResourceReaderRequest { request_id, scheme } => {
                ExternalReaderMessage::InitializeResourceReaderResponse {
                    request_id,
                    spec: resource_spec(&opts, &scheme),
                }
            }
            ExternalReaderMessage::ReadModuleRequest {
                request_id,
                evaluator_id,
                uri,
            } => {
                if opts.mode == Mode::HangOnRead {
                    loop {
                        std::thread::sleep(Duration::from_secs(60));
                    }
                }
                if opts.mode == Mode::CrashOnRead {
                    std::process::exit(43);
                }
                let (contents, error) = handle_read_module(&opts, &uri);
                ExternalReaderMessage::ReadModuleResponse {
                    request_id,
                    evaluator_id,
                    contents,
                    error,
                }
            }
            ExternalReaderMessage::ReadResourceRequest {
                request_id,
                evaluator_id,
                uri,
            } => {
                if opts.mode == Mode::HangOnRead {
                    loop {
                        std::thread::sleep(Duration::from_secs(60));
                    }
                }
                if opts.mode == Mode::CrashOnRead {
                    std::process::exit(43);
                }
                let (contents, error) = handle_read_resource(&opts, &uri);
                ExternalReaderMessage::ReadResourceResponse {
                    request_id,
                    evaluator_id,
                    contents,
                    error,
                }
            }
            ExternalReaderMessage::ListModulesRequest {
                request_id,
                evaluator_id,
                uri,
            } => {
                let (path_elements, error) = handle_list_elements(&opts, &uri, true);
                ExternalReaderMessage::ListModulesResponse {
                    request_id,
                    evaluator_id,
                    path_elements,
                    error,
                }
            }
            ExternalReaderMessage::ListResourcesRequest {
                request_id,
                evaluator_id,
                uri,
            } => {
                let (path_elements, error) = handle_list_elements(&opts, &uri, false);
                ExternalReaderMessage::ListResourcesResponse {
                    request_id,
                    evaluator_id,
                    path_elements,
                    error,
                }
            }
            other => {
                eprintln!("external_reader_fixture: unexpected message {other:?}");
                std::process::exit(4);
            }
        };

        if let Err(err) = codec::write_message(&mut writer, &response) {
            eprintln!("external_reader_fixture write error: {err}");
            std::process::exit(5);
        }
    }
}
