//! Source-derived settings/module invariants. No Java/native execution is claimed.
use pkl_runtime::{
    evaluation::{
        EvaluationOptions,
        session::{SourceSession, SourceSessionOptions},
    },
    host::HostPoll,
    modules::{ModuleRequestKind, default_completion},
    value::{ErrorKind, RuntimeError, Value},
};
fn session(source: &str, paths: &[&str], power: bool) -> SourceSession {
    SourceSession::compile(
        source,
        "file:///stage149-settings/main.pkl",
        "main",
        &paths
            .iter()
            .map(|path| (*path).to_owned())
            .collect::<Vec<_>>(),
        SourceSessionOptions {
            evaluation: EvaluationOptions {
                power_assertions: power,
            },
            ..Default::default()
        },
    )
    .unwrap()
}
fn run(session: &mut SourceSession, index: usize, collect: bool) -> Result<Value, RuntimeError> {
    session.start_path(index, 0, None).unwrap();
    for _ in 0..100_000 {
        match session.poll(0, 512) {
            HostPoll::Yielded => {
                if collect {
                    session.collect(&[]);
                }
            }
            HostPoll::Module { token, request } => {
                assert!(!matches!(
                    request.kind,
                    ModuleRequestKind::LoadSource { .. }
                ));
                let completion = default_completion(&request).expect("bundled settings dependency");
                session.complete_module(&token, completion, 0).unwrap();
                if collect {
                    session.collect(&[]);
                }
            }
            HostPoll::Ready(result) => return result,
            other => panic!("unexpected settings Host effect: {other:?}"),
        }
    }
    panic!("settings source control did not finish");
}
fn text(value: Value, expected: &str) {
    let Value::String(value) = value else {
        panic!("stringified query result")
    };
    assert_eq!(
        value.encode_utf16().collect::<Vec<_>>(),
        expected.encode_utf16().collect::<Vec<_>>()
    );
}
#[test]
fn explicit_os_resolution_does_not_read_platform_current() {
    let source = r#"import "pkl:EvaluatorSettings" as S
import "pkl:platform"
local posix = new platform.OperatingSystem { name = "macOS" }
local windows = new platform.OperatingSystem { name = "Windows" }
a = new S { rootDir = "../other" }.resolveForOs("file:///work/project/PklProject", posix).rootDir
b = new S { rootDir = "../other" }.resolveForOs("file:///C:/work/project/PklProject", windows).rootDir
c = new S { rootDir = "/absolute" }.resolveForOs("file:///work/project/PklProject", posix).rootDir
"#;
    for power in [false, true] {
        let mut s = session(source, &["a", "b", "c"], power);
        text(run(&mut s, 0, false).unwrap(), "/work/other");
        text(run(&mut s, 1, false).unwrap(), r"C:\work\other");
        text(run(&mut s, 2, true).unwrap(), "/absolute");
        s.close();
        assert_eq!(s.collect(&[]).slots, 0);
    }
}
#[test]
fn authored_header_constraints_fail_without_poisoning_the_next_query() {
    let source = r#"import "pkl:EvaluatorSettings" as S
bad = new PcfRenderer {}.renderValue(new S.Http { headers { ["**"] { ["Connection"] = "forbidden" } } })
good = new PcfRenderer {}.renderValue(new S.Http { headers { ["**"] { ["X-Good"] = "allowed" } } }).contains("X-Good")
badValue = new PcfRenderer {}.renderValue(new S.Http { headers { ["**"] { ["X-Good"] = "\u{1f643}" } } })
"#;
    let mut s = session(source, &["bad", "good", "badValue"], false);
    for index in [0, 1, 2, 1] {
        if index == 1 {
            text(run(&mut s, index, true).unwrap(), "true");
        } else {
            assert_eq!(run(&mut s, index, true).unwrap_err().kind, ErrorKind::Type);
        }
    }
    s.close();
    assert_eq!(s.collect(&[]).slots, 0);
}
#[test]
fn cached_project_properties_stay_lazy_until_the_requested_query() {
    let source = r#"import "pkl:Project"
import "pkl:platform"
local p = new Project { projectFileUri = "file:///tmp/project/PklProject"; evaluatorSettings { rootDir = "custom" } }
good = p.evaluatorSettings.resolveForOs(p.projectFileUri, new platform.OperatingSystem { name = "macOS" }).rootDir
needsHost = p.resolvedEvaluatorSettings
"#;
    let mut s = session(source, &["good", "needsHost"], false);
    text(run(&mut s, 0, false).unwrap(), "/tmp/project/custom");
    assert_eq!(
        run(&mut s, 1, true).unwrap_err().kind,
        ErrorKind::Unsupported
    );
    text(run(&mut s, 0, true).unwrap(), "/tmp/project/custom");
    s.close();
    assert_eq!(s.collect(&[]).slots, 0);
}
#[test]
fn external_current_requires_host_data_instead_of_fabricated_defaults() {
    let source = r#"import "pkl:platform"
import "pkl:release"
platformValue = platform.current
releaseValue = release.current
authored = new platform.OperatingSystem { name = "Windows"; version = "authored" }.name
"#;
    let mut s = session(
        source,
        &["platformValue", "releaseValue", "authored"],
        false,
    );
    for index in [0, 2, 1, 2] {
        if index == 2 {
            text(run(&mut s, index, true).unwrap(), "Windows");
        } else {
            assert_eq!(
                run(&mut s, index, true).unwrap_err().kind,
                ErrorKind::Unsupported
            );
        }
    }
    s.close();
    assert_eq!(s.collect(&[]).slots, 0);
}
