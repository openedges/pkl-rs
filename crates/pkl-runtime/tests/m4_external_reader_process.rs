//! Native OS subprocess integration tests for Pkl 0.32.0 external module and
//! resource readers (`NativeExternalReaderProcess` + `ExternalReaderRegistry`
//! + `external_reader_fixture` binary).
//!
//! Strictly gated `#[cfg(not(target_arch = "wasm32"))]`.

#![cfg(not(target_arch = "wasm32"))]

use pkl_runtime::evaluation::frontend::{
    ModuleEvalOptions, ModuleSourceInput, evaluate_module_output_with_glob_host,
};
use pkl_runtime::external_reader::{
    ExternalReaderConfig, ExternalReaderError, ExternalReaderProcessHandle, ExternalReaderRegistry,
    ExternalReaderTransport, NativeExternalReaderProcess,
};
use pkl_runtime::glob::configured_or_unsupported_completion;
use pkl_runtime::modules::{ModuleCompletion, ModuleHostError, default_or_unsupported_completion};
use pkl_runtime::value::PklString;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

fn fixture_binary() -> &'static Path {
    static BIN: OnceLock<PathBuf> = OnceLock::new();
    BIN.get_or_init(|| {
        let exe = std::env::current_exe().expect("current_exe");
        let deps_dir = exe.parent().expect("deps dir");
        let profile_dir = deps_dir.parent().expect("profile dir");
        let bin_name = if cfg!(windows) {
            "external_reader_fixture.exe"
        } else {
            "external_reader_fixture"
        };
        let candidate = profile_dir.join("examples").join(bin_name);
        if candidate.is_file() {
            return candidate;
        }
        let mut cmd = Command::new(env!("CARGO"));
        cmd.args([
            "build",
            "-p",
            "pkl-runtime",
            "--example",
            "external_reader_fixture",
            "--locked",
        ]);
        if profile_dir.file_name().is_some_and(|n| n == "release") {
            cmd.arg("--release");
        }
        let status = cmd.status().expect("cargo build external_reader_fixture");
        assert!(
            status.success(),
            "failed to build external_reader_fixture example"
        );
        assert!(
            candidate.is_file(),
            "built fixture missing at {}",
            candidate.display()
        );
        candidate
    })
    .as_path()
}

fn temp_dir(label: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "pkl-rs-ext-reader-{label}-{}-{nanos}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn eval_with_native_registry(
    source: &str,
    registry: &ExternalReaderRegistry,
    allowed_schemes: &[&str],
) -> Result<String, String> {
    let allowed: Vec<String> = allowed_schemes.iter().map(|s| format!("{s}:")).collect();
    let check_mod = |target: &PklString| -> Result<(), ModuleHostError> {
        let s = target.as_str().unwrap_or("");
        if allowed.iter().any(|p| s.starts_with(p))
            || s.starts_with("file:")
            || s.starts_with("pkl:")
        {
            Ok(())
        } else {
            Err(ModuleHostError::SecurityManagerException {
                message: Some(format!("Refusing to load module `{s}`.").into()),
            })
        }
    };
    let check_res_mod = |target: &PklString| -> Result<(), ModuleHostError> {
        let s = target.as_str().unwrap_or("");
        if allowed.iter().any(|p| s.starts_with(p)) || s.starts_with("file:") {
            Ok(())
        } else {
            Err(ModuleHostError::SecurityManagerException {
                message: Some(format!("Refusing to read resource `{s}`.").into()),
            })
        }
    };
    let check_res = |target: &PklString| -> Result<(), pkl_runtime::host::ReadFailure> {
        let s = target.as_str().unwrap_or("");
        if allowed.iter().any(|p| s.starts_with(p)) || s.starts_with("file:") {
            Ok(())
        } else {
            Err(pkl_runtime::host::ReadFailure::SecurityManagerException {
                message: Some(format!("Refusing to read resource `{s}`.").into()),
            })
        }
    };

    let options = ModuleEvalOptions::default();
    let input = ModuleSourceInput {
        text: source,
        uri: "file:///native_ext_test.pkl",
        inferred_name: "native_ext_test",
    };

    let outcome = evaluate_module_output_with_glob_host(
        input,
        &options,
        |req| -> ModuleCompletion {
            if let Some(comp) = registry.try_complete_module(req, check_mod) {
                return comp;
            }
            default_or_unsupported_completion(req)
        },
        |req| {
            let resolved = req.resolved_uri().ok()?;
            registry.try_complete_read(&resolved, check_res)
        },
        |req| {
            if let Some(comp) = registry.try_complete_glob(req, check_mod, check_res_mod) {
                return Some(comp);
            }
            Some(configured_or_unsupported_completion(req, &[], &[]))
        },
    );

    if outcome.exit_code == 0 {
        Ok(String::from_utf8(outcome.stdout).expect("utf-8"))
    } else {
        Err(String::from_utf8(outcome.stderr).expect("utf-8 stderr"))
    }
}

#[test]
fn lazy_spawn_does_not_launch_unused_reader_process() {
    let cfg = ExternalReaderConfig::new("/nonexistent/definitely-not-a-binary", vec![], None);
    let proc = NativeExternalReaderProcess::new(cfg);
    assert!(!proc.was_spawned());
    assert!(proc.pid().is_none());

    // Closing an unspawned process is an idempotent no-op.
    proc.close();
    proc.close();
    assert!(proc.is_closed());
    assert!(!proc.was_spawned());
}

#[test]
fn real_subprocess_module_resource_glob_stderr_and_shared_pid() {
    let bin = fixture_binary().to_string_lossy().into_owned();
    let marker_dir = temp_dir("graceful");
    let marker = marker_dir.join("closed.marker");

    let cfg = ExternalReaderConfig::new(
        bin,
        vec![
            "--scheme".into(),
            "alpha".into(),
            "--scheme".into(),
            "beta".into(),
            "--mode".into(),
            "tree".into(),
            "--stderr-noise".into(),
            "--close-marker".into(),
            marker.to_string_lossy().into_owned(),
        ],
        None,
    );

    let mut reg = ExternalReaderRegistry::new(0);
    reg.register_module_reader("alpha", cfg.clone());
    reg.register_resource_reader("alpha", cfg.clone());
    reg.register_module_reader("beta", cfg);
    // All three registrations share one OS child process.
    assert_eq!(reg.process_count(), 1);

    let pkl = r#"
import "alpha:/a.pkl" as modA
import "beta:/dir/sub/child.pkl" as child
import* "alpha:/**.pkl" as mods

a = modA.val
parentRes = child.res
modKeys = mods.keys.toList()
rText = read("alpha:/a.txt").text
rMissing = read?("alpha:/does-not-exist.txt")
resKeys = read*("alpha:/**.txt").keys.toList()
"#;
    let out = eval_with_native_registry(pkl, &reg, &["alpha", "beta"])
        .expect("real external reader subprocess evaluation");
    assert!(out.contains("a = \"a\""), "{out}");
    assert!(out.contains("parentRes = \"yes\""), "{out}");
    assert!(
        out.contains(
            "modKeys = List(\"alpha:/a.pkl\", \"alpha:/b.pkl\", \"alpha:/dir/c.pkl\", \"alpha:/dir/sub/d.pkl\")"
        ),
        "{out}"
    );
    assert!(out.contains("rText = \"alpha\""), "{out}");
    assert!(out.contains("rMissing = null"), "{out}");
    assert!(
        out.contains(
            "resKeys = List(\"alpha:/a.txt\", \"alpha:/b.txt\", \"alpha:/dir/c.txt\", \"alpha:/dir/sub/d.txt\")"
        ),
        "{out}"
    );

    // Graceful close sends `CloseExternalProcess` (0x32), fixture writes marker and exits 0.
    reg.close_all();
    assert_eq!(
        std::fs::read_to_string(&marker).expect("close marker written"),
        "closed\n"
    );
    let _ = std::fs::remove_dir_all(&marker_dir);
}

#[test]
fn real_subprocess_working_dir_and_arguments_echo() {
    let bin = fixture_binary().to_string_lossy().into_owned();
    let custom_wd = temp_dir("workdir");
    let canonical_wd = custom_wd
        .canonicalize()
        .unwrap_or_else(|_| custom_wd.clone());

    let cfg = ExternalReaderConfig::new(
        bin,
        vec![
            "--scheme".into(),
            "envcheck".into(),
            "--mode".into(),
            "echo-cwd".into(),
            "--extra-arg".into(),
            "hello world".into(),
        ],
        Some(custom_wd.to_string_lossy().into_owned()),
    );

    let mut reg = ExternalReaderRegistry::new(0);
    reg.register_module_reader("envcheck", cfg.clone());
    reg.register_resource_reader("envcheck", cfg);

    let pkl = r#"
import "envcheck:info" as info
modCwd = info.cwd
modPid = info.pid
modArgs = info.args
resText = read("envcheck:info").text
"#;
    let out = eval_with_native_registry(pkl, &reg, &["envcheck"]).expect("echo-cwd eval");
    let wd_str = canonical_wd.to_string_lossy();
    assert!(
        out.contains(&format!("modCwd = \"{wd_str}\"")),
        "expected workingDir {wd_str} in:\n{out}"
    );
    assert!(out.contains("\"hello world\""), "{out}");
    assert!(out.contains(&format!("cwd={wd_str};")), "{out}");

    // Extract modPid and verify resource reader ran inside the exact same PID.
    let pid_line = out
        .lines()
        .find(|l| l.starts_with("modPid = "))
        .expect("modPid line");
    let pid = pid_line.trim_start_matches("modPid = ").trim();
    assert!(
        out.contains(&format!("pid={pid};")),
        "module & resource readers must share one PID ({pid}):\n{out}"
    );

    reg.close_all();
    let _ = std::fs::remove_dir_all(&custom_wd);
}

#[test]
fn real_subprocess_missing_executable_reports_executable_path() {
    let missing = "/nonexistent/pkl-external-reader-binary-12345";
    let mut reg = ExternalReaderRegistry::new(0);
    reg.register_module_reader("bad", ExternalReaderConfig::new(missing, vec![], None));

    let err = eval_with_native_registry(r#"import "bad:/m.pkl""#, &reg, &["bad"]).unwrap_err();
    assert!(err.contains(missing), "expected missing path in:\n{err}");
}

#[test]
fn real_subprocess_crash_and_corrupt_stdout_fail_fast_without_hanging() {
    let bin = fixture_binary().to_string_lossy().into_owned();

    // 1. Immediate crash on startup -> fails promptly with terminated error.
    let crash_proc = Arc::new(NativeExternalReaderProcess::with_timeouts(
        ExternalReaderConfig::new(
            bin.clone(),
            vec!["--mode".into(), "crash-immediately".into()],
            None,
        ),
        Duration::from_millis(300),
        Some(Duration::from_secs(5)),
    ));
    let handle = ExternalReaderProcessHandle::new(crash_proc);
    let err = handle.get_module_reader_spec("foo").unwrap_err();
    assert!(
        err.to_string().contains("terminated") || err.to_string().contains("Broken pipe"),
        "expected fast termination error, got: {err}"
    );
    handle.close();

    // 2. Crash mid-read -> Initialize succeeds, ReadModule fails immediately (does NOT hang).
    let crash_read = Arc::new(NativeExternalReaderProcess::with_timeouts(
        ExternalReaderConfig::new(
            bin.clone(),
            vec!["--mode".into(), "crash-on-read".into()],
            None,
        ),
        Duration::from_millis(300),
        Some(Duration::from_secs(5)),
    ));
    let handle2 = ExternalReaderProcessHandle::new(crash_read);
    let spec = handle2
        .get_module_reader_spec("foo")
        .expect("init succeeds")
        .expect("spec present");
    assert_eq!(spec.scheme, "foo");
    let resolver = handle2.module_resolver(0).expect("resolver");
    let started = Instant::now();
    let read_err = resolver.resolve_module("foo:bar").unwrap_err();
    assert!(
        started.elapsed() < Duration::from_secs(3),
        "crash-on-read should fail fast, took {:?}",
        started.elapsed()
    );
    assert!(
        read_err.to_string().contains("terminated"),
        "expected terminated error: {read_err}"
    );
    handle2.close();

    // 3. Corrupt stdout bytes -> DecodeException fail-fast
    let corrupt = Arc::new(NativeExternalReaderProcess::with_timeouts(
        ExternalReaderConfig::new(bin, vec!["--mode".into(), "corrupt-stdout".into()], None),
        Duration::from_millis(300),
        Some(Duration::from_secs(5)),
    ));
    let handle3 = ExternalReaderProcessHandle::new(corrupt);
    let corrupt_err = handle3.get_module_reader_spec("foo").unwrap_err();
    assert!(
        corrupt_err.to_string().contains("Malformed message header"),
        "expected decode error: {corrupt_err}"
    );
    handle3.close();
}

#[test]
fn real_subprocess_hang_on_close_is_force_killed_and_reaped() {
    let bin = fixture_binary().to_string_lossy().into_owned();
    let proc = Arc::new(NativeExternalReaderProcess::with_timeouts(
        ExternalReaderConfig::new(bin, vec!["--mode".into(), "hang-on-close".into()], None),
        Duration::from_millis(150),
        Some(Duration::from_secs(5)),
    ));
    let handle =
        ExternalReaderProcessHandle::new(Arc::clone(&proc) as Arc<dyn ExternalReaderTransport>);
    assert!(handle.get_module_reader_spec("foo").unwrap().is_some());
    assert!(proc.pid().is_some());

    let started = Instant::now();
    handle.close();
    let elapsed = started.elapsed();
    assert!(
        elapsed >= Duration::from_millis(120) && elapsed < Duration::from_secs(3),
        "expected ~150ms graceful wait then force-kill, took {elapsed:?}"
    );
    assert!(proc.is_closed());
    // Post-close requests fail immediately with Closed.
    assert!(matches!(
        proc.ensure_alive(),
        Err(ExternalReaderError::Closed)
    ));
}

#[test]
fn real_subprocess_hang_on_read_unblocks_on_cancel_and_timeout() {
    let bin = fixture_binary().to_string_lossy().into_owned();

    // 1. Per-request timeout unblocks promptly and Drop kills + reaps child.
    {
        let proc = Arc::new(NativeExternalReaderProcess::with_timeouts(
            ExternalReaderConfig::new(
                bin.clone(),
                vec!["--mode".into(), "hang-on-read".into()],
                None,
            ),
            Duration::from_millis(100),
            Some(Duration::from_millis(1000)),
        ));
        proc.ensure_alive().expect("spawn fixture");
        let handle =
            ExternalReaderProcessHandle::new(Arc::clone(&proc) as Arc<dyn ExternalReaderTransport>);
        assert!(handle.get_module_reader_spec("foo").unwrap().is_some());
        let resolver = handle.module_resolver(0).unwrap();
        let started = Instant::now();
        let err = resolver.resolve_module("foo:hang").unwrap_err();
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "timeout should fire quickly, took {:?}",
            started.elapsed()
        );
        assert!(err.to_string().contains("timed out"), "{err}");
        // Drop triggers close() -> kill + reap.
    }

    // 2. Concurrent `cancel()` from another thread unblocks an in-flight read immediately.
    let proc = Arc::new(NativeExternalReaderProcess::with_timeouts(
        ExternalReaderConfig::new(bin, vec!["--mode".into(), "hang-on-read".into()], None),
        Duration::from_millis(100),
        None,
    ));
    let handle = Arc::new(ExternalReaderProcessHandle::new(
        Arc::clone(&proc) as Arc<dyn ExternalReaderTransport>
    ));
    assert!(handle.get_module_reader_spec("foo").unwrap().is_some());

    let handle_cancel = Arc::clone(&handle);
    let cancel_thread = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(120));
        handle_cancel.cancel();
    });

    let resolver = handle.module_resolver(0).unwrap();
    let started = Instant::now();
    let err = resolver.resolve_module("foo:hang").unwrap_err();
    let elapsed = started.elapsed();
    cancel_thread.join().expect("cancel thread");
    assert!(
        elapsed < Duration::from_secs(2),
        "cancel should unblock quickly, took {elapsed:?}"
    );
    assert!(err.to_string().contains("cancelled"), "{err}");
    assert!(handle.is_closed());
}

#[test]
fn real_subprocess_cancel_kills_promptly_without_waiting_default_close_timeout() {
    let bin = fixture_binary().to_string_lossy().into_owned();
    // Use `NativeExternalReaderProcess::new` so `close_timeout` is the full
    // `DEFAULT_CLOSE_TIMEOUT` (3 seconds).
    let proc = Arc::new(NativeExternalReaderProcess::new(ExternalReaderConfig::new(
        bin,
        vec!["--mode".into(), "hang-on-read".into()],
        None,
    )));
    let handle = Arc::new(ExternalReaderProcessHandle::new(
        Arc::clone(&proc) as Arc<dyn ExternalReaderTransport>
    ));
    assert!(handle.get_module_reader_spec("foo").unwrap().is_some());

    let handle_cancel = Arc::clone(&handle);
    let cancel_thread = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(100));
        let cancel_started = Instant::now();
        handle_cancel.cancel();
        cancel_started.elapsed()
    });

    let resolver = handle.module_resolver(0).unwrap();
    let started = Instant::now();
    let err = resolver.resolve_module("foo:hang").unwrap_err();
    let read_elapsed = started.elapsed();
    let cancel_elapsed = cancel_thread.join().expect("cancel thread");

    assert!(err.to_string().contains("cancelled"), "{err}");
    assert!(
        cancel_elapsed < Duration::from_secs(1),
        "cancel() must kill+reap promptly instead of blocking for DEFAULT_CLOSE_TIMEOUT (3s); took {cancel_elapsed:?}"
    );
    assert!(
        read_elapsed < Duration::from_secs(1),
        "in-flight read must unblock promptly on cancel(); took {read_elapsed:?}"
    );
    assert!(handle.is_closed());
    assert!(matches!(
        proc.ensure_alive(),
        Err(ExternalReaderError::Cancelled)
    ));
}

#[test]
fn real_subprocess_shared_process_concurrent_resolvers_have_unique_request_ids() {
    let bin = fixture_binary().to_string_lossy().into_owned();
    let proc = Arc::new(NativeExternalReaderProcess::new(ExternalReaderConfig::new(
        bin,
        vec![
            "--scheme".into(),
            "alpha".into(),
            "--scheme".into(),
            "beta".into(),
            "--mode".into(),
            "tree".into(),
        ],
        None,
    )));
    let handle = Arc::new(ExternalReaderProcessHandle::new(
        Arc::clone(&proc) as Arc<dyn ExternalReaderTransport>
    ));
    assert!(handle.get_module_reader_spec("alpha").unwrap().is_some());
    assert!(handle.get_module_reader_spec("beta").unwrap().is_some());
    assert!(handle.get_resource_reader_spec("alpha").unwrap().is_some());

    // Multiple resolvers sharing one ExternalReaderProcessHandle fired concurrently
    // across threads must not collide in `SharedState.pending`.
    let mod_alpha = Arc::new(handle.module_resolver(1).unwrap());
    let mod_beta = Arc::new(handle.module_resolver(2).unwrap());
    let res_alpha = Arc::new(handle.resource_resolver(3).unwrap());

    let t1 = {
        let r = Arc::clone(&mod_alpha);
        std::thread::spawn(move || {
            for uri in [
                "alpha:/a.pkl",
                "alpha:/b.pkl",
                "alpha:/dir/c.pkl",
                "alpha:/dir/sub/d.pkl",
            ] {
                let text = r.resolve_module(uri).expect("alpha module read");
                assert!(text.contains("val ="), "unexpected {uri}: {text}");
            }
        })
    };
    let t2 = {
        let r = Arc::clone(&mod_beta);
        std::thread::spawn(move || {
            for uri in [
                "beta:/a.pkl",
                "beta:/b.pkl",
                "beta:/dir/c.pkl",
                "beta:/dir/sub/d.pkl",
            ] {
                let text = r.resolve_module(uri).expect("beta module read");
                assert!(text.contains("val ="), "unexpected {uri}: {text}");
            }
        })
    };
    let t3 = {
        let r = Arc::clone(&res_alpha);
        std::thread::spawn(move || {
            for (uri, expected) in [
                ("alpha:/a.txt", "alpha"),
                ("alpha:/b.txt", "beta"),
                ("alpha:/dir/c.txt", "gamma"),
                ("alpha:/dir/sub/d.txt", "delta"),
            ] {
                let bytes = r.read(uri).expect("alpha resource read").expect("present");
                assert_eq!(bytes, expected.as_bytes());
            }
        })
    };

    t1.join().expect("mod_alpha thread");
    t2.join().expect("mod_beta thread");
    t3.join().expect("res_alpha thread");
    handle.close();
}

#[test]
fn real_subprocess_bare_executable_resolved_via_path_and_project_relative_resolution() {
    let bin = fixture_binary();
    let bin_dir = bin.parent().expect("examples dir");
    let bin_name = bin.file_name().expect("fixture filename").to_string_lossy();

    // 1. Bare executable name resolved off PATH without mutating global process env
    let proc = Arc::new(
        NativeExternalReaderProcess::new(ExternalReaderConfig::new(
            bin_name.into_owned(),
            vec!["--scheme".into(), "pathscheme".into()],
            None,
        ))
        .with_env_override("PATH", bin_dir.to_string_lossy().into_owned()),
    );
    let handle = ExternalReaderProcessHandle::new(proc);
    let spec = handle
        .get_resource_reader_spec("pathscheme")
        .expect("PATH init")
        .expect("spec present");
    assert_eq!(spec.scheme, "pathscheme");
    let resolver = handle.resource_resolver(0).expect("resolver");
    let bytes = resolver
        .read("pathscheme:hello")
        .expect("read")
        .expect("present");
    assert_eq!(bytes, b"hello:pathscheme:hello");
    handle.close();

    // 2. End-to-end: Pkl `EvaluatorSettings.resolveForOs` resolves relative executable
    //    and default `workingDir = null` against a project directory, then native runner spawns it.
    let proj_dir = temp_dir("proj-rel");
    let canonical_proj = proj_dir.canonicalize().unwrap_or_else(|_| proj_dir.clone());
    let tools_dir = canonical_proj.join("tools");
    std::fs::create_dir_all(&tools_dir).expect("create tools dir");
    let copied_bin = tools_dir.join(if cfg!(windows) {
        "my-reader.exe"
    } else {
        "my-reader"
    });
    std::fs::copy(bin, &copied_bin).expect("copy fixture binary");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::metadata(bin).expect("bin meta").permissions();
        std::fs::set_permissions(&copied_bin, PermissionsExt::from_mode(perms.mode()))
            .expect("chmod fixture copy");
    }

    let proj_file_uri =
        pkl_runtime::packages::resolver::file_uri_from_path(&canonical_proj.join("PklProject"))
            .expect("project file URI");
    let resolve_pkl = format!(
        r#"
import "pkl:EvaluatorSettings"
import "pkl:platform"

local os = new platform.OperatingSystem {{ name = "{}" }}
local s: EvaluatorSettings = new {{
  externalResourceReaders {{
    ["projext"] {{
      executable = "tools/{}"
    }}
  }}
}}
local r = s.resolveForOs("{proj_file_uri}", os)
exe = r.externalResourceReaders!!["projext"].executable
wd = r.externalResourceReaders!!["projext"].workingDir
"#,
        if cfg!(windows) { "Windows" } else { "macOS" },
        if cfg!(windows) {
            "my-reader.exe"
        } else {
            "my-reader"
        }
    );
    let empty_reg = ExternalReaderRegistry::new(0);
    let resolved_pcf = eval_with_native_registry(&resolve_pkl, &empty_reg, &[])
        .expect("resolve EvaluatorSettings");
    let extract = |key: &str| -> String {
        let prefix = format!("{key} = \"");
        let line = resolved_pcf
            .lines()
            .find(|l| l.starts_with(&prefix))
            .unwrap_or_else(|| panic!("missing {key} in:\n{resolved_pcf}"));
        line[prefix.len()..line.len() - 1].replace("\\\\", "\\")
    };
    let resolved_exe = extract("exe");
    let resolved_wd = extract("wd");
    let expected_wd = canonical_proj.to_string_lossy();
    let expected_wd = expected_wd.strip_prefix(r"\\?\").unwrap_or(&expected_wd);
    assert_eq!(resolved_wd, expected_wd);

    let mut reg = ExternalReaderRegistry::new(0);
    reg.register_resource_reader(
        "projext",
        ExternalReaderConfig::new(
            resolved_exe,
            vec![
                "--scheme".into(),
                "projext".into(),
                "--mode".into(),
                "echo-cwd".into(),
            ],
            Some(resolved_wd.clone()),
        ),
    );
    let out = eval_with_native_registry(r#"info = read("projext:check").text"#, &reg, &["projext"])
        .expect("spawn resolved project reader");
    assert!(
        out.contains(&format!("cwd={resolved_wd};")),
        "expected reader cwd={resolved_wd} in:\n{out}"
    );
    reg.close_all();
    let _ = std::fs::remove_dir_all(&proj_dir);
}
