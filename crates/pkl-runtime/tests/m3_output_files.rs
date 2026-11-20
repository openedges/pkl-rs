//! M3-03 Slice B: `Module.output.files`, fast-path `output.bytes`, CLI `-m`/`-f`/`-o`/`-x`/multi-module,
//! and host `PlatformMetadata` wiring parity tests against Pkl 0.32.0.
use pkl_runtime::{
    evaluation::{
        frontend::{DiagnosticProfile, ModuleEvalOptions, ModuleSourceInput},
        output_files::{
            OutputFileEntry, check_path_spec, evaluate_output_files,
            evaluate_output_files_with_host,
        },
        session::{SourceSession, SourceSessionOptions},
    },
    host::HostPoll,
    modules::{ModuleCompletion, ModuleRequest, ModuleRequestKind, ModuleSource},
    probe::{SourceOutputFile, SourceProbe},
    value::{ErrorKind, PklString, Value},
};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

#[path = "../examples/module_eval_probe.rs"]
mod module_eval_probe;

const _: fn() = module_eval_probe::main;

struct TempDir(PathBuf);

impl TempDir {
    fn new(label: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let base = std::env::temp_dir()
            .canonicalize()
            .unwrap_or_else(|_| std::env::temp_dir());
        let dir = base.join(format!(
            "pkl-m3-output-files-{label}-{}-{id}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    fn path(&self) -> &Path {
        &self.0
    }

    fn write(&self, rel: &str, contents: &str) -> PathBuf {
        let path = self.0.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&path, contents).unwrap();
        path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct CliRun {
    exit_code: i32,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

fn run_cli(cwd: &Path, args: &[&str]) -> CliRun {
    let args: Vec<String> = args.iter().map(|s| (*s).to_owned()).collect();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let exit_code = module_eval_probe::run_with_io(&args, cwd, &mut stdout, &mut stderr)
        .expect("probe CLI execution should return exit code");
    CliRun {
        exit_code,
        stdout,
        stderr,
    }
}

fn settle_session(session: &mut SourceSession, now: u64) -> HostPoll {
    for _ in 0..200_000 {
        let event = session.poll(now, 1);
        let roots = match &event {
            HostPoll::Ready(Ok(value)) => vec![value.clone()],
            _ => Vec::new(),
        };
        session.collect(&roots);
        if let HostPoll::Read { token, uri } = &event
            && (uri.starts_with("prop:") || uri.starts_with("env:"))
        {
            session.resume_default_read(token, &[], &[], now).unwrap();
            continue;
        }
        if !matches!(event, HostPoll::Yielded) {
            return event;
        }
    }
    panic!("session did not settle");
}

#[test]
fn evaluate_output_files_basic_and_renderers_match_upstream_test() {
    let program = r#"
output {
  files {
    ["foo.yml"] {
      text = "foo: foo text"
    }
    ["bar.yml"] {
      text = "bar: bar text"
    }
    ["bar/biz.yml"] {
      text = "biz: bar biz"
    }
    ["bar/../bark.yml"] {
      text = "bark: bark bark"
    }
  }
}
"#;
    let res = evaluate_output_files(
        ModuleSourceInput {
            text: program,
            uri: "file:///multi.pkl",
            inferred_name: "multi",
        },
        &ModuleEvalOptions::default(),
    );
    assert_eq!(res.exit_code, 0, "{}", String::from_utf8_lossy(&res.stderr));
    assert!(res.stderr.is_empty());
    assert_eq!(
        res.files,
        vec![
            OutputFileEntry {
                path_spec: "foo.yml".into(),
                bytes: b"foo: foo text".as_slice().into(),
            },
            OutputFileEntry {
                path_spec: "bar.yml".into(),
                bytes: b"bar: bar text".as_slice().into(),
            },
            OutputFileEntry {
                path_spec: "bar/biz.yml".into(),
                bytes: b"biz: bar biz".as_slice().into(),
            },
            OutputFileEntry {
                path_spec: "bar/../bark.yml".into(),
                bytes: b"bark: bark bark".as_slice().into(),
            },
        ]
    );

    let rendered = evaluate_output_files(
        ModuleSourceInput {
            text: r#"
import "pkl:pklbinary"

output {
  files {
    ["foo.json"] {
      value = new {
        foo = "fooey"
        bar = "barrey"
      }
      renderer = new JsonRenderer {}
    }
    ["bar.yaml"] {
      value = new { x = 1 }
      renderer = new YamlRenderer {}
    }
    ["bin.msgpack"] {
      value = Bytes(0, 1, 128, 255)
      renderer = new pklbinary.Renderer {}
    }
  }
}
"#,
            uri: "file:///rendered.pkl",
            inferred_name: "rendered",
        },
        &ModuleEvalOptions::default(),
    );
    assert_eq!(
        rendered.exit_code,
        0,
        "{}",
        String::from_utf8_lossy(&rendered.stderr)
    );
    assert_eq!(rendered.files.len(), 3);
    assert_eq!(rendered.files[0].path_spec, "foo.json");
    assert_eq!(
        rendered.files[0].bytes.as_ref(),
        b"{\n  \"foo\": \"fooey\",\n  \"bar\": \"barrey\"\n}\n"
    );
    assert_eq!(rendered.files[1].path_spec, "bar.yaml");
    assert_eq!(rendered.files[1].bytes.as_ref(), b"x: 1\n");
    assert_eq!(rendered.files[2].path_spec, "bin.msgpack");
    // MessagePack pklbinary encoding of Bytes(0, 1, 128, 255) (verified against Java 0.32.0):
    // fixarray(2) [0x92], fixint(0x0f = Code.BYTES) [0x0f], bin8(4) [0xc4, 0x04], 0x00, 0x01, 0x80, 0xff
    assert_eq!(
        rendered.files[2].bytes.as_ref(),
        &[0x92, 0x0f, 0xc4, 0x04, 0x00, 0x01, 0x80, 0xff]
    );
}

#[test]
fn evaluate_output_files_null_empty_defaults_generators_and_amendment_order() {
    for (name, source) in [
        ("omitted", "x = 1\n"),
        ("explicit_null", "output { files = null }\n"),
        ("empty_mapping", "output { files {} }\n"),
    ] {
        let res = evaluate_output_files(
            ModuleSourceInput {
                text: source,
                uri: "file:///empty.pkl",
                inferred_name: name,
            },
            &ModuleEvalOptions::default(),
        );
        assert_eq!(res.exit_code, 0, "{name}: {:?}", res.stderr);
        assert!(res.files.is_empty(), "{name}: expected empty files");
    }

    let complex = r#"
local baseFiles: Mapping<String, FileOutput> = new {
  ["first.txt"] { text = "original" }
  ["second.txt"] { text = "two" }
}

output {
  files = (baseFiles) {
    default {
      renderer = new JsonRenderer {}
    }
    ["first.txt"] { text = "overridden" }
    for (i in List(1, 2)) {
      ["gen/\(i).json"] {
        value = new { n = i }
      }
    }
    when (false) {
      ["skipped.txt"] { text = "nope" }
    }
  }
}
"#;
    let res = evaluate_output_files(
        ModuleSourceInput {
            text: complex,
            uri: "file:///complex.pkl",
            inferred_name: "complex",
        },
        &ModuleEvalOptions::default(),
    );
    assert_eq!(res.exit_code, 0, "{}", String::from_utf8_lossy(&res.stderr));
    let keys: Vec<_> = res
        .files
        .iter()
        .map(|f| f.path_spec.as_str().unwrap())
        .collect();
    assert_eq!(
        keys,
        vec!["first.txt", "second.txt", "gen/1.json", "gen/2.json"]
    );
    assert_eq!(res.files[0].bytes.as_ref(), b"overridden");
    assert_eq!(res.files[1].bytes.as_ref(), b"two");
    assert_eq!(res.files[2].bytes.as_ref(), b"{\n  \"n\": 1\n}\n");
    assert_eq!(res.files[3].bytes.as_ref(), b"{\n  \"n\": 2\n}\n");

    // Importing another module's `.output` (`ModuleOutput extends FileOutput`)
    let with_import = evaluate_output_files_with_host(
        ModuleSourceInput {
            text: r#"
output {
  files {
    ["child.pcf"] = import("child.pkl").output
  }
}
"#,
            uri: "file:///parent.pkl",
            inferred_name: "parent",
        },
        &ModuleEvalOptions::default(),
        |request: &ModuleRequest| match &request.kind {
            ModuleRequestKind::SelectKey { id, uri, .. } if uri == "file:///child.pkl" => {
                ModuleCompletion::KeySelected(Ok(pkl_runtime::modules::ModuleKey {
                    id: *id,
                    uri: uri.clone(),
                    kind: pkl_runtime::modules::ModuleKeyKind::Host,
                    handle: "child".into(),
                }))
            }
            ModuleRequestKind::QueryCachePolicy { key } if key.uri == "file:///child.pkl" => {
                ModuleCompletion::CachePolicy(Ok(true))
            }
            ModuleRequestKind::Resolve { key, .. } | ModuleRequestKind::ResolveKey { key, .. }
                if key.uri == "file:///child.pkl" =>
            {
                ModuleCompletion::Resolved(Ok(pkl_runtime::modules::ResolvedModule {
                    original_uri: key.uri.clone(),
                    resolved_uri: key.uri.clone(),
                    source_uri: key.uri.clone(),
                }))
            }
            ModuleRequestKind::LoadSource { resolved, .. }
                if resolved.original_uri == "file:///child.pkl" =>
            {
                ModuleCompletion::Source(Ok(ModuleSource {
                    text: "greeting = \"hi\"\n".into(),
                    inferred_name: "child".into(),
                }))
            }
            _ => pkl_runtime::modules::default_or_unsupported_completion(request),
        },
    );
    assert_eq!(
        with_import.exit_code,
        0,
        "{}",
        String::from_utf8_lossy(&with_import.stderr)
    );
    assert_eq!(with_import.files.len(), 1);
    assert_eq!(with_import.files[0].path_spec, "child.pcf");
    assert_eq!(with_import.files[0].bytes.as_ref(), b"greeting = \"hi\"\n");
}

#[test]
fn evaluate_output_files_preserves_exotic_keys_utf16_surrogates_and_all_256_bytes() {
    let mut byte_args = Vec::new();
    for b in 0..=255u16 {
        byte_args.push(b.to_string());
    }
    let all_bytes_expr = format!("Bytes({})", byte_args.join(", "));
    let source = format!(
        r#"
output {{
  files {{
    [""] {{ bytes = Bytes() }}
    ["a:b:c"] {{ bytes = Bytes(65) }}
    ["日本語/🎉.bin"] {{ bytes = Bytes(65, 66) }}
    ["3:bytes:here"] {{ bytes = Bytes(65, 66, 67) }}
    ["all256.bin"] {{ bytes = {all_bytes_expr} }}
    [read("life:surrogate-key")] {{ bytes = Bytes(0, 255, 0) }}
  }}
}}
"#
    );
    let surrogate_key = PklString::from_utf16(vec![0x0061, 0xd800, 0x003a, 0xd83c, 0xdf89]);
    let mut session = SourceSession::compile_output(
        ModuleSourceInput {
            text: &source,
            uri: "file:///exotic.pkl",
            inferred_name: "exotic",
        },
        &[],
        &ModuleEvalOptions::default(),
        SourceSessionOptions::default(),
    )
    .unwrap();
    session.start_output_files(0, None).unwrap();
    let files = loop {
        match settle_session(&mut session, 0) {
            HostPoll::Read { token, uri } if uri == "life:surrogate-key" => {
                session
                    .resume(&token, Ok(Value::String(surrogate_key.clone())), 0)
                    .unwrap();
            }
            HostPoll::Ready(Ok(value)) => break SourceSession::result_output_files(value).unwrap(),
            other => panic!("unexpected event: {other:?}"),
        }
    };
    assert_eq!(files.len(), 6);
    assert_eq!(files[0].path_spec, "");
    assert_eq!(files[0].bytes.as_ref(), b"");
    assert_eq!(files[1].path_spec, "a:b:c");
    assert_eq!(files[1].bytes.as_ref(), b"A");
    assert_eq!(files[2].path_spec, "日本語/🎉.bin");
    assert_eq!(files[2].bytes.as_ref(), b"AB");
    assert_eq!(files[3].path_spec, "3:bytes:here");
    assert_eq!(files[3].bytes.as_ref(), b"ABC");
    assert_eq!(files[4].path_spec, "all256.bin");
    assert_eq!(
        files[4].bytes.as_ref(),
        (0u8..=255u8).collect::<Vec<_>>().as_slice()
    );
    assert_eq!(files[5].path_spec, surrogate_key);
    assert_eq!(files[5].bytes.as_ref(), &[0, 255, 0]);
    session.close();
    session.collect(&[]);
    assert_eq!(session.heap_stats().slots, 0);
}

#[test]
fn evaluate_output_files_invalid_output_and_entry_errors_have_no_repl_frame() {
    let invalid_output = evaluate_output_files(
        ModuleSourceInput {
            text: "output = \"bad\"\n",
            uri: "file:///invalid-output.pkl",
            inferred_name: "invalidOutput",
        },
        &ModuleEvalOptions::default(),
    );
    assert_eq!(invalid_output.exit_code, 1);
    let stderr = String::from_utf8_lossy(&invalid_output.stderr);
    assert!(
        stderr.contains("Expected value of type `ModuleOutput`, but got type `String`."),
        "unexpected stderr: {stderr}"
    );
    assert!(
        stderr.contains("at pkl.base#Module.output"),
        "unexpected stderr: {stderr}"
    );
    assert!(!stderr.contains("repl:text"));

    let mut session = SourceSession::compile_output(
        ModuleSourceInput {
            text: "stable = 7\noutput { files { [\"bad.txt\"] { text = throw(\"file boom\") } } }\n",
            uri: "file:///entry-fail.pkl",
            inferred_name: "entryFail",
        },
        &["stable".into()],
        &ModuleEvalOptions {
            diagnostic_profile: DiagnosticProfile::ApiIdentity,
            ..Default::default()
        },
        SourceSessionOptions::default(),
    )
    .unwrap();
    session.start_output_files(0, None).unwrap();
    let HostPoll::Ready(Err(error)) = settle_session(&mut session, 0) else {
        panic!("expected entry failure");
    };
    let rendered = session.output_failure(error);
    let text = String::from_utf8_lossy(&rendered.stderr);
    assert!(text.contains("file boom"), "{text}");
    assert!(!text.contains("repl:text"), "{text}");
    assert!(!session.setup_failed());

    session.start_path(0, 0, None).unwrap();
    let HostPoll::Ready(Ok(value)) = settle_session(&mut session, 0) else {
        panic!("session should stay usable after output.files failure");
    };
    assert_eq!(
        SourceSession::result_string(value).unwrap().as_str(),
        Some("7")
    );
    session.close();
}

#[test]
fn output_files_session_and_probe_suspend_reenter_gc_timeout_and_close() {
    // 1. SourceSession suspend + reenter_path + reenter_output_files + GC + close
    let mut session = SourceSession::compile_output(
        ModuleSourceInput {
            text: r#"
stable = 9
probe = read("life:outer")
output {
  files {
    ["a.txt"] { text = read("life:body") }
    ["b.txt"] { text = "static-\(stable)" }
  }
}
"#,
            uri: "file:///lifecycle.pkl",
            inferred_name: "lifecycle",
        },
        &["stable".into(), "probe".into()],
        &ModuleEvalOptions {
            diagnostic_profile: DiagnosticProfile::ApiIdentity,
            ..Default::default()
        },
        SourceSessionOptions::default(),
    )
    .unwrap();

    // Start path `probe` -> suspends on life:outer -> reenter_output_files!
    session.start_path(1, 0, None).unwrap();
    let HostPoll::Read {
        token: outer_token,
        uri: outer_uri,
    } = settle_session(&mut session, 0)
    else {
        panic!("expected outer read");
    };
    assert_eq!(outer_uri, "life:outer");

    session.reenter_output_files(&outer_token, 0, None).unwrap();
    let HostPoll::Read {
        token: body_token,
        uri: body_uri,
    } = settle_session(&mut session, 0)
    else {
        panic!("expected nested output.files read");
    };
    assert_eq!(body_uri, "life:body");

    // Reenter a property path from inside the nested output.files read!
    session.reenter_path(&body_token, 0, 0, None).unwrap();
    let HostPoll::Ready(Ok(val)) = settle_session(&mut session, 0) else {
        panic!("expected inner property result");
    };
    assert_eq!(SourceSession::result_string(val).unwrap(), "9");

    // Resume nested output.files read and finish
    session
        .resume(&body_token, Ok(Value::string("dynamic-body")), 0)
        .unwrap();
    let HostPoll::Ready(Ok(files_val)) = settle_session(&mut session, 0) else {
        panic!("expected nested output.files completion");
    };
    let files = SourceSession::result_output_files(files_val).unwrap();
    assert_eq!(
        files,
        vec![
            OutputFileEntry {
                path_spec: "a.txt".into(),
                bytes: b"dynamic-body".as_slice().into(),
            },
            OutputFileEntry {
                path_spec: "b.txt".into(),
                bytes: b"static-9".as_slice().into(),
            },
        ]
    );

    // Resume outer property path
    session
        .resume(&outer_token, Ok(Value::string("done")), 0)
        .unwrap();
    let HostPoll::Ready(Ok(outer_val)) = settle_session(&mut session, 0) else {
        panic!("expected outer completion");
    };
    assert_eq!(SourceSession::result_string(outer_val).unwrap(), "done");

    session.close();
    session.collect(&[]);
    assert_eq!(session.heap_stats().slots, 0);
    assert_eq!(
        session.start_output_files(0, None).unwrap_err().kind,
        ErrorKind::Closed
    );

    // 2. Timeout cancels pending read without caching setup failure
    let mut timeout_session = SourceSession::compile_output(
        ModuleSourceInput {
            text: "output { files { [\"p.txt\"] { text = read(\"life:pending\") } } }\n",
            uri: "file:///timeout.pkl",
            inferred_name: "timeout",
        },
        &[],
        &ModuleEvalOptions::default(),
        SourceSessionOptions::default(),
    )
    .unwrap();
    timeout_session.start_output_files(0, Some(5)).unwrap();
    let HostPoll::Read { token, .. } = settle_session(&mut timeout_session, 0) else {
        panic!("expected pending read");
    };
    let HostPoll::Ready(Err(error)) = settle_session(&mut timeout_session, 5) else {
        panic!("expected timeout");
    };
    assert_eq!(error.kind, ErrorKind::Timeout);
    assert!(!timeout_session.setup_failed());
    assert_eq!(timeout_session.drain_cancelled_reads(), vec![token]);
    timeout_session.close();
    assert_eq!(timeout_session.heap_stats().slots, 0);

    // 3. SourceProbe `"files"` event (start_output_files + reenter_output_files)
    let mut probe = SourceProbe::new_output(
        ModuleSourceInput {
            text: r#"
output {
  files {
    ["one.txt"] { text = read("prop:val") }
    ["two.txt"] { bytes = Bytes(1, 2, 3) }
  }
}
"#,
            uri: "file:///probe-files.pkl",
            inferred_name: "probeFiles",
        },
        &[],
        &ModuleEvalOptions {
            properties: vec![("val".into(), "from-prop".into())],
            diagnostic_profile: DiagnosticProfile::ApiIdentity,
            ..Default::default()
        },
        SourceSessionOptions::default(),
    )
    .unwrap();
    probe.start_output_files(0, None).unwrap();
    let files_event = loop {
        let event = probe.poll(0, 1).unwrap();
        probe.collect();
        match event.kind {
            "yield" => {}
            "read" => probe.respond_configured(&event.token.unwrap(), 0).unwrap(),
            "files" => break event,
            other => panic!("unexpected probe event: {other:?} ({event:?})"),
        }
    };
    assert_eq!(files_event.exit_code, Some(0));
    assert_eq!(files_event.phase, Some("module-output"));
    assert_eq!(
        files_event.files,
        Some(vec![
            SourceOutputFile {
                path_spec: "one.txt".encode_utf16().collect(),
                bytes: b"from-prop".to_vec(),
            },
            SourceOutputFile {
                path_spec: "two.txt".encode_utf16().collect(),
                bytes: vec![1, 2, 3],
            },
        ])
    );
    probe.close();
    probe.collect();
    assert_eq!(probe.heap_slots(), 0);
}

#[test]
fn source_session_and_probe_fast_path_output_bytes_alongside_output_text() {
    let source = r#"
import "pkl:pklbinary"
val = 42
output {
  value = val
  renderer = new pklbinary.Renderer {}
}
"#;
    let mut probe = SourceProbe::new_expressions(
        source,
        "file:///fast-bytes.pkl",
        "fastBytes",
        &["output.bytes".into(), "val".into()],
        "compact",
        false,
        false,
        false,
    )
    .unwrap();

    probe.start_path(0, 0, None).unwrap();
    let bytes_event = loop {
        let event = probe.poll(0, 1).unwrap();
        probe.collect();
        match event.kind {
            "yield" => {}
            "module-request" => {
                let token = event.token.unwrap();
                assert!(probe.complete_default_module(&token, 0).unwrap());
            }
            _ => break event,
        }
    };
    assert_eq!(bytes_event.kind, "bytes");
    // MessagePack fixint 42 = [0x2a]
    assert_eq!(bytes_event.bytes.as_deref(), Some([0x2a].as_slice()));

    probe.start_path(1, 0, None).unwrap();
    let val_event = loop {
        let event = probe.poll(0, 1).unwrap();
        probe.collect();
        match event.kind {
            "yield" => {}
            "module-request" => {
                let token = event.token.unwrap();
                assert!(probe.complete_default_module(&token, 0).unwrap());
            }
            _ => break event,
        }
    };
    assert_eq!(val_event.kind, "value");
    assert_eq!(PklString::from_utf16(val_event.value.unwrap()), "42");
    probe.close();
    assert_eq!(probe.heap_slots(), 0);

    // Invalid `output` fails with CheckModuleOutput and no repl:text frame
    let mut bad = SourceSession::compile_expressions(
        "output = 123\n",
        "file:///bad-output.pkl",
        "badOutput",
        &["output.bytes".into(), "output.text".into()],
        SourceSessionOptions::default(),
    )
    .unwrap();
    bad.start_path(0, 0, None).unwrap();
    let HostPoll::Ready(Err(err)) = settle_session(&mut bad, 0) else {
        panic!("expected CheckModuleOutput error");
    };
    let rendered = bad.format_error(err);
    assert!(
        rendered.contains("Expected value of type `ModuleOutput`, but got type `Int`."),
        "{rendered}"
    );
    assert!(rendered.contains("at pkl.base#Module.output"), "{rendered}");
    assert!(!rendered.contains("repl:text"), "{rendered}");
    bad.close();
}

#[test]
fn check_path_spec_matches_posix_and_windows_upstream_rules() {
    assert!(check_path_spec(&"foo/bar/baz.txt".into(), false).is_ok());
    assert!(check_path_spec(&"foo:bar".into(), false).is_ok());
    assert!(check_path_spec(&"foo\\bar".into(), false).is_ok());
    assert_eq!(
        check_path_spec(&"a\0b".into(), false).unwrap_err(),
        "Path spec `a\0b` contains illegal character `\0`."
    );

    assert!(check_path_spec(&"foo/bar/baz.txt".into(), true).is_ok());
    assert_eq!(
        check_path_spec(&"foo:bar".into(), true).unwrap_err(),
        "Path spec `foo:bar` contains illegal character `:`."
    );
    assert_eq!(
        check_path_spec(&"foo\\bar".into(), true).unwrap_err(),
        "Path spec `foo\\bar` contains illegal character `\\`."
    );
    for ch in ['<', '>', '"', '|', '?', '*', '\x01', '\x1f'] {
        let spec: PklString = format!("bad{ch}name").into();
        let err = check_path_spec(&spec, true).unwrap_err();
        assert_eq!(
            err,
            format!("Path spec `bad{ch}name` contains illegal character `{ch}`.")
        );
    }
}

#[test]
fn cli_multiple_file_output_writes_files_and_prints_relative_paths() {
    let dir = TempDir::new("basic");
    dir.write(
        "test.pkl",
        r#"
output {
  files {
    ["foo.pcf"] {
      value = new Dynamic {
        ["bar"] = "baz"
      }
    }
    ["bar/baz.pcf"] {
      value = new Dynamic {
        ["baz"] = "biz"
      }
    }
    ["bar/../bark.txt"] {
      text = "bark"
    }
    ["buz.bin"] {
      bytes = Bytes(0, 1, 255)
    }
  }
}
"#,
    );

    let res = run_cli(
        dir.path(),
        &[
            "eval",
            "--no-project",
            "--color=never",
            "-m",
            ".my-output/",
            "test.pkl",
        ],
    );
    assert_eq!(res.exit_code, 0, "{}", String::from_utf8_lossy(&res.stderr));
    assert!(res.stderr.is_empty());
    assert_eq!(
        String::from_utf8(res.stdout).unwrap(),
        ".my-output/foo.pcf\n.my-output/bar/baz.pcf\n.my-output/bark.txt\n.my-output/buz.bin\n"
    );
    assert_eq!(
        fs::read_to_string(dir.path().join(".my-output/foo.pcf")).unwrap(),
        "\n[\"bar\"] = \"baz\"\n"
    );
    assert_eq!(
        fs::read_to_string(dir.path().join(".my-output/bar/baz.pcf")).unwrap(),
        "\n[\"baz\"] = \"biz\"\n"
    );
    assert_eq!(
        fs::read_to_string(dir.path().join(".my-output/bark.txt")).unwrap(),
        "bark"
    );
    assert_eq!(
        fs::read(dir.path().join(".my-output/buz.bin")).unwrap(),
        vec![0, 1, 255]
    );

    // Multi-module into the same `-m` directory (unsorted argv `test1.pkl test0.pkl`)
    let multi_dir = TempDir::new("multi-mod");
    multi_dir.write(
        "test0.pkl",
        "output { files { [\"foo.pcf\"] { text = \"from0\" } } }\n",
    );
    multi_dir.write(
        "test1.pkl",
        "output { files { [\"bar.pcf\"] { text = \"from1\" } } }\n",
    );
    let res = run_cli(
        multi_dir.path(),
        &[
            "eval",
            "--no-project",
            "--color=never",
            "--multiple-file-output-path=.",
            "test1.pkl",
            "test0.pkl",
        ],
    );
    assert_eq!(res.exit_code, 0, "{}", String::from_utf8_lossy(&res.stderr));
    assert_eq!(String::from_utf8(res.stdout).unwrap(), "foo.pcf\nbar.pcf\n");
    assert_eq!(
        fs::read_to_string(multi_dir.path().join("foo.pcf")).unwrap(),
        "from0"
    );
    assert_eq!(
        fs::read_to_string(multi_dir.path().join("bar.pcf")).unwrap(),
        "from1"
    );
}

#[test]
fn cli_multiple_file_output_null_files_creates_nothing_and_outputs_nothing() {
    let dir = TempDir::new("null-files");
    dir.write("test.pkl", "x = 1\n");
    let res = run_cli(
        dir.path(),
        &[
            "eval",
            "--no-project",
            "--color=never",
            "-m",
            ".output",
            "test.pkl",
        ],
    );
    assert_eq!(res.exit_code, 0, "{}", String::from_utf8_lossy(&res.stderr));
    assert!(res.stdout.is_empty());
    assert!(res.stderr.is_empty());
    assert!(!dir.path().join(".output").exists());
}

#[test]
fn cli_multiple_file_output_validation_escapes_collisions_and_directories() {
    // 1. Output dir exists and is a regular file
    let dir = TempDir::new("validation");
    dir.write("notadir", "regular file");
    dir.write(
        "ok.pkl",
        "output { files { [\"a.txt\"] { text = \"a\" } } }\n",
    );
    let res = run_cli(
        dir.path(),
        &[
            "eval",
            "--no-project",
            "--color=never",
            "-m",
            "notadir",
            "ok.pkl",
        ],
    );
    assert_eq!(res.exit_code, 1);
    assert!(res.stdout.is_empty());
    assert_eq!(
        String::from_utf8(res.stderr).unwrap(),
        format!(
            "Output path `{}` exists and is not a directory.\n",
            dir.path().join("notadir").display()
        )
    );

    // 2. Escape via `..` checked BEFORE evaluating FileOutput.bytes (throws inside text must not run)
    dir.write(
        "escape.pkl",
        "output { files { [\"../esc.txt\"] { text = throw(\"must not run\") } } }\n",
    );
    let res = run_cli(
        dir.path(),
        &[
            "eval",
            "--no-project",
            "--color=never",
            "-m",
            ".out_esc",
            "escape.pkl",
        ],
    );
    assert_eq!(res.exit_code, 1);
    assert!(res.stdout.is_empty());
    assert_eq!(
        String::from_utf8(res.stderr).unwrap(),
        format!(
            "Output file conflict: `output.files` entry `\"../esc.txt\"` in module `file://{}` resolves to file path `{}`, which is outside output directory `{}`.\n",
            dir.path().join("escape.pkl").display(),
            dir.path().join("esc.txt").display(),
            dir.path().join(".out_esc").display(),
        )
    );

    // 3. Symlink escape (Unix)
    #[cfg(unix)]
    {
        let out_sym = dir.path().join(".out_sym");
        let outside = dir.path().join("outside");
        fs::create_dir_all(&out_sym).unwrap();
        fs::create_dir_all(&outside).unwrap();
        std::os::unix::fs::symlink(&outside, out_sym.join("link")).unwrap();
        dir.write(
            "sym.pkl",
            "output { files { [\"link/evil.txt\"] { text = \"bar\" } } }\n",
        );
        let res = run_cli(
            dir.path(),
            &[
                "eval",
                "--no-project",
                "--color=never",
                "-m",
                ".out_sym",
                "sym.pkl",
            ],
        );
        assert_eq!(res.exit_code, 1);
        assert!(res.stdout.is_empty());
        assert_eq!(
            String::from_utf8(res.stderr).unwrap(),
            format!(
                "Output file conflict: `output.files` entry `\"link/evil.txt\"` in module `file://{}` resolves to file path `{}`, which is outside output directory `{}`.\n",
                dir.path().join("sym.pkl").display(),
                outside.join("evil.txt").display(),
                out_sym.display(),
            )
        );

        // 3b. Symlinked output directory (`CliEvaluatorTest.kt:1688`)
        let real_out = dir.path().join("real-output");
        fs::create_dir_all(&real_out).unwrap();
        let sym_out = dir.path().join("symlink-output");
        std::os::unix::fs::symlink(&real_out, &sym_out).unwrap();
        dir.write(
            "sym_dir.pkl",
            r#"
output {
  files {
    ["pigeon.json"] {
      value = new { name = "Pigeon" }
      renderer = new JsonRenderer {}
    }
    ["birds/parrot.yaml"] {
      value = new { name = "Parrot" }
      renderer = new YamlRenderer {}
    }
  }
}
"#,
        );
        let res = run_cli(
            dir.path(),
            &[
                "eval",
                "--no-project",
                "--color=never",
                "-m",
                "symlink-output",
                "sym_dir.pkl",
            ],
        );
        assert_eq!(res.exit_code, 0, "{}", String::from_utf8_lossy(&res.stderr));
        assert_eq!(
            String::from_utf8(res.stdout).unwrap(),
            "real-output/pigeon.json\nreal-output/birds/parrot.yaml\n"
        );
        assert_eq!(
            fs::read_to_string(real_out.join("pigeon.json")).unwrap(),
            "{\n  \"name\": \"Pigeon\"\n}\n"
        );
        assert_eq!(
            fs::read_to_string(real_out.join("birds/parrot.yaml")).unwrap(),
            "name: Parrot\n"
        );
    }

    // 4. Leading slash writes inside output dir (matching Java Path.iterator() name elements)
    dir.write(
        "leading_slash.pkl",
        "output { files { [\"/abs.txt\"] { text = \"inside\" } } }\n",
    );
    let res = run_cli(
        dir.path(),
        &[
            "eval",
            "--no-project",
            "--color=never",
            "-m",
            ".out_slash",
            "leading_slash.pkl",
        ],
    );
    assert_eq!(res.exit_code, 0, "{}", String::from_utf8_lossy(&res.stderr));
    assert_eq!(
        String::from_utf8(res.stdout).unwrap(),
        ".out_slash/abs.txt\n"
    );
    assert_eq!(
        fs::read_to_string(dir.path().join(".out_slash/abs.txt")).unwrap(),
        "inside"
    );

    // 5. Intra-module collision (`foo.txt` and `./foo.txt`)
    dir.write(
        "intra.pkl",
        r#"
output {
  files {
    ["foo.txt"] { text = "1" }
    ["./foo.txt"] { text = "2" }
  }
}
"#,
    );
    let res = run_cli(
        dir.path(),
        &[
            "eval",
            "--no-project",
            "--color=never",
            "-m",
            ".out_intra",
            "intra.pkl",
        ],
    );
    assert_eq!(res.exit_code, 1);
    assert_eq!(
        String::from_utf8(res.stdout).unwrap(),
        ".out_intra/foo.txt\n"
    );
    assert_eq!(
        String::from_utf8(res.stderr).unwrap(),
        format!(
            "Output file conflict: `output.files` entries `\"foo.txt\"` in module `file://{}` and `\"./foo.txt\"` in module `file://{}` resolve to the same file path `{}`.\n",
            dir.path().join("intra.pkl").display(),
            dir.path().join("intra.pkl").display(),
            dir.path().join(".out_intra/foo.txt").display(),
        )
    );

    // 6. Cross-module collision (`bar.pkl` and `foo.pkl`)
    dir.write(
        "bar.pkl",
        "output { files { [\"same.txt\"] { text = \"from bar\" } } }\n",
    );
    dir.write(
        "foo.pkl",
        "output { files { [\"same.txt\"] { text = \"from foo\" } } }\n",
    );
    let res = run_cli(
        dir.path(),
        &[
            "eval",
            "--no-project",
            "--color=never",
            "-m",
            ".out_cross",
            "foo.pkl",
            "bar.pkl",
        ],
    );
    assert_eq!(res.exit_code, 1);
    assert_eq!(
        String::from_utf8(res.stdout).unwrap(),
        ".out_cross/same.txt\n"
    );
    assert_eq!(
        String::from_utf8(res.stderr).unwrap(),
        format!(
            "Output file conflict: `output.files` entries `\"same.txt\"` in module `file://{}` and `\"same.txt\"` in module `file://{}` resolve to the same file path `{}`.\n",
            dir.path().join("bar.pkl").display(),
            dir.path().join("foo.pkl").display(),
            dir.path().join(".out_cross/same.txt").display(),
        )
    );

    // 7. Target is a directory (`.` and `myDir`)
    fs::create_dir_all(dir.path().join(".out_dir/myDir")).unwrap();
    dir.write(
        "dot.pkl",
        "output { files { [\".\"] { text = \"bar\" } } }\n",
    );
    dir.write(
        "subdir.pkl",
        "output { files { [\"myDir\"] { text = \"bar\" } } }\n",
    );
    for (file, spec, target) in [
        ("dot.pkl", ".", dir.path().join(".out_dir")),
        ("subdir.pkl", "myDir", dir.path().join(".out_dir/myDir")),
    ] {
        let res = run_cli(
            dir.path(),
            &[
                "eval",
                "--no-project",
                "--color=never",
                "-m",
                ".out_dir",
                file,
            ],
        );
        assert_eq!(res.exit_code, 1);
        assert!(res.stdout.is_empty());
        assert_eq!(
            String::from_utf8(res.stderr).unwrap(),
            format!(
                "Output file conflict: `output.files` entry `\"{spec}\"` in module `file://{}` resolves to file path `{}`, which is a directory.\n",
                dir.path().join(file).display(),
                target.display(),
            )
        );
    }

    // 8. Illegal NUL char in path spec
    dir.write(
        "nul.pkl",
        "output { files { [\"a\\u{0}b\"] { text = \"hi\" } } }\n",
    );
    let res = run_cli(
        dir.path(),
        &[
            "eval",
            "--no-project",
            "--color=never",
            "-m",
            ".out_nul",
            "nul.pkl",
        ],
    );
    assert_eq!(res.exit_code, 1);
    assert!(res.stdout.is_empty());
    assert_eq!(
        res.stderr,
        b"Path spec `a\0b` contains illegal character `\0`.\n"
    );

    // 9. Partial failure: first entry succeeds & writes to stdout, second entry throws in FileOutput.bytes
    dir.write(
        "partial.pkl",
        r#"output {
  files {
    ["ok.txt"] { text = "good" }
    ["bad.txt"] { text = throw("boom") }
  }
}
"#,
    );
    let res = run_cli(
        dir.path(),
        &[
            "eval",
            "--no-project",
            "--color=never",
            "-m",
            ".out_partial",
            "partial.pkl",
        ],
    );
    assert_eq!(res.exit_code, 1);
    assert_eq!(
        String::from_utf8(res.stdout).unwrap(),
        ".out_partial/ok.txt\n"
    );
    assert_eq!(
        fs::read_to_string(dir.path().join(".out_partial/ok.txt")).unwrap(),
        "good"
    );
    let stderr = String::from_utf8(res.stderr).unwrap();
    assert!(stderr.starts_with("–– Pkl Error ––\nboom\n"), "{stderr}");
    assert!(!stderr.contains("repl:text"), "{stderr}");
}

#[test]
fn cli_flags_format_output_path_expression_short_flags_and_mutual_exclusion() {
    let dir = TempDir::new("flags");
    dir.write("a.pkl", "a = 2\n");
    dir.write("b.pkl", "a = 20\n");
    dir.write("z.pkl", "z = 1\n");
    dir.write(
        "empty_yaml.pkl",
        "output { value = List(); renderer = new YamlRenderer { isStream = true } }\n",
    );
    dir.write(
        "person.pkl",
        r#"
class Person {
  name: String
  function toString() = "Person(\(name))"
}
person: Person = new { name = "Frodo" }
nested {
  friend { name = "Bilbo" }
}
propVal = read("prop:greeting")
envVal = read("env:GREET_ENV")
"#,
    );

    // 1. `-f` overrides `-p pkl.outputFormat=...` regardless of flag order
    for args in [
        &[
            "eval",
            "--no-project",
            "--color=never",
            "-p",
            "pkl.outputFormat=yaml",
            "-f",
            "json",
            "a.pkl",
        ][..],
        &[
            "eval",
            "--no-project",
            "--color=never",
            "-f",
            "json",
            "-p",
            "pkl.outputFormat=yaml",
            "a.pkl",
        ][..],
    ] {
        let res = run_cli(dir.path(), args);
        assert_eq!(res.exit_code, 0, "{}", String::from_utf8_lossy(&res.stderr));
        assert_eq!(res.stdout, b"{\n  \"a\": 2\n}\n");
    }

    // 1b. Unknown `-f` format surfaces exact `pkl.base` error
    let res = run_cli(
        dir.path(),
        &[
            "eval",
            "--no-project",
            "--color=never",
            "-f",
            "unknown",
            "a.pkl",
        ],
    );
    assert_eq!(res.exit_code, 1);
    assert!(res.stdout.is_empty());
    let unknown_err = String::from_utf8(res.stderr).unwrap();
    assert!(
        unknown_err.contains("Unknown output format: `unknown`. Supported formats are `json`, `jsonnet`, `pcf`, `plist`, `properties`, `textproto`, `xml`, `yaml`, `pkl-binary`."),
        "{unknown_err}"
    );

    // 2. Mutual exclusion `-m` with `-o` and `-m` with `-x`
    for args in [
        &[
            "eval",
            "--no-project",
            "-m",
            "out",
            "-o",
            "file.out",
            "a.pkl",
        ][..],
        &["eval", "--no-project", "-m", "out", "-x", "a", "a.pkl"][..],
    ] {
        let res = run_cli(dir.path(), args);
        assert_eq!(res.exit_code, 1);
        assert!(res.stdout.is_empty());
        assert_eq!(
            String::from_utf8(res.stderr).unwrap(),
            "Usage: pkl eval [<options>] <modules>...\n\nError: invalid value for --multiple-file-output-path: Option is mutually exclusive with -o, --output-path and -x, --expression.\n"
        );
    }

    // 3. Multi-module stdout sorted by URI
    let res = run_cli(
        dir.path(),
        &["eval", "--no-project", "--color=never", "z.pkl", "a.pkl"],
    );
    assert_eq!(res.exit_code, 0);
    assert_eq!(res.stdout, b"a = 2\n---\nz = 1\n");

    // 4. `-o` with placeholders `%{moduleDir}`, `%{moduleName}`, `%{outputFormat}`
    let res = run_cli(
        dir.path(),
        &[
            "eval",
            "--no-project",
            "--color=never",
            "-f",
            "json",
            "-o",
            "gen/%{moduleDir}/%{moduleName}.%{outputFormat}",
            "a.pkl",
        ],
    );
    assert_eq!(res.exit_code, 0, "{}", String::from_utf8_lossy(&res.stderr));
    assert!(res.stdout.is_empty());
    assert_eq!(
        fs::read(dir.path().join("gen/a.json")).unwrap(),
        b"{\n  \"a\": 2\n}\n"
    );

    // 5. `-o` concatenates multi-module output, skipping empty YAML streams, with custom separator
    let res = run_cli(
        dir.path(),
        &[
            "eval",
            "--no-project",
            "--color=never",
            "-f",
            "yaml",
            "-o",
            "combined.yaml",
            "--module-output-separator=//sep",
            "z.pkl",
            "empty_yaml.pkl",
            "a.pkl",
        ],
    );
    assert_eq!(res.exit_code, 0, "{}", String::from_utf8_lossy(&res.stderr));
    assert_eq!(
        fs::read(dir.path().join("combined.yaml")).unwrap(),
        b"a: 2\n//sep\nz: 1\n"
    );

    // 6. `-o` rejects existing directory
    fs::create_dir_all(dir.path().join("existing_dir")).unwrap();
    let res = run_cli(
        dir.path(),
        &[
            "eval",
            "--no-project",
            "--color=never",
            "-o",
            "existing_dir",
            "a.pkl",
        ],
    );
    assert_eq!(res.exit_code, 1);
    assert_eq!(
        String::from_utf8(res.stderr).unwrap(),
        format!(
            "Output file `{}` is a directory. Did you mean `--multiple-file-output-path`?\n",
            dir.path().join("existing_dir").display()
        )
    );

    // 7. `-x` expressions (arithmetic, custom toString, nested Dynamic, output.text, output.bytes, multi-module, -p/-e)
    let res = run_cli(
        dir.path(),
        &[
            "eval",
            "--no-project",
            "--color=never",
            "-x",
            "a + 10",
            "a.pkl",
        ],
    );
    assert_eq!(res.exit_code, 0, "{}", String::from_utf8_lossy(&res.stderr));
    assert_eq!(res.stdout, b"12");

    let res = run_cli(
        dir.path(),
        &[
            "eval",
            "--no-project",
            "--color=never",
            "-x",
            "a",
            "b.pkl",
            "a.pkl",
        ],
    );
    assert_eq!(res.exit_code, 0);
    assert_eq!(res.stdout, b"2---\n20");

    let res = run_cli(
        dir.path(),
        &[
            "eval",
            "--no-project",
            "--color=never",
            "-x",
            "output.text",
            "a.pkl",
        ],
    );
    assert_eq!(res.exit_code, 0);
    assert_eq!(res.stdout, b"a = 2\n");

    let res = run_cli(
        dir.path(),
        &[
            "eval",
            "--no-project",
            "--color=never",
            "-x",
            "output.bytes",
            "a.pkl",
        ],
    );
    assert_eq!(res.exit_code, 0, "{}", String::from_utf8_lossy(&res.stderr));
    assert_eq!(res.stdout, b"Bytes(97, 32, 61, 32, 50, 10)");

    let res = run_cli(
        dir.path(),
        &[
            "eval",
            "--no-project",
            "--color=never",
            "-p",
            "greeting=hello",
            "-e",
            "GREET_ENV=world",
            "-x",
            "\"\\(person)|\\(nested)|\\(propVal)|\\(envVal)\"",
            "--",
            "person.pkl",
        ],
    );
    assert_eq!(res.exit_code, 0, "{}", String::from_utf8_lossy(&res.stderr));
    assert_eq!(
        res.stdout,
        b"Person(Frodo)|new Dynamic { friend { name = \"Bilbo\" } }|hello|world"
    );
}

#[test]
fn cli_platform_metadata_wiring_works_for_pkl_platform_and_release() {
    let dir = TempDir::new("platform");
    dir.write(
        "plat.pkl",
        r#"
import "pkl:platform"
import "pkl:release"

osName = platform.current.operatingSystem.name
arch = platform.current.processor.architecture
relVersion = "\(release.current.version.major).\(release.current.version.minor).\(release.current.version.patch)"
"#,
    );
    let res = run_cli(
        dir.path(),
        &[
            "eval",
            "--no-project",
            "--color=never",
            "-f",
            "json",
            "plat.pkl",
        ],
    );
    assert_eq!(res.exit_code, 0, "{}", String::from_utf8_lossy(&res.stderr));
    let out = String::from_utf8(res.stdout).unwrap();
    assert!(out.contains("\"relVersion\": \"0.32.0\""), "{out}");
    assert!(
        out.contains(&format!("\"arch\": \"{}\"", std::env::consts::ARCH)),
        "{out}"
    );
    #[cfg(target_os = "macos")]
    assert!(out.contains("\"osName\": \"macOS\""), "{out}");
    #[cfg(target_os = "linux")]
    assert!(out.contains("\"osName\": \"Linux\""), "{out}");
}
